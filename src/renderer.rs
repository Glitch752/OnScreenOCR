pub use icon_renderer::{IconContext, IconEvent};

use icon_renderer::IconRenderer;
use ocr_preview_renderer::OCRPreviewRenderer;
use pixels::{
    check_texture_size, wgpu::{self, util::DeviceExt}, PixelsContext, TextureError
};
use winit::event::ElementState;
use crate::{selection::Bounds, wgpu_text::{glyph_brush::ab_glyph::FontRef, BrushBuilder, TextBrush}};

use crate::{screenshot::Screenshot, selection::Selection};

mod icon_renderer;
mod icon_layout_engine;
mod ocr_preview_renderer;
mod animation;

#[allow(dead_code)]
pub enum ZIndex {
    Background = 3, // Not actually sent to the shader for now, just used for illustration
    OCRPreviewText = 2,
    Icon = 1, // Not actually sent to the shader for now, just used for illustration
    IconText = 0
}

impl Into<f32> for ZIndex {
    fn into(self) -> f32 {
        // The clipping plane of orthographic the projection matrix used is -1 to 1, so we need to scale the z-index to fit in that range
        self as i32 as f32 / 10.
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Locals {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    blur_enabled: u32
}

impl Locals {
    pub(crate) fn new(selection: Selection, window_size: (u32, u32), blur_enabled: bool) -> Self {
        let (window_width, window_height) = (window_size.0 as f32, window_size.1 as f32);
        let pos_bounds = selection.bounds.to_positive_size();
        let (selection_x, selection_y, selection_width, selection_height) = (
            pos_bounds.x as f32,
            pos_bounds.y as f32,
            pos_bounds.width as f32,
            pos_bounds.height as f32
        );

        Self {
            x:      selection_x / window_width,
            y:      selection_y / window_height,
            width:  selection_width / window_width,
            height: selection_height / window_height,
            blur_enabled: if blur_enabled { 1 } else { 0 }
        }
    }

    #[allow(dead_code)] // It is actually used... not sure what the warning is about
    pub(crate) fn to_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

impl Default for Locals {
    fn default() -> Self {
        Self {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
            blur_enabled: 0
        }
    }
}

#[allow(dead_code)] // Many of these fields are actually used
pub(crate) struct Renderer {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bg_bind_group_layout: wgpu::BindGroupLayout,
    background_bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    locals_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    text_brush: TextBrush<FontRef<'static>>,
    should_render_text: bool,

    icon_renderer: IconRenderer,
    ocr_preview_renderer: OCRPreviewRenderer,

    last_update: std::time::Instant,
}

impl Renderer {
    pub(crate) fn new(
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
        initial_background_data: &[u8]
    ) -> Result<Self, TextureError> {
        let device = pixels.device();
        let shader = wgpu::include_wgsl!("./shaders/background.wgsl");
        let module = device.create_shader_module(shader);

        let texture = create_texture_with_data(pixels, width, height, initial_background_data)?;
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create a texture sampler with nearest neighbor
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Renderer sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None
        });

        // Create vertex buffer; array-of-array of position and texture coordinates
        let vertex_data: [[f32; 2]; 3] = [
            // One full-screen triangle
            // See: https://github.com/parasyte/pixels/issues/180
            [-1.0, -1.0],
            [3.0, -1.0],
            [-1.0, 3.0],
        ];
        let vertex_data_slice = bytemuck::cast_slice(&vertex_data);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Renderer vertex buffer"),
            contents: vertex_data_slice,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: (vertex_data_slice.len() / vertex_data.len()) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        // Create uniform buffer
        let locals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Renderer u_Locals"),
            contents: bytemuck::bytes_of(&Locals::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Locals>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &texture_view,
            &sampler,
            &locals_buffer,
        );

        let depth_texture = create_depth_texture(pixels, width, height)?;
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_stencil_state = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Renderer pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Renderer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_stencil_state.clone()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pixels.render_texture_format(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let mut icon_renderer = IconRenderer::new(device, depth_stencil_state.clone(), width as f32, height as f32);
        icon_renderer.initialize(pixels.queue());

        let ocr_preview_renderer = OCRPreviewRenderer::new();

        Ok(Self {
            texture,
            texture_view,
            sampler,
            bg_bind_group_layout: bind_group_layout,
            background_bind_group: bind_group,
            background_pipeline: render_pipeline,
            depth_texture,
            depth_view,

            locals_buffer,
            vertex_buffer,
            text_brush: BrushBuilder::using_font_bytes(include_bytes!("../fonts/DejaVuSans.ttf")).expect("Unable to load font")
                .with_depth_stencil(Some(depth_stencil_state))
                .build(
                    device,
                    width,
                    height,
                    pixels.render_texture_format()
                ),
            should_render_text: false,
            icon_renderer,
            ocr_preview_renderer,
            last_update: std::time::Instant::now()
        })
    }

    pub(crate) fn write_screenshot_to_texture(
        &mut self,
        pixels: &pixels::Pixels,
        screenshot: Screenshot
    ) -> Result<(), TextureError> {
        pixels.queue().write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All
            },
            screenshot.bytes.as_slice(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(screenshot.width as u32 * 4),
                rows_per_image: Some(screenshot.height as u32),
            },
            wgpu::Extent3d {
                width: screenshot.width as u32,
                height: screenshot.height as u32,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    pub(crate) fn resize(
        &mut self,
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
        new_background_data: &[u8]
    ) -> Result<(), TextureError> {
        self.texture = create_texture_with_data(pixels, width, height, new_background_data)?;
        self.texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.depth_texture = create_depth_texture(pixels, width, height)?;
        self.depth_view = self.depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        self.background_bind_group = create_bind_group(
            pixels.device(),
            &self.bg_bind_group_layout,
            &self.texture_view,
            &self.sampler,
            &self.locals_buffer,
        );
        
        self.text_brush.resize_view(width as f32, height as f32, pixels.queue());
        self.icon_renderer.resize_view(width as f32, height as f32, pixels.queue());

        Ok(())
    }

    pub(crate) fn mouse_event(&mut self, mouse_pos: (i32, i32), state: ElementState, icon_context: &mut IconContext) -> bool {
        self.icon_renderer.mouse_event(mouse_pos, state, icon_context)
    }

    pub(crate) fn before_reopen_window(&mut self) {
        self.last_update = std::time::Instant::now();
    }

    pub(crate) fn update(
        &mut self,
        context: &PixelsContext,
        window_size: (u32, u32),
        selection: Selection,
        ocr_preview_text: Option<String>,
        relative_mouse_pos: (i32, i32),
        icon_context: &IconContext
    ) {
        let delta = self.last_update.elapsed();
        self.last_update = std::time::Instant::now();

        let device = &context.device;
        let queue = &context.queue;

        let locals = Locals::new(selection, window_size, icon_context.settings.background_blur_enabled);

        queue.write_buffer(&self.locals_buffer, 0, locals.to_bytes());

        let ocr_section = self.ocr_preview_renderer.get_ocr_section(ocr_preview_text, window_size, &mut self.icon_renderer, delta, selection, icon_context);
        let mut sections = self.icon_renderer.get_text_sections();
        if ocr_section.is_some() {
            sections.push(ocr_section.as_ref().unwrap());
        }
        self.should_render_text = sections.len() > 0;
        self.text_brush.queue(device, queue, sections).unwrap();

        self.icon_renderer.update(queue, delta, relative_mouse_pos, icon_context);
    }

    fn render_background<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, clip_rect: (u32, u32, u32, u32)) {
        rpass.set_pipeline(&self.background_pipeline);
        rpass.set_bind_group(0, &self.background_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_scissor_rect(clip_rect.0, clip_rect.1, clip_rect.2, clip_rect.3);
        rpass.draw(0..3, 0..1);
    }

    pub(crate) fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        clip_rect: (u32, u32, u32, u32),
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Renderer render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true
                }),
                stencil_ops: None,
            }),
        });

        self.render_background(&mut rpass, clip_rect);

        if self.should_render_text {
            self.text_brush.draw(&mut rpass);
        }
        
        self.icon_renderer.render(&mut rpass);
    }
}

fn create_texture_with_data(
    pixels: &pixels::Pixels,
    width: u32,
    height: u32,
    data: &[u8],
) -> Result<wgpu::Texture, TextureError> {
    let device = pixels.device();
    check_texture_size(device, width, height)?;
    let texture_descriptor = wgpu::TextureDescriptor {
        label: None,
        size: pixels::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: pixels.render_texture_format(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };

    Ok(device.create_texture_with_data(pixels.queue(), &texture_descriptor, data))
}

fn create_depth_texture(
    pixels: &pixels::Pixels,
    width: u32,
    height: u32,
) -> Result<wgpu::Texture, TextureError> {
    let device = pixels.device();

    let size = wgpu::Extent3d {
        width: width,
        height: height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    let texture = device.create_texture(&desc);

    Ok(texture)
}

fn create_bind_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    locals_buffer: &wgpu::Buffer,
) -> pixels::wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: locals_buffer.as_entire_binding(),
            },
        ],
    })
}