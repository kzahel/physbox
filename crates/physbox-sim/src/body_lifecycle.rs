use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::CapacityRecordV1;
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;

pub const KILL_ZONE_Y: f32 = -10.0;

const WORKGROUP_SIZE: u32 = 256;
const DISPATCHES_PER_STEP: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BodyLifecycleParamsRaw {
    counts: [u32; 4],
    kill_zone: [f32; 4],
}

pub struct GpuBodyLifecycle {
    bind_group: wgpu::BindGroup,
    classify_pipeline: wgpu::ComputePipeline,
    clear_grab_pipeline: wgpu::ComputePipeline,
    scan_blocks_pipeline: wgpu::ComputePipeline,
    scatter_pipeline: wgpu::ComputePipeline,
    finalize_pipeline: wgpu::ComputePipeline,
    body_capacity: u32,
    block_count: u32,
}

impl GpuBodyLifecycle {
    pub fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        construction_state: &wgpu::Buffer,
        grab_state: &wgpu::Buffer,
    ) -> Result<Self, String> {
        let body_capacity = storage.layout().capacity_elements;
        let block_count = body_capacity.div_ceil(WORKGROUP_SIZE);
        let scratch_elements = body_capacity
            .checked_add(block_count)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| "body lifecycle scratch capacity overflowed".to_owned())?;
        let scratch = scratch_buffer(
            device,
            "physbox5_body_lifecycle_scratch",
            u64::from(scratch_elements) * size_of::<u32>() as u64,
        )?;
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_body_lifecycle_params"),
            contents: bytemuck::bytes_of(&BodyLifecycleParamsRaw {
                counts: [body_capacity, block_count, 0, 0],
                kill_zone: [KILL_ZONE_Y, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_lifecycle_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            size_of::<BodyLifecycleParamsRaw>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_lifecycle_bind_group"),
            layout: &layout,
            entries: &[
                binding(0, storage.pose_buffer()),
                binding(1, storage.motion_buffer()),
                binding(2, storage.properties_buffer()),
                binding(3, storage.metadata_buffer()),
                binding(4, storage.active_body_buffer()),
                binding(5, construction_state),
                binding(6, grab_state),
                binding(7, &scratch),
                binding(8, &params),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_lifecycle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_lifecycle.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_body_lifecycle_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |label, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        Ok(Self {
            bind_group,
            classify_pipeline: pipeline(
                "physbox5_body_lifecycle_classify_pipeline",
                "classify_and_scan",
            ),
            clear_grab_pipeline: pipeline(
                "physbox5_body_lifecycle_clear_grab_pipeline",
                "clear_retired_grab",
            ),
            scan_blocks_pipeline: pipeline(
                "physbox5_body_lifecycle_scan_blocks_pipeline",
                "scan_blocks",
            ),
            scatter_pipeline: pipeline(
                "physbox5_body_lifecycle_scatter_pipeline",
                "scatter_live_bodies",
            ),
            finalize_pipeline: pipeline(
                "physbox5_body_lifecycle_finalize_pipeline",
                "finalize_active_bodies",
            ),
            body_capacity,
            block_count,
        })
    }

    pub fn encode_step(&self, encoder: &mut wgpu::CommandEncoder) {
        for (label, pipeline, workgroups) in [
            (
                "physbox5_body_lifecycle_classify_pass",
                &self.classify_pipeline,
                self.block_count,
            ),
            (
                "physbox5_body_lifecycle_clear_grab_pass",
                &self.clear_grab_pipeline,
                1,
            ),
            (
                "physbox5_body_lifecycle_scan_blocks_pass",
                &self.scan_blocks_pipeline,
                1,
            ),
            (
                "physbox5_body_lifecycle_scatter_pass",
                &self.scatter_pipeline,
                self.block_count,
            ),
            (
                "physbox5_body_lifecycle_finalize_pass",
                &self.finalize_pipeline,
                1,
            ),
        ] {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
    }

    pub const fn dispatches_per_step(&self) -> u32 {
        DISPATCHES_PER_STEP
    }

    pub fn allocated_bytes(&self) -> u64 {
        u64::from(self.body_capacity + self.block_count + 1) * size_of::<u32>() as u64
            + size_of::<BodyLifecycleParamsRaw>() as u64
    }

    pub fn capacity_records(&self) -> [CapacityRecordV1; 2] {
        [
            record(
                "body-lifecycle-scratch",
                self.body_capacity + self.block_count + 1,
            ),
            CapacityRecordV1 {
                class: "body-lifecycle-parameters".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: size_of::<BodyLifecycleParamsRaw>() as u64,
                high_water_bytes: size_of::<BodyLifecycleParamsRaw>() as u64,
                overflowed: false,
            },
        ]
    }
}

fn scratch_buffer(
    device: &wgpu::Device,
    label: &'static str,
    bytes: u64,
) -> Result<wgpu::Buffer, String> {
    let limits = device.limits();
    if bytes > limits.max_buffer_size || bytes > u64::from(limits.max_storage_buffer_binding_size) {
        return Err(format!(
            "{label} requires {bytes} bytes, exceeding portable device buffer limits"
        ));
    }
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    }))
}

fn record(class: &'static str, elements: u32) -> CapacityRecordV1 {
    let bytes = u64::from(elements) * size_of::<u32>() as u64;
    CapacityRecordV1 {
        class: class.to_owned(),
        capacity_elements: u64::from(elements),
        used_elements: u64::from(elements),
        allocated_bytes: bytes,
        high_water_bytes: bytes,
        overflowed: false,
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

fn binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
