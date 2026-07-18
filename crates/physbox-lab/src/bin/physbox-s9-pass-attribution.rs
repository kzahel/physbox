use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    ContactIdentityPolicyV1, SCHEMA_VERSION, ScenarioV1, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactPassTraceV3, GpuGeneratedContactRunner, GpuSoftFeedbackPolicyV1,
    GpuSolverPassKindV1, P5_2_SETTLING_EVIDENCE_STEPS, p5_2_centered_rectangle_stack_scenario,
};
use physbox_lab::pass_energy::{
    SettlingPassEnergyReportV1, SettlingStaticSupportAttributionReportV1,
    analyze_settling_pass_energy, analyze_static_support_energy_attribution,
};
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 1;
const MATERIAL_EPSILON: f64 = 1.0e-12;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    schema_version: u32,
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    host: &'static str,
    scenario_name: String,
    source_scenario_hash: String,
    target_step: u32,
    solver_policy: &'static str,
    matched_state_precision: &'static str,
    mapped_after_target_step_only: bool,
    material_epsilon: f64,
    source_scenario: ScenarioV1,
    trace: GpuGeneratedContactPassTraceV3,
    energy: SettlingPassEnergyReportV1,
    static_support_attribution: SettlingStaticSupportAttributionReportV1,
    ranked_pass_families: Vec<PassFamilySummary>,
    routing_signals: RoutingSignals,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PassFamilySummary {
    kind: GpuSolverPassKindV1,
    pass_count: u32,
    injecting_passes: u32,
    net_delta_compensated_f64: f64,
    positive_delta_compensated_f64: f64,
    maximum_positive_delta_compensated_f64: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoutingSignals {
    warm_start_positive_delta_compensated_f64: f64,
    tangent_positive_delta_compensated_f64: f64,
    biased_normal_positive_delta_compensated_f64: f64,
    relaxation_normal_positive_delta_compensated_f64: f64,
    static_warm_start_positive_delta_compensated_f64: f64,
    dynamic_warm_start_positive_delta_compensated_f64: f64,
    static_normal_positive_delta_compensated_f64: f64,
    dynamic_normal_positive_delta_compensated_f64: f64,
    non_gravity_integration_or_refresh_positive_delta_compensated_f64: f64,
    warm_start_material: bool,
    tangent_material: bool,
    static_normal_majority: bool,
    refresh_or_integration_material: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s9-pass-attribution error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S9.2 pass attribution requires a clean source tree".to_owned());
    }
    let rectangle_count = match arguments.scenario.as_str() {
        "centered-stack-8" => 8,
        "centered-stack-16" => 16,
        _ => return Err(format!("unsupported S9.2 scenario {}", arguments.scenario)),
    };
    let source =
        p5_2_centered_rectangle_stack_scenario(rectangle_count, P5_2_SETTLING_EVIDENCE_STEPS)
            .map_err(|error| error.to_string())?;
    if !(900..=1_100).contains(&arguments.target_step) {
        return Err(
            "S9.2 target step must be in the settled diagnostic range 900..=1100".to_owned(),
        );
    }
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let trace = runner.run_contact_policy_pass_trace(
        &source,
        arguments.target_step,
        ContactIdentityPolicyV1::SELECTED,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )?;
    let energy = analyze_settling_pass_energy(&source, &trace)?;
    let static_support_attribution = analyze_static_support_energy_attribution(&trace)?;
    let ranked_pass_families = rank_pass_families(&energy);
    let routing_signals = routing_signals(&ranked_pass_families, &static_support_attribution);
    let adapter_marker = trace.adapter.marker_json();
    let artifact = Artifact {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "p5.2b-s9.2-pass-attribution",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-headless-metal",
        scenario_name: arguments.scenario,
        source_scenario_hash,
        target_step: arguments.target_step,
        solver_policy: "selected-hard-direct-manifold-block-i2",
        matched_state_precision: "f32",
        mapped_after_target_step_only: true,
        material_epsilon: MATERIAL_EPSILON,
        source_scenario: source,
        trace,
        energy,
        static_support_attribution,
        ranked_pass_families,
        routing_signals,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    let hash = write_gzip_json_atomic(&arguments.output, &artifact)?;
    println!("PHYSBOX5_S9_PASS_ATTRIBUTION_ADAPTER {adapter_marker}");
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario={} run_id={} target_step={} sha256={} output={}",
        artifact.scenario_name,
        artifact.run_id,
        artifact.target_step,
        hash,
        arguments.output.display()
    );
    Ok(())
}

fn rank_pass_families(report: &SettlingPassEnergyReportV1) -> Vec<PassFamilySummary> {
    let mut summaries = [
        GpuSolverPassKindV1::IntegrateVelocity,
        GpuSolverPassKindV1::PrepareContacts,
        GpuSolverPassKindV1::RefreshContacts,
        GpuSolverPassKindV1::WarmStart,
        GpuSolverPassKindV1::NormalBiased,
        GpuSolverPassKindV1::TangentBiased,
        GpuSolverPassKindV1::IntegratePosition,
        GpuSolverPassKindV1::NormalRelaxation,
        GpuSolverPassKindV1::TangentRelaxation,
        GpuSolverPassKindV1::Restitution,
    ]
    .map(|kind| {
        let rows = report
            .rows
            .iter()
            .filter(|row| row.descriptor.kind == kind)
            .collect::<Vec<_>>();
        PassFamilySummary {
            kind,
            pass_count: rows.len() as u32,
            injecting_passes: rows
                .iter()
                .filter(|row| row.gpu_delta_compensated_f64 > MATERIAL_EPSILON)
                .count() as u32,
            net_delta_compensated_f64: rows.iter().map(|row| row.gpu_delta_compensated_f64).sum(),
            positive_delta_compensated_f64: rows
                .iter()
                .map(|row| row.gpu_delta_compensated_f64.max(0.0))
                .sum(),
            maximum_positive_delta_compensated_f64: rows
                .iter()
                .map(|row| row.gpu_delta_compensated_f64.max(0.0))
                .fold(0.0_f64, f64::max),
        }
    });
    summaries.sort_by(|left, right| {
        right
            .positive_delta_compensated_f64
            .total_cmp(&left.positive_delta_compensated_f64)
    });
    summaries.to_vec()
}

fn routing_signals(
    families: &[PassFamilySummary],
    attribution: &SettlingStaticSupportAttributionReportV1,
) -> RoutingSignals {
    let positive = |kind| {
        families
            .iter()
            .find(|row| row.kind == kind)
            .map_or(0.0, |row| row.positive_delta_compensated_f64)
    };
    let tangent = positive(GpuSolverPassKindV1::TangentBiased)
        + positive(GpuSolverPassKindV1::TangentRelaxation);
    let warm_start = positive(GpuSolverPassKindV1::WarmStart);
    let biased_normal = positive(GpuSolverPassKindV1::NormalBiased);
    let relaxation_normal = positive(GpuSolverPassKindV1::NormalRelaxation);
    let static_warm_start = attribution
        .rows
        .iter()
        .filter(|row| row.descriptor.kind == GpuSolverPassKindV1::WarmStart)
        .map(|row| row.static_positive_delta_compensated_f64)
        .sum();
    let dynamic_warm_start = attribution
        .rows
        .iter()
        .filter(|row| row.descriptor.kind == GpuSolverPassKindV1::WarmStart)
        .map(|row| row.dynamic_positive_delta_compensated_f64)
        .sum();
    let static_normal = attribution
        .rows
        .iter()
        .filter(|row| {
            matches!(
                row.descriptor.kind,
                GpuSolverPassKindV1::NormalBiased | GpuSolverPassKindV1::NormalRelaxation
            )
        })
        .map(|row| row.static_positive_delta_compensated_f64)
        .sum();
    let dynamic_normal = attribution
        .rows
        .iter()
        .filter(|row| {
            matches!(
                row.descriptor.kind,
                GpuSolverPassKindV1::NormalBiased | GpuSolverPassKindV1::NormalRelaxation
            )
        })
        .map(|row| row.dynamic_positive_delta_compensated_f64)
        .sum();
    let non_gravity_integration_or_refresh = positive(GpuSolverPassKindV1::PrepareContacts)
        + positive(GpuSolverPassKindV1::RefreshContacts)
        + positive(GpuSolverPassKindV1::IntegratePosition);
    RoutingSignals {
        warm_start_positive_delta_compensated_f64: warm_start,
        tangent_positive_delta_compensated_f64: tangent,
        biased_normal_positive_delta_compensated_f64: biased_normal,
        relaxation_normal_positive_delta_compensated_f64: relaxation_normal,
        static_warm_start_positive_delta_compensated_f64: static_warm_start,
        dynamic_warm_start_positive_delta_compensated_f64: dynamic_warm_start,
        static_normal_positive_delta_compensated_f64: static_normal,
        dynamic_normal_positive_delta_compensated_f64: dynamic_normal,
        non_gravity_integration_or_refresh_positive_delta_compensated_f64:
            non_gravity_integration_or_refresh,
        warm_start_material: warm_start > MATERIAL_EPSILON,
        tangent_material: tangent > MATERIAL_EPSILON,
        static_normal_majority: static_normal > dynamic_normal,
        refresh_or_integration_material: non_gravity_integration_or_refresh > MATERIAL_EPSILON,
    }
}

struct Arguments {
    run_id: String,
    scenario: String,
    target_step: u32,
    output: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
        let mut scenario = None;
        let mut target_step = None;
        let mut output = None;
        let mut require_clean = false;
        let mut arguments = arguments;
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
                "--scenario" => scenario = Some(value),
                "--target-step" => {
                    target_step = Some(
                        value
                            .parse::<u32>()
                            .map_err(|error| format!("invalid target step: {error}"))?,
                    );
                }
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            scenario: scenario.ok_or_else(|| "missing --scenario".to_owned())?,
            target_step: target_step.ok_or_else(|| "missing --target-step".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            require_clean,
        })
    }
}

fn write_gzip_json_atomic(path: &Path, value: &impl Serialize) -> Result<String, String> {
    if path.exists() || path.with_extension("partial").exists() {
        return Err("S9.2 output already exists".to_owned());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    let staging = path.with_extension("partial");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(|error| error.to_string())?;
    file.write_all(&compressed)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(&staging, path).map_err(|error| error.to_string())?;
    Ok(sha256_bytes(&compressed))
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
    fn arguments_require_the_frozen_scene_step_and_output_identity() {
        assert!(Arguments::parse(std::iter::empty()).is_err());
        let parsed = Arguments::parse(
            [
                "--run-id",
                "run",
                "--scenario",
                "centered-stack-16",
                "--target-step",
                "900",
                "--output",
                "/tmp/s9.json.gz",
                "--require-clean",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(parsed.target_step, 900);
        assert!(parsed.require_clean);
    }
}
