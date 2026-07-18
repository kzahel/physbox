use physbox_contracts::{BodyStateV1, ScenarioV1, Vec2};
use physbox_engine::{
    GpuColoringEvidenceV4, GpuFixedFloorContactEvidence, GpuGeneratedContactPassTraceV2,
    GpuSolverPassDescriptorV1, GpuSolverPassKindV1,
};
use serde::Serialize;

pub const SETTLING_PASS_ENERGY_REPORT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KineticEnergyAccountingV1 {
    pub ordered_f32: f32,
    pub compensated_f64: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScalarPassCoverageV1 {
    Mirrored,
    PreparedContactBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarStateDifferenceV1 {
    pub compared_f32_values: u32,
    pub exact_f32_values: u32,
    pub max_absolute_difference: f32,
    pub max_ulp_difference: u32,
    pub max_ulp_field: Option<&'static str>,
    pub max_ulp_index: Option<u32>,
}

impl ScalarStateDifferenceV1 {
    pub const fn is_exact(self) -> bool {
        self.compared_f32_values == self.exact_f32_values
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyRowV1 {
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub coverage: ScalarPassCoverageV1,
    pub gpu_before: KineticEnergyAccountingV1,
    pub gpu_after: KineticEnergyAccountingV1,
    pub gpu_delta_ordered_f32: f32,
    pub gpu_delta_compensated_f64: f64,
    pub scalar_after: KineticEnergyAccountingV1,
    pub scalar_delta_ordered_f32: f32,
    pub scalar_delta_compensated_f64: f64,
    pub state_difference: ScalarStateDifferenceV1,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub adapter: physbox_engine::AdapterDiagnostics,
    pub matched_state_precision: &'static str,
    pub energy_accounting: [&'static str; 2],
    pub rows: Vec<SettlingPassEnergyRowV1>,
    pub first_non_exact_pass: Option<u32>,
    pub maximum_ulp_difference: u32,
    pub maximum_ulp_pass: Option<u32>,
}

impl SettlingPassEnergyReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SETTLING_PASS_ENERGY_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.matched_state_precision != "f32"
            || self.rows.is_empty()
        {
            return Err("settling pass-energy report identity is invalid".to_owned());
        }
        for (index, row) in self.rows.iter().enumerate() {
            if row.pass_index != index as u32
                || row.state_difference.compared_f32_values == 0
                || !row.gpu_before.ordered_f32.is_finite()
                || !row.gpu_before.compensated_f64.is_finite()
                || !row.gpu_after.ordered_f32.is_finite()
                || !row.gpu_after.compensated_f64.is_finite()
                || !row.scalar_after.ordered_f32.is_finite()
                || !row.scalar_after.compensated_f64.is_finite()
            {
                return Err("settling pass-energy row is invalid".to_owned());
            }
        }
        let first_non_exact = self
            .rows
            .iter()
            .find(|row| !row.state_difference.is_exact())
            .map(|row| row.pass_index);
        let maximum = self
            .rows
            .iter()
            .map(|row| row.state_difference.max_ulp_difference)
            .max()
            .unwrap_or(0);
        let maximum_pass = self
            .rows
            .iter()
            .max_by_key(|row| row.state_difference.max_ulp_difference)
            .filter(|row| row.state_difference.max_ulp_difference != 0)
            .map(|row| row.pass_index);
        if self.first_non_exact_pass != first_non_exact
            || self.maximum_ulp_difference != maximum
            || self.maximum_ulp_pass != maximum_pass
        {
            return Err("settling pass-energy report summary is inconsistent".to_owned());
        }
        Ok(())
    }
}

pub fn analyze_settling_pass_energy(
    scenario: &ScenarioV1,
    trace: &GpuGeneratedContactPassTraceV2,
) -> Result<SettlingPassEnergyReportV1, String> {
    trace.validate()?;
    if scenario.bodies.len() != trace.states[0].bodies.len()
        || trace.descriptors.len() + 1 != trace.states.len()
    {
        return Err("settling pass-energy scenario and trace disagree".to_owned());
    }
    let source_hash =
        physbox_contracts::canonical_hash(scenario).map_err(|error| error.to_string())?;
    if source_hash != trace.source_scenario_hash {
        return Err("settling pass-energy source hash does not match the trace".to_owned());
    }

    let mut rows = Vec::with_capacity(trace.descriptors.len());
    for (pass_index, ((before, after), descriptor)) in trace
        .states
        .windows(2)
        .map(|states| (&states[0], &states[1]))
        .zip(&trace.descriptors)
        .enumerate()
    {
        let gpu_before = kinetic_energy(&before.bodies);
        let gpu_after = kinetic_energy(&after.bodies);
        let (predicted_bodies, predicted_contacts, coverage) = match descriptor.kind {
            GpuSolverPassKindV1::PrepareContacts | GpuSolverPassKindV1::RefreshContacts => (
                before.bodies.clone(),
                after.contacts.clone(),
                ScalarPassCoverageV1::PreparedContactBoundary,
            ),
            _ => {
                let mut bodies = before.bodies.clone();
                let mut contacts = before.contacts.clone();
                replay_pass(&mut bodies, &mut contacts, descriptor, &trace.coloring)?;
                (bodies, contacts, ScalarPassCoverageV1::Mirrored)
            }
        };
        let scalar_after = kinetic_energy(&predicted_bodies);
        let state_difference = compare_state(
            &predicted_bodies,
            &after.bodies,
            &predicted_contacts,
            &after.contacts,
            coverage == ScalarPassCoverageV1::Mirrored,
        )?;
        rows.push(SettlingPassEnergyRowV1 {
            pass_index: pass_index as u32,
            descriptor: *descriptor,
            coverage,
            gpu_before,
            gpu_after,
            gpu_delta_ordered_f32: gpu_after.ordered_f32 - gpu_before.ordered_f32,
            gpu_delta_compensated_f64: gpu_after.compensated_f64 - gpu_before.compensated_f64,
            scalar_after,
            scalar_delta_ordered_f32: scalar_after.ordered_f32 - gpu_before.ordered_f32,
            scalar_delta_compensated_f64: scalar_after.compensated_f64 - gpu_before.compensated_f64,
            state_difference,
        });
    }
    let first_non_exact_pass = rows
        .iter()
        .find(|row| !row.state_difference.is_exact())
        .map(|row| row.pass_index);
    let maximum_ulp_difference = rows
        .iter()
        .map(|row| row.state_difference.max_ulp_difference)
        .max()
        .unwrap_or(0);
    let maximum_ulp_pass = rows
        .iter()
        .max_by_key(|row| row.state_difference.max_ulp_difference)
        .filter(|row| row.state_difference.max_ulp_difference != 0)
        .map(|row| row.pass_index);
    let report = SettlingPassEnergyReportV1 {
        version: SETTLING_PASS_ENERGY_REPORT_VERSION,
        source_scenario_hash: source_hash,
        target_step: trace.target_step,
        adapter: trace.adapter.clone(),
        matched_state_precision: "f32",
        energy_accounting: ["strict-ordered-f32", "compensated-f64-over-f32-state"],
        rows,
        first_non_exact_pass,
        maximum_ulp_difference,
        maximum_ulp_pass,
    };
    report.validate()?;
    Ok(report)
}

pub fn kinetic_energy(bodies: &[BodyStateV1]) -> KineticEnergyAccountingV1 {
    let mut ordered = 0.0_f32;
    let mut compensated = 0.0_f64;
    let mut correction = 0.0_f64;
    for body in bodies {
        let inverse_mass = body.inverse_mass.get();
        if inverse_mass <= 0.0 {
            continue;
        }
        let mass = inverse_mass.recip();
        let inverse_inertia = body.inverse_inertia.get();
        let inertia = if inverse_inertia > 0.0 {
            inverse_inertia.recip()
        } else {
            0.0
        };
        let linear_squared = body.linear_velocity.x * body.linear_velocity.x
            + body.linear_velocity.y * body.linear_velocity.y;
        let angular_squared = body.angular_velocity.get() * body.angular_velocity.get();
        let term = 0.5_f32 * mass * linear_squared + 0.5_f32 * inertia * angular_squared;
        ordered += term;

        let vx = f64::from(body.linear_velocity.x);
        let vy = f64::from(body.linear_velocity.y);
        let angular = f64::from(body.angular_velocity.get());
        let wider_term = 0.5 * f64::from(mass) * (vx * vx + vy * vy)
            + 0.5 * f64::from(inertia) * angular * angular;
        let adjusted = wider_term - correction;
        let next = compensated + adjusted;
        correction = (next - compensated) - adjusted;
        compensated = next;
    }
    KineticEnergyAccountingV1 {
        ordered_f32: ordered,
        compensated_f64: compensated,
    }
}

fn replay_pass(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
) -> Result<(), String> {
    match descriptor.kind {
        GpuSolverPassKindV1::IntegrateVelocity => {
            let dt = descriptor.parameters.floats0[0];
            let gravity_x = descriptor.parameters.floats1[1];
            let gravity_y = descriptor.parameters.floats1[2];
            for body in bodies {
                if body.inverse_mass.get() > 0.0 {
                    body.linear_velocity.x += gravity_x * dt;
                    body.linear_velocity.y += gravity_y * dt;
                }
            }
        }
        GpuSolverPassKindV1::IntegratePosition => {
            let dt = descriptor.parameters.floats0[0];
            for body in bodies {
                if body.inverse_mass.get() > 0.0 {
                    body.position.x += body.linear_velocity.x * dt;
                    body.position.y += body.linear_velocity.y * dt;
                    let angle = body.angle.get() + body.angular_velocity.get() * dt;
                    body.angle = physbox_contracts::FiniteF32::new(angle)
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        GpuSolverPassKindV1::WarmStart
        | GpuSolverPassKindV1::NormalBiased
        | GpuSolverPassKindV1::TangentBiased
        | GpuSolverPassKindV1::NormalRelaxation
        | GpuSolverPassKindV1::TangentRelaxation
        | GpuSolverPassKindV1::Restitution => {
            replay_colored_manifolds(bodies, contacts, descriptor, coloring)?;
        }
        GpuSolverPassKindV1::Gather => {
            return Err("selected pass-energy recurrence does not admit gather".to_owned());
        }
        GpuSolverPassKindV1::PrepareContacts | GpuSolverPassKindV1::RefreshContacts => {
            return Err(
                "prepared-contact boundaries must be synchronized by the caller".to_owned(),
            );
        }
    }
    Ok(())
}

fn replay_colored_manifolds(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
) -> Result<(), String> {
    if descriptor.first_color != Some(0)
        || descriptor.color_passes != coloring.max_colors
        || coloring.compacted_point_indices.len()
            != coloring
                .color_occupancy
                .iter()
                .map(|&value| value as usize)
                .sum::<usize>()
    {
        return Err("scalar recurrence requires the complete compact color schedule".to_owned());
    }
    let mut offset = 0_usize;
    for &occupancy in &coloring.color_occupancy {
        let end = offset + occupancy as usize;
        for &point_index in &coloring.compacted_point_indices[offset..end] {
            solve_manifold(bodies, contacts, point_index as usize, descriptor)?;
        }
        offset = end;
    }
    Ok(())
}

fn solve_manifold(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    point_index: usize,
    descriptor: &GpuSolverPassDescriptorV1,
) -> Result<(), String> {
    let Some(point0) = contacts.get(point_index) else {
        return Err("scalar recurrence color points outside contact capacity".to_owned());
    };
    if !point0.valid || point0.point_in_manifold != 0 {
        return Err(
            "scalar recurrence color unit is not a valid manifold representative".to_owned(),
        );
    }
    let has_second = contacts.get(point_index + 1).is_some_and(|point1| {
        point1.valid
            && point1.body_a == point0.body_a
            && point1.body_b == point0.body_b
            && point1.manifold_index == point0.manifold_index
            && point1.point_in_manifold == 1
    });
    let body_a = point0.body_a as usize;
    let body_b = point0.body_b as usize;
    if body_a == body_b || body_a >= bodies.len() || body_b >= bodies.len() {
        return Err("scalar recurrence contact bodies are invalid".to_owned());
    }
    let mut motion_a = motion(&bodies[body_a]);
    let mut motion_b = motion(&bodies[body_b]);

    match descriptor.kind {
        GpuSolverPassKindV1::WarmStart => {
            let point = contacts[point_index].clone();
            let impulse = add(
                scale(point.normal, point.normal_impulse),
                scale(point.tangent, point.tangent_impulse),
            );
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let impulse = add(
                    scale(point.normal, point.normal_impulse),
                    scale(point.tangent, point.tangent_impulse),
                );
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        GpuSolverPassKindV1::NormalBiased | GpuSolverPassKindV1::NormalRelaxation => {
            let point0 = contacts[point_index].clone();
            let point1 = has_second.then(|| contacts[point_index + 1].clone());
            let matrix = block_matrix(&bodies[body_a], &bodies[body_b], &point0, point1.as_ref());
            let old = [
                point0.normal_impulse,
                point1.as_ref().map_or(0.0, |point| point.normal_impulse),
            ];
            let equation0 = normal_equation(
                &bodies[body_a],
                &bodies[body_b],
                &point0,
                motion_a,
                motion_b,
                descriptor,
            );
            let equation1 = point1.as_ref().map_or([0.0, 0.0], |point| {
                normal_equation(
                    &bodies[body_a],
                    &bodies[body_b],
                    point,
                    motion_a,
                    motion_b,
                    descriptor,
                )
            });
            let solution = solve_normal_block(
                matrix,
                [equation0[0], equation1[0]],
                old,
                [equation0[1], equation1[1]],
                has_second,
            );
            let applied0 = solution[0] - old[0];
            contacts[point_index].normal_impulse = solution[0];
            contacts[point_index].total_normal_impulse += applied0.max(0.0);
            let impulse0 = scale(point0.normal, applied0);
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse0, -1.0),
                point0.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse0, point0.anchor_b);
            if let Some(point1) = point1 {
                let applied1 = solution[1] - old[1];
                contacts[point_index + 1].normal_impulse = solution[1];
                contacts[point_index + 1].total_normal_impulse += applied1.max(0.0);
                let impulse1 = scale(point1.normal, applied1);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse1, -1.0),
                    point1.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse1, point1.anchor_b);
            }
        }
        GpuSolverPassKindV1::TangentBiased | GpuSolverPassKindV1::TangentRelaxation => {
            let point = contacts[point_index].clone();
            let tangent_velocity = relative_velocity(&point, point.tangent, motion_a, motion_b);
            let old = point.tangent_impulse;
            let limit = point.friction * point.normal_impulse;
            let next = (old - point.tangent_effective_mass * tangent_velocity).clamp(-limit, limit);
            contacts[point_index].tangent_impulse = next;
            let impulse = scale(point.tangent, next - old);
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let tangent_velocity = relative_velocity(&point, point.tangent, motion_a, motion_b);
                let old = point.tangent_impulse;
                let limit = point.friction * point.normal_impulse;
                let next =
                    (old - point.tangent_effective_mass * tangent_velocity).clamp(-limit, limit);
                contacts[point_index + 1].tangent_impulse = next;
                let impulse = scale(point.tangent, next - old);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        GpuSolverPassKindV1::Restitution => {
            let point = contacts[point_index].clone();
            let normal_velocity = relative_velocity(&point, point.normal, motion_a, motion_b);
            let old = point.normal_impulse;
            let eligible = point.restitution > 0.0
                && point.initial_normal_velocity < -descriptor.parameters.floats1[0]
                && point.total_normal_impulse > 0.0;
            let next = if eligible {
                (old - point.normal_effective_mass
                    * (normal_velocity + point.restitution * point.initial_normal_velocity))
                    .max(0.0)
            } else {
                old
            };
            contacts[point_index].normal_impulse = next;
            let impulse = scale(point.normal, next - old);
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let normal_velocity = relative_velocity(&point, point.normal, motion_a, motion_b);
                let old = point.normal_impulse;
                let eligible = point.restitution > 0.0
                    && point.initial_normal_velocity < -descriptor.parameters.floats1[0]
                    && point.total_normal_impulse > 0.0;
                let next = if eligible {
                    (old - point.normal_effective_mass
                        * (normal_velocity + point.restitution * point.initial_normal_velocity))
                        .max(0.0)
                } else {
                    old
                };
                contacts[point_index + 1].normal_impulse = next;
                let impulse = scale(point.normal, next - old);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        _ => return Err("scalar manifold recurrence received a non-contact pass".to_owned()),
    }

    set_motion(&mut bodies[body_a], motion_a)?;
    set_motion(&mut bodies[body_b], motion_b)?;
    Ok(())
}

fn normal_equation(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    point: &GpuFixedFloorContactEvidence,
    motion_a: [f32; 3],
    motion_b: [f32; 3],
    descriptor: &GpuSolverPassDescriptorV1,
) -> [f32; 2] {
    let normal_velocity = relative_velocity(point, point.normal, motion_a, motion_b);
    let mut velocity_bias = 0.0_f32;
    let mut mass_scale = 1.0_f32;
    let mut impulse_scale = 0.0_f32;
    let coefficients = if descriptor.parameters.floats3[3] != 0.0
        && (body_a.inverse_mass.get() == 0.0 || body_b.inverse_mass.get() == 0.0)
    {
        &descriptor.parameters.floats3[..3]
    } else {
        &descriptor.parameters.floats0[1..4]
    };
    if point.separation > 0.0 {
        velocity_bias = point.separation / descriptor.parameters.floats0[0];
    } else if descriptor.parameters.uints1[3] != 0 || descriptor.parameters.floats2[3] != 0.0 {
        if descriptor.parameters.uints1[3] != 0 {
            velocity_bias = coefficients[0] * point.separation / descriptor.parameters.floats0[0];
            if descriptor.parameters.floats2[0] > 0.0 {
                velocity_bias = velocity_bias.max(-descriptor.parameters.floats2[0]);
            }
        }
        mass_scale = coefficients[1];
        impulse_scale = coefficients[2];
    }
    [mass_scale * normal_velocity + velocity_bias, impulse_scale]
}

fn block_matrix(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    point0: &GpuFixedFloorContactEvidence,
    point1: Option<&GpuFixedFloorContactEvidence>,
) -> [f32; 4] {
    let k11 = block_matrix_entry(body_a, body_b, point0, point0);
    point1.map_or([k11, 0.0, 0.0, 0.0], |point1| {
        [
            k11,
            block_matrix_entry(body_a, body_b, point0, point1),
            block_matrix_entry(body_a, body_b, point1, point0),
            block_matrix_entry(body_a, body_b, point1, point1),
        ]
    })
}

fn block_matrix_entry(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    left: &GpuFixedFloorContactEvidence,
    right: &GpuFixedFloorContactEvidence,
) -> f32 {
    body_a.inverse_mass.get()
        + body_b.inverse_mass.get()
        + body_a.inverse_inertia.get()
            * cross(left.anchor_a, left.normal)
            * cross(right.anchor_a, right.normal)
        + body_b.inverse_inertia.get()
            * cross(left.anchor_b, left.normal)
            * cross(right.anchor_b, right.normal)
}

fn solve_normal_block(
    matrix: [f32; 4],
    base: [f32; 2],
    old: [f32; 2],
    impulse_scale: [f32; 2],
    has_second: bool,
) -> [f32; 2] {
    if !has_second {
        let mass = if matrix[0] > 0.0 {
            matrix[0].recip()
        } else {
            0.0
        };
        return [
            (old[0] - mass * base[0] - impulse_scale[0] * old[0]).max(0.0),
            0.0,
        ];
    }
    let scaled_old = [impulse_scale[0] * old[0], impulse_scale[1] * old[1]];
    let rhs = [
        base[0] + matrix[0] * scaled_old[0] + matrix[1] * scaled_old[1],
        base[1] + matrix[2] * scaled_old[0] + matrix[3] * scaled_old[1],
    ];
    let determinant = matrix[0] * matrix[3] - matrix[1] * matrix[2];
    if determinant > 0.0 {
        let delta = [
            (-matrix[3] * rhs[0] + matrix[1] * rhs[1]) / determinant,
            (matrix[2] * rhs[0] - matrix[0] * rhs[1]) / determinant,
        ];
        let candidate = [old[0] + delta[0], old[1] + delta[1]];
        if candidate[0] >= 0.0 && candidate[1] >= 0.0 {
            return candidate;
        }
    }
    if matrix[0] > 0.0 {
        let delta1 = -old[1];
        let delta0 = -(rhs[0] + matrix[1] * delta1) / matrix[0];
        let candidate0 = old[0] + delta0;
        let inactive_velocity = rhs[1] + matrix[2] * delta0 + matrix[3] * delta1;
        if candidate0 >= 0.0 && inactive_velocity >= 0.0 {
            return [candidate0, 0.0];
        }
    }
    if matrix[3] > 0.0 {
        let delta0 = -old[0];
        let delta1 = -(rhs[1] + matrix[2] * delta0) / matrix[3];
        let candidate1 = old[1] + delta1;
        let inactive_velocity = rhs[0] + matrix[0] * delta0 + matrix[1] * delta1;
        if candidate1 >= 0.0 && inactive_velocity >= 0.0 {
            return [0.0, candidate1];
        }
    }
    let delta = [-old[0], -old[1]];
    let velocity = [
        rhs[0] + matrix[0] * delta[0] + matrix[1] * delta[1],
        rhs[1] + matrix[2] * delta[0] + matrix[3] * delta[1],
    ];
    if velocity[0] >= 0.0 && velocity[1] >= 0.0 {
        [0.0, 0.0]
    } else {
        old
    }
}

fn motion(body: &BodyStateV1) -> [f32; 3] {
    [
        body.linear_velocity.x,
        body.linear_velocity.y,
        body.angular_velocity.get(),
    ]
}

fn set_motion(body: &mut BodyStateV1, motion: [f32; 3]) -> Result<(), String> {
    body.linear_velocity = Vec2::new(motion[0], motion[1]);
    body.angular_velocity =
        physbox_contracts::FiniteF32::new(motion[2]).map_err(|error| error.to_string())?;
    Ok(())
}

fn point_velocity(motion: [f32; 3], anchor: Vec2) -> Vec2 {
    Vec2::new(
        motion[0] - motion[2] * anchor.y,
        motion[1] + motion[2] * anchor.x,
    )
}

fn relative_velocity(
    point: &GpuFixedFloorContactEvidence,
    axis: Vec2,
    motion_a: [f32; 3],
    motion_b: [f32; 3],
) -> f32 {
    let velocity_a = point_velocity(motion_a, point.anchor_a);
    let velocity_b = point_velocity(motion_b, point.anchor_b);
    dot(
        Vec2::new(velocity_b.x - velocity_a.x, velocity_b.y - velocity_a.y),
        axis,
    )
}

fn apply_impulse(
    body: &BodyStateV1,
    mut motion: [f32; 3],
    impulse: Vec2,
    anchor: Vec2,
) -> [f32; 3] {
    motion[0] += impulse.x * body.inverse_mass.get();
    motion[1] += impulse.y * body.inverse_mass.get();
    motion[2] += body.inverse_inertia.get() * cross(anchor, impulse);
    motion
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn scale(value: Vec2, scalar: f32) -> Vec2 {
    Vec2::new(value.x * scalar, value.y * scalar)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn compare_state(
    predicted_bodies: &[BodyStateV1],
    gpu_bodies: &[BodyStateV1],
    predicted_contacts: &[GpuFixedFloorContactEvidence],
    gpu_contacts: &[GpuFixedFloorContactEvidence],
    compare_contacts: bool,
) -> Result<ScalarStateDifferenceV1, String> {
    if predicted_bodies.len() != gpu_bodies.len()
        || (compare_contacts && predicted_contacts.len() != gpu_contacts.len())
    {
        return Err("scalar/GPU state shapes disagree".to_owned());
    }
    let mut difference = ScalarStateDifferenceV1 {
        compared_f32_values: 0,
        exact_f32_values: 0,
        max_absolute_difference: 0.0,
        max_ulp_difference: 0,
        max_ulp_field: None,
        max_ulp_index: None,
    };
    for (index, (predicted, gpu)) in predicted_bodies.iter().zip(gpu_bodies).enumerate() {
        for (field, left, right) in [
            ("body.position.x", predicted.position.x, gpu.position.x),
            ("body.position.y", predicted.position.y, gpu.position.y),
            ("body.angle", predicted.angle.get(), gpu.angle.get()),
            (
                "body.linearVelocity.x",
                predicted.linear_velocity.x,
                gpu.linear_velocity.x,
            ),
            (
                "body.linearVelocity.y",
                predicted.linear_velocity.y,
                gpu.linear_velocity.y,
            ),
            (
                "body.angularVelocity",
                predicted.angular_velocity.get(),
                gpu.angular_velocity.get(),
            ),
        ] {
            include_difference(&mut difference, field, index as u32, left, right);
        }
    }
    if compare_contacts {
        for (index, (predicted, gpu)) in predicted_contacts.iter().zip(gpu_contacts).enumerate() {
            for (field, left, right) in [
                ("contact.normal.x", predicted.normal.x, gpu.normal.x),
                ("contact.normal.y", predicted.normal.y, gpu.normal.y),
                ("contact.tangent.x", predicted.tangent.x, gpu.tangent.x),
                ("contact.tangent.y", predicted.tangent.y, gpu.tangent.y),
                ("contact.anchorA.x", predicted.anchor_a.x, gpu.anchor_a.x),
                ("contact.anchorA.y", predicted.anchor_a.y, gpu.anchor_a.y),
                ("contact.anchorB.x", predicted.anchor_b.x, gpu.anchor_b.x),
                ("contact.anchorB.y", predicted.anchor_b.y, gpu.anchor_b.y),
                (
                    "contact.initialNormalVelocity",
                    predicted.initial_normal_velocity,
                    gpu.initial_normal_velocity,
                ),
                (
                    "contact.normalEffectiveMass",
                    predicted.normal_effective_mass,
                    gpu.normal_effective_mass,
                ),
                (
                    "contact.tangentEffectiveMass",
                    predicted.tangent_effective_mass,
                    gpu.tangent_effective_mass,
                ),
                ("contact.separation", predicted.separation, gpu.separation),
                ("contact.friction", predicted.friction, gpu.friction),
                (
                    "contact.restitution",
                    predicted.restitution,
                    gpu.restitution,
                ),
                (
                    "contact.normalImpulse",
                    predicted.normal_impulse,
                    gpu.normal_impulse,
                ),
                (
                    "contact.tangentImpulse",
                    predicted.tangent_impulse,
                    gpu.tangent_impulse,
                ),
                (
                    "contact.totalNormalImpulse",
                    predicted.total_normal_impulse,
                    gpu.total_normal_impulse,
                ),
            ] {
                include_difference(&mut difference, field, index as u32, left, right);
            }
        }
    }
    Ok(difference)
}

fn include_difference(
    difference: &mut ScalarStateDifferenceV1,
    field: &'static str,
    index: u32,
    left: f32,
    right: f32,
) {
    difference.compared_f32_values += 1;
    if left.to_bits() == right.to_bits() {
        difference.exact_f32_values += 1;
        return;
    }
    difference.max_absolute_difference =
        difference.max_absolute_difference.max((left - right).abs());
    let ulp = ulp_difference(left, right);
    if ulp > difference.max_ulp_difference {
        difference.max_ulp_difference = ulp;
        difference.max_ulp_field = Some(field);
        difference.max_ulp_index = Some(index);
    }
}

fn ulp_difference(left: f32, right: f32) -> u32 {
    if left == right && left.to_bits() != right.to_bits() {
        return 1;
    }
    fn ordered(value: f32) -> i32 {
        let bits = value.to_bits() as i32;
        if bits < 0 { i32::MIN - bits } else { bits }
    }
    ordered(left).abs_diff(ordered(right))
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{BodyId, FiniteF32, MaterialV1, ShapeV1};

    use super::*;

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    #[test]
    fn kinetic_energy_reports_strict_and_compensated_accumulations() {
        let bodies = [BodyStateV1 {
            id: BodyId {
                slot: 0,
                generation: 1,
            },
            shape: ShapeV1::Circle {
                radius: finite(1.0),
            },
            position: Vec2::ZERO,
            angle: finite(0.0),
            linear_velocity: Vec2::new(3.0, 4.0),
            angular_velocity: finite(2.0),
            inverse_mass: finite(0.5),
            inverse_inertia: finite(0.25),
            material: MaterialV1 {
                friction: finite(0.5),
                restitution: finite(0.0),
            },
            sleeping: false,
        }];
        let energy = kinetic_energy(&bodies);
        assert_eq!(energy.ordered_f32, 33.0);
        assert_eq!(energy.compensated_f64, 33.0);
    }

    #[test]
    fn ulp_distance_handles_sign_and_adjacent_values() {
        assert_eq!(ulp_difference(-0.0, 0.0), 1);
        assert_eq!(
            ulp_difference(1.0, f32::from_bits(1.0_f32.to_bits() + 1)),
            1
        );
        assert_eq!(ulp_difference(-1.0, -1.0), 0);
    }
}
