#!/bin/bash
#
# Frame screenshots with fastlane frameit and copy them into the metadata
# folders that the upload lanes read from.
#
# Usage:
#   ./scripts/frame-screenshots.sh [platform]
#
# Arguments:
#   platform  - "ios" or "android" (default: auto-detect)

set -euo pipefail

MOBILE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOCALE="${LOCALE:-en-US}"

detect_platform() {
  if [ -n "${1:-}" ]; then
    echo "$1"
    return
  fi
  if xcrun simctl list devices booted 2>/dev/null | grep -q "Booted"; then
    echo "ios"
  elif adb devices 2>/dev/null | grep -q "device$"; then
    echo "android"
  else
    echo "android"
  fi
}

PLATFORM="$(detect_platform "${1:-}")"
SCREENSHOTS_DIR="$MOBILE_DIR/fastlane/screenshots/$PLATFORM/$LOCALE"

# "ipad" reuses the iOS frame config and uploads to the iOS metadata folder
# (App Store keeps every device size in one screenshots directory).
CONFIG_PLATFORM="$PLATFORM"
[ "$PLATFORM" = "ipad" ] && CONFIG_PLATFORM="ios"

if [ ! -d "$SCREENSHOTS_DIR" ]; then
  echo "Error: Screenshots directory not found: $SCREENSHOTS_DIR" >&2
  exit 1
fi

if ! command -v bundle &>/dev/null; then
  echo "Error: bundle not found. Install bundler and run 'bundle install' in mobile/." >&2
  exit 1
fi

echo "Preparing frameit configuration..."
cp "$MOBILE_DIR/fastlane-config/$CONFIG_PLATFORM/Framefile.json" "$SCREENSHOTS_DIR/"
cp "$MOBILE_DIR/fastlane-config/assets/background.jpg" "$SCREENSHOTS_DIR/"
cp "$MOBILE_DIR/fastlane-config/assets/Regular.ttf" "$SCREENSHOTS_DIR/"

cd "$SCREENSHOTS_DIR"

echo "Framing screenshots in $SCREENSHOTS_DIR..."
# frameit reads Framefile.json from the current directory; bundler walks up to mobile/Gemfile.
BUNDLE_GEMFILE="$MOBILE_DIR/Gemfile" bundle exec fastlane frameit silver

if [ "$PLATFORM" = "android" ]; then
  METADATA_DIR="$MOBILE_DIR/fastlane/metadata/android/$LOCALE/images/phoneScreenshots"
else
  METADATA_DIR="$MOBILE_DIR/fastlane/metadata/ios/$LOCALE/screenshots"
fi # iphone + ipad both land in the iOS screenshots dir

mkdir -p "$METADATA_DIR"

echo "Copying framed screenshots to metadata..."
# Pattern: [Device-Name-]NN_name_framed.png -> [Device-Name-]N.png
for src in *_framed.png; do
  [ -f "$src" ] || continue
  if [[ "$src" =~ ^(.*)-([0-9][0-9])_.*_framed\.png$ ]]; then
    device_prefix="${BASH_REMATCH[1]}"
    num_part="${BASH_REMATCH[2]}"
  elif [[ "$src" =~ ^([0-9][0-9])_.*_framed\.png$ ]]; then
    device_prefix=""
    num_part="${BASH_REMATCH[1]}"
  else
    echo "  Skipping unknown filename format: $src"
    continue
  fi

  num="$((10#$num_part))"
  if [ -n "$device_prefix" ]; then
    dest_name="${device_prefix}-${num}.png"
  else
    dest_name="${num}.png"
  fi

  cp "$src" "$METADATA_DIR/$dest_name"
  echo "  $src -> $METADATA_DIR/$dest_name"
done

echo "Done!"
