use physbox_engine::{
    GpuGeneratedContactRunner, GpuNarrowphaseRunner, GpuSolverCandidate, P5_2CandidatePattern,
    P5_2GeneratedScenarioKind, benchmark_gpu_generated_contacts, p5_2_candidate_scenario,
    p5_2_generated_performance_scenario, p5_2_generated_scenario, run_gpu_soft_step_case,
};
use physbox_lab::analytic_narrowphase::authored_solver_projection;

const PASS_TOLERANCE: f32 = 2.0e-5;

#[test]
fn generated_contact_benchmark_uses_qualified_split_scopes() {
    let scenario = p5_2_generated_performance_scenario(32).unwrap();
    let benchmark = pollster::block_on(benchmark_gpu_generated_contacts(&scenario, 1, 2)).unwrap();
    assert!(benchmark.correctness.failure.is_none());
    assert_eq!(benchmark.dispatches_per_step, 282);
    assert_eq!(benchmark.host_completion_seconds.len(), 2);
    assert_eq!(benchmark.cpu_encode_submit_seconds.len(), 2);
    match (
        benchmark.gpu_collision_seconds,
        benchmark.gpu_end_to_end_seconds,
        benchmark.timestamp_period_nanoseconds,
    ) {
        (Some(collision), Some(end_to_end), Some(period)) => {
            assert_eq!(collision.len(), 2);
            assert_eq!(end_to_end.len(), 2);
            assert!(period > 0.0);
            assert!(
                collision
                    .iter()
                    .zip(&end_to_end)
                    .all(|(collision, end_to_end)| collision <= end_to_end)
            );
        }
        (None, None, None) => {}
        mismatch => panic!("inconsistent generated timestamp evidence: {mismatch:?}"),
    }
}

#[test]
fn generated_bridge_matches_the_scalar_qualified_authored_solver() {
    let generated = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let narrowphase = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    for kind in [
        P5_2GeneratedScenarioKind::BilateralCircles,
        P5_2GeneratedScenarioKind::OffCenterRectangles,
        P5_2GeneratedScenarioKind::FrictionSlide,
        P5_2GeneratedScenarioKind::Restitution,
    ] {
        let source = p5_2_generated_scenario(kind, 1).unwrap();
        let contacts = narrowphase.run(&source, None, None).unwrap();
        assert!(contacts.failure.is_none());
        assert!(!contacts.manifolds.is_empty(), "{kind:?}");
        let authored = authored_solver_projection(&source, &contacts.manifolds).unwrap();
        let expected = pollster::block_on(run_gpu_soft_step_case(
            &authored,
            GpuSolverCandidate::JacobiGather,
        ))
        .unwrap();
        let actual = generated.run(&source, 1).unwrap();
        for (actual, expected) in actual.bodies.iter().zip(&expected.bodies) {
            for (actual, expected) in [
                (actual.position.x, expected.position.x),
                (actual.position.y, expected.position.y),
                (actual.angle.get(), expected.angle.get()),
                (actual.linear_velocity.x, expected.linear_velocity.x),
                (actual.linear_velocity.y, expected.linear_velocity.y),
                (
                    actual.angular_velocity.get(),
                    expected.angular_velocity.get(),
                ),
            ] {
                assert!((actual - expected).abs() <= PASS_TOLERANCE, "{kind:?}");
            }
        }
        assert_eq!(actual.cache.len(), expected.contact_cache.len(), "{kind:?}");
        for (actual, expected) in actual.cache.iter().zip(&expected.contact_cache) {
            assert_eq!(actual.key.pair.body_a, expected.body_a);
            assert_eq!(actual.key.pair.body_b, expected.body_b);
            assert_eq!(actual.key.feature_a.index, expected.feature_a.feature_index);
            assert_eq!(actual.key.feature_b.index, expected.feature_b.feature_index);
            assert!(
                (actual.value.normal_impulse.get() - expected.normal_impulse.get()).abs()
                    <= PASS_TOLERANCE,
                "{kind:?}"
            );
            assert!(
                (actual.value.tangent_impulse.get() - expected.tangent_impulse.get()).abs()
                    <= PASS_TOLERANCE,
                "{kind:?}"
            );
        }
    }
}

#[test]
fn generated_floor_contacts_settle_and_replay_exactly() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for (kind, expected_height, steps) in [
        (P5_2GeneratedScenarioKind::RectangleFloor, 0.2, 60),
        (P5_2GeneratedScenarioKind::CircleFloor, 0.5, 60),
    ] {
        let scenario = p5_2_generated_scenario(kind, steps).unwrap();
        let first = runner.run(&scenario, steps).unwrap();
        let replay = runner.run(&scenario, steps).unwrap();
        assert_eq!(first.bodies, replay.bodies);
        assert_eq!(first.cache, replay.cache);
        assert_eq!(first.work, replay.work);
        assert!(first.failure.is_none());
        assert_eq!(first.dispatches_per_step, 282);
        assert!(
            (first.bodies[0].position.y - expected_height).abs() <= 2.0e-2,
            "{kind:?}: {:?}",
            first.bodies[0]
        );
        assert!(
            first.bodies[0].linear_velocity.y.abs() <= 2.0e-2,
            "{kind:?}: {:?}",
            first.bodies[0]
        );
        assert!(!first.cache.is_empty());
        assert!(
            first
                .cache
                .iter()
                .all(|contact| contact.value.last_seen_step == steps)
        );
        assert!(
            first
                .cache
                .iter()
                .any(|contact| contact.value.normal_impulse.get() > 0.0)
        );
    }
}

#[test]
fn generated_impacts_rotation_friction_restitution_and_churn_remain_finite() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for (kind, steps) in [
        (P5_2GeneratedScenarioKind::BilateralCircles, 1),
        (P5_2GeneratedScenarioKind::OffCenterRectangles, 4),
        (P5_2GeneratedScenarioKind::SmallStack, 12),
        (P5_2GeneratedScenarioKind::FrictionSlide, 12),
        (P5_2GeneratedScenarioKind::Restitution, 8),
        (P5_2GeneratedScenarioKind::PairChurn, 12),
    ] {
        let scenario = p5_2_generated_scenario(kind, steps).unwrap();
        let output = runner.run(&scenario, steps).unwrap();
        assert!(output.failure.is_none(), "{kind:?}: {:?}", output.failure);
        assert!(output.capacities.iter().all(|capacity| capacity.accepted));
        assert!(output.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.angle.get().is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        }));
    }
}

#[test]
fn generated_contacts_have_expected_structural_responses() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();

    let bilateral =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::BilateralCircles, 1).unwrap();
    let bilateral = runner.run(&bilateral, 1).unwrap();
    assert!(
        bilateral.bodies[0].linear_velocity.x < 1.0,
        "bilateral bodies: {:?}; work: {:?}; cache: {:?}",
        bilateral.bodies,
        bilateral.work,
        bilateral.cache
    );
    assert!(
        bilateral.bodies[1].linear_velocity.x > -1.0,
        "bilateral bodies: {:?}",
        bilateral.bodies
    );

    let off_center =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::OffCenterRectangles, 4).unwrap();
    let off_center = runner.run(&off_center, 4).unwrap();
    assert!(
        off_center
            .bodies
            .iter()
            .any(|body| body.angular_velocity.get().abs() > 1.0e-3)
    );

    let friction = p5_2_generated_scenario(P5_2GeneratedScenarioKind::FrictionSlide, 12).unwrap();
    let friction = runner.run(&friction, 12).unwrap();
    assert!(friction.bodies[0].linear_velocity.x.abs() < 2.0);

    let restitution = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 8).unwrap();
    let restitution = runner.run(&restitution, 8).unwrap();
    assert!(restitution.bodies[0].linear_velocity.y > 0.0);

    let stack = p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 12).unwrap();
    let stack = runner.run(&stack, 12).unwrap();
    for pair in stack.bodies[1..].windows(2) {
        assert!(pair[0].position.y < pair[1].position.y);
    }
}

#[test]
fn generated_small_stack_long_exact_run_completes() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 180).unwrap();
    let output = runner.run(&scenario, 180).unwrap();
    assert!(output.failure.is_none());
    assert!(output.capacities.iter().all(|capacity| capacity.accepted));
}

#[test]
fn generated_randomized_worlds_replay_and_dense_overflow_is_atomic() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for pattern in [
        P5_2CandidatePattern::Random,
        P5_2CandidatePattern::Touching,
        P5_2CandidatePattern::RotatedThin,
        P5_2CandidatePattern::FixedOnly,
    ] {
        for seed in [19, 71] {
            let scenario = p5_2_candidate_scenario(pattern, 16, seed).unwrap();
            let first = runner.run(&scenario, 2).unwrap();
            let replay = runner.run(&scenario, 2).unwrap();
            assert_eq!(first.bodies, replay.bodies, "{pattern:?} seed {seed}");
            assert_eq!(first.cache, replay.cache, "{pattern:?} seed {seed}");
            assert!(first.failure.is_none(), "{pattern:?} seed {seed}");
        }
    }

    let dense = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 34, 97).unwrap();
    let rejected = runner.run(&dense, 1).unwrap();
    assert!(rejected.failure.is_some());
    assert!(rejected.cache.is_empty());
    let candidate = rejected
        .capacities
        .iter()
        .find(|capacity| capacity.class == "candidate-pairs")
        .unwrap();
    assert!(candidate.overflowed);
    assert!(!candidate.accepted);
    assert_eq!(candidate.committed_elements, 0);
}

#[test]
fn generated_dense_coincident_reproducer_replays_exactly() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for body_count in 2..=16 {
        let scenario =
            p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, body_count, 19).unwrap();
        let first = runner.run(&scenario, 1).unwrap();
        let replay = runner.run(&scenario, 1).unwrap();
        assert_eq!(first.bodies, replay.bodies, "body_count {body_count}");
    }
}
