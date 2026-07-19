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
    GpuGeneratedContactPassMotionWindowV1, GpuGeneratedContactPassTraceV3,
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, GpuSoftFeedbackPolicyV1,
    GpuSolverExecutionV4, GpuSolverPassKindV1, GpuSolverSchedulingPolicyV1,
    P5_2_SETTLING_EVIDENCE_STEPS, p5_2_minimized_mixed_pile_energy_scenario,
    p5_2_settling_soft_stack_matrix_scenario,
};
use physbox_lab::{
    pass_energy::{
        SettlingPassEnergyReportV1, SettlingPassEnergyWindowReportV1,
        SettlingStaticSupportAttributionReportV1, analyze_settling_pass_energy,
        analyze_settling_pass_energy_window, analyze_static_support_energy_attribution,
    },
    settling::{
        awake_settling_measurement_policy_v2, centered_stack_structure,
        observe_generated_settling_series_v2, summarize_settling_v2,
    },
};
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 1;
const FIRST_WINDOW_STEP: u32 = 900;
const LAST_WINDOW_STEP: u32 = 1_200;
const ANCHOR_STEPS: [u32; 3] = [900, 1_000, 1_100];
const SUBMISSION_BATCH_STEPS: u32 = 64;
const SCALAR_MAX_ABSOLUTE_DIFFERENCE: f32 = 2.0e-5;
const SUPPORT_ACCOUNTING_TOLERANCE: f64 = 1.0e-7;

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
    scenario_role: &'static str,
    source_scenario_hash: String,
    source_scenario: ScenarioV1,
    observation_series: GpuGeneratedContactSeriesV1,
    observation_series_hash: String,
    replay_observation_series_hash: String,
    exact_observation_replay: bool,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    first_structural_failure_step: Option<u32>,
    settling_summary: SettlingSummaryV2,
    rising_energy_mode: bool,
    motion_window: GpuGeneratedContactPassMotionWindowV1,
    motion_window_hash: String,
    replay_motion_window_hash: String,
    exact_motion_window_replay: bool,
    motion_window_report: SettlingPassEnergyWindowReportV1,
    anchors: Vec<AnchorEvidence>,
    all_anchor_motions_exact: bool,
    all_anchor_traces_exact_replay: bool,
    scalar_mirror_within_tolerance: bool,
    support_accounting_within_tolerance: bool,
    consistent_material_injectors: Vec<GpuSolverPassKindV1>,
    routing_verdict: RoutingVerdict,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnchorEvidence {
    completed_step: u32,
    trace_hash: String,
    replay_trace_hash: String,
    exact_trace_replay: bool,
    window_motions_exact: bool,
    maximum_scalar_absolute_difference: f32,
    trace: GpuGeneratedContactPassTraceV3,
    energy: SettlingPassEnergyReportV1,
    static_support_attribution: SettlingStaticSupportAttributionReportV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoutingVerdict {
    classification: &'static str,
    hypothesis: &'static str,
    lead_operator_family: Option<GpuSolverPassKindV1>,
    rationale: String,
}

#[derive(Clone, Copy)]
enum SceneKind {
    CenteredStack,
    EnergyOnly,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s11-soft-pass-attribution error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S11.1 soft pass attribution requires a clean source tree".to_owned());
    }
    let (source, kind, scenario_role) = match arguments.scenario.as_str() {
        "soft-stack-16" => (
            p5_2_settling_soft_stack_matrix_scenario(16).map_err(|error| error.to_string())?,
            SceneKind::CenteredStack,
            "primary-derivation-gate",
        ),
        "permanent-four-body-soft" => (
            p5_2_minimized_mixed_pile_energy_scenario().map_err(|error| error.to_string())?,
            SceneKind::EnergyOnly,
            "required-secondary-context",
        ),
        _ => {
            return Err(format!("unsupported S11.1 scenario {}", arguments.scenario));
        }
    };
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;

    eprintln!("S11.1 {} exact observation and replay", arguments.scenario);
    let observation_series = run_observation_series(&runner, &source)?;
    let replay_observation_series = run_observation_series(&runner, &source)?;
    observation_series.validate()?;
    replay_observation_series.validate()?;
    let observation_series_hash = serialized_hash(&observation_series)?;
    let replay_observation_series_hash = serialized_hash(&replay_observation_series)?;
    let exact_observation_replay = observation_series_hash == replay_observation_series_hash;
    let capacity_clean = observation_series
        .snapshots
        .iter()
        .chain(&replay_observation_series.snapshots)
        .all(snapshot_capacity_clean);
    let (structural_pass, structural_reason, first_structural_failure_step) =
        structure_verdict(kind, &source, &observation_series)?;
    let observations = observe_generated_settling_series_v2(&source, &observation_series)?;
    let settling_summary =
        summarize_settling_v2(&observations, awake_settling_measurement_policy_v2())?;
    let rising_energy_mode = rising_energy_mode(&settling_summary);

    eprintln!(
        "S11.1 {} settled pass-motion window and replay",
        arguments.scenario
    );
    let motion_window = runner.run_contact_policy_pass_motion_window(
        &source,
        FIRST_WINDOW_STEP,
        LAST_WINDOW_STEP,
        ContactIdentityPolicyV1::PriorContactHysteresis,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )?;
    let replay_motion_window = runner.run_contact_policy_pass_motion_window(
        &source,
        FIRST_WINDOW_STEP,
        LAST_WINDOW_STEP,
        ContactIdentityPolicyV1::PriorContactHysteresis,
        GpuSoftFeedbackPolicyV1::SELECTED,
    )?;
    let motion_window_hash = serialized_hash(&motion_window)?;
    let replay_motion_window_hash = serialized_hash(&replay_motion_window)?;
    let exact_motion_window_replay = motion_window_hash == replay_motion_window_hash;
    let motion_window_report = analyze_settling_pass_energy_window(&source, &motion_window)?;

    let mut anchors = Vec::new();
    for completed_step in ANCHOR_STEPS {
        eprintln!(
            "S11.1 {} exact full trace and replay at step {completed_step}",
            arguments.scenario
        );
        let trace = runner.run_contact_policy_pass_trace(
            &source,
            completed_step,
            ContactIdentityPolicyV1::PriorContactHysteresis,
            GpuSoftFeedbackPolicyV1::SELECTED,
        )?;
        let replay_trace = runner.run_contact_policy_pass_trace(
            &source,
            completed_step,
            ContactIdentityPolicyV1::PriorContactHysteresis,
            GpuSoftFeedbackPolicyV1::SELECTED,
        )?;
        let trace_hash = serialized_hash(&trace)?;
        let replay_trace_hash = serialized_hash(&replay_trace)?;
        let energy = analyze_settling_pass_energy(&source, &trace)?;
        let static_support_attribution = analyze_static_support_energy_attribution(&trace)?;
        let window_step = motion_window
            .steps
            .iter()
            .find(|step| step.completed_step == completed_step)
            .ok_or_else(|| format!("motion window omitted anchor step {completed_step}"))?;
        let window_motions_exact = trace_motion_exact(&trace, &window_step.states);
        let maximum_scalar_absolute_difference = energy
            .rows
            .iter()
            .map(|row| row.state_difference.max_absolute_difference)
            .fold(0.0_f32, f32::max);
        anchors.push(AnchorEvidence {
            completed_step,
            trace_hash,
            replay_trace_hash,
            exact_trace_replay: serialized_hash(&trace)? == serialized_hash(&replay_trace)?,
            window_motions_exact,
            maximum_scalar_absolute_difference,
            trace,
            energy,
            static_support_attribution,
        });
    }

    let all_anchor_motions_exact = anchors.iter().all(|anchor| anchor.window_motions_exact);
    let all_anchor_traces_exact_replay = anchors.iter().all(|anchor| anchor.exact_trace_replay);
    let scalar_mirror_within_tolerance = anchors
        .iter()
        .all(|anchor| anchor.maximum_scalar_absolute_difference <= SCALAR_MAX_ABSOLUTE_DIFFERENCE);
    let support_accounting_within_tolerance = anchors.iter().all(|anchor| {
        anchor
            .static_support_attribution
            .maximum_accounting_error_compensated_f64
            <= SUPPORT_ACCOUNTING_TOLERANCE
    });
    let consistent_material_injectors = motion_window_report
        .families
        .iter()
        .filter(|family| {
            family.consistent_material_injector
                && family.kind != GpuSolverPassKindV1::IntegrateVelocity
        })
        .map(|family| family.kind)
        .collect::<Vec<_>>();
    let routing_verdict = routing_verdict(&consistent_material_injectors);

    if !exact_observation_replay
        || !exact_motion_window_replay
        || !capacity_clean
        || !structural_pass
        || !all_anchor_motions_exact
        || !all_anchor_traces_exact_replay
        || !scalar_mirror_within_tolerance
        || !support_accounting_within_tolerance
    {
        return Err(format!(
            "S11.1 {} failed replay, capacity, structure, anchor, scalar, or accounting gates",
            arguments.scenario
        ));
    }

    let adapter_marker = motion_window.adapter.marker_json();
    let artifact = Artifact {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "p5.2b-s11.1-soft-pass-attribution",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-headless-metal",
        scenario_name: arguments.scenario,
        scenario_role,
        source_scenario_hash,
        source_scenario: source,
        observation_series,
        observation_series_hash,
        replay_observation_series_hash,
        exact_observation_replay,
        capacity_clean,
        structural_pass,
        structural_reason,
        first_structural_failure_step,
        settling_summary,
        rising_energy_mode,
        motion_window,
        motion_window_hash,
        replay_motion_window_hash,
        exact_motion_window_replay,
        motion_window_report,
        anchors,
        all_anchor_motions_exact,
        all_anchor_traces_exact_replay,
        scalar_mirror_within_tolerance,
        support_accounting_within_tolerance,
        consistent_material_injectors,
        routing_verdict,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    let hash = write_gzip_json_atomic(&arguments.output, &artifact)?;
    println!("PHYSBOX5_S11_PASS_ATTRIBUTION_ADAPTER {adapter_marker}");
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario={} run_id={} sha256={} output={}",
        artifact.scenario_name,
        artifact.run_id,
        hash,
        arguments.output.display()
    );
    Ok(())
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

fn structure_verdict(
    kind: SceneKind,
    source: &ScenarioV1,
    series: &GpuGeneratedContactSeriesV1,
) -> Result<(bool, String, Option<u32>), String> {
    let mut first_failure = None;
    let mut final_reason = String::new();
    for snapshot in &series.snapshots {
        let capacity_failed = !snapshot_capacity_clean(snapshot);
        let (passed, reason) = match kind {
            SceneKind::CenteredStack => {
                centered_stack_structure(source, &snapshot.bodies, capacity_failed, 0.0)
            }
            SceneKind::EnergyOnly => (
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
        return Err("S11.1 observation series has no structural snapshots".to_owned());
    }
    Ok((first_failure.is_none(), final_reason, first_failure))
}

fn trace_motion_exact(trace: &GpuGeneratedContactPassTraceV3, states: &[Vec<[f32; 4]>]) -> bool {
    trace.states.len() == states.len()
        && trace
            .states
            .iter()
            .zip(states)
            .all(|(trace_state, motions)| {
                trace_state.bodies.len() == motions.len()
                    && trace_state
                        .bodies
                        .iter()
                        .zip(motions)
                        .all(|(body, motion)| {
                            body.linear_velocity.x.to_bits() == motion[0].to_bits()
                                && body.linear_velocity.y.to_bits() == motion[1].to_bits()
                                && body.angular_velocity.get().to_bits() == motion[2].to_bits()
                        })
            })
}

fn routing_verdict(injectors: &[GpuSolverPassKindV1]) -> RoutingVerdict {
    let hypothesis_one = injectors.iter().copied().find(|kind| {
        matches!(
            kind,
            GpuSolverPassKindV1::WarmStart
                | GpuSolverPassKindV1::NormalBiased
                | GpuSolverPassKindV1::TangentBiased
                | GpuSolverPassKindV1::NormalRelaxation
                | GpuSolverPassKindV1::TangentRelaxation
        )
    });
    if let Some(kind) = hypothesis_one {
        RoutingVerdict {
            classification: "consistent-material-solver-injector",
            hypothesis: "h1-soft-warm-start-bias-composition",
            lead_operator_family: Some(kind),
            rationale: format!(
                "{kind:?} is a consistent non-gravity material injector over the complete window and all three anchors"
            ),
        }
    } else if let Some(kind) = injectors.first().copied() {
        RoutingVerdict {
            classification: "consistent-material-support-or-persistence-injector",
            hypothesis: "h2-support-transfer-or-persistence-composition",
            lead_operator_family: Some(kind),
            rationale: format!(
                "{kind:?} is the leading consistent non-gravity material injector outside the warm-start/bias families"
            ),
        }
    } else {
        RoutingVerdict {
            classification: "no-consistent-material-single-family",
            hypothesis: "h3-secular-soft-recurrence-drift",
            lead_operator_family: None,
            rationale: "no non-gravity pass family is net materially positive over the complete window and at all three anchors; retain sign-changing family totals for whole-recurrence operator accounting".to_owned(),
        }
    }
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
    scenario: String,
    output: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
        let mut scenario = None;
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
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            scenario: scenario.ok_or_else(|| "missing --scenario".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            require_clean,
        })
    }
}

fn write_gzip_json_atomic(path: &Path, value: &impl Serialize) -> Result<String, String> {
    if path.exists() || path.with_extension("partial").exists() {
        return Err("S11.1 output already exists".to_owned());
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
    fn arguments_require_run_scene_and_output_identity() {
        assert!(Arguments::parse(std::iter::empty()).is_err());
        let parsed = Arguments::parse(
            [
                "--run-id",
                "run",
                "--scenario",
                "soft-stack-16",
                "--output",
                "/tmp/s11.json.gz",
                "--require-clean",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(parsed.scenario, "soft-stack-16");
        assert!(parsed.require_clean);
    }
}
