// apps/studio/src/features/editor/EditorToolbar.tsx

import type { PluginHost } from "../../plugins/PluginHost";
import type { ToolEntry } from "../../plugins/types";

interface EditorToolbarProps {
  host: PluginHost;
  activeToolId: string | null;
  tools: ToolEntry[];
  onActivateTool: (id: string) => void;
}

export function EditorToolbar({ host, activeToolId, tools, onActivateTool }: EditorToolbarProps) {
  const exportPlugins = host.getExports();

  const handleExport = async (pluginId: string) => {
    const exp = exportPlugins.find((e) => e.id === pluginId);
    if (!exp) return;

    try {
      const hostAny = host as any;
      const snapshot = hostAny._lastSnapshot;
      const service = hostAny._lastService;
      if (!snapshot || !service) return;

      const blob = await exp.export(snapshot, service);
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `vecnet-export.${exp.extension}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error(`Export (${pluginId}) failed:`, err);
    }
  };

  return (
    <header className="editor-toolbar">
      <div className="toolbar-group">
        {tools.map((tool) => (
          <button
            key={tool.id}
            className={`tool-button ${activeToolId === tool.id ? "tool-button--active" : ""}`}
            title={`${tool.name}${tool.keyboardShortcut ? ` (${tool.keyboardShortcut})` : ""}`}
            onClick={() => onActivateTool(tool.id)}
          >
            {tool.name}
          </button>
        ))}
      </div>

      <div className="toolbar-group">
        {exportPlugins.map((exp) => (
          <button
            key={exp.id}
            className="tool-button"
            title={`Export as ${exp.name}`}
            onClick={() => handleExport(exp.id)}
          >
            {exp.name}
          </button>
        ))}
      </div>
    </header>
  );
}
