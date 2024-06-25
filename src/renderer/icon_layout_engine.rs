use std::collections::HashMap;

use super::animation::MoveDirection;
use super::animation::SmoothMoveFadeAnimation;
use super::icon_renderer::*;
use super::Bounds;

use glyph_brush::OwnedSection;
use glyph_brush::OwnedText;

pub const ICON_SIZE: f32 = 40.0;
pub const ICON_MARGIN: f32 = 10.0;
pub const TEXT_HEIGHT: f32 = 20.0;

static ATLAS_POSITIONS: &str = include_str!("../icons/atlas_positions.txt");

pub fn get_icon_atlas_pos(id: &str) -> (u32, u32) {
    let pos = ATLAS_POSITIONS.lines().find(|line| line.starts_with(id)).unwrap().split_whitespace().skip(1).collect::<Vec<&str>>();
    (pos[0].parse().unwrap(), pos[1].parse().unwrap())
}

macro_rules! create_icon {
    ($id:literal, $behavior:expr) => {
        {
            use crate::renderer::icon_layout_engine::{ get_icon_atlas_pos, ICON_SIZE };
            use crate::renderer::animation::{ SmoothMoveFadeAnimation, MoveDirection };
            Icon {
                hovered: false,
                pressed: false,
                active: false,

                bounds: Bounds::new(0, 0, ICON_SIZE, ICON_SIZE),
                behavior: $behavior,
                click_callback: None,
                get_active: None,

                visible: true,
                anim: SmoothMoveFadeAnimation::new(true, MoveDirection::Up, 10.0),

                icon_normal_pos: get_icon_atlas_pos(concat!($id, ".png")),
                icon_hovered_pos: get_icon_atlas_pos(concat!($id, "-hover.png")),
                icon_selected_pos: get_icon_atlas_pos(concat!($id, "-selected.png")),
                icon_selected_hovered_pos: get_icon_atlas_pos(concat!($id, "-selected-hover.png"))
            }
        }
    };
}
pub(crate) use create_icon;

macro_rules! create_background {
    ($bounds:expr) => {
        {
            use crate::renderer::icon_layout_engine::{ get_icon_atlas_pos, ICON_SIZE, ICON_MARGIN };
            use crate::renderer::animation::{ SmoothMoveFadeAnimation, MoveDirection };
            Icon {
                hovered: false,
                pressed: false,
                active: false,

                bounds: Bounds::from_center($bounds.0, $bounds.1, ICON_SIZE + ICON_MARGIN, ICON_SIZE + ICON_MARGIN),
                behavior: IconBehavior::Visual,
                click_callback: None,
                get_active: None,

                visible: true,
                anim: SmoothMoveFadeAnimation::new(true, MoveDirection::Up, 10.0),

                icon_normal_pos: get_icon_atlas_pos("background.png"),
                icon_hovered_pos: get_icon_atlas_pos("background.png"),
                icon_selected_pos: get_icon_atlas_pos("background.png"),
                icon_selected_hovered_pos: get_icon_atlas_pos("background.png")
            }
        }
    };
}

pub(crate) struct IconLayouts {
    layouts: HashMap<String, PositionedLayout>
}

impl IconLayouts {
    pub fn new() -> Self {
        IconLayouts {
            layouts: HashMap::new()
        }
    }

    pub fn add_layout(&mut self, label: String, center_position: ScreenRelativePosition, layout: LayoutChild) {
        self.layouts.insert(label, PositionedLayout::new(center_position, layout));
    }

    pub fn set_center(&mut self, label: &str, x: f32, y: f32) {
        self.layouts.get_mut(label).unwrap().set_offset(x, y);
    }

    pub fn set_visible(&mut self, label: &str, visible: bool) {
        self.layouts.get_mut(label).unwrap().set_visible(visible);
    }

    pub fn icons(&self) -> Vec<&Icon> {
        self.layouts.iter().flat_map(|(_, sub_layout)| sub_layout.icons()).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.layouts.iter_mut().flat_map(|(_, sub_layout)| sub_layout.icons_mut()).collect()
    }

    pub fn text_mut(&mut self) -> Vec<&mut IconText> {
        self.layouts.iter_mut().flat_map(|(_, sub_layout)| sub_layout.text_mut()).collect()
    }

    pub fn text_sections(&self) -> Vec<&OwnedSection> {
        self.layouts.iter().flat_map(|(_, sub_layout)| sub_layout.text_sections()).collect()
    }

    pub fn recalculate_positions(&mut self, screen_size: (f32, f32)) -> () {
        for (_, sub_layout) in self.layouts.iter_mut() {
            sub_layout.recalculate_positions(screen_size);
        }
    }

    pub fn initialize(& mut self) {
        for (_, sub_layout) in self.layouts.iter_mut() {
            sub_layout.initialize();
        }
    }

    pub fn update_all(&mut self, mouse_pos: (i32, i32), delta: std::time::Duration, icon_context: &IconContext) {
        for (_, sub_layout) in self.layouts.iter_mut() {
            sub_layout.update_all(mouse_pos, delta, icon_context);
        }
    }
}

pub(crate) struct PositionedLayout {
    center_position: ScreenRelativePosition,
    calculated_center_position: (f32, f32),
    last_center_position: Option<(f32, f32)>,
    layout: LayoutChild,
    last_screen_size: (f32, f32)
}

impl PositionedLayout {
    pub fn new(center_position: ScreenRelativePosition, layout: LayoutChild) -> Self {
        PositionedLayout {
            center_position,
            calculated_center_position: (0.0, 0.0),
            last_center_position: None,
            layout,
            last_screen_size: (0.0, 0.0)
        }
    }

    pub fn icons(&self) -> Vec<&Icon> {
        match &self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons(),
            _ => Vec::new()
        }
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        match &mut self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut(),
            _ => Vec::new()
        }
    }

    pub fn text_mut(&mut self) -> Vec<&mut IconText> {
        match &mut self.layout {
            LayoutChild::Text(text) => vec!(text),
            LayoutChild::Layout(layout) => layout.text_mut(),
            _ => Vec::new()
        }
    }

    pub fn text_sections(&self) -> Vec<&OwnedSection> {
        match &self.layout {
            LayoutChild::Text(text) => if text.visible { vec!(&text.text_section) } else { Vec::new() },
            LayoutChild::Layout(layout) => layout.text_sections(),
            _ => Vec::new()
        }
    }

    pub fn recalculate_positions(&mut self, screen_size: (f32, f32)) -> () {
        self.last_screen_size = screen_size;

        self.calculated_center_position = self.center_position.get_position(screen_size);
        if Some(self.calculated_center_position) == self.last_center_position {
            return;
        }
        self.last_center_position = Some(self.calculated_center_position);

        match &mut self.layout {
            LayoutChild::Icon(icon) => {
                icon.bounds.set_center(self.calculated_center_position.0, self.calculated_center_position.1);
            }
            LayoutChild::Text(text) => {
                text.bounds.set_center(self.calculated_center_position.0, self.calculated_center_position.1);
                text.update_section_position();
            }
            LayoutChild::Layout(layout) => {
                layout.calculated_position = self.calculated_center_position;
                layout.calculate_size();
                layout.calculate_child_positions();
            }
        }
    }

    pub fn initialize(&mut self) {
        match &mut self.layout {
            LayoutChild::Layout(layout) => layout.initialize(),
            _ => ()
        }
    }

    pub fn set_offset(&mut self, x: f32, y: f32) {
        self.center_position.offset = (x, y);
    }

    pub fn set_visible(&mut self, visible: bool) {
        match &mut self.layout {
            LayoutChild::Icon(icon) => icon.visible = visible,
            LayoutChild::Text(text) => text.visible = visible,
            LayoutChild::Layout(layout) => layout.set_visible(visible)
        }
    }

    pub fn update_all(&mut self, mouse_pos: (i32, i32), delta: std::time::Duration, icon_context: &IconContext) {
        self.icons_mut().into_iter().for_each(|icon| icon.update(mouse_pos, delta, icon_context));
        
        let mut any_text_changed = false;
        self.text_mut().into_iter().for_each(|text| { any_text_changed = text.update(delta) || any_text_changed; });
        if any_text_changed {
            self.recalculate_positions(self.last_screen_size);
        }
    }
}

#[allow(unused)]
pub(crate) enum Direction {
    Horizontal,
    Vertical
}

#[allow(unused)]
pub(crate) enum CrossJustify {
    Start,
    Center,
    End
}

pub(crate) struct Layout {
    children: Vec<LayoutChild>,
    direction: Direction,
    cross_justify: CrossJustify,
    spacing: f32,

    has_background: bool,
    background_children: Vec<Icon>,

    calculated_position: (f32, f32),
    calculated_size: (f32, f32)
}

pub(crate) struct IconText {
    bounds: Bounds,
    text_section: OwnedSection,
    visible: bool,
    anim: SmoothMoveFadeAnimation,

    pub get_text: Option<Box<dyn Fn(&IconContext) -> String>>
}

impl IconText {
    pub fn new(string: String) -> Self {
        // Approximate text size
        let bounds = Bounds::new(0, 0, string.len() as f32 * TEXT_HEIGHT * 0.5 + ICON_MARGIN, TEXT_HEIGHT as i32);
        IconText {
            bounds,
            text_section: OwnedSection {
                screen_position: (0.0, 0.0),
                bounds: (f32::INFINITY, f32::INFINITY),
                layout: glyph_brush::Layout::default(),
                text: vec![OwnedText::new(string).with_scale(20.0).with_color([1.0, 1.0, 1.0, 1.0])],
            },
            visible: true,
            anim: SmoothMoveFadeAnimation::new(true, MoveDirection::Up, 10.0),
            get_text: None
        }
    }

    pub fn update_section_position(&mut self) {
        self.text_section.screen_position = self.anim.move_point((self.bounds.x as f32 + ICON_MARGIN, self.bounds.y as f32));
    }

    pub fn update(&mut self, delta: std::time::Duration) -> bool {
        self.anim.update(delta, self.visible);
        self.update_section_position();

        if let Some(get_text) = &self.get_text {
            let current_string = self.text_section.text[0].text.clone();
            let new_string = get_text();
            if current_string != new_string {
                self.text_section.text[0].text = new_string;
                return true;
            }
        }
        false
    }
}

pub(crate) enum LayoutChild {
    Icon(Icon),
    Text(IconText),
    Layout(Layout)
}

impl Layout {
    pub fn new(direction: Direction, cross_justify: CrossJustify, spacing: f32, has_background: bool) -> Self {
        Layout {
            children: Vec::new(),
            direction,
            cross_justify,
            spacing,
            has_background,
            background_children: Vec::new(),
            calculated_position: (0.0, 0.0),
            calculated_size: (0.0, 0.0)
        }
    }

    pub fn initialize(& mut self) {
        self.calculate_size();
        let primary_dimension = match self.direction {
            Direction::Horizontal => self.calculated_size.0,
            Direction::Vertical => self.calculated_size.1
        };
        // There is 1 background child for every (ICON_SIZE * 0.9) length
        let background_icons_required = if self.has_background { (primary_dimension / (ICON_SIZE * 0.9) + 0.2).floor() as u32 } else { 0 };
        for _ in 0..background_icons_required {
            self.background_children.push(create_background!((0, 0)));
        }

        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Layout(layout) => layout.initialize(),
                _ => ()
            }
        }
    }

    pub fn add_icon(&mut self, icon: Icon) {
        self.children.push(LayoutChild::Icon(icon));
    }

    pub fn add_text(&mut self, text: IconText) {
        self.children.push(LayoutChild::Text(text));
    }

    pub fn add_layout(&mut self, layout: Layout) {
        self.children.push(LayoutChild::Layout(layout));
    }

    pub fn icons(&self) -> Vec<&Icon> {
        // Make sure to return background icons first
        self.background_children.iter().chain(self.children.iter().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons(),
            _ => Vec::new()
        })).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.background_children.iter_mut().chain(self.children.iter_mut().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut(),
            _ => Vec::new()
        })).collect()
    }

    pub fn text_mut(&mut self) -> Vec<&mut IconText> {
        self.children.iter_mut().flat_map(|child| match child {
            LayoutChild::Text(text) => vec!(text),
            LayoutChild::Layout(layout) => layout.text_mut(),
            _ => Vec::new()
        }).collect()
    }

    pub fn text_sections(&self) -> Vec<&OwnedSection> {
        self.children.iter().flat_map(|child| match child {
            LayoutChild::Text(text) => if text.visible { vec!(&text.text_section) } else { Vec::new() },
            LayoutChild::Layout(layout) => layout.text_sections(),
            _ => Vec::new()
        }).collect()
    }

    pub fn calculate_size(&mut self) -> (f32, f32) {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(Icon { bounds, .. }) | LayoutChild::Text(IconText { bounds, .. }) => {
                    match self.direction {
                        Direction::Horizontal => {
                            width += bounds.width as f32 + self.spacing;
                            height = height.max(bounds.height as f32);
                        }
                        Direction::Vertical => {
                            width = width.max(bounds.width as f32);
                            height += bounds.height as f32 + self.spacing;
                        }
                    }
                }
                LayoutChild::Layout(layout) => {
                    let (child_width, child_height) = layout.calculate_size();
                    match self.direction {
                        Direction::Horizontal => {
                            width += child_width + self.spacing;
                            height = height.max(child_height);
                        }
                        Direction::Vertical => {
                            width = width.max(child_width);
                            height += child_height + self.spacing;
                        }
                    }
                }
            }
        }
        
        // Remove the extra padding added from the last item
        match self.direction {
            Direction::Horizontal => {
                width -= self.spacing;
            }
            Direction::Vertical => {
                height -= self.spacing;
            }
        }

        self.calculated_size = (width, height);
        (width, height)
    }

    pub fn calculate_child_positions(&mut self) -> () {
        let mut top_left_position = (self.calculated_position.0 - self.calculated_size.0 / 2., self.calculated_position.1 - self.calculated_size.1 / 2.);
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(icon) => {
                    match self.direction {
                        Direction::Horizontal => {
                            match self.cross_justify {
                                CrossJustify::Start => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1);
                                }
                                CrossJustify::Center => {
                                    icon.bounds.set_center(top_left_position.0 + icon.bounds.width as f32 / 2., top_left_position.1 + icon.bounds.height as f32 / 2.);
                                }
                                CrossJustify::End => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1 + self.calculated_size.1 - icon.bounds.height as f32);
                                }
                            }
                            top_left_position.0 += icon.bounds.width as f32 + self.spacing;
                        }
                        Direction::Vertical => {
                            match self.cross_justify {
                                CrossJustify::Start => {
                                    icon.bounds.set_origin(top_left_position.0, top_left_position.1);
                                }
                                CrossJustify::Center => {
                                    icon.bounds.set_center(top_left_position.0 + icon.bounds.width as f32 / 2., top_left_position.1 + icon.bounds.height as f32 / 2.);
                                }
                                CrossJustify::End => {
                                    icon.bounds.set_origin(top_left_position.0 + self.calculated_size.0 - icon.bounds.width as f32, top_left_position.1);
                                }
                            }
                            top_left_position.1 += icon.bounds.height as f32 + self.spacing;
                        }
                    }
                }
                LayoutChild::Layout(layout) => {
                    match self.cross_justify {
                        CrossJustify::Start => {
                            layout.calculated_position = (top_left_position.0 + layout.calculated_size.0 / 2., top_left_position.1 + layout.calculated_size.1 / 2.);
                        }
                        CrossJustify::Center => {
                            layout.calculated_position = (top_left_position.0 + self.calculated_size.0 / 2., top_left_position.1 + layout.calculated_size.1 / 2.);
                        }
                        CrossJustify::End => {
                            layout.calculated_position = (top_left_position.0 + self.calculated_size.0 - layout.calculated_size.0 / 2., top_left_position.1 + layout.calculated_size.1 / 2.);
                        }
                    }
                    layout.calculate_child_positions();
                    match self.direction {
                        Direction::Horizontal => {
                            top_left_position.0 += layout.calculated_size.0 + self.spacing;
                        }
                        Direction::Vertical => {
                            top_left_position.1 += layout.calculated_size.1 + self.spacing;
                        }
                    }
                }
                LayoutChild::Text(text) => {
                    match self.direction {
                        Direction::Horizontal => {
                            text.bounds.set_origin(top_left_position.0, top_left_position.1 + (self.calculated_size.1 - text.bounds.height as f32) / 2.);
                            top_left_position.0 += text.bounds.width as f32 + self.spacing;
                        }
                        Direction::Vertical => {
                            text.bounds.set_origin(top_left_position.0 + (self.calculated_size.0 - text.bounds.width as f32) / 2., top_left_position.1);
                            top_left_position.1 += text.bounds.height as f32 + self.spacing;
                        }
                    }
                    text.update_section_position();
                }
            }
        }

        if !self.has_background {
            return;
        }

        // Evenly space background icons
        let mut top_left_position = (self.calculated_position.0 - self.calculated_size.0 / 2., self.calculated_position.1 - self.calculated_size.1 / 2.);
        let background_children_count = self.background_children.len();
        for background in self.background_children.iter_mut() {
            background.bounds.set_center(top_left_position.0 + ICON_SIZE / 2., top_left_position.1 + ICON_SIZE / 2.);
            match self.direction {
                Direction::Horizontal => {
                    top_left_position.0 += self.calculated_size.0 / background_children_count as f32;
                }
                Direction::Vertical => {
                    top_left_position.1 += self.calculated_size.1 / background_children_count as f32;
                }
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(icon) => icon.visible = visible,
                LayoutChild::Layout(layout) => layout.set_visible(visible),
                LayoutChild::Text(text) => text.visible = visible
            }
        }
        for background in self.background_children.iter_mut() {
            background.visible = visible;
        }
    }
}

#[allow(unused)]
pub(crate) enum ScreenLocation {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight
}

impl ScreenLocation {
    pub fn get_on_screen_size(self: &ScreenLocation, screen_size: (f32, f32)) -> (f32, f32) {
        match self {
            ScreenLocation::TopLeft => (0.0, 0.0),
            ScreenLocation::TopCenter => (screen_size.0 / 2., 0.0),
            ScreenLocation::TopRight => (screen_size.0, 0.0),
            ScreenLocation::CenterLeft => (0.0, screen_size.1 / 2.),
            ScreenLocation::Center => (screen_size.0 / 2., screen_size.1 / 2.),
            ScreenLocation::CenterRight => (screen_size.0, screen_size.1 / 2.),
            ScreenLocation::BottomLeft => (0.0, screen_size.1),
            ScreenLocation::BottomCenter => (screen_size.0 / 2., screen_size.1),
            ScreenLocation::BottomRight => (screen_size.0, screen_size.1)
        }
    }
}

pub(crate) struct ScreenRelativePosition {
    relative_to: ScreenLocation,
    offset: (f32, f32)
}

impl ScreenRelativePosition {
    pub fn new(relative_to: ScreenLocation, offset: (f32, f32)) -> Self {
        ScreenRelativePosition {
            relative_to,
            offset
        }
    }

    pub fn get_position(self: &ScreenRelativePosition, screen_size: (f32, f32)) -> (f32, f32) {
        let (x, y) = self.relative_to.get_on_screen_size(screen_size);
        (x + self.offset.0, y + self.offset.1)
    }
}