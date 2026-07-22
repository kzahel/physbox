use std::{env, fs, path::PathBuf, process::Command, time::SystemTime};

use physbox_contracts::ScenarioV1;
use physbox_engine::{
    AdapterDiagnostics, GpuContactComponentObservationV1, P5_2GeneratedScenarioKind,
    observe_gpu_contact_components, p5_2_centered_rectangle_stack_scenario,
    p5_2_generated_scenario, p5_2_sleeping_city_scenario, s26_bridge_components_scenario,
    s26_isolated_components_scenario, s26_packed_pile_scenario,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactComponentArtifactV1 {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    adapter: AdapterDiagnostics,
    warmup_samples: u32,
    samples: u32,
    timestamp_status: &'static str,
    rows: Vec<ContactComponentRowV1>,
    capacity_limit: CapacityLimitV1,
    all_exact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactComponentRowV1 {
    label: &'static str,
    family: &'static str,
    component_timing: TimingSummaryV1,
    physics_timing: TimingSummaryV1,
    end_to_end_timing: TimingSummaryV1,
    observer_to_physics_p50_ratio: f64,
    observation: GpuContactComponentObservationV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingSummaryV1 {
    samples_seconds: Vec<f64>,
    p50_seconds: f64,
    p95_seconds: f64,
    p99_seconds: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityLimitV1 {
    largest_integrated_product_reserve_measured: u32,
    omitted_reserve: u32,
    disposition: &'static str,
    reason: &'static str,
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
        eprintln!("physbox-contact-components error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("contact-component output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty();
    if source_dirty {
        return Err("contact-component evidence requires a clean source tree".to_owned());
    }

    let churn = p5_2_generated_scenario(P5_2GeneratedScenarioKind::PairChurn, 16)
        .map_err(|error| error.to_string())?;
    let cases = vec![
        Case {
            label: "isolated-fixed-floor",
            family: "correctness",
            scenario: s26_isolated_components_scenario(1).map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "eight-body-chain",
            family: "correctness",
            scenario: p5_2_centered_rectangle_stack_scenario(8, 1)
                .map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "two-piles-one-floor",
            family: "split",
            scenario: s26_bridge_components_scenario(false, 1)
                .map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "dynamic-bridge",
            family: "arch-bridge",
            scenario: s26_bridge_components_scenario(true, 1).map_err(|error| error.to_string())?,
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "pair-churn-before-impact",
            family: "churn",
            scenario: churn.clone(),
            capacity: 64,
            completed_steps: 1,
        },
        Case {
            label: "pair-churn-after-impact",
            family: "churn",
            scenario: churn,
            capacity: 64,
            completed_steps: 16,
        },
        Case {
            label: "packed-pile-100",
            family: "dense-pile",
            scenario: s26_packed_pile_scenario(100, 1).map_err(|error| error.to_string())?,
            capacity: 256,
            completed_steps: 1,
        },
        Case {
            label: "packed-pile-1000",
            family: "dense-pile",
            scenario: s26_packed_pile_scenario(1_000, 1).map_err(|error| error.to_string())?,
            capacity: 1_024,
            completed_steps: 1,
        },
        Case {
            label: "sleeping-city-1000",
            family: "sleeping-city",
            scenario: p5_2_sleeping_city_scenario(1_000, 1).map_err(|error| error.to_string())?,
            capacity: 1_024,
            completed_steps: 1,
        },
        Case {
            label: "sleeping-city-200-reserve-10000",
            family: "capacity",
            scenario: p5_2_sleeping_city_scenario(200, 1).map_err(|error| error.to_string())?,
            capacity: 10_000,
            completed_steps: 1,
        },
    ];

    let mut observations = Vec::with_capacity(cases.len());
    for case in cases {
        let observation = pollster::block_on(observe_gpu_contact_components(
            &case.scenario,
            case.capacity,
            case.completed_steps,
            options.warmup_samples,
            options.samples,
        ))?;
        if !observation.exact || !observation.replay_exact {
            return Err(format!("{} did not produce exact replay", case.label));
        }
        let component_timing = timing(
            observation
                .gpu_component_seconds
                .clone()
                .ok_or_else(|| format!("{} requires GPU component timestamps", case.label))?,
        )?;
        let physics_timing = timing(
            observation
                .gpu_physics_seconds
                .clone()
                .ok_or_else(|| format!("{} requires GPU physics timestamps", case.label))?,
        )?;
        let end_to_end_timing = timing(
            observation
                .gpu_end_to_end_seconds
                .clone()
                .ok_or_else(|| format!("{} requires GPU end-to-end timestamps", case.label))?,
        )?;
        let observer_to_physics_p50_ratio =
            component_timing.p50_seconds / physics_timing.p50_seconds;
        observations.push(ContactComponentRowV1 {
            label: case.label,
            family: case.family,
            component_timing,
            physics_timing,
            end_to_end_timing,
            observer_to_physics_p50_ratio,
            observation,
        });
    }
    let expected_adapter = observations[0].observation.adapter.marker_json();
    if observations
        .iter()
        .any(|row| row.observation.adapter.marker_json() != expected_adapter)
    {
        return Err("contact-component rows changed GPU adapter".to_owned());
    }
    let period = observations[0]
        .observation
        .timestamp_period_nanoseconds
        .ok_or_else(|| "contact-component evidence requires GPU timestamps".to_owned())?;
    if observations
        .iter()
        .any(|row| row.observation.timestamp_period_nanoseconds != Some(period))
    {
        return Err("contact-component rows changed timestamp period".to_owned());
    }

    let run_id = format!(
        "s26-contact-components-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs()
    );
    let artifact = ContactComponentArtifactV1 {
        schema_version: 1,
        artifact_kind: "physbox5-s26-contact-component-observation",
        run_id: run_id.clone(),
        source_revision,
        source_dirty,
        host: "native",
        adapter: observations[0].observation.adapter.clone(),
        warmup_samples: options.warmup_samples,
        samples: options.samples,
        timestamp_status: "available",
        rows: observations,
        capacity_limit: CapacityLimitV1 {
            largest_integrated_product_reserve_measured: 10_000,
            omitted_reserve: 100_000,
            disposition: "not-applicable-to-current-integrated-product-graph",
            reason: "the maintained integrated world-capacity presets end at 10000; the 100000 broadphase-only lane is not an exact-contact graph",
        },
        all_exact: true,
    };
    let encoded = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&options.output, encoded).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s26-contact-components run_id={} rows={} output={}",
        run_id,
        artifact.rows.len(),
        options.output.display()
    );
    Ok(())
}

fn timing(mut samples_seconds: Vec<f64>) -> Result<TimingSummaryV1, String> {
    if samples_seconds.is_empty()
        || samples_seconds
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0)
    {
        return Err("contact-component timing samples are missing or invalid".to_owned());
    }
    let mut sorted = samples_seconds.clone();
    sorted.sort_by(f64::total_cmp);
    Ok(TimingSummaryV1 {
        p50_seconds: percentile(&sorted, 0.50),
        p95_seconds: percentile(&sorted, 0.95),
        p99_seconds: percentile(&sorted, 0.99),
        samples_seconds: std::mem::take(&mut samples_seconds),
    })
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * quantile).ceil() as usize;
    sorted[index]
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut warmup_samples = 3;
    let mut samples = 20;
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
