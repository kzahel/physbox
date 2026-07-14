use std::path::PathBuf;

use physbox_lab::evidence::{EvidenceConfig, generate_evidence};

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-solver-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Arguments::parse()?;
    let manifest = generate_evidence(&config)?;
    println!(
        "PHYSBOX5_SOLVER_EVIDENCE_SUCCESS run_id={} revision={} output={}",
        manifest.run_id,
        manifest.source_revision,
        config.output.display()
    );
    Ok(())
}

struct Arguments;

impl Arguments {
    fn parse() -> Result<EvidenceConfig, String> {
        let mut output = None;
        let mut run_id = None;
        let mut box2d_benchmark = None;
        let mut android_avd = None;
        let mut android_device = None;
        let mut android_device_gap = None;
        let mut warmup_steps = 3;
        let mut samples = 20;
        let mut corpus_seeds = 32;
        let mut quick = false;
        let mut require_clean = false;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--quick" => quick = true,
                "--require-clean" => require_clean = true,
                _ => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| format!("missing value for {argument}"))?;
                    match argument.as_str() {
                        "--output" => output = Some(PathBuf::from(value)),
                        "--run-id" => run_id = Some(value),
                        "--box2d-benchmark" => box2d_benchmark = Some(PathBuf::from(value)),
                        "--android-avd" => android_avd = Some(PathBuf::from(value)),
                        "--android-device" => android_device = Some(PathBuf::from(value)),
                        "--android-device-gap" => android_device_gap = Some(value),
                        "--warmup" => warmup_steps = parse(&value, "warmup")?,
                        "--samples" => samples = parse(&value, "samples")?,
                        "--corpus-seeds" => corpus_seeds = parse(&value, "corpus seeds")?,
                        _ => return Err(format!("unknown argument {argument}")),
                    }
                }
            }
        }
        Ok(EvidenceConfig {
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            box2d_benchmark: box2d_benchmark
                .ok_or_else(|| "missing --box2d-benchmark".to_owned())?,
            android_avd,
            android_device,
            android_device_gap,
            warmup_steps,
            samples,
            corpus_seeds,
            quick,
            require_clean,
        })
    }
}

fn parse<T: std::str::FromStr>(value: &str, name: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {name}: {value}"))
}
