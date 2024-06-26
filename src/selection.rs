use winit::event::{ElementState, MouseButton};

use crate::renderer::IconContext;

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

    pub fn enclose_polygon(&mut self, polygon: &Polygon) {
        if polygon.vertices.is_empty() {
            *self = Bounds::default();
            return;
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for vertex in &polygon.vertices {
            let x = vertex.x;
            let y = vertex.y;
            if x < min_x as f32 {
                min_x = x as i32;
            }
            if y < min_y as f32 {
                min_y = y as i32;
            }
            if x > max_x as f32 {
                max_x = x as i32;
            }
            if y > max_y as f32 {
                max_y = y as i32;
            }
        }

        self.x = min_x;
        self.y = min_y;
        self.width = max_x - min_x;
        self.height = max_y - min_y;
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Selection {
    pub bounds: Bounds,
    pub polygon: Polygon,

    pub mouse_down: bool,
    pub shift_held: bool,
    pub start_drag_location: (i32, i32),
    pub start_drag_origin: (f32, f32),
    pub ctrl_held: bool,
}

impl Selection {
    pub fn reset(&mut self) {
        self.polygon.clear();
        self.bounds = Bounds::default();

        self.mouse_down = false;

        self.shift_held = false;
        self.start_drag_location = (0, 0);
        self.start_drag_origin = (0., 0.);
        
        self.ctrl_held = false;
    }

    pub fn cursor_moved(
        &mut self,
        mouse_position: (i32, i32),
        screen_size: (u32, u32),
        icon_context: &IconContext
    ) -> bool {
        // If shift is held, move the selection instead of resizing
        let (x, y) = (mouse_position.0, mouse_position.1);
        if !self.mouse_down {
            return false;
        }

        if !self.shift_held {
            self.bounds.width = x - self.bounds.x;
            self.bounds.height = y - self.bounds.y;
            self.polygon.set_from_bounds(&self.bounds);
        } else {
            let (start_x, start_y) = self.start_drag_location;
            let (start_bounds_x, start_bounds_y) = self.start_drag_origin;
            let (dx, dy) = (x - start_x, y - start_y);
            self.polygon.set_origin(start_bounds_x + dx as f32, start_bounds_y + dy as f32);
            self.polygon.clamp_to_screen(screen_size);
            self.bounds.enclose_polygon(&self.polygon);
        }

        true
    }

    pub fn mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mouse_position: (i32, i32),
        screen_size: (u32, u32),
        icon_context: &mut IconContext
    ) -> bool {
        let (x, y) = (mouse_position.0, mouse_position.1);
        let mut moved = false;

        if state == winit::event::ElementState::Pressed {
            if !self.shift_held {
                self.bounds.x = x;
                self.bounds.y = y;
                self.bounds.width = 0;
                self.bounds.height = 0;
                moved = true;
            } else {
                self.start_drag_location = (x, y);
                self.start_drag_origin = self.polygon.get_origin();
            }
            self.mouse_down = true;
        } else {
            self.mouse_down = false;
        }

        icon_context.settings_panel_visible = false;

        moved
    }
}

#[repr(C)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Polygon {
    pub vertices: Vec<Vertex>
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub x: f32,
    pub y: f32
}

impl Vertex {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Polygon {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new()
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    pub fn get_origin(&self) -> (f32, f32) {
        let min_x = self.vertices.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
        let min_y = self.vertices.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
        (min_x, min_y)
    }

    pub fn set_origin(&mut self, x: f32, y: f32) {
        let current_origin = self.get_origin();

        for vertex in self.vertices.iter_mut() {
            vertex.x -= current_origin.0;
            vertex.y -= current_origin.1;
            vertex.x += x;
            vertex.y += y;
        }
    }

    pub fn move_by(&mut self, dx: f32, dy: f32) {
        for vertex in self.vertices.iter_mut() {
            vertex.x += dx;
            vertex.y += dy;
        }
    }

    pub fn clamp_to_screen(&mut self, screen_size: (u32, u32)) -> () {
        if self.vertices.is_empty() {
            return;
        }

        let min_x = self.vertices.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
        let min_y = self.vertices.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
        let max_x = self.vertices.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max);
        let max_y = self.vertices.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);

        let mut dx = 0.0;
        let mut dy = 0.0;

        if min_x < 0.0 {
            dx = -min_x;
        }
        if min_y < 0.0 {
            dy = -min_y;
        }

        if max_x > screen_size.0 as f32 {
            dx = screen_size.0 as f32 - max_x;
        }
        if max_y > screen_size.1 as f32 {
            dy = screen_size.1 as f32 - max_y;
        }

        self.move_by(dx, dy);

        // If any vertices are still outside the screen, just clamp them
        for vertex in self.vertices.iter_mut() {
            if vertex.x < 0.0 {
                vertex.x = 0.0;
            }
            if vertex.y < 0.0 {
                vertex.y = 0.0;
            }
            if vertex.x > screen_size.0 as f32 {
                vertex.x = screen_size.0 as f32;
            }
            if vertex.y > screen_size.1 as f32 {
                vertex.y = screen_size.1 as f32;
            }
        }
    }

    pub fn deduplicate(&mut self) {
        let mut deduplicated: Vec<Vertex> = Vec::new();
        for vertex in &self.vertices {
            if deduplicated.iter().find(|v| v.x == vertex.x && v.y == vertex.y).is_none() {
                deduplicated.push(*vertex);
            }
        }
        self.vertices = deduplicated;
    }

    pub fn set_from_bounds(&mut self, bounds: &Bounds) {
        self.vertices = vec![
            Vertex::new(bounds.x as f32, bounds.y as f32),
            Vertex::new(bounds.x as f32 + bounds.width as f32, bounds.y as f32),
            Vertex::new(bounds.x as f32 + bounds.width as f32, bounds.y as f32 + bounds.height as f32),
            Vertex::new(bounds.x as f32, bounds.y as f32 + bounds.height as f32)
        ];
    }
}