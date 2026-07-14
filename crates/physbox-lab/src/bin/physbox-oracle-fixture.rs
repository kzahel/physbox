use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use physbox_contracts::{
    FailureKindV1, FailureV1, FiniteF32, MetricsV1, ORACLE_SUCCESS_SENTINEL, OracleRequestV1,
    OracleResultV1, ProvenanceV1, SCHEMA_VERSION, Validate, WorkVectorV1, canonical_json,
    sha256_bytes,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("oracle transport fixture error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    if arguments.mode == "timeout" {
        thread::sleep(Duration::from_secs(10));
        return Ok(());
    }
    let request_bytes = fs::read(&arguments.request).map_err(|error| error.to_string())?;
    let request: OracleRequestV1 =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    request.validate().map_err(|error| error.to_string())?;
    if arguments.mode == "partial" {
        fs::write(&arguments.result, b"{").map_err(|error| error.to_string())?;
        println!("{ORACLE_SUCCESS_SENTINEL}");
        return Ok(());
    }

    let mut result = success_result(&request, &request_bytes, &arguments.source_revision);
    match arguments.mode.as_str() {
        "success" | "missing-sentinel" => {}
        "stale" => {
            result.run_id = "old-run".to_owned();
            result.provenance.run_id = "old-run".to_owned();
        }
        "wrong-source-hash" => result.source_scenario_hash = "1".repeat(64),
        "wrong-projection-hash" => result.projection_hash = "2".repeat(64),
        "wrong-pin" => result.provenance.source_revision = "wrong-revision".to_owned(),
        "unsupported" => {
            result.failure = Some(FailureV1 {
                kind: FailureKindV1::UnsupportedFeature,
                reason: "fixture rejects the requested shape".to_owned(),
                first_step: None,
                first_stage: None,
                reproducer_hash: Some(request.source_scenario_hash.clone()),
            });
        }
        other => return Err(format!("unknown fixture mode {other}")),
    }
    let bytes = canonical_json(&result).map_err(|error| error.to_string())?;
    fs::write(&arguments.result, bytes).map_err(|error| error.to_string())?;
    if arguments.mode != "missing-sentinel" {
        println!("{ORACLE_SUCCESS_SENTINEL}");
    }
    Ok(())
}

fn success_result(
    request: &OracleRequestV1,
    request_bytes: &[u8],
    source_revision: &str,
) -> OracleResultV1 {
    OracleResultV1 {
        schema_version: SCHEMA_VERSION,
        run_id: request.run_id.clone(),
        source_scenario_hash: request.source_scenario_hash.clone(),
        projection_hash: sha256_bytes(request_bytes),
        provenance: ProvenanceV1 {
            run_id: request.run_id.clone(),
            source_revision: source_revision.to_owned(),
            source_dirty: false,
            source_patch_digest: None,
            host: "native-test-process".to_owned(),
            adapter: "transport-fixture".to_owned(),
            backend: "scalar-cpu".to_owned(),
            driver: "rust".to_owned(),
            toolchain: env!("CARGO_PKG_RUST_VERSION").to_owned(),
            build_options: Vec::new(),
            runtime_settings: Vec::new(),
        },
        bodies: request.scenario.bodies.clone(),
        traces: Vec::new(),
        metrics: MetricsV1 {
            work: WorkVectorV1 {
                bodies_total: request.scenario.bodies.len() as u32,
                bodies_awake: request
                    .scenario
                    .bodies
                    .iter()
                    .filter(|body| !body.sleeping)
                    .count() as u32,
                circles: 0,
                rectangles: 0,
                candidate_pairs: 0,
                overlapping_pairs: 0,
                manifolds: request.scenario.manifolds.len() as u32,
                contact_points: request
                    .scenario
                    .manifolds
                    .iter()
                    .map(|manifold| manifold.points.len() as u32)
                    .sum(),
                solver_work: 0,
                dispatches: 0,
            },
            total_physics: None,
            cpu_encode_submit: None,
            capacities: Vec::new(),
            penetration_max: finite(0.0),
            jitter_rms: finite(0.0),
            energy_delta: finite(0.0),
        },
        failure: None,
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("fixture constants are finite")
}

struct Arguments {
    mode: String,
    source_revision: String,
    request: PathBuf,
    result: PathBuf,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut mode = None;
        let mut source_revision = None;
        let mut request = None;
        let mut result = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--mode" => mode = Some(value),
                "--source-revision" => source_revision = Some(value),
                "--request" => request = Some(Path::new(&value).to_owned()),
                "--result" => result = Some(Path::new(&value).to_owned()),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            mode: mode.ok_or_else(|| "missing --mode".to_owned())?,
            source_revision: source_revision
                .ok_or_else(|| "missing --source-revision".to_owned())?,
            request: request.ok_or_else(|| "missing --request".to_owned())?,
            result: result.ok_or_else(|| "missing --result".to_owned())?,
        })
    }
}
