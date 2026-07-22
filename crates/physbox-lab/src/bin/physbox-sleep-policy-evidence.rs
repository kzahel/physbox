use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use physbox_contracts::{
    BodyId, BodyStateV1, FiniteF32, NeutralActionV1, OracleRequestV1, SCHEMA_VERSION, ShapeV1,
    SleepPolicyV1, TraceRecordV1, TraceStageV1, Validate, Vec2, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    AdapterDiagnostics, AnalyticFixtureKind, GpuGeneratedContactRunner, SolverProfileConfig,
    analytic_fixture_scenario, p5_2_centered_rectangle_stack_scenario,
    p5_2_rectangle_pyramid_scenario, provisional_grab_tuning,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, run_oracle, verify_clean_git_pin,
};
use physbox_launch::PhysicsConfigurationV2;
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 1;
const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const EVIDENCE_SECONDS: u32 = 5;

#[derive(Clone, Copy)]
struct MatrixConfiguration {
    fixed_step_hz: u32,
    substeps: u32,
}

impl MatrixConfiguration {
    const FULL: [Self; 5] = [
        Self {
            fixed_step_hz: 30,
            substeps: 4,
        },
        Self {
            fixed_step_hz: 60,
            substeps: 4,
        },
        Self {
            fixed_step_hz: 120,
            substeps: 4,
        },
        Self {
            fixed_step_hz: 60,
            substeps: 1,
        },
        Self {
            fixed_step_hz: 60,
            substeps: 8,
        },
    ];

    const QUICK: [Self; 1] = [Self {
        fixed_step_hz: 60,
        substeps: 4,
    }];

    fn id(self) -> String {
        format!("{}hz-{}substeps", self.fixed_step_hz, self.substeps)
    }
}

#[derive(Clone, Copy)]
enum FixtureKind {
    IsolatedCircle,
    IsolatedRectangle,
    Stack,
    Pyramid,
    IndependentStructures,
    AppliedImpulse,
    ProjectileImpact,
    Grab,
}

impl FixtureKind {
    const FULL: [Self; 8] = [
        Self::IsolatedCircle,
        Self::IsolatedRectangle,
        Self::Stack,
        Self::Pyramid,
        Self::IndependentStructures,
        Self::AppliedImpulse,
        Self::ProjectileImpact,
        Self::Grab,
    ];

    const QUICK: [Self; 2] = [Self::IsolatedCircle, Self::AppliedImpulse];

    const fn id(self) -> &'static str {
        match self {
            Self::IsolatedCircle => "isolated-circle-rest",
            Self::IsolatedRectangle => "isolated-rectangle-rest",
            Self::Stack => "rectangle-stack-4",
            Self::Pyramid => "rectangle-pyramid-3",
            Self::IndependentStructures => "two-independent-stacks",
            Self::AppliedImpulse => "applied-impulse-after-rest",
            Self::ProjectileImpact => "projectile-impact-after-rest",
            Self::Grab => "grab-begin-hold-release-after-rest",
        }
    }

    const fn stimulus(self) -> &'static str {
        match self {
            Self::AppliedImpulse => "public apply-linear-impulse action",
            Self::ProjectileImpact => "new projectile contact after public impulse action",
            Self::Grab => "public motor-joint grab begin/hold/release mapping",
            _ => "gravity/contact settling only",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepPolicyEvidenceV1 {
    schema_version: u32,
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    host: String,
    box2d_revision: &'static str,
    policy: SleepPolicyV1,
    policy_status: &'static str,
    authority_behavior: AuthorityBehaviorV1,
    physbox_behavior: PhysBoxBehaviorV1,
    adapter: AdapterDiagnostics,
    rows: Vec<EvidenceRowV1>,
    gpu_baseline_comparison: Option<GpuBaselineComparisonV1>,
    all_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuBaselineComparisonV1 {
    baseline_artifact: String,
    baseline_source_revision: String,
    compared_histories: u32,
    snapshot_histories_exact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityBehaviorV1 {
    quiet_metric: &'static str,
    default_speed_threshold_meters_per_second: f32,
    default_quiet_duration_milliseconds: u32,
    admission_grouping: &'static str,
    awake_observation: &'static str,
    transition_cause_status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysBoxBehaviorV1 {
    quiet_metric: &'static str,
    admission_grouping: &'static str,
    transition_velocity: &'static str,
    timer: &'static str,
    effective_sleeping: bool,
    action_history_limitation: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceRowV1 {
    fixture: &'static str,
    stimulus: &'static str,
    fixed_step_hz: u32,
    substeps: u32,
    requested_steps: u32,
    source_scenario_hash: String,
    oracle: OracleSummaryV1,
    physbox_sleep_off: PhysBoxSummaryV1,
    accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OracleSummaryV1 {
    artifact: String,
    replay_artifact: String,
    trace_hash: String,
    exact_replay: bool,
    first_sleep: Vec<TransitionObservationV1>,
    wake_transitions: Vec<WakeObservationV1>,
    awake_counts: Vec<u32>,
    awake_counts_hash: String,
    whole_structure_first_sleep_step: Option<u32>,
    all_sleep_transition_velocities_zero: bool,
    final_dynamic_bodies_sleeping: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysBoxSummaryV1 {
    supported: bool,
    limitation: Option<&'static str>,
    artifact: Option<String>,
    trace_hash: Option<String>,
    exact_replay: Option<bool>,
    first_policy_candidate: Vec<TransitionObservationV1>,
    awake_counts: Vec<u32>,
    awake_counts_hash: Option<String>,
    maximum_farthest_point_speed: Option<f32>,
    all_body_sleep_flags_clear: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransitionObservationV1 {
    body: BodyId,
    completed_step: u32,
    farthest_point_speed: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WakeObservationV1 {
    body: BodyId,
    completed_step: u32,
    cause_status: &'static str,
    inferred_stimulus: &'static str,
}

struct Arguments {
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    run_id: String,
    allow_dirty: bool,
    quick: bool,
    compare_gpu_baseline: Option<PathBuf>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-sleep-policy-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    if arguments.output.exists() {
        return Err("sleep-policy evidence output must be fresh".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if staging.exists() {
        return Err("sleep-policy evidence staging path must be fresh".to_owned());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_revision = git(&repository, &["rev-parse", "HEAD"])?;
    let dirty_bytes = git_bytes(
        &repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let source_dirty = !dirty_bytes.is_empty();
    if source_dirty && !arguments.allow_dirty {
        return Err("sleep-policy evidence requires a clean source tree".to_owned());
    }
    let source_patch_digest = source_dirty.then(|| sha256_bytes(&dirty_bytes));
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    fs::create_dir(&staging).map_err(display)?;
    fs::create_dir(staging.join("oracle")).map_err(display)?;
    fs::create_dir(staging.join("gpu")).map_err(display)?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let adapter = runner.diagnostics().clone();
    let policy = SleepPolicyV1::experimental_s23();
    policy.validate().map_err(|error| error.to_string())?;
    let configurations: &[MatrixConfiguration] = if arguments.quick {
        &MatrixConfiguration::QUICK
    } else {
        &MatrixConfiguration::FULL
    };
    let fixtures: &[FixtureKind] = if arguments.quick {
        &FixtureKind::QUICK
    } else {
        &FixtureKind::FULL
    };
    let mut rows = Vec::with_capacity(configurations.len() * fixtures.len());
    for &configuration in configurations {
        for &fixture in fixtures {
            rows.push(run_row(
                &arguments,
                &staging,
                &box2d,
                &runner,
                policy,
                fixture,
                configuration,
            )?);
        }
    }
    let gpu_baseline_comparison = arguments
        .compare_gpu_baseline
        .as_ref()
        .map(|baseline| compare_gpu_baseline(baseline, &staging))
        .transpose()?;
    let all_accepted = rows.iter().all(|row| row.accepted)
        && gpu_baseline_comparison
            .as_ref()
            .is_none_or(|comparison| comparison.snapshot_histories_exact);
    if !all_accepted {
        return Err("sleep-policy evidence contained a rejected row".to_owned());
    }
    let manifest = SleepPolicyEvidenceV1 {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "s23-sleep-policy-and-sleep-off-baseline",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        host: format!("{}-{}", env::consts::OS, env::consts::ARCH),
        box2d_revision: BOX2D_REVISION,
        policy,
        policy_status: "experimental opt-in; automatic admission is effective and the default remains off",
        authority_behavior: AuthorityBehaviorV1 {
            quiet_metric: "public velocity trace overlaid with |v| + |omega| * radial extent",
            default_speed_threshold_meters_per_second: 0.05,
            default_quiet_duration_milliseconds: 500,
            admission_grouping: "whole Box2D island",
            awake_observation: "public b2Body_IsAwake after every completed outer step",
            transition_cause_status: "Box2D public API exposes awake state, not an internal cause; stimuli are inferred and labeled",
        },
        physbox_behavior: PhysBoxBehaviorV1 {
            quiet_metric: "farthest-point speed |v| + |omega| * radial extent",
            admission_grouping: "per body with later bounded contact-neighbor wake propagation",
            transition_velocity: "zero linear and angular velocity",
            timer: "integer completed outer fixed steps; ceil(500 ms * fixed Hz)",
            effective_sleeping: false,
            action_history_limitation: "the bounded generated-contact series does not consume neutral actions; action rows are explicitly unsupported instead of projected differently",
        },
        adapter,
        rows,
        gpu_baseline_comparison,
        all_accepted,
    };
    write_new(&staging.join("manifest.json"), &json_bytes(&manifest)?)?;
    fs::rename(&staging, &arguments.output).map_err(display)?;
    println!("PHYSBOX5_SLEEP_POLICY_EVIDENCE_SUCCESS");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_row(
    arguments: &Arguments,
    staging: &Path,
    box2d: &physbox_lab::oracle_transport::OracleSourceIdentity,
    runner: &GpuGeneratedContactRunner,
    policy: SleepPolicyV1,
    fixture: FixtureKind,
    configuration: MatrixConfiguration,
) -> Result<EvidenceRowV1, String> {
    let scenario = fixture_scenario(fixture, configuration)?;
    let source_scenario_hash = canonical_hash(&scenario).map_err(display)?;
    let row_id = format!("{}-{}", fixture.id(), configuration.id());
    let oracle_config = || OracleRunConfig {
        artifact_root: staging.join("oracle"),
        timeout: Duration::from_secs(30),
        source: box2d.clone(),
        process: OracleProcess {
            program: arguments.oracle.clone(),
            arguments: vec![
                "--lane".into(),
                "world".into(),
                "--warm-start".into(),
                "on".into(),
                "--capture-step-trace".into(),
                "on".into(),
            ],
        },
    };
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-{}-oracle", arguments.run_id, row_id),
        source_scenario_hash: source_scenario_hash.clone(),
        scenario: scenario.clone(),
    };
    let replay_request = OracleRequestV1 {
        run_id: format!("{}-{}-replay", arguments.run_id, row_id),
        ..request.clone()
    };
    let first_config = oracle_config();
    let first = run_oracle(&first_config, &request).map_err(|error| error.to_string())?;
    let replay_config = oracle_config();
    let replay = run_oracle(&replay_config, &replay_request).map_err(|error| error.to_string())?;
    let exact_replay = first.result.bodies == replay.result.bodies
        && first.result.manifolds == replay.result.manifolds
        && first.result.traces == replay.result.traces;
    let oracle = summarize_oracle(
        fixture,
        &scenario,
        &first.result.traces,
        exact_replay,
        format!("oracle/{}/oracle-result.json", request.run_id),
        format!("oracle/{}/oracle-result.json", replay_request.run_id),
        first
            .result
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0 && body.sleeping)
            .count() as u32,
    )?;
    let physbox_sleep_off = if scenario.actions.is_empty() {
        run_physbox_baseline(staging, runner, policy, &scenario, &row_id)?
    } else {
        PhysBoxSummaryV1 {
            supported: false,
            limitation: Some(
                "neutral action execution is not part of the generated-contact history runner",
            ),
            artifact: None,
            trace_hash: None,
            exact_replay: None,
            first_policy_candidate: Vec::new(),
            awake_counts: Vec::new(),
            awake_counts_hash: None,
            maximum_farthest_point_speed: None,
            all_body_sleep_flags_clear: None,
        }
    };
    let accepted = exact_replay
        && !oracle.first_sleep.is_empty()
        && (physbox_sleep_off.supported
            && physbox_sleep_off.exact_replay == Some(true)
            && physbox_sleep_off.all_body_sleep_flags_clear == Some(true)
            || !physbox_sleep_off.supported && physbox_sleep_off.limitation.is_some());
    Ok(EvidenceRowV1 {
        fixture: fixture.id(),
        stimulus: fixture.stimulus(),
        fixed_step_hz: configuration.fixed_step_hz,
        substeps: configuration.substeps,
        requested_steps: scenario.requested_steps,
        source_scenario_hash,
        oracle,
        physbox_sleep_off,
        accepted,
    })
}

fn summarize_oracle(
    fixture: FixtureKind,
    scenario: &physbox_contracts::ScenarioV1,
    traces: &[TraceRecordV1],
    exact_replay: bool,
    artifact: String,
    replay_artifact: String,
    final_dynamic_bodies_sleeping: u32,
) -> Result<OracleSummaryV1, String> {
    let dynamic = scenario
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .map(|body| body.id)
        .collect::<Vec<_>>();
    let mut by_body = BTreeMap::<BodyId, Vec<&TraceRecordV1>>::new();
    for trace in traces
        .iter()
        .filter(|trace| trace.stage == TraceStageV1::Integrate)
    {
        by_body
            .entry(BodyId {
                slot: trace.manifold_id,
                generation: trace.point_index,
            })
            .or_default()
            .push(trace);
    }
    let mut first_sleep = Vec::new();
    let mut wake_transitions = Vec::new();
    let mut all_sleep_transition_velocities_zero = true;
    for body in &dynamic {
        let records = by_body
            .get(body)
            .ok_or_else(|| format!("oracle trace omitted dynamic body {body:?}"))?;
        let mut previous_awake = true;
        for trace in records {
            let awake = trace_value(trace, "awake")? != 0.0;
            if previous_awake && !awake {
                let speed = trace_value(trace, "farthest_point_speed")?;
                all_sleep_transition_velocities_zero &= speed.to_bits() == 0;
                if !first_sleep
                    .iter()
                    .any(|entry: &TransitionObservationV1| entry.body == *body)
                {
                    first_sleep.push(TransitionObservationV1 {
                        body: *body,
                        completed_step: trace.step,
                        farthest_point_speed: speed,
                    });
                }
            } else if !previous_awake && awake {
                wake_transitions.push(WakeObservationV1 {
                    body: *body,
                    completed_step: trace.step,
                    cause_status: "inferred from governed stimulus; not exposed by public Box2D",
                    inferred_stimulus: fixture.stimulus(),
                });
            }
            previous_awake = awake;
        }
    }
    first_sleep.sort_by_key(|entry| (entry.completed_step, entry.body));
    wake_transitions.sort_by_key(|entry| (entry.completed_step, entry.body));
    let awake_counts = (1..=scenario.requested_steps)
        .map(|step| {
            dynamic
                .iter()
                .filter(|body| {
                    by_body
                        .get(body)
                        .and_then(|records| records.iter().find(|trace| trace.step == step))
                        .and_then(|trace| trace_value(trace, "awake").ok())
                        == Some(1.0)
                })
                .count() as u32
        })
        .collect::<Vec<_>>();
    let first_steps = first_sleep
        .iter()
        .map(|entry| entry.completed_step)
        .collect::<Vec<_>>();
    let whole_structure_first_sleep_step = (!first_steps.is_empty()
        && first_steps.len() == dynamic.len()
        && first_steps.iter().all(|step| *step == first_steps[0]))
    .then_some(first_steps[0]);
    Ok(OracleSummaryV1 {
        artifact,
        replay_artifact,
        trace_hash: json_hash(&traces)?,
        exact_replay,
        first_sleep,
        wake_transitions,
        awake_counts_hash: json_hash(&awake_counts)?,
        awake_counts,
        whole_structure_first_sleep_step,
        all_sleep_transition_velocities_zero,
        final_dynamic_bodies_sleeping,
    })
}

fn run_physbox_baseline(
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    policy: SleepPolicyV1,
    scenario: &physbox_contracts::ScenarioV1,
    row_id: &str,
) -> Result<PhysBoxSummaryV1, String> {
    let first = runner.run_branch_robust_tunable_observation_series_with_submission_batch(
        scenario,
        1,
        scenario.requested_steps,
        8,
    )?;
    let replay = runner.run_branch_robust_tunable_observation_series_with_submission_batch(
        scenario,
        1,
        scenario.requested_steps,
        8,
    )?;
    first.validate()?;
    replay.validate()?;
    let first_bytes = json_bytes(&first)?;
    let replay_bytes = json_bytes(&replay)?;
    let exact_replay = first_bytes == replay_bytes;
    let artifact_name = format!("gpu/{row_id}.json");
    write_new(&staging.join(&artifact_name), &first_bytes)?;
    let dynamic = scenario
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .map(|body| body.id)
        .collect::<Vec<_>>();
    let quiet_steps = policy
        .quiet_outer_steps((1.0 / scenario.settings.fixed_dt.get()).round() as u32)
        .map_err(display)?;
    let mut quiet_counts = BTreeMap::<BodyId, u32>::new();
    let mut first_policy_candidate = Vec::new();
    let mut maximum_farthest_point_speed = 0.0_f32;
    for snapshot in &first.snapshots {
        for body in snapshot
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0)
        {
            let speed = farthest_point_speed(body);
            maximum_farthest_point_speed = maximum_farthest_point_speed.max(speed);
            let count = quiet_counts.entry(body.id).or_default();
            if speed <= policy.speed_threshold.get() {
                *count = count.saturating_add(1);
            } else {
                *count = 0;
            }
            if *count == quiet_steps {
                first_policy_candidate.push(TransitionObservationV1 {
                    body: body.id,
                    completed_step: snapshot.completed_steps,
                    farthest_point_speed: speed,
                });
            }
        }
    }
    first_policy_candidate.sort_by_key(|entry| (entry.completed_step, entry.body));
    let awake_counts = vec![dynamic.len() as u32; scenario.requested_steps as usize];
    let all_body_sleep_flags_clear = first
        .snapshots
        .iter()
        .flat_map(|snapshot| &snapshot.bodies)
        .all(|body| !body.sleeping);
    Ok(PhysBoxSummaryV1 {
        supported: true,
        limitation: None,
        artifact: Some(artifact_name),
        trace_hash: Some(sha256_bytes(&first_bytes)),
        exact_replay: Some(exact_replay),
        first_policy_candidate,
        awake_counts_hash: Some(json_hash(&awake_counts)?),
        awake_counts,
        maximum_farthest_point_speed: Some(maximum_farthest_point_speed),
        all_body_sleep_flags_clear: Some(all_body_sleep_flags_clear),
    })
}

fn fixture_scenario(
    fixture: FixtureKind,
    configuration: MatrixConfiguration,
) -> Result<physbox_contracts::ScenarioV1, String> {
    let steps = configuration
        .fixed_step_hz
        .checked_mul(EVIDENCE_SECONDS)
        .ok_or_else(|| "sleep fixture step count overflowed".to_owned())?;
    let mut scenario = match fixture {
        FixtureKind::IsolatedCircle => {
            analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, steps)
        }
        FixtureKind::IsolatedRectangle | FixtureKind::AppliedImpulse | FixtureKind::Grab => {
            analytic_fixture_scenario(AnalyticFixtureKind::RectangleFloor, steps)
        }
        FixtureKind::Stack | FixtureKind::ProjectileImpact => {
            p5_2_centered_rectangle_stack_scenario(4, steps)
        }
        FixtureKind::Pyramid => p5_2_rectangle_pyramid_scenario(3, steps),
        FixtureKind::IndependentStructures => two_independent_structures(steps),
    }
    .map_err(display)?;
    scenario.id = format!("s23-{}-{}", fixture.id(), configuration.id());
    scenario.manifolds.clear();
    scenario.contact_cache.clear();
    scenario.actions.clear();
    let mut physics = PhysicsConfigurationV2::SOFT_COUPLED;
    physics.fixed_step_hz = configuration.fixed_step_hz;
    physics.substeps = configuration.substeps;
    SolverProfileConfig::from_physics(physics)?.apply_to_scenario(&mut scenario)?;
    let stimulus_step = configuration.fixed_step_hz * 2;
    match fixture {
        FixtureKind::AppliedImpulse => {
            let body = scenario.bodies[0].clone();
            scenario.actions.push(NeutralActionV1::ApplyImpulse {
                step: stimulus_step,
                body: body.id,
                impulse: Vec2::new(1.5, 0.0),
                world_point: body.position,
            });
        }
        FixtureKind::ProjectileImpact => {
            let template = scenario
                .bodies
                .iter()
                .find(|body| body.inverse_mass.get() > 0.0)
                .cloned()
                .ok_or_else(|| "projectile fixture lacks a dynamic template".to_owned())?;
            let mut projectile = template;
            projectile.id = BodyId {
                slot: scenario.bodies.len() as u32,
                generation: 1,
            };
            projectile.shape = ShapeV1::Circle {
                radius: finite(0.2)?,
            };
            projectile.position = Vec2::new(-4.0, 0.2);
            projectile.inverse_inertia = finite(50.0)?;
            projectile.linear_velocity = Vec2::ZERO;
            projectile.angular_velocity = finite(0.0)?;
            scenario.bodies.push(projectile.clone());
            scenario.actions.push(NeutralActionV1::ApplyImpulse {
                step: stimulus_step,
                body: projectile.id,
                impulse: Vec2::new(4.0, 0.0),
                world_point: projectile.position,
            });
        }
        FixtureKind::Grab => {
            let body = scenario.bodies[0].clone();
            let target_step = stimulus_step + (configuration.fixed_step_hz / 10).max(1);
            scenario.actions = vec![
                NeutralActionV1::GrabBegin {
                    step: stimulus_step,
                    body: body.id,
                    world_anchor: body.position,
                    world_target: body.position,
                    tuning: provisional_grab_tuning(),
                },
                NeutralActionV1::GrabTarget {
                    step: target_step,
                    world_target: Vec2::new(body.position.x + 0.75, body.position.y + 0.75),
                },
                NeutralActionV1::GrabEnd {
                    step: configuration.fixed_step_hz * 3,
                },
            ];
        }
        _ => {}
    }
    scenario.validate().map_err(display)?;
    Ok(scenario)
}

fn two_independent_structures(
    requested_steps: u32,
) -> Result<physbox_contracts::ScenarioV1, physbox_engine::ScenarioBuildError> {
    let mut scenario = p5_2_centered_rectangle_stack_scenario(2, requested_steps)?;
    let floor = scenario.bodies[0].clone();
    let templates = scenario.bodies[1..].to_vec();
    let mut bodies = vec![floor];
    for (side, x) in [-1.25_f32, 1.25].into_iter().enumerate() {
        for (level, template) in templates.iter().enumerate() {
            let mut body = template.clone();
            body.id.slot = 1 + side as u32 * 2 + level as u32;
            body.position.x = x;
            bodies.push(body);
        }
    }
    scenario.id = "s23-two-independent-stacks".to_owned();
    scenario.bodies = bodies;
    scenario
        .validate()
        .map_err(|error| physbox_engine::ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

fn trace_value(trace: &TraceRecordV1, name: &str) -> Result<f32, String> {
    trace
        .values
        .iter()
        .find(|value| value.name == name)
        .map(|value| value.value.get())
        .ok_or_else(|| format!("trace step {} omitted {name}", trace.step))
}

fn farthest_point_speed(body: &BodyStateV1) -> f32 {
    let extent = match body.shape {
        ShapeV1::Circle { radius } => radius.get(),
        ShapeV1::Rectangle { half_extents } => half_extents.x.hypot(half_extents.y),
    };
    body.linear_velocity.x.hypot(body.linear_velocity.y)
        + body.angular_velocity.get().abs() * extent
}

fn finite(value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(display)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(display)
}

fn compare_gpu_baseline(
    baseline: &Path,
    current: &Path,
) -> Result<GpuBaselineComparisonV1, String> {
    let baseline_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(baseline.join("manifest.json")).map_err(display)?)
            .map_err(display)?;
    let baseline_source_revision = baseline_manifest["sourceRevision"]
        .as_str()
        .ok_or_else(|| "GPU baseline manifest lacks sourceRevision".to_owned())?
        .to_owned();
    let read_names = |directory: &Path| -> Result<Vec<String>, String> {
        let mut names = fs::read_dir(directory)
            .map_err(display)?
            .map(|entry| {
                entry
                    .map_err(display)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        names.sort();
        Ok(names)
    };
    let names = read_names(&current.join("gpu"))?;
    let baseline_names = read_names(&baseline.join("gpu"))?;
    if names != baseline_names {
        return Err("GPU baseline and current history file sets differ".to_owned());
    }
    if names.is_empty() {
        return Err("GPU baseline comparison found no current histories".to_owned());
    }
    let mut exact = true;
    for name in &names {
        let before: serde_json::Value =
            serde_json::from_slice(&fs::read(baseline.join("gpu").join(name)).map_err(display)?)
                .map_err(display)?;
        let after: serde_json::Value =
            serde_json::from_slice(&fs::read(current.join("gpu").join(name)).map_err(display)?)
                .map_err(display)?;
        exact &= before.get("snapshots") == after.get("snapshots");
    }
    Ok(GpuBaselineComparisonV1 {
        baseline_artifact: baseline.display().to_string(),
        baseline_source_revision,
        compared_histories: names.len() as u32,
        snapshot_histories_exact: exact,
    })
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(display)
}

fn json_hash(value: &impl Serialize) -> Result<String, String> {
    Ok(sha256_bytes(&json_bytes(value)?))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_bytes(repository, arguments)?)
        .trim()
        .to_owned())
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(display)?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut output = None;
    let mut oracle = None;
    let mut box2d_source = None;
    let mut run_id = None;
    let mut allow_dirty = false;
    let mut quick = false;
    let mut compare_gpu_baseline = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => output = Some(PathBuf::from(next(&mut arguments, "--output")?)),
            "--oracle" => oracle = Some(PathBuf::from(next(&mut arguments, "--oracle")?)),
            "--box2d-source" => {
                box2d_source = Some(PathBuf::from(next(&mut arguments, "--box2d-source")?))
            }
            "--run-id" => run_id = Some(next(&mut arguments, "--run-id")?),
            "--allow-dirty" => allow_dirty = true,
            "--quick" => quick = true,
            "--compare-gpu-baseline" => {
                compare_gpu_baseline = Some(PathBuf::from(next(
                    &mut arguments,
                    "--compare-gpu-baseline",
                )?))
            }
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    Ok(Arguments {
        output: output.ok_or_else(|| "missing --output PATH".to_owned())?,
        oracle: oracle.ok_or_else(|| "missing --oracle PATH".to_owned())?,
        box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source PATH".to_owned())?,
        run_id: run_id.ok_or_else(|| "missing --run-id ID".to_owned())?,
        allow_dirty,
        quick,
        compare_gpu_baseline,
    })
}

fn next(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
