use std::collections::BTreeSet;

use physbox_contracts::{
    BodyAabbV1, CandidatePairV1, CollisionWorkV1, ScenarioV1, ShapeV1, Validate, Vec2,
    canonical_hash,
};
use physbox_engine::GpuCandidateOutput;
use serde::Serialize;

pub const CANDIDATE_AABB_ABSOLUTE_TOLERANCE: f32 = 2.0e-5;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarCandidateOutput {
    pub source_scenario_hash: String,
    pub aabbs: Vec<BodyAabbV1>,
    pub candidates: Vec<CandidatePairV1>,
    pub work: CollisionWorkV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDifference {
    pub signature: String,
    pub reason: String,
    pub missing: Vec<CandidatePairV1>,
    pub unexpected: Vec<CandidatePairV1>,
}

pub fn enumerate_scalar_candidates(scenario: &ScenarioV1) -> Result<ScalarCandidateOutput, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    if !scenario.manifolds.is_empty()
        || !scenario.contact_cache.is_empty()
        || !scenario.actions.is_empty()
    {
        return Err(
            "scalar candidate stage requires a neutral scenario without manifolds, cache, or actions"
                .to_owned(),
        );
    }
    let aabbs = scenario
        .bodies
        .iter()
        .map(|body| {
            let extent = match body.shape {
                ShapeV1::Circle { radius } => Vec2::new(radius.get(), radius.get()),
                ShapeV1::Rectangle { half_extents } => {
                    let (sine, cosine) = body.angle.get().sin_cos();
                    Vec2::new(
                        cosine.abs() * half_extents.x + sine.abs() * half_extents.y,
                        sine.abs() * half_extents.x + cosine.abs() * half_extents.y,
                    )
                }
            };
            BodyAabbV1 {
                body: body.id,
                lower: Vec2::new(body.position.x - extent.x, body.position.y - extent.y),
                upper: Vec2::new(body.position.x + extent.x, body.position.y + extent.y),
            }
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut work = CollisionWorkV1 {
        bodies: scenario.bodies.len() as u64,
        ..CollisionWorkV1::default()
    };
    for body_a in 0..scenario.bodies.len() {
        for body_b in body_a + 1..scenario.bodies.len() {
            work.pair_attempts += 1;
            let state_a = &scenario.bodies[body_a];
            let state_b = &scenario.bodies[body_b];
            if state_a.inverse_mass.get() == 0.0 && state_b.inverse_mass.get() == 0.0 {
                work.fixed_fixed_rejected += 1;
                continue;
            }
            let bounds_a = aabbs[body_a];
            let bounds_b = aabbs[body_b];
            let overlaps = bounds_a.lower.x <= bounds_b.upper.x
                && bounds_b.lower.x <= bounds_a.upper.x
                && bounds_a.lower.y <= bounds_b.upper.y
                && bounds_b.lower.y <= bounds_a.upper.y;
            if !overlaps {
                work.aabb_rejected += 1;
                continue;
            }
            candidates.push(CandidatePairV1 {
                body_a: state_a.id,
                body_b: state_b.id,
            });
        }
    }
    work.candidate_pairs = candidates.len() as u64;
    work.validate().map_err(|error| error.to_string())?;
    Ok(ScalarCandidateOutput {
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        aabbs,
        candidates,
        work,
    })
}

pub fn compare_gpu_candidates(
    expected: &ScalarCandidateOutput,
    actual: &GpuCandidateOutput,
) -> Result<(), CandidateDifference> {
    if actual.failure.is_some() {
        return Err(difference(
            "capacity",
            "GPU candidate output was rejected by its capacity contract",
            Vec::new(),
            Vec::new(),
        ));
    }
    if expected.source_scenario_hash != actual.source_scenario_hash {
        return Err(difference(
            "source-hash",
            "GPU and scalar outputs do not identify the same scenario",
            Vec::new(),
            Vec::new(),
        ));
    }
    if expected.aabbs.len() != actual.aabbs.len()
        || expected
            .aabbs
            .iter()
            .zip(&actual.aabbs)
            .any(|(left, right)| left.body != right.body)
    {
        return Err(difference(
            "aabb-identity",
            "GPU and scalar AABB identities differ",
            Vec::new(),
            Vec::new(),
        ));
    }
    for (index, (left, right)) in expected.aabbs.iter().zip(&actual.aabbs).enumerate() {
        for (lane, expected_value, actual_value) in [
            ("lower.x", left.lower.x, right.lower.x),
            ("lower.y", left.lower.y, right.lower.y),
            ("upper.x", left.upper.x, right.upper.x),
            ("upper.y", left.upper.y, right.upper.y),
        ] {
            if (expected_value - actual_value).abs() > CANDIDATE_AABB_ABSOLUTE_TOLERANCE {
                return Err(difference(
                    format!("aabb:{index}:{lane}"),
                    format!(
                        "AABB {index} {lane} differed: scalar={expected_value}, GPU={actual_value}"
                    ),
                    Vec::new(),
                    Vec::new(),
                ));
            }
        }
    }
    let expected_pairs = expected.candidates.iter().copied().collect::<BTreeSet<_>>();
    let actual_pairs = actual.candidates.iter().copied().collect::<BTreeSet<_>>();
    let missing = expected_pairs
        .difference(&actual_pairs)
        .copied()
        .collect::<Vec<_>>();
    let unexpected = actual_pairs
        .difference(&expected_pairs)
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(difference(
            "candidate-set",
            format!(
                "candidate sets differ: {} missing, {} unexpected",
                missing.len(),
                unexpected.len()
            ),
            missing,
            unexpected,
        ));
    }
    for (name, expected_value, actual_value) in [
        ("bodies", expected.work.bodies, actual.work.bodies),
        (
            "pair-attempts",
            expected.work.pair_attempts,
            actual.work.pair_attempts,
        ),
        (
            "fixed-fixed-rejected",
            expected.work.fixed_fixed_rejected,
            actual.work.fixed_fixed_rejected,
        ),
        (
            "aabb-rejected",
            expected.work.aabb_rejected,
            actual.work.aabb_rejected,
        ),
        (
            "candidate-pairs",
            expected.work.candidate_pairs,
            actual.work.candidate_pairs,
        ),
    ] {
        if expected_value != actual_value {
            return Err(difference(
                format!("work:{name}"),
                format!(
                    "work counter {name} differed: scalar={expected_value}, GPU={actual_value}"
                ),
                Vec::new(),
                Vec::new(),
            ));
        }
    }
    Ok(())
}

fn difference(
    signature: impl Into<String>,
    reason: impl Into<String>,
    missing: Vec<CandidatePairV1>,
    unexpected: Vec<CandidatePairV1>,
) -> CandidateDifference {
    CandidateDifference {
        signature: signature.into(),
        reason: reason.into(),
        missing,
        unexpected,
    }
}

#[cfg(test)]
mod tests {
    use physbox_engine::{P5_2CandidatePattern, p5_2_candidate_scenario};

    use crate::failure_automation::minimize_failure;

    use super::*;

    #[test]
    fn scalar_candidates_replay_exactly() {
        let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Random, 32, 5).unwrap();
        assert_eq!(
            enumerate_scalar_candidates(&scenario).unwrap(),
            enumerate_scalar_candidates(&scenario).unwrap()
        );
    }

    #[test]
    fn injected_missing_pair_is_minimized_to_two_bodies() {
        let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 24, 7).unwrap();
        let minimized = minimize_failure(scenario, |candidate| {
            candidate.bodies.len() >= 2
                && enumerate_scalar_candidates(candidate)
                    .is_ok_and(|output| !output.candidates.is_empty())
        })
        .unwrap()
        .unwrap();
        assert_eq!(minimized.scenario.bodies.len(), 2);
        assert_eq!(
            enumerate_scalar_candidates(&minimized.scenario)
                .unwrap()
                .candidates
                .len(),
            1
        );
        assert!(minimized.attempts > 1);
    }
}
