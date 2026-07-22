#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::{collections::BTreeSet, str::FromStr};

use serde::Serialize;

pub const ANDROID_STARTUP_ARGV_INTENT_EXTRA: &str = "physbox5.startup.argv";

pub const DEFAULT_WIDTH: u32 = 960;
pub const DEFAULT_HEIGHT: u32 = 600;
pub const DEFAULT_CAPTURE_FRAMES: u32 = 12;
pub const DEFAULT_CAPTURE_FPS: u32 = 60;
pub const PHYSICS_CONFIGURATION_VERSION: u32 = 2;
pub const MIN_FIXED_STEP_HZ: u32 = 15;
pub const MAX_FIXED_STEP_HZ: u32 = 240;
pub const MIN_SOLVER_SUBSTEPS: u32 = 1;
pub const MAX_SOLVER_SUBSTEPS: u32 = 8;
pub const MIN_BIASED_ITERATIONS: u32 = 1;
pub const MAX_BIASED_ITERATIONS: u32 = 4;
pub const MIN_CONTACT_HZ: f32 = 1.0;
pub const MAX_CONTACT_HZ: f32 = 120.0;
pub const MIN_CONTACT_DAMPING_RATIO: f32 = 0.5;
pub const MAX_CONTACT_DAMPING_RATIO: f32 = 20.0;
pub const MIN_STATIC_CONTACT_HZ_MULTIPLIER: f32 = 0.5;
pub const MAX_STATIC_CONTACT_HZ_MULTIPLIER: f32 = 16.0;
pub const MIN_SPECULATIVE_DISTANCE: f32 = 0.0;
pub const MAX_SPECULATIVE_DISTANCE: f32 = 1.0;
pub const MIN_MAX_DEPENETRATION_VELOCITY: f32 = 0.1;
pub const MAX_MAX_DEPENETRATION_VELOCITY: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorldCapacityPreset {
    #[default]
    Slots64,
    Slots256,
    Slots1000,
    Slots10000,
}

impl WorldCapacityPreset {
    pub const ALL: [Self; 4] = [
        Self::Slots64,
        Self::Slots256,
        Self::Slots1000,
        Self::Slots10000,
    ];

    pub const fn slots(self) -> u32 {
        match self {
            Self::Slots64 => 64,
            Self::Slots256 => 256,
            Self::Slots1000 => 1_000,
            Self::Slots10000 => 10_000,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Slots64 => "64 SLOTS",
            Self::Slots256 => "256 SLOTS",
            Self::Slots1000 => "1K SLOTS",
            Self::Slots10000 => "10K SLOTS",
        }
    }

    fn parse(value: &str) -> Result<Self, LaunchError> {
        match value {
            "64" => Ok(Self::Slots64),
            "256" => Ok(Self::Slots256),
            "1000" | "1k" => Ok(Self::Slots1000),
            "10000" | "10k" => Ok(Self::Slots10000),
            _ => Err(LaunchError(format!(
                "--body-capacity must be one of 64, 256, 1000, or 10000; received {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchHost {
    Desktop,
    Web,
    Headless,
    Android,
}

impl LaunchHost {
    const fn supports_capture(self) -> bool {
        matches!(self, Self::Headless)
    }

    const fn supports_frame_markers(self) -> bool {
        matches!(self, Self::Desktop | Self::Android)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FrameMarkerMode {
    Off,
    #[default]
    Milestones,
    All,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SolverProfileName {
    HardDirect,
    #[default]
    SoftCoupled,
}

impl SolverProfileName {
    pub const fn name(self) -> &'static str {
        match self {
            Self::HardDirect => "hard-direct",
            Self::SoftCoupled => "soft-coupled",
        }
    }

    fn parse(value: &str) -> Result<Self, LaunchError> {
        match value {
            "hard-direct" | "selected" => Ok(Self::HardDirect),
            "soft-coupled" | "branch-robust" => Ok(Self::SoftCoupled),
            _ => Err(LaunchError(format!("unknown solver profile: {value}"))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsConfigurationV2 {
    pub version: u32,
    pub solver_profile: SolverProfileName,
    pub fixed_step_hz: u32,
    pub substeps: u32,
    pub biased_iterations: u32,
    pub contact_hertz: f32,
    pub contact_damping_ratio: f32,
    pub static_contact_hertz_multiplier: f32,
    pub speculative_distance: f32,
    pub max_depenetration_velocity: f32,
    pub sleeping_enabled: bool,
}

impl PhysicsConfigurationV2 {
    pub const HARD_DIRECT: Self = Self {
        version: PHYSICS_CONFIGURATION_VERSION,
        solver_profile: SolverProfileName::HardDirect,
        fixed_step_hz: 60,
        substeps: 4,
        biased_iterations: 2,
        contact_hertz: 30.0,
        contact_damping_ratio: 10.0,
        static_contact_hertz_multiplier: 2.0,
        speculative_distance: 0.02,
        max_depenetration_velocity: 3.0,
        sleeping_enabled: false,
    };

    pub const SOFT_COUPLED: Self = Self {
        solver_profile: SolverProfileName::SoftCoupled,
        biased_iterations: 1,
        ..Self::HARD_DIRECT
    };

    pub const fn for_profile(profile: SolverProfileName) -> Self {
        match profile {
            SolverProfileName::HardDirect => Self::HARD_DIRECT,
            SolverProfileName::SoftCoupled => Self::SOFT_COUPLED,
        }
    }

    pub fn fixed_dt_seconds(self) -> f32 {
        1.0 / self.fixed_step_hz as f32
    }

    pub fn validate(self) -> Result<(), LaunchError> {
        if self.version != PHYSICS_CONFIGURATION_VERSION {
            return Err(LaunchError(format!(
                "unsupported physics configuration version {}, expected {PHYSICS_CONFIGURATION_VERSION}",
                self.version
            )));
        }
        bounded_integer(
            "--sim-hz",
            self.fixed_step_hz,
            MIN_FIXED_STEP_HZ,
            MAX_FIXED_STEP_HZ,
        )?;
        bounded_integer(
            "--substeps",
            self.substeps,
            MIN_SOLVER_SUBSTEPS,
            MAX_SOLVER_SUBSTEPS,
        )?;
        bounded_integer(
            "--iterations",
            self.biased_iterations,
            MIN_BIASED_ITERATIONS,
            MAX_BIASED_ITERATIONS,
        )?;
        bounded_float(
            "--contact-hz",
            self.contact_hertz,
            MIN_CONTACT_HZ,
            MAX_CONTACT_HZ,
        )?;
        bounded_float(
            "--contact-damping",
            self.contact_damping_ratio,
            MIN_CONTACT_DAMPING_RATIO,
            MAX_CONTACT_DAMPING_RATIO,
        )?;
        bounded_float(
            "--static-contact-scale",
            self.static_contact_hertz_multiplier,
            MIN_STATIC_CONTACT_HZ_MULTIPLIER,
            MAX_STATIC_CONTACT_HZ_MULTIPLIER,
        )?;
        bounded_float(
            "--speculative-distance",
            self.speculative_distance,
            MIN_SPECULATIVE_DISTANCE,
            MAX_SPECULATIVE_DISTANCE,
        )?;
        bounded_float(
            "--max-depenetration-velocity",
            self.max_depenetration_velocity,
            MIN_MAX_DEPENETRATION_VELOCITY,
            MAX_MAX_DEPENETRATION_VELOCITY,
        )?;
        Ok(())
    }
}

impl Default for PhysicsConfigurationV2 {
    fn default() -> Self {
        Self::SOFT_COUPLED
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValidationScenario {
    #[default]
    Bootstrap,
    BodyFreeFlight,
    RigidFloor,
    RigidStack,
    RectangleStack,
    CircleBucket100,
    RectangleBucket100,
    ResetGrab,
    SolverJacobi,
    SolverColored,
    SolverManifoldI2,
    SolverParallelColoring,
    SettlingHard8,
    SettlingCandidate8,
    SettlingHard16,
    SettlingIdentity16,
    SettlingSoftness16,
    SettlingCandidate16,
    #[cfg(feature = "solver-lab")]
    SolverLabStack2,
    #[cfg(feature = "solver-lab")]
    SolverLabStack4,
    #[cfg(feature = "solver-lab")]
    SolverLabStack8,
    #[cfg(feature = "solver-lab")]
    SolverLabStack12,
    #[cfg(feature = "solver-lab")]
    SolverLabStack14,
    #[cfg(feature = "solver-lab")]
    SolverLabStack16,
}

impl ValidationScenario {
    pub const ALL: &'static [Self] = &[
        Self::Bootstrap,
        Self::RigidFloor,
        Self::RigidStack,
        Self::RectangleStack,
        Self::CircleBucket100,
        Self::RectangleBucket100,
        Self::BodyFreeFlight,
        Self::ResetGrab,
        Self::SolverJacobi,
        Self::SolverColored,
        Self::SolverManifoldI2,
        Self::SolverParallelColoring,
        Self::SettlingHard8,
        Self::SettlingCandidate8,
        Self::SettlingHard16,
        Self::SettlingIdentity16,
        Self::SettlingSoftness16,
        Self::SettlingCandidate16,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack2,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack4,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack8,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack12,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack14,
        #[cfg(feature = "solver-lab")]
        Self::SolverLabStack16,
    ];

    pub const MENU: &'static [Self] = &[
        Self::Bootstrap,
        Self::RigidFloor,
        Self::RigidStack,
        Self::RectangleStack,
        Self::CircleBucket100,
        Self::RectangleBucket100,
        Self::BodyFreeFlight,
        Self::ResetGrab,
        Self::SolverJacobi,
        Self::SolverColored,
        Self::SolverManifoldI2,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::BodyFreeFlight => "body-free-flight",
            Self::RigidFloor => "rigid-floor",
            Self::RigidStack => "rigid-stack",
            Self::RectangleStack => "rectangle-stack",
            Self::CircleBucket100 => "circle-bucket-100",
            Self::RectangleBucket100 => "rectangle-bucket-100",
            Self::ResetGrab => "reset-grab",
            Self::SolverJacobi => "solver-jacobi",
            Self::SolverColored => "solver-colored",
            Self::SolverManifoldI2 => "solver-manifold-i2",
            Self::SolverParallelColoring => "solver-parallel-coloring",
            Self::SettlingHard8 => "settling-hard-8",
            Self::SettlingCandidate8 => "settling-candidate-8",
            Self::SettlingHard16 => "settling-hard-16",
            Self::SettlingIdentity16 => "settling-identity-16",
            Self::SettlingSoftness16 => "settling-softness-16",
            Self::SettlingCandidate16 => "settling-candidate-16",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack2 => "solver-lab-stack-2",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack4 => "solver-lab-stack-4",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack8 => "solver-lab-stack-8",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack12 => "solver-lab-stack-12",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack14 => "solver-lab-stack-14",
            #[cfg(feature = "solver-lab")]
            Self::SolverLabStack16 => "solver-lab-stack-16",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchOptions {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub fps: u32,
    pub steps: Option<u32>,
    pub output: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub scenario: ValidationScenario,
    pub world_capacity: WorldCapacityPreset,
    pub physics: PhysicsConfigurationV2,
    pub frame_markers: FrameMarkerMode,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            frames: DEFAULT_CAPTURE_FRAMES,
            fps: DEFAULT_CAPTURE_FPS,
            steps: None,
            output: None,
            report: None,
            scenario: ValidationScenario::Bootstrap,
            world_capacity: WorldCapacityPreset::default(),
            physics: PhysicsConfigurationV2::default(),
            frame_markers: FrameMarkerMode::Milestones,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchRequest {
    pub options: LaunchOptions,
    pub help_requested: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchError(String);

impl Display for LaunchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LaunchError {}

pub fn parse_os_args<I>(host: LaunchHost, args: I) -> Result<LaunchRequest, LaunchError>
where
    I: IntoIterator<Item = OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| LaunchError("arguments must be valid UTF-8".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    parse_tokens(host, args.into_iter().skip(1))
}

pub fn parse_tokens<I, S>(host: LaunchHost, args: I) -> Result<LaunchRequest, LaunchError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let solver_profile = preparse_solver_profile(&args)?.unwrap_or_default();
    let mut options = LaunchOptions {
        physics: PhysicsConfigurationV2::for_profile(solver_profile),
        ..LaunchOptions::default()
    };
    let mut help_requested = false;
    let mut seen_physics_fields = BTreeSet::new();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            help_requested = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| LaunchError(format!("missing value for {argument}")))?;
        apply_option(
            host,
            &mut options,
            &mut seen_physics_fields,
            &argument,
            value,
        )?;
    }
    validate(host, &options)?;
    Ok(LaunchRequest {
        options,
        help_requested,
    })
}

pub fn parse_web_query(query: &str) -> Result<LaunchRequest, LaunchError> {
    let mut tokens = Vec::new();
    for (key, value) in form_urlencoded::parse(query.trim_start_matches('?').as_bytes()) {
        let argument = match key.as_ref() {
            "width" => "--width",
            "height" => "--height",
            "scenario" => "--scenario",
            "body-capacity" => "--body-capacity",
            "solver" => "--solver",
            "solver-options" => "--solver-options",
            "sim-hz" => "--sim-hz",
            "substeps" => "--substeps",
            "iterations" => "--iterations",
            "contact-hz" => "--contact-hz",
            "contact-damping" => "--contact-damping",
            "static-contact-scale" => "--static-contact-scale",
            "speculative-distance" => "--speculative-distance",
            "max-depenetration-velocity" => "--max-depenetration-velocity",
            "sleeping" => "--sleeping",
            unknown => return Err(LaunchError(format!("unknown web query option: {unknown}"))),
        };
        tokens.push(argument.to_owned());
        tokens.push(value.into_owned());
    }
    parse_tokens(LaunchHost::Web, tokens)
}

pub fn parse_android_json(json: &str) -> Result<LaunchRequest, LaunchError> {
    let tokens: Vec<String> = serde_json::from_str(json)
        .map_err(|error| LaunchError(format!("invalid Android startup argv JSON: {error}")))?;
    parse_tokens(LaunchHost::Android, tokens)
}

pub fn usage(program: &str, host: LaunchHost) -> String {
    let capture = if host.supports_capture() {
        "\n  --frames N       wall-paced rendered frames (default 12)\n  --fps N          wall-paced capture cadence (default 60)\n  --steps N        exact fixed steps in one rendered frame\n  --output PATH    required PNG output\n  --report PATH    required JSON provenance"
    } else {
        ""
    };
    let frame_markers = if host.supports_frame_markers() {
        "\n  --frame-markers MODE  off, milestones (default), or all"
    } else {
        ""
    };
    let solver = "\n  --body-capacity N  interactive slots: 64 (default), 256, 1000, or 10000\n  --solver NAME    soft-coupled or hard-direct (aliases: branch-robust, selected)\n  --sim-hz N       outer fixed rate, 15..=240 (default 60)\n  --substeps N     solver substeps, 1..=8 (default 4)\n  --iterations N   biased iterations, 1..=4 (soft default 1, hard default 2)\n  --contact-hz N   requested soft contact hertz, 1..=120\n  --contact-damping N  soft damping ratio, 0.5..=20\n  --static-contact-scale N  static contact hertz multiplier, 0.5..=16\n  --speculative-distance N  contact anticipation distance, 0..=1\n  --max-depenetration-velocity N  overlap recovery speed, 0.1..=100\n  --sleeping MODE  experimental sleeping: off (default) or on";
    format!(
        "Usage: {program} [options]\n\n  --width PX       viewport width (default 960)\n  --height PX      viewport height (default 600)\n  --scenario NAME  bootstrap, body-free-flight, rigid-floor, rigid-stack, rectangle-stack, circle-bucket-100, rectangle-bucket-100, reset-grab, solver-jacobi, solver-colored, solver-manifold-i2, solver-parallel-coloring, settling-hard-8, settling-candidate-8, settling-hard-16, settling-identity-16, settling-softness-16, settling-candidate-16{}{solver}{frame_markers}{capture}\n  -h, --help       print this help",
        if cfg!(feature = "solver-lab") {
            ", solver-lab-stack-2, solver-lab-stack-4, solver-lab-stack-8, solver-lab-stack-12, solver-lab-stack-14, or solver-lab-stack-16"
        } else {
            ""
        }
    )
}

fn apply_option(
    host: LaunchHost,
    options: &mut LaunchOptions,
    seen_physics_fields: &mut BTreeSet<&'static str>,
    argument: &str,
    value: String,
) -> Result<(), LaunchError> {
    match argument {
        "--width" => options.width = parse_positive(argument, &value)?,
        "--height" => options.height = parse_positive(argument, &value)?,
        "--scenario" => {
            options.scenario = match value.as_str() {
                "bootstrap" => ValidationScenario::Bootstrap,
                "body-free-flight" => ValidationScenario::BodyFreeFlight,
                "rigid-floor" => ValidationScenario::RigidFloor,
                "rigid-stack" => ValidationScenario::RigidStack,
                "rectangle-stack" => ValidationScenario::RectangleStack,
                "circle-bucket-100" => ValidationScenario::CircleBucket100,
                "rectangle-bucket-100" => ValidationScenario::RectangleBucket100,
                "reset-grab" => ValidationScenario::ResetGrab,
                "solver-jacobi" => ValidationScenario::SolverJacobi,
                "solver-colored" => ValidationScenario::SolverColored,
                "solver-manifold-i2" => ValidationScenario::SolverManifoldI2,
                "solver-parallel-coloring" => ValidationScenario::SolverParallelColoring,
                "settling-hard-8" => ValidationScenario::SettlingHard8,
                "settling-candidate-8" => ValidationScenario::SettlingCandidate8,
                "settling-hard-16" => ValidationScenario::SettlingHard16,
                "settling-identity-16" => ValidationScenario::SettlingIdentity16,
                "settling-softness-16" => ValidationScenario::SettlingSoftness16,
                "settling-candidate-16" => ValidationScenario::SettlingCandidate16,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-2" => ValidationScenario::SolverLabStack2,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-4" => ValidationScenario::SolverLabStack4,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-8" => ValidationScenario::SolverLabStack8,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-12" => ValidationScenario::SolverLabStack12,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-14" => ValidationScenario::SolverLabStack14,
                #[cfg(feature = "solver-lab")]
                "solver-lab-stack-16" => ValidationScenario::SolverLabStack16,
                _ => return Err(LaunchError(format!("unknown scenario: {value}"))),
            }
        }
        "--body-capacity" => options.world_capacity = WorldCapacityPreset::parse(&value)?,
        "--solver" => {
            mark_physics_field(seen_physics_fields, "solver")?;
            options.physics.solver_profile = SolverProfileName::parse(&value)?;
        }
        "--solver-options" => {
            if value.is_empty() {
                return Err(LaunchError("--solver-options cannot be empty".to_owned()));
            }
            apply_legacy_solver_options(&mut options.physics, seen_physics_fields, &value)?;
        }
        "--sim-hz" => {
            mark_physics_field(seen_physics_fields, "sim-hz")?;
            options.physics.fixed_step_hz = parse_integer(argument, &value)?;
        }
        "--substeps" => {
            mark_physics_field(seen_physics_fields, "substeps")?;
            options.physics.substeps = parse_integer(argument, &value)?;
        }
        "--iterations" => {
            mark_physics_field(seen_physics_fields, "iterations")?;
            options.physics.biased_iterations = parse_integer(argument, &value)?;
        }
        "--contact-hz" => {
            mark_physics_field(seen_physics_fields, "contact-hz")?;
            options.physics.contact_hertz = parse_float(argument, &value)?;
        }
        "--contact-damping" => {
            mark_physics_field(seen_physics_fields, "contact-damping")?;
            options.physics.contact_damping_ratio = parse_float(argument, &value)?;
        }
        "--static-contact-scale" => {
            mark_physics_field(seen_physics_fields, "static-contact-scale")?;
            options.physics.static_contact_hertz_multiplier = parse_float(argument, &value)?;
        }
        "--speculative-distance" => {
            mark_physics_field(seen_physics_fields, "speculative-distance")?;
            options.physics.speculative_distance = parse_float(argument, &value)?;
        }
        "--max-depenetration-velocity" => {
            mark_physics_field(seen_physics_fields, "max-depenetration-velocity")?;
            options.physics.max_depenetration_velocity = parse_float(argument, &value)?;
        }
        "--sleeping" => {
            mark_physics_field(seen_physics_fields, "sleeping")?;
            options.physics.sleeping_enabled = match value.as_str() {
                "off" => false,
                "on" => true,
                _ => return Err(LaunchError("--sleeping requires off or on".to_owned())),
            };
        }
        "--frame-markers" if host.supports_frame_markers() => {
            options.frame_markers = match value.as_str() {
                "off" => FrameMarkerMode::Off,
                "milestones" => FrameMarkerMode::Milestones,
                "all" => FrameMarkerMode::All,
                _ => {
                    return Err(LaunchError(format!("unknown frame marker mode: {value}")));
                }
            }
        }
        "--frames" if host.supports_capture() => options.frames = parse_positive(argument, &value)?,
        "--fps" if host.supports_capture() => options.fps = parse_positive(argument, &value)?,
        "--steps" if host.supports_capture() => {
            options.steps = Some(parse_positive(argument, &value)?)
        }
        "--output" if host.supports_capture() => options.output = Some(PathBuf::from(value)),
        "--report" if host.supports_capture() => options.report = Some(PathBuf::from(value)),
        _ => {
            return Err(LaunchError(format!(
                "unknown option for {host:?}: {argument}"
            )));
        }
    }
    Ok(())
}

fn preparse_solver_profile(args: &[String]) -> Result<Option<SolverProfileName>, LaunchError> {
    let mut profile = None;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--help" || argument == "-h" {
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| LaunchError(format!("missing value for {argument}")))?;
        if argument == "--solver" {
            if profile.is_some() {
                return Err(LaunchError("duplicate physics option: solver".to_owned()));
            }
            profile = Some(SolverProfileName::parse(value)?);
        }
        index += 2;
    }
    Ok(profile)
}

fn mark_physics_field(
    seen: &mut BTreeSet<&'static str>,
    field: &'static str,
) -> Result<(), LaunchError> {
    if !seen.insert(field) {
        return Err(LaunchError(format!("duplicate physics option: {field}")));
    }
    Ok(())
}

fn apply_legacy_solver_options(
    physics: &mut PhysicsConfigurationV2,
    seen: &mut BTreeSet<&'static str>,
    encoded: &str,
) -> Result<(), LaunchError> {
    if physics.solver_profile != SolverProfileName::SoftCoupled {
        return Err(LaunchError(
            "hard-direct solver does not accept legacy --solver-options".to_owned(),
        ));
    }
    for field in encoded.split(',') {
        let (name, value) = field.split_once('=').ok_or_else(|| {
            LaunchError(format!("solver option {field} must use key=value syntax"))
        })?;
        match name {
            "hz" => {
                mark_physics_field(seen, "contact-hz")?;
                physics.contact_hertz = parse_float("solver option hz", value)?;
            }
            "damping" => {
                mark_physics_field(seen, "contact-damping")?;
                physics.contact_damping_ratio = parse_float("solver option damping", value)?;
            }
            "static-scale" => {
                mark_physics_field(seen, "static-contact-scale")?;
                physics.static_contact_hertz_multiplier =
                    parse_float("solver option static-scale", value)?;
            }
            _ => return Err(LaunchError(format!("unknown legacy solver option: {name}"))),
        }
    }
    Ok(())
}

fn bounded_integer(
    argument: &str,
    value: u32,
    minimum: u32,
    maximum: u32,
) -> Result<(), LaunchError> {
    if !(minimum..=maximum).contains(&value) {
        return Err(LaunchError(format!(
            "{argument} must be between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

fn bounded_float(
    argument: &str,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), LaunchError> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(LaunchError(format!(
            "{argument} must be finite and between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

fn parse_positive(argument: &str, value: &str) -> Result<u32, LaunchError> {
    let parsed = parse_integer(argument, value)?;
    if parsed == 0 {
        return Err(LaunchError(format!("{argument} must be greater than zero")));
    }
    Ok(parsed)
}

fn parse_integer(argument: &str, value: &str) -> Result<u32, LaunchError> {
    u32::from_str(value)
        .map_err(|_| LaunchError(format!("{argument} must be a nonnegative integer")))
}

fn parse_float(argument: &str, value: &str) -> Result<f32, LaunchError> {
    let parsed = f32::from_str(value)
        .map_err(|_| LaunchError(format!("{argument} must be a finite number")))?;
    if !parsed.is_finite() {
        return Err(LaunchError(format!("{argument} must be finite")));
    }
    Ok(parsed)
}

fn validate(host: LaunchHost, options: &LaunchOptions) -> Result<(), LaunchError> {
    options.physics.validate()?;
    if host.supports_capture() && options.output.is_none() {
        return Err(LaunchError("headless capture requires --output".to_owned()));
    }
    if host.supports_capture() && options.report.is_none() {
        return Err(LaunchError("headless capture requires --report".to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_keeps_the_surface_small() {
        let request = parse_tokens(
            LaunchHost::Desktop,
            [
                "--width",
                "1280",
                "--height",
                "720",
                "--scenario",
                "bootstrap",
            ],
        )
        .unwrap();
        assert_eq!(request.options.width, 1280);
        assert_eq!(request.options.height, 720);
        assert_eq!(request.options.frame_markers, FrameMarkerMode::Milestones);
    }

    #[test]
    fn interactive_frame_markers_are_explicit_and_host_scoped() {
        let desktop = parse_tokens(LaunchHost::Desktop, ["--frame-markers", "all"]).unwrap();
        assert_eq!(desktop.options.frame_markers, FrameMarkerMode::All);

        let android = parse_android_json(r#"["--frame-markers","off"]"#).unwrap();
        assert_eq!(android.options.frame_markers, FrameMarkerMode::Off);

        let error = parse_tokens(
            LaunchHost::Headless,
            [
                "--frame-markers",
                "all",
                "--output",
                "/tmp/out.png",
                "--report",
                "/tmp/out.json",
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown option"));
    }

    #[test]
    fn headless_requires_fresh_artifact_paths() {
        let error = parse_tokens(LaunchHost::Headless, ["--frames", "5"]).unwrap_err();
        assert!(error.to_string().contains("--output"));
    }

    #[test]
    fn web_query_rejects_host_owned_settings() {
        let error = parse_web_query("?solverIterations=4").unwrap_err();
        assert!(error.to_string().contains("unknown web query"));
    }

    #[test]
    fn hard_direct_rejects_legacy_soft_options() {
        let error = parse_web_query("?solver=hard-direct&solver-options=hz%3D30").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("hard-direct solver does not accept")
        );
    }

    #[test]
    fn production_launch_exposes_canonical_profiles_and_migration_aliases() {
        let hard = parse_web_query("?solver=selected").unwrap();
        assert_eq!(
            hard.options.physics.solver_profile,
            SolverProfileName::HardDirect
        );
        let soft = parse_web_query("?solver=branch-robust").unwrap();
        assert_eq!(
            soft.options.physics.solver_profile,
            SolverProfileName::SoftCoupled
        );
    }

    #[test]
    fn fresh_launch_uses_the_reviewed_soft_coupled_seed() {
        let request = parse_tokens(LaunchHost::Desktop, std::iter::empty::<&str>()).unwrap();
        assert_eq!(
            request.options.physics,
            PhysicsConfigurationV2::SOFT_COUPLED
        );
        assert_eq!(
            request.options.physics.solver_profile.name(),
            "soft-coupled"
        );
        assert_eq!(request.options.world_capacity, WorldCapacityPreset::Slots64);
    }

    #[test]
    fn world_capacity_presets_share_every_transport_and_reject_arbitrary_sizes() {
        let native = parse_tokens(LaunchHost::Desktop, ["--body-capacity", "10000"]).unwrap();
        let web = parse_web_query("?body-capacity=10k").unwrap();
        let android = parse_android_json(r#"["--body-capacity","10000"]"#).unwrap();
        assert_eq!(
            native.options.world_capacity,
            WorldCapacityPreset::Slots10000
        );
        assert_eq!(native.options.world_capacity, web.options.world_capacity);
        assert_eq!(
            native.options.world_capacity,
            android.options.world_capacity
        );

        let error = parse_tokens(LaunchHost::Desktop, ["--body-capacity", "4096"]).unwrap_err();
        assert!(error.to_string().contains("one of 64, 256, 1000, or 10000"));
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_lab_profile_and_scene_use_shared_launch_parsing() {
        let request = parse_web_query(
            "?scenario=solver-lab-stack-16&solver=branch-robust&solver-options=hz%3D30%2Cdamping%3D10%2Cstatic-scale%3D8",
        )
        .unwrap();
        assert_eq!(
            request.options.scenario,
            ValidationScenario::SolverLabStack16
        );
        assert_eq!(
            request.options.physics.solver_profile,
            SolverProfileName::SoftCoupled
        );
        assert_eq!(request.options.physics.contact_hertz, 30.0);
        assert_eq!(request.options.physics.contact_damping_ratio, 10.0);
        assert_eq!(request.options.physics.static_contact_hertz_multiplier, 8.0);
        assert!(!ValidationScenario::MENU.contains(&ValidationScenario::SolverLabStack16));
    }

    #[test]
    fn every_transport_produces_the_same_complete_physics_configuration() {
        let native = parse_tokens(
            LaunchHost::Desktop,
            [
                "--solver",
                "soft-coupled",
                "--sim-hz",
                "120",
                "--substeps",
                "8",
                "--iterations",
                "2",
                "--contact-hz",
                "45.5",
                "--contact-damping",
                "8",
                "--static-contact-scale",
                "3",
                "--speculative-distance",
                "0.03",
                "--max-depenetration-velocity",
                "4",
                "--sleeping",
                "on",
            ],
        )
        .unwrap();
        let web = parse_web_query(
            "?solver=soft-coupled&sim-hz=120&substeps=8&iterations=2&contact-hz=45.5&contact-damping=8&static-contact-scale=3&speculative-distance=0.03&max-depenetration-velocity=4&sleeping=on",
        )
        .unwrap();
        let android = parse_android_json(
            r#"["--solver","soft-coupled","--sim-hz","120","--substeps","8","--iterations","2","--contact-hz","45.5","--contact-damping","8","--static-contact-scale","3","--speculative-distance","0.03","--max-depenetration-velocity","4","--sleeping","on"]"#,
        )
        .unwrap();
        assert_eq!(native.options.physics, web.options.physics);
        assert_eq!(native.options.physics, android.options.physics);
        assert_eq!(native.options.physics.fixed_dt_seconds(), 1.0 / 120.0);
        assert!(native.options.physics.sleeping_enabled);
    }

    #[test]
    fn solver_selection_is_order_independent_and_applies_profile_defaults() {
        let request = parse_tokens(
            LaunchHost::Desktop,
            ["--iterations", "3", "--solver", "soft-coupled"],
        )
        .unwrap();
        assert_eq!(
            request.options.physics.solver_profile,
            SolverProfileName::SoftCoupled
        );
        assert_eq!(request.options.physics.biased_iterations, 3);

        let seed = parse_tokens(LaunchHost::Desktop, ["--solver", "soft-coupled"]).unwrap();
        assert_eq!(seed.options.physics, PhysicsConfigurationV2::SOFT_COUPLED);
    }

    #[test]
    fn physics_parser_rejects_duplicate_unknown_nonfinite_and_out_of_range_fields() {
        for (arguments, expected) in [
            (
                vec!["--substeps", "4", "--substeps", "8"],
                "duplicate physics option",
            ),
            (vec!["--contact-hz", "NaN"], "must be finite"),
            (vec!["--sim-hz", "241"], "between 15 and 240"),
            (
                vec!["--solver", "soft-coupled", "--solver-options", "wat=1"],
                "unknown legacy solver option",
            ),
            (vec!["--sleeping", "yes"], "requires off or on"),
        ] {
            let error = parse_tokens(LaunchHost::Desktop, arguments).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn android_uses_json_argv_transport() {
        let request = parse_android_json(r#"["--width","800"]"#).unwrap();
        assert_eq!(request.options.width, 800);
    }

    #[test]
    fn headless_accepts_an_exact_step_count() {
        let request = parse_tokens(
            LaunchHost::Headless,
            [
                "--steps",
                "600",
                "--output",
                "/tmp/out.png",
                "--report",
                "/tmp/out.json",
            ],
        )
        .unwrap();
        assert_eq!(request.options.steps, Some(600));
    }

    #[test]
    fn every_host_uses_the_shared_validation_scenario_names() {
        let web = parse_web_query("?scenario=solver-jacobi").unwrap();
        assert_eq!(web.options.scenario, ValidationScenario::SolverJacobi);
        let android = parse_android_json(r#"["--scenario","body-free-flight"]"#).unwrap();
        assert_eq!(android.options.scenario, ValidationScenario::BodyFreeFlight);
        let floor = parse_web_query("?scenario=rigid-floor").unwrap();
        assert_eq!(floor.options.scenario, ValidationScenario::RigidFloor);
        let stack = parse_web_query("?scenario=rigid-stack").unwrap();
        assert_eq!(stack.options.scenario, ValidationScenario::RigidStack);
        let rectangle_stack = parse_web_query("?scenario=rectangle-stack").unwrap();
        assert_eq!(
            rectangle_stack.options.scenario,
            ValidationScenario::RectangleStack
        );
        let circle_bucket = parse_web_query("?scenario=circle-bucket-100").unwrap();
        assert_eq!(
            circle_bucket.options.scenario,
            ValidationScenario::CircleBucket100
        );
        let rectangle_bucket =
            parse_android_json(r#"["--scenario","rectangle-bucket-100"]"#).unwrap();
        assert_eq!(
            rectangle_bucket.options.scenario,
            ValidationScenario::RectangleBucket100
        );
        let reset_grab = parse_android_json(r#"["--scenario","reset-grab"]"#).unwrap();
        assert_eq!(reset_grab.options.scenario, ValidationScenario::ResetGrab);
        let manifold = parse_web_query("?scenario=solver-manifold-i2").unwrap();
        assert_eq!(
            manifold.options.scenario,
            ValidationScenario::SolverManifoldI2
        );
        let parallel = parse_android_json(r#"["--scenario","solver-parallel-coloring"]"#).unwrap();
        assert_eq!(
            parallel.options.scenario,
            ValidationScenario::SolverParallelColoring
        );
        assert!(!ValidationScenario::MENU.contains(&ValidationScenario::SolverParallelColoring));
        let settling_candidate = parse_web_query("?scenario=settling-candidate-8").unwrap();
        assert_eq!(
            settling_candidate.options.scenario,
            ValidationScenario::SettlingCandidate8
        );
        let settling_control = parse_android_json(r#"["--scenario","settling-hard-16"]"#).unwrap();
        assert_eq!(
            settling_control.options.scenario,
            ValidationScenario::SettlingHard16
        );
        let identity = parse_web_query("?scenario=settling-identity-16").unwrap();
        assert_eq!(
            identity.options.scenario,
            ValidationScenario::SettlingIdentity16
        );
        let softness = parse_android_json(r#"["--scenario","settling-softness-16"]"#).unwrap();
        assert_eq!(
            softness.options.scenario,
            ValidationScenario::SettlingSoftness16
        );
        for scenario in [
            ValidationScenario::SettlingHard8,
            ValidationScenario::SettlingCandidate8,
            ValidationScenario::SettlingHard16,
            ValidationScenario::SettlingIdentity16,
            ValidationScenario::SettlingSoftness16,
            ValidationScenario::SettlingCandidate16,
        ] {
            assert!(!ValidationScenario::MENU.contains(&scenario));
        }
    }
}
