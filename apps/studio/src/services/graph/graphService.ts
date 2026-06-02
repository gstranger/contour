import initWasm, {
  Graph,
  type Err,
  type GraphDocument,
  type HandleTuple,
  type Ok,
  type Result,
} from "vecnet-wasm";
import type { GraphDocumentPayload, GraphEdgeView, GraphPick, GraphSnapshot } from "./types";

type SnapshotListener = (snapshot: GraphSnapshot) => void;

let wasmInitPromise: Promise<void> | null = null;

function ensureWasmReady(): Promise<void> {
  if (!wasmInitPromise) {
    wasmInitPromise = initWasm().then(() => undefined);
  }
  return wasmInitPromise;
}

function unwrapResult<T>(result: Result<T>, context: string): T {
  if (result.ok) {
    return (result as Ok<T>).value;
  }
  const error = (result as Err).error;
  throw new Error(`${context}: ${error.code} - ${error.message}`);
}

function toEdgeKind(kind: number): GraphEdgeView["kind"] {
  if (kind === 1) {
    return "cubic";
  }
  if (kind === 2) {
    return "polyline";
  }
  return "line";
}

function tupleToHandles(tuple: HandleTuple): NonNullable<GraphEdgeView["handles"]> {
  return {
    ax: tuple[0],
    ay: tuple[1],
    bx: tuple[2],
    by: tuple[3],
  };
}

export class GraphService {
  private readonly listeners = new Set<SnapshotListener>();
  private readonly graph: Graph;

  constructor(graph: Graph) {
    this.graph = graph;
  }

  dispose(): void {
    this.listeners.clear();
    this.graph.free();
  }

  subscribe(listener: SnapshotListener): () => void {
    this.listeners.add(listener);
    listener(this.snapshot());
    return () => {
      this.listeners.delete(listener);
    };
  }

  snapshot(): GraphSnapshot {
    const nodeData = this.graph.get_node_data();
    const edgeData = this.graph.get_edge_data();

    const nodeMap = new Map<number, { x: number; y: number }>();
    const nodes = Array.from(nodeData.ids, (id, index) => {
      const x = nodeData.positions[index * 2] ?? 0;
      const y = nodeData.positions[index * 2 + 1] ?? 0;
      nodeMap.set(id, { x, y });
      return { id, x, y };
    });

    const edges = Array.from(edgeData.ids, (id, index) => {
      const a = edgeData.endpoints[index * 2] ?? 0;
      const b = edgeData.endpoints[index * 2 + 1] ?? 0;
      const kind = toEdgeKind(edgeData.kinds[index] ?? 0);
      const width = edgeData.stroke_widths[index] ?? 0;
      const colorOffset = index * 4;
      const rgba: [number, number, number, number] = [
        edgeData.stroke_rgba[colorOffset] ?? 0,
        edgeData.stroke_rgba[colorOffset + 1] ?? 0,
        edgeData.stroke_rgba[colorOffset + 2] ?? 0,
        edgeData.stroke_rgba[colorOffset + 3] ?? 0,
      ];

      const edge: GraphEdgeView = {
        id,
        a,
        b,
        kind,
        stroke: width > 0 ? { color: rgba, width } : null,
      };

      if (kind === "cubic") {
        const handles = this.graph.get_handles(id);
        if (handles) {
          edge.handles = tupleToHandles(handles);
        }
      }

      if (kind === "polyline") {
        const points = this.graph.get_polyline_points(id);
        if (points) {
          edge.polylinePoints = Array.from(points);
        }
      }

      if (!nodeMap.has(a) || !nodeMap.has(b)) {
        edge.kind = "line";
      }

      return edge;
    });

    const regions = this.graph.get_regions().map((region) => ({
      key: region.key,
      area: region.area,
      filled: region.filled,
      color: region.color,
      points: [...region.points],
    }));

    return {
      geomVersion: this.graph.geom_version(),
      nodes,
      edges,
      regions,
    };
  }

  exportDocument(): GraphDocumentPayload {
    return this.graph.to_json();
  }

  importDocument(document: GraphDocument): void {
    unwrapResult(this.graph.from_json_res(document), "Failed to load document");
    this.emit();
  }

  addNode(x: number, y: number): number {
    const id = unwrapResult(this.graph.add_node_res(x, y), "Failed to add node");
    this.emit();
    return id;
  }

  moveNode(id: number, x: number, y: number): void {
    unwrapResult(this.graph.move_node_res(id, x, y), "Failed to move node");
    this.emit();
  }

  addEdge(a: number, b: number): number {
    const id = unwrapResult(this.graph.add_edge_res(a, b), "Failed to add edge");
    this.emit();
    return id;
  }

  pick(x: number, y: number, tolerance: number): GraphPick | null {
    return unwrapResult(this.graph.pick_res(x, y, tolerance), "Failed to pick");
  }

  toggleRegion(key: number): boolean {
    const changed = unwrapResult(this.graph.toggle_region_res(key), "Failed to toggle region fill");
    this.emit();
    return changed;
  }

  setFlattenTolerance(value: number): void {
    unwrapResult(this.graph.set_flatten_tolerance_res(value), "Failed to set flatten tolerance");
    this.emit();
  }

  private emit(): void {
    const snapshot = this.snapshot();
    this.listeners.forEach((listener) => listener(snapshot));
  }
}

export async function createGraphService(): Promise<GraphService> {
  await ensureWasmReady();
  return new GraphService(new Graph());
}
