#![forbid(unsafe_code)]
#![cfg_attr(
    all(feature = "solver-lab", target_arch = "wasm32"),
    allow(dead_code, unused_imports)
)]

mod authority_gpu_probe;
mod awake_points;
mod awake_work;
mod body_benchmark;
mod body_floor_solver;
mod body_lifecycle;
mod body_state;
mod body_world;
mod branch_robust_gpu_promotion;
mod broadphase_benchmark;
#[cfg(all(feature = "solver-lab", target_arch = "wasm32"))]
mod broadphase_browser;
mod broadphase_grid;
mod broadphase_sap;
mod camera;
mod clock;
mod collision_candidates;
mod collision_narrowphase;
mod collision_persistence;
#[cfg(feature = "box2d-comparator")]
mod comparison;
mod config;
mod construction;
mod construction_validation;
#[cfg(any(test, feature = "solver-lab"))]
mod contact_components;
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
mod coupled_evidence;
mod coupled_scenario;
mod engine_selection;
mod exact_sleep_grouping;
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
mod fluid_solid_overlap_evidence;
mod frame;
mod generated_contacts;
mod gpu_context;
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
mod gpu_primitives;
mod gpu_solver;
mod gpu_timing;
mod grab;
mod host;
mod input;
mod pair_slot_persistence;
mod particle_world;
mod physics_render;
mod renderer;
mod report;
mod scenario;
mod sleep_contacts;
mod sleep_diagnostics;
mod solver_profile;
mod ui;

#[cfg(feature = "solver-lab")]
pub use authority_gpu_probe::{
    AUTHORITY_GPU_COLOR_COUNT, AUTHORITY_GPU_DYNAMIC_COLOR_COUNT,
    AUTHORITY_GPU_OVERFLOW_COLOR_COUNT, AUTHORITY_GPU_PROBE_CANDIDATE_ID,
    AUTHORITY_GPU_PROBE_POLICY_VERSION, AUTHORITY_GPU_STATIC_COLOR_COUNT,
    AUTHORITY_GPU_TRANSFORM_STORAGE_VERSION, AuthorityGpuProbePolicyV1,
    SelectedGpuSolverPolicySnapshotV1,
};
#[cfg(not(target_arch = "wasm32"))]
pub use body_benchmark::benchmark_gpu_body_free_flight;
pub use body_benchmark::{
    BODY_BENCHMARK_WORK_VERSION, BodyBenchmarkWork, BodyFinalValidation, BodyTimingSeries,
    GpuBodyBenchmarkOutput, GpuBodyBenchmarkSession, GpuTimestampStatus, PendingBodyValidation,
    PendingCompletion, PendingTimestampReadback,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use body_floor_solver::{
    GPU_SCHEDULER_WORK_VECTOR_VERSION, GpuAuthoredAuthorityProbeOutputV1,
    GpuAuthoredGatherBenchmarkOutput, GpuAuthorityProbeCapacityDispositionV1,
    GpuS13AuthorityProbeBenchmarkV1, GpuS13MeasuredConfigurationV1, GpuSchedulerWorkVectorV1,
    GpuSolverInstrumentedSampleV4, authority_gpu_probe_capacity_disposition,
    benchmark_gpu_authored_gather_topology, benchmark_gpu_authored_solver_experiment,
    benchmark_gpu_authored_solver_policy_experiment, benchmark_gpu_s13_authority_probe,
    benchmark_gpu_s17_branch_robust_promotion, branch_robust_gpu_promotion_capacity_disposition,
    run_gpu_authored_authority_probe, run_gpu_authored_branch_robust_promotion,
};
pub use body_floor_solver::{
    GPU_SOLVER_EXECUTION_VERSION, GPU_SOLVER_PASS_TRACE_VERSION,
    GPU_SOLVER_SCHEDULING_POLICY_VERSION, GPU_STATIC_LATE_COLOR_COUNT, GPU_STATIC_LATE_FIRST_COLOR,
    GpuBlockActiveSetV2, GpuColorMaintenanceV1, GpuColorOrderV4, GpuColorScheduleV4,
    GpuColorUnitV4, GpuColoredDispatchV1, GpuColoringEvidenceV4, GpuContactCoefficientPolicyV1,
    GpuContactSoftnessCoefficientsV1, GpuFixedFloorContactEvidence, GpuFixedFloorPassEvidence,
    GpuGatherTopology, GpuGatherTopologyEvidence, GpuManifoldBlockEvidenceV2,
    GpuNormalRelaxationPolicyV1, GpuNormalSolverV4, GpuRestitutionTimingPolicyV1,
    GpuSoftFeedbackPolicyV1, GpuSolverAccumulationV4, GpuSolverExecutionV4,
    GpuSolverPassDescriptorV1, GpuSolverPassKindV1, GpuSolverPassParametersV1,
    GpuSolverSchedulingPolicyV1, GpuSubstepContactPolicyV1, gpu_contact_softness_coefficients,
};
pub use body_state::{
    BODY_BYTES_PER_SLOT, BODY_COMMAND_BYTES, BODY_COMMAND_CAPACITY, BodySlotRegistry,
    BoundedBodyCommands, GpuBodyStorage, analytic_shape_encoding,
};
pub use body_world::GpuConstructionBodySlotEvidence;
#[cfg(feature = "solver-lab")]
pub use branch_robust_gpu_promotion::{
    BRANCH_ROBUST_GPU_PROMOTION_CANDIDATE_ID, BRANCH_ROBUST_GPU_PROMOTION_FORMULA_ID,
    BRANCH_ROBUST_GPU_PROMOTION_POLICY_VERSION, BranchRobustGpuPromotionPolicyV1,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use broadphase_benchmark::GpuBroadphaseTimingV1;
#[cfg(all(feature = "solver-lab", target_arch = "wasm32"))]
pub use broadphase_browser::{
    GpuBroadphaseBrowserCompletion, GpuBroadphaseBrowserProbeV1, GpuBroadphaseBrowserSession,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use broadphase_grid::{
    GpuGridConfigurationV1, GpuMultiCellGridBenchmarkOutput, GpuMultiCellGridOutput,
    GpuMultiCellGridRunner,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use broadphase_sap::{
    GpuSweepAndPruneBenchmarkOutput, GpuSweepAndPruneOutput, GpuSweepAndPruneRunner,
};
pub use camera::{
    CAMERA_STATE_VERSION, Camera2d, CameraError, CameraFrame2d, MAX_ORTHOGRAPHIC_HALF_HEIGHT,
    MIN_ORTHOGRAPHIC_HALF_HEIGHT,
};
pub use clock::{InteractiveClockAction, PacingMode, PacingReport};
pub use collision_candidates::GpuCandidateOutput;
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use collision_candidates::{
    GpuCandidateBenchmarkOutput, GpuCandidateRunner, run_gpu_collision_candidates,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use collision_narrowphase::{GpuNarrowphaseOutput, GpuNarrowphaseRunner};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use collision_persistence::{
    GpuPersistenceOutput, GpuPersistenceRunner, GpuPersistenceStepOutput,
};
pub use config::EngineConfig;
pub use construction::{
    BoundedConstructionIntents, CONSTRUCTION_CIRCLE_INVERSE_INERTIA,
    CONSTRUCTION_CIRCLE_INVERSE_MASS, CONSTRUCTION_CIRCLE_MASS, CONSTRUCTION_CIRCLE_RADIUS,
    CONSTRUCTION_DENSITY, CONSTRUCTION_FRICTION, CONSTRUCTION_INTENT_BYTES,
    CONSTRUCTION_INTENT_CAPACITY, CONSTRUCTION_RECTANGLE_HALF_EXTENTS,
    CONSTRUCTION_RECTANGLE_INVERSE_INERTIA, CONSTRUCTION_RECTANGLE_INVERSE_MASS,
    CONSTRUCTION_RECTANGLE_MASS, CONSTRUCTION_RESTITUTION, CONSTRUCTION_STATE_BYTES,
    construction_state_from_bytes,
};
pub use construction_validation::{
    CONSTRUCTION_VALIDATION_SCRIPT_VERSION, ConstructionValidationFrameV1,
    ConstructionValidationScriptV1, construction_capacity_validation_script,
    construction_deep_overlap_validation_script, construction_interaction_validation_script,
    construction_invalidation_validation_script,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use contact_components::{
    GpuContactComponentObservationV1, GpuContactComponentStatsV1, GpuSleepDecisionObservationV1,
    GpuSleepDecisionStepV1, GpuSleepDecisionTimingV1, benchmark_gpu_sleep_decision_observer,
    observe_gpu_contact_components, observe_gpu_sleep_decisions,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use coupled_evidence::{
    GPU_COUPLED_EVIDENCE_VERSION, GpuCoupledBodyObservationV1, GpuCoupledBodyTrajectoryV1,
    GpuCoupledEvidenceConfigV1, GpuCoupledEvidenceRun, GpuCoupledEvidenceV2, GpuCoupledExchangeV1,
    GpuCoupledMechanicalStateV1, GpuCoupledTimingSummaryV1, GpuCoupledTimingV1,
    GpuParticleAggregateV1, run_gpu_coupled_evidence,
};
pub use coupled_scenario::{
    COUPLED_CANONICAL_SOURCE_COUNT, COUPLED_CANONICAL_SOURCE_HASHES_V1, CoupledScenarioKind,
    FLUID_SOLID_OVERLAP_PHASE_COUNT, FLUID_SOLID_OVERLAP_PHASE_OFFSETS,
    FLUID_SOLID_OVERLAP_RECOVERY_STEPS, FLUID_SOLID_OVERLAP_SAMPLE_STRIDE,
    FLUID_SOLID_OVERLAP_SETTLE_STEPS, FLUID_SOLID_OVERLAP_SOURCE_VERSION,
    FluidSolidOverlapArrangement, FluidSolidOverlapSourceV1, canonical_coupled_scenario,
    canonical_coupled_scenario_hashes, coupled_capacity_scenario, fluid_solid_overlap_source,
    fluid_solid_overlap_sources, liquid_sandbox_scenario,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use fluid_solid_overlap_evidence::{
    FLUID_SOLID_OVERLAP_TRIALS_PER_MEDIUM, FLUID_SOLID_SIGNIFICANT_OVERLAP,
    FluidSolidOverlapMedium, GPU_FLUID_SOLID_OVERLAP_EVIDENCE_VERSION,
    GpuFluidSolidMotionMetricsV1, GpuFluidSolidOverlapEvidenceV1, GpuFluidSolidOverlapRowV1,
    GpuFluidSolidPairMetricsV1, GpuPostFluidCandidateAuditSampleV1, GpuPostFluidCandidateAuditV1,
    GpuRigidContactDiagnosticsV1, run_gpu_fluid_solid_overlap_evidence,
    run_gpu_fluid_solid_overlap_standard_evidence,
};
pub use frame::{
    FrameInput, InputEvent, PointerButton, PointerEvent, PointerPhase, StepRequest, WheelEvent,
};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use generated_contacts::{
    GPU_AUTHORITY_SEPARATION_DIAGNOSTIC_VERSION, GPU_GENERATED_CONTACT_BODY_HISTORY_VERSION,
    GPU_GENERATED_CONTACT_PASS_MOTION_WINDOW_VERSION, GPU_GENERATED_CONTACT_PASS_TRACE_VERSION,
    GpuAuthoritySeparationDiagnosticV1, GpuAuthoritySeparationInputV1,
    GpuAuthoritySeparationValueV1, GpuGeneratedContactBenchmarkOutput,
    GpuGeneratedContactBodyHistoryV1, GpuGeneratedContactOutput,
    GpuGeneratedContactPassMotionStepV1, GpuGeneratedContactPassMotionWindowV1,
    GpuGeneratedContactPassStateV3, GpuGeneratedContactPassTraceV3, GpuGeneratedContactRunner,
    GpuGeneratedContactSeriesV1, GpuGeneratedContactSnapshotV1, GpuPassTraceDispatchV1,
    benchmark_gpu_generated_contact_experiment,
    benchmark_gpu_generated_contact_identity_experiment,
    benchmark_gpu_generated_contact_scheduling_experiment, benchmark_gpu_generated_contacts,
    benchmark_gpu_generated_contacts_with_gather_topology, benchmark_gpu_sleeping_city,
    benchmark_gpu_sleeping_city_with_grouping, benchmark_gpu_sparse_capacity,
};
pub use generated_contacts::{GpuConstructionGraphEvidence, GpuContactPersistenceTopology};
pub use gpu_context::{AdapterDiagnostics, create_instance};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use gpu_primitives::{GpuPrimitiveBenchmarkOutput, GpuPrimitiveKind, benchmark_gpu_primitive};
pub use gpu_solver::{GpuContributionEvidence, GpuSolverCandidate, GpuSolverOutput};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use gpu_solver::{
    GpuSolverBenchmarkOutput, benchmark_gpu_soft_step_case, run_gpu_soft_step_case,
    run_gpu_soft_step_stress_case, run_gpu_solver_case,
};
pub use grab::{GrabValidationState, provisional_grab_tuning};
pub use input::{INPUT_TRACE_VERSION, InputTraceError, InputTraceFrameV1, InputTraceV1};
#[cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]
pub use pair_slot_persistence::{
    GpuPairSlotPersistenceBenchmarkOutput, GpuPairSlotPersistenceOutput,
    GpuPairSlotPersistenceRunner, GpuPairSlotPersistenceStepOutput,
    benchmark_gpu_pair_slot_persistence,
};
pub use particle_world::{GPU_PARTICLE_DIAGNOSTICS_VERSION, GpuParticleDiagnosticsV4};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::OffscreenRenderer;
pub use renderer::{DisplayMetrics, SurfaceRenderer};
pub use report::{
    BodyCommandReport, CameraReport, ConstructionDiagnosticReport, FrameReport, FrameTimingReport,
    RenderScaleReport, ScenarioResult, SceneContract, UiReport,
};
pub use scenario::{
    AnalyticFixtureKind, P5_2_BUCKET_EVIDENCE_STEPS, P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    P5_2_COLORING_CHURN_VERSION, P5_2_MANIFOLD_COLOR_VALIDATION_STEPS,
    P5_2_SELECTED_SOLVER_ITERATIONS, P5_2_SETTLING_EVIDENCE_STEPS, P5_2BucketShape,
    P5_2CandidatePattern, P5_2CollisionScenarioKind, P5_2ColoringChurnSnapshotV1,
    P5_2GeneratedScenarioKind, P5_2SettlingFixtureKind, P5_2SettlingReviewPolicy,
    P5_3BroadphasePattern, ScenarioBuildError, analytic_fixture_scenario, authored_chain_scenario,
    authored_one_contact_scenario, p5_1_free_flight_scenario, p5_1_rectangle_floor_scenario,
    p5_1_reset_grab_scenario, p5_2_box2d_narrowphase_projection, p5_2_box2d_world_projection,
    p5_2_bucket_scenario, p5_2_candidate_scenario, p5_2_centered_rectangle_stack_scenario,
    p5_2_collision_scenario, p5_2_coloring_churn_snapshots, p5_2_contact_policy_candidate,
    p5_2_generated_floor_scenario, p5_2_generated_performance_scenario,
    p5_2_generated_reset_grab_scenario, p5_2_generated_scenario,
    p5_2_manifold_color_validation_scenario, p5_2_minimized_mixed_pile_energy_scenario,
    p5_2_minimized_mixed_pile_hard_scenario, p5_2_minimized_mixed_pile_soft_i1_scenario,
    p5_2_narrowphase_pair_scenario, p5_2_random_settling_fixture_scenario,
    p5_2_rectangle_pyramid_scenario, p5_2_s11_soft_composition_reproducer_scenario,
    p5_2_s11_soft_depth_sweep_scenario, p5_2_s13_authority_measurement_scenario,
    p5_2_s13_authority_overflow_scenario, p5_2_settling_fixture_scenario,
    p5_2_settling_review_scenario, p5_2_settling_soft_fixture_scenario,
    p5_2_settling_soft_i1_comparator_scenario, p5_2_settling_soft_stack_matrix_scenario,
    p5_2_sleeping_city_scenario, p5_2_stale_reseed_reproducer_scenario, p5_3_broadphase_scenario,
    p5_3_broadphase_snapshot, s26_bridge_components_scenario, s26_isolated_components_scenario,
    s26_packed_pile_scenario, s27_long_path_wake_scenario, s27_poured_pile_scenario,
};
pub use sleep_diagnostics::{
    SLEEP_CYCLE_DIAGNOSTIC_BYTES, SLEEP_CYCLE_DIAGNOSTIC_VERSION, SleepCycleMetricsV1,
};
pub use solver_profile::SolverProfileConfig;
