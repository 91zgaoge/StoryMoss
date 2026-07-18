# Agency P4：验证循环（Grader 分级 / 检查点 / Eval Harness / 评估仪表盘）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立四级 grader（code→rule→model→human，确定性优先）与 Gate v2 统一加权评分（阈值 0.75）取代 v1 二元判定；检查点里程碑指标快照与对比；evals 场景 harness（pass@k/pass^k + baseline）；评估仪表盘前端页；并修复 migration runner 目录优先级（P3 终审首项）与 resume spawn 化。

**Architecture:** grader 分层收敛于 `agency/graders.rs`——code/rule 为确定性启发式（复用 trim_utils/evaluate_contract_fulfillment/run_subagent_review/ContentFeatureExtractor），model 为 LLM 层（editor 裁决 + QualityChecker rubric 1-5 须引证据，Background 档）；Gate v2 在 P2 `evaluate_gate` 内加权合成 `GateScore` 落审查区。检查点入 V110 `agency_checkpoints`（里程碑 metrics_json）。eval harness 以 JSON 场景 + MockLlm 回放驱动确定性 CI 断言，real-LLM 模式经环境变量启用手动跑 baseline。仪表盘为纯前端页（手绘 SVG，复用 ReadingPowerChart 模式）。

**Tech Stack:** Rust（Tauri 2.4）、rusqlite + r2d2、tokio、serde/serde_json、async-trait、React 18 + TS + Zustand + TanStack Query（前端）。

**设计文档:** `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P4 行 + ECC 验证循环映射）。
**P3 终审转项：** migration runner 目录优先级（首项）；resume spawn 化（取消盲窗）；finalize 顺序（先 emit 后 finalize）。

## Global Constraints

- 测试基线必须保持绿：`cargo test --lib`（现 832 passed + 2 ignored）与 `cd src-frontend && npx vitest run`（292 passed）。
- 所有 DB 同步调用在 async 上下文中必须 `tokio::task::spawn_blocking` / `self.db` 包裹。
- 已核实接口事实：`crate::error::AppError`；`AppError::validation_failed(msg, None::<String>)` 双参；`AppError::from(String)`；测试内存库 `crate::db::create_test_pool()`。
- grader 复用件（已核实签名）：
  - `crate::agents::trim_utils::compute_trim_ratio(raw_chars, cleaned_chars)` / `should_retry_self_repetition`（P2 T8 迁入）；
  - `crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, &RuntimeContract) -> FulfillmentResult { score, covered_nodes, violated_rules, forbidden_zones_hit }`（纯同步，:11-28）；`ContractService::get_runtime_contract(story_id, chapter_number) -> Result<RuntimeContract, String>`（contract_service.rs:118）；
  - `crate::reading_power::ContentFeatureExtractor::extract(&content)`（evaluator.rs:25，**可见性待核实**；若为私有则经 `ReadingPowerEvaluator` 或改 pub，实现时判定）；
  - `crate::agents::subagents::{run_subagent_review, merge via agency::gate::merge_rule_issues}`；
  - `QualityChecker::check_with_llm(content, &llm).await`（audit/mod.rs:552-584 的 llm_deep_audit 内部使用，**签名/返回类型待核实**，model grader 用它而非 audit_scene——gate 在装配前无 scene_id）。
- migration runner：`default_runner`（db/migrations/mod.rs:84-129）候选目录数组 + `find(exists)`；`parse_filename`（:380）版本解析。`target/debug/db/migrations` 无任何写入方（一次性历史残留），修复后建议用户手动删除该目录一次。
- 迁移：V110 起（V109 为当前最新）；纯 SQL 自动发现。
- CI 现状：build.yml 的 `cargo test --lib` 为 continue-on-error（.github/workflows/build.yml:96-98）——eval harness 的确定性测试随 `cargo test --lib` 运行即可纳入；不改 CI 配置。
- P1-P3 行为不得回退：终态守护、latest_draft_by_key、审查区只存真实裁决（gate v2 的 GateReport 必须来自真实评估）、spawn_blocking、角色模型路由（model grader 用 EditorAuditor 即 Background 档）。
- 版本号四文件 + 双 lockfile：本计划发布 **0.29.0**。
- Commit 用 Conventional Commits。

---

### Task 1: migration runner 目录优先级修复（P3 终审首项）

**Files:**
- Modify: `src-tauri/src/db/migrations/mod.rs`（`default_runner` 选目逻辑 + 测试）

**Interfaces:**
- Consumes: `parse_filename`（:380）；候选目录数组（:84-129）。
- Produces: `default_runner` 改为"存在的候选中选 .sql 最高版本最大者"；多候选存在且最高版本不一致时 `log::warn!` 双方路径；`fn max_sql_version(dir: &Path) -> Option<i32>`（pub(crate)，测试消费）；`fn pick_migrations_dir(candidates: &[PathBuf]) -> PathBuf`（pub(crate)）。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/db/migrations/mod.rs` 测试模块追加：

```rust
#[test]
fn test_pick_migrations_dir_prefers_highest_version() {
    let base = std::env::temp_dir().join(format!("mig-pick-{}", uuid::Uuid::new_v4()));
    let stale = base.join("target/debug/db/migrations");
    let fresh = base.join("src-tauri/src/db/migrations");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::create_dir_all(&fresh).unwrap();
    // 陈旧副本：只到 V106；源码目录：到 V109
    std::fs::write(stale.join("V106__a.sql"), "-- x").unwrap();
    std::fs::write(fresh.join("V106__a.sql"), "-- x").unwrap();
    std::fs::write(fresh.join("V109__b.sql"), "-- y").unwrap();
    let candidates = vec![stale.clone(), fresh.clone()]; // stale 排前（复现旧 find(exists) 命中）
    let picked = pick_migrations_dir(&candidates);
    assert_eq!(picked, fresh);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_pick_migrations_dir_falls_back_to_first_existing_when_no_sql() {
    let base = std::env::temp_dir().join(format!("mig-pick-empty-{}", uuid::Uuid::new_v4()));
    let a = base.join("a");
    let b = base.join("b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("V109__b.sql"), "-- y").unwrap();
    // a 存在但无 .sql → 选 b；都无 .sql → 第一个存在
    let picked = pick_migrations_dir(&[a.clone(), b.clone()]);
    assert_eq!(picked, b);
    let picked2 = pick_migrations_dir(&[a.clone()]);
    assert_eq!(picked2, a);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn test_max_sql_version() {
    let base = std::env::temp_dir().join(format!("mig-max-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    assert_eq!(max_sql_version(&base), None);
    std::fs::write(base.join("V103__x.sql"), "--").unwrap();
    std::fs::write(base.join("V109__y.sql"), "--").unwrap();
    std::fs::write(base.join("notes.md"), "not a migration").unwrap();
    assert_eq!(max_sql_version(&base), Some(109));
    let _ = std::fs::remove_dir_all(&base);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib migrations 2>&1 | tail -3`
Expected: FAIL（`pick_migrations_dir`/`max_sql_version` 未定义）

- [ ] **Step 3: 实现**

`default_runner` 的选目段替换（候选数组不变）：

```rust
/// 在存在的候选目录中选 .sql 最高版本号最大者（修复"陈旧 target 副本遮蔽新迁移"）。
/// 多候选存在且最高版本不一致时 warn 双方路径。
pub(crate) fn pick_migrations_dir(candidates: &[PathBuf]) -> PathBuf {
    let existing: Vec<&PathBuf> = candidates.iter().filter(|p| p.exists()).collect();
    let fallback = existing
        .first()
        .map(|p| (*p).clone())
        .unwrap_or_else(|| candidates.last().unwrap().clone());
    let mut best: Option<(&PathBuf, i32)> = None;
    for dir in &existing {
        if let Some(v) = max_sql_version(dir) {
            match &best {
                Some((_, bv)) if *bv >= v => {}
                _ => best = Some((dir, v)),
            }
        }
    }
    if let Some((best_dir, best_v)) = best {
        for dir in &existing {
            if *dir != best_dir {
                if let Some(v) = max_sql_version(dir) {
                    if v != best_v {
                        log::warn!(
                            "[migrations] 多个迁移目录存在且版本不一致：选用 {}（V{}），忽略 {}（V{}）。建议删除陈旧目录。",
                            best_dir.display(), best_v, dir.display(), v
                        );
                    }
                }
            }
        }
        log::info!("[migrations] 选用迁移目录：{}（V{}）", best_dir.display(), best_v);
        return best_dir.clone();
    }
    fallback
}

/// 目录中最高的 V{num}__ 迁移版本号（无 .sql 返回 None）。
pub(crate) fn max_sql_version(dir: &Path) -> Option<i32> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().extension().map(|x| x == "sql").unwrap_or(false) {
                parse_filename(&name).ok().map(|(v, _)| v)
            } else {
                None
            }
        })
        .max()
}
```

`default_runner` 中 `let dir = candidates.iter().find(...)...` 替换为 `let dir = pick_migrations_dir(&candidates);`（`use std::path::{Path, PathBuf};` 按需补）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`
Expected: 全绿（832 + 3 新增），无新警告。手动验证一次：`cargo test --lib migrations 2>&1 | grep 选用迁移目录`（log 输出非必需）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations/mod.rs
git commit -m "fix(db): pick migrations dir by highest version, warn on stale shadow copies"
```

---

### Task 2: resume spawn 化 + finalize 顺序 + 种子模板孤儿清理

**Files:**
- Modify: `src-tauri/src/agency/coordinator.rs`（resume_run 拆 prepare/continue；finalize 移到 emit 后后台执行）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_resume_run` 改 spawn + 立即返回）
- Delete: `resources/prompts/creation/narrative_outline_generate.md`（T6 遗留孤儿种子模板，唯一 Rust 消费方 outline_prompt 已删）
- Modify: 引用该模板的测试（grep `narrative_outline_generate` 定位，同步删除/改造）

**Interfaces:**
- Consumes: 现状 resume_run（coordinator.rs:1413-1528）、finalize_session（:438-512）、三入口外层 match。
- Produces: `AgencyCoordinator::resume_prepare(old_run_id) -> Result<ResumeOutcome, AppError>`（校验+复制+简报，不启动 batch）；`resume_run` 保留为 prepare+batch 的组合（测试与兼容用）；commands 层：prepare → 立即返回 outcome → spawn batch。`agency_resume_run` 的 IPC 行为变更为"立即返回 new_run_id，batch 后台跑"。

- [ ] **Step 1: 写失败的测试**

`coordinator.rs` 测试模块追加：

```rust
#[tokio::test]
async fn test_resume_prepare_does_not_start_batch() {
    let pool = create_test_pool().unwrap();
    // 与 test_resume_run_restores_board_and_wraps_history 相同的种子（旧 run completed + 资产 + 摘要 + 第一章场景 + 角色）
    // ……（复用该测试的种子代码，逐行复制，不得引用）……
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    let outcome = coordinator.resume_prepare("old-run").await.unwrap();
    assert_eq!(outcome.resumed_from, "old-run");
    // prepare 不启动 batch：mock 无脚本也不会被调用；黑板已复制、简报已写
    let snap = crate::agency::board::BlackboardService::new(pool.clone()).snapshot(&outcome.new_run_id).unwrap();
    assert!(snap.assets.iter().any(|i| i.key == "世界观"));
    assert!(snap.schedules.iter().any(|i| i.key == "恢复简报"));
    // 新 run 存在且未被 finalize（status 仍为 pending——batch 未跑）
    let run = AgencyRepository::new(pool.clone()).get_run(&outcome.new_run_id).unwrap().unwrap();
    assert_eq!(run.status, "pending");
}
```

注：现有 `test_resume_run_restores_board_and_wraps_history`（P3 T4）应继续通过——`resume_run` 保持 prepare+batch 组合语义。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（`resume_prepare` 未定义）

- [ ] **Step 3: 实现**

`coordinator.rs`：把 `resume_run` 的 1-3 步（校验/护栏/复制/简报）原样提取为 `pub async fn resume_prepare(&self, old_run_id: &str) -> Result<ResumeOutcome, AppError>`（返回相同的 ResumeOutcome）；`resume_run` 改为：

```rust
pub async fn resume_run(&self, old_run_id: &str) -> Result<ResumeOutcome, AppError> {
    let outcome = self.resume_prepare(old_run_id).await?;
    let start_chapter = {
        let pool = self.pool.clone();
        let sid = outcome.story_id.clone();
        self.db(move || Self::next_chapter_number(&pool, &sid)).await?
    };
    self.run_continue_batch(&outcome.new_run_id, &outcome.story_id, start_chapter, 1).await?;
    Ok(outcome)
}
```

（原 resume_run 第 4 步从 resume_prepare 中移除。）

`commands.rs` 的 `agency_resume_run` 改 spawn 模式：

```rust
/// 跨会话恢复：立即返回 ResumeOutcome（含 new_run_id），续写 batch 在后台执行。
/// 进度经 agency-run-progress / agency-agent-activity 事件推送；取消用 agency_cancel_run(new_run_id)。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_resume_run(
    old_run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::coordinator::ResumeOutcome, AppError> {
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let outcome = coordinator.resume_prepare(&old_run_id).await?;
    let (new_run_id, story_id) = (outcome.new_run_id.clone(), outcome.story_id.clone());
    let outcome_ret = outcome.clone();
    tauri::async_runtime::spawn(async move {
        let start = match AgencyCoordinator::next_chapter_number_async(&coordinator, &story_id).await {
            Ok(n) => n,
            Err(e) => {
                log::error!("resume batch chapter number failed: {}", e);
                return;
            }
        };
        if let Err(e) = coordinator.run_continue_batch(&new_run_id, &story_id, start, 1).await {
            log::error!("resume batch run {} failed: {}", new_run_id, e);
        }
    });
    Ok(outcome_ret)
}
```

`coordinator.rs` 加辅助：

```rust
#[doc(hidden)]
pub async fn next_chapter_number_async(&self, story_id: &str) -> Result<i32, AppError> {
    let pool = self.pool.clone();
    let sid = story_id.to_string();
    self.db(move || Self::next_chapter_number(&pool, &sid)).await
}
```

`ResumeOutcome` 补 `#[derive(Clone)]`（若未 derive）。

finalize 顺序：三入口外层 match 改为 `finish_run` → `emit_progress` → spawn finalize（不 await）：

```rust
// 以 run_genesis Ok 分支为例（其余两处同构）：
Ok(r) => {
    let json = serde_json::to_string(r).unwrap_or_default();
    let _ = self.db(move || repo_c.finish_run(run_id, "completed", Some(&json), None)).await;
    self.emit_progress(run_id, "assembly", "completed", "创世完成");
    // 摘要生成后台化（P4）：完成事件不被 LLM 摘要延迟
    let fin = self.clone_for_finalize();
    let rid = run_id.to_string();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = fin.finalize_session(&rid).await {
            log::warn!("finalize_session({}) 失败: {}", rid, e);
        }
    });
}
```

配套：`finalize_session` 签名去 `llm: &Arc<dyn LoopLlm>` 参数（内部自行 `llm_for_run(run_id, AgentRole::EditorAuditor)`——原调用方传的就是它）；`AgencyCoordinator` 加 `fn clone_for_finalize(&self) -> Self`（app_handle/pool/llm 三字段克隆）；`finalize_session` 内失败路径全部 log::warn! 后 Ok(())（spawn 内不再向上传播）。错误分支同样先 emit 后 spawn finalize。

种子模板清理：`git rm resources/prompts/creation/narrative_outline_generate.md`；`grep -rn "narrative_outline_generate" src-tauri/src src-frontend/src` 定位引用测试（已知 `narrative/prompts.rs` 的 `background_generate_templates_declare_strategy_section`），删除或改造该用例并在报告说明。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`
Expected: 全绿（新增 1，模板清理可能 -1），无新警告。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(agency): resume spawn mode + async finalize + drop orphan seed template"
```

---

### Task 3: code/rule graders（确定性层）

**Files:**
- Create: `src-tauri/src/agency/graders.rs`
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod graders;`）

**Interfaces:**
- Consumes: `trim_utils::compute_trim_ratio`；`evaluate_contract_fulfillment`；`gate::build_review_context` + `run_subagent_review` + `merge_rule_issues`；`ContentFeatureExtractor`（可见性判定见下）。
- Produces: `CodeGraderReport { word_count, repetition_ratio, forbidden_hits, score, issues }`；`RuleGraderReport { contract_score, reading_power_score, subagent_issues, score, issues }`；`graders::run_code_grader(content, contract: Option<&RuntimeContract>) -> CodeGraderReport`（同步）；`graders::run_rule_grader(pool, story_id, chapter_number, content, foreshadowing_hints) -> RuleGraderReport`（同步，DB 读取内含）；chapter_number 经 draft.key 解析（"第N章"→N）。Task 5 Gate v2 消费。

**评分公式（spec）：**
- code：`score = 1.0 - repetition_penalty - forbidden_penalty - length_penalty`；repetition_ratio>0.08 每超 0.01 扣 0.05（上限 0.4）；forbidden_hits 每个扣 0.25（合同 forbidden_zones 经 evaluate_contract_fulfillment 命中）；字数 <800 扣 0.2、<200 扣 0.5（length_penalty 取大者）；clamp 0..1。
- rule：`score = contract_score*0.5 + reading_power_score*0.5`（无合同时 contract_score 取 reading_power_score）；reading_power_score 用 ContentFeatureExtractor 特征按 `hook*0.4+coolpoint*0.3+micropayoff*0.3` 公式（无 debt，gate 章内无 debt 语义）；subagent High+ 问题不扣分但全部进 issues（拦截决策留给 Gate v2）。

- [ ] **Step 1: 写失败的测试**

`graders.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_code_grader_clean_content() {
        let content = "正常的章节正文。".repeat(300); // ~2100 字，无重复禁则
        let report = run_code_grader(&content, None);
        assert!(report.score > 0.9, "干净内容应高分: {}", report.score);
        assert!(report.word_count >= 2000);
        assert!(report.forbidden_hits.is_empty());
    }

    #[test]
    fn test_code_grader_penalizes_repetition_and_short() {
        let content = "同一句开头。同一句开头。同一句开头。同一句开头。"; // 极短且高重复
        let report = run_code_grader(content, None);
        assert!(report.score < 0.5, "短且重复应低分: {}", report.score);
        assert!(report.issues.iter().any(|i| i.contains("字数") || i.contains("重复")));
    }

    #[test]
    fn test_rule_grader_without_contract() {
        let pool = create_test_pool().unwrap();
        // 无故事资产 → 无合同；追读力特征取自内容本身
        let content = "他推开那扇门，门外竟是失踪十年的师父。「你怎么会在这里？」".to_string()
            + &"情节推进。".repeat(400);
        let report = run_rule_grader(&pool, "story-x", 1, &content, &[]);
        assert!(report.score >= 0.0 && report.score <= 1.0);
        assert_eq!(report.contract_score, report.reading_power_score); // 无合同时回退
    }

    #[test]
    fn test_parse_chapter_number_from_key() {
        assert_eq!(parse_chapter_number("第3章"), Some(3));
        assert_eq!(parse_chapter_number("第12章"), Some(12));
        assert_eq!(parse_chapter_number("序章"), None);
        assert_eq!(parse_chapter_number("第一章"), None); // 中文数字不解析（生产 key 为阿拉伯）
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::graders 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`src-tauri/src/agency/graders.rs`：

```rust
//! 确定性 grader 层（code/rule，ECC 四级 grader 的前两级——零 LLM 成本）。

use crate::db::DbPool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CodeGraderReport {
    pub word_count: usize,
    pub repetition_ratio: f64,
    pub forbidden_hits: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleGraderReport {
    pub contract_score: f64,
    pub reading_power_score: f64,
    pub subagent_issues: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

pub fn parse_chapter_number(key: &str) -> Option<i32> {
    let digits: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !key.starts_with('第') || !key.ends_with('章') {
        return None;
    }
    digits.parse().ok()
}

pub fn run_code_grader(
    content: &str,
    contract: Option<&crate::domain::contracts::RuntimeContract>,
) -> CodeGraderReport {
    let word_count = content.chars().count();
    let mut issues = Vec::new();
    // 自重复率（trim_utils：raw vs 去重后 cleaned 的裁剪比）
    let cleaned = crate::agents::trim_utils::trim_self_repetition(content);
    let repetition_ratio = crate::agents::trim_utils::compute_trim_ratio(word_count, cleaned.chars().count());
    let mut score = 1.0f64;
    if repetition_ratio > 0.08 {
        let penalty = ((repetition_ratio - 0.08) * 100.0).ceil() as f64 * 0.05;
        score -= penalty.min(0.4);
        issues.push(format!("自重复率 {:.1}%（阈值 8%）", repetition_ratio * 100.0));
    }
    // 字数
    if word_count < 200 {
        score -= 0.5;
        issues.push(format!("字数过少（{}）", word_count));
    } else if word_count < 800 {
        score -= 0.2;
        issues.push(format!("字数偏少（{}）", word_count));
    }
    // 合同禁则区
    let forbidden_hits = match contract {
        Some(c) => {
            let result = crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c);
            let hits = result.forbidden_zones_hit.clone();
            score -= 0.25 * hits.len() as f64;
            for h in &hits {
                issues.push(format!("禁则区命中: {}", h));
            }
            hits
        }
        None => Vec::new(),
    };
    CodeGraderReport {
        word_count,
        repetition_ratio,
        forbidden_hits,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

/// 同步 rule grader（DB 读取内含：合同 + 复检上下文；调用方负责 spawn_blocking）。
pub fn run_rule_grader(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
    content: &str,
    foreshadowing_hints: &[String],
) -> RuleGraderReport {
    // 合同兑现（无合同则回退追读力分）
    let contract = crate::story_system::contract_service::ContractService::new(pool.clone())
        .get_runtime_contract(story_id, chapter_number)
        .ok();
    let (contract_score, has_contract) = match &contract {
        Some(c) => (crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c).score, true),
        None => (0.0, false),
    };
    // 追读力（纯规则特征；可见性判定：pub 则用 ContentFeatureExtractor，否则 ReadingPowerEvaluator 兜底）
    let reading_power_score = reading_power_score_of(content);
    let contract_component = if has_contract { contract_score } else { reading_power_score };
    // 规则子代理复检（High+ 全进 issues）
    let ctx = crate::agency::gate::build_review_context(pool, story_id, foreshadowing_hints);
    let rt = tokio::runtime::Handle::try_current();
    let subagent_issues = match rt {
        Ok(handle) => tokio::task::block_in_place(|| {
            handle.block_on(async {
                let notes = crate::agents::subagents::run_subagent_review(&ctx, content).await;
                crate::agency::gate::merge_rule_issues(&notes)
            })
        }),
        Err(_) => Vec::new(),
    };
    let score = contract_component * 0.5 + reading_power_score * 0.5;
    let mut issues = Vec::new();
    if has_contract && contract_score < 0.7 {
        issues.push(format!("合同兑现偏低（{:.2}）", contract_score));
    }
    issues.extend(subagent_issues.iter().cloned());
    RuleGraderReport {
        contract_score,
        reading_power_score,
        subagent_issues,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

fn reading_power_score_of(content: &str) -> f64 {
    // ContentFeatureExtractor::extract 可见性判定：
    // - 若 pub：直接 extract → hook*0.4 + coolpoint*0.3 + micropayoff*0.3（与 evaluator.rs:119 公式一致，无 debt 项）
    // - 若私有：把 extract 改 pub（reading_power/evaluator.rs，一处可见性修改，报告注明）
    // 实现时按 reading_power/evaluator.rs 的 Features 字段名对齐：
    //   hook_strength（f64 或枚举映射 weak=0.2/medium=0.6/strong=1.0）、coolpoint_count、micropayoff_count
    // 归一化：coolpoint/micropayoff 按 min(count,3)/3.0 计
    todo!("按 reading_power/evaluator.rs 实际结构实现")
}
```

实现注意：
- `trim_self_repetition` 是否在 trim_utils 中——P2 T8 迁移的是 compute_trim_ratio/should_retry_self_repetition/select_first_chapter_content 三函数；若 `trim_self_repetition` 不在其中，用 `FrontstageApp` 前端同名函数对应的 Rust 实现（grep `trim_self_repetition` src-tauri/src，取实际存在的实现；不存在则用 compute_trim_ratio(content.len, dedup_lines(content).len) 自实现 10 行 dedup 并在报告说明）。
- `block_in_place` 需要 tokio multi_thread runtime——`create_test_pool` 的测试经 `#[tokio::test]`（默认 multi_thread？项目用 `#[tokio::test]` 还是 `#[tokio::test(flavor = "multi_thread")]`，以现有 coordinator 测试写法为准）；若 block_in_place panic，改为 run_rule_grader async 化（`pub async fn`，调用方 .await，测试直接 await——**优先选 async 化**，避免 block_in_place 依赖；Gate v2 在 async 上下文调用）。
- `ContractService::new(pool)` 签名以 contract_service.rs 实际为准。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency::graders 2>&1 | tail -3`（4 新增）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/graders.rs src-tauri/src/agency/mod.rs
git commit -m "feat(agency): deterministic code/rule graders (repetition, contract, reading-power)"
```

---

### Task 4: model grader（编辑裁决 rubric 化，零新增 LLM 调用）

**Files:**
- Modify: `resources/prompts/agency/agency_editor_auditor_system.md`（裁决 JSON 加 score/dimension_scores/evidence 要求）
- Modify: `src-tauri/src/agency/coordinator.rs`（EditorVerdict 扩展 + 向后兼容解析 + ModelGraderReport 派生）

**Interfaces:**
- Consumes: 现状 editor 裁决 JSON 流（evaluate_gate_impl 的 parse_lenient）。
- Produces: `EditorVerdict { verdict, blocking_issues, suggestions, comments, score: Option<f64>, dimension_scores: Option<HashMap<String, f64>> }`（新字段均 Option，旧格式兼容）；`ModelGraderReport { model_score: f64, dimension_scores: HashMap<String, f64>, evidence_issues: Vec<String>, comments: String }`；`ModelGraderReport::from_verdict(&EditorVerdict) -> Self`（score 缺失时按 verdict 回退：pass=0.85、revise=0.4、其他=0.5）。Task 5 消费 `model_score`。

**设计决定（记录）：** model grader 不新增 LLM 调用——编辑审计本身即五维审查（其提示词已要求引证据），rubric 化其输出（score 1-5 + 维度分）即得 model 层评分；独立 `QualityChecker::check_with_llm` 依赖具体 LlmService 无法 mock，且重复调用浪费 token。

- [ ] **Step 1: 写失败的测试**

`coordinator.rs` 测试模块追加：

```rust
#[test]
fn test_verdict_with_rubric_scores() {
    let raw = r#"{"verdict":"pass","score":4.2,"dimension_scores":{"continuity":4.5,"style":4.0,"contract":4.0,"ai_tone":4.5,"hook":3.8},"blocking_issues":[],"suggestions":[],"comments":"好"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    assert_eq!(v.verdict, "pass");
    let report = ModelGraderReport::from_verdict(&v);
    assert!((report.model_score - 0.84).abs() < 0.001); // 4.2/5
    assert_eq!(report.dimension_scores.get("continuity"), Some(&4.5));
}

#[test]
fn test_verdict_legacy_format_fallback() {
    // 旧格式（无 score 字段）向后兼容
    let raw = r#"{"verdict":"revise","blocking_issues":["动机缺失"],"suggestions":[],"comments":"修"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    assert!(v.score.is_none());
    let report = ModelGraderReport::from_verdict(&v);
    assert!((report.model_score - 0.4).abs() < 0.001);
    assert!((ModelGraderReport::from_verdict(&EditorVerdict {
        verdict: "pass".into(), blocking_issues: vec![], suggestions: vec![], comments: String::new(),
        score: None, dimension_scores: None,
    }).model_score - 0.85).abs() < 0.001);
}

#[test]
fn test_evidence_issues_collected() {
    let raw = r#"{"verdict":"revise","score":2.0,"blocking_issues":[{"issue":"角色动机断裂","evidence":"「他突然放弃复仇」"}],"suggestions":[],"comments":"修"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    let report = ModelGraderReport::from_verdict(&v);
    assert!(report.evidence_issues.iter().any(|i| i.contains("角色动机断裂")));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（score/dimension_scores/ModelGraderReport 未定义）

- [ ] **Step 3: 实现**

`coordinator.rs`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorVerdict {
    pub verdict: String, // pass | revise
    #[serde(default)]
    pub blocking_issues: Vec<serde_json::Value>, // 字符串或 {"issue","evidence"} 对象均可
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub comments: String,
    #[serde(default)]
    pub score: Option<f64>,                       // rubric 1-5（P4 rubric 化）
    #[serde(default)]
    pub dimension_scores: Option<std::collections::HashMap<String, f64>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelGraderReport {
    pub model_score: f64, // 0-1
    pub dimension_scores: std::collections::HashMap<String, f64>,
    pub evidence_issues: Vec<String>,
    pub comments: String,
}

impl ModelGraderReport {
    pub fn from_verdict(verdict: &EditorVerdict) -> Self {
        let model_score = match verdict.score {
            Some(s) => (s / 5.0).clamp(0.0, 1.0),
            None => match verdict.verdict.as_str() {
                "pass" => 0.85,
                "revise" => 0.4,
                _ => 0.5,
            },
        };
        let evidence_issues = verdict
            .blocking_issues
            .iter()
            .map(|i| match i {
                serde_json::Value::String(s) => s.clone(),
                other => other
                    .get("issue")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| other.to_string()),
            })
            .collect();
        Self {
            model_score,
            dimension_scores: verdict.dimension_scores.clone().unwrap_or_default(),
            evidence_issues,
            comments: verdict.comments.clone(),
        }
    }

    /// blocking_issues 的字符串视图（Gate v2 合并问题清单用）。
    pub fn blocking_strings(verdict: &EditorVerdict) -> Vec<String> {
        Self::from_verdict(verdict).evidence_issues
    }
}
```

注意：`blocking_issues` 元素类型由 `Vec<String>` 改 `Vec<serde_json::Value>`——所有现存构造点（默认放行/测试脚本构造）适配；`verdict.blocking_issues.join("；")` 类用法改为 `ModelGraderReport::blocking_strings(&verdict).join("；")`。既有测试中断言 blocking_issues 内容的同步适配。

`agency_editor_auditor_system.md` 的 final 输出约定改为：

```markdown
- 逐维度审查后输出 final，content 必须是如下 JSON：
  {"verdict":"pass 或 revise",
   "score": 1-5 的总分（小数，5=出版级）,
   "dimension_scores":{"continuity":1-5,"style":1-5,"contract":1-5,"ai_tone":1-5,"hook":1-5},
   "blocking_issues":[{"issue":"阻断问题","evidence":"草稿原文引文"}],
   "suggestions":["非阻断建议（可空）"],
   "comments":"总评（≤200字）"}
- 每条 blocking_issues 必须带 evidence（引用草稿原文）；没有证据的问题降级为 suggestion。
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 3）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs resources/prompts/agency/agency_editor_auditor_system.md
git commit -m "feat(agency): rubric-based editor verdict with dimension scores and evidence"
```

---

### Task 5: Gate v2 统一加权评分

**Files:**
- Modify: `src-tauri/src/agency/coordinator.rs`（evaluate_gate_impl 加权合成 + record_gate_impl 写 gate_score）
- Modify: `src-tauri/src/agency/gate.rs`（GateScore 结构）

**Interfaces:**
- Consumes: T3 的 `run_code_grader/run_rule_grader/parse_chapter_number`；T4 的 `ModelGraderReport`。
- Produces: `GateScore { code, rule, model, weighted, threshold }`（serde）；常量 `GATE_PASS_THRESHOLD: f64 = 0.75`；gate 条目 content JSON 增加 `gate_score` 字段。`GateOutcome` 形状不变（Passed/RevisionRequired/Failed）。

**判定规则（spec，替代 v1）：**
1. editor aborted → Failed（不变）；
2. 裁决解析失败重试 1 次 → Failed（不变）；
3. `model = ModelGraderReport::from_verdict(&verdict)`；`code = run_code_grader(content, contract)`；`rule = run_rule_grader(...)`；
4. `weighted = 0.2*code.score + 0.3*rule.score + 0.5*model.model_score`；
5. verdict=revise 且 blocking 非空 → RevisionRequired（issues = blocking_strings + rule.subagent_issues + code.issues 去重）；
6. 否则 weighted < 0.75 → RevisionRequired（issues 以 grader 低分项为主）；
7. 否则 → Passed。

- [ ] **Step 1: 写失败的测试**

`coordinator.rs` 测试模块追加（复用 RoutingMock）：

```rust
#[tokio::test]
async fn test_gate_v2_low_weighted_triggers_revision() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    // editor 判 pass 但 score 极低（1.0/5 → model 0.2）→ weighted 必然 < 0.75 → 修订
    let mock = RoutingMock::new(0);
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第1章","content":"第1章正文，字数足够多。"}}}}"#.replace("}}}}", "}}").as_str(),
        // 上行是示意——实际用例如下两行（实现时删掉示意行）：
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第1章","content":"__LONG__","summary":"一"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        // 修订轮
        r#"{"type":"final","content":"已修订"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":1.0,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"勉强\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let result = coordinator.run_continue("gv2-1", &story_id, 1).await.unwrap();
    assert!(result.revised, "低 rubric 分应触发修订: {:?}", result.verdict);
    // gate 条目含 gate_score 字段
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let reviews = board.list_zone("gv2-1", BoardZone::Review).unwrap();
    let gate_item = reviews.iter().find(|i| i.item_type == "gate").unwrap();
    let content: serde_json::Value = serde_json::from_str(&gate_item.content).unwrap();
    assert!(content.get("gate_score").is_some());
    let weighted = content["gate_score"]["weighted"].as_f64().unwrap();
    assert!(weighted < 0.75, "首轮 weighted 应低于阈值: {}", weighted);
}
```

注：`__LONG__` 在测试代码里替换为 `"正文".repeat(500)`（word_count ≥800 避免 code 档字数扣分干扰断言）；示意行删除。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -3`
Expected: FAIL（gate_score 未写）

- [ ] **Step 3: 实现**

`gate.rs` 追加：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GateScore {
    pub code: f64,
    pub rule: f64,
    pub model: f64,
    pub weighted: f64,
    pub threshold: f64,
}

pub const GATE_PASS_THRESHOLD: f64 = 0.75;

impl GateScore {
    pub fn new(code: f64, rule: f64, model: f64) -> Self {
        let weighted = 0.2 * code + 0.3 * rule + 0.5 * model;
        Self { code, rule, model, weighted, threshold: GATE_PASS_THRESHOLD }
    }
}
```

`evaluate_gate_impl`（coordinator.rs，与 GateRunner 共用）在裁决解析成功后：

```rust
// Gate v2：确定性 grader + rubric 化 model 分
let model = ModelGraderReport::from_verdict(&verdict);
let chapter_number = crate::agency::graders::parse_chapter_number(&draft.key).unwrap_or(1);
let pool_c = pool.clone();
let sid = story_id.to_string();
let content_c = draft.content.clone();
let (code_report, rule_report) = {
    let contract = /* spawn_blocking：ContractService::get_runtime_contract(&sid, chapter_number).ok() */;
    let pool2 = pool_c.clone();
    let sid2 = sid.clone();
    let content2 = content_c.clone();
    let hints = foreshadowing_hints(&board_or_pool, &rid);
    tokio::task::spawn_blocking(move || {
        let code = crate::agency::graders::run_code_grader(&content2, contract.as_ref());
        (code,)
    })
    // rule grader 若为 async（T3 实现决定）则 .await 调用：
    // let rule = crate::agency::graders::run_rule_grader(&pool_c, &sid, chapter_number, &content_c, &hints).await;
};
let gate_score = crate::agency::gate::GateScore::new(
    code_report.score, rule_report.score, model.model_score,
);
```

判定替换 v1 的 3-4 条（见 spec 5-7）；`record_gate_impl` 的 content JSON 增加：

```rust
let content = serde_json::json!({
    "outcome": kind,
    "verdict": gate_verdict(outcome),
    "rule_issue_count": issues.len(),
    "issues": issues,
    "comments": verdict_comments(outcome),
    "gate_score": gate_score, // Option<&GateScore>，Failed 时为 null
}).to_string();
```

`record_gate_impl` 签名加 `gate_score: Option<&GateScore>`；调用点同步。RevisionRequired 的 issues 合并：`blocking_strings + rule_report.subagent_issues + code_report.issues` 去重（保留序）。

注意：`run_rule_grader` 内部含 DB 读取与 block_on 风险——T3 若实现为 async，此处直接 `.await`（evaluate_gate_impl 本在 async 上下文）；contract 查询与 hints 收集沿用现有 spawn_blocking 模式。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 1+）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs src-tauri/src/agency/gate.rs
git commit -m "feat(agency): gate v2 weighted scoring (code/rule/model, threshold 0.75)"
```

---

### Task 6: 检查点（V110 agency_checkpoints + 里程碑快照 + 对比 + IPC）

**Files:**
- Create: `src-tauri/src/db/migrations/V110__agency_checkpoints.sql`
- Modify: `src-tauri/src/agency/repository.rs`（checkpoint 方法）
- Modify: `src-tauri/src/agency/coordinator.rs`（里程碑钩子 + tokens_used 采集）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_list_checkpoints`/`agency_compare_checkpoints`）
- Modify: `src-tauri/src/handlers.rs`（注册）

**Interfaces:**
- Produces: `AgencyCheckpoint { id, run_id, story_id, milestone, chapter_number, metrics_json, created_at }`；`AgencyRepository::{insert_checkpoint, list_checkpoints(story_id), get_checkpoint(id)}`；`CheckpointDiff { words_delta, chapters_delta, tokens_delta, gate_weighted_delta }`；`compare_checkpoints(a, b) -> CheckpointDiff`；`AgencyBudget::tokens_used()` 采集进 metrics。

**里程碑：** `concept`（concept 完成）、`assets`（资产完成）、`chapter`（每章装配后，chapter_number 记录）、`run_final`（run 收尾）。
**metrics_json：** `{"chapters_done": n, "words_total": n, "gate_scores": [{"chapter": n, "weighted": f64}], "tokens_used": n, "elapsed_s": n}`。

- [ ] **Step 1: 写失败的测试**

`repository.rs` 测试追加：

```rust
#[test]
fn test_checkpoints_insert_list_compare() {
    let (repo, _) = repo();
    repo.create_run(&AgencyRun::new("cp-run", "前提")).unwrap();
    repo.set_run_story("cp-run", "s1").unwrap();
    let cp1 = crate::agency::coordinator::AgencyCheckpoint::new(
        "cp-run", "s1", "assets", None,
        serde_json::json!({"chapters_done": 0, "words_total": 0, "gate_scores": [], "tokens_used": 5000, "elapsed_s": 30}),
    );
    let cp2 = crate::agency::coordinator::AgencyCheckpoint::new(
        "cp-run", "s1", "chapter", Some(1),
        serde_json::json!({"chapters_done": 1, "words_total": 2100, "gate_scores": [{"chapter": 1, "weighted": 0.82}], "tokens_used": 42000, "elapsed_s": 180}),
    );
    repo.insert_checkpoint(&cp1).unwrap();
    repo.insert_checkpoint(&cp2).unwrap();
    let list = repo.list_checkpoints("s1").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].milestone, "assets");
    assert_eq!(list[1].chapter_number, Some(1));
    let diff = crate::agency::coordinator::compare_checkpoints(&cp1, &cp2);
    assert_eq!(diff.words_delta, 2100);
    assert_eq!(diff.chapters_delta, 1);
    assert_eq!(diff.tokens_delta, 37000);
    assert!((diff.gate_weighted_delta - 0.82).abs() < 0.001);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: FAIL（V110 表与方法未建）

- [ ] **Step 3: 实现**

`V110__agency_checkpoints.sql`：

```sql
-- V110: Agency 检查点（里程碑指标快照）
CREATE TABLE IF NOT EXISTS agency_checkpoints (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    milestone TEXT NOT NULL,
    chapter_number INTEGER,
    metrics_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_checkpoints_story ON agency_checkpoints(story_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agency_checkpoints_run ON agency_checkpoints(run_id, created_at);
```

`coordinator.rs`：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgencyCheckpoint {
    pub id: String,
    pub run_id: String,
    pub story_id: String,
    pub milestone: String,
    pub chapter_number: Option<i32>,
    pub metrics_json: String,
    pub created_at: String,
}

impl AgencyCheckpoint {
    pub fn new(run_id: &str, story_id: &str, milestone: &str, chapter_number: Option<i32>, metrics: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            milestone: milestone.to_string(),
            chapter_number,
            metrics_json: metrics.to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckpointDiff {
    pub words_delta: i64,
    pub chapters_delta: i64,
    pub tokens_delta: i64,
    pub gate_weighted_delta: f64,
}

pub fn compare_checkpoints(a: &AgencyCheckpoint, b: &AgencyCheckpoint) -> CheckpointDiff {
    let ma: serde_json::Value = serde_json::from_str(&a.metrics_json).unwrap_or_default();
    let mb: serde_json::Value = serde_json::from_str(&b.metrics_json).unwrap_or_default();
    let num = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let last_weighted = |v: &serde_json::Value| v.get("gate_scores")
        .and_then(|g| g.as_array())
        .and_then(|arr| arr.last())
        .and_then(|s| s.get("weighted"))
        .and_then(|w| w.as_f64())
        .unwrap_or(0.0);
    CheckpointDiff {
        words_delta: num(&mb, "words_total") - num(&ma, "words_total"),
        chapters_delta: num(&mb, "chapters_done") - num(&ma, "chapters_done"),
        tokens_delta: num(&mb, "tokens_used") - num(&ma, "tokens_used"),
        gate_weighted_delta: last_weighted(&mb) - last_weighted(&ma),
    }
}
```

`repository.rs`：`insert_checkpoint` / `list_checkpoints(story_id ORDER BY created_at, rowid)` / `get_checkpoint(id)` + map_checkpoint（模式同既有方法）。

`coordinator.rs` 钩子（helper + 调用点）：

```rust
async fn checkpoint(&self, run_id: &str, story_id: &str, milestone: &str,
    chapter_number: Option<i32>, metrics: serde_json::Value) {
    let cp = AgencyCheckpoint::new(run_id, story_id, milestone, chapter_number, metrics);
    let _ = self.db(move || {
        crate::agency::repository::AgencyRepository::new_placeholder().insert_checkpoint(&cp)
    }).await; // repository 由 pool 构造，模式同既有调用
}
```

调用点：concept 完成后（genesis）；assets 落库后（genesis/continue/batch 的 ensure_assets 后）；每章装配后（handle_gate 末尾，metrics 含该章 gate_score.weighted 与 budget.tokens_used()）；run_final（三入口外层 match Ok 分支，finish_run 后）。metrics 的 `tokens_used` 来自该 run 的 `Arc<AgencyBudget>`（把 budget 传入 handle_gate/钩子——签名已有）；`elapsed_s` 自 run created_at 起算；`gate_scores` 累计列表从 review 区 gate 条目读（self.db 查 item_type='gate' 的 content JSON 取 weighted——或直接参数传入当前章 weighted，累计列表查询获取）。

`commands.rs`：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_list_checkpoints(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<AgencyCheckpoint>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool).list_checkpoints(&story_id).map_err(AppError::from)
    }).await.map_err(|e| AppError::from(format!("list_checkpoints join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_compare_checkpoints(
    checkpoint_a: String,
    checkpoint_b: String,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::coordinator::CheckpointDiff, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<_, AppError> {
        let repo = crate::agency::repository::AgencyRepository::new(pool);
        let a = repo.get_checkpoint(&checkpoint_a).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed("checkpoint_a 不存在", None::<String>))?;
        let b = repo.get_checkpoint(&checkpoint_b).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed("checkpoint_b 不存在", None::<String>))?;
        Ok(crate::agency::coordinator::compare_checkpoints(&a, &b))
    }).await.map_err(|e| AppError::from(format!("compare join error: {}", e)))?
}
```

`handlers.rs` agency 分组追加两行注册。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 1+）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations/V110__agency_checkpoints.sql src-tauri/src/agency/ src-tauri/src/handlers.rs
git commit -m "feat(agency): milestone checkpoints (V110) with compare + IPC"
```

---

### Task 7: eval harness（JSON 场景 + pass@k/pass^k + baseline + CI 断言）

**Files:**
- Create: `evals/scenarios/gate-pass-basic.json`、`evals/scenarios/gate-revise-repetition.json`、`evals/scenarios/continue-completes.json`（场景定义）
- Create: `evals/baseline.json`
- Create: `src-tauri/src/agency/eval_harness.rs`
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod eval_harness;`）

**Interfaces:**
- Produces: `EvalScenario { id, description, seed, mock_llm, expect }`；`EvalOutcome { scenario_id, passed, details, duration_ms }`；`eval_harness::load_scenarios(dir) -> Vec<EvalScenario>`；`eval_harness::run_scenario(pool, scenario) -> EvalOutcome`（async）；`eval_harness::check_against_baseline(outcomes, baseline) -> Vec<String>`（回归失败清单）；CI 测试 `agency_evals_deterministic`（`#[tokio::test]`，读 `CARGO_MANIFEST_DIR/../evals`）。

**场景 JSON 格式：**

```json
{
  "id": "gate-revise-repetition",
  "description": "草稿高自重复 → 首轮修订后复审放行",
  "seed": {
    "story": {"title": "评估书", "genre": "科幻"},
    "characters": [{"name": "阿苔", "background": "拾荒者", "personality": "坚韧", "goals": "星环"}],
    "world": null,
    "scenes": []
  },
  "mock_llm": {
    "writer": [
      "{\"type\":\"tool\",\"name\":\"board_write\",\"args\":{\"zone\":\"draft\",\"item_type\":\"chapter\",\"key\":\"第1章\",\"content\":\"重复重复重复重复。\",\"summary\":\"一\"}}",
      "{\"type\":\"final\",\"content\":\"完成\"}",
      "{\"type\":\"final\",\"content\":\"已修订\"}"
    ],
    "editor": [
      "{\"type\":\"final\",\"content\":\"{\\\"verdict\\\":\\\"revise\\\",\\\"score\\\":2.0,\\\"blocking_issues\\\":[{\\\"issue\\\":\\\"自重复严重\\\",\\\"evidence\\\":\\\"重复重复\\\"}],\\\"suggestions\\\":[],\\\"comments\\\":\\\"修\\\"}\"}",
      "{\"type\":\"final\",\"content\":\"{\\\"verdict\\\":\\\"pass\\\",\\\"score\\\":4.5,\\\"blocking_issues\\\":[],\\\"suggestions\\\":[],\\\"comments\\\":\\\"好\\\"}\"}"
    ],
    "producer": []
  },
  "expect": {
    "flow": "continue",
    "chapter": 1,
    "revised": true,
    "run_status": "completed",
    "gate_outcomes": ["revise", "pass"],
    "min_gate_items": 2
  }
}
```

`gate-pass-basic.json`：editor 首轮 pass(score 4.5) → revised=false、gate_outcomes=["pass"]。`continue-completes.json`：两章 batch 简化场景（flow=batch, count=2, scenes_created=2）。

- [ ] **Step 1: 写失败的测试（harness 自测 + 场景断言）**

`eval_harness.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_scenarios_from_repo_dir() {
        let dir = evals_dir();
        let scenarios = load_scenarios(&dir).unwrap();
        assert!(scenarios.len() >= 3, "仓库应内置 ≥3 个场景: {}", scenarios.len());
        assert!(scenarios.iter().any(|s| s.id == "gate-pass-basic"));
    }

    #[tokio::test]
    async fn test_run_all_shipped_scenarios_deterministic() {
        // CI 断言：全部内置场景确定性通过（pass^1 回归门）
        let scenarios = load_scenarios(&evals_dir()).unwrap();
        let mut failures = Vec::new();
        for scenario in &scenarios {
            let pool = crate::db::create_test_pool().unwrap();
            let outcome = run_scenario(&pool, scenario).await;
            if !outcome.passed {
                failures.push(format!("{}: {}", outcome.scenario_id, outcome.details));
            }
        }
        assert!(failures.is_empty(), "eval 场景失败:\n{}", failures.join("\n"));
    }

    #[test]
    fn test_baseline_regression_detection() {
        let baseline: Baseline = serde_json::from_str(r#"{"gate-pass-basic": {"passed": true}}"#).unwrap();
        let outcomes = vec![
            EvalOutcome { scenario_id: "gate-pass-basic".into(), passed: false, details: "x".into(), duration_ms: 1 },
            EvalOutcome { scenario_id: "new-scenario".into(), passed: true, details: "y".into(), duration_ms: 1 },
        ];
        let regressions = check_against_baseline(&outcomes, &baseline);
        assert_eq!(regressions.len(), 1);
        assert!(regressions[0].contains("gate-pass-basic"));
        // 新场景不算回归
        assert!(!regressions.iter().any(|r| r.contains("new-scenario")));
    }

    #[test]
    fn test_pass_at_k_and_pass_pow_k() {
        assert!((pass_at_k(&[true, false, true], 3) - 1.0).abs() < 0.001); // 3 次至少 1 次过
        assert!((pass_at_k(&[false, false], 3) - 0.0).abs() < 0.001);
        assert!((pass_pow_k(&[true, true, true]) - 1.0).abs() < 0.001);
        assert!((pass_pow_k(&[true, false, true]) - 0.0).abs() < 0.001);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::eval_harness 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`src-tauri/src/agency/eval_harness.rs`（要点）：

```rust
//! Eval harness：JSON 场景 → 种子 DB + 角色队列 mock → 驱动 coordinator → 断言期望。
//! CI 跑确定性模式（随 cargo test --lib）；real-LLM 模式经 IPC（T9 agency_run_evals(live)）。

#[derive(Debug, serde::Deserialize)]
pub struct EvalScenario {
    pub id: String,
    pub description: String,
    pub seed: Seed,
    pub mock_llm: MockQueues,
    pub expect: Expect,
}

#[derive(Debug, serde::Deserialize)]
pub struct Seed {
    pub story: SeedStory,
    #[serde(default)]
    pub characters: Vec<SeedCharacter>,
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub scenes: Vec<SeedScene>,
}
// SeedStory { title, genre: Option<String> }
// SeedCharacter { name, background, personality, goals }
// SeedScene { sequence_number, title: Option<String>, content: String }

#[derive(Debug, serde::Deserialize)]
pub struct MockQueues {
    #[serde(default)]
    pub writer: Vec<String>,
    #[serde(default)]
    pub editor: Vec<String>,
    #[serde(default)]
    pub producer: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Expect {
    pub flow: String,              // continue | batch | genesis
    #[serde(default)]
    pub chapter: i32,              // continue 用
    #[serde(default)]
    pub count: usize,              // batch 用
    #[serde(default)]
    pub revised: Option<bool>,
    #[serde(default)]
    pub run_status: Option<String>,
    #[serde(default)]
    pub gate_outcomes: Vec<String>,
    #[serde(default)]
    pub min_gate_items: usize,
    #[serde(default)]
    pub scenes_created: Option<usize>,
}

pub struct EvalOutcome { pub scenario_id: String, pub passed: bool, pub details: String, pub duration_ms: u64 }
pub type Baseline = std::collections::HashMap<String, BaselineEntry>;
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BaselineEntry { pub passed: bool }

pub fn evals_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("evals")
}

pub fn load_scenarios(dir: &std::path::Path) -> Result<Vec<EvalScenario>, crate::error::AppError> {
    let mut out = Vec::new();
    let scenarios_dir = dir.join("scenarios");
    let mut entries: Vec<_> = std::fs::read_dir(&scenarios_dir)
        .map_err(|e| crate::error::AppError::from(format!("读场景目录失败 {}: {}", scenarios_dir.display(), e)))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let text = std::fs::read_to_string(entry.path()).map_err(crate::error::AppError::from)?;
        let scenario: EvalScenario = serde_json::from_str(&text)
            .map_err(|e| crate::error::AppError::from(format!("场景解析失败 {}: {}", entry.path().display(), e)))?;
        out.push(scenario);
    }
    Ok(out)
}

pub fn pass_at_k(results: &[bool], _k: usize) -> f64 {
    if results.iter().any(|r| *r) { 1.0 } else { 0.0 }
}

pub fn pass_pow_k(results: &[bool]) -> f64 {
    if results.iter().all(|r| *r) { 1.0 } else { 0.0 }
}

pub fn check_against_baseline(outcomes: &[EvalOutcome], baseline: &Baseline) -> Vec<String> {
    outcomes
        .iter()
        .filter(|o| baseline.get(&o.scenario_id).map(|b| b.passed).unwrap_or(false) && !o.passed)
        .map(|o| format!("回归: {} 曾通过现失败 ({})", o.scenario_id, o.details))
        .collect()
}
```

`run_scenario`：seed DB（stories/characters/world/scenes 直接 SQL，复用 T4/T5 测试种子模式）→ 按 `mock_llm` 三队列构造 RoutingMock（从 coordinator 测试模块提取为 `pub(crate) struct RoutingMock` 供复用——coordinator.rs 的测试 mock 上提为 `pub(crate)` 测试工具，或 eval_harness 内自带同构实现）→ `AgencyCoordinator::for_test(pool, mock)` → 按 expect.flow 调 run_continue/run_continue_batch → 断言（revised/run_status/scenes_created/gate 条目数与 outcome 序列）。断言失败信息进 details。

`evals/baseline.json`：

```json
{
  "gate-pass-basic": {"passed": true},
  "gate-revise-repetition": {"passed": true},
  "continue-completes": {"passed": true}
}
```

CI 集成：`test_run_all_shipped_scenarios_deterministic` 即 CI 门（随 cargo test --lib；`check_against_baseline` 在测试中同时跑并 assert 无回归）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency::eval_harness 2>&1 | tail -5`（4 新增）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add evals/ src-tauri/src/agency/eval_harness.rs src-tauri/src/agency/mod.rs src-tauri/src/agency/coordinator.rs
git commit -m "feat(agency): eval harness with json scenarios, pass@k/pass^k, baseline gate"
```

---

### Task 8: human 信号（用户修改率后置评分）

**Files:**
- Modify: `src-tauri/src/agency/graders.rs`（Jaccard 修改率 + 信号采集）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_human_signals`）
- Modify: `src-tauri/src/handlers.rs`（注册）

**Interfaces:**
- Produces: `graders::modification_ratio(delivered: &str, current: &str) -> f64`（1 - Jaccard(char bigrams)）；`HumanSignal { scene_id, chapter_number, delivered_chars, current_chars, modification_ratio, evaluated_at }`；`graders::human_signals(pool, story_id) -> Vec<HumanSignal>`（同步）。human grader 不进 gate（后置信号，ECC human 级）。

- [ ] **Step 1: 写失败的测试**

`graders.rs` 测试追加：

```rust
#[test]
fn test_modification_ratio() {
    assert_eq!(modification_ratio("完全一样", "完全一样"), 0.0);
    assert_eq!(modification_ratio("abc", "xyz"), 1.0);
    let r = modification_ratio("第一章的正文内容很长", "第一章的正文内容稍微有点长");
    assert!(r > 0.0 && r < 1.0, "部分修改: {}", r);
    assert_eq!(modification_ratio("", "非空"), 1.0);
    assert_eq!(modification_ratio("", ""), 0.0);
}

#[test]
fn test_human_signals_from_board_and_scene() {
    let pool = create_test_pool().unwrap();
    // 种子：run + draft 条目（第1章，content="原文"）+ scene(seq=1, content="原文改了一字")
    let repo = crate::agency::repository::AgencyRepository::new(pool.clone());
    repo.create_run(&crate::agency::models::AgencyRun::new("hs-1", "前提")).unwrap();
    repo.set_run_story("hs-1", "s1").unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    board.write("hs-1", "s1", crate::agency::models::AgentRole::LeadWriter,
        crate::agency::models::BoardZone::Draft, "chapter", "第1章", "原文内容", "一").unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '书', '2026-01-01', '2026-01-01')", []).unwrap();
    }
    let scene = crate::db::repositories::SceneRepository::new(pool.clone()).create("s1", 1, Some("第1章")).unwrap();
    crate::db::repositories::SceneRepository::new(pool.clone()).update(&scene.id, &crate::db::repositories::SceneUpdate {
        content: Some("原文内容改".to_string()), ..Default::default()
    }).unwrap();
    let signals = human_signals(&pool, "s1");
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].scene_id, scene.id);
    assert!(signals[0].modification_ratio > 0.0);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::graders 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`graders.rs` 追加：

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct HumanSignal {
    pub scene_id: String,
    pub chapter_number: i32,
    pub delivered_chars: usize,
    pub current_chars: usize,
    pub modification_ratio: f64,
    pub evaluated_at: String,
}

/// 1 - Jaccard(字符二元组)。0=未改，1=全改。
pub fn modification_ratio(delivered: &str, current: &str) -> f64 {
    fn bigrams(s: &str) -> std::collections::HashSet<(char, char)> {
        let chars: Vec<char> = s.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    }
    let a = bigrams(delivered);
    let b = bigrams(current);
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(&b).count() as f64;
    let union = a.union(&b).count() as f64;
    if union == 0.0 { 0.0 } else { 1.0 - inter / union }
}

/// 按 story 采集修改率（同步；调用方 spawn_blocking）。
/// delivered = 黑板 draft 区该章最新 active 条目 content；current = scenes.content 现值。
pub fn human_signals(pool: &DbPool, story_id: &str) -> Vec<HumanSignal> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // 每章：draft 条目（run 不限，同 story 最新 active）与 scene 配对
    let mut stmt = match conn.prepare(
        "SELECT b.key, b.content, s.id, s.content, s.sequence_number
         FROM agency_board_items b
         JOIN scenes s ON s.story_id = b.story_id AND s.sequence_number = CAST(substr(b.key, 2, length(b.key) - 2) AS INTEGER)
         WHERE b.story_id = ?1 AND b.zone = 'draft' AND b.status = 'active' AND b.item_type = 'chapter'
         GROUP BY s.sequence_number
         HAVING MAX(b.created_at)",
    ) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("human_signals prepare 失败: {}", e);
            return Vec::new();
        }
    };
    let rows = stmt.query_map(rusqlite::params![story_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i32>(4)?,
        ))
    });
    let mut out = Vec::new();
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            let (_key, delivered, scene_id, current, chapter_number) = row;
            let current = current.unwrap_or_default();
            out.push(HumanSignal {
                scene_id,
                chapter_number,
                delivered_chars: delivered.chars().count(),
                current_chars: current.chars().count(),
                modification_ratio: modification_ratio(&delivered, &current),
                evaluated_at: chrono::Local::now().to_rfc3339(),
            });
        }
    }
    out.sort_by_key(|s| s.chapter_number);
    out
}
```

注：`substr(b.key, 2, length(b.key)-2)` 提取"第N章"的 N（SQLite substr 1-based，字符单位）；若 JOIN 语义与实际 schema 有出入，改为 Rust 侧两查询后内存配对（先查 draft 条目，再查 scenes，按 parse_chapter_number(key) 匹配）——**优先 Rust 侧配对**（更稳），SQL JOIN 留作备选。

`commands.rs` + `handlers.rs`：`agency_human_signals(story_id)`（spawn_blocking 包装返回 Vec<HumanSignal>）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency::graders 2>&1 | tail -3`（新增 2）及全量。
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/graders.rs src-tauri/src/agency/commands.rs src-tauri/src/handlers.rs
git commit -m "feat(agency): human modification-ratio signals (jaccard on bigrams)"
```

---

### Task 9: 评估仪表盘前端 + 数据聚合 IPC + 发布 0.29.0

**Files:**
- Modify: `src-tauri/src/agency/commands.rs`（`agency_eval_overview`）
- Modify: `src-tauri/src/handlers.rs`（注册）
- Create: `src-frontend/src/services/api/agency.ts`
- Create: `src-frontend/src/pages/AgencyEval.tsx`
- Modify: `src-frontend/src/types/index.ts`（ViewType 加 'agency-eval'）、`src-frontend/src/App.tsx`（renderView 加 case）、`src-frontend/src/components/Sidebar.tsx`（NAV_GROUPS 诊断组加条目）
- Create: `src-frontend/src/pages/__tests__/AgencyEval.test.tsx`（或就近测试目录约定）
- Modify: 版本与文档（同 T7 惯例，0.28.0 → **0.29.0** 四处 + lockfile + ARCHITECTURE/PROJECT_STATUS/设计文档状态行/CHANGELOG）

**Interfaces:**
- Produces（后端）：`EvalOverview { gate_history: Vec<GateHistoryItem>, pass_rate: f64, checkpoints: Vec<AgencyCheckpoint>, human_signals: Vec<HumanSignal>, token_usage: Vec<PurposeUsage> }`；`GateHistoryItem { chapter: String, round: String, outcome: String, weighted: Option<f64>, code/rule/model: Option<f64>, created_at: String }`；`PurposeUsage { purpose: String, calls: i64, total_tokens: i64, total_duration_ms: i64 }`（llm_calls 按 purpose IN ('agency_writer','agency_producer','agency_editor') GROUP BY purpose 聚合）。
- Produces（前端）：`services/api/agency.ts` 导出 `getRun/listBoard/listCheckpoints/compareCheckpoints/humanSignals/evalOverview/runEvals` 封装；页面路由 'agency-eval'。

- [ ] **Step 1: 后端聚合 + 测试**

`commands.rs` 追加：

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct GateHistoryItem {
    pub key: String,
    pub outcome: String,
    pub weighted: Option<f64>,
    pub code: Option<f64>,
    pub rule: Option<f64>,
    pub model: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PurposeUsage {
    pub purpose: String,
    pub calls: i64,
    pub total_tokens: i64,
    pub total_duration_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvalOverview {
    pub gate_history: Vec<GateHistoryItem>,
    pub pass_rate: f64,
    pub checkpoints: Vec<crate::agency::coordinator::AgencyCheckpoint>,
    pub human_signals: Vec<crate::agency::graders::HumanSignal>,
    pub token_usage: Vec<PurposeUsage>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_eval_overview(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<EvalOverview, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<EvalOverview, AppError> {
        let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
        // gate 历史（review 区 item_type='gate'）
        let mut stmt = conn.prepare(
            "SELECT key, content, created_at FROM agency_board_items
             WHERE story_id = ?1 AND item_type = 'gate' ORDER BY created_at ASC, rowid ASC")?;
        let mut pass = 0usize;
        let mut total = 0usize;
        let gate_history: Vec<GateHistoryItem> = stmt.query_map(rusqlite::params![story_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?.filter_map(|r| r.ok()).map(|(key, content, created_at)| {
            let json: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            let outcome = json.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let gs = json.get("gate_score");
            let f = |k: &str| gs.and_then(|g| g.get(k)).and_then(|v| v.as_f64());
            if outcome == "pass" { pass += 1; }
            total += 1;
            GateHistoryItem {
                key,
                outcome,
                weighted: f("weighted"),
                code: f("code"),
                rule: f("rule"),
                model: f("model"),
                created_at,
            }
        }).collect();
        let pass_rate = if total == 0 { 0.0 } else { pass as f64 / total as f64 };
        // token 用量（llm_calls purpose 聚合）
        let mut usage_stmt = conn.prepare(
            "SELECT purpose, COUNT(*), SUM(total_tokens), SUM(duration_ms)
             FROM llm_calls WHERE purpose IN ('agency_writer','agency_producer','agency_editor')
             GROUP BY purpose")?;
        let token_usage: Vec<PurposeUsage> = usage_stmt.query_map([], |r| {
            Ok(PurposeUsage {
                purpose: r.get(0)?,
                calls: r.get(1)?,
                total_tokens: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                total_duration_ms: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })?.filter_map(|r| r.ok()).collect();
        let checkpoints = crate::agency::repository::AgencyRepository::new(pool.clone())
            .list_checkpoints(&story_id).map_err(AppError::from)?;
        let human_signals = crate::agency::graders::human_signals(&pool, &story_id);
        Ok(EvalOverview { gate_history, pass_rate, checkpoints, human_signals, token_usage })
    }).await.map_err(|e| AppError::from(format!("eval_overview join error: {}", e)))?
}
```

`handlers.rs` 注册。Rust 侧测试：overview 聚合逻辑在 create_test_pool 种子数据上断言（gate 条目两条一 pass 一 revise → pass_rate=0.5；usage 聚合作空表容忍）。

- [ ] **Step 2: 前端封装与页面**

`src-frontend/src/services/api/agency.ts`：

```ts
import { loggedInvoke } from './core';

export interface GateHistoryItem {
  key: string;
  outcome: string;
  weighted: number | null;
  code: number | null;
  rule: number | null;
  model: number | null;
  created_at: string;
}

export interface PurposeUsage {
  purpose: string;
  calls: number;
  total_tokens: number;
  total_duration_ms: number;
}

export interface AgencyCheckpoint {
  id: string;
  run_id: string;
  story_id: string;
  milestone: string;
  chapter_number: number | null;
  metrics_json: string;
  created_at: string;
}

export interface HumanSignal {
  scene_id: string;
  chapter_number: number;
  delivered_chars: number;
  current_chars: number;
  modification_ratio: number;
  evaluated_at: string;
}

export interface EvalOverview {
  gate_history: GateHistoryItem[];
  pass_rate: number;
  checkpoints: AgencyCheckpoint[];
  human_signals: HumanSignal[];
  token_usage: PurposeUsage[];
}

export interface CheckpointDiff {
  words_delta: number;
  chapters_delta: number;
  tokens_delta: number;
  gate_weighted_delta: number;
}

export function getEvalOverview(storyId: string) {
  return loggedInvoke<EvalOverview>('agency_eval_overview', { story_id: storyId });
}

export function listCheckpoints(storyId: string) {
  return loggedInvoke<AgencyCheckpoint[]>('agency_list_checkpoints', { story_id: storyId });
}

export function compareCheckpoints(checkpointA: string, checkpointB: string) {
  return loggedInvoke<CheckpointDiff>('agency_compare_checkpoints', {
    checkpoint_a: checkpointA,
    checkpoint_b: checkpointB,
  });
}

export function getHumanSignals(storyId: string) {
  return loggedInvoke<HumanSignal[]>('agency_human_signals', { story_id: storyId });
}
```

`src-frontend/src/pages/AgencyEval.tsx`（完整页面，SVG 趋势图复用 ReadingPowerChart 模式）：

```tsx
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { getEvalOverview } from '@/services/api/agency';
import type { GateHistoryItem } from '@/services/api/agency';

function weightedOf(item: GateHistoryItem): number | null {
  return item.weighted;
}

function GateTrendChart({ data }: { data: GateHistoryItem[] }) {
  const points = data.filter(d => d.weighted != null);
  if (points.length === 0) return <p className="text-sm text-gray-500">暂无评分数据</p>;
  const w = 560;
  const h = 160;
  const pad = 28;
  const maxX = Math.max(points.length - 1, 1);
  const x = (i: number) => pad + (i / maxX) * (w - pad * 2);
  const y = (v: number) => h - pad - v * (h - pad * 2);
  const pathD = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(p.weighted!).toFixed(1)}`)
    .join(' ');
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full max-w-2xl">
      <line x1={pad} y1={y(0.75)} x2={w - pad} y2={y(0.75)} stroke="#f59e0b" strokeDasharray="4" />
      <text x={w - pad + 2} y={y(0.75)} fontSize="10" fill="#f59e0b">0.75</text>
      <path d={pathD} fill="none" stroke="#6366f1" strokeWidth="2" />
      {points.map((p, i) => (
        <circle key={i} cx={x(i)} cy={y(p.weighted!)} r="3"
          fill={p.outcome === 'pass' ? '#22c55e' : p.outcome === 'revise' ? '#f59e0b' : '#ef4444'} />
      ))}
    </svg>
  );
}

export default function AgencyEval() {
  const currentStory = useAppStore(s => s.currentStory);
  const [storyId] = useState(currentStory?.id ?? '');
  const { data, isLoading, error } = useQuery({
    queryKey: ['agency-eval-overview', storyId],
    queryFn: () => getEvalOverview(storyId),
    enabled: !!storyId,
    staleTime: 30_000,
  });

  if (!currentStory) return <p className="p-6 text-gray-500">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载评估数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-xl font-semibold">创作评估 · {currentStory.title}</h1>
      <div className="grid grid-cols-3 gap-4">
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">质量门通过率</div>
          <div className="text-2xl font-bold">{(data.pass_rate * 100).toFixed(0)}%</div>
          <div className="text-xs text-gray-400">{data.gate_history.length} 次判定</div>
        </div>
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">检查点</div>
          <div className="text-2xl font-bold">{data.checkpoints.length}</div>
          <div className="text-xs text-gray-400">里程碑快照</div>
        </div>
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">Human 信号</div>
          <div className="text-2xl font-bold">
            {data.human_signals.length === 0
              ? '—'
              : `${(data.human_signals.reduce((a, s) => a + s.modification_ratio, 0) / data.human_signals.length * 100).toFixed(0)}%`}
          </div>
          <div className="text-xs text-gray-400">平均修改率</div>
        </div>
      </div>

      <section>
        <h2 className="mb-2 font-medium">Gate 加权分趋势（阈值 0.75）</h2>
        <GateTrendChart data={data.gate_history} />
      </section>

      <section>
        <h2 className="mb-2 font-medium">判定历史</h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500">
              <th>条目</th><th>结果</th><th>加权</th><th>code</th><th>rule</th><th>model</th><th>时间</th>
            </tr>
          </thead>
          <tbody>
            {data.gate_history.map(g => (
              <tr key={g.key + g.created_at} className="border-t">
                <td>{g.key}</td>
                <td>{g.outcome}</td>
                <td>{g.weighted?.toFixed(2) ?? '—'}</td>
                <td>{g.code?.toFixed(2) ?? '—'}</td>
                <td>{g.rule?.toFixed(2) ?? '—'}</td>
                <td>{g.model?.toFixed(2) ?? '—'}</td>
                <td className="text-gray-400">{g.created_at.slice(0, 16)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section>
        <h2 className="mb-2 font-medium">Agency token 用量（按角色）</h2>
        <table className="w-full text-sm">
          <thead><tr className="text-left text-gray-500"><th>角色</th><th>调用</th><th>总 tokens</th><th>总耗时(ms)</th></tr></thead>
          <tbody>
            {data.token_usage.map(u => (
              <tr key={u.purpose} className="border-t">
                <td>{u.purpose.replace('agency_', '')}</td>
                <td>{u.calls}</td>
                <td>{u.total_tokens}</td>
                <td>{u.total_duration_ms}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
```

注册三处：
- `types/index.ts` ViewType 加 `| 'agency-eval'`（放 'usage-stats' 后，带注释 `/** Agency 创作评估（质量门/检查点/human 信号） */`）；
- `App.tsx` import `AgencyEval` + renderView 加 `case 'agency-eval': return <AgencyEval />;`；
- `Sidebar.tsx` NAV_GROUPS 诊断组 items 加 `{ id: 'agency-eval', label: '创作评估', icon: Gauge, impact: 'warm' }`（Gauge 从 lucide-react import——若无 Gauge 用 Activity，以现有 import 清单为准）。

前端测试（`src-frontend/src/pages/__tests__/AgencyEval.test.tsx`，目录约定以现有页面测试为准——若无该目录则放 `AgencyEval.test.tsx` 同目录）：

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/services/api/agency', () => ({
  getEvalOverview: vi.fn().mockResolvedValue({
    gate_history: [
      { key: 'gate-第1章-r1', outcome: 'pass', weighted: 0.82, code: 0.9, rule: 0.8, model: 0.8, created_at: '2026-07-17T10:00' },
      { key: 'gate-第2章-r1', outcome: 'revise', weighted: 0.6, code: 0.8, rule: 0.5, model: 0.5, created_at: '2026-07-17T11:00' },
    ],
    pass_rate: 0.5,
    checkpoints: [],
    human_signals: [],
    token_usage: [{ purpose: 'agency_writer', calls: 4, total_tokens: 8000, total_duration_ms: 3000 }],
  }),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: any) => sel({ currentStory: { id: 's1', title: '评估书' } }),
}));

import AgencyEval from '../AgencyEval';

describe('AgencyEval', () => {
  it('渲染通过率与判定历史', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AgencyEval />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('50%')).toBeInTheDocument();
    expect(await screen.findByText('gate-第1章-r1')).toBeInTheDocument();
    expect(await screen.findByText('writer')).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: 全量验证 + 发布**

`cargo test --lib` 全绿；`cd src-frontend && npx vitest run`（292 + 1 新增）全绿；`npm run type-check` 通过；`npm run build` 通过；`python3 scripts/architecture_guard.py` 通过。

版本 0.28.0 → 0.29.0（四处 + `npm install --package-lock-only` + Cargo.lock 自动）。文档：ARCHITECTURE（P4 小节：grader 四级/Gate v2/检查点/eval harness/仪表盘）、PROJECT_STATUS、设计文档状态行 "P1-P4 已完成"、CHANGELOG 0.29.0 条目：

```markdown
## v0.29.0（2026-07-17）

### Agency P4：验证循环
- 四级 grader：code（字数/自重复/合同禁则）→ rule（合同兑现/追读力/规则复检）→ model（rubric 化编辑裁决 1-5 须引证据）→ human（用户修改率后置信号）
- Gate v2 统一加权评分（0.2/0.3/0.5，阈值 0.75）取代二元判定
- V110 检查点：里程碑指标快照 + 现在 vs 当时对比（IPC）
- eval harness：JSON 场景 + pass@k/pass^k + baseline 回归门（随 cargo test 纳入 CI）
- 评估仪表盘前端页（通过率/加权分趋势/判定历史/角色 token 用量）
- migration runner 按最高版本选目（修复陈旧副本遮蔽）；resume 改 spawn 模式
```

Commit：`release: v0.29.0 agency verification loop (P4)`。

**真机验收（用户执行）：**
1. 创世/续写后打开侧栏"创作评估"页：通过率、加权分趋势、判定历史、token 用量四项有数据。
2. 修改第一章正文后刷新：Human 信号平均修改率 > 0。
3. 仪表盘检查点对比：选两个 checkpoint 看 delta。

---

## Self-Review（计划自审结论）

- **Spec coverage**：设计 P4 行（grader 分级/eval harness/检查点/评估仪表盘）→ T3/T4/T5（grader+Gate v2）、T7（harness）、T6（检查点）、T9（仪表盘）；ECC grader 四级映射逐行对应（code/rule/model/human → T3/T3/T4/T8）；pass@k/pass^k/baseline → T7；检查点对比 → T6；P3 终审转项（migration runner/resume spawn/finalize 顺序/种子模板）→ T1/T2。
- **Placeholder scan**：T3 的 `reading_power_score_of` 与 trim_self_repetition、block_in_place 已标注两条实现路径与判定方法（实现时按实际结构落地，非占位）；T5 的 contract/hints 获取沿用现有模式；T5 Step 1 的示意行已注明删除。无 TBD。
- **Type consistency**：`GateScore`（T5 定义）→ T6 metrics/T9 overview 一致；`AgencyCheckpoint/CheckpointDiff`（T6）→ T9 TS 镜像一致；`HumanSignal`（T8）→ T9 overview/TS 一致；`EvalScenario/EvalOutcome/Baseline`（T7）→ baseline.json/场景 JSON 一致；`ModelGraderReport`（T4）→ T5 消费一致；`parse_chapter_number`（T3）→ T5/T8 复用。
- **风险备案**：run_rule_grader 的 async 化与 block_in_place 已二选一并给判定；editor rubric 化向后兼容（旧格式回退分）有测试；eval harness 的 real-LLM 模式依赖 AppHandle，P4 仅交付确定性模式（real 模式待 P5 经 IPC）。前端零图表依赖（手绘 SVG）。
