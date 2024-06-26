use std::{sync::{mpsc, Arc, Mutex}, time::{Duration, Instant}};
use std::thread::{self, JoinHandle};

use crate::{screenshot::Screenshot, selection::{Bounds, Selection}, settings::TesseractSettings};

pub static LATEST_SCREENSHOT_PATH: &str = "latest.png";

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub(crate) enum OCREvent {
    SelectionChanged(Bounds),
    SettingsUpdated(TesseractSettings, Bounds),
    ScreenshotChanged(Screenshot)
}

impl PartialEq for OCREvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OCREvent::SelectionChanged(_), OCREvent::SelectionChanged(_)) => true,
            (OCREvent::SettingsUpdated(_, _), OCREvent::SettingsUpdated(_, _)) => true,
            (OCREvent::ScreenshotChanged(_), OCREvent::ScreenshotChanged(_)) => true,
            _ => false
        }
    }
}

pub(crate) struct OCRHandler {
    pub throttler: OCRThrottler<OCREvent>,
    pub ocr_result_receiver: mpsc::Receiver<String>,
    pub ocr_preview_text: Option<String>,
    pub last_selection_bounds: Option<Bounds>, // Used to recalculate the same OCR when language changes
}

struct InitData {
    tx: mpsc::Sender<String>,
    tess_api: leptess::tesseract::TessApi,
    screenshot_size: (u32, u32)
}

fn configure_tesseract(tesseract_settings: TesseractSettings) -> leptess::tesseract::TessApi {
    let mut tess_api = leptess::tesseract::TessApi::new(Some("./tessdata"), &tesseract_settings.ocr_language_code).expect("Unable to create Tesseract instance");
    tesseract_settings.configure_tesseract(&mut tess_api);
    tess_api
}

impl OCRHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<String>();
        let tesseract_settings = TesseractSettings::default();
        OCRHandler {
            throttler: OCRThrottler::new::<_, _, InitData>(
                DEBOUNE_TIME,
                move |event, init_data| match event {
                    OCREvent::SelectionChanged(bounds) => {
                        perform_ocr(bounds, init_data);
                    }
                    OCREvent::SettingsUpdated(tesseract_settings, bounds) => {
                        init_data.tess_api = configure_tesseract(tesseract_settings);
                        let img = leptess::leptonica::pix_read(std::path::Path::new(LATEST_SCREENSHOT_PATH)).expect("Unable to read image");
                        init_data.tess_api.set_image(&img);
                        perform_ocr(bounds, init_data);
                    }
                    OCREvent::ScreenshotChanged(screenshot) => {
                        init_data.tess_api.raw.set_image(
                            &screenshot.bytes,
                            screenshot.width as i32,
                            screenshot.height as i32,
                            4,
                            4 * screenshot.width as i32
                        ).expect("Unable to set image");
                        init_data.screenshot_size = (screenshot.width as u32, screenshot.height as u32);
                        init_data.tess_api.set_source_resolution(70); // Doesn't matter to us -- just suppress the warning

                        // Also save screenshot as latest.png for screenshot functionality and debugging
                        let screenshot_image = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_vec(screenshot.width as u32, screenshot.height as u32, screenshot.bytes.clone()).expect("Unable to create image buffer");
                        screenshot_image.save(LATEST_SCREENSHOT_PATH).expect("Unable to save latest.png");
                    }
                },
                || {
                    InitData {
                        tess_api: configure_tesseract(tesseract_settings),
                        tx,
                        screenshot_size: (0, 0)
                    }
                },
            ),
            ocr_result_receiver: rx,
            ocr_preview_text: None,
            last_selection_bounds: None
        }
    }

    pub fn set_screenshot(&mut self, screenshot: Screenshot) {
        self.throttler.put(OCREvent::ScreenshotChanged(screenshot));
    }

    pub fn before_reopen_window(&mut self) {
        self.ocr_preview_text = None;
    }

    pub fn selection_changed(&mut self, latest_selection: Selection) {
        self.throttler.put(OCREvent::SelectionChanged(latest_selection.bounds.clone()));
        
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

    pub fn update_ocr_settings(&mut self, settings: TesseractSettings) {
        self.throttler.put(OCREvent::SettingsUpdated(settings, self.last_selection_bounds.unwrap_or_default()));
    }
}

fn perform_ocr(bounds: Bounds, init_data: &mut InitData) {
    let mut pos_bounds = bounds.to_positive_size();
    if pos_bounds.width < 5 || pos_bounds.height < 5 {
        return;
    }
    
    if pos_bounds.x + pos_bounds.width > init_data.screenshot_size.0 as i32 {
        pos_bounds.width = init_data.screenshot_size.0 as i32 - pos_bounds.x;
    }
    if pos_bounds.y + pos_bounds.height > init_data.screenshot_size.1 as i32 {
        pos_bounds.height = init_data.screenshot_size.1 as i32 - pos_bounds.y;
    }

    let tesseract_api = &mut init_data.tess_api;
    tesseract_api.set_rectangle(pos_bounds.x, pos_bounds.y, pos_bounds.width, pos_bounds.height);
    tesseract_api.recognize();

    let text = tesseract_api.get_utf8_text().unwrap_or("".to_string());
    init_data.tx.send(text).expect("Unable to send text");
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
    pub fn new<F, G, R>(delay: Duration, f: F, init_fn: G,) -> Self
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