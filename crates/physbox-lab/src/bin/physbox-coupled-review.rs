use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use physbox_contracts::{BodyId, BodyStateV1, Vec2, sha256_bytes};
use serde::{Deserialize, Serialize};

const REVIEW_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuSuite {
    source_revision: String,
    source_dirty: bool,
    rows: Vec<GpuRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuRow {
    label: String,
    accepted: bool,
    exact_replay: Option<ExactReplay>,
    evidence: GpuEvidence,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExactReplay {
    exact: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuEvidence {
    source_scenario_id: String,
    initial_particles: ParticleAggregate,
    final_particles: ParticleAggregate,
    particle_diagnostics: ParticleDiagnostics,
    bodies: Vec<GpuBody>,
    body_trajectories: serde_json::Value,
    exchange: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticleAggregate {
    active_particles: u32,
    centroid: Vec2,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticleDiagnostics {
    state_finite: bool,
    capacity_clean: bool,
    overflow_flags: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBody {
    state: BodyStateV1,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OracleSuite {
    source_revision: String,
    source_dirty: bool,
    rows: Vec<OracleRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OracleRow {
    lane: String,
    scenario: String,
    supported: bool,
    particle_count: u32,
    particle_centroid: Option<Vec2>,
    bodies: Vec<BodyStateV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewReport {
    version: u32,
    run_id: String,
    gpu_report_sha256: String,
    oracle_report_sha256: String,
    gpu_source_revision: String,
    gpu_source_dirty: bool,
    oracle_source_revision: String,
    oracle_source_dirty: bool,
    rows: Vec<ReviewRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRow {
    scenario: String,
    authority_disposition: &'static str,
    gpu_accepted: bool,
    gpu_state_finite: bool,
    gpu_capacity_clean: bool,
    gpu_overflow_flags: u32,
    gpu_exact_replay: bool,
    gpu_initial_particles: u32,
    gpu_final_particles: u32,
    gpu_particle_centroid: Vec2,
    gpu_body_trajectories: serde_json::Value,
    gpu_exchange: serde_json::Value,
    primary: OracleComparison,
    secondary: OracleComparison,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OracleComparison {
    lane: String,
    supported: bool,
    particle_count: u32,
    particle_count_delta_from_gpu: i64,
    particle_centroid: Option<Vec2>,
    particle_centroid_delta_from_gpu: Option<f32>,
    dynamic_body_deltas: Vec<BodyDelta>,
    max_body_position_delta: f32,
    max_body_angle_delta: f32,
    max_body_linear_velocity_delta: f32,
    max_body_angular_velocity_delta: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyDelta {
    body: BodyId,
    gpu_position: Vec2,
    oracle_position: Vec2,
    position_delta: f32,
    gpu_angle: f32,
    oracle_angle: f32,
    angle_abs_delta: f32,
    linear_velocity_delta: f32,
    angular_velocity_abs_delta: f32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-coupled-review error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [gpu_path, oracle_path, output_path] = arguments.as_slice() else {
        return Err(
            "usage: physbox-coupled-review GPU_REPORT ORACLE_REPORT OUTPUT_REPORT".to_owned(),
        );
    };
    let gpu_path = PathBuf::from(gpu_path);
    let oracle_path = PathBuf::from(oracle_path);
    let output_path = PathBuf::from(output_path);
    let gpu_bytes = std::fs::read(&gpu_path)
        .map_err(|error| format!("read {}: {error}", gpu_path.display()))?;
    let oracle_bytes = std::fs::read(&oracle_path)
        .map_err(|error| format!("read {}: {error}", oracle_path.display()))?;
    let gpu: GpuSuite = serde_json::from_slice(&gpu_bytes)
        .map_err(|error| format!("decode GPU report: {error}"))?;
    let oracle: OracleSuite = serde_json::from_slice(&oracle_bytes)
        .map_err(|error| format!("decode oracle report: {error}"))?;
    let mut rows = Vec::new();
    for gpu_row in gpu
        .rows
        .iter()
        .filter(|row| row.label == row.evidence.source_scenario_id)
    {
        let primary = oracle_row("physbox3-primary", &gpu_row.evidence, &oracle.rows)?;
        let secondary = oracle_row("liquidfun-secondary", &gpu_row.evidence, &oracle.rows)?;
        let authority_disposition = if !gpu_row.accepted
            || !gpu_row.evidence.particle_diagnostics.state_finite
            || !gpu_row.evidence.particle_diagnostics.capacity_clean
        {
            "gpu-rejected"
        } else if !primary.supported {
            "primary-unsupported-requires-human-review"
        } else {
            "measured-primary-comparison-requires-declared-band"
        };
        rows.push(ReviewRow {
            scenario: gpu_row.evidence.source_scenario_id.clone(),
            authority_disposition,
            gpu_accepted: gpu_row.accepted,
            gpu_state_finite: gpu_row.evidence.particle_diagnostics.state_finite,
            gpu_capacity_clean: gpu_row.evidence.particle_diagnostics.capacity_clean,
            gpu_overflow_flags: gpu_row.evidence.particle_diagnostics.overflow_flags,
            gpu_exact_replay: gpu_row
                .exact_replay
                .as_ref()
                .is_some_and(|replay| replay.exact),
            gpu_initial_particles: gpu_row.evidence.initial_particles.active_particles,
            gpu_final_particles: gpu_row.evidence.final_particles.active_particles,
            gpu_particle_centroid: gpu_row.evidence.final_particles.centroid,
            gpu_body_trajectories: gpu_row.evidence.body_trajectories.clone(),
            gpu_exchange: gpu_row.evidence.exchange.clone(),
            primary,
            secondary,
        });
    }
    if rows.len() != 7 {
        return Err(format!(
            "coupled review requires seven canonical GPU rows, found {}",
            rows.len()
        ));
    }
    let report = ReviewReport {
        version: REVIEW_VERSION,
        run_id: run_id(),
        gpu_report_sha256: sha256_bytes(&gpu_bytes),
        oracle_report_sha256: sha256_bytes(&oracle_bytes),
        gpu_source_revision: gpu.source_revision,
        gpu_source_dirty: gpu.source_dirty,
        oracle_source_revision: oracle.source_revision,
        oracle_source_dirty: oracle.source_dirty,
        rows,
    };
    write_new(
        &output_path,
        &serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("encode coupled review report: {error}"))?,
    )?;
    println!(
        "PHYSBOX5_COUPLED_REVIEW_SUCCESS rows={} report={}",
        report.rows.len(),
        output_path.display()
    );
    Ok(())
}

fn oracle_row(
    lane: &str,
    gpu: &GpuEvidence,
    rows: &[OracleRow],
) -> Result<OracleComparison, String> {
    let row = rows
        .iter()
        .find(|row| row.lane == lane && row.scenario == gpu.source_scenario_id)
        .ok_or_else(|| format!("missing {lane} row for {}", gpu.source_scenario_id))?;
    let mut dynamic_body_deltas = Vec::new();
    if row.supported {
        for gpu_body in gpu
            .bodies
            .iter()
            .filter(|body| body.state.inverse_mass.get() > 0.0)
        {
            let Some(oracle_body) = row.bodies.iter().find(|body| body.id == gpu_body.state.id)
            else {
                return Err(format!(
                    "{lane} {} omitted dynamic body {}:{}",
                    row.scenario, gpu_body.state.id.slot, gpu_body.state.id.generation
                ));
            };
            dynamic_body_deltas.push(BodyDelta {
                body: gpu_body.state.id,
                gpu_position: gpu_body.state.position,
                oracle_position: oracle_body.position,
                position_delta: distance(gpu_body.state.position, oracle_body.position),
                gpu_angle: gpu_body.state.angle.get(),
                oracle_angle: oracle_body.angle.get(),
                angle_abs_delta: (gpu_body.state.angle.get() - oracle_body.angle.get()).abs(),
                linear_velocity_delta: distance(
                    gpu_body.state.linear_velocity,
                    oracle_body.linear_velocity,
                ),
                angular_velocity_abs_delta: (gpu_body.state.angular_velocity.get()
                    - oracle_body.angular_velocity.get())
                .abs(),
            });
        }
    }
    Ok(OracleComparison {
        lane: lane.to_owned(),
        supported: row.supported,
        particle_count: row.particle_count,
        particle_count_delta_from_gpu: i64::from(row.particle_count)
            - i64::from(gpu.final_particles.active_particles),
        particle_centroid: row.particle_centroid,
        particle_centroid_delta_from_gpu: row
            .particle_centroid
            .map(|centroid| distance(gpu.final_particles.centroid, centroid)),
        max_body_position_delta: maximum(&dynamic_body_deltas, |delta| delta.position_delta),
        max_body_angle_delta: maximum(&dynamic_body_deltas, |delta| delta.angle_abs_delta),
        max_body_linear_velocity_delta: maximum(&dynamic_body_deltas, |delta| {
            delta.linear_velocity_delta
        }),
        max_body_angular_velocity_delta: maximum(&dynamic_body_deltas, |delta| {
            delta.angular_velocity_abs_delta
        }),
        dynamic_body_deltas,
    })
}

fn maximum(values: &[BodyDelta], value: impl Fn(&BodyDelta) -> f32) -> f32 {
    values.iter().map(value).fold(0.0, f32::max)
}

fn distance(left: Vec2, right: Vec2) -> f32 {
    let x = left.x - right.x;
    let y = left.y - right.y;
    (x * x + y * y).sqrt()
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create fresh {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", path.display()))
}

fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos}-{}", std::process::id())
}
