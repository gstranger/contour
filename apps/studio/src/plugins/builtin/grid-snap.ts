// apps/studio/src/plugins/builtin/grid-snap.ts

import type { ToolPlugin, PluginPointerEvent, ToolResult } from "../types";

const DEFAULT_GRID_SIZE = 20;
const GRID_COLOR = "rgba(16, 94, 66, 0.08)";

/**
 * Shared grid-snap state, persisted across tool switches so the grid stays
 * visible and other tools can read `enabled` to decide whether to snap.
 */
export const gridSnapState = {
  gridSize: DEFAULT_GRID_SIZE,
  enabled: false,
};

export function createGridSnapPlugin(): ToolPlugin {
  return {
    id: "builtin.grid-snap",
    name: "Grid Snap",
    keyboardShortcut: "G",

    renderOverlay(ctx: CanvasRenderingContext2D, width: number, height: number) {
      if (!gridSnapState.enabled) return;

      ctx.save();
      ctx.strokeStyle = GRID_COLOR;
      ctx.lineWidth = 0.5;

      const size = gridSnapState.gridSize;
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

    // Clicking grid-snap toggles the grid on/off. Other tools deactivating us
    // must not clear `enabled` — the grid is a global toggle, not tied to
    // whichever tool happens to be active.
    onActivate() {
      gridSnapState.enabled = !gridSnapState.enabled;
    },

    onPointerDown(_event: PluginPointerEvent): ToolResult {
      return { action: "repaint" };
    },
  };
}

/** Snap (x, y) to the grid using the current grid size. */
export function snapToGrid(x: number, y: number, gridSize?: number): { x: number; y: number } {
  const size = gridSize ?? gridSnapState.gridSize;
  return {
    x: Math.round(x / size) * size,
    y: Math.round(y / size) * size,
  };
}
