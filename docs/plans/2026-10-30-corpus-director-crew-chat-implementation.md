# 实施方案：语料资产 · 导演 Agent · 幕后剧组对话

日期：2026-10-30
状态：设计已获批准；本文为实施方案，代码尚未开始改
设计文档：[2026-10-30-corpus-director-crew-chat-design.md](./2026-10-30-corpus-director-crew-chat-design.md)
目标版本：v0.74.0（P0）

---

## 0. 交付边界

**P0 交付**

1. **并发底座**：`agency_runs.lane` + lane 唯一索引、`agency_agent_tasks`、`story_resource_leases`、`TaskScheduler`/`TaskReporter`、异步派发。一个 story 仍保持一个 `write` lane，但 `review`/`inspect`/`chat` 可以并行；子 agent 不再各自开 run。
2. **语料资产**：语料页、导入（文本/链接/文件/图片）、AI 提炼、转资产草稿、`corpus_link`、语料清理、agency 只读工具。
3. **导演 Agent**：`AgentRole::Director`、导演区黑板、巡检/计划/异步派发/修复、确认策略。
4. **幕后对话**：代理工作室三栏改造、会话/消息落库、`@管理/@编审/@主创` 直达、审批卡、现场任务进程。
5. **迁移/命令/事件/IPC/测试/文档**：V149、新命令注册、事件监听、单测/前端测/E2E、项目强制文档更新。

**P0 不做**

- 多 write 任务并行（两章同时写）：P1 用资源租约 + 原子章号开放。
- 语料进入续写上下文（pinned）：P1。
- token 级流式、会话滚动摘要、自动巡检、图片 OCR：P1/P2。
- 导演直接写正文/资产：永不做。

**完成定义（DoD）**

- 所有验收探针通过；`cargo test --lib`、`npx tsc --noEmit`、`npx vitest run`、`npx playwright test`、`python3 scripts/verify-ipc-manifest.py`、`python3 scripts/architecture_guard.py`、`cargo clippy`、固定 nightly `cargo fmt --check` 全绿。
- 不跑本地 `cargo tauri build`（用户级规则）；推送后由 GitHub Actions 全平台构建。
- 版本号 `0.74.0` 四处一致；`README.md`、`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`、`landing/src/hooks/useLatestRelease.ts` 同步。

---

## 1. 实施顺序（依赖图）

```
A1 V149 迁移
  └→ A2 lane/models/repository
      └→ A3 TaskReporter + ToolLoop cancel
          └→ A4 TaskScheduler
              └→ A5 LeaseManager
                  └→ A6 lane 映射与回归
                      ├→ B 语料后端
                      └→ C 导演 + 对话后端
                            ├→ D 前端
                            └→ E IPC/文档/发布
```

原则：**A 必须先合入并可单测**，否则 B/C 的异步派发没有安全底座；B 与 C 可以并行开发，但 C 依赖 A；D 依赖 B/C 的类型与命令；E 最后。

---

## 2. 阶段 A：并发底座（lane + task + lease）

### A1. V149 迁移

**文件**

- `src-tauri/src/db/migrations/V149__corpus_and_director.rs`（Rust 迁移）
- `src-tauri/src/db/migrations/mod.rs`：`pub mod V149__corpus_and_director;` + `Box::new(...)` 加入 `all_rust_migrations()`

**必须在一个事务里完成**

```rust
fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction()?;
    // 1) lane 列（幂等）
    if !has_column(&tx, "agency_runs", "lane")? {
        tx.execute(
            "ALTER TABLE agency_runs ADD COLUMN lane TEXT NOT NULL DEFAULT 'write'",
            [],
        )?;
    }
    // 2) 原子替换 V109 索引
    tx.execute("DROP INDEX IF EXISTS idx_agency_runs_one_active_per_story", [])?;
    tx.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agency_runs_one_active_per_lane
         ON agency_runs(story_id, lane) WHERE status IN ('pending', 'running')",
        [],
    )?;
    // 3) 新表
    create_agent_tasks(&tx)?;
    create_resource_leases(&tx)?;
    create_chat_sessions(&tx)?;
    create_chat_messages(&tx)?;
    create_director_actions(&tx)?;
    create_corpus_items(&tx)?;
    create_corpus_links(&tx)?;
    tx.commit()?;
    Ok(())
}
```

`has_column` 用 `PRAGMA table_info(agency_runs)` 检查，避免重复 `ALTER`。所有建表用 `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`。

**表清单**

- `agency_agent_tasks`：`run_id`（真实 run 或 `chat:<session>`）、`story_id` FK CASCADE、`role`、`parent_task_id`、`task`、`status`、`phase`、`turn_count`、`tool_calls`、`last_tool`、`heartbeat_at`、`started_at`、`finished_at`、`error`、`result_summary`、`meta_json`、`created_at`。
- `story_resource_leases`：`story_id` FK CASCADE、`resource`、`owner_run_id`、`owner_task_id`、`acquired_at`、`heartbeat_at`、`expires_at`、`UNIQUE(story_id, resource)`。
- `agency_chat_sessions`：`story_id` FK CASCADE、`title`、`target_role`、`summary`、`summary_upto_message_id`、`archived`、时间戳。
- `agency_chat_messages`：`session_id` FK CASCADE、`role`、`content`、`status`、`run_key`、`task_id`、`task_ids_json`、`actions_json`、`meta_json`、`error`、`created_at`。
- `agency_director_actions`：`story_id` FK CASCADE、`session_id`、`message_id`、`run_id`、`action_type`、`target_role`、`target_id`、`params_json`、`status`、`reason`、`result_summary`、`error`、时间戳。
- `corpus_items`：`story_id` FK CASCADE、`kind`、`title`、`summary`、`content`、`source_url`、`source_name`、`tags`、`file_path`、`file_name`、`mime_type`、`file_size`、`file_hash`、`status`、`pinned`、`distilled_json`、时间戳。
- `corpus_links`：`corpus_id` FK CASCADE、`target_type`、`target_id`、`note`、`created_at`。

索引：`agency_agent_tasks(story_id, status, heartbeat_at)`、`(run_id, created_at)`、`(parent_task_id)`；`story_resource_leases(expires_at)`；`agency_chat_messages(session_id, created_at)`；`corpus_items(story_id, kind, created_at)`、`UNIQUE(story_id, file_hash) WHERE file_hash IS NOT NULL`；`agency_director_actions(story_id, created_at)`。

**测试**

- `test_v149_adds_lane_and_replaces_index`：新建测试库跑迁移后，`agency_runs` 有 `lane`，`idx_agency_runs_one_active_per_lane` 存在、旧索引不存在。
- `test_v149_preserves_existing_runs`：迁移前插入一条 `pending` run，迁移后 `lane == "write"`。
- `test_v149_is_idempotent`：重复跑迁移不报错。
- 迁移版本断言：`schema_migrations` 含 149。

---

### A2. lane / models / repository

**文件**

- `src-tauri/src/agency/models.rs`：`AgencyRun` 增加 `lane: String`；`AgencyRun::new` 默认 `lane = "write"`；加 `with_lane`；新增 `AgencyAgentTask`、`ResourceLease`、`ChatSession`、`ChatMessage`、`DirectorAction` 结构（或放各自模块）。
- `src-tauri/src/agency/repository.rs`：
  - `create_run` SQL 增加 `lane`；`get_run` / `list_runs_for_story` 读取并映射 `lane`。
  - `has_running_run_for_story(&self, story_id: &str, lane: &str)`：签名从 1 参数改为 2 参数；更新 3 个调用点（`agency/commands.rs:47`、`agency/commands.rs:109`、`coordinator.rs:6258`）传 `"write"`。
  - `map_active_run_conflict`（`coordinator.rs:942`）同时识别新旧索引名，错误消息区分 lane（write: 该故事已有进行中的创作任务；review/inspect: 该车道已有进行中的任务）。
- 新增 repository（建议拆分，避免 `agency/repository.rs` 继续膨胀）：
  - `src-tauri/src/agency/task_repository.rs`：任务 CRUD、心跳、结束、`list_for_story`、`reap_stale`。
  - `src-tauri/src/agency/chat_repository.rs`：会话/消息 CRUD、`append_task_result`、`latest_session`。
  - `src-tauri/src/agency/director_repository.rs`：`agency_director_actions` CRUD。
  - `src-tauri/src/agency/lease.rs`：`LeaseManager` + lease CRUD（见 A5）。
  - 在 `agency/mod.rs` 声明。
- `src-tauri/src/db/repositories/story_repository.rs`：orphan 检查数组加入 `corpus_items`、`agency_chat_sessions`、`agency_agent_tasks`、`story_resource_leases`。

**lane 映射点（必须逐处改）**

| 创建点 | lane |
|---|---|
| `run_genesis_inner`（story_id 先 NULL，索引豁免） | `write` |
| `run_continue_inner` / `run_batch_inner` / `resume_prepare` / `run_asset_refresh` | `write` |
| `run_role_task`：Producer / LeadWriter / Writer / OutlinePlanner | `write` |
| `run_role_task`：EditorAuditor / Inspector | `review` |
| `run_role_task`：Director | `inspect` |
| `observe::find_or_create_observe_run` | `inspect`（status 仍 `idle`，不进索引） |
| Director turn / 子 agent task | 不建 run；`run_id = chat:<session>` 或当前 write run id |

**测试**

- `test_write_review_lanes_coexist`：同 story 建 write + review 两个 pending run，均成功。
- `test_two_write_runs_conflict`：第二个 write 触发唯一索引，映射为「该故事已有进行中的创作任务」。
- `test_lane_default_is_write`：旧调用点创建的 run 仍为 write。
- `test_has_running_run_for_story_by_lane`：write/review 分别查询。
- 更新 `test_partial_unique_index_one_active_per_story` 为 per-lane。

---

### A3. TaskReporter + ToolLoop cancel

**文件**

- 新增 `src-tauri/src/agency/reporter.rs`：
  - `TaskReporter { pool, task_id, story_id, run_id, role, app_handle }`
  - `start()`：任务行已由 scheduler 创建；写 `status=running`、`started_at`、heartbeat。
  - `heartbeat(phase, turn, tool)`：`spawn_blocking` fire-and-forget 更新 `heartbeat_at`、`turn_count`、`tool_calls`、`last_tool`。
  - `finish(status, summary, error)`：`spawn_blocking(...).await` 落库，并 emit `agency-crew-update`。
  - `ReportingLlm` 包装 `Arc<dyn LoopLlm>`：`complete` / `complete_metered` / `complete_json` 前后调用 `heartbeat("llm")`。
- `src-tauri/src/agency/tool_loop.rs`：
  - `ToolLoop` 增加 `cancel: Option<Arc<AtomicBool>>` 与 `with_cancel(...)`。
  - 每轮开始检查 `cancel`；命中时返回 `LoopResult { aborted: true, abort_reason: None }` 或 `AppError::Cancellation`（按现有调用方语义选；测试钉住）。
- `src-tauri/src/agency/coordinator.rs`：
  - 新增 `run_role_loop_reported(...) -> Result<LoopResult, AppError>`：复制 `run_role_loop` 的 spec/prompt/ToolContext/BudgetedLlm 逻辑，把 `ReportingLlm` 插在 `BudgetedLlm` 外层，并把 `cancel` 传给 ToolLoop。
  - 既有 `run_role_loop` 保持不变；只有 scheduler / Director turn 调用 reported 版本。
- `task_repository::finish_task` 用 `await`；心跳 fire-and-forget。

**测试**

- `test_reporting_llm_heartbeats_before_and_after`：MockLlm 记录调用次数，任务行 heartbeat/turn_count 递增。
- `test_finish_task_is_persisted_before_return`：finish 返回后立即查询状态为 completed/failed。
- `test_tool_loop_cancel_stops_next_turn`：设置 cancel，循环在下一轮开始前退出，`aborted == true`。
- `test_task_status_machine`：queued → running → completed / failed / cancelling → cancelled / stale。

---

### A4. TaskScheduler

**文件**

- 新增 `src-tauri/src/agency/scheduler.rs`：
  ```rust
  pub struct TaskScheduler {
      pool: DbPool,
      app: AppHandle,
      coordinator: Arc<AgencyCoordinator>, // 用 its pool/app/board
      role_sems: HashMap<AgentRole, Arc<Semaphore>>, // Director 1 / Writer 1 / Producer 2 / Editor 2
      story_sem: Mutex<HashMap<String, Arc<Semaphore>>>, // 每 story 总并发 4
      waiters: Mutex<HashMap<String, Vec<oneshot::Sender<()>>>>, // parent_task_id -> 等待者
  }
  ```
  - `dispatch_sub_agent(&self, story_id, run_id, role, instruction, depends_on_task_id: Option<String>, parent_task_id, meta) -> Result<String>`：创建 task 行 `queued`，若有依赖则先等父任务完成（`TaskWaiters` oneshot），再 `tokio::spawn` 内部按序 acquire story/role permit → `running` → `run_role_loop_reported` → `finish`；返回 `task_id` 立即。
  - `dispatch_director_turn(...)`：创建 `role=director` task；`DirectorService::run_turn` 通过 `Weak<TaskScheduler>` 调用，避免 `Arc<TaskScheduler> → DirectorService → Arc<TaskScheduler>` 循环引用；返回 task_id 立即。
  - 父任务 `finish` 时通知 `waiters[parent_task_id]`；子任务醒来后置 `running`。
  - `retry_task(task_id)`：读取失败/取消 task 的 role/task/run_id/story_id/meta，按同样参数重新 dispatch；新 task `parent_task_id = old_task_id`。
  - `cancel_task(task_id)`：设置 cancel token（ToolLoop 检查）+ DB `cancelling`。
  - `list_tasks(story_id, limit)`、`story_snapshot(story_id)` 供 CrewInspector。
  - 完成回调：读取 `meta_json.chat_session_id` / `meta_json.chat_message_id` / `meta_json.append_mode`：
    - `append_mode="follow_up"`：`ChatRepository::append_message(role, result_summary, done)` 追加子 agent 结果；
    - `append_mode="reply"`：把 Director turn 的 reply/审批卡写入占位消息；
    - 每次状态变化 emit `agency-crew-update`。
  - 全局闸门：`budget.rs` 的 `AGENCY_GLOBAL_LLM_SEM` 从 3 改为 4（P0 常量；P1 进设置页）；新增 `DIRECTOR_LLM_SEM = Semaphore(1)`，Director turn 调用时先拿它。
  - `AgencyBudget` 增加 `director_sem: Semaphore(1)`；`acquire` 对 Director 用 `director_sem`（`with_role_permits` 签名不变）。
- `src-tauri/src/agency/commands.rs` 或新 `agency/crew_commands.rs`：`agency_crew_dispatch`、`agency_crew_cancel_task`、`agency_crew_retry_task`、`agency_crew_status`。
- `lib.rs` 启动任务：`reap_stale_tasks(300s)`、`reap_expired_leases()`，与 activity log 剪枝并列。

**测试**

- `test_dispatch_returns_task_id_before_completion`：用 MockLlm 延迟返回；dispatch 立即返回，task 行 status=queued/running。
- `test_dependency_task_waits_for_parent`：child 带 `depends_on_task_id`，parent 完成前 child 不 running，完成后 child 自动 running。
- `test_scheduler_role_limit`：Producer 2 + Editor 2 并发；Director/Writer 1。
- `test_task_completion_appends_chat_message`：meta 有 chat_session/message，完成后消息落库、状态 done。
- `test_retry_task_copies_instruction_and_parent`。
- `test_cancel_task_cancels_next_turn`。
- `test_reap_stale_tasks`：心跳超过阈值 → stale；`agency-crew-update` 事件。

---

### A5. LeaseManager

**文件**

- `src-tauri/src/agency/lease.rs`：
  - `LeaseManager::acquire(story_id, resource, owner_run_id, owner_task_id, ttl_secs) -> Result<ResourceLease, AppError>`：
    - `transaction_with_behavior(TransactionBehavior::Immediate)`；
    - 查 `(story_id, resource)`；不存在 → 插入；存在且 `expires_at < now` → 删旧插新；否则返回 `ResourceBusy { owner_run_id, owner_task_id, expires_at }`。
  - `heartbeat(lease_id, extend_secs)`、`release(lease_id)`、`reap_expired(now)`、`list_for_story(story_id)`。
  - `ResourceBusy` 用结构化 `AppError` 字段（`validation_failed(..., Some("resource_busy"))` 或新错误变体），前端能显示 owner/retry_after。
- 集成点（P0）：
  - `materialize::materialize_assets`：调用前 acquire `assets:characters` / `assets:world` / `assets:outline`（按 item_type），完成后 release。
  - `next_chapter_number` / `assemble_next_chapter`：`chapter_sequence` 租约，持到 scene INSERT 事务提交；P0 已有 write lane，租约是 P1 的底座。
  - `evaluate_gate`：`gate:<chapter>` 租约。
  - `persist_append`：`scene:<id>` 租约（P1 多 write 时启用；P0 可选）。
  - 所有租约带 TTL（默认 300s）+ heartbeat；任务结束 `release`。

**测试**

- `test_lease_acquire_and_release`
- `test_lease_conflict_returns_owner`
- `test_lease_expires_and_reacquired`
- `test_reap_expired_leases`
- 集成测试：两个 `materialize_assets` 任务，第二个进 `waiting`，第一个释放后重试成功。

---

### A6. lane 映射与回归

**文件**

- `src-tauri/src/agency/coordinator.rs`：上述创建点改 lane；`run_role_task` 按 role 映射；`resume_prepare` 的 `has_running_run_for_story` 传 `"write"`。
- `src-tauri/src/agency/observe.rs`：observe run `lane = "inspect"`。
- `src-tauri/src/agency/commands.rs`：`agency_continue_chapter` / `agency_continue_batch` 的 pre-check 传 `"write"`。
- `src-tauri/src/agency/tests.rs`、`repository.rs` 测试更新。

**回归验收**

- 创世、续写、批量续写、恢复、观察、资产刷新、评估/学习页原路径全部通过。
- 旧 `agency_runs` 行仍可读写；`lane` 默认 write。
- V109 的旧索引不再存在；新索引对 write/review 行为符合预期。

---

## 3. 阶段 B：语料后端（Corpus）

### B1. 模型 / repository

**文件**

- `src-tauri/src/corpus/mod.rs`、`repository.rs`、`service.rs`、`commands.rs`、`parser.rs`（或复用 `book_deconstruction/parser.rs`）。
- `db/models.rs` 或 `corpus/models.rs`：`CorpusItem`、`CorpusSummary`、`CorpusDetail`、`CorpusLink`、`CorpusDistillation`、`CorpusPromotionDraft`。
- `CorpusRepository`：
  - `create`, `update`, `delete`, `get`, `list`（story_id/kind/status/query/limit/offset）、`search`、`link`, `unlink`, `delete_by_story`.
- `corpus_items.file_hash` 同 story 唯一；`corpus_links` cascade。

**测试**

- create/list/update/delete。
- 同 story 同 file_hash 去重（第二个返回已有 item 或错误）。
- 删除 item 级联删除 links。
- 删除 story 级联删除 corpus_items/links（FK）。

---

### B2. 导入服务

**文件**

- `src-tauri/src/corpus/service.rs`：
  - `import_text(story_id, kind, title, content, ...)`
  - `import_link(story_id, url, title?, excerpt?)`：reqwest 10s 超时抓取 `<title>` + 去标签正文（最多 50k 字符）；失败保留 URL。
  - `import_file(story_id, kind, file_path, title?, tags?)`：
    - 扩展名：`txt/md/pdf/epub/png/jpg/jpeg/webp/gif`；文本 20MB、图片 10MB。
    - SHA-256 hash；`app_data_dir/corpus/<story_id>/<uuid>.<ext>`。
    - txt/md 直接读；pdf/epub 用 `book_deconstruction::parser::parse_book(...).raw_text`。
    - 图片只存文件 + mime + size，不解析内容。
  - `read_media(id)`：读文件 → base64 data URL（新增 `Cargo.toml` 依赖 `base64 = "0.22"`）；10MB 上限，仅详情页调用。
  - `delete_story_files(story_id)`：删 `app_data_dir/corpus/<story_id>/`。
- `src-tauri/src/commands/story.rs::delete_story`：先查询该故事语料 `file_path` 列表 → `StoryRepository::delete`（FK CASCADE 清行）→ DB 删除成功后调用 `CorpusService::delete_story_files` 删除物理文件；DB 删除失败不提前丢文件。
- `story_repository.rs` orphan 检查已加表。

**测试**

- import text/md/pdf（fixture）/image；file_hash 去重；大小超限错误；删除 story 后物理文件不存在。
- `read_media` 返回 `data:image/png;base64,...`，mime 正确。

---

### B3. AI 提炼 / 转资产

**文件**

- `src-tauri/src/corpus/service.rs`：
  - `distill(id, app_handle) -> CorpusDistillation`：
    - 读 item + story brief（标题/题材/基调/logline/世界观摘要 ≈2k 字符）。
    - `LlmService::generate_for_task_with_system_prompt(TaskType::Analysis, context_label="corpus_distill")`，system 用内联常量（P1 再挪到 prompts registry）。
    - 要求严格 JSON：`facts/conflicts/archetypes/world_hooks/angles/cautions`；解析失败用 `parse_lenient`（把该函数 `pub(crate)` 或复制宽松解析）。
    - 写 `distilled_json`、`status=distilled`；失败只返回错误，不破坏原语料。
  - `promote(id, target_type) -> CorpusPromotionDraft`：
    - target ∈ character/world/foreshadowing/outline/scene。
    - 用 distilled + content 生成**草稿 JSON**，不写资产表。
- `corpus/commands.rs`：`corpus_distill`、`corpus_promote`、`corpus_link`、`corpus_unlink`。

**测试**

- distill 成功落 `distilled_json`；解析失败时 `status` 仍 raw。
- promote 返回目标草稿，不创建 characters/world_buildings。
- link 后 `status=used`。

---

### B4. 命令注册与工具

**命令**

- `corpus_list`, `corpus_get`, `corpus_create`, `corpus_update`, `corpus_delete`, `corpus_import_file`, `corpus_read_media`, `corpus_distill`, `corpus_promote`, `corpus_link`, `corpus_unlink`。
- 注册到 `handlers.rs` 的 Agency/Corpus 段；新增 `tests/ipc_registry_contract_test.rs` 断言 `corpus_` 命令存在。

**工具**

- `src-tauri/src/agency/tools.rs`：
  - 新增 `CorpusSearchTool`（`corpus_search`）与 `CorpusReadTool`（`corpus_read`），只读。
  - `agency_default()` 对所有 `AgentRole::all()` 允许这两个工具（含 Director）。
  - 工具返回摘要/全文，全文截断 4000 字符（与 observation 截断一致）。

**测试**

- `corpus_search` 按标题/内容/标签命中；`corpus_read` 返回全文截断。
- 工具白名单：所有角色可读，不能写 corpus。

---

### B5. 故事删除 / orphan / 缓存

- `StoryRepository::delete` 前清语料物理文件；FK CASCADE 清 `corpus_items/links`。
- orphan 检查数组加入 `corpus_items`。
- 前端 `App.tsx` / `useSyncStore.ts` 删除故事时 remove `['corpus', storyId]`。

---

## 4. 阶段 C：导演 Agent + 对话后端

### C1. AgentRole::Director

**文件**

- `agency/models.rs`：
  - `AgentRole::Director`；`as_str()="director"`、`from_str("director")`、`all() -> [AgentRole; 8]`。
  - `BoardZone::Direction`；`as_str()="direction"`、`from_str`、`all() -> [BoardZone; 5]`、`owner()=Director`。
- `agency/roles.rs`：`spec_for(Director)` → `roles/director.rs`：
  - `prompt_id="agency_director_system"`、`task_type=Analysis`、`max_turns=8`、`max_output_tokens=4096`、`context_budget_chars=32_000`。
  - `test_agency_prompts_loadable` 加 Director。
- `coordinator.rs::AgencyLlm::context_label`：Director → `"agency_director"`。
- `budget.rs`：`AgencyBudget` 增加 `director_sem`；`acquire` Director → `director_sem`；`with_role_permits` 签名不变。
- `llm/service.rs::derive_model_role_from_label`：`agency_director => Tool`。
- `agency/commands.rs::agency_eval_overview`：purpose 聚合加 `agency_director`。
- `agency/tools.rs`：Director 只读工具白名单（`board_read/story_info/asset_query/asset_read/creative_context/corpus_search/corpus_read/crew_status`），**不给** `board_write/board_revise`。
- `agency/repository.rs`：`map_board_item` producer 回退、`map_message` from/to 回退识别 `"director"`。
- `agency/board.rs`、`agency/session.rs`、`agency/tools.rs`：`BoardZone::Direction` 的 snapshot/catalog/循环补全。
- 前端 `AGENCY_ROLES`/`ROLE_NAMES`/`ZONES`/`BoardItem.zone` 同步。

**测试**

- `test_director_spec_complete`
- `test_director_prompt_loadable`
- `test_role_zone_ownership`：Direction owner=Director。
- `test_board_snapshot_includes_direction`
- `test_agent_role_all_len_8`
- `test_context_label_director`

---

### C2. 导演提示词 + 模型路由

**文件**

- `resources/prompts/agency/agency_director_system.md`：
  - frontmatter：`id: agency_director_system`、`category: system`、`variables: [premise]`（**只声明 premise**；动态内容放 user prompt，满足 `prompts/registry.rs` fail-closed 测试）。
  - 正文：身份、三个子 agent、巡检优先、最小干预、作者主权、动作目录、严格 JSON schema、中文风格、禁止事项、确认策略。
- `prompts/registry.rs`：现有 `agency_*` fail-closed 测试自动覆盖；若需要，加 Director 到 `test_agency_prompts_loadable`。
- `DirectorBrief` / `render_director_user_prompt` 放 `agency/director.rs`；用户 prompt 包含：
  - 用户消息 + 最近 8 轮会话
  - `CrewSnapshot` 摘要（run/lane/tasks/alerts/board/review/corpus/leases）
  - 动作目录 + 当前章节/时间
  - 语料摘要（corpus_ids）

**测试**

- prompt 渲染后无残留 `{{...}}`。
- plan JSON schema 示例可解析。

---

### C3. CrewInspector

**文件**

- `agency/crew.rs`：
  - `CrewSnapshot { story, runs, tasks, leases, activity, board, review, corpus, alerts }`
  - `CrewAlert` 枚举：`StaleTask/SlowTask/FailedTask/FailedRun/MissingAsset/GateBlocked/EmptyDraft/BudgetExhausted/ParseCircuitBreak/OrphanTask/ResourceLeaseExpired`。
  - `inspect(pool, story_id) -> CrewSnapshot`：全部只读、确定性、不调 LLM。
  - `agency_crew_status` 命令返回快照；前端现场面板消费。
- 查询：
  - 最近 runs（含 lane/status/phase/error）
  - tasks（limit 20，含 heartbeat）
  - leases（含 owner/expires_at）
  - activity log（最近 30）
  - board（active write run 或最近 run）
  - review gate records/quality debts
  - corpus counts

**测试**

- 缺资产/心跳超时/失败任务/租约过期分别产生对应 alert。
- 无 run 时不误报。

---

### C4. DirectorPlanner / DirectorExecutor / DirectorService

**文件**

- `agency/director.rs`：
  - `DirectorPlan { reply, intent, actions: Vec<DirectorActionSpec>, needs_confirmation, confirmation }`
  - `DirectorActionSpec { id?, action_type, role?, instruction?, target_id?, corpus_ids?, depends_on_action?, reason?, overwrite? }`
  - `plan(snapshot, user_message, history, corpus) -> Result<DirectorPlan, AppError>`：
    - `DIRECTOR_LLM_SEM.acquire()`；
    - `resolve_prompt_with_vars(pool, "agency_director_system", {premise})`（spawn_blocking）；
    - `LlmService::generate_for_task_with_system_prompt(TaskType::Analysis, user_prompt, ..., context_label="agency_director")`；
    - 严格 JSON + `parse_lenient` 兜底；解析失败 → 只返回摘要回复，actions 空。
  - `action_requires_confirmation(action, snapshot) -> bool`：
    - true：LeadWriter 续写/修订、批量、overwrite=true、cancel/resume/recheck_gate/ensure_assets；
    - false：retry_task/materialize_assets/record_debt/escalate/patrol。
  - `DirectorService::run_turn(story_id, session_id, message_id, user_message, corpus_ids)`：
    1. inspect；
    2. plan；
    3. 若 `needs_confirmation`：写 `agency_director_actions` status=proposed，写审批卡消息（actions_json）；
    4. 否则 execute；
    5. 写即时回复消息（「已派发 N 个任务」或 plan.reply）。
  - `DirectorExecutor::execute(story_id, session_id, message_id, actions, scheduler) -> Vec<task_id>`：
    - 若无 `id`，按 `a1..` 编号；维护 `HashMap<action_id, task_id>`，把 `depends_on_action` 解析成父 `task_id`。
    - dispatch → `scheduler.dispatch_sub_agent`（meta: chat_session/message/append_mode=follow_up）
    - retry_task → scheduler.retry_task
    - cancel_task → scheduler.cancel_task
    - cancel_run → coordinator.cancel_agency_run
    - resume_run → coordinator.resume_prepare/resume_run
    - materialize_assets → lease + materialize::materialize_assets
    - ensure_assets → scheduler.dispatch_sub_agent(Producer)
    - recheck_gate → scheduler.dispatch_sub_agent(Editor)
    - record_debt → quality_debt::record_debt
    - escalate → chat message
    - 每个动作落 `agency_director_actions`（executing/done/failed）+ emit `agency-director-action`。
  - `execute_confirmed(action_ids)`：读取 proposed actions → approved → execute；`reject(action_ids)` → rejected。
- `agency_director_execute` / `agency_director_reject` 命令。

**测试**

- `test_director_plan_parse_and_no_side_effect_on_error`
- `test_action_confirmation_policy`
- `test_execute_dispatch_returns_task_ids`
- `test_execute_confirmed_proposed_action`
- `test_director_never_calls_board_write`（白名单断言）
- `test_director_plan_includes_lane_lease_alerts`

---

### C5. ChatService

**文件**

- `agency/chat.rs`：
  - `ChatService { pool, app, scheduler, coordinator }`
  - `send(story_id, session_id, message, target_role, corpus_ids) -> ChatSendResult { session_id, message_id, turn_task_id }`：
    - 默认/新建 session（title=首条消息前 20 字）；
    - insert user message；
    - insert assistant placeholder（status=pending, role=target_role/director）；
    - target_role == director → `scheduler.dispatch_director_turn`；
    - target_role != director → `scheduler.dispatch_sub_agent`；
    - 立即返回，不等待任务完成。
  - `list_sessions(story_id)`、`list_messages(session_id, limit, before)`、`new_session`、`delete_session`。
  - `context(session_id)`：最近 12 条消息 + summary（P1 滚动摘要）。
- `ChatRepository`：会话/消息持久化；`append_task_result` 由 scheduler 调用。
- `agency_chat_sessions/messages` 表已在 A1 建好；`run_key = chat:<session_id>`。

**测试**

- `test_chat_send_returns_immediately`：MockLlm 延迟，命令返回 < 100ms，消息 status=acting。
- `test_chat_send_default_session`。
- `test_chat_history_order_and_limit`。
- `test_task_follow_up_appended`：scheduler 完成后消息 role=producer/editor/lead_writer，status=done。
- `test_delete_session_cascades_messages`。

---

### C6. 命令 / 事件 / IPC

**命令**（全部注册 `handlers.rs` Agency 段）

- `agency_chat_send`, `agency_chat_sessions`, `agency_chat_messages`, `agency_chat_new_session`, `agency_chat_delete_session`
- `agency_crew_status`, `agency_crew_dispatch`, `agency_crew_cancel_task`, `agency_crew_retry_task`, `agency_crew_release_lease`
- `agency_director_execute`, `agency_director_reject`, `agency_director_patrol`

**事件**

- `agency-chat-update` `{session_id, message_id, role, status, content?, stage?, detail?, task_ids?}`
- `agency-crew-update` `{story_id, task}`（含 `waiting`）
- `agency-lease-update` `{story_id, resource, owner_task_id, status}`
- `agency-director-action` `{action_id, status, summary}`

**IPC 契约**

- `src-tauri/src/tests/ipc_registry_contract_test.rs`：
  - 断言 `handlers.rs` 的 Agency 段含 `agency_chat_send`、`agency_crew_status`、`agency_director_execute`。
  - 断言 `V149` 文件含 `idx_agency_runs_one_active_per_lane`。
- `scripts/verify-ipc-manifest.py`：新命令都有前端 invoke；全部通过。
- `agency.ts` 增加 chat/crew/director 命令封装与 DTO。

---

## 5. 阶段 D：前端

### D1. 路由 / 侧栏 / 类型

- `types/index.ts`：`ViewType` 加 `'corpus'`；新增 `CorpusItem/CorpusDetail/CorpusDistillation/CorpusPromotionDraft`、`ChatSession/ChatMessage/CrewTask/CrewSnapshot`、`BoardItem.zone` 加 `'direction'`。
- `App.tsx`：
  - import `Corpus`；`renderView` 加 `case 'corpus'`。
  - 故事切换/窗口恢复 invalidate `['corpus', storyId]`、`['agency-chat', storyId]`、`['agency-crew', storyId]`。
  - 常驻监听 `agency-chat-update`、`agency-crew-update`、`agency-lease-update`、`agency-director-action`。
- `components/Sidebar.tsx`：故事资产组加 `{ id: 'corpus', label: '语料', icon: Library, impact: 'warm' }`。

### D2. 语料页

**文件**

- `pages/Corpus.tsx`：列表/筛选/搜索/详情/导入/空态。
- `components/corpus/CorpusCard.tsx`、`CorpusDetail.tsx`、`CorpusImportDialog.tsx`、`CorpusPromoteDialog.tsx`。
- `hooks/useCorpus.ts`（TanStack Query，key `['corpus', storyId]`）。
- `services/api/corpus.ts`（loggedInvoke 封装）。
- UI：`AiPromptBar`/`AiContextCards`/`AiThinking`/`AiApprovalCard`/`EmptyHint`；只用 `--ai-*` 令牌。

**测试**

- `pages/__tests__/Corpus.test.tsx`：筛选、搜索、卡片、详情、导入、提炼 loading、promote 草稿、交给导演跳转。
- `services/api/__tests__/corpus.test.ts`：命令参数。
- E2E `e2e/corpus.spec.ts`：导入文本、详情可见、切换 tab、删除。

### D3. 代理工作室

**文件**

- `pages/AgencyStudio.tsx` 改造三栏。
- `components/agency/CrewRoster.tsx`、`CrewChat.tsx`、`CrewChatMessage.tsx`、`CrewLivePanel.tsx`、`CrewBoard.tsx`、`CrewTimeline.tsx`、`DirectorApprovalCard.tsx`。
- `stores/agencyChatStore.ts`：当前 session、messages、pending 输入、corpus 引用、approval。
- `stores/agencyCrewStore.ts`：tasks/leases/alerts、event 合并、dedupe by task id。
- `services/api/agency.ts`：chat/crew/director 命令与类型。
- `stores/agencyActivityStore.ts`：`AGENCY_ROLES` 加 `{ key: 'director', name: '导演' }`。
- `AgencyStudio.tsx` 的 `ZONES` 加 `direction`；`ROLE_NAMES` 加 `director`。

**交互**

- 输入条 `AiPromptBar`：`/巡检` `/修复` `/续写` `/语料` 命令、`@管理/@编审/@主创` 数据源。
- 消息：user/director/producer/editor/lead_writer/system；状态 `pending/thinking/acting/waiting/done/failed/degraded`。
- 导演即时回复「已派发」+ 子 agent follow-up；审批卡 `AiApprovalCard` 或专用 `DirectorApprovalCard`。
- 现场：任务行（queued/running/waiting/completed/failed/stale）、告警、租约、黑板 5 分区、时间线。
- 轮询：chat 5s、crew 3s、board 10s；事件补充实时。

**测试**

- `agencyChatStore.test.ts`：消息合并、`acting → done`、task_ids、approval。
- `agencyCrewStore.test.ts`：事件/轮询去重、任务状态排序。
- `CrewChat.test.tsx`：发送立即插入、目标 chips、快捷指令、approval。
- `CrewLivePanel.test.tsx`：任务进程/告警/租约渲染。
- `AgencyStudio.test.tsx`：三栏、空态、切换 run。
- E2E `e2e/agency-director.spec.ts`：发送消息、任务卡、审批卡、巡检；mock-tauri 加新 handler。

---

## 6. 阶段 E：IPC/文档/发布

### E1. 测试矩阵

| 层 | 命令 | 通过标准 |
|---|---|---|
| Rust | `cd src-tauri && cargo test --lib` | 全绿，新增用例 ≥ 25 |
| Rust lint | `cargo clippy` | 0 error |
| Rust fmt | `cargo +nightly-2026-10-05 fmt -- --check` | 无 diff |
| IPC | `python3 scripts/verify-ipc-manifest.py` | exit 0，无 frontend orphan |
| Arch | `python3 scripts/architecture_guard.py` | exit 0 |
| TS | `cd src-frontend && npx tsc --noEmit` | 0 error |
| Vitest | `cd src-frontend && npx vitest run` | 全绿，新增用例 ≥ 20 |
| Format | `cd src-frontend && npm run format:check` | 无 diff |
| E2E | `npx playwright test` | 既存 43 + 新增 ≥ 4 |

### E2. 文档 / 版本

- 版本 0.74.0：`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json`、tag。
- `README.md`、`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。
- `landing/src/hooks/useLatestRelease.ts` 的 `FALLBACK_VERSION` 同步 0.74.0。
- `e2e/mock-tauri.ts`：新增 corpus/chat/crew/director command handlers。
- 推送后由 GitHub Actions 全平台构建；不本地 `cargo tauri build`。

---

## 7. 验收探针（逐条跑，不跑通不得宣称完成）

1. **迁移探针**：启动应用后 `schema_migrations` 含 149；`agency_runs` 有 `lane`；`idx_agency_runs_one_active_per_lane` 存在、旧索引不存在；存量 run `lane='write'`。
2. **lane 并发探针**：同 story 建 write + review 两个 pending run → 都成功；第二个 write → 结构化「已有进行中创作任务」；review 不被 write 挡住。
3. **异步派发探针**：在 write run 运行中发「让编审复审第 12 章」→ `agency_chat_send` 立即返回 `turn_task_id`；随后 `agency-chat-update` 带编审 `task_id`；编审 task 与 write 并行出现；聊天先显示「已派发」，完成后追加编审 follow-up。
4. **租约探针**：两个 `materialize_assets` 同时提交 → 第二个 `waiting` 且带 owner；第一个释放后第二个重试成功；过期租约被回收。
5. **导演修复探针**：注入一条 failed task → `agency_crew_status` 出现 `FailedTask` 告警 → 重试成功 → 告警消失；`agency_director_actions` 有执行记录。
6. **审批探针**：让导演「修订第 12 章」→ 弹审批卡；取消 → 不派发；确认 → 异步派发并落 `approved/executing/done`。
7. **语料探针**：导入文本/图片 → 列表可见 → AI 提炼 → 转为角色草稿 → 保存后 `corpus_links` 有记录 → 删故事后语料行与 `app_data_dir/corpus/<story_id>` 文件消失。
8. **回归探针**：创世、续写、批量、恢复、观察、评估/学习页全跑一遍；`cargo test --lib` 全绿。
9. **UI 探针**：1440/1024/390 宽度；对话、花名册、现场均无横向溢出；`prefers-reduced-motion` 下无动画；语料页图片懒加载。
10. **事件恢复探针**：发消息后刷新页面/重开代理工作室，会话与消息恢复；`acting` 消息在任务完成后变 `done`。

---

## 8. 风险与回滚

| 风险 | 缓解 | 回滚 |
|---|---|---|
| V149 迁移失败 | 事务包裹，失败不落版本；先备份 DB | 恢复备份；旧索引仍在（事务回滚） |
| 任务调度竞态/测试 flaky | 任务结束 await；阈值可配；测试用 MockLlm 固定时序 | 关掉 async dispatch 用 `dispatch_sync` 测试开关（仅测试） |
| 租约死锁 | TTL + heartbeat + `reap_expired`；等待不持锁 | 手动 `agency_crew_release_lease`（需确认） |
| 事件丢失 | 消息/任务落库 + 轮询 + 事件双通道 | 前端重开恢复 |
| 导演/子 agent 抢模型 | `director_sem` 1 + 全局 4 + 角色限流 | P1 设置页调低并发 |
| 覆盖作者内容 | 导演无写权限；子 agent「只补空」；审批卡 | 取消动作；质量债记录 |
| 图片内存 | 10MB 限制、详情懒加载 | 禁用图片导入 |
| prompt 占位符 | Director prompt 只声明 `premise` | registry fail-closed 测试拦截 |
| 旧功能回归 | 每阶段跑全量 Rust/vitest/Playwright | 回滚到上一 tag |

---

## 9. 任务清单（执行顺序）

| # | 任务 | 依赖 | 产出 |
|---|---|---|---|
| 1 | V149 Rust 迁移 + mod 注册 | — | 迁移、schema 测试 |
| 2 | `AgencyRun.lane` + repository + lane 映射 | 1 | 旧索引替换、lane 测试 |
| 3 | TaskReporter + ReportingLlm + ToolLoop cancel | 2 | 心跳/取消测试 |
| 4 | TaskScheduler + TaskRepository | 3 | 异步派发/完成回调测试 |
| 5 | LeaseManager + lease repository | 4 | acquire/expire/release 测试 |
| 6 | lane 映射回归（genesis/continue/batch/resume/observe） | 2,5 | 全量回归 |
| 7 | Corpus models/repository | 1 | CRUD/级联测试 |
| 8 | Corpus import service + media | 7 | 文件/图片测试 |
| 9 | Corpus distill/promote + commands | 7 | AI 草稿测试 |
| 10 | Corpus tools + story delete cleanup | 9 | 工具/清理测试 |
| 11 | AgentRole::Director + BoardZone::Direction + prompt | 2 | 角色/分区测试 |
| 12 | CrewInspector + CrewAlert | 4,5 | 巡检测试 |
| 13 | DirectorPlanner + DirectorExecutor + DirectorService | 11,12 | 计划/派发/审批测试 |
| 14 | ChatService + chat repository | 4,13 | 会话/消息/异步测试 |
| 15 | Commands + handlers + IPC 契约 | 9,10,14 | manifest/契约测试 |
| 16 | 前端类型/services/stores | 15 | tsc/vitest |
| 17 | Corpus 页 | 16 | vitest/E2E |
| 18 | AgencyStudio 三栏改造 | 16 | vitest/E2E |
| 19 | App/Sidebar/事件监听 | 17,18 | 路由/缓存测试 |
| 20 | 文档/版本/发布 | 19 | 全平台 CI |

每个任务提交前：`cargo test --lib`（或对应子集）+ `tsc`（若改前端）+ `verify-ipc-manifest.py`（若加命令）+ `architecture_guard.py`。提交信息按 `feat/fix/test/docs` 前缀。

---

## 10. 首个 commit 建议

把 A1–A5 合为一个 `feat: agency lane/task/lease 并发底座`，但拆成可审查的子提交：

1. `feat: V149 agency lane/task/lease schema`
2. `feat: agency run lane 唯一索引替换 V109`
3. `feat: task reporter + async scheduler + lease manager`
4. `test: lane/task/lease 契约与回归`

这样即使后续 B/C 延迟，并发底座也已经可独立上线、可独立回滚。
