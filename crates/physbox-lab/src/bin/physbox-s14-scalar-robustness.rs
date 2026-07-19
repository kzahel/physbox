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
use physbox_engine::P5_2SettlingFixtureKind;
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePolicyV1, BranchRobustAuthoritySolveResultV1,
        solve_branch_robust_authority_structure_steps_observed,
    },
    branch_robust::{
        BRANCH_ROBUST_CANDIDATE_ID, BRANCH_ROBUST_ERROR_BOUND_VERSION,
        BRANCH_ROBUST_FORMULA_VERSION,
    },
    branch_robust_scalar::{BranchRobustPerturbationV1, BranchRobustRunStatisticsV1},
    scalar::{ScalarObservedSolveResultV1, ScalarStepObservationV1},
    settling::{centered_stack_structure, settling_fixture_structure},
};
use serde::{Deserialize, Serialize};

const S12_MANIFEST: &str = "evidence/p5.2b-s12.2-authority-endpoints/manifest.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SourceRole {
    ResponseDiagnostic,
    Context,
    Governed,
}

#[derive(Clone, Copy)]
enum StructureKind {
    MixedPile,
    CenteredStack,
}

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

#[derive(Clone, Deserialize, Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    version: u32,
    candidate_id: &'static str,
    formula_version: u32,
    error_bound_version: u32,
    formula_arms: u32,
    policy: AuthorityStructurePolicyV1,
    scalar_matrix: ScalarMatrix,
    authority_source: AuthoritySource,
    authority_bands: AuthorityBands,
    cases: Vec<CaseOutput>,
    governed_rows: usize,
    governed_rows_qualified: usize,
    every_governed_row_qualified: bool,
    gpu_port_admitted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScalarMatrix {
    perturbation_lanes: Vec<&'static str>,
    governed_depths: [u32; 2],
    governed_steps: u32,
    settled_window_inclusive: [u32; 2],
    zero_based_step_for_perturbation: bool,
    lower_on_even_alternation: bool,
    hash_sign_expression: &'static str,
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
    perturbation: BranchRobustPerturbationV1,
    perturbation_id: &'static str,
    primary_sha256: String,
    replay_sha256: String,
    exact_replay: bool,
    statistics: BranchRobustRunStatisticsV1,
    replay_statistics_exact: bool,
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
    bound_violation: bool,
    replay: bool,
    ordered_failures: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScalarCapture<'a> {
    version: u32,
    candidate_id: &'static str,
    source_label: &'a str,
    source_scenario_hash: &'a str,
    perturbation: BranchRobustPerturbationV1,
    perturbation_id: &'static str,
    policy: AuthorityStructurePolicyV1,
    statistics: BranchRobustRunStatisticsV1,
    observations: &'a [ScalarStepObservationV1],
    final_bodies: &'a [BodyStateV1],
    final_cache: &'a [ContactCachePointV1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayProjection<'a> {
    statistics: BranchRobustRunStatisticsV1,
    observations: &'a [ScalarStepObservationV1],
    final_bodies: &'a [BodyStateV1],
    final_cache: &'a [ContactCachePointV1],
}

fn main() -> Result<(), String> {
    let output_path = output_path()?;
    let repository = Path::new(".");
    let s12_bytes = fs::read(repository.join(S12_MANIFEST)).map_err(display)?;
    let s12_sha256 = sha256_bytes(&s12_bytes);
    let s12: S12Manifest = serde_json::from_slice(&s12_bytes).map_err(display)?;
    if s12.artifact_kind != "p5.2b-s12.2-authority-endpoints"
        || s12.source_dirty
        || s12.box2d.dirty
        || s12.box2d.patch_digest.is_some()
        || s12.cases.len() != 4
        || !s12.cases.iter().all(|case| {
            case.authority.exact_replay
                && case.authority.capture_exact_replay
                && case.authority.clean_public_trace_provenance
        })
    {
        return Err("frozen S12 authority source is not admissible".to_owned());
    }
    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let capture_directory = output_path
        .parent()
        .ok_or_else(|| "output requires a parent directory".to_owned())?
        .join("captures");
    fs::create_dir_all(&capture_directory).map_err(display)?;
    let mut cases = Vec::new();
    let mut governed_rows = 0;
    let mut governed_rows_qualified = 0;

    for frozen in &s12.cases {
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
        {
            return Err(format!("source contract drift for {}", frozen.label));
        }
        let structure = if frozen.label.contains("centered") {
            StructureKind::CenteredStack
        } else {
            StructureKind::MixedPile
        };
        let mut rows = Vec::new();
        for lane in BranchRobustPerturbationV1::ALL {
            let primary = solve_branch_robust_authority_structure_steps_observed(
                &source, policy, lane, false,
            )
            .map_err(display)?;
            let replay = solve_branch_robust_authority_structure_steps_observed(
                &source, policy, lane, false,
            )
            .map_err(display)?;
            let primary_sha256 = replay_hash(&primary)?;
            let replay_sha256 = replay_hash(&replay)?;
            let exact_replay = primary == replay && primary_sha256 == replay_sha256;
            let quality = scalar_quality(&source, &primary.solve, structure)?;
            let active_support_pairs = scalar_active_support_pairs(&primary.solve);
            let mut failure_vector = failure_vector(
                &source,
                frozen.role,
                &quality,
                &active_support_pairs,
                &frozen.authority.quality,
                &frozen.authority.active_support_pairs,
                &s12.authority_bands,
            );
            failure_vector.bound_violation = primary.statistics.bound_violations != 0
                || primary.statistics.non_finite_results != 0
                || primary.statistics.normal_range_violations != 0;
            failure_vector.replay = !exact_replay || primary.statistics != replay.statistics;
            if failure_vector.bound_violation {
                failure_vector.ordered_failures.push("bound-violation");
            }
            if failure_vector.replay {
                failure_vector.ordered_failures.push("replay");
            }
            let authority_qualified = failure_vector.ordered_failures.is_empty();
            let capture_name = format!("{}-{}.json.gz", frozen.label, lane.id());
            let capture_path = capture_directory.join(&capture_name);
            let capture_bytes = capture_bytes(ScalarCapture {
                version: 1,
                candidate_id: BRANCH_ROBUST_CANDIDATE_ID,
                source_label: &frozen.label,
                source_scenario_hash: &frozen.source_scenario_hash,
                perturbation: lane,
                perturbation_id: lane.id(),
                policy,
                statistics: primary.statistics,
                observations: &primary.solve.observations,
                final_bodies: &primary.solve.result.bodies,
                final_cache: &primary.solve.result.contact_cache,
            })?;
            fs::write(&capture_path, &capture_bytes).map_err(display)?;
            if frozen.role == SourceRole::Governed {
                governed_rows += 1;
                governed_rows_qualified += usize::from(authority_qualified);
            }
            rows.push(RowOutput {
                perturbation: lane,
                perturbation_id: lane.id(),
                primary_sha256,
                replay_sha256,
                exact_replay,
                statistics: primary.statistics,
                replay_statistics_exact: primary.statistics == replay.statistics,
                delta_from_authority: authority_delta(&quality, &frozen.authority.quality),
                quality,
                active_support_pairs,
                failure_vector,
                authority_qualified,
                capture: ArtifactIdentity {
                    path: format!("captures/{capture_name}"),
                    sha256: sha256_bytes(&capture_bytes),
                },
            });
        }
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
    let every_governed_row_qualified = governed_rows == 14 && governed_rows_qualified == 14;
    let output = Output {
        version: 1,
        candidate_id: BRANCH_ROBUST_CANDIDATE_ID,
        formula_version: BRANCH_ROBUST_FORMULA_VERSION,
        error_bound_version: BRANCH_ROBUST_ERROR_BOUND_VERSION,
        formula_arms: 1,
        policy,
        scalar_matrix: ScalarMatrix {
            perturbation_lanes: BranchRobustPerturbationV1::ALL
                .map(|lane| lane.id())
                .to_vec(),
            governed_depths: [11, 16],
            governed_steps: 1_200,
            settled_window_inclusive: [900, 1_200],
            zero_based_step_for_perturbation: true,
            lower_on_even_alternation: true,
            hash_sign_expression: "step*0x9e3779b9 ^ substep*0x85ebca6b ^ manifold*0xc2b2ae35 ^ point*0x27d4eb2f",
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
        gpu_port_admitted: every_governed_row_qualified,
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
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s14.4-scalar-robustness governed={governed_rows_qualified}/{governed_rows} gpu_port_admitted={} output={}",
        output.gpu_port_admitted,
        output_path.display()
    );
    Ok(())
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
        return Err("S14 quality has no settled observations".to_owned());
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
        .ok_or_else(|| "S14 quality has no final observation".to_owned())?;
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

#[allow(clippy::too_many_arguments)]
fn failure_vector(
    source: &ScenarioV1,
    role: SourceRole,
    candidate: &QualitySummary,
    candidate_pairs: &[BodyPair],
    authority: &QualitySummary,
    authority_pairs: &[BodyPair],
    bands: &AuthorityBands,
) -> FailureVector {
    let full_length = source.requested_steps >= 1_200;
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
    let awake_speed = full_length
        && (candidate.settled_linear_speed_max > bands.awake_product_ceiling
            || candidate.settled_angular_edge_speed_max > bands.awake_product_ceiling);
    let positive_run = full_length
        && candidate.positive_energy_run_max
            > authority
                .positive_energy_run_max
                .max(bands.positive_energy_run_floor);
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
        bound_violation: false,
        replay: false,
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
            .map(cache_pair),
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

fn replay_hash(result: &BranchRobustAuthoritySolveResultV1) -> Result<String, String> {
    let projection = ReplayProjection {
        statistics: result.statistics,
        observations: &result.solve.observations,
        final_bodies: &result.solve.result.bodies,
        final_cache: &result.solve.result.contact_cache,
    };
    serde_json::to_vec(&projection)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display)
}

fn capture_bytes(capture: ScalarCapture<'_>) -> Result<Vec<u8>, String> {
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
        _ => Err("usage: physbox-s14-scalar-robustness --output PATH".to_owned()),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
