//! S14 validation-only analytical-separation formulation and forward bound.
//!
//! Transcendental results are materialized as binary32 inputs. The bound then
//! covers the explicitly ordered binary32 arithmetic DAG; it does not claim a
//! bound on a platform's sine or cosine implementation.

use physbox_contracts::Vec2;
use serde::Serialize;

pub const BRANCH_ROBUST_FORMULA_VERSION: u32 = 1;
pub const BRANCH_ROBUST_ERROR_BOUND_VERSION: u32 = 1;
pub const BRANCH_ROBUST_CANDIDATE_ID: &str = "authority-band-cancellation-robust-v1";
pub const BINARY32_UNIT_ROUNDOFF: f64 = 1.0 / 16_777_216.0;

#[derive(Clone, Copy, Debug)]
pub struct AuthoritySeparationInputsV1 {
    pub normal: Vec2,
    pub anchor_a: Vec2,
    pub anchor_b: Vec2,
    pub position_a: Vec2,
    pub position_b: Vec2,
    pub initial_position_a: Vec2,
    pub initial_position_b: Vec2,
    pub angle_a: f32,
    pub angle_b: f32,
    pub initial_angle_a: f32,
    pub initial_angle_b: f32,
    pub input_separation: f32,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobustSeparationV1 {
    pub version: u32,
    pub error_bound_version: u32,
    pub separation: f32,
    pub separation_bits: u32,
    pub real_shadow: f64,
    pub observed_absolute_error: f64,
    pub absolute_error_bound: f64,
    pub lower_endpoint: f32,
    pub lower_endpoint_bits: u32,
    pub upper_endpoint: f32,
    pub upper_endpoint_bits: u32,
    pub normal_range_assumption_satisfied: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobustTraceValueV1 {
    pub name: &'static str,
    pub value: f32,
    pub bits: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobustEvaluationTraceV1 {
    pub result: BranchRobustSeparationV1,
    pub values: Vec<BranchRobustTraceValueV1>,
}

#[derive(Clone, Copy)]
struct Bounded {
    computed: f32,
    exact: f64,
    error: f64,
}

impl Bounded {
    fn input(value: f32) -> Self {
        Self {
            computed: value,
            exact: f64::from(value),
            error: 0.0,
        }
    }

    fn add(self, other: Self) -> Self {
        let computed = self.computed + other.computed;
        let exact = self.exact + other.exact;
        let propagated = self.error + other.error;
        Self {
            computed,
            exact,
            error: propagated + gamma(1) * f64::from(computed).abs(),
        }
    }

    fn sub(self, other: Self) -> Self {
        let computed = self.computed - other.computed;
        let exact = self.exact - other.exact;
        let propagated = self.error + other.error;
        Self {
            computed,
            exact,
            error: propagated + gamma(1) * f64::from(computed).abs(),
        }
    }

    fn mul(self, other: Self) -> Self {
        let computed = self.computed * other.computed;
        let exact = self.exact * other.exact;
        let propagated = self.exact.abs() * other.error
            + other.exact.abs() * self.error
            + self.error * other.error;
        Self {
            computed,
            exact,
            error: propagated + gamma(1) * f64::from(computed).abs(),
        }
    }
}

#[derive(Default)]
struct Trace {
    values: Vec<BranchRobustTraceValueV1>,
}

impl Trace {
    fn retain(&mut self, name: &'static str, value: Bounded) -> Bounded {
        self.values.push(BranchRobustTraceValueV1 {
            name,
            value: value.computed,
            bits: value.computed.to_bits(),
        });
        value
    }
}

pub fn gamma(operations: u32) -> f64 {
    let product = f64::from(operations) * BINARY32_UNIT_ROUNDOFF;
    product / (1.0 - product)
}

pub fn evaluate_original_authority_separation_v1(
    input: AuthoritySeparationInputsV1,
) -> BranchRobustEvaluationTraceV1 {
    evaluate(input, false)
}

pub fn evaluate_branch_robust_authority_separation_v1(
    input: AuthoritySeparationInputsV1,
) -> BranchRobustEvaluationTraceV1 {
    evaluate(input, true)
}

fn evaluate(input: AuthoritySeparationInputsV1, robust: bool) -> BranchRobustEvaluationTraceV1 {
    let mut trace = Trace::default();
    let px_a = Bounded::input(input.position_a.x);
    let py_a = Bounded::input(input.position_a.y);
    let px_b = Bounded::input(input.position_b.x);
    let py_b = Bounded::input(input.position_b.y);
    let initial_px_a = Bounded::input(input.initial_position_a.x);
    let initial_py_a = Bounded::input(input.initial_position_a.y);
    let initial_px_b = Bounded::input(input.initial_position_b.x);
    let initial_py_b = Bounded::input(input.initial_position_b.y);
    let delta_position_a_x = trace.retain("delta-position-a-x", px_a.sub(initial_px_a));
    let delta_position_a_y = trace.retain("delta-position-a-y", py_a.sub(initial_py_a));
    let delta_position_b_x = trace.retain("delta-position-b-x", px_b.sub(initial_px_b));
    let delta_position_b_y = trace.retain("delta-position-b-y", py_b.sub(initial_py_b));
    let delta_position_x = trace.retain(
        "delta-position-x",
        delta_position_b_x.sub(delta_position_a_x),
    );
    let delta_position_y = trace.retain(
        "delta-position-y",
        delta_position_b_y.sub(delta_position_a_y),
    );
    let delta_angle_a = trace.retain(
        "delta-angle-a",
        Bounded::input(input.angle_a).sub(Bounded::input(input.initial_angle_a)),
    );
    let delta_angle_b = trace.retain(
        "delta-angle-b",
        Bounded::input(input.angle_b).sub(Bounded::input(input.initial_angle_b)),
    );
    let (sin_a, cos_a) = delta_angle_a.computed.sin_cos();
    let (sin_b, cos_b) = delta_angle_b.computed.sin_cos();
    let cos_a = trace.retain("cos-delta-angle-a", Bounded::input(cos_a));
    let sin_a = trace.retain("sin-delta-angle-a", Bounded::input(sin_a));
    let cos_b = trace.retain("cos-delta-angle-b", Bounded::input(cos_b));
    let sin_b = trace.retain("sin-delta-angle-b", Bounded::input(sin_b));
    let anchor_a_x = Bounded::input(input.anchor_a.x);
    let anchor_a_y = Bounded::input(input.anchor_a.y);
    let anchor_b_x = Bounded::input(input.anchor_b.x);
    let anchor_b_y = Bounded::input(input.anchor_b.y);
    let rotated_anchor_a_x = trace.retain(
        "rotated-anchor-a-x",
        cos_a.mul(anchor_a_x).sub(sin_a.mul(anchor_a_y)),
    );
    let rotated_anchor_a_y = trace.retain(
        "rotated-anchor-a-y",
        sin_a.mul(anchor_a_x).add(cos_a.mul(anchor_a_y)),
    );
    let rotated_anchor_b_x = trace.retain(
        "rotated-anchor-b-x",
        cos_b.mul(anchor_b_x).sub(sin_b.mul(anchor_b_y)),
    );
    let rotated_anchor_b_y = trace.retain(
        "rotated-anchor-b-y",
        sin_b.mul(anchor_b_x).add(cos_b.mul(anchor_b_y)),
    );
    let normal_x = Bounded::input(input.normal.x);
    let normal_y = Bounded::input(input.normal.y);
    let input_separation = Bounded::input(input.input_separation);

    let separation = if robust {
        let displacement_a_x = trace.retain(
            "rotation-displacement-a-x",
            rotated_anchor_a_x.sub(anchor_a_x),
        );
        let displacement_a_y = trace.retain(
            "rotation-displacement-a-y",
            rotated_anchor_a_y.sub(anchor_a_y),
        );
        let displacement_b_x = trace.retain(
            "rotation-displacement-b-x",
            rotated_anchor_b_x.sub(anchor_b_x),
        );
        let displacement_b_y = trace.retain(
            "rotation-displacement-b-y",
            rotated_anchor_b_y.sub(anchor_b_y),
        );
        let relative_displacement_x = trace.retain(
            "relative-rotation-displacement-x",
            displacement_b_x.sub(displacement_a_x),
        );
        let relative_displacement_y = trace.retain(
            "relative-rotation-displacement-y",
            displacement_b_y.sub(displacement_a_y),
        );
        let motion_x = trace.retain(
            "robust-motion-x",
            delta_position_x.add(relative_displacement_x),
        );
        let motion_y = trace.retain(
            "robust-motion-y",
            delta_position_y.add(relative_displacement_y),
        );
        let projection = trace.retain(
            "robust-projection",
            motion_x.mul(normal_x).add(motion_y.mul(normal_y)),
        );
        trace.retain("robust-separation", input_separation.add(projection))
    } else {
        let prepared_delta_x = trace.retain("prepared-anchor-delta-x", anchor_b_x.sub(anchor_a_x));
        let prepared_delta_y = trace.retain("prepared-anchor-delta-y", anchor_b_y.sub(anchor_a_y));
        let prepared_projection = trace.retain(
            "prepared-anchor-projection",
            prepared_delta_x
                .mul(normal_x)
                .add(prepared_delta_y.mul(normal_y)),
        );
        let base_separation =
            trace.retain("base-separation", input_separation.sub(prepared_projection));
        let rotated_delta_x = trace.retain(
            "rotated-anchor-delta-x",
            rotated_anchor_b_x.sub(rotated_anchor_a_x),
        );
        let rotated_delta_y = trace.retain(
            "rotated-anchor-delta-y",
            rotated_anchor_b_y.sub(rotated_anchor_a_y),
        );
        let motion_x = trace.retain("original-motion-x", delta_position_x.add(rotated_delta_x));
        let motion_y = trace.retain("original-motion-y", delta_position_y.add(rotated_delta_y));
        let projection = trace.retain(
            "original-projection",
            motion_x.mul(normal_x).add(motion_y.mul(normal_y)),
        );
        trace.retain("original-separation", base_separation.add(projection))
    };

    let bound = separation.error;
    let lower_endpoint = round_down(f64::from(separation.computed) - bound);
    let upper_endpoint = round_up(f64::from(separation.computed) + bound);
    BranchRobustEvaluationTraceV1 {
        result: BranchRobustSeparationV1 {
            version: BRANCH_ROBUST_FORMULA_VERSION,
            error_bound_version: BRANCH_ROBUST_ERROR_BOUND_VERSION,
            separation: separation.computed,
            separation_bits: separation.computed.to_bits(),
            real_shadow: separation.exact,
            observed_absolute_error: (f64::from(separation.computed) - separation.exact).abs(),
            absolute_error_bound: bound,
            lower_endpoint,
            lower_endpoint_bits: lower_endpoint.to_bits(),
            upper_endpoint,
            upper_endpoint_bits: upper_endpoint.to_bits(),
            normal_range_assumption_satisfied: trace
                .values
                .iter()
                .all(|value| value.value == 0.0 || value.value.abs() >= f32::MIN_POSITIVE),
        },
        values: trace.values,
    }
}

fn round_down(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) > value {
        next_down(rounded)
    } else {
        rounded
    }
}

fn round_up(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) < value {
        next_up(rounded)
    } else {
        rounded
    }
}

fn next_down(value: f32) -> f32 {
    if value.is_nan() || value == f32::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f32::from_bits(1);
    }
    let bits = value.to_bits();
    f32::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

fn next_up(value: f32) -> f32 {
    if value.is_nan() || value == f32::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f32::from_bits(1);
    }
    let bits = value.to_bits();
    f32::from_bits(if value > 0.0 { bits + 1 } else { bits - 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn real_shadows_are_algebraically_equivalent() {
        let original = evaluate_original_authority_separation_v1(inputs());
        let robust = evaluate_branch_robust_authority_separation_v1(inputs());
        assert!((original.result.real_shadow - robust.result.real_shadow).abs() <= 1.0e-15);
        assert!(original.result.observed_absolute_error <= original.result.absolute_error_bound);
        assert!(robust.result.observed_absolute_error <= robust.result.absolute_error_bound);
    }

    #[test]
    fn outward_endpoints_enclose_real_shadow() {
        let robust = evaluate_branch_robust_authority_separation_v1(inputs()).result;
        assert!(f64::from(robust.lower_endpoint) <= robust.real_shadow);
        assert!(f64::from(robust.upper_endpoint) >= robust.real_shadow);
    }
}
