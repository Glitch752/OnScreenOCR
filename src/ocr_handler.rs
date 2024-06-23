use std::cell::RefCell;
use std::time::Duration;

use debounce::EventDebouncer;

use crate::selection::Selection;

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

thread_local!(static LATEST_SELECTION: RefCell<Selection> = RefCell::new(Selection::default()));

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OCREvent {
    SelectionChanged,
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
            .put(OCREvent::SelectionChanged);

        LATEST_SELECTION.with(|latest_selection_cell| {
            *latest_selection_cell.borrow_mut() = latest_selection;
        });
    }

    fn initialize_debouncer(&mut self) {
        self.debouncer = Some(EventDebouncer::new(DEBOUNE_TIME, move |_data| {
            let latest_selection = LATEST_SELECTION.with(|latest_selection| {
                let latest_selection = latest_selection.borrow();
                let latest_selection = latest_selection.clone();
                latest_selection
            });

            println!(
                "Debounced event; latest selection: {:?} {:?}",
                latest_selection, _data
            );
        }));
    }
}
