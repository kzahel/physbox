use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyStateV1, CandidatePairV1, ContactIdentityPolicyV1, SCHEMA_VERSION, ScenarioV1,
    SettlingObservationV2, SettlingSeriesV2, ShapeV1, WarmCacheStatusV2, canonical_hash,
    sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactBenchmarkOutput, GpuGeneratedContactRunner, P5_2_SETTLING_EVIDENCE_STEPS,
    P5_2SettlingFixtureKind, benchmark_gpu_generated_contact_identity_experiment,
    p5_2_box2d_world_projection, p5_2_centered_rectangle_stack_scenario,
    p5_2_generated_performance_scenario, p5_2_random_settling_fixture_scenario,
    p5_2_settling_fixture_scenario,
};
use physbox_lab::settling::{
    AWAKE_SETTLING_TRACE_VERSION, awake_settling_measurement_policy_v2, centered_stack_structure,
    observe_generated_settling_series_v2, settling_fixture_structure, settling_series_v2,
};
use serde::Serialize;

const BASELINE_MANIFEST: &str = "evidence/p5.2b-s6.2-settling-baseline/manifest.json";
const RANDOM_FIRST_SEED: u64 = 0x6300;
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
    selected_policy: ContactIdentityPolicyV1,
    baseline_manifest: ArtifactIdentity,
    baseline_source_revision: String,
    baseline_box2d_revision: String,
    policies: Vec<PolicyEvidence>,
    scenes: Vec<SceneEvidence>,
    randomized: Vec<RandomizedEvidence>,
    timing: Vec<TimingEvidence>,
    playback: ArtifactIdentity,
    comparator_decisions: ComparatorDecisions,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyEvidence {
    policy: ContactIdentityPolicyV1,
    selected: bool,
    scene_count: u32,
    hard_gate_passes: u32,
    settled_owner_changes: u64,
    settled_feature_changes: u64,
    settled_cache_misses: u64,
    settled_point_count_changes: u64,
    settled_normal_changes: u64,
    maximum_settled_linear_speed: f32,
    maximum_settled_angular_edge_speed: f32,
    energy_envelope_passes: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    candidates: Vec<CandidateEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEvidence {
    policy: ContactIdentityPolicyV1,
    raw_trace_hash: String,
    trace: ArtifactIdentity,
    replay_exact: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    hard_gate_pass: bool,
    hard_gate_failures: Vec<String>,
    chain: CausalChainEvidence,
    summary: physbox_contracts::SettlingSummaryV2,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct CausalChainEvidence {
    stable_reference_owner_changes: u64,
    stable_feature_set_changes: u64,
    owner_change_cold_points: u64,
    owner_change_cache_hits: u64,
    point_count_changes: u64,
    material_normal_changes: u64,
    maximum_pre_flip_accumulated_normal_impulse: f32,
    maximum_post_flip_accumulated_normal_impulse: f32,
    maximum_owner_change_step_linear_speed: f32,
    maximum_owner_change_step_angular_edge_speed: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedEvidence {
    policy: ContactIdentityPolicyV1,
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
    source_scenario_hash: String,
    result_hash: String,
    replay_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    final_linear_speed_max: f32,
    final_angular_edge_speed_max: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingEvidence {
    policy: ContactIdentityPolicyV1,
    body_count: u32,
    samples: u32,
    gpu_end_to_end_p50_seconds: Option<f64>,
    host_completion_p50_seconds: f64,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    work_digest: String,
    raw: ArtifactIdentity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparatorDecisions {
    relative_coherence_bias: &'static str,
    canonical_absolute_band: &'static str,
    true_prior_contact_hysteresis: &'static str,
    canonical_impulse_correspondence: &'static str,
    rationale: &'static str,
    reconsideration_trigger: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

type PlaybackSamples = Vec<(u32, Vec<BodyStateV1>)>;
type PlaybackLane = (ContactIdentityPolicyV1, PlaybackSamples);

struct PlaybackScene {
    label: String,
    source: ScenarioV1,
    lanes: Vec<PlaybackLane>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-contact-identity-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S6.3 identity evidence requires a clean source tree".to_owned());
    }
    let baseline_path = repository.join(BASELINE_MANIFEST);
    let baseline_bytes = fs::read(&baseline_path).map_err(|error| error.to_string())?;
    let baseline: serde_json::Value =
        serde_json::from_slice(&baseline_bytes).map_err(|error| error.to_string())?;
    if baseline["artifactKind"] != "p5.2b-s6.2-awake-settling-baseline"
        || baseline["sourceDirty"] != false
        || baseline["candidateResultsPresent"] != false
    {
        return Err("S6.2 baseline manifest is not the frozen clean control".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    for directory in ["sources", "traces", "timing"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenes = Vec::new();
    let mut playback = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S6.3 identity {}", input.label);
        let (scene, playback_scene) = evaluate_scene(&staging, &runner, input)?;
        scenes.push(scene);
        playback.push(playback_scene);
    }
    let policies = summarize_policies(&scenes);
    let randomized = ContactIdentityPolicyV1::ALL
        .into_iter()
        .map(|policy| randomized_evidence(&runner, policy))
        .collect::<Result<Vec<_>, _>>()?;
    let timing = timing_evidence(&staging, arguments.timing_samples)?;
    let playback = write_artifact(
        &staging,
        "playback.svg",
        render_playback(&playback)?.as_bytes(),
    )?;
    let baseline_manifest = ArtifactIdentity {
        path: BASELINE_MANIFEST.to_owned(),
        sha256: sha256_bytes(&baseline_bytes),
    };
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        observation_version: AWAKE_SETTLING_TRACE_VERSION,
        artifact_kind: "p5.2b-s6.3-contact-identity",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        adapter: runner.diagnostics().clone(),
        selected_policy: ContactIdentityPolicyV1::SELECTED,
        baseline_manifest,
        baseline_source_revision: required_string(&baseline, "sourceRevision")?,
        baseline_box2d_revision: required_string(&baseline, "box2dRevision")?,
        policies,
        scenes,
        randomized,
        timing,
        playback,
        comparator_decisions: ComparatorDecisions {
            relative_coherence_bias: "rejected-deep-stack-instability",
            canonical_absolute_band: "rejected-deep-stack-instability",
            true_prior_contact_hysteresis: "forward-for-s6.4-combination",
            canonical_impulse_correspondence: "rejected-without-safe-geometry-proof",
            rationale: "The stateless coherence candidates remove churn but preserve the wrong near-tie owner long enough to destabilize deep stacks. Generational prior-owner hysteresis preserves the first direct answer and changes geometry only within a bounded band, identical point count, and compatible normal. It removes churn but deliberately leaves the independent damping failure for S6.4.",
            reconsideration_trigger: "A capacity-clean source that churns under temporal hysteresis, a normal/point-count gate that prevents needed coherence, or S6.4 evidence showing that temporal identity does not compose safely with contact softness.",
        },
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s6.3-contact-identity run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [2, 4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            kind: SceneKind::Stack,
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
        });
    }
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            kind: SceneKind::Fixture(kind),
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?,
        });
    }
    Ok(inputs)
}

fn evaluate_scene(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    input: SceneInput,
) -> Result<(SceneEvidence, PlaybackScene), String> {
    let source_scenario_hash = canonical_hash(&input.source).map_err(|error| error.to_string())?;
    let projection =
        p5_2_box2d_world_projection(&input.source).map_err(|error| error.to_string())?;
    let projection_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let source = write_json_artifact(
        staging,
        &format!("sources/{}.json", input.label),
        &input.source,
    )?;
    let mut candidates = Vec::new();
    let mut lanes = Vec::new();
    for identity_policy in ContactIdentityPolicyV1::ALL {
        let raw = runner.run_observation_series_with_identity_policy(
            &input.source,
            1,
            P5_2_SETTLING_EVIDENCE_STEPS,
            identity_policy,
        )?;
        let raw_trace_hash = serialized_hash(&raw)?;
        let replay = runner.run_observation_series_with_identity_policy(
            &input.source,
            1,
            P5_2_SETTLING_EVIDENCE_STEPS,
            identity_policy,
        )?;
        let replay_exact = raw_trace_hash == serialized_hash(&replay)?;
        drop(replay);
        let final_only = runner.run_identity_experiment(
            &input.source,
            P5_2_SETTLING_EVIDENCE_STEPS,
            identity_policy,
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
        let observations = observe_generated_settling_series_v2(&input.source, &raw)?;
        let chain = causal_chain(&observations);
        let trace = settling_series_v2(
            &input.source,
            Some(projection_hash.clone()),
            identity_policy.name(),
            awake_settling_measurement_policy_v2(),
            observations,
            structural_pass,
            structural_reason.clone(),
        )?;
        let trace_identity = write_gzip_json_artifact(
            staging,
            &format!("traces/{}-{}.json.gz", input.label, identity_policy.name()),
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
        lanes.push((
            identity_policy,
            raw.snapshots
                .iter()
                .filter(|snapshot| PLAYBACK_STEPS.contains(&snapshot.completed_steps))
                .map(|snapshot| (snapshot.completed_steps, snapshot.bodies.clone()))
                .collect(),
        ));
        candidates.push(CandidateEvidence {
            policy: identity_policy,
            raw_trace_hash,
            trace: trace_identity,
            replay_exact,
            instrumentation_final_exact,
            capacity_clean,
            structural_pass,
            structural_reason,
            hard_gate_pass: hard_gate_failures.is_empty(),
            hard_gate_failures,
            chain,
            summary: trace.summary,
        });
    }
    Ok((
        SceneEvidence {
            label: input.label.clone(),
            source_scenario_hash,
            projection_hash,
            source,
            candidates,
        },
        PlaybackScene {
            label: input.label,
            source: input.source,
            lanes,
        },
    ))
}

fn causal_chain(observations: &[SettlingObservationV2]) -> CausalChainEvidence {
    let mut chain = CausalChainEvidence::default();
    let mut previous_impulses = BTreeMap::<CandidatePairV1, f32>::new();
    for observation in observations {
        let settled = observation.base.completed_steps >= 900;
        for contact in &observation.contacts {
            if settled && contact.stable_support {
                chain.stable_reference_owner_changes += u64::from(contact.reference_owner_changed);
                chain.stable_feature_set_changes += u64::from(contact.feature_set_changed);
                chain.point_count_changes += u64::from(contact.point_count_changed);
                chain.material_normal_changes += u64::from(contact.normal_changed);
                if contact.reference_owner_changed {
                    chain.maximum_pre_flip_accumulated_normal_impulse = chain
                        .maximum_pre_flip_accumulated_normal_impulse
                        .max(previous_impulses.get(&contact.pair).copied().unwrap_or(0.0));
                    chain.maximum_owner_change_step_linear_speed = chain
                        .maximum_owner_change_step_linear_speed
                        .max(observation.base.residual_linear_speed_max.get());
                    chain.maximum_owner_change_step_angular_edge_speed = chain
                        .maximum_owner_change_step_angular_edge_speed
                        .max(observation.angular_edge_speed_max.get());
                    for point in &contact.cache {
                        chain.owner_change_cold_points += u64::from(matches!(
                            point.status,
                            WarmCacheStatusV2::ReferenceOwnerChanged
                                | WarmCacheStatusV2::FeatureChanged
                        ));
                        chain.owner_change_cache_hits +=
                            u64::from(matches!(point.status, WarmCacheStatusV2::Hit));
                        chain.maximum_post_flip_accumulated_normal_impulse =
                            chain.maximum_post_flip_accumulated_normal_impulse.max(
                                point
                                    .current_normal_impulse
                                    .map_or(0.0, |value| value.get()),
                            );
                    }
                }
            }
            previous_impulses.insert(
                contact.pair,
                contact
                    .cache
                    .iter()
                    .map(|point| {
                        point
                            .current_normal_impulse
                            .map_or(0.0, |value| value.get())
                    })
                    .sum(),
            );
        }
    }
    chain
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

fn summarize_policies(scenes: &[SceneEvidence]) -> Vec<PolicyEvidence> {
    ContactIdentityPolicyV1::ALL
        .into_iter()
        .map(|policy| {
            let candidates = scenes
                .iter()
                .map(|scene| {
                    scene
                        .candidates
                        .iter()
                        .find(|candidate| candidate.policy == policy)
                        .expect("complete policy matrix")
                })
                .collect::<Vec<_>>();
            PolicyEvidence {
                policy,
                selected: policy == ContactIdentityPolicyV1::SELECTED,
                scene_count: candidates.len() as u32,
                hard_gate_passes: candidates
                    .iter()
                    .filter(|candidate| candidate.hard_gate_pass)
                    .count() as u32,
                settled_owner_changes: candidates
                    .iter()
                    .map(|candidate| candidate.chain.stable_reference_owner_changes)
                    .sum(),
                settled_feature_changes: candidates
                    .iter()
                    .map(|candidate| candidate.chain.stable_feature_set_changes)
                    .sum(),
                settled_cache_misses: candidates
                    .iter()
                    .map(|candidate| candidate.summary.warm_cache_misses)
                    .sum(),
                settled_point_count_changes: candidates
                    .iter()
                    .map(|candidate| candidate.chain.point_count_changes)
                    .sum(),
                settled_normal_changes: candidates
                    .iter()
                    .map(|candidate| candidate.chain.material_normal_changes)
                    .sum(),
                maximum_settled_linear_speed: candidates
                    .iter()
                    .map(|candidate| candidate.summary.settled_linear_speed_max.get())
                    .fold(0.0, f32::max),
                maximum_settled_angular_edge_speed: candidates
                    .iter()
                    .map(|candidate| candidate.summary.settled_angular_edge_speed_max.get())
                    .fold(0.0, f32::max),
                energy_envelope_passes: candidates
                    .iter()
                    .filter(|candidate| candidate.summary.energy_envelope_nonincreasing)
                    .count() as u32,
            }
        })
        .collect()
}

fn randomized_evidence(
    runner: &GpuGeneratedContactRunner,
    policy: ContactIdentityPolicyV1,
) -> Result<RandomizedEvidence, String> {
    let mut cases = Vec::new();
    for offset in 0..RANDOM_SEED_COUNT {
        let seed = RANDOM_FIRST_SEED + u64::from(offset);
        let (kind, scenario) =
            p5_2_random_settling_fixture_scenario(seed, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(|error| error.to_string())?;
        let output =
            runner.run_identity_experiment(&scenario, P5_2_SETTLING_EVIDENCE_STEPS, policy)?;
        let replay =
            runner.run_identity_experiment(&scenario, P5_2_SETTLING_EVIDENCE_STEPS, policy)?;
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
            source_scenario_hash: canonical_hash(&scenario).map_err(|error| error.to_string())?,
            result_hash,
            replay_exact,
            capacity_clean,
            structural_pass,
            final_linear_speed_max: linear,
            final_angular_edge_speed_max: angular,
        });
    }
    Ok(RandomizedEvidence {
        policy,
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

fn timing_evidence(root: &Path, samples: u32) -> Result<Vec<TimingEvidence>, String> {
    let scenario = p5_2_generated_performance_scenario(256).map_err(|error| error.to_string())?;
    ContactIdentityPolicyV1::ALL
        .into_iter()
        .map(|policy| {
            let raw = pollster::block_on(benchmark_gpu_generated_contact_identity_experiment(
                &scenario, 5, samples, policy,
            ))?;
            let identity =
                write_json_artifact(root, &format!("timing/{}.json", policy.name()), &raw)?;
            Ok(timing_summary(policy, raw, identity))
        })
        .collect()
}

fn timing_summary(
    policy: ContactIdentityPolicyV1,
    raw: GpuGeneratedContactBenchmarkOutput,
    identity: ArtifactIdentity,
) -> TimingEvidence {
    let gpu = raw.gpu_end_to_end_seconds.as_deref().map(median);
    TimingEvidence {
        policy,
        body_count: raw.body_count,
        samples: raw.samples,
        gpu_end_to_end_p50_seconds: gpu,
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
    let width = 1_650;
    let row_height = 210;
    let height = 70 + scenes.len() * row_height;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/><style>text{{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}}.title{{font:700 18px system-ui}}.meta{{font-size:11px;fill:#91a8ba}}.body{{fill:#54d6db;stroke:#d8ffff;stroke-width:1}}</style><text class=\"title\" x=\"18\" y=\"28\">S6.3 contact identity synchronized playback</text><text class=\"meta\" x=\"18\" y=\"48\">rows: shared neutral source · columns: direct / relative bias / canonical band · snapshots: 600, 900, 1200</text>"
    );
    for (row, scene) in scenes.iter().enumerate() {
        let top = 65 + row * row_height;
        svg.push_str(&format!(
            "<text x=\"18\" y=\"{}\">{}</text>",
            top + 16,
            scene.label
        ));
        for (column, (policy, samples)) in scene.lanes.iter().enumerate() {
            let x = 260 + column * 340;
            svg.push_str(&format!(
                "<text class=\"meta\" x=\"{x}\" y=\"{}\">{}</text>",
                top + 16,
                policy.name()
            ));
            for (sample_index, (step, bodies)) in samples.iter().enumerate() {
                let sample_x = x + sample_index * 105;
                svg.push_str(&format!("<rect x=\"{sample_x}\" y=\"{}\" width=\"98\" height=\"170\" fill=\"#122739\" stroke=\"#29465b\"/><text class=\"meta\" x=\"{}\" y=\"{}\">{step}</text>", top + 26, sample_x + 4, top + 40));
                draw_bodies(&mut svg, &scene.source, bodies, sample_x + 49, top + 188)?;
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
        .map_err(|error| error.to_string())
}

fn write_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    write_artifact(
        root,
        relative,
        &encoder.finish().map_err(|error| error.to_string())?,
    )
}

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    atomic_write(&root.join(relative), bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
    })
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(staging, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn required_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("baseline manifest omitted {key}"))
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
            .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
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
    fn identity_catalog_is_the_frozen_s6_2_matrix() {
        assert_eq!(scene_inputs().unwrap().len(), 11);
        assert_eq!(
            ContactIdentityPolicyV1::SELECTED.name(),
            "direct-maximum-separation"
        );
        assert_eq!(PLAYBACK_STEPS.last(), Some(&1_200));
    }
}
