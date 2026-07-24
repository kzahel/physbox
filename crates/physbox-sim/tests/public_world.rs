use physbox_sim::{BodyDef, ShapeDef, StepDef, World, WorldDef};

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
    assert_eq!(world.render_view().unwrap().version(), 1);

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
