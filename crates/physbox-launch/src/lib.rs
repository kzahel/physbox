#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use serde::Serialize;

pub const ANDROID_STARTUP_ARGV_INTENT_EXTRA: &str = "physbox5.startup.argv";

pub const DEFAULT_WIDTH: u32 = 960;
pub const DEFAULT_HEIGHT: u32 = 600;
pub const DEFAULT_CAPTURE_FRAMES: u32 = 12;
pub const DEFAULT_CAPTURE_FPS: u32 = 60;

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
}

impl ValidationScenario {
    pub const ALL: [Self; 10] = [
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchOptions {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub fps: u32,
    pub steps: Option<u32>,
    pub output: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub scenario: ValidationScenario,
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
            frame_markers: FrameMarkerMode::Milestones,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    let mut options = LaunchOptions::default();
    let mut help_requested = false;
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            help_requested = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| LaunchError(format!("missing value for {argument}")))?;
        apply_option(host, &mut options, &argument, value)?;
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
    format!(
        "Usage: {program} [options]\n\n  --width PX       viewport width (default 960)\n  --height PX      viewport height (default 600)\n  --scenario NAME  bootstrap, body-free-flight, rigid-floor, rigid-stack, rectangle-stack, circle-bucket-100, rectangle-bucket-100, reset-grab, solver-jacobi, or solver-colored{frame_markers}{capture}\n  -h, --help       print this help"
    )
}

fn apply_option(
    host: LaunchHost,
    options: &mut LaunchOptions,
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
                _ => return Err(LaunchError(format!("unknown scenario: {value}"))),
            }
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

fn parse_positive(argument: &str, value: &str) -> Result<u32, LaunchError> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| LaunchError(format!("{argument} must be a positive integer")))?;
    if parsed == 0 {
        return Err(LaunchError(format!("{argument} must be greater than zero")));
    }
    Ok(parsed)
}

fn validate(host: LaunchHost, options: &LaunchOptions) -> Result<(), LaunchError> {
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
    }
}
