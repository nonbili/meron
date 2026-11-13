#!/bin/bash
# Present actual captures on store-sized canvases and stage upload images.
# Usage: ./scripts/frame-screenshots.sh android|ios|ipad
set -euo pipefail
MOBILE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PLATFORM="${1:-}"
if [ -z "$PLATFORM" ]; then
  if xcrun simctl list devices booted 2>/dev/null | grep -q "Booted"; then
    PLATFORM="ios"
  else
    PLATFORM="android"
  fi
fi
exec python3 "$MOBILE_DIR/scripts/frame-screenshots.py" "$PLATFORM"
