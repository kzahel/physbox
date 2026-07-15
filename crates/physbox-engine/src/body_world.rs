use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{BodyCommandBatchV1, BodyLayoutV1, CapacityRecordV1, ScenarioV1, Validate};
use wgpu::util::DeviceExt;

#[cfg(not(target_arch = "wasm32"))]
use crate::body_floor_solver::FixedFloorEvidenceLayout;
use crate::body_state::{BODY_COMMAND_BYTES, BODY_COMMAND_CAPACITY, GpuBodyResetData};
use crate::{GpuBodyStorage, body_floor_solver::GpuFixedFloorSolver};

const WORKGROUP_SIZE: u32 = 64;
const COMMAND_PARAMETER_STRIDE: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct IntegrationParamsRaw {
    dt_gravity: [f32; 4],
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParamsRaw {
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CommandParamsRaw {
    counts: [u32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BodyCommandStepRange {
    pub(crate) start: u32,
    pub(crate) count: u32,
}

pub(crate) struct GpuBodyWorld {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    storage: GpuBodyStorage,
    integration_bind_group: wgpu::BindGroup,
    integration_pipeline: wgpu::ComputePipeline,
    command_params: wgpu::Buffer,
    command_bind_group: wgpu::BindGroup,
    command_pipeline: wgpu::ComputePipeline,
    hold_pipeline: wgpu::ComputePipeline,
    render_params: wgpu::Buffer,
    render_bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    floor_solver: Option<GpuFixedFloorSolver>,
    body_count: u32,
}

impl GpuBodyWorld {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "GPU body count exceeds u32".to_owned())?;
        if body_count == 0 {
            return Err("GPU body world requires at least one body".to_owned());
        }
        let storage =
            GpuBodyStorage::new(device, body_count, BODY_COMMAND_CAPACITY, &scenario.bodies)?;
        let floor_solver = if scenario.manifolds.is_empty() {
            None
        } else {
            Some(GpuFixedFloorSolver::new(device, &storage, scenario)?)
        };
        let integration_params = IntegrationParamsRaw {
            dt_gravity: [
                scenario.settings.fixed_dt.get(),
                scenario.settings.gravity.x,
                scenario.settings.gravity.y,
                0.0,
            ],
            counts: [body_count, 0, 0, 0],
        };
        let integration_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_body_integration_params"),
            contents: bytemuck::bytes_of(&integration_params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let integration_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_body_integration_layout"),
                entries: &[
                    storage_entry(0, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(1, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(2, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(3, true, wgpu::ShaderStages::COMPUTE),
                    uniform_entry(4, wgpu::ShaderStages::COMPUTE),
                ],
            });
        let integration_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_integration_bind_group"),
            layout: &integration_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.motion_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: storage.properties_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: integration_params.as_entire_binding(),
                },
            ],
        });
        let integration_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_integration_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_integrate.wgsl").into()),
        });
        let integration_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_integration_pipeline_layout"),
                bind_group_layouts: &[&integration_layout],
                push_constant_ranges: &[],
            });
        let integration_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_body_integration_pipeline"),
                layout: Some(&integration_pipeline_layout),
                module: &integration_shader,
                entry_point: Some("integrate_bodies"),
                compilation_options: Default::default(),
                cache: None,
            });

        let command_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_command_params"),
            size: u64::from(BODY_COMMAND_CAPACITY + 1) * COMMAND_PARAMETER_STRIDE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let command_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_command_layout"),
            entries: &[
                storage_entry(0, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(1, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(2, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(3, true, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            size_of::<CommandParamsRaw>() as u64
                        ),
                    },
                    count: None,
                },
            ],
        });
        let command_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_command_bind_group"),
            layout: &command_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.motion_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: storage.command_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &command_params,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<CommandParamsRaw>() as u64),
                    }),
                },
            ],
        });
        let command_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_command_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_commands.wgsl").into()),
        });
        let command_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_command_pipeline_layout"),
                bind_group_layouts: &[&command_layout],
                push_constant_ranges: &[],
            });
        let command_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("physbox5_body_command_pipeline"),
            layout: Some(&command_pipeline_layout),
            module: &command_shader,
            entry_point: Some("apply_commands"),
            compilation_options: Default::default(),
            cache: None,
        });
        let hold_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("physbox5_body_hold_pipeline"),
            layout: Some(&command_pipeline_layout),
            module: &command_shader,
            entry_point: Some("hold_grabbed"),
            compilation_options: Default::default(),
            cache: None,
        });

        let render_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_render_params"),
            size: size_of::<RenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_render_layout"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
                storage_entry(2, true, wgpu::ShaderStages::VERTEX),
                uniform_entry(3, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_render_bind_group"),
            layout: &render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.properties_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: render_params.as_entire_binding(),
                },
            ],
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_render_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_render.wgsl").into()),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let pipeline = |label, vertex, fragment, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let background_pipeline = pipeline(
            "physbox5_body_background_pipeline",
            "background_vertex",
            "background_fragment",
            None,
        );
        let body_pipeline = pipeline(
            "physbox5_body_render_pipeline",
            "body_vertex",
            "body_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );

        Ok(Self {
            storage,
            integration_bind_group,
            integration_pipeline,
            command_params,
            command_bind_group,
            command_pipeline,
            hold_pipeline,
            render_params,
            render_bind_group,
            background_pipeline,
            body_pipeline,
            floor_solver,
            body_count,
        })
    }

    pub(crate) fn encode_steps(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        steps: u32,
        batch: &BodyCommandBatchV1,
        ranges: &[BodyCommandStepRange],
    ) -> Result<CapacityRecordV1, String> {
        self.encode_steps_with_timestamp(queue, encoder, steps, batch, ranges, None)
    }

    pub(crate) fn encode_steps_with_timestamp(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        steps: u32,
        batch: &BodyCommandBatchV1,
        ranges: &[BodyCommandStepRange],
        beginning_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) -> Result<CapacityRecordV1, String> {
        if beginning_timestamp.is_some() && steps != 1 {
            return Err("timed body encoding requires exactly one step".to_owned());
        }
        if ranges.len() != steps as usize {
            return Err("body command ranges must match the encoded step count".to_owned());
        }
        let capacity = self.storage.upload_commands(queue, batch)?;
        let mut parameters = vec![CommandParamsRaw {
            counts: [0, 0, self.body_count, 0],
        }];
        let mut parameter_indices = vec![None; ranges.len()];
        for (step, range) in ranges.iter().enumerate() {
            if range.count == 0 {
                continue;
            }
            if range
                .start
                .checked_add(range.count)
                .is_none_or(|end| end > u32::try_from(batch.commands.len()).unwrap_or(u32::MAX))
            {
                return Err("body command range exceeds the uploaded batch".to_owned());
            }
            let index = parameters.len();
            parameters.push(CommandParamsRaw {
                counts: [range.start, range.count, self.body_count, 0],
            });
            parameter_indices[step] = Some(index as u32);
        }
        if parameters.len() > (BODY_COMMAND_CAPACITY + 1) as usize {
            return Err("body command parameter capacity overflowed".to_owned());
        }
        let mut bytes = vec![0_u8; parameters.len() * COMMAND_PARAMETER_STRIDE];
        for (index, parameter) in parameters.iter().enumerate() {
            let start = index * COMMAND_PARAMETER_STRIDE;
            bytes[start..start + size_of::<CommandParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(parameter));
        }
        queue.write_buffer(&self.command_params, 0, &bytes);
        let mut beginning_timestamp = beginning_timestamp;
        for parameter_index in parameter_indices {
            if let Some(parameter_index) = parameter_index {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_body_command_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.command_pipeline);
                pass.set_bind_group(
                    0,
                    &self.command_bind_group,
                    &[parameter_index * COMMAND_PARAMETER_STRIDE as u32],
                );
                pass.dispatch_workgroups(1, 1, 1);
            }
            if let Some(solver) = &self.floor_solver {
                if beginning_timestamp.is_some() {
                    return Err(
                        "timed free-flight encoding cannot include a floor solver".to_owned()
                    );
                }
                solver.encode_step(encoder);
            } else {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_body_integration_pass"),
                    timestamp_writes: beginning_timestamp.take().map(|(query_set, beginning)| {
                        wgpu::ComputePassTimestampWrites {
                            query_set,
                            beginning_of_pass_write_index: Some(beginning),
                            end_of_pass_write_index: None,
                        }
                    }),
                });
                pass.set_pipeline(&self.integration_pipeline);
                pass.set_bind_group(0, &self.integration_bind_group, &[]);
                pass.dispatch_workgroups(self.body_count.div_ceil(WORKGROUP_SIZE), 1, 1);
            }
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_body_hold_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hold_pipeline);
            pass.set_bind_group(0, &self.command_bind_group, &[0]);
            pass.dispatch_workgroups(self.body_count.div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        Ok(capacity)
    }

    pub(crate) fn encode_render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: crate::renderer::RenderFrameTarget<'_>,
        frame_index: u64,
    ) {
        self.encode_render_with_timestamp(queue, encoder, target, frame_index, None);
    }

    pub(crate) fn encode_render_with_timestamp(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: crate::renderer::RenderFrameTarget<'_>,
        frame_index: u64,
        ending_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        let params = RenderParamsRaw {
            viewport: [
                target.size[0] as f32,
                target.size[1] as f32,
                target.size[0] as f32 / target.size[1].max(1) as f32,
                frame_index as f32,
            ],
        };
        queue.write_buffer(&self.render_params, 0, bytemuck::bytes_of(&params));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_body_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.008,
                        g: 0.015,
                        b: 0.035,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: ending_timestamp.map(|(query_set, end)| {
                wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(end),
                }
            }),
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.render_bind_group, &[]);
        pass.set_pipeline(&self.background_pipeline);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.body_pipeline);
        pass.draw(0..6, 0..self.body_count);
    }

    pub(crate) const fn body_count(&self) -> u32 {
        self.body_count
    }

    pub(crate) fn body_layout(&self) -> &BodyLayoutV1 {
        self.storage.layout()
    }

    pub(crate) fn benchmark_reset_data(
        &self,
        bodies: &[physbox_contracts::BodyStateV1],
    ) -> Result<GpuBodyResetData, String> {
        GpuBodyResetData::new(self.body_count, bodies)
    }

    pub(crate) fn reset_from_authored(&self, queue: &wgpu::Queue, data: &GpuBodyResetData) {
        self.storage.upload_reset_data(queue, data);
    }

    pub(crate) fn benchmark_capacities(&self, width: u32, height: u32) -> Vec<CapacityRecordV1> {
        let mut records = vec![
            CapacityRecordV1 {
                class: "body-command-parameters".to_owned(),
                capacity_elements: u64::from(BODY_COMMAND_CAPACITY + 1),
                used_elements: 1,
                allocated_bytes: u64::from(BODY_COMMAND_CAPACITY + 1)
                    * COMMAND_PARAMETER_STRIDE as u64,
                high_water_bytes: COMMAND_PARAMETER_STRIDE as u64,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "body-commands".to_owned(),
                capacity_elements: u64::from(BODY_COMMAND_CAPACITY),
                used_elements: 0,
                allocated_bytes: u64::from(BODY_COMMAND_CAPACITY) * u64::from(BODY_COMMAND_BYTES),
                high_water_bytes: 0,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "body-integration-parameters".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: size_of::<IntegrationParamsRaw>() as u64,
                high_water_bytes: size_of::<IntegrationParamsRaw>() as u64,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "body-render-parameters".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: size_of::<RenderParamsRaw>() as u64,
                high_water_bytes: size_of::<RenderParamsRaw>() as u64,
                overflowed: false,
            },
            self.storage.body_capacity_record(),
            CapacityRecordV1 {
                class: "render-target-rgba8-logical".to_owned(),
                capacity_elements: u64::from(width) * u64::from(height),
                used_elements: u64::from(width) * u64::from(height),
                allocated_bytes: u64::from(width) * u64::from(height) * 4,
                high_water_bytes: u64::from(width) * u64::from(height) * 4,
                overflowed: false,
            },
        ];
        records.sort_by(|left, right| left.class.cmp(&right.class));
        records
    }

    pub(crate) fn copy_full_validation_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        let lane_bytes = u64::from(self.body_count) * size_of::<[u32; 4]>() as u64;
        encoder.copy_buffer_to_buffer(self.storage.pose_buffer(), 0, destination, 0, lane_bytes);
        encoder.copy_buffer_to_buffer(
            self.storage.motion_buffer(),
            0,
            destination,
            lane_bytes,
            lane_bytes,
        );
        encoder.copy_buffer_to_buffer(
            self.storage.metadata_buffer(),
            0,
            destination,
            lane_bytes * 2,
            lane_bytes,
        );
    }

    pub(crate) fn solver_dispatches_per_step(&self) -> u32 {
        self.floor_solver
            .as_ref()
            .map_or(1, GpuFixedFloorSolver::dispatches_per_step)
    }

    pub(crate) fn contact_point_count(&self) -> u32 {
        self.floor_solver
            .as_ref()
            .map_or(0, GpuFixedFloorSolver::point_count)
    }

    pub(crate) fn solver_capacities(&self) -> &[CapacityRecordV1] {
        self.floor_solver
            .as_ref()
            .map_or(&[], GpuFixedFloorSolver::capacities)
    }

    pub(crate) fn solver_capacity_bytes(&self) -> u64 {
        self.solver_capacities()
            .iter()
            .map(|record| record.allocated_bytes)
            .sum()
    }

    pub(crate) fn solver_capacity_overflowed(&self) -> bool {
        self.solver_capacities()
            .iter()
            .any(|record| record.overflowed)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn floor_pass_evidence_layout(&self) -> Result<FixedFloorEvidenceLayout, String> {
        self.floor_solver
            .as_ref()
            .map(GpuFixedFloorSolver::pass_evidence_layout)
            .ok_or_else(|| "pass evidence requires the fixed-floor solver".to_owned())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_floor_pass_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<FixedFloorEvidenceLayout, String> {
        self.floor_solver
            .as_ref()
            .map(|solver| solver.copy_pass_evidence(encoder, destination))
            .ok_or_else(|| "pass evidence requires the fixed-floor solver".to_owned())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_capture_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            self.storage.pose_buffer(),
            0,
            destination,
            0,
            size_of::<[f32; 4]>() as u64,
        );
        encoder.copy_buffer_to_buffer(
            self.storage.motion_buffer(),
            0,
            destination,
            size_of::<[f32; 4]>() as u64,
            size_of::<[f32; 4]>() as u64,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_capture_identity(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            self.storage.metadata_buffer(),
            0,
            destination,
            0,
            size_of::<[u32; 4]>() as u64,
        );
    }
}

fn storage_entry(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
