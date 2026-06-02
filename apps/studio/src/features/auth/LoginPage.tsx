import { useMemo, useState, type FormEvent } from "react";
import { Navigate, useLocation, useNavigate } from "react-router-dom";
import { useAuth } from "./useAuth";

interface LocationState {
  from?: string;
}

export function LoginPage() {
  const { user, login, register } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();

  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const nextRoute = useMemo(() => {
    const state = location.state as LocationState | null;
    return state?.from || "/app/docs";
  }, [location.state]);

  if (user) {
    return <Navigate to="/app/docs" replace />;
  }

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      if (mode === "login") {
        await login(email, password);
      } else {
        await register(email, password);
      }
      navigate(nextRoute, { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Authentication failed.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="auth-page page-enter">
      <div className="auth-card">
        <h1>Vecnet Studio</h1>
        <p>Sign in to access your vector network documents.</p>
        <form onSubmit={onSubmit}>
          <label>
            Email
            <input
              autoComplete="email"
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              required
            />
          </label>
          <label>
            Password
            <input
              autoComplete={mode === "login" ? "current-password" : "new-password"}
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              minLength={6}
              required
            />
          </label>
          {error ? <div className="error-text">{error}</div> : null}
          <button type="submit" className="primary-button" disabled={submitting}>
            {submitting ? "Working..." : mode === "login" ? "Sign in" : "Create account"}
          </button>
        </form>
        <button
          className="ghost-button"
          type="button"
          onClick={() => setMode((current) => (current === "login" ? "register" : "login"))}
        >
          {mode === "login" ? "Need an account? Register" : "Have an account? Sign in"}
        </button>
        <a
          className="subtle-link"
          href="https://github.com/gstranger/vecnet-wasm"
          target="_blank"
          rel="noreferrer"
        >
          Project repository
        </a>
      </div>
    </div>
  );
}
