use std::mem::size_of;
use std::sync::{Arc, Mutex};

use physbox_sim::{
    BodyDef, CommandQueueErrorKind, GPU_BODY_OBSERVATION_LIVE, GPU_BODY_OBSERVATION_VERSION,
    GPU_PARTICLE_OBSERVATION_LIVE, GPU_PARTICLE_OBSERVATION_VERSION, GpuBodyObservationRecord,
    GpuParticleObservationRecord, PARTICLE_COUNT_SNAPSHOT_VERSION,
    PARTICLE_EMISSION_COMMAND_CAPACITY, PREVIEW_WATER_RADIUS, ParticleEmissionDef,
    ParticleRegionDef, ParticleSystemDef, PointConstraintDef, PointQueryDef, ShapeDef, StepDef,
    World, WorldCommandKind, WorldCommandResults, WorldCommandStatus, WorldDef,
};

#[test]
fn public_world_steps_and_observes_without_a_surface() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("test GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("physbox_sim_public_world_test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .expect("test GPU device");

    let definition = WorldDef::default();
    let mut world = World::new(&device, definition).unwrap();
    let floor = world
        .create_body(BodyDef::fixed().with_position([0.0, -0.1]))
        .unwrap();
    world
        .create_shape(floor, ShapeDef::rectangle([8.0, 0.1]))
        .unwrap();
    let ball = world
        .create_body(BodyDef::dynamic().with_position([0.0, 1.0]))
        .unwrap();
    world
        .create_shape(ball, ShapeDef::circle(0.2).with_density(1.0))
        .unwrap();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_world_step"),
    });
    let report = world
        .step(
            &device,
            &queue,
            &mut encoder,
            StepDef::fixed(definition.quality.physics.fixed_dt_seconds()),
        )
        .unwrap();
    queue.submit([encoder.finish()]);
    assert_eq!(report.outer_step, 1);
    assert_eq!(report.live_bodies, 2);

    let observation = world.create_gpu_body_observation(&device).unwrap();
    assert!(world.create_gpu_particle_observation(&device).is_err());
    assert!(
        world
            .queue_particle_emission(ParticleEmissionDef::water_circle([0.0, 1.0], 0.2))
            .unwrap_err()
            .to_string()
            .contains("configured particle system")
    );
    assert_eq!(observation.version(), GPU_BODY_OBSERVATION_VERSION);
    assert_eq!(
        observation.record_stride_bytes() as usize,
        size_of::<GpuBodyObservationRecord>()
    );
    let observation_bytes =
        u64::from(observation.capacity()) * u64::from(observation.record_stride_bytes());
    let observation_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_sim_public_observation_readback"),
        size: observation_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_observation"),
    });
    observation.encode(&mut encoder);
    encoder.copy_buffer_to_buffer(
        observation.buffer(),
        0,
        &observation_readback,
        0,
        observation_bytes,
    );
    queue.submit([encoder.finish()]);
    let completion = Arc::new(Mutex::new(None));
    let callback_completion = Arc::clone(&completion);
    observation_readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback_completion.lock().unwrap() = Some(result);
        });
    device.poll(wgpu::PollType::Wait).unwrap();
    completion
        .lock()
        .unwrap()
        .take()
        .expect("observation mapping completed")
        .expect("observation mapping succeeded");
    {
        let mapped = observation_readback.slice(..).get_mapped_range();
        let stride = size_of::<GpuBodyObservationRecord>();
        let start = ball.slot() as usize * stride;
        let record = bytemuck::pod_read_unaligned::<GpuBodyObservationRecord>(
            &mapped[start..start + stride],
        );
        assert_eq!(record.identity[0], ball.generation());
        assert_eq!(record.identity[2], ball.slot());
        assert_ne!(record.identity[1] & GPU_BODY_OBSERVATION_LIVE, 0);
        assert!(record.pose[0].is_finite());
        assert!(record.pose[1].is_finite());
        assert!(record.pose[2].is_finite());
    }
    observation_readback.unmap();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_world_snapshot"),
    });
    let mut snapshot = world.request_body_snapshot(&device, &mut encoder).unwrap();
    queue.submit([encoder.finish()]);
    snapshot.begin_map().unwrap();
    device.poll(wgpu::PollType::Wait).unwrap();
    let snapshot = snapshot
        .try_take()
        .expect("mapping completed")
        .expect("snapshot succeeded");
    assert_eq!(snapshot.bodies.len(), 2);
    assert!(snapshot.bodies.iter().all(|body| {
        body.position[0].is_finite() && body.position[1].is_finite() && body.angle.is_finite()
    }));
}

#[test]
fn public_world_steps_and_observes_preview_water_without_private_layouts() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("test GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("physbox_sim_public_water_test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .expect("test GPU device");

    let definition = WorldDef::default();
    let fixed_dt = definition.quality.physics.fixed_dt_seconds();
    let mut world = World::new(&device, definition).unwrap();
    let floor = world
        .create_body(BodyDef::fixed().with_position([0.0, -0.15]))
        .unwrap();
    world
        .create_shape(floor, ShapeDef::rectangle([1.7, 0.15]))
        .unwrap();
    let ball = world
        .create_body(BodyDef::dynamic().with_position([0.0, 0.65]))
        .unwrap();
    world
        .create_shape(ball, ShapeDef::circle(0.18).with_density(0.8))
        .unwrap();
    let particles = ParticleSystemDef::water(64).with_region(
        ParticleRegionDef::rectangle([-0.3, 0.0], [0.3, 0.4], 0.1)
            .with_initial_velocity([0.05, 0.0]),
    );
    let initial_count = particles.initial_particle_count().unwrap();
    world.set_particle_system(particles.clone()).unwrap();
    assert!(world.set_particle_system(particles).is_err());
    submit_steps(&mut world, &device, &queue, fixed_dt, 1);
    assert!(
        world
            .set_particle_system(ParticleSystemDef::water(8))
            .is_err()
    );

    let observation = world.create_gpu_particle_observation(&device).unwrap();
    assert_eq!(observation.version(), GPU_PARTICLE_OBSERVATION_VERSION);
    assert_eq!(observation.capacity(), 64);
    assert_eq!(observation.particle_radius(), PREVIEW_WATER_RADIUS);
    assert_eq!(
        observation.record_stride_bytes() as usize,
        size_of::<GpuParticleObservationRecord>()
    );
    let records = observe_particles(&device, &queue, &observation);
    let live = records
        .iter()
        .filter(|record| record.identity[1] & GPU_PARTICLE_OBSERVATION_LIVE != 0)
        .collect::<Vec<_>>();
    assert_eq!(live.len(), initial_count as usize);
    for (slot, record) in live.into_iter().enumerate() {
        assert_eq!(
            record.identity,
            [1, GPU_PARTICLE_OBSERVATION_LIVE, slot as u32, 0]
        );
        assert!(record.position[0].is_finite());
        assert!(record.position[1].is_finite());
        assert!(record.motion[0].is_finite());
        assert!(record.motion[1].is_finite());
    }
    for record in records.iter().skip(initial_count as usize) {
        assert_eq!(record.identity[1] & GPU_PARTICLE_OBSERVATION_LIVE, 0);
    }

    let initial_counts = take_particle_counts(&world, &device, &queue);
    assert_eq!(initial_counts.version, PARTICLE_COUNT_SNAPSHOT_VERSION);
    assert_eq!(initial_counts.capacity, 64);
    assert_eq!(initial_counts.live_particles, initial_count);
    assert_eq!(initial_counts.spawned_particles, 0);
    assert_eq!(initial_counts.dropped_particles, 0);

    let emission = ParticleEmissionDef::water_circle([0.8, 0.8], 0.2);
    let requested = world.queue_particle_emission(emission).unwrap();
    assert_eq!(world.counters().queued_particle_emissions, 1);
    assert_eq!(
        world.counters().particle_emission_capacity,
        PARTICLE_EMISSION_COMMAND_CAPACITY
    );
    let report = submit_steps(&mut world, &device, &queue, fixed_dt, 1);
    assert_eq!(report.queued_operations, 1);
    assert_eq!(world.counters().queued_particle_emissions, 0);
    let emitted_counts = take_particle_counts(&world, &device, &queue);
    assert_eq!(
        emitted_counts.live_particles,
        initial_count + requested,
        "the first bounded emission fits"
    );
    assert_eq!(emitted_counts.spawned_particles, requested);
    assert_eq!(emitted_counts.dropped_particles, 0);

    let overflow_requested = world
        .queue_particle_emission(ParticleEmissionDef::water_circle([0.8, 1.4], 0.5))
        .unwrap();
    submit_steps(&mut world, &device, &queue, fixed_dt, 1);
    let full_counts = take_particle_counts(&world, &device, &queue);
    let available = 64 - emitted_counts.live_particles;
    assert_eq!(full_counts.live_particles, 64);
    assert_eq!(
        full_counts.spawned_particles,
        requested + available,
        "spawn count records only claimed slots"
    );
    assert_eq!(
        full_counts.dropped_particles,
        overflow_requested - available,
        "capacity drops remain visible without blocking the step"
    );

    let mut queued_world = World::new(&device, definition).unwrap();
    queued_world
        .set_particle_system(
            ParticleSystemDef::water(64).with_region(ParticleRegionDef::rectangle(
                [-0.1, 0.0],
                [0.1, 0.2],
                0.1,
            )),
        )
        .unwrap();
    for index in 0..PARTICLE_EMISSION_COMMAND_CAPACITY {
        queued_world
            .queue_particle_emission(ParticleEmissionDef::water_circle(
                [-1.0 + index as f32 * 0.001, 1.0],
                0.05,
            ))
            .unwrap();
    }
    assert!(
        queued_world
            .queue_particle_emission(ParticleEmissionDef::water_circle([0.0, 1.0], 0.05))
            .unwrap_err()
            .to_string()
            .contains("queue capacity")
    );

    let mut empty_world = World::new(&device, definition).unwrap();
    let floor = empty_world
        .create_body(BodyDef::fixed().with_position([0.0, -0.15]))
        .unwrap();
    empty_world
        .create_shape(floor, ShapeDef::rectangle([1.7, 0.15]))
        .unwrap();
    empty_world
        .set_particle_system(ParticleSystemDef::water(64))
        .unwrap();
    assert_eq!(
        empty_world
            .queue_particle_emission(ParticleEmissionDef::water_circle([0.0, 0.8], 0.2))
            .unwrap(),
        requested
    );
    submit_steps(&mut empty_world, &device, &queue, fixed_dt, 1);
    let empty_counts = take_particle_counts(&empty_world, &device, &queue);
    assert_eq!(empty_counts.live_particles, requested);
    assert_eq!(empty_counts.spawned_particles, requested);
    assert_eq!(empty_counts.dropped_particles, 0);
}

#[test]
fn public_world_composes_deferred_drag_release_and_selected_destruction() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("test GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("physbox_sim_public_interaction_test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .expect("test GPU device");

    let mut definition = WorldDef::default();
    definition.quality.physics.sleeping_enabled = false;
    let fixed_dt = definition.quality.physics.fixed_dt_seconds();
    let mut world = World::new(&device, definition).unwrap();
    let floor = world
        .create_body(BodyDef::fixed().with_position([0.0, -0.15]))
        .unwrap();
    world
        .create_shape(floor, ShapeDef::rectangle([1.7, 0.15]))
        .unwrap();
    let ball = world
        .create_body(BodyDef::dynamic().with_position([0.0, 0.9]))
        .unwrap();
    world
        .create_shape(ball, ShapeDef::circle(0.22).with_density(1.0))
        .unwrap();
    submit_steps(&mut world, &device, &queue, fixed_dt, 1);

    let missed = world
        .queue_point_query(PointQueryDef::at([10.0, 10.0]))
        .unwrap();
    let pending = world
        .queue_create_point_constraint(
            missed.selection,
            PointConstraintDef::at_target([10.0, 10.0]),
        )
        .unwrap();
    world
        .queue_set_point_constraint_target(pending.constraint, [9.0, 9.0])
        .unwrap();
    world
        .queue_destroy_point_constraint(pending.constraint)
        .unwrap();
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt);
    assert_result(
        &results,
        WorldCommandKind::PointQuery,
        WorldCommandStatus::Missed,
        None,
    );
    assert_result(
        &results,
        WorldCommandKind::CreatePointConstraint,
        WorldCommandStatus::Missed,
        None,
    );
    assert_result(
        &results,
        WorldCommandKind::SetPointConstraintTarget,
        WorldCommandStatus::Cancelled,
        None,
    );
    assert_result(
        &results,
        WorldCommandKind::DestroyPointConstraint,
        WorldCommandStatus::Cancelled,
        None,
    );

    let selected = world
        .queue_point_query(PointQueryDef::at([0.0, 0.9]))
        .unwrap();
    let constraint = world
        .queue_create_point_constraint(
            selected.selection,
            PointConstraintDef::at_target([0.0, 0.9]),
        )
        .unwrap();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_interaction_create"),
    });
    world
        .step(&device, &queue, &mut encoder, StepDef::fixed(fixed_dt))
        .unwrap();
    queue.submit([encoder.finish()]);
    assert_eq!(
        world
            .queue_point_query(PointQueryDef::at([0.0, 0.9]))
            .unwrap_err()
            .kind(),
        CommandQueueErrorKind::ResultsPending
    );
    let results = take_command_results(&mut world, &device, &queue);
    assert_result(
        &results,
        WorldCommandKind::PointQuery,
        WorldCommandStatus::Succeeded,
        Some(ball),
    );
    assert_result(
        &results,
        WorldCommandKind::CreatePointConstraint,
        WorldCommandStatus::Succeeded,
        Some(ball),
    );

    world
        .queue_set_point_constraint_target(constraint.constraint, [0.45, 0.9])
        .unwrap();
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt);
    assert_result(
        &results,
        WorldCommandKind::SetPointConstraintTarget,
        WorldCommandStatus::Succeeded,
        Some(ball),
    );
    submit_steps(&mut world, &device, &queue, fixed_dt, 30);

    let observation = world.create_gpu_body_observation(&device).unwrap();
    let moved = observe_body(&device, &queue, &observation, ball.slot());
    assert_eq!(moved.identity[0], ball.generation());
    assert_ne!(moved.identity[1] & GPU_BODY_OBSERVATION_LIVE, 0);
    assert!(moved.pose[0] > 0.05);

    world
        .queue_destroy_point_constraint(constraint.constraint)
        .unwrap();
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt);
    assert_result(
        &results,
        WorldCommandKind::DestroyPointConstraint,
        WorldCommandStatus::Succeeded,
        Some(ball),
    );
    assert_eq!(
        world
            .queue_set_point_constraint_target(constraint.constraint, [0.0, 0.0])
            .unwrap_err()
            .kind(),
        CommandQueueErrorKind::InvalidConstraint
    );

    let selected = world
        .queue_point_query(PointQueryDef::at([moved.pose[0], moved.pose[1]]))
        .unwrap();
    world
        .queue_destroy_selected_body(selected.selection)
        .unwrap();
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt);
    assert_result(
        &results,
        WorldCommandKind::DestroySelectedBody,
        WorldCommandStatus::Succeeded,
        Some(ball),
    );
    assert_eq!(world.counters().live_bodies, 1);

    let retired = observe_body(&device, &queue, &observation, ball.slot());
    assert_eq!(retired.identity[0], ball.generation() + 1);
    assert_eq!(retired.identity[1] & GPU_BODY_OBSERVATION_LIVE, 0);

    let replacement = world
        .create_body(BodyDef::dynamic().with_position([-0.8, 1.2]))
        .unwrap();
    assert_eq!(replacement.slot(), ball.slot());
    assert_eq!(replacement.generation(), ball.generation() + 1);
    world
        .create_shape(
            replacement,
            ShapeDef::circle(0.25)
                .with_density(0.8)
                .with_friction(0.5)
                .with_restitution(0.0),
        )
        .unwrap();
    submit_steps(&mut world, &device, &queue, fixed_dt, 1);
    let replacement_record = observe_body(&device, &queue, &observation, replacement.slot());
    assert_eq!(replacement_record.identity[0], replacement.generation());
    assert_ne!(
        replacement_record.identity[1] & GPU_BODY_OBSERVATION_LIVE,
        0
    );
}

fn submit_steps(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixed_dt: f32,
    count: u32,
) -> physbox_sim::StepReport {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_test_steps"),
    });
    let mut report = None;
    for _ in 0..count {
        report = Some(
            world
                .step(device, queue, &mut encoder, StepDef::fixed(fixed_dt))
                .unwrap(),
        );
    }
    queue.submit([encoder.finish()]);
    report.expect("submit at least one public world step")
}

fn submit_command_step(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixed_dt: f32,
) -> WorldCommandResults {
    submit_steps(world, device, queue, fixed_dt, 1);
    take_command_results(world, device, queue)
}

fn take_command_results(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> WorldCommandResults {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_test_results"),
    });
    let mut request = world.request_command_results(device, &mut encoder).unwrap();
    queue.submit([encoder.finish()]);
    request.begin_map().unwrap();
    device.poll(wgpu::PollType::Wait).unwrap();
    world
        .try_take_command_results(&mut request)
        .expect("result callback completed")
        .expect("result batch decoded")
}

fn take_particle_counts(
    world: &World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> physbox_sim::ParticleCountSnapshot {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_test_particle_counts"),
    });
    let mut request = world
        .request_particle_count_snapshot(device, &mut encoder)
        .unwrap();
    queue.submit([encoder.finish()]);
    request.begin_map().unwrap();
    device.poll(wgpu::PollType::Wait).unwrap();
    request
        .try_take()
        .expect("particle-count callback completed")
        .expect("particle-count snapshot decoded")
}

fn assert_result(
    results: &WorldCommandResults,
    kind: WorldCommandKind,
    status: WorldCommandStatus,
    body: Option<physbox_sim::BodyId>,
) {
    assert!(
        results
            .results
            .iter()
            .any(|result| result.kind == kind && result.status == status && result.body == body),
        "missing {kind:?}/{status:?} result for {body:?}: {:?}",
        results.results
    );
}

fn observe_body(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    observation: &physbox_sim::GpuBodyObservation,
    slot: u32,
) -> GpuBodyObservationRecord {
    let byte_count =
        u64::from(observation.capacity()) * size_of::<GpuBodyObservationRecord>() as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_sim_public_test_observation"),
        size: byte_count,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_test_observation_copy"),
    });
    observation.encode(&mut encoder);
    encoder.copy_buffer_to_buffer(observation.buffer(), 0, &readback, 0, byte_count);
    queue.submit([encoder.finish()]);
    let completion = Arc::new(Mutex::new(None));
    let callback_completion = Arc::clone(&completion);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback_completion.lock().unwrap() = Some(result);
        });
    device.poll(wgpu::PollType::Wait).unwrap();
    completion
        .lock()
        .unwrap()
        .take()
        .expect("observation callback completed")
        .expect("observation mapping succeeded");
    let mapped = readback.slice(..).get_mapped_range();
    let stride = size_of::<GpuBodyObservationRecord>();
    let start = slot as usize * stride;
    let record = bytemuck::pod_read_unaligned(&mapped[start..start + stride]);
    drop(mapped);
    readback.unmap();
    record
}

fn observe_particles(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    observation: &physbox_sim::GpuParticleObservation,
) -> Vec<GpuParticleObservationRecord> {
    let byte_count =
        u64::from(observation.capacity()) * size_of::<GpuParticleObservationRecord>() as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_sim_public_test_particle_observation"),
        size: byte_count,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_sim_public_test_particle_observation_copy"),
    });
    observation.encode(&mut encoder);
    encoder.copy_buffer_to_buffer(observation.buffer(), 0, &readback, 0, byte_count);
    queue.submit([encoder.finish()]);
    let completion = Arc::new(Mutex::new(None));
    let callback_completion = Arc::clone(&completion);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback_completion.lock().unwrap() = Some(result);
        });
    device.poll(wgpu::PollType::Wait).unwrap();
    completion
        .lock()
        .unwrap()
        .take()
        .expect("particle observation callback completed")
        .expect("particle observation mapping succeeded");
    let mapped = readback.slice(..).get_mapped_range();
    let records = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    records
}
