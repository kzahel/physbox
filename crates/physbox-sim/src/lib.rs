#![forbid(unsafe_code)]

//! GPU-resident two-dimensional physics without application or presentation
//! ownership.
//!
//! The caller owns the WGPU device, queue, command encoders, submission, and
//! render targets. PhysBox records mechanical work into that lifecycle and
//! exposes live state only through GPU views or explicitly requested
//! asynchronous observations.

mod clock;
mod config;
mod definition;

pub use clock::{
    DEFAULT_FIXED_STEP_HZ, DEFAULT_FIXED_STEP_SECONDS, InteractiveClockAction,
    InteractiveClockInput, MAX_ACTIVE_WALL_SECONDS, PacingMode, PacingReport, SimulationClock,
};
pub use config::{
    ConfigError, FluidSolidCouplingQuality, MAX_BIASED_ITERATIONS, MAX_CONTACT_DAMPING_RATIO,
    MAX_CONTACT_HZ, MAX_FIXED_STEP_HZ, MAX_MAX_DEPENETRATION_VELOCITY, MAX_SLEEP_SPEED_THRESHOLD,
    MAX_SOLVER_SUBSTEPS, MAX_SPECULATIVE_DISTANCE, MAX_STATIC_CONTACT_HZ_MULTIPLIER,
    MIN_BIASED_ITERATIONS, MIN_CONTACT_DAMPING_RATIO, MIN_CONTACT_HZ, MIN_FIXED_STEP_HZ,
    MIN_MAX_DEPENETRATION_VELOCITY, MIN_SLEEP_SPEED_THRESHOLD, MIN_SOLVER_SUBSTEPS,
    MIN_SPECULATIVE_DISTANCE, MIN_STATIC_CONTACT_HZ_MULTIPLIER, PHYSICS_CONFIGURATION_VERSION,
    PhysicsConfigurationV3, QualityDef, SleepGrouping, SolverProfileName, WorldCapacity,
};
pub use definition::{
    BodyDef, BodyId, BodyType, Circle, DefinitionError, MaterialDef, Rectangle, ShapeDef,
    ShapeGeometry, ShapeId, StepDef, WorldDef,
};
