use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyStateV1, CONTACT_SOFTNESS_POLICY_VERSION, ContactIdentityPolicyV1, ContactSoftnessPolicyV1,
    FiniteF32, SCHEMA_VERSION, ScenarioV1, SettlingObservationV2, SettlingSeriesV2, ShapeV1,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactBenchmarkOutput, GpuGeneratedContactRunner, P5_2_SETTLING_EVIDENCE_STEPS,
    P5_2GeneratedScenarioKind, P5_2SettlingFixtureKind,
    benchmark_gpu_generated_contact_identity_experiment, p5_2_box2d_world_projection,
    p5_2_centered_rectangle_stack_scenario, p5_2_generated_performance_scenario,
    p5_2_generated_scenario, p5_2_random_settling_fixture_scenario, p5_2_settling_fixture_scenario,
};
use physbox_lab::settling::{
    AWAKE_SETTLING_TRACE_VERSION, awake_settling_measurement_policy_v2, centered_stack_structure,
    observe_generated_settling_series_v2, settling_fixture_structure, settling_series_v2,
};
use serde::Serialize;

const IDENTITY_MANIFEST: &str = "evidence/p5.2b-s6.3-contact-identity/manifest.json";
const RANDOM_FIRST_SEED: u64 = 0x6400;
const RANDOM_SEED_COUNT: u32 = 14;
const PLAYBACK_STEPS: [u32; 3] = [600, 900, 1_200];

#[derive(Clone, Copy)]
enum SceneKind {
    Stack,
    Fixture(P5_2SettlingFixtureKind),
}

struct SceneInput {
    label: String,
    kind: SceneKind,
    source: ScenarioV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    id: String,
    identity_policy: ContactIdentityPolicyV1,
    softness: Option<ContactSoftnessPolicyV1>,
    role: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    observation_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    adapter: physbox_engine::AdapterDiagnostics,
    identity_manifest: ArtifactIdentity,
    screen_candidates: Vec<Candidate>,
    screen_scene_labels: Vec<String>,
    screen_results: Vec<SceneEvidence>,
    screen_ranking: Vec<CandidateRanking>,
    selected_screen_winner: Candidate,
    finalists: Vec<Candidate>,
    full_results: Vec<SceneEvidence>,
    finalist_summaries: Vec<CandidateRanking>,
    randomized: Vec<RandomizedEvidence>,
    response_checks: Vec<ResponseCheck>,
    timing: Vec<TimingEvidence>,
    playback: ArtifactIdentity,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    authority_source_scenario_hash: String,
    authority_projection_hash: String,
    source: ArtifactIdentity,
    candidates: Vec<CandidateEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEvidence {
    candidate: Candidate,
    candidate_scenario_hash: String,
    raw_trace_hash: String,
    trace: ArtifactIdentity,
    replay_exact: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    hard_gate_pass: bool,
    hard_gate_failures: Vec<String>,
    physical_envelope: PhysicalEnvelope,
    summary: physbox_contracts::SettlingSummaryV2,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalEnvelope {
    settled_center_drift_max: f32,
    settled_stack_height_min: f32,
    settled_stack_height_max: f32,
    settled_penetration_max: f32,
    settled_separation_max: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateRanking {
    rank: u32,
    candidate: Candidate,
    scene_count: u32,
    hard_gate_passes: u32,
    structural_passes: u32,
    energy_envelope_passes: u32,
    replay_exact_runs: u32,
    capacity_clean_runs: u32,
    settled_churn: u64,
    maximum_settled_linear_speed: f32,
    maximum_settled_angular_edge_speed: f32,
    maximum_settled_penetration: f32,
    ranking_contract: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedEvidence {
    candidate: Candidate,
    first_seed: u64,
    seed_count: u32,
    exact_replays: u32,
    capacity_clean_runs: u32,
    structural_passes: u32,
    final_rest_passes: u32,
    cases: Vec<RandomizedCase>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedCase {
    seed: u64,
    kind: String,
    candidate_scenario_hash: String,
    result_hash: String,
    replay_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    final_linear_speed_max: f32,
    final_angular_edge_speed_max: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseCheck {
    candidate: Candidate,
    bilateral_impact_pass: bool,
    off_center_angular_response_pass: bool,
    friction_response_pass: bool,
    restitution_response_pass: bool,
    short_stack_order_pass: bool,
    exact_replay: bool,
    capacity_clean: bool,
    result_hashes: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingEvidence {
    candidate: Candidate,
    body_count: u32,
    samples: u32,
    gpu_end_to_end_p50_seconds: Option<f64>,
    host_completion_p50_seconds: f64,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    work_digest: String,
    raw: ArtifactIdentity,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

type PlaybackSamples = Vec<(u32, Vec<BodyStateV1>)>;

struct PlaybackScene {
    label: String,
    source: ScenarioV1,
    lanes: Vec<(Candidate, PlaybackSamples)>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-soft-contact-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S6.4 soft-contact evidence requires a clean source tree".to_owned());
    }
    let identity_bytes = fs::read(repository.join(IDENTITY_MANIFEST)).map_err(display_error)?;
    let identity: serde_json::Value =
        serde_json::from_slice(&identity_bytes).map_err(display_error)?;
    if identity["artifactKind"] != "p5.2b-s6.3-contact-identity"
        || identity["sourceDirty"] != false
        || identity["physicalDefaultsChanged"] != false
    {
        return Err("S6.3 manifest is not the frozen clean identity control".to_owned());
    }

    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    for directory in ["sources", "screen-traces", "full-traces", "timing"] {
        fs::create_dir_all(staging.join(directory)).map_err(display_error)?;
    }

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let screen_candidates = screen_candidates();
    let screen_inputs = screen_scene_inputs()?;
    let screen_scene_labels = screen_inputs
        .iter()
        .map(|input| input.label.clone())
        .collect();
    let mut screen_results = Vec::new();
    for input in screen_inputs {
        eprintln!("S6.4 screen {}", input.label);
        screen_results.push(
            evaluate_scene(
                &staging,
                "screen",
                &runner,
                input,
                &screen_candidates,
                false,
            )?
            .0,
        );
    }
    let screen_ranking = rank_candidates(&screen_results, &screen_candidates);
    let selected_screen_winner = screen_ranking
        .first()
        .ok_or_else(|| "soft-contact screen produced no ranking".to_owned())?
        .candidate
        .clone();
    let finalists = finalists(&selected_screen_winner);

    let mut full_results = Vec::new();
    let mut playback = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S6.4 finalist {}", input.label);
        let (scene, playback_scene) =
            evaluate_scene(&staging, "full", &runner, input, &finalists, true)?;
        full_results.push(scene);
        playback.push(playback_scene.expect("requested playback"));
    }
    let finalist_summaries = rank_candidates(&full_results, &finalists);
    let randomized = finalists
        .iter()
        .map(|candidate| randomized_evidence(&runner, candidate))
        .collect::<Result<Vec<_>, _>>()?;
    let response_checks = finalists
        .iter()
        .map(|candidate| response_check(&runner, candidate))
        .collect::<Result<Vec<_>, _>>()?;
    let timing = timing_evidence(&staging, &finalists, arguments.timing_samples)?;
    let playback = write_artifact(
        &staging,
        "playback.svg",
        render_playback(&playback)?.as_bytes(),
    )?;

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s6.4-soft-contact-factorial",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        adapter: runner.diagnostics().clone(),
        identity_manifest: ArtifactIdentity {
            path: IDENTITY_MANIFEST.to_owned(),
            sha256: sha256_bytes(&identity_bytes),
        },
        screen_candidates,
        screen_scene_labels,
        screen_results,
        screen_ranking,
        selected_screen_winner,
        finalists,
        full_results,
        finalist_summaries,
        randomized,
        response_checks,
        timing,
        playback,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(display_error)?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes = fs::read(arguments.output.join("manifest.json")).map_err(display_error)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s6.4-soft-contact-factorial run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn screen_candidates() -> Vec<Candidate> {
    let mut candidates = vec![hard_candidate(
        "hard-direct-control",
        ContactIdentityPolicyV1::DirectMaximumSeparation,
        "unchanged-hard-control",
    )];
    for hertz in [15.0, 30.0] {
        for damping in [5.0, 10.0] {
            for static_multiplier in [1.0, 2.0] {
                candidates.push(Candidate {
                    id: format!(
                        "soft-hz{}-d{}-static{}",
                        hertz as u32, damping as u32, static_multiplier as u32
                    ),
                    identity_policy: ContactIdentityPolicyV1::DirectMaximumSeparation,
                    softness: Some(softness(hertz, damping, static_multiplier)),
                    role: "bounded-softness-screen".to_owned(),
                });
            }
        }
    }
    candidates
}

fn finalists(winner: &Candidate) -> Vec<Candidate> {
    vec![
        hard_candidate(
            "hard-direct-control",
            ContactIdentityPolicyV1::DirectMaximumSeparation,
            "unchanged-hard-control",
        ),
        hard_candidate(
            "hard-prior-identity-only",
            ContactIdentityPolicyV1::PriorContactHysteresis,
            "identity-only",
        ),
        Candidate {
            id: format!("{}-direct", winner.id),
            identity_policy: ContactIdentityPolicyV1::DirectMaximumSeparation,
            softness: winner.softness,
            role: "softness-only".to_owned(),
        },
        Candidate {
            id: format!("{}-prior", winner.id),
            identity_policy: ContactIdentityPolicyV1::PriorContactHysteresis,
            softness: winner.softness,
            role: "combined-identity-and-softness".to_owned(),
        },
    ]
}

fn hard_candidate(id: &str, identity_policy: ContactIdentityPolicyV1, role: &str) -> Candidate {
    Candidate {
        id: id.to_owned(),
        identity_policy,
        softness: None,
        role: role.to_owned(),
    }
}

fn softness(hertz: f32, damping: f32, static_multiplier: f32) -> ContactSoftnessPolicyV1 {
    ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(hertz),
        damping_ratio: finite(damping),
        static_contact_hertz_multiplier: finite(static_multiplier),
    }
}

fn screen_scene_inputs() -> Result<Vec<SceneInput>, String> {
    let all = scene_inputs()?;
    let selected = [
        "centered-stack-4",
        "centered-stack-8",
        "centered-stack-16",
        "sequential-rectangles-5-seed-6002",
        "small-mixed-pile-seed-6002",
    ];
    Ok(all
        .into_iter()
        .filter(|input| selected.contains(&input.label.as_str()))
        .collect())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [2, 4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display_error)?,
        });
    }
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display_error)?,
        });
    }
    Ok(inputs)
}

fn evaluate_scene(
    staging: &Path,
    phase: &str,
    runner: &GpuGeneratedContactRunner,
    input: SceneInput,
    candidates: &[Candidate],
    retain_playback: bool,
) -> Result<(SceneEvidence, Option<PlaybackScene>), String> {
    let authority_source_scenario_hash = canonical_hash(&input.source).map_err(display_error)?;
    let projection = p5_2_box2d_world_projection(&input.source).map_err(display_error)?;
    let authority_projection_hash = canonical_hash(&projection).map_err(display_error)?;
    let source = write_json_artifact(
        staging,
        &format!("sources/{phase}-{}.json", input.label),
        &input.source,
    )?;
    let mut evidence = Vec::new();
    let mut lanes = Vec::new();
    for candidate in candidates {
        let mut scenario = input.source.clone();
        scenario.settings.contact_softness = candidate.softness;
        let candidate_scenario_hash = canonical_hash(&scenario).map_err(display_error)?;
        let raw = runner.run_contact_policy_observation_series(
            &scenario,
            1,
            P5_2_SETTLING_EVIDENCE_STEPS,
            candidate.identity_policy,
        )?;
        let raw_trace_hash = serialized_hash(&raw)?;
        let replay = runner.run_contact_policy_observation_series(
            &scenario,
            1,
            P5_2_SETTLING_EVIDENCE_STEPS,
            candidate.identity_policy,
        )?;
        let replay_exact = raw_trace_hash == serialized_hash(&replay)?;
        drop(replay);
        let final_only = runner.run_identity_experiment(
            &scenario,
            P5_2_SETTLING_EVIDENCE_STEPS,
            candidate.identity_policy,
        )?;
        let final_snapshot = raw.snapshots.last().expect("validated nonempty series");
        let instrumentation_final_exact = final_snapshot.bodies == final_only.bodies
            && final_snapshot.manifolds == final_only.manifolds
            && final_snapshot.cache == final_only.cache;
        let capacity_clean = raw.snapshots.iter().all(|snapshot| {
            snapshot.failure.is_none()
                && snapshot
                    .capacities
                    .iter()
                    .all(|capacity| capacity.accepted && !capacity.overflowed)
        });
        let (structural_pass, structural_reason) = structure(
            input.kind,
            &input.source,
            &final_snapshot.bodies,
            !capacity_clean,
        );
        let observations = observe_generated_settling_series_v2(&scenario, &raw)?;
        let physical_envelope = physical_envelope(&observations);
        let trace = settling_series_v2(
            &scenario,
            Some(authority_projection_hash.clone()),
            &candidate.id,
            awake_settling_measurement_policy_v2(),
            observations,
            structural_pass,
            structural_reason.clone(),
        )?;
        let trace_identity = write_gzip_json_artifact(
            staging,
            &format!("{phase}-traces/{}-{}.json.gz", input.label, candidate.id),
            &trace,
        )?;
        let mut hard_gate_failures = hard_gate_failures(&trace);
        if !replay_exact {
            hard_gate_failures.push("exact replay failed".to_owned());
        }
        if !instrumentation_final_exact {
            hard_gate_failures.push("instrumented final differed from final-only".to_owned());
        }
        if !capacity_clean {
            hard_gate_failures.push("capacity failed".to_owned());
        }
        if retain_playback {
            lanes.push((
                candidate.clone(),
                raw.snapshots
                    .iter()
                    .filter(|snapshot| PLAYBACK_STEPS.contains(&snapshot.completed_steps))
                    .map(|snapshot| (snapshot.completed_steps, snapshot.bodies.clone()))
                    .collect(),
            ));
        }
        evidence.push(CandidateEvidence {
            candidate: candidate.clone(),
            candidate_scenario_hash,
            raw_trace_hash,
            trace: trace_identity,
            replay_exact,
            instrumentation_final_exact,
            capacity_clean,
            structural_pass,
            structural_reason,
            hard_gate_pass: hard_gate_failures.is_empty(),
            hard_gate_failures,
            physical_envelope,
            summary: trace.summary,
        });
    }
    Ok((
        SceneEvidence {
            label: input.label.clone(),
            authority_source_scenario_hash,
            authority_projection_hash,
            source,
            candidates: evidence,
        },
        retain_playback.then_some(PlaybackScene {
            label: input.label,
            source: input.source,
            lanes,
        }),
    ))
}

fn physical_envelope(observations: &[SettlingObservationV2]) -> PhysicalEnvelope {
    let settled = observations
        .iter()
        .filter(|observation| observation.base.completed_steps >= 900)
        .collect::<Vec<_>>();
    PhysicalEnvelope {
        settled_center_drift_max: settled
            .iter()
            .map(|observation| observation.base.center_drift_max.get())
            .fold(0.0, f32::max),
        settled_stack_height_min: settled
            .iter()
            .map(|observation| observation.base.stack_height.get())
            .fold(f32::INFINITY, f32::min),
        settled_stack_height_max: settled
            .iter()
            .map(|observation| observation.base.stack_height.get())
            .fold(0.0, f32::max),
        settled_penetration_max: settled
            .iter()
            .map(|observation| observation.base.penetration_max.get())
            .fold(0.0, f32::max),
        settled_separation_max: settled
            .iter()
            .map(|observation| observation.base.separation_max.get())
            .fold(0.0, f32::max),
    }
}

fn hard_gate_failures(trace: &SettlingSeriesV2) -> Vec<String> {
    let mut failures = Vec::new();
    if !trace.structural_pass {
        failures.push(trace.structural_reason.clone());
    }
    if trace.summary.settled_linear_speed_max.get() > trace.policy.rest_linear_speed_limit.get() {
        failures.push("settled linear speed exceeded 0.025 m/s".to_owned());
    }
    if trace.summary.settled_angular_edge_speed_max.get()
        > trace.policy.rest_angular_edge_speed_limit.get()
    {
        failures.push("settled angular-edge speed exceeded 0.025 m/s".to_owned());
    }
    if trace.summary.stable_support_unexplained_churn != 0 {
        failures.push("settled stable support churned".to_owned());
    }
    if !trace.summary.energy_envelope_nonincreasing {
        failures.push("settled energy envelope increased".to_owned());
    }
    failures
}

fn rank_candidates(scenes: &[SceneEvidence], candidates: &[Candidate]) -> Vec<CandidateRanking> {
    let mut ranking = candidates
        .iter()
        .map(|candidate| {
            let rows = scenes
                .iter()
                .filter_map(|scene| {
                    scene
                        .candidates
                        .iter()
                        .find(|row| row.candidate == *candidate)
                })
                .collect::<Vec<_>>();
            CandidateRanking {
                rank: 0,
                candidate: candidate.clone(),
                scene_count: rows.len() as u32,
                hard_gate_passes: rows.iter().filter(|row| row.hard_gate_pass).count() as u32,
                structural_passes: rows.iter().filter(|row| row.structural_pass).count() as u32,
                energy_envelope_passes: rows
                    .iter()
                    .filter(|row| row.summary.energy_envelope_nonincreasing)
                    .count() as u32,
                replay_exact_runs: rows.iter().filter(|row| row.replay_exact).count() as u32,
                capacity_clean_runs: rows.iter().filter(|row| row.capacity_clean).count() as u32,
                settled_churn: rows
                    .iter()
                    .map(|row| row.summary.stable_support_unexplained_churn)
                    .sum(),
                maximum_settled_linear_speed: rows
                    .iter()
                    .map(|row| row.summary.settled_linear_speed_max.get())
                    .fold(0.0, f32::max),
                maximum_settled_angular_edge_speed: rows
                    .iter()
                    .map(|row| row.summary.settled_angular_edge_speed_max.get())
                    .fold(0.0, f32::max),
                maximum_settled_penetration: rows
                    .iter()
                    .map(|row| row.physical_envelope.settled_penetration_max)
                    .fold(0.0, f32::max),
                ranking_contract: "hard gates, structure, energy, replay, capacity descending; maximum linear speed, angular-edge speed, penetration, settled churn, then candidate id ascending",
            }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(compare_rankings);
    for (index, row) in ranking.iter_mut().enumerate() {
        row.rank = index as u32 + 1;
    }
    ranking
}

fn compare_rankings(left: &CandidateRanking, right: &CandidateRanking) -> Ordering {
    right
        .hard_gate_passes
        .cmp(&left.hard_gate_passes)
        .then_with(|| right.structural_passes.cmp(&left.structural_passes))
        .then_with(|| {
            right
                .energy_envelope_passes
                .cmp(&left.energy_envelope_passes)
        })
        .then_with(|| right.replay_exact_runs.cmp(&left.replay_exact_runs))
        .then_with(|| right.capacity_clean_runs.cmp(&left.capacity_clean_runs))
        .then_with(|| {
            left.maximum_settled_linear_speed
                .total_cmp(&right.maximum_settled_linear_speed)
        })
        .then_with(|| {
            left.maximum_settled_angular_edge_speed
                .total_cmp(&right.maximum_settled_angular_edge_speed)
        })
        .then_with(|| {
            left.maximum_settled_penetration
                .total_cmp(&right.maximum_settled_penetration)
        })
        .then_with(|| left.settled_churn.cmp(&right.settled_churn))
        .then_with(|| left.candidate.id.cmp(&right.candidate.id))
}

fn randomized_evidence(
    runner: &GpuGeneratedContactRunner,
    candidate: &Candidate,
) -> Result<RandomizedEvidence, String> {
    let mut cases = Vec::new();
    for offset in 0..RANDOM_SEED_COUNT {
        let seed = RANDOM_FIRST_SEED + u64::from(offset);
        eprintln!("S6.4 randomized {} seed {seed}", candidate.id);
        let (kind, mut scenario) =
            p5_2_random_settling_fixture_scenario(seed, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display_error)?;
        scenario.settings.contact_softness = candidate.softness;
        let output = runner.run_identity_experiment(
            &scenario,
            P5_2_SETTLING_EVIDENCE_STEPS,
            candidate.identity_policy,
        )?;
        let replay = runner.run_identity_experiment(
            &scenario,
            P5_2_SETTLING_EVIDENCE_STEPS,
            candidate.identity_policy,
        )?;
        let result_hash = serialized_hash(&output)?;
        let replay_exact = result_hash == serialized_hash(&replay)?;
        let capacity_clean = output.failure.is_none()
            && output
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed);
        let (structural_pass, _) =
            settling_fixture_structure(kind, &scenario, &output.bodies, !capacity_clean, 0.0);
        let (linear, angular) = body_speeds(&output.bodies);
        cases.push(RandomizedCase {
            seed,
            kind: kind.name().to_owned(),
            candidate_scenario_hash: canonical_hash(&scenario).map_err(display_error)?,
            result_hash,
            replay_exact,
            capacity_clean,
            structural_pass,
            final_linear_speed_max: linear,
            final_angular_edge_speed_max: angular,
        });
    }
    Ok(RandomizedEvidence {
        candidate: candidate.clone(),
        first_seed: RANDOM_FIRST_SEED,
        seed_count: RANDOM_SEED_COUNT,
        exact_replays: cases.iter().filter(|case| case.replay_exact).count() as u32,
        capacity_clean_runs: cases.iter().filter(|case| case.capacity_clean).count() as u32,
        structural_passes: cases.iter().filter(|case| case.structural_pass).count() as u32,
        final_rest_passes: cases
            .iter()
            .filter(|case| {
                case.final_linear_speed_max <= 0.025 && case.final_angular_edge_speed_max <= 0.025
            })
            .count() as u32,
        cases,
    })
}

fn response_check(
    runner: &GpuGeneratedContactRunner,
    candidate: &Candidate,
) -> Result<ResponseCheck, String> {
    let cases = [
        (
            "bilateral-impact",
            P5_2GeneratedScenarioKind::BilateralCircles,
            1,
        ),
        (
            "off-center",
            P5_2GeneratedScenarioKind::OffCenterRectangles,
            4,
        ),
        ("friction", P5_2GeneratedScenarioKind::FrictionSlide, 12),
        ("restitution", P5_2GeneratedScenarioKind::Restitution, 8),
        ("short-stack", P5_2GeneratedScenarioKind::SmallStack, 12),
    ];
    let mut outputs = BTreeMap::new();
    let mut exact_replay = true;
    let mut capacity_clean = true;
    let mut result_hashes = BTreeMap::new();
    for (label, kind, steps) in cases {
        let mut scenario = p5_2_generated_scenario(kind, steps).map_err(display_error)?;
        scenario.settings.contact_softness = candidate.softness;
        let output = runner.run_identity_experiment(&scenario, steps, candidate.identity_policy)?;
        let replay = runner.run_identity_experiment(&scenario, steps, candidate.identity_policy)?;
        let hash = serialized_hash(&output)?;
        exact_replay &= hash == serialized_hash(&replay)?;
        capacity_clean &= output.failure.is_none()
            && output
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed);
        result_hashes.insert(label.to_owned(), hash);
        outputs.insert(label, output.bodies);
    }
    let bilateral = &outputs["bilateral-impact"];
    let off_center = &outputs["off-center"];
    let friction = &outputs["friction"];
    let restitution = &outputs["restitution"];
    let stack = &outputs["short-stack"];
    Ok(ResponseCheck {
        candidate: candidate.clone(),
        bilateral_impact_pass: bilateral[0].linear_velocity.x < 1.0
            && bilateral[1].linear_velocity.x > -1.0,
        off_center_angular_response_pass: off_center
            .iter()
            .any(|body| body.angular_velocity.get().abs() > 1.0e-3),
        friction_response_pass: friction[0].linear_velocity.x.abs() < 2.0,
        restitution_response_pass: restitution[0].linear_velocity.y > 0.0,
        short_stack_order_pass: stack[1..]
            .windows(2)
            .all(|pair| pair[0].position.y < pair[1].position.y),
        exact_replay,
        capacity_clean,
        result_hashes,
    })
}

fn timing_evidence(
    root: &Path,
    candidates: &[Candidate],
    samples: u32,
) -> Result<Vec<TimingEvidence>, String> {
    candidates
        .iter()
        .map(|candidate| {
            let mut scenario = p5_2_generated_performance_scenario(256).map_err(display_error)?;
            scenario.settings.contact_softness = candidate.softness;
            let raw = pollster::block_on(benchmark_gpu_generated_contact_identity_experiment(
                &scenario,
                5,
                samples,
                candidate.identity_policy,
            ))?;
            let identity =
                write_json_artifact(root, &format!("timing/{}.json", candidate.id), &raw)?;
            Ok(timing_summary(candidate, raw, identity))
        })
        .collect()
}

fn timing_summary(
    candidate: &Candidate,
    raw: GpuGeneratedContactBenchmarkOutput,
    identity: ArtifactIdentity,
) -> TimingEvidence {
    TimingEvidence {
        candidate: candidate.clone(),
        body_count: raw.body_count,
        samples: raw.samples,
        gpu_end_to_end_p50_seconds: raw.gpu_end_to_end_seconds.as_deref().map(median),
        host_completion_p50_seconds: median(&raw.host_completion_seconds),
        dispatches_per_step: raw.dispatches_per_step,
        allocated_bytes: raw.allocated_bytes,
        work_digest: serialized_hash(&raw.scheduler_work).expect("serializable work vector"),
        raw: identity,
    }
}

fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn structure(
    kind: SceneKind,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    capacity_failed: bool,
) -> (bool, String) {
    match kind {
        SceneKind::Stack => centered_stack_structure(source, bodies, capacity_failed, 0.0),
        SceneKind::Fixture(kind) => {
            settling_fixture_structure(kind, source, bodies, capacity_failed, 0.0)
        }
    }
}

fn body_speeds(bodies: &[BodyStateV1]) -> (f32, f32) {
    bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .fold((0.0_f32, 0.0_f32), |(linear, angular), body| {
            let speed = body.linear_velocity.x.hypot(body.linear_velocity.y);
            let radius = match body.shape {
                ShapeV1::Circle { radius } => radius.get(),
                ShapeV1::Rectangle { half_extents } => half_extents.x.hypot(half_extents.y),
            };
            (
                linear.max(speed),
                angular.max(body.angular_velocity.get().abs() * radius),
            )
        })
}

fn render_playback(scenes: &[PlaybackScene]) -> Result<String, String> {
    let selected = [
        "centered-stack-4",
        "centered-stack-8",
        "sequential-rectangles-5-seed-6002",
        "small-mixed-pile-seed-6002",
    ];
    let scenes = scenes
        .iter()
        .filter(|scene| selected.contains(&scene.label.as_str()))
        .collect::<Vec<_>>();
    let width = 2_030;
    let row_height = 210;
    let height = 70 + scenes.len() * row_height;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:10px;fill:#91a8ba}}.body{{fill:#54d6db;stroke:#d8ffff;stroke-width:1}}</style><text class=\"title\" x=\"18\" y=\"28\">S6.4 soft-contact synchronized playback</text><text class=\"meta\" x=\"18\" y=\"48\">rows: shared neutral source · columns: hard / identity-only / softness-only / combined · snapshots: 600, 900, 1200</text>"
    );
    for (row, scene) in scenes.iter().enumerate() {
        let top = 65 + row * row_height;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{}\">{}</text>",
            top + 16,
            scene.label
        ));
        for (column, (candidate, samples)) in scene.lanes.iter().enumerate() {
            let x = 260 + column * 435;
            svg.push_str(&format!(
                "<text class=\"meta\" x=\"{x}\" y=\"{}\">{}</text>",
                top + 16,
                candidate.id
            ));
            for (sample_index, (step, bodies)) in samples.iter().enumerate() {
                let sample_x = x + sample_index * 135;
                svg.push_str(&format!("<rect x=\"{sample_x}\" y=\"{}\" width=\"128\" height=\"170\" fill=\"#122739\" stroke=\"#29465b\"/><text class=\"meta\" x=\"{}\" y=\"{}\">{step}</text>", top + 26, sample_x + 4, top + 40));
                draw_bodies(&mut svg, &scene.source, bodies, sample_x + 64, top + 188)?;
            }
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

fn draw_bodies(
    svg: &mut String,
    source: &ScenarioV1,
    bodies: &[BodyStateV1],
    origin_x: usize,
    origin_y: usize,
) -> Result<(), String> {
    let dynamic = bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .collect::<Vec<_>>();
    let maximum_y = dynamic
        .iter()
        .map(|body| body.position.y)
        .fold(1.0_f32, f32::max);
    let scale = (135.0 / maximum_y.max(1.0)).clamp(8.0, 24.0);
    for body in dynamic {
        let source_body = source
            .bodies
            .iter()
            .find(|value| value.id == body.id)
            .ok_or_else(|| "playback body is absent from source".to_owned())?;
        let x = origin_x as f32 + body.position.x * scale;
        let y = origin_y as f32 - body.position.y * scale;
        match source_body.shape {
            ShapeV1::Circle { radius } => svg.push_str(&format!(
                "<circle class=\"body\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\"/>",
                radius.get() * scale
            )),
            ShapeV1::Rectangle { half_extents } => svg.push_str(&format!(
                "<rect class=\"body\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" transform=\"rotate({:.3} {x:.2} {y:.2})\"/>",
                x - half_extents.x * scale,
                y - half_extents.y * scale,
                half_extents.x * scale * 2.0,
                half_extents.y * scale * 2.0,
                -body.angle.get().to_degrees()
            )),
        }
    }
    Ok(())
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display_error)
}

fn write_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display_error)?;
    bytes.push(b'\n');
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(display_error)?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder.write_all(&bytes).map_err(display_error)?;
    write_artifact(root, relative, &encoder.finish().map_err(display_error)?)
}

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    atomic_write(&root.join(relative), bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
    })
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display_error)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = path.with_extension("partial");
    if path.exists() || staging.exists() {
        return Err(format!("refusing to replace artifact {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(display_error)?;
    file.write_all(bytes).map_err(display_error)?;
    file.sync_all().map_err(display_error)?;
    drop(file);
    fs::rename(staging, path).map_err(display_error)
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(display_error)
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    let status = command_output(
        repository,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let patch_digest = if dirty {
        let output = Command::new("git")
            .args(["diff", "--binary", "HEAD"])
            .current_dir(repository)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(display_error)?;
        if !output.status.success() {
            return Err("git diff failed while recording provenance".to_owned());
        }
        Some(sha256_bytes(&output.stdout))
    } else {
        None
    };
    Ok((revision, dirty, patch_digest))
}

fn command_output(directory: &Path, program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(display_error)?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(display_error)
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("soft-contact evidence constants are finite")
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    require_clean: bool,
    timing_samples: u32,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut require_clean = false;
        let mut timing_samples = 10;
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
                "--timing-samples" => {
                    timing_samples = value
                        .parse()
                        .map_err(|_| "invalid --timing-samples".to_owned())?
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        if timing_samples == 0 {
            return Err("timing samples must be positive".to_owned());
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            require_clean,
            timing_samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factorial_catalog_and_finalist_roles_are_frozen() {
        let candidates = screen_candidates();
        assert_eq!(candidates.len(), 9);
        assert_eq!(
            candidates
                .iter()
                .filter(|value| value.softness.is_some())
                .count(),
            8
        );
        assert_eq!(screen_scene_inputs().unwrap().len(), 5);
        assert_eq!(scene_inputs().unwrap().len(), 11);
        let finalists = finalists(&candidates[1]);
        assert_eq!(finalists.len(), 4);
        assert_eq!(finalists[0].role, "unchanged-hard-control");
        assert_eq!(finalists[1].role, "identity-only");
        assert_eq!(finalists[2].role, "softness-only");
        assert_eq!(finalists[3].role, "combined-identity-and-softness");
    }

    #[test]
    fn ranking_prioritizes_frozen_gates_before_residuals() {
        let candidate =
            |id: &str| hard_candidate(id, ContactIdentityPolicyV1::DirectMaximumSeparation, "test");
        let mut weak = CandidateRanking {
            rank: 0,
            candidate: candidate("weak"),
            scene_count: 5,
            hard_gate_passes: 4,
            structural_passes: 5,
            energy_envelope_passes: 5,
            replay_exact_runs: 5,
            capacity_clean_runs: 5,
            settled_churn: 0,
            maximum_settled_linear_speed: 0.0,
            maximum_settled_angular_edge_speed: 0.0,
            maximum_settled_penetration: 0.0,
            ranking_contract: "test",
        };
        let mut strong = weak.clone();
        strong.candidate = candidate("strong");
        strong.hard_gate_passes = 5;
        strong.maximum_settled_linear_speed = 100.0;
        assert_eq!(compare_rankings(&strong, &weak), Ordering::Less);
        weak.hard_gate_passes = 5;
        weak.maximum_settled_linear_speed = 1.0;
        weak.settled_churn = 100;
        assert_eq!(compare_rankings(&weak, &strong), Ordering::Less);
    }
}
