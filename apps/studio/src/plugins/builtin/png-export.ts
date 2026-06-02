// apps/studio/src/plugins/builtin/png-export.ts

import type { ExportPlugin } from "../types";
import type { GraphSnapshot } from "../../services/graph/types";
import type { GraphService } from "../../services/graph/graphService";

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