use image::{GenericImage, ImageBuffer, Rgba};
use std::{sync::{mpsc, Arc, LazyLock, Mutex}, time::{Duration, Instant}};
use std::thread::{self, JoinHandle};

use crate::{screenshot::Screenshot, selection::{Bounds, Selection}};

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

static CURRENT_SCREENSOT: LazyLock<Mutex<Box<Option<Screenshot>>>> = LazyLock::new(|| Mutex::new(Box::new(None)));

#[derive(Debug, Clone)]
pub(crate) enum OCREvent {
    SelectionChanged(Bounds),
    LanguageUpdated(String)
}

impl PartialEq for OCREvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OCREvent::SelectionChanged(_), OCREvent::SelectionChanged(_)) => true,
            (OCREvent::LanguageUpdated(_), OCREvent::LanguageUpdated(_)) => true,
            _ => false
        }
    }
}

pub(crate) struct OCRHandler {
    pub throttler: Option<OCRThrottler<OCREvent>>,
    pub ocr_result_sender: mpsc::Sender<String>,
    pub ocr_result_receiver: mpsc::Receiver<String>,
    pub ocr_preview_text: Option<String>,
    pub last_selection_bounds: Option<Bounds>, // Used to recalculate the same OCR when language changes
}

struct InitData {
    tx: mpsc::Sender<String>,
    leptess: leptess::LepTess,
}

impl OCRHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>();
        OCRHandler {
            throttler: None,
            ocr_result_sender: tx,
            ocr_result_receiver: rx,
            ocr_preview_text: None,
            last_selection_bounds: None
        }
    }

    pub fn set_screenshot(&mut self, screenshot: Screenshot) {
        let mut current_screenshot = CURRENT_SCREENSOT.lock().expect("Couldn't unlock screenshot");
        *current_screenshot = Box::new(Some(screenshot));
    }

    pub fn before_reopen_window(&mut self) {
        self.ocr_preview_text = None;
    }

    pub fn selection_changed(&mut self, latest_selection: Selection) {
        if self.throttler.is_none() {
            self.initialize_throttler();
        }
        self.throttler
            .as_mut()
            .unwrap()
            .put(OCREvent::SelectionChanged(latest_selection.bounds.clone()));
        
        self.last_selection_bounds = Some(latest_selection.bounds);
    }

    pub fn update_ocr_preview_text(&mut self) -> bool {
        if let Some(text) = self.get_ocr_result() {
            if text.is_empty() {
                self.ocr_preview_text = None;
                return false
            } else {
                self.ocr_preview_text = Some(text.clone());
                return true
            }
        }
        false
    }

    fn get_ocr_result(&mut self) -> Option<String> {
        self.ocr_result_receiver.try_recv().ok()
    }

    fn initialize_throttler(&mut self) {
        let tx = self.ocr_result_sender.clone();
        self.throttler = Some(OCRThrottler::new::<_, _, InitData>(
            DEBOUNE_TIME,
            move |event, init_data| match event {
                OCREvent::SelectionChanged(bounds) => {
                    perform_ocr(bounds, &mut init_data.leptess, &init_data.tx);
                }
                OCREvent::LanguageUpdated(language_code) => {
                    init_data.leptess = leptess::LepTess::new(Some("./tessdata"), &language_code).expect("Unable to create Tesseract instance");
                }
            },
            || {
                InitData {
                    leptess: leptess::LepTess::new(Some("./tessdata"), "eng").expect("Unable to create Tesseract instance"),
                    tx: tx
                }
            },
        ));
    }

    pub fn update_ocr_language(&mut self, language_code: String) {
        if let Some(throttler) = &mut self.throttler {
            throttler.put(OCREvent::LanguageUpdated(language_code));
            throttler.put(OCREvent::SelectionChanged(self.last_selection_bounds.clone().unwrap()));
        }
    }
}

fn perform_ocr(bounds: Bounds, leptess: &mut leptess::LepTess, tx: &mpsc::Sender<String>) {
    let mut pos_bounds = bounds.to_positive_size();
    if pos_bounds.width < 5 || pos_bounds.height < 5 {
        return;
    }

    // Get the current screenshot
    let screenshot = CURRENT_SCREENSOT.lock().expect("Couldn't unlock screenshot").clone().expect("No screenshot available");
    
    if pos_bounds.x + pos_bounds.width > screenshot.width as i32 {
        pos_bounds.width = screenshot.width as i32 - pos_bounds.x;
    }
    if pos_bounds.y + pos_bounds.height > screenshot.height as i32 {
        pos_bounds.height = screenshot.height as i32 - pos_bounds.y;
    }

    // Crop the screenshot
    let mut img: ImageBuffer<Rgba<_>, Vec<u8>> = image::ImageBuffer::from_raw(
        screenshot.width as u32,
        screenshot.height as u32,
        screenshot.bytes
    ).unwrap();
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

    leptess.set_image("cropped.png").expect("Unable to set image");
    leptess.recognize();

    let text = leptess.get_utf8_text().unwrap();
    tx.send(text).expect("Unable to send text");
}

enum State<Type> {
    Empty,
    Delay(Duration),
    Ready(Type),
}

struct OCRThrottlerState<Type> {
    delay: Duration,
    last_value: Option<Type>,
    last_time: Instant,
}

impl<Type> OCRThrottlerState<Type> {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
            last_value: None,
            last_time: Instant::now(),
        }
    }

    fn get_state(&mut self) -> State<Type> {
        let elapsed = self.last_time.elapsed();
        if elapsed < self.delay {
            State::Delay(self.delay - elapsed)
        } else {
            if self.last_value.is_none() {
                State::Empty
            } else {
                State::Ready(self.last_value.take().unwrap())
            }
        }
    }

    fn put(&mut self, value: Type) {
        self.last_value = Some(value);
        self.last_time = Instant::now();
    }
}

struct OCRThrottlerThread<Type> {
    mutex: Arc<Mutex<OCRThrottlerState<Type>>>,
    thread: JoinHandle<()>
}

impl<Type> OCRThrottlerThread<Type> {
    fn new<RunFn, InitFn, InitData>(delay: Duration, mut f: RunFn, init_fn: InitFn) -> Self
    where
        Type: PartialEq + Send + 'static,
        RunFn: FnMut(Type, &mut InitData) + Send + 'static,
        InitFn: FnOnce() -> InitData + Send + 'static,
        InitData: 'static,
    {
        let mutex: Arc<Mutex<OCRThrottlerState<Type>>> = Arc::new(Mutex::new(OCRThrottlerState::new(delay)));

        let thread = thread::spawn({
            let mutex = mutex.clone();
            move || {
                let mut init_data: InitData = init_fn();

                loop {
                    let state = mutex.lock().unwrap().get_state();
                    match state {
                        State::Empty => thread::park(),
                        State::Delay(duration) => thread::sleep(duration),
                        State::Ready(data) => f(data, &mut init_data),
                    }
                }
            }
        });
        Self {
            mutex,
            thread
        }
    }
}

pub struct OCRThrottler<T>(OCRThrottlerThread<T>);

impl<T: PartialEq> OCRThrottler<T> {
    pub fn new<F, G, R>(delay: Duration, f: F, init_fn: G) -> Self
    where
        F: FnMut(T, &mut R) + Send + 'static,
        T: Send + 'static,
        G: FnOnce() -> R + Send + 'static,
        R: 'static,
    {
        Self(OCRThrottlerThread::new(delay, f, init_fn))
    }

    pub fn put(&self, data: T) {
        self.0.mutex.lock().unwrap().put(data);
        self.0.thread.thread().unpark();
    }
}