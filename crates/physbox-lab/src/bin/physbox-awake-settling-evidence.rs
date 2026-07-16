use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyId, BodyStateV1, FailureV1, OracleRequestV1, OracleResultV1, SCHEMA_VERSION, ScenarioV1,
    SettlingObservationV1, ShapeV1, SolverSettingsV1, Validate, canonical_hash, canonical_json,
    sha256_bytes,
};
use physbox_engine::{
    GpuColoringEvidenceV4, GpuGeneratedContactBodyHistoryV1, GpuGeneratedContactOutput,
    GpuGeneratedContactRunner, GpuNormalSolverV4, GpuSolverExecutionV4,
    GpuSolverInstrumentedSampleV4, GpuSolverSchedulingPolicyV1, P5_2BucketShape,
    P5_2GeneratedScenarioKind, p5_2_box2d_world_projection, p5_2_bucket_scenario,
    p5_2_centered_rectangle_stack_scenario, p5_2_coloring_churn_snapshots,
    p5_2_generated_performance_scenario, p5_2_generated_scenario, p5_2_rectangle_pyramid_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    parallel_normal::{
        FrozenNormalDiagnosticV1, ParallelNormalModeV1, frozen_rectangle_stack_diagnostic,
    },
    settling::{
        AWAKE_SETTLING_METRICS_VERSION, AwakeFirstDivergenceV1, AwakeSettlingWindowV1,
        BodyAuthorityDeltaV1, ContactBalanceObservationV1, SettlingSnapshotInput,
        centered_stack_structure, compare_awake_authority, first_awake_divergence,
        observe_contact_balance, observe_settling, summarize_awake_window,
    },
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const CHECKPOINTS: [u32; 11] = [1, 60, 180, 360, 600, 900, 960, 1_020, 1_080, 1_140, 1_200];
const BUCKET_CHECKPOINTS: [u32; 7] = [1, 60, 180, 300, 420, 540, 600];
const PLAYBACK_STEPS: [u32; 4] = [1, 180, 600, 1_200];
const SETTLED_WINDOW_START: u32 = 900;
const SETTLED_WINDOW_END: u32 = 1_200;
const FEASIBILITY_BOUNDARY_SECONDS: f64 = 1.0 / 60.0;
const EXISTING_S5_4_SCENES: [&str; 6] = [
    "rectangle-stack-2",
    "rectangle-stack-4",
    "rectangle-stack-8",
    "rectangle-pyramid-2",
    "rectangle-pyramid-3",
    "rectangle-pyramid-4",
];

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    Pyramid,
    Bucket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CandidateFamilyV3 {
    PointJacobi,
    ScalarMassSplit,
    ManifoldBlock,
    BlockMassSplit,
    ColoredInPlace,
    CompactPointControl,
    ManifoldColorForward,
    ManifoldColorAlternating,
}

impl CandidateFamilyV3 {
    const fn is_precolored_experiment(self) -> bool {
        matches!(
            self,
            Self::ScalarMassSplit | Self::ManifoldBlock | Self::BlockMassSplit
        )
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateDefinition {
    id: &'static str,
    family: CandidateFamilyV3,
    iterations: u32,
    settings: SolverSettingsV1,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    question: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    metrics_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    candidates: Vec<CandidateDefinition>,
    frozen_normal_diagnostics: Vec<FrozenNormalDiagnosticV1>,
    scenes: Vec<SceneEvidence>,
    failures: Vec<FailureRecord>,
    gpu_performance: Vec<GpuPerformanceRow>,
    optimized_box2d_performance: Vec<OptimizedBox2dRow>,
    convergence: ConvergenceDecision,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
    playback: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColoredManifest {
    schema_version: u32,
    metrics_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    candidates: Vec<CandidateDefinition>,
    scheduling_control: SchedulingControlEvidence,
    accepted_scenes: Vec<SceneEvidence>,
    diagnostic_scenes: Vec<SceneEvidence>,
    churn: ChurnEvidence,
    failures: Vec<FailureRecord>,
    gpu_performance: Vec<GpuPerformanceRow>,
    optimized_box2d_performance: Vec<OptimizedBox2dRow>,
    decision: ColoredDecision,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
    selected_product_execution: GpuSolverExecutionV4,
    playback: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulingManifest {
    schema_version: u32,
    metrics_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    candidates: Vec<CandidateDefinition>,
    accepted_scenes: Vec<SceneEvidence>,
    diagnostic_scenes: Vec<SceneEvidence>,
    churn: ChurnEvidence,
    failures: Vec<FailureRecord>,
    gpu_performance: Vec<GpuPerformanceRow>,
    optimized_box2d_performance: Vec<OptimizedBox2dRow>,
    decision: SchedulingDecision,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
    selected_product_execution: GpuSolverExecutionV4,
    playback: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulingControlEvidence {
    scenario_hash: String,
    completed_steps: u32,
    iterations: u32,
    byte_exact: bool,
    fixed_result_hash: String,
    compact_result_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChurnEvidence {
    version: u32,
    full_rebuild_each_snapshot: bool,
    incremental_maintenance_included: bool,
    fresh_graph_snapshots: bool,
    snapshot_interpretation: &'static str,
    snapshots: Vec<ChurnSnapshotEvidence>,
    temporal_probes: Vec<TemporalChurnEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TemporalChurnEvidence {
    candidate: &'static str,
    stable_first_step: GpuColoringEvidenceV4,
    stable_eighth_step: GpuColoringEvidenceV4,
    removal_third_step: GpuColoringEvidenceV4,
    exact_replay: bool,
    capacity_clean: bool,
    stable_first_result_hash: String,
    stable_eighth_result_hash: String,
    removal_result_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChurnSnapshotEvidence {
    label: String,
    constructed: Vec<BodyId>,
    erased: Vec<BodyId>,
    source_scenario_hash: String,
    candidates: Vec<ChurnCandidateEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChurnCandidateEvidence {
    candidate: &'static str,
    exact_replay: bool,
    capacity_clean: bool,
    rebuilt_color_units: u32,
    surviving_color_units: u32,
    surviving_color_changes: u32,
    result_hash: String,
    coloring: GpuColoringEvidenceV4,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColoredDecision {
    version: u32,
    feasibility_boundary_seconds: f64,
    accepted_scene_count: u32,
    diagnostic_scene_count: u32,
    candidates: Vec<ColoredCandidateDecision>,
    potential_candidates: Vec<&'static str>,
    incremental_coloring_triggered: bool,
    incremental_coloring_reason: String,
    workgroup_per_island_triggered: bool,
    workgroup_per_island_reason: String,
    production_selection_deferred: bool,
    recommendation_deferred_to_human_review: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColoredCandidateDecision {
    candidate: &'static str,
    family: CandidateFamilyV3,
    iterations: u32,
    accepted_structural_passes: u32,
    accepted_exact_and_capacity_clean: bool,
    diagnostic_structural_passes: u32,
    one_k_complete_step_seconds: Option<f64>,
    one_k_below_feasibility_boundary: bool,
    one_k_color_construction_seconds: Option<f64>,
    one_k_color_compaction_seconds: Option<f64>,
    one_k_active_color_solve_seconds: Option<f64>,
    one_k_empty_color_solve_seconds: Option<f64>,
    potential_candidate: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulingDecision {
    version: u32,
    feasibility_boundary_seconds: f64,
    accepted_scene_count: u32,
    diagnostic_scene_count: u32,
    candidates: Vec<SchedulingCandidateDecision>,
    qualified_candidates: Vec<&'static str>,
    recommended_for_human_review: Option<&'static str>,
    recommendation_reason: String,
    production_selection_deferred: bool,
    quality_band_acceptance_deferred: bool,
    reconsideration_triggers: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulingCandidateDecision {
    candidate: &'static str,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    accepted_structural_passes: u32,
    accepted_exact_and_capacity_clean: bool,
    diagnostic_structural_passes: u32,
    churn_clean: bool,
    one_k_complete_step_seconds: Option<f64>,
    one_k_below_feasibility_boundary: bool,
    one_k_dispatches_per_step: u32,
    one_k_allocated_bytes: u64,
    one_k_color_construction_seconds: Option<f64>,
    one_k_color_compaction_seconds: Option<f64>,
    one_k_active_color_solve_seconds: Option<f64>,
    one_k_empty_color_solve_seconds: Option<f64>,
    fastest_box2d_ratio: Option<f64>,
    qualified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConvergenceDecision {
    version: u32,
    feasibility_boundary_seconds: f64,
    required_existing_scenes: [&'static str; 6],
    monotonic_violations: Vec<MonotonicViolation>,
    candidates: Vec<CandidateEligibility>,
    colored_comparator_triggered: bool,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MonotonicViolation {
    family: CandidateFamilyV3,
    scene: String,
    lower_candidate: &'static str,
    higher_candidate: &'static str,
    dimension: &'static str,
    lower_value: f64,
    higher_value: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEligibility {
    candidate: &'static str,
    family: CandidateFamilyV3,
    iterations: u32,
    exact_and_capacity_clean: bool,
    preserves_existing_s5_4_corpus: bool,
    family_is_monotonic: bool,
    one_k_complete_step_seconds: Option<f64>,
    one_k_below_feasibility_boundary: bool,
    satisfies_conditional_exit: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    source_scenario_hash: String,
    neutral_projection_hash: String,
    source: ScenarioV1,
    authority: Vec<AuthoritySample>,
    authority_exact_replay: bool,
    candidates: Vec<CandidateSceneEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoritySample {
    completed_steps: u32,
    request_hash: String,
    artifact: String,
    result: OracleResultV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateSceneEvidence {
    candidate: &'static str,
    execution_scenario_hash: String,
    exact_replay: bool,
    structural_pass: bool,
    structural_reason: String,
    preserves_structure_through_run: bool,
    first_structural_failure_step: Option<u32>,
    first_execution_failure: Option<FailureV1>,
    window: AwakeSettlingWindowV1,
    first_authority_divergence: Option<AwakeFirstDivergenceV1>,
    samples: Vec<CandidateSample>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateSample {
    completed_steps: u32,
    observation: SettlingObservationV1,
    contact_balance: ContactBalanceObservationV1,
    authority_delta: BodyAuthorityDeltaV1,
    bodies: Vec<BodyStateV1>,
    contact_cache: Vec<physbox_contracts::PersistentContactV1>,
    pass_evidence: physbox_engine::GpuFixedFloorPassEvidence,
    work: physbox_contracts::CollisionWorkV1,
    capacities: Vec<physbox_contracts::CollisionCapacityV1>,
    contact_points: u32,
    cache_points: u32,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    capacity_clean: bool,
    result_hash: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureRecord {
    candidate: &'static str,
    scene: String,
    minimized_scene: String,
    first_failure_step: u32,
    reason: String,
    reproducer: ScenarioV1,
    scenario_hash: String,
    result_hash: String,
    bodies: Vec<BodyStateV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuPerformanceRow {
    candidate: &'static str,
    bodies: u32,
    source_scenario_hash: String,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    collision_persistence_allocated_bytes: u64,
    gpu_end_to_end_seconds: Option<Vec<f64>>,
    gpu_end_to_end_p50_seconds: Option<f64>,
    host_completion_seconds: Vec<f64>,
    cpu_encode_submit_seconds: Vec<f64>,
    instrumented_solver_phases: Option<Vec<GpuSolverInstrumentedSampleV4>>,
    coloring: Option<GpuColoringEvidenceV4>,
    capacity_clean: bool,
    correctness_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptimizedBox2dRow {
    bodies: u32,
    workers: u32,
    artifact: String,
    samples_seconds: Vec<f64>,
    p50_seconds: f64,
    scope: String,
    source_revision: String,
    source_dirty: bool,
    contact_count_min: u64,
    contact_count_max: u64,
    success_sentinel: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformArtifact {
    label: String,
    path: String,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-awake-settling-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    if arguments.scheduling_factorial {
        return run_scheduling_factorial(arguments);
    }
    if arguments.colored_factorial {
        return run_colored_factorial(arguments);
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("awake settling evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("oracles")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("failures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("performance")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let base = p5_2_centered_rectangle_stack_scenario(2, 1)
        .map_err(|error| error.to_string())?
        .settings;
    let candidates = candidate_definitions(base);
    let frozen_normal_diagnostics = frozen_normal_diagnostics()?;
    let mut scene_inputs = Vec::new();
    for count in [2, 4, 8, 16] {
        scene_inputs.push((
            format!("rectangle-stack-{count}"),
            SceneKind::Stack,
            p5_2_centered_rectangle_stack_scenario(count, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
        ));
    }
    for rows in [2, 3, 4] {
        scene_inputs.push((
            format!("rectangle-pyramid-{rows}"),
            SceneKind::Pyramid,
            p5_2_rectangle_pyramid_scenario(rows, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
        ));
    }

    let mut scenes = Vec::new();
    let mut failures = Vec::new();
    for (label, kind, source) in scene_inputs {
        eprintln!("S5.4a scene {label}");
        let scene = evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            kind,
            source,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?;
        for candidate in &scene.candidates {
            if let Some(first_failure_step) = candidate.first_structural_failure_step {
                let definition = candidates
                    .iter()
                    .find(|definition| definition.id == candidate.candidate)
                    .expect("candidate definition exists");
                let mut reproducer = scene.source.clone();
                reproducer.settings = definition.settings;
                reproducer.requested_steps = first_failure_step;
                let output = run_candidate(&runner, &reproducer, first_failure_step, definition)?;
                let overflowed = output.failure.is_some()
                    || output
                        .capacities
                        .iter()
                        .any(|capacity| !capacity.accepted || capacity.overflowed);
                let (_, exact_reason) = structure(kind, &reproducer, &output.bodies, overflowed);
                failures.push(FailureRecord {
                    candidate: candidate.candidate,
                    scene: label.clone(),
                    minimized_scene: label.clone(),
                    first_failure_step,
                    reason: exact_reason,
                    scenario_hash: canonical_hash(&reproducer)
                        .map_err(|error| error.to_string())?,
                    result_hash: serialized_hash(&output)?,
                    reproducer,
                    bodies: output.bodies,
                });
            }
        }
        scenes.push(scene);
    }
    minimize_failure_scene_aliases(&mut failures);
    for failure in &failures {
        atomic_json(
            &staging
                .join("failures")
                .join(format!("{}-{}.json", failure.candidate, failure.scene)),
            failure,
        )?;
    }

    let gpu_performance = performance_matrix(&arguments, &candidates)?;
    let optimized_box2d_performance = optimized_box2d_matrix(&arguments, &staging)?;
    let convergence = convergence_decision(&candidates, &scenes, &gpu_performance)?;
    let platform_artifacts = copy_platform_artifacts(&arguments.platforms, &staging)?;
    atomic_write(
        &staging.join("playback.svg"),
        render_playback(&scenes, &candidates)?.as_bytes(),
    )?;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        metrics_version: AWAKE_SETTLING_METRICS_VERSION,
        artifact_kind: "p5.2b-s5.4a-parallel-normal-factorial",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        candidates,
        frozen_normal_diagnostics,
        scenes,
        failures,
        gpu_performance,
        optimized_box2d_performance,
        convergence,
        platform_artifacts,
        physical_device_gap: arguments.physical_device_gap,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
        playback: "playback.svg",
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s5.4a-parallel-normal-factorial run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn run_colored_factorial(arguments: Arguments) -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S5.4b coloring evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("oracles")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("failures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("performance")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let base = p5_2_centered_rectangle_stack_scenario(2, 1)
        .map_err(|error| error.to_string())?
        .settings;
    let candidates = colored_candidate_definitions(base);
    let scheduling_control = scheduling_control_evidence(&runner, base)?;
    if !scheduling_control.byte_exact {
        return Err("compact point scheduling no longer matches the fixed-grid control".to_owned());
    }

    let mut accepted_scenes = Vec::new();
    let mut accepted_kinds = Vec::new();
    for count in [2, 4, 8, 16] {
        let label = format!("rectangle-stack-{count}");
        eprintln!("S5.4b accepted scene {label}");
        accepted_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Stack,
            p5_2_centered_rectangle_stack_scenario(count, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        accepted_kinds.push(SceneKind::Stack);
    }
    for rows in [2, 3, 4] {
        let label = format!("rectangle-pyramid-{rows}");
        eprintln!("S5.4b accepted scene {label}");
        accepted_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Pyramid,
            p5_2_rectangle_pyramid_scenario(rows, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        accepted_kinds.push(SceneKind::Pyramid);
    }

    let mut diagnostic_scenes = Vec::new();
    let mut diagnostic_kinds = Vec::new();
    for count in [32, 64] {
        let label = format!("rectangle-stack-{count}-diagnostic");
        eprintln!("S5.4b depth diagnostic {label}");
        diagnostic_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Stack,
            p5_2_centered_rectangle_stack_scenario(count, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        diagnostic_kinds.push(SceneKind::Stack);
    }
    for shape in P5_2BucketShape::ALL {
        let label = format!("{}-bucket-100-diagnostic", shape.name());
        eprintln!("S5.4b degree diagnostic {label}");
        diagnostic_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Bucket,
            p5_2_bucket_scenario(shape, 100, 600).map_err(|error| error.to_string())?,
            &BUCKET_CHECKPOINTS,
            300,
            600,
        )?);
        diagnostic_kinds.push(SceneKind::Bucket);
    }

    let mut failures = Vec::new();
    for (scene, kind) in accepted_scenes
        .iter()
        .zip(&accepted_kinds)
        .chain(diagnostic_scenes.iter().zip(&diagnostic_kinds))
    {
        failures.extend(scene_failure_records(scene, *kind, &runner, &candidates)?);
    }
    minimize_failure_scene_aliases(&mut failures);
    for failure in &failures {
        atomic_json(
            &staging
                .join("failures")
                .join(format!("{}-{}.json", failure.candidate, failure.scene)),
            failure,
        )?;
    }

    let churn = coloring_churn_evidence(&runner, &candidates)?;
    let gpu_performance = performance_matrix(&arguments, &candidates)?;
    let optimized_box2d_performance = optimized_box2d_matrix(&arguments, &staging)?;
    let decision = colored_decision(
        &candidates,
        &accepted_scenes,
        &diagnostic_scenes,
        &gpu_performance,
    )?;
    let platform_artifacts = copy_platform_artifacts(&arguments.platforms, &staging)?;
    atomic_write(
        &staging.join("playback.svg"),
        render_colored_playback(&accepted_scenes, &candidates)?.as_bytes(),
    )?;
    let manifest = ColoredManifest {
        schema_version: SCHEMA_VERSION,
        metrics_version: AWAKE_SETTLING_METRICS_VERSION,
        artifact_kind: "p5.2b-s5.4b-compact-coloring-factorial",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        candidates,
        scheduling_control,
        accepted_scenes,
        diagnostic_scenes,
        churn,
        failures,
        gpu_performance,
        optimized_box2d_performance,
        decision,
        platform_artifacts,
        physical_device_gap: arguments.physical_device_gap,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
        selected_product_execution: GpuSolverExecutionV4::SELECTED,
        playback: "playback.svg",
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s5.4b-compact-coloring-factorial run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn run_scheduling_factorial(arguments: Arguments) -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S5.4b scheduling evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("oracles")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("failures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("performance")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let base = p5_2_centered_rectangle_stack_scenario(2, 1)
        .map_err(|error| error.to_string())?
        .settings;
    let candidates = scheduling_candidate_definitions(base);

    let mut accepted_scenes = Vec::new();
    let mut accepted_kinds = Vec::new();
    for count in [2, 4, 8, 16] {
        let label = format!("rectangle-stack-{count}");
        eprintln!("S5.4b scheduling accepted scene {label}");
        accepted_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Stack,
            p5_2_centered_rectangle_stack_scenario(count, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        accepted_kinds.push(SceneKind::Stack);
    }
    for rows in [2, 3, 4] {
        let label = format!("rectangle-pyramid-{rows}");
        eprintln!("S5.4b scheduling accepted scene {label}");
        accepted_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Pyramid,
            p5_2_rectangle_pyramid_scenario(rows, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        accepted_kinds.push(SceneKind::Pyramid);
    }

    let mut diagnostic_scenes = Vec::new();
    let mut diagnostic_kinds = Vec::new();
    for count in [32, 64] {
        let label = format!("rectangle-stack-{count}-diagnostic");
        eprintln!("S5.4b scheduling depth diagnostic {label}");
        diagnostic_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Stack,
            p5_2_centered_rectangle_stack_scenario(count, SETTLED_WINDOW_END)
                .map_err(|error| error.to_string())?,
            &CHECKPOINTS,
            SETTLED_WINDOW_START,
            SETTLED_WINDOW_END,
        )?);
        diagnostic_kinds.push(SceneKind::Stack);
    }
    for shape in P5_2BucketShape::ALL {
        let label = format!("{}-bucket-100-diagnostic", shape.name());
        eprintln!("S5.4b scheduling degree diagnostic {label}");
        diagnostic_scenes.push(evaluate_scene(
            &arguments,
            &staging,
            &box2d,
            &runner,
            &candidates,
            &label,
            SceneKind::Bucket,
            p5_2_bucket_scenario(shape, 100, 600).map_err(|error| error.to_string())?,
            &BUCKET_CHECKPOINTS,
            300,
            600,
        )?);
        diagnostic_kinds.push(SceneKind::Bucket);
    }

    let mut failures = Vec::new();
    for (scene, kind) in accepted_scenes
        .iter()
        .zip(&accepted_kinds)
        .chain(diagnostic_scenes.iter().zip(&diagnostic_kinds))
    {
        failures.extend(scene_failure_records(scene, *kind, &runner, &candidates)?);
    }
    minimize_failure_scene_aliases(&mut failures);
    for failure in &failures {
        atomic_json(
            &staging
                .join("failures")
                .join(format!("{}-{}.json", failure.candidate, failure.scene)),
            failure,
        )?;
    }

    let churn = coloring_churn_evidence(&runner, &candidates)?;
    let gpu_performance = performance_matrix(&arguments, &candidates)?;
    let optimized_box2d_performance = optimized_box2d_matrix(&arguments, &staging)?;
    let decision = scheduling_decision(
        &candidates,
        &accepted_scenes,
        &diagnostic_scenes,
        &churn,
        &gpu_performance,
        &optimized_box2d_performance,
    )?;
    let platform_artifacts = copy_platform_artifacts(&arguments.platforms, &staging)?;
    atomic_write(
        &staging.join("playback.svg"),
        render_scheduling_playback(&accepted_scenes, &candidates)?.as_bytes(),
    )?;
    let manifest = SchedulingManifest {
        schema_version: SCHEMA_VERSION,
        metrics_version: AWAKE_SETTLING_METRICS_VERSION,
        artifact_kind: "p5.2b-s5.4b-scheduling-factorial",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.diagnostics().clone(),
        candidates,
        accepted_scenes,
        diagnostic_scenes,
        churn,
        failures,
        gpu_performance,
        optimized_box2d_performance,
        decision,
        platform_artifacts,
        physical_device_gap: arguments.physical_device_gap,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
        selected_product_execution: GpuSolverExecutionV4::SELECTED,
        playback: "playback.svg",
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s5.4b-scheduling-factorial run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scheduling_control_evidence(
    runner: &GpuGeneratedContactRunner,
    mut settings: SolverSettingsV1,
) -> Result<SchedulingControlEvidence, String> {
    let mut scenario =
        p5_2_centered_rectangle_stack_scenario(8, 8).map_err(|error| error.to_string())?;
    settings.iterations = 4;
    scenario.settings = settings;
    let fixed = runner.run_experiment(
        &scenario,
        8,
        GpuSolverExecutionV4::colored_experiment(true, true),
    )?;
    let compact = runner.run_experiment(
        &scenario,
        8,
        GpuSolverExecutionV4::compact_colored_experiment(true, true),
    )?;
    let fixed_coloring = fixed
        .pass_evidence
        .coloring
        .as_ref()
        .ok_or_else(|| "fixed scheduling control omitted coloring evidence".to_owned())?;
    let compact_coloring = compact
        .pass_evidence
        .coloring
        .as_ref()
        .ok_or_else(|| "compact scheduling control omitted coloring evidence".to_owned())?;
    let byte_exact = fixed.bodies == compact.bodies
        && fixed.cache == compact.cache
        && fixed.pass_evidence.contacts == compact.pass_evidence.contacts
        && fixed.pass_evidence.contributions == compact.pass_evidence.contributions
        && fixed.pass_evidence.manifold_blocks == compact.pass_evidence.manifold_blocks
        && fixed_coloring.point_colors == compact_coloring.point_colors
        && fixed_coloring.color_occupancy == compact_coloring.color_occupancy;
    Ok(SchedulingControlEvidence {
        scenario_hash: canonical_hash(&scenario).map_err(|error| error.to_string())?,
        completed_steps: 8,
        iterations: 4,
        byte_exact,
        fixed_result_hash: serialized_hash(&fixed)?,
        compact_result_hash: serialized_hash(&compact)?,
    })
}

fn candidate_definitions(base: SolverSettingsV1) -> Vec<CandidateDefinition> {
    let definitions = [
        (
            CandidateFamilyV3::PointJacobi,
            GpuNormalSolverV4::PointJacobi,
            "current parallel point solve baseline",
        ),
        (
            CandidateFamilyV3::ScalarMassSplit,
            GpuNormalSolverV4::ScalarMassSplit,
            "does point-incidence mass splitting make convergence monotonic",
        ),
        (
            CandidateFamilyV3::ManifoldBlock,
            GpuNormalSolverV4::ManifoldBlock,
            "does coupled two-point normal solving remove the rocking mode",
        ),
        (
            CandidateFamilyV3::BlockMassSplit,
            GpuNormalSolverV4::BlockMassSplit,
            "does manifold incidence plus block solving resolve chain and rocking modes",
        ),
        (
            CandidateFamilyV3::ColoredInPlace,
            GpuNormalSolverV4::PointJacobi,
            "does disjoint GPU coloring make chain convergence monotonic without a gather",
        ),
    ];
    let ids = [
        [
            "point-jacobi-i1",
            "point-jacobi-i2",
            "point-jacobi-i4",
            "point-jacobi-i8",
        ],
        [
            "scalar-mass-split-i1",
            "scalar-mass-split-i2",
            "scalar-mass-split-i4",
            "scalar-mass-split-i8",
        ],
        [
            "manifold-block-i1",
            "manifold-block-i2",
            "manifold-block-i4",
            "manifold-block-i8",
        ],
        [
            "block-mass-split-i1",
            "block-mass-split-i2",
            "block-mass-split-i4",
            "block-mass-split-i8",
        ],
        [
            "colored-in-place-i1",
            "colored-in-place-i2",
            "colored-in-place-i4",
            "colored-in-place-i8",
        ],
    ];
    let iterations = [1, 2, 4, 8];
    let mut candidates = Vec::with_capacity(20);
    for (family_index, (family, normal_solver, question)) in definitions.into_iter().enumerate() {
        for (iteration_index, iteration_count) in iterations.into_iter().enumerate() {
            let mut settings = base;
            settings.iterations = iteration_count;
            let execution = if family == CandidateFamilyV3::ColoredInPlace {
                GpuSolverExecutionV4::colored_experiment(true, true)
            } else {
                GpuSolverExecutionV4::normal_experiment(normal_solver, true, true)
            };
            candidates.push(CandidateDefinition {
                id: ids[family_index][iteration_index],
                family,
                iterations: iteration_count,
                settings,
                execution,
                scheduling_policy: GpuSolverSchedulingPolicyV1::STANDARD,
                question,
            });
        }
    }
    candidates
}

fn colored_candidate_definitions(base: SolverSettingsV1) -> Vec<CandidateDefinition> {
    let definitions = [
        (
            "point-fixed-i4",
            CandidateFamilyV3::CompactPointControl,
            4,
            GpuSolverExecutionV4::colored_experiment(true, true),
            "fixed-grid point-color scheduling attribution control",
        ),
        (
            "point-compact-i4",
            CandidateFamilyV3::CompactPointControl,
            4,
            GpuSolverExecutionV4::compact_colored_experiment(true, true),
            "does compact indirect scheduling preserve the fixed-grid point result",
        ),
        (
            "manifold-forward-i1",
            CandidateFamilyV3::ManifoldColorForward,
            1,
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
            "does manifold coupling preserve the corpus at one forward iteration",
        ),
        (
            "manifold-forward-i2",
            CandidateFamilyV3::ManifoldColorForward,
            2,
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
            "does manifold coupling preserve the corpus at two forward iterations",
        ),
        (
            "manifold-forward-i4",
            CandidateFamilyV3::ManifoldColorForward,
            4,
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
            "does manifold coupling preserve the corpus at four forward iterations",
        ),
        (
            "manifold-alternating-i1",
            CandidateFamilyV3::ManifoldColorAlternating,
            1,
            GpuSolverExecutionV4::alternating_manifold_colored_block_experiment(true, true),
            "does alternating order improve one-iteration propagation",
        ),
        (
            "manifold-alternating-i2",
            CandidateFamilyV3::ManifoldColorAlternating,
            2,
            GpuSolverExecutionV4::alternating_manifold_colored_block_experiment(true, true),
            "does alternating order improve two-iteration propagation",
        ),
        (
            "manifold-alternating-i4",
            CandidateFamilyV3::ManifoldColorAlternating,
            4,
            GpuSolverExecutionV4::alternating_manifold_colored_block_experiment(true, true),
            "does alternating order improve four-iteration propagation",
        ),
    ];
    definitions
        .into_iter()
        .map(|(id, family, iterations, execution, question)| {
            let mut settings = base;
            settings.iterations = iterations;
            CandidateDefinition {
                id,
                family,
                iterations,
                settings,
                execution,
                scheduling_policy: GpuSolverSchedulingPolicyV1::STANDARD,
                question,
            }
        })
        .collect()
}

fn scheduling_candidate_definitions(base: SolverSettingsV1) -> Vec<CandidateDefinition> {
    [
        (
            "forward-i2-full-per-color",
            GpuSolverSchedulingPolicyV1::STANDARD,
            "full color rebuild with one indirect dispatch per possible color",
        ),
        (
            "forward-i2-incremental-per-color",
            GpuSolverSchedulingPolicyV1::INCREMENTAL,
            "stable-pair incremental color maintenance with per-color dispatch",
        ),
        (
            "forward-i2-full-island",
            GpuSolverSchedulingPolicyV1::WORKGROUP_PER_ISLAND,
            "full color rebuild with one barrier-synchronized workgroup per island",
        ),
        (
            "forward-i2-incremental-island",
            GpuSolverSchedulingPolicyV1::INCREMENTAL_WORKGROUP_PER_ISLAND,
            "incremental colors crossed with one barrier-synchronized workgroup per island",
        ),
    ]
    .into_iter()
    .map(|(id, scheduling_policy, question)| {
        let mut settings = base;
        settings.iterations = 2;
        CandidateDefinition {
            id,
            family: CandidateFamilyV3::ManifoldColorForward,
            iterations: 2,
            settings,
            execution: GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
            scheduling_policy,
            question,
        }
    })
    .collect()
}

fn run_candidate(
    runner: &GpuGeneratedContactRunner,
    scenario: &ScenarioV1,
    completed_steps: u32,
    candidate: &CandidateDefinition,
) -> Result<GpuGeneratedContactOutput, String> {
    runner.run_scheduling_experiment(
        scenario,
        completed_steps,
        candidate.execution,
        candidate.scheduling_policy,
    )
}

fn run_candidate_history(
    runner: &GpuGeneratedContactRunner,
    scenario: &ScenarioV1,
    completed_steps: u32,
    candidate: &CandidateDefinition,
) -> Result<GpuGeneratedContactBodyHistoryV1, String> {
    runner.run_scheduling_experiment_body_history_diagnostic(
        scenario,
        completed_steps,
        candidate.execution,
        candidate.scheduling_policy,
    )
}

fn scene_failure_records(
    scene: &SceneEvidence,
    kind: SceneKind,
    runner: &GpuGeneratedContactRunner,
    candidates: &[CandidateDefinition],
) -> Result<Vec<FailureRecord>, String> {
    let mut failures = Vec::new();
    for candidate in &scene.candidates {
        let execution_failure_step = candidate
            .first_execution_failure
            .as_ref()
            .and_then(|failure| failure.first_step);
        let first_failure_step = match (
            candidate.first_structural_failure_step,
            execution_failure_step,
        ) {
            (Some(structural), Some(execution)) => Some(structural.min(execution)),
            (Some(structural), None) => Some(structural),
            (None, Some(execution)) => Some(execution),
            (None, None) => None,
        };
        let Some(first_failure_step) = first_failure_step else {
            continue;
        };
        let definition = candidates
            .iter()
            .find(|definition| definition.id == candidate.candidate)
            .expect("candidate definition exists");
        let mut reproducer = scene.source.clone();
        reproducer.settings = definition.settings;
        reproducer.requested_steps = first_failure_step;
        let output = run_candidate(runner, &reproducer, first_failure_step, definition)?;
        let overflowed = output.failure.is_some()
            || output
                .capacities
                .iter()
                .any(|capacity| !capacity.accepted || capacity.overflowed);
        let (_, structural_reason) = structure(kind, &reproducer, &output.bodies, overflowed);
        let exact_reason = candidate
            .first_execution_failure
            .as_ref()
            .filter(|failure| failure.first_step == Some(first_failure_step))
            .map_or(structural_reason, |failure| failure.reason.clone());
        failures.push(FailureRecord {
            candidate: candidate.candidate,
            scene: scene.label.clone(),
            minimized_scene: scene.label.clone(),
            first_failure_step,
            reason: exact_reason,
            scenario_hash: canonical_hash(&reproducer).map_err(|error| error.to_string())?,
            result_hash: serialized_hash(&output)?,
            reproducer,
            bodies: output.bodies,
        });
    }
    Ok(failures)
}

type ColorUnitKey = (BodyId, BodyId, u32);

fn color_unit_map(
    scenario: &ScenarioV1,
    output: &physbox_engine::GpuGeneratedContactOutput,
    coloring: &GpuColoringEvidenceV4,
) -> Result<BTreeMap<ColorUnitKey, u32>, String> {
    let mut units = BTreeMap::new();
    for (contact, &color) in output
        .pass_evidence
        .contacts
        .iter()
        .filter(|contact| contact.valid)
        .zip(&coloring.point_colors)
    {
        if coloring.unit == physbox_engine::GpuColorUnitV4::Manifold
            && contact.point_in_manifold != 0
        {
            continue;
        }
        let body_a = scenario
            .bodies
            .get(contact.body_a as usize)
            .ok_or_else(|| "color evidence body A is outside the scenario".to_owned())?
            .id;
        let body_b = scenario
            .bodies
            .get(contact.body_b as usize)
            .ok_or_else(|| "color evidence body B is outside the scenario".to_owned())?
            .id;
        let key = if body_a <= body_b {
            (body_a, body_b, contact.point_in_manifold)
        } else {
            (body_b, body_a, contact.point_in_manifold)
        };
        if units.insert(key, color).is_some() {
            return Err("color churn evidence produced a duplicate unit key".to_owned());
        }
    }
    Ok(units)
}

fn coloring_churn_evidence(
    runner: &GpuGeneratedContactRunner,
    candidates: &[CandidateDefinition],
) -> Result<ChurnEvidence, String> {
    let sources = p5_2_coloring_churn_snapshots(1).map_err(|error| error.to_string())?;
    let mut previous = BTreeMap::<&'static str, BTreeMap<ColorUnitKey, u32>>::new();
    let mut snapshots = Vec::with_capacity(sources.len());
    for source in sources {
        let source_scenario_hash =
            canonical_hash(&source.scenario).map_err(|error| error.to_string())?;
        let mut candidate_evidence = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let mut scenario = source.scenario.clone();
            scenario.settings = candidate.settings;
            let output = run_candidate(runner, &scenario, 1, candidate)?;
            let replay = run_candidate(runner, &scenario, 1, candidate)?;
            let result_hash = serialized_hash(&output)?;
            let exact_replay = serialized_hash(&replay)? == result_hash;
            let capacity_clean = output.failure.is_none()
                && output
                    .capacities
                    .iter()
                    .all(|capacity| capacity.accepted && !capacity.overflowed);
            let coloring = output
                .pass_evidence
                .coloring
                .clone()
                .ok_or_else(|| format!("{} omitted churn coloring evidence", candidate.id))?;
            let colors = color_unit_map(&scenario, &output, &coloring)?;
            let prior = previous.get(candidate.id);
            let surviving_color_units = prior.map_or(0, |prior| {
                colors.keys().filter(|key| prior.contains_key(key)).count() as u32
            });
            let surviving_color_changes = prior.map_or(0, |prior| {
                colors
                    .iter()
                    .filter(|(key, color)| prior.get(key).is_some_and(|prior| prior != *color))
                    .count() as u32
            });
            previous.insert(candidate.id, colors);
            candidate_evidence.push(ChurnCandidateEvidence {
                candidate: candidate.id,
                exact_replay,
                capacity_clean,
                rebuilt_color_units: coloring.color_occupancy.iter().sum(),
                surviving_color_units,
                surviving_color_changes,
                result_hash,
                coloring,
            });
        }
        snapshots.push(ChurnSnapshotEvidence {
            label: source.label,
            constructed: source.constructed,
            erased: source.erased,
            source_scenario_hash,
            candidates: candidate_evidence,
        });
    }
    let temporal_probes = temporal_churn_evidence(runner, candidates)?;
    Ok(ChurnEvidence {
        version: physbox_engine::P5_2_COLORING_CHURN_VERSION,
        full_rebuild_each_snapshot: candidates.iter().all(|candidate| {
            candidate.scheduling_policy.color_maintenance
                == physbox_engine::GpuColorMaintenanceV1::FullRebuild
        }),
        incremental_maintenance_included: candidates.iter().any(|candidate| {
            candidate.scheduling_policy.color_maintenance
                == physbox_engine::GpuColorMaintenanceV1::StablePairIncremental
        }),
        fresh_graph_snapshots: true,
        snapshot_interpretation: "construction/erase snapshots compare deterministic assignments and surviving stable identities across independent fresh graphs; policy-observed retained/new/recolored/removed counters belong to the single-graph temporal probes",
        snapshots,
        temporal_probes,
    })
}

fn temporal_churn_evidence(
    runner: &GpuGeneratedContactRunner,
    candidates: &[CandidateDefinition],
) -> Result<Vec<TemporalChurnEvidence>, String> {
    let stable_source =
        p5_2_centered_rectangle_stack_scenario(4, 8).map_err(|error| error.to_string())?;
    let removal_source = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 3)
        .map_err(|error| error.to_string())?;
    candidates
        .iter()
        .map(|candidate| {
            let mut stable = stable_source.clone();
            stable.settings = candidate.settings;
            let stable_first = run_candidate(runner, &stable, 1, candidate)?;
            let stable_eighth = run_candidate(runner, &stable, 8, candidate)?;
            let stable_replay = run_candidate(runner, &stable, 8, candidate)?;
            let mut removal = removal_source.clone();
            removal.settings = candidate.settings;
            let removal_third = run_candidate(runner, &removal, 3, candidate)?;
            let removal_replay = run_candidate(runner, &removal, 3, candidate)?;
            let capacity_clean = [&stable_first, &stable_eighth, &removal_third]
                .into_iter()
                .all(|output| {
                    output.failure.is_none()
                        && output
                            .capacities
                            .iter()
                            .all(|capacity| capacity.accepted && !capacity.overflowed)
                });
            Ok(TemporalChurnEvidence {
                candidate: candidate.id,
                stable_first_step: stable_first
                    .pass_evidence
                    .coloring
                    .clone()
                    .ok_or_else(|| format!("{} omitted first-step coloring", candidate.id))?,
                stable_eighth_step: stable_eighth
                    .pass_evidence
                    .coloring
                    .clone()
                    .ok_or_else(|| format!("{} omitted steady coloring", candidate.id))?,
                removal_third_step: removal_third
                    .pass_evidence
                    .coloring
                    .clone()
                    .ok_or_else(|| format!("{} omitted removal coloring", candidate.id))?,
                exact_replay: serialized_hash(&stable_eighth)? == serialized_hash(&stable_replay)?
                    && serialized_hash(&removal_third)? == serialized_hash(&removal_replay)?,
                capacity_clean,
                stable_first_result_hash: serialized_hash(&stable_first)?,
                stable_eighth_result_hash: serialized_hash(&stable_eighth)?,
                removal_result_hash: serialized_hash(&removal_third)?,
            })
        })
        .collect()
}

fn phase_p50(
    row: &GpuPerformanceRow,
    value: impl Fn(&GpuSolverInstrumentedSampleV4) -> f64,
) -> Result<Option<f64>, String> {
    row.instrumented_solver_phases
        .as_ref()
        .map(|samples| nearest_rank_p50(&samples.iter().map(value).collect::<Vec<_>>()))
        .transpose()
}

fn colored_decision(
    candidates: &[CandidateDefinition],
    accepted_scenes: &[SceneEvidence],
    diagnostic_scenes: &[SceneEvidence],
    performance: &[GpuPerformanceRow],
) -> Result<ColoredDecision, String> {
    if accepted_scenes.len() != 7 || diagnostic_scenes.len() != 4 {
        return Err("S5.4b evidence has the wrong acceptance or diagnostic scene count".to_owned());
    }
    let mut candidate_decisions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let accepted = accepted_scenes
            .iter()
            .map(|scene| candidate_scene(scene, candidate.id))
            .collect::<Result<Vec<_>, _>>()?;
        let diagnostics = diagnostic_scenes
            .iter()
            .map(|scene| candidate_scene(scene, candidate.id))
            .collect::<Result<Vec<_>, _>>()?;
        let one_k = performance
            .iter()
            .find(|row| row.candidate == candidate.id && row.bodies == 1_024)
            .ok_or_else(|| format!("{} omitted its 1K performance row", candidate.id))?;
        let accepted_exact_and_capacity_clean = accepted.iter().all(|scene| {
            scene.exact_replay
                && scene.preserves_structure_through_run
                && scene.samples.iter().all(|sample| sample.capacity_clean)
        });
        let accepted_structural_passes = accepted
            .iter()
            .filter(|scene| scene.preserves_structure_through_run)
            .count() as u32;
        let diagnostic_structural_passes = diagnostics
            .iter()
            .filter(|scene| scene.preserves_structure_through_run)
            .count() as u32;
        let one_k_complete_step_seconds = one_k.gpu_end_to_end_p50_seconds;
        let potential_candidate = candidate.family != CandidateFamilyV3::CompactPointControl
            && accepted_structural_passes == 7
            && accepted_exact_and_capacity_clean
            && one_k.capacity_clean;
        candidate_decisions.push(ColoredCandidateDecision {
            candidate: candidate.id,
            family: candidate.family,
            iterations: candidate.iterations,
            accepted_structural_passes,
            accepted_exact_and_capacity_clean,
            diagnostic_structural_passes,
            one_k_complete_step_seconds,
            one_k_below_feasibility_boundary: one_k_complete_step_seconds
                .is_some_and(|seconds| seconds < FEASIBILITY_BOUNDARY_SECONDS),
            one_k_color_construction_seconds: phase_p50(one_k, |sample| {
                sample.color_construction_seconds
            })?,
            one_k_color_compaction_seconds: phase_p50(one_k, |sample| {
                sample.color_compaction_seconds
            })?,
            one_k_active_color_solve_seconds: phase_p50(one_k, |sample| {
                sample.active_color_solve_seconds
            })?,
            one_k_empty_color_solve_seconds: phase_p50(one_k, |sample| {
                sample.empty_color_solve_seconds
            })?,
            potential_candidate,
        });
    }
    let potential_candidates = candidate_decisions
        .iter()
        .filter(|candidate| candidate.potential_candidate)
        .map(|candidate| candidate.candidate)
        .collect::<Vec<_>>();
    let leading = candidate_decisions
        .iter()
        .filter(|candidate| candidate.potential_candidate)
        .filter_map(|candidate| {
            candidate
                .one_k_complete_step_seconds
                .map(|seconds| (seconds, candidate))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0));
    let incremental_coloring_triggered = leading.is_some_and(|(_, candidate)| {
        match (
            candidate.one_k_color_construction_seconds,
            candidate.one_k_color_compaction_seconds,
            candidate.one_k_active_color_solve_seconds,
        ) {
            (Some(construction), Some(compaction), Some(active)) => {
                construction + compaction >= active
            }
            _ => false,
        }
    });
    let workgroup_per_island_triggered = leading.is_some_and(|(_, candidate)| {
        !candidate.one_k_below_feasibility_boundary
            || match (
                candidate.one_k_empty_color_solve_seconds,
                candidate.one_k_active_color_solve_seconds,
            ) {
                (Some(empty), Some(active)) => empty >= active,
                _ => false,
            }
    });
    Ok(ColoredDecision {
        version: 4,
        feasibility_boundary_seconds: FEASIBILITY_BOUNDARY_SECONDS,
        accepted_scene_count: accepted_scenes.len() as u32,
        diagnostic_scene_count: diagnostic_scenes.len() as u32,
        candidates: candidate_decisions,
        potential_candidates,
        incremental_coloring_triggered,
        incremental_coloring_reason: if incremental_coloring_triggered {
            "the leading structurally qualified lane spends at least as much diagnostic GPU time rebuilding/compacting colors as executing active colored solve work"
                .to_owned()
        } else {
            "the leading structurally qualified lane does not spend as much diagnostic GPU time rebuilding/compacting colors as executing active colored solve work"
                .to_owned()
        },
        workgroup_per_island_triggered,
        workgroup_per_island_reason: if workgroup_per_island_triggered {
            "the leading structurally qualified lane either misses 16.67 ms complete or spends at least as much diagnostic GPU time on empty indirect color solves as active color solves"
                .to_owned()
        } else {
            "the leading structurally qualified lane is below 16.67 ms complete and empty indirect solve time is below active solve time"
                .to_owned()
        },
        production_selection_deferred: true,
        recommendation_deferred_to_human_review: true,
    })
}

fn scheduling_decision(
    candidates: &[CandidateDefinition],
    accepted_scenes: &[SceneEvidence],
    diagnostic_scenes: &[SceneEvidence],
    churn: &ChurnEvidence,
    performance: &[GpuPerformanceRow],
    optimized_box2d: &[OptimizedBox2dRow],
) -> Result<SchedulingDecision, String> {
    if accepted_scenes.len() != 7 || diagnostic_scenes.len() != 4 {
        return Err(
            "S5.4b scheduling evidence has the wrong acceptance or diagnostic scene count"
                .to_owned(),
        );
    }
    let fastest_box2d = optimized_box2d
        .iter()
        .filter(|row| row.bodies == 1_024 && row.success_sentinel && !row.source_dirty)
        .map(|row| row.p50_seconds)
        .min_by(f64::total_cmp)
        .ok_or_else(|| {
            "S5.4b scheduling evidence omitted a clean successful 1K Box2D row".to_owned()
        })?;
    let mut candidate_decisions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let accepted = accepted_scenes
            .iter()
            .map(|scene| candidate_scene(scene, candidate.id))
            .collect::<Result<Vec<_>, _>>()?;
        let diagnostics = diagnostic_scenes
            .iter()
            .map(|scene| candidate_scene(scene, candidate.id))
            .collect::<Result<Vec<_>, _>>()?;
        let rows = performance
            .iter()
            .filter(|row| row.candidate == candidate.id)
            .collect::<Vec<_>>();
        if rows.len() != 3
            || [32, 256, 1_024]
                .into_iter()
                .any(|bodies| rows.iter().filter(|row| row.bodies == bodies).count() != 1)
        {
            return Err(format!(
                "{} has an incomplete scheduling performance matrix",
                candidate.id
            ));
        }
        let one_k = rows
            .iter()
            .copied()
            .find(|row| row.bodies == 1_024)
            .expect("the matrix check established the 1K row");
        let accepted_structural_passes = accepted
            .iter()
            .filter(|scene| scene.preserves_structure_through_run)
            .count() as u32;
        let accepted_exact_and_capacity_clean = accepted.iter().all(|scene| {
            scene.exact_replay
                && scene.preserves_structure_through_run
                && scene.samples.iter().all(|sample| sample.capacity_clean)
        });
        let diagnostic_structural_passes = diagnostics
            .iter()
            .filter(|scene| scene.preserves_structure_through_run)
            .count() as u32;
        let temporal = churn
            .temporal_probes
            .iter()
            .find(|probe| probe.candidate == candidate.id)
            .ok_or_else(|| format!("{} omitted temporal churn evidence", candidate.id))?;
        let churn_clean = temporal.exact_replay
            && temporal.capacity_clean
            && churn.snapshots.iter().all(|snapshot| {
                snapshot
                    .candidates
                    .iter()
                    .find(|evidence| evidence.candidate == candidate.id)
                    .is_some_and(|evidence| evidence.exact_replay && evidence.capacity_clean)
            });
        let one_k_complete_step_seconds = one_k.gpu_end_to_end_p50_seconds;
        let qualified = accepted_structural_passes == 7
            && accepted_exact_and_capacity_clean
            && churn_clean
            && rows.iter().all(|row| row.capacity_clean);
        candidate_decisions.push(SchedulingCandidateDecision {
            candidate: candidate.id,
            scheduling_policy: candidate.scheduling_policy,
            accepted_structural_passes,
            accepted_exact_and_capacity_clean,
            diagnostic_structural_passes,
            churn_clean,
            one_k_complete_step_seconds,
            one_k_below_feasibility_boundary: one_k.capacity_clean
                && one_k_complete_step_seconds
                    .is_some_and(|seconds| seconds < FEASIBILITY_BOUNDARY_SECONDS),
            one_k_dispatches_per_step: one_k.dispatches_per_step,
            one_k_allocated_bytes: one_k.allocated_bytes,
            one_k_color_construction_seconds: phase_p50(one_k, |sample| {
                sample.color_construction_seconds
            })?,
            one_k_color_compaction_seconds: phase_p50(one_k, |sample| {
                sample.color_compaction_seconds
            })?,
            one_k_active_color_solve_seconds: phase_p50(one_k, |sample| {
                sample.active_color_solve_seconds
            })?,
            one_k_empty_color_solve_seconds: phase_p50(one_k, |sample| {
                sample.empty_color_solve_seconds
            })?,
            fastest_box2d_ratio: one_k_complete_step_seconds.map(|seconds| seconds / fastest_box2d),
            qualified,
        });
    }
    let qualified_candidates = candidate_decisions
        .iter()
        .filter(|candidate| candidate.qualified)
        .map(|candidate| candidate.candidate)
        .collect::<Vec<_>>();
    let recommended_for_human_review = candidate_decisions
        .iter()
        .filter(|candidate| candidate.qualified)
        .filter_map(|candidate| {
            candidate
                .one_k_complete_step_seconds
                .map(|seconds| (seconds, candidate.candidate))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, candidate)| candidate);
    let recommendation_reason = recommended_for_human_review.map_or_else(
        || {
            "no scheduling lane preserved the governed acceptance corpus and churn probes with exact replay and clean capacity, so the evidence makes no recommendation"
                .to_owned()
        },
        |candidate| {
            format!(
                "{candidate} has the lowest ordinary 1K complete-step median among scheduling lanes that preserve all seven accepted structures and churn probes with exact replay and clean capacity; production selection and quality-band acceptance remain human decisions"
            )
        },
    );
    Ok(SchedulingDecision {
        version: 1,
        feasibility_boundary_seconds: FEASIBILITY_BOUNDARY_SECONDS,
        accepted_scene_count: accepted_scenes.len() as u32,
        diagnostic_scene_count: diagnostic_scenes.len() as u32,
        candidates: candidate_decisions,
        qualified_candidates,
        recommended_for_human_review,
        recommendation_reason,
        production_selection_deferred: true,
        quality_band_acceptance_deferred: true,
        reconsideration_triggers: vec![
            "a required browser or Android host changes correctness, capacity, or relative scheduling cost",
            "the recommended lane loses exact replay or governed-corpus structure",
            "island construction, bin storage, or color maintenance dominates at target scale",
            "liquid coupling materially changes the scheduling crossover",
            "a required workload exceeds the 32-color or one-workgroup-per-island boundary",
        ],
    })
}

fn frozen_normal_diagnostics() -> Result<Vec<FrozenNormalDiagnosticV1>, String> {
    let modes = [
        ParallelNormalModeV1::PointJacobi,
        ParallelNormalModeV1::ScalarMassSplit,
        ParallelNormalModeV1::ManifoldBlock,
        ParallelNormalModeV1::BlockMassSplit,
    ];
    let mut diagnostics = Vec::with_capacity(16);
    for rectangle_count in [2, 4, 8, 16] {
        for mode in modes {
            diagnostics.push(frozen_rectangle_stack_diagnostic(rectangle_count, mode, 8)?);
        }
    }
    Ok(diagnostics)
}

fn convergence_decision(
    candidates: &[CandidateDefinition],
    scenes: &[SceneEvidence],
    performance: &[GpuPerformanceRow],
) -> Result<ConvergenceDecision, String> {
    if scenes.len() != EXISTING_S5_4_SCENES.len() + 1 {
        return Err("factorial evidence does not contain exactly seven scenes".to_owned());
    }
    for required in EXISTING_S5_4_SCENES
        .into_iter()
        .chain(["rectangle-stack-16"])
    {
        if scenes
            .iter()
            .filter(|scene| scene.label == required)
            .count()
            != 1
        {
            return Err(format!(
                "factorial evidence omitted or duplicated {required}"
            ));
        }
    }
    let families = [
        CandidateFamilyV3::PointJacobi,
        CandidateFamilyV3::ScalarMassSplit,
        CandidateFamilyV3::ManifoldBlock,
        CandidateFamilyV3::BlockMassSplit,
        CandidateFamilyV3::ColoredInPlace,
    ];
    let mut monotonic_violations = Vec::new();
    for family in families {
        let family_candidates = candidates
            .iter()
            .filter(|candidate| candidate.family == family)
            .collect::<Vec<_>>();
        if family_candidates.len() != 4
            || family_candidates
                .windows(2)
                .any(|pair| pair[0].iterations >= pair[1].iterations)
        {
            return Err(format!(
                "factorial family {family:?} is incomplete or unordered"
            ));
        }
        for pair in family_candidates.windows(2) {
            for scene in scenes {
                let lower = candidate_scene(scene, pair[0].id)?;
                let higher = candidate_scene(scene, pair[1].id)?;
                let lower_failure = lower
                    .first_structural_failure_step
                    .unwrap_or(SETTLED_WINDOW_END + 1);
                let higher_failure = higher
                    .first_structural_failure_step
                    .unwrap_or(SETTLED_WINDOW_END + 1);
                if higher_failure < lower_failure {
                    monotonic_violations.push(MonotonicViolation {
                        family,
                        scene: scene.label.clone(),
                        lower_candidate: pair[0].id,
                        higher_candidate: pair[1].id,
                        dimension: "first-structural-failure-step",
                        lower_value: f64::from(lower_failure),
                        higher_value: f64::from(higher_failure),
                    });
                }
                if !(lower.preserves_structure_through_run
                    && higher.preserves_structure_through_run)
                {
                    continue;
                }
                for (dimension, lower_value, higher_value) in [
                    (
                        "settled-residual-linear-speed-max",
                        lower.window.residual_linear_speed_max,
                        higher.window.residual_linear_speed_max,
                    ),
                    (
                        "settled-residual-angular-speed-max",
                        lower.window.residual_angular_speed_max,
                        higher.window.residual_angular_speed_max,
                    ),
                    (
                        "settled-kinetic-energy-max",
                        lower.window.kinetic_energy_max,
                        higher.window.kinetic_energy_max,
                    ),
                ] {
                    if higher_value > lower_value {
                        monotonic_violations.push(MonotonicViolation {
                            family,
                            scene: scene.label.clone(),
                            lower_candidate: pair[0].id,
                            higher_candidate: pair[1].id,
                            dimension,
                            lower_value: f64::from(lower_value),
                            higher_value: f64::from(higher_value),
                        });
                    }
                }
            }
        }
    }

    let mut eligibility = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let candidate_performance = performance
            .iter()
            .filter(|row| row.candidate == candidate.id)
            .collect::<Vec<_>>();
        if candidate_performance.len() != 3
            || [32, 256, 1_024].into_iter().any(|body_count| {
                candidate_performance
                    .iter()
                    .filter(|row| row.bodies == body_count)
                    .count()
                    != 1
            })
        {
            return Err(format!(
                "candidate {} has an incomplete performance matrix",
                candidate.id
            ));
        }
        let family_is_monotonic = !monotonic_violations
            .iter()
            .any(|violation| violation.family == candidate.family);
        let preserves_existing_s5_4_corpus = EXISTING_S5_4_SCENES.iter().all(|required| {
            scenes
                .iter()
                .find(|scene| scene.label == *required)
                .and_then(|scene| {
                    scene
                        .candidates
                        .iter()
                        .find(|evidence| evidence.candidate == candidate.id)
                })
                .is_some_and(|evidence| evidence.preserves_structure_through_run)
        });
        let exact_and_capacity_clean = scenes.iter().all(|scene| {
            candidate_scene(scene, candidate.id).is_ok_and(|evidence| {
                evidence.exact_replay && evidence.samples.iter().all(|sample| sample.capacity_clean)
            })
        }) && candidate_performance
            .iter()
            .all(|row| row.capacity_clean);
        let one_k = candidate_performance
            .iter()
            .copied()
            .find(|row| row.bodies == 1_024)
            .ok_or_else(|| format!("candidate {} omitted the 1K timing row", candidate.id))?;
        let one_k_complete_step_seconds = one_k.gpu_end_to_end_p50_seconds;
        let one_k_below_feasibility_boundary = one_k.capacity_clean
            && one_k_complete_step_seconds
                .is_some_and(|seconds| seconds < FEASIBILITY_BOUNDARY_SECONDS);
        let satisfies_conditional_exit = candidate.family != CandidateFamilyV3::PointJacobi
            && exact_and_capacity_clean
            && preserves_existing_s5_4_corpus
            && family_is_monotonic
            && one_k_below_feasibility_boundary;
        eligibility.push(CandidateEligibility {
            candidate: candidate.id,
            family: candidate.family,
            iterations: candidate.iterations,
            exact_and_capacity_clean,
            preserves_existing_s5_4_corpus,
            family_is_monotonic,
            one_k_complete_step_seconds,
            one_k_below_feasibility_boundary,
            satisfies_conditional_exit,
        });
    }
    let qualifying = eligibility
        .iter()
        .filter(|candidate| {
            candidate.family.is_precolored_experiment() && candidate.satisfies_conditional_exit
        })
        .map(|candidate| candidate.candidate)
        .collect::<Vec<_>>();
    let colored_comparator_triggered = qualifying.is_empty();
    let reason = if colored_comparator_triggered {
        "no mass-split/block candidate preserves the existing S5.4 corpus with exact clean replay, monotonic family behavior, and a complete 1K step below 16.67 ms"
            .to_owned()
    } else {
        format!(
            "the conditional colored comparator is not triggered because these candidates satisfy every recorded exit gate: {}",
            qualifying.join(", ")
        )
    };
    Ok(ConvergenceDecision {
        version: 3,
        feasibility_boundary_seconds: FEASIBILITY_BOUNDARY_SECONDS,
        required_existing_scenes: EXISTING_S5_4_SCENES,
        monotonic_violations,
        candidates: eligibility,
        colored_comparator_triggered,
        reason,
    })
}

fn candidate_scene<'a>(
    scene: &'a SceneEvidence,
    candidate: &str,
) -> Result<&'a CandidateSceneEvidence, String> {
    scene
        .candidates
        .iter()
        .find(|evidence| evidence.candidate == candidate)
        .ok_or_else(|| format!("scene {} omitted candidate {candidate}", scene.label))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_scene(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    candidates: &[CandidateDefinition],
    label: &str,
    kind: SceneKind,
    source: ScenarioV1,
    checkpoints: &[u32],
    settled_window_start: u32,
    completed_steps: u32,
) -> Result<SceneEvidence, String> {
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let projection = p5_2_box2d_world_projection(&source).map_err(|error| error.to_string())?;
    let neutral_projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let authority =
        run_awake_authority(arguments, staging, box2d, label, &projection, checkpoints)?;
    let authority_replay = run_awake_oracle(
        arguments,
        staging,
        box2d,
        &format!("{label}-replay"),
        projection.clone(),
        completed_steps,
    )?;
    let authority_exact_replay = authority_replay.result.bodies
        == authority
            .last()
            .expect("authority checkpoints are nonempty")
            .result
            .bodies
        && authority_replay.result.manifolds
            == authority
                .last()
                .expect("authority checkpoints are nonempty")
                .result
                .manifolds;

    let mut candidate_evidence = Vec::new();
    for candidate in candidates {
        let mut scenario = source.clone();
        scenario.settings = candidate.settings;
        scenario.validate().map_err(|error| error.to_string())?;
        let execution_scenario_hash =
            canonical_hash(&scenario).map_err(|error| error.to_string())?;
        let mut observations = Vec::new();
        let mut samples = Vec::new();
        let mut deltas = Vec::new();
        let mut previous_manifolds = None;
        let mut previous_cache = None;
        for (&checkpoint, authority_sample) in checkpoints.iter().zip(&authority) {
            let output = run_candidate(runner, &scenario, checkpoint, candidate)?;
            let result_hash = serialized_hash(&output)?;
            let mut final_scenario = scenario.clone();
            final_scenario.bodies.clone_from(&output.bodies);
            let pairs = enumerate_scalar_candidates(&final_scenario)?;
            let manifolds = generate_scalar_narrowphase(&final_scenario, &pairs.candidates)?;
            let observation = observe_settling(
                &scenario,
                SettlingSnapshotInput {
                    completed_steps: checkpoint,
                    bodies: &output.bodies,
                    manifolds: &manifolds,
                    cache: &output.cache,
                    capacities: &output.capacities,
                    previous_manifolds: previous_manifolds.as_deref(),
                    previous_cache: previous_cache.as_deref(),
                    support_height: 0.0,
                },
            )?;
            let contact_balance = observe_contact_balance(checkpoint, &output.pass_evidence)?;
            let authority_contact_points = authority_sample
                .result
                .manifolds
                .iter()
                .map(|manifold| manifold.points.len() as u32)
                .sum();
            let authority_delta = compare_awake_authority(
                checkpoint,
                &output.bodies,
                &authority_sample.result.bodies,
                output.pass_evidence.contacts.len() as u32,
                authority_contact_points,
            )?;
            let capacity_clean = output
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed)
                && output.failure.is_none();
            previous_manifolds = Some(manifolds);
            previous_cache = Some(output.cache.clone());
            observations.push(observation.clone());
            deltas.push(authority_delta.clone());
            let contact_points = output.pass_evidence.contacts.len() as u32;
            let cache_points = output.cache.len() as u32;
            samples.push(CandidateSample {
                completed_steps: checkpoint,
                observation,
                contact_balance,
                authority_delta,
                bodies: output.bodies,
                contact_cache: output.cache,
                pass_evidence: output.pass_evidence,
                work: output.work,
                capacities: output.capacities,
                contact_points,
                cache_points,
                dispatches_per_step: output.dispatches_per_step,
                allocated_bytes: output.allocated_bytes,
                capacity_clean,
                result_hash,
            });
        }
        let final_sample = samples.last().expect("candidate checkpoints are nonempty");
        let (structural_pass, structural_reason) = structure(
            kind,
            &scenario,
            &final_sample.bodies,
            !final_sample.capacity_clean,
        );
        let pose_history = run_candidate_history(runner, &scenario, completed_steps, candidate)?;
        let first_structural_failure_step = pose_history
            .bodies
            .iter()
            .position(|bodies| !structure(kind, &scenario, bodies, false).0)
            .map(|step| step as u32 + 1);
        let first_execution_failure = pose_history.failures.first().cloned();
        let preserves_structure_through_run =
            first_structural_failure_step.is_none() && first_execution_failure.is_none();
        let replay = run_candidate(runner, &scenario, completed_steps, candidate)?;
        let exact_replay = serialized_hash(&replay)? == final_sample.result_hash;
        candidate_evidence.push(CandidateSceneEvidence {
            candidate: candidate.id,
            execution_scenario_hash,
            exact_replay,
            structural_pass,
            structural_reason,
            preserves_structure_through_run,
            first_structural_failure_step,
            first_execution_failure,
            window: summarize_awake_window(&observations, settled_window_start, completed_steps)?,
            first_authority_divergence: first_awake_divergence(&deltas),
            samples,
        });
    }
    Ok(SceneEvidence {
        label: label.to_owned(),
        source_scenario_hash,
        neutral_projection_hash,
        source,
        authority,
        authority_exact_replay,
        candidates: candidate_evidence,
    })
}

fn run_awake_authority(
    arguments: &Arguments,
    staging: &Path,
    source: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
    checkpoints: &[u32],
) -> Result<Vec<AuthoritySample>, String> {
    checkpoints
        .iter()
        .map(|&completed_steps| {
            run_awake_oracle(
                arguments,
                staging,
                source,
                label,
                projection.clone(),
                completed_steps,
            )
        })
        .collect()
}

fn run_awake_oracle(
    arguments: &Arguments,
    staging: &Path,
    source: &OracleSourceIdentity,
    label: &str,
    mut projection: ScenarioV1,
    completed_steps: u32,
) -> Result<AuthoritySample, String> {
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
            source: source.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![OsString::from("--lane"), OsString::from("world-awake")],
            },
        },
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
    Ok(AuthoritySample {
        completed_steps,
        request_hash,
        artifact,
        result: oracle.result,
    })
}

fn structure(
    kind: SceneKind,
    scenario: &ScenarioV1,
    bodies: &[BodyStateV1],
    overflowed: bool,
) -> (bool, String) {
    match kind {
        SceneKind::Stack => centered_stack_structure(scenario, bodies, overflowed, 0.0),
        SceneKind::Pyramid => upright_pyramid_structure(scenario, bodies, overflowed),
        SceneKind::Bucket => {
            let valid = !overflowed
                && bodies.len() == scenario.bodies.len()
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

fn upright_pyramid_structure(
    scenario: &ScenarioV1,
    bodies: &[BodyStateV1],
    overflowed: bool,
) -> (bool, String) {
    if overflowed || bodies.len() != scenario.bodies.len() {
        return (false, "pyramid body or capacity contract failed".to_owned());
    }
    for body in bodies.iter().filter(|body| body.inverse_mass.get() > 0.0) {
        if !body.position.x.is_finite()
            || !body.position.y.is_finite()
            || body.position.y <= 0.0
            || body.angle.get().cos().abs() < body.angle.get().sin().abs()
            || !matches!(body.shape, ShapeV1::Rectangle { .. })
        {
            return (false, "a pyramid rectangle escaped or toppled".to_owned());
        }
    }
    (
        true,
        "all pyramid rectangles remain finite and upright".to_owned(),
    )
}

fn minimize_failure_scene_aliases(failures: &mut [FailureRecord]) {
    for index in 0..failures.len() {
        let candidate = failures[index].candidate;
        let prefix = failures[index]
            .scene
            .rsplit_once('-')
            .map_or(failures[index].scene.as_str(), |(prefix, _)| prefix);
        let smaller = failures
            .iter()
            .filter(|failure| failure.candidate == candidate && failure.scene.starts_with(prefix))
            .min_by_key(|failure| scene_size(&failure.scene))
            .map(|failure| failure.scene.clone());
        if let Some(smaller) = smaller {
            failures[index].minimized_scene = smaller;
        }
    }
}

fn scene_size(scene: &str) -> u32 {
    scene
        .rsplit_once('-')
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(u32::MAX)
}

fn performance_matrix(
    arguments: &Arguments,
    candidates: &[CandidateDefinition],
) -> Result<Vec<GpuPerformanceRow>, String> {
    let mut rows = Vec::new();
    for candidate in candidates {
        for bodies in [32_u32, 256, 1_024] {
            eprintln!("GPU performance {} bodies {bodies}", candidate.id);
            let mut scenario =
                p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
            scenario.settings = candidate.settings;
            let benchmark = pollster::block_on(
                physbox_engine::benchmark_gpu_generated_contact_scheduling_experiment(
                    &scenario,
                    arguments.warmup,
                    arguments.samples,
                    candidate.execution,
                    candidate.scheduling_policy,
                ),
            )?;
            let capacity_clean = benchmark.correctness.failure.is_none()
                && benchmark
                    .correctness
                    .capacities
                    .iter()
                    .all(|capacity| capacity.accepted && !capacity.overflowed);
            let correctness_hash = serialized_hash(&benchmark.correctness)?;
            let gpu_end_to_end_p50_seconds = benchmark
                .gpu_end_to_end_seconds
                .as_deref()
                .map(nearest_rank_p50)
                .transpose()?;
            rows.push(GpuPerformanceRow {
                candidate: candidate.id,
                bodies,
                source_scenario_hash: benchmark.source_scenario_hash,
                dispatches_per_step: benchmark.dispatches_per_step,
                allocated_bytes: benchmark.allocated_bytes,
                collision_persistence_allocated_bytes: benchmark
                    .collision_persistence_allocated_bytes,
                gpu_end_to_end_seconds: benchmark.gpu_end_to_end_seconds,
                gpu_end_to_end_p50_seconds,
                host_completion_seconds: benchmark.host_completion_seconds,
                cpu_encode_submit_seconds: benchmark.cpu_encode_submit_seconds,
                instrumented_solver_phases: benchmark.instrumented_solver_phases,
                coloring: benchmark.correctness.pass_evidence.coloring.clone(),
                capacity_clean,
                correctness_hash,
            });
        }
    }
    Ok(rows)
}

fn optimized_box2d_matrix(
    arguments: &Arguments,
    staging: &Path,
) -> Result<Vec<OptimizedBox2dRow>, String> {
    let mut rows = Vec::new();
    for bodies in [32_u32, 256, 1_024] {
        let scenario =
            p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
        let request = staging.join(format!("performance/box2d-scenario-{bodies}.json"));
        atomic_write(
            &request,
            &canonical_json(&scenario).map_err(|error| error.to_string())?,
        )?;
        for workers in [1_u32, 4] {
            let relative = format!("performance/box2d-{bodies}-workers-{workers}.json");
            let result_path = staging.join(&relative);
            let output = Command::new(&arguments.box2d_benchmark)
                .args([
                    "--request",
                    &request.to_string_lossy(),
                    "--result",
                    &result_path.to_string_lossy(),
                    "--run-id",
                    &format!("{}-box2d-{bodies}-w{workers}", arguments.run_id),
                    "--workers",
                    &workers.to_string(),
                    "--warmup",
                    &arguments.warmup.to_string(),
                    "--samples",
                    &arguments.samples.to_string(),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| error.to_string())?;
            if !output.status.success() {
                return Err(format!(
                    "optimized Box2D failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&result_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let samples_seconds = value["samplesSeconds"]
                .as_array()
                .ok_or_else(|| "optimized Box2D samples are missing".to_owned())?
                .iter()
                .map(|sample| {
                    sample
                        .as_f64()
                        .ok_or_else(|| "optimized Box2D sample is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(OptimizedBox2dRow {
                bodies,
                workers,
                artifact: relative,
                p50_seconds: nearest_rank_p50(&samples_seconds)?,
                samples_seconds,
                scope: json_string(&value, "scope")?,
                source_revision: json_string(&value, "sourceRevision")?,
                source_dirty: value["sourceDirty"].as_bool().unwrap_or(true),
                contact_count_min: value["contactCountMin"].as_u64().unwrap_or(u64::MAX),
                contact_count_max: value["contactCountMax"].as_u64().unwrap_or(u64::MAX),
                success_sentinel: String::from_utf8_lossy(&output.stdout)
                    .contains("PHYSBOX5_BOX2D_BENCHMARK_SUCCESS"),
            });
        }
    }
    Ok(rows)
}

fn nearest_rank_p50(samples: &[f64]) -> Result<f64, String> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0)
    {
        return Err("timing samples are empty or invalid".to_owned());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(sorted[(sorted.len() - 1) / 2])
}

fn json_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("optimized Box2D omitted {key}"))
}

fn render_playback(
    scenes: &[SceneEvidence],
    candidates: &[CandidateDefinition],
) -> Result<String, String> {
    let candidate_ids = [
        "point-jacobi-i1",
        "scalar-mass-split-i4",
        "manifold-block-i4",
        "block-mass-split-i4",
        "colored-in-place-i4",
    ];
    render_playback_for(
        scenes,
        candidates,
        &candidate_ids,
        "PhysBox 5 S5.4a synchronized parallel-normal factorial",
    )
}

fn render_colored_playback(
    scenes: &[SceneEvidence],
    candidates: &[CandidateDefinition],
) -> Result<String, String> {
    render_playback_for(
        scenes,
        candidates,
        &[
            "point-compact-i4",
            "manifold-forward-i2",
            "manifold-forward-i4",
            "manifold-alternating-i2",
            "manifold-alternating-i4",
        ],
        "PhysBox 5 S5.4b synchronized compact-coloring factorial",
    )
}

fn render_scheduling_playback(
    scenes: &[SceneEvidence],
    candidates: &[CandidateDefinition],
) -> Result<String, String> {
    render_playback_for(
        scenes,
        candidates,
        &[
            "forward-i2-full-per-color",
            "forward-i2-incremental-per-color",
            "forward-i2-full-island",
            "forward-i2-incremental-island",
        ],
        "PhysBox 5 S5.4b synchronized coloring-scheduling factorial",
    )
}

fn render_playback_for(
    scenes: &[SceneEvidence],
    candidates: &[CandidateDefinition],
    candidate_ids: &[&str],
    title: &str,
) -> Result<String, String> {
    for id in candidate_ids {
        if !candidates.iter().any(|candidate| candidate.id == *id) {
            return Err(format!("playback candidate {id} is missing"));
        }
    }
    let cell_width = 126.0;
    let cell_height = 78.0;
    let label_width = 180.0;
    let lanes = candidate_ids.len() + 1;
    let width = label_width + cell_width * PLAYBACK_STEPS.len() as f32 + 16.0;
    let height = 42.0 + scenes.len() as f32 * cell_height * lanes as f32;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font:10px monospace;fill:#dce8f2}} .plot{{fill:#132738;stroke:#355166}} .body{{fill:#62dce8;stroke:#d8ffff;stroke-width:.5}} .authority{{fill:#f3a85f;stroke:#ffe1ba;stroke-width:.5}} .fixed{{fill:#60798c}}</style><text x=\"12\" y=\"20\">{title}</text>"
    );
    let mut y = 34.0;
    for scene in scenes {
        let lane_data = candidate_ids
            .iter()
            .map(|id| {
                scene
                    .candidates
                    .iter()
                    .find(|candidate| candidate.candidate == *id)
                    .map(|candidate| (*id, &candidate.samples))
                    .ok_or_else(|| format!("scene {} omitted {id}", scene.label))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for lane in 0..lanes {
            let lane_name = if lane == 0 {
                "Box2D awake"
            } else {
                lane_data[lane - 1].0
            };
            svg.push_str(&format!(
                "<text x=\"8\" y=\"{}\">{} / {lane_name}</text>",
                y + 14.0,
                scene.label
            ));
            for (column, step) in PLAYBACK_STEPS.iter().enumerate() {
                let x = label_width + column as f32 * cell_width;
                let bodies = if lane == 0 {
                    &scene
                        .authority
                        .iter()
                        .find(|sample| sample.completed_steps == *step)
                        .ok_or_else(|| format!("authority omitted playback step {step}"))?
                        .result
                        .bodies
                } else {
                    &lane_data[lane - 1]
                        .1
                        .iter()
                        .find(|sample| sample.completed_steps == *step)
                        .ok_or_else(|| format!("candidate omitted playback step {step}"))?
                        .bodies
                };
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\">step {step}</text><rect class=\"plot\" x=\"{x}\" y=\"{}\" width=\"118\" height=\"60\"/>",
                    x + 2.0,
                    y + 10.0,
                    y + 14.0
                ));
                render_bodies(&mut svg, bodies, x, y + 14.0, lane == 0);
            }
            y += cell_height;
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

fn render_bodies(svg: &mut String, bodies: &[BodyStateV1], x: f32, y: f32, authority: bool) {
    let scale = 25.0;
    let center_x = x + 59.0;
    let floor_y = y + 52.0;
    for body in bodies {
        let class = if body.inverse_mass.get() == 0.0 {
            "fixed"
        } else if authority {
            "authority"
        } else {
            "body"
        };
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
    atomic_write(
        path,
        &serde_json::to_vec(value).map_err(|error| error.to_string())?,
    )
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| error.to_string())
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
    box2d_benchmark: PathBuf,
    warmup: u32,
    samples: u32,
    require_clean: bool,
    platforms: Vec<(String, PathBuf)>,
    physical_device_gap: Option<String>,
    colored_factorial: bool,
    scheduling_factorial: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut box2d_benchmark = None;
        let mut warmup = 2;
        let mut samples = 10;
        let mut require_clean = false;
        let mut platforms = Vec::new();
        let mut physical_device_gap = None;
        let mut colored_factorial = false;
        let mut scheduling_factorial = false;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "--require-clean" {
                require_clean = true;
                continue;
            }
            if argument == "--s5.4b" {
                colored_factorial = true;
                continue;
            }
            if argument == "--s5.4b-scheduling" {
                scheduling_factorial = true;
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
                "--box2d-benchmark" => box2d_benchmark = Some(PathBuf::from(value)),
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
        if colored_factorial && scheduling_factorial {
            return Err("choose only one S5.4b factorial mode".to_owned());
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            box2d_benchmark: box2d_benchmark
                .ok_or_else(|| "missing --box2d-benchmark".to_owned())?,
            warmup,
            samples,
            require_clean,
            platforms,
            physical_device_gap,
            colored_factorial,
            scheduling_factorial,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_matrix_covers_every_governed_question_without_changing_selected() {
        let source = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        let candidates = candidate_definitions(source.settings);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>(),
            vec![
                "point-jacobi-i1",
                "point-jacobi-i2",
                "point-jacobi-i4",
                "point-jacobi-i8",
                "scalar-mass-split-i1",
                "scalar-mass-split-i2",
                "scalar-mass-split-i4",
                "scalar-mass-split-i8",
                "manifold-block-i1",
                "manifold-block-i2",
                "manifold-block-i4",
                "manifold-block-i8",
                "block-mass-split-i1",
                "block-mass-split-i2",
                "block-mass-split-i4",
                "block-mass-split-i8",
                "colored-in-place-i1",
                "colored-in-place-i2",
                "colored-in-place-i4",
                "colored-in-place-i8",
            ]
        );
        assert_eq!(candidates[0].settings.iterations, 1);
        assert_eq!(
            candidates[0].execution,
            GpuSolverExecutionV4::experiment(true, true)
        );
        assert!(candidates.iter().all(|candidate| {
            candidate.settings.iterations == candidate.iterations
                && candidate.execution.warm_start
                && candidate.execution.relaxation
                && candidate.scheduling_policy == GpuSolverSchedulingPolicyV1::STANDARD
        }));
        assert!(candidates.iter().all(|candidate| {
            (candidate.family == CandidateFamilyV3::ColoredInPlace)
                == (candidate.execution.accumulation
                    == physbox_engine::GpuSolverAccumulationV4::ColoredInPlace)
        }));
        for family in [
            CandidateFamilyV3::PointJacobi,
            CandidateFamilyV3::ScalarMassSplit,
            CandidateFamilyV3::ManifoldBlock,
            CandidateFamilyV3::BlockMassSplit,
            CandidateFamilyV3::ColoredInPlace,
        ] {
            assert_eq!(
                candidates
                    .iter()
                    .filter(|candidate| candidate.family == family)
                    .map(|candidate| candidate.iterations)
                    .collect::<Vec<_>>(),
                vec![1, 2, 4, 8]
            );
        }
        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.question.is_empty())
        );
    }

    #[test]
    fn colored_factorial_retains_the_now_selected_forward_i2_lane() {
        let source = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        let candidates = colored_candidate_definitions(source.settings);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>(),
            vec![
                "point-fixed-i4",
                "point-compact-i4",
                "manifold-forward-i1",
                "manifold-forward-i2",
                "manifold-forward-i4",
                "manifold-alternating-i1",
                "manifold-alternating-i2",
                "manifold-alternating-i4",
            ]
        );
        assert!(candidates.iter().all(|candidate| {
            candidate.settings.iterations == candidate.iterations
                && candidate.execution.warm_start
                && candidate.execution.relaxation
                && candidate.scheduling_policy == GpuSolverSchedulingPolicyV1::STANDARD
        }));
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.execution == GpuSolverExecutionV4::SELECTED)
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>(),
            vec![
                "manifold-forward-i1",
                "manifold-forward-i2",
                "manifold-forward-i4"
            ]
        );
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| { candidate.family == CandidateFamilyV3::ManifoldColorForward })
                .map(|candidate| candidate.iterations)
                .collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| {
                    candidate.family == CandidateFamilyV3::ManifoldColorAlternating
                })
                .map(|candidate| candidate.iterations)
                .collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
    }

    #[test]
    fn scheduling_factorial_crosses_only_the_two_scheduling_dimensions() {
        let source = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        let candidates = scheduling_candidate_definitions(source.settings);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>(),
            vec![
                "forward-i2-full-per-color",
                "forward-i2-incremental-per-color",
                "forward-i2-full-island",
                "forward-i2-incremental-island",
            ]
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.scheduling_policy)
                .collect::<Vec<_>>(),
            vec![
                GpuSolverSchedulingPolicyV1::STANDARD,
                GpuSolverSchedulingPolicyV1::INCREMENTAL,
                GpuSolverSchedulingPolicyV1::WORKGROUP_PER_ISLAND,
                GpuSolverSchedulingPolicyV1::INCREMENTAL_WORKGROUP_PER_ISLAND,
            ]
        );
        assert!(candidates.iter().all(|candidate| {
            candidate.family == CandidateFamilyV3::ManifoldColorForward
                && candidate.iterations == 2
                && candidate.settings.iterations == 2
                && candidate.execution
                    == GpuSolverExecutionV4::manifold_colored_block_experiment(true, true)
                && !candidate.question.is_empty()
        }));
    }

    #[test]
    fn failure_aliases_choose_the_smallest_same_family_reproducer() {
        let scenario = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        let failure = |scene: &str| FailureRecord {
            candidate: "candidate",
            scene: scene.to_owned(),
            minimized_scene: scene.to_owned(),
            first_failure_step: 1,
            reason: "failure".to_owned(),
            reproducer: scenario.clone(),
            scenario_hash: "0".repeat(64),
            result_hash: "1".repeat(64),
            bodies: scenario.bodies.clone(),
        };
        let mut failures = vec![failure("rectangle-stack-8"), failure("rectangle-stack-4")];
        minimize_failure_scene_aliases(&mut failures);
        assert!(
            failures
                .iter()
                .all(|failure| failure.minimized_scene == "rectangle-stack-4")
        );
    }

    #[test]
    fn timing_and_pyramid_structure_are_deterministic() {
        assert_eq!(nearest_rank_p50(&[9.0, 1.0, 4.0, 3.0]).unwrap(), 3.0);
        let scenario = p5_2_rectangle_pyramid_scenario(2, 1).unwrap();
        assert!(upright_pyramid_structure(&scenario, &scenario.bodies, false).0);
        let mut toppled = scenario.bodies.clone();
        toppled[1].angle = physbox_contracts::FiniteF32::new(std::f32::consts::FRAC_PI_2).unwrap();
        assert!(!upright_pyramid_structure(&scenario, &toppled, false).0);
    }
}
