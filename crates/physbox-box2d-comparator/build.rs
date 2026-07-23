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

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }

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
    println!(
        "cargo:rerun-if-changed={}",
        vendor.join("SOURCE-MANIFEST.txt").display()
    );
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
