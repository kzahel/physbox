#![forbid(unsafe_code)]

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use physbox_engine::{EngineConfig, FrameInput, FrameReport, OffscreenRenderer};
use physbox_launch::{LaunchHost, parse_os_args, usage};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance<'a> {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    host: &'static str,
    capture_phase: String,
    adapter: &'a physbox_engine::AdapterDiagnostics,
    report: &'a FrameReport,
    gpu_state: [f32; 8],
    fixed_floor_pass: Option<&'a physbox_engine::GpuFixedFloorPassEvidence>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox5 headless error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let request = parse_os_args(LaunchHost::Headless, std::env::args_os()).map_err(|error| {
        format!(
            "{error}\n\n{}",
            usage("physbox-headless", LaunchHost::Headless)
        )
    })?;
    if request.help_requested {
        println!("{}", usage("physbox-headless", LaunchHost::Headless));
        return Ok(());
    }
    let options = request.options;
    let output = options
        .output
        .as_deref()
        .expect("validated headless output path");
    let report_path = options
        .report
        .as_deref()
        .expect("validated headless report path");
    prepare_fresh_path(output)?;
    prepare_fresh_path(report_path)?;

    let mut renderer = pollster::block_on(OffscreenRenderer::new(
        options.width,
        options.height,
        EngineConfig::from(&options),
    ))?;
    println!(
        "PHYSBOX5_MARK_ADAPTER {}",
        renderer.diagnostics().marker_json()
    );

    let mut report = None;
    if let Some(steps) = options.steps {
        report = Some(renderer.render(FrameInput::exact(steps)));
    } else {
        let elapsed = 1.0 / f64::from(options.fps);
        for _ in 0..options.frames {
            report = Some(renderer.render(FrameInput::new(elapsed)));
        }
    }
    let report = report.ok_or_else(|| "capture rendered no frames".to_owned())?;
    if !report.scenario.complete || !report.scenario.passed {
        return Err(format!("scenario did not pass: {}", report.scenario.reason));
    }
    let pixels = renderer.read_pixels()?;
    let gpu_state = renderer.read_capture_state()?;
    if !gpu_state.iter().all(|value| value.is_finite()) {
        return Err("GPU capture state contains a non-finite value".to_owned());
    }
    write_png(output, options.width, options.height, &pixels)?;
    let fixed_floor_pass = (report.scene_contract.contact_point_count > 0)
        .then(|| renderer.read_fixed_floor_pass_evidence())
        .transpose()?;

    let provenance = Provenance {
        run_id: run_id(),
        source_revision: git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned()),
        source_dirty: git_output(&["status", "--porcelain"])
            .is_none_or(|status| !status.is_empty()),
        host: "headless",
        capture_phase: format!("{}-final", report.scenario.name),
        adapter: renderer.diagnostics(),
        report: &report,
        gpu_state,
        fixed_floor_pass: fixed_floor_pass.as_ref(),
    };
    let json = serde_json::to_vec_pretty(&provenance)
        .map_err(|error| format!("failed to serialize provenance: {error}"))?;
    std::fs::write(report_path, json)
        .map_err(|error| format!("failed to write {}: {error}", report_path.display()))?;

    println!(
        "PHYSBOX5_MARK_SCENE_CONTRACT {}",
        serde_json::to_string(&report.scene_contract).unwrap()
    );
    println!("PHYSBOX5_MARK_FRAME_RENDERED {}", report.marker_json());
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=headless scenario={} frames={} output={} report={}",
        report.scenario.name,
        report.frame_index,
        output.display(),
        report_path.display()
    );
    Ok(())
}

fn prepare_fresh_path(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale {}: {error}",
            path.display()
        )),
    }
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_gamma(png::ScaledFloat::from_scaled(45_455));
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("failed to write PNG header: {error}"))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("failed to write PNG pixels: {error}"))
}

fn git_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos}-{}", std::process::id())
}
