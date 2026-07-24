#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "engine-internal"), allow(dead_code))]

//! GPU-resident two-dimensional physics without application or presentation
//! ownership.
//!
//! The caller owns the WGPU device, queue, command encoders, submission, and
//! render targets. PhysBox records mechanical work into that lifecycle and
//! exposes live state only through GPU views or explicitly requested
//! asynchronous observations.

extern crate self as physbox_sim;

mod authority_gpu_probe;
mod awake_points;
mod awake_work;
mod body_floor_solver;
mod body_lifecycle;
mod body_state;
mod body_world;
mod branch_robust_gpu_promotion;
mod broadphase_benchmark;
mod broadphase_sap;
mod clock;
mod collision_candidates;
mod collision_narrowphase;
mod collision_persistence;
mod config;
mod construction;
mod contact_components;
mod definition;
mod exact_sleep_grouping;
mod generated_contacts;
mod gpu_context;
mod gpu_solver;
mod gpu_timing;
mod grab;
mod pair_slot_persistence;
mod particle_world;
mod render_view;
mod sleep_contacts;
mod sleep_diagnostics;
#[cfg(test)]
mod test_support;
mod world;

pub(crate) use authority_gpu_probe::*;
pub(crate) use body_floor_solver::*;
pub(crate) use body_state::*;
pub(crate) use branch_robust_gpu_promotion::*;
pub(crate) use generated_contacts::*;
pub(crate) use gpu_context::*;
pub(crate) use particle_world::*;
#[cfg(test)]
pub(crate) use test_support::p5_1_rectangle_floor_scenario;

#[cfg(feature = "engine-internal")]
#[doc(hidden)]
pub mod __private {
    pub mod authority_gpu_probe {
        pub use crate::authority_gpu_probe::*;
    }

    pub mod awake_points {
        pub use crate::awake_points::*;
    }

    pub mod awake_work {
        pub use crate::awake_work::*;
    }

    pub mod body_lifecycle {
        pub use crate::body_lifecycle::*;
    }

    pub mod body_floor_solver {
        pub use crate::body_floor_solver::*;
    }

    pub mod body_state {
        pub use crate::body_state::*;
    }

    pub mod body_world {
        pub use crate::body_world::*;
    }

    pub mod branch_robust_gpu_promotion {
        pub use crate::branch_robust_gpu_promotion::*;
    }

    pub mod broadphase_benchmark {
        pub use crate::broadphase_benchmark::*;
    }

    pub mod broadphase_sap {
        pub use crate::broadphase_sap::*;
    }

    pub mod collision_candidates {
        pub use crate::collision_candidates::*;
    }

    pub mod collision_narrowphase {
        pub use crate::collision_narrowphase::*;
    }

    pub mod collision_persistence {
        pub use crate::collision_persistence::*;
    }

    pub mod construction {
        pub use crate::construction::*;
    }

    pub mod contact_components {
        pub use crate::contact_components::*;
    }

    pub mod exact_sleep_grouping {
        pub use crate::exact_sleep_grouping::*;
    }

    pub mod generated_contacts {
        pub use crate::generated_contacts::*;
    }

    pub mod gpu_context {
        pub use crate::gpu_context::*;
    }

    pub mod gpu_solver {
        pub use crate::gpu_solver::*;
    }

    pub mod gpu_timing {
        pub use crate::gpu_timing::*;
    }

    pub mod grab {
        pub use crate::grab::*;
    }

    pub mod pair_slot_persistence {
        pub use crate::pair_slot_persistence::*;
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
pub use world::{
    BodySnapshot, BodySnapshotEntry, SnapshotRequest, StepReport, World, WorldCounters, WorldError,
};
