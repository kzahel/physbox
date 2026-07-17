use std::collections::{BTreeMap, BTreeSet};

use physbox_contracts::{
    BodyStateV1, CandidatePairV1, CapacityRecordV1, CollisionCapacityV1, CollisionManifoldV1,
    ContactPointCacheObservationV2, ContactReferenceOwnerV2, FiniteF32, GeometricFeatureKindV1,
    PersistentContactKeyV1, PersistentContactV1, SCHEMA_VERSION, SETTLING_OBSERVATION_V2_VERSION,
    ScenarioV1, SettlingContactObservationV2, SettlingMeasurementPolicyV2, SettlingObservationV1,
    SettlingObservationV2, SettlingSeriesV2, SettlingSummaryV2, ShapeV1, Validate, Vec2,
    WarmCacheStatusV2, canonical_hash,
};
use physbox_engine::{GpuFixedFloorPassEvidence, P5_2SettlingFixtureKind};
use serde::Serialize;

pub const AWAKE_SETTLING_METRICS_VERSION: u32 = 1;
pub const AWAKE_SETTLING_TRACE_VERSION: u32 = SETTLING_OBSERVATION_V2_VERSION;

pub fn awake_settling_measurement_policy_v2() -> SettlingMeasurementPolicyV2 {
    SettlingMeasurementPolicyV2 {
        version: AWAKE_SETTLING_TRACE_VERSION,
        observation_interval_steps: 1,
        transient_end_step: 600,
        settled_start_step: 900,
        settled_end_step: 1_200,
        rest_linear_speed_limit: finite(0.025),
        rest_angular_edge_speed_limit: finite(0.025),
        rest_required_consecutive_observations: 60,
        stable_support_required_consecutive_observations: 30,
        stable_normal_delta_limit_radians: finite(0.5_f32.to_radians()),
        stable_relative_tangent_speed_limit: finite(0.025),
        stable_support_gravity_alignment_min: finite(15.0_f32.to_radians().cos()),
        energy_absolute_epsilon: finite(1.0e-12),
        energy_relative_epsilon: finite(1.0e-5),
        positive_energy_run_limit: 5,
    }
}

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

pub struct SettlingObserverV2 {
    policy: SettlingMeasurementPolicyV2,
    seen_pairs: BTreeSet<CandidatePairV1>,
    stable_support_ages: BTreeMap<CandidatePairV1, u32>,
    previous_completed_steps: u32,
    previous_manifolds: Option<Vec<CollisionManifoldV1>>,
    previous_cache: Option<Vec<PersistentContactV1>>,
}

pub struct SettlingObservationInputV2<'a> {
    pub completed_steps: u32,
    pub bodies: &'a [BodyStateV1],
    pub manifolds: &'a [CollisionManifoldV1],
    pub cache: &'a [PersistentContactV1],
    pub capacities: &'a [CollisionCapacityV1],
    pub support_height: f32,
}

impl SettlingObserverV2 {
    pub fn new(policy: SettlingMeasurementPolicyV2) -> Result<Self, String> {
        policy.validate().map_err(|error| error.to_string())?;
        Ok(Self {
            policy,
            seen_pairs: BTreeSet::new(),
            stable_support_ages: BTreeMap::new(),
            previous_completed_steps: 0,
            previous_manifolds: None,
            previous_cache: None,
        })
    }

    pub const fn policy(&self) -> SettlingMeasurementPolicyV2 {
        self.policy
    }

    pub fn observe(
        &mut self,
        source: &ScenarioV1,
        input: SettlingObservationInputV2<'_>,
    ) -> Result<SettlingObservationV2, String> {
        let SettlingObservationInputV2 {
            completed_steps,
            bodies,
            manifolds,
            cache,
            capacities,
            support_height,
        } = input;
        let expected_step = self
            .previous_completed_steps
            .saturating_add(self.policy.observation_interval_steps);
        if completed_steps != expected_step {
            return Err(format!(
                "settling observer expected completed step {expected_step}, received {completed_steps}"
            ));
        }
        let base = observe_settling(
            source,
            SettlingSnapshotInput {
                completed_steps,
                bodies,
                manifolds,
                cache,
                capacities,
                previous_manifolds: self.previous_manifolds.as_deref(),
                previous_cache: self.previous_cache.as_deref(),
                support_height,
            },
        )?;
        let body_map = bodies
            .iter()
            .map(|body| (body.id, body))
            .collect::<BTreeMap<_, _>>();
        let previous_manifold_map = self
            .previous_manifolds
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|manifold| (manifold.pair, manifold))
            .collect::<BTreeMap<_, _>>();
        let previous_cache_map = cache_map(self.previous_cache.as_deref().unwrap_or_default());
        let current_cache_map = cache_map(cache);
        let action_on_interval = source.actions.iter().any(|action| {
            let step = action.step().saturating_add(1);
            step > completed_steps.saturating_sub(self.policy.observation_interval_steps)
                && step <= completed_steps
        });

        let mut contacts = Vec::with_capacity(manifolds.len());
        for manifold in manifolds {
            let previous = previous_manifold_map.get(&manifold.pair).copied();
            let (current_reference_owner, reference_face_index) = reference_owner(manifold);
            let previous_reference = previous.map(reference_owner);
            let previous_reference_owner = previous_reference.map(|value| value.0);
            let previous_features = previous.map_or_else(Vec::new, manifold_features);
            let current_features = manifold_features(manifold);
            let previous_point_count = previous.map_or(0, |value| value.points.len() as u32);
            let reference_owner_changed =
                previous_reference_owner.is_some_and(|owner| owner != current_reference_owner);
            let point_count_changed =
                previous.is_some() && previous_point_count != manifold.points.len() as u32;
            let feature_set_changed = previous.is_some() && previous_features != current_features;
            let normal_delta_radians =
                previous.map_or(0.0, |value| normal_delta(value.normal, manifold.normal));
            let normal_changed = previous.is_some()
                && normal_delta_radians > self.policy.stable_normal_delta_limit_radians.get();
            let relative_tangent_speed_max = relative_tangent_speed_max(manifold, &body_map)?;
            let gravity_length = (source.settings.gravity.x * source.settings.gravity.x
                + source.settings.gravity.y * source.settings.gravity.y)
                .sqrt();
            let gravity_alignment = if gravity_length > 0.0 {
                (manifold.normal.x * source.settings.gravity.x
                    + manifold.normal.y * source.settings.gravity.y)
                    .abs()
                    / gravity_length
            } else {
                0.0
            };
            let stable_candidate = previous.is_some()
                && !action_on_interval
                && !normal_changed
                && relative_tangent_speed_max
                    <= self.policy.stable_relative_tangent_speed_limit.get()
                && gravity_alignment >= self.policy.stable_support_gravity_alignment_min.get();
            let stable_support_age_steps = if stable_candidate {
                self.stable_support_ages
                    .get(&manifold.pair)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(self.policy.observation_interval_steps)
            } else {
                0
            };
            if stable_support_age_steps == 0 {
                self.stable_support_ages.remove(&manifold.pair);
            } else {
                self.stable_support_ages
                    .insert(manifold.pair, stable_support_age_steps);
            }

            let pair_seen_before = self.seen_pairs.contains(&manifold.pair);
            let mut cache_observations = Vec::with_capacity(manifold.points.len());
            for key in &current_features {
                let previous_value = previous_cache_map.get(key).copied();
                let current_value = current_cache_map.get(key).copied();
                let status = if current_value.is_none() {
                    WarmCacheStatusV2::CurrentEntryMissing
                } else if previous_value.is_some() {
                    WarmCacheStatusV2::Hit
                } else if previous.is_none() {
                    if pair_seen_before {
                        WarmCacheStatusV2::Recontact
                    } else {
                        WarmCacheStatusV2::PairStarted
                    }
                } else if normal_changed {
                    WarmCacheStatusV2::NormalChanged
                } else if point_count_changed {
                    WarmCacheStatusV2::PointCountChanged
                } else if reference_owner_changed {
                    WarmCacheStatusV2::ReferenceOwnerChanged
                } else {
                    WarmCacheStatusV2::FeatureChanged
                };
                cache_observations.push(ContactPointCacheObservationV2 {
                    key: *key,
                    status,
                    previous_normal_impulse: previous_value.map(|value| value.normal_impulse),
                    previous_tangent_impulse: previous_value.map(|value| value.tangent_impulse),
                    current_normal_impulse: current_value.map(|value| value.normal_impulse),
                    current_tangent_impulse: current_value.map(|value| value.tangent_impulse),
                });
            }
            let has_cache_miss = cache_observations
                .iter()
                .any(|value| value.status != WarmCacheStatusV2::Hit);
            let unexplained_stable_churn = stable_support_age_steps
                >= self
                    .policy
                    .stable_support_required_consecutive_observations
                    .saturating_mul(self.policy.observation_interval_steps)
                && (reference_owner_changed
                    || point_count_changed
                    || feature_set_changed
                    || has_cache_miss);
            contacts.push(SettlingContactObservationV2 {
                pair: manifold.pair,
                reference_owner: current_reference_owner,
                reference_face_index,
                previous_reference_owner,
                point_count: manifold.points.len() as u32,
                previous_point_count,
                normal: manifold.normal,
                previous_normal: previous.map(|value| value.normal),
                normal_delta_radians: finite(normal_delta_radians),
                relative_tangent_speed_max: finite(relative_tangent_speed_max),
                stable_support: stable_support_age_steps > 0,
                stable_support_age_steps,
                reference_owner_changed,
                point_count_changed,
                feature_set_changed,
                normal_changed,
                unexplained_stable_churn,
                previous_features,
                current_features,
                cache: cache_observations,
            });
        }
        contacts.sort_by_key(|value| value.pair);

        let dynamic = bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0)
            .collect::<Vec<_>>();
        let mut angular_edge_speed_max = 0.0_f32;
        let mut angular_edge_speed_squared = 0.0_f32;
        for body in &dynamic {
            let speed = body.angular_velocity.get().abs() * shape_support_radius(body);
            angular_edge_speed_max = angular_edge_speed_max.max(speed);
            angular_edge_speed_squared += speed * speed;
        }
        let warm_cache_hits = contacts
            .iter()
            .flat_map(|value| &value.cache)
            .filter(|value| value.status == WarmCacheStatusV2::Hit)
            .count() as u32;
        let warm_cache_misses = contacts
            .iter()
            .map(|value| value.cache.len() as u32)
            .sum::<u32>()
            - warm_cache_hits;
        let observation = SettlingObservationV2 {
            observation_version: AWAKE_SETTLING_TRACE_VERSION,
            base,
            angular_edge_speed_max: finite(angular_edge_speed_max),
            angular_edge_speed_rms: finite(
                (angular_edge_speed_squared / dynamic.len() as f32).sqrt(),
            ),
            stable_support_pairs: contacts.iter().filter(|value| value.stable_support).count()
                as u32,
            reference_owner_changes: contacts
                .iter()
                .filter(|value| value.reference_owner_changed)
                .count() as u32,
            point_count_changes: contacts
                .iter()
                .filter(|value| value.point_count_changed)
                .count() as u32,
            feature_set_changes: contacts
                .iter()
                .filter(|value| value.feature_set_changed)
                .count() as u32,
            contact_normal_changes: contacts.iter().filter(|value| value.normal_changed).count()
                as u32,
            warm_cache_hits,
            warm_cache_misses,
            unexplained_stable_churn: contacts
                .iter()
                .filter(|value| value.unexplained_stable_churn)
                .count() as u32,
            contacts,
        };
        observation.validate().map_err(|error| error.to_string())?;
        self.seen_pairs
            .extend(manifolds.iter().map(|value| value.pair));
        self.previous_completed_steps = completed_steps;
        self.previous_manifolds = Some(manifolds.to_vec());
        self.previous_cache = Some(cache.to_vec());
        Ok(observation)
    }
}

pub fn summarize_settling_v2(
    observations: &[SettlingObservationV2],
    policy: SettlingMeasurementPolicyV2,
) -> Result<SettlingSummaryV2, String> {
    policy.validate().map_err(|error| error.to_string())?;
    let settled = observations
        .iter()
        .filter(|value| {
            (policy.settled_start_step..=policy.settled_end_step)
                .contains(&value.base.completed_steps)
        })
        .collect::<Vec<_>>();
    if settled.len() < 4
        || settled.first().unwrap().base.completed_steps != policy.settled_start_step
        || settled.last().unwrap().base.completed_steps != policy.settled_end_step
    {
        return Err("settling v2 summary does not cover the frozen settled window".to_owned());
    }
    let mut consecutive_rest = 0_u32;
    let mut time_to_rest_step = None;
    for observation in observations
        .iter()
        .filter(|value| value.base.completed_steps > policy.transient_end_step)
    {
        if observation.base.residual_linear_speed_max.get() <= policy.rest_linear_speed_limit.get()
            && observation.angular_edge_speed_max.get()
                <= policy.rest_angular_edge_speed_limit.get()
        {
            consecutive_rest += 1;
            if consecutive_rest >= policy.rest_required_consecutive_observations
                && time_to_rest_step.is_none()
            {
                time_to_rest_step = Some(observation.base.completed_steps.saturating_sub(
                    (policy.rest_required_consecutive_observations - 1)
                        * policy.observation_interval_steps,
                ));
            }
        } else {
            consecutive_rest = 0;
        }
    }
    let quartile = settled.len() / 4;
    let first_energy_quartile_max = settled[..quartile]
        .iter()
        .map(|value| value.base.kinetic_energy.get())
        .fold(0.0_f32, f32::max);
    let last_energy_quartile_max = settled[settled.len() - quartile..]
        .iter()
        .map(|value| value.base.kinetic_energy.get())
        .fold(0.0_f32, f32::max);
    let mut positive_energy_run = 0_u32;
    let mut positive_energy_run_max = 0_u32;
    for pair in settled.windows(2) {
        let previous = pair[0].base.kinetic_energy.get();
        let current = pair[1].base.kinetic_energy.get();
        let epsilon = policy.energy_absolute_epsilon.get()
            + policy.energy_relative_epsilon.get() * previous.abs();
        if current > previous + epsilon {
            positive_energy_run += 1;
            positive_energy_run_max = positive_energy_run_max.max(positive_energy_run);
        } else {
            positive_energy_run = 0;
        }
    }
    let energy_envelope_nonincreasing = last_energy_quartile_max
        <= first_energy_quartile_max
            + policy.energy_absolute_epsilon.get()
            + policy.energy_relative_epsilon.get() * first_energy_quartile_max.abs()
        && positive_energy_run_max < policy.positive_energy_run_limit;
    let summary = SettlingSummaryV2 {
        version: AWAKE_SETTLING_TRACE_VERSION,
        time_to_rest_step,
        settled_linear_speed_max: finite(
            settled
                .iter()
                .map(|value| value.base.residual_linear_speed_max.get())
                .fold(0.0_f32, f32::max),
        ),
        settled_angular_edge_speed_max: finite(
            settled
                .iter()
                .map(|value| value.angular_edge_speed_max.get())
                .fold(0.0_f32, f32::max),
        ),
        stable_support_unexplained_churn: settled
            .iter()
            .map(|value| u64::from(value.unexplained_stable_churn))
            .sum(),
        warm_cache_misses: settled
            .iter()
            .map(|value| u64::from(value.warm_cache_misses))
            .sum(),
        first_energy_quartile_max: finite(first_energy_quartile_max),
        last_energy_quartile_max: finite(last_energy_quartile_max),
        positive_energy_run_max,
        energy_envelope_nonincreasing,
    };
    summary.validate().map_err(|error| error.to_string())?;
    Ok(summary)
}

pub fn settling_series_v2(
    source: &ScenarioV1,
    projection_hash: Option<String>,
    lane: impl Into<String>,
    policy: SettlingMeasurementPolicyV2,
    observations: Vec<SettlingObservationV2>,
    structural_pass: bool,
    structural_reason: impl Into<String>,
) -> Result<SettlingSeriesV2, String> {
    let summary = summarize_settling_v2(&observations, policy)?;
    let series = SettlingSeriesV2 {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        source_scenario_hash: canonical_hash(source).map_err(|error| error.to_string())?,
        projection_hash,
        lane: lane.into(),
        sleeping_enabled: false,
        policy,
        observations,
        summary,
        structural_pass,
        structural_reason: structural_reason.into(),
    };
    series.validate().map_err(|error| error.to_string())?;
    Ok(series)
}

fn cache_map(
    cache: &[PersistentContactV1],
) -> BTreeMap<PersistentContactKeyV1, physbox_contracts::ContactCacheValueV1> {
    cache.iter().map(|value| (value.key, value.value)).collect()
}

fn manifold_features(manifold: &CollisionManifoldV1) -> Vec<PersistentContactKeyV1> {
    manifold
        .points
        .iter()
        .map(|point| point.key(manifold.pair))
        .collect()
}

fn reference_owner(manifold: &CollisionManifoldV1) -> (ContactReferenceOwnerV2, Option<u32>) {
    let faces_a = manifold
        .points
        .iter()
        .filter(|point| point.feature_a.kind == GeometricFeatureKindV1::Face)
        .map(|point| point.feature_a.index)
        .collect::<BTreeSet<_>>();
    let faces_b = manifold
        .points
        .iter()
        .filter(|point| point.feature_b.kind == GeometricFeatureKindV1::Face)
        .map(|point| point.feature_b.index)
        .collect::<BTreeSet<_>>();
    if faces_a.len() == 1 && faces_b.is_empty() {
        (
            ContactReferenceOwnerV2::BodyA,
            faces_a.iter().next().copied(),
        )
    } else if faces_b.len() == 1 && faces_a.is_empty() {
        (
            ContactReferenceOwnerV2::BodyB,
            faces_b.iter().next().copied(),
        )
    } else if faces_a.is_empty() && faces_b.is_empty() {
        (ContactReferenceOwnerV2::None, None)
    } else {
        (ContactReferenceOwnerV2::Ambiguous, None)
    }
}

fn normal_delta(previous: Vec2, current: Vec2) -> f32 {
    (previous.x * current.x + previous.y * current.y)
        .clamp(-1.0, 1.0)
        .acos()
}

fn relative_tangent_speed_max(
    manifold: &CollisionManifoldV1,
    bodies: &BTreeMap<physbox_contracts::BodyId, &BodyStateV1>,
) -> Result<f32, String> {
    let body_a = bodies
        .get(&manifold.pair.body_a)
        .ok_or_else(|| "settling manifold body A is missing".to_owned())?;
    let body_b = bodies
        .get(&manifold.pair.body_b)
        .ok_or_else(|| "settling manifold body B is missing".to_owned())?;
    let tangent = Vec2::new(manifold.normal.y, -manifold.normal.x);
    let mut maximum = 0.0_f32;
    for point in &manifold.points {
        let velocity = |body: &BodyStateV1| {
            let radius = Vec2::new(
                point.world_point.x - body.position.x,
                point.world_point.y - body.position.y,
            );
            Vec2::new(
                body.linear_velocity.x - body.angular_velocity.get() * radius.y,
                body.linear_velocity.y + body.angular_velocity.get() * radius.x,
            )
        };
        let a = velocity(body_a);
        let b = velocity(body_b);
        maximum = maximum.max(((b.x - a.x) * tangent.x + (b.y - a.y) * tangent.y).abs());
    }
    Ok(maximum)
}

fn shape_support_radius(body: &BodyStateV1) -> f32 {
    match body.shape {
        ShapeV1::Circle { radius } => radius.get(),
        ShapeV1::Rectangle { half_extents } => {
            (half_extents.x * half_extents.x + half_extents.y * half_extents.y).sqrt()
        }
    }
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

pub fn settling_fixture_structure(
    kind: P5_2SettlingFixtureKind,
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
    if dynamic.is_empty()
        || dynamic.iter().any(|body| {
            !body.position.x.is_finite()
                || !body.position.y.is_finite()
                || !body.angle.get().is_finite()
                || !body.linear_velocity.x.is_finite()
                || !body.linear_velocity.y.is_finite()
                || !body.angular_velocity.get().is_finite()
                || shape_upper_y(body) <= support_height - 0.02
                || body.position.x.abs() > 2.5
                || body.position.y > 8.0
        })
    {
        return (
            false,
            "a dynamic body became invalid or escaped the fixture".to_owned(),
        );
    }
    if matches!(
        kind,
        P5_2SettlingFixtureKind::BoxOnRestingBoxDrop
            | P5_2SettlingFixtureKind::SequentialRectangles3
            | P5_2SettlingFixtureKind::SequentialRectangles4
            | P5_2SettlingFixtureKind::SequentialRectangles5
    ) && dynamic
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id || pair[0].position.y >= pair[1].position.y)
    {
        return (false, "declared rectangle order changed".to_owned());
    }
    (
        true,
        "all declared bodies remained finite and inside the fixture".to_owned(),
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
    use physbox_contracts::{ContactCacheValueV1, GeometricFeatureKindV1, Validate};
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
            coloring: None,
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

    #[test]
    fn settling_v2_attributes_reference_flip_to_a_warm_cache_miss() {
        let scenario = p5_2_centered_rectangle_stack_scenario(2, 2).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let first_manifolds =
            generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let cache_for = |manifolds: &[CollisionManifoldV1], step| {
            manifolds
                .iter()
                .flat_map(|manifold| {
                    manifold
                        .points
                        .iter()
                        .map(move |point| PersistentContactV1 {
                            key: point.key(manifold.pair),
                            value: ContactCacheValueV1 {
                                normal_impulse: finite(0.25),
                                tangent_impulse: finite(0.0),
                                last_seen_step: step,
                            },
                        })
                })
                .collect::<Vec<_>>()
        };
        let first_cache = cache_for(&first_manifolds, 1);
        let mut policy = awake_settling_measurement_policy_v2();
        policy.stable_support_required_consecutive_observations = 1;
        policy.settled_end_step = 2;
        policy.settled_start_step = 2;
        policy.transient_end_step = 1;
        let mut observer = SettlingObserverV2::new(policy).unwrap();
        let first = observer
            .observe(
                &scenario,
                SettlingObservationInputV2 {
                    completed_steps: 1,
                    bodies: &scenario.bodies,
                    manifolds: &first_manifolds,
                    cache: &first_cache,
                    capacities: &[],
                    support_height: 0.0,
                },
            )
            .unwrap();
        assert_eq!(first.warm_cache_hits, 0);
        assert!(first.warm_cache_misses > 0);

        let mut flipped = first_manifolds.clone();
        for point in &mut flipped[0].points {
            std::mem::swap(&mut point.feature_a, &mut point.feature_b);
        }
        let flipped_pair = flipped[0].pair;
        flipped[0]
            .points
            .sort_by_key(|point| point.key(flipped_pair));
        let flipped_cache = cache_for(&flipped, 2);
        let second = observer
            .observe(
                &scenario,
                SettlingObservationInputV2 {
                    completed_steps: 2,
                    bodies: &scenario.bodies,
                    manifolds: &flipped,
                    cache: &flipped_cache,
                    capacities: &[],
                    support_height: 0.0,
                },
            )
            .unwrap();
        let transition = &second.contacts[0];
        assert!(transition.reference_owner_changed);
        assert!(transition.feature_set_changed);
        assert!(transition.unexplained_stable_churn);
        assert!(transition.cache.iter().all(|value| {
            value.status == WarmCacheStatusV2::ReferenceOwnerChanged
                || value.status == WarmCacheStatusV2::FeatureChanged
        }));
        assert_eq!(transition.reference_owner, ContactReferenceOwnerV2::BodyB);
        assert_eq!(
            transition.current_features[0].feature_b.kind,
            GeometricFeatureKindV1::Face
        );
    }

    #[test]
    fn settling_v2_policy_and_summary_freeze_awake_gate_arithmetic() {
        let policy = awake_settling_measurement_policy_v2();
        policy.validate().unwrap();
        assert_eq!(policy.observation_interval_steps, 1);
        assert_eq!(policy.settled_start_step, 900);
        assert_eq!(policy.settled_end_step, 1_200);
        assert_eq!(policy.rest_linear_speed_limit.get(), 0.025);
        assert_eq!(policy.rest_angular_edge_speed_limit.get(), 0.025);
        assert_eq!(policy.positive_energy_run_limit, 5);

        let scenario = p5_2_centered_rectangle_stack_scenario(2, 1_200).unwrap();
        let mut observations = Vec::with_capacity(1_200);
        for step in 1..=1_200 {
            let mut base = observe_settling(
                &scenario,
                SettlingSnapshotInput {
                    completed_steps: step,
                    bodies: &scenario.bodies,
                    manifolds: &[],
                    cache: &[],
                    capacities: &[],
                    previous_manifolds: Some(&[]),
                    previous_cache: Some(&[]),
                    support_height: 0.0,
                },
            )
            .unwrap();
            base.kinetic_energy = finite((1_201 - step) as f32 * 1.0e-6);
            observations.push(SettlingObservationV2 {
                observation_version: AWAKE_SETTLING_TRACE_VERSION,
                base,
                angular_edge_speed_max: finite(0.0),
                angular_edge_speed_rms: finite(0.0),
                stable_support_pairs: 0,
                reference_owner_changes: 0,
                point_count_changes: 0,
                feature_set_changes: 0,
                contact_normal_changes: 0,
                warm_cache_hits: 0,
                warm_cache_misses: 0,
                unexplained_stable_churn: 0,
                contacts: Vec::new(),
            });
        }
        let series = settling_series_v2(
            &scenario,
            None,
            "unit-awake",
            policy,
            observations,
            true,
            "unit structure remained present",
        )
        .unwrap();
        assert_eq!(series.summary.time_to_rest_step, Some(601));
        assert!(series.summary.energy_envelope_nonincreasing);
        assert_eq!(series.summary.positive_energy_run_max, 0);
        series.validate().unwrap();
        let round_trip: SettlingSeriesV2 =
            serde_json::from_slice(&serde_json::to_vec(&series).unwrap()).unwrap();
        assert_eq!(round_trip, series);

        let mut invalid = series;
        invalid.observations[10].base.completed_steps += 1;
        assert!(invalid.validate().is_err());
    }
}
