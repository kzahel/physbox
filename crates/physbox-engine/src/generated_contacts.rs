use std::{cell::Cell, mem::size_of};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES, COLLISION_CONTACT_POINT_BYTES,
    COLLISION_MANIFOLD_BYTES, COLLISION_WORK_COUNTER_BYTES, CollisionCapacityV1, CollisionWorkV1,
    FailureKindV1, FailureV1, PERSISTENT_CONTACT_KEY_BYTES, PersistentContactV1, ScenarioV1,
    Validate, canonical_hash,
};
use wgpu::util::DeviceExt;

use crate::{
    GpuBodyStorage,
    body_floor_solver::{
        GpuFixedFloorSolver, GpuGatherTopology, GpuGatherTopologyEvidence, PointInputRaw,
        PreparedPointRaw,
    },
    collision_candidates::{CandidateCase, CandidatePipelines},
    collision_narrowphase::{NarrowphaseCase, NarrowphasePipelines},
    collision_persistence::{PersistenceCase, PersistencePipelines},
    pair_slot_persistence::{
        PAIR_RECORD_BYTES, PAIR_SLOT_DISPATCHES, PairRecordRaw, PairSlotCase, PairSlotPipelines,
        invalid_pair, pair_output,
    },
};

const WORKGROUP_SIZE: u32 = 64;
const VALIDATION_STEPS_PER_SUBMISSION: u32 = 64;
const CACHE_VALUE_BYTES: u32 = 16;
const PERSISTENT_RECORD_BYTES: u32 = PERSISTENT_CONTACT_KEY_BYTES + CACHE_VALUE_BYTES;
const COLLISION_DISPATCHES: u32 = 212;
const PAIR_SLOT_COLLISION_DISPATCHES: u32 = 15 + PAIR_SLOT_DISPATCHES;
const BRIDGE_DISPATCHES: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuContactPersistenceTopology {
    FullKeyOracle,
    StablePairSlot,
}

impl GpuContactPersistenceTopology {
    pub const fn name(self) -> &'static str {
        match self {
            Self::FullKeyOracle => "full-key-oracle",
            Self::StablePairSlot => "stable-pair-slot",
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BridgeParamsRaw {
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CapacityRaw {
    values: [u32; 4],
}

struct FullKeyBridgeCase {
    build: wgpu::BindGroup,
    store: wgpu::BindGroup,
}

struct FullKeyPersistenceGraph {
    pipelines: PersistencePipelines,
    cases: [PersistenceCase; 2],
    caches: [wgpu::Buffer; 2],
    capacities: [wgpu::Buffer; 2],
    empty_capacity: wgpu::Buffer,
    step_counter: wgpu::Buffer,
    _bridge_params: wgpu::Buffer,
    bridge_pipeline_build: wgpu::ComputePipeline,
    bridge_pipeline_store: wgpu::ComputePipeline,
    bridge: [FullKeyBridgeCase; 2],
}

struct PairBridgeCase {
    build: wgpu::BindGroup,
    store: wgpu::BindGroup,
}

struct PairSlotPersistenceGraph {
    pipelines: PairSlotPipelines,
    cases: [PairSlotCase; 2],
    pairs: [wgpu::Buffer; 2],
    capacities: [wgpu::Buffer; 2],
    empty_capacity: wgpu::Buffer,
    slot_generations_and_step: wgpu::Buffer,
    _bridge_params: wgpu::Buffer,
    bridge_pipeline_build: wgpu::ComputePipeline,
    bridge_pipeline_store: wgpu::ComputePipeline,
    bridge: [PairBridgeCase; 2],
    pair_capacity: u32,
}

enum GeneratedPersistenceGraph {
    FullKey(Box<FullKeyPersistenceGraph>),
    PairSlot(Box<PairSlotPersistenceGraph>),
}

pub(crate) struct GpuGeneratedContactGraph {
    candidate_pipelines: CandidatePipelines,
    candidate: CandidateCase,
    narrowphase_pipelines: NarrowphasePipelines,
    narrowphase: NarrowphaseCase,
    persistence: GeneratedPersistenceGraph,
    _solver_points: wgpu::Buffer,
    solver: GpuFixedFloorSolver,
    parity: Cell<usize>,
    point_capacity: u32,
}

impl GpuGeneratedContactGraph {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies(
            device,
            storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
        )
    }

    pub(crate) fn new_with_gather_topology(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
    ) -> Result<Self, String> {
        Self::new_with_topologies(
            device,
            storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            gather_topology,
            permute_edge_build,
            false,
            u32::MAX,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
    ) -> Result<Self, String> {
        if !scenario.manifolds.is_empty()
            || !scenario.contact_cache.is_empty()
            || !scenario.actions.is_empty()
        {
            return Err("generated product contacts require a neutral scenario".to_owned());
        }
        if scenario
            .bodies
            .iter()
            .enumerate()
            .any(|(index, body)| body.id.slot != index as u32)
        {
            return Err("generated product contacts currently require dense body slots".to_owned());
        }
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated contact body count exceeds u32".to_owned())?;
        let pair_count = body_count
            .checked_mul(body_count.saturating_sub(1))
            .map(|value| value / 2)
            .ok_or_else(|| "generated contact pair count overflowed".to_owned())?;
        let candidate_capacity = pair_count.min(body_count.saturating_mul(16)).max(1);
        let candidate_pipelines = CandidatePipelines::new(device);
        let candidate = CandidateCase::new_with_storage(
            device,
            &candidate_pipelines,
            scenario,
            Some(candidate_capacity),
            storage,
        )?;
        let narrowphase_pipelines = NarrowphasePipelines::new(device);
        let narrowphase = NarrowphaseCase::new_with_storage(
            device,
            &narrowphase_pipelines,
            &candidate,
            Some(candidate_capacity),
            storage,
        )?;
        let point_capacity = narrowphase.point_capacity();
        let solver_points = storage_buffer(
            device,
            "physbox5_generated_solver_points",
            point_capacity,
            size_of::<PointInputRaw>() as u32,
        )?;
        let solver = GpuFixedFloorSolver::new_generated_with_topology(
            device,
            storage,
            scenario,
            &solver_points,
            point_capacity,
            gather_topology,
            permute_edge_build,
        )?;
        let persistence = GeneratedPersistenceGraph::new(
            device,
            persistence_topology,
            &candidate,
            &narrowphase,
            &solver_points,
            &solver,
            point_capacity,
            permute_pair_build,
            fingerprint_mask,
        )?;
        Ok(Self {
            candidate_pipelines,
            candidate,
            narrowphase_pipelines,
            narrowphase,
            persistence,
            _solver_points: solver_points,
            solver,
            parity: Cell::new(0),
            point_capacity,
        })
    }

    pub(crate) fn encode_step(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        invalidate_previous_cache: bool,
    ) {
        self.encode_step_with_timestamps(encoder, invalidate_previous_cache, None);
    }

    fn encode_step_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        invalidate_previous_cache: bool,
        timestamps: Option<(&wgpu::QuerySet, u32)>,
    ) {
        let parity = self.parity.get();
        self.candidate
            .encode_with_timestamp(encoder, &self.candidate_pipelines, timestamps);
        self.narrowphase
            .encode(encoder, &self.narrowphase_pipelines);
        self.persistence.encode_before_solver(
            encoder,
            parity,
            invalidate_previous_cache,
            self.point_capacity,
            timestamps,
        );
        self.solver.encode_step(encoder);
        self.persistence
            .encode_after_solver(encoder, parity, self.point_capacity, timestamps);
        self.parity.set(1 - parity);
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        self.persistence.dispatches() + BRIDGE_DISPATCHES + self.solver.dispatches_per_step()
    }

    pub(crate) const fn point_capacity(&self) -> u32 {
        self.point_capacity
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.candidate.allocated_bytes()
            + self.narrowphase.allocated_bytes()
            + self.persistence.allocated_bytes()
            + self._solver_points.size()
            + self.solver.allocated_bytes()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn collision_persistence_allocated_bytes(&self) -> u64 {
        self.candidate.allocated_bytes()
            + self.narrowphase.allocated_bytes()
            + self.persistence.persistence_allocated_bytes()
    }

    pub(crate) fn capacity_overflowed(&self) -> bool {
        false
    }

    pub(crate) fn gather_topology_evidence(&self) -> GpuGatherTopologyEvidence {
        self.solver.gather_topology_evidence()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_evidence_layout(
        &self,
    ) -> crate::body_floor_solver::FixedFloorEvidenceLayout {
        self.solver.pass_evidence_layout()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_pass_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> crate::body_floor_solver::FixedFloorEvidenceLayout {
        self.solver.copy_pass_evidence(encoder, destination)
    }
}

impl GeneratedPersistenceGraph {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        topology: GpuContactPersistenceTopology,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        solver: &GpuFixedFloorSolver,
        point_capacity: u32,
        permute_pair_build: bool,
        fingerprint_mask: u32,
    ) -> Result<Self, String> {
        match topology {
            GpuContactPersistenceTopology::FullKeyOracle => {
                Ok(Self::FullKey(Box::new(FullKeyPersistenceGraph::new(
                    device,
                    candidate,
                    narrowphase,
                    solver_points,
                    solver,
                    point_capacity,
                )?)))
            }
            GpuContactPersistenceTopology::StablePairSlot => {
                Ok(Self::PairSlot(Box::new(PairSlotPersistenceGraph::new(
                    device,
                    candidate,
                    narrowphase,
                    solver_points,
                    solver,
                    point_capacity,
                    permute_pair_build,
                    fingerprint_mask,
                )?)))
            }
        }
    }

    fn encode_before_solver(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        parity: usize,
        invalidate_previous_cache: bool,
        point_capacity: u32,
        timestamps: Option<(&wgpu::QuerySet, u32)>,
    ) {
        match self {
            Self::FullKey(graph) => {
                graph.invalidate_previous(encoder, parity, invalidate_previous_cache);
                graph.cases[parity].encode_with_timestamps(
                    encoder,
                    &graph.pipelines,
                    timestamps.map(|(query_set, base)| (query_set, base + 1)),
                    timestamps.map(|(query_set, base)| (query_set, base + 2)),
                );
                encode_bridge(
                    encoder,
                    "physbox5_generated_contact_bridge_build",
                    &graph.bridge_pipeline_build,
                    &graph.bridge[parity].build,
                    point_capacity,
                    None,
                );
            }
            Self::PairSlot(graph) => {
                graph.invalidate_previous(encoder, parity, invalidate_previous_cache);
                graph.cases[parity].encode_with_timestamps(
                    encoder,
                    &graph.pipelines,
                    timestamps.map(|(query_set, base)| (query_set, base + 1, base + 2)),
                );
                encode_bridge(
                    encoder,
                    "physbox5_generated_pair_bridge_build",
                    &graph.bridge_pipeline_build,
                    &graph.bridge[parity].build,
                    point_capacity,
                    None,
                );
            }
        }
    }

    fn encode_after_solver(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        parity: usize,
        point_capacity: u32,
        timestamps: Option<(&wgpu::QuerySet, u32)>,
    ) {
        let timestamp = timestamps.map(|(query_set, base)| (query_set, base + 3));
        match self {
            Self::FullKey(graph) => encode_bridge(
                encoder,
                "physbox5_generated_contact_bridge_store",
                &graph.bridge_pipeline_store,
                &graph.bridge[parity].store,
                point_capacity,
                timestamp,
            ),
            Self::PairSlot(graph) => encode_bridge(
                encoder,
                "physbox5_generated_pair_bridge_store",
                &graph.bridge_pipeline_store,
                &graph.bridge[parity].store,
                graph.pair_capacity,
                timestamp,
            ),
        }
    }

    const fn topology(&self) -> GpuContactPersistenceTopology {
        match self {
            Self::FullKey(_) => GpuContactPersistenceTopology::FullKeyOracle,
            Self::PairSlot(_) => GpuContactPersistenceTopology::StablePairSlot,
        }
    }

    const fn dispatches(&self) -> u32 {
        match self {
            Self::FullKey(_) => COLLISION_DISPATCHES,
            Self::PairSlot(_) => PAIR_SLOT_COLLISION_DISPATCHES,
        }
    }

    fn allocated_bytes(&self) -> u64 {
        match self {
            Self::FullKey(graph) => graph.allocated_bytes(),
            Self::PairSlot(graph) => graph.allocated_bytes(),
        }
    }

    fn persistence_allocated_bytes(&self) -> u64 {
        match self {
            Self::FullKey(graph) => graph.persistence_allocated_bytes(),
            Self::PairSlot(graph) => graph.persistence_allocated_bytes(),
        }
    }

    fn current_records(&self, parity: usize) -> &wgpu::Buffer {
        match self {
            Self::FullKey(graph) => &graph.caches[parity],
            Self::PairSlot(graph) => &graph.pairs[parity],
        }
    }

    fn current_capacity(&self, parity: usize) -> &wgpu::Buffer {
        match self {
            Self::FullKey(graph) => &graph.capacities[parity],
            Self::PairSlot(graph) => &graph.capacities[parity],
        }
    }

    const fn record_capacity(&self, point_capacity: u32) -> u32 {
        match self {
            Self::FullKey(_) => point_capacity,
            Self::PairSlot(graph) => graph.pair_capacity,
        }
    }

    const fn record_stride(&self) -> u32 {
        match self {
            Self::FullKey(_) => PERSISTENT_RECORD_BYTES,
            Self::PairSlot(_) => PAIR_RECORD_BYTES,
        }
    }

    fn step_counter_source(&self) -> (&wgpu::Buffer, u64) {
        match self {
            Self::FullKey(graph) => (&graph.step_counter, 0),
            Self::PairSlot(graph) => (
                &graph.slot_generations_and_step,
                u64::from(graph.pair_capacity) * size_of::<u32>() as u64,
            ),
        }
    }
}

impl FullKeyPersistenceGraph {
    fn new(
        device: &wgpu::Device,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        solver: &GpuFixedFloorSolver,
        point_capacity: u32,
    ) -> Result<Self, String> {
        let pipelines = PersistencePipelines::new(device);
        let caches = [
            storage_buffer(
                device,
                "physbox5_generated_cache_a",
                point_capacity,
                PERSISTENT_RECORD_BYTES,
            )?,
            storage_buffer(
                device,
                "physbox5_generated_cache_b",
                point_capacity,
                PERSISTENT_RECORD_BYTES,
            )?,
        ];
        let empty_capacity_value = CapacityRaw {
            values: [point_capacity, 0, 0, 0],
        };
        let capacities = [
            storage_init(
                device,
                "physbox5_generated_cache_capacity_a",
                &empty_capacity_value,
            ),
            storage_init(
                device,
                "physbox5_generated_cache_capacity_b",
                &empty_capacity_value,
            ),
        ];
        let empty_capacity = storage_init(
            device,
            "physbox5_generated_empty_cache_capacity",
            &empty_capacity_value,
        );
        let step_counter = storage_init(device, "physbox5_generated_step_counter", &0_u32);
        let cases = [
            PersistenceCase::new(
                device,
                &pipelines,
                narrowphase,
                &caches[0],
                &capacities[0],
                &caches[1],
                &capacities[1],
                candidate.work_buffer(),
                &step_counter,
                point_capacity,
                1,
                false,
            )?,
            PersistenceCase::new(
                device,
                &pipelines,
                narrowphase,
                &caches[1],
                &capacities[1],
                &caches[0],
                &capacities[0],
                candidate.work_buffer(),
                &step_counter,
                point_capacity,
                2,
                false,
            )?,
        ];
        let bridge_params =
            bridge_params(device, "physbox5_generated_bridge_params", point_capacity);
        let build_layout = bridge_layout(device, "physbox5_generated_bridge_build_layout", false);
        let store_layout = bridge_layout(device, "physbox5_generated_bridge_store_layout", true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_generated_contact_bridge_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("generated_contacts.wgsl").into()),
        });
        let bridge_pipeline_build = bridge_pipeline(
            device,
            &shader,
            &build_layout,
            "physbox5_generated_bridge_build_pipeline",
            "build_solver_points",
        );
        let bridge_pipeline_store = bridge_pipeline(
            device,
            &shader,
            &store_layout,
            "physbox5_generated_bridge_store_pipeline",
            "store_solver_cache",
        );
        let bridge = [0_usize, 1_usize].map(|parity| {
            let output = 1 - parity;
            FullKeyBridgeCase {
                build: bridge_group(
                    device,
                    &build_layout,
                    &[
                        (0, narrowphase.manifolds()),
                        (1, narrowphase.points()),
                        (2, cases[parity].sorted_current()),
                        (3, &caches[output]),
                        (4, &capacities[output]),
                        (5, solver_points),
                    ],
                    &bridge_params,
                ),
                store: bridge_group(
                    device,
                    &store_layout,
                    &[
                        (3, &caches[output]),
                        (4, &capacities[output]),
                        (5, solver_points),
                        (6, solver.prepared_buffer()),
                    ],
                    &bridge_params,
                ),
            }
        });
        Ok(Self {
            pipelines,
            cases,
            caches,
            capacities,
            empty_capacity,
            step_counter,
            _bridge_params: bridge_params,
            bridge_pipeline_build,
            bridge_pipeline_store,
            bridge,
        })
    }

    fn invalidate_previous(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        parity: usize,
        invalidate: bool,
    ) {
        if invalidate {
            encoder.copy_buffer_to_buffer(
                &self.empty_capacity,
                0,
                &self.capacities[parity],
                0,
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            );
        }
    }

    fn allocated_bytes(&self) -> u64 {
        self.persistence_allocated_bytes() + self._bridge_params.size()
    }

    fn persistence_allocated_bytes(&self) -> u64 {
        self.cases
            .iter()
            .map(PersistenceCase::allocated_bytes)
            .sum::<u64>()
            + self.caches.iter().map(wgpu::Buffer::size).sum::<u64>()
            + self.capacities.iter().map(wgpu::Buffer::size).sum::<u64>()
            + self.empty_capacity.size()
            + self.step_counter.size()
    }
}

impl PairSlotPersistenceGraph {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        candidate: &CandidateCase,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        solver: &GpuFixedFloorSolver,
        point_capacity: u32,
        permute_pair_build: bool,
        fingerprint_mask: u32,
    ) -> Result<Self, String> {
        let pipelines = PairSlotPipelines::new(device);
        let pair_capacity = narrowphase.manifold_capacity();
        let empty_pairs = vec![invalid_pair(); pair_capacity as usize];
        let pairs = [
            storage_slice_init(device, "physbox5_generated_pair_cache_a", &empty_pairs),
            storage_slice_init(device, "physbox5_generated_pair_cache_b", &empty_pairs),
        ];
        let empty_capacity_value = CapacityRaw {
            values: [pair_capacity, 0, 0, 0],
        };
        let capacities = [
            storage_init(
                device,
                "physbox5_generated_pair_capacity_a",
                &empty_capacity_value,
            ),
            storage_init(
                device,
                "physbox5_generated_pair_capacity_b",
                &empty_capacity_value,
            ),
        ];
        let empty_capacity = storage_init(
            device,
            "physbox5_generated_empty_pair_capacity",
            &empty_capacity_value,
        );
        let slot_generations_and_step = storage_slice_init(
            device,
            "physbox5_generated_pair_generations_and_step",
            &vec![0_u32; pair_capacity as usize + 1],
        );
        let cases = [
            PairSlotCase::new(
                device,
                &pipelines,
                narrowphase,
                &pairs[0],
                &capacities[0],
                &pairs[1],
                &capacities[1],
                &slot_generations_and_step,
                candidate.work_buffer(),
                pair_capacity,
                permute_pair_build,
                fingerprint_mask,
            )?,
            PairSlotCase::new(
                device,
                &pipelines,
                narrowphase,
                &pairs[1],
                &capacities[1],
                &pairs[0],
                &capacities[0],
                &slot_generations_and_step,
                candidate.work_buffer(),
                pair_capacity,
                permute_pair_build,
                fingerprint_mask,
            )?,
        ];
        let bridge_params = bridge_params(
            device,
            "physbox5_generated_pair_bridge_params",
            point_capacity,
        );
        let build_layout =
            pair_bridge_layout(device, "physbox5_generated_pair_bridge_build_layout", false);
        let store_layout =
            pair_bridge_layout(device, "physbox5_generated_pair_bridge_store_layout", true);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_generated_pair_bridge_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("generated_pair_contacts.wgsl").into()),
        });
        let bridge_pipeline_build = bridge_pipeline(
            device,
            &shader,
            &build_layout,
            "physbox5_generated_pair_bridge_build_pipeline",
            "build_pair_solver_points",
        );
        let bridge_pipeline_store = bridge_pipeline(
            device,
            &shader,
            &store_layout,
            "physbox5_generated_pair_bridge_store_pipeline",
            "store_pair_solver_cache",
        );
        let bridge = [0_usize, 1_usize].map(|parity| {
            let output = 1 - parity;
            PairBridgeCase {
                build: bridge_group(
                    device,
                    &build_layout,
                    &[
                        (0, narrowphase.manifolds()),
                        (1, narrowphase.points()),
                        (2, &pairs[output]),
                        (3, &capacities[output]),
                        (4, narrowphase.point_capacity_buffer()),
                        (5, solver_points),
                    ],
                    &bridge_params,
                ),
                store: bridge_group(
                    device,
                    &store_layout,
                    &[
                        (0, narrowphase.manifolds()),
                        (2, &pairs[output]),
                        (3, &capacities[output]),
                        (6, solver.prepared_buffer()),
                    ],
                    &bridge_params,
                ),
            }
        });
        Ok(Self {
            pipelines,
            cases,
            pairs,
            capacities,
            empty_capacity,
            slot_generations_and_step,
            _bridge_params: bridge_params,
            bridge_pipeline_build,
            bridge_pipeline_store,
            bridge,
            pair_capacity,
        })
    }

    fn invalidate_previous(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        parity: usize,
        invalidate: bool,
    ) {
        if invalidate {
            encoder.copy_buffer_to_buffer(
                &self.empty_capacity,
                0,
                &self.capacities[parity],
                0,
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
            );
        }
    }

    fn allocated_bytes(&self) -> u64 {
        self.persistence_allocated_bytes() + self._bridge_params.size()
    }

    fn persistence_allocated_bytes(&self) -> u64 {
        self.cases
            .iter()
            .map(PairSlotCase::allocated_bytes)
            .sum::<u64>()
            + self.pairs.iter().map(wgpu::Buffer::size).sum::<u64>()
            + self.capacities.iter().map(wgpu::Buffer::size).sum::<u64>()
            + self.empty_capacity.size()
            + self.slot_generations_and_step.size()
    }
}

fn encode_bridge(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::ComputePipeline,
    group: &wgpu::BindGroup,
    capacity: u32,
    timestamp: Option<(&wgpu::QuerySet, u32)>,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: timestamp.map(|(query_set, end)| wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: None,
            end_of_pass_write_index: Some(end),
        }),
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups(capacity.div_ceil(WORKGROUP_SIZE), 1, 1);
}

fn storage_buffer(
    device: &wgpu::Device,
    label: &'static str,
    count: u32,
    stride: u32,
) -> Result<wgpu::Buffer, String> {
    let size = u64::from(count)
        .checked_mul(u64::from(stride))
        .ok_or_else(|| format!("{label} byte count overflowed"))?;
    let limits = device.limits();
    if size == 0
        || size > limits.max_buffer_size
        || size > u64::from(limits.max_storage_buffer_binding_size)
    {
        return Err(format!(
            "{label} requires {size} bytes outside portable limits"
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

fn storage_init<T: Pod>(device: &wgpu::Device, label: &'static str, value: &T) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(value),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    })
}

fn storage_slice_init<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    values: &[T],
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(values),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    })
}

fn bridge_params(device: &wgpu::Device, label: &'static str, point_capacity: u32) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(&BridgeParamsRaw {
            counts: [point_capacity, 0, 0, 0],
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

fn bridge_layout(
    device: &wgpu::Device,
    label: &'static str,
    store_only: bool,
) -> wgpu::BindGroupLayout {
    let bindings: &[(u32, bool)] = if store_only {
        &[(3, false), (4, false), (5, true), (6, true)]
    } else {
        &[
            (0, true),
            (1, true),
            (2, true),
            (3, false),
            (4, false),
            (5, false),
        ]
    };
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
        binding: 7,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<BridgeParamsRaw>() as u64),
        },
        count: None,
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn pair_bridge_layout(
    device: &wgpu::Device,
    label: &'static str,
    store_only: bool,
) -> wgpu::BindGroupLayout {
    let bindings: &[(u32, bool)] = if store_only {
        &[(0, true), (2, false), (3, false), (6, true)]
    } else {
        &[
            (0, true),
            (1, true),
            (2, false),
            (3, false),
            (4, false),
            (5, false),
        ]
    };
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
        binding: 7,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size_of::<BridgeParamsRaw>() as u64),
        },
        count: None,
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn bridge_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    entry: &'static str,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn bridge_group(
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
        binding: 7,
        resource: params.as_entire_binding(),
    });
    entries.sort_by_key(|entry| entry.binding);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("physbox5_generated_contact_bridge_group"),
        layout,
        entries: &entries,
    })
}

const _: () = assert!(size_of::<PointInputRaw>() == 80);
const _: () = assert!(size_of::<PreparedPointRaw>() == 96);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactOutput {
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub completed_steps: u32,
    pub mapped_after_final_step_only: bool,
    pub bodies: Vec<physbox_contracts::BodyStateV1>,
    pub cache: Vec<PersistentContactV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub persistence_topology: GpuContactPersistenceTopology,
    pub gather_topology: GpuGatherTopologyEvidence,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.completed_steps == 0 || !self.mapped_after_final_step_only {
            return Err("generated contact output has an invalid completion phase".to_owned());
        }
        for body in &self.bodies {
            body.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.cache.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err("generated contact cache is not sorted and unique".to_owned());
            }
        }
        for contact in &self.cache {
            contact.validate().map_err(|error| error.to_string())?;
            if contact.value.last_seen_step > self.completed_steps {
                return Err("generated contact cache was seen after completion".to_owned());
            }
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("generated contact capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        if let Some(failure) = &self.failure {
            failure.validate().map_err(|error| error.to_string())?;
        }
        self.gather_topology.validate()?;
        let rejected = self.capacities.iter().any(|capacity| !capacity.accepted);
        if rejected != self.failure.is_some() || (rejected && !self.cache.is_empty()) {
            return Err("generated contact failure/capacity state is inconsistent".to_owned());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct GpuGeneratedContactRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    diagnostics: crate::AdapterDiagnostics,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactRunner {
    pub async fn new() -> Result<Self, String> {
        let instance = crate::gpu_context::create_instance();
        let adapter = crate::gpu_context::request_adapter(&instance, None).await?;
        let diagnostics = crate::AdapterDiagnostics::from_adapter(&adapter);
        let (device, queue) = crate::gpu_context::request_device(&adapter).await?;
        Ok(Self {
            device,
            queue,
            diagnostics,
        })
    }

    pub fn diagnostics(&self) -> &crate::AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn adapter(&self) -> &crate::AdapterDiagnostics {
        &self.diagnostics
    }

    pub fn run(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_gather_topology(
            scenario,
            completed_steps,
            GpuGatherTopology::StableBodyEdge,
            false,
        )
    }

    pub fn run_with_gather_topology(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            gather_topology,
            permute_edge_build,
            false,
            u32::MAX,
        )
    }

    pub fn run_full_key_oracle(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::FullKeyOracle,
            GpuGatherTopology::CanonicalBodyScan,
            false,
            false,
            u32::MAX,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_with_topologies(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
    ) -> Result<GpuGeneratedContactOutput, String> {
        if completed_steps == 0 {
            return Err("generated contact validation requires at least one step".to_owned());
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated validation body count exceeds u32".to_owned())?;
        let storage = GpuBodyStorage::new(
            &self.device,
            body_count,
            crate::BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let graph = GpuGeneratedContactGraph::new_with_topologies(
            &self.device,
            &storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
        )?;
        let lane_bytes = u64::from(body_count) * size_of::<[u32; 4]>() as u64;
        let state_readback = readback(&self.device, "generated_state_readback", lane_bytes * 3);
        let cache_readback = readback(
            &self.device,
            "generated_cache_readback",
            u64::from(graph.persistence.record_capacity(graph.point_capacity))
                * u64::from(graph.persistence.record_stride()),
        );
        let candidate_capacity_readback = readback(
            &self.device,
            "generated_candidate_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let manifold_capacity_readback = readback(
            &self.device,
            "generated_manifold_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let point_capacity_readback = readback(
            &self.device,
            "generated_point_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let persistence_capacity_readback = readback(
            &self.device,
            "generated_persistence_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        let work_readback = readback(
            &self.device,
            "generated_work_readback",
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        let step_readback = readback(&self.device, "generated_step_readback", 4);
        let mut remaining = completed_steps;
        while remaining > 0 {
            let chunk = remaining.min(VALIDATION_STEPS_PER_SUBMISSION);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_generated_contact_validation_steps"),
                });
            for _ in 0..chunk {
                graph.encode_step(&mut encoder, false);
            }
            self.queue.submit([encoder.finish()]);
            remaining -= chunk;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_generated_contact_validation_readback"),
            });
        let current = graph.parity.get();
        encoder.copy_buffer_to_buffer(storage.pose_buffer(), 0, &state_readback, 0, lane_bytes);
        encoder.copy_buffer_to_buffer(
            storage.motion_buffer(),
            0,
            &state_readback,
            lane_bytes,
            lane_bytes,
        );
        encoder.copy_buffer_to_buffer(
            storage.metadata_buffer(),
            0,
            &state_readback,
            lane_bytes * 2,
            lane_bytes,
        );
        copy_full(
            &mut encoder,
            graph.persistence.current_records(current),
            &cache_readback,
        );
        copy_full(
            &mut encoder,
            graph.candidate.candidate_capacity_buffer(),
            &candidate_capacity_readback,
        );
        copy_full(
            &mut encoder,
            graph.narrowphase.manifold_capacity_buffer(),
            &manifold_capacity_readback,
        );
        copy_full(
            &mut encoder,
            graph.narrowphase.point_capacity_buffer(),
            &point_capacity_readback,
        );
        copy_full(
            &mut encoder,
            graph.persistence.current_capacity(current),
            &persistence_capacity_readback,
        );
        copy_full(&mut encoder, graph.candidate.work_buffer(), &work_readback);
        let (step_source, step_offset) = graph.persistence.step_counter_source();
        encoder.copy_buffer_to_buffer(step_source, step_offset, &step_readback, 0, 4);
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("generated validation poll failed: {error}"))?;

        let poses = crate::collision_candidates::map_values::<[f32; 4]>(
            &self.device,
            &state_readback,
            body_count as usize,
        )?;
        let motions = map_range::<[f32; 4]>(
            &self.device,
            &state_readback,
            lane_bytes,
            body_count as usize,
        )?;
        let metadata = map_range::<[u32; 4]>(
            &self.device,
            &state_readback,
            lane_bytes * 2,
            body_count as usize,
        )?;
        let candidate_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &candidate_capacity_readback,
            1,
        )?[0];
        let manifold_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &manifold_capacity_readback,
            1,
        )?[0];
        let point_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &point_capacity_readback,
            1,
        )?[0];
        let persistence_raw = crate::collision_candidates::map_values::<
            crate::collision_persistence::CapacityRaw,
        >(&self.device, &persistence_capacity_readback, 1)?[0];
        let work = crate::collision_persistence::work_record(
            crate::collision_candidates::map_values::<crate::collision_persistence::WorkRaw>(
                &self.device,
                &work_readback,
                1,
            )?[0],
        );
        let gpu_steps =
            crate::collision_candidates::map_values::<u32>(&self.device, &step_readback, 1)?[0];
        if gpu_steps != completed_steps {
            return Err(format!(
                "GPU completed-step counter {gpu_steps} differed from {completed_steps}"
            ));
        }
        let any_rejected = candidate_raw[3] != 0
            || manifold_raw[2] != manifold_raw[1]
            || point_raw[2] != point_raw[1]
            || persistence_raw.values[2] != persistence_raw.values[1];
        let cache = if any_rejected {
            Vec::new()
        } else {
            match &graph.persistence {
                GeneratedPersistenceGraph::FullKey(_) => {
                    crate::collision_candidates::map_values::<
                        crate::collision_persistence::PersistentRecordRaw,
                    >(
                        &self.device, &cache_readback, graph.point_capacity as usize
                    )?
                    .into_iter()
                    .take(persistence_raw.values[2] as usize)
                    .map(crate::collision_persistence::persistent_contact)
                    .collect::<Result<Vec<_>, _>>()?
                }
                GeneratedPersistenceGraph::PairSlot(pair_graph) => {
                    let records = crate::collision_candidates::map_values::<PairRecordRaw>(
                        &self.device,
                        &cache_readback,
                        pair_graph.pair_capacity as usize,
                    )?;
                    let mut cache = Vec::with_capacity(point_raw[2] as usize);
                    for record in records.into_iter().take(persistence_raw.values[2] as usize) {
                        let (_, mut points) = pair_output(record)?;
                        cache.append(&mut points);
                    }
                    cache
                }
            }
        };
        let mut bodies = scenario.bodies.clone();
        for (index, body) in bodies.iter_mut().enumerate() {
            if metadata[index][0] != body.id.generation {
                return Err(format!("body {index} generation changed during validation"));
            }
            body.position.x = poses[index][0];
            body.position.y = poses[index][1];
            body.angle = physbox_contracts::FiniteF32::new(poses[index][2])
                .map_err(|error| error.to_string())?;
            body.linear_velocity.x = motions[index][0];
            body.linear_velocity.y = motions[index][1];
            body.angular_velocity = physbox_contracts::FiniteF32::new(motions[index][2])
                .map_err(|error| error.to_string())?;
        }
        let candidate_capacity =
            raw_capacity("candidate-pairs", CANDIDATE_PAIR_BYTES, candidate_raw);
        let manifold_capacity =
            raw_capacity("contact-manifolds", COLLISION_MANIFOLD_BYTES, manifold_raw);
        let point_capacity =
            raw_capacity("contact-points", COLLISION_CONTACT_POINT_BYTES, point_raw);
        let mut capacities = graph.candidate.capacities(candidate_capacity);
        capacities.extend(graph.narrowphase.capacities(
            manifold_capacity,
            point_capacity,
            work.narrowphase_tests,
        ));
        match &graph.persistence {
            GeneratedPersistenceGraph::FullKey(_) => {
                let persistence_capacity = crate::collision_persistence::capacity_record(
                    "persistent-cache",
                    PERSISTENT_RECORD_BYTES,
                    persistence_raw,
                );
                capacities.extend(crate::collision_persistence::persistence_capacities(
                    graph.point_capacity,
                    work.cache_previous,
                    work.cache_current,
                    persistence_capacity,
                ));
            }
            GeneratedPersistenceGraph::PairSlot(pair_graph) => {
                capacities.push(crate::collision_persistence::capacity_record(
                    "pair-slot-cache",
                    PAIR_RECORD_BYTES,
                    persistence_raw,
                ));
                capacities.extend(pair_graph.cases[0].capacities().into_iter().map(
                    |mut capacity| {
                        capacity.capacity_elements *= 2;
                        capacity.attempted_elements *= 2;
                        capacity.committed_elements *= 2;
                        capacity.allocated_bytes *= 2;
                        capacity.high_water_bytes *= 2;
                        capacity
                    },
                ));
                capacities.push(fixed_capacity(
                    "pair-slot-cache-buffers",
                    PAIR_RECORD_BYTES,
                    u64::from(pair_graph.pair_capacity) * 2,
                ));
                capacities.push(fixed_capacity(
                    "pair-slot-generations-and-step",
                    4,
                    u64::from(pair_graph.pair_capacity) + 1,
                ));
            }
        }
        capacities.sort_by(|left, right| left.class.cmp(&right.class));
        let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
        let failure = capacities
            .iter()
            .find(|capacity| !capacity.accepted)
            .map(|capacity| FailureV1 {
                kind: FailureKindV1::CapacityOverflow,
                reason: format!(
                    "{} capacity {} rejected {} attempted records",
                    capacity.class, capacity.capacity_elements, capacity.attempted_elements
                ),
                first_step: Some(completed_steps),
                first_stage: Some(if capacity.class == "candidate-pairs" {
                    "candidate".to_owned()
                } else if capacity.class == "persistent-cache"
                    || capacity.class == "pair-slot-cache"
                {
                    "persistence".to_owned()
                } else {
                    "narrowphase".to_owned()
                }),
                reproducer_hash: Some(source_scenario_hash.clone()),
            });
        let output = GpuGeneratedContactOutput {
            adapter: self.diagnostics.clone(),
            source_scenario_hash,
            completed_steps,
            mapped_after_final_step_only: true,
            bodies,
            cache,
            work,
            capacities,
            failure,
            dispatches_per_step: graph.dispatches_per_step(),
            allocated_bytes: graph.allocated_bytes(),
            persistence_topology: graph.persistence.topology(),
            gather_topology: graph.gather_topology_evidence(),
        };
        output.validate()?;
        Ok(output)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactBenchmarkOutput {
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub body_count: u32,
    pub warmup_steps: u32,
    pub samples: u32,
    pub gpu_collision_seconds: Option<Vec<f64>>,
    pub gpu_persistence_seconds: Option<Vec<f64>>,
    pub gpu_end_to_end_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub collision_persistence_allocated_bytes: u64,
    pub correctness: GpuGeneratedContactOutput,
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_generated_contacts(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    benchmark_gpu_generated_contacts_with_gather_topology(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_generated_contacts_with_gather_topology(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    gather_topology: GpuGatherTopology,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    if warmup_steps == 0 || samples == 0 {
        return Err("generated-contact benchmark requires non-zero warmup and samples".to_owned());
    }
    scenario.validate().map_err(|error| error.to_string())?;
    let correctness = {
        let runner = GpuGeneratedContactRunner::new().await?;
        runner.run_with_gather_topology(scenario, 1, gather_topology, false)?
    };
    if correctness.failure.is_some()
        || correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted)
    {
        return Err("generated-contact benchmark correctness/capacity gate failed".to_owned());
    }

    let query_count = samples
        .checked_mul(4)
        .ok_or_else(|| "generated-contact benchmark query count overflowed".to_owned())?;
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
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "generated benchmark body count exceeds u32".to_owned())?;
    let storage = GpuBodyStorage::new(
        &device,
        body_count,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_count, &scenario.bodies)?;
    let graph = GpuGeneratedContactGraph::new_with_gather_topology(
        &device,
        &storage,
        scenario,
        gather_topology,
        false,
    )?;
    let query_resources = timestamps_available.then(|| {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("generated_contact_benchmark_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });
        let byte_count = u64::from(query_count) * size_of::<u64>() as u64;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("generated_contact_timestamp_resolve"),
            size: byte_count,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("generated_contact_timestamp_readback"),
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
            .map_err(|error| format!("poll generated benchmark warmup reset: {error}"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("generated_contact_benchmark_warmup"),
        });
        graph.encode_step(&mut encoder, true);
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll generated benchmark warmup: {error}"))?;
    }

    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        storage.upload_reset_data(&queue, &reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll generated benchmark sample reset: {error}"))?;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("generated_contact_benchmark_sample"),
        });
        let timestamps = query_resources
            .as_ref()
            .map(|(query_set, _, _)| (query_set, sample * 4));
        graph.encode_step_with_timestamps(&mut encoder, true, timestamps);
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll generated benchmark sample: {error}"))?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }

    let (
        gpu_collision_seconds,
        gpu_persistence_seconds,
        gpu_end_to_end_seconds,
        timestamp_period_nanoseconds,
    ) = if let Some((query_set, resolve, readback)) = &query_resources {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("generated_contact_timestamp_copy"),
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
        let collision = values
            .chunks_exact(4)
            .map(|triple| triple[2].saturating_sub(triple[0]) as f64 * f64::from(period) * 1.0e-9)
            .collect();
        let persistence = values
            .chunks_exact(4)
            .map(|quad| quad[2].saturating_sub(quad[1]) as f64 * f64::from(period) * 1.0e-9)
            .collect();
        let end_to_end = values
            .chunks_exact(4)
            .map(|quad| quad[3].saturating_sub(quad[0]) as f64 * f64::from(period) * 1.0e-9)
            .collect();
        (
            Some(collision),
            Some(persistence),
            Some(end_to_end),
            Some(period),
        )
    } else {
        (None, None, None, None)
    };

    Ok(GpuGeneratedContactBenchmarkOutput {
        adapter: diagnostics,
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        body_count,
        warmup_steps,
        samples,
        gpu_collision_seconds,
        gpu_persistence_seconds,
        gpu_end_to_end_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        timestamp_period_nanoseconds,
        dispatches_per_step: graph.dispatches_per_step(),
        allocated_bytes: graph.allocated_bytes(),
        collision_persistence_allocated_bytes: graph.collision_persistence_allocated_bytes(),
        correctness,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn raw_capacity(class: &str, stride: u32, values: [u32; 4]) -> CollisionCapacityV1 {
    CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes: stride,
        capacity_elements: u64::from(values[0]),
        attempted_elements: u64::from(values[1]),
        committed_elements: u64::from(values[2]),
        allocated_bytes: u64::from(values[0]) * u64::from(stride),
        high_water_bytes: u64::from(values[1].min(values[0])) * u64::from(stride),
        overflowed: values[3] != 0,
        accepted: values[3] == 0 && values[2] == values[1],
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fixed_capacity(class: &str, stride: u32, elements: u64) -> CollisionCapacityV1 {
    CollisionCapacityV1 {
        class: class.to_owned(),
        stride_bytes: stride,
        capacity_elements: elements,
        attempted_elements: elements,
        committed_elements: elements,
        allocated_bytes: elements * u64::from(stride),
        high_water_bytes: elements * u64::from(stride),
        overflowed: false,
        accepted: true,
    }
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
fn copy_full(
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Buffer,
    destination: &wgpu::Buffer,
) {
    encoder.copy_buffer_to_buffer(source, 0, destination, 0, destination.size());
}

#[cfg(not(target_arch = "wasm32"))]
fn map_range<T: Pod + Copy>(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    offset: u64,
    count: usize,
) -> Result<Vec<T>, String> {
    use std::sync::mpsc;

    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or_else(|| "generated validation map byte count overflowed".to_owned())?
        as u64;
    let slice = buffer.slice(offset..offset + bytes);
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("generated validation map poll failed: {error}"))?;
    receiver
        .recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("generated validation map failed: {error}"))?;
    let mapped = slice.get_mapped_range();
    let values = bytemuck::cast_slice::<u8, T>(&mapped).to_vec();
    drop(mapped);
    buffer.unmap();
    Ok(values)
}
