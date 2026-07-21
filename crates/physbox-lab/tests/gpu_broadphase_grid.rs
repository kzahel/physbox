use physbox_engine::{
    GpuGridConfigurationV1, GpuMultiCellGridRunner, P5_3BroadphasePattern, p5_3_broadphase_snapshot,
};
use physbox_lab::broadphase::{brute_force_broadphase, evaluate_broadphase};

fn small_configuration() -> GpuGridConfigurationV1 {
    GpuGridConfigurationV1 {
        cell_size: 1.0,
        max_cells_per_fixed: 16,
        cell_record_capacity: 4_096,
        global_static_capacity: 32,
        pair_emission_capacity: 16_384,
        candidate_capacity: 32 * 31 / 2,
    }
}

#[test]
fn gpu_grid_is_complete_exact_and_replayable_on_the_small_corpus() {
    let runner = pollster::block_on(GpuMultiCellGridRunner::new()).unwrap();
    for pattern in P5_3BroadphasePattern::ALL {
        let snapshot = p5_3_broadphase_snapshot(pattern, 32, 0x5030).unwrap();
        let oracle = brute_force_broadphase(&snapshot, None).unwrap();
        let first = runner.run(&snapshot, small_configuration()).unwrap();
        let replay = runner.run(&snapshot, small_configuration()).unwrap();
        assert_eq!(first.result, replay.result, "{}", pattern.name());
        let evaluation = evaluate_broadphase(&oracle, &first.result, true).unwrap();
        assert!(evaluation.accepted, "{}: {evaluation:?}", pattern.name());
        assert_eq!(evaluation.unexpected_pairs, 0, "{}", pattern.name());
        assert_eq!(first.result.work.sort_passes, 18);
        assert_eq!(first.result.work.dispatches, 73);
    }
}

#[test]
fn gpu_grid_rejects_pair_emission_overflow_atomically() {
    let snapshot =
        p5_3_broadphase_snapshot(P5_3BroadphasePattern::CompleteOverlap, 32, 91).unwrap();
    let runner = pollster::block_on(GpuMultiCellGridRunner::new()).unwrap();
    let output = runner
        .run(
            &snapshot,
            GpuGridConfigurationV1 {
                pair_emission_capacity: 1,
                ..small_configuration()
            },
        )
        .unwrap();
    assert!(output.result.pairs.is_empty());
    assert!(output.result.failure.is_some());
    let capacity = output
        .result
        .capacities
        .iter()
        .find(|capacity| capacity.class == "pair-emissions")
        .unwrap();
    assert!(capacity.overflowed);
    assert_eq!(capacity.committed_elements, 0);
}

#[test]
fn gpu_grid_benchmark_separates_steady_state_clocks() {
    let snapshot =
        p5_3_broadphase_snapshot(P5_3BroadphasePattern::SparseMovingGrid, 32, 19).unwrap();
    let runner = pollster::block_on(GpuMultiCellGridRunner::new()).unwrap();
    let benchmark = runner
        .benchmark(&snapshot, small_configuration(), 1, 2)
        .unwrap();
    assert_eq!(benchmark.timing.host_completion_seconds.len(), 2);
    assert_eq!(benchmark.timing.cpu_encode_submit_seconds.len(), 2);
    assert_eq!(
        benchmark.timing.gpu_step_seconds.as_ref().map(Vec::len),
        benchmark.timing.timestamp_period_nanoseconds.map(|_| 2)
    );
}
