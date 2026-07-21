use physbox_engine::{GpuSweepAndPruneRunner, P5_3BroadphasePattern, p5_3_broadphase_snapshot};
use physbox_lab::broadphase::{brute_force_broadphase, evaluate_broadphase};

#[test]
fn gpu_sap_is_complete_exact_and_replayable_on_the_small_corpus() {
    let runner = pollster::block_on(GpuSweepAndPruneRunner::new()).unwrap();
    for pattern in P5_3BroadphasePattern::ALL {
        let snapshot = p5_3_broadphase_snapshot(pattern, 32, 0x5030).unwrap();
        let oracle = brute_force_broadphase(&snapshot, None).unwrap();
        let first = runner.run(&snapshot, 32 * 31 / 2).unwrap();
        let replay = runner.run(&snapshot, 32 * 31 / 2).unwrap();
        assert_eq!(first.result, replay.result, "{}", pattern.name());
        let evaluation = evaluate_broadphase(&oracle, &first.result, true).unwrap();
        assert!(evaluation.accepted, "{}: {evaluation:?}", pattern.name());
        assert_eq!(evaluation.unexpected_pairs, 0, "{}", pattern.name());
        assert_eq!(first.result.work.sort_passes, 18);
        assert_eq!(first.result.work.dispatches, 58);
    }
}

#[test]
fn gpu_sap_rejects_complete_overlap_overflow_atomically() {
    let snapshot =
        p5_3_broadphase_snapshot(P5_3BroadphasePattern::CompleteOverlap, 32, 91).unwrap();
    let oracle = brute_force_broadphase(&snapshot, None).unwrap();
    let runner = pollster::block_on(GpuSweepAndPruneRunner::new()).unwrap();
    let output = runner.run(&snapshot, 1).unwrap();
    assert!(output.result.pairs.is_empty());
    assert!(output.result.failure.is_some());
    assert_eq!(
        output.result.work.unique_candidates,
        oracle.pairs.len() as u64
    );
    let capacity = output
        .result
        .capacities
        .iter()
        .find(|capacity| capacity.class == "candidate-pairs")
        .unwrap();
    assert!(capacity.overflowed);
    assert_eq!(capacity.committed_elements, 0);
}

#[test]
fn gpu_sap_benchmark_separates_steady_state_clocks() {
    let snapshot =
        p5_3_broadphase_snapshot(P5_3BroadphasePattern::SparseMovingGrid, 32, 19).unwrap();
    let runner = pollster::block_on(GpuSweepAndPruneRunner::new()).unwrap();
    let benchmark = runner.benchmark(&snapshot, 64, 1, 2).unwrap();
    assert_eq!(benchmark.timing.host_completion_seconds.len(), 2);
    assert_eq!(benchmark.timing.cpu_encode_submit_seconds.len(), 2);
    assert_eq!(
        benchmark.timing.gpu_step_seconds.as_ref().map(Vec::len),
        benchmark.timing.timestamp_period_nanoseconds.map(|_| 2)
    );
}
