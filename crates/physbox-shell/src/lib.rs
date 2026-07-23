#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use physbox_engine::{
    AdapterDiagnostics, EngineConfig, FrameInput, FrameReport, InputEvent, PointerButton,
    PointerEvent, PointerPhase, StepRequest, SurfaceRenderer, WheelEvent, create_instance,
};
use physbox_launch::FrameMarkerMode;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowAttributes, WindowId};

const MOUSE_POINTER_ID: u64 = 0;
const TOUCH_POINTER_NAMESPACE: u64 = 1_u64 << 63;
const FALLBACK_REFRESH_RATE_MILLIHERTZ: u32 = 60_000;
const MIN_REFRESH_RATE_MILLIHERTZ: u32 = 1_000;
const MAX_REFRESH_RATE_MILLIHERTZ: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletionMarkerKey {
    scenario: &'static str,
    activations: u64,
    restarts: u64,
}

pub struct FrameMarkerGate {
    mode: FrameMarkerMode,
    first_frame_seen: bool,
    last_completion: Option<CompletionMarkerKey>,
}

impl FrameMarkerGate {
    pub fn new(mode: FrameMarkerMode) -> Self {
        Self {
            mode,
            first_frame_seen: false,
            last_completion: None,
        }
    }

    pub fn should_emit(&mut self, report: &FrameReport) -> bool {
        self.observe(
            report.scenario.name,
            report.scenario.complete,
            report.ui.scenario_activations_total,
            report.ui.scenario_restarts_total,
        )
    }

    fn observe(
        &mut self,
        scenario: &'static str,
        complete: bool,
        activations: u64,
        restarts: u64,
    ) -> bool {
        match self.mode {
            FrameMarkerMode::Off => false,
            FrameMarkerMode::All => true,
            FrameMarkerMode::Milestones => {
                let first_frame = !self.first_frame_seen;
                self.first_frame_seen = true;
                let completion = complete.then_some(CompletionMarkerKey {
                    scenario,
                    activations,
                    restarts,
                });
                let new_completion = completion.is_some() && completion != self.last_completion;
                if completion.is_some() {
                    self.last_completion = completion;
                }
                first_frame || new_completion
            }
        }
    }
}

#[derive(Default)]
pub struct WinitFrameOutcome {
    pub exit_requested: bool,
    pub report: Option<FrameReport>,
}

pub struct WinitFrameDriver {
    engine_config: EngineConfig,
    window: Option<Arc<Window>>,
    renderer: Option<SurfaceRenderer>,
    frame_pacer: NativeFramePacer,
    last_frame: Option<Instant>,
    last_cursor: [f32; 2],
    pending_input_events: Vec<InputEvent>,
}

impl WinitFrameDriver {
    pub fn new(engine_config: EngineConfig) -> Self {
        Self {
            engine_config,
            window: None,
            renderer: None,
            frame_pacer: NativeFramePacer::default(),
            last_frame: None,
            last_cursor: [0.5, 0.5],
            pending_input_events: Vec::new(),
        }
    }

    pub fn resume(
        &mut self,
        event_loop: &ActiveEventLoop,
        attributes: WindowAttributes,
    ) -> Result<Option<AdapterDiagnostics>, String> {
        if let Some(window) = &self.window {
            self.frame_pacer.reset();
            window.request_redraw();
            self.last_frame = None;
            return Ok(None);
        }
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| format!("failed to create window: {error}"))?,
        );
        let size = window.inner_size();
        let instance = create_instance();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("failed to create WebGPU surface: {error}"))?;
        let renderer = pollster::block_on(SurfaceRenderer::new(
            &instance,
            surface,
            size.width,
            size.height,
            self.engine_config.clone(),
        ))?;
        let diagnostics = renderer.diagnostics().clone();
        self.renderer = Some(renderer);
        self.window = Some(window.clone());
        self.frame_pacer.reset();
        self.last_frame = None;
        window.request_redraw();
        Ok(Some(diagnostics))
    }

    pub fn suspend(&mut self) {
        self.renderer = None;
        self.window = None;
        self.frame_pacer.reset();
        self.last_frame = None;
        self.pending_input_events.clear();
    }

    pub fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref() else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        match self.frame_pacer.next_redraw {
            Some(deadline) if Instant::now() >= deadline => {
                self.frame_pacer.next_redraw = None;
                window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    pub fn set_window_title(&self, title: &str) {
        if let Some(window) = &self.window {
            window.set_title(title);
        }
    }

    pub fn handle_window_event(
        &mut self,
        window_id: WindowId,
        event: WindowEvent,
    ) -> Result<WinitFrameOutcome, String> {
        let Some(window) = self.window.as_ref() else {
            return Ok(WinitFrameOutcome::default());
        };
        if window.id() != window_id {
            return Ok(WinitFrameOutcome::default());
        }
        match event {
            WindowEvent::CloseRequested => Ok(WinitFrameOutcome {
                exit_requested: true,
                report: None,
            }),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = window.inner_size();
                self.last_cursor =
                    normalized_position(position.x, position.y, size.width, size.height);
                self.push_pointer(
                    MOUSE_POINTER_ID,
                    PointerPhase::Move,
                    PointerButton::Primary,
                    self.last_cursor,
                );
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::CursorLeft { .. } => {
                self.push_pointer(
                    MOUSE_POINTER_ID,
                    PointerPhase::Cancel,
                    PointerButton::Primary,
                    self.last_cursor,
                );
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = pointer_button(button) {
                    self.push_pointer(
                        MOUSE_POINTER_ID,
                        pointer_phase(state),
                        button,
                        self.last_cursor,
                    );
                }
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_steps = match delta {
                    MouseScrollDelta::LineDelta(_, vertical) => vertical.signum(),
                    MouseScrollDelta::PixelDelta(position) => position.y.signum() as f32,
                };
                self.pending_input_events
                    .push(InputEvent::Wheel(WheelEvent {
                        normalized_position: self.last_cursor,
                        zoom_steps,
                    }));
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::Touch(touch) => {
                let size = window.inner_size();
                let position = normalized_position(
                    touch.location.x,
                    touch.location.y,
                    size.width,
                    size.height,
                );
                self.push_pointer(
                    TOUCH_POINTER_NAMESPACE | touch.id,
                    touch_phase(touch.phase),
                    PointerButton::Contact,
                    position,
                );
                Ok(WinitFrameOutcome::default())
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                if !self.frame_pacer.admit(now) {
                    return Ok(WinitFrameOutcome::default());
                }
                let elapsed = self
                    .last_frame
                    .map_or(1.0 / 60.0, |last| now.duration_since(last).as_secs_f64());
                self.last_frame = Some(now);
                let input = FrameInput {
                    step_request: StepRequest::Interactive {
                        elapsed_seconds: elapsed,
                        requested_time_scale: 1.0,
                        action: Default::default(),
                    },
                    input_events: std::mem::take(&mut self.pending_input_events),
                };
                let report = self
                    .renderer
                    .as_mut()
                    .ok_or_else(|| "surface renderer is not initialized".to_owned())?
                    .render(input)?;
                self.frame_pacer.complete_frame(
                    now,
                    window
                        .current_monitor()
                        .and_then(|monitor| monitor.refresh_rate_millihertz()),
                );
                Ok(WinitFrameOutcome {
                    exit_requested: false,
                    report: Some(report),
                })
            }
            _ => Ok(WinitFrameOutcome::default()),
        }
    }

    fn push_pointer(
        &mut self,
        id: u64,
        phase: PointerPhase,
        button: PointerButton,
        normalized_position: [f32; 2],
    ) {
        self.pending_input_events
            .push(InputEvent::Pointer(PointerEvent {
                id,
                phase,
                button,
                normalized_position,
            }));
    }
}

#[derive(Default)]
struct NativeFramePacer {
    next_redraw: Option<Instant>,
}

impl NativeFramePacer {
    fn reset(&mut self) {
        self.next_redraw = None;
    }

    fn admit(&mut self, now: Instant) -> bool {
        if self.next_redraw.is_some_and(|deadline| now < deadline) {
            return false;
        }
        self.next_redraw = None;
        true
    }

    fn complete_frame(&mut self, started: Instant, refresh_rate_millihertz: Option<u32>) {
        self.next_redraw = Some(started + refresh_interval(refresh_rate_millihertz));
    }
}

fn refresh_interval(refresh_rate_millihertz: Option<u32>) -> Duration {
    let millihertz = refresh_rate_millihertz
        .filter(|rate| (MIN_REFRESH_RATE_MILLIHERTZ..=MAX_REFRESH_RATE_MILLIHERTZ).contains(rate))
        .unwrap_or(FALLBACK_REFRESH_RATE_MILLIHERTZ);
    Duration::from_secs_f64(1_000.0 / f64::from(millihertz))
}

fn normalized_position(x: f64, y: f64, width: u32, height: u32) -> [f32; 2] {
    [
        (x / f64::from(width.max(1))).clamp(0.0, 1.0) as f32,
        (y / f64::from(height.max(1))).clamp(0.0, 1.0) as f32,
    ]
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Auxiliary),
        _ => None,
    }
}

fn pointer_phase(state: ElementState) -> PointerPhase {
    match state {
        ElementState::Pressed => PointerPhase::Down,
        ElementState::Released => PointerPhase::Up,
    }
}

fn touch_phase(phase: TouchPhase) -> PointerPhase {
    match phase {
        TouchPhase::Started => PointerPhase::Down,
        TouchPhase::Moved => PointerPhase::Move,
        TouchPhase::Ended => PointerPhase::Up,
        TouchPhase::Cancelled => PointerPhase::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milestone_markers_emit_once_per_completion_event() {
        let mut gate = FrameMarkerGate::new(FrameMarkerMode::Milestones);
        assert!(gate.observe("bootstrap", false, 0, 0));
        assert!(!gate.observe("bootstrap", false, 0, 0));
        assert!(gate.observe("bootstrap", true, 0, 0));
        assert!(!gate.observe("bootstrap", true, 0, 0));
        assert!(gate.observe("rigid-floor", true, 1, 0));
        assert!(!gate.observe("rigid-floor", true, 1, 0));
        assert!(gate.observe("rigid-floor", true, 1, 1));
    }

    #[test]
    fn all_and_off_marker_modes_are_unambiguous() {
        let mut all = FrameMarkerGate::new(FrameMarkerMode::All);
        assert!(all.observe("bootstrap", true, 0, 0));
        assert!(all.observe("bootstrap", true, 0, 0));

        let mut off = FrameMarkerGate::new(FrameMarkerMode::Off);
        assert!(!off.observe("bootstrap", true, 0, 0));
    }

    #[test]
    fn host_coordinates_are_normalized_once() {
        assert_eq!(normalized_position(320.0, 150.0, 640, 300), [0.5, 0.5]);
        assert_eq!(normalized_position(-2.0, 999.0, 100, 100), [0.0, 1.0]);
    }

    #[test]
    fn host_buttons_map_to_neutral_facts() {
        assert_eq!(
            pointer_button(MouseButton::Left),
            Some(PointerButton::Primary)
        );
        assert_eq!(
            pointer_button(MouseButton::Right),
            Some(PointerButton::Secondary)
        );
        assert_eq!(pointer_button(MouseButton::Other(8)), None);
    }

    #[test]
    fn native_frame_pacer_admits_at_most_the_monitor_refresh_rate() {
        let start = Instant::now();
        let interval = refresh_interval(Some(120_000));
        let mut pacer = NativeFramePacer::default();
        assert!(pacer.admit(start));
        pacer.complete_frame(start, Some(120_000));
        assert!(!pacer.admit(start + interval / 2));
        assert!(pacer.admit(start + interval));
    }

    #[test]
    fn native_frame_pacer_never_schedules_catch_up_redraws() {
        let start = Instant::now();
        let interval = refresh_interval(Some(60_000));
        let late = start + interval * 3;
        let mut pacer = NativeFramePacer::default();
        pacer.complete_frame(start, Some(60_000));
        assert!(pacer.admit(late));
        pacer.complete_frame(late, Some(60_000));
        assert_eq!(pacer.next_redraw, Some(late + interval));
    }

    #[test]
    fn native_frame_pacer_uses_a_bounded_fallback_refresh_rate() {
        assert_eq!(
            refresh_interval(None),
            refresh_interval(Some(FALLBACK_REFRESH_RATE_MILLIHERTZ))
        );
        assert_eq!(
            refresh_interval(Some(0)),
            refresh_interval(Some(FALLBACK_REFRESH_RATE_MILLIHERTZ))
        );
    }
}
