use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{OracleResultV1, SCHEMA_VERSION, ScenarioV1, canonical_hash, sha256_bytes};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuSolverExecutionV4, P5_2_SETTLING_EVIDENCE_STEPS,
    p5_2_box2d_world_projection,
};
use physbox_lab::{
    settling::centered_stack_structure,
    stationarity::{
        StationarityAnalysisV1, StationarityCaptureV1, StationarityClassComparisonV1,
        StationarityClassVerdictV1, StationarityManifoldClassV1, StationaritySnapshotInputV1,
        analyze_stationarity_v1, compare_stationarity_v1, decode_box2d_step_trace,
        stationarity_capture_v1, stationarity_metric_contract_v1,
    },
};
use serde::{Deserialize, Serialize};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const S10_1_SOURCE_REVISION: &str = "332aee28f3ef287dbde9d2e7de511c619c25a1e6";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    box2d_revision: &'static str,
    input: InputIdentity,
    adapter: physbox_engine::AdapterDiagnostics,
    stationarity_contract: physbox_lab::stationarity::StationarityMetricContractV1,
    scenes: Vec<SceneEvidence>,
    physbox_analysis: StationarityAnalysisV1,
    authority_analysis: StationarityAnalysisV1,
    class_comparisons: Vec<StationarityClassComparisonV1>,
    hypothesis: HypothesisVerdict,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InputIdentity {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    manifest_path: String,
    manifest_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneEvidence {
    label: String,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    physbox: GpuLaneEvidence,
    authority: AuthorityLaneEvidence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuLaneEvidence {
    lane: &'static str,
    solver_execution: GpuSolverExecutionV4,
    scheduling_policy: physbox_engine::GpuSolverSchedulingPolicyV1,
    raw_series_hash: String,
    s10_1_raw_series_hash_exact: bool,
    exact_replay: bool,
    capture_exact_replay: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    capture: ArtifactIdentity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityLaneEvidence {
    lane: &'static str,
    input_artifact: String,
    input_replay_artifact: String,
    result_sha256: String,
    replay_result_sha256: String,
    exact_replay: bool,
    capture_exact_replay: bool,
    clean_public_trace_provenance: bool,
    public_trace_records: usize,
    capture: ArtifactIdentity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HypothesisVerdict {
    s10_1_majority_removal: bool,
    governed_floor_or_interior_exceedance: bool,
    signed_hypothesis: &'static str,
    rationale: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_1Manifest {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    box2d_revision: String,
    routing: S10_1Routing,
    scenes: Vec<S10_1Scene>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_1Routing {
    majority_removed: bool,
    favored_hypothesis: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_1Scene {
    label: String,
    source_scenario_hash: String,
    projection_hash: String,
    source: InputArtifactIdentity,
    physbox_warm_on: S10_1GpuLane,
    authority_warm_on: S10_1AuthorityLane,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_1GpuLane {
    raw_series_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_1AuthorityLane {
    artifact: String,
    replay_artifact: String,
    exact_replay: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s10-impulse-stationarity error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let input_bytes = fs::read(&arguments.s10_1).map_err(|error| error.to_string())?;
    let input: S10_1Manifest =
        serde_json::from_slice(&input_bytes).map_err(|error| error.to_string())?;
    validate_input(&input)?;
    let input_root = arguments
        .s10_1
        .parent()
        .ok_or_else(|| "S10.1 manifest has no data directory".to_owned())?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S10.2 stationarity capture requires a clean source tree".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("S10.2 output or staging directory already exists".to_owned());
    }
    for directory in ["sources", "captures"] {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut scenes = Vec::new();
    let mut gpu_captures = Vec::new();
    let mut authority_captures = Vec::new();
    for label in ["centered-stack-8", "centered-stack-16"] {
        eprintln!("S10.2 stationarity {label}");
        let input_scene = input
            .scenes
            .iter()
            .find(|scene| scene.label == label)
            .ok_or_else(|| format!("S10.1 input omits {label}"))?;
        let evaluated = evaluate_scene(&staging, input_root, &runner, input_scene)?;
        gpu_captures.push(evaluated.gpu_capture);
        authority_captures.push(evaluated.authority_capture);
        scenes.push(evaluated.evidence);
    }
    let physbox_analysis = analyze_stationarity_v1(&gpu_captures)?;
    let authority_analysis = analyze_stationarity_v1(&authority_captures)?;
    let class_comparisons = compare_stationarity_v1(&physbox_analysis, &authority_analysis)?;
    let governed_exceedance = class_comparisons.iter().any(|comparison| {
        matches!(
            comparison.class,
            StationarityManifoldClassV1::FloorSupport | StationarityManifoldClassV1::Interior
        ) && comparison.metrics.iter().any(|metric| {
            metric.exceeds_authority
                && (metric.metric.starts_with("normal-")
                    || metric.metric.starts_with("active-set-")
                    || metric.metric.starts_with("redistribution-"))
        })
    });
    let hypothesis_1 = input.routing.majority_removed && governed_exceedance;
    let hypothesis = HypothesisVerdict {
        s10_1_majority_removal: input.routing.majority_removed,
        governed_floor_or_interior_exceedance: governed_exceedance,
        signed_hypothesis: if hypothesis_1 {
            "hypothesis-1-non-stationary-stored-impulses"
        } else {
            "hypothesis-2-stationary-lossy-composition"
        },
        rationale: if hypothesis_1 {
            "S10.1 majority removal and a governed floor/interior normal, active-set, or redistribution metric exceeds the warm-on authority band"
        } else {
            "the frozen conjunction for hypothesis 1 is not satisfied"
        },
    };
    if class_comparisons.len() != 3
        || class_comparisons.iter().any(|comparison| {
            comparison.metrics.len() != 8
                || !matches!(
                    comparison.verdict,
                    StationarityClassVerdictV1::Stationary
                        | StationarityClassVerdictV1::Oscillating
                )
        })
    {
        return Err("S10.2 did not compute every frozen class metric".to_owned());
    }
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s10.2-impulse-stationarity",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        box2d_revision: BOX2D_REVISION,
        input: InputIdentity {
            artifact_kind: input.artifact_kind,
            run_id: input.run_id,
            source_revision: input.source_revision,
            manifest_path: arguments.s10_1.to_string_lossy().replace('\\', "/"),
            manifest_sha256: sha256_bytes(&input_bytes),
        },
        adapter: runner.diagnostics().clone(),
        stationarity_contract: stationarity_metric_contract_v1(),
        scenes,
        physbox_analysis,
        authority_analysis,
        class_comparisons,
        hypothesis,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s10.2-impulse-stationarity run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn validate_input(input: &S10_1Manifest) -> Result<(), String> {
    if input.artifact_kind != "p5.2b-s10.1-warm-start-differential"
        || input.source_revision != S10_1_SOURCE_REVISION
        || input.source_dirty
        || input.box2d_revision != BOX2D_REVISION
        || !input.routing.majority_removed
        || input.routing.favored_hypothesis != "hypothesis-1-non-stationary-stored-impulses"
    {
        return Err("S10.2 input is not the governed clean S10.1 H1 route".to_owned());
    }
    Ok(())
}

struct EvaluatedScene {
    evidence: SceneEvidence,
    gpu_capture: StationarityCaptureV1,
    authority_capture: StationarityCaptureV1,
}

fn evaluate_scene(
    staging: &Path,
    input_root: &Path,
    runner: &GpuGeneratedContactRunner,
    input: &S10_1Scene,
) -> Result<EvaluatedScene, String> {
    let source_bytes =
        fs::read(input_root.join(&input.source.path)).map_err(|error| error.to_string())?;
    if sha256_bytes(&source_bytes) != input.source.sha256 {
        return Err(format!("S10.1 source digest differs for {}", input.label));
    }
    let source: ScenarioV1 =
        serde_json::from_slice(&source_bytes).map_err(|error| error.to_string())?;
    if source.requested_steps != P5_2_SETTLING_EVIDENCE_STEPS
        || canonical_hash(&source).map_err(|error| error.to_string())? != input.source_scenario_hash
    {
        return Err(format!("invalid S10.1 source for {}", input.label));
    }
    let projection = p5_2_box2d_world_projection(&source).map_err(|error| error.to_string())?;
    if canonical_hash(&projection).map_err(|error| error.to_string())? != input.projection_hash {
        return Err(format!("S10.1 projection drifted for {}", input.label));
    }
    let source_artifact = write_artifact(
        staging,
        &format!("sources/{}.json", input.label),
        &source_bytes,
    )?;

    let raw = runner.run_selected_observation_series(&source, 1, 1_200)?;
    let replay = runner.run_selected_observation_series(&source, 1, 1_200)?;
    let raw_series_hash = serialized_hash(&raw)?;
    let exact_replay = raw_series_hash == serialized_hash(&replay)?;
    let final_only = runner.run(&source, 1_200)?;
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "S10.2 GPU series is empty".to_owned())?;
    let instrumentation_final_exact = final_snapshot.bodies == final_only.bodies
        && final_snapshot.manifolds == final_only.manifolds
        && final_snapshot.cache == final_only.cache;
    let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean);
    let mut structural_pass = true;
    let mut structural_reason = "structure was not evaluated".to_owned();
    for snapshot in &raw.snapshots {
        let result = centered_stack_structure(
            &source,
            &snapshot.bodies,
            !snapshot_capacity_clean(snapshot),
            0.0,
        );
        structural_pass &= result.0;
        if snapshot.completed_steps == 1_200 {
            structural_reason = result.1;
        }
    }
    let gpu_capture = stationarity_capture_v1(
        "physbox-selected-warm-on",
        &input.label,
        &source,
        &input.source_scenario_hash,
        raw.snapshots
            .iter()
            .map(|snapshot| StationaritySnapshotInputV1 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
            }),
    )?;
    let gpu_replay_capture = stationarity_capture_v1(
        "physbox-selected-warm-on",
        &input.label,
        &source,
        &input.source_scenario_hash,
        replay
            .snapshots
            .iter()
            .map(|snapshot| StationaritySnapshotInputV1 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
            }),
    )?;
    let capture_exact_replay = gpu_capture == gpu_replay_capture;
    let gpu_capture_artifact = write_gzip_json_artifact(
        staging,
        &format!("captures/{}-physbox.json.gz", input.label),
        &gpu_capture,
    )?;

    let first_path = input_root
        .join(&input.authority_warm_on.artifact)
        .join("oracle-result.json");
    let replay_path = input_root
        .join(&input.authority_warm_on.replay_artifact)
        .join("oracle-result.json");
    let first_bytes = fs::read(&first_path).map_err(|error| error.to_string())?;
    let replay_bytes = fs::read(&replay_path).map_err(|error| error.to_string())?;
    let first: OracleResultV1 =
        serde_json::from_slice(&first_bytes).map_err(|error| error.to_string())?;
    let authority_replay: OracleResultV1 =
        serde_json::from_slice(&replay_bytes).map_err(|error| error.to_string())?;
    let exact_authority_replay = input.authority_warm_on.exact_replay
        && result_payload_hash(&first)? == result_payload_hash(&authority_replay)?;
    let clean_public_trace_provenance =
        authority_trace_provenance(&first) && authority_trace_provenance(&authority_replay);
    let snapshots = decode_box2d_step_trace(&projection, &first)?;
    let replay_snapshots = decode_box2d_step_trace(&projection, &authority_replay)?;
    let authority_capture = stationarity_capture_v1(
        "box2d-v3-warm-on-authority",
        &input.label,
        &projection,
        &input.source_scenario_hash,
        snapshots
            .iter()
            .map(|snapshot| StationaritySnapshotInputV1 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
            }),
    )?;
    let authority_replay_capture = stationarity_capture_v1(
        "box2d-v3-warm-on-authority",
        &input.label,
        &projection,
        &input.source_scenario_hash,
        replay_snapshots
            .iter()
            .map(|snapshot| StationaritySnapshotInputV1 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
            }),
    )?;
    let authority_capture_exact_replay = authority_capture == authority_replay_capture;
    let authority_capture_artifact = write_gzip_json_artifact(
        staging,
        &format!("captures/{}-authority.json.gz", input.label),
        &authority_capture,
    )?;

    if !exact_replay
        || raw_series_hash != input.physbox_warm_on.raw_series_hash
        || !capture_exact_replay
        || !instrumentation_final_exact
        || !capacity_clean
        || !structural_pass
        || !exact_authority_replay
        || !authority_capture_exact_replay
        || !clean_public_trace_provenance
    {
        return Err(format!(
            "S10.2 replay, baseline, structure, capacity, or provenance failed for {}",
            input.label
        ));
    }
    Ok(EvaluatedScene {
        evidence: SceneEvidence {
            label: input.label.clone(),
            source_scenario_hash: input.source_scenario_hash.clone(),
            projection_hash: input.projection_hash.clone(),
            source: source_artifact,
            physbox: GpuLaneEvidence {
                lane: "physbox-selected-warm-on",
                solver_execution: raw.solver_execution,
                scheduling_policy: raw.solver_scheduling_policy,
                raw_series_hash,
                s10_1_raw_series_hash_exact: true,
                exact_replay,
                capture_exact_replay,
                instrumentation_final_exact,
                capacity_clean,
                structural_pass,
                structural_reason,
                capture: gpu_capture_artifact,
            },
            authority: AuthorityLaneEvidence {
                lane: "box2d-v3-warm-on-authority",
                input_artifact: input.authority_warm_on.artifact.clone(),
                input_replay_artifact: input.authority_warm_on.replay_artifact.clone(),
                result_sha256: sha256_bytes(&first_bytes),
                replay_result_sha256: sha256_bytes(&replay_bytes),
                exact_replay: exact_authority_replay,
                capture_exact_replay: authority_capture_exact_replay,
                clean_public_trace_provenance,
                public_trace_records: first.traces.len(),
                capture: authority_capture_artifact,
            },
        },
        gpu_capture,
        authority_capture,
    })
}

fn authority_trace_provenance(result: &OracleResultV1) -> bool {
    let setting = |name: &str| {
        result
            .provenance
            .runtime_settings
            .iter()
            .find(|setting| setting.name == name)
            .map(|setting| setting.value.as_str())
    };
    result.failure.is_none()
        && result.provenance.source_revision == BOX2D_REVISION
        && !result.provenance.source_dirty
        && result.provenance.source_patch_digest.is_none()
        && setting("lane") == Some("world-awake")
        && setting("sleep") == Some("false")
        && setting("warm_start") == Some("true")
        && setting("step_trace") == Some("settled-contact-impulses-plus-every-step-bodies")
        && !result.traces.is_empty()
}

fn result_payload_hash(result: &OracleResultV1) -> Result<String, String> {
    serialized_hash(&(
        &result.bodies,
        &result.manifolds,
        &result.traces,
        &result.metrics,
    ))
}

fn snapshot_capacity_clean(snapshot: &physbox_engine::GpuGeneratedContactSnapshotV1) -> bool {
    snapshot.failure.is_none()
        && snapshot
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| error.to_string())
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
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    write_artifact(root, relative, &compressed)
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
    s10_1: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut s10_1 = None;
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
                "--s10-1" => s10_1 = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            s10_1: s10_1.ok_or_else(|| "missing --s10-1".to_owned())?,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn s10_2_catalog_and_governed_metric_names_are_frozen() {
        assert_eq!(["centered-stack-8", "centered-stack-16"].len(), 2);
        let names = physbox_lab::stationarity::StationarityMetricMaximaV1 {
            normal_point_amplitude_max: 0.0,
            normal_point_variance_max: 0.0,
            normal_manifold_amplitude_max: 0.0,
            normal_manifold_variance_max: 0.0,
            tangent_point_amplitude_max: 0.0,
            tangent_manifold_amplitude_max: 0.0,
            active_set_flip_count_max: 0,
            redistribution_transition_max: 0.0,
        }
        .named_values();
        assert_eq!(names.len(), 8);
        assert!(names.contains_key("active-set-flip-count-max"));
        assert!(names.contains_key("redistribution-transition-max"));
    }
}
