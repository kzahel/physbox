use std::{env, fs, path::PathBuf, process::Command, time::SystemTime};

use physbox_contracts::{
    CollisionCapacityV1, CollisionWorkV1, ContactIdentityPolicyV1, ScenarioV1, SolverSettingsV1,
    canonical_hash,
};
use physbox_engine::{
    AdapterDiagnostics, GpuContactCoefficientPolicyV1, GpuGeneratedContactBenchmarkOutput,
    GpuRestitutionTimingPolicyV1, GpuSchedulerWorkVectorV1, GpuSolverExecutionV4,
    GpuSolverInstrumentedSampleV4, GpuSolverSchedulingPolicyV1, GpuSubstepContactPolicyV1,
    benchmark_gpu_sleeping_city, benchmark_gpu_sparse_capacity,
    p5_2_generated_performance_scenario, p5_2_sleeping_city_scenario,
};
use serde::Serialize;

const MATCHED_BODY_COUNT: u32 = 1_000;
const MATCHED_BODY_CAPACITY: u32 = 1_024;
const AWAKE_PERCENTAGES: [u32; 3] = [1, 10, 100];
const RESERVE_BODY_COUNT: u32 = 200;
const RESERVE_CAPACITIES: [u32; 3] = [256, 1_024, 10_000];
const SPARSE_CAPACITIES: [u32; 4] = [64, 256, 1_024, 10_000];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepingCityArtifactV1 {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    pacing_mode: &'static str,
    adapter: AdapterDiagnostics,
    warmup_steps: u32,
    samples: u32,
    timestamp_status: &'static str,
    timestamp_period_nanoseconds: f32,
    configuration: ConfigurationV1,
    matched_normalized_scenario_hash: String,
    matched_rows: Vec<SleepingCityRowV1>,
    reserve_normalized_scenario_hash: String,
    reserve_rows: Vec<SleepingCityRowV1>,
    sparse_scenario_hash: String,
    sparse_rows: Vec<SleepingCityRowV1>,
    sparse_gpu_p50_max_min_ratio: f64,
    reserve_gpu_p50_max_min_ratio: f64,
    dormant_pair_decision: DormantPairDecisionV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationV1 {
    solver_settings: SolverSettingsV1,
    solver_execution: GpuSolverExecutionV4,
    solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    contact_coefficient_policy: GpuContactCoefficientPolicyV1,
    substep_contact_policy: GpuSubstepContactPolicyV1,
    restitution_timing_policy: GpuRestitutionTimingPolicyV1,
    contact_identity_policy: ContactIdentityPolicyV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepingCityRowV1 {
    family: &'static str,
    source_scenario_hash: String,
    body_capacity: u32,
    live_bodies: u32,
    authored_awake_percent: u32,
    awake_dynamic_bodies: u32,
    solver_bodies: u32,
    live_contact_points: u32,
    awake_contact_points: u32,
    awake_point_fraction: f64,
    dispatches_per_step: u32,
    graph_allocated_bytes: u64,
    total_accounted_allocated_bytes: u64,
    collision_persistence_allocated_bytes: u64,
    work: CollisionWorkV1,
    scheduler_work: Option<GpuSchedulerWorkVectorV1>,
    capacities: Vec<CollisionCapacityV1>,
    gpu_collision_and_persistence: TimingSeriesV1,
    gpu_persistence: TimingSeriesV1,
    gpu_solver_and_sleeping: TimingSeriesV1,
    gpu_end_to_end: TimingSeriesV1,
    host_completion: TimingSeriesV1,
    cpu_encode_submit: TimingSeriesV1,
    solver_phases: SolverPhaseSummaryV1,
    replay_exact: bool,
    overflowed: bool,
    completed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingSeriesV1 {
    samples_seconds: Vec<f64>,
    p50_seconds: f64,
    p95_seconds: f64,
    p99_seconds: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SolverPhaseSummaryV1 {
    color_construction: TimingSeriesV1,
    color_compaction: TimingSeriesV1,
    active_color_solve: TimingSeriesV1,
    empty_color_solve: TimingSeriesV1,
    other_solver: TimingSeriesV1,
    total_instrumented: TimingSeriesV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DormantPairDecisionV1 {
    decision: &'static str,
    basis_row: &'static str,
    collision_and_persistence_p50_share: f64,
    persistence_p50_share: f64,
    live_to_awake_point_ratio: f64,
    demonstrated_need: bool,
    rationale: String,
}

struct Options {
    output: PathBuf,
    warmup_steps: u32,
    samples: u32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-sleeping-city-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("sleeping-city output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain"])?.is_empty();
    if source_dirty {
        return Err("sleeping-city evidence requires a clean source tree".to_owned());
    }

    let matched_scenarios = AWAKE_PERCENTAGES
        .into_iter()
        .map(|percent| {
            p5_2_sleeping_city_scenario(MATCHED_BODY_COUNT, percent)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let matched_normalized_scenario_hash = matched_hash(&matched_scenarios)?;
    let matched_results = matched_scenarios
        .iter()
        .map(|scenario| {
            pollster::block_on(benchmark_gpu_sleeping_city(
                scenario,
                MATCHED_BODY_CAPACITY,
                options.warmup_steps,
                options.samples,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_matched(&matched_results, &AWAKE_PERCENTAGES)?;

    let reserve_scenario =
        p5_2_sleeping_city_scenario(RESERVE_BODY_COUNT, 1).map_err(|error| error.to_string())?;
    let reserve_normalized_scenario_hash = matched_hash(std::slice::from_ref(&reserve_scenario))?;
    let reserve_results = RESERVE_CAPACITIES
        .into_iter()
        .map(|capacity| {
            pollster::block_on(benchmark_gpu_sleeping_city(
                &reserve_scenario,
                capacity,
                options.warmup_steps,
                options.samples,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_reserve(&reserve_results, &RESERVE_CAPACITIES)?;

    let sparse_scenario =
        p5_2_generated_performance_scenario(2).map_err(|error| error.to_string())?;
    let sparse_results = SPARSE_CAPACITIES
        .into_iter()
        .map(|capacity| {
            pollster::block_on(benchmark_gpu_sparse_capacity(
                &sparse_scenario,
                capacity,
                options.warmup_steps,
                options.samples,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_reserve(&sparse_results, &SPARSE_CAPACITIES)?;

    let expected_adapter = matched_results[0].adapter.marker_json();
    let expected_configuration = configuration(&matched_results[0]);
    for result in reserve_results.iter().chain(&sparse_results) {
        if result.adapter.marker_json() != expected_adapter
            || configuration(result) != expected_configuration
        {
            return Err("sleeping-city rows changed adapter or selected configuration".to_owned());
        }
    }
    let timestamp_period_nanoseconds = matched_results[0]
        .timestamp_period_nanoseconds
        .ok_or_else(|| "sleeping-city evidence requires native GPU timestamps".to_owned())?;
    for result in matched_results
        .iter()
        .chain(&reserve_results)
        .chain(&sparse_results)
    {
        if result.timestamp_period_nanoseconds != Some(timestamp_period_nanoseconds) {
            return Err("sleeping-city rows changed timestamp capability or period".to_owned());
        }
    }

    let adapter = matched_results[0].adapter.clone();
    let sparse_scenario_hash = sparse_results[0].source_scenario_hash.clone();
    let matched_rows = matched_results
        .into_iter()
        .zip(AWAKE_PERCENTAGES)
        .map(|(result, percent)| row("matched-sleeping-city", percent, result))
        .collect::<Result<Vec<_>, _>>()?;
    let reserve_rows = reserve_results
        .into_iter()
        .map(|result| row("one-percent-cross-capacity", 1, result))
        .collect::<Result<Vec<_>, _>>()?;
    let sparse_rows = sparse_results
        .into_iter()
        .map(|result| row("two-body-sparse-capacity", 100, result))
        .collect::<Result<Vec<_>, _>>()?;
    let sparse_gpu_p50_max_min_ratio = p50_ratio(&sparse_rows)?;
    let reserve_gpu_p50_max_min_ratio = p50_ratio(&reserve_rows)?;
    if sparse_gpu_p50_max_min_ratio > 1.5 || reserve_gpu_p50_max_min_ratio > 1.5 {
        return Err(format!(
            "awake work lost reserve flatness: sparse ratio {sparse_gpu_p50_max_min_ratio:.3}, reserve ratio {reserve_gpu_p50_max_min_ratio:.3}"
        ));
    }
    let dormant_pair_decision = dormant_pair_decision(&matched_rows)?;

    let run_id = format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs(),
        std::process::id(),
    );
    let artifact = SleepingCityArtifactV1 {
        schema_version: 1,
        artifact_kind: "physbox5-sleeping-city-evidence-v1",
        run_id,
        source_revision,
        source_dirty,
        host: "native-gpu",
        pacing_mode: "explicit-step-throughput",
        adapter,
        warmup_steps: options.warmup_steps,
        samples: options.samples,
        timestamp_status: "valid-native-gpu-timestamps",
        timestamp_period_nanoseconds,
        configuration: expected_configuration,
        matched_normalized_scenario_hash,
        matched_rows,
        reserve_normalized_scenario_hash,
        reserve_rows,
        sparse_scenario_hash,
        sparse_rows,
        sparse_gpu_p50_max_min_ratio,
        reserve_gpu_p50_max_min_ratio,
        dormant_pair_decision,
    };
    let staging = options.output.with_extension("json.staging");
    if staging.exists() {
        return Err("sleeping-city staging output must be fresh".to_owned());
    }
    fs::write(
        &staging,
        serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&staging, &options.output).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_SLEEPING_CITY_SUCCESS run_id={} matched_rows={} reserve_rows={} sparse_rows={} decision={} output={}",
        artifact.run_id,
        artifact.matched_rows.len(),
        artifact.reserve_rows.len(),
        artifact.sparse_rows.len(),
        artifact.dormant_pair_decision.decision,
        options.output.display(),
    );
    Ok(())
}

fn validate_matched(
    results: &[GpuGeneratedContactBenchmarkOutput],
    percentages: &[u32],
) -> Result<(), String> {
    if results.len() != percentages.len() || results.is_empty() {
        return Err("sleeping-city matched curve is incomplete".to_owned());
    }
    let first = &results[0];
    let expected_adapter = first.adapter.marker_json();
    let expected_configuration = configuration(first);
    for (result, &percent) in results.iter().zip(percentages) {
        validate_result(result)?;
        let expected_awake = result.body_count * percent / 100;
        if result.body_capacity != MATCHED_BODY_CAPACITY
            || result.adapter.marker_json() != expected_adapter
            || configuration(result) != expected_configuration
            || result.correctness.awake_dynamic_body_count != expected_awake
            || result.correctness.solver_body_count != expected_awake
            || result.correctness.live_contact_point_count
                != first.correctness.live_contact_point_count
            || result.correctness.work != first.correctness.work
        {
            return Err(format!(
                "sleeping-city {percent}% row changed matched scene work or awake counts"
            ));
        }
    }
    for pair in results.windows(2) {
        if pair[0].correctness.awake_contact_point_count
            >= pair[1].correctness.awake_contact_point_count
        {
            return Err(
                "sleeping-city awake point work did not grow with awake fraction".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_reserve(
    results: &[GpuGeneratedContactBenchmarkOutput],
    capacities: &[u32],
) -> Result<(), String> {
    if results.len() != capacities.len() || results.is_empty() {
        return Err("cross-capacity curve is incomplete".to_owned());
    }
    let first = &results[0];
    for (result, &capacity) in results.iter().zip(capacities) {
        validate_result(result)?;
        if result.body_capacity != capacity
            || result.correctness.work != first.correctness.work
            || result.correctness.awake_dynamic_body_count
                != first.correctness.awake_dynamic_body_count
            || result.correctness.awake_contact_point_count
                != first.correctness.awake_contact_point_count
        {
            return Err(format!(
                "capacity {capacity} changed committed work or awake counts"
            ));
        }
    }
    Ok(())
}

fn validate_result(result: &GpuGeneratedContactBenchmarkOutput) -> Result<(), String> {
    let overflowed = result.correctness.failure.is_some()
        || result
            .correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted || capacity.overflowed);
    if !result.replay_exact || overflowed || result.body_count == 0 {
        return Err(format!(
            "capacity {} failed completion, replay, or overflow gates",
            result.body_capacity
        ));
    }
    for (label, samples) in [
        ("host completion", Some(&result.host_completion_seconds)),
        ("CPU encode/submit", Some(&result.cpu_encode_submit_seconds)),
        ("GPU collision", result.gpu_collision_seconds.as_ref()),
        ("GPU persistence", result.gpu_persistence_seconds.as_ref()),
        ("GPU end-to-end", result.gpu_end_to_end_seconds.as_ref()),
    ] {
        let samples = samples.ok_or_else(|| format!("{label} timestamps are unavailable"))?;
        if samples.len() != result.samples as usize
            || samples
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(format!(
                "capacity {} has invalid {label} samples",
                result.body_capacity
            ));
        }
    }
    let phases = result
        .instrumented_solver_phases
        .as_ref()
        .ok_or_else(|| "instrumented solver timestamps are unavailable".to_owned())?;
    if phases.len() != result.samples as usize
        || phases.iter().any(|sample| {
            !sample.total_instrumented_seconds.is_finite()
                || sample.total_instrumented_seconds <= 0.0
        })
    {
        return Err("instrumented solver timing is invalid".to_owned());
    }
    Ok(())
}

fn row(
    family: &'static str,
    authored_awake_percent: u32,
    result: GpuGeneratedContactBenchmarkOutput,
) -> Result<SleepingCityRowV1, String> {
    validate_result(&result)?;
    let gpu_collision = result
        .gpu_collision_seconds
        .ok_or_else(|| "GPU collision timestamps are unavailable".to_owned())?;
    let gpu_persistence = result
        .gpu_persistence_seconds
        .ok_or_else(|| "GPU persistence timestamps are unavailable".to_owned())?;
    let gpu_end_to_end = result
        .gpu_end_to_end_seconds
        .ok_or_else(|| "GPU end-to-end timestamps are unavailable".to_owned())?;
    let gpu_solver_and_sleeping = gpu_end_to_end
        .iter()
        .zip(&gpu_collision)
        .map(|(total, collision)| (total - collision).max(0.0))
        .collect::<Vec<_>>();
    let overflowed = result.correctness.failure.is_some()
        || result
            .correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted || capacity.overflowed);
    let total_accounted_allocated_bytes = result
        .allocated_bytes
        .checked_add(result.body_allocated_bytes)
        .and_then(|value| value.checked_add(result.active_body_allocated_bytes))
        .and_then(|value| value.checked_add(result.sleep_state_allocated_bytes))
        .and_then(|value| value.checked_add(result.awake_body_allocated_bytes))
        .ok_or_else(|| "sleeping-city allocation total overflowed".to_owned())?;
    let awake_point_fraction = if result.correctness.live_contact_point_count == 0 {
        0.0
    } else {
        f64::from(result.correctness.awake_contact_point_count)
            / f64::from(result.correctness.live_contact_point_count)
    };
    let solver_phases = solver_phase_summary(
        result
            .instrumented_solver_phases
            .ok_or_else(|| "instrumented solver timestamps are unavailable".to_owned())?,
    )?;
    Ok(SleepingCityRowV1 {
        family,
        source_scenario_hash: result.source_scenario_hash,
        body_capacity: result.body_capacity,
        live_bodies: result.body_count,
        authored_awake_percent,
        awake_dynamic_bodies: result.correctness.awake_dynamic_body_count,
        solver_bodies: result.correctness.solver_body_count,
        live_contact_points: result.correctness.live_contact_point_count,
        awake_contact_points: result.correctness.awake_contact_point_count,
        awake_point_fraction,
        dispatches_per_step: result.dispatches_per_step,
        graph_allocated_bytes: result.allocated_bytes,
        total_accounted_allocated_bytes,
        collision_persistence_allocated_bytes: result.collision_persistence_allocated_bytes,
        work: result.correctness.work,
        scheduler_work: result.scheduler_work,
        capacities: result.correctness.capacities,
        gpu_collision_and_persistence: timing(gpu_collision)?,
        gpu_persistence: timing(gpu_persistence)?,
        gpu_solver_and_sleeping: timing(gpu_solver_and_sleeping)?,
        gpu_end_to_end: timing(gpu_end_to_end)?,
        host_completion: timing(result.host_completion_seconds)?,
        cpu_encode_submit: timing(result.cpu_encode_submit_seconds)?,
        solver_phases,
        replay_exact: result.replay_exact,
        overflowed,
        completed: result.replay_exact && !overflowed,
    })
}

fn solver_phase_summary(
    samples: Vec<GpuSolverInstrumentedSampleV4>,
) -> Result<SolverPhaseSummaryV1, String> {
    let series = |select: fn(&GpuSolverInstrumentedSampleV4) -> f64| {
        timing(samples.iter().map(select).collect())
    };
    Ok(SolverPhaseSummaryV1 {
        color_construction: series(|sample| sample.color_construction_seconds)?,
        color_compaction: series(|sample| sample.color_compaction_seconds)?,
        active_color_solve: series(|sample| sample.active_color_solve_seconds)?,
        empty_color_solve: series(|sample| sample.empty_color_solve_seconds)?,
        other_solver: series(|sample| sample.other_solver_seconds)?,
        total_instrumented: series(|sample| sample.total_instrumented_seconds)?,
    })
}

fn dormant_pair_decision(rows: &[SleepingCityRowV1]) -> Result<DormantPairDecisionV1, String> {
    let row = rows
        .iter()
        .find(|row| row.authored_awake_percent == 1)
        .ok_or_else(|| "dormant-pair decision has no 1%-awake row".to_owned())?;
    if row.awake_contact_points == 0 {
        return Err("1%-awake row produced no awake contact work".to_owned());
    }
    let total = row.gpu_end_to_end.p50_seconds;
    let collision_share = row.gpu_collision_and_persistence.p50_seconds / total;
    let persistence_share = row.gpu_persistence.p50_seconds / total;
    let live_to_awake = f64::from(row.live_contact_points) / f64::from(row.awake_contact_points);
    let demonstrated_need = collision_share >= 0.5 && live_to_awake >= 4.0;
    let (decision, rationale) = if demonstrated_need {
        (
            "admit-dormant-pair-follow-up",
            format!(
                "the matched 1%-awake row spends {:.1}% of GPU P50 before solve while retaining {:.1}x as many live as awake points",
                collision_share * 100.0,
                live_to_awake,
            ),
        )
    } else {
        (
            "retain-live-pair-processing",
            format!(
                "the matched 1%-awake row spends {:.1}% of GPU P50 before solve with a {:.1}x live/awake point ratio, below the predeclared dominance gate",
                collision_share * 100.0,
                live_to_awake,
            ),
        )
    };
    Ok(DormantPairDecisionV1 {
        decision,
        basis_row: "matched-sleeping-city-1000-bodies-1-percent-awake",
        collision_and_persistence_p50_share: collision_share,
        persistence_p50_share: persistence_share,
        live_to_awake_point_ratio: live_to_awake,
        demonstrated_need,
        rationale,
    })
}

fn matched_hash(scenarios: &[ScenarioV1]) -> Result<String, String> {
    let mut normalized = scenarios
        .first()
        .cloned()
        .ok_or_else(|| "matched scenario family is empty".to_owned())?;
    normalized.id = "p5.2-sleeping-city-normalized".to_owned();
    normalized.seed = 0;
    for body in &mut normalized.bodies {
        body.sleeping = false;
    }
    let expected = canonical_hash(&normalized).map_err(|error| error.to_string())?;
    for scenario in &scenarios[1..] {
        let mut candidate = scenario.clone();
        candidate.id = normalized.id.clone();
        candidate.seed = 0;
        for body in &mut candidate.bodies {
            body.sleeping = false;
        }
        if canonical_hash(&candidate).map_err(|error| error.to_string())? != expected {
            return Err("sleeping-city rows differ beyond authored awake state".to_owned());
        }
    }
    Ok(expected)
}

fn configuration(result: &GpuGeneratedContactBenchmarkOutput) -> ConfigurationV1 {
    ConfigurationV1 {
        solver_settings: result.correctness.solver_settings,
        solver_execution: result.correctness.solver_execution,
        solver_scheduling_policy: result.correctness.solver_scheduling_policy,
        contact_coefficient_policy: result.correctness.contact_coefficient_policy,
        substep_contact_policy: result.correctness.substep_contact_policy,
        restitution_timing_policy: result.correctness.restitution_timing_policy,
        contact_identity_policy: result.correctness.contact_identity_policy,
    }
}

fn timing(samples: Vec<f64>) -> Result<TimingSeriesV1, String> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("timing series requires finite nonnegative samples".to_owned());
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    Ok(TimingSeriesV1 {
        p50_seconds: percentile(&sorted, 0.50),
        p95_seconds: percentile(&sorted, 0.95),
        p99_seconds: percentile(&sorted, 0.99),
        samples_seconds: samples,
    })
}

fn p50_ratio(rows: &[SleepingCityRowV1]) -> Result<f64, String> {
    let mut values = rows
        .iter()
        .map(|row| row.gpu_end_to_end.p50_seconds)
        .collect::<Vec<_>>();
    if values.is_empty() || values.iter().any(|value| *value <= 0.0) {
        return Err("capacity curve has no positive GPU P50".to_owned());
    }
    values.sort_by(f64::total_cmp);
    Ok(values[values.len() - 1] / values[0])
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let rank = ((sorted.len() as f64 * quantile).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut warmup_steps = 4;
    let mut samples = 20;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--warmup" => warmup_steps = parse_u32(&mut args, "--warmup")?,
            "--samples" => samples = parse_u32(&mut args, "--samples")?,
            _ => return Err(format!("unknown sleeping-city argument {arg}")),
        }
    }
    if warmup_steps == 0 || samples == 0 || warmup_steps + samples >= 30 {
        return Err(
            "sleeping-city warmup and samples must be non-zero and total fewer than 30".to_owned(),
        );
    }
    Ok(Options {
        output: output.ok_or_else(|| "--output is required".to_owned())?,
        warmup_steps,
        samples,
    })
}

fn parse_u32(args: &mut impl Iterator<Item = String>, name: &str) -> Result<u32, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<u32>()
        .map_err(|_| format!("{name} requires an unsigned integer"))
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", args.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}
