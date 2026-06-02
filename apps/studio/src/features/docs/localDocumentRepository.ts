import type { GraphDocument } from "vecnet-wasm";
import type { DocumentMeta, StoredDocument } from "./types";

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `doc_${Math.random().toString(36).slice(2, 10)}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

function readJson<T>(storage: Storage, key: string, fallback: T): T {
  try {
    const raw = storage.getItem(key);
    if (!raw) {
      return fallback;
    }
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function emptyGraphDocument(): GraphDocument {
  return {
    version: 1,
    nodes: [],
    edges: [],
    fills: [],
  };
}

function normalizeGraphDocument(value: unknown): GraphDocument | null {
  if (!isRecord(value)) {
    return null;
  }

  if (!Array.isArray(value.nodes) || !Array.isArray(value.edges)) {
    return null;
  }

  const version = typeof value.version === "number" && Number.isFinite(value.version)
    ? value.version
    : 1;

  const normalized: GraphDocument = {
    version,
    nodes: value.nodes as GraphDocument["nodes"],
    edges: value.edges as GraphDocument["edges"],
  };

  if (Array.isArray(value.fills)) {
    normalized.fills = value.fills as GraphDocument["fills"];
  }
  if (Array.isArray(value.layers)) {
    normalized.layers = value.layers as GraphDocument["layers"];
  }
  if (Array.isArray(value.groups)) {
    normalized.groups = value.groups as GraphDocument["groups"];
  }
  if (Array.isArray(value.gradients)) {
    normalized.gradients = value.gradients as GraphDocument["gradients"];
  }
  if (Array.isArray(value.texts)) {
    normalized.texts = value.texts as GraphDocument["texts"];
  }
  if (Array.isArray(value.effects)) {
    normalized.effects = value.effects as GraphDocument["effects"];
  }
  if (Array.isArray(value.effect_bindings)) {
    normalized.effect_bindings = value.effect_bindings as GraphDocument["effect_bindings"];
  }

  return normalized;
}

function normalizeMeta(value: unknown, ownerId: string, docId: string): DocumentMeta {
  const stamp = nowIso();

  if (!isRecord(value)) {
    return {
      id: docId,
      ownerId,
      title: "Recovered document",
      createdAt: stamp,
      updatedAt: stamp,
    };
  }

  const id = typeof value.id === "string" && value.id ? value.id : docId;
  const title = typeof value.title === "string" && value.title.trim()
    ? value.title.trim()
    : "Recovered document";

  const createdAt = typeof value.createdAt === "string" && value.createdAt
    ? value.createdAt
    : stamp;
  const updatedAt = typeof value.updatedAt === "string" && value.updatedAt
    ? value.updatedAt
    : createdAt;

  return {
    id,
    ownerId,
    title,
    createdAt,
    updatedAt,
  };
}

function normalizeMetaList(value: unknown, ownerId: string): DocumentMeta[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((item) => {
      if (!isRecord(item) || typeof item.id !== "string" || !item.id) {
        return null;
      }
      return normalizeMeta(item, ownerId, item.id);
    })
    .filter((item): item is DocumentMeta => item !== null);
}

export class LocalDocumentRepository {
  private readonly storage: Storage;

  constructor(storage: Storage) {
    this.storage = storage;
  }

  list(ownerId: string): DocumentMeta[] {
    const metasRaw = readJson<unknown>(this.storage, this.indexKey(ownerId), []);
    const metas = normalizeMetaList(metasRaw, ownerId);
    this.storage.setItem(this.indexKey(ownerId), JSON.stringify(metas));
    return [...metas].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  get(ownerId: string, docId: string): StoredDocument | null {
    const key = this.documentKey(ownerId, docId);
    const raw = readJson<unknown>(this.storage, key, null);
    if (raw === null) {
      return null;
    }

    let meta: DocumentMeta;
    let graph: GraphDocument | null;

    if (isRecord(raw) && "meta" in raw && "graph" in raw) {
      meta = normalizeMeta(raw.meta, ownerId, docId);
      graph = normalizeGraphDocument(raw.graph);
    } else {
      meta = normalizeMeta(null, ownerId, docId);
      graph = normalizeGraphDocument(raw);
    }

    const normalized: StoredDocument = {
      meta,
      graph: graph ?? emptyGraphDocument(),
    };

    this.writeDocument(ownerId, normalized);
    this.upsertMeta(ownerId, meta);
    return normalized;
  }

  create(ownerId: string, title: string, graph: GraphDocument): DocumentMeta {
    const stamp = nowIso();
    const meta: DocumentMeta = {
      id: createId(),
      ownerId,
      title,
      createdAt: stamp,
      updatedAt: stamp,
    };

    this.writeDocument(ownerId, {
      meta,
      graph: normalizeGraphDocument(graph) ?? emptyGraphDocument(),
    });

    const metas = this.list(ownerId);
    metas.push(meta);
    this.storage.setItem(this.indexKey(ownerId), JSON.stringify(metas));
    return meta;
  }

  rename(ownerId: string, docId: string, title: string): DocumentMeta | null {
    const record = this.get(ownerId, docId);
    if (!record) {
      return null;
    }

    const updatedMeta: DocumentMeta = {
      ...record.meta,
      title,
      updatedAt: nowIso(),
    };

    this.writeDocument(ownerId, {
      ...record,
      meta: updatedMeta,
    });

    this.upsertMeta(ownerId, updatedMeta);
    return updatedMeta;
  }

  saveGraph(ownerId: string, docId: string, graph: GraphDocument): DocumentMeta | null {
    const record = this.get(ownerId, docId);
    if (!record) {
      return null;
    }

    const updatedMeta: DocumentMeta = {
      ...record.meta,
      updatedAt: nowIso(),
    };

    this.writeDocument(ownerId, {
      meta: updatedMeta,
      graph: normalizeGraphDocument(graph) ?? emptyGraphDocument(),
    });

    this.upsertMeta(ownerId, updatedMeta);
    return updatedMeta;
  }

  remove(ownerId: string, docId: string): boolean {
    const existing = this.get(ownerId, docId);
    if (!existing) {
      return false;
    }

    this.storage.removeItem(this.documentKey(ownerId, docId));
    const nextMetas = this.list(ownerId).filter((meta) => meta.id !== docId);
    this.storage.setItem(this.indexKey(ownerId), JSON.stringify(nextMetas));
    return true;
  }

  private upsertMeta(ownerId: string, meta: DocumentMeta): void {
    const metas = this.list(ownerId);
    const next = metas.filter((item) => item.id !== meta.id);
    next.push(meta);
    this.storage.setItem(this.indexKey(ownerId), JSON.stringify(next));
  }

  private writeDocument(ownerId: string, record: StoredDocument): void {
    this.storage.setItem(this.documentKey(ownerId, record.meta.id), JSON.stringify(record));
  }

  private indexKey(ownerId: string): string {
    return `studio:docs:${ownerId}:index`;
  }

  private documentKey(ownerId: string, docId: string): string {
    return `studio:docs:${ownerId}:${docId}`;
  }
}
