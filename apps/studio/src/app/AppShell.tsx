import { Link, NavLink, Outlet } from "react-router-dom";
import { useAuth } from "../features/auth/useAuth";

export function AppShell() {
  const { user, logout } = useAuth();

  const onLogout = () => {
    logout();
  };

  return (
    <div className="app-shell page-enter">
      <header className="app-header">
        <Link to="/app/docs" className="brand">
          Vecnet Studio
        </Link>
        <nav className="nav-links">
          <NavLink to="/app/docs" end>
            Docs
          </NavLink>
        </nav>
        <div className="header-actions">
          <span className="pill">{user?.email}</span>
          <button className="ghost-button" onClick={onLogout} type="button">
            Reset to demo
          </button>
        </div>
      </header>
      <main className="app-main">
        <Outlet />
      </main>
    </div>
  );
}
