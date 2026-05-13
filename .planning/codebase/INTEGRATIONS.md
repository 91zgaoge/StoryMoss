# External Integrations

**Analysis Date:** 2026-05-11

## APIs & External Services

**LLM Providers:**
- OpenAI - Chat completions and embeddings
  - SDK/Client: `reqwest` HTTP client
  - Config: `src-tauri/src/llm/openai.rs`
  - Auth: API key via `LlmProfile.api_key`
  - Models: GPT-4, GPT-3.5-turbo, etc.

- Anthropic - Claude models for generation
  - SDK/Client: `reqwest` HTTP client
  - Config: `src-tauri/src/llm/anthropic.rs`
  - Auth: API key via `LlmProfile.api_key`
  - Models: Claude 3 Opus, Sonnet, Haiku

- Ollama - Local LLM inference
  - SDK/Client: `reqwest` HTTP client
  - Config: `src-tauri/src/llm/ollama.rs`
  - Auth: None (local)
  - Models: Configurable via API base URL

- DeepSeek - Chinese LLM provider
  - SDK/Client: OpenAI-compatible API
  - Config: `src-tauri/src/llm/service.rs` (LlmProvider::DeepSeek)
  - Auth: API key

- Qwen - Alibaba LLM provider
  - SDK/Client: OpenAI-compatible API
  - Config: `src-tauri/src/llm/service.rs` (LlmProvider::Qwen)
  - Auth: API key

**OAuth & Authentication:**
- Google OAuth 2.0
  - Implementation: `src-tauri/src/auth/oauth.rs`
  - PKCE flow with Authorization Code
  - Env vars: `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`
  - Scopes: openid, email, profile

- GitHub OAuth 2.0
  - Implementation: `src-tauri/src/auth/oauth.rs`
  - PKCE flow with Authorization Code
  - Env vars: `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`
  - Scopes: openid, email, profile

- WeChat OAuth (reserved)
  - Env vars: `WECHAT_CLIENT_ID`, `WECHAT_CLIENT_SECRET`
  - Status: Not yet implemented

- QQ OAuth (reserved)
  - Env vars: `QQ_CLIENT_ID`, `QQ_CLIENT_SECRET`
  - Status: Not yet implemented

## Data Storage

**Databases:**
- SQLite (primary)
  - Type: Embedded relational database
  - Connection: `src-tauri/src/db/connection.rs`
  - Client: `rusqlite` with `r2d2` connection pooling
  - Location: `{app_data_dir}/cinema_ai.db`
  - Pool size: 5 connections max
  - Tables: stories, characters, chapters, scenes, embeddings, ai_operations, etc.

- LanceDB (vector store)
  - Type: Vector database for semantic search
  - Connection: `src-tauri/src/vector/lancedb_store.rs`
  - Client: `lancedb` with Arrow data structures
  - Location: `{app_data_dir}/lancedb/`
  - Embedding dimension: 384
  - Index type: ANN (Approximate Nearest Neighbor)
  - Use case: Semantic search for story content, character descriptions

**File Storage:**
- Local filesystem only
  - App data directory: Platform-specific (Windows: `%APPDATA%`, macOS: `~/Library/Application Support`, Linux: `~/.config`)
  - Exports: PDF, EPUB, ZIP formats
  - Temp files: `tempfile` crate for test/processing

**Caching:**
- In-memory caching via Rust `Lazy` statics
  - `INGEST_COOLDOWN` - Prevents duplicate LLM calls (5-minute cooldown)
  - `OAUTH_STATE_STORE` - OAuth flow state management
  - `DB_POOL` - Database connection pool
  - `APP_CONFIG` - Application configuration cache

## Authentication & Identity

**Auth Provider:**
- Custom OAuth2 implementation
  - Implementation: `src-tauri/src/auth/oauth.rs`
  - Approach: PKCE + Authorization Code flow (RFC 7636)
  - Token storage: In-memory during session
  - JWT: `jsonwebtoken` crate for token validation
  - Password hashing: `argon2` for local accounts

**Session Management:**
- Implementation: `src-tauri/src/auth/session.rs`
- Storage: In-memory session store
- Expiration: Configurable via JWT claims

## Monitoring & Observability

**Error Tracking:**
- None detected - No external error tracking service

**Logs:**
- Approach: Structured logging via `tracing` crate
  - Output: JSON format with timestamps
  - Appender: File-based via `tracing-appender`
  - Levels: debug, info, warn, error
  - Modules: Prefixed with `[ModuleName]` for filtering
  - Example: `[LLM] Failed to reload config: {error}`

**Tracing:**
- Framework: `tracing` with `tracing-subscriber`
- Features: env-filter, JSON output, local-time timestamps
- Bridge: `tracing-log` for compatibility with `log` crate

## CI/CD & Deployment

**Hosting:**
- GitHub Releases - Distribution point for desktop app
  - Auto-update endpoint: `https://github.com/91zgaoge/StoryForge/releases/latest/download/latest.json`
  - Configured in: `tauri.conf.json` plugins.updater.endpoints

**CI Pipeline:**
- None detected - No GitHub Actions or CI service configured

**Auto-Update:**
- Tauri Updater plugin v2
  - Config: `src-tauri/tauri.conf.json`
  - Public key: Base64-encoded minisign key
  - Dialog: Disabled (silent updates)
  - Targets: MSI, NSIS, DEB, RPM, DMG, APP

## Environment Configuration

**Required env vars:**
- `POSTGRES_USER` - Database user
- `POSTGRES_PASSWORD` - Database password
- `POSTGRES_DB` - Database name
- `JWT_SECRET` - JWT signing key (min 32 chars)
- `FRONTEND_URL` - Frontend URL for CORS
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` - Google OAuth
- `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` - GitHub OAuth

**Optional env vars:**
- `WECHAT_CLIENT_ID`, `WECHAT_CLIENT_SECRET` - WeChat OAuth (reserved)
- `QQ_CLIENT_ID`, `QQ_CLIENT_SECRET` - QQ OAuth (reserved)

**Secrets location:**
- `.env` file (git-ignored)
- `.env.example` - Template with placeholder values
- LLM API keys: Stored in `config.yaml` (app data directory)
- OAuth tokens: In-memory during session

## Webhooks & Callbacks

**Incoming:**
- OAuth callback handler: `src-tauri/src/auth/commands.rs::oauth_callback`
  - Endpoint: Local redirect server on dynamic port
  - Flow: Authorization Code with PKCE
  - Callback URL: `http://localhost:{port}/callback`

**Outgoing:**
- None detected - No outgoing webhooks

## MCP (Model Context Protocol)

**Implementation:**
- SDK: `rmcp` 0.8 with server and client features
- Config: `src-tauri/src/mcp/`
- Purpose: Standardized protocol for LLM tool integration
- Status: Integrated but specific tools not detailed in stack analysis

## WebSocket

**Real-time Communication:**
- Framework: `tokio-tungstenite` 0.29.0
- Purpose: Collaborative editing support
- Implementation: `src-tauri/src/collab/websocket.rs`
- Server: Listens on configurable address
- Features: Message broadcasting, connection management

## Embeddings

**Generation:**
- Method: Local embedding generation via LLM providers
- Implementation: `src-tauri/src/embeddings.rs`
- Dimension: 384-dimensional vectors
- Storage: LanceDB vector store
- Use cases: Semantic search, similarity matching

---

*Integration audit: 2026-05-11*
