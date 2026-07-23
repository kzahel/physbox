use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use physbox_contracts::{
    CoupledOracleRequestV1, CoupledOracleResultV1, FailureKindV1, ORACLE_SUCCESS_SENTINEL,
    SCHEMA_VERSION, Validate, canonical_hash, canonical_json, sha256_bytes,
};
use physbox_engine::{CoupledScenarioKind, canonical_coupled_scenario};
use serde::Serialize;

const SUITE_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OracleSuiteReport {
    version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    rows: Vec<OracleRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OracleRow {
    lane: &'static str,
    scenario: &'static str,
    supported: bool,
    expected_unsupported: bool,
    completed_steps: u32,
    particle_count: u32,
    body_count: u32,
    density_error_rms: f32,
    fluid_motion_rms: f32,
    particle_body_contacts: u64,
    failure: Option<String>,
    result: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-coupled-oracles error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [output, physbox3, liquidfun] = arguments.as_slice() else {
        return Err(
            "usage: physbox-coupled-oracles OUTPUT_DIRECTORY PHYSBOX3_ORACLE LIQUIDFUN_ORACLE"
                .to_owned(),
        );
    };
    let output = PathBuf::from(output);
    std::fs::create_dir(&output).map_err(|error| {
        format!(
            "create fresh oracle directory {}: {error}",
            output.display()
        )
    })?;
    let suite_run_id = run_id();
    let mut rows = Vec::new();
    for (lane, program) in [
        ("physbox3-primary", Path::new(physbox3)),
        ("liquidfun-secondary", Path::new(liquidfun)),
    ] {
        if !program.is_file() {
            return Err(format!(
                "{lane} oracle does not exist: {}",
                program.display()
            ));
        }
        for kind in CoupledScenarioKind::ALL {
            rows.push(run_row(&output, &suite_run_id, lane, program, kind)?);
        }
    }
    let report = OracleSuiteReport {
        version: SUITE_VERSION,
        run_id: suite_run_id,
        source_revision: git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned()),
        source_dirty: git(&["status", "--porcelain"]).is_none_or(|status| !status.is_empty()),
        rows,
    };
    let report_path = output.join("report.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write {}: {error}", report_path.display()))?;
    println!(
        "PHYSBOX5_COUPLED_ORACLE_SUITE_SUCCESS rows={} supported={} unsupported={} report={}",
        report.rows.len(),
        report.rows.iter().filter(|row| row.supported).count(),
        report.rows.iter().filter(|row| !row.supported).count(),
        report_path.display()
    );
    Ok(())
}

fn run_row(
    output: &Path,
    suite_run_id: &str,
    lane: &'static str,
    program: &Path,
    kind: CoupledScenarioKind,
) -> Result<OracleRow, String> {
    let scenario = canonical_coupled_scenario(kind).map_err(|error| error.to_string())?;
    let source_scenario_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
    let run_id = format!("{suite_run_id}-{lane}-{}", kind.id());
    let request = CoupledOracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id,
        source_scenario_hash,
        scenario,
    };
    request.validate().map_err(|error| error.to_string())?;
    let directory = output.join(format!("{lane}-{}", kind.id()));
    std::fs::create_dir(&directory)
        .map_err(|error| format!("create {}: {error}", directory.display()))?;
    let request_path = directory.join("request.json");
    let result_path = directory.join("result.json");
    let stdout_path = directory.join("stdout.log");
    let stderr_path = directory.join("stderr.log");
    let request_bytes = canonical_json(&request).map_err(|error| error.to_string())?;
    std::fs::write(&request_path, &request_bytes)
        .map_err(|error| format!("write {}: {error}", request_path.display()))?;
    let process = Command::new(program)
        .args(["--request"])
        .arg(&request_path)
        .args(["--result"])
        .arg(&result_path)
        .output()
        .map_err(|error| format!("launch {lane} {}: {error}", kind.id()))?;
    std::fs::write(&stdout_path, &process.stdout)
        .map_err(|error| format!("write {}: {error}", stdout_path.display()))?;
    std::fs::write(&stderr_path, &process.stderr)
        .map_err(|error| format!("write {}: {error}", stderr_path.display()))?;
    if !process.status.success() {
        return Err(format!(
            "{lane} {} exited {}: {}",
            kind.id(),
            process.status,
            String::from_utf8_lossy(&process.stderr)
        ));
    }
    if !String::from_utf8_lossy(&process.stdout)
        .lines()
        .any(|line| line == ORACLE_SUCCESS_SENTINEL)
    {
        return Err(format!("{lane} {} omitted the success sentinel", kind.id()));
    }
    let result_bytes = std::fs::read(&result_path)
        .map_err(|error| format!("read {}: {error}", result_path.display()))?;
    let result: CoupledOracleResultV1 =
        serde_json::from_slice(&result_bytes).map_err(|error| error.to_string())?;
    result.validate().map_err(|error| error.to_string())?;
    if result.run_id != request.run_id
        || result.source_scenario_hash != request.source_scenario_hash
        || result.projection_hash != sha256_bytes(&request_bytes)
    {
        return Err(format!("{lane} {} result identity mismatch", kind.id()));
    }
    let expected_unsupported = kind == CoupledScenarioKind::GrabbedBoundary;
    let supported = result.failure.is_none();
    if expected_unsupported
        != result
            .failure
            .as_ref()
            .is_some_and(|failure| failure.kind == FailureKindV1::UnsupportedFeature)
    {
        return Err(format!(
            "{lane} {} reported an unexpected support disposition",
            kind.id()
        ));
    }
    println!(
        "PHYSBOX5_COUPLED_ORACLE_ROW_SUCCESS lane={} scenario={} supported={} particles={} contacts={}",
        lane,
        kind.id(),
        supported,
        result.particles.len(),
        result.metrics.work.particle_body_contacts,
    );
    Ok(OracleRow {
        lane,
        scenario: kind.id(),
        supported,
        expected_unsupported,
        completed_steps: result.completed_steps,
        particle_count: result.particles.len() as u32,
        body_count: result.bodies.len() as u32,
        density_error_rms: result.metrics.density_error_rms.get(),
        fluid_motion_rms: result.metrics.fluid_motion_rms.get(),
        particle_body_contacts: result.metrics.work.particle_body_contacts,
        failure: result.failure.map(|failure| failure.reason),
        result: result_path
            .strip_prefix(output)
            .unwrap_or(&result_path)
            .display()
            .to_string(),
    })
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
