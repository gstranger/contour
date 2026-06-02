import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const studioRoot = fileURLToPath(new URL(".", import.meta.url));
const repoRoot = path.resolve(studioRoot, "../..");

// https://vite.dev/config/
export default defineConfig({
  assetsInclude: ["**/*.wasm"],
  define: {
    __REPO_ROOT__: JSON.stringify(repoRoot),
  },
  plugins: [react()],
  optimizeDeps: {
    exclude: ["vecnet-wasm"],
  },
  resolve: {
    preserveSymlinks: true,
  },
  server: {
    fs: {
      allow: [repoRoot],
    },
  },
});
