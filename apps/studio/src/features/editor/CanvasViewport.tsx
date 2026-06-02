import { useEffect, useMemo, useRef, type MouseEvent, type PointerEvent } from "react";
import type { GraphService } from "../../services/graph/graphService";
import type { GraphSnapshot } from "../../services/graph/types";

interface CanvasViewportProps {
  service: GraphService;
  snapshot: GraphSnapshot;
  selectedNodeId: number | null;
  edgeStartNodeId: number | null;
  onSelectNode: (id: number | null) => void;
  onEdgeStartNodeChange: (id: number | null) => void;
}

function rgbaToCss(color: [number, number, number, number]): string {
  const [r, g, b, a] = color;
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, a / 255))})`;
}

function getCanvasPoint(canvas: HTMLCanvasElement, event: { clientX: number; clientY: number }) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
  };
}

export function CanvasViewport({
  service,
  snapshot,
  selectedNodeId,
  edgeStartNodeId,
  onSelectNode,
  onEdgeStartNodeChange,
}: CanvasViewportProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const dragNodeIdRef = useRef<number | null>(null);

  const nodeLookup = useMemo(() => {
    return new Map(snapshot.nodes.map((node) => [node.id, node]));
  }, [snapshot.nodes]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.floor(rect.width * dpr));
      canvas.height = Math.max(1, Math.floor(rect.height * dpr));
      const context = canvas.getContext("2d");
      if (!context) {
        return;
      }
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      draw(context, rect.width, rect.height);
    };

    const draw = (context: CanvasRenderingContext2D, width: number, height: number) => {
      context.clearRect(0, 0, width, height);

      context.fillStyle = "#f7faf9";
      context.fillRect(0, 0, width, height);

      context.strokeStyle = "rgba(16, 94, 66, 0.1)";
      context.lineWidth = 1;
      for (let x = 0; x <= width; x += 40) {
        context.beginPath();
        context.moveTo(x, 0);
        context.lineTo(x, height);
        context.stroke();
      }
      for (let y = 0; y <= height; y += 40) {
        context.beginPath();
        context.moveTo(0, y);
        context.lineTo(width, y);
        context.stroke();
      }

      snapshot.regions.forEach((region) => {
        if (!region.filled || region.points.length < 6) {
          return;
        }
        context.beginPath();
        context.moveTo(region.points[0] ?? 0, region.points[1] ?? 0);
        for (let i = 2; i < region.points.length; i += 2) {
          context.lineTo(region.points[i] ?? 0, region.points[i + 1] ?? 0);
        }
        context.closePath();
        context.fillStyle = region.color ? rgbaToCss(region.color) : "rgba(29, 140, 101, 0.16)";
        context.fill();
      });

      snapshot.edges.forEach((edge) => {
        const start = nodeLookup.get(edge.a);
        const end = nodeLookup.get(edge.b);
        if (!start || !end) {
          return;
        }

        context.beginPath();
        context.moveTo(start.x, start.y);
        if (edge.kind === "cubic" && edge.handles) {
          context.bezierCurveTo(
            edge.handles.ax,
            edge.handles.ay,
            edge.handles.bx,
            edge.handles.by,
            end.x,
            end.y,
          );
        } else if (edge.kind === "polyline" && edge.polylinePoints && edge.polylinePoints.length >= 2) {
          for (let i = 0; i < edge.polylinePoints.length; i += 2) {
            context.lineTo(edge.polylinePoints[i] ?? 0, edge.polylinePoints[i + 1] ?? 0);
          }
          context.lineTo(end.x, end.y);
        } else {
          context.lineTo(end.x, end.y);
        }

        context.lineWidth = edge.stroke?.width ?? 2;
        context.strokeStyle = edge.stroke ? rgbaToCss(edge.stroke.color) : "#125f4f";
        context.stroke();
      });

      snapshot.nodes.forEach((node) => {
        const isSelected = node.id === selectedNodeId;
        const isEdgeStart = node.id === edgeStartNodeId;
        const radius = isSelected ? 7 : 5;

        context.beginPath();
        context.arc(node.x, node.y, radius, 0, Math.PI * 2);
        context.fillStyle = isSelected ? "#f38b2f" : "#0f4238";
        context.fill();

        if (isEdgeStart) {
          context.beginPath();
          context.arc(node.x, node.y, radius + 4, 0, Math.PI * 2);
          context.strokeStyle = "#f38b2f";
          context.lineWidth = 2;
          context.stroke();
        }
      });
    };

    resize();

    const observer = new ResizeObserver(() => {
      resize();
    });
    observer.observe(canvas);

    return () => {
      observer.disconnect();
    };
  }, [snapshot, selectedNodeId, edgeStartNodeId, nodeLookup]);

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const point = getCanvasPoint(canvas, event);
    const pick = service.pick(point.x, point.y, 12);

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
    const dragNodeId = dragNodeIdRef.current;
    const canvas = canvasRef.current;
    if (!canvas || dragNodeId === null) {
      return;
    }
    const point = getCanvasPoint(canvas, event);
    service.moveNode(dragNodeId, point.x, point.y);
  };

  const onPointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (canvas && canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    dragNodeIdRef.current = null;
  };

  const onDoubleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const point = getCanvasPoint(canvas, event.nativeEvent);
    const id = service.addNode(point.x, point.y);
    onSelectNode(id);
  };

  return (
    <canvas
      ref={canvasRef}
      className="editor-canvas"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onDoubleClick}
    />
  );
}
