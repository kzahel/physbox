use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process::Command,
};

use physbox_contracts::{canonical_hash, sha256_bytes};
use physbox_engine::{
    AdapterDiagnostics, EngineConfig, FrameInput, GpuFixedFloorContactEvidence, OffscreenRenderer,
    SceneContract, p5_1_rectangle_floor_scenario,
};
use physbox_launch::ValidationScenario;
use serde::Serialize;

const CHECKPOINTS: [u32; 7] = [1, 15, 30, 60, 90, 120, 180];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductPlayback {
    schema_version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_scenario_hash: String,
    neutral_projection_hash: String,
    adapter: AdapterDiagnostics,
    samples: Vec<ProductSample>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductSample {
    completed_steps: u32,
    gpu_state: [f32; 8],
    contacts: Vec<GpuFixedFloorContactEvidence>,
    scene_contract: SceneContract,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-fixed-floor-playback error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let source_dirty = !git(&["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty();
    let source = p5_1_rectangle_floor_scenario(180).map_err(|error| error.to_string())?;
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    let mut neutral = source.clone();
    neutral.id = "p5.1-rectangle-floor-authority-projection".to_owned();
    neutral.manifolds.clear();
    let neutral_projection_hash = canonical_hash(&neutral).map_err(|error| error.to_string())?;

    let mut renderer = pollster::block_on(OffscreenRenderer::new(
        960,
        600,
        EngineConfig {
            scenario: ValidationScenario::RigidFloor,
        },
    ))?;
    let adapter = renderer.diagnostics().clone();
    let mut samples = Vec::with_capacity(CHECKPOINTS.len());
    let mut completed = 0;
    for checkpoint in CHECKPOINTS {
        let report = renderer.render(FrameInput::exact(checkpoint - completed));
        completed = checkpoint;
        let gpu_state = renderer.read_capture_state()?;
        if !gpu_state.iter().all(|value| value.is_finite()) {
            return Err(format!(
                "checkpoint {checkpoint} produced non-finite GPU state"
            ));
        }
        let pass = renderer.read_fixed_floor_pass_evidence()?;
        samples.push(ProductSample {
            completed_steps: checkpoint,
            gpu_state,
            contacts: pass.contacts,
            scene_contract: report.scene_contract,
        });
    }
    let artifact = ProductPlayback {
        schema_version: 1,
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_scenario_hash,
        neutral_projection_hash,
        adapter,
        samples,
    };
    let bytes = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    if let Some(parent) = arguments.output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&arguments.output)
        .map_err(|error| format!("create fresh {}: {error}", arguments.output.display()))?;
    output
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    output.sync_all().map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=headless scenario=fixed-floor-playback artifact_sha256={} output={}",
        sha256_bytes(&bytes),
        arguments.output.display()
    );
    Ok(())
}

fn git(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("git {} failed", arguments.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut output = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
        })
    }
}
