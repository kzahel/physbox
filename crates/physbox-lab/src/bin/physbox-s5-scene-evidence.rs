use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, CollisionManifoldV1, OracleRequestV1, OracleResultV1, PersistentContactV1,
    SCHEMA_VERSION, ScenarioV1, SettlingSeriesV1, ShapeV1, Validate, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuContactPersistenceTopology, GpuGatherTopologyEvidence, GpuGeneratedContactBenchmarkOutput,
    GpuGeneratedContactRunner, P5_2_BUCKET_EVIDENCE_STEPS, P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    P5_2BucketShape, benchmark_gpu_generated_contacts, p5_2_box2d_world_projection,
    p5_2_bucket_scenario, p5_2_centered_rectangle_stack_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    settling::{SettlingSnapshotInput, centered_stack_structure, observe_settling},
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const STACK_CHECKPOINTS: [u32; 7] = [1, 60, 180, 360, 600, 900, 1_200];
const BUCKET_CHECKPOINTS: [u32; 6] = [1, 60, 180, 360, 480, 600];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    all_pairs_baseline: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    sources: Vec<SourceEvidence>,
    performance: Vec<GpuGeneratedContactBenchmarkOutput>,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    playback: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceEvidence {
    label: String,
    source_scenario_hash: String,
    neutral_projection_hash: String,
    source: ScenarioV1,
    gpu_series: SettlingSeriesV1,
    gpu_trace: Vec<GpuTraceSample>,
    box2d_trace: Vec<Box2dTraceSample>,
    gpu_exact_replay: bool,
    box2d_exact_replay: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuTraceSample {
    completed_steps: u32,
    bodies: Vec<BodyStateV1>,
    manifolds: Vec<CollisionManifoldV1>,
    cache: Vec<PersistentContactV1>,
    work: physbox_contracts::CollisionWorkV1,
    capacities: Vec<physbox_contracts::CollisionCapacityV1>,
    failure: Option<physbox_contracts::FailureV1>,
    persistence_topology: GpuContactPersistenceTopology,
    gather_topology: GpuGatherTopologyEvidence,
    dispatches_per_step: u32,
    allocated_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Box2dTraceSample {
    completed_steps: u32,
    scenario_hash: String,
    artifact: String,
    result: OracleResultV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformArtifact {
    label: String,
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureArtifact<'a> {
    schema_version: u32,
    source_scenario_hash: &'a str,
    label: &'a str,
    structural_reason: &'a str,
    final_bodies: &'a [BodyStateV1],
    reconsideration_trigger: &'static str,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s5-scene-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S5 combined scene evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("oracles")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("failures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut sources = Vec::new();
    for count in [2, 4, 8] {
        let source =
            p5_2_centered_rectangle_stack_scenario(count, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?;
        sources.push(evaluate_source(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &format!("rectangle-stack-{count}"),
            source,
            &STACK_CHECKPOINTS,
            0.0,
            Some(count),
        )?);
    }
    for shape in P5_2BucketShape::ALL {
        let source = p5_2_bucket_scenario(shape, 100, P5_2_BUCKET_EVIDENCE_STEPS)
            .map_err(|error| error.to_string())?;
        sources.push(evaluate_source(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &format!("{}-bucket-100", shape.name()),
            source,
            &BUCKET_CHECKPOINTS,
            -0.75,
            None,
        )?);
    }

    for source in &sources {
        if !source.gpu_series.structural_pass && source.label.starts_with("rectangle-stack-") {
            let final_sample = source.gpu_trace.last().expect("nonempty GPU trace");
            atomic_json(
                &staging
                    .join("failures")
                    .join(format!("{}.json", source.label)),
                &FailureArtifact {
                    schema_version: SCHEMA_VERSION,
                    source_scenario_hash: &source.source_scenario_hash,
                    label: &source.label,
                    structural_reason: &source.gpu_series.structural_reason,
                    final_bodies: &final_sample.bodies,
                    reconsideration_trigger: "S5.4 changes the measured failure only through an accepted in-family candidate",
                },
            )?;
        }
    }

    let mut performance = Vec::new();
    for shape in P5_2BucketShape::ALL {
        let source = p5_2_bucket_scenario(shape, 100, 1).map_err(|error| error.to_string())?;
        performance.push(pollster::block_on(benchmark_gpu_generated_contacts(
            &source,
            arguments.warmup,
            arguments.samples,
        ))?);
    }
    let platform_artifacts = copy_platform_artifacts(&arguments.platforms, &staging)?;
    let playback = render_playback(&sources)?;
    atomic_write(&staging.join("playback.svg"), playback.as_bytes())?;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s5.2-combined-scenes-and-settling",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        all_pairs_baseline: true,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        sources,
        performance,
        platform_artifacts,
        physical_device_gap: arguments.physical_device_gap,
        playback: "playback.svg",
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s5.2-combined-scenes run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn evaluate_source(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    label: &str,
    source: ScenarioV1,
    checkpoints: &[u32],
    support_height: f32,
    stack_count: Option<u32>,
) -> Result<SourceEvidence, String> {
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let projection = p5_2_box2d_world_projection(&source).map_err(|error| error.to_string())?;
    let neutral_projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let mut observations = Vec::new();
    let mut gpu_trace = Vec::new();
    let mut previous_manifolds = None;
    let mut previous_cache = None;
    for &completed_steps in checkpoints {
        let output = runner.run(&source, completed_steps)?;
        let mut final_scenario = source.clone();
        final_scenario.bodies.clone_from(&output.bodies);
        let candidates = enumerate_scalar_candidates(&final_scenario)?;
        let manifolds = generate_scalar_narrowphase(&final_scenario, &candidates.candidates)?;
        observations.push(observe_settling(
            &source,
            SettlingSnapshotInput {
                completed_steps,
                bodies: &output.bodies,
                manifolds: &manifolds,
                cache: &output.cache,
                capacities: &output.capacities,
                previous_manifolds: previous_manifolds.as_deref(),
                previous_cache: previous_cache.as_deref(),
                support_height,
            },
        )?);
        previous_manifolds = Some(manifolds.clone());
        previous_cache = Some(output.cache.clone());
        gpu_trace.push(GpuTraceSample {
            completed_steps,
            bodies: output.bodies,
            manifolds,
            cache: output.cache,
            work: output.work,
            capacities: output.capacities,
            failure: output.failure,
            persistence_topology: output.persistence_topology,
            gather_topology: output.gather_topology,
            dispatches_per_step: output.dispatches_per_step,
            allocated_bytes: output.allocated_bytes,
        });
    }
    let final_gpu = gpu_trace.last().expect("checkpoints are nonempty");
    let overflowed = final_gpu
        .capacities
        .iter()
        .any(|capacity| capacity.overflowed);
    let (structural_pass, structural_reason) = if stack_count.is_some() {
        centered_stack_structure(&source, &final_gpu.bodies, overflowed, support_height)
    } else if final_gpu.failure.is_none() && final_gpu.bodies.len() == source.bodies.len() {
        (
            true,
            "all bucket bodies remained finite and capacity-clean".to_owned(),
        )
    } else {
        (false, "bucket body or capacity contract failed".to_owned())
    };
    let final_step = *checkpoints.last().expect("checkpoints are nonempty");
    let series = SettlingSeriesV1 {
        schema_version: SCHEMA_VERSION,
        observation_version: physbox_contracts::SETTLING_OBSERVATION_VERSION,
        source_scenario_hash: source_scenario_hash.clone(),
        projection_hash: Some(neutral_projection_hash.clone()),
        lane: "gpu-generated-all-pairs-pair-slot-stable-body-edge-jacobi".to_owned(),
        sleeping_enabled: false,
        transient_end_step: 180,
        settled_start_step: if final_step >= 1_200 { 600 } else { 360 },
        observations,
        structural_pass,
        structural_reason,
    };
    series.validate().map_err(|error| error.to_string())?;
    let gpu_replay = runner.run(&source, final_step)?;
    let gpu_exact_replay = gpu_replay.bodies == final_gpu.bodies
        && gpu_replay.cache == final_gpu.cache
        && gpu_replay.work == final_gpu.work;

    let oracle_root = staging.join("oracles");
    let mut box2d_trace = Vec::new();
    for &completed_steps in checkpoints {
        let mut checkpoint_projection = projection.clone();
        checkpoint_projection.requested_steps = completed_steps;
        let scenario_hash =
            canonical_hash(&checkpoint_projection).map_err(|error| error.to_string())?;
        let run_id = format!("{}-{label}-{completed_steps:04}", arguments.run_id);
        let request = OracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id,
            source_scenario_hash: scenario_hash.clone(),
            scenario: checkpoint_projection,
        };
        let oracle = run_oracle(
            &oracle_config(&oracle_root, box2d, &arguments.oracle),
            &request,
        )
        .map_err(|error| error.to_string())?;
        let artifact = oracle
            .artifacts
            .directory
            .strip_prefix(staging)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        box2d_trace.push(Box2dTraceSample {
            completed_steps,
            scenario_hash,
            artifact,
            result: oracle.result,
        });
    }
    let mut replay_projection = projection.clone();
    replay_projection.requested_steps = final_step;
    let replay_hash = canonical_hash(&replay_projection).map_err(|error| error.to_string())?;
    let replay_request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-{label}-replay", arguments.run_id),
        source_scenario_hash: replay_hash,
        scenario: replay_projection,
    };
    let box2d_replay = run_oracle(
        &oracle_config(&oracle_root, box2d, &arguments.oracle),
        &replay_request,
    )
    .map_err(|error| error.to_string())?;
    let final_box2d = &box2d_trace.last().expect("nonempty Box2D trace").result;
    let box2d_exact_replay = box2d_replay.result.bodies == final_box2d.bodies
        && box2d_replay.result.manifolds == final_box2d.manifolds;
    Ok(SourceEvidence {
        label: label.to_owned(),
        source_scenario_hash,
        neutral_projection_hash,
        source,
        gpu_series: series,
        gpu_trace,
        box2d_trace,
        gpu_exact_replay,
        box2d_exact_replay,
    })
}

fn oracle_config(
    artifact_root: &Path,
    source: &OracleSourceIdentity,
    executable: &Path,
) -> OracleRunConfig {
    OracleRunConfig {
        artifact_root: artifact_root.to_owned(),
        timeout: Duration::from_secs(20),
        source: source.clone(),
        process: OracleProcess {
            program: executable.to_owned(),
            arguments: vec![OsString::from("--lane"), OsString::from("world")],
        },
    }
}

fn render_playback(sources: &[SourceEvidence]) -> Result<String, String> {
    let panel_width = 150.0;
    let panel_height = 132.0;
    let columns = sources
        .iter()
        .map(|source| source.gpu_trace.len())
        .max()
        .unwrap_or(1);
    let width = 170.0 + columns as f32 * panel_width;
    let height = 32.0 + sources.len() as f32 * panel_height * 2.0;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#102234\"/><style>text{{font:11px monospace;fill:#d8efff}} .gpu{{fill:#62dce8;stroke:#d8ffff;stroke-width:.6}} .cpu{{fill:#f3a85f;stroke:#ffe1ba;stroke-width:.6}}</style>"
    );
    for (source_index, source) in sources.iter().enumerate() {
        let base_y = 28.0 + source_index as f32 * panel_height * 2.0;
        svg.push_str(&format!(
            "<text x=\"8\" y=\"{}\">{}</text><text x=\"8\" y=\"{}\">GPU</text><text x=\"8\" y=\"{}\">Box2D</text>",
            base_y - 10.0,
            source.label,
            base_y + 56.0,
            base_y + panel_height + 56.0
        ));
        for (column, (gpu, cpu)) in source.gpu_trace.iter().zip(&source.box2d_trace).enumerate() {
            let x = 160.0 + column as f32 * panel_width;
            svg.push_str(&format!(
                "<text x=\"{x}\" y=\"{}\">step {}</text>",
                base_y - 10.0,
                gpu.completed_steps
            ));
            render_bodies(&mut svg, &gpu.bodies, x, base_y, "gpu");
            render_bodies(
                &mut svg,
                &cpu.result.bodies,
                x,
                base_y + panel_height,
                "cpu",
            );
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

fn render_bodies(svg: &mut String, bodies: &[BodyStateV1], x: f32, y: f32, class: &str) {
    let scale = 42.0;
    let center_x = x + 65.0;
    let floor_y = y + 102.0;
    for body in bodies {
        match body.shape {
            ShapeV1::Circle { radius } => svg.push_str(&format!(
                "<circle class=\"{class}\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                center_x + body.position.x * scale,
                floor_y - body.position.y * scale,
                radius.get() * scale
            )),
            ShapeV1::Rectangle { half_extents } => {
                let width = half_extents.x * 2.0 * scale;
                let height = half_extents.y * 2.0 * scale;
                let px = center_x + body.position.x * scale;
                let py = floor_y - body.position.y * scale;
                svg.push_str(&format!(
                    "<rect class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{height}\" transform=\"rotate({} {px} {py})\"/>",
                    px - width * 0.5,
                    py - height * 0.5,
                    -body.angle.get().to_degrees()
                ));
            }
        }
    }
}

fn copy_platform_artifacts(
    inputs: &[(String, PathBuf)],
    staging: &Path,
) -> Result<Vec<PlatformArtifact>, String> {
    let mut output = Vec::new();
    for (label, source) in inputs {
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin");
        let relative = format!("platform/{label}.{extension}");
        let bytes = fs::read(source).map_err(|error| error.to_string())?;
        atomic_write(&staging.join(&relative), &bytes)?;
        output.push(PlatformArtifact {
            label: label.clone(),
            path: relative,
            sha256: sha256_bytes(&bytes),
        });
    }
    output.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(output)
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_text(
        Command::new("git")
            .current_dir(repository)
            .args(["rev-parse", "HEAD"]),
    )?;
    let status = command_bytes(Command::new("git").current_dir(repository).args([
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
    ]))?;
    if status.is_empty() {
        return Ok((revision, false, None));
    }
    let mut patch = status;
    patch.extend(command_bytes(
        Command::new("git")
            .current_dir(repository)
            .args(["diff", "--binary", "HEAD", "--"]),
    )?);
    Ok((revision, true, Some(sha256_bytes(&patch))))
}

fn command_text(command: &mut Command) -> Result<String, String> {
    String::from_utf8(command_bytes(command)?)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}

fn command_bytes(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension("partial");
    if path.exists() || partial.exists() {
        return Err(format!("fresh artifact already exists: {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(partial, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    warmup: u32,
    samples: u32,
    require_clean: bool,
    platforms: Vec<(String, PathBuf)>,
    physical_device_gap: Option<String>,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut warmup = 2;
        let mut samples = 10;
        let mut require_clean = false;
        let mut platforms = Vec::new();
        let mut physical_device_gap = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "--require-clean" {
                require_clean = true;
                continue;
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--oracle" => oracle = Some(PathBuf::from(value)),
                "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
                "--warmup" => warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--samples" => samples = value.parse().map_err(|_| "invalid samples")?,
                "--platform" => {
                    let (label, path) = value
                        .split_once('=')
                        .ok_or_else(|| "platform must be LABEL=PATH".to_owned())?;
                    platforms.push((label.to_owned(), PathBuf::from(path)));
                }
                "--physical-device-gap" => physical_device_gap = Some(value),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        if warmup == 0 || samples == 0 {
            return Err("warmup and samples must be positive".to_owned());
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            warmup,
            samples,
            require_clean,
            platforms,
            physical_device_gap,
        })
    }
}
