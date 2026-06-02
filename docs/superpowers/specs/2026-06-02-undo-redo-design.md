# Undo/Redo System — Design Spec

**Date:** 2026-06-02  
**Status:** Approved  
**Milestone:** Near-Term, #48

## Overview

An engine-native undo/redo system using the hybrid model: structural commands for creation/destruction, targeted snapshots for mutative operations. Driven by explicit gesture grouping from the JS UI layer with stack-based nesting.

---

## 1. Data Model

### UndoStack

```rust
const MAX_UNDO_DEPTH: usize = 256;

pub struct UndoStack {
    entries: VecDeque<UndoEntry>,
}

pub struct UndoEntry {
    pub label: String,
    pub action: UndoAction,
}

pub enum UndoAction {
    Command(UndoCommand),
    Snapshot(SnapshotBatch),
}
```

### Commands (structural — creation/destruction)

```rust
pub enum UndoCommand {
    AddNode    { id: u32 },
    RemoveNode { id: u32, pos: (f32, f32), incident: Vec<(u32, Edge)> },
    AddEdge    { id: u32 },
    RemoveEdge { id: u32, endpoint_a: u32, endpoint_b: u32, edge: Edge },
    AddShape   { id: u32 },
    RemoveShape{ id: u32, shape: Shape },
    AddText    { id: u32 },
    RemoveText { id: u32, text: TextElement },
}
```

### Snapshots (mutative — state changes)

```rust
pub struct SnapshotBatch {
    pub nodes: Vec<(u32, Option<Node>)>,
    pub edges: Vec<(u32, Option<Edge>)>,
    pub fills: Vec<(u32, Option<FillState>)>,
    pub shapes: Vec<(u32, Option<Shape>)>,
    pub texts: Vec<(u32, Option<TextElement>)>,
    pub layers: Vec<(u32, LayerSnapshot)>,
    pub groups: Vec<(u32, GroupSnapshot)>,
    pub effects: Vec<(EffectId, Option<Effect>)>,
}

pub struct LayerSnapshot {
    pub visible: bool,
    pub opacity: f32,
    pub z_index: i32,
}

pub struct GroupSnapshot {
    pub visible: bool,
    pub opacity: f32,
}
```

### Graph additions

```rust
pub struct Graph {
    // … existing fields …
    pub(crate) undo_stack: UndoStack,
    pub(crate) redo_stack: UndoStack,
    pub(crate) undo_group_depth: u32,
    pub(crate) current_snapshot: Option<SnapshotBatch>,
}
```

SnapshotBatch is `None` until `begin_undo_group` is called at depth 1 (lazy allocation, zero cost when not in a gesture).

---

## 2. Lifecycle

### Gesture groups (snapshot path)

1. `begin_undo_group(label)` — depth becomes 1, allocates empty `SnapshotBatch`.
2. Any mutative call (`move_node`, `bend_edge_to`, `set_handle_pos`, `toggle_region`, etc.) — engine records the element's "before" state into the batch if not already recorded. Structural calls (`add_node`, `remove_edge`, etc.) also register their inverse command into the batch.
3. `end_undo_group()` — depth becomes 0, wraps the batch into `UndoEntry`, pushes onto `undo_stack`, clears `redo_stack`.

### Nested groups

`begin_undo_group` increments `undo_group_depth`. If depth > 1, nested opens are no-ops (the group is already capturing). `end_undo_group` decrements depth and only commits at depth 0. This allows compound operations (e.g., line→cubic+bend) to nest without splitting undo history.

### Structural commands (non-grouped path)

When NOT inside a gesture group (`undo_group_depth == 0`), structural operations push a `Command` entry directly:

- `add_node` → `UndoEntry { Command(AddNode { id }) }`
- `add_edge` → `UndoEntry { Command(AddEdge { id }) }`
- `remove_node` → `UndoEntry { Command(RemoveNode { id, pos, incident }) }`
- `remove_edge` → `UndoEntry { Command(RemoveEdge { id, ...edge }) }`

### Restore (undo/redo)

- `undo()`: pop entry from `undo_stack`, apply its reverse effect, push the *same entry* onto `redo_stack`, bump `geom_ver`, call `dirty_reset()`.
- `redo()`: pop entry from `redo_stack`, apply its forward effect, push the *same entry* back onto `undo_stack`, bump `geom_ver`, call `dirty_reset()`.
- The redo stack stores original undo entries (not inverse commands). Redo replays them forward.
- `undo()`/`redo()` call `dirty_reset()` internally so consumers see a clean dirty window relative to the restored `geom_ver`.

### Revert logic

| Entry | Undo (reverse) | Redo (forward) |
|---|---|---|
| `AddNode { id }` | `remove_node(id)` — removes node + incident edges | `add_node_at(x, y)` — re-creates node at original position (requires storing position in command or building from snapshot) |
| `RemoveNode { id, pos, incident }` | Restore node at `pos`, restore each incident edge | `remove_node(id)` |
| `AddEdge { id }` | `remove_edge(id)` | Restore edge at original endpoints with original kind |
| `RemoveEdge { id, ...edge }` | Restore edge at original endpoints with original kind | `remove_edge(id)` |
| `AddShape { id }` | `delete_shape(id)` | Restore shape with original edges |
| `RemoveShape { id, shape }` | Restore shape | `delete_shape(id)` |
| `AddText { id }` | `remove_text(id)` | Restore text element |
| `RemoveText { id, text }` | Restore text | `remove_text(id)` |
| `Snapshot(batch)` | Restore all before-states from batch | Restore all after-states from batch (snapshot batch needs to store both before and after, or redo recomputes forward) |

**Redo for snapshots:** The snapshot stores "before" state. To redo forward, the system needs the "after" state. Two options:
- (a) Store both before and after in the snapshot batch — doubles snapshot memory.
- (b) The snapshot path is only used inside gesture groups. On `end_undo_group`, take a fresh snapshot of the current (post-edit) state. The UndoEntry stores the before-snapshot for undo. For redo, re-capture the current state before applying undo, then use that as the redo snapshot.

**Decision: option (b).** Avoids doubling snapshot memory. Redo captures the after-state at the moment undo is invoked.

### Allocated struct extension: AddNode

`AddNode` needs position data for redo. Extend:

```rust
UndoCommand::AddNode { id: u32, pos: (f32, f32) }
```

---

## 3. WASM API

All methods follow the existing `_res` strict error pattern.

### Methods

```rust
// Group lifecycle
pub fn begin_undo_group_res(&mut self, label: &str) -> JsValue;
pub fn end_undo_group_res(&mut self) -> JsValue;

// Undo/redo
pub fn undo_res(&mut self) -> JsValue;    // → { ok: true, value: { label: string, depth_remaining: number } }
pub fn redo_res(&mut self) -> JsValue;    // → { ok: true, value: { label: string, depth_remaining: number } }

// State queries
pub fn can_undo_res(&self) -> JsValue;    // → { ok: true, value: boolean }
pub fn can_redo_res(&self) -> JsValue;    // → { ok: true, value: boolean }

// Management
pub fn undo_clear_res(&mut self) -> JsValue;     // clears both stacks
pub fn undo_depth_res(&self) -> JsValue;         // → { ok: true, value: { undo: number, redo: number } }
```

### Error cases

| Scenario | Error |
|---|---|
| `undo_res` on empty stack | `{ code: "undo_empty", message: "nothing to undo" }` |
| `redo_res` on empty stack | `{ code: "redo_empty", message: "nothing to redo" }` |
| `begin_undo_group` while undo/redo in progress | `{ code: "undo_in_progress", message: "cannot begin group during undo/redo" }` |
| `end_undo_group` with depth 0 | No error; returns `{ value: { committed: false, depth_was_zero: true } }` |
| `undo_res` / `redo_res` while group open | `{ code: "undo_in_progress", message: "cannot undo/redo while a gesture group is open" }` |

### TypeScript types

```typescript
interface UndoResult {
  label: string;
  depth_remaining: number;  // how many more undo steps exist after this one
}

interface UndoDepthResult {
  undo: number;
  redo: number;
}

// New methods on Graph class
declare class Graph {
  begin_undo_group_res(label: string): Result<boolean>;
  end_undo_group_res(): Result<{ committed: boolean }>;
  undo_res(): Result<UndoResult>;
  redo_res(): Result<UndoResult>;
  can_undo_res(): Result<boolean>;
  can_redo_res(): Result<boolean>;
  undo_clear_res(): Result<boolean>;
  undo_depth_res(): Result<UndoDepthResult>;
}
```

---

## 4. Integration Points

### Module structure

```
contour/src/undo.rs              ← UndoStack, UndoEntry, UndoCommand, SnapshotBatch, undo/redo logic
contour/src/lib.rs               ← Graph gains undo fields + begin/end/undo/redo methods
contour-wasm/src/api.rs          ← 8 new WASM methods
contour-wasm/types.d.ts          ← TypeScript declarations
contour/tests/undo.rs            ← unit tests
contour-wasm/tests/undo_tests.rs ← WASM integration tests
```

### Interaction with existing systems

**`geom_ver` / `get_dirty`:** `undo()` and `redo()` bump `geom_ver` once per call and call `dirty_reset()`. Consumers using `get_dirty(since)` will see `full: true` since the dirty window is relative to the post-restore version. This is correct — undo is an atomic state transition.

**`clear()`:** Resets both undo and redo stacks.

**`from_json` / `from_json_res`:** Resets both undo and redo stacks. The loaded document is the new ground truth.

**`geom_ver` bump on mutations:** Structural operations (add/remove) that push undo entries also bump `geom_ver` as normal. Inside a gesture group, mutations bump `geom_ver` per call, but the composite undo entry only bumps once (the existing per-call bumps are sufficient for dirty tracking).

---

## 5. Memory Model

- **Depth cap:** 256 undo + 256 redo = 512 entries max.
- **Command entry:** ~200 bytes (label string + enum variant + payload).
- **Snapshot entry:** ~2KB typical (1 node + 2 edges + fills). Worst case (1000-node transform_all snapshot): ~24KB.
- **Worst case memory:** 256 × 24KB = ~6MB. Typical memory: 256 × 2KB = ~512KB.
- **Eviction:** Oldest-first from undo stack when cap exceeded. Redo cleared on any new mutation.
- **Lazy allocation:** `current_snapshot` is `None` when not in a gesture group. Zero per-frame cost at steady state.

---

## 6. Testing

### Unit tests (`contour/tests/undo.rs`)

| Test | Description |
|---|---|
| `add_node_undo_redo` | Add node → undo (node gone) → redo (node back, same ID/pos) |
| `remove_node_cascades_edges` | 3-edge star → remove center → undo → node + all 3 edges restored |
| `gesture_group_merges` | begin → move_node×10 → end → single undo step, node at original position |
| `nested_groups_merge` | begin("outer") → begin("inner") → mutate → end → end → one undo entry |
| `gesture_then_structural_undo` | Add node → begin group → move → end → undo (move reversed) → undo (node removed) |
| `redo_after_mutation_clears_redo` | Undo → new edit → redo stack empty |
| `depth_cap_evicts_oldest` | Push MAX+1 entries → oldest evicted → undo_depth = MAX |
| `undo_during_group_errors` | begin group → undo → error |
| `geom_ver_bumps_on_undo` | undo → geom_ver changed |
| `clear_resets_undo` | Edit → undo → redo → clear → can_undo=false, can_redo=false |
| `json_load_resets_undo` | Edit → load document → can_undo=false |
| `snapshot_redo_captures_current` | begin → move → end → undo (captures redo state) → redo → node at moved position |

### WASM tests (`contour-wasm/tests/undo_tests.rs`)

| Test | Description |
|---|---|
| `undo_res_empty_stack_error` | undo_res on fresh graph → error with code "undo_empty" |
| `redo_res_empty_stack_error` | redo_res on fresh graph → error with code "redo_empty" |
| `begin_during_undo_errors` | undo in progress → begin → error |
| `end_without_begin_noop` | end_undo_group with depth 0 → ok, committed: false |
| `label_propagation` | begin("move node") → end → undo_res.value.label = "move node" |
| `depth_remaining_decrements` | Two edits → undo → depth_remaining=1 → undo → depth_remaining=0 |

---

## 7. JS UI Integration Pattern

The demo and any consuming UI follow this pattern for each tool:

```typescript
// Node drag
onPointerDown(e) {
  this.graph.begin_undo_group_res("move node");
  this.dragTarget = pickNode(e);
}
onPointerMove(e) {
  if (this.dragTarget) this.graph.move_node(this.dragTarget, e.x, e.y);
}
onPointerUp(e) {
  if (this.dragTarget) {
    this.graph.end_undo_group_res();
    this.dragTarget = null;
  }
}

// Bend tool
onPointerDown(e) {
  this.graph.begin_undo_group_res("bend");
  this.bendTarget = pickEdge(e);
}
onPointerMove(e) { /* bend */ }
onPointerUp(e) { this.graph.end_undo_group_res(); }

// Pen tool point placement
onFirstClick(e) { this.graph.begin_undo_group_res("pen"); /* add node */ }
onSubsequentClick(e) { /* add node + edge */ }
onFinish() { this.graph.end_undo_group_res(); }  // double-click or Enter

// Keyboard shortcut
document.addEventListener("keydown", (e) => {
  if (e.ctrlKey || e.metaKey) {
    if (e.key === "z" && !e.shiftKey) this.graph.undo_res();
    if (e.key === "z" && e.shiftKey) this.graph.redo_res();
  }
});
```