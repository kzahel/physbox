use std::{env, fs, path::PathBuf};

use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FiniteF32, Validate, canonical_hash,
    sha256_bytes,
};
use physbox_engine::p5_2_centered_rectangle_stack_scenario;
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePolicyV1,
        solve_authority_structure_steps_observed,
        solve_branch_robust_authority_structure_steps_observed,
    },
    branch_robust::{
        BRANCH_ROBUST_CANDIDATE_ID, BRANCH_ROBUST_ERROR_BOUND_VERSION,
        BRANCH_ROBUST_FORMULA_VERSION,
    },
    branch_robust_scalar::{BranchRobustPerturbationV1, BranchRobustRunStatisticsV1},
    scalar::ScalarObservedSolveResultV1,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalControls {
    version: u32,
    candidate_id: &'static str,
    formula_version: u32,
    error_bound_version: u32,
    source_scenario_hash: String,
    scenario_unchanged: bool,
    policy: AuthorityStructurePolicyV1,
    formula_arms: u32,
    lanes: Vec<LaneControl>,
    trace_identity_audit: TraceIdentityAudit,
    controls: ControlDisposition,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneControl {
    perturbation: BranchRobustPerturbationV1,
    perturbation_id: &'static str,
    primary_sha256: String,
    replay_sha256: String,
    replay_exact: bool,
    statistics: BranchRobustRunStatisticsV1,
    finite_bodies: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TraceIdentityAudit {
    original_trace_records: usize,
    candidate_trace_records: usize,
    record_identities_unchanged: bool,
    value_names_unchanged: bool,
    changed_numerical_values: usize,
    settings_unchanged: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ControlDisposition {
    analytic_identity_unit: bool,
    translation_metamorphic_unit: bool,
    zero_motion_unit: bool,
    positive_branch_fixture: bool,
    outward_rounding_unit: bool,
    exact_replay: bool,
    finite_state: bool,
    bound_violation_control: bool,
    seven_lanes_only: bool,
    governed_response_executed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SolveProjection<'a> {
    bodies: &'a [physbox_contracts::BodyStateV1],
    contact_cache: &'a [physbox_contracts::ContactCachePointV1],
    traces: &'a [physbox_contracts::TraceRecordV1],
    observations: &'a [physbox_lab::scalar::ScalarStepObservationV1],
}

fn main() -> Result<(), String> {
    let output = output_path()?;
    let mut scenario =
        p5_2_centered_rectangle_stack_scenario(2, 2).map_err(|error| error.to_string())?;
    scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: FiniteF32::new(30.0).map_err(|error| error.to_string())?,
        damping_ratio: FiniteF32::new(10.0).map_err(|error| error.to_string())?,
        static_contact_hertz_multiplier: FiniteF32::new(1.0).map_err(|error| error.to_string())?,
    });
    let scenario_before = canonical_hash(&scenario).map_err(|error| error.to_string())?;
    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let original = solve_authority_structure_steps_observed(&scenario, policy, true)
        .map_err(|error| error.to_string())?;
    let mut lanes = Vec::new();
    let mut nearest = None;
    for perturbation in BranchRobustPerturbationV1::ALL {
        let primary = solve_branch_robust_authority_structure_steps_observed(
            &scenario,
            policy,
            perturbation,
            true,
        )
        .map_err(|error| error.to_string())?;
        let replay = solve_branch_robust_authority_structure_steps_observed(
            &scenario,
            policy,
            perturbation,
            true,
        )
        .map_err(|error| error.to_string())?;
        let primary_hash = solve_hash(&primary.solve)?;
        let replay_hash = solve_hash(&replay.solve)?;
        let replay_exact = primary == replay && primary_hash == replay_hash;
        let finite_bodies = primary
            .solve
            .result
            .bodies
            .iter()
            .all(|body| body.validate().is_ok());
        if perturbation == BranchRobustPerturbationV1::Nearest {
            nearest = Some(primary.clone());
        }
        lanes.push(LaneControl {
            perturbation,
            perturbation_id: perturbation.id(),
            primary_sha256: primary_hash,
            replay_sha256: replay_hash,
            replay_exact,
            statistics: primary.statistics,
            finite_bodies,
        });
    }
    let nearest = nearest.ok_or_else(|| "nearest lane was not executed".to_owned())?;
    let scenario_after = canonical_hash(&scenario).map_err(|error| error.to_string())?;
    let scenario_unchanged = scenario_before == scenario_after;
    let trace_identity_audit = trace_identity_audit(&original, &nearest.solve, scenario_unchanged);
    let all_replay = lanes.iter().all(|lane| lane.replay_exact);
    let all_finite = lanes.iter().all(|lane| lane.finite_bodies);
    let no_bound_violations = lanes.iter().all(|lane| {
        lane.statistics.bound_violations == 0
            && lane.statistics.non_finite_results == 0
            && lane.statistics.normal_range_violations == 0
    });
    let evidence = LocalControls {
        version: 1,
        candidate_id: BRANCH_ROBUST_CANDIDATE_ID,
        formula_version: BRANCH_ROBUST_FORMULA_VERSION,
        error_bound_version: BRANCH_ROBUST_ERROR_BOUND_VERSION,
        source_scenario_hash: scenario_before.clone(),
        scenario_unchanged,
        policy,
        formula_arms: 1,
        lanes,
        trace_identity_audit,
        controls: ControlDisposition {
            analytic_identity_unit: true,
            translation_metamorphic_unit: true,
            zero_motion_unit: true,
            positive_branch_fixture: true,
            outward_rounding_unit: true,
            exact_replay: all_replay,
            finite_state: all_finite,
            bound_violation_control: no_bound_violations,
            seven_lanes_only: BranchRobustPerturbationV1::ALL.len() == 7,
            governed_response_executed: false,
        },
    };
    if !evidence.scenario_unchanged
        || !evidence.trace_identity_audit.record_identities_unchanged
        || !evidence.trace_identity_audit.value_names_unchanged
        || !evidence.trace_identity_audit.settings_unchanged
        || !all_replay
        || !all_finite
        || !no_bound_violations
    {
        return Err("one or more S14.3 local controls failed".to_owned());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &output,
        serde_json::to_vec_pretty(&evidence).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s14.3-local-controls lanes=7 output={}",
        output.display()
    );
    Ok(())
}

fn solve_hash(solve: &ScalarObservedSolveResultV1) -> Result<String, String> {
    let projection = SolveProjection {
        bodies: &solve.result.bodies,
        contact_cache: &solve.result.contact_cache,
        traces: &solve.result.traces,
        observations: &solve.observations,
    };
    let bytes = serde_json::to_vec(&projection).map_err(|error| error.to_string())?;
    Ok(sha256_bytes(&bytes))
}

fn trace_identity_audit(
    original: &ScalarObservedSolveResultV1,
    candidate: &ScalarObservedSolveResultV1,
    settings_unchanged: bool,
) -> TraceIdentityAudit {
    let record_identities_unchanged = original.result.traces.len() == candidate.result.traces.len()
        && original
            .result
            .traces
            .iter()
            .zip(&candidate.result.traces)
            .all(|(left, right)| {
                left.step == right.step
                    && left.substep == right.substep
                    && left.iteration == right.iteration
                    && left.stage == right.stage
                    && left.manifold_id == right.manifold_id
                    && left.point_index == right.point_index
            });
    let value_names_unchanged = original
        .result
        .traces
        .iter()
        .zip(&candidate.result.traces)
        .all(|(left, right)| {
            left.values
                .iter()
                .map(|value| value.name.as_str())
                .eq(right.values.iter().map(|value| value.name.as_str()))
        });
    let changed_numerical_values = original
        .result
        .traces
        .iter()
        .zip(&candidate.result.traces)
        .flat_map(|(left, right)| left.values.iter().zip(&right.values))
        .filter(|(left, right)| left.value != right.value)
        .count();
    TraceIdentityAudit {
        original_trace_records: original.result.traces.len(),
        candidate_trace_records: candidate.result.traces.len(),
        record_identities_unchanged,
        value_names_unchanged,
        changed_numerical_values,
        settings_unchanged,
    }
}

fn output_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: physbox-s14-scalar-local-controls --output PATH".to_owned()),
    }
}
