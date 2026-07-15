use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, FailureKindV1, FailureV1, OracleRequestV1, SCHEMA_VERSION, ScenarioV1, ShapeV1,
    canonical_hash, canonical_json, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactOutput, GpuGeneratedContactRunner, P5_2CandidatePattern,
    P5_2GeneratedScenarioKind, benchmark_gpu_generated_contacts, p5_2_candidate_scenario,
    p5_2_generated_performance_scenario, p5_2_generated_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    projected_impulses,
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const RANDOMIZED_SEEDS: u64 = 32;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    adapter: physbox_engine::AdapterDiagnostics,
    outcome: Outcome,
    behavior: Vec<BehaviorRow>,
    randomized: RandomizedRow,
    capacity_negative: GpuGeneratedContactOutput,
    known_failures: Vec<KnownFailure>,
    performance: Vec<PerformanceRow>,
    optimized_box2d: Vec<OptimizedBox2dRow>,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    playback: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    implementation_accepted: bool,
    behavior_cases: usize,
    exact_replays: usize,
    randomized_cases: u64,
    randomized_replay_differences: u64,
    accepted_overflows: u64,
    authority_failures: u64,
    known_failure_count: usize,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BehaviorRow {
    label: String,
    completed_steps: u32,
    source_scenario_hash: String,
    oracle_projection_hash: String,
    oracle_artifact: String,
    replay_exact: bool,
    projected_impulses: bool,
    gpu_penetration_max: f32,
    gpu_energy_delta: f64,
    box2d_energy_delta: f64,
    maximum_position_delta: f32,
    maximum_angle_delta: f32,
    maximum_linear_velocity_delta: f32,
    maximum_angular_velocity_delta: f32,
    initial_bodies: Vec<BodyStateV1>,
    gpu_bodies: Vec<BodyStateV1>,
    box2d_bodies: Vec<BodyStateV1>,
    gpu_cache_points: usize,
    gpu_work: physbox_contracts::CollisionWorkV1,
    gpu_capacities: Vec<physbox_contracts::CollisionCapacityV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedRow {
    seeds: u64,
    patterns: Vec<&'static str>,
    cases: u64,
    replay_differences: u64,
    accepted_overflows: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KnownFailure {
    failure: FailureV1,
    minimized_body_count: u32,
    scenario_artifact: String,
    maximum_linear_speed: f32,
    maximum_angular_speed: f32,
    initial_energy: f64,
    final_energy: f64,
    reconsideration_trigger: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceRow {
    bodies: u32,
    expected_contacts: u32,
    all_pairs_tests: u64,
    candidate_amplification: f64,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    gpu_collision: Option<Timing>,
    gpu_end_to_end: Option<Timing>,
    host_completion: Timing,
    cpu_encode_submit: Timing,
    timestamp_period_nanoseconds: Option<f32>,
    capacity_accepted: bool,
    failure: Option<FailureV1>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Timing {
    samples: usize,
    minimum_seconds: f64,
    p50_seconds: f64,
    p95_seconds: f64,
    p99_seconds: f64,
    maximum_seconds: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptimizedBox2dRow {
    bodies: u32,
    expected_contacts: u32,
    workers: u32,
    scope: String,
    source_revision: String,
    source_dirty: bool,
    contacts_min: u64,
    contacts_max: u64,
    timing: Timing,
    artifact: String,
    success_sentinel: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformArtifact {
    label: String,
    source: String,
    copied_to: String,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-generated-contact-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("generated-contact evidence requires a clean source tree".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("oracles")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("box2d")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("failures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut behavior = Vec::new();
    for (kind, steps) in behavior_cases() {
        let scenario = p5_2_generated_scenario(kind, steps).map_err(|error| error.to_string())?;
        behavior.push(evaluate_behavior(
            &arguments,
            &staging,
            &box2d,
            &runner,
            kind.name(),
            scenario,
            steps,
        )?);
    }
    let speculative = speculative_gap_scenario()?;
    behavior.push(evaluate_behavior(
        &arguments,
        &staging,
        &box2d,
        &runner,
        "p5.2-generated-speculative-gap",
        speculative,
        1,
    )?);

    let randomized = randomized_evidence(&runner)?;
    let capacity_scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 34, 97)
        .map_err(|error| error.to_string())?;
    let capacity_negative = runner.run(&capacity_scenario, 1)?;
    if capacity_negative.failure.is_none()
        || !capacity_negative.cache.is_empty()
        || capacity_negative
            .capacities
            .iter()
            .all(|capacity| capacity.accepted)
    {
        return Err("dense capacity negative was not rejected atomically".to_owned());
    }
    let known_failures = known_failures(&staging, &runner)?;
    let performance = performance_evidence(arguments.warmup, arguments.samples)?;
    if performance
        .iter()
        .any(|row| !row.capacity_accepted || row.failure.is_some())
    {
        return Err("a generated-contact performance row failed correctness/capacity".to_owned());
    }
    let optimized_box2d = optimized_box2d_evidence(&arguments, &staging)?;
    if optimized_box2d.iter().any(|row| {
        row.source_revision != BOX2D_REVISION
            || row.source_dirty
            || !row.success_sentinel
            || row.contacts_min != u64::from(row.expected_contacts)
            || row.contacts_max != u64::from(row.expected_contacts)
    }) {
        return Err("an optimized Box2D row failed provenance/contact gates".to_owned());
    }
    let platform_artifacts =
        copy_platform_artifacts(&arguments.platforms, &staging, &source_revision)?;
    render_playback(&staging.join("playback.svg"), &behavior, &known_failures)?;

    let exact_replays = behavior.iter().filter(|row| row.replay_exact).count();
    let authority_failures = behavior.len()
        - behavior
            .iter()
            .filter(|row| !row.box2d_bodies.is_empty())
            .count();
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.5a-generated-contact-authority",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        adapter: runner.adapter().clone(),
        outcome: Outcome {
            implementation_accepted: exact_replays == behavior.len()
                && randomized.replay_differences == 0
                && randomized.accepted_overflows == 0
                && authority_failures == 0
                && known_failures.is_empty(),
            behavior_cases: behavior.len(),
            exact_replays,
            randomized_cases: randomized.cases,
            randomized_replay_differences: randomized.replay_differences,
            accepted_overflows: randomized.accepted_overflows,
            authority_failures: authority_failures as u64,
            known_failure_count: known_failures.len(),
            physical_defaults_changed: false,
            numerical_tolerances_changed: false,
        },
        behavior,
        randomized,
        capacity_negative,
        known_failures,
        performance,
        optimized_box2d,
        platform_artifacts,
        physical_device_gap: arguments.physical_device_gap,
        playback: "playback.svg",
    };
    atomic_json(&staging.join("matrix.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    println!(
        "PHYSBOX5_GENERATED_CONTACT_EVIDENCE_SUCCESS run_id={} output={}",
        manifest.run_id,
        arguments.output.display()
    );
    Ok(())
}

fn behavior_cases() -> [(P5_2GeneratedScenarioKind, u32); 8] {
    [
        (P5_2GeneratedScenarioKind::RectangleFloor, 120),
        (P5_2GeneratedScenarioKind::CircleFloor, 120),
        (P5_2GeneratedScenarioKind::BilateralCircles, 1),
        (P5_2GeneratedScenarioKind::OffCenterRectangles, 4),
        (P5_2GeneratedScenarioKind::SmallStack, 180),
        (P5_2GeneratedScenarioKind::FrictionSlide, 60),
        (P5_2GeneratedScenarioKind::Restitution, 30),
        (P5_2GeneratedScenarioKind::PairChurn, 30),
    ]
}

fn speculative_gap_scenario() -> Result<ScenarioV1, String> {
    let mut scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::RectangleFloor, 1)
        .map_err(|error| error.to_string())?;
    scenario.id = "p5.2-generated-speculative-gap".to_owned();
    scenario.bodies[0].position.y = 0.205;
    scenario.bodies[0].linear_velocity.y = -0.5;
    scenario.settings.contact_policy = None;
    Ok(scenario)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_behavior(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    label: &str,
    scenario: ScenarioV1,
    steps: u32,
) -> Result<BehaviorRow, String> {
    let gpu = runner.run(&scenario, steps)?;
    let replay = runner.run(&scenario, steps)?;
    let replay_exact = gpu.bodies == replay.bodies
        && gpu.cache == replay.cache
        && gpu.work == replay.work
        && gpu.capacities == replay.capacities;
    if gpu.failure.is_some() || !replay_exact {
        return Err(format!("{label} failed GPU execution or exact replay"));
    }
    let projected = gpu.cache.iter().all(|contact| {
        let body_a = &scenario.bodies[contact.key.pair.body_a.slot as usize];
        let body_b = &scenario.bodies[contact.key.pair.body_b.slot as usize];
        let friction = (body_a.material.friction.get() * body_b.material.friction.get()).sqrt();
        projected_impulses(
            contact.value.normal_impulse,
            contact.value.tangent_impulse,
            physbox_contracts::FiniteF32::new(friction).expect("finite scenario friction"),
        )
        .is_ok()
    });
    if !projected {
        return Err(format!("{label} produced an unprojected cached impulse"));
    }
    let source_scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
    let oracle_run_id = format!("{}-{}", arguments.run_id, label.replace('.', "-"));
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: oracle_run_id,
        source_scenario_hash: source_scenario_hash.clone(),
        scenario: scenario.clone(),
    };
    let oracle = run_oracle(
        &OracleRunConfig {
            artifact_root: staging.join("oracles"),
            timeout: Duration::from_secs(30),
            source: box2d.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![OsString::from("--lane"), OsString::from("world")],
            },
        },
        &request,
    )
    .map_err(|error| error.to_string())?;
    let (position, angle, linear_velocity, angular_velocity) =
        body_deltas(&gpu.bodies, &oracle.result.bodies)?;
    let oracle_artifact = oracle
        .artifacts
        .directory
        .strip_prefix(staging)
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(BehaviorRow {
        label: label.to_owned(),
        completed_steps: steps,
        source_scenario_hash,
        oracle_projection_hash: oracle.projection_hash,
        oracle_artifact,
        replay_exact,
        projected_impulses: projected,
        gpu_penetration_max: penetration(&scenario, &gpu.bodies)?,
        gpu_energy_delta: energy(&scenario, &gpu.bodies) - energy(&scenario, &scenario.bodies),
        box2d_energy_delta: energy(&scenario, &oracle.result.bodies)
            - energy(&scenario, &scenario.bodies),
        maximum_position_delta: position,
        maximum_angle_delta: angle,
        maximum_linear_velocity_delta: linear_velocity,
        maximum_angular_velocity_delta: angular_velocity,
        initial_bodies: scenario.bodies,
        gpu_bodies: gpu.bodies,
        box2d_bodies: oracle.result.bodies,
        gpu_cache_points: gpu.cache.len(),
        gpu_work: gpu.work,
        gpu_capacities: gpu.capacities,
    })
}

fn randomized_evidence(runner: &GpuGeneratedContactRunner) -> Result<RandomizedRow, String> {
    let patterns = [
        (P5_2CandidatePattern::Random, "random"),
        (P5_2CandidatePattern::Touching, "touching"),
        (P5_2CandidatePattern::RotatedThin, "rotated-thin"),
        (P5_2CandidatePattern::FixedOnly, "fixed-only"),
    ];
    let mut replay_differences = 0;
    let mut accepted_overflows = 0;
    for seed in 0..RANDOMIZED_SEEDS {
        for (pattern, _) in patterns {
            let scenario =
                p5_2_candidate_scenario(pattern, 16, seed).map_err(|error| error.to_string())?;
            let first = runner.run(&scenario, 2)?;
            let replay = runner.run(&scenario, 2)?;
            replay_differences += u64::from(
                first.bodies != replay.bodies
                    || first.cache != replay.cache
                    || first.work != replay.work,
            );
            accepted_overflows += u64::from(
                first.failure.is_none()
                    && first.capacities.iter().any(|capacity| capacity.overflowed),
            );
        }
    }
    Ok(RandomizedRow {
        seeds: RANDOMIZED_SEEDS,
        patterns: patterns.into_iter().map(|(_, name)| name).collect(),
        cases: RANDOMIZED_SEEDS * patterns.len() as u64,
        replay_differences,
        accepted_overflows,
    })
}

fn known_failures(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
) -> Result<Vec<KnownFailure>, String> {
    let mut minimized = None;
    for body_count in 2..=16 {
        let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, body_count, 19)
            .map_err(|error| error.to_string())?;
        let output = runner.run(&scenario, 1)?;
        let speed = maximum_linear_speed(&output.bodies);
        if speed > 100.0 {
            minimized = Some((scenario, output, speed));
            break;
        }
    }
    let (scenario, output, reproduced_linear_speed) = minimized
        .ok_or_else(|| "deep-coincidence energy failure no longer reproduces".to_owned())?;
    let path = staging.join("failures/deep-coincident-energy.json");
    atomic_json(&path, &scenario)?;
    let hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
    let mut failures = vec![KnownFailure {
        failure: FailureV1 {
            kind: FailureKindV1::Invariant,
            reason: "deep coincident spawn injects extreme kinetic energy".to_owned(),
            first_step: Some(1),
            first_stage: Some("generated-contact-solve".to_owned()),
            reproducer_hash: Some(hash),
        },
        minimized_body_count: scenario.bodies.len() as u32,
        scenario_artifact: "failures/deep-coincident-energy.json".to_owned(),
        maximum_linear_speed: reproduced_linear_speed,
        maximum_angular_speed: output
            .bodies
            .iter()
            .map(|body| body.angular_velocity.get().abs())
            .fold(0.0, f32::max),
        initial_energy: energy(&scenario, &scenario.bodies),
        final_energy: energy(&scenario, &output.bodies),
        reconsideration_trigger: "choose a reviewed maximum depenetration or spawn-overlap policy",
    }];
    let speculative = speculative_gap_scenario()?;
    let speculative_output = runner.run(&speculative, 1)?;
    let speculative_path = staging.join("failures/speculative-gap.json");
    atomic_json(&speculative_path, &speculative)?;
    failures.push(KnownFailure {
        failure: FailureV1 {
            kind: FailureKindV1::NumericalDivergence,
            reason: "the product collision path has no reviewed speculative-contact margin"
                .to_owned(),
            first_step: Some(1),
            first_stage: Some("candidate-aabb".to_owned()),
            reproducer_hash: Some(canonical_hash(&speculative).map_err(|error| error.to_string())?),
        },
        minimized_body_count: 2,
        scenario_artifact: "failures/speculative-gap.json".to_owned(),
        maximum_linear_speed: maximum_linear_speed(&speculative_output.bodies),
        maximum_angular_speed: speculative_output
            .bodies
            .iter()
            .map(|body| body.angular_velocity.get().abs())
            .fold(0.0, f32::max),
        initial_energy: energy(&speculative, &speculative.bodies),
        final_energy: energy(&speculative, &speculative_output.bodies),
        reconsideration_trigger: "select a portable speculative margin and matching AABB policy",
    });
    Ok(failures)
}

fn performance_evidence(warmup: u32, samples: u32) -> Result<Vec<PerformanceRow>, String> {
    let mut rows = Vec::new();
    for bodies in [32_u32, 256, 1024] {
        let scenario =
            p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
        let benchmark =
            pollster::block_on(benchmark_gpu_generated_contacts(&scenario, warmup, samples))?;
        let expected_contacts = bodies / 2;
        let all_pairs_tests = u64::from(bodies) * u64::from(bodies - 1) / 2;
        rows.push(PerformanceRow {
            bodies,
            expected_contacts,
            all_pairs_tests,
            candidate_amplification: all_pairs_tests as f64 / f64::from(expected_contacts),
            dispatches_per_step: benchmark.dispatches_per_step,
            allocated_bytes: benchmark.allocated_bytes,
            gpu_collision: benchmark
                .gpu_collision_seconds
                .as_deref()
                .map(timing)
                .transpose()?,
            gpu_end_to_end: benchmark
                .gpu_end_to_end_seconds
                .as_deref()
                .map(timing)
                .transpose()?,
            host_completion: timing(&benchmark.host_completion_seconds)?,
            cpu_encode_submit: timing(&benchmark.cpu_encode_submit_seconds)?,
            timestamp_period_nanoseconds: benchmark.timestamp_period_nanoseconds,
            capacity_accepted: benchmark
                .correctness
                .capacities
                .iter()
                .all(|capacity| capacity.accepted),
            failure: benchmark.correctness.failure,
        });
    }
    Ok(rows)
}

fn optimized_box2d_evidence(
    arguments: &Arguments,
    staging: &Path,
) -> Result<Vec<OptimizedBox2dRow>, String> {
    let mut rows = Vec::new();
    for bodies in [32_u32, 256, 1024] {
        let scenario =
            p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
        let request = staging.join(format!("box2d/scenario-{bodies}.json"));
        let request_bytes = canonical_json(&scenario).map_err(|error| error.to_string())?;
        atomic_write(&request, &request_bytes)?;
        for workers in [1_u32, 4] {
            let relative = format!("box2d/bodies-{bodies}-workers-{workers}.json");
            let result = staging.join(&relative);
            let output = Command::new(&arguments.box2d_benchmark)
                .args([
                    "--request",
                    &request.to_string_lossy(),
                    "--result",
                    &result.to_string_lossy(),
                    "--run-id",
                    &format!("{}-b2-{bodies}-w{workers}", arguments.run_id),
                    "--workers",
                    &workers.to_string(),
                    "--warmup",
                    &arguments.warmup.to_string(),
                    "--samples",
                    &arguments.samples.to_string(),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| error.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !output.status.success() {
                return Err(format!(
                    "optimized Box2D failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&result).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let samples = value["samplesSeconds"]
                .as_array()
                .ok_or_else(|| "Box2D samples are missing".to_owned())?
                .iter()
                .map(|sample| {
                    sample
                        .as_f64()
                        .ok_or_else(|| "invalid Box2D sample".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(OptimizedBox2dRow {
                bodies,
                expected_contacts: bodies / 2,
                workers,
                scope: string(&value, "scope")?,
                source_revision: string(&value, "sourceRevision")?,
                source_dirty: value["sourceDirty"].as_bool().unwrap_or(true),
                contacts_min: value["contactCountMin"].as_u64().unwrap_or(u64::MAX),
                contacts_max: value["contactCountMax"].as_u64().unwrap_or(u64::MAX),
                timing: timing(&samples)?,
                artifact: relative,
                success_sentinel: stdout.contains("PHYSBOX5_BOX2D_BENCHMARK_SUCCESS"),
            });
        }
    }
    Ok(rows)
}

fn copy_platform_artifacts(
    sources: &[(String, PathBuf)],
    staging: &Path,
    source_revision: &str,
) -> Result<Vec<PlatformArtifact>, String> {
    let mut artifacts = Vec::new();
    for (label, source) in sources {
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin");
        let name = format!("platform/{label}.{extension}");
        let bytes =
            fs::read(source).map_err(|error| format!("read {}: {error}", source.display()))?;
        if extension == "json" {
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            if value["sourceRevision"].as_str() != Some(source_revision)
                || value["sourceDirty"].as_bool() != Some(false)
                || value["report"]["sceneContract"]["scenario"].as_str() != Some("rigid-stack")
                || value["report"]["sceneContract"]["capacityOverflowed"].as_bool() != Some(false)
                || value["report"]["scenario"]["passed"].as_bool() != Some(true)
            {
                return Err(format!(
                    "platform artifact {} failed revision/scene/capacity gates",
                    source.display()
                ));
            }
        }
        atomic_write(&staging.join(&name), &bytes)?;
        artifacts.push(PlatformArtifact {
            label: label.clone(),
            source: source.display().to_string(),
            copied_to: name,
            sha256: sha256_bytes(&bytes),
        });
    }
    Ok(artifacts)
}

fn penetration(scenario: &ScenarioV1, bodies: &[BodyStateV1]) -> Result<f32, String> {
    let mut final_scenario = scenario.clone();
    final_scenario.bodies.clone_from_slice(bodies);
    let candidates = enumerate_scalar_candidates(&final_scenario)?;
    let manifolds = generate_scalar_narrowphase(&final_scenario, &candidates.candidates)?;
    Ok(manifolds
        .iter()
        .flat_map(|manifold| &manifold.points)
        .map(|point| (-point.separation.get()).max(0.0))
        .fold(0.0, f32::max))
}

fn energy(scenario: &ScenarioV1, bodies: &[BodyStateV1]) -> f64 {
    bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .map(|body| {
            let mass = 1.0 / f64::from(body.inverse_mass.get());
            let inertia = if body.inverse_inertia.get() > 0.0 {
                1.0 / f64::from(body.inverse_inertia.get())
            } else {
                0.0
            };
            let vx = f64::from(body.linear_velocity.x);
            let vy = f64::from(body.linear_velocity.y);
            let omega = f64::from(body.angular_velocity.get());
            let potential = -mass
                * (f64::from(scenario.settings.gravity.x) * f64::from(body.position.x)
                    + f64::from(scenario.settings.gravity.y) * f64::from(body.position.y));
            0.5 * mass * (vx * vx + vy * vy) + 0.5 * inertia * omega * omega + potential
        })
        .sum()
}

fn maximum_linear_speed(bodies: &[BodyStateV1]) -> f32 {
    bodies
        .iter()
        .map(|body| body.linear_velocity.x.hypot(body.linear_velocity.y))
        .fold(0.0, f32::max)
}

fn body_deltas(
    left: &[BodyStateV1],
    right: &[BodyStateV1],
) -> Result<(f32, f32, f32, f32), String> {
    if left.len() != right.len() {
        return Err("GPU and Box2D body counts differ".to_owned());
    }
    let mut output = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for (left, right) in left.iter().zip(right) {
        output.0 = output
            .0
            .max((left.position.x - right.position.x).hypot(left.position.y - right.position.y));
        output.1 = output.1.max((left.angle.get() - right.angle.get()).abs());
        output.2 = output.2.max(
            (left.linear_velocity.x - right.linear_velocity.x)
                .hypot(left.linear_velocity.y - right.linear_velocity.y),
        );
        output.3 = output
            .3
            .max((left.angular_velocity.get() - right.angular_velocity.get()).abs());
    }
    Ok(output)
}

fn timing(samples: &[f64]) -> Result<Timing, String> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|sample| !sample.is_finite() || *sample <= 0.0)
    {
        return Err("timing samples must be finite and positive".to_owned());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(Timing {
        samples: sorted.len(),
        minimum_seconds: sorted[0],
        p50_seconds: percentile(&sorted, 0.50),
        p95_seconds: percentile(&sorted, 0.95),
        p99_seconds: percentile(&sorted, 0.99),
        maximum_seconds: *sorted.last().expect("non-empty samples"),
    })
}

fn percentile(samples: &[f64], quantile: f64) -> f64 {
    let index = ((samples.len() - 1) as f64 * quantile).ceil() as usize;
    samples[index]
}

fn render_playback(
    path: &Path,
    behavior: &[BehaviorRow],
    failures: &[KnownFailure],
) -> Result<(), String> {
    let height = 80 + behavior.len() * 88;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"{height}\" viewBox=\"0 0 1000 {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:11px;fill:#91a8ba}}</style><text class=\"title\" x=\"18\" y=\"28\">PhysBox 5 generated-contact / Box2D synchronized endpoints</text><text class=\"meta\" x=\"18\" y=\"48\">same neutral scenarios · exact completed steps · objective deltas only · {} known minimized failure(s)</text>",
        failures.len()
    );
    for (row_index, row) in behavior.iter().enumerate() {
        let y = 82 + row_index * 88;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{y}\" font-size=\"12\">{} · {} steps · Δp {:.5} · Δv {:.5}</text>",
            row.label,
            row.completed_steps,
            row.maximum_position_delta,
            row.maximum_linear_velocity_delta
        ));
        for (lane, bodies, color) in [
            ("initial", row.initial_bodies.as_slice(), "#7289a0"),
            ("gpu", row.gpu_bodies.as_slice(), "#5ee1dc"),
            ("box2d", row.box2d_bodies.as_slice(), "#ffc267"),
        ] {
            let lane_y = y
                + 18
                + match lane {
                    "initial" => 0,
                    "gpu" => 18,
                    _ => 36,
                };
            svg.push_str(&format!(
                "<text class=\"meta\" x=\"38\" y=\"{lane_y}\">{lane}</text>"
            ));
            for body in bodies.iter().filter(|body| body.inverse_mass.get() > 0.0) {
                let x = 210.0 + f64::from(body.position.x) * 42.0;
                let py = lane_y as f64 - f64::from(body.position.y) * 12.0;
                let (width, radius) = match body.shape {
                    ShapeV1::Circle { radius } => (f64::from(radius.get()) * 18.0, true),
                    ShapeV1::Rectangle { half_extents } => {
                        (f64::from(half_extents.x) * 30.0, false)
                    }
                };
                if radius {
                    svg.push_str(&format!(
                        "<circle cx=\"{x:.2}\" cy=\"{py:.2}\" r=\"{width:.2}\" fill=\"{color}\"/>"
                    ));
                } else {
                    svg.push_str(&format!("<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"10\" rx=\"2\" fill=\"{color}\"/>", x - width, py - 5.0, width * 2.0));
                }
            }
        }
    }
    svg.push_str("</svg>\n");
    atomic_write(path, svg.as_bytes())
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_text(
        Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["rev-parse", "HEAD"]),
    )?;
    let status = command_bytes(Command::new("git").arg("-C").arg(repository).args([
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
    ]))?;
    let dirty = !status.is_empty();
    let digest = dirty.then(|| sha256_bytes(&status));
    Ok((revision, dirty, digest))
}

fn command_text(command: &mut Command) -> Result<String, String> {
    let bytes = command_bytes(command)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn command_bytes(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output.stdout)
}

fn string(value: &serde_json::Value, name: &str) -> Result<String, String> {
    value[name]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string {name}"))
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension("partial");
    if path.exists() || partial.exists() {
        return Err(format!("fresh artifact already exists: {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(partial, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    box2d_benchmark: PathBuf,
    warmup: u32,
    samples: u32,
    require_clean: bool,
    platforms: Vec<(String, PathBuf)>,
    physical_device_gap: Option<String>,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut box2d_benchmark = None;
        let mut warmup = 5;
        let mut samples = 20;
        let mut require_clean = false;
        let mut platforms = Vec::new();
        let mut physical_device_gap = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "--require-clean" {
                require_clean = true;
                continue;
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--oracle" => oracle = Some(PathBuf::from(value)),
                "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
                "--box2d-benchmark" => box2d_benchmark = Some(PathBuf::from(value)),
                "--warmup" => warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--samples" => samples = value.parse().map_err(|_| "invalid samples")?,
                "--platform" => {
                    let (label, path) = value
                        .split_once('=')
                        .ok_or_else(|| "platform must be LABEL=PATH".to_owned())?;
                    platforms.push((label.to_owned(), PathBuf::from(path)));
                }
                "--physical-device-gap" => physical_device_gap = Some(value),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        if warmup == 0 || samples == 0 {
            return Err("warmup and samples must be non-zero".to_owned());
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            box2d_benchmark: box2d_benchmark
                .ok_or_else(|| "missing --box2d-benchmark".to_owned())?,
            warmup,
            samples,
            require_clean,
            platforms,
            physical_device_gap,
        })
    }
}
