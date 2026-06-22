#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-linux-android}"
ABI="${ABI:-arm64-v8a}"
API_LEVEL="${API_LEVEL:-23}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$MOBILE_DIR/.." && pwd)"
CORE_DIR="$REPO_DIR/meron-core"
JNI_LIB_DIR="$MOBILE_DIR/android/src/main/jniLibs/$ABI"

if ! command -v rustup >/dev/null 2>&1 && [[ -z "${IN_NIX_SHELL:-}" ]] && command -v nix-shell >/dev/null 2>&1; then
  exec nix-shell "$REPO_DIR/shell.nix" --run "$SCRIPT_DIR/$(basename "$0")"
fi

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  for candidate in \
    "$HOME/Library/Android/sdk/ndk/29.0.14206865" \
    "$HOME/Library/Android/sdk/ndk/28.1.13356709" \
    "$HOME/Library/Android/sdk/ndk/27.1.12297006" \
    "$HOME/Library/Android/sdk/ndk/27.0.12077973" \
    "$HOME/Library/Android/sdk/ndk/26.3.11579264"; do
    if [[ -d "$candidate" ]]; then
      export ANDROID_NDK_HOME="$candidate"
      break
    fi
  done
fi

if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
  echo "ANDROID_NDK_HOME is not set and no Android NDK was found under ~/Library/Android/sdk/ndk." >&2
  exit 1
fi

HOST_TAG="darwin-x86_64"
if [[ "$(uname -s)" == "Linux" ]]; then
  HOST_TAG="linux-x86_64"
fi

TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG"
CLANG="$TOOLCHAIN/bin/aarch64-linux-android${API_LEVEL}-clang"
AR="$TOOLCHAIN/bin/llvm-ar"

if [[ ! -x "$CLANG" ]]; then
  echo "Missing Android clang at $CLANG" >&2
  exit 1
fi

if [[ ! -x "$AR" ]]; then
  echo "Missing llvm-ar at $AR" >&2
  exit 1
fi

export "CC_${TARGET//-/_}=$CLANG"
export "AR_${TARGET//-/_}=$AR"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="$AR"

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET" >/dev/null
fi

echo "Building meron-core for $TARGET using $ANDROID_NDK_HOME"
if ! cargo build --manifest-path "$CORE_DIR/Cargo.toml" --lib --target "$TARGET"; then
  cat >&2 <<EOF

Failed to build meron-core for $TARGET.

This usually means the active Rust toolchain does not include the Android
standard library for $TARGET. Install a Rust toolchain with that target, for
example:

  rustup target add $TARGET

The current Nix-provided Rust toolchain in this workspace does not include
rustup or the $TARGET stdlib by default.
EOF
  exit 1
fi

SO_PATH="$CORE_DIR/target/$TARGET/debug/libmeron_core.so"
if [[ ! -f "$SO_PATH" ]]; then
  echo "Build completed, but $SO_PATH was not produced." >&2
  exit 1
fi

mkdir -p "$JNI_LIB_DIR"
cp "$SO_PATH" "$JNI_LIB_DIR/libmeron_core.so"
echo "Packaged $JNI_LIB_DIR/libmeron_core.so"
