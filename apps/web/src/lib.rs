#![forbid(unsafe_code)]

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
use physbox_contracts::{NamedValueV1, ProvenanceV1};
#[cfg(any(target_arch = "wasm32", test))]
use physbox_engine::PointerButton;
#[cfg(target_arch = "wasm32")]
use physbox_engine::{
    DisplayMetrics, EngineConfig, FrameInput, GpuBodyBenchmarkSession, InputEvent, PointerEvent,
    PointerPhase, StepRequest, SurfaceRenderer, WheelEvent, create_instance,
};
#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
use physbox_engine::{
    GpuBroadphaseBrowserSession, LiquidScaleBenchmarkConfigV1, LiquidScaleBenchmarkSession,
    LiquidScaleHostKindV1, LiquidScaleSolidTier, P5_3BroadphasePattern,
};
#[cfg(target_arch = "wasm32")]
use physbox_launch::{WebPreferenceController, WebPreferenceStore};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, Storage};

#[cfg(any(target_arch = "wasm32", test))]
const SMOKE_OBSERVER_QUERY_KEY: &str = "smoke-observer";

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

#[cfg(feature = "box2d-comparator")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = box2dComparatorEnabled))]
pub fn box2d_comparator_enabled() -> bool {
    true
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
    preferences: WebPreferenceController,
    smoke_observer_enabled: bool,
    smoke_observer_frame: Option<String>,
    last_timestamp_ms: Option<f64>,
    pending_input_events: Vec<InputEvent>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebApp {
    #[wasm_bindgen(js_name = observerEnabled)]
    pub fn observer_enabled(&self) -> bool {
        self.smoke_observer_enabled
    }

    #[wasm_bindgen(js_name = diagnosticsJson)]
    pub fn diagnostics_json(&self) -> Result<String, JsValue> {
        self.require_smoke_observer()?;
        Ok(self.renderer.diagnostics().marker_json())
    }

    #[wasm_bindgen(js_name = observerSnapshot)]
    pub fn observer_snapshot(&self) -> Result<String, JsValue> {
        self.require_smoke_observer()?;
        self.smoke_observer_frame
            .clone()
            .ok_or_else(|| js_error("smoke observer has not captured a frame"))
    }

    pub fn render(&mut self, timestamp_ms: f64) -> Result<(), JsValue> {
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
        if let Some(search) = self.preferences.synchronize(
            self.renderer.launch_preference_selection(),
            &BrowserPreferenceStore,
        ) {
            replace_browser_search(&search, self.smoke_observer_enabled)?;
        }
        if self.smoke_observer_enabled {
            self.smoke_observer_frame = Some(report.marker_json());
        }
        Ok(())
    }

    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        pixels_per_point: f64,
    ) -> Result<(), JsValue> {
        self.renderer.resize(
            width,
            height,
            DisplayMetrics::new(pixels_per_point).map_err(js_error)?,
        );
        Ok(())
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

    fn require_smoke_observer(&self) -> Result<(), JsValue> {
        self.smoke_observer_enabled
            .then_some(())
            .ok_or_else(|| js_error("semantic browser observation requires smoke-observer=1"))
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(
    canvas: HtmlCanvasElement,
    search: String,
    comparator_runtime: JsValue,
) -> Result<WebApp, JsValue> {
    console_error_panic_hook::set_once();
    #[cfg(feature = "box2d-comparator")]
    physbox_box2d_comparator::install_web_runtime(comparator_runtime).map_err(js_error)?;
    #[cfg(not(feature = "box2d-comparator"))]
    let _ = comparator_runtime;
    let (search, smoke_observer_enabled) = split_smoke_observer_query(&search);
    let (preferences, request) = WebPreferenceController::restore(&search, &BrowserPreferenceStore)
        .map_err(|error| js_error(error.to_string()))?;
    let options = request.options;
    let display_metrics = DisplayMetrics::new(
        web_sys::window()
            .ok_or_else(|| js_error("browser window is unavailable"))?
            .device_pixel_ratio(),
    )
    .map_err(js_error)?;
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
        display_metrics,
        EngineConfig::try_from(&options).map_err(js_error)?,
    )
    .await
    .map_err(js_error)?;
    Ok(WebApp {
        renderer,
        preferences,
        smoke_observer_enabled,
        smoke_observer_frame: None,
        last_timestamp_ms: None,
        pending_input_events: Vec::new(),
    })
}

#[cfg(target_arch = "wasm32")]
struct BrowserPreferenceStore;

#[cfg(target_arch = "wasm32")]
impl WebPreferenceStore for BrowserPreferenceStore {
    fn read(&self, key: &str) -> Result<Option<String>, String> {
        browser_storage()?
            .map(|storage| storage.get_item(key).map_err(js_value_error))
            .transpose()
            .map(Option::flatten)
    }

    fn write(&self, key: &str, value: &str) -> Result<(), String> {
        if let Some(storage) = browser_storage()? {
            storage.set_item(key, value).map_err(js_value_error)?;
        }
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), String> {
        if let Some(storage) = browser_storage()? {
            storage.remove_item(key).map_err(js_value_error)?;
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_storage() -> Result<Option<Storage>, String> {
    web_sys::window()
        .ok_or_else(|| "browser window is unavailable".to_owned())?
        .local_storage()
        .map_err(js_value_error)
}

#[cfg(target_arch = "wasm32")]
fn replace_browser_search(search: &str, retain_smoke_observer: bool) -> Result<(), JsValue> {
    let search = if !retain_smoke_observer {
        search.to_owned()
    } else if search.is_empty() {
        format!("?{SMOKE_OBSERVER_QUERY_KEY}=1")
    } else {
        format!("{search}&{SMOKE_OBSERVER_QUERY_KEY}=1")
    };
    web_sys::window()
        .ok_or_else(|| js_error("browser window is unavailable"))?
        .history()?
        .replace_state_with_url(&JsValue::NULL, "", Some(&search))
}

#[cfg(target_arch = "wasm32")]
fn js_value_error(value: JsValue) -> String {
    format!("{value:?}")
}

#[cfg(any(target_arch = "wasm32", test))]
fn split_smoke_observer_query(search: &str) -> (String, bool) {
    let mut observer_enabled = false;
    let retained = form_urlencoded::parse(search.trim_start_matches('?').as_bytes())
        .filter_map(|(key, value)| {
            if key == SMOKE_OBSERVER_QUERY_KEY {
                observer_enabled |= value == "1";
                None
            } else {
                Some((key.into_owned(), value.into_owned()))
            }
        })
        .collect::<Vec<_>>();
    if retained.is_empty() {
        return (String::new(), observer_enabled);
    }
    let mut encoded = form_urlencoded::Serializer::new(String::new());
    encoded.extend_pairs(
        retained
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    (format!("?{}", encoded.finish()), observer_enabled)
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

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
#[wasm_bindgen(js_name = runLiquidScaleBenchmark)]
#[allow(clippy::too_many_arguments)]
pub async fn run_liquid_scale_benchmark(
    solid_tier: String,
    particle_count: u32,
    particle_capacity: u32,
    warmup_steps: u32,
    samples: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    browser_version: String,
    host: String,
    toolchain: String,
    timing_mode: String,
    render_width: u32,
    render_height: u32,
    interactive_duration_seconds: f64,
) -> Result<String, JsValue> {
    require_smoke_observer_query()?;
    let solid_tier = parse_liquid_solid_tier(&solid_tier).map_err(js_error)?;
    let full_frame = match timing_mode.as_str() {
        "physics-only" => false,
        "full-frame" => true,
        _ => {
            return Err(js_error(
                "liquid timing mode must be physics-only or full-frame".to_owned(),
            ));
        }
    };
    let config = if full_frame {
        LiquidScaleBenchmarkConfigV1::full_frame(
            solid_tier,
            particle_count,
            warmup_steps,
            samples,
            [render_width, render_height],
            interactive_duration_seconds,
        )
        .with_particle_capacity(particle_capacity)
    } else {
        LiquidScaleBenchmarkConfigV1::physics_only(
            solid_tier,
            particle_count,
            warmup_steps,
            samples,
        )
        .with_particle_capacity(particle_capacity)
    };
    let outer_dt_seconds = config.outer_dt_seconds;
    let provenance = ProvenanceV1 {
        run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        host,
        adapter: "runtime-discovered".to_owned(),
        backend: "runtime-discovered".to_owned(),
        driver: "runtime-discovered".to_owned(),
        toolchain,
        build_options: vec![
            NamedValueV1 {
                name: "box2d-comparator".to_owned(),
                value: "false".to_owned(),
            },
            NamedValueV1 {
                name: "profile".to_owned(),
                value: "release".to_owned(),
            },
        ],
        runtime_settings: vec![
            NamedValueV1 {
                name: "coupling-quality".to_owned(),
                value: "standard".to_owned(),
            },
            NamedValueV1 {
                name: "outer-dt-seconds".to_owned(),
                value: format!("{:.17}", config.outer_dt_seconds),
            },
            NamedValueV1 {
                name: "timing-mode".to_owned(),
                value: if full_frame {
                    "full-frame-interactive"
                } else {
                    "physics-only-explicit-steps"
                }
                .to_owned(),
            },
        ],
    };
    let mut session = LiquidScaleBenchmarkSession::new(
        config,
        LiquidScaleHostKindV1::BrowserWebGpu,
        Some(browser_version),
        provenance,
    )
    .await
    .map_err(js_error)?;
    session.submit_warmup().map_err(js_error)?;
    wait_for_liquid_completion(&session).await?;

    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    let paced_started = performance_now()?;
    for sample in 0..samples {
        if full_frame {
            wait_until_performance(paced_started + f64::from(sample) * outer_dt_seconds * 1_000.0)
                .await?;
        }
        let started = performance_now()?;
        session.submit_sample().map_err(js_error)?;
        cpu_encode_submit_seconds.push((performance_now()? - started) * 0.001);
        wait_for_liquid_completion(&session).await?;
        host_completion_seconds.push((performance_now()? - started) * 0.001);
    }
    let elapsed_seconds = if full_frame {
        wait_until_performance(paced_started + interactive_duration_seconds * 1_000.0).await?;
        (performance_now()? - paced_started) * 0.001
    } else {
        0.0
    };
    let readback = session.begin_final_readback().map_err(js_error)?;
    wait_for_liquid_readback(&session, &readback).await?;
    let readback = readback.finish().map_err(js_error)?;
    let run = if full_frame {
        session.complete_full_frame(
            readback,
            cpu_encode_submit_seconds,
            host_completion_seconds,
            elapsed_seconds,
        )
    } else {
        session.complete(readback, cpu_encode_submit_seconds, host_completion_seconds)
    }
    .map_err(js_error)?;
    serde_json::to_string(&run.artifact).map_err(|error| js_error(error.to_string()))
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
async fn wait_until_performance(deadline_ms: f64) -> Result<(), JsValue> {
    while performance_now()? < deadline_ms {
        yield_browser().await?;
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
fn parse_liquid_solid_tier(value: &str) -> Result<LiquidScaleSolidTier, String> {
    match value {
        "water-only" => Ok(LiquidScaleSolidTier::WaterOnly),
        "20" => Ok(LiquidScaleSolidTier::Solids20),
        "100" => Ok(LiquidScaleSolidTier::Solids100),
        _ => Err("liquid solid tier must be water-only, 20, or 100".to_owned()),
    }
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
fn require_smoke_observer_query() -> Result<(), JsValue> {
    let search = web_sys::window()
        .ok_or_else(|| js_error("browser window is unavailable"))?
        .location()
        .search()?;
    split_smoke_observer_query(&search)
        .1
        .then_some(())
        .ok_or_else(|| js_error("liquid benchmark requires smoke-observer=1"))
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
#[wasm_bindgen(js_name = runBroadphaseBrowserProbe)]
pub async fn run_broadphase_browser_probe(
    counts_csv: String,
    patterns_csv: String,
    seed: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<String, JsValue> {
    let counts = parse_body_counts(&counts_csv).map_err(js_error)?;
    if counts.iter().any(|count| *count < 2) || warmup_steps == 0 || samples == 0 {
        return Err(js_error(
            "browser broadphase probe requires at least two proxies and positive sample counts",
        ));
    }
    let patterns = parse_broadphase_patterns(&patterns_csv).map_err(js_error)?;
    let mut rows = Vec::new();
    for count in counts {
        for pattern in patterns.iter().copied() {
            if pattern == P5_3BroadphasePattern::CompleteOverlap && count > 32 {
                continue;
            }
            let case_seed = u64::from(seed) + u64::from(count) * 17 + pattern as u64;
            let session = GpuBroadphaseBrowserSession::new(pattern, count, case_seed)
                .await
                .map_err(js_error)?;
            for _ in 0..warmup_steps {
                let completion = session.submit_sap().await.map_err(js_error)?;
                wait_for_broadphase_submission(&session, completion).await?;
                let completion = session.submit_grid().await.map_err(js_error)?;
                wait_for_broadphase_submission(&session, completion).await?;
            }
            let mut sap_host_completion_seconds = Vec::with_capacity(samples as usize);
            let mut grid_host_completion_seconds = Vec::with_capacity(samples as usize);
            for _ in 0..samples {
                let started = performance_now()?;
                let completion = session.submit_sap().await.map_err(js_error)?;
                wait_for_broadphase_submission(&session, completion).await?;
                sap_host_completion_seconds.push((performance_now()? - started) * 0.001);
                let started = performance_now()?;
                let completion = session.submit_grid().await.map_err(js_error)?;
                wait_for_broadphase_submission(&session, completion).await?;
                grid_host_completion_seconds.push((performance_now()? - started) * 0.001);
            }
            rows.push(serde_json::json!({
                "probe": session.probe(),
                "warmupSteps": warmup_steps,
                "samples": samples,
                "sapHostCompletionSeconds": sap_host_completion_seconds,
                "gridHostCompletionSeconds": grid_host_completion_seconds,
                "completed": true,
            }));
        }
    }
    serde_json::to_string(&rows).map_err(|error| js_error(error.to_string()))
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
fn parse_broadphase_patterns(value: &str) -> Result<Vec<P5_3BroadphasePattern>, String> {
    let patterns = value
        .split(',')
        .map(|name| {
            P5_3BroadphasePattern::ALL
                .into_iter()
                .find(|pattern| pattern.name() == name)
                .ok_or_else(|| format!("unknown broadphase pattern: {name}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if patterns.is_empty() {
        return Err("browser broadphase patterns must be nonempty".to_owned());
    }
    Ok(patterns)
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

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
async fn wait_for_liquid_completion(session: &LiquidScaleBenchmarkSession) -> Result<(), JsValue> {
    let completion = session.begin_completion_fence();
    let started = performance_now()?;
    while !completion.is_ready() {
        session.poll().map_err(js_error)?;
        if performance_now()? - started > 60_000.0 {
            return Err(js_error("browser liquid benchmark completion timed out"));
        }
        yield_browser().await?;
    }
    completion.finish().map_err(js_error)
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
async fn wait_for_liquid_readback(
    session: &LiquidScaleBenchmarkSession,
    readback: &physbox_engine::PendingLiquidScaleReadback,
) -> Result<(), JsValue> {
    let started = performance_now()?;
    while !readback.is_ready() {
        session.poll().map_err(js_error)?;
        if performance_now()? - started > 60_000.0 {
            return Err(js_error("browser liquid benchmark readback timed out"));
        }
        yield_browser().await?;
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "solver-lab"))]
async fn wait_for_broadphase_submission(
    session: &GpuBroadphaseBrowserSession,
    completion: physbox_engine::GpuBroadphaseBrowserCompletion,
) -> Result<(), JsValue> {
    let started = performance_now()?;
    while !completion.is_ready() {
        session.poll().map_err(js_error)?;
        if performance_now()? - started > 60_000.0 {
            return Err(js_error("browser broadphase probe timed out"));
        }
        yield_browser().await?;
    }
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

    #[test]
    fn smoke_observer_query_is_removed_before_shared_launch_parsing() {
        assert_eq!(
            split_smoke_observer_query("?scenario=rigid-floor&smoke-observer=1"),
            ("?scenario=rigid-floor".to_owned(), true)
        );
        assert_eq!(
            split_smoke_observer_query("?smoke-observer=0&body-capacity=256"),
            ("?body-capacity=256".to_owned(), false)
        );
    }
}
