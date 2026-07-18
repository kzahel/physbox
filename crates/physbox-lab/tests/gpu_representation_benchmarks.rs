use physbox_contracts::FiniteF32;
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
    let serial_assignment_digest = work.assignment_digest.clone();
    let serial_order_digest = work.compacted_order_digest.clone();
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

    let parallel = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &scenario,
        1,
        2,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::PARALLEL_COMPACTION,
    ))
    .unwrap();
    let parallel_work = parallel.scheduler_work.unwrap();
    assert_eq!(parallel.result_digest, benchmark.result_digest);
    assert_eq!(parallel_work.assignment_digest, serial_assignment_digest);
    assert_eq!(parallel_work.compacted_order_digest, serial_order_digest);
    assert_eq!(parallel.topology.color_compaction_dispatches, 4);
    assert_eq!(
        parallel_work.compaction_point_slots_visited,
        parallel_work.contact_point_slots * 2
    );
}

#[test]
fn static_late_keeps_dynamic_only_order_and_fails_closed_at_partition_capacity() {
    let mut dynamic = authored_chain_scenario(9, 47).unwrap();
    dynamic.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    let selected = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &dynamic,
        1,
        1,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::SELECTED,
    ))
    .unwrap();
    let static_late = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &dynamic,
        1,
        1,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::STATIC_LATE,
    ))
    .unwrap();
    assert_eq!(selected.result_digest, static_late.result_digest);
    assert_eq!(
        selected.scheduler_work.as_ref().unwrap().assignment_digest,
        static_late
            .scheduler_work
            .as_ref()
            .unwrap()
            .assignment_digest
    );
    assert_eq!(
        selected
            .scheduler_work
            .as_ref()
            .unwrap()
            .compacted_order_digest,
        static_late
            .scheduler_work
            .as_ref()
            .unwrap()
            .compacted_order_digest
    );

    let mut dynamic_overflow = authored_chain_scenario(30, 47).unwrap();
    dynamic_overflow.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    let center = dynamic_overflow.bodies[0].id;
    for (index, manifold) in dynamic_overflow.manifolds.iter_mut().enumerate() {
        manifold.body_a = center;
        manifold.body_b = dynamic_overflow.bodies[index + 1].id;
    }
    pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &dynamic_overflow,
        1,
        1,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::SELECTED,
    ))
    .unwrap();
    let error = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &dynamic_overflow,
        1,
        1,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::STATIC_LATE,
    ))
    .unwrap_err();
    assert!(error.contains("solver-colors"), "{error}");

    let mut static_overflow = authored_chain_scenario(6, 47).unwrap();
    static_overflow.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    let zero = FiniteF32::new(0.0).unwrap();
    for body in &mut static_overflow.bodies[1..] {
        body.inverse_mass = zero;
        body.inverse_inertia = zero;
    }
    let center = static_overflow.bodies[0].id;
    for (index, manifold) in static_overflow.manifolds.iter_mut().enumerate() {
        manifold.body_a = center;
        manifold.body_b = static_overflow.bodies[index + 1].id;
    }
    let error = pollster::block_on(benchmark_gpu_authored_solver_experiment(
        &static_overflow,
        1,
        1,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::STATIC_LATE,
    ))
    .unwrap_err();
    assert!(error.contains("solver-colors"), "{error}");
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
