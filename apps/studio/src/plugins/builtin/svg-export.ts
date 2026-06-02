// apps/studio/src/plugins/builtin/svg-export.ts

import type { ExportPlugin } from "../types";
import type { GraphSnapshot } from "../../services/graph/types";
import type { GraphService } from "../../services/graph/graphService";

export const svgExportPlugin: ExportPlugin = {
  id: "builtin.export.svg",
  name: "SVG Vector",
  extension: "svg",
  mimeType: "image/svg+xml",

  async export(snapshot: GraphSnapshot, service: GraphService): Promise<Blob> {
    const nodeLookup = new Map(snapshot.nodes.map((n) => [n.id, n]));

    // Try WASM's native SVG export first
    let paths: string[] = [];
    try {
      paths = service.getSvgPaths();
    } catch {
      // Fallback below
    }

    if (paths.length === 0) {
      // Manual fallback: build path data from snapshot
      for (const edge of snapshot.edges) {
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