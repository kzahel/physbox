use std::{env, process::ExitCode, time::Instant};

use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FailureKindV1, FiniteF32, ScenarioV1,
    Validate,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, P5_2BucketShape,
    gpu_contact_softness_coefficients, p5_2_bucket_scenario,
    p5_2_centered_rectangle_stack_scenario,
};
use physbox_lab::settling::centered_stack_structure;

const DEFAULT_HERTZ: f32 = 30.0;
const DEFAULT_DAMPING: f32 = 10.0;
const DEFAULT_STATIC_SCALE: f32 = 2.0;
const DEFAULT_SIM_HERTZ: u32 = 60;
const DEFAULT_SUBSTEPS: u32 = 4;
const DEFAULT_ITERATIONS: u32 = 1;
const SUBMISSION_BATCH_STEPS: u32 = 64;
const QUIET_SPEED_LIMIT: f32 = 0.025;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneSelection {
    Both,
    Selected,
    Candidate,
}

impl LaneSelection {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "both" => Ok(Self::Both),
            "selected" => Ok(Self::Selected),
            "candidate" => Ok(Self::Candidate),
            _ => Err(format!(
                "invalid --lane {value}; expected both, selected, or candidate"
            )),
        }
    }

    fn lanes(self) -> &'static [Lane] {
        match self {
            Self::Both => &[Lane::Selected, Lane::Candidate],
            Self::Selected => &[Lane::Selected],
            Self::Candidate => &[Lane::Candidate],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lane {
    Selected,
    Candidate,
}

impl Lane {
    const fn name(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Candidate => "candidate",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scene {
    Stack(u32),
    Bucket,
}

impl Scene {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "stack4" => Ok(Self::Stack(4)),
            "stack8" => Ok(Self::Stack(8)),
            "stack16" => Ok(Self::Stack(16)),
            "stack32" => Ok(Self::Stack(32)),
            "stack64" => Ok(Self::Stack(64)),
            "bucket" => Ok(Self::Bucket),
            _ => Err(format!(
                "invalid scene {value}; expected stack4, stack8, stack16, stack32, stack64, or bucket"
            )),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Stack(4) => "stack4",
            Self::Stack(8) => "stack8",
            Self::Stack(16) => "stack16",
            Self::Stack(32) => "stack32",
            Self::Stack(64) => "stack64",
            Self::Stack(_) => "stack",
            Self::Bucket => "bucket",
        }
    }

    const fn default_steps(self) -> u32 {
        match self {
            Self::Stack(_) => 1_200,
            Self::Bucket => 600,
        }
    }

    fn scenario(self, steps: u32) -> Result<ScenarioV1, String> {
        match self {
            Self::Stack(count) => {
                p5_2_centered_rectangle_stack_scenario(count, steps).map_err(display)
            }
            Self::Bucket => {
                p5_2_bucket_scenario(P5_2BucketShape::Rectangle, 100, steps).map_err(display)
            }
        }
    }

    fn stands(
        self,
        scenario: &ScenarioV1,
        series: &GpuGeneratedContactSeriesV1,
    ) -> StructureResult {
        let mut first_loss = None;
        let mut final_pass = true;
        for snapshot in &series.snapshots {
            let capacity_clean = snapshot_capacity_clean(snapshot);
            let passed = match self {
                Self::Stack(_) => {
                    centered_stack_structure(scenario, &snapshot.bodies, !capacity_clean, 0.0).0
                }
                Self::Bucket => {
                    capacity_clean
                        && snapshot.bodies.len() == scenario.bodies.len()
                        && snapshot.bodies.iter().all(|body| body.validate().is_ok())
                }
            };
            if !passed && first_loss.is_none() {
                first_loss = Some(snapshot.completed_steps);
            }
            final_pass = passed;
        }
        StructureResult {
            final_pass,
            first_loss,
        }
    }
}

#[derive(Debug)]
struct Arguments {
    sim_hertz: u32,
    substeps: u32,
    iterations: u32,
    hertz: f32,
    damping: f32,
    static_scale: f32,
    lane: LaneSelection,
    scenes: Vec<Scene>,
    steps: Option<u32>,
}

impl Default for Arguments {
    fn default() -> Self {
        Self {
            sim_hertz: DEFAULT_SIM_HERTZ,
            substeps: DEFAULT_SUBSTEPS,
            iterations: DEFAULT_ITERATIONS,
            hertz: DEFAULT_HERTZ,
            damping: DEFAULT_DAMPING,
            static_scale: DEFAULT_STATIC_SCALE,
            lane: LaneSelection::Both,
            scenes: vec![
                Scene::Stack(4),
                Scene::Stack(8),
                Scene::Stack(16),
                Scene::Bucket,
            ],
            steps: None,
        }
    }
}

impl Arguments {
    fn parse() -> Result<Option<Self>, String> {
        let mut arguments = Self::default();
        let values = env::args().skip(1).collect::<Vec<_>>();
        let mut index = 0;
        while index < values.len() {
            let flag = &values[index];
            if matches!(flag.as_str(), "-h" | "--help") {
                print_help();
                return Ok(None);
            }
            let value = values
                .get(index + 1)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            match flag.as_str() {
                "--sim-hz" => arguments.sim_hertz = parse_bounded_u32(flag, value, 15, 240)?,
                "--substeps" => arguments.substeps = parse_bounded_u32(flag, value, 1, 8)?,
                "--iterations" => arguments.iterations = parse_bounded_u32(flag, value, 1, 4)?,
                "--hz" => arguments.hertz = parse_f32(flag, value)?,
                "--damping" => arguments.damping = parse_f32(flag, value)?,
                "--static-scale" => arguments.static_scale = parse_f32(flag, value)?,
                "--lane" => arguments.lane = LaneSelection::parse(value)?,
                "--scenes" => {
                    if value.is_empty() {
                        return Err("--scenes requires at least one scene".to_owned());
                    }
                    let mut scenes = Vec::new();
                    for name in value.split(',') {
                        let scene = Scene::parse(name)?;
                        if scenes.contains(&scene) {
                            return Err(format!("scene {name} was requested more than once"));
                        }
                        scenes.push(scene);
                    }
                    arguments.scenes = scenes;
                }
                "--steps" => {
                    let steps = value
                        .parse::<u32>()
                        .map_err(|error| format!("invalid --steps {value}: {error}"))?;
                    if steps == 0 {
                        return Err("--steps must be positive".to_owned());
                    }
                    arguments.steps = Some(steps);
                }
                _ => return Err(format!("unknown argument {flag}")),
            }
            index += 2;
        }
        arguments.softness()?.validate().map_err(display)?;
        Ok(Some(arguments))
    }

    fn softness(&self) -> Result<ContactSoftnessPolicyV1, String> {
        Ok(ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite("--hz", self.hertz)?,
            damping_ratio: finite("--damping", self.damping)?,
            static_contact_hertz_multiplier: finite("--static-scale", self.static_scale)?,
        })
    }
}

#[derive(Clone, Copy)]
struct StructureResult {
    final_pass: bool,
    first_loss: Option<u32>,
}

struct ValidityResult {
    passed: bool,
    failure: Option<String>,
}

struct Row {
    scene: Scene,
    lane: Lane,
    structure: StructureResult,
    quiet_speed: Option<f32>,
    validity: ValidityResult,
    milliseconds_per_step: f64,
}

fn main() -> ExitCode {
    match Arguments::parse().and_then(|arguments| arguments.map(run).transpose()) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("physbox-check: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Arguments) -> Result<(), String> {
    let softness = arguments.softness()?;
    print_coefficients(&arguments, softness)?;
    println!(
        "{:<8} {:<9} {:<18} {:<18} {:<28} {:>10}",
        "SCENE", "LANE", "STANDS", "QUIET", "VALID", "COST"
    );

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut rows = Vec::new();
    for scene in arguments.scenes.iter().copied() {
        let steps = arguments.steps.unwrap_or_else(|| scene.default_steps());
        let base = scene.scenario(steps)?;
        for lane in arguments.lane.lanes().iter().copied() {
            let mut scenario = base.clone();
            if lane == Lane::Candidate {
                scenario.settings.fixed_dt = finite("--sim-hz", 1.0 / arguments.sim_hertz as f32)?;
                scenario.settings.substeps = arguments.substeps;
                scenario.settings.iterations = arguments.iterations;
                scenario.settings.contact_softness = Some(softness);
            }
            let row = run_row(&runner, scene, lane, &scenario)?;
            print_row(&row);
            rows.push(row);
        }
    }
    for lane in arguments.lane.lanes() {
        let lane_rows = rows.iter().filter(|row| row.lane == *lane);
        let qualified = lane_rows.clone().all(|row| {
            row.structure.final_pass
                && row.structure.first_loss.is_none()
                && row
                    .quiet_speed
                    .is_some_and(|speed| speed <= QUIET_SPEED_LIMIT)
                && row.validity.passed
        });
        println!(
            "QUALIFY {} {}",
            lane.name(),
            if qualified { "PASS" } else { "FAIL" }
        );
    }
    Ok(())
}

fn run_row(
    runner: &GpuGeneratedContactRunner,
    scene: Scene,
    lane: Lane,
    scenario: &ScenarioV1,
) -> Result<Row, String> {
    let started = Instant::now();
    let result = run_series(runner, lane, scenario, scenario.requested_steps);
    let elapsed = started.elapsed();
    let milliseconds_per_step = elapsed.as_secs_f64() * 1_000.0 / scenario.requested_steps as f64;

    match result {
        Ok(series) => {
            let structure = scene.stands(scenario, &series);
            let validity = series_validity(&series);
            let quiet_speed = validity
                .passed
                .then(|| last_quarter_maximum_linear_speed(&series));
            Ok(Row {
                scene,
                lane,
                structure,
                quiet_speed,
                validity,
                milliseconds_per_step,
            })
        }
        Err(error) => {
            eprintln!(
                "physbox-check {} {} terminal error: {}",
                scene.name(),
                lane.name(),
                error
            );
            let (first_invalid_step, prefix) = first_invalid_step(runner, lane, scenario, &error)?;
            let structure = prefix.as_ref().map_or(
                StructureResult {
                    final_pass: false,
                    first_loss: Some(first_invalid_step),
                },
                |series| {
                    let mut result = scene.stands(scenario, series);
                    result.final_pass = false;
                    result.first_loss = result.first_loss.or(Some(first_invalid_step));
                    result
                },
            );
            Ok(Row {
                scene,
                lane,
                structure,
                quiet_speed: None,
                validity: ValidityResult {
                    passed: false,
                    failure: Some(format!(
                        "{}@{}",
                        terminal_failure_kind(&error),
                        first_invalid_step
                    )),
                },
                milliseconds_per_step,
            })
        }
    }
}

fn run_series(
    runner: &GpuGeneratedContactRunner,
    lane: Lane,
    scenario: &ScenarioV1,
    completed_steps: u32,
) -> Result<GpuGeneratedContactSeriesV1, String> {
    match lane {
        Lane::Selected => runner.run_selected_observation_series_with_submission_batch(
            scenario,
            1,
            completed_steps,
            SUBMISSION_BATCH_STEPS,
        ),
        Lane::Candidate => runner
            .run_branch_robust_tunable_observation_series_with_submission_batch(
                scenario,
                1,
                completed_steps,
                SUBMISSION_BATCH_STEPS,
            ),
    }
}

fn first_invalid_step(
    runner: &GpuGeneratedContactRunner,
    lane: Lane,
    scenario: &ScenarioV1,
    terminal_error: &str,
) -> Result<(u32, Option<GpuGeneratedContactSeriesV1>), String> {
    let mut low = 1;
    let mut high = scenario.requested_steps;
    while low < high {
        let middle = low + (high - low) / 2;
        if run_series(runner, lane, scenario, middle).is_ok() {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if run_series(runner, lane, scenario, low).is_ok() {
        return Err(format!(
            "terminal failure did not reproduce under prefix search: {terminal_error}"
        ));
    }
    let prefix = if low > 1 {
        Some(run_series(runner, lane, scenario, low - 1)?)
    } else {
        None
    };
    Ok((low, prefix))
}

fn series_validity(series: &GpuGeneratedContactSeriesV1) -> ValidityResult {
    for snapshot in &series.snapshots {
        if let Some(failure) = &snapshot.failure {
            let step = failure.first_step.unwrap_or(snapshot.completed_steps);
            return ValidityResult {
                passed: false,
                failure: Some(format!("{}@{step}", failure_kind_name(&failure.kind))),
            };
        }
        if snapshot
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted || capacity.overflowed)
        {
            return ValidityResult {
                passed: false,
                failure: Some(format!("CapacityOverflow@{}", snapshot.completed_steps)),
            };
        }
    }
    ValidityResult {
        passed: true,
        failure: None,
    }
}

fn snapshot_capacity_clean(snapshot: &physbox_engine::GpuGeneratedContactSnapshotV1) -> bool {
    snapshot.failure.is_none()
        && snapshot
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
}

fn last_quarter_maximum_linear_speed(series: &GpuGeneratedContactSeriesV1) -> f32 {
    let first_step = series.completed_steps.saturating_mul(3) / 4;
    series
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.completed_steps >= first_step)
        .flat_map(|snapshot| snapshot.bodies.iter())
        .filter(|body| body.inverse_mass.get() > 0.0)
        .map(|body| body.linear_velocity.x.hypot(body.linear_velocity.y))
        .fold(0.0, f32::max)
}

fn print_coefficients(
    arguments: &Arguments,
    softness: ContactSoftnessPolicyV1,
) -> Result<(), String> {
    let fixed_dt = 1.0 / arguments.sim_hertz as f32;
    let substep_dt = fixed_dt / arguments.substeps as f32;
    let effective_dynamic_hertz = softness.contact_hertz.get().min(0.125 / substep_dt);
    let effective_static_hertz =
        effective_dynamic_hertz * softness.static_contact_hertz_multiplier.get();
    let dynamic = gpu_contact_softness_coefficients(softness, substep_dt, false)?;
    let static_contact = gpu_contact_softness_coefficients(softness, substep_dt, true)?;
    println!(
        "physbox-check sim-hz={} substeps={} iterations={}; requested hz={:.6} damping={:.6} static-scale={:.6}; effective dynamic-hz={:.6} static-hz={:.6}",
        arguments.sim_hertz,
        arguments.substeps,
        arguments.iterations,
        arguments.hertz,
        arguments.damping,
        arguments.static_scale,
        effective_dynamic_hertz,
        effective_static_hertz,
    );
    println!(
        "coefficients dynamic[bias={:.8} mass={:.8} impulse={:.8}] static[bias={:.8} mass={:.8} impulse={:.8}] quiet-limit={:.6}m/s",
        dynamic.bias_rate,
        dynamic.mass_scale,
        dynamic.impulse_scale,
        static_contact.bias_rate,
        static_contact.mass_scale,
        static_contact.impulse_scale,
        QUIET_SPEED_LIMIT,
    );
    Ok(())
}

fn print_row(row: &Row) {
    let stands = match (row.structure.final_pass, row.structure.first_loss) {
        (true, None) => "PASS".to_owned(),
        (_, Some(step)) => format!("FAIL@{step}"),
        _ => "FAIL".to_owned(),
    };
    let quiet = row.quiet_speed.map_or_else(
        || "N/A".to_owned(),
        |speed| {
            format!(
                "{} {:.6}",
                if speed <= QUIET_SPEED_LIMIT {
                    "PASS"
                } else {
                    "FAIL"
                },
                speed
            )
        },
    );
    let valid = if row.validity.passed {
        "PASS".to_owned()
    } else {
        format!(
            "FAIL {}",
            row.validity.failure.as_deref().unwrap_or("unknown")
        )
    };
    println!(
        "{:<8} {:<9} {:<18} {:<18} {:<28} {:>7.3}ms",
        row.scene.name(),
        row.lane.name(),
        stands,
        quiet,
        valid,
        row.milliseconds_per_step,
    );
}

fn terminal_failure_kind(error: &str) -> &'static str {
    if error.contains("cache is not sorted and unique")
        || error.contains("collision points must have sorted unique persistent keys")
    {
        "PersistenceMismatch"
    } else if error.contains("manifolds are not sorted and unique") {
        "ManifoldTopology"
    } else {
        "InvalidRequest"
    }
}

fn failure_kind_name(kind: &FailureKindV1) -> &'static str {
    match kind {
        FailureKindV1::UnsupportedFeature => "UnsupportedFeature",
        FailureKindV1::InvalidRequest => "InvalidRequest",
        FailureKindV1::Timeout => "Timeout",
        FailureKindV1::StaleResult => "StaleResult",
        FailureKindV1::HashMismatch => "HashMismatch",
        FailureKindV1::MissingSentinel => "MissingSentinel",
        FailureKindV1::Invariant => "Invariant",
        FailureKindV1::NumericalDivergence => "NumericalDivergence",
        FailureKindV1::CapacityOverflow => "CapacityOverflow",
        FailureKindV1::MissingPair => "MissingPair",
        FailureKindV1::DuplicatePair => "DuplicatePair",
        FailureKindV1::ManifoldTopology => "ManifoldTopology",
        FailureKindV1::StaleIdentity => "StaleIdentity",
        FailureKindV1::PersistenceMismatch => "PersistenceMismatch",
    }
}

fn parse_f32(flag: &str, value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|error| format!("invalid {flag} {value}: {error}"))?;
    if !parsed.is_finite() {
        return Err(format!("{flag} must be finite"));
    }
    Ok(parsed)
}

fn parse_bounded_u32(flag: &str, value: &str, minimum: u32, maximum: u32) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|error| format!("invalid {flag} {value}: {error}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{flag} must be in {minimum}..={maximum}"));
    }
    Ok(parsed)
}

fn finite(label: &str, value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| format!("invalid {label}: {error}"))
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn print_help() {
    println!(
        "physbox-check [--sim-hz N] [--substeps N] [--iterations N] \
         [--hz N] [--damping N] [--static-scale N] \
         [--lane both|selected|candidate] \
         [--scenes stack4,stack8,stack16,bucket] [--steps N]\n\n\
         Defaults: --sim-hz 60 --substeps 4 --iterations 1 --hz 30 \
         --damping 10 --static-scale 2 --lane both \
         --scenes stack4,stack8,stack16,bucket. Stacks run 1200 steps and the \
         rectangle bucket runs 600; --steps overrides every selected scene. \
         stack32 and stack64 are available as optional diagnostics."
    );
}
