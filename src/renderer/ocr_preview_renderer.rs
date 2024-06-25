use glyph_brush::{BuiltInLineBreaker, GlyphPositioner, HorizontalAlign, OwnedSection, OwnedText};

use crate::selection::{Bounds, Selection};

use super::{animation::{MoveDirection, SmoothMoveFadeAnimation}, icon_renderer::IconRenderer};

#[derive(Debug, Clone, Default)]
pub(crate) struct OCRPreviewRenderer {
    anim: SmoothMoveFadeAnimation,
    last_text: Option<String>,
    last_placement: Option<PreviewTextPlacement>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreviewTextPlacement {
    x: f32,
    y: f32,
    horizontal_align: HorizontalAlign,
    max_line_length: f32,
}

impl OCRPreviewRenderer {
    pub(crate) fn new() -> Self {
        Self {
            anim: SmoothMoveFadeAnimation::new(false, MoveDirection::Right, 6.),
            last_text: None,
            last_placement: None,
        }
    }
    
    fn get_preview_text_placement(
        &self,
        is_fading_out: bool,
        window_size: (u32, u32),
        bounds: Bounds,
        text_lines: i32
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

        let y = std::cmp::min(bounds.y, window_size.1 as i32 - ((text_lines - 1) * 19 + margin));

        if left_side_space > right_side_space {
            let max_line_length = bounds.x as f32 - margin as f32 * 2.;
            Some(PreviewTextPlacement {
                x: (bounds.x - margin) as f32,
                y: y as f32,
                horizontal_align: glyph_brush::HorizontalAlign::Right,
                max_line_length
            })
        } else {
            let max_line_length = window_size.0 as f32 - (bounds.x as f32 + bounds.width as f32 * 2. + margin as f32 * 2.);
            Some(PreviewTextPlacement {
                x: (bounds.x + bounds.width + margin) as f32,
                y: y as f32,
                horizontal_align: glyph_brush::HorizontalAlign::Left,
                max_line_length
            })
        }
    }

    pub(crate) fn get_ocr_section(
        &mut self,
        ocr_preview_text: Option<String>,
        window_size: (u32, u32),
        icon_renderer: &mut IconRenderer,
        delta: std::time::Duration,
        selection: Selection
    ) -> Option<OwnedSection> {
        if ocr_preview_text.is_none() && self.last_text.is_none() {
            icon_renderer.update_text_icon_positions(None);
            return None;
        }

        let text = ocr_preview_text.clone().unwrap_or_else(|| self.last_text.clone().unwrap());
        // Add a pilcrow to the end of every line
        let text = text.lines().map(|x| x.to_string() + "¶").collect::<Vec<String>>().join("\n");

        self.last_text = Some(text.clone());

        let placement = self.get_preview_text_placement(ocr_preview_text.is_none(), window_size, selection.bounds, text.lines().count() as i32);
        if placement.is_none() && self.last_placement.is_none() {
            icon_renderer.update_text_icon_positions(None);
            return None;
        }
        let placement = placement.unwrap_or_else(|| self.last_placement.clone().unwrap());

        self.anim.update(delta, ocr_preview_text.is_some());
        self.anim.fade_move_direction = if placement.horizontal_align == HorizontalAlign::Left { MoveDirection::Right } else { MoveDirection::Left };

        icon_renderer.update_text_icon_positions(ocr_preview_text.map(|_| (placement.x + (if placement.horizontal_align == HorizontalAlign::Left { -24. } else { 24. }), placement.y - 18.0)));
        let section = Some(OwnedSection::default()
            .add_text(OwnedText::new("Preview:\n").with_color([1.0, 1.0, 1.0, 0.9 * self.anim.get_opacity()]).with_scale(16.0))
            .add_text(OwnedText::new(text).with_color([0.8, 0.8, 0.8, 0.6 * self.anim.get_opacity()]).with_scale(18.0))
            .with_screen_position(self.anim.move_point((placement.x, placement.y - 18.0)))
            .with_layout(glyph_brush::Layout::default()
                .h_align(placement.horizontal_align)
                .line_breaker(BuiltInLineBreaker::UnicodeLineBreaker)
            ));
        self.last_placement = Some(placement);
        section
    }
}