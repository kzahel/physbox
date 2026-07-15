use std::path::{Path, PathBuf};

use physbox_engine::{GpuBodyBenchmarkOutput, benchmark_gpu_body_free_flight};
use serde::Serialize;

const SUCCESS_SENTINEL: &str = "PHYSBOX5_BODY_BENCHMARK_SUCCESS";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: String,
    capture_phase: &'static str,
    build_profile: &'static str,
    target_os: &'static str,
    target_arch: &'static str,
    seed: u64,
    viewport: [u32; 2],
    warmup_steps: u32,
    samples: u32,
    lanes: Vec<GpuBodyBenchmarkOutput>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-body-benchmark error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let mut lanes = Vec::with_capacity(arguments.counts.len());
    for body_count in &arguments.counts {
        lanes.push(pollster::block_on(benchmark_gpu_body_free_flight(
            *body_count,
            arguments.seed,
            [arguments.width, arguments.height],
            arguments.warmup,
            arguments.samples,
        ))?);
    }
    let first_adapter = &lanes
        .first()
        .ok_or_else(|| "body benchmark produced no lanes".to_owned())?
        .adapter;
    if lanes.iter().any(|lane| {
        lane.adapter.name != first_adapter.name
            || lane.adapter.backend != first_adapter.backend
            || lane.adapter.device_type != first_adapter.device_type
    }) {
        return Err("body benchmark lanes selected different adapters".to_owned());
    }
    let report = Report {
        schema_version: physbox_contracts::SCHEMA_VERSION,
        run_id: arguments.run_id,
        source_revision: arguments.source_revision,
        source_dirty: arguments.source_dirty,
        host: arguments.host,
        capture_phase: "p5.1-body-free-flight-scale",
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        target_os: std::env::consts::OS,
        target_arch: std::env::consts::ARCH,
        seed: arguments.seed,
        viewport: [arguments.width, arguments.height],
        warmup_steps: arguments.warmup,
        samples: arguments.samples,
        lanes,
    };
    let json = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    if let Some(output) = arguments.output {
        write_atomic(&output, json.as_bytes(), &report.run_id)?;
    }
    println!("PHYSBOX5_BODY_BENCHMARK_RESULT {json}");
    println!("{SUCCESS_SENTINEL}");
    Ok(())
}

struct Arguments {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: String,
    counts: Vec<u32>,
    seed: u64,
    width: u32,
    height: u32,
    warmup: u32,
    samples: u32,
    output: Option<PathBuf>,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut source_revision = None;
        let mut source_dirty = None;
        let mut host = None;
        let mut counts = None;
        let mut seed = None;
        let mut width = None;
        let mut height = None;
        let mut warmup = None;
        let mut samples = None;
        let mut output = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--source-revision" => source_revision = Some(value),
                "--source-dirty" => source_dirty = Some(parse_bool(&value)?),
                "--host" => host = Some(value),
                "--counts" => counts = Some(parse_counts(&value)?),
                "--seed" => seed = Some(parse(&value, "seed")?),
                "--width" => width = Some(parse(&value, "width")?),
                "--height" => height = Some(parse(&value, "height")?),
                "--warmup" => warmup = Some(parse(&value, "warmup")?),
                "--samples" => samples = Some(parse(&value, "samples")?),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let output = Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            source_revision: source_revision
                .ok_or_else(|| "missing --source-revision".to_owned())?,
            source_dirty: source_dirty.ok_or_else(|| "missing --source-dirty".to_owned())?,
            host: host.ok_or_else(|| "missing --host".to_owned())?,
            counts: counts.ok_or_else(|| "missing --counts".to_owned())?,
            seed: seed.ok_or_else(|| "missing --seed".to_owned())?,
            width: width.ok_or_else(|| "missing --width".to_owned())?,
            height: height.ok_or_else(|| "missing --height".to_owned())?,
            warmup: warmup.ok_or_else(|| "missing --warmup".to_owned())?,
            samples: samples.ok_or_else(|| "missing --samples".to_owned())?,
            output,
        };
        if !matches!(
            output.host.as_str(),
            "native-offscreen" | "android-native-vulkan"
        ) {
            return Err(format!("unsupported body benchmark host: {}", output.host));
        }
        if output.width == 0 || output.height == 0 || output.warmup == 0 || output.samples == 0 {
            return Err(
                "body benchmark dimensions, warmup, and samples must be non-zero".to_owned(),
            );
        }
        Ok(output)
    }
}

fn parse_counts(value: &str) -> Result<Vec<u32>, String> {
    let mut counts = value
        .split(',')
        .map(|count| parse(count, "body count"))
        .collect::<Result<Vec<_>, _>>()?;
    if counts.is_empty() || counts.iter().any(|count| !(1..=100_000).contains(count)) {
        return Err("body counts must be between 1 and 100000".to_owned());
    }
    let original = counts.clone();
    counts.sort_unstable();
    counts.dedup();
    if counts != original {
        return Err("body counts must be sorted and unique".to_owned());
    }
    Ok(counts)
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean: {value}")),
    }
}

fn write_atomic(path: &Path, bytes: &[u8], run_id: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create body benchmark output parent: {error}"))?;
    }
    let temporary = path.with_extension(format!("tmp-{run_id}"));
    let _ = std::fs::remove_file(&temporary);
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write body benchmark temporary artifact: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish body benchmark artifact: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_counts_are_bounded_sorted_and_unique() {
        assert_eq!(
            parse_counts("1000,10000,100000").unwrap(),
            [1000, 10000, 100000]
        );
        assert!(parse_counts("10000,1000").is_err());
        assert!(parse_counts("1000,1000").is_err());
        assert!(parse_counts("100001").is_err());
    }
}
