#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BODY_AABB_BYTES, BodyAabbV1, CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES,
    COLLISION_WORK_COUNTER_BYTES, CandidatePairV1, CollisionCapacityV1, CollisionWorkV1,
    FailureKindV1, FailureV1, ScenarioV1, Validate, Vec2, canonical_hash,
};

use crate::{
    AdapterDiagnostics, BODY_COMMAND_CAPACITY, GpuBodyStorage,
    broadphase_benchmark::GpuBroadphaseTimingV1,
    gpu_context::{create_instance, request_adapter, request_device_with_features},
};

const AABB_WORKGROUP_SIZE: u32 = 64;
const PAIR_WORKGROUP_SIZE: u32 = 8;
const SCAN_WORKGROUP_SIZE: u32 = 256;
const DISPATCHES: u32 = 8;
const FLAG_LIVE: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyAabbRaw {
    identity_flags: [u32; 4],
    lower_upper: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CandidatePairRaw {
    identities: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CandidateParamsRaw {
    counts: [u32; 4],
    execution: [u32; 4],
    policy: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CollisionWorkRaw {
    values: [u32; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CollisionCapacityRaw {
    values: [u32; 4],
}

const _: () = assert!(size_of::<BodyAabbRaw>() == BODY_AABB_BYTES as usize);
const _: () = assert!(size_of::<CandidatePairRaw>() == CANDIDATE_PAIR_BYTES as usize);
const _: () = assert!(size_of::<CollisionWorkRaw>() == COLLISION_WORK_COUNTER_BYTES as usize);
const _: () =
    assert!(size_of::<CollisionCapacityRaw>() == COLLISION_CAPACITY_COUNTER_BYTES as usize);

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCandidateOutput {
    pub adapter: AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub body_slot_capacity: u32,
    pub pair_attempt_capacity: u32,
    pub candidate_capacity: u32,
    pub aabbs: Vec<BodyAabbV1>,
    pub candidates: Vec<CandidatePairV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
}

impl GpuCandidateOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_scenario_hash.len() != 64
            || !self
                .source_scenario_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("GPU candidate output has an invalid source hash".to_owned());
        }
        for pair in self.aabbs.windows(2) {
            if pair[0].body >= pair[1].body {
                return Err("GPU AABBs are not sorted and unique".to_owned());
            }
        }
        for aabb in &self.aabbs {
            aabb.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.candidates.windows(2) {
            if pair[0] >= pair[1] {
                return Err("GPU candidates are not sorted and unique".to_owned());
            }
        }
        for pair in &self.candidates {
            pair.validate().map_err(|error| error.to_string())?;
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("GPU candidate capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        if let Some(failure) = &self.failure {
            failure.validate().map_err(|error| error.to_string())?;
        }
        let overflowed = self.capacities.iter().any(|capacity| capacity.overflowed);
        if overflowed != self.failure.is_some()
            || (overflowed && !self.candidates.is_empty())
            || self.candidates.len() as u64 != self.work.candidate_pairs && !overflowed
        {
            return Err("GPU candidate output failure/count state is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuCandidateBenchmarkOutput {
    pub correctness: GpuCandidateOutput,
    pub timing: GpuBroadphaseTimingV1,
    pub allocated_bytes: u64,
}

impl GpuCandidateBenchmarkOutput {
    pub fn validate(&self) -> Result<(), String> {
        self.correctness.validate()?;
        self.timing.validate()?;
        if self.allocated_bytes == 0 {
            return Err("GPU all-pairs benchmark omitted allocation bytes".to_owned());
        }
        Ok(())
    }
}

pub(crate) struct CandidatePipelines {
    clear: wgpu::ComputePipeline,
    aabb: wgpu::ComputePipeline,
    classify: wgpu::ComputePipeline,
    scan_blocks: wgpu::ComputePipeline,
    scan_totals: wgpu::ComputePipeline,
    add_offsets: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    scatter: wgpu::ComputePipeline,
    clear_layout: wgpu::BindGroupLayout,
    aabb_layout: wgpu::BindGroupLayout,
    classify_layout: wgpu::BindGroupLayout,
    scan_layout: wgpu::BindGroupLayout,
    finalize_layout: wgpu::BindGroupLayout,
    scatter_layout: wgpu::BindGroupLayout,
}

impl CandidatePipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let clear_layout = bind_group_layout(device, "candidate_clear_layout", &[4, 5, 9, 10]);
        let aabb_layout = bind_group_layout(device, "candidate_aabb_layout", &[0, 1, 2, 3]);
        let classify_layout =
            bind_group_layout(device, "candidate_classify_layout", &[2, 3, 4, 6, 9]);
        let scan_layout = bind_group_layout(device, "candidate_scan_layout", &[4, 5, 8]);
        let finalize_layout =
            bind_group_layout(device, "candidate_finalize_layout", &[4, 5, 9, 10]);
        let scatter_layout =
            bind_group_layout(device, "candidate_scatter_layout", &[4, 5, 6, 7, 10]);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_collision_candidates_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("collision_candidates.wgsl").into()),
        });
        let pipeline = |label: &'static str,
                        entry: &'static str,
                        layout: &wgpu::BindGroupLayout| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[layout],
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
        let clear = pipeline(
            "physbox5_candidate_clear_pipeline",
            "clear_candidate_state",
            &clear_layout,
        );
        let aabb = pipeline(
            "physbox5_candidate_aabb_pipeline",
            "compute_body_aabbs",
            &aabb_layout,
        );
        let classify = pipeline(
            "physbox5_candidate_classify_pipeline",
            "classify_pairs",
            &classify_layout,
        );
        let scan_blocks = pipeline(
            "physbox5_candidate_scan_blocks_pipeline",
            "scan_pair_blocks",
            &scan_layout,
        );
        let scan_totals = pipeline(
            "physbox5_candidate_scan_totals_pipeline",
            "scan_pair_block_totals",
            &scan_layout,
        );
        let add_offsets = pipeline(
            "physbox5_candidate_add_offsets_pipeline",
            "add_pair_block_offsets",
            &scan_layout,
        );
        let finalize = pipeline(
            "physbox5_candidate_finalize_pipeline",
            "finalize_candidate_counts",
            &finalize_layout,
        );
        let scatter = pipeline(
            "physbox5_candidate_scatter_pipeline",
            "scatter_candidates",
            &scatter_layout,
        );
        Self {
            clear,
            aabb,
            classify,
            scan_blocks,
            scan_totals,
            add_offsets,
            finalize,
            scatter,
            clear_layout,
            aabb_layout,
            classify_layout,
            scan_layout,
            finalize_layout,
            scatter_layout,
        }
    }
}

pub(crate) struct CandidateCase {
    owned_storage: Option<GpuBodyStorage>,
    aabbs: wgpu::Buffer,
    _pair_flags: wgpu::Buffer,
    _pair_offsets: wgpu::Buffer,
    _pair_scratch: wgpu::Buffer,
    candidates: wgpu::Buffer,
    _scan_blocks: wgpu::Buffer,
    work: wgpu::Buffer,
    capacity: wgpu::Buffer,
    _params: wgpu::Buffer,
    clear_group: wgpu::BindGroup,
    aabb_group: wgpu::BindGroup,
    classify_group: wgpu::BindGroup,
    scan_group: wgpu::BindGroup,
    finalize_group: wgpu::BindGroup,
    scatter_group: wgpu::BindGroup,
    slot_capacity: u32,
    live_count: u32,
    pair_count: u32,
    candidate_capacity: u32,
    block_count: u32,
    speculative_distance: f32,
}

impl CandidateCase {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &CandidatePipelines,
        scenario: &ScenarioV1,
        requested_candidate_capacity: Option<u32>,
    ) -> Result<Self, String> {
        let slot_capacity = scenario
            .bodies
            .last()
            .and_then(|body| body.id.slot.checked_add(1))
            .ok_or_else(|| "GPU candidate body slot capacity overflowed".to_owned())?;
        let storage = GpuBodyStorage::new(
            device,
            slot_capacity,
            BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let mut case = Self::new_with_storage(
            device,
            pipelines,
            scenario,
            requested_candidate_capacity,
            &storage,
        )?;
        case.owned_storage = Some(storage);
        Ok(case)
    }

    pub(crate) fn new_with_storage(
        device: &wgpu::Device,
        pipelines: &CandidatePipelines,
        scenario: &ScenarioV1,
        requested_candidate_capacity: Option<u32>,
        storage: &GpuBodyStorage,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        if !scenario.manifolds.is_empty()
            || !scenario.contact_cache.is_empty()
            || !scenario.actions.is_empty()
        {
            return Err(
                "GPU candidate stage requires a neutral scenario without manifolds, cache, or actions"
                    .to_owned(),
            );
        }
        if scenario.bodies.len() < 2 {
            return Err("GPU candidate stage requires at least two bodies".to_owned());
        }
        let slot_capacity = storage.layout().capacity_elements;
        let pair_count_u64 = u64::from(slot_capacity)
            .checked_mul(u64::from(slot_capacity.saturating_sub(1)))
            .map(|value| value / 2)
            .ok_or_else(|| "GPU candidate pair count overflowed".to_owned())?;
        let pair_count = u32::try_from(pair_count_u64)
            .map_err(|_| "GPU all-pairs count exceeds u32".to_owned())?;
        let candidate_capacity = requested_candidate_capacity.unwrap_or(pair_count);
        if candidate_capacity == 0 || candidate_capacity > pair_count {
            return Err(format!(
                "candidate capacity must be between 1 and pair count {pair_count}"
            ));
        }
        let block_count = pair_count.div_ceil(SCAN_WORKGROUP_SIZE);
        let maximum_dispatch = device.limits().max_compute_workgroups_per_dimension;
        let required_dispatch = pair_count
            .max(32)
            .div_ceil(AABB_WORKGROUP_SIZE)
            .max(block_count)
            .max(slot_capacity.div_ceil(PAIR_WORKGROUP_SIZE));
        if required_dispatch > maximum_dispatch {
            return Err(format!(
                "GPU all-pairs candidate stage requires {required_dispatch} workgroups in one dimension, above adapter limit {maximum_dispatch}"
            ));
        }
        let speculative_distance = scenario
            .settings
            .contact_policy
            .map_or(0.0, |policy| policy.speculative_distance.get());
        let aabbs = storage_buffer::<BodyAabbRaw>(device, "collision_aabbs", slot_capacity)?;
        let pair_flags = storage_buffer::<u32>(device, "collision_pair_flags", pair_count)?;
        let pair_offsets = storage_buffer::<u32>(device, "collision_pair_offsets", pair_count)?;
        let pair_scratch =
            storage_buffer::<CandidatePairRaw>(device, "collision_pair_scratch", pair_count)?;
        let candidates =
            storage_buffer::<CandidatePairRaw>(device, "collision_candidates", candidate_capacity)?;
        let scan_blocks = storage_buffer::<u32>(device, "collision_scan_blocks", block_count)?;
        let work = storage_buffer::<CollisionWorkRaw>(device, "collision_work", 1)?;
        let capacity = storage_buffer::<CollisionCapacityRaw>(device, "collision_capacity", 1)?;
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("collision_candidate_params"),
            size: size_of::<CandidateParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut mapped = params.slice(..).get_mapped_range_mut();
            mapped.copy_from_slice(bytemuck::bytes_of(&CandidateParamsRaw {
                counts: [slot_capacity, pair_count, candidate_capacity, block_count],
                execution: [scenario.bodies.len() as u32, DISPATCHES, 0, 0],
                policy: [speculative_distance, 0.0, 0.0, 0.0],
            }));
        }
        params.unmap();
        let clear_group = candidate_group(
            device,
            &pipelines.clear_layout,
            &[
                (4, &pair_flags),
                (5, &pair_offsets),
                (9, &work),
                (10, &capacity),
            ],
            &params,
        );
        let aabb_group = candidate_group(
            device,
            &pipelines.aabb_layout,
            &[
                (0, storage.pose_buffer()),
                (1, storage.properties_buffer()),
                (2, storage.metadata_buffer()),
                (3, &aabbs),
            ],
            &params,
        );
        let classify_group = candidate_group(
            device,
            &pipelines.classify_layout,
            &[
                (2, storage.metadata_buffer()),
                (3, &aabbs),
                (4, &pair_flags),
                (6, &pair_scratch),
                (9, &work),
            ],
            &params,
        );
        let scan_group = candidate_group(
            device,
            &pipelines.scan_layout,
            &[(4, &pair_flags), (5, &pair_offsets), (8, &scan_blocks)],
            &params,
        );
        let finalize_group = candidate_group(
            device,
            &pipelines.finalize_layout,
            &[
                (4, &pair_flags),
                (5, &pair_offsets),
                (9, &work),
                (10, &capacity),
            ],
            &params,
        );
        let scatter_group = candidate_group(
            device,
            &pipelines.scatter_layout,
            &[
                (4, &pair_flags),
                (5, &pair_offsets),
                (6, &pair_scratch),
                (7, &candidates),
                (10, &capacity),
            ],
            &params,
        );
        Ok(Self {
            owned_storage: None,
            aabbs,
            _pair_flags: pair_flags,
            _pair_offsets: pair_offsets,
            _pair_scratch: pair_scratch,
            candidates,
            _scan_blocks: scan_blocks,
            work,
            capacity,
            _params: params,
            clear_group,
            aabb_group,
            classify_group,
            scan_group,
            finalize_group,
            scatter_group,
            slot_capacity,
            live_count: scenario.bodies.len() as u32,
            pair_count,
            candidate_capacity,
            block_count,
            speculative_distance,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &CandidatePipelines,
    ) {
        self.encode_with_timestamp(encoder, pipelines, None);
    }

    pub(crate) fn encode_with_timestamp(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &CandidatePipelines,
        beginning_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        self.encode_with_timestamp_range(
            encoder,
            pipelines,
            beginning_timestamp.map(|(query_set, beginning)| (query_set, Some(beginning), None)),
        );
    }

    pub(crate) fn encode_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &CandidatePipelines,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        self.encode_with_timestamp_range(
            encoder,
            pipelines,
            timestamps.map(|(query_set, beginning, end)| (query_set, Some(beginning), Some(end))),
        );
    }

    fn encode_with_timestamp_range(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &CandidatePipelines,
        timestamps: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_collision_candidate_passes"),
            timestamp_writes: timestamps.map(|(query_set, beginning, end)| {
                wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: beginning,
                    end_of_pass_write_index: end,
                }
            }),
        });
        dispatch(
            &mut pass,
            &pipelines.clear,
            &self.clear_group,
            self.pair_count.max(32).div_ceil(AABB_WORKGROUP_SIZE),
            1,
        );
        dispatch(
            &mut pass,
            &pipelines.aabb,
            &self.aabb_group,
            self.slot_capacity.div_ceil(AABB_WORKGROUP_SIZE),
            1,
        );
        dispatch(
            &mut pass,
            &pipelines.classify,
            &self.classify_group,
            self.slot_capacity.div_ceil(PAIR_WORKGROUP_SIZE),
            self.slot_capacity.div_ceil(PAIR_WORKGROUP_SIZE),
        );
        dispatch(
            &mut pass,
            &pipelines.scan_blocks,
            &self.scan_group,
            self.block_count,
            1,
        );
        dispatch(&mut pass, &pipelines.scan_totals, &self.scan_group, 1, 1);
        dispatch(
            &mut pass,
            &pipelines.add_offsets,
            &self.scan_group,
            self.block_count,
            1,
        );
        dispatch(&mut pass, &pipelines.finalize, &self.finalize_group, 1, 1);
        dispatch(
            &mut pass,
            &pipelines.scatter,
            &self.scatter_group,
            self.pair_count.div_ceil(AABB_WORKGROUP_SIZE),
            1,
        );
    }

    pub(crate) fn capacities(&self, candidate: CollisionCapacityV1) -> Vec<CollisionCapacityV1> {
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
        let mut capacities = vec![
            fixed(
                "body-aabbs",
                BODY_AABB_BYTES,
                u64::from(self.slot_capacity),
                u64::from(self.live_count),
            ),
            candidate,
            fixed(
                "pair-flags",
                4,
                u64::from(self.pair_count),
                u64::from(self.pair_count),
            ),
            fixed(
                "pair-offsets",
                4,
                u64::from(self.pair_count),
                u64::from(self.pair_count),
            ),
            fixed(
                "pair-scratch",
                CANDIDATE_PAIR_BYTES,
                u64::from(self.pair_count),
                u64::from(self.pair_count),
            ),
            fixed(
                "scan-blocks",
                4,
                u64::from(self.block_count),
                u64::from(self.block_count),
            ),
            fixed("work-counters", COLLISION_WORK_COUNTER_BYTES, 1, 1),
        ];
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        capacities
    }

    pub(crate) fn storage(&self) -> &GpuBodyStorage {
        self.owned_storage
            .as_ref()
            .expect("validation candidate owns its body storage")
    }

    pub(crate) fn candidate_buffer(&self) -> &wgpu::Buffer {
        &self.candidates
    }

    pub(crate) fn candidate_capacity_buffer(&self) -> &wgpu::Buffer {
        &self.capacity
    }

    pub(crate) fn work_buffer(&self) -> &wgpu::Buffer {
        &self.work
    }

    pub(crate) fn candidate_capacity(&self) -> u32 {
        self.candidate_capacity
    }

    pub(crate) fn speculative_distance(&self) -> f32 {
        self.speculative_distance
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.aabbs.size()
            + self._pair_flags.size()
            + self._pair_offsets.size()
            + self._pair_scratch.size()
            + self.candidates.size()
            + self._scan_blocks.size()
            + self.work.size()
            + self.capacity.size()
            + self._params.size()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_collision_candidates(
    scenario: &ScenarioV1,
    candidate_capacity: Option<u32>,
) -> Result<GpuCandidateOutput, String> {
    GpuCandidateRunner::new()
        .await?
        .run(scenario, candidate_capacity)
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuCandidateRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    pipelines: CandidatePipelines,
    timestamps_available: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuCandidateRunner {
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
        let pipelines = CandidatePipelines::new(&device);
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
        scenario: &ScenarioV1,
        candidate_capacity: Option<u32>,
    ) -> Result<GpuCandidateOutput, String> {
        let device = &self.device;
        let case = CandidateCase::new(device, &self.pipelines, scenario, candidate_capacity)?;
        let aabb_readback = readback_buffer(
            device,
            "collision_aabb_readback",
            u64::from(case.slot_capacity) * u64::from(BODY_AABB_BYTES),
        );
        let candidate_readback = readback_buffer(
            device,
            "collision_candidate_readback",
            u64::from(case.candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        let work_readback = readback_buffer(
            device,
            "collision_work_readback",
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        let capacity_readback = readback_buffer(
            device,
            "collision_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_collision_candidate_encoder"),
        });
        case.encode(&mut encoder, &self.pipelines);
        encoder.copy_buffer_to_buffer(
            &case.aabbs,
            0,
            &aabb_readback,
            0,
            u64::from(case.slot_capacity) * u64::from(BODY_AABB_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &case.candidates,
            0,
            &candidate_readback,
            0,
            u64::from(case.candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &case.work,
            0,
            &work_readback,
            0,
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &case.capacity,
            0,
            &capacity_readback,
            0,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        self.queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU candidate poll failed: {error}"))?;

        let aabb_raw =
            map_values::<BodyAabbRaw>(device, &aabb_readback, case.slot_capacity as usize)?;
        let capacity_raw = map_values::<CollisionCapacityRaw>(device, &capacity_readback, 1)?[0];
        let committed = capacity_raw.values[2] as usize;
        let candidate_raw = map_values::<CandidatePairRaw>(
            device,
            &candidate_readback,
            case.candidate_capacity as usize,
        )?;
        let work_raw = map_values::<CollisionWorkRaw>(device, &work_readback, 1)?[0];
        let aabbs = aabb_raw
            .into_iter()
            .filter(|raw| raw.identity_flags[2] & FLAG_LIVE != 0)
            .map(|raw| BodyAabbV1 {
                body: physbox_contracts::BodyId {
                    slot: raw.identity_flags[0],
                    generation: raw.identity_flags[1],
                },
                lower: Vec2::new(raw.lower_upper[0], raw.lower_upper[1]),
                upper: Vec2::new(raw.lower_upper[2], raw.lower_upper[3]),
            })
            .collect::<Vec<_>>();
        let candidates = candidate_raw
            .into_iter()
            .take(committed)
            .map(|raw| CandidatePairV1 {
                body_a: physbox_contracts::BodyId {
                    slot: raw.identities[0],
                    generation: raw.identities[1],
                },
                body_b: physbox_contracts::BodyId {
                    slot: raw.identities[2],
                    generation: raw.identities[3],
                },
            })
            .collect::<Vec<_>>();
        let values = work_raw.values;
        let work = CollisionWorkV1 {
            bodies: u64::from(values[0]),
            pair_attempts: u64::from(values[1]),
            fixed_fixed_rejected: u64::from(values[2]),
            aabb_rejected: u64::from(values[3]),
            candidate_pairs: u64::from(values[4]),
            narrowphase_tests: u64::from(values[5]),
            manifolds: u64::from(values[6]),
            contact_points: u64::from(values[7]),
            sort_elements: u64::from(values[8]),
            sort_passes: u64::from(values[9]),
            cache_previous: u64::from(values[10]),
            cache_current: u64::from(values[11]),
            cache_matched: u64::from(values[12]),
            cache_cold: u64::from(values[13]),
            cache_expired: u64::from(values[14]),
            dispatches: u64::from(values[15]),
        };
        let raw = capacity_raw.values;
        let candidate_capacity = CollisionCapacityV1 {
            class: "candidate-pairs".to_owned(),
            stride_bytes: CANDIDATE_PAIR_BYTES,
            capacity_elements: u64::from(raw[0]),
            attempted_elements: u64::from(raw[1]),
            committed_elements: u64::from(raw[2]),
            allocated_bytes: u64::from(raw[0]) * u64::from(CANDIDATE_PAIR_BYTES),
            high_water_bytes: u64::from(raw[1].min(raw[0])) * u64::from(CANDIDATE_PAIR_BYTES),
            overflowed: raw[3] != 0,
            accepted: raw[3] == 0,
        };
        let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
        let failure = candidate_capacity.overflowed.then(|| FailureV1 {
            kind: FailureKindV1::CapacityOverflow,
            reason: format!(
                "candidate capacity {} rejected {} attempted pairs",
                candidate_capacity.capacity_elements, candidate_capacity.attempted_elements
            ),
            first_step: Some(0),
            first_stage: Some("candidate".to_owned()),
            reproducer_hash: Some(source_scenario_hash.clone()),
        });
        let output = GpuCandidateOutput {
            adapter: self.diagnostics.clone(),
            source_scenario_hash,
            body_slot_capacity: case.slot_capacity,
            pair_attempt_capacity: case.pair_count,
            candidate_capacity: case.candidate_capacity,
            aabbs,
            candidates,
            work,
            capacities: case.capacities(candidate_capacity),
            failure,
        };
        output.validate()?;
        Ok(output)
    }

    pub fn benchmark(
        &self,
        scenario: &ScenarioV1,
        candidate_capacity: Option<u32>,
        warmup_steps: u32,
        sample_steps: u32,
    ) -> Result<GpuCandidateBenchmarkOutput, String> {
        let case = CandidateCase::new(&self.device, &self.pipelines, scenario, candidate_capacity)?;
        let allocated_bytes = case.allocated_bytes();
        let timing = crate::broadphase_benchmark::benchmark_gpu_case(
            &self.device,
            &self.queue,
            self.timestamps_available,
            warmup_steps,
            sample_steps,
            |encoder, timestamps| {
                case.encode_with_timestamps(encoder, &self.pipelines, timestamps);
            },
        )?;
        let output = GpuCandidateBenchmarkOutput {
            correctness: self.run(scenario, candidate_capacity)?,
            timing,
            allocated_bytes,
        };
        output.validate()?;
        Ok(output)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn map_values<T: Pod + Copy>(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    count: usize,
) -> Result<Vec<T>, String> {
    use std::sync::mpsc;

    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or_else(|| "GPU candidate readback byte count overflowed".to_owned())?;
    let slice = buffer.slice(..bytes as u64);
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("GPU candidate readback poll failed: {error}"))?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("GPU candidate map failed: {error}"))?;
    let mapped = slice.get_mapped_range();
    let values = bytemuck::cast_slice::<u8, T>(&mapped).to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(values)
}

fn bind_group_layout(
    device: &wgpu::Device,
    label: &'static str,
    storage_bindings: &[u32],
) -> wgpu::BindGroupLayout {
    let mut entries = storage_bindings
        .iter()
        .map(|binding| wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect::<Vec<_>>();
    for binding in [0, 1, 2] {
        if storage_bindings.contains(&binding) {
            entries
                .iter_mut()
                .find(|entry| entry.binding == binding)
                .expect("storage entry exists")
                .ty = wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            };
        }
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 11,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<CandidateParamsRaw>() as u64),
        },
        count: None,
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn candidate_group(
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
        binding: 11,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_collision_candidate_group"),
        layout,
        entries: &entries,
    })
}

fn storage_buffer<T>(
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
fn readback_buffer(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn dispatch<'a>(
    pass: &mut wgpu::ComputePass<'a>,
    pipeline: &'a wgpu::ComputePipeline,
    bind_group: &'a wgpu::BindGroup,
    workgroups_x: u32,
    workgroups_y: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use crate::{P5_2CollisionScenarioKind, p5_2_collision_scenario};

    use super::*;

    #[test]
    fn complete_candidate_stage_runs_and_rejects_overflow_atomically() {
        let scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17).unwrap();
        let runner = pollster::block_on(GpuCandidateRunner::new()).unwrap();
        let output = runner.run(&scenario, None).unwrap();
        assert_eq!(output.work.bodies, 6);
        assert_eq!(output.work.pair_attempts, 15);
        assert!(!output.candidates.is_empty());
        assert!(output.failure.is_none());

        let overflow = runner.run(&scenario, Some(1)).unwrap();
        assert!(overflow.candidates.is_empty());
        assert_eq!(overflow.work.candidate_pairs, output.work.candidate_pairs);
        assert_eq!(
            overflow.failure.unwrap().kind,
            FailureKindV1::CapacityOverflow
        );
    }
}
