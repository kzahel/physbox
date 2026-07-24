use super::*;
use crate::Camera2d;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn frozen_settled_water_pixels_ignore_active_list_order() {
    const VIEWPORT: [u32; 2] = [320, 200];
    const SETTLING_STEPS: u32 = 180;
    const SAMPLE_FRAMES: u32 = 8;

    let instance = crate::create_instance();
    let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
        .expect("test GPU adapter");
    let (device, queue) = pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
    let scenario = liquid_sandbox_scenario().unwrap();
    let body_storage = GpuBodyStorage::new(
        &device,
        scenario.rigid.bodies.len() as u32,
        BODY_COMMAND_CAPACITY,
        &scenario.rigid.bodies,
    )
    .unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let particles = GpuParticleWorld::new(&device, format, &scenario, &body_storage).unwrap();

    for batch in 0..SETTLING_STEPS.div_ceil(24) {
        let steps = (SETTLING_STEPS - batch * 24).min(24);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_flicker_settle"),
        });
        for _ in 0..steps {
            particles.encode_step(&mut encoder);
        }
        queue.submit([encoder.finish()]);
    }
    device.poll(wgpu::PollType::Wait).unwrap();

    let state = capture_particle_bytes(&device, &queue, &particles);
    let validation = particles.validation_layout();
    let metadata = particle_u32_lane(&state, validation.metadata_offset, validation.capacity);
    let mut active_slots = metadata
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| (value[3] != 0).then_some(slot as u32))
        .collect::<Vec<_>>();
    let inactive_slots = metadata
        .iter()
        .enumerate()
        .filter_map(|(slot, value)| (value[3] == 0).then_some(slot as u32))
        .collect::<Vec<_>>();
    assert!(active_slots.len() > 1_000, "settled water remains dense");

    let scramble_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("physbox5_particle_flicker_scramble_layout"),
        entries: &[
            storage_layout_entry(0, true, wgpu::ShaderStages::COMPUTE),
            storage_layout_entry(1, false, wgpu::ShaderStages::COMPUTE),
        ],
    });
    let scramble_shader = shader(
        &device,
        "physbox5_particle_flicker_scramble",
        "
@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> destination: array<u32>;

@compute @workgroup_size(64)
fn copy_order(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x < arrayLength(&source) {
        destination[invocation.x] = source[invocation.x];
    }
}
",
    );
    let scramble_pipeline = compute(
        &device,
        "physbox5_particle_flicker_scramble",
        &scramble_layout,
        &scramble_shader,
        "copy_order",
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("physbox5_particle_flicker_target"),
        size: wgpu::Extent3d {
            width: VIEWPORT[0],
            height: VIEWPORT[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let camera = Camera2d::try_new_at_home([0.0, 2.5], 3.5)
        .unwrap()
        .resolve(VIEWPORT)
        .unwrap();
    let mut reference = None;

    for frame in 0..SAMPLE_FRAMES {
        if frame % 2 == 1 {
            active_slots.reverse();
        }
        let active_count = active_slots.len();
        active_slots.rotate_left((frame as usize * 521) % active_count);
        let mut order = active_slots.clone();
        order.extend_from_slice(&inactive_slots);
        assert_eq!(order.len(), particles.capacity() as usize);
        let source = storage_init(
            &device,
            "physbox5_particle_flicker_order",
            &order,
            wgpu::BufferUsages::STORAGE,
        );
        let scramble_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_particle_flicker_scramble_group"),
            layout: &scramble_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particles._active_indices.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_flicker_frame"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_particle_flicker_scramble_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&scramble_pipeline);
            pass.set_bind_group(0, &scramble_group, &[]);
            pass.dispatch_workgroups(particles.capacity().div_ceil(64), 1, 1);
        }
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("physbox5_particle_flicker_clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        particles.encode_render(
            &queue,
            &mut encoder,
            RenderFrameTarget {
                color_view: &view,
                size: VIEWPORT,
            },
            camera,
            0,
            false,
        );
        queue.submit([encoder.finish()]);
        let pixels =
            crate::renderer::read_texture(&device, &queue, &texture, VIEWPORT[0], VIEWPORT[1])
                .unwrap();
        assert!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0),
            "capture contains visible water"
        );
        if let Some(reference) = &reference {
            assert_eq!(
                &pixels, reference,
                "frozen water flickered at frame {frame}"
            );
        } else {
            reference = Some(pixels);
        }
    }
}
