# Vecnet Studio (React Frontend)

Production-style frontend shell for `vecnet-wasm` with:

- React + TypeScript + Vite
- Client routing (`react-router-dom`)
- Typed Graph service layer over the WASM `Graph` API
- Local document management (create/open/rename/delete/save)
- Local auth flow (register/login/logout)
- Canvas-based editor viewport consuming Graph snapshots
- Full legacy workbench embedded in the doc editor route (Pen, Bend, Bucket, Text, primitives, booleans, effects)

## Run

From repo root:

```bash
cd apps/studio
npm install
npm run dev
```

Open `http://localhost:5173`.

## Build

```bash
npm run build
```

## Notes

- The app depends on the local package `vecnet-wasm` via `file:../../npm`.
- Authentication and document storage are localStorage-backed placeholders intended to be replaced by real backend services.
- Doc editor routes (`/app/docs/:id`) run the full `web/index.html` workbench with a per-document storage key and autosave back into the docs repository.
