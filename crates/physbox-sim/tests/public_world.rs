use std::mem::size_of;
use std::sync::{Arc, Mutex};

use physbox_sim::{
    BodyDef, GPU_BODY_OBSERVATION_LIVE, GPU_BODY_OBSERVATION_VERSION, GpuBodyObservationRecord,
    ShapeDef, StepDef, World, WorldDef,
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
