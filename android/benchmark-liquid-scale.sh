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
SOLID_TIER="20"
PARTICLES=4096
WARMUP=16
SAMPLES=30
SKIP_BUILD=0
while (($#)); do
    case "$1" in
        --avd|--device) MODE="${1#--}"; shift ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        --output) OUTPUT="${2:?--output requires a value}"; shift 2 ;;
        --solid-tier) SOLID_TIER="${2:?--solid-tier requires a value}"; shift 2 ;;
        --particles) PARTICLES="${2:?--particles requires a value}"; shift 2 ;;
        --warmup) WARMUP="${2:?--warmup requires a value}"; shift 2 ;;
        --samples) SAMPLES="${2:?--samples requires a value}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        *) echo "Usage: $0 (--avd|--device) [--serial SERIAL] [--output PATH] [--solid-tier water-only|20|100] [--particles N] [--warmup N] [--samples N] [--skip-build]" >&2; exit 2 ;;
    esac
done
[[ -n "$MODE" ]] || { echo "error: choose --avd or --device" >&2; exit 2; }
case "$SOLID_TIER" in water-only|20|100) ;; *) echo "error: unsupported solid tier: $SOLID_TIER" >&2; exit 2 ;; esac
if [[ -z "$OUTPUT" ]]; then OUTPUT="/tmp/physbox5-android-liquid-scale-$MODE.json"; fi
[[ ! -e "$OUTPUT" ]] || { echo "error: output must be fresh: $OUTPUT" >&2; exit 1; }

EMULATOR_PID=""
if [[ "$MODE" = "avd" ]]; then
    EMULATOR="$(physbox5_android_require_tool emulator/emulator emulator)"
    "$EMULATOR" -list-avds | rg -qx "$AVD_NAME" || { echo "error: AVD $AVD_NAME not found" >&2; exit 1; }
    SERIAL="$($ADB devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
    if [[ -n "$SERIAL" ]]; then
        RUNNING_AVD="$($ADB -s "$SERIAL" emu avd name 2>/dev/null | head -1 | tr -d '\r')"
        [[ "$RUNNING_AVD" = "$AVD_NAME" ]] || { echo "error: running emulator is $RUNNING_AVD, expected $AVD_NAME" >&2; exit 1; }
    else
        physbox5_android_configure_emulator_display
        nohup "${PHYSBOX5_ANDROID_EMULATOR_PREFIX[@]}" "$EMULATOR" -avd "$AVD_NAME" -gpu host "${PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS[@]}" -no-snapshot-save -no-audio >/tmp/physbox5-android-liquid-scale-emulator.log 2>&1 </dev/null &
        EMULATOR_PID=$!
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
    if [[ "$MODE" = "avd" ]]; then
        physbox5_android_stop_emulator "$ADB" "$SERIAL" "$EMULATOR_PID" "$AVD_NAME"
    fi
}
trap cleanup EXIT

ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
case "$ABI" in
    arm64-v8a) TARGET=aarch64-linux-android ;;
    x86_64) TARGET=x86_64-linux-android ;;
    *) echo "error: unsupported Android benchmark ABI: $ABI" >&2; exit 1 ;;
esac
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 build --release -p physbox-android-gpu-benchmark --bin physbox-liquid-scale)
fi
BINARY="$REPO_ROOT/target/$TARGET/release/physbox-liquid-scale"
[[ -f "$BINARY" ]] || { echo "error: liquid benchmark binary missing: $BINARY" >&2; exit 1; }

RUN_ID="android-liquid-$(date +%s)-$$"
REMOTE_BINARY=/data/local/tmp/physbox5-liquid-scale
REMOTE_OUTPUT="/data/local/tmp/physbox5-liquid-scale-$RUN_ID.json"
"$ADB" -s "$SERIAL" push "$BINARY" "$REMOTE_BINARY" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$REMOTE_BINARY"
"$ADB" -s "$SERIAL" shell rm -f "$REMOTE_OUTPUT"

SOURCE_STATE="$(node "$REPO_ROOT/scripts/source-state.mjs" "$REPO_ROOT")"
REVISION="$(node -e 'console.log(JSON.parse(process.argv[1]).sourceRevision)' "$SOURCE_STATE")"
DIRTY="$(node -e 'console.log(JSON.parse(process.argv[1]).sourceDirty)' "$SOURCE_STATE")"
PATCH_DIGEST="$(node -e 'console.log(JSON.parse(process.argv[1]).sourcePatchDigest ?? "none")' "$SOURCE_STATE")"
TOOLCHAIN="rustc-$(rustc --version | awk '{print $2}')-android-$TARGET"
HOST="android-$SERIAL-$ABI"
LOG="$(mktemp /tmp/physbox5-android-liquid-log.XXXXXX)"
STAGING="$OUTPUT.tmp-$RUN_ID"
mkdir -p "$(dirname "$OUTPUT")"
rm -f "$STAGING"
if ! "$ADB" -s "$SERIAL" shell "$REMOTE_BINARY --run-id $RUN_ID --source-revision $REVISION --source-dirty $DIRTY --source-patch-digest $PATCH_DIGEST --host $HOST --toolchain $TOOLCHAIN --solid-tier $SOLID_TIER --particles $PARTICLES --warmup $WARMUP --samples $SAMPLES --output $REMOTE_OUTPUT" | tr -d '\r' >"$LOG"; then
    cat "$LOG" >&2
    echo "error: Android liquid benchmark failed; see $LOG" >&2
    exit 1
fi
rg -qx 'PHYSBOX5_LIQUID_SCALE_SUCCESS' "$LOG" || { cat "$LOG" >&2; echo "error: Android liquid benchmark sentinel missing; see $LOG" >&2; exit 1; }
ADAPTER="$(rg '^PHYSBOX5_LIQUID_SCALE_ADAPTER ' "$LOG" | tail -1 | sed 's/^PHYSBOX5_LIQUID_SCALE_ADAPTER //')"
[[ -n "$ADAPTER" ]] || { cat "$LOG" >&2; echo "error: Android liquid adapter diagnostics missing; see $LOG" >&2; exit 1; }
printf '%s\n' "$ADAPTER" | node "$ANDROID_DIR/require-hardware-adapter.mjs"
printf '%s\n' "$ADAPTER" | node -e '
  let input = ""; process.stdin.setEncoding("utf8");
  process.stdin.on("data", chunk => input += chunk);
  process.stdin.on("end", () => {
    const adapter = JSON.parse(input);
    if (adapter.backend !== "Vulkan") throw new Error(`expected Vulkan, found ${adapter.backend}`);
  });
'
"$ADB" -s "$SERIAL" pull "$REMOTE_OUTPUT" "$STAGING" >/dev/null
node -e '
  const fs = require("fs");
  const [path, tier, particles, revision] = process.argv.slice(1);
  const artifact = JSON.parse(fs.readFileSync(path, "utf8"));
  if (artifact.hostKind !== "android-native" || artifact.comparatorEnabled) throw new Error("invalid Android liquid host/build identity");
  if (artifact.config.particleCount !== Number(particles)) throw new Error("Android liquid particle count drifted");
  const expectedTier = tier === "water-only" ? "water-only" : `solids${tier}`;
  if (artifact.config.solidTier !== expectedTier) throw new Error("Android liquid solid tier drifted");
  if (artifact.provenance.sourceRevision !== revision) throw new Error("Android liquid revision drifted");
' "$STAGING" "$SOLID_TIER" "$PARTICLES" "$REVISION"
mv "$STAGING" "$OUTPUT"
"$ADB" -s "$SERIAL" shell rm -f "$REMOTE_OUTPUT" "$REMOTE_BINARY"
cat "$LOG"
rm -f "$LOG"
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
echo "PHYSBOX5_LIQUID_SCALE_SUCCESS host=android target=$MODE model=$MODEL output=$OUTPUT"
