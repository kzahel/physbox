#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
physbox5_android_detect_ndk "27.0.12077973"
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"

TARGET_MODE=""
EVIDENCE_MODE=""
SERIAL="${PHYSBOX5_ANDROID_SERIAL:-}"
AVD_NAME="${PHYSBOX5_ANDROID_AVD:-jstorrent-tablet}"
OUTPUT=""
BASELINE=""
SKIP_BUILD=0
while (($#)); do
    case "$1" in
        --avd|--device) TARGET_MODE="${1#--}"; shift ;;
        --mode) EVIDENCE_MODE="${2:?--mode requires a value}"; shift 2 ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --baseline) BASELINE="${2:?--baseline requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        *)
            echo "Usage: $0 (--device|--avd) --mode (baseline|candidate|implementation|benchmark) --output PATH [--baseline BASELINE_MANIFEST] [--serial SERIAL] [--skip-build]" >&2
            exit 2
            ;;
    esac
done
[[ -n "$TARGET_MODE" ]] || { echo "error: choose --device or --avd" >&2; exit 2; }
case "$EVIDENCE_MODE" in
    baseline|implementation|benchmark) [[ -z "$BASELINE" ]] || { echo "error: --baseline is candidate-only" >&2; exit 2; } ;;
    candidate) [[ -f "$BASELINE" ]] || { echo "error: candidate mode requires a baseline manifest" >&2; exit 2; } ;;
    *) echo "error: choose a valid --mode" >&2; exit 2 ;;
esac
[[ -n "$OUTPUT" ]] || { echo "error: --output is required" >&2; exit 2; }
[[ ! -e "$OUTPUT" && ! -e "$OUTPUT.partial" ]] || { echo "error: output already exists: $OUTPUT" >&2; exit 1; }

if [[ "$TARGET_MODE" = "avd" ]]; then
    EMULATOR="$(physbox5_android_require_tool emulator/emulator emulator)"
    "$EMULATOR" -list-avds | rg -qx "$AVD_NAME" || { echo "error: AVD $AVD_NAME not found" >&2; exit 1; }
    SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
    if [[ -n "$SERIAL" ]]; then
        RUNNING_AVD="$($ADB -s "$SERIAL" emu avd name 2>/dev/null | head -1 | tr -d '\r')"
        [[ "$RUNNING_AVD" = "$AVD_NAME" ]] || { echo "error: running emulator is $RUNNING_AVD, expected $AVD_NAME" >&2; exit 1; }
    else
        nohup "$EMULATOR" -avd "$AVD_NAME" -gpu host -no-snapshot-save -no-audio >/tmp/physbox5-s17-emulator.log 2>&1 </dev/null &
        for _ in $(seq 1 120); do
            SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
            if [[ -n "$SERIAL" && "$($ADB -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]]; then
                break
            fi
            sleep 2
        done
    fi
else
    if [[ -z "$SERIAL" ]]; then
        SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"
    fi
    [[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
    if "$ADB" -s "$SERIAL" shell dumpsys window 2>/dev/null | rg -q 'mDreamingLockscreen=true|mShowingLockscreen=true'; then
        echo "error: Android device $SERIAL is locked; unlock it before S17 qualification" >&2
        exit 1
    fi
fi
[[ -n "$SERIAL" ]] || { echo "error: Android target did not become ready" >&2; exit 1; }

cleanup() {
    if [[ "$TARGET_MODE" = "avd" ]]; then
        "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1 || true
        for _ in $(seq 1 30); do
            if ! "$ADB" devices | awk '$1 ~ /^emulator-/ && $2 == "device" {found=1} END {exit found ? 0 : 1}'; then
                break
            fi
            sleep 1
        done
    fi
}
trap cleanup EXIT INT TERM

ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
case "$ABI" in
    x86_64) TARGET=x86_64-linux-android ;;
    arm64-v8a) TARGET=aarch64-linux-android ;;
    *) echo "error: unsupported Android S17 ABI: $ABI" >&2; exit 1 ;;
esac
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 build --release -p physbox-lab --bin physbox-s17-product-relative)
fi
BINARY="$REPO_ROOT/target/$TARGET/release/physbox-s17-product-relative"
[[ -f "$BINARY" ]] || { echo "error: Android S17 binary missing: $BINARY" >&2; exit 1; }

REMOTE_BINARY=/data/local/tmp/physbox5-s17-product-relative
REMOTE_OUTPUT="/data/local/tmp/physbox5-s17-$EVIDENCE_MODE"
REMOTE_BASELINE=/data/local/tmp/physbox5-s17-baseline-manifest.json
"$ADB" -s "$SERIAL" push "$BINARY" "$REMOTE_BINARY" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$REMOTE_BINARY"
"$ADB" -s "$SERIAL" shell rm -rf "$REMOTE_OUTPUT" "$REMOTE_OUTPUT.partial"
BASELINE_ARGUMENT=""
if [[ "$EVIDENCE_MODE" = "candidate" ]]; then
    "$ADB" -s "$SERIAL" push "$BASELINE" "$REMOTE_BASELINE" >/dev/null
    BASELINE_ARGUMENT="--baseline $REMOTE_BASELINE"
fi

REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD)"
DIRTY=false
[[ -z "$(git -C "$REPO_ROOT" status --porcelain=v1 --untracked-files=all)" ]] || DIRTY=true
[[ "$DIRTY" = false ]] || { echo "error: S17 Android retained evidence requires a clean tree" >&2; exit 1; }
HOST=physical-android
[[ "$TARGET_MODE" = "device" ]] || HOST=android-avd
RUN_ID="s17-$HOST-$EVIDENCE_MODE-$(date +%s)-$$"
LOG="$(mktemp /tmp/physbox5-s17-android-log.XXXXXX)"
if ! "$ADB" -s "$SERIAL" shell "$REMOTE_BINARY --mode $EVIDENCE_MODE --run-id $RUN_ID --output $REMOTE_OUTPUT --host $HOST --source-revision $REVISION --source-dirty false $BASELINE_ARGUMENT" | tr -d '\r' >"$LOG"; then
    cat "$LOG" >&2
    echo "error: Android S17 $EVIDENCE_MODE process failed; see $LOG" >&2
    exit 1
fi
rg -q "^PHYSBOX5_VALIDATION_SUCCESS host=$HOST scenario=s17-$EVIDENCE_MODE " "$LOG" || {
    cat "$LOG" >&2
    echo "error: Android S17 success sentinel missing; see $LOG" >&2
    exit 1
}
"$ADB" -s "$SERIAL" pull "$REMOTE_OUTPUT" "$OUTPUT.partial" >/dev/null
[[ -f "$OUTPUT.partial/manifest.json" ]] || { echo "error: pulled S17 manifest is missing" >&2; exit 1; }
if [[ "$TARGET_MODE" = "avd" ]]; then
    node "$ANDROID_DIR/require-hardware-adapter.mjs" <"$OUTPUT.partial/manifest.json"
fi
mv "$OUTPUT.partial" "$OUTPUT"
cat "$LOG"
rm -f "$LOG"
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
echo "PHYSBOX5_ANDROID_S17_SUCCESS target=$TARGET_MODE model=$MODEL mode=$EVIDENCE_MODE output=$OUTPUT"
