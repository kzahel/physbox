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
    pub requested_time_scale: f64,
    pub clock_action: InteractiveClockAction,
    pub pointer_events: Vec<PointerEvent>,
}

impl FrameInput {
    pub fn new(elapsed_seconds: f64) -> Self {
        Self {
            elapsed_seconds,
            requested_time_scale: 1.0,
            clock_action: InteractiveClockAction::Run,
            pointer_events: Vec::new(),
        }
    }

    pub fn with_clock_action(mut self, clock_action: InteractiveClockAction) -> Self {
        self.clock_action = clock_action;
        self
    }
}
use crate::clock::InteractiveClockAction;
