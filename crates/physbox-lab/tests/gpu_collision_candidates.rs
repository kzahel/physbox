use physbox_engine::{
    GpuCandidateRunner, P5_2CandidatePattern, P5_2CollisionScenarioKind, p5_2_candidate_scenario,
    p5_2_collision_scenario,
};
use physbox_lab::collision_candidates::{compare_gpu_candidates, enumerate_scalar_candidates};

#[test]
fn gpu_candidates_exactly_match_complete_scalar_enumeration() {
    let runner = pollster::block_on(GpuCandidateRunner::new()).unwrap();
    for kind in P5_2CollisionScenarioKind::ALL {
        let scenario = p5_2_collision_scenario(kind, 1, 17).unwrap();
        let expected = enumerate_scalar_candidates(&scenario).unwrap();
        let actual = runner.run(&scenario, None).unwrap();
        compare_gpu_candidates(&expected, &actual).unwrap();
    }
    for seed in 0..16_u64 {
        for pattern in P5_2CandidatePattern::ALL {
            let body_count = 2 + ((seed * 17 + pattern as u64 * 7) % 63) as u32;
            let scenario = p5_2_candidate_scenario(pattern, body_count, seed).unwrap();
            let expected = enumerate_scalar_candidates(&scenario).unwrap();
            let actual = runner.run(&scenario, None).unwrap();
            compare_gpu_candidates(&expected, &actual).unwrap_or_else(|difference| {
                panic!(
                    "{} seed {seed}, {body_count} bodies: {}: {}",
                    pattern.name(),
                    difference.signature,
                    difference.reason
                )
            });
            assert_eq!(actual.work.dispatches, 8);
            assert!(actual.capacities.iter().all(|capacity| {
                !capacity.overflowed
                    && capacity.accepted
                    && capacity.committed_elements == capacity.attempted_elements
            }));
        }
    }
}

#[test]
fn candidate_overflow_rejects_the_whole_prefix() {
    let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 16, 29).unwrap();
    let expected = enumerate_scalar_candidates(&scenario).unwrap();
    assert!(expected.candidates.len() > 1);
    let runner = pollster::block_on(GpuCandidateRunner::new()).unwrap();
    let output = runner.run(&scenario, Some(1)).unwrap();
    let capacity = output
        .capacities
        .iter()
        .find(|capacity| capacity.class == "candidate-pairs")
        .unwrap();
    assert!(capacity.overflowed);
    assert!(!capacity.accepted);
    assert_eq!(
        capacity.attempted_elements,
        expected.candidates.len() as u64
    );
    assert_eq!(capacity.committed_elements, 0);
    assert!(output.candidates.is_empty());
    assert!(output.failure.is_some());
}
