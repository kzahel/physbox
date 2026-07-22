// Product hosts use the shared contact graph below. Native-only experiment
// runners reuse its internals through the opt-in `solver-lab` API.
#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

use std::{cell::Cell, mem::size_of, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CANDIDATE_PAIR_BYTES, COLLISION_CAPACITY_COUNTER_BYTES, COLLISION_CONTACT_POINT_BYTES,
    COLLISION_MANIFOLD_BYTES, COLLISION_WORK_COUNTER_BYTES, CapacityRecordV1, CollisionCapacityV1,
    CollisionWorkV1, ContactIdentityPolicyV1, FailureKindV1, FailureV1,
    PERSISTENT_CONTACT_KEY_BYTES, PersistentContactV1, ScenarioV1, Validate, canonical_hash,
};
use wgpu::util::DeviceExt;

use crate::{
    GpuBodyStorage,
    awake_points::GpuAwakePoints,
    awake_work::GpuAwakeWork,
    body_floor_solver::{
        GpuFixedFloorSolver, GpuGatherTopology, GpuGatherTopologyEvidence, GpuSolverExecutionV4,
        GpuSolverSchedulingPolicyV1, PointInputRaw, PreparedPointRaw, validate_selected_settings,
    },
    broadphase_sap::{SapCase, SapPipelines},
    collision_candidates::CandidateSource,
    collision_narrowphase::{NarrowphaseCase, NarrowphasePipelines, TemporalIdentityStage},
    collision_persistence::{PersistenceCase, PersistencePipelines},
    pair_slot_persistence::{
        PAIR_RECORD_BYTES, PAIR_SLOT_DISPATCHES, PairRecordRaw, PairSlotAllocator, PairSlotCase,
        PairSlotPipelines, invalid_pair, pair_output,
    },
    sleep_contacts::GpuSleepContacts,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::body_floor_solver::{
    BenchmarkTimestampResources, GpuFixedFloorContactEvidence, GpuSchedulerWorkVectorV1,
    GpuSolverInstrumentedSampleV4, GpuSolverPassDescriptorV1, decode_prepared_contact_bytes,
};

const WORKGROUP_SIZE: u32 = 64;
const VALIDATION_STEPS_PER_SUBMISSION: u32 = 64;
#[cfg(not(target_arch = "wasm32"))]
const VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH: u32 = 64;
const CACHE_VALUE_BYTES: u32 = 16;
const PERSISTENT_RECORD_BYTES: u32 = PERSISTENT_CONTACT_KEY_BYTES + CACHE_VALUE_BYTES;
const COLLISION_DISPATCHES: u32 = 212;
const PAIR_SLOT_COLLISION_DISPATCHES: u32 = 15 + PAIR_SLOT_DISPATCHES;
const BRIDGE_DISPATCHES: u32 = 2;
const PRODUCT_CANDIDATE_PAIRS_PER_BODY: u32 = 8;
pub const GPU_GENERATED_CONTACT_BODY_HISTORY_VERSION: u32 = 1;
#[cfg(not(target_arch = "wasm32"))]
pub const GPU_GENERATED_CONTACT_PASS_TRACE_VERSION: u32 = 3;
#[cfg(not(target_arch = "wasm32"))]
pub const GPU_AUTHORITY_SEPARATION_DIAGNOSTIC_VERSION: u32 = 1;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoritySeparationInputV1 {
    pub normal: physbox_contracts::Vec2,
    pub anchor_a: physbox_contracts::Vec2,
    pub anchor_b: physbox_contracts::Vec2,
    pub pose_a: physbox_contracts::Vec2,
    pub pose_b: physbox_contracts::Vec2,
    pub initial_position_a: physbox_contracts::Vec2,
    pub initial_position_b: physbox_contracts::Vec2,
    pub angle_a: f32,
    pub angle_b: f32,
    pub initial_angle_a: f32,
    pub initial_angle_b: f32,
    pub input_separation: f32,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoritySeparationValueV1 {
    pub name: &'static str,
    pub value: f32,
    pub bits: u32,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoritySeparationDiagnosticV1 {
    pub version: u32,
    pub adapter: crate::AdapterDiagnostics,
    pub input: GpuAuthoritySeparationInputV1,
    pub values: Vec<GpuAuthoritySeparationValueV1>,
}

#[cfg(not(target_arch = "wasm32"))]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AuthoritySeparationWordsRaw {
    values: [u32; 64],
}
#[cfg(not(target_arch = "wasm32"))]
pub const GPU_GENERATED_CONTACT_PASS_MOTION_WINDOW_VERSION: u32 = 1;

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
    allocator: PairSlotAllocator,
    _bridge_params: wgpu::Buffer,
    bridge_pipeline_build: wgpu::ComputePipeline,
    bridge_pipeline_store: wgpu::ComputePipeline,
    bridge: [PairBridgeCase; 2],
    pair_capacity: u32,
    temporal_identity: Option<TemporalIdentityStage>,
}

enum GeneratedPersistenceGraph {
    FullKey(Box<FullKeyPersistenceGraph>),
    PairSlot(Box<PairSlotPersistenceGraph>),
}

pub(crate) struct GpuGeneratedContactGraph {
    broadphase_pipelines: SapPipelines,
    broadphase: SapCase,
    narrowphase_pipelines: NarrowphasePipelines,
    narrowphase: NarrowphaseCase,
    persistence: GeneratedPersistenceGraph,
    _solver_points: wgpu::Buffer,
    solver: GpuFixedFloorSolver,
    parity: Cell<usize>,
    point_capacity: u32,
    identity_policy: ContactIdentityPolicyV1,
    contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    substep_contact_policy: crate::GpuSubstepContactPolicyV1,
    restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    authority_identity: bool,
    awake_work: Arc<GpuAwakeWork>,
    sleep_contacts: GpuSleepContacts,
    awake_points: GpuAwakePoints,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuConstructionGraphEvidence {
    pub candidate_pairs: u32,
    pub manifolds: u32,
    pub contact_points: u32,
    pub persistent_pairs: u32,
    pub overflowed: bool,
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
        validate_selected_settings(scenario)?;
        let scheduling_policy = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(
            storage.layout().capacity_elements,
        );
        Self::new_with_topologies_execution_and_scheduling(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_and_scheduling(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_and_identity_policy(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            ContactIdentityPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_scheduling_and_identity_policy(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_identity_and_soft_feedback(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            crate::GpuSoftFeedbackPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_scheduling_identity_and_soft_feedback(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_identity_and_contact_feedback(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            soft_feedback_policy,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_scheduling_identity_and_contact_feedback(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_identity_and_solver_policies(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_scheduling_identity_and_solver_policies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_topologies_execution_scheduling_identity_extended_and_authority_mode(
            device,
            storage,
            scenario,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            false,
        )
    }

    pub(crate) fn new_authority_gpu_probe(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        crate::AuthorityGpuProbePolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::authority_gpu_probe::validate_authority_gpu_probe_settings(&scenario.settings)?;
        Self::new_with_topologies_execution_scheduling_identity_extended_and_authority_mode(
            device,
            storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                storage.layout().capacity_elements,
            ),
            ContactIdentityPolicyV1::SELECTED,
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    pub(crate) fn new_branch_robust_gpu_promotion(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_settings(
            &scenario.settings,
        )?;
        Self::new_with_topologies_execution_scheduling_identity_extended_and_authority_mode(
            device,
            storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                storage.layout().capacity_elements,
            ),
            ContactIdentityPolicyV1::SELECTED,
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    #[cfg_attr(feature = "production-validation", allow(dead_code))]
    pub(crate) fn new_branch_robust_tunable(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        crate::branch_robust_gpu_promotion::validate_branch_robust_tunable_settings(
            &scenario.settings,
        )?;
        Self::new_with_topologies_execution_scheduling_identity_extended_and_authority_mode(
            device,
            storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                storage.layout().capacity_elements,
            ),
            ContactIdentityPolicyV1::SELECTED,
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_topologies_execution_scheduling_identity_extended_and_authority_mode(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
        authority_identity: bool,
    ) -> Result<Self, String> {
        if authority_identity {
            if substep_contact_policy
                == crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
            {
                if contact_coefficient_policy
                    == crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft
                {
                    crate::branch_robust_gpu_promotion::validate_branch_robust_tunable_runtime_tuple(
                        execution,
                        scheduling_policy,
                        soft_feedback_policy,
                        normal_relaxation_policy,
                        contact_coefficient_policy,
                        substep_contact_policy,
                        restitution_timing_policy,
                    )?;
                } else {
                    crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_runtime_tuple(
                        execution,
                        scheduling_policy,
                        soft_feedback_policy,
                        normal_relaxation_policy,
                        contact_coefficient_policy,
                        substep_contact_policy,
                        restitution_timing_policy,
                    )?;
                }
            } else {
                crate::authority_gpu_probe::validate_authority_gpu_probe_runtime_tuple(
                    execution,
                    scheduling_policy,
                    soft_feedback_policy,
                    normal_relaxation_policy,
                    contact_coefficient_policy,
                    substep_contact_policy,
                    restitution_timing_policy,
                )?;
            }
        } else if execution == GpuSolverExecutionV4::AUTHORITY_GPU_PROBE {
            return Err(
                "authority GPU execution requires the versioned midpoint identity path".to_owned(),
            );
        }
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
        let body_count = storage.layout().capacity_elements;
        if substep_contact_policy.is_retained_refresh_candidate()
            && (persistence_topology != GpuContactPersistenceTopology::StablePairSlot
                || gather_topology != GpuGatherTopology::StableBodyEdge
                || permute_edge_build
                || permute_pair_build
                || fingerprint_mask != u32::MAX
                || identity_policy != ContactIdentityPolicyV1::SELECTED)
        {
            return Err(
                "retained-impulse refresh requires the selected contact graph and identity policy"
                    .to_owned(),
            );
        }
        let candidate_capacity = product_candidate_capacity(body_count)?;
        let broadphase_pipelines = SapPipelines::new(device);
        let broadphase = SapCase::new_with_storage(
            device,
            &broadphase_pipelines,
            scenario,
            candidate_capacity,
            storage,
        )?;
        let narrowphase_pipelines = NarrowphasePipelines::new(device);
        let narrowphase = if authority_identity {
            NarrowphaseCase::new_with_candidate_source_and_authority_identity(
                device,
                &narrowphase_pipelines,
                &broadphase,
                Some(candidate_capacity),
                storage,
            )?
        } else {
            NarrowphaseCase::new_with_candidate_source_and_identity_policy(
                device,
                &narrowphase_pipelines,
                &broadphase,
                Some(candidate_capacity),
                storage,
                identity_policy,
            )?
        };
        let point_capacity = narrowphase.point_capacity();
        let solver_points = storage_buffer(
            device,
            "physbox5_generated_solver_points",
            point_capacity,
            size_of::<PointInputRaw>() as u32,
        )?;
        let mut awake_points = GpuAwakePoints::new(
            device,
            storage,
            &solver_points,
            narrowphase.manifold_capacity_buffer(),
            point_capacity,
        )?;
        let solver = GpuFixedFloorSolver::new_generated_experiment_with_topology_scheduling_and_extended_solver_policies(
                device,
                storage,
                scenario,
                awake_points.points(),
                awake_points.state_handle(),
                awake_points.solver_body_state_handle(),
                point_capacity,
                gather_topology,
                permute_edge_build,
                execution,
                scheduling_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )?;
        awake_points.bind_solver_prepared(device, solver.prepared_buffer());
        let persistence = GeneratedPersistenceGraph::new(
            device,
            persistence_topology,
            storage,
            identity_policy,
            &broadphase,
            &narrowphase,
            &solver_points,
            awake_points.persistence_prepared(),
            point_capacity,
            permute_pair_build,
            fingerprint_mask,
        )?;
        let sleep_contacts = GpuSleepContacts::new(
            device,
            storage,
            &solver_points,
            narrowphase.point_capacity_handle(),
        );
        Ok(Self {
            broadphase_pipelines,
            broadphase,
            narrowphase_pipelines,
            narrowphase,
            persistence,
            _solver_points: solver_points,
            solver,
            parity: Cell::new(0),
            point_capacity,
            identity_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            authority_identity,
            awake_work: storage.awake_work_handle(),
            sleep_contacts,
            awake_points,
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
        self.awake_work.encode_begin_step(encoder);
        self.sleep_contacts.encode_erased_neighbor_roots(encoder);
        self.broadphase.encode_with_beginning_timestamp(
            encoder,
            &self.broadphase_pipelines,
            timestamps,
        );
        self.narrowphase
            .encode(encoder, &self.narrowphase_pipelines);
        self.persistence.encode_before_solver(
            encoder,
            parity,
            invalidate_previous_cache,
            self.point_capacity,
            timestamps,
        );
        let wake_frontier = self.sleep_contacts.encode_contact_wakes(encoder);
        self.awake_work
            .encode_finalize_wakes(encoder, wake_frontier);
        self.awake_work.encode_compaction(encoder);
        self.awake_points.encode_compaction(encoder);
        self.awake_points.encode_solver_body_compaction(encoder);
        self.solver.encode_step(encoder);
        self.awake_points.encode_scatter(encoder);
        self.awake_work.encode_sleep_admission(encoder);
        self.persistence
            .encode_after_solver(encoder, parity, self.point_capacity, timestamps);
        self.parity.set(1 - parity);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_with_pass_trace(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        destination: &wgpu::Buffer,
    ) -> Result<Vec<GpuSolverPassDescriptorV1>, String> {
        let parity = self.parity.get();
        self.awake_work.encode_begin_step(encoder);
        self.sleep_contacts.encode_erased_neighbor_roots(encoder);
        self.broadphase
            .encode(encoder, &self.broadphase_pipelines, None);
        self.narrowphase
            .encode(encoder, &self.narrowphase_pipelines);
        self.persistence
            .encode_before_solver(encoder, parity, false, self.point_capacity, None);
        let wake_frontier = self.sleep_contacts.encode_contact_wakes(encoder);
        self.awake_work
            .encode_finalize_wakes(encoder, wake_frontier);
        self.awake_work.encode_compaction(encoder);
        self.awake_points.encode_compaction(encoder);
        self.awake_points.encode_solver_body_compaction(encoder);
        let descriptors = self
            .solver
            .encode_step_with_pass_trace(encoder, storage, destination)?;
        self.awake_points.encode_scatter(encoder);
        self.awake_work.encode_sleep_admission(encoder);
        self.persistence
            .encode_after_solver(encoder, parity, self.point_capacity, None);
        self.parity.set(1 - parity);
        Ok(descriptors)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_with_pass_motion_trace(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        destination: &wgpu::Buffer,
        destination_offset: u64,
    ) -> Result<Vec<GpuSolverPassDescriptorV1>, String> {
        let parity = self.parity.get();
        self.awake_work.encode_begin_step(encoder);
        self.sleep_contacts.encode_erased_neighbor_roots(encoder);
        self.broadphase
            .encode(encoder, &self.broadphase_pipelines, None);
        self.narrowphase
            .encode(encoder, &self.narrowphase_pipelines);
        self.persistence
            .encode_before_solver(encoder, parity, false, self.point_capacity, None);
        let wake_frontier = self.sleep_contacts.encode_contact_wakes(encoder);
        self.awake_work
            .encode_finalize_wakes(encoder, wake_frontier);
        self.awake_work.encode_compaction(encoder);
        self.awake_points.encode_compaction(encoder);
        self.awake_points.encode_solver_body_compaction(encoder);
        let descriptors = self.solver.encode_step_with_pass_motion_trace(
            encoder,
            storage,
            destination,
            destination_offset,
        )?;
        self.awake_points.encode_scatter(encoder);
        self.awake_work.encode_sleep_admission(encoder);
        self.persistence
            .encode_after_solver(encoder, parity, self.point_capacity, None);
        self.parity.set(1 - parity);
        Ok(descriptors)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_solver_instrumented(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        invalidate_previous_cache: bool,
        query_sets: &[&wgpu::QuerySet],
        color_occupancy: &[u32],
    ) -> Result<(), String> {
        let parity = self.parity.get();
        self.awake_work.encode_begin_step(encoder);
        self.sleep_contacts.encode_erased_neighbor_roots(encoder);
        self.broadphase
            .encode(encoder, &self.broadphase_pipelines, None);
        self.narrowphase
            .encode(encoder, &self.narrowphase_pipelines);
        self.persistence.encode_before_solver(
            encoder,
            parity,
            invalidate_previous_cache,
            self.point_capacity,
            None,
        );
        let wake_frontier = self.sleep_contacts.encode_contact_wakes(encoder);
        self.awake_work
            .encode_finalize_wakes(encoder, wake_frontier);
        self.awake_work.encode_compaction(encoder);
        self.awake_points.encode_compaction(encoder);
        self.awake_points.encode_solver_body_compaction(encoder);
        self.solver
            .encode_step_solver_phase_instrumented(encoder, query_sets, color_occupancy)?;
        self.awake_points.encode_scatter(encoder);
        self.awake_work.encode_sleep_admission(encoder);
        self.persistence
            .encode_after_solver(encoder, parity, self.point_capacity, None);
        self.parity.set(1 - parity);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn solver_instrumented_query_counts(
        &self,
        color_occupancy: &[u32],
    ) -> Result<Vec<u32>, String> {
        self.solver
            .solver_phase_instrumented_query_counts(color_occupancy)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn decode_solver_instrumented_sample(
        &self,
        values: &[u64],
        timestamp_period_nanoseconds: f32,
        color_occupancy: &[u32],
    ) -> Result<GpuSolverInstrumentedSampleV4, String> {
        self.solver.decode_instrumented_sample(
            values,
            timestamp_period_nanoseconds,
            color_occupancy,
        )
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        10 + 2 * self.awake_work.wake_propagation_rounds()
            + self.broadphase.dispatches()
            + self.persistence.dispatches()
            + BRIDGE_DISPATCHES
            + self.solver.dispatches_per_step()
    }

    pub(crate) const fn point_capacity(&self) -> u32 {
        self.point_capacity
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_trace_state_bytes(&self) -> u64 {
        self.solver.pass_trace_state_bytes()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_trace_motion_bytes(&self) -> u64 {
        self.solver.pass_trace_motion_bytes()
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.broadphase.allocated_bytes()
            + self.narrowphase.allocated_bytes()
            + self.persistence.allocated_bytes()
            + self._solver_points.size()
            + self.awake_points.capacity_record().allocated_bytes
            + self
                .awake_points
                .solver_body_capacity_record()
                .allocated_bytes
            + self.solver.allocated_bytes()
    }

    pub(crate) fn awake_point_capacity_record(&self) -> CapacityRecordV1 {
        self.awake_points.capacity_record()
    }

    pub(crate) fn solver_body_capacity_record(&self) -> CapacityRecordV1 {
        self.awake_points.solver_body_capacity_record()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_awake_point_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        self.awake_points.copy_state(encoder, destination);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_awake_point_mapping(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        self.awake_points.copy_mapping(encoder, destination);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_solver_body_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        self.awake_points
            .copy_solver_body_state(encoder, destination);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn collision_persistence_allocated_bytes(&self) -> u64 {
        self.broadphase.allocated_bytes()
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn color_status_source(&self) -> Option<(&wgpu::Buffer, u64)> {
        self.solver.color_status_source()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_construction_graph_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        let stride = u64::from(COLLISION_CAPACITY_COUNTER_BYTES);
        let current = self.parity.get();
        for (index, source) in [
            self.broadphase.candidate_capacity_buffer(),
            self.narrowphase.manifold_capacity_buffer(),
            self.narrowphase.point_capacity_buffer(),
            self.persistence.current_capacity(current),
        ]
        .into_iter()
        .enumerate()
        {
            encoder.copy_buffer_to_buffer(source, 0, destination, index as u64 * stride, stride);
        }
    }
}

fn product_candidate_capacity(body_count: u32) -> Result<u32, String> {
    let pair_count = body_count
        .checked_mul(body_count.saturating_sub(1))
        .map(|value| value / 2)
        .ok_or_else(|| "generated contact pair count overflowed".to_owned())?;
    Ok(pair_count
        .min(body_count.saturating_mul(PRODUCT_CANDIDATE_PAIRS_PER_BODY))
        .max(1))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_construction_graph_evidence(
    bytes: &[u8],
) -> Result<GpuConstructionGraphEvidence, String> {
    let expected = COLLISION_CAPACITY_COUNTER_BYTES as usize * 4;
    if bytes.len() != expected {
        return Err(format!(
            "construction graph evidence requires {expected} bytes, received {}",
            bytes.len()
        ));
    }
    let records = bytemuck::try_cast_slice::<u8, [u32; 4]>(bytes)
        .map_err(|error| format!("decode construction graph evidence: {error}"))?;
    Ok(GpuConstructionGraphEvidence {
        candidate_pairs: records[0][2],
        manifolds: records[1][2],
        contact_points: records[2][2],
        persistent_pairs: records[3][2],
        overflowed: records.iter().any(|record| record[3] != 0),
    })
}

impl GeneratedPersistenceGraph {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        topology: GpuContactPersistenceTopology,
        body_storage: &GpuBodyStorage,
        identity_policy: ContactIdentityPolicyV1,
        candidate: &dyn CandidateSource,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        prepared_points: &wgpu::Buffer,
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
                    prepared_points,
                    point_capacity,
                )?)))
            }
            GpuContactPersistenceTopology::StablePairSlot => {
                Ok(Self::PairSlot(Box::new(PairSlotPersistenceGraph::new(
                    device,
                    body_storage,
                    identity_policy,
                    candidate,
                    narrowphase,
                    solver_points,
                    prepared_points,
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
                if let Some(stage) = &graph.temporal_identity {
                    stage.encode(encoder, parity);
                }
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

    fn dispatches(&self) -> u32 {
        match self {
            Self::FullKey(_) => COLLISION_DISPATCHES,
            Self::PairSlot(graph) => {
                PAIR_SLOT_COLLISION_DISPATCHES + u32::from(graph.temporal_identity.is_some())
            }
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
        candidate: &dyn CandidateSource,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        prepared_points: &wgpu::Buffer,
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
                        (6, prepared_points),
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
        body_storage: &GpuBodyStorage,
        identity_policy: ContactIdentityPolicyV1,
        candidate: &dyn CandidateSource,
        narrowphase: &NarrowphaseCase,
        solver_points: &wgpu::Buffer,
        prepared_points: &wgpu::Buffer,
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
        let allocator = PairSlotAllocator::new(device, pair_capacity, 0)?;
        let temporal_identity =
            (identity_policy == ContactIdentityPolicyV1::PriorContactHysteresis).then(|| {
                TemporalIdentityStage::new(
                    device,
                    body_storage,
                    narrowphase,
                    [&pairs[0], &pairs[1]],
                    [&capacities[0], &capacities[1]],
                )
            });
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
                &allocator,
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
                &allocator,
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
                        (6, prepared_points),
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
            allocator,
            _bridge_params: bridge_params,
            bridge_pipeline_build,
            bridge_pipeline_store,
            bridge,
            pair_capacity,
            temporal_identity,
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
            self.allocator.reset(encoder);
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
            + self.allocator.allocated_bytes()
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
    pub solver_settings: physbox_contracts::SolverSettingsV1,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    pub substep_contact_policy: crate::GpuSubstepContactPolicyV1,
    pub restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    pub contact_identity_policy: ContactIdentityPolicyV1,
    pub awake_dynamic_body_count: u32,
    pub solver_body_count: u32,
    pub live_contact_point_count: u32,
    pub awake_contact_point_count: u32,
    pub bodies: Vec<physbox_contracts::BodyStateV1>,
    pub manifolds: Vec<physbox_contracts::CollisionManifoldV1>,
    pub cache: Vec<PersistentContactV1>,
    pub pass_evidence: crate::GpuFixedFloorPassEvidence,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub persistence_topology: GpuContactPersistenceTopology,
    pub gather_topology: GpuGatherTopologyEvidence,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactSnapshotV1 {
    pub version: u32,
    pub completed_steps: u32,
    pub bodies: Vec<physbox_contracts::BodyStateV1>,
    pub manifolds: Vec<physbox_contracts::CollisionManifoldV1>,
    pub cache: Vec<PersistentContactV1>,
    pub work: CollisionWorkV1,
    pub capacities: Vec<CollisionCapacityV1>,
    pub failure: Option<FailureV1>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuPassTraceDispatchV1 {
    DirectOverCompactedColorCapacity,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactPassStateV3 {
    pub version: u32,
    pub after_operation_index: Option<u32>,
    pub bodies: Vec<physbox_contracts::BodyStateV1>,
    pub contacts: Vec<GpuFixedFloorContactEvidence>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactPassTraceV3 {
    pub version: u32,
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub solver_settings: physbox_contracts::SolverSettingsV1,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub contact_identity_policy: ContactIdentityPolicyV1,
    pub soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    pub normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    pub contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    pub substep_contact_policy: crate::GpuSubstepContactPolicyV1,
    pub restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    pub pass_trace_dispatch: GpuPassTraceDispatchV1,
    pub mapped_after_final_step_only: bool,
    pub state_stride_bytes: u64,
    pub descriptors: Vec<GpuSolverPassDescriptorV1>,
    pub states: Vec<GpuGeneratedContactPassStateV3>,
    pub coloring: crate::GpuColoringEvidenceV4,
    pub construction: GpuConstructionGraphEvidence,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactPassMotionStepV1 {
    pub completed_step: u32,
    pub states: Vec<Vec<[f32; 4]>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactPassMotionWindowV1 {
    pub version: u32,
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub first_step: u32,
    pub last_step: u32,
    pub solver_settings: physbox_contracts::SolverSettingsV1,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub contact_identity_policy: ContactIdentityPolicyV1,
    pub soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    pub normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    pub contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    pub substep_contact_policy: crate::GpuSubstepContactPolicyV1,
    pub restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    pub pass_trace_dispatch: GpuPassTraceDispatchV1,
    pub mapped_after_final_step_only: bool,
    pub motion_state_stride_bytes: u64,
    pub descriptors: Vec<GpuSolverPassDescriptorV1>,
    pub steps: Vec<GpuGeneratedContactPassMotionStepV1>,
    pub coloring: crate::GpuColoringEvidenceV4,
    pub construction: GpuConstructionGraphEvidence,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactPassTraceV3 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != GPU_GENERATED_CONTACT_PASS_TRACE_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.pass_trace_dispatch != GpuPassTraceDispatchV1::DirectOverCompactedColorCapacity
            || !self.mapped_after_final_step_only
            || self.state_stride_bytes == 0
            || self.descriptors.is_empty()
            || self.states.len() != self.descriptors.len() + 1
            || self.construction.overflowed
            || self.coloring.overflowed
        {
            return Err("generated pass trace has invalid identity/completion/capacity".to_owned());
        }
        self.solver_settings
            .validate()
            .map_err(|error| error.to_string())?;
        self.solver_execution.validate()?;
        self.solver_scheduling_policy
            .validate(self.solver_execution)?;
        self.contact_coefficient_policy
            .validate(&self.solver_settings)?;
        self.substep_contact_policy.validate_composition(
            &self.solver_settings,
            self.solver_execution,
            self.solver_scheduling_policy,
            self.soft_feedback_policy,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.restitution_timing_policy,
        )?;
        if self.substep_contact_policy.is_retained_refresh_candidate()
            && self.contact_identity_policy != ContactIdentityPolicyV1::SELECTED
        {
            return Err("retained-refresh pass trace changed contact identity".to_owned());
        }
        self.restitution_timing_policy.validate(
            &self.solver_settings,
            self.solver_execution,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.substep_contact_policy,
        )?;
        let normal_relaxation = self
            .descriptors
            .iter()
            .filter(|descriptor| descriptor.kind == crate::GpuSolverPassKindV1::NormalRelaxation)
            .collect::<Vec<_>>();
        let expected_normal_relaxation = if self.solver_execution.relaxation
            && self.normal_relaxation_policy
                != crate::GpuNormalRelaxationPolicyV1::OmitForSoftContacts
        {
            let iterations =
                if self.solver_execution == crate::GpuSolverExecutionV4::AUTHORITY_GPU_PROBE {
                    1
                } else {
                    self.solver_settings.iterations
                };
            (self.solver_settings.substeps * iterations) as usize
        } else {
            0
        };
        let expected_shader_code = match self.normal_relaxation_policy {
            crate::GpuNormalRelaxationPolicyV1::HardDirect
            | crate::GpuNormalRelaxationPolicyV1::OmitForSoftContacts => 0.0,
            crate::GpuNormalRelaxationPolicyV1::SoftCoefficients => 1.0,
            crate::GpuNormalRelaxationPolicyV1::PointSequential => 2.0,
            crate::GpuNormalRelaxationPolicyV1::CoupledEnergyProjection => 3.0,
        };
        let authority_island_relaxation = self.solver_execution
            == crate::GpuSolverExecutionV4::AUTHORITY_GPU_PROBE
            && self
                .solver_scheduling_policy
                .colored_dispatch
                .uses_workgroup_per_island();
        if normal_relaxation.len() != expected_normal_relaxation
            || (!authority_island_relaxation
                && normal_relaxation
                    .iter()
                    .any(|descriptor| descriptor.parameters.floats2[3] != expected_shader_code))
        {
            return Err("generated pass trace relaxation policy disagrees with passes".to_owned());
        }
        if self.states[0].after_operation_index.is_some() {
            return Err("generated pass trace initial state names an operation".to_owned());
        }
        for (index, descriptor) in self.descriptors.iter().enumerate() {
            if descriptor.version != crate::GPU_SOLVER_PASS_TRACE_VERSION
                || self.states[index + 1].after_operation_index != Some(descriptor.operation_index)
            {
                return Err(
                    "generated pass trace operation/state sequence is inconsistent".to_owned(),
                );
            }
        }
        for state in &self.states {
            if state.version != GPU_GENERATED_CONTACT_PASS_TRACE_VERSION {
                return Err("generated pass trace state has an invalid version".to_owned());
            }
            for body in &state.bodies {
                body.validate().map_err(|error| error.to_string())?;
            }
            if state.contacts.iter().any(|contact| {
                contact.valid
                    && (contact.body_a as usize >= state.bodies.len()
                        || contact.body_b as usize >= state.bodies.len())
            }) {
                return Err("generated pass trace contact references an invalid body".to_owned());
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactPassMotionWindowV1 {
    pub fn validate(&self) -> Result<(), String> {
        let expected_steps = self
            .last_step
            .checked_sub(self.first_step)
            .and_then(|difference| difference.checked_add(1))
            .ok_or_else(|| "generated pass-motion window has an invalid step range".to_owned())?;
        let body_count = self
            .steps
            .first()
            .and_then(|step| step.states.first())
            .map(Vec::len)
            .ok_or_else(|| "generated pass-motion window has no body state".to_owned())?;
        if self.version != GPU_GENERATED_CONTACT_PASS_MOTION_WINDOW_VERSION
            || self.source_scenario_hash.len() != 64
            || self.first_step == 0
            || self.pass_trace_dispatch != GpuPassTraceDispatchV1::DirectOverCompactedColorCapacity
            || !self.mapped_after_final_step_only
            || self.motion_state_stride_bytes != (body_count * size_of::<[f32; 4]>()) as u64
            || self.descriptors.is_empty()
            || self.steps.len() != expected_steps as usize
            || self.construction.overflowed
            || self.coloring.overflowed
        {
            return Err(
                "generated pass-motion window has invalid identity/completion/capacity".to_owned(),
            );
        }
        self.solver_settings
            .validate()
            .map_err(|error| error.to_string())?;
        self.solver_execution.validate()?;
        self.solver_scheduling_policy
            .validate(self.solver_execution)?;
        self.contact_coefficient_policy
            .validate(&self.solver_settings)?;
        self.substep_contact_policy.validate_composition(
            &self.solver_settings,
            self.solver_execution,
            self.solver_scheduling_policy,
            self.soft_feedback_policy,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.restitution_timing_policy,
        )?;
        self.restitution_timing_policy.validate(
            &self.solver_settings,
            self.solver_execution,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.substep_contact_policy,
        )?;
        for (index, descriptor) in self.descriptors.iter().enumerate() {
            if descriptor.version != crate::GPU_SOLVER_PASS_TRACE_VERSION
                || descriptor.operation_index < descriptor.first_operation_index
                || (index > 0
                    && descriptor.first_operation_index
                        != self.descriptors[index - 1].operation_index + 1)
            {
                return Err(
                    "generated pass-motion window descriptor sequence is invalid".to_owned(),
                );
            }
        }
        for (index, step) in self.steps.iter().enumerate() {
            if step.completed_step != self.first_step + index as u32
                || step.states.len() != self.descriptors.len() + 1
                || step.states.iter().any(|state| {
                    state.len() != body_count
                        || state
                            .iter()
                            .flat_map(|motion| motion.iter())
                            .any(|value| !value.is_finite())
                })
            {
                return Err("generated pass-motion window state sequence is invalid".to_owned());
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactSnapshotV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.completed_steps == 0 {
            return Err("generated settling snapshot has an invalid version/step".to_owned());
        }
        for body in &self.bodies {
            body.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.manifolds.windows(2) {
            if pair[0].pair >= pair[1].pair {
                return Err("generated settling manifolds are not sorted and unique".to_owned());
            }
        }
        for manifold in &self.manifolds {
            manifold.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.cache.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err("generated settling cache is not sorted and unique".to_owned());
            }
        }
        for value in &self.cache {
            value.validate().map_err(|error| error.to_string())?;
        }
        self.work.validate().map_err(|error| error.to_string())?;
        for capacity in &self.capacities {
            capacity.validate().map_err(|error| error.to_string())?;
        }
        if let Some(failure) = &self.failure {
            failure.validate().map_err(|error| error.to_string())?;
        }
        let points = self
            .manifolds
            .iter()
            .map(|manifold| manifold.points.len() as u64)
            .sum::<u64>();
        if (self.failure.is_some() && (!self.manifolds.is_empty() || !self.cache.is_empty()))
            || (self.failure.is_none()
                && (self.manifolds.len() as u64 != self.work.manifolds
                    || points != self.work.contact_points))
        {
            return Err("generated settling snapshot counts are inconsistent".to_owned());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGeneratedContactSeriesV1 {
    pub version: u32,
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub observation_interval_steps: u32,
    pub completed_steps: u32,
    pub solver_settings: physbox_contracts::SolverSettingsV1,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    pub substep_contact_policy: crate::GpuSubstepContactPolicyV1,
    pub restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    pub contact_identity_policy: ContactIdentityPolicyV1,
    pub soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    pub normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    pub authority_gpu_probe_policy: Option<crate::AuthorityGpuProbePolicyV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_robust_gpu_promotion_policy: Option<crate::BranchRobustGpuPromotionPolicyV1>,
    pub authority_identity: bool,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub snapshots: Vec<GpuGeneratedContactSnapshotV1>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactSeriesV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.source_scenario_hash.len() != 64
            || self.observation_interval_steps == 0
            || self.completed_steps == 0
            || self.snapshots.is_empty()
        {
            return Err("generated settling series has invalid identity/cadence".to_owned());
        }
        self.solver_settings
            .validate()
            .map_err(|error| error.to_string())?;
        self.solver_execution.validate()?;
        self.solver_scheduling_policy
            .validate(self.solver_execution)?;
        self.contact_coefficient_policy
            .validate(&self.solver_settings)?;
        self.substep_contact_policy.validate_composition(
            &self.solver_settings,
            self.solver_execution,
            self.solver_scheduling_policy,
            self.soft_feedback_policy,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.restitution_timing_policy,
        )?;
        if self.authority_gpu_probe_policy.is_some()
            && self.branch_robust_gpu_promotion_policy.is_some()
        {
            return Err("generated series names two authority GPU candidates".to_owned());
        }
        if let Some(policy) = self.authority_gpu_probe_policy {
            policy.validate_product_selection(false)?;
            crate::authority_gpu_probe::validate_authority_gpu_probe_runtime_tuple(
                self.solver_execution,
                self.solver_scheduling_policy,
                self.soft_feedback_policy,
                self.normal_relaxation_policy,
                self.contact_coefficient_policy,
                self.substep_contact_policy,
                self.restitution_timing_policy,
            )?;
            if !self.authority_identity {
                return Err("authority GPU series omitted authority identity".to_owned());
            }
        } else if let Some(policy) = self.branch_robust_gpu_promotion_policy {
            policy.validate_product_selection(false)?;
            crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_runtime_tuple(
                self.solver_execution,
                self.solver_scheduling_policy,
                self.soft_feedback_policy,
                self.normal_relaxation_policy,
                self.contact_coefficient_policy,
                self.substep_contact_policy,
                self.restitution_timing_policy,
            )?;
            if !self.authority_identity {
                return Err("branch-robust GPU series omitted authority identity".to_owned());
            }
        } else if self.contact_coefficient_policy
            == crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft
        {
            crate::branch_robust_gpu_promotion::validate_branch_robust_tunable_runtime_tuple(
                self.solver_execution,
                self.solver_scheduling_policy,
                self.soft_feedback_policy,
                self.normal_relaxation_policy,
                self.contact_coefficient_policy,
                self.substep_contact_policy,
                self.restitution_timing_policy,
            )?;
            if !self.authority_identity {
                return Err("branch-robust tunable series omitted authority identity".to_owned());
            }
        } else if self.authority_identity
            || self.solver_execution == GpuSolverExecutionV4::AUTHORITY_GPU_PROBE
        {
            return Err("generated series has an unowned authority GPU execution".to_owned());
        }
        if self.substep_contact_policy.is_retained_refresh_candidate()
            && self.contact_identity_policy != ContactIdentityPolicyV1::SELECTED
        {
            return Err("retained-refresh series changed contact identity".to_owned());
        }
        self.restitution_timing_policy.validate(
            &self.solver_settings,
            self.solver_execution,
            self.normal_relaxation_policy,
            self.contact_coefficient_policy,
            self.substep_contact_policy,
        )?;
        for (index, snapshot) in self.snapshots.iter().enumerate() {
            snapshot.validate()?;
            if snapshot.completed_steps != (index as u32 + 1) * self.observation_interval_steps {
                return Err("generated settling snapshots violate declared cadence".to_owned());
            }
        }
        if self.snapshots.last().unwrap().completed_steps != self.completed_steps {
            return Err("generated settling series omitted its final step".to_owned());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuGeneratedContactOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.completed_steps == 0 || !self.mapped_after_final_step_only {
            return Err("generated contact output has an invalid completion phase".to_owned());
        }
        self.solver_settings
            .validate()
            .map_err(|error| error.to_string())?;
        self.solver_execution.validate()?;
        self.solver_scheduling_policy
            .validate(self.solver_execution)?;
        self.contact_coefficient_policy
            .validate(&self.solver_settings)?;
        self.substep_contact_policy.validate_composition(
            &self.solver_settings,
            self.solver_execution,
            self.solver_scheduling_policy,
            crate::GpuSoftFeedbackPolicyV1::SELECTED,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
            self.contact_coefficient_policy,
            self.restitution_timing_policy,
        )?;
        if self.substep_contact_policy.is_retained_refresh_candidate()
            && self.contact_identity_policy != ContactIdentityPolicyV1::SELECTED
        {
            return Err("retained-refresh output changed contact identity".to_owned());
        }
        if self.awake_dynamic_body_count > self.solver_body_count
            || self.solver_body_count > self.bodies.len() as u32
            || self.awake_contact_point_count > self.live_contact_point_count
        {
            return Err("generated awake-work counts exceed their live bounds".to_owned());
        }
        self.restitution_timing_policy.validate(
            &self.solver_settings,
            self.solver_execution,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
            self.contact_coefficient_policy,
            self.substep_contact_policy,
        )?;
        for body in &self.bodies {
            body.validate().map_err(|error| error.to_string())?;
        }
        for pair in self.manifolds.windows(2) {
            if pair[0].pair >= pair[1].pair {
                return Err("generated contact manifolds are not sorted and unique".to_owned());
            }
        }
        for manifold in &self.manifolds {
            manifold.validate().map_err(|error| error.to_string())?;
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
        let body_count = self.bodies.len() as u32;
        if self.pass_evidence.contacts.iter().any(|contact| {
            !contact.valid
                || contact.body_a >= body_count
                || contact.body_b >= body_count
                || contact.body_a_normal_mass_incidence == 0
                || contact.body_b_normal_mass_incidence == 0
                || [
                    contact.normal.x,
                    contact.normal.y,
                    contact.tangent.x,
                    contact.tangent.y,
                    contact.anchor_a.x,
                    contact.anchor_a.y,
                    contact.anchor_b.x,
                    contact.anchor_b.y,
                    contact.initial_normal_velocity,
                    contact.normal_effective_mass,
                    contact.tangent_effective_mass,
                    contact.separation,
                    contact.friction,
                    contact.restitution,
                    contact.normal_impulse,
                    contact.tangent_impulse,
                    contact.total_normal_impulse,
                ]
                .into_iter()
                .any(|value| !value.is_finite())
        }) {
            return Err("generated contact pass evidence is invalid".to_owned());
        }
        if self.pass_evidence.contributions.iter().any(|contribution| {
            contribution.body_index >= body_count
                || !contribution.impulse.x.is_finite()
                || !contribution.impulse.y.is_finite()
                || !contribution.anchor.x.is_finite()
                || !contribution.anchor.y.is_finite()
        }) {
            return Err("generated contribution evidence is invalid".to_owned());
        }
        if self.pass_evidence.manifold_blocks.iter().any(|block| {
            block.body_a >= body_count
                || block.body_b >= body_count
                || !(1..=2).contains(&block.point_count)
                || block.body_a_normal_mass_incidence == 0
                || block.body_b_normal_mass_incidence == 0
                || block.active_set == crate::GpuBlockActiveSetV2::Invalid
                || block.effective_mass_matrix[0] <= 0.0
                || (block.point_count == 2 && block.effective_mass_matrix[3] <= 0.0)
                || block
                    .effective_mass_matrix
                    .into_iter()
                    .chain(block.normal_impulses)
                    .any(|value| !value.is_finite())
                || block
                    .normal_impulses
                    .into_iter()
                    .any(|impulse| impulse < 0.0)
        }) {
            return Err("generated manifold-block evidence is invalid".to_owned());
        }
        let traced_block_points = self
            .pass_evidence
            .manifold_blocks
            .iter()
            .map(|block| u64::from(block.point_count))
            .sum::<u64>();
        let coloring_rejected = self
            .pass_evidence
            .coloring
            .as_ref()
            .is_some_and(|coloring| coloring.overflowed);
        let expected_block_points =
            if self.solver_execution.normal_solver.uses_block() && !coloring_rejected {
                self.pass_evidence.contacts.len() as u64
            } else {
                0
            };
        if traced_block_points != expected_block_points {
            return Err(format!(
                "generated manifold-block evidence does not match execution: traced {traced_block_points}, expected {expected_block_points}"
            ));
        }
        match (
            self.solver_execution.accumulation,
            &self.pass_evidence.coloring,
        ) {
            (crate::GpuSolverAccumulationV4::JacobiGather, None) => {}
            (crate::GpuSolverAccumulationV4::ColoredInPlace, Some(coloring)) => {
                if coloring.max_colors != 32
                    || coloring.schedule != self.solver_execution.color_schedule
                    || coloring.unit != self.solver_execution.color_unit
                    || coloring.order != self.solver_execution.color_order
                    || coloring.scheduling_policy != self.solver_scheduling_policy
                    || coloring.point_colors.len() != self.pass_evidence.contacts.len()
                    || coloring.color_count > coloring.max_colors
                    || coloring.color_occupancy.len() != coloring.color_count as usize
                    || coloring.encoded_color_passes
                        != if coloring
                            .scheduling_policy
                            .colored_dispatch
                            .uses_workgroup_per_island()
                        {
                            1
                        } else if coloring.scheduling_policy.uses_authority_source_order() {
                            crate::AUTHORITY_GPU_COLOR_COUNT
                        } else {
                            coloring.max_colors
                        }
                    || coloring.active_color_passes
                        != coloring
                            .color_occupancy
                            .iter()
                            .filter(|&&occupancy| occupancy != 0)
                            .count() as u32
                    || (!coloring.overflowed
                        && coloring.color_occupancy.iter().sum::<u32>()
                            != match coloring.unit {
                                crate::GpuColorUnitV4::Point => coloring.point_colors.len() as u32,
                                crate::GpuColorUnitV4::Manifold => {
                                    self.pass_evidence
                                        .contacts
                                        .iter()
                                        .filter(|contact| contact.point_in_manifold == 0)
                                        .count() as u32
                                }
                            })
                    || (!coloring.overflowed
                        && coloring
                            .point_colors
                            .iter()
                            .any(|&color| color >= coloring.color_count))
                {
                    return Err("generated coloring evidence is invalid".to_owned());
                }
                let colored_units = coloring.color_occupancy.iter().sum::<u32>();
                if !coloring.overflowed
                    && coloring
                        .retained_color_units
                        .checked_add(coloring.new_color_units)
                        .and_then(|value| value.checked_add(coloring.recolored_color_units))
                        != Some(colored_units)
                {
                    return Err("generated color-maintenance evidence is inconsistent".to_owned());
                }
                match coloring.scheduling_policy.color_maintenance {
                    crate::GpuColorMaintenanceV1::FullRebuild
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelCompaction
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculative
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
                    | crate::GpuColorMaintenanceV1::AuthoritySourceOrder
                        if coloring.retained_color_units != 0
                            || coloring.new_color_units != colored_units
                            || coloring.recolored_color_units != 0
                            || coloring.removed_color_units != 0 =>
                    {
                        return Err("full-rebuild color maintenance is invalid".to_owned());
                    }
                    crate::GpuColorMaintenanceV1::StablePairIncremental => {}
                    crate::GpuColorMaintenanceV1::FullRebuild
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelCompaction
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculative
                    | crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
                    | crate::GpuColorMaintenanceV1::AuthoritySourceOrder => {}
                }
                let parallel = matches!(
                    coloring.scheduling_policy.color_maintenance,
                    crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculative
                        | crate::GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
                );
                if parallel
                    != (coloring.parallel_round_limit != 0
                        || coloring.parallel_rounds_used != 0
                        || coloring.parallel_conflict_retries != 0
                        || coloring.parallel_uncolored_units != 0)
                    || coloring.parallel_rounds_used > coloring.parallel_round_limit
                    || (!coloring.overflowed && coloring.parallel_uncolored_units != 0)
                {
                    return Err("generated parallel-coloring evidence is inconsistent".to_owned());
                }
                match coloring.schedule {
                    crate::GpuColorScheduleV4::CompactIndirect => {
                        let expected_indirect = if coloring
                            .scheduling_policy
                            .colored_dispatch
                            .uses_workgroup_per_island()
                        {
                            1
                        } else {
                            coloring.max_colors as usize
                        };
                        if coloring.indirect_workgroups.len() != expected_indirect
                            || coloring.empty_indirect_launches
                                != coloring
                                    .indirect_workgroups
                                    .iter()
                                    .filter(|arguments| arguments[0] == 0)
                                    .count() as u32
                            || coloring.scheduled_point_invocations
                                != coloring
                                    .indirect_workgroups
                                    .iter()
                                    .map(|arguments| u64::from(arguments[0]) * 64)
                                    .sum::<u64>()
                        {
                            return Err("generated compact coloring schedule is invalid".to_owned());
                        }
                        match coloring.scheduling_policy.colored_dispatch {
                            crate::GpuColoredDispatchV1::PerColorIndirect
                            | crate::GpuColoredDispatchV1::PerColorIndirectFusedNormalTangent
                                if coloring.island_count != 0
                                    || !coloring.island_body_occupancy.is_empty()
                                    || !coloring.island_manifold_occupancy.is_empty() =>
                            {
                                return Err(
                                    "per-color scheduling contains island evidence".to_owned()
                                );
                            }
                            crate::GpuColoredDispatchV1::WorkgroupPerIsland
                            | crate::GpuColoredDispatchV1::WorkgroupPerIslandFusedNormalTangent
                                if coloring.island_body_occupancy.len()
                                    != coloring.island_count as usize
                                    || coloring.island_manifold_occupancy.len()
                                        != coloring.island_count as usize
                                    || (!coloring.overflowed
                                        && coloring
                                            .island_manifold_occupancy
                                            .iter()
                                            .sum::<u32>()
                                            != colored_units) =>
                            {
                                return Err("workgroup-per-island evidence is invalid".to_owned());
                            }
                            _ => {}
                        }
                    }
                    crate::GpuColorScheduleV4::FixedFullGrid => {
                        if !coloring.compacted_point_indices.is_empty()
                            || !coloring.indirect_workgroups.is_empty()
                            || coloring.empty_indirect_launches != 0
                        {
                            return Err(
                                "generated fixed-grid coloring schedule is invalid".to_owned()
                            );
                        }
                    }
                    crate::GpuColorScheduleV4::None => {
                        return Err("colored execution lacks a color schedule".to_owned());
                    }
                }
                if !coloring.overflowed {
                    let mut colored_bodies = std::collections::BTreeSet::new();
                    for (index, (contact, &color)) in self
                        .pass_evidence
                        .contacts
                        .iter()
                        .zip(&coloring.point_colors)
                        .enumerate()
                    {
                        if coloring.unit == crate::GpuColorUnitV4::Manifold
                            && contact.point_in_manifold != 0
                        {
                            if index == 0 || coloring.point_colors[index - 1] != color {
                                return Err(
                                    "generated manifold points have different colors".to_owned()
                                );
                            }
                            continue;
                        }
                        if coloring.scheduling_policy.uses_static_late() {
                            let dynamic_a = self.bodies[contact.body_a as usize].inverse_mass.get()
                                > 0.0
                                || self.bodies[contact.body_a as usize].inverse_inertia.get() > 0.0;
                            let dynamic_b = self.bodies[contact.body_b as usize].inverse_mass.get()
                                > 0.0
                                || self.bodies[contact.body_b as usize].inverse_inertia.get() > 0.0;
                            let static_contact = !dynamic_a || !dynamic_b;
                            if (static_contact && color < crate::GPU_STATIC_LATE_FIRST_COLOR)
                                || (!static_contact && color >= crate::GPU_STATIC_LATE_FIRST_COLOR)
                            {
                                return Err(
                                    "generated static-late color partition is invalid".to_owned()
                                );
                            }
                        }
                        if !colored_bodies.insert((color, contact.body_a))
                            || !colored_bodies.insert((color, contact.body_b))
                        {
                            return Err(
                                "generated color contains contacts with a shared body".to_owned()
                            );
                        }
                    }
                }
            }
            _ => return Err("generated coloring evidence does not match execution".to_owned()),
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
        let manifold_points = self
            .manifolds
            .iter()
            .map(|manifold| manifold.points.len() as u64)
            .sum::<u64>();
        if (self.failure.is_some() && !self.manifolds.is_empty())
            || (self.failure.is_none()
                && (self.manifolds.len() as u64 != self.work.manifolds
                    || manifold_points != self.work.contact_points))
        {
            return Err("generated contact manifold counts are inconsistent".to_owned());
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
struct GeneratedSettlingReadbacks {
    snapshot_count: u32,
    lane_bytes: u64,
    state_step_bytes: u64,
    cache_step_bytes: u64,
    manifold_step_bytes: u64,
    point_step_bytes: u64,
    state: wgpu::Buffer,
    cache: wgpu::Buffer,
    candidate_capacity: wgpu::Buffer,
    manifold_capacity: wgpu::Buffer,
    point_capacity: wgpu::Buffer,
    persistence_capacity: wgpu::Buffer,
    manifold_records: wgpu::Buffer,
    point_records: wgpu::Buffer,
    work: wgpu::Buffer,
    step: wgpu::Buffer,
    color_status: Option<wgpu::Buffer>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GeneratedSettlingReadbacks {
    fn new(
        device: &wgpu::Device,
        graph: &GpuGeneratedContactGraph,
        body_count: u32,
        snapshot_count: u32,
    ) -> Self {
        let lane_bytes = u64::from(body_count) * size_of::<[u32; 4]>() as u64;
        let state_step_bytes = lane_bytes * 3;
        let cache_step_bytes = u64::from(graph.persistence.record_capacity(graph.point_capacity))
            * u64::from(graph.persistence.record_stride());
        let manifold_step_bytes =
            u64::from(graph.narrowphase.manifold_capacity()) * u64::from(COLLISION_MANIFOLD_BYTES);
        let point_step_bytes = u64::from(graph.narrowphase.point_capacity())
            * u64::from(COLLISION_CONTACT_POINT_BYTES);
        Self {
            snapshot_count,
            lane_bytes,
            state_step_bytes,
            cache_step_bytes,
            manifold_step_bytes,
            point_step_bytes,
            state: readback(
                device,
                "generated_settling_state",
                state_step_bytes * u64::from(snapshot_count),
            ),
            cache: readback(
                device,
                "generated_settling_cache",
                cache_step_bytes * u64::from(snapshot_count),
            ),
            candidate_capacity: readback(
                device,
                "generated_settling_candidate_capacity",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * u64::from(snapshot_count),
            ),
            manifold_capacity: readback(
                device,
                "generated_settling_manifold_capacity",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * u64::from(snapshot_count),
            ),
            point_capacity: readback(
                device,
                "generated_settling_point_capacity",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * u64::from(snapshot_count),
            ),
            persistence_capacity: readback(
                device,
                "generated_settling_persistence_capacity",
                u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * u64::from(snapshot_count),
            ),
            manifold_records: readback(
                device,
                "generated_settling_manifolds",
                manifold_step_bytes * u64::from(snapshot_count),
            ),
            point_records: readback(
                device,
                "generated_settling_points",
                point_step_bytes * u64::from(snapshot_count),
            ),
            work: readback(
                device,
                "generated_settling_work",
                u64::from(COLLISION_WORK_COUNTER_BYTES) * u64::from(snapshot_count),
            ),
            step: readback(
                device,
                "generated_settling_step",
                4 * u64::from(snapshot_count),
            ),
            color_status: graph.color_status_source().map(|_| {
                readback(
                    device,
                    "generated_settling_color_status",
                    4 * u64::from(snapshot_count),
                )
            }),
        }
    }

    fn encode_copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        graph: &GpuGeneratedContactGraph,
        snapshot_index: u32,
    ) {
        debug_assert!(snapshot_index < self.snapshot_count);
        let state_offset = u64::from(snapshot_index) * self.state_step_bytes;
        encoder.copy_buffer_to_buffer(
            storage.pose_buffer(),
            0,
            &self.state,
            state_offset,
            self.lane_bytes,
        );
        encoder.copy_buffer_to_buffer(
            storage.motion_buffer(),
            0,
            &self.state,
            state_offset + self.lane_bytes,
            self.lane_bytes,
        );
        encoder.copy_buffer_to_buffer(
            storage.metadata_buffer(),
            0,
            &self.state,
            state_offset + self.lane_bytes * 2,
            self.lane_bytes,
        );
        let current = graph.parity.get();
        encoder.copy_buffer_to_buffer(
            graph.persistence.current_records(current),
            0,
            &self.cache,
            u64::from(snapshot_index) * self.cache_step_bytes,
            self.cache_step_bytes,
        );
        let capacity_offset =
            u64::from(snapshot_index) * u64::from(COLLISION_CAPACITY_COUNTER_BYTES);
        encoder.copy_buffer_to_buffer(
            graph.broadphase.candidate_capacity_buffer(),
            0,
            &self.candidate_capacity,
            capacity_offset,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            graph.narrowphase.manifold_capacity_buffer(),
            0,
            &self.manifold_capacity,
            capacity_offset,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            graph.narrowphase.point_capacity_buffer(),
            0,
            &self.point_capacity,
            capacity_offset,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            graph.persistence.current_capacity(current),
            0,
            &self.persistence_capacity,
            capacity_offset,
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            graph.narrowphase.manifolds(),
            0,
            &self.manifold_records,
            u64::from(snapshot_index) * self.manifold_step_bytes,
            self.manifold_step_bytes,
        );
        encoder.copy_buffer_to_buffer(
            graph.narrowphase.points(),
            0,
            &self.point_records,
            u64::from(snapshot_index) * self.point_step_bytes,
            self.point_step_bytes,
        );
        encoder.copy_buffer_to_buffer(
            graph.broadphase.work_buffer(),
            0,
            &self.work,
            u64::from(snapshot_index) * u64::from(COLLISION_WORK_COUNTER_BYTES),
            u64::from(COLLISION_WORK_COUNTER_BYTES),
        );
        let (step_source, step_offset) = graph.persistence.step_counter_source();
        encoder.copy_buffer_to_buffer(
            step_source,
            step_offset,
            &self.step,
            u64::from(snapshot_index) * 4,
            4,
        );
        if let (Some((source, offset)), Some(destination)) =
            (graph.color_status_source(), &self.color_status)
        {
            encoder.copy_buffer_to_buffer(
                source,
                offset,
                destination,
                u64::from(snapshot_index) * 4,
                4,
            );
        }
    }

    fn destroy(self) {
        self.state.destroy();
        self.cache.destroy();
        self.candidate_capacity.destroy();
        self.manifold_capacity.destroy();
        self.point_capacity.destroy();
        self.persistence_capacity.destroy();
        self.manifold_records.destroy();
        self.point_records.destroy();
        self.work.destroy();
        self.step.destroy();
        if let Some(color_status) = self.color_status {
            color_status.destroy();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct GpuGeneratedContactBodyHistoryV1 {
    pub version: u32,
    pub bodies: Vec<Vec<physbox_contracts::BodyStateV1>>,
    pub failures: Vec<FailureV1>,
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

    /// Evaluates the S13 analytical-separation expression on retained binary32
    /// inputs with explicit intermediate materialization. This validation-only
    /// microscope cannot affect a solver step; the pass trace remains the
    /// ground truth when a backend contracts the production expression.
    pub fn run_authority_separation_diagnostic(
        &self,
        input: GpuAuthoritySeparationInputV1,
    ) -> Result<GpuAuthoritySeparationDiagnosticV1, String> {
        let mut input_words = [0_u32; 64];
        for (index, value) in [
            input.normal.x,
            input.normal.y,
            input.anchor_a.x,
            input.anchor_a.y,
            input.anchor_b.x,
            input.anchor_b.y,
            input.pose_a.x,
            input.pose_a.y,
            input.pose_b.x,
            input.pose_b.y,
            input.initial_position_a.x,
            input.initial_position_a.y,
            input.initial_position_b.x,
            input.initial_position_b.y,
            input.angle_a,
            input.angle_b,
            input.initial_angle_a,
            input.initial_angle_b,
            input.input_separation,
        ]
        .into_iter()
        .enumerate()
        {
            input_words[index] = value.to_bits();
        }
        let source = storage_init(
            &self.device,
            "physbox5_authority_separation_diagnostic_input",
            &AuthoritySeparationWordsRaw {
                values: input_words,
            },
        );
        let output = storage_init(
            &self.device,
            "physbox5_authority_separation_diagnostic_output",
            &AuthoritySeparationWordsRaw { values: [0; 64] },
        );
        let readback = readback(
            &self.device,
            "physbox5_authority_separation_diagnostic_readback",
            size_of::<AuthoritySeparationWordsRaw>() as u64,
        );
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_pipeline_layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("authority_separation_diagnostic.wgsl").into(),
                ),
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("evaluate"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_authority_separation_diagnostic_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_authority_separation_diagnostic_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &output,
            0,
            &readback,
            0,
            size_of::<AuthoritySeparationWordsRaw>() as u64,
        );
        self.queue.submit([encoder.finish()]);
        let words = map_range::<AuthoritySeparationWordsRaw>(&self.device, &readback, 0, 1)?
            .pop()
            .ok_or_else(|| "authority separation diagnostic returned no result".to_owned())?;
        const NAMES: [&str; 38] = [
            "delta-position-a-x",
            "delta-position-a-y",
            "delta-position-b-x",
            "delta-position-b-y",
            "delta-position-x",
            "delta-position-y",
            "delta-angle-a",
            "delta-angle-b",
            "cos-delta-angle-a",
            "sin-delta-angle-a",
            "rotated-anchor-a-x",
            "rotated-anchor-a-y",
            "cos-delta-angle-b",
            "sin-delta-angle-b",
            "rotated-anchor-b-x",
            "rotated-anchor-b-y",
            "prepared-anchor-delta-x",
            "prepared-anchor-delta-y",
            "prepared-anchor-projection",
            "base-separation",
            "rotated-anchor-delta-x",
            "rotated-anchor-delta-y",
            "original-motion-x",
            "original-motion-y",
            "original-projection",
            "original-separation",
            "rotation-displacement-a-x",
            "rotation-displacement-a-y",
            "rotation-displacement-b-x",
            "rotation-displacement-b-y",
            "relative-rotation-displacement-x",
            "relative-rotation-displacement-y",
            "robust-motion-x",
            "robust-motion-y",
            "robust-projection",
            "robust-separation",
            "native-original-separation",
            "native-robust-separation",
        ];
        Ok(GpuAuthoritySeparationDiagnosticV1 {
            version: GPU_AUTHORITY_SEPARATION_DIAGNOSTIC_VERSION,
            adapter: self.diagnostics.clone(),
            input,
            values: NAMES
                .into_iter()
                .enumerate()
                .map(|(index, name)| GpuAuthoritySeparationValueV1 {
                    name,
                    value: f32::from_bits(words.values[index]),
                    bits: words.values[index],
                })
                .collect(),
        })
    }

    /// Captures every state-changing solver operation for one exact outer
    /// step. Earlier steps retain one-submission interactive cadence; all
    /// snapshots map only after the target step and its persistence store.
    pub fn run_contact_policy_pass_trace(
        &self,
        scenario: &ScenarioV1,
        target_step: u32,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    ) -> Result<GpuGeneratedContactPassTraceV3, String> {
        self.run_contact_policy_pass_trace_with_relaxation(
            scenario,
            target_step,
            identity_policy,
            soft_feedback_policy,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
        )
    }

    pub fn run_contact_policy_pass_trace_with_relaxation(
        &self,
        scenario: &ScenarioV1,
        target_step: u32,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    ) -> Result<GpuGeneratedContactPassTraceV3, String> {
        self.run_contact_policy_pass_trace_extended(
            scenario,
            target_step,
            identity_policy,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
            soft_feedback_policy,
            normal_relaxation_policy,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
            false,
        )
    }

    pub fn run_authority_gpu_probe_pass_trace(
        &self,
        scenario: &ScenarioV1,
        target_step: u32,
    ) -> Result<GpuGeneratedContactPassTraceV3, String> {
        self.run_contact_policy_pass_trace_extended(
            scenario,
            target_step,
            ContactIdentityPolicyV1::SELECTED,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                scenario.bodies.len() as u32,
            ),
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    /// Captures the sole unpromoted S17 branch-robust candidate using the
    /// exact S13 graph and only the frozen S14 separation expression.
    pub fn run_branch_robust_gpu_promotion_pass_trace(
        &self,
        scenario: &ScenarioV1,
        target_step: u32,
    ) -> Result<GpuGeneratedContactPassTraceV3, String> {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        self.run_contact_policy_pass_trace_extended(
            scenario,
            target_step,
            ContactIdentityPolicyV1::SELECTED,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                scenario.bodies.len() as u32,
            ),
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_contact_policy_pass_trace_extended(
        &self,
        scenario: &ScenarioV1,
        target_step: u32,
        identity_policy: ContactIdentityPolicyV1,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
        authority_gpu_probe: bool,
    ) -> Result<GpuGeneratedContactPassTraceV3, String> {
        if target_step == 0 || target_step > scenario.requested_steps {
            return Err("generated pass trace target must be inside the scenario".to_owned());
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated pass trace body count exceeds u32".to_owned())?;
        let storage = GpuBodyStorage::new(
            &self.device,
            body_count,
            crate::BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let scheduling_policy = if authority_gpu_probe {
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(body_count)
        } else {
            scheduling_policy
        };
        // Pass traces expose one logical operation per color and therefore
        // retain the scalable per-color form even when the small interactive
        // tier dispatches those colors inside one island pass.
        let graph = if authority_gpu_probe {
            if substep_contact_policy
                == crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
            {
                if contact_coefficient_policy
                    == crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft
                {
                    GpuGeneratedContactGraph::new_branch_robust_tunable(
                        &self.device,
                        &storage,
                        scenario,
                    )?
                } else {
                    GpuGeneratedContactGraph::new_branch_robust_gpu_promotion(
                        &self.device,
                        &storage,
                        scenario,
                    )?
                }
            } else {
                GpuGeneratedContactGraph::new_authority_gpu_probe(
                    &self.device,
                    &storage,
                    scenario,
                )?
            }
        } else {
            GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
                &self.device,
                &storage,
                scenario,
                GpuContactPersistenceTopology::StablePairSlot,
                GpuGatherTopology::StableBodyEdge,
                false,
                false,
                u32::MAX,
                execution,
                scheduling_policy,
                identity_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )?
        };
        let expected_descriptors = graph.solver.pass_trace_descriptors();
        let state_stride_bytes = graph.pass_trace_state_bytes();
        let state_count = expected_descriptors.len() + 1;
        let trace_bytes = state_stride_bytes
            .checked_mul(state_count as u64)
            .ok_or_else(|| "generated pass trace byte count overflowed".to_owned())?;
        let state_readback = readback(
            &self.device,
            "generated_pass_trace_state_readback",
            trace_bytes,
        );
        let pass_layout = graph.pass_evidence_layout();
        let pass_readback = readback(
            &self.device,
            "generated_pass_trace_evidence_readback",
            pass_layout.total_bytes,
        );
        let construction_readback = readback(
            &self.device,
            "generated_pass_trace_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * 4,
        );
        let step_readback = readback(&self.device, "generated_pass_trace_step_readback", 4);

        for _ in 1..target_step {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_generated_pass_trace_leading_step"),
                });
            graph.encode_step(&mut encoder, false);
            self.queue.submit([encoder.finish()]);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_generated_pass_trace_target_step"),
            });
        let descriptors =
            graph.encode_step_with_pass_trace(&mut encoder, &storage, &state_readback)?;
        if descriptors != expected_descriptors {
            return Err("generated pass trace schedule changed while encoding".to_owned());
        }
        graph.copy_pass_evidence(&mut encoder, &pass_readback);
        graph.copy_construction_graph_evidence(&mut encoder, &construction_readback);
        let (step_source, step_offset) = graph.persistence.step_counter_source();
        encoder.copy_buffer_to_buffer(step_source, step_offset, &step_readback, 0, 4);
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("generated pass trace poll failed: {error}"))?;

        let state_bytes = crate::collision_candidates::map_values::<u8>(
            &self.device,
            &state_readback,
            trace_bytes as usize,
        )?;
        let body_bytes = body_count as usize * size_of::<[f32; 4]>();
        let prepared_bytes = graph.point_capacity as usize * size_of::<PreparedPointRaw>();
        if state_stride_bytes as usize != body_bytes * 2 + prepared_bytes {
            return Err("generated pass trace state layout is inconsistent".to_owned());
        }
        let mut states = Vec::with_capacity(state_count);
        for state_index in 0..state_count {
            let start = state_index * state_stride_bytes as usize;
            let poses =
                bytemuck::try_cast_slice::<u8, [f32; 4]>(&state_bytes[start..start + body_bytes])
                    .map_err(|error| format!("decode pass-trace poses: {error}"))?;
            let motions = bytemuck::try_cast_slice::<u8, [f32; 4]>(
                &state_bytes[start + body_bytes..start + body_bytes * 2],
            )
            .map_err(|error| format!("decode pass-trace motions: {error}"))?;
            let contacts = decode_prepared_contact_bytes(
                &state_bytes[start + body_bytes * 2..start + body_bytes * 2 + prepared_bytes],
                graph.point_capacity,
            )?;
            let mut bodies = scenario.bodies.clone();
            for ((body, pose), motion) in bodies.iter_mut().zip(poses).zip(motions) {
                body.position.x = pose[0];
                body.position.y = pose[1];
                body.angle = physbox_contracts::FiniteF32::new(pose[2])
                    .map_err(|error| error.to_string())?;
                body.linear_velocity.x = motion[0];
                body.linear_velocity.y = motion[1];
                body.angular_velocity = physbox_contracts::FiniteF32::new(motion[2])
                    .map_err(|error| error.to_string())?;
            }
            states.push(GpuGeneratedContactPassStateV3 {
                version: GPU_GENERATED_CONTACT_PASS_TRACE_VERSION,
                after_operation_index: state_index
                    .checked_sub(1)
                    .map(|index| descriptors[index].operation_index),
                bodies,
                contacts,
            });
        }
        let pass_evidence = crate::body_floor_solver::decode_pass_evidence(
            &crate::collision_candidates::map_values::<u8>(
                &self.device,
                &pass_readback,
                pass_layout.total_bytes as usize,
            )?,
            pass_layout,
        )?;
        let coloring = pass_evidence
            .coloring
            .ok_or_else(|| "generated pass trace omitted coloring evidence".to_owned())?;
        let construction =
            decode_construction_graph_evidence(&crate::collision_candidates::map_values::<u8>(
                &self.device,
                &construction_readback,
                COLLISION_CAPACITY_COUNTER_BYTES as usize * 4,
            )?)?;
        let completed_step =
            crate::collision_candidates::map_values::<u32>(&self.device, &step_readback, 1)?[0];
        if completed_step != target_step {
            return Err(format!(
                "generated pass trace completed step {completed_step}, expected {target_step}"
            ));
        }
        let trace = GpuGeneratedContactPassTraceV3 {
            version: GPU_GENERATED_CONTACT_PASS_TRACE_VERSION,
            adapter: self.diagnostics.clone(),
            source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
            target_step,
            solver_settings: scenario.settings,
            solver_execution: execution,
            solver_scheduling_policy: scheduling_policy,
            contact_coefficient_policy: graph.contact_coefficient_policy,
            substep_contact_policy: graph.substep_contact_policy,
            restitution_timing_policy: graph.restitution_timing_policy,
            contact_identity_policy: identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            pass_trace_dispatch: GpuPassTraceDispatchV1::DirectOverCompactedColorCapacity,
            mapped_after_final_step_only: true,
            state_stride_bytes,
            descriptors,
            states,
            coloring,
            construction,
        };
        trace.validate()?;
        Ok(trace)
    }

    /// Captures motion-only `f32` state at every logical pass boundary for a
    /// contiguous outer-step window. The window stays GPU ordered and maps
    /// once after its final persistence store; exact full-state traces remain
    /// the scalar/contact authority at selected anchor steps.
    pub fn run_contact_policy_pass_motion_window(
        &self,
        scenario: &ScenarioV1,
        first_step: u32,
        last_step: u32,
        identity_policy: ContactIdentityPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    ) -> Result<GpuGeneratedContactPassMotionWindowV1, String> {
        if first_step == 0 || first_step > last_step || last_step > scenario.requested_steps {
            return Err("generated pass-motion window must lie inside the scenario".to_owned());
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated pass-motion body count exceeds u32".to_owned())?;
        let storage = GpuBodyStorage::new(
            &self.device,
            body_count,
            crate::BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let execution = GpuSolverExecutionV4::SELECTED;
        let scheduling_policy = GpuSolverSchedulingPolicyV1::SELECTED;
        let normal_relaxation_policy = crate::GpuNormalRelaxationPolicyV1::SELECTED;
        let graph = GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_identity_and_contact_feedback(
            &self.device,
            &storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
            identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
        )?;
        let expected_descriptors = graph.solver.pass_trace_descriptors();
        let state_count = expected_descriptors.len() + 1;
        let motion_state_stride_bytes = graph.pass_trace_motion_bytes();
        let step_stride_bytes = motion_state_stride_bytes
            .checked_mul(state_count as u64)
            .ok_or_else(|| "generated pass-motion step byte count overflowed".to_owned())?;
        let step_count = last_step - first_step + 1;
        let trace_bytes = step_stride_bytes
            .checked_mul(u64::from(step_count))
            .ok_or_else(|| "generated pass-motion window byte count overflowed".to_owned())?;
        let motion_readback = readback(
            &self.device,
            "generated_pass_motion_window_readback",
            trace_bytes,
        );
        let pass_layout = graph.pass_evidence_layout();
        let pass_readback = readback(
            &self.device,
            "generated_pass_motion_window_evidence_readback",
            pass_layout.total_bytes,
        );
        let construction_readback = readback(
            &self.device,
            "generated_pass_motion_window_capacity_readback",
            u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * 4,
        );
        let step_readback = readback(
            &self.device,
            "generated_pass_motion_window_step_readback",
            4,
        );

        for _ in 1..first_step {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_generated_pass_motion_window_leading_step"),
                });
            graph.encode_step(&mut encoder, false);
            self.queue.submit([encoder.finish()]);
        }
        for completed_step in first_step..=last_step {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_generated_pass_motion_window_step"),
                });
            let destination_offset = u64::from(completed_step - first_step) * step_stride_bytes;
            let descriptors = graph.encode_step_with_pass_motion_trace(
                &mut encoder,
                &storage,
                &motion_readback,
                destination_offset,
            )?;
            if descriptors != expected_descriptors {
                return Err("generated pass-motion schedule changed while encoding".to_owned());
            }
            if completed_step == last_step {
                graph.copy_pass_evidence(&mut encoder, &pass_readback);
                graph.copy_construction_graph_evidence(&mut encoder, &construction_readback);
                let (step_source, step_offset) = graph.persistence.step_counter_source();
                encoder.copy_buffer_to_buffer(step_source, step_offset, &step_readback, 0, 4);
            }
            self.queue.submit([encoder.finish()]);
        }
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("generated pass-motion window poll failed: {error}"))?;

        let motion_bytes = crate::collision_candidates::map_values::<u8>(
            &self.device,
            &motion_readback,
            trace_bytes as usize,
        )?;
        let mut steps = Vec::with_capacity(step_count as usize);
        for step_index in 0..step_count as usize {
            let mut states = Vec::with_capacity(state_count);
            let step_start = step_index * step_stride_bytes as usize;
            for state_index in 0..state_count {
                let start = step_start + state_index * motion_state_stride_bytes as usize;
                let motions = bytemuck::try_cast_slice::<u8, [f32; 4]>(
                    &motion_bytes[start..start + motion_state_stride_bytes as usize],
                )
                .map_err(|error| format!("decode pass-motion state: {error}"))?;
                states.push(motions.to_vec());
            }
            steps.push(GpuGeneratedContactPassMotionStepV1 {
                completed_step: first_step + step_index as u32,
                states,
            });
        }
        let pass_evidence = crate::body_floor_solver::decode_pass_evidence(
            &crate::collision_candidates::map_values::<u8>(
                &self.device,
                &pass_readback,
                pass_layout.total_bytes as usize,
            )?,
            pass_layout,
        )?;
        let coloring = pass_evidence
            .coloring
            .ok_or_else(|| "generated pass-motion window omitted coloring evidence".to_owned())?;
        let construction =
            decode_construction_graph_evidence(&crate::collision_candidates::map_values::<u8>(
                &self.device,
                &construction_readback,
                COLLISION_CAPACITY_COUNTER_BYTES as usize * 4,
            )?)?;
        let completed_step =
            crate::collision_candidates::map_values::<u32>(&self.device, &step_readback, 1)?[0];
        if completed_step != last_step {
            return Err(format!(
                "generated pass-motion window completed step {completed_step}, expected {last_step}"
            ));
        }
        let window = GpuGeneratedContactPassMotionWindowV1 {
            version: GPU_GENERATED_CONTACT_PASS_MOTION_WINDOW_VERSION,
            adapter: self.diagnostics.clone(),
            source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
            first_step,
            last_step,
            solver_settings: scenario.settings,
            solver_execution: execution,
            solver_scheduling_policy: scheduling_policy,
            contact_coefficient_policy: graph.contact_coefficient_policy,
            substep_contact_policy: graph.substep_contact_policy,
            restitution_timing_policy: graph.restitution_timing_policy,
            contact_identity_policy: identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            pass_trace_dispatch: GpuPassTraceDispatchV1::DirectOverCompactedColorCapacity,
            mapped_after_final_step_only: true,
            motion_state_stride_bytes,
            descriptors: expected_descriptors,
            steps,
            coloring,
            construction,
        };
        window.validate()?;
        Ok(window)
    }

    /// Records exact post-step state, narrowphase output, and warm-start state
    /// for bounded native settling validation. The copies stay GPU ordered;
    /// bounded readback windows are mapped without feeding CPU data back into
    /// the simulation state.
    pub fn run_selected_observation_series(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_selected_observation_series_with_submission_batch(
            scenario,
            observation_interval_steps,
            completed_steps,
            VALIDATION_STEPS_PER_SUBMISSION,
        )
    }

    /// Uses the exact selected graph while bounding only command-submission
    /// duration for slower validation adapters. Product execution and the
    /// authored step sequence are unchanged.
    pub fn run_selected_observation_series_with_submission_batch(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        submission_batch_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        validate_selected_settings(scenario)?;
        self.run_contact_policy_observation_series_with_submission_batch(
            scenario,
            observation_interval_steps,
            completed_steps,
            ContactIdentityPolicyV1::SELECTED,
            submission_batch_steps,
        )
    }

    /// Runs the named S10 warm-start-off diagnostic over the otherwise exact
    /// selected hard/direct graph. The fixed construction rejects soft or
    /// otherwise non-selected solver settings instead of composing this
    /// diagnostic with another laboratory policy.
    pub fn run_warm_start_off_diagnostic_observation_series(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        validate_selected_settings(scenario)?;
        let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
        self.run_contact_policy_observation_series_with_extended_solver_policies(
            scenario,
            observation_interval_steps,
            completed_steps,
            ContactIdentityPolicyV1::SELECTED,
            VALIDATION_STEPS_PER_SUBMISSION,
            GpuSolverExecutionV4::WARM_START_OFF_DIAGNOSTIC,
            scheduling_policy,
            crate::GpuSoftFeedbackPolicyV1::SELECTED,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
        )
    }

    pub fn run_observation_series_with_identity_policy(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        validate_selected_settings(scenario)?;
        self.run_contact_policy_observation_series(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
        )
    }

    /// Runs the same bounded GPU-resident observation contract with an
    /// explicitly authored laboratory contact policy. Product callers remain
    /// on `run_selected_observation_series` and reject such settings.
    pub fn run_contact_policy_observation_series(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_submission_batch(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            VALIDATION_STEPS_PER_SUBMISSION,
        )
    }

    /// Varies only command-submission grouping for bounded cross-backend
    /// diagnosis. A batch of one mirrors the interactive outer-step cadence,
    /// while all state remains GPU ordered and maps only after the final step.
    pub fn run_contact_policy_observation_series_with_submission_batch(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_submission_batch_and_soft_feedback(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            crate::GpuSoftFeedbackPolicyV1::SELECTED,
        )
    }

    /// Runs a laboratory-only soft-feedback formulation comparator. The
    /// selected and product-facing series methods always pass the selected
    /// every-biased-iteration policy.
    pub fn run_contact_policy_observation_series_with_submission_batch_and_soft_feedback(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
        self.run_contact_policy_observation_series_with_execution_scheduling_and_soft_feedback(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
            soft_feedback_policy,
        )
    }

    /// Crosses existing laboratory solver and scheduling policies with the
    /// bounded contact-policy trace. Product and standard series remain
    /// selected.
    #[allow(clippy::too_many_arguments)]
    pub fn run_contact_policy_observation_series_with_execution_scheduling_and_soft_feedback(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_execution_scheduling_and_contact_feedback(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            crate::GpuNormalRelaxationPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_contact_policy_observation_series_with_execution_scheduling_and_contact_feedback(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_solver_policies(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_contact_policy_observation_series_with_solver_policies(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_extended_solver_policies(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_contact_policy_observation_series_with_extended_solver_policies(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_contact_policy_observation_series_with_authority_mode(
            scenario,
            observation_interval_steps,
            completed_steps,
            identity_policy,
            submission_batch_steps,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            false,
        )
    }

    /// Executes the one exact, unpromoted S13 candidate. No policy parameter
    /// is exposed, so this validation entry point cannot widen the experiment.
    pub fn run_authority_gpu_probe_observation_series(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        crate::AuthorityGpuProbePolicyV1::AUTHORIZED.validate_product_selection(false)?;
        self.run_contact_policy_observation_series_with_authority_mode(
            scenario,
            observation_interval_steps,
            completed_steps,
            ContactIdentityPolicyV1::SELECTED,
            VALIDATION_STEPS_PER_SUBMISSION,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                scenario.bodies.len() as u32,
            ),
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    /// Executes the sole unpromoted S17 branch-robust candidate. This entry
    /// point exposes no policy parameters and cannot change product selection.
    pub fn run_branch_robust_gpu_promotion_observation_series(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        self.run_branch_robust_gpu_promotion_observation_series_with_submission_batch(
            scenario,
            observation_interval_steps,
            completed_steps,
            VALIDATION_STEPS_PER_SUBMISSION,
        )
    }

    /// Uses the exact unpromoted S17 graph while bounding only validation
    /// command-submission duration. No candidate parameter is exposed.
    pub fn run_branch_robust_gpu_promotion_observation_series_with_submission_batch(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        submission_batch_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        self.run_contact_policy_observation_series_with_authority_mode(
            scenario,
            observation_interval_steps,
            completed_steps,
            ContactIdentityPolicyV1::SELECTED,
            submission_batch_steps,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                scenario.bodies.len() as u32,
            ),
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    /// Coefficient-exploration lane: the exact branch-robust GPU execution
    /// driven by the scenario's own soft-contact coefficients instead of the
    /// frozen S17 tuple. Laboratory-only; product selection stays impossible.
    pub fn run_branch_robust_tunable_observation_series_with_submission_batch(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        submission_batch_steps: u32,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        self.run_contact_policy_observation_series_with_authority_mode(
            scenario,
            observation_interval_steps,
            completed_steps,
            ContactIdentityPolicyV1::SELECTED,
            submission_batch_steps,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                scenario.bodies.len() as u32,
            ),
            crate::GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            crate::GpuNormalRelaxationPolicyV1::PointSequential,
            crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft,
            crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            crate::GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_contact_policy_observation_series_with_authority_mode(
        &self,
        scenario: &ScenarioV1,
        observation_interval_steps: u32,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
        submission_batch_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: crate::GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: crate::GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
        authority_gpu_probe: bool,
    ) -> Result<GpuGeneratedContactSeriesV1, String> {
        if observation_interval_steps == 0
            || completed_steps == 0
            || !completed_steps.is_multiple_of(observation_interval_steps)
            || completed_steps > scenario.requested_steps
        {
            return Err(
                "generated settling series requires an exact positive cadence within the scenario"
                    .to_owned(),
            );
        }
        if submission_batch_steps == 0 || submission_batch_steps > VALIDATION_STEPS_PER_SUBMISSION {
            return Err(format!(
                "generated settling submission batch must be between 1 and {VALIDATION_STEPS_PER_SUBMISSION} steps"
            ));
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated settling body count exceeds u32".to_owned())?;
        let storage = GpuBodyStorage::new(
            &self.device,
            body_count,
            crate::BODY_COMMAND_CAPACITY,
            &scenario.bodies,
        )?;
        let graph = if authority_gpu_probe {
            if substep_contact_policy
                == crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
            {
                if contact_coefficient_policy
                    == crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft
                {
                    GpuGeneratedContactGraph::new_branch_robust_tunable(
                        &self.device,
                        &storage,
                        scenario,
                    )?
                } else {
                    GpuGeneratedContactGraph::new_branch_robust_gpu_promotion(
                        &self.device,
                        &storage,
                        scenario,
                    )?
                }
            } else {
                GpuGeneratedContactGraph::new_authority_gpu_probe(
                    &self.device,
                    &storage,
                    scenario,
                )?
            }
        } else {
            GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
                &self.device,
                &storage,
                scenario,
                GpuContactPersistenceTopology::StablePairSlot,
                GpuGatherTopology::StableBodyEdge,
                false,
                false,
                u32::MAX,
                execution,
                scheduling_policy,
                identity_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )?
        };
        let snapshot_count = completed_steps / observation_interval_steps;
        let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
        let mut snapshots = Vec::with_capacity(snapshot_count as usize);
        let mut encoded_steps = 0;
        let mut decoded_snapshots = 0;
        while decoded_snapshots < snapshot_count {
            let flush_snapshot_count =
                (snapshot_count - decoded_snapshots).min(VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH);
            let flush_completed_steps = (decoded_snapshots + flush_snapshot_count)
                .checked_mul(observation_interval_steps)
                .ok_or_else(|| "generated settling readback cadence overflowed".to_owned())?;
            let readbacks = GeneratedSettlingReadbacks::new(
                &self.device,
                &graph,
                body_count,
                flush_snapshot_count,
            );
            let mut flush_snapshot_index = 0;
            while encoded_steps < flush_completed_steps {
                let chunk = (flush_completed_steps - encoded_steps).min(submission_batch_steps);
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("physbox5_generated_settling_series"),
                        });
                for local_step in 0..chunk {
                    graph.encode_step(&mut encoder, false);
                    let step = encoded_steps + local_step + 1;
                    if step.is_multiple_of(observation_interval_steps) {
                        readbacks.encode_copy(&mut encoder, &storage, &graph, flush_snapshot_index);
                        flush_snapshot_index += 1;
                    }
                }
                self.queue.submit([encoder.finish()]);
                encoded_steps += chunk;
            }
            if flush_snapshot_index != flush_snapshot_count {
                return Err(
                    "generated settling series encoded the wrong flush sample count".to_owned(),
                );
            }
            self.device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("generated settling series poll failed: {error}"))?;
            let mut decoded = self.decode_generated_settling_readbacks(
                scenario,
                &graph,
                &readbacks,
                decoded_snapshots,
                observation_interval_steps,
                &source_scenario_hash,
            )?;
            snapshots.append(&mut decoded);
            readbacks.destroy();
            self.device.poll(wgpu::PollType::Wait).map_err(|error| {
                format!("generated settling readback destruction poll failed: {error}")
            })?;
            decoded_snapshots += flush_snapshot_count;
        }
        if encoded_steps != completed_steps || snapshots.len() != snapshot_count as usize {
            return Err("generated settling series completed the wrong evidence count".to_owned());
        }
        let series = GpuGeneratedContactSeriesV1 {
            version: 1,
            adapter: self.diagnostics.clone(),
            source_scenario_hash,
            observation_interval_steps,
            completed_steps,
            solver_settings: scenario.settings,
            solver_execution: execution,
            solver_scheduling_policy: scheduling_policy,
            contact_coefficient_policy: graph.contact_coefficient_policy,
            substep_contact_policy: graph.substep_contact_policy,
            restitution_timing_policy: graph.restitution_timing_policy,
            contact_identity_policy: identity_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            authority_gpu_probe_policy: (authority_gpu_probe
                && substep_contact_policy
                    == crate::GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation)
                .then_some(crate::AuthorityGpuProbePolicyV1::AUTHORIZED),
            branch_robust_gpu_promotion_policy: (authority_gpu_probe
                && substep_contact_policy
                    == crate::GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
                && contact_coefficient_policy
                    != crate::GpuContactCoefficientPolicyV1::AuthorityTunableSoft)
                .then_some(crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED),
            authority_identity: graph.authority_identity,
            dispatches_per_step: graph.dispatches_per_step(),
            allocated_bytes: graph.allocated_bytes(),
            snapshots,
        };
        series.validate()?;
        Ok(series)
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_generated_settling_readbacks(
        &self,
        scenario: &ScenarioV1,
        graph: &GpuGeneratedContactGraph,
        readbacks: &GeneratedSettlingReadbacks,
        first_snapshot_index: u32,
        observation_interval_steps: u32,
        source_scenario_hash: &str,
    ) -> Result<Vec<GpuGeneratedContactSnapshotV1>, String> {
        let snapshot_count_usize = readbacks.snapshot_count as usize;
        let body_count_usize = scenario.bodies.len();
        let state = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &readbacks.state,
            snapshot_count_usize * body_count_usize * 3,
        )?;
        let candidate_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &readbacks.candidate_capacity,
            snapshot_count_usize,
        )?;
        let manifold_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &readbacks.manifold_capacity,
            snapshot_count_usize,
        )?;
        let point_raw = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &readbacks.point_capacity,
            snapshot_count_usize,
        )?;
        let persistence_raw =
            crate::collision_candidates::map_values::<crate::collision_persistence::CapacityRaw>(
                &self.device,
                &readbacks.persistence_capacity,
                snapshot_count_usize,
            )?;
        let work_raw = crate::collision_candidates::map_values::<
            crate::collision_persistence::WorkRaw,
        >(&self.device, &readbacks.work, snapshot_count_usize)?;
        let gpu_steps = crate::collision_candidates::map_values::<u32>(
            &self.device,
            &readbacks.step,
            snapshot_count_usize,
        )?;
        let color_status = readbacks
            .color_status
            .as_ref()
            .map(|buffer| {
                crate::collision_candidates::map_values::<u32>(
                    &self.device,
                    buffer,
                    snapshot_count_usize,
                )
            })
            .transpose()?
            .unwrap_or_else(|| vec![0; snapshot_count_usize]);
        let rejected = (0..snapshot_count_usize)
            .map(|index| {
                candidate_raw[index][3] != 0
                    || manifold_raw[index][2] != manifold_raw[index][1]
                    || point_raw[index][2] != point_raw[index][1]
                    || persistence_raw[index].values[2] != persistence_raw[index].values[1]
                    || color_status[index] != 0
            })
            .collect::<Vec<_>>();
        let manifold_series = crate::collision_narrowphase::map_manifold_series_from_readbacks(
            &self.device,
            &readbacks.manifold_records,
            &readbacks.point_records,
            graph.narrowphase.manifold_capacity(),
            graph.narrowphase.point_capacity(),
            &manifold_raw
                .iter()
                .map(|values| values[2])
                .collect::<Vec<_>>(),
            &rejected,
        )?;
        let pair_graph = match &graph.persistence {
            GeneratedPersistenceGraph::PairSlot(graph) => graph,
            GeneratedPersistenceGraph::FullKey(_) => {
                return Err("selected settling series did not use pair-slot persistence".to_owned());
            }
        };
        let cache_records = crate::collision_candidates::map_values::<PairRecordRaw>(
            &self.device,
            &readbacks.cache,
            pair_graph.pair_capacity as usize * snapshot_count_usize,
        )?;
        let mut snapshots = Vec::with_capacity(snapshot_count_usize);
        for index in 0..snapshot_count_usize {
            let global_snapshot_index = first_snapshot_index + index as u32;
            let expected_step = (global_snapshot_index + 1) * observation_interval_steps;
            if gpu_steps[index] != expected_step {
                return Err(format!(
                    "GPU settling completed-step counter {} differed from {expected_step}",
                    gpu_steps[index]
                ));
            }
            let state_start = index * body_count_usize * 3;
            let pose_start = state_start;
            let motion_start = state_start + body_count_usize;
            let metadata_start = state_start + body_count_usize * 2;
            let mut bodies = scenario.bodies.clone();
            for (body_index, body) in bodies.iter_mut().enumerate() {
                let pose = state[pose_start + body_index].map(f32::from_bits);
                let motion = state[motion_start + body_index].map(f32::from_bits);
                let metadata = state[metadata_start + body_index];
                if metadata[0] != body.id.generation {
                    return Err(format!(
                        "body {body_index} generation changed at settling step {expected_step}"
                    ));
                }
                body.position.x = pose[0];
                body.position.y = pose[1];
                body.angle = physbox_contracts::FiniteF32::new(pose[2])
                    .map_err(|error| error.to_string())?;
                body.linear_velocity.x = motion[0];
                body.linear_velocity.y = motion[1];
                body.angular_velocity = physbox_contracts::FiniteF32::new(motion[2])
                    .map_err(|error| error.to_string())?;
            }
            let work = crate::collision_persistence::work_record(work_raw[index]);
            let mut capacities = vec![
                raw_capacity(
                    "candidate-pairs",
                    CANDIDATE_PAIR_BYTES,
                    candidate_raw[index],
                ),
                raw_capacity(
                    "contact-manifolds",
                    COLLISION_MANIFOLD_BYTES,
                    manifold_raw[index],
                ),
                raw_capacity(
                    "contact-points",
                    COLLISION_CONTACT_POINT_BYTES,
                    point_raw[index],
                ),
                crate::collision_persistence::capacity_record(
                    "pair-slot-cache",
                    PAIR_RECORD_BYTES,
                    persistence_raw[index],
                ),
            ];
            capacities.push(CollisionCapacityV1 {
                class: "solver-colors".to_owned(),
                stride_bytes: 4,
                capacity_elements: 32,
                attempted_elements: if color_status[index] == 0 { 0 } else { 33 },
                committed_elements: 0,
                allocated_bytes: 128,
                high_water_bytes: if color_status[index] == 0 { 0 } else { 128 },
                overflowed: color_status[index] != 0,
                accepted: color_status[index] == 0,
            });
            capacities.sort_by(|left, right| left.class.cmp(&right.class));
            let failure = capacities
                .iter()
                .find(|capacity| !capacity.accepted)
                .map(|capacity| FailureV1 {
                    kind: FailureKindV1::CapacityOverflow,
                    reason: format!(
                        "{} capacity {} rejected {} attempted records",
                        capacity.class, capacity.capacity_elements, capacity.attempted_elements
                    ),
                    first_step: Some(expected_step),
                    first_stage: Some(if capacity.class == "candidate-pairs" {
                        "candidate".to_owned()
                    } else if capacity.class == "solver-colors" {
                        "solver-coloring".to_owned()
                    } else if capacity.class == "pair-slot-cache" {
                        "persistence".to_owned()
                    } else {
                        "narrowphase".to_owned()
                    }),
                    reproducer_hash: Some(source_scenario_hash.to_owned()),
                });
            let cache = if rejected[index] {
                Vec::new()
            } else {
                let cache_start = index * pair_graph.pair_capacity as usize;
                let mut cache = Vec::with_capacity(point_raw[index][2] as usize);
                for record in cache_records
                    [cache_start..cache_start + persistence_raw[index].values[2] as usize]
                    .iter()
                    .copied()
                {
                    let (_, mut points) = pair_output(record)?;
                    cache.append(&mut points);
                }
                cache
            };
            snapshots.push(GpuGeneratedContactSnapshotV1 {
                version: 1,
                completed_steps: expected_step,
                bodies,
                manifolds: manifold_series[index].clone(),
                cache,
                work,
                capacities,
                failure,
            });
        }
        Ok(snapshots)
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

    /// Final-only companion to the S10 warm-start differential observation
    /// lane. It is deliberately parameter-free so unsupported policy
    /// compositions cannot enter through this API.
    pub fn run_warm_start_off_diagnostic(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
    ) -> Result<GpuGeneratedContactOutput, String> {
        validate_selected_settings(scenario)?;
        self.run_solver_policy_experiment(
            scenario,
            completed_steps,
            GpuSolverExecutionV4::WARM_START_OFF_DIAGNOSTIC,
            selected_scheduling_for_scenario(scenario)?,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
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
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
        )
    }

    pub fn run_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_experiment_with_gather_topology(
            scenario,
            completed_steps,
            GpuGatherTopology::StableBodyEdge,
            false,
            execution,
        )
    }

    pub fn run_experiment_with_gather_topology(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_and_execution(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            gather_topology,
            permute_edge_build,
            false,
            u32::MAX,
            execution,
        )
    }

    pub fn run_scheduling_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_and_scheduling(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
        )
    }

    /// Runs a named laboratory coefficient policy without changing product
    /// construction or the scenario-owned solver settings.
    pub fn run_coefficient_policy_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_identity_and_coefficient_policy(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
            ContactIdentityPolicyV1::SELECTED,
            contact_coefficient_policy,
        )
    }

    /// Runs the complete named S9 solver-policy tuple without changing product
    /// construction or scenario-owned settings.
    #[allow(clippy::too_many_arguments)]
    pub fn run_solver_policy_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
            ContactIdentityPolicyV1::SELECTED,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
        )
    }

    pub fn run_identity_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
        self.run_with_topologies_execution_scheduling_and_identity_policy(
            scenario,
            completed_steps,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
            identity_policy,
        )
    }

    /// Captures post-step body poses for bounded native validation while keeping
    /// every result resident until the requested step count has completed.
    pub fn run_experiment_body_history(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
    ) -> Result<Vec<Vec<physbox_contracts::BodyStateV1>>, String> {
        let history =
            self.run_experiment_body_history_diagnostic(scenario, completed_steps, execution)?;
        if let Some(failure) = history.failures.first() {
            return Err(failure.reason.clone());
        }
        Ok(history.bodies)
    }

    /// Captures the same bounded post-step history while preserving transient
    /// atomic construction and coloring rejections as diagnosed failures.
    pub fn run_experiment_body_history_diagnostic(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
    ) -> Result<GpuGeneratedContactBodyHistoryV1, String> {
        self.run_scheduling_experiment_body_history_diagnostic(
            scenario,
            completed_steps,
            execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
    }

    pub fn run_scheduling_experiment_body_history_diagnostic(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<GpuGeneratedContactBodyHistoryV1, String> {
        if completed_steps == 0 {
            return Err("generated contact pose history requires at least one step".to_owned());
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
        let graph = GpuGeneratedContactGraph::new_with_topologies_execution_and_scheduling(
            &self.device,
            &storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            GpuGatherTopology::StableBodyEdge,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
        )?;
        let pose_step_bytes = u64::from(body_count) * size_of::<[f32; 4]>() as u64;
        let pose_history_bytes = pose_step_bytes
            .checked_mul(u64::from(completed_steps))
            .ok_or_else(|| "generated pose-history byte count overflowed".to_owned())?;
        let capacity_step_bytes = u64::from(COLLISION_CAPACITY_COUNTER_BYTES) * 4;
        let capacity_history_bytes = capacity_step_bytes
            .checked_mul(u64::from(completed_steps))
            .ok_or_else(|| "generated capacity-history byte count overflowed".to_owned())?;
        let pose_history = readback(
            &self.device,
            "generated_pose_history_readback",
            pose_history_bytes,
        );
        let capacity_history = readback(
            &self.device,
            "generated_capacity_history_readback",
            capacity_history_bytes,
        );
        let color_status_history = graph.color_status_source().map(|_| {
            readback(
                &self.device,
                "generated_color_status_history_readback",
                u64::from(completed_steps) * size_of::<u32>() as u64,
            )
        });
        let step_readback = readback(&self.device, "generated_history_step_readback", 4);

        let mut encoded_steps = 0;
        while encoded_steps < completed_steps {
            let chunk = (completed_steps - encoded_steps).min(VALIDATION_STEPS_PER_SUBMISSION);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("physbox5_generated_contact_pose_history"),
                });
            for local_step in 0..chunk {
                graph.encode_step(&mut encoder, false);
                let step = encoded_steps + local_step;
                encoder.copy_buffer_to_buffer(
                    storage.pose_buffer(),
                    0,
                    &pose_history,
                    u64::from(step) * pose_step_bytes,
                    pose_step_bytes,
                );
                let current = graph.parity.get();
                for (index, source) in [
                    graph.broadphase.candidate_capacity_buffer(),
                    graph.narrowphase.manifold_capacity_buffer(),
                    graph.narrowphase.point_capacity_buffer(),
                    graph.persistence.current_capacity(current),
                ]
                .into_iter()
                .enumerate()
                {
                    encoder.copy_buffer_to_buffer(
                        source,
                        0,
                        &capacity_history,
                        u64::from(step) * capacity_step_bytes
                            + index as u64 * u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
                        u64::from(COLLISION_CAPACITY_COUNTER_BYTES),
                    );
                }
                if let (Some((source, source_offset)), Some(destination)) =
                    (graph.color_status_source(), &color_status_history)
                {
                    encoder.copy_buffer_to_buffer(
                        source,
                        source_offset,
                        destination,
                        u64::from(step) * size_of::<u32>() as u64,
                        size_of::<u32>() as u64,
                    );
                }
            }
            self.queue.submit([encoder.finish()]);
            encoded_steps += chunk;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_generated_contact_pose_history_step"),
            });
        let (step_source, step_offset) = graph.persistence.step_counter_source();
        encoder.copy_buffer_to_buffer(step_source, step_offset, &step_readback, 0, 4);
        self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("generated pose-history poll failed: {error}"))?;

        let gpu_steps =
            crate::collision_candidates::map_values::<u32>(&self.device, &step_readback, 1)?[0];
        if gpu_steps != completed_steps {
            return Err(format!(
                "GPU completed-step counter {gpu_steps} differed from {completed_steps}"
            ));
        }
        let poses = crate::collision_candidates::map_values::<[f32; 4]>(
            &self.device,
            &pose_history,
            scenario.bodies.len() * completed_steps as usize,
        )?;
        let capacities = crate::collision_candidates::map_values::<[u32; 4]>(
            &self.device,
            &capacity_history,
            4 * completed_steps as usize,
        )?;
        let color_statuses = color_status_history
            .as_ref()
            .map(|buffer| {
                crate::collision_candidates::map_values::<u32>(
                    &self.device,
                    buffer,
                    completed_steps as usize,
                )
            })
            .transpose()?;
        let mut history = Vec::with_capacity(completed_steps as usize);
        let mut first_construction_failure_step = None;
        let mut first_coloring_failure_step = None;
        for step in 0..completed_steps as usize {
            let step_capacities = &capacities[step * 4..step * 4 + 4];
            if step_capacities[0][3] != 0
                || step_capacities[1][2] != step_capacities[1][1]
                || step_capacities[2][2] != step_capacities[2][1]
                || step_capacities[3][2] != step_capacities[3][1]
            {
                first_construction_failure_step.get_or_insert(step as u32 + 1);
            }
            if color_statuses
                .as_ref()
                .is_some_and(|statuses| statuses[step] != 0)
            {
                first_coloring_failure_step.get_or_insert(step as u32 + 1);
            }
            let mut bodies = scenario.bodies.clone();
            for (body_index, body) in bodies.iter_mut().enumerate() {
                let pose = poses[step * scenario.bodies.len() + body_index];
                if !pose[0].is_finite() || !pose[1].is_finite() {
                    return Err(format!(
                        "body {body_index} has a non-finite post-step {} position",
                        step + 1
                    ));
                }
                body.position.x = pose[0];
                body.position.y = pose[1];
                body.angle = physbox_contracts::FiniteF32::new(pose[2])
                    .map_err(|error| error.to_string())?;
            }
            history.push(bodies);
        }
        let mut failures = Vec::with_capacity(2);
        if let Some(first_step) = first_construction_failure_step {
            failures.push(FailureV1 {
                kind: FailureKindV1::CapacityOverflow,
                reason: format!(
                    "generated construction capacity rejected post-step {first_step} pose history"
                ),
                first_step: Some(first_step),
                first_stage: Some("generated-construction".to_owned()),
                reproducer_hash: None,
            });
        }
        if let Some(first_step) = first_coloring_failure_step {
            failures.push(FailureV1 {
                kind: FailureKindV1::CapacityOverflow,
                reason: format!(
                    "generated solver coloring rejected post-step {first_step} pose history"
                ),
                first_step: Some(first_step),
                first_stage: Some("solver-coloring".to_owned()),
                reproducer_hash: None,
            });
        }
        failures.sort_by_key(|failure| failure.first_step);
        for failure in &failures {
            failure.validate().map_err(|error| error.to_string())?;
        }
        Ok(GpuGeneratedContactBodyHistoryV1 {
            version: GPU_GENERATED_CONTACT_BODY_HISTORY_VERSION,
            bodies: history,
            failures,
        })
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
        validate_selected_settings(scenario)?;
        let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
        self.run_with_topologies_execution_and_scheduling(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            GpuSolverExecutionV4::SELECTED,
            scheduling_policy,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_topology_experiment(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_and_execution(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_and_execution(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_and_scheduling(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_and_scheduling(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_and_identity_policy(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            ContactIdentityPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_scheduling_and_identity_policy(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_identity_and_coefficient_policy(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            crate::GpuContactCoefficientPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_scheduling_identity_and_coefficient_policy(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            contact_coefficient_policy,
            crate::GpuSubstepContactPolicyV1::SELECTED,
            crate::GpuRestitutionTimingPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
    ) -> Result<GpuGeneratedContactOutput, String> {
        let body_capacity = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated validation body capacity exceeds u32".to_owned())?;
        self.run_with_topologies_execution_scheduling_identity_extended_and_body_capacity(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            body_capacity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_scheduling_identity_extended_and_body_capacity(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
        body_capacity: u32,
    ) -> Result<GpuGeneratedContactOutput, String> {
        self.run_with_topologies_execution_scheduling_identity_extended_body_capacity_and_sleep(
            scenario,
            completed_steps,
            persistence_topology,
            gather_topology,
            permute_edge_build,
            permute_pair_build,
            fingerprint_mask,
            execution,
            scheduling_policy,
            identity_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            body_capacity,
            crate::awake_work::SleepExecutionConfig::disabled(60),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_topologies_execution_scheduling_identity_extended_body_capacity_and_sleep(
        &self,
        scenario: &ScenarioV1,
        completed_steps: u32,
        persistence_topology: GpuContactPersistenceTopology,
        gather_topology: GpuGatherTopology,
        permute_edge_build: bool,
        permute_pair_build: bool,
        fingerprint_mask: u32,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        identity_policy: ContactIdentityPolicyV1,
        contact_coefficient_policy: crate::GpuContactCoefficientPolicyV1,
        substep_contact_policy: crate::GpuSubstepContactPolicyV1,
        restitution_timing_policy: crate::GpuRestitutionTimingPolicyV1,
        body_capacity: u32,
        sleep_config: crate::awake_work::SleepExecutionConfig,
    ) -> Result<GpuGeneratedContactOutput, String> {
        if completed_steps == 0 {
            return Err("generated contact validation requires at least one step".to_owned());
        }
        scenario.validate().map_err(|error| error.to_string())?;
        let live_body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "generated validation body count exceeds u32".to_owned())?;
        if body_capacity < live_body_count {
            return Err(format!(
                "generated validation body capacity {body_capacity} cannot hold {live_body_count} live bodies"
            ));
        }
        let body_count = body_capacity;
        let storage = GpuBodyStorage::new_with_sleeping(
            &self.device,
            body_count,
            crate::BODY_COMMAND_CAPACITY,
            &scenario.bodies,
            sleep_config,
        )?;
        let graph =
            GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_identity_and_extended_solver_policies(
                &self.device,
                &storage,
                scenario,
                persistence_topology,
                gather_topology,
                permute_edge_build,
                permute_pair_build,
                fingerprint_mask,
                execution,
                scheduling_policy,
                identity_policy,
                crate::GpuSoftFeedbackPolicyV1::SELECTED,
                crate::GpuNormalRelaxationPolicyV1::SELECTED,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
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
        let awake_point_state_readback = readback(
            &self.device,
            "generated_awake_point_state_readback",
            16 * size_of::<u32>() as u64,
        );
        let awake_point_mapping_readback = readback(
            &self.device,
            "generated_awake_point_mapping_readback",
            u64::from(graph.point_capacity) * size_of::<u32>() as u64,
        );
        let awake_body_state_readback = readback(
            &self.device,
            "generated_awake_body_state_readback",
            u64::from(crate::awake_work::AWAKE_BODY_STATE_WORDS) * size_of::<u32>() as u64,
        );
        let solver_body_state_readback = readback(
            &self.device,
            "generated_solver_body_state_readback",
            20 * size_of::<u32>() as u64,
        );
        let manifold_records_readback = readback(
            &self.device,
            "generated_manifold_records_readback",
            u64::from(graph.narrowphase.manifold_capacity()) * u64::from(COLLISION_MANIFOLD_BYTES),
        );
        let point_records_readback = readback(
            &self.device,
            "generated_point_records_readback",
            u64::from(graph.narrowphase.point_capacity())
                * u64::from(COLLISION_CONTACT_POINT_BYTES),
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
        let pass_layout = graph.pass_evidence_layout();
        let pass_readback = readback(
            &self.device,
            "generated_pass_evidence_readback",
            pass_layout.total_bytes,
        );
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
            graph.broadphase.candidate_capacity_buffer(),
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
        graph.copy_awake_point_state(&mut encoder, &awake_point_state_readback);
        graph.copy_awake_point_mapping(&mut encoder, &awake_point_mapping_readback);
        encoder.copy_buffer_to_buffer(
            storage.awake_body_buffer(),
            0,
            &awake_body_state_readback,
            0,
            u64::from(crate::awake_work::AWAKE_BODY_STATE_WORDS) * size_of::<u32>() as u64,
        );
        graph.copy_solver_body_state(&mut encoder, &solver_body_state_readback);
        copy_full(
            &mut encoder,
            graph.narrowphase.manifolds(),
            &manifold_records_readback,
        );
        copy_full(
            &mut encoder,
            graph.narrowphase.points(),
            &point_records_readback,
        );
        copy_full(
            &mut encoder,
            graph.persistence.current_capacity(current),
            &persistence_capacity_readback,
        );
        copy_full(&mut encoder, graph.broadphase.work_buffer(), &work_readback);
        let (step_source, step_offset) = graph.persistence.step_counter_source();
        encoder.copy_buffer_to_buffer(step_source, step_offset, &step_readback, 0, 4);
        graph.copy_pass_evidence(&mut encoder, &pass_readback);
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
        let awake_point_raw = crate::collision_candidates::map_values::<u32>(
            &self.device,
            &awake_point_state_readback,
            16,
        )?;
        if awake_point_raw[2] > point_raw[2] {
            return Err(format!(
                "awake point count {} exceeds current contact count {}",
                awake_point_raw[2], point_raw[2]
            ));
        }
        let awake_point_mapping = crate::collision_candidates::map_values::<u32>(
            &self.device,
            &awake_point_mapping_readback,
            graph.point_capacity as usize,
        )?;
        let awake_body_raw = crate::collision_candidates::map_values::<u32>(
            &self.device,
            &awake_body_state_readback,
            crate::awake_work::AWAKE_BODY_STATE_WORDS as usize,
        )?;
        let solver_body_raw = crate::collision_candidates::map_values::<u32>(
            &self.device,
            &solver_body_state_readback,
            20,
        )?;
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
        let mut any_rejected = candidate_raw[3] != 0
            || manifold_raw[2] != manifold_raw[1]
            || point_raw[2] != point_raw[1]
            || persistence_raw.values[2] != persistence_raw.values[1];
        let mut pass_evidence = crate::body_floor_solver::decode_pass_evidence(
            &crate::collision_candidates::map_values::<u8>(
                &self.device,
                &pass_readback,
                pass_layout.total_bytes as usize,
            )?,
            pass_layout,
        )?;
        let awake_point_count = awake_point_raw[2];
        let valid_dense_point_indices = pass_evidence
            .contacts
            .iter()
            .filter(|contact| contact.valid && contact.point_index < awake_point_count)
            .map(|contact| contact.point_index)
            .collect::<Vec<_>>();
        pass_evidence
            .contacts
            .retain(|contact| contact.valid && contact.point_index < awake_point_count);
        for contact in &mut pass_evidence.contacts {
            contact.point_index = awake_point_mapping[contact.point_index as usize];
        }
        pass_evidence.contributions = pass_evidence
            .contributions
            .into_iter()
            .filter_map(|mut contribution| {
                (contribution.body_index < body_count
                    && valid_dense_point_indices.contains(&contribution.point_index))
                .then(|| {
                    contribution.point_index =
                        awake_point_mapping[contribution.point_index as usize];
                    contribution
                })
            })
            .collect();
        let coloring_rejected = pass_evidence
            .coloring
            .as_ref()
            .is_some_and(|coloring| coloring.overflowed);
        if execution.normal_solver.uses_block() && !coloring_rejected {
            pass_evidence.manifold_blocks.retain(|block| {
                pass_evidence.contacts.iter().any(|contact| {
                    contact.manifold_index == block.manifold_index
                        && contact.body_a == block.body_a
                        && contact.body_b == block.body_b
                })
            });
        } else {
            pass_evidence.manifold_blocks.clear();
        }
        any_rejected |= pass_evidence
            .coloring
            .as_ref()
            .is_some_and(|coloring| coloring.overflowed);
        let manifolds = crate::collision_narrowphase::map_manifolds_from_readbacks(
            &self.device,
            &manifold_records_readback,
            &point_records_readback,
            graph.narrowphase.manifold_capacity(),
            graph.narrowphase.point_capacity(),
            manifold_raw[2],
            any_rejected,
        )?;
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
        let mut capacities = graph.broadphase.capacities(candidate_capacity);
        capacities.extend(graph.narrowphase.capacities(
            manifold_capacity,
            point_capacity,
            work.narrowphase_tests,
        ));
        if let Some(coloring) = &pass_evidence.coloring {
            let attempted = if coloring.overflowed {
                u64::from(coloring.max_colors) + 1
            } else {
                u64::from(coloring.color_count)
            };
            capacities.push(CollisionCapacityV1 {
                class: "solver-colors".to_owned(),
                stride_bytes: 4,
                capacity_elements: u64::from(coloring.max_colors),
                attempted_elements: attempted,
                committed_elements: if coloring.overflowed { 0 } else { attempted },
                allocated_bytes: u64::from(coloring.max_colors) * 4,
                high_water_bytes: attempted.min(u64::from(coloring.max_colors)) * 4,
                overflowed: coloring.overflowed,
                accepted: !coloring.overflowed,
            });
        }
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
                capacities.extend(pair_graph.allocator.capacities());
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
                } else if capacity.class == "solver-colors" {
                    "solver-coloring".to_owned()
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
            solver_settings: scenario.settings,
            solver_execution: execution,
            solver_scheduling_policy: scheduling_policy,
            contact_coefficient_policy: graph.contact_coefficient_policy,
            substep_contact_policy: graph.substep_contact_policy,
            restitution_timing_policy: graph.restitution_timing_policy,
            contact_identity_policy: graph.identity_policy,
            awake_dynamic_body_count: awake_body_raw[0],
            solver_body_count: solver_body_raw[0],
            live_contact_point_count: point_raw[2],
            awake_contact_point_count: awake_point_raw[2],
            bodies,
            manifolds,
            cache,
            pass_evidence,
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
    pub body_capacity: u32,
    pub body_allocated_bytes: u64,
    pub active_body_allocated_bytes: u64,
    pub sleep_state_allocated_bytes: u64,
    pub awake_body_allocated_bytes: u64,
    pub warmup_steps: u32,
    pub samples: u32,
    pub gpu_collision_seconds: Option<Vec<f64>>,
    pub gpu_persistence_seconds: Option<Vec<f64>>,
    pub gpu_end_to_end_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub instrumented_solver_phases: Option<Vec<GpuSolverInstrumentedSampleV4>>,
    pub scheduler_work: Option<GpuSchedulerWorkVectorV1>,
    pub replay_exact: bool,
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

/// Measures one fixed selected hard/direct graph at an explicit body reserve.
///
/// Unlike interactive construction, this probe deliberately keeps the
/// scalable scheduling form at every reserve so 64/256/1K/10K rows differ
/// only in allocation capacity. GPU-written live/candidate/point counts still
/// own the encoded work, and correctness is replayed at the same reserve.
#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_sparse_capacity(
    scenario: &ScenarioV1,
    body_capacity: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    validate_selected_settings(scenario)?;
    if !matches!(body_capacity, 64 | 256 | 1_024 | 10_000) {
        return Err(
            "sparse-capacity benchmark requires a 64, 256, 1024, or 10000 body reserve".to_owned(),
        );
    }
    benchmark_gpu_generated_contacts_with_execution_scheduling_identity_and_body_capacity(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::SELECTED,
        ContactIdentityPolicyV1::SELECTED,
        body_capacity,
        crate::awake_work::SleepExecutionConfig::disabled(60),
    )
    .await
}

/// Measures one selected contact graph with the authored sleeping state made
/// effective. The caller owns the matched awake-fraction scenario family.
#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_sleeping_city(
    scenario: &ScenarioV1,
    body_capacity: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    validate_selected_settings(scenario)?;
    if warmup_steps
        .checked_add(samples)
        .is_none_or(|steps| steps >= 30)
    {
        return Err(
            "sleeping-city benchmark window must remain below the 60 Hz quiet duration".to_owned(),
        );
    }
    benchmark_gpu_generated_contacts_with_execution_scheduling_identity_and_body_capacity(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::SELECTED,
        ContactIdentityPolicyV1::SELECTED,
        body_capacity,
        crate::awake_work::SleepExecutionConfig::enabled(60),
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
    validate_selected_settings(scenario)?;
    let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
    benchmark_gpu_generated_contacts_with_execution_and_scheduling(
        scenario,
        warmup_steps,
        samples,
        gather_topology,
        GpuSolverExecutionV4::SELECTED,
        scheduling_policy,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_generated_contact_experiment(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    execution: GpuSolverExecutionV4,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    benchmark_gpu_generated_contacts_with_execution(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
        execution,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_generated_contact_scheduling_experiment(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    benchmark_gpu_generated_contacts_with_execution_and_scheduling(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
        execution,
        scheduling_policy,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_generated_contact_identity_experiment(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    identity_policy: ContactIdentityPolicyV1,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    let scheduling_policy = selected_scheduling_for_scenario(scenario)?;
    benchmark_gpu_generated_contacts_with_execution_scheduling_and_identity_policy(
        scenario,
        warmup_steps,
        samples,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        scheduling_policy,
        identity_policy,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
fn selected_scheduling_for_scenario(
    scenario: &ScenarioV1,
) -> Result<GpuSolverSchedulingPolicyV1, String> {
    let body_capacity = u32::try_from(scenario.bodies.len())
        .map_err(|_| "generated scenario body capacity exceeds u32".to_owned())?;
    Ok(GpuSolverSchedulingPolicyV1::selected_for_body_capacity(
        body_capacity,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
async fn benchmark_gpu_generated_contacts_with_execution(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    gather_topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    benchmark_gpu_generated_contacts_with_execution_and_scheduling(
        scenario,
        warmup_steps,
        samples,
        gather_topology,
        execution,
        GpuSolverSchedulingPolicyV1::STANDARD,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
async fn benchmark_gpu_generated_contacts_with_execution_and_scheduling(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    gather_topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    benchmark_gpu_generated_contacts_with_execution_scheduling_and_identity_policy(
        scenario,
        warmup_steps,
        samples,
        gather_topology,
        execution,
        scheduling_policy,
        ContactIdentityPolicyV1::SELECTED,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
async fn benchmark_gpu_generated_contacts_with_execution_scheduling_and_identity_policy(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    gather_topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    identity_policy: ContactIdentityPolicyV1,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    let body_capacity = u32::try_from(scenario.bodies.len())
        .map_err(|_| "generated benchmark body capacity exceeds u32".to_owned())?;
    benchmark_gpu_generated_contacts_with_execution_scheduling_identity_and_body_capacity(
        scenario,
        warmup_steps,
        samples,
        gather_topology,
        execution,
        scheduling_policy,
        identity_policy,
        body_capacity,
        crate::awake_work::SleepExecutionConfig::disabled(60),
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
async fn benchmark_gpu_generated_contacts_with_execution_scheduling_identity_and_body_capacity(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    gather_topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    identity_policy: ContactIdentityPolicyV1,
    body_capacity: u32,
    sleep_config: crate::awake_work::SleepExecutionConfig,
) -> Result<GpuGeneratedContactBenchmarkOutput, String> {
    if warmup_steps == 0 || samples == 0 {
        return Err("generated-contact benchmark requires non-zero warmup and samples".to_owned());
    }
    scenario.validate().map_err(|error| error.to_string())?;
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "generated benchmark body count exceeds u32".to_owned())?;
    if body_capacity < body_count {
        return Err(format!(
            "generated benchmark body capacity {body_capacity} cannot hold {body_count} live bodies"
        ));
    }
    let (correctness, replay) = {
        let runner = GpuGeneratedContactRunner::new().await?;
        let run = || {
            runner
                .run_with_topologies_execution_scheduling_identity_extended_body_capacity_and_sleep(
                    scenario,
                    1,
                    GpuContactPersistenceTopology::StablePairSlot,
                    gather_topology,
                    false,
                    false,
                    u32::MAX,
                    execution,
                    scheduling_policy,
                    identity_policy,
                    crate::GpuContactCoefficientPolicyV1::SELECTED,
                    crate::GpuSubstepContactPolicyV1::SELECTED,
                    crate::GpuRestitutionTimingPolicyV1::SELECTED,
                    body_capacity,
                    sleep_config,
                )
        };
        (run()?, run()?)
    };
    let replay_exact = correctness.bodies == replay.bodies
        && correctness.cache == replay.cache
        && correctness.pass_evidence == replay.pass_evidence
        && correctness.work == replay.work
        && correctness.awake_dynamic_body_count == replay.awake_dynamic_body_count
        && correctness.solver_body_count == replay.solver_body_count
        && correctness.live_contact_point_count == replay.live_contact_point_count
        && correctness.awake_contact_point_count == replay.awake_contact_point_count
        && correctness.capacities == replay.capacities
        && correctness.failure == replay.failure;
    if !replay_exact {
        return Err("generated-contact benchmark replay differed".to_owned());
    }
    if correctness.failure.is_some()
        || correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted)
    {
        return Err("generated-contact benchmark correctness/capacity gate failed".to_owned());
    }
    let scheduler_work = correctness
        .pass_evidence
        .scheduler_work_vector(correctness.solver_body_count)?;

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
    let storage = GpuBodyStorage::new_with_sleeping(
        &device,
        body_capacity,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
        sleep_config,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_capacity, &scenario.bodies)?;
    let graph =
        GpuGeneratedContactGraph::new_with_topologies_execution_scheduling_and_identity_policy(
            &device,
            &storage,
            scenario,
            GpuContactPersistenceTopology::StablePairSlot,
            gather_topology,
            false,
            false,
            u32::MAX,
            execution,
            scheduling_policy,
            identity_policy,
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
    let instrumented_color_occupancy = correctness
        .pass_evidence
        .coloring
        .as_ref()
        .map_or_else(Vec::new, |coloring| coloring.color_occupancy.clone());
    let instrumented_query_resources = if timestamps_available {
        let query_counts = graph.solver_instrumented_query_counts(&instrumented_color_occupancy)?;
        (0..samples)
            .map(|_| {
                query_counts
                    .iter()
                    .map(|&query_count| {
                        BenchmarkTimestampResources::new(
                            &device,
                            "physbox5_generated_contact_solver_subphase_queries",
                            query_count,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

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

    if timestamps_available {
        for resources in &instrumented_query_resources {
            storage.upload_reset_data(&queue, &reset);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll generated solver subphase reset: {error}"))?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("generated_contact_solver_subphase_sample"),
            });
            let query_sets = resources
                .iter()
                .map(|resources| &resources.query_set)
                .collect::<Vec<_>>();
            graph.encode_step_solver_instrumented(
                &mut encoder,
                true,
                &query_sets,
                &instrumented_color_occupancy,
            )?;
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll generated solver subphase sample: {error}"))?;
        }
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

    let instrumented_solver_phases = if timestamps_available {
        let period = queue.get_timestamp_period();
        Some(
            instrumented_query_resources
                .iter()
                .map(|resources| {
                    let mut values = Vec::new();
                    for resources in resources {
                        values.extend(resources.resolve_and_map(&device, &queue)?);
                    }
                    graph.decode_solver_instrumented_sample(
                        &values,
                        period,
                        &instrumented_color_occupancy,
                    )
                })
                .collect::<Result<Vec<_>, String>>()?,
        )
    } else {
        None
    };

    Ok(GpuGeneratedContactBenchmarkOutput {
        adapter: diagnostics,
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        body_count,
        body_capacity,
        body_allocated_bytes: storage.body_capacity_record().allocated_bytes,
        active_body_allocated_bytes: u64::from(
            body_capacity + crate::body_state::ACTIVE_BODY_STATE_WORDS,
        ) * size_of::<u32>() as u64,
        sleep_state_allocated_bytes: storage.sleep_state_capacity_record().allocated_bytes,
        awake_body_allocated_bytes: storage.awake_body_capacity_record().allocated_bytes,
        warmup_steps,
        samples,
        gpu_collision_seconds,
        gpu_persistence_seconds,
        gpu_end_to_end_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        instrumented_solver_phases,
        scheduler_work,
        replay_exact,
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[cfg(not(feature = "production-validation"))]
    use super::VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH;
    #[cfg(not(feature = "production-validation"))]
    use super::benchmark_gpu_sleeping_city;
    use super::{GpuGeneratedContactRunner, product_candidate_capacity};
    use crate::{
        GpuRestitutionTimingPolicyV1, GpuSubstepContactPolicyV1, P5_2SettlingFixtureKind,
        p5_2_centered_rectangle_stack_scenario, p5_2_settling_fixture_scenario,
    };
    #[cfg(not(feature = "production-validation"))]
    use crate::{
        GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1, P5_2SettlingReviewPolicy,
        p5_2_settling_review_scenario, p5_2_sleeping_city_scenario,
    };

    #[test]
    fn product_candidate_capacity_is_linear_and_bounded() {
        assert_eq!(product_candidate_capacity(2).unwrap(), 1);
        assert_eq!(product_candidate_capacity(100).unwrap(), 800);
        assert_eq!(product_candidate_capacity(10_000).unwrap(), 80_000);
        assert!(product_candidate_capacity(u32::MAX).is_err());
    }

    #[test]
    fn selected_settling_series_replays_exact_gpu_manifolds_and_cache() {
        let scenario =
            p5_2_settling_fixture_scenario(P5_2SettlingFixtureKind::BoxOnRestingBoxDrop, 0x6002, 4)
                .unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let first = runner
            .run_selected_observation_series(&scenario, 1, 4)
            .unwrap();
        let replay = runner
            .run_selected_observation_series(&scenario, 1, 4)
            .unwrap();
        let final_only = runner.run(&scenario, 4).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&replay).unwrap()
        );
        assert_eq!(first.snapshots.len(), 4);
        assert_eq!(first.snapshots[3].completed_steps, 4);
        assert_eq!(first.snapshots[3].bodies, final_only.bodies);
        assert_eq!(first.snapshots[3].manifolds, final_only.manifolds);
        assert_eq!(first.snapshots[3].cache, final_only.cache);
        assert_eq!(
            first.substep_contact_policy,
            GpuSubstepContactPolicyV1::SELECTED
        );
        assert_eq!(
            final_only.substep_contact_policy,
            GpuSubstepContactPolicyV1::SELECTED
        );
        assert_eq!(
            first.restitution_timing_policy,
            GpuRestitutionTimingPolicyV1::SELECTED
        );
        assert_eq!(
            final_only.restitution_timing_policy,
            GpuRestitutionTimingPolicyV1::SELECTED
        );
        assert!(first.snapshots.iter().all(|snapshot| {
            snapshot.failure.is_none()
                && snapshot.capacities.iter().all(|capacity| capacity.accepted)
        }));
    }

    #[test]
    fn selected_settling_series_is_exact_across_submission_batches() {
        let scenario = p5_2_centered_rectangle_stack_scenario(4, 4).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let one_step = runner
            .run_selected_observation_series_with_submission_batch(&scenario, 1, 4, 1)
            .unwrap();
        let four_steps = runner
            .run_selected_observation_series_with_submission_batch(&scenario, 1, 4, 4)
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&one_step).unwrap(),
            serde_json::to_vec(&four_steps).unwrap()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn selected_settling_series_replays_across_a_readback_flush_boundary() {
        let scenario = p5_2_settling_fixture_scenario(
            P5_2SettlingFixtureKind::NearFlatFloorDrop,
            0x6002,
            VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH + 1,
        )
        .unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let first = runner
            .run_selected_observation_series(
                &scenario,
                1,
                VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH + 1,
            )
            .unwrap();
        let replay = runner
            .run_selected_observation_series(
                &scenario,
                1,
                VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH + 1,
            )
            .unwrap();
        let final_only = runner
            .run(&scenario, VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH + 1)
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&replay).unwrap()
        );
        assert_eq!(
            first.snapshots.len(),
            (VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH + 1) as usize
        );
        assert_eq!(
            first.snapshots[VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH as usize - 1].completed_steps,
            VALIDATION_SNAPSHOTS_PER_READBACK_FLUSH
        );
        assert_eq!(first.snapshots.last().unwrap().bodies, final_only.bodies);
        assert_eq!(
            first.snapshots.last().unwrap().manifolds,
            final_only.manifolds
        );
        assert_eq!(first.snapshots.last().unwrap().cache, final_only.cache);
    }

    #[test]
    fn selected_settling_series_rejects_inexact_cadence() {
        let scenario =
            p5_2_settling_fixture_scenario(P5_2SettlingFixtureKind::NearFlatFloorDrop, 0x6002, 4)
                .unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        assert!(
            runner
                .run_selected_observation_series(&scenario, 3, 4)
                .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn static_late_partitions_contacts_and_matches_both_dispatch_forms_exactly() {
        let scenario =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::HardControl).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let selected = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
            )
            .unwrap();
        let per_color = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::STATIC_LATE,
            )
            .unwrap();
        let per_color_replay = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::STATIC_LATE,
            )
            .unwrap();
        let islands = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::STATIC_LATE_SMALL_WORLD,
            )
            .unwrap();

        assert_eq!(
            serde_json::to_vec(&per_color).unwrap(),
            serde_json::to_vec(&per_color_replay).unwrap()
        );
        assert_eq!(per_color.bodies, islands.bodies);
        assert_eq!(per_color.manifolds, islands.manifolds);
        assert_eq!(per_color.cache, islands.cache);
        assert_eq!(
            per_color.pass_evidence.contacts,
            islands.pass_evidence.contacts
        );
        assert_eq!(
            per_color
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors,
            islands
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors
        );
        assert!(per_color.failure.is_none());
        assert!(islands.failure.is_none());

        let coloring = per_color.pass_evidence.coloring.as_ref().unwrap();
        let mut static_contacts = 0;
        let mut dynamic_contacts = 0;
        for (contact, &color) in per_color
            .pass_evidence
            .contacts
            .iter()
            .zip(&coloring.point_colors)
        {
            if contact.point_in_manifold != 0 {
                continue;
            }
            let body_a = &per_color.bodies[contact.body_a as usize];
            let body_b = &per_color.bodies[contact.body_b as usize];
            let static_contact =
                body_a.inverse_mass.get() == 0.0 || body_b.inverse_mass.get() == 0.0;
            if static_contact {
                static_contacts += 1;
                assert!(color >= crate::GPU_STATIC_LATE_FIRST_COLOR);
            } else {
                dynamic_contacts += 1;
                assert!(color < crate::GPU_STATIC_LATE_FIRST_COLOR);
            }
        }
        assert!(static_contacts > 0);
        assert!(dynamic_contacts > 0);

        let selected_work = selected
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let static_late_work = per_color
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        assert_ne!(
            selected_work.assignment_digest,
            static_late_work.assignment_digest
        );
        assert_ne!(
            selected_work.compacted_order_digest,
            static_late_work.compacted_order_digest
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn fused_normal_tangent_replays_and_matches_both_dispatch_forms_exactly() {
        let scenario =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::HardControl).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let per_color = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT,
            )
            .unwrap();
        let per_color_replay = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT,
            )
            .unwrap();
        let islands = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT_SMALL_WORLD,
            )
            .unwrap();

        assert_eq!(
            serde_json::to_vec(&per_color).unwrap(),
            serde_json::to_vec(&per_color_replay).unwrap()
        );
        assert_eq!(per_color.bodies, islands.bodies);
        assert_eq!(per_color.manifolds, islands.manifolds);
        assert_eq!(per_color.cache, islands.cache);
        assert_eq!(
            per_color.pass_evidence.contacts,
            islands.pass_evidence.contacts
        );
        assert_eq!(
            per_color
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors,
            islands
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors
        );
        let per_color_work = per_color
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let island_work = islands
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        assert_eq!(
            per_color_work.assignment_digest,
            island_work.assignment_digest
        );
        assert_eq!(
            per_color_work.compacted_order_digest,
            island_work.compacted_order_digest
        );
        assert!(per_color.failure.is_none());
        assert!(islands.failure.is_none());

        let soft =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::SoftCandidate).unwrap();
        let error = runner
            .run_scheduling_experiment(
                &soft,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT,
            )
            .unwrap_err();
        assert!(error.contains("hard/direct"), "{error}");
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn static_stiff_hard_replays_and_matches_both_dispatch_forms_exactly() {
        let scenario =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::HardControl).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let selected = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
            )
            .unwrap();
        let run = |policy| {
            runner
                .run_coefficient_policy_experiment(
                    &scenario,
                    4,
                    GpuSolverExecutionV4::SELECTED,
                    policy,
                    crate::GpuContactCoefficientPolicyV1::StaticStiffHard,
                )
                .unwrap()
        };
        let per_color = run(GpuSolverSchedulingPolicyV1::SELECTED);
        let per_color_replay = run(GpuSolverSchedulingPolicyV1::SELECTED);
        let islands = run(GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);

        assert_eq!(
            serde_json::to_vec(&per_color).unwrap(),
            serde_json::to_vec(&per_color_replay).unwrap()
        );
        assert_ne!(selected.bodies, per_color.bodies);
        assert_eq!(per_color.bodies, islands.bodies);
        assert_eq!(per_color.manifolds, islands.manifolds);
        assert_eq!(per_color.cache, islands.cache);
        assert_eq!(
            per_color.pass_evidence.contacts,
            islands.pass_evidence.contacts
        );
        assert_eq!(
            per_color
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors,
            islands
                .pass_evidence
                .coloring
                .as_ref()
                .unwrap()
                .point_colors
        );
        let per_color_work = per_color
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let selected_work = selected
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let island_work = islands
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        assert_eq!(
            per_color_work.assignment_digest,
            island_work.assignment_digest
        );
        assert_eq!(
            selected_work.assignment_digest,
            per_color_work.assignment_digest
        );
        assert_eq!(
            per_color_work.compacted_order_digest,
            island_work.compacted_order_digest
        );
        assert_eq!(
            selected_work.compacted_order_digest,
            per_color_work.compacted_order_digest
        );
        assert_eq!(
            per_color.contact_coefficient_policy,
            crate::GpuContactCoefficientPolicyV1::StaticStiffHard
        );
        assert!(per_color.failure.is_none());
        assert!(islands.failure.is_none());

        let soft =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::SoftCandidate).unwrap();
        let error = runner
            .run_coefficient_policy_experiment(
                &soft,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                crate::GpuContactCoefficientPolicyV1::StaticStiffHard,
            )
            .unwrap_err();
        assert!(error.contains("hard contacts"), "{error}");
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn fixed_anchor_substeps_replay_and_match_both_dispatch_forms_exactly() {
        let scenario =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::HardControl).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let selected = runner
            .run_scheduling_experiment(
                &scenario,
                4,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
            )
            .unwrap();
        let run = |policy| {
            runner
                .run_solver_policy_experiment(
                    &scenario,
                    4,
                    GpuSolverExecutionV4::SELECTED,
                    policy,
                    crate::GpuContactCoefficientPolicyV1::SELECTED,
                    GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation,
                    GpuRestitutionTimingPolicyV1::SELECTED,
                )
                .unwrap()
        };
        let per_color = run(GpuSolverSchedulingPolicyV1::SELECTED);
        let replay = run(GpuSolverSchedulingPolicyV1::SELECTED);
        let islands = run(GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);

        assert_eq!(
            serde_json::to_vec(&per_color).unwrap(),
            serde_json::to_vec(&replay).unwrap()
        );
        assert_ne!(selected.bodies, per_color.bodies);
        assert_eq!(per_color.bodies, islands.bodies);
        assert_eq!(per_color.manifolds, islands.manifolds);
        assert_eq!(per_color.cache, islands.cache);
        assert_eq!(
            per_color.pass_evidence.contacts,
            islands.pass_evidence.contacts
        );
        let selected_work = selected
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let per_color_work = per_color
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let island_work = islands
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        assert_eq!(
            selected_work.assignment_digest,
            per_color_work.assignment_digest
        );
        assert_eq!(
            per_color_work.assignment_digest,
            island_work.assignment_digest
        );
        assert_eq!(
            per_color_work.compacted_order_digest,
            island_work.compacted_order_digest
        );
        assert_eq!(
            per_color.substep_contact_policy,
            GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation
        );
        assert_eq!(
            selected.dispatches_per_step - per_color.dispatches_per_step,
            scenario.settings.substeps - 1
        );
        assert!(per_color.failure.is_none());
        assert!(islands.failure.is_none());

        let soft =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::SoftCandidate).unwrap();
        let error = runner
            .run_solver_policy_experiment(
                &soft,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                crate::GpuContactCoefficientPolicyV1::SELECTED,
                GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation,
                GpuRestitutionTimingPolicyV1::SELECTED,
            )
            .unwrap_err();
        assert!(error.contains("hard/direct manifold-block i2"), "{error}");
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn retained_impulse_refresh_is_replayable_dispatch_exact_and_product_inert() {
        let scenario =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::HardControl).unwrap();
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let run = |scheduling_policy, substep_contact_policy| {
            runner
                .run_solver_policy_experiment(
                    &scenario,
                    4,
                    GpuSolverExecutionV4::SELECTED,
                    scheduling_policy,
                    crate::GpuContactCoefficientPolicyV1::SELECTED,
                    substep_contact_policy,
                    GpuRestitutionTimingPolicyV1::SELECTED,
                )
                .unwrap()
        };
        let selected = run(
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::SELECTED,
        );
        let candidate = run(
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses,
        );
        let replay = run(
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses,
        );
        let islands = run(
            GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses,
        );
        let product = runner.run(&scenario, 4).unwrap();

        assert_eq!(
            serde_json::to_vec(&candidate).unwrap(),
            serde_json::to_vec(&replay).unwrap()
        );
        assert_ne!(selected.bodies, candidate.bodies);
        assert_eq!(candidate.bodies, islands.bodies);
        assert_eq!(candidate.manifolds, islands.manifolds);
        assert_eq!(candidate.cache, islands.cache);
        assert_eq!(
            candidate.pass_evidence.contacts,
            islands.pass_evidence.contacts
        );
        assert_eq!(candidate.dispatches_per_step, selected.dispatches_per_step);
        let selected_work = selected
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let candidate_work = candidate
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        let island_work = islands
            .pass_evidence
            .scheduler_work_vector(scenario.bodies.len() as u32)
            .unwrap()
            .unwrap();
        assert_eq!(
            selected_work.assignment_digest,
            candidate_work.assignment_digest
        );
        assert_eq!(
            candidate_work.assignment_digest,
            island_work.assignment_digest
        );
        assert_eq!(
            selected_work.compacted_order_digest,
            candidate_work.compacted_order_digest
        );
        assert_eq!(
            candidate_work.compacted_order_digest,
            island_work.compacted_order_digest
        );
        assert_eq!(
            candidate.substep_contact_policy,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses
        );
        assert_eq!(
            product.substep_contact_policy,
            GpuSubstepContactPolicyV1::SELECTED
        );
        assert!(candidate.failure.is_none());
        assert!(islands.failure.is_none());

        let soft =
            p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::SoftCandidate).unwrap();
        let error = runner
            .run_solver_policy_experiment(
                &soft,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                crate::GpuContactCoefficientPolicyV1::SELECTED,
                GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses,
                GpuRestitutionTimingPolicyV1::SELECTED,
            )
            .unwrap_err();
        assert!(
            error.contains("selected hard/direct manifold-block i2"),
            "{error}"
        );

        let error = runner
            .run_solver_policy_experiment(
                &scenario,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                crate::GpuContactCoefficientPolicyV1::StaticStiffHard,
                GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses,
                GpuRestitutionTimingPolicyV1::SELECTED,
            )
            .unwrap_err();
        assert!(error.contains("standalone unpromoted"), "{error}");
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn restitution_after_substeps_is_stack_inert_replayable_and_dispatch_exact() {
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        for count in [4, 8, 16] {
            let scenario = p5_2_centered_rectangle_stack_scenario(count, 4).unwrap();
            let run = |scheduling_policy, restitution_timing_policy| {
                runner
                    .run_solver_policy_experiment(
                        &scenario,
                        4,
                        GpuSolverExecutionV4::SELECTED,
                        scheduling_policy,
                        crate::GpuContactCoefficientPolicyV1::SELECTED,
                        GpuSubstepContactPolicyV1::SELECTED,
                        restitution_timing_policy,
                    )
                    .unwrap()
            };
            let selected = run(
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuRestitutionTimingPolicyV1::SELECTED,
            );
            let per_color = run(
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuRestitutionTimingPolicyV1::AfterSubsteps,
            );
            let replay = run(
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuRestitutionTimingPolicyV1::AfterSubsteps,
            );
            let islands = run(
                GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
                GpuRestitutionTimingPolicyV1::AfterSubsteps,
            );
            let selected_islands = run(
                GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
                GpuRestitutionTimingPolicyV1::SELECTED,
            );

            assert_eq!(per_color.bodies, selected.bodies);
            assert_eq!(per_color.manifolds, selected.manifolds);
            assert_eq!(per_color.cache, selected.cache);
            assert_eq!(
                per_color.pass_evidence.contacts,
                selected.pass_evidence.contacts
            );
            assert_eq!(
                serde_json::to_vec(&per_color).unwrap(),
                serde_json::to_vec(&replay).unwrap()
            );
            assert_eq!(per_color.bodies, islands.bodies);
            assert_eq!(per_color.manifolds, islands.manifolds);
            assert_eq!(per_color.cache, islands.cache);
            assert_eq!(
                selected.dispatches_per_step - per_color.dispatches_per_step,
                (scenario.settings.substeps - 1) * 32
            );
            assert_eq!(
                selected_islands.dispatches_per_step - islands.dispatches_per_step,
                scenario.settings.substeps - 1
            );
            assert_eq!(
                per_color.restitution_timing_policy,
                GpuRestitutionTimingPolicyV1::AfterSubsteps
            );
            assert!(per_color.failure.is_none());
            assert!(islands.failure.is_none());
        }
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn restitution_after_substeps_changes_an_eligible_drop_and_rejects_composition() {
        let mut scenario =
            p5_2_settling_fixture_scenario(P5_2SettlingFixtureKind::NearFlatFloorDrop, 0x6002, 60)
                .unwrap();
        for body in &mut scenario.bodies {
            body.material.restitution = physbox_contracts::FiniteF32::new(0.5).unwrap();
        }
        let runner = pollster::block_on(GpuGeneratedContactRunner::new()).unwrap();
        let run = |timing| {
            runner
                .run_solver_policy_experiment(
                    &scenario,
                    60,
                    GpuSolverExecutionV4::SELECTED,
                    GpuSolverSchedulingPolicyV1::SELECTED,
                    crate::GpuContactCoefficientPolicyV1::SELECTED,
                    GpuSubstepContactPolicyV1::SELECTED,
                    timing,
                )
                .unwrap()
        };
        let selected = run(GpuRestitutionTimingPolicyV1::SELECTED);
        let final_only = run(GpuRestitutionTimingPolicyV1::AfterSubsteps);
        assert_ne!(selected.bodies, final_only.bodies);

        let error = runner
            .run_solver_policy_experiment(
                &scenario,
                1,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                crate::GpuContactCoefficientPolicyV1::SELECTED,
                GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation,
                GpuRestitutionTimingPolicyV1::AfterSubsteps,
            )
            .unwrap_err();
        assert!(
            error.contains("standalone hard/direct selected i2"),
            "{error}"
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn sleeping_city_benchmark_observes_the_authored_one_percent_work() {
        let scenario = p5_2_sleeping_city_scenario(1_000, 1).unwrap();
        let output = pollster::block_on(benchmark_gpu_sleeping_city(&scenario, 1_024, 1, 1))
            .expect("1%-awake sleeping-city benchmark");
        assert_eq!(output.correctness.awake_dynamic_body_count, 10);
        assert_eq!(output.correctness.solver_body_count, 10);
        assert!(output.correctness.live_contact_point_count > 0);
        assert!(
            output.correctness.awake_contact_point_count
                < output.correctness.live_contact_point_count
        );
        assert!(output.replay_exact);
    }
}
