import type { GraphDocument, PickResult, RegionData } from "vecnet-wasm";

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
  color: RegionData["color"];
  points: number[];
}

export interface GraphSnapshot {
  geomVersion: bigint;
  nodes: GraphNodeView[];
  edges: GraphEdgeView[];
  regions: GraphRegionView[];
}

export type GraphDocumentPayload = GraphDocument;
export type GraphPick = PickResult;

export interface BBox {
  x: number;
  y: number;
  width: number;
  height: number;
}
