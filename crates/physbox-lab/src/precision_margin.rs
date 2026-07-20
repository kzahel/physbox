//! S15 validation-only precision-limit and perturbation-margin laboratory.
//!
//! The path-only lane evaluates the original S12 authority-separation DAG in
//! binary64 and rounds once at the existing binary32 separation boundary. The
//! full-solve lane below is an independent binary64 implementation of the
//! exact X1-X4/current-X5 recurrence for the governed rectangle stacks. It is
//! evidence code, not a product CPU backend.

use physbox_contracts::{BodyId, FeatureIdV1, ScenarioV1, ShapeV1, Validate};
use serde::{Deserialize, Serialize};

use crate::branch_robust::AuthoritySeparationInputsV1;
use crate::scalar::ScalarError;

pub const PRECISION_MARGIN_VERSION: u32 = 1;
pub const PRECISION_BASE_CANDIDATE_ID: &str = "authority-x1-x4-current-x5-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrecisionPerturbationV1 {
    Nearest,
    UniformLow,
    UniformHigh,
    PointAlternating,
    SubstepAlternating,
    StepAlternating,
    HashedSignV1,
}

impl PrecisionPerturbationV1 {
    pub const ALL: [Self; 7] = [
        Self::Nearest,
        Self::UniformLow,
        Self::UniformHigh,
        Self::PointAlternating,
        Self::SubstepAlternating,
        Self::StepAlternating,
        Self::HashedSignV1,
    ];

    pub const SIGNED: [Self; 6] = [
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

    fn sign(self, step: u32, substep: u32, manifold: u32, point: u32) -> i8 {
        match self {
            Self::Nearest => 0,
            Self::UniformLow => -1,
            Self::UniformHigh => 1,
            Self::PointAlternating => alternating_sign(manifold ^ point),
            Self::SubstepAlternating => alternating_sign(substep),
            Self::StepAlternating => alternating_sign(step),
            Self::HashedSignV1 => {
                let hash = step.wrapping_mul(0x9e37_79b9)
                    ^ substep.wrapping_mul(0x85eb_ca6b)
                    ^ manifold.wrapping_mul(0xc2b2_ae35)
                    ^ point.wrapping_mul(0x27d4_eb2f);
                alternating_sign(hash)
            }
        }
    }
}

const fn alternating_sign(value: u32) -> i8 {
    if value & 1 == 0 { -1 } else { 1 }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecisionPathStatisticsV1 {
    pub evaluations: u64,
    pub unperturbed_selections: u64,
    pub lower_selections: u64,
    pub upper_selections: u64,
    pub positive_separations: u64,
    pub zero_separations: u64,
    pub negative_separations: u64,
    pub non_finite_results: u64,
    pub maximum_amplitude: f64,
    pub maximum_absolute_rounding_error: f64,
    pub maximum_absolute_position_operand: f64,
    pub maximum_absolute_anchor_operand: f64,
    pub maximum_absolute_projection: f64,
    pub maximum_absolute_real_separation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecisionPathEvaluationV1 {
    pub real_separation: f64,
    pub perturbed_real_separation: f64,
    pub selected_separation: f32,
    pub selected_bits: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_precision_path_v1(
    input: AuthoritySeparationInputsV1,
    perturbation: PrecisionPerturbationV1,
    amplitude: f64,
    zero_based_step: u32,
    substep: u32,
    manifold_id: u32,
    point_index: u32,
    statistics: &mut PrecisionPathStatisticsV1,
) -> PrecisionPathEvaluationV1 {
    let px_a = f64::from(input.position_a.x);
    let py_a = f64::from(input.position_a.y);
    let px_b = f64::from(input.position_b.x);
    let py_b = f64::from(input.position_b.y);
    let initial_px_a = f64::from(input.initial_position_a.x);
    let initial_py_a = f64::from(input.initial_position_a.y);
    let initial_px_b = f64::from(input.initial_position_b.x);
    let initial_py_b = f64::from(input.initial_position_b.y);
    let delta_position_a = DVec2::new(px_a - initial_px_a, py_a - initial_py_a);
    let delta_position_b = DVec2::new(px_b - initial_px_b, py_b - initial_py_b);
    let delta_position = delta_position_b - delta_position_a;
    let delta_angle_a = f64::from(input.angle_a) - f64::from(input.initial_angle_a);
    let delta_angle_b = f64::from(input.angle_b) - f64::from(input.initial_angle_b);
    let anchor_a = DVec2::new(f64::from(input.anchor_a.x), f64::from(input.anchor_a.y));
    let anchor_b = DVec2::new(f64::from(input.anchor_b.x), f64::from(input.anchor_b.y));
    let normal = DVec2::new(f64::from(input.normal.x), f64::from(input.normal.y));
    let rotated_anchor_a = rotate(anchor_a, delta_angle_a);
    let rotated_anchor_b = rotate(anchor_b, delta_angle_b);
    let prepared_projection = (anchor_b - anchor_a).dot(normal);
    let base_separation = f64::from(input.input_separation) - prepared_projection;
    let projection = (delta_position + (rotated_anchor_b - rotated_anchor_a)).dot(normal);
    let real_separation = base_separation + projection;
    let sign = perturbation.sign(zero_based_step, substep, manifold_id, point_index);
    let perturbed_real_separation = real_separation + f64::from(sign) * amplitude;
    let selected_separation = perturbed_real_separation as f32;

    statistics.evaluations += 1;
    match sign {
        -1 => statistics.lower_selections += 1,
        1 => statistics.upper_selections += 1,
        _ => statistics.unperturbed_selections += 1,
    }
    if selected_separation > 0.0 {
        statistics.positive_separations += 1;
    } else if selected_separation < 0.0 {
        statistics.negative_separations += 1;
    } else {
        statistics.zero_separations += 1;
    }
    if !real_separation.is_finite()
        || !perturbed_real_separation.is_finite()
        || !selected_separation.is_finite()
        || !amplitude.is_finite()
        || amplitude < 0.0
    {
        statistics.non_finite_results += 1;
    }
    statistics.maximum_amplitude = statistics.maximum_amplitude.max(amplitude);
    statistics.maximum_absolute_rounding_error = statistics
        .maximum_absolute_rounding_error
        .max((f64::from(selected_separation) - perturbed_real_separation).abs());
    statistics.maximum_absolute_position_operand = statistics
        .maximum_absolute_position_operand
        .max(px_a.abs())
        .max(py_a.abs())
        .max(px_b.abs())
        .max(py_b.abs())
        .max(initial_px_a.abs())
        .max(initial_py_a.abs())
        .max(initial_px_b.abs())
        .max(initial_py_b.abs());
    statistics.maximum_absolute_anchor_operand = statistics
        .maximum_absolute_anchor_operand
        .max(anchor_a.max_abs())
        .max(anchor_b.max_abs());
    statistics.maximum_absolute_projection = statistics
        .maximum_absolute_projection
        .max(prepared_projection.abs())
        .max(projection.abs());
    statistics.maximum_absolute_real_separation = statistics
        .maximum_absolute_real_separation
        .max(real_separation.abs());

    PrecisionPathEvaluationV1 {
        real_separation,
        perturbed_real_separation,
        selected_separation,
        selected_bits: selected_separation.to_bits(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullF64StatisticsV1 {
    pub outer_steps: u32,
    pub substeps: u64,
    pub prepared_contacts: u64,
    pub prepared_points: u64,
    pub separation_evaluations: u64,
    pub positive_separations: u64,
    pub zero_separations: u64,
    pub negative_separations: u64,
    pub non_finite_values: u64,
    pub maximum_absolute_position_operand: f64,
    pub maximum_absolute_anchor_operand: f64,
    pub maximum_absolute_projection: f64,
    pub maximum_absolute_separation: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullF64BodyV1 {
    pub id: BodyId,
    pub shape: ShapeV1,
    pub position: [f64; 2],
    pub angle: f64,
    pub linear_velocity: [f64; 2],
    pub angular_velocity: f64,
    pub inverse_mass: f64,
    pub inverse_inertia: f64,
    pub sleeping: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullF64CachePointV1 {
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub feature_a: FeatureIdV1,
    pub feature_b: FeatureIdV1,
    pub normal_impulse: f64,
    pub tangent_impulse: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullF64ObservationV1 {
    pub completed_steps: u32,
    pub bodies: Vec<FullF64BodyV1>,
    pub contact_cache: Vec<FullF64CachePointV1>,
    pub active_normal_points: u32,
    pub total_normal_impulse: f64,
    pub maximum_penetration: f64,
    pub maximum_separation: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullF64SolveResultV1 {
    pub version: u32,
    pub base_candidate_id: &'static str,
    pub arithmetic_scope: &'static str,
    pub statistics: FullF64StatisticsV1,
    pub observations: Vec<FullF64ObservationV1>,
    pub final_bodies: Vec<FullF64BodyV1>,
    pub final_cache: Vec<FullF64CachePointV1>,
}

pub fn solve_full_f64_authority_v1(
    scenario: &ScenarioV1,
) -> Result<FullF64SolveResultV1, ScalarError> {
    scenario
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    if !scenario.manifolds.is_empty()
        || !scenario.contact_cache.is_empty()
        || !scenario.actions.is_empty()
    {
        return Err(ScalarError(
            "S15 full-f64 lane requires a neutral generated-contact source".to_owned(),
        ));
    }
    if scenario.settings.substeps == 0 {
        return Err(ScalarError(
            "S15 full-f64 lane requires substeps".to_owned(),
        ));
    }
    if scenario
        .bodies
        .iter()
        .any(|body| !matches!(body.shape, ShapeV1::Rectangle { .. }))
    {
        return Err(ScalarError(
            "S15 full-f64 governed lane supports rectangle sources only".to_owned(),
        ));
    }

    let settings = DSettings::from_scenario(scenario);
    let mut bodies = scenario.bodies.iter().map(DBody::from).collect::<Vec<_>>();
    let mut cache = Vec::<DCachePoint>::new();
    let mut observations = Vec::with_capacity(scenario.requested_steps as usize);
    let mut statistics = FullF64StatisticsV1::default();

    for step in 0..scenario.requested_steps {
        let mut contacts = prepare_contacts(&bodies, &cache, &settings, &mut statistics)?;
        order_authority_points(&mut contacts);
        let visit_order = authority_visit_order(&contacts, &bodies);
        for _substep in 0..scenario.settings.substeps {
            statistics.substeps += 1;
            integrate_velocities(&mut bodies, settings.gravity, settings.substep_dt)?;
            for &contact_index in &visit_order {
                warm_start(&contacts[contact_index], &mut bodies)?;
            }
            for &contact_index in &visit_order {
                solve_normal(&mut contacts[contact_index], &mut bodies, &settings, true)?;
                solve_tangent(&mut contacts[contact_index], &mut bodies)?;
            }
            integrate_positions(&mut bodies, settings.substep_dt)?;
            refresh_separations(&mut contacts, &bodies, &mut statistics);
            for &contact_index in &visit_order {
                solve_normal(&mut contacts[contact_index], &mut bodies, &settings, false)?;
                solve_tangent(&mut contacts[contact_index], &mut bodies)?;
                apply_restitution(
                    &mut contacts[contact_index],
                    &mut bodies,
                    settings.restitution_threshold,
                )?;
            }
            cache = collect_cache(&contacts, &bodies);
        }
        statistics.outer_steps = step + 1;
        observations.push(observe(step + 1, &bodies, &cache, &settings)?);
    }

    statistics.non_finite_values = count_non_finite(&bodies, &cache, &observations);
    if statistics.non_finite_values != 0 {
        return Err(ScalarError(
            "S15 full-f64 recurrence produced non-finite state".to_owned(),
        ));
    }
    Ok(FullF64SolveResultV1 {
        version: PRECISION_MARGIN_VERSION,
        base_candidate_id: PRECISION_BASE_CANDIDATE_ID,
        arithmetic_scope: "complete-independent-scalar-recurrence-binary64",
        statistics,
        observations,
        final_bodies: snapshot_bodies(&bodies),
        final_cache: snapshot_cache(&cache),
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DVec2 {
    x: f64,
    y: f64,
}

impl DVec2 {
    const ZERO: Self = Self { x: 0.0, y: 0.0 };

    const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    fn scale(self, scalar: f64) -> Self {
        Self::new(self.x * scalar, self.y * scalar)
    }

    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> Self {
        self.scale(self.length().recip())
    }

    fn max_abs(self) -> f64 {
        self.x.abs().max(self.y.abs())
    }
}

impl std::ops::Add for DVec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for DVec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

fn rotate(vector: DVec2, angle: f64) -> DVec2 {
    let (sine, cosine) = angle.sin_cos();
    DVec2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}

fn inverse_rotate(vector: DVec2, angle: f64) -> DVec2 {
    rotate(vector, -angle)
}

fn right_perpendicular(vector: DVec2) -> DVec2 {
    DVec2::new(vector.y, -vector.x)
}

fn cross_scalar_vector(scalar: f64, vector: DVec2) -> DVec2 {
    DVec2::new(-scalar * vector.y, scalar * vector.x)
}

#[derive(Clone)]
struct DBody {
    id: BodyId,
    shape: ShapeV1,
    position: DVec2,
    angle: f64,
    linear_velocity: DVec2,
    angular_velocity: f64,
    inverse_mass: f64,
    inverse_inertia: f64,
    friction: f64,
    restitution: f64,
    sleeping: bool,
}

impl From<&physbox_contracts::BodyStateV1> for DBody {
    fn from(body: &physbox_contracts::BodyStateV1) -> Self {
        Self {
            id: body.id,
            shape: body.shape,
            position: DVec2::new(f64::from(body.position.x), f64::from(body.position.y)),
            angle: f64::from(body.angle.get()),
            linear_velocity: DVec2::new(
                f64::from(body.linear_velocity.x),
                f64::from(body.linear_velocity.y),
            ),
            angular_velocity: f64::from(body.angular_velocity.get()),
            inverse_mass: f64::from(body.inverse_mass.get()),
            inverse_inertia: f64::from(body.inverse_inertia.get()),
            friction: f64::from(body.material.friction.get()),
            restitution: f64::from(body.material.restitution.get()),
            sleeping: body.sleeping,
        }
    }
}

#[derive(Clone, Copy)]
struct DSettings {
    gravity: DVec2,
    substep_dt: f64,
    contact_hertz: f64,
    damping_ratio: f64,
    speculative_distance: f64,
    max_depenetration_velocity: f64,
    restitution_threshold: f64,
}

impl DSettings {
    fn from_scenario(scenario: &ScenarioV1) -> Self {
        let softness = scenario
            .settings
            .contact_softness
            .expect("S15 governed exact recurrence requires contact softness");
        Self {
            gravity: DVec2::new(
                f64::from(scenario.settings.gravity.x),
                f64::from(scenario.settings.gravity.y),
            ),
            substep_dt: f64::from(scenario.settings.fixed_dt.get())
                / f64::from(scenario.settings.substeps),
            contact_hertz: f64::from(softness.contact_hertz.get()),
            damping_ratio: f64::from(softness.damping_ratio.get()),
            speculative_distance: scenario
                .settings
                .contact_policy
                .map_or(0.0, |policy| f64::from(policy.speculative_distance.get())),
            max_depenetration_velocity: scenario.settings.contact_policy.map_or(0.0, |policy| {
                f64::from(policy.max_depenetration_velocity.get())
            }),
            restitution_threshold: f64::from(scenario.settings.restitution_threshold.get()),
        }
    }
}

#[derive(Clone)]
struct DPoint {
    feature_a: FeatureIdV1,
    feature_b: FeatureIdV1,
    local_anchor_a: DVec2,
    local_anchor_b: DVec2,
    anchor_a: DVec2,
    anchor_b: DVec2,
    input_separation: f64,
    separation: f64,
    normal_mass: f64,
    tangent_mass: f64,
    initial_normal_velocity: f64,
    normal_impulse: f64,
    tangent_impulse: f64,
    total_normal_impulse: f64,
}

#[derive(Clone)]
struct DContact {
    body_a: usize,
    body_b: usize,
    initial_position_a: DVec2,
    initial_position_b: DVec2,
    initial_angle_a: f64,
    initial_angle_b: f64,
    normal: DVec2,
    tangent: DVec2,
    friction: f64,
    restitution: f64,
    points: Vec<DPoint>,
}

#[derive(Clone, PartialEq)]
struct DCachePoint {
    body_a: BodyId,
    body_b: BodyId,
    feature_a: FeatureIdV1,
    feature_b: FeatureIdV1,
    normal_impulse: f64,
    tangent_impulse: f64,
}

#[derive(Clone)]
struct DManifoldPoint {
    local_anchor_a: DVec2,
    local_anchor_b: DVec2,
    separation: f64,
    feature_a: FeatureIdV1,
    feature_b: FeatureIdV1,
}

struct DManifold {
    body_a: usize,
    body_b: usize,
    normal: DVec2,
    friction: f64,
    restitution: f64,
    points: Vec<DManifoldPoint>,
}

fn prepare_contacts(
    bodies: &[DBody],
    cache: &[DCachePoint],
    settings: &DSettings,
    statistics: &mut FullF64StatisticsV1,
) -> Result<Vec<DContact>, ScalarError> {
    let manifolds = generate_manifolds(bodies, settings.speculative_distance)?;
    let mut contacts = Vec::with_capacity(manifolds.len());
    for manifold in manifolds {
        let body_a = &bodies[manifold.body_a];
        let body_b = &bodies[manifold.body_b];
        let tangent = right_perpendicular(manifold.normal);
        let mut points = Vec::with_capacity(manifold.points.len());
        for point in manifold.points {
            let anchor_a = rotate(point.local_anchor_a, body_a.angle);
            let anchor_b = rotate(point.local_anchor_b, body_b.angle);
            let normal_mass = effective_mass(body_a, body_b, anchor_a, anchor_b, manifold.normal);
            let tangent_mass = effective_mass(body_a, body_b, anchor_a, anchor_b, tangent);
            let initial_normal_velocity = relative_velocity(
                bodies,
                manifold.body_a,
                manifold.body_b,
                anchor_a,
                anchor_b,
                manifold.normal,
            );
            let cached = cache.iter().find(|cached| {
                cached.body_a == body_a.id
                    && cached.body_b == body_b.id
                    && cached.feature_a == point.feature_a
                    && cached.feature_b == point.feature_b
            });
            points.push(DPoint {
                feature_a: point.feature_a,
                feature_b: point.feature_b,
                local_anchor_a: point.local_anchor_a,
                local_anchor_b: point.local_anchor_b,
                anchor_a,
                anchor_b,
                input_separation: point.separation,
                separation: point.separation,
                normal_mass,
                tangent_mass,
                initial_normal_velocity,
                normal_impulse: cached.map_or(0.0, |point| point.normal_impulse),
                tangent_impulse: cached.map_or(0.0, |point| point.tangent_impulse),
                total_normal_impulse: 0.0,
            });
        }
        canonicalize_features(&mut points, manifold.normal, tangent);
        statistics.prepared_contacts += 1;
        statistics.prepared_points += points.len() as u64;
        contacts.push(DContact {
            body_a: manifold.body_a,
            body_b: manifold.body_b,
            initial_position_a: body_a.position,
            initial_position_b: body_b.position,
            initial_angle_a: body_a.angle,
            initial_angle_b: body_b.angle,
            normal: manifold.normal,
            tangent,
            friction: manifold.friction,
            restitution: manifold.restitution,
            points,
        });
    }
    Ok(contacts)
}

fn canonicalize_features(points: &mut [DPoint], _normal: DVec2, tangent: DVec2) {
    let reference_a =
        points.len() == 2 && points.iter().all(|point| point.feature_a.feature_type == 1);
    let reference_b =
        points.len() == 2 && points.iter().all(|point| point.feature_b.feature_type == 1);
    let mut order = (0..points.len()).collect::<Vec<_>>();
    order.sort_by(|&left, &right| {
        let left_coordinate = (points[left].anchor_a + points[left].anchor_b).dot(tangent);
        let right_coordinate = (points[right].anchor_a + points[right].anchor_b).dot(tangent);
        right_coordinate.total_cmp(&left_coordinate)
    });
    for (rank, &index) in order.iter().enumerate() {
        let point = &mut points[index];
        if reference_a {
            let face = point.feature_a.feature_index;
            point.feature_a = authority_vertex(if rank == 0 { face } else { (face + 1) % 4 });
            point.feature_b = authority_vertex(point.feature_b.feature_index);
        } else if reference_b {
            let face = point.feature_b.feature_index;
            point.feature_a = authority_vertex(point.feature_a.feature_index);
            point.feature_b = authority_vertex(if rank == 0 { (face + 1) % 4 } else { face });
        } else {
            point.feature_a = nearest_vertex_feature(point.local_anchor_a);
            point.feature_b = nearest_vertex_feature(point.local_anchor_b);
        }
    }
}

const fn authority_vertex(index: u32) -> FeatureIdV1 {
    FeatureIdV1 {
        feature_type: 3,
        feature_index: index,
    }
}

fn nearest_vertex_feature(anchor: DVec2) -> FeatureIdV1 {
    let sign_x = anchor.x >= 0.0;
    let sign_y = anchor.y >= 0.0;
    authority_vertex(match (sign_x, sign_y) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    })
}

fn order_authority_points(contacts: &mut [DContact]) {
    for contact in contacts {
        contact.points.sort_by(|left, right| {
            let left_coordinate = (left.anchor_a + left.anchor_b).dot(contact.tangent);
            let right_coordinate = (right.anchor_a + right.anchor_b).dot(contact.tangent);
            right_coordinate.total_cmp(&left_coordinate)
        });
    }
}

fn authority_visit_order(contacts: &[DContact], bodies: &[DBody]) -> Vec<usize> {
    const DYNAMIC_COLORS: usize = 20;
    const NON_OVERFLOW_COLORS: usize = 23;
    let mut colors = vec![Vec::new(); NON_OVERFLOW_COLORS];
    let mut occupied = vec![vec![false; bodies.len()]; NON_OVERFLOW_COLORS];
    let mut overflow = Vec::new();
    for (contact_index, contact) in contacts.iter().enumerate() {
        let dynamic_a = (bodies[contact.body_a].inverse_mass != 0.0).then_some(contact.body_a);
        let dynamic_b = (bodies[contact.body_b].inverse_mass != 0.0).then_some(contact.body_b);
        let dynamic_bodies = [dynamic_a, dynamic_b];
        let selected = if dynamic_a.is_some() && dynamic_b.is_some() {
            (0..DYNAMIC_COLORS).find(|&color| {
                dynamic_bodies
                    .iter()
                    .flatten()
                    .all(|&body| !occupied[color][body])
            })
        } else {
            (1..NON_OVERFLOW_COLORS).rev().find(|&color| {
                dynamic_bodies
                    .iter()
                    .flatten()
                    .all(|&body| !occupied[color][body])
            })
        };
        if let Some(color) = selected {
            for &body in dynamic_bodies.iter().flatten() {
                occupied[color][body] = true;
            }
            colors[color].push(contact_index);
        } else {
            overflow.push(contact_index);
        }
    }
    overflow
        .into_iter()
        .chain(colors.into_iter().flatten())
        .collect()
}

fn integrate_velocities(
    bodies: &mut [DBody],
    gravity: DVec2,
    substep_dt: f64,
) -> Result<(), ScalarError> {
    for body in bodies {
        if body.inverse_mass != 0.0 && !body.sleeping {
            body.linear_velocity = body.linear_velocity + gravity.scale(substep_dt);
            ensure_vec(body.linear_velocity, "full-f64 gravity velocity")?;
        }
    }
    Ok(())
}

fn integrate_positions(bodies: &mut [DBody], substep_dt: f64) -> Result<(), ScalarError> {
    for body in bodies {
        if body.inverse_mass == 0.0 || body.sleeping {
            continue;
        }
        body.position = body.position + body.linear_velocity.scale(substep_dt);
        let delta_angle = substep_dt * body.angular_velocity;
        let (sine, cosine) = body.angle.sin_cos();
        let integrated_cosine = cosine - delta_angle * sine;
        let integrated_sine = sine + delta_angle * cosine;
        let magnitude =
            (integrated_cosine * integrated_cosine + integrated_sine * integrated_sine).sqrt();
        body.angle = (integrated_sine / magnitude).atan2(integrated_cosine / magnitude);
        ensure_vec(body.position, "full-f64 integrated position")?;
        ensure_finite(body.angle, "full-f64 integrated angle")?;
    }
    Ok(())
}

fn warm_start(contact: &DContact, bodies: &mut [DBody]) -> Result<(), ScalarError> {
    for point in &contact.points {
        let impulse = contact.normal.scale(point.normal_impulse)
            + contact.tangent.scale(point.tangent_impulse);
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            impulse,
        )?;
    }
    Ok(())
}

fn solve_normal(
    contact: &mut DContact,
    bodies: &mut [DBody],
    settings: &DSettings,
    use_bias: bool,
) -> Result<(), ScalarError> {
    let static_contact =
        bodies[contact.body_a].inverse_mass == 0.0 || bodies[contact.body_b].inverse_mass == 0.0;
    let capped_hertz = settings.contact_hertz.min(0.125 / settings.substep_dt);
    let hertz = if static_contact {
        capped_hertz * 2.0
    } else {
        capped_hertz
    };
    let omega = 2.0 * std::f64::consts::PI * hertz;
    let a1 = 2.0 * settings.damping_ratio + settings.substep_dt * omega;
    let a2 = settings.substep_dt * omega * a1;
    let impulse_scale_soft = 1.0 / (1.0 + a2);
    let mass_scale_soft = a2 * impulse_scale_soft;
    let bias_rate_soft = omega / a1;
    for point in &mut contact.points {
        let normal_velocity = relative_velocity(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal,
        );
        let old_impulse = point.normal_impulse;
        let (velocity_bias, mass_scale, impulse_scale) = if point.separation > 0.0 {
            (point.separation / settings.substep_dt, 1.0, 0.0)
        } else if use_bias {
            let unclamped = mass_scale_soft * bias_rate_soft * point.separation;
            let velocity_bias = if settings.max_depenetration_velocity > 0.0 {
                unclamped.max(-settings.max_depenetration_velocity)
            } else {
                unclamped
            };
            (velocity_bias, mass_scale_soft, impulse_scale_soft)
        } else {
            (0.0, 1.0, 0.0)
        };
        let increment = -point.normal_mass * (mass_scale * normal_velocity + velocity_bias)
            - impulse_scale * old_impulse;
        point.normal_impulse = (old_impulse + increment).max(0.0);
        let applied = point.normal_impulse - old_impulse;
        point.total_normal_impulse += applied.max(0.0);
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal.scale(applied),
        )?;
    }
    Ok(())
}

fn solve_tangent(contact: &mut DContact, bodies: &mut [DBody]) -> Result<(), ScalarError> {
    for point in &mut contact.points {
        let tangent_velocity = relative_velocity(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.tangent,
        );
        let old_impulse = point.tangent_impulse;
        let maximum = contact.friction * point.normal_impulse;
        point.tangent_impulse =
            (old_impulse - point.tangent_mass * tangent_velocity).clamp(-maximum, maximum);
        let applied = point.tangent_impulse - old_impulse;
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.tangent.scale(applied),
        )?;
    }
    Ok(())
}

fn apply_restitution(
    contact: &mut DContact,
    bodies: &mut [DBody],
    threshold: f64,
) -> Result<(), ScalarError> {
    for point in &mut contact.points {
        let normal_velocity = relative_velocity(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal,
        );
        let old_impulse = point.normal_impulse;
        if contact.restitution > 0.0
            && point.initial_normal_velocity < -threshold
            && point.total_normal_impulse > 0.0
        {
            let candidate = -point.normal_mass
                * (normal_velocity + contact.restitution * point.initial_normal_velocity);
            point.normal_impulse = (old_impulse + candidate).max(0.0);
        }
        let applied = point.normal_impulse - old_impulse;
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal.scale(applied),
        )?;
    }
    Ok(())
}

fn refresh_separations(
    contacts: &mut [DContact],
    bodies: &[DBody],
    statistics: &mut FullF64StatisticsV1,
) {
    for contact in contacts {
        let delta_position_a = bodies[contact.body_a].position - contact.initial_position_a;
        let delta_position_b = bodies[contact.body_b].position - contact.initial_position_b;
        let delta_position = delta_position_b - delta_position_a;
        let delta_angle_a = bodies[contact.body_a].angle - contact.initial_angle_a;
        let delta_angle_b = bodies[contact.body_b].angle - contact.initial_angle_b;
        for point in &mut contact.points {
            let rotated_anchor_a = rotate(point.anchor_a, delta_angle_a);
            let rotated_anchor_b = rotate(point.anchor_b, delta_angle_b);
            let prepared_projection = (point.anchor_b - point.anchor_a).dot(contact.normal);
            let base_separation = point.input_separation - prepared_projection;
            let projection =
                (delta_position + (rotated_anchor_b - rotated_anchor_a)).dot(contact.normal);
            point.separation = base_separation + projection;
            statistics.separation_evaluations += 1;
            if point.separation > 0.0 {
                statistics.positive_separations += 1;
            } else if point.separation < 0.0 {
                statistics.negative_separations += 1;
            } else {
                statistics.zero_separations += 1;
            }
            statistics.maximum_absolute_position_operand = statistics
                .maximum_absolute_position_operand
                .max(bodies[contact.body_a].position.max_abs())
                .max(bodies[contact.body_b].position.max_abs())
                .max(contact.initial_position_a.max_abs())
                .max(contact.initial_position_b.max_abs());
            statistics.maximum_absolute_anchor_operand = statistics
                .maximum_absolute_anchor_operand
                .max(point.anchor_a.max_abs())
                .max(point.anchor_b.max_abs());
            statistics.maximum_absolute_projection = statistics
                .maximum_absolute_projection
                .max(prepared_projection.abs())
                .max(projection.abs());
            statistics.maximum_absolute_separation = statistics
                .maximum_absolute_separation
                .max(point.separation.abs());
        }
    }
}

fn apply_pair_impulse(
    bodies: &mut [DBody],
    index_a: usize,
    index_b: usize,
    anchor_a: DVec2,
    anchor_b: DVec2,
    impulse: DVec2,
) -> Result<(), ScalarError> {
    let (body_a, body_b) = body_pair_mut(bodies, index_a, index_b);
    body_a.linear_velocity = body_a.linear_velocity - impulse.scale(body_a.inverse_mass);
    body_a.angular_velocity -= body_a.inverse_inertia * anchor_a.cross(impulse);
    body_b.linear_velocity = body_b.linear_velocity + impulse.scale(body_b.inverse_mass);
    body_b.angular_velocity += body_b.inverse_inertia * anchor_b.cross(impulse);
    ensure_vec(body_a.linear_velocity, "full-f64 impulse velocity A")?;
    ensure_finite(body_a.angular_velocity, "full-f64 angular velocity A")?;
    ensure_vec(body_b.linear_velocity, "full-f64 impulse velocity B")?;
    ensure_finite(body_b.angular_velocity, "full-f64 angular velocity B")
}

fn body_pair_mut(bodies: &mut [DBody], index_a: usize, index_b: usize) -> (&mut DBody, &mut DBody) {
    if index_a < index_b {
        let (left, right) = bodies.split_at_mut(index_b);
        (&mut left[index_a], &mut right[0])
    } else {
        let (left, right) = bodies.split_at_mut(index_a);
        (&mut right[0], &mut left[index_b])
    }
}

fn point_velocity(body: &DBody, anchor: DVec2) -> DVec2 {
    body.linear_velocity + cross_scalar_vector(body.angular_velocity, anchor)
}

fn relative_velocity(
    bodies: &[DBody],
    body_a: usize,
    body_b: usize,
    anchor_a: DVec2,
    anchor_b: DVec2,
    axis: DVec2,
) -> f64 {
    (point_velocity(&bodies[body_b], anchor_b) - point_velocity(&bodies[body_a], anchor_a))
        .dot(axis)
}

fn effective_mass(
    body_a: &DBody,
    body_b: &DBody,
    anchor_a: DVec2,
    anchor_b: DVec2,
    axis: DVec2,
) -> f64 {
    let cross_a = anchor_a.cross(axis);
    let cross_b = anchor_b.cross(axis);
    let inverse = body_a.inverse_mass
        + body_b.inverse_mass
        + body_a.inverse_inertia * cross_a * cross_a
        + body_b.inverse_inertia * cross_b * cross_b;
    if inverse > 0.0 { inverse.recip() } else { 0.0 }
}

fn collect_cache(contacts: &[DContact], bodies: &[DBody]) -> Vec<DCachePoint> {
    contacts
        .iter()
        .flat_map(|contact| {
            contact.points.iter().map(move |point| DCachePoint {
                body_a: bodies[contact.body_a].id,
                body_b: bodies[contact.body_b].id,
                feature_a: point.feature_a,
                feature_b: point.feature_b,
                normal_impulse: point.normal_impulse,
                tangent_impulse: point.tangent_impulse,
            })
        })
        .collect()
}

fn observe(
    completed_steps: u32,
    bodies: &[DBody],
    cache: &[DCachePoint],
    settings: &DSettings,
) -> Result<FullF64ObservationV1, ScalarError> {
    let manifolds = generate_manifolds(bodies, settings.speculative_distance)?;
    let mut maximum_penetration = 0.0_f64;
    let mut maximum_separation = 0.0_f64;
    for point in manifolds.iter().flat_map(|manifold| manifold.points.iter()) {
        maximum_penetration = maximum_penetration.max((-point.separation).max(0.0));
        maximum_separation = maximum_separation.max(point.separation.max(0.0));
    }
    Ok(FullF64ObservationV1 {
        completed_steps,
        bodies: snapshot_bodies(bodies),
        contact_cache: snapshot_cache(cache),
        active_normal_points: cache
            .iter()
            .filter(|point| point.normal_impulse > 0.0)
            .count() as u32,
        total_normal_impulse: cache.iter().map(|point| point.normal_impulse).sum(),
        maximum_penetration,
        maximum_separation,
    })
}

fn snapshot_bodies(bodies: &[DBody]) -> Vec<FullF64BodyV1> {
    bodies
        .iter()
        .map(|body| FullF64BodyV1 {
            id: body.id,
            shape: body.shape,
            position: [body.position.x, body.position.y],
            angle: body.angle,
            linear_velocity: [body.linear_velocity.x, body.linear_velocity.y],
            angular_velocity: body.angular_velocity,
            inverse_mass: body.inverse_mass,
            inverse_inertia: body.inverse_inertia,
            sleeping: body.sleeping,
        })
        .collect()
}

fn snapshot_cache(cache: &[DCachePoint]) -> Vec<FullF64CachePointV1> {
    cache
        .iter()
        .map(|point| FullF64CachePointV1 {
            body_a: point.body_a,
            body_b: point.body_b,
            feature_a: point.feature_a,
            feature_b: point.feature_b,
            normal_impulse: point.normal_impulse,
            tangent_impulse: point.tangent_impulse,
        })
        .collect()
}

fn count_non_finite(
    bodies: &[DBody],
    cache: &[DCachePoint],
    observations: &[FullF64ObservationV1],
) -> u64 {
    let body_count = bodies
        .iter()
        .filter(|body| {
            !body.position.x.is_finite()
                || !body.position.y.is_finite()
                || !body.angle.is_finite()
                || !body.linear_velocity.x.is_finite()
                || !body.linear_velocity.y.is_finite()
                || !body.angular_velocity.is_finite()
        })
        .count() as u64;
    let cache_count = cache
        .iter()
        .filter(|point| !point.normal_impulse.is_finite() || !point.tangent_impulse.is_finite())
        .count() as u64;
    let observation_count = observations
        .iter()
        .filter(|observation| {
            !observation.maximum_penetration.is_finite()
                || !observation.maximum_separation.is_finite()
                || !observation.total_normal_impulse.is_finite()
        })
        .count() as u64;
    body_count + cache_count + observation_count
}

fn ensure_finite(value: f64, label: &str) -> Result<(), ScalarError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ScalarError(format!("{label} is non-finite")))
    }
}

fn ensure_vec(value: DVec2, label: &str) -> Result<(), ScalarError> {
    ensure_finite(value.x, label)?;
    ensure_finite(value.y, label)
}

// Binary64 rectangle narrowphase, independently mirroring the selected
// scalar SAT/clipping contract for the two governed S15 sources.

#[derive(Clone, Copy)]
struct DRectangle {
    axis_x: DVec2,
    axis_y: DVec2,
    vertices: [DVec2; 4],
}

impl DRectangle {
    fn new(body: &DBody, half: DVec2) -> Self {
        let axis_x = rotate(DVec2::new(1.0, 0.0), body.angle);
        let axis_y = rotate(DVec2::new(0.0, 1.0), body.angle);
        let local = [
            DVec2::new(-half.x, -half.y),
            DVec2::new(half.x, -half.y),
            DVec2::new(half.x, half.y),
            DVec2::new(-half.x, half.y),
        ];
        Self {
            axis_x,
            axis_y,
            vertices: local.map(|vertex| body.position + rotate(vertex, body.angle)),
        }
    }

    fn face_normal(self, face: u32) -> DVec2 {
        match face {
            0 => self.axis_y.scale(-1.0),
            1 => self.axis_x,
            2 => self.axis_y,
            3 => self.axis_x.scale(-1.0),
            _ => unreachable!(),
        }
    }

    fn face_for_normal(self, normal: DVec2) -> u32 {
        let x = normal.dot(self.axis_x);
        let y = normal.dot(self.axis_y);
        if x.abs() >= y.abs() {
            if x >= 0.0 { 1 } else { 3 }
        } else if y >= 0.0 {
            2
        } else {
            0
        }
    }

    fn incident_face(self, reference_normal: DVec2) -> (u32, u32) {
        match self.face_for_normal(reference_normal.scale(-1.0)) {
            0 => (0, 1),
            1 => (1, 2),
            2 => (2, 3),
            3 => (3, 0),
            _ => unreachable!(),
        }
    }

    fn face_vertices(self, face: u32) -> (DVec2, DVec2) {
        match face {
            0 => (self.vertices[0], self.vertices[1]),
            1 => (self.vertices[1], self.vertices[2]),
            2 => (self.vertices[2], self.vertices[3]),
            3 => (self.vertices[3], self.vertices[0]),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
struct DFaceSeparation {
    face: u32,
    separation: f64,
}

#[derive(Clone, Copy)]
struct DClipPoint {
    position: DVec2,
    vertex: u32,
}

fn generate_manifolds(
    bodies: &[DBody],
    speculative_distance: f64,
) -> Result<Vec<DManifold>, ScalarError> {
    let mut manifolds = Vec::new();
    for body_a in 0..bodies.len() {
        for body_b in body_a + 1..bodies.len() {
            if bodies[body_a].inverse_mass == 0.0 && bodies[body_b].inverse_mass == 0.0 {
                continue;
            }
            let (
                ShapeV1::Rectangle {
                    half_extents: half_a,
                },
                ShapeV1::Rectangle {
                    half_extents: half_b,
                },
            ) = (bodies[body_a].shape, bodies[body_b].shape)
            else {
                return Err(ScalarError(
                    "S15 full-f64 rectangle contract drift".to_owned(),
                ));
            };
            let half_a = DVec2::new(f64::from(half_a.x), f64::from(half_a.y));
            let half_b = DVec2::new(f64::from(half_b.x), f64::from(half_b.y));
            if let Some(manifold) = collide_rectangles(
                body_a,
                body_b,
                &bodies[body_a],
                half_a,
                &bodies[body_b],
                half_b,
                speculative_distance,
            ) {
                manifolds.push(manifold);
            }
        }
    }
    Ok(manifolds)
}

#[allow(clippy::too_many_arguments)]
fn collide_rectangles(
    body_a_index: usize,
    body_b_index: usize,
    body_a: &DBody,
    half_a: DVec2,
    body_b: &DBody,
    half_b: DVec2,
    speculative_distance: f64,
) -> Option<DManifold> {
    let rectangle_a = DRectangle::new(body_a, half_a);
    let rectangle_b = DRectangle::new(body_b, half_b);
    let (separation_a, separation_b) =
        face_separations_in_a_frame(rectangle_a, rectangle_b, half_a, body_a.angle);
    if separation_a.separation > speculative_distance
        || separation_b.separation > speculative_distance
    {
        return None;
    }
    let center_delta = body_b.position - body_a.position;
    let deeply_coincident_identical = half_a == half_b
        && center_delta.dot(center_delta) <= 0.01
        && separation_a.separation <= -0.05
        && separation_b.separation <= -0.05;
    let reference_a = if deeply_coincident_identical {
        separation_b.separation <= 0.98 * separation_a.separation + 1.0e-3
    } else {
        separation_a.separation >= separation_b.separation
    };
    let (reference, incident, reference_normal, reference_face, normal_a_to_b) = if reference_a {
        let normal = rectangle_a.face_normal(separation_a.face);
        (rectangle_a, rectangle_b, normal, separation_a.face, normal)
    } else {
        let normal = rectangle_b.face_normal(separation_b.face);
        (
            rectangle_b,
            rectangle_a,
            normal,
            separation_b.face,
            normal.scale(-1.0),
        )
    };
    let incident_face = incident.incident_face(reference_normal);
    let reference_vertices = reference.face_vertices(reference_face);
    let mut clipped = vec![
        DClipPoint {
            position: incident.vertices[incident_face.0 as usize],
            vertex: incident_face.0,
        },
        DClipPoint {
            position: incident.vertices[incident_face.1 as usize],
            vertex: incident_face.1,
        },
    ];
    let tangent = (reference_vertices.1 - reference_vertices.0).normalized();
    let side_0 = reference_vertices.0.dot(tangent);
    let side_1 = reference_vertices.1.dot(tangent);
    clipped = clip_segment(clipped, tangent, side_0.min(side_1), true);
    clipped = clip_segment(clipped, tangent, side_0.max(side_1), false);
    let reference_center = (reference_vertices.0 + reference_vertices.1).scale(0.5);
    let mut points = Vec::new();
    for clipped_point in clipped {
        let separation = (clipped_point.position - reference_center).dot(reference_normal);
        let reference_anchor = clipped_point.position - reference_normal.scale(separation);
        let (anchor_a, anchor_b, feature_a, feature_b) = if reference_a {
            (
                reference_anchor,
                clipped_point.position,
                face_feature(reference_face),
                vertex_feature(clipped_point.vertex),
            )
        } else {
            (
                clipped_point.position,
                reference_anchor,
                vertex_feature(clipped_point.vertex),
                face_feature(reference_face),
            )
        };
        points.push(DManifoldPoint {
            local_anchor_a: inverse_rotate(anchor_a - body_a.position, body_a.angle),
            local_anchor_b: inverse_rotate(anchor_b - body_b.position, body_b.angle),
            separation,
            feature_a,
            feature_b,
        });
    }
    if points.is_empty() {
        return None;
    }
    points.sort_by_key(|point| {
        (
            point.feature_a.feature_type,
            point.feature_a.feature_index,
            point.feature_b.feature_type,
            point.feature_b.feature_index,
        )
    });
    points.dedup_by_key(|point| {
        (
            point.feature_a.feature_type,
            point.feature_a.feature_index,
            point.feature_b.feature_type,
            point.feature_b.feature_index,
        )
    });
    Some(DManifold {
        body_a: body_a_index,
        body_b: body_b_index,
        normal: normal_a_to_b,
        friction: (body_a.friction * body_b.friction).sqrt(),
        restitution: body_a.restitution.max(body_b.restitution),
        points,
    })
}

fn face_separations_in_a_frame(
    rectangle_a: DRectangle,
    rectangle_b: DRectangle,
    half_a: DVec2,
    angle_a: f64,
) -> (DFaceSeparation, DFaceSeparation) {
    let vertices_a = [
        DVec2::ZERO,
        DVec2::new(2.0 * half_a.x, 0.0),
        DVec2::new(2.0 * half_a.x, 2.0 * half_a.y),
        DVec2::new(0.0, 2.0 * half_a.y),
    ];
    let origin = rectangle_a.vertices[0];
    let vertices_b = rectangle_b
        .vertices
        .map(|vertex| inverse_rotate(vertex - origin, angle_a));
    let normals_a = [
        DVec2::new(0.0, -1.0),
        DVec2::new(1.0, 0.0),
        DVec2::new(0.0, 1.0),
        DVec2::new(-1.0, 0.0),
    ];
    let normals_b = [0, 1, 2, 3].map(|face| inverse_rotate(rectangle_b.face_normal(face), angle_a));
    (
        best_face(&vertices_a, &normals_a, &vertices_b),
        best_face(&vertices_b, &normals_b, &vertices_a),
    )
}

fn best_face(
    reference_vertices: &[DVec2; 4],
    reference_normals: &[DVec2; 4],
    incident_vertices: &[DVec2; 4],
) -> DFaceSeparation {
    let mut best = DFaceSeparation {
        face: 0,
        separation: f64::NEG_INFINITY,
    };
    for face in 0..4 {
        let separation = incident_vertices
            .iter()
            .map(|vertex| reference_normals[face].dot(*vertex - reference_vertices[face]))
            .fold(f64::INFINITY, f64::min);
        if separation > best.separation {
            best = DFaceSeparation {
                face: face as u32,
                separation,
            };
        }
    }
    best
}

fn clip_segment(
    points: Vec<DClipPoint>,
    tangent: DVec2,
    limit: f64,
    keep_greater: bool,
) -> Vec<DClipPoint> {
    if points.len() != 2 {
        return points;
    }
    let signed = |point: DClipPoint| {
        let value = point.position.dot(tangent) - limit;
        if keep_greater { value } else { -value }
    };
    let distance_a = signed(points[0]);
    let distance_b = signed(points[1]);
    let inside_a = distance_a >= 0.0;
    let inside_b = distance_b >= 0.0;
    match (inside_a, inside_b) {
        (true, true) => points,
        (false, false) => Vec::new(),
        _ => {
            let fraction = distance_a / (distance_a - distance_b);
            let intersection = DClipPoint {
                position: points[0].position
                    + (points[1].position - points[0].position).scale(fraction),
                vertex: if inside_a {
                    points[1].vertex
                } else {
                    points[0].vertex
                },
            };
            if inside_a {
                vec![points[0], intersection]
            } else {
                vec![intersection, points[1]]
            }
        }
    }
}

const fn face_feature(index: u32) -> FeatureIdV1 {
    FeatureIdV1 {
        feature_type: 1,
        feature_index: index,
    }
}

const fn vertex_feature(index: u32) -> FeatureIdV1 {
    FeatureIdV1 {
        feature_type: 2,
        feature_index: index,
    }
}

#[cfg(test)]
mod tests {
    use physbox_contracts::Vec2;
    use physbox_engine::p5_2_centered_rectangle_stack_scenario;

    use super::*;
    use crate::authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePolicyV1,
        solve_authority_structure_steps_observed,
    };

    #[test]
    fn path_f64_zero_motion_preserves_input_separation() {
        let inputs = AuthoritySeparationInputsV1 {
            normal: Vec2::new(0.0, 1.0),
            anchor_a: Vec2::new(0.35, -0.125),
            anchor_b: Vec2::new(0.35, -0.125),
            position_a: Vec2::new(0.0, 1.0),
            position_b: Vec2::new(0.0, 1.25),
            initial_position_a: Vec2::new(0.0, 1.0),
            initial_position_b: Vec2::new(0.0, 1.25),
            angle_a: 0.0,
            angle_b: 0.0,
            initial_angle_a: 0.0,
            initial_angle_b: 0.0,
            input_separation: -1.0e-4,
        };
        let mut statistics = PrecisionPathStatisticsV1::default();
        let result = evaluate_precision_path_v1(
            inputs,
            PrecisionPerturbationV1::Nearest,
            0.0,
            0,
            0,
            0,
            0,
            &mut statistics,
        );
        assert_eq!(result.selected_bits, inputs.input_separation.to_bits());
        assert_eq!(statistics.evaluations, 1);
        assert_eq!(statistics.non_finite_results, 0);
    }

    #[test]
    fn full_f64_replays_on_a_short_centered_stack() {
        let mut scenario = p5_2_centered_rectangle_stack_scenario(4, 1).unwrap();
        scenario.settings.contact_softness = Some(physbox_contracts::ContactSoftnessPolicyV1 {
            version: 1,
            contact_hertz: physbox_contracts::FiniteF32::new(30.0).unwrap(),
            damping_ratio: physbox_contracts::FiniteF32::new(10.0).unwrap(),
            static_contact_hertz_multiplier: physbox_contracts::FiniteF32::new(1.0).unwrap(),
        });
        let first = solve_full_f64_authority_v1(&scenario).unwrap();
        let second = solve_full_f64_authority_v1(&scenario).unwrap();
        let binary32 = solve_authority_structure_steps_observed(
            &scenario,
            AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
            false,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.statistics.outer_steps, 1);
        assert_eq!(first.statistics.non_finite_values, 0);
        assert_eq!(first.observations.len(), 1);
        for (full, reference) in first.final_bodies.iter().zip(&binary32.result.bodies) {
            assert_eq!(full.id, reference.id);
            assert!(
                (full.position[0] - f64::from(reference.position.x)).abs() <= 2.0e-5,
                "body {:?} x {} vs {}",
                full.id,
                full.position[0],
                reference.position.x
            );
            assert!(
                (full.position[1] - f64::from(reference.position.y)).abs() <= 2.0e-5,
                "body {:?} y {} vs {}",
                full.id,
                full.position[1],
                reference.position.y
            );
        }
    }
}
