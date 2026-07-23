use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use physbox_engine::{
    CoupledScenarioKind, GpuCoupledEvidenceConfigV1, GpuCoupledEvidenceV1, SolverProfileConfig,
    canonical_coupled_scenario, coupled_capacity_scenario, liquid_sandbox_scenario,
    run_gpu_coupled_evidence,
};
use physbox_launch::{PhysicsConfigurationV3, SleepGrouping};
use serde::Serialize;

const SUITE_VERSION: u32 = 1;
const VIEWPORT: [u32; 2] = [960, 600];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuiteReport {
    version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    suite: &'static str,
    comparator_compiled: bool,
    accepted_rows: u32,
    rejected_rows: u32,
    rows: Vec<EvidenceRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceRow {
    label: String,
    capture: String,
    accepted: bool,
    failure: Option<String>,
    exact_replay: Option<ExactReplay>,
    evidence: GpuCoupledEvidenceV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExactReplay {
    first_state_sha256: String,
    second_state_sha256: String,
    exact: bool,
    active_particles_equal: bool,
    particle_centroid_delta: f32,
    particle_mean_velocity_delta: f32,
    body_position_delta_max: f32,
    body_angle_delta_max: f32,
    density_error_rms_delta: f32,
    fluid_motion_rms_delta: f32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-coupled-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [suite, output] = arguments.as_slice() else {
        return Err(
            "usage: physbox-coupled-evidence corpus OUTPUT_DIRECTORY | capacity OUTPUT_DIRECTORY"
                .to_owned(),
        );
    };
    let output = PathBuf::from(output);
    std::fs::create_dir(&output).map_err(|error| {
        format!(
            "create fresh evidence directory {}: {error}",
            output.display()
        )
    })?;
    let rows = match suite.as_str() {
        "corpus" => corpus(&output)?,
        "capacity" => capacity(&output)?,
        _ => return Err("coupled evidence suite must be corpus or capacity".to_owned()),
    };
    let accepted_rows = rows.iter().filter(|row| row.accepted).count() as u32;
    let rejected_rows = rows.len() as u32 - accepted_rows;
    let report = SuiteReport {
        version: SUITE_VERSION,
        run_id: run_id(),
        source_revision: git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned()),
        source_dirty: git(&["status", "--porcelain"]).is_none_or(|status| !status.is_empty()),
        suite: if suite == "corpus" {
            "coupled-corpus"
        } else {
            "coupled-capacity"
        },
        comparator_compiled: false,
        accepted_rows,
        rejected_rows,
        rows,
    };
    let report_path = output.join("report.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write {}: {error}", report_path.display()))?;
    println!(
        "PHYSBOX5_COUPLED_EVIDENCE_SUCCESS suite={} accepted={} rejected={} report={}",
        report.suite,
        report.accepted_rows,
        report.rejected_rows,
        report_path.display()
    );
    Ok(())
}

fn corpus(output: &Path) -> Result<Vec<EvidenceRow>, String> {
    let mut rows = Vec::new();
    for kind in CoupledScenarioKind::ALL {
        let source = canonical_coupled_scenario(kind).map_err(|error| error.to_string())?;
        let config = GpuCoupledEvidenceConfigV1 {
            viewport: VIEWPORT,
            warmup_steps: 0,
            measured_steps: source.requested_steps,
            steps_per_sample: 64,
        };
        let label = kind.id().to_owned();
        let first = pollster::block_on(run_gpu_coupled_evidence(
            source.clone(),
            SolverProfileConfig::soft_coupled(),
            config,
        ))?;
        let second = pollster::block_on(run_gpu_coupled_evidence(
            source,
            SolverProfileConfig::soft_coupled(),
            config,
        ))?;
        let replay = compare_replay(&first.evidence, &second.evidence);
        require_capacity_clean(&first.evidence)?;
        require_capacity_clean(&second.evidence)?;
        rows.push(write_row(output, label, first, true, None, Some(replay))?);
    }

    for grouping in SleepGrouping::ALL {
        let mut physics = PhysicsConfigurationV3::SOFT_COUPLED;
        physics.sleeping_enabled = true;
        physics.sleep_grouping = grouping;
        let profile = SolverProfileConfig::from_physics(physics)?;
        let source = canonical_coupled_scenario(CoupledScenarioKind::FluidWake)
            .map_err(|error| error.to_string())?;
        let config = GpuCoupledEvidenceConfigV1 {
            viewport: VIEWPORT,
            warmup_steps: 0,
            measured_steps: source.requested_steps,
            steps_per_sample: 64,
        };
        let run = pollster::block_on(run_gpu_coupled_evidence(source, profile, config))?;
        require_capacity_clean(&run.evidence)?;
        let wake_observed = run.evidence.particle_diagnostics.fluid_wake_roots != 0;
        let failure = (!wake_observed).then(|| {
            format!(
                "fluid-wake source produced no final-step wake roots for {}",
                grouping.name()
            )
        });
        rows.push(write_row(
            output,
            format!("coupled-fluid-wake-v1-sleep-{}", grouping.name()),
            run,
            wake_observed,
            failure,
            None,
        )?);
    }
    Ok(rows)
}

fn capacity(output: &Path) -> Result<Vec<EvidenceRow>, String> {
    let mut rows = Vec::new();
    let product = liquid_sandbox_scenario().map_err(|error| error.to_string())?;
    let run = pollster::block_on(run_gpu_coupled_evidence(
        product,
        SolverProfileConfig::soft_coupled(),
        GpuCoupledEvidenceConfigV1 {
            viewport: VIEWPORT,
            warmup_steps: 30,
            measured_steps: 60,
            steps_per_sample: 1,
        },
    ))?;
    require_capacity_clean(&run.evidence)?;
    rows.push(write_row(
        output,
        "product-coupled-3952-cap32768".to_owned(),
        run,
        true,
        None,
        None,
    )?);

    for particles in [1_024, 4_096, 16_384, 32_768] {
        let source = coupled_capacity_scenario(particles).map_err(|error| error.to_string())?;
        let run = pollster::block_on(run_gpu_coupled_evidence(
            source,
            SolverProfileConfig::soft_coupled(),
            GpuCoupledEvidenceConfigV1 {
                viewport: VIEWPORT,
                warmup_steps: 30,
                measured_steps: 60,
                steps_per_sample: 1,
            },
        ))?;
        let disposition = require_capacity_clean(&run.evidence);
        let accepted = disposition.is_ok();
        let failure = disposition.err();
        rows.push(write_row(
            output,
            format!("water-only-{particles}"),
            run,
            accepted,
            failure,
            None,
        )?);
        if !accepted {
            break;
        }
    }
    Ok(rows)
}

fn compare_replay(first: &GpuCoupledEvidenceV1, second: &GpuCoupledEvidenceV1) -> ExactReplay {
    let particle_centroid_delta = distance(
        first.final_particles.centroid,
        second.final_particles.centroid,
    );
    let particle_mean_velocity_delta = distance(
        first.final_particles.mean_velocity,
        second.final_particles.mean_velocity,
    );
    let mut body_position_delta_max = 0.0f32;
    let mut body_angle_delta_max = 0.0f32;
    for (left, right) in first.bodies.iter().zip(&second.bodies) {
        body_position_delta_max =
            body_position_delta_max.max(distance(left.state.position, right.state.position));
        body_angle_delta_max =
            body_angle_delta_max.max((left.state.angle.get() - right.state.angle.get()).abs());
    }
    ExactReplay {
        first_state_sha256: first.exact_state_sha256.clone(),
        second_state_sha256: second.exact_state_sha256.clone(),
        exact: first.exact_state_sha256 == second.exact_state_sha256,
        active_particles_equal: first.final_particles.active_particles
            == second.final_particles.active_particles,
        particle_centroid_delta,
        particle_mean_velocity_delta,
        body_position_delta_max,
        body_angle_delta_max,
        density_error_rms_delta: (first.particle_diagnostics.density_error_rms
            - second.particle_diagnostics.density_error_rms)
            .abs(),
        fluid_motion_rms_delta: (first.particle_diagnostics.fluid_motion_rms
            - second.particle_diagnostics.fluid_motion_rms)
            .abs(),
    }
}

fn distance(left: physbox_contracts::Vec2, right: physbox_contracts::Vec2) -> f32 {
    let x = left.x - right.x;
    let y = left.y - right.y;
    (x * x + y * y).sqrt()
}

fn require_capacity_clean(evidence: &GpuCoupledEvidenceV1) -> Result<(), String> {
    let diagnostics = &evidence.particle_diagnostics;
    if !diagnostics.state_finite
        || !diagnostics.capacity_clean
        || diagnostics.overflow_flags != 0
        || diagnostics.grid_overflow != 0
        || diagnostics.command_drops != 0
        || diagnostics.body_contribution_overflow != 0
        || evidence
            .capacities
            .iter()
            .any(|capacity| capacity.overflowed)
    {
        return Err(format!(
            "{} rejected: finite={} capacity_clean={} overflow_flags={} grid={} commands={} body_contributions={}",
            evidence.source_scenario_id,
            diagnostics.state_finite,
            diagnostics.capacity_clean,
            diagnostics.overflow_flags,
            diagnostics.grid_overflow,
            diagnostics.command_drops,
            diagnostics.body_contribution_overflow,
        ));
    }
    Ok(())
}

fn write_row(
    output: &Path,
    label: String,
    run: physbox_engine::GpuCoupledEvidenceRun,
    accepted: bool,
    failure: Option<String>,
    exact_replay: Option<ExactReplay>,
) -> Result<EvidenceRow, String> {
    let capture = format!("{label}.png");
    write_png(
        &output.join(&capture),
        run.evidence.config.viewport,
        &run.rgba8,
    )?;
    println!(
        "PHYSBOX5_COUPLED_ROW_SUCCESS label={} accepted={} active={} capacity={} host_mean_ms={:.3} state={}",
        label,
        accepted,
        run.evidence.particle_diagnostics.active_count,
        run.evidence.particle_diagnostics.capacity,
        run.evidence
            .timing
            .host_completion
            .mean_seconds_per_outer_step
            * 1_000.0,
        run.evidence.exact_state_sha256,
    );
    Ok(EvidenceRow {
        label,
        capture,
        accepted,
        failure,
        exact_replay,
        evidence: run.evidence,
    })
}

fn write_png(path: &Path, viewport: [u32; 2], pixels: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), viewport[0], viewport[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("write PNG header {}: {error}", path.display()))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("write PNG {}: {error}", path.display()))
}

fn git(arguments: &[&str]) -> Option<String> {
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
