use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use physbox_contracts::sha256_bytes;
use physbox_engine::{
    AdapterDiagnostics, GpuAuthoredGatherBenchmarkOutput, GpuGatherTopology,
    GpuGeneratedContactBenchmarkOutput, GpuSolverExecutionV4, GpuSolverInstrumentedSampleV4,
    GpuSolverSchedulingPolicyV1, P5_2_SELECTED_SOLVER_ITERATIONS, authored_chain_scenario,
    benchmark_gpu_authored_solver_experiment, benchmark_gpu_generated_contacts,
    p5_2_generated_performance_scenario,
};
use physbox_lab::scheduler_benchmark::{
    SCHEDULER_AUTHORED_CONTACT_COUNTS, SCHEDULER_BENCHMARK_CONTRACT_VERSION,
    SCHEDULER_SPARSE_BODY_COUNTS, SchedulerBenchmarkRowV1, SchedulerBenchmarkScopeV1,
    SchedulerMarginalSlopeV1, SchedulerTimingSeriesV1, scheduler_marginal_slope,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulerBenchmarkManifestV1 {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    adapter: AdapterDiagnostics,
    solver_execution: GpuSolverExecutionV4,
    solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    rows: Vec<SchedulerBenchmarkRowV1>,
    slopes: Vec<SchedulerMarginalSlopeV1>,
    selection_made: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
}

struct Arguments {
    output: PathBuf,
    run_id: String,
    warmup: u32,
    samples: u32,
    require_clean: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-scheduler-benchmark error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    if arguments.warmup == 0 || arguments.samples == 0 {
        return Err("scheduler benchmark requires nonzero warmup and samples".to_owned());
    }
    if arguments.output.exists() {
        return Err("scheduler benchmark output must be fresh".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if staging.exists() {
        return Err("scheduler benchmark staging path must be fresh".to_owned());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("scheduler benchmark requires a clean source tree".to_owned());
    }

    let mut rows = Vec::new();
    let mut adapter = None;
    for body_count in SCHEDULER_SPARSE_BODY_COUNTS {
        let scenario =
            p5_2_generated_performance_scenario(body_count).map_err(|error| error.to_string())?;
        let benchmark = pollster::block_on(benchmark_gpu_generated_contacts(
            &scenario,
            arguments.warmup,
            arguments.samples,
        ))?;
        same_adapter(&mut adapter, &benchmark.adapter)?;
        rows.push(generated_row(benchmark)?);
    }
    for contact_count in SCHEDULER_AUTHORED_CONTACT_COUNTS {
        let mut scenario =
            authored_chain_scenario(contact_count + 1, 0x5c4c_0000 + u64::from(contact_count))
                .map_err(|error| error.to_string())?;
        scenario.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
        let benchmark = pollster::block_on(benchmark_gpu_authored_solver_experiment(
            &scenario,
            arguments.warmup,
            arguments.samples,
            GpuGatherTopology::StableBodyEdge,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::STANDARD,
        ))?;
        same_adapter(&mut adapter, &benchmark.adapter)?;
        rows.push(authored_row(benchmark)?);
    }
    for row in &rows {
        row.validate()?;
    }
    let slopes = vec![
        scheduler_marginal_slope(&rows[0], &rows[1])?,
        scheduler_marginal_slope(&rows[1], &rows[2])?,
        scheduler_marginal_slope(&rows[3], &rows[4])?,
    ];
    let manifest = SchedulerBenchmarkManifestV1 {
        schema_version: SCHEDULER_BENCHMARK_CONTRACT_VERSION,
        artifact_kind: "physbox5-scheduler-benchmark-v1",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_digest,
        adapter: adapter.ok_or_else(|| "scheduler benchmark omitted its adapter".to_owned())?,
        solver_execution: GpuSolverExecutionV4::SELECTED,
        solver_scheduling_policy: GpuSolverSchedulingPolicyV1::STANDARD,
        rows,
        slopes,
        selection_made: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(&staging, bytes).map_err(|error| error.to_string())?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    println!("PHYSBOX5_VALIDATION_SUCCESS");
    Ok(())
}

fn generated_row(
    benchmark: GpuGeneratedContactBenchmarkOutput,
) -> Result<SchedulerBenchmarkRowV1, String> {
    let work = benchmark
        .scheduler_work
        .ok_or_else(|| "generated benchmark omitted scheduler work".to_owned())?;
    let timing = timing_series(
        benchmark.gpu_end_to_end_seconds,
        benchmark.host_completion_seconds,
        benchmark.cpu_encode_submit_seconds,
        benchmark.instrumented_solver_phases,
    );
    Ok(SchedulerBenchmarkRowV1 {
        contract_version: SCHEDULER_BENCHMARK_CONTRACT_VERSION,
        case_id: format!("generated-sparse-{}", benchmark.body_count),
        scope: SchedulerBenchmarkScopeV1::GeneratedCompleteStep,
        source_scenario_hash: benchmark.source_scenario_hash,
        body_count: benchmark.body_count,
        authored_contact_count: None,
        warmup_steps: benchmark.warmup_steps,
        samples: benchmark.samples,
        work,
        timing,
        dispatches_per_step: benchmark.dispatches_per_step,
        allocated_bytes: benchmark.allocated_bytes,
        replay_exact: benchmark.replay_exact,
        capacity_accepted: benchmark.correctness.failure.is_none()
            && benchmark
                .correctness
                .capacities
                .iter()
                .all(|capacity| capacity.accepted),
    })
}

fn authored_row(
    benchmark: GpuAuthoredGatherBenchmarkOutput,
) -> Result<SchedulerBenchmarkRowV1, String> {
    let work = benchmark
        .scheduler_work
        .ok_or_else(|| "authored benchmark omitted scheduler work".to_owned())?;
    let timing = timing_series(
        benchmark.gpu_total_seconds,
        benchmark.host_completion_seconds,
        benchmark.cpu_encode_submit_seconds,
        benchmark.instrumented_solver_phases,
    );
    Ok(SchedulerBenchmarkRowV1 {
        contract_version: SCHEDULER_BENCHMARK_CONTRACT_VERSION,
        case_id: format!("authored-contact-rich-{}", benchmark.contact_points),
        scope: SchedulerBenchmarkScopeV1::AuthoredSolverStep,
        source_scenario_hash: benchmark.source_scenario_hash,
        body_count: benchmark.body_count,
        authored_contact_count: Some(benchmark.contact_points),
        warmup_steps: benchmark.warmup_steps,
        samples: benchmark.samples,
        work,
        timing,
        dispatches_per_step: benchmark.dispatches_per_step,
        allocated_bytes: benchmark.allocated_bytes,
        replay_exact: benchmark.replay_exact,
        capacity_accepted: benchmark
            .capacities
            .iter()
            .all(|capacity| !capacity.overflowed),
    })
}

fn timing_series(
    ordinary_gpu_seconds: Option<Vec<f64>>,
    host_completion_seconds: Vec<f64>,
    cpu_encode_submit_seconds: Vec<f64>,
    phases: Option<Vec<GpuSolverInstrumentedSampleV4>>,
) -> SchedulerTimingSeriesV1 {
    let extract = |select: fn(&GpuSolverInstrumentedSampleV4) -> f64| {
        phases
            .as_ref()
            .map(|samples| samples.iter().map(select).collect())
    };
    SchedulerTimingSeriesV1 {
        ordinary_gpu_seconds,
        host_completion_seconds,
        cpu_encode_submit_seconds,
        color_construction_seconds: extract(|sample| sample.color_construction_seconds),
        color_compaction_seconds: extract(|sample| sample.color_compaction_seconds),
        active_color_solve_seconds: extract(|sample| sample.active_color_solve_seconds),
        empty_color_solve_seconds: extract(|sample| sample.empty_color_solve_seconds),
        other_solver_seconds: extract(|sample| sample.other_solver_seconds),
    }
}

fn same_adapter(
    selected: &mut Option<AdapterDiagnostics>,
    candidate: &AdapterDiagnostics,
) -> Result<(), String> {
    if let Some(selected) = selected {
        let selected = serde_json::to_vec(selected).map_err(|error| error.to_string())?;
        let candidate = serde_json::to_vec(candidate).map_err(|error| error.to_string())?;
        if selected != candidate {
            return Err("scheduler benchmark adapter changed between rows".to_owned());
        }
    } else {
        *selected = Some(candidate.clone());
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
    let mut warmup = 4;
    let mut samples = 9;
    let mut require_clean = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => output = arguments.next().map(PathBuf::from),
            "--run-id" => run_id = arguments.next(),
            "--warmup" => {
                warmup = arguments
                    .next()
                    .ok_or_else(|| "--warmup requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--warmup must be an integer".to_owned())?;
            }
            "--samples" => {
                samples = arguments
                    .next()
                    .ok_or_else(|| "--samples requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--samples must be an integer".to_owned())?;
            }
            "--require-clean" => require_clean = true,
            _ => return Err(format!("unknown scheduler benchmark argument {argument}")),
        }
    }
    Ok(Arguments {
        output: output.ok_or_else(|| "--output is required".to_owned())?,
        run_id: run_id.ok_or_else(|| "--run-id is required".to_owned())?,
        warmup,
        samples,
        require_clean,
    })
}
