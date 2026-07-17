# Agency P2：并行化 + 统一输出装配 + 创世切换与旧路径删除 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 P1 骨架之上实现稳态并行创作循环（编辑审 N ‖ 主创写 N+1）、按角色并发预算、质量门 v1（统一输出装配）、续写循环（含资产落库）、request_id 定点取消，并将 `smart_execute` 创世分支切换到 agency 后删除旧创世路径。

**Architecture:** 沿用 P1 黑板模型。取消/预算以"包装层"叠加（RequestRegistry + BudgetedLlm），不改 `LoopLlm` trait；质量门收敛于协调器的 `GateOutcome` 判定；并行用 `tokio::join!` 实现 gate(n-1) ∥ writer(n) 流水线；smart_execute 创世分支保持前端兼容契约（返回形状 + 事件镜像）原样切换到 agency。

**Tech Stack:** Rust（Tauri 2.4）、rusqlite + r2d2、tokio、serde/serde_json、async-trait、once_cell、uuid。

**设计文档:** `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P2 行）。
**P1 终审 P2 立项项：** request_id 定点取消；grader 质量门取代 fail-open 裁决；board 修订走 revise。

## Global Constraints

- 测试基线必须保持绿：`cargo test --lib`（现 817 passed + 2 ignored）与 `cd src-frontend && npx vitest run`（292 passed）。
- 所有 DB 同步调用在 async 上下文中必须 `tokio::task::spawn_blocking` 包裹（P1 修复波已全面合规，不得回退）。
- 已核实接口事实：`crate::error::AppError`；`AppError::validation_failed(msg, None::<String>)` 双参；`AppError::from(String)` 可用；测试内存库 `crate::db::create_test_pool()`。
- `crate::router::{TaskType, RoutingRequest}`（`RoutingRequest` 有 Default）；`LlmService::generate_for_request_with_request_id`（`llm/service.rs:501`）15 参签名：`(request, prompt, max_tokens, temperature, context_label, request_id, timeout_seconds_override, max_retries_override, intent_verb, intent_object, asset_tags, discovered_asset_ids, response_format, system_prompt, trace_id) -> (String, Result<GenerateResponse, AppError>)`。
- `LlmService::cancel_generation(request_id)` 对未知/已完成 id 是 no-op（安全）。
- **删除范围（对设计文档的修正，已核实）：只删除 `narrative/genesis.rs` 与 smart_execute 创世分支旧实现。`GenerationMode::TriShot` 与 `execute_trishot` 保留**——`planner/executor.rs:98-108/1153` 的日常续写快速路径在生产中使用它。`genesis_runs` 表与 `list_genesis_runs/get_genesis_run` 读命令保留（GenesisPanel/ContractsTab 是独立生产消费者）；`BACKGROUND_LLM_SEMAPHORE` 保留（SceneIngestor 使用）。
- **前端兼容契约**（FrontstageApp.tsx:3036-3093/3907-3915）：smart_execute 创世返回必须满足 `success: true`、`final_content` = 完整第一章正文、`messages` 含 `"story_created:{story_id}"`、`"session_id:{run_id}"`、`"novel_bootstrap_first_chapter_ready"`。
- 迁移版本：本计划无新表（P2 复用 V107 三表 + 既有资产表）；如需迁移从 V108 起。
- 事件：保留 P1 `agency-run-progress`/`agency-board-changed`；新增 `agency-agent-activity`；smart_execute 分支镜像 `smart-execute-progress`（`SmartExecuteProgress{stage,message,step_number,total_steps}`，planner/mod.rs:562）与 `novel-bootstrap-progress`。
- 版本号四文件一致（AGENTS.md:44）：git tag、`src-tauri/Cargo.toml:3`、`src-tauri/tauri.conf.json:4`、`src-frontend/package.json:4`；另同步 `AGENTS.md:10`。本计划发布版本 **0.27.0**。
- Commit 用 Conventional Commits。
- P1 既有行为不得回退：审查区只存真实裁决；draft 区写入者只有 LeadWriter；`latest_draft` 只取 status=active 且 content 非空的末条；`finish_run` 终态守护（`WHERE id=? AND status NOT IN ('cancelled','completed','failed')`）。

---

### Task 1: request_id 定点取消 + 入口护栏

**Files:**
- Modify: `src-tauri/src/agency/coordinator.rs`（RequestRegistry + AgencyLlm 重构）
- Modify: `src-tauri/src/agency/commands.rs`（cancel 改定点 + premise 校验）

**Interfaces:**
- Consumes: `generate_for_request_with_request_id`（15 参，见 Global Constraints）；`LlmService::cancel_generation`；`crate::router::RoutingRequest`（Default）。
- Produces: `agency::coordinator::{register_request, unregister_request, cancel_requests_for_run}`（registry 函数，测试与 commands 消费）；`AgencyLlm::new(app_handle, run_id)`（**签名变更**，原 `new(app_handle)` 的调用方只有 `AgencyCoordinator::new`，同步更新）；`validate_premise(premise: &str) -> Result<(), AppError>`（commands 与 Task 4/6 的 continue 入口复用）。

- [ ] **Step 1: 写失败的测试**

追加到 `src-tauri/src/agency/coordinator.rs` 测试模块：

```rust
#[test]
fn test_request_registry_lifecycle() {
    let run = "run-registry-test";
    register_request(run, "req-1");
    register_request(run, "req-2");
    register_request("other-run", "req-x");
    // 收集并清空目标 run 的全部 request_id
    let drained = drain_requests(run);
    assert_eq!(drained.len(), 2);
    assert!(drained.contains(&"req-1".to_string()));
    assert!(drained.contains(&"req-2".to_string()));
    // 已清空，再取为空
    assert!(drain_requests(run).is_empty());
    // 其他 run 不受影响
    assert_eq!(drain_requests("other-run"), vec!["req-x".to_string()]);
}

#[test]
fn test_unregister_request() {
    register_request("run-u", "req-a");
    unregister_request("run-u", "req-a");
    assert!(drain_requests("run-u").is_empty());
}

#[test]
fn test_validate_premise() {
    assert!(validate_premise("一个关于星海拾荒者的故事").is_ok());
    assert!(validate_premise("").is_err());
    assert!(validate_premise("   ").is_err());
    let too_long = "长".repeat(2001);
    assert!(validate_premise(&too_long).is_err());
    let at_limit = "长".repeat(2000);
    assert!(validate_premise(&at_limit).is_ok());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（`register_request` 等未定义）

- [ ] **Step 3: 实现**

`coordinator.rs` 顶部 statics 区（`AGENCY_CANCEL_FLAGS` 旁）追加：

```rust
/// 运行中 run 的在途 LLM request_id 注册表（定点取消用）。
static AGENCY_REQUEST_REGISTRY: Lazy<Mutex<HashMap<String, std::collections::HashSet<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_request(run_id: &str, request_id: &str) {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    registry.entry(run_id.to_string()).or_default().insert(request_id.to_string());
}

pub fn unregister_request(run_id: &str, request_id: &str) {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(set) = registry.get_mut(run_id) {
        set.remove(request_id);
        if set.is_empty() {
            registry.remove(run_id);
        }
    }
}

/// 取走并清空某 run 的全部在途 request_id。
pub fn drain_requests(run_id: &str) -> Vec<String> {
    let mut registry = AGENCY_REQUEST_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    registry.remove(run_id).map(|s| s.into_iter().collect()).unwrap_or_default()
}

/// 定点取消：仅取消该 run 的在途 LLM 调用（对已完成 id 是 no-op）。
pub fn cancel_requests_for_run(llm: &LlmService, run_id: &str) {
    for request_id in drain_requests(run_id) {
        llm.cancel_generation(&request_id);
    }
}

/// 创世/续写前提校验：非空白且 ≤2000 字符。
pub fn validate_premise(premise: &str) -> Result<(), AppError> {
    let trimmed = premise.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_failed("前提不能为空", None::<String>));
    }
    if trimmed.chars().count() > 2000 {
        return Err(AppError::validation_failed("前提过长（≤2000 字符）", None::<String>));
    }
    Ok(())
}
```

`AgencyLlm` 重构（替换 P1 版本）：

```rust
pub struct AgencyLlm {
    llm: LlmService,
    run_id: String,
}

impl AgencyLlm {
    pub fn new(app_handle: AppHandle, run_id: impl Into<String>) -> Self {
        Self { llm: LlmService::new(app_handle), run_id: run_id.into() }
    }
}

#[async_trait::async_trait]
impl LoopLlm for AgencyLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        let request_id = uuid::Uuid::new_v4().to_string();
        register_request(&self.run_id, &request_id);
        let routing = crate::router::RoutingRequest {
            task,
            ..Default::default()
        };
        let (_rid, result) = self.llm
            .generate_for_request_with_request_id(
                routing,
                user_prompt.to_string(),
                Some(max_tokens),
                None,
                Some("agency"),
                Some(request_id.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(system_prompt.to_string()),
                None,
            )
            .await;
        unregister_request(&self.run_id, &request_id);
        result.map(|r| r.content)
    }
}
```

`AgencyCoordinator::new` 相应改为（run_id 改为构造后注入——为兼容 P1 的 `new(app_handle, pool)` 签名，改为在 `run_genesis`/`run_continue` 内按 run_id 构造 AgencyLlm）：

```rust
impl AgencyCoordinator {
    pub fn new(app_handle: AppHandle, pool: DbPool) -> Self {
        Self { app_handle: Some(app_handle), pool, llm: None }
    }

    /// 测试/无界面环境构造：不发 Tauri 事件，使用注入的 mock LLM。
    pub fn for_test(pool: DbPool, llm: Arc<dyn LoopLlm>) -> Self {
        Self { app_handle: None, pool, llm: Some(llm) }
    }

    /// 按 run 取得生产 LLM（带定点取消注册）；测试时返回注入的 mock。
    fn llm_for_run(&self, run_id: &str) -> Arc<dyn LoopLlm> {
        match &self.llm {
            Some(llm) => llm.clone(),
            None => Arc::new(AgencyLlm::new(
                self.app_handle.as_ref().expect("生产 coordinator 必有 app_handle").clone(),
                run_id,
            )),
        }
    }
}
```

字段调整：`llm: Option<Arc<dyn LoopLlm>>`；`run_genesis_inner` 开头 `let llm = self.llm_for_run(run_id);`，后续 `self.llm.complete(...)` 与 `ToolLoop::new(self.llm.clone(), ...)` 全部改用该局部 `llm`（concept 调用与 `run_role` 签名同步改为接收 `llm: &Arc<dyn LoopLlm>`）。

`commands.rs` 两处修改：

```rust
// agency_start_genesis 开头加校验
pub async fn agency_start_genesis(...) -> Result<String, AppError> {
    crate::agency::coordinator::validate_premise(&premise)?;
    // ... 原有逻辑
}

// agency_cancel_run 中替换 cancel_all_generations：
pub async fn agency_cancel_run(
    run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let marked = cancel_agency_run(&run_id);
    if !marked {
        log::warn!("agency_cancel_run: run {} 无取消标记（不存在或已结束）", run_id);
    }
    let llm = crate::llm::LlmService::new(app_handle);
    crate::agency::coordinator::cancel_requests_for_run(&llm, &run_id);
    // ... 其余不变（spawn_blocking 落 cancelled 状态 + log::warn! 分支保留）
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: PASS（新增 3 个测试）；随后 `cargo test --lib 2>&1 | tail -3` 全量全绿。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs src-tauri/src/agency/commands.rs
git commit -m "feat(agency): targeted request-id cancellation + premise validation"
```

---

### Task 2: board_revise 工具 + P1 遗留小项

**Files:**
- Modify: `src-tauri/src/agency/tools.rs`（BoardReviseTool + 白名单 + ToolContext 不变）
- Modify: `src-tauri/src/agency/coordinator.rs`（修订任务指令改用 board_revise）
- Modify: `src-tauri/src/agency/repository.rs`（from_str 回退加 log::warn!）
- Modify: `src-tauri/src/agency/board.rs`（promote 发事件）
- Modify: `src-tauri/src/agency/roles.rs`（prompt 加载回归测试）

**Interfaces:**
- Consumes: `BlackboardService::revise`（P1）；`crate::prompts::registry::resolve_prompt_default`。
- Produces: 工具 `board_revise`（args `{item_id, expected_version, content, summary}`，仅 LeadWriter 白名单）；后续 Task 3/4/6 的修订路径依赖它。

- [ ] **Step 1: 写失败的测试**

`tools.rs` 测试模块追加：

```rust
#[tokio::test]
async fn test_board_revise_tool() {
    let pool = create_test_pool().unwrap();
    seed_run(&pool);
    let registry = ToolRegistry::agency_default();
    let context = ctx(pool.clone(), AgentRole::LeadWriter);
    // 先由 owner 写入 draft
    let draft = context.board.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Draft,
        "chapter", "第一章", "初稿", "初稿").unwrap();
    let revise = registry.get_for_role(AgentRole::LeadWriter, "board_revise")
        .expect("LeadWriter 应有 board_revise");
    let out = revise.execute(&context, serde_json::json!({
        "item_id": draft.id, "expected_version": 1,
        "content": "修订稿", "summary": "修订稿"
    })).await.unwrap();
    assert!(out.contains("v2") || out.contains("version=2"));
    let item = context.board.repo().get_item(&draft.id).unwrap().unwrap();
    assert_eq!(item.content, "修订稿");
    assert_eq!(item.version, 2);
    // 版本冲突 → 错误回显（工具 Ok 但内容提示冲突，或 Err——以实现为准断言其一）
    let conflict = revise.execute(&context, serde_json::json!({
        "item_id": draft.id, "expected_version": 1,
        "content": "并发", "summary": "x"
    })).await;
    assert!(conflict.is_err() || conflict.unwrap().contains("冲突"));
}

#[tokio::test]
async fn test_board_revise_whitelist() {
    let registry = ToolRegistry::agency_default();
    assert!(registry.get_for_role(AgentRole::Producer, "board_revise").is_none());
    assert!(registry.get_for_role(AgentRole::EditorAuditor, "board_revise").is_none());
}
```

`board.rs` 测试模块追加：

```rust
#[test]
fn test_promote_emits_no_panic_without_handle() {
    // 无 app_handle 时 promote 不 panic（事件 best-effort）
    let svc = board();
    seed_run(&svc, "r1");
    let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
        "world", "提案", "x", "提案").unwrap();
    svc.promote(&p.id).unwrap();
    assert_eq!(svc.snapshot("r1").unwrap().assets[0].status, "active");
}
```

`roles.rs` 测试模块追加：

```rust
#[test]
fn test_agency_prompts_loadable() {
    for role in AgentRole::all() {
        let id = spec_for(role).prompt_id;
        assert!(
            crate::prompts::registry::resolve_prompt_default(id).is_some(),
            "提示词应能被注册表加载: {}",
            id
        );
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: FAIL（`board_revise` 未注册）

- [ ] **Step 3: 实现**

`tools.rs` 新增 `BoardReviseTool`（放在 BoardWriteTool 之后）：

```rust
pub struct BoardReviseTool;

#[async_trait::async_trait]
impl AgentTool for BoardReviseTool {
    fn name(&self) -> &'static str { "board_revise" }
    fn description(&self) -> &'static str { "修订自己分区的既有条目（版本乐观锁；用于按审查意见修订草稿）" }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"item_id": "条目 id", "expected_version": "当前版本号（整数）", "content": "修订后全文", "summary": "一句话摘要"})
    }

    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<String, AppError> {
        let item_id = args.get("item_id").and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation_failed("board_revise 缺少 item_id", None::<String>))?.to_string();
        let expected_version = args.get("expected_version").and_then(|v| v.as_i64())
            .ok_or_else(|| AppError::validation_failed("board_revise 缺少 expected_version", None::<String>))? as i32;
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let board = ctx.board.clone();
        let role = ctx.role;
        tokio::task::spawn_blocking(move || {
            board.revise(&item_id, role, &content, &summary, expected_version)
        }).await.map_err(|e| AppError::from(format!("board_revise join error: {}", e)))?
        .map(|item| format!("已修订 [{}/{}] 到 v{}", item.zone.as_str(), item.key, item.version))
    }
}
```

`ToolRegistry::agency_default()` 中追加：

```rust
registry.register(Arc::new(BoardReviseTool));
registry.allow(AgentRole::LeadWriter, "board_revise");
```

`coordinator.rs` 修订任务指令（review 循环内 revise 分支）改为携带条目坐标：

```rust
let revise_out = self.run_role(
    AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
    &format!(
        "修订「{}」。先用 board_revise 直接修订该条目（item_id={}, expected_version={}），content 为完整修订稿。审查阻断问题：{}",
        draft.key, draft.id, draft.version, issues
    ),
).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
```

`repository.rs` 的 `map_board_item` / `map_message` 中 4 处 `unwrap_or` 回退前加日志：

```rust
// 例：zone 字段
let zone = BoardZone::from_str(&zone_str).unwrap_or_else(|| {
    log::warn!("agency_board_items 非法 zone 值 {:?}，回退 asset", zone_str);
    BoardZone::Asset
});
```
（producer/from_role/to_role 三处同模式。）

`board.rs` 的 `promote` 改为发事件：

```rust
pub fn promote(&self, item_id: &str) -> Result<(), AppError> {
    self.repo.promote_item(item_id).map_err(AppError::from)?;
    if let Ok(Some(item)) = self.repo.get_item(item_id) {
        self.emit_changed(&item);
    }
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 4 个）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/tools.rs src-tauri/src/agency/coordinator.rs src-tauri/src/agency/repository.rs src-tauri/src/agency/board.rs src-tauri/src/agency/roles.rs
git commit -m "feat(agency): board_revise tool with optimistic lock + small P1 follow-ups"
```

---

### Task 3: 质量门 v1（统一输出装配器）

**Files:**
- Create: `src-tauri/src/agency/gate.rs`
- Modify: `src-tauri/src/agency/coordinator.rs`（review 块重构为 evaluate_gate 调用）
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod gate;`）

**Interfaces:**
- Consumes: `crate::agents::subagents::{run_subagent_review, ReviewNotes, ReviewSeverity}`；`crate::domain::agent_context::{AgentContext, CharacterInfo}`（`AgentContext::minimal(story_id, input)`，domain/agent_context.rs:159-166）；Task 2 的 board_revise 修订路径。
- Produces: `GateOutcome::{Passed{verdict}, RevisionRequired{verdict, issues}, Failed{reason}}`；`AgencyCoordinator::evaluate_gate(llm, board, registry, run_id, story_id, premise, draft) -> Result<GateOutcome, AppError>`；`gate::merge_rule_issues(notes: &[ReviewNotes]) -> Vec<String>`（High 及以上格式化为 "­[agent] category: description"）；`gate::build_review_context(pool, story_id, foreshadowing_hints) -> AgentContext`。Task 4/6 复用同一门径。

**行为规格（取代 P1 的 fail-open）：**
1. editor LoopResult.aborted → `Failed`（P1 修复波行为，保持）；
2. 裁决 JSON 解析失败 → **重试 editor 一次**；仍失败 → `Failed{reason: "裁决解析失败"}`（不再默认 pass）；
3. verdict=revise 且 blocking_issues 非空 → `RevisionRequired`；
4. verdict=pass → 规则复检 `run_subagent_review`：任一 High+ 问题 → `RevisionRequired`（issues 来自规则审查）；否则 `Passed`；
5. 每次门判定写审查区（item_type="gate"，content=裁决 JSON + 规则问题数，status=active）。

- [ ] **Step 1: 写失败的测试**

`gate.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::subagents::{ReviewIssue, ReviewNotes, ReviewSeverity};

    #[test]
    fn test_merge_rule_issues_high_and_above_only() {
        let mut notes = ReviewNotes::new("style", "风格审查");
        notes.add_issue(ReviewIssue::new(ReviewSeverity::High, "AI腔", "陈词滥调", "删掉"));
        notes.add_issue(ReviewIssue::new(ReviewSeverity::Low, "句长", "偏长", "可拆"));
        let mut notes2 = ReviewNotes::new("continuity", "连续性");
        notes2.add_issue(ReviewIssue::new(ReviewSeverity::Critical, "矛盾", "角色已死却出场", "改"));
        let merged = merge_rule_issues(&[notes, notes2]);
        assert_eq!(merged.len(), 2);
        assert!(merged[0].contains("AI腔"));
        assert!(merged[1].contains("矛盾"));
    }

    #[test]
    fn test_merge_empty() {
        assert!(merge_rule_issues(&[]).is_empty());
        assert!(merge_rule_issues(&[ReviewNotes::new("world", "无问题")]).is_empty());
    }
}
```

`coordinator.rs` 测试模块追加（MockLlm 沿用 P1 模式）：

```rust
#[tokio::test]
async fn test_gate_fails_after_verdict_parse_retry() {
    let pool = create_test_pool().unwrap();
    // concept + producer(tool,final) + writer(tool,final) + editor 两次非法裁决
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
        r#"{"type":"final","content":"资产就绪"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"正文。","summary":"初稿"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        r#"{"type":"final","content":"这根本不是JSON裁决"}"#,
        r#"{"type":"final","content":"依然不是JSON"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let err = coordinator.run_genesis("r-gate-1", "前提").await.unwrap_err();
    assert!(err.to_string().contains("质量门") || err.to_string().contains("裁决") || err.to_string().contains("审查"));
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(repo.get_run("r-gate-1").unwrap().unwrap().status, "failed");
}

#[tokio::test]
async fn test_revision_uses_board_revise_in_place() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
        r#"{"type":"final","content":"资产就绪"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"初稿。","summary":"初稿"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        // editor: revise
        r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"动机缺失\"],\"suggestions\":[],\"comments\":\"修\"}"}"#,
        // writer 修订：走 board_revise（item_id/expected_version 由协调器注入指令，mock 只需给出正确 JSON；测试不校验模型是否真读到指令，校验 revise 语义生效）
        r#"{"type":"tool","name":"board_revise","args":{"item_id":"__WILL_BE_REPLACED__","expected_version":1,"content":"修订稿：他为生存拾荒。","summary":"修订稿"}}"#,
        r#"{"type":"final","content":"修订完成"}"#,
        // 复审：pass
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#,
    ]);
    // 由于 mock 无法预知 item_id，本测试改用动态 mock（见下），此用例在实现时以动态 mock 编写：
    // DynamicMock：收到含 "修订" 的任务时，先 board_read 取 draft 条目 id 再 board_revise。
    // 断言：draft 区仍只有 1 条（revise 原地更新而非新行），version=2，scene 内容为修订稿。
}
```

注：`test_revision_uses_board_revise_in_place` 需要动态 mock——实现一个 `struct DynamicMockLlm`（`complete` 内根据 user_prompt 内容分支应答，先用 `board_read` 的真实逻辑？不允许——mock 只回脚本化应答，但 item_id 可由测试先创建 draft 后填入脚本：测试自己先 board.write 一条 draft 再把 item_id 填入 mock 脚本，coordinator 的 writer 阶段若再写一条则 draft 变 2 条违背断言）。**简化**：该用例改为直接测协调器修订指令的生成：`build_revision_task(draft, issues)` 纯函数包含 item_id 与 expected_version（见 Step 3），动态行为由 `tools.rs` 的 board_revise 测试（Task 2）与端到端 pass 路径覆盖。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: FAIL（`merge_rule_issues` / gate 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/gate.rs`：

```rust
use crate::agents::subagents::{ReviewNotes, ReviewSeverity};
use crate::db::DbPool;
use crate::domain::agent_context::{AgentContext, CharacterInfo};

/// 收集规则审查中 High 及以上问题，格式化为 "[agent] category: description"。
pub fn merge_rule_issues(notes: &[ReviewNotes]) -> Vec<String> {
    notes
        .iter()
        .flat_map(|n| {
            n.issues.iter().filter_map(move |i| {
                if i.severity >= ReviewSeverity::High {
                    Some(format!("[{}] {}: {}", n.agent, i.category, i.description))
                } else {
                    None
                }
            })
        })
        .collect()
}

/// 为规则复检构建最小 AgentContext：角色/世界规则取自 DB 资产（Task 4 落库后可用），
/// 活跃线索取自黑板伏笔条目摘要。
pub fn build_review_context(pool: &DbPool, story_id: &str, foreshadowing_hints: &[String]) -> AgentContext {
    let mut ctx = AgentContext::minimal(story_id.to_string(), String::new());
    if let Ok(chars) = crate::db::repositories::CharacterRepository::new(pool.clone()).get_by_story(story_id) {
        ctx.narrative.characters = chars
            .iter()
            .map(|c| CharacterInfo {
                name: c.name.clone(),
                personality: c.personality.clone().unwrap_or_default(),
                role: String::new(),
                appearance: c.appearance.clone(),
                gender: c.gender.clone(),
                age: c.age,
            })
            .collect();
    }
    if let Ok(Some(world)) = crate::db::repositories::WorldBuildingRepository::new(pool.clone()).get_by_story(story_id) {
        let rules_text = serde_json::to_string(&world.rules).unwrap_or_default();
        ctx.world.world_rules = Some(format!("{}\n{}", world.concept, rules_text));
    }
    ctx.narrative.active_threads = foreshadowing_hints.to_vec();
    ctx
}
```

注：`CharacterRepository::get_by_story` / `WorldBuildingRepository::get_by_story` 的精确方法名用 Grep 在 `src-tauri/src/db/repositories/character_repository.rs` / `world_building_repository.rs` 确认（recon 显示两文件均有按 story 的 SELECT，约 :151 / :138 行）；`Character` 字段见 models.rs:1127（`appearance/gender/age` 为 Option）。若方法名不同以实际为准。

`coordinator.rs`：新增 `GateOutcome` 与 `evaluate_gate`，替换 P1 review 块：

```rust
#[derive(Debug)]
pub enum GateOutcome {
    Passed { verdict: EditorVerdict },
    RevisionRequired { verdict: EditorVerdict, issues: Vec<String> },
    Failed { reason: String },
}

impl AgencyCoordinator {
    /// 质量门：editor 裁决（解析失败重试 1 次）→ pass 后再经规则复检。
    pub(crate) async fn evaluate_gate(
        &self,
        llm: &Arc<dyn LoopLlm>,
        board: &BlackboardService,
        registry: &Arc<ToolRegistry>,
        run_id: &str,
        story_id: &str,
        premise: &str,
        draft: &BoardItem,
    ) -> Result<GateOutcome, AppError> {
        // 1) editor 裁决（解析失败重试一次）
        let mut verdict: Option<EditorVerdict> = None;
        let mut last_raw = String::new();
        for attempt in 0..2 {
            let editor_out = self.run_role_with_llm(
                llm, AgentRole::EditorAuditor, board, registry, run_id, story_id, premise,
                &format!("审查 draft 区的最新章节草稿（{}）。按系统提示词出具裁决 JSON。", draft.key),
            ).await.map_err(|e| AppError::from(format!("编辑审计 Agent 阶段失败: {}", e)))?;
            if editor_out.aborted {
                return Ok(GateOutcome::Failed { reason: "编辑审计 Agent 被熔断".to_string() });
            }
            last_raw = editor_out.output.clone();
            if let Some(v) = parse_lenient::<EditorVerdict>(&editor_out.output) {
                verdict = Some(v);
                break;
            }
            log::warn!("agency gate: 裁决解析失败（第 {} 次）", attempt + 1);
        }
        let verdict = match verdict {
            Some(v) => v,
            None => return Ok(GateOutcome::Failed { reason: format!("裁决解析失败（重试 1 次后仍失败）: {}", last_raw.chars().take(120).collect::<String>()) }),
        };
        // 2) 判定
        let outcome = if verdict.verdict == "revise" && !verdict.blocking_issues.is_empty() {
            GateOutcome::RevisionRequired { issues: verdict.blocking_issues.clone(), verdict }
        } else {
            // 规则复检（确定性优先：LLM 说 pass 也要过规则）
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            let hints = board.list_zone(run_id, BoardZone::Asset)?
                .into_iter()
                .filter(|i| i.item_type == "foreshadowing")
                .map(|i| i.summary)
                .collect::<Vec<_>>();
            let content = draft.content.clone();
            let rule_issues = tokio::task::spawn_blocking(move || {
                let ctx = crate::agency::gate::build_review_context(&pool, &sid, &hints);
                ctx
            }).await.map_err(|e| AppError::from(format!("gate ctx join error: {}", e)))?;
            let notes = crate::agents::subagents::run_subagent_review(&rule_issues, &content).await;
            let merged = crate::agency::gate::merge_rule_issues(&notes);
            if merged.is_empty() {
                GateOutcome::Passed { verdict }
            } else {
                GateOutcome::RevisionRequired { issues: merged, verdict }
            }
        };
        // 3) 判定落审查区
        let (kind, detail) = match &outcome {
            GateOutcome::Passed { .. } => ("pass", String::new()),
            GateOutcome::RevisionRequired { issues, .. } => ("revise", format!("{} 条问题", issues.len())),
            GateOutcome::Failed { reason } => ("failed", reason.clone()),
        };
        let summary = format!("gate:{} {} {}", kind, detail, verdict_comments(&outcome));
        board.write(run_id, story_id, AgentRole::EditorAuditor, BoardZone::Review,
            "gate", &format!("gate-{}", draft.key),
            &format!("{:?}", std::mem::discriminant(&outcome)),
            &summary.chars().take(80).collect::<String>())?;
        Ok(outcome)
    }

    /// 供 Task 2 修订路径与测试使用的指令生成（纯函数）。
    pub(crate) fn build_revision_task(draft: &BoardItem, issues: &[String]) -> String {
        format!(
            "修订「{}」。先用 board_revise 直接修订该条目（item_id={}, expected_version={}），content 为完整修订稿。审查阻断问题：{}",
            draft.key, draft.id, draft.version, issues.join("；")
        )
    }
}

fn verdict_comments(outcome: &GateOutcome) -> String {
    match outcome {
        GateOutcome::Passed { verdict } => verdict.comments.clone(),
        GateOutcome::RevisionRequired { verdict, .. } => verdict.comments.clone(),
        GateOutcome::Failed { .. } => String::new(),
    }
}
```

`run_genesis_inner` 的 review/revision 块重构为：

```rust
// 5) 质量门 + 至多 1 轮修订
let mut revised = false;
let final_verdict = loop {
    repo.update_run_phase(run_id, "running", "review").map_err(AppError::from)?;
    self.emit_progress(run_id, "review", "running", "质量门评估中");
    let outcome = self.evaluate_gate(&llm, &board, &registry, run_id, &story_id, premise, &draft).await?;
    match outcome {
        GateOutcome::Passed { verdict } => break verdict,
        GateOutcome::RevisionRequired { issues, verdict } if !revised => {
            revised = true;
            repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
            self.emit_progress(run_id, "revision", "running", "主创 Agent 正在按审查意见修订");
            let task = Self::build_revision_task(&draft, &issues);
            let revise_out = self.run_role_with_llm(
                &llm, AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise, &task,
            ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
            if revise_out.aborted {
                return Err(AppError::from("主创 Agent 修订轮被熔断"));
            }
            draft = self.latest_draft(&board, run_id)?;
            self.check_cancel(cancel)?;
            // 复审：无论结果都进入装配（Failed 除外）
            let second = self.evaluate_gate(&llm, &board, &registry, run_id, &story_id, premise, &draft).await?;
            match second {
                GateOutcome::Passed { verdict } => break verdict,
                GateOutcome::RevisionRequired { verdict, .. } => break verdict, // 第二轮放行
                GateOutcome::Failed { reason } => {
                    return Err(AppError::from(format!("质量门未通过: {}", reason)));
                }
            }
        }
        GateOutcome::RevisionRequired { verdict, .. } => break verdict,
        GateOutcome::Failed { reason } => {
            return Err(AppError::from(format!("质量门未通过: {}", reason)));
        }
    }
};
```

配套重构：`run_role` 拆为 `run_role_with_llm(llm, role, ...)`（显式传 LLM）+ 保留 `run_role` 委托（减少调用点改动）；`run_role_with_llm` 内部与 P1 `run_role` 相同（spec/system_prompt/ToolContext/ToolLoop）。`draft` 变量需改为 `let mut draft`（P1 已是）。EditorVerdict 的 `final_verdict` 替换 P1 的 `verdict` 用于结果。

注意 P1 兼容性：`evaluate_gate` 中 editor aborted → `Failed`（与 P1 修复波的 `Err` 等价，外层映射为 run failed）；P1 既有测试 `test_genesis_aborts_when_editor_aborted` 仍须通过（Err 文案变为"质量门未通过： 编辑审计 Agent 被熔断"——断言含"熔断"仍成立）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 3+）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/gate.rs src-tauri/src/agency/coordinator.rs src-tauri/src/agency/mod.rs
git commit -m "feat(agency): quality gate v1 replacing fail-open verdict (retry + rule recheck)"
```

---

### Task 4: 续写循环 agency_continue + 资产落库 + AssetQueryTool

**Files:**
- Create: `src-tauri/src/agency/materialize.rs`
- Modify: `src-tauri/src/agency/tools.rs`（AssetQueryTool + 白名单）
- Modify: `src-tauri/src/agency/coordinator.rs`（run_continue + genesis 落库调用）
- Modify: `src-tauri/src/agency/repository.rs`（`has_running_run_for_story` + `next_chapter_number` 辅助可放 coordinator）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_continue_chapter`）
- Modify: `src-tauri/src/handlers.rs`（注册新命令）
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod materialize;`）
- Modify: `resources/prompts/agency/agency_producer_system.md`（JSON 资产格式约定）

**Interfaces:**
- Consumes: 资产表列（Global Constraints 已列，直接 SQL）；`SceneRepository::create/update`；Task 3 的 `evaluate_gate`。
- Produces: `materialize::materialize_assets(pool, story_id, items: &[BoardItem]) -> usize`；`AgencyCoordinator::run_continue(run_id, story_id, chapter_number) -> Result<AgencyContinueResult, AppError>`；`AgencyContinueResult { run_id, story_id, scene_id, chapter_number, revised, verdict }`；工具 `asset_query`（args `{kind: "characters"|"world"|"outline"|"scenes"}`，三角色可读）；`AgencyRepository::has_running_run_for_story(story_id) -> Result<bool, rusqlite::Error>`；IPC `agency_continue_chapter(story_id) -> String`。

**资产表列（直接 SQL 用，已核实）：**
- `characters(id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)`（无 role 列）
- `world_buildings(id, story_id UNIQUE, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)`
- `story_outlines(id, story_id UNIQUE, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)`

- [ ] **Step 1: 写失败的测试**

`materialize.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::db::create_test_pool;
    use crate::db::dto::CreateStoryRequest;
    use crate::db::repositories::StoryRepository;

    fn story(pool: &crate::db::DbPool, id: &str) {
        let s = StoryRepository::new(pool.clone()).create(CreateStoryRequest {
            title: "测试书".into(), description: None, genre: None,
            style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
        }).unwrap();
        // create 生成自己的 id；测试统一改用其返回值
        let conn = pool.get().unwrap();
        conn.execute("UPDATE stories SET id = ?1 WHERE id = ?2", rusqlite::params![id, s.id]).unwrap();
    }

    fn item(item_type: &str, key: &str, content: &str) -> BoardItem {
        BoardItem::new("r1", "s1", BoardZone::Asset, item_type, key, content, "摘要", AgentRole::Producer, "active")
    }

    #[test]
    fn test_materialize_character_json() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("character", "主角", r#"{"name":"阿苔","background":"拾荒者","personality":"坚韧","goals":"找到星环"}"#)];
        let n = materialize_assets(&pool, "s1", &items);
        assert_eq!(n, 1);
        let conn = pool.get().unwrap();
        let name: String = conn.query_row("SELECT name FROM characters WHERE story_id='s1'", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "阿苔");
    }

    #[test]
    fn test_materialize_world_upsert_idempotent() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("world", "世界观", "双星废土，磁力风暴")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        // 再次执行不报错（upsert），仍一行
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM world_buildings WHERE story_id='s1'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_materialize_skips_non_json_character() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("character", "主角", "自由文本不是 JSON")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
        let conn = pool.get().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM characters WHERE story_id='s1'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_materialize_outline() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("outline", "第一卷", "第一卷大纲：起承转合……")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let content: String = conn.query_row("SELECT content FROM story_outlines WHERE story_id='s1'", [], |r| r.get(0)).unwrap();
        assert!(content.contains("起承转合"));
    }
}
```

`coordinator.rs` 测试模块追加（续写端到端，资产已存在则无需 producer）：

```rust
#[tokio::test]
async fn test_continue_chapter_end_to_end() {
    let pool = create_test_pool().unwrap();
    // 预置故事 + 一个角色 + 第一章场景
    let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
        title: "续写书".into(), description: Some("前提".into()), genre: None,
        style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
    }).unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo.update(&ch1.id, &crate::db::repositories::SceneUpdate {
        content: Some("第一章正文。".to_string()),
        ..Default::default()
    }).unwrap();

    let llm = MockLlm::scripted(vec![
        // writer: 查前文 + 写第二章
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第二章","content":"第二章正文：星舰苏醒。","summary":"星舰苏醒"}}"#,
        r#"{"type":"final","content":"第二章完成"}"#,
        // editor: pass
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator.run_continue("rc-1", &story.id, 2).await.unwrap();
    assert_eq!(result.chapter_number, 2);
    let scene = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id).unwrap().unwrap();
    assert_eq!(scene.content.as_deref(), Some("第二章正文：星舰苏醒。"));
    let run = AgencyRepository::new(pool.clone()).get_run("rc-1").unwrap().unwrap();
    assert_eq!(run.status, "completed");
}

#[tokio::test]
async fn test_continue_fails_without_assets_and_producer_aborts() {
    // 无资产且 producer 熔断 → failed（验证资产补齐路径的熔断传播）
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
        title: "无资产书".into(), description: None, genre: None,
        style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
    }).unwrap();
    let llm = MockLlm::scripted(vec!["不是 JSON", "还不是", "依然不是"]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let err = coordinator.run_continue("rc-2", &story.id, 1).await.unwrap_err();
    assert!(err.to_string().contains("管理") || err.to_string().contains("熔断") || err.to_string().contains("资产"));
    assert_eq!(AgencyRepository::new(pool.clone()).get_run("rc-2").unwrap().unwrap().status, "failed");
}
```

`repository.rs` 测试追加：

```rust
#[test]
fn test_has_running_run_for_story() {
    let (repo, _) = repo();
    let mut run = AgencyRun::new("r1", "前提");
    run.story_id = Some("s1".into());
    repo.create_run(&run).unwrap();
    repo.update_run_phase("r1", "running", "assets").unwrap();
    assert!(repo.has_running_run_for_story("s1").unwrap());
    repo.finish_run("r1", "completed", None, None).unwrap();
    assert!(!repo.has_running_run_for_story("s1").unwrap());
    assert!(!repo.has_running_run_for_story("s2").unwrap());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: FAIL（`materialize_assets` / `run_continue` / `has_running_run_for_story` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/materialize.rs`：

```rust
//! 资产落库：把黑板资产区的条目物化到应用资产表（characters/world_buildings/story_outlines）。
//! character 条目 content 须为 JSON {"name","background","personality","goals"}；
//! world/outline 条目 content 为纯文本。解析失败的条目跳过并 log::warn!。

use rusqlite::params;

use crate::agency::models::BoardItem;
use crate::db::DbPool;

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn materialize_assets(pool: &DbPool, story_id: &str, items: &[BoardItem]) -> usize {
    let mut count = 0usize;
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("materialize_assets: pool 获取失败: {}", e);
            return 0;
        }
    };
    for item in items.iter().filter(|i| i.status == "active") {
        match item.item_type.as_str() {
            "character" => {
                let parsed = crate::agency::coordinator::parse_lenient::<serde_json::Value>(&item.content);
                let (name, background, personality, goals) = match parsed.as_ref() {
                    Some(v) => (
                        v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        v.get("background").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        v.get("personality").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        v.get("goals").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ),
                    None => {
                        log::warn!("materialize: 角色条目 {} 非 JSON，跳过", item.key);
                        continue;
                    }
                };
                if name.is_empty() {
                    log::warn!("materialize: 角色条目 {} 缺 name，跳过", item.key);
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                match conn.execute(
                    "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'agency', 1, ?7, ?8)",
                    params![id, story_id, name, background, personality, goals, ts, ts],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 插入角色失败: {}", e),
                }
            }
            "world" => {
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO world_buildings (id, story_id, concept, rules, source, is_auto_generated, created_at, updated_at)
                     VALUES (?1, ?2, ?3, '[]', 'agency', 1, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET concept = excluded.concept, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入世界观失败: {}", e),
                }
            }
            "outline" => {
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO story_outlines (id, story_id, content, act_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 3, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入大纲失败: {}", e),
                }
            }
            _ => {} // foreshadowing 等 P2 不落库
        }
    }
    count
}
```

`tools.rs` 新增 `AssetQueryTool`（三角色白名单 `asset_query`）：

```rust
pub struct AssetQueryTool;

#[async_trait::async_trait]
impl AgentTool for AssetQueryTool {
    fn name(&self) -> &'static str { "asset_query" }
    fn description(&self) -> &'static str { "查询故事资产库：characters 角色卡 / world 世界观 / outline 大纲 / scenes 最近场景摘要" }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"kind": "characters|world|outline|scenes"})
    }

    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<String, AppError> {
        let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let pool = ctx.pool.clone();
        let story_id = ctx.story_id.clone();
        tokio::task::spawn_blocking(move || -> Result<String, AppError> {
            let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let out = match kind.as_str() {
                "characters" => {
                    let mut stmt = conn.prepare(
                        "SELECT name, COALESCE(personality,''), COALESCE(goals,''), COALESCE(background,'')
                         FROM characters WHERE story_id = ?1 ORDER BY created_at LIMIT 20")?;
                    let rows = stmt.query_map(params![story_id], |r| {
                        Ok(format!("- {}｜性格:{}｜目标:{}｜背景:{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
                    })?;
                    let list: Vec<String> = rows.collect::<Result<_, _>>()?;
                    if list.is_empty() { "（资产库无角色）".to_string() } else { list.join("\n") }
                }
                "world" => {
                    conn.query_row(
                        "SELECT concept, COALESCE(history,'') FROM world_buildings WHERE story_id = ?1",
                        params![story_id],
                        |r| Ok(format!("概念: {}\n历史: {}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    ).optional()?.unwrap_or_else(|| "（资产库无世界观）".to_string())
                }
                "outline" => {
                    conn.query_row(
                        "SELECT content FROM story_outlines WHERE story_id = ?1",
                        params![story_id],
                        |r| r.get::<_, String>(0),
                    ).optional()?.unwrap_or_else(|| "（资产库无大纲）".to_string())
                }
                "scenes" => {
                    let mut stmt = conn.prepare(
                        "SELECT sequence_number, COALESCE(title,''), substr(COALESCE(content,''),1,200)
                         FROM scenes WHERE story_id = ?1 ORDER BY sequence_number DESC LIMIT 5")?;
                    let mut rows: Vec<String> = stmt.query_map(params![story_id], |r| {
                        Ok(format!("- 第{}场 {}: {}…", r.get::<_, i32>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                    })?.collect::<Result<_, _>>()?;
                    rows.reverse(); // 恢复时间序
                    if rows.is_empty() { "（尚无场景）".to_string() } else { rows.join("\n") }
                }
                other => return Ok(format!("非法 kind: {}，可选 characters|world|outline|scenes", other)),
            };
            Ok(out)
        }).await.map_err(|e| AppError::from(format!("asset_query join error: {}", e)))?
    }
}
```

注册：`registry.register(Arc::new(AssetQueryTool));` + `for role in AgentRole::all() { registry.allow(role, "asset_query"); }`。`use rusqlite::OptionalExtension;`（tools.rs 已有则复用）。

`repository.rs` 追加：

```rust
pub fn has_running_run_for_story(&self, story_id: &str) -> Result<bool, rusqlite::Error> {
    let conn = self.pool.get().map_err(pool_err)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agency_runs WHERE story_id = ?1 AND status IN ('pending', 'running')",
        params![story_id],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}
```

`coordinator.rs` 追加（含 genesis 落库调用——`run_genesis_inner` 在 producer 阶段完成后、`writer` 阶段前插入）：

```rust
// producer 完成后落库（新增）
{
    let pool = self.pool.clone();
    let sid = story_id.clone();
    let assets = board.list_zone(run_id, BoardZone::Asset)?;
    let inserted = tokio::task::spawn_blocking(move || {
        crate::agency::materialize::materialize_assets(&pool, &sid, &assets)
    }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
    log::info!("agency: 资产落库 {} 条", inserted);
}
```

`AgencyContinueResult` 与 `run_continue`：

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgencyContinueResult {
    pub run_id: String,
    pub story_id: String,
    pub scene_id: String,
    pub chapter_number: i32,
    pub revised: bool,
    pub verdict: EditorVerdict,
}

impl AgencyCoordinator {
    /// 续写循环（串行）：资产确认/补齐 → 写作 → 质量门 → 装配。
    pub async fn run_continue(
        &self,
        run_id: &str,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<AgencyContinueResult, AppError> {
        let repo = AgencyRepository::new(self.pool.clone());
        let cancel = register_agency_cancel(run_id);
        let result = self.run_continue_inner(run_id, story_id, chapter_number, &repo, &cancel).await;
        unregister_agency_cancel(run_id);
        match &result {
            Ok(r) => {
                let json = serde_json::to_string(r).unwrap_or_default();
                let _ = repo.finish_run(run_id, "completed", Some(&json), None);
                self.emit_progress(run_id, "assembly", "completed", "续写完成");
            }
            Err(e) => {
                let status = if cancel.load(std::sync::atomic::Ordering::SeqCst) { "cancelled" } else { "failed" };
                let _ = repo.finish_run(run_id, status, None, Some(&e.to_string()));
                self.emit_progress(run_id, "assembly", status, &e.to_string());
            }
        }
        result
    }

    async fn run_continue_inner(
        &self,
        run_id: &str,
        story_id: &str,
        chapter_number: i32,
        repo: &AgencyRepository,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AgencyContinueResult, AppError> {
        let llm = self.llm_for_run(run_id);
        let title = self.story_title(story_id).await.unwrap_or_else(|| "未命名".to_string());
        let premise = format!("续写《{}》第{}章", title, chapter_number);
        repo.create_run(&AgencyRun::new(run_id, &premise)).map_err(AppError::from)?;
        repo.set_run_story(run_id, story_id).map_err(AppError::from)?;
        repo.update_run_phase(run_id, "running", "assets").map_err(AppError::from)?;
        self.emit_progress(run_id, "assets", "running", "正在确认创作资产");

        // 1) 资产确认/补齐
        let has_assets = self.db(move || {
            let conn = pool_ref... // 用 self.db 辅助（P1 修复波已有），或直接 spawn_blocking
        }).await;
        // 实现：spawn_blocking 查询 COUNT(characters) + 若 0 则先从本 story 历史黑板条目落库
        let character_count = {
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            tokio::task::spawn_blocking(move || -> Result<i64, AppError> {
                let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
                conn.query_row("SELECT COUNT(*) FROM characters WHERE story_id = ?1",
                    rusqlite::params![sid], |r| r.get(0)).map_err(AppError::from)
            }).await.map_err(|e| AppError::from(format!("asset check join error: {}", e)))??
        };
        if character_count == 0 {
            // 先尝试从本 story 历史黑板条目落库（免费路径）
            let pool = self.pool.clone();
            let sid = story_id.to_string();
            let history_items = repo.list_items_for_story(story_id, Some(BoardZone::Asset)).map_err(AppError::from)?;
            let inserted = tokio::task::spawn_blocking(move || {
                crate::agency::materialize::materialize_assets(&pool, &sid, &history_items)
            }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
            if inserted == 0 {
                // 仍无资产：producer 现场补齐
                let board = self.board();
                let registry = Arc::new(ToolRegistry::agency_default());
                let producer_out = self.run_role_with_llm(
                    &llm, AgentRole::Producer, &board, &registry, run_id, story_id, &premise,
                    "为这部已有故事补齐创作资产：先 story_info 与 asset_query 了解现状，再生产世界观/角色卡（JSON 格式）/大纲，写入资产区。",
                ).await.map_err(|e| AppError::from(format!("管理 Agent 资产补齐失败: {}", e)))?;
                if producer_out.aborted {
                    return Err(AppError::from("管理 Agent 被熔断，资产补齐未完成"));
                }
                let assets = board.list_zone(run_id, BoardZone::Asset)?;
                let pool = self.pool.clone();
                let sid = story_id.to_string();
                tokio::task::spawn_blocking(move || {
                    crate::agency::materialize::materialize_assets(&pool, &sid, &assets)
                }).await.map_err(|e| AppError::from(format!("materialize join error: {}", e)))?;
            }
        }
        self.check_cancel(cancel)?;

        // 2) 写作
        repo.update_run_phase(run_id, "running", "writing").map_err(AppError::from)?;
        self.emit_progress(run_id, "writing", "running", &format!("主创 Agent 正在写作第{}章", chapter_number));
        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let key = format!("第{}章", chapter_number);
        let writer_out = self.run_role_with_llm(
            &llm, AgentRole::LeadWriter, &board, &registry, run_id, story_id, &premise,
            &format!(
                "续写{}（1500-2500 字）。先 board_read 读资产区、asset_query(kind=scenes) 读最近场景保持连贯，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key={}）。",
                key, key
            ),
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，本章未完成"));
        }
        let mut draft = self.latest_draft(&board, run_id)?;
        self.check_cancel(cancel)?;

        // 3) 质量门 + 至多 1 轮修订（与 genesis 同门径）
        let mut revised = false;
        let final_verdict = loop {
            repo.update_run_phase(run_id, "running", "review").map_err(AppError::from)?;
            self.emit_progress(run_id, "review", "running", "质量门评估中");
            let outcome = self.evaluate_gate(&llm, &board, &registry, run_id, story_id, &premise, &draft).await?;
            match outcome {
                GateOutcome::Passed { verdict } => break verdict,
                GateOutcome::RevisionRequired { issues, verdict } if !revised => {
                    revised = true;
                    repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
                    let task = Self::build_revision_task(&draft, &issues);
                    let revise_out = self.run_role_with_llm(
                        &llm, AgentRole::LeadWriter, &board, &registry, run_id, story_id, &premise, &task,
                    ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                    if revise_out.aborted {
                        return Err(AppError::from("主创 Agent 修订轮被熔断"));
                    }
                    draft = self.latest_draft(&board, run_id)?;
                    self.check_cancel(cancel)?;
                    let second = self.evaluate_gate(&llm, &board, &registry, run_id, story_id, &premise, &draft).await?;
                    match second {
                        GateOutcome::Passed { verdict } => break verdict,
                        GateOutcome::RevisionRequired { verdict, .. } => break verdict,
                        GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
                    }
                }
                GateOutcome::RevisionRequired { verdict, .. } => break verdict,
                GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
            }
        };

        // 4) 装配
        repo.update_run_phase(run_id, "running", "assembly").map_err(AppError::from)?;
        let pool = self.pool.clone();
        let sid = story_id.to_string();
        let content = draft.content.clone();
        let title_c = key.clone();
        let scene = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
            let repo = crate::db::repositories::SceneRepository::new(pool);
            let scene = repo.create(&sid, chapter_number, Some(&title_c)).map_err(AppError::from)?;
            repo.update(&scene.id, &crate::db::repositories::SceneUpdate {
                content: Some(content),
                ..Default::default()
            }).map_err(AppError::from)?;
            Ok(scene)
        }).await.map_err(|e| AppError::from(format!("scene assembly join error: {}", e)))??;

        Ok(AgencyContinueResult {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            scene_id: scene.id,
            chapter_number,
            revised,
            verdict: final_verdict,
        })
    }

    async fn story_title(&self, story_id: &str) -> Option<String> {
        let pool = self.pool.clone();
        let sid = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().ok()?;
            conn.query_row("SELECT title FROM stories WHERE id = ?1",
                rusqlite::params![sid], |r| r.get::<_, String>(0)).ok()
        }).await.ok().flatten()
    }
}
```

`repository.rs` 追加 `list_items_for_story`：

```rust
pub fn list_items_for_story(&self, story_id: &str, zone: Option<BoardZone>) -> Result<Vec<BoardItem>, rusqlite::Error> {
    let conn = self.pool.get().map_err(pool_err)?;
    let items = match zone {
        Some(z) => {
            let mut stmt = conn.prepare(
                "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                 FROM agency_board_items WHERE story_id = ?1 AND zone = ?2 ORDER BY created_at ASC, rowid ASC")?;
            let rows = stmt.query_map(params![story_id, z.as_str()], map_board_item)?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                 FROM agency_board_items WHERE story_id = ?1 ORDER BY created_at ASC, rowid ASC")?;
            let rows = stmt.query_map(params![story_id], map_board_item)?;
            rows.collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(items)
}
```

`commands.rs` 追加（含并发护栏）：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_chapter(
    story_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();
    // 并发护栏：同一 story 不允许并行 run
    let pool_guard = pool.clone();
    let sid_guard = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard).has_running_run_for_story(&sid_guard)
    }).await.map_err(|e| AppError::from(format!("guard join error: {}", e)))?
        .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed("该故事已有进行中的创作任务", None::<String>));
    }
    // 下一章号
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let chapter_number = tokio::task::spawn_blocking(move || -> Result<i32, AppError> {
        let conn = pool2.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
        conn.query_row("SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM scenes WHERE story_id = ?1",
            rusqlite::params![sid2], |r| r.get(0)).map_err(AppError::from)
    }).await.map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator.run_continue(&rid, &story_id, chapter_number).await {
            log::error!("agency continue run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}
```

`handlers.rs` agency 分组追加 `agency::commands::agency_continue_chapter,`。

`agency_producer_system.md` 在"工作方式"追加一条：

```markdown
- 资产条目格式约定：character 条目的 content 必须是 JSON（{"name":"真名","background":"背景","personality":"性格","goals":"欲望/目标"}）；world 与 outline 条目的 content 为纯文本。
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 7+）及全量；`cd src-frontend && npx vitest run 2>&1 | tail -3`。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/ src-tauri/src/handlers.rs resources/prompts/agency/agency_producer_system.md
git commit -m "feat(agency): continue loop with asset materialization + asset_query tool"
```

---

### Task 5: 并发预算 AgencyBudget + BudgetedLlm

**Files:**
- Create: `src-tauri/src/agency/budget.rs`
- Modify: `src-tauri/src/agency/tool_loop.rs`（`LoopLlm::complete_metered` 默认方法）
- Modify: `src-tauri/src/agency/coordinator.rs`（AgencyLlm 覆盖 complete_metered；run_role_with_llm 包装 BudgetedLlm；run 级预算注入）
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod budget;`）

**Interfaces:**
- Consumes: Task 1 的 `AgencyLlm`（run_id 注册）；`GenerateResponse{content, tokens_used, cost}`。
- Produces: `AgencyBudget::{new, with_role_permits, check, record_usage, acquire, tokens_used}`；`BudgetedLlm::new(inner, budget, role)`（impl LoopLlm）；`LoopLlm::complete_metered`（默认 `(s, 0, 0.0)`）。Task 6 并行循环依赖。

- [ ] **Step 1: 写失败的测试**

`budget.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::AgentRole;
    use crate::agency::tool_loop::LoopLlm;
    use crate::error::AppError;
    use crate::router::TaskType;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct MeteredMock {
        tokens: i32,
        delay_ms: u64,
        calls: Mutex<Vec<std::time::Instant>>,
    }

    #[async_trait::async_trait]
    impl LoopLlm for MeteredMock {
        async fn complete(&self, _s: &str, _u: &str, _t: TaskType, _m: i32) -> Result<String, AppError> {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            self.calls.lock().unwrap().push(std::time::Instant::now());
            Ok("输出".to_string())
        }
        async fn complete_metered(&self, s: &str, u: &str, t: TaskType, m: i32)
            -> Result<(String, i32, f64), AppError> {
            self.complete(s, u, t, m).await.map(|out| (out, self.tokens, 0.01))
        }
    }

    #[tokio::test]
    async fn test_budget_exhaustion_blocks_call() {
        let budget = Arc::new(AgencyBudget::new(100));
        let llm = Arc::new(MeteredMock { tokens: 100, delay_ms: 0, calls: Mutex::new(vec![]) });
        let limited = BudgetedLlm::new(llm, budget.clone(), AgentRole::LeadWriter);
        // 第一次：100 tokens 入账成功
        limited.complete("s", "u", TaskType::CreativeWriting, 100).await.unwrap();
        assert_eq!(budget.tokens_used(), 100);
        // 第二次：已达预算上限 → Err
        let err = limited.complete("s", "u", TaskType::CreativeWriting, 100).await.unwrap_err();
        assert!(err.to_string().contains("预算"));
    }

    #[tokio::test]
    async fn test_role_permits_serialize_same_role() {
        let budget = Arc::new(AgencyBudget::with_role_permits(1, 1, 1, 1_000_000));
        let mock = Arc::new(MeteredMock { tokens: 1, delay_ms: 80, calls: Mutex::new(vec![]) });
        let l1 = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let l2 = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let start = std::time::Instant::now();
        let (r1, r2) = tokio::join!(
            l1.complete("s", "u", TaskType::CreativeWriting, 100),
            l2.complete("s", "u", TaskType::CreativeWriting, 100),
        );
        let elapsed = start.elapsed();
        assert!(r1.is_ok() && r2.is_ok());
        assert!(elapsed >= std::time::Duration::from_millis(150),
            "同角色两次调用应串行（≥160ms），实际 {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_different_roles_run_parallel() {
        let budget = Arc::new(AgencyBudget::with_role_permits(1, 1, 1, 1_000_000));
        let mock = Arc::new(MeteredMock { tokens: 1, delay_ms: 80, calls: Mutex::new(vec![]) });
        let writer = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let editor = BudgetedLlm::new(mock, budget, AgentRole::EditorAuditor);
        let start = std::time::Instant::now();
        let (r1, r2) = tokio::join!(
            writer.complete("s", "u", TaskType::CreativeWriting, 100),
            editor.complete("s", "u", TaskType::Proofreading, 100),
        );
        let elapsed = start.elapsed();
        assert!(r1.is_ok() && r2.is_ok());
        assert!(elapsed < std::time::Duration::from_millis(150),
            "不同角色应并行（<150ms），实际 {:?}", elapsed);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::budget 2>&1 | tail -3`
Expected: FAIL（`AgencyBudget`/`BudgetedLlm` 未定义）

- [ ] **Step 3: 实现**

`tool_loop.rs` trait 扩展（不破坏既有实现）：

```rust
#[async_trait::async_trait]
pub trait LoopLlm: Send + Sync {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError>;

    /// 带计量的完成：返回 (content, tokens_used, cost)。默认实现不计费（mock/测试用）。
    async fn complete_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        self.complete(system_prompt, user_prompt, task, max_tokens)
            .await
            .map(|s| (s, 0, 0.0))
    }
}
```

`src-tauri/src/agency/budget.rs`：

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{Semaphore, SemaphorePermit};

use crate::agency::models::AgentRole;
use crate::agency::tool_loop::LoopLlm;
use crate::error::AppError;
use crate::router::TaskType;

pub const DEFAULT_RUN_TOKEN_BUDGET: u64 = 300_000;

/// 运行级并发预算：按角色信号量限流 + token 预算硬上限。
/// 取代对全局单一 BACKGROUND_LLM_SEMAPHORE 的依赖（agency 内部）。
pub struct AgencyBudget {
    writer_sem: Semaphore,
    producer_sem: Semaphore,
    editor_sem: Semaphore,
    token_budget: u64,
    tokens_used: AtomicU64,
}

impl AgencyBudget {
    pub fn new(token_budget: u64) -> Self {
        Self::with_role_permits(1, 1, 1, token_budget)
    }

    pub fn with_role_permits(writer: usize, producer: usize, editor: usize, token_budget: u64) -> Self {
        Self {
            writer_sem: Semaphore::new(writer),
            producer_sem: Semaphore::new(producer),
            editor_sem: Semaphore::new(editor),
            token_budget,
            tokens_used: AtomicU64::new(0),
        }
    }

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<(), AppError> {
        if self.tokens_used() >= self.token_budget {
            return Err(AppError::from(format!(
                "运行 token 预算耗尽（{}/{}），已熔断",
                self.tokens_used(), self.token_budget
            )));
        }
        Ok(())
    }

    pub fn record_usage(&self, tokens: i32) {
        if tokens > 0 {
            self.tokens_used.fetch_add(tokens as u64, Ordering::SeqCst);
        }
    }

    pub async fn acquire(&self, role: AgentRole) -> Result<SemaphorePermit<'_>, AppError> {
        self.check()?;
        let sem = match role {
            AgentRole::LeadWriter => &self.writer_sem,
            AgentRole::Producer => &self.producer_sem,
            AgentRole::EditorAuditor => &self.editor_sem,
        };
        sem.acquire().await.map_err(|_| AppError::from("预算信号量已关闭"))
    }
}

/// 预算包装层：角色限流 + token 记账，对 ToolLoop 透明。
pub struct BudgetedLlm {
    inner: Arc<dyn LoopLlm>,
    budget: Arc<AgencyBudget>,
    role: AgentRole,
}

impl BudgetedLlm {
    pub fn new(inner: Arc<dyn LoopLlm>, budget: Arc<AgencyBudget>, role: AgentRole) -> Self {
        Self { inner, budget, role }
    }
}

#[async_trait::async_trait]
impl LoopLlm for BudgetedLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        let (content, _t, _c) = self.complete_metered(system_prompt, user_prompt, task, max_tokens).await?;
        Ok(content)
    }

    async fn complete_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        let _permit = self.budget.acquire(self.role).await?;
        let (content, tokens, cost) = self.inner
            .complete_metered(system_prompt, user_prompt, task, max_tokens)
            .await?;
        self.budget.record_usage(tokens);
        Ok((content, tokens, cost))
    }
}
```

`coordinator.rs`：
- `AgencyLlm` 覆盖 `complete_metered`（真实计量）：

```rust
async fn complete_metered(
    &self,
    system_prompt: &str,
    user_prompt: &str,
    task: TaskType,
    max_tokens: i32,
) -> Result<(String, i32, f64), AppError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    register_request(&self.run_id, &request_id);
    let routing = crate::router::RoutingRequest { task, ..Default::default() };
    let (_rid, result) = self.llm
        .generate_for_request_with_request_id(
            routing, user_prompt.to_string(), Some(max_tokens), None,
            Some("agency"), Some(request_id.clone()), None, None, None, None, None, None, None,
            Some(system_prompt.to_string()), None,
        )
        .await;
    unregister_request(&self.run_id, &request_id);
    result.map(|r| (r.content, r.tokens_used, r.cost))
}
```
（`complete` 改为委托 `complete_metered` 取首元素。）

- run 级预算注入：`run_genesis_inner`/`run_continue_inner` 开头：
```rust
let budget = std::sync::Arc::new(crate::agency::budget::AgencyBudget::new(
    crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
));
```
- `run_role_with_llm` 内包装：
```rust
let budgeted: Arc<dyn LoopLlm> = Arc::new(crate::agency::budget::BudgetedLlm::new(
    llm.clone(), self.budget_for_run(), role,
));
ToolLoop::new(budgeted, registry.clone())
```
预算需在 run 生命周期共享：`run_genesis_inner`/`run_continue_inner` 把 `Arc<AgencyBudget>` 传入 `run_role_with_llm`（签名加 `budget: &Arc<AgencyBudget>` 参数；`evaluate_gate` 透传）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 3）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/budget.rs src-tauri/src/agency/tool_loop.rs src-tauri/src/agency/coordinator.rs src-tauri/src/agency/mod.rs
git commit -m "feat(agency): per-role concurrency budget + token metering wrapper"
```

---

### Task 6: 并行稳态循环（gate(n-1) ∥ writer(n)）+ 代理活动事件 + 总线接线

**Files:**
- Modify: `src-tauri/src/agency/coordinator.rs`（重构提取 ensure_assets/write_chapter/handle_gate/gate_runner + run_continue_batch + ProgressSink + agency-agent-activity 事件）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_continue_batch`）
- Modify: `src-tauri/src/handlers.rs`（注册）

**Interfaces:**
- Consumes: Task 3/4/5 全部。
- Produces: `AgencyCoordinator::run_continue_batch(run_id, story_id, start_chapter, count) -> Result<AgencyBatchResult, AppError>`；`AgencyBatchResult { run_id, story_id, chapters: Vec<AgencyContinueResult> }`；`ProgressSink = Arc<dyn Fn(&str, &str, &str) + Send + Sync>`（Task 7 用）；`run_genesis_with_sink(run_id, premise, sink: Option<ProgressSink>)`；事件 `agency-agent-activity`（payload `{run_id, role, action: "start"|"done", detail}`）；`AgencyCoordinator::next_chapter_number(pool, story_id)`；IPC `agency_continue_batch(story_id, count) -> String`。

**并行模型（文字图）：**
```
assets(一次) → writer(1) → ┌ gate(1) ┐ → 装配1 → ┌ gate(2) ┐ → 装配2 → …
                            ∥ writer(2) ─────────┘ ∥ writer(3) ────────┘
```
每章 gate 作为 `tokio::spawn` 的 'static 任务与下一章 writer 并发（`tokio::join!` 汇合）；修订在本章 handle_gate 内串行处理；同一 run 共享 AgencyBudget（同角色自然串行，跨角色并行）。

- [ ] **Step 1: 写失败的测试**

`coordinator.rs` 测试模块追加（并发可证 mock）：

```rust
/// 按系统提示词路由的 mock：区分 主创/编辑/管理 三队列，且记录调用时间窗用于并发断言。
struct RoutingMock {
    writer: Mutex<VecDeque<String>>,
    editor: Mutex<VecDeque<String>>,
    producer: Mutex<VecDeque<String>>,
    intervals: Mutex<Vec<(String, std::time::Instant, std::time::Instant)>>,
    delay_ms: u64,
}

impl RoutingMock {
    fn new(delay_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            writer: Mutex::new(VecDeque::new()),
            editor: Mutex::new(VecDeque::new()),
            producer: Mutex::new(VecDeque::new()),
            intervals: Mutex::new(VecDeque::new()),
            delay_ms,
        })
    }
    fn push(&self, role: &str, lines: Vec<&str>) {
        let q = match role {
            "writer" => &self.writer,
            "editor" => &self.editor,
            _ => &self.producer,
        };
        q.lock().unwrap().extend(lines.into_iter().map(String::from));
    }
}

#[async_trait::async_trait]
impl LoopLlm for RoutingMock {
    async fn complete(&self, system: &str, _u: &str, _t: crate::router::TaskType, _m: i32) -> Result<String, AppError> {
        let role = if system.contains("编辑") { "editor" }
            else if system.contains("主创") { "writer" }
            else { "producer" };
        let start = std::time::Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        let out = {
            let q = match role {
                "editor" => &self.editor,
                "writer" => &self.writer,
                _ => &self.producer,
            };
            q.lock().unwrap().pop_front()
                .ok_or_else(|| AppError::validation_failed(format!("mock[{}] exhausted", role), None::<String>))?
        };
        self.intervals.lock().unwrap().push((role.to_string(), start, std::time::Instant::now()));
        Ok(out)
    }
}

fn seed_story_with_assets(pool: &crate::db::DbPool) -> String {
    let story = crate::db::repositories::StoryRepository::new(pool.clone()).create(crate::db::dto::CreateStoryRequest {
        title: "并行书".into(), description: Some("前提".into()), genre: None,
        style_dna_id: None, genre_profile_id: None, methodology_id: None, reference_book_id: None,
    }).unwrap();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
         VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
        rusqlite::params![story.id],
    ).unwrap();
    story.id
}

#[tokio::test]
async fn test_batch_parallel_two_chapters() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(60);
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"第一章正文。","summary":"一"}}"#,
        r#"{"type":"final","content":"第一章完成"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第二章","content":"第二章正文。","summary":"二"}}"#,
        r#"{"type":"final","content":"第二章完成"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好1\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好2\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock.clone());
    let result = coordinator.run_continue_batch("rb-1", &story_id, 1, 2).await.unwrap();
    assert_eq!(result.chapters.len(), 2);
    // 两章场景均落库
    let scenes = crate::db::repositories::SceneRepository::new(pool.clone()).get_by_story(&story_id).unwrap();
    assert_eq!(scenes.len(), 2);
    // 并发证据：gate1(editor) 与 writer2 的时间窗存在交叠
    let intervals = mock.intervals.lock().unwrap();
    let editor_first = intervals.iter().find(|(r, _, _)| r == "editor").unwrap();
    let writer_windows: Vec<_> = intervals.iter().filter(|(r, _, _)| r == "writer").collect();
    let overlapped = writer_windows.iter().any(|(_, s, e)| *s < editor_first.2 && editor_first.1 < *e);
    assert!(overlapped, "gate(1) 应与 writer(2) 并发: {:?}", *intervals);
    let run = AgencyRepository::new(pool.clone()).get_run("rb-1").unwrap().unwrap();
    assert_eq!(run.status, "completed");
}

#[tokio::test]
async fn test_batch_revision_sends_bus_proposal() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(0);
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"初稿。","summary":"一"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        // 修订（board_revise，item_id 动态——用 DynamicMock 才不依赖 id；此处用"读草稿再修订"两步行不行？
        // 简化：修订轮让 writer 直接 board_revise 会需要 item_id。本用例验证总线消息，
        // 修订指令中的 item_id 由协调器注入任务文本，mock 无法预知。
        // 因此修订轮脚本用 board_read + final 组合，协调器在 revise_out 非 aborted 后
        // 仍读 latest_draft——draft 未变也无妨，本用例只断言 bus 消息与放行。
        r#"{"type":"final","content":"已知晓修订意见"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"动机弱\"],\"suggestions\":[],\"comments\":\"修\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"过\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let result = coordinator.run_continue_batch("rb-2", &story_id, 1, 1).await.unwrap();
    assert_eq!(result.chapters.len(), 1);
    assert!(result.chapters[0].revised);
    // 总线：editor→writer 的 proposal 消息存在
    let bus = crate::agency::bus::MessageBus::new(pool.clone());
    let inbox = bus.inbox("rb-2", AgentRole::LeadWriter).unwrap();
    assert!(inbox.iter().any(|m| m.msg_type == "proposal" && m.payload.contains("动机弱")));
}
```

注：`test_batch_parallel_two_chapters` 的 mock 队列总数：writer 4 条（两章各 2）+ editor 2 条。修订路径（`rb-2`）中 writer 第 3 条响应对应修订轮——协调器修订轮调用 writer 一次（final 即返回，draft 未变，第二轮 gate pass 放行；这是可接受的 mock 简化，真实模型会真的 revise——board_revise 语义已由 Task 2 测试覆盖）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（`run_continue_batch` 未定义）

- [ ] **Step 3: 实现**

`coordinator.rs` 重构与新增（要点代码）：

```rust
pub const EVENT_AGENT_ACTIVITY: &str = "agency-agent-activity";
pub type ProgressSink = std::sync::Arc<dyn Fn(&str, &str, &str) + Send + Sync>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgencyBatchResult {
    pub run_id: String,
    pub story_id: String,
    pub chapters: Vec<AgencyContinueResult>,
}

impl AgencyCoordinator {
    /// sink 版创世（Task 7 smart_execute 用）；默认走 run_genesis（sink=None）。
    pub async fn run_genesis_with_sink(
        &self,
        run_id: &str,
        premise: &str,
        sink: Option<ProgressSink>,
    ) -> Result<AgencyGenesisResult, AppError> {
        *self.progress_sink.borrow_mut() = sink;
        self.run_genesis(run_id, premise).await
    }

    fn emit_activity(&self, run_id: &str, role: AgentRole, action: &str, detail: &str) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(EVENT_AGENT_ACTIVITY, serde_json::json!({
                "run_id": run_id,
                "role": role.as_str(),
                "action": action,
                "detail": detail,
            }));
        }
    }

    pub fn next_chapter_number(pool: &DbPool, story_id: &str) -> Result<i32, AppError> {
        let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
        conn.query_row(
            "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM scenes WHERE story_id = ?1",
            rusqlite::params![story_id],
            |r| r.get(0),
        ).map_err(AppError::from)
    }

    /// 并行稳态循环：gate(n-1) 与 writer(n) 并发。
    pub async fn run_continue_batch(
        &self,
        run_id: &str,
        story_id: &str,
        start_chapter: i32,
        count: usize,
    ) -> Result<AgencyBatchResult, AppError> {
        let repo = AgencyRepository::new(self.pool.clone());
        let cancel = register_agency_cancel(run_id);
        let result = self.run_batch_inner(run_id, story_id, start_chapter, count, &repo, &cancel).await;
        unregister_agency_cancel(run_id);
        match &result {
            Ok(r) => {
                let json = serde_json::to_string(r).unwrap_or_default();
                let _ = repo.finish_run(run_id, "completed", Some(&json), None);
                self.emit_progress(run_id, "assembly", "completed", "批量续写完成");
            }
            Err(e) => {
                let status = if cancel.load(Ordering::SeqCst) { "cancelled" } else { "failed" };
                let _ = repo.finish_run(run_id, status, None, Some(&e.to_string()));
                self.emit_progress(run_id, "assembly", status, &e.to_string());
            }
        }
        result
    }

    async fn run_batch_inner(
        &self,
        run_id: &str,
        story_id: &str,
        start_chapter: i32,
        count: usize,
        repo: &AgencyRepository,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AgencyBatchResult, AppError> {
        let llm = self.llm_for_run(run_id);
        let budget = std::sync::Arc::new(crate::agency::budget::AgencyBudget::new(
            crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
        ));
        let title = self.story_title(story_id).await.unwrap_or_else(|| "未命名".to_string());
        let premise = format!("续写《{}》第{}章起", title, start_chapter);
        repo.create_run(&AgencyRun::new(run_id, &premise)).map_err(AppError::from)?;
        repo.set_run_story(run_id, story_id).map_err(AppError::from)?;
        repo.update_run_phase(run_id, "running", "assets").map_err(AppError::from)?;

        // 资产确认/补齐（复用 Task 4 ensure_assets）
        self.ensure_assets(&llm, &budget, repo, run_id, story_id, &premise).await?;
        self.check_cancel(cancel)?;

        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let mut chapters: Vec<AgencyContinueResult> = Vec::new();
        let mut pending_gate: Option<tokio::task::JoinHandle<Result<GateOutcome, AppError>>> = None;
        let mut pending_chapter: Option<(i32, BoardItem, bool)> = None; // (章号, 草稿, 是否已修订过)

        for offset in 0..count {
            let chapter_number = start_chapter + offset as i32;
            self.check_cancel(cancel)?;
            repo.update_run_phase(run_id, "running", "writing").map_err(AppError::from)?;
            self.emit_activity(run_id, AgentRole::LeadWriter, "start", &format!("第{}章", chapter_number));

            let write_fut = self.write_chapter(&llm, &budget, &board, &registry, run_id, story_id, &premise, chapter_number);
            let draft = match pending_gate.take() {
                Some(jh) => {
                    // gate(n-1) 与 writer(n) 并发
                    let (gate_res, write_res) = tokio::join!(jh, write_fut);
                    let outcome = gate_res.map_err(|e| AppError::from(format!("gate join error: {}", e)))??;
                    let draft = write_res?;
                    self.emit_activity(run_id, AgentRole::LeadWriter, "done", &format!("第{}章草稿", chapter_number));
                    let (prev_num, prev_draft, prev_revised) = pending_chapter.take().unwrap();
                    let prev = self.handle_gate(
                        &llm, &budget, &board, &registry, repo, run_id, story_id, &premise,
                        prev_num, prev_draft, prev_revised, outcome, cancel,
                    ).await?;
                    chapters.push(prev);
                    draft
                }
                None => {
                    let draft = write_fut.await?;
                    self.emit_activity(run_id, AgentRole::LeadWriter, "done", &format!("第{}章草稿", chapter_number));
                    draft
                }
            };

            // spawn gate(n)（'static，与下一轮 writer 并发）
            let runner = self.gate_runner(&llm, &budget, &board, &registry);
            let (rid, sid, prem, d) = (run_id.to_string(), story_id.to_string(), premise.clone(), draft.clone());
            self.emit_activity(run_id, AgentRole::EditorAuditor, "start", &format!("审查第{}章", chapter_number));
            pending_gate = Some(tokio::spawn(async move { runner.evaluate(rid, sid, prem, d).await }));
            pending_chapter = Some((chapter_number, draft, false));
        }

        // 收尾：最后一章 gate
        if let (Some(jh), Some((num, draft, revised))) = (pending_gate.take(), pending_chapter.take()) {
            let outcome = jh.await.map_err(|e| AppError::from(format!("gate join error: {}", e)))??;
            let last = self.handle_gate(
                &llm, &budget, &board, &registry, repo, run_id, story_id, &premise,
                num, draft, revised, outcome, cancel,
            ).await?;
            chapters.push(last);
        }

        Ok(AgencyBatchResult { run_id: run_id.to_string(), story_id: story_id.to_string(), chapters })
    }
}
```

**重构提取**（`run_continue_inner` 同步改为调用这三个 helper，行为不变）：

```rust
impl AgencyCoordinator {
    /// 资产确认/补齐（Task 4 run_continue_inner 第 1 步提取）
    async fn ensure_assets(&self, llm: &Arc<dyn LoopLlm>, budget: &Arc<crate::agency::budget::AgencyBudget>,
        repo: &AgencyRepository, run_id: &str, story_id: &str, premise: &str) -> Result<(), AppError> {
        // ……Task 4 run_continue_inner 的资产段落原样搬入（含 character_count 检查、历史落库、producer 补齐、materialize）
    }

    /// 写一章草稿（Task 4 第 2 步提取）
    async fn write_chapter(&self, llm: &Arc<dyn LoopLlm>, budget: &Arc<crate::agency::budget::AgencyBudget>,
        board: &BlackboardService, registry: &Arc<ToolRegistry>,
        run_id: &str, story_id: &str, premise: &str, chapter_number: i32) -> Result<BoardItem, AppError> {
        let key = format!("第{}章", chapter_number);
        let writer_out = self.run_role_with_llm_and_budget(
            llm, budget, AgentRole::LeadWriter, board, registry, run_id, story_id, premise,
            &format!("续写{}（1500-2500 字）。先 board_read 读资产区、asset_query(kind=scenes) 读最近场景保持连贯，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key={}）。", key, key),
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，本章未完成"));
        }
        self.latest_draft(board, run_id)
    }

    /// 单章 gate 结果处理：修订（≤1 轮，总线记录 proposal）→ 装配 Scene。
    /// 返回该章的 AgencyContinueResult。
    async fn handle_gate(&self, llm: &Arc<dyn LoopLlm>, budget: &Arc<crate::agency::budget::AgencyBudget>,
        board: &BlackboardService, registry: &Arc<ToolRegistry>, repo: &AgencyRepository,
        run_id: &str, story_id: &str, premise: &str,
        chapter_number: i32, draft: BoardItem, mut revised: bool, outcome: GateOutcome,
        cancel: &Arc<AtomicBool>) -> Result<AgencyContinueResult, AppError> {
        let mut draft = draft;
        let final_verdict = match outcome {
            GateOutcome::Passed { verdict } => verdict,
            GateOutcome::RevisionRequired { issues, verdict } if !revised => {
                revised = true;
                // 总线：修订提案（P5 时间线/学习中心数据源）
                let bus = crate::agency::bus::MessageBus::new(self.pool.clone());
                let _ = bus.send(run_id, AgentRole::EditorAuditor, AgentRole::LeadWriter,
                    "proposal", serde_json::json!({"chapter": chapter_number, "issues": issues}));
                repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
                let task = Self::build_revision_task(&draft, &issues);
                let revise_out = self.run_role_with_llm_and_budget(
                    llm, budget, AgentRole::LeadWriter, board, registry, run_id, story_id, premise, &task,
                ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                if revise_out.aborted {
                    return Err(AppError::from("主创 Agent 修订轮被熔断"));
                }
                draft = self.latest_draft(board, run_id)?;
                self.check_cancel(cancel)?;
                let second = self.evaluate_gate(llm, board, registry, run_id, story_id, premise, &draft).await?;
                match second {
                    GateOutcome::Passed { verdict } => verdict,
                    GateOutcome::RevisionRequired { verdict, .. } => verdict,
                    GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
                }
            }
            GateOutcome::RevisionRequired { verdict, .. } => verdict,
            GateOutcome::Failed { reason } => return Err(AppError::from(format!("质量门未通过: {}", reason))),
        };
        // 装配
        let pool = self.pool.clone();
        let sid = story_id.to_string();
        let content = draft.content.clone();
        let title_c = format!("第{}章", chapter_number);
        let scene = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
            let repo = crate::db::repositories::SceneRepository::new(pool);
            let scene = repo.create(&sid, chapter_number, Some(&title_c)).map_err(AppError::from)?;
            repo.update(&scene.id, &crate::db::repositories::SceneUpdate {
                content: Some(content),
                ..Default::default()
            }).map_err(AppError::from)?;
            Ok(scene)
        }).await.map_err(|e| AppError::from(format!("scene assembly join error: {}", e)))??;
        Ok(AgencyContinueResult {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            scene_id: scene.id,
            chapter_number,
            revised,
            verdict: final_verdict,
        })
    }

    /// 'static gate 执行器（spawn 用，全部依赖按值持有）。
    fn gate_runner(&self, llm: &Arc<dyn LoopLlm>, budget: &Arc<crate::agency::budget::AgencyBudget>,
        board: &BlackboardService, registry: &Arc<ToolRegistry>) -> GateRunner {
        GateRunner {
            llm: llm.clone(),
            budget: budget.clone(),
            board: board.clone(),
            registry: registry.clone(),
            pool: self.pool.clone(),
        }
    }
}

/// 见 gate_runner。
pub struct GateRunner {
    llm: Arc<dyn LoopLlm>,
    budget: Arc<crate::agency::budget::AgencyBudget>,
    board: BlackboardService,
    registry: Arc<ToolRegistry>,
    pool: DbPool,
}

impl GateRunner {
    pub async fn evaluate(self, run_id: String, story_id: String, premise: String, draft: BoardItem) -> Result<GateOutcome, AppError> {
        // 与 evaluate_gate 同逻辑，但经自由函数 run_role_loop 驱动 editor 角色
        // （evaluate_gate 本体同步改为委托 GateRunner，避免两份逻辑）
        evaluate_gate_impl(&self.llm, &self.budget, &self.pool, &self.board, &self.registry,
            &run_id, &story_id, &premise, &draft).await
    }
}
```

配套：`evaluate_gate` 的 editor 调用改为经自由函数 `run_role_loop(llm, budget, pool, board, registry, role, run_id, story_id, premise, task)`（从 `run_role_with_llm_and_budget` 提取的纯依赖版本，含 spec/resolve prompt/ToolContext/BudgetedLlm/ToolLoop）；`AgencyCoordinator::evaluate_gate` 委托 `evaluate_gate_impl`；`run_role_with_llm` 更名为 `run_role_with_llm_and_budget`（加 budget 参）并委托 `run_role_loop`。`progress_sink` 字段：`progress_sink: std::cell::RefCell<Option<ProgressSink>>`（for_test/new 初始 None），`emit_progress` 末尾调 sink（`RefCell` 够用——协调器单线程驱动；spawn 的 GateRunner 不持 sink）。若 RefCell 在 async 间借用心智负担大，改用 `std::sync::Mutex<Option<ProgressSink>>`。

`commands.rs` 追加：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_batch(
    story_id: String,
    count: Option<u32>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let count = (count.unwrap_or(3) as usize).clamp(1, 5);
    let pool = pool.inner().clone();
    let pool_guard = pool.clone();
    let sid = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard).has_running_run_for_story(&sid)
    }).await.map_err(|e| AppError::from(format!("guard join error: {}", e)))?
        .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed("该故事已有进行中的创作任务", None::<String>));
    }
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let start_chapter = tokio::task::spawn_blocking(move || {
        AgencyCoordinator::next_chapter_number(&pool2, &sid2)
    }).await.map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator.run_continue_batch(&rid, &story_id, start_chapter, count).await {
            log::error!("agency batch run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}
```

`handlers.rs` agency 分组追加 `agency::commands::agency_continue_batch,`。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 2）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs src-tauri/src/agency/commands.rs src-tauri/src/handlers.rs
git commit -m "feat(agency): parallel steady-state loop (gate(n-1) || writer(n)) + activity events + bus wiring"
```

---

### Task 7: smart_execute 创世分支切换到 agency

**Files:**
- Modify: `src-tauri/src/commands/orchestrator.rs`（创世分支 :243-633 替换为 agency 路径，提取 `run_agency_bootstrap`）
- Modify: `src-tauri/src/agency/coordinator.rs`（`build_bootstrap_result` 纯函数，供测试与分支共用）

**Interfaces:**
- Consumes: Task 6 的 `run_genesis_with_sink`/`ProgressSink`；`crate::planner::{PlanExecutionResult, SmartExecuteProgress}`；`crate::state_sync::service::StateSync::emit_story_created`（现有 :377 调用方式）；`record_ai_operation`（现有 :561-586 代码搬移）；超时读取（`config/settings.rs:333 smart_execute_total_timeout_secs`，默认 600，现有 :46-48 读取代码）。
- Produces: smart_execute 创世分支 agency 化，返回形状严格满足 Global Constraints 的前端兼容契约；`AgencyCoordinator::build_bootstrap_result(result: &AgencyGenesisResult, scene_content: String, run_id: &str) -> crate::planner::PlanExecutionResult`。

**前端兼容契约（逐字，FrontstageApp.tsx:3036-3093/3907-3915 决定）：** `success: true`、`final_content` = 完整第一章正文（非摘要文案）、`messages = ["story_created:{story_id}", "session_id:{run_id}", "novel_bootstrap_first_chapter_ready"]`。事件镜像 `smart-execute-progress`（stage=agency phase、step_number 映射 concept=1/assets=2/writing=3/review=4/revision=4/assembly=5、total_steps=6、完成时 stage="completed" step=6）。`novel-bootstrap-progress` 不再发送（其前端硬编码 total_steps===2/6 已与旧实现脱节，属遗留监听；`smart-execute-progress` 为维护中的通道，FrontstageApp.tsx:1891 与 useBackendActivityListener.ts:359 均消费它）。

- [ ] **Step 1: 写失败的测试**

`coordinator.rs` 测试模块追加（契约纯函数）：

```rust
#[test]
fn test_build_bootstrap_result_contract() {
    let result = AgencyGenesisResult {
        run_id: "r1".into(),
        story_id: "story-9".into(),
        scene_id: "scene-3".into(),
        revised: false,
        verdict: EditorVerdict { verdict: "pass".into(), blocking_issues: vec![], suggestions: vec![], comments: "好".into() },
        chapter_chars: 2000,
    };
    let out = AgencyCoordinator::build_bootstrap_result(&result, "完整第一章正文……".to_string(), "r1");
    assert!(out.success);
    assert_eq!(out.steps_completed, 1);
    assert_eq!(out.final_content.as_deref(), Some("完整第一章正文……"));
    assert_eq!(out.messages, vec![
        "story_created:story-9".to_string(),
        "session_id:r1".to_string(),
        "novel_bootstrap_first_chapter_ready".to_string(),
    ]);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（`build_bootstrap_result` 未定义）

- [ ] **Step 3: 实现**

`coordinator.rs` 追加：

```rust
impl AgencyCoordinator {
    /// smart_execute 创世分支的返回形状（前端兼容契约，见 P2 计划 Global Constraints）。
    pub fn build_bootstrap_result(
        result: &AgencyGenesisResult,
        scene_content: String,
        run_id: &str,
    ) -> crate::planner::PlanExecutionResult {
        crate::planner::PlanExecutionResult {
            success: true,
            steps_completed: 1,
            final_content: Some(scene_content),
            messages: vec![
                format!("story_created:{}", result.story_id),
                format!("session_id:{}", run_id),
                "novel_bootstrap_first_chapter_ready".to_string(),
            ],
            error: None,
        }
    }
}
```

`commands/orchestrator.rs`：创世分支（`if is_bootstrap_intent { ... }`，:243-633）整体替换为：

```rust
if is_bootstrap_intent {
    emit_progress("analyzing", "创世 2.0 启动（多代理）", 0, 6);
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = crate::agency::coordinator::AgencyCoordinator::new(app_handle.clone(), pool.inner().clone());
    // 进度镜像：agency phase → smart-execute-progress
    let sink: crate::agency::coordinator::ProgressSink = std::sync::Arc::new({
        let app = app_handle.clone();
        move |phase: &str, status: &str, message: &str| {
            let step = match phase {
                "concept" => 1, "assets" => 2, "writing" => 3,
                "review" | "revision" => 4, "assembly" => 5, _ => 6,
            };
            let _ = app.emit("smart-execute-progress", crate::planner::SmartExecuteProgress {
                stage: if status == "running" { phase.to_string() } else { status.to_string() },
                message: message.to_string(),
                step_number: step,
                total_steps: 6,
            });
        }
    });
    let genesis_future = coordinator.run_genesis_with_sink(&run_id, &user_input, Some(sink));
    match tokio::time::timeout(std::time::Duration::from_secs(total_timeout), genesis_future).await {
        Ok(Ok(result)) => {
            // 取场景正文（final_content 契约）
            let pool_c = pool.inner().clone();
            let scene_id = result.scene_id.clone();
            let content = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                let scene = crate::db::repositories::SceneRepository::new(pool_c)
                    .get_by_id(&scene_id)
                    .map_err(AppError::from)?
                    .ok_or_else(|| AppError::from("装配场景不存在"))?;
                Ok(scene.content.unwrap_or_default())
            }).await.map_err(|e| AppError::from(format!("scene read join error: {}", e)))??;
            // 与旧路径一致的通知与运营记录
            crate::state_sync::service::StateSync::emit_story_created(&app_handle, &result.story_id, "新故事");
            // record_ai_operation（沿用原 :561-586 代码，operation_type="bootstrap"，details 记 run_id/story_id）
            // ……（原代码搬移，参数按现有签名填充）
            emit_progress("completed", "小说创世完成", 6, 6);
            return Ok(crate::agency::coordinator::AgencyCoordinator::build_bootstrap_result(
                &result, content, &run_id,
            ));
        }
        Ok(Err(e)) => {
            emit_progress("error", &format!("创世失败: {}", e), 6, 6);
            return Err(e);
        }
        Err(_) => {
            let llm = crate::llm::LlmService::new(app_handle.clone());
            crate::agency::coordinator::cancel_requests_for_run(&llm, &run_id);
            emit_progress("timeout", "创世超时", 6, 6);
            return Err(AppError::llm_timeout((total_timeout * 1000) as i64));
        }
    }
}
```

注意：
- `total_timeout` 的读取沿用函数顶部现有代码（:46-48，`config.smart_execute_total_timeout_secs`，默认 600）；旧分支 :253 的 180 回退随删除消失。
- `emit_story_created` 与 `record_ai_operation` 的**精确调用签名**以现有 :377 与 :561-586 代码为准原样搬移，不要凭记忆改写；`AppError::llm_timeout` 的用法以 :618-629 现有映射为准。
- 分支内不再引用 `GenesisContext/GenesisPipeline/GenesisRunRepository/NarrativePipelineExecutor/register_pipeline_cancel`（import 清理在 Task 8 统一做——本任务允许暂时保留 unused import 警告？**不允许**（全局约束：无新警告），本任务即清理创世分支相关的不再使用的 import，但 genesis.rs 文件与其他引用点在 Task 8 处理）。
- `emit_progress` 闭包（:104-115）保持不变并复用。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3` 全量 + `cd src-frontend && npx vitest run 2>&1 | tail -3`。
Expected: PASS（前端零改动）、后端无新警告。

手动冒烟（可选，真机）：`cargo tauri dev` → 幕前输入"写一部……"→ 首章自动投递、进度条推进（走 smart-execute-progress）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/orchestrator.rs src-tauri/src/agency/coordinator.rs
git commit -m "feat(agency): route smart_execute genesis branch through agency framework"
```

---

### Task 8: 删除旧创世路径

**Files:**
- Create: `src-tauri/src/agents/trim_utils.rs`（3 个工具函数迁入）
- Modify: `src-tauri/src/agents/mod.rs`（`pub(crate) mod trim_utils;`）
- Modify: `src-tauri/src/agents/orchestrator.rs`（:1832/:1834/:1904/:1909 引用改 `crate::agents::trim_utils::`）
- Delete: `src-tauri/src/narrative/genesis.rs`
- Modify: `src-tauri/src/narrative/mod.rs`（删 `pub mod genesis;`，:25）
- Modify: `src-tauri/src/commands/orchestrator.rs`（清理残留 import）

**删除清单（已核实，勿超范围）：**
1. `narrative/genesis.rs` 全文件（3881 行，含 10 个 Step、GenesisContext、GenesisPipeline、内部测试）——**先迁出 3 个函数**：
   - `compute_trim_ratio`（:2277）、`should_retry_self_repetition`（:2290）、`select_first_chapter_content`（:2299），均 `pub(crate)`，被 `agents/orchestrator.rs` 的 execute_trishot 续写路径复用（:1832/:1834/:1904/:1909）；
   - 连同其测试（genesis.rs:3782/3793/3808 附近）迁入 `agents/trim_utils.rs`。
2. `narrative/mod.rs:25` 的 `pub mod genesis;`。
3. `commands/orchestrator.rs` 中 Task 7 后残留的创世相关 import（`GenesisContext`、`GenesisPipeline`、`GenesisRunRepository`、`NarrativePipelineExecutor`、`register_pipeline_cancel`、`BootstrapProgressEvent` 等——以编译器 unused 警告为准逐个清理；若 `BootstrapProgressEvent` 仍被该文件其他分支使用则保留）。

**保留清单（已核实，勿误删）：**
- `GenerationMode::TriShot` 与 `execute_trishot`（`planner/executor.rs:98-108/1153` 日常续写使用）；
- `genesis_runs` 表（V066）与 `GenesisRunRepository` 全部方法、`commands/story_system.rs:124/134` 读命令（GenesisPanel/ContractsTab 消费历史数据）；
- `BACKGROUND_LLM_SEMAPHORE`（`story_system/scene_service.rs:136/141` SceneIngestor 使用）；
- `is_novel_creation_intent`（lib.rs:1113，仍是路由判定）；`NarrativeBundle`（domain/narrative_elements.rs:372，多模块共享）；
- `planner/bootstrap.rs` 的类型（若仍被引用）。

- [ ] **Step 1: 迁移 trim 工具函数**

创建 `src-tauri/src/agents/trim_utils.rs`：把 `narrative/genesis.rs:2277-2320` 的 3 个函数与 :3782-3820 的对应测试原样搬入（`pub(crate)` 可见性保持），文件头加注释说明来源。`agents/mod.rs` 加 `pub(crate) mod trim_utils;`。`agents/orchestrator.rs` 的 4 处 `crate::narrative::genesis::{compute_trim_ratio, should_retry_self_repetition, select_first_chapter_content}` 改为 `crate::agents::trim_utils::{...}`。

Run: `cd src-tauri && cargo test --lib agents:: 2>&1 | tail -3`
Expected: PASS（迁移测试随文件走，行为不变）

- [ ] **Step 2: 删除 genesis.rs 并清理**

```bash
git rm src-tauri/src/narrative/genesis.rs
```

`narrative/mod.rs` 删 `pub mod genesis;` 行。编译，按 unused import 警告清理 `commands/orchestrator.rs`（及任何其他文件）的残留 import。

全库验证引用清零：

```bash
grep -rn "narrative::genesis\|GenesisPipeline\|GenesisContext" src-tauri/src --include="*.rs" | grep -v migrations
```

Expected: 无输出（V066 迁移属历史，不命中此模式）。

- [ ] **Step 3: 全量验证**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`
Expected: 全绿（genesis.rs 内部测试随之移除，总数减少属预期；无 failed）

Run: `cd src-frontend && npx vitest run 2>&1 | tail -3`
Expected: 292 passed

Run: `python scripts/architecture_guard.py 2>&1 | tail -3`
Expected: PASSED

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(agency): remove legacy genesis pipeline (replaced by agency framework)"
```

---

### Task 9: 发布就绪（0.27.0 + 文档 + 全量验证）

**Files:**
- Modify: `src-tauri/Cargo.toml:3`、`src-tauri/tauri.conf.json:4`、`src-frontend/package.json:4`、`AGENTS.md:10`（版本 0.26.59 → **0.27.0**）
- Modify: `ARCHITECTURE.md`（agency 小节补 P2：gate/预算/并行/切换）、`PROJECT_STATUS.md`（迭代记录 + 头部日期）、`docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（状态行 → "P1/P2 已完成"）、CHANGELOG 文件（仓库既有 changelog，按既有格式加 0.27.0 条目；先 Glob 找 `CHANGELOG*` 或 `docs/CHANGELOG*`）

- [ ] **Step 1: 版本与文档**

四处版本号改 0.27.0（`grep -rn "0.26.59" src-tauri/Cargo.toml src-tauri/tauri.conf.json src-frontend/package.json AGENTS.md` 确认恰好四处）。文档按 Files 清单更新。CHANGELOG 条目要点：

```markdown
## v0.27.0（2026-07-17）

### Agency 多代理创作框架（创世 2.0）P1+P2
- 新增 agency 模块：黑板协作 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）
- 质量门：编辑裁决 + 规则复检 + 至多 1 轮修订，未过门不装配
- 并行稳态循环：编辑审 N 与主创写 N+1 并发；按角色并发预算与 run 级 token 预算
- request_id 定点取消（不再全局取消）；续写循环 agency_continue_chapter/batch
- 创作资产自动落库（characters/world_buildings/story_outlines）
- smart_execute 创世路径切换到 agency；旧 GenesisPipeline 移除（TriShot 续写保留）
```

- [ ] **Step 2: 全量验证**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3` → 全绿
Run: `cd src-frontend && npx vitest run 2>&1 | tail -3` → 292 passed
Run: `python scripts/architecture_guard.py 2>&1 | tail -3` → PASSED
Run: `cd src-frontend && npm run build 2>&1 | tail -3` → 构建成功（版本号变更后前端产物自检）

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "release: v0.27.0 agency multi-agent framework (P1+P2)"
```

**真机验收（用户执行，发布前）：**
1. `cargo tauri dev` → 幕前输入"写一部双星废土拾荒少女的小说"→ 首章自动投递，进度条经 smart-execute-progress 推进，新故事出现在列表，第一章可见，角色/世界观/大纲出现在资产视图（落库验证）。
2. 控制台 `await window.__TAURI__.core.invoke('agency_continue_batch', { story_id: '<新故事id>', count: 2 })` → 第 2、3 章陆续出现（并行稳态循环）。
3. 创世/续写中途 `await window.__TAURI__.core.invoke('agency_cancel_run', { run_id: '<runId>' })` → 仅该 run 停止，其他写作功能不受影响（定点取消验证）。

---

## Self-Review（计划自审结论）

- **Spec coverage**：设计 P2 行（并行循环/并发预算/统一输出装配/删除旧路径）→ Task 5/6/3/8；P1 终审三项 P2 立项 → Task 1（定点取消）/Task 3（质量门）/Task 2（board_revise）；终审转 P2 小项 → Task 2（from_str warn、promote 事件、prompt 回归测试）、Task 4（入口护栏）、Task 7（smart_execute 切换）；续写循环（设计"稳态持续输出"必需）→ Task 4/6；资产落库缺口（P1 遗留，应用资产视图与规则复检的前提）→ Task 4。前端可视化按计划边界属 P5，本计划零前端改动（仅保持兼容契约）。
- **范围修正（对设计文档）**：TriShot 保留（planner 日常续写生产依赖，recon 证据 planner/executor.rs:98-108/1153）；genesis_runs 表与读命令保留（GenesisPanel/ContractsTab 生产消费）。两条均已在 Global Constraints 与 Task 8 保留清单固化。
- **Placeholder scan**：Task 3 的 `test_revision_uses_board_revise_in_place` 已改写为纯函数契约测试（动态 mock 不可行已说明）；Task 6 修订轮 mock 简化已声明（board_revise 语义由 Task 2 覆盖）；Task 7 的 `emit_story_created`/`record_ai_operation` 调用以现有代码搬移为准（已标注行号锚点）。无 TBD/TODO。
- **Type consistency**：`run_role_with_llm_and_budget`/`run_role_loop`/`GateRunner.evaluate`/`evaluate_gate_impl` 签名在 Task 3/5/6 间一致（Task 3 定义 evaluate_gate 本体，Task 5 加 budget 参，Task 6 提取自由函数与 GateRunner——执行时按 Task 6 重构说明统一）；`AgencyContinueResult` 字段在 Task 4/6 一致；`ProgressSink`/`run_genesis_with_sink` 在 Task 6 定义、Task 7 消费；`build_bootstrap_result` 契约在 Task 7 测试与实现一致。
