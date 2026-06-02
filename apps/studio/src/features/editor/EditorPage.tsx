// apps/studio/src/features/editor/EditorPage.tsx

import { useEffect, useMemo, useState, useCallback } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { documentRepository } from "../../app/services";
import { useAuth } from "../auth/useAuth";
import { useGraphEditor } from "./useGraphEditor";
import { CanvasViewport } from "./CanvasViewport";
import { EditorToolbar } from "./EditorToolbar";
import { EditorSidebar } from "./EditorSidebar";
import { useActiveTool, usePluginTools } from "../../plugins/hooks";
import type { DocumentMeta } from "../docs/types";

function getPluginHost(): import("../../plugins/PluginHost").PluginHost {
  return (window as any).__pluginHost;
}

export function EditorPage() {
  const { user } = useAuth();
  const { docId } = useParams<{ docId: string }>();
  const navigate = useNavigate();

  const [docMeta, setDocMeta] = useState<DocumentMeta | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState("Ready");

  const {
    service,
    snapshot,
    error,
    context,
    canvasRef,
    selectedNodeId,
    setSelectedNodeId,
    edgeStartNodeId,
    setEdgeStartNodeId,
  } = useGraphEditor();

  const host = getPluginHost();
  const tools = usePluginTools(host);
  const activeTool = useActiveTool(host, context);

  // Expose snapshot/service to host for export plugins
  useEffect(() => {
    const h = host as any;
    h._lastSnapshot = snapshot;
    h._lastService = service;
  }, [host, snapshot, service]);

  // Load document from repository into the graph service
  useEffect(() => {
    if (!user || !docId || !service) return;

    const stored = documentRepository.get(user.id, docId);
    if (!stored) {
      navigate("/app/docs", { replace: true });
      return;
    }

    setDocMeta(stored.meta);
    setTitleDraft(stored.meta.title);

    try {
      service.importDocument(stored.graph);
    } catch (err) {
      console.error("Failed to load document:", err);
    }
  }, [docId, navigate, user, service]);

  // Save
  const saveNow = useCallback(() => {
    if (!user || !docMeta || !service) return;
    setIsSaving(true);
    try {
      const payload = service.exportDocument();
      const updated = documentRepository.saveGraph(user.id, docMeta.id, payload);
      if (!updated) {
        setStatus("Could not save document.");
        return;
      }
      setDocMeta(updated);
      setTitleDraft(updated.title);
      setStatus(`Saved at ${new Date(updated.updatedAt).toLocaleTimeString()}`);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : "Save failed.");
    } finally {
      setIsSaving(false);
    }
  }, [docMeta, service, user]);

  // Autosave
  useEffect(() => {
    if (!user || !docMeta || !service) return;
    const timer = window.setInterval(() => {
      try {
        const payload = service.exportDocument();
        const updated = documentRepository.saveGraph(user.id, docMeta.id, payload);
        if (updated) {
          setDocMeta(updated);
        }
      } catch {
        // Silently skip autosave errors
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [docMeta, service, user]);

  const onTitleCommit = () => {
    if (!user || !docMeta) return;
    const normalized = titleDraft.trim();
    if (!normalized || normalized === docMeta.title) {
      setTitleDraft(docMeta.title);
      return;
    }
    const renamed = documentRepository.rename(user.id, docMeta.id, normalized);
    if (renamed) {
      setDocMeta(renamed);
      setStatus("Renamed");
    }
  };

  const handleActivateTool = useCallback((toolId: string) => {
    host.activateTool(toolId);
  }, [host]);

  const handleClear = () => {
    if (!service) return;
    service.clear();
    setSelectedNodeId(null);
    setEdgeStartNodeId(null);
  };

  if (error) {
    return (
      <section className="editor-page">
        <article className="editor-error">
          <h1>Could not open editor</h1>
          <p>{error}</p>
          <Link className="primary-link" to="/app/docs">Back to documents</Link>
        </article>
      </section>
    );
  }

  if (!docMeta || !service || !snapshot) {
    return <section className="editor-page loading-panel">Loading editor...</section>;
  }

  return (
    <section className="editor-page page-enter">
      <EditorToolbar
        host={host}
        activeToolId={host.getActiveToolId() ?? "select"}
        tools={tools}
        onActivateTool={handleActivateTool}
      />

      <div className="editor-content">
        <div className="canvas-wrap">
          <CanvasViewport
            canvasRef={canvasRef}
            service={service}
            snapshot={snapshot}
            host={host}
            activeTool={activeTool}
            context={context}
            selectedNodeId={selectedNodeId}
            edgeStartNodeId={edgeStartNodeId}
            onSelectNode={setSelectedNodeId}
            onEdgeStartNodeChange={setEdgeStartNodeId}
          />
        </div>

        <EditorSidebar
          snapshot={snapshot}
          selectedNodeId={selectedNodeId}
          onClear={handleClear}
        />
      </div>

      <div className="editor-actions-bar">
        <input
          className="title-input"
          value={titleDraft}
          onChange={(e) => setTitleDraft(e.target.value)}
          onBlur={onTitleCommit}
          onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
        />
        <span className="pill">{status}</span>
        <button className="primary-button" onClick={saveNow} disabled={isSaving}>
          {isSaving ? "Saving..." : "Save"}
        </button>
        <Link className="ghost-button" to="/app/docs">Documents</Link>
      </div>
    </section>
  );
}
