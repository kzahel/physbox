#![forbid(unsafe_code)]

//! GPU-resident two-dimensional physics without application or presentation
//! ownership.
//!
//! The caller owns the WGPU device, queue, command encoders, submission, and
//! render targets. PhysBox records mechanical work into that lifecycle and
//! exposes live state only through GPU views or explicitly requested
//! asynchronous observations.

extern crate self as physbox_sim;

mod awake_work;
mod body_lifecycle;
mod body_state;
mod clock;
mod config;
mod construction;
mod definition;
mod exact_sleep_grouping;
mod grab;
mod particle_world;
mod render_view;
mod sleep_contacts;
mod sleep_diagnostics;
#[cfg(test)]
mod test_support;

pub(crate) use body_state::GpuBodyStorage;
#[cfg(test)]
pub(crate) use test_support::{create_instance, gpu_context, p5_1_rectangle_floor_scenario};

#[doc(hidden)]
pub mod __private {
    pub mod awake_work {
        pub use crate::awake_work::*;
    }

    pub mod body_lifecycle {
        pub use crate::body_lifecycle::*;
    }

    pub mod body_state {
        pub use crate::body_state::*;
    }

    pub mod construction {
        pub use crate::construction::*;
    }

    pub mod exact_sleep_grouping {
        pub use crate::exact_sleep_grouping::*;
    }

    pub mod grab {
        pub use crate::grab::*;
    }

    pub mod particle_world {
        pub use crate::particle_world::*;
    }

    pub mod sleep_contacts {
        pub use crate::sleep_contacts::*;
    }

    pub mod sleep_diagnostics {
        pub use crate::sleep_diagnostics::*;
    }
}

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
pub use render_view::{ParticleRenderView, RENDER_VIEW_VERSION, RenderView};
