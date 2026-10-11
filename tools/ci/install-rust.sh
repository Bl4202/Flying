#!/usr/bin/env bash
# Installs the pinned Rust toolchain(s) named in rust-toolchain.toml and rust-nightly-version.
# Usage: tools/ci/install-rust.sh [--nightly]
set -euo pipefail
cd "$(dirname "$0")/../.."

channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
[ -n "$channel" ] || { echo "no channel in rust-toolchain.toml" >&2; exit 1; }
rustup toolchain install "$channel" --profile minimal -c rustfmt -c clippy -t wasm32-unknown-unknown

if [ "${1:-}" = "--nightly" ]; then
  nightly="$(tr -d '[:space:]' < rust-nightly-version)"
  rustup toolchain install "$nightly" --profile minimal -c rust-src -t wasm32-unknown-unknown
fi
rustc --version
