use std::collections::BTreeMap;
use std::fmt;

use physbox_contracts::Vec2;
use serde::{Deserialize, Serialize};

use crate::{Camera2d, InputEvent, PointerButton, PointerEvent, PointerPhase, WheelEvent};

const WHEEL_ZOOM_IN_FACTOR: f32 = 1.05;
const WHEEL_ZOOM_OUT_FACTOR: f32 = 1.0 / WHEEL_ZOOM_IN_FACTOR;
pub const INPUT_TRACE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputTraceV1 {
    pub version: u32,
    pub name: String,
    pub frames: Vec<InputTraceFrameV1>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputTraceFrameV1 {
    pub target_size: [u32; 2],
    pub events: Vec<InputEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputTraceError {
    UnsupportedVersion(u32),
    EmptyName,
    EmptyFrames,
    InvalidTargetSize { frame: usize },
    InvalidPointerPosition { frame: usize, event: usize },
    InvalidWheel { frame: usize, event: usize },
}

impl fmt::Display for InputTraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported input trace version {version}")
            }
            Self::EmptyName => formatter.write_str("input trace name must not be empty"),
            Self::EmptyFrames => formatter.write_str("input trace must contain at least one frame"),
            Self::InvalidTargetSize { frame } => {
                write!(
                    formatter,
                    "input trace frame {frame} has a zero target dimension"
                )
            }
            Self::InvalidPointerPosition { frame, event } => write!(
                formatter,
                "input trace frame {frame} event {event} has an invalid normalized pointer position"
            ),
            Self::InvalidWheel { frame, event } => write!(
                formatter,
                "input trace frame {frame} event {event} has an invalid wheel payload"
            ),
        }
    }
}

impl std::error::Error for InputTraceError {}

impl InputTraceV1 {
    pub fn validate(&self) -> Result<(), InputTraceError> {
        if self.version != INPUT_TRACE_VERSION {
            return Err(InputTraceError::UnsupportedVersion(self.version));
        }
        if self.name.is_empty() {
            return Err(InputTraceError::EmptyName);
        }
        if self.frames.is_empty() {
            return Err(InputTraceError::EmptyFrames);
        }
        for (frame_index, frame) in self.frames.iter().enumerate() {
            if frame.target_size.contains(&0) {
                return Err(InputTraceError::InvalidTargetSize { frame: frame_index });
            }
            for (event_index, event) in frame.events.iter().enumerate() {
                match event {
                    InputEvent::Pointer(pointer)
                        if !valid_normalized_position(pointer.normalized_position) =>
                    {
                        return Err(InputTraceError::InvalidPointerPosition {
                            frame: frame_index,
                            event: event_index,
                        });
                    }
                    InputEvent::Wheel(wheel)
                        if !valid_normalized_position(wheel.normalized_position)
                            || !wheel.zoom_steps.is_finite() =>
                    {
                        return Err(InputTraceError::InvalidWheel {
                            frame: frame_index,
                            event: event_index,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WorldPointerEvent {
    pub id: u64,
    pub phase: PointerPhase,
    pub button: PointerButton,
    pub world_target: Vec2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NavigationChanges {
    pub pan: u32,
    pub zoom: u32,
    pub recenter: u32,
}

#[derive(Debug, Default)]
pub(crate) struct SharedInputState {
    contacts: BTreeMap<u64, [f32; 2]>,
    touch_camera_active: bool,
    secondary_pan: Option<(u64, [f32; 2])>,
    pan_changes_total: u64,
    zoom_changes_total: u64,
    recenter_changes_total: u64,
}

impl SharedInputState {
    pub(crate) fn apply_events(
        &mut self,
        camera: &mut Camera2d,
        events: &[InputEvent],
        target_size: [u32; 2],
    ) -> (Vec<WorldPointerEvent>, NavigationChanges) {
        let mut world_events = Vec::new();
        let mut changes = NavigationChanges::default();
        for event in events {
            match *event {
                InputEvent::Pointer(pointer) => self.apply_pointer(
                    camera,
                    pointer,
                    target_size,
                    &mut world_events,
                    &mut changes,
                ),
                InputEvent::Wheel(wheel) => {
                    self.apply_wheel(camera, wheel, target_size, &mut changes)
                }
                InputEvent::Recenter => {
                    if camera.recenter() {
                        changes.recenter = changes.recenter.saturating_add(1);
                    }
                }
            }
        }
        self.pan_changes_total = self
            .pan_changes_total
            .saturating_add(u64::from(changes.pan));
        self.zoom_changes_total = self
            .zoom_changes_total
            .saturating_add(u64::from(changes.zoom));
        self.recenter_changes_total = self
            .recenter_changes_total
            .saturating_add(u64::from(changes.recenter));
        (world_events, changes)
    }

    fn apply_pointer(
        &mut self,
        camera: &mut Camera2d,
        event: PointerEvent,
        target_size: [u32; 2],
        world_events: &mut Vec<WorldPointerEvent>,
        changes: &mut NavigationChanges,
    ) {
        let position = normalized(event.normalized_position);
        if self.secondary_pan.is_some_and(|(id, _)| id == event.id) {
            match event.phase {
                PointerPhase::Move => {
                    let (_, previous) = self.secondary_pan.expect("secondary capture exists");
                    let delta = [position[0] - previous[0], position[1] - previous[1]];
                    self.secondary_pan = Some((event.id, position));
                    if delta != [0.0, 0.0] && camera.pan_screen(delta, target_size).is_ok() {
                        changes.pan = changes.pan.saturating_add(1);
                    }
                }
                PointerPhase::Up | PointerPhase::Cancel => self.secondary_pan = None,
                PointerPhase::Down => {}
            }
            return;
        }

        if event.phase == PointerPhase::Down && event.button == PointerButton::Secondary {
            self.secondary_pan = Some((event.id, position));
            return;
        }

        if event.button != PointerButton::Contact && !self.contacts.contains_key(&event.id) {
            if event.button == PointerButton::Primary {
                world_events.push(projected(event, position, camera, target_size));
            }
            return;
        }

        match event.phase {
            PointerPhase::Down => {
                self.contacts.insert(event.id, position);
                if self.contacts.len() == 1 && !self.touch_camera_active {
                    world_events.push(projected(event, position, camera, target_size));
                } else if self.contacts.len() >= 2 && !self.touch_camera_active {
                    self.touch_camera_active = true;
                    if let Some((&first_id, &first_position)) = self
                        .contacts
                        .iter()
                        .find(|(pointer_id, _)| **pointer_id != event.id)
                    {
                        world_events.push(projected(
                            PointerEvent {
                                id: first_id,
                                phase: PointerPhase::Cancel,
                                button: PointerButton::Contact,
                                normalized_position: first_position,
                            },
                            first_position,
                            camera,
                            target_size,
                        ));
                    }
                }
            }
            PointerPhase::Move if self.touch_camera_active => {
                let before = touch_pair(&self.contacts);
                self.contacts.insert(event.id, position);
                let after = touch_pair(&self.contacts);
                if let (Some((before_a, before_b)), Some((after_a, after_b))) = (before, after) {
                    let before_midpoint = midpoint(before_a, before_b);
                    let after_midpoint = midpoint(after_a, after_b);
                    let delta = [
                        after_midpoint[0] - before_midpoint[0],
                        after_midpoint[1] - before_midpoint[1],
                    ];
                    if delta != [0.0, 0.0] && camera.pan_screen(delta, target_size).is_ok() {
                        changes.pan = changes.pan.saturating_add(1);
                    }
                    let before_distance = distance(before_a, before_b);
                    let after_distance = distance(after_a, after_b);
                    if before_distance > 1.0e-6
                        && after_distance.is_finite()
                        && camera
                            .zoom_at(
                                after_midpoint,
                                after_distance / before_distance,
                                target_size,
                            )
                            .is_ok()
                    {
                        changes.zoom = changes.zoom.saturating_add(1);
                    }
                }
            }
            PointerPhase::Move => {
                self.contacts.insert(event.id, position);
                world_events.push(projected(event, position, camera, target_size));
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                self.contacts.remove(&event.id);
                if self.touch_camera_active {
                    if self.contacts.is_empty() {
                        self.touch_camera_active = false;
                    }
                } else {
                    world_events.push(projected(event, position, camera, target_size));
                }
            }
        }
    }

    fn apply_wheel(
        &mut self,
        camera: &mut Camera2d,
        event: WheelEvent,
        target_size: [u32; 2],
        changes: &mut NavigationChanges,
    ) {
        let factor = if event.zoom_steps > 0.0 {
            WHEEL_ZOOM_IN_FACTOR
        } else if event.zoom_steps < 0.0 {
            WHEEL_ZOOM_OUT_FACTOR
        } else {
            return;
        };
        if camera
            .zoom_at(normalized(event.normalized_position), factor, target_size)
            .is_ok()
        {
            changes.zoom = changes.zoom.saturating_add(1);
        }
    }

    pub(crate) const fn pan_changes_total(&self) -> u64 {
        self.pan_changes_total
    }

    pub(crate) const fn zoom_changes_total(&self) -> u64 {
        self.zoom_changes_total
    }

    pub(crate) const fn recenter_changes_total(&self) -> u64 {
        self.recenter_changes_total
    }

    pub(crate) const fn touch_camera_active(&self) -> bool {
        self.touch_camera_active
    }

    pub(crate) const fn secondary_pan_active(&self) -> bool {
        self.secondary_pan.is_some()
    }
}

fn projected(
    event: PointerEvent,
    position: [f32; 2],
    camera: &Camera2d,
    target_size: [u32; 2],
) -> WorldPointerEvent {
    let world = camera
        .resolve(target_size)
        .unwrap_or_else(|_| {
            Camera2d::fixed()
                .resolve([1, 1])
                .expect("fixed camera is valid")
        })
        .screen_to_world(position);
    WorldPointerEvent {
        id: event.id,
        phase: event.phase,
        button: event.button,
        world_target: Vec2::new(world[0], world[1]),
    }
}

fn normalized(position: [f32; 2]) -> [f32; 2] {
    [position[0].clamp(0.0, 1.0), position[1].clamp(0.0, 1.0)]
}

fn valid_normalized_position(position: [f32; 2]) -> bool {
    position
        .into_iter()
        .all(|coordinate| coordinate.is_finite() && (0.0..=1.0).contains(&coordinate))
}

fn touch_pair(contacts: &BTreeMap<u64, [f32; 2]>) -> Option<([f32; 2], [f32; 2])> {
    let mut positions = contacts.values().copied();
    Some((positions.next()?, positions.next()?))
}

fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(id: u64, phase: PointerPhase, button: PointerButton, at: [f32; 2]) -> InputEvent {
        InputEvent::Pointer(PointerEvent {
            id,
            phase,
            button,
            normalized_position: at,
        })
    }

    #[test]
    fn secondary_drag_pans_but_primary_projects_to_world() {
        let mut state = SharedInputState::default();
        let mut camera = Camera2d::fixed();
        let (world, changes) = state.apply_events(
            &mut camera,
            &[
                pointer(1, PointerPhase::Down, PointerButton::Secondary, [0.5, 0.5]),
                pointer(1, PointerPhase::Move, PointerButton::Primary, [0.6, 0.4]),
                pointer(1, PointerPhase::Up, PointerButton::Secondary, [0.6, 0.4]),
                pointer(2, PointerPhase::Down, PointerButton::Primary, [0.5, 0.5]),
            ],
            [1000, 500],
        );
        assert_eq!(changes.pan, 1);
        assert_eq!(world.len(), 1);
        assert_eq!(world[0].button, PointerButton::Primary);
        assert!((camera.center()[0] + 0.4).abs() <= 1.0e-5);
        assert!((camera.center()[1] + 0.2).abs() <= 1.0e-5);
        assert!((world[0].world_target.x + 0.4).abs() <= 1.0e-5);
        assert!((world[0].world_target.y + 0.2).abs() <= 1.0e-5);
    }

    #[test]
    fn lost_secondary_capture_rearms_primary_tool_input() {
        let mut state = SharedInputState::default();
        let mut camera = Camera2d::fixed();
        let (world, changes) = state.apply_events(
            &mut camera,
            &[
                pointer(1, PointerPhase::Down, PointerButton::Secondary, [0.5, 0.5]),
                pointer(1, PointerPhase::Cancel, PointerButton::Primary, [0.5, 0.5]),
                pointer(2, PointerPhase::Down, PointerButton::Primary, [0.5, 0.5]),
            ],
            [600, 600],
        );
        assert_eq!(changes, NavigationChanges::default());
        assert!(!state.secondary_pan_active());
        assert_eq!(world.len(), 1);
        assert_eq!(world[0].id, 2);
    }

    #[test]
    fn second_touch_cancels_tool_and_requires_all_contacts_up() {
        let mut state = SharedInputState::default();
        let mut camera = Camera2d::fixed();
        let (world, _) = state.apply_events(
            &mut camera,
            &[
                pointer(10, PointerPhase::Down, PointerButton::Contact, [0.4, 0.5]),
                pointer(11, PointerPhase::Down, PointerButton::Contact, [0.6, 0.5]),
                pointer(10, PointerPhase::Move, PointerButton::Contact, [0.3, 0.5]),
                pointer(11, PointerPhase::Up, PointerButton::Contact, [0.6, 0.5]),
                pointer(10, PointerPhase::Move, PointerButton::Contact, [0.2, 0.5]),
                pointer(10, PointerPhase::Up, PointerButton::Contact, [0.2, 0.5]),
            ],
            [600, 600],
        );
        assert_eq!(world.len(), 2);
        assert_eq!(world[0].phase, PointerPhase::Down);
        assert_eq!(world[1].phase, PointerPhase::Cancel);
        assert!((world[0].world_target.x + 0.2).abs() <= 1.0e-5);
        assert!(world[0].world_target.y.abs() <= 1.0e-5);
        assert_eq!(world[0].world_target, world[1].world_target);
        assert!(!state.touch_camera_active());
        assert_eq!(state.pan_changes_total(), 1);
        assert!((camera.center()[0] - 0.066_666_66).abs() <= 1.0e-5);
        assert!(camera.center()[1].abs() <= 1.0e-5);
        assert!((camera.orthographic_half_height() - 2.0 / 3.0).abs() <= 1.0e-5);
    }

    #[test]
    fn pinch_and_wheel_preserve_their_screen_anchors() {
        let mut state = SharedInputState::default();
        let mut camera = Camera2d::fixed();
        let target = [960, 600];
        let wheel_anchor = [0.75, 0.25];
        let before = camera
            .resolve(target)
            .unwrap()
            .screen_to_world(wheel_anchor);
        state.apply_events(
            &mut camera,
            &[InputEvent::Wheel(WheelEvent {
                normalized_position: wheel_anchor,
                zoom_steps: 1.0,
            })],
            target,
        );
        let after = camera
            .resolve(target)
            .unwrap()
            .screen_to_world(wheel_anchor);
        assert!((before[0] - after[0]).abs() <= 1.0e-5);
        assert!((before[1] - after[1]).abs() <= 1.0e-5);
        assert_eq!(state.zoom_changes_total(), 1);
    }

    #[test]
    fn recenter_is_counted_only_when_state_changes() {
        let mut state = SharedInputState::default();
        let mut camera = Camera2d::try_new([1.0, 2.0], 3.0).unwrap();
        let (_, changes) = state.apply_events(
            &mut camera,
            &[InputEvent::Recenter, InputEvent::Recenter],
            [640, 480],
        );
        assert_eq!(changes.recenter, 1);
        assert_eq!(state.recenter_changes_total(), 1);
        assert_eq!(camera, Camera2d::fixed());
    }

    #[test]
    fn versioned_trace_round_trips_and_replays_across_resize() {
        let trace = InputTraceV1 {
            version: INPUT_TRACE_VERSION,
            name: "mouse-wheel-two-touch-resize".to_owned(),
            frames: vec![
                InputTraceFrameV1 {
                    target_size: [1000, 500],
                    events: vec![
                        pointer(1, PointerPhase::Down, PointerButton::Secondary, [0.5, 0.5]),
                        pointer(1, PointerPhase::Move, PointerButton::Primary, [0.6, 0.4]),
                        pointer(1, PointerPhase::Up, PointerButton::Secondary, [0.6, 0.4]),
                        InputEvent::Wheel(WheelEvent {
                            normalized_position: [0.75, 0.25],
                            zoom_steps: 1.0,
                        }),
                    ],
                },
                InputTraceFrameV1 {
                    target_size: [500, 1000],
                    events: vec![
                        pointer(10, PointerPhase::Down, PointerButton::Contact, [0.4, 0.5]),
                        pointer(11, PointerPhase::Down, PointerButton::Contact, [0.6, 0.5]),
                        pointer(10, PointerPhase::Move, PointerButton::Contact, [0.3, 0.45]),
                        pointer(11, PointerPhase::Move, PointerButton::Contact, [0.7, 0.55]),
                        pointer(
                            10,
                            PointerPhase::Cancel,
                            PointerButton::Contact,
                            [0.3, 0.45],
                        ),
                        pointer(11, PointerPhase::Up, PointerButton::Contact, [0.7, 0.55]),
                    ],
                },
                InputTraceFrameV1 {
                    target_size: [500, 1000],
                    events: vec![InputEvent::Recenter],
                },
            ],
        };
        trace.validate().unwrap();
        let encoded = serde_json::to_string_pretty(&trace).unwrap();
        let decoded: InputTraceV1 = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, trace);

        let replay = |trace: &InputTraceV1| {
            let mut state = SharedInputState::default();
            let mut camera = Camera2d::fixed();
            let mut projected = Vec::new();
            for frame in &trace.frames {
                projected.extend(
                    state
                        .apply_events(&mut camera, &frame.events, frame.target_size)
                        .0,
                );
            }
            (
                camera,
                projected,
                state.pan_changes_total(),
                state.zoom_changes_total(),
                state.recenter_changes_total(),
            )
        };
        assert_eq!(replay(&decoded), replay(&trace));
        assert_eq!(replay(&trace).0, Camera2d::fixed());
    }

    #[test]
    fn trace_validation_rejects_non_finite_adapter_payloads() {
        let trace = InputTraceV1 {
            version: INPUT_TRACE_VERSION,
            name: "invalid".to_owned(),
            frames: vec![InputTraceFrameV1 {
                target_size: [320, 200],
                events: vec![InputEvent::Wheel(WheelEvent {
                    normalized_position: [0.5, 0.5],
                    zoom_steps: f32::NAN,
                })],
            }],
        };
        assert_eq!(
            trace.validate(),
            Err(InputTraceError::InvalidWheel { frame: 0, event: 0 })
        );
    }
}
