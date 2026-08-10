#![cfg(all(feature = "solver-lab", not(target_arch = "wasm32")))]

use physbox_engine::{
    GpuColoredDispatchV1, GpuGeneratedContactRunner, GpuSolverExecutionV4,
    GpuSolverSchedulingPolicyV1, P5_2BucketShape, p5_2_bucket_scenario,
};
use physbox_sim::{BodyDef, ShapeDef, StepDef, World, WorldCapacity, WorldDef};

#[test]
fn selected_policy_qualifies_128_slot_island_route_and_keeps_scalable_fallback() {
    let qualified = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(128);
    let fallback = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(129);

    assert_eq!(qualified, GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);
    assert_eq!(fallback, GpuSolverSchedulingPolicyV1::SELECTED);
    assert_eq!(qualified.color_maintenance, fallback.color_maintenance);
    assert_eq!(
        qualified.colored_dispatch,
        GpuColoredDispatchV1::WorkgroupPerIsland
    );
    assert_eq!(
        fallback.colored_dispatch,
        GpuColoredDispatchV1::PerColorIndirect
    );

    assert_eq!(
        GpuSolverSchedulingPolicyV1::selected_for_live_body_count(23),
        GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD
    );
    assert_eq!(
        GpuSolverSchedulingPolicyV1::selected_for_live_body_count(129),
        GpuSolverSchedulingPolicyV1::SELECTED
    );
}

#[test]
fn island_dispatch_matches_scalable_dispatch_exactly_at_100_solids() {
    let scenario = p5_2_bucket_scenario(P5_2BucketShape::Circle, 100, 4).unwrap();
    let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
    let run = |scheduling| {
        runner
            .run_scheduling_experiment(&scenario, 4, GpuSolverExecutionV4::SELECTED, scheduling)
            .unwrap()
    };

    let scalable = run(GpuSolverSchedulingPolicyV1::SELECTED);
    let islands = run(GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);

    assert_equivalent_schedule_results(&scenario, &scalable, &islands);
}

#[test]
fn island_route_remains_exact_when_gpu_live_work_grows_past_128_bodies() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("physbox5_live_schedule_growth_test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();
    let definition = WorldDef {
        gravity: [0.0, 0.0],
        capacity: WorldCapacity::Slots256,
        ..Default::default()
    };
    let fixed_dt = definition.quality.physics.fixed_dt_seconds();

    let mut grown = World::new(&device, definition).unwrap();
    add_isolated_body(&mut grown, 0);
    submit_world_step(&mut grown, &device, &queue, fixed_dt);
    for first_index in [1, 65] {
        for index in first_index..first_index + 64 {
            add_isolated_body(&mut grown, index);
        }
        submit_world_step(&mut grown, &device, &queue, fixed_dt);
    }
    assert_eq!(grown.counters().live_bodies, 129);

    let mut fallback = World::new(&device, definition).unwrap();
    for index in 0..129 {
        add_isolated_body(&mut fallback, index);
    }
    for _ in 0..3 {
        submit_world_step(&mut fallback, &device, &queue, fixed_dt);
    }

    assert_eq!(
        take_body_snapshot(&grown, &device, &queue),
        take_body_snapshot(&fallback, &device, &queue)
    );
}

fn add_isolated_body(world: &mut World, index: u32) {
    let body = world
        .create_body(BodyDef::dynamic().with_position([index as f32 * 2.0, 0.0]))
        .unwrap();
    world
        .create_shape(body, ShapeDef::standard_runtime_circle())
        .unwrap();
}

fn submit_world_step(world: &mut World, device: &wgpu::Device, queue: &wgpu::Queue, fixed_dt: f32) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_live_schedule_growth_step"),
    });
    world
        .step(device, queue, &mut encoder, StepDef::fixed(fixed_dt))
        .unwrap();
    queue.submit([encoder.finish()]);
    device.poll(wgpu::PollType::Wait).unwrap();
}

fn take_body_snapshot(
    world: &World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<physbox_sim::BodySnapshotEntry> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_live_schedule_growth_snapshot"),
    });
    let mut snapshot = world.request_body_snapshot(device, &mut encoder).unwrap();
    queue.submit([encoder.finish()]);
    snapshot.begin_map().unwrap();
    device.poll(wgpu::PollType::Wait).unwrap();
    snapshot.try_take().unwrap().unwrap().bodies
}

fn assert_equivalent_schedule_results(
    scenario: &physbox_contracts::ScenarioV1,
    scalable: &physbox_engine::GpuGeneratedContactOutput,
    islands: &physbox_engine::GpuGeneratedContactOutput,
) {
    assert_eq!(scalable.bodies, islands.bodies);
    assert_eq!(scalable.manifolds, islands.manifolds);
    assert_eq!(scalable.cache, islands.cache);
    assert_eq!(
        scalable.pass_evidence.contacts,
        islands.pass_evidence.contacts
    );
    let scalable_coloring = scalable.pass_evidence.coloring.as_ref().unwrap();
    let island_coloring = islands.pass_evidence.coloring.as_ref().unwrap();
    assert_eq!(scalable_coloring.point_colors, island_coloring.point_colors);
    assert_eq!(
        scalable_coloring.compacted_point_indices,
        island_coloring.compacted_point_indices
    );
    let scalable_work = scalable
        .pass_evidence
        .scheduler_work_vector(scenario.bodies.len() as u32)
        .unwrap()
        .unwrap();
    let island_work = islands
        .pass_evidence
        .scheduler_work_vector(scenario.bodies.len() as u32)
        .unwrap()
        .unwrap();
    assert_eq!(
        scalable_work.assignment_digest,
        island_work.assignment_digest
    );
    assert_eq!(
        scalable_work.compacted_order_digest,
        island_work.compacted_order_digest
    );
    assert!(scalable.failure.is_none());
    assert!(islands.failure.is_none());
    assert!(scalable.capacities.iter().all(|capacity| capacity.accepted));
    assert!(islands.capacities.iter().all(|capacity| capacity.accepted));
}
