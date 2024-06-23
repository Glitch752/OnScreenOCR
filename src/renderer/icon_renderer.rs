use crate::selection::Bounds;

pub(crate) struct IconRenderer {
    pub format_single_line_icon: Icon,
    pub copy_icon: Icon,
    pub icon_atlas: Vec<u8>,
}


pub(crate) enum IconBehavior {
    Toggle,
    Click,
}

pub(crate) struct Icon {
    pub id: &'static str,
    pub hovered: bool,
    pub selected: bool,
    pub bounds: Bounds,
    pub behavior: IconBehavior,
    pub click_callback: Option<Box<dyn Fn() -> ()>>,

    icon_normal_index: u32,
    icon_hovered_index: u32,
    icon_selected_index: u32,
    icon_selected_hovered_index: u32,
}

macro_rules! image {
    ($path:expr) => {
        {
            let img = image::load_from_memory(include_bytes!($path)).unwrap();
            let raw = img.to_rgba8().into_raw();
            raw
        }
    };
}

static ATLAS_POSITIONS: &str = include_str!("../icons/atlas_positions.txt");

fn get_icon_index(id: &str) -> u32 {
    ATLAS_POSITIONS.lines().find(|line| line.starts_with(id)).unwrap().split_whitespace().last().unwrap().parse().unwrap()
}

macro_rules! create_icon {
    ($id:literal, $behavior:expr) => {
        Icon {
            id: $id,
            hovered: false,
            selected: false,
            bounds: Bounds::default(),
            behavior: $behavior,
            click_callback: None,

            icon_normal_index: get_icon_index(concat!($id, "")),
            icon_hovered_index: get_icon_index(concat!($id, "-hover")),
            icon_selected_index: get_icon_index(concat!($id, "-selected")),
            icon_selected_hovered_index: get_icon_index(concat!($id, "-selected-hover"))
        }
    };
}

impl IconRenderer {
    pub fn new() -> Self {
        IconRenderer {
            format_single_line_icon: create_icon!("new-line", IconBehavior::Toggle),
            copy_icon: create_icon!("copy", IconBehavior::Click),
            icon_atlas: image!("../icons/atlas.png"),
        }
    }

    pub fn icons(&mut self) -> Vec<&mut Icon> {
        vec![&mut self.format_single_line_icon, &mut self.copy_icon]
    }

    pub fn render(&mut self, renderer: &mut super::Renderer) {
        self.icons().iter().for_each(|icon| icon.render(renderer));
    }

    pub fn click(&mut self, mouse_pos: (i32, i32)) {
        self.icons().iter_mut().for_each(|icon| icon.click(mouse_pos));
    }

    pub fn update(&mut self, mouse_pos: (i32, i32)) {
        self.icons().iter_mut().for_each(|icon| icon.hovered = icon.bounds.contains(mouse_pos));
    }
}

impl Icon {
    pub fn render(&self, renderer: &mut super::Renderer) {
        let icon_index = match (self.selected, self.hovered) {
            (false, false) => self.icon_normal_index,
            (false, true) => self.icon_hovered_index,
            (true, false) => self.icon_selected_index,
            (true, true) => self.icon_selected_hovered_index,
        };


    }

    pub fn click(&mut self, mouse_pos: (i32, i32)) {
        if self.bounds.contains(mouse_pos) {
            match self.behavior {
                IconBehavior::Toggle => {
                    self.selected = !self.selected;
                }
                IconBehavior::Click => {
                    if let Some(callback) = &self.click_callback {
                        callback();
                    }
                }
            }
        }
    }
}