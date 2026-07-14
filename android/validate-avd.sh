#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"
EMULATOR="$(physbox5_android_require_tool emulator/emulator emulator)"
AVD_NAME="${PHYSBOX5_ANDROID_AVD:-jstorrent-tablet}"
SCREENSHOT="${PHYSBOX5_ANDROID_SCREENSHOT:-/tmp/physbox5-android-avd.png}"
LOG="${PHYSBOX5_ANDROID_LOGCAT:-/tmp/physbox5-android-avd-logcat.txt}"
PROVENANCE="$SCREENSHOT.json"
SKIP_BUILD=0
KEEP_EMULATOR=0
while (($#)); do
    case "$1" in
        --skip-build) SKIP_BUILD=1; shift ;;
        --keep-emulator) KEEP_EMULATOR=1; shift ;;
        *) echo "Usage: $0 [--skip-build] [--keep-emulator]" >&2; exit 2 ;;
    esac
done
"$EMULATOR" -list-avds | grep -qx "$AVD_NAME" || { echo "error: AVD $AVD_NAME not found" >&2; exit 1; }
rm -f "$SCREENSHOT" "$PROVENANCE" "$LOG"

SERIAL="$("$ADB" devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
STARTED=0
if [[ -z "$SERIAL" ]]; then
    STARTED=1
    EMULATOR_LOG="/tmp/physbox5-android-emulator.log"
    rm -f "$EMULATOR_LOG"
    nohup "$EMULATOR" -avd "$AVD_NAME" -no-snapshot-save -no-audio >"$EMULATOR_LOG" 2>&1 </dev/null &
    for _ in $(seq 1 120); do
        SERIAL="$("$ADB" devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
        if [[ -n "$SERIAL" && "$("$ADB" -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]]; then break; fi
        sleep 2
    done
fi
[[ -n "$SERIAL" ]] || { echo "error: emulator did not become ready" >&2; exit 1; }
cleanup() {
    if [[ "$STARTED" -eq 1 && "$KEEP_EMULATOR" -eq 0 ]]; then "$ADB" -s "$SERIAL" emu kill >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

ABI="$("$ADB" -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ "$SKIP_BUILD" -eq 0 ]]; then PHYSBOX5_ANDROID_ABI="$ABI" "$ANDROID_DIR/build-apk.sh"; fi
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[[ -f "$APK" ]] || { echo "error: APK missing: $APK" >&2; exit 1; }
"$ADB" -s "$SERIAL" install -r "$APK" >/dev/null
"$ADB" -s "$SERIAL" logcat -c
"$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5
"$ADB" -s "$SERIAL" shell am start -n com.kzahel.physbox5/android.app.NativeActivity >/dev/null

PASSED=0
for _ in $(seq 1 90); do
    "$ADB" -s "$SERIAL" logcat -d -t 1000 > "$LOG"
    if rg -q 'PHYSBOX5_MARK_FRAME_RENDERED .*"complete":true,"passed":true' "$LOG"; then PASSED=1; break; fi
    if rg -q 'PHYSBOX5_ANDROID_(STARTUP|RENDER)_ERROR' "$LOG"; then break; fi
    sleep 1
done
[[ "$PASSED" -eq 1 ]] || { echo "error: Android shared scenario did not pass; see $LOG" >&2; exit 1; }
"$ADB" -s "$SERIAL" exec-out screencap -p > "$SCREENSHOT"
REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
ADAPTER="$(rg 'PHYSBOX5_MARK_ADAPTER ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_ADAPTER //' || true)"
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
node -e '
  const fs = require("fs");
  const [path, runId, revision, dirty, report, adapter] = process.argv.slice(1);
  fs.writeFileSync(path, JSON.stringify({
    runId, sourceRevision: revision, sourceDirty: dirty === "true", host: "android",
    capturePhase: "bootstrap-final", report: JSON.parse(report),
    adapter: adapter ? JSON.parse(adapter) : null,
  }, null, 2) + "\n");
' "$PROVENANCE" "$(date +%s)-$$" "$REVISION" "$DIRTY" "$REPORT" "$ADAPTER"
echo "PHYSBOX5_VALIDATION_SUCCESS host=android avd=$AVD_NAME screenshot=$SCREENSHOT provenance=$PROVENANCE"
