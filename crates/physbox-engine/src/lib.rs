#![forbid(unsafe_code)]

mod clock;
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

pub use clock::{InteractiveClockAction, PacingMode, PacingReport};
pub use config::EngineConfig;
pub use frame::{FrameInput, PointerButton, PointerEvent, PointerPhase, StepRequest};
pub use gpu_context::{AdapterDiagnostics, create_instance};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_primitives::{GpuPrimitiveBenchmarkOutput, GpuPrimitiveKind, benchmark_gpu_primitive};
pub use gpu_solver::{GpuContributionEvidence, GpuSolverCandidate, GpuSolverOutput};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu_solver::{
    GpuSolverBenchmarkOutput, benchmark_gpu_soft_step_case, run_gpu_soft_step_case,
    run_gpu_solver_case,
};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::OffscreenRenderer;
pub use renderer::SurfaceRenderer;
pub use report::{FrameReport, ScenarioResult, SceneContract};
pub use scenario::{
    AnalyticFixtureKind, ScenarioBuildError, analytic_fixture_scenario, authored_chain_scenario,
    authored_one_contact_scenario,
};
