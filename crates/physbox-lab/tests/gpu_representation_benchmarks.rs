use physbox_engine::{
    GpuGatherTopology, authored_chain_scenario, benchmark_gpu_authored_gather_topology,
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
