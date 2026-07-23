use physbox_launch::ValidationScenario;
use serde::Serialize;

use crate::{
    Camera2d, InputEvent, PointerButton, PointerEvent, PointerPhase, WheelEvent, ui::UiLayout,
};

pub const CONSTRUCTION_VALIDATION_SCRIPT_VERSION: u32 = 1;
const TARGET_SIZE: [u32; 2] = [960, 600];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionValidationScriptV1 {
    pub version: u32,
    pub name: &'static str,
    pub initial_scenario: ValidationScenario,
    pub target_size: [u32; 2],
    pub frames: Vec<ConstructionValidationFrameV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionValidationFrameV1 {
    pub label: &'static str,
    pub exact_steps: u32,
    pub events: Vec<InputEvent>,
}

impl ConstructionValidationScriptV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != CONSTRUCTION_VALIDATION_SCRIPT_VERSION {
            return Err(format!(
                "unsupported construction validation script version {}",
                self.version
            ));
        }
        if self.name.is_empty() || self.target_size.contains(&0) || self.frames.is_empty() {
            return Err("construction validation script header is invalid".to_owned());
        }
        crate::InputTraceV1 {
            version: crate::INPUT_TRACE_VERSION,
            name: self.name.to_owned(),
            frames: self
                .frames
                .iter()
                .map(|frame| crate::InputTraceFrameV1 {
                    target_size: self.target_size,
                    events: frame.events.clone(),
                })
                .collect(),
        }
        .validate()
        .map_err(|error| error.to_string())
    }
}

pub fn construction_interaction_validation_script() -> ConstructionValidationScriptV1 {
    let target_size = TARGET_SIZE;
    let layout = UiLayout::for_target(target_size);
    let camera = Camera2d::fixed().resolve(target_size).unwrap();
    let mut next_pointer = 1_u64;
    let mut click = |button, at| {
        let id = next_pointer;
        next_pointer += 1;
        [PointerPhase::Down, PointerPhase::Up]
            .into_iter()
            .map(|phase| {
                InputEvent::Pointer(PointerEvent {
                    id,
                    phase,
                    button,
                    normalized_position: at,
                })
            })
            .collect::<Vec<_>>()
    };
    let world = |point| camera.world_to_screen(point);
    let frame = |label, exact_steps, events| ConstructionValidationFrameV1 {
        label,
        exact_steps,
        events,
    };
    let mut high_cadence = Vec::new();
    for x in [-1.35, -1.05, -0.75, -0.45, -0.15, 0.15, 0.45, 0.75] {
        high_cadence.extend(click(PointerButton::Primary, world([x, 0.72])));
    }
    let touch_a = world([0.2, 0.7]);
    let touch_b = world([0.5, 0.7]);
    let script = ConstructionValidationScriptV1 {
        version: CONSTRUCTION_VALIDATION_SCRIPT_VERSION,
        name: "s5.3-construction-interaction",
        initial_scenario: ValidationScenario::Bootstrap,
        target_size,
        frames: vec![
            frame(
                "select-circle",
                0,
                click(PointerButton::Primary, layout.center_normalized("tool", 1)),
            ),
            frame(
                "place-circle",
                1,
                click(PointerButton::Primary, world([-0.9, 0.72])),
            ),
            frame("circle-fall", 90, Vec::new()),
            frame(
                "select-rectangle",
                0,
                click(PointerButton::Primary, layout.center_normalized("tool", 2)),
            ),
            frame(
                "place-rectangle",
                1,
                click(PointerButton::Primary, world([0.9, 0.72])),
            ),
            frame("rectangle-fall", 90, Vec::new()),
            frame(
                "select-grab",
                0,
                click(PointerButton::Primary, layout.center_normalized("tool", 0)),
            ),
            frame(
                "grab-begin",
                1,
                vec![InputEvent::Pointer(PointerEvent {
                    id: 100,
                    phase: PointerPhase::Down,
                    button: PointerButton::Primary,
                    normalized_position: world([-0.9, 0.1]),
                })],
            ),
            frame(
                "grab-move",
                8,
                vec![InputEvent::Pointer(PointerEvent {
                    id: 100,
                    phase: PointerPhase::Move,
                    button: PointerButton::Primary,
                    normalized_position: world([-0.55, 0.5]),
                })],
            ),
            frame(
                "grab-release",
                1,
                vec![InputEvent::Pointer(PointerEvent {
                    id: 100,
                    phase: PointerPhase::Up,
                    button: PointerButton::Primary,
                    normalized_position: world([-0.55, 0.5]),
                })],
            ),
            frame(
                "select-erase",
                0,
                click(PointerButton::Primary, layout.center_normalized("tool", 3)),
            ),
            frame(
                "erase-rectangle",
                1,
                click(PointerButton::Primary, world([0.9, 0.08])),
            ),
            frame(
                "select-circle-reuse",
                0,
                click(PointerButton::Primary, layout.center_normalized("tool", 1)),
            ),
            frame(
                "place-reused-circle",
                1,
                click(PointerButton::Primary, world([0.0, 0.72])),
            ),
            frame("reuse-fall-stack", 90, Vec::new()),
            frame("high-cadence-place", 1, high_cadence),
            frame(
                "two-touch-cancel",
                0,
                vec![
                    InputEvent::Pointer(PointerEvent {
                        id: 200,
                        phase: PointerPhase::Down,
                        button: PointerButton::Contact,
                        normalized_position: touch_a,
                    }),
                    InputEvent::Pointer(PointerEvent {
                        id: 201,
                        phase: PointerPhase::Down,
                        button: PointerButton::Contact,
                        normalized_position: touch_b,
                    }),
                    InputEvent::Pointer(PointerEvent {
                        id: 200,
                        phase: PointerPhase::Up,
                        button: PointerButton::Contact,
                        normalized_position: touch_a,
                    }),
                    InputEvent::Pointer(PointerEvent {
                        id: 201,
                        phase: PointerPhase::Up,
                        button: PointerButton::Contact,
                        normalized_position: touch_b,
                    }),
                ],
            ),
            frame(
                "secondary-pan",
                0,
                vec![
                    InputEvent::Pointer(PointerEvent {
                        id: 300,
                        phase: PointerPhase::Down,
                        button: PointerButton::Secondary,
                        normalized_position: [0.5, 0.55],
                    }),
                    InputEvent::Pointer(PointerEvent {
                        id: 300,
                        phase: PointerPhase::Move,
                        button: PointerButton::Primary,
                        normalized_position: [0.6, 0.45],
                    }),
                    InputEvent::Pointer(PointerEvent {
                        id: 300,
                        phase: PointerPhase::Up,
                        button: PointerButton::Secondary,
                        normalized_position: [0.6, 0.45],
                    }),
                ],
            ),
            frame(
                "wheel-zoom",
                0,
                vec![InputEvent::Wheel(WheelEvent {
                    normalized_position: [0.6, 0.45],
                    zoom_steps: 1.0,
                })],
            ),
            frame(
                "open-menu-recenter",
                0,
                click(PointerButton::Primary, layout.center_normalized("menu", 0)),
            ),
            frame(
                "recenter",
                0,
                click(
                    PointerButton::Primary,
                    layout.center_normalized("recenter", 0),
                ),
            ),
            frame(
                "open-menu-restart",
                0,
                click(PointerButton::Primary, layout.center_normalized("menu", 0)),
            ),
            frame(
                "restart",
                0,
                click(
                    PointerButton::Primary,
                    layout.center_normalized("restart", 0),
                ),
            ),
            frame(
                "open-menu-switch",
                0,
                click(PointerButton::Primary, layout.center_normalized("menu", 0)),
            ),
            frame(
                "switch-rectangle-stack",
                0,
                click(
                    PointerButton::Primary,
                    layout.center_normalized("scenario", 4),
                ),
            ),
            frame(
                "open-menu-switch-back",
                0,
                click(PointerButton::Primary, layout.center_normalized("menu", 0)),
            ),
            frame(
                "switch-bootstrap",
                0,
                click(
                    PointerButton::Primary,
                    layout.center_normalized("scenario", 0),
                ),
            ),
        ],
    };
    script.validate().expect("engine-owned script is valid");
    script
}

pub fn construction_deep_overlap_validation_script() -> ConstructionValidationScriptV1 {
    let target_size = TARGET_SIZE;
    let layout = UiLayout::for_target(target_size);
    let camera = Camera2d::fixed().resolve(target_size).unwrap();
    let script = ConstructionValidationScriptV1 {
        version: CONSTRUCTION_VALIDATION_SCRIPT_VERSION,
        name: "s5.3-construction-deep-overlap",
        initial_scenario: ValidationScenario::Bootstrap,
        target_size,
        frames: vec![
            ConstructionValidationFrameV1 {
                label: "select-circle",
                exact_steps: 0,
                events: click(1, layout.center_normalized("tool", 1)),
            },
            ConstructionValidationFrameV1 {
                label: "reject-deep-overlap",
                exact_steps: 1,
                events: click(2, camera.world_to_screen([0.0, 0.65])),
            },
        ],
    };
    script.validate().expect("engine-owned script is valid");
    script
}

pub fn construction_invalidation_validation_script() -> ConstructionValidationScriptV1 {
    let target_size = TARGET_SIZE;
    let layout = UiLayout::for_target(target_size);
    let camera = Camera2d::fixed().resolve(target_size).unwrap();
    let script = ConstructionValidationScriptV1 {
        version: CONSTRUCTION_VALIDATION_SCRIPT_VERSION,
        name: "s5.3-construction-invalidation",
        initial_scenario: ValidationScenario::RigidFloor,
        target_size,
        frames: vec![
            ConstructionValidationFrameV1 {
                label: "warm-contact",
                exact_steps: 120,
                events: Vec::new(),
            },
            ConstructionValidationFrameV1 {
                label: "select-erase",
                exact_steps: 0,
                events: click(1, layout.center_normalized("tool", 3)),
            },
            ConstructionValidationFrameV1 {
                label: "erase-warm-body",
                exact_steps: 1,
                events: click(2, camera.world_to_screen([0.0, 0.2])),
            },
            ConstructionValidationFrameV1 {
                label: "select-circle",
                exact_steps: 0,
                events: click(3, layout.center_normalized("tool", 1)),
            },
            ConstructionValidationFrameV1 {
                label: "reuse-empty-generation",
                exact_steps: 1,
                events: click(4, camera.world_to_screen([0.0, 0.72])),
            },
            ConstructionValidationFrameV1 {
                label: "generation-recontact",
                exact_steps: 120,
                events: Vec::new(),
            },
        ],
    };
    script.validate().expect("engine-owned script is valid");
    script
}

pub fn construction_capacity_validation_script() -> ConstructionValidationScriptV1 {
    let target_size = TARGET_SIZE;
    let layout = UiLayout::for_target(target_size);
    let camera = Camera2d::fixed().resolve(target_size).unwrap();
    let mut events = Vec::new();
    let mut id = 2_u64;
    'rows: for y in [-0.82, -0.58, -0.34, 0.34, 0.58] {
        for x in [
            -1.43, -1.21, -0.99, -0.77, -0.55, -0.33, -0.11, 0.11, 0.33, 0.55, 0.77, 0.99, 1.21,
            1.43,
        ] {
            let x: f32 = x;
            if x.abs() <= 0.35_f32 && (y - 0.65_f32).abs() <= 0.2_f32 {
                continue;
            }
            let at = camera.world_to_screen([x, y]);
            events.extend(click(id, at));
            id += 1;
            if events.len() == 126 {
                break 'rows;
            }
        }
    }
    assert_eq!(events.len(), 126);
    let script = ConstructionValidationScriptV1 {
        version: CONSTRUCTION_VALIDATION_SCRIPT_VERSION,
        name: "s5.3-construction-capacity",
        initial_scenario: ValidationScenario::Bootstrap,
        target_size,
        frames: vec![
            ConstructionValidationFrameV1 {
                label: "select-circle",
                exact_steps: 0,
                events: click(1, layout.center_normalized("tool", 1)),
            },
            ConstructionValidationFrameV1 {
                label: "fill-and-reject",
                exact_steps: 1,
                events,
            },
        ],
    };
    script.validate().expect("engine-owned script is valid");
    script
}

fn click(id: u64, at: [f32; 2]) -> Vec<InputEvent> {
    [PointerPhase::Down, PointerPhase::Up]
        .into_iter()
        .map(|phase| {
            InputEvent::Pointer(PointerEvent {
                id,
                phase,
                button: PointerButton::Primary,
                normalized_position: at,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_validation_scripts_are_versioned_and_finite() {
        for script in [
            construction_interaction_validation_script(),
            construction_deep_overlap_validation_script(),
            construction_invalidation_validation_script(),
            construction_capacity_validation_script(),
        ] {
            script.validate().unwrap();
            assert!(script.frames.iter().all(|frame| frame.exact_steps <= 120));
        }
    }
}
