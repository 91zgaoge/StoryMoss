# Context: Backend

Glossary for the Rust core layer.

## Terms

| Term | Definition | Avoid using as synonym |
|------|------------|------------------------|
| **Scene** | Drama-conflict-driven narrative unit. The primary logical storytelling unit. Carries dramatic goal, external pressure, conflict type. | "chapter" (see distinction below) |
| **Chapter** | Physical storage/publishing unit. An aggregation of one or more Scenes. Holds the final rendered text for export. | "scene" (see distinction below) |
| **Bootstrap / Genesis** | One-click novel world creation pipeline. 7-step process: concept → first scene (with dramatic goal, conflict, characters) → world-building → outline → characters → scenes → foreshadowing → knowledge graph. v0.23.74: scene-first prompts via `narrative_first_scene_generate`. Synonymous terms. | "creation", "wizard" |
| **Book Deconstruction** | Reverse pipeline: analyze an existing novel (txt/pdf/epub) and convert it into a story project with extracted narrative elements. | "import", "parse" |
| **Ingest Pipeline** | Two-step chain: analyze raw content (chapter text) → generate knowledge → save to knowledge graph + vector store. Triggered after chapter save/update. | "index", "import" |
| **Query Pipeline** | 5-stage memory retrieval: token search → semantic search → fusion → graph expansion → budget control → context assembly. Produces a `MemoryPack`. | "search", "retrieval" |
| **MemoryPack** | Assembled memory context fed into AI generation prompts. Combines working, episodic, and semantic memory within a token budget. | "context", "prompt" |
| **StyleDNA** | 6-dimensional quantitative style fingerprint (sentence length, dialogue ratio, metaphor density, inner monologue, emotional exposure, rhythm). Can be user-selected or evolved from feedback. | "style", "template" |
| **SCENE_COMMIT** | Post-write source-of-truth event triggered by Scene save/update. Drives 5 Projection Writers to update derived state (state deltas, entity index, summary, memory, vector). The canonical trigger unit is always the Scene, never the Chapter. | "save", "flush", "chapter commit" |
| **Projection Writer** | One of 5 state-derivation workers triggered by SCENE_COMMIT: State Writer, Index Writer, Summary Writer, Memory Writer, Vector Writer. All operate on Scene-level deltas. | "handler", "listener" |
| **Foreshadowing** | Narrative device tracked with setup/payoff lifecycle. The `ForeshadowingTracker` monitors time windows and raises alerts when payoffs are overdue. | "hint", "setup" |
| **Capability** | Self-describing agent skill registered in `CapabilityRegistry`. Natural-language description lets the LLM decide when to invoke it. | "function", "tool" |
| **MCP** | Model Context Protocol. External tool integration (e.g. DuckDuckGo search) exposed to the LLM planner. | "plugin", "extension" |
| **AgentOrchestrator** | Single-step writing quality loop: Writer → Inspector → StyleChecker → Rewrite. The gateway for all AI text generation. | "agent", "workflow" |
| **PlanExecutor** | Dynamic-plan executor. Runs LLM-generated plans step-by-step, resolving dependencies between Capabilities. | "executor", "scheduler" |
| **WorkflowScheduler** | Static-workflow engine. Runs predefined DAG workflows (e.g. genesis, standard writing) with retries and queuing. | "pipeline", "engine" |

## Scene vs Chapter distinction

> **Core design principle**: Scene and Chapter are not aliases. They are distinct concepts that must coexist.

- **Scene** — The *creative unit*. The author's narrative atom: dramatic goal, conflict, beats, character arcs. Scenes are where the art happens. They are never optional.
- **Chapter** — The *physical storage / publishing unit*. The reader's consumption boundary: linear text, page breaks, export artifacts. Chapters exist only because books need pagination.
- A Chapter **aggregates** one or more Scenes, ordered by `sequence_number`.
- **Current implementation: v0.23.74 — Scene-first architecture complete.** `scenes.content` is the single source of truth for all narrative content. `chapters.content` is a read-only aggregated projection computed from Scene(s). One Chapter aggregates 1+ Scenes via `scenes.chapter_id` FK.
- The 幕前 editor renders pure prose text — Scene contents are concatenated seamlessly with no divider markers. Users read/write as a continuous book page.
- `chapters.content` is never directly written; all content writes go through `SceneRepository::update()`.
- `ChapterRepository::get_content(chapter_id)` aggregates Scene contents by `sequence_number` into a single continuous string.
- `SceneCommitDebouncer` (30s idle) triggers `SceneCommitService::auto_commit` from Scene saves, not Chapter saves.

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

**Resolved in v0.23 (Phase 6.4).** `reference_characters` and `reference_scenes` have been dropped (Migration V100). Deconstruction elements now enter `narrative_*` tables with `ElementSource::Extracted` and `status = 'reference'`, coexisting with genesis elements (`ElementSource::Generated`). `reference_books` remains as the metadata/aggregation table for uploaded reference novels. The "one-click convert to story" feature activates these reference elements by setting `status = 'active'`.

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

### Business model: subscription unlocks features, not model quotas

**Status: Resolved.** StoryForge does not bill for model usage. Users supply their own API keys or run local models. The software's monetization is subscription-based: subscribed users unlock full functionality; free users have feature limitations.

**Consequence for code:** The existing `check_platform_quota` and per-feature quota checks (`auto_write_quota`, `auto_revise_quota`) in `agents/commands.rs` and `llm/service.rs` contradict this principle. They assume a freemium model where the software controls model consumption. This logic should be removed or refactored into feature-gating (e.g., "Bootstrap is a Pro feature") rather than consumption-metering.

**Model access is entirely user-determined.** The software provides the LLM adapter layer but does not intervene in model billing, token counting, or provider quotas.

### AppError full migration (in progress)

**Status: Infrastructure complete, internal API migration in progress (~10%).**

`AppError` enum is defined (`error.rs`) with 9 variants + IPC serialization (`{ code, message, data }`) + `From` converters for 12 external error types. Quota check point (`check_platform_quota`) already returns `Result<(), AppError>`.

**Gap:** 257 residual `Result<T, String>` across 43 files, including `LlmService` core APIs (`generate_with_profile`, `generate_stream`, `generate_with_context_and_pipeline`). `From<String>` fallback exists but discards structured context before it reaches the IPC boundary.

**Resolution strategy (Option A — full migration):**
- Migrate all internal APIs from `Result<T, String>` to `Result<T, AppError>` in a single release cycle.
- Start with `LlmService` (the root of all AI calls), then cascade through `AgentOrchestrator` → `PlanExecutor` → `WorkflowScheduler` → `Pipeline` → `Bootstrap`.
- `StoryContextBuilder` is a priority target: replace silent `unwrap_or_default()` with `Err(AppError::ContextUnavailable)` so degraded context is visible to the frontend.
- No gradual fallback. `Result<T, String>` is eliminated, not deprecated.

### StoryContextBuilder error classification

**Status: Resolved.** Two-axis classification:

| Axis | Fatal (return `Err`) | Degradable (empty default + warning) |
|------|---------------------|--------------------------------------|
| **Query failure** (DB error, connection lost) | All queries | None |
| **Empty data** (user never created the data) | Scene-dependent: combat/dialogue scenes require characters; exposition/monologue scenes do not | World rules (optional for short/realistic fiction), style DNA (falls back to genre default), relevant entities |

`AgentContext.warnings` is currently write-only (no frontend consumer). Resolution: either pipe warnings to frontstage status bar, or remove the field and rely on `Err` for everything worth surfacing.

### SCENE_COMMIT trigger mechanism

**Status: Implemented (v0.23.74).** Commit granularity is **Scene-only**.

- **Scene-level commit** (text edits, content changes): `SceneCommitDebouncer` fires on Scene save/update via `SceneService::on_scene_updated()`, debounced 30s idle delay. Drives all 5 Projection Writers.
- **Chapter-level events** (structural changes: divider insert/delete, scene reorder): do **not** trigger Projection Writers. They only update the Chapter's read-only aggregated content view.
- `ChapterCommitDebouncer` (v0.7.1–v0.23.73) has been superseded by `SceneCommitDebouncer`. `SceneCommitService` is the canonical commit service.
- Projection Writers operate on Scene deltas (`state_deltas_json`, `entity_deltas_json`, etc.). Chapter has no deltas of its own.

### Orchestration layer boundaries

**Status: Mostly resolved.** The code now largely follows the intended three-layer hierarchy:

| System | Trigger | Granularity | Current state |
|--------|---------|-------------|---------------|
| **AgentOrchestrator** | Direct call or nested | Single generation (write + inspect + rewrite) | **Single gateway for text generation** (`GenerationMode::Fast/Full/Refine/Review`) |
| **PlanExecutor** | LLM-generated plan | Multi-step dynamic plan | `execute_writer` calls `AgentService::execute_task` → `execute_writer` → `AgentOrchestrator` (indirectly nested) |
| **WorkflowScheduler** | Predefined template | Multi-step static DAG | `WriteChapter` directly calls `AgentOrchestrator`; `Revise` calls `AgentService::execute_task` → `AgentOrchestrator` |

**Current gap:** `AgentService::execute_writer` is an unnecessary middle layer that creates its own `AgentOrchestrator`. All upper layers should call `AgentOrchestrator::generate` directly. Hooks (BeforeAiWrite/AfterAiWrite) should move into `AgentOrchestrator`.

**Intended resolution:** Deprecate `AgentService::execute_writer`. Move hooks into `AgentOrchestrator`. All upper layers (`PlanExecutor`, `WorkflowScheduler`, IPC commands) call `AgentOrchestrator::generate(task, mode)` directly.
