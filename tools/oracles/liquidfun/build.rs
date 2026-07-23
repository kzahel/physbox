use std::{env, fs, path::PathBuf, process::Command};

const LIQUIDFUN_REVISION: &str = "7f20402173fd143a3988c921bc384459c6a858f2";

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repository = manifest.join("../../..");
    let source = repository.join("../physbox3/reference/liquidfun");
    let box2d = source.join("liquidfun/Box2D");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let build = output.join("liquidfun-build");

    verify_clean_pin(&source, LIQUIDFUN_REVISION);
    fs::create_dir_all(&build).expect("create LiquidFun out-of-source build directory");
    run(
        Command::new("cmake")
            .arg("-S")
            .arg(&box2d)
            .arg("-B")
            .arg(&build)
            .args([
                "-DCMAKE_BUILD_TYPE=Release",
                "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",
                "-DCMAKE_CXX_FLAGS=-Wno-error=class-memaccess",
                "-DBOX2D_BUILD_SHARED=OFF",
                "-DBOX2D_BUILD_STATIC=ON",
                "-DBOX2D_BUILD_EXAMPLES=OFF",
                "-DBOX2D_BUILD_UNITTESTS=OFF",
                "-DBOX2D_INSTALL=OFF",
                "-DBOX2D_INSTALL_DOC=OFF",
            ]),
        "configure pinned original LiquidFun",
    );
    run(
        Command::new("cmake").arg("--build").arg(&build).args([
            "--config",
            "Release",
            "--target",
            "Box2D",
            "--parallel",
            "2",
        ]),
        "build pinned original LiquidFun",
    );

    let mut compiler = cc::Build::new();
    compiler
        .cpp(true)
        .std("c++11")
        .warnings(false)
        .define("LIQUIDFUN_EXTERNAL_LANGUAGE_API", "1")
        .include(&box2d)
        .file(manifest.join("src/native/oracle_shim.cpp"));
    let compiler_path = compiler.get_compiler().path().to_owned();
    compiler.compile("liquidfun_oracle_shim");

    for directory in [
        build.join("Box2D/Release"),
        build.join("Box2D"),
        build.join("Release"),
        build.clone(),
    ] {
        println!("cargo:rustc-link-search=native={}", directory.display());
    }
    println!("cargo:rustc-link-lib=static=liquidfun");
    println!("cargo:rustc-env=LIQUIDFUN_REVISION={LIQUIDFUN_REVISION}");
    println!(
        "cargo:rustc-env=CXX_VERSION={}",
        first_version_line(
            &mut Command::new(compiler_path),
            "query LiquidFun oracle C++ compiler"
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
    println!("cargo:rerun-if-changed={}", box2d.join("Box2D").display());
}

fn verify_clean_pin(source: &PathBuf, expected: &str) {
    let mut revision = Command::new("git");
    revision
        .args(["-C"])
        .arg(source)
        .args(["rev-parse", "HEAD"]);
    let actual = command_stdout(&mut revision, "read LiquidFun revision");
    assert_eq!(
        actual, expected,
        "LiquidFun is at {actual}, expected {expected}"
    );

    let mut status = Command::new("git");
    status
        .args(["-C"])
        .arg(source)
        .args(["status", "--porcelain=v1", "--untracked-files=all"]);
    let dirty = command_stdout(&mut status, "read LiquidFun status");
    assert!(dirty.is_empty(), "LiquidFun checkout is dirty:\n{dirty}");
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
