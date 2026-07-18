use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use physbox_contracts::{BodyStateV1, ScenarioV1, ShapeV1, Validate, canonical_hash, sha256_bytes};
use physbox_engine::{
    P5_2SettlingFixtureKind, p5_2_minimized_mixed_pile_hard_scenario,
    p5_2_stale_reseed_reproducer_scenario,
};
use physbox_lab::{
    scalar::{
        ScalarObservedSolveResultV1, ScalarStepObservationV1, ScalarSubstepImpulsePolicyV1,
        solve_portable_narrowphase_steps_observed_with_substep_impulses,
    },
    settling::{centered_stack_structure, settling_fixture_structure},
};
use serde::Serialize;

const SCHEMA_VERSION: u32 = 1;
const CANDIDATE: &str = "retain-within-step-impulses-through-refresh";

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
    cases: Vec<CaseResult>,
    scalar_gate_passed: bool,
    second_candidate_slot: SecondCandidateSlot,
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
    manifest_path: String,
    manifest_sha256: String,
    smallest_harmful_depth: u32,
    permanent_reproducer_source_hash: String,
    discarded_impulse_l1_at_minimum: f64,
    active_set_flips_at_minimum: u32,
    redistribution_at_minimum: f64,
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
    single_constraint_statement: &'static str,
    color_composition_statement: &'static str,
    analytic_samples: u32,
    maximum_endpoint_impulse_error: f64,
    maximum_endpoint_velocity_error: f64,
    minimum_seed_deviation_energy: f64,
    selected_refresh_discontinuity_l1: f64,
    candidate_refresh_discontinuity_l1_at_fixed_point: f64,
    passed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseResult {
    label: &'static str,
    role: &'static str,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    requested_steps: u32,
    control: QualitySummary,
    candidate: QualitySummary,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecondCandidateSlot {
    disposition: &'static str,
    rationale: &'static str,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardInput {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    smallest_harmful_depth: u32,
    permanent_reproducer_source_hash: String,
    depths: Vec<HardDepthInput>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardDepthInput {
    depth: u32,
    mechanism: HardMechanismInput,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardMechanismInput {
    discarded_prior_impulse_l1: f64,
    intra_block_active_set_flips: u32,
    intra_manifold_redistribution_max: f64,
    named_mechanism: String,
    stale_reseed_signature: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s10-retained-refresh-scalar: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_revision = git(&repository, &["rev-parse", "HEAD"])?;
    let status = git_bytes(
        &repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let source_dirty = !status.is_empty();
    if arguments.require_clean && source_dirty {
        return Err("S10.4 scalar evidence requires a clean working tree".to_owned());
    }
    let source_patch_digest = source_dirty.then(|| sha256_bytes(&status));
    let hard_bytes = fs::read(&arguments.s10_3_hard).map_err(|error| error.to_string())?;
    let hard: HardInput = serde_json::from_slice(&hard_bytes).map_err(|error| error.to_string())?;
    if hard.artifact_kind != "p5.2b-s10.3-hard-operator-and-depth"
        || hard.source_dirty
        || hard.smallest_harmful_depth != 4
    {
        return Err("S10.3 hard input does not carry the frozen minimum".to_owned());
    }
    let minimum = hard
        .depths
        .iter()
        .find(|entry| entry.depth == 4)
        .ok_or_else(|| "S10.3 hard input omits depth 4".to_owned())?;
    if minimum.mechanism.named_mechanism != "inter-substep-stale-persistence-reseed"
        || !minimum.mechanism.stale_reseed_signature
        || minimum.mechanism.intra_block_active_set_flips != 0
        || minimum.mechanism.intra_manifold_redistribution_max >= 1.0e-6
    {
        return Err("S10.3 minimum does not route the retained-refresh candidate".to_owned());
    }

    let staging = prepare_staging(&arguments.output)?;
    fs::create_dir(staging.join("sources")).map_err(|error| error.to_string())?;
    let scalar_argument = scalar_argument(minimum.mechanism.discarded_prior_impulse_l1)?;
    let mut two_step =
        p5_2_minimized_mixed_pile_hard_scenario().map_err(|error| error.to_string())?;
    two_step.id = "p5.2-s10-retained-refresh-two-step-contract".to_owned();
    two_step.requested_steps = 2;
    two_step.validate().map_err(|error| error.to_string())?;
    let permanent = p5_2_minimized_mixed_pile_hard_scenario().map_err(|error| error.to_string())?;
    let minimum_source =
        p5_2_stale_reseed_reproducer_scenario().map_err(|error| error.to_string())?;
    let cases = vec![
        run_case(
            &staging,
            "two-step-contract",
            "two-step-contract",
            "frozen two-step response and penetration contract",
            &two_step,
            StructureKind::MixedPile,
            false,
        )?,
        run_case(
            &staging,
            "permanent-four-body",
            "permanent-four-body",
            "permanent mixed-pile hard quality reproducer",
            &permanent,
            StructureKind::MixedPile,
            true,
        )?,
        run_case(
            &staging,
            "s10.3-centered-four",
            "s10.3-centered-four",
            "S10.3 minimum stale-reseed reproducer",
            &minimum_source,
            StructureKind::CenteredStack,
            true,
        )?,
    ];
    let scalar_gate_passed =
        scalar_argument.passed && cases.iter().all(|case| case.scalar_quality_passed);
    if !scalar_gate_passed {
        return Err("retained-refresh candidate failed the scalar gate".to_owned());
    }
    let input_path = arguments
        .s10_3_hard
        .strip_prefix(&repository)
        .unwrap_or(&arguments.s10_3_hard)
        .to_string_lossy()
        .to_string();
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s10.4-retained-refresh-scalar",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-scalar",
        input: InputIdentity {
            artifact_kind: hard.artifact_kind,
            run_id: hard.run_id,
            source_revision: hard.source_revision,
            manifest_path: input_path,
            manifest_sha256: sha256_bytes(&hard_bytes),
            smallest_harmful_depth: hard.smallest_harmful_depth,
            permanent_reproducer_source_hash: hard.permanent_reproducer_source_hash,
            discarded_impulse_l1_at_minimum: minimum.mechanism.discarded_prior_impulse_l1,
            active_set_flips_at_minimum: minimum.mechanism.intra_block_active_set_flips,
            redistribution_at_minimum: minimum.mechanism.intra_manifold_redistribution_max,
        },
        candidate: CandidateContract {
            id: CANDIDATE,
            family: "warm-start-bias-composition-correction",
            control_policy: ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
            candidate_policy: ScalarSubstepImpulsePolicyV1::RetainWithinStepImpulses,
            changes: "each full substep geometry/effective-mass refresh retains the just-solved normal and tangent impulse vector instead of reloading the older outer-step persistence seed",
            preserves: "contact regeneration, refreshed anchors, refreshed effective masses, separation, unilateral projection, penetration bias, friction, restitution, deterministic order, and final outer-step persistence",
            free_parameter_count: 0,
            product_policy: "unchanged-selected-hard-direct",
        },
        scalar_argument,
        cases,
        scalar_gate_passed,
        second_candidate_slot: SecondCandidateSlot {
            disposition: "not-routed",
            rationale: "the S10.3 four-body minimum has zero active-set flips and sub-micro redistribution, so a separate active-set or tie rule has no root-mechanism support",
        },
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(staging.join("manifest.json"), &manifest_bytes).map_err(|error| error.to_string())?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s10.4-retained-refresh-scalar run_id={} manifest_sha256={} output={}",
        arguments.run_id,
        sha256_bytes(&manifest_bytes),
        arguments.output.display()
    );
    Ok(())
}

fn scalar_argument(selected_discontinuity: f64) -> Result<ScalarArgument, String> {
    let mut samples = 0_u32;
    let mut maximum_impulse_error = 0.0_f64;
    let mut maximum_velocity_error = 0.0_f64;
    let mut minimum_deviation_energy = f64::INFINITY;
    for inverse_effective_mass in [0.25_f64, 1.0, 7.0] {
        for unconstrained_velocity in [-2.0_f64, -0.1, 0.0, 0.1, 2.0] {
            for bias in [-0.25_f64, 0.0, 0.25] {
                let target = (-(unconstrained_velocity + bias) / inverse_effective_mass).max(0.0);
                for seed in [0.0_f64, 0.01, 0.5, 2.0] {
                    let warmed = unconstrained_velocity + inverse_effective_mass * seed;
                    let endpoint = (seed - (warmed + bias) / inverse_effective_mass).max(0.0);
                    let final_velocity = warmed + inverse_effective_mass * (endpoint - seed);
                    let target_velocity = unconstrained_velocity + inverse_effective_mass * target;
                    maximum_impulse_error = maximum_impulse_error.max((endpoint - target).abs());
                    maximum_velocity_error =
                        maximum_velocity_error.max((final_velocity - target_velocity).abs());
                    let seed_error = seed - target;
                    minimum_deviation_energy = minimum_deviation_energy
                        .min(0.5 * inverse_effective_mass * seed_error * seed_error);
                    samples += 1;
                }
            }
        }
    }
    let passed = maximum_impulse_error <= 1.0e-12
        && maximum_velocity_error <= 1.0e-12
        && minimum_deviation_energy >= 0.0
        && selected_discontinuity > 0.0;
    Ok(ScalarArgument {
        stationarity_statement: "at a settled fixed point, retaining the current impulse vector makes a full geometry/effective-mass refresh identity on impulse state; reloading a different outer-step vector cannot be a fixed map",
        single_constraint_statement: "for an active hard unilateral scalar constraint with exact effective mass, the projected endpoint is independent of the warm seed; retaining the latest seed therefore preserves the endpoint and only removes a coupled-color transient",
        color_composition_statement: "body-disjoint constraints within one color are additive; colors are sequential, so deleting the measured refresh discontinuity cannot alter unilateral projection or bias and removes the stale perturbation before downstream colors",
        analytic_samples: samples,
        maximum_endpoint_impulse_error: maximum_impulse_error,
        maximum_endpoint_velocity_error: maximum_velocity_error,
        minimum_seed_deviation_energy: minimum_deviation_energy,
        selected_refresh_discontinuity_l1: selected_discontinuity,
        candidate_refresh_discontinuity_l1_at_fixed_point: 0.0,
        passed,
    })
}

fn run_case(
    staging: &Path,
    source_name: &'static str,
    label: &'static str,
    role: &'static str,
    source: &ScenarioV1,
    structure_kind: StructureKind,
    frozen_awake_gate_applicable: bool,
) -> Result<CaseResult, String> {
    let source_scenario_hash = canonical_hash(source).map_err(|error| error.to_string())?;
    let source_path = format!("sources/{source_name}.json");
    let source_bytes = serde_json::to_vec_pretty(source).map_err(|error| error.to_string())?;
    fs::write(staging.join(&source_path), &source_bytes).map_err(|error| error.to_string())?;
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
    let control_summary = quality(source, &control, structure_kind)?;
    let candidate_summary = quality(source, &candidate, structure_kind)?;
    let penetration_preserved =
        candidate_summary.maximum_penetration <= control_summary.maximum_penetration;
    let support_transfer_preserved = candidate_summary.final_active_normal_points
        == control_summary.final_active_normal_points
        && (control_summary.final_total_normal_impulse == 0.0
            || candidate_summary.final_total_normal_impulse > 0.0);
    let quiet_by_sag_rejected =
        candidate_summary.structure_pass && penetration_preserved && support_transfer_preserved;
    let two_step_response_exact =
        (source.requested_steps == 2).then(|| candidate.result.bodies == control.result.bodies);
    let frozen_awake_gate_passed = frozen_awake_gate_applicable.then_some({
        candidate_summary.settled_linear_speed_max <= 0.025
            && candidate_summary.settled_angular_edge_speed_max <= 0.025
            && candidate_summary.energy_envelope_nonincreasing
    });
    let scalar_quality_passed = candidate_summary.finite
        && penetration_preserved
        && support_transfer_preserved
        && quiet_by_sag_rejected
        && two_step_response_exact.unwrap_or(true)
        && frozen_awake_gate_passed.unwrap_or(true);
    Ok(CaseResult {
        label,
        role,
        source_scenario_hash,
        source: ArtifactIdentity {
            path: source_path,
            sha256: sha256_bytes(&source_bytes),
        },
        requested_steps: source.requested_steps,
        control: control_summary,
        candidate: candidate_summary,
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
        return Err("scalar quality has no settled observations".to_owned());
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
        .ok_or_else(|| "scalar quality has no final observation".to_owned())?;
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
            let linear = if body.inverse_mass.get() > 0.0 {
                0.5 * f64::from(
                    body.linear_velocity.x * body.linear_velocity.x
                        + body.linear_velocity.y * body.linear_velocity.y,
                ) / f64::from(body.inverse_mass.get())
            } else {
                0.0
            };
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

fn prepare_staging(output: &Path) -> Result<PathBuf, String> {
    if output.exists() {
        return Err(format!("fresh output already exists: {}", output.display()));
    }
    let file_name = output
        .file_name()
        .ok_or_else(|| "output has no final component".to_owned())?;
    let staging = output.with_file_name(format!("{}.partial", file_name.to_string_lossy()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    Ok(staging)
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

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    Ok(output.stdout)
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    s10_3_hard: PathBuf,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut s10_3_hard = None;
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
                "--s10-3-hard" => s10_3_hard = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            s10_3_hard: s10_3_hard.ok_or_else(|| "missing --s10-3-hard".to_owned())?,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_refresh_candidate_and_scalar_sample_count_are_frozen() {
        let argument = scalar_argument(0.25).unwrap();
        assert_eq!(CANDIDATE, "retain-within-step-impulses-through-refresh");
        assert_eq!(argument.analytic_samples, 180);
        assert!(argument.passed);
    }
}
