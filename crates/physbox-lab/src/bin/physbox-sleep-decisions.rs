use std::{env, fs, path::PathBuf, process::Command, time::SystemTime};

use physbox_contracts::ScenarioV1;
use physbox_engine::{
    AdapterDiagnostics, GpuSleepDecisionObservationV1, GpuSleepDecisionTimingV1,
    benchmark_gpu_sleep_decision_observer, observe_gpu_sleep_decisions,
    p5_2_centered_rectangle_stack_scenario, s27_long_path_wake_scenario, s27_poured_pile_scenario,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepDecisionArtifactV1 {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    adapter: AdapterDiagnostics,
    warmup_samples: u32,
    samples: u32,
    rows: Vec<SleepDecisionRowV1>,
    all_exact: bool,
    all_applied_replay_exact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepDecisionRowV1 {
    label: &'static str,
    family: &'static str,
    summary: SleepDecisionSummaryV1,
    component_timing: TimingSummaryV1,
    reduction_timing: TimingSummaryV1,
    observer_timing: TimingSummaryV1,
    observation: GpuSleepDecisionObservationV1,
    timing: GpuSleepDecisionTimingV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SleepDecisionSummaryV1 {
    exact_only_wake_steps: u32,
    exact_only_wake_body_events: u64,
    peak_exact_only_wake_bodies: u32,
    bounded_only_admission_steps: u32,
    bounded_only_admission_body_events: u64,
    peak_bounded_only_admission_bodies: u32,
    projected_additional_exact_awake_body_events: u64,
    first_bounded_all_sleep_step: Option<u32>,
    last_local_wake_root_step: Option<u32>,
    peak_component_count: u32,
    peak_largest_component_size: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingSummaryV1 {
    samples_seconds: Vec<f64>,
    p50_seconds: f64,
    p95_seconds: f64,
    p99_seconds: f64,
}

struct Options {
    output: PathBuf,
    warmup_samples: u32,
    samples: u32,
}

struct Case {
    label: &'static str,
    family: &'static str,
    scenario: ScenarioV1,
    capacity: u32,
    completed_steps: u32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-sleep-decisions error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("sleep-decision output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty();
    if source_dirty {
        return Err("sleep-decision evidence requires a clean source tree".to_owned());
    }
    let cases = vec![
        Case {
            label: "long-path-wake-control",
            family: "synthetic-radius",
            scenario: s27_long_path_wake_scenario(1).map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "centered-tower-2",
            family: "time-evolved-sleep-control",
            scenario: p5_2_centered_rectangle_stack_scenario(2, 600)
                .map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 600,
        },
        Case {
            label: "centered-tower-8",
            family: "time-evolved-tower",
            scenario: p5_2_centered_rectangle_stack_scenario(8, 600)
                .map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 600,
        },
        Case {
            label: "centered-tower-14",
            family: "time-evolved-tower",
            scenario: p5_2_centered_rectangle_stack_scenario(14, 1_200)
                .map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1_200,
        },
        Case {
            label: "poured-pile-100",
            family: "time-evolved-pile",
            scenario: s27_poured_pile_scenario(1_200).map_err(|error| error.to_string())?,
            capacity: 256,
            completed_steps: 1_200,
        },
    ];

    let mut rows = Vec::with_capacity(cases.len());
    for case in cases {
        let observation = pollster::block_on(observe_gpu_sleep_decisions(
            &case.scenario,
            case.capacity,
            case.completed_steps,
        ))?;
        let timing = pollster::block_on(benchmark_gpu_sleep_decision_observer(
            &case.scenario,
            case.capacity,
            case.completed_steps,
            options.warmup_samples,
            options.samples,
        ))?;
        if observation.adapter.marker_json() != timing.adapter.marker_json() {
            return Err(format!(
                "{} changed adapter between trace and timing",
                case.label
            ));
        }
        if observation.source_scenario_hash != timing.source_scenario_hash {
            return Err(format!(
                "{} changed scenario between trace and timing",
                case.label
            ));
        }
        rows.push(SleepDecisionRowV1 {
            label: case.label,
            family: case.family,
            summary: summarize(&observation),
            component_timing: timing_summary(timing.gpu_component_seconds.clone())?,
            reduction_timing: timing_summary(timing.gpu_reduction_seconds.clone())?,
            observer_timing: timing_summary(timing.gpu_observer_seconds.clone())?,
            observation,
            timing,
        });
    }
    let adapter = rows[0].observation.adapter.clone();
    if rows
        .iter()
        .any(|row| row.observation.adapter.marker_json() != adapter.marker_json())
    {
        return Err("sleep-decision rows changed GPU adapter".to_owned());
    }
    let all_exact = rows.iter().all(|row| row.observation.exact);
    let all_applied_replay_exact = rows.iter().all(|row| row.observation.applied_replay_exact);
    if !all_exact || !all_applied_replay_exact {
        return Err("sleep-decision evidence failed correctness or replay".to_owned());
    }
    let run_id = format!(
        "s27-sleep-decisions-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs()
    );
    let artifact = SleepDecisionArtifactV1 {
        schema_version: 1,
        artifact_kind: "physbox5-s27-exact-sleep-decision-observation",
        run_id: run_id.clone(),
        source_revision,
        source_dirty,
        host: "native",
        adapter,
        warmup_samples: options.warmup_samples,
        samples: options.samples,
        rows,
        all_exact,
        all_applied_replay_exact,
    };
    let encoded = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&options.output, encoded).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s27-sleep-decisions run_id={} rows={} output={}",
        run_id,
        artifact.rows.len(),
        options.output.display()
    );
    Ok(())
}

fn summarize(observation: &GpuSleepDecisionObservationV1) -> SleepDecisionSummaryV1 {
    let exact_only_wake_steps = observation
        .steps
        .iter()
        .filter(|step| step.exact_only_wake_bodies != 0)
        .count() as u32;
    let exact_only_wake_body_events = observation
        .steps
        .iter()
        .map(|step| u64::from(step.exact_only_wake_bodies))
        .sum();
    let bounded_only_admission_steps = observation
        .steps
        .iter()
        .filter(|step| step.bounded_only_admission_bodies != 0)
        .count() as u32;
    let bounded_only_admission_body_events = observation
        .steps
        .iter()
        .map(|step| u64::from(step.bounded_only_admission_bodies))
        .sum();
    let exact_only_new_wake_body_events = observation
        .steps
        .iter()
        .map(|step| u64::from(step.exact_only_new_wake_bodies))
        .sum::<u64>();
    SleepDecisionSummaryV1 {
        exact_only_wake_steps,
        exact_only_wake_body_events,
        peak_exact_only_wake_bodies: observation
            .steps
            .iter()
            .map(|step| step.exact_only_wake_bodies)
            .max()
            .unwrap_or(0),
        bounded_only_admission_steps,
        bounded_only_admission_body_events,
        peak_bounded_only_admission_bodies: observation
            .steps
            .iter()
            .map(|step| step.bounded_only_admission_bodies)
            .max()
            .unwrap_or(0),
        projected_additional_exact_awake_body_events: exact_only_new_wake_body_events
            + bounded_only_admission_body_events,
        first_bounded_all_sleep_step: observation.steps.iter().find_map(|step| {
            (step.awake_before_admission == step.bounded_admission_bodies).then_some(step.step)
        }),
        last_local_wake_root_step: observation
            .steps
            .iter()
            .rev()
            .find_map(|step| (step.local_wake_root_bodies != 0).then_some(step.step)),
        peak_component_count: observation
            .steps
            .iter()
            .map(|step| step.component_count)
            .max()
            .unwrap_or(0),
        peak_largest_component_size: observation
            .steps
            .iter()
            .map(|step| step.largest_component_size)
            .max()
            .unwrap_or(0),
    }
}

fn timing_summary(samples_seconds: Vec<f64>) -> Result<TimingSummaryV1, String> {
    if samples_seconds.is_empty()
        || samples_seconds
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0)
    {
        return Err("sleep-decision timing samples are missing or invalid".to_owned());
    }
    let mut sorted = samples_seconds.clone();
    sorted.sort_by(f64::total_cmp);
    Ok(TimingSummaryV1 {
        p50_seconds: percentile(&sorted, 0.50),
        p95_seconds: percentile(&sorted, 0.95),
        p99_seconds: percentile(&sorted, 0.99),
        samples_seconds,
    })
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * quantile).ceil() as usize;
    sorted[index]
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut warmup_samples = 1;
    let mut samples = 5;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        match argument.as_str() {
            "--output" => output = Some(PathBuf::from(value)),
            "--warmup" => {
                warmup_samples = value
                    .parse()
                    .map_err(|_| "invalid --warmup value".to_owned())?
            }
            "--samples" => {
                samples = value
                    .parse()
                    .map_err(|_| "invalid --samples value".to_owned())?
            }
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    if samples == 0 {
        return Err("--samples must be non-zero".to_owned());
    }
    Ok(Options {
        output: output.ok_or_else(|| "missing --output".to_owned())?,
        warmup_samples,
        samples,
    })
}

fn git(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| format!("run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}
