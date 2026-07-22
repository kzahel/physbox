use std::{mem::size_of, sync::Arc};

use bytemuck::cast_slice;
use physbox_contracts::CapacityRecordV1;
use wgpu::util::DeviceExt;

use crate::{
    GpuBodyStorage,
    body_floor_solver::{PointInputRaw, PreparedPointRaw},
};

const POINT_WORK_STATE_WORDS: u32 = 16;
const SOLVER_BODY_STATE_WORDS: u32 = 20;
const POINT_DISPATCH_64_OFFSET: u64 = 4 * size_of::<u32>() as u64;

pub(crate) struct GpuAwakePoints {
    points: wgpu::Buffer,
    state: Arc<wgpu::Buffer>,
    mapping: wgpu::Buffer,
    persistence_prepared: wgpu::Buffer,
    solver_bodies: Arc<wgpu::Buffer>,
    compact_bind_group: wgpu::BindGroup,
    solver_body_bind_group: wgpu::BindGroup,
    scatter_bind_group: wgpu::BindGroup,
    compact_pipeline: wgpu::ComputePipeline,
    solver_body_pipeline: wgpu::ComputePipeline,
    scatter_pipeline: wgpu::ComputePipeline,
    clear_pipeline: wgpu::ComputePipeline,
    _params: wgpu::Buffer,
    point_capacity: u32,
}

impl GpuAwakePoints {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        source_points: &wgpu::Buffer,
        source_state: &wgpu::Buffer,
        point_capacity: u32,
    ) -> Result<Self, String> {
        if point_capacity == 0 {
            return Err("awake-point capacity must be positive".to_owned());
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let buffer = |label, elements: u64, stride: u64, extra| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: elements * stride,
                usage: usage | extra,
                mapped_at_creation: false,
            })
        };
        let points = buffer(
            "physbox5_awake_solver_points",
            u64::from(point_capacity),
            size_of::<PointInputRaw>() as u64,
            wgpu::BufferUsages::empty(),
        );
        let state = Arc::new(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("physbox5_awake_point_state"),
                contents: cast_slice(&[0_u32; POINT_WORK_STATE_WORDS as usize]),
                usage: usage | wgpu::BufferUsages::INDIRECT,
            }),
        );
        let mapping = buffer(
            "physbox5_awake_point_mapping",
            u64::from(point_capacity),
            size_of::<u32>() as u64,
            wgpu::BufferUsages::empty(),
        );
        let persistence_prepared = buffer(
            "physbox5_persistence_prepared_points",
            u64::from(point_capacity),
            size_of::<PreparedPointRaw>() as u64,
            wgpu::BufferUsages::empty(),
        );
        let solver_bodies = Arc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("physbox5_solver_body_state"),
                contents: cast_slice(&vec![
                    0_u32;
                    (SOLVER_BODY_STATE_WORDS + storage.layout().capacity_elements)
                        as usize
                ]),
                usage: usage | wgpu::BufferUsages::INDIRECT,
            },
        ));
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_awake_point_params"),
            contents: cast_slice(&[point_capacity, storage.layout().capacity_elements, 0, 0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let compact_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_awake_point_compact_layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, true),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                uniform_entry(8),
            ],
        });
        let compact_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_awake_point_compact_bind_group"),
            layout: &compact_layout,
            entries: &[
                entry(0, source_points),
                entry(1, source_state),
                entry(2, storage.metadata_buffer()),
                entry(3, storage.awake_work_handle().sleep_state_buffer()),
                entry(4, &points),
                entry(5, &state),
                entry(6, &mapping),
                entry(7, &persistence_prepared),
                entry(8, &params),
            ],
        });
        let scatter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_awake_point_scatter_layout"),
            entries: &[
                storage_entry(5, true),
                storage_entry(6, true),
                storage_entry(7, false),
                storage_entry(9, false),
            ],
        });
        let solver_body_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_solver_body_compact_layout"),
                entries: &[
                    storage_entry(0, true),
                    storage_entry(1, true),
                    storage_entry(2, true),
                    storage_entry(3, true),
                    storage_entry(4, false),
                    uniform_entry(5),
                ],
            });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_awake_point_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("awake_points.wgsl").into()),
        });
        let scatter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_awake_point_scatter_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("awake_point_scatter.wgsl").into()),
        });
        let solver_body_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_solver_body_compact_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("solver_bodies.wgsl").into()),
        });
        let pipeline =
            |label, entry_point, layout: &wgpu::BindGroupLayout, module: &wgpu::ShaderModule| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(label),
                        bind_group_layouts: &[layout],
                        push_constant_ranges: &[],
                    });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    module,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
        let compact_pipeline = pipeline(
            "physbox5_awake_point_compact_pipeline",
            "compact_awake_points",
            &compact_layout,
            &shader,
        );
        let scatter_pipeline = pipeline(
            "physbox5_awake_point_scatter_pipeline",
            "scatter_prepared_points",
            &scatter_layout,
            &scatter_shader,
        );
        let solver_body_pipeline = pipeline(
            "physbox5_solver_body_compact_pipeline",
            "compact_solver_bodies",
            &solver_body_layout,
            &solver_body_shader,
        );
        let clear_pipeline = pipeline(
            "physbox5_awake_point_clear_pipeline",
            "clear_stale_prepared_points",
            &scatter_layout,
            &scatter_shader,
        );
        let scatter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_awake_point_scatter_bind_group_unbound"),
            layout: &scatter_layout,
            entries: &[
                entry(5, &state),
                entry(6, &mapping),
                entry(7, &persistence_prepared),
                entry(9, &persistence_prepared),
            ],
        });
        let solver_body_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_solver_body_compact_bind_group"),
            layout: &solver_body_layout,
            entries: &[
                entry(0, storage.metadata_buffer()),
                entry(1, storage.awake_body_buffer()),
                entry(2, &points),
                entry(3, &state),
                entry(4, &solver_bodies),
                entry(5, &params),
            ],
        });
        Ok(Self {
            points,
            state,
            mapping,
            persistence_prepared,
            solver_bodies,
            compact_bind_group,
            solver_body_bind_group,
            scatter_bind_group,
            compact_pipeline,
            solver_body_pipeline,
            scatter_pipeline,
            clear_pipeline,
            _params: params,
            point_capacity,
        })
    }

    pub(crate) fn bind_solver_prepared(&mut self, device: &wgpu::Device, prepared: &wgpu::Buffer) {
        let layout = self.scatter_pipeline.get_bind_group_layout(0);
        self.scatter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_awake_point_scatter_bind_group"),
            layout: &layout,
            entries: &[
                entry(5, &self.state),
                entry(6, &self.mapping),
                entry(7, &self.persistence_prepared),
                entry(9, prepared),
            ],
        });
    }

    pub(crate) fn encode_compaction(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_awake_point_compact_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compact_pipeline);
        pass.set_bind_group(0, &self.compact_bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub(crate) fn encode_scatter(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_awake_point_scatter_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.scatter_pipeline);
        pass.set_bind_group(0, &self.scatter_bind_group, &[]);
        pass.dispatch_workgroups_indirect(&self.state, POINT_DISPATCH_64_OFFSET);
        pass.set_pipeline(&self.clear_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub(crate) fn encode_solver_body_compaction(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_solver_body_compact_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.solver_body_pipeline);
        pass.set_bind_group(0, &self.solver_body_bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub(crate) fn points(&self) -> &wgpu::Buffer {
        &self.points
    }

    pub(crate) fn state_handle(&self) -> Arc<wgpu::Buffer> {
        Arc::clone(&self.state)
    }

    pub(crate) fn solver_body_state_handle(&self) -> Arc<wgpu::Buffer> {
        Arc::clone(&self.solver_bodies)
    }

    pub(crate) fn persistence_prepared(&self) -> &wgpu::Buffer {
        &self.persistence_prepared
    }

    pub(crate) fn capacity_record(&self) -> CapacityRecordV1 {
        let bytes = self.points.size()
            + self.state.size()
            + self.mapping.size()
            + self.persistence_prepared.size()
            + self._params.size();
        CapacityRecordV1 {
            class: "awake-point-work".to_owned(),
            capacity_elements: u64::from(self.point_capacity),
            used_elements: u64::from(self.point_capacity),
            allocated_bytes: bytes,
            high_water_bytes: bytes,
            overflowed: false,
        }
    }

    pub(crate) fn solver_body_capacity_record(&self) -> CapacityRecordV1 {
        CapacityRecordV1 {
            class: "solver-body-work".to_owned(),
            capacity_elements: self.solver_bodies.size() / size_of::<u32>() as u64
                - u64::from(SOLVER_BODY_STATE_WORDS),
            used_elements: 0,
            allocated_bytes: self.solver_bodies.size(),
            high_water_bytes: self.solver_bodies.size(),
            overflowed: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.state,
            0,
            destination,
            0,
            u64::from(POINT_WORK_STATE_WORDS) * size_of::<u32>() as u64,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_mapping(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.mapping,
            0,
            destination,
            0,
            u64::from(self.point_capacity) * size_of::<u32>() as u64,
        );
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

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
