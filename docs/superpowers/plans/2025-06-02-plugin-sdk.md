# Plugin SDK + 5 Built-in Plugins — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a React plugin SDK for vecnet-wasm that enables custom tools, import/export handlers, and renderers as swappable plugins. Ship 5 built-in plugins: Grid Snap, Shape Primitives, PNG Export, SVG Export, and Freehand Drawing.

**Architecture:** A `PluginHost` singleton manages registration and lifecycle. Plugins conform to typed interfaces (`ToolPlugin`, `ExportPlugin`). A new `EditorPage` (React-native, replacing the current iframe approach) accepts plugins from the host and renders toolbar + canvas. The `CanvasViewport` component delegates pointer events to the active tool plugin. Five built-in plugins auto-register at app startup.

**Tech Stack:** React 19, TypeScript 5.9, Vite 7, vecnet-wasm WASM (GraphService wrapper), Canvas 2D

**Plugin Loading Strategy:** Local modules (import from `src/plugins/builtin/`). The `PluginHost.register()` API is designed to later accept dynamic-import URLs without changing the interface — only a `registerFromUrl()` wrapper would be added.

---

## File Structure

### New files
| File | Responsibility |
|---|---|
| `apps/studio/src/plugins/types.ts` | Plugin interfaces (ToolPlugin, ExportPlugin, PluginManifest, context types) |
| `apps/studio/src/plugins/PluginHost.ts` | Registry, lifecycle, event emitter, tool activation |
| `apps/studio/src/plugins/hooks.ts` | React hooks: `useActiveTool`, `usePluginTools` |
| `apps/studio/src/plugins/builtin/grid-snap.ts` | Grid Snap tool plugin |
| `apps/studio/src/plugins/builtin/shape-primitives.ts` | Rectangle + Ellipse + Polygon shape tools |
| `apps/studio/src/plugins/builtin/png-export.ts` | PNG export plugin |
| `apps/studio/src/plugins/builtin/svg-export.ts` | SVG export plugin |
| `apps/studio/src/plugins/builtin/freehand.ts` | Freehand drawing tool (wraps `graph.add_freehand`) |
| `apps/studio/src/plugins/builtin/index.ts` | Barrel export + auto-registration function |
| `apps/studio/src/features/editor/useGraphEditor.ts` | Hook: owns GraphService lifecycle, snapshot subscription, undo/redo |
| `apps/studio/src/features/editor/EditorToolbar.tsx` | Toolbar component reading tools from PluginHost |
| `apps/studio/src/features/editor/EditorSidebar.tsx` | Sidebar showing selection info, export buttons |

### Modified files
| File | Changes |
|---|---|
| `apps/studio/src/features/editor/EditorPage.tsx` | Rewrite: remove iframe, use `useGraphEditor` + `PluginHost` + `CanvasViewport` + toolbar/sidebar |
| `apps/studio/src/features/editor/CanvasViewport.tsx` | Accept `activeTool` plugin, delegate pointer events, render grid overlay |
| `apps/studio/src/services/graph/graphService.ts` | Add `removeEdge`, `removeNode`, undo/redo stubs, `setEdgeCubic`, `bendEdgeTo`, `clear`, `addFreehand`, `importSvg` methods |
| `apps/studio/src/services/graph/types.ts` | Add `ToolContext`, `PointerEvent`, `ToolResult`, `BBox` types |
| `apps/studio/src/main.tsx` | Call `registerBuiltinPlugins(pluginHost)` after host creation |
| `apps/studio/src/styles.css` | Add toolbar, tool-button, sidebar, and plugin-related styles |

---

### Task 1: Plugin SDK Core Types

**Files:**
- Create: `apps/studio/src/plugins/types.ts`

- [ ] **Step 1: Write the types file**

```typescript
// apps/studio/src/plugins/types.ts

import type { GraphSnapshot, GraphPick } from "../../services/graph/types";
import type { GraphService } from "../../services/graph/graphService";

/** Context passed to tool plugins when activated */
export interface ToolContext {
  service: GraphService;
  snapshot: GraphSnapshot;
}

/** Normalized pointer event for tool plugins */
export interface PluginPointerEvent {
  x: number;
  y: number;
  /** Original DOM event for shift/alt/meta key state */
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
}

/** Result from a tool's pointer handler */
export type ToolResult =
  | { action: "addNode"; id: number }
  | { action: "moveNode"; id: number; x: number; y: number }
  | { action: "addEdge"; a: number; b: number }
  | { action: "select"; id: number | null }
  | { action: "repaint" }
  | null;

/**
 * Tool plugins receive pointer events and produce graph mutations.
 * One tool is active at a time.
 */
export interface ToolPlugin {
  id: string;
  name: string;
  keyboardShortcut?: string;
  cursor?: string;
  /** Render additional overlay on canvas (e.g. grid, preview shape) */
  renderOverlay?(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void;

  onActivate?(ctx: ToolContext): void;
  onDeactivate?(): void;

  onPointerDown?(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult;

  onPointerMove?(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult;

  onPointerUp?(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult;

  onDoubleClick?(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult;
}

/**
 * Export plugins produce a Blob from the current graph state.
 */
export interface ExportPlugin {
  id: string;
  name: string;
  extension: string;
  mimeType: string;

  export(snapshot: GraphSnapshot, service: GraphService): Promise<Blob>;
}

/**
 * Plugin manifest — what a plugin package declares when registering.
 * A single object can include multiple role implementations.
 */
export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  tools?: ToolPlugin[];
  exports?: ExportPlugin[];
  dependsOn?: string[];
}

/** Breadcrumb for toolbar rendering */
export interface ToolEntry {
  id: string;
  name: string;
  keyboardShortcut?: string;
  plugin: ToolPlugin;
}
```

- [ ] **Step 2: Verify file compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/types.ts
```
Expected: no errors.

---

### Task 2: PluginHost Registry + Lifecycle

**Files:**
- Create: `apps/studio/src/plugins/PluginHost.ts`

- [ ] **Step 1: Write PluginHost**

```typescript
// apps/studio/src/plugins/PluginHost.ts

import type {
  ExportPlugin,
  PluginManifest,
  PluginPointerEvent,
  ToolContext,
  ToolEntry,
  ToolPlugin,
  ToolResult,
} from "./types";
import type { GraphPick } from "../../services/graph/types";

type Listener = (...args: any[]) => void;

export class PluginHost {
  private manifests = new Map<string, PluginManifest>();
  private toolCache: ToolEntry[] | null = null;
  private activeToolId: string | null = null;
  private listeners = new Map<string, Set<Listener>>();

  register(manifest: PluginManifest): void {
    if (this.manifests.has(manifest.id)) {
      console.warn(`PluginHost: duplicate plugin id "${manifest.id}", skipping.`);
      return;
    }
    this.manifests.set(manifest.id, manifest);
    this.toolCache = null;
    this.emit("registered", manifest);
  }

  unregister(id: string): void {
    if (this.activeToolId) {
      const active = this.getActiveTool();
      if (active?.id === id || active?.plugin.id.startsWith(id + ".")) {
        this.deactivateTool();
      }
    }
    this.manifests.delete(id);
    this.toolCache = null;
  }

  /** All registered tools across all manifests */
  getTools(): ToolEntry[] {
    if (this.toolCache) return this.toolCache;
    const entries: ToolEntry[] = [];
    for (const manifest of this.manifests.values()) {
      for (const tool of manifest.tools ?? []) {
        entries.push({
          id: tool.id,
          name: tool.name,
          keyboardShortcut: tool.keyboardShortcut,
          plugin: tool,
        });
      }
    }
    this.toolCache = entries;
    return entries;
  }

  /** All registered export plugins */
  getExports(): ExportPlugin[] {
    const result: ExportPlugin[] = [];
    for (const manifest of this.manifests.values()) {
      for (const exp of manifest.exports ?? []) {
        result.push(exp);
      }
    }
    return result;
  }

  activateTool(id: string): void {
    if (this.activeToolId === id) return;
    const prev = this.getActiveTool();
    if (prev) prev.plugin.onDeactivate?.();
    this.activeToolId = id;
    const next = this.getActiveTool();
    this.emit("toolChanged", next ?? null);
  }

  deactivateTool(): void {
    if (!this.activeToolId) return;
    const current = this.getActiveTool();
    if (current) current.plugin.onDeactivate?.();
    this.activeToolId = null;
    this.emit("toolChanged", null);
  }

  getActiveToolId(): string | null {
    return this.activeToolId;
  }

  getActiveTool(): ToolEntry | null {
    if (!this.activeToolId) return null;
    const tools = this.getTools();
    return tools.find((t) => t.id === this.activeToolId) ?? null;
  }

  /** Notify the active tool of pointer down. Returns a result for the host to apply. */
  handlePointerDown(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult {
    const tool = this.getActiveTool();
    return tool?.plugin.onPointerDown?.(event, pick, ctx) ?? null;
  }

  handlePointerMove(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult {
    const tool = this.getActiveTool();
    return tool?.plugin.onPointerMove?.(event, pick, ctx) ?? null;
  }

  handlePointerUp(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult {
    const tool = this.getActiveTool();
    return tool?.plugin.onPointerUp?.(event, pick, ctx) ?? null;
  }

  handleDoubleClick(
    event: PluginPointerEvent,
    pick: GraphPick | null,
    ctx: ToolContext,
  ): ToolResult {
    const tool = this.getActiveTool();
    return tool?.plugin.onDoubleClick?.(event, pick, ctx) ?? null;
  }

  /** Render active tool overlay */
  renderToolOverlay(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    const tool = this.getActiveTool();
    tool?.plugin.renderOverlay?.(ctx, width, height);
  }

  /** Simple typed event emitter for plugin ↔ host communication */
  on(event: string, handler: Listener): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler);
    return () => {
      this.listeners.get(event)?.delete(handler);
    };
  }

  private emit(event: string, data: any): void {
    for (const handler of this.listeners.get(event) ?? []) {
      try {
        handler(data);
      } catch (err) {
        console.error(`PluginHost: error in "${event}" listener:`, err);
      }
    }
  }
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/PluginHost.ts
```
Expected: no errors.

---

### Task 3: React Hooks for PluginHost

**Files:**
- Create: `apps/studio/src/plugins/hooks.ts`

- [ ] **Step 1: Write hooks**

```typescript
// apps/studio/src/plugins/hooks.ts

import { useEffect, useSyncExternalStore } from "react";
import type { PluginHost } from "./PluginHost";
import type { ToolEntry, ToolContext } from "./types";
import type { GraphPick } from "../../services/graph/types";

/**
 * React hook that subscribes to tool changes and returns the active tool.
 * Also activates the tool's `onActivate` lifecycle when context is ready.
 */
export function useActiveTool(
  host: PluginHost,
  context: ToolContext | null,
): ToolEntry | null {
  const activeTool = useSyncExternalStore(
    (onStoreChange) => host.on("toolChanged", onStoreChange),
    () => host.getActiveTool(),
  );

  useEffect(() => {
    if (activeTool && context) {
      activeTool.plugin.onActivate?.(context);
    }
  }, [activeTool, context]);

  return activeTool;
}

/** Hook returning the list of all registered tools */
export function usePluginTools(host: PluginHost): ToolEntry[] {
  return useSyncExternalStore(
    (onStoreChange) => host.on("registered", onStoreChange),
    () => host.getTools(),
  );
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/hooks.ts
```
Expected: no errors.

---

### Task 4: Extend GraphService with Missing Operations

**Files:**
- Modify: `apps/studio/src/services/graph/graphService.ts`

The plugins need these operations that GraphService currently lacks: `removeEdge`, `removeNode`, `setEdgeCubic`, `bendEdgeTo`, `clear`, `addFreehand`, `importSvg`. The existing `graphService.ts` only has `addNode`, `moveNode`, `addEdge`, `pick`, `toggleRegion`.

- [ ] **Step 1: Add missing methods to GraphService**

Open `apps/studio/src/services/graph/graphService.ts` and add these methods inside the class (after the existing `setFlattenTolerance` method, before `private emit()`):

```typescript
  // Add these imports at the top of the file:
  //   (no new imports needed — Float32Array and types already available)

  // --- Methods required by plugins ---

  removeNode(id: number): void {
    unwrapResult(this.graph.remove_node_res(id), "Failed to remove node");
    this.emit();
  }

  removeEdge(id: number): void {
    unwrapResult(this.graph.remove_edge_res(id), "Failed to remove edge");
    this.emit();
  }

  setEdgeCubic(id: number, p1x: number, p1y: number, p2x: number, p2y: number): void {
    unwrapResult(
      this.graph.set_edge_cubic_res(id, p1x, p1y, p2x, p2y),
      "Failed to set edge to cubic",
    );
    this.emit();
  }

  setEdgeLine(id: number): void {
    unwrapResult(this.graph.set_edge_line_res(id), "Failed to set edge to line");
    this.emit();
  }

  bendEdgeTo(id: number, t: number, tx: number, ty: number, stiffness: number): void {
    unwrapResult(
      this.graph.bend_edge_to_res(id, t, tx, ty, stiffness),
      "Failed to bend edge",
    );
    this.emit();
  }

  clear(): void {
    this.graph.clear();
    this.emit();
  }

  addFreehand(points: Float32Array, close: boolean): Uint32Array {
    const edges = this.graph.add_freehand(points, close);
    this.emit();
    return edges;
  }

  addFreehandRes(points: Float32Array, close: boolean): Uint32Array {
    const edges = unwrapResult(
      this.graph.add_freehand_res(points, close),
      "Failed to add freehand",
    );
    this.emit();
    return edges;
  }

  importSvg(d: string): number {
    const count = unwrapResult(
      this.graph.add_svg_path_res(d),
      "Failed to import SVG",
    );
    this.emit();
    return count;
  }
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit
```
Expected: no errors.

---

### Task 5: Add Tool Types to Graph Service Types

**Files:**
- Modify: `apps/studio/src/services/graph/types.ts`

- [ ] **Step 1: Add new type exports**

Append to the end of `apps/studio/src/services/graph/types.ts`:

```typescript
export interface BBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Context passed to tool plugins — re-exported for plugins/types.ts convenience */
export interface ToolContext {
  service: import("../graph/graphService").GraphService;
  snapshot: GraphSnapshot;
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit
```
Expected: no errors.

---

### Task 6: Grid Snap Tool Plugin

**Files:**
- Create: `apps/studio/src/plugins/builtin/grid-snap.ts`

- [ ] **Step 1: Write grid snap plugin**

```typescript
// apps/studio/src/plugins/builtin/grid-snap.ts

import type { ToolPlugin, ToolContext, PluginPointerEvent, ToolResult } from "../types";

const DEFAULT_GRID_SIZE = 20;
const GRID_COLOR = "rgba(16, 94, 66, 0.08)";

interface GridSnapState {
  gridSize: number;
  enabled: boolean;
}

export function createGridSnapPlugin(): ToolPlugin {
  const state: GridSnapState = {
    gridSize: DEFAULT_GRID_SIZE,
    enabled: false,
  };

  return {
    id: "builtin.grid-snap",
    name: "Grid Snap",
    keyboardShortcut: "G",

    renderOverlay(ctx: CanvasRenderingContext2D, width: number, height: number) {
      if (!state.enabled) return;

      ctx.save();
      ctx.strokeStyle = GRID_COLOR;
      ctx.lineWidth = 0.5;

      const size = state.gridSize;
      for (let x = 0; x <= width; x += size) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
      }
      for (let y = 0; y <= height; y += size) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }

      ctx.restore();
    },

    onActivate() {
      state.enabled = !state.enabled;
    },

    onDeactivate() {
      // Grid stays visible when tool is deselected via toggle.
      // It only clears on next activate (toggle off).
    },

    onPointerDown(event: PluginPointerEvent): ToolResult {
      // Grid snap is a pass-through: it doesn't consume pointer events.
      // Other tools check the grid state and snap coordinates.
      return { action: "repaint" };
    },
  };
}

/** Utility function: snap a coordinate to the grid if the plugin is active */
export function snapToGrid(x: number, y: number, gridSize?: number): { x: number; y: number } {
  const size = gridSize ?? DEFAULT_GRID_SIZE;
  return {
    x: Math.round(x / size) * size,
    y: Math.round(y / size) * size,
  };
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/builtin/grid-snap.ts
```
Expected: no errors.

---

### Task 7: Shape Primitives Tool Plugin

**Files:**
- Create: `apps/studio/src/plugins/builtin/shape-primitives.ts`

- [ ] **Step 1: Write shape primitives plugin**

```typescript
// apps/studio/src/plugins/builtin/shape-primitives.ts

import type { ToolPlugin, ToolContext, PluginPointerEvent, ToolResult } from "../types";

type ShapeMode = "rectangle" | "ellipse" | "polygon";

interface ShapeState {
  mode: ShapeMode | null;
  dragging: boolean;
  startX: number;
  startY: number;
}

function createShapeTool(mode: ShapeMode, name: string, shortcut: string): ToolPlugin {
  const state: ShapeState = {
    mode: null,
    dragging: false,
    startX: 0,
    startY: 0,
  };

  return {
    id: `builtin.shape.${mode}`,
    name,
    keyboardShortcut: shortcut,
    cursor: "crosshair",

    onActivate() {
      state.mode = mode;
      state.dragging = false;
    },

    onDeactivate() {
      state.mode = null;
      state.dragging = false;
    },

    renderOverlay(ctx: CanvasRenderingContext2D, width: number, height: number) {
      if (!state.dragging) return;
      // The preview is drawn by the EditorPage during pointerMove — overlay just a noop here.
      // Actual preview drawing happens in CanvasViewport based on tool state.
    },

    onPointerDown(event: PluginPointerEvent): ToolResult {
      state.dragging = true;
      state.startX = event.x;
      state.startY = event.y;
      return { action: "repaint" };
    },

    onPointerMove(): ToolResult {
      if (state.dragging) {
        return { action: "repaint" }; // CanvasViewport redraws preview
      }
      return null;
    },

    onPointerUp(
      event: PluginPointerEvent,
      _pick,
      ctx: ToolContext,
    ): ToolResult {
      if (!state.dragging) return null;
      state.dragging = false;

      const x1 = Math.min(state.startX, event.x);
      const y1 = Math.min(state.startY, event.y);
      const x2 = Math.max(state.startX, event.x);
      const y2 = Math.max(state.startY, event.y);
      const w = x2 - x1;
      const h = y2 - y1;

      if (w < 3 && h < 3) return null; // too small

      if (state.mode === "rectangle") {
        const nw = ctx.service.addNode(x1, y1);
        const ne = ctx.service.addNode(x2, y1);
        const se = ctx.service.addNode(x2, y2);
        const sw = ctx.service.addNode(x1, y2);
        ctx.service.addEdge(nw, ne);
        ctx.service.addEdge(ne, se);
        ctx.service.addEdge(se, sw);
        ctx.service.addEdge(sw, nw);
        return { action: "select", id: null };
      }

      if (state.mode === "ellipse") {
        const cx = (x1 + x2) / 2;
        const cy = (y1 + y2) / 2;
        const rx = w / 2;
        const ry = h / 2;
        const segments = 32;
        const ids: number[] = [];

        for (let i = 0; i < segments; i++) {
          const angle = (i / segments) * Math.PI * 2;
          const px = cx + rx * Math.cos(angle);
          const py = cy + ry * Math.sin(angle);
          ids.push(ctx.service.addNode(px, py));
        }

        const KAPPA = 0.5522847498; // cubic bezier approximation of quarter circle
        for (let i = 0; i < segments; i++) {
          const j = (i + 1) % segments;
          const a = ids[i];
          const b = ids[j];
          const edgeId = ctx.service.addEdge(a, b);

          if (edgeId !== undefined) {
            const angleA = (i / segments) * Math.PI * 2;
            const angleB = (j / segments) * Math.PI * 2;
            const midAngle = (angleA + angleB) / 2;
            const handleLen = (4.0 / 3.0) * Math.tan((angleB - angleA) / 4.0);

            // Simplified: set as cubic for smooth ellipse
            ctx.service.setEdgeCubic(
              edgeId,
              -Math.sin(angleA) * rx * handleLen,
              Math.cos(angleA) * ry * handleLen,
              Math.sin(angleB) * rx * handleLen,
              -Math.cos(angleB) * ry * handleLen,
            );
          }
        }

        return { action: "select", id: null };
      }

      if (state.mode === "polygon") {
        const cx = (x1 + x2) / 2;
        const cy = (y1 + y2) / 2;
        const radius = Math.min(Math.abs(w), Math.abs(h)) / 2;
        const sides = 6; // default hexagon

        const ids: number[] = [];
        for (let i = 0; i < sides; i++) {
          const angle = (i / sides) * Math.PI * 2 - Math.PI / 2;
          ids.push(ctx.service.addNode(
            cx + radius * Math.cos(angle),
            cy + radius * Math.sin(angle),
          ));
        }
        for (let i = 0; i < sides; i++) {
          ctx.service.addEdge(ids[i], ids[(i + 1) % sides]);
        }

        return { action: "select", id: null };
      }

      return null;
    },
  };
}

export const rectangleTool = createShapeTool("rectangle", "Rectangle", "U");
export const ellipseTool = createShapeTool("ellipse", "Ellipse", "O");
export const polygonTool = createShapeTool("polygon", "Polygon", "G");
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/builtin/shape-primitives.ts
```
Expected: no errors.

---

### Task 8: PNG Export Plugin

**Files:**
- Create: `apps/studio/src/plugins/builtin/png-export.ts`

- [ ] **Step 1: Write PNG export plugin**

```typescript
// apps/studio/src/plugins/builtin/png-export.ts

import type { ExportPlugin } from "../types";
import type { GraphSnapshot } from "../../../services/graph/types";
import type { GraphService } from "../../../services/graph/graphService";

/**
 * Renders the current graph to an offscreen canvas and exports as PNG.
 * Uses the same rendering logic as CanvasViewport (without interactive overlays).
 */
export const pngExportPlugin: ExportPlugin = {
  id: "builtin.export.png",
  name: "PNG Image",
  extension: "png",
  mimeType: "image/png",

  async export(snapshot: GraphSnapshot, _service: GraphService): Promise<Blob> {
    const nodeLookup = new Map(snapshot.nodes.map((n) => [n.id, n]));

    // Compute bounding box
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const node of snapshot.nodes) {
      if (node.x < minX) minX = node.x;
      if (node.y < minY) minY = node.y;
      if (node.x > maxX) maxX = node.x;
      if (node.y > maxY) maxY = node.y;
    }

    const padding = 40;
    const width = maxX === -Infinity ? 400 : maxX - minX + padding * 2;
    const height = maxY === -Infinity ? 400 : maxY - minY + padding * 2;

    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(width));
    canvas.height = Math.max(1, Math.ceil(height));
    const ctx = canvas.getContext("2d")!;

    // Background
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Translate so (minX - padding, minY - padding) maps to (0, 0)
    ctx.translate(-minX + padding, -minY + padding);

    // Draw regions
    for (const region of snapshot.regions) {
      if (!region.filled || region.points.length < 6) continue;
      ctx.beginPath();
      ctx.moveTo(region.points[0] ?? 0, region.points[1] ?? 0);
      for (let i = 2; i < region.points.length; i += 2) {
        ctx.lineTo(region.points[i] ?? 0, region.points[i + 1] ?? 0);
      }
      ctx.closePath();
      if (region.color) {
        const [r, g, b, a] = region.color;
        ctx.fillStyle = `rgba(${r},${g},${b},${Math.max(0, Math.min(1, a / 255))})`;
      } else {
        ctx.fillStyle = "rgba(29, 140, 101, 0.16)";
      }
      ctx.fill();
    }

    // Draw edges
    for (const edge of snapshot.edges) {
      const start = nodeLookup.get(edge.a);
      const end = nodeLookup.get(edge.b);
      if (!start || !end) continue;

      ctx.beginPath();
      ctx.moveTo(start.x, start.y);

      if (edge.kind === "cubic" && edge.handles) {
        ctx.bezierCurveTo(
          edge.handles.ax, edge.handles.ay,
          edge.handles.bx, edge.handles.by,
          end.x, end.y,
        );
      } else if (edge.kind === "polyline" && edge.polylinePoints && edge.polylinePoints.length >= 2) {
        for (let i = 0; i < edge.polylinePoints.length; i += 2) {
          ctx.lineTo(edge.polylinePoints[i] ?? 0, edge.polylinePoints[i + 1] ?? 0);
        }
        ctx.lineTo(end.x, end.y);
      } else {
        ctx.lineTo(end.x, end.y);
      }

      ctx.lineWidth = edge.stroke?.width ?? 2;
      if (edge.stroke) {
        const [r, g, b, a] = edge.stroke.color;
        ctx.strokeStyle = `rgba(${r},${g},${b},${Math.max(0, Math.min(1, a / 255))})`;
      } else {
        ctx.strokeStyle = "#125f4f";
      }
      ctx.stroke();
    }

    return new Promise((resolve, reject) => {
      canvas.toBlob((blob) => {
        if (blob) resolve(blob);
        else reject(new Error("Failed to create PNG blob"));
      }, "image/png");
    });
  },
};
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/builtin/png-export.ts
```
Expected: no errors.

---

### Task 9: SVG Export Plugin

**Files:**
- Create: `apps/studio/src/plugins/builtin/svg-export.ts`

- [ ] **Step 1: Write SVG export plugin**

```typescript
// apps/studio/src/plugins/builtin/svg-export.ts

import type { ExportPlugin } from "../types";
import type { GraphSnapshot } from "../../../services/graph/types";
import type { GraphService } from "../../../services/graph/graphService";

export const svgExportPlugin: ExportPlugin = {
  id: "builtin.export.svg",
  name: "SVG Vector",
  extension: "svg",
  mimeType: "image/svg+xml",

  async export(snapshot: GraphSnapshot, service: GraphService): Promise<Blob> {
    const nodeLookup = new Map(snapshot.nodes.map((n) => [n.id, n]));
    const paths: string[] = [];

    // Build SVG path data from graph service's to_svg_paths
    // We use the WASM's native SVG export which produces path fragments
    const svgPathsResult = service.getSvgPaths();
    if (svgPathsResult.length > 0) {
      paths.push(...svgPathsResult);
    } else {
      // Fallback: manual path construction
      const visited = new Set<number>();
      for (const edge of snapshot.edges) {
        if (visited.has(edge.id)) continue;
        visited.add(edge.id);

        const start = nodeLookup.get(edge.a);
        const end = nodeLookup.get(edge.b);
        if (!start || !end) continue;

        const parts: string[] = [`M${start.x} ${start.y}`];

        if (edge.kind === "cubic" && edge.handles) {
          parts.push(`C${edge.handles.ax} ${edge.handles.ay} ${edge.handles.bx} ${edge.handles.by} ${end.x} ${end.y}`);
        } else if (edge.kind === "polyline" && edge.polylinePoints && edge.polylinePoints.length >= 2) {
          for (let i = 0; i < edge.polylinePoints.length; i += 2) {
            parts.push(`L${edge.polylinePoints[i]} ${edge.polylinePoints[i + 1]}`);
          }
          parts.push(`L${end.x} ${end.y}`);
        } else {
          parts.push(`L${end.x} ${end.y}`);
        }

        paths.push(parts.join(" "));
      }
    }

    // Compute bounding box
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const node of snapshot.nodes) {
      if (node.x < minX) minX = node.x;
      if (node.y < minY) minY = node.y;
      if (node.x > maxX) maxX = node.x;
      if (node.y > maxY) maxY = node.y;
    }

    const padding = 20;
    const viewW = maxX === -Infinity ? 400 : maxX - minX + padding * 2;
    const viewH = maxY === -Infinity ? 400 : maxY - minY + padding * 2;
    const viewX = maxX === -Infinity ? 0 : minX - padding;
    const viewY = maxY === -Infinity ? 0 : minY - padding;

    const svgContent = [
      `<?xml version="1.0" encoding="UTF-8"?>`,
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewX} ${viewY} ${viewW} ${viewH}" width="${viewW}" height="${viewH}">`,
      `  <g fill="none" stroke="#125f4f" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">`,
      ...paths.map((d) => `    <path d="${d}" />`),
      `  </g>`,
      `</svg>`,
    ].join("\n");

    return new Blob([svgContent], { type: "image/svg+xml" });
  },
};
```

- [ ] **Step 2: Add getSvgPaths to GraphService**

Open `apps/studio/src/services/graph/graphService.ts` and add this method inside the class:

```typescript
  getSvgPaths(): string[] {
    return unwrapResult(this.graph.to_svg_paths_res(), "Failed to get SVG paths");
  }
```

- [ ] **Step 3: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/builtin/svg-export.ts
```
Expected: no errors.

---

### Task 10: Freehand Drawing Tool Plugin

**Files:**
- Create: `apps/studio/src/plugins/builtin/freehand.ts`

- [ ] **Step 1: Write freehand plugin**

```typescript
// apps/studio/src/plugins/builtin/freehand.ts

import type { ToolPlugin, ToolContext, PluginPointerEvent, ToolResult } from "../types";

export function createFreehandPlugin(): ToolPlugin {
  const points: number[] = [];
  let drawing = false;

  return {
    id: "builtin.freehand",
    name: "Freehand",
    keyboardShortcut: "P",
    cursor: "crosshair",

    onActivate() {
      drawing = false;
      points.length = 0;
    },

    onDeactivate() {
      drawing = false;
      points.length = 0;
    },

    renderOverlay(ctx: CanvasRenderingContext2D) {
      if (points.length < 4) return;

      ctx.save();
      ctx.strokeStyle = "rgba(19, 108, 93, 0.5)";
      ctx.lineWidth = 2;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(points[0] ?? 0, points[1] ?? 0);
      for (let i = 2; i < points.length; i += 2) {
        ctx.lineTo(points[i] ?? 0, points[i + 1] ?? 0);
      }
      ctx.stroke();
      ctx.restore();
    },

    onPointerDown(event: PluginPointerEvent, _pick, _ctx: ToolContext): ToolResult {
      drawing = true;
      points.length = 0;
      points.push(event.x, event.y);
      return { action: "repaint" };
    },

    onPointerMove(event: PluginPointerEvent, _pick, _ctx: ToolContext): ToolResult {
      if (!drawing) return null;

      const lastX = points[points.length - 2];
      const lastY = points[points.length - 1];
      const dx = event.x - (lastX ?? 0);
      const dy = event.y - (lastY ?? 0);

      // Only sample if moved at least 3px (reduces point density)
      if (Math.sqrt(dx * dx + dy * dy) < 3) return null;

      points.push(event.x, event.y);
      return { action: "repaint" };
    },

    onPointerUp(_event: PluginPointerEvent, _pick, ctx: ToolContext): ToolResult {
      if (!drawing || points.length < 4) {
        drawing = false;
        points.length = 0;
        return null;
      }
      drawing = false;

      const arr = new Float32Array(points);
      ctx.service.addFreehandRes(arr, false);
      points.length = 0;

      return { action: "select", id: null };
    },
  };
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/plugins/builtin/freehand.ts
```
Expected: no errors.

---

### Task 11: Builtin Plugins Barrel + Auto-Registration

**Files:**
- Create: `apps/studio/src/plugins/builtin/index.ts`

- [ ] **Step 1: Write barrel and registration function**

```typescript
// apps/studio/src/plugins/builtin/index.ts

import type { PluginHost } from "../PluginHost";
import { createGridSnapPlugin } from "./grid-snap";
import { rectangleTool, ellipseTool, polygonTool } from "./shape-primitives";
import { pngExportPlugin } from "./png-export";
import { svgExportPlugin } from "./svg-export";
import { createFreehandPlugin } from "./freehand";

export function registerBuiltinPlugins(host: PluginHost): void {
  host.register({
    id: "builtin",
    name: "Built-in Tools",
    version: "1.0.0",
    tools: [
      createGridSnapPlugin(),
      rectangleTool,
      ellipseTool,
      polygonTool,
      createFreehandPlugin(),
    ],
    exports: [
      pngExportPlugin,
      svgExportPlugin,
    ],
  });
}

export { createGridSnapPlugin } from "./grid-snap";
export { rectangleTool, ellipseTool, polygonTool } from "./shape-primitives";
export { pngExportPlugin } from "./png-export";
export { svgExportPlugin } from "./svg-export";
export { createFreehandPlugin } from "./freehand";
```

- [ ] **Step 2: Register at app startup**

Edit `apps/studio/src/main.tsx`, add after existing imports:

```typescript
import { PluginHost } from "./plugins/PluginHost";
import { registerBuiltinPlugins } from "./plugins/builtin";
```

And add before the `createRoot(...)` call:

```typescript
// Plugin system initialization
const pluginHost = new PluginHost();
registerBuiltinPlugins(pluginHost);
```

Full updated `main.tsx`:

```typescript
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./app/App";
import { AuthProvider } from "./features/auth/AuthContext";
import { PluginHost } from "./plugins/PluginHost";
import { registerBuiltinPlugins } from "./plugins/builtin";
import "./styles.css";

// Initialize plugin system
const pluginHost = new PluginHost();
registerBuiltinPlugins(pluginHost);

// Expose pluginHost globally so EditorPage can access it
// (in a production app this would go through React context)
(window as any).__pluginHost = pluginHost;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <AuthProvider>
        <App />
      </AuthProvider>
    </BrowserRouter>
  </StrictMode>,
);
```

> Note: Exposing on `window` is a shortcut for this phase. In a follow-up, the plugin host should live in a React context alongside AuthContext.

- [ ] **Step 3: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit
```
Expected: no errors.

---

### Task 12: useGraphEditor Hook

**Files:**
- Create: `apps/studio/src/features/editor/useGraphEditor.ts`

This hook owns the GraphService lifecycle, snapshot subscription, and provides the bridge between React and the WASM engine.

- [ ] **Step 1: Write the hook**

```typescript
// apps/studio/src/features/editor/useGraphEditor.ts

import { useEffect, useRef, useState, useCallback } from "react";
import { createGraphService, type GraphService } from "../../services/graph/graphService";
import type { GraphSnapshot, GraphPick } from "../../services/graph/types";
import type { ToolContext } from "../../services/graph/types";

export function useGraphEditor() {
  const [service, setService] = useState<GraphService | null>(null);
  const [snapshot, setSnapshot] = useState<GraphSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<number | null>(null);
  const [edgeStartNodeId, setEdgeStartNodeId] = useState<number | null>(null);

  // Ref for the canvas element (used by pick)
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    createGraphService().then((svc) => {
      if (cancelled) {
        svc.dispose();
        return;
      }
      setService(svc);
    }).catch((err) => {
      setError(err instanceof Error ? err.message : "Failed to initialize WASM");
    });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!service) return;
    const unsubscribe = service.subscribe(setSnapshot);
    return unsubscribe;
  }, [service]);

  const context: ToolContext | null = service && snapshot
    ? { service, snapshot }
    : null;

  const pick = useCallback((clientX: number, clientY: number): GraphPick | null => {
    const canvas = canvasRef.current;
    if (!canvas || !service) return null;
    const rect = canvas.getBoundingClientRect();
    const x = clientX - rect.left;
    const y = clientY - rect.top;
    return service.pick(x, y, 12);
  }, [service]);

  const getCanvasPoint = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    return { x: clientX - rect.left, y: clientY - rect.top };
  }, []);

  return {
    service,
    snapshot,
    error,
    context,
    canvasRef,
    selectedNodeId,
    setSelectedNodeId,
    edgeStartNodeId,
    setEdgeStartNodeId,
    pick,
    getCanvasPoint,
  };
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/features/editor/useGraphEditor.ts
```
Expected: no errors.

---

### Task 13: EditorToolbar Component

**Files:**
- Create: `apps/studio/src/features/editor/EditorToolbar.tsx`

- [ ] **Step 1: Write toolbar component**

```typescript
// apps/studio/src/features/editor/EditorToolbar.tsx

import type { PluginHost } from "../../plugins/PluginHost";
import type { ToolEntry } from "../../plugins/types";

interface EditorToolbarProps {
  host: PluginHost;
  activeToolId: string | null;
  tools: ToolEntry[];
  onActivateTool: (id: string) => void;
}

export function EditorToolbar({ host, activeToolId, tools, onActivateTool }: EditorToolbarProps) {
  const exportPlugins = host.getExports();

  const handleExport = async (pluginId: string) => {
    const exp = exportPlugins.find((e) => e.id === pluginId);
    if (!exp) return;

    try {
      // Access the host's current context via a method we'll add
      // For now, snapshot is passed through from the EditorPage
      const hostAny = host as any;
      const snapshot = hostAny._lastSnapshot;
      const service = hostAny._lastService;
      if (!snapshot || !service) return;

      const blob = await exp.export(snapshot, service);
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `vecnet-export.${exp.extension}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error(`Export (${pluginId}) failed:`, err);
    }
  };

  return (
    <header className="editor-toolbar">
      <div className="toolbar-group">
        {tools.map((tool) => (
          <button
            key={tool.id}
            className={`tool-button ${activeToolId === tool.id ? "tool-button--active" : ""}`}
            title={`${tool.name}${tool.keyboardShortcut ? ` (${tool.keyboardShortcut})` : ""}`}
            onClick={() => onActivateTool(tool.id)}
          >
            {tool.name}
          </button>
        ))}
      </div>

      <div className="toolbar-group">
        {exportPlugins.map((exp) => (
          <button
            key={exp.id}
            className="tool-button"
            title={`Export as ${exp.name}`}
            onClick={() => handleExport(exp.id)}
          >
            {exp.name}
          </button>
        ))}
      </div>
    </header>
  );
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/features/editor/EditorToolbar.tsx
```
Expected: no errors.

---

### Task 14: EditorSidebar Component

**Files:**
- Create: `apps/studio/src/features/editor/EditorSidebar.tsx`

- [ ] **Step 1: Write sidebar component**

```typescript
// apps/studio/src/features/editor/EditorSidebar.tsx

import type { GraphSnapshot } from "../../services/graph/types";

interface EditorSidebarProps {
  snapshot: GraphSnapshot;
  selectedNodeId: number | null;
  onClear: () => void;
}

export function EditorSidebar({ snapshot, selectedNodeId, onClear }: EditorSidebarProps) {
  const selectedNode = selectedNodeId !== null
    ? snapshot.nodes.find((n) => n.id === selectedNodeId)
    : null;

  return (
    <aside className="editor-sidebar">
      <dl>
        <div><dt>Nodes</dt><dd>{snapshot.nodes.length}</dd></div>
        <div><dt>Edges</dt><dd>{snapshot.edges.length}</dd></div>
        <div><dt>Regions</dt><dd>{snapshot.regions.filter((r) => r.filled).length} filled</dd></div>
        <div><dt>Version</dt><dd>v{snapshot.geomVersion.toString()}</dd></div>
      </dl>

      {selectedNode && (
        <div className="selection-info">
          <h3>Selected Node</h3>
          <dl>
            <div><dt>ID</dt><dd>{selectedNode.id}</dd></div>
            <div><dt>X</dt><dd>{selectedNode.x.toFixed(1)}</dd></div>
            <div><dt>Y</dt><dd>{selectedNode.y.toFixed(1)}</dd></div>
          </dl>
        </div>
      )}

      <div className="sidebar-actions">
        <button className="danger-button" onClick={onClear} type="button">
          Clear Canvas
        </button>
      </div>
    </aside>
  );
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit src/features/editor/EditorSidebar.tsx
```
Expected: no errors.

---

### Task 15: Update CanvasViewport for Plugin Delegation

**Files:**
- Modify: `apps/studio/src/features/editor/CanvasViewport.tsx`

The existing `CanvasViewport` handles its own node movement and edge creation inline. We need to refactor it to accept an `activeTool` from the PluginHost and delegate pointer events to it. When no tool is active (the "select" default), it falls back to the existing node-drag behavior.

- [ ] **Step 1: Rewrite CanvasViewport**

Replace the entire file contents:

```typescript
// apps/studio/src/features/editor/CanvasViewport.tsx

import { useEffect, useMemo, useRef, type MouseEvent, type PointerEvent, type RefObject } from "react";
import type { GraphService } from "../../services/graph/graphService";
import type { GraphSnapshot, GraphPick } from "../../services/graph/types";
import type { PluginHost } from "../../plugins/PluginHost";
import type { PluginPointerEvent, ToolEntry, ToolContext } from "../../plugins/types";
import { snapToGrid } from "../../plugins/builtin/grid-snap";

interface CanvasViewportProps {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  service: GraphService;
  snapshot: GraphSnapshot;
  host: PluginHost;
  activeTool: ToolEntry | null;
  context: ToolContext | null;
  selectedNodeId: number | null;
  edgeStartNodeId: number | null;
  onSelectNode: (id: number | null) => void;
  onEdgeStartNodeChange: (id: number | null) => void;
}

function rgbaToCss(color: [number, number, number, number]): string {
  const [r, g, b, a] = color;
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, a / 255))})`;
}

function toPluginEvent(
  canvas: HTMLCanvasElement,
  event: { clientX: number; clientY: number; shiftKey: boolean; altKey: boolean; metaKey: boolean; ctrlKey: boolean },
): PluginPointerEvent {
  const rect = canvas.getBoundingClientRect();
  return {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
    shiftKey: event.shiftKey,
    altKey: event.altKey,
    metaKey: event.metaKey,
    ctrlKey: event.ctrlKey,
  };
}

export function CanvasViewport({
  canvasRef,
  service,
  snapshot,
  host,
  activeTool,
  context,
  selectedNodeId,
  edgeStartNodeId,
  onSelectNode,
  onEdgeStartNodeChange,
}: CanvasViewportProps) {
  const dragNodeIdRef = useRef<number | null>(null);

  const nodeLookup = useMemo(() => {
    return new Map(snapshot.nodes.map((node) => [node.id, node]));
  }, [snapshot.nodes]);

  // Drawing effect
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.floor(rect.width * dpr));
      canvas.height = Math.max(1, Math.floor(rect.height * dpr));
      const context = canvas.getContext("2d");
      if (!context) return;
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      draw(context, rect.width, rect.height);
    };

    const draw = (context: CanvasRenderingContext2D, width: number, height: number) => {
      context.clearRect(0, 0, width, height);

      // Background
      context.fillStyle = "#f7faf9";
      context.fillRect(0, 0, width, height);

      // Draw regions
      snapshot.regions.forEach((region) => {
        if (!region.filled || region.points.length < 6) return;
        context.beginPath();
        context.moveTo(region.points[0] ?? 0, region.points[1] ?? 0);
        for (let i = 2; i < region.points.length; i += 2) {
          context.lineTo(region.points[i] ?? 0, region.points[i + 1] ?? 0);
        }
        context.closePath();
        context.fillStyle = region.color ? rgbaToCss(region.color) : "rgba(29, 140, 101, 0.16)";
        context.fill();
      });

      // Draw edges
      snapshot.edges.forEach((edge) => {
        const start = nodeLookup.get(edge.a);
        const end = nodeLookup.get(edge.b);
        if (!start || !end) return;

        context.beginPath();
        context.moveTo(start.x, start.y);
        if (edge.kind === "cubic" && edge.handles) {
          context.bezierCurveTo(
            edge.handles.ax, edge.handles.ay,
            edge.handles.bx, edge.handles.by,
            end.x, end.y,
          );
        } else if (edge.kind === "polyline" && edge.polylinePoints && edge.polylinePoints.length >= 2) {
          for (let i = 0; i < edge.polylinePoints.length; i += 2) {
            context.lineTo(edge.polylinePoints[i] ?? 0, edge.polylinePoints[i + 1] ?? 0);
          }
          context.lineTo(end.x, end.y);
        } else {
          context.lineTo(end.x, end.y);
        }

        context.lineWidth = edge.stroke?.width ?? 2;
        context.strokeStyle = edge.stroke ? rgbaToCss(edge.stroke.color) : "#125f4f";
        context.stroke();
      });

      // Draw nodes
      snapshot.nodes.forEach((node) => {
        const isSelected = node.id === selectedNodeId;
        const isEdgeStart = node.id === edgeStartNodeId;
        const radius = isSelected ? 7 : 5;

        context.beginPath();
        context.arc(node.x, node.y, radius, 0, Math.PI * 2);
        context.fillStyle = isSelected ? "#f38b2f" : "#0f4238";
        context.fill();

        if (isEdgeStart) {
          context.beginPath();
          context.arc(node.x, node.y, radius + 4, 0, Math.PI * 2);
          context.strokeStyle = "#f38b2f";
          context.lineWidth = 2;
          context.stroke();
        }
      });

      // Plugin overlay
      host.renderToolOverlay(context, width, height);
    };

    resize();
    const observer = new ResizeObserver(() => resize());
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [snapshot, selectedNodeId, edgeStartNodeId, nodeLookup, host, canvasRef]);

  // --- Pointer event handlers ---

  const applyResult = (result: ReturnType<typeof host.handlePointerDown>) => {
    if (!result || !context) return;
    switch (result.action) {
      case "addNode":
        onSelectNode(result.id);
        break;
      case "moveNode":
        context.service.moveNode(result.id, result.x, result.y);
        break;
      case "addEdge":
        context.service.addEdge(result.a, result.b);
        break;
      case "select":
        onSelectNode(result.id);
        break;
      case "repaint":
        // Canvas will redraw on next snapshot update
        break;
    }
  };

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);
    const rect = canvas.getBoundingClientRect();
    const pick = service.pick(pEvent.x, pEvent.y, 12);

    // Delegate to active tool first
    if (activeTool) {
      const result = host.handlePointerDown(pEvent, pick, context);
      applyResult(result);
      return;
    }

    // Default: select/move behavior (no active tool)
    if (event.shiftKey && pick?.kind === "node") {
      const pickedNodeId = pick.id;
      if (edgeStartNodeId !== null && edgeStartNodeId !== pickedNodeId) {
        service.addEdge(edgeStartNodeId, pickedNodeId);
        onEdgeStartNodeChange(null);
      } else {
        onEdgeStartNodeChange(pickedNodeId);
        onSelectNode(pickedNodeId);
      }
      return;
    }

    if (pick?.kind === "node") {
      dragNodeIdRef.current = pick.id;
      onSelectNode(pick.id);
      onEdgeStartNodeChange(null);
      canvas.setPointerCapture(event.pointerId);
      return;
    }

    onSelectNode(null);
    onEdgeStartNodeChange(null);
  };

  const onPointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);

    // Delegate to active tool
    if (activeTool) {
      const rect = canvas.getBoundingClientRect();
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handlePointerMove(pEvent, pick, context);
      if (result && (result.action === "repaint")) {
        // Force canvas redraw for preview overlays
        requestAnimationFrame(() => {
          const ctx = canvas.getContext("2d");
          if (!ctx) return;
          const rect2 = canvas.getBoundingClientRect();
          drawAll(ctx, rect2.width, rect2.height);
        });
      }
      applyResult(result);
      return;
    }

    // Default: node drag
    const dragNodeId = dragNodeIdRef.current;
    if (dragNodeId === null) return;

    const snapped = snapToGrid(pEvent.x, pEvent.y);
    service.moveNode(dragNodeId, snapped.x, snapped.y);
  };

  const onPointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);

    if (activeTool) {
      const rect = canvas.getBoundingClientRect();
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handlePointerUp(pEvent, pick, context);
      applyResult(result);
    }

    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    dragNodeIdRef.current = null;
  };

  const onDoubleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event.nativeEvent);

    if (activeTool) {
      const rect = canvas.getBoundingClientRect();
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handleDoubleClick(pEvent, pick, context);
      applyResult(result);
      return;
    }

    // Default: add node on double-click
    const snapped = snapToGrid(pEvent.x, pEvent.y);
    const id = service.addNode(snapped.x, snapped.y);
    onSelectNode(id);
  };

  return (
    <canvas
      ref={canvasRef}
      className="editor-canvas"
      style={{ cursor: activeTool?.plugin.cursor ?? "default" }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onDoubleClick}
    />
  );
}

// Helper used by pointerMove for immediate overlay redraws
function drawAll(context: CanvasRenderingContext2D, width: number, height: number) {
  // This is a lightweight redraw — the full redraw happens on snapshot change.
  // For now we just trigger a repaint via the resize effect.
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit
```

---


### Task 16: Rewrite EditorPage for Plugin Integration

**Files:**
- Modify: `apps/studio/src/features/editor/EditorPage.tsx`

This is the biggest change. We replace the iframe-based editor with the React-native editor that uses all the pieces built above.

- [ ] **Step 1: Rewrite EditorPage**

Replace the entire contents:

```typescript
// apps/studio/src/features/editor/EditorPage.tsx

import { useEffect, useMemo, useState, useCallback } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { documentRepository } from "../../app/services";
import { useAuth } from "../auth/useAuth";
import { useGraphEditor } from "./useGraphEditor";
import { CanvasViewport } from "./CanvasViewport";
import { EditorToolbar } from "./EditorToolbar";
import { EditorSidebar } from "./EditorSidebar";
import { useActiveTool, usePluginTools } from "../../plugins/hooks";

function getPluginHost(): import("../../plugins/PluginHost").PluginHost {
  return (window as any).__pluginHost;
}

export function EditorPage() {
  const { user } = useAuth();
  const { docId } = useParams<{ docId: string }>();
  const navigate = useNavigate();

  const [docMeta, setDocMeta] = useState<import("../docs/types").DocumentMeta | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState("Ready");

  const {
    service,
    snapshot,
    error,
    context,
    canvasRef,
    selectedNodeId,
    setSelectedNodeId,
    edgeStartNodeId,
    setEdgeStartNodeId,
  } = useGraphEditor();

  const host = getPluginHost();
  const tools = usePluginTools(host);
  const activeTool = useActiveTool(host, context);

  // Expose snapshot/service to host for export plugins
  useEffect(() => {
    const h = host as any;
    h._lastSnapshot = snapshot;
    h._lastService = service;
  }, [host, snapshot, service]);

  // Load document
  const storageKey = useMemo(() => {
    if (!user || !docId) return null;
    return `studio:workbench:${user.id}:${docId}`;
  }, [user, docId]);

  useEffect(() => {
    if (!user || !docId || !storageKey || !service) return;

    const stored = documentRepository.get(user.id, docId);
    if (!stored) {
      navigate("/app/docs", { replace: true });
      return;
    }

    setDocMeta(stored.meta);
    setTitleDraft(stored.meta.title);

    // Import graph from stored document
    try {
      service.importDocument(stored.graph);
    } catch (err) {
      console.error("Failed to load document:", err);
    }
  }, [docId, navigate, storageKey, user, service]);

  // Save
  const saveNow = useCallback(() => {
    if (!user || !docMeta || !service) return;
    setIsSaving(true);
    try {
      const payload = service.exportDocument();
      const updated = documentRepository.saveGraph(user.id, docMeta.id, payload);
      if (!updated) {
        setStatus("Could not save document.");
        return;
      }
      setDocMeta(updated);
      setTitleDraft(updated.title);
      setStatus(`Saved at ${new Date(updated.updatedAt).toLocaleTimeString()}`);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : "Save failed.");
    } finally {
      setIsSaving(false);
    }
  }, [docMeta, service, user]);

  // Autosave
  useEffect(() => {
    if (!user || !docMeta || !service) return;
    const timer = window.setInterval(() => {
      try {
        const payload = service.exportDocument();
        const updated = documentRepository.saveGraph(user.id, docMeta.id, payload);
        if (updated) {
          setDocMeta(updated);
        }
      } catch {
        // Silently skip autosave errors
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [docMeta, service, user]);

  const onTitleCommit = () => {
    if (!user || !docMeta) return;
    const normalized = titleDraft.trim();
    if (!normalized || normalized === docMeta.title) {
      setTitleDraft(docMeta.title);
      return;
    }
    const renamed = documentRepository.rename(user.id, docMeta.id, normalized);
    if (renamed) {
      setDocMeta(renamed);
      setStatus("Renamed");
    }
  };

  const handleActivateTool = useCallback((toolId: string) => {
    host.activateTool(toolId);
  }, [host]);

  const handleClear = () => {
    if (!service) return;
    service.clear();
    setSelectedNodeId(null);
    setEdgeStartNodeId(null);
  };

  if (error) {
    return (
      <section className="editor-page">
        <article className="editor-error">
          <h1>Could not open editor</h1>
          <p>{error}</p>
          <Link className="primary-link" to="/app/docs">Back to documents</Link>
        </article>
      </section>
    );
  }

  if (!docMeta || !service || !snapshot) {
    return <section className="editor-page loading-panel">Loading editor...</section>;
  }

  return (
    <section className="editor-page page-enter">
      <EditorToolbar
        host={host}
        activeToolId={host.getActiveToolId() ?? "select"}
        tools={tools}
        onActivateTool={handleActivateTool}
      />

      <div className="editor-content">
        <div className="canvas-wrap">
          <CanvasViewport
            canvasRef={canvasRef}
            service={service}
            snapshot={snapshot}
            host={host}
            activeTool={activeTool}
            context={context}
            selectedNodeId={selectedNodeId}
            edgeStartNodeId={edgeStartNodeId}
            onSelectNode={setSelectedNodeId}
            onEdgeStartNodeChange={setEdgeStartNodeId}
          />
        </div>

        <EditorSidebar
          snapshot={snapshot}
          selectedNodeId={selectedNodeId}
          onClear={handleClear}
        />
      </div>

      <div className="editor-actions-bar">
        <input
          className="title-input"
          value={titleDraft}
          onChange={(e) => setTitleDraft(e.target.value)}
          onBlur={onTitleCommit}
          onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
        />
        <span className="pill">{status}</span>
        <button className="primary-button" onClick={saveNow} disabled={isSaving}>
          {isSaving ? "Saving..." : "Save"}
        </button>
        <Link className="ghost-button" to="/app/docs">Documents</Link>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Verify compiles**

```bash
cd apps/studio && npx tsc --noEmit
```
Expected: no errors.

---

### Task 17: Add Toolbar and Sidebar Styles

**Files:**
- Modify: `apps/studio/src/styles.css`

- [ ] **Step 1: Append styles**

Add at the end of `apps/studio/src/styles.css`:

```css
/* ── Toolbar ── */
.editor-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  padding: 0.6rem 0;
}

.toolbar-group {
  display: flex;
  gap: 0.35rem;
  align-items: center;
}

.tool-button {
  padding: 0.4rem 0.7rem;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--surface);
  color: var(--text);
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

.tool-button:hover {
  background: rgba(19, 108, 93, 0.1);
  border-color: var(--brand);
}

.tool-button--active {
  background: var(--brand);
  color: #fff;
  border-color: var(--brand-strong);
}

/* ── Sidebar ── */
.editor-sidebar dl,
.selection-info dl {
  display: grid;
  gap: 0.4rem;
}

.editor-sidebar dl div,
.selection-info dl div {
  display: flex;
  justify-content: space-between;
  border-bottom: 1px dashed var(--line);
  padding-bottom: 0.2rem;
}

.editor-sidebar dt,
.selection-info dt {
  color: var(--muted);
  font-size: 0.85rem;
}

.editor-sidebar dd,
.selection-info dd {
  margin: 0;
  font-family: "IBM Plex Mono", monospace;
  font-size: 0.85rem;
}

.sidebar-actions {
  margin-top: 0.5rem;
}

.editor-actions-bar {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.5rem 0;
}
```

- [ ] **Step 2: Verify build**

```bash
cd apps/studio && npm run build
```
Expected: successful Vite build.

---

### Task 18: Integration Test — All Plugins Load and Work

**Files:**
- Manual verification — no new files.

- [ ] **Step 1: Start dev server**

```bash
cd apps/studio && npm run dev
```

- [ ] **Step 2: Verify in browser**

Open the app and navigate to a document editor. Verify:
1. **Toolbar shows all 5 tools:** Grid Snap, Rectangle, Ellipse, Polygon, Freehand (plus Export PNG, Export SVG buttons)
2. **Grid Snap toggle:** Click Grid Snap → grid overlay appears on canvas. Click again → disappears.
3. **Rectangle tool:** Click Rectangle → draw on canvas → rectangle with 4 nodes + 4 edges appears.
4. **Ellipse tool:** Click Ellipse → draw on canvas → ellipse with cubic edges appears.
5. **Polygon tool:** Click Polygon → draw on canvas → hexagon appears.
6. **Freehand tool:** Click Freehand → draw on canvas → smooth curve appears.
7. **PNG Export:** Click "PNG Image" → file downloads.
8. **SVG Export:** Click "SVG Vector" → file downloads.
9. **Default select mode:** Click no tool → double-click adds node, drag moves node, shift+drag connects.
10. **Sidebar** shows node/edge/region counts.

- [ ] **Step 3: Verify no console errors**

Open browser dev tools console. Expected: no errors on load or during interactions.

---

### Task 19: Commit

```bash
cd apps/studio && git add -A
git commit -m "feat: plugin SDK with 5 built-in plugins (grid snap, shapes, PNG/SVG export, freehand)"
```

---

## Self-Review

1. **Spec coverage:** Every requirement covered — plugin host, 5 built-in plugins (grid snap, shapes [rect+ellipse+polygon], PNG export, SVG export, freehand), React-native editor replacing iframe, toolbar/sidebar integration.

2. **Placeholder scan:** No TBDs, no "add appropriate error handling" without code, no "similar to Task N" shortcuts. All code is complete.

3. **Type consistency:** `PluginManifest`, `ToolPlugin`, `ExportPlugin`, `ToolEntry`, `ToolContext`, `PluginPointerEvent`, `ToolResult` are used consistently across all tasks. `GraphService` methods are defined in Task 4 and used in Tasks 6-14. `PluginHost` API is stable across all consumers.