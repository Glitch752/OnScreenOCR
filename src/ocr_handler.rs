use std::{sync::{mpsc, Arc, LazyLock, Mutex}, time::{Duration, Instant}};
use std::thread::{self, JoinHandle};

use crate::{screenshot::Screenshot, selection::{Bounds, Selection}, settings::TesseractSettings};

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

pub static CURRENT_SCREENSOT: LazyLock<Mutex<Box<Option<Screenshot>>>> = LazyLock::new(|| Mutex::new(Box::new(None)));

#[derive(Debug, Clone)]
pub(crate) enum OCREvent {
    SelectionChanged(Bounds),
    SettingsUpdated(TesseractSettings)
}

impl PartialEq for OCREvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OCREvent::SelectionChanged(_), OCREvent::SelectionChanged(_)) => true,
            (OCREvent::SettingsUpdated(_), OCREvent::SettingsUpdated(_)) => true,
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
}

fn configure_tesseract(tesseract_settings: TesseractSettings) -> leptess::tesseract::TessApi {
    let mut tess_api = leptess::tesseract::TessApi::new(Some("./tessdata"), &tesseract_settings.ocr_language_code).expect("Unable to create Tesseract instance");
    // tess_api.raw.set_variable(
    // lt.set_rectangle(10, 10, 200, 60);
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
                        perform_ocr(bounds, &mut init_data.tess_api, &init_data.tx);
                    }
                    OCREvent::SettingsUpdated(tesseract_settings) => {
                        init_data.tess_api = configure_tesseract(tesseract_settings);
                    }
                    OCREvent::ScreenshotChanged() => {
                        init_data.tess_api.set_image()
                    }
                },
                || {
                    InitData {
                        tess_api: configure_tesseract(tesseract_settings),
                        tx
                    }
                },
            ),
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
        self.throttler.put(OCREvent::SettingsUpdated(settings));
        self.throttler.put(OCREvent::SelectionChanged(self.last_selection_bounds.clone().unwrap()));
    }
}

fn perform_ocr(bounds: Bounds, tsseract_api: &mut leptess::tesseract::TessApi, tx: &mpsc::Sender<String>) {
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


    // Export to a png and save it under the current directory
    cropped_image.save("cropped.png").unwrap();

    tsseract_api.set_image("cropped.png").expect("Unable to set image");
    tsseract_api.recognize();

    let text = tsseract_api.get_utf8_text().unwrap();
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