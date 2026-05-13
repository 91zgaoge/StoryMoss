# Codebase Structure

**Analysis Date:** 2026-05-11

## Directory Layout

```
v2-rust/
├── src-tauri/                  # Rust backend (Tauri v2)
│   ├── src/
│   │   ├── lib.rs              # Main entry point, app initialization
│   │   ├── main.rs             # Minimal main (delegates to lib.rs)
│   │   ├── commands_v3.rs       # Tauri IPC command handlers (157 commands)
│   │   ├── agents/              # AI agent system
│   │   ├── db/                  # Database layer (SQLite + repositories)
│   │   ├── llm/                 # LLM service integration
│   │   ├── memory/              # Memory management and retention
│   │   ├── state_sync/          # Event-driven state synchronization
│   │   ├── skills/              # Skill manager and execution
│   │   ├── creative_engine/     # Core creative writing logic
│   │   ├── export/              # Export to PDF/EPUB/ZIP
│   │   ├── book_deconstruction/ # Book analysis and parsing
│   │   ├── auth/                # Authentication (OAuth2, JWT)
│   │   ├── config/              # Configuration management
│   │   ├── workflow/            # Workflow scheduling and execution
│   │   ├── mcp/                 # Model Context Protocol integration
│   │   ├── collab/              # Collaboration (WebSocket)
│   │   ├── task_system/         # Task scheduling
│   │   ├── audit/               # Audit logging
│   │   ├── analytics/           # Analytics tracking
│   │   ├── logging.rs           # Logging setup
│   │   ├── utils/               # Utility functions
│   │   └── tests/               # Integration tests
│   ├── Cargo.toml               # Rust dependencies
│   ├── tauri.conf.json          # Tauri configuration
│   └── capabilities/            # Tauri security capabilities
│
├── src-frontend/                # React + TypeScript frontend
│   ├── src/
│   │   ├── main.tsx             # React entry point
│   │   ├── App.tsx              # Root component with routing
│   │   ├── pages/               # Page components (Stories, Characters, etc.)
│   │   ├── components/          # Reusable UI components
│   │   ├── hooks/               # Custom React hooks
│   │   ├── stores/              # Zustand state stores
│   │   ├── services/            # API/Tauri service wrappers
│   │   ├── types/               # TypeScript type definitions
│   │   ├── utils/               # Utility functions
│   │   ├── config/              # Frontend configuration
│   │   ├── frontstage/          # Preview window components
│   │   ├── test/                # Test utilities
│   │   └── index.css            # Global styles (Tailwind)
│   ├── package.json             # Node dependencies
│   ├── tsconfig.json            # TypeScript configuration
│   ├── vite.config.ts           # Vite build configuration
│   └── vitest.config.ts         # Vitest test configuration
│
├── Cargo.toml                   # Workspace root (members: src-tauri)
├── package.json                 # Root npm scripts (E2E tests)
├── .planning/codebase/          # GSD codebase analysis documents
├── docs/                        # Documentation
├── e2e/                         # Playwright E2E tests
└── scripts/                     # Build and utility scripts
```

## Directory Purposes

**src-tauri/src/agents/:**
- Purpose: Multi-agent system for AI-powered writing assistance
- Contains: Agent trait implementations, orchestration, specific agents (NovelCreation, StyleMimic, etc.)
- Key files: `mod.rs` (trait definitions), `service.rs` (agent execution), `commands.rs` (IPC handlers)

**src-tauri/src/db/:**
- Purpose: Data persistence and access layer
- Contains: SQLite connection pooling, repository pattern implementations, schema definitions
- Key files: `connection.rs` (pool setup), `repositories_v3.rs` (CRUD operations), `models_v3.rs` (data structures)

**src-tauri/src/memory/:**
- Purpose: Memory management for context retention and knowledge distillation
- Contains: Retention policies, ingest pipelines, memory compression
- Key files: `retention.rs`, `ingest.rs`, `compressor.rs`

**src-tauri/src/state_sync/:**
- Purpose: Real-time state synchronization between backend and frontend
- Contains: Event emission, listener coordination
- Key files: `service.rs` (StateSync implementation), `events.rs` (event types)

**src-tauri/src/skills/:**
- Purpose: Dynamic skill registration and execution system
- Contains: Skill manager, skill discovery, lifecycle management
- Key files: `manager.rs` (SkillManager), `registry.rs` (skill registration)

**src-tauri/src/creative_engine/:**
- Purpose: Core creative writing logic and algorithms
- Contains: Scene generation, character development, plot analysis
- Key files: `mod.rs` (main logic), `generator.rs` (content generation)

**src-tauri/src/export/:**
- Purpose: Export stories to multiple formats
- Contains: PDF, EPUB, ZIP export implementations
- Key files: `mod.rs` (export orchestration), `pdf.rs`, `epub.rs`, `zip.rs`

**src-tauri/src/book_deconstruction/:**
- Purpose: Analyze and parse uploaded books
- Contains: PDF/EPUB parsing, chapter extraction, character identification
- Key files: `analyzer.rs`, `parser.rs`

**src-frontend/src/pages/:**
- Purpose: Top-level page components for each feature area
- Contains: Dashboard, Stories, Characters, Scenes, Skills, Settings, etc.
- Key files: `Dashboard.tsx`, `Stories.tsx`, `Characters.tsx`, `Scenes.tsx`

**src-frontend/src/components/:**
- Purpose: Reusable UI components
- Contains: Editors, dialogs, panels, visualizations
- Key files: `Editor.tsx` (rich text editor), `Sidebar.tsx` (navigation), `ExecutionPanel.tsx` (skill execution)

**src-frontend/src/hooks/:**
- Purpose: Custom React hooks for data fetching and state management
- Contains: TanStack Query wrappers, event listeners, state synchronization
- Key files: `useChapters.ts`, `useCharacters.ts`, `useSyncStore.ts`, `useExecutionState.ts`

**src-frontend/src/stores/:**
- Purpose: Zustand state stores for UI state
- Contains: App state, auth state, UI preferences
- Key files: `appStore.ts` (main app state), `useAuthStore.ts` (authentication)

**src-frontend/src/services/:**
- Purpose: API and service wrappers
- Contains: Tauri IPC wrappers, HTTP clients, authentication
- Key files: `tauri.ts` (all Tauri command wrappers), `auth.ts`, `modelService.ts`

**src-frontend/src/types/:**
- Purpose: TypeScript type definitions
- Contains: Interfaces for all data models
- Key files: `index.ts` (main types), `v3.ts` (v3 API types)

## Key File Locations

**Entry Points:**
- `src-tauri/src/lib.rs`: Backend initialization, Tauri app setup, command registration
- `src-frontend/src/main.tsx`: React app mount, provider setup
- `src-frontend/src/App.tsx`: Root component, routing, global state initialization

**Configuration:**
- `src-tauri/Cargo.toml`: Rust dependencies and build configuration
- `src-frontend/package.json`: Node dependencies and scripts
- `src-tauri/tauri.conf.json`: Tauri window, security, and build settings
- `src-frontend/vite.config.ts`: Frontend build configuration

**Core Logic:**
- `src-tauri/src/commands_v3.rs`: All 157 Tauri IPC command handlers
- `src-tauri/src/agents/service.rs`: Agent execution orchestration
- `src-tauri/src/db/repositories_v3.rs`: Database CRUD operations
- `src-frontend/src/services/tauri.ts`: Frontend Tauri API wrapper

**Testing:**
- `src-tauri/src/tests/`: Integration tests for backend
- `src-frontend/src/test/`: Test utilities and setup
- `e2e/`: Playwright E2E tests

## Naming Conventions

**Files:**
- Rust: `snake_case.rs` (e.g., `novel_creation.rs`, `memory_compressor.rs`)
- TypeScript: `camelCase.ts` or `PascalCase.tsx` for components (e.g., `useChapters.ts`, `Editor.tsx`)
- Tests: `*.test.ts`, `*.spec.ts`, `*.test.tsx`, `*.spec.tsx`

**Directories:**
- Rust modules: `snake_case/` (e.g., `src-tauri/src/creative_engine/`)
- Feature directories: `kebab-case/` (e.g., `src-frontend/src/components/book-deconstruction/`)

**Tauri Commands:**
- Format: `snake_case` (enforced by `#[command(rename_all = "snake_case")]`)
- Examples: `create_story`, `generate_scene_with_agent`, `list_characters`

**React Components:**
- Format: `PascalCase` (e.g., `Editor.tsx`, `Sidebar.tsx`)
- Hooks: `useXxx` (e.g., `useChapters`, `useSyncStore`)
- Stores: `useXxxStore` (e.g., `useAppStore`, `useAuthStore`)

## Where to Add New Code

**New Feature (e.g., "Foreshadowing System"):**
- Backend implementation: `src-tauri/src/foreshadowing/` (new module)
  - `mod.rs`: Module definition and public API
  - `service.rs`: Business logic
  - `commands.rs`: Tauri command handlers
  - `models.rs`: Data structures
- Database: Add repository in `src-tauri/src/db/repositories_v3.rs`
- Frontend page: `src-frontend/src/pages/Foreshadowing.tsx`
- Frontend hooks: `src-frontend/src/hooks/useForeshadowings.ts`
- Frontend types: Add to `src-frontend/src/types/index.ts`

**New Component:**
- Location: `src-frontend/src/components/` (or subdirectory if feature-specific)
- Pattern: Functional component with TypeScript props interface
- Example: `src-frontend/src/components/book-deconstruction/BookDetailView.tsx`

**New Agent:**
- Location: `src-tauri/src/agents/` (new file or extend existing)
- Pattern: Implement `Agent` trait from `src-tauri/src/agents/mod.rs`
- Register: Add to agent registry in `src-tauri/src/agents/service.rs`
- Example: `src-tauri/src/agents/novel_creation.rs`

**Utilities:**
- Shared Rust utilities: `src-tauri/src/utils/`
- Shared TypeScript utilities: `src-frontend/src/utils/`
- Example: `src-frontend/src/utils/logger.ts`

**Tests:**
- Backend integration tests: `src-tauri/src/tests/`
- Frontend unit tests: Co-located with component or in `src-frontend/src/test/`
- E2E tests: `e2e/` directory

## Special Directories

**src-tauri/src/tests/:**
- Purpose: Integration tests for backend functionality
- Generated: No (manually written)
- Committed: Yes

**src-frontend/dist/:**
- Purpose: Built frontend assets
- Generated: Yes (by Vite during build)
- Committed: No

**src-tauri/target/:**
- Purpose: Compiled Rust binaries and artifacts
- Generated: Yes (by Cargo)
- Committed: No

**src-tauri/gen/:**
- Purpose: Generated Tauri bindings and types
- Generated: Yes (by Tauri CLI)
- Committed: No

**.planning/codebase/:**
- Purpose: GSD codebase analysis documents
- Generated: Yes (by GSD agents)
- Committed: Yes

**docs/:**
- Purpose: Project documentation and architecture guides
- Generated: No (manually written)
- Committed: Yes

---

*Structure analysis: 2026-05-11*
