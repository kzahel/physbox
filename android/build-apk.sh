#!/usr/bin/env bash
set -euo pipefail

ANDROID_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ANDROID_DIR/.." && pwd)"
source "$ANDROID_DIR/toolchain.sh"
physbox5_android_detect_sdk
physbox5_android_detect_ndk "27.0.12077973"
ABI_SELECTION="${PHYSBOX5_ANDROID_ABI:-all}"
case "$ABI_SELECTION" in
    all) ABIS=(arm64-v8a x86_64) ;;
    arm64-v8a|x86_64) ABIS=("$ABI_SELECTION") ;;
    *) echo "error: unsupported Android ABI: $ABI_SELECTION" >&2; exit 1 ;;
esac
cargo ndk --version >/dev/null
NDK_PREBUILT="$(find "$PHYSBOX5_ANDROID_NDK/toolchains/llvm/prebuilt" -maxdepth 1 -mindepth 1 -type d | head -1)"
rm -rf "$ANDROID_DIR/jniLibs"
for ABI in "${ABIS[@]}"; do
    case "$ABI" in arm64-v8a) TARGET=aarch64-linux-android ;; x86_64) TARGET=x86_64-linux-android ;; esac
    rustup target list --installed | grep -qx "$TARGET" || { echo "error: missing Rust target $TARGET" >&2; exit 1; }
    (cd "$REPO_ROOT" && cargo ndk -t "$ABI" --platform 28 -o android/jniLibs build --release -p physbox-android --lib)
    cp "$NDK_PREBUILT/sysroot/usr/lib/$TARGET/libc++_shared.so" "$ANDROID_DIR/jniLibs/$ABI/"
done
(cd "$ANDROID_DIR" && ./gradlew assembleDebug)
echo "APK: android/app/build/outputs/apk/debug/app-debug.apk"
