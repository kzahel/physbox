#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/host-env.sh"

MODE=check
case "${1:-}" in
    "") ;;
    --check) MODE=check ;;
    --install) MODE=install ;;
    --print-platform) physbox5_host_platform; exit 0 ;;
    *) echo "usage: $0 [--check|--install|--print-platform]" >&2; exit 2 ;;
esac

PLATFORM="$(physbox5_host_platform)"
[[ "$PLATFORM" != unsupported ]] || { echo "error: unsupported host platform: $(uname -s)" >&2; exit 1; }
printf '[host] platform=%s machine=%s\n' "$PLATFORM" "$(uname -m)"

if [[ "$MODE" = install ]]; then
    case "$PLATFORM" in
        linux)
            command -v apt-get >/dev/null || { echo "error: automatic Linux setup currently requires apt-get" >&2; exit 1; }
            command -v sudo >/dev/null || { echo "error: automatic Linux setup requires sudo" >&2; exit 1; }
            sudo apt-get install -y build-essential cmake libclang-dev pkg-config vulkan-tools xvfb x11-utils
            ;;
        macos)
            command -v xcode-select >/dev/null && xcode-select -p >/dev/null 2>&1 || {
                echo "error: install Xcode Command Line Tools with: xcode-select --install" >&2
                exit 1
            }
            command -v brew >/dev/null || { echo "error: install Homebrew from https://brew.sh" >&2; exit 1; }
            brew install cmake pkg-config
            ;;
    esac
fi

failures=0
check_command() {
    local command="$1" label="$2"
    if command -v "$command" >/dev/null; then
        printf '[ok]   %s: %s\n' "$label" "$(command -v "$command")"
    else
        printf '[fail] %s: missing command %s\n' "$label" "$command" >&2
        failures=$((failures + 1))
    fi
}

check_command git Git
check_command node Node.js
check_command npm npm
check_command cargo Cargo
check_command rustc Rust
check_command rustup rustup
check_command cmake CMake
check_command pkg-config pkg-config

if [[ "$PLATFORM" = linux ]]; then
    check_command cc 'C compiler'
    check_command c++ 'C++ compiler'
    check_command vulkaninfo 'Vulkan tools'
    check_command xvfb-run 'headless X server'
    if compgen -G '/usr/lib/llvm-*/lib/libclang.so*' >/dev/null \
        || compgen -G '/usr/lib/*/libclang-*.so*' >/dev/null; then
        echo '[ok]   libclang: available to bindgen'
    else
        echo '[fail] libclang shared library is missing' >&2
        failures=$((failures + 1))
    fi
    if command -v google-chrome >/dev/null || command -v chromium >/dev/null || command -v chromium-browser >/dev/null; then
        echo '[ok]   Chromium browser: available'
    else
        echo '[fail] Chrome or Chromium is required for the WebGPU browser lane' >&2
        failures=$((failures + 1))
    fi
else
    check_command clang 'Apple Clang'
    if [[ -x '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' ]]; then
        echo '[ok]   Google Chrome: /Applications/Google Chrome.app'
    else
        echo '[fail] Google Chrome is required for the WebGPU browser lane' >&2
        failures=$((failures + 1))
    fi
fi

if command -v rustc >/dev/null; then
    rust_version="$(rustc --version | awk '{print $2}')"
    version_at_least() {
        local actual="${1%%-*}" minimum="${2%%-*}" index actual_part minimum_part
        local IFS=.
        set -- $actual
        actual_parts=("$@")
        set -- $minimum
        minimum_parts=("$@")
        for index in 0 1 2; do
            actual_part="${actual_parts[$index]:-0}"
            minimum_part="${minimum_parts[$index]:-0}"
            if ((10#$actual_part > 10#$minimum_part)); then return 0; fi
            if ((10#$actual_part < 10#$minimum_part)); then return 1; fi
        done
        return 0
    }
    if version_at_least "$rust_version" 1.92.0; then
        printf '[ok]   rustc version: %s\n' "$rust_version"
    else
        printf '[fail] rustc %s is older than the workspace minimum 1.92.0\n' "$rust_version" >&2
        failures=$((failures + 1))
    fi
fi

if command -v rustup >/dev/null; then
    for target in wasm32-unknown-unknown aarch64-linux-android x86_64-linux-android; do
        if rustup target list --installed | grep -qx "$target"; then
            printf '[ok]   Rust target: %s\n' "$target"
        elif [[ "$MODE" = install ]]; then
            rustup target add "$target"
        else
            printf '[fail] Rust target missing: %s\n' "$target" >&2
            failures=$((failures + 1))
        fi
    done
fi

if command -v cargo-ndk >/dev/null; then
    printf '[ok]   cargo-ndk: %s\n' "$(cargo ndk --version | head -1)"
elif [[ "$MODE" = install && -x "$(command -v cargo || true)" ]]; then
    cargo install cargo-ndk
else
    echo '[fail] cargo-ndk is missing; install it with: cargo install cargo-ndk' >&2
    failures=$((failures + 1))
fi

if [[ "$PLATFORM" = linux ]]; then
    physbox5_host_import_graphical_session force
    if physbox5_host_x11_available; then
        printf '[ok]   graphical session: DISPLAY=%s WAYLAND_DISPLAY=%s\n' "$DISPLAY" "${WAYLAND_DISPLAY:-none}"
    else
        echo '[warn] no accessible graphical login session; desktop tests need a login and Android AVDs will use Xvfb'
    fi
fi

if [[ -x "$REPO_ROOT/android/check-env.sh" ]]; then
    if "$REPO_ROOT/android/check-env.sh"; then
        echo '[ok]   Android SDK/NDK'
    else
        echo '[fail] Android SDK/NDK setup is incomplete' >&2
        failures=$((failures + 1))
    fi
fi

if [[ "$MODE" = install && "$failures" -eq 0 ]]; then
    (cd "$REPO_ROOT" && npm ci && npm run references:sync)
elif [[ "$MODE" = check ]] && command -v node >/dev/null; then
    if (cd "$REPO_ROOT" && node scripts/references.mjs status); then
        echo '[ok]   declared source references'
    else
        echo '[fail] source references are incomplete; run: npm run setup:host' >&2
        failures=$((failures + 1))
    fi
fi

if [[ "$failures" -ne 0 ]]; then
    printf 'PHYSBOX5_HOST_SETUP_FAILED failures=%s\n' "$failures" >&2
    exit 1
fi
printf 'PHYSBOX5_HOST_SETUP_SUCCESS platform=%s mode=%s\n' "$PLATFORM" "$MODE"
