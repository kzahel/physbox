#![cfg_attr(not(feature = "solver-lab"), allow(dead_code))]

use serde::Serialize;

use crate::body_floor_solver::{
    GpuContactCoefficientPolicyV1, GpuNormalRelaxationPolicyV1, GpuRestitutionTimingPolicyV1,
    GpuSoftFeedbackPolicyV1, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1,
    GpuSubstepContactPolicyV1,
};
use physbox_contracts::SolverSettingsV1;

pub const AUTHORITY_GPU_PROBE_POLICY_VERSION: u32 = 1;
pub const AUTHORITY_GPU_TRANSFORM_STORAGE_VERSION: u32 = 1;
pub const AUTHORITY_GPU_PROBE_CANDIDATE_ID: &str = "authority-x1-x4-current-x5-v1";
pub const AUTHORITY_GPU_DYNAMIC_COLOR_COUNT: u32 = 20;
pub const AUTHORITY_GPU_STATIC_COLOR_COUNT: u32 = 3;
pub const AUTHORITY_GPU_OVERFLOW_COLOR_COUNT: u32 = 1;
pub const AUTHORITY_GPU_COLOR_COUNT: u32 = AUTHORITY_GPU_DYNAMIC_COLOR_COUNT
    + AUTHORITY_GPU_STATIC_COLOR_COUNT
    + AUTHORITY_GPU_OVERFLOW_COLOR_COUNT;

/// The one validation-only GPU policy authorized at the S12.6 checkpoint.
///
/// This descriptor is deliberately separate from the selected solver policy:
/// it freezes the experiment before its shader/runtime implementation exists
/// and cannot be projected through product construction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityGpuProbePolicyV1 {
    pub version: u32,
    pub candidate_id: &'static str,
    pub product_selectable: bool,
    pub prepare_once_per_outer_step: bool,
    pub stable_authority_vertex_pair_identity: bool,
    pub shared_midpoint_anchors: bool,
    pub fixed_prepared_normal_and_mass: bool,
    pub analytical_separation: bool,
    pub retained_evolving_impulses: bool,
    pub normalized_incremental_rotation: bool,
    pub dynamic_colors: u32,
    pub static_colors: u32,
    pub overflow_colors: u32,
    pub overflow_first: bool,
    pub ascending_color_order: bool,
    pub colored_in_place: bool,
    pub contact_hertz: f32,
    pub damping_ratio: f32,
    pub contact_hertz_cap_per_substep: f32,
    pub static_contact_hertz_multiplier: f32,
    pub biased_passes_per_substep: u32,
    pub relaxation_passes_per_substep: u32,
    pub normal_then_friction: bool,
    pub point_sequential: bool,
    pub restitution_after_each_substep: bool,
    pub substeps: u32,
}

impl AuthorityGpuProbePolicyV1 {
    pub const AUTHORIZED: Self = Self {
        version: AUTHORITY_GPU_PROBE_POLICY_VERSION,
        candidate_id: AUTHORITY_GPU_PROBE_CANDIDATE_ID,
        product_selectable: false,
        prepare_once_per_outer_step: true,
        stable_authority_vertex_pair_identity: true,
        shared_midpoint_anchors: true,
        fixed_prepared_normal_and_mass: true,
        analytical_separation: true,
        retained_evolving_impulses: true,
        normalized_incremental_rotation: true,
        dynamic_colors: AUTHORITY_GPU_DYNAMIC_COLOR_COUNT,
        static_colors: AUTHORITY_GPU_STATIC_COLOR_COUNT,
        overflow_colors: AUTHORITY_GPU_OVERFLOW_COLOR_COUNT,
        overflow_first: true,
        ascending_color_order: true,
        colored_in_place: true,
        contact_hertz: 30.0,
        damping_ratio: 10.0,
        contact_hertz_cap_per_substep: 0.125,
        static_contact_hertz_multiplier: 2.0,
        biased_passes_per_substep: 1,
        relaxation_passes_per_substep: 1,
        normal_then_friction: true,
        point_sequential: true,
        restitution_after_each_substep: true,
        substeps: 4,
    };

    pub fn validate(self) -> Result<(), String> {
        if self != Self::AUTHORIZED {
            return Err(
                "authority GPU probe policy differs from the authorized S13 descriptor".to_owned(),
            );
        }
        Ok(())
    }

    pub fn validate_product_selection(
        self,
        product_selection_requested: bool,
    ) -> Result<(), String> {
        self.validate()?;
        if product_selection_requested || self.product_selectable {
            return Err(
                "authority GPU probe is validation-only and cannot be selected by the product"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

pub fn validate_authority_gpu_probe_settings(settings: &SolverSettingsV1) -> Result<(), String> {
    let Some(softness) = settings.contact_softness else {
        return Err("authority GPU probe requires soft contact".to_owned());
    };
    if settings.substeps != AuthorityGpuProbePolicyV1::AUTHORIZED.substeps
        || settings.iterations != 2
        || softness.version != physbox_contracts::CONTACT_SOFTNESS_POLICY_VERSION
        || softness.contact_hertz.get() != AuthorityGpuProbePolicyV1::AUTHORIZED.contact_hertz
        || softness.damping_ratio.get() != AuthorityGpuProbePolicyV1::AUTHORIZED.damping_ratio
    {
        return Err(
            "authority GPU probe settings differ from the frozen S12 source contract".to_owned(),
        );
    }
    Ok(())
}

pub fn validate_authority_gpu_probe_runtime_tuple(
    execution: GpuSolverExecutionV4,
    scheduling: GpuSolverSchedulingPolicyV1,
    soft_feedback: GpuSoftFeedbackPolicyV1,
    normal_relaxation: GpuNormalRelaxationPolicyV1,
    contact_coefficients: GpuContactCoefficientPolicyV1,
    substep_contacts: GpuSubstepContactPolicyV1,
    restitution_timing: GpuRestitutionTimingPolicyV1,
) -> Result<(), String> {
    AuthorityGpuProbePolicyV1::AUTHORIZED.validate_product_selection(false)?;
    if execution != GpuSolverExecutionV4::AUTHORITY_GPU_PROBE
        || (scheduling != GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE
            && scheduling != GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE_SMALL_WORLD)
        || soft_feedback != GpuSoftFeedbackPolicyV1::EveryBiasedIteration
        || normal_relaxation != GpuNormalRelaxationPolicyV1::PointSequential
        || contact_coefficients != GpuContactCoefficientPolicyV1::AuthorityCappedSoft
        || substep_contacts != GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation
        || restitution_timing != GpuRestitutionTimingPolicyV1::AfterEachSubstep
    {
        return Err("GPU runtime tuple differs from the authorized S13 candidate".to_owned());
    }
    Ok(())
}

/// Snapshot of the independently selected product policy. S13 tests bind this
/// value so probe work cannot silently promote itself by changing a SELECTED
/// constant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedGpuSolverPolicySnapshotV1 {
    pub execution: GpuSolverExecutionV4,
    pub scheduling: GpuSolverSchedulingPolicyV1,
    pub small_world_scheduling: GpuSolverSchedulingPolicyV1,
    pub soft_feedback: GpuSoftFeedbackPolicyV1,
    pub normal_relaxation: GpuNormalRelaxationPolicyV1,
    pub contact_coefficients: GpuContactCoefficientPolicyV1,
    pub substep_contacts: GpuSubstepContactPolicyV1,
    pub restitution_timing: GpuRestitutionTimingPolicyV1,
}

impl SelectedGpuSolverPolicySnapshotV1 {
    pub const FROZEN_AT_S13_AUTHORIZATION: Self = Self {
        execution: GpuSolverExecutionV4::SELECTED,
        scheduling: GpuSolverSchedulingPolicyV1::SELECTED,
        small_world_scheduling: GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
        soft_feedback: GpuSoftFeedbackPolicyV1::SELECTED,
        normal_relaxation: GpuNormalRelaxationPolicyV1::SELECTED,
        contact_coefficients: GpuContactCoefficientPolicyV1::SELECTED,
        substep_contacts: GpuSubstepContactPolicyV1::SELECTED,
        restitution_timing: GpuRestitutionTimingPolicyV1::SELECTED,
    };

    pub const fn current() -> Self {
        Self {
            execution: GpuSolverExecutionV4::SELECTED,
            scheduling: GpuSolverSchedulingPolicyV1::SELECTED,
            small_world_scheduling: GpuSolverSchedulingPolicyV1::SELECTED_SMALL_WORLD,
            soft_feedback: GpuSoftFeedbackPolicyV1::SELECTED,
            normal_relaxation: GpuNormalRelaxationPolicyV1::SELECTED,
            contact_coefficients: GpuContactCoefficientPolicyV1::SELECTED,
            substep_contacts: GpuSubstepContactPolicyV1::SELECTED,
            restitution_timing: GpuRestitutionTimingPolicyV1::SELECTED,
        }
    }
}

#[cfg(all(
    test,
    not(feature = "sim-owner"),
    not(feature = "production-validation")
))]
mod tests {
    use super::*;

    #[test]
    fn authorized_probe_is_exactly_one_unpromoted_policy() {
        let policy = AuthorityGpuProbePolicyV1::AUTHORIZED;
        policy.validate().unwrap();
        policy.validate_product_selection(false).unwrap();
        assert_eq!(policy.candidate_id, AUTHORITY_GPU_PROBE_CANDIDATE_ID);
        assert_eq!(
            policy.dynamic_colors + policy.static_colors + policy.overflow_colors,
            AUTHORITY_GPU_COLOR_COUNT
        );
        assert_eq!(policy.biased_passes_per_substep, 1);
        assert_eq!(policy.relaxation_passes_per_substep, 1);
        assert_eq!(policy.substeps, 4);
    }

    #[test]
    fn probe_rejects_product_selection_and_descriptor_drift() {
        assert!(
            AuthorityGpuProbePolicyV1::AUTHORIZED
                .validate_product_selection(true)
                .is_err()
        );
        let mut changed = AuthorityGpuProbePolicyV1::AUTHORIZED;
        changed.biased_passes_per_substep = 2;
        assert!(changed.validate().is_err());
    }

    #[test]
    fn selected_product_policy_matches_the_authorization_snapshot() {
        assert_eq!(
            SelectedGpuSolverPolicySnapshotV1::current(),
            SelectedGpuSolverPolicySnapshotV1::FROZEN_AT_S13_AUTHORIZATION
        );
        assert_ne!(
            serde_json::to_vec(&AuthorityGpuProbePolicyV1::AUTHORIZED).unwrap(),
            serde_json::to_vec(&SelectedGpuSolverPolicySnapshotV1::current()).unwrap()
        );
    }

    #[test]
    fn authorized_descriptor_maps_to_one_exact_runtime_tuple() {
        validate_authority_gpu_probe_runtime_tuple(
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE,
            GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            GpuNormalRelaxationPolicyV1::PointSequential,
            GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
            GpuRestitutionTimingPolicyV1::AfterEachSubstep,
        )
        .unwrap();
        assert!(
            validate_authority_gpu_probe_runtime_tuple(
                GpuSolverExecutionV4::SELECTED,
                GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE,
                GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
                GpuNormalRelaxationPolicyV1::PointSequential,
                GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
                GpuSubstepContactPolicyV1::AuthorityFixedAnchorAnalyticalSeparation,
                GpuRestitutionTimingPolicyV1::AfterEachSubstep,
            )
            .is_err()
        );
    }
}
