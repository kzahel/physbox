use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use physbox_contracts::{
    BROADPHASE_CONTRACT_VERSION, BroadphaseAlgorithmV1, BroadphaseEvaluationV1, BroadphaseResultV1,
    BroadphaseWorkV1, CandidatePairV1, CollisionCapacityV1, FailureV1, SCHEMA_VERSION, Validate,
    sha256_bytes,
};
use physbox_engine::{
    AdapterDiagnostics, GpuBroadphaseTimingV1, GpuCandidateOutput, GpuCandidateRunner,
    GpuGridConfigurationV1, GpuMultiCellGridRunner, GpuSweepAndPruneRunner, P5_3BroadphasePattern,
    p5_3_broadphase_scenario, p5_3_broadphase_snapshot,
};
use physbox_lab::broadphase::{brute_force_broadphase, evaluate_broadphase};
use serde::Serialize;

const ARTIFACT_VERSION: u32 = 1;
const DEFAULT_COUNTS: [u32; 3] = [32, 256, 1_024];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BroadphaseEvidenceManifestV1 {
    schema_version: u32,
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    profile: &'static str,
    adapter: AdapterDiagnostics,
    warmup_steps: u32,
    sample_steps: u32,
    oracle_max_count: u32,
    brute_max_count: u32,
    capacity_policy: CapacityPolicyV1,
    cases: Vec<CaseEvidenceV1>,
    all_accepted: bool,
    selection_made: bool,
    product_graph_changed: bool,
    physical_defaults_changed: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityPolicyV1 {
    candidate_elements_per_proxy: u32,
    cell_records_per_proxy: u32,
    pair_emissions_per_proxy: u32,
    global_static_capacity: u32,
    cell_size: f32,
    max_cells_per_fixed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseEvidenceV1 {
    pattern: &'static str,
    proxy_count: u32,
    seed: u64,
    snapshot_hash: String,
    oracle_pair_count: Option<u64>,
    brute: Option<CandidateEvidenceV1>,
    sap: CandidateEvidenceV1,
    grid: CandidateEvidenceV1,
    cross_candidate_exact: bool,
    first_cross_candidate_difference: Option<physbox_contracts::CandidatePairV1>,
    accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEvidenceV1 {
    result: BroadphaseResultSummaryV1,
    evaluation: Option<BroadphaseEvaluationV1>,
    exact_replay: bool,
    allocated_bytes: u64,
    high_water_bytes: u64,
    timing: GpuBroadphaseTimingV1,
    gpu_seconds: Option<TimingSummaryV1>,
    host_completion_seconds: TimingSummaryV1,
    cpu_encode_submit_seconds: TimingSummaryV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BroadphaseResultSummaryV1 {
    algorithm: BroadphaseAlgorithmV1,
    source_snapshot_hash: String,
    pair_count: u64,
    pair_digest: String,
    first_pair: Option<CandidatePairV1>,
    last_pair: Option<CandidatePairV1>,
    work: BroadphaseWorkV1,
    capacities: Vec<CollisionCapacityV1>,
    failure: Option<FailureV1>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingSummaryV1 {
    minimum: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    maximum: f64,
}

struct Arguments {
    output: PathBuf,
    run_id: String,
    counts: Vec<u32>,
    patterns: Vec<P5_3BroadphasePattern>,
    warmup_steps: u32,
    sample_steps: u32,
    oracle_max_count: u32,
    brute_max_count: u32,
    capacity_policy: CapacityPolicyV1,
    require_clean: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-broadphase-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    if arguments.output.exists() {
        return Err("broadphase evidence output must be fresh".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if staging.exists() {
        return Err("broadphase evidence staging path must be fresh".to_owned());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("broadphase evidence requires a clean source tree".to_owned());
    }
    let sap_runner = pollster::block_on(GpuSweepAndPruneRunner::new())?;
    let grid_runner = pollster::block_on(GpuMultiCellGridRunner::new())?;
    let brute_runner = pollster::block_on(GpuCandidateRunner::new())?;
    same_adapter(sap_runner.adapter(), grid_runner.adapter())?;
    same_adapter(sap_runner.adapter(), brute_runner.adapter())?;
    let adapter = sap_runner.adapter().clone();
    let mut cases = Vec::new();
    for &count in &arguments.counts {
        for &pattern in &arguments.patterns {
            if pattern == P5_3BroadphasePattern::CompleteOverlap && count > 32 {
                continue;
            }
            let seed = 0x5030_0000 + u64::from(count) * 17 + pattern as u64;
            let snapshot = p5_3_broadphase_snapshot(pattern, count, seed)
                .map_err(|error| error.to_string())?;
            let mut candidate_capacity = scaled_capacity(
                count,
                arguments.capacity_policy.candidate_elements_per_proxy,
            )?;
            let mut pair_emission_capacity =
                scaled_capacity(count, arguments.capacity_policy.pair_emissions_per_proxy)?;
            if pattern == P5_3BroadphasePattern::CompleteOverlap {
                let all_pairs = u64::from(count) * u64::from(count - 1) / 2;
                candidate_capacity = candidate_capacity.max(
                    u32::try_from(all_pairs)
                        .map_err(|_| "complete-overlap candidate count exceeds u32".to_owned())?,
                );
                pair_emission_capacity = pair_emission_capacity.max(
                    u32::try_from(all_pairs * 9)
                        .map_err(|_| "complete-overlap emission count exceeds u32".to_owned())?,
                );
            }
            let grid_configuration = GpuGridConfigurationV1 {
                cell_size: arguments.capacity_policy.cell_size,
                max_cells_per_fixed: arguments.capacity_policy.max_cells_per_fixed,
                cell_record_capacity: scaled_capacity(
                    count,
                    arguments.capacity_policy.cell_records_per_proxy,
                )?,
                global_static_capacity: arguments.capacity_policy.global_static_capacity,
                pair_emission_capacity,
                candidate_capacity,
            };
            let oracle = (count <= arguments.oracle_max_count)
                .then(|| brute_force_broadphase(&snapshot, None))
                .transpose()?;
            let brute = if count <= arguments.brute_max_count {
                let scenario = p5_3_broadphase_scenario(pattern, count, seed)
                    .map_err(|error| error.to_string())?;
                let all_pair_count = u64::from(count) * u64::from(count - 1) / 2;
                let brute_capacity = u32::try_from(all_pair_count)
                    .map_err(|_| "GPU brute-force pair count exceeds u32".to_owned())?
                    .min(candidate_capacity);
                let benchmark = brute_runner.benchmark(
                    &scenario,
                    Some(brute_capacity),
                    arguments.warmup_steps,
                    arguments.sample_steps,
                )?;
                let replay = brute_runner.run(&scenario, Some(brute_capacity))?;
                let result = broadphase_brute_result(&snapshot, benchmark.correctness)?;
                let replay = broadphase_brute_result(&snapshot, replay)?;
                let exact_replay = result == replay;
                let evaluation = oracle
                    .as_ref()
                    .map(|oracle| evaluate_broadphase(oracle, &result, exact_replay))
                    .transpose()?;
                Some(candidate_evidence(
                    result,
                    evaluation,
                    exact_replay,
                    benchmark.timing,
                    Some(benchmark.allocated_bytes),
                )?)
            } else {
                None
            };
            let sap_benchmark = sap_runner.benchmark(
                &snapshot,
                candidate_capacity,
                arguments.warmup_steps,
                arguments.sample_steps,
            )?;
            let sap_replay = sap_runner.run(&snapshot, candidate_capacity)?;
            let sap_exact_replay = sap_replay.result == sap_benchmark.correctness.result;
            let sap_result = sap_benchmark.correctness.result;
            let sap_evaluation = oracle
                .as_ref()
                .map(|oracle| evaluate_broadphase(oracle, &sap_result, sap_exact_replay))
                .transpose()?;

            let grid_benchmark = grid_runner.benchmark(
                &snapshot,
                grid_configuration,
                arguments.warmup_steps,
                arguments.sample_steps,
            )?;
            let grid_replay = grid_runner.run(&snapshot, grid_configuration)?;
            let grid_exact_replay = grid_replay.result == grid_benchmark.correctness.result;
            let grid_result = grid_benchmark.correctness.result;
            let grid_evaluation = oracle
                .as_ref()
                .map(|oracle| evaluate_broadphase(oracle, &grid_result, grid_exact_replay))
                .transpose()?;
            let (cross_candidate_exact, first_cross_candidate_difference) =
                compare_pairs(&sap_result.pairs, &grid_result.pairs);
            let oracle_accepted = sap_evaluation
                .as_ref()
                .is_none_or(|evaluation| evaluation.accepted)
                && grid_evaluation
                    .as_ref()
                    .is_none_or(|evaluation| evaluation.accepted);
            let accepted = cross_candidate_exact
                && oracle_accepted
                && sap_exact_replay
                && grid_exact_replay
                && sap_result.failure.is_none()
                && grid_result.failure.is_none()
                && brute.as_ref().is_none_or(|brute| {
                    brute.exact_replay
                        && brute.result.failure.is_none()
                        && brute
                            .evaluation
                            .as_ref()
                            .is_none_or(|evaluation| evaluation.accepted)
                });
            let snapshot_hash = sap_result.source_snapshot_hash.clone();
            let sap = candidate_evidence(
                sap_result,
                sap_evaluation,
                sap_exact_replay,
                sap_benchmark.timing,
                None,
            )?;
            let grid = candidate_evidence(
                grid_result,
                grid_evaluation,
                grid_exact_replay,
                grid_benchmark.timing,
                None,
            )?;
            cases.push(CaseEvidenceV1 {
                pattern: pattern.name(),
                proxy_count: count,
                seed,
                snapshot_hash,
                oracle_pair_count: oracle.as_ref().map(|oracle| oracle.pairs.len() as u64),
                brute,
                sap,
                grid,
                cross_candidate_exact,
                first_cross_candidate_difference,
                accepted,
            });
        }
    }
    let all_accepted = cases.iter().all(|case| case.accepted);
    let manifest = BroadphaseEvidenceManifestV1 {
        schema_version: SCHEMA_VERSION,
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "physbox5-broadphase-evidence-v1",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        adapter,
        warmup_steps: arguments.warmup_steps,
        sample_steps: arguments.sample_steps,
        oracle_max_count: arguments.oracle_max_count,
        brute_max_count: arguments.brute_max_count,
        capacity_policy: arguments.capacity_policy,
        cases,
        all_accepted,
        selection_made: false,
        product_graph_changed: false,
        physical_defaults_changed: false,
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(&staging, bytes).map_err(|error| error.to_string())?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    if !all_accepted {
        return Err(format!(
            "one or more broadphase rows failed; retained artifact at {}",
            arguments.output.display()
        ));
    }
    println!("PHYSBOX5_VALIDATION_SUCCESS");
    Ok(())
}

fn candidate_evidence(
    result: BroadphaseResultV1,
    evaluation: Option<BroadphaseEvaluationV1>,
    exact_replay: bool,
    timing: GpuBroadphaseTimingV1,
    allocated_bytes_override: Option<u64>,
) -> Result<CandidateEvidenceV1, String> {
    let reported_allocated_bytes = result
        .capacities
        .iter()
        .try_fold(0_u64, |sum, capacity| {
            sum.checked_add(capacity.allocated_bytes)
        })
        .ok_or_else(|| "broadphase allocated byte total overflowed".to_owned())?;
    let allocated_bytes = allocated_bytes_override.unwrap_or(reported_allocated_bytes);
    let high_water_bytes = result
        .capacities
        .iter()
        .try_fold(0_u64, |sum, capacity| {
            sum.checked_add(capacity.high_water_bytes)
        })
        .ok_or_else(|| "broadphase high-water byte total overflowed".to_owned())?;
    let gpu_seconds = timing
        .gpu_step_seconds
        .as_deref()
        .map(timing_summary)
        .transpose()?;
    let host_completion_seconds = timing_summary(&timing.host_completion_seconds)?;
    let cpu_encode_submit_seconds = timing_summary(&timing.cpu_encode_submit_seconds)?;
    let result = BroadphaseResultSummaryV1 {
        algorithm: result.algorithm,
        source_snapshot_hash: result.source_snapshot_hash,
        pair_count: result.pairs.len() as u64,
        pair_digest: sha256_bytes(
            &serde_json::to_vec(&result.pairs).map_err(|error| error.to_string())?,
        ),
        first_pair: result.pairs.first().copied(),
        last_pair: result.pairs.last().copied(),
        work: result.work,
        capacities: result.capacities,
        failure: result.failure,
    };
    Ok(CandidateEvidenceV1 {
        result,
        evaluation,
        exact_replay,
        allocated_bytes,
        high_water_bytes,
        timing,
        gpu_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
    })
}

fn broadphase_brute_result(
    snapshot: &physbox_contracts::BroadphaseSnapshotV1,
    output: GpuCandidateOutput,
) -> Result<BroadphaseResultV1, String> {
    let result = BroadphaseResultV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: BROADPHASE_CONTRACT_VERSION,
        algorithm: BroadphaseAlgorithmV1::BruteForce,
        source_snapshot_hash: physbox_contracts::canonical_hash(snapshot)
            .map_err(|error| error.to_string())?,
        pairs: output.candidates,
        work: BroadphaseWorkV1 {
            proxies: output.work.bodies,
            dynamic_proxies: snapshot
                .proxies
                .iter()
                .filter(|proxy| proxy.mobility == physbox_contracts::BroadphaseMobilityV1::Dynamic)
                .count() as u64,
            fixed_proxies: snapshot
                .proxies
                .iter()
                .filter(|proxy| proxy.mobility == physbox_contracts::BroadphaseMobilityV1::Fixed)
                .count() as u64,
            pair_tests: output.work.pair_attempts,
            fixed_fixed_rejected: output.work.fixed_fixed_rejected,
            candidate_emissions: output.work.candidate_pairs,
            duplicate_emissions: 0,
            unique_candidates: output.work.candidate_pairs,
            sort_elements: 0,
            sort_passes: 0,
            cell_records: 0,
            global_statics: 0,
            maximum_active: 0,
            maximum_cell_occupancy: 0,
            dispatches: output.work.dispatches,
        },
        capacities: output.capacities,
        failure: output.failure,
    };
    result.validate().map_err(|error| error.to_string())?;
    Ok(result)
}

fn timing_summary(samples: &[f64]) -> Result<TimingSummaryV1, String> {
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err("broadphase timing samples must be finite and nonempty".to_owned());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |percent: usize| sorted[(sorted.len() - 1) * percent / 100];
    Ok(TimingSummaryV1 {
        minimum: sorted[0],
        p50: percentile(50),
        p95: percentile(95),
        p99: percentile(99),
        maximum: *sorted.last().expect("samples are nonempty"),
    })
}

fn compare_pairs(
    left: &[physbox_contracts::CandidatePairV1],
    right: &[physbox_contracts::CandidatePairV1],
) -> (bool, Option<physbox_contracts::CandidatePairV1>) {
    if left == right {
        return (true, None);
    }
    let first = left
        .iter()
        .zip(right)
        .find_map(|(left, right)| (left != right).then_some((*left).min(*right)))
        .or_else(|| left.get(right.len()).copied())
        .or_else(|| right.get(left.len()).copied());
    (false, first)
}

fn scaled_capacity(count: u32, multiplier: u32) -> Result<u32, String> {
    count
        .checked_mul(multiplier)
        .map(|capacity| capacity.max(64))
        .ok_or_else(|| format!("capacity {count} * {multiplier} overflowed u32"))
}

fn same_adapter(left: &AdapterDiagnostics, right: &AdapterDiagnostics) -> Result<(), String> {
    let left = serde_json::to_vec(left).map_err(|error| error.to_string())?;
    let right = serde_json::to_vec(right).map_err(|error| error.to_string())?;
    if left != right {
        return Err("broadphase candidate adapters differ".to_owned());
    }
    Ok(())
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    let status = command_bytes(repository, "git", &["status", "--porcelain=v1"])?;
    if status.is_empty() {
        return Ok((revision.trim().to_owned(), false, None));
    }
    let diff = command_bytes(repository, "git", &["diff", "--binary", "HEAD", "--", "."])?;
    let mut patch = status;
    patch.extend_from_slice(&diff);
    Ok((revision.trim().to_owned(), true, Some(sha256_bytes(&patch))))
}

fn command_output(repository: &Path, program: &str, arguments: &[&str]) -> Result<String, String> {
    String::from_utf8(command_bytes(repository, program, arguments)?)
        .map_err(|error| error.to_string())
}

fn command_bytes(repository: &Path, program: &str, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut output = None;
    let mut run_id = None;
    let mut counts = DEFAULT_COUNTS.to_vec();
    let mut patterns = P5_3BroadphasePattern::ALL.to_vec();
    let mut warmup_steps = 4;
    let mut sample_steps = 9;
    let mut oracle_max_count = 1_024;
    let mut brute_max_count = 2_048;
    let mut capacity_policy = CapacityPolicyV1 {
        candidate_elements_per_proxy: 16,
        cell_records_per_proxy: 16,
        pair_emissions_per_proxy: 64,
        global_static_capacity: 2_048,
        cell_size: 1.0,
        max_cells_per_fixed: 16,
    };
    let mut require_clean = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = |arguments: &mut std::iter::Skip<std::env::Args>| {
            arguments
                .next()
                .ok_or_else(|| format!("{argument} requires a value"))
        };
        match argument.as_str() {
            "--output" => output = Some(PathBuf::from(value(&mut arguments)?)),
            "--run-id" => run_id = Some(value(&mut arguments)?),
            "--counts" => counts = parse_counts(&value(&mut arguments)?)?,
            "--patterns" => patterns = parse_patterns(&value(&mut arguments)?)?,
            "--warmup" => warmup_steps = parse_u32(&argument, &value(&mut arguments)?)?,
            "--samples" => sample_steps = parse_u32(&argument, &value(&mut arguments)?)?,
            "--oracle-max" => oracle_max_count = parse_u32(&argument, &value(&mut arguments)?)?,
            "--brute-max" => brute_max_count = parse_u32(&argument, &value(&mut arguments)?)?,
            "--candidate-multiplier" => {
                capacity_policy.candidate_elements_per_proxy =
                    parse_u32(&argument, &value(&mut arguments)?)?
            }
            "--cell-record-multiplier" => {
                capacity_policy.cell_records_per_proxy =
                    parse_u32(&argument, &value(&mut arguments)?)?
            }
            "--emission-multiplier" => {
                capacity_policy.pair_emissions_per_proxy =
                    parse_u32(&argument, &value(&mut arguments)?)?
            }
            "--global-capacity" => {
                capacity_policy.global_static_capacity =
                    parse_u32(&argument, &value(&mut arguments)?)?
            }
            "--max-cells-per-fixed" => {
                capacity_policy.max_cells_per_fixed = parse_u32(&argument, &value(&mut arguments)?)?
            }
            "--cell-size" => {
                capacity_policy.cell_size = value(&mut arguments)?
                    .parse()
                    .map_err(|_| "--cell-size must be a number".to_owned())?
            }
            "--require-clean" => require_clean = true,
            _ => return Err(format!("unknown broadphase evidence argument {argument}")),
        }
    }
    if counts.is_empty() || patterns.is_empty() || warmup_steps == 0 || sample_steps == 0 {
        return Err("broadphase evidence selections and sample counts must be nonempty".to_owned());
    }
    GpuGridConfigurationV1 {
        cell_size: capacity_policy.cell_size,
        max_cells_per_fixed: capacity_policy.max_cells_per_fixed,
        cell_record_capacity: 1,
        global_static_capacity: capacity_policy.global_static_capacity,
        pair_emission_capacity: 1,
        candidate_capacity: 1,
    }
    .validate()?;
    Ok(Arguments {
        output: output.ok_or_else(|| "--output is required".to_owned())?,
        run_id: run_id.ok_or_else(|| "--run-id is required".to_owned())?,
        counts,
        patterns,
        warmup_steps,
        sample_steps,
        oracle_max_count,
        brute_max_count,
        capacity_policy,
        require_clean,
    })
}

fn parse_counts(value: &str) -> Result<Vec<u32>, String> {
    value
        .split(',')
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| format!("invalid broadphase count {value}"))
                .and_then(|count| {
                    (2..=100_000)
                        .contains(&count)
                        .then_some(count)
                        .ok_or_else(|| format!("broadphase count {count} is outside 2..=100000"))
                })
        })
        .collect()
}

fn parse_patterns(value: &str) -> Result<Vec<P5_3BroadphasePattern>, String> {
    value
        .split(',')
        .map(|name| {
            P5_3BroadphasePattern::ALL
                .into_iter()
                .find(|pattern| pattern.name() == name)
                .ok_or_else(|| format!("unknown broadphase pattern {name}"))
        })
        .collect()
}

fn parse_u32(name: &str, value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("{name} must be an integer"))
}
