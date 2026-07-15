use physbox_contracts::{ContactCacheValueV1, FiniteF32, PersistentContactV1, Vec2};
use physbox_engine::{
    GpuPersistenceRunner, P5_2CollisionScenarioKind, p5_2_collision_scenario,
    p5_2_narrowphase_pair_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    contact_persistence::transfer_scalar_persistence,
};

#[test]
fn gpu_persistence_matches_scalar_across_warm_churn_and_recontact() {
    let base =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
    let initial = warm_cache(&base);
    let mut translated = base.clone();
    for body in &mut translated.bodies {
        body.position.x += 2.0;
        body.position.y -= 1.0;
    }
    let mut separated = translated.clone();
    separated.bodies[1].position = Vec2::new(20.0, 20.0);
    let sequence = vec![base.clone(), translated, separated, base];
    let runner = pollster::block_on(GpuPersistenceRunner::new()).unwrap();
    let gpu = runner
        .run_sequence(&sequence, &initial, None, false)
        .unwrap();
    let replay = runner
        .run_sequence(&sequence, &initial, None, false)
        .unwrap();
    let permuted = runner
        .run_sequence(&sequence, &initial, None, true)
        .unwrap();
    assert!(gpu.mapped_after_final_step_only);
    assert_eq!(gpu.steps.len(), sequence.len());
    assert_eq!(gpu.steps, replay.steps);
    assert_eq!(gpu.steps, permuted.steps);

    let mut previous = initial;
    for (index, (scenario, actual)) in sequence.iter().zip(&gpu.steps).enumerate() {
        let candidates = enumerate_scalar_candidates(scenario).unwrap();
        let manifolds = generate_scalar_narrowphase(scenario, &candidates.candidates).unwrap();
        let expected =
            transfer_scalar_persistence(&manifolds, &previous, index as u32 + 1).unwrap();
        assert_eq!(actual.cache, expected.cache, "step {}", index + 1);
        assert_eq!(actual.work.cache_matched, expected.matched);
        assert_eq!(actual.work.cache_cold, expected.cold);
        assert_eq!(actual.work.cache_expired, expected.expired);
        assert_eq!(actual.work.sort_passes, 64);
        assert_eq!(actual.work.dispatches, 212);
        previous = expected.cache;
    }
    assert!(gpu.steps[0].work.cache_matched > 0);
    assert!(gpu.steps[2].cache.is_empty());
    assert!(gpu.steps[2].work.cache_expired > 0);
    assert!(gpu.steps[3].work.cache_cold > 0);
}

#[test]
fn randomized_pair_persistence_matches_scalar_across_feature_transitions() {
    let runner = pollster::block_on(GpuPersistenceRunner::new()).unwrap();
    for seed in 0..8 {
        for kind in [
            P5_2CollisionScenarioKind::CircleCircle,
            P5_2CollisionScenarioKind::CircleRectangle,
            P5_2CollisionScenarioKind::RectangleRectangle,
        ] {
            let base = p5_2_narrowphase_pair_scenario(kind, seed).unwrap();
            let initial = warm_cache(&base);
            let mut transitioned = base.clone();
            transitioned.bodies[1].angle = finite(transitioned.bodies[1].angle.get() + 0.01);
            for body in &mut transitioned.bodies {
                body.position.x += 0.35;
                body.position.y -= 0.2;
            }
            let sequence = [base, transitioned];
            let gpu = runner
                .run_sequence(&sequence, &initial, None, seed % 2 == 1)
                .unwrap();
            let mut previous = initial;
            for (index, (scenario, actual)) in sequence.iter().zip(&gpu.steps).enumerate() {
                let candidates = enumerate_scalar_candidates(scenario).unwrap();
                let manifolds =
                    generate_scalar_narrowphase(scenario, &candidates.candidates).unwrap();
                let expected =
                    transfer_scalar_persistence(&manifolds, &previous, index as u32 + 1).unwrap();
                assert_eq!(actual.cache, expected.cache, "{kind:?} seed {seed}");
                assert_eq!(actual.work.cache_matched, expected.matched);
                assert_eq!(actual.work.cache_cold, expected.cold);
                assert_eq!(actual.work.cache_expired, expected.expired);
                previous = expected.cache;
            }
        }
    }
}

#[test]
fn radix_sort_removes_deliberate_current_input_permutation() {
    let scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17).unwrap();
    let runner = pollster::block_on(GpuPersistenceRunner::new()).unwrap();
    let ordered = runner
        .run_sequence(std::slice::from_ref(&scenario), &[], None, false)
        .unwrap();
    let permuted = runner
        .run_sequence(std::slice::from_ref(&scenario), &[], None, true)
        .unwrap();
    assert_eq!(ordered.steps[0].cache, permuted.steps[0].cache);
    assert_eq!(ordered.steps[0].work, permuted.steps[0].work);
    assert!(permuted.input_permuted_before_sort);
}

#[test]
fn reused_body_generation_never_inherits_impulses() {
    let base =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
    let initial = warm_cache(&base);
    let mut reused = base;
    reused.bodies[1].id.generation += 1;
    let runner = pollster::block_on(GpuPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(std::slice::from_ref(&reused), &initial, None, false)
        .unwrap();
    let step = &output.steps[0];
    assert_eq!(step.work.cache_matched, 0);
    assert_eq!(step.work.cache_cold, step.cache.len() as u64);
    assert_eq!(step.work.cache_expired, initial.len() as u64);
    assert!(step.cache.iter().all(|entry| {
        entry.key.pair.body_b.generation == reused.bodies[1].id.generation
            && entry.value.normal_impulse.get() == 0.0
            && entry.value.tangent_impulse.get() == 0.0
    }));
}

#[test]
fn persistence_overflow_rejects_the_complete_cache() {
    let scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
    let runner = pollster::block_on(GpuPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(std::slice::from_ref(&scenario), &[], Some(1), false)
        .unwrap();
    let step = &output.steps[0];
    assert!(step.cache.is_empty());
    assert_eq!(
        step.failure
            .as_ref()
            .and_then(|failure| failure.first_stage.as_deref()),
        Some("persistence")
    );
    let capacity = step
        .capacities
        .iter()
        .find(|capacity| capacity.class == "persistent-cache")
        .unwrap();
    assert!(capacity.overflowed);
    assert_eq!(capacity.committed_elements, 0);
}

fn warm_cache(scenario: &physbox_contracts::ScenarioV1) -> Vec<PersistentContactV1> {
    let candidates = enumerate_scalar_candidates(scenario).unwrap();
    let manifolds = generate_scalar_narrowphase(scenario, &candidates.candidates).unwrap();
    let mut cache = transfer_scalar_persistence(&manifolds, &[], 0)
        .unwrap()
        .cache;
    for (index, entry) in cache.iter_mut().enumerate() {
        entry.value = ContactCacheValueV1 {
            normal_impulse: finite(1.5 + index as f32),
            tangent_impulse: finite(-0.2 - index as f32 * 0.1),
            last_seen_step: 0,
        };
    }
    cache
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).unwrap()
}
