import type { GraphSnapshot, GraphPick } from "../services/graph/types";
import type { GraphService } from "../services/graph/graphService";

/** Context passed to tool plugins when activated */
export interface ToolContext {
  service: GraphService;
  snapshot: GraphSnapshot;
}

/** Normalized pointer event for tool plugins */
export interface PluginPointerEvent {
  x: number;
  y: number;
  /** PointerEvent.button: 0=primary, 1=auxiliary, 2=secondary */
  button: number;
  /** Original DOM event for shift/alt/meta key state */
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
}

/** Result from a tool's pointer handler */
export type ToolResult =
  | { action: "addNode"; id: number }
  | { action: "removeNode"; id: number }
  | { action: "removeEdge"; id: number }
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
