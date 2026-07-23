use std::{env, fs, path::PathBuf, process::Command, time::SystemTime};

use physbox_engine::{
    AdapterDiagnostics, GpuGeneratedContactBenchmarkOutput,
    benchmark_gpu_sleeping_city_with_grouping, p5_2_sleeping_city_scenario,
};
use physbox_launch::SleepGrouping;
use serde::Serialize;

const BODY_COUNT: u32 = 1_000;
const BODY_CAPACITY: u32 = 1_024;
const AUTHORED_AWAKE_PERCENT: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    adapter: AdapterDiagnostics,
    scenario_hash: String,
    body_count: u32,
    body_capacity: u32,
    authored_awake_percent: u32,
    sleep_speed_threshold: f32,
    warmup_steps: u32,
    samples: u32,
    timestamp_period_nanoseconds: f32,
    rows: Vec<Row>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    grouping: &'static str,
    awake_dynamic_bodies: u32,
    solver_bodies: u32,
    live_contact_points: u32,
    awake_contact_points: u32,
    dispatches_per_step: u32,
    graph_allocated_bytes: u64,
    total_accounted_allocated_bytes: u64,
    gpu_collision_and_persistence: Timing,
    gpu_solver_and_sleeping: Timing,
    gpu_end_to_end: Timing,
    host_completion: Timing,
    replay_exact: bool,
    overflowed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Timing {
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
        eprintln!("physbox-sleep-grouping-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("sleep-grouping output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain"])?.is_empty();
    if source_dirty {
        return Err("sleep-grouping evidence requires a clean source tree".to_owned());
    }

    let scenario = p5_2_sleeping_city_scenario(BODY_COUNT, AUTHORED_AWAKE_PERCENT)
        .map_err(|error| error.to_string())?;
    let mut outputs = Vec::new();
    for grouping in SleepGrouping::ALL {
        let output = pollster::block_on(benchmark_gpu_sleeping_city_with_grouping(
            &scenario,
            BODY_CAPACITY,
            options.warmup_steps,
            options.samples,
            grouping,
        ))?;
        outputs.push((grouping, output));
    }
    let adapter = outputs[0].1.adapter.clone();
    let adapter_marker = adapter.marker_json();
    let scenario_hash = outputs[0].1.source_scenario_hash.clone();
    let timestamp_period_nanoseconds = outputs[0]
        .1
        .timestamp_period_nanoseconds
        .ok_or_else(|| "sleep-grouping evidence requires GPU timestamps".to_owned())?;
    for (_, output) in &outputs {
        if output.adapter.marker_json() != adapter_marker
            || output.source_scenario_hash != scenario_hash
            || output.body_count != BODY_COUNT
            || output.body_capacity != BODY_CAPACITY
            || output.timestamp_period_nanoseconds != Some(timestamp_period_nanoseconds)
        {
            return Err("sleep-grouping rows are not matched".to_owned());
        }
    }
    let rows = outputs
        .into_iter()
        .map(|(grouping, output)| row(grouping, output))
        .collect::<Result<Vec<_>, _>>()?;
    let artifact = Artifact {
        schema_version: 1,
        artifact_kind: "physbox5-sleep-grouping-evidence-v1",
        run_id: format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_secs(),
            std::process::id()
        ),
        source_revision,
        source_dirty,
        host: "native-gpu",
        adapter,
        scenario_hash,
        body_count: BODY_COUNT,
        body_capacity: BODY_CAPACITY,
        authored_awake_percent: AUTHORED_AWAKE_PERCENT,
        sleep_speed_threshold: 0.02,
        warmup_steps: options.warmup_steps,
        samples: options.samples,
        timestamp_period_nanoseconds,
        rows,
    };
    let bytes = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    fs::write(&options.output, bytes).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native-gpu artifact=sleep-grouping output={}",
        options.output.display()
    );
    Ok(())
}

fn row(grouping: SleepGrouping, output: GpuGeneratedContactBenchmarkOutput) -> Result<Row, String> {
    let gpu_collision = output
        .gpu_collision_seconds
        .ok_or_else(|| "sleep-grouping collision timestamps are unavailable".to_owned())?;
    let gpu_end_to_end = output
        .gpu_end_to_end_seconds
        .ok_or_else(|| "sleep-grouping end-to-end timestamps are unavailable".to_owned())?;
    let gpu_solver_and_sleeping = gpu_end_to_end
        .iter()
        .zip(&gpu_collision)
        .map(|(total, collision)| (total - collision).max(0.0))
        .collect::<Vec<_>>();
    let overflowed = output.correctness.failure.is_some()
        || output
            .correctness
            .capacities
            .iter()
            .any(|capacity| !capacity.accepted || capacity.overflowed);
    if !output.replay_exact || overflowed {
        return Err(format!(
            "{} sleep-grouping row failed replay/capacity",
            grouping.name()
        ));
    }
    let total_accounted_allocated_bytes = output
        .allocated_bytes
        .checked_add(output.body_allocated_bytes)
        .and_then(|value| value.checked_add(output.active_body_allocated_bytes))
        .and_then(|value| value.checked_add(output.sleep_state_allocated_bytes))
        .and_then(|value| value.checked_add(output.awake_body_allocated_bytes))
        .ok_or_else(|| "sleep-grouping allocation total overflowed".to_owned())?;
    Ok(Row {
        grouping: grouping.name(),
        awake_dynamic_bodies: output.correctness.awake_dynamic_body_count,
        solver_bodies: output.correctness.solver_body_count,
        live_contact_points: output.correctness.live_contact_point_count,
        awake_contact_points: output.correctness.awake_contact_point_count,
        dispatches_per_step: output.dispatches_per_step,
        graph_allocated_bytes: output.allocated_bytes,
        total_accounted_allocated_bytes,
        gpu_collision_and_persistence: timing(gpu_collision)?,
        gpu_solver_and_sleeping: timing(gpu_solver_and_sleeping)?,
        gpu_end_to_end: timing(gpu_end_to_end)?,
        host_completion: timing(output.host_completion_seconds)?,
        replay_exact: output.replay_exact,
        overflowed,
    })
}

fn timing(mut samples: Vec<f64>) -> Result<Timing, String> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("sleep-grouping timing series is invalid".to_owned());
    }
    samples.sort_by(f64::total_cmp);
    Ok(Timing {
        p50_seconds: percentile(&samples, 0.50),
        p95_seconds: percentile(&samples, 0.95),
        p99_seconds: percentile(&samples, 0.99),
    })
}

fn percentile(samples: &[f64], percentile: f64) -> f64 {
    let rank = (percentile * samples.len() as f64).ceil() as usize;
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut warmup_steps = 4;
    let mut samples = 20;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--warmup" => warmup_steps = parse_u32(&mut args, "--warmup")?,
            "--samples" => samples = parse_u32(&mut args, "--samples")?,
            _ => return Err(format!("unknown sleep-grouping argument {argument}")),
        }
    }
    if warmup_steps == 0 || samples == 0 || warmup_steps + samples >= 30 {
        return Err(
            "sleep-grouping warmup and samples must be non-zero and total fewer than 30".to_owned(),
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

fn git(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}
