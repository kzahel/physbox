use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES, COLLISION_CONTACT_POINT_BYTES,
    COLLISION_MANIFOLD_BYTES, COLLISION_WORK_COUNTER_BYTES, CandidatePairV1, CollisionCapacityV1,
    CollisionWorkV1, ContactCacheValueV1, FailureKindV1, FailureV1, FiniteF32,
    GeometricFeatureKindV1, GeometricFeatureV1, PairSlotAssignmentV1, PersistentContactKeyV1,
    PersistentContactV1, ScenarioV1, Validate, canonical_hash,
};
use wgpu::util::DeviceExt;

use crate::{
    AdapterDiagnostics,
    collision_candidates::{CandidateCase, CandidatePipelines},
    collision_narrowphase::{NarrowphaseCase, NarrowphasePipelines},
    collision_persistence::{CapacityRaw, WorkRaw, capacity_record, work_record},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    collision_candidates::map_values,
    gpu_context::{create_instance, request_adapter, request_device},
};

const WORKGROUP_SIZE: u32 = 64;
const SCAN_WORKGROUP_SIZE: u32 = 256;
const PAIR_SLOT_DISPATCHES: u32 = 10;
const TOTAL_DISPATCHES: u32 = 15 + PAIR_SLOT_DISPATCHES;
const PAIR_RECORD_BYTES: u32 = 96;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PairRecordRaw {
    key: [u32; 4],
    meta: [u32; 4],
    features0: [u32; 4],
    cache0: [u32; 4],
    features1: [u32; 4],
    cache1: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PairSlotParamsRaw {
    counts: [u32; 4],
    control: [u32; 4],
}

const _: () = assert!(size_of::<PairRecordRaw>() == PAIR_RECORD_BYTES as usize);

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPairSlotPersistenceStepOutput {
    pub completed_step: u32,
    pub source_scenario_hash: String,
    pub pair_slots: Vec<PairSlotAssignmentV1>,
    pub cache: Vec<PersistentContactV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
}

impl GpuPairSlotPersistenceStepOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_scenario_hash.len() != 64
            || !self
                .source_scenario_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("GPU pair-slot step has an invalid source hash".to_owned());
        }
        for assignment in &self.pair_slots {
            assignment.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.pair_slots.windows(2) {
            if pair[0].pair >= pair[1].pair {
                return Err("GPU pair-slot assignments are not sorted and unique".to_owned());
            }
        }
        let mut slots = self
            .pair_slots
            .iter()
            .map(|assignment| assignment.slot)
            .collect::<Vec<_>>();
        slots.sort_unstable();
        if slots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("GPU pair-slot assignments reuse an active slot".to_owned());
        }
        for entry in &self.cache {
            entry.validate().map_err(|error| error.to_string())?;
            if entry.value.last_seen_step != self.completed_step {
                return Err("GPU pair-slot cache has stale point state".to_owned());
            }
        }
        for pair in self.cache.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err("GPU pair-slot cache is not sorted and unique".to_owned());
            }
        }
        let mut cache_pairs = Vec::new();
        for entry in &self.cache {
            if cache_pairs.last() != Some(&entry.key.pair) {
                cache_pairs.push(entry.key.pair);
            }
        }
        if cache_pairs
            != self
                .pair_slots
                .iter()
                .map(|assignment| assignment.pair)
                .collect::<Vec<_>>()
        {
            return Err("GPU pair-slot assignments do not cover the canonical cache".to_owned());
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("GPU pair-slot capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        if let Some(failure) = &self.failure {
            failure.validate().map_err(|error| error.to_string())?;
        }
        let rejected = self.capacities.iter().any(|capacity| !capacity.accepted);
        if rejected != self.failure.is_some()
            || (rejected && (!self.cache.is_empty() || !self.pair_slots.is_empty()))
            || (!rejected
                && (self.cache.len() as u64 != self.work.cache_current
                    || self.pair_slots.len() as u64 != self.work.manifolds))
        {
            return Err("GPU pair-slot output/failure state is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPairSlotPersistenceOutput {
    pub adapter: AdapterDiagnostics,
    pub pair_capacity: u32,
    pub mapped_after_final_step_only: bool,
    pub input_permuted_before_lookup: bool,
    pub fingerprint_mask: u32,
    pub steps: Vec<GpuPairSlotPersistenceStepOutput>,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuPairSlotPersistenceRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    candidate_pipelines: CandidatePipelines,
    narrowphase_pipelines: NarrowphasePipelines,
    pair_slot_pipelines: PairSlotPipelines,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuPairSlotPersistenceRunner {
    pub async fn new() -> Result<Self, String> {
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        Ok(Self {
            candidate_pipelines: CandidatePipelines::new(&device),
            narrowphase_pipelines: NarrowphasePipelines::new(&device),
            pair_slot_pipelines: PairSlotPipelines::new(&device),
            device,
            queue,
            diagnostics,
        })
    }

    pub fn adapter(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn run_sequence(
        &self,
        scenarios: &[ScenarioV1],
        initial_cache: &[PersistentContactV1],
        requested_pair_capacity: Option<u32>,
        permute_current_input: bool,
        fingerprint_mask: u32,
    ) -> Result<GpuPairSlotPersistenceOutput, String> {
        if scenarios.is_empty() {
            return Err("GPU pair-slot sequence requires at least one step".to_owned());
        }
        for scenario in scenarios {
            scenario.validate().map_err(|error| error.to_string())?;
        }
        validate_initial_cache(initial_cache)?;

        let mut candidate_cases = Vec::with_capacity(scenarios.len());
        let mut narrowphase_cases = Vec::with_capacity(scenarios.len());
        let mut maximum_pair_capacity = 0_u32;
        for scenario in scenarios {
            let candidate =
                CandidateCase::new(&self.device, &self.candidate_pipelines, scenario, None)?;
            let narrowphase =
                NarrowphaseCase::new(&self.device, &self.narrowphase_pipelines, &candidate, None)?;
            maximum_pair_capacity = maximum_pair_capacity.max(narrowphase.manifold_capacity());
            candidate_cases.push(candidate);
            narrowphase_cases.push(narrowphase);
        }
        let initial_records = pair_records(initial_cache, fingerprint_mask)?;
        let pair_capacity = requested_pair_capacity.unwrap_or(maximum_pair_capacity);
        if pair_capacity == 0 || initial_records.len() > pair_capacity as usize {
            return Err(format!(
                "pair capacity {pair_capacity} cannot hold {} initial pairs",
                initial_records.len()
            ));
        }

        let mut padded_initial = initial_records.clone();
        padded_initial.resize(pair_capacity as usize, invalid_pair());
        let empty_records = vec![invalid_pair(); pair_capacity as usize];
        let pair_buffers = [
            storage_init(&self.device, "pair_slot_cache_a", &padded_initial),
            storage_init(&self.device, "pair_slot_cache_b", &empty_records),
        ];
        let capacity_buffers = [
            storage_init(
                &self.device,
                "pair_slot_capacity_a",
                &[CapacityRaw {
                    values: [
                        pair_capacity,
                        initial_records.len() as u32,
                        initial_records.len() as u32,
                        0,
                    ],
                }],
            ),
            storage::<CapacityRaw>(&self.device, "pair_slot_capacity_b", 1)?,
        ];
        let mut generations = vec![0_u32; pair_capacity as usize];
        for record in &initial_records {
            generations[record.meta[0] as usize] = record.meta[1];
        }
        let slot_generations = storage_init(&self.device, "pair_slot_generations", &generations);
        let mut previous_index = 0_usize;
        let mut output_index = 1_usize;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_pair_slot_persistence_sequence"),
            });
        let mut readbacks = Vec::with_capacity(scenarios.len());

        for (index, ((scenario, candidate), narrowphase)) in scenarios
            .iter()
            .zip(&candidate_cases)
            .zip(&narrowphase_cases)
            .enumerate()
        {
            let completed_step = index as u32 + 1;
            let pair_slot = PairSlotCase::new(
                &self.device,
                &self.pair_slot_pipelines,
                narrowphase,
                &pair_buffers[previous_index],
                &capacity_buffers[previous_index],
                &pair_buffers[output_index],
                &capacity_buffers[output_index],
                &slot_generations,
                candidate.work_buffer(),
                pair_capacity,
                completed_step,
                permute_current_input,
                fingerprint_mask,
            )?;
            candidate.encode(&mut encoder, &self.candidate_pipelines);
            narrowphase.encode(&mut encoder, &self.narrowphase_pipelines);
            pair_slot.encode(&mut encoder, &self.pair_slot_pipelines);
            let readback = PairSlotReadback::new(&self.device, pair_capacity);
            readback.encode(
                &mut encoder,
                candidate,
                narrowphase,
                &pair_buffers[output_index],
                &capacity_buffers[output_index],
            );
            readbacks.push((
                completed_step,
                canonical_hash(scenario).map_err(|error| error.to_string())?,
                readback,
                pair_slot.capacities(),
            ));
            std::mem::swap(&mut previous_index, &mut output_index);
        }

        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU pair-slot poll failed: {error}"))?;

        let mut steps = Vec::with_capacity(readbacks.len());
        for (index, (completed_step, source_hash, readback, scratch_capacities)) in
            readbacks.into_iter().enumerate()
        {
            steps.push(readback.map(
                &self.device,
                &candidate_cases[index],
                &narrowphase_cases[index],
                completed_step,
                source_hash,
                pair_capacity,
                scratch_capacities,
            )?);
        }
        Ok(GpuPairSlotPersistenceOutput {
            adapter: self.diagnostics.clone(),
            pair_capacity,
            mapped_after_final_step_only: true,
            input_permuted_before_lookup: permute_current_input,
            fingerprint_mask,
            steps,
        })
    }
}

pub(crate) struct PairSlotPipelines {
    clear: wgpu::ComputePipeline,
    prepare: wgpu::ComputePipeline,
    match_current: wgpu::ComputePipeline,
    classify: wgpu::ComputePipeline,
    scan_blocks: wgpu::ComputePipeline,
    scan_totals: wgpu::ComputePipeline,
    add_offsets: wgpu::ComputePipeline,
    scatter_free: wgpu::ComputePipeline,
    assign_new: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    clear_layout: wgpu::BindGroupLayout,
    prepare_layout: wgpu::BindGroupLayout,
    match_layout: wgpu::BindGroupLayout,
    classify_layout: wgpu::BindGroupLayout,
    scan_layout: wgpu::BindGroupLayout,
    scatter_layout: wgpu::BindGroupLayout,
    assign_layout: wgpu::BindGroupLayout,
    finalize_layout: wgpu::BindGroupLayout,
}

impl PairSlotPipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let clear_layout = layout(
            device,
            "pair_slot_clear_layout",
            &[
                (8, false),
                (9, false),
                (10, false),
                (11, false),
                (12, false),
                (13, false),
                (16, false),
                (17, false),
            ],
        );
        let prepare_layout = layout(
            device,
            "pair_slot_prepare_layout",
            &[(2, false), (3, false), (5, false), (16, false)],
        );
        let match_layout = layout(
            device,
            "pair_slot_match_layout",
            &[
                (0, true),
                (1, true),
                (4, true),
                (5, false),
                (6, false),
                (15, false),
                (16, false),
                (17, false),
            ],
        );
        let classify_layout = layout(
            device,
            "pair_slot_classify_layout",
            &[(6, false), (8, false), (10, false), (16, false)],
        );
        let scan_layout = layout(
            device,
            "pair_slot_scan_layout",
            &[
                (8, false),
                (9, false),
                (10, false),
                (11, false),
                (12, false),
                (13, false),
            ],
        );
        let scatter_layout = layout(
            device,
            "pair_slot_scatter_layout",
            &[(10, false), (11, false), (14, false)],
        );
        let assign_layout = layout(
            device,
            "pair_slot_assign_layout",
            &[
                (6, false),
                (8, false),
                (9, false),
                (14, false),
                (15, false),
                (16, false),
            ],
        );
        let finalize_layout = layout(
            device,
            "pair_slot_finalize_layout",
            &[(3, false), (4, true), (5, false), (16, false), (17, false)],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_pair_slot_persistence_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("pair_slot_persistence.wgsl").into()),
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
                "pair_slot_clear_pipeline",
                "clear_pair_slots",
                &clear_layout,
            ),
            prepare: pipeline(
                "pair_slot_prepare_pipeline",
                "prepare_pair_slots",
                &prepare_layout,
            ),
            match_current: pipeline(
                "pair_slot_match_pipeline",
                "match_current_pairs",
                &match_layout,
            ),
            classify: pipeline(
                "pair_slot_classify_pipeline",
                "classify_pair_slots",
                &classify_layout,
            ),
            scan_blocks: pipeline(
                "pair_slot_scan_blocks_pipeline",
                "scan_pair_slot_blocks",
                &scan_layout,
            ),
            scan_totals: pipeline(
                "pair_slot_scan_totals_pipeline",
                "scan_pair_slot_totals",
                &scan_layout,
            ),
            add_offsets: pipeline(
                "pair_slot_add_offsets_pipeline",
                "add_pair_slot_offsets",
                &scan_layout,
            ),
            scatter_free: pipeline(
                "pair_slot_scatter_free_pipeline",
                "scatter_free_slots",
                &scatter_layout,
            ),
            assign_new: pipeline(
                "pair_slot_assign_new_pipeline",
                "assign_new_pairs",
                &assign_layout,
            ),
            finalize: pipeline(
                "pair_slot_finalize_pipeline",
                "finalize_pair_slots",
                &finalize_layout,
            ),
            clear_layout,
            prepare_layout,
            match_layout,
            classify_layout,
            scan_layout,
            scatter_layout,
            assign_layout,
            finalize_layout,
        }
    }
}

struct PairSlotCase {
    _new_flags: wgpu::Buffer,
    _new_offsets: wgpu::Buffer,
    _slot_flags: wgpu::Buffer,
    _slot_offsets: wgpu::Buffer,
    _new_blocks: wgpu::Buffer,
    _free_blocks: wgpu::Buffer,
    _free_slots: wgpu::Buffer,
    _params: wgpu::Buffer,
    clear_group: wgpu::BindGroup,
    prepare_group: wgpu::BindGroup,
    match_group: wgpu::BindGroup,
    classify_group: wgpu::BindGroup,
    scan_group: wgpu::BindGroup,
    scatter_group: wgpu::BindGroup,
    assign_group: wgpu::BindGroup,
    finalize_group: wgpu::BindGroup,
    pair_capacity: u32,
    manifold_capacity: u32,
    block_count: u32,
}

impl PairSlotCase {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        pipelines: &PairSlotPipelines,
        narrowphase: &NarrowphaseCase,
        previous_pairs: &wgpu::Buffer,
        previous_capacity: &wgpu::Buffer,
        output_pairs: &wgpu::Buffer,
        output_capacity: &wgpu::Buffer,
        slot_generations: &wgpu::Buffer,
        work: &wgpu::Buffer,
        pair_capacity: u32,
        completed_step: u32,
        permute_current_input: bool,
        fingerprint_mask: u32,
    ) -> Result<Self, String> {
        let block_count = pair_capacity.div_ceil(SCAN_WORKGROUP_SIZE);
        let new_flags = storage::<u32>(device, "pair_slot_new_flags", pair_capacity)?;
        let new_offsets = storage::<u32>(device, "pair_slot_new_offsets", pair_capacity)?;
        let slot_flags = storage::<u32>(device, "pair_slot_active_flags", pair_capacity)?;
        let slot_offsets = storage::<u32>(device, "pair_slot_free_offsets", pair_capacity)?;
        let new_blocks = storage::<u32>(device, "pair_slot_new_blocks", block_count)?;
        let free_blocks = storage::<u32>(device, "pair_slot_free_blocks", block_count)?;
        let free_slots = storage::<u32>(device, "pair_slot_free_slots", pair_capacity)?;
        let params = uniform(
            device,
            "pair_slot_params",
            &PairSlotParamsRaw {
                counts: [
                    pair_capacity,
                    block_count,
                    completed_step,
                    narrowphase.manifold_capacity(),
                ],
                control: [
                    TOTAL_DISPATCHES,
                    u32::from(permute_current_input),
                    fingerprint_mask,
                    0,
                ],
            },
        );
        let clear_group = group(
            device,
            &pipelines.clear_layout,
            &[
                (8, &new_flags),
                (9, &new_offsets),
                (10, &slot_flags),
                (11, &slot_offsets),
                (12, &new_blocks),
                (13, &free_blocks),
                (16, output_capacity),
                (17, work),
            ],
            &params,
        );
        let prepare_group = group(
            device,
            &pipelines.prepare_layout,
            &[
                (2, narrowphase.manifold_capacity_buffer()),
                (3, narrowphase.point_capacity_buffer()),
                (5, previous_capacity),
                (16, output_capacity),
            ],
            &params,
        );
        let match_group = group(
            device,
            &pipelines.match_layout,
            &[
                (0, narrowphase.manifolds()),
                (1, narrowphase.points()),
                (4, previous_pairs),
                (5, previous_capacity),
                (6, output_pairs),
                (15, slot_generations),
                (16, output_capacity),
                (17, work),
            ],
            &params,
        );
        let classify_group = group(
            device,
            &pipelines.classify_layout,
            &[
                (6, output_pairs),
                (8, &new_flags),
                (10, &slot_flags),
                (16, output_capacity),
            ],
            &params,
        );
        let scan_group = group(
            device,
            &pipelines.scan_layout,
            &[
                (8, &new_flags),
                (9, &new_offsets),
                (10, &slot_flags),
                (11, &slot_offsets),
                (12, &new_blocks),
                (13, &free_blocks),
            ],
            &params,
        );
        let scatter_group = group(
            device,
            &pipelines.scatter_layout,
            &[(10, &slot_flags), (11, &slot_offsets), (14, &free_slots)],
            &params,
        );
        let assign_group = group(
            device,
            &pipelines.assign_layout,
            &[
                (6, output_pairs),
                (8, &new_flags),
                (9, &new_offsets),
                (14, &free_slots),
                (15, slot_generations),
                (16, output_capacity),
            ],
            &params,
        );
        let finalize_group = group(
            device,
            &pipelines.finalize_layout,
            &[
                (3, narrowphase.point_capacity_buffer()),
                (4, previous_pairs),
                (5, previous_capacity),
                (16, output_capacity),
                (17, work),
            ],
            &params,
        );
        Ok(Self {
            _new_flags: new_flags,
            _new_offsets: new_offsets,
            _slot_flags: slot_flags,
            _slot_offsets: slot_offsets,
            _new_blocks: new_blocks,
            _free_blocks: free_blocks,
            _free_slots: free_slots,
            _params: params,
            clear_group,
            prepare_group,
            match_group,
            classify_group,
            scan_group,
            scatter_group,
            assign_group,
            finalize_group,
            pair_capacity,
            manifold_capacity: narrowphase.manifold_capacity(),
            block_count,
        })
    }

    fn encode(&self, encoder: &mut wgpu::CommandEncoder, pipelines: &PairSlotPipelines) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_pair_slot_persistence_passes"),
            timestamp_writes: None,
        });
        dispatch(
            &mut pass,
            &pipelines.clear,
            &self.clear_group,
            self.pair_capacity
                .max(self.block_count)
                .max(15)
                .div_ceil(WORKGROUP_SIZE),
        );
        dispatch(&mut pass, &pipelines.prepare, &self.prepare_group, 1);
        dispatch(
            &mut pass,
            &pipelines.match_current,
            &self.match_group,
            self.manifold_capacity.div_ceil(WORKGROUP_SIZE),
        );
        dispatch(
            &mut pass,
            &pipelines.classify,
            &self.classify_group,
            self.pair_capacity.div_ceil(WORKGROUP_SIZE),
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
        dispatch(
            &mut pass,
            &pipelines.scatter_free,
            &self.scatter_group,
            self.pair_capacity.div_ceil(WORKGROUP_SIZE),
        );
        dispatch(
            &mut pass,
            &pipelines.assign_new,
            &self.assign_group,
            self.pair_capacity.div_ceil(WORKGROUP_SIZE),
        );
        dispatch(&mut pass, &pipelines.finalize, &self.finalize_group, 1);
    }

    fn capacities(&self) -> Vec<CollisionCapacityV1> {
        let capacity = u64::from(self.pair_capacity);
        let blocks = u64::from(self.block_count);
        let mut capacities = vec![
            fixed_capacity("pair-slot-active-flags", 4, capacity, capacity),
            fixed_capacity("pair-slot-free-offsets", 4, capacity, capacity),
            fixed_capacity("pair-slot-free-slots", 4, capacity, capacity),
            fixed_capacity("pair-slot-new-flags", 4, capacity, capacity),
            fixed_capacity("pair-slot-new-offsets", 4, capacity, capacity),
            fixed_capacity("pair-slot-scan-blocks", 4, blocks * 2, blocks * 2),
        ];
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        capacities
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PairSlotReadback {
    pairs: wgpu::Buffer,
    candidate_capacity: wgpu::Buffer,
    manifold_capacity: wgpu::Buffer,
    point_capacity: wgpu::Buffer,
    pair_capacity: wgpu::Buffer,
    work: wgpu::Buffer,
}

#[cfg(not(target_arch = "wasm32"))]
impl PairSlotReadback {
    fn new(device: &wgpu::Device, pair_capacity: u32) -> Self {
        Self {
            pairs: readback(
                device,
                "pair_slot_pairs_readback",
                u64::from(pair_capacity) * u64::from(PAIR_RECORD_BYTES),
            ),
            candidate_capacity: readback(
                device,
                "pair_slot_candidate_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            manifold_capacity: readback(
                device,
                "pair_slot_manifold_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            point_capacity: readback(
                device,
                "pair_slot_point_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            pair_capacity: readback(
                device,
                "pair_slot_output_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            work: readback(
                device,
                "pair_slot_work_readback",
                u64::from(COLLISION_WORK_COUNTER_BYTES),
            ),
        }
    }

    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        pairs: &wgpu::Buffer,
        pair_capacity: &wgpu::Buffer,
    ) {
        copy(encoder, pairs, &self.pairs, self.pairs.size());
        copy(
            encoder,
            candidate.candidate_capacity_buffer(),
            &self.candidate_capacity,
            self.candidate_capacity.size(),
        );
        copy(
            encoder,
            narrowphase.manifold_capacity_buffer(),
            &self.manifold_capacity,
            self.manifold_capacity.size(),
        );
        copy(
            encoder,
            narrowphase.point_capacity_buffer(),
            &self.point_capacity,
            self.point_capacity.size(),
        );
        copy(
            encoder,
            pair_capacity,
            &self.pair_capacity,
            self.pair_capacity.size(),
        );
        copy(
            encoder,
            candidate.work_buffer(),
            &self.work,
            self.work.size(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn map(
        self,
        device: &wgpu::Device,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        completed_step: u32,
        source_scenario_hash: String,
        pair_capacity: u32,
        mut scratch_capacities: Vec<CollisionCapacityV1>,
    ) -> Result<GpuPairSlotPersistenceStepOutput, String> {
        let candidate_raw = map_values::<CapacityRaw>(device, &self.candidate_capacity, 1)?[0];
        let manifold_raw = map_values::<CapacityRaw>(device, &self.manifold_capacity, 1)?[0];
        let point_raw = map_values::<CapacityRaw>(device, &self.point_capacity, 1)?[0];
        let pair_raw = map_values::<CapacityRaw>(device, &self.pair_capacity, 1)?[0];
        let work = work_record(map_values::<WorkRaw>(device, &self.work, 1)?[0]);
        let rejected = candidate_raw.values[3] != 0
            || manifold_raw.values[2] != manifold_raw.values[1]
            || point_raw.values[2] != point_raw.values[1]
            || pair_raw.values[2] != pair_raw.values[1];
        let count = if rejected {
            0
        } else {
            pair_raw.values[2] as usize
        };
        let raw_pairs = map_values::<PairRecordRaw>(device, &self.pairs, pair_capacity as usize)?;
        let mut pair_slots = Vec::with_capacity(count);
        let mut cache = Vec::with_capacity(work.cache_current as usize);
        for record in raw_pairs.into_iter().take(count) {
            let (assignment, mut points) = pair_output(record)?;
            pair_slots.push(assignment);
            cache.append(&mut points);
        }
        let candidate_capacity =
            capacity_record("candidate-pairs", CANDIDATE_PAIR_BYTES, candidate_raw);
        let manifold_capacity =
            capacity_record("contact-manifolds", COLLISION_MANIFOLD_BYTES, manifold_raw);
        let point_capacity =
            capacity_record("contact-points", COLLISION_CONTACT_POINT_BYTES, point_raw);
        let pair_capacity_record = capacity_record("pair-slot-cache", PAIR_RECORD_BYTES, pair_raw);
        let mut capacities = candidate.capacities(candidate_capacity);
        capacities.extend(narrowphase.capacities(
            manifold_capacity,
            point_capacity,
            work.narrowphase_tests,
        ));
        capacities.append(&mut scratch_capacities);
        capacities.push(pair_capacity_record);
        capacities.push(fixed_capacity(
            "pair-slot-cache-buffers",
            PAIR_RECORD_BYTES,
            u64::from(pair_capacity) * 2,
            u64::from(pair_capacity) * 2,
        ));
        capacities.push(fixed_capacity(
            "pair-slot-generations",
            4,
            u64::from(pair_capacity),
            u64::from(pair_capacity),
        ));
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        let failure = capacities
            .iter()
            .find(|capacity| capacity.overflowed)
            .map(|capacity| FailureV1 {
                kind: FailureKindV1::CapacityOverflow,
                reason: format!(
                    "{} capacity {} rejected {} attempted records",
                    capacity.class, capacity.capacity_elements, capacity.attempted_elements
                ),
                first_step: Some(completed_step),
                first_stage: Some(if capacity.class == "pair-slot-cache" {
                    "pair-slot-persistence".to_owned()
                } else if capacity.class == "candidate-pairs" {
                    "candidate".to_owned()
                } else {
                    "narrowphase".to_owned()
                }),
                reproducer_hash: Some(source_scenario_hash.clone()),
            });
        let output = GpuPairSlotPersistenceStepOutput {
            completed_step,
            source_scenario_hash,
            pair_slots,
            cache,
            work,
            capacities,
            failure,
        };
        output.validate()?;
        Ok(output)
    }
}

fn validate_initial_cache(cache: &[PersistentContactV1]) -> Result<(), String> {
    for entry in cache {
        entry.validate().map_err(|error| error.to_string())?;
    }
    for pair in cache.windows(2) {
        if pair[0].key >= pair[1].key {
            return Err("initial pair-slot cache must be sorted and unique".to_owned());
        }
    }
    Ok(())
}

fn pair_records(
    cache: &[PersistentContactV1],
    fingerprint_mask: u32,
) -> Result<Vec<PairRecordRaw>, String> {
    let mut output = Vec::new();
    for entry in cache {
        let needs_new_pair = output
            .last()
            .is_none_or(|record: &PairRecordRaw| record.key != pair_words(entry.key.pair));
        if needs_new_pair {
            let mut record = invalid_pair();
            record.key = pair_words(entry.key.pair);
            record.meta = [output.len() as u32, 1, 0, entry.value.last_seen_step];
            output.push(record);
        }
        let record = output.last_mut().expect("pair record exists");
        let point = record.meta[2];
        if point >= 2 {
            return Err("initial pair-slot cache has more than two points for one pair".to_owned());
        }
        let features = feature_words(entry.key);
        let cache = cache_words(entry.value, features, fingerprint_mask);
        if point == 0 {
            record.features0 = features;
            record.cache0 = cache;
        } else {
            record.features1 = features;
            record.cache1 = cache;
        }
        record.meta[2] += 1;
    }
    Ok(output)
}

fn pair_output(
    record: PairRecordRaw,
) -> Result<(PairSlotAssignmentV1, Vec<PersistentContactV1>), String> {
    let pair = CandidatePairV1 {
        body_a: physbox_contracts::BodyId {
            slot: record.key[0],
            generation: record.key[1],
        },
        body_b: physbox_contracts::BodyId {
            slot: record.key[2],
            generation: record.key[3],
        },
    };
    let assignment = PairSlotAssignmentV1 {
        pair,
        slot: record.meta[0],
        generation: record.meta[1],
    };
    assignment.validate().map_err(|error| error.to_string())?;
    if !(1..=2).contains(&record.meta[2]) {
        return Err(format!(
            "GPU pair slot emitted invalid point count {}",
            record.meta[2]
        ));
    }
    let mut points = Vec::with_capacity(record.meta[2] as usize);
    for (features, cache) in [
        (record.features0, record.cache0),
        (record.features1, record.cache1),
    ]
    .into_iter()
    .take(record.meta[2] as usize)
    {
        points.push(PersistentContactV1 {
            key: PersistentContactKeyV1 {
                pair,
                feature_a: feature(features[0], features[1])?,
                feature_b: feature(features[2], features[3])?,
            },
            value: ContactCacheValueV1 {
                normal_impulse: finite(f32::from_bits(cache[0]))?,
                tangent_impulse: finite(f32::from_bits(cache[1]))?,
                last_seen_step: record.meta[3],
            },
        });
    }
    Ok((assignment, points))
}

fn invalid_pair() -> PairRecordRaw {
    PairRecordRaw {
        key: [u32::MAX; 4],
        meta: [u32::MAX, 0, 0, 0],
        features0: [u32::MAX; 4],
        cache0: [0; 4],
        features1: [u32::MAX; 4],
        cache1: [0; 4],
    }
}

fn pair_words(pair: CandidatePairV1) -> [u32; 4] {
    [
        pair.body_a.slot,
        pair.body_a.generation,
        pair.body_b.slot,
        pair.body_b.generation,
    ]
}

fn feature_words(key: PersistentContactKeyV1) -> [u32; 4] {
    [
        feature_kind(key.feature_a.kind),
        key.feature_a.index,
        feature_kind(key.feature_b.kind),
        key.feature_b.index,
    ]
}

fn cache_words(cache: ContactCacheValueV1, features: [u32; 4], fingerprint_mask: u32) -> [u32; 4] {
    [
        cache.normal_impulse.get().to_bits(),
        cache.tangent_impulse.get().to_bits(),
        feature_fingerprint(features) & fingerprint_mask,
        0,
    ]
}

fn feature_fingerprint(features: [u32; 4]) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for word in features {
        hash = (hash ^ word).wrapping_mul(0x0100_0193);
    }
    hash
}

fn feature(kind: u32, index: u32) -> Result<GeometricFeatureV1, String> {
    let kind = match kind {
        0 => GeometricFeatureKindV1::Circle,
        1 => GeometricFeatureKindV1::Face,
        2 => GeometricFeatureKindV1::Vertex,
        _ => return Err(format!("GPU pair slot emitted unknown feature kind {kind}")),
    };
    Ok(GeometricFeatureV1 { kind, index })
}

const fn feature_kind(kind: GeometricFeatureKindV1) -> u32 {
    match kind {
        GeometricFeatureKindV1::Circle => 0,
        GeometricFeatureKindV1::Face => 1,
        GeometricFeatureKindV1::Vertex => 2,
    }
}

fn finite(value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| error.to_string())
}

fn fixed_capacity(class: &str, stride: u32, capacity: u64, used: u64) -> CollisionCapacityV1 {
    CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes: stride,
        capacity_elements: capacity,
        attempted_elements: used,
        committed_elements: used,
        allocated_bytes: capacity * u64::from(stride),
        high_water_bytes: used * u64::from(stride),
        overflowed: false,
        accepted: true,
    }
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
        binding: 18,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<PairSlotParamsRaw>() as u64),
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
        binding: 18,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_pair_slot_group"),
        layout,
        entries: &entries,
    })
}

fn uniform<T: Pod>(device: &wgpu::Device, label: &'static str, value: &T) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(value),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

fn storage_init<T: Pod>(device: &wgpu::Device, label: &'static str, values: &[T]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(values),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
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
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
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
