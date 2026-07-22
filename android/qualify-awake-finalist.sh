#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"

NATIVE_EVIDENCE=""
OUTPUT_DIRECTORY="/tmp/physbox5-s25-android"
SKIP_BUILD=0
while (($#)); do
    case "$1" in
        --native-evidence) NATIVE_EVIDENCE="${2:?--native-evidence requires a path}"; shift 2 ;;
        --output-directory) OUTPUT_DIRECTORY="${2:?--output-directory requires a path}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        *) echo "usage: $0 --native-evidence PATH [--output-directory PATH] [--skip-build]" >&2; exit 2 ;;
    esac
done
[[ -n "$NATIVE_EVIDENCE" && -f "$NATIVE_EVIDENCE" ]] || {
    echo "error: --native-evidence must name the clean native package" >&2
    exit 2
}

REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD)"
[[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ]] || {
    echo "error: Android finalist qualification requires a clean source worktree" >&2
    exit 1
}
node -e '
  const fs = require("fs");
  const evidence = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
  if (evidence.passed !== true || evidence.sourceDirty !== false
      || evidence.sourceRevision !== process.argv[2]
      || evidence.physicsConfiguration?.substeps !== 8
      || evidence.physicsConfiguration?.sleepingEnabled !== false
      || ![2, 4, 8, 12, 14, 16].every((count) => evidence.rows.some((row) => row.rectangleCount === count))) {
    throw new Error("native evidence does not qualify this clean source revision and finalist configuration");
  }
' "$NATIVE_EVIDENCE" "$REVISION"

physbox5_android_detect_sdk
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"
SERIAL="${PHYSBOX5_ANDROID_SERIAL:-}"
if [[ -z "$SERIAL" ]]; then
    SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"
fi
[[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
"$ADB" -s "$SERIAL" get-state 2>/dev/null | rg -qx device || {
    echo "error: Android device $SERIAL is unavailable or unauthorized" >&2
    exit 1
}
if "$ADB" -s "$SERIAL" shell dumpsys window 2>/dev/null | rg -q 'mDreamingLockscreen=true|mShowingLockscreen=true'; then
    echo "error: Android device $SERIAL is locked; unlock it before validation" >&2
    exit 1
fi
cleanup() {
    physbox5_android_stop_product_app "$ADB" "$SERIAL"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

mkdir -p "$OUTPUT_DIRECTORY"
ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    PHYSBOX5_SOLVER_LAB=1 PHYSBOX5_ANDROID_ABI="$ABI" "$ANDROID_DIR/build-apk.sh"
fi
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[[ -f "$APK" ]] || { echo "error: APK missing: $APK" >&2; exit 1; }
"$ADB" -s "$SERIAL" install -r "$APK" >/dev/null

PHYSICS_HASH="$(node -e 'const e=require(process.argv[1]); console.log(e.physicsConfigurationHash)' "$NATIVE_EVIDENCE")"
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
ROW_PATHS=()
for COUNT in 2 4 8 12 14 16; do
    SCENARIO="solver-lab-stack-$COUNT"
    LOG="$OUTPUT_DIRECTORY/android-$SCENARIO-logcat.txt"
    SCREENSHOT="$OUTPUT_DIRECTORY/android-$SCENARIO.png"
    ROW="$OUTPUT_DIRECTORY/android-$SCENARIO.json"
    rm -f "$LOG" "$SCREENSHOT" "$ROW"
    EXPECTED_SCENARIO_HASH="$(node -e '
      const e=require(process.argv[1]);
      const row=e.rows.find((value) => value.rectangleCount === Number(process.argv[2]));
      if (!row) process.exit(1);
      console.log(row.sourceScenarioHash);
    ' "$NATIVE_EVIDENCE" "$COUNT")"
    "$ADB" -s "$SERIAL" logcat -c
    "$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5
    STARTUP_ARGV="[\"--scenario\",\"$SCENARIO\",\"--solver\",\"soft-coupled\",\"--sim-hz\",\"60\",\"--substeps\",\"8\",\"--iterations\",\"1\",\"--contact-hz\",\"30\",\"--contact-damping\",\"10\",\"--static-contact-scale\",\"2\",\"--speculative-distance\",\"0.02\",\"--max-depenetration-velocity\",\"3\",\"--sleeping\",\"off\",\"--frame-markers\",\"all\"]"
    "$ADB" -s "$SERIAL" shell "am start -n com.kzahel.physbox5/android.app.NativeActivity --es physbox5.startup.argv '$STARTUP_ARGV'" >/dev/null
    PASSED=0
    for _ in $(seq 1 55); do
        "$ADB" -s "$SERIAL" logcat -d -t 20000 > "$LOG"
        if rg -q "PHYSBOX5_MARK_FRAME_RENDERED .*\"name\":\"$SCENARIO\",\"complete\":true,\"passed\":true" "$LOG"; then
            PASSED=1
            break
        fi
        if rg -q 'PHYSBOX5_ANDROID_(STARTUP|RENDER)_ERROR' "$LOG"; then break; fi
        sleep 1
    done
    [[ "$PASSED" -eq 1 ]] || { echo "error: Android $SCENARIO did not complete; see $LOG" >&2; exit 1; }
    REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
    ADAPTER="$(rg 'PHYSBOX5_MARK_ADAPTER ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_ADAPTER //')"
    [[ -n "$ADAPTER" ]] || { echo "error: Android adapter diagnostics missing; see $LOG" >&2; exit 1; }
    printf '%s\n' "$ADAPTER" | node "$ANDROID_DIR/require-hardware-adapter.mjs"
    node -e '
      const report=JSON.parse(process.argv[1]);
      const [scenario, scenarioHash, physicsHash] = process.argv.slice(2);
      const contract=report.sceneContract;
      if (report.scenario?.name !== scenario || report.scenario?.complete !== true || report.scenario?.passed !== true
          || contract?.n !== scenario || contract?.h !== scenarioHash
          || contract?.k !== physicsHash || contract?.p !== "soft-coupled"
          || contract?.r !== "hz=30,damping=10,static-scale=2"
          || contract?.w !== false || contract?.y !== false || contract?.o !== false) {
        throw new Error(`Android contract mismatch: ${JSON.stringify(report)}`);
      }
    ' "$REPORT" "$SCENARIO" "$EXPECTED_SCENARIO_HASH" "$PHYSICS_HASH"
    "$ADB" -s "$SERIAL" exec-out screencap -p > "$SCREENSHOT"
    node "$ANDROID_DIR/require-complete-screenshot.mjs" "$SCREENSHOT"
    node -e '
      const fs=require("fs");
      const [path,count,scenario,screenshot,report,adapter] = process.argv.slice(1);
      fs.writeFileSync(path, JSON.stringify({
        role:`stack-${count}`, rectangleCount:Number(count), scenario, screenshot,
        report:JSON.parse(report), adapter:JSON.parse(adapter),
      }, null, 2)+"\n");
    ' "$ROW" "$COUNT" "$SCENARIO" "$SCREENSHOT" "$REPORT" "$ADAPTER"
    ROW_PATHS+=("$ROW")
    echo "ANDROID_FINALIST $SCENARIO PASS scenario-hash=$EXPECTED_SCENARIO_HASH"
done

OUTPUT="$OUTPUT_DIRECTORY/android-s25-awake-finalist.json"
rm -f "$OUTPUT"
node -e '
  const fs=require("fs");
  const [output,nativePath,revision,serial,model,product,...rowPaths]=process.argv.slice(1);
  const native=JSON.parse(fs.readFileSync(nativePath,"utf8"));
  const rows=rowPaths.map((path) => JSON.parse(fs.readFileSync(path,"utf8")));
  const temporary=`${output}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, JSON.stringify({
    version:1, runId:`s25-android-${Date.now()}-${process.pid}`,
    sourceRevision:revision, sourceDirty:false, host:"android",
    capturePhase:"s25-finalist-six-role-complete", nativeEvidence:nativePath,
    physicsConfigurationHash:native.physicsConfigurationHash,
    physicsConfiguration:native.physicsConfiguration,
    device:{serial,model,product}, rows, passed:true,
  }, null, 2)+"\n");
  fs.renameSync(temporary,output);
' "$OUTPUT" "$NATIVE_EVIDENCE" "$REVISION" "$SERIAL" "$MODEL" "$PRODUCT" "${ROW_PATHS[@]}"
echo "PHYSBOX5_VALIDATION_SUCCESS host=android device=$MODEL evidence=s25-awake-finalist output=$OUTPUT"
