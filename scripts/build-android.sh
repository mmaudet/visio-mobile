#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Cross-compiling Rust for Android arm64..."
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release

echo "==> Copying .so files to jniLibs (clean first)..."
rm -rf android/app/src/main/jniLibs/arm64-v8a
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libvisio_ffi.so android/app/src/main/jniLibs/arm64-v8a/
cp target/aarch64-linux-android/release/libvisio_video.so android/app/src/main/jniLibs/arm64-v8a/

echo "==> Copying libwebrtc.jar to app/libs..."
mkdir -p android/app/libs
WEBRTC_JAR=$(find target/release/build -name "libwebrtc.jar" -path "*/android-arm64-release/*" 2>/dev/null | head -1)
if [ -n "$WEBRTC_JAR" ]; then
    cp "$WEBRTC_JAR" android/app/libs/
    echo "    Found: $WEBRTC_JAR"
else
    echo "    WARNING: libwebrtc.jar not found in build artifacts"
fi

echo "==> Building APK..."
cd android
./gradlew assembleDebug

echo "==> Done! APK at:"
find app/build/outputs/apk -name "*.apk" 2>/dev/null
