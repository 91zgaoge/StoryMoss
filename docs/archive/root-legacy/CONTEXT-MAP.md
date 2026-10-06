# Context Map

This repo has multiple bounded contexts. Each has its own `CONTEXT.md` and `docs/adr/`.

| Context | Path | Description |
|---------|------|-------------|
| `frontend` | `src-frontend/src/` | React + TypeScript UI (Frontstage + Backstage) |
| `backend` | `src-tauri/src/` | Rust core (Tauri commands, DB, AI pipelines, memory) |

System-wide ADRs live at `docs/adr/`.
Context-specific ADRs live at `<context>/docs/adr/`.
