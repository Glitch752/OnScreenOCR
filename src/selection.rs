#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub(crate) struct Selection {
    pub bounds: Bounds,

    pub mouse_down: bool,
    pub shift_held: bool,
}

impl Selection {
    pub fn reset(&mut self) {
        self.bounds.x = 0;
        self.bounds.y = 0;
        self.bounds.width = 0;
        self.bounds.height = 0;
        self.mouse_down = false;
        self.shift_held = false;
    }
}