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
- A Chapter aggregates one or more Scenes. In the current implementation this is often 1:1 (Migration 37 linked them bidirectionally), but the model supports 1:N.
- The幕前 editor writes into a Scene. The `chapters.content` field is a cached/aggregated view of its Scene(s) for export.

## Known gaps (intention — to be resolved)

### MemoryPack not yet wired into Writer Agent

`MemoryPack` (via `MemoryOrchestrator`) and `QueryPipeline` are implemented but **not yet injected into `AgentContext`**. The Writer Agent currently relies on `AgentContext.previous_chapters`, which is a simple time-sorted list of the last 5 chapter summaries. Semantic retrieval, graph expansion, and budgeted memory assembly are not yet part of the generation flow.

**Intended resolution:** `StoryContextBuilder` should call `QueryPipeline` + `MemoryOrchestrator` and attach the resulting `MemoryPack` to `AgentContext` (or merge it at prompt-assembly time). `previous_chapters` may be retired or absorbed into `working_memory`.

### Review / Anti-AI / Reading Power relationship

Three quality-evaluation systems exist with overlapping concerns but distinct purposes:

| System | Mechanism | Cost | When it runs | Purpose |
|--------|-----------|------|--------------|---------|
| **Anti-AI Review** | Rule engine (regex/heuristics) | Zero | Real-time, after Ghost Text acceptance | Detect AI clichés, uniformity, emotional labeling |
| **Pipeline Review** | LLM-driven, structured JSON | Token cost | On-demand, during 3-review pipeline | Deep editorial review with configurable dimensions |
| **Reading Power** | Rule + heuristics (partially implemented) | Near-zero | After chapter commit or on-demand | Evaluate reader retention (hooks, coolpoints, debt) |

**Intended architecture:** Layered coexistence (B). Anti-AI is the fast filter, Pipeline Review is the deep editor, Reading Power is the reader-retention analyst. They should feed each other: Anti-AI flags can be injected into Pipeline Review prompt as pre-known issues; Pipeline Review findings can update Reading Power debt ledger; Reading Power override contracts can constrain future Pipeline Reviews.

### Book Deconstruction storage

Deconstruction output currently lands in `reference_books` / `reference_characters` / `reference_scenes` (Migration 16/17), while genesis output lands in `narrative_*` tables (Migration 38). The "one-click convert to story" feature copies data between these disjoint schemas, breaking the "isomorphic pipeline" design promised in v5.3.0.

**Intended resolution:** Deprecate `reference_*` tables. Deconstruction elements should enter `narrative_*` tables with `ElementSource::Extracted`, coexisting with genesis elements (`ElementSource::Generated`). The only distinction between reference material and story project should be a `status` field (`reference` vs `active`). QueryPipeline should search both statuses so deconstructed novels become part of the author's creative memory.

### MCP tool registration in PlanExecutor

`PlanGenerator`'s prompt explicitly instructs the LLM to use `mcp.*` capabilities when external data is needed. However, `build_default_registry()` registers zero MCP capabilities. The validation logic in `planner/mod.rs:343` (`plan.steps.retain`) silently strips any MCP steps the LLM generates. Even if they were retained, `PlanExecutor` has no `CapabilitySource::McpTool` dispatch branch.

**Intended resolution:** MCP tools should be dynamically registered into a global `CapabilityRegistry` when MCP servers connect. `PlanGenerator` should reference the live registry (not a freshly-built static one). `PlanExecutor` must implement `McpTool` dispatch, forwarding calls to the MCP client layer.

### 3-review Pipeline implementation status

The v0.7.0 AI 3-review Pipeline (`Refine → Review → Finalize`) has an unbalanced implementation:

| Phase | Status | Detail |
|-------|--------|--------|
| Refine | TODO placeholder | `refine_draft` builds prompt but returns original content without LLM call |
| Review | TODO placeholder | `review_draft` returns fixed score 85.0 without LLM call |
| Finalize | Partial | State transitions + chapter sync implemented |
| PostProcess (kb_import) | Working | Calls `knowledge_base::import_text` to vector store |
| PostProcess (chapter_notes) | Working | Calls LLM to extract plot notes |
| PostProcess (character_cards) | Working | Calls LLM + JSON parsing to update character states |
| PostProcess (style_analysis) | TODO | Triggered every 5 chapters but empty implementation |

**Intended resolution:** Refine and Review must be wired to real LLM calls with structured JSON output parsing. The Draft/Revision/Review database schema and state machine are already in place and should be reused.

### LLM cancellation only works for streaming

`stream_generate` registers a `cancel_sender` and uses `tokio::select!` to break on cancellation. However, `generate_with_context_and_pipeline` (used by all non-streaming operations: Bootstrap, book deconstruction, AgentOrchestrator loops, PlanExecutor steps) never registers a cancel sender. Calling `cancel_generation` on a sync request is a no-op.

**Consequence:** The most expensive operations (multi-step LLM pipelines, long-form generation) are the ones that cannot be cancelled. Users think they stopped the operation; the backend keeps burning tokens.

**Intended resolution:** All LLM entry points must support cancellation. Even sync generation should wrap the HTTP request in a cancellable future (e.g. `reqwest`'s abort handle or a custom cancellation token) and register with `cancel_senders`.

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

`apply_chapter_commit` is currently an **explicit IPC command** requiring front-end to assemble and pass `outline_snapshot_json`, `review_result_json`, `fulfillment_result_json`, and `accepted_events_json`. It is **not automatically triggered** on `update_chapter` / save.

Consequences:
- Projection Writers (State/Index/Summary/Memory/Vector) only run when the user manually triggers commit.
- `auto_ingest_chapter` (triggered on save, 5-min cooldown) updates the knowledge graph and vector store independently, creating duplicate work with `VectorProjectionWriter`.
- Story System read-models drift out of sync with actual chapter content.

**Intended resolution:** `CHAPTER_COMMIT` should fire automatically after `update_chapter` succeeds, with async debounce (e.g. 30s idle delay). `auto_ingest_chapter` should be absorbed into the commit pipeline as a post-commit step or merged into `VectorProjectionWriter` / `MemoryProjectionWriter`, eliminating duplicate indexing.

### Orchestration layer boundaries

Three orchestration systems exist with overlapping responsibilities and no clear hierarchy:

| System | Trigger | Granularity | Current gap |
|--------|---------|-------------|-------------|
| **AgentOrchestrator** | `commands.rs` direct call | Single generation (write + inspect + rewrite) | Not invoked by `PlanExecutor` or `WorkflowScheduler` |
| **PlanExecutor** | LLM-generated plan | Multi-step dynamic plan (Capability calls) | Writing steps call `execute_writer_raw` directly, bypassing Orchestrator |
| **WorkflowScheduler** | Predefined template | Multi-step static DAG (workflow nodes) | "WriteChapter" node may also bypass Orchestrator |

**Intended architecture:** Three-layer hierarchy:
1. `WorkflowScheduler` owns predefined business processes (genesis, standard writing).
2. `PlanExecutor` owns open-ended user intent; any step that generates text must nest-call `AgentOrchestrator`.
3. `AgentOrchestrator` is the single gateway for all AI text generation; no raw writer call is allowed outside it.
