use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    BodyId, BodyStateV1, PersistentContactV1, SCHEMA_VERSION, ScenarioV1, ShapeV1, Validate,
    canonical_hash, sha256_bytes,
};
use physbox_engine::{
    AuthorityGpuProbePolicyV1, GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1,
    GpuSolverPassKindV1, SelectedGpuSolverPolicySnapshotV1,
    authority_gpu_probe_capacity_disposition, benchmark_gpu_s13_authority_probe,
    p5_2_s13_authority_measurement_scenario, p5_2_s13_authority_overflow_scenario,
    run_gpu_authored_authority_probe,
};
use physbox_lab::{
    authority_structure::{
        AuthorityStructureAxisV1, AuthorityStructurePassKindV1, AuthorityStructurePolicyV1,
        solve_authority_structure_one_step_pass_states, solve_authority_structure_steps_observed,
    },
    settling::centered_stack_structure,
};
use serde::{Deserialize, Serialize};

const SOURCE_ROOT: &str = "evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/sources";
const AUTHORITY_MANIFEST: &str = "evidence/p5.2b-s12.2-authority-endpoints/manifest.json";
const IMPLEMENTATION_BAND: f64 = 5.0e-5;
const SPEED_MULTIPLIER: f64 = 2.0;
const SPEED_ADDITIVE: f64 = 2.0e-5;
const HEIGHT_ABSOLUTE: f64 = 2.0e-4;

const GOVERNED: [GovernedSource; 2] = [
    GovernedSource {
        label: "s11.2-centered-eleven-soft",
        filename: "s11.2-centered-eleven-soft.json",
        depth: 11,
    },
    GovernedSource {
        label: "governed-centered-sixteen-soft",
        filename: "governed-centered-sixteen-soft.json",
        depth: 16,
    },
];

#[derive(Clone, Copy)]
struct GovernedSource {
    label: &'static str,
    filename: &'static str,
    depth: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    candidate: AuthorityGpuProbePolicyV1,
    candidate_hash: String,
    selected_snapshot: SelectedGpuSolverPolicySnapshotV1,
    selected_snapshot_hash: String,
    shader_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImplementationManifest {
    #[serde(flatten)]
    provenance: Provenance,
    source_scenario_hash: String,
    adapter: physbox_engine::AdapterDiagnostics,
    exact_replay: bool,
    primary_sha256: String,
    replay_sha256: String,
    two_step_scalar_maximum: StateDelta,
    implementation_band: f64,
    implementation_agreement: bool,
    first_phase_outside_band: Option<PhaseDelta>,
    selected_product_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateDelta {
    body_index: usize,
    field: &'static str,
    absolute_difference: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseDelta {
    substep: u32,
    gpu_pass: GpuSolverPassKindV1,
    scalar_pass: AuthorityStructurePassKindV1,
    maximum: StateDelta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OverflowManifest {
    #[serde(flatten)]
    provenance: Provenance,
    source_scenario_hash: String,
    source: Artifact,
    gpu_result: physbox_engine::GpuAuthoredAuthorityProbeOutputV1,
    capacity_primary: physbox_engine::GpuAuthorityProbeCapacityDispositionV1,
    capacity_replay: physbox_engine::GpuAuthorityProbeCapacityDispositionV1,
    assignments_match: bool,
    overflow_first: bool,
    exact_replay: bool,
    capacity_exact_replay: bool,
    fail_closed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GovernedManifest {
    #[serde(flatten)]
    provenance: Provenance,
    authority_input: Artifact,
    cases: Vec<GovernedCase>,
    all_cases_qualified: bool,
    exact_candidate_rejected_by_frozen_authority_quality: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GovernedCase {
    label: &'static str,
    depth: u32,
    source_scenario_hash: String,
    source: Artifact,
    primary: Artifact,
    replay: Artifact,
    pass_trace_primary: Artifact,
    pass_trace_replay: Artifact,
    exact_replay: bool,
    pass_trace_exact_replay: bool,
    candidate_quality: Quality,
    authority_quality: Quality,
    candidate_active_support_pairs: Vec<BodyPair>,
    authority_active_support_pairs: Vec<BodyPair>,
    gates: QualityGates,
    qualified: bool,
    awake_product_ceiling_pass: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Quality {
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
struct QualityGates {
    source_identity: bool,
    finite_complete: bool,
    structure: bool,
    support_topology: bool,
    linear_speed: bool,
    angular_edge_speed: bool,
    height_sum: bool,
    penetration: bool,
    energy_ratio: bool,
    positive_energy_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyPair {
    body_a: BodyId,
    body_b: BodyId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityManifest {
    cases: Vec<AuthorityCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityCase {
    label: String,
    source_scenario_hash: String,
    authority: AuthorityEvidence,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorityEvidence {
    quality: Quality,
    active_support_pairs: Vec<BodyPair>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkManifest {
    #[serde(flatten)]
    provenance: Provenance,
    buckets: Vec<BenchmarkBucket>,
    same_named_adapter: bool,
    all_complete: bool,
    hard_execution_constraint: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkBucket {
    manifold_count: u32,
    result: physbox_engine::GpuS13AuthorityProbeBenchmarkV1,
    selected_gpu: SampleStats,
    candidate_gpu: SampleStats,
    selected_cpu: SampleStats,
    candidate_cpu: SampleStats,
    candidate_median_delta_seconds: f64,
    candidate_p95_delta_seconds: f64,
    sixty_hz_reference_seconds: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleStats {
    count: usize,
    minimum: f64,
    median_nearest_rank: f64,
    p95_nearest_rank: f64,
    maximum: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    path: String,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s13-authority-gpu-probe error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or_else(|| "usage: physbox-s13-authority-gpu-probe <implementation|overflow|governed|benchmark> <output> <run-id>".to_owned())?;
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| "missing S13 output directory".to_owned())?,
    );
    let run_id = args.next().ok_or_else(|| "missing S13 run id".to_owned())?;
    if args.next().is_some() {
        return Err("unexpected S13 arguments".to_owned());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let provenance = provenance(&repository, &mode, run_id)?;
    prepare_output(&output)?;
    match mode.as_str() {
        "implementation" => run_implementation(&output, provenance)?,
        "overflow" => run_overflow(&output, provenance)?,
        "governed" => run_governed(&repository, &output, provenance)?,
        "benchmark" => run_benchmark(&output, provenance)?,
        _ => return Err("unknown S13 mode".to_owned()),
    }
    let manifest = fs::read(output.join("manifest.json")).map_err(display)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s13-{mode} manifest_sha256={} output={}",
        sha256_bytes(&manifest),
        output.display()
    );
    Ok(())
}

fn run_implementation(output: &Path, provenance: Provenance) -> Result<(), String> {
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut source = physbox_engine::p5_2_s11_soft_depth_sweep_scenario(11).map_err(display)?;
    source.requested_steps = 2;
    let source_scenario_hash = canonical_hash(&source).map_err(display)?;
    let primary = runner.run_authority_gpu_probe_observation_series(&source, 1, 2)?;
    let replay = runner.run_authority_gpu_probe_observation_series(&source, 1, 2)?;
    primary.validate()?;
    replay.validate()?;
    let primary_bytes = serde_json::to_vec(&primary).map_err(display)?;
    let replay_bytes = serde_json::to_vec(&replay).map_err(display)?;
    let exact_replay = primary_bytes == replay_bytes;
    let scalar = solve_authority_structure_steps_observed(
        &source,
        AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
        false,
    )
    .map_err(display)?;
    let maximum = maximum_body_delta(
        &primary
            .snapshots
            .last()
            .ok_or("missing primary snapshot")?
            .bodies,
        &scalar.result.bodies,
    );

    let mut one_step = source.clone();
    one_step.requested_steps = 1;
    let gpu_trace = runner.run_authority_gpu_probe_pass_trace(&one_step, 1)?;
    let (_, scalar_states) = solve_authority_structure_one_step_pass_states(
        &one_step,
        AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution),
    )
    .map_err(display)?;
    let first_phase_outside_band = gpu_trace
        .descriptors
        .iter()
        .enumerate()
        .filter_map(|(index, descriptor)| {
            pass_kind(descriptor.kind).map(|scalar_pass| (index, descriptor, scalar_pass))
        })
        .filter_map(|(index, descriptor, scalar_pass)| {
            scalar_states
                .iter()
                .find(|state| state.substep == descriptor.substep && state.kind == scalar_pass)
                .map(|state| {
                    (
                        descriptor,
                        scalar_pass,
                        maximum_body_delta(&gpu_trace.states[index + 1].bodies, &state.bodies),
                    )
                })
        })
        .find(|(_, _, delta)| delta.absolute_difference > IMPLEMENTATION_BAND)
        .map(|(descriptor, scalar_pass, maximum)| PhaseDelta {
            substep: descriptor.substep,
            gpu_pass: descriptor.kind,
            scalar_pass,
            maximum,
        });
    let manifest = ImplementationManifest {
        provenance,
        source_scenario_hash,
        adapter: primary.adapter.clone(),
        exact_replay,
        primary_sha256: sha256_bytes(&primary_bytes),
        replay_sha256: sha256_bytes(&replay_bytes),
        implementation_agreement: maximum.absolute_difference <= IMPLEMENTATION_BAND,
        two_step_scalar_maximum: maximum,
        implementation_band: IMPLEMENTATION_BAND,
        first_phase_outside_band,
        selected_product_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
    };
    write_json(output.join("manifest.json"), &manifest)
}

fn run_overflow(output: &Path, provenance: Provenance) -> Result<(), String> {
    let source = p5_2_s13_authority_overflow_scenario().map_err(display)?;
    let source_scenario_hash = canonical_hash(&source).map_err(display)?;
    let source_artifact = write_json_artifact(output, "source.json", &source)?;
    let capacity_primary = authority_gpu_probe_capacity_disposition(&source, 20)?;
    let capacity_replay = authority_gpu_probe_capacity_disposition(&source, 20)?;
    let gpu_result = pollster::block_on(run_gpu_authored_authority_probe(&source))?;
    let coloring = gpu_result
        .pass_evidence
        .coloring
        .as_ref()
        .ok_or_else(|| "overflow result omitted coloring".to_owned())?;
    let assignments_match = coloring.point_colors.len() == 21
        && coloring.point_colors[..20] == (0..20).collect::<Vec<_>>()
        && coloring.point_colors[20] == 23
        && coloring.color_occupancy[23] == 1;
    let overflow_first = gpu_result
        .pass_descriptors
        .iter()
        .filter(|descriptor| descriptor.color_passes != 0)
        .all(|descriptor| descriptor.parameters.uints1[2] == 2);
    let capacity_exact_replay = serde_json::to_vec(&capacity_primary).map_err(display)?
        == serde_json::to_vec(&capacity_replay).map_err(display)?;
    let fail_closed = capacity_primary.rejected
        && capacity_primary.overflowed
        && capacity_primary.attempted == 21
        && capacity_primary.capacity == 20
        && capacity_primary.accepted == 0
        && capacity_primary.committed == 0;
    let exact_replay = gpu_result.replay_exact;
    let manifest = OverflowManifest {
        provenance,
        source_scenario_hash,
        source: source_artifact,
        gpu_result,
        capacity_primary,
        capacity_replay,
        assignments_match,
        overflow_first,
        exact_replay,
        capacity_exact_replay,
        fail_closed,
    };
    write_json(output.join("manifest.json"), &manifest)
}

fn run_governed(repository: &Path, output: &Path, provenance: Provenance) -> Result<(), String> {
    fs::create_dir_all(output.join("sources")).map_err(display)?;
    fs::create_dir_all(output.join("captures")).map_err(display)?;
    let authority_bytes = fs::read(repository.join(AUTHORITY_MANIFEST)).map_err(display)?;
    let authority_input = Artifact {
        path: AUTHORITY_MANIFEST.to_owned(),
        sha256: sha256_bytes(&authority_bytes),
    };
    let authority: AuthorityManifest = serde_json::from_slice(&authority_bytes).map_err(display)?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let mut cases = Vec::new();
    for descriptor in GOVERNED {
        eprintln!("S13 governed depth {} primary/replay", descriptor.depth);
        let source_bytes =
            fs::read(repository.join(SOURCE_ROOT).join(descriptor.filename)).map_err(display)?;
        let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display)?;
        let source_scenario_hash = canonical_hash(&source).map_err(display)?;
        let native = authority
            .cases
            .iter()
            .find(|case| case.label == descriptor.label)
            .ok_or_else(|| format!("missing bound authority case {}", descriptor.label))?;
        let source_artifact = write_bytes_artifact(
            output,
            &format!("sources/{}.json", descriptor.label),
            &source_bytes,
        )?;
        let primary = runner.run_authority_gpu_probe_observation_series(&source, 1, 1_200)?;
        let replay = runner.run_authority_gpu_probe_observation_series(&source, 1, 1_200)?;
        primary.validate()?;
        replay.validate()?;
        let primary_bytes = serde_json::to_vec(&primary).map_err(display)?;
        let replay_bytes = serde_json::to_vec(&replay).map_err(display)?;
        let exact_replay = primary_bytes == replay_bytes;
        let primary_artifact = write_gzip_artifact(
            output,
            &format!("captures/{}-primary.json.gz", descriptor.label),
            &primary_bytes,
        )?;
        let replay_artifact = write_gzip_artifact(
            output,
            &format!("captures/{}-replay.json.gz", descriptor.label),
            &replay_bytes,
        )?;
        eprintln!("S13 governed depth {} pass-trace replay", descriptor.depth);
        let pass_primary = runner.run_authority_gpu_probe_pass_trace(&source, 1_200)?;
        let pass_replay = runner.run_authority_gpu_probe_pass_trace(&source, 1_200)?;
        let pass_primary_bytes = serde_json::to_vec(&pass_primary).map_err(display)?;
        let pass_replay_bytes = serde_json::to_vec(&pass_replay).map_err(display)?;
        let pass_trace_exact_replay = pass_primary_bytes == pass_replay_bytes;
        let pass_primary_artifact = write_gzip_artifact(
            output,
            &format!("captures/{}-pass-primary.json.gz", descriptor.label),
            &pass_primary_bytes,
        )?;
        let pass_replay_artifact = write_gzip_artifact(
            output,
            &format!("captures/{}-pass-replay.json.gz", descriptor.label),
            &pass_replay_bytes,
        )?;
        let candidate_quality = gpu_quality(&source, &primary)?;
        let candidate_pairs = gpu_active_support_pairs(&primary);
        let authority_pairs = normalize_pairs(native.authority.active_support_pairs.clone());
        let gates = quality_gates(
            &candidate_quality,
            &candidate_pairs,
            &native.authority.quality,
            &authority_pairs,
            source_scenario_hash == native.source_scenario_hash,
        );
        let qualified = exact_replay
            && pass_trace_exact_replay
            && gates.all()
            && primary.completed_steps == 1_200;
        cases.push(GovernedCase {
            label: descriptor.label,
            depth: descriptor.depth,
            source_scenario_hash,
            source: source_artifact,
            primary: primary_artifact,
            replay: replay_artifact,
            pass_trace_primary: pass_primary_artifact,
            pass_trace_replay: pass_replay_artifact,
            exact_replay,
            pass_trace_exact_replay,
            awake_product_ceiling_pass: candidate_quality.settled_linear_speed_max <= 0.025
                && candidate_quality.settled_angular_edge_speed_max <= 0.025,
            candidate_quality,
            authority_quality: native.authority.quality.clone(),
            candidate_active_support_pairs: candidate_pairs,
            authority_active_support_pairs: authority_pairs,
            gates,
            qualified,
        });
    }
    let all_cases_qualified = cases.iter().all(|case| case.qualified);
    let manifest = GovernedManifest {
        provenance,
        authority_input,
        exact_candidate_rejected_by_frozen_authority_quality: !all_cases_qualified,
        cases,
        all_cases_qualified,
    };
    write_json(output.join("manifest.json"), &manifest)
}

fn run_benchmark(output: &Path, provenance: Provenance) -> Result<(), String> {
    let mut buckets = Vec::new();
    for manifold_count in [32, 256, 1_024] {
        eprintln!("S13 benchmark {manifold_count} manifolds");
        let source = p5_2_s13_authority_measurement_scenario(manifold_count).map_err(display)?;
        let result = pollster::block_on(benchmark_gpu_s13_authority_probe(&source, 5, 20))?;
        let selected_gpu = sample_stats(&result.selected.gpu_complete_step_seconds)?;
        let candidate_gpu = sample_stats(&result.candidate.gpu_complete_step_seconds)?;
        let selected_cpu = sample_stats(&result.selected.cpu_encode_submit_seconds)?;
        let candidate_cpu = sample_stats(&result.candidate.cpu_encode_submit_seconds)?;
        buckets.push(BenchmarkBucket {
            manifold_count,
            candidate_median_delta_seconds: candidate_gpu.median_nearest_rank
                - selected_gpu.median_nearest_rank,
            candidate_p95_delta_seconds: candidate_gpu.p95_nearest_rank
                - selected_gpu.p95_nearest_rank,
            result,
            selected_gpu,
            candidate_gpu,
            selected_cpu,
            candidate_cpu,
            sixty_hz_reference_seconds: 1.0 / 60.0,
        });
    }
    let same_named_adapter = buckets.windows(2).all(|pair| {
        pair[0].result.adapter.name == pair[1].result.adapter.name
            && pair[0].result.adapter.backend == pair[1].result.adapter.backend
    });
    let all_complete = same_named_adapter
        && buckets.iter().all(|bucket| {
            bucket.result.warmup_samples == 5
                && bucket.result.retained_samples == 20
                && bucket.result.selected.replay_exact
                && bucket.result.candidate.replay_exact
                && bucket.result.selected.gpu_complete_step_seconds.len() == 20
                && bucket.result.candidate.gpu_complete_step_seconds.len() == 20
        });
    let manifest = BenchmarkManifest {
        provenance,
        buckets,
        same_named_adapter,
        all_complete,
        hard_execution_constraint: false,
    };
    write_json(output.join("manifest.json"), &manifest)
}

impl QualityGates {
    fn all(&self) -> bool {
        self.source_identity
            && self.finite_complete
            && self.structure
            && self.support_topology
            && self.linear_speed
            && self.angular_edge_speed
            && self.height_sum
            && self.penetration
            && self.energy_ratio
            && self.positive_energy_run
    }
}

fn quality_gates(
    candidate: &Quality,
    candidate_pairs: &[BodyPair],
    authority: &Quality,
    authority_pairs: &[BodyPair],
    source_identity: bool,
) -> QualityGates {
    let height_band = HEIGHT_ABSOLUTE.max(authority.maximum_penetration);
    let energy_limit = authority.energy_quartile_growth_ratio.max(1.0);
    QualityGates {
        source_identity,
        finite_complete: candidate.finite
            && candidate.settled_start_step == 900
            && candidate.settled_end_step == 1_200,
        structure: candidate.structure_pass,
        support_topology: candidate_pairs == authority_pairs,
        linear_speed: candidate.settled_linear_speed_max
            <= SPEED_MULTIPLIER * authority.settled_linear_speed_max + SPEED_ADDITIVE,
        angular_edge_speed: candidate.settled_angular_edge_speed_max
            <= SPEED_MULTIPLIER * authority.settled_angular_edge_speed_max + SPEED_ADDITIVE,
        height_sum: (candidate.final_dynamic_center_height_sum
            - authority.final_dynamic_center_height_sum)
            .abs()
            <= height_band,
        penetration: candidate.maximum_penetration
            <= authority.maximum_penetration + HEIGHT_ABSOLUTE,
        energy_ratio: candidate.energy_quartile_growth_ratio
            <= energy_limit + 1.0e-12 + 1.0e-5 * energy_limit,
        positive_energy_run: candidate.positive_energy_run_max
            <= authority.positive_energy_run_max.max(4),
    }
}

fn gpu_quality(
    source: &ScenarioV1,
    series: &GpuGeneratedContactSeriesV1,
) -> Result<Quality, String> {
    let settled = series
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.completed_steps >= 900)
        .collect::<Vec<_>>();
    if settled.len() != 301 {
        return Err(format!(
            "governed S13 settled window has {} snapshots",
            settled.len()
        ));
    }
    let energy = settled
        .iter()
        .map(|snapshot| kinetic_energy(&snapshot.bodies))
        .collect::<Vec<_>>();
    let quartile = settled.len() / 4;
    let first_energy_quartile_max = energy[..quartile].iter().copied().fold(0.0, f64::max);
    let last_energy_quartile_max = energy[energy.len() - quartile..]
        .iter()
        .copied()
        .fold(0.0, f64::max);
    let mut run = 0;
    let mut maximum_run = 0;
    for pair in energy.windows(2) {
        if pair[1] > pair[0] + 1.0e-12 + 1.0e-5 * pair[0].abs() {
            run += 1;
            maximum_run = maximum_run.max(run);
        } else {
            run = 0;
        }
    }
    let ratio = if first_energy_quartile_max > 0.0 {
        last_energy_quartile_max / first_energy_quartile_max
    } else if last_energy_quartile_max == 0.0 {
        1.0
    } else {
        f64::INFINITY
    };
    let final_snapshot = series
        .snapshots
        .last()
        .ok_or("missing governed final snapshot")?;
    let (structure_pass, structure_reason) =
        centered_stack_structure(source, &final_snapshot.bodies, false, 0.0);
    let penetration = |snapshot: &&physbox_engine::GpuGeneratedContactSnapshotV1| {
        snapshot
            .manifolds
            .iter()
            .flat_map(|manifold| &manifold.points)
            .map(|point| f64::from((-point.separation.get()).max(0.0)))
            .fold(0.0, f64::max)
    };
    let separation = |snapshot: &&physbox_engine::GpuGeneratedContactSnapshotV1| {
        snapshot
            .manifolds
            .iter()
            .flat_map(|manifold| &manifold.points)
            .map(|point| f64::from(point.separation.get().max(0.0)))
            .fold(0.0, f64::max)
    };
    Ok(Quality {
        settled_start_step: 900,
        settled_end_step: 1_200,
        settled_linear_speed_max: settled
            .iter()
            .flat_map(|snapshot| dynamic_bodies(&snapshot.bodies))
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
            .flat_map(|snapshot| dynamic_bodies(&snapshot.bodies))
            .map(|body| f64::from(body.angular_velocity.get().abs() * support_radius(body)))
            .fold(0.0, f64::max),
        first_energy_quartile_max,
        last_energy_quartile_max,
        energy_quartile_growth_ratio: ratio,
        positive_energy_run_max: maximum_run,
        energy_envelope_nonincreasing: ratio <= 1.0 + 1.0e-12 + 1.0e-5 && maximum_run < 5,
        maximum_penetration: settled.iter().map(penetration).fold(0.0, f64::max),
        maximum_separation: settled.iter().map(separation).fold(0.0, f64::max),
        final_active_normal_points: final_snapshot
            .cache
            .iter()
            .filter(|contact| contact.value.normal_impulse.get() > 0.0)
            .count() as u32,
        final_total_normal_impulse: final_snapshot
            .cache
            .iter()
            .map(|contact| f64::from(contact.value.normal_impulse.get()))
            .sum(),
        final_dynamic_center_height_sum: dynamic_bodies(&final_snapshot.bodies)
            .map(|body| f64::from(body.position.y))
            .sum(),
        structure_pass,
        structure_reason,
        finite: series.snapshots.iter().all(|snapshot| {
            snapshot.failure.is_none()
                && snapshot.bodies.iter().all(|body| body.validate().is_ok())
                && snapshot
                    .manifolds
                    .iter()
                    .all(|manifold| manifold.validate().is_ok())
                && snapshot
                    .cache
                    .iter()
                    .all(|contact| contact.validate().is_ok())
        }),
    })
}

fn gpu_active_support_pairs(series: &GpuGeneratedContactSeriesV1) -> Vec<BodyPair> {
    normalize_pairs(
        series
            .snapshots
            .last()
            .into_iter()
            .flat_map(|snapshot| &snapshot.cache)
            .filter(|contact| contact.value.normal_impulse.get() > 0.0)
            .map(cache_pair),
    )
}

fn cache_pair(contact: &PersistentContactV1) -> BodyPair {
    BodyPair {
        body_a: contact.key.pair.body_a,
        body_b: contact.key.pair.body_b,
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

fn maximum_body_delta(actual: &[BodyStateV1], expected: &[BodyStateV1]) -> StateDelta {
    let mut maximum = StateDelta {
        body_index: 0,
        field: "none",
        absolute_difference: 0.0,
    };
    for (body_index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        for (field, difference) in [
            (
                "position-x",
                f64::from((actual.position.x - expected.position.x).abs()),
            ),
            (
                "position-y",
                f64::from((actual.position.y - expected.position.y).abs()),
            ),
            (
                "angle",
                f64::from((actual.angle.get() - expected.angle.get()).abs()),
            ),
            (
                "linear-velocity-x",
                f64::from((actual.linear_velocity.x - expected.linear_velocity.x).abs()),
            ),
            (
                "linear-velocity-y",
                f64::from((actual.linear_velocity.y - expected.linear_velocity.y).abs()),
            ),
            (
                "angular-velocity",
                f64::from((actual.angular_velocity.get() - expected.angular_velocity.get()).abs()),
            ),
        ] {
            if difference > maximum.absolute_difference {
                maximum = StateDelta {
                    body_index,
                    field,
                    absolute_difference: difference,
                };
            }
        }
    }
    maximum
}

fn pass_kind(kind: GpuSolverPassKindV1) -> Option<AuthorityStructurePassKindV1> {
    match kind {
        GpuSolverPassKindV1::IntegrateVelocity => {
            Some(AuthorityStructurePassKindV1::IntegrateVelocity)
        }
        GpuSolverPassKindV1::WarmStart => Some(AuthorityStructurePassKindV1::WarmStart),
        GpuSolverPassKindV1::NormalBiased => Some(AuthorityStructurePassKindV1::BiasedSolve),
        GpuSolverPassKindV1::IntegratePosition => {
            Some(AuthorityStructurePassKindV1::IntegratePosition)
        }
        GpuSolverPassKindV1::RefreshContacts => Some(AuthorityStructurePassKindV1::Refresh),
        GpuSolverPassKindV1::NormalRelaxation => Some(AuthorityStructurePassKindV1::Relaxation),
        GpuSolverPassKindV1::Restitution => Some(AuthorityStructurePassKindV1::Restitution),
        _ => None,
    }
}

fn kinetic_energy(bodies: &[BodyStateV1]) -> f64 {
    dynamic_bodies(bodies)
        .map(|body| {
            let inverse_mass = f64::from(body.inverse_mass.get());
            let inverse_inertia = f64::from(body.inverse_inertia.get());
            let linear_squared = f64::from(
                body.linear_velocity.x * body.linear_velocity.x
                    + body.linear_velocity.y * body.linear_velocity.y,
            );
            let angular = f64::from(body.angular_velocity.get());
            0.5 * linear_squared / inverse_mass + 0.5 * angular * angular / inverse_inertia
        })
        .sum()
}

fn dynamic_bodies(bodies: &[BodyStateV1]) -> impl Iterator<Item = &BodyStateV1> {
    bodies.iter().filter(|body| body.inverse_mass.get() > 0.0)
}

fn support_radius(body: &BodyStateV1) -> f32 {
    match body.shape {
        ShapeV1::Circle { radius } => radius.get(),
        ShapeV1::Rectangle { half_extents } => {
            (half_extents.x * half_extents.x + half_extents.y * half_extents.y).sqrt()
        }
    }
}

fn sample_stats(values: &[f64]) -> Result<SampleStats, String> {
    if values.len() != 20
        || values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("S13 timing sample set is incomplete or non-finite".to_owned());
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(SampleStats {
        count: sorted.len(),
        minimum: sorted[0],
        median_nearest_rank: sorted[9],
        p95_nearest_rank: sorted[18],
        maximum: sorted[19],
    })
}

fn provenance(repository: &Path, mode: &str, run_id: String) -> Result<Provenance, String> {
    let revision = command_output(repository, &["rev-parse", "HEAD"])?;
    let dirty = !command_output(repository, &["status", "--porcelain"])?.is_empty();
    if dirty {
        return Err("S13 retained evidence requires a clean source tree".to_owned());
    }
    let candidate = AuthorityGpuProbePolicyV1::AUTHORIZED;
    let selected_snapshot = SelectedGpuSolverPolicySnapshotV1::current();
    Ok(Provenance {
        schema_version: SCHEMA_VERSION,
        artifact_kind: match mode {
            "implementation" => "p5.2b-s13.2-exact-gpu-implementation",
            "overflow" => "p5.2b-s13.3-overflow-capacity",
            "governed" => "p5.2b-s13.4-gpu-authority-qualification",
            "benchmark" => "p5.2b-s13.5-gpu-work-time",
            _ => return Err("unknown S13 provenance mode".to_owned()),
        },
        run_id,
        source_revision: revision,
        source_dirty: false,
        candidate_hash: sha256_bytes(&serde_json::to_vec(&candidate).map_err(display)?),
        candidate,
        selected_snapshot_hash: sha256_bytes(
            &serde_json::to_vec(&selected_snapshot).map_err(display)?,
        ),
        selected_snapshot,
        shader_sha256: sha256_bytes(
            &fs::read(repository.join("crates/physbox-engine/src/body_floor_solver.wgsl"))
                .map_err(display)?,
        ),
    })
}

fn command_output(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(display)?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(display)
}

fn prepare_output(output: &Path) -> Result<(), String> {
    if output.exists() {
        return Err(format!("S13 output already exists: {}", output.display()));
    }
    fs::create_dir_all(output).map_err(display)
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display)?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(display)
}

fn write_json_artifact(
    output: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<Artifact, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(display)?;
    bytes.push(b'\n');
    write_bytes_artifact(output, relative, &bytes)
}

fn write_bytes_artifact(output: &Path, relative: &str, bytes: &[u8]) -> Result<Artifact, String> {
    let path = output.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(display)?;
    }
    fs::write(&path, bytes).map_err(display)?;
    Ok(Artifact {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
    })
}

fn write_gzip_artifact(output: &Path, relative: &str, bytes: &[u8]) -> Result<Artifact, String> {
    let path = output.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(display)?;
    }
    let file = fs::File::create(&path).map_err(display)?;
    let mut encoder = GzBuilder::new().mtime(0).write(file, Compression::best());
    encoder.write_all(bytes).map_err(display)?;
    encoder.finish().map_err(display)?;
    let compressed = fs::read(&path).map_err(display)?;
    Ok(Artifact {
        path: relative.to_owned(),
        sha256: sha256_bytes(&compressed),
    })
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
