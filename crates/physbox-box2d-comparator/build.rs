use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const BOX2D_REVISION: &str = "3a4f0da8374af61293a03021c9a0b3ebcfe67948";

fn main() {
    println!("cargo:rustc-env=PHYSBOX5_BOX2D_REVISION={BOX2D_REVISION}");
    println!("cargo:rerun-if-changed=src/native/comparator_shim.c");
    println!("cargo:rerun-if-changed=src/native/comparator_shim.h");

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let vendor = manifest.join("../../third_party/box2d-comparator");
    let source_manifest =
        fs::read_to_string(vendor.join("SOURCE-MANIFEST.txt")).expect("read Box2D source manifest");
    assert!(
        source_manifest
            .lines()
            .any(|line| line == format!("revision {BOX2D_REVISION}")),
        "vendored Box2D source manifest does not record the selected revision"
    );
    verify_source_digests(&vendor, &source_manifest);

    println!(
        "cargo:rerun-if-changed={}",
        vendor.join("SOURCE-MANIFEST.txt").display()
    );

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        verify_web_digests(&vendor);
        return;
    }

    let mut build = cc::Build::new();
    build
        .std("gnu17")
        .include(vendor.join("include"))
        .include(vendor.join("src"))
        .include(manifest.join("src/native"))
        .define("BOX2D_DISABLE_SIMD", None)
        .warnings(false)
        .flag_if_supported("-ffp-contract=off");

    for entry in fs::read_dir(vendor.join("src")).expect("read vendored Box2D sources") {
        let path = entry.expect("read source entry").path();
        if path.extension().is_some_and(|extension| extension == "c") {
            build.file(path);
        }
    }
    build.file(manifest.join("src/native/comparator_shim.c"));
    build.compile("physbox5_box2d_comparator");

    println!(
        "cargo:rerun-if-changed={}",
        vendor.join("include").display()
    );
    println!("cargo:rerun-if-changed={}", vendor.join("src").display());
}

fn verify_source_digests(vendor: &Path, manifest: &str) {
    let mut verified = 0_u32;
    for line in manifest.lines() {
        let Some((expected, relative)) = line.split_once("  ") else {
            continue;
        };
        assert_eq!(
            expected.len(),
            64,
            "invalid digest in Box2D source manifest: {line}"
        );
        let bytes = fs::read(vendor.join(relative))
            .unwrap_or_else(|error| panic!("read vendored Box2D file {relative}: {error}"));
        let actual = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(
            actual, expected,
            "vendored Box2D file {relative} differs from its source manifest"
        );
        verified += 1;
    }
    assert_eq!(
        verified, 63,
        "Box2D source manifest must enumerate the exact 63-file comparator subset"
    );
}

fn verify_web_digests(vendor: &Path) {
    let web = vendor.join("web");
    let manifest_path = web.join("WEB-MANIFEST.txt");
    let manifest = fs::read_to_string(&manifest_path).expect("read Box2D web artifact manifest");
    assert!(
        manifest
            .lines()
            .any(|line| line == format!("revision {BOX2D_REVISION}")),
        "Box2D web artifact manifest does not record the selected revision"
    );
    assert!(
        manifest
            .lines()
            .any(|line| line.contains(" 3.1.74 ") || line.ends_with(" 3.1.74")),
        "Box2D web artifacts must record the pinned Emscripten 3.1.74 compiler"
    );

    let mut verified = 0_u32;
    for line in manifest.lines() {
        let Some(rest) = line.strip_prefix("sha256 ") else {
            continue;
        };
        let Some((expected, relative)) = rest.split_once("  ") else {
            panic!("invalid digest entry in Box2D web artifact manifest: {line}");
        };
        assert_eq!(
            expected.len(),
            64,
            "invalid digest in Box2D web artifact manifest: {line}"
        );
        let bytes = fs::read(web.join(relative))
            .unwrap_or_else(|error| panic!("read Box2D web artifact {relative}: {error}"));
        let actual = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(
            actual, expected,
            "Box2D web artifact {relative} differs from its manifest"
        );
        println!("cargo:rerun-if-changed={}", web.join(relative).display());
        verified += 1;
    }
    assert_eq!(
        verified, 3,
        "Box2D web artifact manifest must enumerate the module, wasm binary, and loader"
    );
    println!("cargo:rerun-if-changed={}", manifest_path.display());
}
