// apps/studio/src/features/editor/CanvasViewport.tsx

import { useEffect, useMemo, useRef, type MouseEvent, type PointerEvent, type RefObject } from "react";
import type { GraphService } from "../../services/graph/graphService";
import type { GraphSnapshot } from "../../services/graph/types";
import type { PluginHost } from "../../plugins/PluginHost";
import type { PluginPointerEvent, ToolEntry, ToolContext } from "../../plugins/types";
import { snapToGrid } from "../../plugins/builtin/grid-snap";

interface CanvasViewportProps {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  service: GraphService;
  snapshot: GraphSnapshot;
  host: PluginHost;
  activeTool: ToolEntry | null;
  context: ToolContext | null;
  selectedNodeId: number | null;
  edgeStartNodeId: number | null;
  onSelectNode: (id: number | null) => void;
  onEdgeStartNodeChange: (id: number | null) => void;
}

function rgbaToCss(color: [number, number, number, number]): string {
  const [r, g, b, a] = color;
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, a / 255))})`;
}

function toPluginEvent(
  canvas: HTMLCanvasElement,
  event: { clientX: number; clientY: number; shiftKey: boolean; altKey: boolean; metaKey: boolean; ctrlKey: boolean },
): PluginPointerEvent {
  const rect = canvas.getBoundingClientRect();
  return {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
    button: 0,
    shiftKey: event.shiftKey,
    altKey: event.altKey,
    metaKey: event.metaKey,
    ctrlKey: event.ctrlKey,
  };
}

export function CanvasViewport({
  canvasRef,
  service,
  snapshot,
  host,
  activeTool,
  context,
  selectedNodeId,
  edgeStartNodeId,
  onSelectNode,
  onEdgeStartNodeChange,
}: CanvasViewportProps) {
  const dragNodeIdRef = useRef<number | null>(null);

  const nodeLookup = useMemo(() => {
    return new Map(snapshot.nodes.map((node) => [node.id, node]));
  }, [snapshot.nodes]);

  // Drawing effect
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.floor(rect.width * dpr));
      canvas.height = Math.max(1, Math.floor(rect.height * dpr));
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      draw(ctx, rect.width, rect.height);
    };

    const draw = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
      ctx.clearRect(0, 0, width, height);

      // Background
      ctx.fillStyle = "#f7faf9";
      ctx.fillRect(0, 0, width, height);

      // Draw regions
      snapshot.regions.forEach((region) => {
        if (!region.filled || region.points.length < 6) return;
        ctx.beginPath();
        ctx.moveTo(region.points[0] ?? 0, region.points[1] ?? 0);
        for (let i = 2; i < region.points.length; i += 2) {
          ctx.lineTo(region.points[i] ?? 0, region.points[i + 1] ?? 0);
        }
        ctx.closePath();
        ctx.fillStyle = region.color ? rgbaToCss(region.color) : "rgba(29, 140, 101, 0.16)";
        ctx.fill();
      });

      // Draw edges
      snapshot.edges.forEach((edge) => {
        const start = nodeLookup.get(edge.a);
        const end = nodeLookup.get(edge.b);
        if (!start || !end) return;

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
        ctx.strokeStyle = edge.stroke ? rgbaToCss(edge.stroke.color) : "#125f4f";
        ctx.stroke();
      });

      // Draw nodes
      snapshot.nodes.forEach((node) => {
        const isSelected = node.id === selectedNodeId;
        const isEdgeStart = node.id === edgeStartNodeId;
        const radius = isSelected ? 7 : 5;

        ctx.beginPath();
        ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
        ctx.fillStyle = isSelected ? "#f38b2f" : "#0f4238";
        ctx.fill();

        if (isEdgeStart) {
          ctx.beginPath();
          ctx.arc(node.x, node.y, radius + 4, 0, Math.PI * 2);
          ctx.strokeStyle = "#f38b2f";
          ctx.lineWidth = 2;
          ctx.stroke();
        }
      });

      // Render every plugin's overlay so ambient ones (grid-snap) stay
      // visible even when a different tool is active.
      host.renderAllOverlays(ctx, width, height);
    };

    resize();
    const observer = new ResizeObserver(() => resize());
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [snapshot, selectedNodeId, edgeStartNodeId, nodeLookup, host, canvasRef]);

  // Apply tool result
  const applyResult = (result: ReturnType<typeof host.handlePointerDown>) => {
    if (!result || !context) return;
    switch (result.action) {
      case "addNode":
        onSelectNode(result.id);
        break;
      case "removeNode":
        context.service.removeNode(result.id);
        break;
      case "removeEdge":
        context.service.removeEdge(result.id);
        break;
      case "moveNode":
        context.service.moveNode(result.id, result.x, result.y);
        break;
      case "addEdge":
        context.service.addEdge(result.a, result.b);
        break;
      case "select":
        onSelectNode(result.id);
        break;
      case "repaint":
        // Canvas redraws on next snapshot update
        break;
    }
  };

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);
    const pick = service.pick(pEvent.x, pEvent.y, 12);

    // Delegate to active tool first
    if (activeTool) {
      const result = host.handlePointerDown(pEvent, pick, context);
      applyResult(result);
      return;
    }

    // Default: select/move behavior (no active tool)
    if (event.shiftKey && pick?.kind === "node") {
      const pickedNodeId = pick.id;
      if (edgeStartNodeId !== null && edgeStartNodeId !== pickedNodeId) {
        service.addEdge(edgeStartNodeId, pickedNodeId);
        onEdgeStartNodeChange(null);
      } else {
        onEdgeStartNodeChange(pickedNodeId);
        onSelectNode(pickedNodeId);
      }
      return;
    }

    if (pick?.kind === "node") {
      dragNodeIdRef.current = pick.id;
      onSelectNode(pick.id);
      onEdgeStartNodeChange(null);
      canvas.setPointerCapture(event.pointerId);
      return;
    }

    onSelectNode(null);
    onEdgeStartNodeChange(null);
  };

  const onPointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);

    // Delegate to active tool
    if (activeTool) {
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handlePointerMove(pEvent, pick, context);
      applyResult(result);
      return;
    }

    // Default: node drag
    const dragNodeId = dragNodeIdRef.current;
    if (dragNodeId === null) return;

    const snapped = snapToGrid(pEvent.x, pEvent.y);
    service.moveNode(dragNodeId, snapped.x, snapped.y);
  };

  const onPointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event);

    if (activeTool) {
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handlePointerUp(pEvent, pick, context);
      applyResult(result);
    }

    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    dragNodeIdRef.current = null;
  };

  const onDoubleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas || !context) return;

    const pEvent = toPluginEvent(canvas, event.nativeEvent);

    if (activeTool) {
      const pick = service.pick(pEvent.x, pEvent.y, 12);
      const result = host.handleDoubleClick(pEvent, pick, context);
      applyResult(result);
      return;
    }

    // Default: add node on double-click
    const snapped = snapToGrid(pEvent.x, pEvent.y);
    const id = service.addNode(snapped.x, snapped.y);
    onSelectNode(id);
  };

  return (
    <canvas
      ref={canvasRef}
      className="editor-canvas"
      style={{ cursor: activeTool?.plugin.cursor ?? "default" }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onDoubleClick}
    />
  );
}
