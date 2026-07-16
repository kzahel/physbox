use std::collections::{BTreeMap, BTreeSet};

use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, CollisionCapacityV1, CollisionManifoldV1, FiniteF32,
    PersistentContactKeyV1, PersistentContactV1, ScenarioV1, SettlingObservationV1, ShapeV1,
};
use physbox_engine::GpuFixedFloorPassEvidence;
use serde::Serialize;

pub const AWAKE_SETTLING_METRICS_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwakeSettlingWindowV1 {
    pub version: u32,
    pub start_step: u32,
    pub end_step: u32,
    pub samples: u32,
    pub residual_linear_speed_max: f32,
    pub residual_linear_speed_rms: f32,
    pub residual_angular_speed_max: f32,
    pub residual_angular_speed_rms: f32,
    pub kinetic_energy_max: f32,
    pub penetration_max: f32,
    pub separation_max: f32,
    pub contact_feature_churn_total: u64,
    pub contact_feature_churn_max: u32,
    pub normal_impulse_variation_total: f32,
    pub normal_impulse_variation_max: f32,
    pub tangent_impulse_variation_total: f32,
    pub tangent_impulse_variation_max: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactBalanceObservationV1 {
    pub version: u32,
    pub completed_steps: u32,
    pub contact_points: u32,
    pub two_point_manifolds: u32,
    pub max_two_point_normal_impulse_imbalance: f32,
    pub max_body_angular_impulse_abs: f32,
    pub max_normal_impulse: f32,
    pub max_tangent_impulse_abs: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyAuthorityDeltaV1 {
    pub version: u32,
    pub completed_steps: u32,
    pub position_delta_max: f32,
    pub angle_delta_max: f32,
    pub linear_velocity_delta_max: f32,
    pub angular_velocity_delta_max: f32,
    pub gpu_contact_points: u32,
    pub authority_contact_points: u32,
    pub gpu_dynamic_sleeping: u32,
    pub authority_dynamic_sleeping: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwakeFirstDivergenceV1 {
    pub version: u32,
    pub first_step: u32,
    pub reason: String,
    pub delta: BodyAuthorityDeltaV1,
}

pub fn summarize_awake_window(
    observations: &[SettlingObservationV1],
    start_step: u32,
    end_step: u32,
) -> Result<AwakeSettlingWindowV1, String> {
    if start_step == 0 || end_step < start_step {
        return Err("awake settling window is invalid".to_owned());
    }
    let samples = observations
        .iter()
        .filter(|observation| (start_step..=end_step).contains(&observation.completed_steps))
        .collect::<Vec<_>>();
    if samples.is_empty()
        || samples.first().unwrap().completed_steps != start_step
        || samples.last().unwrap().completed_steps != end_step
    {
        return Err("awake settling window does not cover both declared endpoints".to_owned());
    }
    let count = samples.len() as f32;
    let mut output = AwakeSettlingWindowV1 {
        version: AWAKE_SETTLING_METRICS_VERSION,
        start_step,
        end_step,
        samples: samples.len() as u32,
        residual_linear_speed_max: 0.0,
        residual_linear_speed_rms: 0.0,
        residual_angular_speed_max: 0.0,
        residual_angular_speed_rms: 0.0,
        kinetic_energy_max: 0.0,
        penetration_max: 0.0,
        separation_max: 0.0,
        contact_feature_churn_total: 0,
        contact_feature_churn_max: 0,
        normal_impulse_variation_total: 0.0,
        normal_impulse_variation_max: 0.0,
        tangent_impulse_variation_total: 0.0,
        tangent_impulse_variation_max: 0.0,
    };
    for observation in samples {
        output.residual_linear_speed_max = output
            .residual_linear_speed_max
            .max(observation.residual_linear_speed_max.get());
        output.residual_linear_speed_rms += observation.residual_linear_speed_rms.get().powi(2);
        output.residual_angular_speed_max = output
            .residual_angular_speed_max
            .max(observation.residual_angular_speed_max.get());
        output.residual_angular_speed_rms += observation.residual_angular_speed_rms.get().powi(2);
        output.kinetic_energy_max = output
            .kinetic_energy_max
            .max(observation.kinetic_energy.get());
        output.penetration_max = output
            .penetration_max
            .max(observation.penetration_max.get());
        output.separation_max = output.separation_max.max(observation.separation_max.get());
        output.contact_feature_churn_total += u64::from(observation.contact_feature_churn);
        output.contact_feature_churn_max = output
            .contact_feature_churn_max
            .max(observation.contact_feature_churn);
        if let Some(value) = observation.normal_impulse_variation {
            output.normal_impulse_variation_total += value.get();
            output.normal_impulse_variation_max =
                output.normal_impulse_variation_max.max(value.get());
        }
        if let Some(value) = observation.tangent_impulse_variation {
            output.tangent_impulse_variation_total += value.get();
            output.tangent_impulse_variation_max =
                output.tangent_impulse_variation_max.max(value.get());
        }
    }
    output.residual_linear_speed_rms = (output.residual_linear_speed_rms / count).sqrt();
    output.residual_angular_speed_rms = (output.residual_angular_speed_rms / count).sqrt();
    if all_finite([
        output.residual_linear_speed_max,
        output.residual_linear_speed_rms,
        output.residual_angular_speed_max,
        output.residual_angular_speed_rms,
        output.kinetic_energy_max,
        output.penetration_max,
        output.separation_max,
        output.normal_impulse_variation_total,
        output.normal_impulse_variation_max,
        output.tangent_impulse_variation_total,
        output.tangent_impulse_variation_max,
    ]) {
        Ok(output)
    } else {
        Err("awake settling window became non-finite".to_owned())
    }
}

pub fn observe_contact_balance(
    completed_steps: u32,
    evidence: &GpuFixedFloorPassEvidence,
) -> Result<ContactBalanceObservationV1, String> {
    if completed_steps == 0 || evidence.contacts.iter().any(|contact| !contact.valid) {
        return Err("contact balance requires a completed valid pass".to_owned());
    }
    let mut manifolds = BTreeMap::<(u32, u32, u32), Vec<_>>::new();
    let mut angular_impulses = BTreeMap::<u32, f32>::new();
    let mut max_normal_impulse = 0.0_f32;
    let mut max_tangent_impulse_abs = 0.0_f32;
    for contact in &evidence.contacts {
        manifolds
            .entry((contact.body_a, contact.body_b, contact.manifold_index))
            .or_default()
            .push(contact);
        max_normal_impulse = max_normal_impulse.max(contact.normal_impulse.abs());
        max_tangent_impulse_abs = max_tangent_impulse_abs.max(contact.tangent_impulse.abs());
        let impulse = (
            contact.normal.x * contact.normal_impulse + contact.tangent.x * contact.tangent_impulse,
            contact.normal.y * contact.normal_impulse + contact.tangent.y * contact.tangent_impulse,
        );
        *angular_impulses.entry(contact.body_a).or_default() +=
            cross(contact.anchor_a, (-impulse.0, -impulse.1));
        *angular_impulses.entry(contact.body_b).or_default() += cross(contact.anchor_b, impulse);
    }
    let mut two_point_manifolds = 0;
    let mut max_imbalance = 0.0_f32;
    for contacts in manifolds.values().filter(|contacts| contacts.len() == 2) {
        two_point_manifolds += 1;
        let left = contacts[0].normal_impulse.abs();
        let right = contacts[1].normal_impulse.abs();
        let total = left + right;
        let imbalance = if total > 0.0 {
            (left - right).abs() / total
        } else {
            0.0
        };
        max_imbalance = max_imbalance.max(imbalance);
    }
    let max_body_angular_impulse_abs = angular_impulses
        .values()
        .fold(0.0_f32, |maximum, value| maximum.max(value.abs()));
    if all_finite([
        max_imbalance,
        max_body_angular_impulse_abs,
        max_normal_impulse,
        max_tangent_impulse_abs,
    ]) {
        Ok(ContactBalanceObservationV1 {
            version: AWAKE_SETTLING_METRICS_VERSION,
            completed_steps,
            contact_points: evidence.contacts.len() as u32,
            two_point_manifolds,
            max_two_point_normal_impulse_imbalance: max_imbalance,
            max_body_angular_impulse_abs,
            max_normal_impulse,
            max_tangent_impulse_abs,
        })
    } else {
        Err("contact balance became non-finite".to_owned())
    }
}

pub fn compare_awake_authority(
    completed_steps: u32,
    gpu_bodies: &[BodyStateV1],
    authority_bodies: &[BodyStateV1],
    gpu_contact_points: u32,
    authority_contact_points: u32,
) -> Result<BodyAuthorityDeltaV1, String> {
    if completed_steps == 0 || gpu_bodies.len() != authority_bodies.len() {
        return Err("awake authority comparison has incompatible bodies".to_owned());
    }
    let authority = authority_bodies
        .iter()
        .map(|body| (body.id, body))
        .collect::<BTreeMap<_, _>>();
    let mut output = BodyAuthorityDeltaV1 {
        version: AWAKE_SETTLING_METRICS_VERSION,
        completed_steps,
        position_delta_max: 0.0,
        angle_delta_max: 0.0,
        linear_velocity_delta_max: 0.0,
        angular_velocity_delta_max: 0.0,
        gpu_contact_points,
        authority_contact_points,
        gpu_dynamic_sleeping: 0,
        authority_dynamic_sleeping: authority_bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0 && body.sleeping)
            .count() as u32,
    };
    for gpu in gpu_bodies {
        let reference = authority
            .get(&gpu.id)
            .ok_or_else(|| format!("authority omitted body {:?}", gpu.id))?;
        output.position_delta_max = output.position_delta_max.max(
            ((gpu.position.x - reference.position.x).powi(2)
                + (gpu.position.y - reference.position.y).powi(2))
            .sqrt(),
        );
        output.angle_delta_max = output
            .angle_delta_max
            .max((gpu.angle.get() - reference.angle.get()).abs());
        output.linear_velocity_delta_max = output.linear_velocity_delta_max.max(
            ((gpu.linear_velocity.x - reference.linear_velocity.x).powi(2)
                + (gpu.linear_velocity.y - reference.linear_velocity.y).powi(2))
            .sqrt(),
        );
        output.angular_velocity_delta_max = output
            .angular_velocity_delta_max
            .max((gpu.angular_velocity.get() - reference.angular_velocity.get()).abs());
        output.gpu_dynamic_sleeping += u32::from(gpu.inverse_mass.get() > 0.0 && gpu.sleeping);
    }
    if all_finite([
        output.position_delta_max,
        output.angle_delta_max,
        output.linear_velocity_delta_max,
        output.angular_velocity_delta_max,
    ]) {
        Ok(output)
    } else {
        Err("awake authority delta became non-finite".to_owned())
    }
}

pub fn first_awake_divergence(deltas: &[BodyAuthorityDeltaV1]) -> Option<AwakeFirstDivergenceV1> {
    deltas.iter().find_map(|delta| {
        let reason = if delta.position_delta_max > 0.0
            || delta.angle_delta_max > 0.0
            || delta.linear_velocity_delta_max > 0.0
            || delta.angular_velocity_delta_max > 0.0
        {
            "body kinematics first differ bit-exactly"
        } else if delta.gpu_contact_points != delta.authority_contact_points {
            "contact-point counts first differ"
        } else if delta.gpu_dynamic_sleeping != delta.authority_dynamic_sleeping {
            "dynamic awake state first differs"
        } else {
            return None;
        };
        Some(AwakeFirstDivergenceV1 {
            version: AWAKE_SETTLING_METRICS_VERSION,
            first_step: delta.completed_steps,
            reason: reason.to_owned(),
            delta: delta.clone(),
        })
    })
}

fn cross(anchor: physbox_contracts::Vec2, impulse: (f32, f32)) -> f32 {
    anchor.x * impulse.1 - anchor.y * impulse.0
}

fn all_finite<const N: usize>(values: [f32; N]) -> bool {
    values.into_iter().all(f32::is_finite)
}

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
    use physbox_engine::{GpuFixedFloorContactEvidence, p5_2_centered_rectangle_stack_scenario};

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

    #[test]
    fn awake_window_aggregates_every_sample_without_hiding_peaks() {
        let scenario = p5_2_centered_rectangle_stack_scenario(2, 3).unwrap();
        let mut observations = Vec::new();
        for step in 1..=3 {
            let mut bodies = scenario.bodies.clone();
            bodies[1].linear_velocity.x = step as f32;
            observations.push(
                observe_settling(
                    &scenario,
                    SettlingSnapshotInput {
                        completed_steps: step,
                        bodies: &bodies,
                        manifolds: &[],
                        cache: &[],
                        capacities: &[],
                        previous_manifolds: Some(&[]),
                        previous_cache: Some(&[]),
                        support_height: 0.0,
                    },
                )
                .unwrap(),
            );
        }
        let window = summarize_awake_window(&observations, 1, 3).unwrap();
        assert_eq!(window.samples, 3);
        assert_eq!(window.residual_linear_speed_max, 3.0);
        assert!(window.residual_linear_speed_rms > 1.0);
        assert!(summarize_awake_window(&observations, 2, 4).is_err());
    }

    #[test]
    fn contact_balance_reports_two_point_impulse_and_torque_asymmetry() {
        let contact = |point_index, anchor_x, normal_impulse| GpuFixedFloorContactEvidence {
            point_index,
            valid: true,
            body_a: 0,
            body_b: 1,
            manifold_index: 0,
            point_in_manifold: point_index,
            normal: physbox_contracts::Vec2::new(0.0, 1.0),
            tangent: physbox_contracts::Vec2::new(1.0, 0.0),
            anchor_a: physbox_contracts::Vec2::new(anchor_x, -0.1),
            anchor_b: physbox_contracts::Vec2::new(anchor_x, 0.1),
            initial_normal_velocity: 0.0,
            normal_effective_mass: 0.5,
            tangent_effective_mass: 0.5,
            separation: 0.0,
            friction: 0.5,
            restitution: 0.0,
            normal_impulse,
            tangent_impulse: 0.0,
            total_normal_impulse: normal_impulse,
            body_a_normal_mass_incidence: 2,
            body_b_normal_mass_incidence: 2,
        };
        let evidence = GpuFixedFloorPassEvidence {
            contacts: vec![contact(0, -0.25, 3.0), contact(1, 0.25, 1.0)],
            contributions: Vec::new(),
            manifold_blocks: Vec::new(),
        };
        let balance = observe_contact_balance(12, &evidence).unwrap();
        assert_eq!(balance.contact_points, 2);
        assert_eq!(balance.two_point_manifolds, 1);
        assert_eq!(balance.max_two_point_normal_impulse_imbalance, 0.5);
        assert!(balance.max_body_angular_impulse_abs > 0.0);
    }

    #[test]
    fn awake_authority_delta_matches_bodies_by_generation_safe_id() {
        let scenario = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        let mut authority = scenario.bodies.clone();
        authority[1].position.x += 0.25;
        authority[2].angular_velocity = finite(0.5);
        let delta = compare_awake_authority(1, &scenario.bodies, &authority, 4, 3).unwrap();
        assert_eq!(delta.position_delta_max, 0.25);
        assert_eq!(delta.angular_velocity_delta_max, 0.5);
        assert_eq!(delta.gpu_contact_points, 4);
        assert_eq!(delta.authority_contact_points, 3);
        let first = first_awake_divergence(&[delta]).unwrap();
        assert_eq!(first.first_step, 1);
        assert!(first.reason.contains("kinematics"));
    }
}
