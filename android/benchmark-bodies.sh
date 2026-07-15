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
COUNTS="1000,10000,100000"
SEED=24301
WIDTH=960
HEIGHT=600
WARMUP=3
SAMPLES=20
SKIP_BUILD=0
KEEP_EMULATOR=0
while (($#)); do
    case "$1" in
        --avd|--device) MODE="${1#--}"; shift ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --counts) COUNTS="${2:?--counts requires a value}"; shift 2 ;;
        --seed) SEED="${2:?--seed requires a value}"; shift 2 ;;
        --width) WIDTH="${2:?--width requires a value}"; shift 2 ;;
        --height) HEIGHT="${2:?--height requires a value}"; shift 2 ;;
        --warmup) WARMUP="${2:?--warmup requires a value}"; shift 2 ;;
        --samples) SAMPLES="${2:?--samples requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        --keep-emulator) KEEP_EMULATOR=1; shift ;;
        *) echo "Usage: $0 (--avd|--device) [--serial SERIAL] [--output PATH] [--counts CSV] [--seed N] [--width PX] [--height PX] [--warmup N] [--samples N] [--skip-build] [--keep-emulator]" >&2; exit 2 ;;
    esac
done
[[ -n "$MODE" ]] || { echo "error: choose --avd or --device" >&2; exit 2; }
if [[ -z "$OUTPUT" ]]; then OUTPUT="/tmp/physbox5-android-bodies-$MODE.json"; fi
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
        nohup "$EMULATOR" -avd "$AVD_NAME" -no-snapshot-save -no-audio >/tmp/physbox5-android-body-benchmark-emulator.log 2>&1 </dev/null &
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
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 build --release -p physbox-android-gpu-benchmark --bin physbox-body-benchmark)
fi
BINARY="$REPO_ROOT/target/$TARGET/release/physbox-body-benchmark"
[[ -f "$BINARY" ]] || { echo "error: body benchmark binary missing: $BINARY" >&2; exit 1; }
REMOTE=/data/local/tmp/physbox5-body-benchmark
"$ADB" -s "$SERIAL" push "$BINARY" "$REMOTE" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$REMOTE"
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
RUN_ID="android-bodies-$(date +%s)-$$"
LOG="$(mktemp /tmp/physbox5-android-body-log.XXXXXX)"
"$ADB" -s "$SERIAL" shell "$REMOTE --run-id $RUN_ID --source-revision $REVISION --source-dirty $DIRTY --host android-native-vulkan --counts $COUNTS --seed $SEED --width $WIDTH --height $HEIGHT --warmup $WARMUP --samples $SAMPLES" | tr -d '\r' | tee "$LOG"
rg -qx 'PHYSBOX5_BODY_BENCHMARK_SUCCESS' "$LOG" || { echo "error: Android body benchmark sentinel missing" >&2; exit 1; }
REPORT="$(rg '^PHYSBOX5_BODY_BENCHMARK_RESULT ' "$LOG" | tail -1 | sed 's/^PHYSBOX5_BODY_BENCHMARK_RESULT //')"
[[ -n "$REPORT" ]] || { echo "error: Android body benchmark result missing" >&2; exit 1; }
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
MANUFACTURER="$($ADB -s "$SERIAL" shell getprop ro.product.manufacturer | tr -d '\r')"
ANDROID_RELEASE="$($ADB -s "$SERIAL" shell getprop ro.build.version.release | tr -d '\r')"
SDK_LEVEL="$($ADB -s "$SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')"
rm -f "$OUTPUT" "$OUTPUT.tmp"
node -e '
  const fs = require("fs");
  const [path, mode, avd, serial, model, product, manufacturer, androidRelease, sdkLevel, report] = process.argv.slice(1);
  const artifact = JSON.parse(report);
  artifact.target = { mode, avd: mode === "avd" ? avd : null, serial, model, product, manufacturer, androidRelease, sdkLevel: Number(sdkLevel) };
  fs.writeFileSync(path + ".tmp", JSON.stringify(artifact, null, 2) + "\n");
  fs.renameSync(path + ".tmp", path);
' "$OUTPUT" "$MODE" "$AVD_NAME" "$SERIAL" "$MODEL" "$PRODUCT" "$MANUFACTURER" "$ANDROID_RELEASE" "$SDK_LEVEL" "$REPORT"
rm -f "$LOG"
echo "PHYSBOX5_BODY_BENCHMARK_SUCCESS target=$MODE model=$MODEL output=$OUTPUT"
