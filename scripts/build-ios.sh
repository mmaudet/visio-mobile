#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

TARGET="${1:-device}"

if [ "$TARGET" = "sim" ]; then
    RUST_TARGET="aarch64-apple-ios-sim"
    echo "==> Building for iOS Simulator..."
else
    RUST_TARGET="aarch64-apple-ios"
    echo "==> Building for iOS Device..."
fi

cargo build --target "$RUST_TARGET" -p visio-ffi -p visio-video --release

echo "==> Libraries at:"
ls -la "target/$RUST_TARGET/release/libvisio_ffi.a"
ls -la "target/$RUST_TARGET/release/libvisio_video.a"

echo ""
echo "To integrate with Xcode:"
echo "  1. Add libvisio_ffi.a and libvisio_video.a to Link Binary With Libraries"
echo "  2. Set Library Search Path to: \$(PROJECT_DIR)/../../target/$RUST_TARGET/release"
echo "  3. Add Other Linker Flags: -lvisio_ffi -lvisio_video"
echo "  4. Add bridging header pointing to ios/VisioMobile/Generated/visioFFI.h"
