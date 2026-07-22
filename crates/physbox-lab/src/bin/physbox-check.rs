use std::{env, process::ExitCode, time::Instant};

use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FailureKindV1, FiniteF32, ScenarioV1,
    Validate,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, P5_2BucketShape,
    P5_2GeneratedScenarioKind, gpu_contact_softness_coefficients, p5_2_bucket_scenario,
    p5_2_centered_rectangle_stack_scenario,
};
use physbox_lab::settling::{
    awake_settling_measurement_policy_v2, centered_stack_structure,
    observe_generated_settling_series_v2, summarize_settling_v2,
};

const DEFAULT_HERTZ: f32 = 30.0;
const DEFAULT_DAMPING: f32 = 10.0;
const DEFAULT_STATIC_SCALE: f32 = 2.0;
const DEFAULT_SIM_HERTZ: u32 = 60;
const DEFAULT_SUBSTEPS: u32 = 4;
const DEFAULT_ITERATIONS: u32 = 1;
const SUBMISSION_BATCH_STEPS: u32 = 64;
const QUIET_SPEED_LIMIT: f32 = 0.025;
const GEOMETRY_ABSOLUTE_BAND: f32 = 0.0002;
const CURRENT_12_SCREEN_BUDGET_SECONDS: f64 = 60.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    Current12,
}

impl Screen {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "current-12" => Ok(Self::Current12),
            _ => Err(format!("invalid --screen {value}; expected current-12")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ScreenConfiguration {
    id: &'static str,
    sim_hertz: u32,
    substeps: u32,
    iterations: u32,
    hertz: f32,
    damping: f32,
    static_scale: f32,
}

impl ScreenConfiguration {
    fn softness(self) -> Result<ContactSoftnessPolicyV1, String> {
        let softness = ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite("screen contact hertz", self.hertz)?,
            damping_ratio: finite("screen damping", self.damping)?,
            static_contact_hertz_multiplier: finite(
                "screen static-contact scale",
                self.static_scale,
            )?,
        };
        softness.validate().map_err(display)?;
        Ok(softness)
    }
}

const CURRENT_12_SCREEN: [ScreenConfiguration; 6] = [
    ScreenConfiguration {
        id: "baseline",
        sim_hertz: 60,
        substeps: 4,
        iterations: 1,
        hertz: 30.0,
        damping: 10.0,
        static_scale: 2.0,
    },
    ScreenConfiguration {
        id: "static-frontier",
        sim_hertz: 60,
        substeps: 4,
        iterations: 1,
        hertz: 30.0,
        damping: 10.0,
        static_scale: 8.0,
    },
    ScreenConfiguration {
        id: "damping-12",
        sim_hertz: 60,
        substeps: 4,
        iterations: 1,
        hertz: 30.0,
        damping: 12.0,
        static_scale: 2.0,
    },
    ScreenConfiguration {
        id: "damping-16",
        sim_hertz: 60,
        substeps: 4,
        iterations: 1,
        hertz: 30.0,
        damping: 16.0,
        static_scale: 2.0,
    },
    ScreenConfiguration {
        id: "damping-static",
        sim_hertz: 60,
        substeps: 4,
        iterations: 1,
        hertz: 30.0,
        damping: 12.0,
        static_scale: 8.0,
    },
    ScreenConfiguration {
        id: "resolution-control",
        sim_hertz: 60,
        substeps: 8,
        iterations: 1,
        hertz: 30.0,
        damping: 10.0,
        static_scale: 2.0,
    },
];

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
    BilateralImpact,
    OffCenter,
    Friction,
    Restitution,
    ShortStack,
}

impl Scene {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "stack2" => Ok(Self::Stack(2)),
            "stack4" => Ok(Self::Stack(4)),
            "stack8" => Ok(Self::Stack(8)),
            "stack12" => Ok(Self::Stack(12)),
            "stack14" => Ok(Self::Stack(14)),
            "stack16" => Ok(Self::Stack(16)),
            "stack32" => Ok(Self::Stack(32)),
            "stack64" => Ok(Self::Stack(64)),
            "bucket" => Ok(Self::Bucket),
            "impact" => Ok(Self::BilateralImpact),
            "off-center" => Ok(Self::OffCenter),
            "friction" => Ok(Self::Friction),
            "restitution" => Ok(Self::Restitution),
            "short-stack" => Ok(Self::ShortStack),
            _ => Err(format!(
                "invalid scene {value}; expected stack2, stack4, stack8, stack12, stack14, stack16, stack32, stack64, bucket, impact, off-center, friction, restitution, or short-stack"
            )),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Stack(2) => "stack2",
            Self::Stack(4) => "stack4",
            Self::Stack(8) => "stack8",
            Self::Stack(12) => "stack12",
            Self::Stack(14) => "stack14",
            Self::Stack(16) => "stack16",
            Self::Stack(32) => "stack32",
            Self::Stack(64) => "stack64",
            Self::Stack(_) => "stack",
            Self::Bucket => "bucket",
            Self::BilateralImpact => "impact",
            Self::OffCenter => "off-center",
            Self::Friction => "friction",
            Self::Restitution => "restitution",
            Self::ShortStack => "short-stack",
        }
    }

    const fn default_steps(self) -> u32 {
        match self {
            Self::Stack(_) => 1_200,
            Self::Bucket => 600,
            Self::BilateralImpact => 1,
            Self::OffCenter => 4,
            Self::Friction => 12,
            Self::Restitution => 8,
            Self::ShortStack => 12,
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
            Self::BilateralImpact => generated(P5_2GeneratedScenarioKind::BilateralCircles, steps),
            Self::OffCenter => generated(P5_2GeneratedScenarioKind::OffCenterRectangles, steps),
            Self::Friction => generated(P5_2GeneratedScenarioKind::FrictionSlide, steps),
            Self::Restitution => generated(P5_2GeneratedScenarioKind::Restitution, steps),
            Self::ShortStack => generated(P5_2GeneratedScenarioKind::SmallStack, steps),
        }
    }

    fn stands(
        self,
        scenario: &ScenarioV1,
        series: &GpuGeneratedContactSeriesV1,
    ) -> StructureResult {
        if !matches!(self, Self::Stack(_) | Self::Bucket) {
            let Some(snapshot) = series.snapshots.last() else {
                return StructureResult {
                    final_pass: false,
                    first_loss: Some(0),
                };
            };
            let bodies = &snapshot.bodies;
            let final_pass = snapshot_capacity_clean(snapshot)
                && match self {
                    Self::BilateralImpact => {
                        bodies[0].linear_velocity.x < 1.0 && bodies[1].linear_velocity.x > -1.0
                    }
                    Self::OffCenter => bodies
                        .iter()
                        .any(|body| body.angular_velocity.get().abs() > 1.0e-3),
                    Self::Friction => bodies[0].linear_velocity.x.abs() < 2.0,
                    Self::Restitution => bodies[0].linear_velocity.y > 0.0,
                    Self::ShortStack => bodies[1..]
                        .windows(2)
                        .all(|pair| pair[0].position.y < pair[1].position.y),
                    Self::Stack(_) | Self::Bucket => unreachable!(),
                };
            return StructureResult {
                final_pass,
                first_loss: (!final_pass).then_some(snapshot.completed_steps),
            };
        }

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
                Self::BilateralImpact
                | Self::OffCenter
                | Self::Friction
                | Self::Restitution
                | Self::ShortStack => unreachable!(),
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

    const fn requires_quiet(self) -> bool {
        matches!(self, Self::Stack(_) | Self::Bucket)
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
    screen: Option<Screen>,
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
            screen: None,
        }
    }
}

impl Arguments {
    fn parse() -> Result<Option<Self>, String> {
        let mut arguments = Self::default();
        let values = env::args().skip(1).collect::<Vec<_>>();
        if values.first().is_some_and(|value| value == "--screen") {
            if values.len() != 2 {
                return Err(
                    "--screen is a named frozen matrix and cannot be combined with other flags"
                        .to_owned(),
                );
            }
            arguments.screen = Some(Screen::parse(&values[1])?);
            for configuration in CURRENT_12_SCREEN {
                configuration.softness()?;
            }
            return Ok(Some(arguments));
        }
        if values.iter().any(|value| value == "--screen") {
            return Err("--screen must be the first and only flag".to_owned());
        }
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

#[derive(Clone, Copy, Debug)]
struct StackSettlingMetrics {
    linear_speed_max: f32,
    angular_edge_speed_max: f32,
    penetration_max: f32,
    final_stack_height: f32,
    center_drift_max: f32,
    first_energy_quartile_max: f32,
    last_energy_quartile_max: f32,
    positive_energy_run_max: u32,
    energy_envelope_nonincreasing: bool,
    time_to_rest_step: Option<u32>,
}

struct Row {
    scene: Scene,
    lane: Lane,
    structure: StructureResult,
    quiet_speed: Option<f32>,
    stack_settling: Option<StackSettlingMetrics>,
    validity: ValidityResult,
    milliseconds_per_step: f64,
}

struct ScreenRow {
    configuration: ScreenConfiguration,
    row: Row,
    geometry_pass: bool,
    survivor: bool,
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
    if arguments.screen == Some(Screen::Current12) {
        return run_current_12_screen();
    }
    let softness = arguments.softness()?;
    print_coefficients(&arguments, softness)?;
    println!(
        "{:<8} {:<9} {:<18} {:<18} {:<28} {:>10}",
        "SCENE", "LANE", "STANDS", "QUIET", "VALID", "COST"
    );

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    println!("adapter {}", runner.diagnostics());
    println!(
        "lane mapping: selected=historical hard/direct control; candidate=tunable soft recurrence (default tuple is the current soft-coupled product)"
    );
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
                && (!row.scene.requires_quiet()
                    || row
                        .quiet_speed
                        .is_some_and(|speed| speed <= QUIET_SPEED_LIMIT))
                && row
                    .stack_settling
                    .is_none_or(|settling| settling.angular_edge_speed_max <= QUIET_SPEED_LIMIT)
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

fn run_current_12_screen() -> Result<(), String> {
    let screen_started = Instant::now();
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    println!("physbox-check screen=current-12 steps=1200 sleeping=off");
    println!("adapter {}", runner.diagnostics());
    println!(
        "screen contract: linear<=0.025000m/s angular-edge<=0.025000m/s; geometry relative to baseline penetration<=+0.000200m height>=-0.000200m"
    );
    let mut warmup = Scene::Stack(12).scenario(1)?;
    apply_screen_configuration(&mut warmup, CURRENT_12_SCREEN[0])?;
    run_series(&runner, Lane::Candidate, &warmup, 1)?;
    println!("screen warmup: one baseline step excluded from rows and row timings");
    let base = Scene::Stack(12).scenario(1_200)?;
    let mut raw_rows = Vec::with_capacity(CURRENT_12_SCREEN.len());
    for configuration in CURRENT_12_SCREEN {
        print_screen_configuration(configuration)?;
        let mut scenario = base.clone();
        apply_screen_configuration(&mut scenario, configuration)?;
        raw_rows.push((
            configuration,
            run_row(&runner, Scene::Stack(12), Lane::Candidate, &scenario)?,
        ));
    }

    let baseline = raw_rows
        .first()
        .and_then(|(_, row)| row.stack_settling)
        .ok_or("current-12 screen baseline has no settling metrics")?;
    let rows = raw_rows
        .into_iter()
        .map(|(configuration, row)| {
            let settling = row
                .stack_settling
                .ok_or_else(|| format!("{} has no settling metrics", configuration.id))?;
            let geometry_pass = settling.penetration_max
                <= baseline.penetration_max + GEOMETRY_ABSOLUTE_BAND
                && settling.final_stack_height + GEOMETRY_ABSOLUTE_BAND
                    >= baseline.final_stack_height;
            let survivor = screen_quality_pass(&row) && geometry_pass;
            Ok(ScreenRow {
                configuration,
                row,
                geometry_pass,
                survivor,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut advance = rows.iter().filter(|row| row.survivor).collect::<Vec<_>>();
    advance.sort_by(|left, right| {
        let left_settling = left.row.stack_settling.unwrap();
        let right_settling = right.row.stack_settling.unwrap();
        left_settling
            .linear_speed_max
            .max(left_settling.angular_edge_speed_max)
            .total_cmp(
                &right_settling
                    .linear_speed_max
                    .max(right_settling.angular_edge_speed_max),
            )
            .then_with(|| {
                left.row
                    .milliseconds_per_step
                    .total_cmp(&right.row.milliseconds_per_step)
            })
    });
    advance.truncate(2);
    let advance_ids = advance
        .iter()
        .map(|row| row.configuration.id)
        .collect::<Vec<_>>();

    println!(
        "{:<19} {:>2} {:>5} {:>6} {:<10} {:>9} {:>9} {:>9} {:>9} {:>9} {:>6} {:<7} {:>8} {:<8} RESULT",
        "CONFIG",
        "SS",
        "DAMP",
        "STATIC",
        "STANDS",
        "LINEAR",
        "ANG-EDGE",
        "PEN",
        "HEIGHT",
        "DRIFT",
        "ENERGY",
        "VALID",
        "COST",
        "GEOM",
    );
    for screen_row in &rows {
        let settling = screen_row.row.stack_settling.unwrap();
        let result = if advance_ids.contains(&screen_row.configuration.id) {
            "ADVANCE"
        } else if screen_row.survivor {
            "SURVIVOR-CAPPED"
        } else if !screen_quality_pass(&screen_row.row) {
            "REJECT-QUALITY"
        } else {
            "REJECT-GEOMETRY"
        };
        println!(
            "{:<19} {:>2} {:>5.1} {:>6.1} {:<10} {:>9.6} {:>9.6} {:>9.6} {:>9.6} {:>9.6} {:>6} {:<7} {:>7.3}ms {:<8} {}",
            screen_row.configuration.id,
            screen_row.configuration.substeps,
            screen_row.configuration.damping,
            screen_row.configuration.static_scale,
            structure_label(screen_row.row.structure),
            settling.linear_speed_max,
            settling.angular_edge_speed_max,
            settling.penetration_max,
            settling.final_stack_height,
            settling.center_drift_max,
            if settling.energy_envelope_nonincreasing {
                "PASS"
            } else {
                "FAIL"
            },
            validity_label(&screen_row.row.validity),
            screen_row.row.milliseconds_per_step,
            if screen_row.geometry_pass {
                "PASS"
            } else {
                "FAIL"
            },
            result,
        );
    }

    let elapsed = screen_started.elapsed().as_secs_f64();
    println!(
        "SCREEN current-12 survivors={} advance={} elapsed={:.3}s budget={}<=60.000s",
        rows.iter().filter(|row| row.survivor).count(),
        if advance_ids.is_empty() {
            "none".to_owned()
        } else {
            advance_ids.join(",")
        },
        elapsed,
        if elapsed <= CURRENT_12_SCREEN_BUDGET_SECONDS {
            "PASS"
        } else {
            "FAIL"
        },
    );
    if elapsed > CURRENT_12_SCREEN_BUDGET_SECONDS {
        return Err(format!(
            "current-12 screen exceeded its {:.3}s budget: {:.3}s",
            CURRENT_12_SCREEN_BUDGET_SECONDS, elapsed
        ));
    }
    Ok(())
}

fn apply_screen_configuration(
    scenario: &mut ScenarioV1,
    configuration: ScreenConfiguration,
) -> Result<(), String> {
    scenario.settings.fixed_dt = finite(
        "screen simulation hertz",
        1.0 / configuration.sim_hertz as f32,
    )?;
    scenario.settings.substeps = configuration.substeps;
    scenario.settings.iterations = configuration.iterations;
    scenario.settings.contact_softness = Some(configuration.softness()?);
    Ok(())
}

fn screen_quality_pass(row: &Row) -> bool {
    let Some(settling) = row.stack_settling else {
        return false;
    };
    row.structure.final_pass
        && row.structure.first_loss.is_none()
        && row.validity.passed
        && settling.linear_speed_max <= QUIET_SPEED_LIMIT
        && settling.angular_edge_speed_max <= QUIET_SPEED_LIMIT
}

fn print_screen_configuration(configuration: ScreenConfiguration) -> Result<(), String> {
    let softness = configuration.softness()?;
    let fixed_dt = 1.0 / configuration.sim_hertz as f32;
    let substep_dt = fixed_dt / configuration.substeps as f32;
    let effective_dynamic_hertz = softness.contact_hertz.get().min(0.125 / substep_dt);
    let effective_static_hertz =
        effective_dynamic_hertz * softness.static_contact_hertz_multiplier.get();
    let dynamic = gpu_contact_softness_coefficients(softness, substep_dt, false)?;
    let static_contact = gpu_contact_softness_coefficients(softness, substep_dt, true)?;
    println!(
        "SCREEN_CONFIG id={} sim-hz={} substeps={} iterations={} requested={:.3}/{:.3}/{:.3} effective-hz={:.3}/{:.3} dynamic={:.8}/{:.8}/{:.8} static={:.8}/{:.8}/{:.8}",
        configuration.id,
        configuration.sim_hertz,
        configuration.substeps,
        configuration.iterations,
        configuration.hertz,
        configuration.damping,
        configuration.static_scale,
        effective_dynamic_hertz,
        effective_static_hertz,
        dynamic.bias_rate,
        dynamic.mass_scale,
        dynamic.impulse_scale,
        static_contact.bias_rate,
        static_contact.mass_scale,
        static_contact.impulse_scale,
    );
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
            let stack_settling = if validity.passed
                && matches!(scene, Scene::Stack(_))
                && series.completed_steps == 1_200
                && series.observation_interval_steps == 1
            {
                Some(stack_settling_metrics(scenario, &series)?)
            } else {
                None
            };
            let quiet_speed = if let Some(settling) = stack_settling {
                Some(settling.linear_speed_max)
            } else {
                (validity.passed && scene.requires_quiet())
                    .then(|| last_quarter_maximum_linear_speed(&series))
            };
            Ok(Row {
                scene,
                lane,
                structure,
                quiet_speed,
                stack_settling,
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
                stack_settling: None,
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

fn stack_settling_metrics(
    scenario: &ScenarioV1,
    series: &GpuGeneratedContactSeriesV1,
) -> Result<StackSettlingMetrics, String> {
    let policy = awake_settling_measurement_policy_v2();
    let observations = observe_generated_settling_series_v2(scenario, series)?;
    let summary = summarize_settling_v2(&observations, policy)?;
    let settled = observations
        .iter()
        .filter(|observation| {
            (policy.settled_start_step..=policy.settled_end_step)
                .contains(&observation.base.completed_steps)
        })
        .collect::<Vec<_>>();
    let final_observation = observations
        .last()
        .ok_or("stack settling series has no observations")?;
    Ok(StackSettlingMetrics {
        linear_speed_max: summary.settled_linear_speed_max.get(),
        angular_edge_speed_max: summary.settled_angular_edge_speed_max.get(),
        penetration_max: settled
            .iter()
            .map(|observation| observation.base.penetration_max.get())
            .fold(0.0, f32::max),
        final_stack_height: final_observation.base.stack_height.get(),
        center_drift_max: settled
            .iter()
            .map(|observation| observation.base.center_drift_max.get())
            .fold(0.0, f32::max),
        first_energy_quartile_max: summary.first_energy_quartile_max.get(),
        last_energy_quartile_max: summary.last_energy_quartile_max.get(),
        positive_energy_run_max: summary.positive_energy_run_max,
        energy_envelope_nonincreasing: summary.energy_envelope_nonincreasing,
        time_to_rest_step: summary.time_to_rest_step,
    })
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
    let stands = structure_label(row.structure);
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
    let valid = validity_label(&row.validity);
    println!(
        "{:<8} {:<9} {:<18} {:<18} {:<28} {:>7.3}ms",
        row.scene.name(),
        row.lane.name(),
        stands,
        quiet,
        valid,
        row.milliseconds_per_step,
    );
    if let Some(settling) = row.stack_settling {
        println!(
            "  SETTLING angular-edge={:.6}m/s penetration={:.6}m final-height={:.6}m drift={:.6}m energy={} energy-first={:.9}J energy-last={:.9}J positive-run={} rest-step={}",
            settling.angular_edge_speed_max,
            settling.penetration_max,
            settling.final_stack_height,
            settling.center_drift_max,
            if settling.energy_envelope_nonincreasing {
                "PASS"
            } else {
                "FAIL"
            },
            settling.first_energy_quartile_max,
            settling.last_energy_quartile_max,
            settling.positive_energy_run_max,
            settling
                .time_to_rest_step
                .map_or_else(|| "none".to_owned(), |step| step.to_string()),
        );
    }
}

fn structure_label(structure: StructureResult) -> String {
    match (structure.final_pass, structure.first_loss) {
        (true, None) => "PASS".to_owned(),
        (_, Some(step)) => format!("FAIL@{step}"),
        _ => "FAIL".to_owned(),
    }
}

fn validity_label(validity: &ValidityResult) -> String {
    if validity.passed {
        "PASS".to_owned()
    } else {
        format!("FAIL {}", validity.failure.as_deref().unwrap_or("unknown"))
    }
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

fn generated(kind: P5_2GeneratedScenarioKind, steps: u32) -> Result<ScenarioV1, String> {
    physbox_engine::p5_2_generated_scenario(kind, steps).map_err(display)
}

fn print_help() {
    println!(
        "physbox-check --screen current-12\n\
         physbox-check [--sim-hz N] [--substeps N] [--iterations N] \
         [--hz N] [--damping N] [--static-scale N] \
         [--lane both|selected|candidate] \
         [--scenes stack2,stack4,stack8,stack12,stack14,stack16,bucket,impact,off-center,friction,restitution,short-stack] [--steps N]\n\n\
         Defaults: --sim-hz 60 --substeps 4 --iterations 1 --hz 30 \
         --damping 10 --static-scale 2 --lane both \
         --scenes stack4,stack8,stack16,bucket. Stacks run 1200 steps and the \
         rectangle bucket runs 600; --steps overrides every selected scene. \
         stack2/12/14 provide the current quality hierarchy; stack32, stack64, \
         and the five response probes are optional diagnostics. The named \
         current-12 screen is a frozen six-configuration, one-runner matrix \
         and cannot be combined with ad-hoc flags."
    );
}
