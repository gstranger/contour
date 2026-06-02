import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { documentRepository } from "../../app/services";
import { useAuth } from "../auth/useAuth";
import { createGraphService } from "../../services/graph/graphService";
import type { DocumentMeta } from "./types";

export function DocumentsPage() {
  const { user } = useAuth();
  const navigate = useNavigate();

  const [docs, setDocs] = useState<DocumentMeta[]>([]);
  const [creating, setCreating] = useState(false);

  const canCreate = useMemo(() => !creating && Boolean(user), [creating, user]);

  const refresh = useCallback(() => {
    if (!user) {
      setDocs([]);
      return;
    }
    setDocs(documentRepository.list(user.id));
  }, [user]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const onCreate = async () => {
    if (!user || !canCreate) {
      return;
    }

    setCreating(true);
    try {
      const service = await createGraphService();
      const title = `Untitled ${docs.length + 1}`;
      const meta = documentRepository.create(user.id, title, service.exportDocument());
      service.dispose();
      navigate(`/app/docs/${meta.id}`);
    } finally {
      setCreating(false);
      refresh();
    }
  };

  const onRename = (doc: DocumentMeta) => {
    if (!user) {
      return;
    }
    const nextTitle = window.prompt("Rename document", doc.title)?.trim();
    if (!nextTitle) {
      return;
    }
    documentRepository.rename(user.id, doc.id, nextTitle);
    refresh();
  };

  const onDelete = (doc: DocumentMeta) => {
    if (!user) {
      return;
    }
    const confirmed = window.confirm(`Delete "${doc.title}"? This cannot be undone.`);
    if (!confirmed) {
      return;
    }
    documentRepository.remove(user.id, doc.id);
    refresh();
  };

  return (
    <section className="docs-page page-enter">
      <header className="docs-header">
        <div>
          <h1>Documents</h1>
          <p>Open a graph document or create a new one for editing.</p>
        </div>
        <button className="primary-button" onClick={onCreate} type="button" disabled={!canCreate}>
          {creating ? "Creating..." : "New document"}
        </button>
      </header>

      <div className="doc-grid">
        {docs.length === 0 ? (
          <article className="doc-card doc-empty">
            <h2>No documents yet</h2>
            <p>Create your first doc to start editing nodes and edges.</p>
          </article>
        ) : (
          docs.map((doc, index) => (
            <article
              key={doc.id}
              className="doc-card"
              style={{ animationDelay: `${index * 40}ms` }}
            >
              <h2>{doc.title}</h2>
              <p>Updated {new Date(doc.updatedAt).toLocaleString()}</p>
              <div className="doc-actions">
                <Link className="primary-link" to={`/app/docs/${doc.id}`}>
                  Open
                </Link>
                <button type="button" className="ghost-button" onClick={() => onRename(doc)}>
                  Rename
                </button>
                <button type="button" className="danger-button" onClick={() => onDelete(doc)}>
                  Delete
                </button>
              </div>
            </article>
          ))
        )}
      </div>
    </section>
  );
}
