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
SCENARIO="bootstrap"
while (($#)); do
    case "$1" in
        --skip-build) SKIP_BUILD=1; shift ;;
        --keep-emulator) KEEP_EMULATOR=1; shift ;;
        --scenario) SCENARIO="${2:?--scenario requires a value}"; shift 2 ;;
        *) echo "Usage: $0 [--skip-build] [--keep-emulator] [--scenario NAME]" >&2; exit 2 ;;
    esac
done
case "$SCENARIO" in
    bootstrap|body-free-flight|rigid-floor|rigid-stack|liquid-sandbox|rectangle-stack|circle-bucket-100|rectangle-bucket-100|reset-grab|solver-jacobi|solver-colored|solver-manifold-i2|solver-parallel-coloring|settling-hard-8|settling-candidate-8|settling-hard-16|settling-identity-16|settling-softness-16|settling-candidate-16) ;;
    *) echo "error: unsupported shared scenario: $SCENARIO" >&2; exit 2 ;;
esac
"$EMULATOR" -list-avds | grep -qx "$AVD_NAME" || { echo "error: AVD $AVD_NAME not found" >&2; exit 1; }
rm -f "$SCREENSHOT" "$PROVENANCE" "$LOG"

SERIAL="$("$ADB" devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
STARTED=0
EMULATOR_PID=""
if [[ -n "$SERIAL" ]]; then
    RUNNING_AVD="$("$ADB" -s "$SERIAL" emu avd name 2>/dev/null | head -1 | tr -d '\r')"
    [[ "$RUNNING_AVD" = "$AVD_NAME" ]] || { echo "error: running emulator is $RUNNING_AVD, expected $AVD_NAME" >&2; exit 1; }
fi
if [[ -z "$SERIAL" ]]; then
    STARTED=1
    EMULATOR_LOG="/tmp/physbox5-android-emulator.log"
    rm -f "$EMULATOR_LOG"
    physbox5_android_configure_emulator_display
    nohup "${PHYSBOX5_ANDROID_EMULATOR_PREFIX[@]}" "$EMULATOR" -avd "$AVD_NAME" -gpu host "${PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS[@]}" -no-snapshot-save -no-audio >"$EMULATOR_LOG" 2>&1 </dev/null &
    EMULATOR_PID=$!
    for _ in $(seq 1 120); do
        SERIAL="$("$ADB" devices | awk '$1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
        if [[ -n "$SERIAL" && "$("$ADB" -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]]; then break; fi
        sleep 2
    done
fi
[[ -n "$SERIAL" ]] || { echo "error: emulator did not become ready" >&2; exit 1; }
cleanup() {
    physbox5_android_stop_product_app "$ADB" "$SERIAL"
    if [[ "$STARTED" -eq 1 && "$KEEP_EMULATOR" -eq 0 ]]; then
        physbox5_android_stop_emulator "$ADB" "$SERIAL" "$EMULATOR_PID" "$AVD_NAME"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

ABI="$("$ADB" -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ "$SKIP_BUILD" -eq 0 ]]; then PHYSBOX5_ANDROID_ABI="$ABI" "$ANDROID_DIR/build-apk.sh"; fi
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[[ -f "$APK" ]] || { echo "error: APK missing: $APK" >&2; exit 1; }
"$ADB" -s "$SERIAL" install -r "$APK" >/dev/null
"$ADB" -s "$SERIAL" logcat -c
"$ADB" -s "$SERIAL" shell am force-stop com.kzahel.physbox5
STARTUP_ARGV="[\"--scenario\",\"$SCENARIO\",\"--frame-markers\",\"all\"]"
START_COMMAND="am start -n com.kzahel.physbox5/android.app.NativeActivity --es physbox5.startup.argv '$STARTUP_ARGV'"
"$ADB" -s "$SERIAL" shell "$START_COMMAND" >/dev/null

PASSED=0
for _ in $(seq 1 90); do
    "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
    if rg -q 'PHYSBOX5_MARK_FRAME_RENDERED .*"complete":true,"passed":true' "$LOG"; then PASSED=1; break; fi
    if rg -q 'PHYSBOX5_ANDROID_(STARTUP|RENDER)_ERROR' "$LOG"; then break; fi
    sleep 1
done
[[ "$PASSED" -eq 1 ]] || { echo "error: Android shared scenario did not pass; see $LOG" >&2; exit 1; }
REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
[[ "$(node -e 'console.log(JSON.parse(process.argv[1]).scenario.name)' "$REPORT")" = "$SCENARIO" ]] || {
    echo "error: Android reported the wrong shared scenario; see $LOG" >&2; exit 1;
}
ADAPTER="$(rg 'PHYSBOX5_MARK_ADAPTER ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_ADAPTER //' || true)"
[[ -n "$ADAPTER" ]] || { echo "error: Android adapter diagnostics missing; see $LOG" >&2; exit 1; }
printf '%s\n' "$ADAPTER" | node "$ANDROID_DIR/require-hardware-adapter.mjs"
CAPTURE_PHASE="${SCENARIO}-final"
if [[ "${PHYSBOX5_ANDROID_SLEEP_VISUALS:-0}" = "1" && "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" ]]; then
    echo "error: Android sleep visuals require PHYSBOX5_ANDROID_SLEEP_TOGGLE=1" >&2; exit 2
fi
if [[ "${PHYSBOX5_ANDROID_SLEEP_CONFIG:-0}" = "1" && "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" ]]; then
    echo "error: Android sleep configuration requires PHYSBOX5_ANDROID_SLEEP_TOGGLE=1" >&2; exit 2
fi
if [[ "${PHYSBOX5_ANDROID_LIQUID_CAPTURE:-0}" = "1" ]]; then
    [[ "$SCENARIO" = "liquid-sandbox" ]] || { echo "error: Android liquid capture requires liquid-sandbox" >&2; exit 2; }
    [[ "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_CONSTRUCTION_CAPTURE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || {
        echo "error: choose only one Android interaction capture" >&2; exit 2;
    }
fi
if [[ "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" = "1" ]]; then
    [[ "${PHYSBOX5_ANDROID_CONSTRUCTION_CAPTURE:-0}" != "1" && "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || {
        echo "error: choose only one Android interaction capture" >&2; exit 2;
    }
    node -e '
      const frame = JSON.parse(process.argv[1]);
      process.exit(frame.sceneContract.w === true && frame.sceneContract.y === true
        && frame.sceneContract.j === 0 ? 0 : 1);
    ' "$REPORT" || { echo "error: Android sleep toggle must start from the on/exact default; see $LOG" >&2; exit 1; }
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || { echo "error: could not determine Android display size" >&2; exit 1; }
    UI_POINTS="$(node -e '
      const w = +process.argv[1], h = +process.argv[2];
      const [version, pixelsPerPoint, toolbarScale, menuScale] =
        JSON.parse(process.argv[3]).v;
      if (version !== 12 || !(pixelsPerPoint > 0 && toolbarScale > 0 && menuScale > 0)) {
        throw new Error("invalid UI density marker");
      }
      const compact = h / pixelsPerPoint < 520 && w > h;
      const menuPoints = [
        300, 104, 192, 331,
        compact ? 496 : 192, compact ? 217 : 377,
        compact ? 496 : 192, compact ? 263 : 423,
        compact ? 496 : 192, compact ? 309 : 469,
      ];
      console.log([
        Math.round(38 * toolbarScale), Math.round(50 * toolbarScale),
        ...menuPoints.map((value) => Math.round(value * menuScale)),
      ].join(" "));
    ' "$WIDTH" "$HEIGHT" "$REPORT")"
    read -r MENU_X MENU_Y WORLD_X WORLD_Y SLEEP_X SLEEP_Y THRESHOLD_X THRESHOLD_Y \
        GROUPING_X GROUPING_Y VISUALS_X VISUALS_Y <<< "$UI_POINTS"
    "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$SLEEP_X" "$SLEEP_Y"
    SLEEPING_DISABLED=0
    for _ in $(seq 1 40); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]);
          process.exit(frame.sceneContract.w === false && frame.sceneContract.y === false
            && frame.scenario.complete === true && frame.scenario.passed === true ? 0 : 1);
        ' "$REPORT"; then SLEEPING_DISABLED=1; break; fi
        sleep 0.25
    done
    [[ "$SLEEPING_DISABLED" -eq 1 ]] || { echo "error: Android menu did not disable sleeping; see $LOG" >&2; exit 1; }
    "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$SLEEP_X" "$SLEEP_Y"
    SLEEPING_EFFECTIVE=0
    for _ in $(seq 1 40); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]);
          process.exit(frame.sceneContract.w === true && frame.sceneContract.y === true
            && frame.scenario.complete === true && frame.scenario.passed === true ? 0 : 1);
        ' "$REPORT"; then SLEEPING_EFFECTIVE=1; break; fi
        sleep 0.25
    done
    [[ "$SLEEPING_EFFECTIVE" -eq 1 ]] || { echo "error: Android menu did not re-enable sleeping; see $LOG" >&2; exit 1; }
    "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
    sleep 0.25
    if [[ "${PHYSBOX5_ANDROID_SLEEP_CONFIG:-0}" = "1" ]]; then
        "$ADB" -s "$SERIAL" shell input tap "$THRESHOLD_X" "$THRESHOLD_Y"
        for EXPECTED_GROUPING in 4 8 0; do
            "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
            sleep 0.25
            "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
            sleep 0.25
            "$ADB" -s "$SERIAL" shell input tap "$GROUPING_X" "$GROUPING_Y"
            CONFIG_APPLIED=0
            for _ in $(seq 1 40); do
                "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
                REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
                if node -e '
                  const frame = JSON.parse(process.argv[1]);
                  const expectedGrouping = Number(process.argv[2]);
                  process.exit(frame.sceneContract.t === 0.05
                    && frame.sceneContract.j === expectedGrouping
                    && frame.sceneContract.w === true && frame.sceneContract.y === true ? 0 : 1);
                ' "$REPORT" "$EXPECTED_GROUPING"; then CONFIG_APPLIED=1; break; fi
                sleep 0.25
            done
            [[ "$CONFIG_APPLIED" -eq 1 ]] || {
                echo "error: Android menu did not apply sleep threshold/grouping; see $LOG" >&2; exit 1;
            }
        done
        "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
        sleep 0.25
        "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
        sleep 0.25
    fi
    if [[ "${PHYSBOX5_ANDROID_SLEEP_VISUALS:-0}" = "1" ]]; then
        "$ADB" -s "$SERIAL" shell input tap "$VISUALS_X" "$VISUALS_Y"
        VISUALS_ENABLED=0
        for _ in $(seq 1 20); do
            "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
            REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
            if node -e '
              const frame = JSON.parse(process.argv[1]);
              process.exit((frame.u & (1 << 30)) !== 0 && frame.sceneContract.w === true
                && frame.sceneContract.y === true ? 0 : 1);
            ' "$REPORT"; then VISUALS_ENABLED=1; break; fi
            sleep 0.25
        done
        [[ "$VISUALS_ENABLED" -eq 1 ]] || { echo "error: Android menu did not enable sleep visuals; see $LOG" >&2; exit 1; }
        CAPTURE_PHASE="${SCENARIO}-sleep-visuals"
    elif [[ "${PHYSBOX5_ANDROID_SLEEP_CONFIG:-0}" = "1" ]]; then
        CAPTURE_PHASE="${SCENARIO}-sleep-threshold-0.05-exact-menu"
    else
        CAPTURE_PHASE="${SCENARIO}-sleep-request-on-menu"
    fi
    "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
    REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
fi
if [[ "${PHYSBOX5_ANDROID_CONSTRUCTION_CAPTURE:-0}" = "1" ]]; then
    [[ "$SCENARIO" = "bootstrap" ]] || { echo "error: Android construction capture requires bootstrap" >&2; exit 2; }
    [[ "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || { echo "error: choose either grab or construction capture" >&2; exit 2; }
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || { echo "error: could not determine Android display size" >&2; exit 1; }
    BEFORE_CONSTRUCTION="$(node -e 'console.log(JSON.parse(process.argv[1]).bodyCommands.c)' "$REPORT")"
    TOOL_POINT="$(node -e '
      const [version, , toolbarScale] = JSON.parse(process.argv[1]).v;
      if (version !== 12 || !(toolbarScale > 0)) throw new Error("invalid UI density marker");
      console.log(`${Math.round(229 * toolbarScale)} ${Math.round(50 * toolbarScale)}`);
    ' "$REPORT")"
    read -r TOOL_X TOOL_Y <<< "$TOOL_POINT"
    "$ADB" -s "$SERIAL" shell input tap "$TOOL_X" "$TOOL_Y"
    "$ADB" -s "$SERIAL" shell input tap $((WIDTH * 75 / 100)) $((HEIGHT * 30 / 100))
    CONSTRUCTION_VISIBLE=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]); const before = Number(process.argv[2]);
          process.exit(frame.bodyCommands.c > before && frame.d?.a >= 1 && frame.d?.r === "placed" && frame.d?.b === false ? 0 : 1);
        ' "$REPORT" "$BEFORE_CONSTRUCTION"; then CONSTRUCTION_VISIBLE=1; break; fi
        sleep 0.25
    done
    [[ "$CONSTRUCTION_VISIBLE" -eq 1 ]] || { echo "error: Android construction diagnostic did not become visible; see $LOG" >&2; exit 1; }
    CAPTURE_PHASE="${SCENARIO}-construction-placed"
fi
if [[ "${PHYSBOX5_ANDROID_LIQUID_CAPTURE:-0}" = "1" ]]; then
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || { echo "error: could not determine Android display size" >&2; exit 1; }
    TOOL_POINTS="$(node -e '
      const w = +process.argv[1], h = +process.argv[2];
      const [version, , toolbarScale] = JSON.parse(process.argv[3]).v;
      if (version !== 12 || !(toolbarScale > 0)) throw new Error("invalid UI density marker");
      const logicalWidth = w / toolbarScale;
      const stride = logicalWidth < 460 ? Math.max(1, (logicalWidth - 78) / 6) : 58;
      const width = logicalWidth < 460 ? Math.max(1, stride - 6) : 52;
      const center = (index) => Math.round((70 + index * stride + width * 0.5) * toolbarScale);
      console.log(`${center(4)} ${center(3)} ${center(5)} ${Math.round(50 * toolbarScale)}`);
    ' "$WIDTH" "$HEIGHT" "$REPORT")"
    read -r WATER_X ERASE_X SURFACE_X TOOL_Y <<< "$TOOL_POINTS"
    TARGET_X=$((WIDTH * 68 / 100)); TARGET_Y=$((HEIGHT * 34 / 100))
    BEFORE_PARTICLES="$(node -e 'console.log(JSON.parse(process.argv[1]).bodyCommands.e)' "$REPORT")"
    "$ADB" -s "$SERIAL" shell input tap "$WATER_X" "$TOOL_Y"
    "$ADB" -s "$SERIAL" shell input tap "$TARGET_X" "$TARGET_Y"
    WATER_EXECUTED=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]); const before = Number(process.argv[2]);
          process.exit(frame.bodyCommands.e > before ? 0 : 1);
        ' "$REPORT" "$BEFORE_PARTICLES"; then WATER_EXECUTED=1; break; fi
        sleep 0.25
    done
    [[ "$WATER_EXECUTED" -eq 1 ]] || { echo "error: Android Water tool did not execute; see $LOG" >&2; exit 1; }

    AFTER_WATER="$(node -e 'console.log(JSON.parse(process.argv[1]).bodyCommands.e)' "$REPORT")"
    "$ADB" -s "$SERIAL" shell input tap "$ERASE_X" "$TOOL_Y"
    "$ADB" -s "$SERIAL" shell input tap "$TARGET_X" "$TARGET_Y"
    ERASE_EXECUTED=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]); const before = Number(process.argv[2]);
          process.exit(frame.bodyCommands.e > before ? 0 : 1);
        ' "$REPORT" "$AFTER_WATER"; then ERASE_EXECUTED=1; break; fi
        sleep 0.25
    done
    [[ "$ERASE_EXECUTED" -eq 1 ]] || { echo "error: Android particle Erase did not execute; see $LOG" >&2; exit 1; }

    BEFORE_SURFACE="$(node -e 'console.log(JSON.parse(process.argv[1]).bodyCommands.c)' "$REPORT")"
    "$ADB" -s "$SERIAL" shell input tap "$SURFACE_X" "$TOOL_Y"
    X1=$((WIDTH * 58 / 100)); Y1=$((HEIGHT * 48 / 100))
    X2=$((WIDTH * 80 / 100)); Y2=$((HEIGHT * 40 / 100))
    "$ADB" -s "$SERIAL" shell input motionevent DOWN "$X1" "$Y1"
    "$ADB" -s "$SERIAL" shell input motionevent MOVE "$X2" "$Y2"
    sleep 0.2
    "$ADB" -s "$SERIAL" shell input motionevent UP "$X2" "$Y2"
    SURFACE_EXECUTED=0
    for _ in $(seq 1 40); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]); const before = Number(process.argv[2]);
          process.exit(frame.bodyCommands.c > before
            && frame.d?.r === "placed" && frame.d?.b === false ? 0 : 1);
        ' "$REPORT" "$BEFORE_SURFACE"; then SURFACE_EXECUTED=1; break; fi
        sleep 0.25
    done
    [[ "$SURFACE_EXECUTED" -eq 1 ]] || { echo "error: Android Surface tool did not commit; see $LOG" >&2; exit 1; }
    CAPTURE_PHASE="${SCENARIO}-water-erase-surface"
fi
if [[ "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" = "1" ]]; then
    [[ "$SCENARIO" = "rigid-floor" ]] || { echo "error: Android grab capture requires rigid-floor" >&2; exit 2; }
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || { echo "error: could not determine Android display size" >&2; exit 1; }
    X1=$((WIDTH * 50 / 100)); Y1=$((HEIGHT * 40 / 100))
    X2=$((WIDTH * 80 / 100)); Y2=$((HEIGHT * 70 / 100))
    "$ADB" -s "$SERIAL" shell input motionevent DOWN "$X1" "$Y1"
    "$ADB" -s "$SERIAL" shell input motionevent MOVE "$X2" "$Y2"
    GRAB_ACTIVE=0
    for _ in $(seq 1 20); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        if rg -q 'PHYSBOX5_MARK_FRAME_RENDERED .*"g":true' "$LOG"; then GRAB_ACTIVE=1; break; fi
        sleep 0.25
    done
    [[ "$GRAB_ACTIVE" -eq 1 ]] || { "$ADB" -s "$SERIAL" shell input motionevent UP "$X2" "$Y2" || true; echo "error: Android grab did not become active; see $LOG" >&2; exit 1; }
    REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
    CAPTURE_PHASE="${SCENARIO}-grab-active"
fi
FINAL_SCENARIO_PASSED=0
for _ in $(seq 1 90); do
    "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
    REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
    if node -e '
      const frame = JSON.parse(process.argv[1]);
      process.exit(frame.scenario.complete === true && frame.scenario.passed === true ? 0 : 1);
    ' "$REPORT"; then FINAL_SCENARIO_PASSED=1; break; fi
    sleep 1
done
[[ "$FINAL_SCENARIO_PASSED" -eq 1 ]] || { echo "error: Android scenario did not repass after interaction; see $LOG" >&2; exit 1; }
CAPTURED=0
for _ in $(seq 1 5); do
    "$ADB" -s "$SERIAL" exec-out screencap -p > "$SCREENSHOT"
    if node "$ANDROID_DIR/require-complete-screenshot.mjs" "$SCREENSHOT"; then CAPTURED=1; break; fi
    sleep 0.2
done
[[ "$CAPTURED" -eq 1 ]] || { echo "error: Android screenshot remained incomplete" >&2; exit 1; }
if [[ "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" = "1" ]]; then
    node "$ANDROID_DIR/require-grab-tether.mjs" "$SCREENSHOT" "$X2" "$Y2"
    "$ADB" -s "$SERIAL" shell input motionevent UP "$X2" "$Y2"
fi
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
MODEL="$("$ADB" -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$("$ADB" -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
node -e '
  const fs = require("fs");
  const [path, runId, revision, dirty, report, adapter, scenario, capturePhase, avd, serial, model, product] = process.argv.slice(1);
  fs.writeFileSync(path, JSON.stringify({
    runId, sourceRevision: revision, sourceDirty: dirty === "true", host: "android",
    capturePhase, report: JSON.parse(report),
    device: { avd, serial, model, product },
    adapter: adapter ? JSON.parse(adapter) : null,
  }, null, 2) + "\n");
' "$PROVENANCE" "$(date +%s)-$$" "$REVISION" "$DIRTY" "$REPORT" "$ADAPTER" "$SCENARIO" "$CAPTURE_PHASE" \
  "$AVD_NAME" "$SERIAL" "$MODEL" "$PRODUCT"
echo "PHYSBOX5_VALIDATION_SUCCESS host=android avd=$AVD_NAME scenario=$SCENARIO screenshot=$SCREENSHOT provenance=$PROVENANCE"
