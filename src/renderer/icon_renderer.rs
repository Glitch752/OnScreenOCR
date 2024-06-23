use pixels::wgpu::{self, Device, Queue, RenderPass};

use crate::selection::Bounds;

pub(crate) struct IconRenderer {
    pub format_single_line_icon: Icon,
    pub copy_icon: Icon,
    pub icon_atlas: Vec<u8>,

    pub icon_atlas_texture: wgpu::Texture,
    pub icon_atlas_view: wgpu::TextureView,
    pub icon_atlas_sampler: wgpu::Sampler,
    pub icon_atlas_bind_group: wgpu::BindGroup,
    pub icon_atlas_bind_group_layout: wgpu::BindGroupLayout,
    pub icon_atlas_pipeline: wgpu::RenderPipeline,
    pub icon_atlas_vertex_buffer: wgpu::Buffer,
    pub icon_atlas_index_buffer: wgpu::Buffer,
}


pub(crate) enum IconBehavior {
    Toggle,
    Click,
}

pub(crate) struct Icon {
    pub id: &'static str,
    pub hovered: bool,
    pub selected: bool,
    pub bounds: Bounds,
    pub behavior: IconBehavior,
    pub click_callback: Option<Box<dyn Fn() -> ()>>,

    icon_normal_pos: u32,
    icon_hovered_pos: u32,
    icon_selected_pos: u32,
    icon_selected_hovered_pos: u32,
}

macro_rules! image {
    ($path:expr) => {
        {
            let img = image::load_from_memory(include_bytes!($path)).unwrap();
            let raw = img.to_rgba8().into_raw();
            raw
        }
    };
}

static ATLAS_POSITIONS: &str = include_str!("../icons/atlas_positions.txt");

fn get_icon_index(id: &str) -> u32 {
    ATLAS_POSITIONS.lines().find(|line| line.starts_with(id)).unwrap().split_whitespace().last().unwrap().parse().unwrap()
}

macro_rules! create_icon {
    ($id:literal, $behavior:expr) => {
        Icon {
            id: $id,
            hovered: false,
            selected: false,
            bounds: Bounds::default(),
            behavior: $behavior,
            click_callback: None,

            icon_normal_pos: get_icon_index(concat!($id, "")),
            icon_hovered_pos: get_icon_index(concat!($id, "-hover")),
            icon_selected_pos: get_icon_index(concat!($id, "-selected")),
            icon_selected_hovered_pos: get_icon_index(concat!($id, "-selected-hover"))
        }
    };
}

fn create_texture(device: Device, icon_atlas: Vec<u8>, icon_atlas_width: u32, icon_atlas_height: u32) -> wgpu::Texture {
    let icon_atlas_size = wgpu::Extent3d {
        width: icon_atlas_width,
        height: icon_atlas_height,
        depth_or_array_layers: 1
    };

    device.create_texture(&wgpu::TextureDescriptor {
        size: icon_atlas_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
        label: Some("Icon Atlas Texture"),
        view_formats: &[]
    })
}

impl IconRenderer {
    pub fn new(device: Device) -> Self {
        let icon_atlas = image!("../icons/atlas.png");

        // TODO: This could be stored at build time
        let icon_atlas_height = 512;
        let icon_atlas_width = icon_atlas.len() as u32 / icon_atlas_height / 4;

        let icon_atlas_texture = create_texture(device, icon_atlas, icon_atlas_width, icon_atlas_height);
        let icon_atlas_view = icon_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let icon_atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Icon Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None
        });

        let vertex_data: [[f32; 2]; 4] = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0]
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertex_data),
            usage: wgpu::BufferUsage::VERTEX
        });
        
        let index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Index Buffer"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsage::INDEX
        });

        let icon_atlas_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Icon Atlas Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        dimension: wgpu::TextureViewDimension::D2,
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false
                    },
                    count: None
                }
            ]
        });
        let icon_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &icon_atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&icon_atlas_sampler)
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&icon_atlas_view)
                }
            ],
            label: Some("Icon Atlas Bind Group")
        });

        let icon_atlas_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Icon Atlas Pipeline Layout"),
            bind_group_layouts: &[&icon_atlas_bind_group_layout],
            push_constant_ranges: &[]
        });

        let module = device.create_shader_module(&wgpu::include_wgsl!("icons.wgsl"));
        let icon_atlas_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Icon Atlas Render Pipeline"),
            layout: Some(&icon_atlas_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0
                        }
                    ]
                }]
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None
        });

        IconRenderer {
            format_single_line_icon: create_icon!("new-line", IconBehavior::Toggle),
            copy_icon: create_icon!("copy", IconBehavior::Click),
            icon_atlas,
            
            icon_atlas_texture,
            icon_atlas_view,
            icon_atlas_sampler,
            icon_atlas_bind_group,
            icon_atlas_bind_group_layout,
            icon_atlas_pipeline,
            icon_atlas_vertex_buffer,
            icon_atlas_index_buffer,
        }
    }

    pub fn icons(&mut self) -> Vec<&mut Icon> {
        vec![&mut self.format_single_line_icon, &mut self.copy_icon]
    }

    pub fn render(&mut self, rpass: &mut RenderPass) {
        self.icons().iter().for_each(|icon| icon.render(rpass));
    }

    pub fn click(&mut self, mouse_pos: (i32, i32)) {
        self.icons().iter_mut().for_each(|icon| icon.click(mouse_pos));
    }

    pub fn update(&mut self, queue: Queue, mouse_pos: (i32, i32)) {
        self.icons().iter_mut().for_each(|icon| icon.hovered = icon.bounds.contains(mouse_pos));


    }
}

impl Icon {
    pub fn render(&self, rpass: &mut RenderPass) {
        
    }

    pub fn click(&mut self, mouse_pos: (i32, i32)) {
        if self.bounds.contains(mouse_pos) {
            match self.behavior {
                IconBehavior::Toggle => {
                    self.selected = !self.selected;
                }
                IconBehavior::Click => {
                    if let Some(callback) = &self.click_callback {
                        callback();
                    }
                }
            }
        }
    }
}