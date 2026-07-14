use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use physbox_box2d_oracle::BOX2D_REVISION;
use physbox_contracts::{OracleRequestV1, SCHEMA_VERSION, canonical_hash};
use physbox_engine::{AnalyticFixtureKind, analytic_fixture_scenario};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleRunErrorKind, run_oracle, verify_clean_git_pin,
};

#[test]
fn clean_pinned_process_produces_fresh_world_and_narrowphase_results() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = verify_clean_git_pin(&repository.join("reference/box2d"), BOX2D_REVISION)
        .expect("clean pinned Box2D checkout");
    let artifact_root = temporary_root("box2d-process");

    let world = run_oracle(
        &config(&artifact_root, &source, "world"),
        &request("world-first", AnalyticFixtureKind::CircleFloor, 180),
    )
    .expect("accepted world authority result");
    assert!(world.artifacts.result.exists());
    assert!(
        !world
            .artifacts
            .directory
            .join("oracle-result.partial.json")
            .exists()
    );
    assert!(world.result.failure.is_none());
    assert!(world.result.bodies[0].sleeping);
    assert!((world.result.bodies[0].position.y - 0.5).abs() <= 1.0e-3);

    let replay = run_oracle(
        &config(&artifact_root, &source, "world"),
        &request("world-replay", AnalyticFixtureKind::CircleFloor, 180),
    )
    .expect("accepted deterministic replay");
    assert_eq!(world.result.bodies, replay.result.bodies);
    assert_eq!(world.result.manifolds, replay.result.manifolds);

    let narrowphase = run_oracle(
        &config(&artifact_root, &source, "narrowphase"),
        &request("narrowphase", AnalyticFixtureKind::BoxOnBox, 1),
    )
    .expect("accepted public narrowphase result");
    assert_eq!(narrowphase.result.manifolds.len(), 1);
    assert_eq!(narrowphase.result.manifolds[0].points.len(), 2);

    let mut unsupported = request("unsupported", AnalyticFixtureKind::CircleFloor, 1);
    unsupported.scenario.bodies[0].inverse_inertia =
        physbox_contracts::FiniteF32::new(0.0).unwrap();
    unsupported.source_scenario_hash = canonical_hash(&unsupported.scenario).unwrap();
    let error = run_oracle(&config(&artifact_root, &source, "world"), &unsupported)
        .expect_err("unsupported projections must not be accepted");
    assert_eq!(error.kind, OracleRunErrorKind::UnsupportedFeature);
    let rejected = error
        .artifact_directory
        .expect("failure artifact directory")
        .join("oracle-result.rejected.json");
    assert!(rejected.exists());
    assert!(!rejected.with_file_name("oracle-result.json").exists());

    fs::remove_dir_all(&artifact_root).expect("remove process-test artifacts");
}

fn request(run_id: &str, fixture: AnalyticFixtureKind, steps: u32) -> OracleRequestV1 {
    let scenario = analytic_fixture_scenario(fixture, steps).unwrap();
    OracleRequestV1 {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.to_owned(),
        source_scenario_hash: canonical_hash(&scenario).unwrap(),
        scenario,
    }
}

fn config(
    artifact_root: &Path,
    source: &physbox_lab::oracle_transport::OracleSourceIdentity,
    lane: &str,
) -> OracleRunConfig {
    OracleRunConfig {
        artifact_root: artifact_root.to_owned(),
        timeout: Duration::from_secs(10),
        source: source.clone(),
        process: OracleProcess {
            program: PathBuf::from(env!("CARGO_BIN_EXE_physbox-box2d-oracle")),
            arguments: vec![OsString::from("--lane"), OsString::from(lane)],
        },
    }
}

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "physbox5-{label}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&root);
    root
}
