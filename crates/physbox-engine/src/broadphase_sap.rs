#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BODY_AABB_BYTES, BROADPHASE_CONTRACT_VERSION, BodyAabbV1, BroadphaseAlgorithmV1,
    BroadphaseMobilityV1, BroadphaseResultV1, BroadphaseSnapshotV1, BroadphaseWorkV1,
    CANDIDATE_PAIR_BYTES, COLLISION_WORK_COUNTER_BYTES, CandidatePairV1, CollisionCapacityV1,
    CollisionWorkV1, FailureKindV1, FailureV1, SCHEMA_VERSION, ScenarioV1, Validate, Vec2,
    canonical_hash,
};

use crate::{
    AdapterDiagnostics, BODY_COMMAND_CAPACITY, GpuBodyStorage,
    broadphase_benchmark::GpuBroadphaseTimingV1,
    collision_candidates::{CandidateSource, GpuCandidateOutput},
    gpu_context::{create_instance, request_adapter, request_device_with_features},
};

const RADIX_WORKGROUP_SIZE: u32 = 256;
const GENERAL_WORKGROUP_SIZE: u32 = 64;
const PROXY_RADIX_PASSES: u32 = 8;
// Snapshot ordinals are bounded below 2^20 by the 100K corpus contract. Five
// four-bit digits per ordinal preserve the full canonical pair key.
const PAIR_RADIX_PASSES: u32 = 10;
pub(crate) const SAP_DISPATCHES: u32 =
    1 + PROXY_RADIX_PASSES * 3 + 1 + 1 + PAIR_RADIX_PASSES * 3 + 1;
pub(crate) const PRODUCT_SAP_DISPATCHES: u32 = SAP_DISPATCHES + 1;
const BROADPHASE_WORK_BYTES: u32 = 64;
const PARAMETER_BYTES: u32 = 64;
const MOBILITY_FIXED: u32 = 1;
const MOBILITY_LIVE: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ProxyRaw {
    identity_mobility: [u32; 4],
    lower_upper: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PairIndexRaw {
    values: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CandidatePairRaw {
    identities: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct SapParamsRaw {
    counts: [u32; 4],
    radix: [u32; 4],
    execution: [u32; 4],
    policy: [f32; 4],
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CollisionWorkRaw {
    values: [u32; 32],
}

const _: () = assert!(size_of::<ProxyRaw>() == BODY_AABB_BYTES as usize);
const _: () = assert!(size_of::<PairIndexRaw>() == CANDIDATE_PAIR_BYTES as usize);
const _: () = assert!(size_of::<CandidatePairRaw>() == CANDIDATE_PAIR_BYTES as usize);
const _: () = assert!(size_of::<SapParamsRaw>() == PARAMETER_BYTES as usize);
const _: () = assert!(size_of::<BroadphaseWorkRaw>() == BROADPHASE_WORK_BYTES as usize);
const _: () = assert!(size_of::<CapacityRaw>() == 16);
const _: () = assert!(size_of::<CollisionWorkRaw>() == COLLISION_WORK_COUNTER_BYTES as usize);

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSweepAndPruneOutput {
    pub adapter: AdapterDiagnostics,
    pub result: BroadphaseResultV1,
}

impl GpuSweepAndPruneOutput {
    pub fn validate(&self) -> Result<(), String> {
        self.result.validate().map_err(|error| error.to_string())?;
        if self.result.algorithm != BroadphaseAlgorithmV1::SweepAndPrune {
            return Err("GPU SAP output has the wrong algorithm identity".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSweepAndPruneBenchmarkOutput {
    pub correctness: GpuSweepAndPruneOutput,
    pub timing: GpuBroadphaseTimingV1,
}

impl GpuSweepAndPruneBenchmarkOutput {
    pub fn validate(&self) -> Result<(), String> {
        self.correctness.validate()?;
        self.timing.validate()
    }
}

pub(crate) struct SapPipelines {
    body_aabb: wgpu::ComputePipeline,
    clear: wgpu::ComputePipeline,
    proxy_histograms: wgpu::ComputePipeline,
    proxy_prefix: wgpu::ComputePipeline,
    proxy_scatter: wgpu::ComputePipeline,
    enumerate: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    pair_histograms: wgpu::ComputePipeline,
    pair_prefix: wgpu::ComputePipeline,
    pair_scatter: wgpu::ComputePipeline,
    materialize: wgpu::ComputePipeline,
    body_aabb_layout: wgpu::BindGroupLayout,
    clear_layout: wgpu::BindGroupLayout,
    proxy_radix_layout: wgpu::BindGroupLayout,
    enumerate_layout: wgpu::BindGroupLayout,
    finalize_layout: wgpu::BindGroupLayout,
    pair_radix_layout: wgpu::BindGroupLayout,
    materialize_layout: wgpu::BindGroupLayout,
}

impl SapPipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let body_aabb_layout = layout(
            device,
            "sap_body_aabb_layout",
            &[(0, false), (13, true), (14, true), (15, true)],
        );
        let clear_layout = layout(
            device,
            "sap_clear_layout",
            &[(8, false), (9, false), (11, false), (12, false)],
        );
        let proxy_radix_layout = layout(
            device,
            "sap_proxy_radix_layout",
            &[(1, false), (2, false), (3, false)],
        );
        let enumerate_layout = layout(
            device,
            "sap_enumerate_layout",
            &[(1, false), (4, false), (8, false)],
        );
        let finalize_layout = layout(
            device,
            "sap_finalize_layout",
            &[(8, false), (9, false), (11, false), (12, false)],
        );
        let pair_radix_layout = layout(
            device,
            "sap_pair_radix_layout",
            &[(4, false), (5, false), (6, false), (9, false)],
        );
        let materialize_layout = layout(
            device,
            "sap_materialize_layout",
            &[(0, false), (4, false), (7, false), (9, false)],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_broadphase_sap_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("broadphase_sap.wgsl").into()),
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
        Self {
            body_aabb: pipeline(
                "physbox5_sap_body_aabb_pipeline",
                "compute_sap_body_aabbs",
                &body_aabb_layout,
            ),
            clear: pipeline(
                "physbox5_sap_clear_pipeline",
                "clear_sap_state",
                &clear_layout,
            ),
            proxy_histograms: pipeline(
                "physbox5_sap_proxy_histograms_pipeline",
                "proxy_radix_histograms",
                &proxy_radix_layout,
            ),
            proxy_prefix: pipeline(
                "physbox5_sap_proxy_prefix_pipeline",
                "proxy_radix_prefix",
                &proxy_radix_layout,
            ),
            proxy_scatter: pipeline(
                "physbox5_sap_proxy_scatter_pipeline",
                "proxy_radix_scatter",
                &proxy_radix_layout,
            ),
            enumerate: pipeline(
                "physbox5_sap_enumerate_pipeline",
                "enumerate_sap_pairs",
                &enumerate_layout,
            ),
            finalize: pipeline(
                "physbox5_sap_finalize_pipeline",
                "finalize_sap_counts",
                &finalize_layout,
            ),
            pair_histograms: pipeline(
                "physbox5_sap_pair_histograms_pipeline",
                "pair_radix_histograms",
                &pair_radix_layout,
            ),
            pair_prefix: pipeline(
                "physbox5_sap_pair_prefix_pipeline",
                "pair_radix_prefix",
                &pair_radix_layout,
            ),
            pair_scatter: pipeline(
                "physbox5_sap_pair_scatter_pipeline",
                "pair_radix_scatter",
                &pair_radix_layout,
            ),
            materialize: pipeline(
                "physbox5_sap_materialize_pipeline",
                "materialize_sap_pairs",
                &materialize_layout,
            ),
            body_aabb_layout,
            clear_layout,
            proxy_radix_layout,
            enumerate_layout,
            finalize_layout,
            pair_radix_layout,
            materialize_layout,
        }
    }
}

pub(crate) struct SapCase {
    input: wgpu::Buffer,
    proxies_a: wgpu::Buffer,
    _proxies_b: wgpu::Buffer,
    _proxy_blocks: wgpu::Buffer,
    _pairs_a: wgpu::Buffer,
    _pairs_b: wgpu::Buffer,
    _pair_blocks: wgpu::Buffer,
    candidates: wgpu::Buffer,
    work: wgpu::Buffer,
    collision_work: wgpu::Buffer,
    capacity: wgpu::Buffer,
    indirect: wgpu::Buffer,
    _parameters: Vec<wgpu::Buffer>,
    body_aabb_group: Option<wgpu::BindGroup>,
    clear_group: wgpu::BindGroup,
    proxy_radix_groups: Vec<wgpu::BindGroup>,
    enumerate_group: wgpu::BindGroup,
    finalize_group: wgpu::BindGroup,
    pair_radix_groups: Vec<wgpu::BindGroup>,
    materialize_group: wgpu::BindGroup,
    proxy_count: u32,
    proxy_blocks_count: u32,
    candidate_capacity: u32,
    pair_blocks_count: u32,
    speculative_distance: f32,
    dispatches: u32,
}

impl SapCase {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &SapPipelines,
        snapshot: &BroadphaseSnapshotV1,
        candidate_capacity: u32,
    ) -> Result<Self, String> {
        snapshot.validate().map_err(|error| error.to_string())?;
        if candidate_capacity == 0 {
            return Err("GPU SAP candidate capacity must be positive".to_owned());
        }
        let proxy_count = u32::try_from(snapshot.proxies.len())
            .map_err(|_| "GPU SAP proxy count exceeds u32".to_owned())?;
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
            .map(|(ordinal, proxy)| ProxyRaw {
                identity_mobility: [
                    proxy.aabb.body.slot,
                    proxy.aabb.body.generation,
                    MOBILITY_LIVE
                        | (u32::from(proxy.mobility == BroadphaseMobilityV1::Fixed)
                            * MOBILITY_FIXED),
                    ordinal as u32,
                ],
                lower_upper: [
                    proxy.aabb.lower.x,
                    proxy.aabb.lower.y,
                    proxy.aabb.upper.x,
                    proxy.aabb.upper.y,
                ],
            })
            .collect::<Vec<_>>();
        let input = initialized_storage(device, "sap_input_proxies", &input_values)?;
        Self::new_from_input(
            device,
            pipelines,
            input,
            None,
            proxy_count,
            dynamic_count,
            fixed_count,
            candidate_capacity,
            0.0,
            SAP_DISPATCHES,
        )
    }

    pub(crate) fn new_with_storage(
        device: &wgpu::Device,
        pipelines: &SapPipelines,
        scenario: &ScenarioV1,
        candidate_capacity: u32,
        body_storage: &GpuBodyStorage,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        if candidate_capacity == 0 {
            return Err("product SAP candidate capacity must be positive".to_owned());
        }
        let proxy_count = body_storage.layout().capacity_elements;
        if !(2..(1 << 20)).contains(&proxy_count) {
            return Err("product SAP requires between 2 and 1048575 body slots".to_owned());
        }
        let maximum_pairs = u64::from(proxy_count) * u64::from(proxy_count - 1) / 2;
        if u64::from(candidate_capacity) > maximum_pairs {
            return Err(format!(
                "product SAP candidate capacity {candidate_capacity} exceeds {maximum_pairs} possible pairs"
            ));
        }
        let fixed_count = scenario
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() == 0.0)
            .count() as u32;
        let dynamic_count = scenario.bodies.len() as u32 - fixed_count;
        let speculative_distance = scenario
            .settings
            .contact_policy
            .map_or(0.0, |policy| policy.speculative_distance.get());
        let input = storage::<ProxyRaw>(
            device,
            "product_sap_input_proxies",
            proxy_count,
            wgpu::BufferUsages::COPY_SRC,
        )?;
        Self::new_from_input(
            device,
            pipelines,
            input,
            Some(body_storage),
            proxy_count,
            dynamic_count,
            fixed_count,
            candidate_capacity,
            speculative_distance,
            PRODUCT_SAP_DISPATCHES,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_from_input(
        device: &wgpu::Device,
        pipelines: &SapPipelines,
        input: wgpu::Buffer,
        body_storage: Option<&GpuBodyStorage>,
        proxy_count: u32,
        dynamic_count: u32,
        fixed_count: u32,
        candidate_capacity: u32,
        speculative_distance: f32,
        dispatches: u32,
    ) -> Result<Self, String> {
        let proxy_blocks_count = proxy_count.div_ceil(RADIX_WORKGROUP_SIZE);
        let pair_blocks_count = candidate_capacity.div_ceil(RADIX_WORKGROUP_SIZE);
        let proxies_a = storage::<ProxyRaw>(
            device,
            "sap_proxy_sort_a",
            proxy_count,
            wgpu::BufferUsages::COPY_DST,
        )?;
        let proxies_b = storage::<ProxyRaw>(
            device,
            "sap_proxy_sort_b",
            proxy_count,
            wgpu::BufferUsages::empty(),
        )?;
        let proxy_blocks = storage::<u32>(
            device,
            "sap_proxy_radix_blocks",
            proxy_blocks_count * 16,
            wgpu::BufferUsages::empty(),
        )?;
        let pairs_a = storage::<PairIndexRaw>(
            device,
            "sap_pair_sort_a",
            candidate_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let pairs_b = storage::<PairIndexRaw>(
            device,
            "sap_pair_sort_b",
            candidate_capacity,
            wgpu::BufferUsages::empty(),
        )?;
        let pair_blocks = storage::<u32>(
            device,
            "sap_pair_radix_blocks",
            pair_blocks_count * 16,
            wgpu::BufferUsages::empty(),
        )?;
        let candidates = storage::<CandidatePairRaw>(
            device,
            "sap_candidates",
            candidate_capacity,
            wgpu::BufferUsages::COPY_SRC,
        )?;
        let work =
            storage::<BroadphaseWorkRaw>(device, "sap_work", 1, wgpu::BufferUsages::COPY_SRC)?;
        let collision_work = storage::<CollisionWorkRaw>(
            device,
            "sap_collision_work",
            1,
            wgpu::BufferUsages::COPY_SRC,
        )?;
        let capacity =
            storage::<CapacityRaw>(device, "sap_capacity", 1, wgpu::BufferUsages::COPY_SRC)?;
        let indirect = storage::<u32>(device, "sap_indirect", 6, wgpu::BufferUsages::INDIRECT)?;

        let base = SapParamsRaw {
            counts: [
                proxy_count,
                proxy_blocks_count,
                candidate_capacity,
                pair_blocks_count,
            ],
            radix: [0, 0, PROXY_RADIX_PASSES, PAIR_RADIX_PASSES],
            execution: [dynamic_count, fixed_count, dispatches, 0],
            policy: [speculative_distance, 0.0, 0.0, 0.0],
        };
        let mut parameters =
            Vec::with_capacity((1 + PROXY_RADIX_PASSES + PAIR_RADIX_PASSES) as usize);
        parameters.push(uniform(device, "sap_base_params", &base));
        let clear_group = group(
            device,
            &pipelines.clear_layout,
            &[
                (8, &work),
                (9, &capacity),
                (11, &indirect),
                (12, &collision_work),
            ],
            &parameters[0],
        );
        let body_aabb_group = body_storage.map(|storage| {
            group(
                device,
                &pipelines.body_aabb_layout,
                &[
                    (0, &input),
                    (13, storage.pose_buffer()),
                    (14, storage.properties_buffer()),
                    (15, storage.metadata_buffer()),
                ],
                &parameters[0],
            )
        });
        let mut proxy_radix_groups = Vec::with_capacity(PROXY_RADIX_PASSES as usize);
        for pass in 0..PROXY_RADIX_PASSES {
            let mut values = base;
            values.radix[1] = pass * 4;
            parameters.push(uniform(device, "sap_proxy_radix_params", &values));
            let (source, destination) = if pass % 2 == 0 {
                (&proxies_a, &proxies_b)
            } else {
                (&proxies_b, &proxies_a)
            };
            proxy_radix_groups.push(group(
                device,
                &pipelines.proxy_radix_layout,
                &[(1, source), (2, destination), (3, &proxy_blocks)],
                parameters.last().expect("proxy radix parameter exists"),
            ));
        }
        let enumerate_group = group(
            device,
            &pipelines.enumerate_layout,
            &[(1, &proxies_a), (4, &pairs_a), (8, &work)],
            &parameters[0],
        );
        let finalize_group = group(
            device,
            &pipelines.finalize_layout,
            &[
                (8, &work),
                (9, &capacity),
                (11, &indirect),
                (12, &collision_work),
            ],
            &parameters[0],
        );
        let mut pair_radix_groups = Vec::with_capacity(PAIR_RADIX_PASSES as usize);
        for pass in 0..PAIR_RADIX_PASSES {
            let mut values = base;
            values.radix[0] = u32::from(pass >= 5);
            values.radix[1] = (pass % 5) * 4;
            parameters.push(uniform(device, "sap_pair_radix_params", &values));
            let (source, destination) = if pass % 2 == 0 {
                (&pairs_a, &pairs_b)
            } else {
                (&pairs_b, &pairs_a)
            };
            pair_radix_groups.push(group(
                device,
                &pipelines.pair_radix_layout,
                &[
                    (4, source),
                    (5, destination),
                    (6, &pair_blocks),
                    (9, &capacity),
                ],
                parameters.last().expect("pair radix parameter exists"),
            ));
        }
        let materialize_group = group(
            device,
            &pipelines.materialize_layout,
            &[(0, &input), (4, &pairs_a), (7, &candidates), (9, &capacity)],
            &parameters[0],
        );
        Ok(Self {
            input,
            proxies_a,
            _proxies_b: proxies_b,
            _proxy_blocks: proxy_blocks,
            _pairs_a: pairs_a,
            _pairs_b: pairs_b,
            _pair_blocks: pair_blocks,
            candidates,
            work,
            collision_work,
            capacity,
            indirect,
            _parameters: parameters,
            body_aabb_group,
            clear_group,
            proxy_radix_groups,
            enumerate_group,
            finalize_group,
            pair_radix_groups,
            materialize_group,
            proxy_count,
            proxy_blocks_count,
            candidate_capacity,
            pair_blocks_count,
            speculative_distance,
            dispatches,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &SapPipelines,
        timestamps: Option<(&wgpu::QuerySet, u32, u32)>,
    ) {
        self.encode_with_timestamp_range(
            encoder,
            pipelines,
            timestamps.map(|(query_set, beginning, end)| (query_set, Some(beginning), Some(end))),
        );
    }

    pub(crate) fn encode_with_beginning_timestamp(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &SapPipelines,
        timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        self.encode_with_timestamp_range(
            encoder,
            pipelines,
            timestamp.map(|(query_set, beginning)| (query_set, Some(beginning), None)),
        );
    }

    fn encode_with_timestamp_range(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &SapPipelines,
        timestamps: Option<(&wgpu::QuerySet, Option<u32>, Option<u32>)>,
    ) {
        dispatch_pass(
            encoder,
            "physbox5_sap_clear_pass",
            &pipelines.clear,
            &self.clear_group,
            1,
            timestamps.and_then(|(query_set, beginning, _)| {
                beginning.map(|beginning| (query_set, Some(beginning), None))
            }),
        );
        if let Some(group) = &self.body_aabb_group {
            dispatch_pass(
                encoder,
                "physbox5_sap_body_aabb_pass",
                &pipelines.body_aabb,
                group,
                self.proxy_count.div_ceil(GENERAL_WORKGROUP_SIZE),
                None,
            );
        }
        encoder.copy_buffer_to_buffer(
            &self.input,
            0,
            &self.proxies_a,
            0,
            u64::from(self.proxy_count) * size_of::<ProxyRaw>() as u64,
        );
        for group in &self.proxy_radix_groups {
            dispatch_pass(
                encoder,
                "physbox5_sap_proxy_histogram_pass",
                &pipelines.proxy_histograms,
                group,
                self.proxy_blocks_count,
                None,
            );
            dispatch_pass(
                encoder,
                "physbox5_sap_proxy_prefix_pass",
                &pipelines.proxy_prefix,
                group,
                1,
                None,
            );
            dispatch_pass(
                encoder,
                "physbox5_sap_proxy_scatter_pass",
                &pipelines.proxy_scatter,
                group,
                self.proxy_blocks_count,
                None,
            );
        }
        dispatch_pass(
            encoder,
            "physbox5_sap_enumerate_pass",
            &pipelines.enumerate,
            &self.enumerate_group,
            self.proxy_count.div_ceil(GENERAL_WORKGROUP_SIZE),
            None,
        );
        dispatch_pass(
            encoder,
            "physbox5_sap_finalize_pass",
            &pipelines.finalize,
            &self.finalize_group,
            1,
            None,
        );
        for group in &self.pair_radix_groups {
            dispatch_indirect_pass(
                encoder,
                "physbox5_sap_pair_histogram_pass",
                &pipelines.pair_histograms,
                group,
                &self.indirect,
                0,
                None,
            );
            dispatch_pass(
                encoder,
                "physbox5_sap_pair_prefix_pass",
                &pipelines.pair_prefix,
                group,
                1,
                None,
            );
            dispatch_indirect_pass(
                encoder,
                "physbox5_sap_pair_scatter_pass",
                &pipelines.pair_scatter,
                group,
                &self.indirect,
                0,
                None,
            );
        }
        dispatch_indirect_pass(
            encoder,
            "physbox5_sap_materialize_pass",
            &pipelines.materialize,
            &self.materialize_group,
            &self.indirect,
            12,
            timestamps.and_then(|(query_set, _, end)| end.map(|end| (query_set, None, Some(end)))),
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn copy_capacity_to(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(&self.capacity, 0, destination, 0, 16);
    }

    pub(crate) fn capacities(&self, candidate: CollisionCapacityV1) -> Vec<CollisionCapacityV1> {
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
        let mut values = vec![
            candidate,
            fixed("capacity-counter", 16, 1),
            fixed("input-aabbs", BODY_AABB_BYTES, u64::from(self.proxy_count)),
            fixed("indirect-arguments", 4, 6),
            fixed(
                "pair-index-sort-a",
                CANDIDATE_PAIR_BYTES,
                u64::from(self.candidate_capacity),
            ),
            fixed(
                "pair-index-sort-b",
                CANDIDATE_PAIR_BYTES,
                u64::from(self.candidate_capacity),
            ),
            fixed(
                "pair-radix-blocks",
                4,
                u64::from(self.pair_blocks_count) * 16,
            ),
            fixed(
                "parameters",
                PARAMETER_BYTES,
                u64::from(1 + PROXY_RADIX_PASSES + PAIR_RADIX_PASSES),
            ),
            fixed(
                "proxy-radix-blocks",
                4,
                u64::from(self.proxy_blocks_count) * 16,
            ),
            fixed("proxy-sort-a", BODY_AABB_BYTES, u64::from(self.proxy_count)),
            fixed("proxy-sort-b", BODY_AABB_BYTES, u64::from(self.proxy_count)),
            fixed("collision-work-counters", COLLISION_WORK_COUNTER_BYTES, 1),
            fixed("work-counters", BROADPHASE_WORK_BYTES, 1),
        ];
        values.sort_by(|left, right| left.class.cmp(&right.class));
        values
    }

    pub(crate) const fn dispatches(&self) -> u32 {
        self.dispatches
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.input.size()
            + self.proxies_a.size()
            + self._proxies_b.size()
            + self._proxy_blocks.size()
            + self._pairs_a.size()
            + self._pairs_b.size()
            + self._pair_blocks.size()
            + self.candidates.size()
            + self.work.size()
            + self.collision_work.size()
            + self.capacity.size()
            + self.indirect.size()
            + self._parameters.iter().map(wgpu::Buffer::size).sum::<u64>()
    }
}

impl CandidateSource for SapCase {
    fn candidate_buffer(&self) -> &wgpu::Buffer {
        &self.candidates
    }

    fn candidate_capacity_buffer(&self) -> &wgpu::Buffer {
        &self.capacity
    }

    fn work_buffer(&self) -> &wgpu::Buffer {
        &self.collision_work
    }

    fn candidate_capacity(&self) -> u32 {
        self.candidate_capacity
    }

    fn speculative_distance(&self) -> f32 {
        self.speculative_distance
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuSweepAndPruneRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: AdapterDiagnostics,
    pipelines: SapPipelines,
    timestamps_available: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuSweepAndPruneRunner {
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
        let pipelines = SapPipelines::new(&device);
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

    pub fn run_product_scenario(
        &self,
        scenario: &ScenarioV1,
        candidate_capacity: u32,
    ) -> Result<GpuCandidateOutput, String> {
        let slot_capacity = scenario
            .bodies
            .last()
            .and_then(|body| body.id.slot.checked_add(1))
            .ok_or_else(|| "product SAP scenario body slot capacity overflowed".to_owned())?;
        let storage = GpuBodyStorage::new(
            &self.device,
            slot_capacity,
            BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let case = SapCase::new_with_storage(
            &self.device,
            &self.pipelines,
            scenario,
            candidate_capacity,
            &storage,
        )?;
        let input_readback = readback(
            &self.device,
            "product_sap_input_readback",
            u64::from(slot_capacity) * u64::from(BODY_AABB_BYTES),
        );
        let candidate_readback = readback(
            &self.device,
            "product_sap_candidate_readback",
            u64::from(candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        let work_readback = readback(
            &self.device,
            "product_sap_collision_work_readback",
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        let capacity_readback = readback(&self.device, "product_sap_capacity_readback", 16);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_product_sap_encoder"),
            });
        case.encode(&mut encoder, &self.pipelines, None);
        encoder.copy_buffer_to_buffer(&case.input, 0, &input_readback, 0, input_readback.size());
        encoder.copy_buffer_to_buffer(
            &case.candidates,
            0,
            &candidate_readback,
            0,
            candidate_readback.size(),
        );
        encoder.copy_buffer_to_buffer(
            &case.collision_work,
            0,
            &work_readback,
            0,
            work_readback.size(),
        );
        encoder.copy_buffer_to_buffer(
            &case.capacity,
            0,
            &capacity_readback,
            0,
            capacity_readback.size(),
        );
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("product SAP poll failed: {error}"))?;

        let input = crate::collision_candidates::map_values::<ProxyRaw>(
            &self.device,
            &input_readback,
            slot_capacity as usize,
        )?;
        let capacity = crate::collision_candidates::map_values::<CapacityRaw>(
            &self.device,
            &capacity_readback,
            1,
        )?[0]
            .values;
        let committed = capacity[2] as usize;
        let candidates = crate::collision_candidates::map_values::<CandidatePairRaw>(
            &self.device,
            &candidate_readback,
            candidate_capacity as usize,
        )?
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
        let aabbs = input
            .into_iter()
            .filter(|raw| raw.identity_mobility[2] & MOBILITY_LIVE != 0)
            .map(|raw| BodyAabbV1 {
                body: physbox_contracts::BodyId {
                    slot: raw.identity_mobility[0],
                    generation: raw.identity_mobility[1],
                },
                lower: Vec2::new(raw.lower_upper[0], raw.lower_upper[1]),
                upper: Vec2::new(raw.lower_upper[2], raw.lower_upper[3]),
            })
            .collect::<Vec<_>>();
        let values = crate::collision_candidates::map_values::<CollisionWorkRaw>(
            &self.device,
            &work_readback,
            1,
        )?[0]
            .values;
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
        let candidate_record = CollisionCapacityV1 {
            class: "candidate-pairs".to_owned(),
            stride_bytes: CANDIDATE_PAIR_BYTES,
            capacity_elements: u64::from(capacity[0]),
            attempted_elements: u64::from(capacity[1]),
            committed_elements: u64::from(capacity[2]),
            allocated_bytes: u64::from(capacity[0]) * u64::from(CANDIDATE_PAIR_BYTES),
            high_water_bytes: u64::from(capacity[0].min(capacity[1]))
                * u64::from(CANDIDATE_PAIR_BYTES),
            overflowed: capacity[3] != 0,
            accepted: capacity[3] == 0,
        };
        let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
        let failure = candidate_record.overflowed.then(|| FailureV1 {
            kind: FailureKindV1::CapacityOverflow,
            reason: format!(
                "product SAP capacity {} rejected {} attempted pairs",
                candidate_record.capacity_elements, candidate_record.attempted_elements
            ),
            first_step: Some(0),
            first_stage: Some("broadphase-sap".to_owned()),
            reproducer_hash: Some(source_scenario_hash.clone()),
        });
        let maximum_pairs = u64::from(slot_capacity) * u64::from(slot_capacity - 1) / 2;
        let output = GpuCandidateOutput {
            adapter: self.diagnostics.clone(),
            source_scenario_hash,
            body_slot_capacity: slot_capacity,
            pair_attempt_capacity: u32::try_from(maximum_pairs)
                .map_err(|_| "product SAP pair-attempt capacity exceeds u32".to_owned())?,
            candidate_capacity,
            aabbs,
            candidates,
            work,
            capacities: case.capacities(candidate_record),
            failure,
        };
        output.validate()?;
        Ok(output)
    }

    pub fn run(
        &self,
        snapshot: &BroadphaseSnapshotV1,
        candidate_capacity: u32,
    ) -> Result<GpuSweepAndPruneOutput, String> {
        let case = SapCase::new(&self.device, &self.pipelines, snapshot, candidate_capacity)?;
        let candidates_readback = readback(
            &self.device,
            "sap_candidates_readback",
            u64::from(candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        let work_readback = readback(
            &self.device,
            "sap_work_readback",
            u64::from(BROADPHASE_WORK_BYTES),
        );
        let capacity_readback = readback(&self.device, "sap_capacity_readback", 16);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_sap_encoder"),
            });
        case.encode(&mut encoder, &self.pipelines, None);
        encoder.copy_buffer_to_buffer(
            &case.candidates,
            0,
            &candidates_readback,
            0,
            u64::from(candidate_capacity) * u64::from(CANDIDATE_PAIR_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &case.work,
            0,
            &work_readback,
            0,
            u64::from(BROADPHASE_WORK_BYTES),
        );
        encoder.copy_buffer_to_buffer(&case.capacity, 0, &capacity_readback, 0, 16);
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("GPU SAP poll failed: {error}"))?;
        let capacity_raw = crate::collision_candidates::map_values::<CapacityRaw>(
            &self.device,
            &capacity_readback,
            1,
        )?[0];
        let work_raw = crate::collision_candidates::map_values::<BroadphaseWorkRaw>(
            &self.device,
            &work_readback,
            1,
        )?[0];
        let raw = capacity_raw.values;
        let committed = raw[2] as usize;
        let candidate_raw = crate::collision_candidates::map_values::<CandidatePairRaw>(
            &self.device,
            &candidates_readback,
            candidate_capacity as usize,
        )?;
        let pairs = candidate_raw
            .into_iter()
            .take(committed)
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
        let overflowed = raw[3] != 0;
        let candidate_capacity_record = CollisionCapacityV1 {
            class: "candidate-pairs".to_owned(),
            stride_bytes: CANDIDATE_PAIR_BYTES,
            capacity_elements: u64::from(raw[0]),
            attempted_elements: u64::from(raw[1]),
            committed_elements: u64::from(raw[2]),
            allocated_bytes: u64::from(raw[0]) * u64::from(CANDIDATE_PAIR_BYTES),
            high_water_bytes: u64::from(raw[0].min(raw[1])) * u64::from(CANDIDATE_PAIR_BYTES),
            overflowed,
            accepted: !overflowed,
        };
        let source_snapshot_hash = canonical_hash(snapshot).map_err(|error| error.to_string())?;
        let failure = overflowed.then(|| FailureV1 {
            kind: FailureKindV1::CapacityOverflow,
            reason: format!(
                "SAP candidate capacity {} rejected {} attempted pairs",
                raw[0], raw[1]
            ),
            first_step: Some(0),
            first_stage: Some("broadphase-sap".to_owned()),
            reproducer_hash: Some(source_snapshot_hash.clone()),
        });
        let result = BroadphaseResultV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: BROADPHASE_CONTRACT_VERSION,
            algorithm: BroadphaseAlgorithmV1::SweepAndPrune,
            source_snapshot_hash,
            pairs,
            work,
            capacities: case.capacities(candidate_capacity_record),
            failure,
        };
        let output = GpuSweepAndPruneOutput {
            adapter: self.diagnostics.clone(),
            result,
        };
        output.validate()?;
        Ok(output)
    }

    pub fn benchmark(
        &self,
        snapshot: &BroadphaseSnapshotV1,
        candidate_capacity: u32,
        warmup_steps: u32,
        sample_steps: u32,
    ) -> Result<GpuSweepAndPruneBenchmarkOutput, String> {
        let case = SapCase::new(&self.device, &self.pipelines, snapshot, candidate_capacity)?;
        let timing = crate::broadphase_benchmark::benchmark_gpu_case(
            &self.device,
            &self.queue,
            self.timestamps_available,
            warmup_steps,
            sample_steps,
            |encoder, timestamps| case.encode(encoder, &self.pipelines, timestamps),
        )?;
        let output = GpuSweepAndPruneBenchmarkOutput {
            correctness: self.run(snapshot, candidate_capacity)?,
            timing,
        };
        output.validate()?;
        Ok(output)
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
        binding: 10,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<SapParamsRaw>() as u64),
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
        binding: 10,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_sap_group"),
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

fn uniform(device: &wgpu::Device, label: &'static str, values: &SapParamsRaw) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size_of::<SapParamsRaw>() as u64,
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
