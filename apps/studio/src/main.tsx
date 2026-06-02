import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./app/App";
import { AuthProvider } from "./features/auth/AuthContext";
import { PluginHost } from "./plugins/PluginHost";
import { registerBuiltinPlugins } from "./plugins/builtin";
import "./styles.css";

// Initialize plugin system
const pluginHost = new PluginHost();
registerBuiltinPlugins(pluginHost);

// Expose pluginHost so EditorPage can access it
(window as any).__pluginHost = pluginHost;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <AuthProvider>
        <App />
      </AuthProvider>
    </BrowserRouter>
  </StrictMode>,
);
