use std::time::Instant;

use glyph_brush::{ab_glyph::FontRef, BuiltInLineBreaker, HorizontalAlign, OwnedSection, OwnedText};
use pixels::{wgpu, PixelsContext};

use crate::{selection::Bounds, wgpu_text::{BrushBuilder, TextBrush}};

use super::{animation::{MoveDirection, SmoothMoveFadeAnimation}, icon_renderer::{TEXT_HEIGHT, IconRenderer}, IconContext};

pub(crate) struct OCRPreviewRenderer {
    anim: SmoothMoveFadeAnimation,
    last_text: Option<String>,
    last_placement: Option<PreviewTextPlacement>,

    text_brush: TextBrush<FontRef<'static>>,
    should_render_text: bool,

    active_feedback_text: Option<String>,
    active_feedback_color: [f32; 3],
    feedback_text_anim: SmoothMoveFadeAnimation,
    current_feedback_start_time: Instant,

    feedback_text_queue: Vec<(String, [f32; 3])>
}

#[derive(Debug, Clone)]
pub(crate) struct PreviewTextPlacement {
    x: f32,
    y: f32,
    horizontal_align: HorizontalAlign,
    max_line_length: f32,
}

impl OCRPreviewRenderer {
    pub(crate) fn new(
        pixels: &pixels::Pixels,
        width: u32,
        height: u32,
    ) -> Self {
        let device = pixels.device();
        Self {
            anim: SmoothMoveFadeAnimation::new(false, MoveDirection::Right, 6.),
            last_text: None,
            last_placement: None,
            text_brush: BrushBuilder::using_font_bytes(include_bytes!("../../fonts/DejaVuSans.ttf")).expect("Unable to load font")
                .build(
                    device,
                    width,
                    height,
                    pixels.render_texture_format()
                ),
            should_render_text: false,

            active_feedback_text: None,
            active_feedback_color: [1.0, 1.0, 1.0],
            feedback_text_anim: SmoothMoveFadeAnimation::new(false, MoveDirection::Down, 10.),
            feedback_text_queue: vec![],
            current_feedback_start_time: Instant::now()
        }
    }
    
    fn get_preview_text_placement(
        &self,
        is_fading_out: bool,
        window_size: (u32, u32),
        bounds: Bounds,
        text_lines: i32,
        max_line_characters: i32
    ) -> Option<PreviewTextPlacement> {
        if is_fading_out {
            return self.last_placement.clone();
        }

        let bounds = bounds.to_positive_size();
        if bounds.width == 0 || bounds.height == 0 {
            return None;
        }

        let left_side_space = bounds.x;
        let right_side_space = window_size.0 as i32 - (bounds.x + bounds.width);

        let margin = 10;

        let y = std::cmp::max(margin + 16, std::cmp::min(bounds.y, window_size.1 as i32 - ((text_lines - 1) * 18 + margin)));

        let minimum_side_space = 100;

        if left_side_space >= right_side_space {
            if left_side_space < minimum_side_space {
                return Some(PreviewTextPlacement {
                    x: margin as f32,
                    y: y as f32,
                    horizontal_align: glyph_brush::HorizontalAlign::Left,
                    max_line_length: window_size.0 as f32 - margin as f32 * 2.
                });
            }

            let max_line_length = bounds.x as f32 - margin as f32 * 2.;
            // If we have more than 3 lines and any line is very long, we should align to the left at the edge of the screen instead since it just looks better
            // Very long is subjective here -- we could come up with a real heuristic but that would require feedback from the layout engine which I do not want to do.
            if text_lines > 3 && max_line_characters as f32 * TEXT_HEIGHT as f32 / 2. > max_line_length {
                return Some(PreviewTextPlacement {
                    x: margin as f32,
                    y: y as f32,
                    horizontal_align: glyph_brush::HorizontalAlign::Left,
                    max_line_length
                });
            }

            Some(PreviewTextPlacement {
                x: (bounds.x - margin) as f32,
                y: y as f32,
                horizontal_align: glyph_brush::HorizontalAlign::Right,
                max_line_length
            })
        } else {
            if right_side_space < minimum_side_space {
                return Some(PreviewTextPlacement {
                    x: window_size.0 as f32 - margin as f32,
                    y: y as f32,
                    horizontal_align: glyph_brush::HorizontalAlign::Right,
                    max_line_length: window_size.0 as f32 - margin as f32 * 2.
                });
            }

            let max_line_length = window_size.0 as f32 - (bounds.x as f32 + bounds.width as f32 + margin as f32);
            Some(PreviewTextPlacement {
                x: (bounds.x + bounds.width + margin) as f32,
                y: y as f32,
                horizontal_align: glyph_brush::HorizontalAlign::Left,
                max_line_length
            })
        }
    }

    fn get_ocr_section(
        &mut self,
        ocr_preview_text: Option<String>,
        window_size: (u32, u32),
        icon_renderer: &mut IconRenderer,
        delta: std::time::Duration,
        bounds: Bounds,
        #[allow(unused_variables)]
        icon_context: &super::IconContext
    ) -> Option<OwnedSection> {
        if ocr_preview_text.is_none() && self.last_text.is_none() {
            self.last_placement = None;
            icon_renderer.update_text_icon_positions(None);
            return None;
        }

        let mut text = ocr_preview_text.clone().unwrap_or_else(|| self.last_text.clone().unwrap());

        text = text.replace("\t", "  ");

        if text.trim().is_empty() {
            self.last_text = None;
            self.last_placement = None;
            icon_renderer.update_text_icon_positions(None);
            return None;
        }

        if icon_context.settings.add_pilcrow_in_preview {
            text = text.lines().map(|x| x.to_string() + " ¶").collect::<Vec<String>>().join("\n");
            // Remove the last pilcrow
            text.pop();
        }

        self.last_text = Some(text.clone());

        let visible = ocr_preview_text.is_some(); // && !icon_context.settings_panel_visible;

        let max_line_chars = text.lines().map(|x| x.chars().count()).max().unwrap_or(0) as i32;
        let placement = self.get_preview_text_placement(self.anim.fading_out(), window_size, bounds, text.lines().count() as i32, max_line_chars);
        if placement.is_none() && self.last_placement.is_none() {
            self.last_text = None;
            icon_renderer.update_text_icon_positions(None);
            return None;
        }
        let placement = placement.unwrap_or_else(|| self.last_placement.clone().unwrap());

        self.anim.update(delta, visible);
        self.anim.fade_move_direction = if placement.horizontal_align == HorizontalAlign::Left { MoveDirection::Right } else { MoveDirection::Left };

        icon_renderer.update_text_icon_positions(ocr_preview_text.map(|_| (placement.x + (if placement.horizontal_align == HorizontalAlign::Left { -24. } else { 24. }), placement.y - 18.0)));
        let section = Some(OwnedSection::default()
            .add_text(OwnedText::new("Preview:\n").with_color([1.0, 1.0, 1.0, 0.9 * self.anim.get_opacity()]).with_scale(16.0))
            .add_text(OwnedText::new(text).with_color([0.8, 0.8, 0.8, 0.8 * self.anim.get_opacity()]).with_scale(18.0))
            .with_screen_position(self.anim.move_point((placement.x, placement.y - 18.0)))
            .with_layout(glyph_brush::Layout::default()
                .h_align(placement.horizontal_align)
                .line_breaker(BuiltInLineBreaker::UnicodeLineBreaker)
            )
            .with_bounds((placement.max_line_length, window_size.1 as f32))
        );
        if self.anim.fading_out() {
            self.last_placement = Some(placement);
        } else {
            self.last_placement = None;
        }

        section
    }

    pub(crate) fn resize(
        &mut self,
        pixels: &pixels::Pixels,
        width: u32,
        height: u32
    ) -> () {
        self.text_brush.resize_view(width as f32, height as f32, pixels.queue());
    }

    fn get_feedback_text(
        &mut self,
        delta: std::time::Duration,
        window_size: (u32, u32)
    ) -> Option<OwnedSection> {
        if self.active_feedback_text.is_none() {
            if self.feedback_text_queue.is_empty() {
                self.feedback_text_anim.update(delta, false);
                return None;
            }
            let (text, color) = self.feedback_text_queue.remove(0);
            self.active_feedback_text = Some(text);
            self.active_feedback_color = color;
            self.current_feedback_start_time = Instant::now();
        }

        let visible = self.current_feedback_start_time.elapsed().as_secs_f32() < 1.5;
        self.feedback_text_anim.update(delta, visible);
        self.feedback_text_anim.fade_move_direction = MoveDirection::Up;

        let section = Some(OwnedSection::default()
            .add_text(OwnedText::new(self.active_feedback_text.clone().unwrap()).with_color([
                self.active_feedback_color[0],
                self.active_feedback_color[1],
                self.active_feedback_color[2],
                0.9 * self.feedback_text_anim.get_opacity()
            ]).with_scale(24.0))
            .with_screen_position(self.feedback_text_anim.move_point((window_size.0 as f32 / 2., 65.)))
            .with_layout(glyph_brush::Layout::default()
                .h_align(glyph_brush::HorizontalAlign::Center)
            )
        );

        if !self.feedback_text_anim.visible_at_all() {
            self.active_feedback_text = None;
        }

        section
    }

    pub(crate) fn show_user_feedback(
        &mut self,
        text: String,
        color: [f32; 3]
    ) -> () {
        self.feedback_text_queue.push((text, color));
    }

    pub(crate) fn update(
        &mut self,
        context: &PixelsContext,
        window_size: (u32, u32),
        bounds: Bounds,
        ocr_preview_text: Option<String>,
        icon_context: &IconContext,
        delta: std::time::Duration,
        icon_renderer: &mut IconRenderer
    ) {
        let device = &context.device;
        let queue = &context.queue;

        let mut sections = Vec::new();
        
        let ocr_section = self.get_ocr_section(ocr_preview_text, window_size, icon_renderer, delta, bounds, icon_context);
        if ocr_section.is_some() {
            sections.push(ocr_section.as_ref().unwrap());
        }

        let feedback_text = self.get_feedback_text(delta, window_size);
        if feedback_text.is_some() {
            sections.push(feedback_text.as_ref().unwrap());
        }

        self.should_render_text = sections.len() > 0;
        if self.should_render_text {
            self.text_brush.queue(device, queue, sections).unwrap();
        }
    }

    pub(crate) fn render<'pass>(
        &'pass mut self,
        rpass: &mut wgpu::RenderPass<'pass>
    ) -> () {
        if self.should_render_text {
            self.text_brush.draw(rpass);
        }
    }
}