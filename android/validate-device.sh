#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
ADB="$(physbox5_android_require_tool platform-tools/adb adb)"
SERIAL="${PHYSBOX5_ANDROID_SERIAL:-}"
SCENARIO="bootstrap"
SKIP_BUILD=0
SCREENSHOT="${PHYSBOX5_ANDROID_DEVICE_SCREENSHOT:-/tmp/physbox5-android-device.png}"
LOG="${PHYSBOX5_ANDROID_DEVICE_LOGCAT:-/tmp/physbox5-android-device-logcat.txt}"
PROVENANCE="$SCREENSHOT.json"
while (($#)); do
    case "$1" in
        --skip-build) SKIP_BUILD=1; shift ;;
        --scenario) SCENARIO="${2:?--scenario requires a value}"; shift 2 ;;
        --serial) SERIAL="${2:?--serial requires a value}"; shift 2 ;;
        *) echo "Usage: $0 [--skip-build] [--scenario NAME] [--serial SERIAL]" >&2; exit 2 ;;
    esac
done
case "$SCENARIO" in
    bootstrap|body-free-flight|rigid-floor|rigid-stack|reset-grab|solver-jacobi|solver-colored) ;;
    *) echo "error: unsupported shared scenario: $SCENARIO" >&2; exit 2 ;;
esac
if [[ -z "$SERIAL" ]]; then
    SERIAL="$($ADB devices | awk '$1 !~ /^emulator-/ && $2 == "device" {print $1; exit}')"
fi
[[ -n "$SERIAL" ]] || { echo "error: no connected physical Android device" >&2; exit 1; }
"$ADB" -s "$SERIAL" get-state 2>/dev/null | rg -qx device || {
    echo "error: Android device $SERIAL is unavailable or unauthorized" >&2; exit 1;
}
if "$ADB" -s "$SERIAL" shell dumpsys window 2>/dev/null | rg -q 'mDreamingLockscreen=true|mShowingLockscreen=true'; then
    echo "error: Android device $SERIAL is locked; unlock it before validation" >&2
    exit 1
fi

rm -f "$SCREENSHOT" "$PROVENANCE" "$LOG"
ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ "$SKIP_BUILD" -eq 0 ]]; then PHYSBOX5_ANDROID_ABI="$ABI" "$ANDROID_DIR/build-apk.sh"; fi
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[[ -f "$APK" ]] || { echo "error: APK missing: $APK" >&2; exit 1; }
"$ADB" -s "$SERIAL" install -r "$APK" >/dev/null
"$ADB" -s "$SERIAL" logcat -c
"$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5
STARTUP_ARGV="[\"--scenario\",\"$SCENARIO\"]"
START_COMMAND="am start -n com.kzahel.physbox5/android.app.NativeActivity --es physbox5.startup.argv '$STARTUP_ARGV'"
"$ADB" -s "$SERIAL" shell "$START_COMMAND" >/dev/null

PASSED=0
for _ in $(seq 1 90); do
    "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
    if rg -q 'PHYSBOX5_MARK_FRAME_RENDERED .*"complete":true,"passed":true' "$LOG"; then PASSED=1; break; fi
    if rg -q 'PHYSBOX5_ANDROID_(STARTUP|RENDER)_ERROR' "$LOG"; then break; fi
    sleep 1
done
[[ "$PASSED" -eq 1 ]] || { echo "error: Android device scenario did not pass; see $LOG" >&2; exit 1; }
"$ADB" -s "$SERIAL" exec-out screencap -p > "$SCREENSHOT"
REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
[[ "$(node -e 'console.log(JSON.parse(process.argv[1]).scenario.name)' "$REPORT")" = "$SCENARIO" ]] || {
    echo "error: Android device reported the wrong shared scenario; see $LOG" >&2; exit 1;
}
ADAPTER="$(rg 'PHYSBOX5_MARK_ADAPTER ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_ADAPTER //' || true)"
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
node -e '
  const fs = require("fs");
  const [path, runId, revision, dirty, report, adapter, scenario, serial, model, product] = process.argv.slice(1);
  fs.writeFileSync(path, JSON.stringify({
    runId, sourceRevision: revision, sourceDirty: dirty === "true", host: "android",
    capturePhase: `${scenario}-final`, device: { serial, model, product },
    report: JSON.parse(report), adapter: adapter ? JSON.parse(adapter) : null,
  }, null, 2) + "\n");
' "$PROVENANCE" "$(date +%s)-$$" "$REVISION" "$DIRTY" "$REPORT" "$ADAPTER" \
  "$SCENARIO" "$SERIAL" "$MODEL" "$PRODUCT"
echo "PHYSBOX5_VALIDATION_SUCCESS host=android device=$MODEL scenario=$SCENARIO screenshot=$SCREENSHOT provenance=$PROVENANCE"
