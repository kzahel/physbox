#![cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]

use physbox_engine::{
    GpuColoredDispatchV1, GpuGeneratedContactRunner, GpuSolverExecutionV4,
    GpuSolverSchedulingPolicyV1, P5_2BucketShape, p5_2_bucket_scenario,
};

#[test]
fn selected_policy_qualifies_128_slot_island_route_and_keeps_scalable_fallback() {
    let qualified = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(128);
    let fallback = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(129);

    assert_eq!(qualified, GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);
    assert_eq!(fallback, GpuSolverSchedulingPolicyV1::SELECTED);
    assert_eq!(qualified.color_maintenance, fallback.color_maintenance);
    assert_eq!(
        qualified.colored_dispatch,
        GpuColoredDispatchV1::WorkgroupPerIsland
    );
    assert_eq!(
        fallback.colored_dispatch,
        GpuColoredDispatchV1::PerColorIndirect
    );
}

#[test]
fn island_dispatch_matches_scalable_dispatch_exactly_at_100_solids() {
    let scenario = p5_2_bucket_scenario(P5_2BucketShape::Circle, 100, 4).unwrap();
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let run = |scheduling| {
        runner
            .run_scheduling_experiment(&scenario, 4, GpuSolverExecutionV4::SELECTED, scheduling)
            .unwrap()
    };

    let scalable = run(GpuSolverSchedulingPolicyV1::SELECTED);
    let islands = run(GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);

    assert_equivalent_schedule_results(&scenario, &scalable, &islands);
}

fn assert_equivalent_schedule_results(
    scenario: &physbox_contracts::ScenarioV1,
    scalable: &physbox_engine::GpuGeneratedContactOutput,
    islands: &physbox_engine::GpuGeneratedContactOutput,
) {
    assert_eq!(scalable.bodies, islands.bodies);
    assert_eq!(scalable.manifolds, islands.manifolds);
    assert_eq!(scalable.cache, islands.cache);
    assert_eq!(
        scalable.pass_evidence.contacts,
        islands.pass_evidence.contacts
    );
    let scalable_coloring = scalable.pass_evidence.coloring.as_ref().unwrap();
    let island_coloring = islands.pass_evidence.coloring.as_ref().unwrap();
    assert_eq!(scalable_coloring.point_colors, island_coloring.point_colors);
    assert_eq!(
        scalable_coloring.compacted_point_indices,
        island_coloring.compacted_point_indices
    );
    let scalable_work = scalable
        .pass_evidence
        .scheduler_work_vector(scenario.bodies.len() as u32)
        .unwrap()
        .unwrap();
    let island_work = islands
        .pass_evidence
        .scheduler_work_vector(scenario.bodies.len() as u32)
        .unwrap()
        .unwrap();
    assert_eq!(
        scalable_work.assignment_digest,
        island_work.assignment_digest
    );
    assert_eq!(
        scalable_work.compacted_order_digest,
        island_work.compacted_order_digest
    );
    assert!(scalable.failure.is_none());
    assert!(islands.failure.is_none());
    assert!(scalable.capacities.iter().all(|capacity| capacity.accepted));
    assert!(islands.capacities.iter().all(|capacity| capacity.accepted));
}
