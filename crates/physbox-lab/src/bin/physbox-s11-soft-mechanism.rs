use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    ContactIdentityPolicyV1, SCHEMA_VERSION, ScenarioV1, SettlingSummaryV2, canonical_hash,
    sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactPassTraceV3, GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1,
    GpuSoftFeedbackPolicyV1, GpuSolverExecutionV4, GpuSolverPassKindV1,
    GpuSolverSchedulingPolicyV1, P5_2_SETTLING_EVIDENCE_STEPS,
    p5_2_minimized_mixed_pile_energy_scenario, p5_2_s11_soft_composition_reproducer_scenario,
    p5_2_s11_soft_depth_sweep_scenario,
};
use physbox_lab::{
    pass_energy::{
        SoftCrossStepImpulseHandoffV1, SoftImpulseOperatorFamilyV1, SoftImpulseOperatorReportV1,
        SoftNormalCompositionV1, analyze_soft_impulse_operator,
    },
    settling::{
        awake_settling_measurement_policy_v2, centered_stack_structure,
        observe_generated_settling_series_v2, summarize_settling_v2,
    },
};
use serde::Serialize;

const TARGET_STEP: u32 = 1_100;
const SUBMISSION_BATCH_STEPS: u32 = 64;
const FULL_DEPTH_CATALOG: [u32; 11] = [2, 4, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const SCALAR_MAX_ABSOLUTE_DIFFERENCE: f32 = 2.0e-5;
const S11_1_MANIFEST_PATH: &str = "evidence/p5.2b-s11.1-soft-pass-attribution/manifest.json";
const S11_1_MANIFEST_SHA256: &str =
    "632fea781378572ce075e4c52c0902f63a4da8e6c07e8fc4a621941fd70f806d";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
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
    host: &'static str,
    target_step: u32,
    input_s11_1: ArtifactIdentity,
    depth_catalog: Vec<u32>,
    depths: Vec<CaseEvidence>,
    four_body: Option<CaseEvidence>,
    smallest_rising_energy_depth: Option<u32>,
    smallest_primary_signature_depth: Option<u32>,
    permanent_reproducer_source_hash: Option<String>,
    permanent_reproducer_exact_alias: bool,
    signature_contract: SignatureContract,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignatureContract {
    target_step: u32,
    requires_rising_energy_mode: bool,
    requires_material_normal_biased_delta: bool,
    requires_oscillating_normal_biased_maps: bool,
    requires_contracting_normal_relaxation_maps: bool,
    requires_three_exact_first_prepared_reseeds: bool,
    requires_exact_outer_step_handoff: bool,
    requires_discarded_within_step_impulse: bool,
    material_absolute_epsilon: f64,
    material_relative_epsilon: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseEvidence {
    label: String,
    depth: Option<u32>,
    role: &'static str,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    capture: ArtifactIdentity,
    observation_series_hash: String,
    replay_observation_series_hash: String,
    exact_observation_replay: bool,
    previous_trace_hash: String,
    replay_previous_trace_hash: String,
    exact_previous_trace_replay: bool,
    target_trace_hash: String,
    replay_target_trace_hash: String,
    exact_target_trace_replay: bool,
    operator_report_hash: String,
    replay_operator_report_hash: String,
    exact_operator_replay: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_failure_step: Option<u32>,
    maximum_scalar_absolute_difference: f32,
    settling_summary: SettlingSummaryV2,
    rising_energy_mode: bool,
    families: Vec<SoftImpulseOperatorFamilyV1>,
    cross_step_handoff: SoftCrossStepImpulseHandoffV1,
    composition: SoftNormalCompositionV1,
    primary_signature: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseCapture {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    label: String,
    source_scenario_hash: String,
    source_scenario: ScenarioV1,
    observation_series: GpuGeneratedContactSeriesV1,
    previous_trace: GpuGeneratedContactPassTraceV3,
    target_trace: GpuGeneratedContactPassTraceV3,
    operator: SoftImpulseOperatorReportV1,
}

#[derive(Clone, Copy)]
enum StructureKind {
    CenteredStack,
    EnergyOnly,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s11-soft-mechanism error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S11.2 mechanism evidence requires a clean source tree".to_owned());
    }
    if arguments.require_clean
        && (arguments.depths.as_slice() != FULL_DEPTH_CATALOG || arguments.skip_four_body)
    {
        return Err(
            "clean S11.2 evidence requires the complete depth catalog and four-body context"
                .to_owned(),
        );
    }
    let s11_1_bytes =
        fs::read(repository.join(S11_1_MANIFEST_PATH)).map_err(|error| error.to_string())?;
    if sha256_bytes(&s11_1_bytes) != S11_1_MANIFEST_SHA256 {
        return Err("S11.1 input manifest digest changed".to_owned());
    }
    let staging = prepare_staging(&arguments.output, &["sources", "captures"])?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut depths = Vec::new();
    for &depth in &arguments.depths {
        let label = format!("soft-centered-stack-{depth}");
        eprintln!("S11.2 mechanism case {label}");
        let source =
            p5_2_s11_soft_depth_sweep_scenario(depth).map_err(|error| error.to_string())?;
        depths.push(run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            &runner,
            source,
            label,
            Some(depth),
            "depth-minimization",
            StructureKind::CenteredStack,
        )?);
    }
    let four_body = if arguments.skip_four_body {
        None
    } else {
        eprintln!("S11.2 mechanism case permanent-four-body-soft");
        Some(run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            &runner,
            p5_2_minimized_mixed_pile_energy_scenario().map_err(|error| error.to_string())?,
            "permanent-four-body-soft".to_owned(),
            None,
            "required-secondary-context",
            StructureKind::EnergyOnly,
        )?)
    };
    let smallest_primary_signature_depth = depths
        .iter()
        .find(|case| case.primary_signature)
        .and_then(|case| case.depth);
    let smallest_rising_energy_depth = depths
        .iter()
        .find(|case| case.rising_energy_mode)
        .and_then(|case| case.depth);
    let permanent_reproducer =
        p5_2_s11_soft_composition_reproducer_scenario().map_err(|error| error.to_string())?;
    let permanent_reproducer_source_hash =
        canonical_hash(&permanent_reproducer).map_err(|error| error.to_string())?;
    let permanent_reproducer_exact_alias = smallest_primary_signature_depth == Some(11)
        && depths
            .iter()
            .find(|case| case.depth == Some(11))
            .is_some_and(|case| case.source_scenario_hash == permanent_reproducer_source_hash);
    if arguments.depths.as_slice() == FULL_DEPTH_CATALOG && !permanent_reproducer_exact_alias {
        return Err(
            "S11.2 permanent reproducer no longer aliases the smallest signature".to_owned(),
        );
    }
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s11.2-soft-operator-depth-sweep",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-headless-metal",
        target_step: TARGET_STEP,
        input_s11_1: ArtifactIdentity {
            path: S11_1_MANIFEST_PATH.to_owned(),
            sha256: S11_1_MANIFEST_SHA256.to_owned(),
        },
        depth_catalog: arguments.depths,
        depths,
        four_body,
        smallest_rising_energy_depth,
        smallest_primary_signature_depth,
        permanent_reproducer_source_hash: Some(permanent_reproducer_source_hash),
        permanent_reproducer_exact_alias,
        signature_contract: SignatureContract {
            target_step: TARGET_STEP,
            requires_rising_energy_mode: true,
            requires_material_normal_biased_delta: true,
            requires_oscillating_normal_biased_maps: true,
            requires_contracting_normal_relaxation_maps: true,
            requires_three_exact_first_prepared_reseeds: true,
            requires_exact_outer_step_handoff: true,
            requires_discarded_within_step_impulse: true,
            material_absolute_epsilon: 1.0e-12,
            material_relative_epsilon: 1.0e-5,
        },
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    publish_manifest(&staging, &arguments.output, &manifest)?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s11.2-soft-operator-depth-sweep run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    arguments: &Arguments,
    source_revision: &str,
    source_dirty: bool,
    staging: &Path,
    runner: &GpuGeneratedContactRunner,
    source: ScenarioV1,
    label: String,
    depth: Option<u32>,
    role: &'static str,
    structure_kind: StructureKind,
) -> Result<CaseEvidence, String> {
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let source_artifact = write_json_artifact(staging, &format!("sources/{label}.json"), &source)?;
    let observation_series = run_observation_series(runner, &source)?;
    let replay_observation_series = run_observation_series(runner, &source)?;
    let observation_series_hash = serialized_hash(&observation_series)?;
    let replay_observation_series_hash = serialized_hash(&replay_observation_series)?;
    let exact_observation_replay = observation_series_hash == replay_observation_series_hash;
    let capacity_clean = observation_series
        .snapshots
        .iter()
        .chain(&replay_observation_series.snapshots)
        .all(snapshot_capacity_clean);
    let (structural_pass, structural_reason, first_structural_failure_step) =
        structure_verdict(structure_kind, &source, &observation_series)?;
    let observations = observe_generated_settling_series_v2(&source, &observation_series)?;
    let settling_summary =
        summarize_settling_v2(&observations, awake_settling_measurement_policy_v2())?;
    let rising_energy_mode = rising_energy_mode(&settling_summary);

    let previous_trace = run_trace(runner, &source, TARGET_STEP - 1)?;
    let replay_previous_trace = run_trace(runner, &source, TARGET_STEP - 1)?;
    let target_trace = run_trace(runner, &source, TARGET_STEP)?;
    let replay_target_trace = run_trace(runner, &source, TARGET_STEP)?;
    let previous_trace_hash = serialized_hash(&previous_trace)?;
    let replay_previous_trace_hash = serialized_hash(&replay_previous_trace)?;
    let target_trace_hash = serialized_hash(&target_trace)?;
    let replay_target_trace_hash = serialized_hash(&replay_target_trace)?;
    let exact_previous_trace_replay = previous_trace_hash == replay_previous_trace_hash;
    let exact_target_trace_replay = target_trace_hash == replay_target_trace_hash;
    let operator = analyze_soft_impulse_operator(&source, &previous_trace, &target_trace)?;
    let replay_operator =
        analyze_soft_impulse_operator(&source, &replay_previous_trace, &replay_target_trace)?;
    let operator_report_hash = serialized_hash(&operator)?;
    let replay_operator_report_hash = serialized_hash(&replay_operator)?;
    let exact_operator_replay = operator_report_hash == replay_operator_report_hash;
    let maximum_scalar_absolute_difference = operator
        .rows
        .iter()
        .filter_map(|row| row.scalar_replay_state_difference)
        .map(|difference| difference.max_absolute_difference)
        .fold(0.0_f32, f32::max);
    let normal_biased = operator
        .families
        .iter()
        .find(|family| family.kind == GpuSolverPassKindV1::NormalBiased)
        .ok_or_else(|| "soft operator omitted normal-biased family".to_owned())?;
    let normal_relaxation = operator
        .families
        .iter()
        .find(|family| family.kind == GpuSolverPassKindV1::NormalRelaxation)
        .ok_or_else(|| "soft operator omitted normal-relaxation family".to_owned())?;
    let primary_signature = rising_energy_mode
        && operator.composition.normal_biased_materially_positive
        && normal_biased.oscillation_maps == normal_biased.pass_count
        && normal_relaxation.contraction_maps == normal_relaxation.pass_count
        && operator.composition.exact_first_prepared_reseeds == 3
        && operator.cross_step_handoff.exact_slot_and_impulse_handoff
        && operator.composition.discarded_prior_impulse_l1 > 0.0;
    if !exact_observation_replay
        || !exact_previous_trace_replay
        || !exact_target_trace_replay
        || !exact_operator_replay
        || !capacity_clean
        || !structural_pass
        || maximum_scalar_absolute_difference > SCALAR_MAX_ABSOLUTE_DIFFERENCE
    {
        return Err(format!(
            "S11.2 {label} failed replay, capacity, structure, or scalar gates"
        ));
    }

    let capture = CaseCapture {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s11.2-soft-operator-capture",
        run_id: arguments.run_id.clone(),
        source_revision: source_revision.to_owned(),
        source_dirty,
        label: label.clone(),
        source_scenario_hash: source_scenario_hash.clone(),
        source_scenario: source,
        observation_series,
        previous_trace,
        target_trace,
        operator: operator.clone(),
    };
    let capture_artifact =
        write_gzip_json_artifact(staging, &format!("captures/{label}.json.gz"), &capture)?;
    Ok(CaseEvidence {
        label,
        depth,
        role,
        source_scenario_hash,
        source: source_artifact,
        capture: capture_artifact,
        observation_series_hash,
        replay_observation_series_hash,
        exact_observation_replay,
        previous_trace_hash,
        replay_previous_trace_hash,
        exact_previous_trace_replay,
        target_trace_hash,
        replay_target_trace_hash,
        exact_target_trace_replay,
        operator_report_hash,
        replay_operator_report_hash,
        exact_operator_replay,
        capacity_clean,
        structural_pass,
        structural_reason,
        first_structural_failure_step,
        maximum_scalar_absolute_difference,
        settling_summary,
        rising_energy_mode,
        families: operator.families,
        cross_step_handoff: operator.cross_step_handoff,
        composition: operator.composition,
        primary_signature,
    })
}

fn run_observation_series(
    runner: &GpuGeneratedContactRunner,
    source: &ScenarioV1,
) -> Result<GpuGeneratedContactSeriesV1, String> {
    runner.run_contact_policy_observation_series_with_execution_scheduling_and_soft_feedback(
        source,
        1,
        P5_2_SETTLING_EVIDENCE_STEPS,
        ContactIdentityPolicyV1::PriorContactHysteresis,
        SUBMISSION_BATCH_STEPS,
        GpuSolverExecutionV4::SELECTED,
        GpuSolverSchedulingPolicyV1::SELECTED,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )
}

fn run_trace(
    runner: &GpuGeneratedContactRunner,
    source: &ScenarioV1,
    target_step: u32,
) -> Result<GpuGeneratedContactPassTraceV3, String> {
    runner.run_contact_policy_pass_trace(
        source,
        target_step,
        ContactIdentityPolicyV1::PriorContactHysteresis,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )
}

fn structure_verdict(
    kind: StructureKind,
    source: &ScenarioV1,
    series: &GpuGeneratedContactSeriesV1,
) -> Result<(bool, String, Option<u32>), String> {
    let mut first_failure = None;
    let mut final_reason = String::new();
    for snapshot in &series.snapshots {
        let capacity_failed = !snapshot_capacity_clean(snapshot);
        let (passed, reason) = match kind {
            StructureKind::CenteredStack => {
                centered_stack_structure(source, &snapshot.bodies, capacity_failed, 0.0)
            }
            StructureKind::EnergyOnly => (
                !capacity_failed,
                if capacity_failed {
                    "energy reproducer exceeded capacity"
                } else {
                    "energy-only reproducer has no structural acceptance gate"
                }
                .to_owned(),
            ),
        };
        if !passed && first_failure.is_none() {
            first_failure = Some(snapshot.completed_steps);
        }
        final_reason = reason;
    }
    if final_reason.is_empty() {
        return Err("S11.2 observation series has no structural snapshots".to_owned());
    }
    Ok((first_failure.is_none(), final_reason, first_failure))
}

fn rising_energy_mode(summary: &SettlingSummaryV2) -> bool {
    !summary.energy_envelope_nonincreasing
        && summary.positive_energy_run_max > 5
        && summary.last_energy_quartile_max.get() > summary.first_energy_quartile_max.get()
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

struct Arguments {
    run_id: String,
    output: PathBuf,
    depths: Vec<u32>,
    skip_four_body: bool,
    require_clean: bool,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut depths = FULL_DEPTH_CATALOG.to_vec();
        let mut skip_four_body = false;
        let mut require_clean = false;
        let mut arguments = arguments;
        while let Some(argument) = arguments.next() {
            if argument == "--require-clean" {
                require_clean = true;
                continue;
            }
            if argument == "--skip-four-body" {
                skip_four_body = true;
                continue;
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--depths" => depths = parse_depths(&value)?,
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            depths,
            skip_four_body,
            require_clean,
        })
    }
}

fn parse_depths(value: &str) -> Result<Vec<u32>, String> {
    let depths = value
        .split(',')
        .map(|item| {
            item.parse::<u32>()
                .map_err(|error| format!("invalid S11.2 depth {item}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if depths.is_empty()
        || depths.windows(2).any(|pair| pair[0] >= pair[1])
        || depths
            .iter()
            .any(|depth| !FULL_DEPTH_CATALOG.contains(depth))
    {
        return Err("S11.2 depths must be a sorted unique subset of the frozen catalog".to_owned());
    }
    Ok(depths)
}

fn prepare_staging(output: &Path, directories: &[&str]) -> Result<PathBuf, String> {
    let mut staging_name = output.as_os_str().to_owned();
    staging_name.push(".partial");
    let staging = PathBuf::from(staging_name);
    if output.exists() || staging.exists() {
        return Err(format!(
            "refusing to replace S11.2 output or staging {}",
            output.display()
        ));
    }
    for directory in directories {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }
    Ok(staging)
}

fn publish_manifest(
    staging: &Path,
    output: &Path,
    manifest: &impl Serialize,
) -> Result<(), String> {
    atomic_json(&staging.join("manifest.json"), manifest)?;
    sync_directory(staging)?;
    fs::rename(staging, output).map_err(|error| error.to_string())?;
    sync_directory(output.parent().unwrap_or(Path::new(".")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_parser_accepts_only_sorted_frozen_subsets() {
        assert_eq!(parse_depths("2,4,16").unwrap(), vec![2, 4, 16]);
        assert!(parse_depths("4,2").is_err());
        assert!(parse_depths("2,2").is_err());
        assert!(parse_depths("3").is_err());
    }
}
