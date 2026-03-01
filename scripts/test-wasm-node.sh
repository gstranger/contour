#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT/scripts/ensure-wasm-bindgen.sh"

cd "$ROOT/contour-wasm"
wasm-pack test --node "$@"
