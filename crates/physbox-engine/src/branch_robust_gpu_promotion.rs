use serde::Serialize;

use crate::body_floor_solver::{
    GpuContactCoefficientPolicyV1, GpuNormalRelaxationPolicyV1, GpuRestitutionTimingPolicyV1,
    GpuSoftFeedbackPolicyV1, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1,
    GpuSubstepContactPolicyV1,
};
use physbox_contracts::SolverSettingsV1;

pub const BRANCH_ROBUST_GPU_PROMOTION_POLICY_VERSION: u32 = 1;
pub const BRANCH_ROBUST_GPU_PROMOTION_CANDIDATE_ID: &str = "branch-robust-product-promotion-v1";
pub const BRANCH_ROBUST_GPU_PROMOTION_FORMULA_ID: &str = "authority-band-cancellation-robust-v1";

/// The sole validation-only Track A candidate authorized by S17.
///
/// The descriptor inherits the exact S13 execution tuple and changes only the
/// analytical separation expression to the already frozen S14 displacement
/// formula. Product constructors cannot request this policy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobustGpuPromotionPolicyV1 {
    pub version: u32,
    pub candidate_id: &'static str,
    pub product_selectable: bool,
    pub base_candidate_id: &'static str,
    pub separation_formula_id: &'static str,
    pub changed_operation_family: &'static str,
    pub contact_hertz: f32,
    pub damping_ratio: f32,
    pub static_contact_hertz_multiplier: f32,
    pub substeps: u32,
    pub biased_passes_per_substep: u32,
    pub relaxation_passes_per_substep: u32,
    pub restitution_after_each_substep: bool,
}

impl BranchRobustGpuPromotionPolicyV1 {
    pub const AUTHORIZED: Self = Self {
        version: BRANCH_ROBUST_GPU_PROMOTION_POLICY_VERSION,
        candidate_id: BRANCH_ROBUST_GPU_PROMOTION_CANDIDATE_ID,
        product_selectable: false,
        base_candidate_id: crate::AUTHORITY_GPU_PROBE_CANDIDATE_ID,
        separation_formula_id: BRANCH_ROBUST_GPU_PROMOTION_FORMULA_ID,
        changed_operation_family: "analytical-separation-expression-only",
        contact_hertz: 30.0,
        damping_ratio: 10.0,
        static_contact_hertz_multiplier: 2.0,
        substeps: 4,
        biased_passes_per_substep: 1,
        relaxation_passes_per_substep: 1,
        restitution_after_each_substep: true,
    };

    pub fn validate(self) -> Result<(), String> {
        if self != Self::AUTHORIZED {
            return Err("branch-robust GPU promotion policy differs from S17".to_owned());
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
                "branch-robust GPU promotion candidate cannot be selected before human review"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

pub(crate) fn validate_branch_robust_gpu_promotion_settings(
    settings: &SolverSettingsV1,
) -> Result<(), String> {
    crate::authority_gpu_probe::validate_authority_gpu_probe_settings(settings)
}

pub(crate) fn validate_branch_robust_gpu_promotion_runtime_tuple(
    execution: GpuSolverExecutionV4,
    scheduling: GpuSolverSchedulingPolicyV1,
    soft_feedback: GpuSoftFeedbackPolicyV1,
    normal_relaxation: GpuNormalRelaxationPolicyV1,
    contact_coefficients: GpuContactCoefficientPolicyV1,
    substep_contacts: GpuSubstepContactPolicyV1,
    restitution_timing: GpuRestitutionTimingPolicyV1,
) -> Result<(), String> {
    BranchRobustGpuPromotionPolicyV1::AUTHORIZED.validate_product_selection(false)?;
    if execution != GpuSolverExecutionV4::AUTHORITY_GPU_PROBE
        || (scheduling != GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE
            && scheduling != GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE_SMALL_WORLD)
        || soft_feedback != GpuSoftFeedbackPolicyV1::EveryBiasedIteration
        || normal_relaxation != GpuNormalRelaxationPolicyV1::PointSequential
        || contact_coefficients != GpuContactCoefficientPolicyV1::AuthorityCappedSoft
        || substep_contacts
            != GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation
        || restitution_timing != GpuRestitutionTimingPolicyV1::AfterEachSubstep
    {
        return Err("GPU runtime tuple differs from the authorized S17 candidate".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SelectedGpuSolverPolicySnapshotV1;

    #[test]
    fn authorized_candidate_is_one_unpromoted_s14_on_s13_policy() {
        let policy = BranchRobustGpuPromotionPolicyV1::AUTHORIZED;
        policy.validate().unwrap();
        policy.validate_product_selection(false).unwrap();
        assert_eq!(
            policy.base_candidate_id,
            crate::AUTHORITY_GPU_PROBE_CANDIDATE_ID
        );
        assert_eq!(
            policy.separation_formula_id,
            BRANCH_ROBUST_GPU_PROMOTION_FORMULA_ID
        );
        assert_eq!(
            policy.changed_operation_family,
            "analytical-separation-expression-only"
        );
        assert!(!policy.product_selectable);
        assert_eq!(
            SelectedGpuSolverPolicySnapshotV1::current(),
            SelectedGpuSolverPolicySnapshotV1::FROZEN_AT_S13_AUTHORIZATION
        );
    }

    #[test]
    fn runtime_tuple_accepts_only_branch_robust_substep_policy() {
        validate_branch_robust_gpu_promotion_runtime_tuple(
            GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
            GpuSolverSchedulingPolicyV1::AUTHORITY_GPU_PROBE,
            GpuSoftFeedbackPolicyV1::EveryBiasedIteration,
            GpuNormalRelaxationPolicyV1::PointSequential,
            GpuContactCoefficientPolicyV1::AuthorityCappedSoft,
            GpuSubstepContactPolicyV1::AuthorityBranchRobustFixedAnchorAnalyticalSeparation,
            GpuRestitutionTimingPolicyV1::AfterEachSubstep,
        )
        .unwrap();
        assert!(
            validate_branch_robust_gpu_promotion_runtime_tuple(
                GpuSolverExecutionV4::AUTHORITY_GPU_PROBE,
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
