#!/usr/bin/env bash
# Verify that the built WASM bundle stays within a gzipped size budget.
#
# Budgets gate regressions against the current measured size with modest slack;
# they are NOT the v1 aspirational target (300 KB wasm / 30 KB JS gz). The
# aspirational target is tracked separately under issue "[v1] Enforce size
# budgets" and should be reached by code-size reductions, not by relaxing the
# gate.
#
# Override via environment when tuning:
#   WASM_BUDGET_BYTES   gzipped .wasm budget (default 320000)
#   JS_BUDGET_BYTES     gzipped JS-glue budget (default 16000)
#   PKG_DIR             directory containing wasm-pack output (default ./pkg)
set -euo pipefail

PKG_DIR="${PKG_DIR:-pkg}"
WASM_BUDGET_BYTES="${WASM_BUDGET_BYTES:-320000}"
JS_BUDGET_BYTES="${JS_BUDGET_BYTES:-16000}"

if [ ! -d "$PKG_DIR" ]; then
  echo "size-check: $PKG_DIR not found; run wasm-pack build first" >&2
  exit 2
fi

wasm_file=$(ls "$PKG_DIR"/*_bg.wasm 2>/dev/null | head -1)
js_file=$(ls "$PKG_DIR"/*.js 2>/dev/null | grep -v '\.d\.ts$' | head -1)

if [ -z "$wasm_file" ] || [ -z "$js_file" ]; then
  echo "size-check: missing .wasm or .js in $PKG_DIR" >&2
  ls "$PKG_DIR" >&2
  exit 2
fi

wasm_gz=$(gzip -c -9 "$wasm_file" | wc -c | tr -d ' ')
js_gz=$(gzip -c -9 "$js_file" | wc -c | tr -d ' ')

printf '%-12s  %s  gz=%d  budget=%d\n' wasm "$wasm_file" "$wasm_gz" "$WASM_BUDGET_BYTES"
printf '%-12s  %s  gz=%d  budget=%d\n' js   "$js_file"   "$js_gz"   "$JS_BUDGET_BYTES"

fail=0
if [ "$wasm_gz" -gt "$WASM_BUDGET_BYTES" ]; then
  over=$((wasm_gz - WASM_BUDGET_BYTES))
  echo "FAIL: wasm gz=$wasm_gz exceeds budget by $over bytes" >&2
  fail=1
fi
if [ "$js_gz" -gt "$JS_BUDGET_BYTES" ]; then
  over=$((js_gz - JS_BUDGET_BYTES))
  echo "FAIL: js gz=$js_gz exceeds budget by $over bytes" >&2
  fail=1
fi

exit "$fail"
