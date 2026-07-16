use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, OracleRequestV1, OracleResultV1, SCHEMA_VERSION, ScenarioV1,
    SettlingObservationV1, ShapeV1, SolverSettingsV1, Validate, canonical_hash, canonical_json,
    sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuNormalSolverV2, GpuSolverExecutionV2,
    p5_2_box2d_world_projection, p5_2_centered_rectangle_stack_scenario,
    p5_2_generated_performance_scenario, p5_2_rectangle_pyramid_scenario,
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
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateDefinition {
    id: &'static str,
    family: GpuNormalSolverV2,
    iterations: u32,
    settings: SolverSettingsV1,
    execution: GpuSolverExecutionV2,
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
    family: GpuNormalSolverV2,
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
    family: GpuNormalSolverV2,
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
                let output =
                    runner.run_experiment(&reproducer, first_failure_step, definition.execution)?;
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

fn candidate_definitions(base: SolverSettingsV1) -> Vec<CandidateDefinition> {
    let definitions = [
        (
            GpuNormalSolverV2::PointJacobi,
            "current parallel point solve baseline",
        ),
        (
            GpuNormalSolverV2::ScalarMassSplit,
            "does point-incidence mass splitting make convergence monotonic",
        ),
        (
            GpuNormalSolverV2::ManifoldBlock,
            "does coupled two-point normal solving remove the rocking mode",
        ),
        (
            GpuNormalSolverV2::BlockMassSplit,
            "does manifold incidence plus block solving resolve chain and rocking modes",
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
    ];
    let iterations = [1, 2, 4, 8];
    let mut candidates = Vec::with_capacity(16);
    for (family_index, (family, question)) in definitions.into_iter().enumerate() {
        for (iteration_index, iteration_count) in iterations.into_iter().enumerate() {
            let mut settings = base;
            settings.iterations = iteration_count;
            let execution = if family == GpuNormalSolverV2::PointJacobi && iteration_count == 1 {
                GpuSolverExecutionV2::SELECTED
            } else {
                GpuSolverExecutionV2::normal_experiment(family, true, true)
            };
            candidates.push(CandidateDefinition {
                id: ids[family_index][iteration_index],
                family,
                iterations: iteration_count,
                settings,
                execution,
                question,
            });
        }
    }
    candidates
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
        GpuNormalSolverV2::PointJacobi,
        GpuNormalSolverV2::ScalarMassSplit,
        GpuNormalSolverV2::ManifoldBlock,
        GpuNormalSolverV2::BlockMassSplit,
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
        let satisfies_conditional_exit = candidate.family != GpuNormalSolverV2::PointJacobi
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
        .filter(|candidate| candidate.satisfies_conditional_exit)
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
        version: 2,
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
) -> Result<SceneEvidence, String> {
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let projection = p5_2_box2d_world_projection(&source).map_err(|error| error.to_string())?;
    let neutral_projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let authority = run_awake_authority(arguments, staging, box2d, label, &projection)?;
    let authority_replay = run_awake_oracle(
        arguments,
        staging,
        box2d,
        &format!("{label}-replay"),
        projection.clone(),
        SETTLED_WINDOW_END,
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
        for (&completed_steps, authority_sample) in CHECKPOINTS.iter().zip(&authority) {
            let output = runner.run_experiment(&scenario, completed_steps, candidate.execution)?;
            let result_hash = serialized_hash(&output)?;
            let mut final_scenario = scenario.clone();
            final_scenario.bodies.clone_from(&output.bodies);
            let pairs = enumerate_scalar_candidates(&final_scenario)?;
            let manifolds = generate_scalar_narrowphase(&final_scenario, &pairs.candidates)?;
            let observation = observe_settling(
                &scenario,
                SettlingSnapshotInput {
                    completed_steps,
                    bodies: &output.bodies,
                    manifolds: &manifolds,
                    cache: &output.cache,
                    capacities: &output.capacities,
                    previous_manifolds: previous_manifolds.as_deref(),
                    previous_cache: previous_cache.as_deref(),
                    support_height: 0.0,
                },
            )?;
            let contact_balance = observe_contact_balance(completed_steps, &output.pass_evidence)?;
            let authority_contact_points = authority_sample
                .result
                .manifolds
                .iter()
                .map(|manifold| manifold.points.len() as u32)
                .sum();
            let authority_delta = compare_awake_authority(
                completed_steps,
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
                completed_steps,
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
        let pose_history = runner.run_experiment_body_history(
            &scenario,
            SETTLED_WINDOW_END,
            candidate.execution,
        )?;
        let first_structural_failure_step = pose_history
            .iter()
            .position(|bodies| !structure(kind, &scenario, bodies, false).0)
            .map(|step| step as u32 + 1);
        let preserves_structure_through_run = first_structural_failure_step.is_none();
        let replay = runner.run_experiment(&scenario, SETTLED_WINDOW_END, candidate.execution)?;
        let exact_replay = serialized_hash(&replay)? == final_sample.result_hash;
        candidate_evidence.push(CandidateSceneEvidence {
            candidate: candidate.id,
            execution_scenario_hash,
            exact_replay,
            structural_pass,
            structural_reason,
            preserves_structure_through_run,
            first_structural_failure_step,
            window: summarize_awake_window(
                &observations,
                SETTLED_WINDOW_START,
                SETTLED_WINDOW_END,
            )?,
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
) -> Result<Vec<AuthoritySample>, String> {
    CHECKPOINTS
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
            eprintln!("S5.4a performance {} bodies {bodies}", candidate.id);
            let mut scenario =
                p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
            scenario.settings = candidate.settings;
            let benchmark =
                pollster::block_on(physbox_engine::benchmark_gpu_generated_contact_experiment(
                    &scenario,
                    arguments.warmup,
                    arguments.samples,
                    candidate.execution,
                ))?;
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
    ];
    for id in candidate_ids {
        if !candidates.iter().any(|candidate| candidate.id == id) {
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
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font:10px monospace;fill:#dce8f2}} .plot{{fill:#132738;stroke:#355166}} .body{{fill:#62dce8;stroke:#d8ffff;stroke-width:.5}} .authority{{fill:#f3a85f;stroke:#ffe1ba;stroke-width:.5}} .fixed{{fill:#60798c}}</style><text x=\"12\" y=\"20\">PhysBox 5 S5.4a synchronized parallel-normal factorial</text>"
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
            ]
        );
        assert_eq!(candidates[0].settings, source.settings);
        assert_eq!(candidates[0].execution, GpuSolverExecutionV2::SELECTED);
        assert!(candidates.iter().all(|candidate| {
            candidate.settings.iterations == candidate.iterations
                && candidate.execution.normal_solver == candidate.family
                && candidate.execution.warm_start
                && candidate.execution.relaxation
        }));
        for family in [
            GpuNormalSolverV2::PointJacobi,
            GpuNormalSolverV2::ScalarMassSplit,
            GpuNormalSolverV2::ManifoldBlock,
            GpuNormalSolverV2::BlockMassSplit,
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
