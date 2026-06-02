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