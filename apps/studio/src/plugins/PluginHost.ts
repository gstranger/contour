// apps/studio/src/plugins/PluginHost.ts

import type {
  ExportPlugin,
  PluginManifest,
  PluginPointerEvent,
  ToolContext,
  ToolEntry,
  ToolResult,
} from "./types";
import type { GraphPick } from "../services/graph/types";

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