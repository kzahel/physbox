use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyId, BodyStateV1, CandidatePairV1, CollisionManifoldV1, PersistentContactKeyV1,
    SCHEMA_VERSION, ShapeV1, Vec2, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, P5_2BucketShape, SolverProfileConfig,
    p5_2_bucket_scenario,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, run_oracle, verify_clean_git_pin,
};
use physbox_launch::{PhysicsConfigurationV3, SleepGrouping};
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 4;
const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const SLEEP_THRESHOLD: f32 = 0.02;
const DEFAULT_STEPS: u32 = 1_200;
const WINDOW_START_STEP: u32 = 600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactV1 {
    schema_version: u32,
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    sleep_threshold: f32,
    measurement_policy: MeasurementPolicyV1,
    adapter: physbox_engine::AdapterDiagnostics,
    scenes: Vec<SceneV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeasurementPolicyV1 {
    first_measured_step: u32,
    last_measured_step: u32,
    physbox_observation_interval_steps: u32,
    box2d_observation_steps: Vec<u32>,
    blocker_metric: &'static str,
    contact_tangent_speed: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneV1 {
    shape: &'static str,
    physbox: Vec<PhysBoxLaneV1>,
    box2d: Vec<Box2dLaneV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysBoxLaneV1 {
    lane: &'static str,
    sleeping_enabled: bool,
    sleep_grouping: SleepGrouping,
    sleep_contact_scan: &'static str,
    source_scenario_hash: String,
    solver_settings: physbox_contracts::SolverSettingsV1,
    solver_execution: physbox_engine::GpuSolverExecutionV4,
    solver_scheduling: physbox_engine::GpuSolverSchedulingPolicyV1,
    substep_contact_policy: physbox_engine::GpuSubstepContactPolicyV1,
    contact_coefficient_policy: physbox_engine::GpuContactCoefficientPolicyV1,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    motion: MotionWindowV1,
    contacts: ContactWindowV1,
    capacity_clean: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Box2dLaneV1 {
    lane: &'static str,
    warm_start: bool,
    artifact_paths: Vec<String>,
    observations: Vec<MotionSnapshotV1>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MotionSnapshotV1 {
    step: u32,
    dynamic_bodies: u32,
    awake_dynamic_bodies: u32,
    sleeping_dynamic_bodies: u32,
    blockers: u32,
    linear_only_blockers: u32,
    angular_only_blockers: u32,
    combined_only_blockers: u32,
    both_term_blockers: u32,
    angular_dominant_blockers: u32,
    maximum_linear_speed: f32,
    maximum_angular_edge_speed: f32,
    maximum_farthest_point_speed: f32,
    translational_kinetic_energy: f64,
    rotational_kinetic_energy: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MotionWindowV1 {
    total_awake_body_time: u64,
    total_sleeping_body_time: u64,
    sleep_body_events: u64,
    wake_body_events: u64,
    sleep_transition_steps: u32,
    wake_transition_steps: u32,
    first_sleep_transition_step: Option<u32>,
    first_wake_transition_step: Option<u32>,
    peak_sleep_body_events_in_step: u32,
    peak_wake_body_events_in_step: u32,
    first_step: u32,
    last_step: u32,
    sampled_steps: u32,
    dynamic_body_samples: u64,
    blocker_body_samples: u64,
    linear_only_blocker_body_samples: u64,
    angular_only_blocker_body_samples: u64,
    combined_only_blocker_body_samples: u64,
    both_term_blocker_body_samples: u64,
    angular_dominant_blocker_body_samples: u64,
    all_bodies_quiet_steps: u32,
    longest_all_bodies_quiet_run: u32,
    farthest_point_speed_p50: f32,
    farthest_point_speed_p95: f32,
    farthest_point_speed_p99: f32,
    maximum_linear_speed: f32,
    maximum_angular_edge_speed: f32,
    maximum_farthest_point_speed: f32,
    mean_translational_kinetic_energy: f64,
    mean_rotational_kinetic_energy: f64,
    maximum_translational_kinetic_energy: f64,
    maximum_rotational_kinetic_energy: f64,
    final_snapshot: MotionSnapshotV1,
    step_extrema: Vec<MotionSnapshotV1>,
    final_bodies_by_farthest_speed: Vec<BodyMotionV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyMotionV1 {
    slot: u32,
    linear_speed: f32,
    angular_velocity: f32,
    angular_edge_speed: f32,
    farthest_point_speed: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactWindowV1 {
    point_samples: u64,
    one_point_manifold_samples: u64,
    two_point_manifold_samples: u64,
    mean_absolute_tangent_impulse: f64,
    maximum_absolute_tangent_impulse: f32,
    mean_absolute_relative_tangent_speed: f64,
    maximum_absolute_relative_tangent_speed: f32,
    new_persistent_points: u64,
    lost_persistent_points: u64,
    new_manifold_pairs: u64,
    lost_manifold_pairs: u64,
    mean_dynamic_component_count: f64,
    minimum_dynamic_component_count: u32,
    maximum_dynamic_component_count: u32,
    mean_largest_dynamic_component_size: f64,
    minimum_largest_dynamic_component_size: u32,
    maximum_largest_dynamic_component_size: u32,
    mixed_sleep_state_dynamic_pair_samples: u64,
}

struct Arguments {
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    run_id: String,
    steps: u32,
    allow_dirty: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-circle-sleep-diagnostic error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    if arguments.steps < WINDOW_START_STEP {
        return Err(format!(
            "--steps must be at least {WINDOW_START_STEP} for the settled-window diagnostic"
        ));
    }
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("diagnostic output and staging paths must be fresh".to_owned());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if source_dirty && !arguments.allow_dirty {
        return Err("diagnostic source is dirty; pass --allow-dirty to record it".to_owned());
    }
    let box2d = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let box2d_steps = observation_steps(arguments.steps);
    let oracle_root = arguments.output.with_extension("oracle");
    if oracle_root.exists() {
        return Err("diagnostic oracle root must be fresh".to_owned());
    }

    let mut scenes = Vec::new();
    for shape in P5_2BucketShape::ALL {
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled exact sleep, packed control",
            shape.name()
        );
        let soft_exact =
            configured_bucket(shape, arguments.steps, PhysicsConfigurationV3::SOFT_COUPLED)?;
        let soft_exact_series = runner
            .run_branch_robust_tunable_packed_sleep_scan_observation_series_with_submission_batch(
                &soft_exact,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Exact,
            )?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled exact sleep, pair-slot span",
            shape.name()
        );
        let soft_exact_pair_slot_series = runner
            .run_branch_robust_tunable_sleeping_observation_series_with_submission_batch(
                &soft_exact,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Exact,
            )?;
        validate_isolated_sleep_scan_ab(&soft_exact_series, &soft_exact_pair_slot_series)?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled local-4 sleep, packed control",
            shape.name()
        );
        let soft_local4 = configured_bucket(
            shape,
            arguments.steps,
            soft_physics(true, SleepGrouping::Local4, 1),
        )?;
        let soft_local4_series = runner
            .run_branch_robust_tunable_packed_sleep_scan_observation_series_with_submission_batch(
                &soft_local4,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Local4,
            )?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled local-4 sleep, pair-slot span",
            shape.name()
        );
        let soft_local4_pair_slot_series = runner
            .run_branch_robust_tunable_sleeping_observation_series_with_submission_batch(
                &soft_local4,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Local4,
            )?;
        validate_isolated_sleep_scan_ab(&soft_local4_series, &soft_local4_pair_slot_series)?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled local-8 sleep, packed control",
            shape.name()
        );
        let soft_local8 = configured_bucket(
            shape,
            arguments.steps,
            soft_physics(true, SleepGrouping::Local8, 1),
        )?;
        let soft_local8_series = runner
            .run_branch_robust_tunable_packed_sleep_scan_observation_series_with_submission_batch(
                &soft_local8,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Local8,
            )?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled local-8 sleep, pair-slot span",
            shape.name()
        );
        let soft_local8_pair_slot_series = runner
            .run_branch_robust_tunable_sleeping_observation_series_with_submission_batch(
                &soft_local8,
                1,
                arguments.steps,
                32,
                PhysicsConfigurationV3::SOFT_COUPLED.fixed_step_hz,
                PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
                SleepGrouping::Local8,
            )?;
        validate_isolated_sleep_scan_ab(&soft_local8_series, &soft_local8_pair_slot_series)?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled world awake",
            shape.name()
        );
        let soft_awake = configured_bucket(
            shape,
            arguments.steps,
            soft_physics(false, SleepGrouping::Exact, 1),
        )?;
        let soft_awake_series = runner
            .run_branch_robust_tunable_observation_series_with_submission_batch(
                &soft_awake,
                1,
                arguments.steps,
                32,
            )?;
        eprintln!(
            "diagnostic {} bucket: PhysBox soft-coupled i4 world awake",
            shape.name()
        );
        let soft_awake_i4 = configured_bucket(
            shape,
            arguments.steps,
            soft_physics(false, SleepGrouping::Exact, 4),
        )?;
        let soft_awake_i4_series = runner
            .run_branch_robust_tunable_observation_series_with_submission_batch(
                &soft_awake_i4,
                1,
                arguments.steps,
                32,
            )?;
        let physbox = vec![
            summarize_physbox_lane(
                "soft-coupled-exact-sleep",
                &soft_exact_series,
                true,
                SleepGrouping::Exact,
                "packed-point-count",
            )?,
            summarize_physbox_lane(
                "soft-coupled-exact-sleep-pair-slot-span",
                &soft_exact_pair_slot_series,
                true,
                SleepGrouping::Exact,
                "pair-slot-manifold-span",
            )?,
            summarize_physbox_lane(
                "soft-coupled-local-4-sleep",
                &soft_local4_series,
                true,
                SleepGrouping::Local4,
                "packed-point-count",
            )?,
            summarize_physbox_lane(
                "soft-coupled-local-4-sleep-pair-slot-span",
                &soft_local4_pair_slot_series,
                true,
                SleepGrouping::Local4,
                "pair-slot-manifold-span",
            )?,
            summarize_physbox_lane(
                "soft-coupled-local-8-sleep",
                &soft_local8_series,
                true,
                SleepGrouping::Local8,
                "packed-point-count",
            )?,
            summarize_physbox_lane(
                "soft-coupled-local-8-sleep-pair-slot-span",
                &soft_local8_pair_slot_series,
                true,
                SleepGrouping::Local8,
                "pair-slot-manifold-span",
            )?,
            summarize_physbox_lane(
                "soft-coupled-world-awake",
                &soft_awake_series,
                false,
                SleepGrouping::Exact,
                "disabled",
            )?,
            summarize_physbox_lane(
                "soft-coupled-i4-world-awake",
                &soft_awake_i4_series,
                false,
                SleepGrouping::Exact,
                "disabled",
            )?,
        ];

        let mut box2d_lanes = Vec::new();
        for warm_start in [true, false] {
            let lane = if warm_start {
                "world-awake-warm-start-on"
            } else {
                "world-awake-warm-start-off"
            };
            eprintln!("diagnostic {} bucket: Box2D {lane}", shape.name());
            let mut observations = Vec::new();
            let mut artifact_paths = Vec::new();
            for step in &box2d_steps {
                let scenario =
                    configured_bucket(shape, *step, PhysicsConfigurationV3::SOFT_COUPLED)?;
                let source_scenario_hash =
                    canonical_hash(&scenario).map_err(|error| error.to_string())?;
                let request = physbox_contracts::OracleRequestV1 {
                    schema_version: SCHEMA_VERSION,
                    run_id: format!(
                        "{}-{}-{}-{}",
                        arguments.run_id,
                        shape.name(),
                        if warm_start { "warm-on" } else { "warm-off" },
                        step
                    ),
                    source_scenario_hash,
                    scenario,
                };
                let config = OracleRunConfig {
                    artifact_root: oracle_root.clone(),
                    timeout: Duration::from_secs(30),
                    source: box2d.clone(),
                    process: OracleProcess {
                        program: arguments.oracle.clone(),
                        arguments: vec![
                            "--lane".into(),
                            "world-awake".into(),
                            "--warm-start".into(),
                            if warm_start { "on" } else { "off" }.into(),
                            "--capture-step-trace".into(),
                            "off".into(),
                        ],
                    },
                };
                let result = run_oracle(&config, &request).map_err(|error| error.to_string())?;
                observations.push(motion_snapshot(*step, &result.result.bodies));
                artifact_paths.push(result.artifacts.result.to_string_lossy().replace('\\', "/"));
            }
            box2d_lanes.push(Box2dLaneV1 {
                lane,
                warm_start,
                artifact_paths,
                observations,
            });
        }
        scenes.push(SceneV1 {
            shape: shape.name(),
            physbox,
            box2d: box2d_lanes,
        });
    }

    let artifact = ArtifactV1 {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "physbox5-circle-sleep-residual-diagnostic",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        sleep_threshold: SLEEP_THRESHOLD,
        measurement_policy: MeasurementPolicyV1 {
            first_measured_step: WINDOW_START_STEP,
            last_measured_step: arguments.steps,
            physbox_observation_interval_steps: 1,
            box2d_observation_steps: box2d_steps,
            blocker_metric: "|linear velocity| + |angular velocity| * radial extent",
            contact_tangent_speed: "absolute relative point velocity projected onto the manifold tangent",
        },
        adapter: runner.diagnostics().clone(),
        scenes,
    };
    let bytes = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    fs::write(&staging, &bytes).map_err(|error| error.to_string())?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=circle-sleep-diagnostic run_id={} output={} sha256={}",
        artifact.run_id,
        arguments.output.display(),
        sha256_bytes(&bytes),
    );
    Ok(())
}

fn configured_bucket(
    shape: P5_2BucketShape,
    steps: u32,
    physics: PhysicsConfigurationV3,
) -> Result<physbox_contracts::ScenarioV1, String> {
    let mut scenario =
        p5_2_bucket_scenario(shape, 100, steps).map_err(|error| error.to_string())?;
    SolverProfileConfig::from_physics(physics)?.apply_to_scenario(&mut scenario)?;
    Ok(scenario)
}

fn soft_physics(
    sleeping_enabled: bool,
    sleep_grouping: SleepGrouping,
    biased_iterations: u32,
) -> PhysicsConfigurationV3 {
    PhysicsConfigurationV3 {
        sleeping_enabled,
        sleep_grouping,
        biased_iterations,
        ..PhysicsConfigurationV3::SOFT_COUPLED
    }
}

fn summarize_physbox_lane(
    lane: &'static str,
    series: &GpuGeneratedContactSeriesV1,
    sleeping_enabled: bool,
    sleep_grouping: SleepGrouping,
    sleep_contact_scan: &'static str,
) -> Result<PhysBoxLaneV1, String> {
    series.validate()?;
    let capacity_clean = series.snapshots.iter().all(|snapshot| {
        snapshot.failure.is_none() && snapshot.capacities.iter().all(|capacity| capacity.accepted)
    });
    Ok(PhysBoxLaneV1 {
        lane,
        sleeping_enabled,
        sleep_grouping,
        sleep_contact_scan,
        source_scenario_hash: series.source_scenario_hash.clone(),
        solver_settings: series.solver_settings,
        solver_execution: series.solver_execution,
        solver_scheduling: series.solver_scheduling_policy,
        substep_contact_policy: series.substep_contact_policy,
        contact_coefficient_policy: series.contact_coefficient_policy,
        dispatches_per_step: series.dispatches_per_step,
        allocated_bytes: series.allocated_bytes,
        motion: summarize_motion(series)?,
        contacts: summarize_contacts(series),
        capacity_clean,
    })
}

fn validate_isolated_sleep_scan_ab(
    packed: &GpuGeneratedContactSeriesV1,
    pair_slot_span: &GpuGeneratedContactSeriesV1,
) -> Result<(), String> {
    if packed.source_scenario_hash != pair_slot_span.source_scenario_hash
        || packed.solver_settings != pair_slot_span.solver_settings
        || packed.solver_execution != pair_slot_span.solver_execution
        || packed.solver_scheduling_policy != pair_slot_span.solver_scheduling_policy
        || packed.contact_coefficient_policy != pair_slot_span.contact_coefficient_policy
        || packed.substep_contact_policy != pair_slot_span.substep_contact_policy
        || packed.restitution_timing_policy != pair_slot_span.restitution_timing_policy
        || packed.contact_identity_policy != pair_slot_span.contact_identity_policy
        || packed.soft_feedback_policy != pair_slot_span.soft_feedback_policy
        || packed.normal_relaxation_policy != pair_slot_span.normal_relaxation_policy
    {
        return Err("sleep contact-scan A/B changed a solver or scenario policy".to_owned());
    }
    if packed.dispatches_per_step != pair_slot_span.dispatches_per_step
        || packed.allocated_bytes != pair_slot_span.allocated_bytes
    {
        return Err("sleep contact-scan A/B changed dispatch or allocation cost".to_owned());
    }
    Ok(())
}

fn summarize_motion(series: &GpuGeneratedContactSeriesV1) -> Result<MotionWindowV1, String> {
    let mut previous_sleeping = BTreeSet::<BodyId>::new();
    let mut total_awake_body_time = 0_u64;
    let mut total_sleeping_body_time = 0_u64;
    let mut sleep_body_events = 0_u64;
    let mut wake_body_events = 0_u64;
    let mut sleep_transition_steps = 0_u32;
    let mut wake_transition_steps = 0_u32;
    let mut first_sleep_transition_step = None;
    let mut first_wake_transition_step = None;
    let mut peak_sleep_body_events_in_step = 0_u32;
    let mut peak_wake_body_events_in_step = 0_u32;
    for snapshot in &series.snapshots {
        let dynamic = snapshot
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0)
            .count() as u64;
        let sleeping = snapshot
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0 && body.sleeping)
            .map(|body| body.id)
            .collect::<BTreeSet<_>>();
        total_sleeping_body_time += sleeping.len() as u64;
        total_awake_body_time += dynamic - sleeping.len() as u64;
        let slept = sleeping.difference(&previous_sleeping).count() as u32;
        let woke = previous_sleeping.difference(&sleeping).count() as u32;
        if slept != 0 {
            sleep_body_events += u64::from(slept);
            sleep_transition_steps += 1;
            first_sleep_transition_step.get_or_insert(snapshot.completed_steps);
            peak_sleep_body_events_in_step = peak_sleep_body_events_in_step.max(slept);
        }
        if woke != 0 {
            wake_body_events += u64::from(woke);
            wake_transition_steps += 1;
            first_wake_transition_step.get_or_insert(snapshot.completed_steps);
            peak_wake_body_events_in_step = peak_wake_body_events_in_step.max(woke);
        }
        previous_sleeping = sleeping;
    }
    let snapshots = series
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.completed_steps >= WINDOW_START_STEP)
        .collect::<Vec<_>>();
    let final_state = snapshots
        .last()
        .ok_or_else(|| "diagnostic motion window is empty".to_owned())?;
    let mut farthest_speeds = Vec::new();
    let mut blocker_body_samples = 0_u64;
    let mut linear_only = 0_u64;
    let mut angular_only = 0_u64;
    let mut combined_only = 0_u64;
    let mut both_terms = 0_u64;
    let mut angular_dominant = 0_u64;
    let mut dynamic_body_samples = 0_u64;
    let mut all_quiet_steps = 0_u32;
    let mut current_all_quiet_run = 0_u32;
    let mut longest_all_quiet_run = 0_u32;
    let mut maximum_linear_speed = 0.0_f32;
    let mut maximum_angular_edge_speed = 0.0_f32;
    let mut maximum_farthest_point_speed = 0.0_f32;
    let mut translational_energy_sum = 0.0_f64;
    let mut rotational_energy_sum = 0.0_f64;
    let mut maximum_translational_energy = 0.0_f64;
    let mut maximum_rotational_energy = 0.0_f64;
    let mut step_extrema = Vec::new();
    for snapshot in &snapshots {
        let metrics = motion_snapshot(snapshot.completed_steps, &snapshot.bodies);
        dynamic_body_samples += u64::from(metrics.dynamic_bodies);
        blocker_body_samples += u64::from(metrics.blockers);
        linear_only += u64::from(metrics.linear_only_blockers);
        angular_only += u64::from(metrics.angular_only_blockers);
        combined_only += u64::from(metrics.combined_only_blockers);
        both_terms += u64::from(metrics.both_term_blockers);
        angular_dominant += u64::from(metrics.angular_dominant_blockers);
        maximum_linear_speed = maximum_linear_speed.max(metrics.maximum_linear_speed);
        maximum_angular_edge_speed =
            maximum_angular_edge_speed.max(metrics.maximum_angular_edge_speed);
        maximum_farthest_point_speed =
            maximum_farthest_point_speed.max(metrics.maximum_farthest_point_speed);
        translational_energy_sum += metrics.translational_kinetic_energy;
        rotational_energy_sum += metrics.rotational_kinetic_energy;
        maximum_translational_energy =
            maximum_translational_energy.max(metrics.translational_kinetic_energy);
        maximum_rotational_energy =
            maximum_rotational_energy.max(metrics.rotational_kinetic_energy);
        if metrics.blockers == 0 {
            all_quiet_steps += 1;
            current_all_quiet_run += 1;
            longest_all_quiet_run = longest_all_quiet_run.max(current_all_quiet_run);
        } else {
            current_all_quiet_run = 0;
        }
        for body in snapshot
            .bodies
            .iter()
            .filter(|body| body.inverse_mass.get() > 0.0)
        {
            farthest_speeds.push(body_motion(body).farthest_point_speed);
        }
        if snapshot.completed_steps == WINDOW_START_STEP
            || snapshot.completed_steps.is_multiple_of(50)
            || snapshot.completed_steps == series.completed_steps
        {
            step_extrema.push(metrics);
        }
    }
    farthest_speeds.sort_by(f32::total_cmp);
    let mut final_bodies = final_state
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .map(body_motion)
        .collect::<Vec<_>>();
    final_bodies.sort_by(|left, right| {
        right
            .farthest_point_speed
            .total_cmp(&left.farthest_point_speed)
            .then_with(|| left.slot.cmp(&right.slot))
    });
    final_bodies.truncate(12);
    let sampled_steps = snapshots.len() as u32;
    Ok(MotionWindowV1 {
        total_awake_body_time,
        total_sleeping_body_time,
        sleep_body_events,
        wake_body_events,
        sleep_transition_steps,
        wake_transition_steps,
        first_sleep_transition_step,
        first_wake_transition_step,
        peak_sleep_body_events_in_step,
        peak_wake_body_events_in_step,
        first_step: snapshots[0].completed_steps,
        last_step: final_state.completed_steps,
        sampled_steps,
        dynamic_body_samples,
        blocker_body_samples,
        linear_only_blocker_body_samples: linear_only,
        angular_only_blocker_body_samples: angular_only,
        combined_only_blocker_body_samples: combined_only,
        both_term_blocker_body_samples: both_terms,
        angular_dominant_blocker_body_samples: angular_dominant,
        all_bodies_quiet_steps: all_quiet_steps,
        longest_all_bodies_quiet_run: longest_all_quiet_run,
        farthest_point_speed_p50: percentile(&farthest_speeds, 50),
        farthest_point_speed_p95: percentile(&farthest_speeds, 95),
        farthest_point_speed_p99: percentile(&farthest_speeds, 99),
        maximum_linear_speed,
        maximum_angular_edge_speed,
        maximum_farthest_point_speed,
        mean_translational_kinetic_energy: translational_energy_sum / f64::from(sampled_steps),
        mean_rotational_kinetic_energy: rotational_energy_sum / f64::from(sampled_steps),
        maximum_translational_kinetic_energy: maximum_translational_energy,
        maximum_rotational_kinetic_energy: maximum_rotational_energy,
        final_snapshot: motion_snapshot(final_state.completed_steps, &final_state.bodies),
        step_extrema,
        final_bodies_by_farthest_speed: final_bodies,
    })
}

fn motion_snapshot(step: u32, bodies: &[BodyStateV1]) -> MotionSnapshotV1 {
    let mut output = MotionSnapshotV1 {
        step,
        dynamic_bodies: 0,
        awake_dynamic_bodies: 0,
        sleeping_dynamic_bodies: 0,
        blockers: 0,
        linear_only_blockers: 0,
        angular_only_blockers: 0,
        combined_only_blockers: 0,
        both_term_blockers: 0,
        angular_dominant_blockers: 0,
        maximum_linear_speed: 0.0,
        maximum_angular_edge_speed: 0.0,
        maximum_farthest_point_speed: 0.0,
        translational_kinetic_energy: 0.0,
        rotational_kinetic_energy: 0.0,
    };
    for body in bodies.iter().filter(|body| body.inverse_mass.get() > 0.0) {
        let motion = body_motion(body);
        output.dynamic_bodies += 1;
        output.sleeping_dynamic_bodies += u32::from(body.sleeping);
        output.awake_dynamic_bodies += u32::from(!body.sleeping);
        output.maximum_linear_speed = output.maximum_linear_speed.max(motion.linear_speed);
        output.maximum_angular_edge_speed = output
            .maximum_angular_edge_speed
            .max(motion.angular_edge_speed);
        output.maximum_farthest_point_speed = output
            .maximum_farthest_point_speed
            .max(motion.farthest_point_speed);
        let linear_exceeds = motion.linear_speed > SLEEP_THRESHOLD;
        let angular_exceeds = motion.angular_edge_speed > SLEEP_THRESHOLD;
        let blocks = motion.farthest_point_speed > SLEEP_THRESHOLD;
        if blocks {
            output.blockers += 1;
            output.angular_dominant_blockers +=
                u32::from(motion.angular_edge_speed > motion.linear_speed);
            match (linear_exceeds, angular_exceeds) {
                (true, false) => output.linear_only_blockers += 1,
                (false, true) => output.angular_only_blockers += 1,
                (true, true) => output.both_term_blockers += 1,
                (false, false) => output.combined_only_blockers += 1,
            }
        }
        let inverse_mass = body.inverse_mass.get();
        let inverse_inertia = body.inverse_inertia.get();
        output.translational_kinetic_energy +=
            0.5 * f64::from(motion.linear_speed * motion.linear_speed / inverse_mass);
        if inverse_inertia > 0.0 {
            output.rotational_kinetic_energy += 0.5
                * f64::from(
                    body.angular_velocity.get() * body.angular_velocity.get() / inverse_inertia,
                );
        }
    }
    output
}

fn body_motion(body: &BodyStateV1) -> BodyMotionV1 {
    let linear_speed = body.linear_velocity.x.hypot(body.linear_velocity.y);
    let angular_velocity = body.angular_velocity.get();
    let angular_edge_speed = angular_velocity.abs() * radial_extent(body);
    BodyMotionV1 {
        slot: body.id.slot,
        linear_speed,
        angular_velocity,
        angular_edge_speed,
        farthest_point_speed: linear_speed + angular_edge_speed,
    }
}

fn radial_extent(body: &BodyStateV1) -> f32 {
    match body.shape {
        ShapeV1::Circle { radius } => radius.get(),
        ShapeV1::Rectangle { half_extents } => half_extents.x.hypot(half_extents.y),
    }
}

fn summarize_contacts(series: &GpuGeneratedContactSeriesV1) -> ContactWindowV1 {
    let mut point_samples = 0_u64;
    let mut one_point_manifolds = 0_u64;
    let mut two_point_manifolds = 0_u64;
    let mut tangent_impulse_sum = 0.0_f64;
    let mut tangent_impulse_max = 0.0_f32;
    let mut tangent_speed_sum = 0.0_f64;
    let mut tangent_speed_max = 0.0_f32;
    let mut new_points = 0_u64;
    let mut lost_points = 0_u64;
    let mut new_pairs = 0_u64;
    let mut lost_pairs = 0_u64;
    let mut component_samples = 0_u64;
    let mut component_count_sum = 0_u64;
    let mut minimum_component_count = u32::MAX;
    let mut maximum_component_count = 0_u32;
    let mut largest_component_sum = 0_u64;
    let mut minimum_largest_component = u32::MAX;
    let mut maximum_largest_component = 0_u32;
    let mut mixed_sleep_state_pairs = 0_u64;
    let mut previous = BTreeSet::<PersistentContactKeyV1>::new();
    let mut previous_pairs = BTreeSet::<CandidatePairV1>::new();
    for snapshot in series
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.completed_steps >= WINDOW_START_STEP)
    {
        let components = dynamic_contact_components(&snapshot.bodies, &snapshot.manifolds);
        component_samples += 1;
        component_count_sum += u64::from(components.component_count);
        minimum_component_count = minimum_component_count.min(components.component_count);
        maximum_component_count = maximum_component_count.max(components.component_count);
        largest_component_sum += u64::from(components.largest_component_size);
        minimum_largest_component =
            minimum_largest_component.min(components.largest_component_size);
        maximum_largest_component =
            maximum_largest_component.max(components.largest_component_size);
        mixed_sleep_state_pairs += u64::from(components.mixed_sleep_state_pairs);
        for value in &snapshot.cache {
            let impulse = value.value.tangent_impulse.get().abs();
            tangent_impulse_sum += f64::from(impulse);
            tangent_impulse_max = tangent_impulse_max.max(impulse);
        }
        for manifold in &snapshot.manifolds {
            match manifold.points.len() {
                1 => one_point_manifolds += 1,
                2 => two_point_manifolds += 1,
                _ => {}
            }
            for point in &manifold.points {
                let speed = relative_tangent_speed(
                    &snapshot.bodies,
                    manifold,
                    point.local_anchor_a,
                    point.local_anchor_b,
                );
                tangent_speed_sum += f64::from(speed);
                tangent_speed_max = tangent_speed_max.max(speed);
                point_samples += 1;
            }
        }
        let current = snapshot
            .cache
            .iter()
            .map(|value| value.key)
            .collect::<BTreeSet<_>>();
        if !previous.is_empty() {
            new_points += current.difference(&previous).count() as u64;
            lost_points += previous.difference(&current).count() as u64;
        }
        previous = current;
        let current_pairs = snapshot
            .manifolds
            .iter()
            .map(|manifold| manifold.pair)
            .collect::<BTreeSet<_>>();
        if !previous_pairs.is_empty() {
            new_pairs += current_pairs.difference(&previous_pairs).count() as u64;
            lost_pairs += previous_pairs.difference(&current_pairs).count() as u64;
        }
        previous_pairs = current_pairs;
    }
    ContactWindowV1 {
        point_samples,
        one_point_manifold_samples: one_point_manifolds,
        two_point_manifold_samples: two_point_manifolds,
        mean_absolute_tangent_impulse: if point_samples == 0 {
            0.0
        } else {
            tangent_impulse_sum / point_samples as f64
        },
        maximum_absolute_tangent_impulse: tangent_impulse_max,
        mean_absolute_relative_tangent_speed: if point_samples == 0 {
            0.0
        } else {
            tangent_speed_sum / point_samples as f64
        },
        maximum_absolute_relative_tangent_speed: tangent_speed_max,
        new_persistent_points: new_points,
        lost_persistent_points: lost_points,
        new_manifold_pairs: new_pairs,
        lost_manifold_pairs: lost_pairs,
        mean_dynamic_component_count: component_count_sum as f64 / component_samples as f64,
        minimum_dynamic_component_count: minimum_component_count,
        maximum_dynamic_component_count: maximum_component_count,
        mean_largest_dynamic_component_size: largest_component_sum as f64
            / component_samples as f64,
        minimum_largest_dynamic_component_size: minimum_largest_component,
        maximum_largest_dynamic_component_size: maximum_largest_component,
        mixed_sleep_state_dynamic_pair_samples: mixed_sleep_state_pairs,
    }
}

struct DynamicComponents {
    component_count: u32,
    largest_component_size: u32,
    mixed_sleep_state_pairs: u32,
}

fn dynamic_contact_components(
    bodies: &[BodyStateV1],
    manifolds: &[CollisionManifoldV1],
) -> DynamicComponents {
    let mut parents = (0..bodies.len()).collect::<Vec<_>>();
    let dynamic = bodies
        .iter()
        .map(|body| body.inverse_mass.get() > 0.0)
        .collect::<Vec<_>>();
    let mut mixed_sleep_state_pairs = 0_u32;
    for manifold in manifolds {
        let a = manifold.pair.body_a.slot as usize;
        let b = manifold.pair.body_b.slot as usize;
        if !dynamic[a] || !dynamic[b] {
            continue;
        }
        mixed_sleep_state_pairs += u32::from(bodies[a].sleeping != bodies[b].sleeping);
        union(&mut parents, a, b);
    }
    let mut sizes = vec![0_u32; bodies.len()];
    for (slot, is_dynamic) in dynamic.iter().copied().enumerate() {
        if is_dynamic {
            let root = find(&mut parents, slot);
            sizes[root] += 1;
        }
    }
    DynamicComponents {
        component_count: sizes.iter().filter(|size| **size != 0).count() as u32,
        largest_component_size: sizes.into_iter().max().unwrap_or(0),
        mixed_sleep_state_pairs,
    }
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find(parents, left);
    let right_root = find(parents, right);
    if left_root != right_root {
        let low = left_root.min(right_root);
        let high = left_root.max(right_root);
        parents[high] = low;
    }
}

fn find(parents: &mut [usize], mut slot: usize) -> usize {
    while parents[slot] != slot {
        parents[slot] = parents[parents[slot]];
        slot = parents[slot];
    }
    slot
}

fn relative_tangent_speed(
    bodies: &[BodyStateV1],
    manifold: &CollisionManifoldV1,
    local_anchor_a: Vec2,
    local_anchor_b: Vec2,
) -> f32 {
    let body_a = &bodies[manifold.pair.body_a.slot as usize];
    let body_b = &bodies[manifold.pair.body_b.slot as usize];
    debug_assert_eq!(body_a.id, manifold.pair.body_a);
    debug_assert_eq!(body_b.id, manifold.pair.body_b);
    let anchor_a = rotate(local_anchor_a, body_a.angle.get());
    let anchor_b = rotate(local_anchor_b, body_b.angle.get());
    let velocity_a = point_velocity(body_a, anchor_a);
    let velocity_b = point_velocity(body_b, anchor_b);
    let tangent = Vec2::new(manifold.normal.y, -manifold.normal.x);
    ((velocity_b.x - velocity_a.x) * tangent.x + (velocity_b.y - velocity_a.y) * tangent.y).abs()
}

fn point_velocity(body: &BodyStateV1, anchor: Vec2) -> Vec2 {
    let angular = body.angular_velocity.get();
    Vec2::new(
        body.linear_velocity.x - angular * anchor.y,
        body.linear_velocity.y + angular * anchor.x,
    )
}

fn rotate(value: Vec2, angle: f32) -> Vec2 {
    let cosine = angle.cos();
    let sine = angle.sin();
    Vec2::new(
        cosine * value.x - sine * value.y,
        sine * value.x + cosine * value.y,
    )
}

fn percentile(sorted: &[f32], percentile: usize) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) * percentile) / 100;
    sorted[index]
}

fn observation_steps(completed_steps: u32) -> Vec<u32> {
    [WINDOW_START_STEP, 900, completed_steps]
        .into_iter()
        .filter(|step| *step <= completed_steps)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut run_id = None;
        let mut steps = DEFAULT_STEPS;
        let mut allow_dirty = false;
        let mut arguments = env::args().skip(1);
        while let Some(argument) = arguments.next() {
            if argument == "--allow-dirty" {
                allow_dirty = true;
                continue;
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--output" => output = Some(PathBuf::from(value)),
                "--oracle" => oracle = Some(PathBuf::from(value)),
                "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
                "--run-id" => run_id = Some(value),
                "--steps" => steps = value.parse().map_err(|_| "invalid --steps".to_owned())?,
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            steps,
            allow_dirty,
        })
    }
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    let status = command_output(
        repository,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let patch_digest = dirty
        .then(|| {
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
            Ok(sha256_bytes(&output.stdout))
        })
        .transpose()?;
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
