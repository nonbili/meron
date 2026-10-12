#!/usr/bin/env bash
set -euo pipefail
test_dir=$(cd "$(dirname "$0")" && pwd)
test_build=$(mktemp -d)
trap 'rm -rf "$test_build"' EXIT
xcrun clang -fobjc-arc -framework Cocoa \
  "$test_dir/tray_badge_darwin.m" -o "$test_build/tray-badge-test"
"$test_build/tray-badge-test" "$test_dir/../../../build/trayicon-mono.png"
