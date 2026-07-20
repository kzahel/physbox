//! Validation-only discrete event contract for the S16 precision study.
//!
//! The records in this module describe branch and ownership choices. Their
//! continuous fields are diagnostic payload only and are never an alternate
//! scalar specification or product state.

use physbox_contracts::{BodyId, FeatureIdV1};
use serde::{Deserialize, Serialize};

pub const EVENT_STRUCTURE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventFamilyV1 {
    ContactAdmissibilityAndPointCount,
    ReferenceOwnerAndAuthorityFeatureIdentity,
    SeparationPositiveAndSpeculativeBranch,
    UnilateralActiveSetProjection,
    WarmCacheMatchAndImpulseOwnership,
    SupportGraphColorAndOverflowOrder,
}

impl EventFamilyV1 {
    pub const ALL: [Self; 6] = [
        Self::ContactAdmissibilityAndPointCount,
        Self::ReferenceOwnerAndAuthorityFeatureIdentity,
        Self::SeparationPositiveAndSpeculativeBranch,
        Self::UnilateralActiveSetProjection,
        Self::WarmCacheMatchAndImpulseOwnership,
        Self::SupportGraphColorAndOverflowOrder,
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventLaneV1 {
    ExactS12F32,
    CancellationPathF64,
    FullSolveF64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NormalPassV1 {
    Biased,
    Relaxation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPointIdentityV1 {
    pub feature_a: FeatureIdV1,
    pub feature_b: FeatureIdV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPairV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventGraphClassV1 {
    Overflow,
    Color(u8),
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPointEventV1 {
    pub point_ordinal: u32,
    pub identity: EventPointIdentityV1,
    pub cache_hit: bool,
    pub cache_owner: Option<EventPointIdentityV1>,
    pub normal_impulse: f64,
    pub tangent_impulse: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedContactEventV1 {
    pub completed_step: u32,
    pub preparation_ordinal: u32,
    pub pair: EventPairV1,
    pub admitted: bool,
    pub point_count: u32,
    pub reference_owner: Option<u8>,
    pub reference_face: Option<u32>,
    pub graph_class: EventGraphClassV1,
    pub visit_rank: u32,
    pub points: Vec<PreparedPointEventV1>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalDecisionEventV1 {
    pub completed_step: u32,
    pub substep: u32,
    pub pass: NormalPassV1,
    pub visit_rank: u32,
    pub pair: EventPairV1,
    pub point_ordinal: u32,
    pub identity: EventPointIdentityV1,
    pub separation_positive: bool,
    pub candidate_selected: bool,
    pub separation: f64,
    pub old_impulse: f64,
    pub unclamped_impulse: f64,
    pub new_impulse: f64,
    pub applied_impulse: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventStreamV1 {
    pub version: u32,
    pub lane: EventLaneV1,
    pub prepared_contacts: Vec<PreparedContactEventV1>,
    pub normal_decisions: Vec<NormalDecisionEventV1>,
}

impl EventStreamV1 {
    pub fn new(lane: EventLaneV1) -> Self {
        Self {
            version: EVENT_STRUCTURE_VERSION,
            lane,
            prepared_contacts: Vec::new(),
            normal_decisions: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NormalDecisionRawV1 {
    pub point_ordinal: u32,
    pub identity: EventPointIdentityV1,
    pub separation_positive: bool,
    pub candidate_selected: bool,
    pub separation: f64,
    pub old_impulse: f64,
    pub unclamped_impulse: f64,
    pub new_impulse: f64,
    pub applied_impulse: f64,
}

pub(crate) fn reference_owner_and_face(
    points: &[EventPointIdentityV1],
) -> (Option<u8>, Option<u32>) {
    if points.len() != 2 {
        return (None, None);
    }
    let reference_a = points.iter().all(|point| point.feature_a.feature_type == 1);
    let reference_b = points.iter().all(|point| point.feature_b.feature_type == 1);
    if reference_a {
        (Some(0), Some(points[0].feature_a.feature_index))
    } else if reference_b {
        (Some(1), Some(points[0].feature_b.feature_index))
    } else {
        (None, None)
    }
}
