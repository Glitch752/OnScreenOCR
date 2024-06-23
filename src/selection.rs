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

    pub fn to_positive_size(&self) -> Bounds {
        let (mut x, mut y, mut width, mut height) = (self.x, self.y, self.width, self.height);
        if width < 0 {
            x += width;
            width = -width;
        }
        if height < 0 {
            y += height;
            height = -height;
        }
        Bounds {
            x, y, width, height
        }
    }

    pub fn clamp_to_screen(&mut self, screen_size: (u32, u32)) -> () {
        let pos_self = self.to_positive_size();
        let (mut x, mut y, width, height) = (pos_self.x, pos_self.y, pos_self.width, pos_self.height);
        
        if x < 0 {
            x = 0;
        }
        if y < 0 {
            y = 0;
        }
        if x + width > screen_size.0 as i32 {
            x = screen_size.0 as i32 - width;
        }
        if y + height > screen_size.1 as i32 {
            y = screen_size.1 as i32 - height;
        }

        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn contains(&self, point: (i32, i32)) -> bool {
        let (x, y) = point;
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub(crate) struct Selection {
    pub bounds: Bounds,

    pub mouse_down: bool,
    pub shift_held: bool,
    pub ctrl_held: bool,
}

impl Selection {
    pub fn reset(&mut self) {
        self.bounds.x = 0;
        self.bounds.y = 0;
        self.bounds.width = 0;
        self.bounds.height = 0;
        self.mouse_down = false;
        self.shift_held = false;
        self.ctrl_held = false;
    }
}