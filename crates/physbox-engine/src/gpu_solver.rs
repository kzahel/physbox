use std::{collections::BTreeSet, mem::size_of, sync::mpsc};

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, ContactCachePointV1, FeatureIdV1, FiniteF32, ScenarioV1,
    TraceRecordV1, TraceStageV1, TraceValueV1, Validate, Vec2,
};
use wgpu::util::DeviceExt;

use crate::gpu_context::{AdapterDiagnostics, create_instance, request_adapter, request_device};

const WORKGROUP_SIZE: u32 = 64;
const PARAMETER_STRIDE: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuSolverCandidate {
    JacobiGather,
    ColoredSoftTgs,
}

impl GpuSolverCandidate {
    pub const fn name(self) -> &'static str {
        match self {
            Self::JacobiGather => "jacobi-gather",
            Self::ColoredSoftTgs => "colored-soft-tgs",
        }
    }
}

#[derive(Clone, Debug)]
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
    uints0: [u32; 4],
    uints1: [u32; 4],
}

#[derive(Clone, Copy)]
enum PipelineKind {
    Prepare,
    Warm,
    Normal,
    Tangent,
    Restitution,
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

struct GpuSolverLab {
    body_a: wgpu::Buffer,
    prepared: wgpu::Buffer,
    contributions: wgpu::Buffer,
    traces: wgpu::Buffer,
    bind_ab: wgpu::BindGroup,
    bind_ba: wgpu::BindGroup,
    prepare_pipeline: wgpu::ComputePipeline,
    warm_pipeline: wgpu::ComputePipeline,
    normal_pipeline: wgpu::ComputePipeline,
    tangent_pipeline: wgpu::ComputePipeline,
    restitution_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    colored_pipeline: wgpu::ComputePipeline,
    operations: Vec<Operation>,
    point_identities: Vec<PointIdentity>,
    body_count: usize,
    point_count: usize,
    trace_count: usize,
    color_count: u32,
    capacity_records: Vec<CapacityRecordV1>,
}

impl GpuSolverLab {
    fn new(
        device: &wgpu::Device,
        scenario: &ScenarioV1,
        candidate: GpuSolverCandidate,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        let bodies = scenario.bodies.iter().map(body_raw).collect::<Vec<_>>();
        let (points, point_identities) = point_inputs(scenario)?;
        if points.is_empty() {
            return Err("GPU solver laboratory requires authored contact points".to_owned());
        }
        let colors = graph_colors(&points, bodies.len());
        let (topology, incident_base, color_ranges) = topology(&points, bodies.len(), &colors);
        let trace_count = points.len() * (5 + 2 * scenario.settings.iterations as usize);
        let (parameters, operations) = operations(
            candidate,
            scenario,
            points.len(),
            bodies.len(),
            incident_base,
            &color_ranges,
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
        let topology_buffer = storage_init(
            device,
            "solver_topology",
            bytemuck::cast_slice(&topology),
            false,
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
                storage_entry(5, true),
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
            &topology_buffer,
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
            &topology_buffer,
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
            topology.len(),
            trace_count,
        );
        Ok(Self {
            body_a,
            prepared,
            contributions,
            traces,
            bind_ab,
            bind_ba,
            prepare_pipeline: pipeline("solver_prepare", "prepare"),
            warm_pipeline: pipeline("solver_warm", "emit_warm"),
            normal_pipeline: pipeline("solver_normal", "emit_normal"),
            tangent_pipeline: pipeline("solver_tangent", "emit_tangent"),
            restitution_pipeline: pipeline("solver_restitution", "emit_restitution"),
            gather_pipeline: pipeline("solver_gather", "gather"),
            colored_pipeline: pipeline("solver_colored", "colored_stage"),
            operations,
            point_identities,
            body_count: bodies.len(),
            point_count: points.len(),
            trace_count,
            color_count: colors.len() as u32,
            capacity_records,
        })
    }

    fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("solver_candidate_passes"),
            timestamp_writes: None,
        });
        for operation in &self.operations {
            pass.set_pipeline(match operation.pipeline {
                PipelineKind::Prepare => &self.prepare_pipeline,
                PipelineKind::Warm => &self.warm_pipeline,
                PipelineKind::Normal => &self.normal_pipeline,
                PipelineKind::Tangent => &self.tangent_pipeline,
                PipelineKind::Restitution => &self.restitution_pipeline,
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

    fn copy_readback(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) -> Readback {
        let body_bytes = self.body_count * size_of::<BodyRaw>();
        let prepared_bytes = self.point_count * size_of::<PreparedPointRaw>();
        let contribution_bytes = self.point_count * 2 * size_of::<ContributionRaw>();
        let trace_bytes = self.trace_count * size_of::<TraceRaw>();
        let prepared_offset = body_bytes;
        let contribution_offset = prepared_offset + prepared_bytes;
        let trace_offset = contribution_offset + contribution_bytes;
        let total = trace_offset + trace_bytes;
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
        Readback {
            buffer,
            prepared_offset,
            contribution_offset,
            trace_offset,
            total,
        }
    }
}

struct Readback {
    buffer: wgpu::Buffer,
    prepared_offset: usize,
    contribution_offset: usize,
    trace_offset: usize,
    total: usize,
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_solver_case(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
) -> Result<GpuSolverOutput, String> {
    let instance = create_instance();
    let adapter = request_adapter(&instance, None).await?;
    let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) = request_device(&adapter).await?;
    let lab = GpuSolverLab::new(&device, scenario, candidate)?;
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
    let trace_raw: &[TraceRaw] = bytemuck::cast_slice(&mapped[readback.trace_offset..]);
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
    let traces = trace_raw
        .iter()
        .map(trace_record)
        .collect::<Result<Vec<_>, _>>()?;
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
    drop(mapped);
    readback.buffer.unmap();
    Ok(GpuSolverOutput {
        candidate,
        adapter: diagnostics,
        bodies,
        contact_cache,
        traces,
        contributions,
        capacities: lab.capacity_records,
        dispatches: lab.operations.len() as u32,
        color_count: lab.color_count,
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

fn graph_colors(points: &[PointInputRaw], body_count: usize) -> Vec<Vec<u32>> {
    let mut colors: Vec<(Vec<u32>, BTreeSet<u32>)> = Vec::new();
    for (point_index, point) in points.iter().enumerate() {
        let color = colors.iter().position(|(_, bodies)| {
            !bodies.contains(&point.ids[0]) && !bodies.contains(&point.ids[1])
        });
        let color = color.unwrap_or_else(|| {
            colors.push((Vec::new(), BTreeSet::new()));
            colors.len() - 1
        });
        colors[color].0.push(point_index as u32);
        colors[color].1.insert(point.ids[0]);
        colors[color].1.insert(point.ids[1]);
    }
    debug_assert!(
        points
            .iter()
            .all(|point| point.ids[0] < body_count as u32 && point.ids[1] < body_count as u32)
    );
    colors.into_iter().map(|(points, _)| points).collect()
}

fn topology(
    points: &[PointInputRaw],
    body_count: usize,
    colors: &[Vec<u32>],
) -> (Vec<u32>, u32, Vec<(u32, u32)>) {
    let mut incidents = vec![Vec::new(); body_count];
    for (point_index, point) in points.iter().enumerate() {
        incidents[point.ids[0] as usize].push((point_index * 2) as u32);
        incidents[point.ids[1] as usize].push((point_index * 2 + 1) as u32);
    }
    let incident_base = (body_count * 2) as u32;
    let mut output = Vec::with_capacity(body_count * 2 + points.len() * 2 + points.len());
    let mut start = 0_u32;
    for body in &incidents {
        output.push(start);
        output.push(body.len() as u32);
        start += body.len() as u32;
    }
    for body in incidents {
        output.extend(body);
    }
    let mut ranges = Vec::new();
    for color in colors {
        let offset = output.len() as u32;
        output.extend(color);
        ranges.push((offset, color.len() as u32));
    }
    (output, incident_base, ranges)
}

fn operations(
    candidate: GpuSolverCandidate,
    scenario: &ScenarioV1,
    point_count: usize,
    body_count: usize,
    incident_base: u32,
    color_ranges: &[(u32, u32)],
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    let mut parameters = Vec::new();
    let mut operations = Vec::new();
    let point_workgroups = workgroups(point_count);
    let body_workgroups = workgroups(body_count);
    let mut trace_base = 0_u32;
    let base = |stage, iteration, trace_base, use_bias, topology_offset, count| PassParamsRaw {
        floats0: [
            scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32,
            scenario.settings.bias_rate.get(),
            scenario.settings.mass_coefficient.get(),
            scenario.settings.impulse_coefficient.get(),
        ],
        floats1: [scenario.settings.restitution_threshold.get(), 0.0, 0.0, 0.0],
        uints0: [
            body_count as u32,
            point_count as u32,
            topology_offset,
            count,
        ],
        uints1: [iteration, stage, trace_base, use_bias],
    };
    push_operation(
        &mut parameters,
        &mut operations,
        base(0, 0, trace_base, 0, 0, 0),
        PipelineKind::Prepare,
        BufferDirection::AtoB,
        point_workgroups,
    );
    trace_base += point_count as u32;
    match candidate {
        GpuSolverCandidate::JacobiGather => {
            jacobi_stage(
                &mut parameters,
                &mut operations,
                base(1, 0, trace_base, 0, 0, 0),
                PipelineKind::Warm,
                BufferDirection::AtoB,
                point_workgroups,
                body_workgroups,
                incident_base,
            );
            trace_base += point_count as u32;
            for iteration in 0..scenario.settings.iterations {
                jacobi_stage(
                    &mut parameters,
                    &mut operations,
                    base(2, iteration, trace_base, 1, 0, 0),
                    PipelineKind::Normal,
                    BufferDirection::BtoA,
                    point_workgroups,
                    body_workgroups,
                    incident_base,
                );
                trace_base += point_count as u32;
                jacobi_stage(
                    &mut parameters,
                    &mut operations,
                    base(3, iteration, trace_base, 0, 0, 0),
                    PipelineKind::Tangent,
                    BufferDirection::AtoB,
                    point_workgroups,
                    body_workgroups,
                    incident_base,
                );
                trace_base += point_count as u32;
            }
            jacobi_stage(
                &mut parameters,
                &mut operations,
                base(4, 0, trace_base, 0, 0, 0),
                PipelineKind::Normal,
                BufferDirection::BtoA,
                point_workgroups,
                body_workgroups,
                incident_base,
            );
            trace_base += point_count as u32;
            jacobi_stage(
                &mut parameters,
                &mut operations,
                base(5, 0, trace_base, 0, 0, 0),
                PipelineKind::Tangent,
                BufferDirection::AtoB,
                point_workgroups,
                body_workgroups,
                incident_base,
            );
            trace_base += point_count as u32;
            jacobi_stage(
                &mut parameters,
                &mut operations,
                base(6, 0, trace_base, 0, 0, 0),
                PipelineKind::Restitution,
                BufferDirection::BtoA,
                point_workgroups,
                body_workgroups,
                incident_base,
            );
        }
        GpuSolverCandidate::ColoredSoftTgs => {
            colored_stage(
                &mut parameters,
                &mut operations,
                &base,
                color_ranges,
                1,
                0,
                trace_base,
                0,
            );
            trace_base += point_count as u32;
            for iteration in 0..scenario.settings.iterations {
                colored_stage(
                    &mut parameters,
                    &mut operations,
                    &base,
                    color_ranges,
                    2,
                    iteration,
                    trace_base,
                    1,
                );
                trace_base += point_count as u32;
                colored_stage(
                    &mut parameters,
                    &mut operations,
                    &base,
                    color_ranges,
                    3,
                    iteration,
                    trace_base,
                    0,
                );
                trace_base += point_count as u32;
            }
            colored_stage(
                &mut parameters,
                &mut operations,
                &base,
                color_ranges,
                4,
                0,
                trace_base,
                0,
            );
            trace_base += point_count as u32;
            colored_stage(
                &mut parameters,
                &mut operations,
                &base,
                color_ranges,
                5,
                0,
                trace_base,
                0,
            );
            trace_base += point_count as u32;
            colored_stage(
                &mut parameters,
                &mut operations,
                &base,
                color_ranges,
                6,
                0,
                trace_base,
                0,
            );
        }
    }
    (parameters, operations)
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
    base: &impl Fn(u32, u32, u32, u32, u32, u32) -> PassParamsRaw,
    color_ranges: &[(u32, u32)],
    stage: u32,
    iteration: u32,
    trace_base: u32,
    use_bias: u32,
) {
    for &(offset, count) in color_ranges {
        push_operation(
            parameters,
            operations,
            base(stage, iteration, trace_base, use_bias, offset, count),
            PipelineKind::Colored,
            BufferDirection::AtoB,
            workgroups(count as usize),
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
        substep: 0,
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
                wgpu::BufferUsages::COPY_SRC
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
