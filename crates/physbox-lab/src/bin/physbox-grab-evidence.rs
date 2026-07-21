use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, FiniteF32, GrabTuningV1, NeutralActionV1, OracleRequestV1, SCHEMA_VERSION, Vec2,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    AdapterDiagnostics, EngineConfig, FrameInput, FrameReport, GrabValidationState, InputEvent,
    OffscreenRenderer, PointerButton, PointerEvent, PointerPhase, p5_2_generated_floor_scenario,
    provisional_grab_tuning,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
};
use physbox_launch::ValidationScenario;
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const VIEWPORT: [u32; 2] = [640, 400];
const CHECKPOINTS: [u32; 7] = [1, 2, 4, 8, 14, 15, 30];

#[derive(Clone, Copy)]
struct GrabCase {
    label: &'static str,
    anchor: [f32; 2],
    target: [f32; 2],
}

const CASES: [GrabCase; 3] = [
    GrabCase {
        label: "center-horizontal",
        anchor: [0.0, 0.65],
        target: [0.8, 0.65],
    },
    GrabCase {
        label: "off-center-up",
        anchor: [0.3, 0.65],
        target: [0.3, 0.9],
    },
    GrabCase {
        label: "floor-obstacle",
        anchor: [0.0, 0.65],
        target: [0.0, -0.5],
    },
];

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
    adapter: AdapterDiagnostics,
    tuning: GrabTuningV1,
    contract: ContractSummary,
    outcome: Outcome,
    cases: Vec<CaseEvidence>,
    captures: Vec<String>,
    physbox4_reference: PhysBox4Reference,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractSummary {
    grab_intent_contract_version: u32,
    grab_intent_capacity: u32,
    grab_state_bytes: u32,
    intent_dispatches_on_begin: u32,
    constraint_dispatches_per_admitted_step: u32,
    tether_prepare_dispatches_per_active_frame: u32,
    tether_draw_calls_per_frame: u32,
    gpu_selection_readback: bool,
    selection_order: &'static str,
    authority_mapping: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    cases: usize,
    checkpoints_per_case: usize,
    exact_gpu_replays: usize,
    oracle_failures: usize,
    invariant_failures: Vec<String>,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    constants_provisional: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseEvidence {
    label: &'static str,
    anchor: [f32; 2],
    target: [f32; 2],
    checkpoints: Vec<CheckpointEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointEvidence {
    completed_steps: u32,
    source_scenario_hash: String,
    oracle_projection_hash: String,
    oracle_artifact: String,
    gpu_replay_exact: bool,
    gpu_body: BodyStateV1,
    box2d_body: BodyStateV1,
    position_delta: f32,
    angle_delta: f32,
    linear_velocity_delta: f32,
    angular_velocity_delta: f32,
    gpu_grab: GrabValidationState,
    gpu_frame: FrameReport,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysBox4Reference {
    revision: &'static str,
    source_file: &'static str,
    source_file_revision: &'static str,
    selectively_adapted: Vec<&'static str>,
    excluded: Vec<&'static str>,
}

struct GpuRun {
    adapter: AdapterDiagnostics,
    body: BodyStateV1,
    grab: GrabValidationState,
    frame: FrameReport,
    pixels: Vec<u8>,
}

struct Args {
    run_id: String,
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-grab-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.output.exists() {
        return Err(format!(
            "fresh evidence output already exists: {}",
            args.output.display()
        ));
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    let box2d = verify_clean_git_pin(&args.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let parent = args
        .output
        .parent()
        .ok_or_else(|| "evidence output needs a parent directory".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let staging = parent.join(format!(".{}.partial-{}", args.run_id, std::process::id()));
    if staging.exists() {
        return Err(format!(
            "stale staging directory exists: {}",
            staging.display()
        ));
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;

    let result = generate(
        &args,
        &staging,
        &box2d,
        source_revision,
        source_dirty,
        source_patch_digest,
    );
    match result {
        Ok(manifest) => {
            let manifest_path = staging.join("manifest.json");
            atomic_json(&manifest_path, &manifest)?;
            atomic_write(
                &staging.join("playback.svg"),
                render_svg(&manifest).as_bytes(),
            )?;
            let manifest_sha256 =
                sha256_bytes(&fs::read(&manifest_path).map_err(|error| error.to_string())?);
            sync_directory(&staging)?;
            fs::rename(&staging, &args.output).map_err(|error| error.to_string())?;
            println!(
                "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=gpu-anchored-grab run_id={} manifest_sha256={} output={}",
                args.run_id,
                manifest_sha256,
                args.output.display()
            );
            Ok(())
        }
        Err(error) => Err(format!("{error}; partial artifacts: {}", staging.display())),
    }
}

fn generate(
    args: &Args,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
) -> Result<Manifest, String> {
    let oracle_root = staging.join("oracles");
    let oracle_config = OracleRunConfig {
        artifact_root: oracle_root.clone(),
        timeout: Duration::from_secs(10),
        source: box2d.clone(),
        process: OracleProcess {
            program: args.oracle.clone(),
            arguments: vec![OsString::from("--lane"), OsString::from("world")],
        },
    };
    let tuning = provisional_grab_tuning();
    let mut cases = Vec::new();
    let mut captures = Vec::new();
    let mut adapter = None;
    let mut exact_gpu_replays = 0;
    let mut invariant_failures = Vec::new();

    for case in CASES {
        let mut checkpoints = Vec::new();
        for completed_steps in CHECKPOINTS {
            let gpu = run_gpu(case, completed_steps)?;
            let replay = run_gpu(case, completed_steps)?;
            let gpu_replay_exact = gpu.body == replay.body && gpu.grab == replay.grab;
            exact_gpu_replays += usize::from(gpu_replay_exact);
            adapter.get_or_insert_with(|| gpu.adapter.clone());

            if completed_steps == 14 {
                let capture = format!("tether-{}.png", case.label);
                write_png(
                    &staging.join(&capture),
                    VIEWPORT[0],
                    VIEWPORT[1],
                    &gpu.pixels,
                )?;
                captures.push(capture);
            }

            apply_invariants(case, completed_steps, &gpu, &mut invariant_failures);
            let scenario = oracle_scenario(case, completed_steps, tuning)?;
            let source_scenario_hash =
                canonical_hash(&scenario).map_err(|error| error.to_string())?;
            let request = OracleRequestV1 {
                schema_version: SCHEMA_VERSION,
                run_id: format!("{}-{}-{completed_steps}", args.run_id, case.label),
                source_scenario_hash: source_scenario_hash.clone(),
                scenario,
            };
            let authority =
                run_oracle(&oracle_config, &request).map_err(|error| error.to_string())?;
            let box2d_body = authority.result.bodies[0].clone();
            let oracle_artifact = authority
                .artifacts
                .directory
                .strip_prefix(staging)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .into_owned();
            checkpoints.push(CheckpointEvidence {
                completed_steps,
                source_scenario_hash,
                oracle_projection_hash: authority.projection_hash,
                oracle_artifact,
                gpu_replay_exact,
                position_delta: distance(gpu.body.position, box2d_body.position),
                angle_delta: (gpu.body.angle.get() - box2d_body.angle.get()).abs(),
                linear_velocity_delta: distance(
                    gpu.body.linear_velocity,
                    box2d_body.linear_velocity,
                ),
                angular_velocity_delta: (gpu.body.angular_velocity.get()
                    - box2d_body.angular_velocity.get())
                .abs(),
                gpu_body: gpu.body,
                box2d_body,
                gpu_grab: gpu.grab,
                gpu_frame: gpu.frame,
            });
        }
        cases.push(CaseEvidence {
            label: case.label,
            anchor: case.anchor,
            target: case.target,
            checkpoints,
        });
    }

    let expected_replays = CASES.len() * CHECKPOINTS.len();
    if exact_gpu_replays != expected_replays || !invariant_failures.is_empty() {
        atomic_json(
            &staging.join("failure.json"),
            &serde_json::json!({
                "exactGpuReplays": exact_gpu_replays,
                "expectedGpuReplays": expected_replays,
                "invariantFailures": invariant_failures,
            }),
        )?;
        return Err("grab evidence invariants or exact replay failed".to_owned());
    }

    let first = &cases[0].checkpoints[0];
    Ok(Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2a-s4.3-gpu-anchored-grab",
        run_id: args.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: adapter.expect("three GPU cases ran"),
        tuning,
        contract: ContractSummary {
            grab_intent_contract_version: first
                .gpu_frame
                .body_commands
                .grab_intent_contract_version,
            grab_intent_capacity: first.gpu_frame.body_commands.capacity_grab_intents,
            grab_state_bytes: first.gpu_frame.body_commands.grab_state_bytes,
            intent_dispatches_on_begin: first
                .gpu_frame
                .body_commands
                .grab_intent_dispatches_this_frame,
            constraint_dispatches_per_admitted_step: 1,
            tether_prepare_dispatches_per_active_frame: first
                .gpu_frame
                .body_commands
                .grab_tether_prepare_dispatches_this_frame,
            tether_draw_calls_per_frame: first
                .gpu_frame
                .body_commands
                .grab_tether_draw_calls_this_frame,
            gpu_selection_readback: first.gpu_frame.body_commands.gpu_selection_readback,
            selection_order: "nearest squared center distance, then ascending body slot, then generation",
            authority_mapping: "Box2D public kinematic-body motor joint; hertz=sqrt(k)/tau; dampingRatio=c/(2*sqrt(k)); maxSpringForce=mass*maxAcceleration; product linear/angular safety bounds source-only",
        },
        outcome: Outcome {
            cases: CASES.len(),
            checkpoints_per_case: CHECKPOINTS.len(),
            exact_gpu_replays,
            oracle_failures: 0,
            invariant_failures,
            physical_defaults_changed: false,
            numerical_tolerances_changed: false,
            constants_provisional: true,
        },
        cases,
        captures,
        physbox4_reference: PhysBox4Reference {
            revision: "4295957e15480ef701e6006c7cafb5f0728ef101",
            source_file: "crates/physbox-rigid/src/rapier_backend.rs",
            source_file_revision: "2d514fb",
            selectively_adapted: vec![
                "body-local anchor",
                "point spring acceleration 45",
                "anchor damping rate 10",
                "maximum acceleration 80",
                "visible pointer tether",
            ],
            excluded: vec![
                "Rapier dependency",
                "CPU rigid snapshot",
                "CPU body selection",
                "PhysBox 4 scene compatibility",
                "asynchronous CPU/GPU coupling",
            ],
        },
    })
}

fn run_gpu(case: GrabCase, completed_steps: u32) -> Result<GpuRun, String> {
    let mut renderer = pollster::block_on(OffscreenRenderer::new(
        VIEWPORT[0],
        VIEWPORT[1],
        EngineConfig {
            scenario: ValidationScenario::RigidFloor,
            ..Default::default()
        },
    ))?;
    let frame = physbox_engine::Camera2d::fixed()
        .resolve(VIEWPORT)
        .map_err(|error| error.to_string())?;
    let pointer = |phase, world: [f32; 2]| {
        InputEvent::Pointer(PointerEvent {
            id: 43,
            phase,
            button: PointerButton::Primary,
            normalized_position: frame.world_to_screen(world),
        })
    };
    let mut report = renderer.render(FrameInput {
        step_request: physbox_engine::StepRequest::Exact { steps: 1 },
        input_events: vec![pointer(PointerPhase::Down, case.anchor)],
    });
    let held_steps = completed_steps.saturating_sub(1).min(13);
    if held_steps > 0 {
        report = renderer.render(FrameInput {
            step_request: physbox_engine::StepRequest::Exact { steps: held_steps },
            input_events: vec![pointer(PointerPhase::Move, case.target)],
        });
    }
    if completed_steps >= 15 {
        report = renderer.render(FrameInput {
            step_request: physbox_engine::StepRequest::Exact { steps: 1 },
            input_events: vec![pointer(PointerPhase::Up, case.target)],
        });
        let remaining = completed_steps - 15;
        if remaining > 0 {
            report = renderer.render(FrameInput::exact(remaining));
        }
    }
    let raw = renderer.read_capture_state()?;
    let mut body = p5_2_generated_floor_scenario(1)
        .map_err(|error| error.to_string())?
        .bodies[0]
        .clone();
    body.position = Vec2::new(raw[0], raw[1]);
    body.angle = finite(raw[2])?;
    body.linear_velocity = Vec2::new(raw[4], raw[5]);
    body.angular_velocity = finite(raw[6])?;
    Ok(GpuRun {
        adapter: renderer.diagnostics().clone(),
        body,
        grab: renderer.read_grab_state()?,
        frame: report,
        pixels: renderer.read_pixels()?,
    })
}

fn oracle_scenario(
    case: GrabCase,
    completed_steps: u32,
    tuning: GrabTuningV1,
) -> Result<physbox_contracts::ScenarioV1, String> {
    let mut scenario =
        p5_2_generated_floor_scenario(completed_steps).map_err(|error| error.to_string())?;
    let body = scenario.bodies[0].id;
    let actions = [
        NeutralActionV1::GrabBegin {
            step: 0,
            body,
            world_anchor: Vec2::new(case.anchor[0], case.anchor[1]),
            world_target: Vec2::new(case.anchor[0], case.anchor[1]),
            tuning,
        },
        NeutralActionV1::GrabTarget {
            step: 1,
            world_target: Vec2::new(case.target[0], case.target[1]),
        },
        NeutralActionV1::GrabEnd { step: 14 },
    ];
    scenario.actions = actions
        .into_iter()
        .filter(|action| action.step() < completed_steps)
        .collect();
    Ok(scenario)
}

fn apply_invariants(
    case: GrabCase,
    completed_steps: u32,
    gpu: &GpuRun,
    failures: &mut Vec<String>,
) {
    if !gpu.body.position.x.is_finite()
        || !gpu.body.position.y.is_finite()
        || !gpu.body.angle.get().is_finite()
        || !gpu.body.linear_velocity.x.is_finite()
        || !gpu.body.linear_velocity.y.is_finite()
        || !gpu.body.angular_velocity.get().is_finite()
    {
        failures.push(format!(
            "{} step {completed_steps}: non-finite body",
            case.label
        ));
    }
    if completed_steps <= 14 && !gpu.grab.active {
        failures.push(format!(
            "{} step {completed_steps}: grab unexpectedly inactive",
            case.label
        ));
    }
    if completed_steps >= 15 && gpu.grab.active {
        failures.push(format!(
            "{} step {completed_steps}: release remained active",
            case.label
        ));
    }
    if gpu.body.linear_velocity.x.hypot(gpu.body.linear_velocity.y) > tuning_max_linear() + 1.0e-5 {
        failures.push(format!(
            "{} step {completed_steps}: linear speed exceeded bound",
            case.label
        ));
    }
    if gpu.body.angular_velocity.get().abs() > tuning_max_angular() + 1.0e-5 {
        failures.push(format!(
            "{} step {completed_steps}: angular speed exceeded bound",
            case.label
        ));
    }
    if case.label == "off-center-up" && completed_steps == 14 && gpu.body.angle.get() <= 0.0 {
        failures.push("off-center-up step 14: angular response had wrong sign".to_owned());
    }
    if case.label == "floor-obstacle" && completed_steps == 14 && gpu.body.position.y < 0.17 {
        failures.push("floor-obstacle step 14: collidable body crossed floor".to_owned());
    }
}

fn tuning_max_linear() -> f32 {
    provisional_grab_tuning().max_linear_speed.get()
}

fn tuning_max_angular() -> f32 {
    provisional_grab_tuning().max_angular_speed.get()
}

fn finite(value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| error.to_string())
}

fn distance(a: Vec2, b: Vec2) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn render_svg(manifest: &Manifest) -> String {
    let width = 1120;
    let row_height = 150;
    let height = 90 + manifest.cases.len() as i32 * row_height;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#081322\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:11px;fill:#91a8ba}}.gpu{{fill:#55d6be}}.box{{fill:#ffb86b}}</style><text class=\"title\" x=\"18\" y=\"28\">PhysBox 5 GPU grab / Box2D motor-joint playback</text><text class=\"meta\" x=\"18\" y=\"48\">same neutral actions · checkpoints 1/2/4/8/14/15/30 · objective deltas, no acceptance tolerance</text>"
    );
    for (case_index, case) in manifest.cases.iter().enumerate() {
        let top = 72 + case_index as i32 * row_height;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{}\">{}</text>",
            top + 14,
            case.label
        ));
        for (index, sample) in case.checkpoints.iter().enumerate() {
            let x = 42 + index as i32 * 150;
            let base_y = top + 95;
            let gx = x as f32 + sample.gpu_body.position.x * 25.0;
            let gy = base_y as f32 - sample.gpu_body.position.y * 25.0;
            let bx = x as f32 + sample.box2d_body.position.x * 25.0;
            let by = base_y as f32 - sample.box2d_body.position.y * 25.0;
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#26384c\"/><circle class=\"gpu\" cx=\"{gx:.2}\" cy=\"{gy:.2}\" r=\"5\"/><circle class=\"box\" cx=\"{bx:.2}\" cy=\"{by:.2}\" r=\"3\"/><text class=\"meta\" x=\"{}\" y=\"{}\">s{} Δp {:.3}</text>",
                x - 5,
                base_y,
                x + 105,
                base_y,
                x - 5,
                base_y + 26,
                sample.completed_steps,
                sample.position_delta
            ));
        }
    }
    svg.push_str("</svg>");
    svg
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|error| error.to_string())
}

fn parse_args() -> Result<Args, String> {
    let mut run_id = None;
    let mut output = None;
    let mut oracle = None;
    let mut box2d_source = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        match argument.as_str() {
            "--run-id" => run_id = Some(value),
            "--output" => output = Some(PathBuf::from(value)),
            "--oracle" => oracle = Some(PathBuf::from(value)),
            "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    Ok(Args {
        run_id: run_id.ok_or_else(usage)?,
        output: output.ok_or_else(usage)?,
        oracle: oracle.ok_or_else(usage)?,
        box2d_source: box2d_source.ok_or_else(usage)?,
    })
}

fn usage() -> String {
    "usage: physbox-grab-evidence --run-id ID --output DIR --oracle PATH --box2d-source DIR"
        .to_owned()
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = git(repository, &["rev-parse", "HEAD"])?;
    let status = git_bytes(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let digest = if dirty {
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
            let relative = std::str::from_utf8(raw_path).map_err(|error| error.to_string())?;
            patch.extend_from_slice(relative.as_bytes());
            patch.push(0);
            patch.extend(fs::read(repository.join(relative)).map_err(|error| error.to_string())?);
            patch.push(0);
        }
        Some(sha256_bytes(&patch))
    } else {
        None
    };
    Ok((revision, dirty, digest))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let bytes = git_bytes(repository, arguments)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output.stdout)
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
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
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}
