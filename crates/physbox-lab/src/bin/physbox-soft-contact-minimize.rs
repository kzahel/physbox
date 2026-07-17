use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactIdentityPolicyV1, ContactSoftnessPolicyV1, FiniteF32,
    SCHEMA_VERSION, ScenarioV1, SettlingSeriesV2, Validate, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, P5_2_SETTLING_EVIDENCE_STEPS, P5_2SettlingFixtureKind,
    p5_2_centered_rectangle_stack_scenario, p5_2_settling_fixture_scenario,
};
use physbox_lab::{
    failure_automation::minimize_dense_generated_failure,
    settling::{
        awake_settling_measurement_policy_v2, centered_stack_structure,
        observe_generated_settling_series_v2, settling_fixture_structure, settling_series_v2,
    },
};
use serde::Serialize;

const FACTORIAL_MANIFEST: &str = "evidence/p5.2b-s6.4-soft-contact/manifest.json";
const IDENTITY_POLICY: ContactIdentityPolicyV1 = ContactIdentityPolicyV1::PriorContactHysteresis;

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
    candidate_id: &'static str,
    identity_policy: ContactIdentityPolicyV1,
    softness: ContactSoftnessPolicyV1,
    stack_sweep: Vec<FailureEvaluation>,
    minimum_failing_stack_count: Option<u32>,
    mixed_pile_original: FailureEvaluation,
    mixed_pile_minimized: FailureEvaluation,
    mixed_pile_minimization_attempts: u32,
    minimized_scenario: ArtifactIdentity,
    minimized_trace: ArtifactIdentity,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    product_constructor_changed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureEvaluation {
    label: String,
    source_scenario_hash: String,
    requested_steps: u32,
    body_count: u32,
    dynamic_body_count: u32,
    exact_replay: bool,
    capacity_clean: bool,
    structural_pass: Option<bool>,
    energy_envelope_nonincreasing: bool,
    settled_linear_speed_max: f32,
    settled_angular_edge_speed_max: f32,
    first_energy_quartile_max: f32,
    last_energy_quartile_max: f32,
    positive_energy_run_max: u32,
    failure_reproduced: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

struct EvaluatedTrace {
    evidence: FailureEvaluation,
    trace: SettlingSeriesV2,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-soft-contact-minimize error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S6.4 failure minimization requires a clean source tree".to_owned());
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
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("failure output or staging directory already exists".to_owned());
    }
    fs::create_dir_all(&staging).map_err(display_error)?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut stack_sweep = Vec::new();
    for count in 8..=16 {
        eprintln!("S6.4 minimize stack {count}");
        let mut scenario = minimized_stack_source(count)?;
        scenario.settings.contact_softness = Some(softness());
        stack_sweep.push(
            evaluate(
                &runner,
                &format!("centered-stack-{count}"),
                &scenario,
                Structure::Stack,
                true,
            )?
            .evidence,
        );
    }
    let minimum_failing_stack_count = stack_sweep
        .iter()
        .find(|evaluation| evaluation.failure_reproduced)
        .map(|evaluation| evaluation.dynamic_body_count);

    let mut mixed = p5_2_settling_fixture_scenario(
        P5_2SettlingFixtureKind::SmallMixedPile,
        0x6002,
        P5_2_SETTLING_EVIDENCE_STEPS,
    )
    .map_err(display_error)?;
    mixed.settings.contact_softness = Some(softness());
    let mixed_original = evaluate(
        &runner,
        "small-mixed-pile-seed-6002",
        &mixed,
        Structure::Fixture(P5_2SettlingFixtureKind::SmallMixedPile),
        true,
    )?;
    if !mixed_original.evidence.failure_reproduced {
        return Err("the published mixed-pile energy failure did not reproduce".to_owned());
    }
    let mut evaluation_error = None;
    let minimized = minimize_dense_generated_failure(mixed, |candidate| {
        if candidate.requested_steps < 900 {
            return false;
        }
        eprintln!(
            "S6.4 minimize mixed bodies={} steps={}",
            candidate.bodies.len(),
            candidate.requested_steps
        );
        match evaluate_once(&runner, candidate) {
            Ok(trace) => !trace.summary.energy_envelope_nonincreasing,
            Err(error) => {
                evaluation_error.get_or_insert(error);
                false
            }
        }
    })?
    .ok_or_else(|| "mixed-pile energy failure did not survive minimization".to_owned())?;
    if let Some(error) = evaluation_error {
        return Err(format!("mixed-pile minimization execution failed: {error}"));
    }
    let minimized_evaluation = evaluate(
        &runner,
        "minimized-small-mixed-pile-energy",
        &minimized.scenario,
        Structure::None,
        true,
    )?;
    if !minimized_evaluation.evidence.failure_reproduced {
        return Err("minimized mixed-pile energy failure did not reproduce".to_owned());
    }
    let minimized_scenario = write_json_artifact(
        &staging,
        "mixed-pile-minimized-scenario.json",
        &minimized.scenario,
    )?;
    let minimized_trace = write_gzip_json_artifact(
        &staging,
        "mixed-pile-minimized-trace.json.gz",
        &minimized_evaluation.trace,
    )?;

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s6.4-soft-contact-failure-minima",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        factorial_manifest: ArtifactIdentity {
            path: FACTORIAL_MANIFEST.to_owned(),
            sha256: sha256_bytes(&factorial_bytes),
        },
        candidate_id: "soft-hz30-d10-static1-prior",
        identity_policy: IDENTITY_POLICY,
        softness: softness(),
        stack_sweep,
        minimum_failing_stack_count,
        mixed_pile_original: mixed_original.evidence,
        mixed_pile_minimized: minimized_evaluation.evidence,
        mixed_pile_minimization_attempts: minimized.attempts,
        minimized_scenario,
        minimized_trace,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        product_constructor_changed: false,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(display_error)?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    let manifest_bytes = fs::read(arguments.output.join("manifest.json")).map_err(display_error)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s6.4-soft-contact-minima run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn minimized_stack_source(count: u32) -> Result<ScenarioV1, String> {
    if !(8..=16).contains(&count) {
        return Err("S6.4 stack minimization is bounded to depths 8 through 16".to_owned());
    }
    let mut scenario = p5_2_centered_rectangle_stack_scenario(16, P5_2_SETTLING_EVIDENCE_STEPS)
        .map_err(display_error)?;
    scenario
        .bodies
        .retain(|body| body.inverse_mass.get() == 0.0 || body.id.slot <= count);
    if scenario.bodies.len() != count as usize + 1
        || scenario
            .bodies
            .iter()
            .enumerate()
            .any(|(slot, body)| body.id.slot != slot as u32)
    {
        return Err("truncated stack did not preserve dense floor/body slots".to_owned());
    }
    scenario.validate().map_err(display_error)?;
    Ok(scenario)
}

#[derive(Clone, Copy)]
enum Structure {
    Stack,
    Fixture(P5_2SettlingFixtureKind),
    None,
}

fn evaluate(
    runner: &GpuGeneratedContactRunner,
    label: &str,
    scenario: &ScenarioV1,
    structure: Structure,
    replay: bool,
) -> Result<EvaluatedTrace, String> {
    let trace = evaluate_once(runner, scenario)?;
    let replay_exact = if replay {
        let second = evaluate_once(runner, scenario)?;
        serialized_hash(&trace)? == serialized_hash(&second)?
    } else {
        true
    };
    let raw = runner.run_contact_policy_observation_series(
        scenario,
        1,
        scenario.requested_steps,
        IDENTITY_POLICY,
    )?;
    let capacity_clean = raw.snapshots.iter().all(|snapshot| {
        snapshot.failure.is_none()
            && snapshot
                .capacities
                .iter()
                .all(|capacity| capacity.accepted && !capacity.overflowed)
    });
    let final_bodies = &raw.snapshots.last().expect("nonempty series").bodies;
    let structural_pass = match structure {
        Structure::Stack => {
            Some(centered_stack_structure(scenario, final_bodies, !capacity_clean, 0.0).0)
        }
        Structure::Fixture(kind) => {
            Some(settling_fixture_structure(kind, scenario, final_bodies, !capacity_clean, 0.0).0)
        }
        Structure::None => None,
    };
    Ok(EvaluatedTrace {
        evidence: FailureEvaluation {
            label: label.to_owned(),
            source_scenario_hash: canonical_hash(scenario).map_err(display_error)?,
            requested_steps: scenario.requested_steps,
            body_count: scenario.bodies.len() as u32,
            dynamic_body_count: scenario
                .bodies
                .iter()
                .filter(|body| body.inverse_mass.get() > 0.0)
                .count() as u32,
            exact_replay: replay_exact,
            capacity_clean,
            structural_pass,
            energy_envelope_nonincreasing: trace.summary.energy_envelope_nonincreasing,
            settled_linear_speed_max: trace.summary.settled_linear_speed_max.get(),
            settled_angular_edge_speed_max: trace.summary.settled_angular_edge_speed_max.get(),
            first_energy_quartile_max: trace.summary.first_energy_quartile_max.get(),
            last_energy_quartile_max: trace.summary.last_energy_quartile_max.get(),
            positive_energy_run_max: trace.summary.positive_energy_run_max,
            failure_reproduced: !trace.summary.energy_envelope_nonincreasing,
        },
        trace,
    })
}

fn evaluate_once(
    runner: &GpuGeneratedContactRunner,
    scenario: &ScenarioV1,
) -> Result<SettlingSeriesV2, String> {
    let raw = runner.run_contact_policy_observation_series(
        scenario,
        1,
        scenario.requested_steps,
        IDENTITY_POLICY,
    )?;
    let observations = observe_generated_settling_series_v2(scenario, &raw)?;
    settling_series_v2(
        scenario,
        None,
        "soft-hz30-d10-static1-prior-minimization",
        awake_settling_measurement_policy_v2(),
        observations,
        true,
        "failure minimization evaluates the frozen energy signature".to_owned(),
    )
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
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(display_error)?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder.write_all(&bytes).map_err(display_error)?;
    write_artifact(root, relative, &encoder.finish().map_err(display_error)?)
}

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    atomic_write(&root.join(relative), bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
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
    FiniteF32::new(value).expect("failure-minimization constants are finite")
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
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
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimized_candidate_is_the_factorial_leader() {
        let policy = softness();
        assert_eq!(policy.contact_hertz.get(), 30.0);
        assert_eq!(policy.damping_ratio.get(), 10.0);
        assert_eq!(policy.static_contact_hertz_multiplier.get(), 1.0);
        assert_eq!(
            IDENTITY_POLICY,
            ContactIdentityPolicyV1::PriorContactHysteresis
        );
        assert_eq!(minimized_stack_source(9).unwrap().bodies.len(), 10);
        assert!(minimized_stack_source(7).is_err());
    }
}
