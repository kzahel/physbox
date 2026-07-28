use std::mem::size_of;
use std::sync::mpsc;

use physbox_sim::{
    BodyDef, BodyId, CommandQueueErrorKind, GpuBodyObservationRecord, GpuParticleObservationRecord,
    ParticleRegionDef, ParticleSystemDef, PointConstraintDef, PointQueryDef, ShapeDef, StepDef,
    WORLD_COMMAND_CAPACITY, World, WorldCommandKind, WorldCommandResults, WorldCommandStatus,
    WorldDef,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("physbox_external_consumer"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))?;

    let mut definition = WorldDef::default();
    definition.quality.physics.sleeping_enabled = false;
    let fixed_dt = definition.quality.physics.fixed_dt_seconds();
    let mut world = World::new(&device, definition)?;
    let floor = world.create_body(BodyDef::fixed().with_position([0.0, -0.15]))?;
    world.create_shape(floor, ShapeDef::rectangle([1.7, 0.15]))?;
    let ball = world.create_body(BodyDef::dynamic().with_position([0.0, 0.9]))?;
    world.create_shape(ball, ShapeDef::circle(0.22).with_density(1.0))?;

    submit_steps(&mut world, &device, &queue, fixed_dt, 1)?;

    let missed = world.queue_point_query(PointQueryDef::at([10.0, 10.0]))?;
    let missed_constraint = world.queue_create_point_constraint(
        missed.selection,
        PointConstraintDef::at_target([10.0, 10.0]),
    )?;
    world.queue_set_point_constraint_target(missed_constraint.constraint, [9.0, 9.0])?;
    world.queue_destroy_point_constraint(missed_constraint.constraint)?;
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    require_result(
        &results,
        WorldCommandKind::PointQuery,
        WorldCommandStatus::Missed,
        None,
    )?;
    require_result(
        &results,
        WorldCommandKind::CreatePointConstraint,
        WorldCommandStatus::Missed,
        None,
    )?;
    require_result(
        &results,
        WorldCommandKind::SetPointConstraintTarget,
        WorldCommandStatus::Cancelled,
        None,
    )?;
    require_result(
        &results,
        WorldCommandKind::DestroyPointConstraint,
        WorldCommandStatus::Cancelled,
        None,
    )?;

    let selection = world.queue_point_query(PointQueryDef::at([0.0, 0.9]))?;
    let stale_selection_error = world
        .queue_destroy_selected_body(selection.selection)
        .and_then(|_| world.queue_destroy_selected_body(selection.selection))
        .expect_err("a deferred selection may be consumed only once");
    if stale_selection_error.kind() != CommandQueueErrorKind::InvalidSelection {
        return Err("selection reuse did not report InvalidSelection".into());
    }
    let cancelled = world.cancel_queued_commands();
    if cancelled.results.len() != 2
        || cancelled
            .results
            .iter()
            .any(|result| result.status != WorldCommandStatus::Cancelled)
    {
        return Err("queued-command cancellation did not terminate every accepted command".into());
    }

    let selection = world.queue_point_query(PointQueryDef::at([0.0, 0.9]))?;
    let constraint = world.queue_create_point_constraint(
        selection.selection,
        PointConstraintDef::at_target([0.0, 0.9]),
    )?;
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    require_result(
        &results,
        WorldCommandKind::PointQuery,
        WorldCommandStatus::Succeeded,
        Some(ball),
    )?;
    require_result(
        &results,
        WorldCommandKind::CreatePointConstraint,
        WorldCommandStatus::Succeeded,
        Some(ball),
    )?;

    world.queue_set_point_constraint_target(constraint.constraint, [0.45, 0.9])?;
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    require_result(
        &results,
        WorldCommandKind::SetPointConstraintTarget,
        WorldCommandStatus::Succeeded,
        Some(ball),
    )?;
    submit_steps(&mut world, &device, &queue, fixed_dt, 30)?;

    let observation = world.create_gpu_body_observation(&device)?;
    let records = read_observation(&device, &queue, &observation)?;
    let moved_ball = records[ball.slot() as usize];
    if moved_ball.identity[0] != ball.generation()
        || moved_ball.identity[1] & physbox_sim::GPU_BODY_OBSERVATION_LIVE == 0
        || moved_ball.pose[0] <= 0.05
    {
        return Err("point constraint did not move the selected ball".into());
    }

    world.queue_destroy_point_constraint(constraint.constraint)?;
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    require_result(
        &results,
        WorldCommandKind::DestroyPointConstraint,
        WorldCommandStatus::Succeeded,
        Some(ball),
    )?;

    let erase_selection =
        world.queue_point_query(PointQueryDef::at([moved_ball.pose[0], moved_ball.pose[1]]))?;
    world.queue_destroy_selected_body(erase_selection.selection)?;
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    require_result(
        &results,
        WorldCommandKind::DestroySelectedBody,
        WorldCommandStatus::Succeeded,
        Some(ball),
    )?;
    if world.counters().live_bodies != 1 {
        return Err("selected-body retirement was not reconciled into World".into());
    }

    let replacement = world.create_body(BodyDef::dynamic().with_position([-0.8, 1.2]))?;
    if replacement.slot() != ball.slot() || replacement.generation() != ball.generation() + 1 {
        return Err("retired body slot was not reused with the next generation".into());
    }
    world.create_shape(
        replacement,
        ShapeDef::circle(0.25)
            .with_density(0.8)
            .with_friction(0.5)
            .with_restitution(0.0),
    )?;
    submit_steps(&mut world, &device, &queue, fixed_dt, 1)?;
    let records = read_observation(&device, &queue, &observation)?;
    let replacement_record = records[replacement.slot() as usize];
    if replacement_record.identity[0] != replacement.generation()
        || replacement_record.identity[1] & physbox_sim::GPU_BODY_OBSERVATION_LIVE == 0
    {
        return Err("reused body slot was not live in the canonical GPU observation".into());
    }

    for _ in 0..WORLD_COMMAND_CAPACITY {
        world.queue_point_query(PointQueryDef::at([10.0, 10.0]))?;
    }
    let overflow = world
        .queue_point_query(PointQueryDef::at([10.0, 10.0]))
        .expect_err("the bounded command queue must reject its sixty-fifth command");
    if overflow.kind() != CommandQueueErrorKind::QueueFull {
        return Err("command overflow did not report QueueFull".into());
    }
    let results = submit_command_step(&mut world, &device, &queue, fixed_dt)?;
    if results.results.len() != WORLD_COMMAND_CAPACITY as usize
        || results
            .results
            .iter()
            .any(|result| result.status != WorldCommandStatus::Missed)
    {
        return Err("result-capacity batch lost an accepted point query".into());
    }

    let water_particles = prove_public_water(&device, &queue)?;
    println!(
        "PHYSBOX_EXTERNAL_CONSUMER_SUCCESS bodies={} steps={} moved_x={:.6} replacement={}:{} sleeping={} water_particles={water_particles}",
        world.counters().live_bodies,
        world.counters().outer_steps,
        moved_ball.pose[0],
        replacement.slot(),
        replacement.generation(),
        definition.quality.physics.sleeping_enabled
    );
    Ok(())
}

fn prove_public_water(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<usize, Box<dyn std::error::Error>> {
    let definition = WorldDef::default();
    let mut world = World::new(device, definition)?;
    let floor = world.create_body(BodyDef::fixed().with_position([0.0, -0.15]))?;
    world.create_shape(floor, ShapeDef::rectangle([1.7, 0.15]))?;
    let ball = world.create_body(BodyDef::dynamic().with_position([0.0, 0.65]))?;
    world.create_shape(ball, ShapeDef::circle(0.18).with_density(0.8))?;
    world.set_particle_system(
        ParticleSystemDef::water(64).with_region(ParticleRegionDef::rectangle(
            [-0.3, 0.0],
            [0.3, 0.4],
            0.1,
        )),
    )?;
    submit_steps(
        &mut world,
        device,
        queue,
        definition.quality.physics.fixed_dt_seconds(),
        1,
    )?;
    let observation = world.create_gpu_particle_observation(device)?;
    let records = read_particle_observation(device, queue, &observation)?;
    let live = records
        .iter()
        .filter(|record| record.identity[1] & physbox_sim::GPU_PARTICLE_OBSERVATION_LIVE != 0)
        .count();
    if live == 0 {
        return Err("public particle observation contained no live water".into());
    }
    Ok(live)
}

fn submit_steps(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixed_dt: f32,
    count: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_steps"),
    });
    for _ in 0..count {
        world.step(device, queue, &mut encoder, StepDef::fixed(fixed_dt))?;
    }
    queue.submit([encoder.finish()]);
    Ok(())
}

fn submit_command_step(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixed_dt: f32,
) -> Result<WorldCommandResults, Box<dyn std::error::Error>> {
    submit_steps(world, device, queue, fixed_dt, 1)?;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_command_results"),
    });
    let mut request = world.request_command_results(device, &mut encoder)?;
    queue.submit([encoder.finish()]);
    request.begin_map()?;
    device.poll(wgpu::PollType::Wait)?;
    world
        .try_take_command_results(&mut request)
        .ok_or_else(|| "command-result callback did not complete".into())
        .and_then(|results| results.map_err(Into::into))
}

fn require_result(
    results: &WorldCommandResults,
    kind: WorldCommandKind,
    status: WorldCommandStatus,
    body: Option<BodyId>,
) -> Result<(), Box<dyn std::error::Error>> {
    if results
        .results
        .iter()
        .any(|result| result.kind == kind && result.status == status && result.body == body)
    {
        Ok(())
    } else {
        Err(format!("missing {kind:?}/{status:?} result for {body:?}").into())
    }
}

fn read_observation(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    observation: &physbox_sim::GpuBodyObservation,
) -> Result<Vec<GpuBodyObservationRecord>, Box<dyn std::error::Error>> {
    let byte_count =
        u64::from(observation.capacity()) * size_of::<GpuBodyObservationRecord>() as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_external_observation_readback"),
        size: byte_count,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_observe"),
    });
    observation.encode(&mut encoder);
    encoder.copy_buffer_to_buffer(observation.buffer(), 0, &readback, 0, byte_count);
    queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::Wait)?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mapped = readback.slice(..).get_mapped_range();
    let records = bytemuck::cast_slice::<u8, GpuBodyObservationRecord>(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    Ok(records)
}

fn read_particle_observation(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    observation: &physbox_sim::GpuParticleObservation,
) -> Result<Vec<GpuParticleObservationRecord>, Box<dyn std::error::Error>> {
    let byte_count =
        u64::from(observation.capacity()) * size_of::<GpuParticleObservationRecord>() as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_external_particle_observation_readback"),
        size: byte_count,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_observe_particles"),
    });
    observation.encode(&mut encoder);
    encoder.copy_buffer_to_buffer(observation.buffer(), 0, &readback, 0, byte_count);
    queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::Wait)?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mapped = readback.slice(..).get_mapped_range();
    let records = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    Ok(records)
}
