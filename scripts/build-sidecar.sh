#!/usr/bin/env bash
# Builds the qiyas-mcp bridge in release and puts it where Tauri's bundler looks for it:
# crates/qiyas-desktop/binaries/qiyas-mcp-<target triple> (git-ignored). Run it before bundling:
#   scripts/build-sidecar.sh
#   pnpm tauri build --config crates/qiyas-desktop/tauri.bundle.conf.json
# Pass a target triple to build for another target.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ $# -gt 0 ]]; then
  triple="$1"
  cargo build --release -p qiyas-mcp --target "$triple"
  out="target/$triple/release"
else
  triple="$(rustc -vV | sed -n 's/^host: //p')"
  cargo build --release -p qiyas-mcp
  out="target/release"
fi

ext=""
if [[ "$triple" == *windows* ]]; then ext=".exe"; fi
mkdir -p crates/qiyas-desktop/binaries
cp "$out/qiyas-mcp$ext" "crates/qiyas-desktop/binaries/qiyas-mcp-$triple$ext"
echo "crates/qiyas-desktop/binaries/qiyas-mcp-$triple$ext"
