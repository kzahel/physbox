#![forbid(unsafe_code)]

mod config;
mod frame;
mod gpu_context;
mod host;
mod renderer;
mod report;

pub use config::EngineConfig;
pub use frame::{FrameInput, PointerButton, PointerEvent, PointerPhase};
pub use gpu_context::{AdapterDiagnostics, create_instance};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::OffscreenRenderer;
pub use renderer::SurfaceRenderer;
pub use report::{FrameReport, ScenarioResult, SceneContract};
