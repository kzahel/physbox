use std::{env, fs, path::PathBuf, process::Command};

use physbox_contracts::{CollisionCapacityV1, CollisionWorkV1};
use physbox_engine::{
    AdapterDiagnostics, benchmark_gpu_generated_contacts, p5_2_generated_performance_scenario,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductScaleArtifactV1 {
    version: u32,
    artifact_kind: &'static str,
    source_revision: String,
    source_dirty: bool,
    adapter: AdapterDiagnostics,
    warmup_steps: u32,
    samples: u32,
    rows: Vec<ProductScaleRowV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductScaleRowV1 {
    body_count: u32,
    source_scenario_hash: String,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    collision_persistence_allocated_bytes: u64,
    work: CollisionWorkV1,
    capacities: Vec<CollisionCapacityV1>,
    gpu_collision_seconds: Option<Vec<f64>>,
    gpu_persistence_seconds: Option<Vec<f64>>,
    gpu_end_to_end_seconds: Option<Vec<f64>>,
    host_completion_seconds: Vec<f64>,
    cpu_encode_submit_seconds: Vec<f64>,
    replay_exact: bool,
}

struct Options {
    output: PathBuf,
    counts: Vec<u32>,
    warmup_steps: u32,
    samples: u32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-product-scale error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    if options.output.exists() {
        return Err("product-scale output must be fresh".to_owned());
    }
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain"])?.is_empty();
    if source_dirty {
        return Err("product-scale evidence requires a clean source tree".to_owned());
    }

    let mut adapter = None;
    let mut rows = Vec::with_capacity(options.counts.len());
    for body_count in &options.counts {
        let scenario =
            p5_2_generated_performance_scenario(*body_count).map_err(|error| error.to_string())?;
        let result = pollster::block_on(benchmark_gpu_generated_contacts(
            &scenario,
            options.warmup_steps,
            options.samples,
        ))?;
        if result.correctness.failure.is_some()
            || !result.replay_exact
            || result
                .correctness
                .capacities
                .iter()
                .any(|capacity| !capacity.accepted)
        {
            return Err(format!(
                "{body_count}-body integrated product row failed correctness/capacity/replay"
            ));
        }
        if adapter
            .as_ref()
            .is_some_and(|expected: &AdapterDiagnostics| {
                expected.marker_json() != result.adapter.marker_json()
            })
        {
            return Err("product-scale rows used different adapters".to_owned());
        }
        adapter.get_or_insert_with(|| result.adapter.clone());
        rows.push(ProductScaleRowV1 {
            body_count: *body_count,
            source_scenario_hash: result.source_scenario_hash,
            dispatches_per_step: result.dispatches_per_step,
            allocated_bytes: result.allocated_bytes,
            collision_persistence_allocated_bytes: result.collision_persistence_allocated_bytes,
            work: result.correctness.work,
            capacities: result.correctness.capacities,
            gpu_collision_seconds: result.gpu_collision_seconds,
            gpu_persistence_seconds: result.gpu_persistence_seconds,
            gpu_end_to_end_seconds: result.gpu_end_to_end_seconds,
            host_completion_seconds: result.host_completion_seconds,
            cpu_encode_submit_seconds: result.cpu_encode_submit_seconds,
            replay_exact: result.replay_exact,
        });
    }
    let artifact = ProductScaleArtifactV1 {
        version: 1,
        artifact_kind: "physbox5-integrated-product-scale-v1",
        source_revision,
        source_dirty,
        adapter: adapter.ok_or_else(|| "product-scale run produced no rows".to_owned())?,
        warmup_steps: options.warmup_steps,
        samples: options.samples,
        rows,
    };
    let staging = options.output.with_extension("json.staging");
    if staging.exists() {
        return Err("product-scale staging output must be fresh".to_owned());
    }
    fs::write(
        &staging,
        serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&staging, &options.output).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_PRODUCT_SCALE_SUCCESS rows={} output={}",
        artifact.rows.len(),
        options.output.display()
    );
    Ok(())
}

fn options() -> Result<Options, String> {
    let mut output = None;
    let mut counts = vec![100, 1_000, 10_000];
    let mut warmup_steps = 1;
    let mut samples = 3;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--counts" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--counts requires a comma-separated value".to_owned())?;
                counts = raw
                    .split(',')
                    .map(|value| {
                        value
                            .parse::<u32>()
                            .map_err(|_| format!("invalid product-scale count {value}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--warmup" => warmup_steps = parse_u32(&mut args, "--warmup")?,
            "--samples" => samples = parse_u32(&mut args, "--samples")?,
            _ => return Err(format!("unknown product-scale argument {arg}")),
        }
    }
    if counts.is_empty() || warmup_steps == 0 || samples == 0 {
        return Err("product-scale counts, warmup, and samples must be non-zero".to_owned());
    }
    Ok(Options {
        output: output.ok_or_else(|| "--output is required".to_owned())?,
        counts,
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
