#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Instant;

use physbox_engine::{
    AdapterDiagnostics, EngineConfig, FrameInput, FrameReport, InputEvent, PointerButton,
    PointerEvent, PointerPhase, StepRequest, SurfaceRenderer, WheelEvent, create_instance,
};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

const MOUSE_POINTER_ID: u64 = 0;
const TOUCH_POINTER_NAMESPACE: u64 = 1_u64 << 63;

#[derive(Default)]
pub struct WinitFrameOutcome {
    pub exit_requested: bool,
    pub report: Option<FrameReport>,
}

pub struct WinitFrameDriver {
    engine_config: EngineConfig,
    window: Option<Arc<Window>>,
    renderer: Option<SurfaceRenderer>,
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
        self.last_frame = None;
        window.request_redraw();
        Ok(Some(diagnostics))
    }

    pub fn suspend(&mut self) {
        self.renderer = None;
        self.window = None;
        self.last_frame = None;
        self.pending_input_events.clear();
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
                window.request_redraw();
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
                window.request_redraw();
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
}
