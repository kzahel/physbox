use std::{env, fs, path::PathBuf, process::Command};

const SIDECAR_REVISION: &str = "ef8d9526ae79377da3d610e5e4d3bd227f072cdb";
const SIDECAR_INTRODUCTION_REVISION: &str = "3884106dd415802d2139d163fca56e65b98645c6";
const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";
const BOX2CPP_REVISION: &str = "45f07785abe6bfeba518fb0256a37dcd050dce32";

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repository = manifest.join("../../..");
    let source = repository.join("reference/physbox3-particle-sidecar");
    let box2d = source.join("box2d");
    let box2cpp = source.join("box2cpp");
    let sidecar = source.join("box2d3-wasm/csrc/particle_sidecar");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let build = output.join("box2d-build");

    verify_clean_pin(&source, SIDECAR_REVISION, "PhysBox 3 particle sidecar");
    verify_clean_pin(&box2d, BOX2D_REVISION, "PhysBox 3 Box2D");
    verify_clean_pin(&box2cpp, BOX2CPP_REVISION, "box2cpp");
    verify_sidecar_unchanged(&source);
    fs::create_dir_all(&build).expect("create Box2D out-of-source build directory");

    run(
        Command::new("cmake")
            .arg("-S")
            .arg(&box2d)
            .arg("-B")
            .arg(&build)
            .args([
                "-DCMAKE_BUILD_TYPE=Release",
                "-DBUILD_SHARED_LIBS=OFF",
                "-DBOX2D_SAMPLES=OFF",
                "-DBOX2D_BENCHMARKS=OFF",
                "-DBOX2D_DOCS=OFF",
                "-DBOX2D_PROFILE=OFF",
                "-DBOX2D_UNIT_TESTS=OFF",
                "-DBOX2D_VALIDATE=ON",
                "-DBOX2D_DISABLE_SIMD=ON",
                "-DBOX2D_COMPILE_WARNING_AS_ERROR=OFF",
            ]),
        "configure pinned PhysBox 3 Box2D",
    );
    run(
        Command::new("cmake").arg("--build").arg(&build).args([
            "--config",
            "Release",
            "--target",
            "box2d",
            "--parallel",
            "2",
        ]),
        "build pinned PhysBox 3 Box2D",
    );

    let mut compiler = cc::Build::new();
    compiler
        .cpp(true)
        .std("c++20")
        .warnings(false)
        .include(box2d.join("include"))
        .include(box2cpp.join("include"))
        .include(&sidecar)
        .file(sidecar.join("particle_bridge.cpp"))
        .file(sidecar.join("particle_system.cpp"))
        .file(manifest.join("src/native/oracle_shim.cpp"));
    let compiler_path = compiler.get_compiler().path().to_owned();
    compiler.compile("physbox3_particle_oracle_shim");

    println!(
        "cargo:rustc-link-search=native={}",
        build.join("src").display()
    );
    println!("cargo:rustc-link-lib=static=box2d");
    println!("cargo:rustc-env=PHYSBOX3_SIDECAR_REVISION={SIDECAR_REVISION}");
    println!(
        "cargo:rustc-env=PHYSBOX3_SIDECAR_INTRODUCTION_REVISION={SIDECAR_INTRODUCTION_REVISION}"
    );
    println!("cargo:rustc-env=PHYSBOX3_BOX2D_REVISION={BOX2D_REVISION}");
    println!("cargo:rustc-env=PHYSBOX3_BOX2CPP_REVISION={BOX2CPP_REVISION}");
    println!(
        "cargo:rustc-env=CXX_VERSION={}",
        first_version_line(
            &mut Command::new(compiler_path),
            "query particle oracle C++ compiler"
        )
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/native/oracle_shim.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/native/oracle_shim.h").display()
    );
    println!("cargo:rerun-if-changed={}", sidecar.display());
    println!("cargo:rerun-if-changed={}", box2d.join("include").display());
    println!(
        "cargo:rerun-if-changed={}",
        box2cpp.join("include").display()
    );
}

fn verify_clean_pin(source: &PathBuf, expected: &str, name: &str) {
    let mut revision = Command::new("git");
    revision
        .args(["-C"])
        .arg(source)
        .args(["rev-parse", "HEAD"]);
    let actual = command_stdout(&mut revision, &format!("read {name} revision"));
    assert_eq!(
        actual, expected,
        "{name} is at {actual}, expected {expected}"
    );

    let mut status = Command::new("git");
    status
        .args(["-C"])
        .arg(source)
        .args(["status", "--porcelain=v1", "--untracked-files=all"]);
    let dirty = command_stdout(&mut status, &format!("read {name} status"));
    assert!(dirty.is_empty(), "{name} checkout is dirty:\n{dirty}");
}

fn verify_sidecar_unchanged(source: &PathBuf) {
    let mut diff = Command::new("git");
    diff.args(["-C"]).arg(source).args([
        "diff",
        "--quiet",
        SIDECAR_INTRODUCTION_REVISION,
        SIDECAR_REVISION,
        "--",
        "box2d3-wasm/csrc/particle_sidecar",
    ]);
    let status = diff
        .status()
        .expect("compare particle sidecar source revisions");
    assert!(
        status.success(),
        "particle sidecar changed after its recorded introduction revision"
    );
}

fn first_version_line(command: &mut Command, action: &str) -> String {
    command.arg("--version");
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{action}: {error}"));
    assert!(
        output.status.success(),
        "{action} failed with {}",
        output.status
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .expect("version command produced no output")
        .trim()
        .to_owned()
}

fn command_stdout(command: &mut Command, action: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{action}: {error}"));
    assert!(
        output.status.success(),
        "{action} failed with {}",
        output.status
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("{action}: {error}"));
    assert!(status.success(), "{action} failed with {status}");
}
