export interface AuthUser {
  id: string;
  email: string;
}

interface StoredAuthUser extends AuthUser {
  password: string;
}

const USERS_KEY = "studio:auth:users";
const SESSION_KEY = "studio:auth:session";

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

function writeJson(storage: Storage, key: string, value: unknown): void {
  storage.setItem(key, JSON.stringify(value));
}

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `u_${Math.random().toString(36).slice(2, 10)}`;
}

export class LocalAuthStore {
  private readonly storage: Storage;

  constructor(storage: Storage) {
    this.storage = storage;
  }

  getSessionUser(): AuthUser | null {
    const users = readJson<StoredAuthUser[]>(this.storage, USERS_KEY, []);
    const session = this.storage.getItem(SESSION_KEY);
    if (!session) {
      return null;
    }
    const user = users.find((entry) => entry.id === session);
    if (!user) {
      return null;
    }
    return { id: user.id, email: user.email };
  }

  login(email: string, password: string): AuthUser {
    const normalizedEmail = email.trim().toLowerCase();
    const users = readJson<StoredAuthUser[]>(this.storage, USERS_KEY, []);
    const user = users.find((entry) => entry.email.toLowerCase() === normalizedEmail);

    if (!user || user.password !== password) {
      throw new Error("Invalid email or password.");
    }

    this.storage.setItem(SESSION_KEY, user.id);
    return { id: user.id, email: user.email };
  }

  register(email: string, password: string): AuthUser {
    const normalizedEmail = email.trim().toLowerCase();
    if (!normalizedEmail) {
      throw new Error("Email is required.");
    }
    if (password.length < 6) {
      throw new Error("Password must be at least 6 characters.");
    }

    const users = readJson<StoredAuthUser[]>(this.storage, USERS_KEY, []);
    if (users.some((entry) => entry.email.toLowerCase() === normalizedEmail)) {
      throw new Error("An account already exists for this email.");
    }

    const user: StoredAuthUser = {
      id: createId(),
      email: normalizedEmail,
      password,
    };

    users.push(user);
    writeJson(this.storage, USERS_KEY, users);
    this.storage.setItem(SESSION_KEY, user.id);
    return { id: user.id, email: user.email };
  }

  logout(): void {
    this.storage.removeItem(SESSION_KEY);
  }
}
