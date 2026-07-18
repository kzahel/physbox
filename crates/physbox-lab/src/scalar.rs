use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredManifoldV1, BodyId, BodyStateV1, ContactCachePointV1, ContactSoftnessPolicyV1,
    FiniteF32, ScenarioV1, TraceRecordV1, TraceStageV1, TraceValueV1, Validate, Vec2,
};

use crate::fixture_contacts::generate_fixture_contacts;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarError(pub String);

impl Display for ScalarError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ScalarError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarSolveResult {
    pub bodies: Vec<BodyStateV1>,
    pub contact_cache: Vec<ContactCachePointV1>,
    pub traces: Vec<TraceRecordV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceDivergence {
    pub record_index: usize,
    pub value_name: String,
    pub expected: Option<f32>,
    pub actual: Option<f32>,
    pub reason: String,
}

/// Scalar result of the pinned Box2D v3 `b2MakeSoft` equation.
///
/// Provenance: clean Box2D revision `3a4f0da8374af61293a03021c9a0b3ebcfe67948`,
/// `src/solver.h`, plus the contact/static selection and hertz cap in
/// `src/physics_world.c`. This is an independent Rust specification, not
/// linked or copied product code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactSoftnessCoefficients {
    pub bias_rate: f32,
    pub mass_scale: f32,
    pub impulse_scale: f32,
}

/// Inputs to the pinned Box2D v3 scalar normal-contact update.
///
/// The penetration coefficients are the already selected contact softness for
/// the body pair. Positive separation always selects the authority's
/// speculative branch; `use_bias = false` selects the hard relaxation branch
/// only for non-positive separation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2dPointNormalInput {
    pub separation: f32,
    pub substep_dt: f32,
    pub normal_velocity: f32,
    pub normal_mass: f32,
    pub old_impulse: f32,
    pub penetration_velocity_bias: f32,
    pub penetration_mass_scale: f32,
    pub penetration_impulse_scale: f32,
    pub use_bias: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2dPointNormalUpdate {
    pub velocity_bias: f32,
    pub mass_scale: f32,
    pub impulse_scale: f32,
    pub impulse: f32,
    pub applied_impulse: f32,
}

/// Independent Rust transcription of the pinned Box2D v3 point-normal update.
///
/// Provenance: clean revision `3a4f0da8374af61293a03021c9a0b3ebcfe67948`,
/// `src/contact_solver.c` in the scalar overflow and SIMD colored paths. This
/// function documents the public recurrence; it does not link or copy Box2D.
pub fn box2d_point_normal_update(
    input: Box2dPointNormalInput,
) -> Result<Box2dPointNormalUpdate, ScalarError> {
    let finite = [
        input.separation,
        input.substep_dt,
        input.normal_velocity,
        input.normal_mass,
        input.old_impulse,
        input.penetration_velocity_bias,
        input.penetration_mass_scale,
        input.penetration_impulse_scale,
    ]
    .into_iter()
    .all(f32::is_finite);
    if !finite
        || input.substep_dt <= 0.0
        || input.normal_mass < 0.0
        || input.old_impulse < 0.0
        || input.penetration_mass_scale < 0.0
        || input.penetration_impulse_scale < 0.0
    {
        return Err(ScalarError(
            "Box2D point-normal input is outside the finite unilateral domain".to_owned(),
        ));
    }
    let (velocity_bias, mass_scale, impulse_scale) = if input.separation > 0.0 {
        (input.separation / input.substep_dt, 1.0, 0.0)
    } else if input.use_bias {
        (
            input.penetration_velocity_bias,
            input.penetration_mass_scale,
            input.penetration_impulse_scale,
        )
    } else {
        (0.0, 1.0, 0.0)
    };
    let candidate = -input.normal_mass * (mass_scale * input.normal_velocity + velocity_bias)
        - impulse_scale * input.old_impulse;
    let impulse = (input.old_impulse + candidate).max(0.0);
    let update = Box2dPointNormalUpdate {
        velocity_bias,
        mass_scale,
        impulse_scale,
        impulse,
        applied_impulse: impulse - input.old_impulse,
    };
    if ![
        update.velocity_bias,
        update.mass_scale,
        update.impulse_scale,
        update.impulse,
        update.applied_impulse,
    ]
    .into_iter()
    .all(f32::is_finite)
    {
        return Err(ScalarError(
            "Box2D point-normal update produced a non-finite value".to_owned(),
        ));
    }
    Ok(update)
}

/// Exact kinetic-energy change for a scalar impulse applied along a constraint
/// with pre-update normal velocity `v` and inverse effective mass `k`.
pub fn point_impulse_kinetic_energy_delta(
    normal_velocity: f64,
    inverse_effective_mass: f64,
    applied_impulse: f64,
) -> f64 {
    normal_velocity * applied_impulse
        + 0.5 * inverse_effective_mass * applied_impulse * applied_impulse
}

pub fn box2d_softness_coefficients(
    hertz: f32,
    damping_ratio: f32,
    substep_dt: f32,
) -> Result<ContactSoftnessCoefficients, ScalarError> {
    if !hertz.is_finite()
        || !damping_ratio.is_finite()
        || !substep_dt.is_finite()
        || hertz < 0.0
        || damping_ratio < 0.0
        || substep_dt <= 0.0
    {
        return Err(ScalarError(
            "softness coefficients require finite nonnegative hertz/damping and positive dt"
                .to_owned(),
        ));
    }
    if hertz == 0.0 {
        return Ok(ContactSoftnessCoefficients {
            bias_rate: 0.0,
            mass_scale: 0.0,
            impulse_scale: 0.0,
        });
    }
    let omega = 2.0 * std::f32::consts::PI * hertz;
    let a1 = 2.0 * damping_ratio + substep_dt * omega;
    let a2 = substep_dt * omega * a1;
    let impulse_scale = 1.0 / (1.0 + a2);
    let coefficients = ContactSoftnessCoefficients {
        bias_rate: omega / a1,
        mass_scale: a2 * impulse_scale,
        impulse_scale,
    };
    if !coefficients.bias_rate.is_finite()
        || !coefficients.mass_scale.is_finite()
        || !coefficients.impulse_scale.is_finite()
    {
        return Err(ScalarError(
            "softness coefficient derivation produced a non-finite value".to_owned(),
        ));
    }
    Ok(coefficients)
}

#[derive(Clone, Debug)]
struct PreparedContact {
    manifold_id: u32,
    body_a: usize,
    body_b: usize,
    normal: Vec2,
    tangent: Vec2,
    friction: f32,
    restitution: f32,
    points: Vec<PreparedPoint>,
}

#[derive(Clone, Debug)]
struct PreparedPoint {
    feature_a: physbox_contracts::FeatureIdV1,
    feature_b: physbox_contracts::FeatureIdV1,
    local_anchor_a: Vec2,
    local_anchor_b: Vec2,
    anchor_a: Vec2,
    anchor_b: Vec2,
    separation: f32,
    normal_mass: f32,
    tangent_mass: f32,
    initial_normal_velocity: f32,
    normal_impulse: f32,
    tangent_impulse: f32,
    total_normal_impulse: f32,
}

pub fn solve_authored_contact_pass(
    scenario: &ScenarioV1,
) -> Result<ScalarSolveResult, ScalarError> {
    scenario
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    let mut bodies = scenario.bodies.clone();
    let mut traces = Vec::new();
    let mut contacts = Vec::with_capacity(scenario.manifolds.len());
    for manifold in &scenario.manifolds {
        contacts.push(prepare_contact(
            &bodies,
            manifold,
            &scenario.contact_cache,
            &mut traces,
        )?);
    }

    for contact in &contacts {
        warm_start(contact, &mut bodies, &mut traces)?;
    }
    for iteration in 0..scenario.settings.iterations {
        for contact in &mut contacts {
            solve_normal(
                contact,
                &mut bodies,
                scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32,
                scenario.settings.bias_rate.get(),
                maximum_depenetration_velocity(scenario),
                scenario.settings.mass_coefficient.get(),
                scenario.settings.impulse_coefficient.get(),
                scenario.settings.contact_softness,
                true,
                iteration,
                TraceStageV1::NormalSolve,
                &mut traces,
            )?;
            solve_tangent(
                contact,
                &mut bodies,
                iteration,
                TraceStageV1::TangentSolve,
                &mut traces,
            )?;
        }
    }
    for contact in &mut contacts {
        solve_normal(
            contact,
            &mut bodies,
            scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32,
            scenario.settings.bias_rate.get(),
            maximum_depenetration_velocity(scenario),
            scenario.settings.mass_coefficient.get(),
            scenario.settings.impulse_coefficient.get(),
            scenario.settings.contact_softness,
            false,
            0,
            TraceStageV1::Relaxation,
            &mut traces,
        )?;
        solve_tangent(
            contact,
            &mut bodies,
            0,
            TraceStageV1::Relaxation,
            &mut traces,
        )?;
    }
    for contact in &mut contacts {
        apply_restitution(
            contact,
            &mut bodies,
            scenario.settings.restitution_threshold.get(),
            &mut traces,
        )?;
    }

    let contact_cache = collect_contact_cache(&contacts, &bodies)?;
    Ok(ScalarSolveResult {
        bodies,
        contact_cache,
        traces,
    })
}

pub fn solve_analytic_fixture_steps(
    scenario: &ScenarioV1,
) -> Result<ScalarSolveResult, ScalarError> {
    scenario
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    if !scenario.manifolds.is_empty() {
        return Err(ScalarError(
            "analytic fixture steps regenerate contacts and require an empty authored manifold list"
                .to_owned(),
        ));
    }
    if !scenario.actions.is_empty() {
        return Err(ScalarError(
            "analytic fixture steps do not support actions".to_owned(),
        ));
    }
    let mut bodies = scenario.bodies.clone();
    let mut contact_cache = scenario.contact_cache.clone();
    let mut traces = Vec::new();
    let substep_dt = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
    for step in 0..scenario.requested_steps {
        for substep in 0..scenario.settings.substeps {
            let manifolds = generate_fixture_contacts(&bodies)
                .map_err(|error| ScalarError(error.to_string()))?;
            let trace_start = traces.len();
            let mut contacts = Vec::with_capacity(manifolds.len());
            for manifold in &manifolds {
                contacts.push(prepare_contact(
                    &bodies,
                    manifold,
                    &contact_cache,
                    &mut traces,
                )?);
            }

            integrate_velocities(&mut bodies, scenario.settings.gravity, substep_dt)?;
            for contact in &contacts {
                warm_start(contact, &mut bodies, &mut traces)?;
            }
            for iteration in 0..scenario.settings.iterations {
                for contact in &mut contacts {
                    solve_normal(
                        contact,
                        &mut bodies,
                        substep_dt,
                        scenario.settings.bias_rate.get(),
                        maximum_depenetration_velocity(scenario),
                        scenario.settings.mass_coefficient.get(),
                        scenario.settings.impulse_coefficient.get(),
                        scenario.settings.contact_softness,
                        true,
                        iteration,
                        TraceStageV1::NormalSolve,
                        &mut traces,
                    )?;
                    solve_tangent(
                        contact,
                        &mut bodies,
                        iteration,
                        TraceStageV1::TangentSolve,
                        &mut traces,
                    )?;
                }
            }

            integrate_positions(&mut bodies, substep_dt, &mut traces)?;
            refresh_separations(&mut contacts, &bodies);
            for contact in &mut contacts {
                solve_normal(
                    contact,
                    &mut bodies,
                    substep_dt,
                    scenario.settings.bias_rate.get(),
                    maximum_depenetration_velocity(scenario),
                    scenario.settings.mass_coefficient.get(),
                    scenario.settings.impulse_coefficient.get(),
                    scenario.settings.contact_softness,
                    false,
                    0,
                    TraceStageV1::Relaxation,
                    &mut traces,
                )?;
                solve_tangent(
                    contact,
                    &mut bodies,
                    0,
                    TraceStageV1::Relaxation,
                    &mut traces,
                )?;
                apply_restitution(
                    contact,
                    &mut bodies,
                    scenario.settings.restitution_threshold.get(),
                    &mut traces,
                )?;
            }
            contact_cache = collect_contact_cache(&contacts, &bodies)?;
            for trace in &mut traces[trace_start..] {
                trace.step = step;
                trace.substep = substep;
            }
        }
    }
    Ok(ScalarSolveResult {
        bodies,
        contact_cache,
        traces,
    })
}

pub fn first_trace_divergence(
    expected: &[TraceRecordV1],
    actual: &[TraceRecordV1],
    absolute_tolerance: f32,
    relative_tolerance: f32,
) -> Option<TraceDivergence> {
    let count = expected.len().max(actual.len());
    for record_index in 0..count {
        let Some(expected_record) = expected.get(record_index) else {
            return Some(structural_divergence(
                record_index,
                "unexpected actual record",
            ));
        };
        let Some(actual_record) = actual.get(record_index) else {
            return Some(structural_divergence(record_index, "missing actual record"));
        };
        if trace_identity(expected_record) != trace_identity(actual_record) {
            return Some(structural_divergence(
                record_index,
                "trace record identity differs",
            ));
        }
        if expected_record.values.len() != actual_record.values.len() {
            return Some(structural_divergence(
                record_index,
                "trace value count differs",
            ));
        }
        for (expected_value, actual_value) in
            expected_record.values.iter().zip(&actual_record.values)
        {
            if expected_value.name != actual_value.name {
                return Some(structural_divergence(
                    record_index,
                    "trace value name differs",
                ));
            }
            let expected_number = expected_value.value.get();
            let actual_number = actual_value.value.get();
            let tolerance = absolute_tolerance
                + relative_tolerance * expected_number.abs().max(actual_number.abs());
            if (expected_number - actual_number).abs() > tolerance {
                return Some(TraceDivergence {
                    record_index,
                    value_name: expected_value.name.clone(),
                    expected: Some(expected_number),
                    actual: Some(actual_number),
                    reason: format!(
                        "value differs by more than absolute {absolute_tolerance} plus relative {relative_tolerance}"
                    ),
                });
            }
        }
    }
    None
}

fn prepare_contact(
    bodies: &[BodyStateV1],
    manifold: &AuthoredManifoldV1,
    cache: &[ContactCachePointV1],
    traces: &mut Vec<TraceRecordV1>,
) -> Result<PreparedContact, ScalarError> {
    let body_a = find_body(bodies, manifold.body_a)?;
    let body_b = find_body(bodies, manifold.body_b)?;
    if body_a == body_b {
        return Err(ScalarError(format!(
            "manifold {} references the same body twice",
            manifold.id
        )));
    }
    let normal = manifold.normal;
    let tangent = right_perpendicular(normal);
    let mut points = Vec::with_capacity(manifold.points.len());
    for (point_index, point) in manifold.points.iter().enumerate() {
        let anchor_a = rotate(point.local_anchor_a, bodies[body_a].angle.get());
        let anchor_b = rotate(point.local_anchor_b, bodies[body_b].angle.get());
        let normal_mass =
            effective_mass(&bodies[body_a], &bodies[body_b], anchor_a, anchor_b, normal);
        let tangent_mass = effective_mass(
            &bodies[body_a],
            &bodies[body_b],
            anchor_a,
            anchor_b,
            tangent,
        );
        let initial_normal_velocity = dot(
            sub(
                point_velocity(&bodies[body_b], anchor_b),
                point_velocity(&bodies[body_a], anchor_a),
            ),
            normal,
        );
        let cached = cache.iter().find(|cached| {
            cached.body_a == manifold.body_a
                && cached.body_b == manifold.body_b
                && cached.feature_a == point.feature_a
                && cached.feature_b == point.feature_b
        });
        let prepared = PreparedPoint {
            feature_a: point.feature_a,
            feature_b: point.feature_b,
            local_anchor_a: point.local_anchor_a,
            local_anchor_b: point.local_anchor_b,
            anchor_a,
            anchor_b,
            separation: point.separation.get(),
            normal_mass,
            tangent_mass,
            initial_normal_velocity,
            normal_impulse: cached.map_or(0.0, |cached| cached.normal_impulse.get()),
            tangent_impulse: cached.map_or(0.0, |cached| cached.tangent_impulse.get()),
            total_normal_impulse: 0.0,
        };
        traces.push(trace(
            TraceStageV1::Prepare,
            manifold.id,
            point_index,
            0,
            vec![
                ("anchorAX", anchor_a.x),
                ("anchorAY", anchor_a.y),
                ("anchorBX", anchor_b.x),
                ("anchorBY", anchor_b.y),
                ("initialNormalVelocity", initial_normal_velocity),
                ("normalMass", normal_mass),
                ("separation", prepared.separation),
                ("tangentMass", tangent_mass),
            ],
        )?);
        points.push(prepared);
    }
    Ok(PreparedContact {
        manifold_id: manifold.id,
        body_a,
        body_b,
        normal,
        tangent,
        friction: manifold.material.friction.get(),
        restitution: manifold.material.restitution.get(),
        points,
    })
}

fn warm_start(
    contact: &PreparedContact,
    bodies: &mut [BodyStateV1],
    traces: &mut Vec<TraceRecordV1>,
) -> Result<(), ScalarError> {
    for (point_index, point) in contact.points.iter().enumerate() {
        let impulse = add(
            scale(contact.normal, point.normal_impulse),
            scale(contact.tangent, point.tangent_impulse),
        );
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            impulse,
        )?;
        traces.push(trace(
            TraceStageV1::WarmStart,
            contact.manifold_id,
            point_index,
            0,
            vec![
                ("appliedImpulseX", impulse.x),
                ("appliedImpulseY", impulse.y),
                ("normalImpulse", point.normal_impulse),
                ("tangentImpulse", point.tangent_impulse),
            ],
        )?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn solve_normal(
    contact: &mut PreparedContact,
    bodies: &mut [BodyStateV1],
    substep_dt: f32,
    bias_rate: f32,
    max_depenetration_velocity: f32,
    configured_mass_scale: f32,
    configured_impulse_scale: f32,
    contact_softness: Option<ContactSoftnessPolicyV1>,
    use_bias: bool,
    iteration: u32,
    stage: TraceStageV1,
    traces: &mut Vec<TraceRecordV1>,
) -> Result<(), ScalarError> {
    let (configured_bias_rate, configured_mass_scale, configured_impulse_scale) =
        configured_contact_coefficients(
            bodies,
            contact,
            substep_dt,
            bias_rate,
            configured_mass_scale,
            configured_impulse_scale,
            contact_softness,
        )?;
    for (point_index, point) in contact.points.iter_mut().enumerate() {
        let normal_velocity = relative_velocity(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal,
        );
        let old_impulse = point.normal_impulse;
        let update = box2d_point_normal_update(Box2dPointNormalInput {
            separation: point.separation,
            substep_dt,
            normal_velocity,
            normal_mass: point.normal_mass,
            old_impulse,
            penetration_velocity_bias: clamp_depenetration_bias(
                configured_bias_rate * point.separation / substep_dt,
                max_depenetration_velocity,
            ),
            penetration_mass_scale: configured_mass_scale,
            penetration_impulse_scale: configured_impulse_scale,
            use_bias,
        })?;
        let velocity_bias = update.velocity_bias;
        let mass_scale = update.mass_scale;
        let impulse_scale = update.impulse_scale;
        point.normal_impulse = update.impulse;
        let applied_impulse = update.applied_impulse;
        point.total_normal_impulse += applied_impulse.max(0.0);
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            scale(contact.normal, applied_impulse),
        )?;
        traces.push(trace(
            stage.clone(),
            contact.manifold_id,
            point_index,
            iteration,
            vec![
                ("appliedNormalImpulse", applied_impulse),
                ("impulseScale", impulse_scale),
                ("massScale", mass_scale),
                ("normalImpulse", point.normal_impulse),
                ("normalVelocity", normal_velocity),
                ("velocityBias", velocity_bias),
            ],
        )?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn configured_contact_coefficients(
    bodies: &[BodyStateV1],
    contact: &PreparedContact,
    substep_dt: f32,
    hard_bias_rate: f32,
    hard_mass_scale: f32,
    hard_impulse_scale: f32,
    policy: Option<ContactSoftnessPolicyV1>,
) -> Result<(f32, f32, f32), ScalarError> {
    let Some(policy) = policy else {
        return Ok((hard_bias_rate, hard_mass_scale, hard_impulse_scale));
    };
    policy
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    let capped_contact_hertz = policy.contact_hertz.get().min(0.125 / substep_dt);
    let static_contact = bodies[contact.body_a].inverse_mass.get() == 0.0
        || bodies[contact.body_b].inverse_mass.get() == 0.0;
    let hertz = if static_contact {
        capped_contact_hertz * policy.static_contact_hertz_multiplier.get()
    } else {
        capped_contact_hertz
    };
    let coefficients = box2d_softness_coefficients(hertz, policy.damping_ratio.get(), substep_dt)?;
    // The existing solver surface stores the hard Baumgarte factor as a
    // dimensionless rate divided by h. Box2D's soft bias is
    // massScale * biasRate * separation, so project that exact velocity term
    // through the same surface without changing the selected hard path.
    Ok((
        coefficients.mass_scale * coefficients.bias_rate * substep_dt,
        coefficients.mass_scale,
        coefficients.impulse_scale,
    ))
}

fn maximum_depenetration_velocity(scenario: &ScenarioV1) -> f32 {
    scenario
        .settings
        .contact_policy
        .map_or(0.0, |policy| policy.max_depenetration_velocity.get())
}

fn clamp_depenetration_bias(velocity_bias: f32, maximum: f32) -> f32 {
    if maximum > 0.0 {
        velocity_bias.max(-maximum)
    } else {
        velocity_bias
    }
}

fn solve_tangent(
    contact: &mut PreparedContact,
    bodies: &mut [BodyStateV1],
    iteration: u32,
    stage: TraceStageV1,
    traces: &mut Vec<TraceRecordV1>,
) -> Result<(), ScalarError> {
    for (point_index, point) in contact.points.iter_mut().enumerate() {
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
        let applied_impulse = point.tangent_impulse - old_impulse;
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            scale(contact.tangent, applied_impulse),
        )?;
        traces.push(trace(
            stage.clone(),
            contact.manifold_id,
            point_index,
            iteration,
            vec![
                ("appliedTangentImpulse", applied_impulse),
                ("frictionLimit", maximum),
                ("tangentImpulse", point.tangent_impulse),
                ("tangentVelocity", tangent_velocity),
            ],
        )?);
    }
    Ok(())
}

fn apply_restitution(
    contact: &mut PreparedContact,
    bodies: &mut [BodyStateV1],
    threshold: f32,
    traces: &mut Vec<TraceRecordV1>,
) -> Result<(), ScalarError> {
    for (point_index, point) in contact.points.iter_mut().enumerate() {
        let normal_velocity = relative_velocity(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            contact.normal,
        );
        let old_impulse = point.normal_impulse;
        let eligible = contact.restitution > 0.0
            && point.initial_normal_velocity < -threshold
            && point.total_normal_impulse > 0.0;
        if eligible {
            let candidate = -point.normal_mass
                * (normal_velocity + contact.restitution * point.initial_normal_velocity);
            point.normal_impulse = (old_impulse + candidate).max(0.0);
        }
        let applied_impulse = point.normal_impulse - old_impulse;
        apply_pair_impulse(
            bodies,
            contact.body_a,
            contact.body_b,
            point.anchor_a,
            point.anchor_b,
            scale(contact.normal, applied_impulse),
        )?;
        traces.push(trace(
            TraceStageV1::Restitution,
            contact.manifold_id,
            point_index,
            0,
            vec![
                ("appliedRestitutionImpulse", applied_impulse),
                ("eligible", if eligible { 1.0 } else { 0.0 }),
                ("initialNormalVelocity", point.initial_normal_velocity),
                ("normalImpulse", point.normal_impulse),
                ("normalVelocity", normal_velocity),
            ],
        )?);
    }
    Ok(())
}

fn integrate_velocities(
    bodies: &mut [BodyStateV1],
    gravity: Vec2,
    substep_dt: f32,
) -> Result<(), ScalarError> {
    for body in bodies {
        if body.inverse_mass.get() == 0.0 || body.sleeping {
            continue;
        }
        let velocity = add(body.linear_velocity, scale(gravity, substep_dt));
        velocity
            .validate()
            .map_err(|_| ScalarError("gravity produced a non-finite velocity".to_owned()))?;
        body.linear_velocity = velocity;
    }
    Ok(())
}

fn integrate_positions(
    bodies: &mut [BodyStateV1],
    substep_dt: f32,
    traces: &mut Vec<TraceRecordV1>,
) -> Result<(), ScalarError> {
    for (body_index, body) in bodies.iter_mut().enumerate() {
        if body.inverse_mass.get() != 0.0 && !body.sleeping {
            let position = add(body.position, scale(body.linear_velocity, substep_dt));
            position.validate().map_err(|_| {
                ScalarError("integration produced a non-finite position".to_owned())
            })?;
            body.position = position;
            body.angle = finite_result(
                body.angle.get() + substep_dt * body.angular_velocity.get(),
                "integrated angle",
            )?;
        }
        traces.push(trace(
            TraceStageV1::Integrate,
            u32::MAX,
            body_index,
            0,
            vec![
                ("angle", body.angle.get()),
                ("angularVelocity", body.angular_velocity.get()),
                ("positionX", body.position.x),
                ("positionY", body.position.y),
                ("velocityX", body.linear_velocity.x),
                ("velocityY", body.linear_velocity.y),
            ],
        )?);
    }
    Ok(())
}

fn refresh_separations(contacts: &mut [PreparedContact], bodies: &[BodyStateV1]) {
    for contact in contacts {
        for point in &mut contact.points {
            let current_anchor_a = rotate(point.local_anchor_a, bodies[contact.body_a].angle.get());
            let current_anchor_b = rotate(point.local_anchor_b, bodies[contact.body_b].angle.get());
            let world_a = add(bodies[contact.body_a].position, current_anchor_a);
            let world_b = add(bodies[contact.body_b].position, current_anchor_b);
            point.separation = dot(sub(world_b, world_a), contact.normal);
        }
    }
}

fn collect_contact_cache(
    contacts: &[PreparedContact],
    bodies: &[BodyStateV1],
) -> Result<Vec<ContactCachePointV1>, ScalarError> {
    let mut cache = Vec::new();
    for contact in contacts {
        let body_a = bodies[contact.body_a].id;
        let body_b = bodies[contact.body_b].id;
        for point in &contact.points {
            cache.push(ContactCachePointV1 {
                body_a,
                body_b,
                feature_a: point.feature_a,
                feature_b: point.feature_b,
                normal_impulse: finite_result(point.normal_impulse, "cached normal impulse")?,
                tangent_impulse: finite_result(point.tangent_impulse, "cached tangent impulse")?,
            });
        }
    }
    Ok(cache)
}

fn apply_pair_impulse(
    bodies: &mut [BodyStateV1],
    index_a: usize,
    index_b: usize,
    anchor_a: Vec2,
    anchor_b: Vec2,
    impulse: Vec2,
) -> Result<(), ScalarError> {
    let (body_a, body_b) = body_pair_mut(bodies, index_a, index_b);
    let velocity_a = sub(
        body_a.linear_velocity,
        scale(impulse, body_a.inverse_mass.get()),
    );
    let angular_a =
        body_a.angular_velocity.get() - body_a.inverse_inertia.get() * cross(anchor_a, impulse);
    let velocity_b = add(
        body_b.linear_velocity,
        scale(impulse, body_b.inverse_mass.get()),
    );
    let angular_b =
        body_b.angular_velocity.get() + body_b.inverse_inertia.get() * cross(anchor_b, impulse);
    velocity_a
        .validate()
        .map_err(|_| ScalarError("contact impulse produced non-finite velocity A".to_owned()))?;
    velocity_b
        .validate()
        .map_err(|_| ScalarError("contact impulse produced non-finite velocity B".to_owned()))?;
    body_a.linear_velocity = velocity_a;
    body_a.angular_velocity = finite_result(angular_a, "angular velocity A")?;
    body_b.linear_velocity = velocity_b;
    body_b.angular_velocity = finite_result(angular_b, "angular velocity B")?;
    Ok(())
}

fn body_pair_mut(
    bodies: &mut [BodyStateV1],
    index_a: usize,
    index_b: usize,
) -> (&mut BodyStateV1, &mut BodyStateV1) {
    if index_a < index_b {
        let (left, right) = bodies.split_at_mut(index_b);
        (&mut left[index_a], &mut right[0])
    } else {
        let (left, right) = bodies.split_at_mut(index_a);
        (&mut right[0], &mut left[index_b])
    }
}

fn relative_velocity(
    bodies: &[BodyStateV1],
    body_a: usize,
    body_b: usize,
    anchor_a: Vec2,
    anchor_b: Vec2,
    axis: Vec2,
) -> f32 {
    dot(
        sub(
            point_velocity(&bodies[body_b], anchor_b),
            point_velocity(&bodies[body_a], anchor_a),
        ),
        axis,
    )
}

fn point_velocity(body: &BodyStateV1, anchor: Vec2) -> Vec2 {
    add(
        body.linear_velocity,
        cross_scalar_vector(body.angular_velocity.get(), anchor),
    )
}

fn effective_mass(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    anchor_a: Vec2,
    anchor_b: Vec2,
    axis: Vec2,
) -> f32 {
    let cross_a = cross(anchor_a, axis);
    let cross_b = cross(anchor_b, axis);
    let inverse = body_a.inverse_mass.get()
        + body_b.inverse_mass.get()
        + body_a.inverse_inertia.get() * cross_a * cross_a
        + body_b.inverse_inertia.get() * cross_b * cross_b;
    if inverse > 0.0 { 1.0 / inverse } else { 0.0 }
}

fn find_body(bodies: &[BodyStateV1], id: BodyId) -> Result<usize, ScalarError> {
    bodies
        .binary_search_by_key(&id, |body| body.id)
        .map_err(|_| ScalarError(format!("manifold references missing body {id:?}")))
}

fn trace(
    stage: TraceStageV1,
    manifold_id: u32,
    point_index: usize,
    iteration: u32,
    mut values: Vec<(&str, f32)>,
) -> Result<TraceRecordV1, ScalarError> {
    values.sort_unstable_by_key(|(name, _)| *name);
    let record = TraceRecordV1 {
        step: 0,
        substep: 0,
        iteration,
        stage,
        manifold_id,
        point_index: point_index as u32,
        values: values
            .into_iter()
            .map(|(name, value)| {
                Ok(TraceValueV1 {
                    name: name.to_owned(),
                    value: finite_result(value, name)?,
                    bits: value.to_bits(),
                })
            })
            .collect::<Result<Vec<_>, ScalarError>>()?,
    };
    record
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    Ok(record)
}

fn trace_identity(record: &TraceRecordV1) -> (u32, u32, u32, &TraceStageV1, u32, u32) {
    (
        record.step,
        record.substep,
        record.iteration,
        &record.stage,
        record.manifold_id,
        record.point_index,
    )
}

fn structural_divergence(record_index: usize, reason: &str) -> TraceDivergence {
    TraceDivergence {
        record_index,
        value_name: "<structure>".to_owned(),
        expected: None,
        actual: None,
        reason: reason.to_owned(),
    }
}

fn finite_result(value: f32, name: &str) -> Result<FiniteF32, ScalarError> {
    FiniteF32::new(value)
        .map_err(|_| ScalarError(format!("scalar {name} produced a non-finite value")))
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

fn scale(a: Vec2, scalar: f32) -> Vec2 {
    Vec2::new(a.x * scalar, a.y * scalar)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn cross_scalar_vector(scalar: f32, vector: Vec2) -> Vec2 {
    Vec2::new(-scalar * vector.y, scalar * vector.x)
}

fn right_perpendicular(vector: Vec2) -> Vec2 {
    Vec2::new(vector.y, -vector.x)
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{
        AuthoredContactPointV1, CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1,
        FeatureIdV1, MaterialV1, ShapeV1,
    };
    use physbox_engine::{
        AnalyticFixtureKind, analytic_fixture_scenario, authored_one_contact_scenario,
        p5_2_contact_policy_candidate,
    };

    use super::*;

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    fn softness(hertz: f32, damping: f32, static_multiplier: f32) -> ContactSoftnessPolicyV1 {
        ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite(hertz),
            damping_ratio: finite(damping),
            static_contact_hertz_multiplier: finite(static_multiplier),
        }
    }

    #[test]
    fn pinned_softness_equation_has_analytic_limits_and_complements() {
        let zero = box2d_softness_coefficients(0.0, 10.0, 1.0 / 240.0).unwrap();
        assert_eq!(zero.bias_rate, 0.0);
        assert_eq!(zero.mass_scale, 0.0);
        assert_eq!(zero.impulse_scale, 0.0);

        let soft = box2d_softness_coefficients(30.0, 10.0, 1.0 / 240.0).unwrap();
        assert!(soft.bias_rate > 0.0);
        assert!(soft.mass_scale > 0.0 && soft.mass_scale < 1.0);
        assert!(soft.impulse_scale > 0.0 && soft.impulse_scale < 1.0);
        assert_close(soft.mass_scale + soft.impulse_scale, 1.0);

        let stiff = box2d_softness_coefficients(60.0, 10.0, 1.0 / 240.0).unwrap();
        assert!(stiff.mass_scale > soft.mass_scale);
        assert!(stiff.impulse_scale < soft.impulse_scale);
        assert!(box2d_softness_coefficients(-1.0, 10.0, 1.0 / 240.0).is_err());
        assert!(box2d_softness_coefficients(30.0, -1.0, 1.0 / 240.0).is_err());
        assert!(box2d_softness_coefficients(30.0, 10.0, 0.0).is_err());
    }

    #[test]
    fn pinned_point_recurrence_selects_speculative_soft_and_relaxation_branches() {
        let common = Box2dPointNormalInput {
            separation: -0.01,
            substep_dt: 1.0 / 240.0,
            normal_velocity: -0.25,
            normal_mass: 0.5,
            old_impulse: 0.2,
            penetration_velocity_bias: -0.1,
            penetration_mass_scale: 0.75,
            penetration_impulse_scale: 0.25,
            use_bias: true,
        };
        let soft = box2d_point_normal_update(common).unwrap();
        assert_close(soft.velocity_bias, -0.1);
        assert_close(soft.mass_scale, 0.75);
        assert_close(soft.impulse_scale, 0.25);
        assert_close(soft.impulse, 0.29375);

        let relax = box2d_point_normal_update(Box2dPointNormalInput {
            use_bias: false,
            ..common
        })
        .unwrap();
        assert_eq!(relax.velocity_bias, 0.0);
        assert_eq!(relax.mass_scale, 1.0);
        assert_eq!(relax.impulse_scale, 0.0);
        assert_close(relax.impulse, 0.325);

        let speculative = box2d_point_normal_update(Box2dPointNormalInput {
            separation: 0.001,
            use_bias: false,
            ..common
        })
        .unwrap();
        assert_close(speculative.velocity_bias, 0.24);
        assert_eq!(speculative.mass_scale, 1.0);
        assert_eq!(speculative.impulse_scale, 0.0);
        assert_close(speculative.impulse, 0.205);
    }

    #[test]
    fn hard_relaxation_projection_is_dissipative_with_unilateral_clamping() {
        for inverse_effective_mass in [0.25_f32, 1.0, 7.0] {
            for normal_velocity in [-2.0_f32, -0.1, 0.0, 0.1, 2.0] {
                for old_impulse in [0.0_f32, 0.01, 0.5] {
                    let update = box2d_point_normal_update(Box2dPointNormalInput {
                        separation: -0.001,
                        substep_dt: 1.0 / 240.0,
                        normal_velocity,
                        normal_mass: inverse_effective_mass.recip(),
                        old_impulse,
                        penetration_velocity_bias: -1.0,
                        penetration_mass_scale: 0.5,
                        penetration_impulse_scale: 0.5,
                        use_bias: false,
                    })
                    .unwrap();
                    let delta = point_impulse_kinetic_energy_delta(
                        f64::from(normal_velocity),
                        f64::from(inverse_effective_mass),
                        f64::from(update.applied_impulse),
                    );
                    assert!(delta <= 1.0e-12, "energy delta {delta}");
                }
            }
        }
    }

    #[test]
    fn pinned_speculative_relaxation_can_inject_kinetic_energy() {
        let update = box2d_point_normal_update(Box2dPointNormalInput {
            separation: 1.0e-5,
            substep_dt: 1.0 / 240.0,
            normal_velocity: 0.0,
            normal_mass: 0.25,
            old_impulse: 0.1,
            penetration_velocity_bias: 0.0,
            penetration_mass_scale: 1.0,
            penetration_impulse_scale: 0.0,
            use_bias: false,
        })
        .unwrap();
        let delta = point_impulse_kinetic_energy_delta(0.0, 4.0, f64::from(update.applied_impulse));
        assert!(update.applied_impulse < 0.0);
        assert!(delta > 0.0);
    }

    #[test]
    fn scalar_soft_contact_uses_capped_static_coefficients() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.settings.contact_softness = Some(softness(60.0, 10.0, 2.0));
        scenario.manifolds[0].material.restitution = finite(0.0);
        scenario.manifolds[0].points[0].separation = finite(-0.01);
        scenario.bodies[0].linear_velocity = Vec2::ZERO;

        let result = solve_authored_contact_pass(&scenario).unwrap();
        let substep_dt = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
        // The requested 60 Hz is capped to 30 Hz before the static 2x path.
        let expected = box2d_softness_coefficients(60.0, 10.0, substep_dt).unwrap();
        assert_close(
            trace_value(&result.traces[2], "massScale"),
            expected.mass_scale,
        );
        assert_close(
            trace_value(&result.traces[2], "impulseScale"),
            expected.impulse_scale,
        );
        assert_close(
            trace_value(&result.traces[2], "velocityBias"),
            expected.mass_scale * expected.bias_rate * -0.01,
        );
    }

    #[test]
    fn central_frictionless_impact_has_analytic_impulse_and_effective_mass() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.manifolds[0].material.friction = finite(0.0);
        scenario.manifolds[0].material.restitution = finite(0.0);
        scenario.bodies[0].linear_velocity = Vec2::new(0.0, -2.0);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_close(result.bodies[0].linear_velocity.y, 0.0);
        assert_eq!(
            result.contact_cache[0].normal_impulse.get().to_bits(),
            2.0f32.to_bits()
        );
        assert_eq!(
            trace_value(&result.traces[0], "normalMass").to_bits(),
            1.0f32.to_bits()
        );
    }

    #[test]
    fn friction_projects_the_accumulated_tangent_impulse() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.manifolds[0].material.restitution = finite(0.0);
        scenario.bodies[0].linear_velocity = Vec2::new(2.0, -2.0);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_close(result.contact_cache[0].normal_impulse.get(), 2.0);
        assert_close(result.contact_cache[0].tangent_impulse.get(), -1.0);
        assert_close(result.bodies[0].linear_velocity.x, 1.0);
    }

    #[test]
    fn restitution_uses_the_pre_solve_impact_velocity_and_threshold() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.manifolds[0].material.friction = finite(0.0);
        scenario.manifolds[0].material.restitution = finite(0.5);
        scenario.bodies[0].linear_velocity = Vec2::new(0.0, -4.0);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_close(result.bodies[0].linear_velocity.y, 2.0);

        scenario.settings.restitution_threshold = finite(5.0);
        let below_threshold = solve_authored_contact_pass(&scenario).unwrap();
        assert_close(below_threshold.bodies[0].linear_velocity.y, 0.0);
    }

    #[test]
    fn separating_speculative_contact_generates_no_normal_impulse() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.manifolds[0].material.restitution = finite(0.0);
        scenario.manifolds[0].points[0].separation = finite(0.1);
        scenario.bodies[0].linear_velocity = Vec2::new(0.0, 1.0);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_eq!(result.contact_cache[0].normal_impulse.get(), 0.0);
        assert_eq!(result.bodies[0].linear_velocity.y, 1.0);
    }

    #[test]
    fn candidate_policy_clamps_deep_overlap_velocity_bias() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
        scenario.manifolds[0].material.restitution = finite(0.0);
        scenario.manifolds[0].points[0].separation = finite(-1.0);
        scenario.bodies[0].linear_velocity = Vec2::ZERO;
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_eq!(trace_value(&result.traces[2], "velocityBias"), -3.0);
        assert_eq!(trace_value(&result.traces[2], "appliedNormalImpulse"), 3.0);
    }

    #[test]
    fn off_center_effective_mass_includes_rotational_response() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.manifolds[0].points[0].local_anchor_a = Vec2::new(1.0, 0.0);
        scenario.bodies[0].inverse_inertia = finite(2.0);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_eq!(
            trace_value(&result.traces[0], "normalMass").to_bits(),
            (1.0f32 / 3.0).to_bits()
        );
    }

    #[test]
    fn bias_relaxation_and_warm_start_have_explicit_pass_baselines() {
        let mut biased = authored_one_contact_scenario(7);
        biased.manifolds[0].material.restitution = finite(0.0);
        biased.manifolds[0].points[0].separation = finite(-0.01);
        biased.bodies[0].linear_velocity = Vec2::ZERO;
        let result = solve_authored_contact_pass(&biased).unwrap();
        assert_close(trace_value(&result.traces[2], "velocityBias"), -0.48);
        assert_close(trace_value(&result.traces[2], "appliedNormalImpulse"), 0.48);
        assert_close(
            trace_value(&result.traces[4], "appliedNormalImpulse"),
            -0.48,
        );

        let mut warm = authored_one_contact_scenario(7);
        warm.manifolds[0].material.restitution = finite(0.0);
        warm.bodies[0].linear_velocity = Vec2::new(0.0, -2.0);
        warm.contact_cache = vec![ContactCachePointV1 {
            body_a: warm.manifolds[0].body_a,
            body_b: warm.manifolds[0].body_b,
            feature_a: warm.manifolds[0].points[0].feature_a,
            feature_b: warm.manifolds[0].points[0].feature_b,
            normal_impulse: finite(1.0),
            tangent_impulse: finite(0.0),
        }];
        warm.settings.mass_coefficient = finite(0.5);
        warm.settings.impulse_coefficient = finite(0.25);
        let result = solve_authored_contact_pass(&warm).unwrap();
        assert_close(trace_value(&result.traces[1], "normalImpulse"), 1.0);
        assert_close(trace_value(&result.traces[2], "massScale"), 0.5);
        assert_close(trace_value(&result.traces[2], "impulseScale"), 0.25);
        assert_close(result.contact_cache[0].normal_impulse.get(), 2.0);
    }

    #[test]
    fn finite_input_that_overflows_scalar_math_is_a_diagnosed_error() {
        let mut scenario = authored_one_contact_scenario(7);
        scenario.bodies[0].angular_velocity = finite(f32::MAX);
        scenario.manifolds[0].points[0].local_anchor_a = Vec2::new(f32::MAX, 0.0);
        let error = solve_authored_contact_pass(&scenario).unwrap_err();
        assert!(error.0.contains("non-finite"));
    }

    #[test]
    fn translation_rotation_and_normal_convention_are_metamorphic() {
        let mut scenario = two_dynamic_scenario();
        let original = solve_authored_contact_pass(&scenario).unwrap();

        for body in &mut scenario.bodies {
            body.position = add(body.position, Vec2::new(17.0, -9.0));
        }
        scenario.manifolds[0].points[0].world_point = add(
            scenario.manifolds[0].points[0].world_point,
            Vec2::new(17.0, -9.0),
        );
        let translated = solve_authored_contact_pass(&scenario).unwrap();
        assert_eq!(original.contact_cache, translated.contact_cache);
        assert_eq!(
            original.bodies[0].linear_velocity,
            translated.bodies[0].linear_velocity
        );
        assert_eq!(
            original.bodies[1].linear_velocity,
            translated.bodies[1].linear_velocity
        );

        let swapped = swap_manifold_orientation(&scenario);
        let swapped_result = solve_authored_contact_pass(&swapped).unwrap();
        for (expected, actual) in original.bodies.iter().zip(&swapped_result.bodies) {
            assert_vec_close(expected.linear_velocity, actual.linear_velocity);
            assert_close(
                expected.angular_velocity.get(),
                actual.angular_velocity.get(),
            );
        }

        let rotated = rotate_scenario_quarter_turn(&scenario);
        let rotated_result = solve_authored_contact_pass(&rotated).unwrap();
        for (expected, actual) in translated.bodies.iter().zip(&rotated_result.bodies) {
            assert_vec_close(
                quarter_turn(expected.linear_velocity),
                actual.linear_velocity,
            );
            assert_close(
                expected.angular_velocity.get(),
                actual.angular_velocity.get(),
            );
        }
    }

    #[test]
    fn isolated_frictionless_contact_conserves_linear_and_angular_momentum() {
        let scenario = two_dynamic_scenario();
        let before_linear = linear_momentum(&scenario.bodies);
        let before_angular = angular_momentum(&scenario.bodies);
        let result = solve_authored_contact_pass(&scenario).unwrap();
        assert_vec_close(before_linear, linear_momentum(&result.bodies));
        assert_close(before_angular, angular_momentum(&result.bodies));
    }

    #[test]
    fn first_divergence_names_the_earliest_pass_value() {
        let result = solve_authored_contact_pass(&authored_one_contact_scenario(9)).unwrap();
        let mut actual = result.traces.clone();
        let value = &mut actual[2].values[0];
        value.value = finite(value.value.get() + 0.25);
        value.bits = value.value.get().to_bits();
        let divergence = first_trace_divergence(&result.traces, &actual, 1.0e-6, 1.0e-6)
            .expect("altered trace must diverge");
        assert_eq!(divergence.record_index, 2);
        assert_eq!(divergence.value_name, actual[2].values[0].name);
    }

    #[test]
    fn circle_and_rectangle_contacts_regenerate_through_exact_step_counts() {
        for kind in [
            AnalyticFixtureKind::CircleFloor,
            AnalyticFixtureKind::RectangleFloor,
        ] {
            let scenario = analytic_fixture_scenario(kind, 180).unwrap();
            let result = solve_analytic_fixture_steps(&scenario).unwrap();
            eprintln!(
                "{kind:?}: position={:?} velocity={:?} angle={} traces={}",
                result.bodies[0].position,
                result.bodies[0].linear_velocity,
                result.bodies[0].angle.get(),
                result.traces.len()
            );
            assert!((0.45..=0.55).contains(&result.bodies[0].position.y));
            assert!(result.bodies[0].linear_velocity.y.abs() <= 0.1);
            assert!(
                result
                    .traces
                    .iter()
                    .any(|trace| trace.stage == TraceStageV1::Prepare)
            );
            assert_eq!(
                result
                    .traces
                    .iter()
                    .filter(|trace| trace.stage == TraceStageV1::Integrate)
                    .count(),
                scenario.requested_steps as usize
                    * scenario.settings.substeps as usize
                    * scenario.bodies.len()
            );
            assert_eq!(
                result.traces.last().map(|trace| trace.step),
                Some(scenario.requested_steps - 1)
            );
            assert_eq!(result, solve_analytic_fixture_steps(&scenario).unwrap());
        }
    }

    #[test]
    fn sliding_friction_reduces_speed_and_sleeping_input_does_not_drift() {
        let mut sliding =
            analytic_fixture_scenario(AnalyticFixtureKind::RectangleFloor, 60).unwrap();
        sliding.bodies[0].position.y = 0.5;
        sliding.bodies[0].linear_velocity.x = 2.0;
        let result = solve_analytic_fixture_steps(&sliding).unwrap();
        eprintln!(
            "sliding: position={:?} velocity={:?} angle={}",
            result.bodies[0].position,
            result.bodies[0].linear_velocity,
            result.bodies[0].angle.get()
        );
        assert!(result.bodies[0].linear_velocity.x.abs() < 2.0);
        assert!(result.bodies[0].linear_velocity.x >= -0.05);

        let mut sleeping = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 20).unwrap();
        sleeping.bodies[0].position.y = 0.5;
        sleeping.bodies[0].sleeping = true;
        let result = solve_analytic_fixture_steps(&sleeping).unwrap();
        assert_eq!(result.bodies[0], sleeping.bodies[0]);
        assert!(
            result
                .traces
                .iter()
                .any(|trace| trace.stage == TraceStageV1::Prepare)
        );
    }

    #[test]
    fn fixture_stepper_rejects_authored_manifolds_and_actions() {
        let mut authored = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        authored.manifolds = authored_one_contact_scenario(1).manifolds;
        assert!(
            solve_analytic_fixture_steps(&authored)
                .unwrap_err()
                .0
                .contains("empty authored manifold")
        );

        let mut action = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        action
            .actions
            .push(physbox_contracts::NeutralActionV1::SetGravity {
                step: 0,
                gravity: Vec2::ZERO,
            });
        assert!(
            solve_analytic_fixture_steps(&action)
                .unwrap_err()
                .0
                .contains("do not support actions")
        );
    }

    fn two_dynamic_scenario() -> ScenarioV1 {
        let mut scenario = authored_one_contact_scenario(3);
        scenario.bodies[0].shape = ShapeV1::Circle {
            radius: finite(0.5),
        };
        scenario.bodies[0].position = Vec2::new(-0.5, 0.0);
        scenario.bodies[0].linear_velocity = Vec2::new(2.0, 0.0);
        scenario.bodies[0].inverse_mass = finite(0.5);
        scenario.bodies[0].inverse_inertia = finite(1.0);
        scenario.bodies[1].shape = ShapeV1::Circle {
            radius: finite(0.5),
        };
        scenario.bodies[1].position = Vec2::new(0.5, 0.0);
        scenario.bodies[1].linear_velocity = Vec2::new(-1.0, 0.0);
        scenario.bodies[1].inverse_mass = finite(1.0);
        scenario.bodies[1].inverse_inertia = finite(2.0);
        let material = MaterialV1 {
            friction: finite(0.0),
            restitution: finite(0.0),
        };
        scenario.manifolds = vec![AuthoredManifoldV1 {
            id: 0,
            body_a: scenario.bodies[0].id,
            body_b: scenario.bodies[1].id,
            normal: Vec2::new(1.0, 0.0),
            material,
            points: vec![AuthoredContactPointV1 {
                local_anchor_a: Vec2::new(0.5, 0.0),
                local_anchor_b: Vec2::new(-0.5, 0.0),
                world_point: Vec2::ZERO,
                separation: finite(0.0),
                feature_a: FeatureIdV1 {
                    feature_type: 0,
                    feature_index: 0,
                },
                feature_b: FeatureIdV1 {
                    feature_type: 0,
                    feature_index: 0,
                },
            }],
        }];
        scenario.contact_cache.clear();
        scenario
    }

    fn swap_manifold_orientation(scenario: &ScenarioV1) -> ScenarioV1 {
        let mut swapped = scenario.clone();
        let manifold = &mut swapped.manifolds[0];
        std::mem::swap(&mut manifold.body_a, &mut manifold.body_b);
        manifold.normal = scale(manifold.normal, -1.0);
        for point in &mut manifold.points {
            std::mem::swap(&mut point.local_anchor_a, &mut point.local_anchor_b);
            std::mem::swap(&mut point.feature_a, &mut point.feature_b);
        }
        swapped
    }

    fn rotate_scenario_quarter_turn(scenario: &ScenarioV1) -> ScenarioV1 {
        let mut rotated = scenario.clone();
        for body in &mut rotated.bodies {
            body.position = quarter_turn(body.position);
            body.angle = finite(body.angle.get() + std::f32::consts::FRAC_PI_2);
            body.linear_velocity = quarter_turn(body.linear_velocity);
        }
        for manifold in &mut rotated.manifolds {
            manifold.normal = quarter_turn(manifold.normal);
            for point in &mut manifold.points {
                point.world_point = quarter_turn(point.world_point);
            }
        }
        rotated
    }

    fn quarter_turn(vector: Vec2) -> Vec2 {
        Vec2::new(-vector.y, vector.x)
    }

    fn linear_momentum(bodies: &[BodyStateV1]) -> Vec2 {
        bodies.iter().fold(Vec2::ZERO, |sum, body| {
            add(
                sum,
                scale(body.linear_velocity, 1.0 / body.inverse_mass.get()),
            )
        })
    }

    fn angular_momentum(bodies: &[BodyStateV1]) -> f32 {
        bodies.iter().fold(0.0, |sum, body| {
            let mass = 1.0 / body.inverse_mass.get();
            let inertia = 1.0 / body.inverse_inertia.get();
            sum + inertia * body.angular_velocity.get()
                + cross(body.position, scale(body.linear_velocity, mass))
        })
    }

    fn trace_value(record: &TraceRecordV1, name: &str) -> f32 {
        record
            .values
            .iter()
            .find(|value| value.name == name)
            .unwrap()
            .value
            .get()
    }

    fn assert_vec_close(actual: Vec2, expected: Vec2) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 2.0e-5,
            "actual {actual} expected {expected}"
        );
    }
}
