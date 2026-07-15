use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{CapacityRecordV1, ScenarioV1};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;
use crate::gpu_solver::GpuContributionEvidence;

const WORKGROUP_SIZE: u32 = 64;
const PARAMETER_STRIDE: usize = 256;

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
    uints0: [u32; 4],
    uints1: [u32; 4],
}

#[derive(Clone, Copy)]
enum PipelineKind {
    TopologyClear,
    TopologyCount,
    TopologyPrefix,
    TopologyScatter,
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
    incident_base: u32,
    cursor_base: u32,
    element_count: u32,
}

impl TopologyLayout {
    fn new(body_count: u32, point_count: u32) -> Result<Self, String> {
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
            incident_base,
            cursor_base,
            element_count,
        })
    }
}

pub(crate) struct GpuFixedFloorSolver {
    _motion_scratch: wgpu::Buffer,
    _prepared: wgpu::Buffer,
    _contributions: wgpu::Buffer,
    _topology: wgpu::Buffer,
    bind_ab: wgpu::BindGroup,
    bind_ba: wgpu::BindGroup,
    topology_clear_pipeline: wgpu::ComputePipeline,
    topology_count_pipeline: wgpu::ComputePipeline,
    topology_prefix_pipeline: wgpu::ComputePipeline,
    topology_scatter_pipeline: wgpu::ComputePipeline,
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
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "fixed-floor body count exceeds u32".to_owned())?;
        let (points, point_count) = point_inputs(scenario)?;
        let topology_layout = TopologyLayout::new(body_count, point_count)?;
        let (parameters, operations) =
            operations(scenario, body_count, point_count, topology_layout);

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
        let point_buffer = storage_init(
            device,
            "physbox5_floor_points",
            bytemuck::cast_slice(&points),
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
            &point_buffer,
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
            &point_buffer,
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
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let capacities = vec![
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
            capacity(
                "solver-topology",
                topology_layout.element_count,
                size_of::<u32>() as u64,
            ),
        ];
        Ok(Self {
            _motion_scratch: motion_scratch,
            _prepared: prepared,
            _contributions: contributions,
            _topology: topology,
            bind_ab,
            bind_ba,
            topology_clear_pipeline: pipeline("floor_topology_clear", "clear_topology"),
            topology_count_pipeline: pipeline("floor_topology_count", "count_edges"),
            topology_prefix_pipeline: pipeline("floor_topology_prefix", "prefix_edges"),
            topology_scatter_pipeline: pipeline("floor_topology_scatter", "scatter_edges"),
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
        })
    }

    pub(crate) fn encode_step(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_fixed_floor_solve"),
            timestamp_writes: None,
        });
        for operation in &self.operations {
            pass.set_pipeline(match operation.pipeline {
                PipelineKind::TopologyClear => &self.topology_clear_pipeline,
                PipelineKind::TopologyCount => &self.topology_count_pipeline,
                PipelineKind::TopologyPrefix => &self.topology_prefix_pipeline,
                PipelineKind::TopologyScatter => &self.topology_scatter_pipeline,
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
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    let mut parameters = Vec::new();
    let mut operations = Vec::new();
    let body_workgroups = workgroups(body_count);
    let point_workgroups = workgroups(point_count);
    let topology_params = PassParamsRaw {
        floats0: [0.0; 4],
        floats1: [0.0; 4],
        uints0: [
            body_count,
            point_count,
            topology.incident_base,
            topology.cursor_base,
        ],
        uints1: [0, 0, 0, topology.element_count],
    };
    for (kind, workgroups) in [
        (
            PipelineKind::TopologyClear,
            workgroups(topology.element_count),
        ),
        (PipelineKind::TopologyCount, point_workgroups),
        (PipelineKind::TopologyPrefix, 1),
        (PipelineKind::TopologyScatter, point_workgroups),
    ] {
        push_operation(
            &mut parameters,
            &mut operations,
            topology_params,
            kind,
            MotionDirection::AtoB,
            workgroups,
        );
    }
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
            topology.incident_base,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 2, 0, 1),
            PipelineKind::Normal,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology.incident_base,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 3, 0, 0),
            PipelineKind::Tangent,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology.incident_base,
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
            topology.incident_base,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 5, 0, 0),
            PipelineKind::Tangent,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            topology.incident_base,
        );
        jacobi_stage(
            &mut parameters,
            &mut operations,
            pass_params(scenario, body_count, point_count, 6, 0, 0),
            PipelineKind::Restitution,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            topology.incident_base,
        );
    }
    (parameters, operations)
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
        let topology = TopologyLayout::new(2, 2).unwrap();
        let (parameters, operations) = operations(&scenario, 2, 2, topology);
        assert_eq!(operations.len(), 68);
        assert_eq!(parameters.len(), operations.len());
        assert_eq!(topology.incident_base, 4);
        assert_eq!(topology.cursor_base, 8);
        assert_eq!(topology.element_count, 10);
    }

    #[test]
    fn fixed_floor_rejects_unreviewed_solver_defaults() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        validate_selected_settings(&scenario).unwrap();
        scenario.settings.iterations = 2;
        assert!(validate_selected_settings(&scenario).is_err());
    }
}
