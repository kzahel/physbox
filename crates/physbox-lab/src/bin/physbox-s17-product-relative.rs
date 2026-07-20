use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyStateV1, CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FiniteF32,
    SCHEMA_VERSION, ScenarioV1, SettlingObservationV2, SettlingSummaryV2, Validate, Vec2,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    BranchRobustGpuPromotionPolicyV1, GpuAuthoritySeparationInputV1, GpuGeneratedContactRunner,
    GpuSolverPassKindV1, P5_2_SETTLING_EVIDENCE_STEPS, P5_2BucketShape, P5_2GeneratedScenarioKind,
    P5_2SettlingFixtureKind, SelectedGpuSolverPolicySnapshotV1,
    benchmark_gpu_s17_branch_robust_promotion, branch_robust_gpu_promotion_capacity_disposition,
    p5_2_box2d_world_projection, p5_2_bucket_scenario, p5_2_centered_rectangle_stack_scenario,
    p5_2_generated_scenario, p5_2_minimized_mixed_pile_hard_scenario,
    p5_2_s11_soft_depth_sweep_scenario, p5_2_s13_authority_measurement_scenario,
    p5_2_s13_authority_overflow_scenario, p5_2_settling_fixture_scenario,
    run_gpu_authored_branch_robust_promotion,
};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePassKindV1, AuthorityStructurePolicyV1,
        solve_branch_robust_authority_structure_one_step_pass_states,
        solve_branch_robust_authority_structure_steps_observed,
    },
    branch_robust::{AuthoritySeparationInputsV1, evaluate_branch_robust_authority_separation_v1},
    branch_robust_scalar::BranchRobustPerturbationV1,
    settling::{
        AWAKE_SETTLING_TRACE_VERSION, awake_settling_measurement_policy_v2,
        centered_stack_structure, observe_generated_settling_series_v2, settling_fixture_structure,
        settling_series_v2,
    },
};
use serde::{Deserialize, Serialize};

const FULL_CHECKPOINTS: [u32; 7] = [1, 60, 180, 600, 900, 1_050, 1_200];
const BUCKET_CHECKPOINTS: [u32; 7] = [1, 60, 180, 300, 420, 540, 600];
const IMPLEMENTATION_BAND: f64 = 5.0e-5;
const GEOMETRY_MARGIN: f32 = 2.0e-4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Baseline,
    Candidate,
    Implementation,
    Benchmark,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "candidate" => Ok(Self::Candidate),
            "implementation" => Ok(Self::Implementation),
            "benchmark" => Ok(Self::Benchmark),
            _ => Err(format!("unknown S17 mode {value}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Candidate => "candidate",
            Self::Implementation => "implementation",
            Self::Benchmark => "benchmark",
        }
    }
}

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    schema_version: u32,
    observation_version: u32,
    artifact_kind: &'static str,
    mode: &'static str,
    run_id: String,
    host: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    selected_snapshot: SelectedGpuSolverPolicySnapshotV1,
    selected_snapshot_hash: String,
    candidate: BranchRobustGpuPromotionPolicyV1,
    candidate_hash: String,
    body_floor_shader_sha256: String,
    separation_diagnostic_shader_sha256: String,
    sleeping_changed: bool,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    authority_bands_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorpusManifest {
    #[serde(flatten)]
    provenance: Provenance,
    adapter: physbox_engine::AdapterDiagnostics,
    policy: physbox_contracts::SettlingMeasurementPolicyV2,
    lane: &'static str,
    scenes: Vec<SceneEvidence>,
    response: ResponseEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline: Option<ArtifactIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product_relative_gate: Option<ProductRelativeGate>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    kind: String,
    accepted: bool,
    requested_steps: u32,
    source_scenario_hash: String,
    execution_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    checkpoints: &'static [u32],
    lane: LaneEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_loss_reproducer: Option<FirstLossReproducer>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaneEvidence {
    lane: String,
    raw_series_hash: String,
    trace: ArtifactIdentity,
    exact_replay: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    finite: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_loss_step: Option<u32>,
    first_execution_failure_step: Option<u32>,
    summary: Option<SettlingSummaryV2>,
    physical_envelope: Option<PhysicalEnvelope>,
    scheduling_policy_digest: String,
    work_vector_digest: String,
    dispatches_per_step: u32,
    allocated_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalEnvelope {
    settled_center_drift_max: f32,
    settled_stack_height_min: f32,
    settled_stack_height_max: f32,
    final_stack_height: f32,
    settled_penetration_max: f32,
    settled_separation_max: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FirstLossReproducer {
    completed_steps: u32,
    scenario_hash: String,
    source: ArtifactIdentity,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseEvidence {
    lane: &'static str,
    cases: Vec<ResponseCase>,
    all_predicates_pass: bool,
    exact_replay: bool,
    capacity_clean: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseCase {
    label: &'static str,
    requested_steps: u32,
    result_hash: String,
    replay_hash: String,
    exact_replay: bool,
    capacity_clean: bool,
    predicate_pass: bool,
    bodies: Vec<BodyStateV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductRelativeGate {
    version: u32,
    baseline_source_revision: String,
    baseline_code_equivalent: bool,
    full_corpus_identity: bool,
    accepted_structure_clean: bool,
    replay_and_capacity_clean: bool,
    diagnostic_first_loss_nonregressing: bool,
    two_stack_material_improvements_including_16: bool,
    remaining_stack_regression_within_five_percent: bool,
    sixteen_stack_positive_run_nonregressing: bool,
    sixteen_stack_quartile_envelope_nonrising: bool,
    sixteen_stack_structure_preserved: bool,
    geometry_nonregressing: bool,
    response_pass: bool,
    passed: bool,
    stack_comparisons: Vec<StackComparison>,
    diagnostic_comparisons: Vec<DiagnosticComparison>,
    scene_comparisons: Vec<SceneComparison>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackComparison {
    label: String,
    baseline_settled_linear_speed_max: f32,
    candidate_settled_linear_speed_max: f32,
    linear_improvement_fraction: f32,
    baseline_last_energy_quartile_max: f32,
    candidate_last_energy_quartile_max: f32,
    energy_improvement_fraction: f32,
    improves_both_by_twenty_five_percent: bool,
    speed_regression_within_five_percent: bool,
    energy_regression_within_five_percent: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticComparison {
    label: String,
    baseline_first_structural_loss_step: Option<u32>,
    candidate_first_structural_loss_step: Option<u32>,
    nonregressing: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneComparison {
    label: String,
    accepted: bool,
    structure_nonregressing: bool,
    penetration_delta: Option<f32>,
    separation_delta: Option<f32>,
    center_drift_delta: Option<f32>,
    final_height_absolute_delta: Option<f32>,
    allowed_final_height_delta: Option<f32>,
    geometry_nonregressing: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineManifest {
    artifact_kind: String,
    mode: String,
    source_revision: String,
    source_dirty: bool,
    host: String,
    scenes: Vec<BaselineScene>,
    response: BaselineResponse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineScene {
    label: String,
    accepted: bool,
    requested_steps: u32,
    source_scenario_hash: String,
    lane: LaneEvidence,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineResponse {
    all_predicates_pass: bool,
    exact_replay: bool,
    capacity_clean: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImplementationManifest {
    #[serde(flatten)]
    provenance: Provenance,
    adapter: physbox_engine::AdapterDiagnostics,
    source_scenario_hash: String,
    exact_replay: bool,
    pass_trace_exact_replay: bool,
    instrumentation_final_exact: bool,
    scalar_maximum: StateDelta,
    implementation_band: f64,
    implementation_agreement: bool,
    pass_deltas: Vec<PhaseDelta>,
    first_phase_outside_band: Option<PhaseDelta>,
    candidate_policy_attested: bool,
    selected_snapshot_unchanged: bool,
    separation_diagnostic: physbox_engine::GpuAuthoritySeparationDiagnosticV1,
    separation_diagnostic_matches_rust: bool,
    overflow: OverflowEvidence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OverflowEvidence {
    source_scenario_hash: String,
    capacity_primary: physbox_engine::GpuAuthorityProbeCapacityDispositionV1,
    capacity_replay: physbox_engine::GpuAuthorityProbeCapacityDispositionV1,
    capacity_exact_replay: bool,
    one_below_demand_fail_closed: bool,
    authored: physbox_engine::GpuAuthoredAuthorityProbeOutputV1,
    overflow_color_used: bool,
    overflow_first: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateDelta {
    body_index: usize,
    field: &'static str,
    absolute_difference: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseDelta {
    substep: u32,
    gpu_pass: GpuSolverPassKindV1,
    scalar_pass: AuthorityStructurePassKindV1,
    maximum: StateDelta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkManifest {
    #[serde(flatten)]
    provenance: Provenance,
    adapter: physbox_engine::AdapterDiagnostics,
    rows: Vec<BenchmarkRow>,
    metal_gate_applicable: bool,
    metal_256_and_1024_within_five_percent: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkRow {
    manifold_count: u32,
    source_scenario_hash: String,
    selected_gpu_p50_seconds: f64,
    candidate_gpu_p50_seconds: f64,
    gpu_p50_delta_fraction: f64,
    selected_cpu_encode_submit_p50_seconds: f64,
    candidate_cpu_encode_submit_p50_seconds: f64,
    selected_dispatches_per_step: u32,
    candidate_dispatches_per_step: u32,
    selected_allocated_bytes: u64,
    candidate_allocated_bytes: u64,
    selected_replay_exact: bool,
    candidate_replay_exact: bool,
    raw: ArtifactIdentity,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s17-product-relative error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let provenance = provenance(&arguments)?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("S17 output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(&staging).map_err(display)?;
    let result = match arguments.mode {
        Mode::Baseline | Mode::Candidate => run_corpus(&arguments, &staging, provenance),
        Mode::Implementation => run_implementation(&staging, provenance),
        Mode::Benchmark => run_benchmark(&arguments, &staging, provenance),
    };
    result?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(display)?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest = fs::read(arguments.output.join("manifest.json")).map_err(display)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host={} scenario=s17-{} run_id={} manifest_sha256={} output={}",
        arguments.host,
        arguments.mode.name(),
        arguments.run_id,
        sha256_bytes(&manifest),
        arguments.output.display()
    );
    Ok(())
}

fn run_corpus(arguments: &Arguments, staging: &Path, provenance: Provenance) -> Result<(), String> {
    for directory in ["sources", "traces", "failures"] {
        fs::create_dir_all(staging.join(directory)).map_err(display)?;
    }
    let candidate = arguments.mode == Mode::Candidate;
    let baseline = if candidate {
        let path = arguments
            .baseline
            .as_ref()
            .ok_or_else(|| "S17 candidate mode requires --baseline".to_owned())?;
        let bytes = fs::read(path).map_err(display)?;
        let manifest: BaselineManifest = serde_json::from_slice(&bytes).map_err(display)?;
        validate_baseline(&manifest, &arguments.host)?;
        let code_equivalent =
            baseline_code_equivalent(&manifest.source_revision, &provenance.source_revision)?;
        if !code_equivalent {
            return Err("S17 baseline and candidate physics code differ".to_owned());
        }
        Some((
            manifest,
            ArtifactIdentity {
                path: path.to_string_lossy().replace('\\', "/"),
                sha256: sha256_bytes(&bytes),
            },
            code_equivalent,
        ))
    } else {
        if arguments.baseline.is_some() {
            return Err("S17 baseline mode does not accept --baseline".to_owned());
        }
        None
    };
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let inputs = scene_inputs()?;
    let mut scenes = Vec::with_capacity(inputs.len());
    for input in inputs {
        eprintln!(
            "S17 {} {} {}",
            arguments.host,
            if candidate { "candidate" } else { "baseline" },
            input.label
        );
        scenes.push(evaluate_scene(staging, &runner, input, candidate)?);
    }
    let response = response_evidence(&runner, candidate)?;
    let product_relative_gate = baseline
        .as_ref()
        .map(|(baseline, _, code_equivalent)| {
            product_relative_gate(baseline, *code_equivalent, &scenes, &response)
        })
        .transpose()?;
    let manifest = CorpusManifest {
        provenance,
        adapter: runner.diagnostics().clone(),
        policy: awake_settling_measurement_policy_v2(),
        lane: if candidate {
            "branch-robust-product-promotion-v1"
        } else {
            "current-selected-product"
        },
        scenes,
        response,
        baseline: baseline.map(|(_, identity, _)| identity),
        product_relative_gate,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)
}

fn evaluate_scene(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: SceneInput,
    candidate: bool,
) -> Result<SceneEvidence, String> {
    let mut scenario = input.source.clone();
    if candidate {
        scenario.settings.contact_softness = Some(candidate_softness());
        scenario.validate().map_err(display)?;
    }
    let source_scenario_hash = canonical_hash(&input.source).map_err(display)?;
    let execution_scenario_hash = canonical_hash(&scenario).map_err(display)?;
    let projection = p5_2_box2d_world_projection(&input.source).map_err(display)?;
    let projection_hash = canonical_hash(&projection).map_err(display)?;
    let source = write_json_artifact(staging, &format!("sources/{}.json", input.label), &scenario)?;
    let lane = evaluate_lane(
        staging,
        runner,
        &input,
        &scenario,
        &projection_hash,
        candidate,
    )?;
    let first_loss_reproducer = lane
        .first_structural_loss_step
        .map(|completed_steps| {
            let mut reproducer = scenario.clone();
            reproducer.requested_steps = completed_steps;
            let scenario_hash = canonical_hash(&reproducer).map_err(display)?;
            let source = write_json_artifact(
                staging,
                &format!("failures/{}-first-loss.json", input.label),
                &reproducer,
            )?;
            Ok::<_, String>(FirstLossReproducer {
                completed_steps,
                scenario_hash,
                source,
            })
        })
        .transpose()?;
    Ok(SceneEvidence {
        label: input.label,
        kind: input.kind.name().to_owned(),
        accepted: input.accepted,
        requested_steps: scenario.requested_steps,
        source_scenario_hash,
        execution_scenario_hash,
        projection_hash,
        source,
        checkpoints: input.checkpoints,
        lane,
        first_loss_reproducer,
    })
}

fn evaluate_lane(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: &SceneInput,
    scenario: &ScenarioV1,
    projection_hash: &str,
    candidate: bool,
) -> Result<LaneEvidence, String> {
    let run_series = |interval| {
        if candidate {
            runner.run_branch_robust_gpu_promotion_observation_series(
                scenario,
                interval,
                scenario.requested_steps,
            )
        } else {
            runner.run_selected_observation_series(scenario, interval, scenario.requested_steps)
        }
    };
    let raw = run_series(1)?;
    raw.validate()?;
    let replay = run_series(1)?;
    replay.validate()?;
    let raw_series_hash = serialized_hash(&raw)?;
    let exact_replay = raw_series_hash == serialized_hash(&replay)?;
    let final_only = run_series(scenario.requested_steps)?;
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "empty S17 series".to_owned())?;
    let separate_final = final_only
        .snapshots
        .last()
        .ok_or_else(|| "empty S17 final-only series".to_owned())?;
    let instrumentation_final_exact = final_snapshot.bodies == separate_final.bodies
        && final_snapshot.manifolds == separate_final.manifolds
        && final_snapshot.cache == separate_final.cache;
    let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean);
    let finite = raw
        .snapshots
        .iter()
        .all(|snapshot| snapshot.bodies.iter().all(|body| body.validate().is_ok()));
    let mut first_structural_loss_step = None;
    let mut final_structure = (true, "structure is not gated".to_owned());
    for snapshot in &raw.snapshots {
        let structure = structure(
            input.kind,
            scenario,
            &snapshot.bodies,
            !snapshot_capacity_clean(snapshot),
        );
        if !structure.0 && first_structural_loss_step.is_none() {
            first_structural_loss_step = Some(snapshot.completed_steps);
        }
        if snapshot.completed_steps == scenario.requested_steps {
            final_structure = structure;
        }
    }
    let lane = if candidate {
        "branch-robust-product-promotion-v1"
    } else {
        "current-selected-product"
    };
    let (summary, physical_envelope) = if scenario.requested_steps == P5_2_SETTLING_EVIDENCE_STEPS {
        let observations = observe_generated_settling_series_v2(scenario, &raw)?;
        let envelope = physical_envelope(&observations)?;
        let trace = settling_series_v2(
            scenario,
            Some(projection_hash.to_owned()),
            lane,
            awake_settling_measurement_policy_v2(),
            observations,
            final_structure.0,
            final_structure.1.clone(),
        )?;
        let summary = trace.summary.clone();
        write_gzip_json_artifact(
            staging,
            &format!("traces/{}-{lane}.json.gz", input.label),
            &trace,
        )?;
        (Some(summary), Some(envelope))
    } else {
        write_gzip_json_artifact(
            staging,
            &format!("traces/{}-{lane}.json.gz", input.label),
            &raw,
        )?;
        (None, None)
    };
    let trace = artifact_identity(staging, &format!("traces/{}-{lane}.json.gz", input.label))?;
    let work_vector_digest = serialized_hash(&(
        raw.solver_scheduling_policy,
        &final_snapshot.work,
        &final_snapshot.capacities,
    ))?;
    Ok(LaneEvidence {
        lane: lane.to_owned(),
        raw_series_hash,
        trace,
        exact_replay,
        instrumentation_final_exact,
        capacity_clean,
        finite,
        structural_pass: first_structural_loss_step.is_none(),
        structural_reason: final_structure.1,
        first_structural_loss_step,
        first_execution_failure_step: raw
            .snapshots
            .iter()
            .find_map(|snapshot| snapshot.failure.as_ref()?.first_step),
        summary,
        physical_envelope,
        scheduling_policy_digest: serialized_hash(&raw.solver_scheduling_policy)?,
        work_vector_digest,
        dispatches_per_step: raw.dispatches_per_step,
        allocated_bytes: raw.allocated_bytes,
    })
}

fn physical_envelope(observations: &[SettlingObservationV2]) -> Result<PhysicalEnvelope, String> {
    let settled = observations
        .iter()
        .filter(|observation| observation.base.completed_steps >= 900)
        .collect::<Vec<_>>();
    let final_observation = observations
        .last()
        .ok_or_else(|| "S17 settling observations are empty".to_owned())?;
    if settled.is_empty() {
        return Err("S17 settled window is empty".to_owned());
    }
    Ok(PhysicalEnvelope {
        settled_center_drift_max: settled
            .iter()
            .map(|observation| observation.base.center_drift_max.get())
            .fold(0.0, f32::max),
        settled_stack_height_min: settled
            .iter()
            .map(|observation| observation.base.stack_height.get())
            .fold(f32::INFINITY, f32::min),
        settled_stack_height_max: settled
            .iter()
            .map(|observation| observation.base.stack_height.get())
            .fold(0.0, f32::max),
        final_stack_height: final_observation.base.stack_height.get(),
        settled_penetration_max: settled
            .iter()
            .map(|observation| observation.base.penetration_max.get())
            .fold(0.0, f32::max),
        settled_separation_max: settled
            .iter()
            .map(|observation| observation.base.separation_max.get())
            .fold(0.0, f32::max),
    })
}

fn response_evidence(
    runner: &GpuGeneratedContactRunner,
    candidate: bool,
) -> Result<ResponseEvidence, String> {
    let inputs = [
        (
            "bilateral-impact",
            P5_2GeneratedScenarioKind::BilateralCircles,
            1,
        ),
        (
            "off-center",
            P5_2GeneratedScenarioKind::OffCenterRectangles,
            4,
        ),
        ("friction", P5_2GeneratedScenarioKind::FrictionSlide, 12),
        ("restitution", P5_2GeneratedScenarioKind::Restitution, 8),
        ("short-stack", P5_2GeneratedScenarioKind::SmallStack, 12),
    ];
    let mut cases = Vec::new();
    for (label, kind, steps) in inputs {
        let mut scenario = p5_2_generated_scenario(kind, steps).map_err(display)?;
        if candidate {
            scenario.settings.contact_softness = Some(candidate_softness());
        }
        let run = || {
            if candidate {
                runner.run_branch_robust_gpu_promotion_observation_series(&scenario, steps, steps)
            } else {
                runner.run_selected_observation_series(&scenario, steps, steps)
            }
        };
        let output = run()?;
        let replay = run()?;
        let snapshot = output.snapshots.last().ok_or("empty response output")?;
        let result_hash = serialized_hash(snapshot)?;
        let replay_hash = serialized_hash(replay.snapshots.last().ok_or("empty response replay")?)?;
        let capacity_clean = snapshot_capacity_clean(snapshot);
        let bodies = snapshot.bodies.clone();
        let predicate_pass = match label {
            "bilateral-impact" => {
                bodies[0].linear_velocity.x < 1.0 && bodies[1].linear_velocity.x > -1.0
            }
            "off-center" => bodies
                .iter()
                .any(|body| body.angular_velocity.get().abs() > 1.0e-3),
            "friction" => bodies[0].linear_velocity.x.abs() < 2.0,
            "restitution" => bodies[0].linear_velocity.y > 0.0,
            "short-stack" => bodies[1..]
                .windows(2)
                .all(|pair| pair[0].position.y < pair[1].position.y),
            _ => unreachable!(),
        };
        cases.push(ResponseCase {
            label,
            requested_steps: steps,
            exact_replay: result_hash == replay_hash,
            result_hash,
            replay_hash,
            capacity_clean,
            predicate_pass,
            bodies,
        });
    }
    Ok(ResponseEvidence {
        lane: if candidate {
            "branch-robust-product-promotion-v1"
        } else {
            "current-selected-product"
        },
        all_predicates_pass: cases.iter().all(|case| case.predicate_pass),
        exact_replay: cases.iter().all(|case| case.exact_replay),
        capacity_clean: cases.iter().all(|case| case.capacity_clean),
        cases,
    })
}

fn validate_baseline(baseline: &BaselineManifest, host: &str) -> Result<(), String> {
    if baseline.artifact_kind != "p5.2b-s17-selected-baseline"
        || baseline.mode != "baseline"
        || baseline.source_dirty
        || baseline.host != host
        || baseline.scenes.len() != 15
        || baseline
            .scenes
            .iter()
            .filter(|scene| scene.accepted)
            .count()
            != 13
        || !baseline.response.all_predicates_pass
        || !baseline.response.exact_replay
        || !baseline.response.capacity_clean
        || baseline.scenes.iter().any(|scene| {
            !scene.lane.exact_replay
                || !scene.lane.instrumentation_final_exact
                || !scene.lane.capacity_clean
                || !scene.lane.finite
                || (scene.accepted
                    && (!scene.lane.structural_pass
                        || (scene.requested_steps == P5_2_SETTLING_EVIDENCE_STEPS
                            && scene.lane.summary.is_none())))
        })
    {
        return Err("S17 baseline is not a complete clean same-host selected corpus".to_owned());
    }
    Ok(())
}

fn baseline_code_equivalent(
    baseline_revision: &str,
    candidate_revision: &str,
) -> Result<bool, String> {
    if baseline_revision == candidate_revision {
        return Ok(true);
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            baseline_revision,
            candidate_revision,
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "crates/physbox-contracts",
            "crates/physbox-engine",
            "crates/physbox-lab/src/settling.rs",
        ])
        .current_dir(repository)
        .status()
        .map_err(display)?;
    Ok(status.success())
}

fn product_relative_gate(
    baseline: &BaselineManifest,
    baseline_code_equivalent: bool,
    candidates: &[SceneEvidence],
    response: &ResponseEvidence,
) -> Result<ProductRelativeGate, String> {
    let baseline_scene = |label: &str| {
        baseline
            .scenes
            .iter()
            .find(|scene| scene.label == label)
            .ok_or_else(|| format!("baseline omits {label}"))
    };
    let candidate_scene = |label: &str| {
        candidates
            .iter()
            .find(|scene| scene.label == label)
            .ok_or_else(|| format!("candidate omits {label}"))
    };
    let full_corpus_identity = candidates.len() == 15
        && candidates.iter().all(|scene| {
            baseline_scene(&scene.label).is_ok_and(|pinned| {
                pinned.accepted == scene.accepted
                    && pinned.requested_steps == scene.requested_steps
                    && pinned.source_scenario_hash == scene.source_scenario_hash
            })
        });
    let accepted_structure_clean = candidates
        .iter()
        .filter(|scene| scene.accepted)
        .all(|scene| scene.lane.structural_pass && scene.lane.finite);
    let replay_and_capacity_clean = candidates.iter().all(|scene| {
        scene.lane.exact_replay
            && scene.lane.instrumentation_final_exact
            && scene.lane.capacity_clean
            && scene.lane.first_execution_failure_step.is_none()
    });
    let mut diagnostic_comparisons = Vec::new();
    for label in [
        "centered-stack-32-diagnostic",
        "centered-stack-64-diagnostic",
    ] {
        let pinned = baseline_scene(label)?.lane.first_structural_loss_step;
        let candidate = candidate_scene(label)?.lane.first_structural_loss_step;
        diagnostic_comparisons.push(DiagnosticComparison {
            label: label.to_owned(),
            baseline_first_structural_loss_step: pinned,
            candidate_first_structural_loss_step: candidate,
            nonregressing: match (pinned, candidate) {
                (None, None) => true,
                (None, Some(_)) => false,
                (Some(_), None) => true,
                (Some(left), Some(right)) => right >= left,
            },
        });
    }
    let diagnostic_first_loss_nonregressing =
        diagnostic_comparisons.iter().all(|row| row.nonregressing);
    let mut stack_comparisons = Vec::new();
    for label in ["centered-stack-4", "centered-stack-8", "centered-stack-16"] {
        let pinned = baseline_scene(label)?
            .lane
            .summary
            .as_ref()
            .ok_or_else(|| format!("baseline summary omitted for {label}"))?;
        let candidate = candidate_scene(label)?
            .lane
            .summary
            .as_ref()
            .ok_or_else(|| format!("candidate summary omitted for {label}"))?;
        let baseline_speed = pinned.settled_linear_speed_max.get();
        let candidate_speed = candidate.settled_linear_speed_max.get();
        let baseline_energy = pinned.last_energy_quartile_max.get();
        let candidate_energy = candidate.last_energy_quartile_max.get();
        let linear_improvement_fraction = improvement(baseline_speed, candidate_speed);
        let energy_improvement_fraction = improvement(baseline_energy, candidate_energy);
        stack_comparisons.push(StackComparison {
            label: label.to_owned(),
            baseline_settled_linear_speed_max: baseline_speed,
            candidate_settled_linear_speed_max: candidate_speed,
            linear_improvement_fraction,
            baseline_last_energy_quartile_max: baseline_energy,
            candidate_last_energy_quartile_max: candidate_energy,
            energy_improvement_fraction,
            improves_both_by_twenty_five_percent: linear_improvement_fraction >= 0.25
                && energy_improvement_fraction >= 0.25,
            speed_regression_within_five_percent: candidate_speed <= baseline_speed * 1.05,
            energy_regression_within_five_percent: candidate_energy <= baseline_energy * 1.05,
        });
    }
    let two_stack_material_improvements_including_16 = stack_comparisons
        .iter()
        .filter(|row| row.improves_both_by_twenty_five_percent)
        .count()
        >= 2
        && stack_comparisons.iter().any(|row| {
            row.label == "centered-stack-16" && row.improves_both_by_twenty_five_percent
        });
    let remaining_stack_regression_within_five_percent = stack_comparisons
        .iter()
        .filter(|row| !row.improves_both_by_twenty_five_percent)
        .all(|row| {
            row.speed_regression_within_five_percent && row.energy_regression_within_five_percent
        });
    let pinned_sixteen = baseline_scene("centered-stack-16")?
        .lane
        .summary
        .as_ref()
        .ok_or("baseline 16-stack summary omitted")?;
    let candidate_sixteen_scene = candidate_scene("centered-stack-16")?;
    let candidate_sixteen = candidate_sixteen_scene
        .lane
        .summary
        .as_ref()
        .ok_or("candidate 16-stack summary omitted")?;
    let sixteen_stack_positive_run_nonregressing =
        candidate_sixteen.positive_energy_run_max <= pinned_sixteen.positive_energy_run_max;
    let policy = awake_settling_measurement_policy_v2();
    let first_energy = candidate_sixteen.first_energy_quartile_max.get();
    let sixteen_stack_quartile_envelope_nonrising =
        candidate_sixteen.last_energy_quartile_max.get()
            <= first_energy
                + policy.energy_absolute_epsilon.get()
                + policy.energy_relative_epsilon.get() * first_energy.abs();
    let sixteen_stack_structure_preserved = candidate_sixteen_scene.lane.structural_pass;
    let mut scene_comparisons = Vec::new();
    for candidate in candidates {
        let pinned = baseline_scene(&candidate.label)?;
        let (
            penetration_delta,
            separation_delta,
            center_drift_delta,
            final_height_absolute_delta,
            allowed_final_height_delta,
            geometry_nonregressing,
        ) = match (
            &pinned.lane.physical_envelope,
            &candidate.lane.physical_envelope,
        ) {
            (Some(left), Some(right)) if candidate.accepted => {
                let height_delta = (right.final_stack_height - left.final_stack_height).abs();
                let allowed_height = GEOMETRY_MARGIN.max(left.settled_penetration_max);
                let penetration_delta =
                    right.settled_penetration_max - left.settled_penetration_max;
                let separation_delta = right.settled_separation_max - left.settled_separation_max;
                let drift_delta = right.settled_center_drift_max - left.settled_center_drift_max;
                (
                    Some(penetration_delta),
                    Some(separation_delta),
                    Some(drift_delta),
                    Some(height_delta),
                    Some(allowed_height),
                    penetration_delta <= GEOMETRY_MARGIN
                        && separation_delta <= GEOMETRY_MARGIN
                        && drift_delta <= GEOMETRY_MARGIN
                        && height_delta <= allowed_height,
                )
            }
            _ => (None, None, None, None, None, true),
        };
        scene_comparisons.push(SceneComparison {
            label: candidate.label.clone(),
            accepted: candidate.accepted,
            structure_nonregressing: !candidate.accepted || candidate.lane.structural_pass,
            penetration_delta,
            separation_delta,
            center_drift_delta,
            final_height_absolute_delta,
            allowed_final_height_delta,
            geometry_nonregressing,
        });
    }
    let geometry_nonregressing = scene_comparisons
        .iter()
        .filter(|row| row.accepted)
        .all(|row| row.geometry_nonregressing);
    let response_pass =
        response.all_predicates_pass && response.exact_replay && response.capacity_clean;
    let passed = full_corpus_identity
        && accepted_structure_clean
        && replay_and_capacity_clean
        && diagnostic_first_loss_nonregressing
        && two_stack_material_improvements_including_16
        && remaining_stack_regression_within_five_percent
        && sixteen_stack_positive_run_nonregressing
        && sixteen_stack_quartile_envelope_nonrising
        && sixteen_stack_structure_preserved
        && geometry_nonregressing
        && response_pass;
    Ok(ProductRelativeGate {
        version: 1,
        baseline_source_revision: baseline.source_revision.clone(),
        baseline_code_equivalent,
        full_corpus_identity,
        accepted_structure_clean,
        replay_and_capacity_clean,
        diagnostic_first_loss_nonregressing,
        two_stack_material_improvements_including_16,
        remaining_stack_regression_within_five_percent,
        sixteen_stack_positive_run_nonregressing,
        sixteen_stack_quartile_envelope_nonrising,
        sixteen_stack_structure_preserved,
        geometry_nonregressing,
        response_pass,
        passed,
        stack_comparisons,
        diagnostic_comparisons,
        scene_comparisons,
    })
}

fn improvement(baseline: f32, candidate: f32) -> f32 {
    if baseline == 0.0 {
        if candidate == 0.0 {
            0.0
        } else {
            f32::NEG_INFINITY
        }
    } else {
        (baseline - candidate) / baseline
    }
}

fn run_implementation(staging: &Path, provenance: Provenance) -> Result<(), String> {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut source = p5_2_s11_soft_depth_sweep_scenario(11).map_err(display)?;
    source.requested_steps = 2;
    let source_scenario_hash = canonical_hash(&source).map_err(display)?;
    let primary = runner.run_branch_robust_gpu_promotion_observation_series(&source, 1, 2)?;
    let replay = runner.run_branch_robust_gpu_promotion_observation_series(&source, 1, 2)?;
    let final_only = runner.run_branch_robust_gpu_promotion_observation_series(&source, 2, 2)?;
    let primary_bytes = serde_json::to_vec(&primary).map_err(display)?;
    let replay_bytes = serde_json::to_vec(&replay).map_err(display)?;
    let exact_replay = primary_bytes == replay_bytes;
    let final_snapshot = primary
        .snapshots
        .last()
        .ok_or("missing implementation snapshot")?;
    let separate_final = final_only
        .snapshots
        .last()
        .ok_or("missing final-only snapshot")?;
    let instrumentation_final_exact = final_snapshot.bodies == separate_final.bodies
        && final_snapshot.manifolds == separate_final.manifolds
        && final_snapshot.cache == separate_final.cache;
    let scalar = solve_branch_robust_authority_structure_steps_observed(
        &source,
        AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
        BranchRobustPerturbationV1::Nearest,
        false,
    )
    .map_err(display)?;
    let scalar_maximum = maximum_body_delta(&final_snapshot.bodies, &scalar.solve.result.bodies);
    let mut one_step = source.clone();
    one_step.requested_steps = 1;
    let pass_trace = runner.run_branch_robust_gpu_promotion_pass_trace(&one_step, 1)?;
    let pass_trace_replay = runner.run_branch_robust_gpu_promotion_pass_trace(&one_step, 1)?;
    let pass_trace_exact_replay =
        serialized_hash(&pass_trace)? == serialized_hash(&pass_trace_replay)?;
    let (_, scalar_pass_states) = solve_branch_robust_authority_structure_one_step_pass_states(
        &one_step,
        AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
    )
    .map_err(display)?;
    let pass_deltas = pass_trace
        .descriptors
        .iter()
        .enumerate()
        .filter_map(|(index, descriptor)| {
            pass_kind(descriptor.kind).map(|scalar_pass| (index, descriptor, scalar_pass))
        })
        .filter_map(|(index, descriptor, scalar_pass)| {
            scalar_pass_states
                .iter()
                .find(|state| state.substep == descriptor.substep && state.kind == scalar_pass)
                .map(|state| PhaseDelta {
                    substep: descriptor.substep,
                    gpu_pass: descriptor.kind,
                    scalar_pass,
                    maximum: maximum_body_delta(
                        &pass_trace.states[index + 1].bodies,
                        &state.bodies,
                    ),
                })
        })
        .collect::<Vec<_>>();
    let first_phase_outside_band = pass_deltas
        .iter()
        .find(|delta| delta.maximum.absolute_difference > IMPLEMENTATION_BAND)
        .cloned();
    let overflow_source = p5_2_s13_authority_overflow_scenario().map_err(display)?;
    let capacity_primary = branch_robust_gpu_promotion_capacity_disposition(&overflow_source, 20)?;
    let capacity_replay = branch_robust_gpu_promotion_capacity_disposition(&overflow_source, 20)?;
    let authored = pollster::block_on(run_gpu_authored_branch_robust_promotion(&overflow_source))?;
    let coloring = authored
        .pass_evidence
        .coloring
        .as_ref()
        .ok_or("S17 authored overflow omitted coloring")?;
    let overflow_color_used = coloring.point_colors.len() == 21
        && coloring.point_colors[..20] == (0..20).collect::<Vec<_>>()
        && coloring.point_colors[20] == 23
        && coloring.color_occupancy[23] == 1;
    let overflow_first = authored
        .pass_descriptors
        .iter()
        .filter(|descriptor| descriptor.color_passes != 0)
        .all(|descriptor| descriptor.parameters.uints1[2] == 2);
    let capacity_exact_replay =
        serialized_hash(&capacity_primary)? == serialized_hash(&capacity_replay)?;
    let one_below_demand_fail_closed = capacity_primary.rejected
        && capacity_primary.overflowed
        && capacity_primary.attempted == 21
        && capacity_primary.capacity == 20
        && capacity_primary.accepted == 0
        && capacity_primary.committed == 0;
    let candidate_policy_attested = primary
        .branch_robust_gpu_promotion_policy
        .is_some_and(|policy| policy == BranchRobustGpuPromotionPolicyV1::AUTHORIZED)
        && primary.authority_gpu_probe_policy.is_none();
    let selected_snapshot_unchanged = SelectedGpuSolverPolicySnapshotV1::current()
        == SelectedGpuSolverPolicySnapshotV1::FROZEN_AT_S13_AUTHORIZATION;
    let diagnostic_input = GpuAuthoritySeparationInputV1 {
        normal: Vec2::new(0.0, 1.0),
        anchor_a: Vec2::new(0.35, -0.125),
        anchor_b: Vec2::new(0.35, -0.125),
        pose_a: Vec2::new(0.0, 1.0),
        pose_b: Vec2::new(0.0, 1.25),
        initial_position_a: Vec2::new(0.0, 1.0),
        initial_position_b: Vec2::new(0.0, 1.25),
        angle_a: 1.0e-4,
        angle_b: -2.0e-4,
        initial_angle_a: 0.0,
        initial_angle_b: 0.0,
        input_separation: 0.0,
    };
    let rust_diagnostic =
        evaluate_branch_robust_authority_separation_v1(AuthoritySeparationInputsV1 {
            normal: diagnostic_input.normal,
            anchor_a: diagnostic_input.anchor_a,
            anchor_b: diagnostic_input.anchor_b,
            position_a: diagnostic_input.pose_a,
            position_b: diagnostic_input.pose_b,
            initial_position_a: diagnostic_input.initial_position_a,
            initial_position_b: diagnostic_input.initial_position_b,
            angle_a: diagnostic_input.angle_a,
            angle_b: diagnostic_input.angle_b,
            initial_angle_a: diagnostic_input.initial_angle_a,
            initial_angle_b: diagnostic_input.initial_angle_b,
            input_separation: diagnostic_input.input_separation,
        });
    let separation_diagnostic = runner.run_authority_separation_diagnostic(diagnostic_input)?;
    let separation_diagnostic_matches_rust = separation_diagnostic
        .values
        .iter()
        .find(|value| value.name == "native-robust-separation")
        .is_some_and(|value| value.bits == rust_diagnostic.result.separation_bits);
    atomic_json(
        &staging.join("manifest.json"),
        &ImplementationManifest {
            provenance,
            adapter: primary.adapter.clone(),
            source_scenario_hash,
            exact_replay,
            pass_trace_exact_replay,
            instrumentation_final_exact,
            scalar_maximum: scalar_maximum.clone(),
            implementation_band: IMPLEMENTATION_BAND,
            implementation_agreement: scalar_maximum.absolute_difference <= IMPLEMENTATION_BAND,
            pass_deltas,
            first_phase_outside_band,
            candidate_policy_attested,
            selected_snapshot_unchanged,
            separation_diagnostic,
            separation_diagnostic_matches_rust,
            overflow: OverflowEvidence {
                source_scenario_hash: canonical_hash(&overflow_source).map_err(display)?,
                capacity_primary,
                capacity_replay,
                capacity_exact_replay,
                one_below_demand_fail_closed,
                authored,
                overflow_color_used,
                overflow_first,
            },
        },
    )
}

fn run_benchmark(
    arguments: &Arguments,
    staging: &Path,
    provenance: Provenance,
) -> Result<(), String> {
    fs::create_dir_all(staging.join("raw")).map_err(display)?;
    let mut rows = Vec::new();
    let mut adapter = None;
    for manifold_count in [32, 256, 1_024] {
        eprintln!(
            "S17 {} benchmark {} manifolds",
            arguments.host, manifold_count
        );
        let source = p5_2_s13_authority_measurement_scenario(manifold_count).map_err(display)?;
        let source_scenario_hash = canonical_hash(&source).map_err(display)?;
        let raw = pollster::block_on(benchmark_gpu_s17_branch_robust_promotion(&source, 5, 20))?;
        if adapter.as_ref().is_some_and(|pinned| {
            serialized_hash(pinned).ok() != serialized_hash(&raw.adapter).ok()
        }) {
            return Err("S17 benchmark adapter changed within the matrix".to_owned());
        }
        adapter.get_or_insert_with(|| raw.adapter.clone());
        let raw_artifact =
            write_json_artifact(staging, &format!("raw/{manifold_count}.json"), &raw)?;
        let selected_gpu = median(&raw.selected.gpu_complete_step_seconds)?;
        let candidate_gpu = median(&raw.candidate.gpu_complete_step_seconds)?;
        rows.push(BenchmarkRow {
            manifold_count,
            source_scenario_hash,
            selected_gpu_p50_seconds: selected_gpu,
            candidate_gpu_p50_seconds: candidate_gpu,
            gpu_p50_delta_fraction: (candidate_gpu - selected_gpu) / selected_gpu,
            selected_cpu_encode_submit_p50_seconds: median(
                &raw.selected.cpu_encode_submit_seconds,
            )?,
            candidate_cpu_encode_submit_p50_seconds: median(
                &raw.candidate.cpu_encode_submit_seconds,
            )?,
            selected_dispatches_per_step: raw.selected.dispatches_per_step,
            candidate_dispatches_per_step: raw.candidate.dispatches_per_step,
            selected_allocated_bytes: raw.selected.allocated_bytes,
            candidate_allocated_bytes: raw.candidate.allocated_bytes,
            selected_replay_exact: raw.selected.replay_exact,
            candidate_replay_exact: raw.candidate.replay_exact,
            raw: raw_artifact,
        });
    }
    let metal_gate_applicable = arguments.host == "metal";
    let metal_256_and_1024_within_five_percent = metal_gate_applicable.then(|| {
        rows.iter()
            .filter(|row| matches!(row.manifold_count, 256 | 1_024))
            .all(|row| row.gpu_p50_delta_fraction <= 0.05)
    });
    atomic_json(
        &staging.join("manifest.json"),
        &BenchmarkManifest {
            provenance,
            adapter: adapter.ok_or("S17 benchmark produced no adapter")?,
            rows,
            metal_gate_applicable,
            metal_256_and_1024_within_five_percent,
        },
    )
}

fn median(values: &[f64]) -> Result<f64, String> {
    if values.len() != 20
        || values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("S17 benchmark requires 20 finite retained samples".to_owned());
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    Ok(values[9])
}

fn maximum_body_delta(left: &[BodyStateV1], right: &[BodyStateV1]) -> StateDelta {
    let mut maximum = StateDelta {
        body_index: 0,
        field: "none",
        absolute_difference: 0.0,
    };
    for (body_index, (left, right)) in left.iter().zip(right).enumerate() {
        for (field, absolute_difference) in [
            (
                "position-x",
                (f64::from(left.position.x) - f64::from(right.position.x)).abs(),
            ),
            (
                "position-y",
                (f64::from(left.position.y) - f64::from(right.position.y)).abs(),
            ),
            (
                "angle",
                (f64::from(left.angle.get()) - f64::from(right.angle.get())).abs(),
            ),
            (
                "linear-velocity-x",
                (f64::from(left.linear_velocity.x) - f64::from(right.linear_velocity.x)).abs(),
            ),
            (
                "linear-velocity-y",
                (f64::from(left.linear_velocity.y) - f64::from(right.linear_velocity.y)).abs(),
            ),
            (
                "angular-velocity",
                (f64::from(left.angular_velocity.get()) - f64::from(right.angular_velocity.get()))
                    .abs(),
            ),
        ] {
            if absolute_difference > maximum.absolute_difference {
                maximum = StateDelta {
                    body_index,
                    field,
                    absolute_difference,
                };
            }
        }
    }
    maximum
}

fn pass_kind(kind: GpuSolverPassKindV1) -> Option<AuthorityStructurePassKindV1> {
    match kind {
        GpuSolverPassKindV1::IntegrateVelocity => {
            Some(AuthorityStructurePassKindV1::IntegrateVelocity)
        }
        GpuSolverPassKindV1::WarmStart => Some(AuthorityStructurePassKindV1::WarmStart),
        GpuSolverPassKindV1::NormalBiased => Some(AuthorityStructurePassKindV1::BiasedSolve),
        GpuSolverPassKindV1::IntegratePosition => {
            Some(AuthorityStructurePassKindV1::IntegratePosition)
        }
        GpuSolverPassKindV1::RefreshContacts => Some(AuthorityStructurePassKindV1::Refresh),
        GpuSolverPassKindV1::NormalRelaxation => Some(AuthorityStructurePassKindV1::Relaxation),
        GpuSolverPassKindV1::Restitution => Some(AuthorityStructurePassKindV1::Restitution),
        _ => None,
    }
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            accepted: true,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display)?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    inputs.push(SceneInput {
        label: "permanent-four-body-reproducer".to_owned(),
        kind: SceneKind::Fixture(P5_2SettlingFixtureKind::SmallMixedPile),
        accepted: true,
        source: p5_2_minimized_mixed_pile_hard_scenario().map_err(display)?,
        checkpoints: &FULL_CHECKPOINTS,
    });
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            accepted: true,
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display)?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    for count in [32, 64] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}-diagnostic"),
            kind: SceneKind::Stack,
            accepted: false,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display)?,
            checkpoints: &FULL_CHECKPOINTS,
        });
    }
    for shape in P5_2BucketShape::ALL {
        inputs.push(SceneInput {
            label: format!("{}-bucket-100", shape.name()),
            kind: SceneKind::Bucket,
            accepted: true,
            source: p5_2_bucket_scenario(shape, 100, 600).map_err(display)?,
            checkpoints: &BUCKET_CHECKPOINTS,
        });
    }
    Ok(inputs)
}

fn candidate_softness() -> ContactSoftnessPolicyV1 {
    ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("finite S17 constant")
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

fn provenance(arguments: &Arguments) -> Result<Provenance, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (revision, dirty, patch_digest) = if let Some(revision) = &arguments.source_revision {
        (revision.clone(), arguments.source_dirty, None)
    } else {
        repository_state(&repository)?
    };
    if arguments.require_clean && dirty {
        return Err("S17 retained evidence requires a clean source tree".to_owned());
    }
    let selected_snapshot = SelectedGpuSolverPolicySnapshotV1::current();
    let candidate = BranchRobustGpuPromotionPolicyV1::AUTHORIZED;
    candidate.validate_product_selection(false)?;
    Ok(Provenance {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: match arguments.mode {
            Mode::Baseline => "p5.2b-s17-selected-baseline",
            Mode::Candidate => "p5.2b-s17-product-relative-candidate",
            Mode::Implementation => "p5.2b-s17-branch-robust-gpu-port",
            Mode::Benchmark => "p5.2b-s17-complete-step-timing",
        },
        mode: arguments.mode.name(),
        run_id: arguments.run_id.clone(),
        host: arguments.host.clone(),
        source_revision: revision,
        source_dirty: dirty,
        source_patch_digest: patch_digest,
        selected_snapshot_hash: serialized_hash(&selected_snapshot)?,
        selected_snapshot,
        candidate_hash: serialized_hash(&candidate)?,
        candidate,
        body_floor_shader_sha256: sha256_bytes(include_bytes!(
            "../../../physbox-engine/src/body_floor_solver.wgsl"
        )),
        separation_diagnostic_shader_sha256: sha256_bytes(include_bytes!(
            "../../../physbox-engine/src/authority_separation_diagnostic.wgsl"
        )),
        sleeping_changed: false,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        authority_bands_changed: false,
    })
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, &["rev-parse", "HEAD"])?;
    let status = command_output(
        repository,
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
            .map_err(display)?;
        if !output.status.success() {
            return Err("git diff failed while recording S17 provenance".to_owned());
        }
        Some(sha256_bytes(&output.stdout))
    } else {
        None
    };
    Ok((revision, dirty, patch_digest))
}

fn command_output(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(display)?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(display)
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display)
}

fn write_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display)?;
    bytes.push(b'\n');
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(display)?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder.write_all(&bytes).map_err(display)?;
    let compressed = encoder.finish().map_err(display)?;
    write_artifact(root, relative, &compressed)
}

fn artifact_identity(root: &Path, relative: &str) -> Result<ArtifactIdentity, String> {
    let bytes = fs::read(root.join(relative)).map_err(display)?;
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
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(display)?;
    }
    let staging = path.with_extension("partial");
    if path.exists() || staging.exists() {
        return Err(format!("refusing to replace artifact {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(display)?;
    file.write_all(bytes).map_err(display)?;
    file.sync_all().map_err(display)?;
    drop(file);
    fs::rename(staging, path).map_err(display)
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct Arguments {
    mode: Mode,
    run_id: String,
    output: PathBuf,
    host: String,
    baseline: Option<PathBuf>,
    require_clean: bool,
    source_revision: Option<String>,
    source_dirty: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut mode = None;
        let mut run_id = None;
        let mut output = None;
        let mut host = None;
        let mut baseline = None;
        let mut require_clean = false;
        let mut source_revision = None;
        let mut source_dirty = false;
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
                "--mode" => mode = Some(Mode::parse(&value)?),
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--host" => host = Some(value),
                "--baseline" => baseline = Some(PathBuf::from(value)),
                "--source-revision" => source_revision = Some(value),
                "--source-dirty" => source_dirty = value.parse::<bool>().map_err(display)?,
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            mode: mode.ok_or("missing --mode")?,
            run_id: run_id.ok_or("missing --run-id")?,
            output: output.ok_or("missing --output")?,
            host: host.ok_or("missing --host")?,
            baseline,
            require_clean,
            source_revision,
            source_dirty,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s17_corpus_is_the_frozen_s9_set() {
        let scenes = scene_inputs().unwrap();
        assert_eq!(scenes.len(), 15);
        assert_eq!(scenes.iter().filter(|scene| scene.accepted).count(), 13);
        assert_eq!(scenes.iter().filter(|scene| !scene.accepted).count(), 2);
        assert_eq!(
            scenes
                .iter()
                .find(|scene| scene.label == "centered-stack-16")
                .unwrap()
                .source
                .settings
                .contact_softness,
            None
        );
    }

    #[test]
    fn s17_candidate_tuple_and_product_snapshot_are_frozen() {
        BranchRobustGpuPromotionPolicyV1::AUTHORIZED
            .validate_product_selection(false)
            .unwrap();
        assert_eq!(
            SelectedGpuSolverPolicySnapshotV1::current(),
            SelectedGpuSolverPolicySnapshotV1::FROZEN_AT_S13_AUTHORIZATION
        );
        assert_eq!(candidate_softness().contact_hertz.get(), 30.0);
        assert_eq!(candidate_softness().damping_ratio.get(), 10.0);
    }
}
