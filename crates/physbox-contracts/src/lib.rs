#![forbid(unsafe_code)]

mod collision;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: u32 = 1;
pub const BODY_LAYOUT_VERSION: u32 = 1;
pub const ORACLE_SUCCESS_SENTINEL: &str = "PHYSBOX5_ORACLE_SUCCESS";

pub use collision::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractError(pub String);

impl Display for ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ContractError {}

pub trait Validate {
    fn validate(&self) -> Result<(), ContractError>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Validate for Vec2 {
    fn validate(&self) -> Result<(), ContractError> {
        finite_f32("vec2.x", self.x)?;
        finite_f32("vec2.y", self.y)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ShapeV1 {
    Circle { radius: FiniteF32 },
    Rectangle { half_extents: Vec2 },
}

impl Validate for ShapeV1 {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Circle { radius } if radius.get() > 0.0 => Ok(()),
            Self::Circle { .. } => Err(ContractError("circle radius must be positive".to_owned())),
            Self::Rectangle { half_extents } => {
                half_extents.validate()?;
                if half_extents.x > 0.0 && half_extents.y > 0.0 {
                    Ok(())
                } else {
                    Err(ContractError(
                        "rectangle half extents must be positive".to_owned(),
                    ))
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FiniteF32(f32);

impl FiniteF32 {
    pub fn new(value: f32) -> Result<Self, ContractError> {
        finite_f32("value", value)?;
        Ok(Self(value))
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl Validate for FiniteF32 {
    fn validate(&self) -> Result<(), ContractError> {
        finite_f32("value", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialV1 {
    pub friction: FiniteF32,
    pub restitution: FiniteF32,
}

impl Validate for MaterialV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.friction.validate()?;
        self.restitution.validate()?;
        if self.friction.get() < 0.0 {
            return Err(ContractError("friction must be nonnegative".to_owned()));
        }
        if !(0.0..=1.0).contains(&self.restitution.get()) {
            return Err(ContractError(
                "restitution must be between zero and one".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyStateV1 {
    pub id: BodyId,
    pub shape: ShapeV1,
    pub position: Vec2,
    pub angle: FiniteF32,
    pub linear_velocity: Vec2,
    pub angular_velocity: FiniteF32,
    pub inverse_mass: FiniteF32,
    pub inverse_inertia: FiniteF32,
    pub material: MaterialV1,
    pub sleeping: bool,
}

impl Validate for BodyStateV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.id.generation == 0 {
            return Err(ContractError(
                "body state requires a nonzero generation".to_owned(),
            ));
        }
        self.shape.validate()?;
        self.position.validate()?;
        self.angle.validate()?;
        self.linear_velocity.validate()?;
        self.angular_velocity.validate()?;
        self.inverse_mass.validate()?;
        self.inverse_inertia.validate()?;
        self.material.validate()?;
        if self.inverse_mass.get() < 0.0 || self.inverse_inertia.get() < 0.0 {
            return Err(ContractError(
                "inverse mass and inertia must be nonnegative".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BodyLaneAccessV1 {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyLayoutLaneV1 {
    pub name: String,
    pub element_stride_bytes: u32,
    pub capacity_elements: u32,
    pub allocated_bytes: u64,
    pub access: BodyLaneAccessV1,
}

impl Validate for BodyLayoutLaneV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.name.is_empty() || self.element_stride_bytes == 0 || self.capacity_elements == 0 {
            return Err(ContractError(
                "body layout lanes require a name, stride, and capacity".to_owned(),
            ));
        }
        let expected = u64::from(self.element_stride_bytes)
            .checked_mul(u64::from(self.capacity_elements))
            .ok_or_else(|| ContractError("body layout lane byte count overflowed".to_owned()))?;
        if self.allocated_bytes != expected {
            return Err(ContractError(format!(
                "body layout lane {} allocated {} bytes, expected {expected}",
                self.name, self.allocated_bytes
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyLayoutV1 {
    pub schema_version: u32,
    pub layout_version: u32,
    pub capacity_elements: u32,
    pub live_elements: u32,
    pub attempted_elements: u64,
    pub bytes_per_body: u32,
    pub allocated_bytes: u64,
    pub overflowed: bool,
    pub lanes: Vec<BodyLayoutLaneV1>,
}

impl Validate for BodyLayoutV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.layout_version != BODY_LAYOUT_VERSION {
            return Err(ContractError(format!(
                "unsupported body layout version {}, expected {BODY_LAYOUT_VERSION}",
                self.layout_version
            )));
        }
        if self.capacity_elements == 0
            || self.live_elements > self.capacity_elements
            || self.attempted_elements < u64::from(self.live_elements)
            || self.lanes.is_empty()
        {
            return Err(ContractError(
                "invalid body layout counts or empty lanes".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.lanes.iter().map(|lane| &lane.name),
            "body layout lanes",
        )?;
        for lane in &self.lanes {
            lane.validate()?;
            if lane.capacity_elements != self.capacity_elements {
                return Err(ContractError(
                    "every body layout lane must use the declared capacity".to_owned(),
                ));
            }
        }
        let expected_stride = self
            .lanes
            .iter()
            .try_fold(0_u32, |total, lane| {
                total.checked_add(lane.element_stride_bytes)
            })
            .ok_or_else(|| ContractError("body layout stride overflowed".to_owned()))?;
        let expected_allocated = self
            .lanes
            .iter()
            .try_fold(0_u64, |total, lane| total.checked_add(lane.allocated_bytes))
            .ok_or_else(|| ContractError("body layout allocation overflowed".to_owned()))?;
        if self.bytes_per_body != expected_stride || self.allocated_bytes != expected_allocated {
            return Err(ContractError(
                "body layout byte totals do not match its lanes".to_owned(),
            ));
        }
        if self.overflowed != (self.attempted_elements > u64::from(self.capacity_elements)) {
            return Err(ContractError(
                "body layout overflow flag does not match attempted capacity".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BodyCommandV1 {
    Reset {
        sequence: u32,
        body: BodyId,
        position: Vec2,
        angle: FiniteF32,
        linear_velocity: Vec2,
        angular_velocity: FiniteF32,
    },
    GrabBegin {
        sequence: u32,
        body: BodyId,
        world_target: Vec2,
    },
    GrabTarget {
        sequence: u32,
        body: BodyId,
        world_target: Vec2,
    },
    GrabEnd {
        sequence: u32,
        body: BodyId,
    },
}

impl BodyCommandV1 {
    pub const fn sequence(&self) -> u32 {
        match self {
            Self::Reset { sequence, .. }
            | Self::GrabBegin { sequence, .. }
            | Self::GrabTarget { sequence, .. }
            | Self::GrabEnd { sequence, .. } => *sequence,
        }
    }

    pub const fn body(&self) -> BodyId {
        match self {
            Self::Reset { body, .. }
            | Self::GrabBegin { body, .. }
            | Self::GrabTarget { body, .. }
            | Self::GrabEnd { body, .. } => *body,
        }
    }
}

impl Validate for BodyCommandV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.body().generation == 0 {
            return Err(ContractError(
                "body commands require a nonzero target generation".to_owned(),
            ));
        }
        match self {
            Self::Reset {
                position,
                angle,
                linear_velocity,
                angular_velocity,
                ..
            } => {
                position.validate()?;
                angle.validate()?;
                linear_velocity.validate()?;
                angular_velocity.validate()
            }
            Self::GrabBegin { world_target, .. } | Self::GrabTarget { world_target, .. } => {
                world_target.validate()
            }
            Self::GrabEnd { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyCommandBatchV1 {
    pub schema_version: u32,
    pub capacity_commands: u32,
    pub attempted_commands: u64,
    pub overflowed: bool,
    pub commands: Vec<BodyCommandV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBodyCommandV1 {
    pub step: u32,
    pub command: BodyCommandV1,
}

impl Validate for ScheduledBodyCommandV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.command.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyCommandScenarioV1 {
    pub schema_version: u32,
    pub id: String,
    pub scenario: ScenarioV1,
    pub scheduled_commands: Vec<ScheduledBodyCommandV1>,
}

impl Validate for BodyCommandScenarioV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.id.is_empty() {
            return Err(ContractError(
                "body-command scenario requires an id".to_owned(),
            ));
        }
        self.scenario.validate()?;
        for scheduled in &self.scheduled_commands {
            scheduled.validate()?;
            if scheduled.step >= self.scenario.requested_steps {
                return Err(ContractError(format!(
                    "body command step {} lies outside requested step count {}",
                    scheduled.step, self.scenario.requested_steps
                )));
            }
            if self
                .scenario
                .bodies
                .binary_search_by_key(&scheduled.command.body(), |body| body.id)
                .is_err()
            {
                return Err(ContractError(format!(
                    "body command references missing body {:?}",
                    scheduled.command.body()
                )));
            }
        }
        for pair in self.scheduled_commands.windows(2) {
            let left = (pair[0].step, pair[0].command.sequence());
            let right = (pair[1].step, pair[1].command.sequence());
            if left >= right {
                return Err(ContractError(
                    "scheduled body commands must be sorted by step and sequence".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

impl Validate for BodyCommandBatchV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.capacity_commands == 0
            || self.commands.len() > self.capacity_commands as usize
            || self.attempted_commands < self.commands.len() as u64
        {
            return Err(ContractError(
                "invalid body command batch counts".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.commands.iter().map(BodyCommandV1::sequence),
            "body command sequences",
        )?;
        for command in &self.commands {
            command.validate()?;
        }
        if self.overflowed != (self.attempted_commands > u64::from(self.capacity_commands)) {
            return Err(ContractError(
                "body command overflow flag does not match attempted capacity".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureIdV1 {
    pub feature_type: u32,
    pub feature_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredContactPointV1 {
    pub local_anchor_a: Vec2,
    pub local_anchor_b: Vec2,
    pub world_point: Vec2,
    pub separation: FiniteF32,
    pub feature_a: FeatureIdV1,
    pub feature_b: FeatureIdV1,
}

impl Validate for AuthoredContactPointV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.local_anchor_a.validate()?;
        self.local_anchor_b.validate()?;
        self.world_point.validate()?;
        self.separation.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredManifoldV1 {
    pub id: u32,
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub normal: Vec2,
    pub material: MaterialV1,
    pub points: Vec<AuthoredContactPointV1>,
}

impl Validate for AuthoredManifoldV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.normal.validate()?;
        let length_squared = self.normal.x * self.normal.x + self.normal.y * self.normal.y;
        if (length_squared - 1.0).abs() > 1.0e-4 {
            return Err(ContractError(
                "manifold normal must be normalized".to_owned(),
            ));
        }
        self.material.validate()?;
        if self.points.is_empty() || self.points.len() > 2 {
            return Err(ContractError(
                "a manifold must contain one or two points".to_owned(),
            ));
        }
        for point in &self.points {
            point.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactCachePointV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub feature_a: FeatureIdV1,
    pub feature_b: FeatureIdV1,
    pub normal_impulse: FiniteF32,
    pub tangent_impulse: FiniteF32,
}

impl Validate for ContactCachePointV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.normal_impulse.validate()?;
        self.tangent_impulse.validate()?;
        if self.normal_impulse.get() < 0.0 {
            return Err(ContractError(
                "cached normal impulse must be nonnegative".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolverSettingsV1 {
    pub fixed_dt: FiniteF32,
    pub gravity: Vec2,
    pub substeps: u32,
    pub iterations: u32,
    pub bias_rate: FiniteF32,
    pub mass_coefficient: FiniteF32,
    pub impulse_coefficient: FiniteF32,
    pub restitution_threshold: FiniteF32,
}

impl Validate for SolverSettingsV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.fixed_dt.validate()?;
        self.gravity.validate()?;
        self.bias_rate.validate()?;
        self.mass_coefficient.validate()?;
        self.impulse_coefficient.validate()?;
        self.restitution_threshold.validate()?;
        if self.fixed_dt.get() <= 0.0 || self.substeps == 0 || self.iterations == 0 {
            return Err(ContractError(
                "fixed dt, substeps, and iterations must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NeutralActionV1 {
    ApplyImpulse {
        step: u32,
        body: BodyId,
        impulse: Vec2,
        world_point: Vec2,
    },
    SetGravity {
        step: u32,
        gravity: Vec2,
    },
}

impl Validate for NeutralActionV1 {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::ApplyImpulse {
                impulse,
                world_point,
                ..
            } => {
                impulse.validate()?;
                world_point.validate()
            }
            Self::SetGravity { gravity, .. } => gravity.validate(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioV1 {
    pub schema_version: u32,
    pub id: String,
    pub seed: u64,
    pub requested_steps: u32,
    pub settings: SolverSettingsV1,
    pub bodies: Vec<BodyStateV1>,
    pub manifolds: Vec<AuthoredManifoldV1>,
    pub contact_cache: Vec<ContactCachePointV1>,
    pub actions: Vec<NeutralActionV1>,
}

impl Validate for ScenarioV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.id.is_empty() || self.requested_steps == 0 {
            return Err(ContractError(
                "scenario id and requested steps must be present".to_owned(),
            ));
        }
        self.settings.validate()?;
        validate_sorted_unique(self.bodies.iter().map(|body| body.id), "body ids")?;
        for body in &self.bodies {
            body.validate()?;
        }
        validate_sorted_unique(
            self.manifolds.iter().map(|manifold| manifold.id),
            "manifold ids",
        )?;
        for manifold in &self.manifolds {
            manifold.validate()?;
        }
        for cache in &self.contact_cache {
            cache.validate()?;
        }
        for action in &self.actions {
            action.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedValueV1 {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceV1 {
    pub run_id: String,
    pub source_revision: String,
    pub source_dirty: bool,
    pub source_patch_digest: Option<String>,
    pub host: String,
    pub adapter: String,
    pub backend: String,
    pub driver: String,
    pub toolchain: String,
    pub build_options: Vec<NamedValueV1>,
    pub runtime_settings: Vec<NamedValueV1>,
}

impl Validate for ProvenanceV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.run_id.is_empty()
            || self.source_revision.is_empty()
            || self.host.is_empty()
            || self.adapter.is_empty()
            || self.backend.is_empty()
            || self.driver.is_empty()
            || self.toolchain.is_empty()
        {
            return Err(ContractError(
                "complete run, source, host, adapter, backend, driver, and toolchain provenance is required"
                    .to_owned(),
            ));
        }
        validate_sorted_unique(
            self.build_options.iter().map(|value| &value.name),
            "build options",
        )?;
        validate_sorted_unique(
            self.runtime_settings.iter().map(|value| &value.name),
            "runtime settings",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleRequestV1 {
    pub schema_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub scenario: ScenarioV1,
}

impl Validate for OracleRequestV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.run_id.is_empty() || !is_sha256(&self.source_scenario_hash) {
            return Err(ContractError(
                "oracle request requires run id and source scenario SHA-256".to_owned(),
            ));
        }
        self.scenario.validate()?;
        let actual = canonical_hash(&self.scenario)?;
        if actual != self.source_scenario_hash {
            return Err(ContractError("source scenario hash mismatch".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceStageV1 {
    Prepare,
    WarmStart,
    NormalSolve,
    TangentSolve,
    Relaxation,
    Restitution,
    Integrate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceValueV1 {
    pub name: String,
    pub value: FiniteF32,
    pub bits: u32,
}

impl Validate for TraceValueV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.value.validate()?;
        if self.value.get().to_bits() != self.bits {
            return Err(ContractError("trace value bits do not match".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceRecordV1 {
    pub step: u32,
    pub substep: u32,
    pub iteration: u32,
    pub stage: TraceStageV1,
    pub manifold_id: u32,
    pub point_index: u32,
    pub values: Vec<TraceValueV1>,
}

impl Validate for TraceRecordV1 {
    fn validate(&self) -> Result<(), ContractError> {
        validate_sorted_unique(self.values.iter().map(|value| &value.name), "trace values")?;
        for value in &self.values {
            value.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkVectorV1 {
    pub bodies_total: u32,
    pub bodies_awake: u32,
    pub circles: u32,
    pub rectangles: u32,
    pub candidate_pairs: u64,
    pub overlapping_pairs: u64,
    pub manifolds: u32,
    pub contact_points: u32,
    pub solver_work: u64,
    pub dispatches: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingSummaryV1 {
    pub sample_count: u32,
    pub p50_seconds: FiniteF32,
    pub p95_seconds: FiniteF32,
    pub p99_seconds: FiniteF32,
    pub gpu_timestamps_available: bool,
    pub timestamp_quantization_seconds: Option<FiniteF32>,
}

impl Validate for TimingSummaryV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.p50_seconds.validate()?;
        self.p95_seconds.validate()?;
        self.p99_seconds.validate()?;
        if self.sample_count == 0
            || self.p50_seconds.get() < 0.0
            || self.p50_seconds.get() > self.p95_seconds.get()
            || self.p95_seconds.get() > self.p99_seconds.get()
        {
            return Err(ContractError("invalid timing summary".to_owned()));
        }
        if let Some(quantization) = self.timestamp_quantization_seconds {
            quantization.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityRecordV1 {
    pub class: String,
    pub capacity_elements: u64,
    pub used_elements: u64,
    pub allocated_bytes: u64,
    pub high_water_bytes: u64,
    pub overflowed: bool,
}

impl Validate for CapacityRecordV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.class.is_empty()
            || self.used_elements > self.capacity_elements
            || self.high_water_bytes > self.allocated_bytes
        {
            return Err(ContractError("invalid capacity record".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsV1 {
    pub work: WorkVectorV1,
    pub total_physics: Option<TimingSummaryV1>,
    pub cpu_encode_submit: Option<TimingSummaryV1>,
    pub capacities: Vec<CapacityRecordV1>,
    pub penetration_max: FiniteF32,
    pub jitter_rms: FiniteF32,
    pub energy_delta: FiniteF32,
}

impl Validate for MetricsV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if let Some(timing) = &self.total_physics {
            timing.validate()?;
        }
        if let Some(timing) = &self.cpu_encode_submit {
            timing.validate()?;
        }
        validate_sorted_unique(
            self.capacities.iter().map(|record| &record.class),
            "capacities",
        )?;
        for capacity in &self.capacities {
            capacity.validate()?;
        }
        self.penetration_max.validate()?;
        self.jitter_rms.validate()?;
        self.energy_delta.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureKindV1 {
    UnsupportedFeature,
    InvalidRequest,
    Timeout,
    StaleResult,
    HashMismatch,
    MissingSentinel,
    Invariant,
    NumericalDivergence,
    CapacityOverflow,
    MissingPair,
    DuplicatePair,
    ManifoldTopology,
    StaleIdentity,
    PersistenceMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureV1 {
    pub kind: FailureKindV1,
    pub reason: String,
    pub first_step: Option<u32>,
    pub first_stage: Option<String>,
    pub reproducer_hash: Option<String>,
}

impl Validate for FailureV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.reason.is_empty() {
            return Err(ContractError("failure reason is required".to_owned()));
        }
        if self
            .reproducer_hash
            .as_deref()
            .is_some_and(|hash| !is_sha256(hash))
        {
            return Err(ContractError("invalid reproducer hash".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleResultV1 {
    pub schema_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub projection_hash: String,
    pub provenance: ProvenanceV1,
    pub bodies: Vec<BodyStateV1>,
    pub manifolds: Vec<AuthoredManifoldV1>,
    pub traces: Vec<TraceRecordV1>,
    pub metrics: MetricsV1,
    pub failure: Option<FailureV1>,
}

impl Validate for OracleResultV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.run_id.is_empty()
            || !is_sha256(&self.source_scenario_hash)
            || !is_sha256(&self.projection_hash)
        {
            return Err(ContractError("invalid result identity hashes".to_owned()));
        }
        self.provenance.validate()?;
        validate_sorted_unique(self.bodies.iter().map(|body| body.id), "result body ids")?;
        for body in &self.bodies {
            body.validate()?;
        }
        validate_sorted_unique(
            self.manifolds.iter().map(|manifold| manifold.id),
            "result manifold ids",
        )?;
        for manifold in &self.manifolds {
            manifold.validate()?;
        }
        for trace in &self.traces {
            trace.validate()?;
        }
        self.metrics.validate()?;
        if let Some(failure) = &self.failure {
            failure.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackArtifactV1 {
    pub schema_version: u32,
    pub run_id: String,
    pub harness_provenance: ProvenanceV1,
    pub tracks: Vec<PlaybackTrackV1>,
}

impl Validate for PlaybackArtifactV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.run_id.is_empty() || self.harness_provenance.run_id != self.run_id {
            return Err(ContractError(
                "playback run id must be present and match its provenance".to_owned(),
            ));
        }
        self.harness_provenance.validate()?;
        if self.tracks.is_empty() {
            return Err(ContractError(
                "playback requires at least one track".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.tracks.iter().map(|track| &track.id),
            "playback track ids",
        )?;
        for track in &self.tracks {
            track.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTrackV1 {
    pub id: String,
    pub source_scenario_hash: String,
    pub source_scenario: ScenarioV1,
    pub samples: Vec<PlaybackSampleV1>,
}

impl Validate for PlaybackTrackV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.id.is_empty() || !is_sha256(&self.source_scenario_hash) {
            return Err(ContractError(
                "playback track requires an id and source scenario hash".to_owned(),
            ));
        }
        self.source_scenario.validate()?;
        if canonical_hash(&self.source_scenario)? != self.source_scenario_hash {
            return Err(ContractError(
                "playback source scenario hash mismatch".to_owned(),
            ));
        }
        if self.samples.is_empty() {
            return Err(ContractError(
                "playback track requires at least one sample".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.samples.iter().map(|sample| sample.completed_steps),
            "playback sample steps",
        )?;
        for sample in &self.samples {
            sample.validate()?;
            if sample.completed_steps == 0
                || sample.completed_steps > self.source_scenario.requested_steps
            {
                return Err(ContractError(
                    "playback sample step lies outside the source scenario".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSampleV1 {
    pub completed_steps: u32,
    pub scalar_bodies: Vec<BodyStateV1>,
    pub authority_bodies: Vec<BodyStateV1>,
    pub scalar_trace_records: u64,
    pub scalar_trace_hash: String,
    pub oracle_scenario_hash: String,
    pub oracle_projection_hash: String,
    pub oracle_provenance: ProvenanceV1,
    pub oracle_artifact: String,
    pub position_delta_max: FiniteF32,
    pub angle_delta_max: FiniteF32,
    pub linear_velocity_delta_max: FiniteF32,
    pub angular_velocity_delta_max: FiniteF32,
    pub dynamic_sleep_state_match: bool,
}

impl Validate for PlaybackSampleV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if !is_sha256(&self.scalar_trace_hash)
            || !is_sha256(&self.oracle_scenario_hash)
            || !is_sha256(&self.oracle_projection_hash)
            || self.oracle_artifact.is_empty()
        {
            return Err(ContractError(
                "playback sample requires trace/oracle hashes and an artifact path".to_owned(),
            ));
        }
        self.oracle_provenance.validate()?;
        if self.oracle_artifact != format!("oracles/{}", self.oracle_provenance.run_id) {
            return Err(ContractError(
                "playback oracle artifact must name its provenance run directory".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.scalar_bodies.iter().map(|body| body.id),
            "scalar playback body ids",
        )?;
        validate_sorted_unique(
            self.authority_bodies.iter().map(|body| body.id),
            "authority playback body ids",
        )?;
        if self.scalar_bodies.len() != self.authority_bodies.len()
            || self
                .scalar_bodies
                .iter()
                .zip(&self.authority_bodies)
                .any(|(scalar, authority)| scalar.id != authority.id)
        {
            return Err(ContractError(
                "playback scalar and authority body identities differ".to_owned(),
            ));
        }
        for body in self.scalar_bodies.iter().chain(&self.authority_bodies) {
            body.validate()?;
        }
        for delta in [
            self.position_delta_max,
            self.angle_delta_max,
            self.linear_velocity_delta_max,
            self.angular_velocity_delta_max,
        ] {
            delta.validate()?;
            if delta.get() < 0.0 {
                return Err(ContractError(
                    "playback deltas must be nonnegative".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

pub fn canonical_json<T: Serialize + Validate>(value: &T) -> Result<Vec<u8>, ContractError> {
    value.validate()?;
    serde_json::to_vec(value).map_err(|error| ContractError(error.to_string()))
}

pub fn canonical_hash<T: Serialize + Validate>(value: &T) -> Result<String, ContractError> {
    Ok(sha256_bytes(&canonical_json(value)?))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn finite_f32(name: &str, value: f32) -> Result<(), ContractError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ContractError(format!("{name} must be finite")))
    }
}

fn version(actual: u32) -> Result<(), ContractError> {
    if actual == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ContractError(format!(
            "unsupported schema version {actual}, expected {SCHEMA_VERSION}"
        )))
    }
}

fn validate_sorted_unique<T: Ord>(
    values: impl IntoIterator<Item = T>,
    name: &str,
) -> Result<(), ContractError> {
    let mut previous = None;
    for value in values {
        if previous.as_ref().is_some_and(|previous| previous >= &value) {
            return Err(ContractError(format!("{name} must be sorted and unique")));
        }
        previous = Some(value);
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    fn scenario() -> ScenarioV1 {
        ScenarioV1 {
            schema_version: SCHEMA_VERSION,
            id: "one-contact".to_owned(),
            seed: 7,
            requested_steps: 1,
            settings: SolverSettingsV1 {
                fixed_dt: finite(1.0 / 60.0),
                gravity: Vec2::new(0.0, -10.0),
                substeps: 4,
                iterations: 1,
                bias_rate: finite(0.2),
                mass_coefficient: finite(1.0),
                impulse_coefficient: finite(0.0),
                restitution_threshold: finite(1.0),
            },
            bodies: vec![BodyStateV1 {
                id: BodyId {
                    slot: 0,
                    generation: 1,
                },
                shape: ShapeV1::Circle {
                    radius: finite(0.5),
                },
                position: Vec2::ZERO,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(2.0),
                material: MaterialV1 {
                    friction: finite(0.5),
                    restitution: finite(0.1),
                },
                sleeping: false,
            }],
            manifolds: Vec::new(),
            contact_cache: Vec::new(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn canonical_round_trip_and_hash_are_stable() {
        let scenario = scenario();
        let bytes = canonical_json(&scenario).unwrap();
        let decoded: ScenarioV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, scenario);
        assert_eq!(canonical_json(&decoded).unwrap(), bytes);
        assert_eq!(canonical_hash(&decoded).unwrap(), sha256_bytes(&bytes));
    }

    #[test]
    fn non_finite_values_are_rejected_before_serialization() {
        assert!(FiniteF32::new(f32::NAN).is_err());
        let mut scenario = scenario();
        scenario.bodies[0].position.x = f32::INFINITY;
        assert!(canonical_json(&scenario).is_err());
    }

    #[test]
    fn wrong_version_and_hash_fail_deterministically() {
        let mut wrong_version = scenario();
        wrong_version.schema_version = 99;
        assert!(
            wrong_version
                .validate()
                .unwrap_err()
                .0
                .contains("version 99")
        );

        let scenario = scenario();
        let request = OracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "run-1".to_owned(),
            source_scenario_hash: "0".repeat(64),
            scenario,
        };
        assert_eq!(
            request.validate().unwrap_err().0,
            "source scenario hash mismatch"
        );
    }

    #[test]
    fn ordering_is_part_of_the_contract() {
        let mut scenario = scenario();
        let mut second = scenario.bodies[0].clone();
        second.id.slot = 0;
        scenario.bodies.push(second);
        assert!(
            scenario
                .validate()
                .unwrap_err()
                .0
                .contains("body ids must be sorted and unique")
        );
    }

    #[test]
    fn body_layout_bytes_and_overflow_are_self_consistent() {
        let layout = BodyLayoutV1 {
            schema_version: SCHEMA_VERSION,
            layout_version: BODY_LAYOUT_VERSION,
            capacity_elements: 4,
            live_elements: 2,
            attempted_elements: 2,
            bytes_per_body: 32,
            allocated_bytes: 128,
            overflowed: false,
            lanes: vec![
                BodyLayoutLaneV1 {
                    name: "motion".to_owned(),
                    element_stride_bytes: 16,
                    capacity_elements: 4,
                    allocated_bytes: 64,
                    access: BodyLaneAccessV1::ReadWrite,
                },
                BodyLayoutLaneV1 {
                    name: "pose".to_owned(),
                    element_stride_bytes: 16,
                    capacity_elements: 4,
                    allocated_bytes: 64,
                    access: BodyLaneAccessV1::ReadWrite,
                },
            ],
        };
        layout.validate().unwrap();

        let mut wrong_total = layout.clone();
        wrong_total.bytes_per_body = 16;
        assert!(
            wrong_total
                .validate()
                .unwrap_err()
                .0
                .contains("byte totals")
        );

        let mut silent_overflow = layout;
        silent_overflow.attempted_elements = 5;
        assert!(
            silent_overflow
                .validate()
                .unwrap_err()
                .0
                .contains("overflow flag")
        );
    }

    #[test]
    fn command_batches_require_finite_ordered_bounded_records() {
        let body = BodyId {
            slot: 3,
            generation: 9,
        };
        let command = BodyCommandV1::GrabTarget {
            sequence: 1,
            body,
            world_target: Vec2::new(0.25, 0.75),
        };
        let batch = BodyCommandBatchV1 {
            schema_version: SCHEMA_VERSION,
            capacity_commands: 2,
            attempted_commands: 1,
            overflowed: false,
            commands: vec![command.clone()],
        };
        batch.validate().unwrap();
        let bytes = canonical_json(&batch).unwrap();
        let decoded: BodyCommandBatchV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, batch);

        let mut duplicate = batch.clone();
        duplicate.commands.push(command);
        duplicate.attempted_commands = 2;
        assert!(
            duplicate
                .validate()
                .unwrap_err()
                .0
                .contains("sorted and unique")
        );

        let mut overflow = batch;
        overflow.attempted_commands = 3;
        assert!(overflow.validate().is_err());
        overflow.overflowed = true;
        overflow.validate().unwrap();
    }
}
