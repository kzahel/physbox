use physbox_engine::{
    GpuGatherTopology, GpuGeneratedContactOutput, GpuGeneratedContactRunner, P5_2CandidatePattern,
    P5_2GeneratedScenarioKind, benchmark_gpu_generated_contacts_with_gather_topology,
    p5_2_candidate_scenario, p5_2_generated_performance_scenario, p5_2_generated_scenario,
};

#[test]
fn stable_body_edges_match_the_canonical_gather_for_every_behavior_family() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for kind in P5_2GeneratedScenarioKind::ALL {
        let scenario = p5_2_generated_scenario(kind, 1).unwrap();
        compare_topologies(&runner, &scenario, 1, kind.name());
    }
}

#[test]
fn stable_body_edges_replay_pair_churn_without_intermediate_mapping() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::PairChurn, 30).unwrap();
    let first = runner
        .run_with_gather_topology(&scenario, 30, GpuGatherTopology::StableBodyEdge, false)
        .unwrap();
    let replay = runner
        .run_with_gather_topology(&scenario, 30, GpuGatherTopology::StableBodyEdge, false)
        .unwrap();
    let permuted = runner
        .run_with_gather_topology(&scenario, 30, GpuGatherTopology::StableBodyEdge, true)
        .unwrap();
    assert_equivalent(&first, &replay, "pair-churn replay");
    assert_equivalent(&first, &permuted, "pair-churn permuted build");
    assert!(first.mapped_after_final_step_only);
    assert!(permuted.gather_topology.edge_build_permuted);
}

#[test]
fn stable_body_edges_preserve_high_contention_contribution_order() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 8, 29).unwrap();
    compare_topologies(&runner, &scenario, 1, "eight-body coincidence");
}

#[test]
fn stable_body_edges_preserve_upstream_atomic_overflow() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 34, 7).unwrap();
    let baseline = runner.run(&scenario, 1).unwrap();
    let candidate = runner
        .run_with_gather_topology(&scenario, 1, GpuGatherTopology::StableBodyEdge, false)
        .unwrap();
    assert_equivalent(&baseline, &candidate, "atomic overflow");
    assert!(candidate.failure.is_some());
    assert!(candidate.cache.is_empty());
    assert!(
        candidate
            .capacities
            .iter()
            .any(|capacity| capacity.overflowed && !capacity.accepted)
    );
}

#[test]
fn stable_body_edge_benchmark_retains_qualified_completion() {
    let scenario = p5_2_generated_performance_scenario(32).unwrap();
    let benchmark = pollster::block_on(benchmark_gpu_generated_contacts_with_gather_topology(
        &scenario,
        1,
        2,
        GpuGatherTopology::StableBodyEdge,
    ))
    .unwrap();
    assert_eq!(benchmark.correctness.gather_topology.radix_passes, 2);
    assert_eq!(benchmark.correctness.gather_topology.dispatches, 9);
    assert_eq!(benchmark.correctness.gather_topology.point_capacity, 992);
    assert_eq!(benchmark.correctness.gather_topology.edge_capacity, 1_984);
    assert_eq!(
        benchmark.correctness.gather_topology.allocated_bytes,
        32_512
    );
    assert_eq!(
        benchmark
            .correctness
            .gather_topology
            .capacities
            .iter()
            .map(|capacity| capacity.allocated_bytes)
            .collect::<Vec<_>>(),
        vec![31_744, 512, 256]
    );
    assert_eq!(benchmark.dispatches_per_step, 287);
    assert_eq!(benchmark.host_completion_seconds.len(), 2);
    assert!(benchmark.correctness.failure.is_none());
}

fn compare_topologies(
    runner: &GpuGeneratedContactRunner,
    scenario: &physbox_contracts::ScenarioV1,
    steps: u32,
    label: &str,
) {
    let baseline = runner.run(scenario, steps).unwrap();
    let candidate = runner
        .run_with_gather_topology(scenario, steps, GpuGatherTopology::StableBodyEdge, false)
        .unwrap();
    let permuted = runner
        .run_with_gather_topology(scenario, steps, GpuGatherTopology::StableBodyEdge, true)
        .unwrap();
    assert_equivalent(&baseline, &candidate, label);
    assert_equivalent(&candidate, &permuted, label);
    assert_eq!(
        baseline.gather_topology.topology,
        GpuGatherTopology::CanonicalBodyScan
    );
    assert_eq!(
        candidate.gather_topology.topology,
        GpuGatherTopology::StableBodyEdge
    );
    assert_eq!(baseline.gather_topology.dispatches, 4);
    assert_eq!(
        candidate.dispatches_per_step,
        baseline.dispatches_per_step - baseline.gather_topology.dispatches
            + candidate.gather_topology.dispatches
    );
    assert_eq!(
        candidate.gather_topology.edge_capacity,
        candidate.gather_topology.point_capacity * 2
    );
    assert_eq!(candidate.gather_topology.canonical_scan_attempts, 0);
    assert!(baseline.gather_topology.canonical_scan_attempts > 0);
    assert!(candidate.gather_topology.radix_passes > 0);
    assert!(candidate.mapped_after_final_step_only);
}

fn assert_equivalent(
    left: &GpuGeneratedContactOutput,
    right: &GpuGeneratedContactOutput,
    label: &str,
) {
    assert_eq!(
        left.source_scenario_hash, right.source_scenario_hash,
        "{label}"
    );
    assert_eq!(left.completed_steps, right.completed_steps, "{label}");
    assert_eq!(left.bodies, right.bodies, "{label}");
    assert_eq!(left.cache, right.cache, "{label}");
    assert_eq!(left.work, right.work, "{label}");
    assert_eq!(left.capacities, right.capacities, "{label}");
    assert_eq!(left.failure, right.failure, "{label}");
}
