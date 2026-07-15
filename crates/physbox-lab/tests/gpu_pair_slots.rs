use std::collections::BTreeMap;

use physbox_contracts::{
    ContactCacheValueV1, FiniteF32, GeometricFeatureKindV1, PersistentContactV1, Vec2,
};
use physbox_engine::{
    GpuPairSlotPersistenceRunner, P5_2CollisionScenarioKind, p5_2_collision_scenario,
    p5_2_narrowphase_pair_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    contact_persistence::transfer_scalar_persistence,
};

const FULL_FINGERPRINT: u32 = u32::MAX;

#[test]
fn pair_slots_match_scalar_across_warm_churn_and_recontact() {
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
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
    let gpu = runner
        .run_sequence(&sequence, &initial, None, false, FULL_FINGERPRINT)
        .unwrap();
    let replay = runner
        .run_sequence(&sequence, &initial, None, false, FULL_FINGERPRINT)
        .unwrap();
    let permuted = runner
        .run_sequence(&sequence, &initial, None, true, FULL_FINGERPRINT)
        .unwrap();
    assert!(gpu.mapped_after_final_step_only);
    assert_eq!(gpu.steps, replay.steps);
    assert_eq!(gpu.steps, permuted.steps);

    let mut previous = initial;
    for (index, (scenario, actual)) in sequence.iter().zip(&gpu.steps).enumerate() {
        let expected = scalar_step(scenario, &previous, index as u32 + 1);
        assert_eq!(actual.cache, expected.cache, "step {}", index + 1);
        assert_eq!(actual.work.cache_matched, expected.matched);
        assert_eq!(actual.work.cache_cold, expected.cold);
        assert_eq!(actual.work.cache_expired, expected.expired);
        assert_eq!(actual.work.sort_passes, 0);
        assert_eq!(actual.work.dispatches, 25);
        previous = expected.cache;
    }

    let first = gpu.steps[0].pair_slots[0];
    assert_eq!(gpu.steps[1].pair_slots[0], first);
    assert!(gpu.steps[2].pair_slots.is_empty());
    let recontact = gpu.steps[3].pair_slots[0];
    assert_eq!(recontact.slot, first.slot);
    assert_eq!(recontact.generation, first.generation + 1);
    assert!(gpu.steps[3].work.cache_cold > 0);
}

#[test]
fn randomized_pair_slots_are_scalar_equivalent_and_permutation_invariant() {
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
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
            let ordered = runner
                .run_sequence(&sequence, &initial, None, false, FULL_FINGERPRINT)
                .unwrap();
            let permuted = runner
                .run_sequence(&sequence, &initial, None, true, FULL_FINGERPRINT)
                .unwrap();
            assert_eq!(ordered.steps, permuted.steps, "{kind:?} seed {seed}");
            let mut previous = initial;
            for (index, (scenario, actual)) in sequence.iter().zip(&ordered.steps).enumerate() {
                let expected = scalar_step(scenario, &previous, index as u32 + 1);
                assert_eq!(actual.cache, expected.cache, "{kind:?} seed {seed}");
                previous = expected.cache;
            }
        }
    }
}

#[test]
fn partial_pair_churn_retains_live_slots_and_generationally_recycles_lost_slots() {
    let base = p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17).unwrap();
    let initial = warm_cache(&base);
    let mut churned = base.clone();
    churned.bodies[0].position = Vec2::new(20.0, 20.0);
    let sequence = [base.clone(), churned, base];
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(&sequence, &initial, None, false, FULL_FINGERPRINT)
        .unwrap();
    let mut previous = initial;
    for (index, (scenario, actual)) in sequence.iter().zip(&output.steps).enumerate() {
        let expected = scalar_step(scenario, &previous, index as u32 + 1);
        assert_eq!(actual.cache, expected.cache);
        previous = expected.cache;
    }

    let before = slot_map(&output.steps[0].pair_slots);
    let during = slot_map(&output.steps[1].pair_slots);
    let after = slot_map(&output.steps[2].pair_slots);
    assert!(!during.is_empty());
    assert!(during.len() < before.len());
    for (pair, retained) in &during {
        assert_eq!(Some(retained), before.get(pair));
        assert_eq!(Some(retained), after.get(pair));
    }
    let recycled = before
        .iter()
        .filter(|(pair, _)| !during.contains_key(pair))
        .collect::<Vec<_>>();
    assert!(!recycled.is_empty());
    for (pair, old) in recycled {
        let new = after.get(pair).unwrap();
        assert_eq!(new.slot, old.slot);
        assert_eq!(new.generation, old.generation + 1);
    }
}

#[test]
fn full_feature_identity_rejects_a_forced_fingerprint_collision() {
    let scenario = p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, 1, 17).unwrap();
    let mut initial = warm_cache(&scenario);
    initial[0].key.feature_a.kind = GeometricFeatureKindV1::Face;
    initial[0].key.feature_a.index = 3;
    let expected = scalar_step(&scenario, &initial, 1);
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(std::slice::from_ref(&scenario), &initial, None, false, 0)
        .unwrap();
    let actual = &output.steps[0];
    assert_eq!(actual.cache, expected.cache);
    assert_eq!(actual.work.cache_matched, 0);
    assert_eq!(actual.work.cache_cold, actual.cache.len() as u64);
    assert!(actual.cache.iter().all(|entry| {
        entry.value.normal_impulse.get() == 0.0 && entry.value.tangent_impulse.get() == 0.0
    }));
}

#[test]
fn reused_body_generation_never_inherits_pair_slot_impulses() {
    let base =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
    let initial = warm_cache(&base);
    let mut reused = base;
    reused.bodies[1].id.generation += 1;
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(
            std::slice::from_ref(&reused),
            &initial,
            None,
            false,
            FULL_FINGERPRINT,
        )
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
fn pair_slot_overflow_rejects_the_complete_cache() {
    let scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17).unwrap();
    let runner = pollster::block_on(GpuPairSlotPersistenceRunner::new()).unwrap();
    let output = runner
        .run_sequence(
            std::slice::from_ref(&scenario),
            &[],
            Some(1),
            false,
            FULL_FINGERPRINT,
        )
        .unwrap();
    let step = &output.steps[0];
    assert!(step.cache.is_empty());
    assert!(step.pair_slots.is_empty());
    assert_eq!(
        step.failure
            .as_ref()
            .and_then(|failure| failure.first_stage.as_deref()),
        Some("pair-slot-persistence")
    );
    let capacity = step
        .capacities
        .iter()
        .find(|capacity| capacity.class == "pair-slot-cache")
        .unwrap();
    assert!(capacity.overflowed);
    assert_eq!(capacity.committed_elements, 0);
}

fn scalar_step(
    scenario: &physbox_contracts::ScenarioV1,
    previous: &[PersistentContactV1],
    completed_step: u32,
) -> physbox_lab::contact_persistence::ScalarPersistenceStep {
    let candidates = enumerate_scalar_candidates(scenario).unwrap();
    let manifolds = generate_scalar_narrowphase(scenario, &candidates.candidates).unwrap();
    transfer_scalar_persistence(&manifolds, previous, completed_step).unwrap()
}

fn slot_map(
    assignments: &[physbox_contracts::PairSlotAssignmentV1],
) -> BTreeMap<physbox_contracts::CandidatePairV1, physbox_contracts::PairSlotAssignmentV1> {
    assignments
        .iter()
        .map(|assignment| (assignment.pair, *assignment))
        .collect()
}

fn warm_cache(scenario: &physbox_contracts::ScenarioV1) -> Vec<PersistentContactV1> {
    let mut cache = scalar_step(scenario, &[], 0).cache;
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
