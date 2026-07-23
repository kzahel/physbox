use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;

const WORKGROUP_SIZE: u32 = 64;
const COMPONENT_STAT_WORDS: u32 = 16;
const POINT_DISPATCH_64_OFFSET: u64 = 10 * size_of::<u32>() as u64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ComponentParamsRaw {
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ExactParamsRaw {
    counts: [u32; 4],
}

struct ComponentPipelines {
    initialize: wgpu::ComputePipeline,
    hook: wgpu::ComputePipeline,
    shortcut: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    validate_edges: wgpu::ComputePipeline,
}

struct ExactPipelines {
    aggregate: wgpu::ComputePipeline,
    broadcast: wgpu::ComputePipeline,
}

/// Product-sized exact contact components and whole-component sleep reductions.
///
/// This is the applied counterpart of S26/S27. It retains only the fixed
/// working set needed for one current graph; validation trace buffers and CPU
/// readback are deliberately absent.
pub(crate) struct GpuExactSleepGrouping {
    component_pipelines: ComponentPipelines,
    component_group: wgpu::BindGroup,
    exact_pipelines: ExactPipelines,
    exact_group: wgpu::BindGroup,
    parents: wgpu::Buffer,
    component_values: wgpu::Buffer,
    stats: wgpu::Buffer,
    _component_params: wgpu::Buffer,
    _exact_params: wgpu::Buffer,
    component_params_stride: u32,
    body_capacity: u32,
    phase_bound: u32,
}

impl GpuExactSleepGrouping {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        solver_points: &wgpu::Buffer,
        body_capacity: u32,
        point_capacity: u32,
    ) -> Result<Self, String> {
        let phase_bound = component_phase_bound(body_capacity)?;
        let component_params_stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
        let params_count = phase_bound
            .checked_add(1)
            .ok_or_else(|| "exact sleep component parameter count overflowed".to_owned())?;
        let params_bytes = component_params_stride
            .checked_mul(params_count)
            .ok_or_else(|| "exact sleep component parameter bytes overflowed".to_owned())?;
        let mut parameter_data = vec![0_u8; params_bytes as usize];
        for phase in 0..=phase_bound {
            let value = ComponentParamsRaw {
                counts: [body_capacity, point_capacity, phase, phase_bound],
            };
            let start = phase as usize * component_params_stride as usize;
            parameter_data[start..start + size_of::<ComponentParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(&value));
        }

        let storage_buffer = |label, elements: u32| -> Result<wgpu::Buffer, String> {
            let size = u64::from(elements)
                .checked_mul(size_of::<u32>() as u64)
                .ok_or_else(|| format!("{label} byte count overflowed"))?;
            Ok(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        };
        let parents = storage_buffer("physbox5_exact_sleep_parents", body_capacity)?;
        let component_values =
            storage_buffer("physbox5_exact_sleep_component_values", body_capacity)?;
        let stats = storage_buffer("physbox5_exact_sleep_component_stats", COMPONENT_STAT_WORDS)?;
        let component_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_exact_sleep_component_params"),
            contents: &parameter_data,
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let component_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_exact_sleep_component_layout"),
            entries: &[
                storage_layout_entry(0, true),
                storage_layout_entry(1, true),
                storage_layout_entry(2, false),
                storage_layout_entry(3, false),
                storage_layout_entry(4, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            size_of::<ComponentParamsRaw>() as u64
                        ),
                    },
                    count: None,
                },
            ],
        });
        let component_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_exact_sleep_component_pipeline_layout"),
                bind_group_layouts: &[&component_layout],
                push_constant_ranges: &[],
            });
        let component_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_exact_sleep_component_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("contact_components.wgsl").into()),
        });
        let component_pipeline = |entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&component_pipeline_layout),
                module: &component_shader,
                entry_point: Some(entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let component_pipelines = ComponentPipelines {
            initialize: component_pipeline("initialize"),
            hook: component_pipeline("hook"),
            shortcut: component_pipeline("shortcut"),
            finalize: component_pipeline("finalize"),
            validate_edges: component_pipeline("validate_edges"),
        };
        let component_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_exact_sleep_component_group"),
            layout: &component_layout,
            entries: &[
                entry(0, storage.metadata_buffer()),
                entry(1, solver_points),
                entry(2, &parents),
                entry(3, &component_values),
                entry(4, &stats),
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &component_params,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<ComponentParamsRaw>() as u64),
                    }),
                },
            ],
        });

        let awake_work = storage.awake_work_handle();
        let exact_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_exact_sleep_params"),
            contents: bytemuck::bytes_of(&ExactParamsRaw {
                counts: [body_capacity, 0, 0, 0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let exact_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_exact_sleep_layout"),
            entries: &[
                storage_layout_entry(0, true),
                storage_layout_entry(1, false),
                storage_layout_entry(2, false),
                storage_layout_entry(3, false),
                storage_layout_entry(4, true),
                storage_layout_entry(5, false),
                storage_layout_entry(6, false),
                uniform_layout_entry(7, size_of::<ExactParamsRaw>() as u64),
            ],
        });
        let exact_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_exact_sleep_pipeline_layout"),
                bind_group_layouts: &[&exact_layout],
                push_constant_ranges: &[],
            });
        let exact_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_exact_sleep_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("exact_sleep_grouping.wgsl").into()),
        });
        let exact_pipeline = |entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&exact_pipeline_layout),
                module: &exact_shader,
                entry_point: Some(entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let exact_pipelines = ExactPipelines {
            aggregate: exact_pipeline("aggregate_component_values"),
            broadcast: exact_pipeline("broadcast_component_values"),
        };
        let exact_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_exact_sleep_group"),
            layout: &exact_layout,
            entries: &[
                entry(0, storage.metadata_buffer()),
                entry(1, &parents),
                entry(2, &component_values),
                entry(3, &stats),
                entry(4, awake_work.sleep_state_buffer()),
                entry(5, awake_work.wake_frontier_buffer(0)),
                entry(6, awake_work.wake_frontier_buffer(1)),
                entry(7, &exact_params),
            ],
        });

        Ok(Self {
            component_pipelines,
            component_group,
            exact_pipelines,
            exact_group,
            parents,
            component_values,
            stats,
            _component_params: component_params,
            _exact_params: exact_params,
            component_params_stride,
            body_capacity,
            phase_bound,
        })
    }

    pub(crate) fn encode_components(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        point_capacity_state: &wgpu::Buffer,
    ) {
        encoder.clear_buffer(&self.component_values, 0, None);
        encoder.clear_buffer(&self.stats, 0, None);
        self.direct_component_pass(
            encoder,
            "physbox5_exact_sleep_component_initialize",
            &self.component_pipelines.initialize,
            0,
        );
        for phase in 1..=self.phase_bound {
            self.indirect_component_pass(
                encoder,
                "physbox5_exact_sleep_component_hook",
                &self.component_pipelines.hook,
                phase,
                point_capacity_state,
            );
            self.direct_component_pass(
                encoder,
                "physbox5_exact_sleep_component_shortcut",
                &self.component_pipelines.shortcut,
                phase,
            );
        }
        self.direct_component_pass(
            encoder,
            "physbox5_exact_sleep_component_finalize",
            &self.component_pipelines.finalize,
            0,
        );
        self.indirect_component_pass(
            encoder,
            "physbox5_exact_sleep_component_validate",
            &self.component_pipelines.validate_edges,
            0,
            point_capacity_state,
        );
    }

    /// Reduces frontier A by exact component and broadcasts into frontier B.
    pub(crate) fn encode_frontier(&self, encoder: &mut wgpu::CommandEncoder) -> usize {
        encoder.clear_buffer(&self.component_values, 0, None);
        self.direct_exact_pass(
            encoder,
            "physbox5_exact_sleep_aggregate",
            &self.exact_pipelines.aggregate,
        );
        self.direct_exact_pass(
            encoder,
            "physbox5_exact_sleep_broadcast",
            &self.exact_pipelines.broadcast,
        );
        1
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        self.phase_bound * 2 + 7
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.parents.size()
            + self.component_values.size()
            + self.stats.size()
            + self._component_params.size()
            + self._exact_params.size()
    }

    fn direct_component_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        phase: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(
            0,
            &self.component_group,
            &[phase * self.component_params_stride],
        );
        pass.dispatch_workgroups(self.body_capacity.div_ceil(WORKGROUP_SIZE), 1, 1);
    }

    fn indirect_component_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        phase: u32,
        point_capacity_state: &wgpu::Buffer,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(
            0,
            &self.component_group,
            &[phase * self.component_params_stride],
        );
        pass.dispatch_workgroups_indirect(point_capacity_state, POINT_DISPATCH_64_OFFSET);
    }

    fn direct_exact_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.exact_group, &[]);
        pass.dispatch_workgroups(self.body_capacity.div_ceil(WORKGROUP_SIZE), 1, 1);
    }
}

fn component_phase_bound(body_capacity: u32) -> Result<u32, String> {
    if body_capacity == 0 {
        return Err("exact sleep grouping requires a nonzero body capacity".to_owned());
    }

    let logarithm = u32::BITS - body_capacity.saturating_sub(1).leading_zeros();
    logarithm
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "exact sleep grouping phase bound overflowed".to_owned())
}

fn entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn storage_layout_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
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

fn uniform_layout_entry(binding: u32, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}
