use std::{collections::BTreeSet, env, fs, path::PathBuf};

use physbox_contracts::{
    BodyId, BodyStateV1, ContactCachePointV1, ScenarioV1, ShapeV1, Validate, canonical_hash,
    sha256_bytes,
};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePolicyV1, PrecisionPathAuthoritySolveResultV1,
        solve_precision_path_authority_structure_steps_observed,
    },
    precision_margin::{PrecisionPathStatisticsV1, PrecisionPerturbationV1},
    scalar::{ScalarObservedSolveResultV1, ScalarStepObservationV1},
    settling::centered_stack_structure,
};
use serde::{Deserialize, Serialize};

const S12_MANIFEST: &str = "evidence/p5.2b-s12.2-authority-endpoints/manifest.json";
const PRECISION_CONTRACT: &str =
    "evidence/p5.2b-s15.1-precision-margin-contract/study-contract.json";
const PRECISION_SUMMARY: &str = "evidence/p5.2b-s15.2-f64-precision-limit/summary.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Manifest {
    artifact_kind: String,
    source_dirty: bool,
    authority_bands: AuthorityBands,
    cases: Vec<S12Case>,
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
    version: u32,
    base_candidate_id: String,
    conditional_candidate_id: String,
    margin_curve: MarginCurveContract,
    precision_matrix: PrecisionMatrixContract,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarginCurveContract {
    governed_depth: u32,
    baseline: String,
    perturbed_value: String,
    amplitudes_meters: AmplitudeContract,
    schedules: Vec<String>,
    hash_sign_expression: String,
    exact_replay: bool,
    certification: String,
    study_margin: String,
    reentry_does_not_extend_margin: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmplitudeContract {
    zero_included: bool,
    power_of_two_exponent_inclusive: [i32; 2],
    integer_exponents_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionMatrixContract {
    candidate_requires_every_row: bool,
    margin_measurement_requires_every_row: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionSummary {
    governed_rows: usize,
    governed_rows_qualified: usize,
    precision_candidate_condition_satisfied: bool,
    margin_measurement_required: bool,
    candidate_implemented: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarginOutput {
    version: u32,
    base_candidate_id: String,
    conditional_candidate_id: String,
    candidate_implemented: bool,
    policy: AuthorityStructurePolicyV1,
    source_label: String,
    source_scenario_hash: String,
    projection_hash: String,
    source: ArtifactIdentity,
    authority_quality: QualitySummary,
    authority_active_support_pairs: Vec<BodyPair>,
    authority_bands: AuthorityBands,
    precision_summary: BoundInput,
    contract: MarginContractOutput,
    baseline: CellOutput,
    schedules: Vec<ScheduleOutput>,
    study_margin_meters: f64,
    study_margin_exponent: Option<i32>,
    study_margin_censored_at_upper_lattice: bool,
    all_cells_exact_replay: bool,
    precision_candidate_condition_satisfied: bool,
    eft_feasibility_bound_derived: bool,
    conditional_candidate_admitted: bool,
    route: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BoundInput {
    path: &'static str,
    sha256: String,
    governed_rows: usize,
    governed_rows_qualified: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarginContractOutput {
    governed_depth: u32,
    baseline: String,
    perturbed_value: String,
    zero_included: bool,
    power_of_two_exponent_inclusive: [i32; 2],
    integer_exponents_only: bool,
    schedules: Vec<String>,
    hash_sign_expression: String,
    exact_replay: bool,
    certification: String,
    study_margin: String,
    reentry_does_not_extend_margin: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleOutput {
    perturbation: PrecisionPerturbationV1,
    perturbation_id: &'static str,
    cells: Vec<CellOutput>,
    certified_margin_meters: f64,
    certified_margin_exponent: Option<i32>,
    first_failure_exponent: Option<i32>,
    reentry_exponents: Vec<i32>,
    censored_at_upper_lattice: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CellOutput {
    amplitude_meters: f64,
    amplitude_exponent: Option<i32>,
    primary_sha256: String,
    replay_sha256: String,
    exact_replay: bool,
    statistics: PrecisionPathStatisticsV1,
    quality: QualitySummary,
    active_support_pairs: Vec<BodyPair>,
    failure_vector: FailureVector,
    authority_qualified: bool,
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

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output_path = output_path()?;
    let contract_bytes = fs::read(PRECISION_CONTRACT).map_err(display)?;
    let contract: StudyContract = serde_json::from_slice(&contract_bytes).map_err(display)?;
    validate_contract(&contract)?;
    let precision_bytes = fs::read(PRECISION_SUMMARY).map_err(display)?;
    let precision: PrecisionSummary = serde_json::from_slice(&precision_bytes).map_err(display)?;
    if precision.governed_rows != 4
        || precision.governed_rows_qualified != 1
        || precision.precision_candidate_condition_satisfied
        || !precision.margin_measurement_required
        || precision.candidate_implemented
    {
        return Err("S15.2 precision summary drift".to_owned());
    }
    let s12_bytes = fs::read(S12_MANIFEST).map_err(display)?;
    let s12: S12Manifest = serde_json::from_slice(&s12_bytes).map_err(display)?;
    if s12.artifact_kind != "p5.2b-s12.2-authority-endpoints" || s12.source_dirty {
        return Err("frozen S12 authority source is not admissible".to_owned());
    }
    let frozen = s12
        .cases
        .iter()
        .find(|case| case.label == "governed-centered-sixteen-soft")
        .ok_or_else(|| "missing governed depth-16 source".to_owned())?;
    if frozen.role != SourceRole::Governed
        || !frozen.authority.exact_replay
        || !frozen.authority.capture_exact_replay
        || !frozen.authority.clean_public_trace_provenance
        || frozen.requested_steps != 1_200
    {
        return Err("depth-16 authority source drift".to_owned());
    }
    let source_path =
        PathBuf::from("evidence/p5.2b-s12.2-authority-endpoints").join(&frozen.source.path);
    let source_bytes = fs::read(&source_path).map_err(display)?;
    if sha256_bytes(&source_bytes) != frozen.source.sha256 {
        return Err("depth-16 source digest drift".to_owned());
    }
    let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display)?;
    source.validate().map_err(display)?;
    if canonical_hash(&source).map_err(display)? != frozen.source_scenario_hash {
        return Err("depth-16 source canonical hash drift".to_owned());
    }

    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let baseline = run_cell(
        &source,
        policy,
        PrecisionPerturbationV1::Nearest,
        0.0,
        None,
        &frozen.authority.quality,
        &frozen.authority.active_support_pairs,
        &s12.authority_bands,
    )?;
    if !baseline.authority_qualified {
        return Err("depth-16 zero-amplitude baseline no longer qualifies".to_owned());
    }

    let lower = contract
        .margin_curve
        .amplitudes_meters
        .power_of_two_exponent_inclusive[0];
    let upper = contract
        .margin_curve
        .amplitudes_meters
        .power_of_two_exponent_inclusive[1];
    let mut schedules = Vec::new();
    for perturbation in PrecisionPerturbationV1::SIGNED {
        let mut cells = Vec::with_capacity((upper - lower + 2) as usize);
        cells.push(run_cell(
            &source,
            policy,
            perturbation,
            0.0,
            None,
            &frozen.authority.quality,
            &frozen.authority.active_support_pairs,
            &s12.authority_bands,
        )?);
        for exponent in lower..=upper {
            cells.push(run_cell(
                &source,
                policy,
                perturbation,
                2.0_f64.powi(exponent),
                Some(exponent),
                &frozen.authority.quality,
                &frozen.authority.active_support_pairs,
                &s12.authority_bands,
            )?);
        }
        let mut prefix_open = cells[0].authority_qualified;
        let mut certified_margin_meters = 0.0;
        let mut certified_margin_exponent = None;
        let mut first_failure_exponent = None;
        let mut reentry_exponents = Vec::new();
        for cell in cells.iter().skip(1) {
            let exponent = cell
                .amplitude_exponent
                .expect("nonzero margin cell exponent");
            if prefix_open && cell.authority_qualified {
                certified_margin_meters = cell.amplitude_meters;
                certified_margin_exponent = Some(exponent);
            } else if prefix_open {
                prefix_open = false;
                first_failure_exponent = Some(exponent);
            } else if cell.authority_qualified {
                reentry_exponents.push(exponent);
            }
        }
        let censored_at_upper_lattice = first_failure_exponent.is_none();
        schedules.push(ScheduleOutput {
            perturbation,
            perturbation_id: perturbation.id(),
            cells,
            certified_margin_meters,
            certified_margin_exponent,
            first_failure_exponent,
            reentry_exponents,
            censored_at_upper_lattice,
        });
    }
    let study_margin_meters = schedules
        .iter()
        .map(|schedule| schedule.certified_margin_meters)
        .fold(f64::INFINITY, f64::min);
    let study_margin_exponent = schedules
        .iter()
        .min_by(|left, right| {
            left.certified_margin_meters
                .total_cmp(&right.certified_margin_meters)
        })
        .and_then(|schedule| schedule.certified_margin_exponent);
    let study_margin_censored_at_upper_lattice = schedules
        .iter()
        .all(|schedule| schedule.censored_at_upper_lattice);
    let all_cells_exact_replay = baseline.exact_replay
        && schedules
            .iter()
            .flat_map(|schedule| &schedule.cells)
            .all(|cell| cell.exact_replay);
    let output = MarginOutput {
        version: 1,
        base_candidate_id: contract.base_candidate_id,
        conditional_candidate_id: contract.conditional_candidate_id,
        candidate_implemented: false,
        policy,
        source_label: frozen.label.clone(),
        source_scenario_hash: frozen.source_scenario_hash.clone(),
        projection_hash: frozen.projection_hash.clone(),
        source: frozen.source.clone(),
        authority_quality: frozen.authority.quality.clone(),
        authority_active_support_pairs: frozen.authority.active_support_pairs.clone(),
        authority_bands: s12.authority_bands,
        precision_summary: BoundInput {
            path: PRECISION_SUMMARY,
            sha256: sha256_bytes(&precision_bytes),
            governed_rows: precision.governed_rows,
            governed_rows_qualified: precision.governed_rows_qualified,
        },
        contract: MarginContractOutput {
            governed_depth: contract.margin_curve.governed_depth,
            baseline: contract.margin_curve.baseline,
            perturbed_value: contract.margin_curve.perturbed_value,
            zero_included: contract.margin_curve.amplitudes_meters.zero_included,
            power_of_two_exponent_inclusive: contract
                .margin_curve
                .amplitudes_meters
                .power_of_two_exponent_inclusive,
            integer_exponents_only: contract
                .margin_curve
                .amplitudes_meters
                .integer_exponents_only,
            schedules: contract.margin_curve.schedules,
            hash_sign_expression: contract.margin_curve.hash_sign_expression,
            exact_replay: contract.margin_curve.exact_replay,
            certification: contract.margin_curve.certification,
            study_margin: contract.margin_curve.study_margin,
            reentry_does_not_extend_margin: contract.margin_curve.reentry_does_not_extend_margin,
        },
        baseline,
        schedules,
        study_margin_meters,
        study_margin_exponent,
        study_margin_censored_at_upper_lattice,
        all_cells_exact_replay,
        precision_candidate_condition_satisfied: false,
        eft_feasibility_bound_derived: false,
        conditional_candidate_admitted: false,
        route: "no-candidate-decomposition-addendum",
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
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s15.3-depth16-margin study_margin={study_margin_meters:.17e} exponent={:?} exact_replay={} precision_candidate_condition=false candidate_admitted=false output={}",
        output.study_margin_exponent,
        output.all_cells_exact_replay,
        output_path.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_cell(
    source: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    perturbation: PrecisionPerturbationV1,
    amplitude: f64,
    amplitude_exponent: Option<i32>,
    authority: &QualitySummary,
    authority_pairs: &[BodyPair],
    bands: &AuthorityBands,
) -> Result<CellOutput, String> {
    let primary = solve_precision_path_authority_structure_steps_observed(
        source,
        policy,
        perturbation,
        amplitude,
        false,
    )
    .map_err(display)?;
    let replay = solve_precision_path_authority_structure_steps_observed(
        source,
        policy,
        perturbation,
        amplitude,
        false,
    )
    .map_err(display)?;
    let primary_sha256 = replay_hash(&primary)?;
    let replay_sha256 = replay_hash(&replay)?;
    let exact_replay = primary == replay && primary_sha256 == replay_sha256;
    let quality = scalar_quality(source, &primary.solve)?;
    let active_support_pairs = scalar_active_support_pairs(&primary.solve);
    let failure_vector = failure_vector(
        &quality,
        &active_support_pairs,
        authority,
        authority_pairs,
        bands,
        exact_replay,
    );
    let authority_qualified = failure_vector.ordered_failures.is_empty();
    Ok(CellOutput {
        amplitude_meters: amplitude,
        amplitude_exponent,
        primary_sha256,
        replay_sha256,
        exact_replay,
        statistics: primary.statistics,
        quality,
        active_support_pairs,
        failure_vector,
        authority_qualified,
    })
}

fn validate_contract(contract: &StudyContract) -> Result<(), String> {
    let expected_schedules = PrecisionPerturbationV1::ALL
        .map(|schedule| schedule.id().to_owned())
        .to_vec();
    if contract.version != 2
        || contract.base_candidate_id != "authority-x1-x4-current-x5-v1"
        || contract.conditional_candidate_id != "authority-exact-separation-two-float-v1"
        || contract.margin_curve.governed_depth != 16
        || contract.margin_curve.baseline != "cancellation-path-f64"
        || contract
            .margin_curve
            .amplitudes_meters
            .power_of_two_exponent_inclusive
            != [-40, -16]
        || !contract.margin_curve.amplitudes_meters.zero_included
        || !contract
            .margin_curve
            .amplitudes_meters
            .integer_exponents_only
        || contract.margin_curve.schedules != expected_schedules
        || !contract.margin_curve.exact_replay
        || contract.margin_curve.certification != "largest-all-passing-prefix"
        || !contract.margin_curve.reentry_does_not_extend_margin
        || !contract.precision_matrix.candidate_requires_every_row
        || contract
            .precision_matrix
            .margin_measurement_requires_every_row
    {
        return Err("S15.1 margin contract drift".to_owned());
    }
    Ok(())
}

fn scalar_quality(
    source: &ScenarioV1,
    solve: &ScalarObservedSolveResultV1,
) -> Result<QualitySummary, String> {
    let settled = solve
        .observations
        .iter()
        .filter(|observation| observation.completed_steps >= 900)
        .collect::<Vec<_>>();
    if settled.is_empty() {
        return Err("S15 margin quality has no settled observations".to_owned());
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
    let energy_quartile_growth_ratio = if first_energy_quartile_max > 0.0 {
        last_energy_quartile_max / first_energy_quartile_max
    } else if last_energy_quartile_max == 0.0 {
        1.0
    } else {
        f64::INFINITY
    };
    let final_observation = solve
        .observations
        .last()
        .ok_or_else(|| "S15 margin quality has no final observation".to_owned())?;
    let (structure_pass, structure_reason) =
        centered_stack_structure(source, &final_observation.bodies, false, 0.0);
    Ok(QualitySummary {
        settled_start_step: 900,
        settled_end_step: source.requested_steps,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| f64::from(body.linear_velocity.x).hypot(f64::from(body.linear_velocity.y)))
            .fold(0.0, f64::max),
        settled_angular_edge_speed_max: settled
            .iter()
            .flat_map(|observation| dynamic_bodies(&observation.bodies))
            .map(|body| f64::from(body.angular_velocity.get().abs() * support_radius(body)))
            .fold(0.0, f64::max),
        first_energy_quartile_max,
        last_energy_quartile_max,
        energy_quartile_growth_ratio,
        positive_energy_run_max,
        energy_envelope_nonincreasing: last_energy_quartile_max
            <= first_energy_quartile_max + 1.0e-12 + 1.0e-5 * first_energy_quartile_max.abs()
            && positive_energy_run_max < 5,
        maximum_penetration: settled
            .iter()
            .map(|observation| f64::from(observation.maximum_penetration))
            .fold(0.0, f64::max),
        maximum_separation: settled
            .iter()
            .map(|observation| f64::from(observation.maximum_separation))
            .fold(0.0, f64::max),
        final_active_normal_points: final_observation.active_normal_points,
        final_total_normal_impulse: final_observation.total_normal_impulse,
        final_dynamic_center_height_sum: dynamic_bodies(&final_observation.bodies)
            .map(|body| f64::from(body.position.y))
            .sum(),
        structure_pass,
        structure_reason,
        finite: solve.observations.iter().all(|observation| {
            observation.maximum_penetration.is_finite()
                && observation.maximum_separation.is_finite()
                && observation.total_normal_impulse.is_finite()
                && observation.bodies.iter().all(body_finite)
        }),
    })
}

fn failure_vector(
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

fn replay_hash(result: &PrecisionPathAuthoritySolveResultV1) -> Result<String, String> {
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

fn output_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: physbox-s15-margin --output PATH".to_owned()),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
