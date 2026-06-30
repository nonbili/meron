#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-linux-android}"
ABI="${ABI:-arm64-v8a}"
API_LEVEL="${API_LEVEL:-23}"
# PROFILE selects the Cargo profile. The default `debug` keeps local dev builds
# fast; release builds optimize and get stripped (the debug .so ships ~186MB of
# unstripped debug_info, which is why release APKs must use PROFILE=release).
PROFILE="${PROFILE:-debug}"
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

RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
STRIP="$TOOLCHAIN/bin/llvm-strip"

# The vendored OpenSSL build (openssl-src) drives OpenSSL's own Makefile, which
# otherwise picks up the host macOS `ar` and emits BSD-format archives. rustc,
# bundling these Android (ELF) static libs, expects GNU-format archives and
# misreads a BSD long-name table as a member name ("invalid utf-8 sequence").
# The NDK's llvm-ar already writes GNU-format archives, so force AR/RANLIB (in
# both the unscoped and per-target forms the cc/openssl-src crates consult) to
# the NDK tools. CC is set the same way so OpenSSL cross-compiles for Android.
export "CC_${TARGET//-/_}=$CLANG"
export "AR_${TARGET//-/_}=$AR"
export "RANLIB_${TARGET//-/_}=$RANLIB"
# Unscoped AR/RANLIB are what OpenSSL's own Makefile (driven by openssl-src)
# reads; without these it falls back to the host macOS `ar` (BSD format).
export AR="$AR"
export RANLIB="$RANLIB"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="$AR"

if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET" >/dev/null
fi

CARGO_PROFILE_ARGS=()
if [[ "$PROFILE" == "release" ]]; then
  CARGO_PROFILE_ARGS=(--release)
elif [[ "$PROFILE" != "debug" ]]; then
  echo "Unsupported PROFILE='$PROFILE' (expected 'debug' or 'release')." >&2
  exit 1
fi

echo "Building meron-core ($PROFILE) for $TARGET using $ANDROID_NDK_HOME"
if ! cargo build --manifest-path "$CORE_DIR/Cargo.toml" --lib --target "$TARGET" "${CARGO_PROFILE_ARGS[@]}"; then
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

SO_PATH="$CORE_DIR/target/$TARGET/$PROFILE/libmeron_core.so"
if [[ ! -f "$SO_PATH" ]]; then
  echo "Build completed, but $SO_PATH was not produced." >&2
  exit 1
fi

mkdir -p "$JNI_LIB_DIR"
cp "$SO_PATH" "$JNI_LIB_DIR/libmeron_core.so"

# Release builds carry full debug_info (~186MB). Strip it so the packaged .so —
# and the APK that bundles it — stays small.
if [[ "$PROFILE" == "release" ]]; then
  "$STRIP" --strip-unneeded "$JNI_LIB_DIR/libmeron_core.so"
fi
echo "Packaged $JNI_LIB_DIR/libmeron_core.so"
