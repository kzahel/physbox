use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, OracleRequestV1, SCHEMA_VERSION, ScenarioV1, SettlingSeriesV2, ShapeV1,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactOutput, GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1,
    P5_2_SETTLING_EVIDENCE_STEPS, P5_2SettlingFixtureKind, p5_2_box2d_world_projection,
    p5_2_centered_rectangle_stack_scenario, p5_2_random_settling_fixture_scenario,
    p5_2_settling_fixture_scenario,
};
use physbox_lab::{
    failure_automation::minimize_dense_generated_failure,
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    settling::{
        AWAKE_SETTLING_TRACE_VERSION, SettlingObservationInputV2, SettlingObserverV2,
        awake_settling_measurement_policy_v2, centered_stack_structure, settling_fixture_structure,
        settling_series_v2,
    },
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const AUTHORITY_CHECKPOINTS: [u32; 7] = [1, 60, 180, 600, 900, 1_050, 1_200];
const PLAYBACK_STEPS: [u32; 4] = [1, 600, 900, 1_200];
const RANDOM_FIRST_SEED: u64 = 0x6100;
const RANDOM_SEED_COUNT: u32 = 14;
const MAX_MINIMIZED_FAILURES: usize = 2;

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    Fixture(P5_2SettlingFixtureKind),
}

impl SceneKind {
    fn name(self) -> String {
        match self {
            Self::Stack => "centered-stack".to_owned(),
            Self::Fixture(kind) => kind.name().to_owned(),
        }
    }
}

struct SceneInput {
    label: String,
    kind: SceneKind,
    seed: u64,
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
    scenes: Vec<SceneEvidence>,
    randomized: RandomizedEvidence,
    minimized_failures: Vec<MinimizedFailureEvidence>,
    playback: ArtifactIdentity,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
    candidate_results_present: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    kind: String,
    seed: u64,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    gpu_trace: ArtifactIdentity,
    gpu_raw_trace_hash: String,
    gpu_replay_exact: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    hard_gate_pass: bool,
    hard_gate_failures: Vec<String>,
    summary: physbox_contracts::SettlingSummaryV2,
    authority: Vec<AuthoritySampleEvidence>,
    authority_final_replay_exact: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoritySampleEvidence {
    completed_steps: u32,
    request_hash: String,
    result_payload_hash: String,
    artifact: String,
    dynamic_linear_speed_max: f32,
    dynamic_angular_edge_speed_max: f32,
    contact_points: u32,
}

struct AuthorityCapture {
    evidence: AuthoritySampleEvidence,
    bodies: Vec<BodyStateV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedEvidence {
    first_seed: u64,
    seed_count: u32,
    exact_replays: u32,
    capacity_clean_runs: u32,
    structural_passes: u32,
    observed_final_rest_failures: u32,
    cases: Vec<RandomizedCaseEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedCaseEvidence {
    seed: u64,
    kind: String,
    source_scenario_hash: String,
    result_hash: String,
    replay_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    final_linear_speed_max: f32,
    final_angular_edge_speed_max: f32,
    final_rest_failure: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MinimizedFailureEvidence {
    kind: String,
    original_seed: u64,
    attempts: u32,
    source_scenario_hash: String,
    result_hash: String,
    final_linear_speed_max: f32,
    final_angular_edge_speed_max: f32,
    artifact: ArtifactIdentity,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MinimizedFailureArtifact<'a> {
    schema_version: u32,
    kind: &'a str,
    original_seed: u64,
    attempts: u32,
    source_scenario_hash: &'a str,
    result_hash: &'a str,
    final_linear_speed_max: f32,
    final_angular_edge_speed_max: f32,
    scenario: &'a ScenarioV1,
    bodies: &'a [BodyStateV1],
}

struct PlaybackScene {
    label: String,
    source: ScenarioV1,
    gpu: Vec<(u32, Vec<BodyStateV1>)>,
    authority: Vec<(u32, Vec<BodyStateV1>)>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-settling-baseline-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S6.2 baseline evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    for directory in ["sources", "traces", "oracles", "failures"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let policy = awake_settling_measurement_policy_v2();
    let mut scenes = Vec::new();
    let mut playback = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S6.2 baseline {}", input.label);
        let (evidence, playback_scene) =
            evaluate_scene(&arguments, &staging, &box2d, &runner, input)?;
        scenes.push(evidence);
        playback.push(playback_scene);
    }

    let (randomized, minimized_failures) = randomized_evidence(&staging, &runner)?;
    let playback_identity = write_artifact(
        &staging,
        "playback.svg",
        render_playback(&playback)?.as_bytes(),
    )?;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s6.2-awake-settling-baseline",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        policy,
        scenes,
        randomized,
        minimized_failures,
        playback: playback_identity,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
        candidate_results_present: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s6.2-awake-settling-baseline run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [2, 4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            seed: 0,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
        });
    }
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            seed: 0x6002,
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
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
) -> Result<(SceneEvidence, PlaybackScene), String> {
    let source_hash = canonical_hash(&input.source).map_err(|error| error.to_string())?;
    let projection =
        p5_2_box2d_world_projection(&input.source).map_err(|error| error.to_string())?;
    let projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let source_identity = write_json_artifact(
        staging,
        &format!("sources/{}.json", input.label),
        &input.source,
    )?;

    let raw =
        runner.run_selected_observation_series(&input.source, 1, P5_2_SETTLING_EVIDENCE_STEPS)?;
    let replay =
        runner.run_selected_observation_series(&input.source, 1, P5_2_SETTLING_EVIDENCE_STEPS)?;
    let raw_hash = serialized_hash(&raw)?;
    let gpu_replay_exact = raw_hash == serialized_hash(&replay)?;
    let trace = measure_series(&input, &projection_hash, &raw)?;
    let trace_identity =
        write_json_artifact(staging, &format!("traces/{}-gpu.json", input.label), &trace)?;
    let final_only = runner.run(&input.source, P5_2_SETTLING_EVIDENCE_STEPS)?;
    let final_snapshot = raw
        .snapshots
        .last()
        .expect("validated series has a final sample");
    let instrumentation_final_exact = final_snapshot.bodies == final_only.bodies
        && final_snapshot.manifolds == final_only.manifolds
        && final_snapshot.cache == final_only.cache;
    let capacity_clean = raw.snapshots.iter().all(|snapshot| {
        snapshot.failure.is_none()
            && snapshot
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed)
    });
    let (structural_pass, structural_reason) = structure(
        input.kind,
        &input.source,
        &final_snapshot.bodies,
        !capacity_clean,
    );
    let mut hard_gate_failures = Vec::new();
    if !structural_pass {
        hard_gate_failures.push(structural_reason.clone());
    }
    if trace.summary.settled_linear_speed_max.get() > trace.policy.rest_linear_speed_limit.get() {
        hard_gate_failures.push("settled linear-speed maximum exceeded 0.025 m/s".to_owned());
    }
    if trace.summary.settled_angular_edge_speed_max.get()
        > trace.policy.rest_angular_edge_speed_limit.get()
    {
        hard_gate_failures.push("settled angular-edge-speed maximum exceeded 0.025 m/s".to_owned());
    }
    if trace.summary.stable_support_unexplained_churn != 0 {
        hard_gate_failures.push("stable support had unexplained identity/cache churn".to_owned());
    }
    if !trace.summary.energy_envelope_nonincreasing {
        hard_gate_failures.push("settled kinetic-energy envelope increased".to_owned());
    }
    if !gpu_replay_exact {
        hard_gate_failures.push("selected GPU trace did not replay exactly".to_owned());
    }
    if !instrumentation_final_exact {
        hard_gate_failures.push("instrumented and final-only execution differed".to_owned());
    }
    if !capacity_clean {
        hard_gate_failures.push("capacity failed during the observation series".to_owned());
    }

    let authority = authority_series(arguments, staging, box2d, &input.label, &projection)?;
    let authority_replay = run_authority(
        arguments,
        staging,
        box2d,
        &format!("{}-replay", input.label),
        &projection,
        P5_2_SETTLING_EVIDENCE_STEPS,
    )?;
    let authority_final_replay_exact = authority.last().is_some_and(|sample| {
        sample.evidence.result_payload_hash == authority_replay.evidence.result_payload_hash
    });
    if !authority_final_replay_exact {
        hard_gate_failures
            .push("Box2D authority final checkpoint did not replay exactly".to_owned());
    }

    let playback_scene = PlaybackScene {
        label: input.label.clone(),
        source: input.source.clone(),
        gpu: PLAYBACK_STEPS
            .iter()
            .map(|&step| {
                let snapshot = &raw.snapshots[step as usize - 1];
                (step, snapshot.bodies.clone())
            })
            .collect(),
        authority: authority
            .iter()
            .filter(|sample| PLAYBACK_STEPS.contains(&sample.evidence.completed_steps))
            .map(|sample| (sample.evidence.completed_steps, sample.bodies.clone()))
            .collect(),
    };
    let evidence = SceneEvidence {
        label: input.label,
        kind: input.kind.name(),
        seed: input.seed,
        source_scenario_hash: source_hash,
        projection_hash,
        source: source_identity,
        gpu_trace: trace_identity,
        gpu_raw_trace_hash: raw_hash,
        gpu_replay_exact,
        instrumentation_final_exact,
        capacity_clean,
        structural_pass,
        structural_reason,
        hard_gate_pass: hard_gate_failures.is_empty(),
        hard_gate_failures,
        summary: trace.summary,
        authority: authority
            .into_iter()
            .map(|sample| sample.evidence)
            .collect(),
        authority_final_replay_exact,
    };
    Ok((evidence, playback_scene))
}

fn measure_series(
    input: &SceneInput,
    projection_hash: &str,
    raw: &GpuGeneratedContactSeriesV1,
) -> Result<SettlingSeriesV2, String> {
    let policy = awake_settling_measurement_policy_v2();
    let mut observer = SettlingObserverV2::new(policy)?;
    let mut observations = Vec::with_capacity(raw.snapshots.len());
    for snapshot in &raw.snapshots {
        observations.push(observer.observe(
            &input.source,
            SettlingObservationInputV2 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
                capacities: &snapshot.capacities,
                support_height: 0.0,
            },
        )?);
    }
    let capacity_failed = raw
        .snapshots
        .iter()
        .any(|snapshot| snapshot.failure.is_some());
    let final_bodies = &raw.snapshots.last().expect("nonempty series").bodies;
    let (structural_pass, structural_reason) =
        structure(input.kind, &input.source, final_bodies, capacity_failed);
    settling_series_v2(
        &input.source,
        Some(projection_hash.to_owned()),
        "selected-gpu-baseline",
        policy,
        observations,
        structural_pass,
        structural_reason,
    )
}

fn authority_series(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
) -> Result<Vec<AuthorityCapture>, String> {
    AUTHORITY_CHECKPOINTS
        .iter()
        .map(|&step| run_authority(arguments, staging, box2d, label, projection, step))
        .collect()
}

fn run_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
    completed_steps: u32,
) -> Result<AuthorityCapture, String> {
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
    let payload_hash = serialized_hash(&(
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
    let (linear, angular) = body_speeds(&oracle.result.bodies);
    let contact_points = oracle
        .result
        .manifolds
        .iter()
        .map(|manifold| manifold.points.len() as u32)
        .sum();
    Ok(AuthorityCapture {
        evidence: AuthoritySampleEvidence {
            completed_steps,
            request_hash,
            result_payload_hash: payload_hash,
            artifact,
            dynamic_linear_speed_max: linear,
            dynamic_angular_edge_speed_max: angular,
            contact_points,
        },
        bodies: oracle.result.bodies,
    })
}

fn randomized_evidence(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
) -> Result<(RandomizedEvidence, Vec<MinimizedFailureEvidence>), String> {
    let mut cases = Vec::new();
    let mut failures = Vec::new();
    let mut minimized_kinds = BTreeSet::new();
    for offset in 0..RANDOM_SEED_COUNT {
        let seed = RANDOM_FIRST_SEED + u64::from(offset);
        let (kind, scenario) =
            p5_2_random_settling_fixture_scenario(seed, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?;
        eprintln!("S6.2 randomized {} seed {seed}", kind.name());
        let output = runner.run(&scenario, P5_2_SETTLING_EVIDENCE_STEPS)?;
        let replay = runner.run(&scenario, P5_2_SETTLING_EVIDENCE_STEPS)?;
        let result_hash = serialized_hash(&output)?;
        let replay_exact = result_hash == serialized_hash(&replay)?;
        let capacity_clean = output_capacity_clean(&output);
        let (structural_pass, _) =
            settling_fixture_structure(kind, &scenario, &output.bodies, !capacity_clean, 0.0);
        let (linear, angular) = body_speeds(&output.bodies);
        let final_rest_failure =
            !capacity_clean || !structural_pass || linear > 0.025 || angular > 0.025;
        cases.push(RandomizedCaseEvidence {
            seed,
            kind: kind.name().to_owned(),
            source_scenario_hash: canonical_hash(&scenario).map_err(|error| error.to_string())?,
            result_hash,
            replay_exact,
            capacity_clean,
            structural_pass,
            final_linear_speed_max: linear,
            final_angular_edge_speed_max: angular,
            final_rest_failure,
        });
        if final_rest_failure
            && failures.len() < MAX_MINIMIZED_FAILURES
            && minimized_kinds.insert(kind.name())
        {
            failures.push(minimize_observed_failure(
                staging, runner, kind, seed, scenario,
            )?);
        }
    }
    Ok((
        RandomizedEvidence {
            first_seed: RANDOM_FIRST_SEED,
            seed_count: RANDOM_SEED_COUNT,
            exact_replays: cases.iter().filter(|case| case.replay_exact).count() as u32,
            capacity_clean_runs: cases.iter().filter(|case| case.capacity_clean).count() as u32,
            structural_passes: cases.iter().filter(|case| case.structural_pass).count() as u32,
            observed_final_rest_failures: cases
                .iter()
                .filter(|case| case.final_rest_failure)
                .count() as u32,
            cases,
        },
        failures,
    ))
}

fn minimize_observed_failure(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    kind: P5_2SettlingFixtureKind,
    seed: u64,
    scenario: ScenarioV1,
) -> Result<MinimizedFailureEvidence, String> {
    let mut evaluation_error = None;
    let minimized = minimize_dense_generated_failure(scenario, |candidate| {
        if candidate.requested_steps < 900 {
            return false;
        }
        match runner.run(candidate, candidate.requested_steps) {
            Ok(output) => {
                let capacity_clean = output_capacity_clean(&output);
                let (structural_pass, _) = settling_fixture_structure(
                    kind,
                    candidate,
                    &output.bodies,
                    !capacity_clean,
                    0.0,
                );
                let (linear, angular) = body_speeds(&output.bodies);
                !capacity_clean || !structural_pass || linear > 0.025 || angular > 0.025
            }
            Err(error) => {
                evaluation_error.get_or_insert(error);
                false
            }
        }
    })?
    .ok_or_else(|| {
        "observed randomized failure did not reproduce during minimization".to_owned()
    })?;
    if let Some(error) = evaluation_error {
        return Err(format!("settling minimization execution failed: {error}"));
    }
    let output = runner.run(&minimized.scenario, minimized.scenario.requested_steps)?;
    let (linear, angular) = body_speeds(&output.bodies);
    let source_hash = canonical_hash(&minimized.scenario).map_err(|error| error.to_string())?;
    let result_hash = serialized_hash(&output)?;
    let relative = format!("failures/{}-seed-{seed}.json", kind.name());
    let artifact = write_json_artifact(
        staging,
        &relative,
        &MinimizedFailureArtifact {
            schema_version: SCHEMA_VERSION,
            kind: kind.name(),
            original_seed: seed,
            attempts: minimized.attempts,
            source_scenario_hash: &source_hash,
            result_hash: &result_hash,
            final_linear_speed_max: linear,
            final_angular_edge_speed_max: angular,
            scenario: &minimized.scenario,
            bodies: &output.bodies,
        },
    )?;
    Ok(MinimizedFailureEvidence {
        kind: kind.name().to_owned(),
        original_seed: seed,
        attempts: minimized.attempts,
        source_scenario_hash: source_hash,
        result_hash,
        final_linear_speed_max: linear,
        final_angular_edge_speed_max: angular,
        artifact,
    })
}

fn structure(
    kind: SceneKind,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    capacity_failed: bool,
) -> (bool, String) {
    match kind {
        SceneKind::Stack => centered_stack_structure(source, bodies, capacity_failed, 0.0),
        SceneKind::Fixture(kind) => {
            settling_fixture_structure(kind, source, bodies, capacity_failed, 0.0)
        }
    }
}

fn output_capacity_clean(output: &GpuGeneratedContactOutput) -> bool {
    output.failure.is_none()
        && output
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
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

fn render_playback(scenes: &[PlaybackScene]) -> Result<String, String> {
    let row_height = 185;
    let height = 70 + scenes.len() * row_height;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"{height}\" viewBox=\"0 0 1200 {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:11px;fill:#91a8ba}}.body{{fill:#54d6db;stroke:#d8ffff;stroke-width:1}}.oracle{{fill:#f0b45c;stroke:#fff0ca;stroke-width:1}}</style><text class=\"title\" x=\"18\" y=\"28\">S6.2 awake settling baseline / pinned Box2D</text><text class=\"meta\" x=\"18\" y=\"48\">same neutral source · sleeping off · selected GPU cyan · Box2D orange · steps 1 / 600 / 900 / 1200</text>"
    );
    for (scene_index, scene) in scenes.iter().enumerate() {
        let top = 65 + scene_index * row_height;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{}\">{}</text>",
            top + 16,
            scene.label
        ));
        for (column, &step) in PLAYBACK_STEPS.iter().enumerate() {
            let x = 180 + column * 250;
            svg.push_str(&format!(
                "<text class=\"meta\" x=\"{}\" y=\"{}\">step {step}</text><rect x=\"{}\" y=\"{}\" width=\"235\" height=\"145\" fill=\"#122739\" stroke=\"#29465b\"/>",
                x,
                top + 16,
                x,
                top + 24
            ));
            let gpu = scene
                .gpu
                .iter()
                .find(|sample| sample.0 == step)
                .ok_or_else(|| "GPU playback omitted a requested step".to_owned())?;
            let authority = scene
                .authority
                .iter()
                .find(|sample| sample.0 == step)
                .ok_or_else(|| "authority playback omitted a requested step".to_owned())?;
            draw_bodies(&mut svg, &scene.source, &gpu.1, x + 58, top + 138, "body")?;
            draw_bodies(
                &mut svg,
                &scene.source,
                &authority.1,
                x + 176,
                top + 138,
                "oracle",
            )?;
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

fn draw_bodies(
    svg: &mut String,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    origin_x: usize,
    origin_y: usize,
    class: &str,
) -> Result<(), String> {
    for body in bodies.iter().filter(|body| body.inverse_mass.get() > 0.0) {
        let source_body = source
            .bodies
            .iter()
            .find(|source| source.id == body.id)
            .ok_or_else(|| "playback body is absent from source".to_owned())?;
        let x = origin_x as f32 + body.position.x * 38.0;
        let y = origin_y as f32 - body.position.y * 38.0;
        match source_body.shape {
            ShapeV1::Circle { radius } => svg.push_str(&format!(
                "<circle class=\"{class}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\"/>",
                radius.get() * 38.0
            )),
            ShapeV1::Rectangle { half_extents } => svg.push_str(&format!(
                "<rect class=\"{class}\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" transform=\"rotate({:.3} {x:.2} {y:.2})\"/>",
                x - half_extents.x * 38.0,
                y - half_extents.y * 38.0,
                half_extents.x * 76.0,
                half_extents.y * 76.0,
                -body.angle.get().to_degrees()
            )),
        }
    }
    Ok(())
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

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    let path = root.join(relative);
    atomic_write(&path, bytes)?;
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
        return Err(format!(
            "refusing to replace existing artifact {}",
            path.display()
        ));
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
    fn baseline_catalog_and_policy_are_frozen_without_candidates() {
        let inputs = scene_inputs().unwrap();
        assert_eq!(inputs.len(), 11);
        assert_eq!(inputs[0].label, "centered-stack-2");
        assert_eq!(inputs[10].label, "small-mixed-pile-seed-6002");
        assert_eq!(AUTHORITY_CHECKPOINTS.last(), Some(&1_200));
        assert_eq!(
            awake_settling_measurement_policy_v2().settled_start_step,
            900
        );
    }
}
