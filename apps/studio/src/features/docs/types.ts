import type { GraphDocument } from "vecnet-wasm";

export interface DocumentMeta {
  id: string;
  ownerId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface StoredDocument {
  meta: DocumentMeta;
  graph: GraphDocument;
}
