use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use physbox_contracts::{
    OracleRequestV1, SCHEMA_VERSION, canonical_hash, canonical_json, sha256_bytes,
};
use physbox_engine::authored_one_contact_scenario;
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleRunErrorKind, OracleSourceIdentity, run_oracle,
    verify_clean_git_pin,
};

const SOURCE_REVISION: &str = "fixture-source-revision";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[test]
fn successful_process_publishes_one_fresh_atomic_result() {
    let root = TempRoot::new("success");
    let request = request("run-success");
    let run = run_oracle(&config(root.path(), "success", 10_000), &request).unwrap();
    assert_eq!(run.result.run_id, request.run_id);
    assert_eq!(
        run.projection_hash,
        sha256_bytes(&canonical_json(&request).unwrap())
    );
    assert!(run.artifacts.result.is_file());
    assert!(
        !run.artifacts
            .directory
            .join("oracle-result.partial.json")
            .exists()
    );
    assert!(
        !run.artifacts
            .directory
            .join("oracle-result.rejected.json")
            .exists()
    );

    let stale = run_oracle(&config(root.path(), "success", 10_000), &request).unwrap_err();
    assert_eq!(stale.kind, OracleRunErrorKind::StaleResult);
    assert_eq!(
        fs::read(&run.artifacts.result).unwrap(),
        canonical_json(&run.result).unwrap()
    );
}

#[test]
fn request_and_result_identity_negatives_never_publish_an_accepted_result() {
    let root = TempRoot::new("identity-negatives");
    let mut invalid_request = request("bad-source-request");
    invalid_request.source_scenario_hash = "0".repeat(64);
    let error = run_oracle(&config(root.path(), "success", 10_000), &invalid_request).unwrap_err();
    assert_eq!(error.kind, OracleRunErrorKind::InvalidRequest);
    assert!(!root.path().join("bad-source-request").exists());

    for (mode, kind) in [
        ("stale", OracleRunErrorKind::StaleResult),
        ("wrong-source-hash", OracleRunErrorKind::HashMismatch),
        ("wrong-projection-hash", OracleRunErrorKind::HashMismatch),
        ("wrong-pin", OracleRunErrorKind::WrongSourcePin),
        ("unsupported", OracleRunErrorKind::UnsupportedFeature),
    ] {
        let run_id = format!("negative-{mode}");
        let error = run_oracle(&config(root.path(), mode, 10_000), &request(&run_id)).unwrap_err();
        assert_eq!(error.kind, kind, "mode {mode}");
        let directory = error.artifact_directory.unwrap();
        assert!(
            !directory.join("oracle-result.json").exists(),
            "mode {mode}"
        );
        assert!(
            directory.join("oracle-result.rejected.json").exists(),
            "mode {mode} did not preserve its rejected artifact"
        );
    }
}

#[test]
fn transport_negatives_reject_partial_missing_sentinel_and_timeout() {
    let root = TempRoot::new("transport-negatives");
    for (mode, kind, timeout_ms) in [
        ("partial", OracleRunErrorKind::PartialResult, 10_000),
        (
            "missing-sentinel",
            OracleRunErrorKind::MissingSentinel,
            10_000,
        ),
        ("timeout", OracleRunErrorKind::Timeout, 25),
    ] {
        let run_id = format!("transport-{mode}");
        let error =
            run_oracle(&config(root.path(), mode, timeout_ms), &request(&run_id)).unwrap_err();
        assert_eq!(error.kind, kind, "mode {mode}");
        let directory = error.artifact_directory.unwrap();
        assert!(
            !directory.join("oracle-result.json").exists(),
            "mode {mode}"
        );
    }
}

#[test]
fn clean_pin_check_rejects_a_wrong_revision_before_launch() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let error = verify_clean_git_pin(&repository, &"0".repeat(40)).unwrap_err();
    assert_eq!(error.kind, OracleRunErrorKind::WrongSourcePin);
    assert!(error.artifact_directory.is_none());
}

fn request(run_id: &str) -> OracleRequestV1 {
    let scenario = authored_one_contact_scenario(17);
    OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.to_owned(),
        source_scenario_hash: canonical_hash(&scenario).unwrap(),
        scenario,
    }
}

fn config(root: &Path, mode: &str, timeout_ms: u64) -> OracleRunConfig {
    OracleRunConfig {
        artifact_root: root.to_owned(),
        timeout: Duration::from_millis(timeout_ms),
        source: OracleSourceIdentity {
            revision: SOURCE_REVISION.to_owned(),
            dirty: false,
            patch_digest: None,
        },
        process: OracleProcess {
            program: PathBuf::from(env!("CARGO_BIN_EXE_physbox-oracle-fixture")),
            arguments: vec![
                OsString::from("--mode"),
                OsString::from(mode),
                OsString::from("--source-revision"),
                OsString::from(SOURCE_REVISION),
            ],
        },
    }
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "physbox5-oracle-transport-{label}-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
