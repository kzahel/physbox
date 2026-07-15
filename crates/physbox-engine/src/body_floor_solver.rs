use std::mem::size_of;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{CapacityRecordV1, ScenarioV1};
#[cfg(not(target_arch = "wasm32"))]
use physbox_contracts::{Validate, canonical_hash, sha256_bytes};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;
use crate::gpu_solver::GpuContributionEvidence;

const WORKGROUP_SIZE: u32 = 64;
const RADIX_WORKGROUP_SIZE: u32 = 256;
const PARAMETER_STRIDE: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuGatherTopology {
    CanonicalBodyScan,
    StableBodyEdge,
}

impl GpuGatherTopology {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CanonicalBodyScan => "canonical-body-scan",
            Self::StableBodyEdge => "stable-body-edge",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGatherTopologyEvidence {
    pub topology: GpuGatherTopology,
    pub edge_build_permuted: bool,
    pub dispatches: u32,
    pub allocated_bytes: u64,
    pub point_capacity: u32,
    pub edge_capacity: u32,
    pub radix_passes: u32,
    pub canonical_scan_attempts: u64,
    pub capacities: Vec<CapacityRecordV1>,
}

impl GpuGatherTopologyEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.point_capacity == 0
            || self.edge_capacity != self.point_capacity.saturating_mul(2)
            || self.allocated_bytes == 0
            || self
                .capacities
                .iter()
                .map(|capacity| capacity.allocated_bytes)
                .sum::<u64>()
                != self.allocated_bytes
        {
            return Err("gather topology has invalid capacity accounting".to_owned());
        }
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("gather topology capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            physbox_contracts::Validate::validate(capacity).map_err(|error| error.to_string())?;
        }
        match self.topology {
            GpuGatherTopology::CanonicalBodyScan => {
                if self.edge_build_permuted
                    || self.dispatches != 4
                    || self.radix_passes != 0
                    || self.canonical_scan_attempts == 0
                {
                    return Err("canonical gather topology has invalid work accounting".to_owned());
                }
            }
            GpuGatherTopology::StableBodyEdge => {
                if self.dispatches != 3 + self.radix_passes * 3
                    || self.radix_passes == 0
                    || self.canonical_scan_attempts != 0
                {
                    return Err("stable body-edge topology has invalid work accounting".to_owned());
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoredGatherBenchmarkOutput {
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub body_count: u32,
    pub contact_points: u32,
    pub warmup_steps: u32,
    pub samples: u32,
    pub topology: GpuGatherTopologyEvidence,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub gpu_total_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub instrumented_index_build_seconds: Option<Vec<f64>>,
    pub instrumented_offset_construction_seconds: Option<Vec<f64>>,
    pub instrumented_gather_seconds: Option<Vec<f64>>,
    pub instrumented_solve_seconds: Option<Vec<f64>>,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub replay_exact: bool,
    pub result_digest: String,
    pub capacities: Vec<CapacityRecordV1>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct PointInputRaw {
    ids: [u32; 4],
    normal: [f32; 2],
    local_anchor_a: [f32; 2],
    local_anchor_b: [f32; 2],
    padding0: [f32; 2],
    material: [f32; 4],
    cache: [f32; 2],
    padding1: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct PreparedPointRaw {
    ids: [u32; 4],
    normal: [f32; 2],
    tangent: [f32; 2],
    anchor_a: [f32; 2],
    anchor_b: [f32; 2],
    data0: [f32; 4],
    data1: [f32; 4],
    data2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ContributionRaw {
    impulse: [f32; 2],
    anchor: [f32; 2],
    body_index: u32,
    point_index: u32,
    side: u32,
    padding: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFixedFloorContactEvidence {
    pub point_index: u32,
    pub separation: f32,
    pub friction: f32,
    pub restitution: f32,
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub total_normal_impulse: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFixedFloorPassEvidence {
    pub contacts: Vec<GpuFixedFloorContactEvidence>,
    pub contributions: Vec<GpuContributionEvidence>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(crate) struct FixedFloorEvidenceLayout {
    pub(crate) point_count: u32,
    pub(crate) prepared_bytes: u64,
    pub(crate) total_bytes: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PassParamsRaw {
    floats0: [f32; 4],
    floats1: [f32; 4],
    floats2: [f32; 4],
    uints0: [u32; 4],
    uints1: [u32; 4],
}

#[derive(Clone, Copy)]
enum PipelineKind {
    TopologyClear,
    TopologyCount,
    TopologyPrefix,
    TopologyScatter,
    EdgeBuild,
    EdgeRadixHistograms,
    EdgeRadixPrefix,
    EdgeRadixScatter,
    EdgeRanges,
    IntegrateVelocity,
    Prepare,
    Refresh,
    Warm,
    Normal,
    Tangent,
    Restitution,
    IntegratePosition,
    Gather,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
enum InstrumentedPhase {
    IndexBuild,
    OffsetConstruction,
    Gather,
    Solve,
}

#[cfg(not(target_arch = "wasm32"))]
impl PipelineKind {
    const fn instrumented_phase(self) -> InstrumentedPhase {
        match self {
            Self::TopologyClear
            | Self::TopologyCount
            | Self::TopologyScatter
            | Self::EdgeBuild
            | Self::EdgeRadixHistograms
            | Self::EdgeRadixScatter => InstrumentedPhase::IndexBuild,
            Self::TopologyPrefix | Self::EdgeRadixPrefix | Self::EdgeRanges => {
                InstrumentedPhase::OffsetConstruction
            }
            Self::Gather => InstrumentedPhase::Gather,
            Self::IntegrateVelocity
            | Self::Prepare
            | Self::Refresh
            | Self::Warm
            | Self::Normal
            | Self::Tangent
            | Self::Restitution
            | Self::IntegratePosition => InstrumentedPhase::Solve,
        }
    }
}

#[derive(Clone, Copy)]
enum MotionDirection {
    AtoB,
    BtoA,
}

struct Operation {
    pipeline: PipelineKind,
    direction: MotionDirection,
    workgroups: u32,
    parameter_index: u32,
}

#[derive(Clone, Copy)]
struct TopologyLayout {
    kind: GpuGatherTopology,
    incident_base: u32,
    cursor_base: u32,
    edge_a_base: u32,
    edge_b_base: u32,
    blocks_base: u32,
    ranges_base: u32,
    edge_count: u32,
    block_count: u32,
    radix_passes: u32,
    sorted_edge_base: u32,
    element_count: u32,
}

impl TopologyLayout {
    fn new(kind: GpuGatherTopology, body_count: u32, point_count: u32) -> Result<Self, String> {
        match kind {
            GpuGatherTopology::CanonicalBodyScan => Self::canonical(body_count, point_count),
            GpuGatherTopology::StableBodyEdge => Self::stable_body_edge(body_count, point_count),
        }
    }

    fn canonical(body_count: u32, point_count: u32) -> Result<Self, String> {
        let incident_base = body_count
            .checked_mul(2)
            .ok_or_else(|| "fixed-floor topology body records overflowed".to_owned())?;
        let cursor_base = incident_base
            .checked_add(
                point_count
                    .checked_mul(2)
                    .ok_or_else(|| "fixed-floor topology incidents overflowed".to_owned())?,
            )
            .ok_or_else(|| "fixed-floor topology incident range overflowed".to_owned())?;
        let element_count = cursor_base
            .checked_add(body_count)
            .ok_or_else(|| "fixed-floor topology cursor range overflowed".to_owned())?;
        Ok(Self {
            kind: GpuGatherTopology::CanonicalBodyScan,
            incident_base,
            cursor_base,
            edge_a_base: 0,
            edge_b_base: 0,
            blocks_base: 0,
            ranges_base: 0,
            edge_count: point_count.saturating_mul(2),
            block_count: 0,
            radix_passes: 0,
            sorted_edge_base: 0,
            element_count,
        })
    }

    fn stable_body_edge(body_count: u32, point_count: u32) -> Result<Self, String> {
        let edge_count = point_count
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge count overflowed".to_owned())?;
        let edge_words = edge_count
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge record range overflowed".to_owned())?;
        let edge_a_base = 0;
        let edge_b_base = edge_words;
        let blocks_base = edge_words
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge ping-pong range overflowed".to_owned())?;
        let block_count = edge_count.div_ceil(RADIX_WORKGROUP_SIZE);
        let block_words = block_count
            .checked_mul(16)
            .ok_or_else(|| "stable body-edge radix blocks overflowed".to_owned())?;
        let ranges_base = blocks_base
            .checked_add(block_words)
            .ok_or_else(|| "stable body-edge block range overflowed".to_owned())?;
        let element_count = ranges_base
            .checked_add(
                body_count
                    .checked_mul(2)
                    .ok_or_else(|| "stable body ranges overflowed".to_owned())?,
            )
            .ok_or_else(|| "stable body-edge topology range overflowed".to_owned())?;
        let body_bits = 32 - body_count.leading_zeros();
        let radix_passes = body_bits.max(1).div_ceil(4);
        let sorted_edge_base = if radix_passes.is_multiple_of(2) {
            edge_a_base
        } else {
            edge_b_base
        };
        Ok(Self {
            kind: GpuGatherTopology::StableBodyEdge,
            incident_base: 0,
            cursor_base: 0,
            edge_a_base,
            edge_b_base,
            blocks_base,
            ranges_base,
            edge_count,
            block_count,
            radix_passes,
            sorted_edge_base,
            element_count,
        })
    }

    const fn dispatches(self) -> u32 {
        match self.kind {
            GpuGatherTopology::CanonicalBodyScan => 4,
            GpuGatherTopology::StableBodyEdge => 3 + self.radix_passes * 3,
        }
    }
}

pub(crate) struct GpuFixedFloorSolver {
    _owned_point_input: Option<wgpu::Buffer>,
    _motion_scratch: wgpu::Buffer,
    _prepared: wgpu::Buffer,
    _contributions: wgpu::Buffer,
    _topology: wgpu::Buffer,
    _parameter_buffer: wgpu::Buffer,
    bind_ab: wgpu::BindGroup,
    bind_ba: wgpu::BindGroup,
    topology_clear_pipeline: wgpu::ComputePipeline,
    topology_count_pipeline: wgpu::ComputePipeline,
    topology_prefix_pipeline: wgpu::ComputePipeline,
    topology_scatter_pipeline: wgpu::ComputePipeline,
    edge_build_pipeline: wgpu::ComputePipeline,
    edge_radix_histograms_pipeline: wgpu::ComputePipeline,
    edge_radix_prefix_pipeline: wgpu::ComputePipeline,
    edge_radix_scatter_pipeline: wgpu::ComputePipeline,
    edge_ranges_pipeline: wgpu::ComputePipeline,
    integrate_velocity_pipeline: wgpu::ComputePipeline,
    prepare_pipeline: wgpu::ComputePipeline,
    refresh_pipeline: wgpu::ComputePipeline,
    warm_pipeline: wgpu::ComputePipeline,
    normal_pipeline: wgpu::ComputePipeline,
    tangent_pipeline: wgpu::ComputePipeline,
    restitution_pipeline: wgpu::ComputePipeline,
    integrate_position_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    operations: Vec<Operation>,
    capacities: Vec<CapacityRecordV1>,
    point_count: u32,
    body_count: u32,
    topology_layout: TopologyLayout,
    edge_build_permuted: bool,
}

impl GpuFixedFloorSolver {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        validate_selected_settings(scenario)?;
        if scenario.manifolds.len() != 1 || scenario.manifolds[0].points.len() != 2 {
            return Err(
                "P5.1 fixed-floor solver requires one authored two-point manifold".to_owned(),
            );
        }
        let (points, point_count) = point_inputs(scenario)?;
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_floor_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            GpuGatherTopology::CanonicalBodyScan,
            false,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_authored_with_topology(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        topology: GpuGatherTopology,
    ) -> Result<Self, String> {
        validate_selected_settings(scenario)?;
        if scenario.manifolds.is_empty() {
            return Err("authored gather benchmark requires contact manifolds".to_owned());
        }
        let (points, point_count) = point_inputs(scenario)?;
        if point_count == 0 {
            return Err("authored gather benchmark requires contact points".to_owned());
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_authored_benchmark_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            topology,
            false,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_generated_with_topology(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_capacity: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
    ) -> Result<Self, String> {
        validate_selected_settings(scenario)?;
        if !scenario.manifolds.is_empty() || point_capacity == 0 {
            return Err(
                "generated contact solver requires a neutral scenario and nonzero point capacity"
                    .to_owned(),
            );
        }
        Self::new_with_point_buffer(
            device,
            storage,
            scenario,
            point_buffer,
            point_capacity,
            topology,
            permute_edge_build,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_point_buffer(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_count: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
    ) -> Result<Self, String> {
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "contact solver body count exceeds u32".to_owned())?;
        let topology_layout = TopologyLayout::new(topology, body_count, point_count)?;
        let (parameters, operations) = operations(
            scenario,
            body_count,
            point_count,
            topology_layout,
            permute_edge_build,
        );
        let initial_motion = scenario
            .bodies
            .iter()
            .map(|body| {
                [
                    body.linear_velocity.x,
                    body.linear_velocity.y,
                    body.angular_velocity.get(),
                    0.0,
                ]
            })
            .collect::<Vec<_>>();
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let motion_scratch = storage_init(
            device,
            "physbox5_floor_motion_scratch",
            bytemuck::cast_slice(&initial_motion),
            usage,
        );
        let prepared = storage_empty::<PreparedPointRaw>(
            device,
            "physbox5_floor_prepared",
            point_count,
            usage,
        );
        let contributions = storage_empty::<ContributionRaw>(
            device,
            "physbox5_floor_contributions",
            point_count * 2,
            usage,
        );
        let topology = storage_empty::<u32>(
            device,
            "physbox5_floor_topology",
            topology_layout.element_count,
            usage,
        );
        let mut parameter_bytes = vec![0_u8; parameters.len() * PARAMETER_STRIDE];
        for (index, parameter) in parameters.iter().enumerate() {
            let start = index * PARAMETER_STRIDE;
            parameter_bytes[start..start + size_of::<PassParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(parameter));
        }
        let parameter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_floor_parameters"),
            contents: &parameter_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_floor_solver_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, true),
                storage_entry(4, true),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(size_of::<PassParamsRaw>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let bind_ab = bind_group(
            device,
            &layout,
            "physbox5_floor_bind_ab",
            storage,
            storage.motion_buffer(),
            &motion_scratch,
            point_buffer,
            &prepared,
            &contributions,
            &topology,
            &parameter_buffer,
        );
        let bind_ba = bind_group(
            device,
            &layout,
            "physbox5_floor_bind_ba",
            storage,
            &motion_scratch,
            storage.motion_buffer(),
            point_buffer,
            &prepared,
            &contributions,
            &topology,
            &parameter_buffer,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_floor_solver_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_floor_solver.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_floor_solver_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |label, entry| {
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
        let mut capacities = vec![
            capacity(
                "body-motion-scratch",
                body_count,
                size_of::<[f32; 4]>() as u64,
            ),
            capacity(
                "contact-points",
                point_count,
                size_of::<PointInputRaw>() as u64,
            ),
            capacity(
                "contact-prepared",
                point_count,
                size_of::<PreparedPointRaw>() as u64,
            ),
            capacity(
                "solver-contributions",
                point_count * 2,
                size_of::<ContributionRaw>() as u64,
            ),
        ];
        capacities.extend(topology_capacities(
            body_count,
            point_count,
            topology_layout,
        ));
        Ok(Self {
            _owned_point_input: None,
            _motion_scratch: motion_scratch,
            _prepared: prepared,
            _contributions: contributions,
            _topology: topology,
            _parameter_buffer: parameter_buffer,
            bind_ab,
            bind_ba,
            topology_clear_pipeline: pipeline("floor_topology_clear", "clear_topology"),
            topology_count_pipeline: pipeline("floor_topology_count", "count_edges"),
            topology_prefix_pipeline: pipeline("floor_topology_prefix", "prefix_edges"),
            topology_scatter_pipeline: pipeline("floor_topology_scatter", "scatter_edges"),
            edge_build_pipeline: pipeline("floor_edge_build", "build_stable_edges"),
            edge_radix_histograms_pipeline: pipeline(
                "floor_edge_radix_histograms",
                "edge_radix_histograms",
            ),
            edge_radix_prefix_pipeline: pipeline("floor_edge_radix_prefix", "edge_radix_prefix"),
            edge_radix_scatter_pipeline: pipeline("floor_edge_radix_scatter", "edge_radix_scatter"),
            edge_ranges_pipeline: pipeline("floor_edge_ranges", "build_edge_ranges"),
            integrate_velocity_pipeline: pipeline("floor_integrate_velocity", "integrate_velocity"),
            prepare_pipeline: pipeline("floor_prepare", "prepare"),
            refresh_pipeline: pipeline("floor_refresh", "refresh_contacts"),
            warm_pipeline: pipeline("floor_warm", "emit_warm"),
            normal_pipeline: pipeline("floor_normal", "emit_normal"),
            tangent_pipeline: pipeline("floor_tangent", "emit_tangent"),
            restitution_pipeline: pipeline("floor_restitution", "emit_restitution"),
            integrate_position_pipeline: pipeline("floor_integrate_position", "integrate_position"),
            gather_pipeline: pipeline("floor_gather", "gather"),
            operations,
            capacities,
            point_count,
            body_count,
            topology_layout,
            edge_build_permuted: permute_edge_build,
        })
    }

    pub(crate) fn encode_step(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_fixed_floor_solve"),
            timestamp_writes: None,
        });
        for operation in &self.operations {
            self.encode_operation(&mut pass, operation);
        }
    }

    fn encode_operation<'a>(&'a self, pass: &mut wgpu::ComputePass<'a>, operation: &Operation) {
        pass.set_pipeline(match operation.pipeline {
            PipelineKind::TopologyClear => &self.topology_clear_pipeline,
            PipelineKind::TopologyCount => &self.topology_count_pipeline,
            PipelineKind::TopologyPrefix => &self.topology_prefix_pipeline,
            PipelineKind::TopologyScatter => &self.topology_scatter_pipeline,
            PipelineKind::EdgeBuild => &self.edge_build_pipeline,
            PipelineKind::EdgeRadixHistograms => &self.edge_radix_histograms_pipeline,
            PipelineKind::EdgeRadixPrefix => &self.edge_radix_prefix_pipeline,
            PipelineKind::EdgeRadixScatter => &self.edge_radix_scatter_pipeline,
            PipelineKind::EdgeRanges => &self.edge_ranges_pipeline,
            PipelineKind::IntegrateVelocity => &self.integrate_velocity_pipeline,
            PipelineKind::Prepare => &self.prepare_pipeline,
            PipelineKind::Refresh => &self.refresh_pipeline,
            PipelineKind::Warm => &self.warm_pipeline,
            PipelineKind::Normal => &self.normal_pipeline,
            PipelineKind::Tangent => &self.tangent_pipeline,
            PipelineKind::Restitution => &self.restitution_pipeline,
            PipelineKind::IntegratePosition => &self.integrate_position_pipeline,
            PipelineKind::Gather => &self.gather_pipeline,
        });
        pass.set_bind_group(
            0,
            match operation.direction {
                MotionDirection::AtoB => &self.bind_ab,
                MotionDirection::BtoA => &self.bind_ba,
            },
            &[operation.parameter_index * PARAMETER_STRIDE as u32],
        );
        pass.dispatch_workgroups(operation.workgroups, 1, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_set: &wgpu::QuerySet,
        beginning: u32,
        end: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_authored_gather_benchmark_total"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(beginning),
                end_of_pass_write_index: Some(end),
            }),
        });
        for operation in &self.operations {
            self.encode_operation(&mut pass, operation);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_instrumented(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_set: &wgpu::QuerySet,
        base: u32,
    ) {
        for (index, operation) in self.operations.iter().enumerate() {
            let beginning = base + index as u32 * 2;
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_authored_gather_benchmark_subphase"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(beginning),
                    end_of_pass_write_index: Some(beginning + 1),
                }),
            });
            self.encode_operation(&mut pass, operation);
        }
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        self.operations.len() as u32
    }

    pub(crate) const fn point_count(&self) -> u32 {
        self.point_count
    }

    pub(crate) fn capacities(&self) -> &[CapacityRecordV1] {
        &self.capacities
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self._owned_point_input
            .as_ref()
            .map_or(0, wgpu::Buffer::size)
            + self._motion_scratch.size()
            + self._prepared.size()
            + self._contributions.size()
            + self._topology.size()
            + self._parameter_buffer.size()
    }

    pub(crate) fn gather_topology_evidence(&self) -> GpuGatherTopologyEvidence {
        GpuGatherTopologyEvidence {
            topology: self.topology_layout.kind,
            edge_build_permuted: self.edge_build_permuted,
            dispatches: self.topology_layout.dispatches(),
            allocated_bytes: self._topology.size(),
            point_capacity: self.point_count,
            edge_capacity: self.topology_layout.edge_count,
            radix_passes: self.topology_layout.radix_passes,
            canonical_scan_attempts: if self.topology_layout.kind
                == GpuGatherTopology::CanonicalBodyScan
            {
                u64::from(self.body_count) * u64::from(self.point_count)
            } else {
                0
            },
            capacities: topology_capacities(
                self.body_count,
                self.point_count,
                self.topology_layout,
            ),
        }
    }

    pub(crate) fn prepared_buffer(&self) -> &wgpu::Buffer {
        &self._prepared
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_evidence_layout(&self) -> FixedFloorEvidenceLayout {
        let prepared_bytes = u64::from(self.point_count) * size_of::<PreparedPointRaw>() as u64;
        let contribution_bytes =
            u64::from(self.point_count * 2) * size_of::<ContributionRaw>() as u64;
        FixedFloorEvidenceLayout {
            point_count: self.point_count,
            prepared_bytes,
            total_bytes: prepared_bytes + contribution_bytes,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_pass_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> FixedFloorEvidenceLayout {
        let layout = self.pass_evidence_layout();
        let prepared_bytes = layout.prepared_bytes;
        let contribution_bytes = layout.total_bytes - prepared_bytes;
        encoder.copy_buffer_to_buffer(&self._prepared, 0, destination, 0, prepared_bytes);
        encoder.copy_buffer_to_buffer(
            &self._contributions,
            0,
            destination,
            prepared_bytes,
            contribution_bytes,
        );
        layout
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_authored_gather_topology(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    topology: GpuGatherTopology,
) -> Result<GpuAuthoredGatherBenchmarkOutput, String> {
    if warmup_steps == 0 || samples == 0 {
        return Err("authored gather benchmark requires nonzero warmup and samples".to_owned());
    }
    scenario.validate().map_err(|error| error.to_string())?;
    let (_, point_count) = point_inputs(scenario)?;
    if point_count == 0 {
        return Err("authored gather benchmark requires contact points".to_owned());
    }
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "authored gather body count exceeds u32".to_owned())?;
    let instance = crate::gpu_context::create_instance();
    let adapter = crate::gpu_context::request_adapter(&instance, None).await?;
    let diagnostics = crate::AdapterDiagnostics::from_adapter(&adapter);
    let timestamps_available = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let required_features = if timestamps_available {
        wgpu::Features::TIMESTAMP_QUERY
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) =
        crate::gpu_context::request_device_with_features(&adapter, required_features).await?;
    let storage = GpuBodyStorage::new(
        &device,
        body_count,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_count, &scenario.bodies)?;
    let solver =
        GpuFixedFloorSolver::new_authored_with_topology(&device, &storage, scenario, topology)?;
    let operation_count = u32::try_from(solver.operations.len())
        .map_err(|_| "authored gather operation count exceeds u32".to_owned())?;
    let total_query_count = samples
        .checked_mul(2)
        .ok_or_else(|| "authored gather total query count overflowed".to_owned())?;
    let instrumented_queries_per_sample = operation_count
        .checked_mul(2)
        .ok_or_else(|| "authored gather subphase query count overflowed".to_owned())?;
    let query_count = total_query_count
        .checked_add(
            samples
                .checked_mul(instrumented_queries_per_sample)
                .ok_or_else(|| "authored gather query count overflowed".to_owned())?,
        )
        .ok_or_else(|| "authored gather query count overflowed".to_owned())?;
    let query_resources = timestamps_available.then(|| {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("physbox5_authored_gather_queries"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });
        let byte_count = u64::from(query_count) * size_of::<u64>() as u64;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_authored_gather_query_resolve"),
            size: byte_count,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_authored_gather_query_readback"),
            size: byte_count,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        (query_set, resolve, readback)
    });

    for _ in 0..warmup_steps {
        storage.upload_reset_data(&queue, &reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather warmup reset: {error}"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_warmup"),
        });
        solver.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather warmup: {error}"))?;
    }

    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        storage.upload_reset_data(&queue, &reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather sample reset: {error}"))?;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_sample"),
        });
        if let Some((query_set, _, _)) = &query_resources {
            solver.encode_step_with_timestamps(&mut encoder, query_set, sample * 2, sample * 2 + 1);
        } else {
            solver.encode_step(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather sample: {error}"))?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }

    if let Some((query_set, _, _)) = &query_resources {
        for sample in 0..samples {
            storage.upload_reset_data(&queue, &reset);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored gather subphase reset: {error}"))?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_authored_gather_subphase_sample"),
            });
            solver.encode_step_instrumented(
                &mut encoder,
                query_set,
                total_query_count + sample * instrumented_queries_per_sample,
            );
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored gather subphase sample: {error}"))?;
        }
    }

    let mut gpu_total_seconds = None;
    let mut index_build_seconds = None;
    let mut offset_construction_seconds = None;
    let mut gather_seconds = None;
    let mut solve_seconds = None;
    let mut timestamp_period_nanoseconds = None;
    if let Some((query_set, resolve, readback)) = &query_resources {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_query_copy"),
        });
        encoder.resolve_query_set(query_set, 0..query_count, resolve, 0);
        encoder.copy_buffer_to_buffer(
            resolve,
            0,
            readback,
            0,
            u64::from(query_count) * size_of::<u64>() as u64,
        );
        queue.submit([encoder.finish()]);
        let values = crate::collision_candidates::map_values::<u64>(
            &device,
            readback,
            query_count as usize,
        )?;
        let period = queue.get_timestamp_period();
        let seconds =
            |start: u64, end: u64| end.saturating_sub(start) as f64 * f64::from(period) * 1.0e-9;
        let total = values[..total_query_count as usize]
            .chunks_exact(2)
            .map(|pair| seconds(pair[0], pair[1]))
            .collect::<Vec<_>>();
        let mut index = Vec::with_capacity(samples as usize);
        let mut offsets = Vec::with_capacity(samples as usize);
        let mut gather = Vec::with_capacity(samples as usize);
        let mut solve = Vec::with_capacity(samples as usize);
        for sample in 0..samples as usize {
            let start =
                total_query_count as usize + sample * instrumented_queries_per_sample as usize;
            let sample_values = &values[start..start + instrumented_queries_per_sample as usize];
            let mut totals = [0.0_f64; 4];
            for (operation, pair) in solver.operations.iter().zip(sample_values.chunks_exact(2)) {
                let phase = match operation.pipeline.instrumented_phase() {
                    InstrumentedPhase::IndexBuild => 0,
                    InstrumentedPhase::OffsetConstruction => 1,
                    InstrumentedPhase::Gather => 2,
                    InstrumentedPhase::Solve => 3,
                };
                totals[phase] += seconds(pair[0], pair[1]);
            }
            index.push(totals[0]);
            offsets.push(totals[1]);
            gather.push(totals[2]);
            solve.push(totals[3]);
        }
        gpu_total_seconds = Some(total);
        index_build_seconds = Some(index);
        offset_construction_seconds = Some(offsets);
        gather_seconds = Some(gather);
        solve_seconds = Some(solve);
        timestamp_period_nanoseconds = Some(period);
    }

    let first_digest = authored_gather_result_digest(&device, &queue, &storage, &reset, &solver)?;
    let second_digest = authored_gather_result_digest(&device, &queue, &storage, &reset, &solver)?;
    let replay_exact = first_digest == second_digest;
    if !replay_exact {
        return Err("authored gather benchmark replay differed".to_owned());
    }
    let mut capacities = vec![storage.body_capacity_record()];
    capacities.extend_from_slice(solver.capacities());
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    for capacity in &capacities {
        capacity.validate().map_err(|error| error.to_string())?;
        if capacity.overflowed {
            return Err(format!(
                "authored gather benchmark overflowed {}",
                capacity.class
            ));
        }
    }
    Ok(GpuAuthoredGatherBenchmarkOutput {
        adapter: diagnostics,
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        body_count,
        contact_points: point_count,
        warmup_steps,
        samples,
        topology: solver.gather_topology_evidence(),
        dispatches_per_step: solver.dispatches_per_step(),
        allocated_bytes: storage.layout().allocated_bytes + solver.allocated_bytes(),
        gpu_total_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        instrumented_index_build_seconds: index_build_seconds,
        instrumented_offset_construction_seconds: offset_construction_seconds,
        instrumented_gather_seconds: gather_seconds,
        instrumented_solve_seconds: solve_seconds,
        timestamp_period_nanoseconds,
        replay_exact,
        result_digest: first_digest,
        capacities,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn authored_gather_result_digest(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    storage: &GpuBodyStorage,
    reset: &crate::body_state::GpuBodyResetData,
    solver: &GpuFixedFloorSolver,
) -> Result<String, String> {
    storage.upload_reset_data(queue, reset);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("poll authored gather digest reset: {error}"))?;
    let lane_bytes = storage.pose_buffer().size();
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_authored_gather_result_readback"),
        size: lane_bytes * 2,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_authored_gather_result_digest"),
    });
    solver.encode_step(&mut encoder);
    encoder.copy_buffer_to_buffer(storage.pose_buffer(), 0, &readback, 0, lane_bytes);
    encoder.copy_buffer_to_buffer(
        storage.motion_buffer(),
        0,
        &readback,
        lane_bytes,
        lane_bytes,
    );
    queue.submit([encoder.finish()]);
    let values = crate::collision_candidates::map_values::<u32>(
        device,
        &readback,
        (lane_bytes * 2 / size_of::<u32>() as u64) as usize,
    )?;
    Ok(sha256_bytes(bytemuck::cast_slice(&values)))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_pass_evidence(
    bytes: &[u8],
    layout: FixedFloorEvidenceLayout,
) -> Result<GpuFixedFloorPassEvidence, String> {
    if bytes.len() != layout.total_bytes as usize {
        return Err("fixed-floor evidence readback had the wrong byte length".to_owned());
    }
    let prepared =
        bytemuck::try_cast_slice::<u8, PreparedPointRaw>(&bytes[..layout.prepared_bytes as usize])
            .map_err(|error| format!("decode fixed-floor prepared points: {error}"))?;
    if prepared.len() != layout.point_count as usize {
        return Err("fixed-floor prepared-point count did not match its layout".to_owned());
    }
    let raw_contributions =
        bytemuck::try_cast_slice::<u8, ContributionRaw>(&bytes[layout.prepared_bytes as usize..])
            .map_err(|error| format!("decode fixed-floor contributions: {error}"))?;
    let contacts = prepared
        .iter()
        .enumerate()
        .map(|(index, point)| GpuFixedFloorContactEvidence {
            point_index: index as u32,
            separation: point.data0[3],
            friction: point.data1[0],
            restitution: point.data1[1],
            normal_impulse: point.data1[2],
            tangent_impulse: point.data1[3],
            total_normal_impulse: point.data2[0],
        })
        .collect();
    let contributions = raw_contributions
        .iter()
        .map(|contribution| GpuContributionEvidence {
            point_index: contribution.point_index,
            body_index: contribution.body_index,
            side: contribution.side,
            impulse: physbox_contracts::Vec2::new(contribution.impulse[0], contribution.impulse[1]),
            anchor: physbox_contracts::Vec2::new(contribution.anchor[0], contribution.anchor[1]),
        })
        .collect();
    Ok(GpuFixedFloorPassEvidence {
        contacts,
        contributions,
    })
}

fn validate_selected_settings(scenario: &ScenarioV1) -> Result<(), String> {
    let settings = &scenario.settings;
    let selected = settings.fixed_dt.get().to_bits() == (1.0_f32 / 60.0).to_bits()
        && settings.substeps == 4
        && settings.iterations == 1
        && settings.bias_rate.get().to_bits() == 0.2_f32.to_bits()
        && settings.mass_coefficient.get().to_bits() == 1.0_f32.to_bits()
        && settings.impulse_coefficient.get().to_bits() == 0.0_f32.to_bits()
        && settings.restitution_threshold.get().to_bits() == 1.0_f32.to_bits();
    if !selected {
        return Err("P5.1 fixed-floor settings differ from the accepted S1.6 baseline".to_owned());
    }
    Ok(())
}

fn point_inputs(scenario: &ScenarioV1) -> Result<(Vec<PointInputRaw>, u32), String> {
    let mut points = Vec::new();
    for manifold in &scenario.manifolds {
        let body_a = scenario
            .bodies
            .binary_search_by_key(&manifold.body_a, |body| body.id)
            .map_err(|_| format!("missing fixed-floor body {:?}", manifold.body_a))?;
        let body_b = scenario
            .bodies
            .binary_search_by_key(&manifold.body_b, |body| body.id)
            .map_err(|_| format!("missing fixed-floor body {:?}", manifold.body_b))?;
        for (point_index, point) in manifold.points.iter().enumerate() {
            points.push(PointInputRaw {
                ids: [
                    body_a as u32,
                    body_b as u32,
                    manifold.id,
                    point_index as u32,
                ],
                normal: [manifold.normal.x, manifold.normal.y],
                local_anchor_a: [point.local_anchor_a.x, point.local_anchor_a.y],
                local_anchor_b: [point.local_anchor_b.x, point.local_anchor_b.y],
                padding0: [0.0; 2],
                material: [
                    point.separation.get(),
                    manifold.material.friction.get(),
                    manifold.material.restitution.get(),
                    0.0,
                ],
                cache: [0.0; 2],
                padding1: [0.0; 2],
            });
        }
    }
    let count = u32::try_from(points.len())
        .map_err(|_| "fixed-floor point count exceeds u32".to_owned())?;
    Ok((points, count))
}

fn operations(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    let mut parameters = Vec::new();
    let mut operations = Vec::new();
    let body_workgroups = workgroups(body_count);
    let point_workgroups = workgroups(point_count);
    topology_operations(
        &mut parameters,
        &mut operations,
        body_count,
        point_count,
        topology,
        permute_edge_build,
    );
    for substep in 0..scenario.settings.substeps {
        push_operation(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 0, 0, 0),
            PipelineKind::IntegrateVelocity,
            MotionDirection::AtoB,
            body_workgroups,
        );
        let mut prepare = pass_params(scenario, body_count, point_count, 0, 0, 0);
        let prepare_kind = if substep == 0 {
            PipelineKind::Prepare
        } else {
            prepare.uints0[3] = 1;
            PipelineKind::Refresh
        };
        push_operation(
            &mut parameters,
            &mut operations,
            prepare,
            prepare_kind,
            MotionDirection::AtoB,
            point_workgroups,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 1, 0, 0),
            PipelineKind::Warm,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 2, 0, 1),
            PipelineKind::Normal,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 3, 0, 0),
            PipelineKind::Tangent,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology,
        );
        push_operation(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 0, 0, 0),
            PipelineKind::IntegratePosition,
            MotionDirection::BtoA,
            body_workgroups,
        );
        push_operation(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 0, 0, 0),
            PipelineKind::Refresh,
            MotionDirection::BtoA,
            point_workgroups,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 4, 0, 0),
            PipelineKind::Normal,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 5, 0, 0),
            PipelineKind::Tangent,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 6, 0, 0),
            PipelineKind::Restitution,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology,
        );
    }
    (parameters, operations)
}

fn topology_operations(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
) {
    let clear = PassParamsRaw {
        floats0: [0.0; 4],
        floats1: [0.0; 4],
        floats2: [0.0; 4],
        uints0: [body_count, point_count, 0, 0],
        uints1: [0, 0, 0, topology.element_count],
    };
    push_operation(
        parameters,
        operations,
        clear,
        PipelineKind::TopologyClear,
        MotionDirection::AtoB,
        workgroups(topology.element_count),
    );
    match topology.kind {
        GpuGatherTopology::CanonicalBodyScan => {
            let mut params = clear;
            params.uints0[2] = topology.incident_base;
            params.uints0[3] = topology.cursor_base;
            for (kind, workgroups) in [
                (PipelineKind::TopologyCount, workgroups(point_count)),
                (PipelineKind::TopologyPrefix, 1),
                (PipelineKind::TopologyScatter, workgroups(body_count)),
            ] {
                push_operation(
                    parameters,
                    operations,
                    params,
                    kind,
                    MotionDirection::AtoB,
                    workgroups,
                );
            }
        }
        GpuGatherTopology::StableBodyEdge => {
            let mut build = clear;
            build.uints0[2] = topology.edge_a_base;
            build.uints1[0] = topology.edge_count;
            build.uints1[1] = u32::from(permute_edge_build);
            push_operation(
                parameters,
                operations,
                build,
                PipelineKind::EdgeBuild,
                MotionDirection::AtoB,
                workgroups(point_count),
            );
            for digit in 0..topology.radix_passes {
                let (source, destination) = if digit % 2 == 0 {
                    (topology.edge_a_base, topology.edge_b_base)
                } else {
                    (topology.edge_b_base, topology.edge_a_base)
                };
                let radix = PassParamsRaw {
                    floats0: [0.0; 4],
                    floats1: [0.0; 4],
                    floats2: [0.0; 4],
                    uints0: [
                        topology.edge_count,
                        topology.block_count,
                        source,
                        destination,
                    ],
                    uints1: [topology.blocks_base, digit * 4, 0, 0],
                };
                for (kind, workgroups) in [
                    (PipelineKind::EdgeRadixHistograms, topology.block_count),
                    (PipelineKind::EdgeRadixPrefix, 1),
                    (PipelineKind::EdgeRadixScatter, topology.block_count),
                ] {
                    push_operation(
                        parameters,
                        operations,
                        radix,
                        kind,
                        MotionDirection::AtoB,
                        workgroups,
                    );
                }
            }
            let ranges = PassParamsRaw {
                floats0: [0.0; 4],
                floats1: [0.0; 4],
                floats2: [0.0; 4],
                uints0: [
                    topology.edge_count,
                    body_count,
                    topology.sorted_edge_base,
                    topology.ranges_base,
                ],
                uints1: [0; 4],
            };
            push_operation(
                parameters,
                operations,
                ranges,
                PipelineKind::EdgeRanges,
                MotionDirection::AtoB,
                workgroups(topology.edge_count),
            );
        }
    }
}

fn pass_params(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    stage: u32,
    iteration: u32,
    use_bias: u32,
) -> PassParamsRaw {
    PassParamsRaw {
        floats0: [
            scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32,
            scenario.settings.bias_rate.get(),
            scenario.settings.mass_coefficient.get(),
            scenario.settings.impulse_coefficient.get(),
        ],
        floats1: [
            scenario.settings.restitution_threshold.get(),
            scenario.settings.gravity.x,
            scenario.settings.gravity.y,
            0.0,
        ],
        floats2: [
            scenario
                .settings
                .contact_policy
                .map_or(0.0, |policy| policy.max_depenetration_velocity.get()),
            0.0,
            0.0,
            0.0,
        ],
        uints0: [body_count, point_count, 0, 0],
        uints1: [iteration, stage, 0, use_bias],
    }
}

#[allow(clippy::too_many_arguments)]
fn jacobi_stage(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: MotionDirection,
    point_workgroups: u32,
    body_workgroups: u32,
    topology: TopologyLayout,
) {
    push_operation(
        parameters,
        operations,
        parameter,
        pipeline,
        direction,
        point_workgroups,
    );
    let mut gather = parameter;
    gather.uints0[2] = u32::from(topology.kind == GpuGatherTopology::StableBodyEdge);
    gather.uints0[3] = topology.ranges_base;
    gather.uints1[2] = topology.sorted_edge_base;
    gather.uints1[3] = topology.incident_base;
    push_operation(
        parameters,
        operations,
        gather,
        PipelineKind::Gather,
        direction,
        body_workgroups,
    );
}

fn push_operation(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: MotionDirection,
    workgroups: u32,
) {
    let parameter_index = parameters.len() as u32;
    parameters.push(parameter);
    operations.push(Operation {
        pipeline,
        direction,
        workgroups,
        parameter_index,
    });
}

fn workgroups(elements: u32) -> u32 {
    elements.div_ceil(WORKGROUP_SIZE)
}

fn topology_capacities(
    body_count: u32,
    _point_count: u32,
    topology: TopologyLayout,
) -> Vec<CapacityRecordV1> {
    match topology.kind {
        GpuGatherTopology::CanonicalBodyScan => vec![capacity(
            "solver-topology",
            topology.element_count,
            size_of::<u32>() as u64,
        )],
        GpuGatherTopology::StableBodyEdge => vec![
            capacity(
                "solver-body-edge-index-buffers",
                topology.edge_count * 2,
                size_of::<[u32; 2]>() as u64,
            ),
            capacity(
                "solver-body-edge-radix-blocks",
                topology.block_count * 16,
                size_of::<u32>() as u64,
            ),
            capacity(
                "solver-body-edge-ranges",
                body_count,
                size_of::<[u32; 2]>() as u64,
            ),
        ],
    }
}

fn capacity(class: &str, elements: u32, stride: u64) -> CapacityRecordV1 {
    let bytes = u64::from(elements) * stride;
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

#[allow(clippy::too_many_arguments)]
fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    storage: &GpuBodyStorage,
    source_motion: &wgpu::Buffer,
    destination_motion: &wgpu::Buffer,
    points: &wgpu::Buffer,
    prepared: &wgpu::Buffer,
    contributions: &wgpu::Buffer,
    topology: &wgpu::Buffer,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: storage.pose_buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: source_motion.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: destination_motion.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: storage.properties_buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: points.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: prepared.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: contributions.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: topology.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: parameters,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<PassParamsRaw>() as u64),
                }),
            },
        ],
    })
}

fn storage_init(
    device: &wgpu::Device,
    label: &'static str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}

fn storage_empty<T>(
    device: &wgpu::Device,
    label: &'static str,
    elements: u32,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: u64::from(elements) * size_of::<T>() as u64,
        usage,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_fixed_floor_schedule_is_exactly_sixty_eight_dispatches() {
        let scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        let topology = TopologyLayout::new(GpuGatherTopology::CanonicalBodyScan, 2, 2).unwrap();
        let (parameters, operations) = operations(&scenario, 2, 2, topology, false);
        assert_eq!(operations.len(), 68);
        assert_eq!(parameters.len(), operations.len());
        assert_eq!(topology.incident_base, 4);
        assert_eq!(topology.cursor_base, 8);
        assert_eq!(topology.element_count, 10);
    }

    #[test]
    fn stable_body_edge_schedule_scales_with_body_key_width() {
        let scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        let topology = TopologyLayout::new(GpuGatherTopology::StableBodyEdge, 2, 2).unwrap();
        let (parameters, operations) = operations(&scenario, 2, 2, topology, true);
        assert_eq!(topology.edge_count, 4);
        assert_eq!(topology.radix_passes, 1);
        assert_eq!(topology.dispatches(), 6);
        assert_eq!(operations.len(), 70);
        assert_eq!(parameters.len(), operations.len());

        let scale = TopologyLayout::new(GpuGatherTopology::StableBodyEdge, 1_024, 512).unwrap();
        assert_eq!(scale.radix_passes, 3);
        assert_eq!(scale.dispatches(), 12);
    }

    #[test]
    fn fixed_floor_rejects_unreviewed_solver_defaults() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        validate_selected_settings(&scenario).unwrap();
        scenario.settings.iterations = 2;
        assert!(validate_selected_settings(&scenario).is_err());
    }
}
