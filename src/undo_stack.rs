use crate::selection::{Bounds, Polygon, Selection};

pub(crate) struct UndoStack {
    stack: Vec<SelectionSnapshot>,
    current_index: usize
}

impl UndoStack {
    pub fn new() -> Self {
        UndoStack {
            stack: Vec::new(),
            current_index: 0
        }
    }

    pub fn take_snapshot(&mut self, selection: &Selection) {
        self.current_index += 1;
        self.stack.truncate(self.current_index);
        self.stack.push(SelectionSnapshot::from(selection));
    }

    pub fn undo(&mut self, selection: &mut Selection) -> Result<(), ()> {
        if self.current_index > 0 {
            self.current_index -= 1;
            selection.bounds = self.stack[self.current_index].bounds.clone();
            selection.polygon = self.stack[self.current_index].polygon.clone();
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn redo(&mut self, selection: &mut Selection) -> Result<(), ()> {
        if self.current_index < self.stack.len() - 1 {
            self.current_index += 1;
            selection.bounds = self.stack[self.current_index].bounds.clone();
            selection.polygon = self.stack[self.current_index].polygon.clone();
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.current_index = 0;
    }
}

struct SelectionSnapshot {
    pub bounds: Bounds,
    pub polygon: Polygon
}

impl From<&Selection> for SelectionSnapshot {
    fn from(selection: &Selection) -> Self {
        SelectionSnapshot {
            bounds: selection.bounds.clone(),
            polygon: selection.polygon.clone()
        }
    }
}