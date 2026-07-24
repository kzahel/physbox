use physbox_sim::{BodyDef, ShapeDef, StepDef, World, WorldDef};

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

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_steps"),
    });
    for _ in 0..4 {
        world.step(&device, &queue, &mut encoder, StepDef::fixed(fixed_dt))?;
    }
    queue.submit([encoder.finish()]);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_observe"),
    });
    let mut snapshot = world.request_body_snapshot(&device, &mut encoder)?;
    queue.submit([encoder.finish()]);

    snapshot.begin_map()?;
    device.poll(wgpu::PollType::Wait)?;
    let snapshot = snapshot
        .try_take()
        .ok_or("snapshot callback did not complete")??;
    let ball_y = snapshot
        .bodies
        .iter()
        .find(|body| body.body == ball)
        .ok_or("ball was absent from the asynchronous snapshot")?
        .position[1];
    if !(ball_y.is_finite() && ball_y < 0.9) {
        return Err("ball did not advance under gravity".into());
    }
    println!(
        "PHYSBOX_EXTERNAL_CONSUMER_SUCCESS bodies={} steps={} ball_y={ball_y:.6} sleeping={}",
        snapshot.bodies.len(),
        world.counters().outer_steps,
        definition.quality.physics.sleeping_enabled
    );
    Ok(())
}
