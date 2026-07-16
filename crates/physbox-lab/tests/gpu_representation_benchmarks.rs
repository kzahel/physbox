use physbox_engine::{
    GpuGatherTopology, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1,
    P5_2_SELECTED_SOLVER_ITERATIONS, authored_chain_scenario,
    benchmark_gpu_authored_gather_topology, benchmark_gpu_authored_solver_experiment,
    benchmark_gpu_pair_slot_persistence, p5_2_generated_performance_scenario,
};

#[test]
fn authored_gather_benchmark_is_replayable_and_topology_exact() {
    let scenario = authored_chain_scenario(33, 47).unwrap();
    let baseline = pollster::block_on(benchmark_gpu_authored_gather_topology(
        &scenario,
        1,
        2,
        GpuGatherTopology::CanonicalBodyScan,
    ))
    .unwrap();
    let candidate = pollster::block_on(benchmark_gpu_authored_gather_topology(
        &scenario,
        1,
        2,
        GpuGatherTopology::StableBodyEdge,
    ))
    .unwrap();
    assert!(baseline.replay_exact);
    assert!(candidate.replay_exact);
    assert_eq!(baseline.result_digest, candidate.result_digest);
    assert_eq!(baseline.contact_points, 32);
    assert_eq!(candidate.contact_points, 32);
    assert_eq!(baseline.topology.dispatches, 4);
    assert_eq!(candidate.topology.dispatches, 9);
    assert!(baseline.scheduler_work.is_none());
    assert!(candidate.scheduler_work.is_none());
    assert_eq!(baseline.host_completion_seconds.len(), 2);
    assert_eq!(candidate.host_completion_seconds.len(), 2);
    assert!(
        baseline
            .capacities
            .iter()
            .all(|capacity| !capacity.overflowed)
    );
    assert!(
        candidate
            .capacities
            .iter()
            .all(|capacity| !capacity.overflowed)
    );
}

#[test]
fn authored_selected_solver_records_exact_scheduler_work() {
    let mut scenario = authored_chain_scenario(33, 47).unwrap();
    scenario.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    let benchmark = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &scenario,
        1,
        2,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::STANDARD,
    ))
    .unwrap();
    let work = benchmark.scheduler_work.unwrap();
    assert!(benchmark.replay_exact);
    assert_eq!(benchmark.manifold_count, 32);
    assert_eq!(benchmark.contact_points, 32);
    assert_eq!(benchmark.solver_execution, GpuSolverExecutionV4::SELECTED);
    assert_eq!(work.body_slots, 33);
    assert_eq!(work.contact_point_slots, 32);
    assert_eq!(work.live_contact_points, 32);
    assert_eq!(work.color_units, 32);
    assert_eq!(work.active_colors, 2);
    assert_eq!(work.color_occupancy, vec![16, 16]);
    assert_eq!(work.body_mask_slots_cleared, 33);
    assert_eq!(work.assignment_point_slots_visited, 32);
    assert_eq!(work.compaction_point_slots_visited, 32);
    assert_eq!(work.compacted_color_units, 32);
    assert_eq!(work.encoded_color_passes, 32);
    assert_eq!(work.active_color_passes, 2);
    assert_eq!(work.empty_indirect_launches, 30);
    assert_ne!(work.assignment_digest, work.compacted_order_digest);
    assert_eq!(
        benchmark.instrumented_solver_phases.as_ref().map(Vec::len),
        benchmark.timestamp_period_nanoseconds.map(|_| 2)
    );
}

#[test]
fn pair_slot_benchmark_retains_the_correctness_gate() {
    let scenario = p5_2_generated_performance_scenario(32).unwrap();
    let benchmark =
        pollster::block_on(benchmark_gpu_pair_slot_persistence(&scenario, 1, 2)).unwrap();
    assert_eq!(benchmark.dispatches_per_step, 25);
    assert_eq!(benchmark.host_completion_seconds.len(), 2);
    assert_eq!(benchmark.correctness.steps.len(), 1);
    assert!(benchmark.correctness.steps[0].failure.is_none());
    assert!(
        benchmark.correctness.steps[0]
            .capacities
            .iter()
            .all(|capacity| capacity.accepted)
    );
}
