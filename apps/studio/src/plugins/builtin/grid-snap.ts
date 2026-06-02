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
