#!/usr/bin/env bash
set -euo pipefail

# Space-separated Rust target triples to build. Override TARGETS (or TARGET,
# for a single triple) to restrict the build.
TARGETS="${TARGETS:-${TARGET:-aarch64-linux-android armv7-linux-androideabi}}"
API_LEVEL="${API_LEVEL:-23}"

abi_for_target() {
  case "$1" in
    aarch64-linux-android) echo "arm64-v8a" ;;
    armv7-linux-androideabi) echo "armeabi-v7a" ;;
    x86_64-linux-android) echo "x86_64" ;;
    i686-linux-android) echo "x86" ;;
    *)
      echo "Unknown Android ABI for Rust target '$1'." >&2
      exit 1
      ;;
  esac
}

# The NDK clang wrapper for 32-bit ARM is named armv7a-linux-androideabi*, not
# armv7-linux-androideabi* like the Rust triple.
clang_prefix_for_target() {
  case "$1" in
    armv7-linux-androideabi) echo "armv7a-linux-androideabi" ;;
    *) echo "$1" ;;
  esac
}
# PROFILE selects the Cargo profile. The default `debug` keeps local dev builds
# fast; release builds optimize and get stripped (the debug .so ships ~186MB of
# unstripped debug_info, which is why release APKs must use PROFILE=release).
PROFILE="${PROFILE:-debug}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$MOBILE_DIR/.." && pwd)"
CORE_DIR="$REPO_DIR/meron-core"
TARGET_DIR="${CARGO_TARGET_DIR:-$CORE_DIR/target}"

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
AR="$TOOLCHAIN/bin/llvm-ar"

if [[ ! -x "$AR" ]]; then
  echo "Missing llvm-ar at $AR" >&2
  exit 1
fi

RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
STRIP="$TOOLCHAIN/bin/llvm-strip"

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]] && git -C "$REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  export SOURCE_DATE_EPOCH="$(git -C "$REPO_DIR" log -1 --format=%ct)"
fi

append_rustflag() {
  case " ${RUSTFLAGS:-} " in
    *" $1 "*) ;;
    *) export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }$1" ;;
  esac
}

CARGO_HOME_PATH="${CARGO_HOME:-${HOME:-}/.cargo}"
if [[ -n "$CARGO_HOME_PATH" ]]; then
  mkdir -p "$CARGO_HOME_PATH"
  CARGO_HOME_PATH="$(cd "$CARGO_HOME_PATH" && pwd)"
  append_rustflag "--remap-path-prefix=$CARGO_HOME_PATH=/cargo"
fi
append_rustflag "--remap-path-prefix=$REPO_DIR=/build/meron"

# The vendored OpenSSL build (openssl-src) drives OpenSSL's own Makefile, which
# otherwise picks up the host macOS `ar` and emits BSD-format archives. rustc,
# bundling these Android (ELF) static libs, expects GNU-format archives and
# misreads a BSD long-name table as a member name ("invalid utf-8 sequence").
# The NDK's llvm-ar already writes GNU-format archives, so force AR/RANLIB (in
# both the unscoped and per-target forms the cc/openssl-src crates consult) to
# the NDK tools. CC is set the same way so OpenSSL cross-compiles for Android.
# Unscoped AR/RANLIB are what OpenSSL's own Makefile (driven by openssl-src)
# reads; without these it falls back to the host macOS `ar` (BSD format).
export AR="$AR"
export RANLIB="$RANLIB"

CARGO_PROFILE_ARGS=()
if [[ "$PROFILE" == "release" ]]; then
  CARGO_PROFILE_ARGS=(--release)
elif [[ "$PROFILE" != "debug" ]]; then
  echo "Unsupported PROFILE='$PROFILE' (expected 'debug' or 'release')." >&2
  exit 1
fi

build_target() {
  local target="$1"
  local abi clang jni_lib_dir target_uc
  abi="$(abi_for_target "$target")"
  clang="$TOOLCHAIN/bin/$(clang_prefix_for_target "$target")${API_LEVEL}-clang"
  jni_lib_dir="$MOBILE_DIR/android/src/main/jniLibs/$abi"
  target_uc="$(echo "${target//-/_}" | tr '[:lower:]' '[:upper:]')"

  if [[ ! -x "$clang" ]]; then
    echo "Missing Android clang at $clang" >&2
    exit 1
  fi

  export "CC_${target//-/_}=$clang"
  export "AR_${target//-/_}=$AR"
  export "RANLIB_${target//-/_}=$RANLIB"
  export "CARGO_TARGET_${target_uc}_LINKER=$clang"
  export "CARGO_TARGET_${target_uc}_AR=$AR"

  if command -v rustup >/dev/null 2>&1; then
    rustup target add "$target" >/dev/null
  fi

  echo "Building meron-core ($PROFILE) for $target using $ANDROID_NDK_HOME"
  if ! cargo build --manifest-path "$CORE_DIR/Cargo.toml" --lib --target "$target" "${CARGO_PROFILE_ARGS[@]}"; then
    cat >&2 <<EOF

Failed to build meron-core for $target.

This usually means the active Rust toolchain does not include the Android
standard library for $target. Install a Rust toolchain with that target, for
example:

  rustup target add $target

The current Nix-provided Rust toolchain in this workspace does not include
rustup or the $target stdlib by default.
EOF
    exit 1
  fi

  local so_path="$TARGET_DIR/$target/$PROFILE/libmeron_core.so"
  if [[ ! -f "$so_path" ]]; then
    echo "Build completed, but $so_path was not produced." >&2
    exit 1
  fi

  mkdir -p "$jni_lib_dir"
  cp "$so_path" "$jni_lib_dir/libmeron_core.so"

  # Release builds carry full debug_info (~186MB). Strip it so the packaged
  # .so — and the APK that bundles it — stays small.
  if [[ "$PROFILE" == "release" ]]; then
    "$STRIP" --strip-unneeded "$jni_lib_dir/libmeron_core.so"
  fi
  echo "Packaged $jni_lib_dir/libmeron_core.so"
}

for target in $TARGETS; do
  build_target "$target"
done
