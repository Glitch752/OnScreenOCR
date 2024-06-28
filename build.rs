use std::env;
use std::io::Write;

use image::{GenericImage, GenericImageView};

// Looks best if it's a multiple of the icon size in icon_layout_engine.rs
const DOWNSCALE_SIZE: f32 = 80.0;

fn main() {  
    embed_resource::compile("tray_icon.rc", embed_resource::NONE);

    let generate_icon_atlas = env::var("GENERATE_ICON_ATLAS")
        .map(|v| v == "1")
        .unwrap_or(false);

    if generate_icon_atlas {
        generate_atlas();
    }
}

pub fn generate_atlas() -> () {
    // Load all the files in src/icons
    let icon_paths = std::fs::read_dir("src/icons").unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        // Exclude the atlas.png and atlas_positions.txt files
        .filter(|path| path.file_name().unwrap() != "atlas.png" && path.file_name().unwrap() != "atlas_positions.txt")
        .collect::<Vec<_>>();

    // Load all the images
    let icon_images = icon_paths.iter()
        .map(|path| image::open(path).unwrap())
        .collect::<Vec<_>>();

    // Dramatically downscale images (512x512 -> ICON_SIZE x ICON_SIZE)
    let icon_images = icon_images.iter()
        .map(|img| {
            let (width, height) = img.dimensions();
            let scale = DOWNSCALE_SIZE / width as f32;
            img.resize((width as f32 * scale) as u32, (height as f32 * scale) as u32, image::imageops::FilterType::Nearest)
        })
        .collect::<Vec<_>>();
        
    let max_icon_size = icon_images.iter()
        .map(|img| img.dimensions())
        .fold(0, |max, (width, height)| {
            max.max(width).max(height)
        });

    let image_count = icon_images.len();
    let min_image_width = (image_count as f32).sqrt().ceil() as u32;
    let min_image_height = (image_count as f32 / min_image_width as f32).ceil() as u32;

    // Merge all of the images
    let mut atlas = image::DynamicImage::new_rgba8(min_image_width * max_icon_size, min_image_height * max_icon_size);
    let mut image_name_to_position = std::collections::HashMap::new();
    for (i, img) in icon_images.iter().enumerate() {
        let x = (i as u32 % min_image_width) * max_icon_size;
        let y = (i as u32 / min_image_width) * max_icon_size;

        atlas.copy_from(img, x, y).unwrap();
        image_name_to_position.insert(icon_paths[i].file_name().unwrap().to_str().unwrap().to_string(), format!("{} {}", x, y));
    }

    // Save the atlas
    atlas.save("src/icons/atlas.png").unwrap();

    // Save the image name to index map to atlas_positions.txt
    let mut file = std::fs::File::create("src/icons/atlas_positions.txt").unwrap();
    file.write_all(format!("{} {} {}\n", max_icon_size, min_image_width, min_image_height).as_bytes()).unwrap();
    for (name, index) in image_name_to_position.iter() {
        file.write_all(format!("{} {}\n", name, index).as_bytes()).unwrap();
    }
}