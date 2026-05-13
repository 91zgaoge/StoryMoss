# Technology Stack

**Analysis Date:** 2026-05-11

## Languages

**Primary:**
- Rust 2021 edition - Backend/Tauri desktop application core
- TypeScript 5.8.3 - Frontend React application
- JavaScript - Build scripts and test utilities

**Secondary:**
- YAML - Configuration files (serde_yaml 0.9)
- JSON - Data serialization and configuration

## Runtime

**Environment:**
- Tauri 2.4 - Desktop application framework (Rust + WebView)
- Node.js - Frontend development and build tooling
- Tokio 1.44 - Async runtime for Rust backend

**Package Manager:**
- npm - Frontend dependencies
- Cargo - Rust dependencies
- Lockfile: `package-lock.json` (frontend), `Cargo.lock` (backend)

## Frameworks

**Core:**
- Tauri 2.4 - Desktop application framework with IPC bridge
- React 18.3.1 - Frontend UI framework
- Vite 6.2.5 - Frontend build tool and dev server

**UI & Styling:**
- Tailwind CSS 3.4.17 - Utility-first CSS framework
- Framer Motion 12.38.0 - Animation library
- Lucide React 0.487.0 - Icon library
- TipTap 3.22.3 - Rich text editor with extensions
- React Flow 11.11.4 - Node-based UI library
- dnd-kit 6.3.1 - Drag and drop toolkit

**State Management:**
- Zustand 5.0.3 - Lightweight state management
- React Query 5.71.0 - Server state management
- React Hook Form 7.72.1 - Form state management

**Testing:**
- Vitest 4.1.4 - Unit/integration test runner
- Playwright 1.59.1 - E2E testing framework
- PropTest 1 - Property-based testing (Rust)
- Serial Test 3 - Test serialization (Rust)

**Build/Dev:**
- TypeScript 5.8.3 - Type checking
- ESLint 8.57.0 - Linting
- Prettier - Code formatting (via Tailwind)
- PostCSS 8.5.3 - CSS processing
- Autoprefixer 10.4.21 - CSS vendor prefixes

## Key Dependencies

**Critical:**
- reqwest 0.12.4 - HTTP client for API calls (OpenAI, Anthropic, Ollama)
- serde/serde_json 1 - Serialization/deserialization
- tokio 1.44 - Async runtime with full features
- rusqlite 0.39 - SQLite database driver with bundled SQLite
- r2d2/r2d2_sqlite 0.8/0.33 - Connection pooling for SQLite

**Vector & Embeddings:**
- lancedb 0.27 - Vector database for semantic search
- arrow-array/arrow-schema 57 - Apache Arrow data structures

**LLM Integration:**
- oauth2 5.0 - OAuth2 authentication flow
- jsonwebtoken 9 - JWT token handling
- argon2 0.5 - Password hashing

**File Processing:**
- pdf-extract 0.7 - PDF text extraction
- epub 2.1 - EPUB parsing
- printpdf 0.7 - PDF generation
- epub-builder 0.7 - EPUB generation
- zip 0.6 - ZIP file handling
- tera 1.20 - Template engine for exports

**Utilities:**
- chrono 0.4 - Date/time handling
- uuid 1.16 - UUID generation
- regex 1.11 - Regular expressions
- walkdir 2 - Directory traversal
- notify 8.0 - File system watching
- cron 0.15 - Cron expression parsing
- sha2/hex 0.10/0.4 - File hashing

**Tauri Plugins (v2.x):**
- tauri-plugin-fs 2 - File system access
- tauri-plugin-dialog 2 - Native dialogs
- tauri-plugin-shell 2 - Shell command execution
- tauri-plugin-http 2 - HTTP requests
- tauri-plugin-updater 2 - Auto-update functionality

**MCP (Model Context Protocol):**
- rmcp 0.8 - MCP server/client implementation

**Logging:**
- log 0.4 - Logging facade
- tracing 0.1 - Structured tracing
- tracing-subscriber 0.3 - Tracing implementation with JSON output
- tracing-appender 0.2 - Log file appending
- tracing-log 0.2 - Log bridge

**WebSocket:**
- tokio-tungstenite 0.29.0 - WebSocket implementation
- futures-util 0.3.32 - Async utilities

**Frontend HTTP:**
- @tauri-apps/plugin-http 2.4.0 - HTTP client for Tauri
- react-hot-toast 2.5.2 - Toast notifications

## Configuration

**Environment:**
- `.env` file for secrets (PostgreSQL credentials, JWT secret, OAuth keys)
- `tauri.conf.json` - Tauri application configuration
- `vite.config.ts` - Frontend build configuration
- `tsconfig.json` - TypeScript compiler options
- `Cargo.toml` - Rust project manifest

**Key configs required:**
- `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` - Database credentials
- `JWT_SECRET` - JWT signing key (min 32 chars)
- `FRONTEND_URL` - CORS configuration
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` - Google OAuth
- `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` - GitHub OAuth
- LLM API keys (OpenAI, Anthropic, etc.) - Configured via UI

**Build:**
- `tauri.conf.json` - Defines build process, window configuration, updater endpoints
- `vite.config.ts` - Defines frontend build output, path aliases, dev server
- `tsconfig.json` - TypeScript compilation targets ES2020, strict mode enabled

## Platform Requirements

**Development:**
- Node.js (for frontend tooling)
- Rust toolchain (1.70+)
- Tauri CLI
- Windows 10+ / macOS 10.13+ / Linux (GTK 3.6+)

**Production:**
- Tauri desktop application (Windows MSI/NSIS, macOS DMG/APP, Linux DEB/RPM)
- Auto-update via GitHub releases (configured in `tauri.conf.json`)
- SQLite database (bundled, stored in app data directory)
- LanceDB vector store (local file-based)

---

*Stack analysis: 2026-05-11*
