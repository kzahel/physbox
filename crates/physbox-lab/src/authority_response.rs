//! Frozen native-authority response evaluation shared by S16 diagnostics.

use std::collections::BTreeSet;

use physbox_contracts::{BodyId, BodyStateV1, ScenarioV1, ShapeV1};
use serde::{Deserialize, Serialize};

use crate::{
    precision_margin::{FullF64BodyV1, FullF64SolveResultV1},
    scalar::{ScalarError, ScalarObservedSolveResultV1},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityBandsV1 {
    pub speed_multiplier: f64,
    pub speed_additive: f64,
    pub height_absolute: f64,
    pub penetration_absolute: f64,
    pub awake_product_ceiling: f64,
    pub positive_energy_run_floor: u32,
    pub energy_growth_rule: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityQualityV1 {
    pub settled_start_step: u32,
    pub settled_end_step: u32,
    pub settled_linear_speed_max: f64,
    pub settled_angular_edge_speed_max: f64,
    pub first_energy_quartile_max: f64,
    pub last_energy_quartile_max: f64,
    pub energy_quartile_growth_ratio: f64,
    pub positive_energy_run_max: u32,
    pub energy_envelope_nonincreasing: bool,
    pub maximum_penetration: f64,
    pub maximum_separation: f64,
    pub final_active_normal_points: u32,
    pub final_total_normal_impulse: f64,
    pub final_dynamic_center_height_sum: f64,
    pub structure_pass: bool,
    pub structure_reason: String,
    pub finite: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityBodyPairV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityFailureVectorV1 {
    pub non_finite: bool,
    pub structure: bool,
    pub authority_response: bool,
    pub penetration_support: bool,
    pub awake_speed: bool,
    pub positive_run: bool,
    pub energy_envelope: bool,
    pub replay: bool,
    pub ordered_failures: Vec<&'static str>,
}

pub fn scalar_authority_quality_v1(
    source: &ScenarioV1,
    solve: &ScalarObservedSolveResultV1,
) -> Result<AuthorityQualityV1, ScalarError> {
    let observations = solve
        .observations
        .iter()
        .map(|observation| QualityObservation {
            completed_steps: observation.completed_steps,
            bodies: observation
                .bodies
                .iter()
                .map(QualityBody::from)
                .collect::<Vec<_>>(),
            maximum_penetration: f64::from(observation.maximum_penetration),
            maximum_separation: f64::from(observation.maximum_separation),
            active_normal_points: observation.active_normal_points,
            total_normal_impulse: observation.total_normal_impulse,
        })
        .collect::<Vec<_>>();
    quality(source, &observations)
}

pub fn full_f64_authority_quality_v1(
    source: &ScenarioV1,
    solve: &FullF64SolveResultV1,
) -> Result<AuthorityQualityV1, ScalarError> {
    let observations = solve
        .observations
        .iter()
        .map(|observation| QualityObservation {
            completed_steps: observation.completed_steps,
            bodies: observation
                .bodies
                .iter()
                .map(QualityBody::from)
                .collect::<Vec<_>>(),
            maximum_penetration: observation.maximum_penetration,
            maximum_separation: observation.maximum_separation,
            active_normal_points: observation.active_normal_points,
            total_normal_impulse: observation.total_normal_impulse,
        })
        .collect::<Vec<_>>();
    quality(source, &observations)
}

pub fn authority_failure_vector_v1(
    candidate: &AuthorityQualityV1,
    candidate_pairs: &[AuthorityBodyPairV1],
    authority: &AuthorityQualityV1,
    authority_pairs: &[AuthorityBodyPairV1],
    bands: &AuthorityBandsV1,
    exact_replay: bool,
) -> AuthorityFailureVectorV1 {
    let non_finite = !candidate.finite;
    let structure = !candidate.structure_pass;
    let authority_response = candidate.settled_linear_speed_max
        > bands.speed_multiplier * authority.settled_linear_speed_max + bands.speed_additive
        || candidate.settled_angular_edge_speed_max
            > bands.speed_multiplier * authority.settled_angular_edge_speed_max
                + bands.speed_additive;
    let height_band = bands.height_absolute.max(authority.maximum_penetration);
    let penetration_support = candidate_pairs != authority_pairs
        || (candidate.final_dynamic_center_height_sum - authority.final_dynamic_center_height_sum)
            .abs()
            > height_band
        || candidate.maximum_penetration
            > authority.maximum_penetration + bands.penetration_absolute;
    let awake_speed = candidate.settled_linear_speed_max > bands.awake_product_ceiling
        || candidate.settled_angular_edge_speed_max > bands.awake_product_ceiling;
    let positive_run = candidate.positive_energy_run_max
        > authority
            .positive_energy_run_max
            .max(bands.positive_energy_run_floor);
    let authority_growth_band = authority.energy_quartile_growth_ratio.max(1.0);
    let energy_envelope = candidate.energy_quartile_growth_ratio
        > authority_growth_band + 1.0e-12 + 1.0e-5 * authority_growth_band;
    let replay = !exact_replay;
    let ordered_failures = [
        (non_finite, "non-finite"),
        (structure, "structure"),
        (authority_response, "authority-response"),
        (penetration_support, "penetration-support"),
        (awake_speed, "awake-speed"),
        (positive_run, "positive-run"),
        (energy_envelope, "energy-envelope"),
        (replay, "replay"),
    ]
    .into_iter()
    .filter_map(|(failed, label)| failed.then_some(label))
    .collect();
    AuthorityFailureVectorV1 {
        non_finite,
        structure,
        authority_response,
        penetration_support,
        awake_speed,
        positive_run,
        energy_envelope,
        replay,
        ordered_failures,
    }
}

pub fn scalar_active_support_pairs_v1(
    solve: &ScalarObservedSolveResultV1,
) -> Vec<AuthorityBodyPairV1> {
    normalize_pairs(
        solve
            .observations
            .last()
            .into_iter()
            .flat_map(|observation| &observation.contact_cache)
            .filter(|point| point.normal_impulse.get() > 0.0)
            .map(|point| AuthorityBodyPairV1 {
                body_a: point.body_a,
                body_b: point.body_b,
            }),
    )
}

pub fn full_f64_active_support_pairs_v1(solve: &FullF64SolveResultV1) -> Vec<AuthorityBodyPairV1> {
    normalize_pairs(
        solve
            .observations
            .last()
            .into_iter()
            .flat_map(|observation| &observation.contact_cache)
            .filter(|point| point.normal_impulse > 0.0)
            .map(|point| AuthorityBodyPairV1 {
                body_a: point.body_a,
                body_b: point.body_b,
            }),
    )
}

#[derive(Clone)]
struct QualityBody {
    id: BodyId,
    shape: ShapeV1,
    position: [f64; 2],
    angle: f64,
    linear_velocity: [f64; 2],
    angular_velocity: f64,
    inverse_mass: f64,
    inverse_inertia: f64,
}

impl From<&BodyStateV1> for QualityBody {
    fn from(body: &BodyStateV1) -> Self {
        Self {
            id: body.id,
            shape: body.shape,
            position: [f64::from(body.position.x), f64::from(body.position.y)],
            angle: f64::from(body.angle.get()),
            linear_velocity: [
                f64::from(body.linear_velocity.x),
                f64::from(body.linear_velocity.y),
            ],
            angular_velocity: f64::from(body.angular_velocity.get()),
            inverse_mass: f64::from(body.inverse_mass.get()),
            inverse_inertia: f64::from(body.inverse_inertia.get()),
        }
    }
}

impl From<&FullF64BodyV1> for QualityBody {
    fn from(body: &FullF64BodyV1) -> Self {
        Self {
            id: body.id,
            shape: body.shape,
            position: body.position,
            angle: body.angle,
            linear_velocity: body.linear_velocity,
            angular_velocity: body.angular_velocity,
            inverse_mass: body.inverse_mass,
            inverse_inertia: body.inverse_inertia,
        }
    }
}

struct QualityObservation {
    completed_steps: u32,
    bodies: Vec<QualityBody>,
    maximum_penetration: f64,
    maximum_separation: f64,
    active_normal_points: u32,
    total_normal_impulse: f64,
}

fn quality(
    source: &ScenarioV1,
    observations: &[QualityObservation],
) -> Result<AuthorityQualityV1, ScalarError> {
    let settled = observations
        .iter()
        .filter(|observation| observation.completed_steps >= 900)
        .collect::<Vec<_>>();
    if settled.is_empty() {
        return Err(ScalarError(
            "authority quality has no settled observations".to_owned(),
        ));
    }
    let energy = settled
        .iter()
        .map(|observation| kinetic_energy(&observation.bodies))
        .collect::<Vec<_>>();
    let quartile = (settled.len() / 4).max(1);
    let first_energy_quartile_max = energy[..quartile].iter().copied().fold(0.0, f64::max);
    let last_energy_quartile_max = energy[energy.len() - quartile..]
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let mut positive_energy_run = 0;
    let mut positive_energy_run_max = 0;
    for pair in energy.windows(2) {
        let epsilon = 1.0e-12 + 1.0e-5 * pair[0].abs();
        if pair[1] > pair[0] + epsilon {
            positive_energy_run += 1;
            positive_energy_run_max = positive_energy_run_max.max(positive_energy_run);
        } else {
            positive_energy_run = 0;
        }
    }
    let energy_envelope_nonincreasing = last_energy_quartile_max
        <= first_energy_quartile_max + 1.0e-12 + 1.0e-5 * first_energy_quartile_max.abs()
        && positive_energy_run_max < 5;
    let energy_quartile_growth_ratio = if first_energy_quartile_max > 0.0 {
        last_energy_quartile_max / first_energy_quartile_max
    } else if last_energy_quartile_max == 0.0 {
        1.0
    } else {
        f64::INFINITY
    };
    let final_observation = observations
        .last()
        .ok_or_else(|| ScalarError("authority quality has no final observation".to_owned()))?;
    let (structure_pass, structure_reason) = centered_structure(source, &final_observation.bodies);
    Ok(AuthorityQualityV1 {
        settled_start_step: 900,
        settled_end_step: source.requested_steps,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| body.linear_velocity[0].hypot(body.linear_velocity[1]))
            .fold(0.0, f64::max),
        settled_angular_edge_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| body.angular_velocity.abs() * support_radius(body))
            .fold(0.0, f64::max),
        first_energy_quartile_max,
        last_energy_quartile_max,
        energy_quartile_growth_ratio,
        positive_energy_run_max,
        energy_envelope_nonincreasing,
        maximum_penetration: settled
            .iter()
            .map(|observation| observation.maximum_penetration)
            .fold(0.0, f64::max),
        maximum_separation: settled
            .iter()
            .map(|observation| observation.maximum_separation)
            .fold(0.0, f64::max),
        final_active_normal_points: final_observation.active_normal_points,
        final_total_normal_impulse: final_observation.total_normal_impulse,
        final_dynamic_center_height_sum: dynamic_bodies(&final_observation.bodies)
            .map(|body| body.position[1])
            .sum(),
        structure_pass,
        structure_reason,
        finite: observations.iter().all(|observation| {
            observation.maximum_penetration.is_finite()
                && observation.maximum_separation.is_finite()
                && observation.total_normal_impulse.is_finite()
                && observation.bodies.iter().all(body_finite)
        }),
    })
}

fn centered_structure(source: &ScenarioV1, bodies: &[QualityBody]) -> (bool, String) {
    if source.bodies.len() != bodies.len() {
        return (false, "body count changed".to_owned());
    }
    let dynamic = bodies
        .iter()
        .filter(|body| body.inverse_mass > 0.0)
        .collect::<Vec<_>>();
    if dynamic.iter().any(|body| {
        !body.position[0].is_finite() || !body.position[1].is_finite() || shape_upper_y(body) <= 0.0
    }) {
        return (
            false,
            "a dynamic body became invalid or escaped below the support".to_owned(),
        );
    }
    if dynamic
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id || pair[0].position[1] >= pair[1].position[1])
    {
        return (false, "rectangle stack order changed".to_owned());
    }
    let speculative_distance = source
        .settings
        .contact_policy
        .map_or(0.0, |policy| f64::from(policy.speculative_distance.get()));
    let mut previous_top = 0.0;
    let mut previous_horizontal: Option<(f64, f64)> = None;
    for body in dynamic {
        let ShapeV1::Rectangle { .. } = body.shape else {
            return (
                false,
                "centered stack contains a non-rectangle body".to_owned(),
            );
        };
        let (extent_x, extent_y) = shape_extents(body);
        if body.angle.cos().abs() < body.angle.sin().abs() {
            return (
                false,
                "a rectangle rotated beyond the upright range".to_owned(),
            );
        }
        let lower = body.position[1] - extent_y;
        if lower > previous_top + speculative_distance {
            return (false, "the vertical support chain separated".to_owned());
        }
        let horizontal = (body.position[0] - extent_x, body.position[0] + extent_x);
        if previous_horizontal.is_some_and(|(left, right)| {
            horizontal.0 > right + speculative_distance
                || left > horizontal.1 + speculative_distance
        }) {
            return (false, "the horizontal support chain separated".to_owned());
        }
        previous_top = body.position[1] + extent_y;
        previous_horizontal = Some(horizontal);
    }
    (
        true,
        "all rectangle generations remained finite, supported, and vertically ordered".to_owned(),
    )
}

fn shape_extents(body: &QualityBody) -> (f64, f64) {
    match body.shape {
        ShapeV1::Circle { radius } => (f64::from(radius.get()), f64::from(radius.get())),
        ShapeV1::Rectangle { half_extents } => {
            let cosine = body.angle.cos().abs();
            let sine = body.angle.sin().abs();
            (
                cosine * f64::from(half_extents.x) + sine * f64::from(half_extents.y),
                sine * f64::from(half_extents.x) + cosine * f64::from(half_extents.y),
            )
        }
    }
}

fn shape_upper_y(body: &QualityBody) -> f64 {
    body.position[1] + shape_extents(body).1
}

fn dynamic_bodies(bodies: &[QualityBody]) -> impl Iterator<Item = &QualityBody> {
    bodies.iter().filter(|body| body.inverse_mass > 0.0)
}

fn kinetic_energy(bodies: &[QualityBody]) -> f64 {
    dynamic_bodies(bodies)
        .map(|body| {
            let linear = 0.5
                * (body.linear_velocity[0] * body.linear_velocity[0]
                    + body.linear_velocity[1] * body.linear_velocity[1])
                / body.inverse_mass;
            let angular = if body.inverse_inertia > 0.0 {
                0.5 * body.angular_velocity * body.angular_velocity / body.inverse_inertia
            } else {
                0.0
            };
            linear + angular
        })
        .sum()
}

fn support_radius(body: &QualityBody) -> f64 {
    match body.shape {
        ShapeV1::Circle { radius } => f64::from(radius.get()),
        ShapeV1::Rectangle { half_extents } => {
            f64::from(half_extents.x).hypot(f64::from(half_extents.y))
        }
    }
}

fn body_finite(body: &QualityBody) -> bool {
    body.position[0].is_finite()
        && body.position[1].is_finite()
        && body.angle.is_finite()
        && body.linear_velocity[0].is_finite()
        && body.linear_velocity[1].is_finite()
        && body.angular_velocity.is_finite()
}

fn normalize_pairs(
    pairs: impl IntoIterator<Item = AuthorityBodyPairV1>,
) -> Vec<AuthorityBodyPairV1> {
    pairs
        .into_iter()
        .map(|pair| {
            if pair.body_a <= pair.body_b {
                pair
            } else {
                AuthorityBodyPairV1 {
                    body_a: pair.body_b,
                    body_b: pair.body_a,
                }
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
