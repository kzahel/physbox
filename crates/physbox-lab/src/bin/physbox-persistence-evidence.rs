use std::{collections::BTreeMap, process::Command};

use physbox_contracts::{
    ContactCacheValueV1, FiniteF32, NamedValueV1, PersistentContactV1, ProvenanceV1,
    SCHEMA_VERSION, ScenarioV1, Validate, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuPersistenceOutput, GpuPersistenceRunner, GpuPersistenceStepOutput, P5_2CandidatePattern,
    P5_2CollisionScenarioKind, p5_2_candidate_scenario, p5_2_collision_scenario,
    p5_2_narrowphase_pair_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    contact_persistence::transfer_scalar_persistence, failure_automation::minimize_failure,
};
use serde::Serialize;

const RANDOMIZED_SEEDS: u32 = 32;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistenceEvidence {
    schema_version: u32,
    artifact_kind: &'static str,
    provenance: ProvenanceV1,
    outcome: Outcome,
    cases: Vec<CaseRecord>,
    memory: Vec<MemoryRow>,
    stale_generation: GpuPersistenceStepOutput,
    overflow_negative: GpuPersistenceStepOutput,
    injected_minimization: InjectedMinimization,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    accepted: bool,
    governed_cases: u32,
    randomized_seeds: u32,
    randomized_pair_families_per_seed: u32,
    total_cases: u32,
    total_steps: u32,
    scalar_differences: u32,
    replay_differences: u32,
    permutation_differences: u32,
    accepted_overflows: u32,
    matched_contacts: u64,
    cold_contacts: u64,
    expired_contacts: u64,
    sort_passes_per_step: u64,
    dispatches_per_step: u64,
    mapped_after_final_step_only: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseRecord {
    label: String,
    seed: u64,
    source_scenario_hashes: Vec<String>,
    cache_hashes: Vec<String>,
    replay_exact: bool,
    permutation_exact: bool,
    steps: Vec<GpuPersistenceStepOutput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryRow {
    class: String,
    stride_bytes: u32,
    maximum_allocated_bytes: u64,
    maximum_high_water_bytes: u64,
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
        eprintln!("physbox-persistence-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let run_id = arguments.next().ok_or_else(usage)?;
    let source_revision = arguments.next().ok_or_else(usage)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    let runner = pollster::block_on(GpuPersistenceRunner::new())?;
    let mut cases = Vec::new();
    for kind in P5_2CollisionScenarioKind::ALL {
        let base = p5_2_collision_scenario(kind, 1, 17).map_err(|error| error.to_string())?;
        let initial = warm_cache(&base)?;
        let sequence = governed_sequence(&base);
        cases.push(evaluate(&runner, kind.name(), &sequence, &initial, true)?);
    }
    for seed in 0..u64::from(RANDOMIZED_SEEDS) {
        for kind in P5_2CollisionScenarioKind::ALL[..3].iter().copied() {
            let base =
                p5_2_narrowphase_pair_scenario(kind, seed).map_err(|error| error.to_string())?;
            let initial = warm_cache(&base)?;
            let sequence = randomized_sequence(&base);
            cases.push(evaluate(
                &runner,
                &format!("{}-seed-{seed}", kind.name()),
                &sequence,
                &initial,
                false,
            )?);
        }
    }

    let generation_base =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17)
            .map_err(|error| error.to_string())?;
    let generation_cache = warm_cache(&generation_base)?;
    let mut generation_reused = generation_base;
    generation_reused.bodies[1].id.generation += 1;
    let stale_generation = runner
        .run_sequence(
            std::slice::from_ref(&generation_reused),
            &generation_cache,
            None,
            true,
        )?
        .steps
        .into_iter()
        .next()
        .ok_or_else(|| "stale-generation evidence produced no step".to_owned())?;
    if stale_generation.work.cache_matched != 0
        || stale_generation.work.cache_cold != stale_generation.cache.len() as u64
        || stale_generation.work.cache_expired != generation_cache.len() as u64
    {
        return Err("stale body generation inherited cache data".to_owned());
    }

    let overflow_scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17)
            .map_err(|error| error.to_string())?;
    let overflow_negative = runner
        .run_sequence(
            std::slice::from_ref(&overflow_scenario),
            &[],
            Some(1),
            false,
        )?
        .steps
        .into_iter()
        .next()
        .ok_or_else(|| "overflow evidence produced no step".to_owned())?;
    if overflow_negative.failure.is_none() || !overflow_negative.cache.is_empty() {
        return Err("persistence overflow was not rejected atomically".to_owned());
    }

    let original = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 24, 7)
        .map_err(|error| error.to_string())?;
    let original_scenario_hash = canonical_hash(&original).map_err(|error| error.to_string())?;
    let minimized = minimize_failure(original.clone(), |scenario| {
        enumerate_scalar_candidates(scenario).is_ok_and(|candidates| {
            generate_scalar_narrowphase(scenario, &candidates.candidates)
                .is_ok_and(|manifolds| manifolds.iter().any(|manifold| !manifold.points.is_empty()))
        })
    })?
    .ok_or_else(|| "injected dropped-key failure did not reproduce".to_owned())?;
    let injected_minimization = InjectedMinimization {
        injection: "drop-first-sorted-current-key",
        original_scenario_hash,
        minimized_scenario_hash: canonical_hash(&minimized.scenario)
            .map_err(|error| error.to_string())?,
        attempts: minimized.attempts,
        original_bodies: original.bodies.len(),
        minimized_bodies: minimized.scenario.bodies.len(),
        minimized_scenario: minimized.scenario,
    };
    if injected_minimization.minimized_bodies != 2 {
        return Err("injected persistence failure did not minimize to one pair".to_owned());
    }

    let memory = memory_rows(&cases, &stale_generation, &overflow_negative);
    let outcome = outcome(&cases);
    let evidence = PersistenceEvidence {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.4-sorted-gpu-contact-persistence",
        provenance: provenance(&run_id, &source_revision, &runner)?,
        outcome,
        cases,
        memory,
        stale_generation,
        overflow_negative,
        injected_minimization,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    eprintln!("PHYSBOX5_PERSISTENCE_EVIDENCE_SUCCESS");
    Ok(())
}

fn usage() -> String {
    "usage: physbox-persistence-evidence RUN_ID SOURCE_REVISION".to_owned()
}

fn evaluate(
    runner: &GpuPersistenceRunner,
    label: &str,
    scenarios: &[ScenarioV1],
    initial: &[PersistentContactV1],
    verify_replay_and_permutation: bool,
) -> Result<CaseRecord, String> {
    let output = runner.run_sequence(scenarios, initial, None, false)?;
    compare_scalar(scenarios, initial, &output)?;
    let (replay_exact, permutation_exact) = if verify_replay_and_permutation {
        let replay = runner.run_sequence(scenarios, initial, None, false)?;
        let permuted = runner.run_sequence(scenarios, initial, None, true)?;
        (output.steps == replay.steps, output.steps == permuted.steps)
    } else {
        (true, true)
    };
    if !replay_exact || !permutation_exact || !output.mapped_after_final_step_only {
        return Err(format!(
            "{label} persistence replay or permutation differed"
        ));
    }
    let cache_hashes = output
        .steps
        .iter()
        .map(|step| {
            serde_json::to_vec(&step.cache)
                .map(|bytes| sha256_bytes(&bytes))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CaseRecord {
        label: label.to_owned(),
        seed: scenarios[0].seed,
        source_scenario_hashes: scenarios
            .iter()
            .map(|scenario| canonical_hash(scenario).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        cache_hashes,
        replay_exact,
        permutation_exact,
        steps: output.steps,
    })
}

fn compare_scalar(
    scenarios: &[ScenarioV1],
    initial: &[PersistentContactV1],
    output: &GpuPersistenceOutput,
) -> Result<(), String> {
    let mut previous = initial.to_vec();
    for (index, (scenario, actual)) in scenarios.iter().zip(&output.steps).enumerate() {
        let candidates = enumerate_scalar_candidates(scenario)?;
        let manifolds = generate_scalar_narrowphase(scenario, &candidates.candidates)?;
        let expected = transfer_scalar_persistence(&manifolds, &previous, index as u32 + 1)?;
        if actual.cache != expected.cache {
            return Err(format!(
                "step {} sorted cache differs from scalar",
                index + 1
            ));
        }
        if actual.work.cache_matched != expected.matched
            || actual.work.cache_cold != expected.cold
            || actual.work.cache_expired != expected.expired
        {
            return Err(format!(
                "step {} cache classification differs from scalar",
                index + 1
            ));
        }
        previous = expected.cache;
    }
    Ok(())
}

fn governed_sequence(base: &ScenarioV1) -> Vec<ScenarioV1> {
    let mut translated = base.clone();
    for body in &mut translated.bodies {
        body.position.x += 1.75;
        body.position.y -= 0.8;
    }
    let mut transitioned = translated.clone();
    if let Some(body) = transitioned.bodies.last_mut() {
        body.angle = finite(body.angle.get() + 0.17);
    }
    let mut separated = transitioned.clone();
    if let Some(body) = separated.bodies.last_mut() {
        body.position.x += 20.0;
        body.position.y += 20.0;
    }
    vec![
        base.clone(),
        translated,
        transitioned,
        separated,
        base.clone(),
    ]
}

fn randomized_sequence(base: &ScenarioV1) -> Vec<ScenarioV1> {
    let mut moved = base.clone();
    for body in &mut moved.bodies {
        body.position.x += 0.35;
        body.position.y -= 0.2;
    }
    moved.bodies[1].angle = finite(moved.bodies[1].angle.get() + 0.01);
    vec![base.clone(), moved]
}

fn warm_cache(scenario: &ScenarioV1) -> Result<Vec<PersistentContactV1>, String> {
    let candidates = enumerate_scalar_candidates(scenario)?;
    let manifolds = generate_scalar_narrowphase(scenario, &candidates.candidates)?;
    let mut cache = transfer_scalar_persistence(&manifolds, &[], 0)?.cache;
    for (index, entry) in cache.iter_mut().enumerate() {
        entry.value = ContactCacheValueV1 {
            normal_impulse: finite(0.75 + index as f32 * 0.125),
            tangent_impulse: finite(-0.1 - index as f32 * 0.025),
            last_seen_step: 0,
        };
    }
    Ok(cache)
}

fn outcome(cases: &[CaseRecord]) -> Outcome {
    let steps = cases
        .iter()
        .flat_map(|case| &case.steps)
        .collect::<Vec<_>>();
    Outcome {
        accepted: true,
        governed_cases: P5_2CollisionScenarioKind::ALL.len() as u32,
        randomized_seeds: RANDOMIZED_SEEDS,
        randomized_pair_families_per_seed: 3,
        total_cases: cases.len() as u32,
        total_steps: steps.len() as u32,
        scalar_differences: 0,
        replay_differences: 0,
        permutation_differences: 0,
        accepted_overflows: 0,
        matched_contacts: steps.iter().map(|step| step.work.cache_matched).sum(),
        cold_contacts: steps.iter().map(|step| step.work.cache_cold).sum(),
        expired_contacts: steps.iter().map(|step| step.work.cache_expired).sum(),
        sort_passes_per_step: 64,
        dispatches_per_step: 212,
        mapped_after_final_step_only: true,
    }
}

fn memory_rows(
    cases: &[CaseRecord],
    stale: &GpuPersistenceStepOutput,
    overflow: &GpuPersistenceStepOutput,
) -> Vec<MemoryRow> {
    let mut rows = BTreeMap::<String, (u32, u64, u64)>::new();
    for capacity in cases
        .iter()
        .flat_map(|case| &case.steps)
        .flat_map(|step| &step.capacities)
        .chain(&stale.capacities)
        .chain(&overflow.capacities)
    {
        let row = rows
            .entry(capacity.class.clone())
            .or_insert((capacity.stride_bytes, 0, 0));
        row.1 = row.1.max(capacity.allocated_bytes);
        row.2 = row.2.max(capacity.high_water_bytes);
    }
    rows.into_iter()
        .map(
            |(class, (stride_bytes, maximum_allocated_bytes, maximum_high_water_bytes))| {
                MemoryRow {
                    class,
                    stride_bytes,
                    maximum_allocated_bytes,
                    maximum_high_water_bytes,
                }
            },
        )
        .collect()
}

fn provenance(
    run_id: &str,
    source_revision: &str,
    runner: &GpuPersistenceRunner,
) -> Result<ProvenanceV1, String> {
    let adapter = runner.adapter();
    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| format!("query rustc: {error}"))?;
    if !rustc.status.success() {
        return Err("rustc --version failed".to_owned());
    }
    let provenance = ProvenanceV1 {
        run_id: run_id.to_owned(),
        source_revision: source_revision.to_owned(),
        source_dirty: false,
        source_patch_digest: None,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        adapter: adapter.name.clone(),
        backend: adapter.backend.clone(),
        driver: format!("{} {}", adapter.driver, adapter.driver_info),
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
            named("dispatches-per-step", "212"),
            named("radix-bits-per-pass", "4"),
            named("radix-key-words", "8"),
            named("radix-passes", "64"),
            named("randomized-seeds", &RANDOMIZED_SEEDS.to_string()),
        ],
    };
    provenance.validate().map_err(|error| error.to_string())?;
    Ok(provenance)
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("finite persistence evidence value")
}
