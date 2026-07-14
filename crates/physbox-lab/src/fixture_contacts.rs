use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyStateV1, FeatureIdV1, FiniteF32, MaterialV1,
    ShapeV1, Vec2,
};

const AXIS_EPSILON: f32 = 1.0e-5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureContactError(pub String);

impl Display for FixtureContactError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FixtureContactError {}

pub fn generate_fixture_contacts(
    bodies: &[BodyStateV1],
) -> Result<Vec<AuthoredManifoldV1>, FixtureContactError> {
    let mut manifolds = Vec::new();
    for body_a in 0..bodies.len() {
        for body_b in body_a + 1..bodies.len() {
            if let Some(mut manifold) = pair_contact(&bodies[body_a], &bodies[body_b])? {
                manifold.id = manifolds.len() as u32;
                manifolds.push(manifold);
            }
        }
    }
    Ok(manifolds)
}

fn pair_contact(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
) -> Result<Option<AuthoredManifoldV1>, FixtureContactError> {
    match (body_a.shape, body_b.shape) {
        (ShapeV1::Circle { radius }, ShapeV1::Rectangle { half_extents })
            if is_fixed_axis_aligned(body_b) =>
        {
            circle_floor(body_a, radius.get(), body_b, half_extents)
        }
        (ShapeV1::Rectangle { half_extents }, ShapeV1::Circle { radius })
            if is_fixed_axis_aligned(body_a) =>
        {
            circle_floor(body_b, radius.get(), body_a, half_extents)
        }
        (
            ShapeV1::Rectangle {
                half_extents: half_a,
            },
            ShapeV1::Rectangle {
                half_extents: half_b,
            },
        ) if is_axis_aligned(body_a) && is_axis_aligned(body_b) => {
            axis_aligned_boxes(body_a, half_a, body_b, half_b)
        }
        (ShapeV1::Rectangle { half_extents }, ShapeV1::Rectangle { .. })
            if is_fixed_axis_aligned(body_b) =>
        {
            rectangle_floor(body_a, half_extents, body_b)
        }
        (ShapeV1::Rectangle { .. }, ShapeV1::Rectangle { half_extents })
            if is_fixed_axis_aligned(body_a) =>
        {
            rectangle_floor(body_b, half_extents, body_a)
        }
        (shape_a, shape_b) => Err(FixtureContactError(format!(
            "analytic fixture generator does not support {:?} at angle {} versus {:?} at angle {}",
            shape_a,
            body_a.angle.get(),
            shape_b,
            body_b.angle.get()
        ))),
    }
}

fn circle_floor(
    circle: &BodyStateV1,
    radius: f32,
    floor: &BodyStateV1,
    floor_half_extents: Vec2,
) -> Result<Option<AuthoredManifoldV1>, FixtureContactError> {
    let floor_left = floor.position.x - floor_half_extents.x;
    let floor_right = floor.position.x + floor_half_extents.x;
    if circle.position.x < floor_left || circle.position.x > floor_right {
        return Ok(None);
    }
    let floor_top = floor.position.y + floor_half_extents.y;
    let circle_bottom = circle.position.y - radius;
    let separation = circle_bottom - floor_top;
    if separation > 0.0 {
        return Ok(None);
    }
    let anchor_a_world = Vec2::new(circle.position.x, circle_bottom);
    let anchor_b_world = Vec2::new(circle.position.x, floor_top);
    Ok(Some(AuthoredManifoldV1 {
        id: 0,
        body_a: circle.id,
        body_b: floor.id,
        normal: Vec2::new(0.0, -1.0),
        material: mix_material(circle.material, floor.material)?,
        points: vec![contact_point(
            circle,
            floor,
            anchor_a_world,
            anchor_b_world,
            separation,
            FeatureIdV1 {
                feature_type: 0,
                feature_index: 0,
            },
            FeatureIdV1 {
                feature_type: 1,
                feature_index: 2,
            },
        )?],
    }))
}

fn rectangle_floor(
    rectangle: &BodyStateV1,
    half_extents: Vec2,
    floor: &BodyStateV1,
) -> Result<Option<AuthoredManifoldV1>, FixtureContactError> {
    let ShapeV1::Rectangle {
        half_extents: floor_half_extents,
    } = floor.shape
    else {
        unreachable!("floor shape was matched as a rectangle")
    };
    let floor_top = floor.position.y + floor_half_extents.y;
    let floor_left = floor.position.x - floor_half_extents.x;
    let floor_right = floor.position.x + floor_half_extents.x;
    let local_vertices = [
        Vec2::new(-half_extents.x, -half_extents.y),
        Vec2::new(half_extents.x, -half_extents.y),
        Vec2::new(half_extents.x, half_extents.y),
        Vec2::new(-half_extents.x, half_extents.y),
    ];
    let world_vertices =
        local_vertices.map(|local| add(rectangle.position, rotate(local, rectangle.angle.get())));
    let minimum_y = world_vertices
        .iter()
        .map(|vertex| vertex.y)
        .fold(f32::INFINITY, f32::min);
    if minimum_y > floor_top {
        return Ok(None);
    }
    let mut points = Vec::new();
    for (feature_index, (local, world)) in local_vertices.iter().zip(world_vertices).enumerate() {
        if (world.y - minimum_y).abs() <= AXIS_EPSILON
            && world.x >= floor_left
            && world.x <= floor_right
        {
            let floor_anchor = Vec2::new(world.x, floor_top);
            points.push(AuthoredContactPointV1 {
                local_anchor_a: *local,
                local_anchor_b: inverse_rotate(
                    sub(floor_anchor, floor.position),
                    floor.angle.get(),
                ),
                world_point: midpoint(world, floor_anchor),
                separation: finite(world.y - floor_top)?,
                feature_a: FeatureIdV1 {
                    feature_type: 2,
                    feature_index: feature_index as u32,
                },
                feature_b: FeatureIdV1 {
                    feature_type: 1,
                    feature_index: 2,
                },
            });
        }
    }
    if points.is_empty() {
        return Ok(None);
    }
    Ok(Some(AuthoredManifoldV1 {
        id: 0,
        body_a: rectangle.id,
        body_b: floor.id,
        normal: Vec2::new(0.0, -1.0),
        material: mix_material(rectangle.material, floor.material)?,
        points,
    }))
}

fn axis_aligned_boxes(
    body_a: &BodyStateV1,
    half_a: Vec2,
    body_b: &BodyStateV1,
    half_b: Vec2,
) -> Result<Option<AuthoredManifoldV1>, FixtureContactError> {
    let delta = sub(body_b.position, body_a.position);
    let overlap_x = half_a.x + half_b.x - delta.x.abs();
    let overlap_y = half_a.y + half_b.y - delta.y.abs();
    if overlap_x < 0.0 || overlap_y < 0.0 {
        return Ok(None);
    }
    let (normal, anchors, feature_a, feature_b) = if overlap_x < overlap_y {
        let sign = if delta.x >= 0.0 { 1.0 } else { -1.0 };
        let normal = Vec2::new(sign, 0.0);
        let x_a = body_a.position.x + sign * half_a.x;
        let x_b = body_b.position.x - sign * half_b.x;
        let bottom = (body_a.position.y - half_a.y).max(body_b.position.y - half_b.y);
        let top = (body_a.position.y + half_a.y).min(body_b.position.y + half_b.y);
        (
            normal,
            interval_anchors(
                Vec2::new(x_a, bottom),
                Vec2::new(x_b, bottom),
                Vec2::new(x_a, top),
                Vec2::new(x_b, top),
            ),
            FeatureIdV1 {
                feature_type: 1,
                feature_index: if sign > 0.0 { 1 } else { 3 },
            },
            FeatureIdV1 {
                feature_type: 1,
                feature_index: if sign > 0.0 { 3 } else { 1 },
            },
        )
    } else {
        let sign = if delta.y >= 0.0 { 1.0 } else { -1.0 };
        let normal = Vec2::new(0.0, sign);
        let y_a = body_a.position.y + sign * half_a.y;
        let y_b = body_b.position.y - sign * half_b.y;
        let left = (body_a.position.x - half_a.x).max(body_b.position.x - half_b.x);
        let right = (body_a.position.x + half_a.x).min(body_b.position.x + half_b.x);
        (
            normal,
            interval_anchors(
                Vec2::new(left, y_a),
                Vec2::new(left, y_b),
                Vec2::new(right, y_a),
                Vec2::new(right, y_b),
            ),
            FeatureIdV1 {
                feature_type: 1,
                feature_index: if sign > 0.0 { 2 } else { 0 },
            },
            FeatureIdV1 {
                feature_type: 1,
                feature_index: if sign > 0.0 { 0 } else { 2 },
            },
        )
    };
    let points = anchors
        .into_iter()
        .enumerate()
        .map(|(index, (anchor_a, anchor_b))| {
            let separation = dot(sub(anchor_b, anchor_a), normal);
            contact_point(
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                separation,
                FeatureIdV1 {
                    feature_type: feature_a.feature_type,
                    feature_index: feature_a.feature_index * 2 + index as u32,
                },
                FeatureIdV1 {
                    feature_type: feature_b.feature_type,
                    feature_index: feature_b.feature_index * 2 + index as u32,
                },
            )
        })
        .collect::<Result<Vec<_>, FixtureContactError>>()?;
    Ok(Some(AuthoredManifoldV1 {
        id: 0,
        body_a: body_a.id,
        body_b: body_b.id,
        normal,
        material: mix_material(body_a.material, body_b.material)?,
        points,
    }))
}

fn interval_anchors(
    first_a: Vec2,
    first_b: Vec2,
    second_a: Vec2,
    second_b: Vec2,
) -> Vec<(Vec2, Vec2)> {
    if squared_distance(first_a, second_a) <= AXIS_EPSILON * AXIS_EPSILON {
        vec![(first_a, first_b)]
    } else {
        vec![(first_a, first_b), (second_a, second_b)]
    }
}

fn contact_point(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    anchor_a_world: Vec2,
    anchor_b_world: Vec2,
    separation: f32,
    feature_a: FeatureIdV1,
    feature_b: FeatureIdV1,
) -> Result<AuthoredContactPointV1, FixtureContactError> {
    Ok(AuthoredContactPointV1 {
        local_anchor_a: inverse_rotate(sub(anchor_a_world, body_a.position), body_a.angle.get()),
        local_anchor_b: inverse_rotate(sub(anchor_b_world, body_b.position), body_b.angle.get()),
        world_point: midpoint(anchor_a_world, anchor_b_world),
        separation: finite(separation)?,
        feature_a,
        feature_b,
    })
}

fn mix_material(a: MaterialV1, b: MaterialV1) -> Result<MaterialV1, FixtureContactError> {
    Ok(MaterialV1 {
        friction: finite((a.friction.get() * b.friction.get()).sqrt())?,
        restitution: finite(a.restitution.get().max(b.restitution.get()))?,
    })
}

fn is_fixed_axis_aligned(body: &BodyStateV1) -> bool {
    body.inverse_mass.get() == 0.0 && is_axis_aligned(body)
}

fn is_axis_aligned(body: &BodyStateV1) -> bool {
    body.angle.get().abs() <= AXIS_EPSILON
}

fn finite(value: f32) -> Result<FiniteF32, FixtureContactError> {
    FiniteF32::new(value).map_err(|error| FixtureContactError(error.to_string()))
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y))
}

fn squared_distance(a: Vec2, b: Vec2) -> f32 {
    let delta = sub(a, b);
    dot(delta, delta)
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

fn inverse_rotate(vector: Vec2, angle: f32) -> Vec2 {
    rotate(vector, -angle)
}

#[cfg(test)]
mod tests {
    use physbox_contracts::Validate;
    use physbox_engine::{AnalyticFixtureKind, analytic_fixture_scenario};

    use super::*;

    #[test]
    fn circle_floor_generates_one_stable_contact() {
        let mut scenario = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        scenario.bodies[0].position.y = 0.49;
        let contacts = generate_fixture_contacts(&scenario.bodies).unwrap();
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].points.len(), 1);
        assert!((contacts[0].points[0].separation.get() + 0.01).abs() < 1.0e-6);
        contacts[0].validate().unwrap();
    }

    #[test]
    fn flat_and_rotated_rectangles_generate_two_and_one_floor_points() {
        let mut scenario =
            analytic_fixture_scenario(AnalyticFixtureKind::RectangleFloor, 1).unwrap();
        scenario.bodies[0].position.y = 0.49;
        let flat = generate_fixture_contacts(&scenario.bodies).unwrap();
        assert_eq!(flat[0].points.len(), 2);

        scenario.bodies[0].angle = finite(0.2).unwrap();
        scenario.bodies[0].position.y = 0.5;
        let corner = generate_fixture_contacts(&scenario.bodies).unwrap();
        assert_eq!(corner[0].points.len(), 1);
    }

    #[test]
    fn axis_aligned_boxes_cover_vertical_and_side_contacts() {
        let mut scenario = analytic_fixture_scenario(AnalyticFixtureKind::BoxOnBox, 1).unwrap();
        let vertical = generate_fixture_contacts(&scenario.bodies).unwrap();
        assert_eq!(vertical.len(), 1);
        assert_eq!(vertical[0].normal, Vec2::new(0.0, -1.0));
        assert_eq!(vertical[0].points.len(), 2);

        scenario.bodies[0].position = Vec2::new(-0.99, 0.0);
        scenario.bodies[1].position = Vec2::new(0.0, 0.0);
        let side = generate_fixture_contacts(&scenario.bodies).unwrap();
        assert_eq!(side[0].normal, Vec2::new(1.0, 0.0));
        assert_eq!(side[0].points.len(), 2);
    }

    #[test]
    fn unsupported_circle_pair_is_rejected_instead_of_omitted() {
        let mut scenario = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        scenario.bodies[1].shape = ShapeV1::Circle {
            radius: finite(1.0).unwrap(),
        };
        let error = generate_fixture_contacts(&scenario.bodies).unwrap_err();
        assert!(error.0.contains("does not support"));
    }
}
