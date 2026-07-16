use physbox_contracts::Validate;
use physbox_engine::{
    GpuContactPersistenceTopology, GpuGatherTopology, GpuGeneratedContactRunner,
    GpuNarrowphaseRunner, GpuNormalSolverV4, GpuSolverAccumulationV4, GpuSolverCandidate,
    GpuSolverExecutionV4, P5_2_BUCKET_EVIDENCE_STEPS, P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    P5_2BucketShape, P5_2CandidatePattern, P5_2GeneratedScenarioKind,
    benchmark_gpu_generated_contact_experiment, benchmark_gpu_generated_contacts,
    p5_2_bucket_scenario, p5_2_candidate_scenario, p5_2_centered_rectangle_stack_scenario,
    p5_2_contact_policy_candidate, p5_2_generated_performance_scenario, p5_2_generated_scenario,
    run_gpu_soft_step_case,
};
use physbox_lab::analytic_narrowphase::authored_solver_projection;
use physbox_lab::settling::centered_stack_structure;

const PASS_TOLERANCE: f32 = 2.0e-5;

#[test]
fn generated_contact_benchmark_uses_qualified_split_scopes() {
    let scenario = p5_2_generated_performance_scenario(32).unwrap();
    let benchmark = pollster::block_on(benchmark_gpu_generated_contacts(&scenario, 1, 2)).unwrap();
    assert!(benchmark.correctness.failure.is_none());
    assert_eq!(benchmark.dispatches_per_step, 100);
    assert_eq!(benchmark.host_completion_seconds.len(), 2);
    assert_eq!(benchmark.cpu_encode_submit_seconds.len(), 2);
    match (
        benchmark.gpu_collision_seconds,
        benchmark.gpu_end_to_end_seconds,
        benchmark.timestamp_period_nanoseconds,
    ) {
        (Some(collision), Some(end_to_end), Some(period)) => {
            assert_eq!(collision.len(), 2);
            assert_eq!(end_to_end.len(), 2);
            assert!(period > 0.0);
            assert!(
                collision
                    .iter()
                    .zip(&end_to_end)
                    .all(|(collision, end_to_end)| collision <= end_to_end)
            );
        }
        (None, None, None) => {}
        mismatch => panic!("inconsistent generated timestamp evidence: {mismatch:?}"),
    }
}

#[test]
fn solver_experiments_are_explicit_replayable_and_not_product_defaults() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(2, 30).unwrap();
    scenario.settings.iterations = 2;
    assert!(runner.run(&scenario, 30).is_err());
    assert_eq!(
        GpuSolverExecutionV4::SELECTED.normal_solver,
        GpuNormalSolverV4::PointJacobi
    );

    let execution = GpuSolverExecutionV4::experiment(true, true);
    let first = runner.run_experiment(&scenario, 30, execution).unwrap();
    let replay = runner.run_experiment(&scenario, 30, execution).unwrap();
    assert_eq!(first.solver_settings.iterations, 2);
    assert_eq!(first.solver_execution, execution);
    assert_eq!(first.dispatches_per_step, 129);
    assert_eq!(first.bodies, replay.bodies);
    assert_eq!(first.cache, replay.cache);
    let benchmark = pollster::block_on(benchmark_gpu_generated_contact_experiment(
        &scenario, 1, 1, execution,
    ))
    .unwrap();
    assert_eq!(benchmark.dispatches_per_step, 129);
    assert_eq!(benchmark.correctness.solver_execution, execution);

    let no_warm = runner
        .run_experiment(&scenario, 30, GpuSolverExecutionV4::experiment(false, true))
        .unwrap();
    assert!(!no_warm.solver_execution.warm_start);
    assert!(no_warm.bodies.iter().all(|body| {
        body.position.x.is_finite()
            && body.position.y.is_finite()
            && body.angle.get().is_finite()
            && body.linear_velocity.x.is_finite()
            && body.linear_velocity.y.is_finite()
            && body.angular_velocity.get().is_finite()
    }));
}

#[test]
fn pose_history_matches_every_exact_prefix_without_intermediate_mapping() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(16, 8).unwrap();
    scenario.settings.iterations = 8;
    let execution = GpuSolverExecutionV4::experiment(true, true);
    let history = runner
        .run_experiment_body_history(&scenario, 8, execution)
        .unwrap();
    assert_eq!(history.len(), 8);

    let history_failure = history
        .iter()
        .position(|bodies| !centered_stack_structure(&scenario, bodies, false, 0.0).0);
    let mut prefix_failure = None;
    for completed_steps in 1..=8 {
        let prefix = runner
            .run_experiment(&scenario, completed_steps, execution)
            .unwrap();
        for (history_body, prefix_body) in history[completed_steps as usize - 1]
            .iter()
            .zip(&prefix.bodies)
        {
            assert_eq!(history_body.position, prefix_body.position);
            assert_eq!(history_body.angle, prefix_body.angle);
        }
        if prefix_failure.is_none()
            && !centered_stack_structure(&scenario, &prefix.bodies, false, 0.0).0
        {
            prefix_failure = Some(completed_steps as usize - 1);
        }
    }
    assert_eq!(history_failure, prefix_failure);
    assert!(history_failure.is_some(), "fixture must exercise a failure");
}

#[test]
fn colored_in_place_uses_disjoint_generated_contact_batches_and_replays() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(4, 4).unwrap();
    scenario.settings.iterations = 2;
    let execution = GpuSolverExecutionV4::colored_experiment(true, true);
    let first = runner.run_experiment(&scenario, 4, execution).unwrap();
    let replay = runner.run_experiment(&scenario, 4, execution).unwrap();
    assert_eq!(
        first.solver_execution.accumulation,
        GpuSolverAccumulationV4::ColoredInPlace
    );
    assert_eq!(first.bodies, replay.bodies);
    assert_eq!(first.cache, replay.cache);
    assert_eq!(first.pass_evidence.coloring, replay.pass_evidence.coloring);
    let coloring = first.pass_evidence.coloring.as_ref().unwrap();
    assert!(!coloring.overflowed);
    assert!((1..=coloring.max_colors).contains(&coloring.color_count));
    assert_eq!(
        coloring.point_colors.len(),
        first.pass_evidence.contacts.len()
    );
    assert_eq!(first.gather_topology.coloring_dispatches, 1);
    assert!(first.capacities.iter().any(|capacity| {
        capacity.class == "solver-colors" && capacity.accepted && !capacity.overflowed
    }));
    assert!(first.failure.is_none());
}

#[test]
fn compact_point_coloring_is_byte_exact_with_the_fixed_grid_control() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(8, 8).unwrap();
    scenario.settings.iterations = 2;
    let fixed = runner
        .run_experiment(
            &scenario,
            8,
            GpuSolverExecutionV4::colored_experiment(true, true),
        )
        .unwrap();
    let execution = GpuSolverExecutionV4::compact_colored_experiment(true, true);
    let compact = runner.run_experiment(&scenario, 8, execution).unwrap();
    let replay = runner.run_experiment(&scenario, 8, execution).unwrap();

    assert_eq!(compact.bodies, fixed.bodies);
    assert_eq!(compact.cache, fixed.cache);
    assert_eq!(compact.pass_evidence.contacts, fixed.pass_evidence.contacts);
    assert_eq!(
        compact.pass_evidence.contributions,
        fixed.pass_evidence.contributions
    );
    assert_eq!(
        compact.pass_evidence.manifold_blocks,
        fixed.pass_evidence.manifold_blocks
    );
    assert_eq!(compact.bodies, replay.bodies);
    assert_eq!(compact.cache, replay.cache);
    assert_eq!(compact.pass_evidence, replay.pass_evidence);

    let fixed_colors = fixed.pass_evidence.coloring.as_ref().unwrap();
    let compact_colors = compact.pass_evidence.coloring.as_ref().unwrap();
    assert_eq!(fixed_colors.point_colors, compact_colors.point_colors);
    assert_eq!(fixed_colors.color_occupancy, compact_colors.color_occupancy);
    assert_eq!(fixed_colors.color_count, compact_colors.color_count);
    assert_eq!(
        compact_colors.compacted_point_indices.len(),
        compact_colors.point_colors.len()
    );
    assert_eq!(compact_colors.indirect_workgroups.len(), 32);
    assert_eq!(
        compact_colors.empty_indirect_launches,
        compact_colors.max_colors - compact_colors.active_color_passes
    );
    assert!(compact_colors.scheduled_point_invocations < fixed_colors.scheduled_point_invocations);
    assert!(compact.failure.is_none());
}

#[test]
fn manifold_colored_block_replays_disjoint_units_and_traces_coupled_normals() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(4, 4).unwrap();
    scenario.settings.iterations = 2;
    let execution = GpuSolverExecutionV4::manifold_colored_block_experiment(true, true);
    let first = runner.run_experiment(&scenario, 8, execution).unwrap();
    let replay = runner.run_experiment(&scenario, 8, execution).unwrap();
    assert_eq!(first.bodies, replay.bodies);
    assert_eq!(first.cache, replay.cache);
    assert_eq!(first.pass_evidence, replay.pass_evidence);
    assert_eq!(
        first.solver_execution.normal_solver,
        GpuNormalSolverV4::ManifoldBlock
    );

    let coloring = first.pass_evidence.coloring.as_ref().unwrap();
    assert_eq!(coloring.unit, physbox_engine::GpuColorUnitV4::Manifold);
    assert_eq!(
        coloring.color_occupancy.iter().sum::<u32>(),
        first
            .pass_evidence
            .contacts
            .iter()
            .filter(|contact| contact.point_in_manifold == 0)
            .count() as u32
    );
    for (points, colors) in first
        .pass_evidence
        .contacts
        .chunks(2)
        .zip(coloring.point_colors.chunks(2))
    {
        if points.len() == 2 && points[1].point_in_manifold == 1 {
            assert_eq!(colors[0], colors[1]);
        }
    }
    assert_eq!(
        first
            .pass_evidence
            .manifold_blocks
            .iter()
            .map(|block| block.point_count)
            .sum::<u32>(),
        first.pass_evidence.contacts.len() as u32
    );
    assert!(first.pass_evidence.manifold_blocks.iter().all(|block| {
        block.active_set != physbox_engine::GpuBlockActiveSetV2::Invalid
            && block.body_a_normal_mass_incidence == 1
            && block.body_b_normal_mass_incidence == 1
    }));
    assert!(first.failure.is_none());
}

#[test]
fn one_point_manifold_colored_block_degenerates_exactly_to_scalar_coloring() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 8).unwrap();
    let scalar = runner
        .run_experiment(
            &scenario,
            8,
            GpuSolverExecutionV4::compact_colored_experiment(true, true),
        )
        .unwrap();
    let block = runner
        .run_experiment(
            &scenario,
            8,
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
        )
        .unwrap();
    assert_eq!(block.bodies, scalar.bodies);
    assert_eq!(block.cache, scalar.cache);
    assert_eq!(block.pass_evidence.contacts, scalar.pass_evidence.contacts);
    assert_eq!(
        block.pass_evidence.contributions,
        scalar.pass_evidence.contributions
    );
    assert!(
        block
            .pass_evidence
            .manifold_blocks
            .iter()
            .all(|evidence| evidence.point_count == 1
                && evidence.active_set == physbox_engine::GpuBlockActiveSetV2::Scalar)
    );
}

#[test]
fn colored_in_place_degenerates_exactly_to_jacobi_for_one_live_point() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 8).unwrap();
    let jacobi = runner
        .run_experiment(&scenario, 8, GpuSolverExecutionV4::experiment(true, true))
        .unwrap();
    let colored = runner
        .run_experiment(
            &scenario,
            8,
            GpuSolverExecutionV4::colored_experiment(true, true),
        )
        .unwrap();
    assert_eq!(colored.bodies, jacobi.bodies);
    assert_eq!(colored.cache, jacobi.cache);
    assert_eq!(
        colored.pass_evidence.contacts,
        jacobi.pass_evidence.contacts
    );
}

#[test]
fn colored_in_place_rejects_more_than_thirty_two_incident_points_atomically() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_centered_rectangle_stack_scenario(16, 1).unwrap();
    let mut extra = scenario.bodies[1].clone();
    extra.id.slot = 17;
    scenario.bodies.push(extra);
    if let physbox_contracts::ShapeV1::Rectangle { half_extents } = &mut scenario.bodies[0].shape {
        half_extents.x = 10.0;
    }
    for (index, body) in scenario.bodies[1..].iter_mut().enumerate() {
        body.position.x = (index as f32 - 8.0) * 0.9;
        body.position.y = 0.125;
    }
    scenario.validate().unwrap();

    let output = runner
        .run_experiment(
            &scenario,
            1,
            GpuSolverExecutionV4::colored_experiment(true, true),
        )
        .unwrap();
    let coloring = output.pass_evidence.coloring.as_ref().unwrap();
    assert!(coloring.overflowed);
    assert!(output.cache.is_empty());
    assert!(output.failure.is_some());
    let capacity = output
        .capacities
        .iter()
        .find(|capacity| capacity.class == "solver-colors")
        .unwrap();
    assert_eq!(capacity.capacity_elements, 32);
    assert_eq!(capacity.attempted_elements, 33);
    assert_eq!(capacity.committed_elements, 0);
    assert!(capacity.overflowed);
    assert!(!capacity.accepted);

    let compact = runner
        .run_experiment(
            &scenario,
            1,
            GpuSolverExecutionV4::compact_colored_experiment(true, true),
        )
        .unwrap();
    let compact_coloring = compact.pass_evidence.coloring.as_ref().unwrap();
    assert!(compact_coloring.overflowed);
    assert!(
        compact_coloring
            .indirect_workgroups
            .iter()
            .all(|arguments| *arguments == [0, 1, 1])
    );
    assert_eq!(compact_coloring.empty_indirect_launches, 32);
    assert_eq!(compact_coloring.scheduled_point_invocations, 0);
    assert!(compact.cache.is_empty());
    assert!(compact.failure.is_some());

    while scenario.bodies.len() < 34 {
        let mut body = scenario.bodies[1].clone();
        body.id.slot = scenario.bodies.len() as u32;
        scenario.bodies.push(body);
    }
    if let physbox_contracts::ShapeV1::Rectangle { half_extents } = &mut scenario.bodies[0].shape {
        half_extents.x = 20.0;
    }
    for (index, body) in scenario.bodies[1..].iter_mut().enumerate() {
        body.position.x = (index as f32 - 16.0) * 1.1;
        body.position.y = 0.125;
    }
    scenario.validate().unwrap();
    let manifold = runner
        .run_experiment(
            &scenario,
            1,
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true),
        )
        .unwrap();
    let manifold_coloring = manifold.pass_evidence.coloring.as_ref().unwrap();
    assert!(manifold_coloring.overflowed);
    assert!(
        manifold_coloring
            .indirect_workgroups
            .iter()
            .all(|arguments| *arguments == [0, 1, 1])
    );
    assert!(manifold.cache.is_empty());
    assert!(manifold.failure.is_some());
}

#[test]
fn scalar_mass_split_uses_exact_gpu_point_incidence_on_both_gather_topologies() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
    let execution =
        GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::ScalarMassSplit, true, true);
    let stable = runner
        .run_experiment_with_gather_topology(
            &scenario,
            1,
            GpuGatherTopology::StableBodyEdge,
            true,
            execution,
        )
        .unwrap();
    let canonical = runner
        .run_experiment_with_gather_topology(
            &scenario,
            1,
            GpuGatherTopology::CanonicalBodyScan,
            false,
            execution,
        )
        .unwrap();
    assert_eq!(stable.solver_execution, execution);
    assert_eq!(stable.bodies, canonical.bodies);
    assert_eq!(stable.cache, canonical.cache);

    let mut expected_counts = vec![0_u32; stable.bodies.len()];
    for contact in &stable.pass_evidence.contacts {
        expected_counts[contact.body_a as usize] += 1;
        expected_counts[contact.body_b as usize] += 1;
    }
    for contact in &stable.pass_evidence.contacts {
        assert_eq!(
            contact.body_a_normal_mass_incidence,
            expected_counts[contact.body_a as usize]
        );
        assert_eq!(
            contact.body_b_normal_mass_incidence,
            expected_counts[contact.body_b as usize]
        );
        let body_a = &scenario.bodies[contact.body_a as usize];
        let body_b = &scenario.bodies[contact.body_b as usize];
        let normal_cross_a =
            contact.anchor_a.x * contact.normal.y - contact.anchor_a.y * contact.normal.x;
        let normal_cross_b =
            contact.anchor_b.x * contact.normal.y - contact.anchor_b.y * contact.normal.x;
        let expected_inverse_normal = contact.body_a_normal_mass_incidence as f32
            * (body_a.inverse_mass.get()
                + body_a.inverse_inertia.get() * normal_cross_a * normal_cross_a)
            + contact.body_b_normal_mass_incidence as f32
                * (body_b.inverse_mass.get()
                    + body_b.inverse_inertia.get() * normal_cross_b * normal_cross_b);
        assert!(
            (contact.normal_effective_mass - 1.0 / expected_inverse_normal).abs() <= 1.0e-6,
            "contact {contact:?}, expected inverse {expected_inverse_normal}"
        );
    }
    assert_eq!(expected_counts, vec![2, 4, 2]);
    assert_eq!(
        stable.pass_evidence.contacts,
        canonical.pass_evidence.contacts
    );
}

#[test]
fn manifold_block_degenerates_to_point_jacobi_for_one_point_contacts() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 1).unwrap();
    let point = runner
        .run_experiment(&scenario, 1, GpuSolverExecutionV4::experiment(true, true))
        .unwrap();
    let block_execution =
        GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::ManifoldBlock, true, true);
    let block = runner
        .run_experiment(&scenario, 1, block_execution)
        .unwrap();
    assert_eq!(point.bodies, block.bodies);
    assert_eq!(point.cache, block.cache);
    assert_eq!(block.pass_evidence.manifold_blocks.len(), 1);
    assert_eq!(
        block.pass_evidence.manifold_blocks[0].active_set,
        physbox_engine::GpuBlockActiveSetV2::Scalar
    );
    assert_eq!(
        block.gather_topology.manifold_count_dispatches, 0,
        "the unsplit block candidate needs no incidence pass"
    );
    assert_eq!(block.dispatches_per_step, point.dispatches_per_step);
    assert_eq!(
        block.gather_topology.allocated_bytes,
        point.gather_topology.allocated_bytes
    );

    let replay_scenario =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 8).unwrap();
    let point_replay = runner
        .run_experiment(
            &replay_scenario,
            8,
            GpuSolverExecutionV4::experiment(true, true),
        )
        .unwrap();
    let block_replay = runner
        .run_experiment(&replay_scenario, 8, block_execution)
        .unwrap();
    assert_eq!(point_replay.bodies, block_replay.bodies);
    assert_eq!(point_replay.cache, block_replay.cache);
}

#[test]
fn block_mass_split_uses_exact_manifold_incidence_and_coupled_matrix() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
    let execution =
        GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::BlockMassSplit, true, true);
    let output = runner.run_experiment(&scenario, 1, execution).unwrap();
    let unsplit = runner
        .run_experiment(
            &scenario,
            1,
            GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::ManifoldBlock, true, true),
        )
        .unwrap();
    let canonical = runner
        .run_experiment_with_gather_topology(
            &scenario,
            1,
            GpuGatherTopology::CanonicalBodyScan,
            false,
            execution,
        )
        .unwrap();
    assert_eq!(output.bodies, canonical.bodies);
    assert_eq!(output.cache, canonical.cache);
    assert_eq!(
        output.pass_evidence.manifold_blocks,
        canonical.pass_evidence.manifold_blocks
    );
    assert_eq!(output.pass_evidence.manifold_blocks.len(), 2);
    let mut expected_counts = vec![0_u32; output.bodies.len()];
    for block in &output.pass_evidence.manifold_blocks {
        expected_counts[block.body_a as usize] += 1;
        expected_counts[block.body_b as usize] += 1;
    }
    assert_eq!(expected_counts, vec![1, 2, 1]);
    for block in &output.pass_evidence.manifold_blocks {
        assert_eq!(
            block.body_a_normal_mass_incidence,
            expected_counts[block.body_a as usize]
        );
        assert_eq!(
            block.body_b_normal_mass_incidence,
            expected_counts[block.body_b as usize]
        );
        assert_eq!(block.point_count, 2);
        let points = output
            .pass_evidence
            .contacts
            .iter()
            .filter(|contact| contact.manifold_index == block.manifold_index)
            .collect::<Vec<_>>();
        assert_eq!(points.len(), 2);
        let body_a = &scenario.bodies[block.body_a as usize];
        let body_b = &scenario.bodies[block.body_b as usize];
        let matrix_entry =
            |left: &physbox_engine::GpuFixedFloorContactEvidence,
             right: &physbox_engine::GpuFixedFloorContactEvidence| {
                let cross_a_left =
                    left.anchor_a.x * left.normal.y - left.anchor_a.y * left.normal.x;
                let cross_a_right =
                    right.anchor_a.x * right.normal.y - right.anchor_a.y * right.normal.x;
                let cross_b_left =
                    left.anchor_b.x * left.normal.y - left.anchor_b.y * left.normal.x;
                let cross_b_right =
                    right.anchor_b.x * right.normal.y - right.anchor_b.y * right.normal.x;
                block.body_a_normal_mass_incidence as f32
                    * (body_a.inverse_mass.get()
                        + body_a.inverse_inertia.get() * cross_a_left * cross_a_right)
                    + block.body_b_normal_mass_incidence as f32
                        * (body_b.inverse_mass.get()
                            + body_b.inverse_inertia.get() * cross_b_left * cross_b_right)
            };
        let expected = [
            matrix_entry(points[0], points[0]),
            matrix_entry(points[0], points[1]),
            matrix_entry(points[1], points[0]),
            matrix_entry(points[1], points[1]),
        ];
        for (actual, expected) in block.effective_mass_matrix.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= 1.0e-6, "block {block:?}");
        }
    }
    assert!(
        output
            .gather_topology
            .capacities
            .iter()
            .any(|capacity| capacity.class == "solver-manifold-incidence")
    );
    assert_eq!(output.gather_topology.manifold_count_dispatches, 1);
    assert_eq!(output.dispatches_per_step, unsplit.dispatches_per_step + 1);
    assert_eq!(
        output.gather_topology.allocated_bytes,
        unsplit.gather_topology.allocated_bytes + scenario.bodies.len() as u64 * 4
    );
}

#[test]
fn product_pair_slots_and_body_edges_match_full_key_canonical_oracles() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for kind in P5_2GeneratedScenarioKind::ALL {
        let steps = if kind == P5_2GeneratedScenarioKind::PairChurn {
            30
        } else {
            4
        };
        let scenario = p5_2_generated_scenario(kind, steps).unwrap();
        for completed_steps in 1..=steps {
            let product = runner.run(&scenario, completed_steps).unwrap();
            let full_key = runner
                .run_full_key_oracle(&scenario, completed_steps)
                .unwrap();
            let reversed_and_colliding = runner
                .run_with_topologies(
                    &scenario,
                    completed_steps,
                    GpuContactPersistenceTopology::StablePairSlot,
                    GpuGatherTopology::CanonicalBodyScan,
                    false,
                    true,
                    0,
                )
                .unwrap();

            assert_eq!(
                product.persistence_topology,
                GpuContactPersistenceTopology::StablePairSlot
            );
            assert_eq!(
                full_key.persistence_topology,
                GpuContactPersistenceTopology::FullKeyOracle
            );
            assert_eq!(
                product.cache, full_key.cache,
                "{kind:?} step {completed_steps}"
            );
            assert_eq!(
                product.bodies, full_key.bodies,
                "{kind:?} step {completed_steps}"
            );
            assert_eq!(
                product.bodies, reversed_and_colliding.bodies,
                "{kind:?} step {completed_steps}"
            );
            assert_eq!(
                product.cache, reversed_and_colliding.cache,
                "{kind:?} step {completed_steps}"
            );
            assert_eq!(
                product.failure, reversed_and_colliding.failure,
                "{kind:?} step {completed_steps}"
            );
            assert_eq!(product.work.cache_matched, full_key.work.cache_matched);
            assert_eq!(product.work.cache_cold, full_key.work.cache_cold);
            assert_eq!(product.work.cache_expired, full_key.work.cache_expired);
            assert_eq!(product.work.sort_passes, 0);
            assert_eq!(full_key.work.sort_passes, 64);
            assert_eq!(
                product.gather_topology.topology,
                GpuGatherTopology::StableBodyEdge
            );
            assert_eq!(
                reversed_and_colliding.gather_topology.topology,
                GpuGatherTopology::CanonicalBodyScan
            );
            assert_eq!(product.dispatches_per_step, 97);
            assert_eq!(full_key.dispatches_per_step, 282);
        }
    }
}

#[test]
fn generated_bridge_matches_the_scalar_qualified_authored_solver() {
    let generated = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let narrowphase = pollster::block_on(GpuNarrowphaseRunner::new()).unwrap();
    for kind in [
        P5_2GeneratedScenarioKind::BilateralCircles,
        P5_2GeneratedScenarioKind::OffCenterRectangles,
        P5_2GeneratedScenarioKind::FrictionSlide,
        P5_2GeneratedScenarioKind::Restitution,
    ] {
        let source = p5_2_generated_scenario(kind, 1).unwrap();
        let contacts = narrowphase.run(&source, None, None).unwrap();
        assert!(contacts.failure.is_none());
        assert!(!contacts.manifolds.is_empty(), "{kind:?}");
        let authored = authored_solver_projection(&source, &contacts.manifolds).unwrap();
        let expected = pollster::block_on(run_gpu_soft_step_case(
            &authored,
            GpuSolverCandidate::JacobiGather,
        ))
        .unwrap();
        let actual = generated.run(&source, 1).unwrap();
        assert_eq!(
            actual.pass_evidence.contacts.len(),
            actual.cache.len(),
            "{kind:?} pass/cache point count"
        );
        assert_eq!(
            actual.pass_evidence.contributions.len(),
            actual.pass_evidence.contacts.len() * 2
        );
        assert!(actual.pass_evidence.contacts.iter().all(|contact| {
            contact.valid
                && contact.body_a != contact.body_b
                && contact.normal_effective_mass > 0.0
                && contact.tangent_effective_mass > 0.0
        }));
        for (actual, expected) in actual.bodies.iter().zip(&expected.bodies) {
            for (actual, expected) in [
                (actual.position.x, expected.position.x),
                (actual.position.y, expected.position.y),
                (actual.angle.get(), expected.angle.get()),
                (actual.linear_velocity.x, expected.linear_velocity.x),
                (actual.linear_velocity.y, expected.linear_velocity.y),
                (
                    actual.angular_velocity.get(),
                    expected.angular_velocity.get(),
                ),
            ] {
                assert!((actual - expected).abs() <= PASS_TOLERANCE, "{kind:?}");
            }
        }
        assert_eq!(actual.cache.len(), expected.contact_cache.len(), "{kind:?}");
        for (actual, expected) in actual.cache.iter().zip(&expected.contact_cache) {
            assert_eq!(actual.key.pair.body_a, expected.body_a);
            assert_eq!(actual.key.pair.body_b, expected.body_b);
            assert_eq!(actual.key.feature_a.index, expected.feature_a.feature_index);
            assert_eq!(actual.key.feature_b.index, expected.feature_b.feature_index);
            assert!(
                (actual.value.normal_impulse.get() - expected.normal_impulse.get()).abs()
                    <= PASS_TOLERANCE,
                "{kind:?}"
            );
            assert!(
                (actual.value.tangent_impulse.get() - expected.tangent_impulse.get()).abs()
                    <= PASS_TOLERANCE,
                "{kind:?}"
            );
        }
    }
}

#[test]
fn generated_floor_contacts_settle_and_replay_exactly() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for (kind, expected_height, steps) in [
        (P5_2GeneratedScenarioKind::RectangleFloor, 0.2, 60),
        (P5_2GeneratedScenarioKind::CircleFloor, 0.5, 60),
    ] {
        let scenario = p5_2_generated_scenario(kind, steps).unwrap();
        let first = runner.run(&scenario, steps).unwrap();
        let replay = runner.run(&scenario, steps).unwrap();
        assert_eq!(first.bodies, replay.bodies);
        assert_eq!(first.cache, replay.cache);
        assert_eq!(first.work, replay.work);
        assert!(first.failure.is_none());
        assert_eq!(first.dispatches_per_step, 97);
        assert!(
            (first.bodies[0].position.y - expected_height).abs() <= 2.0e-2,
            "{kind:?}: {:?}",
            first.bodies[0]
        );
        assert!(
            first.bodies[0].linear_velocity.y.abs() <= 2.0e-2,
            "{kind:?}: {:?}",
            first.bodies[0]
        );
        assert!(!first.cache.is_empty());
        assert!(
            first
                .cache
                .iter()
                .all(|contact| contact.value.last_seen_step == steps)
        );
        assert!(
            first
                .cache
                .iter()
                .any(|contact| contact.value.normal_impulse.get() > 0.0)
        );
    }
}

#[test]
fn generated_impacts_rotation_friction_restitution_and_churn_remain_finite() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for (kind, steps) in [
        (P5_2GeneratedScenarioKind::BilateralCircles, 1),
        (P5_2GeneratedScenarioKind::OffCenterRectangles, 4),
        (P5_2GeneratedScenarioKind::SmallStack, 12),
        (P5_2GeneratedScenarioKind::FrictionSlide, 12),
        (P5_2GeneratedScenarioKind::Restitution, 8),
        (P5_2GeneratedScenarioKind::PairChurn, 12),
    ] {
        let scenario = p5_2_generated_scenario(kind, steps).unwrap();
        let output = runner.run(&scenario, steps).unwrap();
        assert!(output.failure.is_none(), "{kind:?}: {:?}", output.failure);
        assert!(output.capacities.iter().all(|capacity| capacity.accepted));
        assert!(output.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.angle.get().is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        }));
    }
}

#[test]
fn generated_contacts_have_expected_structural_responses() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();

    let bilateral =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::BilateralCircles, 1).unwrap();
    let bilateral = runner.run(&bilateral, 1).unwrap();
    assert!(
        bilateral.bodies[0].linear_velocity.x < 1.0,
        "bilateral bodies: {:?}; work: {:?}; cache: {:?}",
        bilateral.bodies,
        bilateral.work,
        bilateral.cache
    );
    assert!(
        bilateral.bodies[1].linear_velocity.x > -1.0,
        "bilateral bodies: {:?}",
        bilateral.bodies
    );

    let off_center =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::OffCenterRectangles, 4).unwrap();
    let off_center = runner.run(&off_center, 4).unwrap();
    assert!(
        off_center
            .bodies
            .iter()
            .any(|body| body.angular_velocity.get().abs() > 1.0e-3)
    );

    let friction = p5_2_generated_scenario(P5_2GeneratedScenarioKind::FrictionSlide, 12).unwrap();
    let friction = runner.run(&friction, 12).unwrap();
    assert!(friction.bodies[0].linear_velocity.x.abs() < 2.0);

    let restitution = p5_2_generated_scenario(P5_2GeneratedScenarioKind::Restitution, 8).unwrap();
    let restitution = runner.run(&restitution, 8).unwrap();
    assert!(restitution.bodies[0].linear_velocity.y > 0.0);

    let stack = p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 12).unwrap();
    let stack = runner.run(&stack, 12).unwrap();
    for pair in stack.bodies[1..].windows(2) {
        assert!(pair[0].position.y < pair[1].position.y);
    }
}

#[test]
fn generated_small_stack_long_exact_run_completes() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 180).unwrap();
    let output = runner.run(&scenario, 180).unwrap();
    assert!(output.failure.is_none());
    assert!(output.capacities.iter().all(|capacity| capacity.accepted));
}

#[test]
fn centered_rectangle_stacks_complete_exact_steps_and_replay() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for count in [2, 4, 8] {
        let scenario =
            p5_2_centered_rectangle_stack_scenario(count, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                .unwrap();
        let output = runner
            .run(&scenario, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
            .unwrap();
        assert!(
            output.failure.is_none(),
            "count {count}: {:?}",
            output.failure
        );
        assert_eq!(output.completed_steps, P5_2_CENTERED_STACK_EVIDENCE_STEPS);
        assert!(output.capacities.iter().all(|capacity| capacity.accepted));
        let overflowed = output.capacities.iter().any(|capacity| capacity.overflowed);
        let (passed, reason) = centered_stack_structure(&scenario, &output.bodies, overflowed, 0.0);
        assert_eq!(
            passed,
            count == 2,
            "count {count}: {reason}: {:?}",
            output.bodies
        );
        let replay = runner
            .run(&scenario, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
            .unwrap();
        assert_eq!(output.bodies, replay.bodies);
        assert_eq!(output.cache, replay.cache);
        assert_eq!(output.work, replay.work);
    }
}

#[test]
fn all_pairs_bucket_baselines_complete_at_capacity_and_replay() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for shape in P5_2BucketShape::ALL {
        let scenario = p5_2_bucket_scenario(shape, 100, P5_2_BUCKET_EVIDENCE_STEPS).unwrap();
        let output = runner.run(&scenario, P5_2_BUCKET_EVIDENCE_STEPS).unwrap();
        assert!(output.failure.is_none(), "{shape:?}: {:?}", output.failure);
        assert_eq!(output.completed_steps, P5_2_BUCKET_EVIDENCE_STEPS);
        assert_eq!(output.bodies.len(), 103);
        assert_eq!(output.work.pair_attempts, 5_253);
        assert_eq!(output.work.fixed_fixed_rejected, 3);
        assert!(
            output
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed)
        );
        assert!(output.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.angle.get().is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        }));
        let replay = runner.run(&scenario, P5_2_BUCKET_EVIDENCE_STEPS).unwrap();
        assert_eq!(output.bodies, replay.bodies, "{shape:?}");
        assert_eq!(output.cache, replay.cache, "{shape:?}");
        assert_eq!(output.work, replay.work, "{shape:?}");
    }
}

#[test]
fn generated_randomized_worlds_replay_and_dense_overflow_is_atomic() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for pattern in [
        P5_2CandidatePattern::Random,
        P5_2CandidatePattern::Touching,
        P5_2CandidatePattern::RotatedThin,
        P5_2CandidatePattern::FixedOnly,
    ] {
        for seed in [19, 71] {
            let scenario = p5_2_candidate_scenario(pattern, 16, seed).unwrap();
            let first = runner.run(&scenario, 2).unwrap();
            let replay = runner.run(&scenario, 2).unwrap();
            assert_eq!(first.bodies, replay.bodies, "{pattern:?} seed {seed}");
            assert_eq!(first.cache, replay.cache, "{pattern:?} seed {seed}");
            assert!(first.failure.is_none(), "{pattern:?} seed {seed}");
        }
    }

    let dense = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 34, 97).unwrap();
    let rejected = runner.run(&dense, 1).unwrap();
    assert!(rejected.failure.is_some());
    assert!(rejected.cache.is_empty());
    let candidate = rejected
        .capacities
        .iter()
        .find(|capacity| capacity.class == "candidate-pairs")
        .unwrap();
    assert!(candidate.overflowed);
    assert!(!candidate.accepted);
    assert_eq!(candidate.committed_elements, 0);
}

#[test]
fn generated_dense_coincident_reproducer_replays_exactly() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    for body_count in 2..=16 {
        let scenario =
            p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, body_count, 19).unwrap();
        let first = runner.run(&scenario, 1).unwrap();
        let replay = runner.run(&scenario, 1).unwrap();
        assert_eq!(first.bodies, replay.bodies, "body_count {body_count}");
    }
}

#[test]
fn candidate_policy_closes_the_preserved_speculative_gap_reproducer() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario =
        p5_2_generated_scenario(P5_2GeneratedScenarioKind::RectangleFloor, 1).unwrap();
    scenario.id = "p5.2-generated-speculative-gap".to_owned();
    scenario.bodies[0].position.y = 0.205;
    scenario.bodies[0].linear_velocity.y = -0.5;
    let output = runner.run(&scenario, 1).unwrap();
    assert!(output.failure.is_none());
    assert!(!output.cache.is_empty());
    assert!(
        output.bodies[0].position.y >= 0.199,
        "speculative body: {:?}; cache: {:?}",
        output.bodies[0],
        output.cache
    );
}

#[test]
fn candidate_policy_bounds_the_preserved_deep_coincidence_reproducer() {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let mut scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 6, 19).unwrap();
    scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    let output = runner.run(&scenario, 1).unwrap();
    let maximum_speed = output
        .bodies
        .iter()
        .map(|body| {
            (body.linear_velocity.x * body.linear_velocity.x
                + body.linear_velocity.y * body.linear_velocity.y)
                .sqrt()
        })
        .fold(0.0, f32::max);
    assert!(output.failure.is_none());
    assert!(
        maximum_speed <= 100.0,
        "maximum bounded-depenetration speed was {maximum_speed}: {:?}",
        output.bodies
    );
}
