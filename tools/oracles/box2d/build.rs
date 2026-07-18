use std::{env, fs, path::PathBuf, process::Command};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repository = manifest.join("../../..");
    let source = repository.join("reference/box2d");
    let include = source.join("include");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let build = output.join("box2d-build");
    verify_clean_pin(&source);
    fs::create_dir_all(&build).expect("create Box2D out-of-source build directory");

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
                "-DBOX2D_VALIDATE=ON",
                "-DBOX2D_DISABLE_SIMD=ON",
                "-DBOX2D_COMPILE_WARNING_AS_ERROR=OFF",
            ]),
        "configure pinned Box2D",
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
        "build pinned Box2D",
    );
    let cmake_version = first_version_line(&mut Command::new("cmake"), "query CMake version");
    let compiler = cmake_c_compiler(&build.join("CMakeCache.txt"));
    let compiler_version = first_version_line(
        &mut Command::new(&compiler),
        "query the C compiler selected by CMake",
    );

    let bindings = bindgen::Builder::default()
        .header(include.join("box2d/box2d.h").display().to_string())
        .clang_arg(format!("-I{}", include.display()))
        .allowlist_function("b2(DefaultWorldDef|CreateWorld|DestroyWorld|World_Step|World_SetGravity|World_GetCounters|World_EnableWarmStarting|World_IsWarmStartingEnabled)")
        .allowlist_function("b2(DefaultBodyDef|CreateBody|DestroyBody|Body_GetPosition|Body_GetRotation|Body_GetLinearVelocity|Body_GetAngularVelocity|Body_GetLocalPoint|Body_GetMassData|Body_SetMassData|Body_SetTargetTransform|Body_IsAwake|Body_ApplyLinearImpulse|Body_GetContactCapacity|Body_GetContactData)")
        .allowlist_function("b2(DefaultShapeDef|CreateCircleShape|CreatePolygonShape|MakeBox)")
        .allowlist_function("b2(DefaultMotorJointDef|CreateMotorJoint|DestroyJoint)")
        .allowlist_function("b2(CollideCircles|CollidePolygonAndCircle|CollidePolygons|ComputeCosSin|Atan2)")
        .allowlist_type("b2(WorldDef|WorldId|BodyDef|BodyId|ShapeDef|ShapeId|MassData|JointDef|MotorJointDef|JointId|Vec2|Rot|Transform|Circle|Polygon|Manifold|ManifoldPoint|ContactData|ContactId|Counters|BodyType)")
        .allowlist_var("b2_(staticBody|kinematicBody|dynamicBody)")
        .derive_debug(true)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("generate direct Box2D C FFI bindings");
    bindings
        .write_to_file(output.join("box2d_bindings.rs"))
        .expect("write Box2D bindings");

    println!(
        "cargo:rustc-link-search=native={}",
        build.join("src").display()
    );
    println!("cargo:rustc-link-lib=static=box2d");
    println!("cargo:rustc-env=PHYSBOX5_BOX2D_REVISION={BOX2D_REVISION}");
    let rustc = Command::new(env::var_os("RUSTC").unwrap())
        .arg("--version")
        .output()
        .expect("query rustc version");
    println!(
        "cargo:rustc-env=RUSTC_VERSION={}",
        String::from_utf8_lossy(&rustc.stdout).trim()
    );
    println!("cargo:rustc-env=CMAKE_VERSION={cmake_version}");
    println!("cargo:rustc-env=C_COMPILER_VERSION={compiler_version}");
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
    let mut revision = Command::new("git");
    revision
        .args(["-C"])
        .arg(source)
        .args(["rev-parse", "HEAD"]);
    let actual = command_stdout(&mut revision, "read Box2D source revision");
    assert_eq!(
        actual, BOX2D_REVISION,
        "Box2D checkout is at {actual}, expected {BOX2D_REVISION}"
    );

    let mut status = Command::new("git");
    status
        .args(["-C"])
        .arg(source)
        .args(["status", "--porcelain=v1", "--untracked-files=all"]);
    let dirty = command_stdout(&mut status, "read Box2D source status");
    assert!(
        dirty.is_empty(),
        "Box2D authority checkout is dirty:\n{dirty}"
    );
}

fn cmake_c_compiler(cache: &PathBuf) -> PathBuf {
    let contents = fs::read_to_string(cache).expect("read configured Box2D CMake cache");
    let value = contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("CMAKE_C_COMPILER:FILEPATH=")
                .or_else(|| line.strip_prefix("CMAKE_C_COMPILER:STRING="))
        })
        .expect("find the configured Box2D C compiler");
    PathBuf::from(value)
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
