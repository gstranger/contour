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

    renderOverlay(_ctx: CanvasRenderingContext2D, _width: number, _height: number) {
      // Preview rendering is handled by the CanvasViewport redraw loop.
      // We don't have access to live mouse position here, so overlay is a noop.
    },

    onPointerDown(event: PluginPointerEvent): ToolResult {
      state.dragging = true;
      state.startX = event.x;
      state.startY = event.y;
      return { action: "repaint" };
    },

    onPointerMove(): ToolResult {
      if (state.dragging) {
        return { action: "repaint" };
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

      if (w < 3 && h < 3) return null; // too small — ignore

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
        const dAngle = (Math.PI * 2) / segments;
        // Standard cubic Bézier circle approximation constant:
        // handle length = (4/3) * tan(arc_angle/4)
        const handleLen = (4.0 / 3.0) * Math.tan(dAngle / 4.0);

        // Create nodes evenly spaced around the ellipse
        const ids: number[] = [];
        for (let i = 0; i < segments; i++) {
          const angle = i * dAngle;
          ids.push(
            ctx.service.addNode(
              cx + rx * Math.cos(angle),
              cy + ry * Math.sin(angle),
            ),
          );
        }

        // Connect with cubic edges
        for (let i = 0; i < segments; i++) {
          const j = (i + 1) % segments;
          const edgeId = ctx.service.addEdge(ids[i], ids[j]);

          const a1 = i * dAngle;
          const a2 = j * dAngle;

          // Absolute node positions
          const nx1 = cx + rx * Math.cos(a1);
          const ny1 = cy + ry * Math.sin(a1);
          const nx2 = cx + rx * Math.cos(a2);
          const ny2 = cy + ry * Math.sin(a2);

          // Tangent at angle a on ellipse (rx*cos(a), ry*sin(a)) is (-rx*sin(a), ry*cos(a)).
          // Control point 1 extends from start node in the tangent direction.
          const p1x = nx1 - rx * Math.sin(a1) * handleLen;
          const p1y = ny1 + ry * Math.cos(a1) * handleLen;

          // Control point 2 extends backward from end node (approaching from tangent).
          const p2x = nx2 + rx * Math.sin(a2) * handleLen;
          const p2y = ny2 - ry * Math.cos(a2) * handleLen;

          // setEdgeCubic takes absolute world coordinates for both control points.
          ctx.service.setEdgeCubic(edgeId, p1x, p1y, p2x, p2y);
        }

        return { action: "select", id: null };
      }

      if (state.mode === "polygon") {
        const cx = (x1 + x2) / 2;
        const cy = (y1 + y2) / 2;
        const radius = Math.min(Math.abs(w), Math.abs(h)) / 2;
        const sides = 6;

        const ids: number[] = [];
        for (let i = 0; i < sides; i++) {
          const angle = (i / sides) * Math.PI * 2 - Math.PI / 2;
          ids.push(
            ctx.service.addNode(
              cx + radius * Math.cos(angle),
              cy + radius * Math.sin(angle),
            ),
          );
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