use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES, COLLISION_CONTACT_POINT_BYTES,
    COLLISION_MANIFOLD_BYTES, COLLISION_WORK_COUNTER_BYTES, CandidatePairV1, CollisionCapacityV1,
    CollisionContactPointV1, CollisionManifoldV1, CollisionWorkV1, ContactIdentityPolicyV1,
    FailureKindV1, FailureV1, FiniteF32, GeometricFeatureKindV1, GeometricFeatureV1, MaterialV1,
    ScenarioV1, Validate, Vec2, canonical_hash,
};

use crate::{
    AdapterDiagnostics,
    collision_candidates::{CandidateCase, CandidatePipelines},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    collision_candidates::map_values,
    gpu_context::{create_instance, request_adapter, request_device},
};

const WORKGROUP_SIZE: u32 = 64;
const SCAN_WORKGROUP_SIZE: u32 = 256;
const TOTAL_DISPATCHES: u32 = 15;
const NARROW_SCRATCH_BYTES: u32 = 144;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ContactPointRaw {
    anchors: [f32; 4],
    world_separation: [f32; 4],
    features: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct NarrowScratchRaw {
    identities: [u32; 4],
    normal_material: [f32; 4],
    point_count: [u32; 4],
    points: [ContactPointRaw; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ManifoldRaw {
    identities: [u32; 4],
    normal_material: [f32; 4],
    point_range: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CapacityRaw {
    values: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct WorkRaw {
    values: [u32; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct NarrowParamsRaw {
    counts: [u32; 4],
    execution: [u32; 4],
    policy: [f32; 4],
}

const _: () = assert!(size_of::<ContactPointRaw>() == COLLISION_CONTACT_POINT_BYTES as usize);
const _: () = assert!(size_of::<NarrowScratchRaw>() == NARROW_SCRATCH_BYTES as usize);
const _: () = assert!(size_of::<ManifoldRaw>() == COLLISION_MANIFOLD_BYTES as usize);
const _: () = assert!(size_of::<CapacityRaw>() == COLLISION_CAPACITY_COUNTER_BYTES as usize);
const _: () = assert!(size_of::<WorkRaw>() == COLLISION_WORK_COUNTER_BYTES as usize);

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuNarrowphaseOutput {
    pub adapter: AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub candidate_capacity: u32,
    pub manifold_capacity: u32,
    pub point_capacity: u32,
    pub manifolds: Vec<CollisionManifoldV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
}

impl GpuNarrowphaseOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_scenario_hash.len() != 64
            || !self
                .source_scenario_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("GPU narrowphase output has an invalid source hash".to_owned());
        }
        for pair in self.manifolds.windows(2) {
            if pair[0].pair >= pair[1].pair {
                return Err("GPU manifolds are not sorted and unique".to_owned());
            }
        }
        for manifold in &self.manifolds {
            manifold.validate().map_err(|error| error.to_string())?;
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("GPU narrowphase capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        if let Some(failure) = &self.failure {
            failure.validate().map_err(|error| error.to_string())?;
        }
        let overflowed = self.capacities.iter().any(|capacity| capacity.overflowed);
        let output_points = self
            .manifolds
            .iter()
            .map(|manifold| manifold.points.len() as u64)
            .sum::<u64>();
        if overflowed != self.failure.is_some()
            || (overflowed && !self.manifolds.is_empty())
            || (!overflowed
                && (self.manifolds.len() as u64 != self.work.manifolds
                    || output_points != self.work.contact_points))
        {
            return Err("GPU narrowphase failure/count state is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuNarrowphaseRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    candidate_pipelines: CandidatePipelines,
    narrowphase_pipelines: NarrowphasePipelines,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuNarrowphaseRunner {
    pub async fn new() -> Result<Self, String> {
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        let candidate_pipelines = CandidatePipelines::new(&device);
        let narrowphase_pipelines = NarrowphasePipelines::new(&device);
        Ok(Self {
            device,
            queue,
            diagnostics,
            candidate_pipelines,
            narrowphase_pipelines,
        })
    }

    pub fn adapter(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn run(
        &self,
        scenario: &ScenarioV1,
        candidate_capacity: Option<u32>,
        manifold_capacity: Option<u32>,
    ) -> Result<GpuNarrowphaseOutput, String> {
        self.run_with_identity_policy(
            scenario,
            candidate_capacity,
            manifold_capacity,
            ContactIdentityPolicyV1::SELECTED,
        )
    }

    pub fn run_with_identity_policy(
        &self,
        scenario: &ScenarioV1,
        candidate_capacity: Option<u32>,
        manifold_capacity: Option<u32>,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<GpuNarrowphaseOutput, String> {
        let candidate_case = CandidateCase::new(
            &self.device,
            &self.candidate_pipelines,
            scenario,
            candidate_capacity,
        )?;
        let narrowphase_case = NarrowphaseCase::new_with_identity_policy(
            &self.device,
            &self.narrowphase_pipelines,
            &candidate_case,
            manifold_capacity,
            identity_policy,
        )?;
        let manifold_readback = readback(
            &self.device,
            "collision_manifold_readback",
            u64::from(narrowphase_case.manifold_capacity) * u64::from(COLLISION_MANIFOLD_BYTES),
        );
        let point_readback = readback(
            &self.device,
            "collision_point_readback",
            u64::from(narrowphase_case.point_capacity) * u64::from(COLLISION_CONTACT_POINT_BYTES),
        );
        let candidate_capacity_readback = readback(
            &self.device,
            "collision_candidate_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let manifold_capacity_readback = readback(
            &self.device,
            "collision_manifold_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let point_capacity_readback = readback(
            &self.device,
            "collision_point_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let work_readback = readback(
            &self.device,
            "collision_narrowphase_work_readback",
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_collision_narrowphase_encoder"),
            });
        candidate_case.encode(&mut encoder, &self.candidate_pipelines);
        narrowphase_case.encode(&mut encoder, &self.narrowphase_pipelines);
        copy(
            &mut encoder,
            &narrowphase_case.manifolds,
            &manifold_readback,
            u64::from(narrowphase_case.manifold_capacity) * u64::from(COLLISION_MANIFOLD_BYTES),
        );
        copy(
            &mut encoder,
            &narrowphase_case.points,
            &point_readback,
            u64::from(narrowphase_case.point_capacity) * u64::from(COLLISION_CONTACT_POINT_BYTES),
        );
        copy(
            &mut encoder,
            candidate_case.candidate_capacity_buffer(),
            &candidate_capacity_readback,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        copy(
            &mut encoder,
            &narrowphase_case.manifold_capacity_buffer,
            &manifold_capacity_readback,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        copy(
            &mut encoder,
            &narrowphase_case.point_capacity_buffer,
            &point_capacity_readback,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        copy(
            &mut encoder,
            candidate_case.work_buffer(),
            &work_readback,
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU narrowphase poll failed: {error}"))?;

        let candidate_raw =
            map_values::<CapacityRaw>(&self.device, &candidate_capacity_readback, 1)?[0];
        let manifold_raw =
            map_values::<CapacityRaw>(&self.device, &manifold_capacity_readback, 1)?[0];
        let point_capacity_raw =
            map_values::<CapacityRaw>(&self.device, &point_capacity_readback, 1)?[0];
        let manifold_records = map_values::<ManifoldRaw>(
            &self.device,
            &manifold_readback,
            narrowphase_case.manifold_capacity as usize,
        )?;
        let point_records = map_values::<ContactPointRaw>(
            &self.device,
            &point_readback,
            narrowphase_case.point_capacity as usize,
        )?;
        let work_raw = map_values::<WorkRaw>(&self.device, &work_readback, 1)?[0];
        let any_overflow = candidate_raw.values[3] != 0
            || manifold_raw.values[3] != 0
            || point_capacity_raw.values[3] != 0;
        let manifold_count = if any_overflow {
            0
        } else {
            manifold_raw.values[2] as usize
        };
        let mut manifolds = Vec::with_capacity(manifold_count);
        for raw in manifold_records.into_iter().take(manifold_count) {
            let start = raw.point_range[0] as usize;
            let count = raw.point_range[1] as usize;
            if count == 0 || count > 2 || start + count > point_records.len() {
                return Err("GPU narrowphase emitted an invalid point range".to_owned());
            }
            let points = point_records[start..start + count]
                .iter()
                .copied()
                .map(contact_point)
                .collect::<Result<Vec<_>, _>>()?;
            manifolds.push(CollisionManifoldV1 {
                pair: CandidatePairV1 {
                    body_a: physbox_contracts::BodyId {
                        slot: raw.identities[0],
                        generation: raw.identities[1],
                    },
                    body_b: physbox_contracts::BodyId {
                        slot: raw.identities[2],
                        generation: raw.identities[3],
                    },
                },
                normal: Vec2::new(raw.normal_material[0], raw.normal_material[1]),
                material: MaterialV1 {
                    friction: finite(raw.normal_material[2])?,
                    restitution: finite(raw.normal_material[3])?,
                },
                points,
            });
        }
        let work = work_record(work_raw);
        let candidate_capacity_record =
            capacity_record("candidate-pairs", CANDIDATE_PAIR_BYTES, candidate_raw);
        let manifold_capacity_record =
            capacity_record("contact-manifolds", COLLISION_MANIFOLD_BYTES, manifold_raw);
        let point_capacity_record = capacity_record(
            "contact-points",
            COLLISION_CONTACT_POINT_BYTES,
            point_capacity_raw,
        );
        let mut capacities = candidate_case.capacities(candidate_capacity_record);
        capacities.extend(narrowphase_case.capacities(
            manifold_capacity_record,
            point_capacity_record,
            u64::from(candidate_raw.values[2]),
        ));
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
        let failure = capacities
            .iter()
            .find(|capacity| capacity.overflowed)
            .map(|capacity| FailureV1 {
                kind: FailureKindV1::CapacityOverflow,
                reason: format!(
                    "{} capacity {} rejected {} attempted records",
                    capacity.class, capacity.capacity_elements, capacity.attempted_elements
                ),
                first_step: Some(0),
                first_stage: Some(if capacity.class == "candidate-pairs" {
                    "candidate".to_owned()
                } else {
                    "narrowphase".to_owned()
                }),
                reproducer_hash: Some(source_scenario_hash.clone()),
            });
        let output = GpuNarrowphaseOutput {
            adapter: self.diagnostics.clone(),
            source_scenario_hash,
            candidate_capacity: candidate_case.candidate_capacity(),
            manifold_capacity: narrowphase_case.manifold_capacity,
            point_capacity: narrowphase_case.point_capacity,
            manifolds,
            work,
            capacities,
            failure,
        };
        output.validate()?;
        Ok(output)
    }
}

pub(crate) struct NarrowphasePipelines {
    clear: wgpu::ComputePipeline,
    generate: wgpu::ComputePipeline,
    scan_blocks: wgpu::ComputePipeline,
    scan_totals: wgpu::ComputePipeline,
    add_offsets: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    scatter: wgpu::ComputePipeline,
    clear_layout: wgpu::BindGroupLayout,
    generate_layout: wgpu::BindGroupLayout,
    scan_layout: wgpu::BindGroupLayout,
    finalize_layout: wgpu::BindGroupLayout,
    scatter_layout: wgpu::BindGroupLayout,
}

impl NarrowphasePipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let clear_layout = layout(
            device,
            "narrowphase_clear_layout",
            &[(6, false), (7, false), (11, false), (12, false)],
        );
        let generate_layout = layout(
            device,
            "narrowphase_generate_layout",
            &[
                (0, true),
                (1, true),
                (2, true),
                (3, true),
                (4, false),
                (5, false),
                (6, false),
                (12, false),
            ],
        );
        let scan_layout = layout(
            device,
            "narrowphase_scan_layout",
            &[(6, false), (7, false), (8, false)],
        );
        let finalize_layout = layout(
            device,
            "narrowphase_finalize_layout",
            &[
                (4, false),
                (6, false),
                (7, false),
                (11, false),
                (12, false),
                (13, false),
            ],
        );
        let scatter_layout = layout(
            device,
            "narrowphase_scatter_layout",
            &[
                (5, false),
                (6, false),
                (7, false),
                (9, false),
                (10, false),
                (11, false),
                (12, false),
            ],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_collision_narrowphase_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("collision_narrowphase.wgsl").into()),
        });
        let pipeline = |label: &'static str, entry: &'static str, bind: &wgpu::BindGroupLayout| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[bind],
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
            clear: pipeline(
                "narrowphase_clear_pipeline",
                "clear_narrowphase",
                &clear_layout,
            ),
            generate: pipeline(
                "narrowphase_generate_pipeline",
                "generate_narrowphase",
                &generate_layout,
            ),
            scan_blocks: pipeline(
                "narrowphase_scan_blocks_pipeline",
                "scan_manifold_blocks",
                &scan_layout,
            ),
            scan_totals: pipeline(
                "narrowphase_scan_totals_pipeline",
                "scan_manifold_block_totals",
                &scan_layout,
            ),
            add_offsets: pipeline(
                "narrowphase_add_offsets_pipeline",
                "add_manifold_block_offsets",
                &scan_layout,
            ),
            finalize: pipeline(
                "narrowphase_finalize_pipeline",
                "finalize_narrowphase",
                &finalize_layout,
            ),
            scatter: pipeline(
                "narrowphase_scatter_pipeline",
                "scatter_narrowphase",
                &scatter_layout,
            ),
            clear_layout,
            generate_layout,
            scan_layout,
            finalize_layout,
            scatter_layout,
        }
    }
}

pub(crate) struct NarrowphaseCase {
    _scratch: wgpu::Buffer,
    _flags: wgpu::Buffer,
    _offsets: wgpu::Buffer,
    _scan_blocks: wgpu::Buffer,
    manifolds: wgpu::Buffer,
    points: wgpu::Buffer,
    manifold_capacity_buffer: wgpu::Buffer,
    point_capacity_buffer: wgpu::Buffer,
    params: wgpu::Buffer,
    clear_group: wgpu::BindGroup,
    generate_group: wgpu::BindGroup,
    scan_group: wgpu::BindGroup,
    finalize_group: wgpu::BindGroup,
    scatter_group: wgpu::BindGroup,
    candidate_capacity: u32,
    manifold_capacity: u32,
    point_capacity: u32,
    block_count: u32,
}

impl NarrowphaseCase {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &NarrowphasePipelines,
        candidate: &CandidateCase,
        requested_manifold_capacity: Option<u32>,
    ) -> Result<Self, String> {
        Self::new_with_storage_and_identity_policy(
            device,
            pipelines,
            candidate,
            requested_manifold_capacity,
            candidate.storage(),
            ContactIdentityPolicyV1::SELECTED,
        )
    }

    pub(crate) fn new_with_identity_policy(
        device: &wgpu::Device,
        pipelines: &NarrowphasePipelines,
        candidate: &CandidateCase,
        requested_manifold_capacity: Option<u32>,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_storage_and_identity_policy(
            device,
            pipelines,
            candidate,
            requested_manifold_capacity,
            candidate.storage(),
            identity_policy,
        )
    }

    pub(crate) fn new_with_storage_and_identity_policy(
        device: &wgpu::Device,
        pipelines: &NarrowphasePipelines,
        candidate: &CandidateCase,
        requested_manifold_capacity: Option<u32>,
        body_storage: &crate::GpuBodyStorage,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<Self, String> {
        let candidate_capacity = candidate.candidate_capacity();
        let manifold_capacity = requested_manifold_capacity.unwrap_or(candidate_capacity);
        if manifold_capacity == 0 || manifold_capacity > candidate_capacity {
            return Err(format!(
                "manifold capacity must be between 1 and candidate capacity {candidate_capacity}"
            ));
        }
        let point_capacity = manifold_capacity
            .checked_mul(2)
            .ok_or_else(|| "narrowphase point capacity overflowed".to_owned())?;
        let block_count = candidate_capacity.div_ceil(SCAN_WORKGROUP_SIZE);
        let scratch =
            storage::<NarrowScratchRaw>(device, "narrowphase_scratch", candidate_capacity)?;
        let flags = storage::<u32>(device, "narrowphase_flags", candidate_capacity)?;
        let offsets = storage::<u32>(device, "narrowphase_offsets", candidate_capacity)?;
        let scan_blocks = storage::<u32>(device, "narrowphase_scan_blocks", block_count)?;
        let manifolds = storage::<ManifoldRaw>(device, "collision_manifolds", manifold_capacity)?;
        let points =
            storage::<ContactPointRaw>(device, "collision_contact_points", point_capacity)?;
        let manifold_capacity_buffer = storage::<CapacityRaw>(device, "manifold_capacity", 1)?;
        let point_capacity_buffer = storage::<CapacityRaw>(device, "point_capacity", 1)?;
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("collision_narrowphase_params"),
            size: size_of::<NarrowParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut mapped = params.slice(..).get_mapped_range_mut();
            mapped.copy_from_slice(bytemuck::bytes_of(&NarrowParamsRaw {
                counts: [
                    candidate_capacity,
                    manifold_capacity,
                    point_capacity,
                    block_count,
                ],
                execution: [TOTAL_DISPATCHES, 0, 0, 0],
                policy: [
                    candidate.speculative_distance(),
                    match identity_policy {
                        ContactIdentityPolicyV1::DirectMaximumSeparation => 0.0,
                        ContactIdentityPolicyV1::RelativeCoherenceBias => 1.0,
                        ContactIdentityPolicyV1::CanonicalAbsoluteBand => 2.0,
                        ContactIdentityPolicyV1::PriorContactHysteresis => 3.0,
                    },
                    0.98,
                    1.0e-3,
                ],
            }));
        }
        params.unmap();
        let clear_group = group(
            device,
            &pipelines.clear_layout,
            &[
                (6, &flags),
                (7, &offsets),
                (11, &manifold_capacity_buffer),
                (12, &point_capacity_buffer),
            ],
            &params,
        );
        let generate_group = group(
            device,
            &pipelines.generate_layout,
            &[
                (0, body_storage.pose_buffer()),
                (1, body_storage.properties_buffer()),
                (2, body_storage.metadata_buffer()),
                (3, candidate.candidate_buffer()),
                (4, candidate.candidate_capacity_buffer()),
                (5, &scratch),
                (6, &flags),
                (12, &point_capacity_buffer),
            ],
            &params,
        );
        let scan_group = group(
            device,
            &pipelines.scan_layout,
            &[(6, &flags), (7, &offsets), (8, &scan_blocks)],
            &params,
        );
        let finalize_group = group(
            device,
            &pipelines.finalize_layout,
            &[
                (4, candidate.candidate_capacity_buffer()),
                (6, &flags),
                (7, &offsets),
                (11, &manifold_capacity_buffer),
                (12, &point_capacity_buffer),
                (13, candidate.work_buffer()),
            ],
            &params,
        );
        let scatter_group = group(
            device,
            &pipelines.scatter_layout,
            &[
                (5, &scratch),
                (6, &flags),
                (7, &offsets),
                (9, &manifolds),
                (10, &points),
                (11, &manifold_capacity_buffer),
                (12, &point_capacity_buffer),
            ],
            &params,
        );
        Ok(Self {
            _scratch: scratch,
            _flags: flags,
            _offsets: offsets,
            _scan_blocks: scan_blocks,
            manifolds,
            points,
            manifold_capacity_buffer,
            point_capacity_buffer,
            params,
            clear_group,
            generate_group,
            scan_group,
            finalize_group,
            scatter_group,
            candidate_capacity,
            manifold_capacity,
            point_capacity,
            block_count,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &NarrowphasePipelines,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_collision_narrowphase_passes"),
            timestamp_writes: None,
        });
        dispatch(
            &mut pass,
            &pipelines.clear,
            &self.clear_group,
            self.candidate_capacity.max(4).div_ceil(WORKGROUP_SIZE),
        );
        dispatch(
            &mut pass,
            &pipelines.generate,
            &self.generate_group,
            self.candidate_capacity.div_ceil(WORKGROUP_SIZE),
        );
        dispatch(
            &mut pass,
            &pipelines.scan_blocks,
            &self.scan_group,
            self.block_count,
        );
        dispatch(&mut pass, &pipelines.scan_totals, &self.scan_group, 1);
        dispatch(
            &mut pass,
            &pipelines.add_offsets,
            &self.scan_group,
            self.block_count,
        );
        dispatch(&mut pass, &pipelines.finalize, &self.finalize_group, 1);
        dispatch(
            &mut pass,
            &pipelines.scatter,
            &self.scatter_group,
            self.candidate_capacity.div_ceil(WORKGROUP_SIZE),
        );
    }

    pub(crate) fn capacities(
        &self,
        manifold: CollisionCapacityV1,
        points: CollisionCapacityV1,
        narrowphase_tests: u64,
    ) -> Vec<CollisionCapacityV1> {
        let fixed = |class: &str, stride: u32, capacity: u64, used: u64| CollisionCapacityV1 {
            class: class.to_owned(),
            stride_bytes: stride,
            capacity_elements: capacity,
            attempted_elements: used,
            committed_elements: used,
            allocated_bytes: capacity * u64::from(stride),
            high_water_bytes: used * u64::from(stride),
            overflowed: false,
            accepted: true,
        };
        vec![
            manifold,
            points,
            fixed(
                "manifold-flags",
                4,
                u64::from(self.candidate_capacity),
                u64::from(self.candidate_capacity),
            ),
            fixed(
                "manifold-offsets",
                4,
                u64::from(self.candidate_capacity),
                u64::from(self.candidate_capacity),
            ),
            fixed(
                "narrowphase-scratch",
                NARROW_SCRATCH_BYTES,
                u64::from(self.candidate_capacity),
                narrowphase_tests,
            ),
            fixed(
                "narrowphase-scan-blocks",
                4,
                u64::from(self.block_count),
                u64::from(self.block_count),
            ),
        ]
    }

    pub(crate) fn manifolds(&self) -> &wgpu::Buffer {
        &self.manifolds
    }

    pub(crate) fn points(&self) -> &wgpu::Buffer {
        &self.points
    }

    pub(crate) fn manifold_capacity_buffer(&self) -> &wgpu::Buffer {
        &self.manifold_capacity_buffer
    }

    pub(crate) fn point_capacity_buffer(&self) -> &wgpu::Buffer {
        &self.point_capacity_buffer
    }

    pub(crate) fn manifold_capacity(&self) -> u32 {
        self.manifold_capacity
    }

    pub(crate) fn point_capacity(&self) -> u32 {
        self.point_capacity
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self._scratch.size()
            + self._flags.size()
            + self._offsets.size()
            + self._scan_blocks.size()
            + self.manifolds.size()
            + self.points.size()
            + self.manifold_capacity_buffer.size()
            + self.point_capacity_buffer.size()
            + self.params.size()
    }
}

pub(crate) struct TemporalIdentityStage {
    pipeline: wgpu::ComputePipeline,
    groups: [wgpu::BindGroup; 2],
    manifold_capacity: u32,
}

impl TemporalIdentityStage {
    pub(crate) fn new(
        device: &wgpu::Device,
        body_storage: &crate::GpuBodyStorage,
        narrowphase: &NarrowphaseCase,
        previous_pairs: [&wgpu::Buffer; 2],
        previous_capacities: [&wgpu::Buffer; 2],
    ) -> Self {
        let stage_layout = layout(
            device,
            "narrowphase_temporal_identity_layout",
            &[
                (0, true),
                (1, true),
                (2, true),
                (9, false),
                (10, false),
                (11, false),
                (15, true),
                (16, false),
            ],
        );
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("narrowphase_temporal_identity_pipeline_layout"),
            bind_group_layouts: &[&stage_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("narrowphase_temporal_identity_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("collision_narrowphase.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("narrowphase_temporal_identity_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("stabilize_narrowphase_identity"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[],
                zero_initialize_workgroup_memory: false,
            },
            cache: None,
        });
        let groups = [0_usize, 1_usize].map(|parity| {
            group(
                device,
                &stage_layout,
                &[
                    (0, body_storage.pose_buffer()),
                    (1, body_storage.properties_buffer()),
                    (2, body_storage.metadata_buffer()),
                    (9, narrowphase.manifolds()),
                    (10, narrowphase.points()),
                    (11, narrowphase.manifold_capacity_buffer()),
                    (15, previous_pairs[parity]),
                    (16, previous_capacities[parity]),
                ],
                &narrowphase.params,
            )
        });
        Self {
            pipeline,
            groups,
            manifold_capacity: narrowphase.manifold_capacity(),
        }
    }

    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder, parity: usize) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_temporal_contact_identity"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.groups[parity], &[]);
        pass.dispatch_workgroups(self.manifold_capacity.div_ceil(WORKGROUP_SIZE), 1, 1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn contact_point(raw: ContactPointRaw) -> Result<CollisionContactPointV1, String> {
    Ok(CollisionContactPointV1 {
        local_anchor_a: Vec2::new(raw.anchors[0], raw.anchors[1]),
        local_anchor_b: Vec2::new(raw.anchors[2], raw.anchors[3]),
        world_point: Vec2::new(raw.world_separation[0], raw.world_separation[1]),
        separation: finite(raw.world_separation[2])?,
        feature_a: feature(raw.features[0], raw.features[1])?,
        feature_b: feature(raw.features[2], raw.features[3])?,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn map_manifolds_from_readbacks(
    device: &wgpu::Device,
    manifold_readback: &wgpu::Buffer,
    point_readback: &wgpu::Buffer,
    manifold_capacity: u32,
    point_capacity: u32,
    manifold_count: u32,
    rejected: bool,
) -> Result<Vec<CollisionManifoldV1>, String> {
    let manifold_records =
        map_values::<ManifoldRaw>(device, manifold_readback, manifold_capacity as usize)?;
    let point_records =
        map_values::<ContactPointRaw>(device, point_readback, point_capacity as usize)?;
    manifolds_from_records(&manifold_records, &point_records, manifold_count, rejected)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn map_manifold_series_from_readbacks(
    device: &wgpu::Device,
    manifold_readback: &wgpu::Buffer,
    point_readback: &wgpu::Buffer,
    manifold_capacity: u32,
    point_capacity: u32,
    manifold_counts: &[u32],
    rejected: &[bool],
) -> Result<Vec<Vec<CollisionManifoldV1>>, String> {
    if manifold_counts.len() != rejected.len() {
        return Err("GPU narrowphase series count/status lengths differ".to_owned());
    }
    let manifold_records = map_values::<ManifoldRaw>(
        device,
        manifold_readback,
        manifold_capacity as usize * manifold_counts.len(),
    )?;
    let point_records = map_values::<ContactPointRaw>(
        device,
        point_readback,
        point_capacity as usize * manifold_counts.len(),
    )?;
    manifold_counts
        .iter()
        .zip(rejected)
        .enumerate()
        .map(|(index, (&count, &rejected))| {
            let manifold_start = index * manifold_capacity as usize;
            let point_start = index * point_capacity as usize;
            manifolds_from_records(
                &manifold_records[manifold_start..manifold_start + manifold_capacity as usize],
                &point_records[point_start..point_start + point_capacity as usize],
                count,
                rejected,
            )
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn manifolds_from_records(
    manifold_records: &[ManifoldRaw],
    point_records: &[ContactPointRaw],
    manifold_count: u32,
    rejected: bool,
) -> Result<Vec<CollisionManifoldV1>, String> {
    if rejected {
        return Ok(Vec::new());
    }
    let mut manifolds = Vec::with_capacity(manifold_count as usize);
    for raw in manifold_records.iter().take(manifold_count as usize) {
        let start = raw.point_range[0] as usize;
        let count = raw.point_range[1] as usize;
        if count == 0 || count > 2 || start + count > point_records.len() {
            return Err("GPU narrowphase emitted an invalid point range".to_owned());
        }
        let points = point_records[start..start + count]
            .iter()
            .copied()
            .map(contact_point)
            .collect::<Result<Vec<_>, _>>()?;
        manifolds.push(CollisionManifoldV1 {
            pair: CandidatePairV1 {
                body_a: physbox_contracts::BodyId {
                    slot: raw.identities[0],
                    generation: raw.identities[1],
                },
                body_b: physbox_contracts::BodyId {
                    slot: raw.identities[2],
                    generation: raw.identities[3],
                },
            },
            normal: Vec2::new(raw.normal_material[0], raw.normal_material[1]),
            material: MaterialV1 {
                friction: finite(raw.normal_material[2])?,
                restitution: finite(raw.normal_material[3])?,
            },
            points,
        });
    }
    Ok(manifolds)
}

#[cfg(not(target_arch = "wasm32"))]
fn feature(kind: u32, index: u32) -> Result<GeometricFeatureV1, String> {
    let kind = match kind {
        0 => GeometricFeatureKindV1::Circle,
        1 => GeometricFeatureKindV1::Face,
        2 => GeometricFeatureKindV1::Vertex,
        _ => {
            return Err(format!(
                "GPU narrowphase emitted unknown feature kind {kind}"
            ));
        }
    };
    Ok(GeometricFeatureV1 { kind, index })
}

#[cfg(not(target_arch = "wasm32"))]
fn work_record(raw: WorkRaw) -> CollisionWorkV1 {
    let value = raw.values;
    CollisionWorkV1 {
        bodies: u64::from(value[0]),
        pair_attempts: u64::from(value[1]),
        fixed_fixed_rejected: u64::from(value[2]),
        aabb_rejected: u64::from(value[3]),
        candidate_pairs: u64::from(value[4]),
        narrowphase_tests: u64::from(value[5]),
        manifolds: u64::from(value[6]),
        contact_points: u64::from(value[7]),
        sort_elements: u64::from(value[8]),
        sort_passes: u64::from(value[9]),
        cache_previous: u64::from(value[10]),
        cache_current: u64::from(value[11]),
        cache_matched: u64::from(value[12]),
        cache_cold: u64::from(value[13]),
        cache_expired: u64::from(value[14]),
        dispatches: u64::from(value[15]),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn capacity_record(class: &str, stride: u32, raw: CapacityRaw) -> CollisionCapacityV1 {
    CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes: stride,
        capacity_elements: u64::from(raw.values[0]),
        attempted_elements: u64::from(raw.values[1]),
        committed_elements: u64::from(raw.values[2]),
        allocated_bytes: u64::from(raw.values[0]) * u64::from(stride),
        high_water_bytes: u64::from(raw.values[1].min(raw.values[0])) * u64::from(stride),
        overflowed: raw.values[3] != 0,
        accepted: raw.values[3] == 0 && raw.values[2] == raw.values[1],
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn finite(value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| error.to_string())
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
        binding: 14,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<NarrowParamsRaw>() as u64),
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
        binding: 14,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_collision_narrowphase_group"),
        layout,
        entries: &entries,
    })
}

fn storage<T>(
    device: &wgpu::Device,
    label: &'static str,
    count: u32,
) -> Result<wgpu::Buffer, String> {
    let size = u64::from(count)
        .checked_mul(size_of::<T>() as u64)
        .ok_or_else(|| format!("{label} byte count overflowed"))?;
    let limits = device.limits();
    if size == 0
        || size > limits.max_buffer_size
        || size > u64::from(limits.max_storage_buffer_binding_size)
    {
        return Err(format!(
            "{label} requires {size} bytes outside portable storage limits"
        ));
    }
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    }))
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

#[cfg(not(target_arch = "wasm32"))]
fn copy(
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Buffer,
    destination: &wgpu::Buffer,
    size: u64,
) {
    encoder.copy_buffer_to_buffer(source, 0, destination, 0, size);
}

fn dispatch<'a>(
    pass: &mut wgpu::ComputePass<'a>,
    pipeline: &'a wgpu::ComputePipeline,
    group: &'a wgpu::BindGroup,
    workgroups: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups(workgroups, 1, 1);
}
