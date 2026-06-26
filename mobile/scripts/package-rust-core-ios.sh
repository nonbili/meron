#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-apple-ios-sim}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$MOBILE_DIR/.." && pwd)"
CORE_DIR="$REPO_DIR/meron-core"
OUT_DIR="$MOBILE_DIR/ios/RustCore"

if ! command -v rustup >/dev/null 2>&1 && [[ -z "${IN_NIX_SHELL:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  exec nix-shell "$REPO_DIR/shell.nix" --run "$SCRIPT_DIR/$(basename "$0")"
fi

if [[ -z "${DEVELOPER_DIR:-}" || "${DEVELOPER_DIR:-}" == /nix/store/* ]]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
fi
export PATH="/usr/bin:/bin:$PATH"

SDK_NAME="iphonesimulator"
MIN_VER_FLAG="-mios-simulator-version-min=17.0"
if [[ "$TARGET" == "aarch64-apple-ios" ]]; then
  SDK_NAME="iphoneos"
  MIN_VER_FLAG="-miphoneos-version-min=17.0"
fi

IOS_SDKROOT="$(DEVELOPER_DIR="$DEVELOPER_DIR" /usr/bin/xcrun --sdk "$SDK_NAME" --show-sdk-path)"
IOS_CLANG="$(DEVELOPER_DIR="$DEVELOPER_DIR" /usr/bin/xcrun --sdk "$SDK_NAME" -find clang)"
export SDKROOT="$IOS_SDKROOT"
export "CC_${TARGET//-/_}=$IOS_CLANG"
export "CFLAGS_${TARGET//-/_}=-isysroot $IOS_SDKROOT $MIN_VER_FLAG"

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET" >/dev/null
fi

echo "Building meron-core for $TARGET"
cargo build --manifest-path "$CORE_DIR/Cargo.toml" --lib --target "$TARGET"

LIB_PATH="$CORE_DIR/target/$TARGET/debug/libmeron_core.a"
if [[ ! -f "$LIB_PATH" ]]; then
  echo "Build completed, but $LIB_PATH was not produced." >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
cp "$LIB_PATH" "$OUT_DIR/libmeron_core.a"
echo "Packaged $OUT_DIR/libmeron_core.a"
