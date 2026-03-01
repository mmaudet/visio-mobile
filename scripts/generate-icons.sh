#!/usr/bin/env bash
# generate-icons.sh — Generate all platform icon PNGs from the SVG source.
# Requires: rsvg-convert (from librsvg) or Inkscape.
#   macOS: brew install librsvg
#   Linux: apt install librsvg2-bin
#
# Usage: ./scripts/generate-icons.sh
#
# Generates:
#   - Desktop (Tauri): icons/icon.png (512), 32x32.png, 128x128.png, 128x128@2x.png
#   - Android: mipmap-mdpi (48), mipmap-hdpi (72), mipmap-xhdpi (96),
#              mipmap-xxhdpi (144), mipmap-xxxhdpi (192)
#   - iOS: icon-1024.png (1024)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SVG_SQUARE="$ROOT/assets/icon/visio-icon.svg"
SVG_ROUNDED="$ROOT/assets/icon/visio-icon-rounded.svg"

# Check for rsvg-convert
if ! command -v rsvg-convert &>/dev/null; then
    echo "ERROR: rsvg-convert not found. Install with: brew install librsvg"
    exit 1
fi

echo "=== Generating Visio Mobile icons from SVG ==="

# --- Desktop (Tauri) ---
DESKTOP_ICONS="$ROOT/crates/visio-desktop/icons"
mkdir -p "$DESKTOP_ICONS"

echo "[Desktop] icon.png (512x512)"
rsvg-convert -w 512 -h 512 "$SVG_SQUARE" -o "$DESKTOP_ICONS/icon.png"

echo "[Desktop] 32x32.png"
rsvg-convert -w 32 -h 32 "$SVG_SQUARE" -o "$DESKTOP_ICONS/32x32.png"

echo "[Desktop] 128x128.png"
rsvg-convert -w 128 -h 128 "$SVG_SQUARE" -o "$DESKTOP_ICONS/128x128.png"

echo "[Desktop] 128x128@2x.png (256x256)"
rsvg-convert -w 256 -h 256 "$SVG_SQUARE" -o "$DESKTOP_ICONS/128x128@2x.png"

# --- macOS .icns (if sips and iconutil available) ---
if command -v iconutil &>/dev/null; then
    echo "[Desktop] icon.icns (macOS)"
    ICONSET=$(mktemp -d)/icon.iconset
    mkdir -p "$ICONSET"
    rsvg-convert -w 16   -h 16   "$SVG_SQUARE" -o "$ICONSET/icon_16x16.png"
    rsvg-convert -w 32   -h 32   "$SVG_SQUARE" -o "$ICONSET/icon_16x16@2x.png"
    rsvg-convert -w 32   -h 32   "$SVG_SQUARE" -o "$ICONSET/icon_32x32.png"
    rsvg-convert -w 64   -h 64   "$SVG_SQUARE" -o "$ICONSET/icon_32x32@2x.png"
    rsvg-convert -w 128  -h 128  "$SVG_SQUARE" -o "$ICONSET/icon_128x128.png"
    rsvg-convert -w 256  -h 256  "$SVG_SQUARE" -o "$ICONSET/icon_128x128@2x.png"
    rsvg-convert -w 256  -h 256  "$SVG_SQUARE" -o "$ICONSET/icon_256x256.png"
    rsvg-convert -w 512  -h 512  "$SVG_SQUARE" -o "$ICONSET/icon_256x256@2x.png"
    rsvg-convert -w 512  -h 512  "$SVG_SQUARE" -o "$ICONSET/icon_512x512.png"
    rsvg-convert -w 1024 -h 1024 "$SVG_SQUARE" -o "$ICONSET/icon_512x512@2x.png"
    iconutil -c icns "$ICONSET" -o "$DESKTOP_ICONS/icon.icns"
    rm -rf "$(dirname "$ICONSET")"
fi

# --- Windows .ico (if ImageMagick available) ---
if command -v magick &>/dev/null; then
    echo "[Desktop] icon.ico (Windows)"
    TMPDIR_ICO=$(mktemp -d)
    for SIZE in 16 24 32 48 64 128 256; do
        rsvg-convert -w $SIZE -h $SIZE "$SVG_SQUARE" -o "$TMPDIR_ICO/${SIZE}.png"
    done
    magick "$TMPDIR_ICO/16.png" "$TMPDIR_ICO/24.png" "$TMPDIR_ICO/32.png" \
           "$TMPDIR_ICO/48.png" "$TMPDIR_ICO/64.png" "$TMPDIR_ICO/128.png" \
           "$TMPDIR_ICO/256.png" "$DESKTOP_ICONS/icon.ico"
    rm -rf "$TMPDIR_ICO"
fi

# --- Android mipmap PNGs (fallback for pre-API 26 devices) ---
ANDROID_RES="$ROOT/android/app/src/main/res"

echo "[Android] mipmap-mdpi (48x48)"
rsvg-convert -w 48 -h 48 "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-mdpi/ic_launcher.png"

echo "[Android] mipmap-hdpi (72x72)"
rsvg-convert -w 72 -h 72 "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-hdpi/ic_launcher.png"

echo "[Android] mipmap-xhdpi (96x96)"
rsvg-convert -w 96 -h 96 "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-xhdpi/ic_launcher.png"

echo "[Android] mipmap-xxhdpi (144x144)"
rsvg-convert -w 144 -h 144 "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-xxhdpi/ic_launcher.png"

echo "[Android] mipmap-xxxhdpi (192x192)"
rsvg-convert -w 192 -h 192 "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher.png"

# Round variants (same as square for now; adaptive icon handles masking on API 26+)
for DPI_SIZE in "mdpi 48" "hdpi 72" "xhdpi 96" "xxhdpi 144" "xxxhdpi 192"; do
    DPI=$(echo "$DPI_SIZE" | cut -d' ' -f1)
    SIZE=$(echo "$DPI_SIZE" | cut -d' ' -f2)
    rsvg-convert -w "$SIZE" -h "$SIZE" "$SVG_SQUARE" -o "$ANDROID_RES/mipmap-$DPI/ic_launcher_round.png"
done

# --- iOS ---
IOS_APPICON="$ROOT/ios/VisioMobile/Assets.xcassets/AppIcon.appiconset"
mkdir -p "$IOS_APPICON"

echo "[iOS] icon-1024.png (1024x1024)"
rsvg-convert -w 1024 -h 1024 "$SVG_ROUNDED" -o "$IOS_APPICON/icon-1024.png"

echo ""
echo "=== Done! All icons generated. ==="
echo ""
echo "Notes:"
echo "  - Android API 26+ uses adaptive icon (vector drawable XML)."
echo "  - Android pre-26 falls back to mipmap PNGs."
echo "  - iOS uses the single 1024x1024 PNG (Xcode generates all sizes)."
echo "  - Desktop uses icon.png (512) + icon.icns (macOS) + icon.ico (Windows)."
