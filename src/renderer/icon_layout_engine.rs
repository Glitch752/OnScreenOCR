use super::icon_renderer::*;
use super::Bounds;

pub const ICON_SIZE: f32 = 50.0;
pub const ICON_MARGIN: f32 = 10.0;

static ATLAS_POSITIONS: &str = include_str!("../icons/atlas_positions.txt");

pub fn get_icon_atlas_pos(id: &str) -> u32 {
    let pos = ATLAS_POSITIONS.lines().find(|line| line.starts_with(id)).unwrap().split_whitespace().last().unwrap().parse().unwrap();
    pos
}

macro_rules! create_icon {
    ($id:literal, $behavior:expr, $bounds:expr) => {
        {
            use crate::renderer::icon_layout_engine::{ get_icon_atlas_pos, ICON_SIZE };
            Icon {
                hovered: false,
                selected: false,
                bounds: Bounds::from_center($bounds.0, $bounds.1, ICON_SIZE, ICON_SIZE),
                behavior: $behavior,
                click_callback: None,

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
            Icon {
                hovered: false,
                selected: false,
                bounds: Bounds::from_center($bounds.0, $bounds.1, ICON_SIZE + ICON_MARGIN, ICON_SIZE + ICON_MARGIN),
                behavior: IconBehavior::Visual,
                click_callback: None,

                icon_normal_pos: get_icon_atlas_pos("background.png"),
                icon_hovered_pos: get_icon_atlas_pos("background.png"),
                icon_selected_pos: get_icon_atlas_pos("background.png"),
                icon_selected_hovered_pos: get_icon_atlas_pos("background.png")
            }
        }
    };
}

pub(crate) struct IconLayouts {
    layouts: Vec<PositionedLayout>
}

impl IconLayouts {
    pub fn new() -> Self {
        IconLayouts {
            layouts: Vec::new()
        }
    }

    pub fn add_layout(&mut self, center_position: (f32, f32), layout: LayoutChild) {
        self.layouts.push(PositionedLayout::new(center_position, layout));
    }

    pub fn icons(&self) -> Vec<&Icon> {
        self.layouts.iter().flat_map(|sub_layout| sub_layout.icons()).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.layouts.iter_mut().flat_map(|sub_layout| sub_layout.icons_mut()).collect()
    }

    pub fn recalculate_positions(&mut self) -> () {
        for sub_layout in self.layouts.iter_mut() {
            sub_layout.recalculate_positions();
        }
    }

    pub fn initialize(&mut self) {
        for sub_layout in self.layouts.iter_mut() {
            sub_layout.initialize();
        }
    }
}

pub(crate) struct PositionedLayout {
    // TODO: Change to an object that allows screen-size-relative positioning
    center_position: (f32, f32),
    last_center_position: Option<(f32, f32)>,
    layout: LayoutChild
}

impl PositionedLayout {
    pub fn new(center_position: (f32, f32), layout: LayoutChild) -> Self {
        PositionedLayout {
            center_position,
            last_center_position: None,
            layout
        }
    }

    pub fn icons(&self) -> Vec<&Icon> {
        match &self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons()
        }
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        match &mut self.layout {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut()
        }
    }

    pub fn recalculate_positions(&mut self) -> () {
        if Some(self.center_position) == self.last_center_position {
            return;
        }
        self.last_center_position = Some(self.center_position);

        match &mut self.layout {
            LayoutChild::Icon(icon) => {
                icon.bounds.set_center(self.center_position.0, self.center_position.1);
            }
            LayoutChild::Layout(layout) => {
                layout.calculated_position = self.center_position;
                layout.calculate_size();
                layout.calculate_child_positions();
            }
        }
    }

    pub fn initialize(&mut self) {
        match &mut self.layout {
            LayoutChild::Icon(_) => (),
            LayoutChild::Layout(layout) => layout.initialize()
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

pub(crate) enum LayoutChild {
    Icon(Icon),
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

    pub fn initialize(&mut self) {
        let mut icon_children_count = 0;
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(_) => icon_children_count += 1,
                LayoutChild::Layout(layout) => layout.initialize()
            }
        }
        
        // There is 1 background child for every icon and 1 for every space between icons
        let background_icons_required = if self.has_background { icon_children_count * 2 - 1 } else { 0 };
        for _ in 0..background_icons_required {
            self.background_children.push(create_background!((0, 0)));
        }
    }

    pub fn add_icon(&mut self, icon: Icon) {
        self.children.push(LayoutChild::Icon(icon));
    }

    pub fn icons(&self) -> Vec<&Icon> {
        // Make sure to return background icons first
        self.background_children.iter().chain(self.children.iter().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons()
        })).collect()
    }

    pub fn icons_mut(&mut self) -> Vec<&mut Icon> {
        self.background_children.iter_mut().chain(self.children.iter_mut().flat_map(|child| match child {
            LayoutChild::Icon(icon) => vec!(icon),
            LayoutChild::Layout(layout) => layout.icons_mut()
        })).collect()
    }

    pub fn calculate_size(&mut self) -> (f32, f32) {
        let mut width = 0.0;
        let mut height = 0.0;
        for child in self.children.iter_mut() {
            match child {
                LayoutChild::Icon(icon) => {
                    width += icon.bounds.width as f32;
                    height += icon.bounds.height as f32;
                }
                LayoutChild::Layout(layout) => {
                    let (child_width, child_height) = layout.calculate_size();
                    match self.direction {
                        Direction::Horizontal => {
                            width += child_width;
                            height = height.max(child_height);
                        }
                        Direction::Vertical => {
                            width = width.max(child_width);
                            height += child_height;
                        }
                    }
                }
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
                    layout.calculated_position = top_left_position;
                    layout.calculate_child_positions();
                    top_left_position.0 += layout.calculated_size.0 + self.spacing;
                }
            }
        }

        if !self.has_background {
            return;
        }

        let mut last_position: Option<(i32, i32)> = None;
        let mut background_icon_index = 0;
        let icon_children = self.children.iter().filter_map(|child| match child {
            LayoutChild::Icon(icon) => Some(icon),
            LayoutChild::Layout(_) => None
        });
        for icon in icon_children {
            let icon_position = icon.bounds.get_center();
            if let Some(last_position) = last_position {
                let background = self.background_children.get_mut(background_icon_index).unwrap();
                background.bounds.set_center((icon_position.0 + last_position.0) as f32 / 2., (icon_position.1 + last_position.1) as f32 / 2.);
                background_icon_index += 1;
            }
            last_position = Some(icon_position);

            let background = self.background_children.get_mut(background_icon_index).unwrap();
            background.bounds.set_center(icon_position.0 as f32, icon_position.1 as f32);
            background_icon_index += 1;
        }
    }
}