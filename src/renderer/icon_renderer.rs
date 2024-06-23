use pixels::wgpu::{self, util::DeviceExt, Device, Queue};

use crate::{selection::Bounds, wgpu_text::Matrix};

pub(crate) struct IconRenderer {
    pub format_single_line_icon: Icon,
    pub copy_icon: Icon,

    pub icon_atlas: Vec<u8>,
    pub icon_atlas_width: u32,
    pub icon_atlas_height: u32,

    pub icon_atlas_texture: wgpu::Texture,
    pub icon_atlas_view: wgpu::TextureView,
    pub icon_atlas_sampler: wgpu::Sampler,

    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,

    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,

    pub matrix_buffer: wgpu::Buffer,
}


pub(crate) enum IconBehavior {
    Toggle,
    Click,
}

pub(crate) struct Icon {
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

fn create_texture(device: &Device, icon_atlas_width: u32, icon_atlas_height: u32) -> wgpu::Texture {
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
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: Some("Icon Atlas Texture"),
        view_formats: &[]
    })
}

impl IconRenderer {
    pub fn new(device: &Device) -> Self {
        let icon_atlas = image!("../icons/atlas.png");

        // TODO: This could be stored at build time
        let icon_atlas_height = 512;
        let icon_atlas_width = icon_atlas.len() as u32 / icon_atlas_height / 4;

        let icon_atlas_texture = create_texture(device, icon_atlas_width, icon_atlas_height);
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

        let matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Matrix Buffer"),
            contents: bytemuck::cast_slice(&crate::wgpu_text::ortho(1.0, 1.0)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
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
            usage: wgpu::BufferUsages::VERTEX
        });
        
        let index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Index Buffer"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX
        });
        
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Icon Atlas Instance Buffer"),
            size: 4 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Icon Atlas Bind Group Layout"),
            entries: &[
                // Icon atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None
                },
                // Icon atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None
                },
                // Projection matrix
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<Matrix>() as wgpu::BufferAddress,
                        ),
                    },
                    count: None
                }
            ]
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&icon_atlas_sampler)
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&icon_atlas_view)
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: matrix_buffer.as_entire_binding(),
                },
            ],
            label: Some("Icon Atlas Bind Group")
        });

        let icon_atlas_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Icon Atlas Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[]
        });

        let module = device.create_shader_module(wgpu::include_wgsl!("../shaders/icons.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Icon Atlas Render Pipeline"),
            layout: Some(&icon_atlas_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    // Vertex position
                    array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0
                        }
                    ]
                }, wgpu::VertexBufferLayout {
                    // Icon position
                    array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 1
                        }
                    ]
                }],
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
            icon_atlas_width,
            icon_atlas_height,
            
            icon_atlas_texture,
            icon_atlas_view,
            icon_atlas_sampler,
            bind_group,
            bind_group_layout,
            pipeline,

            vertex_buffer,
            index_buffer,
            instance_buffer,

            matrix_buffer
        }
    }

    pub fn write_icon_atlas(&self, queue: &Queue) {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.icon_atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All
            },
            &self.icon_atlas,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.icon_atlas_width),
                rows_per_image: Some(self.icon_atlas_height),
            },
            wgpu::Extent3d {
                width: self.icon_atlas_width,
                height: self.icon_atlas_height,
                depth_or_array_layers: 1
            }
        );
    }

    pub fn icons(&self) -> Vec<&Icon> {
        vec![&self.format_single_line_icon, &self.copy_icon]
    }
    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        vec![&mut self.format_single_line_icon, &mut self.copy_icon]
    }

    pub fn render<'pass>(&'pass self, rpass: &mut wgpu::RenderPass<'pass>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        // Vertex position
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        // Instance position
        rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        rpass.draw_indexed(0..6, 0, 0..self.icons().len() as u32);
    }

    pub fn click(&mut self, mouse_pos: (i32, i32)) {
        self.icons_mut().iter_mut().for_each(|icon| icon.click(mouse_pos));
    }

    pub fn update(&mut self, queue: &Queue, mouse_pos: (i32, i32)) {
        self.icons_mut().into_iter().for_each(|icon| icon.hovered = icon.bounds.contains(mouse_pos));

        self.update_instance_buffer(queue);
    }

    fn update_instance_buffer(&mut self, queue: &Queue) {
        let instance_data: Vec<f32> = self.icons().iter().flat_map(|icon| {
            let icon_pos = if icon.selected {
                if icon.hovered {
                    icon.icon_selected_hovered_pos
                } else {
                    icon.icon_selected_pos
                }
            } else {
                if icon.hovered {
                    icon.icon_hovered_pos
                } else {
                    icon.icon_normal_pos
                }
            };

            vec![icon.bounds.x as f32, icon.bounds.y as f32, icon_pos as f32, 0.0]
        }).collect();

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instance_data));
    }

    pub fn resize_view(&self, width: f32, height: f32, queue: &wgpu::Queue) {
        self.update_matrix(crate::wgpu_text::ortho(width, height), queue);
    }

    fn update_matrix(&self, matrix: crate::wgpu_text::Matrix, queue: &wgpu::Queue) {
        queue.write_buffer(&self.matrix_buffer, 0, bytemuck::cast_slice(&matrix));
    }
}

impl Icon {
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