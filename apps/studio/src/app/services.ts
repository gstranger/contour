import { LocalAuthStore } from "../features/auth/authStore";
import { LocalDocumentRepository } from "../features/docs/localDocumentRepository";

export const authStore = new LocalAuthStore(window.localStorage);
export const documentRepository = new LocalDocumentRepository(window.localStorage);
