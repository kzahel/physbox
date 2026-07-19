use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{BodyStateV1, ScenarioV1, ShapeV1, Validate, canonical_hash, sha256_bytes};
use physbox_engine::{
    P5_2SettlingFixtureKind, p5_2_minimized_mixed_pile_energy_scenario,
    p5_2_s11_soft_composition_reproducer_scenario, p5_2_s11_soft_depth_sweep_scenario,
};
use physbox_lab::{
    scalar::{
        ScalarObservedSolveResultV1, ScalarStepObservationV1, ScalarSubstepImpulsePolicyV1,
        box2d_softness_coefficients,
        solve_portable_narrowphase_steps_observed_with_substep_impulses,
    },
    settling::{centered_stack_structure, settling_fixture_structure},
};
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
const CANDIDATE: &str = "retain-within-step-impulses-through-refresh";
const S11_2_MANIFEST_PATH: &str = "evidence/p5.2b-s11.2-soft-mechanism/manifest.json";
const S11_2_MANIFEST_SHA256: &str =
    "b0551bfe6cc4712c93b96c3cba292aa34508094da671538266fdc687e7301537";

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
    input: InputIdentity,
    candidate: CandidateContract,
    scalar_argument: ScalarArgument,
    cases: Vec<CaseEvidence>,
    scalar_gate_passed: bool,
    gpu_port_admitted: bool,
    candidate_disposition: &'static str,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InputIdentity {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    manifest_path: &'static str,
    manifest_sha256: &'static str,
    smallest_signature_depth: u32,
    permanent_reproducer_source_hash: String,
    depth_11_discarded_impulse_l1: f64,
    depth_16_discarded_impulse_l1: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateContract {
    id: &'static str,
    family: &'static str,
    control_policy: ScalarSubstepImpulsePolicyV1,
    candidate_policy: ScalarSubstepImpulsePolicyV1,
    changes: &'static str,
    preserves: &'static str,
    free_parameter_count: u32,
    product_policy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScalarArgument {
    stationarity_statement: &'static str,
    soft_complement_statement: &'static str,
    color_composition_statement: &'static str,
    analytic_samples: u32,
    maximum_soft_complement_error: f64,
    maximum_endpoint_impulse_error: f64,
    maximum_endpoint_velocity_error: f64,
    depth_11_selected_refresh_discontinuity_l1: f64,
    depth_16_selected_refresh_discontinuity_l1: f64,
    candidate_refresh_discontinuity_l1_at_fixed_point: f64,
    passed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseEvidence {
    label: &'static str,
    role: &'static str,
    depth: Option<u32>,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    capture: ArtifactIdentity,
    requested_steps: u32,
    control: QualitySummary,
    candidate: QualitySummary,
    exact_control_replay: bool,
    exact_candidate_replay: bool,
    penetration_preserved: bool,
    support_transfer_preserved: bool,
    quiet_by_sag_rejected: bool,
    two_step_response_exact: Option<bool>,
    frozen_awake_gate_applicable: bool,
    frozen_awake_gate_passed: Option<bool>,
    scalar_quality_passed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseCapture<'a> {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: &'a str,
    source_revision: &'a str,
    source_dirty: bool,
    label: &'static str,
    source_scenario_hash: &'a str,
    source_scenario: &'a ScenarioV1,
    control_observations: &'a [ScalarStepObservationV1],
    candidate_observations: &'a [ScalarStepObservationV1],
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
    positive_energy_run_max: u32,
    energy_envelope_nonincreasing: bool,
    maximum_penetration: f64,
    maximum_separation: f64,
    final_active_normal_points: u32,
    final_total_normal_impulse: f64,
    settled_total_normal_impulse_min: f64,
    settled_total_normal_impulse_max: f64,
    final_dynamic_center_height_sum: f64,
    structure_pass: bool,
    structure_reason: String,
    finite: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoftInput {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    smallest_primary_signature_depth: Option<u32>,
    permanent_reproducer_source_hash: Option<String>,
    permanent_reproducer_exact_alias: bool,
    depths: Vec<SoftDepthInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoftDepthInput {
    depth: u32,
    source_scenario_hash: String,
    primary_signature: bool,
    cross_step_handoff: SoftHandoffInput,
    composition: SoftCompositionInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoftHandoffInput {
    exact_slot_and_impulse_handoff: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoftCompositionInput {
    exact_first_prepared_reseeds: u32,
    discarded_prior_impulse_l1: f64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s11-retained-refresh-scalar error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err(
            "S11.3 retained-refresh scalar evidence requires a clean source tree".to_owned(),
        );
    }
    let input_bytes =
        fs::read(repository.join(S11_2_MANIFEST_PATH)).map_err(|error| error.to_string())?;
    if sha256_bytes(&input_bytes) != S11_2_MANIFEST_SHA256 {
        return Err("S11.2 input manifest digest changed".to_owned());
    }
    let input: SoftInput =
        serde_json::from_slice(&input_bytes).map_err(|error| error.to_string())?;
    let (depth_11, depth_16) = validate_input(&input)?;
    let depth_11_discarded_impulse_l1 = depth_11.composition.discarded_prior_impulse_l1;
    let depth_16_discarded_impulse_l1 = depth_16.composition.discarded_prior_impulse_l1;
    let staging = prepare_staging(&arguments.output, &["sources", "captures"])?;
    let scalar_argument =
        scalar_argument(depth_11_discarded_impulse_l1, depth_16_discarded_impulse_l1)?;

    let mut two_step =
        p5_2_minimized_mixed_pile_energy_scenario().map_err(|error| error.to_string())?;
    two_step.id = "p5.2-s11-retained-refresh-two-step-soft-contract".to_owned();
    two_step.requested_steps = 2;
    two_step.validate().map_err(|error| error.to_string())?;
    let permanent =
        p5_2_minimized_mixed_pile_energy_scenario().map_err(|error| error.to_string())?;
    let depth_11_source =
        p5_2_s11_soft_composition_reproducer_scenario().map_err(|error| error.to_string())?;
    let depth_16_source =
        p5_2_s11_soft_depth_sweep_scenario(16).map_err(|error| error.to_string())?;
    let cases = vec![
        run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            "two-step-soft-contract",
            "two-step soft response and penetration contract",
            None,
            &two_step,
            StructureKind::MixedPile,
            false,
        )?,
        run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            "permanent-four-body-soft",
            "permanent four-body soft recurrence context",
            None,
            &permanent,
            StructureKind::MixedPile,
            true,
        )?,
        run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            "s11.2-centered-eleven-soft",
            "S11.2 exact minimum soft composition reproducer",
            Some(11),
            &depth_11_source,
            StructureKind::CenteredStack,
            true,
        )?,
        run_case(
            &arguments,
            &source_revision,
            source_dirty,
            &staging,
            "governed-centered-sixteen-soft",
            "governed full-length soft 16-stack derivation gate",
            Some(16),
            &depth_16_source,
            StructureKind::CenteredStack,
            true,
        )?,
    ];
    let scalar_gate_passed =
        scalar_argument.passed && cases.iter().all(|case| case.scalar_quality_passed);
    let candidate_disposition = if scalar_gate_passed {
        "scalar-pass-gpu-port-admitted"
    } else {
        "discarded-at-scalar-stage"
    };
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s11.3-retained-refresh-soft-scalar",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-scalar",
        input: InputIdentity {
            artifact_kind: input.artifact_kind,
            run_id: input.run_id,
            source_revision: input.source_revision,
            manifest_path: S11_2_MANIFEST_PATH,
            manifest_sha256: S11_2_MANIFEST_SHA256,
            smallest_signature_depth: 11,
            permanent_reproducer_source_hash: input
                .permanent_reproducer_source_hash
                .ok_or_else(|| "S11.2 input omitted permanent source hash".to_owned())?,
            depth_11_discarded_impulse_l1,
            depth_16_discarded_impulse_l1,
        },
        candidate: CandidateContract {
            id: CANDIDATE,
            family: "soft-warm-start-normal-composition-correction",
            control_policy: ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
            candidate_policy: ScalarSubstepImpulsePolicyV1::RetainWithinStepImpulses,
            changes: "each full inter-substep geometry/effective-mass refresh retains the just-solved normal and tangent impulse vector instead of restoring the first prepared outer-step seed",
            preserves: "contact regeneration, refreshed anchors and effective masses, separation, unilateral projection, pinned soft coefficients, penetration bias, friction, restitution, deterministic colored order, and final persistence",
            free_parameter_count: 0,
            product_policy: "unchanged-selected-hard-direct",
        },
        scalar_argument,
        cases,
        scalar_gate_passed,
        gpu_port_admitted: scalar_gate_passed,
        candidate_disposition,
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    publish_manifest(&staging, &arguments.output, &manifest)?;
    let manifest_bytes =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native-scalar scenario=s11.3-retained-refresh run_id={} disposition={} manifest_sha256={} output={}",
        arguments.run_id,
        candidate_disposition,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn validate_input(input: &SoftInput) -> Result<(&SoftDepthInput, &SoftDepthInput), String> {
    if input.artifact_kind != "p5.2b-s11.2-soft-operator-depth-sweep"
        || input.source_dirty
        || input.smallest_primary_signature_depth != Some(11)
        || !input.permanent_reproducer_exact_alias
    {
        return Err("S11.2 input does not carry the frozen soft minimizer".to_owned());
    }
    let depth_11 = input
        .depths
        .iter()
        .find(|entry| entry.depth == 11)
        .ok_or_else(|| "S11.2 input omits depth 11".to_owned())?;
    let depth_16 = input
        .depths
        .iter()
        .find(|entry| entry.depth == 16)
        .ok_or_else(|| "S11.2 input omits depth 16".to_owned())?;
    if !depth_11.primary_signature
        || !depth_16.primary_signature
        || !depth_11.cross_step_handoff.exact_slot_and_impulse_handoff
        || !depth_16.cross_step_handoff.exact_slot_and_impulse_handoff
        || depth_11.composition.exact_first_prepared_reseeds != 3
        || depth_16.composition.exact_first_prepared_reseeds != 3
        || depth_11.composition.discarded_prior_impulse_l1 <= 0.0
        || depth_16.composition.discarded_prior_impulse_l1 <= 0.0
        || input.permanent_reproducer_source_hash.as_deref()
            != Some(depth_11.source_scenario_hash.as_str())
    {
        return Err("S11.2 input does not route retained refresh".to_owned());
    }
    Ok((depth_11, depth_16))
}

fn scalar_argument(
    depth_11_discarded: f64,
    depth_16_discarded: f64,
) -> Result<ScalarArgument, String> {
    let selected =
        box2d_softness_coefficients(30.0, 10.0, 1.0 / 240.0).map_err(|error| error.to_string())?;
    let coefficient_pairs = [
        (
            f64::from(selected.mass_scale),
            f64::from(selected.impulse_scale),
        ),
        (0.25, 0.75),
        (0.5, 0.5),
        (0.9, 0.1),
    ];
    let mut samples = 0_u32;
    let mut maximum_complement_error = 0.0_f64;
    let mut maximum_impulse_error = 0.0_f64;
    let mut maximum_velocity_error = 0.0_f64;
    for (mass_scale, impulse_scale) in coefficient_pairs {
        maximum_complement_error =
            maximum_complement_error.max((mass_scale + impulse_scale - 1.0).abs());
        for inverse_effective_mass in [0.25_f64, 1.0, 7.0] {
            for unconstrained_velocity in [-2.0_f64, -0.1, 0.0, 0.1, 2.0] {
                for velocity_bias in [-0.25_f64, 0.0, 0.25] {
                    let target = (-(mass_scale * unconstrained_velocity + velocity_bias)
                        / inverse_effective_mass)
                        .max(0.0);
                    for seed in [0.0_f64, 0.01, 0.5, 2.0] {
                        let warmed = unconstrained_velocity + inverse_effective_mass * seed;
                        let delta = -(mass_scale * warmed + velocity_bias) / inverse_effective_mass
                            - impulse_scale * seed;
                        let endpoint = (seed + delta).max(0.0);
                        let final_velocity = warmed + inverse_effective_mass * (endpoint - seed);
                        let target_velocity =
                            unconstrained_velocity + inverse_effective_mass * target;
                        maximum_impulse_error =
                            maximum_impulse_error.max((endpoint - target).abs());
                        maximum_velocity_error =
                            maximum_velocity_error.max((final_velocity - target_velocity).abs());
                        samples += 1;
                    }
                }
            }
        }
    }
    let f32_rounding_ceiling = 4.0 * f64::from(f32::EPSILON);
    let passed = maximum_complement_error <= f64::from(f32::EPSILON)
        && maximum_impulse_error <= f32_rounding_ceiling
        && maximum_velocity_error <= f32_rounding_ceiling
        && depth_11_discarded > 0.0
        && depth_16_discarded > depth_11_discarded;
    Ok(ScalarArgument {
        stationarity_statement: "at a settled fixed point, retaining the current impulse makes a full geometry/effective-mass refresh identity on impulse state; restoring a different first-prepared vector is a non-fixed perturbation",
        soft_complement_statement: "for one active unilateral soft constraint, warm application followed by the pinned update cancels seed dependence algebraically because massScale plus impulseScale is one; the serialized f32 coefficients leave only the separately measured sub-four-epsilon endpoint residual, and unilateral projection does not enlarge it",
        color_composition_statement: "body-disjoint constraints within a color are additive and colors are sequential, so deleting the measured refresh rewind preserves each projection while removing its perturbation before downstream colors",
        analytic_samples: samples,
        maximum_soft_complement_error: maximum_complement_error,
        maximum_endpoint_impulse_error: maximum_impulse_error,
        maximum_endpoint_velocity_error: maximum_velocity_error,
        depth_11_selected_refresh_discontinuity_l1: depth_11_discarded,
        depth_16_selected_refresh_discontinuity_l1: depth_16_discarded,
        candidate_refresh_discontinuity_l1_at_fixed_point: 0.0,
        passed,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    arguments: &Arguments,
    source_revision: &str,
    source_dirty: bool,
    staging: &Path,
    label: &'static str,
    role: &'static str,
    depth: Option<u32>,
    source: &ScenarioV1,
    structure_kind: StructureKind,
    frozen_awake_gate_applicable: bool,
) -> Result<CaseEvidence, String> {
    eprintln!("S11.3 retained-refresh scalar case {label}");
    let source_scenario_hash = canonical_hash(source).map_err(|error| error.to_string())?;
    let source_artifact = write_json_artifact(staging, &format!("sources/{label}.json"), source)?;
    let control = solve_portable_narrowphase_steps_observed_with_substep_impulses(
        source,
        ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
        false,
    )
    .map_err(|error| error.to_string())?;
    let candidate = solve_portable_narrowphase_steps_observed_with_substep_impulses(
        source,
        ScalarSubstepImpulsePolicyV1::RetainWithinStepImpulses,
        false,
    )
    .map_err(|error| error.to_string())?;
    let replay_control = solve_portable_narrowphase_steps_observed_with_substep_impulses(
        source,
        ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
        false,
    )
    .map_err(|error| error.to_string())?;
    let replay_candidate = solve_portable_narrowphase_steps_observed_with_substep_impulses(
        source,
        ScalarSubstepImpulsePolicyV1::RetainWithinStepImpulses,
        false,
    )
    .map_err(|error| error.to_string())?;
    let exact_control_replay = control == replay_control;
    let exact_candidate_replay = candidate == replay_candidate;
    if !exact_control_replay || !exact_candidate_replay {
        return Err(format!("S11.3 scalar replay changed for {label}"));
    }
    let control_summary = quality(source, &control, structure_kind)?;
    let candidate_summary = quality(source, &candidate, structure_kind)?;
    let penetration_preserved =
        candidate_summary.maximum_penetration <= control_summary.maximum_penetration;
    let support_transfer_preserved = candidate_summary.final_active_normal_points
        == control_summary.final_active_normal_points
        && (control_summary.final_total_normal_impulse == 0.0
            || candidate_summary.final_total_normal_impulse > 0.0);
    let maximum_height_loss = control_summary
        .maximum_penetration
        .max(candidate_summary.maximum_penetration);
    let quiet_by_sag_rejected = candidate_summary.structure_pass
        && penetration_preserved
        && support_transfer_preserved
        && candidate_summary.final_dynamic_center_height_sum + maximum_height_loss
            >= control_summary.final_dynamic_center_height_sum;
    let two_step_response_exact =
        (source.requested_steps == 2).then(|| candidate.result.bodies == control.result.bodies);
    let frozen_awake_gate_passed = frozen_awake_gate_applicable.then_some({
        candidate_summary.settled_linear_speed_max <= 0.025
            && candidate_summary.settled_angular_edge_speed_max <= 0.025
            && candidate_summary.positive_energy_run_max < 5
            && candidate_summary.energy_envelope_nonincreasing
    });
    let scalar_quality_passed = candidate_summary.finite
        && penetration_preserved
        && support_transfer_preserved
        && quiet_by_sag_rejected
        && two_step_response_exact.unwrap_or(true)
        && frozen_awake_gate_passed.unwrap_or(true);
    let capture = CaseCapture {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s11.3-retained-refresh-soft-scalar-capture",
        run_id: &arguments.run_id,
        source_revision,
        source_dirty,
        label,
        source_scenario_hash: &source_scenario_hash,
        source_scenario: source,
        control_observations: &control.observations,
        candidate_observations: &candidate.observations,
    };
    let capture_artifact =
        write_gzip_json_artifact(staging, &format!("captures/{label}.json.gz"), &capture)?;
    Ok(CaseEvidence {
        label,
        role,
        depth,
        source_scenario_hash,
        source: source_artifact,
        capture: capture_artifact,
        requested_steps: source.requested_steps,
        control: control_summary,
        candidate: candidate_summary,
        exact_control_replay,
        exact_candidate_replay,
        penetration_preserved,
        support_transfer_preserved,
        quiet_by_sag_rejected,
        two_step_response_exact,
        frozen_awake_gate_applicable,
        frozen_awake_gate_passed,
        scalar_quality_passed,
    })
}

#[derive(Clone, Copy)]
enum StructureKind {
    MixedPile,
    CenteredStack,
}

fn quality(
    source: &ScenarioV1,
    solve: &ScalarObservedSolveResultV1,
    structure_kind: StructureKind,
) -> Result<QualitySummary, String> {
    let observations = &solve.observations;
    let settled_start_step = if source.requested_steps >= 1_200 {
        900
    } else {
        1
    };
    let settled = observations
        .iter()
        .filter(|value| value.completed_steps >= settled_start_step)
        .collect::<Vec<_>>();
    if settled.is_empty() {
        return Err("S11.3 scalar quality has no settled observations".to_owned());
    }
    let energy = settled
        .iter()
        .map(|value| kinetic_energy(&value.bodies))
        .collect::<Vec<_>>();
    let quartile = (settled.len() / 4).max(1);
    let first_energy_quartile_max = energy[..quartile].iter().copied().fold(0.0, f64::max);
    let last_energy_quartile_max = energy[energy.len() - quartile..]
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let mut positive_energy_run = 0_u32;
    let mut positive_energy_run_max = 0_u32;
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
    let final_observation = observations
        .last()
        .ok_or_else(|| "S11.3 scalar quality has no final observation".to_owned())?;
    let (structure_pass, structure_reason) = match structure_kind {
        StructureKind::MixedPile => settling_fixture_structure(
            P5_2SettlingFixtureKind::SmallMixedPile,
            source,
            &final_observation.bodies,
            false,
            0.0,
        ),
        StructureKind::CenteredStack => {
            centered_stack_structure(source, &final_observation.bodies, false, 0.0)
        }
    };
    Ok(QualitySummary {
        settled_start_step,
        settled_end_step: source.requested_steps,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|value| dynamic_bodies(&value.bodies))
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
            .flat_map(|value| dynamic_bodies(&value.bodies))
            .map(|body| f64::from(body.angular_velocity.get().abs() * support_radius(body)))
            .fold(0.0, f64::max),
        first_energy_quartile_max,
        last_energy_quartile_max,
        positive_energy_run_max,
        energy_envelope_nonincreasing,
        maximum_penetration: observations
            .iter()
            .map(|value| f64::from(value.maximum_penetration))
            .fold(0.0, f64::max),
        maximum_separation: observations
            .iter()
            .map(|value| f64::from(value.maximum_separation))
            .fold(0.0, f64::max),
        final_active_normal_points: final_observation.active_normal_points,
        final_total_normal_impulse: final_observation.total_normal_impulse,
        settled_total_normal_impulse_min: settled
            .iter()
            .map(|value| value.total_normal_impulse)
            .fold(f64::INFINITY, f64::min),
        settled_total_normal_impulse_max: settled
            .iter()
            .map(|value| value.total_normal_impulse)
            .fold(0.0, f64::max),
        final_dynamic_center_height_sum: dynamic_bodies(&final_observation.bodies)
            .map(|body| f64::from(body.position.y))
            .sum(),
        structure_pass,
        structure_reason,
        finite: observations.iter().all(observation_finite),
    })
}

fn observation_finite(observation: &ScalarStepObservationV1) -> bool {
    observation.maximum_penetration.is_finite()
        && observation.maximum_separation.is_finite()
        && observation.total_normal_impulse.is_finite()
        && observation.bodies.iter().all(|body| {
            body.position.x.is_finite()
                && body.position.y.is_finite()
                && body.angle.get().is_finite()
                && body.linear_velocity.x.is_finite()
                && body.linear_velocity.y.is_finite()
                && body.angular_velocity.get().is_finite()
        })
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

fn prepare_staging(output: &Path, directories: &[&str]) -> Result<PathBuf, String> {
    let mut staging_name = output.as_os_str().to_owned();
    staging_name.push(".partial");
    let staging = PathBuf::from(staging_name);
    if output.exists() || staging.exists() {
        return Err(format!(
            "refusing to replace S11.3 scalar output or staging {}",
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
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("artifact")
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = git(repository, &["rev-parse", "HEAD"])?;
    let status = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(repository)
        .output()
        .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut run_id = None;
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
    fn soft_retained_refresh_argument_and_candidate_are_frozen() {
        let argument = scalar_argument(1.0e-6, 1.0e-4).unwrap();
        assert_eq!(CANDIDATE, "retain-within-step-impulses-through-refresh");
        assert_eq!(argument.analytic_samples, 720);
        assert!(
            argument.passed,
            "complement={} impulse={} velocity={}",
            argument.maximum_soft_complement_error,
            argument.maximum_endpoint_impulse_error,
            argument.maximum_endpoint_velocity_error
        );
    }
}
