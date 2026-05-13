# Architecture

**Analysis Date:** 2026-05-11

## Pattern Overview

**Overall:** Tauri Desktop Application with Layered Backend Architecture

**Key Characteristics:**
- Desktop-first architecture using Tauri v2 for cross-platform support
- Rust backend with async/await patterns (Tokio runtime)
- React 18 + TypeScript frontend with Zustand state management
- IPC-based communication between frontend and backend via Tauri commands
- Event-driven state synchronization across windows (backstage/frontstage)
- Multi-agent system for creative writing assistance
- Vector database integration (LanceDB) for semantic search

## Layers

**Presentation Layer (Frontend):**
- Purpose: User interface and interaction management
- Location: `src-frontend/src/`
- Contains: React components, pages, hooks, stores, services
- Depends on: Tauri IPC API, TanStack Query for data fetching, Zustand for state
- Used by: End users through Tauri desktop window

**IPC/Command Layer:**
- Purpose: Bridge between frontend and backend via Tauri commands
- Location: `src-tauri/src/commands_v3.rs`, `src-tauri/src/agents/commands.rs`
- Contains: Tauri `#[command]` handlers with snake_case naming (v2 convention)
- Depends on: Database repositories, service layer, state management
- Used by: Frontend via `invoke()` calls from `src-frontend/src/services/tauri.ts`

**Service Layer:**
- Purpose: Business logic orchestration and domain operations
- Location: `src-tauri/src/agents/`, `src-tauri/src/creative_engine/`, `src-tauri/src/memory/`
- Contains: Agent implementations, LLM service, memory management, skill execution
- Depends on: Database layer, LLM providers, vector database
- Used by: Command layer, state sync system

**Data Access Layer:**
- Purpose: Database operations and persistence
- Location: `src-tauri/src/db/`
- Contains: SQLite connection pooling (r2d2), repositories for all entities
- Depends on: SQLite database, schema definitions
- Used by: Service layer, command handlers

**Infrastructure Layer:**
- Purpose: Cross-cutting concerns and utilities
- Location: `src-tauri/src/config/`, `src-tauri/src/logging.rs`, `src-tauri/src/utils/`
- Contains: Configuration management, logging setup, utility functions
- Depends on: Environment variables, file system
- Used by: All layers

## Data Flow

**Story Creation Flow:**

1. User submits form in `CreationWizard.tsx` component
2. Frontend calls `createStory()` from `src-frontend/src/services/tauri.ts`
3. Tauri invokes `create_story` command in `src-tauri/src/commands_v3.rs`
4. Command handler uses `StoryRepository` to persist to SQLite
5. Backend emits `story_created` event via `StateSync::emit_story_created()`
6. Frontend `useSyncStore()` hook listens and invalidates TanStack Query cache
7. UI re-renders with new story data

**Agent Execution Flow:**

1. User triggers skill/agent from UI (e.g., "Generate Scene")
2. Frontend invokes command like `generate_scene_with_agent()`
3. Command handler instantiates appropriate agent (e.g., `NovelCreationAgent`)
4. Agent calls `LlmService` to stream LLM responses
5. Backend emits progress events via Tauri event system
6. Frontend listens via `listen()` and updates UI in real-time
7. Final result persisted to database and synced to frontend

**State Management:**

- **Backend State:** SQLite database as source of truth, in-memory caches (INGEST_COOLDOWN, SKILL_MANAGER)
- **Frontend State:** Zustand store (`useAppStore`) for UI state, TanStack Query for server state
- **Synchronization:** Event-driven via `state_sync` module emitting typed events (story_created, scene_updated, etc.)
- **Window Coordination:** Backstage (main) and Frontstage (preview) windows sync via shared event system

## Key Abstractions

**Agent System:**
- Purpose: Pluggable AI-powered writing assistants
- Examples: `src-tauri/src/agents/novel_creation.rs`, `src-tauri/src/agents/orchestrator.rs`
- Pattern: Trait-based (`Agent` trait) with async execution, context passing, result serialization

**Repository Pattern:**
- Purpose: Encapsulate database access for each entity
- Examples: `StoryRepository`, `CharacterRepository`, `SceneRepository` in `src-tauri/src/db/repositories_v3.rs`
- Pattern: CRUD operations with error handling, transaction support

**Hook System (Frontend):**
- Purpose: Encapsulate data fetching and state management logic
- Examples: `useChapters()`, `useCharacters()`, `useSyncStore()` in `src-frontend/src/hooks/`
- Pattern: React hooks using TanStack Query for server state, custom logic for side effects

**Skill Manager:**
- Purpose: Dynamic skill registration and execution
- Location: `src-tauri/src/skills/`
- Pattern: Singleton pattern via `OnceCell`, skill discovery and lifecycle management

## Entry Points

**Backend Entry:**
- Location: `src-tauri/src/lib.rs`
- Triggers: Tauri app initialization via `tauri::Builder::default()`
- Responsibilities: Database initialization, plugin setup, window event handling, command registration

**Frontend Entry:**
- Location: `src-frontend/src/main.tsx`
- Triggers: React app mount
- Responsibilities: QueryClient setup, error boundary wrapping, provider initialization

**Main UI Entry:**
- Location: `src-frontend/src/App.tsx`
- Triggers: After React initialization
- Responsibilities: Route management, global state setup, event listeners, window coordination

## Error Handling

**Strategy:** Layered error propagation with user-facing messages

**Patterns:**
- Backend commands return `Result<T, String>` for Tauri serialization
- Frontend wraps calls in try-catch, displays toast notifications via `react-hot-toast`
- `ErrorBoundary` component catches React rendering errors
- Logging via `tracing` crate (backend) and custom logger (frontend)
- Failed operations logged but don't crash app; graceful degradation

## Cross-Cutting Concerns

**Logging:** 
- Backend: `tracing` + `tracing-subscriber` with JSON output to files
- Frontend: Custom logger in `src-frontend/src/utils/logger.ts` with sanitization of sensitive data

**Validation:**
- Backend: Repository layer validates inputs before database operations
- Frontend: React Hook Form for form validation, type safety via TypeScript

**Authentication:**
- Backend: OAuth2 + JWT via `src-tauri/src/auth/` module
- Frontend: Session stored in Zustand store, checked on app startup via `LoginModal`

**State Sync:**
- Centralized via `src-tauri/src/state_sync/` module
- Events emitted after mutations, frontend listeners invalidate caches
- Ensures backstage/frontstage windows stay in sync

---

*Architecture analysis: 2026-05-11*
