#!/usr/bin/env bash
# Installs pinned build tools into ~/.local/bin on Linux x86_64 (CI), verifying SHA-256.
# Usage: tools/ci/install-tools.sh [binaryen] [cargo-deny] [naga]
set -euo pipefail

BINARYEN_VERSION=version_133
BINARYEN_SHA256=2dc9c7813f5375db93d96ead4b78222fcc3e2677bbb832297af4797782a37489
CARGO_DENY_VERSION=0.20.2
CARGO_DENY_SHA256=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f
NAGA_CLI_VERSION=30.0.1

bin="$HOME/.local/bin"
mkdir -p "$bin"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

verify() { echo "$1  $2" | sha256sum -c -; }

for tool in "$@"; do
  case "$tool" in
    binaryen)
      f="binaryen-${BINARYEN_VERSION}-x86_64-linux.tar.gz"
      curl -fsSL -o "$work/$f" "https://github.com/WebAssembly/binaryen/releases/download/${BINARYEN_VERSION}/$f"
      verify "$BINARYEN_SHA256" "$work/$f"
      mkdir -p "$HOME/.local/share/binaryen"
      tar -xzf "$work/$f" -C "$HOME/.local/share/binaryen" --strip-components=1
      ln -sf "$HOME/.local/share/binaryen/bin/wasm-opt" "$bin/wasm-opt"
      wasm-opt --version
      ;;
    cargo-deny)
      f="cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz"
      curl -fsSL -o "$work/$f" "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/$f"
      verify "$CARGO_DENY_SHA256" "$work/$f"
      tar -xzf "$work/$f" -C "$work" --strip-components=1
      install -m 0755 "$work/cargo-deny" "$bin/cargo-deny"
      cargo-deny --version
      ;;
    naga)
      if ! "$bin/naga" --version 2>/dev/null | grep -q "$NAGA_CLI_VERSION"; then
        cargo install naga-cli --locked --version "$NAGA_CLI_VERSION" --root "$HOME/.local"
      fi
      "$bin/naga" --version
      ;;
    *) echo "unknown tool $tool" >&2; exit 2 ;;
  esac
done
