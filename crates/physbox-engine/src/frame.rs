use crate::clock::InteractiveClockAction;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Contact,
    Auxiliary,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PointerEvent {
    pub id: u64,
    pub phase: PointerPhase,
    pub button: PointerButton,
    pub normalized_position: [f32; 2],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WheelEvent {
    pub normalized_position: [f32; 2],
    /// Positive values zoom in and negative values zoom out. Platform adapters
    /// normalize only the host scroll direction; shared engine policy owns the
    /// zoom factor.
    pub zoom_steps: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputEvent {
    Pointer(PointerEvent),
    Wheel(WheelEvent),
    Recenter,
}

#[derive(Clone, Debug)]
pub struct FrameInput {
    pub step_request: StepRequest,
    pub input_events: Vec<InputEvent>,
}

impl FrameInput {
    pub fn new(elapsed_seconds: f64) -> Self {
        Self {
            step_request: StepRequest::Interactive {
                elapsed_seconds,
                requested_time_scale: 1.0,
                action: InteractiveClockAction::Run,
            },
            input_events: Vec::new(),
        }
    }

    pub fn exact(steps: u32) -> Self {
        Self {
            step_request: StepRequest::Exact { steps },
            input_events: Vec::new(),
        }
    }

    pub fn with_clock_action(mut self, clock_action: InteractiveClockAction) -> Self {
        if let StepRequest::Interactive { action, .. } = &mut self.step_request {
            *action = clock_action;
        }
        self
    }

    pub fn with_input_events(mut self, input_events: Vec<InputEvent>) -> Self {
        self.input_events = input_events;
        self
    }
}

impl Default for FrameInput {
    fn default() -> Self {
        Self::new(0.0)
    }
}
