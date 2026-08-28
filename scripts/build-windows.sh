#!/usr/bin/env bash
# Cross-build a Windows Meron from Linux, for testing the Windows startup path
# without waiting on CI.
#
# The sidecar targets x86_64-pc-windows-gnu, not the MSVC target CI ships: the
# vendored OpenSSL that rusqlite's bundled-sqlcipher-vendored-openssl pulls in
# needs nmake for an MSVC build, which does not exist here. mingw builds it with
# plain make instead. A gnu-target sidecar is fine for reproducing bugs, but it
# is not the artifact to release.
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-gnu
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
# Static libgcc/libwinpthread so the sidecar does not need the mingw runtime
# DLLs beside it on the test machine.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-args=-static"

echo "==> Building Rust core engine sidecar ($TARGET)"
cargo build --release --manifest-path meron-core/Cargo.toml --target "$TARGET"

echo "==> Staging sidecar for embedding"
mkdir -p desktop/build/sidecar
# go:embed expects desktop/build/sidecar/meron-core with no extension on every platform;
# the app appends .exe when it extracts on Windows.
cp "meron-core/target/$TARGET/release/meron-core.exe" desktop/build/sidecar/meron-core

echo "==> Building Wails app for windows/amd64"
(
  cd desktop
  CGO_ENABLED=0 wails build -clean -platform windows/amd64 -tags embed_sidecar
)

echo "==> Done: desktop/build/bin/meron.exe"
