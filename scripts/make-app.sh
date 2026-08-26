#!/usr/bin/env bash
# Build md2pdf.app — a double-clickable macOS application bundle.
#
# Run it on the Mac. The container this repo is usually worked in is Linux and cannot
# link `md2pdf-gui` at all (no system GL/X11), let alone produce a Mach-O binary.
#
#   ./scripts/make-app.sh              # build and install to /Applications
#   INSTALL_TO=~/Desktop ./scripts/make-app.sh
#   INSTALL_TO= ./scripts/make-app.sh  # build only, leave it in target/
#
# **Unsigned, deliberately.** Signing and notarization were declined by decision
# (2026-08-16, `roadmap.md` §6) at ~$200-500/yr. It does not matter for a locally built
# app: quarantine is set by the *downloader*, so a bundle assembled on this machine opens
# on a double-click like any other. It would matter the moment one is sent to someone.
set -euo pipefail
cd "$(dirname "$0")/.."

# **The identity, decided deliberately** — `roadmap.md` §5 wants it set no later than the
# settings phase, because changing it after anything has been persisted under it strands
# what was persisted. Note that md2pdf's *own* config directory does not depend on it:
# `roots::config_dir` resolves `~/Library/Application Support/md2pdf` from the name.
BUNDLE_ID="com.rhizolabs.md2pdf"
INSTALL_TO="${INSTALL_TO-/Applications}"

APP_NAME="md2pdf"
ICON_PNG="$PWD/assets/icon/md2pdf-1024.png"
STAGE="target/bundle"
APP="$STAGE/$APP_NAME.app"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "make-app: this builds a macOS bundle and needs macOS — sips, iconutil and a"
    echo "          Mach-O toolchain. Run it on the Mac, not in the container."
    exit 1
fi

# One source of truth for the version: the workspace manifest.
VERSION=$(awk '/^\[workspace\.package\]/{f=1} f&&/^version/{gsub(/[^0-9.]/,"");print;exit}' Cargo.toml)

echo "=== binary ==="
# Release, not dev: the dev profile's preview raster is noticeably slow, and this is the
# build someone will actually use.
cargo build --release -p md2pdf-gui

echo
echo "=== icon ==="
# Drawn by md2pdf itself — see `walking_skeleton/look.rs`. Regenerated only when missing,
# because it costs a test-profile build of the engine. ICON_OUT must be absolute: cargo
# runs the test with the *crate* as its working directory.
if [ ! -f "$ICON_PNG" ]; then
    echo "no $ICON_PNG yet — drawing it"
    mkdir -p "$(dirname "$ICON_PNG")"
    ICON_OUT="$ICON_PNG" cargo test -p md2pdf-engine --test walking_skeleton \
        draw_the_icon -- --ignored --nocapture
fi

# `.icns` wants every size as its own file, at 1x and 2x. Downscaled from the single
# 1024 master rather than re-rendered per size: the wordmark is the same drawing at every
# scale, so there is nothing a re-render would say that a resample does not.
ICONSET="$STAGE/$APP_NAME.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ICON_PNG" \
        --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "$ICON_PNG" \
        --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil --convert icns "$ICONSET" --output "$STAGE/$APP_NAME.icns"
rm -rf "$ICONSET"

echo
echo "=== bundle ==="
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/release/md2pdf-gui" "$APP/Contents/MacOS/$APP_NAME"
cp "$STAGE/$APP_NAME.icns" "$APP/Contents/Resources/$APP_NAME.icns"

# **Beside the binary, not in Resources.** `roots::beside_binary()` looks in
# `templates/` next to the executable, which inside a bundle is `Contents/MacOS/`. The
# Mac convention would put them in `Contents/Resources/`, and following it would need a
# macOS arm in `roots` — a code change, so not made silently here. Either way the app
# still runs without them: the catalogue falls back to the built-in `github-print`.
cp -R templates "$APP/Contents/MacOS/templates"

# `NSHighResolutionCapable` is not optional: without it macOS runs the app through the
# 2x upscaler and the preview — the whole point of the window — renders soft.
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>                  <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>           <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>            <string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key>            <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>              <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>           <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
    <key>CFBundleVersion</key>               <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>        <string>11.0</string>
    <key>LSApplicationCategoryType</key>     <string>public.app-category.productivity</string>
    <key>NSHighResolutionCapable</key>       <true/>
</dict>
</plist>
PLIST

echo "built $APP"

if [ -n "$INSTALL_TO" ]; then
    echo
    echo "=== install ==="
    # Removed first: `cp -R` over a live bundle merges rather than replaces, which is how
    # a stale executable survives an upgrade.
    rm -rf "${INSTALL_TO%/}/$APP_NAME.app"
    cp -R "$APP" "${INSTALL_TO%/}/"
    # Finder caches icons per path; touching the bundle is what makes the new one appear
    # without a relaunch of the Dock.
    touch "${INSTALL_TO%/}/$APP_NAME.app"
    echo "installed to ${INSTALL_TO%/}/$APP_NAME.app"
    echo
    echo "open it:  open '${INSTALL_TO%/}/$APP_NAME.app'"
    echo "dock it:  drag it from Finder, or right-click its Dock icon > Options > Keep in Dock"
fi
