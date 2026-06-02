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
