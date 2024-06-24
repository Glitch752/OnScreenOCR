use pixels::wgpu::{self, util::DeviceExt, Device, Queue};
use winit::event::ElementState;

use crate::{selection::Bounds, wgpu_text::Matrix};

const ICON_SIZE: f32 = 50.0;
const ICON_MARGIN: f32 = 10.0;

static ATLAS_POSITIONS: &str = include_str!("../icons/atlas_positions.txt");

fn get_icon_pos(id: &str) -> u32 {
    let pos = ATLAS_POSITIONS.lines().find(|line| line.starts_with(id)).unwrap().split_whitespace().last().unwrap().parse().unwrap();
    pos
}

macro_rules! create_icon {
    ($id:literal, $behavior:expr, $bounds:expr) => {
        Icon {
            hovered: false,
            selected: false,
            bounds: Bounds::from_center($bounds.0, $bounds.1, ICON_SIZE, ICON_SIZE),
            behavior: $behavior,
            click_callback: None,

            icon_normal_pos: get_icon_pos(concat!($id, ".png")),
            icon_hovered_pos: get_icon_pos(concat!($id, "-hover.png")),
            icon_selected_pos: get_icon_pos(concat!($id, "-selected.png")),
            icon_selected_hovered_pos: get_icon_pos(concat!($id, "-selected-hover.png"))
        }
    };
}

macro_rules! create_background {
    ($bounds:expr) => {
        Icon {
            hovered: false,
            selected: false,
            bounds: Bounds::from_center($bounds.0, $bounds.1, ICON_SIZE + ICON_MARGIN * 1.5, ICON_SIZE + ICON_MARGIN * 1.5),
            behavior: IconBehavior::Visual,
            click_callback: None,

            icon_normal_pos: get_icon_pos("background.png"),
            icon_hovered_pos: get_icon_pos("background.png"),
            icon_selected_pos: get_icon_pos("background.png"),
            icon_selected_hovered_pos: get_icon_pos("background.png")
        }
    };

}

pub(crate) struct IconLayouts {
    layouts: Vec<PositionedLayout>
}

impl IconLayouts {
    pub fn new() -> Self {
        IconLayouts {
            layouts: Vec::new()
        }
    }

    pub fn add_layout(&mut self, center_position: (f32, f32), layout: LayoutChild) {
        self.layouts.push(PositionedLayout::new(center_position, layout));
    }

    pub fn icons(&self) -> Vec<&Icon> {
        self.layouts.iter().flat_map(|sub_layout| sub_layout.icons()).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.layouts.iter_mut().flat_map(|sub_layout| sub_layout.icons_mut()).collect()
    }

    pub fn recalculate_positions(&mut self) -> () {
        for sub_layout in self.layouts.iter_mut() {
            sub_layout.recalculate_positions();
        }
    }
}

pub(crate) struct PositionedLayout {
    // TODO: Change to an object that allows screen-size-relative positioning
    center_position: (f32, f32),
    last_center_position: Option<(f32, f32)>,
    layout: LayoutChild
}

impl PositionedLayout {
    pub fn new(center_position: (f32, f32), layout: LayoutChild) -> Self {
        PositionedLayout {
            center_position,
            last_center_position: None,
            layout
        }
    }

    pub fn icons(&self) -> Vec<&Icon> {
        match &self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons()
        }
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        match &mut self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut()
        }
    }

    pub fn recalculate_positions(&mut self) -> () {
        if Some(self.center_position) == self.last_center_position {
            return;
        }
        self.last_center_position = Some(self.center_position);

        match &mut self.layout {
            LayoutChild::Icon(icon) => {
                icon.bounds.set_center(self.center_position.0, self.center_position.1);
            }
            LayoutChild::Layout(layout) => {
                layout.calculated_position = self.center_position;
                layout.calculate_size();
                layout.calculate_child_positions();
            }
        }
    }
}

#[allow(unused)]
pub(crate) enum Direction {
    Horizontal,
    Vertical
}

#[allow(unused)]
pub(crate) enum CrossJustify {
    Start,
    Center,
    End
}

pub(crate) struct Layout {
    children: Vec<LayoutChild>,
    direction: Direction,
    cross_justify: CrossJustify,
    spacing: f32,

    has_background: bool,
    background_children: Vec<Icon>,

    calculated_position: (f32, f32),
    calculated_size: (f32, f32)
}

pub(crate) enum LayoutChild {
    Icon(Icon),
    Layout(Layout)
}

impl Layout {
    pub fn new(direction: Direction, cross_justify: CrossJustify, spacing: f32, has_background: bool) -> Self {
        Layout {
            children: Vec::new(),
            direction,
            cross_justify,
            spacing,
            has_background,
            background_children: Vec::new(),
            calculated_position: (0.0, 0.0),
            calculated_size: (0.0, 0.0)
        }
    }

    pub fn add_icon(&mut self, icon: Icon) {
        self.children.push(LayoutChild::Icon(icon));
    }

    pub fn icons(&self) -> Vec<&Icon> {
        self.children.iter().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons()
        }).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.children.iter_mut().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut()
        }).collect()
    }

    pub fn calculate_size(&mut self) -> (f32, f32) {
        let mut width = 0.0;
        let mut height = 0.0;
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(icon) => {
                    width += icon.bounds.width as f32;
                    height += icon.bounds.height as f32;
                }
                LayoutChild::Layout(layout) => {
                    let (child_width, child_height) = layout.calculate_size();
                    match self.direction {
                        Direction::Horizontal => {
                            width += child_width;
                            height = height.max(child_height);
                        }
                        Direction::Vertical => {
                            width = width.max(child_width);
                            height += child_height;
                        }
                    }
                }
            }
        }
        self.calculated_size = (width, height);
        (width, height)
    }

    pub fn calculate_child_positions(&mut self) -> () {
        let mut top_left_position = (self.calculated_position.0 - self.calculated_size.0 / 2., self.calculated_position.1 - self.calculated_size.1 / 2.);
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(icon) => {
                    match self.direction {
                        Direction::Horizontal => {
                            match self.cross_justify {
                                CrossJustify::Start => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1);
                                }
                                CrossJustify::Center => {
                                    icon.bounds.set_center(top_left_position.0 + icon.bounds.width as f32 / 2., top_left_position.1 + icon.bounds.height as f32 / 2.);
                                }
                                CrossJustify::End => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1 + self.calculated_size.1 - icon.bounds.height as f32);
                                }
                            }
                            top_left_position.0 += icon.bounds.width as f32 + self.spacing;
                        }
                        Direction::Vertical => {
                            match self.cross_justify {
                                CrossJustify::Start => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1);
                                }
                                CrossJustify::Center => {
                                    icon.bounds.set_center(top_left_position.0 + icon.bounds.width as f32 / 2., top_left_position.1 + icon.bounds.height as f32 / 2.);
                                }
                                CrossJustify::End => {
                                    icon.bounds.set_origin(top_left_position.0 + self.calculated_size.0 - icon.bounds.width as f32, top_left_position.1);
                                }
                            }
                            top_left_position.1 += icon.bounds.height as f32 + self.spacing;
                        }
                    }
                }
                LayoutChild::Layout(layout) => {
                    layout.calculated_position = top_left_position;
                    layout.calculate_child_positions();
                    top_left_position.0 += layout.calculated_size.0 + self.spacing;
                }
            }
        }
        
        if !self.has_background {
            return;
        }

        // There is 1 background child for every icon and 1 for every space between icons
        let icon_children = self.children.iter().filter(|child| matches!(child, LayoutChild::Icon(_))).map(|child| match child {
            LayoutChild::Icon(icon) => icon,
            _ => unreachable!()
        });
        let background_icons_required = icon_children.clone().count() * 2 - 1;
        if self.background_children.len() != background_icons_required {
            if self.background_children.len() < background_icons_required {
                let mut new_background_children = Vec::new();
                for _ in 0..background_icons_required - self.background_children.len() {
                    new_background_children.push(create_background!((0, 0)));
                }
                self.background_children.append(&mut new_background_children);
            } else {
                self.background_children.truncate(background_icons_required);
            }
        }

        let mut last_position: Option<(i32, i32)> = None;
        let mut background_icon_index = 0;
        for icon in icon_children {
            let icon_position = icon.bounds.get_center();
            if let Some(last_position) = last_position {
                let background = self.background_children.get_mut(background_icon_index).unwrap();
                background.bounds.set_center((icon_position.0 + last_position.0) as f32 / 2., (icon_position.1 + last_position.1) as f32 / 2.);
                background_icon_index += 1;
            }
            last_position = Some(icon_position);

            let background = self.background_children.get_mut(background_icon_index).unwrap();
            background.bounds.set_center(icon_position.0 as f32, icon_position.1 as f32);
            background_icon_index += 1;
        }
    }
}

pub(crate) struct IconRenderer {
    pub icons: IconLayouts,

    pub icon_atlas: Vec<u8>,
    pub icon_atlas_width: u32,
    pub icon_atlas_height: u32,

    pub icon_atlas_texture: wgpu::Texture,

    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,

    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_icon_position_buffer: wgpu::Buffer,
    pub instance_icon_state_buffer: wgpu::Buffer,

    pub matrix_buffer: wgpu::Buffer,
}

pub(crate) enum IconBehavior {
    Toggle,
    Click,
    Visual
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
    pub fn new(device: &Device, width: f32, height: f32) -> Self {
        // let icons = vec![
        //     create_background!((width / 2. + ICON_SIZE / 2. + ICON_MARGIN / 3., ICON_SIZE / 2. + ICON_MARGIN * 2.)),
        //     create_background!((width / 2. - ICON_SIZE / 2. - ICON_MARGIN / 3., ICON_SIZE / 2. + ICON_MARGIN * 2.)),
        //     create_background!((width / 2., ICON_SIZE / 2. + ICON_MARGIN * 2.)),
        //     create_icon!("new-line", IconBehavior::Toggle, (width / 2. - ICON_SIZE / 2. - ICON_MARGIN / 3., ICON_SIZE / 2. + ICON_MARGIN * 2.)),
        //     create_icon!("settings", IconBehavior::Click,  (width / 2. + ICON_SIZE / 2. + ICON_MARGIN / 3., ICON_SIZE / 2. + ICON_MARGIN * 2.)),
        //     {
        //         let mut icon = create_icon!("copy", IconBehavior::Click, (0, 0)); // Set on-the-fly
        //         icon.click_callback = Some(Box::new(|| {
        //             println!("Copy clicked!");
        //         }));
        //         icon
        //     },
        // ];
        let mut menubar_layout = Layout::new(Direction::Horizontal, CrossJustify::Center, ICON_MARGIN, true);
        menubar_layout.add_icon(create_icon!("new-line", IconBehavior::Toggle, (0, 0)));
        menubar_layout.add_icon(create_icon!("settings", IconBehavior::Click, (0, 0)));
        menubar_layout.add_icon({
            let mut icon = create_icon!("copy", IconBehavior::Click, (0, 0));
            icon.click_callback = Some(Box::new(|| {
                println!("Copy clicked!");
            }));
            icon
        });

        let mut icon_layouts = IconLayouts::new();
        icon_layouts.add_layout((width / 2., 100.), LayoutChild::Layout(menubar_layout));

        let icon_count = icon_layouts.icons().len();
        let icon_variant_count = 13; // Needs to be manually defined for now; some icons have multiple variants and some are ued multiple times

        let icon_atlas = image!("../icons/atlas.png");

        // TODO: This could be stored at build time
        let icon_atlas_height = 512;
        let icon_atlas_width = icon_variant_count * 512;

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
            lod_max_clamp: 4.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None
        });

        let matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Matrix Buffer"),
            contents: bytemuck::cast_slice(&crate::wgpu_text::ortho(width, height)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
        });
        let icon_atlas_icons_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Icons Buffer"), // u32
            contents: bytemuck::cast_slice(&[icon_variant_count]),
            usage: wgpu::BufferUsages::UNIFORM
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
        
        let instance_icon_position_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Icon Atlas Instance Position Buffer"),
            size: (icon_count * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false
        });
        let instance_icon_state_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Icon Atlas Instance State Buffer"),
            size: (icon_count * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
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
                },
                // Icon count
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<u32>() as wgpu::BufferAddress,
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: icon_atlas_icons_buffer.as_entire_binding(),
                }
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
                    // Icon position and size (shouldn't change frequently)
                    // x, y, width, height
                    array_stride: 4 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 1
                        }
                    ]
                }, wgpu::VertexBufferLayout {
                    // Icon state (will change frequently)
                    array_stride: std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 0,
                            shader_location: 2
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
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add
                        },
                        alpha: wgpu::BlendComponent::OVER
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None
        });

        IconRenderer {
            icons: icon_layouts,

            icon_atlas,
            icon_atlas_width,
            icon_atlas_height,
            
            icon_atlas_texture,
            bind_group,
            pipeline,

            vertex_buffer,
            index_buffer,
            instance_icon_position_buffer,
            instance_icon_state_buffer,

            matrix_buffer
        }
    }

    pub fn initialize(&mut self, queue: &Queue) {
        // Write the icon atlas to the texture
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

        // Write the icon positions to the instance buffer
        self.update_icon_position_buffer(queue);
    }

    pub fn icons(&self) -> Vec<&Icon> {
        self.icons.icons()
    }
    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.icons.icons_mut()
    }

    pub fn render<'pass>(&'pass self, rpass: &mut wgpu::RenderPass<'pass>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        // Vertex position
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        // Instance position
        rpass.set_vertex_buffer(1, self.instance_icon_position_buffer.slice(..));
        // Instance state
        rpass.set_vertex_buffer(2, self.instance_icon_state_buffer.slice(..));
        rpass.draw_indexed(0..6, 0, 0..self.icons().len() as u32);
    }

    pub fn mouse_event(&mut self, mouse_pos: (i32, i32), state: ElementState) {
        self.icons_mut().iter_mut().for_each(|icon| icon.mouse_event(mouse_pos, state));
    }

    pub fn update(&mut self, queue: &Queue, mouse_pos: (i32, i32)) {
        self.icons.recalculate_positions();

        self.icons_mut().into_iter().for_each(|icon| icon.update(mouse_pos));

        self.update_icon_state_buffer(queue);
        self.update_icon_position_buffer(queue);
    }

    fn update_icon_position_buffer(&mut self, queue: &Queue) {
        let instance_data: Vec<f32> = self.icons().iter().flat_map(|icon| {
            vec![icon.bounds.x as f32, icon.bounds.y as f32, icon.bounds.width as f32, icon.bounds.height as f32]
        }).collect();

        queue.write_buffer(&self.instance_icon_position_buffer, 0, bytemuck::cast_slice(&instance_data));
    }

    fn update_icon_state_buffer(&mut self, queue: &Queue) {
        let instance_data: Vec<f32> = self.icons().iter().map(|icon| {
            let active_icon_pos = match (icon.selected, icon.hovered) {
                (true, true) => icon.icon_selected_hovered_pos,
                (true, false) => icon.icon_selected_pos,
                (false, true) => icon.icon_hovered_pos,
                (false, false) => icon.icon_normal_pos
            };
            active_icon_pos as f32 / self.icon_atlas_width as f32
        }).collect();

        queue.write_buffer(&self.instance_icon_state_buffer, 0, bytemuck::cast_slice(&instance_data));
    }

    pub fn resize_view(&self, width: f32, height: f32, queue: &wgpu::Queue) {
        self.update_matrix(crate::wgpu_text::ortho(width, height), queue);
    }

    fn update_matrix(&self, matrix: crate::wgpu_text::Matrix, queue: &wgpu::Queue) {
        queue.write_buffer(&self.matrix_buffer, 0, bytemuck::cast_slice(&matrix));
    }
}

impl Icon {
    pub fn mouse_event(&mut self, mouse_pos: (i32, i32), state: ElementState) {
        if self.bounds.contains(mouse_pos) {
            match self.behavior {
                IconBehavior::Toggle => {
                    if state == ElementState::Pressed {
                        self.selected = !self.selected;
                    }
                }
                IconBehavior::Click => {
                    self.selected = state == ElementState::Pressed;
                    if let Some(callback) = &self.click_callback {
                        callback();
                    }
                }
                IconBehavior::Visual => {
                    // Doesn't matter
                }
            }
        }
    }

    pub fn update(&mut self, mouse_pos: (i32, i32)) {
        // Update position
        // TODO

        // Update hover
        self.hovered = self.bounds.contains(mouse_pos);
        // If not hovered and a click button, unselect
        if !self.hovered && self.selected && matches!(self.behavior, IconBehavior::Click) {
            self.selected = false;
        }
    }
}