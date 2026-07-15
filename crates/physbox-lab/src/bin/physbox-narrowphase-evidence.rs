use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use physbox_contracts::{
    CollisionCapacityV1, CollisionManifoldV1, CollisionWorkV1, NamedValueV1, OracleRequestV1,
    ProvenanceV1, SCHEMA_VERSION, ScenarioV1, Validate, canonical_hash,
};
use physbox_engine::{
    GpuNarrowphaseOutput, GpuNarrowphaseRunner, P5_2CandidatePattern, P5_2CollisionScenarioKind,
    p5_2_box2d_narrowphase_projection, p5_2_candidate_scenario, p5_2_collision_scenario,
    p5_2_narrowphase_pair_scenario,
};
use physbox_lab::{
    analytic_narrowphase::{
        AuthorityNarrowphaseComparison, compare_box2d_narrowphase_detailed, compare_narrowphase,
        generate_scalar_narrowphase,
    },
    collision_candidates::enumerate_scalar_candidates,
    failure_automation::minimize_failure,
    oracle_transport::{OracleProcess, OracleRunConfig, run_oracle, verify_clean_git_pin},
};
use serde::Serialize;

const GPU_RANDOMIZED_SEEDS: u32 = 64;
const AUTHORITY_RANDOMIZED_SEEDS: u32 = 32;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NarrowphaseEvidence {
    schema_version: u32,
    artifact_kind: &'static str,
    provenance: ProvenanceV1,
    outcome: Outcome,
    gpu_cases: Vec<GpuCase>,
    authority_cases: Vec<AuthorityCase>,
    governed_playback: Vec<PlaybackCase>,
    overflow_negative: GpuNarrowphaseOutput,
    injected_minimization: InjectedMinimization,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    accepted: bool,
    gpu_governed_cases: u32,
    gpu_randomized_seeds: u32,
    gpu_adversarial_patterns_per_seed: u32,
    gpu_total_cases: u32,
    authority_governed_cases: u32,
    authority_randomized_seeds: u32,
    authority_pair_families_per_seed: u32,
    authority_total_cases: u32,
    topology_or_feature_failures: u32,
    accepted_overflows: u32,
    maximum_gpu_absolute_error: f32,
    maximum_authority_normal_absolute_error: f32,
    maximum_authority_world_point_absolute_error: f32,
    maximum_authority_separation_absolute_error: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuCase {
    label: String,
    seed: u64,
    bodies: u32,
    source_scenario_hash: String,
    candidates: u32,
    manifolds: u32,
    contact_points: u32,
    maximum_absolute_error: f32,
    work: CollisionWorkV1,
    capacities: Vec<CollisionCapacityV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityCase {
    label: String,
    seed: u64,
    source_scenario_hash: String,
    projection_scenario_hash: String,
    oracle_request_hash: String,
    oracle_result: String,
    authority_provenance: ProvenanceV1,
    comparison: AuthorityNarrowphaseComparison,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackCase {
    label: String,
    source: ScenarioV1,
    scalar: Vec<CollisionManifoldV1>,
    gpu: Vec<CollisionManifoldV1>,
    box2d: Vec<physbox_contracts::AuthoredManifoldV1>,
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
        eprintln!("physbox-narrowphase-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let run_id = next_argument(&mut arguments, "RUN_ID")?;
    let source_revision = next_argument(&mut arguments, "SOURCE_REVISION")?;
    let repository = PathBuf::from(next_argument(&mut arguments, "REPOSITORY")?);
    let oracle_program = PathBuf::from(next_argument(&mut arguments, "ORACLE_PROGRAM")?);
    let artifact_root = PathBuf::from(next_argument(&mut arguments, "ORACLE_ARTIFACT_ROOT")?);
    let box2d_revision = next_argument(&mut arguments, "BOX2D_REVISION")?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    let oracle_source = verify_clean_git_pin(&repository.join("reference/box2d"), &box2d_revision)
        .map_err(|error| error.to_string())?;
    let oracle_config = OracleRunConfig {
        artifact_root: artifact_root.clone(),
        timeout: Duration::from_secs(10),
        source: oracle_source,
        process: OracleProcess {
            program: oracle_program,
            arguments: vec![OsString::from("--lane"), OsString::from("narrowphase")],
        },
    };

    let runner = pollster::block_on(GpuNarrowphaseRunner::new())?;
    let mut gpu_cases = Vec::new();
    let mut governed_playback = Vec::new();
    for kind in P5_2CollisionScenarioKind::ALL {
        let scenario = p5_2_collision_scenario(kind, 1, 17).map_err(|error| error.to_string())?;
        let (record, scalar, gpu) = evaluate_gpu(&runner, kind.name(), &scenario)?;
        gpu_cases.push(record);
        governed_playback.push(PlaybackCase {
            label: kind.name().to_owned(),
            source: scenario,
            scalar,
            gpu,
            box2d: Vec::new(),
        });
    }
    for seed in 0..u64::from(GPU_RANDOMIZED_SEEDS) {
        for pattern in P5_2CandidatePattern::ALL {
            let body_count = 2 + ((seed * 13 + pattern as u64 * 11) % 63) as u32;
            let scenario = p5_2_candidate_scenario(pattern, body_count, seed)
                .map_err(|error| error.to_string())?;
            let label = format!("{}-seed-{seed}", pattern.name());
            gpu_cases.push(evaluate_gpu(&runner, &label, &scenario)?.0);
        }
    }

    let mut authority_cases = Vec::new();
    for (index, kind) in P5_2CollisionScenarioKind::ALL.into_iter().enumerate() {
        let source = p5_2_collision_scenario(kind, 1, 17).map_err(|error| error.to_string())?;
        let (record, authority) = evaluate_authority(
            &oracle_config,
            &artifact_root,
            &format!("{run_id}-governed-{index}"),
            kind.name(),
            &source,
        )?;
        authority_cases.push(record);
        governed_playback[index].box2d = authority;
    }
    for seed in 0..u64::from(AUTHORITY_RANDOMIZED_SEEDS) {
        for kind in P5_2CollisionScenarioKind::ALL[..3].iter().copied() {
            let source =
                p5_2_narrowphase_pair_scenario(kind, seed).map_err(|error| error.to_string())?;
            let label = format!("{}-seed-{seed}", kind.name());
            authority_cases.push(
                evaluate_authority(
                    &oracle_config,
                    &artifact_root,
                    &format!("{run_id}-{}-{seed}", kind.name()),
                    &label,
                    &source,
                )?
                .0,
            );
        }
    }

    let overflow_scenario =
        p5_2_collision_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1, 17)
            .map_err(|error| error.to_string())?;
    let overflow_negative = runner.run(&overflow_scenario, None, Some(1))?;
    if overflow_negative.failure.is_none()
        || !overflow_negative.manifolds.is_empty()
        || !overflow_negative.capacities.iter().any(|capacity| {
            capacity.class == "contact-manifolds" && capacity.overflowed && !capacity.accepted
        })
    {
        return Err("narrowphase overflow was not rejected atomically".to_owned());
    }

    let original = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 24, 7)
        .map_err(|error| error.to_string())?;
    let original_scenario_hash = canonical_hash(&original).map_err(|error| error.to_string())?;
    let minimized = minimize_failure(original.clone(), |candidate| {
        enumerate_scalar_candidates(candidate).is_ok_and(|candidates| {
            generate_scalar_narrowphase(candidate, &candidates.candidates)
                .is_ok_and(|manifolds| !manifolds.is_empty())
        })
    })?
    .ok_or_else(|| "injected missing-manifold failure did not reproduce".to_owned())?;
    if minimized.scenario.bodies.len() != 2 {
        return Err("injected narrowphase reproducer did not minimize to two bodies".to_owned());
    }
    let injected_minimization = InjectedMinimization {
        injection: "drop-first-expected-manifold",
        original_scenario_hash,
        minimized_scenario_hash: canonical_hash(&minimized.scenario)
            .map_err(|error| error.to_string())?,
        attempts: minimized.attempts,
        original_bodies: original.bodies.len(),
        minimized_bodies: minimized.scenario.bodies.len(),
        minimized_scenario: minimized.scenario,
    };

    let outcome = summarize(&gpu_cases, &authority_cases);
    let evidence = NarrowphaseEvidence {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.3-box2d-checked-gpu-narrowphase",
        provenance: provenance(&run_id, &source_revision, &runner)?,
        outcome,
        gpu_cases,
        authority_cases,
        governed_playback,
        overflow_negative,
        injected_minimization,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    eprintln!("PHYSBOX5_NARROWPHASE_EVIDENCE_SUCCESS");
    Ok(())
}

fn next_argument(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("missing {name}; {}", usage()))
}

fn usage() -> String {
    "usage: physbox-narrowphase-evidence RUN_ID SOURCE_REVISION REPOSITORY ORACLE_PROGRAM ORACLE_ARTIFACT_ROOT BOX2D_REVISION".to_owned()
}

fn evaluate_gpu(
    runner: &GpuNarrowphaseRunner,
    label: &str,
    scenario: &ScenarioV1,
) -> Result<(GpuCase, Vec<CollisionManifoldV1>, Vec<CollisionManifoldV1>), String> {
    let candidates = enumerate_scalar_candidates(scenario)?;
    let scalar = generate_scalar_narrowphase(scenario, &candidates.candidates)?;
    let gpu = runner.run(scenario, None, None)?;
    compare_narrowphase(&scalar, &gpu.manifolds).map_err(|difference| {
        format!("{label}: {}: {}", difference.signature, difference.reason)
    })?;
    let maximum_absolute_error = maximum_manifold_error(&scalar, &gpu.manifolds);
    Ok((
        GpuCase {
            label: label.to_owned(),
            seed: scenario.seed,
            bodies: scenario.bodies.len() as u32,
            source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
            candidates: candidates.candidates.len() as u32,
            manifolds: gpu.manifolds.len() as u32,
            contact_points: gpu
                .manifolds
                .iter()
                .map(|manifold| manifold.points.len() as u32)
                .sum(),
            maximum_absolute_error,
            work: gpu.work,
            capacities: gpu.capacities,
        },
        scalar,
        gpu.manifolds,
    ))
}

fn evaluate_authority(
    config: &OracleRunConfig,
    artifact_root: &Path,
    oracle_run_id: &str,
    label: &str,
    source: &ScenarioV1,
) -> Result<(AuthorityCase, Vec<physbox_contracts::AuthoredManifoldV1>), String> {
    let candidates = enumerate_scalar_candidates(source)?;
    let scalar = generate_scalar_narrowphase(source, &candidates.candidates)?;
    let projection =
        p5_2_box2d_narrowphase_projection(source).map_err(|error| error.to_string())?;
    let projection_scenario_hash =
        canonical_hash(&projection).map_err(|error| error.to_string())?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: oracle_run_id.to_owned(),
        source_scenario_hash: projection_scenario_hash.clone(),
        scenario: projection,
    };
    let authority = run_oracle(config, &request).map_err(|error| error.to_string())?;
    let comparison = compare_box2d_narrowphase_detailed(
        &candidates.candidates,
        &scalar,
        &authority.result.manifolds,
    )
    .map_err(|difference| format!("{}: {}", difference.signature, difference.reason))?;
    let oracle_result = authority
        .artifacts
        .result
        .strip_prefix(artifact_root)
        .unwrap_or(&authority.artifacts.result)
        .to_string_lossy()
        .to_string();
    Ok((
        AuthorityCase {
            label: label.to_owned(),
            seed: source.seed,
            source_scenario_hash: canonical_hash(source).map_err(|error| error.to_string())?,
            projection_scenario_hash,
            oracle_request_hash: authority.projection_hash,
            oracle_result,
            authority_provenance: authority.result.provenance,
            comparison,
        },
        authority.result.manifolds,
    ))
}

fn maximum_manifold_error(expected: &[CollisionManifoldV1], actual: &[CollisionManifoldV1]) -> f32 {
    expected
        .iter()
        .zip(actual)
        .flat_map(|(expected, actual)| {
            let mut errors = vec![
                (expected.normal.x - actual.normal.x).abs(),
                (expected.normal.y - actual.normal.y).abs(),
            ];
            for (expected, actual) in expected.points.iter().zip(&actual.points) {
                errors.extend([
                    (expected.local_anchor_a.x - actual.local_anchor_a.x).abs(),
                    (expected.local_anchor_a.y - actual.local_anchor_a.y).abs(),
                    (expected.local_anchor_b.x - actual.local_anchor_b.x).abs(),
                    (expected.local_anchor_b.y - actual.local_anchor_b.y).abs(),
                    (expected.world_point.x - actual.world_point.x).abs(),
                    (expected.world_point.y - actual.world_point.y).abs(),
                    (expected.separation.get() - actual.separation.get()).abs(),
                ]);
            }
            errors
        })
        .fold(0.0_f32, f32::max)
}

fn summarize(gpu_cases: &[GpuCase], authority_cases: &[AuthorityCase]) -> Outcome {
    Outcome {
        accepted: true,
        gpu_governed_cases: P5_2CollisionScenarioKind::ALL.len() as u32,
        gpu_randomized_seeds: GPU_RANDOMIZED_SEEDS,
        gpu_adversarial_patterns_per_seed: P5_2CandidatePattern::ALL.len() as u32,
        gpu_total_cases: gpu_cases.len() as u32,
        authority_governed_cases: P5_2CollisionScenarioKind::ALL.len() as u32,
        authority_randomized_seeds: AUTHORITY_RANDOMIZED_SEEDS,
        authority_pair_families_per_seed: 3,
        authority_total_cases: authority_cases.len() as u32,
        topology_or_feature_failures: 0,
        accepted_overflows: 0,
        maximum_gpu_absolute_error: gpu_cases
            .iter()
            .map(|case| case.maximum_absolute_error)
            .fold(0.0_f32, f32::max),
        maximum_authority_normal_absolute_error: authority_cases
            .iter()
            .map(|case| case.comparison.maximum_normal_absolute_error)
            .fold(0.0_f32, f32::max),
        maximum_authority_world_point_absolute_error: authority_cases
            .iter()
            .map(|case| case.comparison.maximum_world_point_absolute_error)
            .fold(0.0_f32, f32::max),
        maximum_authority_separation_absolute_error: authority_cases
            .iter()
            .map(|case| case.comparison.maximum_separation_absolute_error)
            .fold(0.0_f32, f32::max),
    }
}

fn provenance(
    run_id: &str,
    source_revision: &str,
    runner: &GpuNarrowphaseRunner,
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
            named("absolute-tolerance", "0.00002"),
            named(
                "authority-randomized-seeds",
                &AUTHORITY_RANDOMIZED_SEEDS.to_string(),
            ),
            named("candidate-stage-dispatches", "8"),
            named("gpu-randomized-seeds", &GPU_RANDOMIZED_SEEDS.to_string()),
            named("narrowphase-stage-dispatches", "7"),
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
