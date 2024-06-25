use std::sync::mpsc;

use glyph_brush::{ab_glyph::FontRef, OwnedSection, OwnedText};
use icon_layout::get_icon_layouts;
use pixels::{wgpu::{self, util::DeviceExt, Device, Queue}, Pixels};
use winit::event::ElementState;

use crate::{selection::Bounds, settings::SettingsManager, wgpu_text::{BrushBuilder, Matrix, TextBrush}};
use super::animation::SmoothMoveFadeAnimation;
use icon_layout_engine::{create_icon, IconLayouts};

pub use icon_layout_engine::TEXT_HEIGHT;
pub use icon_layout::IconEvent;

mod icon_layout_engine;
mod icon_layout;

pub struct IconContext {
    pub settings: SettingsManager,
    pub settings_panel_visible: bool,
    pub copy_key_held: bool,
    pub screenshot_key_held: bool,
    pub has_selection: bool,

    pub(crate) channel: mpsc::Sender<IconEvent>
}

impl IconContext {
    pub fn new(channel: mpsc::Sender<IconEvent>) -> Self {
        Self {
            settings: SettingsManager::new(),
            settings_panel_visible: false,
            copy_key_held: false,
            has_selection: false,
            screenshot_key_held: false,
            channel
        }
    }

    pub fn reset(&mut self) {
        self.settings_panel_visible = false;
    }
}

#[derive(PartialEq)]
pub(crate) enum IconBehavior {
    SettingToggle,
    Click,
    Visual
}

pub(crate) struct Icon {
    pub hovered: bool,
    pub pressed: bool,
    pub active: bool,
    pub disabled: bool,

    pub bounds: Bounds,

    pub visible: bool,
    pub anim: SmoothMoveFadeAnimation,

    pub behavior: IconBehavior,
    pub click_callback: Option<Box<dyn Fn(&mut IconContext) -> ()>>,
    pub get_active: Option<Box<dyn Fn(&IconContext) -> bool>>,
    pub get_disabled: Option<Box<dyn Fn(&IconContext) -> bool>>,

    pub tooltip_text: Option<String>,

    pub(crate) icon_normal_pos: (u32, u32),
    pub(crate) icon_hovered_pos: (u32, u32),
    pub(crate) icon_selected_pos: (u32, u32),
    pub(crate) icon_selected_hovered_pos: (u32, u32)
}

pub(crate) struct TooltipState {
    pub(crate) start_time: std::time::Instant,
    pub(crate) position: (f32, f32),
    pub(crate) text: String,
    pub(crate) hidden: bool,
    pub(crate) disabled_icon: bool
}

impl TooltipState {
    pub fn new(bounds: Bounds, text: String, disabled_icon: bool) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            position: (bounds.x as f32 + bounds.width as f32 / 2., bounds.y as f32 + bounds.height as f32 + 10.),
            text,
            hidden: false,
            disabled_icon
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn should_show(&self) -> bool {
        self.elapsed() > std::time::Duration::from_secs_f32(0.4) && !self.hidden
    }
}

impl Default for TooltipState {
    fn default() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            position: (0., 0.),
            text: String::new(),
            hidden: true,
            disabled_icon: false
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

    pub current_screen_size: (f32, f32),

    pub text_brush: TextBrush<FontRef<'static>>,
    pub should_render_text: bool,

    pub icon_tooltip_state: TooltipState,
    pub icon_tooltip_anim: SmoothMoveFadeAnimation,
    pub icon_tooltip_section: OwnedSection
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
    pub fn new(pixels: &Pixels, width: f32, height: f32) -> Self {
        let device = pixels.device();
        let icon_layouts = get_icon_layouts();

        let icon_count = icon_layouts.icons().len();

        let icon_atlas = image!("../icons/atlas.png");

        let atlas_metadata = include_str!("../icons/atlas_positions.txt").lines().next().expect("Atlas positions file is empty").split_whitespace().collect::<Vec<_>>();
        let atlas_icon_size =
            atlas_metadata.get(0).expect("Atlas metadata doesn't include icon size").parse::<u32>().expect("Unable to parse atlas metadata icon size");
        let icon_atlas_width =
            atlas_metadata.get(1).expect("Atlas metadata doesn't include atlas width").parse::<u32>().expect("Unable to parse atlas metadata atlas width") * atlas_icon_size;
        let icon_atlas_height =
            atlas_metadata.get(2).expect("Atlas metadata doesn't include atlas height").parse::<u32>().expect("Unable to parse atlas metadata atlas height") * atlas_icon_size;

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
            contents: bytemuck::cast_slice(&crate::wgpu_text::ortho(width, height)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
        });
        let icon_atlas_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Icon Atlas Icons Buffer"), // vec2<u32>
            contents: bytemuck::cast_slice(&[icon_atlas_width / atlas_icon_size, icon_atlas_height / atlas_icon_size]),
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
            label: Some("Icon Atlas Instance State Buffer"), // vec3<f32>
            size: (3 * icon_count * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
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
                            // vec2<u32>
                            2 * std::mem::size_of::<u32>() as wgpu::BufferAddress,
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
                    resource: icon_atlas_size_buffer.as_entire_binding(),
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
                    // Icon position and size
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
                    // Icon atlas position, opacity
                    array_stride: 3 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
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
                        // result = operation((src * srcFactor),  (dst * dstFactor))
                        // Where src is value from fragment shader and dst is value already in the texture
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Max,
                        }
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

            matrix_buffer,

            current_screen_size: (width, height),
            
            text_brush: BrushBuilder::using_font_bytes(include_bytes!("../../fonts/DejaVuSans.ttf")).expect("Unable to load font")
                .build(
                    device,
                    width as u32,
                    height as u32,
                    pixels.render_texture_format()
                ),
            should_render_text: false,

            icon_tooltip_state: TooltipState::default(),
            icon_tooltip_anim: SmoothMoveFadeAnimation::new(false, super::animation::MoveDirection::Up, 10.),
            icon_tooltip_section: OwnedSection::default()
                .add_text(OwnedText::new("").with_scale(18.0)) // Color and position are set when updating
                .with_layout(glyph_brush::Layout::default_single_line().h_align(glyph_brush::HorizontalAlign::Center))
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

    pub fn mouse_event(&mut self, mouse_pos: (i32, i32), state: ElementState, icon_context: &mut IconContext) -> bool {
        let mut found = false;
        self.icons_mut().iter_mut().for_each(|icon| found = icon.mouse_event(mouse_pos, state, icon_context) || found);
        found
    }

    pub fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        delta: std::time::Duration,
        mouse_pos: (i32, i32),
        icon_context: &IconContext
    ) {
        self.icons.recalculate_positions(self.current_screen_size);

        let hover_state = self.icons.update_all(mouse_pos, delta, icon_context);
        if let Some(mut state) = hover_state {
            if self.icon_tooltip_state.hidden {
                self.icon_tooltip_state.hidden = false;
                self.icon_tooltip_state.start_time = state.start_time;
            }
            state.start_time = self.icon_tooltip_state.start_time;
            if state.should_show() {
                self.icon_tooltip_state = state;
            }
        } else {
            self.icon_tooltip_state.hidden = true;
        }
        self.icon_tooltip_anim.update(delta, self.icon_tooltip_state.should_show());

        self.update_icon_state_buffer(queue);
        self.update_icon_position_buffer(queue);

        self.icons.set_visible("settings", icon_context.settings_panel_visible);

        // Update text
        let mut sections: Vec<&glyph_brush::OwnedSection> = self.icons.text_sections();
        if self.icon_tooltip_anim.visible_at_all() {
            self.icon_tooltip_section.screen_position = self.icon_tooltip_anim.move_point(self.icon_tooltip_state.position);
            self.icon_tooltip_section.text[0].text = self.icon_tooltip_state.text.clone();
            self.icon_tooltip_section.text[0].extra.color = [1.0, 1.0, 1.0, self.icon_tooltip_anim.get_opacity() * if self.icon_tooltip_state.disabled_icon { 0.5 } else { 1.0 }];
            sections.push(&self.icon_tooltip_section);
        }
        self.should_render_text = sections.len() > 0;
        self.text_brush.queue(device, queue, sections).unwrap();
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

        // Render text
        if self.should_render_text {
            self.text_brush.draw(rpass);
        }
    }

    pub fn update_text_icon_positions(&mut self, pos: Option<(f32, f32)>) {
        if pos.is_none() {
            self.icons.set_visible("copy", false);
            return;
        }
        self.icons.set_visible("copy", true);
        self.icons.set_center("copy", pos.unwrap().0, pos.unwrap().1);
    }

    fn update_icon_position_buffer(&mut self, queue: &Queue) {
        let instance_data: Vec<f32> = self.icons().iter().flat_map(|icon| {
            let pos = icon.anim.move_point((icon.bounds.x as f32, icon.bounds.y as f32));
            vec![pos.0, pos.1, icon.bounds.width as f32, icon.bounds.height as f32]
        }).collect();

        queue.write_buffer(&self.instance_icon_position_buffer, 0, bytemuck::cast_slice(&instance_data));
    }

    fn update_icon_state_buffer(&mut self, queue: &Queue) {
        let instance_data: Vec<f32> = self.icons().iter().flat_map(|icon| {
            let active_icon_pos = match (icon.active, icon.hovered) {
                (true, true) => icon.icon_selected_hovered_pos,
                (true, false) => icon.icon_selected_pos,
                (false, true) => icon.icon_hovered_pos,
                (false, false) => icon.icon_normal_pos
            };
            vec![
                active_icon_pos.0 as f32 / self.icon_atlas_width as f32,
                active_icon_pos.1 as f32 / self.icon_atlas_height as f32,
                icon.anim.get_opacity() * if icon.disabled { 0.5 } else { 1.0 }
            ]
        }).collect();

        queue.write_buffer(&self.instance_icon_state_buffer, 0, bytemuck::cast_slice(&instance_data));
    }

    pub fn resize_view(&mut self, width: f32, height: f32, queue: &wgpu::Queue) {
        self.update_matrix(crate::wgpu_text::ortho(width, height), queue);
        self.text_brush.resize_view(width as f32, height as f32, queue);
        
        self.current_screen_size = (width, height);
    }

    fn update_matrix(&self, matrix: crate::wgpu_text::Matrix, queue: &wgpu::Queue) {
        queue.write_buffer(&self.matrix_buffer, 0, bytemuck::cast_slice(&matrix));
    }
}

impl Icon {
    pub fn mouse_event(&mut self, mouse_pos: (i32, i32), state: ElementState, icon_context: &mut IconContext) -> bool {
        if self.bounds.contains(mouse_pos) && self.visible {
            match self.behavior {
                IconBehavior::Click => {
                    if let Some(callback) = &self.click_callback {
                        if state == ElementState::Released {
                            callback(icon_context);
                        }
                    }
                    self.pressed = state == ElementState::Pressed;
                }
                IconBehavior::SettingToggle => {
                    if let Some(callback) = &self.click_callback {
                        if state == ElementState::Pressed {
                            callback(icon_context);
                        }
                    }
                }
                IconBehavior::Visual => {
                    // Doesn't matter, although we still return true because we don't want to be able to click through visual icons
                }
            };
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, mouse_pos: (i32, i32), delta: std::time::Duration, icon_context: &IconContext) -> Option<TooltipState> {
        if let Some(get_disabled) = self.get_disabled.as_ref() {
            self.disabled = get_disabled(icon_context);
        }

        // If not hovered and a click button, unselect
        if !self.hovered && self.pressed && matches!(self.behavior, IconBehavior::Click) {
            self.pressed = false;
        }

        // Update hover
        let mouse_over = self.bounds.contains(mouse_pos);
        self.hovered = mouse_over && !self.disabled;
        self.active = self.get_active.as_ref().map_or(false, |get_active| get_active(icon_context)) || self.pressed;

        self.anim.update(delta, self.visible);

        if mouse_over && self.tooltip_text.is_some() {
            Some(TooltipState::new(self.bounds, self.tooltip_text.clone().unwrap(), self.disabled))
        } else {
            None
        }
    }
}