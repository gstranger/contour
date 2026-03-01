#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCKFILE="$ROOT/Cargo.lock"

if [[ ! -f "$LOCKFILE" ]]; then
  echo "error: Cargo.lock not found at $LOCKFILE" >&2
  exit 1
fi

REQUIRED_VERSION="$(
  awk '
    /^name = "wasm-bindgen"$/ { in_pkg = 1; next }
    in_pkg && /^version = / { gsub(/"/, "", $3); print $3; exit }
    in_pkg && /^\[/ { in_pkg = 0 }
  ' "$LOCKFILE"
)"

if [[ -z "$REQUIRED_VERSION" ]]; then
  echo "error: could not determine wasm-bindgen version from Cargo.lock" >&2
  exit 1
fi

INSTALLED_VERSION=""
if command -v wasm-bindgen >/dev/null 2>&1; then
  INSTALLED_VERSION="$(wasm-bindgen --version | awk '{ print $2 }')"
fi

if [[ "$INSTALLED_VERSION" == "$REQUIRED_VERSION" ]]; then
  echo "wasm-bindgen-cli v$REQUIRED_VERSION already installed."
  exit 0
fi

echo "Installing wasm-bindgen-cli v$REQUIRED_VERSION..."
cargo install --locked wasm-bindgen-cli --version "$REQUIRED_VERSION"
