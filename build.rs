use std::env;
use std::io::Write;

use image::{GenericImage, GenericImageView};

fn main() {  
    // let generate_icon_atlas = env::var("GENERATE_ICON_ATLAS")
    //     .map(|v| v == "1")
    //     .unwrap_or(false);

    // if generate_icon_atlas {
        generate_atlas();
    // }
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

    let max_icon_size = icon_images.iter()
        .map(|img| img.dimensions())
        .fold((0, 0), |(max_width, max_height), (width, height)| {
            (max_width.max(width), max_height.max(height))
        });

    // Merge all of the images
    let mut atlas = image::DynamicImage::new_rgba8(max_icon_size.0 * icon_images.len() as u32, max_icon_size.1);
    let mut image_name_to_position = std::collections::HashMap::new();
    for (i, img) in icon_images.iter().enumerate() {
        atlas.copy_from(img, max_icon_size.0 * i as u32, 0).unwrap();
        image_name_to_position.insert(icon_paths[i].file_name().unwrap().to_str().unwrap(), format!("{}", i as u32 * max_icon_size.0));
    }

    // Save the atlas
    atlas.save("src/icons/atlas.png").unwrap();

    // Save the image name to index map to atlas_positions.txt
    let mut file = std::fs::File::create("src/icons/atlas_positions.txt").unwrap();
    for (name, index) in image_name_to_position.iter() {
        file.write_all(format!("{} {}\n", name, index).as_bytes()).unwrap();
    }
}