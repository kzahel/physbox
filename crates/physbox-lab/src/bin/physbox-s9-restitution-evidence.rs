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
    SettlingSummaryV2, ShapeV1, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuContactCoefficientPolicyV1, GpuGeneratedContactRunner, GpuNormalRelaxationPolicyV1,
    GpuRestitutionTimingPolicyV1, GpuSoftFeedbackPolicyV1, GpuSolverExecutionV4,
    GpuSolverSchedulingPolicyV1, GpuSubstepContactPolicyV1, P5_2SettlingFixtureKind,
    p5_2_box2d_world_projection, p5_2_centered_rectangle_stack_scenario,
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
const COMPLETED_STEPS: u32 = 1_200;
const SUBMISSION_BATCH_STEPS: u32 = 64;
const AUTHORITY_CHECKPOINTS: [u32; 7] = [1, 60, 180, 600, 900, 1_050, 1_200];

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    Fixture(P5_2SettlingFixtureKind),
}

struct SceneInput {
    label: String,
    kind: SceneKind,
    source: ScenarioV1,
    authority: bool,
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
    selected_restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    candidate_restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    scenes: Vec<SceneEvidence>,
    resting_stack_byte_inert: bool,
    drop_authority_rows_fresh: bool,
    passed: bool,
    disposition: &'static str,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    capacity_policy_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    kind: &'static str,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    selected: LaneEvidence,
    candidate: LaneEvidence,
    snapshots_byte_exact: bool,
    final_state_byte_exact: bool,
    authority: Vec<AuthorityEvidence>,
    authority_final_replay_exact: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneEvidence {
    lane: &'static str,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    raw_series_hash: String,
    raw_series: ArtifactIdentity,
    trace: ArtifactIdentity,
    exact_replay: bool,
    final_only_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    summary: SettlingSummaryV2,
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
        eprintln!("physbox-s9-restitution-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S9 E6 evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("S9 E6 output or staging directory already exists".to_owned());
    }
    for directory in ["sources", "series", "traces", "oracles"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenes = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S9.4 restitution-after-substeps {}", input.label);
        scenes.push(evaluate_scene(
            &arguments, &staging, &box2d, &runner, input,
        )?);
    }
    let resting_stack_byte_inert = scenes
        .iter()
        .filter(|scene| scene.kind == "centered-stack")
        .all(|scene| scene.snapshots_byte_exact && scene.final_state_byte_exact);
    let drop_authority_rows_fresh = scenes
        .iter()
        .filter(|scene| scene.kind == "drop-fixture")
        .all(|scene| {
            scene.authority.len() == AUTHORITY_CHECKPOINTS.len()
                && scene.authority_final_replay_exact == Some(true)
        });
    let lanes_clean = scenes.iter().all(|scene| {
        scene.selected.exact_replay
            && scene.selected.final_only_exact
            && scene.selected.capacity_clean
            && scene.selected.structural_pass
            && scene.candidate.exact_replay
            && scene.candidate.final_only_exact
            && scene.candidate.capacity_clean
            && scene.candidate.structural_pass
    });
    let passed = resting_stack_byte_inert && drop_authority_rows_fresh && lanes_clean;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s9.4-restitution-hygiene",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        policy: awake_settling_measurement_policy_v2(),
        selected_restitution_timing_policy: GpuRestitutionTimingPolicyV1::SELECTED,
        candidate_restitution_timing_policy: GpuRestitutionTimingPolicyV1::AfterSubsteps,
        scenes,
        resting_stack_byte_inert,
        drop_authority_rows_fresh,
        passed,
        disposition: if passed {
            "hygiene-pass"
        } else {
            "hygiene-fail"
        },
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        capacity_policy_changed: false,
        oracle_precedence_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    if !passed {
        return Err("S9 E6 hygiene gate failed; evidence was published for review".to_owned());
    }
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s9.4-restitution-hygiene run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = [4, 8, 16]
        .into_iter()
        .map(|count| {
            Ok(SceneInput {
                label: format!("centered-stack-{count}"),
                kind: SceneKind::Stack,
                source: p5_2_centered_rectangle_stack_scenario(count, COMPLETED_STEPS)
                    .map_err(|error| error.to_string())?,
                authority: false,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for kind in [
        P5_2SettlingFixtureKind::NearFlatFloorDrop,
        P5_2SettlingFixtureKind::BoxOnRestingBoxDrop,
    ] {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            source: p5_2_settling_fixture_scenario(kind, 0x6002, COMPLETED_STEPS)
                .map_err(|error| error.to_string())?,
            authority: true,
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
    let selected_raw = runner.run_selected_observation_series(&input.source, 1, COMPLETED_STEPS)?;
    let scheduling_policy =
        GpuSolverSchedulingPolicyV1::selected_for_body_capacity(input.source.bodies.len() as u32);
    let candidate_raw = runner
        .run_contact_policy_observation_series_with_extended_solver_policies(
            &input.source,
            1,
            COMPLETED_STEPS,
            ContactIdentityPolicyV1::SELECTED,
            SUBMISSION_BATCH_STEPS,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
            GpuSoftFeedbackPolicyV1::SELECTED,
            GpuNormalRelaxationPolicyV1::SELECTED,
            GpuContactCoefficientPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::SELECTED,
            GpuRestitutionTimingPolicyV1::AfterSubsteps,
        )?;
    let snapshots_byte_exact = serde_json::to_vec(&selected_raw.snapshots)
        .map_err(|error| error.to_string())?
        == serde_json::to_vec(&candidate_raw.snapshots).map_err(|error| error.to_string())?;
    let selected = evaluate_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "selected-product",
        selected_raw,
        GpuRestitutionTimingPolicyV1::SELECTED,
    )?;
    let candidate = evaluate_lane(
        staging,
        runner,
        &input,
        &projection_hash,
        "restitution-after-substeps",
        candidate_raw,
        GpuRestitutionTimingPolicyV1::AfterSubsteps,
    )?;
    let selected_final = runner.run(&input.source, COMPLETED_STEPS)?;
    let candidate_final = runner.run_solver_policy_experiment(
        &input.source,
        COMPLETED_STEPS,
        GpuSolverExecutionV4::SELECTED,
        scheduling_policy,
        GpuContactCoefficientPolicyV1::SELECTED,
        GpuSubstepContactPolicyV1::SELECTED,
        GpuRestitutionTimingPolicyV1::AfterSubsteps,
    )?;
    let final_state_byte_exact = serde_json::to_vec(&(
        &selected_final.bodies,
        &selected_final.manifolds,
        &selected_final.cache,
        &selected_final.pass_evidence.contacts,
    ))
    .map_err(|error| error.to_string())?
        == serde_json::to_vec(&(
            &candidate_final.bodies,
            &candidate_final.manifolds,
            &candidate_final.cache,
            &candidate_final.pass_evidence.contacts,
        ))
        .map_err(|error| error.to_string())?;
    let authority = if input.authority {
        AUTHORITY_CHECKPOINTS
            .into_iter()
            .map(|step| run_authority(arguments, staging, box2d, &input.label, &projection, step))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let authority_final_replay_exact = if input.authority {
        let replay = run_authority(
            arguments,
            staging,
            box2d,
            &format!("{}-replay", input.label),
            &projection,
            COMPLETED_STEPS,
        )?;
        Some(
            authority
                .last()
                .is_some_and(|row| row.result_payload_hash == replay.result_payload_hash),
        )
    } else {
        None
    };
    Ok(SceneEvidence {
        label: input.label,
        kind: match input.kind {
            SceneKind::Stack => "centered-stack",
            SceneKind::Fixture(_) => "drop-fixture",
        },
        source_scenario_hash,
        projection_hash,
        source,
        selected,
        candidate,
        snapshots_byte_exact,
        final_state_byte_exact,
        authority,
        authority_final_replay_exact,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_lane(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: &SceneInput,
    projection_hash: &str,
    lane: &'static str,
    raw: physbox_engine::GpuGeneratedContactSeriesV1,
    restitution_timing_policy: GpuRestitutionTimingPolicyV1,
) -> Result<LaneEvidence, String> {
    let scheduling_policy = raw.solver_scheduling_policy;
    let replay = if restitution_timing_policy == GpuRestitutionTimingPolicyV1::SELECTED {
        runner.run_selected_observation_series(&input.source, 1, COMPLETED_STEPS)?
    } else {
        runner.run_contact_policy_observation_series_with_extended_solver_policies(
            &input.source,
            1,
            COMPLETED_STEPS,
            ContactIdentityPolicyV1::SELECTED,
            SUBMISSION_BATCH_STEPS,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
            GpuSoftFeedbackPolicyV1::SELECTED,
            GpuNormalRelaxationPolicyV1::SELECTED,
            GpuContactCoefficientPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::SELECTED,
            restitution_timing_policy,
        )?
    };
    let raw_series_hash = serialized_hash(&raw)?;
    let exact_replay = raw_series_hash == serialized_hash(&replay)?;
    let final_only = if restitution_timing_policy == GpuRestitutionTimingPolicyV1::SELECTED {
        runner.run(&input.source, COMPLETED_STEPS)?
    } else {
        runner.run_solver_policy_experiment(
            &input.source,
            COMPLETED_STEPS,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
            GpuContactCoefficientPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::SELECTED,
            restitution_timing_policy,
        )?
    };
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "S9 E6 lane omitted its final snapshot".to_owned())?;
    let final_only_exact = final_snapshot.bodies == final_only.bodies
        && final_snapshot.manifolds == final_only.manifolds
        && final_snapshot.cache == final_only.cache;
    let capacity_clean = raw.snapshots.iter().all(|snapshot| {
        snapshot.failure.is_none()
            && snapshot
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed)
    });
    let (structural_pass, structural_reason) = match input.kind {
        SceneKind::Stack => {
            centered_stack_structure(&input.source, &final_snapshot.bodies, !capacity_clean, 0.0)
        }
        SceneKind::Fixture(kind) => settling_fixture_structure(
            kind,
            &input.source,
            &final_snapshot.bodies,
            !capacity_clean,
            0.0,
        ),
    };
    let observations = observe_generated_settling_series_v2(&input.source, &raw)?;
    let trace = settling_series_v2(
        &input.source,
        Some(projection_hash.to_owned()),
        lane,
        awake_settling_measurement_policy_v2(),
        observations,
        structural_pass,
        structural_reason.clone(),
    )?;
    let raw_series = write_gzip_json_artifact(
        staging,
        &format!("series/{}-{lane}.json.gz", input.label),
        &raw,
    )?;
    let trace_artifact = write_gzip_json_artifact(
        staging,
        &format!("traces/{}-{lane}.json.gz", input.label),
        &trace,
    )?;
    Ok(LaneEvidence {
        lane,
        scheduling_policy,
        restitution_timing_policy,
        dispatches_per_step: raw.dispatches_per_step,
        allocated_bytes: raw.allocated_bytes,
        raw_series_hash,
        raw_series,
        trace: trace_artifact,
        exact_replay,
        final_only_exact,
        capacity_clean,
        structural_pass,
        structural_reason,
        summary: trace.summary,
    })
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
    fn e6_catalog_is_exactly_three_stacks_and_two_drop_fixtures() {
        let inputs = scene_inputs().unwrap();
        assert_eq!(inputs.len(), 5);
        assert_eq!(inputs.iter().filter(|input| input.authority).count(), 2);
        assert_eq!(
            inputs
                .iter()
                .filter(|input| matches!(input.kind, SceneKind::Stack))
                .count(),
            3
        );
        assert!(inputs.iter().all(|input| {
            input.source.requested_steps == COMPLETED_STEPS
                && input.source.settings.contact_softness.is_none()
                && input
                    .source
                    .bodies
                    .iter()
                    .all(|body| body.material.restitution.get() == 0.0)
        }));
    }
}
