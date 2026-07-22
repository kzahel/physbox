use std::{env, fs, path::PathBuf, process::Command, time::SystemTime};

use physbox_contracts::{
    CollisionCapacityV1, CollisionWorkV1, ContactIdentityPolicyV1, SolverSettingsV1,
};
use physbox_engine::{
    AdapterDiagnostics, GpuContactCoefficientPolicyV1, GpuGeneratedContactBenchmarkOutput,
    GpuRestitutionTimingPolicyV1, GpuSchedulerWorkVectorV1, GpuSolverExecutionV4,
    GpuSolverSchedulingPolicyV1, GpuSubstepContactPolicyV1, benchmark_gpu_sparse_capacity,
    p5_2_generated_performance_scenario,
};
use serde::Serialize;

const SPARSE_CAPACITIES: [u32; 4] = [64, 256, 1_024, 10_000];
const DENSE_CONTROL_BODIES: u32 = 64;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SparseCapacityArtifactV1 {
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
    configuration: SparseCapacityConfigurationV1,
    sparse_scenario_hash: String,
    sparse_rows: Vec<SparseCapacityRowV1>,
    dense_control_scenario_hash: String,
    dense_control: SparseCapacityRowV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SparseCapacityConfigurationV1 {
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
struct SparseCapacityRowV1 {
    body_capacity: u32,
    live_bodies: u32,
    dynamic_bodies: u32,
    fixed_bodies: u32,
    dispatches_per_step: u32,
    graph_allocated_bytes: u64,
    body_allocated_bytes: u64,
    active_body_allocated_bytes: u64,
    total_accounted_allocated_bytes: u64,
    collision_persistence_allocated_bytes: u64,
    work: CollisionWorkV1,
    scheduler_work: Option<GpuSchedulerWorkVectorV1>,
    capacities: Vec<CollisionCapacityV1>,
    gpu_collision: Option<TimingSeriesV1>,
    gpu_persistence: Option<TimingSeriesV1>,
    gpu_end_to_end: Option<TimingSeriesV1>,
    host_completion: TimingSeriesV1,
    cpu_encode_submit: TimingSeriesV1,
    timestamp_period_nanoseconds: Option<f32>,
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

struct Options {
    output: PathBuf,
    warmup_steps: u32,
    samples: u32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-sparse-capacity error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("sparse-capacity output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain"])?.is_empty();
    if source_dirty {
        return Err("sparse-capacity evidence requires a clean source tree".to_owned());
    }

    let sparse_scenario =
        p5_2_generated_performance_scenario(2).map_err(|error| error.to_string())?;
    let mut sparse_results = Vec::with_capacity(SPARSE_CAPACITIES.len());
    for body_capacity in SPARSE_CAPACITIES {
        sparse_results.push(pollster::block_on(benchmark_gpu_sparse_capacity(
            &sparse_scenario,
            body_capacity,
            options.warmup_steps,
            options.samples,
        ))?);
    }
    validate_sparse_results(&sparse_results)?;

    let dense_scenario = p5_2_generated_performance_scenario(DENSE_CONTROL_BODIES)
        .map_err(|error| error.to_string())?;
    let dense_result = pollster::block_on(benchmark_gpu_sparse_capacity(
        &dense_scenario,
        DENSE_CONTROL_BODIES,
        options.warmup_steps,
        options.samples,
    ))?;
    validate_result(&dense_result)?;
    let sparse_work = &sparse_results[0].correctness.work;
    if dense_result.correctness.work.bodies <= sparse_work.bodies
        || dense_result.correctness.work.candidate_pairs <= sparse_work.candidate_pairs
        || dense_result.correctness.work.contact_points <= sparse_work.contact_points
    {
        return Err("dense control did not increase live collision/contact work".to_owned());
    }

    let expected_adapter = sparse_results[0].adapter.marker_json();
    if dense_result.adapter.marker_json() != expected_adapter {
        return Err("dense control used a different adapter".to_owned());
    }
    let expected_configuration = configuration(&sparse_results[0]);
    if configuration(&dense_result) != expected_configuration {
        return Err("dense control used a different physics/scheduling configuration".to_owned());
    }

    let adapter = sparse_results[0].adapter.clone();
    let sparse_scenario_hash = sparse_results[0].source_scenario_hash.clone();
    let dense_control_scenario_hash = dense_result.source_scenario_hash.clone();
    let sparse_rows = sparse_results
        .into_iter()
        .map(row)
        .collect::<Result<Vec<_>, _>>()?;
    let dense_control = row(dense_result)?;
    let run_id = format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs(),
        std::process::id(),
    );
    let artifact = SparseCapacityArtifactV1 {
        schema_version: 1,
        artifact_kind: "physbox5-active-work-sparse-capacity-v1",
        run_id,
        source_revision,
        source_dirty,
        host: "native-gpu",
        pacing_mode: "explicit-step-throughput",
        adapter,
        warmup_steps: options.warmup_steps,
        samples: options.samples,
        configuration: expected_configuration,
        sparse_scenario_hash,
        sparse_rows,
        dense_control_scenario_hash,
        dense_control,
    };
    let staging = options.output.with_extension("json.staging");
    if staging.exists() {
        return Err("sparse-capacity staging output must be fresh".to_owned());
    }
    fs::write(
        &staging,
        serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&staging, &options.output).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_SPARSE_CAPACITY_SUCCESS run_id={} rows={} output={}",
        artifact.run_id,
        artifact.sparse_rows.len(),
        options.output.display(),
    );
    Ok(())
}

fn validate_sparse_results(results: &[GpuGeneratedContactBenchmarkOutput]) -> Result<(), String> {
    if results.len() != SPARSE_CAPACITIES.len() {
        return Err("sparse-capacity probe did not produce the frozen four rows".to_owned());
    }
    let first = &results[0];
    validate_result(first)?;
    let expected_hash = &first.source_scenario_hash;
    let expected_adapter = first.adapter.marker_json();
    let expected_configuration = configuration(first);
    let expected_work = &first.correctness.work;
    let expected_capacity_classes = capacity_classes(&first.correctness.capacities);
    let expected_timestamp_presence = first.gpu_end_to_end_seconds.is_some();
    for (index, result) in results.iter().enumerate() {
        validate_result(result)?;
        if result.body_capacity != SPARSE_CAPACITIES[index]
            || &result.source_scenario_hash != expected_hash
            || result.adapter.marker_json() != expected_adapter
            || configuration(result) != expected_configuration
            || &result.correctness.work != expected_work
            || result.correctness.bodies != first.correctness.bodies
            || capacity_classes(&result.correctness.capacities) != expected_capacity_classes
            || result.gpu_end_to_end_seconds.is_some() != expected_timestamp_presence
        {
            return Err(format!(
                "sparse-capacity row {} changed scene, adapter, configuration, live work, or timestamp scope",
                result.body_capacity
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
        if samples.is_some_and(|samples| {
            samples.len() != result.samples as usize
                || samples
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0)
        }) {
            return Err(format!(
                "capacity {} has invalid {label} samples",
                result.body_capacity
            ));
        }
    }
    Ok(())
}

fn configuration(result: &GpuGeneratedContactBenchmarkOutput) -> SparseCapacityConfigurationV1 {
    SparseCapacityConfigurationV1 {
        solver_settings: result.correctness.solver_settings,
        solver_execution: result.correctness.solver_execution,
        solver_scheduling_policy: result.correctness.solver_scheduling_policy,
        contact_coefficient_policy: result.correctness.contact_coefficient_policy,
        substep_contact_policy: result.correctness.substep_contact_policy,
        restitution_timing_policy: result.correctness.restitution_timing_policy,
        contact_identity_policy: result.correctness.contact_identity_policy,
    }
}

fn capacity_classes(capacities: &[CollisionCapacityV1]) -> Vec<&str> {
    capacities
        .iter()
        .map(|capacity| capacity.class.as_str())
        .collect()
}

fn row(result: GpuGeneratedContactBenchmarkOutput) -> Result<SparseCapacityRowV1, String> {
    let total_accounted_allocated_bytes = result
        .allocated_bytes
        .checked_add(result.body_allocated_bytes)
        .and_then(|value| value.checked_add(result.active_body_allocated_bytes))
        .ok_or_else(|| "sparse-capacity allocation total overflowed".to_owned())?;
    let overflowed = result.correctness.failure.is_some()
        || result
            .correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted || capacity.overflowed);
    let fixed_bodies = result
        .correctness
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() == 0.0)
        .count() as u32;
    let dynamic_bodies = result.body_count - fixed_bodies;
    Ok(SparseCapacityRowV1 {
        body_capacity: result.body_capacity,
        live_bodies: result.body_count,
        dynamic_bodies,
        fixed_bodies,
        dispatches_per_step: result.dispatches_per_step,
        graph_allocated_bytes: result.allocated_bytes,
        body_allocated_bytes: result.body_allocated_bytes,
        active_body_allocated_bytes: result.active_body_allocated_bytes,
        total_accounted_allocated_bytes,
        collision_persistence_allocated_bytes: result.collision_persistence_allocated_bytes,
        work: result.correctness.work,
        scheduler_work: result.scheduler_work,
        capacities: result.correctness.capacities,
        gpu_collision: optional_timing(result.gpu_collision_seconds)?,
        gpu_persistence: optional_timing(result.gpu_persistence_seconds)?,
        gpu_end_to_end: optional_timing(result.gpu_end_to_end_seconds)?,
        host_completion: timing(result.host_completion_seconds)?,
        cpu_encode_submit: timing(result.cpu_encode_submit_seconds)?,
        timestamp_period_nanoseconds: result.timestamp_period_nanoseconds,
        replay_exact: result.replay_exact,
        overflowed,
        completed: result.replay_exact && !overflowed,
    })
}

fn optional_timing(samples: Option<Vec<f64>>) -> Result<Option<TimingSeriesV1>, String> {
    samples.map(timing).transpose()
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

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let rank = ((sorted.len() as f64 * quantile).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut warmup_steps = 5;
    let mut samples = 20;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--warmup" => warmup_steps = parse_u32(&mut args, "--warmup")?,
            "--samples" => samples = parse_u32(&mut args, "--samples")?,
            _ => return Err(format!("unknown sparse-capacity argument {arg}")),
        }
    }
    if warmup_steps == 0 || samples == 0 {
        return Err("sparse-capacity warmup and samples must be non-zero".to_owned());
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
