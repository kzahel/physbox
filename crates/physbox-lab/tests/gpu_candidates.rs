use physbox_engine::{
    GpuSolverCandidate, authored_chain_scenario, authored_one_contact_scenario, run_gpu_solver_case,
};
use physbox_lab::{
    capacity_is_valid, equal_and_opposite,
    scalar::{first_trace_divergence, solve_authored_contact_pass},
};

#[test]
fn isolated_contact_passes_match_scalar_and_emit_paired_contributions() {
    let scenario = authored_one_contact_scenario(7);
    let scalar = solve_authored_contact_pass(&scenario).unwrap();
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let gpu = pollster::block_on(run_gpu_solver_case(&scenario, candidate)).unwrap();
        let divergence = first_trace_divergence(&scalar.traces, &gpu.traces, 2.0e-5, 2.0e-5);
        assert!(divergence.is_none(), "{candidate:?}: {divergence:?}");
        for (expected, actual) in scalar.bodies.iter().zip(&gpu.bodies) {
            assert_close(expected.linear_velocity.x, actual.linear_velocity.x);
            assert_close(expected.linear_velocity.y, actual.linear_velocity.y);
            assert_close(
                expected.angular_velocity.get(),
                actual.angular_velocity.get(),
            );
        }
        if candidate == GpuSolverCandidate::JacobiGather {
            assert_eq!(gpu.contributions.len(), 2);
            assert!(gpu.contributions[0].impulse.x != 0.0 || gpu.contributions[0].impulse.y != 0.0);
            equal_and_opposite(
                gpu.contributions[0].impulse,
                gpu.contributions[1].impulse,
                2.0e-5,
            )
            .unwrap();
        } else {
            assert!(gpu.contributions.is_empty());
        }
        capacity_is_valid(&gpu.capacities).unwrap();
    }
}

#[test]
fn authored_chain_runs_both_accumulation_architectures_without_overflow() {
    let scenario = authored_chain_scenario(64, 91).unwrap();
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let first = pollster::block_on(run_gpu_solver_case(&scenario, candidate)).unwrap();
        let replay = pollster::block_on(run_gpu_solver_case(&scenario, candidate)).unwrap();
        assert_eq!(first.bodies, replay.bodies, "{candidate:?}");
        assert_eq!(first.contact_cache, replay.contact_cache, "{candidate:?}");
        assert!(first.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        }));
        capacity_is_valid(&first.capacities).unwrap();
        if candidate == GpuSolverCandidate::ColoredSoftTgs {
            assert!(first.color_count >= 2);
        }
    }
}

fn assert_close(expected: f32, actual: f32) {
    assert!(
        (expected - actual).abs() <= 2.0e-5,
        "expected {expected}, actual {actual}"
    );
}
