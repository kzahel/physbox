use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyId, BodyStateV1, ContactCachePointV1, OracleRequestV1, OracleResultV1, SCHEMA_VERSION,
    ScenarioV1, ShapeV1, canonical_hash, sha256_bytes,
};
use physbox_engine::{P5_2SettlingFixtureKind, p5_2_box2d_world_projection};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructureConfigurationV1, AuthorityStructurePolicyV1,
        mandatory_authority_structure_configurations_v1, solve_authority_structure_steps_observed,
    },
    oracle_transport::{
        OracleProcess, OracleRunConfig, OracleSourceIdentity, run_oracle, verify_clean_git_pin,
    },
    scalar::{
        ScalarObservedSolveResultV1, ScalarStepObservationV1, ScalarSubstepImpulsePolicyV1,
        solve_portable_narrowphase_steps_observed_with_substep_impulses,
    },
    settling::{centered_stack_structure, settling_fixture_structure},
    stationarity::{AuthorityTraceSnapshotV1, decode_box2d_step_trace},
};
use serde::{Deserialize, Serialize};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const SOURCE_ROOT: &str = "evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/sources";
const SOURCE_MANIFEST: &str = "evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/manifest.json";
const SOURCE_MANIFEST_SHA256: &str =
    "346f91558c1132381eff3c1eca14d54d26873d680ca726bb98eb96757e7b9496";
const AUTHORITY_SPEED_MULTIPLIER: f64 = 2.0;
const AUTHORITY_SPEED_ADDITIVE_BAND: f64 = 2.0e-5;
const AUTHORITY_HEIGHT_ABSOLUTE_BAND: f64 = 2.0e-4;

const SOURCES: [SourceDescriptor; 4] = [
    SourceDescriptor {
        label: "two-step-soft-contract",
        filename: "two-step-soft-contract.json",
        structure: StructureKind::MixedPile,
        role: SourceRole::ResponseDiagnostic,
    },
    SourceDescriptor {
        label: "permanent-four-body-soft",
        filename: "permanent-four-body-soft.json",
        structure: StructureKind::MixedPile,
        role: SourceRole::Context,
    },
    SourceDescriptor {
        label: "s11.2-centered-eleven-soft",
        filename: "s11.2-centered-eleven-soft.json",
        structure: StructureKind::CenteredStack,
        role: SourceRole::Governed,
    },
    SourceDescriptor {
        label: "governed-centered-sixteen-soft",
        filename: "governed-centered-sixteen-soft.json",
        structure: StructureKind::CenteredStack,
        role: SourceRole::Governed,
    },
];

#[derive(Clone, Copy)]
struct SourceDescriptor {
    label: &'static str,
    filename: &'static str,
    structure: StructureKind,
    role: SourceRole,
}

#[derive(Clone, Copy)]
enum StructureKind {
    MixedPile,
    CenteredStack,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SourceRole {
    ResponseDiagnostic,
    Context,
    Governed,
}

#[derive(Clone, Copy)]
enum EvidenceMode {
    Endpoints,
    SingleAxis,
    Interactions,
}

impl EvidenceMode {
    const fn artifact_kind(self) -> &'static str {
        match self {
            Self::Endpoints => "p5.2b-s12.2-authority-endpoints",
            Self::SingleAxis => "p5.2b-s12.3-single-axis-decomposition",
            Self::Interactions => "p5.2b-s12.4-structure-minimization",
        }
    }

    const fn success_scenario(self) -> &'static str {
        match self {
            Self::Endpoints => "s12-authority-endpoints",
            Self::SingleAxis => "s12-single-axis-decomposition",
            Self::Interactions => "s12-structure-minimization",
        }
    }
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
    source_contract: ArtifactIdentity,
    box2d: SourceIdentity,
    authority_bands: AuthorityBands,
    configurations: Vec<AuthorityStructureConfigurationV1>,
    cases: Vec<CaseEvidence>,
    routing_input: Option<ArtifactIdentity>,
    single_axis_decomposition: Option<SingleAxisDecomposition>,
    interaction_minimization: Option<InteractionMinimization>,
    p_endpoint_exact: bool,
    a_endpoint_qualified: bool,
    decomposition_admitted: bool,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityBands {
    speed_multiplier: f64,
    speed_additive: f64,
    height_absolute: f64,
    penetration_absolute: f64,
    awake_product_ceiling: f64,
    positive_energy_run_floor: u32,
    energy_growth_rule: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SingleAxisDecomposition {
    ordered_failure_vector: [&'static str; 7],
    axes: Vec<AxisDisposition>,
    interaction_routes: Vec<InteractionRoute>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AxisDisposition {
    axis: AuthorityStructureAxisV1,
    axis_number: u8,
    p_plus_id: String,
    a_minus_id: String,
    necessary: bool,
    individually_sufficient: bool,
    individually_improving: bool,
    p_plus_governed_sources: Vec<GovernedDisposition>,
    a_minus_governed_sources: Vec<GovernedDisposition>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GovernedDisposition {
    label: &'static str,
    authority_qualified: bool,
    ordered_failures: Vec<&'static str>,
    settled_linear_speed_max: f64,
    settled_angular_edge_speed_max: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionRoute {
    id: String,
    axes: Option<[AuthorityStructureAxisV1; 2]>,
    policy: Option<AuthorityStructurePolicyV1>,
    status: &'static str,
    reason: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoutingManifest {
    artifact_kind: String,
    single_axis_decomposition: RoutingDecomposition,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoutingDecomposition {
    axes: Vec<RoutingAxisDisposition>,
    interaction_routes: Vec<RoutingInteraction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoutingAxisDisposition {
    axis: AuthorityStructureAxisV1,
    necessary: bool,
    individually_sufficient: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoutingInteraction {
    id: String,
    axes: Option<[AuthorityStructureAxisV1; 2]>,
    policy: Option<AuthorityStructurePolicyV1>,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionMinimization {
    routes: Vec<InteractionOutcome>,
    smallest_qualified_measured_cardinality: u8,
    minimal_sufficient_measured_axes: Vec<AuthorityStructureAxisV1>,
    minimal_sufficient_measured_configuration: String,
    bounded_minimum_cardinality: [u8; 2],
    unmeasured_three_axis_expansion: bool,
    statement: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionOutcome {
    id: String,
    axes: [AuthorityStructureAxisV1; 2],
    policy: AuthorityStructurePolicyV1,
    all_sources_qualified: bool,
    governed_sources_qualified: bool,
    governed_sources: Vec<GovernedDisposition>,
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
struct CaseEvidence {
    label: &'static str,
    role: SourceRole,
    requested_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    authority: AuthorityEvidence,
    p_exact: PExactEvidence,
    configurations: Vec<ConfigurationEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityEvidence {
    primary_artifact: String,
    replay_artifact: String,
    primary_result_sha256: String,
    replay_result_sha256: String,
    exact_replay: bool,
    capture_exact_replay: bool,
    clean_public_trace_provenance: bool,
    public_trace_records: usize,
    quality: QualitySummary,
    active_support_pairs: Vec<BodyPair>,
    capture: ArtifactIdentity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PExactEvidence {
    exact: bool,
    compared_outer_steps: u32,
    exact_step_trace_digest: String,
    full_observations_exact: bool,
    final_bodies_exact: bool,
    final_cache_exact: bool,
    known_s11_failure_reproduced: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationEvidence {
    id: String,
    policy: AuthorityStructurePolicyV1,
    exact_replay: bool,
    quality: QualitySummary,
    active_support_pairs: Vec<BodyPair>,
    delta_from_authority: AuthorityDelta,
    failure_vector: FailureVector,
    authority_qualified: bool,
    capture: ArtifactIdentity,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QualitySummary {
    settled_start_step: u32,
    settled_end_step: u32,
    settled_linear_speed_max: f64,
    settled_angular_edge_speed_max: f64,
    first_energy_quartile_max: f64,
    last_energy_quartile_max: f64,
    energy_quartile_growth_ratio: f64,
    positive_energy_run_max: u32,
    energy_envelope_nonincreasing: bool,
    maximum_penetration: f64,
    maximum_separation: f64,
    final_active_normal_points: u32,
    final_total_normal_impulse: f64,
    final_dynamic_center_height_sum: f64,
    structure_pass: bool,
    structure_reason: String,
    finite: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityDelta {
    linear_speed: f64,
    angular_edge_speed: f64,
    final_dynamic_center_height_sum: f64,
    maximum_penetration: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureVector {
    non_finite: bool,
    structure: bool,
    authority_response: bool,
    penetration_support: bool,
    awake_speed: bool,
    positive_run: bool,
    energy_envelope: bool,
    ordered_failures: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyPair {
    body_a: BodyId,
    body_b: BodyId,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScalarCapture<'a> {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: &'a str,
    source_scenario_hash: &'a str,
    configuration_id: &'a str,
    policy: AuthorityStructurePolicyV1,
    observations: &'a [ScalarStepObservationV1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityCapture<'a> {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: &'a str,
    source_scenario_hash: &'a str,
    observations: &'a [AuthorityTraceSnapshotV1],
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s12-authority-structure error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S12 authority evidence requires a clean source tree".to_owned());
    }
    let source_manifest_bytes =
        fs::read(repository.join(SOURCE_MANIFEST)).map_err(display_error)?;
    if sha256_bytes(&source_manifest_bytes) != SOURCE_MANIFEST_SHA256 {
        return Err("S12 frozen S11.3 source manifest digest changed".to_owned());
    }
    let box2d =
        verify_clean_git_pin(&arguments.box2d_source, BOX2D_REVISION).map_err(display_error)?;
    let staging = prepare_staging(&arguments.output, &["sources", "captures", "oracles"])?;
    let (routing_manifest, routing_input) = if matches!(arguments.mode, EvidenceMode::Interactions)
    {
        let path = arguments
            .routing_manifest
            .as_ref()
            .ok_or_else(|| "interaction mode requires --routing-manifest".to_owned())?;
        let path = if path.is_absolute() {
            path.clone()
        } else {
            repository.join(path)
        };
        let bytes = fs::read(&path).map_err(display_error)?;
        let routing: RoutingManifest = serde_json::from_slice(&bytes).map_err(display_error)?;
        let relative = path
            .strip_prefix(&repository)
            .map_err(|_| "routing manifest must be inside the repository".to_owned())?;
        (
            Some(routing),
            Some(ArtifactIdentity {
                path: relative.to_string_lossy().replace('\\', "/"),
                sha256: sha256_bytes(&bytes),
            }),
        )
    } else {
        if arguments.routing_manifest.is_some() {
            return Err("--routing-manifest is valid only in interaction mode".to_owned());
        }
        (None, None)
    };
    let configurations = match arguments.mode {
        EvidenceMode::Endpoints => mandatory_authority_structure_configurations_v1()
            .into_iter()
            .take(2)
            .collect(),
        EvidenceMode::SingleAxis => mandatory_authority_structure_configurations_v1(),
        EvidenceMode::Interactions => {
            let routing = routing_manifest
                .as_ref()
                .ok_or_else(|| "S12 interaction routing is missing".to_owned())?;
            interaction_configurations(routing)?
        }
    };
    let mut cases = Vec::new();
    for descriptor in SOURCES {
        eprintln!("S12 endpoints {}", descriptor.label);
        let source_path = repository.join(SOURCE_ROOT).join(descriptor.filename);
        let source_bytes = fs::read(&source_path).map_err(display_error)?;
        let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display_error)?;
        cases.push(evaluate_case(
            &arguments,
            &staging,
            &box2d,
            descriptor,
            source,
            &configurations,
        )?);
    }
    let p_endpoint_exact = cases.iter().all(|case| case.p_exact.exact);
    let a_endpoint_qualified = cases.iter().all(|case| {
        case.configurations
            .iter()
            .find(|configuration| configuration.id == "A")
            .is_some_and(|configuration| configuration.authority_qualified)
    });
    let single_axis_decomposition = match arguments.mode {
        EvidenceMode::Endpoints => None,
        EvidenceMode::SingleAxis => Some(single_axis_decomposition(&cases)?),
        EvidenceMode::Interactions => None,
    };
    let interaction_minimization = match arguments.mode {
        EvidenceMode::Interactions => Some(interaction_minimization(
            &cases,
            routing_manifest
                .as_ref()
                .ok_or_else(|| "S12 interaction routing is missing".to_owned())?,
        )?),
        EvidenceMode::Endpoints | EvidenceMode::SingleAxis => None,
    };
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: arguments.mode.artifact_kind(),
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        source_contract: ArtifactIdentity {
            path: SOURCE_MANIFEST.to_owned(),
            sha256: SOURCE_MANIFEST_SHA256.to_owned(),
        },
        box2d: source_identity(&box2d),
        authority_bands: AuthorityBands {
            speed_multiplier: AUTHORITY_SPEED_MULTIPLIER,
            speed_additive: AUTHORITY_SPEED_ADDITIVE_BAND,
            height_absolute: AUTHORITY_HEIGHT_ABSOLUTE_BAND,
            penetration_absolute: AUTHORITY_HEIGHT_ABSOLUTE_BAND,
            awake_product_ceiling: 0.025,
            positive_energy_run_floor: 4,
            energy_growth_rule: "candidate last/first settled energy quartile ratio no greater than max(1, authority ratio), with 1e-5 relative comparison slack",
        },
        configurations,
        cases,
        routing_input,
        single_axis_decomposition,
        interaction_minimization,
        p_endpoint_exact,
        a_endpoint_qualified,
        decomposition_admitted: p_endpoint_exact && a_endpoint_qualified,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    publish_manifest(&staging, &arguments.output, &manifest)?;
    let manifest_bytes = fs::read(arguments.output.join("manifest.json")).map_err(display_error)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario={} run_id={} manifest_sha256={} output={}",
        arguments.mode.success_scenario(),
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    if !manifest.decomposition_admitted {
        return Err("S12 endpoint qualification failed; decomposition is not admitted".to_owned());
    }
    Ok(())
}

fn evaluate_case(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    descriptor: SourceDescriptor,
    source: ScenarioV1,
    configurations: &[AuthorityStructureConfigurationV1],
) -> Result<CaseEvidence, String> {
    let source_scenario_hash = canonical_hash(&source).map_err(display_error)?;
    let source_identity = write_json_artifact(
        staging,
        &format!("sources/{}.json", descriptor.label),
        &source,
    )?;
    let projection = p5_2_box2d_world_projection(&source).map_err(display_error)?;
    let projection_hash = canonical_hash(&projection).map_err(display_error)?;
    let authority = run_authority(
        arguments,
        staging,
        box2d,
        descriptor,
        &source_scenario_hash,
        &projection,
    )?;
    let p_exact = prove_p_exact(&source, descriptor.role)?;
    let mut configuration_evidence = Vec::new();
    for configuration in configurations {
        let primary =
            solve_authority_structure_steps_observed(&source, configuration.policy, false)
                .map_err(display_error)?;
        let replay = solve_authority_structure_steps_observed(&source, configuration.policy, false)
            .map_err(display_error)?;
        let exact_replay = primary == replay;
        let quality = scalar_quality(&source, &primary, descriptor.structure)?;
        let active_support_pairs = scalar_active_support_pairs(&primary);
        let delta_from_authority = authority_delta(&quality, &authority.quality);
        let failure_vector = failure_vector(
            &source,
            descriptor.role,
            &quality,
            &active_support_pairs,
            &authority.quality,
            &authority.active_support_pairs,
        );
        let authority_qualified = exact_replay && failure_vector.ordered_failures.is_empty();
        let capture = ScalarCapture {
            schema_version: SCHEMA_VERSION,
            artifact_kind: "p5.2b-s12.2-authority-structure-scalar-capture",
            run_id: &arguments.run_id,
            source_scenario_hash: &source_scenario_hash,
            configuration_id: &configuration.id,
            policy: configuration.policy,
            observations: &primary.observations,
        };
        let capture = write_gzip_json_artifact(
            staging,
            &format!(
                "captures/{}-{}.json.gz",
                descriptor.label,
                configuration.id.to_ascii_lowercase()
            ),
            &capture,
        )?;
        configuration_evidence.push(ConfigurationEvidence {
            id: configuration.id.clone(),
            policy: configuration.policy,
            exact_replay,
            quality,
            active_support_pairs,
            delta_from_authority,
            failure_vector,
            authority_qualified,
            capture,
        });
    }
    Ok(CaseEvidence {
        label: descriptor.label,
        role: descriptor.role,
        requested_steps: source.requested_steps,
        source_scenario_hash,
        projection_hash,
        source: source_identity,
        authority,
        p_exact,
        configurations: configuration_evidence,
    })
}

fn run_authority(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    descriptor: SourceDescriptor,
    source_scenario_hash: &str,
    projection: &ScenarioV1,
) -> Result<AuthorityEvidence, String> {
    let primary = run_authority_once(
        arguments,
        staging,
        box2d,
        descriptor.label,
        projection,
        "primary",
    )?;
    let replay = run_authority_once(
        arguments,
        staging,
        box2d,
        descriptor.label,
        projection,
        "replay",
    )?;
    let primary_result_bytes =
        fs::read(primary.artifacts.directory.join("oracle-result.json")).map_err(display_error)?;
    let replay_result_bytes =
        fs::read(replay.artifacts.directory.join("oracle-result.json")).map_err(display_error)?;
    let exact_replay =
        result_payload_hash(&primary.result)? == result_payload_hash(&replay.result)?;
    let primary_snapshots = decode_box2d_step_trace(projection, &primary.result)?;
    let replay_snapshots = decode_box2d_step_trace(projection, &replay.result)?;
    let capture_exact_replay = primary_snapshots == replay_snapshots;
    let clean_public_trace_provenance =
        authority_trace_provenance(&primary.result) && authority_trace_provenance(&replay.result);
    let quality = authority_quality(projection, &primary_snapshots, descriptor.structure)?;
    let active_support_pairs = authority_active_support_pairs(&primary_snapshots);
    let capture = AuthorityCapture {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s12.2-box2d-authority-capture",
        run_id: &arguments.run_id,
        source_scenario_hash,
        observations: &primary_snapshots,
    };
    let capture = write_gzip_json_artifact(
        staging,
        &format!("captures/{}-authority.json.gz", descriptor.label),
        &capture,
    )?;
    Ok(AuthorityEvidence {
        primary_artifact: relative_directory(staging, &primary.artifacts.directory)?,
        replay_artifact: relative_directory(staging, &replay.artifacts.directory)?,
        primary_result_sha256: sha256_bytes(&primary_result_bytes),
        replay_result_sha256: sha256_bytes(&replay_result_bytes),
        exact_replay,
        capture_exact_replay,
        clean_public_trace_provenance,
        public_trace_records: primary.result.traces.len(),
        quality,
        active_support_pairs,
        capture,
    })
}

fn run_authority_once(
    arguments: &Arguments,
    staging: &Path,
    box2d: &OracleSourceIdentity,
    label: &str,
    projection: &ScenarioV1,
    phase: &str,
) -> Result<physbox_lab::oracle_transport::OracleRun, String> {
    let request_hash = canonical_hash(projection).map_err(display_error)?;
    let request = OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!("{}-{label}-{phase}", arguments.run_id),
        source_scenario_hash: request_hash,
        scenario: projection.clone(),
    };
    run_oracle(
        &OracleRunConfig {
            artifact_root: staging.join("oracles"),
            timeout: Duration::from_secs(120),
            source: box2d.clone(),
            process: OracleProcess {
                program: arguments.oracle.clone(),
                arguments: vec![
                    OsString::from("--lane"),
                    OsString::from("world-awake"),
                    OsString::from("--capture-step-trace"),
                    OsString::from("on"),
                ],
            },
        },
        &request,
    )
    .map_err(display_error)
}

fn prove_p_exact(source: &ScenarioV1, role: SourceRole) -> Result<PExactEvidence, String> {
    let expected_full = solve_portable_narrowphase_steps_observed_with_substep_impulses(
        source,
        ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
        false,
    )
    .map_err(display_error)?;
    let actual_full = solve_authority_structure_steps_observed(
        source,
        AuthorityStructurePolicyV1::physbox(),
        false,
    )
    .map_err(display_error)?;
    let full_observations_exact = expected_full.observations == actual_full.observations;
    let final_bodies_exact = expected_full.result.bodies == actual_full.result.bodies;
    let final_cache_exact = expected_full.result.contact_cache == actual_full.result.contact_cache;

    let mut current = source.clone();
    current.requested_steps = 1;
    let mut step_hashes = Vec::with_capacity(source.requested_steps as usize);
    for _ in 0..source.requested_steps {
        let expected = solve_portable_narrowphase_steps_observed_with_substep_impulses(
            &current,
            ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
            true,
        )
        .map_err(display_error)?;
        let actual = solve_authority_structure_steps_observed(
            &current,
            AuthorityStructurePolicyV1::physbox(),
            true,
        )
        .map_err(display_error)?;
        if expected != actual {
            return Err("S12 bridge P differs from scalar P in a one-step trace chunk".to_owned());
        }
        step_hashes.push(serialized_hash(&(
            &expected.result.bodies,
            &expected.result.contact_cache,
            &expected.result.traces,
            &expected.observations,
        ))?);
        current.bodies = expected.result.bodies;
        current.contact_cache = expected.result.contact_cache;
    }
    let exact_step_trace_digest = serialized_hash(&step_hashes)?;
    let quality = scalar_quality(
        source,
        &actual_full,
        if source.id.contains("centered") || source.id.contains("depth-sweep") {
            StructureKind::CenteredStack
        } else {
            StructureKind::MixedPile
        },
    )?;
    let known_s11_failure_reproduced = match role {
        SourceRole::ResponseDiagnostic => source.requested_steps < 1_200,
        SourceRole::Context => {
            quality.finite
                && quality.structure_pass
                && quality.settled_linear_speed_max <= 0.025
                && quality.settled_angular_edge_speed_max <= 0.025
        }
        SourceRole::Governed => {
            !quality.structure_pass
                || quality.settled_linear_speed_max > 0.025
                || quality.settled_angular_edge_speed_max > 0.025
                || quality.positive_energy_run_max >= 5
                || !quality.energy_envelope_nonincreasing
        }
    };
    Ok(PExactEvidence {
        exact: full_observations_exact && final_bodies_exact && final_cache_exact,
        compared_outer_steps: source.requested_steps,
        exact_step_trace_digest,
        full_observations_exact,
        final_bodies_exact,
        final_cache_exact,
        known_s11_failure_reproduced,
    })
}

fn scalar_quality(
    source: &ScenarioV1,
    solve: &ScalarObservedSolveResultV1,
    structure: StructureKind,
) -> Result<QualitySummary, String> {
    let observations = solve
        .observations
        .iter()
        .map(|observation| QualityObservation {
            completed_steps: observation.completed_steps,
            bodies: &observation.bodies,
            maximum_penetration: f64::from(observation.maximum_penetration),
            maximum_separation: f64::from(observation.maximum_separation),
            active_normal_points: observation.active_normal_points,
            total_normal_impulse: observation.total_normal_impulse,
        })
        .collect::<Vec<_>>();
    quality(source, &observations, structure)
}

fn authority_quality(
    source: &ScenarioV1,
    snapshots: &[AuthorityTraceSnapshotV1],
    structure: StructureKind,
) -> Result<QualitySummary, String> {
    let observations = snapshots
        .iter()
        .map(|snapshot| {
            let mut maximum_penetration = 0.0_f64;
            let mut maximum_separation = 0.0_f64;
            for point in snapshot
                .manifolds
                .iter()
                .flat_map(|manifold| manifold.points.iter())
            {
                maximum_penetration =
                    maximum_penetration.max(f64::from((-point.separation.get()).max(0.0)));
                maximum_separation =
                    maximum_separation.max(f64::from(point.separation.get().max(0.0)));
            }
            QualityObservation {
                completed_steps: snapshot.completed_steps,
                bodies: snapshot.bodies.as_slice(),
                maximum_penetration,
                maximum_separation,
                active_normal_points: snapshot
                    .cache
                    .iter()
                    .filter(|contact| contact.value.normal_impulse.get() > 0.0)
                    .count() as u32,
                total_normal_impulse: snapshot
                    .cache
                    .iter()
                    .map(|contact| f64::from(contact.value.normal_impulse.get()))
                    .sum(),
            }
        })
        .collect::<Vec<_>>();
    quality(source, &observations, structure)
}

struct QualityObservation<'a> {
    completed_steps: u32,
    bodies: &'a [BodyStateV1],
    maximum_penetration: f64,
    maximum_separation: f64,
    active_normal_points: u32,
    total_normal_impulse: f64,
}

fn quality(
    source: &ScenarioV1,
    observations: &[QualityObservation<'_>],
    structure: StructureKind,
) -> Result<QualitySummary, String> {
    let settled_start_step = if source.requested_steps >= 1_200 {
        900
    } else {
        1
    };
    let settled = observations
        .iter()
        .filter(|observation| observation.completed_steps >= settled_start_step)
        .collect::<Vec<_>>();
    if settled.is_empty() {
        return Err("S12 quality has no settled observations".to_owned());
    }
    let energy = settled
        .iter()
        .map(|observation| kinetic_energy(observation.bodies))
        .collect::<Vec<_>>();
    let quartile = (settled.len() / 4).max(1);
    let first_energy_quartile_max = energy[..quartile].iter().copied().fold(0.0, f64::max);
    let last_energy_quartile_max = energy[energy.len() - quartile..]
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let mut positive_energy_run = 0;
    let mut positive_energy_run_max = 0;
    for pair in energy.windows(2) {
        let epsilon = 1.0e-12 + 1.0e-5 * pair[0].abs();
        if pair[1] > pair[0] + epsilon {
            positive_energy_run += 1;
            positive_energy_run_max = positive_energy_run_max.max(positive_energy_run);
        } else {
            positive_energy_run = 0;
        }
    }
    let energy_envelope_nonincreasing = last_energy_quartile_max
        <= first_energy_quartile_max + 1.0e-12 + 1.0e-5 * first_energy_quartile_max.abs()
        && positive_energy_run_max < 5;
    let energy_quartile_growth_ratio = if first_energy_quartile_max > 0.0 {
        last_energy_quartile_max / first_energy_quartile_max
    } else if last_energy_quartile_max == 0.0 {
        1.0
    } else {
        f64::INFINITY
    };
    let final_observation = observations
        .last()
        .ok_or_else(|| "S12 quality has no final observation".to_owned())?;
    let (structure_pass, structure_reason) = match structure {
        StructureKind::MixedPile => settling_fixture_structure(
            P5_2SettlingFixtureKind::SmallMixedPile,
            source,
            final_observation.bodies,
            false,
            0.0,
        ),
        StructureKind::CenteredStack => {
            centered_stack_structure(source, final_observation.bodies, false, 0.0)
        }
    };
    Ok(QualitySummary {
        settled_start_step,
        settled_end_step: source.requested_steps,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(observation.bodies))
            .map(|body| {
                f64::from(
                    (body.linear_velocity.x * body.linear_velocity.x
                        + body.linear_velocity.y * body.linear_velocity.y)
                        .sqrt(),
                )
            })
            .fold(0.0, f64::max),
        settled_angular_edge_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(observation.bodies))
            .map(|body| f64::from(body.angular_velocity.get().abs() * support_radius(body)))
            .fold(0.0, f64::max),
        first_energy_quartile_max,
        last_energy_quartile_max,
        energy_quartile_growth_ratio,
        positive_energy_run_max,
        energy_envelope_nonincreasing,
        maximum_penetration: settled
            .iter()
            .map(|observation| observation.maximum_penetration)
            .fold(0.0, f64::max),
        maximum_separation: settled
            .iter()
            .map(|observation| observation.maximum_separation)
            .fold(0.0, f64::max),
        final_active_normal_points: final_observation.active_normal_points,
        final_total_normal_impulse: final_observation.total_normal_impulse,
        final_dynamic_center_height_sum: dynamic_bodies(final_observation.bodies)
            .map(|body| f64::from(body.position.y))
            .sum(),
        structure_pass,
        structure_reason,
        finite: observations.iter().all(|observation| {
            observation.maximum_penetration.is_finite()
                && observation.maximum_separation.is_finite()
                && observation.total_normal_impulse.is_finite()
                && observation.bodies.iter().all(body_finite)
        }),
    })
}

fn failure_vector(
    source: &ScenarioV1,
    role: SourceRole,
    candidate: &QualitySummary,
    candidate_pairs: &[BodyPair],
    authority: &QualitySummary,
    authority_pairs: &[BodyPair],
) -> FailureVector {
    let full_length = source.requested_steps >= 1_200;
    let non_finite = !candidate.finite;
    let structure = !candidate.structure_pass;
    let authority_response = candidate.settled_linear_speed_max
        > AUTHORITY_SPEED_MULTIPLIER * authority.settled_linear_speed_max
            + AUTHORITY_SPEED_ADDITIVE_BAND
        || candidate.settled_angular_edge_speed_max
            > AUTHORITY_SPEED_MULTIPLIER * authority.settled_angular_edge_speed_max
                + AUTHORITY_SPEED_ADDITIVE_BAND;
    let height_band = AUTHORITY_HEIGHT_ABSOLUTE_BAND.max(authority.maximum_penetration);
    let penetration_support = candidate_pairs != authority_pairs
        || (candidate.final_dynamic_center_height_sum - authority.final_dynamic_center_height_sum)
            .abs()
            > height_band
        || candidate.maximum_penetration
            > authority.maximum_penetration + AUTHORITY_HEIGHT_ABSOLUTE_BAND;
    let awake_speed = full_length
        && (candidate.settled_linear_speed_max > 0.025
            || candidate.settled_angular_edge_speed_max > 0.025);
    let positive_run =
        full_length && candidate.positive_energy_run_max > authority.positive_energy_run_max.max(4);
    let authority_growth_band = authority.energy_quartile_growth_ratio.max(1.0);
    let energy_envelope = full_length
        && candidate.energy_quartile_growth_ratio
            > authority_growth_band + 1.0e-12 + 1.0e-5 * authority_growth_band;
    let (authority_response, penetration_support, awake_speed, positive_run, energy_envelope) =
        match role {
            SourceRole::ResponseDiagnostic => (authority_response, false, false, false, false),
            SourceRole::Context => (false, false, awake_speed, false, false),
            SourceRole::Governed => (
                authority_response,
                penetration_support,
                awake_speed,
                positive_run,
                energy_envelope,
            ),
        };
    let ordered_failures = [
        (non_finite, "non-finite"),
        (structure, "structure"),
        (authority_response, "authority-response"),
        (penetration_support, "penetration-support"),
        (awake_speed, "awake-speed"),
        (positive_run, "positive-run"),
        (energy_envelope, "energy-envelope"),
    ]
    .into_iter()
    .filter_map(|(failed, label)| failed.then_some(label))
    .collect();
    FailureVector {
        non_finite,
        structure,
        authority_response,
        penetration_support,
        awake_speed,
        positive_run,
        energy_envelope,
        ordered_failures,
    }
}

fn authority_delta(candidate: &QualitySummary, authority: &QualitySummary) -> AuthorityDelta {
    AuthorityDelta {
        linear_speed: candidate.settled_linear_speed_max - authority.settled_linear_speed_max,
        angular_edge_speed: candidate.settled_angular_edge_speed_max
            - authority.settled_angular_edge_speed_max,
        final_dynamic_center_height_sum: candidate.final_dynamic_center_height_sum
            - authority.final_dynamic_center_height_sum,
        maximum_penetration: candidate.maximum_penetration - authority.maximum_penetration,
    }
}

fn single_axis_decomposition(cases: &[CaseEvidence]) -> Result<SingleAxisDecomposition, String> {
    let governed = cases
        .iter()
        .filter(|case| matches!(case.role, SourceRole::Governed))
        .collect::<Vec<_>>();
    if governed.len() != 2 {
        return Err("S12 decomposition requires exactly two governed sources".to_owned());
    }
    let mut axes = Vec::with_capacity(5);
    for axis in AuthorityStructureAxisV1::ALL {
        let p_plus_id = format!("P+X{}", axis.number());
        let a_minus_id = format!("A-X{}", axis.number());
        let p_plus = governed
            .iter()
            .map(|case| governed_disposition(case, &p_plus_id))
            .collect::<Result<Vec<_>, _>>()?;
        let a_minus = governed
            .iter()
            .map(|case| governed_disposition(case, &a_minus_id))
            .collect::<Result<Vec<_>, _>>()?;
        let necessary = a_minus.iter().any(|value| !value.authority_qualified);
        let individually_sufficient = p_plus.iter().all(|value| value.authority_qualified);
        let improvement = governed
            .iter()
            .map(|case| configuration_improvement(case, &p_plus_id))
            .collect::<Result<Vec<_>, _>>()?;
        let individually_improving = improvement.iter().all(Result::is_ok)
            && improvement
                .iter()
                .any(|comparison| matches!(comparison, Ok(true)));
        axes.push(AxisDisposition {
            axis,
            axis_number: axis.number(),
            p_plus_id,
            a_minus_id,
            necessary,
            individually_sufficient,
            individually_improving,
            p_plus_governed_sources: p_plus,
            a_minus_governed_sources: a_minus,
        });
    }

    let necessary = axes
        .iter()
        .filter(|value| value.necessary)
        .map(|value| value.axis)
        .collect::<Vec<_>>();
    let improving = axes
        .iter()
        .filter(|value| value.individually_improving)
        .map(|value| value.axis)
        .collect::<Vec<_>>();
    let mut routed = Vec::<[AuthorityStructureAxisV1; 2]>::new();
    for (index, &first) in necessary.iter().enumerate() {
        for &second in &necessary[index + 1..] {
            routed.push([first, second]);
        }
    }
    for &first in &necessary {
        for &second in &improving {
            if first == second {
                continue;
            }
            let pair = ordered_axis_pair(first, second);
            if !routed.contains(&pair) {
                routed.push(pair);
            }
        }
    }
    routed.truncate(3);
    let interaction_routes = (0..3)
        .map(|index| {
            let id = format!("I{}", index + 1);
            if let Some([first, second]) = routed.get(index).copied() {
                InteractionRoute {
                    id,
                    axes: Some([first, second]),
                    policy: Some(AuthorityStructurePolicyV1::interaction(first, second)),
                    status: "routed",
                    reason: "necessary-axis pairs precede necessary-plus-improving pairs",
                }
            } else {
                InteractionRoute {
                    id,
                    axes: None,
                    policy: None,
                    status: "not-routed",
                    reason: "fewer than three eligible pairs",
                }
            }
        })
        .collect();
    Ok(SingleAxisDecomposition {
        ordered_failure_vector: [
            "non-finite",
            "structure",
            "authority-response",
            "penetration-support",
            "awake-speed",
            "positive-run",
            "energy-envelope",
        ],
        axes,
        interaction_routes,
    })
}

fn interaction_configurations(
    routing: &RoutingManifest,
) -> Result<Vec<AuthorityStructureConfigurationV1>, String> {
    if routing.artifact_kind != "p5.2b-s12.3-single-axis-decomposition"
        || routing.single_axis_decomposition.axes.len() != 5
        || routing
            .single_axis_decomposition
            .axes
            .iter()
            .any(|axis| axis.individually_sufficient)
        || routing.single_axis_decomposition.interaction_routes.len() != 3
    {
        return Err("invalid S12.3 routing manifest".to_owned());
    }
    let mut configurations = mandatory_authority_structure_configurations_v1()
        .into_iter()
        .take(2)
        .collect::<Vec<_>>();
    for (index, route) in routing
        .single_axis_decomposition
        .interaction_routes
        .iter()
        .enumerate()
    {
        let expected_id = format!("I{}", index + 1);
        if route.id != expected_id || route.status != "routed" {
            return Err(format!("S12 interaction slot {expected_id} is not routed"));
        }
        let axes = route
            .axes
            .ok_or_else(|| format!("S12 interaction slot {expected_id} omits axes"))?;
        let policy = route
            .policy
            .ok_or_else(|| format!("S12 interaction slot {expected_id} omits policy"))?;
        if axes[0].number() >= axes[1].number()
            || policy != AuthorityStructurePolicyV1::interaction(axes[0], axes[1])
        {
            return Err(format!("S12 interaction slot {expected_id} policy differs"));
        }
        configurations.push(AuthorityStructureConfigurationV1 {
            id: expected_id,
            policy,
        });
    }
    Ok(configurations)
}

fn interaction_minimization(
    cases: &[CaseEvidence],
    routing: &RoutingManifest,
) -> Result<InteractionMinimization, String> {
    let governed = cases
        .iter()
        .filter(|case| matches!(case.role, SourceRole::Governed))
        .collect::<Vec<_>>();
    let mut outcomes = Vec::with_capacity(3);
    for route in &routing.single_axis_decomposition.interaction_routes {
        let axes = route
            .axes
            .ok_or_else(|| format!("routed interaction {} omits axes", route.id))?;
        let policy = route
            .policy
            .ok_or_else(|| format!("routed interaction {} omits policy", route.id))?;
        let governed_sources = governed
            .iter()
            .map(|case| governed_disposition(case, &route.id))
            .collect::<Result<Vec<_>, _>>()?;
        outcomes.push(InteractionOutcome {
            id: route.id.clone(),
            axes,
            policy,
            all_sources_qualified: cases.iter().all(|case| {
                configuration(case, &route.id).is_ok_and(|value| value.authority_qualified)
            }),
            governed_sources_qualified: governed_sources
                .iter()
                .all(|value| value.authority_qualified),
            governed_sources,
        });
    }
    if let Some(survivor) = outcomes
        .iter()
        .find(|outcome| outcome.all_sources_qualified && outcome.governed_sources_qualified)
    {
        let survivor_axes = survivor.axes.to_vec();
        let survivor_id = survivor.id.clone();
        return Ok(InteractionMinimization {
            routes: outcomes,
            smallest_qualified_measured_cardinality: 2,
            minimal_sufficient_measured_axes: survivor_axes,
            minimal_sufficient_measured_configuration: survivor_id,
            bounded_minimum_cardinality: [2, 2],
            unmeasured_three_axis_expansion: false,
            statement: "a routed pair is sufficient and no singleton qualifies",
        });
    }
    let necessary = routing
        .single_axis_decomposition
        .axes
        .iter()
        .filter(|axis| axis.necessary)
        .map(|axis| axis.axis)
        .collect::<Vec<_>>();
    if necessary.len() != 4 {
        return Err(
            "no routed pair qualified and the S12.3 four-axis survivor is unavailable".to_owned(),
        );
    }
    Ok(InteractionMinimization {
        routes: outcomes,
        smallest_qualified_measured_cardinality: 4,
        minimal_sufficient_measured_axes: necessary,
        minimal_sufficient_measured_configuration: "A-X5".to_owned(),
        bounded_minimum_cardinality: [3, 4],
        unmeasured_three_axis_expansion: false,
        statement: "no routed pair qualifies; A-X5 is the smallest measured survivor and the bounded minimum cardinality is three through four",
    })
}

fn governed_disposition(
    case: &CaseEvidence,
    configuration_id: &str,
) -> Result<GovernedDisposition, String> {
    let configuration = configuration(case, configuration_id)?;
    Ok(GovernedDisposition {
        label: case.label,
        authority_qualified: configuration.authority_qualified,
        ordered_failures: configuration.failure_vector.ordered_failures.clone(),
        settled_linear_speed_max: configuration.quality.settled_linear_speed_max,
        settled_angular_edge_speed_max: configuration.quality.settled_angular_edge_speed_max,
    })
}

/// Returns `Ok(Ok(improved))` when the candidate introduces no earlier
/// governed failure, and `Ok(Err(()))` when it does. The outer result carries
/// malformed-matrix errors.
fn configuration_improvement(
    case: &CaseEvidence,
    configuration_id: &str,
) -> Result<Result<bool, ()>, String> {
    let control = configuration(case, "P")?;
    let candidate = configuration(case, configuration_id)?;
    let control_rank = first_failure_rank(&control.failure_vector);
    let candidate_rank = first_failure_rank(&candidate.failure_vector);
    if candidate_rank < control_rank {
        return Ok(Err(()));
    }
    if candidate_rank > control_rank {
        return Ok(Ok(true));
    }
    let authority = &case.authority.quality;
    Ok(Ok(failure_magnitude(candidate, authority, candidate_rank)
        < failure_magnitude(control, authority, control_rank)))
}

fn configuration<'a>(
    case: &'a CaseEvidence,
    configuration_id: &str,
) -> Result<&'a ConfigurationEvidence, String> {
    case.configurations
        .iter()
        .find(|value| value.id == configuration_id)
        .ok_or_else(|| {
            format!(
                "S12 case {} omits configuration {configuration_id}",
                case.label
            )
        })
}

fn first_failure_rank(vector: &FailureVector) -> usize {
    [
        vector.non_finite,
        vector.structure,
        vector.authority_response,
        vector.penetration_support,
        vector.awake_speed,
        vector.positive_run,
        vector.energy_envelope,
    ]
    .into_iter()
    .position(|failed| failed)
    .unwrap_or(7)
}

fn failure_magnitude(
    configuration: &ConfigurationEvidence,
    authority: &QualitySummary,
    rank: usize,
) -> f64 {
    let quality = &configuration.quality;
    match rank {
        0 | 1 => 1.0,
        2 => {
            let linear_limit = AUTHORITY_SPEED_MULTIPLIER * authority.settled_linear_speed_max
                + AUTHORITY_SPEED_ADDITIVE_BAND;
            let angular_limit = AUTHORITY_SPEED_MULTIPLIER
                * authority.settled_angular_edge_speed_max
                + AUTHORITY_SPEED_ADDITIVE_BAND;
            (quality.settled_linear_speed_max / linear_limit)
                .max(quality.settled_angular_edge_speed_max / angular_limit)
        }
        3 => {
            if configuration.active_support_pairs.is_empty() {
                f64::INFINITY
            } else {
                let height_limit =
                    AUTHORITY_HEIGHT_ABSOLUTE_BAND.max(authority.maximum_penetration);
                (configuration
                    .delta_from_authority
                    .final_dynamic_center_height_sum
                    .abs()
                    / height_limit)
                    .max(
                        quality.maximum_penetration
                            / (authority.maximum_penetration + AUTHORITY_HEIGHT_ABSOLUTE_BAND),
                    )
            }
        }
        4 => (quality.settled_linear_speed_max / 0.025)
            .max(quality.settled_angular_edge_speed_max / 0.025),
        5 => f64::from(quality.positive_energy_run_max),
        6 => quality.energy_quartile_growth_ratio / authority.energy_quartile_growth_ratio.max(1.0),
        _ => 0.0,
    }
}

fn ordered_axis_pair(
    first: AuthorityStructureAxisV1,
    second: AuthorityStructureAxisV1,
) -> [AuthorityStructureAxisV1; 2] {
    if first.number() < second.number() {
        [first, second]
    } else {
        [second, first]
    }
}

fn scalar_active_support_pairs(solve: &ScalarObservedSolveResultV1) -> Vec<BodyPair> {
    normalize_pairs(
        solve
            .observations
            .last()
            .into_iter()
            .flat_map(|observation| &observation.contact_cache)
            .filter(|point| point.normal_impulse.get() > 0.0)
            .map(cache_pair),
    )
}

fn authority_active_support_pairs(snapshots: &[AuthorityTraceSnapshotV1]) -> Vec<BodyPair> {
    normalize_pairs(
        snapshots
            .last()
            .into_iter()
            .flat_map(|snapshot| &snapshot.cache)
            .filter(|contact| contact.value.normal_impulse.get() > 0.0)
            .map(|contact| BodyPair {
                body_a: contact.key.pair.body_a,
                body_b: contact.key.pair.body_b,
            }),
    )
}

fn cache_pair(point: &ContactCachePointV1) -> BodyPair {
    BodyPair {
        body_a: point.body_a,
        body_b: point.body_b,
    }
}

fn normalize_pairs(pairs: impl IntoIterator<Item = BodyPair>) -> Vec<BodyPair> {
    pairs
        .into_iter()
        .map(|pair| {
            if pair.body_a <= pair.body_b {
                pair
            } else {
                BodyPair {
                    body_a: pair.body_b,
                    body_b: pair.body_a,
                }
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

fn dynamic_bodies(bodies: &[BodyStateV1]) -> impl Iterator<Item = &BodyStateV1> {
    bodies.iter().filter(|body| body.inverse_mass.get() > 0.0)
}

fn kinetic_energy(bodies: &[BodyStateV1]) -> f64 {
    dynamic_bodies(bodies)
        .map(|body| {
            let linear =
                0.5 * f64::from(
                    body.linear_velocity.x * body.linear_velocity.x
                        + body.linear_velocity.y * body.linear_velocity.y,
                ) / f64::from(body.inverse_mass.get());
            let angular = if body.inverse_inertia.get() > 0.0 {
                0.5 * f64::from(body.angular_velocity.get()).powi(2)
                    / f64::from(body.inverse_inertia.get())
            } else {
                0.0
            };
            linear + angular
        })
        .sum()
}

fn support_radius(body: &BodyStateV1) -> f32 {
    match body.shape {
        ShapeV1::Circle { radius } => radius.get(),
        ShapeV1::Rectangle { half_extents } => {
            (half_extents.x * half_extents.x + half_extents.y * half_extents.y).sqrt()
        }
    }
}

fn body_finite(body: &BodyStateV1) -> bool {
    body.position.x.is_finite()
        && body.position.y.is_finite()
        && body.angle.get().is_finite()
        && body.linear_velocity.x.is_finite()
        && body.linear_velocity.y.is_finite()
        && body.angular_velocity.get().is_finite()
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display_error)
}

fn source_identity(source: &OracleSourceIdentity) -> SourceIdentity {
    SourceIdentity {
        revision: source.revision.clone(),
        dirty: source.dirty,
        patch_digest: source.patch_digest.clone(),
    }
}

fn relative_directory(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(display_error)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

fn prepare_staging(output: &Path, directories: &[&str]) -> Result<PathBuf, String> {
    let staging = output.with_extension("partial");
    if output.exists() || staging.exists() {
        return Err(format!(
            "refusing to replace S12 output or staging {}",
            output.display()
        ));
    }
    for directory in directories {
        fs::create_dir_all(staging.join(directory)).map_err(display_error)?;
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
    fs::rename(staging, output).map_err(display_error)?;
    sync_directory(output.parent().unwrap_or(Path::new(".")))
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
    let bytes = encoder.finish().map_err(display_error)?;
    write_artifact(root, relative, &bytes)
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
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("artifact")
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(display_error)?;
    file.write_all(bytes).map_err(display_error)?;
    file.sync_all().map_err(display_error)?;
    fs::rename(temporary, path).map_err(display_error)
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(display_error)
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = git(repository, &["rev-parse", "HEAD"])?;
    let status = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(repository)
        .output()
        .map_err(display_error)?;
    if !status.status.success() {
        return Err("git status failed".to_owned());
    }
    let dirty = !status.stdout.is_empty();
    Ok((revision, dirty, dirty.then(|| sha256_bytes(&status.stdout))))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(display_error)?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(display_error)
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    oracle: PathBuf,
    box2d_source: PathBuf,
    require_clean: bool,
    mode: EvidenceMode,
    routing_manifest: Option<PathBuf>,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut oracle = None;
        let mut box2d_source = None;
        let mut require_clean = false;
        let mut mode = None;
        let mut routing_manifest = None;
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
                "--mode" => {
                    mode = Some(match value.as_str() {
                        "endpoints" => EvidenceMode::Endpoints,
                        "single-axis" => EvidenceMode::SingleAxis,
                        "interactions" => EvidenceMode::Interactions,
                        _ => return Err(format!("unknown S12 evidence mode {value}")),
                    })
                }
                "--routing-manifest" => routing_manifest = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            oracle: oracle.ok_or_else(|| "missing --oracle".to_owned())?,
            box2d_source: box2d_source.ok_or_else(|| "missing --box2d-source".to_owned())?,
            require_clean,
            mode: mode.ok_or_else(|| "missing --mode".to_owned())?,
            routing_manifest,
        })
    }
}
