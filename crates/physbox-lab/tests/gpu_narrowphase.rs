use physbox_contracts::Vec2;
use physbox_engine::{
    GpuNarrowphaseRunner, P5_2CandidatePattern, P5_2CollisionScenarioKind, p5_2_candidate_scenario,
    p5_2_collision_scenario, p5_2_narrowphase_pair_scenario,
};
use physbox_lab::{
    analytic_narrowphase::{compare_narrowphase, generate_scalar_narrowphase},
    collision_candidates::enumerate_scalar_candidates,
};

#[test]
fn gpu_narrowphase_matches_scalar_for_all_governed_pairs() {
    let runner = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    for kind in P5_2CollisionScenarioKind::ALL {
        let scenario = p5_2_collision_scenario(kind, 1, 17).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let expected = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let actual = runner.run(&scenario, None, None).unwrap();
        let replay = runner.run(&scenario, None, None).unwrap();
        compare_narrowphase(&expected, &actual.manifolds).unwrap_or_else(|difference| {
            panic!("{kind:?}: {}: {}", difference.signature, difference.reason)
        });
        assert_eq!(
            actual.work.narrowphase_tests,
            candidates.candidates.len() as u64
        );
        assert_eq!(actual.work.manifolds, expected.len() as u64);
        assert_eq!(actual.work.dispatches, 15);
        assert!(actual.failure.is_none());
        assert_eq!(actual.manifolds, replay.manifolds);
        assert_eq!(actual.work, replay.work);
        assert_eq!(actual.capacities, replay.capacities);
    }
}

#[test]
fn gpu_circle_contact_creation_and_removal_boundary_is_exact() {
    let mut scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, 1, 17).unwrap();
    scenario.bodies[0].position = Vec2::ZERO;
    scenario.bodies[1].position = Vec2::new(0.7, 0.0);
    let runner = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    let touching = runner.run(&scenario, None, None).unwrap();
    assert_eq!(touching.manifolds.len(), 1);
    assert!(touching.manifolds[0].points[0].separation.get().abs() <= 1.0e-7);

    scenario.bodies[1].position.x = 0.7001;
    let separated = runner.run(&scenario, None, None).unwrap();
    assert!(separated.manifolds.is_empty());

    scenario.bodies[1].position.x = 0.6999;
    let penetrating = runner.run(&scenario, None, None).unwrap();
    assert!(penetrating.manifolds[0].points[0].separation.get() < 0.0);
}

#[test]
fn gpu_narrowphase_matches_randomized_and_adversarial_scalar_cases() {
    let runner = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    for seed in 0..64_u64 {
        for pattern in P5_2CandidatePattern::ALL {
            let body_count = 2 + ((seed * 11 + pattern as u64 * 5) % 31) as u32;
            let scenario = p5_2_candidate_scenario(pattern, body_count, seed).unwrap();
            let candidates = enumerate_scalar_candidates(&scenario).unwrap();
            let expected = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
            let actual = runner.run(&scenario, None, None).unwrap();
            compare_narrowphase(&expected, &actual.manifolds).unwrap_or_else(|difference| {
                panic!(
                    "{} seed {seed}, {body_count} bodies: {}: {}",
                    pattern.name(),
                    difference.signature,
                    difference.reason
                )
            });
        }
    }
}

#[test]
fn gpu_narrowphase_matches_randomized_overlapping_pair_families() {
    let runner = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    for seed in 0..32_u64 {
        for kind in P5_2CollisionScenarioKind::ALL[..3].iter().copied() {
            let scenario = p5_2_narrowphase_pair_scenario(kind, seed).unwrap();
            let candidates = enumerate_scalar_candidates(&scenario).unwrap();
            let expected = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
            assert!(!expected.is_empty(), "{kind:?} seed {seed} did not overlap");
            let actual = runner.run(&scenario, None, None).unwrap();
            compare_narrowphase(&expected, &actual.manifolds).unwrap_or_else(|difference| {
                panic!(
                    "{kind:?} seed {seed}: {}: {}",
                    difference.signature, difference.reason
                )
            });
        }
    }
}

#[test]
fn narrowphase_overflow_rejects_manifolds_and_points_together() {
    let scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17).unwrap();
    let runner = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    let output = runner.run(&scenario, None, Some(1)).unwrap();
    assert!(output.work.manifolds > 1);
    assert!(output.manifolds.is_empty());
    assert_eq!(
        output
            .failure
            .as_ref()
            .and_then(|failure| failure.first_stage.as_deref()),
        Some("narrowphase")
    );
    let manifold_capacity = output
        .capacities
        .iter()
        .find(|capacity| capacity.class == "contact-manifolds")
        .unwrap();
    let point_capacity = output
        .capacities
        .iter()
        .find(|capacity| capacity.class == "contact-points")
        .unwrap();
    assert!(manifold_capacity.overflowed);
    assert!(!point_capacity.overflowed);
    assert!(!point_capacity.accepted);
    assert_eq!(manifold_capacity.committed_elements, 0);
    assert_eq!(point_capacity.committed_elements, 0);
}
