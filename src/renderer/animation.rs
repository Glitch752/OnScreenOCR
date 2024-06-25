#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum MoveDirection {
    Up,
    Down,
    Left,
    Right
}

impl Default for MoveDirection {
    fn default() -> Self {
        Self::Up
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SmoothMoveFadeAnimation {
    target_visibility: bool,
    visible_ratio: f32,

    pub fade_move_direction: MoveDirection,
    pub fade_move_amount: f32
}

#[allow(dead_code)]
impl SmoothMoveFadeAnimation {
    pub fn new(start_visible: bool, fade_move_direction: MoveDirection, fade_move_amount: f32) -> Self {
        Self {
            target_visibility: start_visible,
            visible_ratio: if start_visible { 1. } else { 0. },
            fade_move_direction,
            fade_move_amount
        }
    }

    pub fn update(&mut self, delta: std::time::Duration, target_visibility: bool) {
        self.target_visibility = target_visibility;

        let target_ratio = if self.target_visibility { 1. } else { 0. };
        self.visible_ratio += (self.visible_ratio - target_ratio) * (1. - (delta.as_millis_f32() * 0.025).exp());
        // Just in case something goes wrong
        if self.visible_ratio.is_nan() || self.visible_ratio < 0. || self.visible_ratio > 1. {
            self.visible_ratio = target_ratio;
        }

        if self.visible_ratio < 0.01 {
            self.visible_ratio = 0.;
        }
    }

    pub fn visible_at_all(&self) -> bool {
        self.visible_ratio > 0.
    }

    pub fn fully_visible(&self) -> bool {
        (self.visible_ratio - 1.).abs() < 0.01
    }

    pub fn get_opacity(&self) -> f32 {
        self.visible_ratio
    }

    pub fn move_point(&self, point: (f32, f32)) -> (f32, f32) {
        let (x, y) = point;
        match self.fade_move_direction {
            MoveDirection::Up => (x, y - (1. - self.fade_move_amount) * self.visible_ratio),
            MoveDirection::Down => (x, y + (1. - self.fade_move_amount) * self.visible_ratio),
            MoveDirection::Left => (x - (1. - self.fade_move_amount) * self.visible_ratio, y),
            MoveDirection::Right => (x + (1. - self.fade_move_amount) * self.visible_ratio, y)
        }
    }

    pub fn is_finished(&self) -> bool {
        (self.visible_ratio - 1.).abs() < 0.01 || (self.visible_ratio - 0.).abs() < 0.01
    }
}