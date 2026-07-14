use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use physbox_contracts::{
    NamedValueV1, OracleRequestV1, PlaybackArtifactV1, PlaybackTrackV1, ProvenanceV1,
    SCHEMA_VERSION, Validate, canonical_hash, canonical_json, sha256_bytes,
};
use physbox_engine::{AnalyticFixtureKind, analytic_fixture_scenario};
use physbox_lab::{
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    playback::{render_svg, synchronized_sample},
    scalar::solve_analytic_fixture_steps,
};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const CHECKPOINTS: [u32; 7] = [1, 15, 30, 60, 90, 120, 180];

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-synchronized-playback error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    validate_run_id(&arguments.run_id)?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let harness_provenance = harness_provenance(&repository, &arguments.run_id)?;
    let box2d_source = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;

    fs::create_dir_all(&arguments.output_root).map_err(|error| error.to_string())?;
    let run_directory = arguments.output_root.join(&arguments.run_id);
    fs::create_dir(&run_directory).map_err(|error| {
        format!(
            "create fresh playback run directory {}: {error}",
            run_directory.display()
        )
    })?;
    sync_directory(&arguments.output_root)?;
    let oracle_root = run_directory.join("oracles");

    let mut tracks = Vec::new();
    for (track_id, source_scenario) in playback_scenarios()? {
        let source_scenario_hash = canonical_hash(&source_scenario).map_err(|e| e.to_string())?;
        let mut samples = Vec::new();
        for completed_steps in CHECKPOINTS {
            let mut scenario = source_scenario.clone();
            scenario.requested_steps = completed_steps;
            let scalar = solve_analytic_fixture_steps(&scenario).map_err(|e| e.to_string())?;
            let oracle_run_id = format!("{}-{}-{completed_steps:03}", arguments.run_id, track_id);
            let request = OracleRequestV1 {
                schema_version: SCHEMA_VERSION,
                run_id: oracle_run_id,
                source_scenario_hash: canonical_hash(&scenario).map_err(|e| e.to_string())?,
                scenario,
            };
            let oracle = run_oracle(
                &oracle_config(&oracle_root, &box2d_source, &arguments.oracle_executable),
                &request,
            )
            .map_err(|error| error.to_string())?;
            let relative_artifact = oracle
                .artifacts
                .directory
                .strip_prefix(&run_directory)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            samples.push(
                synchronized_sample(completed_steps, scalar, &oracle.result, relative_artifact)
                    .map_err(|error| error.to_string())?,
            );
        }
        tracks.push(PlaybackTrackV1 {
            id: track_id.to_owned(),
            source_scenario_hash,
            source_scenario,
            samples,
        });
    }

    let artifact = PlaybackArtifactV1 {
        schema_version: SCHEMA_VERSION,
        run_id: arguments.run_id.clone(),
        harness_provenance,
        tracks,
    };
    artifact.validate().map_err(|error| error.to_string())?;
    let manifest = canonical_json(&artifact).map_err(|error| error.to_string())?;
    let svg = render_svg(&artifact).map_err(|error| error.to_string())?;
    atomic_write(&run_directory.join("manifest.json"), &manifest)?;
    atomic_write(&run_directory.join("playback.svg"), svg.as_bytes())?;
    sync_directory(&run_directory)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=synchronized-playback run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest),
        run_directory.display()
    );
    Ok(())
}

fn playback_scenarios() -> Result<Vec<(&'static str, physbox_contracts::ScenarioV1)>, String> {
    let circle = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 180)
        .map_err(|error| error.to_string())?;
    let rectangle = analytic_fixture_scenario(AnalyticFixtureKind::RectangleFloor, 180)
        .map_err(|error| error.to_string())?;
    let mut sliding = rectangle.clone();
    sliding.id = "analytic-rectangle-floor-sliding".to_owned();
    sliding.bodies[0].position.y = 0.5;
    sliding.bodies[0].linear_velocity.x = 2.0;
    Ok(vec![
        ("circle-floor", circle),
        ("rectangle-floor", rectangle),
        ("sliding-friction", sliding),
    ])
}

fn oracle_config(
    artifact_root: &Path,
    source: &OracleSourceIdentity,
    executable: &Path,
) -> OracleRunConfig {
    OracleRunConfig {
        artifact_root: artifact_root.to_owned(),
        timeout: Duration::from_secs(15),
        source: source.clone(),
        process: OracleProcess {
            program: executable.to_owned(),
            arguments: vec![OsString::from("--lane"), OsString::from("world")],
        },
    }
}

fn harness_provenance(repository: &Path, run_id: &str) -> Result<ProvenanceV1, String> {
    let source_revision = git_text(repository, &["rev-parse", "HEAD"])?;
    let status = git_bytes(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let source_dirty = !status.is_empty();
    let source_patch_digest = if source_dirty {
        let mut patch = status;
        patch.extend(git_bytes(repository, &["diff", "--binary", "HEAD", "--"])?);
        let untracked = git_bytes(
            repository,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        for raw_path in untracked
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let relative = std::str::from_utf8(raw_path).map_err(|error| error.to_string())?;
            patch.extend_from_slice(relative.as_bytes());
            patch.push(0);
            patch.extend(fs::read(repository.join(relative)).map_err(|error| error.to_string())?);
            patch.push(0);
        }
        Some(sha256_bytes(&patch))
    } else {
        None
    };
    let rustc = command_text(Command::new("rustc").arg("--version"), "query rustc")?;
    Ok(ProvenanceV1 {
        run_id: run_id.to_owned(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        adapter: "physbox-synchronized-playback".to_owned(),
        backend: "native-cpu".to_owned(),
        driver: std::env::consts::ARCH.to_owned(),
        toolchain: rustc,
        build_options: vec![named(
            "profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        )],
        runtime_settings: vec![
            named("checkpoints", "1,15,30,60,90,120,180"),
            named("fixtures", "circle-floor,rectangle-floor,sliding-friction"),
            named("oracle_lane", "world"),
        ],
    })
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn git_text(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    String::from_utf8(git_bytes(repository, arguments)?).map_err(|error| error.to_string())
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut stdout = output.stdout;
    while stdout
        .last()
        .is_some_and(|byte| matches!(byte, b'\n' | b'\r'))
    {
        stdout.pop();
    }
    Ok(stdout)
}

fn command_text(command: &mut Command, action: &str) -> Result<String, String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{action} failed with {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    if !run_id.is_empty()
        && run_id.len() <= 48
        && run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Ok(())
    } else {
        Err("run id must be 1-48 portable filename characters".to_owned())
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension("partial");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&partial, path).map_err(|error| error.to_string())?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

struct Arguments {
    oracle_executable: PathBuf,
    box2d_source: PathBuf,
    output_root: PathBuf,
    run_id: String,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut oracle_executable = None;
        let mut box2d_source = None;
        let mut output_root = None;
        let mut run_id = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--oracle" => oracle_executable = Some(PathBuf::from(value)),
                "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
                "--output-root" => output_root = Some(PathBuf::from(value)),
                "--run-id" => run_id = Some(value),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            oracle_executable: oracle_executable.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            output_root: output_root.ok_or_else(|| "missing --output-root".to_owned())?,
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
        })
    }
}
