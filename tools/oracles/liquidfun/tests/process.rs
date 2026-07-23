use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use physbox_contracts::{
    CoupledOracleRequestV1, CoupledOracleResultV1, ORACLE_SUCCESS_SENTINEL, SCHEMA_VERSION,
    Validate, canonical_hash, canonical_json,
};
use physbox_engine::{CoupledScenarioKind, canonical_coupled_scenario};

#[test]
fn native_process_emits_fresh_valid_result_and_sentinel() {
    let scenario = canonical_coupled_scenario(CoupledScenarioKind::Displacement).unwrap();
    let request = CoupledOracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: format!(
            "liquidfun-process-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ),
        source_scenario_hash: canonical_hash(&scenario).unwrap(),
        scenario,
    };
    let directory = std::env::temp_dir().join(&request.run_id);
    fs::create_dir(&directory).unwrap();
    let request_path = directory.join("request.json");
    let result_path = directory.join("result.json");
    fs::write(&request_path, canonical_json(&request).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_physbox-liquidfun-oracle"))
        .args(["--request"])
        .arg(&request_path)
        .args(["--result"])
        .arg(&result_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(ORACLE_SUCCESS_SENTINEL),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let result: CoupledOracleResultV1 =
        serde_json::from_slice(&fs::read(&result_path).unwrap()).unwrap();
    result.validate().unwrap();
    assert_eq!(result.run_id, request.run_id);
    assert_eq!(result.source_scenario_hash, request.source_scenario_hash);
    assert!(result.failure.is_none());
    fs::remove_dir_all(directory).unwrap();
}
