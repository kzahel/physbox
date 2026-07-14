use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, SolverSettingsV1, canonical_hash, canonical_json, sha256_bytes,
};
use physbox_engine::{
    AdapterDiagnostics, GpuPrimitiveKind, GpuSolverCandidate, authored_chain_scenario,
    benchmark_gpu_primitive, benchmark_gpu_soft_step_case, run_gpu_soft_step_stress_case,
};
use serde::{Deserialize, Serialize};

use crate::{capacity_is_valid, failure_automation::run_randomized_gpu_corpus};

#[derive(Clone, Debug)]
pub struct EvidenceConfig {
    pub output: PathBuf,
    pub run_id: String,
    pub box2d_benchmark: PathBuf,
    pub android_avd: Option<PathBuf>,
    pub android_device: Option<PathBuf>,
    pub android_device_gap: Option<String>,
    pub warmup_steps: u32,
    pub samples: u32,
    pub corpus_seeds: u32,
    pub quick: bool,
    pub require_clean: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceManifest {
    pub schema_version: u32,
    pub run_id: String,
    pub source_revision: String,
    pub source_dirty: bool,
    pub source_patch_digest: Option<String>,
    pub host: String,
    pub toolchain: String,
    pub profile: String,
    pub warmup_steps: u32,
    pub samples: u32,
    pub corpus_seeds: u32,
    pub solver_timings: Vec<SolverTimingRow>,
    pub correctness: Vec<CorrectnessRow>,
    pub convergence: Vec<ConvergenceRow>,
    pub primitive_timings: Vec<PrimitiveTimingRow>,
    pub box2d_timings: Vec<Box2dTimingRow>,
    pub randomized_corpus_artifact: String,
    pub candidate_playback_manifest: String,
    pub candidate_playback_svg: String,
    pub android_avd_artifact: Option<String>,
    pub android_device_artifact: Option<String>,
    pub android_device_gap: Option<String>,
    pub optional_xpbd_status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingEvidence {
    pub sample_count: u32,
    pub p50_seconds: f64,
    pub p95_seconds: f64,
    pub p99_seconds: f64,
    pub raw_seconds: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolverTimingRow {
    pub candidate: String,
    pub contacts: u32,
    pub bodies: u32,
    pub scenario_hash: String,
    pub settings: SolverSettingsV1,
    pub adapter: AdapterDiagnostics,
    pub gpu: Option<TimingEvidence>,
    pub host_completion: TimingEvidence,
    pub cpu_encode_submit: TimingEvidence,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub dispatches: u32,
    pub allocated_bytes: u64,
    pub high_water_bytes: u64,
    pub capacities: Vec<CapacityRecordV1>,
    pub valid: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectnessRow {
    pub candidate: String,
    pub contacts: u32,
    pub scenario_hash: String,
    pub bodies: u32,
    pub contact_cache_points: u32,
    pub finite_state: bool,
    pub capacity_valid: bool,
    pub replay_match: Option<bool>,
    pub velocity_residual: f64,
    pub color_count: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvergenceRow {
    pub candidate: String,
    pub substeps: u32,
    pub contacts: u32,
    pub velocity_residual: f64,
    pub gpu: Option<TimingEvidence>,
    pub host_completion: TimingEvidence,
    pub dispatches: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveTimingRow {
    pub primitive: String,
    pub elements: u32,
    pub adapter: AdapterDiagnostics,
    pub gpu: Option<TimingEvidence>,
    pub host_completion: TimingEvidence,
    pub cpu_encode_submit: TimingEvidence,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub dispatches: u32,
    pub output_elements: u32,
    pub allocated_bytes: u64,
    pub high_water_bytes: u64,
    pub capacities: Vec<CapacityRecordV1>,
    pub valid: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Box2dTimingRow {
    pub contacts: u32,
    pub bodies: u32,
    pub workers: u32,
    pub scenario_hash: String,
    pub scope: String,
    pub wall_step: TimingEvidence,
    pub contact_count_min: u32,
    pub contact_count_max: u32,
    pub task_count_max: u32,
    pub box2d_bytes_max: u64,
    pub artifact: String,
    pub valid: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidatePlayback {
    schema_version: u32,
    run_id: String,
    source_scenario_hash: String,
    initial_bodies: Vec<BodyStateV1>,
    lanes: Vec<CandidatePlaybackLane>,
    max_position_delta: f32,
    max_velocity_delta: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidatePlaybackLane {
    candidate: String,
    completed_steps: u32,
    bodies: Vec<BodyStateV1>,
}

pub fn generate_evidence(config: &EvidenceConfig) -> Result<EvidenceManifest, String> {
    validate_config(config)?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_revision = git_text(&repository, &["rev-parse", "HEAD"])?;
    let status = git_bytes(
        &repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let source_dirty = !status.is_empty();
    if config.require_clean && source_dirty {
        return Err("evidence generation requires a clean working tree".to_owned());
    }
    let source_patch_digest = dirty_digest(&repository, status)?;
    let toolchain = command_text(Command::new("rustc").arg("--version"), "query rustc")?;
    fs::create_dir(&config.output).map_err(|error| {
        format!(
            "create fresh evidence directory {}: {error}",
            config.output.display()
        )
    })?;
    let box2d_directory = config.output.join("box2d");
    fs::create_dir(&box2d_directory).map_err(|error| error.to_string())?;

    let solver_counts = if config.quick {
        vec![1, 4, 32]
    } else {
        (1..=32).chain([1_000, 10_000, 30_000]).collect()
    };
    let correctness_counts = if config.quick {
        vec![1, 32]
    } else {
        vec![1, 32, 1_000, 10_000, 30_000]
    };
    let primitive_counts = if config.quick {
        vec![1_000]
    } else {
        vec![1_000, 10_000, 30_000]
    };
    let box2d_counts = if config.quick {
        vec![32]
    } else {
        vec![32, 1_000, 10_000, 30_000]
    };
    let convergence_substeps = if config.quick {
        vec![1, 4]
    } else {
        vec![1, 2, 4, 8]
    };

    let mut solver_timings = Vec::new();
    for contacts in solver_counts {
        let scenario =
            authored_chain_scenario(contacts + 1, 0x5eed).map_err(|error| error.to_string())?;
        let scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
        for candidate in candidates() {
            let result = pollster::block_on(benchmark_gpu_soft_step_case(
                &scenario,
                candidate,
                config.warmup_steps,
                config.samples,
            ))?;
            let valid = capacity_is_valid(&result.capacities).is_ok()
                && samples_valid(&result.host_completion_seconds)
                && samples_valid(&result.cpu_encode_submit_seconds)
                && result
                    .gpu_step_seconds
                    .as_ref()
                    .is_none_or(|samples| samples_valid(samples));
            solver_timings.push(SolverTimingRow {
                candidate: candidate.name().to_owned(),
                contacts,
                bodies: contacts + 1,
                scenario_hash: scenario_hash.clone(),
                settings: scenario.settings,
                adapter: result.adapter,
                gpu: result.gpu_step_seconds.map(timing).transpose()?,
                host_completion: timing(result.host_completion_seconds)?,
                cpu_encode_submit: timing(result.cpu_encode_submit_seconds)?,
                timestamp_period_nanoseconds: result.timestamp_period_nanoseconds,
                dispatches: result.dispatches,
                allocated_bytes: allocated_bytes(&result.capacities),
                high_water_bytes: high_water_bytes(&result.capacities),
                capacities: result.capacities,
                valid,
            });
        }
    }

    let mut correctness = Vec::new();
    for contacts in correctness_counts {
        let scenario =
            authored_chain_scenario(contacts + 1, 91).map_err(|error| error.to_string())?;
        let scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
        for candidate in candidates() {
            let result = pollster::block_on(run_gpu_soft_step_stress_case(&scenario, candidate))?;
            let finite_state = bodies_finite(&result.bodies);
            let capacity_valid = capacity_is_valid(&result.capacities).is_ok();
            let replay_match = if contacts <= 32 {
                let replay =
                    pollster::block_on(run_gpu_soft_step_stress_case(&scenario, candidate))?;
                Some(result.bodies == replay.bodies && result.contact_cache == replay.contact_cache)
            } else {
                None
            };
            correctness.push(CorrectnessRow {
                candidate: candidate.name().to_owned(),
                contacts,
                scenario_hash: scenario_hash.clone(),
                bodies: result.bodies.len() as u32,
                contact_cache_points: result.contact_cache.len() as u32,
                finite_state,
                capacity_valid,
                replay_match,
                velocity_residual: velocity_residual(&result.bodies),
                color_count: result.color_count,
            });
        }
    }

    let mut convergence = Vec::new();
    for candidate in candidates() {
        for substeps in &convergence_substeps {
            let mut scenario =
                authored_chain_scenario(33, 91).map_err(|error| error.to_string())?;
            scenario.settings.substeps = *substeps;
            let state = pollster::block_on(run_gpu_soft_step_stress_case(&scenario, candidate))?;
            let benchmark = pollster::block_on(benchmark_gpu_soft_step_case(
                &scenario,
                candidate,
                config.warmup_steps,
                config.samples,
            ))?;
            convergence.push(ConvergenceRow {
                candidate: candidate.name().to_owned(),
                substeps: *substeps,
                contacts: 32,
                velocity_residual: velocity_residual(&state.bodies),
                gpu: benchmark.gpu_step_seconds.map(timing).transpose()?,
                host_completion: timing(benchmark.host_completion_seconds)?,
                dispatches: benchmark.dispatches,
            });
        }
    }

    let mut primitive_timings = Vec::new();
    for elements in primitive_counts {
        for primitive in primitives() {
            let result = pollster::block_on(benchmark_gpu_primitive(
                primitive,
                elements,
                config.warmup_steps,
                config.samples,
            ))?;
            let valid = capacity_is_valid(&result.capacities).is_ok()
                && samples_valid(&result.host_completion_seconds)
                && result
                    .gpu_step_seconds
                    .as_ref()
                    .is_none_or(|samples| samples_valid(samples));
            primitive_timings.push(PrimitiveTimingRow {
                primitive: primitive.name().to_owned(),
                elements,
                adapter: result.adapter,
                gpu: result.gpu_step_seconds.map(timing).transpose()?,
                host_completion: timing(result.host_completion_seconds)?,
                cpu_encode_submit: timing(result.cpu_encode_submit_seconds)?,
                timestamp_period_nanoseconds: result.timestamp_period_nanoseconds,
                dispatches: result.dispatches,
                output_elements: result.output_elements,
                allocated_bytes: allocated_bytes(&result.capacities),
                high_water_bytes: high_water_bytes(&result.capacities),
                capacities: result.capacities,
                valid,
            });
        }
    }

    let box2d_timings =
        run_box2d_matrix(config, &box2d_directory, &box2d_counts, &source_revision)?;
    let corpus = run_randomized_gpu_corpus(1, config.corpus_seeds)?;
    let corpus_path = config.output.join("randomized-corpus.json");
    atomic_json(&corpus_path, &corpus)?;
    let (playback_manifest, playback_svg) = candidate_playback(config)?;
    atomic_json(
        &config.output.join("candidate-playback.json"),
        &playback_manifest,
    )?;
    atomic_write(
        &config.output.join("candidate-playback.svg"),
        playback_svg.as_bytes(),
    )?;

    let android_avd_artifact = copy_android_artifact(
        config.android_avd.as_deref(),
        &config.output,
        "android-avd.json",
        &source_revision,
        config.require_clean,
    )?;
    let android_device_artifact = copy_android_artifact(
        config.android_device.as_deref(),
        &config.output,
        "android-device.json",
        &source_revision,
        config.require_clean,
    )?;
    let manifest = EvidenceManifest {
        schema_version: physbox_contracts::SCHEMA_VERSION,
        run_id: config.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        toolchain,
        profile: if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "release".to_owned()
        },
        warmup_steps: config.warmup_steps,
        samples: config.samples,
        corpus_seeds: config.corpus_seeds,
        solver_timings,
        correctness,
        convergence,
        primitive_timings,
        box2d_timings,
        randomized_corpus_artifact: "randomized-corpus.json".to_owned(),
        candidate_playback_manifest: "candidate-playback.json".to_owned(),
        candidate_playback_svg: "candidate-playback.svg".to_owned(),
        android_avd_artifact,
        android_device_artifact,
        android_device_gap: config.android_device_gap.clone(),
        optional_xpbd_status: "not implemented: Jacobi and colored Soft-Step leave no material comparator question before human review".to_owned(),
    };
    atomic_json(&config.output.join("matrix.json"), &manifest)?;
    sync_directory(&config.output)?;
    Ok(manifest)
}

fn run_box2d_matrix(
    config: &EvidenceConfig,
    output: &Path,
    counts: &[u32],
    source_revision: &str,
) -> Result<Vec<Box2dTimingRow>, String> {
    let mut rows = Vec::new();
    for contacts in counts {
        let scenario =
            authored_chain_scenario(*contacts + 1, 0x5eed).map_err(|error| error.to_string())?;
        let scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
        let request =
            std::env::temp_dir().join(format!("{}-box2d-{contacts}.scenario.json", config.run_id));
        fs::write(
            &request,
            canonical_json(&scenario).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        for workers in [1_u32, 4] {
            let name = format!("contacts-{contacts}-workers-{workers}.json");
            let result_path = output.join(&name);
            let run_id = format!("{}-b2-{contacts}-w{workers}", config.run_id);
            let arguments = vec![
                "--request".to_owned(),
                request.to_string_lossy().into_owned(),
                "--result".to_owned(),
                result_path.to_string_lossy().into_owned(),
                "--run-id".to_owned(),
                run_id,
                "--workers".to_owned(),
                workers.to_string(),
                "--warmup".to_owned(),
                config.warmup_steps.to_string(),
                "--samples".to_owned(),
                config.samples.to_string(),
            ];
            run_process(
                &config.box2d_benchmark,
                &arguments,
                "PHYSBOX5_BOX2D_BENCHMARK_SUCCESS",
                Duration::from_secs(300),
            )?;
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&result_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            if value["sourceRevision"].as_str() != Some("3a4f0da8374af61293a03021c9a0b3ebcfe67948")
                || value["sourceDirty"].as_bool() != Some(false)
            {
                return Err("optimized Box2D result has invalid source provenance".to_owned());
            }
            let raw = json_f64_array(&value["samplesSeconds"])?;
            let contact_min = json_u32(&value, "contactCountMin")?;
            let contact_max = json_u32(&value, "contactCountMax")?;
            rows.push(Box2dTimingRow {
                contacts: *contacts,
                bodies: *contacts + 1,
                workers,
                scenario_hash: scenario_hash.clone(),
                scope: value["scope"].as_str().unwrap_or("unknown").to_owned(),
                wall_step: timing(raw)?,
                contact_count_min: contact_min,
                contact_count_max: contact_max,
                task_count_max: json_u32(&value, "taskCountMax")?,
                box2d_bytes_max: value["box2dBytesMax"]
                    .as_u64()
                    .ok_or_else(|| "Box2D byte counter is missing".to_owned())?,
                artifact: format!("box2d/{name}"),
                valid: contact_min == *contacts
                    && contact_max == *contacts
                    && value["sourceScenarioHash"].as_str() == Some(scenario_hash.as_str()),
            });
        }
        let _ = fs::remove_file(&request);
    }
    if source_revision.is_empty() {
        return Err("missing harness source revision".to_owned());
    }
    Ok(rows)
}

fn candidate_playback(config: &EvidenceConfig) -> Result<(CandidatePlayback, String), String> {
    let scenario = authored_chain_scenario(8, 91).map_err(|error| error.to_string())?;
    let mut lanes = Vec::new();
    for candidate in candidates() {
        let result = pollster::block_on(run_gpu_soft_step_stress_case(&scenario, candidate))?;
        lanes.push(CandidatePlaybackLane {
            candidate: candidate.name().to_owned(),
            completed_steps: 1,
            bodies: result.bodies,
        });
    }
    let (position, velocity) = body_deltas(&lanes[0].bodies, &lanes[1].bodies)?;
    let playback = CandidatePlayback {
        schema_version: physbox_contracts::SCHEMA_VERSION,
        run_id: config.run_id.clone(),
        source_scenario_hash: canonical_hash(&scenario).map_err(|error| error.to_string())?,
        initial_bodies: scenario.bodies,
        lanes,
        max_position_delta: position,
        max_velocity_delta: velocity,
    };
    let svg = render_candidate_svg(&playback);
    Ok((playback, svg))
}

fn render_candidate_svg(playback: &CandidatePlayback) -> String {
    let width = 940;
    let height = 280;
    let colors = ["#7289a0", "#5ee1dc", "#ffc267"];
    let labels = ["initial", "jacobi-gather", "colored-soft-tgs"];
    let lanes = [
        playback.initial_bodies.as_slice(),
        playback.lanes[0].bodies.as_slice(),
        playback.lanes[1].bodies.as_slice(),
    ];
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:11px;fill:#91a8ba}}</style><text class=\"title\" x=\"18\" y=\"28\">PhysBox 5 synchronized candidate playback</text><text class=\"meta\" x=\"18\" y=\"47\">one shared authored chain · one complete fixed step · Δp {:.6} · Δv {:.6}</text>",
        playback.max_position_delta, playback.max_velocity_delta
    );
    for (lane_index, bodies) in lanes.iter().enumerate() {
        let y = 82 + lane_index * 62;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{}\" font-size=\"12\" fill=\"{}\">{}</text><line x1=\"150\" x2=\"910\" y1=\"{}\" y2=\"{}\" stroke=\"#29465a\"/>",
            y + 6,
            colors[lane_index],
            labels[lane_index],
            y,
            y
        ));
        for body in *bodies {
            let x = 170.0 + body.position.x * 88.0;
            let velocity = body.linear_velocity.x * 18.0;
            svg.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{}\" width=\"22\" height=\"22\" rx=\"3\" fill=\"{}\"/><line x1=\"{:.2}\" x2=\"{:.2}\" y1=\"{}\" y2=\"{}\" stroke=\"#f07872\" stroke-width=\"2\"/>",
                x - 11.0,
                y - 11,
                colors[lane_index],
                x,
                x + velocity,
                y,
                y
            ));
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn copy_android_artifact(
    source: Option<&Path>,
    output: &Path,
    name: &str,
    source_revision: &str,
    require_clean: bool,
) -> Result<Option<String>, String> {
    let Some(source) = source else {
        return Ok(None);
    };
    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let report = &value["report"];
    if report["sourceRevision"].as_str() != Some(source_revision) {
        return Err(format!(
            "Android artifact {} source revision mismatch",
            source.display()
        ));
    }
    if require_clean && report["sourceDirty"].as_bool() != Some(false) {
        return Err(format!(
            "Android artifact {} was captured dirty",
            source.display()
        ));
    }
    atomic_write(&output.join(name), &bytes)?;
    Ok(Some(name.to_owned()))
}

fn timing(mut samples: Vec<f64>) -> Result<TimingEvidence, String> {
    if !samples_valid(&samples) {
        return Err("timing samples must be finite, positive, and non-empty".to_owned());
    }
    let raw_seconds = samples.clone();
    samples.sort_by(f64::total_cmp);
    Ok(TimingEvidence {
        sample_count: samples.len() as u32,
        p50_seconds: percentile(&samples, 0.50),
        p95_seconds: percentile(&samples, 0.95),
        p99_seconds: percentile(&samples, 0.99),
        raw_seconds,
    })
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let rank = (percentile * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn samples_valid(samples: &[f64]) -> bool {
    !samples.is_empty()
        && samples
            .iter()
            .all(|sample| sample.is_finite() && *sample > 0.0)
}

fn bodies_finite(bodies: &[BodyStateV1]) -> bool {
    bodies.iter().all(|body| {
        body.position.x.is_finite()
            && body.position.y.is_finite()
            && body.angle.get().is_finite()
            && body.linear_velocity.x.is_finite()
            && body.linear_velocity.y.is_finite()
            && body.angular_velocity.get().is_finite()
    })
}

fn velocity_residual(bodies: &[BodyStateV1]) -> f64 {
    bodies
        .windows(2)
        .map(|pair| f64::from((pair[1].linear_velocity.x - pair[0].linear_velocity.x).abs()))
        .sum()
}

fn body_deltas(left: &[BodyStateV1], right: &[BodyStateV1]) -> Result<(f32, f32), String> {
    if left.len() != right.len() {
        return Err("candidate playback body count mismatch".to_owned());
    }
    let mut position = 0.0_f32;
    let mut velocity = 0.0_f32;
    for (left, right) in left.iter().zip(right) {
        position = position.max(
            ((left.position.x - right.position.x).powi(2)
                + (left.position.y - right.position.y).powi(2))
            .sqrt(),
        );
        velocity = velocity.max(
            ((left.linear_velocity.x - right.linear_velocity.x).powi(2)
                + (left.linear_velocity.y - right.linear_velocity.y).powi(2))
            .sqrt(),
        );
    }
    Ok((position, velocity))
}

fn allocated_bytes(records: &[CapacityRecordV1]) -> u64 {
    records.iter().map(|record| record.allocated_bytes).sum()
}

fn high_water_bytes(records: &[CapacityRecordV1]) -> u64 {
    records.iter().map(|record| record.high_water_bytes).sum()
}

fn candidates() -> [GpuSolverCandidate; 2] {
    [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ]
}

fn primitives() -> [GpuPrimitiveKind; 4] {
    [
        GpuPrimitiveKind::PrefixScan,
        GpuPrimitiveKind::RadixSort,
        GpuPrimitiveKind::Unique,
        GpuPrimitiveKind::Compaction,
    ]
}

fn validate_config(config: &EvidenceConfig) -> Result<(), String> {
    if config.output.exists()
        || config.run_id.is_empty()
        || config.warmup_steps == 0
        || config.samples == 0
        || config.corpus_seeds == 0
        || !config.box2d_benchmark.is_file()
    {
        return Err(
            "evidence config requires a fresh output, run id, executable, and non-zero counts"
                .to_owned(),
        );
    }
    if config.android_device.is_some() && config.android_device_gap.is_some() {
        return Err("Android device artifact and device gap are mutually exclusive".to_owned());
    }
    Ok(())
}

fn run_process(
    executable: &Path,
    arguments: &[String],
    sentinel: &str,
    timeout: Duration,
) -> Result<(), String> {
    let mut child = Command::new(executable)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(format!("process {} timed out", executable.display()));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success()
        || !String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == sentinel)
    {
        return Err(format!(
            "process {} failed: {}",
            executable.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn json_f64_array(value: &serde_json::Value) -> Result<Vec<f64>, String> {
    value
        .as_array()
        .ok_or_else(|| "timing sample array missing".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| "invalid timing sample".to_owned())
        })
        .collect()
}

fn json_u32(value: &serde_json::Value, name: &str) -> Result<u32, String> {
    value[name]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("Box2D {name} missing or out of range"))
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension("partial");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&partial, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn git_text(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    String::from_utf8(git_bytes(repository, arguments)?).map_err(|error| error.to_string())
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    let mut stdout = output.stdout;
    while stdout
        .last()
        .is_some_and(|byte| matches!(byte, b'\n' | b'\r'))
    {
        stdout.pop();
    }
    Ok(stdout)
}

fn dirty_digest(repository: &Path, status: Vec<u8>) -> Result<Option<String>, String> {
    if status.is_empty() {
        return Ok(None);
    }
    let mut patch = status;
    patch.extend(git_bytes(repository, &["diff", "--binary", "HEAD", "--"])?);
    let untracked = git_bytes(
        repository,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for raw_path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(raw_path).map_err(|error| error.to_string())?;
        patch.extend_from_slice(path.as_bytes());
        patch.extend(fs::read(repository.join(path)).map_err(|error| error.to_string())?);
    }
    Ok(Some(sha256_bytes(&patch)))
}

fn command_text(command: &mut Command, action: &str) -> Result<String, String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{action} failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles_retain_raw_samples() {
        let summary = timing((1..=20).map(|value| f64::from(value) / 1000.0).collect()).unwrap();
        assert_eq!(summary.p50_seconds, 0.010);
        assert_eq!(summary.p95_seconds, 0.019);
        assert_eq!(summary.p99_seconds, 0.020);
        assert_eq!(summary.raw_seconds.len(), 20);
    }

    #[test]
    fn candidate_playback_is_compact_and_names_both_lanes() {
        let playback = CandidatePlayback {
            schema_version: 1,
            run_id: "test".to_owned(),
            source_scenario_hash: "0".repeat(64),
            initial_bodies: Vec::new(),
            lanes: vec![
                CandidatePlaybackLane {
                    candidate: "jacobi-gather".to_owned(),
                    completed_steps: 1,
                    bodies: Vec::new(),
                },
                CandidatePlaybackLane {
                    candidate: "colored-soft-tgs".to_owned(),
                    completed_steps: 1,
                    bodies: Vec::new(),
                },
            ],
            max_position_delta: 0.0,
            max_velocity_delta: 0.0,
        };
        let svg = render_candidate_svg(&playback);
        assert!(svg.contains("jacobi-gather"));
        assert!(svg.contains("colored-soft-tgs"));
    }
}
