#!/usr/bin/env bash
set -euo pipefail
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to NDK r28c or newer}"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
script_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
ndk_tools="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk_tools/aarch64-linux-android29-clang"
export CC_aarch64_linux_android="$ndk_tools/aarch64-linux-android29-clang"
export AR_aarch64_linux_android="$ndk_tools/llvm-ar"
export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384'
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-1}"
cargo rustc --locked --manifest-path "$script_root/native-runtime/Cargo.toml" --crate-type cdylib --lib --no-default-features --release --target aarch64-linux-android
mkdir -p "$script_root/native-android/app/src/main/jniLibs/arm64-v8a"
native_target_dir="${CARGO_TARGET_DIR:-$script_root/native-runtime/target}"
cp "$native_target_dir/aarch64-linux-android/release/libravenfield_native_host.so" "$script_root/native-android/app/src/main/jniLibs/arm64-v8a/"
"$script_root/gradlew" -p "$script_root" :nativeApp:assembleDebug :nativeApp:testDebugUnitTest :nativeApp:lintDebug
