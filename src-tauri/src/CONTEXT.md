# Context: Backend

Glossary for the Rust core layer.

## Terms

| Term | Definition | Avoid using as synonym |
|------|------------|------------------------|
| **Scene** | Drama-conflict-driven narrative unit. The primary logical storytelling unit. Carries dramatic goal, external pressure, conflict type. | "chapter" (see distinction below) |
| **Chapter** | Physical storage/publishing unit. An aggregation of one or more Scenes. Holds the final rendered text for export. | "scene" (see distinction below) |
| **Bootstrap / Genesis** | One-click novel world creation pipeline. 7-step process: concept → first chapter → world-building → outline → characters → scenes → foreshadowing → knowledge graph. Synonymous terms. | "creation", "wizard" |
| **Book Deconstruction** | Reverse pipeline: analyze an existing novel (txt/pdf/epub) and convert it into a story project with extracted narrative elements. | "import", "parse" |
| **Ingest Pipeline** | Two-step chain: analyze raw content (chapter text) → generate knowledge → save to knowledge graph + vector store. Triggered after chapter save/update. | "index", "import" |
| **Query Pipeline** | 5-stage memory retrieval: token search → semantic search → fusion → graph expansion → budget control → context assembly. Produces a `MemoryPack`. | "search", "retrieval" |
| **MemoryPack** | Assembled memory context fed into AI generation prompts. Combines working, episodic, and semantic memory within a token budget. | "context", "prompt" |
| **StyleDNA** | 6-dimensional quantitative style fingerprint (sentence length, dialogue ratio, metaphor density, inner monologue, emotional exposure, rhythm). Can be user-selected or evolved from feedback. | "style", "template" |
| **CHAPTER_COMMIT** | Post-write source-of-truth event that triggers 5 Projection Writers to update derived state. | "save", "flush" |
| **Projection Writer** | One of 5 state-derivation workers triggered by CHAPTER_COMMIT: State Writer, Index Writer, Summary Writer, Memory Writer, Vector Writer. | "handler", "listener" |
| **Foreshadowing** | Narrative device tracked with setup/payoff lifecycle. The `ForeshadowingTracker` monitors time windows and raises alerts when payoffs are overdue. | "hint", "setup" |
| **Capability** | Self-describing agent skill registered in `CapabilityRegistry`. Natural-language description lets the LLM decide when to invoke it. | "function", "tool" |
| **MCP** | Model Context Protocol. External tool integration (e.g. DuckDuckGo search) exposed to the LLM planner. | "plugin", "extension" |
| **AgentOrchestrator** | Single-step writing quality loop: Writer → Inspector → StyleChecker → Rewrite. The gateway for all AI text generation. | "agent", "workflow" |
| **PlanExecutor** | Dynamic-plan executor. Runs LLM-generated plans step-by-step, resolving dependencies between Capabilities. | "executor", "scheduler" |
| **WorkflowScheduler** | Static-workflow engine. Runs predefined DAG workflows (e.g. genesis, standard writing) with retries and queuing. | "pipeline", "engine" |

## Scene vs Chapter distinction

- **Scene** is the author's creative unit: dramatic goal, conflict, beats.
- **Chapter** is the reader's consumption unit: linear text, page breaks.
- A Chapter aggregates one or more Scenes. **Current implementation is strictly 1:1** (`chapters.scene_id` → `scenes.id`, single-valued FK, Migration 37). **Target architecture is 1:N** (`scenes.chapter_id` FK, one Chapter contains multiple Scenes ordered by `sequence_number`).
- The 幕前 editor currently writes into a single Scene. In 1:N mode, it will aggregate multiple Scenes into a continuous editing surface with `scene-divider` Nodes.
- The `chapters.content` field is a cached/aggregated view of its Scene(s) for export.

## Known gaps (intention — to be resolved)

### MemoryPack not yet wired into Writer Agent

`MemoryPack` (via `MemoryOrchestrator`) and `QueryPipeline` are implemented but **not yet injected into `AgentContext`**. The Writer Agent currently relies on `AgentContext.previous_chapters`, which is a simple time-sorted list of the last 5 chapter summaries. Semantic retrieval, graph expansion, and budgeted memory assembly are not yet part of the generation flow.

**Status: Resolved.** `MemoryPack` injection architecture determined:
- `GenerationMode::Fast` (Ghost Text): lightweight context (`previous_chapters` cache), no QueryPipeline overhead.
- `GenerationMode::Full` (standard writing, chapter generation): full `QueryPipeline` + `MemoryOrchestrator` → `MemoryPack` injected into `AgentContext`.
- `previous_chapters` absorbed into `MemoryPack.working_memory`.
- **Pending implementation:** wiring `StoryContextBuilder` to call `QueryPipeline` + `MemoryOrchestrator` for Full mode.

### Review / Anti-AI / Reading Power relationship

Three quality-evaluation systems exist with overlapping concerns but distinct purposes:

| System | Mechanism | Cost | When it runs | Purpose |
|--------|-----------|------|--------------|---------|
| **Anti-AI Review** | Rule engine (regex/heuristics) | Zero | Real-time, after Ghost Text acceptance | Detect AI clichés, uniformity, emotional labeling |
| **Pipeline Review** | LLM-driven, structured JSON | Token cost | On-demand, during 3-review pipeline | Deep editorial review with configurable dimensions |
| **Reading Power** | Rule + heuristics (partially implemented) | Near-zero | After chapter commit or on-demand | Evaluate reader retention (hooks, coolpoints, debt) |

**Status: Resolved.** Three-layer coexistence architecture determined:

| Linkage | Direction | Mechanism |
|---------|-----------|-----------|
| Anti-AI → Pipeline Review | Flags as pre-known issues | `text_annotations` (`annotation_type: "anti_ai_flag"`) injected into `build_review_prompt` |
| Pipeline Review → Reading Power | Quality defects become debt | Critical/high `review_issues` (continuity/foreshadow/pacing) auto-create `ChaseDebt` entries via `DebtManager` |
| Reading Power → Pipeline Review | Constraints steer review focus | Pending `OverrideContract`s injected into `review_focus` parameter of `build_review_prompt` |
| Reading Power → Anti-AI | Style drift detection | `StyleDNA` deviation threshold triggers "style drift" flags in Anti-AI |

**Pending implementation:** `DebtManager::create_debt` needs `DebtSource` enum; `build_review_prompt` needs contract injection.

### Book Deconstruction storage

Deconstruction output currently lands in `reference_books` / `reference_characters` / `reference_scenes` (Migration 16/17), while genesis output lands in `narrative_*` tables (Migration 38). The "one-click convert to story" feature copies data between these disjoint schemas, breaking the "isomorphic pipeline" design promised in v5.3.0.

**Intended resolution:** Deprecate `reference_*` tables. Deconstruction elements should enter `narrative_*` tables with `ElementSource::Extracted`, coexisting with genesis elements (`ElementSource::Generated`). The only distinction between reference material and story project should be a `status` field (`reference` vs `active`). QueryPipeline should search both statuses so deconstructed novels become part of the author's creative memory.

### MCP tool registration in PlanExecutor

`PlanGenerator`'s prompt explicitly instructs the LLM to use `mcp.*` capabilities when external data is needed. However, `build_default_registry()` registers zero MCP capabilities. The validation logic in `planner/mod.rs:343` (`plan.steps.retain`) silently strips any MCP steps the LLM generates. Even if they were retained, `PlanExecutor` has no `CapabilitySource::McpTool` dispatch branch.

**Intended resolution:** MCP tools should be dynamically registered into a global `CapabilityRegistry` when MCP servers connect. `PlanGenerator` should reference the live registry (not a freshly-built static one). `PlanExecutor` must implement `McpTool` dispatch, forwarding calls to the MCP client layer.

### 3-review Pipeline implementation status

The v0.7.0 AI 3-review Pipeline (`Refine → Review → Finalize`) implementation status:

| Phase | Status | Detail |
|-------|--------|--------|
| Refine | **Working** | Full LLM call (`pipeline/refine.rs`), prompt building, revision record creation, diff calculation |
| Review | **Working** | Full LLM call (`pipeline/review.rs`), structured JSON parsing with fallback, review record save |
| Finalize | Partial | State transitions + chapter sync implemented |
| PostProcess (kb_import) | Working | Calls `knowledge_base::import_text` to vector store |
| PostProcess (chapter_notes) | Working | Calls LLM to extract plot notes |
| PostProcess (character_cards) | Working | Calls LLM + JSON parsing to update character states |
| PostProcess (style_analysis) | TODO | Triggered every 5 chapters but empty implementation (3 TODOs in `post_process.rs:354`) |

**Pending implementation:** Only `style_analysis` remains empty. It should read last 5 chapters, compute StyleDNA 6-dim vector, save snapshot + delta.

### LLM cancellation only works for streaming

`generate_with_context_and_pipeline` already supports cancellation via `tokio::select!` + `cancel_rx` (`llm/service.rs:453`). The `cancel_senders` HashMap is registered and listened to.

**Current gap:** `request_id` is generated internally and **not returned to callers**. Frontend and upper layers (Bootstrap, PlanExecutor, WorkflowScheduler) have no way to know which `request_id` to pass to `cancel_generation()`.

**Intended resolution:** `LlmService::generate_with_context_and_pipeline` should return `(String, Result<LlmResponse, String>)` where the first `String` is the `request_id`. `AgentOrchestrator` stores it in `AgentTask.metadata`. Long-running operations expose `request_id` through `PipelineCallbacks` so the frontend can cancel.

### Quota checks bypassed by most AI entry points

Quota enforcement only exists for `auto_write` and `auto_revise` IPC commands. All other AI entry points (`execute_agent_task`, `execute_smart_agent`, `PlanExecutor`, `WorkflowScheduler`, Bootstrap, book deconstruction, 3-review pipeline) call `LlmService` directly without any quota check.

**Consequence:** Free users can bypass daily limits by using `/` menu commands, WenSiPanel dialog, Bootstrap genesis, or any plan/workflow path. The "analysis free, modification charged" strategy is unenforced.

**Intended resolution:** Move quota enforcement into `LlmService.generate()` (and `stream_generate()`). Before every LLM call, check the user's tier and the operation type's remaining quota. Return a structured `QuotaExceeded` error that frontends can translate into upgrade prompts. All upper layers (Agent, PlanExecutor, WorkflowScheduler, Bootstrap) inherit enforcement automatically.

### Error handling: 450 `map_err(|e| e.to_string())` across 35 files

Every internal function returns `Result<T, String>`, erasing typed errors. Frontends receive plain strings via Tauri's IPC boundary and cannot distinguish "quota exhausted" from "model timeout" from "DB locked".

**Consequence:** UX cannot adapt to error type. A quota error should show an upgrade button; a model timeout should suggest checking Ollama; a DB lock should suggest retrying. All three currently show the same generic error toast.

**Intended resolution:** Define a unified `AppError` enum (`QuotaExceeded { feature, limit, used, resets_at }`, `LlmTimeout { model, elapsed }`, `DbLocked { table }`, `ValidationFailed { field, reason }`, etc.). All internal APIs return `Result<T, AppError>`. IPC commands serialize `AppError` into structured JSON `{ code, message, data }`. Frontends match on `code` to render appropriate recovery UI.

### Silent failure pattern in StoryContextBuilder and beyond

`StoryContextBuilder::build()` catches every database error with `unwrap_or_else(|e| { log::warn!(...); default })`. The method signature returns `Result<AgentContext, String>` but practically never returns `Err`. Characters, scenes, world-building, and style queries that fail return empty defaults.

**Consequence:** AI generates content against a degraded or empty context. The user sees "the AI suddenly forgot my characters" with no error indication. The same `let _ =` silent-drop pattern appears in state_sync emissions, auto_ingest, and Projection Writers.

**Intended resolution:** Distinguish recoverable vs fatal errors. Missing core data (characters, story metadata) should be fatal — return `Err` so the frontend can show "context unavailable, please check your story data". Auxiliary data (relevant entities, optional style) may degrade but must populate an `AgentContext.warnings` vector that the frontend displays as "generating with limited context".

### CHAPTER_COMMIT trigger mechanism

**Status: Resolved in v0.7.1.** `ChapterCommitService::auto_commit` now handles debounced auto-commit (30s idle delay). `auto_ingest_chapter` has been removed, eliminating duplicate indexing with `VectorProjectionWriter`. Projection Writers run automatically on chapter save.

**Remaining refinement:** In 1:N mode, `CHAPTER_COMMIT` granularity needs clarification — Scene-level commit for text edits, Chapter-level commit for structural changes (divider insert/delete).

### Orchestration layer boundaries

**Status: Mostly resolved.** The code now largely follows the intended three-layer hierarchy:

| System | Trigger | Granularity | Current state |
|--------|---------|-------------|---------------|
| **AgentOrchestrator** | Direct call or nested | Single generation (write + inspect + rewrite) | **Single gateway for text generation** (`GenerationMode::Fast/Full/Refine/Review`) |
| **PlanExecutor** | LLM-generated plan | Multi-step dynamic plan | `execute_writer` calls `AgentService::execute_task` → `execute_writer` → `AgentOrchestrator` (indirectly nested) |
| **WorkflowScheduler** | Predefined template | Multi-step static DAG | `WriteChapter` directly calls `AgentOrchestrator`; `Revise` calls `AgentService::execute_task` → `AgentOrchestrator` |

**Current gap:** `AgentService::execute_writer` is an unnecessary middle layer that creates its own `AgentOrchestrator`. All upper layers should call `AgentOrchestrator::generate` directly. Hooks (BeforeAiWrite/AfterAiWrite) should move into `AgentOrchestrator`.

**Intended resolution:** Deprecate `AgentService::execute_writer`. Move hooks into `AgentOrchestrator`. All upper layers (`PlanExecutor`, `WorkflowScheduler`, IPC commands) call `AgentOrchestrator::generate(task, mode)` directly.
