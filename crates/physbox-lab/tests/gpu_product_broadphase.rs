use physbox_engine::{
    GpuSweepAndPruneRunner, P5_2CandidatePattern, P5_2CollisionScenarioKind,
    p5_2_candidate_scenario, p5_2_collision_scenario,
};
use physbox_lab::collision_candidates::{
    CANDIDATE_AABB_ABSOLUTE_TOLERANCE, enumerate_scalar_candidates,
};

#[test]
fn product_sap_matches_scalar_pairs_and_aabbs() {
    let runner = pollster::block_on(GpuSweepAndPruneRunner::new()).unwrap();
    let mut scenarios = P5_2CollisionScenarioKind::ALL
        .into_iter()
        .map(|kind| p5_2_collision_scenario(kind, 1, 17).unwrap())
        .collect::<Vec<_>>();
    scenarios.extend(
        P5_2CandidatePattern::ALL
            .into_iter()
            .map(|pattern| p5_2_candidate_scenario(pattern, 32, 0x5030).unwrap()),
    );

    for scenario in scenarios {
        let body_count = scenario.bodies.len() as u32;
        let possible_pairs = body_count * (body_count - 1) / 2;
        let candidate_capacity = possible_pairs.min(body_count * 16).max(1);
        let expected = enumerate_scalar_candidates(&scenario).unwrap();
        let actual = runner
            .run_product_scenario(&scenario, candidate_capacity)
            .unwrap_or_else(|error| panic!("{}: {error}", scenario.id));
        assert_eq!(actual.candidates, expected.candidates, "{}", scenario.id);
        assert_eq!(actual.aabbs.len(), expected.aabbs.len(), "{}", scenario.id);
        for (expected, actual) in expected.aabbs.iter().zip(&actual.aabbs) {
            assert_eq!(actual.body, expected.body, "{}", scenario.id);
            for (expected, actual) in [
                (expected.lower.x, actual.lower.x),
                (expected.lower.y, actual.lower.y),
                (expected.upper.x, actual.upper.x),
                (expected.upper.y, actual.upper.y),
            ] {
                assert!(
                    (expected - actual).abs() <= CANDIDATE_AABB_ABSOLUTE_TOLERANCE,
                    "{} AABB differed: expected={expected}, actual={actual}",
                    scenario.id
                );
            }
        }
        assert_eq!(actual.work.dispatches, 59, "{}", scenario.id);
        assert!(actual.failure.is_none(), "{}", scenario.id);
        assert!(actual.capacities.iter().all(|capacity| capacity.accepted));
    }
}

#[test]
fn product_sap_rejects_capacity_overflow_atomically() {
    let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 16, 29).unwrap();
    let runner = pollster::block_on(GpuSweepAndPruneRunner::new()).unwrap();
    let output = runner.run_product_scenario(&scenario, 1).unwrap();
    let capacity = output
        .capacities
        .iter()
        .find(|capacity| capacity.class == "candidate-pairs")
        .unwrap();
    assert!(capacity.overflowed);
    assert!(!capacity.accepted);
    assert_eq!(capacity.committed_elements, 0);
    assert!(output.candidates.is_empty());
    assert!(output.failure.is_some());
}
