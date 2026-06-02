import type { GraphDocument } from "vecnet-wasm";

export type EdgeKind = "line" | "cubic" | "polyline";

export interface GraphNodeView {
  id: number;
  x: number;
  y: number;
}

export interface GraphEdgeView {
  id: number;
  a: number;
  b: number;
  kind: EdgeKind;
  stroke: {
    color: [number, number, number, number];
    width: number;
  } | null;
  handles?: {
    ax: number;
    ay: number;
    bx: number;
    by: number;
  };
  polylinePoints?: number[];
}

export interface GraphRegionView {
  key: number;
  area: number;
  filled: boolean;
  color?: [number, number, number, number];
  points: number[];
}

export interface GraphSnapshot {
  geomVersion: number;
  nodes: GraphNodeView[];
  edges: GraphEdgeView[];
  regions: GraphRegionView[];
}

export type GraphDocumentPayload = GraphDocument;

/** Pick result from canvas picking */
export type GraphPick = { kind: "node"; id: number; dist: number }
  | { kind: "edge"; id: number; t: number; dist: number }
  | { kind: "handle"; edge: number; end: number; dist: number }
  | null;

export interface BBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Context passed to tool plugins */
export interface ToolContext {
  service: import("../graph/graphService").GraphService;
  snapshot: GraphSnapshot;
}