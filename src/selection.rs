#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Selection {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,

    pub mouse_down: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Selection {
            x: 300,
            y: 300,
            width: 500,
            height: 500,
            mouse_down: false,
        }
    }
}