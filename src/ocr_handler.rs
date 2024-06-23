use debounce::EventDebouncer;
use image::{GenericImageView, ImageBuffer, Rgba};
use std::{sync::{Mutex, LazyLock}, time::Duration};
use std::sync::mpsc;

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
    pub debouncer: Option<EventDebouncer<OCREvent>>,
    pub leptess: leptess::LepTess,
}

impl Default for OCRHandler {
    fn default() -> Self {
        OCRHandler {
            debouncer: None,
            // TODO: Support for other languages?
            leptess: leptess::LepTess::new(Some("./tessdata"), "eng")
                .expect("Unable to create Tesseract instance"),
        }
    }
}

impl OCRHandler {
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
    let img: ImageBuffer<Rgba<_>, Vec<u8>> = image::ImageBuffer::from_raw(
        screenshot.width as u32,
        screenshot.height as u32,
        screenshot.bytes
    ).unwrap();
    let imageView = img.view(
        bounds.x as u32,
        bounds.y as u32,
        bounds.width as u32,
        bounds.height as u32
    );
    let croppedImage = imageView.to_image();
    // Export to a png and save it under the current directory
    croppedImage.save("cropped.png").unwrap();
}
