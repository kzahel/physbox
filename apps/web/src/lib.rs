#![forbid(unsafe_code)]

#[cfg(any(target_arch = "wasm32", test))]
use physbox_engine::PointerButton;
#[cfg(target_arch = "wasm32")]
use physbox_engine::{
    EngineConfig, FrameInput, GpuBodyBenchmarkSession, InputEvent, PointerEvent, PointerPhase,
    StepRequest, SurfaceRenderer, WheelEvent, create_instance,
};
#[cfg(target_arch = "wasm32")]
use physbox_launch::parse_web_query;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = solverLabEnabled))]
pub fn solver_lab_enabled() -> bool {
    cfg!(feature = "solver-lab")
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
    pending_input_events: Vec<InputEvent>,
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
                input_events: std::mem::take(&mut self.pending_input_events),
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
        self.pending_input_events
            .push(InputEvent::Pointer(PointerEvent {
                id: u64::from(id),
                phase,
                button: decode_pointer_button(button).map_err(js_error)?,
                normalized_position: [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)],
            }));
        Ok(())
    }

    #[wasm_bindgen(js_name = wheelEvent)]
    pub fn wheel_event(&mut self, x: f32, y: f32, zoom_steps: f32) {
        self.pending_input_events
            .push(InputEvent::Wheel(WheelEvent {
                normalized_position: [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)],
                zoom_steps: zoom_steps.signum(),
            }));
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
        EngineConfig::try_from(&options).map_err(js_error)?,
    )
    .await
    .map_err(js_error)?;
    Ok(WebApp {
        renderer,
        last_timestamp_ms: None,
        pending_input_events: Vec::new(),
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = runBodyBenchmark)]
pub async fn run_body_benchmark(
    counts_csv: String,
    seed: u32,
    width: u32,
    height: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<String, JsValue> {
    let counts = parse_body_counts(&counts_csv).map_err(js_error)?;
    let mut lanes = Vec::with_capacity(counts.len());
    for body_count in counts {
        let mut session = GpuBodyBenchmarkSession::new(
            body_count,
            u64::from(seed),
            [width, height],
            warmup_steps,
            samples,
        )
        .await
        .map_err(js_error)?;
        for _ in 0..warmup_steps {
            session.reset();
            wait_for_submission(&session).await?;
            session.submit_step(None).map_err(js_error)?;
            wait_for_submission(&session).await?;
        }
        let mut host_completion_seconds = Vec::with_capacity(samples as usize);
        let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
        for sample in 0..samples {
            session.reset();
            wait_for_submission(&session).await?;
            let started = performance_now()?;
            session.submit_step(Some(sample)).map_err(js_error)?;
            cpu_encode_submit_seconds.push((performance_now()? - started) * 0.001);
            wait_for_submission(&session).await?;
            host_completion_seconds.push((performance_now()? - started) * 0.001);
        }
        let validation = session.begin_final_validation();
        wait_for_mapping(&session, || validation.is_ready()).await?;
        let validation = validation.finish().map_err(js_error)?;
        let gpu_seconds = if let Some(timestamps) = session.begin_timestamp_readback() {
            wait_for_mapping(&session, || timestamps.is_ready()).await?;
            Some(timestamps.finish().map_err(js_error)?)
        } else {
            None
        };
        lanes.push(
            session
                .complete(
                    gpu_seconds,
                    host_completion_seconds,
                    cpu_encode_submit_seconds,
                    validation,
                )
                .map_err(js_error)?,
        );
    }
    serde_json::to_string(&lanes).map_err(|error| js_error(error.to_string()))
}

#[cfg(any(target_arch = "wasm32", test))]
fn parse_body_counts(value: &str) -> Result<Vec<u32>, String> {
    let mut counts = value
        .split(',')
        .map(|count| {
            count
                .parse::<u32>()
                .map_err(|_| format!("invalid body count: {count}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if counts.is_empty() || counts.iter().any(|count| !(1..=100_000).contains(count)) {
        return Err("body counts must be between 1 and 100000".to_owned());
    }
    let original = counts.clone();
    counts.sort_unstable();
    counts.dedup();
    if counts != original {
        return Err("body counts must be sorted and unique".to_owned());
    }
    Ok(counts)
}

#[cfg(target_arch = "wasm32")]
async fn wait_for_submission(session: &GpuBodyBenchmarkSession) -> Result<(), JsValue> {
    let completion = session.begin_completion_fence();
    wait_for_mapping(session, || completion.is_ready()).await?;
    completion.finish().map_err(js_error)
}

#[cfg(target_arch = "wasm32")]
async fn wait_for_mapping(
    session: &GpuBodyBenchmarkSession,
    mut is_ready: impl FnMut() -> bool,
) -> Result<(), JsValue> {
    let started = performance_now()?;
    while !is_ready() {
        session.poll().map_err(js_error)?;
        if performance_now()? - started > 60_000.0 {
            return Err(js_error("browser body benchmark readback timed out"));
        }
        yield_browser().await?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn performance_now() -> Result<f64, JsValue> {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .ok_or_else(|| js_error("browser performance clock is unavailable"))
}

#[cfg(target_arch = "wasm32")]
async fn yield_browser() -> Result<(), JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let Some(window) = web_sys::window() else {
            let _ = reject.call1(
                &JsValue::UNDEFINED,
                &js_error("browser window is unavailable"),
            );
            return;
        };
        if let Err(error) =
            window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 1)
        {
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
        }
    });
    JsFuture::from(promise).await.map(|_| ())
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

    #[test]
    fn body_benchmark_counts_are_bounded_sorted_and_unique() {
        assert_eq!(
            parse_body_counts("1000,10000,100000").unwrap(),
            [1000, 10000, 100000]
        );
        assert!(parse_body_counts("10000,1000").is_err());
        assert!(parse_body_counts("1000,1000").is_err());
        assert!(parse_body_counts("100001").is_err());
    }
}
