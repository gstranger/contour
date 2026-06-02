import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { documentRepository } from "../../app/services";
import { useAuth } from "../auth/useAuth";
import type { DocumentMeta } from "../docs/types";
import type { GraphDocument } from "vecnet-wasm";

const normalizedRepoRoot = __REPO_ROOT__.replaceAll("\\", "/");
const legacyWorkbenchBase = `/@fs${normalizedRepoRoot}/web/index.html`;

export function EditorPage() {
  const { user } = useAuth();
  const { docId } = useParams<{ docId: string }>();
  const navigate = useNavigate();

  const [docMeta, setDocMeta] = useState<DocumentMeta | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState<string | null>(null);

  const lastSavedRawRef = useRef<string | null>(null);

  const storageKey = useMemo(() => {
    if (!user || !docId) {
      return null;
    }
    return `studio:workbench:${user.id}:${docId}`;
  }, [user, docId]);

  const workbenchUrl = useMemo(() => {
    if (!storageKey) {
      return null;
    }
    return `${legacyWorkbenchBase}?storageKey=${encodeURIComponent(storageKey)}`;
  }, [storageKey]);

  useEffect(() => {
    if (!user || !docId || !storageKey) {
      return;
    }

    const stored = documentRepository.get(user.id, docId);
    if (!stored) {
      navigate("/app/docs", { replace: true });
      return;
    }

    setError(null);
    setDocMeta(stored.meta);
    setTitleDraft(stored.meta.title);

    const existing = localStorage.getItem(storageKey);
    if (!existing) {
      const payload = JSON.stringify(stored.graph);
      localStorage.setItem(storageKey, payload);
      lastSavedRawRef.current = payload;
    } else {
      lastSavedRawRef.current = existing;
    }
  }, [docId, navigate, storageKey, user]);

  const saveNow = useCallback(() => {
    if (!user || !docMeta || !storageKey) {
      return;
    }

    setIsSaving(true);
    try {
      const raw = localStorage.getItem(storageKey);
      if (!raw) {
        setStatus("No workbench state to save");
        return;
      }

      const parsed = JSON.parse(raw) as GraphDocument;
      const updated = documentRepository.saveGraph(user.id, docMeta.id, parsed);
      if (!updated) {
        setError("Could not save document.");
        return;
      }

      setDocMeta(updated);
      setTitleDraft(updated.title);
      setStatus(`Saved at ${new Date(updated.updatedAt).toLocaleTimeString()}`);
      lastSavedRawRef.current = raw;
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "Save failed.");
    } finally {
      setIsSaving(false);
    }
  }, [docMeta, storageKey, user]);

  useEffect(() => {
    if (!user || !docMeta || !storageKey) {
      return;
    }

    const timer = window.setInterval(() => {
      const raw = localStorage.getItem(storageKey);
      if (!raw || raw === lastSavedRawRef.current) {
        return;
      }

      try {
        const parsed = JSON.parse(raw) as GraphDocument;
        const updated = documentRepository.saveGraph(user.id, docMeta.id, parsed);
        if (updated) {
          setDocMeta(updated);
          lastSavedRawRef.current = raw;
          setStatus(`Autosaved at ${new Date(updated.updatedAt).toLocaleTimeString()}`);
        }
      } catch {
        setStatus("Autosave paused (invalid JSON payload)");
      }
    }, 1200);

    return () => {
      window.clearInterval(timer);
    };
  }, [docMeta, storageKey, user]);

  const onTitleCommit = () => {
    if (!user || !docMeta) {
      return;
    }

    const normalized = titleDraft.trim();
    if (!normalized || normalized === docMeta.title) {
      setTitleDraft(docMeta.title);
      return;
    }

    const renamed = documentRepository.rename(user.id, docMeta.id, normalized);
    if (renamed) {
      setDocMeta(renamed);
      setStatus(`Renamed at ${new Date(renamed.updatedAt).toLocaleTimeString()}`);
    }
  };

  if (error) {
    return (
      <section className="editor-page">
        <article className="editor-error">
          <h1>Could not open document</h1>
          <p>{error}</p>
          <Link className="primary-link" to="/app/docs">
            Back to documents
          </Link>
        </article>
      </section>
    );
  }

  if (!docMeta || !workbenchUrl) {
    return <section className="editor-page loading-panel">Loading workbench...</section>;
  }

  return (
    <section className="editor-page page-enter full-workbench-page">
      <header className="editor-toolbar">
        <div className="editor-title-group">
          <Link className="ghost-button" to="/app/docs">
            Back
          </Link>
          <input
            className="title-input"
            value={titleDraft}
            onChange={(event) => setTitleDraft(event.target.value)}
            onBlur={onTitleCommit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              }
            }}
          />
        </div>
        <div className="editor-actions">
          <span className="pill">{status}</span>
          <button className="primary-button" type="button" onClick={saveNow}>
            {isSaving ? "Saving..." : "Save to Docs"}
          </button>
        </div>
      </header>

      <div className="legacy-workbench-wrap">
        <iframe
          title="Vecnet Legacy Workbench"
          className="legacy-workbench-frame"
          src={workbenchUrl}
        />
      </div>
    </section>
  );
}
