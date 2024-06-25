#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32
}

// Implement f32 -> i32 lossy conversion for Bounds
pub trait BoundsNumber: Copy {
    fn lossy_into(self) -> i32;
}

impl BoundsNumber for f32 {
    fn lossy_into(self) -> i32 {
        self as i32
    }
}

impl BoundsNumber for i32 {
    fn lossy_into(self) -> i32 {
        self
    }
}

impl Bounds {
    // Not sure if there's a better way of doing this, but this works for now
    pub fn new<X: BoundsNumber, Y: BoundsNumber, W: BoundsNumber, H: BoundsNumber>(x: X, y: Y, width: W, height: H) -> Self {
        Self {
            x: x.lossy_into(),
            y: y.lossy_into(),
            width: width.lossy_into(),
            height: height.lossy_into()
        }
    }
    
    pub fn from_center<X: BoundsNumber, Y: BoundsNumber, W: BoundsNumber, H: BoundsNumber>(x: X, y: Y, width: W, height: H) -> Self {
        let x = x.lossy_into();
        let y = y.lossy_into();
        let width = width.lossy_into();
        let height = height.lossy_into();
        Self {
            x: x - width / 2,
            y: y - height / 2,
            width,
            height
        }
    }

    pub fn set_center<X: BoundsNumber, Y: BoundsNumber>(&mut self, x: X, y: Y) {
        let x = x.lossy_into();
        let y = y.lossy_into();
        self.x = x - self.width / 2;
        self.y = y - self.height / 2;
    }

    pub fn set_origin<X: BoundsNumber, Y: BoundsNumber>(&mut self, x: X, y: Y) {
        self.x = x.lossy_into();
        self.y = y.lossy_into();
    }

    pub fn set_size<W: BoundsNumber, H: BoundsNumber>(&mut self, width: W, height: H) {
        self.width = width.lossy_into();
        self.height = height.lossy_into();
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
        let was_neg_x = self.width < 0;
        let was_neg_y = self.height < 0;
        let pos_self = self.to_positive_size();
        let (mut x, mut y, mut width, mut height) = (pos_self.x, pos_self.y, pos_self.width, pos_self.height);
        
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

        if was_neg_x {
            x += width;
            width = -width;
        }
        if was_neg_y {
            y += height;
            height = -height;
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
    pub start_drag_location: (i32, i32),
    pub start_drag_bounds_origin: (i32, i32),
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
        self.start_drag_location = (0, 0);
        self.start_drag_bounds_origin = (0, 0);
        
        self.ctrl_held = false;
    }
}