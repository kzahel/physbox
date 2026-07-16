#![forbid(unsafe_code)]

mod body_benchmark;
mod body_floor_solver;
mod body_state;
mod body_world;
mod camera;
mod clock;
mod collision_candidates;
mod collision_narrowphase;
mod collision_persistence;
mod config;
mod construction;
mod construction_validation;
mod frame;
mod generated_contacts;
mod gpu_context;
#[cfg(not(target_arch = "wasm32"))]
mod gpu_primitives;
mod gpu_solver;
mod grab;
mod host;
mod input;
mod pair_slot_persistence;
mod renderer;
mod report;
mod scenario;
mod ui;
mod ui_font;

#[cfg(not(target_arch = "wasm32"))]
pub use body_benchmark::benchmark_gpu_body_free_flight;
pub use body_benchmark::{
    BODY_BENCHMARK_WORK_VERSION, BodyBenchmarkWork, BodyFinalValidation, BodyTimingSeries,
    GpuBodyBenchmarkOutput, GpuBodyBenchmarkSession, GpuTimestampStatus, PendingBodyValidation,
    PendingCompletion, PendingTimestampReadback,
};
pub use body_floor_solver::{
    GPU_SOLVER_EXECUTION_VERSION, GPU_SOLVER_SCHEDULING_POLICY_VERSION, GpuBlockActiveSetV2,
    GpuColorMaintenanceV1, GpuColorOrderV4, GpuColorScheduleV4, GpuColorUnitV4,
    GpuColoredDispatchV1, GpuColoringEvidenceV4, GpuFixedFloorContactEvidence,
    GpuFixedFloorPassEvidence, GpuGatherTopology, GpuGatherTopologyEvidence,
    GpuManifoldBlockEvidenceV2, GpuNormalSolverV4, GpuSolverAccumulationV4, GpuSolverExecutionV4,
    GpuSolverSchedulingPolicyV1,
};
#[cfg(not(target_arch = "wasm32"))]
pub use body_floor_solver::{
    GpuAuthoredGatherBenchmarkOutput, GpuSolverInstrumentedSampleV4,
    benchmark_gpu_authored_gather_topology,
};
pub use body_state::{
    BODY_BYTES_PER_SLOT, BODY_COMMAND_BYTES, BODY_COMMAND_CAPACITY, BodySlotRegistry,
    BoundedBodyCommands, GpuBodyStorage, analytic_shape_encoding,
};
pub use body_world::GpuConstructionBodySlotEvidence;
pub use camera::{
    CAMERA_STATE_VERSION, Camera2d, CameraError, CameraFrame2d, MAX_ORTHOGRAPHIC_HALF_HEIGHT,
    MIN_ORTHOGRAPHIC_HALF_HEIGHT,
};
pub use clock::{InteractiveClockAction, PacingMode, PacingReport};
pub use collision_candidates::GpuCandidateOutput;
#[cfg(not(target_arch = "wasm32"))]
pub use collision_candidates::{GpuCandidateRunner, run_gpu_collision_candidates};
#[cfg(not(target_arch = "wasm32"))]
pub use collision_narrowphase::{GpuNarrowphaseOutput, GpuNarrowphaseRunner};
#[cfg(not(target_arch = "wasm32"))]
pub use collision_persistence::{
    GpuPersistenceOutput, GpuPersistenceRunner, GpuPersistenceStepOutput,
};
pub use config::EngineConfig;
pub use construction::{
    BoundedConstructionIntents, CONSTRUCTION_CIRCLE_RADIUS, CONSTRUCTION_FRICTION,
    CONSTRUCTION_INTENT_BYTES, CONSTRUCTION_INTENT_CAPACITY, CONSTRUCTION_RECTANGLE_HALF_EXTENTS,
    CONSTRUCTION_RESTITUTION, CONSTRUCTION_STATE_BYTES, construction_state_from_bytes,
};
pub use construction_validation::{
    CONSTRUCTION_VALIDATION_SCRIPT_VERSION, ConstructionValidationFrameV1,
    ConstructionValidationScriptV1, construction_capacity_validation_script,
    construction_deep_overlap_validation_script, construction_interaction_validation_script,
    construction_invalidation_validation_script,
};
pub use frame::{
    FrameInput, InputEvent, PointerButton, PointerEvent, PointerPhase, StepRequest, WheelEvent,
};
#[cfg(not(target_arch = "wasm32"))]
pub use generated_contacts::{
    GPU_GENERATED_CONTACT_BODY_HISTORY_VERSION, GpuGeneratedContactBenchmarkOutput,
    GpuGeneratedContactBodyHistoryV1, GpuGeneratedContactOutput, GpuGeneratedContactRunner,
    benchmark_gpu_generated_contact_experiment,
    benchmark_gpu_generated_contact_scheduling_experiment, benchmark_gpu_generated_contacts,
    benchmark_gpu_generated_contacts_with_gather_topology,
};
pub use generated_contacts::{GpuConstructionGraphEvidence, GpuContactPersistenceTopology};
pub use gpu_context::{AdapterDiagnostics, create_instance};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_primitives::{GpuPrimitiveBenchmarkOutput, GpuPrimitiveKind, benchmark_gpu_primitive};
pub use gpu_solver::{GpuContributionEvidence, GpuSolverCandidate, GpuSolverOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_solver::{
    GpuSolverBenchmarkOutput, benchmark_gpu_soft_step_case, run_gpu_soft_step_case,
    run_gpu_soft_step_stress_case, run_gpu_solver_case,
};
pub use grab::{GrabValidationState, provisional_grab_tuning};
pub use input::{INPUT_TRACE_VERSION, InputTraceError, InputTraceFrameV1, InputTraceV1};
#[cfg(not(target_arch = "wasm32"))]
pub use pair_slot_persistence::{
    GpuPairSlotPersistenceBenchmarkOutput, GpuPairSlotPersistenceOutput,
    GpuPairSlotPersistenceRunner, GpuPairSlotPersistenceStepOutput,
    benchmark_gpu_pair_slot_persistence,
};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::OffscreenRenderer;
pub use renderer::SurfaceRenderer;
pub use report::{
    BodyCommandReport, CameraReport, ConstructionDiagnosticReport, FrameReport, ScenarioResult,
    SceneContract, UiReport,
};
pub use scenario::{
    AnalyticFixtureKind, P5_2_BUCKET_EVIDENCE_STEPS, P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    P5_2_COLORING_CHURN_VERSION, P5_2_MANIFOLD_COLOR_VALIDATION_STEPS, P5_2BucketShape,
    P5_2CandidatePattern, P5_2CollisionScenarioKind, P5_2ColoringChurnSnapshotV1,
    P5_2GeneratedScenarioKind, ScenarioBuildError, analytic_fixture_scenario,
    authored_chain_scenario, authored_one_contact_scenario, p5_1_free_flight_scenario,
    p5_1_rectangle_floor_scenario, p5_1_reset_grab_scenario, p5_2_box2d_narrowphase_projection,
    p5_2_box2d_world_projection, p5_2_bucket_scenario, p5_2_candidate_scenario,
    p5_2_centered_rectangle_stack_scenario, p5_2_collision_scenario, p5_2_coloring_churn_snapshots,
    p5_2_contact_policy_candidate, p5_2_generated_floor_scenario,
    p5_2_generated_performance_scenario, p5_2_generated_reset_grab_scenario,
    p5_2_generated_scenario, p5_2_manifold_color_validation_scenario,
    p5_2_narrowphase_pair_scenario, p5_2_rectangle_pyramid_scenario,
};
