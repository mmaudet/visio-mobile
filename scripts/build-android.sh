#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Cross-compiling Rust for Android arm64..."
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release

echo "==> Copying .so files to jniLibs..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libvisio_ffi.so android/app/src/main/jniLibs/arm64-v8a/
cp target/aarch64-linux-android/release/libvisio_video.so android/app/src/main/jniLibs/arm64-v8a/

echo "==> Building APK..."
cd android
./gradlew assembleDebug

echo "==> Done! APK at:"
find app/build/outputs/apk -name "*.apk" 2>/dev/null
