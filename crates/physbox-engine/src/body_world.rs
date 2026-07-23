#![cfg_attr(not(feature = "solver-lab"), allow(dead_code))]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyCommandBatchV1, BodyLayoutV1, CapacityRecordV1, ConstructionIntentBatchV1,
    ContactIdentityPolicyV1, GrabIntentBatchV1, ScenarioV1, Validate,
};
use physbox_launch::WorldCapacityPreset;
use wgpu::util::DeviceExt;

use crate::awake_work::SleepExecutionConfig;
#[cfg(not(target_arch = "wasm32"))]
use crate::body_floor_solver::FixedFloorEvidenceLayout;
use crate::body_state::{
    ACTIVE_BODY_DISPATCH_64_OFFSET, ACTIVE_BODY_DRAW_OFFSET, BODY_COMMAND_BYTES,
    BODY_COMMAND_CAPACITY, GpuBodyResetData,
};
use crate::construction::{
    CONSTRUCTION_CIRCLE_RADIUS, CONSTRUCTION_FRICTION, CONSTRUCTION_INTENT_BYTES,
    CONSTRUCTION_INTENT_CAPACITY, CONSTRUCTION_RECTANGLE_HALF_EXTENTS, CONSTRUCTION_RESTITUTION,
    CONSTRUCTION_STATE_BYTES, ConstructionIntentRaw, construction_intent_raw,
    empty_construction_intent_batch, initial_construction_state,
};
use crate::grab::{
    GRAB_DAMPING_RATE, GRAB_INTENT_BYTES, GRAB_INTENT_CAPACITY, GRAB_MAX_ACCELERATION,
    GRAB_MAX_ANGULAR_SPEED, GRAB_MAX_LINEAR_SPEED, GRAB_SPRING_ACCELERATION, GRAB_STATE_BYTES,
    GrabParamsRaw, GrabStateRaw, NO_GRAB_SLOT, grab_intent_raw,
};
use crate::{
    GpuBodyStorage, GpuGatherTopology, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1,
    body_floor_solver::GpuFixedFloorSolver, generated_contacts::GpuGeneratedContactGraph,
    particle_world::GpuParticleWorld,
};

const COMMAND_PARAMETER_STRIDE: usize = 256;
const CONSTRUCTION_PARAMETER_STRIDE: usize = 256;
const GRAB_RENDER_VERTEX_COUNT: u32 = 12;
const GRAB_RENDER_VERTEX_BYTES: u32 = GRAB_RENDER_VERTEX_COUNT * 16;

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
    camera: [f32; 4],
    sleep_visualization: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CommandParamsRaw {
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConstructionParamsRaw {
    counts: [u32; 4],
    circle_material: [f32; 4],
    rectangle_mass: [f32; 4],
    material_padding: [f32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BodyCommandStepRange {
    pub(crate) start: u32,
    pub(crate) count: u32,
}

pub(crate) struct BodyStepPlan<'a> {
    pub(crate) batch: &'a BodyCommandBatchV1,
    pub(crate) ranges: &'a [BodyCommandStepRange],
    pub(crate) grab_batch: &'a GrabIntentBatchV1,
    pub(crate) grab_ranges: &'a [BodyCommandStepRange],
    pub(crate) construction_batch: &'a ConstructionIntentBatchV1,
    pub(crate) construction_ranges: &'a [BodyCommandStepRange],
}

pub(crate) struct BodyRenderPlan<'a> {
    pub(crate) frame_index: u64,
    pub(crate) camera: crate::CameraFrame2d,
    pub(crate) grab_pointer_active: bool,
    pub(crate) sleep_visuals_enabled: bool,
    pub(crate) current_outer_step: u32,
    pub(crate) quiet_outer_steps: u32,
    pub(crate) wake_flash_outer_steps: u32,
    pub(crate) ending_timestamp: Option<(&'a wgpu::QuerySet, u32)>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuConstructionBodySlotEvidence {
    pub slot: u32,
    pub pose: [f32; 4],
    pub motion: [f32; 4],
    pub properties: [f32; 4],
    pub metadata: [u32; 4],
}

pub(crate) struct GpuBodyWorld {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    storage: GpuBodyStorage,
    integration_bind_group: wgpu::BindGroup,
    integration_pipeline: wgpu::ComputePipeline,
    command_params: wgpu::Buffer,
    command_bind_group: wgpu::BindGroup,
    command_grab_pipeline: wgpu::ComputePipeline,
    grab_constraint_pipeline: wgpu::ComputePipeline,
    grab_intents: wgpu::Buffer,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    grab_state: wgpu::Buffer,
    construction_params: wgpu::Buffer,
    construction_intents: wgpu::Buffer,
    construction_state: wgpu::Buffer,
    construction_bind_group: wgpu::BindGroup,
    construction_pipeline: wgpu::ComputePipeline,
    grab_render_vertices: wgpu::Buffer,
    grab_render_bind_group: wgpu::BindGroup,
    grab_render_prepare_pipeline: wgpu::ComputePipeline,
    render_params: wgpu::Buffer,
    render_bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    grab_pipeline: wgpu::RenderPipeline,
    floor_solver: Option<GpuFixedFloorSolver>,
    generated_contacts: Option<GpuGeneratedContactGraph>,
    particles: Option<GpuParticleWorld>,
    generated_solver_configuration: Option<(GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1)>,
    generated_contact_coefficient_policy: Option<crate::GpuContactCoefficientPolicyV1>,
    body_count: u32,
    initial_live_count: u32,
    sleeping_enabled: bool,
}

struct GeneratedContactBuildOptions {
    solver_configuration: Option<(GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1)>,
    identity_policy: ContactIdentityPolicyV1,
    use_contact_policy_comparator: bool,
    branch_robust_tunable: bool,
    sleep_config: SleepExecutionConfig,
}

impl GeneratedContactBuildOptions {
    fn disabled(fixed_step_hz: u32) -> Self {
        Self {
            solver_configuration: None,
            identity_policy: ContactIdentityPolicyV1::SELECTED,
            use_contact_policy_comparator: false,
            branch_robust_tunable: false,
            sleep_config: SleepExecutionConfig::disabled(fixed_step_hz),
        }
    }

    fn selected(scheduling_policy: GpuSolverSchedulingPolicyV1, fixed_step_hz: u32) -> Self {
        Self {
            solver_configuration: Some((GpuSolverExecutionV4::SELECTED, scheduling_policy)),
            identity_policy: ContactIdentityPolicyV1::SELECTED,
            use_contact_policy_comparator: false,
            branch_robust_tunable: false,
            sleep_config: SleepExecutionConfig::disabled(fixed_step_hz),
        }
    }
}

impl GpuBodyWorld {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        let fixed_step_hz = scenario_fixed_step_hz(scenario)?;
        Self::new_with_generated_contacts(
            device,
            color_format,
            scenario,
            WorldCapacityPreset::default(),
            GeneratedContactBuildOptions::disabled(fixed_step_hz),
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_generated(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        world_capacity: WorldCapacityPreset,
    ) -> Result<Self, String> {
        let fixed_step_hz = scenario_fixed_step_hz(scenario)?;
        let scheduling_policy = selected_interactive_scheduling(scenario, world_capacity)?;
        Self::new_with_generated_contacts(
            device,
            color_format,
            scenario,
            world_capacity,
            GeneratedContactBuildOptions::selected(scheduling_policy, fixed_step_hz),
            None,
        )
    }

    pub(crate) fn new_generated_with_scheduling(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        world_capacity: WorldCapacityPreset,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<Self, String> {
        let fixed_step_hz = scenario_fixed_step_hz(scenario)?;
        Self::new_with_generated_contacts(
            device,
            color_format,
            scenario,
            world_capacity,
            GeneratedContactBuildOptions::selected(scheduling_policy, fixed_step_hz),
            None,
        )
    }

    pub(crate) fn new_generated_with_contact_policy(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        world_capacity: WorldCapacityPreset,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<Self, String> {
        let fixed_step_hz = scenario_fixed_step_hz(scenario)?;
        let scheduling_policy = selected_interactive_scheduling(scenario, world_capacity)?;
        Self::new_with_generated_contacts(
            device,
            color_format,
            scenario,
            world_capacity,
            GeneratedContactBuildOptions {
                identity_policy,
                use_contact_policy_comparator: true,
                ..GeneratedContactBuildOptions::selected(scheduling_policy, fixed_step_hz)
            },
            None,
        )
    }

    pub(crate) fn new_generated_with_solver_profile(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        world_capacity: WorldCapacityPreset,
        profile: &crate::SolverProfileConfig,
    ) -> Result<Self, String> {
        let physics = profile.physics();
        let sleep_config = SleepExecutionConfig::configured(
            physics.sleeping_enabled,
            physics.fixed_step_hz,
            physics.sleep_speed_threshold,
            physics.sleep_grouping,
        );
        if profile.is_selected() {
            let scheduling_policy = selected_interactive_scheduling(scenario, world_capacity)?;
            return Self::new_with_generated_contacts(
                device,
                color_format,
                scenario,
                world_capacity,
                GeneratedContactBuildOptions {
                    sleep_config,
                    ..GeneratedContactBuildOptions::selected(
                        scheduling_policy,
                        physics.fixed_step_hz,
                    )
                },
                None,
            );
        }
        let initial_body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "GPU body count exceeds u32".to_owned())?;
        let scheduling_policy = GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
            interactive_body_capacity(initial_body_count, world_capacity)?,
        );
        Self::new_with_generated_contacts(
            device,
            color_format,
            scenario,
            world_capacity,
            GeneratedContactBuildOptions {
                solver_configuration: Some((
                    GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
                    scheduling_policy,
                )),
                identity_policy: ContactIdentityPolicyV1::SELECTED,
                use_contact_policy_comparator: false,
                branch_robust_tunable: true,
                sleep_config,
            },
            None,
        )
    }

    pub(crate) fn new_coupled_with_solver_profile(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &physbox_contracts::CoupledScenarioV1,
        world_capacity: WorldCapacityPreset,
        profile: &crate::SolverProfileConfig,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        let physics = profile.physics();
        let sleep_config = SleepExecutionConfig::configured(
            physics.sleeping_enabled,
            physics.fixed_step_hz,
            physics.sleep_speed_threshold,
            physics.sleep_grouping,
        );
        let (execution, scheduling_policy, branch_robust_tunable) = if profile.is_selected() {
            (
                GpuSolverExecutionV4::SELECTED,
                selected_interactive_scheduling(&scenario.rigid, world_capacity)?,
                false,
            )
        } else {
            let initial_body_count = u32::try_from(scenario.rigid.bodies.len())
                .map_err(|_| "GPU body count exceeds u32".to_owned())?;
            (
                GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
                GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                    interactive_body_capacity(initial_body_count, world_capacity)?,
                ),
                true,
            )
        };
        Self::new_with_generated_contacts(
            device,
            color_format,
            &scenario.rigid,
            world_capacity,
            GeneratedContactBuildOptions {
                solver_configuration: Some((execution, scheduling_policy)),
                identity_policy: ContactIdentityPolicyV1::SELECTED,
                use_contact_policy_comparator: false,
                branch_robust_tunable,
                sleep_config,
            },
            Some(scenario),
        )
    }

    fn new_with_generated_contacts(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
        world_capacity: WorldCapacityPreset,
        options: GeneratedContactBuildOptions,
        coupled: Option<&physbox_contracts::CoupledScenarioV1>,
    ) -> Result<Self, String> {
        let GeneratedContactBuildOptions {
            solver_configuration: generated_solver_configuration,
            identity_policy,
            use_contact_policy_comparator,
            branch_robust_tunable,
            sleep_config,
        } = options;
        scenario.validate().map_err(|error| error.to_string())?;
        let initial_live_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "GPU body count exceeds u32".to_owned())?;
        if initial_live_count == 0 {
            return Err("GPU body world requires at least one body".to_owned());
        }
        let body_count = if generated_solver_configuration.is_some() {
            interactive_body_capacity(initial_live_count, world_capacity)?
        } else {
            initial_live_count
        };
        let sleeping_enabled = sleep_config.enabled;
        let storage = GpuBodyStorage::new_with_sleeping(
            device,
            body_count,
            BODY_COMMAND_CAPACITY,
            &scenario.bodies,
            sleep_config,
        )?;
        let floor_solver =
            if generated_solver_configuration.is_some() || scenario.manifolds.is_empty() {
                None
            } else {
                Some(GpuFixedFloorSolver::new(device, &storage, scenario)?)
            };
        let generated_contacts = generated_solver_configuration
            .map(|(execution, scheduling_policy)| {
                if branch_robust_tunable {
                    GpuGeneratedContactGraph::new_branch_robust_tunable(device, &storage, scenario)
                } else if execution == GpuSolverExecutionV4::SELECTED
                    && scheduling_policy
                        == GpuSolverSchedulingPolicyV1::selected_for_body_capacity(body_count)
                    && identity_policy == ContactIdentityPolicyV1::SELECTED
                    && !use_contact_policy_comparator
                {
                    GpuGeneratedContactGraph::new(device, &storage, scenario)
                } else {
                    GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_and_identity_policy(
                        device,
                        &storage,
                        scenario,
                        crate::GpuContactPersistenceTopology::StablePairSlot,
                        GpuGatherTopology::StableBodyEdge,
                        false,
                        false,
                        u32::MAX,
                        execution,
                        scheduling_policy,
                        identity_policy,
                    )
                }
            })
            .transpose()?;
        let particles = coupled
            .map(|coupled| GpuParticleWorld::new(device, color_format, coupled, &storage))
            .transpose()?;
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
                    storage_entry(5, true, wgpu::ShaderStages::COMPUTE),
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: storage.awake_body_buffer().as_entire_binding(),
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
            size: u64::from(BODY_COMMAND_CAPACITY + GRAB_INTENT_CAPACITY + 1)
                * COMMAND_PARAMETER_STRIDE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grab_intents = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_grab_intents"),
            size: u64::from(GRAB_INTENT_CAPACITY) * u64::from(GRAB_INTENT_BYTES),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let initial_grab_state = GrabStateRaw {
            identity_status: [NO_GRAB_SLOT, 0, 0, 0],
            local_anchor_target: [0.0; 4],
            counters: [0; 4],
            activity: [0; 4],
            last_selection: [0, NO_GRAB_SLOT, 0, 0],
        };
        let grab_state = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_grab_state"),
            contents: bytemuck::bytes_of(&initial_grab_state),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let grab_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_grab_params"),
            contents: bytemuck::bytes_of(&GrabParamsRaw {
                dt_tuning: [
                    scenario.settings.fixed_dt.get(),
                    GRAB_SPRING_ACCELERATION,
                    GRAB_DAMPING_RATE,
                    GRAB_MAX_ACCELERATION,
                ],
                limits_padding: [GRAB_MAX_LINEAR_SPEED, GRAB_MAX_ANGULAR_SPEED, 0.0, 0.0],
                counts: [body_count, 0, 0, 0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
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
                storage_entry(5, true, wgpu::ShaderStages::COMPUTE),
                storage_entry(6, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(7, true, wgpu::ShaderStages::COMPUTE),
                uniform_entry(8, wgpu::ShaderStages::COMPUTE),
                storage_entry(9, false, wgpu::ShaderStages::COMPUTE),
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: storage.properties_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: grab_state.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: grab_intents.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: grab_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: storage.wake_request_buffer().as_entire_binding(),
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
        let command_grab_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_body_command_grab_pipeline"),
                layout: Some(&command_pipeline_layout),
                module: &command_shader,
                entry_point: Some("apply_step_commands_and_grab"),
                compilation_options: Default::default(),
                cache: None,
            });
        let grab_constraint_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_grab_constraint_pipeline"),
                layout: Some(&command_pipeline_layout),
                module: &command_shader,
                entry_point: Some("steer_grab"),
                compilation_options: Default::default(),
                cache: None,
            });

        let construction_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_construction_params"),
            size: u64::from(CONSTRUCTION_INTENT_CAPACITY + 1)
                * CONSTRUCTION_PARAMETER_STRIDE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let construction_intents = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_construction_intents"),
            size: u64::from(CONSTRUCTION_INTENT_CAPACITY) * u64::from(CONSTRUCTION_INTENT_BYTES),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let construction_state = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_construction_state"),
            contents: bytemuck::bytes_of(&initial_construction_state(
                body_count,
                initial_live_count,
            )),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let construction_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_construction_layout"),
                entries: &[
                    storage_entry(0, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(1, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(2, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(3, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(4, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(5, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(6, false, wgpu::ShaderStages::COMPUTE),
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(
                                size_of::<ConstructionParamsRaw>() as u64,
                            ),
                        },
                        count: None,
                    },
                    storage_entry(8, false, wgpu::ShaderStages::COMPUTE),
                ],
            });
        let construction_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_construction_bind_group"),
            layout: &construction_layout,
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
                    resource: construction_intents.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: construction_state.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: grab_state.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &construction_params,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<ConstructionParamsRaw>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: storage.active_body_buffer().as_entire_binding(),
                },
            ],
        });
        let construction_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_construction_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_construction.wgsl").into()),
        });
        let construction_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_construction_pipeline_layout"),
                bind_group_layouts: &[&construction_layout],
                push_constant_ranges: &[],
            });
        let construction_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_construction_pipeline"),
                layout: Some(&construction_pipeline_layout),
                module: &construction_shader,
                entry_point: Some("apply_construction_intents"),
                compilation_options: Default::default(),
                cache: None,
            });

        let render_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_render_params"),
            size: size_of::<RenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grab_render_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_grab_render_vertices"),
            size: u64::from(GRAB_RENDER_VERTEX_BYTES),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let grab_render_prepare_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_grab_render_prepare_layout"),
                entries: &[
                    storage_entry(0, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(1, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(2, true, wgpu::ShaderStages::COMPUTE),
                    uniform_entry(3, wgpu::ShaderStages::COMPUTE),
                    storage_entry(4, false, wgpu::ShaderStages::COMPUTE),
                ],
            });
        let grab_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_grab_render_prepare_bind_group"),
            layout: &grab_render_prepare_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: grab_state.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: render_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: grab_render_vertices.as_entire_binding(),
                },
            ],
        });
        let grab_render_prepare_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("physbox5_grab_render_prepare_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("grab_render_prepare.wgsl").into()),
            });
        let grab_render_prepare_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_grab_render_prepare_pipeline_layout"),
                bind_group_layouts: &[&grab_render_prepare_layout],
                push_constant_ranges: &[],
            });
        let grab_render_prepare_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_grab_render_prepare_pipeline"),
                layout: Some(&grab_render_prepare_pipeline_layout),
                module: &grab_render_prepare_shader,
                entry_point: Some("prepare_grab_vertices"),
                compilation_options: Default::default(),
                cache: None,
            });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_render_layout"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
                storage_entry(2, true, wgpu::ShaderStages::VERTEX),
                uniform_entry(3, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                storage_entry(4, true, wgpu::ShaderStages::VERTEX),
                storage_entry(5, true, wgpu::ShaderStages::VERTEX),
                storage_entry(6, true, wgpu::ShaderStages::VERTEX),
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: storage.active_body_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: storage.sleep_state_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: grab_state.as_entire_binding(),
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
        let pipeline = |label, vertex, fragment, blend, buffers| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some(vertex),
                    buffers,
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
            &[],
        );
        let body_pipeline = pipeline(
            "physbox5_body_render_pipeline",
            "body_vertex",
            "body_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            &[],
        );
        let grab_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            }],
        };
        let grab_pipeline = pipeline(
            "physbox5_grab_render_pipeline",
            "grab_vertex",
            "grab_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
            &[grab_vertex_layout],
        );

        Ok(Self {
            storage,
            integration_bind_group,
            integration_pipeline,
            command_params,
            command_bind_group,
            command_grab_pipeline,
            grab_constraint_pipeline,
            grab_intents,
            grab_state,
            construction_params,
            construction_intents,
            construction_state,
            construction_bind_group,
            construction_pipeline,
            grab_render_vertices,
            grab_render_bind_group,
            grab_render_prepare_pipeline,
            render_params,
            render_bind_group,
            background_pipeline,
            body_pipeline,
            grab_pipeline,
            floor_solver,
            generated_contacts,
            particles,
            generated_solver_configuration,
            generated_contact_coefficient_policy: generated_solver_configuration.map(|_| {
                if branch_robust_tunable {
                    crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft
                } else {
                    crate::GpuContactCoefficientPolicyV1::SELECTED
                }
            }),
            body_count,
            initial_live_count,
            sleeping_enabled,
        })
    }

    pub(crate) fn encode_steps_with_grab_intents(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        steps: u32,
        plan: BodyStepPlan<'_>,
    ) -> Result<CapacityRecordV1, String> {
        self.encode_steps_internal(
            queue,
            encoder,
            steps,
            plan.batch,
            plan.ranges,
            plan.grab_batch,
            plan.grab_ranges,
            plan.construction_batch,
            plan.construction_ranges,
            None,
        )
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
        let grab_batch = crate::grab::empty_grab_intent_batch();
        let grab_ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        let construction_batch = empty_construction_intent_batch();
        let construction_ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        self.encode_steps_internal(
            queue,
            encoder,
            steps,
            batch,
            ranges,
            &grab_batch,
            &grab_ranges,
            &construction_batch,
            &construction_ranges,
            beginning_timestamp,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_steps_internal(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        steps: u32,
        batch: &BodyCommandBatchV1,
        ranges: &[BodyCommandStepRange],
        grab_batch: &GrabIntentBatchV1,
        grab_ranges: &[BodyCommandStepRange],
        construction_batch: &ConstructionIntentBatchV1,
        construction_ranges: &[BodyCommandStepRange],
        beginning_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) -> Result<CapacityRecordV1, String> {
        if beginning_timestamp.is_some() && steps != 1 {
            return Err("timed body encoding requires exactly one step".to_owned());
        }
        if ranges.len() != steps as usize
            || grab_ranges.len() != steps as usize
            || construction_ranges.len() != steps as usize
        {
            return Err(
                "body, grab, and construction ranges must match the encoded step count".to_owned(),
            );
        }
        let capacity = self.storage.upload_commands(queue, batch)?;
        grab_batch.validate().map_err(|error| error.to_string())?;
        if grab_batch.capacity_intents != GRAB_INTENT_CAPACITY {
            return Err(format!(
                "grab intent batch capacity {} does not match GPU capacity {GRAB_INTENT_CAPACITY}",
                grab_batch.capacity_intents
            ));
        }
        let raw_intents = grab_batch
            .intents
            .iter()
            .copied()
            .map(grab_intent_raw)
            .collect::<Vec<_>>();
        if !raw_intents.is_empty() {
            queue.write_buffer(&self.grab_intents, 0, bytemuck::cast_slice(&raw_intents));
        }
        construction_batch
            .validate()
            .map_err(|error| error.to_string())?;
        if construction_batch.capacity_intents != CONSTRUCTION_INTENT_CAPACITY {
            return Err(format!(
                "construction intent batch capacity {} does not match GPU capacity {CONSTRUCTION_INTENT_CAPACITY}",
                construction_batch.capacity_intents
            ));
        }
        let raw_construction = construction_batch
            .intents
            .iter()
            .copied()
            .map(construction_intent_raw)
            .collect::<Vec<ConstructionIntentRaw>>();
        if !raw_construction.is_empty() {
            queue.write_buffer(
                &self.construction_intents,
                0,
                bytemuck::cast_slice(&raw_construction),
            );
        }
        let mut parameters = vec![CommandParamsRaw { counts: [0; 4] }];
        let mut step_parameter_indices = vec![None; ranges.len()];
        for (step, (command_range, grab_range)) in ranges.iter().zip(grab_ranges).enumerate() {
            if command_range
                .start
                .checked_add(command_range.count)
                .is_none_or(|end| end > u32::try_from(batch.commands.len()).unwrap_or(u32::MAX))
            {
                return Err("body command range exceeds the uploaded batch".to_owned());
            }
            if grab_range
                .start
                .checked_add(grab_range.count)
                .is_none_or(|end| end > u32::try_from(grab_batch.intents.len()).unwrap_or(u32::MAX))
            {
                return Err("grab intent range exceeds the uploaded batch".to_owned());
            }
            if command_range.count == 0 && grab_range.count == 0 {
                continue;
            }
            let index = parameters.len();
            parameters.push(CommandParamsRaw {
                counts: [
                    command_range.start,
                    command_range.count,
                    grab_range.start,
                    grab_range.count,
                ],
            });
            step_parameter_indices[step] = Some(index as u32);
        }
        if parameters.len() > (BODY_COMMAND_CAPACITY + GRAB_INTENT_CAPACITY + 1) as usize {
            return Err("body command parameter capacity overflowed".to_owned());
        }
        let mut bytes = vec![0_u8; parameters.len() * COMMAND_PARAMETER_STRIDE];
        for (index, parameter) in parameters.iter().enumerate() {
            let start = index * COMMAND_PARAMETER_STRIDE;
            bytes[start..start + size_of::<CommandParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(parameter));
        }
        queue.write_buffer(&self.command_params, 0, &bytes);
        let rectangle_inverse_inertia = 3.0
            / (CONSTRUCTION_RECTANGLE_HALF_EXTENTS[0] * CONSTRUCTION_RECTANGLE_HALF_EXTENTS[0]
                + CONSTRUCTION_RECTANGLE_HALF_EXTENTS[1] * CONSTRUCTION_RECTANGLE_HALF_EXTENTS[1]);
        let mut construction_parameters = vec![ConstructionParamsRaw {
            counts: [0, 0, self.body_count, 0],
            circle_material: [
                CONSTRUCTION_CIRCLE_RADIUS,
                1.0,
                2.0 / (CONSTRUCTION_CIRCLE_RADIUS * CONSTRUCTION_CIRCLE_RADIUS),
                CONSTRUCTION_FRICTION,
            ],
            rectangle_mass: [
                CONSTRUCTION_RECTANGLE_HALF_EXTENTS[0],
                CONSTRUCTION_RECTANGLE_HALF_EXTENTS[1],
                1.0,
                rectangle_inverse_inertia,
            ],
            material_padding: [CONSTRUCTION_RESTITUTION, 0.0, 0.0, 0.0],
        }];
        let mut construction_parameter_indices = vec![None; construction_ranges.len()];
        for (step, range) in construction_ranges.iter().enumerate() {
            if range.start.checked_add(range.count).is_none_or(|end| {
                end > u32::try_from(construction_batch.intents.len()).unwrap_or(u32::MAX)
            }) {
                return Err("construction intent range exceeds the uploaded batch".to_owned());
            }
            if range.count == 0 {
                continue;
            }
            let index = construction_parameters.len();
            let mut parameter = construction_parameters[0];
            parameter.counts[0] = range.start;
            parameter.counts[1] = range.count;
            construction_parameters.push(parameter);
            construction_parameter_indices[step] = Some(index as u32);
        }
        if construction_parameters.len() > (CONSTRUCTION_INTENT_CAPACITY + 1) as usize {
            return Err("construction parameter capacity overflowed".to_owned());
        }
        let mut construction_bytes =
            vec![0_u8; construction_parameters.len() * CONSTRUCTION_PARAMETER_STRIDE];
        for (index, parameter) in construction_parameters.iter().enumerate() {
            let start = index * CONSTRUCTION_PARAMETER_STRIDE;
            construction_bytes[start..start + size_of::<ConstructionParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(parameter));
        }
        queue.write_buffer(&self.construction_params, 0, &construction_bytes);
        let mut beginning_timestamp = beginning_timestamp;
        for step_index in 0..steps as usize {
            if let Some(parameter_index) = construction_parameter_indices[step_index] {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_construction_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.construction_pipeline);
                pass.set_bind_group(
                    0,
                    &self.construction_bind_group,
                    &[parameter_index * CONSTRUCTION_PARAMETER_STRIDE as u32],
                );
                pass.dispatch_workgroups(1, 1, 1);
            }
            if let Some(parameter_index) = step_parameter_indices[step_index] {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_body_command_grab_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.command_grab_pipeline);
                pass.set_bind_group(
                    0,
                    &self.command_bind_group,
                    &[parameter_index * COMMAND_PARAMETER_STRIDE as u32],
                );
                pass.dispatch_workgroups(1, 1, 1);
            } else {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_grab_constraint_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.grab_constraint_pipeline);
                pass.set_bind_group(0, &self.command_bind_group, &[0]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            if let Some(generated) = &self.generated_contacts {
                if beginning_timestamp.is_some() {
                    return Err(
                        "timed free-flight encoding cannot include generated contacts".to_owned(),
                    );
                }
                let range = ranges[step_index];
                let invalidate_cache = batch.commands
                    [range.start as usize..(range.start + range.count) as usize]
                    .iter()
                    .any(|command| {
                        matches!(command, physbox_contracts::BodyCommandV1::Reset { .. })
                    });
                generated.encode_step(encoder, invalidate_cache);
            } else if let Some(solver) = &self.floor_solver {
                if beginning_timestamp.is_some() {
                    return Err(
                        "timed free-flight encoding cannot include a floor solver".to_owned()
                    );
                }
                self.storage.encode_awake_compaction(encoder);
                solver.encode_step(encoder);
                self.storage.encode_sleep_admission(encoder);
            } else {
                self.storage.encode_awake_compaction(encoder);
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
                pass.dispatch_workgroups_indirect(
                    self.storage.awake_body_buffer(),
                    ACTIVE_BODY_DISPATCH_64_OFFSET,
                );
                drop(pass);
                self.storage.encode_sleep_admission(encoder);
            }
            if let Some(particles) = &self.particles {
                particles.encode_step(encoder);
            }
        }
        Ok(capacity)
    }

    pub(crate) fn encode_render_with_timestamp(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: crate::renderer::RenderFrameTarget<'_>,
        plan: BodyRenderPlan<'_>,
    ) {
        let params = RenderParamsRaw {
            viewport: [
                target.size[0] as f32,
                target.size[1] as f32,
                target.size[0] as f32 / target.size[1].max(1) as f32,
                plan.frame_index as f32,
            ],
            camera: [
                plan.camera.center()[0],
                plan.camera.center()[1],
                plan.camera.half_extents()[0],
                plan.camera.half_extents()[1],
            ],
            sleep_visualization: [
                u32::from(plan.sleep_visuals_enabled),
                plan.current_outer_step,
                plan.quiet_outer_steps,
                plan.wake_flash_outer_steps,
            ],
        };
        queue.write_buffer(&self.render_params, 0, bytemuck::bytes_of(&params));
        if plan.grab_pointer_active {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_grab_render_prepare_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.grab_render_prepare_pipeline);
            pass.set_bind_group(0, &self.grab_render_bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
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
            timestamp_writes: plan.ending_timestamp.map(|(query_set, end)| {
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
        pass.draw_indirect(self.storage.active_body_buffer(), ACTIVE_BODY_DRAW_OFFSET);
        if plan.grab_pointer_active {
            pass.set_pipeline(&self.grab_pipeline);
            pass.set_vertex_buffer(0, self.grab_render_vertices.slice(..));
            pass.draw(0..GRAB_RENDER_VERTEX_COUNT, 0..1);
        }
        drop(pass);
        if let Some(particles) = &self.particles {
            particles.encode_render(queue, encoder, target, plan.camera, plan.frame_index, false);
        }
    }

    pub(crate) const fn initial_body_count(&self) -> u32 {
        self.initial_live_count
    }

    pub(crate) fn particle_capacity(&self) -> u32 {
        self.particles
            .as_ref()
            .map_or(0, GpuParticleWorld::capacity)
    }

    pub(crate) fn initial_particle_count(&self) -> u32 {
        self.particles
            .as_ref()
            .map_or(0, GpuParticleWorld::initial_count)
    }

    pub(crate) fn particle_allocated_bytes(&self) -> u64 {
        self.particles
            .as_ref()
            .map_or(0, GpuParticleWorld::allocated_bytes)
    }

    pub(crate) fn particle_dispatches_per_step(&self) -> u32 {
        self.particles
            .as_ref()
            .map_or(0, GpuParticleWorld::dispatches_per_step)
    }

    pub(crate) fn particle_validation_layout(
        &self,
    ) -> Option<crate::particle_world::ParticleValidationLayout> {
        self.particles
            .as_ref()
            .map(GpuParticleWorld::validation_layout)
    }

    pub(crate) fn copy_particle_validation_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        let particles = self
            .particles
            .as_ref()
            .ok_or_else(|| "active GPU body world has no particle state".to_owned())?;
        particles.copy_validation_state(encoder, destination);
        Ok(())
    }

    pub(crate) fn decode_particle_validation_state(
        &self,
        bytes: &[u8],
    ) -> Result<crate::GpuParticleDiagnosticsV2, String> {
        self.particles
            .as_ref()
            .ok_or_else(|| "active GPU body world has no particle state".to_owned())?
            .decode_validation_state(bytes)
    }

    pub(crate) fn copy_grab_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.grab_state,
            0,
            destination,
            0,
            u64::from(GRAB_STATE_BYTES),
        );
    }

    pub(crate) fn copy_construction_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.construction_state,
            0,
            destination,
            0,
            u64::from(CONSTRUCTION_STATE_BYTES),
        );
    }

    #[cfg(test)]
    pub(crate) fn overwrite_body_generation_for_test(
        &self,
        queue: &wgpu::Queue,
        slot: u32,
        generation: u32,
    ) {
        assert!(slot < self.body_count);
        assert_ne!(generation, 0);
        queue.write_buffer(
            self.storage.metadata_buffer(),
            u64::from(slot) * size_of::<[u32; 4]>() as u64,
            bytemuck::bytes_of(&generation),
        );
    }

    #[cfg(test)]
    pub(crate) fn overwrite_body_pose_for_test(
        &self,
        queue: &wgpu::Queue,
        slot: u32,
        pose: [f32; 4],
    ) {
        assert!(slot < self.body_count);
        queue.write_buffer(
            self.storage.pose_buffer(),
            u64::from(slot) * size_of::<[f32; 4]>() as u64,
            bytemuck::bytes_of(&pose),
        );
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
        if let Some(generated) = &self.generated_contacts {
            generated.reset_sleep_cycle_metrics(queue);
        }
    }

    pub(crate) fn copy_sleep_cycle_metrics(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        self.generated_contacts
            .as_ref()
            .ok_or_else(|| "sleep-cycle diagnostics require generated contacts".to_owned())?
            .copy_sleep_cycle_metrics(encoder, destination)
    }

    pub(crate) fn benchmark_capacities(&self, width: u32, height: u32) -> Vec<CapacityRecordV1> {
        let mut records = vec![
            CapacityRecordV1 {
                class: "body-command-parameters".to_owned(),
                capacity_elements: u64::from(BODY_COMMAND_CAPACITY + GRAB_INTENT_CAPACITY + 1),
                used_elements: 1,
                allocated_bytes: u64::from(BODY_COMMAND_CAPACITY + GRAB_INTENT_CAPACITY + 1)
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
                class: "grab-intents".to_owned(),
                capacity_elements: u64::from(GRAB_INTENT_CAPACITY),
                used_elements: 0,
                allocated_bytes: u64::from(GRAB_INTENT_CAPACITY) * u64::from(GRAB_INTENT_BYTES),
                high_water_bytes: 0,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "grab-state".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: u64::from(GRAB_STATE_BYTES),
                high_water_bytes: u64::from(GRAB_STATE_BYTES),
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "construction-intents".to_owned(),
                capacity_elements: u64::from(CONSTRUCTION_INTENT_CAPACITY),
                used_elements: 0,
                allocated_bytes: u64::from(CONSTRUCTION_INTENT_CAPACITY)
                    * u64::from(CONSTRUCTION_INTENT_BYTES),
                high_water_bytes: 0,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "construction-state".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: u64::from(CONSTRUCTION_STATE_BYTES),
                high_water_bytes: u64::from(CONSTRUCTION_STATE_BYTES),
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "construction-parameters".to_owned(),
                capacity_elements: u64::from(CONSTRUCTION_INTENT_CAPACITY + 1),
                used_elements: 1,
                allocated_bytes: u64::from(CONSTRUCTION_INTENT_CAPACITY + 1)
                    * CONSTRUCTION_PARAMETER_STRIDE as u64,
                high_water_bytes: CONSTRUCTION_PARAMETER_STRIDE as u64,
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "grab-render-vertices".to_owned(),
                capacity_elements: u64::from(GRAB_RENDER_VERTEX_COUNT),
                used_elements: u64::from(GRAB_RENDER_VERTEX_COUNT),
                allocated_bytes: u64::from(GRAB_RENDER_VERTEX_BYTES),
                high_water_bytes: u64::from(GRAB_RENDER_VERTEX_BYTES),
                overflowed: false,
            },
            CapacityRecordV1 {
                class: "grab-parameters".to_owned(),
                capacity_elements: 1,
                used_elements: 1,
                allocated_bytes: u64::from(crate::grab::GRAB_PARAMETER_BYTES),
                high_water_bytes: u64::from(crate::grab::GRAB_PARAMETER_BYTES),
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
            CapacityRecordV1 {
                class: "active-body-data".to_owned(),
                capacity_elements: u64::from(self.body_count),
                used_elements: u64::from(self.initial_live_count),
                allocated_bytes: u64::from(
                    self.body_count + crate::body_state::ACTIVE_BODY_STATE_WORDS,
                ) * size_of::<u32>() as u64,
                high_water_bytes: u64::from(
                    self.initial_live_count + crate::body_state::ACTIVE_BODY_STATE_WORDS,
                ) * size_of::<u32>() as u64,
                overflowed: false,
            },
            self.storage.awake_body_capacity_record(),
            self.storage.sleep_wake_capacity_record(),
            self.storage.body_capacity_record(),
            self.storage.sleep_state_capacity_record(),
            CapacityRecordV1 {
                class: "render-target-rgba8-logical".to_owned(),
                capacity_elements: u64::from(width) * u64::from(height),
                used_elements: u64::from(width) * u64::from(height),
                allocated_bytes: u64::from(width) * u64::from(height) * 4,
                high_water_bytes: u64::from(width) * u64::from(height) * 4,
                overflowed: false,
            },
        ];
        if let Some(generated) = &self.generated_contacts {
            records.push(generated.awake_point_capacity_record());
            records.push(generated.solver_body_capacity_record());
        }
        if let Some(particles) = &self.particles {
            records.extend(particles.capacities());
        }
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

    #[cfg(test)]
    fn copy_active_body_data(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            self.storage.active_body_buffer(),
            0,
            destination,
            0,
            u64::from(self.body_count + crate::body_state::ACTIVE_BODY_STATE_WORDS)
                * size_of::<u32>() as u64,
        );
    }

    #[cfg(test)]
    fn copy_awake_point_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        self.generated_contacts
            .as_ref()
            .expect("awake-point state requires generated contacts")
            .copy_awake_point_state(encoder, destination);
    }

    #[cfg(test)]
    fn copy_awake_work_data(&self, encoder: &mut wgpu::CommandEncoder, destination: &wgpu::Buffer) {
        let sleep_bytes =
            u64::from(self.body_count) * u64::from(crate::awake_work::SLEEP_STATE_BYTES);
        encoder.copy_buffer_to_buffer(
            self.storage.sleep_state_buffer(),
            0,
            destination,
            0,
            sleep_bytes,
        );
        encoder.copy_buffer_to_buffer(
            self.storage.awake_body_buffer(),
            0,
            destination,
            sleep_bytes,
            u64::from(self.body_count + crate::awake_work::AWAKE_BODY_STATE_WORDS)
                * size_of::<u32>() as u64,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_construction_graph_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        self.generated_contacts
            .as_ref()
            .ok_or_else(|| "construction graph evidence requires generated contacts".to_owned())?
            .copy_construction_graph_evidence(encoder, destination);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_construction_body_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        let lane_bytes = u64::from(self.body_count) * size_of::<[u32; 4]>() as u64;
        for (index, source) in [
            self.storage.pose_buffer(),
            self.storage.motion_buffer(),
            self.storage.properties_buffer(),
            self.storage.metadata_buffer(),
        ]
        .into_iter()
        .enumerate()
        {
            encoder.copy_buffer_to_buffer(
                source,
                0,
                destination,
                index as u64 * lane_bytes,
                lane_bytes,
            );
        }
    }

    pub(crate) const fn body_capacity(&self) -> u32 {
        self.body_count
    }

    pub(crate) const fn sleeping_enabled(&self) -> bool {
        self.sleeping_enabled
    }

    pub(crate) fn solver_dispatches_per_step(&self) -> u32 {
        self.generated_contacts.as_ref().map_or_else(
            || {
                3 + self
                    .floor_solver
                    .as_ref()
                    .map_or(1, GpuFixedFloorSolver::dispatches_per_step)
            },
            GpuGeneratedContactGraph::dispatches_per_step,
        )
    }

    pub(crate) const fn generated_solver_configuration(
        &self,
    ) -> Option<(GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1)> {
        self.generated_solver_configuration
    }

    pub(crate) const fn generated_contact_coefficient_policy(
        &self,
    ) -> Option<crate::GpuContactCoefficientPolicyV1> {
        self.generated_contact_coefficient_policy
    }

    pub(crate) fn contact_point_count(&self) -> u32 {
        self.generated_contacts.as_ref().map_or_else(
            || {
                self.floor_solver
                    .as_ref()
                    .map_or(0, GpuFixedFloorSolver::point_count)
            },
            GpuGeneratedContactGraph::point_capacity,
        )
    }

    pub(crate) fn solver_capacities(&self) -> &[CapacityRecordV1] {
        self.floor_solver
            .as_ref()
            .map_or(&[], GpuFixedFloorSolver::capacities)
    }

    pub(crate) fn solver_capacity_bytes(&self) -> u64 {
        self.generated_contacts.as_ref().map_or_else(
            || {
                self.solver_capacities()
                    .iter()
                    .map(|record| record.allocated_bytes)
                    .sum()
            },
            GpuGeneratedContactGraph::allocated_bytes,
        )
    }

    pub(crate) fn solver_capacity_overflowed(&self) -> bool {
        self.generated_contacts.as_ref().map_or_else(
            || {
                self.solver_capacities()
                    .iter()
                    .any(|record| record.overflowed)
            },
            GpuGeneratedContactGraph::capacity_overflowed,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn floor_pass_evidence_layout(&self) -> Result<FixedFloorEvidenceLayout, String> {
        self.generated_contacts.as_ref().map_or_else(
            || {
                self.floor_solver
                    .as_ref()
                    .map(GpuFixedFloorSolver::pass_evidence_layout)
                    .ok_or_else(|| "pass evidence requires a contact solver".to_owned())
            },
            |generated| Ok(generated.pass_evidence_layout()),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_floor_pass_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<FixedFloorEvidenceLayout, String> {
        if let Some(generated) = &self.generated_contacts {
            Ok(generated.copy_pass_evidence(encoder, destination))
        } else {
            self.floor_solver
                .as_ref()
                .map(|solver| solver.copy_pass_evidence(encoder, destination))
                .ok_or_else(|| "pass evidence requires a contact solver".to_owned())
        }
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

fn interactive_body_capacity(
    initial_live_count: u32,
    world_capacity: WorldCapacityPreset,
) -> Result<u32, String> {
    initial_live_count
        .checked_add(1)
        .and_then(u32::checked_next_power_of_two)
        .map(|minimum| minimum.max(world_capacity.slots()))
        .ok_or_else(|| "interactive body capacity overflowed".to_owned())
}

fn scenario_fixed_step_hz(scenario: &ScenarioV1) -> Result<u32, String> {
    let reciprocal = 1.0 / scenario.settings.fixed_dt.get();
    let rounded = reciprocal.round();
    if !rounded.is_finite()
        || rounded < 1.0
        || rounded > u32::MAX as f32
        || (reciprocal - rounded).abs() > 1.0e-3
    {
        return Err("sleep execution requires an integer fixed-step hertz".to_owned());
    }
    Ok(rounded as u32)
}

fn selected_interactive_scheduling(
    scenario: &ScenarioV1,
    world_capacity: WorldCapacityPreset,
) -> Result<GpuSolverSchedulingPolicyV1, String> {
    let initial_live_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "GPU body count exceeds u32".to_owned())?;
    let body_capacity = interactive_body_capacity(initial_live_count, world_capacity)?;
    Ok(GpuSolverSchedulingPolicyV1::selected_for_body_capacity(
        body_capacity,
    ))
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use physbox_contracts::{
        BodyCommandBatchV1, BodyCommandV1, ConstructionIntentV1, GRAB_INTENT_CONTRACT_VERSION,
        GrabIntentBatchV1, GrabIntentV1, SCHEMA_VERSION, Vec2,
    };

    use super::*;
    #[cfg(not(feature = "production-validation"))]
    use crate::scenario::{P5_2SettlingReviewPolicy, p5_2_settling_review_scenario};
    use crate::{
        BoundedConstructionIntents, construction_state_from_bytes,
        grab::{GRAB_INTENT_CAPACITY, GRAB_STATE_BYTES, decode_grab_state},
    };

    fn empty_commands() -> BodyCommandBatchV1 {
        BodyCommandBatchV1 {
            schema_version: SCHEMA_VERSION,
            capacity_commands: BODY_COMMAND_CAPACITY,
            attempted_commands: 0,
            overflowed: false,
            commands: Vec::new(),
        }
    }

    fn grab_batch(intents: Vec<GrabIntentV1>) -> GrabIntentBatchV1 {
        GrabIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: GRAB_INTENT_CONTRACT_VERSION,
            capacity_intents: GRAB_INTENT_CAPACITY,
            attempted_intents: intents.len() as u64,
            overflowed: false,
            intents,
        }
    }

    fn encode(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
        grab: &GrabIntentBatchV1,
        construction: &ConstructionIntentBatchV1,
    ) {
        let commands = empty_commands();
        encode_with_commands(device, queue, world, &commands, grab, construction);
    }

    fn encode_with_commands(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
        commands: &BodyCommandBatchV1,
        grab: &GrabIntentBatchV1,
        construction: &ConstructionIntentBatchV1,
    ) {
        let command_ranges = [BodyCommandStepRange {
            start: 0,
            count: commands.commands.len() as u32,
        }];
        let grab_ranges = [BodyCommandStepRange {
            start: 0,
            count: grab.intents.len() as u32,
        }];
        let construction_ranges = [BodyCommandStepRange {
            start: 0,
            count: construction.intents.len() as u32,
        }];
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("construction_test_step"),
        });
        world
            .encode_steps_with_grab_intents(
                queue,
                &mut encoder,
                1,
                BodyStepPlan {
                    batch: commands,
                    ranges: &command_ranges,
                    grab_batch: grab,
                    grab_ranges: &grab_ranges,
                    construction_batch: construction,
                    construction_ranges: &construction_ranges,
                },
            )
            .unwrap();
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();
    }

    fn read_buffer(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        device.poll(wgpu::PollType::Wait).unwrap();
        receiver.recv().unwrap().unwrap();
        let bytes = slice.get_mapped_range().to_vec();
        buffer.unmap();
        bytes
    }

    fn read_construction(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> physbox_contracts::ConstructionStateV1 {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("construction_test_state_readback"),
            size: u64::from(CONSTRUCTION_STATE_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("construction_test_state_copy"),
        });
        world.copy_construction_state(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
        construction_state_from_bytes(&read_buffer(device, &buffer)).unwrap()
    }

    fn read_active_bodies(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> Vec<u32> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("active_body_test_readback"),
            size: u64::from(world.body_count + crate::body_state::ACTIVE_BODY_STATE_WORDS)
                * size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("active_body_test_copy"),
        });
        world.copy_active_body_data(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
        bytemuck::cast_slice(&read_buffer(device, &buffer)).to_vec()
    }

    fn read_awake_work(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> (Vec<physbox_contracts::SleepStateV1>, Vec<u32>) {
        let sleep_bytes =
            u64::from(world.body_count) * u64::from(crate::awake_work::SLEEP_STATE_BYTES);
        let awake_bytes = u64::from(world.body_count + crate::awake_work::AWAKE_BODY_STATE_WORDS)
            * size_of::<u32>() as u64;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("awake_work_test_readback"),
            size: sleep_bytes + awake_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("awake_work_test_copy"),
        });
        world.copy_awake_work_data(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
        let bytes = read_buffer(device, &buffer);
        let state_words = bytemuck::cast_slice::<u8, u32>(&bytes[..sleep_bytes as usize]);
        let states = state_words
            .chunks_exact(crate::awake_work::SLEEP_STATE_WORDS as usize)
            .enumerate()
            .map(|(slot, words)| physbox_contracts::SleepStateV1 {
                layout_version: words[6],
                body: physbox_contracts::BodyId {
                    slot: slot as u32,
                    generation: words[0],
                },
                awake: words[1] & 1 != 0,
                sleep_eligible: words[1] & 2 != 0,
                quiet_outer_steps: words[2],
                last_transition_step: words[3],
                transition_causes: words[4],
                policy_version: words[5],
            })
            .collect::<Vec<_>>();
        for state in &states {
            state.validate().unwrap();
        }
        let awake = bytemuck::cast_slice::<u8, u32>(&bytes[sleep_bytes as usize..]).to_vec();
        (states, awake)
    }

    fn read_awake_point_state(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> Vec<u32> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("awake_point_state_readback"),
            size: 16 * size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("awake_point_state_copy"),
        });
        world.copy_awake_point_state(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
        bytemuck::cast_slice(&read_buffer(device, &buffer)).to_vec()
    }

    fn read_sleep_cycle_metrics(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> crate::SleepCycleMetricsV1 {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sleep_cycle_test_readback"),
            size: u64::from(crate::SLEEP_CYCLE_DIAGNOSTIC_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sleep_cycle_test_copy"),
        });
        world
            .copy_sleep_cycle_metrics(&mut encoder, &buffer)
            .unwrap();
        queue.submit([encoder.finish()]);
        crate::sleep_diagnostics::decode_sleep_cycle_metrics(&read_buffer(device, &buffer)).unwrap()
    }

    fn sleeping_chain_world(
        device: &wgpu::Device,
        scenario: &ScenarioV1,
        wake_propagation_rounds: u32,
    ) -> GpuBodyWorld {
        sleeping_world_with_grouping(
            device,
            scenario,
            60,
            wake_propagation_rounds,
            true,
            if wake_propagation_rounds == 8 {
                physbox_launch::SleepGrouping::Local8
            } else {
                physbox_launch::SleepGrouping::Local4
            },
        )
    }

    fn sleeping_world(
        device: &wgpu::Device,
        scenario: &ScenarioV1,
        fixed_step_hz: u32,
        wake_propagation_rounds: u32,
        enabled: bool,
    ) -> GpuBodyWorld {
        sleeping_world_with_grouping(
            device,
            scenario,
            fixed_step_hz,
            wake_propagation_rounds,
            enabled,
            physbox_launch::SleepGrouping::Local4,
        )
    }

    fn sleeping_world_with_grouping(
        device: &wgpu::Device,
        scenario: &ScenarioV1,
        fixed_step_hz: u32,
        wake_propagation_rounds: u32,
        enabled: bool,
        grouping: physbox_launch::SleepGrouping,
    ) -> GpuBodyWorld {
        let scheduling = GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD;
        GpuBodyWorld::new_with_generated_contacts(
            device,
            wgpu::TextureFormat::Rgba8Unorm,
            scenario,
            WorldCapacityPreset::default(),
            GeneratedContactBuildOptions {
                sleep_config: SleepExecutionConfig {
                    enabled,
                    fixed_step_hz,
                    wake_propagation_rounds,
                    speed_threshold: 0.02,
                    grouping,
                },
                branch_robust_tunable: fixed_step_hz != 60,
                ..GeneratedContactBuildOptions::selected(scheduling, fixed_step_hz)
            },
            None,
        )
        .unwrap()
    }

    fn read_validation_state_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &GpuBodyWorld,
    ) -> Vec<u8> {
        let byte_count = u64::from(world.body_count) * size_of::<[u32; 4]>() as u64 * 3;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sleep_validation_state_readback"),
            size: byte_count,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sleep_validation_state_copy"),
        });
        world.copy_full_validation_state(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
        read_buffer(device, &buffer)
    }

    #[test]
    fn gpu_sleep_admission_uses_exact_outer_steps_and_stops_pose_drift() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        #[cfg(feature = "production-validation")]
        let rate_rows = [(60, 30)];
        #[cfg(not(feature = "production-validation"))]
        let rate_rows = [(30, 15), (60, 30), (120, 60)];
        for (fixed_step_hz, quiet_steps) in rate_rows {
            let mut scenario = crate::p5_1_rectangle_floor_scenario(1).unwrap();
            let profile = if fixed_step_hz == 60 {
                crate::SolverProfileConfig::hard_direct()
            } else {
                crate::SolverProfileConfig::soft_coupled()
            };
            profile.apply_to_scenario(&mut scenario).unwrap();
            scenario.manifolds.clear();
            scenario.settings.gravity = Vec2::ZERO;
            scenario.settings.fixed_dt =
                physbox_contracts::FiniteF32::new(1.0 / fixed_step_hz as f32).unwrap();
            scenario.bodies[0].linear_velocity = Vec2::new(0.01, 0.0);
            scenario.bodies[0].angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            scenario.bodies[0].sleeping = false;

            let enabled = sleeping_world(&device, &scenario, fixed_step_hz, 4, true);
            let disabled = sleeping_world(&device, &scenario, fixed_step_hz, 4, false);
            for _ in 0..quiet_steps - 1 {
                encode(
                    &device,
                    &queue,
                    &enabled,
                    &grab_batch(Vec::new()),
                    &empty_construction_intent_batch(),
                );
                encode(
                    &device,
                    &queue,
                    &disabled,
                    &grab_batch(Vec::new()),
                    &empty_construction_intent_batch(),
                );
            }
            let (states, awake) = read_awake_work(&device, &queue, &enabled);
            assert!(states[0].awake);
            assert_eq!(states[0].quiet_outer_steps, quiet_steps - 1);
            assert_eq!(awake[0], 1);
            assert_eq!(
                read_validation_state_bytes(&device, &queue, &enabled),
                read_validation_state_bytes(&device, &queue, &disabled),
                "sleep-enabled motion diverged before the {fixed_step_hz} Hz transition"
            );

            let zero_step_state = read_validation_state_bytes(&device, &queue, &enabled);
            assert_eq!(
                read_validation_state_bytes(&device, &queue, &enabled),
                zero_step_state,
                "zero-step presentation changed GPU state"
            );
            encode(
                &device,
                &queue,
                &enabled,
                &grab_batch(Vec::new()),
                &empty_construction_intent_batch(),
            );
            let (states, awake) = read_awake_work(&device, &queue, &enabled);
            assert!(!states[0].awake);
            assert_eq!(states[0].quiet_outer_steps, quiet_steps);
            assert_eq!(states[0].last_transition_step, quiet_steps);
            assert_ne!(
                states[0].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_AUTOMATIC,
                0
            );
            assert_eq!(awake[0], 0);
            let sleeping_state = read_validation_state_bytes(&device, &queue, &enabled);
            for _ in 0..10 {
                encode(
                    &device,
                    &queue,
                    &enabled,
                    &grab_batch(Vec::new()),
                    &empty_construction_intent_batch(),
                );
            }
            assert_eq!(
                read_validation_state_bytes(&device, &queue, &enabled),
                sleeping_state,
                "sleeping body drifted at {fixed_step_hz} Hz"
            );
            for _ in 11..quiet_steps {
                encode(
                    &device,
                    &queue,
                    &enabled,
                    &grab_batch(Vec::new()),
                    &empty_construction_intent_batch(),
                );
            }
            let metrics = read_sleep_cycle_metrics(&device, &queue, &enabled);
            assert_eq!(metrics.first_stable_sleep_step, Some(quiet_steps));
            assert_eq!(metrics.stable_confirmation_steps, quiet_steps);
            assert_eq!(metrics.awake_body_time, u64::from(quiet_steps - 1));
        }
    }

    #[test]
    fn gpu_contact_neighbors_sleep_together_despite_misaligned_quiet_timers() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = crate::authored_chain_scenario(2, 71).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.9, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = false;
        }
        let world = sleeping_chain_world(&device, &scenario, 4);
        let quiet_steps = physbox_contracts::SleepPolicyV1::experimental_s23()
            .quiet_outer_steps(60)
            .unwrap();
        for (body, initial_quiet_steps) in scenario.bodies.iter().zip([quiet_steps - 1, 0]) {
            let state = [
                body.id.generation,
                3,
                initial_quiet_steps,
                0,
                physbox_contracts::SLEEP_TRANSITION_CAUSE_INITIALIZATION,
                physbox_contracts::SLEEP_POLICY_VERSION,
                physbox_contracts::SLEEP_STATE_LAYOUT_VERSION,
                0,
            ];
            queue.write_buffer(
                world.storage.sleep_state_buffer(),
                u64::from(body.id.slot) * u64::from(crate::awake_work::SLEEP_STATE_BYTES),
                bytemuck::cast_slice(&state),
            );
        }

        for _ in 0..quiet_steps - 1 {
            encode(
                &device,
                &queue,
                &world,
                &grab_batch(Vec::new()),
                &empty_construction_intent_batch(),
            );
        }
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], 2);
        assert!(states[0].awake && states[1].awake);
        assert_eq!(states[0].quiet_outer_steps, quiet_steps);
        assert_eq!(states[1].quiet_outer_steps, quiet_steps - 1);

        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], 0);
        for state in &states[..2] {
            assert!(!state.awake);
            assert_eq!(state.quiet_outer_steps, quiet_steps);
            assert_eq!(state.last_transition_step, quiet_steps);
            assert_ne!(
                state.transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_AUTOMATIC,
                0
            );
        }

        for _ in 0..10 {
            encode(
                &device,
                &queue,
                &world,
                &grab_batch(Vec::new()),
                &empty_construction_intent_batch(),
            );
        }
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], 0);
        assert!(states[..2].iter().all(|state| !state.awake));
    }

    #[test]
    fn gpu_solver_work_follows_awake_bodies_and_points_then_restores_on_wake() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = crate::authored_chain_scenario(2, 43).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.9, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = true;
        }
        let world = sleeping_chain_world(&device, &scenario, 4);
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        let points = read_awake_point_state(&device, &queue, &world);
        assert!(!states[0].awake && !states[1].awake);
        assert_eq!(awake[0], 0);
        assert_eq!(points[2], 0);

        encode(
            &device,
            &queue,
            &world,
            &grab_batch(vec![GrabIntentV1::Begin {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.0),
            }]),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        let points = read_awake_point_state(&device, &queue, &world);
        assert!(states[0].awake && states[1].awake);
        assert_eq!(awake[0], 2);
        assert_eq!(points[2], 2);
        assert_ne!(
            states[0].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_GRAB,
            0
        );
        assert_ne!(
            states[1].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_PROPAGATION,
            0
        );
    }

    #[test]
    fn gpu_new_contact_wakes_a_sleeping_endpoint() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = crate::authored_chain_scenario(2, 79).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.9, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = index == 1;
        }
        let world = sleeping_chain_world(&device, &scenario, 4);
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], 2);
        assert!(states[1].awake);
        assert_eq!(states[1].last_transition_step, 1);
        assert_ne!(
            states[1].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_CONTACT,
            0
        );
    }

    #[test]
    fn gpu_wake_propagation_is_bounded_generation_safe_and_selects_four_rounds() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        for body_count in [16, 64] {
            let mut comparison = crate::authored_chain_scenario(body_count, 23).unwrap();
            crate::SolverProfileConfig::hard_direct()
                .apply_to_scenario(&mut comparison)
                .unwrap();
            comparison.manifolds.clear();
            comparison.settings.gravity = Vec2::ZERO;
            for (index, body) in comparison.bodies.iter_mut().enumerate() {
                body.position = Vec2::new(index as f32 * 0.89, 0.0);
                body.linear_velocity = Vec2::ZERO;
                body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
                body.sleeping = true;
            }
            for rounds in [1, 2, 4, 8] {
                let world = sleeping_chain_world(&device, &comparison, rounds);
                encode(
                    &device,
                    &queue,
                    &world,
                    &grab_batch(vec![GrabIntentV1::Begin {
                        sequence: 1,
                        world_target: Vec2::new(0.0, 0.0),
                    }]),
                    &empty_construction_intent_batch(),
                );
                let (states, awake) = read_awake_work(&device, &queue, &world);
                let expected_awake = rounds + 1;
                assert_eq!(
                    awake[0], expected_awake,
                    "{body_count} bodies, wake rounds {rounds}"
                );
                for state in &states[..expected_awake as usize] {
                    assert!(
                        state.awake,
                        "wake rounds {rounds}, slot {}",
                        state.body.slot
                    );
                }
                for state in &states[expected_awake as usize..body_count as usize] {
                    assert!(
                        !state.awake,
                        "wake rounds {rounds}, slot {}",
                        state.body.slot
                    );
                }
            }
        }

        let mut scenario = crate::authored_chain_scenario(10, 23).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.89, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = true;
        }
        let world = sleeping_chain_world(&device, &scenario, 4);
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(vec![GrabIntentV1::Begin {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.0),
            }]),
            &empty_construction_intent_batch(),
        );
        let (states, _) = read_awake_work(&device, &queue, &world);
        assert!(states[3].awake && states[4].awake);
        assert!(!states[5].awake);
        assert_eq!(states[4].last_transition_step, 1);
        assert_ne!(
            states[4].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_PROPAGATION,
            0
        );
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], 5);
        assert!(!states[5].awake);
        assert_eq!(states[5].last_transition_step, 0);
    }

    #[test]
    fn gpu_exact_sleep_grouping_wakes_the_whole_contact_component() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let body_count = 16;
        let mut scenario = crate::authored_chain_scenario(body_count, 31).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.89, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = true;
        }
        let world = sleeping_world_with_grouping(
            &device,
            &scenario,
            60,
            0,
            true,
            physbox_launch::SleepGrouping::Exact,
        );
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(vec![GrabIntentV1::Begin {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.0),
            }]),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(awake[0], body_count);
        assert!(
            states[..body_count as usize]
                .iter()
                .all(|state| state.awake)
        );
    }

    #[test]
    fn gpu_sleep_cycle_metrics_distinguish_local_four_local_eight_and_exact() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let body_count = 16;
        let mut scenario = crate::authored_chain_scenario(body_count, 37).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.89, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = true;
        }
        for (grouping, rounds, expected_awake, expected_cuts) in [
            (physbox_launch::SleepGrouping::Local4, 4, 5, 1),
            (physbox_launch::SleepGrouping::Local8, 8, 9, 1),
            (physbox_launch::SleepGrouping::Exact, 0, 16, 0),
        ] {
            let world =
                sleeping_world_with_grouping(&device, &scenario, 60, rounds, true, grouping);
            encode(
                &device,
                &queue,
                &world,
                &grab_batch(vec![GrabIntentV1::Begin {
                    sequence: 1,
                    world_target: Vec2::new(0.0, 0.0),
                }]),
                &empty_construction_intent_batch(),
            );
            let metrics = read_sleep_cycle_metrics(&device, &queue, &world);
            assert_eq!(metrics.observed_step, 1, "{grouping:?}");
            assert_eq!(metrics.awake_bodies, expected_awake, "{grouping:?}");
            assert_eq!(
                metrics.sleeping_bodies,
                body_count - expected_awake,
                "{grouping:?}"
            );
            assert_eq!(metrics.woke_this_step, expected_awake, "{grouping:?}");
            assert_eq!(
                metrics.mixed_awake_asleep_contact_cuts,
                Some(expected_cuts),
                "{grouping:?}"
            );
            assert_eq!(
                metrics.awake_body_time,
                u64::from(expected_awake),
                "{grouping:?}"
            );
        }
    }

    #[test]
    fn gpu_wake_roots_cover_grab_reset_construction_and_erased_neighbors() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = crate::authored_chain_scenario(6, 29).unwrap();
        crate::SolverProfileConfig::hard_direct()
            .apply_to_scenario(&mut scenario)
            .unwrap();
        scenario.manifolds.clear();
        scenario.settings.gravity = Vec2::ZERO;
        for (index, body) in scenario.bodies.iter_mut().enumerate() {
            body.position = Vec2::new(index as f32 * 0.9, 0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = physbox_contracts::FiniteF32::new(0.0).unwrap();
            body.sleeping = true;
        }

        let grabbed = sleeping_chain_world(&device, &scenario, 4);
        encode(
            &device,
            &queue,
            &grabbed,
            &grab_batch(vec![GrabIntentV1::Begin {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.0),
            }]),
            &empty_construction_intent_batch(),
        );
        let (states, awake) = read_awake_work(&device, &queue, &grabbed);
        assert_eq!(awake[0], 5);
        assert!(states[0].awake && !states[0].sleep_eligible);
        assert_ne!(
            states[0].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_GRAB,
            0
        );

        let reset = sleeping_chain_world(&device, &scenario, 4);
        let reset_batch = BodyCommandBatchV1 {
            schema_version: SCHEMA_VERSION,
            capacity_commands: BODY_COMMAND_CAPACITY,
            attempted_commands: 1,
            overflowed: false,
            commands: vec![BodyCommandV1::Reset {
                sequence: 1,
                body: scenario.bodies[0].id,
                position: scenario.bodies[0].position,
                angle: scenario.bodies[0].angle,
                linear_velocity: Vec2::ZERO,
                angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            }],
        };
        encode_with_commands(
            &device,
            &queue,
            &reset,
            &reset_batch,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let (states, _) = read_awake_work(&device, &queue, &reset);
        assert!(states[0].awake);
        assert_ne!(
            states[0].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_RESET,
            0
        );

        let lifecycle = sleeping_chain_world(&device, &scenario, 4);
        encode(
            &device,
            &queue,
            &lifecycle,
            &grab_batch(Vec::new()),
            &empty_construction_intent_batch(),
        );
        let mut erase = BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY).unwrap();
        erase
            .push(ConstructionIntentV1::EraseAt {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.0),
            })
            .unwrap();
        encode(
            &device,
            &queue,
            &lifecycle,
            &grab_batch(Vec::new()),
            &erase.artifact(),
        );
        let (states, _) = read_awake_work(&device, &queue, &lifecycle);
        assert!(states[1].awake);
        assert_ne!(
            states[1].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_ERASED_NEIGHBOR,
            0
        );

        let mut place = BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY).unwrap();
        place
            .push(ConstructionIntentV1::PlaceCircle {
                sequence: 2,
                world_target: Vec2::new(-2.0, 0.2),
            })
            .unwrap();
        encode(
            &device,
            &queue,
            &lifecycle,
            &grab_batch(Vec::new()),
            &place.artifact(),
        );
        let (states, _) = read_awake_work(&device, &queue, &lifecycle);
        assert_eq!(states[0].body.generation, 2);
        assert!(states[0].awake);
        assert_ne!(
            states[0].transition_causes & physbox_contracts::SLEEP_TRANSITION_CAUSE_CONSTRUCTION,
            0
        );
    }

    #[test]
    fn gpu_construction_reuses_generations_clears_grab_and_rejects_failures() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario = crate::p5_2_generated_floor_scenario(8).unwrap();
        let world = GpuBodyWorld::new_generated(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            &scenario,
            WorldCapacityPreset::default(),
        )
        .unwrap();
        assert_eq!(world.body_count, 64);

        encode(
            &device,
            &queue,
            &world,
            &grab_batch(vec![GrabIntentV1::Begin {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.65),
            }]),
            &empty_construction_intent_batch(),
        );

        let grab_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("construction_test_grab_readback"),
            size: u64::from(GRAB_STATE_BYTES),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut grab_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("construction_test_grab_copy"),
        });
        world.copy_grab_state(&mut grab_encoder, &grab_readback);
        queue.submit([grab_encoder.finish()]);
        assert!(
            decode_grab_state(&read_buffer(&device, &grab_readback))
                .unwrap()
                .active
        );

        let mut intents = BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY).unwrap();
        for intent in [
            ConstructionIntentV1::EraseAt {
                sequence: 2,
                world_target: Vec2::new(0.0, 0.65),
            },
            ConstructionIntentV1::PlaceCircle {
                sequence: 3,
                world_target: Vec2::new(0.0, 0.65),
            },
            ConstructionIntentV1::PlaceRectangle {
                sequence: 4,
                world_target: Vec2::new(-0.6, 1.0),
            },
            ConstructionIntentV1::PlaceCircle {
                sequence: 5,
                world_target: Vec2::new(-0.6, 1.0),
            },
            ConstructionIntentV1::EraseAt {
                sequence: 6,
                world_target: Vec2::new(20.0, 20.0),
            },
        ] {
            intents.push(intent).unwrap();
        }
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &intents.artifact(),
        );
        let state = read_construction(&device, &queue, &world);
        assert_eq!(state.capacity_bodies, 64);
        assert_eq!(state.live_bodies, 3);
        assert_eq!(state.attempted_intents, 5);
        assert_eq!(state.succeeded_intents, 3);
        assert_eq!(state.placed_circles, 1);
        assert_eq!(state.placed_rectangles, 1);
        assert_eq!(state.erased_bodies, 1);
        assert_eq!(state.deep_overlap_failures, 1);
        assert_eq!(state.erase_miss_failures, 1);
        assert_eq!(state.grab_invalidations, 1);
        let active = read_active_bodies(&device, &queue, &world);
        assert_eq!(&active[..4], &[3, 2, 1, 3]);
        assert_eq!(&active[4..10], &[1, 1, 1, 1, 1, 1]);
        assert_eq!(&active[13..17], &[6, 3, 0, 0]);
        assert_eq!(&active[20..23], &[0, 1, 2]);
        let (sleep_states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(&awake[..3], &[2, 2, 0]);
        assert_eq!(&awake[20..22], &[0, 2]);
        assert_eq!(sleep_states[0].body.generation, 2);
        assert!(sleep_states[0].awake && sleep_states[0].sleep_eligible);
        assert!(!sleep_states[1].awake && !sleep_states[1].sleep_eligible);
        assert!(sleep_states[2].awake && sleep_states[2].sleep_eligible);

        let lane_bytes = u64::from(world.body_count) * size_of::<[u32; 4]>() as u64;
        let validation = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("construction_test_body_readback"),
            size: lane_bytes * 3,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("construction_test_body_copy"),
        });
        world.copy_full_validation_state(&mut encoder, &validation);
        queue.submit([encoder.finish()]);
        let bytes = read_buffer(&device, &validation);
        let metadata_offset = (lane_bytes * 2) as usize;
        let metadata = bytemuck::pod_read_unaligned::<[u32; 4]>(
            &bytes[metadata_offset..metadata_offset + size_of::<[u32; 4]>()],
        );
        assert_eq!(metadata[0], 2);
        assert_ne!(metadata[1] & 1, 0);
        assert_ne!(
            metadata[1] & physbox_contracts::ANALYTIC_SHAPE_FLAG_CIRCLE,
            0
        );

        let reset = world.benchmark_reset_data(&scenario.bodies).unwrap();
        world.reset_from_authored(&queue, &reset);
        let (sleep_states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(&awake[..3], &[1, 1, 0]);
        assert_eq!(awake[20], 0);
        assert_eq!(sleep_states[0].body.generation, 1);
        assert!(sleep_states[0].awake);
        assert!(!sleep_states[1].awake);
    }

    #[test]
    fn gpu_construction_capacity_and_generation_failures_are_atomic() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario = crate::p5_2_generated_floor_scenario(2).unwrap();
        let world = GpuBodyWorld::new_generated(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            &scenario,
            WorldCapacityPreset::default(),
        )
        .unwrap();
        let mut fill = BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY).unwrap();
        for index in 0..63_u32 {
            fill.push(ConstructionIntentV1::PlaceCircle {
                sequence: index + 1,
                world_target: Vec2::new(-20.0 + index as f32 * 0.3, 10.0),
            })
            .unwrap();
        }
        encode(
            &device,
            &queue,
            &world,
            &grab_batch(Vec::new()),
            &fill.artifact(),
        );
        let full = read_construction(&device, &queue, &world);
        assert_eq!(full.live_bodies, 64);
        assert_eq!(full.succeeded_intents, 62);
        assert_eq!(full.capacity_failures, 1);
        let active = read_active_bodies(&device, &queue, &world);
        assert_eq!(&active[..3], &[64, 63, 1]);
        assert_eq!(&active[20..84], &(0..64).collect::<Vec<_>>());
        let (sleep_states, awake) = read_awake_work(&device, &queue, &world);
        assert_eq!(&awake[..3], &[63, 63, 0]);
        assert_eq!(awake[20], 0);
        assert_eq!(awake[21], 2);
        assert_eq!(awake[82], 63);
        assert!(sleep_states[0].awake);
        assert!(!sleep_states[1].awake);
        assert!(sleep_states[63].awake);

        let exhausted = GpuBodyWorld::new_generated(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            &scenario,
            WorldCapacityPreset::default(),
        )
        .unwrap();
        let active_before = read_active_bodies(&device, &queue, &exhausted);
        exhausted.overwrite_body_generation_for_test(&queue, 0, u32::MAX);
        let mut erase = BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY).unwrap();
        erase
            .push(ConstructionIntentV1::EraseAt {
                sequence: 1,
                world_target: Vec2::new(0.0, 0.65),
            })
            .unwrap();
        encode(
            &device,
            &queue,
            &exhausted,
            &grab_batch(Vec::new()),
            &erase.artifact(),
        );
        let generation = read_construction(&device, &queue, &exhausted);
        assert_eq!(generation.live_bodies, 2);
        assert_eq!(generation.succeeded_intents, 0);
        assert_eq!(generation.generation_failures, 1);
        assert_eq!(
            read_active_bodies(&device, &queue, &exhausted),
            active_before
        );
        let (sleep_states, awake) = read_awake_work(&device, &queue, &exhausted);
        assert_eq!(sleep_states[0].body.generation, u32::MAX);
        assert_eq!(&awake[..3], &[1, 1, 0]);
        assert_eq!(awake[20], 0);
    }

    #[test]
    fn interactive_capacity_tiers_are_bounded_and_leave_current_scene_headroom() {
        assert_eq!(
            interactive_body_capacity(2, WorldCapacityPreset::Slots64).unwrap(),
            64
        );
        assert_eq!(
            interactive_body_capacity(5, WorldCapacityPreset::Slots256).unwrap(),
            256
        );
        assert_eq!(
            interactive_body_capacity(64, WorldCapacityPreset::Slots64).unwrap(),
            128
        );
        assert_eq!(
            interactive_body_capacity(103, WorldCapacityPreset::Slots64).unwrap(),
            128
        );
        assert_eq!(
            interactive_body_capacity(2, WorldCapacityPreset::Slots1000).unwrap(),
            1_000
        );
        assert_eq!(
            interactive_body_capacity(2, WorldCapacityPreset::Slots10000).unwrap(),
            10_000
        );
        assert!(interactive_body_capacity(u32::MAX, WorldCapacityPreset::Slots64).is_err());
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn contact_policy_comparator_does_not_relax_the_product_constructor() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, _) = pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario =
            p5_2_settling_review_scenario(16, P5_2SettlingReviewPolicy::SoftnessOnly).unwrap();

        let product = GpuBodyWorld::new_generated(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            &scenario,
            WorldCapacityPreset::default(),
        );
        assert!(
            product.as_ref().is_err_and(
                |error| error == "solver settings differ from the accepted S5.4 baseline"
            )
        );

        GpuBodyWorld::new_generated_with_contact_policy(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            &scenario,
            WorldCapacityPreset::default(),
            ContactIdentityPolicyV1::DirectMaximumSeparation,
        )
        .expect("explicit contact-policy comparator accepts laboratory softness");
    }
}
