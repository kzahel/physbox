#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Contact,
    Auxiliary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub id: u64,
    pub phase: PointerPhase,
    pub button: PointerButton,
    pub normalized_position: [f32; 2],
}

#[derive(Clone, Debug, Default)]
pub struct FrameInput {
    pub elapsed_seconds: f64,
    pub pointer_events: Vec<PointerEvent>,
}

impl FrameInput {
    pub fn new(elapsed_seconds: f64) -> Self {
        Self {
            elapsed_seconds,
            pointer_events: Vec::new(),
        }
    }
}
