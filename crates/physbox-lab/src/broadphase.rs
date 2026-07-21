use std::collections::BTreeSet;

use physbox_contracts::{
    BODY_AABB_BYTES, BROADPHASE_CONTRACT_VERSION, BroadphaseAlgorithmV1, BroadphaseEvaluationV1,
    BroadphaseMobilityV1, BroadphaseResultV1, BroadphaseSnapshotV1, BroadphaseWorkV1,
    CANDIDATE_PAIR_BYTES, CandidatePairV1, CollisionCapacityV1, FailureKindV1, FailureV1,
    FiniteF32, SCHEMA_VERSION, Validate, canonical_hash,
};

pub fn brute_force_broadphase(
    snapshot: &BroadphaseSnapshotV1,
    requested_candidate_capacity: Option<u64>,
) -> Result<BroadphaseResultV1, String> {
    snapshot.validate().map_err(|error| error.to_string())?;
    if requested_candidate_capacity == Some(0) {
        return Err("broadphase candidate capacity must be positive".to_owned());
    }
    let source_snapshot_hash = canonical_hash(snapshot).map_err(|error| error.to_string())?;
    let mut pairs = Vec::new();
    let mut work = BroadphaseWorkV1 {
        proxies: snapshot.proxies.len() as u64,
        dynamic_proxies: snapshot
            .proxies
            .iter()
            .filter(|proxy| proxy.mobility == BroadphaseMobilityV1::Dynamic)
            .count() as u64,
        fixed_proxies: snapshot
            .proxies
            .iter()
            .filter(|proxy| proxy.mobility == BroadphaseMobilityV1::Fixed)
            .count() as u64,
        ..Default::default()
    };
    for (index_a, proxy_a) in snapshot.proxies.iter().enumerate() {
        for proxy_b in &snapshot.proxies[index_a + 1..] {
            work.pair_tests += 1;
            if proxy_a.mobility == BroadphaseMobilityV1::Fixed
                && proxy_b.mobility == BroadphaseMobilityV1::Fixed
            {
                work.fixed_fixed_rejected += 1;
                continue;
            }
            if overlaps(proxy_a.aabb, proxy_b.aabb) {
                pairs.push(CandidatePairV1 {
                    body_a: proxy_a.aabb.body,
                    body_b: proxy_b.aabb.body,
                });
            }
        }
    }
    work.candidate_emissions = pairs.len() as u64;
    work.unique_candidates = pairs.len() as u64;
    let candidate_capacity = requested_candidate_capacity.unwrap_or(work.unique_candidates.max(1));
    let overflowed = work.unique_candidates > candidate_capacity;
    let committed = if overflowed {
        pairs.clear();
        0
    } else {
        work.unique_candidates
    };
    let fixed = |class: &str, stride_bytes: u32, elements: u64| CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes,
        capacity_elements: elements,
        attempted_elements: elements,
        committed_elements: elements,
        allocated_bytes: elements * u64::from(stride_bytes),
        high_water_bytes: elements * u64::from(stride_bytes),
        overflowed: false,
        accepted: true,
    };
    let mut capacities = vec![
        CollisionCapacityV1 {
            class: "candidate-pairs".to_owned(),
            stride_bytes: CANDIDATE_PAIR_BYTES,
            capacity_elements: candidate_capacity,
            attempted_elements: work.unique_candidates,
            committed_elements: committed,
            allocated_bytes: candidate_capacity * u64::from(CANDIDATE_PAIR_BYTES),
            high_water_bytes: work.unique_candidates.min(candidate_capacity)
                * u64::from(CANDIDATE_PAIR_BYTES),
            overflowed,
            accepted: !overflowed,
        },
        fixed("capacity-counter", 16, 1),
        fixed("input-aabbs", BODY_AABB_BYTES, work.proxies),
    ];
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    let failure = overflowed.then(|| FailureV1 {
        kind: FailureKindV1::CapacityOverflow,
        reason: format!(
            "brute-force candidate capacity {candidate_capacity} rejected {} attempted pairs",
            work.unique_candidates
        ),
        first_step: Some(0),
        first_stage: Some("broadphase-brute-force".to_owned()),
        reproducer_hash: Some(source_snapshot_hash.clone()),
    });
    let result = BroadphaseResultV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: BROADPHASE_CONTRACT_VERSION,
        algorithm: BroadphaseAlgorithmV1::BruteForce,
        source_snapshot_hash,
        pairs,
        work,
        capacities,
        failure,
    };
    result.validate().map_err(|error| error.to_string())?;
    Ok(result)
}

pub fn evaluate_broadphase(
    oracle: &BroadphaseResultV1,
    candidate: &BroadphaseResultV1,
    exact_replay: bool,
) -> Result<BroadphaseEvaluationV1, String> {
    oracle.validate().map_err(|error| error.to_string())?;
    candidate.validate().map_err(|error| error.to_string())?;
    if oracle.algorithm != BroadphaseAlgorithmV1::BruteForce {
        return Err("broadphase evaluation requires the brute-force authority".to_owned());
    }
    if oracle.failure.is_some() {
        return Err("broadphase authority must be capacity-clean".to_owned());
    }
    if oracle.source_snapshot_hash != candidate.source_snapshot_hash {
        return Err("broadphase authority and candidate snapshot hashes differ".to_owned());
    }
    let expected = oracle.pairs.iter().copied().collect::<BTreeSet<_>>();
    let actual = candidate.pairs.iter().copied().collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
    let capacity_clean = candidate.failure.is_none()
        && candidate
            .capacities
            .iter()
            .all(|capacity| !capacity.overflowed && capacity.accepted);
    let evaluation = BroadphaseEvaluationV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: BROADPHASE_CONTRACT_VERSION,
        algorithm: candidate.algorithm,
        oracle_pairs: expected.len() as u64,
        candidate_pairs: actual.len() as u64,
        missing_pairs: missing.len() as u64,
        unexpected_pairs: unexpected.len() as u64,
        first_missing_pair: missing.first().copied(),
        first_unexpected_pair: unexpected.first().copied(),
        candidate_amplification: FiniteF32::new(actual.len() as f32 / expected.len().max(1) as f32)
            .map_err(|error| error.to_string())?,
        capacity_clean,
        exact_replay,
        accepted: missing.is_empty() && capacity_clean && exact_replay,
    };
    evaluation.validate().map_err(|error| error.to_string())?;
    Ok(evaluation)
}

fn overlaps(left: physbox_contracts::BodyAabbV1, right: physbox_contracts::BodyAabbV1) -> bool {
    left.lower.x <= right.upper.x
        && right.lower.x <= left.upper.x
        && left.lower.y <= right.upper.y
        && right.lower.y <= left.upper.y
}

#[cfg(test)]
mod tests {
    use physbox_engine::{P5_3BroadphasePattern, p5_3_broadphase_snapshot};

    use super::*;

    #[test]
    fn brute_force_authority_replays_every_broadphase_family() {
        for pattern in P5_3BroadphasePattern::ALL {
            let snapshot = p5_3_broadphase_snapshot(pattern, 32, 0x5030).unwrap();
            let first = brute_force_broadphase(&snapshot, None).unwrap();
            let replay = brute_force_broadphase(&snapshot, None).unwrap();
            assert_eq!(first, replay, "{}", pattern.name());
            assert_eq!(
                first.work.pair_tests,
                u64::from(32_u32 * 31 / 2),
                "{}",
                pattern.name()
            );
            assert!(first.failure.is_none(), "{}", pattern.name());
        }
    }

    #[test]
    fn brute_force_overflow_rejects_every_pair_atomically() {
        let snapshot =
            p5_3_broadphase_snapshot(P5_3BroadphasePattern::CompleteOverlap, 16, 7).unwrap();
        let output = brute_force_broadphase(&snapshot, Some(1)).unwrap();
        assert!(output.pairs.is_empty());
        assert!(output.failure.is_some());
        let capacity = output
            .capacities
            .iter()
            .find(|capacity| capacity.class == "candidate-pairs")
            .unwrap();
        assert!(capacity.overflowed);
        assert_eq!(capacity.committed_elements, 0);
    }

    #[test]
    fn evaluation_records_the_first_missing_pair() {
        let snapshot = p5_3_broadphase_snapshot(P5_3BroadphasePattern::Packed, 8, 11).unwrap();
        let oracle = brute_force_broadphase(&snapshot, None).unwrap();
        assert!(!oracle.pairs.is_empty());
        let mut broken = oracle.clone();
        broken.algorithm = BroadphaseAlgorithmV1::SweepAndPrune;
        broken.pairs.remove(0);
        broken.work.unique_candidates -= 1;
        broken.work.candidate_emissions -= 1;
        let candidate_capacity = broken
            .capacities
            .iter_mut()
            .find(|capacity| capacity.class == "candidate-pairs")
            .unwrap();
        candidate_capacity.attempted_elements -= 1;
        candidate_capacity.committed_elements -= 1;
        candidate_capacity.high_water_bytes -= u64::from(CANDIDATE_PAIR_BYTES);
        let evaluation = evaluate_broadphase(&oracle, &broken, true).unwrap();
        assert_eq!(evaluation.missing_pairs, 1);
        assert_eq!(evaluation.first_missing_pair, oracle.pairs.first().copied());
        assert!(!evaluation.accepted);
    }
}
