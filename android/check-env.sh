#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
physbox5_android_detect_ndk "27.0.12077973"
printf 'SDK: %s\nNDK: %s\n' "$PHYSBOX5_ANDROID_SDK" "$PHYSBOX5_ANDROID_NDK"
cargo ndk --version
for target in aarch64-linux-android x86_64-linux-android; do
    rustup target list --installed | grep -qx "$target" && echo "Rust target: $target installed" || echo "Rust target: $target missing"
done
