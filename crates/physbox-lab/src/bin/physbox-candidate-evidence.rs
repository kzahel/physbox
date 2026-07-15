use std::process::Command;

use physbox_contracts::{
    CollisionCapacityV1, CollisionWorkV1, NamedValueV1, ProvenanceV1, SCHEMA_VERSION, ScenarioV1,
    Validate, canonical_hash,
};
use physbox_engine::{
    GpuCandidateOutput, GpuCandidateRunner, P5_2CandidatePattern, P5_2CollisionScenarioKind,
    p5_2_candidate_scenario, p5_2_collision_scenario,
};
use physbox_lab::{
    collision_candidates::{compare_gpu_candidates, enumerate_scalar_candidates},
    failure_automation::minimize_failure,
};
use serde::Serialize;

const RANDOMIZED_SEED_COUNT: u32 = 64;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEvidence {
    schema_version: u32,
    artifact_kind: &'static str,
    provenance: ProvenanceV1,
    outcome: Outcome,
    cases: Vec<CaseRecord>,
    overflow_negative: GpuCandidateOutput,
    injected_minimization: InjectedMinimization,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    accepted: bool,
    governed_scenarios: u32,
    randomized_seeds: u32,
    adversarial_patterns_per_seed: u32,
    total_cases: u32,
    exact_candidate_set_failures: u32,
    accepted_overflows: u32,
    maximum_aabb_absolute_error: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseRecord {
    label: String,
    seed: u64,
    bodies: u32,
    source_scenario_hash: String,
    exact_candidate_set_match: bool,
    maximum_aabb_absolute_error: f32,
    work: CollisionWorkV1,
    capacities: Vec<CollisionCapacityV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InjectedMinimization {
    injection: &'static str,
    original_scenario_hash: String,
    minimized_scenario_hash: String,
    attempts: u32,
    original_bodies: usize,
    minimized_bodies: usize,
    minimized_scenario: ScenarioV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-candidate-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let run_id = arguments
        .next()
        .ok_or_else(|| "usage: physbox-candidate-evidence RUN_ID SOURCE_REVISION".to_owned())?;
    let source_revision = arguments
        .next()
        .ok_or_else(|| "usage: physbox-candidate-evidence RUN_ID SOURCE_REVISION".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: physbox-candidate-evidence RUN_ID SOURCE_REVISION".to_owned());
    }

    let runner = pollster::block_on(GpuCandidateRunner::new())?;
    let mut cases = Vec::new();
    for kind in P5_2CollisionScenarioKind::ALL {
        let scenario = p5_2_collision_scenario(kind, 1, 17).map_err(|error| error.to_string())?;
        cases.push(evaluate(&runner, kind.name(), &scenario)?);
    }
    for seed in 0..u64::from(RANDOMIZED_SEED_COUNT) {
        for pattern in P5_2CandidatePattern::ALL {
            let body_count = 2 + ((seed * 17 + pattern as u64 * 7) % 63) as u32;
            let scenario = p5_2_candidate_scenario(pattern, body_count, seed)
                .map_err(|error| error.to_string())?;
            cases.push(evaluate(
                &runner,
                &format!("{}-seed-{seed}", pattern.name()),
                &scenario,
            )?);
        }
    }

    let overflow_scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 16, 29)
        .map_err(|error| error.to_string())?;
    let overflow_negative = runner.run(&overflow_scenario, Some(1))?;
    if overflow_negative.failure.is_none()
        || !overflow_negative.candidates.is_empty()
        || !overflow_negative
            .capacities
            .iter()
            .any(|capacity| capacity.class == "candidate-pairs" && capacity.overflowed)
    {
        return Err("candidate overflow was not rejected atomically".to_owned());
    }

    let original = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 24, 7)
        .map_err(|error| error.to_string())?;
    let original_scenario_hash = canonical_hash(&original).map_err(|error| error.to_string())?;
    let minimized = minimize_failure(original.clone(), |candidate| {
        candidate.bodies.len() >= 2
            && enumerate_scalar_candidates(candidate)
                .is_ok_and(|output| !output.candidates.is_empty())
    })?
    .ok_or_else(|| "injected missing-pair failure did not reproduce".to_owned())?;
    if minimized.scenario.bodies.len() != 2 {
        return Err("injected missing-pair reproducer did not minimize to two bodies".to_owned());
    }
    let injected_minimization = InjectedMinimization {
        injection: "drop-first-expected-candidate",
        original_scenario_hash,
        minimized_scenario_hash: canonical_hash(&minimized.scenario)
            .map_err(|error| error.to_string())?,
        attempts: minimized.attempts,
        original_bodies: original.bodies.len(),
        minimized_bodies: minimized.scenario.bodies.len(),
        minimized_scenario: minimized.scenario,
    };

    let maximum_aabb_absolute_error = cases
        .iter()
        .map(|case| case.maximum_aabb_absolute_error)
        .fold(0.0_f32, f32::max);
    let outcome = Outcome {
        accepted: true,
        governed_scenarios: P5_2CollisionScenarioKind::ALL.len() as u32,
        randomized_seeds: RANDOMIZED_SEED_COUNT,
        adversarial_patterns_per_seed: P5_2CandidatePattern::ALL.len() as u32,
        total_cases: cases.len() as u32,
        exact_candidate_set_failures: 0,
        accepted_overflows: 0,
        maximum_aabb_absolute_error,
    };
    let provenance = provenance(&run_id, &source_revision, &runner)?;
    let evidence = CandidateEvidence {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.2-complete-all-pairs-candidates",
        provenance,
        outcome,
        cases,
        overflow_negative,
        injected_minimization,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    eprintln!("PHYSBOX5_CANDIDATE_EVIDENCE_SUCCESS");
    Ok(())
}

fn evaluate(
    runner: &GpuCandidateRunner,
    label: &str,
    scenario: &ScenarioV1,
) -> Result<CaseRecord, String> {
    let expected = enumerate_scalar_candidates(scenario)?;
    let actual = runner.run(scenario, None)?;
    compare_gpu_candidates(&expected, &actual)
        .map_err(|difference| format!("{}: {}", difference.signature, difference.reason))?;
    let maximum_aabb_absolute_error = expected
        .aabbs
        .iter()
        .zip(&actual.aabbs)
        .flat_map(|(left, right)| {
            [
                (left.lower.x - right.lower.x).abs(),
                (left.lower.y - right.lower.y).abs(),
                (left.upper.x - right.upper.x).abs(),
                (left.upper.y - right.upper.y).abs(),
            ]
        })
        .fold(0.0_f32, f32::max);
    Ok(CaseRecord {
        label: label.to_owned(),
        seed: scenario.seed,
        bodies: scenario.bodies.len() as u32,
        source_scenario_hash: expected.source_scenario_hash,
        exact_candidate_set_match: true,
        maximum_aabb_absolute_error,
        work: actual.work,
        capacities: actual.capacities,
    })
}

fn provenance(
    run_id: &str,
    source_revision: &str,
    runner: &GpuCandidateRunner,
) -> Result<ProvenanceV1, String> {
    let adapter = runner.adapter();
    let driver = format!("{} {}", adapter.driver, adapter.driver_info);
    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| format!("query rustc: {error}"))?;
    if !rustc.status.success() {
        return Err("rustc --version failed".to_owned());
    }
    let record = ProvenanceV1 {
        run_id: run_id.to_owned(),
        source_revision: source_revision.to_owned(),
        source_dirty: false,
        source_patch_digest: None,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        adapter: adapter.name.clone(),
        backend: adapter.backend.clone(),
        driver: if driver.trim().is_empty() {
            "system-default".to_owned()
        } else {
            driver
        },
        toolchain: String::from_utf8_lossy(&rustc.stdout).trim().to_owned(),
        build_options: vec![named(
            "profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        )],
        runtime_settings: vec![
            named("candidate-capacity", "exact-all-pairs"),
            named("candidate-stage-dispatches", "8"),
            named("randomized-seeds", &RANDOMIZED_SEED_COUNT.to_string()),
            named("scan-workgroup-size", "256"),
        ],
    };
    record.validate().map_err(|error| error.to_string())?;
    Ok(record)
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}
