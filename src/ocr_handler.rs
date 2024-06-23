use std::time::Duration;

use debounce::EventDebouncer;

use crate::selection::Selection;

const DEBOUNE_TIME: Duration = Duration::from_millis(50);

pub(crate) struct OCRHandler {
    pub debouncer: EventDebouncer<Selection>
}

impl Default for OCRHandler {
    fn default() -> Self {
        OCRHandler {
            debouncer: EventDebouncer::new(DEBOUNE_TIME, |data| {
                println!("Debounced event: {:?}", data);
            })
        }
    }
}

impl OCRHandler {
    pub fn put(&self, data: Selection) {
        self.debouncer.put(data);
    }
}