use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

use crate::GpuObservationError;

pub const GPU_PARTICLE_OBSERVATION_VERSION: u32 = 1;
pub const GPU_PARTICLE_OBSERVATION_LIVE: u32 = 1 << 0;

/// Stable, render-neutral particle state written by a GPU observation request.
///
/// Records are indexed by particle slot. `identity[0]` is the current
/// generation, `identity[1]` contains only public flags, `identity[2]` repeats
/// the slot, and `identity[3]` is the semantic material identifier. Consumers
/// must ignore position and motion when the live flag is absent.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuParticleObservationRecord {
    /// `[position_x, position_y, reserved, reserved]`.
    pub position: [f32; 4],
    /// `[linear_velocity_x, linear_velocity_y, reserved, reserved]`.
    pub motion: [f32; 4],
    /// `[generation, public_flags, slot, material_id]`.
    pub identity: [u32; 4],
}

const _: () = assert!(size_of::<GpuParticleObservationRecord>() == 48);

/// A same-device, explicitly encoded observation of public particle state.
///
/// This is a physics query, not a renderer. The buffer has `STORAGE` and
/// `COPY_SRC` usage and contains no private grid, density, pressure,
/// correction, coupling, dispatch, camera, target, or draw-command state.
pub struct GpuParticleObservation {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    capacity: u32,
    particle_radius: f32,
}

impl GpuParticleObservation {
    pub(crate) fn new(
        device: &wgpu::Device,
        positions: &wgpu::Buffer,
        velocities: &wgpu::Buffer,
        metadata: &wgpu::Buffer,
        capacity: u32,
        particle_radius: f32,
    ) -> Result<Self, GpuObservationError> {
        let byte_size = u64::from(capacity)
            .checked_mul(size_of::<GpuParticleObservationRecord>() as u64)
            .ok_or_else(|| GpuObservationError::new("GPU particle observation size overflowed"))?;
        let limits = device.limits();
        if byte_size > limits.max_buffer_size
            || byte_size > u64::from(limits.max_storage_buffer_binding_size)
        {
            return Err(GpuObservationError::new(
                "GPU particle observation exceeds the device storage-buffer limit",
            ));
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox_sim_public_particle_observation"),
            size: byte_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox_sim_public_particle_observation_layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, false),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox_sim_public_particle_observation_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: positions.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: velocities.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: metadata.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox_sim_public_particle_observation_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_particle_observation.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox_sim_public_particle_observation_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("physbox_sim_public_particle_observation_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("observe_particles"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self {
            buffer,
            bind_group,
            pipeline,
            capacity,
            particle_radius,
        })
    }

    pub const fn version(&self) -> u32 {
        GPU_PARTICLE_OBSERVATION_VERSION
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub const fn particle_radius(&self) -> f32 {
        self.particle_radius
    }

    pub const fn record_stride_bytes(&self) -> u32 {
        size_of::<GpuParticleObservationRecord>() as u32
    }

    pub const fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Records a current-state projection into the public observation buffer.
    ///
    /// The caller owns submission and ordering relative to simulation and its
    /// own GPU consumers.
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox_sim_public_particle_observation_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch_workgroups(self.capacity.div_ceil(64), 1, 1);
    }
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
