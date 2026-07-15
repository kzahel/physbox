use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES, COLLISION_CONTACT_CACHE_VALUE_BYTES,
    COLLISION_CONTACT_POINT_BYTES, COLLISION_MANIFOLD_BYTES, COLLISION_WORK_COUNTER_BYTES,
    CollisionCapacityV1, CollisionWorkV1, ContactCacheValueV1, FailureKindV1, FailureV1, FiniteF32,
    GeometricFeatureKindV1, GeometricFeatureV1, PERSISTENT_CONTACT_KEY_BYTES,
    PersistentContactKeyV1, PersistentContactV1, ScenarioV1, Validate, canonical_hash,
};
use wgpu::util::DeviceExt;

use crate::{
    AdapterDiagnostics,
    collision_candidates::{CandidateCase, CandidatePipelines},
    collision_narrowphase::{NarrowphaseCase, NarrowphasePipelines},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    collision_candidates::map_values,
    gpu_context::{create_instance, request_adapter, request_device},
};

const WORKGROUP_SIZE: u32 = 64;
const RADIX_WORKGROUP_SIZE: u32 = 256;
const RADIX_PASSES: u32 = 64;
const PERSISTENCE_DISPATCHES: u32 = 3 + RADIX_PASSES * 3 + 2;
const TOTAL_DISPATCHES: u32 = 15 + PERSISTENCE_DISPATCHES;
const CURRENT_RECORD_BYTES: u32 = 64;
const PERSISTENT_RECORD_BYTES: u32 =
    PERSISTENT_CONTACT_KEY_BYTES + COLLISION_CONTACT_CACHE_VALUE_BYTES;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CurrentRecordRaw {
    key0: [u32; 4],
    key1: [u32; 4],
    cache: [u32; 4],
    location: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct PersistentRecordRaw {
    key0: [u32; 4],
    key1: [u32; 4],
    cache: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct CapacityRaw {
    pub(crate) values: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct WorkRaw {
    pub(crate) values: [u32; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PersistenceParamsRaw {
    counts: [u32; 4],
    radix: [u32; 4],
}

const _: () = assert!(size_of::<CurrentRecordRaw>() == CURRENT_RECORD_BYTES as usize);
const _: () = assert!(size_of::<PersistentRecordRaw>() == PERSISTENT_RECORD_BYTES as usize);
const _: () = assert!(size_of::<CapacityRaw>() == COLLISION_CAPACITY_COUNTER_BYTES as usize);
const _: () = assert!(size_of::<WorkRaw>() == COLLISION_WORK_COUNTER_BYTES as usize);

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPersistenceStepOutput {
    pub completed_step: u32,
    pub source_scenario_hash: String,
    pub cache: Vec<PersistentContactV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
}

impl GpuPersistenceStepOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_scenario_hash.len() != 64
            || !self
                .source_scenario_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("GPU persistence step has an invalid source hash".to_owned());
        }
        for pair in self.cache.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err("GPU persistence cache is not sorted and unique".to_owned());
            }
        }
        for entry in &self.cache {
            entry.validate().map_err(|error| error.to_string())?;
            if entry.value.last_seen_step != self.completed_step {
                return Err("GPU persistence cache has a stale last-seen step".to_owned());
            }
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("GPU persistence capacities are not sorted and unique".to_owned());
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
            || (rejected && !self.cache.is_empty())
            || (!rejected && self.cache.len() as u64 != self.work.cache_current)
        {
            return Err("GPU persistence cache/failure state is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuPersistenceOutput {
    pub adapter: AdapterDiagnostics,
    pub cache_capacity: u32,
    pub mapped_after_final_step_only: bool,
    pub input_permuted_before_sort: bool,
    pub steps: Vec<GpuPersistenceStepOutput>,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuPersistenceRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    candidate_pipelines: CandidatePipelines,
    narrowphase_pipelines: NarrowphasePipelines,
    persistence_pipelines: PersistencePipelines,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuPersistenceRunner {
    pub async fn new() -> Result<Self, String> {
        let instance = create_instance();
        let adapter = request_adapter(&instance, None).await?;
        let diagnostics = AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = request_device(&adapter).await?;
        let candidate_pipelines = CandidatePipelines::new(&device);
        let narrowphase_pipelines = NarrowphasePipelines::new(&device);
        let persistence_pipelines = PersistencePipelines::new(&device);
        Ok(Self {
            device,
            queue,
            diagnostics,
            candidate_pipelines,
            narrowphase_pipelines,
            persistence_pipelines,
        })
    }

    pub fn adapter(&self) -> &AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn run_sequence(
        &self,
        scenarios: &[ScenarioV1],
        initial_cache: &[PersistentContactV1],
        requested_cache_capacity: Option<u32>,
        permute_current_input: bool,
    ) -> Result<GpuPersistenceOutput, String> {
        if scenarios.is_empty() {
            return Err("GPU persistence sequence requires at least one step".to_owned());
        }
        for scenario in scenarios {
            scenario.validate().map_err(|error| error.to_string())?;
        }
        validate_initial_cache(initial_cache)?;

        let mut candidate_cases = Vec::with_capacity(scenarios.len());
        let mut narrowphase_cases = Vec::with_capacity(scenarios.len());
        let mut maximum_point_capacity = 0;
        for scenario in scenarios {
            let candidate =
                CandidateCase::new(&self.device, &self.candidate_pipelines, scenario, None)?;
            let narrowphase =
                NarrowphaseCase::new(&self.device, &self.narrowphase_pipelines, &candidate, None)?;
            maximum_point_capacity = maximum_point_capacity.max(narrowphase.point_capacity());
            candidate_cases.push(candidate);
            narrowphase_cases.push(narrowphase);
        }
        let cache_capacity = requested_cache_capacity.unwrap_or(maximum_point_capacity);
        if cache_capacity == 0 || initial_cache.len() > cache_capacity as usize {
            return Err(format!(
                "cache capacity {cache_capacity} cannot hold {} initial contacts",
                initial_cache.len()
            ));
        }

        let initial_records = padded_cache(initial_cache, cache_capacity)?;
        let empty_records = vec![invalid_persistent(); cache_capacity as usize];
        let cache_buffers = [
            storage_init(&self.device, "persistence_cache_a", &initial_records),
            storage_init(&self.device, "persistence_cache_b", &empty_records),
        ];
        let capacity_buffers = [
            storage_init(
                &self.device,
                "persistence_capacity_a",
                &[CapacityRaw {
                    values: [
                        cache_capacity,
                        initial_cache.len() as u32,
                        initial_cache.len() as u32,
                        0,
                    ],
                }],
            ),
            storage::<CapacityRaw>(&self.device, "persistence_capacity_b", 1)?,
        ];
        let mut previous_index = 0_usize;
        let mut output_index = 1_usize;
        let step_counter = storage_init(&self.device, "persistence_step_counter", &[0_u32]);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_collision_persistence_sequence"),
            });
        let mut readbacks = Vec::with_capacity(scenarios.len());

        for (index, ((scenario, candidate), narrowphase)) in scenarios
            .iter()
            .zip(&candidate_cases)
            .zip(&narrowphase_cases)
            .enumerate()
        {
            let completed_step = index as u32 + 1;
            let persistence = PersistenceCase::new(
                &self.device,
                &self.persistence_pipelines,
                narrowphase,
                &cache_buffers[previous_index],
                &capacity_buffers[previous_index],
                &cache_buffers[output_index],
                &capacity_buffers[output_index],
                candidate.work_buffer(),
                &step_counter,
                cache_capacity,
                completed_step,
                permute_current_input,
            )?;
            candidate.encode(&mut encoder, &self.candidate_pipelines);
            narrowphase.encode(&mut encoder, &self.narrowphase_pipelines);
            persistence.encode(&mut encoder, &self.persistence_pipelines);
            let step_readback = StepReadback::new(
                &self.device,
                cache_capacity,
                candidate,
                narrowphase,
                &cache_buffers[output_index],
                &capacity_buffers[output_index],
            );
            step_readback.encode(
                &mut encoder,
                candidate,
                narrowphase,
                &cache_buffers[output_index],
                &capacity_buffers[output_index],
            );
            readbacks.push((
                completed_step,
                canonical_hash(scenario).map_err(|error| error.to_string())?,
                step_readback,
            ));
            std::mem::swap(&mut previous_index, &mut output_index);
        }

        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU persistence sequence poll failed: {error}"))?;

        let mut steps = Vec::with_capacity(readbacks.len());
        for (candidate, (completed_step, source_scenario_hash, readback)) in
            candidate_cases.iter().zip(readbacks)
        {
            let step = readback.map(
                &self.device,
                candidate,
                &narrowphase_cases[completed_step as usize - 1],
                completed_step,
                source_scenario_hash,
                cache_capacity,
            )?;
            steps.push(step);
        }
        Ok(GpuPersistenceOutput {
            adapter: self.diagnostics.clone(),
            cache_capacity,
            mapped_after_final_step_only: true,
            input_permuted_before_sort: permute_current_input,
            steps,
        })
    }
}

pub(crate) struct PersistencePipelines {
    clear: wgpu::ComputePipeline,
    prepare: wgpu::ComputePipeline,
    build: wgpu::ComputePipeline,
    radix_histograms: wgpu::ComputePipeline,
    radix_prefix: wgpu::ComputePipeline,
    radix_scatter: wgpu::ComputePipeline,
    merge: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    clear_layout: wgpu::BindGroupLayout,
    prepare_layout: wgpu::BindGroupLayout,
    build_layout: wgpu::BindGroupLayout,
    radix_layout: wgpu::BindGroupLayout,
    merge_layout: wgpu::BindGroupLayout,
    finalize_layout: wgpu::BindGroupLayout,
}

impl PersistencePipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let clear_layout = layout(
            device,
            "persistence_clear_layout",
            &[
                (4, false),
                (5, false),
                (6, false),
                (9, false),
                (10, false),
                (11, false),
            ],
        );
        let prepare_layout = layout(
            device,
            "persistence_prepare_layout",
            &[(2, false), (3, false), (8, false), (10, false), (13, false)],
        );
        let build_layout = layout(
            device,
            "persistence_build_layout",
            &[(0, true), (1, true), (2, false), (4, false), (10, false)],
        );
        let radix_layout = layout(
            device,
            "persistence_radix_layout",
            &[(4, false), (5, false), (6, false)],
        );
        let merge_layout = layout(
            device,
            "persistence_merge_layout",
            &[
                (4, false),
                (7, true),
                (8, false),
                (9, false),
                (10, false),
                (11, false),
                (13, false),
            ],
        );
        let finalize_layout = layout(
            device,
            "persistence_finalize_layout",
            &[(8, false), (10, false), (11, false)],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_collision_persistence_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("collision_persistence.wgsl").into()),
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
                "persistence_clear_pipeline",
                "clear_persistence",
                &clear_layout,
            ),
            prepare: pipeline(
                "persistence_prepare_pipeline",
                "prepare_persistence",
                &prepare_layout,
            ),
            build: pipeline(
                "persistence_build_pipeline",
                "build_current_keys",
                &build_layout,
            ),
            radix_histograms: pipeline(
                "persistence_radix_histograms_pipeline",
                "radix_histograms",
                &radix_layout,
            ),
            radix_prefix: pipeline(
                "persistence_radix_prefix_pipeline",
                "radix_prefix",
                &radix_layout,
            ),
            radix_scatter: pipeline(
                "persistence_radix_scatter_pipeline",
                "radix_scatter",
                &radix_layout,
            ),
            merge: pipeline("persistence_merge_pipeline", "merge_cache", &merge_layout),
            finalize: pipeline(
                "persistence_finalize_pipeline",
                "finalize_persistence",
                &finalize_layout,
            ),
            clear_layout,
            prepare_layout,
            build_layout,
            radix_layout,
            merge_layout,
            finalize_layout,
        }
    }
}

pub(crate) struct PersistenceCase {
    _sort_a: wgpu::Buffer,
    _sort_b: wgpu::Buffer,
    _blocks: wgpu::Buffer,
    _params: Vec<wgpu::Buffer>,
    clear_group: wgpu::BindGroup,
    prepare_group: wgpu::BindGroup,
    build_group: wgpu::BindGroup,
    radix_groups: Vec<wgpu::BindGroup>,
    merge_group: wgpu::BindGroup,
    finalize_group: wgpu::BindGroup,
    cache_capacity: u32,
    manifold_capacity: u32,
    block_count: u32,
}

#[allow(clippy::too_many_arguments)]
impl PersistenceCase {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &PersistencePipelines,
        narrowphase: &NarrowphaseCase,
        previous_cache: &wgpu::Buffer,
        previous_capacity: &wgpu::Buffer,
        output_cache: &wgpu::Buffer,
        output_capacity: &wgpu::Buffer,
        work: &wgpu::Buffer,
        step_counter: &wgpu::Buffer,
        cache_capacity: u32,
        completed_step: u32,
        permute_current_input: bool,
    ) -> Result<Self, String> {
        let block_count = cache_capacity.div_ceil(RADIX_WORKGROUP_SIZE);
        let sort_a = storage::<CurrentRecordRaw>(device, "persistence_sort_a", cache_capacity)?;
        let sort_b = storage::<CurrentRecordRaw>(device, "persistence_sort_b", cache_capacity)?;
        let blocks = storage::<u32>(device, "persistence_radix_blocks", block_count * 16)?;
        let base = PersistenceParamsRaw {
            counts: [
                cache_capacity,
                block_count,
                completed_step,
                u32::from(permute_current_input),
            ],
            radix: [0, 0, RADIX_PASSES, TOTAL_DISPATCHES],
        };
        let mut params = Vec::with_capacity(RADIX_PASSES as usize + 1);
        params.push(uniform(device, "persistence_parameters", &base));
        let clear_group = group(
            device,
            &pipelines.clear_layout,
            &[
                (4, &sort_a),
                (5, &sort_b),
                (6, &blocks),
                (9, output_cache),
                (10, output_capacity),
                (11, work),
            ],
            &params[0],
        );
        let prepare_group = group(
            device,
            &pipelines.prepare_layout,
            &[
                (2, narrowphase.manifold_capacity_buffer()),
                (3, narrowphase.point_capacity_buffer()),
                (8, previous_capacity),
                (10, output_capacity),
                (13, step_counter),
            ],
            &params[0],
        );
        let build_group = group(
            device,
            &pipelines.build_layout,
            &[
                (0, narrowphase.manifolds()),
                (1, narrowphase.points()),
                (2, narrowphase.manifold_capacity_buffer()),
                (4, &sort_a),
                (10, output_capacity),
            ],
            &params[0],
        );
        let mut radix_groups = Vec::with_capacity(RADIX_PASSES as usize);
        let mut pass = 0_u32;
        for field in (0..8_u32).rev() {
            for shift in (0..32_u32).step_by(4) {
                let mut radix_params = base;
                radix_params.radix[0] = field;
                radix_params.radix[1] = shift;
                params.push(uniform(
                    device,
                    "persistence_radix_parameters",
                    &radix_params,
                ));
                let (source, destination) = if pass.is_multiple_of(2) {
                    (&sort_a, &sort_b)
                } else {
                    (&sort_b, &sort_a)
                };
                radix_groups.push(group(
                    device,
                    &pipelines.radix_layout,
                    &[(4, source), (5, destination), (6, &blocks)],
                    params.last().unwrap(),
                ));
                pass += 1;
            }
        }
        debug_assert_eq!(pass, RADIX_PASSES);
        let merge_group = group(
            device,
            &pipelines.merge_layout,
            &[
                (4, &sort_a),
                (7, previous_cache),
                (8, previous_capacity),
                (9, output_cache),
                (10, output_capacity),
                (11, work),
                (13, step_counter),
            ],
            &params[0],
        );
        let finalize_group = group(
            device,
            &pipelines.finalize_layout,
            &[(8, previous_capacity), (10, output_capacity), (11, work)],
            &params[0],
        );
        Ok(Self {
            _sort_a: sort_a,
            _sort_b: sort_b,
            _blocks: blocks,
            _params: params,
            clear_group,
            prepare_group,
            build_group,
            radix_groups,
            merge_group,
            finalize_group,
            cache_capacity,
            manifold_capacity: narrowphase.manifold_capacity(),
            block_count,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &PersistencePipelines,
    ) {
        self.encode_with_timestamp(encoder, pipelines, None);
    }

    pub(crate) fn encode_with_timestamp(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &PersistencePipelines,
        ending_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        self.encode_with_timestamps(encoder, pipelines, None, ending_timestamp);
    }

    pub(crate) fn encode_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &PersistencePipelines,
        beginning_timestamp: Option<(&wgpu::QuerySet, u32)>,
        ending_timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        let query_set = beginning_timestamp
            .map(|(query_set, _)| query_set)
            .or_else(|| ending_timestamp.map(|(query_set, _)| query_set));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_collision_persistence_passes"),
            timestamp_writes: query_set.map(|query_set| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: beginning_timestamp.map(|(_, index)| index),
                end_of_pass_write_index: ending_timestamp.map(|(_, index)| index),
            }),
        });
        dispatch(
            &mut pass,
            &pipelines.clear,
            &self.clear_group,
            self.cache_capacity
                .max(self.block_count * 16)
                .max(15)
                .div_ceil(WORKGROUP_SIZE),
        );
        dispatch(&mut pass, &pipelines.prepare, &self.prepare_group, 1);
        dispatch(
            &mut pass,
            &pipelines.build,
            &self.build_group,
            self.manifold_capacity.div_ceil(WORKGROUP_SIZE),
        );
        for group in &self.radix_groups {
            dispatch(
                &mut pass,
                &pipelines.radix_histograms,
                group,
                self.block_count,
            );
            dispatch(&mut pass, &pipelines.radix_prefix, group, 1);
            dispatch(&mut pass, &pipelines.radix_scatter, group, self.block_count);
        }
        dispatch(
            &mut pass,
            &pipelines.merge,
            &self.merge_group,
            self.cache_capacity.div_ceil(WORKGROUP_SIZE),
        );
        dispatch(&mut pass, &pipelines.finalize, &self.finalize_group, 1);
    }

    pub(crate) fn sorted_current(&self) -> &wgpu::Buffer {
        &self._sort_a
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self._sort_a.size()
            + self._sort_b.size()
            + self._blocks.size()
            + self._params.iter().map(wgpu::Buffer::size).sum::<u64>()
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct StepReadback {
    cache: wgpu::Buffer,
    candidate_capacity: wgpu::Buffer,
    manifold_capacity: wgpu::Buffer,
    point_capacity: wgpu::Buffer,
    persistence_capacity: wgpu::Buffer,
    work: wgpu::Buffer,
}

#[cfg(not(target_arch = "wasm32"))]
impl StepReadback {
    fn new(
        device: &wgpu::Device,
        cache_capacity: u32,
        _candidate: &CandidateCase,
        _narrowphase: &NarrowphaseCase,
        _cache: &wgpu::Buffer,
        _capacity: &wgpu::Buffer,
    ) -> Self {
        Self {
            cache: readback(
                device,
                "persistence_cache_readback",
                u64::from(cache_capacity) * u64::from(PERSISTENT_RECORD_BYTES),
            ),
            candidate_capacity: readback(
                device,
                "persistence_candidate_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            manifold_capacity: readback(
                device,
                "persistence_manifold_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            point_capacity: readback(
                device,
                "persistence_point_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            persistence_capacity: readback(
                device,
                "persistence_capacity_readback",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            ),
            work: readback(
                device,
                "persistence_work_readback",
                u64::from(COLLISION_WORK_COUNTER_BYTES),
            ),
        }
    }

    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        cache: &wgpu::Buffer,
        capacity: &wgpu::Buffer,
    ) {
        copy(encoder, cache, &self.cache, self.cache.size());
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
            capacity,
            &self.persistence_capacity,
            self.persistence_capacity.size(),
        );
        copy(
            encoder,
            candidate.work_buffer(),
            &self.work,
            self.work.size(),
        );
    }

    fn map(
        self,
        device: &wgpu::Device,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        completed_step: u32,
        source_scenario_hash: String,
        cache_capacity: u32,
    ) -> Result<GpuPersistenceStepOutput, String> {
        let candidate_raw = map_values::<CapacityRaw>(device, &self.candidate_capacity, 1)?[0];
        let manifold_raw = map_values::<CapacityRaw>(device, &self.manifold_capacity, 1)?[0];
        let point_raw = map_values::<CapacityRaw>(device, &self.point_capacity, 1)?[0];
        let persistence_raw = map_values::<CapacityRaw>(device, &self.persistence_capacity, 1)?[0];
        let work = work_record(map_values::<WorkRaw>(device, &self.work, 1)?[0]);
        let rejected = candidate_raw.values[3] != 0
            || manifold_raw.values[2] != manifold_raw.values[1]
            || point_raw.values[2] != point_raw.values[1]
            || persistence_raw.values[2] != persistence_raw.values[1];
        let cache_count = if rejected {
            0
        } else {
            persistence_raw.values[2] as usize
        };
        let cache =
            map_values::<PersistentRecordRaw>(device, &self.cache, cache_capacity as usize)?
                .into_iter()
                .take(cache_count)
                .map(persistent_contact)
                .collect::<Result<Vec<_>, _>>()?;
        let candidate_capacity =
            capacity_record("candidate-pairs", CANDIDATE_PAIR_BYTES, candidate_raw);
        let manifold_capacity =
            capacity_record("contact-manifolds", COLLISION_MANIFOLD_BYTES, manifold_raw);
        let point_capacity =
            capacity_record("contact-points", COLLISION_CONTACT_POINT_BYTES, point_raw);
        let persistence_capacity =
            capacity_record("persistent-cache", PERSISTENT_RECORD_BYTES, persistence_raw);
        let mut capacities = candidate.capacities(candidate_capacity);
        capacities.extend(narrowphase.capacities(
            manifold_capacity,
            point_capacity,
            work.narrowphase_tests,
        ));
        capacities.extend(persistence_capacities(
            cache_capacity,
            work.cache_previous,
            work.cache_current,
            persistence_capacity,
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
                first_stage: Some(if capacity.class == "persistent-cache" {
                    "persistence".to_owned()
                } else if capacity.class == "candidate-pairs" {
                    "candidate".to_owned()
                } else {
                    "narrowphase".to_owned()
                }),
                reproducer_hash: Some(source_scenario_hash.clone()),
            });
        let output = GpuPersistenceStepOutput {
            completed_step,
            source_scenario_hash,
            cache,
            work,
            capacities,
            failure,
        };
        output.validate()?;
        Ok(output)
    }
}

pub(crate) fn persistence_capacities(
    cache_capacity: u32,
    previous: u64,
    current: u64,
    output: CollisionCapacityV1,
) -> Vec<CollisionCapacityV1> {
    let block_count = cache_capacity.div_ceil(RADIX_WORKGROUP_SIZE);
    vec![
        output,
        fixed_capacity(
            "persistence-cache-slots",
            PERSISTENT_RECORD_BYTES,
            u64::from(cache_capacity) * 2,
            previous + current,
        ),
        fixed_capacity(
            "persistence-radix-blocks",
            4,
            u64::from(block_count) * 16,
            u64::from(block_count) * 16,
        ),
        fixed_capacity(
            "persistence-sort-records",
            CURRENT_RECORD_BYTES,
            u64::from(cache_capacity) * 2,
            u64::from(cache_capacity) * 2,
        ),
    ]
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

fn validate_initial_cache(cache: &[PersistentContactV1]) -> Result<(), String> {
    for entry in cache {
        entry.validate().map_err(|error| error.to_string())?;
    }
    for pair in cache.windows(2) {
        if pair[0].key >= pair[1].key {
            return Err("initial persistence cache must be sorted and unique".to_owned());
        }
    }
    Ok(())
}

fn padded_cache(
    cache: &[PersistentContactV1],
    capacity: u32,
) -> Result<Vec<PersistentRecordRaw>, String> {
    let mut output = cache
        .iter()
        .map(persistent_raw)
        .collect::<Result<Vec<_>, _>>()?;
    output.resize(capacity as usize, invalid_persistent());
    Ok(output)
}

fn persistent_raw(contact: &PersistentContactV1) -> Result<PersistentRecordRaw, String> {
    contact.validate().map_err(|error| error.to_string())?;
    Ok(PersistentRecordRaw {
        key0: [
            contact.key.pair.body_a.slot,
            contact.key.pair.body_a.generation,
            contact.key.pair.body_b.slot,
            contact.key.pair.body_b.generation,
        ],
        key1: [
            feature_kind(contact.key.feature_a.kind),
            contact.key.feature_a.index,
            feature_kind(contact.key.feature_b.kind),
            contact.key.feature_b.index,
        ],
        cache: [
            contact.value.normal_impulse.get().to_bits(),
            contact.value.tangent_impulse.get().to_bits(),
            contact.value.last_seen_step,
            0,
        ],
    })
}

fn invalid_persistent() -> PersistentRecordRaw {
    PersistentRecordRaw {
        key0: [u32::MAX; 4],
        key1: [u32::MAX; 4],
        cache: [0; 4],
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn persistent_contact(raw: PersistentRecordRaw) -> Result<PersistentContactV1, String> {
    Ok(PersistentContactV1 {
        key: PersistentContactKeyV1 {
            pair: physbox_contracts::CandidatePairV1 {
                body_a: physbox_contracts::BodyId {
                    slot: raw.key0[0],
                    generation: raw.key0[1],
                },
                body_b: physbox_contracts::BodyId {
                    slot: raw.key0[2],
                    generation: raw.key0[3],
                },
            },
            feature_a: geometric_feature(raw.key1[0], raw.key1[1])?,
            feature_b: geometric_feature(raw.key1[2], raw.key1[3])?,
        },
        value: ContactCacheValueV1 {
            normal_impulse: finite(f32::from_bits(raw.cache[0]))?,
            tangent_impulse: finite(f32::from_bits(raw.cache[1]))?,
            last_seen_step: raw.cache[2],
        },
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn geometric_feature(kind: u32, index: u32) -> Result<GeometricFeatureV1, String> {
    let kind = match kind {
        0 => GeometricFeatureKindV1::Circle,
        1 => GeometricFeatureKindV1::Face,
        2 => GeometricFeatureKindV1::Vertex,
        _ => {
            return Err(format!(
                "GPU persistence emitted unknown feature kind {kind}"
            ));
        }
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn work_record(raw: WorkRaw) -> CollisionWorkV1 {
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
pub(crate) fn capacity_record(class: &str, stride: u32, raw: CapacityRaw) -> CollisionCapacityV1 {
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
        binding: 12,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<PersistenceParamsRaw>() as u64),
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
        binding: 12,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_collision_persistence_group"),
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
