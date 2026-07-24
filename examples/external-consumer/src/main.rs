use std::sync::{Arc, Mutex};

use physbox_render::{Background, PhysicsRenderer, RenderOptions, RenderTarget, ViewTransform};
use physbox_sim::{BodyDef, ShapeDef, StepDef, World, WorldDef};

const EXTENT: [u32; 2] = [64, 64];
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const PIXEL_BYTES: u64 = EXTENT[0] as u64 * EXTENT[1] as u64 * 4;

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
        label: Some("physbox_external_step"),
    });
    world.step(&device, &queue, &mut encoder, StepDef::fixed(fixed_dt))?;
    queue.submit([encoder.finish()]);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("physbox_external_target"),
        size: wgpu::Extent3d {
            width: EXTENT[0],
            height: EXTENT[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let renderer = PhysicsRenderer::new(&device, FORMAT, world.render_view()?)?;
    let pixels = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox_external_pixels"),
        size: PIXEL_BYTES,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox_external_render_and_observe"),
    });
    renderer.encode(
        &queue,
        &mut encoder,
        RenderTarget {
            color_view: &view,
            size: EXTENT,
        },
        RenderOptions {
            view: ViewTransform {
                center: [0.0, 0.6],
                half_extents: [2.0, 2.0],
            },
            background: Background::Transparent,
            ..RenderOptions::default()
        },
    )?;
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &pixels,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(EXTENT[0] * 4),
                rows_per_image: Some(EXTENT[1]),
            },
        },
        wgpu::Extent3d {
            width: EXTENT[0],
            height: EXTENT[1],
            depth_or_array_layers: 1,
        },
    );
    let mut snapshot = world.request_body_snapshot(&device, &mut encoder)?;
    queue.submit([encoder.finish()]);

    snapshot.begin_map()?;
    let pixel_ready = Arc::new(Mutex::new(None));
    let callback_ready = Arc::clone(&pixel_ready);
    pixels
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback_ready
                .lock()
                .expect("pixel completion lock is not poisoned") =
                Some(result.map_err(|error| error.to_string()));
        });
    device.poll(wgpu::PollType::Wait)?;
    let snapshot = snapshot
        .try_take()
        .ok_or("snapshot callback did not complete")??;
    pixel_ready
        .lock()
        .expect("pixel completion lock is not poisoned")
        .take()
        .ok_or("pixel callback did not complete")??;
    let bytes = pixels.slice(..).get_mapped_range();
    let visible_pixels = bytes.chunks_exact(4).filter(|pixel| pixel[3] != 0).count();
    drop(bytes);
    pixels.unmap();
    if visible_pixels == 0 {
        return Err("rendered texture contained no physics pixels".into());
    }
    println!(
        "PHYSBOX_EXTERNAL_CONSUMER_SUCCESS bodies={} visible_pixels={} sleeping={}",
        snapshot.bodies.len(),
        visible_pixels,
        definition.quality.physics.sleeping_enabled
    );
    Ok(())
}
