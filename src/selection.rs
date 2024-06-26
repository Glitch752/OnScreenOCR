use winit::{event::{ElementState, KeyEvent, MouseButton}, keyboard::{Key, NamedKey}};

use crate::renderer::{IconContext, SmoothFadeAnimation};

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
    ShiftPolygonEdge(PolygonEdgeEditState),

    CreatePolygon,
}

#[derive(Debug, Clone, PartialEq)]
struct NewBoxEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
struct PolygonVertexEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32),
    vertex_index: usize
}

#[derive(Debug, Clone, PartialEq)]
struct PolygonEdgeEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32),
    edge_index: usize
}

#[derive(Debug, Clone, PartialEq)]
struct ShiftSelectionEditState {
    start_location: (i32, i32),
    start_origin: (f32, f32),
    from_edge: bool
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Selection {
    pub bounds: Bounds,
    pub polygon: Polygon,

    pub mouse_down: bool,
    pub shift_held: bool,
    pub ctrl_held: bool,

    drag_state: DraggingEditState,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SelectionInputResult {
    Nothing,
    Changed,
    CompletelyMoved,
    SelectionFinished
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
        screen_size: (u32, u32),
        icon_context: &mut IconContext
    ) -> SelectionInputResult {
        // If shift is held, move the selection instead of resizing
        let (x, y) = (mouse_position.0, mouse_position.1);

        let hit = self.detect_polygon_hit(mouse_position);
        match hit {
            PolygonHitResult::Vertex(index) => {
                self.polygon.hovered_edge = None;
                self.polygon.hovered_vertex = Some(index);
            }
            PolygonHitResult::Edge(index) => {
                self.polygon.hovered_edge = Some(index);
                self.polygon.hovered_vertex = None;
            }
            _ => {
                self.polygon.hovered_edge = None;
                self.polygon.hovered_vertex = None;
            }
        }

        match self.drag_state {
            DraggingEditState::None => {},
            DraggingEditState::NewBox(ref state) => {
                if !self.mouse_down {
                    return SelectionInputResult::Nothing;
                }

                self.bounds.x = state.start_origin.0 as i32;
                self.bounds.y = state.start_origin.1 as i32;
                self.bounds.width = x - self.bounds.x;
                self.bounds.height = y - self.bounds.y;
                self.polygon.set_from_bounds(&self.bounds);
                
                if self.shift_held {
                    self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                        start_location: (x, y),
                        start_origin: self.polygon.get_origin(),
                        from_edge: false
                    });
                }
                return SelectionInputResult::Changed;
            }
            DraggingEditState::ShiftSelection(ref state) => {
                if !self.mouse_down {
                    return SelectionInputResult::Nothing;
                }
                
                let (start_x, start_y) = state.start_location;
                let (start_bounds_x, start_bounds_y) = state.start_origin;
                let (dx, dy) = (x - start_x, y - start_y);
                self.polygon.set_origin(start_bounds_x + dx as f32, start_bounds_y + dy as f32);
                self.polygon.clamp_to_screen(screen_size);
                
                if !self.shift_held {
                    let origin = self.polygon.get_origin();
                    if state.from_edge {
                        self.drag_state = DraggingEditState::ShiftPolygonEdge(PolygonEdgeEditState {
                            edge_index: self.get_nearest_edge_index(x, y),
                            start_origin: origin,
                            start_location: (origin.0 as i32, origin.1 as i32)
                        });
                    } else {
                        self.drag_state = DraggingEditState::PolygonVertex(PolygonVertexEditState {
                            vertex_index: self.get_nearest_vertex_index(x, y),
                            start_origin: origin,
                            start_location: (origin.0 as i32, origin.1 as i32)
                        });
                    }
                }
                
                self.bounds.enclose_polygon(&self.polygon);
                return SelectionInputResult::Changed;
            }
            DraggingEditState::ShiftPolygonEdge(ref edge) => {
                if !self.mouse_down {
                    return SelectionInputResult::Nothing;
                }

                if icon_context.settings.use_polygon {
                    let (start_x, start_y) = edge.start_location;
                    let (start_bounds_x, start_bounds_y) = edge.start_origin;
                    let (dx, dy) = (x - start_x, y - start_y);
                    self.polygon.set_edge_origin(edge.edge_index, (start_bounds_x + dx as f32, start_bounds_y + dy as f32));
                    self.polygon.clamp_to_screen(screen_size);
                    
                    if !self.shift_held {
                        self.check_edge_split_input(x, y, edge.edge_index, icon_context.settings.use_polygon);
                    }
                
                    self.bounds.enclose_polygon(&self.polygon);
                } else {
                    let first_vertex = edge.edge_index;
                    let second_vertex = (edge.edge_index + 1) % self.polygon.vertices.len();

                    if first_vertex % 2 == 0 {
                        let y = edge.start_origin.1 + y as f32 - edge.start_location.1 as f32;
                        self.polygon.vertices[first_vertex].y = y;
                        self.polygon.vertices[second_vertex].y = y;
                    } else {
                        let x = edge.start_origin.0 + x as f32 - edge.start_location.0 as f32;
                        self.polygon.vertices[first_vertex].x = x;
                        self.polygon.vertices[second_vertex].x = x;
                    }
                }

                if self.shift_held && !icon_context.settings.use_polygon {
                    self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                        start_location: (x, y),
                        start_origin: self.polygon.get_origin(),
                        from_edge: true
                    });
                }

                self.bounds.enclose_polygon(&self.polygon);
                return SelectionInputResult::Changed;
            }
            DraggingEditState::PolygonVertex(ref vertex) => {
                if !self.mouse_down {
                    return SelectionInputResult::Nothing;
                }

                if icon_context.settings.use_polygon {
                    self.polygon.vertices[vertex.vertex_index].x = x as f32;
                    self.polygon.vertices[vertex.vertex_index].y = y as f32;
                    
                    let pos = self.should_merge_surrounding_edges(vertex.vertex_index);
                    if pos.is_some() {
                        let (x, y) = pos.unwrap();
                        self.polygon.vertices[vertex.vertex_index].x = x;
                        self.polygon.vertices[vertex.vertex_index].y = y;
                    }

                    let deduplicated_pos = self.polygon.should_deduplicate(vertex.vertex_index);
                    if deduplicated_pos.is_some() {
                        let (x, y) = deduplicated_pos.unwrap();
                        self.polygon.vertices[vertex.vertex_index].x = x;
                        self.polygon.vertices[vertex.vertex_index].y = y;
                    }
                } else {
                    let curr_vertex = vertex.vertex_index;
                    let next_vertex = (vertex.vertex_index + 1) % self.polygon.vertices.len();
                    let prev_vertex = (vertex.vertex_index + self.polygon.vertices.len() - 1) % self.polygon.vertices.len();
                    
                    let (dx, dy) = (x - vertex.start_location.0, y - vertex.start_location.1);
                    self.polygon.vertices[curr_vertex].x = vertex.start_origin.0 + dx as f32;
                    self.polygon.vertices[curr_vertex].y = vertex.start_origin.1 + dy as f32;
                    
                    if vertex.vertex_index % 2 == 0 {
                        self.polygon.vertices[prev_vertex].x = vertex.start_origin.0 + dx as f32;
                        self.polygon.vertices[next_vertex].y = vertex.start_origin.1 + dy as f32;
                    } else {
                        self.polygon.vertices[next_vertex].x = vertex.start_origin.0 + dx as f32;
                        self.polygon.vertices[prev_vertex].y = vertex.start_origin.1 + dy as f32;
                    }
                }

                if self.shift_held {
                    self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                        start_location: (x, y),
                        start_origin: self.polygon.get_origin(),
                        from_edge: false
                    });
                }

                self.bounds.enclose_polygon(&self.polygon);
                return SelectionInputResult::Changed;
            }

            DraggingEditState::CreatePolygon => {
                if self.polygon.vertices.len() <= 1 {
                    self.polygon.vertices.push(Vertex::new(x as f32, y as f32));
                }

                let last_vertex = self.polygon.vertices.iter_mut().last().unwrap();
                last_vertex.x = x as f32;
                last_vertex.y = y as f32;

                self.bounds.enclose_polygon(&self.polygon);

                if !self.ctrl_held {
                    self.end_polygon_creation();
                    return SelectionInputResult::SelectionFinished;
                }
                return SelectionInputResult::Changed;
            }
        }

        SelectionInputResult::Nothing
    }

    pub fn get_nearest_vertex_index(&self, x: i32, y: i32) -> usize {
        let mut min_distance = f32::INFINITY;
        let mut min_index = 0;
        for (i, vertex) in self.polygon.vertices.iter().enumerate() {
            let distance = ((vertex.x - x as f32).powi(2) + (vertex.y - y as f32).powi(2)).sqrt();
            if distance < min_distance {
                min_distance = distance;
                min_index = i;
            }
        }

        min_index
    }

    pub fn get_nearest_edge_index(&self, x: i32, y: i32) -> usize {
        let mut min_distance = f32::INFINITY;
        let mut min_index = 0;
        for i in 0..self.polygon.vertices.len() {
            let vertex1 = &self.polygon.vertices[i];
            let vertex2 = &self.polygon.vertices[(i + 1) % self.polygon.vertices.len()];
            let (x1, y1) = (vertex1.x, vertex1.y);
            let (x2, y2) = (vertex2.x, vertex2.y);

            let dx = x2 - x1;
            let dy = y2 - y1;
            let d = ((x1 - x as f32) * dy - (y1 - y as f32) * dx).abs() / (dx * dx + dy * dy).sqrt();
            if d < min_distance {
                min_distance = d;
                min_index = i;
            }
        }

        min_index
    }

    pub fn change_use_polygon(&mut self, new_use_polygon: bool) {
        if !new_use_polygon {
            self.polygon.set_from_bounds(&self.bounds);
        }
    }

    fn check_edge_split_input(&mut self, x: i32, y: i32, index: usize, use_polygon: bool) {
        if !self.shift_held && use_polygon {
            // Split the edge
            let new_vertex = Vertex::new(x as f32, y as f32);

            self.polygon.vertices.insert(index + 1, new_vertex);
            self.drag_state = DraggingEditState::PolygonVertex(PolygonVertexEditState {
                start_location: (x, y),
                start_origin: self.polygon.vertices[index].pos_tuple(),
                vertex_index: index + 1
            });
        } else {
            self.drag_state = DraggingEditState::ShiftPolygonEdge(PolygonEdgeEditState {
                start_location: (x, y),
                start_origin: self.polygon.get_edge_origin(index),
                edge_index: index
            });
        }
    }

    pub fn mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
        mouse_position: (i32, i32),
        icon_context: &mut IconContext
    ) -> SelectionInputResult {
        let (x, y) = (mouse_position.0, mouse_position.1);

        let mut result = SelectionInputResult::Nothing;

        if state == winit::event::ElementState::Pressed {
            if self.drag_state == DraggingEditState::CreatePolygon {
                self.polygon.vertices.push(Vertex::new(x as f32, y as f32));
            } else {
                let hit = self.detect_polygon_hit(mouse_position);
                match hit {
                    PolygonHitResult::Vertex(index) => {
                        if button == MouseButton::Right && icon_context.settings.use_polygon {
                            if self.polygon.vertices.len() <= 3 {
                                return SelectionInputResult::Nothing;
                            }
                            self.polygon.vertices.remove(index);
                        } else {
                            self.drag_state = DraggingEditState::PolygonVertex(PolygonVertexEditState {
                                start_location: (x, y),
                                start_origin: self.polygon.vertices[index].pos_tuple(),
                                vertex_index: index
                            });
                        }
                    },
                    PolygonHitResult::Edge(index) => {
                        if button == MouseButton::Right && icon_context.settings.use_polygon {
                            if self.polygon.vertices.len() <= 4 {
                                return SelectionInputResult::Nothing;
                            }
                            self.polygon.vertices.remove(index);
                            self.polygon.vertices.remove(index % self.polygon.vertices.len());
                        } else {
                            self.check_edge_split_input(x, y, index, icon_context.settings.use_polygon);
                        }
                    },
                    PolygonHitResult::None => {
                        // Create a new box
                        if !self.shift_held {
                            if self.ctrl_held && icon_context.settings.use_polygon {
                                self.polygon.clear();
                                self.polygon.vertices.push(Vertex::new(x as f32, y as f32));
                                self.polygon.vertices.push(Vertex::new(x as f32, y as f32));
                                self.drag_state = DraggingEditState::CreatePolygon;
                            } else {
                                self.bounds.x = x;
                                self.bounds.y = y;
                                self.bounds.width = 0;
                                self.bounds.height = 0;
                                self.polygon.set_from_bounds(&self.bounds);

                                result = SelectionInputResult::CompletelyMoved;
                                self.drag_state = DraggingEditState::NewBox(NewBoxEditState {
                                    start_location: (x, y),
                                    start_origin: self.polygon.get_origin()
                                });
                            }
                        } else {
                            self.drag_state = DraggingEditState::ShiftSelection(ShiftSelectionEditState {
                                start_location: (x, y),
                                start_origin: self.polygon.get_origin(),
                                from_edge: false
                            });

                            self.bounds.enclose_polygon(&self.polygon);
                        }
                    }
                }
            }

            self.bounds.enclose_polygon(&self.polygon);
            
            self.mouse_down = true;
        } else {
            match &self.drag_state {
                DraggingEditState::PolygonVertex(index) => {
                    if self.polygon.vertices.len() > 3 {
                        if self.should_merge_surrounding_edges(index.vertex_index).is_some() {
                            self.polygon.vertices.remove(index.vertex_index);
                        }
    
                        // Also check the two neighboring vertices
                        let prev_vertex_index = (index.vertex_index + self.polygon.vertices.len() - 1) % self.polygon.vertices.len();
    
                        let next_vertex_index = (index.vertex_index + 1) % self.polygon.vertices.len();
                        if self.should_merge_surrounding_edges(prev_vertex_index).is_some() {
                            self.polygon.vertices.remove(prev_vertex_index);
                        }
                        if self.should_merge_surrounding_edges(next_vertex_index).is_some() {
                            self.polygon.vertices.remove(next_vertex_index);
                        }

                        if self.polygon.vertices.len() < 3 {
                            self.polygon.vertices.clear();
                        }

                        self.polygon.deduplicate();
                    }
                }
                DraggingEditState::ShiftPolygonEdge(edge) => {
                    if self.should_merge_surrounding_edges(edge.edge_index).is_some() {
                        self.polygon.vertices.remove(edge.edge_index);
                    }
                    if self.should_merge_surrounding_edges((edge.edge_index + 1) % self.polygon.vertices.len()).is_some() {
                        self.polygon.vertices.remove((edge.edge_index + 1) % self.polygon.vertices.len());
                    }

                    if self.polygon.vertices.len() < 3 {
                        self.polygon.vertices.clear();
                    }

                    self.polygon.deduplicate();
                }
                _ => {}
            }
            if self.drag_state != DraggingEditState::CreatePolygon {
                result = SelectionInputResult::SelectionFinished;

                self.drag_state = DraggingEditState::None;
                self.mouse_down = false;
                
                self.bounds.enclose_polygon(&self.polygon);
            }
        }

        icon_context.settings_panel_visible = false;

        return result;
    }

    pub fn keyboard_event(&mut self, event: &KeyEvent) -> SelectionInputResult {
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Shift) => {
                self.shift_held = event.state == winit::event::ElementState::Pressed;
            }
            Key::Named(NamedKey::Control) => {
                self.ctrl_held = event.state == winit::event::ElementState::Pressed;
                if self.drag_state == DraggingEditState::CreatePolygon && !self.ctrl_held {
                    self.end_polygon_creation();
                    return SelectionInputResult::SelectionFinished;
                }
            }
            _ => {
                return SelectionInputResult::Nothing;
            }
        }
        SelectionInputResult::Nothing
    }

    fn end_polygon_creation(&mut self) {
        if self.polygon.vertices.len() > 2 {
            self.polygon.vertices.pop();
        } else {
            self.polygon.clear();
        }
        self.drag_state = DraggingEditState::None;
    }

    fn should_merge_surrounding_edges(&self, vertex_index: usize) -> Option<(f32, f32)> {
        // If the two surrounding edges are within a small angle of each other, merge them
        let vertex = &self.polygon.vertices[vertex_index];
        let prev_vertex = &self.polygon.vertices[(vertex_index + self.polygon.vertices.len() - 1) % self.polygon.vertices.len()];
        let next_vertex = &self.polygon.vertices[(vertex_index + 1) % self.polygon.vertices.len()];

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

            // Additionally, check if the vertex is close enough, so it doesn't depend as much on the line length.
            let distance = ((vertex.x - x).powi(2) + (vertex.y - y).powi(2)).sqrt();
            if distance < if self.shift_held || self.ctrl_held { 4.0 } else { 15.0 } {
                return Some((x, y));
            }

            None
        } else {
            None
        }
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
            let vertex1 = &self.polygon.vertices[i];
            let vertex2 = &self.polygon.vertices[(i + 1) % self.polygon.vertices.len()];
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

impl Into<Bounds> for (u32, u32) {
    fn into(self) -> Bounds {
        Bounds::new(0, 0, self.0 as i32, self.1 as i32)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Polygon {
    pub vertices: Vec<Vertex>,
    pub hovered_vertex: Option<usize>,
    pub hovered_edge: Option<usize>
}

impl Default for Polygon {
    fn default() -> Self {
        // The default state includes some vertices so we don't need to immediately resize the buffer
        Self {
            vertices: vec![
                Vertex::new(-50.0, -50.0),
                Vertex::new(-50.0, -50.0),
                Vertex::new(-50.0, -50.0),
                Vertex::new(-50.0, -50.0)
            ],
            hovered_vertex: None,
            hovered_edge: None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Vertex {
    pub x: f32,
    pub y: f32,

    pub vertex_highlight: SmoothFadeAnimation,
    pub edge_highlight: SmoothFadeAnimation
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GPUVertex {
    pub position: [f32; 2],
    pub animation: u32,
    pub _padding: u32
}

impl GPUVertex {
    pub fn new(x: f32, y: f32, animation: u32) -> Self {
        Self {
            position: [x, y],
            animation,
            _padding: 0
        }
    }
}

impl Vertex {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x, y,
            vertex_highlight: SmoothFadeAnimation::new(false),
            edge_highlight: SmoothFadeAnimation::new(false)
        }
    }

    pub fn as_gpu_vertex(&self) -> GPUVertex {
        GPUVertex::new(self.x, self.y, self.get_animation_int())
    }

    fn get_animation_int(&self) -> u32 {
        // We encode vertex and edge animations into the same integer
        // The first 16 bits are this vertex's opacity, and the next 16 bits are the opacity of the edge connecting this vertex and the next one.
        let vertex_opacity = (self.vertex_highlight.get_opacity() * 65535.0) as u32;
        let edge_opacity = (self.edge_highlight.get_opacity() * 65535.0) as u32;
        vertex_opacity << 16 | edge_opacity
    }

    fn update(&mut self, delta: std::time::Duration, edge_highlight: bool, vertex_highlight: bool) {
        self.vertex_highlight.update(delta, vertex_highlight);
        self.edge_highlight.update(delta, edge_highlight);
    }

    fn pos_tuple(&self) -> (f32, f32) {
        (self.x, self.y)
    }
}

impl Polygon {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            hovered_edge: None,
            hovered_vertex: None
        }
    }

    pub fn get_device_coords_polygon(&self, screen_size: (u32, u32)) -> Vec<GPUVertex> {
        let mut vertices = Vec::new();
        for vertex in &self.vertices {
            let mut vertex = vertex.clone();
            vertex.x /= screen_size.0 as f32;
            vertex.y /= screen_size.1 as f32;
            vertices.push(vertex.as_gpu_vertex());
        }

        vertices
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

    pub fn should_deduplicate(&self, vertex_index: usize) -> Option<(f32, f32)> {
        let margin = 5.0;
        let vertex = &self.vertices[vertex_index];
        for i in 0..self.vertices.len() {
            if i == vertex_index {
                continue;
            }
            let other_vertex = &self.vertices[i];
            if (vertex.x - other_vertex.x).abs() < margin && (vertex.y - other_vertex.y).abs() < margin {
                return Some((other_vertex.x, other_vertex.y));
            }
        }

        None
    }

    pub fn deduplicate(&mut self) {
        let mut i = 0;
        while i < self.vertices.len() {
            if self.should_deduplicate(i).is_some() {
                self.vertices.remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn set_from_bounds(&mut self, bounds: &Bounds) {
        self.vertices = vec![
            Vertex::new(bounds.x as f32, bounds.y as f32),
            Vertex::new(bounds.x as f32 + bounds.width as f32, bounds.y as f32),
            Vertex::new(bounds.x as f32 + bounds.width as f32, bounds.y as f32 + bounds.height as f32),
            Vertex::new(bounds.x as f32, bounds.y as f32 + bounds.height as f32)
        ];
    }

    pub fn get_edge_origin(&self, edge_index: usize) -> (f32, f32) {
        let vertex1 = &self.vertices[edge_index];
        let vertex2 = &self.vertices[(edge_index + 1) % self.vertices.len()];
        let x = (vertex1.x + vertex2.x) / 2.0;
        let y = (vertex1.y + vertex2.y) / 2.0;
        (x, y)
    }

    pub fn set_edge_origin(&mut self, edge_index: usize, origin: (f32, f32)) {
        let current_origin = self.get_edge_origin(edge_index);
        let dx = origin.0 - current_origin.0;
        let dy = origin.1 - current_origin.1;

        let vertex1 = &mut self.vertices[edge_index];
        vertex1.x += dx;
        vertex1.y += dy;
        
        let vertices = self.vertices.len();
        let vertex2 = &mut self.vertices[(edge_index + 1) % vertices];
        vertex2.x += dx;
        vertex2.y += dy;
    }

    pub fn as_gpu_vertices(&self) -> Vec<GPUVertex> {
        self.vertices.iter().map(|v| v.as_gpu_vertex()).collect()
    }

    pub fn update(&mut self, delta: std::time::Duration) {
        let vertices = self.vertices.len();
        for (vertex, i) in self.vertices.iter_mut().zip(0..) {
            let prev_vertex_index = (i + vertices - 1) % vertices;
            vertex.update(delta, self.hovered_edge.is_some_and(|idx| idx == prev_vertex_index), self.hovered_vertex.is_some_and(|idx| idx == i));
        }
    }
}