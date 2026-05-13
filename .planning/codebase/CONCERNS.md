# Codebase Concerns

**Analysis Date:** 2026-05-11

## Tech Debt

### Database Foreign Key Constraints Not Enforced (P0 - Critical)

**Issue:** SQLite foreign key constraints are defined but not enforced at runtime. The r2d2 connection pool does not execute `PRAGMA foreign_keys = ON` when creating connections, causing `FOREIGN KEY ... ON DELETE CASCADE` declarations to be ignored.

**Files:** 
- `src-tauri/src/db/connection.rs` (lines 38-51, pool initialization)
- `src-tauri/src/tests/bug_condition_v57.rs` (test C_1_1 validates this bug exists)

**Impact:** 
- Deleting a story leaves orphaned records in 20+ related tables (chapters, characters, scenes, kg_entities, kg_relations, foreshadowing_tracker, world_buildings, story_outlines, character_relationships, etc.)
- Orphaned data pollutes subsequent queries and causes referential integrity violations
- Knowledge graph queries return dangling references

**Fix approach:** 
Enable foreign keys via `r2d2_sqlite::SqliteConnectionManager::with_init` callback to execute `PRAGMA foreign_keys = ON` on each connection. Alternatively, implement explicit cascading deletes in transaction handlers for `delete_story`, `delete_character`, and related operations.

---

### Incomplete Cascade Delete Operations (P0 - Critical)

**Issue:** Delete operations (`delete_story`, `delete_character`) only delete the primary record without explicitly cleaning related tables or relying on working foreign key cascades.

**Files:**
- `src-tauri/src/db/repositories_v3.rs` (SceneRepository, CharacterRepository implementations)
- `src-tauri/src/commands_v3.rs` (delete command handlers)

**Impact:**
- `delete_story` leaves chapters, characters, scenes, world_buildings, kg_entities, kg_relations, foreshadowing_tracker, story_outlines, character_relationships, narrative_* records orphaned
- `delete_character` leaves character_relationships (as source or target), canonical_character_states, and scene.characters_present references intact
- Subsequent queries on related entities return invalid data

**Fix approach:**
Wrap delete operations in transactions that explicitly clean related tables in dependency order, or ensure foreign key cascades are enabled (see above). Emit `storyDeleted` + `dataRefresh(all)` or `characterDeleted` events after successful deletion.

---

### Missing IPC Commands for Character Relationships (P1 - Functional Gap)

**Issue:** Frontend can only read character relationships via `get_character_relationships`, but no IPC commands exist to create, update, or delete them. The "Relationships" tab in backstage cannot modify data.

**Files:**
- `src-tauri/src/commands_v3.rs` (only `get_character_relationships` exposed, no create/update/delete)
- `src-frontend/src/pages/Characters.tsx` (manual `invalidateQueries` workaround)

**Impact:**
- Users cannot modify character relationships from backstage UI
- Frontend must manually refresh queries instead of receiving sync events
- Other pages and frontstage don't see relationship updates

**Fix approach:**
Expose `create_character_relationship`, `update_character_relationship`, `delete_character_relationship` IPC commands. Emit `dataRefresh(story_id, "characterRelationships")` after each operation. Add corresponding `case 'characterRelationships'` handler in `useSyncStore` to invalidate `['character-relationships', storyId]` cache.

---

### Knowledge Graph Updates Not Synchronized (P1 - Functional Gap)

**Issue:** When `create_entity`, `update_entity`, or `create_relation` IPC commands complete, no sync events are emitted. The `StateSync` module has no `emit_knowledge_graph_updated` method. Frontend KG visualizations and Canonical State caches don't refresh automatically.

**Files:**
- `src-tauri/src/commands_v3.rs` (entity/relation mutation commands lack event emission)
- `src-frontend/src/hooks/useSyncStore.ts` (no `knowledgeGraph` case in sync handler)

**Impact:**
- KG visualization pages don't reflect manual entity/relation changes
- Canonical State cache in frontstage becomes stale
- Users must manually switch pages or restart to see updates

**Fix approach:**
Add `StateSync::emit_data_refresh(story_id, "knowledgeGraph")` calls after entity/relation mutations. Add `case 'knowledgeGraph'` handler in `useSyncStore` to invalidate `['knowledge-graph', storyId]` cache.

---

### Ingestion Pipeline Events Not Emitted (P1 - Functional Gap)

**Issue:** When `auto_ingest_chapter` or Workflow `VectorIndex` node completes ingestion and writes new entities to kg_entities, no `ingestion-completed` or `knowledge-graph-updated` events are emitted. Frontend caches for knowledge graph and foreshadowings don't auto-refresh.

**Files:**
- `src-tauri/src/memory/ingest.rs` (IngestPipeline completion path)
- `src-tauri/src/workflow/scheduler.rs` (VectorIndex node execution)
- `src-frontend/src/hooks/useSyncStore.ts` (no `ingestionCompleted` case)

**Impact:**
- KG visualizations don't reflect newly ingested entities
- Foreshadowing board doesn't update with new connections
- Canonical State cache remains stale after ingestion

**Fix approach:**
Emit `dataRefresh(story_id, "knowledgeGraph")` and `ingestion-completed` events after successful ingestion. Add `case 'ingestionCompleted'` handler in `useSyncStore` to invalidate related caches.

---

### Plan Template Library Not Persisted (P1 - Data Loss)

**Issue:** `PlanTemplateLibrary` only stores templates in memory (`Mutex<Vec<PlanTemplate>>`). When `PlanExecutor::execute_plan` records a successful plan via `template_library.record_success`, the template is lost on application restart. Design claims "PlanTemplateLibrary records successful execution for request reuse" but this doesn't work across sessions.

**Files:**
- `src-tauri/src/planner/executor.rs` (line ~979, `record_success` call)
- `src-tauri/src/db/models_v3.rs` (no `plan_templates` table defined)

**Impact:**
- Learned plan templates are discarded on restart
- Cross-session plan optimization doesn't work
- Design promise of "adaptive planning" is unfulfilled

**Fix approach:**
Create `plan_templates` SQLite table with schema: `id, name, input_signature, output_signature, success_count, last_used_at, created_at`. Persist templates to database in `record_success`. Load templates from database in `PlanExecutor::new`.

---

### Capability Evolution Only Triggered at Startup (P1 - Functional Gap)

**Issue:** `evolve_capability_descriptions` is triggered only once, 30 seconds after app startup via `tauri::async_runtime::spawn`. After that, capability descriptions don't evolve even as new execution records accumulate. Design claims "continuous capability evolution feedback loop" but implementation is startup-only.

**Files:**
- `src-tauri/src/lib.rs` (lines ~120-130, one-time spawn after 30s)
- `src-tauri/src/capabilities/mod.rs` (no scheduled trigger)

**Impact:**
- Capability descriptions become stale as system learns from new executions
- Adaptive behavior doesn't improve over time during a session
- Design promise unfulfilled

**Fix approach:**
Implement a scheduled task (via `TaskService` or `WorkflowScheduler`) that checks execution record count every hour or triggers when count >= 5 new records. Call `evolve_capability_descriptions` based on threshold, not just at startup.

---

### Payoff Ledger Cache Not Invalidated (P1 - Stale Data)

**Issue:** Commands like `update_story_outline`, `update_payoff_ledger_fields`, `detect_overdue_payoffs` emit `dataRefresh` events, but `useSyncStore` has no `case 'payoffLedger'` handler. The `['payoff-ledger', storyId]` cache doesn't auto-invalidate. Only Foreshadowing page's manual window listener triggers refresh.

**Files:**
- `src-tauri/src/commands_v3.rs` (payoff ledger mutation commands emit `dataRefresh`)
- `src-frontend/src/hooks/useSyncStore.ts` (missing `payoffLedger` case)

**Impact:**
- Payoff ledger page shows stale data after updates
- Users must manually refresh to see changes
- Inconsistent sync behavior across different resource types

**Fix approach:**
Add `case 'payoffLedger'` handler in `useSyncStore` to invalidate `['payoff-ledger', storyId]` cache. Ensure all payoff ledger mutations emit `dataRefresh(story_id, "payoffLedger")`.

---

### Workflow Instances Not Resumed After Restart (P1 - Functional Gap)

**Issue:** When `WorkflowEngine::with_pool` starts, it loads pending/running instances from the database via `load_instances_from_db`, but doesn't re-enqueue them to `WorkflowScheduler`. Instances remain in `workflow_instances` table but no background thread advances them. Users must manually call `start_workflow_instance` to resume.

**Files:**
- `src-tauri/src/workflow/scheduler.rs` (lines ~675, `load_instances_from_db` called but not re-queued)
- `src-tauri/src/workflow/engine.rs` (no auto-resume logic)

**Impact:**
- Long-running workflows interrupted by app restart don't resume automatically
- Workflow state is persisted but not executed
- Users lose progress on multi-step workflows

**Fix approach:**
After loading instances from database in `WorkflowEngine::with_pool`, filter for `Pending` or `Running` status and re-enqueue them to `WorkflowScheduler.queue`. Let `start_auto_drain` worker continue execution in background.

---

## Known Bugs

### Image LLM Profile Type Not Implemented (P2 - Dead Code)

**Issue:** UI allows users to create LLM profiles with type `image`, but `config::commands::test_model_connection` returns hardcoded error "图像生成模型暂未实现" (Image generation model not yet implemented). Users can configure image profiles but cannot test or use them.

**Files:**
- `src-tauri/src/config/commands.rs` (line ~422, `// TODO: 实现图像生成模型`)

**Impact:**
- Users hit dead end when trying to use image generation features
- Misleading UI suggests feature is available
- No clear error message or UI restriction

**Fix approach:**
Either (a) implement basic image generation connection test via OpenAI/Stable Diffusion endpoints, or (b) mark `image` type as experimental in UI and prevent creation of image profiles until implemented.

---

## Performance Bottlenecks

### Excessive Clone Operations in State Management (P2 - Performance)

**Issue:** Global state accessors (`get_pool()`, `get_config()`) and initialization code clone `DbPool` and `AppConfig` 1441+ times throughout the codebase. Each clone increments reference counts and may trigger unnecessary allocations.

**Files:**
- `src-tauri/src/lib.rs` (lines 84-85, `get_pool().clone()` and `get_config().clone()`)
- `src-tauri/src/lib.rs` (lines ~120-150, initialization clones pool/app_handle 10+ times)
- `src-tauri/src/commands_v3.rs` (frequent `pool.inner().clone()`)

**Impact:**
- Increased memory pressure during high-concurrency operations
- Potential latency spikes when many commands execute simultaneously
- Unnecessary Arc/Mutex contention

**Fix approach:**
Use reference counting more efficiently. Consider passing `&DbPool` or `Arc<DbPool>` directly instead of cloning. Profile hot paths to identify unnecessary clones. Use `Arc::clone` explicitly to make intent clear.

---

### Unwrap Calls in Lock Operations (P2 - Reliability)

**Issue:** 103 instances of `lock().unwrap()` throughout the codebase. If a Mutex is poisoned (thread panicked while holding lock), the unwrap will panic and crash the application.

**Files:**
- `src-tauri/src/lib.rs` (lines 84-85, `DB_POOL.lock().unwrap()`, `APP_CONFIG.lock().unwrap()`)
- `src-tauri/src/lib.rs` (multiple `INGEST_COOLDOWN.lock().unwrap()` calls)
- Throughout `src-tauri/src` (103 total instances)

**Impact:**
- Single panicked thread can crash entire application
- No graceful degradation or recovery
- Difficult to debug in production

**Fix approach:**
Replace `lock().unwrap()` with `lock().expect("descriptive message")` or handle poisoning explicitly. Consider using `parking_lot::Mutex` which doesn't poison on panic. Add logging when lock acquisition fails.

---

### Large Component Files (P2 - Maintainability)

**Issue:** Frontend components exceed 1000 lines, making them difficult to maintain and test:
- `FrontstageApp.tsx`: 1870 lines
- `Settings.tsx`: 1713 lines
- `CreationWizard.tsx`: 1383 lines
- `SceneEditor.tsx`: 1054 lines

**Files:**
- `src-frontend/src/frontstage/FrontstageApp.tsx`
- `src-frontend/src/pages/Settings.tsx`
- `src-frontend/src/pages/CreationWizard.tsx`
- `src-frontend/src/components/SceneEditor.tsx`

**Impact:**
- Difficult to understand component logic
- Hard to test individual features
- Increased risk of bugs during refactoring
- Slower IDE performance

**Fix approach:**
Extract sub-components and custom hooks. Break `FrontstageApp` into smaller focused components (e.g., `StatusIcon`, `PipelineProgress`, `AILearning` as separate files). Move business logic to custom hooks (e.g., `useFrontstageState`, `usePipelineExecution`).

---

## Fragile Areas

### State Synchronization Between Frontstage and Backstage (P1 - Fragile)

**Issue:** Frontstage and backstage windows must stay in sync via `StateSync` events. Multiple sync event types exist (`scene_created`, `scene_updated`, `dataRefresh`, etc.) but coverage is incomplete. Missing event emissions in some mutation paths cause cache inconsistencies.

**Files:**
- `src-tauri/src/state_sync.rs` (event emission logic)
- `src-tauri/src/commands_v3.rs` (mutation commands with inconsistent event emission)
- `src-frontend/src/hooks/useSyncStore.ts` (event consumption)

**Why fragile:**
- New mutations must remember to emit events or caches become stale
- Event types and cache keys must match exactly
- No compile-time verification that all mutations emit events
- Easy to add a new command and forget the event

**Safe modification:**
- Create a wrapper function `emit_mutation_event(story_id, resource_type, operation)` that enforces event emission
- Add compile-time checks or tests that verify all mutation commands emit events
- Document the sync event contract in code comments
- Test coverage gaps: `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts` exists but may be incomplete

---

### Database Schema Evolution and Migrations (P1 - Fragile)

**Issue:** Database schema is defined inline in `connection.rs` with 40+ migrations. Adding new tables or columns requires careful ordering to avoid foreign key conflicts. The schema has evolved through multiple versions with some backward compatibility concerns.

**Files:**
- `src-tauri/src/db/connection.rs` (all migrations inline, 1975 lines)

**Why fragile:**
- Migrations are not versioned or tracked separately
- Foreign key constraints can fail if tables are created in wrong order
- No rollback mechanism if migration fails
- Hard to review schema changes in git history

**Safe modification:**
- Extract migrations to separate files with version numbers
- Add migration tracking table to database
- Test migrations on both fresh and upgraded databases
- Document foreign key dependencies between tables

---

### Ingest Pipeline Cooldown State (P1 - Fragile)

**Issue:** `INGEST_COOLDOWN` HashMap tracks chapter ingestion state to prevent duplicate LLM calls. State is stored in memory with 5-minute expiry. If app crashes, cooldown state is lost and duplicate ingestions may occur.

**Files:**
- `src-tauri/src/lib.rs` (lines 71-72, `INGEST_COOLDOWN` definition)

**Why fragile:**
- No persistence across restarts
- Manual cleanup logic with time-based expiry
- Concurrent access via `lock().unwrap()`
- No monitoring of cooldown state

**Safe modification:**
- Persist cooldown state to database (new table `ingest_cooldowns`)
- Add cleanup task to remove expired entries
- Add logging/metrics for cooldown hits and misses
- Consider using a more robust cache library

---

## Test Coverage Gaps

### Property-Based Tests for Bug Conditions (P1 - Incomplete)

**Issue:** `src-tauri/src/tests/bug_condition_v57.rs` contains exploratory property-based tests (PBTs) designed to fail on unfixed code. Tests validate 12 bug conditions but are marked as exploratory and may not be comprehensive.

**Files:**
- `src-tauri/src/tests/bug_condition_v57.rs` (673 lines, PBT cases set to 8 for speed)

**What's not tested:**
- Integration tests for complete delete workflows (story → chapters → scenes → entities)
- End-to-end sync event propagation from backend to frontend
- Workflow instance recovery after restart
- Plan template persistence across sessions
- Capability evolution scheduling

**Risk:** 
- Bugs may be fixed locally but regressions introduced elsewhere
- No continuous validation that fixes remain in place

**Priority:** High - These gaps directly correspond to P0/P1 concerns above.

---

### Frontend Sync Store Tests (P1 - Incomplete)

**Issue:** `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts` exists but test coverage for sync event handling is incomplete. Missing cases for new resource types (payoffLedger, knowledgeGraph, characterRelationships).

**Files:**
- `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts`

**What's not tested:**
- `case 'payoffLedger'` cache invalidation
- `case 'knowledgeGraph'` cache invalidation
- `case 'characterRelationships'` cache invalidation
- Event ordering and race conditions
- Stale cache detection

**Risk:**
- Sync events may be emitted but not consumed correctly
- Cache inconsistencies go undetected

---

## Scaling Limits

### SQLite Connection Pool Size (P2 - Scaling)

**Issue:** Database connection pool is configured with `max_size(5)` in `connection.rs` line 42. This may be insufficient for high-concurrency scenarios with many simultaneous IPC commands.

**Files:**
- `src-tauri/src/db/connection.rs` (line 42, `Pool::builder().max_size(5)`)

**Current capacity:** 5 concurrent database connections

**Limit:** When more than 5 commands execute simultaneously, additional commands queue and wait for a connection to become available. This can cause latency spikes.

**Scaling path:**
- Profile actual concurrency under load
- Increase pool size if needed (consider memory/resource constraints)
- Implement connection pooling metrics/monitoring
- Consider async connection handling or connection multiplexing

---

### In-Memory State Structures (P2 - Scaling)

**Issue:** Global state like `INGEST_COOLDOWN`, `SKILL_MANAGER`, `MCP_CONNECTIONS` are stored in memory. As the application runs longer, these structures may grow unbounded or cause memory pressure.

**Files:**
- `src-tauri/src/lib.rs` (lines 65-72, global state definitions)

**Current capacity:** Unbounded for most structures

**Limit:** Memory usage grows with number of stories, skills, MCP connections, and ingest operations.

**Scaling path:**
- Add memory limits and eviction policies
- Persist state to database where appropriate
- Implement metrics to monitor memory usage
- Add cleanup tasks for stale entries

---

## Dependencies at Risk

### Reqwest Version Pinned to Exact Version (P2 - Maintenance)

**Issue:** `reqwest` is pinned to exact version `=0.12.4` in `Cargo.toml` line 20. This prevents automatic security updates and may cause compatibility issues with other dependencies.

**Files:**
- `src-tauri/src/Cargo.toml` (line 20, `reqwest = { version = "=0.12.4", ... }`)

**Risk:**
- Security vulnerabilities in 0.12.4 won't be auto-patched
- Dependency conflicts if other crates require newer reqwest
- Manual intervention needed for updates

**Migration plan:**
- Review reqwest 0.12.x changelog for breaking changes
- Update to `"0.12"` (caret range) to allow patch updates
- Test thoroughly after update
- Consider upgrading to 0.13+ if available and compatible

---

### Tauri v2.4 Compatibility (P2 - Maintenance)

**Issue:** Project uses Tauri v2.4 with multiple plugins (fs, dialog, shell, http, updater). Plugin versions must stay synchronized with Tauri core version.

**Files:**
- `src-tauri/src/Cargo.toml` (lines 10, 80-84)

**Risk:**
- Plugin version mismatches can cause runtime errors
- Tauri v2.x may have security updates that require plugin updates
- Breaking changes in Tauri v3 will require significant refactoring

**Migration plan:**
- Monitor Tauri releases for security updates
- Keep all plugins synchronized with core version
- Plan for Tauri v3 migration when stable

---

## Missing Critical Features

### Image Generation Model Support (P2 - Feature Gap)

**Issue:** UI allows configuration of image-type LLM profiles but backend has no implementation. Users cannot test or use image generation features.

**Files:**
- `src-tauri/src/config/commands.rs` (line ~422, TODO comment)

**Blocks:**
- Image-based story visualization
- AI-generated cover art
- Scene illustration generation

**Implementation path:**
- Add OpenAI DALL-E or Stable Diffusion integration
- Implement connection testing for image endpoints
- Add image generation command to IPC
- Update UI to show image generation status

---

### Persistent Plan Template Learning (P2 - Feature Gap)

**Issue:** Plan templates are learned during execution but not persisted. Each session starts with empty template library.

**Files:**
- `src-tauri/src/planner/executor.rs`
- `src-tauri/src/db/models_v3.rs`

**Blocks:**
- Cross-session plan optimization
- Adaptive planning improvements over time
- Design promise of "learning system"

**Implementation path:**
- Create `plan_templates` database table
- Persist templates on successful execution
- Load templates on executor initialization
- Add metrics for template hit rate

---

## Summary

**Critical Issues (P0):** 3
- Foreign key constraints not enforced
- Incomplete cascade deletes
- Data integrity at risk

**Functional Gaps (P1):** 9
- Missing IPC commands and sync events
- Incomplete automation loops
- Stale caches and data inconsistencies

**Code Quality (P2):** 6
- Performance concerns (clones, locks)
- Large components
- Fragile state management
- Test coverage gaps

**Recommended Priority Order:**
1. Fix foreign key enforcement (P0)
2. Implement missing sync events (P1)
3. Persist plan templates (P1)
4. Resume workflows after restart (P1)
5. Refactor large components (P2)

---

*Concerns audit: 2026-05-11*
