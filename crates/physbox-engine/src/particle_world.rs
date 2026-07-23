#![cfg_attr(not(test), allow(dead_code))]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CapacityRecordV1, CoupledScenarioV1, ParticleMaterialV1, ParticleRegionV1, Validate, Vec2,
};
use wgpu::util::DeviceExt;

use crate::{
    CameraFrame2d, GpuBodyStorage, body_lifecycle::KILL_ZONE_Y, renderer::RenderFrameTarget,
};

pub(crate) const MAX_PRODUCT_PARTICLE_CAPACITY: u32 = 32_768;
pub(crate) const LIQUID_SANDBOX_PARTICLE_CAPACITY: u32 = 8_192;
pub(crate) const PARTICLE_COMMAND_CAPACITY: u32 = 64;
const PARTICLE_COUNTER_WORDS: u32 = 21;
const GRID_WIDTH: u32 = 64;
const GRID_HEIGHT: u32 = 68;
const GRID_CELL_COUNT: u32 = GRID_WIDTH * GRID_HEIGHT;
const PARTICLE_WORKGROUP_SIZE: u32 = 64;
const PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE: u32 = 256;
const PARTICLE_GRID_BLOCK_SPAN: u32 = 512;
const PARTICLE_SUBSTEPS: u32 = 2;
const PARTICLE_BYTES_PER_SLOT: u64 = 64;
pub const GPU_PARTICLE_DIAGNOSTICS_VERSION: u32 = 4;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuParticleDiagnosticsV4 {
    pub version: u32,
    pub work_scope: &'static str,
    pub capacity: u32,
    pub active_count: u32,
    pub active_flags_count: u32,
    pub allocation_cursor: u32,
    pub overflow_flags: u32,
    pub grid_insertions: u32,
    pub occupied_grid_cells: u32,
    pub grid_overflow: u32,
    pub neighbor_checks: u32,
    pub accepted_neighbors: u32,
    pub density_evaluations: u64,
    pub correction_evaluations: u64,
    pub body_contacts: u32,
    pub escaped_particles: u32,
    pub spawned_particles: u32,
    pub erased_particles: u32,
    pub killed_particles: u32,
    pub command_drops: u32,
    pub corrections: u32,
    pub clamped_corrections: u32,
    pub body_contribution_edges: u32,
    pub fluid_wake_roots: u32,
    pub body_contribution_overflow: u32,
    pub boundary_density_contacts: u32,
    pub boundary_pressure_edges: u32,
    pub dispatches_per_step: u32,
    pub render_instances: u32,
    pub allocated_bytes: u64,
    pub high_water_bytes: u64,
    pub density_error_rms: f32,
    pub fluid_motion_rms: f32,
    pub state_finite: bool,
    pub capacity_clean: bool,
    pub blocking_readback_for_advancement: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleParamsRaw {
    dt_gravity: [f32; 4],
    material: [f32; 4],
    pressure: [f32; 4],
    grid: [f32; 4],
    counts: [u32; 4],
    limits: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleRenderParamsRaw {
    viewport: [f32; 4],
    camera: [f32; 4],
    material: [f32; 4],
    debug: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleCommandRaw {
    kind_count_material_sequence: [u32; 4],
    center_radius_spacing: [f32; 4],
    velocity_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParticleCommandBatchRaw {
    counts: [u32; 4],
    commands: [ParticleCommandRaw; PARTICLE_COMMAND_CAPACITY as usize],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ParticleCommand {
    EmitCircle {
        center: Vec2,
        radius: f32,
        spacing: f32,
        initial_velocity: Vec2,
        material_id: u32,
        sequence: u32,
    },
    EraseCircle {
        center: Vec2,
        radius: f32,
        sequence: u32,
    },
}

impl ParticleCommand {
    fn raw(self) -> Result<ParticleCommandRaw, String> {
        match self {
            Self::EmitCircle {
                center,
                radius,
                spacing,
                initial_velocity,
                material_id,
                sequence,
            } => {
                center.validate().map_err(|error| error.to_string())?;
                initial_velocity
                    .validate()
                    .map_err(|error| error.to_string())?;
                if !radius.is_finite() || !spacing.is_finite() || radius <= 0.0 || spacing <= 0.0 {
                    return Err("particle emission requires positive finite radius/spacing".into());
                }
                let spawn_count = circle_spawn_count(radius, spacing)?;
                Ok(ParticleCommandRaw {
                    kind_count_material_sequence: [1, spawn_count, material_id, sequence],
                    center_radius_spacing: [center.x, center.y, radius, spacing],
                    velocity_padding: [initial_velocity.x, initial_velocity.y, 0.0, 0.0],
                })
            }
            Self::EraseCircle {
                center,
                radius,
                sequence,
            } => {
                center.validate().map_err(|error| error.to_string())?;
                if !radius.is_finite() || radius <= 0.0 {
                    return Err("particle erase requires a positive finite radius".into());
                }
                Ok(ParticleCommandRaw {
                    kind_count_material_sequence: [2, 0, 0, sequence],
                    center_radius_spacing: [center.x, center.y, radius, 0.0],
                    velocity_padding: [0.0; 4],
                })
            }
        }
    }

    const fn sequence(self) -> u32 {
        match self {
            Self::EmitCircle { sequence, .. } | Self::EraseCircle { sequence, .. } => sequence,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParticleValidationLayout {
    pub(crate) capacity: u32,
    pub(crate) positions_offset: u64,
    pub(crate) velocities_offset: u64,
    pub(crate) metadata_offset: u64,
    pub(crate) active_flags_offset: u64,
    pub(crate) density_offset: u64,
    pub(crate) counters_offset: u64,
    pub(crate) cell_counts_offset: u64,
    pub(crate) total_bytes: u64,
}

pub(crate) struct GpuParticleWorld {
    capacity: u32,
    initial_count: u32,
    pressure_iterations: u32,
    material: ParticleMaterialV1,
    positions: wgpu::Buffer,
    velocities: wgpu::Buffer,
    metadata: wgpu::Buffer,
    active_flags: wgpu::Buffer,
    _active_indices: wgpu::Buffer,
    _active_prefixes: wgpu::Buffer,
    _active_block_offsets: wgpu::Buffer,
    density_states: wgpu::Buffer,
    _corrections: wgpu::Buffer,
    _body_contributions: wgpu::Buffer,
    _body_contact_generations: wgpu::Buffer,
    _body_partials: wgpu::Buffer,
    counters: wgpu::Buffer,
    indirect: wgpu::Buffer,
    _initial_positions: wgpu::Buffer,
    _initial_velocities: wgpu::Buffer,
    _initial_metadata: wgpu::Buffer,
    _cell_counts: wgpu::Buffer,
    _cell_offsets: wgpu::Buffer,
    _cell_bins: wgpu::Buffer,
    _cell_block_keys: wgpu::Buffer,
    command_batch: wgpu::Buffer,
    _params: wgpu::Buffer,
    render_params: wgpu::Buffer,
    core_group: wgpu::BindGroup,
    core_begin_step: wgpu::ComputePipeline,
    core_integrate: wgpu::ComputePipeline,
    active_group: wgpu::BindGroup,
    core_count_active: wgpu::ComputePipeline,
    core_scan_active_blocks: wgpu::ComputePipeline,
    core_scatter_active: wgpu::ComputePipeline,
    grid_group: wgpu::BindGroup,
    grid_clear: wgpu::ComputePipeline,
    grid_count: wgpu::ComputePipeline,
    grid_order_group: wgpu::BindGroup,
    grid_scan_ranges: wgpu::ComputePipeline,
    grid_scatter: wgpu::ComputePipeline,
    density_group: wgpu::BindGroup,
    density_estimate: wgpu::ComputePipeline,
    correction_group: wgpu::BindGroup,
    correction_gather: wgpu::ComputePipeline,
    correction_apply: wgpu::ComputePipeline,
    boundary_group: wgpu::BindGroup,
    boundary_project: wgpu::ComputePipeline,
    boundary_density_group: wgpu::BindGroup,
    boundary_density_pipeline: wgpu::ComputePipeline,
    boundary_pressure_group: wgpu::BindGroup,
    boundary_pressure_pipeline: wgpu::ComputePipeline,
    body_reduce_group: wgpu::BindGroup,
    body_reduce_pipeline: wgpu::ComputePipeline,
    body_gather_group: wgpu::BindGroup,
    body_position_gather_pipeline: wgpu::ComputePipeline,
    body_velocity_gather_pipeline: wgpu::ComputePipeline,
    body_velocity_group: wgpu::BindGroup,
    body_velocity_pipeline: wgpu::ComputePipeline,
    velocity_group: wgpu::BindGroup,
    velocity_reconstruct: wgpu::ComputePipeline,
    velocity_gather: wgpu::ComputePipeline,
    velocity_apply: wgpu::ComputePipeline,
    tools_group: wgpu::BindGroup,
    tools_erase: wgpu::ComputePipeline,
    tools_spawn: wgpu::ComputePipeline,
    reset_group: wgpu::BindGroup,
    reset_pipeline: wgpu::ComputePipeline,
    render_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    body_capacity: u32,
    body_tile_count: u32,
}

impl GpuParticleWorld {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &CoupledScenarioV1,
        body_storage: &GpuBodyStorage,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        if scenario.materials.len() != 1 {
            return Err("first GPU particle world requires exactly one material".to_owned());
        }
        let material = scenario.materials[0];
        let capacity = scenario.particle_capacity;
        if capacity == 0 || capacity > MAX_PRODUCT_PARTICLE_CAPACITY {
            return Err(format!(
                "particle capacity {capacity} is outside the product bound 1..={MAX_PRODUCT_PARTICLE_CAPACITY}"
            ));
        }
        let initial = initial_particle_data(&scenario.regions, material, capacity)?;
        let initial_count = initial.active_count;
        let particle_bytes = u64::from(capacity) * 16;
        ensure_storage_size(device, particle_bytes, "particle 16-byte lane")?;
        ensure_storage_size(
            device,
            u64::from(capacity) * 4,
            "compact particle grid records",
        )?;

        let storage_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let positions = storage_init(
            device,
            "physbox5_particle_positions",
            &initial.positions,
            storage_usage,
        );
        let velocities = storage_init(
            device,
            "physbox5_particle_velocities",
            &initial.velocities,
            storage_usage,
        );
        let metadata = storage_init(
            device,
            "physbox5_particle_metadata",
            &initial.metadata,
            storage_usage,
        );
        let active_flags = storage_init(
            device,
            "physbox5_particle_active_flags",
            &initial.active_flags,
            storage_usage,
        );
        let active_indices = storage_init(
            device,
            "physbox5_particle_active_indices",
            &initial.active_indices,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let active_prefixes = empty_storage(
            device,
            "physbox5_particle_active_prefixes",
            u64::from(capacity) * 4,
            wgpu::BufferUsages::empty(),
        );
        let active_block_offsets = empty_storage(
            device,
            "physbox5_particle_active_block_offsets",
            u64::from(capacity.div_ceil(PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE)) * 4,
            wgpu::BufferUsages::empty(),
        );
        let density_states = empty_storage(
            device,
            "physbox5_particle_density_states",
            particle_bytes,
            wgpu::BufferUsages::COPY_SRC,
        );
        let corrections = empty_storage(
            device,
            "physbox5_particle_corrections",
            u64::from(capacity) * 8,
            wgpu::BufferUsages::empty(),
        );
        let body_contributions = empty_storage(
            device,
            "physbox5_particle_body_contributions",
            u64::from(capacity) * 16,
            wgpu::BufferUsages::COPY_SRC,
        );
        let body_contact_generations = empty_storage(
            device,
            "physbox5_particle_body_contact_generations",
            u64::from(capacity) * 4,
            wgpu::BufferUsages::COPY_SRC,
        );
        let body_tile_count = capacity.div_ceil(PARTICLE_WORKGROUP_SIZE);
        let body_partial_bytes =
            u64::from(body_storage.layout().capacity_elements) * u64::from(body_tile_count) * 16;
        ensure_storage_size(device, body_partial_bytes, "particle body partials")?;
        let body_partials = empty_storage(
            device,
            "physbox5_particle_body_partials",
            body_partial_bytes,
            wgpu::BufferUsages::COPY_SRC,
        );
        let counters = storage_init(
            device,
            "physbox5_particle_counters",
            &initial.counters,
            storage_usage,
        );
        let indirect = storage_init(
            device,
            "physbox5_particle_indirect",
            &initial.indirect,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_SRC,
        );
        let initial_positions = storage_init(
            device,
            "physbox5_particle_initial_positions",
            &initial.positions,
            wgpu::BufferUsages::STORAGE,
        );
        let initial_velocities = storage_init(
            device,
            "physbox5_particle_initial_velocities",
            &initial.velocities,
            wgpu::BufferUsages::STORAGE,
        );
        let initial_metadata = storage_init(
            device,
            "physbox5_particle_initial_metadata",
            &initial.metadata,
            wgpu::BufferUsages::STORAGE,
        );
        let cell_counts = empty_storage(
            device,
            "physbox5_particle_grid_counts",
            u64::from(GRID_CELL_COUNT) * 4,
            wgpu::BufferUsages::COPY_SRC,
        );
        let cell_offsets = empty_storage(
            device,
            "physbox5_particle_grid_offsets",
            u64::from(GRID_CELL_COUNT) * 4,
            wgpu::BufferUsages::COPY_SRC,
        );
        let cell_bins = empty_storage(
            device,
            "physbox5_particle_grid_records",
            u64::from(capacity) * 4,
            wgpu::BufferUsages::empty(),
        );
        let cell_block_keys = empty_storage(
            device,
            "physbox5_particle_grid_block_keys",
            u64::from(capacity.div_ceil(PARTICLE_GRID_BLOCK_SPAN))
                * u64::from(PARTICLE_GRID_BLOCK_SPAN)
                * 4,
            wgpu::BufferUsages::empty(),
        );
        let command_batch = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_particle_command_batch"),
            size: size_of::<ParticleCommandBatchRaw>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let substep_dt = scenario.rigid.settings.fixed_dt.get() / PARTICLE_SUBSTEPS as f32;
        let params_value = ParticleParamsRaw {
            dt_gravity: [
                substep_dt,
                scenario.rigid.settings.gravity.x,
                scenario.rigid.settings.gravity.y,
                material.gravity_scale.get(),
            ],
            material: [
                material.radius.get(),
                material.support_radius(),
                material.rest_density.get(),
                material.damping_strength.get(),
            ],
            pressure: [
                material.pressure_strength.get(),
                material.near_pressure_strength.get(),
                material.radius.get() * material.max_pressure_displacement_multiplier.get(),
                material.particle_mass().recip(),
            ],
            grid: [-16.0, KILL_ZONE_Y, 0.5, KILL_ZONE_Y],
            counts: [
                capacity,
                body_storage.layout().capacity_elements,
                GRID_WIDTH,
                GRID_HEIGHT,
            ],
            limits: [
                capacity,
                PARTICLE_COMMAND_CAPACITY,
                initial_count,
                scenario.settings.pressure_iterations,
            ],
        };
        let params = uniform_init(device, "physbox5_particle_params", &params_value);
        let render_params = uniform_init(
            device,
            "physbox5_particle_render_params",
            &ParticleRenderParamsRaw::zeroed(),
        );

        let core_layout = layout(
            device,
            "physbox5_particle_core_layout",
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, false),
                (4, false),
                (5, false),
                (6, false),
            ],
            7,
            wgpu::ShaderStages::COMPUTE,
        );
        let core_group = bind_group(
            device,
            "physbox5_particle_core_group",
            &core_layout,
            &[
                &positions,
                &velocities,
                &metadata,
                &active_flags,
                &active_indices,
                &counters,
                &indirect,
                &params,
            ],
        );
        let core_shader = shader(
            device,
            "physbox5_particle_core",
            include_str!("particle_core.wgsl"),
        );
        let core_begin_step = compute(
            device,
            "particle_begin_step",
            &core_layout,
            &core_shader,
            "begin_step",
        );
        let core_integrate = compute(
            device,
            "particle_integrate",
            &core_layout,
            &core_shader,
            "integrate",
        );
        let active_layout = layout(
            device,
            "physbox5_particle_active_compaction_layout",
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, false),
                (4, false),
                (5, false),
            ],
            6,
            wgpu::ShaderStages::COMPUTE,
        );
        let active_group = bind_group(
            device,
            "physbox5_particle_active_compaction_group",
            &active_layout,
            &[
                &active_flags,
                &active_indices,
                &active_prefixes,
                &active_block_offsets,
                &counters,
                &indirect,
                &params,
            ],
        );
        let active_shader = shader(
            device,
            "physbox5_particle_active_compaction",
            include_str!("particle_active_compaction.wgsl"),
        );
        let core_count_active = compute(
            device,
            "particle_count_active",
            &active_layout,
            &active_shader,
            "count_active",
        );
        let core_scan_active_blocks = compute(
            device,
            "particle_scan_active_blocks",
            &active_layout,
            &active_shader,
            "scan_active_blocks",
        );
        let core_scatter_active = compute(
            device,
            "particle_scatter_active",
            &active_layout,
            &active_shader,
            "scatter_active",
        );

        let grid_layout = layout(
            device,
            "physbox5_particle_grid_layout",
            &[
                (0, true),
                (1, false),
                (2, false),
                (3, true),
                (4, false),
                (5, false),
                (6, false),
            ],
            7,
            wgpu::ShaderStages::COMPUTE,
        );
        let grid_group = bind_group(
            device,
            "physbox5_particle_grid_group",
            &grid_layout,
            &[
                &positions,
                &metadata,
                &active_flags,
                &active_indices,
                &cell_counts,
                &cell_block_keys,
                &counters,
                &params,
            ],
        );
        let grid_shader = shader(
            device,
            "physbox5_particle_grid",
            include_str!("particle_grid.wgsl"),
        );
        let grid_clear = compute(
            device,
            "particle_grid_clear",
            &grid_layout,
            &grid_shader,
            "clear_grid",
        );
        let grid_count = compute(
            device,
            "particle_grid_count",
            &grid_layout,
            &grid_shader,
            "count_grid",
        );
        let grid_order_layout = layout(
            device,
            "physbox5_particle_grid_order_layout",
            &[
                (0, true),
                (1, false),
                (2, true),
                (3, true),
                (4, false),
                (5, false),
                (6, true),
                (7, false),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let grid_order_group = bind_group(
            device,
            "physbox5_particle_grid_order_group",
            &grid_order_layout,
            &[
                &metadata,
                &active_flags,
                &active_indices,
                &cell_counts,
                &cell_offsets,
                &cell_bins,
                &cell_block_keys,
                &counters,
                &params,
            ],
        );
        let grid_order_shader = shader(
            device,
            "physbox5_particle_grid_order",
            include_str!("particle_grid_order.wgsl"),
        );
        let grid_scan_ranges = compute(
            device,
            "particle_grid_scan_ranges",
            &grid_order_layout,
            &grid_order_shader,
            "scan_grid_ranges",
        );
        let grid_scatter = compute(
            device,
            "particle_grid_scatter",
            &grid_order_layout,
            &grid_order_shader,
            "scatter_grid",
        );

        let density_layout = layout(
            device,
            "physbox5_particle_density_layout",
            &[
                (0, true),
                (1, false),
                (2, false),
                (3, true),
                (4, true),
                (5, false),
                (6, false),
            ],
            7,
            wgpu::ShaderStages::COMPUTE,
        );
        let density_group = bind_group(
            device,
            "physbox5_particle_density_group",
            &density_layout,
            &[
                &positions,
                &active_flags,
                &cell_counts,
                &cell_offsets,
                &cell_bins,
                &density_states,
                &counters,
                &params,
            ],
        );
        let density_shader = shader(
            device,
            "physbox5_particle_density",
            include_str!("particle_density.wgsl"),
        );
        let density_estimate = compute(
            device,
            "particle_density_estimate",
            &density_layout,
            &density_shader,
            "estimate_density",
        );

        let correction_layout = layout(
            device,
            "physbox5_particle_correction_layout",
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, true),
                (4, true),
                (5, true),
                (6, false),
                (7, false),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let correction_group = bind_group(
            device,
            "physbox5_particle_correction_group",
            &correction_layout,
            &[
                &positions,
                &active_flags,
                &cell_counts,
                &cell_offsets,
                &cell_bins,
                &density_states,
                &corrections,
                &counters,
                &params,
            ],
        );
        let correction_shader = shader(
            device,
            "physbox5_particle_correction",
            include_str!("particle_correction.wgsl"),
        );
        let correction_gather = compute(
            device,
            "particle_correction_gather",
            &correction_layout,
            &correction_shader,
            "gather_corrections",
        );
        let correction_apply = compute(
            device,
            "particle_correction_apply",
            &correction_layout,
            &correction_shader,
            "apply_corrections",
        );

        let boundary_layout = layout(
            device,
            "physbox5_particle_boundary_layout",
            &[
                (0, false),
                (1, false),
                (2, true),
                (3, true),
                (4, true),
                (5, false),
                (6, false),
                (7, false),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let boundary_group = bind_group(
            device,
            "physbox5_particle_boundary_group",
            &boundary_layout,
            &[
                &positions,
                &active_flags,
                body_storage.pose_buffer(),
                body_storage.properties_buffer(),
                body_storage.metadata_buffer(),
                &body_contributions,
                &body_contact_generations,
                &counters,
                &params,
            ],
        );
        let boundary_shader = shader(
            device,
            "physbox5_particle_boundary",
            include_str!("particle_boundary.wgsl"),
        );
        let boundary_project = compute(
            device,
            "particle_boundary_project",
            &boundary_layout,
            &boundary_shader,
            "project_boundaries",
        );
        let boundary_density_layout = layout(
            device,
            "physbox5_particle_boundary_density_layout",
            &[
                (0, true),
                (1, false),
                (2, false),
                (3, true),
                (4, true),
                (5, true),
                (6, false),
            ],
            7,
            wgpu::ShaderStages::COMPUTE,
        );
        let boundary_density_group = bind_group(
            device,
            "physbox5_particle_boundary_density_group",
            &boundary_density_layout,
            &[
                &positions,
                &active_flags,
                &density_states,
                body_storage.pose_buffer(),
                body_storage.properties_buffer(),
                body_storage.metadata_buffer(),
                &counters,
                &params,
            ],
        );
        let boundary_density_shader = shader(
            device,
            "physbox5_particle_boundary_density",
            include_str!("particle_boundary_density.wgsl"),
        );
        let boundary_density_pipeline = compute(
            device,
            "particle_boundary_density",
            &boundary_density_layout,
            &boundary_density_shader,
            "add_boundary_density",
        );
        let boundary_pressure_layout = layout(
            device,
            "physbox5_particle_boundary_pressure_layout",
            &[
                (0, false),
                (1, false),
                (2, true),
                (3, true),
                (4, true),
                (5, true),
                (6, false),
                (7, false),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let boundary_pressure_group = bind_group(
            device,
            "physbox5_particle_boundary_pressure_group",
            &boundary_pressure_layout,
            &[
                &positions,
                &active_flags,
                &density_states,
                body_storage.pose_buffer(),
                body_storage.properties_buffer(),
                body_storage.metadata_buffer(),
                &body_contributions,
                &body_contact_generations,
                &params,
            ],
        );
        let boundary_pressure_shader = shader(
            device,
            "physbox5_particle_boundary_pressure",
            include_str!("particle_boundary_pressure.wgsl"),
        );
        let boundary_pressure_pipeline = compute(
            device,
            "particle_boundary_pressure",
            &boundary_pressure_layout,
            &boundary_pressure_shader,
            "project_boundary_pressure",
        );
        let body_gather_layout = layout(
            device,
            "physbox5_particle_body_gather_layout",
            &[
                (0, true),
                (1, false),
                (2, false),
                (3, true),
                (4, false),
                (5, false),
            ],
            6,
            wgpu::ShaderStages::COMPUTE,
        );
        let body_gather_group = bind_group(
            device,
            "physbox5_particle_body_gather_group",
            &body_gather_layout,
            &[
                &body_partials,
                body_storage.pose_buffer(),
                body_storage.motion_buffer(),
                body_storage.metadata_buffer(),
                body_storage.wake_request_buffer(),
                &counters,
                &params,
            ],
        );
        let body_reduce_layout = layout(
            device,
            "physbox5_particle_body_reduce_layout",
            &[(0, true), (1, true), (2, false), (3, false)],
            4,
            wgpu::ShaderStages::COMPUTE,
        );
        let body_reduce_group = bind_group(
            device,
            "physbox5_particle_body_reduce_group",
            &body_reduce_layout,
            &[
                &body_contributions,
                body_storage.metadata_buffer(),
                &body_partials,
                &counters,
                &params,
            ],
        );
        let body_reduce_shader = shader(
            device,
            "physbox5_particle_body_reduce",
            include_str!("particle_body_reduce.wgsl"),
        );
        let body_reduce_pipeline = compute(
            device,
            "particle_body_reduce",
            &body_reduce_layout,
            &body_reduce_shader,
            "reduce_body_contributions",
        );
        let body_gather_shader = shader(
            device,
            "physbox5_particle_body_gather",
            include_str!("particle_body_gather.wgsl"),
        );
        let body_position_gather_pipeline = compute(
            device,
            "particle_body_position_gather",
            &body_gather_layout,
            &body_gather_shader,
            "gather_body_position_response",
        );
        let body_velocity_gather_pipeline = compute(
            device,
            "particle_body_velocity_gather",
            &body_gather_layout,
            &body_gather_shader,
            "gather_body_velocity_response",
        );
        let body_velocity_layout = layout(
            device,
            "physbox5_particle_body_velocity_layout",
            &[
                (0, true),
                (1, false),
                (2, true),
                (3, false),
                (4, true),
                (5, true),
                (6, true),
                (7, true),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let body_velocity_group = bind_group(
            device,
            "physbox5_particle_body_velocity_group",
            &body_velocity_layout,
            &[
                &positions,
                &velocities,
                &body_contact_generations,
                &body_contributions,
                body_storage.pose_buffer(),
                body_storage.motion_buffer(),
                body_storage.properties_buffer(),
                body_storage.metadata_buffer(),
                &params,
            ],
        );
        let body_velocity_shader = shader(
            device,
            "physbox5_particle_body_velocity",
            include_str!("particle_body_velocity.wgsl"),
        );
        let body_velocity_pipeline = compute(
            device,
            "particle_body_velocity",
            &body_velocity_layout,
            &body_velocity_shader,
            "solve_body_velocity",
        );

        let velocity_layout = layout(
            device,
            "physbox5_particle_velocity_layout",
            &[
                (0, true),
                (1, false),
                (2, false),
                (3, false),
                (4, true),
                (5, true),
                (6, false),
            ],
            7,
            wgpu::ShaderStages::COMPUTE,
        );
        let velocity_group = bind_group(
            device,
            "physbox5_particle_velocity_group",
            &velocity_layout,
            &[
                &positions,
                &velocities,
                &active_flags,
                &cell_counts,
                &cell_offsets,
                &cell_bins,
                &corrections,
                &params,
            ],
        );
        let velocity_shader = shader(
            device,
            "physbox5_particle_velocity",
            include_str!("particle_velocity.wgsl"),
        );
        let velocity_reconstruct = compute(
            device,
            "particle_velocity_reconstruct",
            &velocity_layout,
            &velocity_shader,
            "reconstruct_velocity",
        );
        let velocity_gather = compute(
            device,
            "particle_velocity_gather",
            &velocity_layout,
            &velocity_shader,
            "gather_viscosity",
        );
        let velocity_apply = compute(
            device,
            "particle_velocity_apply",
            &velocity_layout,
            &velocity_shader,
            "apply_viscosity",
        );

        let tools_layout = layout(
            device,
            "physbox5_particle_tools_layout",
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, false),
                (4, false),
                (5, true),
            ],
            6,
            wgpu::ShaderStages::COMPUTE,
        );
        let tools_group = bind_group(
            device,
            "physbox5_particle_tools_group",
            &tools_layout,
            &[
                &positions,
                &velocities,
                &metadata,
                &active_flags,
                &counters,
                &command_batch,
                &params,
            ],
        );
        let tools_shader = shader(
            device,
            "physbox5_particle_tools",
            include_str!("particle_tools.wgsl"),
        );
        let tools_erase = compute(
            device,
            "particle_tools_erase",
            &tools_layout,
            &tools_shader,
            "erase",
        );
        let tools_spawn = compute(
            device,
            "particle_tools_spawn",
            &tools_layout,
            &tools_shader,
            "spawn",
        );

        let reset_layout = layout(
            device,
            "physbox5_particle_reset_layout",
            &[
                (0, false),
                (1, false),
                (2, false),
                (3, false),
                (4, true),
                (5, true),
                (6, true),
                (7, false),
            ],
            8,
            wgpu::ShaderStages::COMPUTE,
        );
        let reset_group = bind_group(
            device,
            "physbox5_particle_reset_group",
            &reset_layout,
            &[
                &positions,
                &velocities,
                &metadata,
                &active_flags,
                &initial_positions,
                &initial_velocities,
                &initial_metadata,
                &counters,
                &params,
            ],
        );
        let reset_shader = shader(
            device,
            "physbox5_particle_reset",
            include_str!("particle_reset.wgsl"),
        );
        let reset_pipeline = compute(
            device,
            "particle_reset",
            &reset_layout,
            &reset_shader,
            "reset_particles",
        );

        let render_layout = layout(
            device,
            "physbox5_particle_render_layout",
            &[(0, true), (1, true), (2, true), (3, true)],
            4,
            wgpu::ShaderStages::VERTEX,
        );
        let render_group = bind_group(
            device,
            "physbox5_particle_render_group",
            &render_layout,
            &[
                &positions,
                &velocities,
                &density_states,
                &metadata,
                &render_params,
            ],
        );
        let render_shader = shader(
            device,
            "physbox5_particle_render",
            include_str!("particle_render.wgsl"),
        );
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_particle_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox5_particle_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("particle_vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("particle_fragment"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            capacity,
            initial_count,
            pressure_iterations: scenario.settings.pressure_iterations,
            material,
            positions,
            velocities,
            metadata,
            active_flags,
            _active_indices: active_indices,
            _active_prefixes: active_prefixes,
            _active_block_offsets: active_block_offsets,
            density_states,
            _corrections: corrections,
            _body_contributions: body_contributions,
            _body_contact_generations: body_contact_generations,
            _body_partials: body_partials,
            counters,
            indirect,
            _initial_positions: initial_positions,
            _initial_velocities: initial_velocities,
            _initial_metadata: initial_metadata,
            _cell_counts: cell_counts,
            _cell_offsets: cell_offsets,
            _cell_bins: cell_bins,
            _cell_block_keys: cell_block_keys,
            command_batch,
            _params: params,
            render_params,
            core_group,
            core_begin_step,
            core_integrate,
            active_group,
            core_count_active,
            core_scan_active_blocks,
            core_scatter_active,
            grid_group,
            grid_clear,
            grid_count,
            grid_order_group,
            grid_scan_ranges,
            grid_scatter,
            density_group,
            density_estimate,
            correction_group,
            correction_gather,
            correction_apply,
            boundary_group,
            boundary_project,
            boundary_density_group,
            boundary_density_pipeline,
            boundary_pressure_group,
            boundary_pressure_pipeline,
            body_reduce_group,
            body_reduce_pipeline,
            body_gather_group,
            body_position_gather_pipeline,
            body_velocity_gather_pipeline,
            body_velocity_group,
            body_velocity_pipeline,
            velocity_group,
            velocity_reconstruct,
            velocity_gather,
            velocity_apply,
            tools_group,
            tools_erase,
            tools_spawn,
            reset_group,
            reset_pipeline,
            render_group,
            render_pipeline,
            body_capacity: body_storage.layout().capacity_elements,
            body_tile_count,
        })
    }

    pub(crate) fn encode_step(&self, encoder: &mut wgpu::CommandEncoder) {
        self.dispatch(
            encoder,
            "particle_begin_step",
            &self.core_begin_step,
            &self.core_group,
            [1, 1, 1],
        );
        for _ in 0..PARTICLE_SUBSTEPS {
            self.dispatch_capacity(
                encoder,
                "particle_integrate",
                &self.core_integrate,
                &self.core_group,
            );
            self.encode_boundary_coupling(encoder, "particle_boundary_after_integrate");
            for _ in 0..self.pressure_iterations {
                self.encode_grid_rebuild(encoder);
                self.dispatch_capacity(
                    encoder,
                    "particle_density",
                    &self.density_estimate,
                    &self.density_group,
                );
                self.encode_boundary_density(encoder, "particle_boundary_density");
                self.encode_boundary_pressure_coupling(encoder);
                self.dispatch_capacity(
                    encoder,
                    "particle_correction_gather",
                    &self.correction_gather,
                    &self.correction_group,
                );
                self.dispatch_capacity(
                    encoder,
                    "particle_correction_apply",
                    &self.correction_apply,
                    &self.correction_group,
                );
                self.encode_boundary_coupling(encoder, "particle_boundary_after_correction");
            }
            self.encode_grid_rebuild(encoder);
            self.dispatch_capacity(
                encoder,
                "particle_density_final",
                &self.density_estimate,
                &self.density_group,
            );
            self.encode_boundary_density(encoder, "particle_boundary_density_final");
            self.dispatch_capacity(
                encoder,
                "particle_velocity_reconstruct",
                &self.velocity_reconstruct,
                &self.velocity_group,
            );
            self.dispatch_capacity(
                encoder,
                "particle_body_velocity",
                &self.body_velocity_pipeline,
                &self.body_velocity_group,
            );
            self.encode_body_gather(
                encoder,
                "particle_body_velocity_gather",
                &self.body_velocity_gather_pipeline,
            );
            self.dispatch_capacity(
                encoder,
                "particle_viscosity_gather",
                &self.velocity_gather,
                &self.velocity_group,
            );
            self.dispatch_capacity(
                encoder,
                "particle_viscosity_apply",
                &self.velocity_apply,
                &self.velocity_group,
            );
        }
        self.encode_active_compaction(encoder);
    }

    pub(crate) fn encode_commands(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[ParticleCommand],
    ) -> Result<(), String> {
        if commands.len() > PARTICLE_COMMAND_CAPACITY as usize {
            return Err(format!(
                "particle command capacity {PARTICLE_COMMAND_CAPACITY} exceeded by {} commands",
                commands.len()
            ));
        }
        let mut previous_sequence = None;
        let mut batch = ParticleCommandBatchRaw::zeroed();
        batch.counts[0] = commands.len() as u32;
        let mut has_spawn = false;
        for (index, command) in commands.iter().copied().enumerate() {
            if previous_sequence.is_some_and(|previous| previous >= command.sequence()) {
                return Err("particle command sequences must be strictly increasing".to_owned());
            }
            previous_sequence = Some(command.sequence());
            let raw = command.raw()?;
            has_spawn |= raw.kind_count_material_sequence[0] == 1;
            batch.commands[index] = raw;
        }
        queue.write_buffer(&self.command_batch, 0, bytemuck::bytes_of(&batch));
        if commands.is_empty() {
            return Ok(());
        }
        self.dispatch_capacity(
            encoder,
            "particle_tool_erase",
            &self.tools_erase,
            &self.tools_group,
        );
        if has_spawn {
            self.dispatch(
                encoder,
                "particle_tool_spawn",
                &self.tools_spawn,
                &self.tools_group,
                [1, 1, 1],
            );
        }
        self.encode_active_compaction(encoder);
        Ok(())
    }

    pub(crate) fn encode_reset(&self, encoder: &mut wgpu::CommandEncoder) {
        self.dispatch_capacity(
            encoder,
            "particle_reset",
            &self.reset_pipeline,
            &self.reset_group,
        );
        self.encode_active_compaction(encoder);
    }

    pub(crate) fn encode_render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        camera: CameraFrame2d,
        frame_index: u64,
        density_debug: bool,
    ) {
        queue.write_buffer(
            &self.render_params,
            0,
            bytemuck::bytes_of(&ParticleRenderParamsRaw {
                viewport: [
                    target.size[0] as f32,
                    target.size[1] as f32,
                    target.size[0] as f32 / target.size[1].max(1) as f32,
                    frame_index as f32,
                ],
                camera: [
                    camera.center()[0],
                    camera.center()[1],
                    camera.half_extents()[0],
                    camera.half_extents()[1],
                ],
                material: [
                    self.material.radius.get() * 1.12,
                    self.material.support_radius(),
                    self.material.rest_density.get(),
                    0.0,
                ],
                debug: [u32::from(density_debug), 0, 0, 0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_particle_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(0, &self.render_group, &[]);
        pass.draw(0..6, 0..self.capacity);
    }

    pub(crate) const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub(crate) const fn initial_count(&self) -> u32 {
        self.initial_count
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        u64::from(self.capacity) * (PARTICLE_BYTES_PER_SLOT + 24)
            + u64::from(self.capacity.div_ceil(PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE)) * 4
            + u64::from(self.capacity.div_ceil(PARTICLE_GRID_BLOCK_SPAN))
                * u64::from(PARTICLE_GRID_BLOCK_SPAN)
                * 4
            + u64::from(self.body_capacity) * u64::from(self.body_tile_count) * 16
            + u64::from(GRID_CELL_COUNT) * 8
            + u64::from(self.capacity) * 4
            + size_of::<ParticleCommandBatchRaw>() as u64
            + u64::from(PARTICLE_COUNTER_WORDS) * 4
            + 8 * 4
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        1 + PARTICLE_SUBSTEPS * (4 + self.pressure_iterations * 14 + 12) + 3
    }

    pub(crate) fn capacities(&self) -> Vec<CapacityRecordV1> {
        let mut records = vec![
            capacity(
                "particle-active-compaction-blocks",
                u64::from(self.capacity.div_ceil(PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE)),
                u64::from(self.capacity.div_ceil(PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE)),
                4,
            ),
            capacity(
                "particle-active-prefixes",
                u64::from(self.capacity),
                u64::from(self.initial_count),
                4,
            ),
            capacity(
                "particle-body-contributions",
                u64::from(self.capacity),
                u64::from(self.initial_count),
                16,
            ),
            capacity(
                "particle-body-contact-generations",
                u64::from(self.capacity),
                u64::from(self.initial_count),
                4,
            ),
            capacity(
                "particle-body-partials",
                u64::from(self.body_capacity) * u64::from(self.body_tile_count),
                u64::from(self.body_capacity) * u64::from(self.body_tile_count),
                16,
            ),
            capacity(
                "particle-commands",
                u64::from(PARTICLE_COMMAND_CAPACITY),
                0,
                size_of::<ParticleCommandRaw>() as u64,
            ),
            capacity(
                "particle-grid-cell-ranges",
                u64::from(GRID_CELL_COUNT),
                u64::from(GRID_CELL_COUNT),
                12,
            ),
            capacity(
                "particle-grid-records",
                u64::from(self.capacity),
                u64::from(self.initial_count),
                4,
            ),
            capacity(
                "particle-grid-stable-block-keys",
                u64::from(self.capacity.div_ceil(PARTICLE_GRID_BLOCK_SPAN))
                    * u64::from(PARTICLE_GRID_BLOCK_SPAN),
                u64::from(self.initial_count.div_ceil(PARTICLE_GRID_BLOCK_SPAN))
                    * u64::from(PARTICLE_GRID_BLOCK_SPAN),
                4,
            ),
            capacity(
                "particle-slots",
                u64::from(self.capacity),
                u64::from(self.initial_count),
                PARTICLE_BYTES_PER_SLOT,
            ),
        ];
        records.sort_by(|left, right| left.class.cmp(&right.class));
        records
    }

    pub(crate) const fn validation_layout(&self) -> ParticleValidationLayout {
        let lane_16 = self.capacity as u64 * 16;
        let flags = self.capacity as u64 * 4;
        let density = self.capacity as u64 * 16;
        let counters = PARTICLE_COUNTER_WORDS as u64 * 4;
        let cell_counts = GRID_CELL_COUNT as u64 * 4;
        ParticleValidationLayout {
            capacity: self.capacity,
            positions_offset: 0,
            velocities_offset: lane_16,
            metadata_offset: lane_16 * 2,
            active_flags_offset: lane_16 * 3,
            density_offset: lane_16 * 3 + flags,
            counters_offset: lane_16 * 3 + flags + density,
            cell_counts_offset: lane_16 * 3 + flags + density + counters,
            total_bytes: lane_16 * 3 + flags + density + counters + cell_counts,
        }
    }

    pub(crate) fn copy_validation_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        let layout = self.validation_layout();
        let lane_16 = u64::from(self.capacity) * 16;
        encoder.copy_buffer_to_buffer(
            &self.positions,
            0,
            destination,
            layout.positions_offset,
            lane_16,
        );
        encoder.copy_buffer_to_buffer(
            &self.velocities,
            0,
            destination,
            layout.velocities_offset,
            lane_16,
        );
        encoder.copy_buffer_to_buffer(
            &self.metadata,
            0,
            destination,
            layout.metadata_offset,
            lane_16,
        );
        encoder.copy_buffer_to_buffer(
            &self.active_flags,
            0,
            destination,
            layout.active_flags_offset,
            u64::from(self.capacity) * 4,
        );
        encoder.copy_buffer_to_buffer(
            &self.density_states,
            0,
            destination,
            layout.density_offset,
            lane_16,
        );
        encoder.copy_buffer_to_buffer(
            &self.counters,
            0,
            destination,
            layout.counters_offset,
            u64::from(PARTICLE_COUNTER_WORDS) * 4,
        );
        encoder.copy_buffer_to_buffer(
            &self._cell_counts,
            0,
            destination,
            layout.cell_counts_offset,
            u64::from(GRID_CELL_COUNT) * 4,
        );
    }

    pub(crate) fn decode_validation_state(
        &self,
        bytes: &[u8],
    ) -> Result<GpuParticleDiagnosticsV4, String> {
        let layout = self.validation_layout();
        if bytes.len() != layout.total_bytes as usize {
            return Err(format!(
                "particle validation readback has {} bytes, expected {}",
                bytes.len(),
                layout.total_bytes
            ));
        }
        let lane_16 = layout.capacity as usize * size_of::<[u32; 4]>();
        let positions = bytemuck::try_cast_slice::<u8, [f32; 4]>(
            &bytes[layout.positions_offset as usize..layout.positions_offset as usize + lane_16],
        )
        .map_err(|error| format!("decode particle positions: {error}"))?;
        let velocities = bytemuck::try_cast_slice::<u8, [f32; 4]>(
            &bytes[layout.velocities_offset as usize..layout.velocities_offset as usize + lane_16],
        )
        .map_err(|error| format!("decode particle velocities: {error}"))?;
        let flags_bytes = layout.capacity as usize * size_of::<u32>();
        let active_flags = bytemuck::try_cast_slice::<u8, u32>(
            &bytes[layout.active_flags_offset as usize
                ..layout.active_flags_offset as usize + flags_bytes],
        )
        .map_err(|error| format!("decode particle active flags: {error}"))?;
        let densities = bytemuck::try_cast_slice::<u8, [f32; 4]>(
            &bytes[layout.density_offset as usize..layout.density_offset as usize + lane_16],
        )
        .map_err(|error| format!("decode particle density states: {error}"))?;
        let counters = bytemuck::try_cast_slice::<u8, u32>(
            &bytes[layout.counters_offset as usize..layout.cell_counts_offset as usize],
        )
        .map_err(|error| format!("decode particle counters: {error}"))?;
        if counters.len() != PARTICLE_COUNTER_WORDS as usize {
            return Err("particle validation readback has the wrong counter count".to_owned());
        }
        let cell_counts = bytemuck::try_cast_slice::<u8, u32>(
            &bytes[layout.cell_counts_offset as usize..layout.total_bytes as usize],
        )
        .map_err(|error| format!("decode particle grid-cell counts: {error}"))?;
        if cell_counts.len() != GRID_CELL_COUNT as usize {
            return Err("particle validation readback has the wrong grid-cell count".to_owned());
        }

        let mut active_flags_count = 0u32;
        let mut density_error_squared = 0.0f64;
        let mut motion_squared = 0.0f64;
        let mut state_finite = true;
        for index in 0..layout.capacity as usize {
            if active_flags[index] == 0 {
                continue;
            }
            active_flags_count = active_flags_count.saturating_add(1);
            let position = positions[index];
            let velocity = velocities[index];
            let density = densities[index][0];
            state_finite &= position.into_iter().all(f32::is_finite)
                && velocity.into_iter().all(f32::is_finite)
                && densities[index].into_iter().all(f32::is_finite);
            let density_error = density - self.material.rest_density.get();
            density_error_squared += f64::from(density_error) * f64::from(density_error);
            motion_squared += f64::from(velocity[0]) * f64::from(velocity[0])
                + f64::from(velocity[1]) * f64::from(velocity[1]);
        }
        let divisor = f64::from(active_flags_count.max(1));
        let density_error_rms = (density_error_squared / divisor).sqrt() as f32;
        let fluid_motion_rms = (motion_squared / divisor).sqrt() as f32;
        state_finite &= density_error_rms.is_finite() && fluid_motion_rms.is_finite();
        let capacity_clean = counters[1] == 0
            && counters[3] == 0
            && counters[12] == 0
            && counters[18] == 0
            && counters[0] <= layout.capacity
            && counters[0] == active_flags_count;

        let active = u64::from(counters[0]);
        let capacities = self.capacities();
        Ok(GpuParticleDiagnosticsV4 {
            version: GPU_PARTICLE_DIAGNOSTICS_VERSION,
            work_scope: "final-outer-step",
            capacity: layout.capacity,
            active_count: counters[0],
            active_flags_count,
            allocation_cursor: counters[8],
            overflow_flags: counters[1],
            grid_insertions: counters[2],
            occupied_grid_cells: cell_counts.iter().filter(|count| **count != 0).count() as u32,
            grid_overflow: counters[3],
            neighbor_checks: counters[4],
            accepted_neighbors: counters[5],
            density_evaluations: active
                * u64::from(PARTICLE_SUBSTEPS)
                * u64::from(self.pressure_iterations + 1),
            correction_evaluations: active
                * u64::from(PARTICLE_SUBSTEPS)
                * u64::from(self.pressure_iterations),
            body_contacts: counters[6],
            escaped_particles: counters[7],
            spawned_particles: counters[9],
            erased_particles: counters[10],
            killed_particles: counters[11],
            command_drops: counters[12],
            corrections: counters[13],
            clamped_corrections: counters[14],
            body_contribution_edges: counters[16],
            fluid_wake_roots: counters[17],
            body_contribution_overflow: counters[18],
            boundary_density_contacts: counters[19],
            boundary_pressure_edges: counters[20],
            dispatches_per_step: self.dispatches_per_step(),
            render_instances: counters[0],
            allocated_bytes: self.allocated_bytes(),
            high_water_bytes: capacities
                .iter()
                .map(|capacity| capacity.high_water_bytes)
                .sum(),
            density_error_rms,
            fluid_motion_rms,
            state_finite,
            capacity_clean,
            blocking_readback_for_advancement: false,
        })
    }

    fn encode_grid_rebuild(&self, encoder: &mut wgpu::CommandEncoder) {
        self.dispatch(
            encoder,
            "particle_grid_clear",
            &self.grid_clear,
            &self.grid_group,
            [GRID_CELL_COUNT.div_ceil(PARTICLE_WORKGROUP_SIZE), 1, 1],
        );
        self.dispatch_active(
            encoder,
            "particle_grid_count",
            &self.grid_count,
            &self.grid_group,
        );
        self.dispatch(
            encoder,
            "particle_grid_scan_ranges",
            &self.grid_scan_ranges,
            &self.grid_order_group,
            [1, 1, 1],
        );
        self.dispatch_active(
            encoder,
            "particle_grid_scatter",
            &self.grid_scatter,
            &self.grid_order_group,
        );
    }

    fn encode_boundary_density(&self, encoder: &mut wgpu::CommandEncoder, label: &'static str) {
        self.dispatch_capacity(
            encoder,
            label,
            &self.boundary_density_pipeline,
            &self.boundary_density_group,
        );
    }

    fn encode_boundary_pressure_coupling(&self, encoder: &mut wgpu::CommandEncoder) {
        self.dispatch_capacity(
            encoder,
            "particle_boundary_pressure",
            &self.boundary_pressure_pipeline,
            &self.boundary_pressure_group,
        );
        self.encode_body_gather(
            encoder,
            "particle_body_pressure_gather",
            &self.body_position_gather_pipeline,
        );
    }

    fn encode_boundary_coupling(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        boundary_label: &'static str,
    ) {
        self.dispatch_capacity(
            encoder,
            boundary_label,
            &self.boundary_project,
            &self.boundary_group,
        );
        self.encode_body_gather(
            encoder,
            "particle_body_gather",
            &self.body_position_gather_pipeline,
        );
    }

    fn encode_body_gather(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        gather_label: &'static str,
        gather_pipeline: &wgpu::ComputePipeline,
    ) {
        self.dispatch(
            encoder,
            "particle_body_reduce",
            &self.body_reduce_pipeline,
            &self.body_reduce_group,
            [
                self.body_tile_count,
                self.body_capacity.div_ceil(PARTICLE_WORKGROUP_SIZE),
                1,
            ],
        );
        self.dispatch(
            encoder,
            gather_label,
            gather_pipeline,
            &self.body_gather_group,
            [self.body_capacity.div_ceil(PARTICLE_WORKGROUP_SIZE), 1, 1],
        );
    }

    fn encode_active_compaction(&self, encoder: &mut wgpu::CommandEncoder) {
        let workgroups = self.capacity.div_ceil(PARTICLE_ACTIVE_SCAN_WORKGROUP_SIZE);
        self.dispatch(
            encoder,
            "particle_active_count",
            &self.core_count_active,
            &self.active_group,
            [workgroups, 1, 1],
        );
        self.dispatch(
            encoder,
            "particle_active_scan_blocks",
            &self.core_scan_active_blocks,
            &self.active_group,
            [1, 1, 1],
        );
        self.dispatch(
            encoder,
            "particle_active_scatter",
            &self.core_scatter_active,
            &self.active_group,
            [workgroups, 1, 1],
        );
    }

    fn dispatch_capacity(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        group: &wgpu::BindGroup,
    ) {
        self.dispatch(
            encoder,
            label,
            pipeline,
            group,
            [self.capacity.div_ceil(PARTICLE_WORKGROUP_SIZE), 1, 1],
        );
    }

    fn dispatch_active(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.dispatch_workgroups_indirect(
            &self.indirect,
            4 * size_of::<u32>() as wgpu::BufferAddress,
        );
    }

    fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        group: &wgpu::BindGroup,
        workgroups: [u32; 3],
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
    }
}

#[derive(Debug)]
struct InitialParticleData {
    positions: Vec<[f32; 4]>,
    velocities: Vec<[f32; 4]>,
    metadata: Vec<[u32; 4]>,
    active_flags: Vec<u32>,
    active_indices: Vec<u32>,
    counters: [u32; PARTICLE_COUNTER_WORDS as usize],
    indirect: [u32; 8],
    active_count: u32,
}

fn initial_particle_data(
    regions: &[ParticleRegionV1],
    material: ParticleMaterialV1,
    capacity: u32,
) -> Result<InitialParticleData, String> {
    let mut positions = vec![[0.0; 4]; capacity as usize];
    let mut velocities = vec![[0.0; 4]; capacity as usize];
    let mut metadata = vec![[1, material.id, 0, 0]; capacity as usize];
    let mut active_flags = vec![0; capacity as usize];
    let mut active_indices = vec![0; capacity as usize];
    let mut active_count = 0u32;
    for region in regions {
        region.validate().map_err(|error| error.to_string())?;
        if region.material_id != material.id {
            return Err(format!(
                "particle region {} references unsupported material {}",
                region.id, region.material_id
            ));
        }
        let mut y = region.lower.y + material.radius.get();
        while y <= region.upper.y - material.radius.get() + 1.0e-6 {
            let mut x = region.lower.x + material.radius.get();
            while x <= region.upper.x - material.radius.get() + 1.0e-6 {
                if active_count == capacity {
                    return Err(format!(
                        "authored particle regions overflow capacity {capacity}"
                    ));
                }
                let index = active_count as usize;
                positions[index] = [x, y, x, y];
                velocities[index] = [
                    region.initial_velocity.x,
                    region.initial_velocity.y,
                    0.0,
                    0.0,
                ];
                metadata[index] = [1, material.id, 0, 1];
                active_flags[index] = 1;
                active_indices[index] = active_count;
                active_count += 1;
                x += region.spacing.get();
            }
            y += region.spacing.get();
        }
    }
    if active_count == 0 {
        return Err("authored particle regions produced no particles".to_owned());
    }
    let mut counters = [0; PARTICLE_COUNTER_WORDS as usize];
    counters[0] = active_count;
    counters[8] = active_count;
    Ok(InitialParticleData {
        positions,
        velocities,
        metadata,
        active_flags,
        active_indices,
        counters,
        indirect: [
            6,
            active_count,
            0,
            0,
            active_count.div_ceil(PARTICLE_WORKGROUP_SIZE),
            1,
            1,
            0,
        ],
        active_count,
    })
}

fn circle_spawn_count(radius: f32, spacing: f32) -> Result<u32, String> {
    let approximate = (std::f32::consts::PI * radius * radius / (spacing * spacing)).ceil();
    if !approximate.is_finite() || approximate <= 0.0 || approximate > 4_096.0 {
        return Err("particle emission count is outside 1..=4096".to_owned());
    }
    Ok(approximate as u32)
}

fn layout(
    device: &wgpu::Device,
    label: &'static str,
    storage: &[(u32, bool)],
    uniform_binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries = storage
        .iter()
        .map(|(binding, read_only)| storage_layout_entry(*binding, *read_only, visibility))
        .collect::<Vec<_>>();
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: uniform_binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn storage_layout_entry(
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

fn bind_group(
    device: &wgpu::Device,
    label: &'static str,
    layout: &wgpu::BindGroupLayout,
    buffers: &[&wgpu::Buffer],
) -> wgpu::BindGroup {
    let entries = buffers
        .iter()
        .enumerate()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: binding as u32,
            resource: buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
}

fn shader(device: &wgpu::Device, label: &'static str, source: &'static str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

fn compute(
    device: &wgpu::Device,
    label: &'static str,
    layout: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    entry: &'static str,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some(entry),
        compilation_options: wgpu::PipelineCompilationOptions {
            constants: &[],
            zero_initialize_workgroup_memory: false,
        },
        cache: None,
    })
}

fn storage_init<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    data: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage,
    })
}

fn uniform_init<T: Pod>(device: &wgpu::Device, label: &'static str, value: &T) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(value),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn empty_storage(
    device: &wgpu::Device,
    label: &'static str,
    size: u64,
    additional_usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | additional_usage,
        mapped_at_creation: false,
    })
}

fn ensure_storage_size(device: &wgpu::Device, bytes: u64, label: &str) -> Result<(), String> {
    let limits = device.limits();
    if bytes == 0
        || bytes > limits.max_buffer_size
        || bytes > u64::from(limits.max_storage_buffer_binding_size)
    {
        return Err(format!(
            "{label} requires {bytes} bytes outside portable storage limits"
        ));
    }
    Ok(())
}

fn capacity(
    class: &str,
    capacity_elements: u64,
    used_elements: u64,
    stride: u64,
) -> CapacityRecordV1 {
    CapacityRecordV1 {
        class: class.to_owned(),
        capacity_elements,
        used_elements,
        allocated_bytes: capacity_elements * stride,
        high_water_bytes: used_elements.min(capacity_elements) * stride,
        overflowed: used_elements > capacity_elements,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::{
        BODY_COMMAND_CAPACITY, CoupledScenarioKind, canonical_coupled_scenario,
        liquid_sandbox_scenario,
    };

    #[test]
    fn authored_regions_pack_stably_and_fail_closed() {
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        let material = scenario.materials[0];
        let first =
            initial_particle_data(&scenario.regions, material, scenario.particle_capacity).unwrap();
        let second =
            initial_particle_data(&scenario.regions, material, scenario.particle_capacity).unwrap();
        assert_eq!(first.active_count, second.active_count);
        assert_eq!(first.positions, second.positions);
        assert_eq!(first.metadata, second.metadata);
        assert!(first.active_count > 0);

        let mut too_small = scenario.clone();
        too_small.particle_capacity = 1;
        assert!(
            initial_particle_data(&too_small.regions, material, too_small.particle_capacity)
                .unwrap_err()
                .contains("overflow capacity")
        );
    }

    #[test]
    fn particle_commands_are_bounded_and_sequence_checked() {
        let emit = ParticleCommand::EmitCircle {
            center: Vec2::ZERO,
            radius: 0.5,
            spacing: 0.1,
            initial_velocity: Vec2::ZERO,
            material_id: 0,
            sequence: 1,
        };
        let raw = emit.raw().unwrap();
        assert_eq!(raw.kind_count_material_sequence[0], 1);
        assert!(raw.kind_count_material_sequence[1] > 1);
        assert_eq!(emit.sequence(), 1);
        let erase = ParticleCommand::EraseCircle {
            center: Vec2::ZERO,
            radius: 0.25,
            sequence: 2,
        };
        assert_eq!(erase.raw().unwrap().kind_count_material_sequence[0], 2);
        assert!(circle_spawn_count(100.0, 0.001).is_err());
    }

    #[test]
    fn particle_render_order_is_stable_by_slot() {
        let shader = include_str!("particle_render.wgsl");
        assert!(shader.contains("let index = instance_index;"));
        assert!(shader.contains("if metadata[index].w == 0u"));
        assert!(!shader.contains("active_indices[instance_index]"));
    }

    #[test]
    fn body_identity_float_lane_sentinels_are_webgpu_finite() {
        for bits in [0x7f7f_fffe_u32, 0x7f7f_ffff_u32] {
            assert!(f32::from_bits(bits).is_finite());
            assert!(bits > physbox_launch::WorldCapacityPreset::Slots10000.slots());
        }
        for shader in [
            include_str!("particle_boundary.wgsl"),
            include_str!("particle_boundary_pressure.wgsl"),
            include_str!("particle_body_velocity.wgsl"),
            include_str!("particle_body_reduce.wgsl"),
        ] {
            assert!(shader.contains("0x7f7fffff"));
            assert!(!shader.contains("0xffffffffu"));
        }
    }

    #[test]
    fn gpu_particle_pipelines_compile_and_encode_on_the_product_device_contract() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario = liquid_sandbox_scenario().unwrap();
        let body_capacity = scenario.rigid.bodies.len() as u32;
        let body_storage = GpuBodyStorage::new(
            &device,
            body_capacity,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_pipeline_test"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();
        let validation_error = pollster::block_on(device.pop_error_scope());
        assert!(
            validation_error.is_none(),
            "GPU particle pipeline validation failed: {validation_error:?}"
        );
    }

    #[test]
    fn gpu_particle_killbox_despawns_without_capacity_escape() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario = liquid_sandbox_scenario().unwrap();
        let body_storage = GpuBodyStorage::new(
            &device,
            scenario.rigid.bodies.len() as u32,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        queue.write_buffer(
            &particles.positions,
            0,
            bytemuck::bytes_of(&[0.0, KILL_ZONE_Y - 1.0, 0.0, KILL_ZONE_Y - 1.0]),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_killbox_step"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();

        let bytes = capture_particle_bytes(&device, &queue, &particles);
        let diagnostics = particles.decode_validation_state(&bytes).unwrap();
        assert_eq!(diagnostics.active_count, particles.initial_count - 1);
        assert_eq!(diagnostics.active_flags_count, particles.initial_count - 1);
        assert_eq!(diagnostics.killed_particles, 1);
        assert_eq!(diagnostics.escaped_particles, 0);
        assert_eq!(diagnostics.overflow_flags, 0);
        assert!(diagnostics.capacity_clean);
    }

    #[test]
    fn gpu_particle_reset_spawn_erase_and_reuse_replay_exactly() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = liquid_sandbox_scenario().unwrap();
        scenario.particle_capacity = 3_970;
        let body_storage = GpuBodyStorage::new(
            &device,
            scenario.rigid.bodies.len() as u32,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();
        let first = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let second = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let commands = [
            ParticleCommand::EraseCircle {
                center: Vec2::new(0.0, 2.0),
                radius: 0.35,
                sequence: 1,
            },
            ParticleCommand::EmitCircle {
                center: Vec2::new(0.0, 4.5),
                radius: 0.3,
                spacing: 0.1,
                initial_velocity: Vec2::new(0.5, -0.25),
                material_id: 0,
                sequence: 2,
            },
        ];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_command_replay"),
        });
        first
            .encode_commands(&queue, &mut encoder, &commands)
            .unwrap();
        second
            .encode_commands(&queue, &mut encoder, &commands)
            .unwrap();
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();

        let first_bytes = capture_particle_bytes(&device, &queue, &first);
        let second_bytes = capture_particle_bytes(&device, &queue, &second);
        assert_eq!(first_bytes, second_bytes);
        let layout = first.validation_layout();
        let counter_bytes =
            &first_bytes[layout.counters_offset as usize..layout.cell_counts_offset as usize];
        let counters = bytemuck::cast_slice::<u8, u32>(counter_bytes);
        assert!(counters[9] > 0, "spawn counter");
        assert!(counters[10] > 0, "erase counter");
        assert_eq!(counters[12], 0, "command drops");
        assert_eq!(counters[1], 0, "command overflow flags");
        assert_eq!(
            counters[0],
            first.initial_count() - counters[10] + counters[9]
        );

        let before_reset_metadata =
            particle_u32_lane(&first_bytes, layout.metadata_offset, layout.capacity);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_reset_replay"),
        });
        first.encode_reset(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();
        let reset_bytes = capture_particle_bytes(&device, &queue, &first);
        let reset_metadata =
            particle_u32_lane(&reset_bytes, layout.metadata_offset, layout.capacity);
        let reset_flags =
            particle_u32_scalars(&reset_bytes, layout.active_flags_offset, layout.capacity);
        let initial = initial_particle_data(
            &scenario.regions,
            scenario.materials[0],
            scenario.particle_capacity,
        )
        .unwrap();
        assert_eq!(
            &reset_bytes[layout.positions_offset as usize..layout.velocities_offset as usize],
            bytemuck::cast_slice::<[f32; 4], u8>(&initial.positions)
        );
        assert_eq!(
            &reset_bytes[layout.velocities_offset as usize..layout.metadata_offset as usize],
            bytemuck::cast_slice::<[f32; 4], u8>(&initial.velocities)
        );
        assert_eq!(reset_flags, initial.active_flags);
        for index in 0..layout.capacity as usize {
            assert!(reset_metadata[index][0] > before_reset_metadata[index][0]);
            assert_eq!(reset_metadata[index][1..], initial.metadata[index][1..]);
        }
        let reset_counters = bytemuck::cast_slice::<u8, u32>(
            &reset_bytes[layout.counters_offset as usize..layout.total_bytes as usize],
        );
        assert_eq!(reset_counters[0], first.initial_count());
        assert_eq!(reset_counters[8], first.initial_count());
        assert!(reset_counters[1..8].iter().all(|value| *value == 0));
        assert!(reset_counters[9..].iter().all(|value| *value == 0));
    }

    #[test]
    fn gpu_particle_solver_replays_exactly_with_canonical_work_order() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::Displacement).unwrap();
        let first_bodies = GpuBodyStorage::new(
            &device,
            scenario.rigid.bodies.len() as u32,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();
        let second_bodies = GpuBodyStorage::new(
            &device,
            scenario.rigid.bodies.len() as u32,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();
        let first = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &first_bodies,
        )
        .unwrap();
        let second = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &second_bodies,
        )
        .unwrap();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_canonical_order_replay"),
        });
        for _ in 0..8 {
            first.encode_step(&mut encoder);
            second.encode_step(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();

        assert_eq!(
            capture_particle_bytes(&device, &queue, &first),
            capture_particle_bytes(&device, &queue, &second)
        );
        assert_eq!(
            capture_body_kinematics(&device, &queue, &first_bodies),
            capture_body_kinematics(&device, &queue, &second_bodies)
        );
    }

    #[test]
    fn gpu_particle_integration_matches_the_scalar_contract() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        scenario.particle_capacity = 1;
        scenario.regions = vec![ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower: Vec2::new(0.0, 10.0),
            upper: Vec2::new(0.2, 10.2),
            spacing: physbox_contracts::FiniteF32::new(0.2).unwrap(),
            initial_velocity: Vec2::new(0.35, -0.2),
        }];
        scenario.validate().unwrap();
        let body_storage = GpuBodyStorage::new(
            &device,
            scenario.rigid.bodies.len() as u32,
            BODY_COMMAND_CAPACITY,
            &scenario.rigid.bodies,
        )
        .unwrap();
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_scalar_integration"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();
        let bytes = capture_particle_bytes(&device, &queue, &particles);
        let positions = bytemuck::cast_slice::<u8, [f32; 4]>(&bytes[0..size_of::<[f32; 4]>()]);
        let velocities = bytemuck::cast_slice::<u8, [f32; 4]>(
            &bytes[size_of::<[f32; 4]>()..2 * size_of::<[f32; 4]>()],
        );

        let material = scenario.materials[0];
        let substep_dt = scenario.rigid.settings.fixed_dt.get() / PARTICLE_SUBSTEPS as f32;
        let mut expected = physbox_contracts::ParticleIntegrationOutputV1 {
            position: Vec2::new(0.1, 10.1),
            velocity: scenario.regions[0].initial_velocity,
        };
        for _ in 0..PARTICLE_SUBSTEPS {
            expected = physbox_contracts::integrate_particle_v1(
                physbox_contracts::ParticleIntegrationInputV1 {
                    position: expected.position,
                    velocity: expected.velocity,
                    gravity: scenario.rigid.settings.gravity,
                    gravity_scale: material.gravity_scale.get(),
                    dt: substep_dt,
                },
            )
            .unwrap();
        }
        assert!((positions[0][0] - expected.position.x).abs() <= 1.0e-6);
        assert!((positions[0][1] - expected.position.y).abs() <= 1.0e-6);
        assert!((velocities[0][0] - expected.velocity.x).abs() <= 1.0e-4);
        assert!((velocities[0][1] - expected.velocity.y).abs() <= 1.0e-4);
    }

    #[test]
    fn gpu_particle_body_response_is_same_step_and_equal_opposite() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        let material = scenario.materials[0];
        scenario.particle_capacity = 1;
        scenario.settings.schedule = physbox_contracts::CoupledScheduleV1::RigidThenCoupledBatch;
        scenario.settings.pressure_iterations = 1;
        scenario.rigid.settings.gravity = Vec2::ZERO;
        scenario.rigid.bodies = vec![physbox_contracts::BodyStateV1 {
            id: physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            },
            shape: physbox_contracts::ShapeV1::Circle {
                radius: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            },
            position: Vec2::ZERO,
            angle: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            linear_velocity: Vec2::ZERO,
            angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            inverse_mass: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            inverse_inertia: physbox_contracts::FiniteF32::new(1.0).unwrap(),
            material: physbox_contracts::MaterialV1 {
                friction: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                restitution: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            },
            sleeping: false,
        }];
        scenario.regions = vec![ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower: Vec2::new(0.45, -0.1),
            upper: Vec2::new(0.65, 0.1),
            spacing: physbox_contracts::FiniteF32::new(0.2).unwrap(),
            initial_velocity: Vec2::new(-1.0, 0.0),
        }];
        scenario.validate().unwrap();
        let body_storage =
            GpuBodyStorage::new(&device, 1, BODY_COMMAND_CAPACITY, &scenario.rigid.bodies).unwrap();
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_body_same_step"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();

        let particle_bytes = capture_particle_bytes(&device, &queue, &particles);
        let particle_position = bytemuck::cast_slice::<u8, [f32; 4]>(&particle_bytes[0..16])[0];
        let velocity_offset = particles.validation_layout().velocities_offset as usize;
        let particle_velocity = bytemuck::cast_slice::<u8, [f32; 4]>(
            &particle_bytes[velocity_offset..velocity_offset + 16],
        )[0];
        let diagnostics = particles.decode_validation_state(&particle_bytes).unwrap();
        let (body_pose, body_motion) = capture_body_kinematics(&device, &queue, &body_storage);

        assert!(particle_position[0] > 0.55, "particle moved out of body");
        assert!(body_pose[0] < 0.0, "body received opposite translation");
        assert!(body_motion[0] < 0.0, "body received opposite velocity");
        assert!(
            body_motion[2].abs() <= 1.0e-5,
            "centered load has no torque"
        );
        assert!(diagnostics.body_contribution_edges > 0);
        assert!(diagnostics.fluid_wake_roots > 0);
        assert_eq!(diagnostics.body_contribution_overflow, 0);

        let initial_momentum = -material.particle_mass();
        let final_momentum = material.particle_mass() * particle_velocity[0]
            + scenario.rigid.bodies[0].inverse_mass.get().recip() * body_motion[0];
        assert!(
            (final_momentum - initial_momentum).abs() <= 2.0e-4,
            "linear momentum residual {}",
            final_momentum - initial_momentum
        );
    }

    #[test]
    fn gpu_boundary_pressure_reacts_before_particle_penetration() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        let material = scenario.materials[0];
        scenario.particle_capacity = 1;
        scenario.settings.schedule = physbox_contracts::CoupledScheduleV1::RigidThenCoupledBatch;
        scenario.settings.pressure_iterations = 1;
        scenario.rigid.settings.gravity = Vec2::ZERO;
        scenario.rigid.bodies = vec![physbox_contracts::BodyStateV1 {
            id: physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            },
            shape: physbox_contracts::ShapeV1::Circle {
                radius: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            },
            position: Vec2::ZERO,
            angle: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            linear_velocity: Vec2::ZERO,
            angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            inverse_mass: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            inverse_inertia: physbox_contracts::FiniteF32::new(1.0).unwrap(),
            material: physbox_contracts::MaterialV1 {
                friction: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                restitution: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            },
            sleeping: false,
        }];
        scenario.regions = vec![ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower: Vec2::new(0.5, -0.1),
            upper: Vec2::new(0.7, 0.1),
            spacing: physbox_contracts::FiniteF32::new(0.2).unwrap(),
            initial_velocity: Vec2::ZERO,
        }];
        scenario.validate().unwrap();
        let body_storage =
            GpuBodyStorage::new(&device, 1, BODY_COMMAND_CAPACITY, &scenario.rigid.bodies).unwrap();
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_boundary_pressure"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();

        let particle_bytes = capture_particle_bytes(&device, &queue, &particles);
        let particle_position = bytemuck::cast_slice::<u8, [f32; 4]>(&particle_bytes[0..16])[0];
        let velocity_offset = particles.validation_layout().velocities_offset as usize;
        let particle_velocity = bytemuck::cast_slice::<u8, [f32; 4]>(
            &particle_bytes[velocity_offset..velocity_offset + 16],
        )[0];
        let diagnostics = particles.decode_validation_state(&particle_bytes).unwrap();
        let (body_pose, body_motion) = capture_body_kinematics(&device, &queue, &body_storage);

        assert!(
            particle_position[0] > 0.6,
            "pressure moved particle outward"
        );
        assert!(body_pose[0] < 0.0, "pressure moved body oppositely");
        assert!(diagnostics.boundary_density_contacts > 0);
        assert!(diagnostics.boundary_pressure_edges > 0);
        assert_eq!(diagnostics.body_contribution_overflow, 0);
        let final_momentum = material.particle_mass() * particle_velocity[0]
            + scenario.rigid.bodies[0].inverse_mass.get().recip() * body_motion[0];
        assert!(
            final_momentum.abs() <= 2.0e-4,
            "boundary pressure momentum residual {final_momentum}"
        );
    }

    #[test]
    fn gpu_boundary_pressure_fails_closed_on_two_dynamic_targets() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        scenario.particle_capacity = 1;
        scenario.settings.schedule = physbox_contracts::CoupledScheduleV1::RigidThenCoupledBatch;
        scenario.settings.pressure_iterations = 1;
        scenario.rigid.settings.gravity = Vec2::ZERO;
        let left = physbox_contracts::BodyStateV1 {
            id: physbox_contracts::BodyId {
                slot: 0,
                generation: 1,
            },
            shape: physbox_contracts::ShapeV1::Circle {
                radius: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            },
            position: Vec2::ZERO,
            angle: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            linear_velocity: Vec2::ZERO,
            angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            inverse_mass: physbox_contracts::FiniteF32::new(0.5).unwrap(),
            inverse_inertia: physbox_contracts::FiniteF32::new(1.0).unwrap(),
            material: physbox_contracts::MaterialV1 {
                friction: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                restitution: physbox_contracts::FiniteF32::new(0.0).unwrap(),
            },
            sleeping: false,
        };
        let mut right = left.clone();
        right.id.slot = 1;
        right.position.x = 1.24;
        scenario.rigid.bodies = vec![left, right];
        scenario.regions = vec![ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower: Vec2::new(0.52, -0.1),
            upper: Vec2::new(0.72, 0.1),
            spacing: physbox_contracts::FiniteF32::new(0.2).unwrap(),
            initial_velocity: Vec2::ZERO,
        }];
        scenario.validate().unwrap();
        let body_storage =
            GpuBodyStorage::new(&device, 2, BODY_COMMAND_CAPACITY, &scenario.rigid.bodies).unwrap();
        let particles = GpuParticleWorld::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &scenario,
            &body_storage,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_boundary_pressure_overflow"),
        });
        particles.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::Wait).unwrap();
        let particle_bytes = capture_particle_bytes(&device, &queue, &particles);
        let diagnostics = particles.decode_validation_state(&particle_bytes).unwrap();

        assert!(diagnostics.boundary_density_contacts > 0);
        assert!(diagnostics.body_contribution_overflow > 0);
        assert_ne!(diagnostics.overflow_flags & (1 << 4), 0);
        assert!(!diagnostics.capacity_clean);
    }

    fn capture_particle_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        particles: &GpuParticleWorld,
    ) -> Vec<u8> {
        let layout = particles.validation_layout();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_particle_test_readback"),
            size: layout.total_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_test_copy"),
        });
        particles.copy_validation_state(&mut encoder, &buffer);
        queue.submit([encoder.finish()]);
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

    fn particle_u32_lane(bytes: &[u8], offset: u64, capacity: u32) -> Vec<[u32; 4]> {
        let end = offset as usize + capacity as usize * size_of::<[u32; 4]>();
        bytemuck::cast_slice::<u8, [u32; 4]>(&bytes[offset as usize..end]).to_vec()
    }

    fn particle_u32_scalars(bytes: &[u8], offset: u64, capacity: u32) -> Vec<u32> {
        let end = offset as usize + capacity as usize * size_of::<u32>();
        bytemuck::cast_slice::<u8, u32>(&bytes[offset as usize..end]).to_vec()
    }

    fn capture_body_kinematics(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        storage: &GpuBodyStorage,
    ) -> ([f32; 4], [f32; 4]) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_particle_body_kinematics_readback"),
            size: 32,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_particle_body_kinematics_copy"),
        });
        encoder.copy_buffer_to_buffer(storage.pose_buffer(), 0, &buffer, 0, 16);
        encoder.copy_buffer_to_buffer(storage.motion_buffer(), 0, &buffer, 16, 16);
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        device.poll(wgpu::PollType::Wait).unwrap();
        receiver.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let lanes = bytemuck::cast_slice::<u8, [f32; 4]>(&mapped);
        let result = (lanes[0], lanes[1]);
        drop(mapped);
        buffer.unmap();
        result
    }
}
