#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"
SERIAL="${PHYSBOX5_ANDROID_SERIAL:-}"
OUTPUT="${PHYSBOX5_ANDROID_PACING_OUTPUT:-/tmp/physbox5-android-pacing.json}"
WARMUP_SECONDS=4
SAMPLE_SECONDS=6
MIN_PRESENTED_FPS=55
MAX_P95_MILLISECONDS=20
MIN_SIMULATION_SCALE=0.90
SKIP_BUILD=0
while (($#)); do
    case "$1" in
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --warmup-seconds) WARMUP_SECONDS="${2:?--warmup-seconds requires a value}"; shift 2 ;;
        --sample-seconds) SAMPLE_SECONDS="${2:?--sample-seconds requires a value}"; shift 2 ;;
        --min-presented-fps) MIN_PRESENTED_FPS="${2:?--min-presented-fps requires a value}"; shift 2 ;;
        --max-p95-ms) MAX_P95_MILLISECONDS="${2:?--max-p95-ms requires a value}"; shift 2 ;;
        --min-simulation-scale) MIN_SIMULATION_SCALE="${2:?--min-simulation-scale requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        *)
            echo "Usage: $0 [--serial SERIAL] [--output PATH] [--skip-build] [--warmup-seconds N] [--sample-seconds N] [--min-presented-fps N] [--max-p95-ms N] [--min-simulation-scale N]" >&2
            exit 2
            ;;
    esac
done
for VALUE in "$WARMUP_SECONDS" "$SAMPLE_SECONDS" "$MIN_PRESENTED_FPS" "$MAX_P95_MILLISECONDS" "$MIN_SIMULATION_SCALE"; do
    [[ "$VALUE" =~ ^[0-9]+([.][0-9]+)?$ ]] || { echo "error: pacing limits must be nonnegative numbers" >&2; exit 2; }
done
node -e '
  const [warmup, sample, fps, p95, scale] = process.argv.slice(1).map(Number);
  if (!(warmup >= 1 && sample >= 3 && fps > 0 && p95 > 0 && scale > 0 && scale <= 1)) process.exit(1);
' "$WARMUP_SECONDS" "$SAMPLE_SECONDS" "$MIN_PRESENTED_FPS" "$MAX_P95_MILLISECONDS" "$MIN_SIMULATION_SCALE" || {
    echo "error: warmup must be >=1, sample >=3, and pacing limits must be positive" >&2
    exit 2
}

if [[ -z "$SERIAL" ]]; then
    SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"
fi
[[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
"$ADB" -s "$SERIAL" get-state 2>/dev/null | rg -qx device || {
    echo "error: Android device $SERIAL is unavailable or unauthorized" >&2
    exit 1
}
if "$ADB" -s "$SERIAL" shell dumpsys window 2>/dev/null | rg -q 'mDreamingLockscreen=true|mShowingLockscreen=true'; then
    echo "error: Android device $SERIAL is locked; unlock it before pacing validation" >&2
    exit 1
fi

ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    PHYSBOX5_ANDROID_ABI="$ABI" "$ANDROID_DIR/build-apk.sh"
fi
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[[ -f "$APK" ]] || { echo "error: APK missing: $APK" >&2; exit 1; }
rm -f "$OUTPUT" "$OUTPUT.tmp"
PACING_TEMP="$(mktemp -d /tmp/physbox5-android-pacing.XXXXXX)"
restore_product_launch() {
    "$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5 >/dev/null 2>&1 || true
    "$ADB" -s "$SERIAL" shell "am start -n com.kzahel.physbox5/android.app.NativeActivity --es physbox5.startup.argv '[\"--scenario\",\"bootstrap\"]'" >/dev/null 2>&1 || true
    rm -rf "$PACING_TEMP"
}
trap restore_product_launch EXIT

"$ADB" -s "$SERIAL" install -r "$APK" >/dev/null

launch_scenario() {
    local scenario="$1"
    local markers="$2"
    "$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5
    "$ADB" -s "$SERIAL" shell "am start -n com.kzahel.physbox5/android.app.NativeActivity --es physbox5.startup.argv '[\"--scenario\",\"$scenario\",\"--frame-markers\",\"$markers\"]'" >/dev/null
}

find_surface_layer() {
    local layer=""
    for _ in $(seq 1 40); do
        layer="$($ADB -s "$SERIAL" shell dumpsys SurfaceFlinger --list | tr -d '\r' | sed -n 's/^RequestedLayerState{\(com\.kzahel\.physbox5\/android\.app\.NativeActivity#[0-9]*\) .*/\1/p' | tail -1)"
        if [[ -n "$layer" ]]; then
            printf '%s\n' "$layer"
            return 0
        fi
        sleep 0.25
    done
    return 1
}

capture_presentation() {
    local scenario="$1"
    local stem="$2"
    launch_scenario "$scenario" off
    sleep "$WARMUP_SECONDS"
    local layer
    layer="$(find_surface_layer)" || { echo "error: SurfaceFlinger layer missing for $scenario" >&2; return 1; }
    "$ADB" -s "$SERIAL" shell dumpsys SurfaceFlinger --latency-clear "$layer" >/dev/null
    sleep "$SAMPLE_SECONDS"
    "$ADB" -s "$SERIAL" shell dumpsys SurfaceFlinger --latency "$layer" | tr -d '\r' > "$PACING_TEMP/$stem.txt"
    node "$ANDROID_DIR/summarize-surface-latency.mjs" < "$PACING_TEMP/$stem.txt" > "$PACING_TEMP/$stem.json"
}

capture_presentation body-free-flight control
capture_presentation bootstrap product

launch_scenario bootstrap all
sleep "$WARMUP_SECONDS"
"$ADB" -s "$SERIAL" logcat -c
sleep "$SAMPLE_SECONDS"
"$ADB" -s "$SERIAL" logcat -v epoch -d -t 10000 > "$PACING_TEMP/frame-markers.txt"
node "$ANDROID_DIR/summarize-frame-markers.mjs" < "$PACING_TEMP/frame-markers.txt" > "$PACING_TEMP/physics.json"

REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false
[[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
RUN_ID="android-pacing-$(date +%s)-$$"
node -e '
  const fs = require("fs");
  const [path, controlPath, productPath, physicsPath, runId, revision, dirty, serial, model, product, warmup, sample, minFps, maxP95, minScale] = process.argv.slice(1);
  const control = JSON.parse(fs.readFileSync(controlPath));
  const presentation = JSON.parse(fs.readFileSync(productPath));
  const physics = JSON.parse(fs.readFileSync(physicsPath));
  const thresholds = {
    minimumPresentedFramesPerSecond: Number(minFps),
    maximumP95Milliseconds: Number(maxP95),
    minimumEffectiveSimulationScale: Number(minScale),
  };
  const failures = [];
  for (const [label, summary] of [["control", control], ["product", presentation]]) {
    if (summary.meanFramesPerSecond < thresholds.minimumPresentedFramesPerSecond) failures.push(`${label} mean presentation ${summary.meanFramesPerSecond.toFixed(2)} FPS is below ${thresholds.minimumPresentedFramesPerSecond}`);
    if (summary.p95Milliseconds > thresholds.maximumP95Milliseconds) failures.push(`${label} presentation p95 ${summary.p95Milliseconds.toFixed(2)} ms exceeds ${thresholds.maximumP95Milliseconds} ms`);
  }
  if (physics.effectiveSimulationScale < thresholds.minimumEffectiveSimulationScale) failures.push(`physics scale ${physics.effectiveSimulationScale.toFixed(3)} is below ${thresholds.minimumEffectiveSimulationScale}`);
  if (physics.finalReport?.sceneContract?.n !== "bootstrap") failures.push("physics marker did not report bootstrap");
  const report = {
    schemaVersion: 1,
    runId,
    sourceRevision: revision,
    sourceDirty: dirty === "true",
    host: "android-physical",
    device: { serial, model, product },
    measurement: { warmupSeconds: Number(warmup), sampleSeconds: Number(sample), frameMarkers: "off for presentation; all for physics pacing" },
    thresholds,
    control: { scenario: "body-free-flight", presentation: control },
    product: { scenario: "bootstrap", presentation, physics },
    passed: failures.length === 0,
    failures,
  };
  fs.writeFileSync(`${path}.tmp`, `${JSON.stringify(report, null, 2)}\n`);
  fs.renameSync(`${path}.tmp`, path);
' "$OUTPUT" "$PACING_TEMP/control.json" "$PACING_TEMP/product.json" "$PACING_TEMP/physics.json" \
  "$RUN_ID" "$REVISION" "$DIRTY" "$SERIAL" "$MODEL" "$PRODUCT" "$WARMUP_SECONDS" "$SAMPLE_SECONDS" \
  "$MIN_PRESENTED_FPS" "$MAX_P95_MILLISECONDS" "$MIN_SIMULATION_SCALE"

if ! node -e 'const report=require(process.argv[1]); process.exit(report.passed ? 0 : 1)' "$OUTPUT"; then
    node -e 'const report=require(process.argv[1]); for (const failure of report.failures) console.error(`error: ${failure}`)' "$OUTPUT"
    echo "error: physical Android pacing validation failed; see $OUTPUT" >&2
    exit 1
fi
echo "PHYSBOX5_VALIDATION_SUCCESS host=android-physical model=$MODEL pacing=$OUTPUT"
