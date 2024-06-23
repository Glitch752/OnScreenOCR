use debounce::EventDebouncer;
use image::{GenericImage, ImageBuffer, Pixel, Rgba};
use std::{sync::{LazyLock, Mutex}, time::Duration};

use crate::{screenshot::Screenshot, selection::{Bounds, Selection}};

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

static CURRENT_SCREENSOT: LazyLock<Mutex<Box<Option<Screenshot>>>> = LazyLock::new(|| Mutex::new(Box::new(None)));

#[derive(Debug, Copy, Clone)]
pub(crate) enum OCREvent {
    SelectionChanged(Bounds),
}

impl PartialEq for OCREvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OCREvent::SelectionChanged(_), OCREvent::SelectionChanged(_)) => true,
        }
    }
}

pub(crate) struct OCRHandler {
    pub debouncer: Option<EventDebouncer<OCREvent>>
}

impl Default for OCRHandler {
    fn default() -> Self {
        OCRHandler {
            debouncer: None
        }
    }
}

impl OCRHandler {
    pub fn set_screenshot(&mut self, screenshot: Screenshot) {
        let mut current_screenshot = CURRENT_SCREENSOT.lock().expect("Couldn't unlock screenshot");
        *current_screenshot = Box::new(Some(screenshot));
    }

    pub fn selection_changed(&mut self, latest_selection: Selection) {
        if self.debouncer.is_none() {
            self.initialize_debouncer();
        }
        self.debouncer
            .as_mut()
            .unwrap()
            .put(OCREvent::SelectionChanged(latest_selection.bounds.clone()));
    }

    fn initialize_debouncer(&mut self) {
        self.debouncer = Some(EventDebouncer::new(
            DEBOUNE_TIME,
            move |event| match event {
                OCREvent::SelectionChanged(bounds) => {
                    perform_ocr(bounds);
                }
            },
        ));
    }
}

fn perform_ocr(bounds: Bounds) {
    // Get the current screenshot
    let screenshot = CURRENT_SCREENSOT.lock().expect("Couldn't unlock screenshot").clone().expect("No screenshot available");
    
    // Crop the screenshot
    let mut img: ImageBuffer<Rgba<_>, Vec<u8>> = image::ImageBuffer::from_raw(
        screenshot.width as u32,
        screenshot.height as u32,
        screenshot.bytes
    ).unwrap();
    let pos_bounds = bounds.to_positive_size();
    let image_view = img.sub_image(
        pos_bounds.x as u32,
        pos_bounds.y as u32,
        pos_bounds.width as u32,
        pos_bounds.height as u32
    );
    let mut cropped_data = image_view.to_image().to_vec().into_iter().collect::<Vec<u8>>();
    // Enumerate over 4-tuples (x, y, r, g, b)
    for pixel in cropped_data.chunks_exact_mut(4) {
        // Image is bgra, so we need to swap r and b
        pixel.swap(0, 2);
    }
    let cropped_image: ImageBuffer<Rgba<_>, Vec<u8>> = image::ImageBuffer::from_vec(
        pos_bounds.width as u32,
        pos_bounds.height as u32,
        cropped_data
    ).unwrap();

    // Export to a png and save it under the current directory
    cropped_image.save("cropped.png").unwrap();

    let mut leptess = leptess::LepTess::new(Some("./tessdata"), "eng")
                .expect("Unable to create Tesseract instance");

    leptess.set_image("cropped.png").expect("Unable to set image");
    leptess.recognize();

    let text = leptess.get_utf8_text().unwrap();
    println!("Recognized text: {}", text);
}
