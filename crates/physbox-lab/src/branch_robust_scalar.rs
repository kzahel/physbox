//! Frozen S14 scalar perturbation lanes around the independently frozen
//! cancellation-robust separation formulation.

use serde::{Deserialize, Serialize};

use crate::branch_robust::{
    BranchRobustSeparationV1, evaluate_branch_robust_authority_separation_v1,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchRobustPerturbationV1 {
    Nearest,
    UniformLow,
    UniformHigh,
    PointAlternating,
    SubstepAlternating,
    StepAlternating,
    HashedSignV1,
}

impl BranchRobustPerturbationV1 {
    pub const ALL: [Self; 7] = [
        Self::Nearest,
        Self::UniformLow,
        Self::UniformHigh,
        Self::PointAlternating,
        Self::SubstepAlternating,
        Self::StepAlternating,
        Self::HashedSignV1,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::UniformLow => "uniform-low",
            Self::UniformHigh => "uniform-high",
            Self::PointAlternating => "point-alternating",
            Self::SubstepAlternating => "substep-alternating",
            Self::StepAlternating => "step-alternating",
            Self::HashedSignV1 => "hashed-sign-v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobustRunStatisticsV1 {
    pub evaluations: u64,
    pub nearest_selections: u64,
    pub lower_selections: u64,
    pub upper_selections: u64,
    pub positive_separations: u64,
    pub zero_separations: u64,
    pub negative_separations: u64,
    pub bound_violations: u64,
    pub non_finite_results: u64,
    pub normal_range_violations: u64,
    pub absolute_error_bound_high_water: f64,
}

pub fn evaluate_branch_robust_scalar_v1(
    inputs: crate::branch_robust::AuthoritySeparationInputsV1,
) -> BranchRobustSeparationV1 {
    evaluate_branch_robust_authority_separation_v1(inputs).result
}

pub fn select_branch_robust_perturbation_v1(
    result: BranchRobustSeparationV1,
    lane: BranchRobustPerturbationV1,
    zero_based_step: u32,
    substep: u32,
    manifold_id: u32,
    point_index: u32,
    statistics: &mut BranchRobustRunStatisticsV1,
) -> f32 {
    let selection = match lane {
        BranchRobustPerturbationV1::Nearest => 0,
        BranchRobustPerturbationV1::UniformLow => -1,
        BranchRobustPerturbationV1::UniformHigh => 1,
        BranchRobustPerturbationV1::PointAlternating => alternating_sign(manifold_id ^ point_index),
        BranchRobustPerturbationV1::SubstepAlternating => alternating_sign(substep),
        BranchRobustPerturbationV1::StepAlternating => alternating_sign(zero_based_step),
        BranchRobustPerturbationV1::HashedSignV1 => {
            let hash = zero_based_step.wrapping_mul(0x9e37_79b9)
                ^ substep.wrapping_mul(0x85eb_ca6b)
                ^ manifold_id.wrapping_mul(0xc2b2_ae35)
                ^ point_index.wrapping_mul(0x27d4_eb2f);
            alternating_sign(hash)
        }
    };
    let selected = match selection {
        -1 => {
            statistics.lower_selections += 1;
            result.lower_endpoint
        }
        1 => {
            statistics.upper_selections += 1;
            result.upper_endpoint
        }
        _ => {
            statistics.nearest_selections += 1;
            result.separation
        }
    };
    statistics.evaluations += 1;
    if selected > 0.0 {
        statistics.positive_separations += 1;
    } else if selected < 0.0 {
        statistics.negative_separations += 1;
    } else {
        statistics.zero_separations += 1;
    }
    if !selected.is_finite()
        || !result.separation.is_finite()
        || !result.absolute_error_bound.is_finite()
    {
        statistics.non_finite_results += 1;
    }
    if !result.normal_range_assumption_satisfied {
        statistics.normal_range_violations += 1;
    }
    if result.observed_absolute_error > result.absolute_error_bound
        || f64::from(result.lower_endpoint) > result.real_shadow
        || f64::from(result.upper_endpoint) < result.real_shadow
    {
        statistics.bound_violations += 1;
    }
    statistics.absolute_error_bound_high_water = statistics
        .absolute_error_bound_high_water
        .max(result.absolute_error_bound);
    selected
}

const fn alternating_sign(value: u32) -> i8 {
    if value & 1 == 0 { -1 } else { 1 }
}

#[cfg(test)]
mod tests {
    use physbox_contracts::Vec2;

    use super::*;
    use crate::branch_robust::AuthoritySeparationInputsV1;

    fn inputs() -> AuthoritySeparationInputsV1 {
        AuthoritySeparationInputsV1 {
            normal: Vec2::new(0.0, 1.0),
            anchor_a: Vec2::new(0.35, -0.125),
            anchor_b: Vec2::new(0.35, -0.125),
            position_a: Vec2::new(0.0, 1.0),
            position_b: Vec2::new(0.0, 1.25),
            initial_position_a: Vec2::new(0.0, 1.0),
            initial_position_b: Vec2::new(0.0, 1.25),
            angle_a: 1.0e-4,
            angle_b: -2.0e-4,
            initial_angle_a: 0.0,
            initial_angle_b: 0.0,
            input_separation: 0.0,
        }
    }

    #[test]
    fn common_translation_is_metamorphically_inert() {
        let input = inputs();
        let mut translated = input;
        let translation = Vec2::new(1024.0, -512.0);
        translated.position_a.x += translation.x;
        translated.position_a.y += translation.y;
        translated.position_b.x += translation.x;
        translated.position_b.y += translation.y;
        translated.initial_position_a.x += translation.x;
        translated.initial_position_a.y += translation.y;
        translated.initial_position_b.x += translation.x;
        translated.initial_position_b.y += translation.y;
        assert_eq!(
            evaluate_branch_robust_scalar_v1(input).separation_bits,
            evaluate_branch_robust_scalar_v1(translated).separation_bits
        );
    }

    #[test]
    fn zero_motion_preserves_input_separation() {
        let mut input = inputs();
        input.position_a = input.initial_position_a;
        input.position_b = input.initial_position_b;
        input.angle_a = input.initial_angle_a;
        input.angle_b = input.initial_angle_b;
        assert_eq!(
            evaluate_branch_robust_scalar_v1(input).separation_bits,
            input.input_separation.to_bits()
        );
    }

    #[test]
    fn seven_lanes_are_fixed_and_accounted() {
        assert_eq!(
            BranchRobustPerturbationV1::ALL.map(BranchRobustPerturbationV1::id),
            [
                "nearest",
                "uniform-low",
                "uniform-high",
                "point-alternating",
                "substep-alternating",
                "step-alternating",
                "hashed-sign-v1",
            ]
        );
        let result = evaluate_branch_robust_scalar_v1(inputs());
        for lane in BranchRobustPerturbationV1::ALL {
            let mut statistics = BranchRobustRunStatisticsV1::default();
            let selected =
                select_branch_robust_perturbation_v1(result, lane, 0, 0, 4, 1, &mut statistics);
            assert!(selected.is_finite());
            assert_eq!(statistics.evaluations, 1);
            assert_eq!(statistics.bound_violations, 0);
            assert_eq!(statistics.non_finite_results, 0);
            assert_eq!(
                statistics.nearest_selections
                    + statistics.lower_selections
                    + statistics.upper_selections,
                1
            );
        }
    }
}
