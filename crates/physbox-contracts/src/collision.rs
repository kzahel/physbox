use serde::{Deserialize, Serialize};

use crate::{
    BodyId, ContractError, FailureKindV1, FailureV1, FiniteF32, MaterialV1, ProvenanceV1,
    SCHEMA_VERSION, TraceValueV1, Validate, Vec2,
};

pub const COLLISION_LAYOUT_VERSION: u32 = 1;
pub const ANALYTIC_SHAPE_LAYOUT_VERSION: u32 = 1;
pub const ANALYTIC_SHAPE_FLAG_RECTANGLE: u32 = 1 << 8;
pub const ANALYTIC_SHAPE_FLAG_CIRCLE: u32 = 1 << 9;
pub const ANALYTIC_SHAPE_FLAG_MASK: u32 =
    ANALYTIC_SHAPE_FLAG_RECTANGLE | ANALYTIC_SHAPE_FLAG_CIRCLE;

pub const BODY_AABB_BYTES: u32 = 32;
pub const CANDIDATE_PAIR_BYTES: u32 = 16;
pub const COLLISION_CAPACITY_COUNTER_BYTES: u32 = 16;
pub const COLLISION_CONTACT_CACHE_VALUE_BYTES: u32 = 16;
pub const COLLISION_MANIFOLD_BYTES: u32 = 48;
pub const COLLISION_CONTACT_POINT_BYTES: u32 = 48;
pub const PERSISTENT_CONTACT_KEY_BYTES: u32 = 32;
pub const COLLISION_WORK_COUNTER_BYTES: u32 = 128;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyticShapeKindV1 {
    Circle,
    Rectangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticShapeEncodingV1 {
    pub schema_version: u32,
    pub layout_version: u32,
    pub body: BodyId,
    pub kind: AnalyticShapeKindV1,
    pub property_payload: Vec2,
    pub metadata_flag: u32,
    pub property_lane_stride_bytes: u32,
    pub metadata_lane_stride_bytes: u32,
}

impl Validate for AnalyticShapeEncodingV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.layout_version != ANALYTIC_SHAPE_LAYOUT_VERSION {
            return Err(ContractError(format!(
                "unsupported analytic shape layout version {}",
                self.layout_version
            )));
        }
        body_id("analytic shape", self.body)?;
        self.property_payload.validate()?;
        let expected_flag = match self.kind {
            AnalyticShapeKindV1::Circle => {
                if self.property_payload.x <= 0.0
                    || self.property_payload.x.to_bits() != self.property_payload.y.to_bits()
                {
                    return Err(ContractError(
                        "circle payload must repeat one positive radius".to_owned(),
                    ));
                }
                ANALYTIC_SHAPE_FLAG_CIRCLE
            }
            AnalyticShapeKindV1::Rectangle => {
                if self.property_payload.x <= 0.0 || self.property_payload.y <= 0.0 {
                    return Err(ContractError(
                        "rectangle payload requires positive half extents".to_owned(),
                    ));
                }
                ANALYTIC_SHAPE_FLAG_RECTANGLE
            }
        };
        if self.metadata_flag != expected_flag
            || self.metadata_flag & ANALYTIC_SHAPE_FLAG_MASK != expected_flag
            || self.property_lane_stride_bytes != 16
            || self.metadata_lane_stride_bytes != 16
        {
            return Err(ContractError(
                "analytic shape flag or accepted body-lane stride mismatch".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionRecordLayoutV1 {
    pub name: String,
    pub stride_bytes: u32,
    pub alignment_bytes: u32,
}

impl Validate for CollisionRecordLayoutV1 {
    fn validate(&self) -> Result<(), ContractError> {
        let expected = expected_stride(&self.name).ok_or_else(|| {
            ContractError(format!("unknown collision record layout {}", self.name))
        })?;
        if self.stride_bytes != expected || self.alignment_bytes != 16 {
            return Err(ContractError(format!(
                "collision record {} requires stride {expected} and alignment 16",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionLayoutV1 {
    pub schema_version: u32,
    pub layout_version: u32,
    pub records: Vec<CollisionRecordLayoutV1>,
}

impl Validate for CollisionLayoutV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.layout_version != COLLISION_LAYOUT_VERSION {
            return Err(ContractError(format!(
                "unsupported collision layout version {}",
                self.layout_version
            )));
        }
        let expected_names = [
            "body-aabb",
            "candidate-pair",
            "capacity-counter",
            "contact-cache-value",
            "contact-manifold",
            "contact-point",
            "persistent-contact-key",
            "work-counters",
        ];
        if self.records.len() != expected_names.len()
            || self
                .records
                .iter()
                .zip(expected_names)
                .any(|(record, expected)| record.name != expected)
        {
            return Err(ContractError(
                "collision record layouts must be complete, sorted, and unique".to_owned(),
            ));
        }
        for record in &self.records {
            record.validate()?;
        }
        Ok(())
    }
}

pub fn collision_layout_v1() -> CollisionLayoutV1 {
    let record = |name: &str| CollisionRecordLayoutV1 {
        name: name.to_owned(),
        stride_bytes: expected_stride(name).expect("built-in collision record name"),
        alignment_bytes: 16,
    };
    CollisionLayoutV1 {
        schema_version: SCHEMA_VERSION,
        layout_version: COLLISION_LAYOUT_VERSION,
        records: [
            "body-aabb",
            "candidate-pair",
            "capacity-counter",
            "contact-cache-value",
            "contact-manifold",
            "contact-point",
            "persistent-contact-key",
            "work-counters",
        ]
        .into_iter()
        .map(record)
        .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyAabbV1 {
    pub body: BodyId,
    pub lower: Vec2,
    pub upper: Vec2,
}

impl Validate for BodyAabbV1 {
    fn validate(&self) -> Result<(), ContractError> {
        body_id("body AABB", self.body)?;
        self.lower.validate()?;
        self.upper.validate()?;
        if self.lower.x > self.upper.x || self.lower.y > self.upper.y {
            return Err(ContractError(
                "body AABB lower bound exceeds upper bound".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePairV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
}

impl Validate for CandidatePairV1 {
    fn validate(&self) -> Result<(), ContractError> {
        body_id("candidate body A", self.body_a)?;
        body_id("candidate body B", self.body_b)?;
        if self.body_a >= self.body_b {
            return Err(ContractError(
                "candidate pair bodies must be canonical and distinct".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairSlotAssignmentV1 {
    pub pair: CandidatePairV1,
    pub slot: u32,
    pub generation: u32,
}

impl Validate for PairSlotAssignmentV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.pair.validate()?;
        if self.generation == 0 {
            return Err(ContractError(
                "pair-slot generation must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeometricFeatureKindV1 {
    Circle,
    Face,
    Vertex,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometricFeatureV1 {
    pub kind: GeometricFeatureKindV1,
    pub index: u32,
}

impl Validate for GeometricFeatureV1 {
    fn validate(&self) -> Result<(), ContractError> {
        let valid = match self.kind {
            GeometricFeatureKindV1::Circle => self.index == 0,
            GeometricFeatureKindV1::Face | GeometricFeatureKindV1::Vertex => self.index < 4,
        };
        if !valid {
            return Err(ContractError(
                "analytic feature index is outside its canonical range".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentContactKeyV1 {
    pub pair: CandidatePairV1,
    pub feature_a: GeometricFeatureV1,
    pub feature_b: GeometricFeatureV1,
}

impl Validate for PersistentContactKeyV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.pair.validate()?;
        self.feature_a.validate()?;
        self.feature_b.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionContactPointV1 {
    pub local_anchor_a: Vec2,
    pub local_anchor_b: Vec2,
    pub world_point: Vec2,
    pub separation: FiniteF32,
    pub feature_a: GeometricFeatureV1,
    pub feature_b: GeometricFeatureV1,
}

impl CollisionContactPointV1 {
    pub const fn key(&self, pair: CandidatePairV1) -> PersistentContactKeyV1 {
        PersistentContactKeyV1 {
            pair,
            feature_a: self.feature_a,
            feature_b: self.feature_b,
        }
    }
}

impl Validate for CollisionContactPointV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.local_anchor_a.validate()?;
        self.local_anchor_b.validate()?;
        self.world_point.validate()?;
        self.separation.validate()?;
        self.feature_a.validate()?;
        self.feature_b.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionManifoldV1 {
    pub pair: CandidatePairV1,
    pub normal: Vec2,
    pub material: MaterialV1,
    pub points: Vec<CollisionContactPointV1>,
}

impl Validate for CollisionManifoldV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.pair.validate()?;
        self.normal.validate()?;
        let length_squared = self.normal.x * self.normal.x + self.normal.y * self.normal.y;
        if (length_squared - 1.0).abs() > 1.0e-4 {
            return Err(ContractError(
                "collision manifold normal must be normalized".to_owned(),
            ));
        }
        self.material.validate()?;
        if self.points.is_empty() || self.points.len() > 2 {
            return Err(ContractError(
                "collision manifold requires one or two points".to_owned(),
            ));
        }
        for point in &self.points {
            point.validate()?;
        }
        for points in self.points.windows(2) {
            if points[0].key(self.pair) >= points[1].key(self.pair) {
                return Err(ContractError(
                    "collision points must have sorted unique persistent keys".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactCacheValueV1 {
    pub normal_impulse: FiniteF32,
    pub tangent_impulse: FiniteF32,
    pub last_seen_step: u32,
}

impl Validate for ContactCacheValueV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.normal_impulse.validate()?;
        self.tangent_impulse.validate()?;
        if self.normal_impulse.get() < 0.0 {
            return Err(ContractError(
                "persistent normal impulse must be nonnegative".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentContactV1 {
    pub key: PersistentContactKeyV1,
    pub value: ContactCacheValueV1,
}

impl Validate for PersistentContactV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.key.validate()?;
        self.value.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionCapacityV1 {
    pub class: String,
    pub stride_bytes: u32,
    pub capacity_elements: u64,
    pub attempted_elements: u64,
    pub committed_elements: u64,
    pub allocated_bytes: u64,
    pub high_water_bytes: u64,
    pub overflowed: bool,
    pub accepted: bool,
}

impl Validate for CollisionCapacityV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.class.is_empty() || self.stride_bytes == 0 || self.capacity_elements == 0 {
            return Err(ContractError(
                "collision capacity requires class, stride, and capacity".to_owned(),
            ));
        }
        let allocated = self
            .capacity_elements
            .checked_mul(u64::from(self.stride_bytes))
            .ok_or_else(|| ContractError("collision capacity bytes overflowed".to_owned()))?;
        let overflowed = self.attempted_elements > self.capacity_elements;
        let fully_committed =
            !overflowed && self.accepted && self.committed_elements == self.attempted_elements;
        let atomically_rejected = !self.accepted && self.committed_elements == 0;
        let high_water = self
            .attempted_elements
            .min(self.capacity_elements)
            .checked_mul(u64::from(self.stride_bytes))
            .ok_or_else(|| ContractError("collision high-water bytes overflowed".to_owned()))?;
        if self.allocated_bytes != allocated
            || self.overflowed != overflowed
            || !(fully_committed || atomically_rejected)
            || self.high_water_bytes != high_water
        {
            return Err(ContractError(
                "collision capacity does not describe atomic overflow semantics".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionWorkV1 {
    pub bodies: u64,
    pub pair_attempts: u64,
    pub fixed_fixed_rejected: u64,
    pub aabb_rejected: u64,
    pub candidate_pairs: u64,
    pub narrowphase_tests: u64,
    pub manifolds: u64,
    pub contact_points: u64,
    pub sort_elements: u64,
    pub sort_passes: u64,
    pub cache_previous: u64,
    pub cache_current: u64,
    pub cache_matched: u64,
    pub cache_cold: u64,
    pub cache_expired: u64,
    pub dispatches: u64,
}

impl Validate for CollisionWorkV1 {
    fn validate(&self) -> Result<(), ContractError> {
        let accounted_pairs = self
            .fixed_fixed_rejected
            .checked_add(self.aabb_rejected)
            .and_then(|value| value.checked_add(self.candidate_pairs))
            .ok_or_else(|| ContractError("collision pair accounting overflowed".to_owned()))?;
        if accounted_pairs != self.pair_attempts
            || self.narrowphase_tests > self.candidate_pairs
            || self.manifolds > self.narrowphase_tests
            || self.contact_points < self.manifolds
            || self.contact_points > self.manifolds.saturating_mul(2)
            || self.sort_elements != self.cache_current
            || self.cache_current != self.cache_matched.saturating_add(self.cache_cold)
            || self.cache_matched > self.cache_previous
            || self.cache_expired > self.cache_previous
        {
            return Err(ContractError(
                "collision work counters are internally inconsistent".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollisionTraceStageV1 {
    BodyAabb,
    Candidate,
    Narrowphase,
    PersistentKey,
    CacheMerge,
    SolverProjection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionTraceV1 {
    pub step: u32,
    pub stage: CollisionTraceStageV1,
    pub pair: Option<CandidatePairV1>,
    pub key: Option<PersistentContactKeyV1>,
    pub values: Vec<TraceValueV1>,
}

impl Validate for CollisionTraceV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if let Some(pair) = self.pair {
            pair.validate()?;
        }
        if let Some(key) = self.key {
            key.validate()?;
            if self.pair.is_some_and(|pair| pair != key.pair) {
                return Err(ContractError(
                    "collision trace pair and persistent key disagree".to_owned(),
                ));
            }
        }
        let mut previous = None;
        for value in &self.values {
            value.validate()?;
            if previous
                .as_ref()
                .is_some_and(|name: &&String| *name >= &value.name)
            {
                return Err(ContractError(
                    "collision trace values must be sorted and unique".to_owned(),
                ));
            }
            previous = Some(&value.name);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionArtifactV1 {
    pub schema_version: u32,
    pub collision_layout_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub projection_hash: String,
    pub completed_step: u32,
    pub provenance: ProvenanceV1,
    pub layout: CollisionLayoutV1,
    pub shape_encodings: Vec<AnalyticShapeEncodingV1>,
    pub aabbs: Vec<BodyAabbV1>,
    pub candidates: Vec<CandidatePairV1>,
    pub manifolds: Vec<CollisionManifoldV1>,
    pub cache: Vec<PersistentContactV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub traces: Vec<CollisionTraceV1>,
    pub failure: Option<FailureV1>,
}

impl Validate for CollisionArtifactV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.collision_layout_version != COLLISION_LAYOUT_VERSION
            || self.run_id.is_empty()
            || !is_sha256(&self.source_scenario_hash)
            || !is_sha256(&self.projection_hash)
        {
            return Err(ContractError(
                "invalid collision artifact identity or version".to_owned(),
            ));
        }
        self.provenance.validate()?;
        if self.provenance.run_id != self.run_id {
            return Err(ContractError(
                "collision artifact and provenance run ids differ".to_owned(),
            ));
        }
        self.layout.validate()?;
        sorted_unique_by(
            &self.shape_encodings,
            |encoding| encoding.body,
            "shape encodings",
        )?;
        for encoding in &self.shape_encodings {
            encoding.validate()?;
        }
        sorted_unique_by(&self.aabbs, |aabb| aabb.body, "body AABBs")?;
        for aabb in &self.aabbs {
            aabb.validate()?;
        }
        sorted_unique(&self.candidates, "candidate pairs")?;
        for pair in &self.candidates {
            pair.validate()?;
            referenced_pair(pair, &self.aabbs)?;
        }
        sorted_unique_by(&self.manifolds, |manifold| manifold.pair, "manifolds")?;
        for manifold in &self.manifolds {
            manifold.validate()?;
            referenced_pair(&manifold.pair, &self.aabbs)?;
        }
        sorted_unique_by(&self.cache, |entry| entry.key, "persistent contacts")?;
        for entry in &self.cache {
            entry.validate()?;
            referenced_pair(&entry.key.pair, &self.aabbs)?;
        }
        self.work.validate()?;
        sorted_unique_by(
            &self.capacities,
            |capacity| capacity.class.clone(),
            "capacities",
        )?;
        for capacity in &self.capacities {
            capacity.validate()?;
        }
        for trace in &self.traces {
            trace.validate()?;
        }
        if let Some(failure) = &self.failure {
            failure.validate()?;
        }
        let overflowed = self.capacities.iter().any(|capacity| capacity.overflowed);
        if overflowed
            && self.failure.as_ref().map(|failure| &failure.kind)
                != Some(&FailureKindV1::CapacityOverflow)
        {
            return Err(ContractError(
                "collision capacity overflow requires structured failure data".to_owned(),
            ));
        }
        Ok(())
    }
}

fn expected_stride(name: &str) -> Option<u32> {
    match name {
        "body-aabb" => Some(BODY_AABB_BYTES),
        "candidate-pair" => Some(CANDIDATE_PAIR_BYTES),
        "capacity-counter" => Some(COLLISION_CAPACITY_COUNTER_BYTES),
        "contact-cache-value" => Some(COLLISION_CONTACT_CACHE_VALUE_BYTES),
        "contact-manifold" => Some(COLLISION_MANIFOLD_BYTES),
        "contact-point" => Some(COLLISION_CONTACT_POINT_BYTES),
        "persistent-contact-key" => Some(PERSISTENT_CONTACT_KEY_BYTES),
        "work-counters" => Some(COLLISION_WORK_COUNTER_BYTES),
        _ => None,
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

fn body_id(name: &str, id: BodyId) -> Result<(), ContractError> {
    if id.generation == 0 {
        Err(ContractError(format!(
            "{name} requires a nonzero body generation"
        )))
    } else {
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sorted_unique<T: Ord>(values: &[T], name: &str) -> Result<(), ContractError> {
    for pair in values.windows(2) {
        if pair[0] >= pair[1] {
            return Err(ContractError(format!("{name} must be sorted and unique")));
        }
    }
    Ok(())
}

fn sorted_unique_by<T, K: Ord>(
    values: &[T],
    key: impl Fn(&T) -> K,
    name: &str,
) -> Result<(), ContractError> {
    for pair in values.windows(2) {
        if key(&pair[0]) >= key(&pair[1]) {
            return Err(ContractError(format!("{name} must be sorted and unique")));
        }
    }
    Ok(())
}

fn referenced_pair(pair: &CandidatePairV1, aabbs: &[BodyAabbV1]) -> Result<(), ContractError> {
    if aabbs
        .binary_search_by_key(&pair.body_a, |aabb| aabb.body)
        .is_err()
        || aabbs
            .binary_search_by_key(&pair.body_b, |aabb| aabb.body)
            .is_err()
    {
        return Err(ContractError(
            "collision pair references a missing or stale body AABB".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NamedValueV1, canonical_json};

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    fn id(slot: u32, generation: u32) -> BodyId {
        BodyId { slot, generation }
    }

    fn pair() -> CandidatePairV1 {
        CandidatePairV1 {
            body_a: id(0, 1),
            body_b: id(1, 1),
        }
    }

    fn feature(kind: GeometricFeatureKindV1, index: u32) -> GeometricFeatureV1 {
        GeometricFeatureV1 { kind, index }
    }

    #[test]
    fn pair_slot_assignment_requires_a_positive_generation() {
        let valid = PairSlotAssignmentV1 {
            pair: pair(),
            slot: 4,
            generation: 2,
        };
        valid.validate().unwrap();
        assert!(
            PairSlotAssignmentV1 {
                generation: 0,
                ..valid
            }
            .validate()
            .is_err()
        );
    }

    fn provenance() -> ProvenanceV1 {
        ProvenanceV1 {
            run_id: "collision-contract-test".to_owned(),
            source_revision: "revision".to_owned(),
            source_dirty: true,
            source_patch_digest: Some("patch".to_owned()),
            host: "test".to_owned(),
            adapter: "test".to_owned(),
            backend: "test".to_owned(),
            driver: "test".to_owned(),
            toolchain: "test".to_owned(),
            build_options: vec![NamedValueV1 {
                name: "profile".to_owned(),
                value: "test".to_owned(),
            }],
            runtime_settings: Vec::new(),
        }
    }

    fn artifact() -> CollisionArtifactV1 {
        let pair = pair();
        let point = CollisionContactPointV1 {
            local_anchor_a: Vec2::new(0.5, 0.0),
            local_anchor_b: Vec2::new(-0.5, 0.0),
            world_point: Vec2::ZERO,
            separation: finite(-0.01),
            feature_a: feature(GeometricFeatureKindV1::Vertex, 1),
            feature_b: feature(GeometricFeatureKindV1::Face, 3),
        };
        CollisionArtifactV1 {
            schema_version: SCHEMA_VERSION,
            collision_layout_version: COLLISION_LAYOUT_VERSION,
            run_id: "collision-contract-test".to_owned(),
            source_scenario_hash: "0".repeat(64),
            projection_hash: "1".repeat(64),
            completed_step: 1,
            provenance: provenance(),
            layout: collision_layout_v1(),
            shape_encodings: vec![
                AnalyticShapeEncodingV1 {
                    schema_version: SCHEMA_VERSION,
                    layout_version: ANALYTIC_SHAPE_LAYOUT_VERSION,
                    body: id(0, 1),
                    kind: AnalyticShapeKindV1::Circle,
                    property_payload: Vec2::new(0.5, 0.5),
                    metadata_flag: ANALYTIC_SHAPE_FLAG_CIRCLE,
                    property_lane_stride_bytes: 16,
                    metadata_lane_stride_bytes: 16,
                },
                AnalyticShapeEncodingV1 {
                    schema_version: SCHEMA_VERSION,
                    layout_version: ANALYTIC_SHAPE_LAYOUT_VERSION,
                    body: id(1, 1),
                    kind: AnalyticShapeKindV1::Rectangle,
                    property_payload: Vec2::new(0.5, 0.25),
                    metadata_flag: ANALYTIC_SHAPE_FLAG_RECTANGLE,
                    property_lane_stride_bytes: 16,
                    metadata_lane_stride_bytes: 16,
                },
            ],
            aabbs: vec![
                BodyAabbV1 {
                    body: id(0, 1),
                    lower: Vec2::new(-0.5, -0.5),
                    upper: Vec2::new(0.5, 0.5),
                },
                BodyAabbV1 {
                    body: id(1, 1),
                    lower: Vec2::new(0.4, -0.25),
                    upper: Vec2::new(1.4, 0.25),
                },
            ],
            candidates: vec![pair],
            manifolds: vec![CollisionManifoldV1 {
                pair,
                normal: Vec2::new(1.0, 0.0),
                material: MaterialV1 {
                    friction: finite(0.5),
                    restitution: finite(0.0),
                },
                points: vec![point],
            }],
            cache: vec![PersistentContactV1 {
                key: point.key(pair),
                value: ContactCacheValueV1 {
                    normal_impulse: finite(0.25),
                    tangent_impulse: finite(-0.05),
                    last_seen_step: 1,
                },
            }],
            work: CollisionWorkV1 {
                bodies: 2,
                pair_attempts: 1,
                candidate_pairs: 1,
                narrowphase_tests: 1,
                manifolds: 1,
                contact_points: 1,
                sort_elements: 1,
                cache_current: 1,
                cache_cold: 1,
                ..Default::default()
            },
            capacities: vec![CollisionCapacityV1 {
                class: "candidates".to_owned(),
                stride_bytes: CANDIDATE_PAIR_BYTES,
                capacity_elements: 4,
                attempted_elements: 1,
                committed_elements: 1,
                allocated_bytes: 64,
                high_water_bytes: 16,
                overflowed: false,
                accepted: true,
            }],
            traces: Vec::new(),
            failure: None,
        }
    }

    #[test]
    fn collision_contract_round_trips_with_complete_byte_table() {
        let artifact = artifact();
        artifact.validate().unwrap();
        let bytes = canonical_json(&artifact).unwrap();
        let decoded: CollisionArtifactV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, artifact);
        assert_eq!(decoded.layout, collision_layout_v1());
        assert_eq!(decoded.layout.records.len(), 8);
    }

    #[test]
    fn candidate_only_work_is_valid_but_over_testing_is_not() {
        let mut work = CollisionWorkV1 {
            bodies: 3,
            pair_attempts: 3,
            fixed_fixed_rejected: 1,
            aabb_rejected: 1,
            candidate_pairs: 1,
            ..CollisionWorkV1::default()
        };
        work.validate().unwrap();
        work.narrowphase_tests = 2;
        assert!(work.validate().is_err());
    }

    #[test]
    fn stale_identity_and_noncanonical_pairs_are_rejected() {
        let mut stale = artifact();
        stale.candidates[0].body_a.generation = 2;
        assert!(stale.validate().unwrap_err().0.contains("missing or stale"));

        let mut reversed = artifact();
        let candidate = &mut reversed.candidates[0];
        std::mem::swap(&mut candidate.body_a, &mut candidate.body_b);
        assert!(reversed.validate().unwrap_err().0.contains("canonical"));
    }

    #[test]
    fn capacities_reject_silent_or_partial_overflow() {
        let mut overflow = artifact();
        overflow.capacities[0].attempted_elements = 5;
        assert!(overflow.validate().is_err());
        overflow.capacities[0].committed_elements = 0;
        overflow.capacities[0].high_water_bytes = 64;
        overflow.capacities[0].overflowed = true;
        overflow.capacities[0].accepted = false;
        overflow.failure = Some(FailureV1 {
            kind: FailureKindV1::CapacityOverflow,
            reason: "candidate capacity overflow".to_owned(),
            first_step: Some(1),
            first_stage: Some("candidate".to_owned()),
            reproducer_hash: Some("2".repeat(64)),
        });
        overflow.validate().unwrap();

        let mut cascaded = artifact().capacities[0].clone();
        cascaded.committed_elements = 0;
        cascaded.accepted = false;
        cascaded.validate().unwrap();
    }

    #[test]
    fn feature_ranges_and_shape_flags_are_explicit() {
        let mut invalid = artifact();
        invalid.manifolds[0].points[0].feature_a.index = 4;
        assert!(invalid.validate().unwrap_err().0.contains("feature index"));

        let mut encoding = artifact().shape_encodings[0];
        encoding.metadata_flag = ANALYTIC_SHAPE_FLAG_RECTANGLE;
        assert!(encoding.validate().unwrap_err().0.contains("shape flag"));
    }
}
