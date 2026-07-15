use std::collections::{BTreeMap, BTreeSet};

use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, CollisionCapacityV1, CollisionManifoldV1, FiniteF32,
    PersistentContactKeyV1, PersistentContactV1, ScenarioV1, SettlingObservationV1, ShapeV1,
};

pub struct SettlingSnapshotInput<'a> {
    pub completed_steps: u32,
    pub bodies: &'a [BodyStateV1],
    pub manifolds: &'a [CollisionManifoldV1],
    pub cache: &'a [PersistentContactV1],
    pub capacities: &'a [CollisionCapacityV1],
    pub previous_manifolds: Option<&'a [CollisionManifoldV1]>,
    pub previous_cache: Option<&'a [PersistentContactV1]>,
    pub support_height: f32,
}

pub fn observe_settling(
    source: &ScenarioV1,
    input: SettlingSnapshotInput<'_>,
) -> Result<SettlingObservationV1, String> {
    let SettlingSnapshotInput {
        completed_steps,
        bodies,
        manifolds,
        cache,
        capacities,
        previous_manifolds,
        previous_cache,
        support_height,
    } = input;
    if source.bodies.len() != bodies.len() || !support_height.is_finite() {
        return Err("settling observation body count or support height is invalid".to_owned());
    }
    let source_by_id = source
        .bodies
        .iter()
        .map(|body| (body.id, body))
        .collect::<BTreeMap<_, _>>();
    let dynamic = bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .collect::<Vec<_>>();
    if dynamic.is_empty() {
        return Err("settling observation requires dynamic bodies".to_owned());
    }

    let mut linear_max = 0.0_f32;
    let mut angular_max = 0.0_f32;
    let mut linear_squared = 0.0_f32;
    let mut angular_squared = 0.0_f32;
    let mut center_drift_max = 0.0_f32;
    let mut stack_height = 0.0_f32;
    let mut kinetic_energy = 0.0_f32;
    for body in &dynamic {
        let initial = source_by_id
            .get(&body.id)
            .ok_or_else(|| format!("settling body {:?} is absent from the source", body.id))?;
        let linear_speed = (body.linear_velocity.x.powi(2) + body.linear_velocity.y.powi(2)).sqrt();
        let angular_speed = body.angular_velocity.get().abs();
        linear_max = linear_max.max(linear_speed);
        angular_max = angular_max.max(angular_speed);
        linear_squared += linear_speed * linear_speed;
        angular_squared += angular_speed * angular_speed;
        center_drift_max = center_drift_max.max(
            ((body.position.x - initial.position.x).powi(2)
                + (body.position.y - initial.position.y).powi(2))
            .sqrt(),
        );
        stack_height = stack_height.max(shape_upper_y(body) - support_height);
        let mass = body.inverse_mass.get().recip();
        let inertia = if body.inverse_inertia.get() > 0.0 {
            body.inverse_inertia.get().recip()
        } else {
            0.0
        };
        kinetic_energy += 0.5 * mass * linear_speed * linear_speed
            + 0.5 * inertia * angular_speed * angular_speed;
    }

    let mut penetration_max = 0.0_f32;
    let mut separation_max = 0.0_f32;
    let mut current_features = BTreeSet::new();
    for manifold in manifolds {
        for point in &manifold.points {
            penetration_max = penetration_max.max((-point.separation.get()).max(0.0));
            separation_max = separation_max.max(point.separation.get().max(0.0));
            current_features.insert(point.key(manifold.pair));
        }
    }
    let previous_features = previous_manifolds.map(feature_keys).unwrap_or_default();
    let contact_feature_churn = if previous_manifolds.is_some() {
        current_features
            .symmetric_difference(&previous_features)
            .count() as u32
    } else {
        0
    };

    let (normal_impulse_variation, tangent_impulse_variation) = previous_cache
        .map(|previous| impulse_variation(cache, previous))
        .transpose()?
        .map_or((None, None), |(normal, tangent)| {
            (Some(finite(normal)), Some(finite(tangent)))
        });
    let capacities = capacities
        .iter()
        .map(|capacity| CapacityRecordV1 {
            class: capacity.class.clone(),
            capacity_elements: capacity.capacity_elements,
            used_elements: capacity.committed_elements,
            allocated_bytes: capacity.allocated_bytes,
            high_water_bytes: capacity.high_water_bytes,
            overflowed: capacity.overflowed,
        })
        .collect();

    Ok(SettlingObservationV1 {
        completed_steps,
        dynamic_body_count: dynamic.len() as u32,
        residual_linear_speed_max: finite(linear_max),
        residual_linear_speed_rms: finite((linear_squared / dynamic.len() as f32).sqrt()),
        residual_angular_speed_max: finite(angular_max),
        residual_angular_speed_rms: finite((angular_squared / dynamic.len() as f32).sqrt()),
        center_drift_max: finite(center_drift_max),
        stack_height: finite(stack_height.max(0.0)),
        penetration_max: finite(penetration_max),
        separation_max: finite(separation_max),
        contact_points: current_features.len() as u32,
        contact_feature_churn,
        normal_impulse_variation,
        tangent_impulse_variation,
        kinetic_energy: finite(kinetic_energy),
        capacities,
    })
}

pub fn centered_stack_structure(
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    capacity_overflowed: bool,
    support_height: f32,
) -> (bool, String) {
    if capacity_overflowed {
        return (false, "collision capacity overflowed".to_owned());
    }
    if source.bodies.len() != bodies.len() {
        return (false, "body count changed".to_owned());
    }
    let dynamic = bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .collect::<Vec<_>>();
    if dynamic.iter().any(|body| {
        !body.position.x.is_finite()
            || !body.position.y.is_finite()
            || shape_upper_y(body) <= support_height
    }) {
        return (
            false,
            "a dynamic body became invalid or escaped below the support".to_owned(),
        );
    }
    if dynamic
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id || pair[0].position.y >= pair[1].position.y)
    {
        return (false, "rectangle stack order changed".to_owned());
    }
    let speculative_distance = source
        .settings
        .contact_policy
        .map_or(0.0, |policy| policy.speculative_distance.get());
    let mut previous_top = support_height;
    let mut previous_horizontal = None;
    for body in dynamic {
        let ShapeV1::Rectangle { .. } = body.shape else {
            return (
                false,
                "centered stack contains a non-rectangle body".to_owned(),
            );
        };
        let (extent_x, extent_y) = shape_extents(body);
        if body.angle.get().cos().abs() < body.angle.get().sin().abs() {
            return (
                false,
                "a rectangle rotated beyond the upright range".to_owned(),
            );
        }
        let lower = body.position.y - extent_y;
        if lower > previous_top + speculative_distance {
            return (false, "the vertical support chain separated".to_owned());
        }
        let horizontal = (body.position.x - extent_x, body.position.x + extent_x);
        if previous_horizontal.is_some_and(|(left, right)| {
            horizontal.0 > right + speculative_distance
                || left > horizontal.1 + speculative_distance
        }) {
            return (false, "the horizontal support chain separated".to_owned());
        }
        previous_top = body.position.y + extent_y;
        previous_horizontal = Some(horizontal);
    }
    (
        true,
        "all rectangle generations remained finite, supported, and vertically ordered".to_owned(),
    )
}

fn feature_keys(manifolds: &[CollisionManifoldV1]) -> BTreeSet<PersistentContactKeyV1> {
    manifolds
        .iter()
        .flat_map(|manifold| manifold.points.iter().map(|point| point.key(manifold.pair)))
        .collect()
}

fn impulse_variation(
    current: &[PersistentContactV1],
    previous: &[PersistentContactV1],
) -> Result<(f32, f32), String> {
    let current = current
        .iter()
        .map(|value| (value.key, value.value))
        .collect::<BTreeMap<_, _>>();
    let previous = previous
        .iter()
        .map(|value| (value.key, value.value))
        .collect::<BTreeMap<_, _>>();
    let keys = current
        .keys()
        .chain(previous.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut normal = 0.0;
    let mut tangent = 0.0;
    for key in keys {
        let current = current.get(&key);
        let previous = previous.get(&key);
        normal += (current.map_or(0.0, |value| value.normal_impulse.get())
            - previous.map_or(0.0, |value| value.normal_impulse.get()))
        .abs();
        tangent += (current.map_or(0.0, |value| value.tangent_impulse.get())
            - previous.map_or(0.0, |value| value.tangent_impulse.get()))
        .abs();
    }
    if normal.is_finite() && tangent.is_finite() {
        Ok((normal, tangent))
    } else {
        Err("settling impulse variation became non-finite".to_owned())
    }
}

fn shape_upper_y(body: &BodyStateV1) -> f32 {
    body.position.y + shape_extents(body).1
}

fn shape_extents(body: &BodyStateV1) -> (f32, f32) {
    match body.shape {
        ShapeV1::Circle { radius } => (radius.get(), radius.get()),
        ShapeV1::Rectangle { half_extents } => {
            let (sine, cosine) = body.angle.get().sin_cos();
            (
                cosine.abs() * half_extents.x + sine.abs() * half_extents.y,
                sine.abs() * half_extents.x + cosine.abs() * half_extents.y,
            )
        }
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("settling values were checked or derived from finite contracts")
}

#[cfg(test)]
mod tests {
    use physbox_contracts::Validate;
    use physbox_engine::p5_2_centered_rectangle_stack_scenario;

    use super::*;
    use crate::{
        analytic_narrowphase::generate_scalar_narrowphase,
        collision_candidates::enumerate_scalar_candidates,
    };

    #[test]
    fn settling_observation_reports_every_governed_dimension() {
        let scenario = p5_2_centered_rectangle_stack_scenario(4, 600).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let manifolds = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let observation = observe_settling(
            &scenario,
            SettlingSnapshotInput {
                completed_steps: 1,
                bodies: &scenario.bodies,
                manifolds: &manifolds,
                cache: &[],
                capacities: &[],
                previous_manifolds: None,
                previous_cache: None,
                support_height: 0.0,
            },
        )
        .unwrap();
        observation.validate().unwrap();
        assert_eq!(observation.dynamic_body_count, 4);
        assert_eq!(observation.residual_linear_speed_max.get(), 0.0);
        assert_eq!(observation.residual_angular_speed_max.get(), 0.0);
        assert_eq!(observation.center_drift_max.get(), 0.0);
        assert!((observation.stack_height.get() - 1.0).abs() <= f32::EPSILON);
        assert_eq!(observation.contact_points, 8);
        assert_eq!(observation.contact_feature_churn, 0);
        assert!(observation.normal_impulse_variation.is_none());
        assert!(centered_stack_structure(&scenario, &scenario.bodies, false, 0.0).0);
    }
}
