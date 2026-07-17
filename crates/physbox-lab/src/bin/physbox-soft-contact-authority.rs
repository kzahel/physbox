use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use physbox_contracts::{
    BodyStateV1, CONTACT_SOFTNESS_POLICY_VERSION, ContactIdentityPolicyV1, ContactSoftnessPolicyV1,
    FiniteF32, OracleRequestV1, SCHEMA_VERSION, ScenarioV1, ShapeV1, canonical_hash,
    canonical_json, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactBenchmarkOutput, P5_2_SETTLING_EVIDENCE_STEPS, P5_2SettlingFixtureKind,
    benchmark_gpu_generated_contact_identity_experiment, p5_2_box2d_world_projection,
    p5_2_centered_rectangle_stack_scenario, p5_2_generated_performance_scenario,
    p5_2_settling_fixture_scenario,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
};
use serde::Serialize;

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const FACTORIAL_MANIFEST: &str = "evidence/p5.2b-s6.4-soft-contact/manifest.json";
const REVIEW_SCENES: [&str; 4] = [
    "centered-stack-4",
    "centered-stack-8",
    "sequential-rectangles-5-seed-6002",
    "small-mixed-pile-seed-6002",
];

struct SceneInput {
    label: String,
    source: ScenarioV1,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    id: &'static str,
    identity_policy: ContactIdentityPolicyV1,
    softness: Option<ContactSoftnessPolicyV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    factorial_manifest: ArtifactIdentity,
    box2d: SourceIdentity,
    authority: Vec<SceneAuthority>,
    gpu_performance: Vec<GpuPerformance>,
    optimized_box2d_performance: Vec<Box2dPerformance>,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceIdentity {
    revision: String,
    dirty: bool,
    patch_digest: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneAuthority {
    label: String,
    source_scenario_hash: String,
    projection_hash: String,
    samples: Vec<AuthoritySample>,
    final_replay_exact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoritySample {
    phase: &'static str,
    completed_steps: u32,
    request_hash: String,
    result_payload_hash: String,
    artifact: String,
    dynamic_linear_speed_max: f32,
    dynamic_angular_edge_speed_max: f32,
    contact_points: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuPerformance {
    candidate: Candidate,
    bodies: u32,
    source_scenario_hash: String,
    samples: u32,
    gpu_end_to_end_seconds: Option<Vec<f64>>,
    gpu_end_to_end_p50_seconds: Option<f64>,
    host_completion_seconds: Vec<f64>,
    host_completion_p50_seconds: f64,
    cpu_encode_submit_seconds: Vec<f64>,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    collision_persistence_allocated_bytes: u64,
    replay_exact: bool,
    capacity_clean: bool,
    raw: ArtifactIdentity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Box2dPerformance {
    bodies: u32,
    workers: u32,
    samples_seconds: Vec<f64>,
    p50_seconds: f64,
    scope: String,
    source_revision: String,
    source_dirty: bool,
    contact_count_min: u64,
    contact_count_max: u64,
    success_sentinel: bool,
    artifact: ArtifactIdentity,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-soft-contact-authority error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S6.4 authority evidence requires a clean source tree".to_owned());
    }
    let factorial_bytes = fs::read(repository.join(FACTORIAL_MANIFEST)).map_err(display_error)?;
    let factorial: serde_json::Value =
        serde_json::from_slice(&factorial_bytes).map_err(display_error)?;
    if factorial["artifactKind"] != "p5.2b-s6.4-soft-contact-factorial"
        || factorial["sourceDirty"] != false
        || factorial["productConstructorChanged"] != false
    {
        return Err("S6.4 factorial manifest is not the clean frozen source".to_owned());
    }
    let box2d =
        verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION).map_err(display_error)?;
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("authority output or staging directory already exists".to_owned());
    }
    for directory in ["oracles", "gpu", "box2d"] {
        fs::create_dir_all(staging.join(directory)).map_err(display_error)?;
    }

    let mut authority = Vec::new();
    for input in scene_inputs()? {
        eprintln!("S6.4 Box2D authority {}", input.label);
        authority.push(run_scene_authority(&arguments, &staging, &box2d, input)?);
    }
    let gpu_performance = gpu_performance(&staging, arguments.warmup, arguments.samples)?;
    let optimized_box2d_performance =
        optimized_box2d_performance(&arguments, &staging, arguments.warmup, arguments.samples)?;
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s6.4-soft-contact-authority-performance",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        factorial_manifest: ArtifactIdentity {
            path: FACTORIAL_MANIFEST.to_owned(),
            sha256: sha256_bytes(&factorial_bytes),
        },
        box2d: source_identity(&box2d),
        authority,
        gpu_performance,
        optimized_box2d_performance,
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
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s6.4-soft-contact-authority run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn run_scene_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    input: SceneInput,
) -> Result<SceneAuthority, String> {
    let source_scenario_hash = canonical_hash(&input.source).map_err(display_error)?;
    let projection = p5_2_box2d_world_projection(&input.source).map_err(display_error)?;
    if projection.settings.contact_softness.is_some() {
        return Err("authority projection retained candidate-only softness".to_owned());
    }
    let projection_hash = canonical_hash(&projection).map_err(display_error)?;
    let mut samples = Vec::new();
    if REVIEW_SCENES.contains(&input.label.as_str()) {
        for completed_steps in [600, 900] {
            samples.push(run_authority(
                arguments,
                staging,
                box2d,
                &input.label,
                &projection,
                completed_steps,
                "checkpoint",
            )?);
        }
    }
    let primary = run_authority(
        arguments,
        staging,
        box2d,
        &input.label,
        &projection,
        P5_2_SETTLING_EVIDENCE_STEPS,
        "primary",
    )?;
    let replay = run_authority(
        arguments,
        staging,
        box2d,
        &input.label,
        &projection,
        P5_2_SETTLING_EVIDENCE_STEPS,
        "replay",
    )?;
    let final_replay_exact = primary.result_payload_hash == replay.result_payload_hash;
    samples.push(primary);
    samples.push(replay);
    Ok(SceneAuthority {
        label: input.label,
        source_scenario_hash,
        projection_hash,
        samples,
        final_replay_exact,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
    completed_steps: u32,
    phase: &'static str,
) -> Result<AuthoritySample, String> {
    let mut projection = projection.clone();
    projection.requested_steps = completed_steps;
    let request_hash = canonical_hash(&projection).map_err(display_error)?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-{label}-{completed_steps:04}-{phase}", arguments.run_id),
        source_scenario_hash: request_hash.clone(),
        scenario: projection,
    };
    let oracle = run_oracle(
        &OracleRunConfig {
            artifact_root: staging.join("oracles"),
            timeout: Duration::from_secs(20),
            source: box2d.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![OsString::from("--lane"), OsString::from("world-awake")],
            },
        },
        &request,
    )
    .map_err(display_error)?;
    let result_payload_hash = serialized_hash(&(
        &oracle.result.bodies,
        &oracle.result.manifolds,
        &oracle.result.traces,
        &oracle.result.metrics,
    ))?;
    let artifact = oracle
        .artifacts
        .directory
        .strip_prefix(staging)
        .map_err(display_error)?
        .to_string_lossy()
        .replace('\\', "/");
    let (linear, angular) = body_speeds(&oracle.result.bodies);
    Ok(AuthoritySample {
        phase,
        completed_steps,
        request_hash,
        result_payload_hash,
        artifact,
        dynamic_linear_speed_max: linear,
        dynamic_angular_edge_speed_max: angular,
        contact_points: oracle
            .result
            .manifolds
            .iter()
            .map(|manifold| manifold.points.len() as u32)
            .sum(),
    })
}

fn gpu_performance(root: &Path, warmup: u32, samples: u32) -> Result<Vec<GpuPerformance>, String> {
    let candidates = [
        Candidate {
            id: "hard-direct-control",
            identity_policy: ContactIdentityPolicyV1::DirectMaximumSeparation,
            softness: None,
        },
        Candidate {
            id: "soft-hz30-d10-static1-prior",
            identity_policy: ContactIdentityPolicyV1::PriorContactHysteresis,
            softness: Some(softness()),
        },
    ];
    let mut rows = Vec::new();
    for bodies in [32, 256, 1_024] {
        for candidate in &candidates {
            eprintln!("S6.4 GPU performance {} bodies {bodies}", candidate.id);
            let mut scenario =
                p5_2_generated_performance_scenario(bodies).map_err(display_error)?;
            scenario.settings.contact_softness = candidate.softness;
            let raw = pollster::block_on(benchmark_gpu_generated_contact_identity_experiment(
                &scenario,
                warmup,
                samples,
                candidate.identity_policy,
            ))?;
            let raw_identity =
                write_json_artifact(root, &format!("gpu/{}-{bodies}.json", candidate.id), &raw)?;
            rows.push(gpu_performance_row(candidate, raw, raw_identity));
        }
    }
    Ok(rows)
}

fn gpu_performance_row(
    candidate: &Candidate,
    raw: GpuGeneratedContactBenchmarkOutput,
    identity: ArtifactIdentity,
) -> GpuPerformance {
    let capacity_clean = raw.correctness.failure.is_none()
        && raw
            .correctness
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed);
    GpuPerformance {
        candidate: candidate.clone(),
        bodies: raw.body_count,
        source_scenario_hash: raw.source_scenario_hash,
        samples: raw.samples,
        gpu_end_to_end_p50_seconds: raw.gpu_end_to_end_seconds.as_deref().map(p50),
        gpu_end_to_end_seconds: raw.gpu_end_to_end_seconds,
        host_completion_p50_seconds: p50(&raw.host_completion_seconds),
        host_completion_seconds: raw.host_completion_seconds,
        cpu_encode_submit_seconds: raw.cpu_encode_submit_seconds,
        dispatches_per_step: raw.dispatches_per_step,
        allocated_bytes: raw.allocated_bytes,
        collision_persistence_allocated_bytes: raw.collision_persistence_allocated_bytes,
        replay_exact: raw.replay_exact,
        capacity_clean,
        raw: identity,
    }
}

fn optimized_box2d_performance(
    arguments: &Arguments,
    staging: &Path,
    warmup: u32,
    samples: u32,
) -> Result<Vec<Box2dPerformance>, String> {
    let mut rows = Vec::new();
    for bodies in [32_u32, 256, 1_024] {
        let scenario = p5_2_generated_performance_scenario(bodies).map_err(display_error)?;
        let request = staging.join(format!("box2d/scenario-{bodies}.json"));
        atomic_write(&request, &canonical_json(&scenario).map_err(display_error)?)?;
        for workers in [1_u32, 4] {
            eprintln!("S6.4 optimized Box2D bodies {bodies} workers {workers}");
            let relative = format!("box2d/result-{bodies}-workers-{workers}.json");
            let result_path = staging.join(&relative);
            let output = Command::new(&arguments.box2d_benchmark)
                .args([
                    "--request",
                    &request.to_string_lossy(),
                    "--result",
                    &result_path.to_string_lossy(),
                    "--run-id",
                    &format!("{}-box2d-{bodies}-w{workers}", arguments.run_id),
                    "--workers",
                    &workers.to_string(),
                    "--warmup",
                    &warmup.to_string(),
                    "--samples",
                    &samples.to_string(),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(display_error)?;
            if !output.status.success() {
                return Err(format!(
                    "optimized Box2D failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let bytes = fs::read(&result_path).map_err(display_error)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(display_error)?;
            let samples_seconds = value["samplesSeconds"]
                .as_array()
                .ok_or_else(|| "optimized Box2D samples are missing".to_owned())?
                .iter()
                .map(|sample| {
                    sample
                        .as_f64()
                        .ok_or_else(|| "optimized Box2D sample is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(Box2dPerformance {
                bodies,
                workers,
                p50_seconds: p50(&samples_seconds),
                samples_seconds,
                scope: json_string(&value, "scope")?,
                source_revision: json_string(&value, "sourceRevision")?,
                source_dirty: value["sourceDirty"].as_bool().unwrap_or(true),
                contact_count_min: value["contactCountMin"].as_u64().unwrap_or(u64::MAX),
                contact_count_max: value["contactCountMax"].as_u64().unwrap_or(u64::MAX),
                success_sentinel: String::from_utf8_lossy(&output.stdout)
                    .contains("PHYSBOX5_BOX2D_BENCHMARK_SUCCESS"),
                artifact: ArtifactIdentity {
                    path: relative,
                    sha256: sha256_bytes(&bytes),
                },
            });
        }
    }
    Ok(rows)
}

fn scene_inputs() -> Result<Vec<SceneInput>, String> {
    let mut inputs = Vec::new();
    for count in [2, 4, 8, 16] {
        inputs.push(SceneInput {
            label: format!("centered-stack-{count}"),
            source: p5_2_centered_rectangle_stack_scenario(count, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display_error)?,
        });
    }
    for kind in P5_2SettlingFixtureKind::ALL {
        inputs.push(SceneInput {
            label: format!("{}-seed-6002", kind.name()),
            source: p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS)
                .map_err(display_error)?,
        });
    }
    Ok(inputs)
}

fn body_speeds(bodies: &[BodyStateV1]) -> (f32, f32) {
    bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .fold((0.0_f32, 0.0_f32), |(linear, angular), body| {
            let radius = match body.shape {
                ShapeV1::Circle { radius } => radius.get(),
                ShapeV1::Rectangle { half_extents } => half_extents.x.hypot(half_extents.y),
            };
            (
                linear.max(body.linear_velocity.x.hypot(body.linear_velocity.y)),
                angular.max(body.angular_velocity.get().abs() * radius),
            )
        })
}

fn p50(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[(values.len() - 1) / 2]
}

fn json_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("optimized Box2D result omitted {key}"))
}

fn source_identity(source: &OracleSourceIdentity) -> SourceIdentity {
    SourceIdentity {
        revision: source.revision.clone(),
        dirty: source.dirty,
        patch_digest: source.patch_digest.clone(),
    }
}

fn softness() -> ContactSoftnessPolicyV1 {
    ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    }
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
    atomic_write(&root.join(relative), &bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(&bytes),
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
    FiniteF32::new(value).expect("authority candidate constants are finite")
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
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
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut box2d_benchmark = None;
        let mut warmup = 10;
        let mut samples = 20;
        let mut require_clean = false;
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
                "--warmup" => warmup = value.parse().map_err(|_| "invalid warmup".to_owned())?,
                "--samples" => samples = value.parse().map_err(|_| "invalid samples".to_owned())?,
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        if warmup == 0 || samples == 0 {
            return Err("warmup and samples must be positive".to_owned());
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_catalog_and_candidate_are_frozen() {
        assert_eq!(scene_inputs().unwrap().len(), 11);
        assert_eq!(REVIEW_SCENES.len(), 4);
        let policy = softness();
        assert_eq!(policy.contact_hertz.get(), 30.0);
        assert_eq!(policy.static_contact_hertz_multiplier.get(), 1.0);
    }
}
