#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LANG="${1:-all}"
UDL="crates/visio-ffi/src/visio.udl"

generate_kotlin() {
    echo "==> Generating Kotlin UniFFI bindings..."
    # On Linux, ort_sys and webrtc_sys both bundle protobuf, causing duplicate
    # symbol errors when linking the host binary.  Allow multiple definitions
    # so the uniffi-bindgen CLI (which only parses the UDL) can link.
    local extra_flags=""
    if [ "$(uname)" = "Linux" ]; then
        extra_flags="-C link-args=-Wl,--allow-multiple-definition"
    fi
    RUSTFLAGS="${RUSTFLAGS:-} $extra_flags" \
    cargo run -p visio-ffi --features cli --bin uniffi-bindgen generate \
        "$UDL" --language kotlin \
        --out-dir android/app/src/main/kotlin/generated/
    echo "    Done."
}

generate_swift() {
    echo "==> Generating Swift UniFFI bindings..."
    cargo run -p visio-ffi --features cli --bin uniffi-bindgen generate \
        "$UDL" --language swift \
        --out-dir ios/VisioMobile/Generated/

    # Patch modulemap to include visio_native.h (raw C FFI functions).
    # UniFFI only generates the visioFFI.h header in the modulemap,
    # but iOS also needs visio_native.h for audio/video C FFI.
    MODULEMAP="ios/VisioMobile/Generated/visioFFI.modulemap"
    if ! grep -q 'visio_native.h' "$MODULEMAP"; then
        sed -i '' 's|header "visioFFI.h"|header "visioFFI.h"\n    header "visio_native.h"|' "$MODULEMAP"
        echo "    Patched modulemap to include visio_native.h"
    fi
    echo "    Done."
}

case "$LANG" in
    kotlin)  generate_kotlin ;;
    swift)   generate_swift ;;
    all)     generate_kotlin; generate_swift ;;
    *)       echo "Usage: $0 [kotlin|swift|all]"; exit 1 ;;
esac
