// apps/studio/src/features/editor/useGraphEditor.ts

import { useEffect, useRef, useState, useCallback } from "react";
import { createGraphService, type GraphService } from "../../services/graph/graphService";
import type { GraphSnapshot, GraphPick, ToolContext } from "../../services/graph/types";

export function useGraphEditor() {
  const [service, setService] = useState<GraphService | null>(null);
  const [snapshot, setSnapshot] = useState<GraphSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<number | null>(null);
  const [edgeStartNodeId, setEdgeStartNodeId] = useState<number | null>(null);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    createGraphService().then((svc) => {
      if (cancelled) {
        svc.dispose();
        return;
      }
      setService(svc);
    }).catch((err) => {
      setError(err instanceof Error ? err.message : "Failed to initialize WASM");
    });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!service) return;
    const unsubscribe = service.subscribe(setSnapshot);
    return unsubscribe;
  }, [service]);

  const context: ToolContext | null = service && snapshot
    ? { service, snapshot }
    : null;

  const pick = useCallback((clientX: number, clientY: number): GraphPick | null => {
    const canvas = canvasRef.current;
    if (!canvas || !service) return null;
    const rect = canvas.getBoundingClientRect();
    const x = clientX - rect.left;
    const y = clientY - rect.top;
    return service.pick(x, y, 12);
  }, [service]);

  return {
    service,
    snapshot,
    error,
    context,
    canvasRef,
    selectedNodeId,
    setSelectedNodeId,
    edgeStartNodeId,
    setEdgeStartNodeId,
    pick,
  };
}
