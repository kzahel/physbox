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
    bootstrap|body-free-flight|rigid-floor|rigid-stack|rectangle-stack|circle-bucket-100|rectangle-bucket-100|reset-grab|solver-jacobi|solver-colored|solver-manifold-i2|solver-parallel-coloring|settling-hard-8|settling-candidate-8|settling-hard-16|settling-identity-16|settling-softness-16|settling-candidate-16) ;;
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
cleanup() {
    physbox5_android_stop_product_app "$ADB" "$SERIAL"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

rm -f "$SCREENSHOT" "$PROVENANCE" "$LOG"
ABI="$($ADB -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
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
[[ "$PASSED" -eq 1 ]] || { echo "error: Android device scenario did not pass; see $LOG" >&2; exit 1; }
REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
[[ "$(node -e 'console.log(JSON.parse(process.argv[1]).scenario.name)' "$REPORT")" = "$SCENARIO" ]] || {
    echo "error: Android device reported the wrong shared scenario; see $LOG" >&2; exit 1;
}
ADAPTER="$(rg 'PHYSBOX5_MARK_ADAPTER ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_ADAPTER //' || true)"
[[ -n "$ADAPTER" ]] || { echo "error: Android device adapter diagnostics missing; see $LOG" >&2; exit 1; }
printf '%s\n' "$ADAPTER" | node "$ANDROID_DIR/require-hardware-adapter.mjs"
CAPTURE_PHASE="${SCENARIO}-final"
if [[ "${PHYSBOX5_ANDROID_COMPARATOR_TOGGLE:-0}" = "1" ]]; then
    [[ "$SCENARIO" = "bootstrap" ]] || {
        echo "error: Android comparator toggle requires bootstrap" >&2; exit 2;
    }
    [[ "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_CONSTRUCTION_CAPTURE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || {
        echo "error: choose only the Android comparator interaction capture" >&2; exit 2;
    }
    node -e '
      const frame = JSON.parse(process.argv[1]);
      const ui = frame.u >>> 0;
      process.exit((ui & (1 << 28)) !== 0 && (ui & (1 << 27)) !== 0
        && (ui & (1 << 29)) === 0 && (ui & (1 << 26)) === 0
        && (ui & (1 << 25)) === 0 && (ui & (1 << 24)) === 0
        && (ui & (1 << 23)) === 0 ? 0 : 1);
    ' "$REPORT" || { echo "error: Android comparator must start aligned in PhysBox view; see $LOG" >&2; exit 1; }
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || {
        echo "error: could not determine Android display size" >&2; exit 1;
    }
    UI_POINTS="$(node -e '
      const w = +process.argv[1], h = +process.argv[2];
      const [version, pixelsPerPoint, toolbarScale, menuScale] =
        JSON.parse(process.argv[3]).v;
      if (version !== 12 || !(pixelsPerPoint > 0 && toolbarScale > 0 && menuScale > 0)) {
        throw new Error("invalid UI density marker");
      }
      console.log([
        Math.round(38 * toolbarScale), Math.round(50 * toolbarScale),
        Math.round(300 * menuScale), Math.round(104 * menuScale),
        Math.round(291 * menuScale), Math.round(135 * menuScale),
        Math.round(154 * toolbarScale), Math.round(50 * toolbarScale),
      ].join(" "));
    ' "$WIDTH" "$HEIGHT" "$REPORT")"
    read -r MENU_X MENU_Y WORLD_X WORLD_Y BOX2D_X BOX2D_Y CIRCLE_X CIRCLE_Y <<< "$UI_POINTS"
    BEFORE_CONSTRUCTION="$(node -e 'console.log(JSON.parse(process.argv[1]).bodyCommands.c)' "$REPORT")"
    BEFORE_BOX2D_BODIES="$(node -e 'console.log(JSON.parse(process.argv[1]).b)' "$REPORT")"
    "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$WORLD_X" "$WORLD_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$BOX2D_X" "$BOX2D_Y"
    BOX2D_SELECTED=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]);
          const ui = frame.u >>> 0;
          process.exit((ui & (1 << 29)) !== 0 && (ui & (1 << 28)) !== 0
            && (ui & (1 << 27)) !== 0 && (ui & (1 << 26)) === 0
            && (ui & (1 << 25)) === 0 ? 0 : 1);
        ' "$REPORT"; then BOX2D_SELECTED=1; break; fi
        sleep 0.25
    done
    [[ "$BOX2D_SELECTED" -eq 1 ]] || {
        echo "error: Android menu did not select the aligned Box2D view; see $LOG" >&2; exit 1;
    }
    "$ADB" -s "$SERIAL" shell input tap "$CIRCLE_X" "$CIRCLE_Y"
    PLACE_X=$((WIDTH * 80 / 100)); PLACE_Y=$((HEIGHT * 20 / 100))
    "$ADB" -s "$SERIAL" shell input tap "$PLACE_X" "$PLACE_Y"
    COMPARATOR_ACTION_VISIBLE=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]);
          const beforeConstruction = Number(process.argv[2]);
          const beforeBodies = Number(process.argv[3]);
          const ui = frame.u >>> 0;
          process.exit(frame.bodyCommands.c > beforeConstruction && frame.b > beforeBodies
            && (ui & (1 << 29)) !== 0 && (ui & (1 << 27)) !== 0
            && (ui & (1 << 26)) === 0 && (ui & (1 << 25)) === 0
            && (ui & (1 << 24)) === 0 && (ui & (1 << 23)) === 0 ? 0 : 1);
        ' "$REPORT" "$BEFORE_CONSTRUCTION" "$BEFORE_BOX2D_BODIES"; then
            COMPARATOR_ACTION_VISIBLE=1; break
        fi
        sleep 0.25
    done
    [[ "$COMPARATOR_ACTION_VISIBLE" -eq 1 ]] || {
        echo "error: Android Box2D placement did not reconcile with PhysBox; see $LOG" >&2; exit 1;
    }
    CAPTURE_PHASE="${SCENARIO}-box2d-placement"
fi
if [[ "${PHYSBOX5_ANDROID_SLEEP_VISUALS:-0}" = "1" && "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" ]]; then
    echo "error: Android sleep visuals require PHYSBOX5_ANDROID_SLEEP_TOGGLE=1" >&2; exit 2
fi
if [[ "${PHYSBOX5_ANDROID_SOLVER_SLIDER:-0}" = "1" ]]; then
    [[ "$SCENARIO" = "bootstrap" ]] || {
        echo "error: Android solver slider validation requires bootstrap" >&2; exit 2;
    }
    [[ "${PHYSBOX5_ANDROID_COMPARATOR_TOGGLE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_CONSTRUCTION_CAPTURE:-0}" != "1" \
        && "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || {
        echo "error: choose only the Android solver slider interaction" >&2; exit 2;
    }
fi
if [[ "${PHYSBOX5_ANDROID_SLEEP_CONFIG:-0}" = "1" && "${PHYSBOX5_ANDROID_SLEEP_TOGGLE:-0}" != "1" ]]; then
    echo "error: Android sleep configuration requires PHYSBOX5_ANDROID_SLEEP_TOGGLE=1" >&2; exit 2
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
if [[ "${PHYSBOX5_ANDROID_SOLVER_SLIDER:-0}" = "1" ]]; then
    SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
    WIDTH="${SIZE%x*}"; HEIGHT="${SIZE#*x}"
    [[ "$WIDTH" =~ ^[0-9]+$ && "$HEIGHT" =~ ^[0-9]+$ ]] || {
        echo "error: could not determine Android display size" >&2; exit 1;
    }
    SLIDER_POINTS="$(node -e '
      const w = +process.argv[1], h = +process.argv[2];
      const [version, pixelsPerPoint, toolbarScale, menuScale] =
        JSON.parse(process.argv[3]).v;
      if (version !== 12 || !(pixelsPerPoint > 0 && toolbarScale > 0 && menuScale > 0)) {
        throw new Error("invalid UI density marker");
      }
      const compact = h / pixelsPerPoint < 520 && w > h;
      const sliderX = compact ? 160 : 166;
      const sliderWidth = compact ? 128 : 194;
      const sliderY = (compact ? 176 + 30 : 236 + 38) + (compact ? 28 : 34) * 0.5;
      const x = (normalized) => Math.round((sliderX + sliderWidth * normalized) * menuScale);
      console.log([
        Math.round(38 * toolbarScale), Math.round(50 * toolbarScale),
        Math.round(188 * menuScale), Math.round(104 * menuScale),
        x((4 - 1) / (8 - 1)), Math.round(sliderY * menuScale),
        x((5 - 1) / (8 - 1)), Math.round(sliderY * menuScale),
        Math.round(279 * menuScale),
        Math.round((compact ? 316 : 541) * menuScale),
      ].join(" "));
    ' "$WIDTH" "$HEIGHT" "$REPORT")"
    read -r MENU_X MENU_Y SOLVERS_X SOLVERS_Y SLIDER_START_X SLIDER_START_Y \
        SLIDER_END_X SLIDER_END_Y APPLY_X APPLY_Y <<< "$SLIDER_POINTS"
    BEFORE_DISPATCHES="$(node -e 'console.log(JSON.parse(process.argv[1]).sceneContract.d)' "$REPORT")"
    BEFORE_HASH="$(node -e 'console.log(JSON.parse(process.argv[1]).sceneContract.k)' "$REPORT")"
    "$ADB" -s "$SERIAL" shell input tap "$MENU_X" "$MENU_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$SOLVERS_X" "$SOLVERS_Y"
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input swipe \
        "$SLIDER_START_X" "$SLIDER_START_Y" "$SLIDER_END_X" "$SLIDER_END_Y" 500
    sleep 0.25
    "$ADB" -s "$SERIAL" shell input tap "$APPLY_X" "$APPLY_Y"
    SLIDER_APPLIED=0
    for _ in $(seq 1 40); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]);
          const beforeDispatches = Number(process.argv[2]);
          const beforeHash = process.argv[3];
          process.exit(frame.sceneContract.d !== beforeDispatches
            && frame.sceneContract.k !== beforeHash
            && frame.scenario.complete === true && frame.scenario.passed === true ? 0 : 1);
        ' "$REPORT" "$BEFORE_DISPATCHES" "$BEFORE_HASH"; then
            SLIDER_APPLIED=1; break
        fi
        sleep 0.25
    done
    [[ "$SLIDER_APPLIED" -eq 1 ]] || {
        echo "error: Android solver slider did not apply a changed configuration; see $LOG" >&2; exit 1;
    }
    CAPTURE_PHASE="${SCENARIO}-solver-slider-applied"
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
      console.log(`${Math.round(154 * toolbarScale)} ${Math.round(50 * toolbarScale)}`);
    ' "$REPORT")"
    read -r TOOL_X TOOL_Y <<< "$TOOL_POINT"
    "$ADB" -s "$SERIAL" shell input tap "$TOOL_X" "$TOOL_Y"
    PLACE_X=$((WIDTH * 75 / 100)); PLACE_Y=$((HEIGHT * 30 / 100))
    "$ADB" -s "$SERIAL" shell input swipe "$PLACE_X" "$PLACE_Y" "$PLACE_X" "$PLACE_Y" 700
    CONSTRUCTION_VISIBLE=0
    for _ in $(seq 1 30); do
        "$ADB" -s "$SERIAL" logcat -d -t 10000 > "$LOG"
        REPORT="$(rg 'PHYSBOX5_MARK_FRAME_RENDERED ' "$LOG" | tail -1 | sed 's/^.*PHYSBOX5_MARK_FRAME_RENDERED //')"
        if node -e '
          const frame = JSON.parse(process.argv[1]); const before = Number(process.argv[2]);
          process.exit(frame.bodyCommands.c >= before + 2 && frame.d?.a >= 2 && frame.d?.b === false ? 0 : 1);
        ' "$REPORT" "$BEFORE_CONSTRUCTION"; then CONSTRUCTION_VISIBLE=1; break; fi
        sleep 0.25
    done
    [[ "$CONSTRUCTION_VISIBLE" -eq 1 ]] || { echo "error: Android hold-repeat construction diagnostic did not become visible; see $LOG" >&2; exit 1; }
    CAPTURE_PHASE="${SCENARIO}-construction-hold-repeat"
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
WINDOW_SIZE="$(physbox5_android_product_window_size "$ADB" "$SERIAL")"
DISPLAY_SIZE="$("$ADB" -s "$SERIAL" shell wm size | sed -n 's/.*Physical size: //p' | tail -1 | tr -d '\r')"
CAPTURE_KIND="app-screenshot"
if [[ "$WINDOW_SIZE" = "$DISPLAY_SIZE" ]]; then
    CAPTURED=0
    for _ in $(seq 1 5); do
        "$ADB" -s "$SERIAL" exec-out screencap -p > "$SCREENSHOT"
        if node "$ANDROID_DIR/require-complete-screenshot.mjs" "$SCREENSHOT"; then CAPTURED=1; break; fi
        sleep 0.2
    done
    [[ "$CAPTURED" -eq 1 ]] || { echo "error: Android screenshot remained incomplete" >&2; exit 1; }
else
    [[ "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" != "1" ]] || {
        echo "error: Android grab pixels require a directly capturable app window" >&2; exit 1;
    }
    CAPTURE_KIND="behavior-only-windowed-device"
    CAPTURE_PHASE="${CAPTURE_PHASE}-behavior-only"
    rm -f "$SCREENSHOT"
fi
if [[ "${PHYSBOX5_ANDROID_GRAB_CAPTURE:-0}" = "1" && "$CAPTURE_KIND" = "app-screenshot" ]]; then
    node "$ANDROID_DIR/require-grab-tether.mjs" "$SCREENSHOT" "$X2" "$Y2"
    "$ADB" -s "$SERIAL" shell input motionevent UP "$X2" "$Y2"
fi
REVISION="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY=false; [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]] && DIRTY=true
MODEL="$($ADB -s "$SERIAL" shell getprop ro.product.model | tr -d '\r')"
PRODUCT="$($ADB -s "$SERIAL" shell getprop ro.product.device | tr -d '\r')"
node -e '
  const fs = require("fs");
  const [path, runId, revision, dirty, report, adapter, scenario, capturePhase, captureKind,
    windowSize, displaySize, serial, model, product] = process.argv.slice(1);
  fs.writeFileSync(path, JSON.stringify({
    runId, sourceRevision: revision, sourceDirty: dirty === "true", host: "android",
    capturePhase, captureKind, windowSize, displaySize, device: { serial, model, product },
    report: JSON.parse(report), adapter: adapter ? JSON.parse(adapter) : null,
  }, null, 2) + "\n");
' "$PROVENANCE" "$(date +%s)-$$" "$REVISION" "$DIRTY" "$REPORT" "$ADAPTER" \
  "$SCENARIO" "$CAPTURE_PHASE" "$CAPTURE_KIND" "$WINDOW_SIZE" "$DISPLAY_SIZE" "$SERIAL" "$MODEL" "$PRODUCT"
echo "PHYSBOX5_VALIDATION_SUCCESS host=android device=$MODEL scenario=$SCENARIO capture=$CAPTURE_KIND provenance=$PROVENANCE"
