use background_renderer::BackgroundRenderer;
pub use icon_renderer::{IconContext, IconEvent};

use icon_renderer::IconRenderer;
use ocr_preview_renderer::OCRPreviewRenderer;
use pixels::{wgpu, PixelsContext, TextureError};
use winit::event::ElementState;
use crate::selection::Bounds;

pub(crate) use animation::{SmoothMoveFadeAnimation, SmoothFadeAnimation};

use crate::{screenshot::Screenshot, selection::Selection};

mod icon_renderer;
mod ocr_preview_renderer;
mod animation;
mod background_renderer;

#[allow(dead_code)] // Many of these fields are actually used
pub(crate) struct Renderer {
    background_renderer: BackgroundRenderer,
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
        let mut icon_renderer = IconRenderer::new(pixels, width as f32, height as f32);
        icon_renderer.initialize(pixels.queue());

        let ocr_preview_renderer = OCRPreviewRenderer::new(pixels, width, height);
        let background_renderer = BackgroundRenderer::new(pixels, width, height, initial_background_data)?;

        Ok(Self {
            icon_renderer,
            ocr_preview_renderer,
            background_renderer,
            last_update: std::time::Instant::now()
        })
    }

    pub(crate) fn write_screenshot_to_texture(
        &mut self,
        pixels: &pixels::Pixels,
        screenshot: &Screenshot
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
        self.ocr_preview_renderer.resize(pixels, width, height);
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
        selection: &Selection,
        ocr_preview_text: Option<String>,
        relative_mouse_pos: (i32, i32),
        icon_context: &IconContext
    ) {
        let delta = self.last_update.elapsed();
        self.last_update = std::time::Instant::now();

        self.ocr_preview_renderer.update(context, window_size, selection.bounds, ocr_preview_text, icon_context, delta, &mut self.icon_renderer);
        self.background_renderer.update(context, window_size, selection, icon_context);
        self.icon_renderer.update(context, delta, relative_mouse_pos, icon_context);
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
        self.ocr_preview_renderer.render(&mut rpass);
        self.icon_renderer.render(&mut rpass);
    }
}