use std::{sync::{mpsc, Arc, Mutex}, time::{Duration, Instant}};
use std::thread::{self, JoinHandle};

use crate::{screenshot::Screenshot, selection::{Bounds, Selection}, settings::{SettingsManager, TesseractSettings}};

pub static LATEST_SCREENSHOT_PATH: &str = "latest.png";

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub(crate) enum OCREvent {
    SelectionChanged(Bounds),
    SettingsUpdated(TesseractSettings),
    ScreenshotChanged(Screenshot),
    FormatOptionChanged(FormatOptions),
}

impl PartialEq for OCREvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OCREvent::SelectionChanged(_), OCREvent::SelectionChanged(_)) => true,
            (OCREvent::SettingsUpdated(_), OCREvent::SettingsUpdated(_)) => true,
            (OCREvent::ScreenshotChanged(_), OCREvent::ScreenshotChanged(_)) => true,
            _ => false
        }
    }
}

pub(crate) struct OCRHandler {
    pub throttler: OCRThrottler<OCREvent>,
    pub ocr_result_receiver: mpsc::Receiver<String>,
    pub ocr_preview_text: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FormatOptions {
    reformat_and_correct: bool,
    maintain_newlines: bool,
}

impl FormatOptions {
    pub fn from_settings(settings: &SettingsManager) -> Self {
        Self {
            reformat_and_correct: settings.reformat_and_correct,
            maintain_newlines: settings.maintain_newline,
        }
    }
}

struct InitData {
    tx: mpsc::Sender<String>,
    tess_api: leptess::tesseract::TessApi,
    screenshot_size: (u32, u32),
    format_options: FormatOptions,
    last_selection_bounds: Option<Bounds>, // Used to recalculate the same OCR when language changes

    hyphenated_word_list_cache: Vec<String>,
}

fn configure_tesseract(tesseract_settings: TesseractSettings) -> leptess::tesseract::TessApi {
    let mut tess_api = leptess::tesseract::TessApi::new(Some("./tessdata"), &tesseract_settings.ocr_language_code).expect("Unable to create Tesseract instance");
    tesseract_settings.configure_tesseract(&mut tess_api);
    tess_api
}

impl OCRHandler {
    pub fn new(initial_format_options: FormatOptions) -> Self {
        let (tx, rx) = mpsc::channel::<String>();
        let tesseract_settings = TesseractSettings::default();
        OCRHandler {
            throttler: OCRThrottler::new::<_, _, InitData>(
                DEBOUNE_TIME,
                move |event, init_data| match event {
                    OCREvent::SelectionChanged(bounds) => {
                        init_data.last_selection_bounds = Some(bounds);
                        perform_ocr(bounds, init_data);
                    }
                    OCREvent::FormatOptionChanged(format_options) => {
                        init_data.format_options = format_options;
                        if let Some(bounds) = init_data.last_selection_bounds {
                            perform_ocr(bounds, init_data);
                        }
                    }
                    OCREvent::SettingsUpdated(tesseract_settings) => {
                        init_data.hyphenated_word_list_cache = get_hyphenated_word_list_cache(&tesseract_settings.ocr_language_code);

                        init_data.tess_api = configure_tesseract(tesseract_settings);
                        let img = leptess::leptonica::pix_read(std::path::Path::new(LATEST_SCREENSHOT_PATH)).expect("Unable to read image");
                        init_data.tess_api.set_image(&img);
                        if let Some(bounds) = init_data.last_selection_bounds {
                            perform_ocr(bounds, init_data);
                        }
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
                move || {
                    InitData {
                        hyphenated_word_list_cache: get_hyphenated_word_list_cache(&tesseract_settings.ocr_language_code),
                        tess_api: configure_tesseract(tesseract_settings),
                        tx,
                        screenshot_size: (0, 0),
                        format_options: initial_format_options,
                        last_selection_bounds: None
                    }
                },
            ),
            ocr_result_receiver: rx,
            ocr_preview_text: None
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
    }

    pub fn format_option_changed(&mut self, format_options: FormatOptions) {
        self.throttler.put(OCREvent::FormatOptionChanged(format_options));
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

    let mut text = tesseract_api.get_utf8_text().unwrap_or("".to_string());

    if init_data.format_options.reformat_and_correct {
        let mut corrected_text = text.clone();
        if init_data.format_options.reformat_and_correct {
            corrected_text = reformat_and_correct_text(corrected_text, init_data);
        }
        init_data.tx.send(corrected_text).expect("Unable to send text");
    } else {
        if !init_data.format_options.maintain_newlines {
            text = text.replace("\n", " ");
        }
        init_data.tx.send(text).expect("Unable to send text");
    }
}

fn get_hyphenated_word_list_cache(language_code: &str) -> Vec<String> {
    let path = format!("./correction_data/hyphenated/{}.txt", language_code);
    // If the file doesn't exist, return an empty list
    if !std::fs::try_exists(&path).unwrap_or(false) {
        return Vec::new();
    }

    // The files are split by newlines
    let file = std::fs::read_to_string(path).expect("Unable to read hyphenated word list");
    file.lines().map(|x| x.to_string()).collect()
}

fn reformat_and_correct_text(text: String, init_data: &mut InitData) -> String {
    // 1. If a line ends with a hyphen and the word isn't detected to be a hyphenated word, remove the hyphen
    let hyphenated_words = &init_data.hyphenated_word_list_cache;
    let mut lines = text.lines().map(|x| format!("{}\n", x.to_string())).collect::<Vec<String>>();

    // Remove empty lines. This may not be an ideal solution, but it works for now.
    lines.retain(|x| !x.trim().is_empty());

    let lines_loop = lines.clone();
    for (line, i) in lines_loop.iter().zip(0..) {
        if line.ends_with("-\n") {
            // The last word of this line plus the first word of the next is our query
            let last_word = line.split_whitespace().last().unwrap_or("");
            let next_first_word = lines_loop.get(i + 1).map(|x| x.split_whitespace().next()).flatten().unwrap_or("");
            let query = format!("{}{}", last_word, next_first_word);
            if hyphenated_words.contains(&query) {
                continue;
            }

            // Remove the hyphen and the newline
            lines.get_mut(i).map(|x| {
                let last = x.pop();
                if last == Some('\n') {
                    x.pop();
                }
            });
            // Add a newline to the end of the word on the next line to roughly maintain the same formatting
            lines.get_mut(i + 1).map(|x| {
                let words = x.split_whitespace().collect::<Vec<&str>>();
                if let Some(first) = words.first() {
                    *x = format!("{}\n{}", first, &x[first.len()..].trim_start());
                }
            });
        }
    }
    let mut text = lines.join("");

    if !init_data.format_options.maintain_newlines {
        text = text.replace("\n", " ");
    }

    return text;
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