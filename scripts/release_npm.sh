#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRATE_DIR="$ROOT/contour-wasm"
NPM_DIR="$ROOT/npm"
PKG_DIR="$NPM_DIR/pkg"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$CRATE_DIR/Cargo.toml" | head -n1)

if [[ -z "$VERSION" ]]; then
  echo "Failed to determine crate version" >&2
  exit 1
fi

echo "Building contour npm artifacts v$VERSION"

rm -rf "$PKG_DIR/default" "$PKG_DIR/simd" "$PKG_DIR/threads"
mkdir -p "$PKG_DIR/default" "$PKG_DIR/simd" "$PKG_DIR/threads"

update_js_import() {
  local file="$1"
  local wasm_name="$2"
  if [[ -f "$file" ]]; then
    perl -0pi -e 's/contour_bg\.wasm/'"$wasm_name"'/g' "$file"
  fi
}

build_variant() {
  local variant="$1"
  local cargo_features="$2"
  local rustflags="$3"
  local wasm_extra_flags="$4"
  local dest="$PKG_DIR/$variant"
  local tmp
  tmp=$(mktemp -d)

  if [[ -n "$rustflags" ]]; then
    export RUSTFLAGS="$rustflags"
    export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="$rustflags"
  else
    unset RUSTFLAGS
    unset CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS
  fi

  export WASM_BINDGEN_FLAGS="--keep-debug --generate-sourcemap $wasm_extra_flags"
  pushd "$CRATE_DIR" >/dev/null
    if [[ -n "$cargo_features" ]]; then
      wasm-pack build --release --target web --out-dir "$tmp" --out-name contour -- --features "$cargo_features"
    else
      wasm-pack build --release --target web --out-dir "$tmp" --out-name contour
    fi
  popd >/dev/null
  unset WASM_BINDGEN_FLAGS

  mkdir -p "$dest"
  local wasm_suffix=""
  if [[ "$variant" != "default" ]]; then
    wasm_suffix="_${variant}"
  fi
  local wasm_name="contour_bg_v${VERSION}${wasm_suffix}.wasm"

  mv "$tmp/contour.js" "$dest/contour.js"
  mv "$tmp/contour.d.ts" "$dest/contour.d.ts"
  mv "$tmp/contour_bg.wasm" "$dest/${wasm_name}"
  if [[ -f "$tmp/contour_bg.wasm.map" ]]; then
    mv "$tmp/contour_bg.wasm.map" "$dest/${wasm_name}.map"
  fi
  if [[ -f "$tmp/contour_bg.wasm.d.ts" ]]; then
    mv "$tmp/contour_bg.wasm.d.ts" "$dest/contour_bg_v${VERSION}${wasm_suffix}.wasm.d.ts"
  fi
  if [[ -d "$tmp/snippets" ]]; then
    rm -rf "$dest/snippets"
    mv "$tmp/snippets" "$dest/snippets"
  fi
  if [[ -f "$tmp/package.json" ]]; then
    mv "$tmp/package.json" "$dest/wasm-pack-package.json"
  fi

  update_js_import "$dest/contour.js" "$wasm_name"
  rm -rf "$tmp"
}

build_variant "default" "" "" ""
build_variant "simd" "simd" "-C target-feature=+simd128" ""
build_variant "threads" "threads" "-C target-feature=+atomics,+bulk-memory,+mutable-globals" "--enable-threading"

node <<'NODE' "$NPM_DIR/package.json" "$VERSION"
const fs = require('fs');
const pkgPath = process.argv[1];
const version = process.argv[2];
const data = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
data.version = version;
fs.writeFileSync(pkgPath, JSON.stringify(data, null, 2) + '\n');
NODE

cat <<'EODEFAULT' > "$PKG_DIR/index.js"
export * from "./default/contour.js";
export { default } from "./default/contour.js";
EODEFAULT

cat <<'EODEFAULTTYPES' > "$PKG_DIR/index.d.ts"
export * from "./default/contour";
export { default } from "./default/contour";
EODEFAULTTYPES

cat <<'EOSIMD' > "$PKG_DIR/simd/index.js"
export * from "./contour.js";
export { default } from "./contour.js";
EOSIMD

cat <<'EOSIMDTYPES' > "$PKG_DIR/simd/index.d.ts"
export * from "./contour";
export { default } from "./contour";
EOSIMDTYPES

cat <<'EOTHREADS' > "$PKG_DIR/threads/index.js"
export * from "./contour.js";
export { default } from "./contour.js";
EOTHREADS

cat <<'EOTHREADSTYPES' > "$PKG_DIR/threads/index.d.ts"
export * from "./contour";
export { default } from "./contour";
EOTHREADSTYPES

cat <<EOF
Artifacts written to $PKG_DIR.
Run the smoke test with:
  (cd $ROOT/examples/npm-smoke && npm install && npm test)
Publish with:
  (cd $NPM_DIR && npm publish)
EOF
