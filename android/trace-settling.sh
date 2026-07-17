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
SCENARIO="settling-candidate-16"
SKIP_BUILD=0
KEEP_EMULATOR=0
while (($#)); do
    case "$1" in
        --avd|--device) MODE="${1#--}"; shift ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --scenario) SCENARIO="${2:?--scenario requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        --keep-emulator) KEEP_EMULATOR=1; shift ;;
        *) echo "Usage: $0 (--avd|--device) [--serial SERIAL] [--output PATH] [--scenario NAME] [--skip-build] [--keep-emulator]" >&2; exit 2 ;;
    esac
done
[[ -n "$MODE" ]] || { echo "error: choose --avd or --device" >&2; exit 2; }
case "$SCENARIO" in
    settling-hard-16|settling-identity-16|settling-softness-16|settling-candidate-16|minimized-mixed-candidate) ;;
    *) echo "error: unsupported settling trace scenario: $SCENARIO" >&2; exit 2 ;;
esac
if [[ -z "$OUTPUT" ]]; then OUTPUT="/tmp/physbox5-settling-$MODE-$SCENARIO.json.gz"; fi
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
        nohup "$EMULATOR" -avd "$AVD_NAME" -gpu host -no-snapshot-save -no-audio >/tmp/physbox5-android-trace-emulator.log 2>&1 </dev/null &
        for _ in $(seq 1 120); do
            SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
            if [[ -n "$SERIAL" && "$($ADB -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]]; then break; fi
            sleep 2
        done
    fi
else
    if [[ -z "$SERIAL" ]]; then SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"; fi
    [[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
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
    *) echo "error: unsupported Android trace ABI: $ABI" >&2; exit 1 ;;
esac
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 build --release -p physbox-lab --bin physbox-settling-backend-trace)
fi
BINARY="$REPO_ROOT/target/$TARGET/release/physbox-settling-backend-trace"
[[ -f "$BINARY" ]] || { echo "error: Android trace binary missing: $BINARY" >&2; exit 1; }
REMOTE_BINARY=/data/local/tmp/physbox5-settling-backend-trace
REMOTE_OUTPUT=/data/local/tmp/physbox5-settling-backend-trace.json.gz
"$ADB" -s "$SERIAL" push "$BINARY" "$REMOTE_BINARY" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$REMOTE_BINARY"
"$ADB" -s "$SERIAL" shell rm -f "$REMOTE_OUTPUT" "$REMOTE_OUTPUT.tmp"
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
RUN_ID="android-settling-trace-$(date +%s)-$$"
LOG="$(mktemp /tmp/physbox5-android-settling-trace-log.XXXXXX)"
rm -f "$OUTPUT.tmp"
if ! "$ADB" -s "$SERIAL" shell "$REMOTE_BINARY --run-id $RUN_ID --source-revision $REVISION --source-dirty $DIRTY --scenario $SCENARIO --output $REMOTE_OUTPUT" | tr -d '\r' >"$LOG"; then
    cat "$LOG" >&2
    echo "error: Android settling trace process failed; see $LOG" >&2
    exit 1
fi
rg -q "^PHYSBOX5_SETTLING_BACKEND_TRACE_SUCCESS scenario=$SCENARIO " "$LOG" || { cat "$LOG" >&2; echo "error: Android settling trace sentinel missing; see $LOG" >&2; exit 1; }
ADAPTER="$(rg '^PHYSBOX5_SETTLING_TRACE_ADAPTER ' "$LOG" | tail -1 | sed 's/^PHYSBOX5_SETTLING_TRACE_ADAPTER //')"
[[ -n "$ADAPTER" ]] || { cat "$LOG" >&2; echo "error: Android settling trace adapter missing; see $LOG" >&2; exit 1; }
if [[ "$MODE" = "avd" ]]; then printf '%s\n' "$ADAPTER" | node "$ANDROID_DIR/require-hardware-adapter.mjs"; fi
"$ADB" -s "$SERIAL" pull "$REMOTE_OUTPUT" "$OUTPUT.tmp" >/dev/null
mv "$OUTPUT.tmp" "$OUTPUT"
cat "$LOG"
rm -f "$LOG"
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
echo "PHYSBOX5_ANDROID_SETTLING_TRACE_SUCCESS target=$MODE model=$MODEL scenario=$SCENARIO output=$OUTPUT"
