#![forbid(unsafe_code)]

mod body_benchmark;
mod body_floor_solver;
mod body_state;
mod body_world;
mod clock;
mod collision_candidates;
mod config;
mod frame;
mod gpu_context;
#[cfg(not(target_arch = "wasm32"))]
mod gpu_primitives;
mod gpu_solver;
mod host;
mod renderer;
mod report;
mod scenario;

#[cfg(not(target_arch = "wasm32"))]
pub use body_benchmark::benchmark_gpu_body_free_flight;
pub use body_benchmark::{
    BodyBenchmarkWork, BodyFinalValidation, BodyTimingSeries, GpuBodyBenchmarkOutput,
    GpuBodyBenchmarkSession, GpuTimestampStatus, PendingBodyValidation, PendingCompletion,
    PendingTimestampReadback,
};
pub use body_floor_solver::{GpuFixedFloorContactEvidence, GpuFixedFloorPassEvidence};
pub use body_state::{
    BODY_BYTES_PER_SLOT, BODY_COMMAND_BYTES, BODY_COMMAND_CAPACITY, BodySlotRegistry,
    BoundedBodyCommands, GpuBodyStorage, analytic_shape_encoding,
};
pub use clock::{InteractiveClockAction, PacingMode, PacingReport};
pub use collision_candidates::GpuCandidateOutput;
#[cfg(not(target_arch = "wasm32"))]
pub use collision_candidates::{GpuCandidateRunner, run_gpu_collision_candidates};
pub use config::EngineConfig;
pub use frame::{FrameInput, PointerButton, PointerEvent, PointerPhase, StepRequest};
pub use gpu_context::{AdapterDiagnostics, create_instance};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_primitives::{GpuPrimitiveBenchmarkOutput, GpuPrimitiveKind, benchmark_gpu_primitive};
pub use gpu_solver::{GpuContributionEvidence, GpuSolverCandidate, GpuSolverOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_solver::{
    GpuSolverBenchmarkOutput, benchmark_gpu_soft_step_case, run_gpu_soft_step_case,
    run_gpu_soft_step_stress_case, run_gpu_solver_case,
};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::OffscreenRenderer;
pub use renderer::SurfaceRenderer;
pub use report::{BodyCommandReport, FrameReport, ScenarioResult, SceneContract};
pub use scenario::{
    AnalyticFixtureKind, P5_2CandidatePattern, P5_2CollisionScenarioKind, ScenarioBuildError,
    analytic_fixture_scenario, authored_chain_scenario, authored_one_contact_scenario,
    p5_1_free_flight_scenario, p5_1_rectangle_floor_scenario, p5_1_reset_grab_scenario,
    p5_2_box2d_narrowphase_projection, p5_2_candidate_scenario, p5_2_collision_scenario,
};
