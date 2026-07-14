use std::{fs, io::Write, path::PathBuf};

use physbox_box2d_benchmark::{SUCCESS_SENTINEL, benchmark_scenario};
use physbox_contracts::{ScenarioV1, Validate, sha256_bytes};

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-box2d-benchmark error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    if arguments.result.exists() {
        return Err(format!(
            "result already exists: {}",
            arguments.result.display()
        ));
    }
    let partial = arguments.result.with_extension("partial.json");
    if partial.exists() {
        return Err(format!(
            "partial result already exists: {}",
            partial.display()
        ));
    }
    let bytes = fs::read(&arguments.request).map_err(|error| error.to_string())?;
    let scenario: ScenarioV1 = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    scenario.validate().map_err(|error| error.to_string())?;
    let result = benchmark_scenario(
        &scenario,
        &arguments.run_id,
        sha256_bytes(&bytes),
        arguments.workers,
        arguments.warmup,
        arguments.samples,
    )?;
    let result_bytes = serde_json::to_vec_pretty(&result).map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(&result_bytes)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&partial, &arguments.result).map_err(|error| error.to_string())?;
    println!("{SUCCESS_SENTINEL}");
    Ok(())
}

struct Arguments {
    request: PathBuf,
    result: PathBuf,
    run_id: String,
    workers: u32,
    warmup: u32,
    samples: u32,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut request = None;
        let mut result = None;
        let mut run_id = None;
        let mut workers = None;
        let mut warmup = None;
        let mut samples = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--request" => request = Some(PathBuf::from(value)),
                "--result" => result = Some(PathBuf::from(value)),
                "--run-id" => run_id = Some(value),
                "--workers" => workers = Some(parse(&value, "workers")?),
                "--warmup" => warmup = Some(parse(&value, "warmup")?),
                "--samples" => samples = Some(parse(&value, "samples")?),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            request: request.ok_or_else(|| "missing --request".to_owned())?,
            result: result.ok_or_else(|| "missing --result".to_owned())?,
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            workers: workers.ok_or_else(|| "missing --workers".to_owned())?,
            warmup: warmup.ok_or_else(|| "missing --warmup".to_owned())?,
            samples: samples.ok_or_else(|| "missing --samples".to_owned())?,
        })
    }
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}
