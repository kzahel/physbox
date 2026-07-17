#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::mem::size_of;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, ContactCachePointV1, FeatureIdV1, FiniteF32, ScenarioV1,
    TraceRecordV1, TraceStageV1, TraceValueV1, Validate, Vec2,
};
use serde::Serialize;
use wgpu::util::DeviceExt;

#[cfg(not(target_arch = "wasm32"))]
use crate::gpu_context::{
    create_instance, request_adapter, request_device, request_device_with_features,
};
use crate::{
    body_floor_solver::gpu_contact_softness_coefficients, gpu_context::AdapterDiagnostics,
};

const WORKGROUP_SIZE: u32 = 64;
const PARAMETER_STRIDE: usize = 256;
const MAX_COLORS: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuSolverCandidate {
    JacobiGather,
    ColoredSoftTgs,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum GpuSolverMode {
    ContactPass,
    SoftStep,
    BenchmarkSoftStep,
}

impl GpuSolverCandidate {
    pub const fn name(self) -> &'static str {
        match self {
            Self::JacobiGather => "jacobi-gather",
            Self::ColoredSoftTgs => "colored-soft-tgs",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuContributionEvidence {
    pub point_index: u32,
    pub body_index: u32,
    pub side: u32,
    pub impulse: Vec2,
    pub anchor: Vec2,
}

#[derive(Clone, Debug)]
pub struct GpuSolverOutput {
    pub candidate: GpuSolverCandidate,
    pub adapter: AdapterDiagnostics,
    pub bodies: Vec<BodyStateV1>,
    pub contact_cache: Vec<ContactCachePointV1>,
    pub traces: Vec<TraceRecordV1>,
    pub contributions: Vec<GpuContributionEvidence>,
    pub capacities: Vec<CapacityRecordV1>,
    pub dispatches: u32,
    pub color_count: u32,
    pub contact_colors: Vec<u32>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct GpuSolverBenchmarkOutput {
    pub candidate: GpuSolverCandidate,
    pub adapter: AdapterDiagnostics,
    pub warmup_steps: u32,
    pub samples: u32,
    pub gpu_step_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub capacities: Vec<CapacityRecordV1>,
    pub dispatches: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BodyRaw {
    position: [f32; 2],
    angle: f32,
    inverse_mass: f32,
    linear_velocity: [f32; 2],
    angular_velocity: f32,
    inverse_inertia: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointInputRaw {
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
struct PreparedPointRaw {
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TraceRaw {
    meta: [u32; 4],
    values0: [f32; 4],
    values1: [f32; 4],
    tail: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PassParamsRaw {
    floats0: [f32; 4],
    floats1: [f32; 4],
    floats2: [f32; 4],
    floats3: [f32; 4],
    uints0: [u32; 4],
    uints1: [u32; 4],
}

#[derive(Clone, Copy)]
enum PipelineKind {
    TopologyClear,
    TopologyCount,
    TopologyPrefix,
    TopologyScatter,
    TopologyColor,
    IntegrateVelocity,
    Prepare,
    Refresh,
    Warm,
    Normal,
    Tangent,
    Restitution,
    IntegratePosition,
    Gather,
    Colored,
}

#[derive(Clone, Copy)]
enum BufferDirection {
    AtoB,
    BtoA,
}

struct Operation {
    pipeline: PipelineKind,
    direction: BufferDirection,
    workgroups: u32,
    parameter_index: u32,
}

struct PointIdentity {
    body_a: physbox_contracts::BodyId,
    body_b: physbox_contracts::BodyId,
    feature_a: FeatureIdV1,
    feature_b: FeatureIdV1,
}

#[derive(Clone, Copy)]
struct TopologyLayout {
    incident_base: u32,
    cursor_base: u32,
    color_base: u32,
    color_mask_base: u32,
    status_index: u32,
    color_count_index: u32,
    element_count: u32,
}

impl TopologyLayout {
    fn new(body_count: usize, point_count: usize) -> Result<Self, String> {
        let body_count = u32::try_from(body_count)
            .map_err(|_| "GPU solver body count exceeds u32".to_owned())?;
        let point_count = u32::try_from(point_count)
            .map_err(|_| "GPU solver point count exceeds u32".to_owned())?;
        let incident_base = body_count
            .checked_mul(2)
            .ok_or_else(|| "GPU topology body records overflow u32".to_owned())?;
        let cursor_base = incident_base
            .checked_add(
                point_count
                    .checked_mul(2)
                    .ok_or_else(|| "GPU topology incidents overflow u32".to_owned())?,
            )
            .ok_or_else(|| "GPU topology incident range overflows u32".to_owned())?;
        let color_base = cursor_base
            .checked_add(body_count)
            .ok_or_else(|| "GPU topology cursors overflow u32".to_owned())?;
        let color_mask_base = color_base
            .checked_add(point_count)
            .ok_or_else(|| "GPU topology colors overflow u32".to_owned())?;
        let status_index = color_mask_base
            .checked_add(body_count)
            .ok_or_else(|| "GPU topology masks overflow u32".to_owned())?;
        let color_count_index = status_index
            .checked_add(1)
            .ok_or_else(|| "GPU topology status overflows u32".to_owned())?;
        let element_count = color_count_index
            .checked_add(1)
            .ok_or_else(|| "GPU topology element count overflows u32".to_owned())?;
        Ok(Self {
            incident_base,
            cursor_base,
            color_base,
            color_mask_base,
            status_index,
            color_count_index,
            element_count,
        })
    }
}

pub(crate) struct GpuSolverLab {
    body_a: wgpu::Buffer,
    body_b: wgpu::Buffer,
    prepared: wgpu::Buffer,
    contributions: wgpu::Buffer,
    topology: wgpu::Buffer,
    traces: wgpu::Buffer,
    bind_ab: wgpu::BindGroup,
    bind_ba: wgpu::BindGroup,
    topology_clear_pipeline: wgpu::ComputePipeline,
    topology_count_pipeline: wgpu::ComputePipeline,
    topology_prefix_pipeline: wgpu::ComputePipeline,
    topology_scatter_pipeline: wgpu::ComputePipeline,
    topology_color_pipeline: wgpu::ComputePipeline,
    integrate_velocity_pipeline: wgpu::ComputePipeline,
    prepare_pipeline: wgpu::ComputePipeline,
    refresh_pipeline: wgpu::ComputePipeline,
    warm_pipeline: wgpu::ComputePipeline,
    normal_pipeline: wgpu::ComputePipeline,
    tangent_pipeline: wgpu::ComputePipeline,
    restitution_pipeline: wgpu::ComputePipeline,
    integrate_position_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    colored_pipeline: wgpu::ComputePipeline,
    operations: Vec<Operation>,
    point_identities: Vec<PointIdentity>,
    body_count: usize,
    point_count: usize,
    trace_count: usize,
    topology_layout: TopologyLayout,
    capacity_records: Vec<CapacityRecordV1>,
}

impl GpuSolverLab {
    pub(crate) fn new(
        device: &wgpu::Device,
        scenario: &ScenarioV1,
        candidate: GpuSolverCandidate,
        mode: GpuSolverMode,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        let bodies = scenario.bodies.iter().map(body_raw).collect::<Vec<_>>();
        let (points, point_identities) = point_inputs(scenario)?;
        if points.is_empty() {
            return Err("GPU solver laboratory requires authored contact points".to_owned());
        }
        let topology_layout = TopologyLayout::new(bodies.len(), points.len())?;
        let solve_count = match mode {
            GpuSolverMode::ContactPass => 1,
            GpuSolverMode::SoftStep => scenario.settings.substeps as usize,
            GpuSolverMode::BenchmarkSoftStep => 0,
        };
        let trace_count =
            (points.len() * (5 + 2 * scenario.settings.iterations as usize) * solve_count).max(1);
        let (parameters, operations) = operations(
            candidate,
            scenario,
            points.len(),
            bodies.len(),
            topology_layout,
            mode,
        );

        let body_a = storage_init(
            device,
            "solver_bodies_a",
            bytemuck::cast_slice(&bodies),
            true,
        );
        let body_b = storage_init(
            device,
            "solver_bodies_b",
            bytemuck::cast_slice(&bodies),
            true,
        );
        let point_buffer = storage_init(
            device,
            "solver_point_inputs",
            bytemuck::cast_slice(&points),
            false,
        );
        let prepared =
            storage_empty::<PreparedPointRaw>(device, "solver_prepared_points", points.len(), true);
        let contributions = storage_empty::<ContributionRaw>(
            device,
            "solver_contributions",
            points.len() * 2,
            true,
        );
        let topology = storage_empty::<u32>(
            device,
            "solver_topology",
            topology_layout.element_count as usize,
            true,
        );
        let traces = storage_empty::<TraceRaw>(device, "solver_traces", trace_count, true);
        let mut parameter_bytes = vec![0_u8; parameters.len() * PARAMETER_STRIDE];
        for (index, parameter) in parameters.iter().enumerate() {
            let start = index * PARAMETER_STRIDE;
            parameter_bytes[start..start + size_of::<PassParamsRaw>()]
                .copy_from_slice(bytemuck::bytes_of(parameter));
        }
        let parameter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("solver_pass_parameters"),
            contents: &parameter_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("solver_bind_group_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, false),
                storage_entry(2, true),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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
            "solver_bind_ab",
            &body_a,
            &body_b,
            &point_buffer,
            &prepared,
            &contributions,
            &topology,
            &traces,
            &parameter_buffer,
        );
        let bind_ba = bind_group(
            device,
            &layout,
            "solver_bind_ba",
            &body_b,
            &body_a,
            &point_buffer,
            &prepared,
            &contributions,
            &topology,
            &traces,
            &parameter_buffer,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("solver_candidate_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_solver.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("solver_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |label: &'static str, entry: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let capacity_records = capacities(
            bodies.len(),
            points.len(),
            operations.len(),
            topology_layout.element_count as usize,
            trace_count,
        );
        Ok(Self {
            body_a,
            body_b,
            prepared,
            contributions,
            topology,
            traces,
            bind_ab,
            bind_ba,
            topology_clear_pipeline: pipeline("solver_topology_clear", "clear_topology"),
            topology_count_pipeline: pipeline("solver_topology_count", "count_edges"),
            topology_prefix_pipeline: pipeline("solver_topology_prefix", "prefix_edges"),
            topology_scatter_pipeline: pipeline("solver_topology_scatter", "scatter_edges"),
            topology_color_pipeline: pipeline("solver_topology_color", "color_contacts"),
            integrate_velocity_pipeline: pipeline(
                "solver_integrate_velocity",
                "integrate_velocity",
            ),
            prepare_pipeline: pipeline("solver_prepare", "prepare"),
            refresh_pipeline: pipeline("solver_refresh", "refresh_contacts"),
            warm_pipeline: pipeline("solver_warm", "emit_warm"),
            normal_pipeline: pipeline("solver_normal", "emit_normal"),
            tangent_pipeline: pipeline("solver_tangent", "emit_tangent"),
            restitution_pipeline: pipeline("solver_restitution", "emit_restitution"),
            integrate_position_pipeline: pipeline(
                "solver_integrate_position",
                "integrate_position",
            ),
            gather_pipeline: pipeline("solver_gather", "gather"),
            colored_pipeline: pipeline("solver_colored", "colored_stage"),
            operations,
            point_identities,
            body_count: bodies.len(),
            point_count: points.len(),
            trace_count,
            topology_layout,
            capacity_records,
        })
    }

    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        self.encode_with_timestamps(encoder, None);
    }

    fn encode_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("solver_candidate_passes"),
            timestamp_writes: timestamps.map(|(query_set, beginning, end)| {
                wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(beginning),
                    end_of_pass_write_index: Some(end),
                }
            }),
        });
        for operation in &self.operations {
            pass.set_pipeline(match operation.pipeline {
                PipelineKind::TopologyClear => &self.topology_clear_pipeline,
                PipelineKind::TopologyCount => &self.topology_count_pipeline,
                PipelineKind::TopologyPrefix => &self.topology_prefix_pipeline,
                PipelineKind::TopologyScatter => &self.topology_scatter_pipeline,
                PipelineKind::TopologyColor => &self.topology_color_pipeline,
                PipelineKind::IntegrateVelocity => &self.integrate_velocity_pipeline,
                PipelineKind::Prepare => &self.prepare_pipeline,
                PipelineKind::Refresh => &self.refresh_pipeline,
                PipelineKind::Warm => &self.warm_pipeline,
                PipelineKind::Normal => &self.normal_pipeline,
                PipelineKind::Tangent => &self.tangent_pipeline,
                PipelineKind::Restitution => &self.restitution_pipeline,
                PipelineKind::IntegratePosition => &self.integrate_position_pipeline,
                PipelineKind::Gather => &self.gather_pipeline,
                PipelineKind::Colored => &self.colored_pipeline,
            });
            let bind_group = match operation.direction {
                BufferDirection::AtoB => &self.bind_ab,
                BufferDirection::BtoA => &self.bind_ba,
            };
            pass.set_bind_group(
                0,
                bind_group,
                &[operation.parameter_index * PARAMETER_STRIDE as u32],
            );
            pass.dispatch_workgroups(operation.workgroups, 1, 1);
        }
    }

    pub(crate) fn body_buffer(&self) -> &wgpu::Buffer {
        &self.body_a
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reset_bodies(&self, queue: &wgpu::Queue, bodies: &[BodyRaw]) {
        let bytes = bytemuck::cast_slice(bodies);
        queue.write_buffer(&self.body_a, 0, bytes);
        queue.write_buffer(&self.body_b, 0, bytes);
    }

    fn copy_readback(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) -> Readback {
        let body_bytes = self.body_count * size_of::<BodyRaw>();
        let prepared_bytes = self.point_count * size_of::<PreparedPointRaw>();
        let contribution_bytes = self.point_count * 2 * size_of::<ContributionRaw>();
        let trace_bytes = self.trace_count * size_of::<TraceRaw>();
        let topology_bytes = self.topology_layout.element_count as usize * size_of::<u32>();
        let prepared_offset = body_bytes;
        let contribution_offset = prepared_offset + prepared_bytes;
        let trace_offset = contribution_offset + contribution_bytes;
        let topology_offset = trace_offset + trace_bytes;
        let total = topology_offset + topology_bytes;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("solver_readback"),
            size: total as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&self.body_a, 0, &buffer, 0, body_bytes as u64);
        encoder.copy_buffer_to_buffer(
            &self.prepared,
            0,
            &buffer,
            prepared_offset as u64,
            prepared_bytes as u64,
        );
        encoder.copy_buffer_to_buffer(
            &self.contributions,
            0,
            &buffer,
            contribution_offset as u64,
            contribution_bytes as u64,
        );
        encoder.copy_buffer_to_buffer(
            &self.traces,
            0,
            &buffer,
            trace_offset as u64,
            trace_bytes as u64,
        );
        encoder.copy_buffer_to_buffer(
            &self.topology,
            0,
            &buffer,
            topology_offset as u64,
            topology_bytes as u64,
        );
        Readback {
            buffer,
            prepared_offset,
            contribution_offset,
            trace_offset,
            topology_offset,
            total,
        }
    }
}

struct Readback {
    buffer: wgpu::Buffer,
    prepared_offset: usize,
    contribution_offset: usize,
    trace_offset: usize,
    topology_offset: usize,
    total: usize,
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_solver_case(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
) -> Result<GpuSolverOutput, String> {
    run_gpu_solver_mode(scenario, candidate, GpuSolverMode::ContactPass).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_soft_step_case(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
) -> Result<GpuSolverOutput, String> {
    run_gpu_solver_mode(scenario, candidate, GpuSolverMode::SoftStep).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_soft_step_stress_case(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
) -> Result<GpuSolverOutput, String> {
    run_gpu_solver_mode(scenario, candidate, GpuSolverMode::BenchmarkSoftStep).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_soft_step_case(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuSolverBenchmarkOutput, String> {
    if warmup_steps == 0 || samples == 0 {
        return Err("GPU solver benchmark requires non-zero warmup and samples".to_owned());
    }
    let query_count = samples
        .checked_mul(2)
        .ok_or_else(|| "GPU solver benchmark query count overflow".to_owned())?;
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
    let lab = GpuSolverLab::new(
        &device,
        scenario,
        candidate,
        GpuSolverMode::BenchmarkSoftStep,
    )?;
    let body_raw = scenario.bodies.iter().map(body_raw).collect::<Vec<_>>();
    let query_resources = timestamps_available.then(|| {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("solver_benchmark_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });
        let byte_count = u64::from(query_count) * size_of::<u64>() as u64;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("solver_benchmark_timestamp_resolve"),
            size: byte_count,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("solver_benchmark_timestamp_readback"),
            size: byte_count,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        (query_set, resolve, readback)
    });

    for _ in 0..warmup_steps {
        lab.reset_bodies(&queue, &body_raw);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll GPU solver warmup reset: {error}"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("solver_benchmark_warmup"),
        });
        lab.encode(&mut encoder);
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll GPU solver warmup: {error}"))?;
    }

    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        lab.reset_bodies(&queue, &body_raw);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll GPU solver sample reset: {error}"))?;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("solver_benchmark_sample"),
        });
        let timestamps = query_resources
            .as_ref()
            .map(|(query_set, _, _)| (query_set, sample * 2, sample * 2 + 1));
        lab.encode_with_timestamps(&mut encoder, timestamps);
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll GPU solver benchmark sample: {error}"))?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }

    let (gpu_step_seconds, timestamp_period_nanoseconds) =
        if let Some((query_set, resolve, readback)) = &query_resources {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("solver_benchmark_timestamp_copy"),
            });
            encoder.resolve_query_set(query_set, 0..query_count, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, u64::from(query_count) * 8);
            queue.submit([encoder.finish()]);
            let values = map_timestamp_readback(&device, readback, query_count as usize)?;
            let period = queue.get_timestamp_period();
            let seconds = values
                .chunks_exact(2)
                .map(|pair| pair[1].saturating_sub(pair[0]) as f64 * f64::from(period) * 1.0e-9)
                .collect();
            (Some(seconds), Some(period))
        } else {
            (None, None)
        };

    Ok(GpuSolverBenchmarkOutput {
        candidate,
        adapter: diagnostics,
        warmup_steps,
        samples,
        gpu_step_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        timestamp_period_nanoseconds,
        capacities: lab.capacity_records,
        dispatches: lab.operations.len() as u32,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn map_timestamp_readback(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    count: usize,
) -> Result<Vec<u64>, String> {
    let slice = buffer.slice(..);
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("poll GPU timestamp readback: {error}"))?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("map GPU timestamp readback: {error}"))?;
    let mapped = slice.get_mapped_range();
    let values = bytemuck::cast_slice::<u8, u64>(&mapped)
        .get(..count)
        .ok_or_else(|| "GPU timestamp readback length mismatch".to_owned())?
        .to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(values)
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_gpu_solver_mode(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
    mode: GpuSolverMode,
) -> Result<GpuSolverOutput, String> {
    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) = request_device(&adapter).await?;
    let lab = GpuSolverLab::new(&device, scenario, candidate, mode)?;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("solver_validation_encoder"),
    });
    lab.encode(&mut encoder);
    let readback = lab.copy_readback(&device, &mut encoder);
    queue.submit([encoder.finish()]);
    let slice = readback.buffer.slice(..);
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("poll GPU solver readback: {error}"))?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("map GPU solver readback: {error}"))?;
    let mapped = slice.get_mapped_range();
    if mapped.len() != readback.total {
        return Err("GPU solver readback length mismatch".to_owned());
    }
    let body_raw: &[BodyRaw] = bytemuck::cast_slice(&mapped[..readback.prepared_offset]);
    let prepared_raw: &[PreparedPointRaw] =
        bytemuck::cast_slice(&mapped[readback.prepared_offset..readback.contribution_offset]);
    let contribution_raw: &[ContributionRaw] =
        bytemuck::cast_slice(&mapped[readback.contribution_offset..readback.trace_offset]);
    let trace_raw: &[TraceRaw] =
        bytemuck::cast_slice(&mapped[readback.trace_offset..readback.topology_offset]);
    let topology_raw: &[u32] = bytemuck::cast_slice(&mapped[readback.topology_offset..]);
    let topology_overflowed = topology_raw[lab.topology_layout.status_index as usize] != 0;
    let color_count = topology_raw[lab.topology_layout.color_count_index as usize];
    let mut bodies = scenario.bodies.clone();
    for (body, raw) in bodies.iter_mut().zip(body_raw) {
        body.position = Vec2::new(raw.position[0], raw.position[1]);
        body.angle = finite(raw.angle, "body angle")?;
        body.linear_velocity = Vec2::new(raw.linear_velocity[0], raw.linear_velocity[1]);
        body.angular_velocity = finite(raw.angular_velocity, "body angular velocity")?;
        body.validate().map_err(|error| error.to_string())?;
    }
    let contact_cache = prepared_raw
        .iter()
        .zip(&lab.point_identities)
        .map(|(point, identity)| {
            Ok(ContactCachePointV1 {
                body_a: identity.body_a,
                body_b: identity.body_b,
                feature_a: identity.feature_a,
                feature_b: identity.feature_b,
                normal_impulse: finite(point.data1[2], "normal impulse")?,
                tangent_impulse: finite(point.data1[3], "tangent impulse")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let traces = if mode == GpuSolverMode::BenchmarkSoftStep {
        Vec::new()
    } else {
        trace_raw
            .iter()
            .filter(|trace| !topology_overflowed || trace.tail[0] != 0)
            .map(trace_record)
            .collect::<Result<Vec<_>, _>>()?
    };
    let contributions = if candidate == GpuSolverCandidate::JacobiGather {
        contribution_raw
            .iter()
            .map(|raw| GpuContributionEvidence {
                point_index: raw.point_index,
                body_index: raw.body_index,
                side: raw.side,
                impulse: Vec2::new(raw.impulse[0], raw.impulse[1]),
                anchor: Vec2::new(raw.anchor[0], raw.anchor[1]),
            })
            .collect()
    } else {
        Vec::new()
    };
    let contact_colors = if candidate == GpuSolverCandidate::ColoredSoftTgs {
        let start = lab.topology_layout.color_base as usize;
        topology_raw[start..start + lab.point_count].to_vec()
    } else {
        Vec::new()
    };
    let mut capacities = lab.capacity_records;
    capacities
        .iter_mut()
        .find(|record| record.class == "topology")
        .expect("GPU solver topology capacity is recorded")
        .overflowed = topology_overflowed;
    drop(mapped);
    readback.buffer.unmap();
    Ok(GpuSolverOutput {
        candidate,
        adapter: diagnostics,
        bodies,
        contact_cache,
        traces,
        contributions,
        capacities,
        dispatches: lab.operations.len() as u32,
        color_count,
        contact_colors,
    })
}

fn point_inputs(scenario: &ScenarioV1) -> Result<(Vec<PointInputRaw>, Vec<PointIdentity>), String> {
    let mut points = Vec::new();
    let mut identities = Vec::new();
    for manifold in &scenario.manifolds {
        let body_a = scenario
            .bodies
            .binary_search_by_key(&manifold.body_a, |body| body.id)
            .map_err(|_| format!("missing manifold body {:?}", manifold.body_a))?;
        let body_b = scenario
            .bodies
            .binary_search_by_key(&manifold.body_b, |body| body.id)
            .map_err(|_| format!("missing manifold body {:?}", manifold.body_b))?;
        if body_a == body_b {
            return Err(format!(
                "manifold {} references the same body twice",
                manifold.id
            ));
        }
        for (point_index, point) in manifold.points.iter().enumerate() {
            let cached = scenario.contact_cache.iter().find(|cached| {
                cached.body_a == manifold.body_a
                    && cached.body_b == manifold.body_b
                    && cached.feature_a == point.feature_a
                    && cached.feature_b == point.feature_b
            });
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
                cache: [
                    cached.map_or(0.0, |value| value.normal_impulse.get()),
                    cached.map_or(0.0, |value| value.tangent_impulse.get()),
                ],
                padding1: [0.0; 2],
            });
            identities.push(PointIdentity {
                body_a: manifold.body_a,
                body_b: manifold.body_b,
                feature_a: point.feature_a,
                feature_b: point.feature_b,
            });
        }
    }
    Ok((points, identities))
}

fn operations(
    candidate: GpuSolverCandidate,
    scenario: &ScenarioV1,
    point_count: usize,
    body_count: usize,
    topology: TopologyLayout,
    mode: GpuSolverMode,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    let mut parameters = Vec::new();
    let mut operations = Vec::new();
    let point_workgroups = workgroups(point_count);
    let body_workgroups = workgroups(body_count);
    let mut trace_base = 0_u32;
    let topology_parameter = PassParamsRaw {
        floats0: [0.0; 4],
        floats1: [0.0; 4],
        floats2: [0.0; 4],
        floats3: [0.0; 4],
        uints0: [
            body_count as u32,
            point_count as u32,
            topology.incident_base,
            topology.cursor_base,
        ],
        uints1: [
            topology.color_base,
            topology.color_mask_base,
            topology.status_index,
            topology.element_count,
        ],
    };
    let mut topology_operation = |pipeline, dispatches| {
        push_operation(
            &mut parameters,
            &mut operations,
            topology_parameter,
            pipeline,
            BufferDirection::AtoB,
            dispatches,
        );
    };
    topology_operation(
        PipelineKind::TopologyClear,
        workgroups(topology.element_count as usize),
    );
    match candidate {
        GpuSolverCandidate::JacobiGather => {
            topology_operation(PipelineKind::TopologyCount, point_workgroups);
            topology_operation(PipelineKind::TopologyPrefix, 1);
            topology_operation(PipelineKind::TopologyScatter, point_workgroups);
        }
        GpuSolverCandidate::ColoredSoftTgs => {
            topology_operation(PipelineKind::TopologyColor, 1);
        }
    }
    let substeps = match mode {
        GpuSolverMode::ContactPass => 1,
        GpuSolverMode::SoftStep | GpuSolverMode::BenchmarkSoftStep => scenario.settings.substeps,
    };
    for substep in 0..substeps {
        let soft_step = mode != GpuSolverMode::ContactPass;
        let trace_substep = if mode == GpuSolverMode::BenchmarkSoftStep {
            -1.0
        } else {
            substep as f32
        };
        if soft_step {
            push_operation(
                &mut parameters,
                &mut operations,
                pass_params(
                    scenario,
                    body_count,
                    point_count,
                    0,
                    0,
                    trace_base,
                    0,
                    trace_substep,
                ),
                PipelineKind::IntegrateVelocity,
                BufferDirection::AtoB,
                body_workgroups,
            );
        }

        let mut prepare = pass_params(
            scenario,
            body_count,
            point_count,
            0,
            0,
            trace_base,
            0,
            trace_substep,
        );
        let prepare_pipeline = if soft_step && substep > 0 {
            prepare.uints0[2] = 1;
            prepare.uints0[3] = 1;
            PipelineKind::Refresh
        } else {
            PipelineKind::Prepare
        };
        push_operation(
            &mut parameters,
            &mut operations,
            prepare,
            prepare_pipeline,
            BufferDirection::AtoB,
            point_workgroups,
        );
        trace_base += point_count as u32;

        candidate_stage(
            candidate,
            &mut parameters,
            &mut operations,
            pass_params(
                scenario,
                body_count,
                point_count,
                1,
                0,
                trace_base,
                0,
                trace_substep,
            ),
            PipelineKind::Warm,
            BufferDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology,
        );
        trace_base += point_count as u32;
        for iteration in 0..scenario.settings.iterations {
            candidate_stage(
                candidate,
                &mut parameters,
                &mut operations,
                pass_params(
                    scenario,
                    body_count,
                    point_count,
                    2,
                    iteration,
                    trace_base,
                    1,
                    trace_substep,
                ),
                PipelineKind::Normal,
                BufferDirection::BtoA,
                point_workgroups,
                body_workgroups,
                topology,
            );
            trace_base += point_count as u32;
            candidate_stage(
                candidate,
                &mut parameters,
                &mut operations,
                pass_params(
                    scenario,
                    body_count,
                    point_count,
                    3,
                    iteration,
                    trace_base,
                    0,
                    trace_substep,
                ),
                PipelineKind::Tangent,
                BufferDirection::AtoB,
                point_workgroups,
                body_workgroups,
                topology,
            );
            trace_base += point_count as u32;
        }

        if soft_step {
            let integrated_direction = match candidate {
                GpuSolverCandidate::JacobiGather => BufferDirection::BtoA,
                GpuSolverCandidate::ColoredSoftTgs => BufferDirection::AtoB,
            };
            push_operation(
                &mut parameters,
                &mut operations,
                pass_params(
                    scenario,
                    body_count,
                    point_count,
                    0,
                    0,
                    trace_base,
                    0,
                    trace_substep,
                ),
                PipelineKind::IntegratePosition,
                integrated_direction,
                body_workgroups,
            );
            let mut refresh = pass_params(
                scenario,
                body_count,
                point_count,
                0,
                0,
                trace_base,
                0,
                trace_substep,
            );
            refresh.uints0[2] = 0;
            refresh.uints0[3] = 0;
            push_operation(
                &mut parameters,
                &mut operations,
                refresh,
                PipelineKind::Refresh,
                integrated_direction,
                point_workgroups,
            );
        }

        candidate_stage(
            candidate,
            &mut parameters,
            &mut operations,
            pass_params(
                scenario,
                body_count,
                point_count,
                4,
                0,
                trace_base,
                0,
                trace_substep,
            ),
            PipelineKind::Normal,
            BufferDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology,
        );
        trace_base += point_count as u32;
        candidate_stage(
            candidate,
            &mut parameters,
            &mut operations,
            pass_params(
                scenario,
                body_count,
                point_count,
                5,
                0,
                trace_base,
                0,
                trace_substep,
            ),
            PipelineKind::Tangent,
            BufferDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology,
        );
        trace_base += point_count as u32;
        candidate_stage(
            candidate,
            &mut parameters,
            &mut operations,
            pass_params(
                scenario,
                body_count,
                point_count,
                6,
                0,
                trace_base,
                0,
                trace_substep,
            ),
            PipelineKind::Restitution,
            BufferDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology,
        );
        trace_base += point_count as u32;
    }
    (parameters, operations)
}

#[allow(clippy::too_many_arguments)]
fn pass_params(
    scenario: &ScenarioV1,
    body_count: usize,
    point_count: usize,
    stage: u32,
    iteration: u32,
    trace_base: u32,
    use_bias: u32,
    trace_substep: f32,
) -> PassParamsRaw {
    let substep_dt = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
    let softness = scenario.settings.contact_softness.map(|policy| {
        (
            gpu_contact_softness_coefficients(policy, substep_dt, false)
                .expect("validated GPU contact softness"),
            gpu_contact_softness_coefficients(policy, substep_dt, true)
                .expect("validated GPU static contact softness"),
        )
    });
    let dynamic = softness.map_or(
        [
            scenario.settings.bias_rate.get(),
            scenario.settings.mass_coefficient.get(),
            scenario.settings.impulse_coefficient.get(),
        ],
        |(dynamic, _)| {
            [
                dynamic.mass_scale * dynamic.bias_rate * substep_dt,
                dynamic.mass_scale,
                dynamic.impulse_scale,
            ]
        },
    );
    PassParamsRaw {
        floats0: [substep_dt, dynamic[0], dynamic[1], dynamic[2]],
        floats1: [
            scenario.settings.restitution_threshold.get(),
            scenario.settings.gravity.x,
            scenario.settings.gravity.y,
            trace_substep,
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
        floats3: softness.map_or([0.0; 4], |(_, static_contact)| {
            [
                static_contact.mass_scale * static_contact.bias_rate * substep_dt,
                static_contact.mass_scale,
                static_contact.impulse_scale,
                1.0,
            ]
        }),
        uints0: [body_count as u32, point_count as u32, 0, 0],
        uints1: [iteration, stage, trace_base, use_bias],
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate_stage(
    candidate: GpuSolverCandidate,
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: BufferDirection,
    point_workgroups: u32,
    body_workgroups: u32,
    topology: TopologyLayout,
) {
    match candidate {
        GpuSolverCandidate::JacobiGather => jacobi_stage(
            parameters,
            operations,
            parameter,
            pipeline,
            direction,
            point_workgroups,
            body_workgroups,
            topology.incident_base,
        ),
        GpuSolverCandidate::ColoredSoftTgs => colored_stage(
            parameters,
            operations,
            parameter,
            topology.color_base,
            point_workgroups,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn jacobi_stage(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: BufferDirection,
    point_workgroups: u32,
    body_workgroups: u32,
    incident_base: u32,
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
    gather.uints1[3] = incident_base;
    push_operation(
        parameters,
        operations,
        gather,
        PipelineKind::Gather,
        direction,
        body_workgroups,
    );
}

#[allow(clippy::too_many_arguments)]
fn colored_stage(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    color_base: u32,
    point_workgroups: u32,
) {
    for color in 0..MAX_COLORS {
        let mut color_parameter = parameter;
        color_parameter.uints0[2] = color_base;
        color_parameter.uints0[3] = color;
        push_operation(
            parameters,
            operations,
            color_parameter,
            PipelineKind::Colored,
            BufferDirection::AtoB,
            point_workgroups,
        );
    }
}

fn push_operation(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: BufferDirection,
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

fn trace_record(raw: &TraceRaw) -> Result<TraceRecordV1, String> {
    let (stage, names): (TraceStageV1, &[&str]) = match raw.meta[0] {
        0 => (
            TraceStageV1::Prepare,
            &[
                "anchorAX",
                "anchorAY",
                "anchorBX",
                "anchorBY",
                "initialNormalVelocity",
                "normalMass",
                "separation",
                "tangentMass",
            ],
        ),
        1 => (
            TraceStageV1::WarmStart,
            &[
                "appliedImpulseX",
                "appliedImpulseY",
                "normalImpulse",
                "tangentImpulse",
            ],
        ),
        2 => (TraceStageV1::NormalSolve, normal_names()),
        3 => (TraceStageV1::TangentSolve, tangent_names()),
        4 => (TraceStageV1::Relaxation, normal_names()),
        5 => (TraceStageV1::Relaxation, tangent_names()),
        6 => (
            TraceStageV1::Restitution,
            &[
                "appliedRestitutionImpulse",
                "eligible",
                "initialNormalVelocity",
                "normalImpulse",
                "normalVelocity",
            ],
        ),
        unknown => return Err(format!("unknown GPU trace stage {unknown}")),
    };
    if raw.tail[0] as usize != names.len() {
        return Err("GPU trace value count mismatch".to_owned());
    }
    let numbers = raw.values0.into_iter().chain(raw.values1);
    let values = names
        .iter()
        .zip(numbers)
        .map(|(name, value)| {
            Ok(TraceValueV1 {
                name: (*name).to_owned(),
                value: finite(value, name)?,
                bits: value.to_bits(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let record = TraceRecordV1 {
        step: 0,
        substep: raw.tail[1],
        iteration: raw.meta[1],
        stage,
        manifold_id: raw.meta[2],
        point_index: raw.meta[3],
        values,
    };
    record.validate().map_err(|error| error.to_string())?;
    Ok(record)
}

fn normal_names() -> &'static [&'static str] {
    &[
        "appliedNormalImpulse",
        "impulseScale",
        "massScale",
        "normalImpulse",
        "normalVelocity",
        "velocityBias",
    ]
}

fn tangent_names() -> &'static [&'static str] {
    &[
        "appliedTangentImpulse",
        "frictionLimit",
        "tangentImpulse",
        "tangentVelocity",
    ]
}

fn body_raw(body: &BodyStateV1) -> BodyRaw {
    BodyRaw {
        position: [body.position.x, body.position.y],
        angle: body.angle.get(),
        inverse_mass: body.inverse_mass.get(),
        linear_velocity: [body.linear_velocity.x, body.linear_velocity.y],
        angular_velocity: body.angular_velocity.get(),
        inverse_inertia: body.inverse_inertia.get(),
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
    label: &str,
    source: &wgpu::Buffer,
    destination: &wgpu::Buffer,
    points: &wgpu::Buffer,
    prepared: &wgpu::Buffer,
    contributions: &wgpu::Buffer,
    topology: &wgpu::Buffer,
    traces: &wgpu::Buffer,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            entry(0, source),
            entry(1, destination),
            entry(2, points),
            entry(3, prepared),
            entry(4, contributions),
            entry(5, topology),
            entry(6, traces),
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: parameters,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<PassParamsRaw>() as u64),
                }),
            },
        ],
    })
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn storage_init(device: &wgpu::Device, label: &str, bytes: &[u8], copy: bool) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE
            | if copy {
                wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST
            } else {
                wgpu::BufferUsages::empty()
            },
    })
}

fn storage_empty<T>(device: &wgpu::Device, label: &str, count: usize, copy: bool) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count * size_of::<T>()) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | if copy {
                wgpu::BufferUsages::COPY_SRC
            } else {
                wgpu::BufferUsages::empty()
            },
        mapped_at_creation: false,
    })
}

fn workgroups(elements: usize) -> u32 {
    (elements as u32).div_ceil(WORKGROUP_SIZE)
}

fn finite(value: f32, name: &str) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|_| format!("GPU {name} is non-finite"))
}

fn capacities(
    body_count: usize,
    point_count: usize,
    parameter_count: usize,
    topology_count: usize,
    trace_count: usize,
) -> Vec<CapacityRecordV1> {
    let record = |class: &str, count: usize, stride: usize| CapacityRecordV1 {
        class: class.to_owned(),
        capacity_elements: count as u64,
        used_elements: count as u64,
        allocated_bytes: (count * stride) as u64,
        high_water_bytes: (count * stride) as u64,
        overflowed: false,
    };
    vec![
        record("bodies", body_count * 2, size_of::<BodyRaw>()),
        record(
            "contributions",
            point_count * 2,
            size_of::<ContributionRaw>(),
        ),
        record("parameters", parameter_count, PARAMETER_STRIDE),
        record(
            "points",
            point_count,
            size_of::<PointInputRaw>() + size_of::<PreparedPointRaw>(),
        ),
        record("topology", topology_count, size_of::<u32>()),
        record("traces", trace_count, size_of::<TraceRaw>()),
    ]
}
