use std::collections::BTreeMap;

use crate::{Point, Rect};

/// Stable identity selected by the application for one widget.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WidgetId(u64);

impl WidgetId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable identity for one active pointer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PointerId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub id: PointerId,
    pub phase: PointerPhase,
    pub button: PointerButton,
    pub position: Point,
}

/// Interaction behavior associated with one hit target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetRole {
    Button,
    /// A horizontal slider. `steps` snaps the normalized value into that many
    /// equal intervals; zero keeps the value continuous.
    HorizontalSlider {
        steps: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTarget {
    pub id: WidgetId,
    pub bounds: Rect,
    pub role: WidgetRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetEventKind {
    Pressed,
    Dragged,
    Released,
    Activated,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetEvent {
    pub id: WidgetId,
    pub pointer: PointerId,
    pub kind: WidgetEventKind,
    pub position: Point,
    /// Present for slider events and normalized to the inclusive `[0, 1]`
    /// track domain.
    pub normalized_value: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RouteReport {
    pub widget_events: Vec<WidgetEvent>,
    pub unhandled_events: Vec<PointerEvent>,
    pub captured_pointers: usize,
}

/// Pointer capture and neutral hit routing shared by mouse and touch.
#[derive(Debug, Default)]
pub struct InteractionState {
    captures: BTreeMap<PointerId, HitTarget>,
}

impl InteractionState {
    pub fn route(&mut self, events: &[PointerEvent], targets: &[HitTarget]) -> RouteReport {
        let mut report = RouteReport::default();
        for event in events {
            match event.phase {
                PointerPhase::Down => {
                    let target = targets
                        .iter()
                        .rev()
                        .copied()
                        .find(|target| target.bounds.contains(event.position));
                    if let Some(target) = target {
                        self.captures.insert(event.id, target);
                        report.widget_events.push(widget_event(
                            target,
                            *event,
                            WidgetEventKind::Pressed,
                        ));
                    } else {
                        report.unhandled_events.push(*event);
                    }
                }
                PointerPhase::Move => {
                    if let Some(target) = self.current_target(event.id, targets) {
                        report.widget_events.push(widget_event(
                            target,
                            *event,
                            WidgetEventKind::Dragged,
                        ));
                    } else {
                        report.unhandled_events.push(*event);
                    }
                }
                PointerPhase::Up => {
                    if let Some(target) = self.current_target(event.id, targets) {
                        report.widget_events.push(widget_event(
                            target,
                            *event,
                            WidgetEventKind::Released,
                        ));
                        if target.bounds.contains(event.position)
                            || matches!(target.role, WidgetRole::HorizontalSlider { .. })
                        {
                            report.widget_events.push(widget_event(
                                target,
                                *event,
                                WidgetEventKind::Activated,
                            ));
                        }
                        self.captures.remove(&event.id);
                    } else {
                        report.unhandled_events.push(*event);
                    }
                }
                PointerPhase::Cancel => {
                    if let Some(target) = self.current_target(event.id, targets) {
                        report.widget_events.push(widget_event(
                            target,
                            *event,
                            WidgetEventKind::Cancelled,
                        ));
                        self.captures.remove(&event.id);
                    } else {
                        report.unhandled_events.push(*event);
                    }
                }
            }
        }
        report.captured_pointers = self.captures.len();
        report
    }

    pub fn cancel_all(&mut self) {
        self.captures.clear();
    }

    pub fn captured_pointers(&self) -> usize {
        self.captures.len()
    }

    pub fn is_captured(&self, pointer: PointerId) -> bool {
        self.captures.contains_key(&pointer)
    }

    fn current_target(&mut self, pointer: PointerId, targets: &[HitTarget]) -> Option<HitTarget> {
        let captured = *self.captures.get(&pointer)?;
        let current = targets
            .iter()
            .copied()
            .find(|target| target.id == captured.id)
            .unwrap_or(captured);
        self.captures.insert(pointer, current);
        Some(current)
    }
}

fn widget_event(target: HitTarget, event: PointerEvent, kind: WidgetEventKind) -> WidgetEvent {
    WidgetEvent {
        id: target.id,
        pointer: event.id,
        kind,
        position: event.position,
        normalized_value: match target.role {
            WidgetRole::Button => None,
            WidgetRole::HorizontalSlider { steps } => {
                let raw = ((event.position.x - target.bounds.x)
                    / target.bounds.width.max(0.000_001))
                .clamp(0.0, 1.0);
                Some(if steps == 0 {
                    raw
                } else {
                    (raw * steps as f32).round() / steps as f32
                })
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(id: u64, phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            id: PointerId(id),
            phase,
            button: PointerButton::Primary,
            position: Point::new(x, y),
        }
    }

    #[test]
    fn topmost_target_captures_until_release() {
        let targets = [
            HitTarget {
                id: WidgetId::new(1),
                bounds: Rect::new(0.0, 0.0, 40.0, 40.0),
                role: WidgetRole::Button,
            },
            HitTarget {
                id: WidgetId::new(2),
                bounds: Rect::new(10.0, 10.0, 40.0, 40.0),
                role: WidgetRole::Button,
            },
        ];
        let report = InteractionState::default().route(
            &[
                pointer(7, PointerPhase::Down, 20.0, 20.0),
                pointer(7, PointerPhase::Move, 70.0, 20.0),
                pointer(7, PointerPhase::Up, 70.0, 20.0),
            ],
            &targets,
        );
        assert_eq!(report.widget_events.len(), 3);
        assert!(
            report
                .widget_events
                .iter()
                .all(|event| event.id == WidgetId::new(2))
        );
        assert!(
            !report
                .widget_events
                .iter()
                .any(|event| event.kind == WidgetEventKind::Activated)
        );
        assert_eq!(report.captured_pointers, 0);
    }

    #[test]
    fn slider_drag_clamps_and_snaps_outside_track() {
        let target = HitTarget {
            id: WidgetId::new(4),
            bounds: Rect::new(20.0, 10.0, 100.0, 30.0),
            role: WidgetRole::HorizontalSlider { steps: 4 },
        };
        let mut state = InteractionState::default();
        let report = state.route(
            &[
                pointer(2, PointerPhase::Down, 21.0, 20.0),
                pointer(2, PointerPhase::Move, 84.0, 20.0),
                pointer(2, PointerPhase::Move, 200.0, 20.0),
                pointer(2, PointerPhase::Up, 200.0, 20.0),
            ],
            &[target],
        );
        let values = report
            .widget_events
            .iter()
            .filter_map(|event| event.normalized_value)
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0.0, 0.75, 1.0, 1.0, 1.0]);
        assert_eq!(state.captured_pointers(), 0);
    }

    #[test]
    fn uncaptured_and_cancelled_pointers_remain_isolated() {
        let target = HitTarget {
            id: WidgetId::new(9),
            bounds: Rect::new(0.0, 0.0, 20.0, 20.0),
            role: WidgetRole::Button,
        };
        let mut state = InteractionState::default();
        let report = state.route(
            &[
                pointer(1, PointerPhase::Down, 4.0, 4.0),
                pointer(2, PointerPhase::Down, 50.0, 50.0),
                pointer(1, PointerPhase::Cancel, 4.0, 4.0),
                pointer(2, PointerPhase::Move, 55.0, 55.0),
            ],
            &[target],
        );
        assert_eq!(report.unhandled_events.len(), 2);
        assert!(
            report
                .widget_events
                .iter()
                .any(|event| event.kind == WidgetEventKind::Cancelled)
        );
        assert_eq!(state.captured_pointers(), 0);
    }
}
