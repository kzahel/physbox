use physbox_engine::{
    GpuSolverCandidate, authored_chain_scenario, authored_one_contact_scenario,
    run_gpu_soft_step_case, run_gpu_solver_case,
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
            assert_eq!(first.contact_colors.len(), 63);
            assert!(first.contact_colors.iter().all(|color| *color < 32));
            assert!(
                first
                    .contact_colors
                    .windows(2)
                    .all(|colors| colors[0] != colors[1])
            );
        } else {
            assert!(first.contact_colors.is_empty());
        }
    }
}

#[test]
fn bounded_gpu_coloring_reports_high_degree_overflow_without_harming_gather() {
    let mut scenario = authored_chain_scenario(34, 117).unwrap();
    let center = scenario.bodies[0].id;
    for (index, manifold) in scenario.manifolds.iter_mut().enumerate() {
        manifold.body_a = center;
        manifold.body_b = scenario.bodies[index + 1].id;
    }

    let jacobi = pollster::block_on(run_gpu_solver_case(
        &scenario,
        GpuSolverCandidate::JacobiGather,
    ))
    .unwrap();
    capacity_is_valid(&jacobi.capacities).unwrap();

    let colored = pollster::block_on(run_gpu_solver_case(
        &scenario,
        GpuSolverCandidate::ColoredSoftTgs,
    ))
    .unwrap();
    assert_eq!(colored.color_count, 32);
    assert_eq!(colored.contact_colors.last(), Some(&u32::MAX));
    assert!(capacity_is_valid(&colored.capacities).is_err());
}

#[test]
fn soft_step_retains_anchors_across_substeps_and_converges_the_chain() {
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let mut one_substep = authored_chain_scenario(64, 91).unwrap();
        one_substep.settings.substeps = 1;
        let mut four_substeps = one_substep.clone();
        four_substeps.settings.substeps = 4;

        let once = pollster::block_on(run_gpu_soft_step_case(&one_substep, candidate)).unwrap();
        let four = pollster::block_on(run_gpu_soft_step_case(&four_substeps, candidate)).unwrap();
        let replay = pollster::block_on(run_gpu_soft_step_case(&four_substeps, candidate)).unwrap();
        assert_eq!(four.bodies, replay.bodies, "{candidate:?}");
        assert_eq!(four.contact_cache, replay.contact_cache, "{candidate:?}");
        assert!(chain_velocity_residual(&four.bodies) <= chain_velocity_residual(&once.bodies));
        assert!(four.bodies[0].position.y < one_substep.bodies[0].position.y);
        assert_eq!(
            four.traces.len(),
            four_substeps.manifolds.len()
                * (5 + 2 * four_substeps.settings.iterations as usize)
                * four_substeps.settings.substeps as usize
        );
        assert_eq!(four.traces.iter().map(|trace| trace.substep).max(), Some(3));
        capacity_is_valid(&four.capacities).unwrap();
    }
}

fn chain_velocity_residual(bodies: &[physbox_contracts::BodyStateV1]) -> f32 {
    bodies
        .windows(2)
        .map(|pair| (pair[1].linear_velocity.x - pair[0].linear_velocity.x).abs())
        .sum()
}

fn assert_close(expected: f32, actual: f32) {
    assert!(
        (expected - actual).abs() <= 2.0e-5,
        "expected {expected}, actual {actual}"
    );
}
