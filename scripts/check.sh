#!/usr/bin/env bash
# Local checks. Run before every commit. Stops at the first failure.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Release-only speed guard for the pick index (codegen-units = 1 in Cargo.toml).
cargo test -p qiyas-render --release pick_index_one_million
pnpm typecheck
pnpm test
