#!/usr/bin/env bash
# Build and package the LightCraft IR fork as a standalone macOS .app bundle
# (`LightCraft IR.app`, bundle ID `art.irchrome.lightcraft-ir`) and optionally
# install it to /Applications or create a distributable DMG.
#
# Usage:
#   tools/install-macos-app.sh [--install] [--dmg PATH] [--nikon-he] [--skip-build]
#
# Options:
#   --install       Install the built bundle to /Applications/LightCraft IR.app
#   --dmg PATH      Create a drag-to-Applications DMG at PATH (plus .sha256)
#   --nikon-he      Enable local LibRaw Nikon HE NEF decoding (local use only;
#                   do NOT use for public MIT/Apache-2.0 release binaries)
#   --skip-build    Reuse existing release binary in $CARGO_TARGET_DIR/release/lightcraft
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET=11.0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/ir-agent}"

INSTALL=0
DMG_OUT=""
NIKON_HE=0
SKIP_BUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install) INSTALL=1; shift ;;
    --dmg) DMG_OUT="$2"; shift 2 ;;
    --nikon-he) NIKON_HE=1; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ "$INSTALL" = 0 && -z "$DMG_OUT" ]]; then
  INSTALL=1
fi

SHORT_VERSION="$(awk '
  /^\[/ { in_pkg = ($0 == "[workspace.package]"); next }
  in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
' "$ROOT/Cargo.toml")"
BUILD_SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
FULL_VERSION="${LIGHTCRAFT_IR_VERSION:-${SHORT_VERSION}-ir}"

echo "==> LightCraft IR ${FULL_VERSION} (commit ${BUILD_SHA}, nikon-he=${NIKON_HE})"

if [[ "$SKIP_BUILD" = 0 ]]; then
  BUILD_ARGS=(build --release -p lightcraft -p lightcraft-cli)
  if [[ "$NIKON_HE" = 1 ]]; then
    BUILD_ARGS+=(--features lightcraft/nikon-he,lightcraft-cli/nikon-he)
  fi
  (cd "$ROOT" && cargo "${BUILD_ARGS[@]}")
fi

BIN="$CARGO_TARGET_DIR/release/lightcraft"
if [[ ! -x "$BIN" ]]; then
  echo "error: binary not found at $BIN" >&2
  exit 1
fi

WORK="$CARGO_TARGET_DIR/macos-ir-bundle"
APP="$WORK/LightCraft IR.app"
rm -rf "$WORK"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/Licenses"

cp "$BIN" "$APP/Contents/MacOS/LightCraft IR"
cp "$ROOT/assets/app-icon/lightcraft-ir.icns" "$APP/Contents/Resources/LightCraft.icns"

sed \
  -e "s|<string>LightCraft</string>|<string>LightCraft IR</string>|g" \
  -e "s|<string>ai.storyteller.lightcraft</string>|<string>art.irchrome.lightcraft-ir</string>|g" \
  -e "s|<string>LightCraft IR</string><!--icon-->|<string>LightCraft</string>|g" \
  -e "s/@VERSION@/$FULL_VERSION/g" \
  -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" \
  -e "s/@BUILD_SHA@/$BUILD_SHA/g" \
  "$ROOT/packaging/macos/Info.plist.in" >"$APP/Contents/Info.plist"

# Keep CFBundleIconFile pointing to LightCraft.icns
/usr/libexec/PlistBuddy -c "Set :CFBundleIconFile LightCraft" "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
printf 'APPL????' >"$APP/Contents/PkgInfo"

for f in README.md LICENSE-MIT LICENSE-APACHE NOTICE; do
  [[ -f "$ROOT/$f" ]] && cp "$ROOT/$f" "$APP/Contents/Resources/Licenses/"
done
[[ -f "$ROOT/assets/fonts/OFL-Inter.txt" ]] && cp "$ROOT/assets/fonts/OFL-Inter.txt" "$APP/Contents/Resources/Licenses/"

xattr -cr "$APP"
ENTITLEMENTS="$ROOT/packaging/macos/entitlements.plist"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENTITLEMENTS" "$APP/Contents/MacOS/LightCraft IR"
codesign --force --sign - --timestamp=none --options runtime --entitlements "$ENTITLEMENTS" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"

if [[ "$INSTALL" = 1 ]]; then
  DEST="/Applications/LightCraft IR.app"
  echo "==> installing to $DEST"
  rm -rf "$DEST"
  ditto "$APP" "$DEST"
  codesign --verify --strict --deep "$DEST"
  echo "==> installed: $DEST"
fi

if [[ -n "$DMG_OUT" ]]; then
  mkdir -p "$(dirname "$DMG_OUT")"
  STAGE="$WORK/dmg"
  mkdir -p "$STAGE"
  ditto "$APP" "$STAGE/LightCraft IR.app"
  ln -s /Applications "$STAGE/Applications"
  rm -f "$DMG_OUT"
  hdiutil create -srcfolder "$STAGE" -volname "LightCraft IR" -fs HFS+ -format UDZO \
    -imagekey zlib-level=9 -ov "$DMG_OUT"
  shasum -a 256 "$DMG_OUT" > "${DMG_OUT}.sha256"
  echo "==> created DMG: $DMG_OUT"
  cat "${DMG_OUT}.sha256"
fi
