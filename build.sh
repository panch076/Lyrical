#!/bin/bash
# Builds Lyrical.app into ./build
set -euo pipefail
cd "$(dirname "$0")"

CONFIG="${CONFIG:-release}"
APP="build/Lyrical.app"

# UNIVERSAL=1 builds a fat arm64 + x86_64 binary for distribution.
if [ "${UNIVERSAL:-0}" = "1" ]; then
    swift build -c "$CONFIG" --arch arm64 --arch x86_64
    BINARY=".build/apple/Products/$(tr '[:lower:]' '[:upper:]' <<< "${CONFIG:0:1}")${CONFIG:1}/Lyrical"
else
    swift build -c "$CONFIG"
    BINARY=".build/$CONFIG/Lyrical"
fi

if [ ! -f Resources/AppIcon.icns ]; then
    echo "Rendering app icon…"
    ICONSET="$(mktemp -d)/AppIcon.iconset"
    mkdir -p "$ICONSET"
    swift scripts/make_icon.swift "$ICONSET/icon_1024.png"
    for s in 16 32 128 256 512; do
        sips -z $s $s "$ICONSET/icon_1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
        sips -z $((s*2)) $((s*2)) "$ICONSET/icon_1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
    done
    rm "$ICONSET/icon_1024.png"
    iconutil -c icns "$ICONSET" -o Resources/AppIcon.icns
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/Lyrical"
cp Resources/Info.plist "$APP/Contents/Info.plist"
cp Resources/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

# Sign with a Developer ID / Apple Development identity if provided (needed for
# ShazamKit's catalog), otherwise ad-hoc.
codesign --force --deep --sign "${SIGN_IDENTITY:--}" "$APP"
echo "Built $APP"
