// apps/studio/src/features/editor/EditorSidebar.tsx

import type { GraphSnapshot } from "../../services/graph/types";

interface EditorSidebarProps {
  snapshot: GraphSnapshot;
  selectedNodeId: number | null;
  onClear: () => void;
}

export function EditorSidebar({ snapshot, selectedNodeId, onClear }: EditorSidebarProps) {
  const selectedNode = selectedNodeId !== null
    ? snapshot.nodes.find((n) => n.id === selectedNodeId)
    : null;

  return (
    <aside className="editor-sidebar">
      <dl>
        <div><dt>Nodes</dt><dd>{snapshot.nodes.length}</dd></div>
        <div><dt>Edges</dt><dd>{snapshot.edges.length}</dd></div>
        <div><dt>Regions</dt><dd>{snapshot.regions.filter((r) => r.filled).length} filled</dd></div>
        <div><dt>Version</dt><dd>v{snapshot.geomVersion.toString()}</dd></div>
      </dl>

      {selectedNode && (
        <div className="selection-info">
          <h3>Selected Node</h3>
          <dl>
            <div><dt>ID</dt><dd>{selectedNode.id}</dd></div>
            <div><dt>X</dt><dd>{selectedNode.x.toFixed(1)}</dd></div>
            <div><dt>Y</dt><dd>{selectedNode.y.toFixed(1)}</dd></div>
          </dl>
        </div>
      )}

      <div className="sidebar-actions">
        <button className="danger-button" onClick={onClear} type="button">
          Clear Canvas
        </button>
      </div>
    </aside>
  );
}
