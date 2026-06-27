#!/bin/bash
#
# Capture screenshots with Maestro and organize them for fastlane frameit.
#
# Usage:
#   ./scripts/maestro-screenshots.sh [platform] [device-name]
#
# Arguments:
#   platform     - "ios" or "android" (default: auto-detect from running simulator/emulator)
#   device-name  - optional fastlane device name to prefix screenshots with
#
# Output (frameit-compatible):
#   fastlane/screenshots/ios/en-US/*.png
#   fastlane/screenshots/android/en-US/*.png
#
# Then frame them:
#   ./scripts/frame-screenshots.sh <platform>

set -euo pipefail

MOBILE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FLOW="$MOBILE_DIR/.maestro/screenshots.yaml"
LOCALE="${LOCALE:-en-US}"

if [ -d "$HOME/.maestro/bin" ]; then
  export PATH="$HOME/.maestro/bin:$PATH"
fi

if ! command -v maestro &>/dev/null; then
  echo "Error: maestro not found. Install it with:" >&2
  echo "  curl -Ls \"https://get.maestro.mobile.dev\" | bash" >&2
  exit 1
fi

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
    echo "Error: No running simulator or emulator detected." >&2
    echo "Start a simulator/emulator first, or pass 'ios' or 'android'." >&2
    exit 1
  fi
}

PLATFORM="$(detect_platform "${1:-}")"
DEVICE_NAME="${2:-}"
OUTPUT_DIR="$MOBILE_DIR/fastlane/screenshots/$PLATFORM/$LOCALE"

# Resolve the device id so Maestro targets the right platform when both a
# simulator and emulator are running. "ipad" runs on a booted iOS simulator;
# pass the device name (e.g. "iPad Pro 12.9-inch") as the second argument.
DEVICE_ARG=()
if [ "$PLATFORM" = "ios" ] || [ "$PLATFORM" = "ipad" ]; then
  if [ -n "$DEVICE_NAME" ]; then
    UDID="$(xcrun simctl list devices booted | grep -F "$DEVICE_NAME (" | grep -Eo '[0-9A-F-]{36}' | head -n 1 || true)"
  fi
  UDID="${UDID:-$(xcrun simctl list devices booted | grep -Eo '[0-9A-F-]{36}' | head -n 1 || true)}"
  [ -n "$UDID" ] && DEVICE_ARG=(--udid="$UDID")
else
  SERIAL="$(adb devices | awk '/device$/{print $1; exit}' || true)"
  [ -n "$SERIAL" ] && DEVICE_ARG=(--udid="$SERIAL")
fi

echo "Platform: $PLATFORM"
echo "Output:   $OUTPUT_DIR"
echo ""

mkdir -p "$OUTPUT_DIR"
# Drop any previous captures so stale screens don't linger.
rm -f "$OUTPUT_DIR"/[0-9][0-9]_*.png

echo "Running Maestro flow..."
cd "$OUTPUT_DIR"
maestro "${DEVICE_ARG[@]}" test "$FLOW"

# Optionally prefix with a fastlane device name (Device-Name-NN_name.png).
if [ -n "$DEVICE_NAME" ]; then
  echo "Prefixing screenshots with device name '$DEVICE_NAME'..."
  for f in [0-9][0-9]_*.png; do
    [ -f "$f" ] || continue
    mv -f "$f" "$DEVICE_NAME-$f"
  done
fi

echo ""
echo "Screenshots saved to: $OUTPUT_DIR"
ls -1 "$OUTPUT_DIR"/*.png 2>/dev/null || echo "(no screenshots found)"
echo ""
echo "Next: ./scripts/frame-screenshots.sh $PLATFORM"
