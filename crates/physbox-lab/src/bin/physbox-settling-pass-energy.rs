use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    ContactIdentityPolicyV1, OracleRequestV1, OracleResultV1, SCHEMA_VERSION, ScenarioV1,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactPassTraceV1, GpuGeneratedContactRunner, GpuSoftFeedbackPolicyV1,
    p5_2_box2d_world_projection, p5_2_minimized_mixed_pile_energy_scenario,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
};
use physbox_lab::pass_energy::{
    KineticEnergyAccountingV1, SETTLING_PASS_ENERGY_REPORT_VERSION, SettlingPassEnergyReportV1,
    analyze_settling_pass_energy, kinetic_energy,
};
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 1;
const SUCCESS_SENTINEL: &str = "PHYSBOX5_SETTLING_PASS_ENERGY_SUCCESS";
const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    schema_version: u32,
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    scenario_name: &'static str,
    target_step: u32,
    matched_state_precision: &'static str,
    mapped_after_final_step_only: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
    product_constructor_changed: bool,
    source_scenario: ScenarioV1,
    trace: GpuGeneratedContactPassTraceV1,
    energy: SettlingPassEnergyReportV1,
    box2d: Box2dBehaviorComparison,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Box2dBehaviorComparison {
    authority: SourceIdentity,
    lane: &'static str,
    projection_policy: &'static str,
    before: Box2dSample,
    after: Box2dSample,
    kinetic_energy_delta_ordered_f32: f32,
    kinetic_energy_delta_compensated_f64: f64,
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
struct Box2dSample {
    completed_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    result_sha256: String,
    kinetic_energy: KineticEnergyAccountingV1,
    result: OracleResultV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-settling-pass-energy error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let source = p5_2_minimized_mixed_pile_energy_scenario().map_err(|error| error.to_string())?;
    if arguments.target_step == 0 || arguments.target_step > source.requested_steps {
        return Err(format!(
            "target step must be in 1..={}",
            source.requested_steps
        ));
    }
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let trace = runner.run_contact_policy_pass_trace(
        &source,
        arguments.target_step,
        ContactIdentityPolicyV1::PriorContactHysteresis,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )?;
    let energy = analyze_settling_pass_energy(&source, &trace)?;
    if energy.version != SETTLING_PASS_ENERGY_REPORT_VERSION {
        return Err("pass-energy report version changed unexpectedly".to_owned());
    }
    let box2d_source = verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION)
        .map_err(|error| error.to_string())?;
    let box2d = run_box2d_comparison(&arguments, &source, &box2d_source)?;
    let adapter_marker = trace.adapter.marker_json();
    let artifact = Artifact {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "p5.2b-s6.5b-settling-pass-energy",
        run_id: arguments.run_id,
        source_revision: arguments.source_revision,
        source_dirty: arguments.source_dirty,
        host: if cfg!(target_os = "android") {
            "android-native-vulkan"
        } else {
            "native-headless"
        },
        scenario_name: "minimized-mixed-candidate",
        target_step: arguments.target_step,
        matched_state_precision: "f32",
        mapped_after_final_step_only: true,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
        product_constructor_changed: false,
        source_scenario: source,
        trace,
        energy,
        box2d,
    };
    let hash = write_gzip_json_atomic(&arguments.output, &artifact)?;
    println!("PHYSBOX5_SETTLING_PASS_ENERGY_ADAPTER {adapter_marker}");
    println!(
        "{SUCCESS_SENTINEL} target_step={} sha256={} output={}",
        arguments.target_step,
        hash,
        arguments.output.display()
    );
    Ok(())
}

struct Arguments {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    target_step: u32,
    box2d_source: PathBuf,
    oracle: PathBuf,
    oracle_artifact_root: PathBuf,
    output: PathBuf,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
        let mut source_revision = None;
        let mut source_dirty = None;
        let mut target_step = None;
        let mut box2d_source = None;
        let mut oracle = None;
        let mut oracle_artifact_root = None;
        let mut output = None;
        let mut arguments = arguments;
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--source-revision" => source_revision = Some(value),
                "--source-dirty" => source_dirty = Some(parse_bool(&value)?),
                "--target-step" => {
                    target_step = Some(
                        value
                            .parse::<u32>()
                            .map_err(|error| format!("invalid target step: {error}"))?,
                    )
                }
                "--box2d-source" => box2d_source = Some(PathBuf::from(value)),
                "--oracle" => oracle = Some(PathBuf::from(value)),
                "--oracle-artifact-root" => {
                    oracle_artifact_root = Some(PathBuf::from(value));
                }
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            source_revision: source_revision
                .ok_or_else(|| "missing --source-revision".to_owned())?,
            source_dirty: source_dirty.ok_or_else(|| "missing --source-dirty".to_owned())?,
            target_step: target_step.ok_or_else(|| "missing --target-step".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            oracle_artifact_root: oracle_artifact_root
                .ok_or_else(|| "missing --oracle-artifact-root".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
        })
    }
}

fn run_box2d_comparison(
    arguments: &Arguments,
    source: &ScenarioV1,
    authority: &OracleSourceIdentity,
) -> Result<Box2dBehaviorComparison, String> {
    if arguments.target_step < 2 {
        return Err("Box2D before/after comparison requires target step >= 2".to_owned());
    }
    let mut authority_source = source.clone();
    authority_source.id = format!("{}-box2d-rigid-target", source.id);
    authority_source.settings.contact_softness = None;
    let base_projection =
        p5_2_box2d_world_projection(&authority_source).map_err(|error| error.to_string())?;
    let before = run_box2d_sample(
        arguments,
        authority,
        &base_projection,
        arguments.target_step - 1,
    )?;
    let after = run_box2d_sample(
        arguments,
        authority,
        &base_projection,
        arguments.target_step,
    )?;
    Ok(Box2dBehaviorComparison {
        authority: SourceIdentity {
            revision: authority.revision.clone(),
            dirty: authority.dirty,
            patch_digest: authority.patch_digest.clone(),
        },
        lane: "world-awake",
        projection_policy: "clear-candidate-only-softness-use-pinned-box2d-rigid-target",
        kinetic_energy_delta_ordered_f32: after.kinetic_energy.ordered_f32
            - before.kinetic_energy.ordered_f32,
        kinetic_energy_delta_compensated_f64: after.kinetic_energy.compensated_f64
            - before.kinetic_energy.compensated_f64,
        before,
        after,
    })
}

fn run_box2d_sample(
    arguments: &Arguments,
    authority: &OracleSourceIdentity,
    base_projection: &ScenarioV1,
    completed_steps: u32,
) -> Result<Box2dSample, String> {
    let mut projection = base_projection.clone();
    projection.requested_steps = completed_steps;
    let source_scenario_hash = canonical_hash(&projection).map_err(|error| error.to_string())?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-box2d-{completed_steps:04}", arguments.run_id),
        source_scenario_hash: source_scenario_hash.clone(),
        scenario: projection,
    };
    let run = run_oracle(
        &OracleRunConfig {
            artifact_root: arguments.oracle_artifact_root.clone(),
            timeout: Duration::from_secs(20),
            source: authority.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![OsString::from("--lane"), OsString::from("world-awake")],
            },
        },
        &request,
    )
    .map_err(|error| error.to_string())?;
    let result_bytes = fs::read(&run.artifacts.result).map_err(|error| error.to_string())?;
    Ok(Box2dSample {
        completed_steps,
        source_scenario_hash,
        projection_hash: run.projection_hash,
        result_sha256: sha256_bytes(&result_bytes),
        kinetic_energy: kinetic_energy(&run.result.bodies),
        result: run.result,
    })
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean {value}")),
    }
}

fn write_gzip_json_atomic(path: &Path, value: &impl Serialize) -> Result<String, String> {
    if path.exists() {
        return Err("pass-energy output already exists".to_owned());
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
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(&compressed)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    Ok(sha256_bytes(&compressed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_require_exact_provenance_and_target() {
        let parsed = Arguments::parse(
            [
                "--run-id",
                "run",
                "--source-revision",
                "abc",
                "--source-dirty",
                "false",
                "--target-step",
                "900",
                "--box2d-source",
                "reference/box2d",
                "--oracle",
                "target/release/physbox-box2d-oracle",
                "--oracle-artifact-root",
                "/tmp/pass-oracle",
                "--output",
                "/tmp/pass.json.gz",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(parsed.target_step, 900);
        assert!(!parsed.source_dirty);
    }
}
