#![forbid(unsafe_code)]

mod collision;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: u32 = 1;
pub const BODY_LAYOUT_VERSION: u32 = 1;
pub const GRAB_INTENT_CONTRACT_VERSION: u32 = 1;
pub const CONSTRUCTION_INTENT_CONTRACT_VERSION: u32 = 1;
pub const SETTLING_OBSERVATION_VERSION: u32 = 1;
pub const SETTLING_OBSERVATION_V2_VERSION: u32 = 2;
pub const CONTACT_IDENTITY_POLICY_VERSION: u32 = 1;
pub const CONTACT_SOFTNESS_POLICY_VERSION: u32 = 1;
pub const ORACLE_SUCCESS_SENTINEL: &str = "PHYSBOX5_ORACLE_SUCCESS";

pub use collision::*;

/// Laboratory policy for choosing the reference face of a rectangle contact.
///
/// This is deliberately separate from `SolverSettingsV1`: changing contact
/// identity is an evaluated narrowphase candidate, not an accepted physical
/// default.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactIdentityPolicyV1 {
    DirectMaximumSeparation,
    RelativeCoherenceBias,
    CanonicalAbsoluteBand,
    PriorContactHysteresis,
}

impl ContactIdentityPolicyV1 {
    pub const SELECTED: Self = Self::DirectMaximumSeparation;

    pub const ALL: [Self; 4] = [
        Self::DirectMaximumSeparation,
        Self::RelativeCoherenceBias,
        Self::CanonicalAbsoluteBand,
        Self::PriorContactHysteresis,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::DirectMaximumSeparation => "direct-maximum-separation",
            Self::RelativeCoherenceBias => "relative-coherence-bias",
            Self::CanonicalAbsoluteBand => "canonical-absolute-band",
            Self::PriorContactHysteresis => "prior-contact-hysteresis",
        }
    }
}

/// Reversible laboratory policy for deriving soft-contact coefficients.
///
/// The selected product leaves this absent and continues to use the explicit
/// hard coefficients in `SolverSettingsV1`. The hertz/damping candidate is a
/// source setting so scalar, GPU, native, browser, and Android lanes all
/// execute the same declared experiment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactSoftnessPolicyV1 {
    pub version: u32,
    pub contact_hertz: FiniteF32,
    pub damping_ratio: FiniteF32,
    pub static_contact_hertz_multiplier: FiniteF32,
}

impl Validate for ContactSoftnessPolicyV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.contact_hertz.validate()?;
        self.damping_ratio.validate()?;
        self.static_contact_hertz_multiplier.validate()?;
        if self.version != CONTACT_SOFTNESS_POLICY_VERSION
            || self.contact_hertz.get() <= 0.0
            || self.damping_ratio.get() < 0.0
            || self.static_contact_hertz_multiplier.get() <= 0.0
        {
            return Err(ContractError(
                "contact softness requires version 1, positive hertz/static multiplier, and nonnegative damping"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

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

/// Body-free interactive grab intent. The GPU resolves `Begin` against the
/// current body state and retains the selected generational identity; hosts do
/// not provide or observe that identity to advance interaction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GrabIntentV1 {
    Begin { sequence: u32, world_target: Vec2 },
    Target { sequence: u32, world_target: Vec2 },
    End { sequence: u32 },
}

impl GrabIntentV1 {
    pub const fn sequence(self) -> u32 {
        match self {
            Self::Begin { sequence, .. }
            | Self::Target { sequence, .. }
            | Self::End { sequence } => sequence,
        }
    }
}

impl Validate for GrabIntentV1 {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Begin { world_target, .. } | Self::Target { world_target, .. } => {
                world_target.validate()
            }
            Self::End { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabIntentBatchV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub capacity_intents: u32,
    pub attempted_intents: u64,
    pub overflowed: bool,
    pub intents: Vec<GrabIntentV1>,
}

/// Body-free construction intent. The GPU resolves placement capacity and
/// erase selection against current device-owned occupancy; hosts never choose
/// or observe a body slot to advance construction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ConstructionIntentV1 {
    PlaceCircle { sequence: u32, world_target: Vec2 },
    PlaceRectangle { sequence: u32, world_target: Vec2 },
    EraseAt { sequence: u32, world_target: Vec2 },
}

impl ConstructionIntentV1 {
    pub const fn sequence(self) -> u32 {
        match self {
            Self::PlaceCircle { sequence, .. }
            | Self::PlaceRectangle { sequence, .. }
            | Self::EraseAt { sequence, .. } => sequence,
        }
    }

    pub const fn world_target(self) -> Vec2 {
        match self {
            Self::PlaceCircle { world_target, .. }
            | Self::PlaceRectangle { world_target, .. }
            | Self::EraseAt { world_target, .. } => world_target,
        }
    }
}

impl Validate for ConstructionIntentV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.world_target().validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionIntentBatchV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub capacity_intents: u32,
    pub attempted_intents: u64,
    pub overflowed: bool,
    pub intents: Vec<ConstructionIntentV1>,
}

impl Validate for ConstructionIntentBatchV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.contract_version != CONSTRUCTION_INTENT_CONTRACT_VERSION {
            return Err(ContractError(format!(
                "unsupported construction intent contract version {}",
                self.contract_version
            )));
        }
        if self.capacity_intents == 0
            || self.intents.len() > self.capacity_intents as usize
            || self.attempted_intents < self.intents.len() as u64
        {
            return Err(ContractError(
                "invalid construction intent batch counts".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.intents
                .iter()
                .copied()
                .map(ConstructionIntentV1::sequence),
            "construction intent sequences",
        )?;
        for intent in &self.intents {
            intent.validate()?;
        }
        if self.overflowed != (self.attempted_intents > u64::from(self.capacity_intents)) {
            return Err(ContractError(
                "construction intent overflow flag does not match attempted capacity".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConstructionResultV1 {
    #[default]
    None,
    Placed,
    Erased,
    CapacityFull,
    DeepOverlap,
    EraseMiss,
    GenerationExhausted,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionStateV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub capacity_bodies: u32,
    pub live_bodies: u32,
    pub attempted_intents: u64,
    pub succeeded_intents: u64,
    pub placed_circles: u64,
    pub placed_rectangles: u64,
    pub erased_bodies: u64,
    pub capacity_failures: u64,
    pub deep_overlap_failures: u64,
    pub erase_miss_failures: u64,
    pub generation_failures: u64,
    pub grab_invalidations: u64,
    pub last_sequence: u32,
    pub last_result: ConstructionResultV1,
    pub last_body: Option<BodyId>,
}

impl Validate for ConstructionStateV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.contract_version != CONSTRUCTION_INTENT_CONTRACT_VERSION {
            return Err(ContractError(format!(
                "unsupported construction state contract version {}",
                self.contract_version
            )));
        }
        if self.capacity_bodies == 0 || self.live_bodies > self.capacity_bodies {
            return Err(ContractError(
                "construction state has invalid body capacity".to_owned(),
            ));
        }
        let succeeded = self
            .placed_circles
            .checked_add(self.placed_rectangles)
            .and_then(|value| value.checked_add(self.erased_bodies))
            .ok_or_else(|| ContractError("construction success counts overflowed".to_owned()))?;
        if succeeded != self.succeeded_intents {
            return Err(ContractError(
                "construction success total does not match its operations".to_owned(),
            ));
        }
        let failures = self
            .capacity_failures
            .checked_add(self.deep_overlap_failures)
            .and_then(|value| value.checked_add(self.erase_miss_failures))
            .and_then(|value| value.checked_add(self.generation_failures))
            .ok_or_else(|| ContractError("construction failure counts overflowed".to_owned()))?;
        let accounted = succeeded
            .checked_add(failures)
            .ok_or_else(|| ContractError("construction result counts overflowed".to_owned()))?;
        if self.attempted_intents != accounted {
            return Err(ContractError(
                "construction attempt total does not match successes and failures".to_owned(),
            ));
        }
        if let Some(body) = self.last_body {
            if body.generation == 0 {
                return Err(ContractError(
                    "construction state has an invalid last body generation".to_owned(),
                ));
            }
            if !matches!(
                self.last_result,
                ConstructionResultV1::Placed | ConstructionResultV1::Erased
            ) {
                return Err(ContractError(
                    "only successful construction results may name a body".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

impl Validate for GrabIntentBatchV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.contract_version != GRAB_INTENT_CONTRACT_VERSION {
            return Err(ContractError(format!(
                "unsupported grab intent contract version {}",
                self.contract_version
            )));
        }
        if self.capacity_intents == 0
            || self.intents.len() > self.capacity_intents as usize
            || self.attempted_intents < self.intents.len() as u64
        {
            return Err(ContractError("invalid grab intent batch counts".to_owned()));
        }
        validate_sorted_unique(
            self.intents.iter().copied().map(GrabIntentV1::sequence),
            "grab intent sequences",
        )?;
        for intent in &self.intents {
            intent.validate()?;
        }
        if self.overflowed != (self.attempted_intents > u64::from(self.capacity_intents)) {
            return Err(ContractError(
                "grab intent overflow flag does not match attempted capacity".to_owned(),
            ));
        }
        Ok(())
    }
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
pub struct ContactPolicyV1 {
    pub speculative_distance: FiniteF32,
    pub max_depenetration_velocity: FiniteF32,
}

impl Validate for ContactPolicyV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.speculative_distance.validate()?;
        self.max_depenetration_velocity.validate()?;
        if self.speculative_distance.get() < 0.0 || self.max_depenetration_velocity.get() <= 0.0 {
            return Err(ContractError(
                "speculative distance must be nonnegative and maximum depenetration velocity must be positive"
                    .to_owned(),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_softness: Option<ContactSoftnessPolicyV1>,
    pub restitution_threshold: FiniteF32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_policy: Option<ContactPolicyV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabTuningV1 {
    pub spring_acceleration: FiniteF32,
    pub damping_rate: FiniteF32,
    pub max_acceleration: FiniteF32,
    pub max_linear_speed: FiniteF32,
    pub max_angular_speed: FiniteF32,
}

impl Validate for GrabTuningV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.spring_acceleration.validate()?;
        self.damping_rate.validate()?;
        self.max_acceleration.validate()?;
        self.max_linear_speed.validate()?;
        self.max_angular_speed.validate()?;
        if self.spring_acceleration.get() <= 0.0
            || self.damping_rate.get() < 0.0
            || self.max_acceleration.get() <= 0.0
            || self.max_linear_speed.get() <= 0.0
            || self.max_angular_speed.get() <= 0.0
        {
            return Err(ContractError(
                "grab spring and limits must be positive and damping must be nonnegative"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

impl Validate for SolverSettingsV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.fixed_dt.validate()?;
        self.gravity.validate()?;
        self.bias_rate.validate()?;
        self.mass_coefficient.validate()?;
        self.impulse_coefficient.validate()?;
        if let Some(policy) = self.contact_softness {
            policy.validate()?;
        }
        self.restitution_threshold.validate()?;
        if let Some(policy) = self.contact_policy {
            policy.validate()?;
        }
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
    GrabBegin {
        step: u32,
        body: BodyId,
        world_anchor: Vec2,
        world_target: Vec2,
        tuning: GrabTuningV1,
    },
    GrabTarget {
        step: u32,
        world_target: Vec2,
    },
    GrabEnd {
        step: u32,
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
            Self::GrabBegin {
                world_anchor,
                world_target,
                tuning,
                ..
            } => {
                world_anchor.validate()?;
                world_target.validate()?;
                tuning.validate()
            }
            Self::GrabTarget { world_target, .. } => world_target.validate(),
            Self::GrabEnd { .. } => Ok(()),
        }
    }
}

impl NeutralActionV1 {
    pub const fn step(&self) -> u32 {
        match self {
            Self::ApplyImpulse { step, .. }
            | Self::SetGravity { step, .. }
            | Self::GrabBegin { step, .. }
            | Self::GrabTarget { step, .. }
            | Self::GrabEnd { step } => *step,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingObservationV1 {
    pub completed_steps: u32,
    pub dynamic_body_count: u32,
    pub residual_linear_speed_max: FiniteF32,
    pub residual_linear_speed_rms: FiniteF32,
    pub residual_angular_speed_max: FiniteF32,
    pub residual_angular_speed_rms: FiniteF32,
    pub center_drift_max: FiniteF32,
    pub stack_height: FiniteF32,
    pub penetration_max: FiniteF32,
    pub separation_max: FiniteF32,
    pub contact_points: u32,
    pub contact_feature_churn: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_impulse_variation: Option<FiniteF32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tangent_impulse_variation: Option<FiniteF32>,
    pub kinetic_energy: FiniteF32,
    pub capacities: Vec<CapacityRecordV1>,
}

impl Validate for SettlingObservationV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.completed_steps == 0 || self.dynamic_body_count == 0 {
            return Err(ContractError(
                "settling observation requires completed steps and dynamic bodies".to_owned(),
            ));
        }
        for value in [
            self.residual_linear_speed_max,
            self.residual_linear_speed_rms,
            self.residual_angular_speed_max,
            self.residual_angular_speed_rms,
            self.center_drift_max,
            self.stack_height,
            self.penetration_max,
            self.separation_max,
            self.kinetic_energy,
        ] {
            value.validate()?;
            if value.get() < 0.0 {
                return Err(ContractError(
                    "settling magnitudes must be nonnegative".to_owned(),
                ));
            }
        }
        for value in [
            self.normal_impulse_variation,
            self.tangent_impulse_variation,
        ]
        .into_iter()
        .flatten()
        {
            value.validate()?;
            if value.get() < 0.0 {
                return Err(ContractError(
                    "settling impulse variation must be nonnegative".to_owned(),
                ));
            }
        }
        validate_sorted_unique(
            self.capacities.iter().map(|record| &record.class),
            "settling capacities",
        )?;
        for capacity in &self.capacities {
            capacity.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingSeriesV1 {
    pub schema_version: u32,
    pub observation_version: u32,
    pub source_scenario_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_hash: Option<String>,
    pub lane: String,
    pub sleeping_enabled: bool,
    pub transient_end_step: u32,
    pub settled_start_step: u32,
    pub observations: Vec<SettlingObservationV1>,
    pub structural_pass: bool,
    pub structural_reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactReferenceOwnerV2 {
    None,
    BodyA,
    BodyB,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarmCacheStatusV2 {
    Hit,
    PairStarted,
    Recontact,
    NormalChanged,
    PointCountChanged,
    ReferenceOwnerChanged,
    FeatureChanged,
    CurrentEntryMissing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactPointCacheObservationV2 {
    pub key: PersistentContactKeyV1,
    pub status: WarmCacheStatusV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_normal_impulse: Option<FiniteF32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_tangent_impulse: Option<FiniteF32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_normal_impulse: Option<FiniteF32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_tangent_impulse: Option<FiniteF32>,
}

impl Validate for ContactPointCacheObservationV2 {
    fn validate(&self) -> Result<(), ContractError> {
        self.key.validate()?;
        for value in [
            self.previous_normal_impulse,
            self.previous_tangent_impulse,
            self.current_normal_impulse,
            self.current_tangent_impulse,
        ]
        .into_iter()
        .flatten()
        {
            value.validate()?;
        }
        if self
            .previous_normal_impulse
            .is_some_and(|value| value.get() < 0.0)
            || self
                .current_normal_impulse
                .is_some_and(|value| value.get() < 0.0)
        {
            return Err(ContractError(
                "settling cache normal impulses must be nonnegative".to_owned(),
            ));
        }
        if matches!(self.status, WarmCacheStatusV2::Hit)
            && (self.previous_normal_impulse.is_none() || self.current_normal_impulse.is_none())
        {
            return Err(ContractError(
                "a settling cache hit requires previous and current impulses".to_owned(),
            ));
        }
        if matches!(self.status, WarmCacheStatusV2::CurrentEntryMissing)
            != self.current_normal_impulse.is_none()
        {
            return Err(ContractError(
                "current-entry-missing must match the current cache payload".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingContactObservationV2 {
    pub pair: CandidatePairV1,
    pub reference_owner: ContactReferenceOwnerV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_face_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_reference_owner: Option<ContactReferenceOwnerV2>,
    pub point_count: u32,
    pub previous_point_count: u32,
    pub normal: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_normal: Option<Vec2>,
    pub normal_delta_radians: FiniteF32,
    pub relative_tangent_speed_max: FiniteF32,
    pub stable_support: bool,
    pub stable_support_age_steps: u32,
    pub reference_owner_changed: bool,
    pub point_count_changed: bool,
    pub feature_set_changed: bool,
    pub normal_changed: bool,
    pub unexplained_stable_churn: bool,
    pub previous_features: Vec<PersistentContactKeyV1>,
    pub current_features: Vec<PersistentContactKeyV1>,
    pub cache: Vec<ContactPointCacheObservationV2>,
}

impl Validate for SettlingContactObservationV2 {
    fn validate(&self) -> Result<(), ContractError> {
        self.pair.validate()?;
        if self.point_count == 0
            || self.point_count > 2
            || self.current_features.len() != self.point_count as usize
            || self.previous_features.len() != self.previous_point_count as usize
            || self.cache.len() != self.point_count as usize
            || self.reference_face_index.is_some_and(|index| index >= 4)
        {
            return Err(ContractError(
                "invalid settling contact point/reference counts".to_owned(),
            ));
        }
        if matches!(
            self.reference_owner,
            ContactReferenceOwnerV2::BodyA | ContactReferenceOwnerV2::BodyB
        ) != self.reference_face_index.is_some()
        {
            return Err(ContractError(
                "a body-owned settling reference requires one face index".to_owned(),
            ));
        }
        self.normal.validate()?;
        if let Some(previous) = self.previous_normal {
            previous.validate()?;
        }
        self.normal_delta_radians.validate()?;
        self.relative_tangent_speed_max.validate()?;
        if self.normal_delta_radians.get() < 0.0
            || self.normal_delta_radians.get() > std::f32::consts::PI
            || self.relative_tangent_speed_max.get() < 0.0
            || self.stable_support != (self.stable_support_age_steps > 0)
        {
            return Err(ContractError(
                "invalid settling contact motion/support state".to_owned(),
            ));
        }
        validate_sorted_unique(self.previous_features.iter(), "previous settling features")?;
        validate_sorted_unique(self.current_features.iter(), "current settling features")?;
        validate_sorted_unique(
            self.cache.iter().map(|value| &value.key),
            "settling cache observations",
        )?;
        if self
            .previous_features
            .iter()
            .chain(&self.current_features)
            .chain(self.cache.iter().map(|value| &value.key))
            .any(|key| key.pair != self.pair)
        {
            return Err(ContractError(
                "settling contact features must belong to their declared pair".to_owned(),
            ));
        }
        for cache in &self.cache {
            cache.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingObservationV2 {
    pub observation_version: u32,
    pub base: SettlingObservationV1,
    pub angular_edge_speed_max: FiniteF32,
    pub angular_edge_speed_rms: FiniteF32,
    pub stable_support_pairs: u32,
    pub reference_owner_changes: u32,
    pub point_count_changes: u32,
    pub feature_set_changes: u32,
    pub contact_normal_changes: u32,
    pub warm_cache_hits: u32,
    pub warm_cache_misses: u32,
    pub unexplained_stable_churn: u32,
    pub contacts: Vec<SettlingContactObservationV2>,
}

impl Validate for SettlingObservationV2 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.observation_version != SETTLING_OBSERVATION_V2_VERSION {
            return Err(ContractError(format!(
                "unsupported settling observation v2 version {}",
                self.observation_version
            )));
        }
        self.base.validate()?;
        self.angular_edge_speed_max.validate()?;
        self.angular_edge_speed_rms.validate()?;
        if self.angular_edge_speed_max.get() < 0.0 || self.angular_edge_speed_rms.get() < 0.0 {
            return Err(ContractError(
                "settling angular edge speeds must be nonnegative".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.contacts.iter().map(|value| value.pair),
            "settling contact pairs",
        )?;
        for contact in &self.contacts {
            contact.validate()?;
        }
        let cache_points = self
            .contacts
            .iter()
            .map(|value| value.cache.len() as u32)
            .sum::<u32>();
        if self.base.contact_points
            != self
                .contacts
                .iter()
                .map(|value| value.point_count)
                .sum::<u32>()
            || self.stable_support_pairs
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.stable_support)
                    .count() as u32
            || self.reference_owner_changes
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.reference_owner_changed)
                    .count() as u32
            || self.point_count_changes
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.point_count_changed)
                    .count() as u32
            || self.feature_set_changes
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.feature_set_changed)
                    .count() as u32
            || self.contact_normal_changes
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.normal_changed)
                    .count() as u32
            || self.unexplained_stable_churn
                != self
                    .contacts
                    .iter()
                    .filter(|value| value.unexplained_stable_churn)
                    .count() as u32
            || self.warm_cache_hits + self.warm_cache_misses != cache_points
        {
            return Err(ContractError(
                "settling observation aggregates do not match contact details".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingMeasurementPolicyV2 {
    pub version: u32,
    pub observation_interval_steps: u32,
    pub transient_end_step: u32,
    pub settled_start_step: u32,
    pub settled_end_step: u32,
    pub rest_linear_speed_limit: FiniteF32,
    pub rest_angular_edge_speed_limit: FiniteF32,
    pub rest_required_consecutive_observations: u32,
    pub stable_support_required_consecutive_observations: u32,
    pub stable_normal_delta_limit_radians: FiniteF32,
    pub stable_relative_tangent_speed_limit: FiniteF32,
    pub stable_support_gravity_alignment_min: FiniteF32,
    pub energy_absolute_epsilon: FiniteF32,
    pub energy_relative_epsilon: FiniteF32,
    pub positive_energy_run_limit: u32,
}

impl Validate for SettlingMeasurementPolicyV2 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != SETTLING_OBSERVATION_V2_VERSION
            || self.observation_interval_steps == 0
            || self.transient_end_step == 0
            || self.settled_start_step <= self.transient_end_step
            || self.settled_end_step < self.settled_start_step
            || self.rest_required_consecutive_observations == 0
            || self.stable_support_required_consecutive_observations == 0
            || self.positive_energy_run_limit == 0
        {
            return Err(ContractError(
                "invalid settling measurement cadence/window".to_owned(),
            ));
        }
        for value in [
            self.rest_linear_speed_limit,
            self.rest_angular_edge_speed_limit,
            self.stable_normal_delta_limit_radians,
            self.stable_relative_tangent_speed_limit,
            self.stable_support_gravity_alignment_min,
            self.energy_absolute_epsilon,
            self.energy_relative_epsilon,
        ] {
            value.validate()?;
            if value.get() < 0.0 {
                return Err(ContractError(
                    "settling measurement limits must be nonnegative".to_owned(),
                ));
            }
        }
        if self.rest_linear_speed_limit.get() == 0.0
            || self.rest_angular_edge_speed_limit.get() == 0.0
            || self.stable_normal_delta_limit_radians.get() > std::f32::consts::PI
            || self.stable_relative_tangent_speed_limit.get() == 0.0
            || self.stable_support_gravity_alignment_min.get() > 1.0
        {
            return Err(ContractError(
                "invalid settling measurement thresholds".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingSummaryV2 {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_rest_step: Option<u32>,
    pub settled_linear_speed_max: FiniteF32,
    pub settled_angular_edge_speed_max: FiniteF32,
    pub stable_support_unexplained_churn: u64,
    pub warm_cache_misses: u64,
    pub first_energy_quartile_max: FiniteF32,
    pub last_energy_quartile_max: FiniteF32,
    pub positive_energy_run_max: u32,
    pub energy_envelope_nonincreasing: bool,
}

impl Validate for SettlingSummaryV2 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != SETTLING_OBSERVATION_V2_VERSION {
            return Err(ContractError("invalid settling summary version".to_owned()));
        }
        for value in [
            self.settled_linear_speed_max,
            self.settled_angular_edge_speed_max,
            self.first_energy_quartile_max,
            self.last_energy_quartile_max,
        ] {
            value.validate()?;
            if value.get() < 0.0 {
                return Err(ContractError(
                    "settling summary magnitudes must be nonnegative".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingSeriesV2 {
    pub schema_version: u32,
    pub observation_version: u32,
    pub source_scenario_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_hash: Option<String>,
    pub lane: String,
    pub sleeping_enabled: bool,
    pub policy: SettlingMeasurementPolicyV2,
    pub observations: Vec<SettlingObservationV2>,
    pub summary: SettlingSummaryV2,
    pub structural_pass: bool,
    pub structural_reason: String,
}

impl Validate for SettlingSeriesV2 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        self.policy.validate()?;
        self.summary.validate()?;
        if self.observation_version != SETTLING_OBSERVATION_V2_VERSION
            || !is_sha256(&self.source_scenario_hash)
            || self
                .projection_hash
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || self.lane.is_empty()
            || self.structural_reason.is_empty()
            || self.observations.is_empty()
            || self.sleeping_enabled
        {
            return Err(ContractError(
                "invalid settling series v2 identity".to_owned(),
            ));
        }
        let first = self
            .observations
            .first()
            .expect("nonempty checked")
            .base
            .completed_steps;
        let last = self
            .observations
            .last()
            .expect("nonempty checked")
            .base
            .completed_steps;
        if first != self.policy.observation_interval_steps || last != self.policy.settled_end_step {
            return Err(ContractError(
                "settling observations do not span the frozen cadence".to_owned(),
            ));
        }
        for (index, observation) in self.observations.iter().enumerate() {
            observation.validate()?;
            let expected = (index as u32 + 1)
                .checked_mul(self.policy.observation_interval_steps)
                .ok_or_else(|| ContractError("settling cadence overflowed".to_owned()))?;
            if observation.base.completed_steps != expected {
                return Err(ContractError(
                    "settling observations violate frozen cadence".to_owned(),
                ));
            }
        }
        if self
            .summary
            .time_to_rest_step
            .is_some_and(|step| step > last)
        {
            return Err(ContractError(
                "settling time to rest exceeds the series".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Validate for SettlingSeriesV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.observation_version != SETTLING_OBSERVATION_VERSION
            || !is_sha256(&self.source_scenario_hash)
            || self
                .projection_hash
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || self.lane.is_empty()
            || self.structural_reason.is_empty()
            || self.transient_end_step == 0
            || self.settled_start_step <= self.transient_end_step
            || self.observations.is_empty()
        {
            return Err(ContractError(
                "invalid settling series identity or window".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.observations
                .iter()
                .map(|observation| observation.completed_steps),
            "settling observation steps",
        )?;
        for observation in &self.observations {
            observation.validate()?;
        }
        let final_step = self
            .observations
            .last()
            .expect("nonempty observations checked above")
            .completed_steps;
        if self.transient_end_step > final_step || self.settled_start_step > final_step {
            return Err(ContractError(
                "settling windows lie outside the observation series".to_owned(),
            ));
        }
        Ok(())
    }
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
                contact_softness: None,
                restitution_threshold: finite(1.0),
                contact_policy: None,
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
        assert!(!String::from_utf8_lossy(&bytes).contains("contactPolicy"));
        assert!(!String::from_utf8_lossy(&bytes).contains("contactSoftness"));
        let decoded: ScenarioV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, scenario);
        assert_eq!(canonical_json(&decoded).unwrap(), bytes);
        assert_eq!(canonical_hash(&decoded).unwrap(), sha256_bytes(&bytes));
    }

    #[test]
    fn contact_policy_is_explicit_optional_and_validated() {
        let mut scenario = scenario();
        scenario.settings.contact_policy = Some(ContactPolicyV1 {
            speculative_distance: finite(0.02),
            max_depenetration_velocity: finite(3.0),
        });
        scenario.validate().unwrap();
        let bytes = canonical_json(&scenario).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(
            "\"contactPolicy\":{\"speculativeDistance\":0.02,\"maxDepenetrationVelocity\":3.0}"
        ));
        assert_eq!(
            serde_json::from_slice::<ScenarioV1>(&bytes).unwrap(),
            scenario
        );

        scenario
            .settings
            .contact_policy
            .as_mut()
            .unwrap()
            .speculative_distance = finite(-0.01);
        assert!(scenario.validate().is_err());
        scenario
            .settings
            .contact_policy
            .as_mut()
            .unwrap()
            .speculative_distance = finite(0.02);
        scenario
            .settings
            .contact_policy
            .as_mut()
            .unwrap()
            .max_depenetration_velocity = finite(0.0);
        assert!(scenario.validate().is_err());
    }

    #[test]
    fn contact_softness_is_versioned_optional_and_validated() {
        let mut scenario = scenario();
        scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite(30.0),
            damping_ratio: finite(10.0),
            static_contact_hertz_multiplier: finite(2.0),
        });
        scenario.validate().unwrap();
        let bytes = canonical_json(&scenario).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(
            "\"contactSoftness\":{\"version\":1,\"contactHertz\":30.0,\"dampingRatio\":10.0,\"staticContactHertzMultiplier\":2.0}"
        ));
        assert_eq!(
            serde_json::from_slice::<ScenarioV1>(&bytes).unwrap(),
            scenario
        );

        scenario.settings.contact_softness.as_mut().unwrap().version = 99;
        assert!(scenario.validate().is_err());
        scenario.settings.contact_softness.as_mut().unwrap().version =
            CONTACT_SOFTNESS_POLICY_VERSION;
        scenario
            .settings
            .contact_softness
            .as_mut()
            .unwrap()
            .contact_hertz = finite(0.0);
        assert!(scenario.validate().is_err());
        scenario
            .settings
            .contact_softness
            .as_mut()
            .unwrap()
            .contact_hertz = finite(30.0);
        scenario
            .settings
            .contact_softness
            .as_mut()
            .unwrap()
            .damping_ratio = finite(-1.0);
        assert!(scenario.validate().is_err());
        scenario
            .settings
            .contact_softness
            .as_mut()
            .unwrap()
            .damping_ratio = finite(10.0);
        scenario
            .settings
            .contact_softness
            .as_mut()
            .unwrap()
            .static_contact_hertz_multiplier = finite(0.0);
        assert!(scenario.validate().is_err());
    }

    #[test]
    fn neutral_grab_actions_are_explicit_finite_and_versioned_with_the_scenario() {
        let tuning = GrabTuningV1 {
            spring_acceleration: finite(45.0),
            damping_rate: finite(10.0),
            max_acceleration: finite(80.0),
            max_linear_speed: finite(20.0),
            max_angular_speed: finite(30.0),
        };
        let mut scenario = scenario();
        scenario.actions = vec![
            NeutralActionV1::GrabBegin {
                step: 0,
                body: scenario.bodies[0].id,
                world_anchor: Vec2::new(0.2, 0.0),
                world_target: Vec2::new(0.2, 0.0),
                tuning,
            },
            NeutralActionV1::GrabTarget {
                step: 0,
                world_target: Vec2::new(0.3, 0.4),
            },
            NeutralActionV1::GrabEnd { step: 0 },
        ];
        scenario.validate().unwrap();
        let bytes = canonical_json(&scenario).unwrap();
        assert_eq!(
            serde_json::from_slice::<ScenarioV1>(&bytes).unwrap(),
            scenario
        );
        assert_eq!(scenario.actions[0].step(), 0);

        let NeutralActionV1::GrabBegin { tuning, .. } = &mut scenario.actions[0] else {
            unreachable!()
        };
        tuning.max_acceleration = finite(0.0);
        assert!(scenario.validate().is_err());
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

    #[test]
    fn grab_intents_are_body_free_versioned_ordered_and_bounded() {
        let batch = GrabIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: GRAB_INTENT_CONTRACT_VERSION,
            capacity_intents: 3,
            attempted_intents: 3,
            overflowed: false,
            intents: vec![
                GrabIntentV1::Begin {
                    sequence: 1,
                    world_target: Vec2::new(0.25, 0.5),
                },
                GrabIntentV1::Target {
                    sequence: 2,
                    world_target: Vec2::new(0.5, 0.75),
                },
                GrabIntentV1::End { sequence: 3 },
            ],
        };
        batch.validate().unwrap();
        let encoded = canonical_json(&batch).unwrap();
        let decoded: GrabIntentBatchV1 = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, batch);
        assert!(!encoded.windows(4).any(|window| window == b"body"));

        let mut wrong_version = batch.clone();
        wrong_version.contract_version += 1;
        assert!(wrong_version.validate().is_err());
        let mut duplicate = batch.clone();
        duplicate.intents[1] = GrabIntentV1::End { sequence: 1 };
        assert!(duplicate.validate().is_err());
        let mut overflow = batch;
        overflow.attempted_intents = 4;
        overflow.overflowed = true;
        overflow.validate().unwrap();
    }

    #[test]
    fn construction_intents_are_body_free_versioned_ordered_and_bounded() {
        let batch = ConstructionIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: CONSTRUCTION_INTENT_CONTRACT_VERSION,
            capacity_intents: 3,
            attempted_intents: 3,
            overflowed: false,
            intents: vec![
                ConstructionIntentV1::PlaceCircle {
                    sequence: 1,
                    world_target: Vec2::new(-0.5, 0.75),
                },
                ConstructionIntentV1::PlaceRectangle {
                    sequence: 2,
                    world_target: Vec2::new(0.0, 0.75),
                },
                ConstructionIntentV1::EraseAt {
                    sequence: 3,
                    world_target: Vec2::new(-0.5, 0.75),
                },
            ],
        };
        batch.validate().unwrap();
        let encoded = canonical_json(&batch).unwrap();
        let decoded: ConstructionIntentBatchV1 = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, batch);
        assert!(!encoded.windows(4).any(|window| window == b"body"));

        let mut wrong_version = batch.clone();
        wrong_version.contract_version += 1;
        assert!(wrong_version.validate().is_err());
        let mut duplicate = batch.clone();
        duplicate.intents[1] = ConstructionIntentV1::EraseAt {
            sequence: 1,
            world_target: Vec2::ZERO,
        };
        assert!(duplicate.validate().is_err());
        let mut overflow = batch;
        overflow.attempted_intents = 4;
        overflow.overflowed = true;
        overflow.validate().unwrap();
    }
}
