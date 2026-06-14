#!/usr/bin/env bash
# Build a release Meron binary with the Rust core engine sidecar baked in.
#
# Plain `wails build` resolves the sidecar via a working-directory-relative
# path, so the resulting binary only works when launched from the repo root.
# This script builds the sidecar in release mode, stages it where the
# `embed_sidecar` build tag expects it, and embeds it into the Go binary so the
# app works regardless of where it is launched from.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Building Rust core engine sidecar (release)"
cargo build --release --manifest-path meron-core/Cargo.toml

echo "==> Staging sidecar for embedding"
mkdir -p build/sidecar
cp meron-core/target/release/meron-core build/sidecar/meron-core

echo "==> Building Wails app with embedded sidecar"
wails build -tags embed_sidecar "$@"

echo "==> Done: build/bin/meron"
