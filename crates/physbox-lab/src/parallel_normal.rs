use std::collections::BTreeMap;

use physbox_contracts::{
    BodyId, BodyStateV1, FiniteF32, ScenarioV1, Validate, Vec2, canonical_hash,
};
use physbox_engine::p5_2_centered_rectangle_stack_scenario;
use serde::Serialize;

use crate::{
    analytic_narrowphase::{authored_solver_projection, generate_scalar_narrowphase},
    collision_candidates::enumerate_scalar_candidates,
};

pub const PARALLEL_NORMAL_DIAGNOSTIC_VERSION: u32 = 1;
pub const ROCKING_PROBE_ANGULAR_SPEED: f32 = 0.01;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParallelNormalModeV1 {
    PointJacobi,
    ScalarMassSplit,
    ManifoldBlock,
    BlockMassSplit,
}

impl ParallelNormalModeV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PointJacobi => "point-jacobi",
            Self::ScalarMassSplit => "scalar-mass-split",
            Self::ManifoldBlock => "manifold-block",
            Self::BlockMassSplit => "block-mass-split",
        }
    }

    const fn uses_block(self) -> bool {
        matches!(self, Self::ManifoldBlock | Self::BlockMassSplit)
    }

    const fn count_kind(self) -> CountKind {
        match self {
            Self::PointJacobi | Self::ManifoldBlock => CountKind::One,
            Self::ScalarMassSplit => CountKind::Point,
            Self::BlockMassSplit => CountKind::Manifold,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyIncidenceV1 {
    pub body: BodyId,
    pub point_count: u32,
    pub manifold_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockActiveSetV1 {
    IndependentPoints,
    Both,
    First,
    Second,
    None,
    Scalar,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifoldIterationV1 {
    pub manifold_id: u32,
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub normal: Vec2,
    pub point_count: u32,
    pub effective_mass_matrix: [f32; 4],
    pub active_set: BlockActiveSetV1,
    pub impulse_before: [f32; 2],
    pub impulse_after: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenNormalIterationV1 {
    pub iteration: u32,
    pub complementarity_residual_before: f32,
    pub complementarity_residual_after: f32,
    pub residual_ratio: f32,
    pub max_closing_speed_after: f32,
    pub kinetic_energy_after: f32,
    pub manifolds: Vec<ManifoldIterationV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalImpulseV1 {
    pub manifold_id: u32,
    pub point_index: u32,
    pub impulse: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenNormalDiagnosticV1 {
    pub version: u32,
    pub mode: ParallelNormalModeV1,
    pub source_scenario: String,
    pub source_scenario_hash: String,
    pub rectangle_count: Option<u32>,
    pub applied_gravity_seconds: f32,
    pub applied_rocking_angular_speed: f32,
    pub incidence: Vec<BodyIncidenceV1>,
    pub iterations: Vec<FrozenNormalIterationV1>,
    pub impulses: Vec<NormalImpulseV1>,
    pub bodies: Vec<BodyStateV1>,
}

#[derive(Clone, Copy)]
enum CountKind {
    One,
    Point,
    Manifold,
}

#[derive(Clone)]
struct PreparedManifold {
    id: u32,
    body_a: usize,
    body_b: usize,
    normal: Vec2,
    points: Vec<PreparedPoint>,
}

#[derive(Clone)]
struct PreparedPoint {
    anchor_a: Vec2,
    anchor_b: Vec2,
    impulse: f32,
}

pub fn frozen_rectangle_stack_diagnostic(
    rectangle_count: u32,
    mode: ParallelNormalModeV1,
    iterations: u32,
) -> Result<FrozenNormalDiagnosticV1, String> {
    let (projection, gravity_seconds) =
        rectangle_stack_projection(rectangle_count, ROCKING_PROBE_ANGULAR_SPEED)?;
    let mut diagnostic = solve_frozen_normal_system(&projection, mode, iterations)?;
    diagnostic.rectangle_count = Some(rectangle_count);
    diagnostic.applied_gravity_seconds = gravity_seconds;
    diagnostic.applied_rocking_angular_speed = ROCKING_PROBE_ANGULAR_SPEED;
    Ok(diagnostic)
}

fn rectangle_stack_projection(
    rectangle_count: u32,
    rocking_angular_speed: f32,
) -> Result<(ScenarioV1, f32), String> {
    let source = p5_2_centered_rectangle_stack_scenario(rectangle_count, 1)
        .map_err(|error| error.to_string())?;
    let candidates = enumerate_scalar_candidates(&source)?;
    let manifolds = generate_scalar_narrowphase(&source, &candidates.candidates)?;
    let mut projection = authored_solver_projection(&source, &manifolds)?;
    let gravity_seconds = projection.settings.fixed_dt.get() / projection.settings.substeps as f32;
    for (index, body) in projection.bodies.iter_mut().enumerate() {
        if body.inverse_mass.get() > 0.0 {
            body.linear_velocity.x += projection.settings.gravity.x * gravity_seconds;
            body.linear_velocity.y += projection.settings.gravity.y * gravity_seconds;
            body.angular_velocity = FiniteF32::new(if index % 2 == 0 {
                rocking_angular_speed
            } else {
                -rocking_angular_speed
            })
            .map_err(|error| error.to_string())?;
        }
    }
    Ok((projection, gravity_seconds))
}

pub fn solve_frozen_normal_system(
    scenario: &ScenarioV1,
    mode: ParallelNormalModeV1,
    iterations: u32,
) -> Result<FrozenNormalDiagnosticV1, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    let source_scenario_hash = canonical_hash(scenario).map_err(|error| error.to_string())?;
    if scenario.manifolds.is_empty() {
        return Err("frozen normal diagnostic requires authored manifolds".to_owned());
    }
    if !scenario.contact_cache.is_empty() || !scenario.actions.is_empty() {
        return Err("frozen normal diagnostic requires cold contacts and no actions".to_owned());
    }
    if iterations == 0 {
        return Err("frozen normal diagnostic requires at least one iteration".to_owned());
    }

    let body_indices = scenario
        .bodies
        .iter()
        .enumerate()
        .map(|(index, body)| (body.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut point_counts = vec![0_u32; scenario.bodies.len()];
    let mut manifold_counts = vec![0_u32; scenario.bodies.len()];
    let mut prepared = Vec::with_capacity(scenario.manifolds.len());
    for manifold in &scenario.manifolds {
        if manifold.points.is_empty() || manifold.points.len() > 2 {
            return Err(format!(
                "frozen normal manifold {} requires one or two points",
                manifold.id
            ));
        }
        let body_a = *body_indices
            .get(&manifold.body_a)
            .ok_or_else(|| format!("missing body {:?}", manifold.body_a))?;
        let body_b = *body_indices
            .get(&manifold.body_b)
            .ok_or_else(|| format!("missing body {:?}", manifold.body_b))?;
        manifold_counts[body_a] += 1;
        manifold_counts[body_b] += 1;
        let points = manifold
            .points
            .iter()
            .map(|point| {
                point_counts[body_a] += 1;
                point_counts[body_b] += 1;
                PreparedPoint {
                    anchor_a: rotate(point.local_anchor_a, scenario.bodies[body_a].angle.get()),
                    anchor_b: rotate(point.local_anchor_b, scenario.bodies[body_b].angle.get()),
                    impulse: 0.0,
                }
            })
            .collect();
        prepared.push(PreparedManifold {
            id: manifold.id,
            body_a,
            body_b,
            normal: manifold.normal,
            points,
        });
    }

    let incidence = scenario
        .bodies
        .iter()
        .enumerate()
        .map(|(index, body)| BodyIncidenceV1 {
            body: body.id,
            point_count: point_counts[index],
            manifold_count: manifold_counts[index],
        })
        .collect();
    let mut bodies = scenario.bodies.clone();
    let mut samples = Vec::with_capacity(iterations as usize);
    for iteration in 0..iterations {
        let residual_before = complementarity_residual(&bodies, &prepared);
        let source = bodies.clone();
        let mut destination = source.clone();
        let mut manifold_samples = Vec::with_capacity(prepared.len());
        for manifold in &mut prepared {
            let count_a = split_count(
                mode.count_kind(),
                manifold.body_a,
                &point_counts,
                &manifold_counts,
            );
            let count_b = split_count(
                mode.count_kind(),
                manifold.body_b,
                &point_counts,
                &manifold_counts,
            );
            let matrix = effective_mass_matrix(&source, manifold, count_a, count_b);
            let impulse_before = impulses(manifold);
            let velocities = normal_velocities(&source, manifold);
            let (impulse_after, active_set) = if mode.uses_block() {
                solve_block_lcp(matrix, velocities, impulse_before, manifold.points.len())?
            } else {
                let mut output = impulse_before;
                for point_index in 0..manifold.points.len() {
                    let diagonal = if point_index == 0 {
                        matrix[0]
                    } else {
                        matrix[3]
                    };
                    let effective_mass = if diagonal > 0.0 { 1.0 / diagonal } else { 0.0 };
                    output[point_index] = (impulse_before[point_index]
                        - effective_mass * velocities[point_index])
                        .max(0.0);
                }
                (output, BlockActiveSetV1::IndependentPoints)
            };
            for point_index in 0..manifold.points.len() {
                let delta = impulse_after[point_index] - impulse_before[point_index];
                manifold.points[point_index].impulse = impulse_after[point_index];
                let impulse = scale(manifold.normal, delta);
                apply_impulse(
                    &mut destination[manifold.body_a],
                    scale(impulse, -1.0),
                    manifold.points[point_index].anchor_a,
                )?;
                apply_impulse(
                    &mut destination[manifold.body_b],
                    impulse,
                    manifold.points[point_index].anchor_b,
                )?;
            }
            manifold_samples.push(ManifoldIterationV1 {
                manifold_id: manifold.id,
                body_a: source[manifold.body_a].id,
                body_b: source[manifold.body_b].id,
                normal: manifold.normal,
                point_count: manifold.points.len() as u32,
                effective_mass_matrix: matrix,
                active_set: if manifold.points.len() == 1 && mode.uses_block() {
                    BlockActiveSetV1::Scalar
                } else {
                    active_set
                },
                impulse_before,
                impulse_after,
            });
        }
        bodies = destination;
        validate_finite_bodies(&bodies)?;
        let residual_after = complementarity_residual(&bodies, &prepared);
        let residual_ratio = if residual_before > 0.0 {
            residual_after / residual_before
        } else {
            0.0
        };
        let closing_speed = max_closing_speed(&bodies, &prepared);
        let energy = kinetic_energy(&bodies);
        validate_metric("complementarity residual before", residual_before)?;
        validate_metric("complementarity residual after", residual_after)?;
        validate_metric("residual ratio", residual_ratio)?;
        validate_metric("maximum closing speed", closing_speed)?;
        validate_metric("kinetic energy", energy)?;
        samples.push(FrozenNormalIterationV1 {
            iteration,
            complementarity_residual_before: residual_before,
            complementarity_residual_after: residual_after,
            residual_ratio,
            max_closing_speed_after: closing_speed,
            kinetic_energy_after: energy,
            manifolds: manifold_samples,
        });
    }

    let impulses = prepared
        .iter()
        .flat_map(|manifold| {
            manifold
                .points
                .iter()
                .enumerate()
                .map(|(point_index, point)| NormalImpulseV1 {
                    manifold_id: manifold.id,
                    point_index: point_index as u32,
                    impulse: point.impulse,
                })
        })
        .collect();
    Ok(FrozenNormalDiagnosticV1 {
        version: PARALLEL_NORMAL_DIAGNOSTIC_VERSION,
        mode,
        source_scenario: scenario.id.clone(),
        source_scenario_hash,
        rectangle_count: None,
        applied_gravity_seconds: 0.0,
        applied_rocking_angular_speed: 0.0,
        incidence,
        iterations: samples,
        impulses,
        bodies,
    })
}

fn split_count(kind: CountKind, body: usize, point_counts: &[u32], manifold_counts: &[u32]) -> f32 {
    match kind {
        CountKind::One => 1.0,
        CountKind::Point => point_counts[body].max(1) as f32,
        CountKind::Manifold => manifold_counts[body].max(1) as f32,
    }
}

fn effective_mass_matrix(
    bodies: &[BodyStateV1],
    manifold: &PreparedManifold,
    count_a: f32,
    count_b: f32,
) -> [f32; 4] {
    let entry = |left: &PreparedPoint, right: &PreparedPoint| {
        let body_a = &bodies[manifold.body_a];
        let body_b = &bodies[manifold.body_b];
        count_a * body_a.inverse_mass.get()
            + count_b * body_b.inverse_mass.get()
            + count_a
                * body_a.inverse_inertia.get()
                * cross(left.anchor_a, manifold.normal)
                * cross(right.anchor_a, manifold.normal)
            + count_b
                * body_b.inverse_inertia.get()
                * cross(left.anchor_b, manifold.normal)
                * cross(right.anchor_b, manifold.normal)
    };
    let k11 = entry(&manifold.points[0], &manifold.points[0]);
    if manifold.points.len() == 1 {
        [k11, 0.0, 0.0, 0.0]
    } else {
        let k12 = entry(&manifold.points[0], &manifold.points[1]);
        let k22 = entry(&manifold.points[1], &manifold.points[1]);
        [k11, k12, k12, k22]
    }
}

fn solve_block_lcp(
    matrix: [f32; 4],
    velocity: [f32; 2],
    old: [f32; 2],
    point_count: usize,
) -> Result<([f32; 2], BlockActiveSetV1), String> {
    if point_count == 1 {
        let mass = if matrix[0] > 0.0 {
            1.0 / matrix[0]
        } else {
            0.0
        };
        return Ok((
            [(old[0] - mass * velocity[0]).max(0.0), 0.0],
            BlockActiveSetV1::Scalar,
        ));
    }
    let [k11, k12, _, k22] = matrix;
    let determinant = k11 * k22 - k12 * k12;
    if determinant > 0.0 {
        let delta0 = (-k22 * velocity[0] + k12 * velocity[1]) / determinant;
        let delta1 = (k12 * velocity[0] - k11 * velocity[1]) / determinant;
        let candidate = [old[0] + delta0, old[1] + delta1];
        if candidate[0] >= 0.0 && candidate[1] >= 0.0 {
            return Ok((candidate, BlockActiveSetV1::Both));
        }
    }

    if k11 > 0.0 {
        let delta1 = -old[1];
        let delta0 = -(velocity[0] + k12 * delta1) / k11;
        let candidate0 = old[0] + delta0;
        let inactive_velocity = velocity[1] + k12 * delta0 + k22 * delta1;
        if candidate0 >= 0.0 && inactive_velocity >= 0.0 {
            return Ok(([candidate0, 0.0], BlockActiveSetV1::First));
        }
    }

    if k22 > 0.0 {
        let delta0 = -old[0];
        let delta1 = -(velocity[1] + k12 * delta0) / k22;
        let candidate1 = old[1] + delta1;
        let inactive_velocity = velocity[0] + k11 * delta0 + k12 * delta1;
        if candidate1 >= 0.0 && inactive_velocity >= 0.0 {
            return Ok(([0.0, candidate1], BlockActiveSetV1::Second));
        }
    }

    let delta = [-old[0], -old[1]];
    let velocity0 = velocity[0] + k11 * delta[0] + k12 * delta[1];
    let velocity1 = velocity[1] + k12 * delta[0] + k22 * delta[1];
    if velocity0 >= 0.0 && velocity1 >= 0.0 {
        return Ok(([0.0, 0.0], BlockActiveSetV1::None));
    }
    Err("two-point active-set solve found no complementarity case".to_owned())
}

fn impulses(manifold: &PreparedManifold) -> [f32; 2] {
    [
        manifold.points[0].impulse,
        manifold.points.get(1).map_or(0.0, |point| point.impulse),
    ]
}

fn normal_velocities(bodies: &[BodyStateV1], manifold: &PreparedManifold) -> [f32; 2] {
    let velocity = |point: &PreparedPoint| {
        dot(
            sub(
                point_velocity(&bodies[manifold.body_b], point.anchor_b),
                point_velocity(&bodies[manifold.body_a], point.anchor_a),
            ),
            manifold.normal,
        )
    };
    [
        velocity(&manifold.points[0]),
        manifold.points.get(1).map_or(0.0, velocity),
    ]
}

fn complementarity_residual(bodies: &[BodyStateV1], manifolds: &[PreparedManifold]) -> f32 {
    let mut sum = 0.0;
    for manifold in manifolds {
        let velocities = normal_velocities(bodies, manifold);
        for (index, point) in manifold.points.iter().enumerate() {
            let residual = if point.impulse > 0.0 {
                velocities[index].abs()
            } else {
                (-velocities[index]).max(0.0)
            };
            sum += residual * residual;
        }
    }
    sum.sqrt()
}

fn max_closing_speed(bodies: &[BodyStateV1], manifolds: &[PreparedManifold]) -> f32 {
    manifolds
        .iter()
        .flat_map(|manifold| {
            let velocities = normal_velocities(bodies, manifold);
            velocities.into_iter().take(manifold.points.len())
        })
        .fold(0.0_f32, |maximum, velocity| {
            maximum.max((-velocity).max(0.0))
        })
}

fn kinetic_energy(bodies: &[BodyStateV1]) -> f32 {
    bodies.iter().fold(0.0, |energy, body| {
        let inverse_mass = body.inverse_mass.get();
        let inverse_inertia = body.inverse_inertia.get();
        let linear = if inverse_mass > 0.0 {
            0.5 * dot(body.linear_velocity, body.linear_velocity) / inverse_mass
        } else {
            0.0
        };
        let angular = if inverse_inertia > 0.0 {
            0.5 * body.angular_velocity.get().powi(2) / inverse_inertia
        } else {
            0.0
        };
        energy + linear + angular
    })
}

fn apply_impulse(body: &mut BodyStateV1, impulse: Vec2, anchor: Vec2) -> Result<(), String> {
    body.linear_velocity.x += impulse.x * body.inverse_mass.get();
    body.linear_velocity.y += impulse.y * body.inverse_mass.get();
    body.angular_velocity = FiniteF32::new(
        body.angular_velocity.get() + body.inverse_inertia.get() * cross(anchor, impulse),
    )
    .map_err(|error| format!("normal impulse produced invalid angular velocity: {error}"))?;
    Ok(())
}

fn validate_finite_bodies(bodies: &[BodyStateV1]) -> Result<(), String> {
    for body in bodies {
        body.validate().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn validate_metric(name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!(
            "frozen normal diagnostic produced invalid {name}: {value}"
        ))
    }
}

fn point_velocity(body: &BodyStateV1, anchor: Vec2) -> Vec2 {
    Vec2::new(
        body.linear_velocity.x - body.angular_velocity.get() * anchor.y,
        body.linear_velocity.y + body.angular_velocity.get() * anchor.x,
    )
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let cosine = angle.cos();
    let sine = angle.sin();
    Vec2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn scale(vector: Vec2, scalar: f32) -> Vec2 {
    Vec2::new(vector.x * scalar, vector.y * scalar)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_incidence_distinguishes_points_from_manifolds() {
        let diagnostic =
            frozen_rectangle_stack_diagnostic(2, ParallelNormalModeV1::ScalarMassSplit, 1).unwrap();
        assert_eq!(
            diagnostic
                .incidence
                .iter()
                .map(|body| (body.point_count, body.manifold_count))
                .collect::<Vec<_>>(),
            vec![(2, 1), (4, 2), (2, 1)]
        );
    }

    #[test]
    fn split_mass_changes_preparation_but_applies_full_physical_mass() {
        let baseline =
            frozen_rectangle_stack_diagnostic(2, ParallelNormalModeV1::PointJacobi, 1).unwrap();
        let split =
            frozen_rectangle_stack_diagnostic(2, ParallelNormalModeV1::ScalarMassSplit, 1).unwrap();
        let baseline_matrix = baseline.iterations[0].manifolds[0].effective_mass_matrix;
        let split_matrix = split.iterations[0].manifolds[0].effective_mass_matrix;
        assert_eq!(
            split_matrix[0].to_bits(),
            (baseline_matrix[0] * 4.0).to_bits()
        );
        let dynamic_bodies = split
            .bodies
            .iter()
            .map(|body| (body.id, body.inverse_mass.get()))
            .collect::<BTreeMap<_, _>>();
        let external_impulse = split.iterations[0]
            .manifolds
            .iter()
            .filter_map(|manifold| {
                let inverse_mass_a = dynamic_bodies[&manifold.body_a];
                let inverse_mass_b = dynamic_bodies[&manifold.body_b];
                if (inverse_mass_a == 0.0) == (inverse_mass_b == 0.0) {
                    return None;
                }
                let impulse = manifold.impulse_after[0] + manifold.impulse_after[1];
                let direction = if inverse_mass_a == 0.0 { 1.0 } else { -1.0 };
                Some(direction * manifold.normal.y * impulse)
            })
            .sum::<f32>();
        let initial_momentum = -2.0 * 10.0 / 240.0;
        let final_momentum = split
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0)
            .map(|body| body.linear_velocity.y / body.inverse_mass.get())
            .sum::<f32>();
        assert!((final_momentum - initial_momentum - external_impulse).abs() < 1.0e-6);
    }

    #[test]
    fn block_mass_split_uses_manifold_count_and_preserves_symmetric_torque() {
        let (projection, _) = rectangle_stack_projection(2, 0.0).unwrap();
        let baseline =
            solve_frozen_normal_system(&projection, ParallelNormalModeV1::ManifoldBlock, 1)
                .unwrap();
        let split =
            solve_frozen_normal_system(&projection, ParallelNormalModeV1::BlockMassSplit, 1)
                .unwrap();
        let baseline_matrix = baseline.iterations[0].manifolds[0].effective_mass_matrix;
        let split_matrix = split.iterations[0].manifolds[0].effective_mass_matrix;
        assert_eq!(
            split_matrix[0].to_bits(),
            (baseline_matrix[0] * 2.0).to_bits()
        );
        for body in &split.bodies {
            assert!(body.angular_velocity.get().abs() < 1.0e-6);
        }
    }

    #[test]
    fn mass_splitting_turns_deep_chain_residual_toward_contraction() {
        let jacobi =
            frozen_rectangle_stack_diagnostic(16, ParallelNormalModeV1::PointJacobi, 8).unwrap();
        let split = frozen_rectangle_stack_diagnostic(16, ParallelNormalModeV1::ScalarMassSplit, 8)
            .unwrap();
        assert!(jacobi.iterations.last().unwrap().residual_ratio > 1.0);
        assert!(
            split
                .iterations
                .iter()
                .all(|sample| sample.residual_ratio < 1.0)
        );
        assert!(
            split
                .iterations
                .last()
                .unwrap()
                .complementarity_residual_after
                < jacobi
                    .iterations
                    .last()
                    .unwrap()
                    .complementarity_residual_after
        );
        assert!(split.iterations.last().unwrap().residual_ratio < 1.0);
    }

    #[test]
    fn frozen_rocking_probe_replays_exactly() {
        let first =
            frozen_rectangle_stack_diagnostic(16, ParallelNormalModeV1::BlockMassSplit, 8).unwrap();
        let second =
            frozen_rectangle_stack_diagnostic(16, ParallelNormalModeV1::BlockMassSplit, 8).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.applied_rocking_angular_speed.to_bits(),
            ROCKING_PROBE_ANGULAR_SPEED.to_bits()
        );
    }

    #[test]
    fn frozen_probe_covers_every_governed_stack_depth_and_mode() {
        for count in [2, 4, 8, 16] {
            for mode in [
                ParallelNormalModeV1::PointJacobi,
                ParallelNormalModeV1::ScalarMassSplit,
                ParallelNormalModeV1::ManifoldBlock,
                ParallelNormalModeV1::BlockMassSplit,
            ] {
                let diagnostic = frozen_rectangle_stack_diagnostic(count, mode, 8).unwrap();
                assert_eq!(diagnostic.rectangle_count, Some(count));
                assert_eq!(diagnostic.mode, mode);
                assert_eq!(diagnostic.iterations.len(), 8);
            }
        }
    }

    #[test]
    fn block_active_set_enumerates_all_complementarity_cases() {
        let matrix = [2.0, 0.5, 0.5, 2.0];
        for (velocity, expected) in [
            ([-1.0, -1.0], BlockActiveSetV1::Both),
            ([-1.0, 1.0], BlockActiveSetV1::First),
            ([1.0, -1.0], BlockActiveSetV1::Second),
            ([1.0, 1.0], BlockActiveSetV1::None),
        ] {
            assert_eq!(
                solve_block_lcp(matrix, velocity, [0.0; 2], 2).unwrap().1,
                expected
            );
        }
    }

    #[test]
    fn one_point_block_degenerates_to_scalar_projected_solve() {
        let matrix = [2.0, 0.0, 0.0, 0.0];
        let (impulse, active_set) = solve_block_lcp(matrix, [-1.0, 0.0], [0.25, 0.0], 1).unwrap();
        assert_eq!(active_set, BlockActiveSetV1::Scalar);
        assert_eq!(impulse[0].to_bits(), 0.75_f32.to_bits());
        assert_eq!(impulse[1].to_bits(), 0.0_f32.to_bits());
    }
}
