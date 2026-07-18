use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyStateV1, ContactIdentityPolicyV1, OracleRequestV1, SCHEMA_VERSION, ScenarioV1,
    SettlingSummaryV2, ShapeV1, Validate, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuNormalRelaxationPolicyV1, GpuSoftFeedbackPolicyV1,
    GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1, P5_2_SETTLING_EVIDENCE_STEPS,
    P5_2BucketShape, P5_2SettlingFixtureKind, p5_2_box2d_world_projection, p5_2_bucket_scenario,
    p5_2_centered_rectangle_stack_scenario, p5_2_minimized_mixed_pile_hard_scenario,
    p5_2_settling_fixture_scenario,
};
use physbox_lab::{
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    settling::{
        AWAKE_SETTLING_TRACE_VERSION, awake_settling_measurement_policy_v2,
        centered_stack_structure, observe_generated_settling_series_v2, settling_fixture_structure,
        settling_series_v2,
    },
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const FULL_CHECKPOINTS: [u32; 7] = [1, 60, 180, 600, 900, 1_050, 1_200];
const BUCKET_CHECKPOINTS: [u32; 7] = [1, 60, 180, 300, 420, 540, 600];
const SUBMISSION_BATCH_STEPS: u32 = 64;

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    Fixture(P5_2SettlingFixtureKind),
    Bucket,
}

impl SceneKind {
    fn name(self) -> &'static str {
        match self {
            Self::Stack => "centered-stack",
            Self::Fixture(kind) => kind.name(),
            Self::Bucket => "bucket",
        }
    }
}

struct SceneInput {
    label: String,
    kind: SceneKind,
    accepted: bool,
    source: ScenarioV1,
    checkpoints: &'static [u32],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    observation_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    policy: physbox_contracts::SettlingMeasurementPolicyV2,
    scenes: Vec<SceneEvidence>,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    kind: String,
    accepted: bool,
    requested_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    selected_product: LaneEvidence,
    serial_control: LaneEvidence,
    authority: Vec<AuthorityEvidence>,
    authority_final_replay_exact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneEvidence {
    lane: &'static str,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    scheduling_policy_digest: String,
    work_vector_digest: String,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    raw_series_hash: String,
    trace: ArtifactIdentity,
    exact_replay: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_loss_step: Option<u32>,
    summary: Option<SettlingSummaryV2>,
    orientation_window: Option<OrientationWindow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrientationWindow {
    sample_steps: [u32; 6],
    settled_linear_speed_max: f32,
    kinetic_energy_max: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityEvidence {
    completed_steps: u32,
    request_hash: String,
    result_payload_hash: String,
    artifact: String,
    dynamic_linear_speed_max: f32,
    dynamic_angular_edge_speed_max: f32,
    contact_points: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s9-baseline-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S9.1 baseline evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("S9.1 output or staging directory already exists".to_owned());
    }
    for directory in ["sources", "traces", "oracles"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenes = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S9.1 baseline {}", input.label);
        scenes.push(evaluate_scene(
            &arguments, &staging, &box2d, &runner, input,
        )?);
    }
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s9.1-baseline-repin",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        policy: awake_settling_measurement_policy_v2(),
        scenes,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s9.1-baseline-repin run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            accepted: true,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    inputs.push(SceneInput {
        label: "permanent-four-body-reproducer".to_owned(),
        kind: SceneKind::Fixture(P5_2SettlingFixtureKind::SmallMixedPile),
        accepted: true,
        source: p5_2_minimized_mixed_pile_hard_scenario().map_err(|error| error.to_string())?,
        checkpoints: &FULL_CHECKPOINTS,
    });
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            accepted: true,
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    for count in [32, 64] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}-diagnostic"),
            kind: SceneKind::Stack,
            accepted: false,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    for shape in P5_2BucketShape::ALL {
        inputs.push(SceneInput {
            label: format!("{}-bucket-100", shape.name()),
            kind: SceneKind::Bucket,
            accepted: true,
            source: p5_2_bucket_scenario(shape, 100, 600).map_err(|error| error.to_string())?,
            checkpoints: &BUCKET_CHECKPOINTS,
        });
    }
    Ok(inputs)
}

fn evaluate_scene(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    input: SceneInput,
) -> Result<SceneEvidence, String> {
    let source_scenario_hash = canonical_hash(&input.source).map_err(|error| error.to_string())?;
    let projection =
        p5_2_box2d_world_projection(&input.source).map_err(|error| error.to_string())?;
    let projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let source = write_json_artifact(
        staging,
        &format!("sources/{}.json", input.label),
        &input.source,
    )?;
    let selected_product = evaluate_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "selected-product",
        None,
    )?;
    let serial_control = evaluate_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "serial-control",
        Some(GpuSolverSchedulingPolicyV1::SERIAL_CONTROL),
    )?;
    let authority = input
        .checkpoints
        .iter()
        .map(|&step| run_authority(arguments, staging, box2d, &input.label, &projection, step))
        .collect::<Result<Vec<_>, _>>()?;
    let replay = run_authority(
        arguments,
        staging,
        box2d,
        &format!("{}-replay", input.label),
        &projection,
        input.source.requested_steps,
    )?;
    let authority_final_replay_exact = authority
        .last()
        .is_some_and(|sample| sample.result_payload_hash == replay.result_payload_hash);
    Ok(SceneEvidence {
        label: input.label,
        kind: input.kind.name().to_owned(),
        accepted: input.accepted,
        requested_steps: input.source.requested_steps,
        source_scenario_hash,
        projection_hash,
        source,
        selected_product,
        serial_control,
        authority,
        authority_final_replay_exact,
    })
}

fn evaluate_lane(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: &SceneInput,
    projection_hash: &str,
    lane: &'static str,
    scheduling_policy: Option<GpuSolverSchedulingPolicyV1>,
) -> Result<LaneEvidence, String> {
    let run_series = || match scheduling_policy {
        None => {
            runner.run_selected_observation_series(&input.source, 1, input.source.requested_steps)
        }
        Some(policy) => runner
            .run_contact_policy_observation_series_with_execution_scheduling_and_contact_feedback(
                &input.source,
                1,
                input.source.requested_steps,
                ContactIdentityPolicyV1::SELECTED,
                SUBMISSION_BATCH_STEPS,
                GpuSolverExecutionV4::SELECTED,
                policy,
                GpuSoftFeedbackPolicyV1::SELECTED,
                GpuNormalRelaxationPolicyV1::SELECTED,
            ),
    };
    let raw = run_series()?;
    let replay = run_series()?;
    let raw_series_hash = serialized_hash(&raw)?;
    let exact_replay = raw_series_hash == serialized_hash(&replay)?;
    let final_only = match scheduling_policy {
        None => runner.run(&input.source, input.source.requested_steps)?,
        Some(policy) => runner.run_scheduling_experiment(
            &input.source,
            input.source.requested_steps,
            GpuSolverExecutionV4::SELECTED,
            policy,
        )?,
    };
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "empty S9 series".to_owned())?;
    let instrumentation_final_exact = final_snapshot.bodies == final_only.bodies
        && final_snapshot.manifolds == final_only.manifolds
        && final_snapshot.cache == final_only.cache
        && final_snapshot.work == final_only.work
        && final_snapshot.capacities == final_only.capacities;
    let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean);
    let mut first_structural_loss_step = None;
    let mut final_structure = (true, "structure is not gated".to_owned());
    for snapshot in &raw.snapshots {
        let result = structure(
            input.kind,
            &input.source,
            &snapshot.bodies,
            !snapshot_capacity_clean(snapshot),
        );
        if !result.0 && first_structural_loss_step.is_none() {
            first_structural_loss_step = Some(snapshot.completed_steps);
        }
        if snapshot.completed_steps == input.source.requested_steps {
            final_structure = result;
        }
    }
    let (summary, orientation_window) =
        if input.source.requested_steps == P5_2_SETTLING_EVIDENCE_STEPS {
            let observations = observe_generated_settling_series_v2(&input.source, &raw)?;
            let trace = settling_series_v2(
                &input.source,
                Some(projection_hash.to_owned()),
                lane,
                awake_settling_measurement_policy_v2(),
                observations,
                final_structure.0,
                final_structure.1.clone(),
            )?;
            let summary = trace.summary.clone();
            let sample_steps = [900, 960, 1_020, 1_080, 1_140, 1_200];
            let sampled = trace
                .observations
                .iter()
                .filter(|observation| sample_steps.contains(&observation.base.completed_steps))
                .collect::<Vec<_>>();
            if sampled.len() != sample_steps.len() {
                return Err("S9 orientation window omitted a frozen sample".to_owned());
            }
            let orientation = OrientationWindow {
                sample_steps,
                settled_linear_speed_max: sampled
                    .iter()
                    .map(|observation| observation.base.residual_linear_speed_max.get())
                    .fold(0.0_f32, f32::max),
                kinetic_energy_max: sampled
                    .iter()
                    .map(|observation| observation.base.kinetic_energy.get())
                    .fold(0.0_f32, f32::max),
            };
            write_gzip_json_artifact(
                staging,
                &format!("traces/{}-{lane}.json.gz", input.label),
                &trace,
            )?;
            (Some(summary), Some(orientation))
        } else {
            write_gzip_json_artifact(
                staging,
                &format!("traces/{}-{lane}.json.gz", input.label),
                &raw,
            )?;
            (None, None)
        };
    let trace = artifact_identity(staging, &format!("traces/{}-{lane}.json.gz", input.label))?;
    let scheduling_policy = raw.solver_scheduling_policy;
    let work_vector_digest = serialized_hash(&(
        scheduling_policy,
        &final_snapshot.work,
        &final_snapshot.capacities,
    ))?;
    Ok(LaneEvidence {
        lane,
        scheduling_policy,
        scheduling_policy_digest: serialized_hash(&scheduling_policy)?,
        work_vector_digest,
        dispatches_per_step: raw.dispatches_per_step,
        allocated_bytes: raw.allocated_bytes,
        raw_series_hash,
        trace,
        exact_replay,
        instrumentation_final_exact,
        capacity_clean,
        structural_pass: first_structural_loss_step.is_none(),
        structural_reason: final_structure.1,
        first_structural_loss_step,
        summary,
        orientation_window,
    })
}

fn snapshot_capacity_clean(snapshot: &physbox_engine::GpuGeneratedContactSnapshotV1) -> bool {
    snapshot.failure.is_none()
        && snapshot
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
}

fn structure(
    kind: SceneKind,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    overflowed: bool,
) -> (bool, String) {
    match kind {
        SceneKind::Stack => centered_stack_structure(source, bodies, overflowed, 0.0),
        SceneKind::Fixture(kind) => {
            settling_fixture_structure(kind, source, bodies, overflowed, 0.0)
        }
        SceneKind::Bucket => {
            let valid = !overflowed
                && bodies.len() == source.bodies.len()
                && bodies.iter().all(|body| body.validate().is_ok());
            if valid {
                (
                    true,
                    "all bucket bodies remain finite and capacity-clean".to_owned(),
                )
            } else {
                (false, "bucket body or capacity contract failed".to_owned())
            }
        }
    }
}

fn run_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
    completed_steps: u32,
) -> Result<AuthorityEvidence, String> {
    let mut projection = projection.clone();
    projection.requested_steps = completed_steps;
    let request_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-{label}-{completed_steps:04}", arguments.run_id),
        source_scenario_hash: request_hash.clone(),
        scenario: projection,
    };
    let oracle = run_oracle(
        &OracleRunConfig {
            artifact_root: staging.join("oracles"),
            timeout: Duration::from_secs(20),
            source: box2d.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![OsString::from("--lane"), OsString::from("world-awake")],
            },
        },
        &request,
    )
    .map_err(|error| error.to_string())?;
    let result_payload_hash = serialized_hash(&(
        &oracle.result.bodies,
        &oracle.result.manifolds,
        &oracle.result.traces,
        &oracle.result.metrics,
    ))?;
    let artifact = oracle
        .artifacts
        .directory
        .strip_prefix(staging)
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let (dynamic_linear_speed_max, dynamic_angular_edge_speed_max) =
        body_speeds(&oracle.result.bodies);
    let contact_points = oracle
        .result
        .manifolds
        .iter()
        .map(|manifold| manifold.points.len() as u32)
        .sum();
    Ok(AuthorityEvidence {
        completed_steps,
        request_hash,
        result_payload_hash,
        artifact,
        dynamic_linear_speed_max,
        dynamic_angular_edge_speed_max,
        contact_points,
    })
}

fn body_speeds(bodies: &[BodyStateV1]) -> (f32, f32) {
    bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .fold((0.0_f32, 0.0_f32), |(linear, angular), body| {
            let linear_speed =
                (body.linear_velocity.x.powi(2) + body.linear_velocity.y.powi(2)).sqrt();
            let radius = match body.shape {
                ShapeV1::Circle { radius } => radius.get(),
                ShapeV1::Rectangle { half_extents } => {
                    (half_extents.x.powi(2) + half_extents.y.powi(2)).sqrt()
                }
            };
            (
                linear.max(linear_speed),
                angular.max(body.angular_velocity.get().abs() * radius),
            )
        })
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| error.to_string())
}

fn write_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    write_artifact(root, relative, &compressed)
}

fn artifact_identity(root: &Path, relative: &str) -> Result<ArtifactIdentity, String> {
    let bytes = fs::read(root.join(relative)).map_err(|error| error.to_string())?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(&bytes),
    })
}

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    atomic_write(&root.join(relative), bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
    })
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = path.with_extension("partial");
    if path.exists() || staging.exists() {
        return Err(format!("refusing to replace artifact {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(staging, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    let status = command_output(
        repository,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let patch_digest = if dirty {
        let output = Command::new("git")
            .args(["diff", "--binary", "HEAD"])
            .current_dir(repository)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err("git diff failed while recording provenance".to_owned());
        }
        Some(sha256_bytes(&output.stdout))
    } else {
        None
    };
    Ok((revision, dirty, patch_digest))
}

fn command_output(directory: &Path, program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut require_clean = false;
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
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s9_baseline_catalog_is_frozen() {
        let inputs = scene_inputs().unwrap();
        assert_eq!(inputs.len(), 15);
        assert_eq!(inputs.iter().filter(|scene| scene.accepted).count(), 13);
        assert_eq!(inputs.iter().filter(|scene| !scene.accepted).count(), 2);
        assert!(
            inputs
                .iter()
                .find(|scene| scene.label == "permanent-four-body-reproducer")
                .unwrap()
                .source
                .settings
                .contact_softness
                .is_none()
        );
    }
}
