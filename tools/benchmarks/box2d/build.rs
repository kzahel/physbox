use std::{env, fs, path::PathBuf, process::Command};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repository = manifest.join("../../..");
    let source = repository.join("reference/box2d");
    let include = source.join("include");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let build = output.join("box2d-performance-build");
    verify_clean_pin(&source);
    fs::create_dir_all(&build).expect("create optimized Box2D build directory");
    run(
        Command::new("cmake")
            .arg("-S")
            .arg(&source)
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
                "-DBOX2D_VALIDATE=OFF",
                "-DBOX2D_DISABLE_SIMD=OFF",
                "-DBOX2D_COMPILE_WARNING_AS_ERROR=OFF",
            ]),
        "configure optimized pinned Box2D",
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
        "build optimized pinned Box2D",
    );
    cc::Build::new()
        .cpp(true)
        .file(manifest.join("src/perf_scheduler.cpp"))
        .include(&include)
        .flag_if_supported("-std=c++17")
        .compile("physbox_box2d_perf_scheduler");
    let bindings = bindgen::Builder::default()
        .header(manifest.join("src/wrapper.h").display().to_string())
        .clang_arg(format!("-I{}", include.display()))
        .clang_arg(format!("-I{}", manifest.join("src").display()))
        .allowlist_function("b2(DefaultWorldDef|CreateWorld|DestroyWorld|World_Step|World_GetCounters)")
        .allowlist_function("b2(DefaultBodyDef|CreateBody|Body_GetPosition|Body_GetRotation|Body_GetLinearVelocity|Body_GetAngularVelocity|Body_SetMassData)")
        .allowlist_function("b2(DefaultShapeDef|CreateCircleShape|CreatePolygonShape|MakeBox)")
        .allowlist_function("physbox_.*")
        .allowlist_type("b2(WorldDef|WorldId|BodyDef|BodyId|ShapeDef|ShapeId|MassData|Vec2|Rot|Circle|Polygon|Counters|BodyType|TaskCallback)")
        .allowlist_type("physbox_scheduler")
        .allowlist_var("b2_(staticBody|dynamicBody)")
        .derive_debug(true)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("generate optimized Box2D benchmark bindings");
    bindings
        .write_to_file(output.join("box2d_benchmark_bindings.rs"))
        .expect("write optimized Box2D benchmark bindings");
    println!(
        "cargo:rustc-link-search=native={}",
        build.join("src").display()
    );
    println!("cargo:rustc-link-lib=static=box2d");
    println!("cargo:rustc-env=PHYSBOX5_BOX2D_REVISION={BOX2D_REVISION}");
    println!(
        "cargo:rustc-env=PHYSBOX5_RUSTC_VERSION={}",
        first_version_line(Command::new(env::var_os("RUSTC").unwrap()), "query rustc")
    );
    println!(
        "cargo:rustc-env=PHYSBOX5_CMAKE_VERSION={}",
        first_version_line(Command::new("cmake"), "query CMake")
    );
    let compiler = cmake_compiler(&build.join("CMakeCache.txt"));
    println!(
        "cargo:rustc-env=PHYSBOX5_CXX_VERSION={}",
        first_version_line(Command::new(compiler), "query C++ compiler")
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/perf_scheduler.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/perf_scheduler.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/wrapper.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source.join("CMakeLists.txt").display()
    );
    println!("cargo:rerun-if-changed={}", source.join("src").display());
    println!("cargo:rerun-if-changed={}", include.display());
    println!(
        "cargo:rerun-if-changed={}",
        source.join(".git/HEAD").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source.join(".git/index").display()
    );
}

fn verify_clean_pin(source: &PathBuf) {
    let actual = command_stdout(
        Command::new("git")
            .args(["-C"])
            .arg(source)
            .args(["rev-parse", "HEAD"]),
        "read Box2D source revision",
    );
    assert_eq!(
        actual, BOX2D_REVISION,
        "optimized Box2D source pin mismatch"
    );
    let dirty = command_stdout(
        Command::new("git").args(["-C"]).arg(source).args([
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        ]),
        "read Box2D source status",
    );
    assert!(dirty.is_empty(), "Box2D source checkout is dirty:\n{dirty}");
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

fn first_version_line(mut command: Command, action: &str) -> String {
    command.arg("--version");
    command_stdout(&mut command, action)
        .lines()
        .next()
        .expect("version command produced no output")
        .to_owned()
}

fn cmake_compiler(cache: &PathBuf) -> PathBuf {
    let contents = fs::read_to_string(cache).expect("read Box2D CMake cache");
    contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("CMAKE_CXX_COMPILER:FILEPATH=")
                .or_else(|| line.strip_prefix("CMAKE_CXX_COMPILER:STRING="))
        })
        .map(PathBuf::from)
        .expect("find Box2D C++ compiler")
}

fn run(command: &mut Command, action: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("{action}: {error}"));
    assert!(status.success(), "{action} failed with {status}");
}
