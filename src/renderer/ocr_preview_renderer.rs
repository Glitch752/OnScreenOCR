use glyph_brush::{BuiltInLineBreaker, HorizontalAlign, OwnedSection, OwnedText};

use crate::selection::{Bounds, Selection};

use super::{animation::{MoveDirection, SmoothMoveFadeAnimation}, icon_layout_engine::TEXT_HEIGHT, icon_renderer::IconRenderer, ZIndex};

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

        let y = std::cmp::min(bounds.y, window_size.1 as i32 - ((text_lines - 1) * 19 + margin));

        if left_side_space > right_side_space {
            let max_line_length = bounds.x as f32 - margin as f32 * 2.;
            // If we have more than 3 lines and any line is very long, we should align to the left at the edge of the screen instead since it just looks better
            // Very long is subjective here -- we could come up with a real heuristic but that would require feedback from the layout engine which I do not want to do.
            if text_lines > 3 && max_line_characters as f32 > max_line_length * TEXT_HEIGHT as f32 / 2. {
                return Some(PreviewTextPlacement {
                    x: margin as f32,
                    y: y as f32,
                    horizontal_align: glyph_brush::HorizontalAlign::Left,
                    max_line_length: window_size.0 as f32 - margin as f32 * 2.
                });
            }

            Some(PreviewTextPlacement {
                x: (bounds.x - margin) as f32,
                y: y as f32,
                horizontal_align: glyph_brush::HorizontalAlign::Right,
                max_line_length
            })
        } else {
            let max_line_length = window_size.0 as f32 - (bounds.x as f32 + bounds.width as f32 + margin as f32);
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
        selection: Selection,
        #[allow(unused_variables)]
        icon_context: &super::IconContext
    ) -> Option<OwnedSection> {
        if ocr_preview_text.is_none() && self.last_text.is_none() {
            icon_renderer.update_text_icon_positions(None);
            return None;
        }

        let text = ocr_preview_text.clone().unwrap_or_else(|| self.last_text.clone().unwrap());
        // Add a pilcrow to the end of every line
        let text = text.lines().map(|x| x.to_string() + "¶").collect::<Vec<String>>().join("\n");

        self.last_text = Some(text.clone());

        let visible = ocr_preview_text.is_some(); // && !icon_context.settings_panel_visible;

        let max_line_chars = text.lines().map(|x| x.chars().count()).max().unwrap_or(0) as i32;
        let placement = self.get_preview_text_placement(!visible, window_size, selection.bounds, text.lines().count() as i32, max_line_chars);
        if placement.is_none() && self.last_placement.is_none() {
            icon_renderer.update_text_icon_positions(None);
            return None;
        }
        let placement = placement.unwrap_or_else(|| self.last_placement.clone().unwrap());

        self.anim.update(delta, visible);
        self.anim.fade_move_direction = if placement.horizontal_align == HorizontalAlign::Left { MoveDirection::Right } else { MoveDirection::Left };

        icon_renderer.update_text_icon_positions(ocr_preview_text.map(|_| (placement.x + (if placement.horizontal_align == HorizontalAlign::Left { -24. } else { 24. }), placement.y - 18.0)));
        let section = Some(OwnedSection::default()
            .add_text(OwnedText::new("Preview:\n").with_z(ZIndex::OCRPreviewText).with_color([1.0, 1.0, 1.0, 0.9 * self.anim.get_opacity()]).with_scale(16.0))
            .add_text(OwnedText::new(text).with_z(ZIndex::OCRPreviewText).with_color([0.8, 0.8, 0.8, 0.8 * self.anim.get_opacity()]).with_scale(18.0))
            .with_screen_position(self.anim.move_point((placement.x, placement.y - 18.0)))
            .with_layout(glyph_brush::Layout::default()
                .h_align(placement.horizontal_align)
                .line_breaker(BuiltInLineBreaker::UnicodeLineBreaker)
            )
            .with_bounds((placement.max_line_length, window_size.1 as f32))
        );
        self.last_placement = Some(placement);
        section
    }
}