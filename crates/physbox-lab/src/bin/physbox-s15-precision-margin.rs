use std::{
    collections::BTreeSet,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyId, BodyStateV1, ContactCachePointV1, ScenarioV1, ShapeV1, Validate, canonical_hash,
    sha256_bytes,
};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePolicyV1,
        solve_precision_path_authority_structure_steps_observed,
    },
    precision_margin::{
        FullF64BodyV1, FullF64SolveResultV1, PrecisionPathStatisticsV1, PrecisionPerturbationV1,
        solve_full_f64_authority_v1,
    },
    scalar::{ScalarObservedSolveResultV1, ScalarStepObservationV1},
};
use serde::{Deserialize, Serialize};

const S12_MANIFEST: &str = "evidence/p5.2b-s12.2-authority-endpoints/manifest.json";
const PRECISION_CONTRACT: &str =
    "evidence/p5.2b-s15.1-precision-margin-contract/study-contract.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Manifest {
    artifact_kind: String,
    source_revision: String,
    source_dirty: bool,
    box2d: SourceIdentity,
    authority_bands: AuthorityBands,
    cases: Vec<S12Case>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceIdentity {
    revision: String,
    dirty: bool,
    patch_digest: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityBands {
    speed_multiplier: f64,
    speed_additive: f64,
    height_absolute: f64,
    penetration_absolute: f64,
    awake_product_ceiling: f64,
    positive_energy_run_floor: u32,
    energy_growth_rule: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SourceRole {
    ResponseDiagnostic,
    Context,
    Governed,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Case {
    label: String,
    role: SourceRole,
    requested_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    authority: S12Authority,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Authority {
    exact_replay: bool,
    capture_exact_replay: bool,
    clean_public_trace_provenance: bool,
    quality: QualitySummary,
    active_support_pairs: Vec<BodyPair>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyPair {
    body_a: BodyId,
    body_b: BodyId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StudyContract {
    base_candidate_id: String,
    rejected_candidate_id: String,
    conditional_candidate_id: String,
    conditional_candidate_count: u32,
    measurement_lanes: Vec<MeasurementLane>,
    precision_matrix: PrecisionMatrix,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeasurementLane {
    id: String,
    binary64_scope: String,
    candidate: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionMatrix {
    governed_depths: [u32; 2],
    governed_steps: u32,
    settled_window_inclusive: [u32; 2],
    substeps: u32,
    rows: usize,
    exact_replay: bool,
    candidate_requires_every_row: bool,
    margin_measurement_requires_every_row: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionOutput {
    version: u32,
    base_candidate_id: String,
    rejected_candidate_id: String,
    conditional_candidate_id: String,
    conditional_candidate_count: u32,
    candidate_implemented: bool,
    policy: AuthorityStructurePolicyV1,
    measurement_lanes: Vec<LaneAttestation>,
    precision_matrix: PrecisionMatrixOutput,
    authority_source: AuthoritySource,
    authority_bands: AuthorityBands,
    cases: Vec<CaseOutput>,
    governed_rows: usize,
    governed_rows_qualified: usize,
    every_governed_row_qualified: bool,
    precision_candidate_condition_satisfied: bool,
    margin_measurement_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneAttestation {
    id: String,
    binary64_scope: String,
    candidate: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionMatrixOutput {
    governed_depths: [u32; 2],
    governed_steps: u32,
    settled_window_inclusive: [u32; 2],
    substeps: u32,
    rows: usize,
    exact_replay: bool,
    candidate_requires_every_row: bool,
    margin_measurement_requires_every_row: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoritySource {
    manifest_path: &'static str,
    manifest_sha256: String,
    source_revision: String,
    source_dirty: bool,
    box2d: SourceIdentity,
    native_exact_replay: bool,
    clean_public_trace_provenance: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseOutput {
    label: String,
    role: SourceRole,
    requested_steps: u32,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    authority_quality: QualitySummary,
    authority_active_support_pairs: Vec<BodyPair>,
    rows: Vec<RowOutput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowOutput {
    lane: String,
    arithmetic_scope: String,
    primary_sha256: String,
    replay_sha256: String,
    exact_replay: bool,
    statistics: serde_json::Value,
    quality: QualitySummary,
    active_support_pairs: Vec<BodyPair>,
    delta_from_authority: AuthorityDelta,
    failure_vector: FailureVector,
    authority_qualified: bool,
    capture: ArtifactIdentity,
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
    replay: bool,
    ordered_failures: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathCapture<'a> {
    version: u32,
    lane: &'static str,
    arithmetic_scope: &'static str,
    source_label: &'a str,
    source_scenario_hash: &'a str,
    perturbation: PrecisionPerturbationV1,
    amplitude: f64,
    statistics: PrecisionPathStatisticsV1,
    observations: &'a [ScalarStepObservationV1],
    final_bodies: &'a [BodyStateV1],
    final_cache: &'a [ContactCachePointV1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FullCapture<'a> {
    version: u32,
    lane: &'static str,
    arithmetic_scope: &'static str,
    source_label: &'a str,
    source_scenario_hash: &'a str,
    result: &'a FullF64SolveResultV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output_path = output_path()?;
    let s12_bytes = fs::read(S12_MANIFEST).map_err(display)?;
    let s12_sha256 = sha256_bytes(&s12_bytes);
    let s12: S12Manifest = serde_json::from_slice(&s12_bytes).map_err(display)?;
    if s12.artifact_kind != "p5.2b-s12.2-authority-endpoints"
        || s12.source_dirty
        || s12.cases.iter().any(|case| {
            !case.authority.exact_replay
                || !case.authority.capture_exact_replay
                || !case.authority.clean_public_trace_provenance
        })
    {
        return Err("frozen S12 authority source is not admissible".to_owned());
    }
    let contract_bytes = fs::read(PRECISION_CONTRACT).map_err(display)?;
    let contract: StudyContract = serde_json::from_slice(&contract_bytes).map_err(display)?;
    validate_contract(&contract)?;

    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let capture_directory = output_path
        .parent()
        .ok_or_else(|| "output requires a parent directory".to_owned())?
        .join("captures");
    fs::create_dir_all(&capture_directory).map_err(display)?;
    let mut cases = Vec::new();
    let mut governed_rows = 0;
    let mut governed_rows_qualified = 0;

    for frozen in s12
        .cases
        .iter()
        .filter(|case| case.role == SourceRole::Governed)
    {
        let source_path =
            Path::new("evidence/p5.2b-s12.2-authority-endpoints").join(&frozen.source.path);
        let source_bytes = fs::read(&source_path).map_err(display)?;
        if sha256_bytes(&source_bytes) != frozen.source.sha256 {
            return Err(format!("changed S12 source {}", frozen.label));
        }
        let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display)?;
        source.validate().map_err(display)?;
        if canonical_hash(&source).map_err(display)? != frozen.source_scenario_hash
            || source.requested_steps != frozen.requested_steps
            || source.settings.substeps != contract.precision_matrix.substeps
        {
            return Err(format!("source contract drift for {}", frozen.label));
        }
        let mut rows = Vec::new();

        let path_primary = solve_precision_path_authority_structure_steps_observed(
            &source,
            policy,
            PrecisionPerturbationV1::Nearest,
            0.0,
            false,
        )
        .map_err(display)?;
        let path_replay = solve_precision_path_authority_structure_steps_observed(
            &source,
            policy,
            PrecisionPerturbationV1::Nearest,
            0.0,
            false,
        )
        .map_err(display)?;
        let path_primary_hash = path_replay_hash(&path_primary)?;
        let path_replay_hash = path_replay_hash(&path_replay)?;
        let path_exact = path_primary == path_replay && path_primary_hash == path_replay_hash;
        let path_quality = scalar_quality(&source, &path_primary.solve)?;
        let path_pairs = scalar_active_support_pairs(&path_primary.solve);
        let path_failure = failure_vector(
            &source,
            &path_quality,
            &path_pairs,
            &frozen.authority.quality,
            &frozen.authority.active_support_pairs,
            &s12.authority_bands,
            path_exact,
        );
        let path_qualified = path_failure.ordered_failures.is_empty();
        let path_capture_name = format!("{}-cancellation-path-f64.json.gz", frozen.label);
        let path_capture_bytes = capture_bytes(PathCapture {
            version: 1,
            lane: "cancellation-path-f64",
            arithmetic_scope: "original-authority-separation-dag-only-binary64",
            source_label: &frozen.label,
            source_scenario_hash: &frozen.source_scenario_hash,
            perturbation: PrecisionPerturbationV1::Nearest,
            amplitude: 0.0,
            statistics: path_primary.statistics,
            observations: &path_primary.solve.observations,
            final_bodies: &path_primary.solve.result.bodies,
            final_cache: &path_primary.solve.result.contact_cache,
        })?;
        fs::write(
            capture_directory.join(&path_capture_name),
            &path_capture_bytes,
        )
        .map_err(display)?;
        governed_rows += 1;
        governed_rows_qualified += usize::from(path_qualified);
        rows.push(RowOutput {
            lane: "cancellation-path-f64".to_owned(),
            arithmetic_scope: "original-authority-separation-dag-only-binary64".to_owned(),
            primary_sha256: path_primary_hash,
            replay_sha256: path_replay_hash,
            exact_replay: path_exact,
            statistics: serde_json::to_value(path_primary.statistics).map_err(display)?,
            delta_from_authority: authority_delta(&path_quality, &frozen.authority.quality),
            quality: path_quality,
            active_support_pairs: path_pairs,
            failure_vector: path_failure,
            authority_qualified: path_qualified,
            capture: ArtifactIdentity {
                path: format!("captures/{path_capture_name}"),
                sha256: sha256_bytes(&path_capture_bytes),
            },
        });

        let full_primary = solve_full_f64_authority_v1(&source).map_err(display)?;
        let full_replay = solve_full_f64_authority_v1(&source).map_err(display)?;
        let full_primary_hash = full_replay_hash(&full_primary)?;
        let full_replay_hash = full_replay_hash(&full_replay)?;
        let full_exact = full_primary == full_replay && full_primary_hash == full_replay_hash;
        let full_quality = full_quality(&source, &full_primary)?;
        let full_pairs = full_active_support_pairs(&full_primary);
        let full_failure = failure_vector(
            &source,
            &full_quality,
            &full_pairs,
            &frozen.authority.quality,
            &frozen.authority.active_support_pairs,
            &s12.authority_bands,
            full_exact,
        );
        let full_qualified = full_failure.ordered_failures.is_empty();
        let full_capture_name = format!("{}-full-solve-f64.json.gz", frozen.label);
        let full_capture_bytes = capture_bytes(FullCapture {
            version: 1,
            lane: "full-solve-f64",
            arithmetic_scope: "complete-independent-scalar-recurrence-binary64",
            source_label: &frozen.label,
            source_scenario_hash: &frozen.source_scenario_hash,
            result: &full_primary,
        })?;
        fs::write(
            capture_directory.join(&full_capture_name),
            &full_capture_bytes,
        )
        .map_err(display)?;
        governed_rows += 1;
        governed_rows_qualified += usize::from(full_qualified);
        rows.push(RowOutput {
            lane: "full-solve-f64".to_owned(),
            arithmetic_scope: "complete-independent-scalar-recurrence-binary64".to_owned(),
            primary_sha256: full_primary_hash,
            replay_sha256: full_replay_hash,
            exact_replay: full_exact,
            statistics: serde_json::to_value(full_primary.statistics).map_err(display)?,
            delta_from_authority: authority_delta(&full_quality, &frozen.authority.quality),
            quality: full_quality,
            active_support_pairs: full_pairs,
            failure_vector: full_failure,
            authority_qualified: full_qualified,
            capture: ArtifactIdentity {
                path: format!("captures/{full_capture_name}"),
                sha256: sha256_bytes(&full_capture_bytes),
            },
        });

        cases.push(CaseOutput {
            label: frozen.label.clone(),
            role: frozen.role,
            requested_steps: frozen.requested_steps,
            source_scenario_hash: frozen.source_scenario_hash.clone(),
            projection_hash: frozen.projection_hash.clone(),
            source: frozen.source.clone(),
            authority_quality: frozen.authority.quality.clone(),
            authority_active_support_pairs: frozen.authority.active_support_pairs.clone(),
            rows,
        });
    }

    let every_governed_row_qualified = governed_rows == contract.precision_matrix.rows
        && governed_rows_qualified == contract.precision_matrix.rows;
    let output = PrecisionOutput {
        version: 1,
        base_candidate_id: contract.base_candidate_id,
        rejected_candidate_id: contract.rejected_candidate_id,
        conditional_candidate_id: contract.conditional_candidate_id,
        conditional_candidate_count: contract.conditional_candidate_count,
        candidate_implemented: false,
        policy,
        measurement_lanes: contract
            .measurement_lanes
            .into_iter()
            .map(|lane| LaneAttestation {
                id: lane.id,
                binary64_scope: lane.binary64_scope,
                candidate: lane.candidate,
            })
            .collect(),
        precision_matrix: PrecisionMatrixOutput {
            governed_depths: contract.precision_matrix.governed_depths,
            governed_steps: contract.precision_matrix.governed_steps,
            settled_window_inclusive: contract.precision_matrix.settled_window_inclusive,
            substeps: contract.precision_matrix.substeps,
            rows: contract.precision_matrix.rows,
            exact_replay: contract.precision_matrix.exact_replay,
            candidate_requires_every_row: contract.precision_matrix.candidate_requires_every_row,
            margin_measurement_requires_every_row: contract
                .precision_matrix
                .margin_measurement_requires_every_row,
        },
        authority_source: AuthoritySource {
            manifest_path: S12_MANIFEST,
            manifest_sha256: s12_sha256,
            source_revision: s12.source_revision,
            source_dirty: s12.source_dirty,
            box2d: s12.box2d,
            native_exact_replay: true,
            clean_public_trace_provenance: true,
        },
        authority_bands: s12.authority_bands,
        cases,
        governed_rows,
        governed_rows_qualified,
        every_governed_row_qualified,
        precision_candidate_condition_satisfied: every_governed_row_qualified,
        margin_measurement_required: true,
    };
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(display)?;
    }
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&output).map_err(display)?,
    )
    .map_err(display)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s15.2-f64-precision-limit governed={governed_rows_qualified}/{governed_rows} precision_candidate_condition={} margin_required=true output={}",
        output.precision_candidate_condition_satisfied,
        output_path.display()
    );
    Ok(())
}

fn validate_contract(contract: &StudyContract) -> Result<(), String> {
    if contract.base_candidate_id != "authority-x1-x4-current-x5-v1"
        || contract.rejected_candidate_id != "authority-band-cancellation-robust-v1"
        || contract.conditional_candidate_id != "authority-exact-separation-two-float-v1"
        || contract.conditional_candidate_count != 1
        || contract.measurement_lanes.len() != 2
        || contract.measurement_lanes.iter().any(|lane| lane.candidate)
        || contract.measurement_lanes[0].id != "cancellation-path-f64"
        || contract.measurement_lanes[1].id != "full-solve-f64"
        || contract.precision_matrix.governed_depths != [11, 16]
        || contract.precision_matrix.governed_steps != 1_200
        || contract.precision_matrix.settled_window_inclusive != [900, 1_200]
        || contract.precision_matrix.substeps != 4
        || contract.precision_matrix.rows != 4
        || !contract.precision_matrix.exact_replay
        || !contract.precision_matrix.candidate_requires_every_row
        || contract
            .precision_matrix
            .margin_measurement_requires_every_row
    {
        return Err("S15.1 precision contract drift".to_owned());
    }
    Ok(())
}

fn scalar_quality(
    source: &ScenarioV1,
    solve: &ScalarObservedSolveResultV1,
) -> Result<QualitySummary, String> {
    let observations = solve
        .observations
        .iter()
        .map(|observation| QualityObservation {
            completed_steps: observation.completed_steps,
            bodies: observation
                .bodies
                .iter()
                .map(QualityBody::from)
                .collect::<Vec<_>>(),
            maximum_penetration: f64::from(observation.maximum_penetration),
            maximum_separation: f64::from(observation.maximum_separation),
            active_normal_points: observation.active_normal_points,
            total_normal_impulse: observation.total_normal_impulse,
        })
        .collect::<Vec<_>>();
    quality(source, &observations)
}

fn full_quality(
    source: &ScenarioV1,
    solve: &FullF64SolveResultV1,
) -> Result<QualitySummary, String> {
    let observations = solve
        .observations
        .iter()
        .map(|observation| QualityObservation {
            completed_steps: observation.completed_steps,
            bodies: observation
                .bodies
                .iter()
                .map(QualityBody::from)
                .collect::<Vec<_>>(),
            maximum_penetration: observation.maximum_penetration,
            maximum_separation: observation.maximum_separation,
            active_normal_points: observation.active_normal_points,
            total_normal_impulse: observation.total_normal_impulse,
        })
        .collect::<Vec<_>>();
    quality(source, &observations)
}

#[derive(Clone)]
struct QualityBody {
    id: BodyId,
    shape: ShapeV1,
    position: [f64; 2],
    angle: f64,
    linear_velocity: [f64; 2],
    angular_velocity: f64,
    inverse_mass: f64,
    inverse_inertia: f64,
}

impl From<&BodyStateV1> for QualityBody {
    fn from(body: &BodyStateV1) -> Self {
        Self {
            id: body.id,
            shape: body.shape,
            position: [f64::from(body.position.x), f64::from(body.position.y)],
            angle: f64::from(body.angle.get()),
            linear_velocity: [
                f64::from(body.linear_velocity.x),
                f64::from(body.linear_velocity.y),
            ],
            angular_velocity: f64::from(body.angular_velocity.get()),
            inverse_mass: f64::from(body.inverse_mass.get()),
            inverse_inertia: f64::from(body.inverse_inertia.get()),
        }
    }
}

impl From<&FullF64BodyV1> for QualityBody {
    fn from(body: &FullF64BodyV1) -> Self {
        Self {
            id: body.id,
            shape: body.shape,
            position: body.position,
            angle: body.angle,
            linear_velocity: body.linear_velocity,
            angular_velocity: body.angular_velocity,
            inverse_mass: body.inverse_mass,
            inverse_inertia: body.inverse_inertia,
        }
    }
}

struct QualityObservation {
    completed_steps: u32,
    bodies: Vec<QualityBody>,
    maximum_penetration: f64,
    maximum_separation: f64,
    active_normal_points: u32,
    total_normal_impulse: f64,
}

fn quality(
    source: &ScenarioV1,
    observations: &[QualityObservation],
) -> Result<QualitySummary, String> {
    let settled_start_step = 900;
    let settled = observations
        .iter()
        .filter(|observation| observation.completed_steps >= settled_start_step)
        .collect::<Vec<_>>();
    if settled.is_empty() {
        return Err("S15 quality has no settled observations".to_owned());
    }
    let energy = settled
        .iter()
        .map(|observation| kinetic_energy(&observation.bodies))
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
        .ok_or_else(|| "S15 quality has no final observation".to_owned())?;
    let (structure_pass, structure_reason) = centered_structure(source, &final_observation.bodies);
    Ok(QualitySummary {
        settled_start_step,
        settled_end_step: source.requested_steps,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| body.linear_velocity[0].hypot(body.linear_velocity[1]))
            .fold(0.0, f64::max),
        settled_angular_edge_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| body.angular_velocity.abs() * support_radius(body))
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
        final_dynamic_center_height_sum: dynamic_bodies(&final_observation.bodies)
            .map(|body| body.position[1])
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

fn centered_structure(source: &ScenarioV1, bodies: &[QualityBody]) -> (bool, String) {
    if source.bodies.len() != bodies.len() {
        return (false, "body count changed".to_owned());
    }
    let dynamic = bodies
        .iter()
        .filter(|body| body.inverse_mass > 0.0)
        .collect::<Vec<_>>();
    if dynamic.iter().any(|body| {
        !body.position[0].is_finite() || !body.position[1].is_finite() || shape_upper_y(body) <= 0.0
    }) {
        return (
            false,
            "a dynamic body became invalid or escaped below the support".to_owned(),
        );
    }
    if dynamic
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id || pair[0].position[1] >= pair[1].position[1])
    {
        return (false, "rectangle stack order changed".to_owned());
    }
    let speculative_distance = source
        .settings
        .contact_policy
        .map_or(0.0, |policy| f64::from(policy.speculative_distance.get()));
    let mut previous_top = 0.0;
    let mut previous_horizontal: Option<(f64, f64)> = None;
    for body in dynamic {
        let ShapeV1::Rectangle { .. } = body.shape else {
            return (
                false,
                "centered stack contains a non-rectangle body".to_owned(),
            );
        };
        let (extent_x, extent_y) = shape_extents(body);
        if body.angle.cos().abs() < body.angle.sin().abs() {
            return (
                false,
                "a rectangle rotated beyond the upright range".to_owned(),
            );
        }
        let lower = body.position[1] - extent_y;
        if lower > previous_top + speculative_distance {
            return (false, "the vertical support chain separated".to_owned());
        }
        let horizontal = (body.position[0] - extent_x, body.position[0] + extent_x);
        if previous_horizontal.is_some_and(|(left, right)| {
            horizontal.0 > right + speculative_distance
                || left > horizontal.1 + speculative_distance
        }) {
            return (false, "the horizontal support chain separated".to_owned());
        }
        previous_top = body.position[1] + extent_y;
        previous_horizontal = Some(horizontal);
    }
    (
        true,
        "all rectangle generations remained finite, supported, and vertically ordered".to_owned(),
    )
}

fn shape_extents(body: &QualityBody) -> (f64, f64) {
    match body.shape {
        ShapeV1::Circle { radius } => (f64::from(radius.get()), f64::from(radius.get())),
        ShapeV1::Rectangle { half_extents } => {
            let cosine = body.angle.cos().abs();
            let sine = body.angle.sin().abs();
            (
                cosine * f64::from(half_extents.x) + sine * f64::from(half_extents.y),
                sine * f64::from(half_extents.x) + cosine * f64::from(half_extents.y),
            )
        }
    }
}

fn shape_upper_y(body: &QualityBody) -> f64 {
    body.position[1] + shape_extents(body).1
}

fn failure_vector(
    _source: &ScenarioV1,
    candidate: &QualitySummary,
    candidate_pairs: &[BodyPair],
    authority: &QualitySummary,
    authority_pairs: &[BodyPair],
    bands: &AuthorityBands,
    exact_replay: bool,
) -> FailureVector {
    let non_finite = !candidate.finite;
    let structure = !candidate.structure_pass;
    let authority_response = candidate.settled_linear_speed_max
        > bands.speed_multiplier * authority.settled_linear_speed_max + bands.speed_additive
        || candidate.settled_angular_edge_speed_max
            > bands.speed_multiplier * authority.settled_angular_edge_speed_max
                + bands.speed_additive;
    let height_band = bands.height_absolute.max(authority.maximum_penetration);
    let penetration_support = candidate_pairs != authority_pairs
        || (candidate.final_dynamic_center_height_sum - authority.final_dynamic_center_height_sum)
            .abs()
            > height_band
        || candidate.maximum_penetration
            > authority.maximum_penetration + bands.penetration_absolute;
    let awake_speed = candidate.settled_linear_speed_max > bands.awake_product_ceiling
        || candidate.settled_angular_edge_speed_max > bands.awake_product_ceiling;
    let positive_run = candidate.positive_energy_run_max
        > authority
            .positive_energy_run_max
            .max(bands.positive_energy_run_floor);
    let authority_growth_band = authority.energy_quartile_growth_ratio.max(1.0);
    let energy_envelope = candidate.energy_quartile_growth_ratio
        > authority_growth_band + 1.0e-12 + 1.0e-5 * authority_growth_band;
    let replay = !exact_replay;
    let ordered_failures = [
        (non_finite, "non-finite"),
        (structure, "structure"),
        (authority_response, "authority-response"),
        (penetration_support, "penetration-support"),
        (awake_speed, "awake-speed"),
        (positive_run, "positive-run"),
        (energy_envelope, "energy-envelope"),
        (replay, "replay"),
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
        replay,
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

fn scalar_active_support_pairs(solve: &ScalarObservedSolveResultV1) -> Vec<BodyPair> {
    normalize_pairs(
        solve
            .observations
            .last()
            .into_iter()
            .flat_map(|observation| &observation.contact_cache)
            .filter(|point| point.normal_impulse.get() > 0.0)
            .map(|point| BodyPair {
                body_a: point.body_a,
                body_b: point.body_b,
            }),
    )
}

fn full_active_support_pairs(solve: &FullF64SolveResultV1) -> Vec<BodyPair> {
    normalize_pairs(
        solve
            .observations
            .last()
            .into_iter()
            .flat_map(|observation| &observation.contact_cache)
            .filter(|point| point.normal_impulse > 0.0)
            .map(|point| BodyPair {
                body_a: point.body_a,
                body_b: point.body_b,
            }),
    )
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

fn path_replay_hash(
    result: &physbox_lab::authority_structure::PrecisionPathAuthoritySolveResultV1,
) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Projection<'a> {
        statistics: PrecisionPathStatisticsV1,
        observations: &'a [ScalarStepObservationV1],
        final_bodies: &'a [BodyStateV1],
        final_cache: &'a [ContactCachePointV1],
    }
    serde_json::to_vec(&Projection {
        statistics: result.statistics,
        observations: &result.solve.observations,
        final_bodies: &result.solve.result.bodies,
        final_cache: &result.solve.result.contact_cache,
    })
    .map(|bytes| sha256_bytes(&bytes))
    .map_err(display)
}

fn full_replay_hash(result: &FullF64SolveResultV1) -> Result<String, String> {
    serde_json::to_vec(result)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display)
}

fn capture_bytes(capture: impl Serialize) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(&capture).map_err(display)?;
    let mut output = Vec::new();
    {
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .write(&mut output, Compression::best());
        encoder.write_all(&json).map_err(display)?;
        encoder.finish().map_err(display)?;
    }
    Ok(output)
}

fn dynamic_bodies(bodies: &[QualityBody]) -> impl Iterator<Item = &QualityBody> {
    bodies.iter().filter(|body| body.inverse_mass > 0.0)
}

fn kinetic_energy(bodies: &[QualityBody]) -> f64 {
    dynamic_bodies(bodies)
        .map(|body| {
            let linear = 0.5
                * (body.linear_velocity[0] * body.linear_velocity[0]
                    + body.linear_velocity[1] * body.linear_velocity[1])
                / body.inverse_mass;
            let angular = if body.inverse_inertia > 0.0 {
                0.5 * body.angular_velocity * body.angular_velocity / body.inverse_inertia
            } else {
                0.0
            };
            linear + angular
        })
        .sum()
}

fn support_radius(body: &QualityBody) -> f64 {
    match body.shape {
        ShapeV1::Circle { radius } => f64::from(radius.get()),
        ShapeV1::Rectangle { half_extents } => {
            f64::from(half_extents.x).hypot(f64::from(half_extents.y))
        }
    }
}

fn body_finite(body: &QualityBody) -> bool {
    body.position[0].is_finite()
        && body.position[1].is_finite()
        && body.angle.is_finite()
        && body.linear_velocity[0].is_finite()
        && body.linear_velocity[1].is_finite()
        && body.angular_velocity.is_finite()
}

fn output_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--precision-output"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: physbox-s15-precision-margin --precision-output PATH".to_owned()),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
