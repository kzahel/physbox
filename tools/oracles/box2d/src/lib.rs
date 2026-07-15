use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyStateV1, FailureKindV1, FailureV1, FeatureIdV1,
    FiniteF32, MaterialV1, MetricsV1, NamedValueV1, OracleRequestV1, OracleResultV1, ProvenanceV1,
    SCHEMA_VERSION, ShapeV1, Validate, Vec2, WorkVectorV1, sha256_bytes,
};

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/box2d_bindings.rs"));
}

pub const BOX2D_REVISION: &str = env!("PHYSBOX5_BOX2D_REVISION");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleLane {
    World,
    Narrowphase,
}

impl OracleLane {
    pub fn parse(value: &str) -> Result<Self, AdapterError> {
        match value {
            "world" => Ok(Self::World),
            "narrowphase" => Ok(Self::Narrowphase),
            _ => Err(AdapterError::Invalid(format!(
                "unsupported Box2D oracle lane {value}"
            ))),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::Narrowphase => "narrowphase",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterError {
    Invalid(String),
    Unsupported(String),
    Numerical(String),
}

impl Display for AdapterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(formatter, "invalid request: {reason}"),
            Self::Unsupported(reason) => write!(formatter, "unsupported feature: {reason}"),
            Self::Numerical(reason) => write!(formatter, "non-finite Box2D result: {reason}"),
        }
    }
}

impl std::error::Error for AdapterError {}

pub fn execute_request(
    request: &OracleRequestV1,
    request_bytes: &[u8],
    lane: OracleLane,
) -> Result<OracleResultV1, AdapterError> {
    request
        .validate()
        .map_err(|error| AdapterError::Invalid(error.to_string()))?;
    validate_projection(request, lane)?;
    match lane {
        OracleLane::World => execute_world(request, request_bytes),
        OracleLane::Narrowphase => execute_narrowphase(request, request_bytes),
    }
}

pub fn unsupported_result(
    request: &OracleRequestV1,
    request_bytes: &[u8],
    lane: OracleLane,
    reason: String,
) -> OracleResultV1 {
    OracleResultV1 {
        schema_version: SCHEMA_VERSION,
        run_id: request.run_id.clone(),
        source_scenario_hash: request.source_scenario_hash.clone(),
        projection_hash: sha256_bytes(request_bytes),
        provenance: provenance(request, lane),
        bodies: request.scenario.bodies.clone(),
        manifolds: Vec::new(),
        traces: Vec::new(),
        metrics: empty_metrics(&request.scenario.bodies),
        failure: Some(FailureV1 {
            kind: FailureKindV1::UnsupportedFeature,
            reason,
            first_step: None,
            first_stage: None,
            reproducer_hash: Some(request.source_scenario_hash.clone()),
        }),
    }
}

fn validate_projection(request: &OracleRequestV1, lane: OracleLane) -> Result<(), AdapterError> {
    for body in &request.scenario.bodies {
        if body.inverse_mass.get() == 0.0 && body.inverse_inertia.get() != 0.0 {
            return Err(AdapterError::Unsupported(format!(
                "static body {:?} has nonzero inverse inertia",
                body.id
            )));
        }
        if matches!(lane, OracleLane::World)
            && body.inverse_mass.get() > 0.0
            && body.inverse_inertia.get() == 0.0
        {
            return Err(AdapterError::Unsupported(format!(
                "dynamic body {:?} requests locked rotation, which is not in the neutral projection",
                body.id
            )));
        }
    }
    if matches!(lane, OracleLane::World) {
        for action in &request.scenario.actions {
            let step = match action {
                physbox_contracts::NeutralActionV1::ApplyImpulse { step, .. }
                | physbox_contracts::NeutralActionV1::SetGravity { step, .. } => *step,
            };
            if step >= request.scenario.requested_steps {
                return Err(AdapterError::Unsupported(format!(
                    "action step {step} is outside requested step count {}",
                    request.scenario.requested_steps
                )));
            }
        }
    }
    Ok(())
}

fn execute_world(
    request: &OracleRequestV1,
    request_bytes: &[u8],
) -> Result<OracleResultV1, AdapterError> {
    // SAFETY: all definitions come from Box2D's public constructors, remain alive for each call,
    // and the world is exclusively owned by this process and destroyed by WorldGuard.
    unsafe {
        let mut world_def = ffi::b2DefaultWorldDef();
        world_def.gravity = raw_vec(request.scenario.settings.gravity);
        world_def.restitutionThreshold = request.scenario.settings.restitution_threshold.get();
        world_def.workerCount = 1;
        world_def.enqueueTask = None;
        world_def.finishTask = None;
        world_def.enableSleep = true;
        world_def.enableContinuous = true;
        world_def.enableContactSoftening = false;
        let world = WorldGuard(ffi::b2CreateWorld(&raw const world_def));
        let mut raw_bodies = Vec::with_capacity(request.scenario.bodies.len());
        for body in &request.scenario.bodies {
            let mut body_def = ffi::b2DefaultBodyDef();
            body_def.type_ = if body.inverse_mass.get() == 0.0 {
                ffi::b2BodyType_b2_staticBody
            } else {
                ffi::b2BodyType_b2_dynamicBody
            };
            body_def.position = raw_vec(body.position);
            body_def.rotation = raw_rotation(body.angle.get());
            body_def.linearVelocity = raw_vec(body.linear_velocity);
            body_def.angularVelocity = body.angular_velocity.get();
            body_def.isAwake = !body.sleeping;
            body_def.enableSleep = true;
            let raw_body = ffi::b2CreateBody(world.0, &raw const body_def);

            let mut shape_def = ffi::b2DefaultShapeDef();
            shape_def.material.friction = body.material.friction.get();
            shape_def.material.restitution = body.material.restitution.get();
            shape_def.material.rollingResistance = 0.0;
            shape_def.material.tangentSpeed = 0.0;
            shape_def.density = 1.0;
            match body.shape {
                ShapeV1::Circle { radius } => {
                    let circle = ffi::b2Circle {
                        center: ffi::b2Vec2 { x: 0.0, y: 0.0 },
                        radius: radius.get(),
                    };
                    ffi::b2CreateCircleShape(raw_body, &raw const shape_def, &raw const circle);
                }
                ShapeV1::Rectangle { half_extents } => {
                    let polygon = ffi::b2MakeBox(half_extents.x, half_extents.y);
                    ffi::b2CreatePolygonShape(raw_body, &raw const shape_def, &raw const polygon);
                }
            }
            if body.inverse_mass.get() > 0.0 {
                ffi::b2Body_SetMassData(
                    raw_body,
                    ffi::b2MassData {
                        mass: 1.0 / body.inverse_mass.get(),
                        center: ffi::b2Vec2 { x: 0.0, y: 0.0 },
                        rotationalInertia: 1.0 / body.inverse_inertia.get(),
                    },
                );
            }
            raw_bodies.push(raw_body);
        }

        for step in 0..request.scenario.requested_steps {
            for action in &request.scenario.actions {
                match action {
                    physbox_contracts::NeutralActionV1::ApplyImpulse {
                        step: action_step,
                        body,
                        impulse,
                        world_point,
                    } if *action_step == step => {
                        let index = request
                            .scenario
                            .bodies
                            .binary_search_by_key(body, |candidate| candidate.id)
                            .map_err(|_| {
                                AdapterError::Unsupported(format!(
                                    "impulse action references missing body {body:?}"
                                ))
                            })?;
                        ffi::b2Body_ApplyLinearImpulse(
                            raw_bodies[index],
                            raw_vec(*impulse),
                            raw_vec(*world_point),
                            true,
                        );
                    }
                    physbox_contracts::NeutralActionV1::SetGravity {
                        step: action_step,
                        gravity,
                    } if *action_step == step => {
                        ffi::b2World_SetGravity(world.0, raw_vec(*gravity));
                    }
                    _ => {}
                }
            }
            ffi::b2World_Step(
                world.0,
                request.scenario.settings.fixed_dt.get(),
                request.scenario.settings.substeps as i32,
            );
        }

        let mut bodies = request.scenario.bodies.clone();
        for (body, raw_body) in bodies.iter_mut().zip(&raw_bodies) {
            body.position = contract_vec(ffi::b2Body_GetPosition(*raw_body))?;
            let rotation = ffi::b2Body_GetRotation(*raw_body);
            body.angle = finite(ffi::b2Atan2(rotation.s, rotation.c), "body angle")?;
            body.linear_velocity = contract_vec(ffi::b2Body_GetLinearVelocity(*raw_body))?;
            body.angular_velocity = finite(
                ffi::b2Body_GetAngularVelocity(*raw_body),
                "body angular velocity",
            )?;
            body.sleeping = !ffi::b2Body_IsAwake(*raw_body);
        }
        let counters = ffi::b2World_GetCounters(world.0);
        let manifolds = public_manifolds(&bodies)?;
        let contact_points = manifolds.iter().map(|manifold| manifold.points.len()).sum();
        let work = work_vector(
            &bodies,
            counters.contactCount.max(0) as u32,
            contact_points,
            0,
            request.scenario.requested_steps,
            request.scenario.settings.substeps,
        );
        Ok(result(
            request,
            request_bytes,
            OracleLane::World,
            bodies,
            manifolds,
            work,
        ))
    }
}

fn execute_narrowphase(
    request: &OracleRequestV1,
    request_bytes: &[u8],
) -> Result<OracleResultV1, AdapterError> {
    if !request.scenario.actions.is_empty() {
        return Err(AdapterError::Unsupported(
            "narrowphase lane does not consume actions".to_owned(),
        ));
    }
    let manifolds = public_manifolds(&request.scenario.bodies)?;
    let contact_points = manifolds.iter().map(|manifold| manifold.points.len()).sum();
    let body_count = request.scenario.bodies.len() as u64;
    let work = work_vector(
        &request.scenario.bodies,
        manifolds.len() as u32,
        contact_points,
        body_count.saturating_mul(body_count.saturating_sub(1)) / 2,
        0,
        0,
    );
    Ok(result(
        request,
        request_bytes,
        OracleLane::Narrowphase,
        request.scenario.bodies.clone(),
        manifolds,
        work,
    ))
}

pub fn public_manifolds(bodies: &[BodyStateV1]) -> Result<Vec<AuthoredManifoldV1>, AdapterError> {
    let mut output = Vec::new();
    for index_a in 0..bodies.len() {
        for index_b in index_a + 1..bodies.len() {
            if let Some(manifold) = public_pair(&bodies[index_a], &bodies[index_b])? {
                output.push(AuthoredManifoldV1 {
                    id: output.len() as u32,
                    ..manifold
                });
            }
        }
    }
    Ok(output)
}

fn public_pair(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
) -> Result<Option<AuthoredManifoldV1>, AdapterError> {
    // SAFETY: shape and transform values are fully initialized public Box2D POD types.
    unsafe {
        let transform_a = raw_transform(body_a);
        let transform_b = raw_transform(body_b);
        let (raw, output_a, output_b) = match (body_a.shape, body_b.shape) {
            (ShapeV1::Circle { radius: radius_a }, ShapeV1::Circle { radius: radius_b }) => {
                let circle_a = raw_circle(radius_a.get());
                let circle_b = raw_circle(radius_b.get());
                (
                    ffi::b2CollideCircles(
                        &raw const circle_a,
                        transform_a,
                        &raw const circle_b,
                        transform_b,
                    ),
                    body_a,
                    body_b,
                )
            }
            (ShapeV1::Rectangle { half_extents }, ShapeV1::Circle { radius }) => {
                let polygon = ffi::b2MakeBox(half_extents.x, half_extents.y);
                let circle = raw_circle(radius.get());
                (
                    ffi::b2CollidePolygonAndCircle(
                        &raw const polygon,
                        transform_a,
                        &raw const circle,
                        transform_b,
                    ),
                    body_a,
                    body_b,
                )
            }
            (ShapeV1::Circle { radius }, ShapeV1::Rectangle { half_extents }) => {
                let polygon = ffi::b2MakeBox(half_extents.x, half_extents.y);
                let circle = raw_circle(radius.get());
                (
                    ffi::b2CollidePolygonAndCircle(
                        &raw const polygon,
                        transform_b,
                        &raw const circle,
                        transform_a,
                    ),
                    body_b,
                    body_a,
                )
            }
            (
                ShapeV1::Rectangle {
                    half_extents: half_a,
                },
                ShapeV1::Rectangle {
                    half_extents: half_b,
                },
            ) => {
                let polygon_a = ffi::b2MakeBox(half_a.x, half_a.y);
                let polygon_b = ffi::b2MakeBox(half_b.x, half_b.y);
                (
                    ffi::b2CollidePolygons(
                        &raw const polygon_a,
                        transform_a,
                        &raw const polygon_b,
                        transform_b,
                    ),
                    body_a,
                    body_b,
                )
            }
        };
        convert_manifold(raw, output_a, output_b)
    }
}

fn convert_manifold(
    raw: ffi::b2Manifold,
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
) -> Result<Option<AuthoredManifoldV1>, AdapterError> {
    if raw.pointCount == 0 {
        return Ok(None);
    }
    if !(1..=2).contains(&raw.pointCount) {
        return Err(AdapterError::Numerical(format!(
            "Box2D returned invalid manifold point count {}",
            raw.pointCount
        )));
    }
    let mut points = Vec::with_capacity(raw.pointCount as usize);
    for point in &raw.points[..raw.pointCount as usize] {
        points.push(AuthoredContactPointV1 {
            local_anchor_a: inverse_rotate(contract_vec(point.anchorA)?, body_a.angle.get()),
            local_anchor_b: inverse_rotate(contract_vec(point.anchorB)?, body_b.angle.get()),
            world_point: contract_vec(point.point)?,
            separation: finite(point.separation, "manifold separation")?,
            feature_a: FeatureIdV1 {
                feature_type: 3,
                feature_index: u32::from(point.id >> 8),
            },
            feature_b: FeatureIdV1 {
                feature_type: 3,
                feature_index: u32::from(point.id & 0xff),
            },
        });
    }
    Ok(Some(AuthoredManifoldV1 {
        id: 0,
        body_a: body_a.id,
        body_b: body_b.id,
        normal: contract_vec(raw.normal)?,
        material: mix_material(body_a.material, body_b.material)?,
        points,
    }))
}

fn result(
    request: &OracleRequestV1,
    request_bytes: &[u8],
    lane: OracleLane,
    bodies: Vec<BodyStateV1>,
    manifolds: Vec<AuthoredManifoldV1>,
    work: WorkVectorV1,
) -> OracleResultV1 {
    OracleResultV1 {
        schema_version: SCHEMA_VERSION,
        run_id: request.run_id.clone(),
        source_scenario_hash: request.source_scenario_hash.clone(),
        projection_hash: sha256_bytes(request_bytes),
        provenance: provenance(request, lane),
        bodies,
        manifolds,
        traces: Vec::new(),
        metrics: MetricsV1 {
            work,
            total_physics: None,
            cpu_encode_submit: None,
            capacities: Vec::new(),
            penetration_max: finite_infallible(0.0),
            jitter_rms: finite_infallible(0.0),
            energy_delta: finite_infallible(0.0),
        },
        failure: None,
    }
}

fn provenance(request: &OracleRequestV1, lane: OracleLane) -> ProvenanceV1 {
    ProvenanceV1 {
        run_id: request.run_id.clone(),
        source_revision: BOX2D_REVISION.to_owned(),
        source_dirty: false,
        source_patch_digest: None,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        adapter: "box2d-v3-authority".to_owned(),
        backend: "native-cpu".to_owned(),
        driver: std::env::consts::ARCH.to_owned(),
        toolchain: format!(
            "{}; {}; {}",
            env!("RUSTC_VERSION"),
            env!("CMAKE_VERSION"),
            env!("C_COMPILER_VERSION")
        ),
        build_options: vec![
            named("box2d_disable_simd", "on"),
            named("box2d_validate", "on"),
            named("build_shared_libs", "off"),
            named("build_type", "release"),
            named("ffp_contract", "off"),
        ],
        runtime_settings: vec![
            named(
                "candidate_bias_rate",
                &format!(
                    "{} (source-only; not applied by Box2D)",
                    request.scenario.settings.bias_rate.get()
                ),
            ),
            named(
                "candidate_impulse_coefficient",
                &format!(
                    "{} (source-only; not applied by Box2D)",
                    request.scenario.settings.impulse_coefficient.get()
                ),
            ),
            named(
                "candidate_iterations",
                &format!(
                    "{} (source-only; not applied by Box2D)",
                    request.scenario.settings.iterations
                ),
            ),
            named(
                "candidate_mass_coefficient",
                &format!(
                    "{} (source-only; not applied by Box2D)",
                    request.scenario.settings.mass_coefficient.get()
                ),
            ),
            named(
                "candidate_max_depenetration_velocity",
                &request.scenario.settings.contact_policy.map_or_else(
                    || "disabled".to_owned(),
                    |policy| {
                        format!(
                            "{} (source policy; Box2D contact_speed is configured independently)",
                            policy.max_depenetration_velocity.get()
                        )
                    },
                ),
            ),
            named(
                "candidate_speculative_distance",
                &request.scenario.settings.contact_policy.map_or_else(
                    || "disabled".to_owned(),
                    |policy| {
                        format!(
                            "{} (source-only; Box2D owns its internal speculative policy)",
                            policy.speculative_distance.get()
                        )
                    },
                ),
            ),
            named("contact_damping_ratio", "10"),
            named("contact_hertz", "30"),
            named("contact_softening", "false"),
            named("contact_speed", "3"),
            named("continuous", "true"),
            named("lane", lane.name()),
            named("narrowphase_transform", "engine-authored-sin-cos"),
            named(
                "restitution_threshold",
                &request
                    .scenario
                    .settings
                    .restitution_threshold
                    .get()
                    .to_string(),
            ),
            named("sleep", "true"),
            named("substeps", &request.scenario.settings.substeps.to_string()),
            named("worker_count", "1"),
        ],
    }
}

fn work_vector(
    bodies: &[BodyStateV1],
    contact_count: u32,
    contact_points: usize,
    candidate_pairs: u64,
    steps: u32,
    substeps: u32,
) -> WorkVectorV1 {
    WorkVectorV1 {
        bodies_total: bodies.len() as u32,
        bodies_awake: bodies.iter().filter(|body| !body.sleeping).count() as u32,
        circles: bodies
            .iter()
            .filter(|body| matches!(body.shape, ShapeV1::Circle { .. }))
            .count() as u32,
        rectangles: bodies
            .iter()
            .filter(|body| matches!(body.shape, ShapeV1::Rectangle { .. }))
            .count() as u32,
        candidate_pairs,
        overlapping_pairs: u64::from(contact_count),
        manifolds: contact_count,
        contact_points: contact_points as u32,
        solver_work: u64::from(contact_count) * u64::from(steps) * u64::from(substeps),
        dispatches: 0,
    }
}

fn empty_metrics(bodies: &[BodyStateV1]) -> MetricsV1 {
    MetricsV1 {
        work: work_vector(bodies, 0, 0, 0, 0, 0),
        total_physics: None,
        cpu_encode_submit: None,
        capacities: Vec::new(),
        penetration_max: finite_infallible(0.0),
        jitter_rms: finite_infallible(0.0),
        energy_delta: finite_infallible(0.0),
    }
}

fn raw_vec(value: Vec2) -> ffi::b2Vec2 {
    ffi::b2Vec2 {
        x: value.x,
        y: value.y,
    }
}

fn contract_vec(value: ffi::b2Vec2) -> Result<Vec2, AdapterError> {
    let output = Vec2::new(value.x, value.y);
    output
        .validate()
        .map_err(|_| AdapterError::Numerical("vector".to_owned()))?;
    Ok(output)
}

fn raw_rotation(angle: f32) -> ffi::b2Rot {
    // SAFETY: b2ComputeCosSin accepts every finite angle admitted by the schema.
    let pair = unsafe { ffi::b2ComputeCosSin(angle) };
    ffi::b2Rot {
        c: pair.cosine,
        s: pair.sine,
    }
}

fn raw_transform(body: &BodyStateV1) -> ffi::b2Transform {
    // The direct public-collision lane receives the engine-authored geometric transform.
    // Box2D's world/body creation path intentionally keeps `raw_rotation`, which uses
    // b2ComputeCosSin for Box2D's own deterministic angle integration semantics.
    let (sine, cosine) = body.angle.get().sin_cos();
    ffi::b2Transform {
        p: raw_vec(body.position),
        q: ffi::b2Rot { c: cosine, s: sine },
    }
}

fn raw_circle(radius: f32) -> ffi::b2Circle {
    ffi::b2Circle {
        center: ffi::b2Vec2 { x: 0.0, y: 0.0 },
        radius,
    }
}

fn inverse_rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = (-angle).sin_cos();
    Vec2::new(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

fn mix_material(a: MaterialV1, b: MaterialV1) -> Result<MaterialV1, AdapterError> {
    Ok(MaterialV1 {
        friction: finite(
            (a.friction.get() * b.friction.get()).sqrt(),
            "mixed friction",
        )?,
        restitution: finite(
            a.restitution.get().max(b.restitution.get()),
            "mixed restitution",
        )?,
    })
}

fn finite(value: f32, name: &str) -> Result<FiniteF32, AdapterError> {
    FiniteF32::new(value).map_err(|_| AdapterError::Numerical(name.to_owned()))
}

fn finite_infallible(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("constant is finite")
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

struct WorldGuard(ffi::b2WorldId);

impl Drop for WorldGuard {
    fn drop(&mut self) {
        // SAFETY: the guard uniquely owns this live world and destroys it exactly once.
        unsafe { ffi::b2DestroyWorld(self.0) }
    }
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{OracleRequestV1, SCHEMA_VERSION, canonical_hash, canonical_json};
    use physbox_engine::{
        AnalyticFixtureKind, P5_2CandidatePattern, P5_2CollisionScenarioKind,
        analytic_fixture_scenario, p5_2_candidate_scenario, p5_2_collision_scenario,
    };
    use physbox_lab::{
        analytic_narrowphase::{compare_box2d_narrowphase, generate_scalar_narrowphase},
        collision_candidates::enumerate_scalar_candidates,
        fixture_contacts::generate_fixture_contacts,
    };

    use super::*;

    #[test]
    fn public_collision_functions_match_scoped_fixture_topology_and_geometry() {
        for kind in [
            AnalyticFixtureKind::CircleFloor,
            AnalyticFixtureKind::RectangleFloor,
            AnalyticFixtureKind::BoxOnBox,
        ] {
            let mut scenario = analytic_fixture_scenario(kind, 1).unwrap();
            if kind != AnalyticFixtureKind::BoxOnBox {
                scenario.bodies[0].position.y = 0.49;
            }
            let expected = generate_fixture_contacts(&scenario.bodies).unwrap();
            let actual = public_manifolds(&scenario.bodies).unwrap();
            assert_eq!(actual.len(), expected.len(), "{kind:?}");
            let actual = orient_like(&actual[0], &expected[0]);
            assert_eq!(actual.points.len(), expected[0].points.len(), "{kind:?}");
            assert_vec_close(actual.normal, expected[0].normal, 2.0e-5);
            let mut actual_points = actual.points.clone();
            let mut expected_points = expected[0].points.clone();
            actual_points.sort_by(|a, b| a.world_point.x.total_cmp(&b.world_point.x));
            expected_points.sort_by(|a, b| a.world_point.x.total_cmp(&b.world_point.x));
            for (actual, expected) in actual_points.iter().zip(&expected_points) {
                assert_close(actual.separation.get(), expected.separation.get(), 2.0e-5);
                assert_vec_close(actual.world_point, expected.world_point, 2.0e-5);
            }
        }
    }

    #[test]
    fn public_collision_functions_match_p5_2_scalar_topology_and_geometry() {
        for kind in P5_2CollisionScenarioKind::ALL {
            let scenario = p5_2_collision_scenario(kind, 1, 17).unwrap();
            let candidates = enumerate_scalar_candidates(&scenario).unwrap();
            let expected = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
            let actual = public_manifolds(&scenario.bodies).unwrap();
            assert_eq!(actual.len(), expected.len(), "{kind:?}");
            for expected in expected {
                let actual = actual
                    .iter()
                    .find(|actual| {
                        (actual.body_a == expected.pair.body_a
                            && actual.body_b == expected.pair.body_b)
                            || (actual.body_a == expected.pair.body_b
                                && actual.body_b == expected.pair.body_a)
                    })
                    .unwrap_or_else(|| panic!("missing authority pair {:?}", expected.pair));
                let expected_orientation = AuthoredManifoldV1 {
                    id: 0,
                    body_a: expected.pair.body_a,
                    body_b: expected.pair.body_b,
                    normal: expected.normal,
                    material: expected.material,
                    points: Vec::new(),
                };
                let actual = orient_like(actual, &expected_orientation);
                assert_eq!(actual.points.len(), expected.points.len(), "{kind:?}");
                assert_vec_close(actual.normal, expected.normal, 2.0e-5);
                let mut actual_points = actual.points.clone();
                let mut expected_points = expected.points.clone();
                actual_points.sort_by(|a, b| {
                    a.world_point
                        .x
                        .total_cmp(&b.world_point.x)
                        .then_with(|| a.world_point.y.total_cmp(&b.world_point.y))
                });
                expected_points.sort_by(|a, b| {
                    a.world_point
                        .x
                        .total_cmp(&b.world_point.x)
                        .then_with(|| a.world_point.y.total_cmp(&b.world_point.y))
                });
                for (actual, expected) in actual_points.iter().zip(&expected_points) {
                    assert_close(actual.separation.get(), expected.separation.get(), 2.0e-5);
                    assert_vec_close(actual.world_point, expected.world_point, 2.0e-5);
                }
            }
        }
    }

    #[test]
    fn randomized_non_degenerate_manifolds_match_public_box2d_functions() {
        let mut differences = Vec::new();
        for seed in 0..16_u64 {
            for pattern in [
                P5_2CandidatePattern::Random,
                P5_2CandidatePattern::Touching,
                P5_2CandidatePattern::RotatedThin,
            ] {
                let scenario =
                    p5_2_candidate_scenario(pattern, 2 + (seed % 15) as u32, seed).unwrap();
                let candidates = enumerate_scalar_candidates(&scenario).unwrap();
                let expected =
                    generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
                let mut authority = public_manifolds(&scenario.bodies).unwrap();
                authority.retain(|manifold| {
                    manifold
                        .points
                        .iter()
                        .any(|point| point.separation.get() <= 1.0e-6)
                });
                if let Err(difference) =
                    compare_box2d_narrowphase(&candidates.candidates, &expected, &authority)
                {
                    differences.push(format!(
                        "{} seed {seed}: {}: {}",
                        pattern.name(),
                        difference.signature,
                        difference.reason
                    ));
                }
            }
        }
        assert!(differences.is_empty(), "{differences:#?}");
    }

    #[test]
    fn deterministic_world_lane_settles_shared_circle_fixture() {
        let scenario = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 180).unwrap();
        let request = request("box2d-world", scenario);
        let bytes = canonical_json(&request).unwrap();
        let result = execute_request(&request, &bytes, OracleLane::World).unwrap();
        eprintln!(
            "Box2D circle final position={:?} velocity={:?} sleeping={}",
            result.bodies[0].position, result.bodies[0].linear_velocity, result.bodies[0].sleeping
        );
        assert!((0.49..=0.51).contains(&result.bodies[0].position.y));
        assert!(result.bodies[0].linear_velocity.y.abs() <= 0.02);
        assert_eq!(result.provenance.source_revision, BOX2D_REVISION);
        result.validate().unwrap();

        let replay = execute_request(&request, &bytes, OracleLane::World).unwrap();
        assert_eq!(result.bodies, replay.bodies);
        assert_eq!(result.manifolds, replay.manifolds);
    }

    #[test]
    fn unsupported_projection_becomes_structured_failure_data() {
        let mut scenario = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        scenario.bodies[0].inverse_inertia = finite_infallible(0.0);
        let request = request("unsupported", scenario);
        let bytes = canonical_json(&request).unwrap();
        let error = execute_request(&request, &bytes, OracleLane::World).unwrap_err();
        let AdapterError::Unsupported(reason) = error else {
            panic!("expected unsupported error")
        };
        let result = unsupported_result(&request, &bytes, OracleLane::World, reason);
        assert_eq!(
            result.failure.as_ref().map(|failure| &failure.kind),
            Some(&FailureKindV1::UnsupportedFeature)
        );
        result.validate().unwrap();
    }

    fn request(run_id: &str, scenario: physbox_contracts::ScenarioV1) -> OracleRequestV1 {
        OracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            scenario,
        }
    }

    fn orient_like(
        actual: &AuthoredManifoldV1,
        expected: &AuthoredManifoldV1,
    ) -> AuthoredManifoldV1 {
        if actual.body_a == expected.body_a && actual.body_b == expected.body_b {
            return actual.clone();
        }
        assert_eq!(actual.body_a, expected.body_b);
        assert_eq!(actual.body_b, expected.body_a);
        let mut oriented = actual.clone();
        std::mem::swap(&mut oriented.body_a, &mut oriented.body_b);
        oriented.normal.x = -oriented.normal.x;
        oriented.normal.y = -oriented.normal.y;
        for point in &mut oriented.points {
            std::mem::swap(&mut point.local_anchor_a, &mut point.local_anchor_b);
            std::mem::swap(&mut point.feature_a, &mut point.feature_b);
        }
        oriented
    }

    fn assert_vec_close(actual: Vec2, expected: Vec2, tolerance: f32) {
        assert_close(actual.x, expected.x, tolerance);
        assert_close(actual.y, expected.y, tolerance);
    }

    fn assert_close(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "actual {actual} expected {expected} tolerance {tolerance}"
        );
    }
}
