use crate::selection::Bounds;

pub(crate) struct IconRenderer {
    pub format_single_line_icon: Icon,
    pub copy_icon: Icon,
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

    icon_normal: Vec<u8>,
    icon_hovered: Vec<u8>,
    icon_selected: Vec<u8>,
    icon_selected_hovered: Vec<u8>,

    icon_normal_id: u32,
    icon_hovered_id: u32,
    icon_selected_id: u32,
    icon_selected_hovered_id: u32,
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

macro_rules! create_icon {
    ($id:literal, $behavior:expr) => {
        Icon {
            id: $id,
            hovered: false,
            selected: false,
            bounds: Bounds::default(),
            behavior: $behavior,
            click_callback: None,

            icon_normal: image!(concat!("../icons/", $id, ".png")),
            icon_hovered: image!(concat!("../icons/", $id, "-hover.png")),
            icon_selected: image!(concat!("../icons/", $id, "-selected.png")),
            icon_selected_hovered: image!(concat!("../icons/", $id, "-selected-hover.png")),

            icon_normal_id: 0,
            icon_hovered_id: 1,
            icon_selected_id: 2,
            icon_selected_hovered_id: 3
        }
    };
}

impl IconRenderer {
    pub fn new() -> Self {
        let mut renderer = IconRenderer {
            format_single_line_icon: create_icon!("new-line", IconBehavior::Toggle),
            copy_icon: create_icon!("copy", IconBehavior::Click),
        };
        renderer.generate_atlas();
        renderer
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

    pub fn generate_atlas(&mut self) -> Vec<u8> {
        let image_size: u32 = 512; // Maybe there's a better way to actually define this based on the images input, but this works for now
        let icons: u32 = (self.icons().len() * 4) as u32;

        let mut atlas_pixels = vec![0; (image_size * image_size * 4 * icons) as usize];
        let mut offset = 0;

        for (icon, i) in self.icons().iter_mut().zip(0..) {
            // For each of the 4 images
            for i in 0..4 {
                let icon_data = match i {
                    0 => &icon.icon_normal,
                    1 => &icon.icon_hovered,
                    2 => &icon.icon_selected,
                    3 => &icon.icon_selected_hovered,
                    _ => unreachable!()
                };

                let icon_size = (icon_data.len() as f32 / 4.0).sqrt() as usize;
                let icon_data: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::ImageBuffer::from_raw(icon_size as u32, icon_size as u32, icon_data.clone()).unwrap();

                let atlas_x: u32 = offset * image_size as u32;

                for (icon_x, icon_y, pixel) in icon_data.enumerate_pixels() {
                    let pixel = pixel.0;
                    let pixel_index = icon_x + atlas_x + icon_y * image_size * icons;
                    let index = (pixel_index * 4) as usize;
                    atlas_pixels[index] = pixel[0];
                    atlas_pixels[index + 1] = pixel[1];
                    atlas_pixels[index + 2] = pixel[2];
                    atlas_pixels[index + 3] = pixel[3];
                }

                offset += 1;
            }

            icon.icon_normal_id += i as u32 * 4;
            icon.icon_hovered_id += i as u32 * 4;
            icon.icon_selected_id += i as u32 * 4;
            icon.icon_selected_hovered_id += i as u32 * 4;
        }

        println!("Generated atlas with {} icons", icons);

        // Temporary: save the image to disk
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::ImageBuffer::from_raw((image_size * icons) as u32, image_size as u32, atlas_pixels.clone()).unwrap();
        img.save("atlas.png").unwrap();

        atlas_pixels
    }
}

impl Icon {
    pub fn render(&self, renderer: &mut super::Renderer) {
        let icon = if self.selected {
            if self.hovered {
                &self.icon_selected_hovered
            } else {
                &self.icon_selected
            }
        } else {
            if self.hovered {
                &self.icon_hovered
            } else {
                &self.icon_normal
            }
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