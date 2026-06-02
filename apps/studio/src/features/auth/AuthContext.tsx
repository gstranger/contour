import {
  createContext,
  useCallback,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { authStore } from "../../app/services";
import type { AuthUser } from "./authStore";

interface AuthContextValue {
  user: AuthUser | null;
  initializing: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);
const DEMO_USER: AuthUser = { id: "demo-user", email: "demo@vecnet.local" };

interface AuthProviderProps {
  children: ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  const [user, setUser] = useState<AuthUser | null>(() => authStore.getSessionUser() ?? DEMO_USER);
  const initializing = false;

  const login = useCallback(async (email: string, password: string) => {
    const nextUser = authStore.login(email, password);
    setUser(nextUser);
  }, []);

  const register = useCallback(async (email: string, password: string) => {
    const nextUser = authStore.register(email, password);
    setUser(nextUser);
  }, []);

  const logout = useCallback(() => {
    authStore.logout();
    setUser(DEMO_USER);
  }, []);

  const value = useMemo<AuthContextValue>(
    () => ({ user, initializing, login, register, logout }),
    [user, initializing, login, register, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export { AuthContext };
