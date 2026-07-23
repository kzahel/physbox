// Native laboratory runners consume additional diagnostic fields and helpers.
// They intentionally disappear from the product API when `solver-lab` is off.
#![cfg_attr(not(feature = "solver-lab"), allow(dead_code, unused_imports))]

use std::{mem::size_of, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CapacityRecordV1, ContactSoftnessPolicyV1, ScenarioV1, SolverSettingsV1, Validate,
};
#[cfg(not(target_arch = "wasm32"))]
use physbox_contracts::{canonical_hash, sha256_bytes};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;
use crate::body_state::{ACTIVE_BODY_DISPATCH_64_OFFSET, ACTIVE_BODY_WINNER_DISPATCH_OFFSET};
use crate::gpu_solver::GpuContributionEvidence;

const WORKGROUP_SIZE: u32 = 64;
const RADIX_WORKGROUP_SIZE: u32 = 256;
const PARAMETER_STRIDE: usize = 256;
const MAX_COLORS: u32 = 32;
pub const GPU_STATIC_LATE_COLOR_COUNT: u32 = 4;
pub const GPU_STATIC_LATE_FIRST_COLOR: u32 = MAX_COLORS - GPU_STATIC_LATE_COLOR_COUNT;
const COLOR_OFFSET_COUNT: u32 = MAX_COLORS + 1;
const COLOR_INDIRECT_WORDS: u32 = MAX_COLORS * 3;
const PARALLEL_COLOR_ROUNDS: u32 = MAX_COLORS;
const ISLAND_INDIRECT_WORDS: u32 = 3;
const SOLVER_WORK_STATE_WORDS: u32 = 16;
const ISLAND_INDIRECT_OFFSET: u64 = COLOR_INDIRECT_WORDS as u64 * size_of::<u32>() as u64;
const POINT_DISPATCH_64_OFFSET: u64 = 4 * size_of::<u32>() as u64;
const DOUBLE_POINT_DISPATCH_64_OFFSET: u64 = 10 * size_of::<u32>() as u64;
const DOUBLE_POINT_DISPATCH_256_OFFSET: u64 = 13 * size_of::<u32>() as u64;
const TOPOLOGY_DISPATCH_64_OFFSET: u64 = 17 * size_of::<u32>() as u64;
#[cfg(not(target_arch = "wasm32"))]
const MAX_INSTRUMENTED_QUERIES_PER_SET: u32 = 4_096;

pub const GPU_SOLVER_EXECUTION_VERSION: u32 = 4;
pub const GPU_SOLVER_SCHEDULING_POLICY_VERSION: u32 = 1;
pub const GPU_SOLVER_PASS_TRACE_VERSION: u32 = 1;
#[cfg(not(target_arch = "wasm32"))]
pub const GPU_SCHEDULER_WORK_VECTOR_VERSION: u32 = 2;

/// Independently derived engine-side coefficients for the portable GPU
/// comparator. The scalar authority specification lives in `physbox-lab` and
/// is intentionally not called from this path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuContactSoftnessCoefficientsV1 {
    pub bias_rate: f32,
    pub mass_scale: f32,
    pub impulse_scale: f32,
}

/// Laboratory-only control over how often the soft-contact feedback tuple is
/// applied inside the biased solve loop. Product construction always uses
/// `EveryBiasedIteration`; the alternate value is diagnostic evidence only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuSoftFeedbackPolicyV1 {
    EveryBiasedIteration,
    FirstBiasedIterationOnly,
}

/// Laboratory-only control over hard-contact coefficients. Product
/// construction always derives one uniform tuple from `SolverSettingsV1`.
/// `AuthorityTunableSoft` is the coefficient-exploration lane: it runs the
/// branch-robust execution with the scenario's own soft-contact tuple instead
/// of the frozen 30 Hz / damping 10 / static-x2 values, and is never
/// product-selectable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuContactCoefficientPolicyV1 {
    SettingsDerived,
    StaticStiffHard,
    AuthorityCappedSoft,
    AuthorityTunableSoft,
}

/// Laboratory-only control over contact preparation across solver substeps.
/// Product construction always refreshes anchors and effective masses while
/// reloading the outer-step persistence seed. The alternatives are the
/// conditional S9 E5 arm and the single scalar-qualified S10.4 retained-seed
/// port.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuSubstepContactPolicyV1 {
    RefreshAnchorsAndMass,
    RefreshAnchorsAndMassRetainImpulses,
    FixedAnchorAnalyticalSeparation,
    AuthorityFixedAnchorAnalyticalSeparation,
    AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
}

/// Laboratory-only control over restitution placement relative to substeps.
/// Product construction keeps the historical per-substep timing; the
/// authority-style alternative is the conditional S9 E6 hygiene arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuRestitutionTimingPolicyV1 {
    AfterEachSubstep,
    AfterSubsteps,
}

impl GpuRestitutionTimingPolicyV1 {
    pub const SELECTED: Self = Self::AfterEachSubstep;

    pub(crate) fn validate(
        self,
        settings: &SolverSettingsV1,
        execution: GpuSolverExecutionV4,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
        substep_contact_policy: GpuSubstepContactPolicyV1,
    ) -> Result<(), String> {
        if self == Self::AfterSubsteps
            && (settings.contact_softness.is_some()
                || settings.iterations != 2
                || execution != GpuSolverExecutionV4::SELECTED
                || normal_relaxation_policy != GpuNormalRelaxationPolicyV1::SELECTED
                || contact_coefficient_policy != GpuContactCoefficientPolicyV1::SELECTED
                || substep_contact_policy != GpuSubstepContactPolicyV1::SELECTED)
        {
            return Err(
                "restitution-after-substeps requires the standalone hard/direct selected i2 policy"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl GpuSubstepContactPolicyV1 {
    pub const SELECTED: Self = Self::RefreshAnchorsAndMass;

    const fn refreshes_anchors_and_mass(self) -> bool {
        matches!(
            self,
            Self::RefreshAnchorsAndMass | Self::RefreshAnchorsAndMassRetainImpulses
        )
    }

    pub(crate) const fn is_retained_refresh_candidate(self) -> bool {
        matches!(self, Self::RefreshAnchorsAndMassRetainImpulses)
    }

    pub(crate) fn validate(
        self,
        settings: &SolverSettingsV1,
        execution: GpuSolverExecutionV4,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
    ) -> Result<(), String> {
        if self == Self::FixedAnchorAnalyticalSeparation
            && (settings.contact_softness.is_some()
                || settings.iterations != 2
                || !execution.normal_solver.uses_block()
                || !execution.relaxation
                || normal_relaxation_policy != GpuNormalRelaxationPolicyV1::HardDirect)
        {
            return Err(
                "fixed-anchor analytical separation requires hard/direct manifold-block i2"
                    .to_owned(),
            );
        }
        if matches!(
            self,
            Self::AuthorityFixedAnchorAnalyticalSeparation
                | Self::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
        ) && (settings.contact_softness.is_none()
            || execution != GpuSolverExecutionV4::AUTHORITY_GPU_PROBE
            || normal_relaxation_policy != GpuNormalRelaxationPolicyV1::PointSequential)
        {
            return Err(
                "authority fixed-anchor separation requires an authorized S13-derived GPU probe"
                    .to_owned(),
            );
        }
        if self.is_retained_refresh_candidate()
            && (settings.contact_softness.is_some()
                || settings.iterations != 2
                || execution != GpuSolverExecutionV4::SELECTED
                || normal_relaxation_policy != GpuNormalRelaxationPolicyV1::HardDirect)
        {
            return Err(
                "retained-impulse refresh requires the selected hard/direct manifold-block i2 execution"
                    .to_owned(),
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_composition(
        self,
        settings: &SolverSettingsV1,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
        restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    ) -> Result<(), String> {
        self.validate(settings, execution, normal_relaxation_policy)?;
        if self.is_retained_refresh_candidate()
            && (scheduling_policy != GpuSolverSchedulingPolicyV1::SELECTED
                && scheduling_policy != GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD
                || soft_feedback_policy != GpuSoftFeedbackPolicyV1::SELECTED
                || contact_coefficient_policy != GpuContactCoefficientPolicyV1::SELECTED
                || restitution_timing_policy != GpuRestitutionTimingPolicyV1::SELECTED)
        {
            return Err(
                "retained-impulse refresh is a standalone unpromoted selected-policy port"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl GpuContactCoefficientPolicyV1 {
    pub const SELECTED: Self = Self::SettingsDerived;
    pub const STATIC_STIFF_BIAS_RATE: f32 = 0.4;

    pub(crate) fn validate(self, settings: &SolverSettingsV1) -> Result<(), String> {
        if self == Self::StaticStiffHard && settings.contact_softness.is_some() {
            return Err("static-stiff-hard requires hard contacts".to_owned());
        }
        if self == Self::AuthorityCappedSoft {
            let Some(softness) = settings.contact_softness else {
                return Err("authority capped coefficients require soft contact".to_owned());
            };
            if softness.contact_hertz.get() != 30.0
                || softness.damping_ratio.get() != 10.0
                || settings.substeps != 4
            {
                return Err(
                    "authority capped coefficients require the frozen 30 Hz / damping 10 / four-substep source"
                        .to_owned(),
                );
            }
        }
        if self == Self::AuthorityTunableSoft && settings.contact_softness.is_none() {
            return Err("tunable soft coefficients require soft contact".to_owned());
        }
        Ok(())
    }
}

/// Laboratory-only control over the normal relaxation recurrence. Product
/// construction always uses `HardDirect`; alternate values exist only for the
/// predeclared S6.5c factorial and the single scalar-qualified S6.5d port.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuNormalRelaxationPolicyV1 {
    HardDirect,
    OmitForSoftContacts,
    SoftCoefficients,
    PointSequential,
    CoupledEnergyProjection,
}

impl GpuNormalRelaxationPolicyV1 {
    pub const SELECTED: Self = Self::HardDirect;

    const fn emits_normal_pass(self) -> bool {
        !matches!(self, Self::OmitForSoftContacts)
    }

    const fn shader_code(self) -> u32 {
        match self {
            Self::HardDirect | Self::OmitForSoftContacts => 0,
            Self::SoftCoefficients => 1,
            Self::PointSequential => 2,
            Self::CoupledEnergyProjection => 3,
        }
    }

    fn validate(
        self,
        settings: &SolverSettingsV1,
        execution: GpuSolverExecutionV4,
    ) -> Result<(), String> {
        if self != Self::SELECTED && (settings.contact_softness.is_none() || !execution.relaxation)
        {
            return Err(
                "normal-relaxation comparators require soft contact and enabled relaxation"
                    .to_owned(),
            );
        }
        if self == Self::CoupledEnergyProjection && !execution.normal_solver.uses_block() {
            return Err(
                "coupled energy projection requires the manifold-block normal solver".to_owned(),
            );
        }
        Ok(())
    }
}

impl GpuSoftFeedbackPolicyV1 {
    pub const SELECTED: Self = Self::EveryBiasedIteration;

    const fn use_bias(self, iteration: u32) -> u32 {
        match self {
            Self::EveryBiasedIteration => 1,
            Self::FirstBiasedIterationOnly => {
                if iteration == 0 {
                    1
                } else {
                    0
                }
            }
        }
    }

    fn validate(self, settings: &SolverSettingsV1) -> Result<(), String> {
        if self == Self::FirstBiasedIterationOnly
            && (settings.contact_softness.is_none() || settings.iterations < 2)
        {
            return Err(
                "first-biased-iteration softness requires soft contact and at least two iterations"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

pub fn gpu_contact_softness_coefficients(
    policy: ContactSoftnessPolicyV1,
    substep_dt: f32,
    static_contact: bool,
) -> Result<GpuContactSoftnessCoefficientsV1, String> {
    policy.validate().map_err(|error| error.to_string())?;
    if !substep_dt.is_finite() || substep_dt <= 0.0 {
        return Err("GPU contact softness requires a finite positive substep dt".to_owned());
    }
    Ok(gpu_contact_softness_coefficients_validated(
        policy,
        substep_dt,
        static_contact,
    ))
}

fn gpu_contact_softness_coefficients_validated(
    policy: ContactSoftnessPolicyV1,
    substep_dt: f32,
    static_contact: bool,
) -> GpuContactSoftnessCoefficientsV1 {
    let contact_hertz = policy.contact_hertz.get().min(0.125 / substep_dt);
    let hertz = if static_contact {
        contact_hertz * policy.static_contact_hertz_multiplier.get()
    } else {
        contact_hertz
    };
    let omega = 2.0 * std::f32::consts::PI * hertz;
    let a1 = 2.0 * policy.damping_ratio.get() + substep_dt * omega;
    let a2 = substep_dt * omega * a1;
    let impulse_scale = 1.0 / (1.0 + a2);
    GpuContactSoftnessCoefficientsV1 {
        bias_rate: omega / a1,
        mass_scale: a2 * impulse_scale,
        impulse_scale,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuColorMaintenanceV1 {
    FullRebuild,
    FullRebuildParallelCompaction,
    FullRebuildParallelSpeculative,
    FullRebuildParallelSpeculativeStaticLate,
    StablePairIncremental,
    AuthoritySourceOrder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuColoredDispatchV1 {
    PerColorIndirect,
    WorkgroupPerIsland,
    PerColorIndirectFusedNormalTangent,
    WorkgroupPerIslandFusedNormalTangent,
}

impl GpuColoredDispatchV1 {
    pub const fn uses_workgroup_per_island(self) -> bool {
        matches!(
            self,
            Self::WorkgroupPerIsland | Self::WorkgroupPerIslandFusedNormalTangent
        )
    }

    pub const fn fuses_normal_tangent(self) -> bool {
        matches!(
            self,
            Self::PerColorIndirectFusedNormalTangent | Self::WorkgroupPerIslandFusedNormalTangent
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSolverSchedulingPolicyV1 {
    pub version: u32,
    pub color_maintenance: GpuColorMaintenanceV1,
    pub colored_dispatch: GpuColoredDispatchV1,
}

impl GpuSolverSchedulingPolicyV1 {
    pub const SMALL_WORLD_BODY_CAPACITY_MAX: u32 = 64;

    pub const SERIAL_CONTROL: Self = Self {
        version: GPU_SOLVER_SCHEDULING_POLICY_VERSION,
        color_maintenance: GpuColorMaintenanceV1::FullRebuild,
        colored_dispatch: GpuColoredDispatchV1::PerColorIndirect,
    };

    // Retained for the historical S5 factorials. Product construction uses
    // SELECTED so an old control cannot silently follow a later promotion.
    pub const STANDARD: Self = Self::SERIAL_CONTROL;

    pub const SELECTED: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::FullRebuildParallelSpeculative,
        ..Self::SERIAL_CONTROL
    };

    pub const SELECTED_SMALL_WORLD: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIsland,
        ..Self::SELECTED
    };

    pub const INCREMENTAL: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::StablePairIncremental,
        ..Self::SERIAL_CONTROL
    };

    pub const PARALLEL_COMPACTION: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::FullRebuildParallelCompaction,
        ..Self::SERIAL_CONTROL
    };

    pub const PARALLEL_COLORING: Self = Self::SELECTED;

    /// S9 E2 laboratory arm. Product construction never selects this policy.
    pub const STATIC_LATE: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate,
        ..Self::SELECTED
    };

    /// S9 E2 small-world dispatch-form control. Product construction never
    /// selects this policy.
    pub const STATIC_LATE_SMALL_WORLD: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIsland,
        ..Self::STATIC_LATE
    };

    /// S9 E3 laboratory arm. Product construction never selects this policy.
    pub const FUSED_NORMAL_TANGENT: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::PerColorIndirectFusedNormalTangent,
        ..Self::SELECTED
    };

    /// Exact validation-only schedule authorized by S13. Product construction
    /// never selects this policy.
    pub const AUTHORITY_GPU_PROBE: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::AuthoritySourceOrder,
        colored_dispatch: GpuColoredDispatchV1::PerColorIndirectFusedNormalTangent,
        ..Self::SERIAL_CONTROL
    };

    /// Body-capacity-bounded dispatch form of the same exact S13 policy.
    pub const AUTHORITY_GPU_PROBE_SMALL_WORLD: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIslandFusedNormalTangent,
        ..Self::AUTHORITY_GPU_PROBE
    };

    pub const fn authority_gpu_probe_for_body_capacity(body_capacity: u32) -> Self {
        if body_capacity <= Self::SMALL_WORLD_BODY_CAPACITY_MAX {
            Self::AUTHORITY_GPU_PROBE_SMALL_WORLD
        } else {
            Self::AUTHORITY_GPU_PROBE
        }
    }

    /// S9 E3 small-world dispatch-form control. Product construction never
    /// selects this policy.
    pub const FUSED_NORMAL_TANGENT_SMALL_WORLD: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIslandFusedNormalTangent,
        ..Self::FUSED_NORMAL_TANGENT
    };

    pub const WORKGROUP_PER_ISLAND: Self = Self {
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIsland,
        ..Self::SERIAL_CONTROL
    };

    pub const INCREMENTAL_WORKGROUP_PER_ISLAND: Self = Self {
        color_maintenance: GpuColorMaintenanceV1::StablePairIncremental,
        colored_dispatch: GpuColoredDispatchV1::WorkgroupPerIsland,
        ..Self::SERIAL_CONTROL
    };

    pub const fn selected_for_body_capacity(body_capacity: u32) -> Self {
        if body_capacity <= Self::SMALL_WORLD_BODY_CAPACITY_MAX {
            Self::SELECTED_SMALL_WORLD
        } else {
            Self::SELECTED
        }
    }

    pub const fn static_late_for_body_capacity(body_capacity: u32) -> Self {
        if body_capacity <= Self::SMALL_WORLD_BODY_CAPACITY_MAX {
            Self::STATIC_LATE_SMALL_WORLD
        } else {
            Self::STATIC_LATE
        }
    }

    pub const fn fused_normal_tangent_for_body_capacity(body_capacity: u32) -> Self {
        if body_capacity <= Self::SMALL_WORLD_BODY_CAPACITY_MAX {
            Self::FUSED_NORMAL_TANGENT_SMALL_WORLD
        } else {
            Self::FUSED_NORMAL_TANGENT
        }
    }

    const fn uses_parallel_compaction(self) -> bool {
        matches!(
            self.color_maintenance,
            GpuColorMaintenanceV1::FullRebuildParallelCompaction
                | GpuColorMaintenanceV1::FullRebuildParallelSpeculative
                | GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
        )
    }

    const fn uses_parallel_coloring(self) -> bool {
        matches!(
            self.color_maintenance,
            GpuColorMaintenanceV1::FullRebuildParallelSpeculative
                | GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
        )
    }

    pub const fn uses_static_late(self) -> bool {
        matches!(
            self.color_maintenance,
            GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate
        )
    }

    pub const fn uses_authority_source_order(self) -> bool {
        matches!(
            self.color_maintenance,
            GpuColorMaintenanceV1::AuthoritySourceOrder
        )
    }

    pub(crate) fn validate(self, execution: GpuSolverExecutionV4) -> Result<(), String> {
        if self.version != GPU_SOLVER_SCHEDULING_POLICY_VERSION {
            return Err(format!(
                "GPU solver scheduling policy version {} differs from {}",
                self.version, GPU_SOLVER_SCHEDULING_POLICY_VERSION
            ));
        }
        if self != Self::SERIAL_CONTROL
            && (execution.accumulation != GpuSolverAccumulationV4::ColoredInPlace
                || execution.color_schedule != GpuColorScheduleV4::CompactIndirect
                || execution.color_unit != GpuColorUnitV4::Manifold)
        {
            return Err(
                "solver scheduling comparators require compact manifold-colored execution"
                    .to_owned(),
            );
        }
        if self.colored_dispatch.fuses_normal_tangent()
            && (!execution.normal_solver.uses_manifold_kernel() || !execution.relaxation)
        {
            return Err(
                "fused normal/tangent scheduling requires block normals and relaxation".to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuNormalSolverV4 {
    PointJacobi,
    ScalarMassSplit,
    ManifoldBlock,
    BlockMassSplit,
    ManifoldPointSequential,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuSolverAccumulationV4 {
    JacobiGather,
    ColoredInPlace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuColorScheduleV4 {
    None,
    FixedFullGrid,
    CompactIndirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuColorUnitV4 {
    Point,
    Manifold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuColorOrderV4 {
    Forward,
    Alternating,
    AuthorityOverflowFirst,
}

impl GpuNormalSolverV4 {
    const fn shader_code(self) -> u32 {
        match self {
            Self::PointJacobi => 0,
            Self::ScalarMassSplit => 1,
            Self::ManifoldBlock => 2,
            Self::BlockMassSplit => 3,
            Self::ManifoldPointSequential => 4,
        }
    }

    pub const fn uses_block(self) -> bool {
        matches!(self, Self::ManifoldBlock | Self::BlockMassSplit)
    }

    pub const fn uses_manifold_kernel(self) -> bool {
        matches!(
            self,
            Self::ManifoldBlock | Self::BlockMassSplit | Self::ManifoldPointSequential
        )
    }

    const fn uses_manifold_mass_split(self) -> bool {
        matches!(self, Self::BlockMassSplit)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSolverExecutionV4 {
    pub version: u32,
    pub accumulation: GpuSolverAccumulationV4,
    pub normal_solver: GpuNormalSolverV4,
    pub color_schedule: GpuColorScheduleV4,
    pub color_unit: GpuColorUnitV4,
    pub color_order: GpuColorOrderV4,
    pub warm_start: bool,
    pub relaxation: bool,
}

impl GpuSolverExecutionV4 {
    pub const SELECTED: Self = Self {
        version: GPU_SOLVER_EXECUTION_VERSION,
        accumulation: GpuSolverAccumulationV4::ColoredInPlace,
        normal_solver: GpuNormalSolverV4::ManifoldBlock,
        color_schedule: GpuColorScheduleV4::CompactIndirect,
        color_unit: GpuColorUnitV4::Manifold,
        color_order: GpuColorOrderV4::Forward,
        warm_start: true,
        relaxation: true,
    };

    /// S10 diagnostic execution that differs from the selected product only
    /// by suppressing application of the persisted contact impulse seed.
    /// Product construction never selects this execution.
    pub const WARM_START_OFF_DIAGNOSTIC: Self = Self {
        warm_start: false,
        ..Self::SELECTED
    };

    /// Exact validation-only X1-X4 execution authorized by S13.
    pub const AUTHORITY_GPU_PROBE: Self = Self {
        version: GPU_SOLVER_EXECUTION_VERSION,
        accumulation: GpuSolverAccumulationV4::ColoredInPlace,
        normal_solver: GpuNormalSolverV4::ManifoldPointSequential,
        color_schedule: GpuColorScheduleV4::CompactIndirect,
        color_unit: GpuColorUnitV4::Manifold,
        color_order: GpuColorOrderV4::AuthorityOverflowFirst,
        warm_start: true,
        relaxation: true,
    };

    pub const fn experiment(warm_start: bool, relaxation: bool) -> Self {
        Self {
            version: GPU_SOLVER_EXECUTION_VERSION,
            accumulation: GpuSolverAccumulationV4::JacobiGather,
            normal_solver: GpuNormalSolverV4::PointJacobi,
            color_schedule: GpuColorScheduleV4::None,
            color_unit: GpuColorUnitV4::Point,
            color_order: GpuColorOrderV4::Forward,
            warm_start,
            relaxation,
        }
    }

    pub const fn normal_experiment(
        normal_solver: GpuNormalSolverV4,
        warm_start: bool,
        relaxation: bool,
    ) -> Self {
        Self {
            version: GPU_SOLVER_EXECUTION_VERSION,
            accumulation: GpuSolverAccumulationV4::JacobiGather,
            normal_solver,
            color_schedule: GpuColorScheduleV4::None,
            color_unit: GpuColorUnitV4::Point,
            color_order: GpuColorOrderV4::Forward,
            warm_start,
            relaxation,
        }
    }

    pub const fn colored_experiment(warm_start: bool, relaxation: bool) -> Self {
        Self {
            version: GPU_SOLVER_EXECUTION_VERSION,
            accumulation: GpuSolverAccumulationV4::ColoredInPlace,
            normal_solver: GpuNormalSolverV4::PointJacobi,
            color_schedule: GpuColorScheduleV4::FixedFullGrid,
            color_unit: GpuColorUnitV4::Point,
            color_order: GpuColorOrderV4::Forward,
            warm_start,
            relaxation,
        }
    }

    pub const fn compact_colored_experiment(warm_start: bool, relaxation: bool) -> Self {
        Self {
            color_schedule: GpuColorScheduleV4::CompactIndirect,
            ..Self::colored_experiment(warm_start, relaxation)
        }
    }

    pub const fn manifold_colored_block_experiment(warm_start: bool, relaxation: bool) -> Self {
        Self {
            version: GPU_SOLVER_EXECUTION_VERSION,
            accumulation: GpuSolverAccumulationV4::ColoredInPlace,
            normal_solver: GpuNormalSolverV4::ManifoldBlock,
            color_schedule: GpuColorScheduleV4::CompactIndirect,
            color_unit: GpuColorUnitV4::Manifold,
            color_order: GpuColorOrderV4::Forward,
            warm_start,
            relaxation,
        }
    }

    pub const fn alternating_manifold_colored_block_experiment(
        warm_start: bool,
        relaxation: bool,
    ) -> Self {
        Self {
            color_order: GpuColorOrderV4::Alternating,
            ..Self::manifold_colored_block_experiment(warm_start, relaxation)
        }
    }

    const fn uses_colored_in_place(self) -> bool {
        matches!(self.accumulation, GpuSolverAccumulationV4::ColoredInPlace)
    }

    pub(crate) fn validate(self) -> Result<(), String> {
        if self.version != GPU_SOLVER_EXECUTION_VERSION {
            return Err(format!(
                "GPU solver execution version {} differs from {}",
                self.version, GPU_SOLVER_EXECUTION_VERSION
            ));
        }
        if self.uses_colored_in_place()
            && !matches!(
                (self.normal_solver, self.color_unit),
                (GpuNormalSolverV4::PointJacobi, GpuColorUnitV4::Point)
                    | (GpuNormalSolverV4::ManifoldBlock, GpuColorUnitV4::Manifold)
                    | (
                        GpuNormalSolverV4::ManifoldPointSequential,
                        GpuColorUnitV4::Manifold
                    )
            )
        {
            return Err("colored solver and color unit disagree".to_owned());
        }
        match (self.accumulation, self.color_schedule) {
            (GpuSolverAccumulationV4::JacobiGather, GpuColorScheduleV4::None)
            | (
                GpuSolverAccumulationV4::ColoredInPlace,
                GpuColorScheduleV4::FixedFullGrid | GpuColorScheduleV4::CompactIndirect,
            ) => {}
            _ => return Err("GPU solver accumulation and color schedule disagree".to_owned()),
        }
        if self.color_unit == GpuColorUnitV4::Manifold
            && self.color_schedule != GpuColorScheduleV4::CompactIndirect
        {
            return Err("manifold coloring requires the compact indirect schedule".to_owned());
        }
        if self.color_order == GpuColorOrderV4::Alternating
            && self.color_schedule != GpuColorScheduleV4::CompactIndirect
        {
            return Err("alternating coloring requires the compact indirect schedule".to_owned());
        }
        if self.color_order == GpuColorOrderV4::AuthorityOverflowFirst
            && self != Self::AUTHORITY_GPU_PROBE
        {
            return Err(
                "authority overflow-first order requires the exact S13 GPU probe".to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuGatherTopology {
    CanonicalBodyScan,
    StableBodyEdge,
}

impl GpuGatherTopology {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CanonicalBodyScan => "canonical-body-scan",
            Self::StableBodyEdge => "stable-body-edge",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuGatherTopologyEvidence {
    pub topology: GpuGatherTopology,
    pub edge_build_permuted: bool,
    pub dispatches: u32,
    pub allocated_bytes: u64,
    pub point_capacity: u32,
    pub edge_capacity: u32,
    pub radix_passes: u32,
    pub manifold_count_dispatches: u32,
    pub coloring_dispatches: u32,
    pub color_compaction_dispatches: u32,
    pub island_construction_dispatches: u32,
    pub canonical_scan_attempts: u64,
    pub capacities: Vec<CapacityRecordV1>,
}

impl GpuGatherTopologyEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.point_capacity == 0
            || self.edge_capacity != self.point_capacity.saturating_mul(2)
            || self.allocated_bytes == 0
            || self
                .capacities
                .iter()
                .map(|capacity| capacity.allocated_bytes)
                .sum::<u64>()
                != self.allocated_bytes
        {
            return Err("gather topology has invalid capacity accounting".to_owned());
        }
        for pair in self.capacities.windows(2) {
            if pair[0].class >= pair[1].class {
                return Err("gather topology capacities are not sorted and unique".to_owned());
            }
        }
        for capacity in &self.capacities {
            physbox_contracts::Validate::validate(capacity).map_err(|error| error.to_string())?;
        }
        match self.topology {
            GpuGatherTopology::CanonicalBodyScan => {
                if self.edge_build_permuted
                    || self.dispatches
                        != 4 + self.manifold_count_dispatches
                            + self.coloring_dispatches
                            + self.color_compaction_dispatches
                            + self.island_construction_dispatches
                    || self.radix_passes != 0
                    || self.manifold_count_dispatches > 1
                    || !matches!(self.coloring_dispatches, 0 | 1 | 98)
                    || !matches!(self.color_compaction_dispatches, 0 | 1 | 4)
                    || self.island_construction_dispatches > 1
                    || self.canonical_scan_attempts == 0
                {
                    return Err("canonical gather topology has invalid work accounting".to_owned());
                }
            }
            GpuGatherTopology::StableBodyEdge => {
                if self.dispatches
                    != 3 + self.radix_passes * 3
                        + self.manifold_count_dispatches
                        + self.coloring_dispatches
                        + self.color_compaction_dispatches
                        + self.island_construction_dispatches
                    || self.radix_passes == 0
                    || self.manifold_count_dispatches > 1
                    || !matches!(self.coloring_dispatches, 0 | 1 | 98)
                    || !matches!(self.color_compaction_dispatches, 0 | 1 | 4)
                    || self.island_construction_dispatches > 1
                    || self.canonical_scan_attempts != 0
                {
                    return Err("stable body-edge topology has invalid work accounting".to_owned());
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoredGatherBenchmarkOutput {
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub body_count: u32,
    pub manifold_count: u32,
    pub contact_points: u32,
    pub warmup_steps: u32,
    pub samples: u32,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub contact_coefficient_policy: GpuContactCoefficientPolicyV1,
    pub scheduler_work: Option<GpuSchedulerWorkVectorV1>,
    pub topology: GpuGatherTopologyEvidence,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub gpu_total_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub instrumented_index_build_seconds: Option<Vec<f64>>,
    pub instrumented_offset_construction_seconds: Option<Vec<f64>>,
    pub instrumented_gather_seconds: Option<Vec<f64>>,
    pub instrumented_solve_seconds: Option<Vec<f64>>,
    pub instrumented_solver_phases: Option<Vec<GpuSolverInstrumentedSampleV4>>,
    pub timestamp_period_nanoseconds: Option<f32>,
    pub replay_exact: bool,
    pub result_digest: String,
    pub capacities: Vec<CapacityRecordV1>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthorityProbeCapacityDispositionV1 {
    pub capacity_class: &'static str,
    pub capacity: u32,
    pub attempted: u32,
    pub accepted: u32,
    pub committed: u32,
    pub overflowed: bool,
    pub rejected: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAuthoredAuthorityProbeOutputV1 {
    pub adapter: crate::AdapterDiagnostics,
    pub source_scenario_hash: String,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub pass_descriptors: Vec<GpuSolverPassDescriptorV1>,
    pub bodies: Vec<physbox_contracts::BodyStateV1>,
    pub pass_evidence: GpuFixedFloorPassEvidence,
    pub capacities: Vec<CapacityRecordV1>,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub replay_exact: bool,
    pub result_digest: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuS13MeasuredConfigurationV1 {
    pub configuration: &'static str,
    pub solver_execution: GpuSolverExecutionV4,
    pub solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub topology: GpuGatherTopologyEvidence,
    pub pass_descriptors: Vec<GpuSolverPassDescriptorV1>,
    pub coloring: GpuColoringEvidenceV4,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub gpu_complete_step_seconds: Vec<f64>,
    pub host_complete_step_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub capacities: Vec<CapacityRecordV1>,
    pub replay_exact: bool,
    pub result_digest: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuS13AuthorityProbeBenchmarkV1 {
    pub adapter: crate::AdapterDiagnostics,
    pub candidate_source_scenario_hash: String,
    pub selected_projected_source_scenario_hash: String,
    pub body_count: u32,
    pub manifold_count: u32,
    pub contact_points: u32,
    pub warmup_samples: u32,
    pub retained_samples: u32,
    pub invocation_order: Vec<[&'static str; 2]>,
    pub timestamp_period_nanoseconds: f32,
    pub candidate_specific_transform_bytes: u64,
    pub candidate_allocation_delta_bytes: i64,
    pub selected: GpuS13MeasuredConfigurationV1,
    pub candidate: GpuS13MeasuredConfigurationV1,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct BenchmarkTimestampResources {
    pub(crate) query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    count: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl BenchmarkTimestampResources {
    pub(crate) fn new(device: &wgpu::Device, label: &'static str, count: u32) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some(label),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let byte_count = u64::from(count) * size_of::<u64>() as u64;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: byte_count,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: byte_count,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            query_set,
            resolve,
            readback,
            count,
        }
    }

    pub(crate) fn resolve_and_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Vec<u64>, String> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_query_copy"),
        });
        encoder.resolve_query_set(&self.query_set, 0..self.count, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve,
            0,
            &self.readback,
            0,
            u64::from(self.count) * size_of::<u64>() as u64,
        );
        queue.submit([encoder.finish()]);
        crate::collision_candidates::map_values::<u64>(device, &self.readback, self.count as usize)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct PointInputRaw {
    ids: [u32; 4],
    normal: [f32; 2],
    local_anchor_a: [f32; 2],
    local_anchor_b: [f32; 2],
    padding0: [f32; 2],
    material: [f32; 4],
    cache: [f32; 2],
    padding1: [f32; 2],
}

impl PointInputRaw {
    pub(crate) const fn component_ids(&self) -> [u32; 4] {
        self.ids
    }

    pub(crate) fn component_generations(&self) -> [u32; 2] {
        [self.padding1[0].to_bits(), self.padding1[1].to_bits()]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct PreparedPointRaw {
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFixedFloorContactEvidence {
    pub point_index: u32,
    pub valid: bool,
    pub body_a: u32,
    pub body_b: u32,
    pub manifold_index: u32,
    pub point_in_manifold: u32,
    pub normal: physbox_contracts::Vec2,
    pub tangent: physbox_contracts::Vec2,
    pub anchor_a: physbox_contracts::Vec2,
    pub anchor_b: physbox_contracts::Vec2,
    pub initial_normal_velocity: f32,
    pub normal_effective_mass: f32,
    pub tangent_effective_mass: f32,
    pub separation: f32,
    pub friction: f32,
    pub restitution: f32,
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub total_normal_impulse: f32,
    pub body_a_normal_mass_incidence: u32,
    pub body_b_normal_mass_incidence: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBlockActiveSetV2 {
    Both,
    First,
    Second,
    None,
    Scalar,
    Invalid,
}

impl GpuBlockActiveSetV2 {
    fn from_shader_code(code: u32) -> Self {
        match code {
            0 => Self::Both,
            1 => Self::First,
            2 => Self::Second,
            3 => Self::None,
            4 => Self::Scalar,
            _ => Self::Invalid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuManifoldBlockEvidenceV2 {
    pub manifold_index: u32,
    pub body_a: u32,
    pub body_b: u32,
    pub point_count: u32,
    pub effective_mass_matrix: [f32; 4],
    pub active_set: GpuBlockActiveSetV2,
    pub body_a_normal_mass_incidence: u32,
    pub body_b_normal_mass_incidence: u32,
    pub normal_impulses: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuColoringEvidenceV4 {
    pub schedule: GpuColorScheduleV4,
    pub unit: GpuColorUnitV4,
    pub order: GpuColorOrderV4,
    pub max_colors: u32,
    pub color_count: u32,
    pub point_colors: Vec<u32>,
    pub color_occupancy: Vec<u32>,
    pub compacted_point_indices: Vec<u32>,
    pub indirect_workgroups: Vec<[u32; 3]>,
    pub encoded_color_passes: u32,
    pub active_color_passes: u32,
    pub empty_indirect_launches: u32,
    pub scheduled_point_invocations: u64,
    pub scheduling_policy: GpuSolverSchedulingPolicyV1,
    pub retained_color_units: u32,
    pub new_color_units: u32,
    pub recolored_color_units: u32,
    pub removed_color_units: u32,
    pub parallel_round_limit: u32,
    pub parallel_rounds_used: u32,
    pub parallel_conflict_retries: u32,
    pub parallel_uncolored_units: u32,
    pub island_count: u32,
    pub island_body_occupancy: Vec<u32>,
    pub island_manifold_occupancy: Vec<u32>,
    pub overflowed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuFixedFloorPassEvidence {
    pub contacts: Vec<GpuFixedFloorContactEvidence>,
    pub contributions: Vec<GpuContributionEvidence>,
    pub manifold_blocks: Vec<GpuManifoldBlockEvidenceV2>,
    pub coloring: Option<GpuColoringEvidenceV4>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSchedulerWorkVectorV1 {
    pub version: u32,
    pub body_slots: u32,
    pub contact_point_slots: u32,
    pub live_contact_points: u32,
    pub color_units: u32,
    pub maximum_colors: u32,
    pub active_colors: u32,
    pub color_occupancy: Vec<u32>,
    pub body_mask_slots_cleared: u32,
    pub color_count_slots_cleared: u32,
    pub assignment_point_slots_visited: u32,
    pub conflict_winner_slots_visited: u64,
    pub parallel_round_limit: u32,
    pub parallel_rounds_used: u32,
    pub conflict_retries: u32,
    pub uncolored_units: u32,
    pub prefix_color_slots_visited: u32,
    pub compaction_point_slots_visited: u32,
    pub compacted_color_units: u32,
    pub encoded_color_passes: u32,
    pub active_color_passes: u32,
    pub empty_indirect_launches: u32,
    pub scheduled_color_unit_invocations: u64,
    pub assignment_digest: String,
    pub compacted_order_digest: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuFixedFloorPassEvidence {
    pub fn scheduler_work_vector(
        &self,
        body_slots: u32,
    ) -> Result<Option<GpuSchedulerWorkVectorV1>, String> {
        let Some(coloring) = &self.coloring else {
            return Ok(None);
        };
        let contact_point_slots = u32::try_from(self.contacts.len())
            .map_err(|_| "scheduler point-slot count exceeds u32".to_owned())?;
        let live_contact_points =
            u32::try_from(self.contacts.iter().filter(|contact| contact.valid).count())
                .map_err(|_| "scheduler live-point count exceeds u32".to_owned())?;
        let color_units = coloring
            .color_occupancy
            .iter()
            .try_fold(0_u32, |sum, value| {
                sum.checked_add(*value)
                    .ok_or_else(|| "scheduler color-unit count overflowed".to_owned())
            })?;
        let compacted_color_units = u32::try_from(coloring.compacted_point_indices.len())
            .map_err(|_| "scheduler compacted-unit count exceeds u32".to_owned())?;
        let parallel_compaction = coloring.scheduling_policy.uses_parallel_compaction();
        let parallel_coloring = coloring.scheduling_policy.uses_parallel_coloring();
        let color_blocks = contact_point_slots.div_ceil(WORKGROUP_SIZE);
        let prefix_color_slots_visited = if parallel_compaction {
            color_blocks
                .checked_mul(coloring.max_colors)
                .and_then(|slots| slots.checked_add(coloring.max_colors))
                .ok_or_else(|| "scheduler prefix work overflowed".to_owned())?
        } else {
            coloring.max_colors
        };
        let compaction_point_slots_visited =
            if coloring.schedule == GpuColorScheduleV4::CompactIndirect {
                contact_point_slots
                    .checked_mul(if parallel_compaction { 2 } else { 1 })
                    .ok_or_else(|| "scheduler compaction work overflowed".to_owned())?
            } else {
                0
            };
        let assignment_point_slots_visited = contact_point_slots
            .checked_mul(if parallel_coloring {
                2 + PARALLEL_COLOR_ROUNDS * 2
            } else {
                1
            })
            .ok_or_else(|| "scheduler assignment work overflowed".to_owned())?;
        let conflict_winner_slots_visited = if parallel_coloring {
            u64::from(body_slots)
                .checked_mul(u64::from(coloring.max_colors))
                .and_then(|slots| slots.checked_mul(u64::from(PARALLEL_COLOR_ROUNDS)))
                .ok_or_else(|| "scheduler conflict-winner work overflowed".to_owned())?
        } else {
            0
        };
        if coloring.overflowed {
            return Err("scheduler work vector rejected solver-colors overflow".to_owned());
        }
        if coloring.max_colors == 0
            || coloring.color_count > coloring.max_colors
            || coloring.color_occupancy.len() != coloring.color_count as usize
            || coloring.parallel_uncolored_units != 0
            || (coloring.schedule == GpuColorScheduleV4::CompactIndirect
                && compacted_color_units != color_units)
        {
            return Err("scheduler work vector requires clean coherent coloring".to_owned());
        }
        Ok(Some(GpuSchedulerWorkVectorV1 {
            version: GPU_SCHEDULER_WORK_VECTOR_VERSION,
            body_slots,
            contact_point_slots,
            live_contact_points,
            color_units,
            maximum_colors: coloring.max_colors,
            active_colors: coloring.color_count,
            color_occupancy: coloring.color_occupancy.clone(),
            body_mask_slots_cleared: body_slots,
            color_count_slots_cleared: coloring.max_colors,
            assignment_point_slots_visited,
            conflict_winner_slots_visited,
            parallel_round_limit: coloring.parallel_round_limit,
            parallel_rounds_used: coloring.parallel_rounds_used,
            conflict_retries: coloring.parallel_conflict_retries,
            uncolored_units: coloring.parallel_uncolored_units,
            prefix_color_slots_visited,
            compaction_point_slots_visited,
            compacted_color_units,
            encoded_color_passes: coloring.encoded_color_passes,
            active_color_passes: coloring.active_color_passes,
            empty_indirect_launches: coloring.empty_indirect_launches,
            scheduled_color_unit_invocations: coloring.scheduled_point_invocations,
            assignment_digest: sha256_bytes(bytemuck::cast_slice(&coloring.point_colors)),
            compacted_order_digest: sha256_bytes(bytemuck::cast_slice(
                &coloring.compacted_point_indices,
            )),
        }))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSolverInstrumentedSampleV4 {
    pub color_construction_seconds: f64,
    pub color_compaction_seconds: f64,
    pub active_color_solve_seconds: f64,
    pub empty_color_solve_seconds: f64,
    pub other_solver_seconds: f64,
    pub total_instrumented_seconds: f64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(crate) struct FixedFloorEvidenceLayout {
    pub(crate) point_count: u32,
    pub(crate) body_count: u32,
    pub(crate) color_schedule: GpuColorScheduleV4,
    pub(crate) color_unit: GpuColorUnitV4,
    pub(crate) color_order: GpuColorOrderV4,
    pub(crate) scheduling_policy: GpuSolverSchedulingPolicyV1,
    topology_layout: TopologyLayout,
    pub(crate) prepared_bytes: u64,
    pub(crate) contribution_bytes: u64,
    pub(crate) color_topology_bytes: u64,
    pub(crate) color_indirect_bytes: u64,
    pub(crate) coloring_bytes: u64,
    pub(crate) total_bytes: u64,
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SolverWorkParamsRaw {
    values: [u32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuSolverPassKindV1 {
    IntegrateVelocity,
    PrepareContacts,
    RefreshContacts,
    WarmStart,
    NormalBiased,
    TangentBiased,
    IntegratePosition,
    NormalRelaxation,
    TangentRelaxation,
    Restitution,
    Gather,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSolverPassParametersV1 {
    pub floats0: [f32; 4],
    pub floats1: [f32; 4],
    pub floats2: [f32; 4],
    pub floats3: [f32; 4],
    pub uints0: [u32; 4],
    pub uints1: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSolverPassDescriptorV1 {
    pub version: u32,
    pub first_operation_index: u32,
    pub operation_index: u32,
    pub substep: u32,
    pub kind: GpuSolverPassKindV1,
    pub iteration: u32,
    pub first_color: Option<u32>,
    pub last_color: Option<u32>,
    pub color_passes: u32,
    pub parameters: GpuSolverPassParametersV1,
}

#[derive(Clone, Copy)]
enum PipelineKind {
    TopologyClear,
    TopologyCount,
    ManifoldCount,
    TopologyPrefix,
    TopologyScatter,
    EdgeBuild,
    EdgeRadixHistograms,
    EdgeRadixPrefix,
    EdgeRadixScatter,
    EdgeRanges,
    Color,
    ColorConflictClear,
    ColorPropose,
    ColorResolve,
    ColorFinalize,
    ColorCompact,
    ColorBlockCount,
    ColorBlockPrefix,
    ColorRangePrefix,
    ColorBlockScatter,
    IslandBuild,
    IntegrateVelocity,
    Prepare,
    Refresh,
    Warm,
    Normal,
    NormalBlock,
    Tangent,
    Restitution,
    IntegratePosition,
    Gather,
    Colored,
    AuthorityOverflow,
    IslandColored,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
enum InstrumentedPhase {
    IndexBuild,
    OffsetConstruction,
    Gather,
    Solve,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Eq, PartialEq)]
enum SolverInstrumentedPhase {
    ColorConstruction,
    ColorCompaction,
    ActiveColorSolve,
    EmptyColorSolve,
    OtherSolver,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct SolverInstrumentedGroup {
    phase: SolverInstrumentedPhase,
    start: usize,
    end: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl PipelineKind {
    const fn instrumented_phase(self) -> InstrumentedPhase {
        match self {
            Self::TopologyClear
            | Self::TopologyCount
            | Self::ManifoldCount
            | Self::TopologyScatter
            | Self::EdgeBuild
            | Self::EdgeRadixHistograms
            | Self::EdgeRadixScatter
            | Self::Color
            | Self::ColorConflictClear
            | Self::ColorPropose
            | Self::ColorResolve
            | Self::ColorFinalize
            | Self::ColorCompact
            | Self::ColorBlockCount
            | Self::ColorBlockPrefix
            | Self::ColorRangePrefix
            | Self::ColorBlockScatter
            | Self::IslandBuild => InstrumentedPhase::IndexBuild,
            Self::TopologyPrefix | Self::EdgeRadixPrefix | Self::EdgeRanges => {
                InstrumentedPhase::OffsetConstruction
            }
            Self::Gather => InstrumentedPhase::Gather,
            Self::IntegrateVelocity
            | Self::Prepare
            | Self::Refresh
            | Self::Warm
            | Self::Normal
            | Self::NormalBlock
            | Self::Tangent
            | Self::Restitution
            | Self::IntegratePosition
            | Self::Colored
            | Self::AuthorityOverflow
            | Self::IslandColored => InstrumentedPhase::Solve,
        }
    }
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
    indirect_offset: Option<u64>,
    parameter_index: u32,
    color: Option<u32>,
}

struct DynamicSolverWork {
    counts_pipeline: wgpu::ComputePipeline,
    bodies_pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    point_indirect: Arc<wgpu::Buffer>,
    body_indirect: Arc<wgpu::Buffer>,
}

#[derive(Clone, Copy)]
enum DynamicDispatch {
    Topology64,
    Point64,
    DoublePoint64,
    DoublePoint256,
    Body64,
    Winner64,
}

#[derive(Clone, Copy)]
struct TopologyLayout {
    kind: GpuGatherTopology,
    incident_base: u32,
    cursor_base: u32,
    edge_a_base: u32,
    edge_b_base: u32,
    blocks_base: u32,
    ranges_base: u32,
    edge_count: u32,
    block_count: u32,
    radix_passes: u32,
    sorted_edge_base: u32,
    manifold_counts_base: u32,
    has_manifold_counts: bool,
    color_base: u32,
    color_mask_base: u32,
    color_status_index: u32,
    color_count_index: u32,
    color_counts_base: u32,
    color_offsets_base: u32,
    color_cursors_base: u32,
    color_indices_base: u32,
    color_block_counts_base: u32,
    color_block_offsets_base: u32,
    color_local_ranks_base: u32,
    color_block_count: u32,
    parallel_color_winners_base: u32,
    parallel_color_winner_count: u32,
    parallel_color_stats_base: u32,
    color_identity_base: u32,
    color_identity_capacity: u32,
    color_maintenance_counts_base: u32,
    color_maintenance_state_base: u32,
    island_parent_base: u32,
    body_island_base: u32,
    island_roots_base: u32,
    island_body_counts_base: u32,
    island_manifold_counts_base: u32,
    island_bin_counts_base: u32,
    island_bin_offsets_base: u32,
    island_bin_cursors_base: u32,
    island_point_indices_base: u32,
    island_count_index: u32,
    authority_initial_transforms_base: u32,
    color_unit: GpuColorUnitV4,
    has_coloring: bool,
    has_compact_coloring: bool,
    has_parallel_compaction: bool,
    has_parallel_coloring: bool,
    has_incremental_coloring: bool,
    has_island_dispatch: bool,
    has_authority_initial_transforms: bool,
    static_late: bool,
    work_state_base: u32,
    element_count: u32,
}

impl TopologyLayout {
    fn enable_dynamic_work(&mut self, body_capacity: u32) -> Result<(), String> {
        self.work_state_base = self.element_count;
        self.element_count = self
            .element_count
            .checked_add(SOLVER_WORK_STATE_WORDS)
            .and_then(|end| end.checked_add(body_capacity))
            .ok_or_else(|| "solver active-work range overflowed".to_owned())?;
        Ok(())
    }

    fn new(
        kind: GpuGatherTopology,
        body_count: u32,
        point_count: u32,
        include_manifold_counts: bool,
        color_schedule: GpuColorScheduleV4,
        color_unit: GpuColorUnitV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<Self, String> {
        let mut layout = match kind {
            GpuGatherTopology::CanonicalBodyScan => Self::canonical(body_count, point_count),
            GpuGatherTopology::StableBodyEdge => Self::stable_body_edge(body_count, point_count),
        }?;
        layout.static_late = scheduling_policy.uses_static_late();
        if include_manifold_counts {
            layout.manifold_counts_base = layout.element_count;
            layout.has_manifold_counts = true;
            layout.element_count = layout
                .element_count
                .checked_add(body_count)
                .ok_or_else(|| "solver manifold-count range overflowed".to_owned())?;
        }
        if color_schedule != GpuColorScheduleV4::None {
            layout.color_base = layout.element_count;
            layout.color_mask_base = layout
                .color_base
                .checked_add(point_count)
                .ok_or_else(|| "solver color assignment range overflowed".to_owned())?;
            layout.color_status_index = layout
                .color_mask_base
                .checked_add(body_count)
                .ok_or_else(|| "solver color mask range overflowed".to_owned())?;
            layout.color_count_index = layout
                .color_status_index
                .checked_add(1)
                .ok_or_else(|| "solver color status range overflowed".to_owned())?;
            layout.element_count = layout
                .color_count_index
                .checked_add(1)
                .ok_or_else(|| "solver color evidence range overflowed".to_owned())?;
            layout.has_coloring = true;
            layout.color_unit = color_unit;
            if color_schedule == GpuColorScheduleV4::CompactIndirect {
                layout.color_counts_base = layout.element_count;
                layout.color_offsets_base = layout
                    .color_counts_base
                    .checked_add(MAX_COLORS)
                    .ok_or_else(|| "solver color counts range overflowed".to_owned())?;
                layout.color_cursors_base = layout
                    .color_offsets_base
                    .checked_add(COLOR_OFFSET_COUNT)
                    .ok_or_else(|| "solver color offsets range overflowed".to_owned())?;
                layout.color_indices_base = layout
                    .color_cursors_base
                    .checked_add(MAX_COLORS)
                    .ok_or_else(|| "solver color cursors range overflowed".to_owned())?;
                layout.element_count = layout
                    .color_indices_base
                    .checked_add(point_count)
                    .ok_or_else(|| "solver compact color range overflowed".to_owned())?;
                layout.has_compact_coloring = true;
                if scheduling_policy.uses_parallel_compaction() {
                    layout.color_block_count = point_count.div_ceil(WORKGROUP_SIZE);
                    let block_color_words = layout
                        .color_block_count
                        .checked_mul(MAX_COLORS)
                        .ok_or_else(|| "solver block-color range overflowed".to_owned())?;
                    layout.color_block_counts_base = layout.element_count;
                    layout.color_block_offsets_base = layout
                        .color_block_counts_base
                        .checked_add(block_color_words)
                        .ok_or_else(|| "solver block-color count range overflowed".to_owned())?;
                    layout.color_local_ranks_base = layout
                        .color_block_offsets_base
                        .checked_add(block_color_words)
                        .ok_or_else(|| "solver block-color offset range overflowed".to_owned())?;
                    layout.element_count =
                        layout
                            .color_local_ranks_base
                            .checked_add(point_count)
                            .ok_or_else(|| "solver local color-rank range overflowed".to_owned())?;
                    layout.has_parallel_compaction = true;
                }
            }
            if scheduling_policy.uses_parallel_coloring() {
                layout.parallel_color_winner_count = body_count
                    .checked_mul(MAX_COLORS)
                    .ok_or_else(|| "solver parallel color-winner capacity overflowed".to_owned())?;
                layout.parallel_color_winners_base = layout.element_count;
                layout.parallel_color_stats_base = layout
                    .parallel_color_winners_base
                    .checked_add(layout.parallel_color_winner_count)
                    .ok_or_else(|| "solver parallel color-winner range overflowed".to_owned())?;
                layout.element_count = layout
                    .parallel_color_stats_base
                    .checked_add(3)
                    .ok_or_else(|| "solver parallel color-stat range overflowed".to_owned())?;
                layout.has_parallel_coloring = true;
            }
            if scheduling_policy.color_maintenance == GpuColorMaintenanceV1::StablePairIncremental {
                let minimum_identity_capacity = point_count
                    .checked_mul(2)
                    .ok_or_else(|| "solver color identity capacity overflowed".to_owned())?;
                layout.color_identity_capacity = minimum_identity_capacity
                    .max(1)
                    .checked_next_power_of_two()
                    .ok_or_else(|| "solver color identity capacity overflowed".to_owned())?;
                layout.color_identity_base = layout.element_count;
                layout.color_maintenance_counts_base = layout
                    .color_identity_base
                    .checked_add(
                        layout
                            .color_identity_capacity
                            .checked_mul(6)
                            .and_then(|words| words.checked_mul(2))
                            .ok_or_else(|| "solver color identity range overflowed".to_owned())?,
                    )
                    .ok_or_else(|| "solver color identity range overflowed".to_owned())?;
                layout.color_maintenance_state_base = layout
                    .color_maintenance_counts_base
                    .checked_add(4)
                    .ok_or_else(|| "solver color maintenance range overflowed".to_owned())?;
                layout.element_count = layout
                    .color_maintenance_state_base
                    .checked_add(4)
                    .ok_or_else(|| "solver color maintenance state overflowed".to_owned())?;
                layout.has_incremental_coloring = true;
            }
            if scheduling_policy
                .colored_dispatch
                .uses_workgroup_per_island()
            {
                let island_bin_capacity = body_count
                    .checked_mul(MAX_COLORS)
                    .ok_or_else(|| "solver island-bin capacity overflowed".to_owned())?;
                layout.island_parent_base = layout.element_count;
                layout.body_island_base = layout
                    .island_parent_base
                    .checked_add(body_count)
                    .ok_or_else(|| "solver island parent range overflowed".to_owned())?;
                layout.island_roots_base = layout
                    .body_island_base
                    .checked_add(body_count)
                    .ok_or_else(|| "solver body-island range overflowed".to_owned())?;
                layout.island_body_counts_base =
                    layout
                        .island_roots_base
                        .checked_add(body_count)
                        .ok_or_else(|| "solver island-root range overflowed".to_owned())?;
                layout.island_manifold_counts_base = layout
                    .island_body_counts_base
                    .checked_add(body_count)
                    .ok_or_else(|| "solver island body-count range overflowed".to_owned())?;
                layout.island_bin_counts_base = layout
                    .island_manifold_counts_base
                    .checked_add(body_count)
                    .ok_or_else(|| "solver island manifold-count range overflowed".to_owned())?;
                layout.island_bin_offsets_base = layout
                    .island_bin_counts_base
                    .checked_add(island_bin_capacity)
                    .ok_or_else(|| "solver island-bin count range overflowed".to_owned())?;
                layout.island_bin_cursors_base = layout
                    .island_bin_offsets_base
                    .checked_add(island_bin_capacity)
                    .and_then(|end| end.checked_add(1))
                    .ok_or_else(|| "solver island-bin offset range overflowed".to_owned())?;
                layout.island_point_indices_base = layout
                    .island_bin_cursors_base
                    .checked_add(island_bin_capacity)
                    .ok_or_else(|| "solver island-bin cursor range overflowed".to_owned())?;
                layout.island_count_index = layout
                    .island_point_indices_base
                    .checked_add(point_count)
                    .ok_or_else(|| "solver island point-index range overflowed".to_owned())?;
                layout.element_count = layout
                    .island_count_index
                    .checked_add(1)
                    .ok_or_else(|| "solver island count range overflowed".to_owned())?;
                layout.has_island_dispatch = true;
            }
            if scheduling_policy.uses_authority_source_order() {
                layout.authority_initial_transforms_base = layout.element_count;
                let transform_words = body_count
                    .checked_mul(3)
                    .ok_or_else(|| "authority initial-transform capacity overflowed".to_owned())?;
                layout.element_count = layout
                    .element_count
                    .checked_add(transform_words)
                    .ok_or_else(|| "authority initial-transform range overflowed".to_owned())?;
                layout.has_authority_initial_transforms = true;
            }
        }
        layout.work_state_base = layout.element_count;
        Ok(layout)
    }

    fn canonical(body_count: u32, point_count: u32) -> Result<Self, String> {
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
            kind: GpuGatherTopology::CanonicalBodyScan,
            incident_base,
            cursor_base,
            edge_a_base: 0,
            edge_b_base: 0,
            blocks_base: 0,
            ranges_base: 0,
            edge_count: point_count.saturating_mul(2),
            block_count: 0,
            radix_passes: 0,
            sorted_edge_base: 0,
            manifold_counts_base: 0,
            has_manifold_counts: false,
            color_base: 0,
            color_mask_base: 0,
            color_status_index: 0,
            color_count_index: 0,
            color_counts_base: 0,
            color_offsets_base: 0,
            color_cursors_base: 0,
            color_indices_base: 0,
            color_block_counts_base: 0,
            color_block_offsets_base: 0,
            color_local_ranks_base: 0,
            color_block_count: 0,
            parallel_color_winners_base: 0,
            parallel_color_winner_count: 0,
            parallel_color_stats_base: 0,
            color_identity_base: 0,
            color_identity_capacity: 0,
            color_maintenance_counts_base: 0,
            color_maintenance_state_base: 0,
            island_parent_base: 0,
            body_island_base: 0,
            island_roots_base: 0,
            island_body_counts_base: 0,
            island_manifold_counts_base: 0,
            island_bin_counts_base: 0,
            island_bin_offsets_base: 0,
            island_bin_cursors_base: 0,
            island_point_indices_base: 0,
            island_count_index: 0,
            authority_initial_transforms_base: 0,
            color_unit: GpuColorUnitV4::Point,
            has_coloring: false,
            has_compact_coloring: false,
            has_parallel_compaction: false,
            has_parallel_coloring: false,
            has_incremental_coloring: false,
            has_island_dispatch: false,
            has_authority_initial_transforms: false,
            static_late: false,
            work_state_base: 0,
            element_count,
        })
    }

    fn stable_body_edge(body_count: u32, point_count: u32) -> Result<Self, String> {
        let edge_count = point_count
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge count overflowed".to_owned())?;
        let edge_words = edge_count
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge record range overflowed".to_owned())?;
        let edge_a_base = 0;
        let edge_b_base = edge_words;
        let blocks_base = edge_words
            .checked_mul(2)
            .ok_or_else(|| "stable body-edge ping-pong range overflowed".to_owned())?;
        let block_count = edge_count.div_ceil(RADIX_WORKGROUP_SIZE);
        let block_words = block_count
            .checked_mul(16)
            .ok_or_else(|| "stable body-edge radix blocks overflowed".to_owned())?;
        let ranges_base = blocks_base
            .checked_add(block_words)
            .ok_or_else(|| "stable body-edge block range overflowed".to_owned())?;
        let element_count = ranges_base
            .checked_add(
                body_count
                    .checked_mul(2)
                    .ok_or_else(|| "stable body ranges overflowed".to_owned())?,
            )
            .ok_or_else(|| "stable body-edge topology range overflowed".to_owned())?;
        let body_bits = 32 - body_count.leading_zeros();
        let radix_passes = body_bits.max(1).div_ceil(4);
        let sorted_edge_base = if radix_passes.is_multiple_of(2) {
            edge_a_base
        } else {
            edge_b_base
        };
        Ok(Self {
            kind: GpuGatherTopology::StableBodyEdge,
            incident_base: 0,
            cursor_base: 0,
            edge_a_base,
            edge_b_base,
            blocks_base,
            ranges_base,
            edge_count,
            block_count,
            radix_passes,
            sorted_edge_base,
            manifold_counts_base: 0,
            has_manifold_counts: false,
            color_base: 0,
            color_mask_base: 0,
            color_status_index: 0,
            color_count_index: 0,
            color_counts_base: 0,
            color_offsets_base: 0,
            color_cursors_base: 0,
            color_indices_base: 0,
            color_block_counts_base: 0,
            color_block_offsets_base: 0,
            color_local_ranks_base: 0,
            color_block_count: 0,
            parallel_color_winners_base: 0,
            parallel_color_winner_count: 0,
            parallel_color_stats_base: 0,
            color_identity_base: 0,
            color_identity_capacity: 0,
            color_maintenance_counts_base: 0,
            color_maintenance_state_base: 0,
            island_parent_base: 0,
            body_island_base: 0,
            island_roots_base: 0,
            island_body_counts_base: 0,
            island_manifold_counts_base: 0,
            island_bin_counts_base: 0,
            island_bin_offsets_base: 0,
            island_bin_cursors_base: 0,
            island_point_indices_base: 0,
            island_count_index: 0,
            authority_initial_transforms_base: 0,
            color_unit: GpuColorUnitV4::Point,
            has_coloring: false,
            has_compact_coloring: false,
            has_parallel_compaction: false,
            has_parallel_coloring: false,
            has_incremental_coloring: false,
            has_island_dispatch: false,
            has_authority_initial_transforms: false,
            static_late: false,
            work_state_base: 0,
            element_count,
        })
    }

    const fn dispatches(self) -> u32 {
        let base = match self.kind {
            GpuGatherTopology::CanonicalBodyScan => 4,
            GpuGatherTopology::StableBodyEdge => 3 + self.radix_passes * 3,
        };
        base + self.has_manifold_counts as u32
            + self.color_construction_dispatches()
            + self.color_compaction_dispatches()
            + self.has_island_dispatch as u32
    }

    const fn color_compaction_dispatches(self) -> u32 {
        if self.has_parallel_compaction {
            4
        } else {
            self.has_compact_coloring as u32
        }
    }

    const fn color_construction_dispatches(self) -> u32 {
        if self.has_parallel_coloring {
            2 + PARALLEL_COLOR_ROUNDS * 3
        } else {
            self.has_coloring as u32
        }
    }
}

pub(crate) struct GpuFixedFloorSolver {
    _owned_point_input: Option<wgpu::Buffer>,
    _motion_scratch: wgpu::Buffer,
    _prepared: wgpu::Buffer,
    _contributions: wgpu::Buffer,
    _topology: wgpu::Buffer,
    _color_indirect: Option<wgpu::Buffer>,
    _parameter_buffer: wgpu::Buffer,
    _work_parameter_buffer: wgpu::Buffer,
    parameters: Vec<PassParamsRaw>,
    bind_ab: wgpu::BindGroup,
    bind_ba: wgpu::BindGroup,
    color_bind_ab: Option<wgpu::BindGroup>,
    dynamic_work: Option<DynamicSolverWork>,
    topology_clear_pipeline: wgpu::ComputePipeline,
    topology_count_pipeline: wgpu::ComputePipeline,
    manifold_count_pipeline: Option<wgpu::ComputePipeline>,
    topology_prefix_pipeline: wgpu::ComputePipeline,
    topology_scatter_pipeline: wgpu::ComputePipeline,
    edge_build_pipeline: wgpu::ComputePipeline,
    edge_radix_histograms_pipeline: wgpu::ComputePipeline,
    edge_radix_prefix_pipeline: wgpu::ComputePipeline,
    edge_radix_scatter_pipeline: wgpu::ComputePipeline,
    edge_ranges_pipeline: wgpu::ComputePipeline,
    color_pipeline: Option<wgpu::ComputePipeline>,
    color_conflict_clear_pipeline: Option<wgpu::ComputePipeline>,
    color_propose_pipeline: Option<wgpu::ComputePipeline>,
    color_resolve_pipeline: Option<wgpu::ComputePipeline>,
    color_finalize_pipeline: Option<wgpu::ComputePipeline>,
    color_compact_pipeline: Option<wgpu::ComputePipeline>,
    color_block_count_pipeline: Option<wgpu::ComputePipeline>,
    color_block_prefix_pipeline: Option<wgpu::ComputePipeline>,
    color_range_prefix_pipeline: Option<wgpu::ComputePipeline>,
    color_block_scatter_pipeline: Option<wgpu::ComputePipeline>,
    island_build_pipeline: Option<wgpu::ComputePipeline>,
    integrate_velocity_pipeline: wgpu::ComputePipeline,
    prepare_pipeline: wgpu::ComputePipeline,
    refresh_pipeline: wgpu::ComputePipeline,
    warm_pipeline: wgpu::ComputePipeline,
    normal_pipeline: wgpu::ComputePipeline,
    normal_block_pipeline: Option<wgpu::ComputePipeline>,
    tangent_pipeline: wgpu::ComputePipeline,
    restitution_pipeline: wgpu::ComputePipeline,
    integrate_position_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    colored_pipeline: Option<wgpu::ComputePipeline>,
    authority_overflow_pipeline: Option<wgpu::ComputePipeline>,
    island_colored_pipeline: Option<wgpu::ComputePipeline>,
    operations: Vec<Operation>,
    capacities: Vec<CapacityRecordV1>,
    point_count: u32,
    body_count: u32,
    topology_layout: TopologyLayout,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    edge_build_permuted: bool,
}

impl GpuFixedFloorSolver {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        validate_authored_settings(scenario)?;
        if scenario.manifolds.len() != 1 || scenario.manifolds[0].points.len() != 2 {
            return Err(
                "P5.1 fixed-floor solver requires one authored two-point manifold".to_owned(),
            );
        }
        let (points, point_count) = point_inputs(scenario)?;
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_floor_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            GpuGatherTopology::CanonicalBodyScan,
            false,
            GpuSolverExecutionV4::experiment(true, true),
            GpuSolverSchedulingPolicyV1::STANDARD,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_authored_with_topology_and_coefficients(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        topology: GpuGatherTopology,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
    ) -> Result<Self, String> {
        validate_experiment_settings(&scenario.settings, execution)?;
        scheduling_policy.validate(execution)?;
        contact_coefficient_policy.validate(&scenario.settings)?;
        if scenario.manifolds.is_empty() {
            return Err("authored gather benchmark requires contact manifolds".to_owned());
        }
        let (points, point_count) = point_inputs(scenario)?;
        if point_count == 0 {
            return Err("authored gather benchmark requires contact points".to_owned());
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_authored_benchmark_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer_and_solver_policies(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            topology,
            false,
            execution,
            scheduling_policy,
            GpuSoftFeedbackPolicyV1::SELECTED,
            GpuNormalRelaxationPolicyV1::SELECTED,
            contact_coefficient_policy,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_authored_authority_gpu_probe(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        crate::AuthorityGpuProbePolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::authority_gpu_probe::validate_authority_gpu_probe_settings(&scenario.settings)?;
        if scenario.manifolds.is_empty() {
            return Err("authored authority probe requires contact manifolds".to_owned());
        }
        let (points, point_count) = point_inputs(scenario)?;
        if point_count == 0 {
            return Err("authored authority probe requires contact points".to_owned());
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_authored_authority_probe_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer_and_extended_solver_policies(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            GpuGatherTopology::StableBodyEdge,
            false,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                storage.layout().capacity_elements,
            ),
            GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            GpuNormalRelaxationPolicyV1::PointSequential,
            GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
            GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            None,
            None,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new_authored_branch_robust_gpu_promotion(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_settings(
            &scenario.settings,
        )?;
        if scenario.manifolds.is_empty() {
            return Err(
                "authored branch-robust promotion probe requires contact manifolds".to_owned(),
            );
        }
        let (points, point_count) = point_inputs(scenario)?;
        if point_count == 0 {
            return Err(
                "authored branch-robust promotion probe requires contact points".to_owned(),
            );
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let point_buffer = storage_init(
            device,
            "physbox5_authored_branch_robust_promotion_points",
            bytemuck::cast_slice(&points),
            usage,
        );
        let mut solver = Self::new_with_point_buffer_and_extended_solver_policies(
            device,
            storage,
            scenario,
            &point_buffer,
            point_count,
            GpuGatherTopology::StableBodyEdge,
            false,
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(
                storage.layout().capacity_elements,
            ),
            GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            GpuNormalRelaxationPolicyV1::PointSequential,
            GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            None,
            None,
        )?;
        solver._owned_point_input = Some(point_buffer);
        Ok(solver)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_generated_experiment_with_topology_scheduling_and_extended_solver_policies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_work_state: Arc<wgpu::Buffer>,
        body_work_state: Arc<wgpu::Buffer>,
        point_capacity: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
        substep_contact_policy: GpuSubstepContactPolicyV1,
        restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    ) -> Result<Self, String> {
        validate_experiment_settings(&scenario.settings, execution)?;
        scheduling_policy.validate(execution)?;
        soft_feedback_policy.validate(&scenario.settings)?;
        normal_relaxation_policy.validate(&scenario.settings, execution)?;
        contact_coefficient_policy.validate(&scenario.settings)?;
        substep_contact_policy.validate_composition(
            &scenario.settings,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            restitution_timing_policy,
        )?;
        restitution_timing_policy.validate(
            &scenario.settings,
            execution,
            normal_relaxation_policy,
            contact_coefficient_policy,
            substep_contact_policy,
        )?;
        if !scenario.manifolds.is_empty() || point_capacity == 0 {
            return Err(
                "generated contact solver requires a neutral scenario and nonzero point capacity"
                    .to_owned(),
            );
        }
        Self::new_with_point_buffer_and_extended_solver_policies(
            device,
            storage,
            scenario,
            point_buffer,
            point_capacity,
            topology,
            permute_edge_build,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            substep_contact_policy,
            restitution_timing_policy,
            Some(point_work_state),
            Some(body_work_state),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_point_buffer(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_count: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_point_buffer_and_contact_feedback(
            device,
            storage,
            scenario,
            point_buffer,
            point_count,
            topology,
            permute_edge_build,
            execution,
            scheduling_policy,
            GpuSoftFeedbackPolicyV1::SELECTED,
            GpuNormalRelaxationPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_point_buffer_and_contact_feedback(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_count: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_point_buffer_and_solver_policies(
            device,
            storage,
            scenario,
            point_buffer,
            point_count,
            topology,
            permute_edge_build,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            GpuContactCoefficientPolicyV1::SELECTED,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_point_buffer_and_solver_policies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_count: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
    ) -> Result<Self, String> {
        Self::new_with_point_buffer_and_extended_solver_policies(
            device,
            storage,
            scenario,
            point_buffer,
            point_count,
            topology,
            permute_edge_build,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            GpuSubstepContactPolicyV1::SELECTED,
            GpuRestitutionTimingPolicyV1::SELECTED,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_point_buffer_and_extended_solver_policies(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        scenario: &ScenarioV1,
        point_buffer: &wgpu::Buffer,
        point_count: u32,
        topology: GpuGatherTopology,
        permute_edge_build: bool,
        execution: GpuSolverExecutionV4,
        scheduling_policy: GpuSolverSchedulingPolicyV1,
        soft_feedback_policy: GpuSoftFeedbackPolicyV1,
        normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
        contact_coefficient_policy: GpuContactCoefficientPolicyV1,
        substep_contact_policy: GpuSubstepContactPolicyV1,
        restitution_timing_policy: GpuRestitutionTimingPolicyV1,
        point_work_state: Option<Arc<wgpu::Buffer>>,
        body_work_state: Option<Arc<wgpu::Buffer>>,
    ) -> Result<Self, String> {
        let exact_authority_probe =
            crate::authority_gpu_probe::validate_authority_gpu_probe_runtime_tuple(
                execution,
                scheduling_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )
            .is_ok();
        let branch_robust_authority_probe =
            crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_runtime_tuple(
                execution,
                scheduling_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )
            .is_ok();
        let branch_robust_tunable_probe =
            crate::branch_robust_gpu_promotion::validate_branch_robust_tunable_runtime_tuple(
                execution,
                scheduling_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )
            .is_ok();
        let authority_probe =
            exact_authority_probe || branch_robust_authority_probe || branch_robust_tunable_probe;
        if exact_authority_probe {
            crate::authority_gpu_probe::validate_authority_gpu_probe_settings(&scenario.settings)?;
        } else if branch_robust_authority_probe {
            crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_settings(
                &scenario.settings,
            )?;
        } else if branch_robust_tunable_probe {
            crate::branch_robust_gpu_promotion::validate_branch_robust_tunable_settings(
                &scenario.settings,
            )?;
        }
        scheduling_policy.validate(execution)?;
        soft_feedback_policy.validate(&scenario.settings)?;
        normal_relaxation_policy.validate(&scenario.settings, execution)?;
        contact_coefficient_policy.validate(&scenario.settings)?;
        substep_contact_policy.validate_composition(
            &scenario.settings,
            execution,
            scheduling_policy,
            soft_feedback_policy,
            normal_relaxation_policy,
            contact_coefficient_policy,
            restitution_timing_policy,
        )?;
        restitution_timing_policy.validate(
            &scenario.settings,
            execution,
            normal_relaxation_policy,
            contact_coefficient_policy,
            substep_contact_policy,
        )?;
        if scheduling_policy.colored_dispatch.fuses_normal_tangent()
            && (scenario.settings.contact_softness.is_some()
                || soft_feedback_policy != GpuSoftFeedbackPolicyV1::SELECTED
                || normal_relaxation_policy != GpuNormalRelaxationPolicyV1::SELECTED)
            && !authority_probe
        {
            return Err(
                "fused normal/tangent scheduling is limited to the hard/direct policy".to_owned(),
            );
        }
        let body_count = storage.layout().capacity_elements;
        let mut topology_layout = TopologyLayout::new(
            topology,
            body_count,
            point_count,
            execution.normal_solver.uses_manifold_mass_split(),
            execution.color_schedule,
            execution.color_unit,
            scheduling_policy,
        )?;
        if point_work_state.is_some() {
            topology_layout.enable_dynamic_work(body_count)?;
        }
        if execution.uses_colored_in_place() && topology != GpuGatherTopology::StableBodyEdge {
            return Err(
                "colored generated-contact comparison requires the stable body-edge view"
                    .to_owned(),
            );
        }
        let (parameters, operations) = if soft_feedback_policy == GpuSoftFeedbackPolicyV1::SELECTED
            && normal_relaxation_policy == GpuNormalRelaxationPolicyV1::SELECTED
            && contact_coefficient_policy == GpuContactCoefficientPolicyV1::SELECTED
            && substep_contact_policy == GpuSubstepContactPolicyV1::SELECTED
            && restitution_timing_policy == GpuRestitutionTimingPolicyV1::SELECTED
        {
            operations(
                scenario,
                body_count,
                point_count,
                topology_layout,
                permute_edge_build,
                execution,
                scheduling_policy,
            )
        } else {
            operations_with_extended_solver_policies(
                scenario,
                body_count,
                point_count,
                topology_layout,
                permute_edge_build,
                execution,
                scheduling_policy,
                soft_feedback_policy,
                normal_relaxation_policy,
                contact_coefficient_policy,
                substep_contact_policy,
                restitution_timing_policy,
            )
        };
        let mut initial_motion = vec![[0.0; 4]; body_count as usize];
        for body in &scenario.bodies {
            initial_motion[body.id.slot as usize] = [
                body.linear_velocity.x,
                body.linear_velocity.y,
                body.angular_velocity.get(),
                0.0,
            ];
        }
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let motion_scratch = storage_init(
            device,
            "physbox5_floor_motion_scratch",
            bytemuck::cast_slice(&initial_motion),
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
        let color_indirect = topology_layout.has_coloring.then(|| {
            let words = COLOR_INDIRECT_WORDS
                + u32::from(topology_layout.has_island_dispatch) * ISLAND_INDIRECT_WORDS;
            storage_empty::<u32>(
                device,
                "physbox5_floor_color_indirect",
                words,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_SRC,
            )
        });
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
        let work_parameter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_floor_work_parameters"),
            contents: bytemuck::bytes_of(&SolverWorkParamsRaw {
                values: [
                    u32::from(point_work_state.is_some()),
                    topology_layout.work_state_base,
                    0,
                    0,
                ],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layout_entries = [
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
            wgpu::BindGroupLayoutEntry {
                binding: 10,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<SolverWorkParamsRaw>() as u64),
                },
                count: None,
            },
        ];
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_floor_solver_layout"),
            entries: &layout_entries,
        });
        let color_layout = if topology_layout.has_coloring {
            Some(
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("physbox5_floor_color_layout"),
                    entries: &[
                        storage_entry(3, true),
                        storage_entry(4, true),
                        storage_entry(7, false),
                        layout_entries[8],
                        storage_entry(9, false),
                        layout_entries[9],
                    ],
                }),
            )
        } else {
            None
        };
        let bind_ab = bind_group(
            device,
            &layout,
            "physbox5_floor_bind_ab",
            storage,
            storage.motion_buffer(),
            &motion_scratch,
            point_buffer,
            &prepared,
            &contributions,
            &topology,
            &parameter_buffer,
            &work_parameter_buffer,
            None,
        );
        let bind_ba = bind_group(
            device,
            &layout,
            "physbox5_floor_bind_ba",
            storage,
            &motion_scratch,
            storage.motion_buffer(),
            point_buffer,
            &prepared,
            &contributions,
            &topology,
            &parameter_buffer,
            &work_parameter_buffer,
            None,
        );
        let color_bind_ab = match (&color_layout, &color_indirect) {
            (Some(layout), Some(indirect)) => Some(color_bind_group(
                device,
                layout,
                "physbox5_floor_color_bind_ab",
                storage.properties_buffer(),
                point_buffer,
                &topology,
                &parameter_buffer,
                &work_parameter_buffer,
                indirect,
            )),
            (None, None) => None,
            _ => return Err("solver color bind resources disagree".to_owned()),
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_floor_solver_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_floor_solver.wgsl").into()),
        });
        let dynamic_work = if let (Some(point_indirect), Some(body_indirect)) =
            (point_work_state, body_work_state)
        {
            let work_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_floor_dynamic_work_layout"),
                entries: &[
                    storage_entry(7, false),
                    layout_entries[9],
                    storage_entry(11, true),
                    storage_entry(12, true),
                ],
            });
            let work_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("physbox5_floor_dynamic_work_bind_group"),
                layout: &work_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: topology.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: work_parameter_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: point_indirect.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12,
                        resource: body_indirect.as_entire_binding(),
                    },
                ],
            });
            let work_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("physbox5_floor_dynamic_work_pipeline_layout"),
                    bind_group_layouts: &[&work_layout],
                    push_constant_ranges: &[],
                });
            let work_pipeline = |label, entry_point| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&work_pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
            };
            Some(DynamicSolverWork {
                counts_pipeline: work_pipeline(
                    "physbox5_floor_prepare_dynamic_work_counts",
                    "prepare_solver_work_counts",
                ),
                bodies_pipeline: work_pipeline(
                    "physbox5_floor_prepare_dynamic_work_bodies",
                    "prepare_solver_work_bodies",
                ),
                bind_group: work_bind_group,
                point_indirect,
                body_indirect,
            })
        } else {
            None
        };
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[],
                    zero_initialize_workgroup_memory: false,
                },
                cache: None,
            })
        };
        let color_pipeline = match &color_layout {
            Some(layout) => {
                let color_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("physbox5_floor_color_pipeline_layout"),
                        bind_group_layouts: &[layout],
                        push_constant_ranges: &[],
                    });
                Some(
                    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("floor_color_contacts"),
                        layout: Some(&color_pipeline_layout),
                        module: &shader,
                        entry_point: Some(match scheduling_policy.color_maintenance {
                            GpuColorMaintenanceV1::StablePairIncremental => {
                                "color_contacts_incremental"
                            }
                            GpuColorMaintenanceV1::FullRebuildParallelSpeculative => {
                                "initialize_parallel_colors"
                            }
                            GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate => {
                                "initialize_parallel_colors"
                            }
                            _ => "color_contacts",
                        }),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &[],
                            zero_initialize_workgroup_memory: false,
                        },
                        cache: None,
                    }),
                )
            }
            None => None,
        };
        let color_stage_pipeline = |label: &'static str,
                                    entry: &'static str,
                                    enabled: bool|
         -> Option<wgpu::ComputePipeline> {
            if !enabled {
                return None;
            }
            let layout = color_layout.as_ref()?;
            let color_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(label),
                    bind_group_layouts: &[layout],
                    push_constant_ranges: &[],
                });
            Some(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&color_pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[],
                        zero_initialize_workgroup_memory: false,
                    },
                    cache: None,
                }),
            )
        };
        let compact_coloring = execution.color_schedule == GpuColorScheduleV4::CompactIndirect;
        let parallel_compaction = topology_layout.has_parallel_compaction;
        let parallel_coloring = topology_layout.has_parallel_coloring;
        let color_conflict_clear_pipeline = color_stage_pipeline(
            "floor_clear_color_conflicts",
            "clear_parallel_color_winners",
            parallel_coloring,
        );
        let color_propose_pipeline = color_stage_pipeline(
            "floor_propose_parallel_colors",
            "propose_parallel_colors",
            parallel_coloring,
        );
        let color_resolve_pipeline = color_stage_pipeline(
            "floor_resolve_parallel_colors",
            "resolve_parallel_colors",
            parallel_coloring,
        );
        let color_finalize_pipeline = color_stage_pipeline(
            "floor_finalize_parallel_colors",
            "finalize_parallel_colors",
            parallel_coloring,
        );
        let color_compact_pipeline = color_stage_pipeline(
            "floor_compact_colors",
            "compact_colors",
            compact_coloring && !parallel_compaction,
        );
        let color_block_count_pipeline = color_stage_pipeline(
            "floor_count_color_blocks",
            "count_color_blocks",
            parallel_compaction,
        );
        let color_block_prefix_pipeline = color_stage_pipeline(
            "floor_prefix_color_blocks",
            "prefix_color_blocks",
            parallel_compaction,
        );
        let color_range_prefix_pipeline = color_stage_pipeline(
            "floor_prefix_color_ranges",
            "prefix_color_ranges",
            parallel_compaction,
        );
        let color_block_scatter_pipeline = color_stage_pipeline(
            "floor_scatter_color_blocks",
            "scatter_color_blocks",
            parallel_compaction,
        );
        let island_build_pipeline = match &color_layout {
            Some(layout) if topology_layout.has_island_dispatch => {
                let island_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("physbox5_floor_island_build_pipeline_layout"),
                        bind_group_layouts: &[layout],
                        push_constant_ranges: &[],
                    });
                Some(
                    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("floor_build_solver_islands"),
                        layout: Some(&island_pipeline_layout),
                        module: &shader,
                        entry_point: Some("build_solver_islands"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &[],
                            zero_initialize_workgroup_memory: false,
                        },
                        cache: None,
                    }),
                )
            }
            _ => None,
        };
        let mut capacities = vec![
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
        ];
        capacities.extend(topology_capacities(
            body_count,
            point_count,
            topology_layout,
        ));
        Ok(Self {
            _owned_point_input: None,
            _motion_scratch: motion_scratch,
            _prepared: prepared,
            _contributions: contributions,
            _topology: topology,
            _color_indirect: color_indirect,
            _parameter_buffer: parameter_buffer,
            _work_parameter_buffer: work_parameter_buffer,
            parameters,
            bind_ab,
            bind_ba,
            color_bind_ab,
            dynamic_work,
            topology_clear_pipeline: pipeline("floor_topology_clear", "clear_topology"),
            topology_count_pipeline: pipeline("floor_topology_count", "count_edges"),
            manifold_count_pipeline: execution
                .normal_solver
                .uses_manifold_mass_split()
                .then(|| pipeline("floor_manifold_count", "count_manifolds")),
            topology_prefix_pipeline: pipeline("floor_topology_prefix", "prefix_edges"),
            topology_scatter_pipeline: pipeline("floor_topology_scatter", "scatter_edges"),
            edge_build_pipeline: pipeline("floor_edge_build", "build_stable_edges"),
            edge_radix_histograms_pipeline: pipeline(
                "floor_edge_radix_histograms",
                "edge_radix_histograms",
            ),
            edge_radix_prefix_pipeline: pipeline("floor_edge_radix_prefix", "edge_radix_prefix"),
            edge_radix_scatter_pipeline: pipeline("floor_edge_radix_scatter", "edge_radix_scatter"),
            edge_ranges_pipeline: pipeline("floor_edge_ranges", "build_edge_ranges"),
            color_pipeline,
            color_conflict_clear_pipeline,
            color_propose_pipeline,
            color_resolve_pipeline,
            color_finalize_pipeline,
            color_compact_pipeline,
            color_block_count_pipeline,
            color_block_prefix_pipeline,
            color_range_prefix_pipeline,
            color_block_scatter_pipeline,
            island_build_pipeline,
            integrate_velocity_pipeline: pipeline("floor_integrate_velocity", "integrate_velocity"),
            prepare_pipeline: pipeline("floor_prepare", "prepare"),
            refresh_pipeline: pipeline("floor_refresh", "refresh_contacts"),
            warm_pipeline: pipeline("floor_warm", "emit_warm"),
            normal_pipeline: pipeline("floor_normal", "emit_normal"),
            normal_block_pipeline: execution
                .normal_solver
                .uses_block()
                .then(|| pipeline("floor_normal_block", "emit_normal_block")),
            tangent_pipeline: pipeline("floor_tangent", "emit_tangent"),
            restitution_pipeline: pipeline("floor_restitution", "emit_restitution"),
            integrate_position_pipeline: pipeline("floor_integrate_position", "integrate_position"),
            gather_pipeline: pipeline("floor_gather", "gather"),
            colored_pipeline: execution
                .uses_colored_in_place()
                .then(|| pipeline("floor_colored", "colored_stage")),
            authority_overflow_pipeline: (execution.color_order
                == GpuColorOrderV4::AuthorityOverflowFirst)
                .then(|| pipeline("floor_authority_overflow", "authority_overflow_stage")),
            island_colored_pipeline: topology_layout
                .has_island_dispatch
                .then(|| pipeline("floor_island_colored", "island_colored_stage")),
            operations,
            capacities,
            point_count,
            body_count,
            topology_layout,
            execution,
            scheduling_policy,
            edge_build_permuted: permute_edge_build,
        })
    }

    pub(crate) fn encode_step(&self, encoder: &mut wgpu::CommandEncoder) {
        self.encode_dynamic_work(encoder);
        if let Some(split) = self
            .operations
            .iter()
            .position(|operation| operation.indirect_offset.is_some())
        {
            let mut preparation = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_fixed_floor_compact_color_prepare"),
                timestamp_writes: None,
            });
            for operation in &self.operations[..split] {
                self.encode_operation(&mut preparation, operation);
            }
            drop(preparation);
            let mut solve = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_fixed_floor_compact_color_solve"),
                timestamp_writes: None,
            });
            for operation in &self.operations[split..] {
                self.encode_operation(&mut solve, operation);
            }
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_fixed_floor_solve"),
            timestamp_writes: None,
        });
        for operation in &self.operations {
            self.encode_operation(&mut pass, operation);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn encode_step_with_pass_trace(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        destination: &wgpu::Buffer,
    ) -> Result<Vec<GpuSolverPassDescriptorV1>, String> {
        self.encode_dynamic_work(encoder);
        let descriptors = self.pass_trace_descriptors();
        let first_operation = descriptors
            .first()
            .map(|descriptor| descriptor.first_operation_index as usize)
            .ok_or_else(|| "solver pass trace has no state-changing operations".to_owned())?;
        let mut expected_operation = first_operation;
        for descriptor in &descriptors {
            if descriptor.first_operation_index as usize != expected_operation
                || descriptor.operation_index < descriptor.first_operation_index
            {
                return Err(
                    "solver pass trace requires contiguous logical operation groups".to_owned(),
                );
            }
            expected_operation = descriptor.operation_index as usize + 1;
        }
        if expected_operation != self.operations.len() {
            return Err("solver pass trace omitted a state-changing operation".to_owned());
        }
        if first_operation != 0 {
            let mut preparation = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_solver_pass_trace_topology"),
                timestamp_writes: None,
            });
            for operation in &self.operations[..first_operation] {
                self.encode_operation(&mut preparation, operation);
            }
        }
        self.copy_pass_trace_state(encoder, storage, destination, 0);
        let stride = self.pass_trace_state_bytes();
        for (index, descriptor) in descriptors.iter().enumerate() {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_solver_pass_trace_operation"),
                timestamp_writes: None,
            });
            for operation in &self.operations
                [descriptor.first_operation_index as usize..=descriptor.operation_index as usize]
            {
                self.encode_trace_operation(&mut pass, operation);
            }
            drop(pass);
            self.copy_pass_trace_state(encoder, storage, destination, (index as u64 + 1) * stride);
        }
        Ok(descriptors)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn encode_step_with_pass_motion_trace(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        destination: &wgpu::Buffer,
        destination_offset: u64,
    ) -> Result<Vec<GpuSolverPassDescriptorV1>, String> {
        self.encode_dynamic_work(encoder);
        let descriptors = self.pass_trace_descriptors();
        let first_operation = descriptors
            .first()
            .map(|descriptor| descriptor.first_operation_index as usize)
            .ok_or_else(|| {
                "solver pass-motion trace has no state-changing operations".to_owned()
            })?;
        let mut expected_operation = first_operation;
        for descriptor in &descriptors {
            if descriptor.first_operation_index as usize != expected_operation
                || descriptor.operation_index < descriptor.first_operation_index
            {
                return Err(
                    "solver pass-motion trace requires contiguous logical operation groups"
                        .to_owned(),
                );
            }
            expected_operation = descriptor.operation_index as usize + 1;
        }
        if expected_operation != self.operations.len() {
            return Err("solver pass-motion trace omitted a state-changing operation".to_owned());
        }
        if first_operation != 0 {
            let mut preparation = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_solver_pass_motion_trace_topology"),
                timestamp_writes: None,
            });
            for operation in &self.operations[..first_operation] {
                self.encode_operation(&mut preparation, operation);
            }
        }
        self.copy_pass_trace_motion(storage, encoder, destination, destination_offset);
        let stride = self.pass_trace_motion_bytes();
        for (index, descriptor) in descriptors.iter().enumerate() {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_solver_pass_motion_trace_operation"),
                timestamp_writes: None,
            });
            for operation in &self.operations
                [descriptor.first_operation_index as usize..=descriptor.operation_index as usize]
            {
                self.encode_trace_operation(&mut pass, operation);
            }
            drop(pass);
            self.copy_pass_trace_motion(
                storage,
                encoder,
                destination,
                destination_offset + (index as u64 + 1) * stride,
            );
        }
        Ok(descriptors)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_trace_state_bytes(&self) -> u64 {
        u64::from(self.body_count) * size_of::<[f32; 4]>() as u64 * 2
            + u64::from(self.point_count) * size_of::<PreparedPointRaw>() as u64
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_trace_motion_bytes(&self) -> u64 {
        u64::from(self.body_count) * size_of::<[f32; 4]>() as u64
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn copy_pass_trace_motion(
        &self,
        storage: &GpuBodyStorage,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
        destination_offset: u64,
    ) {
        encoder.copy_buffer_to_buffer(
            storage.motion_buffer(),
            0,
            destination,
            destination_offset,
            self.pass_trace_motion_bytes(),
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn copy_pass_trace_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &GpuBodyStorage,
        destination: &wgpu::Buffer,
        destination_offset: u64,
    ) {
        let body_bytes = u64::from(self.body_count) * size_of::<[f32; 4]>() as u64;
        let prepared_bytes = u64::from(self.point_count) * size_of::<PreparedPointRaw>() as u64;
        encoder.copy_buffer_to_buffer(
            storage.pose_buffer(),
            0,
            destination,
            destination_offset,
            body_bytes,
        );
        encoder.copy_buffer_to_buffer(
            storage.motion_buffer(),
            0,
            destination,
            destination_offset + body_bytes,
            body_bytes,
        );
        encoder.copy_buffer_to_buffer(
            &self._prepared,
            0,
            destination,
            destination_offset + body_bytes * 2,
            prepared_bytes,
        );
    }

    fn encode_operation<'a>(&'a self, pass: &mut wgpu::ComputePass<'a>, operation: &Operation) {
        self.encode_operation_with_dispatch(pass, operation, false);
    }

    fn encode_dynamic_work(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(work) = &self.dynamic_work else {
            return;
        };
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_floor_prepare_dynamic_work"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &work.bind_group, &[]);
        pass.set_pipeline(&work.counts_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
        pass.set_pipeline(&work.bodies_pipeline);
        pass.dispatch_workgroups_indirect(&work.body_indirect, ACTIVE_BODY_DISPATCH_64_OFFSET);
    }

    fn dynamic_dispatch(&self, operation: &Operation) -> Option<DynamicDispatch> {
        self.dynamic_work.as_ref()?;
        match operation.pipeline {
            PipelineKind::TopologyClear => Some(DynamicDispatch::Topology64),
            PipelineKind::TopologyScatter
            | PipelineKind::IntegrateVelocity
            | PipelineKind::IntegratePosition
            | PipelineKind::Gather => Some(DynamicDispatch::Body64),
            PipelineKind::TopologyCount
            | PipelineKind::ManifoldCount
            | PipelineKind::EdgeBuild
            | PipelineKind::ColorPropose
            | PipelineKind::ColorResolve
            | PipelineKind::ColorFinalize
            | PipelineKind::ColorBlockCount
            | PipelineKind::ColorBlockScatter
            | PipelineKind::Prepare
            | PipelineKind::Refresh
            | PipelineKind::Warm
            | PipelineKind::Normal
            | PipelineKind::NormalBlock
            | PipelineKind::Tangent
            | PipelineKind::Restitution => Some(DynamicDispatch::Point64),
            PipelineKind::Color if self.topology_layout.has_parallel_coloring => {
                Some(DynamicDispatch::Point64)
            }
            PipelineKind::EdgeRanges => Some(DynamicDispatch::DoublePoint64),
            PipelineKind::EdgeRadixHistograms | PipelineKind::EdgeRadixScatter => {
                Some(DynamicDispatch::DoublePoint256)
            }
            PipelineKind::ColorConflictClear => Some(DynamicDispatch::Winner64),
            _ => None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_trace_operation<'a>(
        &'a self,
        pass: &mut wgpu::ComputePass<'a>,
        operation: &Operation,
    ) {
        self.encode_operation_with_dispatch(pass, operation, true);
    }

    fn encode_operation_with_dispatch<'a>(
        &'a self,
        pass: &mut wgpu::ComputePass<'a>,
        operation: &Operation,
        trace_direct_colors: bool,
    ) {
        pass.set_pipeline(match operation.pipeline {
            PipelineKind::TopologyClear => &self.topology_clear_pipeline,
            PipelineKind::TopologyCount => &self.topology_count_pipeline,
            PipelineKind::ManifoldCount => self
                .manifold_count_pipeline
                .as_ref()
                .expect("manifold count operations require the candidate pipeline"),
            PipelineKind::TopologyPrefix => &self.topology_prefix_pipeline,
            PipelineKind::TopologyScatter => &self.topology_scatter_pipeline,
            PipelineKind::EdgeBuild => &self.edge_build_pipeline,
            PipelineKind::EdgeRadixHistograms => &self.edge_radix_histograms_pipeline,
            PipelineKind::EdgeRadixPrefix => &self.edge_radix_prefix_pipeline,
            PipelineKind::EdgeRadixScatter => &self.edge_radix_scatter_pipeline,
            PipelineKind::EdgeRanges => &self.edge_ranges_pipeline,
            PipelineKind::Color => self
                .color_pipeline
                .as_ref()
                .expect("color operations require the comparator pipeline"),
            PipelineKind::ColorConflictClear => self
                .color_conflict_clear_pipeline
                .as_ref()
                .expect("parallel coloring requires the conflict-clear pipeline"),
            PipelineKind::ColorPropose => self
                .color_propose_pipeline
                .as_ref()
                .expect("parallel coloring requires the proposal pipeline"),
            PipelineKind::ColorResolve => self
                .color_resolve_pipeline
                .as_ref()
                .expect("parallel coloring requires the conflict-resolution pipeline"),
            PipelineKind::ColorFinalize => self
                .color_finalize_pipeline
                .as_ref()
                .expect("parallel coloring requires the finalization pipeline"),
            PipelineKind::ColorCompact => self
                .color_compact_pipeline
                .as_ref()
                .expect("color compaction operations require the compact pipeline"),
            PipelineKind::ColorBlockCount => self
                .color_block_count_pipeline
                .as_ref()
                .expect("parallel color count operations require the count pipeline"),
            PipelineKind::ColorBlockPrefix => self
                .color_block_prefix_pipeline
                .as_ref()
                .expect("parallel color prefix operations require the block-prefix pipeline"),
            PipelineKind::ColorRangePrefix => self
                .color_range_prefix_pipeline
                .as_ref()
                .expect("parallel color range operations require the range-prefix pipeline"),
            PipelineKind::ColorBlockScatter => self
                .color_block_scatter_pipeline
                .as_ref()
                .expect("parallel color scatter operations require the scatter pipeline"),
            PipelineKind::IslandBuild => self
                .island_build_pipeline
                .as_ref()
                .expect("island-build operations require the island pipeline"),
            PipelineKind::IntegrateVelocity => &self.integrate_velocity_pipeline,
            PipelineKind::Prepare => &self.prepare_pipeline,
            PipelineKind::Refresh => &self.refresh_pipeline,
            PipelineKind::Warm => &self.warm_pipeline,
            PipelineKind::Normal => &self.normal_pipeline,
            PipelineKind::NormalBlock => self
                .normal_block_pipeline
                .as_ref()
                .expect("normal block operations require the candidate pipeline"),
            PipelineKind::Tangent => &self.tangent_pipeline,
            PipelineKind::Restitution => &self.restitution_pipeline,
            PipelineKind::IntegratePosition => &self.integrate_position_pipeline,
            PipelineKind::Gather => &self.gather_pipeline,
            PipelineKind::Colored => self
                .colored_pipeline
                .as_ref()
                .expect("colored operations require the comparator pipeline"),
            PipelineKind::AuthorityOverflow => self
                .authority_overflow_pipeline
                .as_ref()
                .expect("authority overflow operations require the candidate pipeline"),
            PipelineKind::IslandColored => self
                .island_colored_pipeline
                .as_ref()
                .expect("island-colored operations require the comparator pipeline"),
        });
        let bind_group = if matches!(
            operation.pipeline,
            PipelineKind::Color
                | PipelineKind::ColorConflictClear
                | PipelineKind::ColorPropose
                | PipelineKind::ColorResolve
                | PipelineKind::ColorFinalize
                | PipelineKind::ColorCompact
                | PipelineKind::ColorBlockCount
                | PipelineKind::ColorBlockPrefix
                | PipelineKind::ColorRangePrefix
                | PipelineKind::ColorBlockScatter
                | PipelineKind::IslandBuild
        ) {
            self.color_bind_ab
                .as_ref()
                .expect("color operations require the indirect-argument bind group")
        } else {
            match operation.direction {
                MotionDirection::AtoB => &self.bind_ab,
                MotionDirection::BtoA => &self.bind_ba,
            }
        };
        pass.set_bind_group(
            0,
            bind_group,
            &[operation.parameter_index * PARAMETER_STRIDE as u32],
        );
        if trace_direct_colors && operation.indirect_offset.is_some() {
            pass.dispatch_workgroups(workgroups(self.point_count), 1, 1);
        } else if let Some(offset) = operation.indirect_offset {
            pass.dispatch_workgroups_indirect(
                self._color_indirect
                    .as_ref()
                    .expect("indirect color operations require the argument buffer"),
                offset,
            );
        } else if let Some(dispatch) = self.dynamic_dispatch(operation) {
            let work = self
                .dynamic_work
                .as_ref()
                .expect("dynamic dispatch requires active-work resources");
            let (buffer, offset) = match dispatch {
                // Global topology counters still need one clearing invocation
                // when both active body and point work are empty. The prepared
                // body-work record carries a dedicated nonzero dispatch.
                DynamicDispatch::Topology64 => {
                    (work.body_indirect.as_ref(), TOPOLOGY_DISPATCH_64_OFFSET)
                }
                DynamicDispatch::Point64 => {
                    (work.point_indirect.as_ref(), POINT_DISPATCH_64_OFFSET)
                }
                DynamicDispatch::DoublePoint64 => (
                    work.point_indirect.as_ref(),
                    DOUBLE_POINT_DISPATCH_64_OFFSET,
                ),
                DynamicDispatch::DoublePoint256 => (
                    work.point_indirect.as_ref(),
                    DOUBLE_POINT_DISPATCH_256_OFFSET,
                ),
                DynamicDispatch::Body64 => {
                    (work.body_indirect.as_ref(), ACTIVE_BODY_DISPATCH_64_OFFSET)
                }
                DynamicDispatch::Winner64 => (
                    work.body_indirect.as_ref(),
                    ACTIVE_BODY_WINNER_DISPATCH_OFFSET,
                ),
            };
            pass.dispatch_workgroups_indirect(buffer, offset);
        } else {
            pass.dispatch_workgroups(operation.workgroups, 1, 1);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encode_step_with_timestamps(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_set: &wgpu::QuerySet,
        beginning: u32,
        end: u32,
    ) {
        self.encode_dynamic_work(encoder);
        if let Some(split) = self
            .operations
            .iter()
            .position(|operation| operation.indirect_offset.is_some())
        {
            let mut preparation = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_authored_gather_benchmark_color_prepare"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(beginning),
                    end_of_pass_write_index: None,
                }),
            });
            for operation in &self.operations[..split] {
                self.encode_operation(&mut preparation, operation);
            }
            drop(preparation);
            let mut solve = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_authored_gather_benchmark_compact_solve"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(end),
                }),
            });
            for operation in &self.operations[split..] {
                self.encode_operation(&mut solve, operation);
            }
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_authored_gather_benchmark_total"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(beginning),
                end_of_pass_write_index: Some(end),
            }),
        });
        for operation in &self.operations {
            self.encode_operation(&mut pass, operation);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn encode_step_instrumented(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_sets: &[&wgpu::QuerySet],
    ) -> Result<(), String> {
        self.encode_dynamic_work(encoder);
        let query_counts = self.instrumented_query_counts()?;
        if query_sets.len() != query_counts.len() {
            return Err("solver instrumented query-set partition is incomplete".to_owned());
        }
        let mut operation_offset = 0_usize;
        for (query_set, query_count) in query_sets.iter().zip(query_counts) {
            let operation_count = query_count as usize / 2;
            let end = operation_offset
                .checked_add(operation_count)
                .ok_or_else(|| "solver instrumented operation partition overflowed".to_owned())?;
            for (index, operation) in self.operations[operation_offset..end].iter().enumerate() {
                let beginning = index as u32 * 2;
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_authored_gather_benchmark_subphase"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: Some(beginning),
                        end_of_pass_write_index: Some(beginning + 1),
                    }),
                });
                self.encode_operation(&mut pass, operation);
            }
            operation_offset = end;
        }
        if operation_offset != self.operations.len() {
            return Err("solver instrumented query-set partition omitted operations".to_owned());
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn instrumented_query_count(&self) -> Result<u32, String> {
        u32::try_from(self.operations.len())
            .map_err(|_| "solver operation count exceeds u32".to_owned())?
            .checked_mul(2)
            .ok_or_else(|| "solver instrumented query count overflowed".to_owned())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn instrumented_query_counts(&self) -> Result<Vec<u32>, String> {
        let mut remaining = self.instrumented_query_count()?;
        let mut counts = Vec::new();
        while remaining != 0 {
            let count = remaining.min(MAX_INSTRUMENTED_QUERIES_PER_SET);
            if count % 2 != 0 {
                return Err("solver instrumented query partition split an operation".to_owned());
            }
            counts.push(count);
            remaining -= count;
        }
        Ok(counts)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn solver_instrumented_groups(&self, color_occupancy: &[u32]) -> Vec<SolverInstrumentedGroup> {
        let phase = |operation: &Operation| match operation.pipeline {
            PipelineKind::Color
            | PipelineKind::ColorConflictClear
            | PipelineKind::ColorPropose
            | PipelineKind::ColorResolve
            | PipelineKind::ColorFinalize => SolverInstrumentedPhase::ColorConstruction,
            PipelineKind::ColorCompact
            | PipelineKind::ColorBlockCount
            | PipelineKind::ColorBlockPrefix
            | PipelineKind::ColorRangePrefix
            | PipelineKind::ColorBlockScatter => SolverInstrumentedPhase::ColorCompaction,
            PipelineKind::IslandBuild => SolverInstrumentedPhase::ColorCompaction,
            PipelineKind::Colored => {
                let color = operation
                    .color
                    .expect("colored solver operations record their color");
                if color_occupancy.get(color as usize).copied().unwrap_or(0) == 0 {
                    SolverInstrumentedPhase::EmptyColorSolve
                } else {
                    SolverInstrumentedPhase::ActiveColorSolve
                }
            }
            PipelineKind::AuthorityOverflow => SolverInstrumentedPhase::ActiveColorSolve,
            PipelineKind::IslandColored => SolverInstrumentedPhase::ActiveColorSolve,
            _ => SolverInstrumentedPhase::OtherSolver,
        };
        let mut groups = Vec::<SolverInstrumentedGroup>::new();
        for (index, operation) in self.operations.iter().enumerate() {
            let operation_phase = phase(operation);
            if let Some(group) = groups.last_mut()
                && group.phase == operation_phase
            {
                group.end = index + 1;
                continue;
            }
            groups.push(SolverInstrumentedGroup {
                phase: operation_phase,
                start: index,
                end: index + 1,
            });
        }
        groups
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn solver_phase_instrumented_query_counts(
        &self,
        color_occupancy: &[u32],
    ) -> Result<Vec<u32>, String> {
        let mut remaining = u32::try_from(self.solver_instrumented_groups(color_occupancy).len())
            .map_err(|_| "solver instrumented phase-group count exceeds u32".to_owned())?
            .checked_mul(2)
            .ok_or_else(|| "solver instrumented phase-group query count overflowed".to_owned())?;
        let mut counts = Vec::new();
        while remaining != 0 {
            let count = remaining.min(MAX_INSTRUMENTED_QUERIES_PER_SET);
            if count % 2 != 0 {
                return Err("solver phase query partition split a group".to_owned());
            }
            counts.push(count);
            remaining -= count;
        }
        Ok(counts)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn encode_step_solver_phase_instrumented(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_sets: &[&wgpu::QuerySet],
        color_occupancy: &[u32],
    ) -> Result<(), String> {
        self.encode_dynamic_work(encoder);
        let groups = self.solver_instrumented_groups(color_occupancy);
        let query_counts = self.solver_phase_instrumented_query_counts(color_occupancy)?;
        if query_sets.len() != query_counts.len() {
            return Err("solver phase query-set partition is incomplete".to_owned());
        }
        let mut group_offset = 0_usize;
        for (query_set, query_count) in query_sets.iter().zip(query_counts) {
            let group_count = query_count as usize / 2;
            let end = group_offset
                .checked_add(group_count)
                .ok_or_else(|| "solver instrumented phase partition overflowed".to_owned())?;
            for (index, group) in groups[group_offset..end].iter().enumerate() {
                let beginning = index as u32 * 2;
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_solver_phase_benchmark"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: Some(beginning),
                        end_of_pass_write_index: Some(beginning + 1),
                    }),
                });
                for operation in &self.operations[group.start..group.end] {
                    self.encode_operation(&mut pass, operation);
                }
            }
            group_offset = end;
        }
        if group_offset != groups.len() {
            return Err("solver phase query-set partition omitted groups".to_owned());
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn decode_instrumented_sample(
        &self,
        values: &[u64],
        timestamp_period_nanoseconds: f32,
        color_occupancy: &[u32],
    ) -> Result<GpuSolverInstrumentedSampleV4, String> {
        let groups = self.solver_instrumented_groups(color_occupancy);
        if values.len() != groups.len() * 2 {
            return Err("solver instrumented sample has the wrong query count".to_owned());
        }
        let seconds = |pair: &[u64]| {
            pair[1].saturating_sub(pair[0]) as f64
                * f64::from(timestamp_period_nanoseconds)
                * 1.0e-9
        };
        let mut sample = GpuSolverInstrumentedSampleV4 {
            color_construction_seconds: 0.0,
            color_compaction_seconds: 0.0,
            active_color_solve_seconds: 0.0,
            empty_color_solve_seconds: 0.0,
            other_solver_seconds: 0.0,
            total_instrumented_seconds: 0.0,
        };
        for (group, pair) in groups.iter().zip(values.chunks_exact(2)) {
            let elapsed = seconds(pair);
            match group.phase {
                SolverInstrumentedPhase::ColorConstruction => {
                    sample.color_construction_seconds += elapsed;
                }
                SolverInstrumentedPhase::ColorCompaction => {
                    sample.color_compaction_seconds += elapsed;
                }
                SolverInstrumentedPhase::ActiveColorSolve => {
                    sample.active_color_solve_seconds += elapsed;
                }
                SolverInstrumentedPhase::EmptyColorSolve => {
                    sample.empty_color_solve_seconds += elapsed;
                }
                SolverInstrumentedPhase::OtherSolver => sample.other_solver_seconds += elapsed,
            }
            sample.total_instrumented_seconds += elapsed;
        }
        Ok(sample)
    }

    pub(crate) fn dispatches_per_step(&self) -> u32 {
        self.operations.len() as u32 + u32::from(self.dynamic_work.is_some()) * 2
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_trace_descriptors(&self) -> Vec<GpuSolverPassDescriptorV1> {
        pass_trace_descriptors(&self.parameters, &self.operations)
    }

    pub(crate) const fn point_count(&self) -> u32 {
        self.point_count
    }

    pub(crate) fn capacities(&self) -> &[CapacityRecordV1] {
        &self.capacities
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self._owned_point_input
            .as_ref()
            .map_or(0, wgpu::Buffer::size)
            + self._motion_scratch.size()
            + self._prepared.size()
            + self._contributions.size()
            + self._topology.size()
            + self._color_indirect.as_ref().map_or(0, wgpu::Buffer::size)
            + self._parameter_buffer.size()
    }

    pub(crate) fn gather_topology_evidence(&self) -> GpuGatherTopologyEvidence {
        GpuGatherTopologyEvidence {
            topology: self.topology_layout.kind,
            edge_build_permuted: self.edge_build_permuted,
            dispatches: self.topology_layout.dispatches(),
            allocated_bytes: self._topology.size()
                + self._color_indirect.as_ref().map_or(0, wgpu::Buffer::size),
            point_capacity: self.point_count,
            edge_capacity: self.topology_layout.edge_count,
            radix_passes: self.topology_layout.radix_passes,
            manifold_count_dispatches: self.topology_layout.has_manifold_counts as u32,
            coloring_dispatches: self.topology_layout.color_construction_dispatches(),
            color_compaction_dispatches: self.topology_layout.color_compaction_dispatches(),
            island_construction_dispatches: self.topology_layout.has_island_dispatch as u32,
            canonical_scan_attempts: if self.topology_layout.kind
                == GpuGatherTopology::CanonicalBodyScan
            {
                u64::from(self.body_count) * u64::from(self.point_count)
            } else {
                0
            },
            capacities: topology_capacities(
                self.body_count,
                self.point_count,
                self.topology_layout,
            ),
        }
    }

    pub(crate) fn prepared_buffer(&self) -> &wgpu::Buffer {
        &self._prepared
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pass_evidence_layout(&self) -> FixedFloorEvidenceLayout {
        let prepared_bytes = u64::from(self.point_count) * size_of::<PreparedPointRaw>() as u64;
        let contribution_bytes =
            u64::from(self.point_count * 2) * size_of::<ContributionRaw>() as u64;
        let color_topology_bytes = if self.topology_layout.has_coloring {
            u64::from(self.topology_layout.element_count - self.topology_layout.color_base)
                * size_of::<u32>() as u64
        } else {
            0
        };
        let color_indirect_bytes = self._color_indirect.as_ref().map_or(0, wgpu::Buffer::size);
        let coloring_bytes = color_topology_bytes + color_indirect_bytes;
        FixedFloorEvidenceLayout {
            point_count: self.point_count,
            body_count: self.body_count,
            color_schedule: self.execution.color_schedule,
            color_unit: self.execution.color_unit,
            color_order: self.execution.color_order,
            scheduling_policy: self.scheduling_policy,
            topology_layout: self.topology_layout,
            prepared_bytes,
            contribution_bytes,
            color_topology_bytes,
            color_indirect_bytes,
            coloring_bytes,
            total_bytes: prepared_bytes + contribution_bytes + coloring_bytes,
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
        let contribution_bytes = layout.contribution_bytes;
        encoder.copy_buffer_to_buffer(&self._prepared, 0, destination, 0, prepared_bytes);
        encoder.copy_buffer_to_buffer(
            &self._contributions,
            0,
            destination,
            prepared_bytes,
            contribution_bytes,
        );
        if layout.color_topology_bytes != 0 {
            encoder.copy_buffer_to_buffer(
                &self._topology,
                u64::from(self.topology_layout.color_base) * size_of::<u32>() as u64,
                destination,
                prepared_bytes + contribution_bytes,
                layout.color_topology_bytes,
            );
        }
        if let Some(indirect) = &self._color_indirect {
            encoder.copy_buffer_to_buffer(
                indirect,
                0,
                destination,
                prepared_bytes + contribution_bytes + layout.color_topology_bytes,
                layout.color_indirect_bytes,
            );
        }
        layout
    }

    pub(crate) fn color_status_source(&self) -> Option<(&wgpu::Buffer, u64)> {
        self.topology_layout.has_coloring.then_some((
            &self._topology,
            u64::from(self.topology_layout.color_status_index) * size_of::<u32>() as u64,
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_authored_gather_topology(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    topology: GpuGatherTopology,
) -> Result<GpuAuthoredGatherBenchmarkOutput, String> {
    validate_authored_settings(scenario)?;
    benchmark_gpu_authored_solver_experiment(
        scenario,
        warmup_steps,
        samples,
        topology,
        GpuSolverExecutionV4::experiment(true, true),
        GpuSolverSchedulingPolicyV1::STANDARD,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_authored_solver_experiment(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
) -> Result<GpuAuthoredGatherBenchmarkOutput, String> {
    benchmark_gpu_authored_solver_policy_experiment(
        scenario,
        warmup_steps,
        samples,
        topology,
        execution,
        scheduling_policy,
        GpuContactCoefficientPolicyV1::SELECTED,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_authored_solver_policy_experiment(
    scenario: &ScenarioV1,
    warmup_steps: u32,
    samples: u32,
    topology: GpuGatherTopology,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
) -> Result<GpuAuthoredGatherBenchmarkOutput, String> {
    if warmup_steps == 0 || samples == 0 {
        return Err("authored gather benchmark requires nonzero warmup and samples".to_owned());
    }
    scenario.validate().map_err(|error| error.to_string())?;
    let (_, point_count) = point_inputs(scenario)?;
    if point_count == 0 {
        return Err("authored gather benchmark requires contact points".to_owned());
    }
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "authored gather body count exceeds u32".to_owned())?;
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
    let storage = GpuBodyStorage::new(
        &device,
        body_count,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_count, &scenario.bodies)?;
    let solver = GpuFixedFloorSolver::new_authored_with_topology_and_coefficients(
        &device,
        &storage,
        scenario,
        topology,
        execution,
        scheduling_policy,
        contact_coefficient_policy,
    )?;
    let (first_digest, first_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &solver)?;
    let scheduler_work = first_evidence.scheduler_work_vector(body_count)?;
    let color_occupancy = first_evidence
        .coloring
        .as_ref()
        .map_or_else(Vec::new, |coloring| coloring.color_occupancy.clone());
    let total_query_count = samples
        .checked_mul(2)
        .ok_or_else(|| "authored gather total query count overflowed".to_owned())?;
    let instrumented_query_counts = solver.instrumented_query_counts()?;
    let total_query_resources = timestamps_available.then(|| {
        BenchmarkTimestampResources::new(
            &device,
            "physbox5_authored_gather_total_queries",
            total_query_count,
        )
    });
    let instrumented_query_resources = if timestamps_available && !execution.uses_colored_in_place()
    {
        (0..samples)
            .map(|_| {
                instrumented_query_counts
                    .iter()
                    .map(|&query_count| {
                        BenchmarkTimestampResources::new(
                            &device,
                            "physbox5_authored_gather_subphase_queries",
                            query_count,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let solver_phase_query_resources = if timestamps_available {
        let query_counts = solver.solver_phase_instrumented_query_counts(&color_occupancy)?;
        (0..samples)
            .map(|_| {
                query_counts
                    .iter()
                    .map(|&query_count| {
                        BenchmarkTimestampResources::new(
                            &device,
                            "physbox5_authored_solver_phase_queries",
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
            .map_err(|error| format!("poll authored gather warmup reset: {error}"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_warmup"),
        });
        solver.encode_step(&mut encoder);
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather warmup: {error}"))?;
    }

    let mut host_completion_seconds = Vec::with_capacity(samples as usize);
    let mut cpu_encode_submit_seconds = Vec::with_capacity(samples as usize);
    for sample in 0..samples {
        storage.upload_reset_data(&queue, &reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather sample reset: {error}"))?;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("physbox5_authored_gather_sample"),
        });
        if let Some(resources) = &total_query_resources {
            solver.encode_step_with_timestamps(
                &mut encoder,
                &resources.query_set,
                sample * 2,
                sample * 2 + 1,
            );
        } else {
            solver.encode_step(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        cpu_encode_submit_seconds.push(started.elapsed().as_secs_f64());
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll authored gather sample: {error}"))?;
        host_completion_seconds.push(started.elapsed().as_secs_f64());
    }

    if timestamps_available {
        for resources in &instrumented_query_resources {
            storage.upload_reset_data(&queue, &reset);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored gather subphase reset: {error}"))?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_authored_gather_subphase_sample"),
            });
            let query_sets = resources
                .iter()
                .map(|resources| &resources.query_set)
                .collect::<Vec<_>>();
            solver.encode_step_instrumented(&mut encoder, &query_sets)?;
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored gather subphase sample: {error}"))?;
        }
        for resources in &solver_phase_query_resources {
            storage.upload_reset_data(&queue, &reset);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored solver phase reset: {error}"))?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("physbox5_authored_solver_phase_sample"),
            });
            let query_sets = resources
                .iter()
                .map(|resources| &resources.query_set)
                .collect::<Vec<_>>();
            solver.encode_step_solver_phase_instrumented(
                &mut encoder,
                &query_sets,
                &color_occupancy,
            )?;
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait)
                .map_err(|error| format!("poll authored solver phase sample: {error}"))?;
        }
    }

    let mut gpu_total_seconds = None;
    let mut index_build_seconds = None;
    let mut offset_construction_seconds = None;
    let mut gather_seconds = None;
    let mut solve_seconds = None;
    let mut timestamp_period_nanoseconds = None;
    if let Some(resources) = &total_query_resources {
        let values = resources.resolve_and_map(&device, &queue)?;
        let period = queue.get_timestamp_period();
        let seconds =
            |start: u64, end: u64| end.saturating_sub(start) as f64 * f64::from(period) * 1.0e-9;
        let total = values
            .chunks_exact(2)
            .map(|pair| seconds(pair[0], pair[1]))
            .collect::<Vec<_>>();
        if !instrumented_query_resources.is_empty() {
            let mut index = Vec::with_capacity(samples as usize);
            let mut offsets = Vec::with_capacity(samples as usize);
            let mut gather = Vec::with_capacity(samples as usize);
            let mut solve = Vec::with_capacity(samples as usize);
            for resources in &instrumented_query_resources {
                let mut sample_values = Vec::new();
                for resources in resources {
                    sample_values.extend(resources.resolve_and_map(&device, &queue)?);
                }
                let mut totals = [0.0_f64; 4];
                for (operation, pair) in solver.operations.iter().zip(sample_values.chunks_exact(2))
                {
                    let phase = match operation.pipeline.instrumented_phase() {
                        InstrumentedPhase::IndexBuild => 0,
                        InstrumentedPhase::OffsetConstruction => 1,
                        InstrumentedPhase::Gather => 2,
                        InstrumentedPhase::Solve => 3,
                    };
                    totals[phase] += seconds(pair[0], pair[1]);
                }
                index.push(totals[0]);
                offsets.push(totals[1]);
                gather.push(totals[2]);
                solve.push(totals[3]);
            }
            index_build_seconds = Some(index);
            offset_construction_seconds = Some(offsets);
            gather_seconds = Some(gather);
            solve_seconds = Some(solve);
        }
        gpu_total_seconds = Some(total);
        timestamp_period_nanoseconds = Some(period);
    }

    let instrumented_solver_phases = if timestamps_available {
        let period = queue.get_timestamp_period();
        Some(
            solver_phase_query_resources
                .iter()
                .map(|resources| {
                    let mut values = Vec::new();
                    for resources in resources {
                        values.extend(resources.resolve_and_map(&device, &queue)?);
                    }
                    solver.decode_instrumented_sample(&values, period, &color_occupancy)
                })
                .collect::<Result<Vec<_>, String>>()?,
        )
    } else {
        None
    };

    let (second_digest, second_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &solver)?;
    let replay_exact = first_digest == second_digest && first_evidence == second_evidence;
    if !replay_exact {
        return Err("authored gather benchmark replay differed".to_owned());
    }
    let mut capacities = vec![storage.body_capacity_record()];
    capacities.extend_from_slice(solver.capacities());
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    for capacity in &capacities {
        capacity.validate().map_err(|error| error.to_string())?;
        if capacity.overflowed {
            return Err(format!(
                "authored gather benchmark overflowed {}",
                capacity.class
            ));
        }
    }
    Ok(GpuAuthoredGatherBenchmarkOutput {
        adapter: diagnostics,
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        body_count,
        manifold_count: u32::try_from(scenario.manifolds.len())
            .map_err(|_| "authored gather manifold count exceeds u32".to_owned())?,
        contact_points: point_count,
        warmup_steps,
        samples,
        solver_execution: execution,
        solver_scheduling_policy: scheduling_policy,
        contact_coefficient_policy,
        scheduler_work,
        topology: solver.gather_topology_evidence(),
        dispatches_per_step: solver.dispatches_per_step(),
        allocated_bytes: storage.layout().allocated_bytes + solver.allocated_bytes(),
        gpu_total_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        instrumented_index_build_seconds: index_build_seconds,
        instrumented_offset_construction_seconds: offset_construction_seconds,
        instrumented_gather_seconds: gather_seconds,
        instrumented_solve_seconds: solve_seconds,
        instrumented_solver_phases,
        timestamp_period_nanoseconds,
        replay_exact,
        result_digest: first_digest,
        capacities,
    })
}

/// Runs the frozen S13 selected/candidate timing pair on one adapter. Each
/// warm-up and retained sample resets the same body storage, and retained
/// invocation order alternates by sample.
#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_s13_authority_probe(
    scenario: &ScenarioV1,
    warmup_samples: u32,
    retained_samples: u32,
) -> Result<GpuS13AuthorityProbeBenchmarkV1, String> {
    benchmark_gpu_authority_derived_pair(scenario, warmup_samples, retained_samples, false).await
}

/// Runs the S17 selected/branch-robust timing pair under the unchanged S13
/// complete-step protocol.
#[cfg(not(target_arch = "wasm32"))]
pub async fn benchmark_gpu_s17_branch_robust_promotion(
    scenario: &ScenarioV1,
    warmup_samples: u32,
    retained_samples: u32,
) -> Result<GpuS13AuthorityProbeBenchmarkV1, String> {
    crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
    benchmark_gpu_authority_derived_pair(scenario, warmup_samples, retained_samples, true).await
}

#[cfg(not(target_arch = "wasm32"))]
async fn benchmark_gpu_authority_derived_pair(
    scenario: &ScenarioV1,
    warmup_samples: u32,
    retained_samples: u32,
    branch_robust: bool,
) -> Result<GpuS13AuthorityProbeBenchmarkV1, String> {
    if warmup_samples == 0 || retained_samples == 0 {
        return Err("S13 benchmark requires nonzero warm-up and retained samples".to_owned());
    }
    scenario.validate().map_err(|error| error.to_string())?;
    crate::authority_gpu_probe::validate_authority_gpu_probe_settings(&scenario.settings)?;
    let (_, point_count) = point_inputs(scenario)?;
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "S13 benchmark body count exceeds u32".to_owned())?;
    let manifold_count = u32::try_from(scenario.manifolds.len())
        .map_err(|_| "S13 benchmark manifold count exceeds u32".to_owned())?;
    let mut selected_scenario = scenario.clone();
    selected_scenario.settings.contact_softness = None;
    validate_selected_settings(&selected_scenario)?;

    let instance = crate::gpu_context::create_instance();
    let adapter = crate::gpu_context::request_adapter(&instance, None).await?;
    if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        return Err("S13 benchmark requires supported GPU timestamp queries".to_owned());
    }
    let diagnostics = crate::AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) =
        crate::gpu_context::request_device_with_features(&adapter, wgpu::Features::TIMESTAMP_QUERY)
            .await?;
    let storage = GpuBodyStorage::new(
        &device,
        body_count,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_count, &scenario.bodies)?;
    let selected_scheduling = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(body_count);
    let selected_solver = GpuFixedFloorSolver::new_authored_with_topology_and_coefficients(
        &device,
        &storage,
        &selected_scenario,
        GpuGatherTopology::StableBodyEdge,
        GpuSolverExecutionV4::SELECTED,
        selected_scheduling,
        GpuContactCoefficientPolicyV1::SELECTED,
    )?;
    let candidate_solver = if branch_robust {
        GpuFixedFloorSolver::new_authored_branch_robust_gpu_promotion(&device, &storage, scenario)?
    } else {
        GpuFixedFloorSolver::new_authored_authority_gpu_probe(&device, &storage, scenario)?
    };

    let (selected_digest, selected_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &selected_solver)?;
    let (candidate_digest, candidate_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &candidate_solver)?;
    let selected_coloring = selected_evidence
        .coloring
        .clone()
        .ok_or_else(|| "S13 selected control omitted coloring evidence".to_owned())?;
    let candidate_coloring = candidate_evidence
        .coloring
        .clone()
        .ok_or_else(|| "S13 candidate omitted coloring evidence".to_owned())?;

    let selected_queries = BenchmarkTimestampResources::new(
        &device,
        "physbox5_s13_selected_complete_step",
        retained_samples
            .checked_mul(2)
            .ok_or_else(|| "S13 selected timestamp count overflowed".to_owned())?,
    );
    let candidate_queries = BenchmarkTimestampResources::new(
        &device,
        "physbox5_s13_candidate_complete_step",
        retained_samples
            .checked_mul(2)
            .ok_or_else(|| "S13 candidate timestamp count overflowed".to_owned())?,
    );

    let execute = |label: &'static str,
                   solver: &GpuFixedFloorSolver,
                   timestamp: Option<(&BenchmarkTimestampResources, u32)>|
     -> Result<(f64, f64), String> {
        storage.upload_reset_data(&queue, &reset);
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll S13 {label} reset: {error}"))?;
        let started = Instant::now();
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        if let Some((resources, sample)) = timestamp {
            solver.encode_step_with_timestamps(
                &mut encoder,
                &resources.query_set,
                sample * 2,
                sample * 2 + 1,
            );
        } else {
            solver.encode_step(&mut encoder);
        }
        queue.submit([encoder.finish()]);
        let cpu = started.elapsed().as_secs_f64();
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|error| format!("poll S13 {label} completion: {error}"))?;
        Ok((cpu, started.elapsed().as_secs_f64()))
    };

    for sample in 0..warmup_samples {
        if sample % 2 == 0 {
            execute("physbox5_s13_selected_warmup", &selected_solver, None)?;
            execute("physbox5_s13_candidate_warmup", &candidate_solver, None)?;
        } else {
            execute("physbox5_s13_candidate_warmup", &candidate_solver, None)?;
            execute("physbox5_s13_selected_warmup", &selected_solver, None)?;
        }
    }

    let mut invocation_order = Vec::with_capacity(retained_samples as usize);
    let mut selected_cpu = vec![0.0_f64; retained_samples as usize];
    let mut candidate_cpu = vec![0.0_f64; retained_samples as usize];
    let mut selected_host = vec![0.0_f64; retained_samples as usize];
    let mut candidate_host = vec![0.0_f64; retained_samples as usize];
    for sample in 0..retained_samples {
        if sample % 2 == 0 {
            invocation_order.push(["selected", "candidate"]);
            (
                selected_cpu[sample as usize],
                selected_host[sample as usize],
            ) = execute(
                "physbox5_s13_selected_sample",
                &selected_solver,
                Some((&selected_queries, sample)),
            )?;
            (
                candidate_cpu[sample as usize],
                candidate_host[sample as usize],
            ) = execute(
                "physbox5_s13_candidate_sample",
                &candidate_solver,
                Some((&candidate_queries, sample)),
            )?;
        } else {
            invocation_order.push(["candidate", "selected"]);
            (
                candidate_cpu[sample as usize],
                candidate_host[sample as usize],
            ) = execute(
                "physbox5_s13_candidate_sample",
                &candidate_solver,
                Some((&candidate_queries, sample)),
            )?;
            (
                selected_cpu[sample as usize],
                selected_host[sample as usize],
            ) = execute(
                "physbox5_s13_selected_sample",
                &selected_solver,
                Some((&selected_queries, sample)),
            )?;
        }
    }
    let period = queue.get_timestamp_period();
    let decode_times = |values: Vec<u64>| {
        values
            .chunks_exact(2)
            .map(|pair| pair[1].saturating_sub(pair[0]) as f64 * f64::from(period) * 1.0e-9)
            .collect::<Vec<_>>()
    };
    let selected_gpu = decode_times(selected_queries.resolve_and_map(&device, &queue)?);
    let candidate_gpu = decode_times(candidate_queries.resolve_and_map(&device, &queue)?);

    let (selected_replay_digest, selected_replay_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &selected_solver)?;
    let (candidate_replay_digest, candidate_replay_evidence) =
        authored_gather_result_evidence(&device, &queue, &storage, &reset, &candidate_solver)?;
    let selected_replay_exact =
        selected_digest == selected_replay_digest && selected_evidence == selected_replay_evidence;
    let candidate_replay_exact = candidate_digest == candidate_replay_digest
        && candidate_evidence == candidate_replay_evidence;
    if !selected_replay_exact || !candidate_replay_exact {
        return Err(format!(
            "S13 benchmark replay differed: selected={selected_replay_exact} candidate={candidate_replay_exact} selected_digests={selected_digest}/{selected_replay_digest} candidate_digests={candidate_digest}/{candidate_replay_digest} selected_parts={}/{}/{}/{} candidate_parts={}/{}/{}/{}",
            selected_evidence.contacts == selected_replay_evidence.contacts,
            selected_evidence.contributions == selected_replay_evidence.contributions,
            selected_evidence.manifold_blocks == selected_replay_evidence.manifold_blocks,
            selected_evidence.coloring == selected_replay_evidence.coloring,
            candidate_evidence.contacts == candidate_replay_evidence.contacts,
            candidate_evidence.contributions == candidate_replay_evidence.contributions,
            candidate_evidence.manifold_blocks == candidate_replay_evidence.manifold_blocks,
            candidate_evidence.coloring == candidate_replay_evidence.coloring,
        ));
    }
    let capacity_records = |solver: &GpuFixedFloorSolver| -> Result<Vec<CapacityRecordV1>, String> {
        let mut records = vec![storage.body_capacity_record()];
        records.extend_from_slice(solver.capacities());
        records.sort_by(|left, right| left.class.cmp(&right.class));
        for record in &records {
            record.validate().map_err(|error| error.to_string())?;
            if record.overflowed {
                return Err(format!("S13 benchmark overflowed {}", record.class));
            }
        }
        Ok(records)
    };
    let selected_allocated = storage.layout().allocated_bytes + selected_solver.allocated_bytes();
    let candidate_allocated = storage.layout().allocated_bytes + candidate_solver.allocated_bytes();
    Ok(GpuS13AuthorityProbeBenchmarkV1 {
        adapter: diagnostics,
        candidate_source_scenario_hash: canonical_hash(scenario)
            .map_err(|error| error.to_string())?,
        selected_projected_source_scenario_hash: canonical_hash(&selected_scenario)
            .map_err(|error| error.to_string())?,
        body_count,
        manifold_count,
        contact_points: point_count,
        warmup_samples,
        retained_samples,
        invocation_order,
        timestamp_period_nanoseconds: period,
        candidate_specific_transform_bytes: u64::from(body_count) * 3 * 4,
        candidate_allocation_delta_bytes: i64::try_from(candidate_allocated)
            .map_err(|_| "candidate allocation exceeds i64".to_owned())?
            - i64::try_from(selected_allocated)
                .map_err(|_| "selected allocation exceeds i64".to_owned())?,
        selected: GpuS13MeasuredConfigurationV1 {
            configuration: "selected-hard-direct",
            solver_execution: GpuSolverExecutionV4::SELECTED,
            solver_scheduling_policy: selected_scheduling,
            topology: selected_solver.gather_topology_evidence(),
            pass_descriptors: selected_solver.pass_trace_descriptors(),
            coloring: selected_coloring,
            dispatches_per_step: selected_solver.dispatches_per_step(),
            allocated_bytes: selected_allocated,
            gpu_complete_step_seconds: selected_gpu,
            host_complete_step_seconds: selected_host,
            cpu_encode_submit_seconds: selected_cpu,
            capacities: capacity_records(&selected_solver)?,
            replay_exact: selected_replay_exact,
            result_digest: selected_digest,
        },
        candidate: GpuS13MeasuredConfigurationV1 {
            configuration: if branch_robust {
                crate::BRANCH_ROBUST_GPU_PROMOTION_CANDIDATE_ID
            } else {
                crate::AUTHORITY_GPU_PROBE_CANDIDATE_ID
            },
            solver_execution: GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            solver_scheduling_policy: candidate_solver.scheduling_policy,
            topology: candidate_solver.gather_topology_evidence(),
            pass_descriptors: candidate_solver.pass_trace_descriptors(),
            coloring: candidate_coloring,
            dispatches_per_step: candidate_solver.dispatches_per_step(),
            allocated_bytes: candidate_allocated,
            gpu_complete_step_seconds: candidate_gpu,
            host_complete_step_seconds: candidate_host,
            cpu_encode_submit_seconds: candidate_cpu,
            capacities: capacity_records(&candidate_solver)?,
            replay_exact: candidate_replay_exact,
            result_digest: candidate_digest,
        },
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn authority_gpu_probe_capacity_disposition(
    scenario: &ScenarioV1,
    manifold_capacity: u32,
) -> Result<GpuAuthorityProbeCapacityDispositionV1, String> {
    authority_derived_capacity_disposition(scenario, manifold_capacity, false)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn branch_robust_gpu_promotion_capacity_disposition(
    scenario: &ScenarioV1,
    manifold_capacity: u32,
) -> Result<GpuAuthorityProbeCapacityDispositionV1, String> {
    authority_derived_capacity_disposition(scenario, manifold_capacity, true)
}

#[cfg(not(target_arch = "wasm32"))]
fn authority_derived_capacity_disposition(
    scenario: &ScenarioV1,
    manifold_capacity: u32,
    branch_robust: bool,
) -> Result<GpuAuthorityProbeCapacityDispositionV1, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    if branch_robust {
        crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::branch_robust_gpu_promotion::validate_branch_robust_gpu_promotion_settings(
            &scenario.settings,
        )?;
    } else {
        crate::AuthorityGpuProbePolicyV1::AUTHORIZED.validate_product_selection(false)?;
        crate::authority_gpu_probe::validate_authority_gpu_probe_settings(&scenario.settings)?;
    }
    let attempted = u32::try_from(scenario.manifolds.len())
        .map_err(|_| "authored authority manifold demand exceeds u32".to_owned())?;
    let rejected = attempted > manifold_capacity;
    Ok(GpuAuthorityProbeCapacityDispositionV1 {
        capacity_class: "solver-manifolds",
        capacity: manifold_capacity,
        attempted,
        accepted: if rejected { 0 } else { attempted },
        committed: if rejected { 0 } else { attempted },
        overflowed: rejected,
        rejected,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_authored_authority_probe(
    scenario: &ScenarioV1,
) -> Result<GpuAuthoredAuthorityProbeOutputV1, String> {
    run_gpu_authored_authority_derived_probe(scenario, false).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_gpu_authored_branch_robust_promotion(
    scenario: &ScenarioV1,
) -> Result<GpuAuthoredAuthorityProbeOutputV1, String> {
    crate::BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
    run_gpu_authored_authority_derived_probe(scenario, true).await
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_gpu_authored_authority_derived_probe(
    scenario: &ScenarioV1,
    branch_robust: bool,
) -> Result<GpuAuthoredAuthorityProbeOutputV1, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    let body_count = u32::try_from(scenario.bodies.len())
        .map_err(|_| "authored authority body count exceeds u32".to_owned())?;
    let instance = crate::gpu_context::create_instance();
    let adapter = crate::gpu_context::request_adapter(&instance, None).await?;
    let diagnostics = crate::AdapterDiagnostics::from_adapter(&adapter);
    let (device, queue) = crate::gpu_context::request_device(&adapter).await?;
    let storage = GpuBodyStorage::new(
        &device,
        body_count,
        crate::BODY_COMMAND_CAPACITY,
        &scenario.bodies,
    )?;
    let reset = crate::body_state::GpuBodyResetData::new(body_count, &scenario.bodies)?;
    let solver = if branch_robust {
        GpuFixedFloorSolver::new_authored_branch_robust_gpu_promotion(&device, &storage, scenario)?
    } else {
        GpuFixedFloorSolver::new_authored_authority_gpu_probe(&device, &storage, scenario)?
    };
    let first =
        authored_authority_probe_result(&device, &queue, &storage, &reset, &solver, scenario)?;
    let replay =
        authored_authority_probe_result(&device, &queue, &storage, &reset, &solver, scenario)?;
    let replay_exact = first == replay;
    if !replay_exact {
        return Err("authored authority probe replay differed".to_owned());
    }
    let (result_digest, pass_evidence, bodies) = first;
    let mut capacities = vec![storage.body_capacity_record()];
    capacities.extend_from_slice(solver.capacities());
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    if capacities.iter().any(|capacity| capacity.overflowed) {
        return Err("authored authority probe overflowed bounded storage".to_owned());
    }
    Ok(GpuAuthoredAuthorityProbeOutputV1 {
        adapter: diagnostics,
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        solver_execution: solver.execution,
        solver_scheduling_policy: solver.scheduling_policy,
        pass_descriptors: solver.pass_trace_descriptors(),
        bodies,
        pass_evidence,
        capacities,
        dispatches_per_step: solver.dispatches_per_step(),
        allocated_bytes: storage.layout().allocated_bytes + solver.allocated_bytes(),
        replay_exact,
        result_digest,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn authored_authority_probe_result(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    storage: &GpuBodyStorage,
    reset: &crate::body_state::GpuBodyResetData,
    solver: &GpuFixedFloorSolver,
    scenario: &ScenarioV1,
) -> Result<
    (
        String,
        GpuFixedFloorPassEvidence,
        Vec<physbox_contracts::BodyStateV1>,
    ),
    String,
> {
    storage.upload_reset_data(queue, reset);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("poll authored authority reset: {error}"))?;
    let lane_bytes = storage.pose_buffer().size();
    let state_bytes = lane_bytes * 2;
    let state_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_authored_authority_state_readback"),
        size: state_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let evidence_layout = solver.pass_evidence_layout();
    let evidence_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_authored_authority_evidence_readback"),
        size: evidence_layout.total_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_authored_authority_step"),
    });
    solver.encode_step(&mut encoder);
    encoder.copy_buffer_to_buffer(storage.pose_buffer(), 0, &state_readback, 0, lane_bytes);
    encoder.copy_buffer_to_buffer(
        storage.motion_buffer(),
        0,
        &state_readback,
        lane_bytes,
        lane_bytes,
    );
    solver.copy_pass_evidence(&mut encoder, &evidence_readback);
    queue.submit([encoder.finish()]);
    let state = crate::collision_candidates::map_values::<[f32; 4]>(
        device,
        &state_readback,
        (state_bytes as usize) / size_of::<[f32; 4]>(),
    )?;
    let body_count = scenario.bodies.len();
    let mut bodies = scenario.bodies.clone();
    for (index, body) in bodies.iter_mut().enumerate() {
        let pose = state[index];
        let motion = state[body_count + index];
        body.position.x = pose[0];
        body.position.y = pose[1];
        body.angle =
            physbox_contracts::FiniteF32::new(pose[2]).map_err(|error| error.to_string())?;
        body.linear_velocity.x = motion[0];
        body.linear_velocity.y = motion[1];
        body.angular_velocity =
            physbox_contracts::FiniteF32::new(motion[2]).map_err(|error| error.to_string())?;
    }
    let evidence_bytes = crate::collision_candidates::map_values::<u8>(
        device,
        &evidence_readback,
        evidence_layout.total_bytes as usize,
    )?;
    let evidence = decode_pass_evidence(&evidence_bytes, evidence_layout)?;
    Ok((sha256_bytes(bytemuck::cast_slice(&state)), evidence, bodies))
}

#[cfg(not(target_arch = "wasm32"))]
fn authored_gather_result_evidence(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    storage: &GpuBodyStorage,
    reset: &crate::body_state::GpuBodyResetData,
    solver: &GpuFixedFloorSolver,
) -> Result<(String, GpuFixedFloorPassEvidence), String> {
    storage.upload_reset_data(queue, reset);
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|error| format!("poll authored gather digest reset: {error}"))?;
    let lane_bytes = storage.pose_buffer().size();
    let evidence_layout = solver.pass_evidence_layout();
    let result_bytes = lane_bytes * 2;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_authored_gather_result_readback"),
        size: result_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let evidence_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("physbox5_authored_gather_evidence_readback"),
        size: evidence_layout.total_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("physbox5_authored_gather_result_digest"),
    });
    solver.encode_step(&mut encoder);
    encoder.copy_buffer_to_buffer(storage.pose_buffer(), 0, &readback, 0, lane_bytes);
    encoder.copy_buffer_to_buffer(
        storage.motion_buffer(),
        0,
        &readback,
        lane_bytes,
        lane_bytes,
    );
    solver.copy_pass_evidence(&mut encoder, &evidence_readback);
    queue.submit([encoder.finish()]);
    let values =
        crate::collision_candidates::map_values::<u8>(device, &readback, result_bytes as usize)?;
    let digest = sha256_bytes(&values);
    let evidence_values = crate::collision_candidates::map_values::<u8>(
        device,
        &evidence_readback,
        evidence_layout.total_bytes as usize,
    )?;
    let evidence = decode_pass_evidence(&evidence_values, evidence_layout)?;
    Ok((digest, evidence))
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
    let contribution_start = layout.prepared_bytes as usize;
    let contribution_end = (layout.prepared_bytes + layout.contribution_bytes) as usize;
    let raw_contributions = bytemuck::try_cast_slice::<u8, ContributionRaw>(
        &bytes[contribution_start..contribution_end],
    )
    .map_err(|error| format!("decode fixed-floor contributions: {error}"))?;
    let active_point_count = if layout.topology_layout.element_count
        > layout.topology_layout.work_state_base
        && layout.coloring_bytes != 0
    {
        let work_offset = contribution_end
            + (layout.topology_layout.work_state_base - layout.topology_layout.color_base) as usize
                * size_of::<u32>();
        let work_end = work_offset + size_of::<u32>();
        let count = u32::from_le_bytes(
            bytes
                .get(work_offset..work_end)
                .ok_or_else(|| "solver active point count was outside evidence".to_owned())?
                .try_into()
                .map_err(|_| "solver active point count had the wrong width".to_owned())?,
        );
        count.min(layout.point_count) as usize
    } else {
        layout.point_count as usize
    };
    let mut contacts = decode_prepared_contacts(prepared);
    for contact in &mut contacts[active_point_count..] {
        contact.valid = false;
    }
    let manifold_blocks = prepared
        .iter()
        .take(active_point_count)
        .enumerate()
        .filter_map(|(index, point)| {
            if point.ids[0] == u32::MAX || point.ids[1] == u32::MAX || point.ids[3] != 0 {
                return None;
            }
            let first_contribution = index * 2;
            let k11 = f32::from_bits(raw_contributions[first_contribution].padding);
            if !k11.is_finite() || k11 <= 0.0 {
                return None;
            }
            let has_second = index + 1 < prepared.len()
                && prepared[index + 1].ids[0] != u32::MAX
                && prepared[index + 1].ids[0] == point.ids[0]
                && prepared[index + 1].ids[1] == point.ids[1]
                && prepared[index + 1].ids[2] == point.ids[2]
                && prepared[index + 1].ids[3] == 1;
            let (point_count, matrix, second_impulse) = if has_second {
                (
                    2,
                    [
                        k11,
                        f32::from_bits(raw_contributions[first_contribution + 1].padding),
                        f32::from_bits(raw_contributions[first_contribution + 1].padding),
                        f32::from_bits(raw_contributions[first_contribution + 2].padding),
                    ],
                    prepared[index + 1].data1[2],
                )
            } else {
                (1, [k11, 0.0, 0.0, 0.0], 0.0)
            };
            Some(GpuManifoldBlockEvidenceV2 {
                manifold_index: point.ids[2],
                body_a: point.ids[0],
                body_b: point.ids[1],
                point_count,
                effective_mass_matrix: matrix,
                active_set: GpuBlockActiveSetV2::from_shader_code(point.data2[3] as u32),
                body_a_normal_mass_incidence: point.data2[1] as u32,
                body_b_normal_mass_incidence: point.data2[2] as u32,
                normal_impulses: [point.data1[2], second_impulse],
            })
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
    let coloring = if layout.coloring_bytes == 0 {
        None
    } else {
        let raw = bytemuck::try_cast_slice::<u8, u32>(&bytes[contribution_end..])
            .map_err(|error| format!("decode fixed-floor coloring: {error}"))?;
        let topology_words = layout.color_topology_bytes as usize / size_of::<u32>();
        let indirect_words = layout.color_indirect_bytes as usize / size_of::<u32>();
        let expected_indirect_words = COLOR_INDIRECT_WORDS
            + u32::from(layout.topology_layout.has_island_dispatch) * ISLAND_INDIRECT_WORDS;
        if raw.len() != topology_words + indirect_words
            || topology_words < layout.point_count as usize + layout.body_count as usize + 2
            || indirect_words != expected_indirect_words as usize
        {
            return Err("fixed-floor coloring evidence had the wrong word count".to_owned());
        }
        let status_offset = layout.point_count as usize + layout.body_count as usize;
        let color_count = raw[status_offset + 1];
        let overflowed = raw[status_offset] != 0;
        let point_colors = raw[..layout.point_count as usize]
            .iter()
            .zip(&contacts)
            .filter_map(|(&color, contact)| contact.valid.then_some(color))
            .collect::<Vec<_>>();
        if color_count > MAX_COLORS {
            return Err("fixed-floor coloring reported too many colors".to_owned());
        }
        let mut full_occupancy = vec![0_u32; MAX_COLORS as usize];
        for (&color, contact) in raw[..layout.point_count as usize].iter().zip(&contacts) {
            if !contact.valid
                || (layout.color_unit == GpuColorUnitV4::Manifold && contact.point_in_manifold != 0)
            {
                continue;
            }
            if color < MAX_COLORS {
                full_occupancy[color as usize] += 1;
            } else if !overflowed {
                return Err("fixed-floor coloring reported an invalid assignment".to_owned());
            }
        }
        let color_occupancy = full_occupancy[..color_count as usize].to_vec();
        let active_color_passes = color_occupancy
            .iter()
            .filter(|&&occupancy| occupancy != 0)
            .count() as u32;
        let (
            compacted_point_indices,
            mut indirect_workgroups,
            mut empty_indirect_launches,
            mut scheduled_point_invocations,
        ) = if layout.color_schedule == GpuColorScheduleV4::CompactIndirect {
            let counts_start = status_offset + 2;
            let offsets_start = counts_start + MAX_COLORS as usize;
            let cursors_start = offsets_start + COLOR_OFFSET_COUNT as usize;
            let indices_start = cursors_start + MAX_COLORS as usize;
            let expected_topology_words =
                (layout.topology_layout.element_count - layout.topology_layout.color_base) as usize;
            if topology_words != expected_topology_words {
                return Err("compact color topology had the wrong word count".to_owned());
            }
            let counts = &raw[counts_start..offsets_start];
            let offsets = &raw[offsets_start..cursors_start];
            let cursors = &raw[cursors_start..indices_start];
            if counts != full_occupancy || cursors != counts {
                return Err(format!(
                    "compact color counts disagree with assignments: occupancy={full_occupancy:?} counts={counts:?} cursors={cursors:?}"
                ));
            }
            let mut running = 0_u32;
            for (color, &count) in counts.iter().enumerate() {
                if offsets[color] != running {
                    return Err("compact color offsets are not an exact prefix".to_owned());
                }
                running = running
                    .checked_add(count)
                    .ok_or_else(|| "compact color count overflowed".to_owned())?;
            }
            if offsets[MAX_COLORS as usize] != running || running > layout.point_count {
                return Err("compact color terminal offset is invalid".to_owned());
            }
            let compacted = raw[indices_start..indices_start + running as usize].to_vec();
            let expected_compacted = (0..MAX_COLORS)
                .flat_map(|color| {
                    raw[..layout.point_count as usize]
                        .iter()
                        .zip(&contacts)
                        .enumerate()
                        .filter(move |&(_, (&assigned, contact))| {
                            assigned == color
                                && contact.valid
                                && (layout.color_unit == GpuColorUnitV4::Point
                                    || contact.point_in_manifold == 0)
                        })
                        .map(|(point, _)| point as u32)
                })
                .collect::<Vec<_>>();
            if compacted != expected_compacted {
                return Err("compact color point order is not stable".to_owned());
            }
            let indirect = raw[topology_words..topology_words + COLOR_INDIRECT_WORDS as usize]
                .chunks_exact(3)
                .map(|words| [words[0], words[1], words[2]])
                .collect::<Vec<_>>();
            if indirect.len() != MAX_COLORS as usize
                || indirect.iter().enumerate().any(|(color, arguments)| {
                    let expected_x = if overflowed {
                        0
                    } else {
                        counts[color].div_ceil(WORKGROUP_SIZE)
                    };
                    *arguments != [expected_x, 1, 1]
                })
            {
                return Err("compact color indirect arguments are invalid".to_owned());
            }
            let executed_indirect = if layout.color_order == GpuColorOrderV4::AuthorityOverflowFirst
            {
                &indirect[..crate::AUTHORITY_GPU_COLOR_COUNT as usize - 1]
            } else {
                &indirect[..]
            };
            let empty = executed_indirect
                .iter()
                .filter(|arguments| arguments[0] == 0)
                .count() as u32;
            let invocations = executed_indirect
                .iter()
                .map(|arguments| u64::from(arguments[0]) * u64::from(WORKGROUP_SIZE))
                .sum::<u64>()
                + u64::from(layout.color_order == GpuColorOrderV4::AuthorityOverflowFirst);
            (compacted, indirect, empty, invocations)
        } else {
            let expected_topology_words = status_offset + 2;
            if topology_words != expected_topology_words {
                return Err("fixed-grid color topology had the wrong word count".to_owned());
            }
            (
                Vec::new(),
                Vec::new(),
                0,
                u64::from(workgroups(layout.point_count))
                    * u64::from(WORKGROUP_SIZE)
                    * u64::from(MAX_COLORS),
            )
        };
        let relative = |absolute: u32| (absolute - layout.topology_layout.color_base) as usize;
        let active_body_slots = if layout.topology_layout.work_state_base != 0 {
            let work_start = relative(layout.topology_layout.work_state_base);
            let count = raw[work_start + 1];
            if count > layout.body_count {
                return Err("solver active body count exceeds capacity".to_owned());
            }
            let slots = raw[work_start + SOLVER_WORK_STATE_WORDS as usize
                ..work_start + SOLVER_WORK_STATE_WORDS as usize + count as usize]
                .to_vec();
            let mut seen = vec![false; layout.body_count as usize];
            for &slot in &slots {
                let Some(seen_slot) = seen.get_mut(slot as usize) else {
                    return Err("solver active body slot exceeds capacity".to_owned());
                };
                if *seen_slot {
                    return Err("solver active body slots contain a duplicate".to_owned());
                }
                *seen_slot = true;
            }
            slots
        } else {
            (0..layout.body_count).collect()
        };
        let (island_count, island_body_occupancy, island_manifold_occupancy) = if layout
            .topology_layout
            .has_island_dispatch
        {
            let island_count = raw[relative(layout.topology_layout.island_count_index)];
            if island_count > active_body_slots.len() as u32 || (overflowed && island_count != 0) {
                return Err("solver island count is invalid".to_owned());
            }
            let island_count_usize = island_count as usize;
            let body_counts_start = relative(layout.topology_layout.island_body_counts_base);
            let manifold_counts_start =
                relative(layout.topology_layout.island_manifold_counts_base);
            let body_occupancy =
                raw[body_counts_start..body_counts_start + island_count_usize].to_vec();
            let manifold_occupancy =
                raw[manifold_counts_start..manifold_counts_start + island_count_usize].to_vec();
            if body_occupancy.contains(&0) || manifold_occupancy.contains(&0) {
                return Err("solver island evidence contains an empty island".to_owned());
            }

            let body_islands_start = relative(layout.topology_layout.body_island_base);
            let body_islands =
                &raw[body_islands_start..body_islands_start + layout.body_count as usize];
            let mut expected_body_occupancy = vec![0_u32; island_count_usize];
            for body in active_body_slots.iter().copied() {
                let island = body_islands[body as usize];
                if island < island_count {
                    expected_body_occupancy[island as usize] += 1;
                } else if island != u32::MAX {
                    return Err("solver body references an invalid island".to_owned());
                }
            }
            if body_occupancy != expected_body_occupancy {
                return Err("solver island body occupancy disagrees with assignments".to_owned());
            }

            let bin_count = island_count_usize * MAX_COLORS as usize;
            let bin_counts_start = relative(layout.topology_layout.island_bin_counts_base);
            let bin_offsets_start = relative(layout.topology_layout.island_bin_offsets_base);
            let bin_cursors_start = relative(layout.topology_layout.island_bin_cursors_base);
            let point_indices_start = relative(layout.topology_layout.island_point_indices_base);
            let bin_counts = &raw[bin_counts_start..bin_counts_start + bin_count];
            let bin_offsets = &raw[bin_offsets_start..bin_offsets_start + bin_count + 1];
            let bin_cursors = &raw[bin_cursors_start..bin_cursors_start + bin_count];
            if bin_counts != bin_cursors {
                return Err("solver island color cursors disagree with counts".to_owned());
            }
            let mut running = 0_u32;
            for (bin, &count) in bin_counts.iter().enumerate() {
                if bin_offsets[bin] != running {
                    return Err("solver island color offsets are not an exact prefix".to_owned());
                }
                running = running
                    .checked_add(count)
                    .ok_or_else(|| "solver island color count overflowed".to_owned())?;
            }
            if bin_offsets[bin_count] != running || running > layout.point_count {
                return Err("solver island color terminal offset is invalid".to_owned());
            }
            let island_points = &raw[point_indices_start..point_indices_start + running as usize];
            let mut expected_points = Vec::with_capacity(running as usize);
            let mut expected_manifold_occupancy = vec![0_u32; island_count_usize];
            for island in 0..island_count {
                for color in 0..MAX_COLORS {
                    for (point_index, (&assigned, contact)) in raw[..layout.point_count as usize]
                        .iter()
                        .zip(&contacts)
                        .enumerate()
                    {
                        if !contact.valid || contact.point_in_manifold != 0 || assigned != color {
                            continue;
                        }
                        let body_a_island = body_islands[contact.body_a as usize];
                        let body_b_island = body_islands[contact.body_b as usize];
                        if body_a_island != u32::MAX
                            && body_b_island != u32::MAX
                            && body_a_island != body_b_island
                        {
                            return Err("dynamic contact spans solver islands".to_owned());
                        }
                        let contact_island = if body_a_island != u32::MAX {
                            body_a_island
                        } else {
                            body_b_island
                        };
                        if contact_island == island {
                            expected_points.push(point_index as u32);
                            expected_manifold_occupancy[island as usize] += 1;
                        }
                    }
                }
            }
            if island_points != expected_points || manifold_occupancy != expected_manifold_occupancy
            {
                return Err("solver island compacted manifolds are inconsistent".to_owned());
            }

            let island_indirect = raw
                [topology_words + COLOR_INDIRECT_WORDS as usize..topology_words + indirect_words]
                .chunks_exact(3)
                .map(|words| [words[0], words[1], words[2]])
                .collect::<Vec<_>>();
            let expected_x = if overflowed { 0 } else { island_count };
            if island_indirect != vec![[expected_x, 1, 1]] {
                return Err("solver island indirect arguments are invalid".to_owned());
            }
            indirect_workgroups = island_indirect;
            empty_indirect_launches = u32::from(expected_x == 0);
            scheduled_point_invocations = u64::from(expected_x) * u64::from(WORKGROUP_SIZE);
            (island_count, body_occupancy, manifold_occupancy)
        } else {
            (0, Vec::new(), Vec::new())
        };
        let color_units = color_occupancy.iter().sum();
        let (retained_color_units, new_color_units, recolored_color_units, removed_color_units) =
            if layout.scheduling_policy.color_maintenance
                == GpuColorMaintenanceV1::StablePairIncremental
            {
                let maintenance_start =
                    relative(layout.topology_layout.color_maintenance_counts_base);
                (
                    raw[maintenance_start],
                    raw[maintenance_start + 1],
                    raw[maintenance_start + 2],
                    raw[maintenance_start + 3],
                )
            } else {
                (0, color_units, 0, 0)
            };
        let (
            parallel_round_limit,
            parallel_rounds_used,
            parallel_conflict_retries,
            parallel_uncolored_units,
        ) = if layout.topology_layout.has_parallel_coloring {
            let stats_start = relative(layout.topology_layout.parallel_color_stats_base);
            let conflict_retries = raw[stats_start];
            let uncolored_units = raw[stats_start + 1];
            let rounds_used = raw[stats_start + 2];
            if rounds_used > PARALLEL_COLOR_ROUNDS || (uncolored_units != 0 && !overflowed) {
                return Err("parallel coloring reported incoherent convergence state".to_owned());
            }
            (
                PARALLEL_COLOR_ROUNDS,
                rounds_used,
                conflict_retries,
                uncolored_units,
            )
        } else {
            (0, 0, 0, 0)
        };
        Some(GpuColoringEvidenceV4 {
            schedule: layout.color_schedule,
            unit: layout.color_unit,
            order: layout.color_order,
            max_colors: MAX_COLORS,
            color_count,
            point_colors,
            color_occupancy,
            compacted_point_indices,
            indirect_workgroups,
            encoded_color_passes: if layout.topology_layout.has_island_dispatch {
                1
            } else if layout.color_order == GpuColorOrderV4::AuthorityOverflowFirst {
                crate::AUTHORITY_GPU_COLOR_COUNT
            } else {
                MAX_COLORS
            },
            active_color_passes,
            empty_indirect_launches,
            scheduled_point_invocations,
            scheduling_policy: layout.scheduling_policy,
            retained_color_units,
            new_color_units,
            recolored_color_units,
            removed_color_units,
            parallel_round_limit,
            parallel_rounds_used,
            parallel_conflict_retries,
            parallel_uncolored_units,
            island_count,
            island_body_occupancy,
            island_manifold_occupancy,
            overflowed,
        })
    };
    Ok(GpuFixedFloorPassEvidence {
        contacts,
        contributions,
        manifold_blocks,
        coloring,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_prepared_contact_bytes(
    bytes: &[u8],
    point_count: u32,
) -> Result<Vec<GpuFixedFloorContactEvidence>, String> {
    let expected = point_count as usize * size_of::<PreparedPointRaw>();
    if bytes.len() != expected {
        return Err(format!(
            "prepared contact trace requires {expected} bytes, received {}",
            bytes.len()
        ));
    }
    let prepared = bytemuck::try_cast_slice::<u8, PreparedPointRaw>(bytes)
        .map_err(|error| format!("decode prepared contact trace: {error}"))?;
    Ok(decode_prepared_contacts(prepared))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_prepared_contacts(prepared: &[PreparedPointRaw]) -> Vec<GpuFixedFloorContactEvidence> {
    prepared
        .iter()
        .enumerate()
        .map(|(index, point)| GpuFixedFloorContactEvidence {
            point_index: index as u32,
            valid: point.ids[0] != u32::MAX && point.ids[1] != u32::MAX,
            body_a: point.ids[0],
            body_b: point.ids[1],
            manifold_index: point.ids[2],
            point_in_manifold: point.ids[3],
            normal: physbox_contracts::Vec2::new(point.normal[0], point.normal[1]),
            tangent: physbox_contracts::Vec2::new(point.tangent[0], point.tangent[1]),
            anchor_a: physbox_contracts::Vec2::new(point.anchor_a[0], point.anchor_a[1]),
            anchor_b: physbox_contracts::Vec2::new(point.anchor_b[0], point.anchor_b[1]),
            initial_normal_velocity: point.data0[0],
            normal_effective_mass: point.data0[1],
            tangent_effective_mass: point.data0[2],
            separation: point.data0[3],
            friction: point.data1[0],
            restitution: point.data1[1],
            normal_impulse: point.data1[2],
            tangent_impulse: point.data1[3],
            total_normal_impulse: point.data2[0],
            body_a_normal_mass_incidence: point.data2[1] as u32,
            body_b_normal_mass_incidence: point.data2[2] as u32,
        })
        .collect()
}

pub(crate) fn validate_selected_settings(scenario: &ScenarioV1) -> Result<(), String> {
    let settings = &scenario.settings;
    let selected = settings.fixed_dt.get().to_bits() == (1.0_f32 / 60.0).to_bits()
        && settings.substeps == 4
        && settings.iterations == 2
        && settings.bias_rate.get().to_bits() == 0.2_f32.to_bits()
        && settings.mass_coefficient.get().to_bits() == 1.0_f32.to_bits()
        && settings.impulse_coefficient.get().to_bits() == 0.0_f32.to_bits()
        && settings.contact_softness.is_none()
        && settings.restitution_threshold.get().to_bits() == 1.0_f32.to_bits();
    if !selected {
        return Err("solver settings differ from the accepted S5.4 baseline".to_owned());
    }
    Ok(())
}

fn validate_authored_settings(scenario: &ScenarioV1) -> Result<(), String> {
    let settings = &scenario.settings;
    let accepted = settings.fixed_dt.get().to_bits() == (1.0_f32 / 60.0).to_bits()
        && settings.substeps == 4
        && settings.iterations == 1
        && settings.bias_rate.get().to_bits() == 0.2_f32.to_bits()
        && settings.mass_coefficient.get().to_bits() == 1.0_f32.to_bits()
        && settings.impulse_coefficient.get().to_bits() == 0.0_f32.to_bits()
        && settings.contact_softness.is_none()
        && settings.restitution_threshold.get().to_bits() == 1.0_f32.to_bits();
    if !accepted {
        return Err("authored fixed-floor settings differ from the S1.6 control".to_owned());
    }
    Ok(())
}

fn validate_experiment_settings(
    settings: &SolverSettingsV1,
    execution: GpuSolverExecutionV4,
) -> Result<(), String> {
    execution.validate()?;
    if !(1..=16).contains(&settings.substeps) || !(1..=8).contains(&settings.iterations) {
        return Err(
            "GPU solver experiments require 1..=16 substeps and 1..=8 iterations".to_owned(),
        );
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
    permute_edge_build: bool,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    operations_with_contact_feedback(
        scenario,
        body_count,
        point_count,
        topology,
        permute_edge_build,
        execution,
        scheduling_policy,
        GpuSoftFeedbackPolicyV1::SELECTED,
        GpuNormalRelaxationPolicyV1::SELECTED,
    )
}

#[allow(clippy::too_many_arguments)]
fn operations_with_contact_feedback(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    soft_feedback_policy: GpuSoftFeedbackPolicyV1,
    normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    operations_with_solver_policies(
        scenario,
        body_count,
        point_count,
        topology,
        permute_edge_build,
        execution,
        scheduling_policy,
        soft_feedback_policy,
        normal_relaxation_policy,
        GpuContactCoefficientPolicyV1::SELECTED,
    )
}

#[allow(clippy::too_many_arguments)]
fn operations_with_solver_policies(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    soft_feedback_policy: GpuSoftFeedbackPolicyV1,
    normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    operations_with_extended_solver_policies(
        scenario,
        body_count,
        point_count,
        topology,
        permute_edge_build,
        execution,
        scheduling_policy,
        soft_feedback_policy,
        normal_relaxation_policy,
        contact_coefficient_policy,
        GpuSubstepContactPolicyV1::SELECTED,
        GpuRestitutionTimingPolicyV1::SELECTED,
    )
}

#[allow(clippy::too_many_arguments)]
fn operations_with_extended_solver_policies(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
    execution: GpuSolverExecutionV4,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
    soft_feedback_policy: GpuSoftFeedbackPolicyV1,
    normal_relaxation_policy: GpuNormalRelaxationPolicyV1,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
    substep_contact_policy: GpuSubstepContactPolicyV1,
    restitution_timing_policy: GpuRestitutionTimingPolicyV1,
) -> (Vec<PassParamsRaw>, Vec<Operation>) {
    let mut parameters = Vec::new();
    let mut operations = Vec::new();
    let authority_probe = execution == GpuSolverExecutionV4::AUTHORITY_GPU_PROBE;
    let iterations_per_substep = if authority_probe
        && contact_coefficient_policy != GpuContactCoefficientPolicyV1::AuthorityTunableSoft
    {
        1
    } else {
        scenario.settings.iterations
    };
    let body_workgroups = workgroups(body_count);
    let point_workgroups = workgroups(point_count);
    let normal_pipeline = if execution.normal_solver.uses_block() {
        PipelineKind::NormalBlock
    } else {
        PipelineKind::Normal
    };
    let fused_normal_tangent = scheduling_policy.colored_dispatch.fuses_normal_tangent();
    topology_operations(
        &mut parameters,
        &mut operations,
        body_count,
        point_count,
        topology,
        permute_edge_build,
        scheduling_policy,
    );
    for substep in 0..scenario.settings.substeps {
        let mut integrate_velocity = pass_params(
            scenario,
            body_count,
            point_count,
            0,
            0,
            0,
            contact_coefficient_policy,
        );
        integrate_velocity.uints1[2] = u32::from(authority_probe);
        push_operation(
            &mut parameters,
            &mut operations,
            integrate_velocity,
            PipelineKind::IntegrateVelocity,
            MotionDirection::AtoB,
            body_workgroups,
        );
        if substep == 0 || substep_contact_policy.refreshes_anchors_and_mass() {
            let mut prepare = preparation_params(
                scenario,
                body_count,
                point_count,
                topology,
                execution.normal_solver,
                contact_coefficient_policy,
            );
            let prepare_kind = if substep == 0 {
                PipelineKind::Prepare
            } else {
                prepare.uints0[3] =
                    u32::from(substep_contact_policy == GpuSubstepContactPolicyV1::SELECTED);
                PipelineKind::Refresh
            };
            prepare.uints1[0] = u32::from(execution.warm_start);
            push_operation(
                &mut parameters,
                &mut operations,
                prepare,
                prepare_kind,
                MotionDirection::AtoB,
                point_workgroups,
            );
        }
        solver_stage(
            &mut parameters,
            &mut operations,
            execution,
            pass_params(
                scenario,
                body_count,
                point_count,
                1,
                0,
                0,
                contact_coefficient_policy,
            ),
            PipelineKind::Warm,
            MotionDirection::AtoB,
            point_workgroups,
            body_workgroups,
            color_sweep_reversed(execution, substep, 0, iterations_per_substep),
            topology,
        );
        for iteration in 0..iterations_per_substep {
            solver_stage(
                &mut parameters,
                &mut operations,
                execution,
                normal_params(
                    scenario,
                    body_count,
                    point_count,
                    if fused_normal_tangent { 7 } else { 2 },
                    iteration,
                    soft_feedback_policy.use_bias(iteration),
                    topology,
                    execution.normal_solver,
                    contact_coefficient_policy,
                ),
                normal_pipeline,
                MotionDirection::BtoA,
                point_workgroups,
                body_workgroups,
                color_sweep_reversed(execution, substep, iteration, iterations_per_substep),
                topology,
            );
            if !fused_normal_tangent {
                solver_stage(
                    &mut parameters,
                    &mut operations,
                    execution,
                    pass_params(
                        scenario,
                        body_count,
                        point_count,
                        3,
                        iteration,
                        0,
                        contact_coefficient_policy,
                    ),
                    PipelineKind::Tangent,
                    MotionDirection::AtoB,
                    point_workgroups,
                    body_workgroups,
                    color_sweep_reversed(execution, substep, iteration, iterations_per_substep),
                    topology,
                );
            }
        }
        let mut integrate_position = pass_params(
            scenario,
            body_count,
            point_count,
            0,
            0,
            0,
            contact_coefficient_policy,
        );
        integrate_position.uints1[2] = u32::from(authority_probe);
        integrate_position.uints1[3] = u32::from(authority_probe && substep == 0);
        integrate_position.uints0[2] = topology.authority_initial_transforms_base;
        push_operation(
            &mut parameters,
            &mut operations,
            integrate_position,
            PipelineKind::IntegratePosition,
            if execution.uses_colored_in_place() {
                MotionDirection::AtoB
            } else {
                MotionDirection::BtoA
            },
            body_workgroups,
        );
        let mut refresh = preparation_params(
            scenario,
            body_count,
            point_count,
            topology,
            execution.normal_solver,
            contact_coefficient_policy,
        );
        if substep_contact_policy == GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation {
            refresh.uints0[3] = 2;
        } else if substep_contact_policy
            == GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation
        {
            refresh.uints0[3] = 3;
            refresh.uints0[2] = topology.authority_initial_transforms_base;
        } else if substep_contact_policy
            == GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
        {
            refresh.uints0[3] = 4;
            refresh.uints0[2] = topology.authority_initial_transforms_base;
        }
        push_operation(
            &mut parameters,
            &mut operations,
            refresh,
            PipelineKind::Refresh,
            if execution.uses_colored_in_place() {
                MotionDirection::AtoB
            } else {
                MotionDirection::BtoA
            },
            point_workgroups,
        );
        if execution.relaxation {
            for iteration in 0..iterations_per_substep {
                if normal_relaxation_policy.emits_normal_pass() {
                    let mut params = normal_params(
                        scenario,
                        body_count,
                        point_count,
                        if fused_normal_tangent { 8 } else { 4 },
                        iteration,
                        0,
                        topology,
                        execution.normal_solver,
                        contact_coefficient_policy,
                    );
                    params.floats2[3] = normal_relaxation_policy.shader_code() as f32;
                    solver_stage(
                        &mut parameters,
                        &mut operations,
                        execution,
                        params,
                        normal_pipeline,
                        MotionDirection::BtoA,
                        point_workgroups,
                        body_workgroups,
                        color_sweep_reversed(execution, substep, iteration, iterations_per_substep),
                        topology,
                    );
                }
                if !fused_normal_tangent {
                    solver_stage(
                        &mut parameters,
                        &mut operations,
                        execution,
                        pass_params(
                            scenario,
                            body_count,
                            point_count,
                            5,
                            iteration,
                            0,
                            contact_coefficient_policy,
                        ),
                        PipelineKind::Tangent,
                        MotionDirection::AtoB,
                        point_workgroups,
                        body_workgroups,
                        color_sweep_reversed(execution, substep, iteration, iterations_per_substep),
                        topology,
                    );
                }
            }
        }
        if restitution_timing_policy == GpuRestitutionTimingPolicyV1::SELECTED {
            solver_stage(
                &mut parameters,
                &mut operations,
                execution,
                pass_params(
                    scenario,
                    body_count,
                    point_count,
                    6,
                    0,
                    0,
                    contact_coefficient_policy,
                ),
                PipelineKind::Restitution,
                MotionDirection::BtoA,
                point_workgroups,
                body_workgroups,
                color_sweep_reversed(
                    execution,
                    substep,
                    iterations_per_substep - 1,
                    iterations_per_substep,
                ),
                topology,
            );
        }
    }
    if restitution_timing_policy == GpuRestitutionTimingPolicyV1::AfterSubsteps {
        solver_stage(
            &mut parameters,
            &mut operations,
            execution,
            pass_params(
                scenario,
                body_count,
                point_count,
                6,
                0,
                0,
                contact_coefficient_policy,
            ),
            PipelineKind::Restitution,
            MotionDirection::BtoA,
            point_workgroups,
            body_workgroups,
            color_sweep_reversed(
                execution,
                scenario.settings.substeps - 1,
                iterations_per_substep - 1,
                iterations_per_substep,
            ),
            topology,
        );
    }
    (parameters, operations)
}

fn preparation_params(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    normal_solver: GpuNormalSolverV4,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
) -> PassParamsRaw {
    let mut params = pass_params(
        scenario,
        body_count,
        point_count,
        0,
        0,
        0,
        contact_coefficient_policy,
    );
    params.uints0[2] = topology.ranges_base;
    params.uints1[1] = normal_solver.shader_code();
    params.uints1[2] = u32::from(topology.kind == GpuGatherTopology::StableBodyEdge);
    params.uints1[3] = topology.manifold_counts_base;
    params
}

#[allow(clippy::too_many_arguments)]
fn normal_params(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    stage: u32,
    iteration: u32,
    use_bias: u32,
    topology: TopologyLayout,
    normal_solver: GpuNormalSolverV4,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
) -> PassParamsRaw {
    let mut params = pass_params(
        scenario,
        body_count,
        point_count,
        stage,
        iteration,
        use_bias,
        contact_coefficient_policy,
    );
    params.uints0[2] = topology.ranges_base;
    params.uints0[3] = topology.manifold_counts_base;
    params.uints1[2] = u32::from(topology.kind == GpuGatherTopology::StableBodyEdge);
    params.floats2[1] = normal_solver.shader_code() as f32;
    params
}

fn topology_operations(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
    permute_edge_build: bool,
    scheduling_policy: GpuSolverSchedulingPolicyV1,
) {
    let clear_coloring = topology.has_coloring
        && scheduling_policy.color_maintenance != GpuColorMaintenanceV1::StablePairIncremental;
    let clear_flags = u32::from(topology.kind == GpuGatherTopology::StableBodyEdge)
        | (u32::from(topology.has_manifold_counts) << 1)
        | (u32::from(clear_coloring) << 2)
        | (u32::from(topology.has_parallel_coloring) << 3)
        | (u32::from(topology.has_compact_coloring) << 4);
    let base = PassParamsRaw {
        floats0: [0.0; 4],
        floats1: [0.0; 4],
        floats2: [0.0; 4],
        floats3: [0.0; 4],
        uints0: [body_count, point_count, 0, 0],
        uints1: [0; 4],
    };
    let mut clear = base;
    clear.floats0[0] = f32::from_bits(topology.parallel_color_stats_base);
    clear.floats0[3] = clear_flags as f32;
    clear.uints0 = [
        topology.ranges_base,
        topology.manifold_counts_base,
        topology.color_mask_base,
        body_count,
    ];
    clear.uints1 = [
        topology.color_status_index,
        topology.color_count_index,
        topology.color_counts_base,
        topology.color_cursors_base,
    ];
    push_operation(
        parameters,
        operations,
        clear,
        PipelineKind::TopologyClear,
        MotionDirection::AtoB,
        workgroups(body_count.max(1)),
    );
    match topology.kind {
        GpuGatherTopology::CanonicalBodyScan => {
            let mut params = base;
            params.uints0[2] = topology.incident_base;
            params.uints0[3] = topology.cursor_base;
            for (kind, workgroups) in [
                (PipelineKind::TopologyCount, workgroups(point_count)),
                (PipelineKind::TopologyPrefix, 1),
                (PipelineKind::TopologyScatter, workgroups(body_count)),
            ] {
                push_operation(
                    parameters,
                    operations,
                    params,
                    kind,
                    MotionDirection::AtoB,
                    workgroups,
                );
            }
        }
        GpuGatherTopology::StableBodyEdge => {
            let mut build = base;
            build.uints0[2] = topology.edge_a_base;
            build.uints1[0] = topology.edge_count;
            build.uints1[1] = u32::from(permute_edge_build);
            push_operation(
                parameters,
                operations,
                build,
                PipelineKind::EdgeBuild,
                MotionDirection::AtoB,
                workgroups(point_count),
            );
            for digit in 0..topology.radix_passes {
                let (source, destination) = if digit % 2 == 0 {
                    (topology.edge_a_base, topology.edge_b_base)
                } else {
                    (topology.edge_b_base, topology.edge_a_base)
                };
                let radix = PassParamsRaw {
                    floats0: [0.0; 4],
                    floats1: [0.0; 4],
                    floats2: [0.0; 4],
                    floats3: [0.0; 4],
                    uints0: [
                        topology.edge_count,
                        topology.block_count,
                        source,
                        destination,
                    ],
                    uints1: [topology.blocks_base, digit * 4, 0, 0],
                };
                for (kind, workgroups) in [
                    (PipelineKind::EdgeRadixHistograms, topology.block_count),
                    (PipelineKind::EdgeRadixPrefix, 1),
                    (PipelineKind::EdgeRadixScatter, topology.block_count),
                ] {
                    push_operation(
                        parameters,
                        operations,
                        radix,
                        kind,
                        MotionDirection::AtoB,
                        workgroups,
                    );
                }
            }
            let ranges = PassParamsRaw {
                floats0: [0.0; 4],
                floats1: [0.0; 4],
                floats2: [0.0; 4],
                floats3: [0.0; 4],
                uints0: [
                    topology.edge_count,
                    body_count,
                    topology.sorted_edge_base,
                    topology.ranges_base,
                ],
                uints1: [0; 4],
            };
            push_operation(
                parameters,
                operations,
                ranges,
                PipelineKind::EdgeRanges,
                MotionDirection::AtoB,
                workgroups(topology.edge_count),
            );
        }
    }
    if topology.has_manifold_counts {
        let mut counts = base;
        counts.uints0[2] = topology.manifold_counts_base;
        push_operation(
            parameters,
            operations,
            counts,
            PipelineKind::ManifoldCount,
            MotionDirection::AtoB,
            workgroups(point_count),
        );
    }
    if topology.has_coloring {
        let colors = PassParamsRaw {
            floats0: [
                f32::from_bits(topology.parallel_color_winners_base),
                f32::from_bits(topology.parallel_color_stats_base),
                0.0,
                0.0,
            ],
            floats1: [0.0; 4],
            floats2: [0.0; 4],
            floats3: [0.0; 4],
            uints0: [
                body_count,
                point_count,
                topology.color_base,
                topology.color_mask_base,
            ],
            uints1: [
                topology.color_status_index,
                topology.color_count_index,
                MAX_COLORS,
                u32::from(topology.has_compact_coloring)
                    | (u32::from(topology.color_unit == GpuColorUnitV4::Manifold) << 1)
                    | (u32::from(topology.has_parallel_compaction) << 2)
                    | (u32::from(scheduling_policy.uses_static_late()) << 3)
                    | (u32::from(scheduling_policy.uses_authority_source_order()) << 4),
            ],
        };
        let mut colors = colors;
        if topology.has_incremental_coloring {
            colors.uints0[0] = topology.color_identity_base;
        }
        if topology.has_parallel_coloring {
            push_operation(
                parameters,
                operations,
                colors,
                PipelineKind::Color,
                MotionDirection::AtoB,
                workgroups(point_count),
            );
            for round in 0..PARALLEL_COLOR_ROUNDS {
                let mut round_params = colors;
                round_params.floats0[2] = f32::from_bits(round);
                for (pipeline, workgroups) in [
                    (
                        PipelineKind::ColorConflictClear,
                        workgroups(topology.parallel_color_winner_count),
                    ),
                    (PipelineKind::ColorPropose, workgroups(point_count)),
                    (PipelineKind::ColorResolve, workgroups(point_count)),
                ] {
                    push_operation(
                        parameters,
                        operations,
                        round_params,
                        pipeline,
                        MotionDirection::AtoB,
                        workgroups,
                    );
                }
            }
            push_operation(
                parameters,
                operations,
                colors,
                PipelineKind::ColorFinalize,
                MotionDirection::AtoB,
                workgroups(point_count),
            );
        } else {
            push_operation(
                parameters,
                operations,
                colors,
                PipelineKind::Color,
                MotionDirection::AtoB,
                1,
            );
        }
        if topology.has_compact_coloring {
            if topology.has_parallel_compaction {
                let parallel = PassParamsRaw {
                    floats0: [
                        f32::from_bits(topology.color_indices_base),
                        f32::from_bits(topology.color_status_index),
                        f32::from_bits(topology.color_block_count),
                        f32::from_bits(u32::from(topology.color_unit == GpuColorUnitV4::Manifold)),
                    ],
                    floats1: [0.0; 4],
                    floats2: [0.0; 4],
                    floats3: [0.0; 4],
                    uints0: [
                        point_count,
                        topology.color_base,
                        topology.color_block_counts_base,
                        topology.color_block_offsets_base,
                    ],
                    uints1: [
                        topology.color_local_ranks_base,
                        topology.color_counts_base,
                        topology.color_offsets_base,
                        topology.color_cursors_base,
                    ],
                };
                for (pipeline, workgroups) in [
                    (PipelineKind::ColorBlockCount, topology.color_block_count),
                    (PipelineKind::ColorBlockPrefix, 1),
                    (PipelineKind::ColorRangePrefix, 1),
                    (PipelineKind::ColorBlockScatter, topology.color_block_count),
                ] {
                    push_operation(
                        parameters,
                        operations,
                        parallel,
                        pipeline,
                        MotionDirection::AtoB,
                        workgroups,
                    );
                }
            } else {
                push_operation(
                    parameters,
                    operations,
                    colors,
                    PipelineKind::ColorCompact,
                    MotionDirection::AtoB,
                    1,
                );
            }
        }
        if topology.has_island_dispatch {
            let islands = PassParamsRaw {
                floats0: [
                    f32::from_bits(topology.island_roots_base),
                    f32::from_bits(topology.island_body_counts_base),
                    f32::from_bits(topology.island_manifold_counts_base),
                    f32::from_bits(topology.island_bin_cursors_base),
                ],
                floats1: [
                    f32::from_bits(topology.island_point_indices_base),
                    f32::from_bits(topology.island_count_index),
                    0.0,
                    0.0,
                ],
                floats2: [0.0; 4],
                floats3: [0.0; 4],
                uints0: [
                    body_count,
                    point_count,
                    topology.island_parent_base,
                    topology.body_island_base,
                ],
                uints1: [
                    topology.color_base,
                    topology.color_status_index,
                    topology.island_bin_counts_base,
                    topology.island_bin_offsets_base,
                ],
            };
            push_operation(
                parameters,
                operations,
                islands,
                PipelineKind::IslandBuild,
                MotionDirection::AtoB,
                1,
            );
        }
    }
}

fn pass_params(
    scenario: &ScenarioV1,
    body_count: u32,
    point_count: u32,
    stage: u32,
    iteration: u32,
    use_bias: u32,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
) -> PassParamsRaw {
    let substep_dt = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
    let softness = scenario.settings.contact_softness.map(|mut policy| {
        if contact_coefficient_policy == GpuContactCoefficientPolicyV1::AuthorityCappedSoft {
            policy.static_contact_hertz_multiplier = physbox_contracts::FiniteF32::new(2.0)
                .expect("the frozen authority static multiplier is finite");
        }
        (
            gpu_contact_softness_coefficients_validated(policy, substep_dt, false),
            gpu_contact_softness_coefficients_validated(policy, substep_dt, true),
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
            0.0,
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
        floats3: softness.map_or_else(
            || match contact_coefficient_policy {
                GpuContactCoefficientPolicyV1::SettingsDerived => [0.0; 4],
                GpuContactCoefficientPolicyV1::StaticStiffHard => [
                    GpuContactCoefficientPolicyV1::STATIC_STIFF_BIAS_RATE,
                    scenario.settings.mass_coefficient.get(),
                    scenario.settings.impulse_coefficient.get(),
                    1.0,
                ],
                GpuContactCoefficientPolicyV1::AuthorityCappedSoft
                | GpuContactCoefficientPolicyV1::AuthorityTunableSoft => {
                    unreachable!("authority soft coefficients require soft contact")
                }
            },
            |(_, static_contact)| {
                [
                    static_contact.mass_scale * static_contact.bias_rate * substep_dt,
                    static_contact.mass_scale,
                    static_contact.impulse_scale,
                    1.0,
                ]
            },
        ),
        uints0: [body_count, point_count, 0, 0],
        uints1: [iteration, stage, 0, use_bias],
    }
}

#[allow(clippy::too_many_arguments)]
fn solver_stage(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    execution: GpuSolverExecutionV4,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: MotionDirection,
    point_workgroups: u32,
    body_workgroups: u32,
    reverse_colors: bool,
    topology: TopologyLayout,
) {
    if execution.uses_colored_in_place() {
        if topology.has_island_dispatch {
            let mut island = parameter;
            island.uints0[0] = topology.island_count_index;
            island.uints0[2] = topology.island_bin_offsets_base;
            island.uints0[3] = topology.island_point_indices_base;
            island.uints1[2] = if execution.color_order == GpuColorOrderV4::AuthorityOverflowFirst {
                2
            } else {
                u32::from(reverse_colors)
            };
            island.floats2[3] = f32::from_bits(topology.color_count_index);
            push_indirect_operation(
                parameters,
                operations,
                island,
                PipelineKind::IslandColored,
                MotionDirection::AtoB,
                ISLAND_INDIRECT_OFFSET,
            );
            return;
        }
        let authority_overflow_first =
            execution.color_order == GpuColorOrderV4::AuthorityOverflowFirst;
        let scheduled_color_count = if authority_overflow_first {
            crate::AUTHORITY_GPU_DYNAMIC_COLOR_COUNT + crate::AUTHORITY_GPU_STATIC_COLOR_COUNT
        } else {
            MAX_COLORS
        };
        if authority_overflow_first {
            let mut overflow = parameter;
            overflow.uints0[0] = topology.color_counts_base;
            overflow.uints0[2] = topology.color_indices_base;
            overflow.uints0[3] = crate::AUTHORITY_GPU_COLOR_COUNT - 1;
            overflow.uints1[2] = topology.color_offsets_base;
            overflow.floats2[2] = 1.0;
            push_operation(
                parameters,
                operations,
                overflow,
                PipelineKind::AuthorityOverflow,
                MotionDirection::AtoB,
                1,
            );
        }
        for scheduled_color in 0..scheduled_color_count {
            let color = if reverse_colors {
                scheduled_color_count - 1 - scheduled_color
            } else {
                scheduled_color
            };
            let mut colored = parameter;
            colored.uints0[0] = if topology.has_compact_coloring {
                topology.color_counts_base
            } else {
                0
            };
            colored.uints0[2] = if topology.has_compact_coloring {
                topology.color_indices_base
            } else {
                topology.color_base
            };
            colored.uints0[3] = color;
            colored.uints1[2] = topology.color_offsets_base;
            colored.floats1[3] = u32::from(topology.static_late) as f32;
            colored.floats2[2] = match execution.color_unit {
                GpuColorUnitV4::Point => 0.0,
                GpuColorUnitV4::Manifold => 1.0,
            };
            if execution.color_schedule == GpuColorScheduleV4::CompactIndirect {
                push_indirect_operation(
                    parameters,
                    operations,
                    colored,
                    PipelineKind::Colored,
                    MotionDirection::AtoB,
                    u64::from(color * 3) * size_of::<u32>() as u64,
                );
            } else {
                push_operation(
                    parameters,
                    operations,
                    colored,
                    PipelineKind::Colored,
                    MotionDirection::AtoB,
                    point_workgroups,
                );
            }
        }
    } else {
        jacobi_stage(
            parameters,
            operations,
            parameter,
            pipeline,
            direction,
            point_workgroups,
            body_workgroups,
            topology,
        );
    }
}

fn color_sweep_reversed(
    execution: GpuSolverExecutionV4,
    substep: u32,
    iteration: u32,
    iterations_per_substep: u32,
) -> bool {
    execution.color_order == GpuColorOrderV4::Alternating
        && (substep * iterations_per_substep + iteration) % 2 == 1
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
    topology: TopologyLayout,
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
    gather.uints0[2] = u32::from(topology.kind == GpuGatherTopology::StableBodyEdge);
    gather.uints0[3] = topology.ranges_base;
    gather.uints1[2] = topology.sorted_edge_base;
    gather.uints1[3] = topology.incident_base;
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
    let color = matches!(
        pipeline,
        PipelineKind::Colored | PipelineKind::AuthorityOverflow
    )
    .then_some(parameter.uints0[3]);
    parameters.push(parameter);
    operations.push(Operation {
        pipeline,
        direction,
        workgroups,
        indirect_offset: None,
        parameter_index,
        color,
    });
}

fn push_indirect_operation(
    parameters: &mut Vec<PassParamsRaw>,
    operations: &mut Vec<Operation>,
    parameter: PassParamsRaw,
    pipeline: PipelineKind,
    direction: MotionDirection,
    indirect_offset: u64,
) {
    let parameter_index = parameters.len() as u32;
    let color = matches!(pipeline, PipelineKind::Colored).then_some(parameter.uints0[3]);
    parameters.push(parameter);
    operations.push(Operation {
        pipeline,
        direction,
        workgroups: 0,
        indirect_offset: Some(indirect_offset),
        parameter_index,
        color,
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn pass_trace_descriptors(
    parameters: &[PassParamsRaw],
    operations: &[Operation],
) -> Vec<GpuSolverPassDescriptorV1> {
    let mut substep = None::<u32>;
    let raw = operations
        .iter()
        .enumerate()
        .filter_map(|(operation_index, operation)| {
            if matches!(operation.pipeline, PipelineKind::IntegrateVelocity) {
                substep = Some(substep.map_or(0, |value| value + 1));
            }
            let substep = substep?;
            let parameter = parameters[operation.parameter_index as usize];
            let stage = parameter.uints1[1];
            let kind = match operation.pipeline {
                PipelineKind::IntegrateVelocity => GpuSolverPassKindV1::IntegrateVelocity,
                PipelineKind::Prepare => GpuSolverPassKindV1::PrepareContacts,
                PipelineKind::Refresh => GpuSolverPassKindV1::RefreshContacts,
                PipelineKind::Warm => GpuSolverPassKindV1::WarmStart,
                PipelineKind::Normal | PipelineKind::NormalBlock => match stage {
                    2 => GpuSolverPassKindV1::NormalBiased,
                    4 => GpuSolverPassKindV1::NormalRelaxation,
                    _ => return None,
                },
                PipelineKind::Tangent => match stage {
                    3 => GpuSolverPassKindV1::TangentBiased,
                    5 => GpuSolverPassKindV1::TangentRelaxation,
                    _ => return None,
                },
                PipelineKind::Restitution => GpuSolverPassKindV1::Restitution,
                PipelineKind::IntegratePosition => GpuSolverPassKindV1::IntegratePosition,
                PipelineKind::Gather => GpuSolverPassKindV1::Gather,
                PipelineKind::Colored
                | PipelineKind::AuthorityOverflow
                | PipelineKind::IslandColored => match stage {
                    1 => GpuSolverPassKindV1::WarmStart,
                    2 => GpuSolverPassKindV1::NormalBiased,
                    3 => GpuSolverPassKindV1::TangentBiased,
                    4 => GpuSolverPassKindV1::NormalRelaxation,
                    5 => GpuSolverPassKindV1::TangentRelaxation,
                    6 => GpuSolverPassKindV1::Restitution,
                    7 => GpuSolverPassKindV1::NormalBiased,
                    8 => GpuSolverPassKindV1::NormalRelaxation,
                    _ => return None,
                },
                _ => return None,
            };
            Some(GpuSolverPassDescriptorV1 {
                version: GPU_SOLVER_PASS_TRACE_VERSION,
                first_operation_index: operation_index as u32,
                operation_index: operation_index as u32,
                substep,
                kind,
                iteration: parameter.uints1[0],
                first_color: operation.color,
                last_color: operation.color,
                color_passes: u32::from(operation.color.is_some()),
                parameters: GpuSolverPassParametersV1 {
                    floats0: parameter.floats0,
                    floats1: parameter.floats1,
                    floats2: parameter.floats2,
                    floats3: parameter.floats3,
                    uints0: parameter.uints0,
                    uints1: parameter.uints1,
                },
            })
        })
        .collect::<Vec<_>>();
    let mut grouped = Vec::<GpuSolverPassDescriptorV1>::new();
    for descriptor in raw {
        if let Some(previous) = grouped.last_mut()
            && previous.operation_index + 1 == descriptor.first_operation_index
            && previous.substep == descriptor.substep
            && previous.kind == descriptor.kind
            && previous.iteration == descriptor.iteration
            && previous.color_passes != 0
            && descriptor.color_passes != 0
        {
            previous.operation_index = descriptor.operation_index;
            previous.last_color = descriptor.last_color;
            previous.color_passes += descriptor.color_passes;
            continue;
        }
        grouped.push(descriptor);
    }
    grouped
}

fn workgroups(elements: u32) -> u32 {
    elements.div_ceil(WORKGROUP_SIZE)
}

fn topology_capacities(
    body_count: u32,
    point_count: u32,
    topology: TopologyLayout,
) -> Vec<CapacityRecordV1> {
    let mut capacities = match topology.kind {
        GpuGatherTopology::CanonicalBodyScan => vec![capacity(
            "solver-topology",
            topology.work_state_base - u32::from(topology.has_manifold_counts) * body_count,
            size_of::<u32>() as u64,
        )],
        GpuGatherTopology::StableBodyEdge => vec![
            capacity(
                "solver-body-edge-index-buffers",
                topology.edge_count * 2,
                size_of::<[u32; 2]>() as u64,
            ),
            capacity(
                "solver-body-edge-radix-blocks",
                topology.block_count * 16,
                size_of::<u32>() as u64,
            ),
            capacity(
                "solver-body-edge-ranges",
                body_count,
                size_of::<[u32; 2]>() as u64,
            ),
        ],
    };
    if topology.has_manifold_counts {
        capacities.push(capacity(
            "solver-manifold-incidence",
            body_count,
            size_of::<u32>() as u64,
        ));
    }
    if topology.has_coloring {
        capacities.extend([
            capacity(
                "solver-color-assignments",
                point_count,
                size_of::<u32>() as u64,
            ),
            capacity(
                "solver-color-body-masks",
                body_count,
                size_of::<u32>() as u64,
            ),
            capacity("solver-color-status", 2, size_of::<u32>() as u64),
            capacity(
                "solver-color-indirect-arguments",
                MAX_COLORS + u32::from(topology.has_island_dispatch),
                size_of::<[u32; 3]>() as u64,
            ),
        ]);
        if topology.has_compact_coloring {
            capacities.extend([
                capacity("solver-color-counts", MAX_COLORS, size_of::<u32>() as u64),
                capacity(
                    "solver-color-offsets",
                    COLOR_OFFSET_COUNT,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-color-cursors", MAX_COLORS, size_of::<u32>() as u64),
                capacity(
                    "solver-color-compacted-points",
                    point_count,
                    size_of::<u32>() as u64,
                ),
            ]);
            if topology.has_parallel_compaction {
                let block_color_elements = topology.color_block_count * MAX_COLORS;
                capacities.extend([
                    capacity(
                        "solver-color-block-counts",
                        block_color_elements,
                        size_of::<u32>() as u64,
                    ),
                    capacity(
                        "solver-color-block-offsets",
                        block_color_elements,
                        size_of::<u32>() as u64,
                    ),
                    capacity(
                        "solver-color-local-ranks",
                        point_count,
                        size_of::<u32>() as u64,
                    ),
                ]);
            }
        }
        if topology.has_incremental_coloring {
            capacities.extend([
                capacity(
                    "solver-color-persistent-identity-tables",
                    topology.color_identity_capacity * 2,
                    size_of::<[u32; 6]>() as u64,
                ),
                capacity(
                    "solver-color-maintenance-counts",
                    4,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-color-maintenance-state", 4, size_of::<u32>() as u64),
            ]);
        }
        if topology.has_parallel_coloring {
            capacities.extend([
                capacity(
                    "solver-color-conflict-winners",
                    topology.parallel_color_winner_count,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-color-parallel-stats", 3, size_of::<u32>() as u64),
            ]);
        }
        if topology.has_island_dispatch {
            let bins = body_count * MAX_COLORS;
            capacities.extend([
                capacity("solver-island-parents", body_count, size_of::<u32>() as u64),
                capacity("solver-body-islands", body_count, size_of::<u32>() as u64),
                capacity("solver-island-roots", body_count, size_of::<u32>() as u64),
                capacity(
                    "solver-island-body-counts",
                    body_count,
                    size_of::<u32>() as u64,
                ),
                capacity(
                    "solver-island-manifold-counts",
                    body_count,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-island-color-counts", bins, size_of::<u32>() as u64),
                capacity(
                    "solver-island-color-offsets",
                    bins + 1,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-island-color-cursors", bins, size_of::<u32>() as u64),
                capacity(
                    "solver-island-compacted-points",
                    point_count,
                    size_of::<u32>() as u64,
                ),
                capacity("solver-island-count", 1, size_of::<u32>() as u64),
            ]);
        }
    }
    if topology.element_count > topology.work_state_base {
        capacities.push(capacity(
            "solver-active-work",
            SOLVER_WORK_STATE_WORDS + body_count,
            size_of::<u32>() as u64,
        ));
    }
    capacities.sort_by(|left, right| left.class.cmp(&right.class));
    capacities
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
    work_parameters: &wgpu::Buffer,
    color_indirect: Option<&wgpu::Buffer>,
) -> wgpu::BindGroup {
    let mut entries = vec![
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
        wgpu::BindGroupEntry {
            binding: 10,
            resource: work_parameters.as_entire_binding(),
        },
    ];
    if let Some(indirect) = color_indirect {
        entries.push(wgpu::BindGroupEntry {
            binding: 9,
            resource: indirect.as_entire_binding(),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
}

#[allow(clippy::too_many_arguments)]
fn color_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &'static str,
    properties: &wgpu::Buffer,
    points: &wgpu::Buffer,
    topology: &wgpu::Buffer,
    parameters: &wgpu::Buffer,
    work_parameters: &wgpu::Buffer,
    color_indirect: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 3,
                resource: properties.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: points.as_entire_binding(),
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
            wgpu::BindGroupEntry {
                binding: 9,
                resource: color_indirect.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: work_parameters.as_entire_binding(),
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
    fn soft_coupled_graph_uses_configured_substeps_and_biased_iterations() {
        let operation_count = |substeps, iterations| {
            let mut scenario = crate::p5_2_centered_rectangle_stack_scenario(4, 120).unwrap();
            let mut physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
            physics.substeps = substeps;
            physics.biased_iterations = iterations;
            crate::SolverProfileConfig::from_physics(physics)
                .unwrap()
                .apply_to_scenario(&mut scenario)
                .unwrap();
            let body_count = scenario.bodies.len() as u32;
            let point_capacity = body_count * (body_count - 1);
            let scheduling =
                GpuSolverSchedulingPolicyV1::authority_gpu_probe_for_body_capacity(body_count);
            let topology = TopologyLayout::new(
                GpuGatherTopology::StableBodyEdge,
                body_count,
                point_capacity,
                false,
                GpuColorScheduleV4::CompactIndirect,
                GpuColorUnitV4::Manifold,
                scheduling,
            )
            .unwrap();
            let (parameters, operations) = operations_with_extended_solver_policies(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
                scheduling,
                GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
                GpuNormalRelaxationPolicyV1::PointSequential,
                GpuContactCoefficientPolicyV1::AuthorityTunableSoft,
                GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
                GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            );
            assert_eq!(parameters.len(), operations.len());
            operations.len()
        };

        let one_by_one = operation_count(1, 1);
        let two_by_one = operation_count(2, 1);
        let four_by_one = operation_count(4, 1);
        let four_by_two = operation_count(4, 2);
        let eight_by_four = operation_count(8, 4);
        assert!(one_by_one < two_by_one && two_by_one < four_by_one);
        assert!(four_by_one < four_by_two && four_by_two < eight_by_four);
        assert_eq!(four_by_two - four_by_one, 8);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn accepted_fixed_floor_schedule_is_exactly_sixty_eight_dispatches() {
        let scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        let topology = TopologyLayout::new(
            GpuGatherTopology::CanonicalBodyScan,
            2,
            2,
            false,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let (parameters, operations) = operations(
            &scenario,
            2,
            2,
            topology,
            false,
            GpuSolverExecutionV4::experiment(true, true),
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(operations.len(), 68);
        assert_eq!(parameters.len(), operations.len());
        assert_eq!(topology.incident_base, 4);
        assert_eq!(topology.cursor_base, 8);
        assert_eq!(topology.element_count, 10);
    }

    #[test]
    fn selected_dispatch_policy_keeps_the_scalable_schedule_above_the_small_world_tier() {
        let small = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(64);
        let scalable = GpuSolverSchedulingPolicyV1::selected_for_body_capacity(65);
        assert_eq!(small, GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD);
        assert_eq!(scalable, GpuSolverSchedulingPolicyV1::SELECTED);
        assert_eq!(small.color_maintenance, scalable.color_maintenance);
        assert_eq!(
            small.colored_dispatch,
            GpuColoredDispatchV1::WorkgroupPerIsland
        );
        assert_eq!(
            scalable.colored_dispatch,
            GpuColoredDispatchV1::PerColorIndirect
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn soft_feedback_diagnostic_changes_only_second_biased_iteration_parameters() {
        let scenario = crate::p5_2_settling_review_scenario(
            16,
            crate::P5_2SettlingReviewPolicy::SoftCandidate,
        )
        .unwrap();
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            17,
            32,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let (selected_parameters, selected_operations) = operations_with_contact_feedback(
            &scenario,
            17,
            32,
            topology,
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSoftFeedbackPolicyV1::SELECTED,
            GpuNormalRelaxationPolicyV1::SELECTED,
        );
        let (diagnostic_parameters, diagnostic_operations) = operations_with_contact_feedback(
            &scenario,
            17,
            32,
            topology,
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSoftFeedbackPolicyV1::FirstBiasedIterationOnly,
            GpuNormalRelaxationPolicyV1::SELECTED,
        );
        assert_eq!(selected_operations.len(), diagnostic_operations.len());
        assert_eq!(selected_parameters.len(), diagnostic_parameters.len());
        let mut differences = 0;
        for (selected, diagnostic) in selected_parameters.iter().zip(diagnostic_parameters.iter()) {
            assert_eq!(selected.floats0, diagnostic.floats0);
            assert_eq!(selected.floats1, diagnostic.floats1);
            assert_eq!(selected.floats2, diagnostic.floats2);
            assert_eq!(selected.floats3, diagnostic.floats3);
            assert_eq!(selected.uints0, diagnostic.uints0);
            assert_eq!(selected.uints1[..3], diagnostic.uints1[..3]);
            if selected.uints1[3] != diagnostic.uints1[3] {
                differences += 1;
                assert_eq!(selected.uints1[1], 2);
                assert_eq!(selected.uints1[0], 1);
                assert_eq!(selected.uints1[3], 1);
                assert_eq!(diagnostic.uints1[3], 0);
            }
        }
        assert!(differences > 0);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn relaxation_factorial_arms_change_only_predeclared_normal_passes() {
        let scenario = crate::p5_2_minimized_mixed_pile_energy_scenario().unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let descriptors = |policy| {
            let (parameters, operations) = operations_with_contact_feedback(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuSoftFeedbackPolicyV1::SELECTED,
                policy,
            );
            pass_trace_descriptors(&parameters, &operations)
        };

        let omitted = descriptors(GpuNormalRelaxationPolicyV1::OmitForSoftContacts);
        assert_eq!(omitted.len(), 48);
        assert!(
            omitted
                .iter()
                .all(|pass| pass.kind != GpuSolverPassKindV1::NormalRelaxation)
        );
        assert_eq!(
            omitted
                .iter()
                .filter(|pass| pass.kind == GpuSolverPassKindV1::TangentRelaxation)
                .count(),
            8
        );

        let softened = descriptors(GpuNormalRelaxationPolicyV1::SoftCoefficients);
        assert_eq!(softened.len(), 56);
        assert_eq!(
            softened
                .iter()
                .filter(|pass| pass.kind == GpuSolverPassKindV1::NormalRelaxation)
                .count(),
            8
        );
        for pass in softened {
            assert_eq!(
                pass.parameters.floats2[3],
                u32::from(pass.kind == GpuSolverPassKindV1::NormalRelaxation) as f32
            );
        }

        let sequential = descriptors(GpuNormalRelaxationPolicyV1::PointSequential);
        assert_eq!(sequential.len(), 56);
        for pass in sequential {
            assert_eq!(
                pass.parameters.floats2[3],
                if pass.kind == GpuSolverPassKindV1::NormalRelaxation {
                    2.0
                } else {
                    0.0
                }
            );
        }

        let projection = descriptors(GpuNormalRelaxationPolicyV1::CoupledEnergyProjection);
        assert_eq!(projection.len(), 56);
        for pass in projection {
            assert_eq!(
                pass.parameters.floats2[3],
                if pass.kind == GpuSolverPassKindV1::NormalRelaxation {
                    3.0
                } else {
                    0.0
                }
            );
        }

        let hard =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        assert!(
            GpuNormalRelaxationPolicyV1::SoftCoefficients
                .validate(&hard.settings, GpuSolverExecutionV4::SELECTED)
                .is_err()
        );
    }

    #[test]
    fn selected_colored_pass_trace_has_exact_physical_operation_boundaries() {
        let scenario = crate::p5_2_minimized_mixed_pile_energy_scenario().unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let (parameters, operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            topology,
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
        );
        let descriptors = pass_trace_descriptors(&parameters, &operations);
        assert_eq!(descriptors.len(), 56);
        assert_eq!(descriptors[0].kind, GpuSolverPassKindV1::IntegrateVelocity);
        assert_eq!(descriptors[0].substep, 0);
        assert_eq!(
            descriptors.last().unwrap().kind,
            GpuSolverPassKindV1::Restitution
        );
        assert_eq!(descriptors.last().unwrap().substep, 3);
        assert_eq!(
            descriptors
                .iter()
                .filter(|pass| pass.kind == GpuSolverPassKindV1::NormalBiased)
                .count(),
            8
        );
        assert_eq!(
            descriptors
                .iter()
                .filter(|pass| pass.kind == GpuSolverPassKindV1::NormalRelaxation)
                .count(),
            8
        );
        assert!(
            descriptors
                .iter()
                .filter(|pass| pass.color_passes != 0)
                .all(|pass| pass.first_color == Some(0)
                    && pass.last_color == Some(31)
                    && pass.color_passes == 32)
        );
        assert!(descriptors.iter().all(|pass| pass.version == 1));
    }

    #[test]
    fn stable_body_edge_schedule_scales_with_body_key_width() {
        let scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let (parameters, operations) = operations(
            &scenario,
            2,
            2,
            topology,
            true,
            GpuSolverExecutionV4::experiment(true, true),
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(topology.edge_count, 4);
        assert_eq!(topology.radix_passes, 1);
        assert_eq!(topology.dispatches(), 6);
        assert_eq!(operations.len(), 70);
        assert_eq!(parameters.len(), operations.len());

        let scale = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            1_024,
            512,
            false,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        assert_eq!(scale.radix_passes, 3);
        assert_eq!(scale.dispatches(), 12);
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn fixed_floor_retains_the_authored_s1_6_control() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        assert_eq!(
            GpuSolverExecutionV4::SELECTED.normal_solver,
            GpuNormalSolverV4::ManifoldBlock
        );
        assert_eq!(
            GpuSolverExecutionV4::SELECTED.accumulation,
            GpuSolverAccumulationV4::ColoredInPlace
        );
        validate_authored_settings(&scenario).unwrap();
        scenario.settings.iterations = 2;
        assert!(validate_authored_settings(&scenario).is_err());
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn scheduling_comparators_are_versioned_and_limited_to_compact_manifolds() {
        let execution = GpuSolverExecutionV4::manifold_colored_block_experiment(true, true);
        GpuSolverSchedulingPolicyV1::STANDARD
            .validate(GpuSolverExecutionV4::SELECTED)
            .unwrap();
        GpuSolverSchedulingPolicyV1::INCREMENTAL
            .validate(execution)
            .unwrap();
        GpuSolverSchedulingPolicyV1::PARALLEL_COMPACTION
            .validate(execution)
            .unwrap();
        GpuSolverSchedulingPolicyV1::PARALLEL_COLORING
            .validate(execution)
            .unwrap();
        GpuSolverSchedulingPolicyV1::STATIC_LATE
            .validate(execution)
            .unwrap();
        GpuSolverSchedulingPolicyV1::WORKGROUP_PER_ISLAND
            .validate(execution)
            .unwrap();
        assert!(
            GpuSolverSchedulingPolicyV1::INCREMENTAL
                .validate(GpuSolverExecutionV4::compact_colored_experiment(true, true))
                .is_err()
        );

        let parallel = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            1_024,
            2_048,
            false,
            execution.color_schedule,
            execution.color_unit,
            GpuSolverSchedulingPolicyV1::PARALLEL_COMPACTION,
        )
        .unwrap();
        assert!(parallel.has_parallel_compaction);
        assert_eq!(parallel.color_block_count, 32);
        assert_eq!(parallel.color_compaction_dispatches(), 4);
        let speculative = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            1_024,
            2_048,
            false,
            execution.color_schedule,
            execution.color_unit,
            GpuSolverSchedulingPolicyV1::PARALLEL_COLORING,
        )
        .unwrap();
        assert!(speculative.has_parallel_coloring);
        assert!(speculative.has_parallel_compaction);
        assert_eq!(speculative.parallel_color_winner_count, 32_768);
        assert_eq!(speculative.color_construction_dispatches(), 98);
        assert_eq!(speculative.color_compaction_dispatches(), 4);
        let static_late = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            1_024,
            2_048,
            false,
            execution.color_schedule,
            execution.color_unit,
            GpuSolverSchedulingPolicyV1::STATIC_LATE,
        )
        .unwrap();
        assert!(static_late.has_parallel_coloring);
        assert!(static_late.has_parallel_compaction);
        assert!(static_late.static_late);
        assert_eq!(GPU_STATIC_LATE_COLOR_COUNT, 4);
        assert_eq!(GPU_STATIC_LATE_FIRST_COLOR, 28);
        assert!(
            GpuSolverSchedulingPolicyV1 {
                version: GPU_SOLVER_SCHEDULING_POLICY_VERSION + 1,
                ..GpuSolverSchedulingPolicyV1::STANDARD
            }
            .validate(GpuSolverExecutionV4::SELECTED)
            .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn static_late_is_named_unpromoted_and_attested_in_colored_passes() {
        assert!(!GpuSolverSchedulingPolicyV1::SELECTED.uses_static_late());
        assert!(!GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD.uses_static_late());
        assert_eq!(
            GpuSolverSchedulingPolicyV1::selected_for_body_capacity(16),
            GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD
        );
        assert_eq!(
            GpuSolverSchedulingPolicyV1::static_late_for_body_capacity(16),
            GpuSolverSchedulingPolicyV1::STATIC_LATE_SMALL_WORLD
        );

        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::STATIC_LATE,
        )
        .unwrap();
        let (parameters, operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            topology,
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::STATIC_LATE,
        );
        let descriptors = pass_trace_descriptors(&parameters, &operations);
        assert!(descriptors.iter().all(|descriptor| {
            descriptor.color_passes == 0 || descriptor.parameters.floats1[3] == 1.0
        }));
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter.uints1[3] & 8 != 0)
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn fused_normal_tangent_is_named_unpromoted_and_halves_contact_sweeps() {
        assert!(
            !GpuSolverSchedulingPolicyV1::SELECTED
                .colored_dispatch
                .fuses_normal_tangent()
        );
        assert!(
            !GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD
                .colored_dispatch
                .fuses_normal_tangent()
        );
        assert_eq!(
            GpuSolverSchedulingPolicyV1::fused_normal_tangent_for_body_capacity(16),
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT_SMALL_WORLD
        );
        assert_eq!(
            GpuSolverSchedulingPolicyV1::fused_normal_tangent_for_body_capacity(65),
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT
        );
        assert!(
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT
                .validate(GpuSolverExecutionV4::compact_colored_experiment(true, true))
                .is_err()
        );
        assert!(
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT
                .validate(GpuSolverExecutionV4::manifold_colored_block_experiment(
                    true, false,
                ))
                .is_err()
        );

        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let per_color = |policy| {
            TopologyLayout::new(
                GpuGatherTopology::StableBodyEdge,
                body_count,
                point_capacity,
                false,
                GpuColorScheduleV4::CompactIndirect,
                GpuColorUnitV4::Manifold,
                policy,
            )
            .unwrap()
        };
        let (selected_parameters, selected_operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            per_color(GpuSolverSchedulingPolicyV1::SELECTED),
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
        );
        let (fused_parameters, fused_operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            per_color(GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT),
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT,
        );
        let selected_descriptors =
            pass_trace_descriptors(&selected_parameters, &selected_operations);
        let fused_descriptors = pass_trace_descriptors(&fused_parameters, &fused_operations);
        assert_eq!(selected_descriptors.len(), 56);
        assert_eq!(fused_descriptors.len(), 40);
        assert_eq!(
            selected_operations.len() - fused_operations.len(),
            scenario.settings.substeps as usize
                * scenario.settings.iterations as usize
                * 2
                * MAX_COLORS as usize
        );
        assert!(fused_descriptors.iter().all(|descriptor| {
            !matches!(
                descriptor.kind,
                GpuSolverPassKindV1::TangentBiased | GpuSolverPassKindV1::TangentRelaxation
            )
        }));
        assert!(fused_descriptors.iter().any(|descriptor| {
            descriptor.kind == GpuSolverPassKindV1::NormalBiased
                && descriptor.parameters.uints1[1] == 7
        }));
        assert!(fused_descriptors.iter().any(|descriptor| {
            descriptor.kind == GpuSolverPassKindV1::NormalRelaxation
                && descriptor.parameters.uints1[1] == 8
        }));

        let island_topology = |policy| {
            TopologyLayout::new(
                GpuGatherTopology::StableBodyEdge,
                body_count,
                point_capacity,
                false,
                GpuColorScheduleV4::CompactIndirect,
                GpuColorUnitV4::Manifold,
                policy,
            )
            .unwrap()
        };
        let (_, selected_island_operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            island_topology(GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD),
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
        );
        let (_, fused_island_operations) = operations(
            &scenario,
            body_count,
            point_capacity,
            island_topology(GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT_SMALL_WORLD),
            false,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::FUSED_NORMAL_TANGENT_SMALL_WORLD,
        );
        assert_eq!(
            selected_island_operations.len() - fused_island_operations.len(),
            scenario.settings.substeps as usize * scenario.settings.iterations as usize * 2
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn static_stiff_hard_changes_only_the_attested_static_coefficient_tuple() {
        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let parameters = |coefficient_policy| {
            operations_with_solver_policies(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuSoftFeedbackPolicyV1::SELECTED,
                GpuNormalRelaxationPolicyV1::SELECTED,
                coefficient_policy,
            )
        };
        let (selected_parameters, selected_operations) =
            parameters(GpuContactCoefficientPolicyV1::SELECTED);
        let (stiff_parameters, stiff_operations) =
            parameters(GpuContactCoefficientPolicyV1::StaticStiffHard);
        assert_eq!(selected_operations.len(), stiff_operations.len());
        assert_eq!(selected_parameters.len(), stiff_parameters.len());
        let mut changed = 0;
        for (selected, stiff) in selected_parameters.iter().zip(stiff_parameters.iter()) {
            assert_eq!(selected.floats0, stiff.floats0);
            assert_eq!(selected.floats1, stiff.floats1);
            assert_eq!(selected.floats2, stiff.floats2);
            assert_eq!(selected.uints0, stiff.uints0);
            assert_eq!(selected.uints1, stiff.uints1);
            if selected.floats3 != stiff.floats3 {
                changed += 1;
                assert_eq!(selected.floats3, [0.0; 4]);
                assert_eq!(stiff.floats3, [0.4, 1.0, 0.0, 1.0]);
            }
        }
        assert!(changed > 0);

        let soft =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::SoftCandidate)
                .unwrap();
        assert!(
            GpuContactCoefficientPolicyV1::StaticStiffHard
                .validate(&soft.settings)
                .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn authority_transform_storage_is_candidate_only_and_prepared_layout_is_unchanged() {
        assert_eq!(size_of::<PreparedPointRaw>(), 96);
        let body_count = 12;
        let point_count = 22;
        let selected = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_count,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let candidate = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_count,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE,
        )
        .unwrap();
        assert!(!selected.has_authority_initial_transforms);
        assert!(candidate.has_authority_initial_transforms);
        assert_eq!(
            candidate.element_count - candidate.authority_initial_transforms_base,
            body_count * 3
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn fixed_anchor_substeps_prepare_once_and_refresh_only_separation() {
        assert_eq!(
            GpuSubstepContactPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMass
        );
        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let schedule = |policy| {
            operations_with_extended_solver_policies(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuSoftFeedbackPolicyV1::SELECTED,
                GpuNormalRelaxationPolicyV1::SELECTED,
                GpuContactCoefficientPolicyV1::SELECTED,
                policy,
                GpuRestitutionTimingPolicyV1::SELECTED,
            )
        };
        let (_, selected_operations) = schedule(GpuSubstepContactPolicyV1::SELECTED);
        let (fixed_parameters, fixed_operations) =
            schedule(GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation);
        assert_eq!(
            selected_operations.len() - fixed_operations.len(),
            scenario.settings.substeps as usize - 1
        );
        let descriptors = pass_trace_descriptors(&fixed_parameters, &fixed_operations);
        assert_eq!(
            descriptors
                .iter()
                .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::PrepareContacts)
                .count(),
            1
        );
        let refreshes = descriptors
            .iter()
            .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::RefreshContacts)
            .collect::<Vec<_>>();
        assert_eq!(refreshes.len(), scenario.settings.substeps as usize);
        assert!(
            refreshes
                .iter()
                .all(|descriptor| descriptor.parameters.uints0[3] == 2)
        );

        let soft =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::SoftCandidate)
                .unwrap();
        assert!(
            GpuSubstepContactPolicyV1::FixedAnchorAnalyticalSeparation
                .validate(
                    &soft.settings,
                    GpuSolverExecutionV4::SELECTED,
                    GpuNormalRelaxationPolicyV1::SELECTED,
                )
                .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn retained_impulse_refresh_changes_only_inter_substep_reset_words() {
        assert_eq!(
            GpuSubstepContactPolicyV1::SELECTED,
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMass
        );
        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let schedule = |policy| {
            operations_with_extended_solver_policies(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuSoftFeedbackPolicyV1::SELECTED,
                GpuNormalRelaxationPolicyV1::SELECTED,
                GpuContactCoefficientPolicyV1::SELECTED,
                policy,
                GpuRestitutionTimingPolicyV1::SELECTED,
            )
        };
        let (selected_parameters, selected_operations) =
            schedule(GpuSubstepContactPolicyV1::SELECTED);
        let (candidate_parameters, candidate_operations) =
            schedule(GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses);
        assert_eq!(selected_operations.len(), candidate_operations.len());
        let selected = pass_trace_descriptors(&selected_parameters, &selected_operations);
        let candidate = pass_trace_descriptors(&candidate_parameters, &candidate_operations);
        assert_eq!(selected.len(), candidate.len());
        let mut changed = 0;
        for (selected, candidate) in selected.iter().zip(&candidate) {
            let mut expected = *selected;
            if selected.kind == GpuSolverPassKindV1::RefreshContacts
                && selected.parameters.uints0[3] == 1
            {
                expected.parameters.uints0[3] = 0;
                changed += 1;
            }
            assert_eq!(&expected, candidate);
        }
        assert_eq!(changed, scenario.settings.substeps - 1);
        assert_eq!(
            candidate
                .iter()
                .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::PrepareContacts)
                .count(),
            1
        );
        assert_eq!(
            candidate
                .iter()
                .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::RefreshContacts)
                .count(),
            (scenario.settings.substeps * 2 - 1) as usize
        );
        assert!(
            candidate
                .iter()
                .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::RefreshContacts)
                .all(|descriptor| descriptor.parameters.uints0[3] == 0)
        );

        let soft =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::SoftCandidate)
                .unwrap();
        assert!(
            GpuSubstepContactPolicyV1::RefreshAnchorsAndMassRetainImpulses
                .validate(
                    &soft.settings,
                    GpuSolverExecutionV4::SELECTED,
                    GpuNormalRelaxationPolicyV1::SELECTED,
                )
                .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn restitution_after_substeps_is_one_standalone_final_stage() {
        assert_eq!(
            GpuRestitutionTimingPolicyV1::SELECTED,
            GpuRestitutionTimingPolicyV1::AfterEachSubstep
        );
        let scenario =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
        let body_count = scenario.bodies.len() as u32;
        let point_capacity = body_count * (body_count - 1);
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            body_count,
            point_capacity,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::SELECTED,
        )
        .unwrap();
        let schedule = |policy| {
            operations_with_extended_solver_policies(
                &scenario,
                body_count,
                point_capacity,
                topology,
                false,
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::SELECTED,
                GpuSoftFeedbackPolicyV1::SELECTED,
                GpuNormalRelaxationPolicyV1::SELECTED,
                GpuContactCoefficientPolicyV1::SELECTED,
                GpuSubstepContactPolicyV1::SELECTED,
                policy,
            )
        };
        let (selected_parameters, selected_operations) =
            schedule(GpuRestitutionTimingPolicyV1::SELECTED);
        let (final_parameters, final_operations) =
            schedule(GpuRestitutionTimingPolicyV1::AfterSubsteps);
        let selected_descriptors =
            pass_trace_descriptors(&selected_parameters, &selected_operations);
        let final_descriptors = pass_trace_descriptors(&final_parameters, &final_operations);
        assert_eq!(
            selected_descriptors
                .iter()
                .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::Restitution)
                .count(),
            scenario.settings.substeps as usize
        );
        let final_restitution = final_descriptors
            .iter()
            .filter(|descriptor| descriptor.kind == GpuSolverPassKindV1::Restitution)
            .collect::<Vec<_>>();
        assert_eq!(final_restitution.len(), 1);
        assert_eq!(final_restitution[0].substep, scenario.settings.substeps - 1);
        assert_eq!(
            selected_operations.len() - final_operations.len(),
            (scenario.settings.substeps as usize - 1) * MAX_COLORS as usize
        );

        let soft =
            crate::p5_2_settling_review_scenario(8, crate::P5_2SettlingReviewPolicy::SoftCandidate)
                .unwrap();
        assert!(
            GpuRestitutionTimingPolicyV1::AfterSubsteps
                .validate(
                    &soft.settings,
                    GpuSolverExecutionV4::SELECTED,
                    GpuNormalRelaxationPolicyV1::SELECTED,
                    GpuContactCoefficientPolicyV1::SELECTED,
                    GpuSubstepContactPolicyV1::SELECTED,
                )
                .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn experiment_schedule_is_bounded_and_dispatch_accounted() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(120).unwrap();
        scenario.settings.iterations = 2;
        let topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let (_, with_relaxation) = operations(
            &scenario,
            2,
            2,
            topology,
            false,
            GpuSolverExecutionV4::experiment(true, true),
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        let (_, without_relaxation) = operations(
            &scenario,
            2,
            2,
            topology,
            false,
            GpuSolverExecutionV4::experiment(false, false),
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(with_relaxation.len(), 102);
        assert_eq!(without_relaxation.len(), 70);
        validate_experiment_settings(
            &scenario.settings,
            GpuSolverExecutionV4::experiment(true, true),
        )
        .unwrap();
        validate_experiment_settings(
            &scenario.settings,
            GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::ScalarMassSplit, true, true),
        )
        .unwrap();
        let block_execution =
            GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::ManifoldBlock, true, true);
        validate_experiment_settings(&scenario.settings, block_execution).unwrap();
        scenario.settings.fixed_dt = physbox_contracts::FiniteF32::new(1.0 / 75.0).unwrap();
        validate_experiment_settings(&scenario.settings, block_execution).unwrap();
        let block_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let (_, block_operations) = operations(
            &scenario,
            2,
            2,
            block_topology,
            false,
            block_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(block_operations.len(), with_relaxation.len());
        assert_eq!(block_topology.dispatches(), topology.dispatches());
        assert_eq!(block_topology.element_count, topology.element_count);
        let split_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            true,
            GpuColorScheduleV4::None,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let split_execution =
            GpuSolverExecutionV4::normal_experiment(GpuNormalSolverV4::BlockMassSplit, true, true);
        let (_, split_operations) = operations(
            &scenario,
            2,
            2,
            split_topology,
            false,
            split_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(split_operations.len(), with_relaxation.len() + 1);
        assert_eq!(split_topology.dispatches(), topology.dispatches() + 1);
        assert_eq!(split_topology.element_count, topology.element_count + 2);
        let colored_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::FixedFullGrid,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let colored_execution = GpuSolverExecutionV4::colored_experiment(true, true);
        let (_, colored_operations) = operations(
            &scenario,
            2,
            2,
            colored_topology,
            false,
            colored_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(colored_operations.len(), 1_303);
        assert!(
            colored_operations
                .iter()
                .all(|operation| operation.indirect_offset.is_none())
        );
        assert_eq!(colored_topology.dispatches(), topology.dispatches() + 1);
        assert_eq!(colored_topology.element_count, topology.element_count + 6);
        let compact_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Point,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let compact_execution = GpuSolverExecutionV4::compact_colored_experiment(true, true);
        let (_, compact_operations) = operations(
            &scenario,
            2,
            2,
            compact_topology,
            false,
            compact_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(compact_operations.len(), colored_operations.len() + 1);
        assert_eq!(
            compact_operations
                .iter()
                .filter(|operation| operation.indirect_offset.is_some())
                .count(),
            40 * MAX_COLORS as usize
        );
        assert_eq!(
            compact_topology.element_count,
            colored_topology.element_count + MAX_COLORS * 2 + COLOR_OFFSET_COUNT + 2
        );
        let manifold_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::STANDARD,
        )
        .unwrap();
        let manifold_execution =
            GpuSolverExecutionV4::manifold_colored_block_experiment(true, true);
        manifold_execution.validate().unwrap();
        let (_, manifold_operations) = operations(
            &scenario,
            2,
            2,
            manifold_topology,
            false,
            manifold_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        assert_eq!(manifold_operations.len(), compact_operations.len());
        assert_eq!(
            manifold_topology.element_count,
            compact_topology.element_count
        );
        assert_eq!(manifold_topology.color_unit, GpuColorUnitV4::Manifold);
        let incremental_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::INCREMENTAL,
        )
        .unwrap();
        let (incremental_parameters, incremental_operations) = operations(
            &scenario,
            2,
            2,
            incremental_topology,
            false,
            manifold_execution,
            GpuSolverSchedulingPolicyV1::INCREMENTAL,
        );
        assert!(incremental_topology.has_incremental_coloring);
        assert_eq!(
            incremental_topology.element_count,
            manifold_topology.element_count
                + incremental_topology.color_identity_capacity * 6 * 2
                + 4
                + 4
        );
        assert_eq!(incremental_operations.len(), manifold_operations.len());
        assert_eq!(
            incremental_parameters[0].uints1[3],
            incremental_topology.color_cursors_base
        );
        assert_eq!(incremental_parameters[0].floats0[3] as u32 & (1 << 2), 0);
        let island_topology = TopologyLayout::new(
            GpuGatherTopology::StableBodyEdge,
            2,
            2,
            false,
            GpuColorScheduleV4::CompactIndirect,
            GpuColorUnitV4::Manifold,
            GpuSolverSchedulingPolicyV1::WORKGROUP_PER_ISLAND,
        )
        .unwrap();
        let (_, island_operations) = operations(
            &scenario,
            2,
            2,
            island_topology,
            false,
            manifold_execution,
            GpuSolverSchedulingPolicyV1::WORKGROUP_PER_ISLAND,
        );
        assert!(island_topology.has_island_dispatch);
        assert_eq!(
            island_topology.dispatches(),
            manifold_topology.dispatches() + 1
        );
        assert_eq!(
            island_operations.len(),
            manifold_operations.len() - 40 * MAX_COLORS as usize + 40 + 1
        );
        assert_eq!(
            island_operations
                .iter()
                .filter_map(|operation| operation.indirect_offset)
                .collect::<Vec<_>>(),
            vec![ISLAND_INDIRECT_OFFSET; 40]
        );
        let alternating_execution =
            GpuSolverExecutionV4::alternating_manifold_colored_block_experiment(true, true);
        alternating_execution.validate().unwrap();
        let (_, alternating_operations) = operations(
            &scenario,
            2,
            2,
            manifold_topology,
            false,
            alternating_execution,
            GpuSolverSchedulingPolicyV1::STANDARD,
        );
        let alternating_offsets = alternating_operations
            .iter()
            .filter_map(|operation| operation.indirect_offset)
            .collect::<Vec<_>>();
        assert_eq!(alternating_offsets.len(), 40 * MAX_COLORS as usize);
        assert_eq!(
            alternating_offsets[..MAX_COLORS as usize],
            (0..MAX_COLORS)
                .map(|color| u64::from(color * 3) * size_of::<u32>() as u64)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            alternating_offsets[3 * MAX_COLORS as usize],
            u64::from((MAX_COLORS - 1) * 3) * size_of::<u32>() as u64
        );
        assert!(!color_sweep_reversed(alternating_execution, 0, 0, 1));
        assert!(color_sweep_reversed(alternating_execution, 1, 0, 1));
        assert!(color_sweep_reversed(alternating_execution, 0, 1, 2));
        assert!(!color_sweep_reversed(alternating_execution, 1, 0, 2));
        assert!(
            GpuSolverExecutionV4 {
                normal_solver: GpuNormalSolverV4::ManifoldBlock,
                ..colored_execution
            }
            .validate()
            .is_err()
        );
        scenario.settings.iterations = 9;
        assert!(
            validate_experiment_settings(
                &scenario.settings,
                GpuSolverExecutionV4::experiment(true, true),
            )
            .is_err()
        );
    }

    #[cfg(not(feature = "production-validation"))]
    #[test]
    fn branch_robust_refresh_changes_only_the_declared_separation_dag() {
        let shader = include_str!("body_floor_solver.wgsl");
        assert_eq!(shader.matches("if (params.uints0.w == 4u)").count(), 1);
        assert_eq!(
            shader
                .matches("let rotation_displacement_a = authority_anchor_a - fixed.anchor_a;")
                .count(),
            1
        );
        assert_eq!(
            shader
                .matches("let rotation_displacement_b = authority_anchor_b - fixed.anchor_b;")
                .count(),
            1
        );
        assert_eq!(
            shader
                .matches("rotation_displacement_b - rotation_displacement_a;")
                .count(),
            1
        );
        assert_eq!(
            shader
                .matches("let robust_motion = delta_position + relative_rotation_displacement;")
                .count(),
            1
        );
        assert_eq!(
            shader
                .matches("fixed.data0.w = input.material.x + robust_projection;")
                .count(),
            1
        );
    }
}
