use crate::clock::InteractiveClockAction;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepRequest {
    Interactive {
        elapsed_seconds: f64,
        requested_time_scale: f64,
        action: InteractiveClockAction,
    },
    Exact {
        steps: u32,
    },
}

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

#[derive(Clone, Debug)]
pub struct FrameInput {
    pub step_request: StepRequest,
    pub pointer_events: Vec<PointerEvent>,
}

impl FrameInput {
    pub fn new(elapsed_seconds: f64) -> Self {
        Self {
            step_request: StepRequest::Interactive {
                elapsed_seconds,
                requested_time_scale: 1.0,
                action: InteractiveClockAction::Run,
            },
            pointer_events: Vec::new(),
        }
    }

    pub fn exact(steps: u32) -> Self {
        Self {
            step_request: StepRequest::Exact { steps },
            pointer_events: Vec::new(),
        }
    }

    pub fn with_clock_action(mut self, clock_action: InteractiveClockAction) -> Self {
        if let StepRequest::Interactive { action, .. } = &mut self.step_request {
            *action = clock_action;
        }
        self
    }
}

impl Default for FrameInput {
    fn default() -> Self {
        Self::new(0.0)
    }
}
