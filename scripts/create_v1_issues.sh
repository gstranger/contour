#!/usr/bin/env bash
set -euo pipefail

# Requires: GitHub CLI (gh) authenticated, jq installed, and milestone/labels created.
# Run scripts/gh_bootstrap.sh once before this to create labels and the v1.0 milestone.

MILESTONE="v1.0"

create_issue() {
  local title="$1"; shift
  local labels="$1"; shift
  local body="$1"; shift
  # shellcheck disable=SC2086
  gh issue create \
    --title "$title" \
    --milestone "$MILESTONE" \
    $(printf -- "--label %q " ${labels//,/ }) \
    --body "$body"
}

# 1. Core Robustness
create_issue "[v1] Degenerate geometry handling" "v1,area:core,type:quality,priority:P0" "Goal\n- Normalize epsilons; clamp t∈[0,1]; handle zero-length edges, coincident endpoints, tiny faces; edits become no-ops, not panics.\n\nAcceptance Criteria\n- No panics across 10k property/fuzz cases.\n- Consistent epsilon policy in one module.\n- All edit paths gracefully handle degenerates.\n\nTasks\n- Centralize tolerances; add clamps/guards.\n- Add property tests for zero-length and coincident cases.\n- Audit bend, move_node, set_handle_pos, convert line→cubic." 

create_issue "[v1] Stable region keys + remap" "v1,area:core,type:quality,priority:P0" "Goal\n- Deterministic face keys that persist under bends; nearest-centroid remap on topology change.\n\nAcceptance Criteria\n- Determinism test stable across Chrome/Firefox/WebKitGTK.\n- ≥99.5% fill persistence over 1k random edits in synthetic scenes.\n\nTasks\n- Canonicalize edge-loop sequences and hash.\n- Implement remap by centroid with tie-breaking.\n- Add determinism + persistence tests." 

create_issue "[v1] Robust planarization backbone" "v1,area:core,type:quality,priority:P1" "Goal\n- Flattened-segment graph with robust predicates; face-walk via angle sort; document unsupported self-intersections if any.\n\nAcceptance Criteria\n- Self-touching within tolerance does not crash; faces are consistent.\n- Quantization produces stable vertices without excessive merge.\n\nTasks\n- Implement or adopt segment intersection with sweep-line or robust tests.\n- Build half-edges; sort outgoing by angle; traverse faces.\n- Property tests for half-edge pairing and closure." 

create_issue "[v1] Handle modes invariants" "v1,area:core,type:quality,priority:P1" "Goal\n- Enforce Free/Mirrored/Aligned algebra for all edits (bend, set_handle_pos, move_node, convert).\n\nAcceptance Criteria\n- Mirrored lengths remain equal; Aligned directions opposite; no jumps.\n\nTasks\n- Centralize handle constraint application.\n- Apply constraints post-bend LS solution.\n- Add invariants to property tests." 

# 2. Performance & Memory
create_issue "[v1] Spatial index for picking" "v1,area:perf,type:perf,priority:P1" "Goal\n- O(log n) picking with grid or R-tree; incremental updates keyed by geom_ver.\n\nAcceptance Criteria\n- Median pick latency <0.3 ms at 5k edges.\n\nTasks\n- Implement grid/R-tree for nodes, edges, handles.\n- Update index on geometry edits.\n- Bench and integrate into picking." 

create_issue "[v1] Incremental region recompute" "v1,area:perf,type:perf,priority:P0" "Goal\n- Recompute only neighborhoods affected by edited nodes/edges; cache results.\n\nAcceptance Criteria\n- Typical edit region pass <2 ms; worst-case full recompute <8 ms @5k edges.\n\nTasks\n- Track dirty sets via geom diffs.\n- Local face rebuild; invalidate caches.\n- Bench on synthetic meshes." 

create_issue "[v1] Worker offload for flattening/regions" "v1,area:perf,area:tooling,type:perf,priority:P1" "Goal\n- Move flattening + face extraction to Web Worker (WASM); support cancel/coalesce.\n\nAcceptance Criteria\n- Main thread stays <4 ms/frame during drag @5k edges.\n\nTasks\n- Worker wrapper + message protocol.\n- Job coalescing and cancellation.\n- Swap results safely by geom_ver." 

create_issue "[v1] Adaptive flattening + cache" "v1,area:perf,type:perf,priority:P2" "Goal\n- Zoom-aware tolerance; per-edge flatten cache keyed by (edge_id, geom_ver, tolerance bucket).\n\nAcceptance Criteria\n- Tessellation count scales with zoom; no popping.\n\nTasks\n- Implement adaptive tolerance policy.\n- Cache invalidation by geom_ver and tol.\n- Visual tests for stability." 

# 3. WASM API Hardening
create_issue "[v1] API input validation + error enums" "v1,area:wasm,type:quality,priority:P0" "Goal\n- Never panic across the WASM boundary; return typed errors for invalid ids, NaNs, out-of-range t, bad modes.\n\nAcceptance Criteria\n- Fuzz shows zero aborts; errors are typed and documented.\n\nTasks\n- Audit API entry points.\n- Define error enums + JS mapping.\n- Update docs with invariants." 

create_issue "[v1] Typed array lifetime + buffer reuse" "v1,area:wasm,type:perf,priority:P1" "Goal\n- Zero-copy views with documented lifetime; avoid per-call allocation in hot paths.\n\nAcceptance Criteria\n- No per-call allocs in hot paths; leak checks clean.\n\nTasks\n- Pool buffers; expose stable views.\n- Document ownership and mutation rules.\n- Bench interop cost." 

create_issue "[v1] Dirty queries: get_dirty(since)" "v1,area:wasm,type:feature,priority:P1" "Goal\n- Efficient UI sync via dirty diffs for nodes/edges/regions since a version.\n\nAcceptance Criteria\n- UI can refresh without full scans; deltas match edits.\n\nTasks\n- Track dirty sets per kind.\n- Implement query and tests.\n- Integrate in demo UI." 

create_issue "[v1] API versioning + JSON migration" "v1,area:wasm,type:quality,priority:P1" "Goal\n- Freeze v1 API; version JSON schema; provide migration from prior versions.\n\nAcceptance Criteria\n- from_json(v0) migrates; round-trip stable.\n\nTasks\n- Add schema version field.\n- Implement migrators + tests.\n- Document semver policy." 

# 4. Rendering & Interaction
create_issue "[v1] Deterministic rendering + DPR normalization" "v1,area:rendering,type:quality,priority:P1" "Goal\n- Stable face/stroke order, pixel alignment, device-pixel-ratio aware drawing.\n\nAcceptance Criteria\n- Screenshot diffs deterministic across browsers/OS.\n\nTasks\n- Normalize canvas state and z-order.\n- Add screenshot test harness.\n- Document rendering guarantees." 

create_issue "[v1] Selection visuals (halo + 2px editing)" "v1,area:rendering,type:feature,priority:P2" "Goal\n- Halo underlay and keep strokes at 2 px while editing to avoid fill overlap artifacts.\n\nAcceptance Criteria\n- Visual clarity during edit; no stroke/fill clash.\n\nTasks\n- Implement halo layer.\n- Editing stroke scaling.\n- Update demo UI toggles." 

create_issue "[v1] Bend preview overlay during drag" "v1,area:rendering,type:feature,priority:P2" "Goal\n- Draw transient curve overlay during bend; swap in recomputed faces next frame.\n\nAcceptance Criteria\n- No visible face flicker during drag.\n\nTasks\n- Overlay path rendering.\n- rAF swap integration.\n- Demo UX polish." 

create_issue "[v1] Input scaling (zoom-aware pick, touch/HIDPI)" "v1,area:rendering,type:feature,priority:P1" "Goal\n- Zoom-aware pick tolerance; unify world/screen coords; support touch events and high-DPI.\n\nAcceptance Criteria\n- Finger picks match cursor picks within 2 px.\n\nTasks\n- Scale tolerances by zoom/DPR.\n- Add touch handlers.\n- Test on HIDPI devices." 

# 5. SVG & I/O
create_issue "[v1] SVG import coverage incl. arcs (A)" "v1,area:svg,type:feature,priority:P1" "Goal\n- Harden M/L/C/Z and add A (arc) via cubic approximation; support multiple subpaths and rel/abs mixing.\n\nAcceptance Criteria\n- W3C SVG test snippets import without crash; geometry matches within tolerance.\n\nTasks\n- Implement arc-to-cubic.\n- Parser robustness + tests.\n- Edge cases: closepath, subpaths." 

create_issue "[v1] Endpoint merging tolerance" "v1,area:svg,type:quality,priority:P1" "Goal\n- Merge coincident endpoints robustly with unit/zoom-aware tolerance; avoid false snaps.\n\nAcceptance Criteria\n- Coincident endpoints unify; distant points do not snap.\n\nTasks\n- Tolerance policy and unit scaling.\n- Merge logic in importer.\n- Tests on noisy inputs." 

create_issue "[v1] Export precision + path compaction" "v1,area:svg,type:feature,priority:P2" "Goal\n- Precision controls, optional rounding, and simple path compaction on export.\n\nAcceptance Criteria\n- Round-trip preserves topology and fills.\n\nTasks\n- Precision settings.\n- Compaction heuristics.\n- Round-trip tests." 

# 6. Testing & QA
create_issue "[v1] Property tests for invariants" "v1,area:testing,type:quality,priority:P0" "Goal\n- Proptest scenarios for add/move/remove cycles, bends, handle modes with invariants: half-edge pairing, no dangling refs, faces close.\n\nAcceptance Criteria\n- 10k cases pass locally and in CI.\n\nTasks\n- Define strategies.\n- Implement invariants.\n- Integrate into CI." 

create_issue "[v1] Fuzzing (cargo-fuzz) for SVG/JSON/regions" "v1,area:testing,type:quality,priority:P1" "Goal\n- Fuzz parsers and region builder to catch panics/UB.\n\nAcceptance Criteria\n- 24h fuzz run finds no panics/UB.\n\nTasks\n- Add fuzz targets.\n- CI smoke run.\n- Local long-run guidance." 

create_issue "[v1] Cross-browser wasm tests (CI matrix)" "v1,area:testing,area:tooling,type:infra,priority:P1" "Goal\n- wasm-bindgen tests on Chrome, Firefox, WebKitGTK; matrix with SIMD on/off, threads on/off.\n\nAcceptance Criteria\n- Green matrix in CI.\n\nTasks\n- Set up runners/containers.\n- Configure wasm-pack test invocations.\n- Stabilize flakiness." 

create_issue "[v1] Performance benchmarks + CI gates" "v1,area:testing,area:perf,type:infra,priority:P1" "Goal\n- Micro (cubic eval/flatten/pick) and macro (region recompute) benchmarks enforced by CI thresholds.\n\nAcceptance Criteria\n- Budgets met and regressions blocked.\n\nTasks\n- Bench harness + datasets.\n- Threshold config.\n- CI integration." 

create_issue "[v1] Determinism tests for keys/polys" "v1,area:testing,type:quality,priority:P1" "Goal\n- Same inputs produce same region keys and face polygons across runs/platforms.\n\nAcceptance Criteria\n- Hash comparisons stable in CI.\n\nTasks\n- Golden data and hashing.\n- Cross-platform runs.\n- Report diffs when unstable." 

# 7. Stability & Failure Modes
create_issue "[v1] Graceful degradation + telemetry hooks" "v1,area:core,area:tooling,type:quality,priority:P1" "Goal\n- If worker compute overruns budget, keep UI responsive; mark regions unstable; surface warnings via callback.\n\nAcceptance Criteria\n- Worst-case scenes avoid >16 ms frame drops during edit.\n\nTasks\n- Timeouts + fallback states.\n\n- Warning/event API.\n- Demo handling." 

create_issue "[v1] Centralize tolerances/units/scaling" "v1,area:core,type:quality,priority:P1" "Goal\n- One module that defines epsilons and numeric policy; remove magic numbers.\n\nAcceptance Criteria\n- All thresholds flow from one source; documented.\n\nTasks\n- Introduce constants/types.\n- Replace ad-hoc thresholds.\n- Document policy." 

# 8. Security & Safety
create_issue "[v1] Input caps and sanitization" "v1,area:security,type:quality,priority:P0" "Goal\n- Harden JSON/SVG loaders against untrusted input (size/depth limits, numeric bounds).\n\nAcceptance Criteria\n- Malformed inputs return errors, not panics.\n\nTasks\n- Define caps.\n- Validate on parse.\n- Add negative tests." 

create_issue "[v1] Memory safety checks (MIRI/ASAN)" "v1,area:security,type:quality,priority:P1" "Goal\n- Keep unsafe off or isolate and audit; run MIRI and ASAN/UBSAN for WASM-safe parts.\n\nAcceptance Criteria\n- MIRI + sanitizers pass.\n\nTasks\n- Configure runs.\n- Fix findings.\n- Document guarantees." 

# 9. Tooling & Release
create_issue "[v1] CI pipeline with gates" "v1,area:tooling,type:infra,priority:P0" "Goal\n- fmt, clippy, unit + wasm tests, fuzz smoke, size budget, and benchmarks gate PRs.\n\nAcceptance Criteria\n- Regressions blocked in CI.\n\nTasks\n- CI config.\n- Size/bench checks.\n- Status badges." 

create_issue "[v1] Release artifacts (npm + types)" "v1,area:tooling,area:wasm,type:infra,priority:P0" "Goal\n- Publish npm package (ESM) with .d.ts, versioned WASM, source maps; feature flags for SIMD/threads.\n\nAcceptance Criteria\n- npm install vecnet-wasm works in a minimal app.\n\nTasks\n- Package config + types.\n- Release script.\n- Smoke test app." 

create_issue "[v1] Enforce size budgets" "v1,area:tooling,area:perf,type:infra,priority:P1" "Goal\n- WASM <300 KB gz, JS glue <30 KB gz with CI enforcement.\n\nAcceptance Criteria\n- Budgets enforced; alerts on regressions.\n\nTasks\n- Size measurement in CI.\n- Thresholds + reporting.\n- Docs on budget policy." 

# 10. Documentation & Samples
create_issue "[v1] API reference (docs.rs/site)" "v1,area:docs,type:docs,priority:P1" "Goal\n- Complete API docs with invariants, errors, and performance notes.\n\nAcceptance Criteria\n- wasm-pack doc/site publishes; sections complete.\n\nTasks\n- Doc comments.\n- Reference site generation.\n- CI doc publish." 

create_issue "[v1] Cookbook samples (Canvas/React/Worker)" "v1,area:docs,type:docs,priority:P2" "Goal\n- Minimal Canvas demo, React wrapper, Worker integration, localStorage save/load, custom renderer example.\n\nAcceptance Criteria\n- Each sample runs with one command.\n\nTasks\n- Sample apps.\n- Instructions.\n- CI smoke run." 

create_issue "[v1] Quickstart & Migration guide" "v1,area:docs,type:docs,priority:P1" "Goal\n- Quickstart (build/serve/use) and migration notes for JSON schema/versioning.\n\nAcceptance Criteria\n- New user can import SVG, edit, save in <5 minutes.\n\nTasks\n- Quickstart page.\n- Migration doc.\n- Link from README." 

echo "Queued creation of v1 issues. If nothing errored, issues are now open." 

