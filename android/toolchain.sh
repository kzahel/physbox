#!/usr/bin/env bash

PHYSBOX5_ANDROID_TOOLCHAIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$PHYSBOX5_ANDROID_TOOLCHAIN_DIR/../scripts/host-env.sh"

physbox5_android_detect_sdk() {
    local candidates=()
    [[ -z "${ANDROID_HOME:-}" ]] || candidates+=("$ANDROID_HOME")
    [[ -z "${ANDROID_SDK_ROOT:-}" ]] || candidates+=("$ANDROID_SDK_ROOT")
    case "$(uname -s)" in
        Darwin) candidates+=("$HOME/Library/Android/sdk") ;;
        Linux) candidates+=("$HOME/Android/Sdk") ;;
    esac
    local candidate
    for candidate in "${candidates[@]}"; do
        if [[ -x "$candidate/platform-tools/adb" ]]; then
            PHYSBOX5_ANDROID_SDK="$candidate"
            export ANDROID_HOME="$candidate" ANDROID_SDK_ROOT="$candidate"
            return 0
        fi
    done
    echo "error: Android SDK not found; set ANDROID_HOME or ANDROID_SDK_ROOT" >&2
    return 1
}

physbox5_android_detect_ndk() {
    local version="$1" candidates=()
    [[ -z "${ANDROID_NDK_HOME:-}" ]] || candidates+=("$ANDROID_NDK_HOME")
    [[ -z "${ANDROID_NDK_ROOT:-}" ]] || candidates+=("$ANDROID_NDK_ROOT")
    candidates+=("$PHYSBOX5_ANDROID_SDK/ndk/$version")
    local candidate
    for candidate in "${candidates[@]}"; do
        if [[ -d "$candidate/toolchains/llvm/prebuilt" ]]; then
            PHYSBOX5_ANDROID_NDK="$candidate"
            export ANDROID_NDK_HOME="$candidate" ANDROID_NDK_ROOT="$candidate"
            return 0
        fi
    done
    echo "error: Android NDK $version not found" >&2
    return 1
}

physbox5_android_require_tool() {
    local relative="$1" description="$2" path
    path="$PHYSBOX5_ANDROID_SDK/$relative"
    [[ -x "$path" ]] || { echo "error: $description not found: $path" >&2; return 1; }
    printf '%s\n' "$path"
}

physbox5_android_configure_emulator_display() {
    local shell_had_x11=1
    PHYSBOX5_ANDROID_EMULATOR_PREFIX=()
    PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS=()
    if [[ "$(physbox5_host_platform)" = linux ]] && ! physbox5_host_x11_available; then
        shell_had_x11=0
        physbox5_host_import_graphical_session force
    fi
    if [[ "$(physbox5_host_platform)" = linux && "$shell_had_x11" -eq 0 ]]; then
        PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS=(-no-window)
        if ! physbox5_host_x11_available; then
            command -v xvfb-run >/dev/null || {
                echo "error: Linux AVD validation needs an accessible graphical session or xvfb-run" >&2
                return 1
            }
            PHYSBOX5_ANDROID_EMULATOR_PREFIX=(xvfb-run -a)
        fi
    fi
}

physbox5_android_stop_emulator() {
    local adb="$1" serial="$2" emulator_pid="$3" avd_name="$4"
    "$adb" -s "$serial" emu kill >/dev/null 2>&1 || true
    for _ in $(seq 1 30); do
        local process_live=0 device_live=0
        if [[ -n "$emulator_pid" ]] && kill -0 "$emulator_pid" 2>/dev/null; then
            process_live=1
        fi
        if "$adb" devices | awk -v serial="$serial" '$1 == serial && $2 == "device" {found=1} END {exit found ? 0 : 1}'; then
            device_live=1
        fi
        if [[ "$process_live" -eq 0 && "$device_live" -eq 0 ]]; then
            [[ -z "$emulator_pid" ]] || wait "$emulator_pid" 2>/dev/null || true
            echo "PHYSBOX5_ANDROID_AVD_SHUTDOWN_SUCCESS avd=$avd_name"
            return 0
        fi
        sleep 1
    done
    if [[ -n "$emulator_pid" ]] && kill -0 "$emulator_pid" 2>/dev/null; then
        kill -TERM "$emulator_pid" 2>/dev/null || true
        sleep 2
    fi
    if [[ -n "$emulator_pid" ]] && kill -0 "$emulator_pid" 2>/dev/null; then
        kill -KILL "$emulator_pid" 2>/dev/null || true
        sleep 1
    fi
    [[ -z "$emulator_pid" ]] || wait "$emulator_pid" 2>/dev/null || true
    if "$adb" devices | awk -v serial="$serial" '$1 == serial && $2 == "device" {found=1} END {exit found ? 0 : 1}'; then
        echo "error: AVD $avd_name ($serial) remained live after shutdown" >&2
        return 1
    fi
    echo "PHYSBOX5_ANDROID_AVD_SHUTDOWN_SUCCESS avd=$avd_name"
}
