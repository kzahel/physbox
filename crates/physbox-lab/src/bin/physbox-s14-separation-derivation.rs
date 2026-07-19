use std::{env, fs, path::PathBuf};

use physbox_contracts::{BodyStateV1, TraceStageV1};
use physbox_engine::{
    GPU_AUTHORITY_SEPARATION_DIAGNOSTIC_VERSION, GpuAuthoritySeparationInputV1,
    GpuGeneratedContactRunner, GpuSolverPassKindV1, p5_2_s11_soft_depth_sweep_scenario,
};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePassKindV1, AuthorityStructurePolicyV1,
        solve_authority_structure_one_step_pass_states,
    },
    branch_robust::{
        AuthoritySeparationInputsV1, BINARY32_UNIT_ROUNDOFF, BRANCH_ROBUST_CANDIDATE_ID,
        BRANCH_ROBUST_ERROR_BOUND_VERSION, BRANCH_ROBUST_FORMULA_VERSION,
        BranchRobustEvaluationTraceV1, evaluate_branch_robust_authority_separation_v1,
        evaluate_original_authority_separation_v1,
    },
};
use serde::Serialize;

const MANIFOLD_ID: u32 = 4;
const POINT_INDEX: u32 = 1;
const SUBSTEP: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RetainedValue {
    name: &'static str,
    value: f32,
    bits: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchOutcome {
    gpu_separation: RetainedValue,
    gpu_positive: bool,
    scalar_separation: RetainedValue,
    scalar_positive: bool,
    ordered_binary32_distance: u64,
    gpu_mass_scale: f32,
    gpu_impulse_scale: f32,
    scalar_mass_scale: f32,
    scalar_impulse_scale: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownstreamDifference {
    gpu_normal_impulse: RetainedValue,
    scalar_normal_impulse: RetainedValue,
    impulse_absolute_difference: f32,
    impulse_ordered_binary32_distance: u64,
    body_index: usize,
    body_field: &'static str,
    gpu_body_value: RetainedValue,
    scalar_body_value: RetainedValue,
    body_absolute_difference: f32,
    body_ordered_binary32_distance: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityCheck {
    gpu_original_real_shadow: f64,
    gpu_robust_real_shadow: f64,
    gpu_absolute_difference: f64,
    scalar_original_real_shadow: f64,
    scalar_robust_real_shadow: f64,
    scalar_absolute_difference: f64,
    retained_gpu_original_absolute_error: f64,
    retained_gpu_original_error_bound: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DerivationEvidence {
    version: u32,
    candidate_id: &'static str,
    formula_version: u32,
    error_bound_version: u32,
    binary32_unit_roundoff: f64,
    gamma_definition: &'static str,
    gpu_diagnostic_version: u32,
    source_scenario_hash: String,
    adapter: physbox_engine::AdapterDiagnostics,
    location: Location,
    gpu_inputs: Vec<RetainedValue>,
    gpu_expression: physbox_engine::GpuAuthoritySeparationDiagnosticV1,
    gpu_original_bound: BranchRobustEvaluationTraceV1,
    gpu_robust_bound: BranchRobustEvaluationTraceV1,
    scalar_original_bound: BranchRobustEvaluationTraceV1,
    scalar_robust_bound: BranchRobustEvaluationTraceV1,
    identity_check: IdentityCheck,
    branch_outcome: BranchOutcome,
    first_downstream_difference: DownstreamDifference,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    outer_step: u32,
    substep: u32,
    gpu_pass: &'static str,
    scalar_pass: &'static str,
    manifold_id: u32,
    point_index: u32,
}

fn main() -> Result<(), String> {
    let output = output_path()?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenario = p5_2_s11_soft_depth_sweep_scenario(11).map_err(|error| error.to_string())?;
    scenario.requested_steps = 1;
    let gpu = runner.run_authority_gpu_probe_pass_trace(&scenario, 1)?;
    let (scalar, scalar_states) = solve_authority_structure_one_step_pass_states(
        &scenario,
        AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
    )
    .map_err(|error| error.to_string())?;

    let biased_index = gpu
        .descriptors
        .iter()
        .position(|descriptor| {
            descriptor.kind == GpuSolverPassKindV1::NormalBiased && descriptor.substep == SUBSTEP
        })
        .ok_or_else(|| "missing retained biased-normal pass".to_owned())?;
    let prepare_index = gpu
        .descriptors
        .iter()
        .position(|descriptor| descriptor.kind == GpuSolverPassKindV1::PrepareContacts)
        .ok_or_else(|| "missing retained prepare pass".to_owned())?;
    let gpu_input_state = &gpu.states[biased_index];
    let gpu_output_state = &gpu.states[biased_index + 1];
    let prepared_state = &gpu.states[prepare_index + 1];
    let gpu_input_contact = retained_contact(gpu_input_state)?;
    let gpu_output_contact = retained_contact(gpu_output_state)?;
    let prepared_contact = retained_contact(prepared_state)?;
    let body_a_index = gpu_input_contact.body_a as usize;
    let body_b_index = gpu_input_contact.body_b as usize;
    let initial_a = &scenario.bodies[body_a_index];
    let initial_b = &scenario.bodies[body_b_index];
    let gpu_body_a = &gpu_input_state.bodies[body_a_index];
    let gpu_body_b = &gpu_input_state.bodies[body_b_index];
    let gpu_formula_input = formula_input(
        gpu_input_contact,
        prepared_contact.separation,
        gpu_body_a,
        gpu_body_b,
        initial_a,
        initial_b,
    );
    let gpu_diagnostic_input = GpuAuthoritySeparationInputV1 {
        normal: gpu_formula_input.normal,
        anchor_a: gpu_formula_input.anchor_a,
        anchor_b: gpu_formula_input.anchor_b,
        pose_a: gpu_formula_input.position_a,
        pose_b: gpu_formula_input.position_b,
        initial_position_a: gpu_formula_input.initial_position_a,
        initial_position_b: gpu_formula_input.initial_position_b,
        angle_a: gpu_formula_input.angle_a,
        angle_b: gpu_formula_input.angle_b,
        initial_angle_a: gpu_formula_input.initial_angle_a,
        initial_angle_b: gpu_formula_input.initial_angle_b,
        input_separation: gpu_formula_input.input_separation,
    };
    let gpu_expression = runner.run_authority_separation_diagnostic(gpu_diagnostic_input)?;
    gpu_expression
        .values
        .iter()
        .find(|value| value.name == "native-original-separation")
        .ok_or_else(|| "diagnostic omitted original separation".to_owned())?;

    let scalar_input_state = scalar_states
        .iter()
        .find(|state| {
            state.substep == SUBSTEP && state.kind == AuthorityStructurePassKindV1::WarmStart
        })
        .ok_or_else(|| "missing scalar biased-normal input state".to_owned())?;
    let scalar_output_state = scalar_states
        .iter()
        .find(|state| {
            state.substep == SUBSTEP && state.kind == AuthorityStructurePassKindV1::BiasedSolve
        })
        .ok_or_else(|| "missing scalar biased-normal output state".to_owned())?;
    let scalar_formula_input = formula_input(
        gpu_input_contact,
        prepared_contact.separation,
        &scalar_input_state.bodies[body_a_index],
        &scalar_input_state.bodies[body_b_index],
        initial_a,
        initial_b,
    );
    let gpu_original_bound = evaluate_original_authority_separation_v1(gpu_formula_input);
    let gpu_robust_bound = evaluate_branch_robust_authority_separation_v1(gpu_formula_input);
    let scalar_original_bound = evaluate_original_authority_separation_v1(scalar_formula_input);
    let scalar_robust_bound = evaluate_branch_robust_authority_separation_v1(scalar_formula_input);
    let scalar_separation = scalar_original_bound.result.separation;
    let scalar_record = scalar
        .result
        .traces
        .iter()
        .find(|record| {
            record.substep == SUBSTEP
                && record.stage == TraceStageV1::NormalSolve
                && record.manifold_id == MANIFOLD_ID
                && record.point_index == POINT_INDEX
        })
        .ok_or_else(|| "missing retained scalar normal-solve trace".to_owned())?;
    let scalar_mass_scale = trace_value(scalar_record, "massScale")?;
    let scalar_impulse_scale = trace_value(scalar_record, "impulseScale")?;
    let scalar_normal_impulse = trace_value(scalar_record, "normalImpulse")?;
    if !(scalar_separation > 0.0 && scalar_mass_scale == 1.0 && scalar_impulse_scale == 0.0) {
        return Err("scalar replay did not retain the positive speculative branch".to_owned());
    }
    let gpu_positive = gpu_input_contact.separation > 0.0;
    let (gpu_mass_scale, gpu_impulse_scale) = if gpu_positive {
        (1.0, 0.0)
    } else {
        let h = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
        let coefficients = physbox_engine::gpu_contact_softness_coefficients(
            scenario.settings.contact_softness.unwrap(),
            h,
            false,
        )?;
        (coefficients.mass_scale, coefficients.impulse_scale)
    };
    if gpu_positive || gpu_input_contact.separation.to_bits() != 0 {
        return Err("GPU replay did not retain the zero non-positive branch".to_owned());
    }

    let (body_index, body_field, gpu_body_value, scalar_body_value) =
        maximum_body_delta(&gpu_output_state.bodies, &scalar_output_state.bodies);
    if body_index != 4 || body_field != "angular-velocity" {
        return Err(format!(
            "unexpected first downstream body maximum: body {body_index} {body_field}"
        ));
    }
    let identity_check = IdentityCheck {
        gpu_original_real_shadow: gpu_original_bound.result.real_shadow,
        gpu_robust_real_shadow: gpu_robust_bound.result.real_shadow,
        gpu_absolute_difference: (gpu_original_bound.result.real_shadow
            - gpu_robust_bound.result.real_shadow)
            .abs(),
        scalar_original_real_shadow: scalar_original_bound.result.real_shadow,
        scalar_robust_real_shadow: scalar_robust_bound.result.real_shadow,
        scalar_absolute_difference: (scalar_original_bound.result.real_shadow
            - scalar_robust_bound.result.real_shadow)
            .abs(),
        retained_gpu_original_absolute_error: (f64::from(gpu_input_contact.separation)
            - gpu_original_bound.result.real_shadow)
            .abs(),
        retained_gpu_original_error_bound: gpu_original_bound.result.absolute_error_bound,
    };
    if identity_check.gpu_absolute_difference > 1.0e-15
        || identity_check.scalar_absolute_difference > 1.0e-15
        || identity_check.retained_gpu_original_absolute_error
            > identity_check.retained_gpu_original_error_bound
    {
        return Err("real-equivalent identity check failed".to_owned());
    }
    for result in [
        gpu_original_bound.result,
        gpu_robust_bound.result,
        scalar_original_bound.result,
        scalar_robust_bound.result,
    ] {
        if !result.normal_range_assumption_satisfied
            || result.observed_absolute_error > result.absolute_error_bound
            || f64::from(result.lower_endpoint) > result.real_shadow
            || f64::from(result.upper_endpoint) < result.real_shadow
        {
            return Err("retained-sample forward bound or outward endpoint failed".to_owned());
        }
    }

    let evidence = DerivationEvidence {
        version: 1,
        candidate_id: BRANCH_ROBUST_CANDIDATE_ID,
        formula_version: BRANCH_ROBUST_FORMULA_VERSION,
        error_bound_version: BRANCH_ROBUST_ERROR_BOUND_VERSION,
        binary32_unit_roundoff: BINARY32_UNIT_ROUNDOFF,
        gamma_definition: "n*u/(1-n*u)",
        gpu_diagnostic_version: GPU_AUTHORITY_SEPARATION_DIAGNOSTIC_VERSION,
        source_scenario_hash: gpu.source_scenario_hash,
        adapter: gpu.adapter,
        location: Location {
            outer_step: 1,
            substep: SUBSTEP,
            gpu_pass: "normal-biased",
            scalar_pass: "biased-solve",
            manifold_id: MANIFOLD_ID,
            point_index: POINT_INDEX,
        },
        gpu_inputs: retained_inputs(gpu_formula_input),
        gpu_expression,
        gpu_original_bound,
        gpu_robust_bound,
        scalar_original_bound,
        scalar_robust_bound,
        identity_check,
        branch_outcome: BranchOutcome {
            gpu_separation: value("gpu-separation", gpu_input_contact.separation),
            gpu_positive,
            scalar_separation: value("scalar-separation", scalar_separation),
            scalar_positive: scalar_separation > 0.0,
            ordered_binary32_distance: ulp_distance(
                gpu_input_contact.separation,
                scalar_separation,
            ),
            gpu_mass_scale,
            gpu_impulse_scale,
            scalar_mass_scale,
            scalar_impulse_scale,
        },
        first_downstream_difference: DownstreamDifference {
            gpu_normal_impulse: value("gpu-normal-impulse", gpu_output_contact.normal_impulse),
            scalar_normal_impulse: value("scalar-normal-impulse", scalar_normal_impulse),
            impulse_absolute_difference: (gpu_output_contact.normal_impulse
                - scalar_normal_impulse)
                .abs(),
            impulse_ordered_binary32_distance: ulp_distance(
                gpu_output_contact.normal_impulse,
                scalar_normal_impulse,
            ),
            body_index,
            body_field,
            gpu_body_value: value("gpu-body-value", gpu_body_value),
            scalar_body_value: value("scalar-body-value", scalar_body_value),
            body_absolute_difference: (gpu_body_value - scalar_body_value).abs(),
            body_ordered_binary32_distance: ulp_distance(gpu_body_value, scalar_body_value),
        },
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &output,
        serde_json::to_vec_pretty(&evidence).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s14.2-separation-derivation output={}",
        output.display()
    );
    Ok(())
}

fn output_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: physbox-s14-separation-derivation --output PATH".to_owned()),
    }
}

fn retained_contact(
    state: &physbox_engine::GpuGeneratedContactPassStateV3,
) -> Result<&physbox_engine::GpuFixedFloorContactEvidence, String> {
    state
        .contacts
        .iter()
        .find(|contact| {
            contact.valid
                && contact.manifold_index == MANIFOLD_ID
                && contact.point_in_manifold == POINT_INDEX
        })
        .ok_or_else(|| "retained contact is missing".to_owned())
}

fn formula_input(
    contact: &physbox_engine::GpuFixedFloorContactEvidence,
    input_separation: f32,
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    initial_a: &BodyStateV1,
    initial_b: &BodyStateV1,
) -> AuthoritySeparationInputsV1 {
    AuthoritySeparationInputsV1 {
        normal: contact.normal,
        anchor_a: contact.anchor_a,
        anchor_b: contact.anchor_b,
        position_a: body_a.position,
        position_b: body_b.position,
        initial_position_a: initial_a.position,
        initial_position_b: initial_b.position,
        angle_a: body_a.angle.get(),
        angle_b: body_b.angle.get(),
        initial_angle_a: initial_a.angle.get(),
        initial_angle_b: initial_b.angle.get(),
        input_separation,
    }
}

fn retained_inputs(input: AuthoritySeparationInputsV1) -> Vec<RetainedValue> {
    [
        ("normal-x", input.normal.x),
        ("normal-y", input.normal.y),
        ("anchor-a-x", input.anchor_a.x),
        ("anchor-a-y", input.anchor_a.y),
        ("anchor-b-x", input.anchor_b.x),
        ("anchor-b-y", input.anchor_b.y),
        ("position-a-x", input.position_a.x),
        ("position-a-y", input.position_a.y),
        ("position-b-x", input.position_b.x),
        ("position-b-y", input.position_b.y),
        ("initial-position-a-x", input.initial_position_a.x),
        ("initial-position-a-y", input.initial_position_a.y),
        ("initial-position-b-x", input.initial_position_b.x),
        ("initial-position-b-y", input.initial_position_b.y),
        ("angle-a", input.angle_a),
        ("angle-b", input.angle_b),
        ("initial-angle-a", input.initial_angle_a),
        ("initial-angle-b", input.initial_angle_b),
        ("input-separation", input.input_separation),
    ]
    .into_iter()
    .map(|(name, number)| value(name, number))
    .collect()
}

fn trace_value(record: &physbox_contracts::TraceRecordV1, name: &str) -> Result<f32, String> {
    record
        .values
        .iter()
        .find(|value| value.name == name)
        .map(|value| value.value.get())
        .ok_or_else(|| format!("missing scalar trace value {name}"))
}

fn value(name: &'static str, number: f32) -> RetainedValue {
    RetainedValue {
        name,
        value: number,
        bits: number.to_bits(),
    }
}

fn ulp_distance(left: f32, right: f32) -> u64 {
    u64::from(ordered_bits(left).abs_diff(ordered_bits(right)))
}

fn ordered_bits(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

fn maximum_body_delta(
    gpu: &[BodyStateV1],
    scalar: &[BodyStateV1],
) -> (usize, &'static str, f32, f32) {
    let mut maximum = (0, "", 0.0, 0.0, 0.0);
    for (body_index, (gpu, scalar)) in gpu.iter().zip(scalar).enumerate() {
        for (field, gpu_value, scalar_value) in [
            ("position-x", gpu.position.x, scalar.position.x),
            ("position-y", gpu.position.y, scalar.position.y),
            ("angle", gpu.angle.get(), scalar.angle.get()),
            (
                "linear-velocity-x",
                gpu.linear_velocity.x,
                scalar.linear_velocity.x,
            ),
            (
                "linear-velocity-y",
                gpu.linear_velocity.y,
                scalar.linear_velocity.y,
            ),
            (
                "angular-velocity",
                gpu.angular_velocity.get(),
                scalar.angular_velocity.get(),
            ),
        ] {
            let difference = (gpu_value - scalar_value).abs();
            if difference > maximum.2 {
                maximum = (body_index, field, difference, gpu_value, scalar_value);
            }
        }
    }
    (maximum.0, maximum.1, maximum.3, maximum.4)
}
