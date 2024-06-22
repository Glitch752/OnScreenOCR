use pixels::{
    check_texture_size,
    wgpu::{self, util::DeviceExt},
    TextureError,
};

use crate::{screenshot::Screenshot, selection::Selection};

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
        let (mut selection_width, mut selection_height) = (selection.width as f32, selection.height as f32);
        let (mut selection_x, mut selection_y) = (selection.x as f32, selection.y as f32);

        // Ensure dimensions are positive
        if selection_width < 0.0 {
            selection_x += selection_width;
            selection_width = -selection_width;
        }
        if selection_height < 0.0 {
            selection_y += selection_height;
            selection_height = -selection_height;
        }

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
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    locals_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
}

impl Renderer {
    pub(crate) fn new(
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
    ) -> Result<Self, TextureError> {
        let device = pixels.device();
        let shader = wgpu::include_wgsl!("./shader.wgsl");
        let module = device.create_shader_module(shader);

        let texture = create_texture(pixels, width, height)?;
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
            border_color: None,
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
            depth_stencil: None,
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

        Ok(Self {
            texture,
            texture_view,
            sampler,
            bind_group_layout,
            bind_group,
            render_pipeline,
            locals_buffer,
            vertex_buffer,
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
    ) -> Result<(), TextureError> {
        self.texture = create_texture(pixels, width, height)?;
        self.texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        self.bind_group = create_bind_group(
            pixels.device(),
            &self.bind_group_layout,
            &self.texture_view,
            &self.sampler,
            &self.locals_buffer,
        );

        Ok(())
    }

    pub(crate) fn update(&self, queue: &wgpu::Queue, locals: Locals) {
        queue.write_buffer(&self.locals_buffer, 0, locals.to_bytes());
    }

    pub(crate) fn render(
        &self,
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
            depth_stencil_attachment: None,
        });
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_scissor_rect(clip_rect.0, clip_rect.1, clip_rect.2, clip_rect.3);
        rpass.draw(0..3, 0..1);
    }
}

fn create_texture(
    pixels: &pixels::Pixels,
    width: u32,
    height: u32,
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

    Ok(device.create_texture(&texture_descriptor))
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