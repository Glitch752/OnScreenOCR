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
    pub ctrl_held: bool,

    drag_state: DraggingEditState,
}

enum PolygonHitResult {
    None,
    Vertex(usize),
    Edge(usize)
}

#[derive(Debug, Clone, Default, PartialEq)]
enum DraggingEditState {
    #[default]
    None,
    NewBox(NewBoxEditState),
    ShiftSelection(ShiftSelectionEditState),
    PolygonVertex(PolygonVertexEditState),
}

#[derive(Debug, Clone, PartialEq)]
struct NewBoxEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
struct PolygonVertexEditState {
    vertex_index: usize
}

#[derive(Debug, Clone, PartialEq)]
struct ShiftSelectionEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32)
}

impl Selection {
    pub fn reset(&mut self) {
        self.polygon.clear();
        self.bounds = Bounds::default();

        self.mouse_down = false;

        self.shift_held = false;
        self.drag_state = DraggingEditState::None;
        
        self.ctrl_held = false;
    }

    pub fn cursor_moved(
        &mut self,
        mouse_position: (i32, i32),
        screen_size: (u32, u32)
    ) -> bool {
        // If shift is held, move the selection instead of resizing
        let (x, y) = (mouse_position.0, mouse_position.1);
        if !self.mouse_down {
            return false;
        }

        match self.drag_state {
            DraggingEditState::None => {},
            DraggingEditState::NewBox(ref state) => {
                self.bounds.width = x - self.bounds.x;
                self.bounds.height = y - self.bounds.y;
                self.polygon.set_from_bounds(&self.bounds);
                
                if self.shift_held {
                    self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                        start_location: state.start_location,
                        start_origin: self.polygon.get_origin()
                    });
                }
            }
            DraggingEditState::ShiftSelection(ref state) => {
                let (start_x, start_y) = state.start_location;
                let (start_bounds_x, start_bounds_y) = state.start_origin;
                let (dx, dy) = (x - start_x, y - start_y);
                self.polygon.set_origin(start_bounds_x + dx as f32, start_bounds_y + dy as f32);
                self.polygon.clamp_to_screen(screen_size);
                self.bounds.enclose_polygon(&self.polygon);
                
                if !self.shift_held {
                    self.drag_state = DraggingEditState::NewBox(NewBoxEditState {
                        start_location: state.start_location,
                        start_origin: self.polygon.get_origin()
                    });
                }
            }
            DraggingEditState::PolygonVertex(ref vertex) => {
                self.polygon.vertices[vertex.vertex_index].x = x as f32;
                self.polygon.vertices[vertex.vertex_index].y = y as f32;
                
                let pos = self.should_merge_surrounding_edges(vertex.vertex_index);
                if pos.is_some() {
                    let (x, y) = pos.unwrap();
                    self.polygon.vertices[vertex.vertex_index].x = x;
                    self.polygon.vertices[vertex.vertex_index].y = y;
                }
                self.bounds.enclose_polygon(&self.polygon);
            }
        }

        true
    }

    pub fn mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mouse_position: (i32, i32),
        icon_context: &mut IconContext
    ) -> bool {
        let (x, y) = (mouse_position.0, mouse_position.1);
        let mut completely_moved = false;

        if state == winit::event::ElementState::Pressed {
            let mut new_box = false;
            if icon_context.settings.use_polygon {
                let hit = self.detect_polygon_hit(mouse_position);
                match hit {
                    PolygonHitResult::Vertex(index) => {
                        if button == MouseButton::Right {
                            if self.polygon.vertices.len() <= 3 {
                                return false;
                            }
                            self.polygon.vertices.remove(index);
                        } else {
                            self.drag_state = DraggingEditState::PolygonVertex(PolygonVertexEditState { vertex_index: index });
                        }
                    },
                    PolygonHitResult::Edge(index) => {
                        if button == MouseButton::Right {
                            if self.polygon.vertices.len() <= 3 {
                                return false;
                            }
                            self.polygon.vertices.remove(index);
                            self.polygon.vertices.remove(index + 1 % self.polygon.vertices.len());
                        } else {
                            // Split the edge
                            let new_vertex = Vertex {
                                x: x as f32,
                                y: y as f32
                            };

                            self.polygon.vertices.insert(index + 1, new_vertex);
                            self.drag_state = DraggingEditState::PolygonVertex(PolygonVertexEditState { vertex_index: index + 1 });
                        }
                    },
                    PolygonHitResult::None => {
                        new_box = true;
                    }
                }
            } else {
                new_box = true;
            }
            
            if new_box {
                if !self.shift_held {
                    self.bounds.x = x;
                    self.bounds.y = y;
                    self.bounds.width = 0;
                    self.bounds.height = 0;
                    self.polygon.set_from_bounds(&self.bounds);

                    completely_moved = true;
                    self.drag_state = DraggingEditState::NewBox(NewBoxEditState {
                        start_location: (x, y),
                        start_origin: self.polygon.get_origin()
                    });
                } else {
                    self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                        start_location: (x, y),
                        start_origin: self.polygon.get_origin()
                    });
                }
            }
            self.mouse_down = true;
        } else {
            match &self.drag_state {
                DraggingEditState::PolygonVertex(index) => {
                    if self.polygon.vertices.len() <= 3 {
                        return false;
                    }
                    
                    if self.should_merge_surrounding_edges(index.vertex_index).is_some() {
                        self.polygon.vertices.remove(index.vertex_index);
                    }
                }
                _ => {}
            }
            self.drag_state = DraggingEditState::None;
            self.mouse_down = false;
        }

        icon_context.settings_panel_visible = false;

        completely_moved
    }

    fn should_merge_surrounding_edges(&self, vertex_index: usize) -> Option<(f32, f32)> {
        // If the two surrounding edges are within a small angle of each other, merge them
        let vertex = self.polygon.vertices[vertex_index];
        let prev_vertex = self.polygon.vertices[(vertex_index + self.polygon.vertices.len() - 1) % self.polygon.vertices.len()];
        let next_vertex = self.polygon.vertices[(vertex_index + 1) % self.polygon.vertices.len()];

        let (prev_dx, prev_dy) = (vertex.x - prev_vertex.x, vertex.y - prev_vertex.y);
        let (next_dx, next_dy) = (next_vertex.x - vertex.x, next_vertex.y - vertex.y);

        let prev_angle = prev_dy.atan2(prev_dx);
        let next_angle = next_dy.atan2(next_dx);

        let angle_diff = (prev_angle - next_angle).abs();

        let angle_margin: f32 = if self.shift_held || self.ctrl_held { 1.0 } else { 10.0 };
        if angle_diff < angle_margin.to_radians() {
            // Get the position on the line segment that the vertex would effectively be at -- the nearest position on the line segment
            let t = ((vertex.x - prev_vertex.x) * (next_vertex.x - prev_vertex.x) + (vertex.y - prev_vertex.y) * (next_vertex.y - prev_vertex.y)) / ((next_vertex.x - prev_vertex.x).powi(2) + (next_vertex.y - prev_vertex.y).powi(2));
            let x = prev_vertex.x + t * (next_vertex.x - prev_vertex.x);
            let y = prev_vertex.y + t * (next_vertex.y - prev_vertex.y);

            Some((x, y))
        } else {
            None
        }
    }

    pub fn get_device_coords_polygon(&self, window_width: f32, window_height: f32) -> Polygon {
        let mut polygon = Polygon::new();
        for vertex in self.polygon.vertices.iter() {
            polygon.vertices.push(Vertex {
                x: vertex.x / window_width,
                y: vertex.y / window_height
            });
        }
        polygon
    }

    fn detect_polygon_hit(&self, mouse_position: (i32, i32)) -> PolygonHitResult {
        let margin = 5.0;

        for vertex in &self.polygon.vertices {
            let (x, y) = (vertex.x, vertex.y);
            if (x - mouse_position.0 as f32).abs() < margin && (y - mouse_position.1 as f32).abs() < margin {
                return PolygonHitResult::Vertex(self.polygon.vertices.iter().position(|v| v.x == x && v.y == y).unwrap());
            }
        }

        for i in 0..self.polygon.vertices.len() {
            let vertex1 = self.polygon.vertices[i];
            let vertex2 = self.polygon.vertices[(i + 1) % self.polygon.vertices.len()];
            let (x1, y1) = (vertex1.x, vertex1.y);
            let (x2, y2) = (vertex2.x, vertex2.y);

            let dx = x2 - x1;
            let dy = y2 - y1;
            let d = ((x1 - mouse_position.0 as f32) * dy - (y1 - mouse_position.1 as f32) * dx).abs() / (dx * dx + dy * dy).sqrt();
            if d < margin {
                // Ensure the point is within the line segment
                let dot = (mouse_position.0 as f32 - x1) * dx + (mouse_position.1 as f32 - y1) * dy;
                if dot >= 0.0 && dot <= dx * dx + dy * dy {
                    return PolygonHitResult::Edge(i);
                }
            }
        }

        PolygonHitResult::None
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