#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$MOBILE_DIR/.." && pwd)"

RUN_DESKTOP=1
RUN_WAILS=0
RUN_ANDROID_CONNECTED=1
RUN_IOS=1

usage() {
  cat <<EOF
Usage: mobile/scripts/verify-local.sh [options]

Runs Meron's repeatable local verification gates for the native mobile plan.
Provider-console Gmail/Outlook registration and live provider OAuth acceptance
are intentionally not covered by this script.

Options:
  --skip-desktop             Skip cargo and bun desktop/core regression tests.
  --wails                    Also run a clean Wails production build.
  --skip-android-connected   Skip Android emulator instrumentation tests.
  --skip-ios                 Skip iOS simulator XCTest.
  -h, --help                 Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-desktop)
      RUN_DESKTOP=0
      ;;
    --wails)
      RUN_WAILS=1
      ;;
    --skip-android-connected)
      RUN_ANDROID_CONNECTED=0
      ;;
    --skip-ios)
      RUN_IOS=0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

run() {
  echo
  echo "==> $*"
  "$@"
}

has_android_device() {
  command -v adb >/dev/null 2>&1 && adb devices | awk 'NR > 1 && $2 == "device" { found = 1 } END { exit found ? 0 : 1 }'
}

if [[ "$RUN_DESKTOP" -eq 1 ]]; then
  run cargo test --manifest-path "$REPO_DIR/meron-core/Cargo.toml"
  run bun run test
  if [[ "$RUN_WAILS" -eq 1 ]]; then
    run wails build -clean
  fi
fi

run "$MOBILE_DIR/scripts/verify-oauth-association.sh"
run gradle -p "$MOBILE_DIR" \
  :shared:check \
  :ui:compileAndroidMain \
  :ui:compileKotlinIosArm64 \
  :ui:compileKotlinIosSimulatorArm64 \
  :ui:linkDebugFrameworkIosSimulatorArm64 \
  :android:assembleDebug \
  :android:assembleAndroidTest

if [[ "$RUN_ANDROID_CONNECTED" -eq 1 ]]; then
  if has_android_device; then
    run gradle -p "$MOBILE_DIR" :android:connectedDebugAndroidTest
  else
    echo
    echo "==> Skipping Android connected tests: no adb device is connected."
  fi
fi

if [[ "$RUN_IOS" -eq 1 ]]; then
  if command -v xcodebuild >/dev/null 2>&1; then
    run xcodebuild test -quiet \
      -project "$MOBILE_DIR/ios/Meron.xcodeproj" \
      -scheme Meron \
      -sdk iphonesimulator \
      -destination "platform=iOS Simulator,name=iPhone 17" \
      -derivedDataPath /tmp/meron-xcode-derived-local-verify \
      CODE_SIGNING_ALLOWED=NO
  else
    echo
    echo "==> Skipping iOS tests: xcodebuild is not on PATH."
  fi
fi

echo
echo "Local verification complete."
