use background_renderer::BackgroundRenderer;
pub use icon_renderer::{IconContext, IconEvent};

use icon_renderer::IconRenderer;
use ocr_preview_renderer::OCRPreviewRenderer;
use pixels::{wgpu, PixelsContext, TextureError};
use winit::event::ElementState;
use crate::{selection::Bounds, wgpu_text::{glyph_brush::ab_glyph::FontRef, BrushBuilder, TextBrush}};

use crate::{screenshot::Screenshot, selection::Selection};

mod icon_renderer;
mod icon_layout_engine;
mod ocr_preview_renderer;
mod animation;
mod background_renderer;

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
#[allow(dead_code)] // Many of these fields are actually used
pub(crate) struct Renderer {
    background_renderer: BackgroundRenderer,

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

        let mut icon_renderer = IconRenderer::new(device, width as f32, height as f32);
        icon_renderer.initialize(pixels.queue());

        let ocr_preview_renderer = OCRPreviewRenderer::new();

        let background_renderer = BackgroundRenderer::new(pixels, width, height, initial_background_data)?;

        Ok(Self {
            text_brush: BrushBuilder::using_font_bytes(include_bytes!("../fonts/DejaVuSans.ttf")).expect("Unable to load font")
                .build(
                    device,
                    width,
                    height,
                    pixels.render_texture_format()
                ),
            should_render_text: false,
            icon_renderer,
            ocr_preview_renderer,
            background_renderer,
            last_update: std::time::Instant::now()
        })
    }

    pub(crate) fn write_screenshot_to_texture(
        &mut self,
        pixels: &pixels::Pixels,
        screenshot: Screenshot
    ) -> Result<(), TextureError> {
        self.background_renderer.write_screenshot_to_texture(pixels, screenshot)?;
        Ok(())
    }

    pub(crate) fn resize(
        &mut self,
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
        new_background_data: &[u8]
    ) -> Result<(), TextureError> {
        self.text_brush.resize_view(width as f32, height as f32, pixels.queue());
        self.icon_renderer.resize_view(width as f32, height as f32, pixels.queue());
        self.background_renderer.resize(pixels, width, height, new_background_data)?;

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

        self.background_renderer.update(queue, window_size, selection, icon_context);

        let ocr_section = self.ocr_preview_renderer.get_ocr_section(ocr_preview_text, window_size, &mut self.icon_renderer, delta, selection, icon_context);
        let mut sections = self.icon_renderer.get_text_sections();
        if ocr_section.is_some() {
            sections.push(ocr_section.as_ref().unwrap());
        }
        self.should_render_text = sections.len() > 0;
        self.text_brush.queue(device, queue, sections).unwrap();

        self.icon_renderer.update(queue, delta, relative_mouse_pos, icon_context);
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
            depth_stencil_attachment: None,
        });

        self.background_renderer.render(&mut rpass, clip_rect);

        if self.should_render_text {
            self.text_brush.draw(&mut rpass);
        }
        
        self.icon_renderer.render(&mut rpass);
    }
}