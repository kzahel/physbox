use physbox_contracts::{
    AuthoredManifoldV1, BodyStateV1, CandidatePairV1, CollisionContactPointV1, CollisionManifoldV1,
    ContactIdentityPolicyV1, FeatureIdV1, FiniteF32, GeometricFeatureKindV1, GeometricFeatureV1,
    MaterialV1, ScenarioV1, ShapeV1, Validate, Vec2,
};
use serde::Serialize;

const NORMAL_EPSILON_SQUARED: f32 = 1.0e-12;
const REFERENCE_AXIS_RELATIVE_BIAS: f32 = 0.98;
const REFERENCE_AXIS_ABSOLUTE_BIAS: f32 = 1.0e-3;
pub const NARROWPHASE_ABSOLUTE_TOLERANCE: f32 = 2.0e-5;

pub fn select_rectangle_reference_is_a(
    separation_a: f32,
    separation_b: f32,
    policy: ContactIdentityPolicyV1,
    previous_reference_is_a: Option<bool>,
) -> bool {
    let direct = separation_a >= separation_b;
    let relative_bias =
        separation_b <= REFERENCE_AXIS_RELATIVE_BIAS * separation_a + REFERENCE_AXIS_ABSOLUTE_BIAS;
    let absolute_band =
        direct || (separation_a - separation_b).abs() <= REFERENCE_AXIS_ABSOLUTE_BIAS;
    match policy {
        ContactIdentityPolicyV1::DirectMaximumSeparation => direct,
        ContactIdentityPolicyV1::RelativeCoherenceBias => relative_bias,
        ContactIdentityPolicyV1::CanonicalAbsoluteBand => absolute_band,
        ContactIdentityPolicyV1::PriorContactHysteresis => match previous_reference_is_a {
            Some(true) if separation_b <= separation_a + REFERENCE_AXIS_ABSOLUTE_BIAS => true,
            Some(false) if separation_a <= separation_b + REFERENCE_AXIS_ABSOLUTE_BIAS => false,
            _ => direct,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrowphaseDifference {
    pub signature: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityFeatureCorrespondence {
    pub pair: CandidatePairV1,
    pub authority_feature_a: FeatureIdV1,
    pub authority_feature_b: FeatureIdV1,
    pub product_feature_a: GeometricFeatureV1,
    pub product_feature_b: GeometricFeatureV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityNarrowphaseComparison {
    pub feature_correspondence: Vec<AuthorityFeatureCorrespondence>,
    pub maximum_normal_absolute_error: f32,
    pub maximum_world_point_absolute_error: f32,
    pub maximum_separation_absolute_error: f32,
}

pub fn compare_box2d_narrowphase(
    candidates: &[CandidatePairV1],
    expected: &[CollisionManifoldV1],
    authority: &[AuthoredManifoldV1],
) -> Result<Vec<AuthorityFeatureCorrespondence>, NarrowphaseDifference> {
    Ok(compare_box2d_narrowphase_detailed(candidates, expected, authority)?.feature_correspondence)
}

pub fn compare_box2d_narrowphase_detailed(
    candidates: &[CandidatePairV1],
    expected: &[CollisionManifoldV1],
    authority: &[AuthoredManifoldV1],
) -> Result<AuthorityNarrowphaseComparison, NarrowphaseDifference> {
    let mut oriented = authority
        .iter()
        .filter_map(|manifold| {
            let (pair, swapped) = if manifold.body_a < manifold.body_b {
                (
                    CandidatePairV1 {
                        body_a: manifold.body_a,
                        body_b: manifold.body_b,
                    },
                    false,
                )
            } else {
                (
                    CandidatePairV1 {
                        body_a: manifold.body_b,
                        body_b: manifold.body_a,
                    },
                    true,
                )
            };
            candidates.binary_search(&pair).ok().map(|_| {
                let mut manifold = manifold.clone();
                if swapped {
                    std::mem::swap(&mut manifold.body_a, &mut manifold.body_b);
                    manifold.normal = scale(manifold.normal, -1.0);
                    for point in &mut manifold.points {
                        std::mem::swap(&mut point.local_anchor_a, &mut point.local_anchor_b);
                        std::mem::swap(&mut point.feature_a, &mut point.feature_b);
                    }
                }
                manifold
            })
        })
        .collect::<Vec<_>>();
    oriented.sort_by_key(|manifold| (manifold.body_a, manifold.body_b));
    if oriented.len() != expected.len() {
        return Err(difference(
            "authority:manifold-count",
            format!(
                "scalar emitted {} candidate manifolds and Box2D emitted {}",
                expected.len(),
                oriented.len()
            ),
        ));
    }
    let mut correspondence = Vec::new();
    let mut maximum_normal_absolute_error = 0.0_f32;
    let mut maximum_world_point_absolute_error = 0.0_f32;
    let mut maximum_separation_absolute_error = 0.0_f32;
    for (manifold_index, (expected, authority)) in expected.iter().zip(&oriented).enumerate() {
        if authority.body_a != expected.pair.body_a || authority.body_b != expected.pair.body_b {
            return Err(difference(
                format!("authority:manifold:{manifold_index}:pair"),
                "Box2D and scalar manifold pairs differ",
            ));
        }
        if authority.points.len() != expected.points.len() {
            return Err(difference(
                format!("authority:manifold:{manifold_index}:point-count"),
                format!(
                    "scalar emitted {} points and Box2D emitted {}",
                    expected.points.len(),
                    authority.points.len()
                ),
            ));
        }
        compare_vec(
            &format!("authority:manifold:{manifold_index}:normal"),
            expected.normal,
            authority.normal,
        )?;
        maximum_normal_absolute_error = maximum_normal_absolute_error
            .max((expected.normal.x - authority.normal.x).abs())
            .max((expected.normal.y - authority.normal.y).abs());
        let mut authority_points = authority.points.clone();
        let mut expected_points = expected.points.clone();
        authority_points.sort_by(|left, right| {
            left.world_point
                .x
                .total_cmp(&right.world_point.x)
                .then_with(|| left.world_point.y.total_cmp(&right.world_point.y))
        });
        expected_points.sort_by(|left, right| {
            left.world_point
                .x
                .total_cmp(&right.world_point.x)
                .then_with(|| left.world_point.y.total_cmp(&right.world_point.y))
        });
        for (point_index, (expected_point, authority_point)) in
            expected_points.iter().zip(&authority_points).enumerate()
        {
            compare_vec(
                &format!("authority:manifold:{manifold_index}:point:{point_index}:world-point"),
                expected_point.world_point,
                authority_point.world_point,
            )?;
            compare_value(
                &format!("authority:manifold:{manifold_index}:point:{point_index}:separation"),
                expected_point.separation.get(),
                authority_point.separation.get(),
            )?;
            maximum_world_point_absolute_error = maximum_world_point_absolute_error
                .max((expected_point.world_point.x - authority_point.world_point.x).abs())
                .max((expected_point.world_point.y - authority_point.world_point.y).abs());
            maximum_separation_absolute_error = maximum_separation_absolute_error
                .max((expected_point.separation.get() - authority_point.separation.get()).abs());
            correspondence.push(AuthorityFeatureCorrespondence {
                pair: expected.pair,
                authority_feature_a: authority_point.feature_a,
                authority_feature_b: authority_point.feature_b,
                product_feature_a: expected_point.feature_a,
                product_feature_b: expected_point.feature_b,
            });
        }
    }
    Ok(AuthorityNarrowphaseComparison {
        feature_correspondence: correspondence,
        maximum_normal_absolute_error,
        maximum_world_point_absolute_error,
        maximum_separation_absolute_error,
    })
}

pub fn compare_narrowphase(
    expected: &[CollisionManifoldV1],
    actual: &[CollisionManifoldV1],
) -> Result<(), NarrowphaseDifference> {
    if expected.len() != actual.len() {
        return Err(difference(
            "manifold-count",
            format!(
                "scalar emitted {} manifolds and GPU emitted {}",
                expected.len(),
                actual.len()
            ),
        ));
    }
    for (manifold_index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        if expected.pair != actual.pair {
            return Err(difference(
                format!("manifold:{manifold_index}:pair"),
                format!(
                    "scalar pair {:?} differs from GPU pair {:?}",
                    expected.pair, actual.pair
                ),
            ));
        }
        if expected.points.len() != actual.points.len() {
            return Err(difference(
                format!("manifold:{manifold_index}:point-count"),
                format!(
                    "scalar emitted {} points and GPU emitted {}",
                    expected.points.len(),
                    actual.points.len()
                ),
            ));
        }
        compare_vec(
            &format!("manifold:{manifold_index}:normal"),
            expected.normal,
            actual.normal,
        )
        .map_err(|mut difference| {
            difference.reason = format!("pair {:?}: {}", expected.pair, difference.reason);
            difference
        })?;
        compare_value(
            &format!("manifold:{manifold_index}:friction"),
            expected.material.friction.get(),
            actual.material.friction.get(),
        )?;
        compare_value(
            &format!("manifold:{manifold_index}:restitution"),
            expected.material.restitution.get(),
            actual.material.restitution.get(),
        )?;
        for (point_index, (expected, actual)) in
            expected.points.iter().zip(&actual.points).enumerate()
        {
            if expected.feature_a != actual.feature_a || expected.feature_b != actual.feature_b {
                return Err(difference(
                    format!("manifold:{manifold_index}:point:{point_index}:features"),
                    format!(
                        "scalar features {:?}/{:?} differ from GPU {:?}/{:?}",
                        expected.feature_a, expected.feature_b, actual.feature_a, actual.feature_b
                    ),
                ));
            }
            compare_vec(
                &format!("manifold:{manifold_index}:point:{point_index}:local-anchor-a"),
                expected.local_anchor_a,
                actual.local_anchor_a,
            )?;
            compare_vec(
                &format!("manifold:{manifold_index}:point:{point_index}:local-anchor-b"),
                expected.local_anchor_b,
                actual.local_anchor_b,
            )?;
            compare_vec(
                &format!("manifold:{manifold_index}:point:{point_index}:world-point"),
                expected.world_point,
                actual.world_point,
            )?;
            compare_value(
                &format!("manifold:{manifold_index}:point:{point_index}:separation"),
                expected.separation.get(),
                actual.separation.get(),
            )?;
        }
    }
    Ok(())
}

fn compare_vec(signature: &str, expected: Vec2, actual: Vec2) -> Result<(), NarrowphaseDifference> {
    compare_value(&format!("{signature}.x"), expected.x, actual.x)?;
    compare_value(&format!("{signature}.y"), expected.y, actual.y)
}

fn compare_value(signature: &str, expected: f32, actual: f32) -> Result<(), NarrowphaseDifference> {
    if (expected - actual).abs() > NARROWPHASE_ABSOLUTE_TOLERANCE {
        Err(difference(
            signature,
            format!("scalar value {expected} differs from GPU value {actual}"),
        ))
    } else {
        Ok(())
    }
}

fn difference(signature: impl Into<String>, reason: impl Into<String>) -> NarrowphaseDifference {
    NarrowphaseDifference {
        signature: signature.into(),
        reason: reason.into(),
    }
}

pub fn generate_scalar_narrowphase(
    scenario: &ScenarioV1,
    candidates: &[CandidatePairV1],
) -> Result<Vec<CollisionManifoldV1>, String> {
    generate_scalar_narrowphase_with_identity_policy(
        scenario,
        candidates,
        ContactIdentityPolicyV1::SELECTED,
    )
}

pub fn generate_scalar_narrowphase_with_identity_policy(
    scenario: &ScenarioV1,
    candidates: &[CandidatePairV1],
    identity_policy: ContactIdentityPolicyV1,
) -> Result<Vec<CollisionManifoldV1>, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    let speculative_distance = scenario
        .settings
        .contact_policy
        .map_or(0.0, |policy| policy.speculative_distance.get());
    let mut manifolds = Vec::new();
    for pair in candidates {
        pair.validate().map_err(|error| error.to_string())?;
        let body_a = body(scenario, pair.body_a)?;
        let body_b = body(scenario, pair.body_b)?;
        let manifold = match (body_a.shape, body_b.shape) {
            (ShapeV1::Circle { radius: radius_a }, ShapeV1::Circle { radius: radius_b }) => {
                collide_circles(
                    *pair,
                    body_a,
                    radius_a.get(),
                    body_b,
                    radius_b.get(),
                    speculative_distance,
                )?
            }
            (ShapeV1::Circle { radius }, ShapeV1::Rectangle { half_extents }) => {
                collide_circle_rectangle(
                    *pair,
                    body_a,
                    radius.get(),
                    body_b,
                    half_extents,
                    speculative_distance,
                )?
            }
            (ShapeV1::Rectangle { half_extents }, ShapeV1::Circle { radius }) => {
                collide_rectangle_circle(
                    *pair,
                    body_a,
                    half_extents,
                    body_b,
                    radius.get(),
                    speculative_distance,
                )?
            }
            (
                ShapeV1::Rectangle {
                    half_extents: half_a,
                },
                ShapeV1::Rectangle {
                    half_extents: half_b,
                },
            ) => collide_rectangles(
                *pair,
                body_a,
                half_a,
                body_b,
                half_b,
                speculative_distance,
                identity_policy,
            )?,
        };
        if let Some(mut manifold) = manifold {
            manifold
                .points
                .sort_by_key(|point| point.key(manifold.pair));
            manifold
                .points
                .dedup_by_key(|point| point.key(manifold.pair));
            manifold.validate().map_err(|error| error.to_string())?;
            manifolds.push(manifold);
        }
    }
    Ok(manifolds)
}

pub fn authored_solver_projection(
    source: &ScenarioV1,
    manifolds: &[CollisionManifoldV1],
) -> Result<ScenarioV1, String> {
    source.validate().map_err(|error| error.to_string())?;
    if !source.manifolds.is_empty()
        || !source.contact_cache.is_empty()
        || !source.actions.is_empty()
    {
        return Err("authored solver projection requires a neutral source scenario".to_owned());
    }
    let mut projection = source.clone();
    projection.id = format!("{}-authored-solver-projection", source.id);
    projection.requested_steps = 1;
    projection.manifolds = manifolds
        .iter()
        .enumerate()
        .map(|(id, manifold)| AuthoredManifoldV1 {
            id: id as u32,
            body_a: manifold.pair.body_a,
            body_b: manifold.pair.body_b,
            normal: manifold.normal,
            material: manifold.material,
            points: manifold
                .points
                .iter()
                .map(|point| physbox_contracts::AuthoredContactPointV1 {
                    local_anchor_a: point.local_anchor_a,
                    local_anchor_b: point.local_anchor_b,
                    world_point: point.world_point,
                    separation: point.separation,
                    feature_a: authored_feature(point.feature_a),
                    feature_b: authored_feature(point.feature_b),
                })
                .collect(),
        })
        .collect();
    projection.validate().map_err(|error| error.to_string())?;
    Ok(projection)
}

fn authored_feature(feature: GeometricFeatureV1) -> FeatureIdV1 {
    FeatureIdV1 {
        feature_type: match feature.kind {
            GeometricFeatureKindV1::Circle => 0,
            GeometricFeatureKindV1::Face => 1,
            GeometricFeatureKindV1::Vertex => 2,
        },
        feature_index: feature.index,
    }
}

fn collide_circles(
    pair: CandidatePairV1,
    body_a: &BodyStateV1,
    radius_a: f32,
    body_b: &BodyStateV1,
    radius_b: f32,
    speculative_distance: f32,
) -> Result<Option<CollisionManifoldV1>, String> {
    let delta = sub(body_b.position, body_a.position);
    let distance_squared = dot(delta, delta);
    let combined_radius = radius_a + radius_b;
    let speculative_radius = combined_radius + speculative_distance;
    if distance_squared > speculative_radius * speculative_radius {
        return Ok(None);
    }
    let distance = distance_squared.sqrt();
    let normal = if distance_squared > NORMAL_EPSILON_SQUARED {
        scale(delta, 1.0 / distance)
    } else {
        Vec2::new(1.0, 0.0)
    };
    let anchor_a = add(body_a.position, scale(normal, radius_a));
    let anchor_b = sub(body_b.position, scale(normal, radius_b));
    Ok(Some(manifold(
        pair,
        normal,
        body_a,
        body_b,
        vec![point(
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            distance - combined_radius,
            feature(GeometricFeatureKindV1::Circle, 0),
            feature(GeometricFeatureKindV1::Circle, 0),
        )?],
    )?))
}

fn collide_circle_rectangle(
    pair: CandidatePairV1,
    circle: &BodyStateV1,
    radius: f32,
    rectangle: &BodyStateV1,
    half_extents: Vec2,
    speculative_distance: f32,
) -> Result<Option<CollisionManifoldV1>, String> {
    let Some(contact) = rectangle_circle_contact(
        rectangle,
        half_extents,
        circle,
        radius,
        speculative_distance,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(manifold(
        pair,
        scale(contact.normal_rectangle_to_circle, -1.0),
        circle,
        rectangle,
        vec![point(
            circle,
            rectangle,
            contact.anchor_circle,
            contact.anchor_rectangle,
            contact.separation,
            feature(GeometricFeatureKindV1::Circle, 0),
            contact.rectangle_feature,
        )?],
    )?))
}

fn collide_rectangle_circle(
    pair: CandidatePairV1,
    rectangle: &BodyStateV1,
    half_extents: Vec2,
    circle: &BodyStateV1,
    radius: f32,
    speculative_distance: f32,
) -> Result<Option<CollisionManifoldV1>, String> {
    let Some(contact) = rectangle_circle_contact(
        rectangle,
        half_extents,
        circle,
        radius,
        speculative_distance,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(manifold(
        pair,
        contact.normal_rectangle_to_circle,
        rectangle,
        circle,
        vec![point(
            rectangle,
            circle,
            contact.anchor_rectangle,
            contact.anchor_circle,
            contact.separation,
            contact.rectangle_feature,
            feature(GeometricFeatureKindV1::Circle, 0),
        )?],
    )?))
}

struct RectangleCircleContact {
    normal_rectangle_to_circle: Vec2,
    anchor_rectangle: Vec2,
    anchor_circle: Vec2,
    separation: f32,
    rectangle_feature: GeometricFeatureV1,
}

fn rectangle_circle_contact(
    rectangle: &BodyStateV1,
    half: Vec2,
    circle: &BodyStateV1,
    radius: f32,
    speculative_distance: f32,
) -> Result<Option<RectangleCircleContact>, String> {
    let circle_local = inverse_rotate(
        sub(circle.position, rectangle.position),
        rectangle.angle.get(),
    );
    let closest = Vec2::new(
        circle_local.x.clamp(-half.x, half.x),
        circle_local.y.clamp(-half.y, half.y),
    );
    let local_delta = sub(circle_local, closest);
    let distance_squared = dot(local_delta, local_delta);
    let (local_normal, rectangle_local, rectangle_feature, separation) =
        if distance_squared > NORMAL_EPSILON_SQUARED {
            let speculative_radius = radius + speculative_distance;
            if distance_squared > speculative_radius * speculative_radius {
                return Ok(None);
            }
            let distance = distance_squared.sqrt();
            let clamped_x = circle_local.x < -half.x || circle_local.x > half.x;
            let clamped_y = circle_local.y < -half.y || circle_local.y > half.y;
            let rectangle_feature = if clamped_x && clamped_y {
                let index = match (closest.x > 0.0, closest.y > 0.0) {
                    (false, false) => 0,
                    (true, false) => 1,
                    (true, true) => 2,
                    (false, true) => 3,
                };
                feature(GeometricFeatureKindV1::Vertex, index)
            } else if clamped_x {
                feature(
                    GeometricFeatureKindV1::Face,
                    if closest.x > 0.0 { 1 } else { 3 },
                )
            } else {
                feature(
                    GeometricFeatureKindV1::Face,
                    if closest.y > 0.0 { 2 } else { 0 },
                )
            };
            (
                scale(local_delta, 1.0 / distance),
                closest,
                rectangle_feature,
                distance - radius,
            )
        } else {
            let margin_x = half.x - circle_local.x.abs();
            let margin_y = half.y - circle_local.y.abs();
            if margin_x <= margin_y {
                let sign = if circle_local.x >= 0.0 { 1.0 } else { -1.0 };
                (
                    Vec2::new(sign, 0.0),
                    Vec2::new(sign * half.x, circle_local.y),
                    feature(GeometricFeatureKindV1::Face, if sign > 0.0 { 1 } else { 3 }),
                    -margin_x - radius,
                )
            } else {
                let sign = if circle_local.y >= 0.0 { 1.0 } else { -1.0 };
                (
                    Vec2::new(0.0, sign),
                    Vec2::new(circle_local.x, sign * half.y),
                    feature(GeometricFeatureKindV1::Face, if sign > 0.0 { 2 } else { 0 }),
                    -margin_y - radius,
                )
            }
        };
    let normal = rotate(local_normal, rectangle.angle.get());
    let anchor_rectangle = add(
        rectangle.position,
        rotate(rectangle_local, rectangle.angle.get()),
    );
    let anchor_circle = sub(circle.position, scale(normal, radius));
    Ok(Some(RectangleCircleContact {
        normal_rectangle_to_circle: normal,
        anchor_rectangle,
        anchor_circle,
        separation,
        rectangle_feature,
    }))
}

fn collide_rectangles(
    pair: CandidatePairV1,
    body_a: &BodyStateV1,
    half_a: Vec2,
    body_b: &BodyStateV1,
    half_b: Vec2,
    speculative_distance: f32,
    identity_policy: ContactIdentityPolicyV1,
) -> Result<Option<CollisionManifoldV1>, String> {
    let rectangle_a = Rectangle::new(body_a, half_a);
    let rectangle_b = Rectangle::new(body_b, half_b);
    let (separation_a, separation_b) =
        face_separations_in_a_frame(&rectangle_a, &rectangle_b, half_a, body_a.angle.get());
    if separation_a.separation > speculative_distance
        || separation_b.separation > speculative_distance
    {
        return Ok(None);
    }
    let center_delta = sub(body_b.position, body_a.position);
    let deeply_coincident_identical = half_a == half_b
        && dot(center_delta, center_delta) <= 0.01
        && separation_a.separation <= -0.05
        && separation_b.separation <= -0.05;
    // Deeply coincident identical rectangles have no unique physical reference axis.
    // Keep those adversarial initial states coherent; use direct selection elsewhere.
    let reference_a = if deeply_coincident_identical {
        separation_b.separation
            <= REFERENCE_AXIS_RELATIVE_BIAS * separation_a.separation + REFERENCE_AXIS_ABSOLUTE_BIAS
    } else {
        // A single-frame projection has no generational prior pair, so the
        // temporal candidate begins from the same direct answer as the GPU.
        select_rectangle_reference_is_a(
            separation_a.separation,
            separation_b.separation,
            identity_policy,
            None,
        )
    };
    let (reference, incident, reference_normal, reference_face, normal_a_to_b) = if reference_a {
        let normal = rectangle_a.face_normal(separation_a.face);
        (
            &rectangle_a,
            &rectangle_b,
            normal,
            separation_a.face,
            normal,
        )
    } else {
        let normal = rectangle_b.face_normal(separation_b.face);
        (
            &rectangle_b,
            &rectangle_a,
            normal,
            separation_b.face,
            scale(normal, -1.0),
        )
    };
    let incident_face = incident.incident_face(reference_normal);
    let reference_vertices = reference.face_vertices(reference_face);
    let mut clipped = vec![
        ClipPoint {
            position: incident.vertices[incident_face.0 as usize],
            vertex: incident_face.0,
        },
        ClipPoint {
            position: incident.vertices[incident_face.1 as usize],
            vertex: incident_face.1,
        },
    ];
    let tangent = normalize(sub(reference_vertices.1, reference_vertices.0));
    let side_0 = dot(reference_vertices.0, tangent);
    let side_1 = dot(reference_vertices.1, tangent);
    clipped = clip_segment(clipped, tangent, side_0.min(side_1), true);
    clipped = clip_segment(clipped, tangent, side_0.max(side_1), false);
    let reference_center = midpoint(reference_vertices.0, reference_vertices.1);
    let mut points = Vec::new();
    for clipped_point in clipped {
        let separation = dot(
            sub(clipped_point.position, reference_center),
            reference_normal,
        );
        let reference_anchor = sub(clipped_point.position, scale(reference_normal, separation));
        let (anchor_a, anchor_b, feature_a, feature_b) = if reference_a {
            (
                reference_anchor,
                clipped_point.position,
                feature(GeometricFeatureKindV1::Face, reference_face),
                feature(GeometricFeatureKindV1::Vertex, clipped_point.vertex),
            )
        } else {
            (
                clipped_point.position,
                reference_anchor,
                feature(GeometricFeatureKindV1::Vertex, clipped_point.vertex),
                feature(GeometricFeatureKindV1::Face, reference_face),
            )
        };
        points.push(point(
            body_a, body_b, anchor_a, anchor_b, separation, feature_a, feature_b,
        )?);
    }
    if points.is_empty() {
        return Ok(None);
    }
    Ok(Some(manifold(pair, normal_a_to_b, body_a, body_b, points)?))
}

#[derive(Clone, Copy)]
struct FaceSeparation {
    face: u32,
    separation: f32,
}

struct Rectangle {
    axis_x: Vec2,
    axis_y: Vec2,
    vertices: [Vec2; 4],
}

impl Rectangle {
    fn new(body: &BodyStateV1, half: Vec2) -> Self {
        let axis_x = rotate(Vec2::new(1.0, 0.0), body.angle.get());
        let axis_y = rotate(Vec2::new(0.0, 1.0), body.angle.get());
        let local = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ];
        Self {
            axis_x,
            axis_y,
            vertices: local.map(|vertex| add(body.position, rotate(vertex, body.angle.get()))),
        }
    }

    fn face_normal(&self, face: u32) -> Vec2 {
        match face {
            0 => scale(self.axis_y, -1.0),
            1 => self.axis_x,
            2 => self.axis_y,
            3 => scale(self.axis_x, -1.0),
            _ => unreachable!(),
        }
    }

    fn face_for_normal(&self, normal: Vec2) -> u32 {
        let x = dot(normal, self.axis_x);
        let y = dot(normal, self.axis_y);
        if x.abs() >= y.abs() {
            if x >= 0.0 { 1 } else { 3 }
        } else if y >= 0.0 {
            2
        } else {
            0
        }
    }

    fn incident_face(&self, reference_normal: Vec2) -> (u32, u32) {
        match self.face_for_normal(scale(reference_normal, -1.0)) {
            0 => (0, 1),
            1 => (1, 2),
            2 => (2, 3),
            3 => (3, 0),
            _ => unreachable!(),
        }
    }

    fn face_vertices(&self, face: u32) -> (Vec2, Vec2) {
        match face {
            0 => (self.vertices[0], self.vertices[1]),
            1 => (self.vertices[1], self.vertices[2]),
            2 => (self.vertices[2], self.vertices[3]),
            3 => (self.vertices[3], self.vertices[0]),
            _ => unreachable!(),
        }
    }
}

fn face_separations_in_a_frame(
    rectangle_a: &Rectangle,
    rectangle_b: &Rectangle,
    half_a: Vec2,
    angle_a: f32,
) -> (FaceSeparation, FaceSeparation) {
    let vertices_a = [
        Vec2::ZERO,
        Vec2::new(2.0 * half_a.x, 0.0),
        Vec2::new(2.0 * half_a.x, 2.0 * half_a.y),
        Vec2::new(0.0, 2.0 * half_a.y),
    ];
    let origin = rectangle_a.vertices[0];
    let vertices_b = rectangle_b
        .vertices
        .map(|vertex| inverse_rotate(sub(vertex, origin), angle_a));
    let normals_a = [
        Vec2::new(0.0, -1.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(-1.0, 0.0),
    ];
    let normals_b = [0, 1, 2, 3].map(|face| inverse_rotate(rectangle_b.face_normal(face), angle_a));
    (
        best_face(&vertices_a, &normals_a, &vertices_b),
        best_face(&vertices_b, &normals_b, &vertices_a),
    )
}

fn best_face(
    reference_vertices: &[Vec2; 4],
    reference_normals: &[Vec2; 4],
    incident_vertices: &[Vec2; 4],
) -> FaceSeparation {
    let mut best = FaceSeparation {
        face: 0,
        separation: f32::NEG_INFINITY,
    };
    for face in 0..4 {
        let separation = incident_vertices
            .iter()
            .map(|vertex| {
                dot(
                    reference_normals[face],
                    sub(*vertex, reference_vertices[face]),
                )
            })
            .fold(f32::INFINITY, f32::min);
        if separation > best.separation {
            best = FaceSeparation {
                face: face as u32,
                separation,
            };
        }
    }
    best
}

#[derive(Clone, Copy)]
struct ClipPoint {
    position: Vec2,
    vertex: u32,
}

fn clip_segment(
    points: Vec<ClipPoint>,
    tangent: Vec2,
    limit: f32,
    keep_greater: bool,
) -> Vec<ClipPoint> {
    if points.len() != 2 {
        return points;
    }
    let signed = |point: ClipPoint| {
        let value = dot(point.position, tangent) - limit;
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
            let intersection = ClipPoint {
                position: add(
                    points[0].position,
                    scale(sub(points[1].position, points[0].position), fraction),
                ),
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

fn manifold(
    pair: CandidatePairV1,
    normal: Vec2,
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    points: Vec<CollisionContactPointV1>,
) -> Result<CollisionManifoldV1, String> {
    Ok(CollisionManifoldV1 {
        pair,
        normal,
        material: MaterialV1 {
            friction: finite(
                (body_a.material.friction.get() * body_b.material.friction.get()).sqrt(),
            )?,
            restitution: finite(
                body_a
                    .material
                    .restitution
                    .get()
                    .max(body_b.material.restitution.get()),
            )?,
        },
        points,
    })
}

fn point(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    anchor_a: Vec2,
    anchor_b: Vec2,
    separation: f32,
    feature_a: GeometricFeatureV1,
    feature_b: GeometricFeatureV1,
) -> Result<CollisionContactPointV1, String> {
    Ok(CollisionContactPointV1 {
        local_anchor_a: inverse_rotate(sub(anchor_a, body_a.position), body_a.angle.get()),
        local_anchor_b: inverse_rotate(sub(anchor_b, body_b.position), body_b.angle.get()),
        world_point: midpoint(anchor_a, anchor_b),
        separation: finite(separation)?,
        feature_a,
        feature_b,
    })
}

fn body(scenario: &ScenarioV1, id: physbox_contracts::BodyId) -> Result<&BodyStateV1, String> {
    scenario
        .bodies
        .binary_search_by_key(&id, |body| body.id)
        .map(|index| &scenario.bodies[index])
        .map_err(|_| {
            format!(
                "candidate references missing body {}:{}",
                id.slot, id.generation
            )
        })
}

const fn feature(kind: GeometricFeatureKindV1, index: u32) -> GeometricFeatureV1 {
    GeometricFeatureV1 { kind, index }
}

fn finite(value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| error.to_string())
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

fn scale(vector: Vec2, scalar: f32) -> Vec2 {
    Vec2::new(vector.x * scalar, vector.y * scalar)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    scale(add(a, b), 0.5)
}

fn normalize(vector: Vec2) -> Vec2 {
    scale(vector, dot(vector, vector).sqrt().recip())
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sine, cosine) = angle.sin_cos();
    Vec2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}

fn inverse_rotate(vector: Vec2, angle: f32) -> Vec2 {
    rotate(vector, -angle)
}

#[cfg(test)]
mod tests {
    use physbox_engine::{
        P5_2CandidatePattern, P5_2CollisionScenarioKind, p5_2_candidate_scenario,
        p5_2_collision_scenario, p5_2_contact_policy_candidate,
    };

    use crate::collision_candidates::enumerate_scalar_candidates;
    use crate::failure_automation::minimize_failure;

    use super::*;

    #[test]
    fn all_analytic_pair_families_produce_valid_manifolds() {
        for kind in P5_2CollisionScenarioKind::ALL {
            let scenario = p5_2_collision_scenario(kind, 1, 17).unwrap();
            let candidates = enumerate_scalar_candidates(&scenario).unwrap();
            let manifolds = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
            assert!(!manifolds.is_empty(), "{kind:?}");
            assert!(manifolds.iter().all(|manifold| manifold.validate().is_ok()));
        }
    }

    #[test]
    fn translation_and_rotation_preserve_topology_and_separation() {
        let scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let baseline = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let mut transformed = scenario.clone();
        let rotation = 0.73;
        let translation = Vec2::new(3.5, -2.0);
        for body in &mut transformed.bodies {
            body.position = add(rotate(body.position, rotation), translation);
            body.angle = finite(body.angle.get() + rotation).unwrap();
        }
        let candidates = enumerate_scalar_candidates(&transformed).unwrap();
        let actual = generate_scalar_narrowphase(&transformed, &candidates.candidates).unwrap();
        assert_eq!(baseline.len(), actual.len());
        assert_eq!(baseline[0].points.len(), actual[0].points.len());
        assert_eq!(
            baseline[0].points[0].feature_a,
            actual[0].points[0].feature_a
        );
        assert_eq!(
            baseline[0].points[0].feature_b,
            actual[0].points[0].feature_b
        );
        assert_vec_close(
            actual[0].normal,
            rotate(baseline[0].normal, rotation),
            NARROWPHASE_ABSOLUTE_TOLERANCE,
        );
        for (expected, actual) in baseline[0].points.iter().zip(&actual[0].points) {
            assert!((expected.separation.get() - actual.separation.get()).abs() <= 2.0e-5);
        }
    }

    #[test]
    fn label_swap_reverses_normal_and_preserves_geometric_features() {
        let scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleRectangle, 1, 17).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let baseline = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let mut swapped = scenario.clone();
        let id_a = swapped.bodies[0].id;
        let id_b = swapped.bodies[1].id;
        swapped.bodies.swap(0, 1);
        swapped.bodies[0].id = id_a;
        swapped.bodies[1].id = id_b;
        let candidates = enumerate_scalar_candidates(&swapped).unwrap();
        let actual = generate_scalar_narrowphase(&swapped, &candidates.candidates).unwrap();
        assert_vec_close(
            actual[0].normal,
            scale(baseline[0].normal, -1.0),
            NARROWPHASE_ABSOLUTE_TOLERANCE,
        );
        assert_eq!(
            actual[0].points[0].feature_a,
            baseline[0].points[0].feature_b
        );
        assert_eq!(
            actual[0].points[0].feature_b,
            baseline[0].points[0].feature_a
        );
        assert_vec_close(
            actual[0].points[0].world_point,
            baseline[0].points[0].world_point,
            NARROWPHASE_ABSOLUTE_TOLERANCE,
        );
        assert!(
            (actual[0].points[0].separation.get() - baseline[0].points[0].separation.get()).abs()
                <= NARROWPHASE_ABSOLUTE_TOLERANCE
        );
    }

    #[test]
    fn exact_corner_ties_follow_box2d_face_enumeration() {
        let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Touching, 3, 1).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let manifolds = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        assert_eq!(manifolds.len(), 2);
        assert_eq!(manifolds[0].normal, Vec2::new(1.0, 0.0));
        assert_eq!(manifolds[1].normal, Vec2::new(0.0, -1.0));
        assert_eq!(manifolds[0].points.len(), 2);
        assert_eq!(manifolds[1].points.len(), 2);
    }

    #[test]
    fn prior_reference_hysteresis_is_symmetric_bounded_and_history_free_on_start() {
        let policy = ContactIdentityPolicyV1::PriorContactHysteresis;
        assert!(select_rectangle_reference_is_a(
            -0.001,
            -0.0005,
            policy,
            Some(true)
        ));
        assert!(!select_rectangle_reference_is_a(
            -0.0005,
            -0.001,
            policy,
            Some(false)
        ));
        assert!(!select_rectangle_reference_is_a(
            -0.002,
            0.0,
            policy,
            Some(true)
        ));
        assert!(select_rectangle_reference_is_a(
            0.0,
            -0.002,
            policy,
            Some(false)
        ));
        assert!(!select_rectangle_reference_is_a(
            -0.001, -0.0005, policy, None
        ));
        assert!(select_rectangle_reference_is_a(
            -0.0005, -0.001, policy, None
        ));
    }

    #[test]
    fn circle_contact_creation_and_removal_boundary_is_exact() {
        let mut scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, 1, 17).unwrap();
        scenario.bodies[0].position = Vec2::ZERO;
        scenario.bodies[1].position = Vec2::new(0.7, 0.0);
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let touching = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        assert_eq!(touching.len(), 1);
        assert_eq!(touching[0].points[0].separation.get(), 0.0);

        scenario.bodies[1].position.x = 0.7001;
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        assert!(candidates.candidates.is_empty());

        scenario.bodies[1].position.x = 0.6999;
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let penetrating = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        assert!(penetrating[0].points[0].separation.get() < 0.0);
    }

    #[test]
    fn speculative_circle_admission_preserves_actual_separation() {
        let mut scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, 1, 17).unwrap();
        scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
        scenario.bodies[0].position = Vec2::ZERO;
        scenario.bodies[1].position = Vec2::new(0.71, 0.0);
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        assert_eq!(candidates.candidates.len(), 1);
        let admitted = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        assert_eq!(admitted.len(), 1);
        assert!((admitted[0].points[0].separation.get() - 0.01).abs() <= 1.0e-6);

        scenario.bodies[1].position.x = 0.721;
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        assert_eq!(candidates.candidates.len(), 1);
        assert!(
            generate_scalar_narrowphase(&scenario, &candidates.candidates)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn injected_missing_manifold_minimizes_to_one_pair() {
        let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 24, 12).unwrap();
        let minimized = minimize_failure(scenario, |candidate| {
            if candidate.bodies.len() < 2 {
                return false;
            }
            let Ok(candidates) = enumerate_scalar_candidates(candidate) else {
                return false;
            };
            generate_scalar_narrowphase(candidate, &candidates.candidates)
                .is_ok_and(|manifolds| !manifolds.is_empty())
        })
        .unwrap()
        .unwrap();
        assert_eq!(minimized.scenario.bodies.len(), 2);
        assert!(minimized.attempts > 1);
    }

    fn assert_vec_close(actual: Vec2, expected: Vec2, tolerance: f32) {
        assert!((actual.x - expected.x).abs() <= tolerance);
        assert!((actual.y - expected.y).abs() <= tolerance);
    }
}
