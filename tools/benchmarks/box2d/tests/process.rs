use std::{fs, path::PathBuf, process::Command};

use physbox_box2d_benchmark::{Box2dBenchmarkResult, SUCCESS_SENTINEL};
use physbox_contracts::canonical_json;
use physbox_engine::authored_chain_scenario;

#[test]
fn optimized_process_publishes_one_fresh_atomic_result() {
    let root = std::env::temp_dir().join(format!(
        "physbox5-box2d-performance-process-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let request = root.join("scenario.json");
    let result = root.join("result.json");
    fs::write(
        &request,
        canonical_json(&authored_chain_scenario(64, 7).unwrap()).unwrap(),
    )
    .unwrap();
    let output = launch(&request, &result);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(SUCCESS_SENTINEL));
    assert!(result.exists());
    assert!(!result.with_extension("partial.json").exists());
    let published: Box2dBenchmarkResult =
        serde_json::from_slice(&fs::read(&result).unwrap()).unwrap();
    assert_eq!(published.worker_count, 4);
    assert_eq!(published.samples_seconds.len(), 2);
    assert!(published.contact_count_max > 0);

    let stale = launch(&request, &result);
    assert!(!stale.status.success());
    assert!(!String::from_utf8_lossy(&stale.stdout).contains(SUCCESS_SENTINEL));
    fs::remove_dir_all(root).unwrap();
}

fn launch(request: &PathBuf, result: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_physbox-box2d-benchmark"))
        .args(["--request"])
        .arg(request)
        .args(["--result"])
        .arg(result)
        .args([
            "--run-id",
            "box2d-process-test",
            "--workers",
            "4",
            "--warmup",
            "1",
            "--samples",
            "2",
        ])
        .output()
        .unwrap()
}
