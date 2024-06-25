use glyph_brush::{HorizontalAlign, OwnedSection, OwnedText};

use crate::selection::{Bounds, Selection};

use super::icon_renderer::IconRenderer;

#[derive(Debug, Clone, Default)]
pub(crate) struct OCRPreviewRenderer {
    text_opacity: f32,
    last_text: Option<String>,
    last_placement: Option<(f32, f32, HorizontalAlign)>,
}

impl OCRPreviewRenderer {
    pub(crate) fn new() -> Self {
        Self {
            text_opacity: 0.,
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
    ) -> Option<(f32, f32, glyph_brush::HorizontalAlign)> {
        if is_fading_out {
            return self.last_placement;
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
            Some((
                (bounds.x - margin) as f32,
                y as f32,
                glyph_brush::HorizontalAlign::Right
            ))
        } else {
            Some((
                (bounds.x + bounds.width + margin) as f32,
                y as f32,
                glyph_brush::HorizontalAlign::Left
            ))
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
        self.last_text = Some(text.clone());

        let placement = self.get_preview_text_placement(ocr_preview_text.is_none(), window_size, selection.bounds, text.lines().count() as i32);
        if placement.is_none() && self.last_placement.is_none() {
            icon_renderer.update_text_icon_positions(None);
            return None;
        }
        let placement = placement.unwrap_or_else(|| self.last_placement.unwrap());
        self.last_placement = Some(placement);

        let target_opacity = if ocr_preview_text.is_some() { 1. } else { 0. };
        self.text_opacity += (self.text_opacity - target_opacity) * (1. - (delta.as_millis_f32() * 0.02).exp());
        // Just in case something goes wrong
        if self.text_opacity.is_nan() || self.text_opacity < 0. || self.text_opacity > 1. {
            self.text_opacity = target_opacity;
        }
        if self.text_opacity < 0.01 {
            self.last_text = None;
            self.last_placement = None;
            self.text_opacity = 0.;
            return None;
        }
        
        icon_renderer.update_text_icon_positions(ocr_preview_text.map(|_| (placement.0 + (if placement.2 == HorizontalAlign::Left { -24. } else { 24. }), placement.1 - 18.0)));

        let animate_direction = if placement.2 == HorizontalAlign::Left { 1. } else { -1. };
        return Some(OwnedSection::default()
            .add_text(OwnedText::new("Preview:\n").with_color([1.0, 1.0, 1.0, 0.9 * self.text_opacity]).with_scale(16.0))
            .add_text(OwnedText::new(text).with_color([0.8, 0.8, 0.8, 0.6 * self.text_opacity]).with_scale(18.0))
            .with_screen_position((placement.0 + (1. - self.text_opacity) * 5. * animate_direction, placement.1 - 18.0))
            .with_layout(glyph_brush::Layout::default().h_align(placement.2)));
    }
}