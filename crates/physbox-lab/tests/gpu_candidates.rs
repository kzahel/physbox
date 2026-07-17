use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FiniteF32, TraceStageV1, Vec2,
};
use physbox_engine::{
    EngineConfig, FrameInput, GpuSolverCandidate, OffscreenRenderer, authored_chain_scenario,
    authored_one_contact_scenario, benchmark_gpu_soft_step_case, gpu_contact_softness_coefficients,
    p5_2_contact_policy_candidate, run_gpu_soft_step_case, run_gpu_soft_step_stress_case,
    run_gpu_solver_case,
};
use physbox_lab::{
    capacity_is_valid, equal_and_opposite, projected_impulses,
    scalar::{box2d_softness_coefficients, first_trace_divergence, solve_authored_contact_pass},
};
use physbox_launch::ValidationScenario;

fn softness(hertz: f32, damping: f32, static_multiplier: f32) -> ContactSoftnessPolicyV1 {
    ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: FiniteF32::new(hertz).unwrap(),
        damping_ratio: FiniteF32::new(damping).unwrap(),
        static_contact_hertz_multiplier: FiniteF32::new(static_multiplier).unwrap(),
    }
}

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
fn both_gpu_accumulation_paths_match_the_bounded_depenetration_specification() {
    let mut scenario = authored_one_contact_scenario(7);
    scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    scenario.manifolds[0].points[0].separation = FiniteF32::new(-1.0).unwrap();
    let scalar = solve_authored_contact_pass(&scenario).unwrap();
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let gpu = pollster::block_on(run_gpu_solver_case(&scenario, candidate)).unwrap();
        let divergence = first_trace_divergence(&scalar.traces, &gpu.traces, 2.0e-5, 2.0e-5);
        assert!(divergence.is_none(), "{candidate:?}: {divergence:?}");
        let normal = gpu
            .traces
            .iter()
            .find(|trace| trace.stage == TraceStageV1::NormalSolve)
            .unwrap();
        assert_eq!(
            normal
                .values
                .iter()
                .find(|value| value.name == "velocityBias")
                .unwrap()
                .value
                .get(),
            -3.0
        );
    }
}

#[test]
fn independent_gpu_softness_derivation_and_shader_match_the_scalar_specification() {
    let substep_dt = 1.0 / 240.0;
    for hertz in [15.0, 30.0, 60.0] {
        for damping in [5.0, 10.0] {
            for static_multiplier in [1.0, 2.0] {
                let policy = softness(hertz, damping, static_multiplier);
                for static_contact in [false, true] {
                    let gpu = gpu_contact_softness_coefficients(policy, substep_dt, static_contact)
                        .unwrap();
                    let capped = hertz.min(0.125 / substep_dt);
                    let effective_hertz = if static_contact {
                        capped * static_multiplier
                    } else {
                        capped
                    };
                    let scalar =
                        box2d_softness_coefficients(effective_hertz, damping, substep_dt).unwrap();
                    assert_close(gpu.bias_rate, scalar.bias_rate);
                    assert_close(gpu.mass_scale, scalar.mass_scale);
                    assert_close(gpu.impulse_scale, scalar.impulse_scale);
                }
            }
        }
    }

    let mut scenario = authored_one_contact_scenario(7);
    scenario.settings.contact_softness = Some(softness(30.0, 10.0, 2.0));
    scenario.manifolds[0].points[0].separation = FiniteF32::new(-0.01).unwrap();
    scenario.manifolds[0].material.restitution = FiniteF32::new(0.0).unwrap();
    scenario.bodies[0].linear_velocity = Vec2::ZERO;
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

#[test]
fn benchmark_mode_warms_resets_and_delays_timestamp_readback() {
    let scenario = authored_chain_scenario(8, 91).unwrap();
    let result = pollster::block_on(benchmark_gpu_soft_step_case(
        &scenario,
        GpuSolverCandidate::JacobiGather,
        2,
        3,
    ))
    .unwrap();
    assert_eq!(result.warmup_steps, 2);
    assert_eq!(result.samples, 3);
    assert_eq!(result.host_completion_seconds.len(), 3);
    assert_eq!(result.cpu_encode_submit_seconds.len(), 3);
    assert!(
        result
            .host_completion_seconds
            .iter()
            .all(|sample| *sample > 0.0)
    );
    assert!(
        result
            .cpu_encode_submit_seconds
            .iter()
            .all(|sample| *sample > 0.0)
    );
    assert_eq!(
        result
            .capacities
            .iter()
            .find(|record| record.class == "traces")
            .unwrap()
            .capacity_elements,
        1
    );
    match (
        &result.gpu_step_seconds,
        result.timestamp_period_nanoseconds,
    ) {
        (Some(samples), Some(period)) => {
            assert_eq!(samples.len(), 3);
            assert!(period > 0.0);
            assert!(samples.iter().all(|sample| *sample > 0.0));
        }
        (None, None) => {}
        mismatch => panic!("inconsistent timestamp evidence: {mismatch:?}"),
    }
    capacity_is_valid(&result.capacities).unwrap();
}

#[test]
fn trace_free_stress_validation_reads_complete_thirty_thousand_contact_state() {
    let scenario = authored_chain_scenario(30_001, 91).unwrap();
    for candidate in [
        GpuSolverCandidate::JacobiGather,
        GpuSolverCandidate::ColoredSoftTgs,
    ] {
        let result =
            pollster::block_on(run_gpu_soft_step_stress_case(&scenario, candidate)).unwrap();
        assert_eq!(result.bodies.len(), 30_001);
        assert_eq!(result.contact_cache.len(), 30_000);
        assert!(result.traces.is_empty());
        assert!(result.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        }));
        capacity_is_valid(&result.capacities).unwrap();
    }
}

#[test]
fn product_generated_floor_landing_stays_bounded_and_preserves_invariants() {
    let mut renderer = pollster::block_on(OffscreenRenderer::new(
        320,
        200,
        EngineConfig {
            scenario: ValidationScenario::RigidFloor,
        },
    ))
    .unwrap();
    let report = renderer.render(FrameInput::exact(120));
    assert!(report.scenario.complete);
    assert!(report.scenario.passed);
    assert_eq!(report.scene_contract.contact_point_count, 2_048);
    assert_eq!(report.scene_contract.solver_dispatches_per_step, 1_434);
    assert!(!report.scene_contract.capacity_overflowed);
    let actual = renderer.read_capture_state().unwrap();
    assert!(actual.into_iter().all(f32::is_finite));
    assert!(
        (actual[1] - 0.2).abs() <= 0.002,
        "GPU landing height was {}",
        actual[1]
    );
    assert!(
        actual[2].abs() <= 0.002,
        "GPU landing angle was {}",
        actual[2]
    );
    assert!(
        actual[4].hypot(actual[5]) <= 0.002,
        "GPU landing linear speed was ({}, {})",
        actual[4],
        actual[5]
    );
    assert!(
        actual[6].abs() <= 0.002,
        "GPU landing angular speed was {}",
        actual[6]
    );
    let pass = renderer.read_fixed_floor_pass_evidence().unwrap();
    assert_eq!(pass.contacts.len(), 2_048);
    assert_eq!(pass.contributions.len(), 4_096);
    for contact in pass.contacts.iter().take(2) {
        projected_impulses(
            FiniteF32::new(contact.normal_impulse).unwrap(),
            FiniteF32::new(contact.tangent_impulse).unwrap(),
            FiniteF32::new(contact.friction).unwrap(),
        )
        .unwrap();
        assert!(contact.total_normal_impulse >= 0.0);
    }
    for pair in pass.contributions[..4].chunks_exact(2) {
        assert_eq!(pair[0].point_index, pair[1].point_index);
        assert_eq!(pair[0].side, 0);
        assert_eq!(pair[1].side, 1);
        equal_and_opposite(pair[0].impulse, pair[1].impulse, 1.0e-7).unwrap();
    }
    assert!(pass.contacts[2..].iter().all(|contact| {
        contact.normal_impulse == 0.0
            && contact.tangent_impulse == 0.0
            && contact.total_normal_impulse == 0.0
    }));
    assert!(
        pass.contributions[4..]
            .iter()
            .all(|contribution| contribution.impulse == Vec2::ZERO)
    );
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
