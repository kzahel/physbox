use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use physbox_box2d_oracle::BOX2D_REVISION;
use physbox_contracts::{OracleRequestV1, SCHEMA_VERSION, canonical_hash};
use physbox_engine::{AnalyticFixtureKind, analytic_fixture_scenario};
use physbox_engine::{
    P5_2_CENTERED_STACK_EVIDENCE_STEPS, P5_2CollisionScenarioKind,
    p5_2_box2d_narrowphase_projection, p5_2_box2d_world_projection,
    p5_2_centered_rectangle_stack_scenario, p5_2_collision_scenario,
    p5_2_narrowphase_pair_scenario,
};
use physbox_lab::oracle_transport::{
    OracleProcess, OracleRunConfig, OracleRunErrorKind, run_oracle, verify_clean_git_pin,
};
use physbox_lab::{
    analytic_narrowphase::{compare_box2d_narrowphase, generate_scalar_narrowphase},
    collision_candidates::enumerate_scalar_candidates,
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

#[test]
fn clean_pinned_process_matches_all_governed_p5_2_manifolds() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source_identity = verify_clean_git_pin(&repository.join("reference/box2d"), BOX2D_REVISION)
        .expect("clean pinned Box2D checkout");
    let artifact_root = temporary_root("box2d-p5.2-narrowphase");
    for (index, kind) in P5_2CollisionScenarioKind::ALL.into_iter().enumerate() {
        let source = p5_2_collision_scenario(kind, 1, 17).unwrap();
        let projection = p5_2_box2d_narrowphase_projection(&source).unwrap();
        let request = OracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: format!("p5.2-narrowphase-{index}"),
            source_scenario_hash: canonical_hash(&projection).unwrap(),
            scenario: projection,
        };
        let authority = run_oracle(
            &config(&artifact_root, &source_identity, "narrowphase"),
            &request,
        )
        .expect("accepted public narrowphase authority result");
        let candidates = enumerate_scalar_candidates(&source).unwrap();
        let scalar = generate_scalar_narrowphase(&source, &candidates.candidates).unwrap();
        compare_box2d_narrowphase(&candidates.candidates, &scalar, &authority.result.manifolds)
            .unwrap_or_else(|difference| {
                panic!("{kind:?}: {}: {}", difference.signature, difference.reason)
            });
    }
    fs::remove_dir_all(&artifact_root).expect("remove process-test artifacts");
}

#[test]
fn clean_pinned_process_matches_randomized_p5_2_pair_families() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source_identity = verify_clean_git_pin(&repository.join("reference/box2d"), BOX2D_REVISION)
        .expect("clean pinned Box2D checkout");
    let artifact_root = temporary_root("box2d-p5.2-randomized-narrowphase");
    for seed in 0..8_u64 {
        for kind in P5_2CollisionScenarioKind::ALL[..3].iter().copied() {
            let source = p5_2_narrowphase_pair_scenario(kind, seed).unwrap();
            let projection = p5_2_box2d_narrowphase_projection(&source).unwrap();
            let request = OracleRequestV1 {
                schema_version: SCHEMA_VERSION,
                run_id: format!("p5.2-randomized-{}-{seed}", kind.name()),
                source_scenario_hash: canonical_hash(&projection).unwrap(),
                scenario: projection,
            };
            let authority = run_oracle(
                &config(&artifact_root, &source_identity, "narrowphase"),
                &request,
            )
            .expect("accepted randomized public narrowphase authority result");
            let candidates = enumerate_scalar_candidates(&source).unwrap();
            let scalar = generate_scalar_narrowphase(&source, &candidates.candidates).unwrap();
            assert!(!scalar.is_empty(), "{kind:?} seed {seed} did not overlap");
            compare_box2d_narrowphase(&candidates.candidates, &scalar, &authority.result.manifolds)
                .unwrap_or_else(|difference| {
                    panic!(
                        "{kind:?} seed {seed}: {}: {}",
                        difference.signature, difference.reason
                    )
                });
        }
    }
    fs::remove_dir_all(&artifact_root).expect("remove process-test artifacts");
}

#[test]
fn clean_pinned_process_replays_centered_rectangle_stack_projections() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source_identity = verify_clean_git_pin(&repository.join("reference/box2d"), BOX2D_REVISION)
        .expect("clean pinned Box2D checkout");
    let artifact_root = temporary_root("box2d-centered-stacks");
    for count in [2, 4, 8] {
        let source =
            p5_2_centered_rectangle_stack_scenario(count, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                .unwrap();
        let projection = p5_2_box2d_world_projection(&source).unwrap();
        let request = OracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: format!("centered-stack-{count}"),
            source_scenario_hash: canonical_hash(&projection).unwrap(),
            scenario: projection,
        };
        let authority = run_oracle(&config(&artifact_root, &source_identity, "world"), &request)
            .expect("accepted centered stack world authority result");
        assert!(authority.result.failure.is_none());
        assert_eq!(authority.result.bodies.len(), count as usize + 1);
        assert!(
            authority.result.bodies[1..]
                .windows(2)
                .all(|pair| { pair[0].position.y < pair[1].position.y })
        );
    }
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
