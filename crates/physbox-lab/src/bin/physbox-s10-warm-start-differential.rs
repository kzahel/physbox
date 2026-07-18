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
    BodyStateV1, OracleRequestV1, SCHEMA_VERSION, ScenarioV1, SettlingSummaryV2, canonical_hash,
    sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuSolverExecutionV4, P5_2_SETTLING_EVIDENCE_STEPS,
    P5_2SettlingFixtureKind, p5_2_box2d_world_projection, p5_2_centered_rectangle_stack_scenario,
    p5_2_minimized_mixed_pile_hard_scenario,
};
use physbox_lab::{
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    settling::{
        AWAKE_SETTLING_TRACE_VERSION, SettlingObservationInputV2, SettlingObserverV2,
        awake_settling_measurement_policy_v2, centered_stack_structure,
        observe_generated_settling_series_v2, settling_fixture_structure, settling_series_v2,
    },
    stationarity::{decode_box2d_step_trace, stationarity_metric_contract_v1},
};
use serde::{Deserialize, Serialize};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    PermanentReproducer,
}

struct SceneInput {
    label: String,
    kind: SceneKind,
    source: ScenarioV1,
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
    stationarity_contract: physbox_lab::stationarity::StationarityMetricContractV1,
    baseline: BaselineIdentity,
    scenes: Vec<SceneEvidence>,
    routing: RoutingVerdict,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaselineIdentity {
    artifact_kind: String,
    source_revision: String,
    manifest_path: String,
    manifest_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    requested_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    physbox_warm_on: GpuLaneEvidence,
    physbox_warm_off: GpuLaneEvidence,
    authority_warm_on: AuthorityLaneEvidence,
    authority_warm_off: AuthorityLaneEvidence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuLaneEvidence {
    lane: &'static str,
    solver_execution: GpuSolverExecutionV4,
    scheduling_policy: physbox_engine::GpuSolverSchedulingPolicyV1,
    raw_series_hash: String,
    trace: ArtifactIdentity,
    exact_replay: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_loss_step: Option<u32>,
    summary: SettlingSummaryV2,
    settled_penetration_max: f32,
    s9_summary_exact: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityLaneEvidence {
    lane: &'static str,
    warm_start: bool,
    artifact: String,
    replay_artifact: String,
    result_payload_hash: String,
    trace: ArtifactIdentity,
    exact_replay: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_loss_step: Option<u32>,
    summary: SettlingSummaryV2,
    settled_penetration_max: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoutingVerdict {
    scene: &'static str,
    warm_on_settled_linear_speed_max: f32,
    warm_off_settled_linear_speed_max: f32,
    speed_removed_fraction: f32,
    warm_on_last_energy_quartile_max: f32,
    warm_off_last_energy_quartile_max: f32,
    energy_removed_fraction: f32,
    warm_off_structure_preserved: bool,
    majority_removed: bool,
    favored_hypothesis: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineManifest {
    artifact_kind: String,
    source_revision: String,
    source_dirty: bool,
    observation_version: u32,
    policy: physbox_contracts::SettlingMeasurementPolicyV2,
    scenes: Vec<BaselineScene>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineScene {
    label: String,
    selected_product: BaselineLane,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineLane {
    summary: Option<SettlingSummaryV2>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s10-warm-start-differential error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let baseline_bytes = fs::read(&arguments.baseline).map_err(|error| error.to_string())?;
    let baseline: BaselineManifest =
        serde_json::from_slice(&baseline_bytes).map_err(|error| error.to_string())?;
    validate_baseline(&baseline)?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S10.1 differential requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("S10.1 output or staging directory already exists".to_owned());
    }
    for directory in ["sources", "traces", "oracles"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenes = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S10.1 differential {}", input.label);
        scenes.push(evaluate_scene(
            &arguments, &staging, &box2d, &runner, &baseline, input,
        )?);
    }
    let routing = routing_verdict(&scenes)?;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s10.1-warm-start-differential",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        policy: awake_settling_measurement_policy_v2(),
        stationarity_contract: stationarity_metric_contract_v1(),
        baseline: BaselineIdentity {
            artifact_kind: baseline.artifact_kind,
            source_revision: baseline.source_revision,
            manifest_path: arguments.baseline.to_string_lossy().replace('\\', "/"),
            manifest_sha256: sha256_bytes(&baseline_bytes),
        },
        scenes,
        routing,
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
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s10.1-warm-start-differential run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn validate_baseline(baseline: &BaselineManifest) -> Result<(), String> {
    if baseline.artifact_kind != "p5.2b-s9.1-baseline-repin"
        || baseline.source_dirty
        || baseline.observation_version != AWAKE_SETTLING_TRACE_VERSION
        || serialized_hash(&baseline.policy)?
            != serialized_hash(&awake_settling_measurement_policy_v2())?
    {
        return Err("S10.1 input is not the frozen clean S9.1 baseline".to_owned());
    }
    for label in [
        "centered-stack-4",
        "centered-stack-8",
        "centered-stack-16",
        "permanent-four-body-reproducer",
    ] {
        if baseline
            .scenes
            .iter()
            .find(|scene| scene.label == label)
            .and_then(|scene| scene.selected_product.summary.as_ref())
            .is_none()
        {
            return Err(format!("S9.1 baseline omits {label}"));
        }
    }
    Ok(())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
        });
    }
    inputs.push(SceneInput {
        label: "permanent-four-body-reproducer".to_owned(),
        kind: SceneKind::PermanentReproducer,
        source: p5_2_minimized_mixed_pile_hard_scenario().map_err(|error| error.to_string())?,
    });
    Ok(inputs)
}

fn evaluate_scene(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    baseline: &BaselineManifest,
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
    let pinned = baseline
        .scenes
        .iter()
        .find(|scene| scene.label == input.label)
        .and_then(|scene| scene.selected_product.summary.as_ref())
        .ok_or_else(|| format!("S9.1 baseline omits {}", input.label))?;
    let physbox_warm_on = evaluate_gpu_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "physbox-warm-on",
        true,
        Some(pinned),
    )?;
    let physbox_warm_off = evaluate_gpu_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "physbox-warm-off",
        false,
        None,
    )?;
    let authority_warm_on = evaluate_authority_lane(
        arguments,
        staging,
        box2d,
        &input,
        &projection,
        &projection_hash,
        "authority-warm-on",
        true,
    )?;
    let authority_warm_off = evaluate_authority_lane(
        arguments,
        staging,
        box2d,
        &input,
        &projection,
        &projection_hash,
        "authority-warm-off",
        false,
    )?;
    Ok(SceneEvidence {
        label: input.label,
        requested_steps: input.source.requested_steps,
        source_scenario_hash,
        projection_hash,
        source,
        physbox_warm_on,
        physbox_warm_off,
        authority_warm_on,
        authority_warm_off,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_gpu_lane(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: &SceneInput,
    projection_hash: &str,
    lane: &'static str,
    warm_start: bool,
    pinned: Option<&SettlingSummaryV2>,
) -> Result<GpuLaneEvidence, String> {
    let run_series = || {
        if warm_start {
            runner.run_selected_observation_series(&input.source, 1, P5_2_SETTLING_EVIDENCE_STEPS)
        } else {
            runner.run_warm_start_off_diagnostic_observation_series(
                &input.source,
                1,
                P5_2_SETTLING_EVIDENCE_STEPS,
            )
        }
    };
    let raw = run_series()?;
    let replay = run_series()?;
    let raw_series_hash = serialized_hash(&raw)?;
    let exact_replay = raw_series_hash == serialized_hash(&replay)?;
    let final_only = if warm_start {
        runner.run(&input.source, P5_2_SETTLING_EVIDENCE_STEPS)?
    } else {
        runner.run_warm_start_off_diagnostic(&input.source, P5_2_SETTLING_EVIDENCE_STEPS)?
    };
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "S10.1 GPU series is empty".to_owned())?;
    let instrumentation_final_exact = final_snapshot.bodies == final_only.bodies
        && final_snapshot.manifolds == final_only.manifolds
        && final_snapshot.cache == final_only.cache;
    let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean);
    let (structural_pass, structural_reason, first_structural_loss_step) =
        structure_history(input, &raw.snapshots);
    let observations = observe_generated_settling_series_v2(&input.source, &raw)?;
    let settled_penetration_max = observations
        .iter()
        .filter(|observation| (900..=1_200).contains(&observation.base.completed_steps))
        .map(|observation| observation.base.penetration_max.get())
        .fold(0.0_f32, f32::max);
    let series = settling_series_v2(
        &input.source,
        Some(projection_hash.to_owned()),
        lane,
        awake_settling_measurement_policy_v2(),
        observations,
        structural_pass,
        structural_reason.clone(),
    )?;
    let trace = write_gzip_json_artifact(
        staging,
        &format!("traces/{}-{lane}.json.gz", input.label),
        &series,
    )?;
    Ok(GpuLaneEvidence {
        lane,
        solver_execution: raw.solver_execution,
        scheduling_policy: raw.solver_scheduling_policy,
        raw_series_hash,
        trace,
        exact_replay,
        instrumentation_final_exact,
        capacity_clean,
        structural_pass,
        structural_reason,
        first_structural_loss_step,
        s9_summary_exact: pinned.map(|summary| summary == &series.summary),
        summary: series.summary,
        settled_penetration_max,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_authority_lane(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    input: &SceneInput,
    projection: &ScenarioV1,
    projection_hash: &str,
    lane: &'static str,
    warm_start: bool,
) -> Result<AuthorityLaneEvidence, String> {
    let first = run_authority(
        arguments,
        staging,
        box2d,
        &input.label,
        lane,
        projection,
        warm_start,
        false,
    )?;
    let replay = run_authority(
        arguments,
        staging,
        box2d,
        &input.label,
        lane,
        projection,
        warm_start,
        true,
    )?;
    let result_payload_hash = serialized_hash(&(
        &first.result.bodies,
        &first.result.manifolds,
        &first.result.traces,
        &first.result.metrics,
    ))?;
    let replay_hash = serialized_hash(&(
        &replay.result.bodies,
        &replay.result.manifolds,
        &replay.result.traces,
        &replay.result.metrics,
    ))?;
    let snapshots = decode_box2d_step_trace(projection, &first.result)?;
    let mut observer = SettlingObserverV2::new(awake_settling_measurement_policy_v2())?;
    let mut observations = Vec::with_capacity(snapshots.len());
    let mut first_structural_loss_step = None;
    let mut final_structure = (true, "structure is not gated".to_owned());
    for snapshot in &snapshots {
        let structure = structure(input.kind, projection, &snapshot.bodies, false);
        if !structure.0 && first_structural_loss_step.is_none() {
            first_structural_loss_step = Some(snapshot.completed_steps);
        }
        if snapshot.completed_steps == projection.requested_steps {
            final_structure = structure;
        }
        observations.push(observer.observe(
            projection,
            SettlingObservationInputV2 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
                capacities: &[],
                support_height: 0.0,
            },
        )?);
    }
    let structural_pass = first_structural_loss_step.is_none();
    let settled_penetration_max = observations
        .iter()
        .filter(|observation| (900..=1_200).contains(&observation.base.completed_steps))
        .map(|observation| observation.base.penetration_max.get())
        .fold(0.0_f32, f32::max);
    let series = settling_series_v2(
        projection,
        Some(projection_hash.to_owned()),
        lane,
        awake_settling_measurement_policy_v2(),
        observations,
        structural_pass,
        final_structure.1.clone(),
    )?;
    let trace = write_gzip_json_artifact(
        staging,
        &format!("traces/{}-{lane}.json.gz", input.label),
        &series,
    )?;
    Ok(AuthorityLaneEvidence {
        lane,
        warm_start,
        artifact: relative_directory(staging, &first.artifacts.directory)?,
        replay_artifact: relative_directory(staging, &replay.artifacts.directory)?,
        result_payload_hash,
        trace,
        exact_replay: replay_hash
            == serialized_hash(&(
                &first.result.bodies,
                &first.result.manifolds,
                &first.result.traces,
                &first.result.metrics,
            ))?,
        structural_pass,
        structural_reason: final_structure.1,
        first_structural_loss_step,
        summary: series.summary,
        settled_penetration_max,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    scene_label: &str,
    lane: &str,
    projection: &ScenarioV1,
    warm_start: bool,
    replay: bool,
) -> Result<physbox_lab::oracle_transport::OracleRun, String> {
    let request_hash = canonical_hash(projection).map_err(|error| error.to_string())?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!(
            "{}-{scene_label}-{lane}{}",
            arguments.run_id,
            if replay { "-replay" } else { "" }
        ),
        source_scenario_hash: request_hash,
        scenario: projection.clone(),
    };
    run_oracle(
        &OracleRunConfig {
            artifact_root: staging.join("oracles"),
            timeout: Duration::from_secs(120),
            source: box2d.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![
                    OsString::from("--lane"),
                    OsString::from("world-awake"),
                    OsString::from("--warm-start"),
                    OsString::from(if warm_start { "on" } else { "off" }),
                    OsString::from("--capture-step-trace"),
                    OsString::from("on"),
                ],
            },
        },
        &request,
    )
    .map_err(|error| error.to_string())
}

fn routing_verdict(scenes: &[SceneEvidence]) -> Result<RoutingVerdict, String> {
    if scenes.iter().any(|scene| {
        scene.physbox_warm_on.s9_summary_exact != Some(true)
            || !scene.physbox_warm_on.exact_replay
            || !scene.physbox_warm_off.exact_replay
            || !scene.authority_warm_on.exact_replay
            || !scene.authority_warm_off.exact_replay
            || !scene.physbox_warm_on.capacity_clean
            || !scene.physbox_warm_off.capacity_clean
    }) {
        return Err("S10.1 replay, capacity, or S9.1 reconciliation failed".to_owned());
    }
    let scene = scenes
        .iter()
        .find(|scene| scene.label == "centered-stack-16")
        .ok_or_else(|| "S10.1 omits centered-stack-16".to_owned())?;
    let speed_on = scene.physbox_warm_on.summary.settled_linear_speed_max.get();
    let speed_off = scene
        .physbox_warm_off
        .summary
        .settled_linear_speed_max
        .get();
    let energy_on = scene.physbox_warm_on.summary.last_energy_quartile_max.get();
    let energy_off = scene
        .physbox_warm_off
        .summary
        .last_energy_quartile_max
        .get();
    if speed_on <= 0.0 || energy_on <= 0.0 {
        return Err("S10.1 16-stack warm-on residual is not positive".to_owned());
    }
    let speed_removed_fraction = (speed_on - speed_off) / speed_on;
    let energy_removed_fraction = (energy_on - energy_off) / energy_on;
    let majority_removed = scene.physbox_warm_off.structural_pass
        && speed_removed_fraction > 0.5
        && energy_removed_fraction > 0.5;
    Ok(RoutingVerdict {
        scene: "centered-stack-16",
        warm_on_settled_linear_speed_max: speed_on,
        warm_off_settled_linear_speed_max: speed_off,
        speed_removed_fraction,
        warm_on_last_energy_quartile_max: energy_on,
        warm_off_last_energy_quartile_max: energy_off,
        energy_removed_fraction,
        warm_off_structure_preserved: scene.physbox_warm_off.structural_pass,
        majority_removed,
        favored_hypothesis: if majority_removed {
            "hypothesis-1-non-stationary-stored-impulses"
        } else {
            "hypothesis-2-stationary-lossy-composition"
        },
    })
}

fn structure_history(
    input: &SceneInput,
    snapshots: &[physbox_engine::GpuGeneratedContactSnapshotV1],
) -> (bool, String, Option<u32>) {
    let mut first_loss = None;
    let mut final_result = (true, "structure is not gated".to_owned());
    for snapshot in snapshots {
        let result = structure(
            input.kind,
            &input.source,
            &snapshot.bodies,
            !snapshot_capacity_clean(snapshot),
        );
        if !result.0 && first_loss.is_none() {
            first_loss = Some(snapshot.completed_steps);
        }
        if snapshot.completed_steps == input.source.requested_steps {
            final_result = result;
        }
    }
    (first_loss.is_none(), final_result.1, first_loss)
}

fn structure(
    kind: SceneKind,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    overflowed: bool,
) -> (bool, String) {
    match kind {
        SceneKind::Stack => centered_stack_structure(source, bodies, overflowed, 0.0),
        SceneKind::PermanentReproducer => settling_fixture_structure(
            P5_2SettlingFixtureKind::SmallMixedPile,
            source,
            bodies,
            overflowed,
            0.0,
        ),
    }
}

fn snapshot_capacity_clean(snapshot: &physbox_engine::GpuGeneratedContactSnapshotV1) -> bool {
    snapshot.failure.is_none()
        && snapshot
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| error.to_string())
}

fn relative_directory(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
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
    baseline: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut baseline = None;
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
                "--baseline" => baseline = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            baseline: baseline.ok_or_else(|| "missing --baseline".to_owned())?,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differential_catalog_and_named_execution_are_frozen() {
        let scenes = scene_inputs().unwrap();
        assert_eq!(scenes.len(), 4);
        assert_eq!(scenes[0].label, "centered-stack-4");
        assert_eq!(scenes[3].label, "permanent-four-body-reproducer");
        let mut expected = GpuSolverExecutionV4::SELECTED;
        expected.warm_start = false;
        assert_eq!(expected, GpuSolverExecutionV4::WARM_START_OFF_DIAGNOSTIC);
    }
}
