#![forbid(unsafe_code)]

#[cfg(any(target_arch = "wasm32", test))]
use physbox_engine::PointerButton;
#[cfg(target_arch = "wasm32")]
use physbox_engine::{
    EngineConfig, FrameInput, PointerEvent, PointerPhase, StepRequest, SurfaceRenderer,
    create_instance,
};
#[cfg(target_arch = "wasm32")]
use physbox_launch::parse_web_query;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PointerButtonCode {
    Primary = 0,
    Secondary = 1,
    Contact = 2,
    Auxiliary = 3,
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode_pointer_button(code: u8) -> Result<PointerButton, String> {
    match code {
        0 => Ok(PointerButton::Primary),
        1 => Ok(PointerButton::Secondary),
        2 => Ok(PointerButton::Contact),
        3 => Ok(PointerButton::Auxiliary),
        _ => Err(format!("unknown pointer button code {code}")),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WebApp {
    renderer: SurfaceRenderer,
    last_timestamp_ms: Option<f64>,
    pending_pointer_events: Vec<PointerEvent>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebApp {
    #[wasm_bindgen(js_name = diagnosticsJson)]
    pub fn diagnostics_json(&self) -> String {
        self.renderer.diagnostics().marker_json()
    }

    pub fn render(&mut self, timestamp_ms: f64) -> Result<String, JsValue> {
        let elapsed_seconds = self
            .last_timestamp_ms
            .map_or(1.0 / 60.0, |last| (timestamp_ms - last) / 1000.0);
        self.last_timestamp_ms = Some(timestamp_ms);
        let report = self
            .renderer
            .render(FrameInput {
                step_request: StepRequest::Interactive {
                    elapsed_seconds,
                    requested_time_scale: 1.0,
                    action: Default::default(),
                },
                pointer_events: std::mem::take(&mut self.pending_pointer_events),
            })
            .map_err(js_error)?;
        Ok(report.marker_json())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    #[wasm_bindgen(js_name = pointerEvent)]
    pub fn pointer_event(
        &mut self,
        id: u32,
        phase: u8,
        x: f32,
        y: f32,
        button: u8,
    ) -> Result<(), JsValue> {
        let phase = match phase {
            0 => PointerPhase::Down,
            1 => PointerPhase::Move,
            2 => PointerPhase::Up,
            3 => PointerPhase::Cancel,
            _ => return Err(js_error(format!("unknown pointer phase code {phase}"))),
        };
        self.pending_pointer_events.push(PointerEvent {
            id: u64::from(id),
            phase,
            button: decode_pointer_button(button).map_err(js_error)?,
            normalized_position: [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)],
        });
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas: HtmlCanvasElement, search: String) -> Result<WebApp, JsValue> {
    console_error_panic_hook::set_once();
    let request = parse_web_query(&search).map_err(|error| js_error(error.to_string()))?;
    let options = request.options;
    canvas.set_width(options.width);
    canvas.set_height(options.height);
    let instance = create_instance();
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|error| js_error(format!("failed to create browser WebGPU surface: {error}")))?;
    let renderer = SurfaceRenderer::new(
        &instance,
        surface,
        options.width,
        options.height,
        EngineConfig::from(&options),
    )
    .await
    .map_err(js_error)?;
    Ok(WebApp {
        renderer,
        last_timestamp_ms: None,
        pending_pointer_events: Vec::new(),
    })
}

#[cfg(target_arch = "wasm32")]
fn js_error(message: impl Into<String>) -> JsValue {
    js_sys::Error::new(&message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_codes_map_to_neutral_buttons() {
        assert_eq!(decode_pointer_button(0), Ok(PointerButton::Primary));
        assert_eq!(decode_pointer_button(2), Ok(PointerButton::Contact));
        assert!(decode_pointer_button(255).is_err());
    }
}
