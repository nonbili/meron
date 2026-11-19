#!/usr/bin/env bash
# Build a release Meron binary with the Rust core engine sidecar baked in (on
# macOS: placed inside the app bundle).
#
# Plain `bun run build` resolves the sidecar via a working-directory-relative
# path, so the resulting binary only works when launched from the repo root.
# This script builds the sidecar in release mode, stages it where the
# `embed_sidecar` build tag expects it, and embeds it into the Go binary so the
# app works regardless of where it is launched from.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Building Rust core engine sidecar (release)"
cargo build --release --manifest-path meron-core/Cargo.toml

if [ "$(uname)" = Darwin ]; then
  # macOS ships the core inside the bundle, where the bundle's signature
  # covers it (see the release workflow); embedding is for Linux and Windows.
  echo "==> Building Wails app"
  bun scripts/build-desktop.ts "$@"
  cp meron-core/target/release/meron-core desktop/build/bin/meron.app/Contents/MacOS/meron-core
  echo "==> Done: desktop/build/bin/meron.app (core inside, unsigned)"
  exit 0
fi

echo "==> Staging sidecar for embedding"
mkdir -p desktop/build/sidecar
cp meron-core/target/release/meron-core desktop/build/sidecar/meron-core

echo "==> Building Wails app with embedded sidecar"
bun scripts/build-desktop.ts -tags embed_sidecar "$@"

echo "==> Done: desktop/build/bin/meron"
