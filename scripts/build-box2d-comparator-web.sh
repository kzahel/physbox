#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EMCC_BIN="${EMCC:-emcc}"
SOURCE_ROOT="$REPO_ROOT/third_party/box2d-comparator"
OUTPUT_ROOT="$SOURCE_ROOT/web"
SHIM="$REPO_ROOT/crates/physbox-box2d-comparator/src/native/comparator_shim.c"

command -v "$EMCC_BIN" >/dev/null || {
    echo "error: emcc is required; set EMCC to the pinned Emscripten 3.1.74 executable" >&2
    exit 1
}

mkdir -p "$OUTPUT_ROOT"
mapfile -t BOX2D_SOURCES < <(find "$SOURCE_ROOT/src" -maxdepth 1 -type f -name '*.c' | sort)

"$EMCC_BIN" \
    "${BOX2D_SOURCES[@]}" \
    "$SHIM" \
    -I"$SOURCE_ROOT/include" \
    -I"$SOURCE_ROOT/src" \
    -I"$REPO_ROOT/crates/physbox-box2d-comparator/src/native" \
    -O3 \
    -std=gnu17 \
    -DBOX2D_DISABLE_SIMD \
    -ffp-contract=off \
    --no-entry \
    -s ASSERTIONS=0 \
    -s ENVIRONMENT=web \
    -s EXPORT_ES6=1 \
    -s FILESYSTEM=0 \
    -s MALLOC=emmalloc \
    -s MODULARIZE=1 \
    -s ALLOW_MEMORY_GROWTH=1 \
    -s EXPORTED_RUNTIME_METHODS='["HEAPU8"]' \
    -s EXPORTED_FUNCTIONS='["_malloc","_free","_pb5_box2d_world_create","_pb5_box2d_world_destroy","_pb5_box2d_world_add_body","_pb5_box2d_world_remove_body","_pb5_box2d_world_apply_impulse","_pb5_box2d_world_begin_grab","_pb5_box2d_world_update_grab","_pb5_box2d_world_end_grab","_pb5_box2d_world_set_gravity","_pb5_box2d_world_step","_pb5_box2d_world_snapshot","_pb5_box2d_world_last_error"]' \
    -o "$OUTPUT_ROOT/box2d-comparator.mjs"

EMCC_VERSION="$("$EMCC_BIN" --version | head -n 1)"
MJS_SHA="$(sha256sum "$OUTPUT_ROOT/box2d-comparator.mjs" | cut -d' ' -f1)"
WASM_SHA="$(sha256sum "$OUTPUT_ROOT/box2d-comparator.wasm" | cut -d' ' -f1)"
LOADER_SHA="$(sha256sum "$OUTPUT_ROOT/loader.js" | cut -d' ' -f1)"
printf '%s\n' \
    'revision 3a4f0da8374af61293a03021c9a0b3ebcfe67948' \
    "compiler $EMCC_VERSION" \
    'configuration release-single-threaded-no-filesystem-memory-growth' \
    "sha256 $MJS_SHA  box2d-comparator.mjs" \
    "sha256 $WASM_SHA  box2d-comparator.wasm" \
    "sha256 $LOADER_SHA  loader.js" \
    > "$OUTPUT_ROOT/WEB-MANIFEST.txt"

echo "Box2D comparator web runtime ready at $OUTPUT_ROOT"
