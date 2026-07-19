#![forbid(unsafe_code)]

use std::fmt::{Display, Formatter};

use physbox_contracts::{CapacityRecordV1, FiniteF32, Vec2};

pub mod analytic_narrowphase;
pub mod authority_structure;
pub mod branch_robust;
pub mod branch_robust_scalar;
pub mod collision_candidates;
pub mod contact_persistence;
pub mod evidence;
pub mod failure_automation;
pub mod fixture_contacts;
pub mod oracle_transport;
pub mod parallel_normal;
pub mod pass_energy;
pub mod playback;
pub mod scalar;
pub mod scheduler_benchmark;
pub mod settling;
pub mod stationarity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvariantViolation {
    pub invariant: &'static str,
    pub reason: String,
}

impl Display for InvariantViolation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.invariant, self.reason)
    }
}

impl std::error::Error for InvariantViolation {}

pub fn finite_vec(name: &'static str, value: Vec2) -> Result<(), InvariantViolation> {
    if value.x.is_finite() && value.y.is_finite() {
        Ok(())
    } else {
        Err(violation(name, "non-finite vector"))
    }
}

pub fn equal_and_opposite(
    impulse_a: Vec2,
    impulse_b: Vec2,
    tolerance: f32,
) -> Result<(), InvariantViolation> {
    finite_vec("equal-and-opposite", impulse_a)?;
    finite_vec("equal-and-opposite", impulse_b)?;
    if (impulse_a.x + impulse_b.x).abs() <= tolerance
        && (impulse_a.y + impulse_b.y).abs() <= tolerance
    {
        Ok(())
    } else {
        Err(violation(
            "equal-and-opposite",
            "contact contributions do not cancel",
        ))
    }
}

pub fn projected_impulses(
    normal_impulse: FiniteF32,
    tangent_impulse: FiniteF32,
    friction: FiniteF32,
) -> Result<(), InvariantViolation> {
    if normal_impulse.get() < 0.0 {
        return Err(violation("projected-impulse", "normal impulse is negative"));
    }
    let limit = friction.get() * normal_impulse.get();
    if tangent_impulse.get().abs() > limit + 1.0e-6 {
        return Err(violation(
            "projected-impulse",
            "tangent impulse exceeds the Coulomb cone",
        ));
    }
    Ok(())
}

pub fn capacity_is_valid(records: &[CapacityRecordV1]) -> Result<(), InvariantViolation> {
    for record in records {
        if record.overflowed
            || record.used_elements > record.capacity_elements
            || record.high_water_bytes > record.allocated_bytes
        {
            return Err(violation(
                "capacity",
                format!("{} overflowed or exceeded its allocation", record.class),
            ));
        }
    }
    Ok(())
}

pub fn label_swap_symmetry(
    original_a: Vec2,
    original_b: Vec2,
    swapped_a: Vec2,
    swapped_b: Vec2,
    tolerance: f32,
) -> Result<(), InvariantViolation> {
    if distance(original_a, swapped_b) <= tolerance && distance(original_b, swapped_a) <= tolerance
    {
        Ok(())
    } else {
        Err(violation(
            "label-swap-symmetry",
            "swapping body labels changed the paired response",
        ))
    }
}

fn distance(a: Vec2, b: Vec2) -> f32 {
    let x = a.x - b.x;
    let y = a.y - b.y;
    (x * x + y * y).sqrt()
}

fn violation(invariant: &'static str, reason: impl Into<String>) -> InvariantViolation {
    InvariantViolation {
        invariant,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{Validate, canonical_json};
    use physbox_engine::{authored_chain_scenario, authored_one_contact_scenario};

    use super::*;

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    #[test]
    fn engine_scenarios_replay_exactly_through_the_lab() {
        let scenario = authored_one_contact_scenario(1234);
        scenario.validate().unwrap();
        assert_eq!(
            canonical_json(&scenario).unwrap(),
            canonical_json(&authored_one_contact_scenario(1234)).unwrap()
        );
        authored_chain_scenario(1024, 99)
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn deliberately_broken_equal_opposite_response_fails() {
        equal_and_opposite(Vec2::new(2.0, 1.0), Vec2::new(-2.0, -1.0), 1e-6).unwrap();
        assert!(equal_and_opposite(Vec2::new(2.0, 1.0), Vec2::new(-1.0, -1.0), 1e-6).is_err());
    }

    #[test]
    fn deliberately_broken_projection_fails() {
        projected_impulses(finite(2.0), finite(0.5), finite(0.5)).unwrap();
        assert!(projected_impulses(finite(-0.1), finite(0.0), finite(0.5)).is_err());
        assert!(projected_impulses(finite(1.0), finite(0.6), finite(0.5)).is_err());
    }

    #[test]
    fn deliberately_broken_capacity_fails() {
        let good = CapacityRecordV1 {
            class: "contacts".to_owned(),
            capacity_elements: 10,
            used_elements: 10,
            allocated_bytes: 640,
            high_water_bytes: 640,
            overflowed: false,
        };
        capacity_is_valid(std::slice::from_ref(&good)).unwrap();
        let mut broken = good;
        broken.overflowed = true;
        assert!(capacity_is_valid(&[broken]).is_err());
    }

    #[test]
    fn deliberately_broken_label_swap_fails() {
        label_swap_symmetry(
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            1e-6,
        )
        .unwrap();
        assert!(
            label_swap_symmetry(
                Vec2::new(1.0, 0.0),
                Vec2::new(-1.0, 0.0),
                Vec2::ZERO,
                Vec2::ZERO,
                1e-6,
            )
            .is_err()
        );
    }
}
