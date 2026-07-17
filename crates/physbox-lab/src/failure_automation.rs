use physbox_contracts::{
    FailureKindV1, FailureV1, FiniteF32, ScenarioV1, Validate, canonical_hash,
};
use physbox_engine::{
    GpuSolverCandidate, authored_chain_scenario, authored_one_contact_scenario, run_gpu_solver_case,
};
use serde::Serialize;

use crate::{
    capacity_is_valid, equal_and_opposite, projected_impulses,
    scalar::{first_trace_divergence, solve_authored_contact_pass},
};

const PASS_ABSOLUTE_TOLERANCE: f32 = 2.0e-5;
const PASS_RELATIVE_TOLERANCE: f32 = 2.0e-5;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusReport {
    pub first_seed: u64,
    pub seed_count: u32,
    pub scenarios_tested: u32,
    pub candidate_runs: u32,
    pub failures: Vec<MinimizedFailure>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimizedFailure {
    pub candidate: String,
    pub original_scenario_hash: String,
    pub minimized_scenario_hash: String,
    pub attempts: u32,
    pub failure: FailureV1,
    pub original_scenario: ScenarioV1,
    pub minimized_scenario: ScenarioV1,
}

pub fn run_randomized_gpu_corpus(first_seed: u64, seed_count: u32) -> Result<CorpusReport, String> {
    if seed_count == 0 {
        return Err("randomized corpus requires at least one seed".to_owned());
    }
    let mut failures = Vec::new();
    let mut scenarios_tested = 0_u32;
    let mut candidate_runs = 0_u32;
    for offset in 0..seed_count {
        let seed = first_seed.wrapping_add(u64::from(offset));
        let chain_count = 2 + (seed % 31) as u32;
        for scenario in [
            authored_one_contact_scenario(seed),
            authored_chain_scenario(chain_count, seed).map_err(|error| error.to_string())?,
        ] {
            scenarios_tested += 1;
            for candidate in [
                GpuSolverCandidate::JacobiGather,
                GpuSolverCandidate::ColoredSoftTgs,
            ] {
                candidate_runs += 1;
                if let Err(evaluation) = evaluate_candidate(&scenario, candidate) {
                    let signature = evaluation.signature.clone();
                    let minimized = minimize_failure(scenario.clone(), |candidate_scenario| {
                        evaluate_candidate(candidate_scenario, candidate)
                            .is_err_and(|failure| failure.signature == signature)
                    })?
                    .ok_or_else(|| "failing corpus case did not reproduce".to_owned())?;
                    let minimized_hash =
                        canonical_hash(&minimized.scenario).map_err(|error| error.to_string())?;
                    failures.push(MinimizedFailure {
                        candidate: candidate.name().to_owned(),
                        original_scenario_hash: canonical_hash(&scenario)
                            .map_err(|error| error.to_string())?,
                        minimized_scenario_hash: minimized_hash.clone(),
                        attempts: minimized.attempts,
                        failure: FailureV1 {
                            kind: FailureKindV1::NumericalDivergence,
                            reason: evaluation.reason,
                            first_step: Some(0),
                            first_stage: Some("authored-contact-pass".to_owned()),
                            reproducer_hash: Some(minimized_hash),
                        },
                        original_scenario: scenario.clone(),
                        minimized_scenario: minimized.scenario,
                    });
                }
            }
        }
    }
    Ok(CorpusReport {
        first_seed,
        seed_count,
        scenarios_tested,
        candidate_runs,
        failures,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct MinimizedScenario {
    pub scenario: ScenarioV1,
    pub attempts: u32,
}

/// Minimizes a neutral dense-body generated-contact reproducer while
/// preserving the slot/index invariant required by the product graph.
pub fn minimize_dense_generated_failure<F>(
    scenario: ScenarioV1,
    mut fails: F,
) -> Result<Option<MinimizedScenario>, String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    scenario.validate().map_err(|error| error.to_string())?;
    if !scenario.manifolds.is_empty()
        || !scenario.contact_cache.is_empty()
        || !scenario.actions.is_empty()
        || scenario
            .bodies
            .iter()
            .enumerate()
            .any(|(index, body)| body.id.slot != index as u32)
    {
        return Err("dense generated minimization requires a neutral dense-body source".to_owned());
    }
    if !fails(&scenario) {
        return Ok(None);
    }
    let mut minimized = scenario;
    let mut attempts = 1_u32;

    let mut index = 0;
    while index < minimized.bodies.len() {
        if minimized.bodies[index].inverse_mass.get() == 0.0
            || minimized
                .bodies
                .iter()
                .filter(|body| body.inverse_mass.get() > 0.0)
                .count()
                <= 1
        {
            index += 1;
            continue;
        }
        let mut candidate = minimized.clone();
        candidate.bodies.remove(index);
        renumber_dense_bodies(&mut candidate);
        attempts += 1;
        if valid_failure(&candidate, &mut fails)? {
            minimized = candidate;
        } else {
            index += 1;
        }
    }

    for body_index in 0..minimized.bodies.len() {
        if minimized.bodies[body_index].inverse_mass.get() == 0.0 {
            continue;
        }
        for component in 0..5 {
            let mut candidate = minimized.clone();
            let body = &mut candidate.bodies[body_index];
            match component {
                0 => body.position.x = 0.0,
                1 => body.angle = FiniteF32::new(0.0).unwrap(),
                2 => body.linear_velocity.x = 0.0,
                3 => body.linear_velocity.y = 0.0,
                4 => body.angular_velocity = FiniteF32::new(0.0).unwrap(),
                _ => unreachable!(),
            }
            attempts += 1;
            if valid_failure(&candidate, &mut fails)? {
                minimized = candidate;
            }
        }
    }

    for requested_steps in [1, 30, 60, 120, 240, 480, 600, 900] {
        if requested_steps >= minimized.requested_steps {
            break;
        }
        let mut candidate = minimized.clone();
        candidate.requested_steps = requested_steps;
        attempts += 1;
        if valid_failure(&candidate, &mut fails)? {
            minimized = candidate;
            break;
        }
    }
    minimized.validate().map_err(|error| error.to_string())?;
    Ok(Some(MinimizedScenario {
        scenario: minimized,
        attempts,
    }))
}

fn renumber_dense_bodies(scenario: &mut ScenarioV1) {
    for (slot, body) in scenario.bodies.iter_mut().enumerate() {
        body.id.slot = slot as u32;
    }
}

pub fn minimize_failure<F>(
    scenario: ScenarioV1,
    mut fails: F,
) -> Result<Option<MinimizedScenario>, String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    scenario.validate().map_err(|error| error.to_string())?;
    if !fails(&scenario) {
        return Ok(None);
    }
    let mut minimized = scenario;
    let mut attempts = 1_u32;

    minimize_manifolds(&mut minimized, &mut attempts, &mut fails)?;
    minimize_actions(&mut minimized, &mut attempts, &mut fails)?;
    minimize_bodies(&mut minimized, &mut attempts, &mut fails)?;
    for requested_steps in 1..minimized.requested_steps {
        let mut candidate = minimized.clone();
        candidate.requested_steps = requested_steps;
        attempts += 1;
        if valid_failure(&candidate, &mut fails)? {
            minimized = candidate;
            break;
        }
    }
    minimized.validate().map_err(|error| error.to_string())?;
    Ok(Some(MinimizedScenario {
        scenario: minimized,
        attempts,
    }))
}

#[derive(Clone, Debug)]
struct EvaluationFailure {
    signature: String,
    reason: String,
}

fn evaluate_candidate(
    scenario: &ScenarioV1,
    candidate: GpuSolverCandidate,
) -> Result<(), EvaluationFailure> {
    let scalar = solve_authored_contact_pass(scenario)
        .map_err(|error| evaluation_failure("scalar", error.to_string()))?;
    let gpu = pollster::block_on(run_gpu_solver_case(scenario, candidate))
        .map_err(|error| evaluation_failure("gpu-execution", error))?;
    let replay = pollster::block_on(run_gpu_solver_case(scenario, candidate))
        .map_err(|error| evaluation_failure("gpu-replay", error))?;
    if gpu.bodies != replay.bodies
        || gpu.contact_cache != replay.contact_cache
        || gpu.traces != replay.traces
    {
        return Err(evaluation_failure(
            "repeatability",
            "GPU candidate did not replay byte-for-byte",
        ));
    }
    let point_count = scenario
        .manifolds
        .iter()
        .map(|manifold| manifold.points.len())
        .sum::<usize>();
    if point_count == 1
        && let Some(divergence) = first_trace_divergence(
            &scalar.traces,
            &gpu.traces,
            PASS_ABSOLUTE_TOLERANCE,
            PASS_RELATIVE_TOLERANCE,
        )
    {
        return Err(evaluation_failure(
            format!("trace:{}", divergence.value_name),
            format!(
                "trace {} {}: {}",
                divergence.record_index, divergence.value_name, divergence.reason
            ),
        ));
    }
    capacity_is_valid(&gpu.capacities)
        .map_err(|error| evaluation_failure("capacity", error.to_string()))?;
    if gpu.bodies.iter().any(|body| {
        !body.position.x.is_finite()
            || !body.position.y.is_finite()
            || !body.linear_velocity.x.is_finite()
            || !body.linear_velocity.y.is_finite()
            || !body.angle.get().is_finite()
            || !body.angular_velocity.get().is_finite()
    }) {
        return Err(evaluation_failure(
            "finite-state",
            "GPU candidate emitted a non-finite body state",
        ));
    }
    for cached in &gpu.contact_cache {
        let friction = scenario
            .manifolds
            .iter()
            .find(|manifold| {
                manifold.body_a == cached.body_a
                    && manifold.body_b == cached.body_b
                    && manifold.points.iter().any(|point| {
                        point.feature_a == cached.feature_a && point.feature_b == cached.feature_b
                    })
            })
            .ok_or_else(|| {
                evaluation_failure(
                    "contact-identity",
                    "GPU emitted an unknown contact cache key",
                )
            })?
            .material
            .friction;
        projected_impulses(cached.normal_impulse, cached.tangent_impulse, friction)
            .map_err(|error| evaluation_failure("projected-impulse", error.to_string()))?;
    }
    if candidate == GpuSolverCandidate::JacobiGather {
        if gpu.contributions.len() != point_count * 2 {
            return Err(evaluation_failure(
                "contribution-count",
                "Jacobi contribution count does not equal two records per contact point",
            ));
        }
        for pair in gpu.contributions.chunks_exact(2) {
            equal_and_opposite(pair[0].impulse, pair[1].impulse, PASS_ABSOLUTE_TOLERANCE)
                .map_err(|error| evaluation_failure("equal-and-opposite", error.to_string()))?;
        }
    }
    Ok(())
}

fn evaluation_failure(
    signature: impl Into<String>,
    reason: impl Into<String>,
) -> EvaluationFailure {
    EvaluationFailure {
        signature: signature.into(),
        reason: reason.into(),
    }
}

fn minimize_manifolds<F>(
    scenario: &mut ScenarioV1,
    attempts: &mut u32,
    fails: &mut F,
) -> Result<(), String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    let mut granularity = 2_usize;
    while !scenario.manifolds.is_empty() {
        let chunk_size = scenario.manifolds.len().div_ceil(granularity);
        let mut reduced = false;
        for start in (0..scenario.manifolds.len()).step_by(chunk_size) {
            let end = (start + chunk_size).min(scenario.manifolds.len());
            let mut candidate = scenario.clone();
            candidate.manifolds.drain(start..end);
            clean_references(&mut candidate);
            *attempts += 1;
            if valid_failure(&candidate, fails)? {
                *scenario = candidate;
                granularity = 2;
                reduced = true;
                break;
            }
        }
        if reduced {
            continue;
        }
        if granularity >= scenario.manifolds.len() {
            break;
        }
        granularity = (granularity * 2).min(scenario.manifolds.len());
    }
    Ok(())
}

fn minimize_actions<F>(
    scenario: &mut ScenarioV1,
    attempts: &mut u32,
    fails: &mut F,
) -> Result<(), String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    let mut index = 0;
    while index < scenario.actions.len() {
        let mut candidate = scenario.clone();
        candidate.actions.remove(index);
        *attempts += 1;
        if valid_failure(&candidate, fails)? {
            *scenario = candidate;
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn minimize_bodies<F>(
    scenario: &mut ScenarioV1,
    attempts: &mut u32,
    fails: &mut F,
) -> Result<(), String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    let mut index = 0;
    while index < scenario.bodies.len() {
        let mut candidate = scenario.clone();
        let removed = candidate.bodies.remove(index).id;
        candidate
            .manifolds
            .retain(|manifold| manifold.body_a != removed && manifold.body_b != removed);
        candidate
            .contact_cache
            .retain(|point| point.body_a != removed && point.body_b != removed);
        candidate.actions.retain(|action| match action {
            physbox_contracts::NeutralActionV1::ApplyImpulse { body, .. }
            | physbox_contracts::NeutralActionV1::GrabBegin { body, .. } => *body != removed,
            physbox_contracts::NeutralActionV1::SetGravity { .. }
            | physbox_contracts::NeutralActionV1::GrabTarget { .. }
            | physbox_contracts::NeutralActionV1::GrabEnd { .. } => true,
        });
        clean_references(&mut candidate);
        *attempts += 1;
        if valid_failure(&candidate, fails)? {
            *scenario = candidate;
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn clean_references(scenario: &mut ScenarioV1) {
    scenario.contact_cache.retain(|cached| {
        scenario.manifolds.iter().any(|manifold| {
            manifold.body_a == cached.body_a
                && manifold.body_b == cached.body_b
                && manifold.points.iter().any(|point| {
                    point.feature_a == cached.feature_a && point.feature_b == cached.feature_b
                })
        })
    });
}

fn valid_failure<F>(candidate: &ScenarioV1, fails: &mut F) -> Result<bool, String>
where
    F: FnMut(&ScenarioV1) -> bool,
{
    candidate.validate().map_err(|error| error.to_string())?;
    Ok(fails(candidate))
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{NeutralActionV1, Vec2, canonical_json};
    use physbox_engine::{P5_2SettlingFixtureKind, p5_2_settling_fixture_scenario};

    use super::*;

    #[test]
    fn injected_failure_is_reduced_to_essential_contacts_and_bodies() {
        let scenario = authored_chain_scenario(16, 9).unwrap();
        let minimized = minimize_failure(scenario, |candidate| candidate.manifolds.len() >= 3)
            .unwrap()
            .unwrap();
        assert_eq!(minimized.scenario.manifolds.len(), 3);
        assert_eq!(minimized.scenario.bodies.len(), 4);
        assert!(minimized.attempts > 1);
    }

    #[test]
    fn action_and_fixture_window_minimization_is_deterministic() {
        let mut scenario = authored_one_contact_scenario(44);
        scenario.requested_steps = 8;
        for step in 0..4 {
            scenario.actions.push(NeutralActionV1::SetGravity {
                step,
                gravity: Vec2::new(0.0, -10.0 - step as f32),
            });
        }
        let predicate =
            |candidate: &ScenarioV1| candidate.actions.len() >= 2 && candidate.requested_steps >= 3;
        let first = minimize_failure(scenario.clone(), predicate)
            .unwrap()
            .unwrap();
        let replay = minimize_failure(scenario, predicate).unwrap().unwrap();
        assert_eq!(first.scenario.actions.len(), 2);
        assert_eq!(first.scenario.requested_steps, 3);
        assert_eq!(
            canonical_json(&first.scenario).unwrap(),
            canonical_json(&replay.scenario).unwrap()
        );
    }

    #[test]
    fn non_failure_is_not_published_as_a_reproducer() {
        let scenario = authored_one_contact_scenario(1);
        assert!(minimize_failure(scenario, |_| false).unwrap().is_none());
    }

    #[test]
    fn dense_generated_minimization_is_deterministic_and_keeps_slots_dense() {
        let scenario = p5_2_settling_fixture_scenario(
            P5_2SettlingFixtureKind::SequentialRectangles5,
            0x6002,
            1_200,
        )
        .unwrap();
        let predicate = |candidate: &ScenarioV1| {
            candidate
                .bodies
                .iter()
                .filter(|body| body.inverse_mass.get() > 0.0)
                .count()
                >= 2
                && candidate.requested_steps >= 60
        };
        let first = minimize_dense_generated_failure(scenario.clone(), predicate)
            .unwrap()
            .unwrap();
        let replay = minimize_dense_generated_failure(scenario, predicate)
            .unwrap()
            .unwrap();
        assert_eq!(
            canonical_json(&first.scenario).unwrap(),
            canonical_json(&replay.scenario).unwrap()
        );
        assert_eq!(first.scenario.requested_steps, 60);
        assert!(
            first
                .scenario
                .bodies
                .iter()
                .enumerate()
                .all(|(index, body)| { body.id.slot == index as u32 })
        );
        assert_eq!(
            first
                .scenario
                .bodies
                .iter()
                .filter(|body| body.inverse_mass.get() > 0.0)
                .count(),
            2
        );
    }
}
