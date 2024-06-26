use pixels::{check_texture_size, wgpu::{self, util::DeviceExt}, PixelsContext, TextureError};

use crate::{screenshot::Screenshot, selection::{Polygon, Selection, Vertex}};

use super::IconContext;

#[repr(C)]
#[derive(Clone, Debug)]
pub(crate) struct Locals {
    blur_enabled: u32,
    polygon: Polygon
}

impl Locals {
    pub(crate) fn new(selection: &Selection, window_size: (u32, u32), blur_enabled: bool) -> Self {
        let (window_width, window_height) = (window_size.0 as f32, window_size.1 as f32);
        Self {
            blur_enabled: if blur_enabled { 1 } else { 0 },
            // Temporary, until we get polygon logic working for the actual selection
            polygon: selection.get_device_coords_polygon(window_width, window_height)
        }
    }

    pub(crate) fn as_bytes(&self) -> Vec<u8> {
        let blur_enabled_bytes = bytemuck::bytes_of(&self.blur_enabled);

        let vertex_count = self.polygon.vertices.len() as u32;
        let vertex_count_bytes = bytemuck::bytes_of(&vertex_count);

        let gpu_vertices = self.polygon.as_gpu_vertices();
        let polygon_bytes = bytemuck::try_cast_slice(&gpu_vertices);
        if polygon_bytes.is_err() {
            eprintln!("Failed to cast polygon vertices to bytes");
            return vec![];
        }

        let polygon_bytes = polygon_bytes.unwrap();
        let mut bytes = Vec::with_capacity(blur_enabled_bytes.len() + vertex_count_bytes.len() + polygon_bytes.len());
        bytes.extend_from_slice(blur_enabled_bytes);
        bytes.extend_from_slice(vertex_count_bytes);
        bytes.extend_from_slice(&polygon_bytes);

        bytes
    }
}

impl Default for Locals {
    fn default() -> Self {
        Self {
            polygon: Polygon {
                vertices: vec![
                    Vertex::new(0.0, 0.0),
                    Vertex::new(1.0, 0.0),
                    Vertex::new(1.0, 1.0),
                    Vertex::new(0.0, 1.0),
                ]
            },
            blur_enabled: 0
        }
    }
}

pub(crate) struct BackgroundRenderer {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bg_bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    locals_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
}

impl BackgroundRenderer {
    pub(crate) fn new(
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
        initial_background_data: &[u8]
    ) -> Result<Self, TextureError> {
        let device = pixels.device();
        let shader = wgpu::include_wgsl!("../shaders/background.wgsl");
        let module = device.create_shader_module(shader);

        let texture = create_texture_with_data(pixels, width, height, initial_background_data)?;
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create a texture sampler with nearest neighbor
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Background renderer sampler"),
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
            label: Some("Background renderer vertex buffer"),
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
            label: Some("Background renderer u_Locals"),
            contents: &Locals::default().as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
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
            label: Some("Background renderer pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Background renderer pipeline"),
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
            bg_bind_group_layout: bind_group_layout,
            bind_group,
            background_pipeline: render_pipeline,

            locals_buffer,
            vertex_buffer
        })
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
        
        self.bind_group = create_bind_group(
            pixels.device(),
            &self.bg_bind_group_layout,
            &self.texture_view,
            &self.sampler,
            &self.locals_buffer,
        );

        Ok(())
    }

    pub(crate) fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, clip_rect: (u32, u32, u32, u32)) {
        rpass.set_pipeline(&self.background_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_scissor_rect(clip_rect.0, clip_rect.1, clip_rect.2, clip_rect.3);
        rpass.draw(0..3, 0..1);
    }
    pub(crate) fn write_screenshot_to_texture(
        &mut self,
        pixels: &pixels::Pixels,
        screenshot: &Screenshot
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

    pub(crate) fn update(
        &mut self,
        context: &PixelsContext,
        window_size: (u32, u32),
        selection: &Selection,
        icon_context: &IconContext,
    ) {
        let locals = Locals::new(selection, window_size, icon_context.settings.background_blur_enabled);

        let device = &context.device;
        let queue = &context.queue;

        let local_data = locals.as_bytes();
        let current_locals_size = self.locals_buffer.size();
        if local_data.len() > current_locals_size as usize {
            // Resize the locals buffer
            self.locals_buffer.destroy();
            self.locals_buffer = {
                let unpadded_size = current_locals_size * 2;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Background renderer locals buffer"),
                    // Properly align the buffer
                    size: (unpadded_size + (wgpu::COPY_BUFFER_ALIGNMENT - 1)) & !(wgpu::COPY_BUFFER_ALIGNMENT - 1),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false
                });
                buffer
            };
            self.bind_group = create_bind_group(
                device,
                &self.bg_bind_group_layout,
                &self.texture_view,
                &self.sampler,
                &self.locals_buffer,
            );
        }
        queue.write_buffer(&self.locals_buffer, 0, &local_data);
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
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };

    Ok(device.create_texture_with_data(pixels.queue(), &texture_descriptor, data))
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