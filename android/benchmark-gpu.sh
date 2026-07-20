#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
physbox5_android_detect_ndk "27.0.12077973"
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"
MODE=""
SERIAL="${PHYSBOX5_ANDROID_SERIAL:-}"
AVD_NAME="${PHYSBOX5_ANDROID_AVD:-jstorrent-tablet}"
OUTPUT=""
SOLVER_CONTACTS=1000
PRIMITIVE_ELEMENTS=10000
WARMUP=3
SAMPLES=20
SKIP_BUILD=0
KEEP_EMULATOR=0
while (($#)); do
    case "$1" in
        --avd|--device) MODE="${1#--}"; shift ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --solver-contacts) SOLVER_CONTACTS="${2:?--solver-contacts requires a value}"; shift 2 ;;
        --primitive-elements) PRIMITIVE_ELEMENTS="${2:?--primitive-elements requires a value}"; shift 2 ;;
        --warmup) WARMUP="${2:?--warmup requires a value}"; shift 2 ;;
        --samples) SAMPLES="${2:?--samples requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        --keep-emulator) KEEP_EMULATOR=1; shift ;;
        *) echo "Usage: $0 (--avd|--device) [--serial SERIAL] [--output PATH] [--skip-build] [--keep-emulator] [--solver-contacts N] [--primitive-elements N] [--warmup N] [--samples N]" >&2; exit 2 ;;
    esac
done
[[ -n "$MODE" ]] || { echo "error: choose --avd or --device" >&2; exit 2; }
if [[ -z "$OUTPUT" ]]; then OUTPUT="/tmp/physbox5-android-gpu-$MODE.json"; fi
STARTED=0
if [[ "$MODE" = "avd" ]]; then
    EMULATOR="$(physbox5_android_require_tool emulator/emulator emulator)"
    "$EMULATOR" -list-avds | rg -qx "$AVD_NAME" || { echo "error: AVD $AVD_NAME not found" >&2; exit 1; }
    SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
    if [[ -n "$SERIAL" ]]; then
        RUNNING_AVD="$($ADB -s "$SERIAL" emu avd name 2>/dev/null | head -1 | tr -d '\r')"
        [[ "$RUNNING_AVD" = "$AVD_NAME" ]] || { echo "error: running emulator is $RUNNING_AVD, expected $AVD_NAME" >&2; exit 1; }
    fi
    if [[ -z "$SERIAL" ]]; then
        STARTED=1
        physbox5_android_configure_emulator_display
        nohup "${PHYSBOX5_ANDROID_EMULATOR_PREFIX[@]}" "$EMULATOR" -avd "$AVD_NAME" -gpu host "${PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS[@]}" -no-snapshot-save -no-audio >/tmp/physbox5-android-benchmark-emulator.log 2>&1 </dev/null &
        for _ in $(seq 1 120); do
            SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
            if [[ -n "$SERIAL" && "$($ADB -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]]; then break; fi
            sleep 2
        done
    fi
else
    if [[ -z "$SERIAL" ]]; then SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"; fi
    [[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
    if "$ADB" -s "$SERIAL" shell dumpsys window 2>/dev/null | rg -q 'mDreamingLockscreen=true|mShowingLockscreen=true'; then
        echo "error: Android device $SERIAL is locked; unlock it before benchmark" >&2
        exit 1
    fi
fi
[[ -n "$SERIAL" ]] || { echo "error: Android target did not become ready" >&2; exit 1; }
cleanup() {
    if [[ "$STARTED" -eq 1 && "$KEEP_EMULATOR" -eq 0 ]]; then "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT
ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
case "$ABI" in
    x86_64) TARGET=x86_64-linux-android ;;
    arm64-v8a) TARGET=aarch64-linux-android ;;
    *) echo "error: unsupported Android benchmark ABI: $ABI" >&2; exit 1 ;;
esac
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 build --release -p physbox-android-gpu-benchmark)
fi
BINARY="$REPO_ROOT/target/$TARGET/release/physbox-android-gpu-benchmark"
[[ -f "$BINARY" ]] || { echo "error: Android benchmark binary missing: $BINARY" >&2; exit 1; }
REMOTE=/data/local/tmp/physbox5-android-gpu-benchmark
"$ADB" -s "$SERIAL" push "$BINARY" "$REMOTE" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$REMOTE"
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
RUN_ID="android-gpu-$(date +%s)-$$"
LOG="$(mktemp /tmp/physbox5-android-gpu-log.XXXXXX)"
rm -f "$OUTPUT" "$OUTPUT.tmp"
if ! "$ADB" -s "$SERIAL" shell "$REMOTE --run-id $RUN_ID --source-revision $REVISION --source-dirty $DIRTY --solver-contacts $SOLVER_CONTACTS --primitive-elements $PRIMITIVE_ELEMENTS --warmup $WARMUP --samples $SAMPLES" | tr -d '\r' >"$LOG"; then
    cat "$LOG" >&2
    echo "error: Android GPU benchmark process failed; see $LOG" >&2
    exit 1
fi
rg -qx 'PHYSBOX5_ANDROID_GPU_BENCHMARK_SUCCESS' "$LOG" || { cat "$LOG" >&2; echo "error: Android GPU benchmark sentinel missing; see $LOG" >&2; exit 1; }
REPORT="$(rg '^PHYSBOX5_ANDROID_BENCHMARK_RESULT ' "$LOG" | tail -1 | sed 's/^PHYSBOX5_ANDROID_BENCHMARK_RESULT //')"
[[ -n "$REPORT" ]] || { cat "$LOG" >&2; echo "error: Android GPU benchmark result missing; see $LOG" >&2; exit 1; }
if [[ "$MODE" = "avd" ]]; then
    printf '%s\n' "$REPORT" | node "$ANDROID_DIR/require-hardware-adapter.mjs"
fi
cat "$LOG"
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
node -e '
  const fs = require("fs");
  const [path, mode, avd, serial, model, product, report] = process.argv.slice(1);
  fs.writeFileSync(path + ".tmp", JSON.stringify({
    target: { mode, avd: mode === "avd" ? avd : null, serial, model, product },
    report: JSON.parse(report),
  }, null, 2) + "\n");
  fs.renameSync(path + ".tmp", path);
' "$OUTPUT" "$MODE" "$AVD_NAME" "$SERIAL" "$MODEL" "$PRODUCT" "$REPORT"
rm -f "$LOG"
echo "PHYSBOX5_ANDROID_GPU_BENCHMARK_SUCCESS target=$MODE model=$MODEL output=$OUTPUT"
