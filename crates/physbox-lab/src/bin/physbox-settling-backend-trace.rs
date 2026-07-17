use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    ContactIdentityPolicyV1, SCHEMA_VERSION, ScenarioV1, SettlingSeriesV2, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, P5_2_SETTLING_EVIDENCE_STEPS,
    P5_2SettlingReviewPolicy, p5_2_minimized_mixed_pile_energy_scenario,
    p5_2_settling_review_scenario,
};
use physbox_lab::settling::{
    awake_settling_measurement_policy_v2, centered_stack_structure,
    observe_generated_settling_series_v2, settling_series_v2,
};
use serde::Serialize;

const TRACE_VERSION: u32 = 1;
const SUBMISSION_BATCH_STEPS: u32 = 1;
const SUCCESS_SENTINEL: &str = "PHYSBOX5_SETTLING_BACKEND_TRACE_SUCCESS";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceScenario {
    Hard16,
    Identity16,
    Softness16,
    Candidate8,
    Candidate9,
    Candidate10,
    Candidate11,
    Candidate12,
    Candidate13,
    Candidate14,
    Candidate15,
    Candidate16,
    MinimizedMixedCandidate,
}

impl TraceScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "settling-hard-16" => Ok(Self::Hard16),
            "settling-identity-16" => Ok(Self::Identity16),
            "settling-softness-16" => Ok(Self::Softness16),
            "settling-candidate-8" => Ok(Self::Candidate8),
            "settling-candidate-9" => Ok(Self::Candidate9),
            "settling-candidate-10" => Ok(Self::Candidate10),
            "settling-candidate-11" => Ok(Self::Candidate11),
            "settling-candidate-12" => Ok(Self::Candidate12),
            "settling-candidate-13" => Ok(Self::Candidate13),
            "settling-candidate-14" => Ok(Self::Candidate14),
            "settling-candidate-15" => Ok(Self::Candidate15),
            "settling-candidate-16" => Ok(Self::Candidate16),
            "minimized-mixed-candidate" => Ok(Self::MinimizedMixedCandidate),
            _ => Err(format!("unknown settling trace scenario {value}")),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Hard16 => "settling-hard-16",
            Self::Identity16 => "settling-identity-16",
            Self::Softness16 => "settling-softness-16",
            Self::Candidate8 => "settling-candidate-8",
            Self::Candidate9 => "settling-candidate-9",
            Self::Candidate10 => "settling-candidate-10",
            Self::Candidate11 => "settling-candidate-11",
            Self::Candidate12 => "settling-candidate-12",
            Self::Candidate13 => "settling-candidate-13",
            Self::Candidate14 => "settling-candidate-14",
            Self::Candidate15 => "settling-candidate-15",
            Self::Candidate16 => "settling-candidate-16",
            Self::MinimizedMixedCandidate => "minimized-mixed-candidate",
        }
    }

    fn source(self) -> Result<(ScenarioV1, ContactIdentityPolicyV1, Structure), String> {
        let review = |policy| {
            p5_2_settling_review_scenario(16, policy)
                .map(|source| (source, policy.identity_policy(), Structure::CenteredStack))
                .map_err(|error| error.to_string())
        };
        match self {
            Self::Hard16 => review(P5_2SettlingReviewPolicy::HardControl),
            Self::Identity16 => review(P5_2SettlingReviewPolicy::IdentityOnly),
            Self::Softness16 => review(P5_2SettlingReviewPolicy::SoftnessOnly),
            Self::Candidate8 => {
                p5_2_settling_review_scenario(8, P5_2SettlingReviewPolicy::SoftCandidate)
                    .map(|source| {
                        (
                            source,
                            ContactIdentityPolicyV1::PriorContactHysteresis,
                            Structure::CenteredStack,
                        )
                    })
                    .map_err(|error| error.to_string())
            }
            Self::Candidate9 => candidate_stack(9),
            Self::Candidate10 => candidate_stack(10),
            Self::Candidate11 => candidate_stack(11),
            Self::Candidate12 => candidate_stack(12),
            Self::Candidate13 => candidate_stack(13),
            Self::Candidate14 => candidate_stack(14),
            Self::Candidate15 => candidate_stack(15),
            Self::Candidate16 => review(P5_2SettlingReviewPolicy::SoftCandidate),
            Self::MinimizedMixedCandidate => p5_2_minimized_mixed_pile_energy_scenario()
                .map(|source| {
                    (
                        source,
                        ContactIdentityPolicyV1::PriorContactHysteresis,
                        Structure::EnergyOnly,
                    )
                })
                .map_err(|error| error.to_string()),
        }
    }
}

fn candidate_stack(
    rectangle_count: u32,
) -> Result<(ScenarioV1, ContactIdentityPolicyV1, Structure), String> {
    p5_2_settling_review_scenario(rectangle_count, P5_2SettlingReviewPolicy::SoftCandidate)
        .map(|source| {
            (
                source,
                ContactIdentityPolicyV1::PriorContactHysteresis,
                Structure::CenteredStack,
            )
        })
        .map_err(|error| error.to_string())
}

#[derive(Clone, Copy)]
enum Structure {
    CenteredStack,
    EnergyOnly,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuralObservation {
    completed_steps: u32,
    passed: bool,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    trace_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    scenario_name: &'static str,
    mapped_after_final_step_only: bool,
    submission_batch_steps: u32,
    source_scenario: ScenarioV1,
    raw: GpuGeneratedContactSeriesV1,
    settling: SettlingSeriesV2,
    structural_observations: Vec<StructuralObservation>,
    first_structural_failure_step: Option<u32>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-settling-backend-trace error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let (source, identity_policy, structure) = arguments.scenario.source()?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let raw = runner.run_contact_policy_observation_series_with_submission_batch(
        &source,
        1,
        P5_2_SETTLING_EVIDENCE_STEPS,
        identity_policy,
        SUBMISSION_BATCH_STEPS,
    )?;
    let structural_observations = raw
        .snapshots
        .iter()
        .map(|snapshot| {
            let capacity_failed = snapshot.failure.is_some()
                || snapshot
                    .capacities
                    .iter()
                    .any(|capacity| !capacity.accepted || capacity.overflowed);
            let (passed, reason) = match structure {
                Structure::CenteredStack => {
                    centered_stack_structure(&source, &snapshot.bodies, capacity_failed, 0.0)
                }
                Structure::EnergyOnly => (
                    !capacity_failed,
                    if capacity_failed {
                        "energy reproducer exceeded capacity"
                    } else {
                        "energy-only reproducer has no structural acceptance gate"
                    }
                    .to_owned(),
                ),
            };
            StructuralObservation {
                completed_steps: snapshot.completed_steps,
                passed,
                reason,
            }
        })
        .collect::<Vec<_>>();
    let first_structural_failure_step = structural_observations
        .iter()
        .find(|observation| !observation.passed)
        .map(|observation| observation.completed_steps);
    let final_structure = structural_observations
        .last()
        .ok_or_else(|| "settling trace omitted final structure".to_owned())?;
    let observations = observe_generated_settling_series_v2(&source, &raw)?;
    let settling = settling_series_v2(
        &source,
        None,
        format!("{}-backend-trace", arguments.scenario.name()),
        awake_settling_measurement_policy_v2(),
        observations,
        final_structure.passed,
        final_structure.reason.clone(),
    )?;
    let adapter_marker = raw.adapter.marker_json();
    let report = Report {
        schema_version: SCHEMA_VERSION,
        trace_version: TRACE_VERSION,
        artifact_kind: "p5.2b-s6.5a-settling-backend-trace",
        run_id: arguments.run_id,
        source_revision: arguments.source_revision,
        source_dirty: arguments.source_dirty,
        host: if cfg!(target_os = "android") {
            "android-native-vulkan"
        } else {
            "native-headless"
        },
        scenario_name: arguments.scenario.name(),
        mapped_after_final_step_only: true,
        submission_batch_steps: SUBMISSION_BATCH_STEPS,
        source_scenario: source,
        raw,
        settling,
        structural_observations,
        first_structural_failure_step,
    };
    let hash = write_gzip_json_atomic(&arguments.output, &report)?;
    println!("PHYSBOX5_SETTLING_TRACE_ADAPTER {adapter_marker}");
    println!(
        "{SUCCESS_SENTINEL} scenario={} sha256={} output={}",
        arguments.scenario.name(),
        hash,
        arguments.output.display()
    );
    Ok(())
}

struct Arguments {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    scenario: TraceScenario,
    output: PathBuf,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
        let mut source_revision = None;
        let mut source_dirty = None;
        let mut scenario = None;
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
                "--scenario" => scenario = Some(TraceScenario::parse(&value)?),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            source_revision: source_revision
                .ok_or_else(|| "missing --source-revision".to_owned())?,
            source_dirty: source_dirty.ok_or_else(|| "missing --source-dirty".to_owned())?,
            scenario: scenario.ok_or_else(|| "missing --scenario".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
        })
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean {value}")),
    }
}

fn write_gzip_json_atomic(path: &Path, value: &impl Serialize) -> Result<String, String> {
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
        .create(true)
        .truncate(true)
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
    use physbox_contracts::canonical_hash;

    use super::*;

    #[test]
    fn minimized_source_matches_permanent_reproducer() {
        let (source, identity, structure) =
            TraceScenario::MinimizedMixedCandidate.source().unwrap();
        assert_eq!(
            canonical_hash(&source).unwrap(),
            "1df188838272f87e212cc00172207d84b6007423853573dac08751dc4973ca5c"
        );
        assert_eq!(identity, ContactIdentityPolicyV1::PriorContactHysteresis);
        assert!(matches!(structure, Structure::EnergyOnly));
    }

    #[test]
    fn arguments_require_explicit_provenance_and_output() {
        assert!(Arguments::parse(Vec::<String>::new().into_iter()).is_err());
        let arguments = Arguments::parse(
            [
                "--run-id",
                "run",
                "--source-revision",
                "abc",
                "--source-dirty",
                "false",
                "--scenario",
                "settling-candidate-16",
                "--output",
                "/tmp/trace.json.gz",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(arguments.scenario, TraceScenario::Candidate16);
    }

    #[test]
    fn candidate_depth_aliases_share_the_frozen_policy() {
        for count in 8..=16 {
            let alias = TraceScenario::parse(&format!("settling-candidate-{count}")).unwrap();
            let (source, identity, structure) = alias.source().unwrap();
            assert_eq!(source.bodies.len(), count as usize + 1);
            assert_eq!(identity, ContactIdentityPolicyV1::PriorContactHysteresis);
            assert!(source.settings.contact_softness.is_some());
            assert!(matches!(structure, Structure::CenteredStack));
        }
    }
}
