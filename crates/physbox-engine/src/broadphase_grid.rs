#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BODY_AABB_BYTES, BROADPHASE_CONTRACT_VERSION, BroadphaseAlgorithmV1, BroadphaseMobilityV1,
    BroadphaseResultV1, BroadphaseSnapshotV1, BroadphaseWorkV1, CANDIDATE_PAIR_BYTES,
    CandidatePairV1, CollisionCapacityV1, FailureKindV1, FailureV1, SCHEMA_VERSION, Validate,
    canonical_hash,
};

use crate::{
    AdapterDiagnostics,
    broadphase_benchmark::GpuBroadphaseTimingV1,
    gpu_context::{create_instance, request_adapter, request_device_with_features},
};

const RADIX_WORKGROUP_SIZE: u32 = 256;
const GENERAL_WORKGROUP_SIZE: u32 = 64;
// Grid coordinates are biased signed 16-bit keys. Snapshot ordinals are below
// 2^20, so canonical pairs require five four-bit digits per endpoint.
const CELL_RADIX_PASSES: u32 = 8;
const PAIR_RADIX_PASSES: u32 = 10;
pub(crate) const GRID_DISPATCHES: u32 = 73;
const BROADPHASE_WORK_BYTES: u32 = 64;
const PARAMETER_BYTES: u32 = 80;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ProxyRaw {
    identity_mobility: [u32; 4],
    lower_upper: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct SortRecordRaw {
    values: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CandidatePairRaw {
    identities: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GridParamsRaw {
    counts0: [u32; 4],
    counts1: [u32; 4],
    counts2: [u32; 4],
    radix: [u32; 4],
    grid: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BroadphaseWorkRaw {
    values: [u32; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CapacityRaw {
    values: [u32; 4],
}

const _: () = assert!(size_of::<ProxyRaw>() == BODY_AABB_BYTES as usize);
const _: () = assert!(size_of::<SortRecordRaw>() == CANDIDATE_PAIR_BYTES as usize);
const _: () = assert!(size_of::<CandidatePairRaw>() == CANDIDATE_PAIR_BYTES as usize);
const _: () = assert!(size_of::<GridParamsRaw>() == PARAMETER_BYTES as usize);
const _: () = assert!(size_of::<BroadphaseWorkRaw>() == BROADPHASE_WORK_BYTES as usize);
const _: () = assert!(size_of::<CapacityRaw>() == 16);

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGridConfigurationV1 {
    pub cell_size: f32,
    pub max_cells_per_fixed: u32,
    pub cell_record_capacity: u32,
    pub global_static_capacity: u32,
    pub pair_emission_capacity: u32,
    pub candidate_capacity: u32,
}

impl GpuGridConfigurationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 {
            return Err("GPU grid cell size must be finite and positive".to_owned());
        }
        if self.max_cells_per_fixed == 0
            || self.cell_record_capacity == 0
            || self.global_static_capacity == 0
            || self.pair_emission_capacity == 0
            || self.candidate_capacity == 0
        {
            return Err("GPU grid thresholds and capacities must be positive".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuMultiCellGridOutput {
    pub adapter: AdapterDiagnostics,
    pub configuration: GpuGridConfigurationV1,
    pub result: BroadphaseResultV1,
}

impl GpuMultiCellGridOutput {
    pub fn validate(&self) -> Result<(), String> {
        self.configuration.validate()?;
        self.result.validate().map_err(|error| error.to_string())?;
        if self.result.algorithm != BroadphaseAlgorithmV1::MultiCellGrid {
            return Err("GPU grid output has the wrong algorithm identity".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuMultiCellGridBenchmarkOutput {
    pub correctness: GpuMultiCellGridOutput,
    pub timing: GpuBroadphaseTimingV1,
}

impl GpuMultiCellGridBenchmarkOutput {
    pub fn validate(&self) -> Result<(), String> {
        self.correctness.validate()?;
        self.timing.validate()
    }
}

pub(crate) struct GridPipelines {
    clear_proxy: wgpu::ComputePipeline,
    classify: wgpu::ComputePipeline,
    scan_blocks: wgpu::ComputePipeline,
    scan_totals: wgpu::ComputePipeline,
    scan_add: wgpu::ComputePipeline,
    finalize_inputs: wgpu::ComputePipeline,
    scatter: wgpu::ComputePipeline,
    cell_histograms: wgpu::ComputePipeline,
    cell_prefix: wgpu::ComputePipeline,
    cell_scatter: wgpu::ComputePipeline,
    enumerate_cells: wgpu::ComputePipeline,
    enumerate_globals: wgpu::ComputePipeline,
    finalize_emissions: wgpu::ComputePipeline,
    pair_histograms: wgpu::ComputePipeline,
    pair_prefix: wgpu::ComputePipeline,
    pair_scatter: wgpu::ComputePipeline,
    mark_unique: wgpu::ComputePipeline,
    scan_unique_blocks: wgpu::ComputePipeline,
    scan_unique_totals: wgpu::ComputePipeline,
    add_unique_offsets: wgpu::ComputePipeline,
    finalize_unique: wgpu::ComputePipeline,
    materialize: wgpu::ComputePipeline,
    clear_proxy_layout: wgpu::BindGroupLayout,
    classify_layout: wgpu::BindGroupLayout,
    scan_layout: wgpu::BindGroupLayout,
    finalize_inputs_layout: wgpu::BindGroupLayout,
    scatter_layout: wgpu::BindGroupLayout,
    cell_radix_layout: wgpu::BindGroupLayout,
    enumerate_cells_layout: wgpu::BindGroupLayout,
    enumerate_globals_layout: wgpu::BindGroupLayout,
    finalize_emissions_layout: wgpu::BindGroupLayout,
    pair_radix_layout: wgpu::BindGroupLayout,
    mark_unique_layout: wgpu::BindGroupLayout,
    unique_scan_layout: wgpu::BindGroupLayout,
    finalize_unique_layout: wgpu::BindGroupLayout,
    materialize_layout: wgpu::BindGroupLayout,
}

impl GridPipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let clear_proxy_layout = layout(
            device,
            "grid_clear_proxy_layout",
            &[
                (4, false),
                (5, false),
                (6, false),
                (7, false),
                (18, false),
                (19, false),
                (21, false),
            ],
        );
        let classify_layout = layout(
            device,
            "grid_classify_layout",
            &[(0, true), (4, false), (6, false)],
        );
        let scan_layout = layout(
            device,
            "grid_scan_layout",
            &[(1, false), (2, false), (3, false)],
        );
        let finalize_inputs_layout = layout(
            device,
            "grid_finalize_inputs_layout",
            &[
                (4, false),
                (5, false),
                (6, false),
                (7, false),
                (18, false),
                (19, false),
                (21, false),
            ],
        );
        let scatter_layout = layout(
            device,
            "grid_scatter_layout",
            &[
                (0, true),
                (5, false),
                (6, false),
                (7, false),
                (8, false),
                (9, false),
                (19, false),
            ],
        );
        let cell_radix_layout = layout(
            device,
            "grid_cell_radix_layout",
            &[(9, false), (10, false), (11, false), (19, false)],
        );
        let enumerate_cells_layout = layout(
            device,
            "grid_enumerate_cells_layout",
            &[(0, true), (9, false), (12, false), (18, false), (19, false)],
        );
        let enumerate_globals_layout = layout(
            device,
            "grid_enumerate_globals_layout",
            &[(0, true), (8, false), (12, false), (18, false), (19, false)],
        );
        let finalize_emissions_layout = layout(
            device,
            "grid_finalize_emissions_layout",
            &[(18, false), (19, false), (21, false)],
        );
        let pair_radix_layout = layout(
            device,
            "grid_pair_radix_layout",
            &[(12, false), (13, false), (14, false), (19, false)],
        );
        let mark_unique_layout = layout(
            device,
            "grid_mark_unique_layout",
            &[(12, false), (15, false), (19, false)],
        );
        let unique_scan_layout = layout(
            device,
            "grid_unique_scan_layout",
            &[(3, false), (15, false), (16, false), (19, false)],
        );
        let finalize_unique_layout = layout(
            device,
            "grid_finalize_unique_layout",
            &[(15, false), (16, false), (18, false), (19, false)],
        );
        let materialize_layout = layout(
            device,
            "grid_materialize_layout",
            &[
                (0, true),
                (12, false),
                (15, false),
                (16, false),
                (17, false),
                (19, false),
            ],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_broadphase_grid_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("broadphase_grid.wgsl").into()),
        });
        let pipeline = |label: &'static str,
                        entry: &'static str,
                        bind_layout: &wgpu::BindGroupLayout| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[bind_layout],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[],
                    zero_initialize_workgroup_memory: false,
                },
                cache: None,
            })
        };
        Self {
            clear_proxy: pipeline(
                "physbox5_grid_clear_proxy_pipeline",
                "clear_grid_proxy_state",
                &clear_proxy_layout,
            ),
            classify: pipeline(
                "physbox5_grid_classify_pipeline",
                "classify_grid_proxies",
                &classify_layout,
            ),
            scan_blocks: pipeline(
                "physbox5_grid_scan_blocks_pipeline",
                "scan_grid_blocks",
                &scan_layout,
            ),
            scan_totals: pipeline(
                "physbox5_grid_scan_totals_pipeline",
                "scan_grid_totals",
                &scan_layout,
            ),
            scan_add: pipeline(
                "physbox5_grid_scan_add_pipeline",
                "add_grid_offsets",
                &scan_layout,
            ),
            finalize_inputs: pipeline(
                "physbox5_grid_finalize_inputs_pipeline",
                "finalize_grid_inputs",
                &finalize_inputs_layout,
            ),
            scatter: pipeline(
                "physbox5_grid_scatter_pipeline",
                "scatter_grid_records",
                &scatter_layout,
            ),
            cell_histograms: pipeline(
                "physbox5_grid_cell_histograms_pipeline",
                "cell_radix_histograms",
                &cell_radix_layout,
            ),
            cell_prefix: pipeline(
                "physbox5_grid_cell_prefix_pipeline",
                "cell_radix_prefix",
                &cell_radix_layout,
            ),
            cell_scatter: pipeline(
                "physbox5_grid_cell_scatter_pipeline",
                "cell_radix_scatter",
                &cell_radix_layout,
            ),
            enumerate_cells: pipeline(
                "physbox5_grid_enumerate_cells_pipeline",
                "enumerate_grid_cells",
                &enumerate_cells_layout,
            ),
            enumerate_globals: pipeline(
                "physbox5_grid_enumerate_globals_pipeline",
                "enumerate_global_statics",
                &enumerate_globals_layout,
            ),
            finalize_emissions: pipeline(
                "physbox5_grid_finalize_emissions_pipeline",
                "finalize_grid_emissions",
                &finalize_emissions_layout,
            ),
            pair_histograms: pipeline(
                "physbox5_grid_pair_histograms_pipeline",
                "pair_radix_histograms",
                &pair_radix_layout,
            ),
            pair_prefix: pipeline(
                "physbox5_grid_pair_prefix_pipeline",
                "pair_radix_prefix",
                &pair_radix_layout,
            ),
            pair_scatter: pipeline(
                "physbox5_grid_pair_scatter_pipeline",
                "pair_radix_scatter",
                &pair_radix_layout,
            ),
            mark_unique: pipeline(
                "physbox5_grid_mark_unique_pipeline",
                "mark_unique_pairs",
                &mark_unique_layout,
            ),
            scan_unique_blocks: pipeline(
                "physbox5_grid_scan_unique_blocks_pipeline",
                "scan_unique_blocks",
                &unique_scan_layout,
            ),
            scan_unique_totals: pipeline(
                "physbox5_grid_scan_unique_totals_pipeline",
                "scan_unique_totals",
                &unique_scan_layout,
            ),
            add_unique_offsets: pipeline(
                "physbox5_grid_add_unique_offsets_pipeline",
                "add_unique_offsets",
                &unique_scan_layout,
            ),
            finalize_unique: pipeline(
                "physbox5_grid_finalize_unique_pipeline",
                "finalize_unique_pairs",
                &finalize_unique_layout,
            ),
            materialize: pipeline(
                "physbox5_grid_materialize_pipeline",
                "materialize_unique_pairs",
                &materialize_layout,
            ),
            clear_proxy_layout,
            classify_layout,
            scan_layout,
            finalize_inputs_layout,
            scatter_layout,
            cell_radix_layout,
            enumerate_cells_layout,
            enumerate_globals_layout,
            finalize_emissions_layout,
            pair_radix_layout,
            mark_unique_layout,
            unique_scan_layout,
            finalize_unique_layout,
            materialize_layout,
        }
    }
}

pub(crate) struct GridCase {
    _input: wgpu::Buffer,
    _cell_counts: wgpu::Buffer,
    _cell_offsets: wgpu::Buffer,
    _global_flags: wgpu::Buffer,
    _global_offsets: wgpu::Buffer,
    _global_statics: wgpu::Buffer,
    _scan_blocks: wgpu::Buffer,
    _cells_a: wgpu::Buffer,
    _cells_b: wgpu::Buffer,
    _cell_radix_blocks: wgpu::Buffer,
    _pairs_a: wgpu::Buffer,
    _pairs_b: wgpu::Buffer,
    _pair_radix_blocks: wgpu::Buffer,
    _unique_flags: wgpu::Buffer,
    _unique_offsets: wgpu::Buffer,
    candidates: wgpu::Buffer,
    work: wgpu::Buffer,
    capacity: wgpu::Buffer,
    indirect: wgpu::Buffer,
    _parameters: Vec<wgpu::Buffer>,
    clear_proxy_group: wgpu::BindGroup,
    classify_group: wgpu::BindGroup,
    cell_scan_group: wgpu::BindGroup,
    global_scan_group: wgpu::BindGroup,
    finalize_inputs_group: wgpu::BindGroup,
    scatter_group: wgpu::BindGroup,
    cell_radix_groups: Vec<wgpu::BindGroup>,
    enumerate_cells_group: wgpu::BindGroup,
    enumerate_globals_group: wgpu::BindGroup,
    finalize_emissions_group: wgpu::BindGroup,
    pair_radix_groups: Vec<wgpu::BindGroup>,
    mark_unique_group: wgpu::BindGroup,
    unique_scan_group: wgpu::BindGroup,
    finalize_unique_group: wgpu::BindGroup,
    materialize_group: wgpu::BindGroup,
    proxy_count: u32,
    proxy_blocks: u32,
    cell_blocks: u32,
    emission_blocks: u32,
    scan_blocks_count: u32,
    configuration: GpuGridConfigurationV1,
}

impl GridCase {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &GridPipelines,
        snapshot: &BroadphaseSnapshotV1,
        configuration: GpuGridConfigurationV1,
    ) -> Result<Self, String> {
        snapshot.validate().map_err(|error| error.to_string())?;
        configuration.validate()?;
        let proxy_count = u32::try_from(snapshot.proxies.len())
            .map_err(|_| "GPU grid proxy count exceeds u32".to_owned())?;
        let proxy_blocks = proxy_count.div_ceil(RADIX_WORKGROUP_SIZE);
        let cell_blocks = configuration
            .cell_record_capacity
            .div_ceil(RADIX_WORKGROUP_SIZE);
        let emission_blocks = configuration
            .pair_emission_capacity
            .div_ceil(RADIX_WORKGROUP_SIZE);
        let scan_blocks_count = proxy_blocks.max(emission_blocks);
        let dynamic_count = snapshot
            .proxies
            .iter()
            .filter(|proxy| proxy.mobility == BroadphaseMobilityV1::Dynamic)
            .count() as u32;
        let fixed_count = proxy_count - dynamic_count;
        let input_values = snapshot
            .proxies
            .iter()
            .enumerate()
            .map(|(ordinal, proxy)| {
                validate_proxy_grid_range(proxy, configuration.cell_size)?;
                Ok(ProxyRaw {
                    identity_mobility: [
                        proxy.aabb.body.slot,
                        proxy.aabb.body.generation,
                        u32::from(proxy.mobility == BroadphaseMobilityV1::Fixed),
                        ordinal as u32,
                    ],
                    lower_upper: [
                        proxy.aabb.lower.x,
                        proxy.aabb.lower.y,
                        proxy.aabb.upper.x,
                        proxy.aabb.upper.y,
                    ],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let input = initialized_storage(device, "grid_input_proxies", &input_values)?;
        let cell_counts = storage::<u32>(
            device,
            "grid_cell_counts",
            proxy_count,
            wgpu::BufferUsages::empty(),
        )?;
        let cell_offsets = storage::<u32>(
            device,
            "grid_cell_offsets",
            proxy_count,
            wgpu::BufferUsages::empty(),
        )?;
        let global_flags = storage::<u32>(
            device,
            "grid_global_flags",
            proxy_count,
            wgpu::BufferUsages::empty(),
        )?;
        let global_offsets = storage::<u32>(
            device,
            "grid_global_offsets",
            proxy_count,
            wgpu::BufferUsages::empty(),
        )?;
        let global_statics = storage::<u32>(
            device,
            "grid_global_statics",
            configuration.global_static_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let scan_blocks = storage::<u32>(
            device,
            "grid_scan_blocks",
            scan_blocks_count,
            wgpu::BufferUsages::empty(),
        )?;
        let cells_a = storage::<SortRecordRaw>(
            device,
            "grid_cell_sort_a",
            configuration.cell_record_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let cells_b = storage::<SortRecordRaw>(
            device,
            "grid_cell_sort_b",
            configuration.cell_record_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let cell_radix_blocks = storage::<u32>(
            device,
            "grid_cell_radix_blocks",
            cell_blocks
                .checked_mul(16)
                .ok_or_else(|| "grid cell radix block count overflowed".to_owned())?,
            wgpu::BufferUsages::empty(),
        )?;
        let pairs_a = storage::<SortRecordRaw>(
            device,
            "grid_pair_sort_a",
            configuration.pair_emission_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let pairs_b = storage::<SortRecordRaw>(
            device,
            "grid_pair_sort_b",
            configuration.pair_emission_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let pair_radix_blocks = storage::<u32>(
            device,
            "grid_pair_radix_blocks",
            emission_blocks
                .checked_mul(16)
                .ok_or_else(|| "grid pair radix block count overflowed".to_owned())?,
            wgpu::BufferUsages::empty(),
        )?;
        let unique_flags = storage::<u32>(
            device,
            "grid_unique_flags",
            configuration.pair_emission_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let unique_offsets = storage::<u32>(
            device,
            "grid_unique_offsets",
            configuration.pair_emission_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let candidates = storage::<CandidatePairRaw>(
            device,
            "grid_candidates",
            configuration.candidate_capacity,
            wgpu::BufferUsages::COPY_SRC,
        )?;
        let work =
            storage::<BroadphaseWorkRaw>(device, "grid_work", 1, wgpu::BufferUsages::COPY_SRC)?;
        let capacity =
            storage::<CapacityRaw>(device, "grid_capacity", 4, wgpu::BufferUsages::COPY_SRC)?;
        let indirect = storage::<u32>(device, "grid_indirect", 15, wgpu::BufferUsages::INDIRECT)?;

        let base = GridParamsRaw {
            counts0: [
                proxy_count,
                proxy_blocks,
                configuration.cell_record_capacity,
                cell_blocks,
            ],
            counts1: [
                configuration.global_static_capacity,
                configuration.pair_emission_capacity,
                emission_blocks,
                configuration.candidate_capacity,
            ],
            counts2: [
                dynamic_count,
                fixed_count,
                configuration.max_cells_per_fixed,
                GRID_DISPATCHES,
            ],
            radix: [0; 4],
            grid: [configuration.cell_size, 0.0, 0.0, 0.0],
        };
        let mut parameters = Vec::with_capacity(35);
        parameters.push(uniform(device, "grid_base_params", &base));
        let clear_proxy_group = group(
            device,
            &pipelines.clear_proxy_layout,
            &[
                (4, &cell_counts),
                (5, &cell_offsets),
                (6, &global_flags),
                (7, &global_offsets),
                (18, &work),
                (19, &capacity),
                (21, &indirect),
            ],
            &parameters[0],
        );
        let classify_group = group(
            device,
            &pipelines.classify_layout,
            &[(0, &input), (4, &cell_counts), (6, &global_flags)],
            &parameters[0],
        );

        let cell_scan_params = scan_parameters(
            device,
            &mut parameters,
            base,
            proxy_count,
            proxy_blocks,
            "grid_cell_scan_params",
        );
        let cell_scan_group = group(
            device,
            &pipelines.scan_layout,
            &[(1, &cell_counts), (2, &cell_offsets), (3, &scan_blocks)],
            cell_scan_params,
        );
        let global_scan_params = scan_parameters(
            device,
            &mut parameters,
            base,
            proxy_count,
            proxy_blocks,
            "grid_global_scan_params",
        );
        let global_scan_group = group(
            device,
            &pipelines.scan_layout,
            &[(1, &global_flags), (2, &global_offsets), (3, &scan_blocks)],
            global_scan_params,
        );
        let finalize_inputs_group = group(
            device,
            &pipelines.finalize_inputs_layout,
            &[
                (4, &cell_counts),
                (5, &cell_offsets),
                (6, &global_flags),
                (7, &global_offsets),
                (18, &work),
                (19, &capacity),
                (21, &indirect),
            ],
            &parameters[0],
        );
        let scatter_group = group(
            device,
            &pipelines.scatter_layout,
            &[
                (0, &input),
                (5, &cell_offsets),
                (6, &global_flags),
                (7, &global_offsets),
                (8, &global_statics),
                (9, &cells_a),
                (19, &capacity),
            ],
            &parameters[0],
        );

        let mut cell_radix_groups = Vec::with_capacity(CELL_RADIX_PASSES as usize);
        for pass in 0..CELL_RADIX_PASSES {
            let mut values = base;
            values.radix[0] = u32::from(pass >= 4);
            values.radix[1] = (pass % 4) * 4;
            parameters.push(uniform(device, "grid_cell_radix_params", &values));
            let (source, destination) = if pass % 2 == 0 {
                (&cells_a, &cells_b)
            } else {
                (&cells_b, &cells_a)
            };
            cell_radix_groups.push(group(
                device,
                &pipelines.cell_radix_layout,
                &[
                    (9, source),
                    (10, destination),
                    (11, &cell_radix_blocks),
                    (19, &capacity),
                ],
                parameters.last().expect("cell radix parameter exists"),
            ));
        }
        let enumerate_cells_group = group(
            device,
            &pipelines.enumerate_cells_layout,
            &[
                (0, &input),
                (9, &cells_a),
                (12, &pairs_a),
                (18, &work),
                (19, &capacity),
            ],
            &parameters[0],
        );
        let enumerate_globals_group = group(
            device,
            &pipelines.enumerate_globals_layout,
            &[
                (0, &input),
                (8, &global_statics),
                (12, &pairs_a),
                (18, &work),
                (19, &capacity),
            ],
            &parameters[0],
        );
        let finalize_emissions_group = group(
            device,
            &pipelines.finalize_emissions_layout,
            &[(18, &work), (19, &capacity), (21, &indirect)],
            &parameters[0],
        );

        let mut pair_radix_groups = Vec::with_capacity(PAIR_RADIX_PASSES as usize);
        for pass in 0..PAIR_RADIX_PASSES {
            let mut values = base;
            values.radix[0] = u32::from(pass >= 5);
            values.radix[1] = (pass % 5) * 4;
            parameters.push(uniform(device, "grid_pair_radix_params", &values));
            let (source, destination) = if pass % 2 == 0 {
                (&pairs_a, &pairs_b)
            } else {
                (&pairs_b, &pairs_a)
            };
            pair_radix_groups.push(group(
                device,
                &pipelines.pair_radix_layout,
                &[
                    (12, source),
                    (13, destination),
                    (14, &pair_radix_blocks),
                    (19, &capacity),
                ],
                parameters.last().expect("pair radix parameter exists"),
            ));
        }
        let mark_unique_group = group(
            device,
            &pipelines.mark_unique_layout,
            &[(12, &pairs_a), (15, &unique_flags), (19, &capacity)],
            &parameters[0],
        );
        let unique_scan_group = group(
            device,
            &pipelines.unique_scan_layout,
            &[
                (3, &scan_blocks),
                (15, &unique_flags),
                (16, &unique_offsets),
                (19, &capacity),
            ],
            &parameters[0],
        );
        let finalize_unique_group = group(
            device,
            &pipelines.finalize_unique_layout,
            &[
                (15, &unique_flags),
                (16, &unique_offsets),
                (18, &work),
                (19, &capacity),
            ],
            &parameters[0],
        );
        let materialize_group = group(
            device,
            &pipelines.materialize_layout,
            &[
                (0, &input),
                (12, &pairs_a),
                (15, &unique_flags),
                (16, &unique_offsets),
                (17, &candidates),
                (19, &capacity),
            ],
            &parameters[0],
        );

        Ok(Self {
            _input: input,
            _cell_counts: cell_counts,
            _cell_offsets: cell_offsets,
            _global_flags: global_flags,
            _global_offsets: global_offsets,
            _global_statics: global_statics,
            _scan_blocks: scan_blocks,
            _cells_a: cells_a,
            _cells_b: cells_b,
            _cell_radix_blocks: cell_radix_blocks,
            _pairs_a: pairs_a,
            _pairs_b: pairs_b,
            _pair_radix_blocks: pair_radix_blocks,
            _unique_flags: unique_flags,
            _unique_offsets: unique_offsets,
            candidates,
            work,
            capacity,
            indirect,
            _parameters: parameters,
            clear_proxy_group,
            classify_group,
            cell_scan_group,
            global_scan_group,
            finalize_inputs_group,
            scatter_group,
            cell_radix_groups,
            enumerate_cells_group,
            enumerate_globals_group,
            finalize_emissions_group,
            pair_radix_groups,
            mark_unique_group,
            unique_scan_group,
            finalize_unique_group,
            materialize_group,
            proxy_count,
            proxy_blocks,
            cell_blocks,
            emission_blocks,
            scan_blocks_count,
            configuration,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &GridPipelines,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        dispatch_pass(
            encoder,
            "physbox5_grid_clear_proxy_pass",
            &pipelines.clear_proxy,
            &self.clear_proxy_group,
            self.proxy_count.max(16).div_ceil(GENERAL_WORKGROUP_SIZE),
            timestamps.map(|(query_set, beginning, _)| (query_set, Some(beginning), None)),
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_classify_pass",
            &pipelines.classify,
            &self.classify_group,
            self.proxy_count.div_ceil(GENERAL_WORKGROUP_SIZE),
            None,
        );
        scan_passes(encoder, pipelines, &self.cell_scan_group, self.proxy_blocks);
        scan_passes(
            encoder,
            pipelines,
            &self.global_scan_group,
            self.proxy_blocks,
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_finalize_inputs_pass",
            &pipelines.finalize_inputs,
            &self.finalize_inputs_group,
            1,
            None,
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_scatter_pass",
            &pipelines.scatter,
            &self.scatter_group,
            self.proxy_count.div_ceil(GENERAL_WORKGROUP_SIZE),
            None,
        );
        for group in &self.cell_radix_groups {
            dispatch_indirect_pass(
                encoder,
                "physbox5_grid_cell_histogram_pass",
                &pipelines.cell_histograms,
                group,
                &self.indirect,
                0,
                None,
            );
            dispatch_pass(
                encoder,
                "physbox5_grid_cell_prefix_pass",
                &pipelines.cell_prefix,
                group,
                1,
                None,
            );
            dispatch_indirect_pass(
                encoder,
                "physbox5_grid_cell_scatter_pass",
                &pipelines.cell_scatter,
                group,
                &self.indirect,
                0,
                None,
            );
        }
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_enumerate_cells_pass",
            &pipelines.enumerate_cells,
            &self.enumerate_cells_group,
            &self.indirect,
            48,
            None,
        );
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_enumerate_globals_pass",
            &pipelines.enumerate_globals,
            &self.enumerate_globals_group,
            &self.indirect,
            36,
            None,
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_finalize_emissions_pass",
            &pipelines.finalize_emissions,
            &self.finalize_emissions_group,
            1,
            None,
        );
        for group in &self.pair_radix_groups {
            dispatch_indirect_pass(
                encoder,
                "physbox5_grid_pair_histogram_pass",
                &pipelines.pair_histograms,
                group,
                &self.indirect,
                12,
                None,
            );
            dispatch_pass(
                encoder,
                "physbox5_grid_pair_prefix_pass",
                &pipelines.pair_prefix,
                group,
                1,
                None,
            );
            dispatch_indirect_pass(
                encoder,
                "physbox5_grid_pair_scatter_pass",
                &pipelines.pair_scatter,
                group,
                &self.indirect,
                12,
                None,
            );
        }
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_mark_unique_pass",
            &pipelines.mark_unique,
            &self.mark_unique_group,
            &self.indirect,
            24,
            None,
        );
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_scan_unique_blocks_pass",
            &pipelines.scan_unique_blocks,
            &self.unique_scan_group,
            &self.indirect,
            12,
            None,
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_scan_unique_totals_pass",
            &pipelines.scan_unique_totals,
            &self.unique_scan_group,
            1,
            None,
        );
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_add_unique_offsets_pass",
            &pipelines.add_unique_offsets,
            &self.unique_scan_group,
            &self.indirect,
            12,
            None,
        );
        dispatch_pass(
            encoder,
            "physbox5_grid_finalize_unique_pass",
            &pipelines.finalize_unique,
            &self.finalize_unique_group,
            1,
            None,
        );
        dispatch_indirect_pass(
            encoder,
            "physbox5_grid_materialize_pass",
            &pipelines.materialize,
            &self.materialize_group,
            &self.indirect,
            24,
            timestamps.map(|(query_set, _, end)| (query_set, None, Some(end))),
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn copy_capacity_to(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(&self.capacity, 0, destination, 0, 64);
    }

    fn capacities(&self, mut dynamic: Vec<CollisionCapacityV1>) -> Vec<CollisionCapacityV1> {
        let fixed = |class: &str, stride: u32, elements: u64| CollisionCapacityV1 {
            class: class.to_owned(),
            stride_bytes: stride,
            capacity_elements: elements,
            attempted_elements: elements,
            committed_elements: elements,
            allocated_bytes: elements * u64::from(stride),
            high_water_bytes: elements * u64::from(stride),
            overflowed: false,
            accepted: true,
        };
        dynamic.extend([
            fixed("capacity-counters", 16, 4),
            fixed("cell-offsets", 4, u64::from(self.proxy_count)),
            fixed("cell-radix-blocks", 4, u64::from(self.cell_blocks) * 16),
            fixed(
                "cell-sort-b",
                CANDIDATE_PAIR_BYTES,
                u64::from(self.configuration.cell_record_capacity),
            ),
            fixed("global-flags", 4, u64::from(self.proxy_count)),
            fixed("global-offsets", 4, u64::from(self.proxy_count)),
            fixed("input-aabbs", BODY_AABB_BYTES, u64::from(self.proxy_count)),
            fixed("indirect-arguments", 4, 15),
            fixed("pair-radix-blocks", 4, u64::from(self.emission_blocks) * 16),
            fixed(
                "pair-sort-b",
                CANDIDATE_PAIR_BYTES,
                u64::from(self.configuration.pair_emission_capacity),
            ),
            fixed("parameters", PARAMETER_BYTES, 35),
            fixed("per-proxy-cell-counts", 4, u64::from(self.proxy_count)),
            fixed("scan-blocks", 4, u64::from(self.scan_blocks_count)),
            fixed(
                "unique-flags",
                4,
                u64::from(self.configuration.pair_emission_capacity),
            ),
            fixed(
                "unique-offsets",
                4,
                u64::from(self.configuration.pair_emission_capacity),
            ),
            fixed("work-counters", BROADPHASE_WORK_BYTES, 1),
        ]);
        dynamic.sort_by(|left, right| left.class.cmp(&right.class));
        dynamic
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuMultiCellGridRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    pipelines: GridPipelines,
    timestamps_available: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuMultiCellGridRunner {
    pub async fn new() -> Result<Self, String> {
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamps_available {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = request_device_with_features(&adapter, required_features).await?;
        let pipelines = GridPipelines::new(&device);
        Ok(Self {
            device,
            queue,
            diagnostics,
            pipelines,
            timestamps_available,
        })
    }

    pub fn adapter(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn run(
        &self,
        snapshot: &BroadphaseSnapshotV1,
        configuration: GpuGridConfigurationV1,
    ) -> Result<GpuMultiCellGridOutput, String> {
        let case = GridCase::new(&self.device, &self.pipelines, snapshot, configuration)?;
        let candidates_readback = readback(
            &self.device,
            "grid_candidates_readback",
            u64::from(configuration.candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        let work_readback = readback(
            &self.device,
            "grid_work_readback",
            u64::from(BROADPHASE_WORK_BYTES),
        );
        let capacity_readback = readback(&self.device, "grid_capacity_readback", 64);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_grid_encoder"),
            });
        case.encode(&mut encoder, &self.pipelines, None);
        encoder.copy_buffer_to_buffer(
            &case.candidates,
            0,
            &candidates_readback,
            0,
            u64::from(configuration.candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &case.work,
            0,
            &work_readback,
            0,
            u64::from(BROADPHASE_WORK_BYTES),
        );
        encoder.copy_buffer_to_buffer(&case.capacity, 0, &capacity_readback, 0, 64);
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU grid poll failed: {error}"))?;

        let raw_capacities = crate::collision_candidates::map_values::<CapacityRaw>(
            &self.device,
            &capacity_readback,
            4,
        )?;
        let work_raw = crate::collision_candidates::map_values::<BroadphaseWorkRaw>(
            &self.device,
            &work_readback,
            1,
        )?[0];
        let candidate_raw = raw_capacities[3].values;
        let pairs_raw = crate::collision_candidates::map_values::<CandidatePairRaw>(
            &self.device,
            &candidates_readback,
            configuration.candidate_capacity as usize,
        )?;
        let pairs = pairs_raw
            .into_iter()
            .take(candidate_raw[2] as usize)
            .map(|pair| CandidatePairV1 {
                body_a: physbox_contracts::BodyId {
                    slot: pair.identities[0],
                    generation: pair.identities[1],
                },
                body_b: physbox_contracts::BodyId {
                    slot: pair.identities[2],
                    generation: pair.identities[3],
                },
            })
            .collect::<Vec<_>>();
        let values = work_raw.values;
        let work = BroadphaseWorkV1 {
            proxies: u64::from(values[0]),
            dynamic_proxies: u64::from(values[1]),
            fixed_proxies: u64::from(values[2]),
            pair_tests: u64::from(values[3]),
            fixed_fixed_rejected: u64::from(values[4]),
            candidate_emissions: u64::from(values[5]),
            duplicate_emissions: u64::from(values[6]),
            unique_candidates: u64::from(values[7]),
            sort_elements: u64::from(values[8]),
            sort_passes: u64::from(values[9]),
            cell_records: u64::from(values[10]),
            global_statics: u64::from(values[11]),
            maximum_active: u64::from(values[12]),
            maximum_cell_occupancy: u64::from(values[13]),
            dispatches: u64::from(values[14]),
        };
        let dynamic_capacities = [
            ("cell-records", CANDIDATE_PAIR_BYTES, raw_capacities[0]),
            ("global-statics", 4, raw_capacities[1]),
            ("pair-emissions", CANDIDATE_PAIR_BYTES, raw_capacities[2]),
            ("candidate-pairs", CANDIDATE_PAIR_BYTES, raw_capacities[3]),
        ]
        .into_iter()
        .map(|(class, stride, raw)| capacity_record(class, stride, raw.values))
        .collect::<Vec<_>>();
        let overflow = dynamic_capacities
            .iter()
            .find(|capacity| capacity.overflowed);
        let source_snapshot_hash = canonical_hash(snapshot).map_err(|error| error.to_string())?;
        let failure = overflow.map(|capacity| FailureV1 {
            kind: FailureKindV1::CapacityOverflow,
            reason: format!(
                "grid {} capacity {} rejected {} attempted elements",
                capacity.class, capacity.capacity_elements, capacity.attempted_elements
            ),
            first_step: Some(0),
            first_stage: Some("broadphase-grid".to_owned()),
            reproducer_hash: Some(source_snapshot_hash.clone()),
        });
        let result = BroadphaseResultV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: BROADPHASE_CONTRACT_VERSION,
            algorithm: BroadphaseAlgorithmV1::MultiCellGrid,
            source_snapshot_hash,
            pairs,
            work,
            capacities: case.capacities(dynamic_capacities),
            failure,
        };
        let output = GpuMultiCellGridOutput {
            adapter: self.diagnostics.clone(),
            configuration,
            result,
        };
        output.validate()?;
        Ok(output)
    }

    pub fn benchmark(
        &self,
        snapshot: &BroadphaseSnapshotV1,
        configuration: GpuGridConfigurationV1,
        warmup_steps: u32,
        sample_steps: u32,
    ) -> Result<GpuMultiCellGridBenchmarkOutput, String> {
        let case = GridCase::new(&self.device, &self.pipelines, snapshot, configuration)?;
        let timing = crate::broadphase_benchmark::benchmark_gpu_case(
            &self.device,
            &self.queue,
            self.timestamps_available,
            warmup_steps,
            sample_steps,
            |encoder, timestamps| case.encode(encoder, &self.pipelines, timestamps),
        )?;
        let output = GpuMultiCellGridBenchmarkOutput {
            correctness: self.run(snapshot, configuration)?,
            timing,
        };
        output.validate()?;
        Ok(output)
    }
}

fn capacity_record(class: &str, stride: u32, raw: [u32; 4]) -> CollisionCapacityV1 {
    CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes: stride,
        capacity_elements: u64::from(raw[0]),
        attempted_elements: u64::from(raw[1]),
        committed_elements: u64::from(raw[2]),
        allocated_bytes: u64::from(raw[0]) * u64::from(stride),
        high_water_bytes: u64::from(raw[0].min(raw[1])) * u64::from(stride),
        overflowed: raw[3] != 0,
        accepted: raw[3] == 0,
    }
}

fn validate_proxy_grid_range(
    proxy: &physbox_contracts::BroadphaseProxyV1,
    cell_size: f32,
) -> Result<(), String> {
    let coordinates = [
        (proxy.aabb.lower.x / cell_size).floor(),
        (proxy.aabb.lower.y / cell_size).floor(),
        (proxy.aabb.upper.x / cell_size).floor(),
        (proxy.aabb.upper.y / cell_size).floor(),
    ];
    if coordinates.iter().any(|coordinate| {
        !coordinate.is_finite() || *coordinate < -32768.0 || *coordinate > 32767.0
    }) {
        return Err(format!(
            "proxy {:?} lies outside the grid's signed 16-bit cell range",
            proxy.aabb.body
        ));
    }
    let width = coordinates[2] as i64 - coordinates[0] as i64 + 1;
    let height = coordinates[3] as i64 - coordinates[1] as i64 + 1;
    let records = width
        .checked_mul(height)
        .ok_or_else(|| "grid proxy cell count overflowed".to_owned())?;
    if records <= 0 || records > i64::from(u32::MAX) {
        return Err(format!(
            "proxy {:?} cell count is outside the GPU grid range",
            proxy.aabb.body
        ));
    }
    Ok(())
}

fn scan_parameters<'a>(
    device: &wgpu::Device,
    parameters: &'a mut Vec<wgpu::Buffer>,
    mut base: GridParamsRaw,
    count: u32,
    blocks: u32,
    label: &'static str,
) -> &'a wgpu::Buffer {
    base.radix[2] = count;
    base.radix[3] = blocks;
    parameters.push(uniform(device, label, &base));
    parameters.last().expect("scan parameter exists")
}

fn scan_passes(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &GridPipelines,
    group: &wgpu::BindGroup,
    blocks: u32,
) {
    dispatch_pass(
        encoder,
        "physbox5_grid_scan_blocks_pass",
        &pipelines.scan_blocks,
        group,
        blocks,
        None,
    );
    dispatch_pass(
        encoder,
        "physbox5_grid_scan_totals_pass",
        &pipelines.scan_totals,
        group,
        1,
        None,
    );
    dispatch_pass(
        encoder,
        "physbox5_grid_scan_add_pass",
        &pipelines.scan_add,
        group,
        blocks,
        None,
    );
}

fn layout(
    device: &wgpu::Device,
    label: &'static str,
    bindings: &[(u32, bool)],
) -> wgpu::BindGroupLayout {
    let mut entries = bindings
        .iter()
        .map(|(binding, read_only)| wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *read_only,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect::<Vec<_>>();
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 20,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<GridParamsRaw>() as u64),
        },
        count: None,
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: &[(u32, &wgpu::Buffer)],
    params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let mut entries = buffers
        .iter()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    entries.push(wgpu::BindGroupEntry {
        binding: 20,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_grid_group"),
        layout,
        entries: &entries,
    })
}

fn initialized_storage<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    values: &[T],
) -> Result<wgpu::Buffer, String> {
    let size = values
        .len()
        .checked_mul(size_of::<T>())
        .ok_or_else(|| format!("{label} byte count overflowed"))? as u64;
    check_storage_size(device, label, size)?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    });
    buffer
        .slice(..)
        .get_mapped_range_mut()
        .copy_from_slice(bytemuck::cast_slice(values));
    buffer.unmap();
    Ok(buffer)
}

fn storage<T>(
    device: &wgpu::Device,
    label: &'static str,
    count: u32,
    extra_usage: wgpu::BufferUsages,
) -> Result<wgpu::Buffer, String> {
    let size = u64::from(count)
        .checked_mul(size_of::<T>() as u64)
        .ok_or_else(|| format!("{label} byte count overflowed"))?;
    check_storage_size(device, label, size)?;
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | extra_usage,
        mapped_at_creation: false,
    }))
}

fn check_storage_size(device: &wgpu::Device, label: &str, size: u64) -> Result<(), String> {
    let limits = device.limits();
    if size == 0
        || size > limits.max_buffer_size
        || size > u64::from(limits.max_storage_buffer_binding_size)
    {
        return Err(format!(
            "{label} requires {size} bytes outside portable storage limits"
        ));
    }
    Ok(())
}

fn uniform(device: &wgpu::Device, label: &'static str, values: &GridParamsRaw) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size_of::<GridParamsRaw>() as u64,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: true,
    });
    buffer
        .slice(..)
        .get_mapped_range_mut()
        .copy_from_slice(bytemuck::bytes_of(values));
    buffer.unmap();
    buffer
}

fn dispatch_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    workgroups: u32,
    timestamps: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: timestamps.map(|(query_set, beginning, end)| {
            wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: beginning,
                end_of_pass_write_index: end,
            }
        }),
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups(workgroups, 1, 1);
}

fn dispatch_indirect_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    indirect: &wgpu::Buffer,
    offset: u64,
    timestamps: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: timestamps.map(|(query_set, beginning, end)| {
            wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: beginning,
                end_of_pass_write_index: end,
            }
        }),
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups_indirect(indirect, offset);
}

#[cfg(not(target_arch = "wasm32"))]
fn readback(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}
