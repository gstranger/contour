import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./AppShell";
import { RequireAuth } from "./RequireAuth";
import { LoginPage } from "../features/auth/LoginPage";
import { DocumentsPage } from "../features/docs/DocumentsPage";
import { EditorPage } from "../features/editor/EditorPage";

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route
        path="/app"
        element={
          <RequireAuth>
            <AppShell />
          </RequireAuth>
        }
      >
        <Route index element={<Navigate to="docs" replace />} />
        <Route path="docs" element={<DocumentsPage />} />
        <Route path="docs/:docId" element={<EditorPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/app" replace />} />
    </Routes>
  );
}
