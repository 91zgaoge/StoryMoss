# Agency P5：持续学习（双轨）+ 代理可视化收尾 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 ECC 式持续学习双轨（观察层 observations.jsonl → 后台 analyzer 提炼 instinct → 置信度引擎 → 晋升物化为技能，人工确认生效）与完整代理可视化（学习中心 + 代理工作室），并完成 P4 转项小批次（eval CI step、checkpoint 对比 UI、story 级 token 聚合、追读力口径统一）。

**Architecture:** 学习数据全部走 `.storymoss/learning/` 文件层（git 自动提交范围内，双轨制的文件轨）：`observations.jsonl`（追加、10MB 轮转、防自观察）→ analyzer（Background 档 LLM）→ `instincts/*.md`（YAML frontmatter：trigger/action/confidence/evidence_count/scope/status）→ 晋升候选（≥0.8 且跨 story 复现）→ 用户确认后物化为 `skill.yaml` 目录技能（`import_skill` 持久化）+ 启动 `reload_skills()` 补现状缺口。前端两新页复用 AgencyEval 的 useQuery + listen 模式，注册走既有三处。

**Tech Stack:** Rust（Tauri 2.4）、rusqlite + r2d2、tokio、serde/serde_json/serde_yaml、async-trait、React 18 + TS + Zustand + TanStack Query、@tauri-apps/api/event。

**设计文档:** `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（P5 行 + ECC 持续学习映射；前端完整可视化 §8）。
**P4 终审转项：** eval 专用 CI step；checkpoint 对比 UI；story 级 token 聚合（经 agency_checkpoints，llm_calls 启动归零不可作跨会话真源）；追读力口径统一。

## Global Constraints

- 测试基线必须保持绿：`cargo test --lib`（现 855 passed + 2 ignored）与 `cd src-frontend && npx vitest run`（293 passed）。
- 所有 DB 同步调用在 async 上下文中必须 `tokio::task::spawn_blocking` / `self.db` 包裹。
- 已核实接口事实：`crate::error::AppError`；`AppError::validation_failed(msg, None::<String>)`；`AppError::from(String)`；测试内存库 `crate::db::create_test_pool()`。
- WorkspaceService：`new(app: &AppHandle, pool)`（workspace/mod.rs:34）；**测试绕过 AppHandle 的先例**——测试模块内直接构造私有字段 `WorkspaceService { app_dir: tmp.path().to_path_buf(), pool }`（mod.rs:747-767）；`git_commit_sync` 对 `.storymoss` 整目录提交，learning/ 子目录天然在范围内；`get_workspace_file(story_id, "learning/observations.jsonl")` 子目录文件名现有命令即可读（commands/workspace.rs）。
- 技能系统：目录格式 `skill.yaml` + `main.prompt`（system/user 以 `---` 分隔，loader.rs:104-125）；`SkillManager::import_skill(path)` 拷到 `skills_dir/<id>`（data_dir/cinema-ai/skills，mod.rs:260-265）；registry 纯内存、启动不 reload——本计划修复：lib.rs setup 调 `reload_skills()`；`save_override` 要求 prompt_id 已在内置 registry（agency 三个角色 prompt 已注册，可用）。
- frontmatter：唯一解析器 `split_frontmatter`（prompts/registry.rs:354，私有）——本计划提为 `pub(crate)` 复用；serde_yaml 已是依赖。
- LLM 计量：`complete_metered` 返回 `(content, tokens_used, cost)`；防自观察标签 `agency_observer`（ObservationLogger::should_record 过滤）。
- 版本号四文件 + 双 lockfile：本计划发布 **0.30.0**。
- P1-P4 行为不得回退：终态守护、Gate v2（含 spec 5.5）、spawn_blocking、角色模型路由（analyzer 用 Background 档）。
- Commit 用 Conventional Commits。

---

### Task 1: 观察层（ObservationLogger + 四类埋点）

**Files:**
- Create: `src-tauri/src/agency/learning.rs`
- Modify: `src-tauri/src/agency/mod.rs`（`pub mod learning;`）
- Modify: `src-tauri/src/agency/coordinator.rs`（complete_metered/gate/revision 埋点；AgencyLlm 加 story_id）
- Modify: `src-tauri/src/scene_commands.rs`（user_edit 埋点）

**Interfaces:**
- Consumes: WorkspaceService 目录约定（`{app_dir}/stories/{story_id}/.storymoss/learning/`）。
- Produces: `learning::Observation { ts, story_id, kind, actor, payload }`（serde）；`learning::ObservationLogger::new(app_dir: PathBuf)`；`log(story_id, kind, actor, payload)`（同步、best-effort）；`recent(story_id, n) -> Vec<Observation>`；`count_unanalyzed(story_id) -> usize`（经 `analyzer_state.json` 的 `analyzed_through: usize` 行号）；`should_record(context_label) -> bool`（`!label.contains("agency_observer")`）；kind 枚举字符串：`"llm_call" | "gate" | "revision" | "user_edit" | "promotion"`。`AgencyLlm::new(app_handle, run_id, role, story_id)`（**签名加 story_id**，全部调用点同步）。T2 的 analyzer 消费 `recent/count_unanalyzed`。

**观察记录格式（JSONL 每行一条）：**
```json
{"ts":"2026-07-18T10:00:00+08:00","story_id":"s1","kind":"llm_call","actor":"lead_writer","payload":{"model":"gpt-x","tokens":1234,"cost":0.002,"task":"CreativeWriting","aborted":false}}
```
payload 值一律截断 ≤500 字符（脱敏：不记 content/prompt 正文，只记元数据）。

- [ ] **Step 1: 写失败的测试**

`learning.rs`（测试模块先行，用 tempdir）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn logger() -> (ObservationLogger, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        (ObservationLogger::new(tmp.path().to_path_buf()), tmp)
    }

    #[test]
    fn test_log_and_recent() {
        let (logger, _tmp) = logger();
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "pass", "weighted": 0.82}));
        logger.log("s1", "llm_call", "lead_writer", serde_json::json!({"tokens": 100}));
        logger.log("s2", "gate", "editor_auditor", serde_json::json!({"outcome": "revise"}));
        let recent = logger.recent("s1", 10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].kind, "gate");
        assert_eq!(recent[1].kind, "llm_call");
        assert_eq!(recent[0].payload["weighted"].as_f64().unwrap(), 0.82);
        assert_eq!(logger.recent("s2", 10).len(), 1);
    }

    #[test]
    fn test_count_unanalyzed() {
        let (logger, _tmp) = logger();
        for i in 0..5 {
            logger.log("s1", "gate", "editor_auditor", serde_json::json!({"i": i}));
        }
        assert_eq!(logger.count_unanalyzed("s1"), 5);
        logger.mark_analyzed("s1").unwrap();
        assert_eq!(logger.count_unanalyzed("s1"), 0);
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"i": 9}));
        assert_eq!(logger.count_unanalyzed("s1"), 1);
    }

    #[test]
    fn test_should_record() {
        assert!(ObservationLogger::should_record("agency_writer"));
        assert!(!ObservationLogger::should_record("agency_observer"));
        assert!(!ObservationLogger::should_record("agency_observer_analyzer"));
    }

    #[test]
    fn test_rotation_keeps_tail() {
        let (logger, tmp) = logger();
        let path = logger.observations_path("s1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let big_line = serde_json::json!({"ts":"t","story_id":"s1","kind":"gate","actor":"x","payload":{"blob":"x".repeat(1000)}}).to_string();
        // 写超过阈值（测试用小阈值注入）
        let mut content = String::new();
        for _ in 0..200 {
            content.push_str(&big_line);
            content.push('\n');
        }
        std::fs::write(&path, &content).unwrap();
        logger.rotate_if_needed(&path, 10_000).unwrap(); // 测试阈值 10KB
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.len() <= 10_000 + 1200, "轮转后应接近阈值: {}", after.len());
        assert!(after.ends_with('\n'));
        // 保留的是尾部行
        let lines: Vec<&str> = after.lines().collect();
        assert!(!lines.is_empty());
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "gate");
    }

    #[test]
    fn test_payload_truncation() {
        let (logger, _tmp) = logger();
        let long = "长".repeat(2000);
        logger.log("s1", "user_edit", "human", serde_json::json!({"note": long}));
        let recent = logger.recent("s1", 1);
        let note = recent[0].payload["note"].as_str().unwrap();
        assert!(note.chars().count() <= 520, "payload 应截断: {}", note.len());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::learning 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`src-tauri/src/agency/learning.rs`：

```rust
//! 持续学习·观察层（ECC observe 模式）：四类观察点 → .storymoss/learning/observations.jsonl。
//! 双轨制的文件轨：JSONL 追加写、10MB 轮转、防自观察、payload 截断脱敏。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const LEARNING_DIR: &str = "learning";
pub const OBSERVATIONS_FILE: &str = "observations.jsonl";
pub const ANALYZER_STATE_FILE: &str = "analyzer_state.json";
pub const INSTINCTS_DIR: &str = "instincts";
const ROTATE_BYTES: u64 = 10 * 1024 * 1024; // 10MB
const PAYLOAD_MAX_CHARS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub ts: String,
    pub story_id: String,
    pub kind: String,
    pub actor: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct ObservationLogger {
    app_dir: PathBuf,
}

impl ObservationLogger {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }

    pub fn should_record(context_label: &str) -> bool {
        !context_label.contains("agency_observer")
    }

    pub fn observations_path(&self, story_id: &str) -> PathBuf {
        self.app_dir
            .join("stories")
            .join(story_id)
            .join(".storymoss")
            .join(LEARNING_DIR)
            .join(OBSERVATIONS_FILE)
    }

    fn instincts_path(&self, story_id: &str) -> PathBuf {
        self.app_dir
            .join("stories")
            .join(story_id)
            .join(".storymoss")
            .join(LEARNING_DIR)
            .join(INSTINCTS_DIR)
    }

    fn state_path(&self, story_id: &str) -> PathBuf {
        self.observations_path(story_id)
            .parent()
            .unwrap()
            .join(ANALYZER_STATE_FILE)
    }

    /// 追加一条观察（同步、best-effort：任何 IO 错误只 warn 不传播）。
    pub fn log(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) {
        if let Err(e) = self.log_inner(story_id, kind, actor, payload) {
            log::warn!("observation log 失败（忽略）: {}", e);
        }
    }

    fn log_inner(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) -> Result<(), String> {
        let path = self.observations_path(story_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        self.rotate_if_needed(&path, ROTATE_BYTES)?;
        let observation = Observation {
            ts: chrono::Local::now().to_rfc3339(),
            story_id: story_id.to_string(),
            kind: kind.to_string(),
            actor: actor.to_string(),
            payload: truncate_payload(payload),
        };
        let line = serde_json::to_string(&observation).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 10MB 轮转：保留尾部 ~max_bytes（按行对齐）。
    pub fn rotate_if_needed(&self, path: &Path, max_bytes: u64) -> Result<(), String> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        if meta.len() <= max_bytes {
            return Ok(());
        }
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let keep_from = content.len().saturating_sub(max_bytes as usize);
        // 对齐到行边界
        let aligned = content[keep_from..]
            .find('\n')
            .map(|i| keep_from + i + 1)
            .unwrap_or(keep_from);
        let tail = &content[aligned.min(content.len())..];
        std::fs::write(path, tail).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn recent(&self, story_id: &str, n: usize) -> Vec<Observation> {
        let path = self.observations_path(story_id);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut items: Vec<Observation> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if items.len() > n {
            items = items.split_off(items.len() - n);
        }
        items
    }

    pub fn count_unanalyzed(&self, story_id: &str) -> usize {
        let total = self.observations_path(story_id)
            .exists()
            .then(|| {
                std::fs::read_to_string(self.observations_path(story_id))
                    .map(|c| c.lines().count())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let analyzed = self.analyzed_through(story_id);
        total.saturating_sub(analyzed)
    }

    fn analyzed_through(&self, story_id: &str) -> usize {
        let path = self.state_path(story_id);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .and_then(|v| v.get("analyzed_through").and_then(|x| x.as_u64()))
            .map(|x| x as usize)
            .unwrap_or(0)
    }

    pub fn mark_analyzed(&self, story_id: &str) -> Result<(), crate::error::AppError> {
        let total = self.observations_path(story_id)
            .exists()
            .then(|| {
                std::fs::read_to_string(self.observations_path(story_id))
                    .map(|c| c.lines().count())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let path = self.state_path(story_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
        }
        let state = serde_json::json!({
            "analyzed_through": total,
            "last_analysis_ts": chrono::Local::now().to_rfc3339(),
        });
        std::fs::write(&path, state.to_string()).map_err(crate::error::AppError::from)?;
        Ok(())
    }
}

fn truncate_payload(payload: serde_json::Value) -> serde_json::Value {
    match payload {
        serde_json::Value::Object(map) => {
            let truncated = map
                .into_iter()
                .map(|(k, v)| {
                    let v = match &v {
                        serde_json::Value::String(s) if s.chars().count() > PAYLOAD_MAX_CHARS => {
                            serde_json::Value::String(format!("{}…(截断)", s.chars().take(PAYLOAD_MAX_CHARS).collect::<String>()))
                        }
                        other => other.clone(),
                    };
                    (k, v)
                })
                .collect();
            serde_json::Value::Object(truncated)
        }
        other => other,
    }
}
```

埋点（均 best-effort，调用方包 spawn_blocking 或 fire-and-forget）：

1. **llm_call**（`AgencyLlm::complete_metered`）：`AgencyLlm` 加 `story_id: String` 字段（`new(app_handle, run_id, role, story_id)` 签名变更，全部调用点同步——run_role_loop / evaluate_gate_impl / concept / finalize / gate_runner）；末尾成功时：
```rust
if ObservationLogger::should_record(&label) {
    let app_dir = self.app_handle.path().app_data_dir().ok();
    if let Some(dir) = app_dir {
        let logger = ObservationLogger::new(dir);
        let sid = self.story_id.clone();
        let role = self.role.as_str().to_string();
        tokio::spawn(async move {
            logger.log(&sid, "llm_call", &role, serde_json::json!({
                "model": model, "tokens": tokens, "cost": cost, "task": format!("{:?}", task),
            }));
        });
    }
}
```
（`complete` 与 `complete_metered` 双路径；label 即 context_label。）
2. **gate**（`record_gate_impl`）：outcome/kind/weighted/key 元数据落观察（`self` 无 story_id 问题——record_gate_impl 参数有 story_id；logger 的 app_dir 从 board 持有的 app_handle 或参数传入——若 record_gate_impl 无 app_handle，经 coordinator 调用点补一行：coordinator 有 app_handle，gate 判定后在 evaluate_gate 外层埋点更稳：`self.log_observation(story_id, "gate", "editor_auditor", json)`，helper 内部从 self.app_handle 取 app_dir，无则跳过）。
3. **revision**（`handle_gate` 修订分支，与 bus.send 同点）：`{"chapter": n, "issues_count": issues.len()}`。
4. **user_edit**（`scene_commands.rs` update_scene 的 `if let Some(ref story_id)` 块内，content_changed 时）：app_dir 经 `app_handle.path().app_data_dir()`，`{"scene_id": scene_id, "word_count": word_count}`。注意该路径由人类编辑触发（SceneUpdate.content.is_some() 且 `source` 非 agency——若 SceneUpdate.source == Some("agency") 跳过，防自观察）。

coordinator 辅助：

```rust
fn log_observation(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) {
    let Some(app) = &self.app_handle else { return };
    let Ok(dir) = app.path().app_data_dir() else { return };
    let logger = crate::agency::learning::ObservationLogger::new(dir);
    let sid = story_id.to_string();
    let kind = kind.to_string();
    let actor = actor.to_string();
    tokio::spawn(async move {
        logger.log(&sid, &kind, &actor, payload);
    });
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 5）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/learning.rs src-tauri/src/agency/mod.rs src-tauri/src/agency/coordinator.rs src-tauri/src/scene_commands.rs
git commit -m "feat(agency): observation layer with jsonl log, rotation, self-observation guards"
```

---

### Task 2: analyzer（后台分析 → instinct 生成）

**Files:**
- Modify: `src-tauri/src/agency/learning.rs`（`analyze_story` + instinct 写入）
- Modify: `src-tauri/src/agency/coordinator.rs`（观察累计 ≥20 自动触发后台分析）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_analyze_learning`）
- Modify: `src-tauri/src/handlers.rs`（注册）

**Interfaces:**
- Consumes: T1 的 `ObservationLogger::{recent, count_unanalyzed, mark_analyzed}`；`LoopLlm`（Background 档——`AgencyLlm` role=EditorAuditor 即可，label `agency_observer` 使 analyzer 自身调用被防自观察过滤）。
- Produces: `learning::Instinct { id, trigger, action, confidence, evidence_count, scope, status, created_at, updated_at, evolved_from }`；`learning::analyze_story(llm, logger, story_id) -> Result<AnalyzeOutcome, AppError>`（async）；`AnalyzeOutcome { new_instincts: usize, updated_instincts: usize, analyzed: usize }`；instinct 文件 `.storymoss/learning/instincts/<id>.md`（YAML frontmatter + 正文）；IPC `agency_analyze_learning(story_id) -> AnalyzeOutcome`；自动触发阈值 `ANALYZE_THRESHOLD: usize = 20`。

**instinct 文件格式（逐字模板）：**
```markdown
---
id: inst-<yyyymmdd>-<6位hex>
trigger: "触发条件（一句话，从观察模式归纳）"
action: "建议动作（一句话，可操作的创作指导）"
confidence: 0.3
evidence_count: 1
scope: story
status: pending
created_at: "<rfc3339>"
updated_at: "<rfc3339>"
evolved_from: ["gate", "revision"]
---

## 模式描述
（2-3 句：观察到的重复模式）

## 证据摘要
（最近 3 条相关观察的一句话概括）
```

- [ ] **Step 1: 写失败的测试**

`learning.rs` 测试模块追加：

```rust
struct MockAnalyzerLlm {
    response: String,
}

#[async_trait::async_trait]
impl crate::agency::tool_loop::LoopLlm for MockAnalyzerLlm {
    async fn complete(&self, _s: &str, _u: &str, _t: crate::router::TaskType, _m: i32) -> Result<String, crate::error::AppError> {
        Ok(self.response.clone())
    }
}

fn analyzer_mock() -> std::sync::Arc<MockAnalyzerLlm> {
    std::sync::Arc::new(MockAnalyzerLlm {
        response: r#"```yaml
- trigger: "当编辑审计连续两轮判定 revise"
  action: "修订前先复读资产区角色卡与大纲"
  evolved_from: ["gate", "revision"]
```"#.to_string(),
    })
}

#[tokio::test]
async fn test_analyze_creates_instinct_files() {
    let (logger, _tmp) = logger();
    for i in 0..3 {
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "revise", "i": i}));
    }
    let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
    assert_eq!(outcome.new_instincts, 1);
    assert_eq!(outcome.analyzed, 3);
    assert_eq!(logger.count_unanalyzed("s1"), 0);
    let instincts = list_instincts(&logger, "s1").unwrap();
    assert_eq!(instincts.len(), 1);
    let inst = &instincts[0];
    assert!(inst.trigger.contains("连续两轮"));
    assert!(inst.action.contains("复读"));
    assert!((inst.confidence - 0.3).abs() < 0.001); // evidence_count=1 → 0.3
    assert_eq!(inst.status, "pending");
    assert_eq!(inst.scope, "story");
}

#[tokio::test]
async fn test_analyze_updates_existing_instinct() {
    let (logger, _tmp) = logger();
    logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "revise"}));
    analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
    // 同 trigger 再来一轮观察 + 分析 → 同 trigger instinct 的 evidence_count 递增、confidence 升档
    for _ in 0..4 {
        logger.log("s1", "revision", "editor_auditor", serde_json::json!({"chapter": 1}));
    }
    let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
    assert_eq!(outcome.updated_instincts, 1);
    assert_eq!(outcome.new_instincts, 0);
    let instincts = list_instincts(&logger, "s1").unwrap();
    assert_eq!(instincts.len(), 1);
    assert_eq!(instincts[0].evidence_count, 5);
    assert!((instincts[0].confidence - 0.5).abs() < 0.001); // 3-5 → 0.5
}

#[tokio::test]
async fn test_analyze_skips_when_insufficient() {
    let (logger, _tmp) = logger();
    logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "pass"}));
    let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
    assert_eq!(outcome.analyzed, 0);
    assert_eq!(outcome.new_instincts, 0);
    // 未达到最小样本（<2 条新观察）不调用 LLM、不推进游标
    assert_eq!(logger.count_unanalyzed("s1"), 1);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::learning 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`learning.rs` 追加（frontmatter 解析复用 `crate::prompts::registry::split_frontmatter`——**先将其提为 `pub(crate)`**）：

```rust
pub const ANALYZE_THRESHOLD: usize = 20;
const ANALYZE_MIN_NEW: usize = 2;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Instinct {
    pub id: String,
    pub trigger: String,
    pub action: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub scope: String,   // story | global
    pub status: String,  // pending | candidate | promoted | rejected
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub evolved_from: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalyzeOutcome {
    pub new_instincts: usize,
    pub updated_instincts: usize,
    pub analyzed: usize,
}

pub fn confidence_for_evidence(count: u32) -> f64 {
    match count {
        0..=2 => 0.3,
        3..=5 => 0.5,
        6..=10 => 0.7,
        _ => 0.85,
    }
}

pub fn list_instincts(logger: &ObservationLogger, story_id: &str) -> Result<Vec<Instinct>, crate::error::AppError> {
    let dir = logger.instincts_path(story_id);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|x| x == "md").unwrap_or(false) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Some(inst) = parse_instinct(&text) {
                    out.push(inst);
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn parse_instinct(text: &str) -> Option<Instinct> {
    let (fm, _body) = crate::prompts::registry::split_frontmatter(text)?;
    #[derive(serde::Deserialize)]
    struct Fm {
        id: String,
        trigger: String,
        action: String,
        confidence: f64,
        evidence_count: u32,
        scope: String,
        status: String,
        created_at: String,
        updated_at: String,
        #[serde(default)]
        evolved_from: Vec<String>,
    }
    let fm: Fm = serde_yaml::from_str(fm).ok()?;
    Some(Instinct {
        id: fm.id, trigger: fm.trigger, action: fm.action,
        confidence: fm.confidence, evidence_count: fm.evidence_count,
        scope: fm.scope, status: fm.status,
        created_at: fm.created_at, updated_at: fm.updated_at,
        evolved_from: fm.evolved_from,
    })
}

fn render_instinct(inst: &Instinct, body: &str) -> String {
    format!(
        "---\nid: {}\ntrigger: {:?}\naction: {:?}\nconfidence: {}\nevidence_count: {}\nscope: {}\nstatus: {}\ncreated_at: {:?}\nupdated_at: {:?}\nevolved_from: {:?}\n---\n\n{}\n",
        inst.id, inst.trigger, inst.action, inst.confidence, inst.evidence_count,
        inst.scope, inst.status, inst.created_at, inst.updated_at,
        serde_yaml::to_string(&inst.evolved_from).unwrap_or_else(|_| "[]".into()).trim(),
        body
    )
}

pub async fn analyze_story(
    llm: std::sync::Arc<dyn crate::agency::tool_loop::LoopLlm>,
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<AnalyzeOutcome, crate::error::AppError> {
    let new_count = logger.count_unanalyzed(story_id);
    if new_count < ANALYZE_MIN_NEW {
        return Ok(AnalyzeOutcome { new_instincts: 0, updated_instincts: 0, analyzed: 0 });
    }
    let observations = logger.recent(story_id, 50);
    let existing = list_instincts(logger, story_id).unwrap_or_default();
    let digest: String = observations
        .iter()
        .map(|o| format!("- [{}] {} by {}: {}", o.ts.get(..10).unwrap_or(&o.ts), o.kind, o.actor, o.payload))
        .collect::<Vec<_>>()
        .join("\n");
    let existing_digest: String = existing
        .iter()
        .map(|i| format!("- {}（evidence:{}）", i.trigger, i.evidence_count))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "你是创作模式分析器。以下是小说创作过程的最近观察与既有模式（instinct）。\n\
         任务：归纳出 0-3 条可复用的创作模式（trigger=何时适用，action=可操作的创作指导，evolved_from=相关观察 kind）。\n\
         规则：只输出 YAML 列表（```yaml 包裹）；不要泛化无依据的模式；与既有模式重复的给出相同 trigger 以便归并。\n\n\
         最近观察：\n{}\n\n既有模式：\n{}",
        digest, if existing_digest.is_empty() { "（无）".into() } else { existing_digest }
    );
    let raw = llm.complete(
        "你是创作模式分析器，只输出 YAML。",
        &prompt,
        crate::router::TaskType::Analysis,
        1500,
    ).await?;
    let proposals = parse_analyzer_yaml(&raw);
    let mut new_instincts = 0usize;
    let mut updated_instincts = 0usize;
    for proposal in proposals {
        let dir = logger.instincts_path(story_id);
        std::fs::create_dir_all(&dir).map_err(crate::error::AppError::from)?;
        if let Some(mut hit) = existing.iter().find(|e| e.trigger == proposal.trigger).cloned() {
            hit.evidence_count += new_count as u32;
            hit.confidence = confidence_for_evidence(hit.evidence_count);
            hit.updated_at = chrono::Local::now().to_rfc3339();
            std::fs::write(dir.join(format!("{}.md", hit.id)), render_instinct(&hit, "（更新：证据累积）"))
                .map_err(crate::error::AppError::from)?;
            updated_instincts += 1;
        } else {
            let now = chrono::Local::now().to_rfc3339();
            let inst = Instinct {
                id: format!("inst-{}-{:06x}",
                    now.get(..10).unwrap_or(&now).replace('-', ""),
                    crc32_simple(&proposal.trigger)),
                trigger: proposal.trigger.clone(),
                action: proposal.action.clone(),
                confidence: confidence_for_evidence(new_count as u32),
                evidence_count: new_count as u32,
                scope: "story".to_string(),
                status: "pending".to_string(),
                created_at: now.clone(),
                updated_at: now,
                evolved_from: proposal.evolved_from.clone(),
            };
            let body = format!("## 模式描述\n{}\n\n## 证据摘要\n（来自最近 {} 条观察）", proposal.action, new_count);
            std::fs::write(dir.join(format!("{}.md", inst.id)), render_instinct(&inst, &body))
                .map_err(crate::error::AppError::from)?;
            new_instincts += 1;
        }
    }
    logger.mark_analyzed(story_id)?;
    Ok(AnalyzeOutcome { new_instincts, updated_instincts, analyzed: new_count })
}

#[derive(Debug)]
struct AnalyzerProposal {
    trigger: String,
    action: String,
    evolved_from: Vec<String>,
}

fn parse_analyzer_yaml(raw: &str) -> Vec<AnalyzerProposal> {
    // 截取 ```yaml ... ``` 或首个 '- trigger' 起的列表
    let body = if let (Some(s), Some(e)) = (raw.find("```yaml"), raw.rfind("```")) {
        raw.get(s + 7..e).unwrap_or(raw)
    } else {
        raw
    };
    #[derive(serde::Deserialize)]
    struct P {
        trigger: String,
        action: String,
        #[serde(default)]
        evolved_from: Vec<String>,
    }
    let items: Vec<P> = serde_yaml::from_str(body).unwrap_or_default();
    items
        .into_iter()
        .map(|p| AnalyzerProposal { trigger: p.trigger, action: p.action, evolved_from: p.evolved_from })
        .collect()
}

fn crc32_simple(s: &str) -> u32 {
    // 简单稳定散列（非加密）：FNV-1a
    let mut hash: u32 = 2166136261;
    for b in s.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash & 0xFFFFFF
}
```

`prompts/registry.rs`：`fn split_frontmatter` → `pub(crate) fn split_frontmatter`。

`coordinator.rs` 的 `log_observation` 追加自动触发：

```rust
fn log_observation(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) {
    // ……原 log 调用……
    // 自动分析：未分析观察累计 ≥20 触发（后台、Background 档、防自观察 label）
    let count = crate::agency::learning::ObservationLogger::new(dir.clone()).count_unanalyzed(story_id);
    if count >= crate::agency::learning::ANALYZE_THRESHOLD {
        if let Some(app) = &self.app_handle {
            let llm = self.llm_for_run(&uuid::Uuid::new_v4().to_string(), AgentRole::EditorAuditor);
            let logger = crate::agency::learning::ObservationLogger::new(dir);
            let sid = story_id.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::agency::learning::analyze_story(llm, &logger, &sid).await {
                    log::warn!("learning analyzer 失败: {}", e);
                }
            });
        }
    }
}
```

`commands.rs`：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_analyze_learning(
    story_id: String,
    app_handle: AppHandle,
) -> Result<crate::agency::learning::AnalyzeOutcome, AppError> {
    let dir = app_handle.path().app_data_dir()
        .map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    let logger = crate::agency::learning::ObservationLogger::new(dir);
    let llm = crate::agency::coordinator::AgencyLlm::new(
        app_handle.clone(),
        uuid::Uuid::new_v4().to_string(),
        crate::agency::models::AgentRole::EditorAuditor,
        story_id.clone(),
    );
    crate::agency::learning::analyze_story(std::sync::Arc::new(llm), &logger, &story_id).await
}
```

注：analyzer 的 context_label 为 `agency_editor`（EditorAuditor 档），其 llm_call 埋点会被 ObservationLogger::should_record 过滤吗？不会——label 是 agency_editor 不含 agency_observer。**修正**：AgencyLlm 加 `label_override: Option<String>`，analyzer 构造时 `.with_label("agency_observer")`，complete_metered 用 override 标签（路由上 agency_observer 不匹配任何 agency_ 前缀 → 走 TaskClass 推导，Analysis 任务仍可接受；或 override 为 "agency_observer_editor" 保持 Background——`derive_model_role_from_label` 对 "agency_observer_editor"：`strip_prefix("agency_")` 得 "observer_editor" → starts_with("editor") 命中 Background 档。用这个）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 3）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/ src-tauri/src/prompts/registry.rs src-tauri/src/handlers.rs
git commit -m "feat(agency): learning analyzer producing instinct files (background tier)"
```

---

### Task 3: instinct 置信度引擎（采纳/纠正/衰减/prune）

**Files:**
- Modify: `src-tauri/src/agency/learning.rs`（`apply_feedback` / `apply_weekly_decay` / `prune_instincts`）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_instinct_feedback`）

**Interfaces:**
- Produces: `learning::apply_feedback(logger, story_id, instinct_id, accepted: bool) -> Result<Instinct, AppError>`（采纳 +0.05 / 纠正 −0.1，clamp 0..1）；`learning::apply_weekly_decay(logger, story_id) -> usize`（每条按 updated_at 每满 7 天 −0.02，写回）；`learning::prune_instincts(logger, story_id) -> usize`（confidence <0.2 或 pending 且 ≥90 天未更新的删除）；常量 `FEEDBACK_ACCEPT: f64 = 0.05` / `FEEDBACK_REJECT: f64 = -0.1` / `WEEKLY_DECAY: f64 = -0.02` / `PRUNE_CONFIDENCE: f64 = 0.2` / `PRUNE_TTL_DAYS: i64 = 90`。T4 晋升判定消费 confidence；T5 前端经 IPC 调 feedback。

- [ ] **Step 1: 写失败的测试**

`learning.rs` 测试模块追加：

```rust
fn seed_instinct(logger: &ObservationLogger, story_id: &str, id: &str, confidence: f64, updated_at: &str) {
    let inst = Instinct {
        id: id.to_string(),
        trigger: "测试触发".to_string(),
        action: "测试动作".to_string(),
        confidence,
        evidence_count: 3,
        scope: "story".to_string(),
        status: "pending".to_string(),
        created_at: updated_at.to_string(),
        updated_at: updated_at.to_string(),
        evolved_from: vec![],
    };
    let dir = logger.instincts_path(story_id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{}.md", id)), render_instinct(&inst, "body")).unwrap();
}

#[test]
fn test_feedback_accept_and_reject() {
    let (logger, _tmp) = logger();
    seed_instinct(&logger, "s1", "inst-a", 0.5, "2026-07-01T00:00:00+08:00");
    let updated = apply_feedback(&logger, "s1", "inst-a", true).unwrap();
    assert!((updated.confidence - 0.55).abs() < 0.001);
    let updated2 = apply_feedback(&logger, "s1", "inst-a", false).unwrap();
    assert!((updated2.confidence - 0.45).abs() < 0.001);
    // 下限 clamp
    seed_instinct(&logger, "s1", "inst-b", 0.05, "2026-07-01T00:00:00+08:00");
    let clamped = apply_feedback(&logger, "s1", "inst-b", false).unwrap();
    assert!(clamped.confidence >= 0.0);
}

#[test]
fn test_weekly_decay() {
    let (logger, _tmp) = logger();
    let old = chrono::Local::now() - chrono::Duration::days(14);
    seed_instinct(&logger, "s1", "inst-old", 0.5, &old.to_rfc3339());
    let fresh = chrono::Local::now() - chrono::Duration::days(3);
    seed_instinct(&logger, "s1", "inst-fresh", 0.5, &fresh.to_rfc3339());
    let decayed = apply_weekly_decay(&logger, "s1").unwrap();
    assert_eq!(decayed, 1); // 只有 14 天前的那条衰减（每满 7 天 -0.02，14 天 -0.04）
    let instincts = list_instincts(&logger, "s1").unwrap();
    let old_inst = instincts.iter().find(|i| i.id == "inst-old").unwrap();
    assert!((old_inst.confidence - 0.46).abs() < 0.001);
    let fresh_inst = instincts.iter().find(|i| i.id == "inst-fresh").unwrap();
    assert!((fresh_inst.confidence - 0.5).abs() < 0.001);
}

#[test]
fn test_prune() {
    let (logger, _tmp) = logger();
    seed_instinct(&logger, "s1", "inst-weak", 0.1, &chrono::Local::now().to_rfc3339());
    let old = chrono::Local::now() - chrono::Duration::days(100);
    seed_instinct(&logger, "s1", "inst-stale", 0.5, &old.to_rfc3339());
    seed_instinct(&logger, "s1", "inst-good", 0.5, &chrono::Local::now().to_rfc3339());
    let pruned = prune_instincts(&logger, "s1").unwrap();
    assert_eq!(pruned, 2);
    let remaining = list_instincts(&logger, "s1").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "inst-good");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::learning 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`learning.rs` 追加：

```rust
pub const FEEDBACK_ACCEPT: f64 = 0.05;
pub const FEEDBACK_REJECT: f64 = -0.1;
pub const WEEKLY_DECAY: f64 = -0.02;
pub const PRUNE_CONFIDENCE: f64 = 0.2;
pub const PRUNE_TTL_DAYS: i64 = 90;

fn read_instinct_file(logger: &ObservationLogger, story_id: &str, id: &str) -> Result<(Instinct, std::path::PathBuf), crate::error::AppError> {
    let path = logger.instincts_path(story_id).join(format!("{}.md", id));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| crate::error::AppError::validation_failed(format!("instinct 不存在: {} ({})", id, e), None::<String>))?;
    let inst = parse_instinct(&text)
        .ok_or_else(|| crate::error::AppError::validation_failed(format!("instinct 解析失败: {}", id), None::<String>))?;
    Ok((inst, path))
}

fn write_instinct_file(path: &std::path::Path, inst: &Instinct) -> Result<(), crate::error::AppError> {
    std::fs::write(path, render_instinct(inst, "（反馈/衰减更新）")).map_err(crate::error::AppError::from)
}

pub fn apply_feedback(logger: &ObservationLogger, story_id: &str, instinct_id: &str, accepted: bool) -> Result<Instinct, crate::error::AppError> {
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    let delta = if accepted { FEEDBACK_ACCEPT } else { FEEDBACK_REJECT };
    inst.confidence = (inst.confidence + delta).clamp(0.0, 1.0);
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(inst)
}

pub fn apply_weekly_decay(logger: &ObservationLogger, story_id: &str) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut decayed = 0usize;
    for mut inst in instincts {
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let weeks = (now - updated).num_days() / 7;
        if weeks >= 1 {
            inst.confidence = (inst.confidence + weeks as f64 * WEEKLY_DECAY).clamp(0.0, 1.0);
            inst.updated_at = now.to_rfc3339();
            let path = logger.instincts_path(story_id).join(format!("{}.md", inst.id));
            write_instinct_file(&path, &inst)?;
            decayed += 1;
        }
    }
    Ok(decayed)
}

pub fn prune_instincts(logger: &ObservationLogger, story_id: &str) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut pruned = 0usize;
    for inst in instincts {
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let stale_days = (now - updated).num_days();
        let should_prune = inst.confidence < PRUNE_CONFIDENCE
            || (inst.status == "pending" && stale_days >= PRUNE_TTL_DAYS);
        if should_prune {
            let path = logger.instincts_path(story_id).join(format!("{}.md", inst.id));
            if std::fs::remove_file(&path).is_ok() {
                pruned += 1;
            }
        }
    }
    Ok(pruned)
}
```

`commands.rs`：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_instinct_feedback(
    story_id: String,
    instinct_id: String,
    accepted: bool,
    app_handle: AppHandle,
) -> Result<crate::agency::learning::Instinct, AppError> {
    let dir = app_handle.path().app_data_dir()
        .map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    let logger = crate::agency::learning::ObservationLogger::new(dir);
    tokio::task::spawn_blocking(move || {
        crate::agency::learning::apply_feedback(&logger, &story_id, &instinct_id, accepted)
    })
    .await
    .map_err(|e| AppError::from(format!("feedback join error: {}", e)))?
}
```

`handlers.rs` 注册 `agency_analyze_learning` 与 `agency_instinct_feedback`。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`（新增 3）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/learning.rs src-tauri/src/agency/commands.rs src-tauri/src/handlers.rs
git commit -m "feat(agency): instinct confidence engine (feedback, weekly decay, prune)"
```

---

### Task 4: 晋升管线 + 物化为技能（含 reload_skills 启动修复）

**Files:**
- Modify: `src-tauri/src/agency/learning.rs`（`promotion_candidates` / `confirm_promotion` / `reject_promotion` / `materialize_as_skill`）
- Modify: `src-tauri/src/agency/commands.rs`（`agency_promotion_candidates` / `agency_confirm_promotion` / `agency_reject_promotion`）
- Modify: `src-tauri/src/lib.rs`（setup 中 `SkillManager::reload_skills()`）
- Modify: `src-tauri/src/handlers.rs`（注册）

**Interfaces:**
- Consumes: T3 的 confidence/status；`SkillManager::import_skill`（skills/mod.rs:274）；技能目录格式 `skill.yaml + main.prompt`（loader.rs:104-125）。
- Produces: `learning::promotion_candidates(logger, story_id) -> Vec<Instinct>`（status=pending|candidate 且 confidence ≥0.8 且同 trigger 在 ≥2 story 出现——跨 story 扫描 `{app_dir}/stories/*/.storymoss/learning/instincts/`）；`learning::confirm_promotion(logger, story_id, instinct_id, skills_manager) -> Result<PromoteOutcome, AppError>`；`PromoteOutcome { instinct: Instinct, skill_id: String }`；`reject_promotion(logger, story_id, instinct_id) -> Instinct`（confidence −0.1、status=rejected）；技能 id 规范 `learned.<instinct_id>`。

**物化技能文件（模板逐字）：**
```yaml
# skill.yaml
id: learned.<instinct_id>
name: "学到的模式：<trigger 前 30 字>"
version: "0.1.0"
description: "<trigger 全文>"
author: "StoryMoss Learning"
category: custom
entry_point: "main.prompt"
parameters: []
capabilities: []
hooks: []
config:
  evolved_from: "<instinct_id>"
  confidence: <confidence>
```
```
# main.prompt
你是小说创作助手。以下是从创作过程学到的模式，请在适用时遵循：

触发条件：<trigger>
指导动作：<action>
---
{{instruction}}
```

- [ ] **Step 1: 写失败的测试**

`learning.rs` 测试模块追加：

```rust
#[test]
fn test_promotion_candidates_cross_story() {
    let (logger, _tmp) = logger();
    // 同 trigger 在 s1/s2 各一条（s1 confidence 0.85，s2 0.8）
    seed_instinct(&logger, "s1", "inst-x", 0.85, &chrono::Local::now().to_rfc3339());
    seed_instinct(&logger, "s2", "inst-y", 0.8, &chrono::Local::now().to_rfc3339());
    // s3 只有一条同 trigger（不重复出现 → 不算跨 story）
    seed_instinct(&logger, "s3", "inst-z", 0.3, &chrono::Local::now().to_rfc3339());
    let candidates = promotion_candidates(&logger, "s1").unwrap();
    assert!(candidates.iter().any(|i| i.id == "inst-x"), "s1 的高置信跨 story instinct 应为候选");
    // s3 的 inst-z confidence 0.3 不达标
    assert!(!candidates.iter().any(|i| i.id == "inst-z"));
}

#[test]
fn test_reject_promotion() {
    let (logger, _tmp) = logger();
    seed_instinct(&logger, "s1", "inst-r", 0.85, &chrono::Local::now().to_rfc3339());
    let rejected = reject_promotion(&logger, "s1", "inst-r").unwrap();
    assert_eq!(rejected.status, "rejected");
    assert!((rejected.confidence - 0.75).abs() < 0.001);
}

#[test]
fn test_materialize_as_skill_files() {
    let (logger, tmp) = logger();
    seed_instinct(&logger, "s1", "inst-m", 0.85, &chrono::Local::now().to_rfc3339());
    let skills_dir = tmp.path().join("skills");
    let skill_dir = materialize_as_skill(&logger, "s1", "inst-m", &skills_dir).unwrap();
    let manifest = std::fs::read_to_string(skill_dir.join("skill.yaml")).unwrap();
    assert!(manifest.contains("id: learned.inst-m"));
    assert!(manifest.contains("evolved_from: \"inst-m\"") || manifest.contains("evolved_from: 'inst-m'") || manifest.contains("evolved_from: inst-m"));
    let prompt = std::fs::read_to_string(skill_dir.join("main.prompt")).unwrap();
    assert!(prompt.contains("---"));
    assert!(prompt.contains("{{instruction}}"));
    // skill.yaml 可被 serde_yaml 解析为 SkillManifest（loader 兼容）
    let parsed: crate::skills::SkillManifest = serde_yaml::from_str(&manifest).unwrap();
    assert_eq!(parsed.id, "learned.inst-m");
    assert_eq!(parsed.entry_point, "main.prompt");
}

#[test]
fn test_confirm_promotion_end_to_end() {
    let (logger, tmp) = logger();
    seed_instinct(&logger, "s1", "inst-c", 0.85, &chrono::Local::now().to_rfc3339());
    seed_instinct(&logger, "s2", "inst-c2", 0.8, &chrono::Local::now().to_rfc3339()); // 同 trigger
    let skills_dir = tmp.path().join("skills");
    let outcome = confirm_promotion(&logger, "s1", "inst-c", &skills_dir).unwrap();
    assert_eq!(outcome.skill_id, "learned.inst-c");
    assert_eq!(outcome.instinct.status, "promoted");
    assert_eq!(outcome.instinct.scope, "global");
    // 技能目录已生成
    assert!(skills_dir.join("learned.inst-c/skill.yaml").exists());
    // 失败路径：confidence 不足
    seed_instinct(&logger, "s1", "inst-low", 0.3, &chrono::Local::now().to_rfc3339());
    assert!(confirm_promotion(&logger, "s1", "inst-low", &skills_dir).is_err());
}
```

注：`confirm_promotion` 的 skills_manager 参数在测试中以 skills_dir 传入——实现时把"物化+注册"拆为 `materialize_as_skill(logger, story_id, id, skills_dir) -> PathBuf`（纯文件）+ commands 层调 `SkillManager::import_skill(&skill_dir)`（需要 SkillManager State）。`seed_instinct` 的同 trigger 约定：本测试全部用 T3 的默认 "测试触发"。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::learning 2>&1 | tail -3`
Expected: FAIL

- [ ] **Step 3: 实现**

`learning.rs` 追加：

```rust
pub const PROMOTE_CONFIDENCE: f64 = 0.8;
pub const PROMOTE_MIN_STORIES: usize = 2;

pub fn promotion_candidates(logger: &ObservationLogger, story_id: &str) -> Result<Vec<Instinct>, crate::error::AppError> {
    // 跨 story 统计 trigger 出现次数
    let counts = trigger_story_counts(logger)?;
    let instincts = list_instincts(logger, story_id)?;
    Ok(instincts
        .into_iter()
        .filter(|i| {
            (i.status == "pending" || i.status == "candidate")
                && i.confidence >= PROMOTE_CONFIDENCE
                && counts.get(&i.trigger).copied().unwrap_or(0) >= PROMOTE_MIN_STORIES
        })
        .collect())
}

/// 扫描全部 story 的 learning/instincts，统计每个 trigger 出现在多少个 story。
fn trigger_story_counts(logger: &ObservationLogger) -> Result<std::collections::HashMap<String, usize>, crate::error::AppError> {
    let stories_dir = logger.app_dir.join("stories");
    let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let entries = match std::fs::read_dir(&stories_dir) {
        Ok(e) => e,
        Err(_) => return Ok(map),
    };
    for entry in entries.flatten() {
        let story_dir = entry.path();
        if !story_dir.is_dir() {
            continue;
        }
        let story_id = entry.file_name().to_string_lossy().to_string();
        let instincts = list_instincts(logger, &story_id).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for inst in instincts {
            seen.insert(inst.trigger);
        }
        for trigger in seen {
            *map.entry(trigger).or_insert(0) += 1;
        }
    }
    Ok(map)
}

pub fn reject_promotion(logger: &ObservationLogger, story_id: &str, instinct_id: &str) -> Result<Instinct, crate::error::AppError> {
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    inst.confidence = (inst.confidence + FEEDBACK_REJECT).clamp(0.0, 1.0);
    inst.status = "rejected".to_string();
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(inst)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PromoteOutcome {
    pub instinct: Instinct,
    pub skill_id: String,
}

pub fn confirm_promotion(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
    skills_dir: &std::path::Path,
) -> Result<PromoteOutcome, crate::error::AppError> {
    // 校验候选资格
    let candidates = promotion_candidates(logger, story_id)?;
    if !candidates.iter().any(|i| i.id == instinct_id) {
        return Err(crate::error::AppError::validation_failed(
            format!("instinct {} 不满足晋升条件（需 confidence≥{} 且跨 {} 个 story 复现）", instinct_id, PROMOTE_CONFIDENCE, PROMOTE_MIN_STORIES),
            None::<String>,
        ));
    }
    let skill_dir = materialize_as_skill(logger, story_id, instinct_id, skills_dir)?;
    // instinct 状态与作用域更新
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    inst.status = "promoted".to_string();
    inst.scope = "global".to_string();
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(PromoteOutcome {
        skill_id: skill_dir.file_name().unwrap().to_string_lossy().to_string(),
        instinct: inst,
    })
}

/// 物化为 skill.yaml 目录技能（纯文件操作；注册由 commands 层经 SkillManager::import_skill 完成）。
pub fn materialize_as_skill(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
    skills_dir: &std::path::Path,
) -> Result<std::path::PathBuf, crate::error::AppError> {
    let (inst, _) = read_instinct_file(logger, story_id, instinct_id)?;
    let skill_id = format!("learned.{}", inst.id);
    let skill_dir = skills_dir.join(&skill_id);
    std::fs::create_dir_all(&skill_dir).map_err(crate::error::AppError::from)?;
    let name: String = inst.trigger.chars().take(30).collect();
    let manifest = format!(
        "id: {}\nname: \"学到的模式：{}\"\nversion: \"0.1.0\"\ndescription: {:?}\nauthor: \"StoryMoss Learning\"\ncategory: custom\nentry_point: \"main.prompt\"\nparameters: []\ncapabilities: []\nhooks: []\nconfig:\n  evolved_from: {:?}\n  confidence: {}\n",
        skill_id, name, inst.trigger, inst.id, inst.confidence
    );
    std::fs::write(skill_dir.join("skill.yaml"), manifest).map_err(crate::error::AppError::from)?;
    let prompt = format!(
        "你是小说创作助手。以下是从创作过程学到的模式，请在适用时遵循：\n\n触发条件：{}\n指导动作：{}\n---\n{{{{instruction}}}}\n",
        inst.trigger, inst.action
    );
    std::fs::write(skill_dir.join("main.prompt"), prompt).map_err(crate::error::AppError::from)?;
    Ok(skill_dir)
}
```

注意：`logger.app_dir` 需改为 `pub(crate)` 可见性（trigger_story_counts 用）；`SkillManifest` 的 `category: custom` 对应 `SkillCategory::Custom`（snake_case serde，loader 兼容——测试已断言 serde_yaml 解析）。

`commands.rs`：

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_promotion_candidates(
    story_id: String,
    app_handle: AppHandle,
) -> Result<Vec<crate::agency::learning::Instinct>, AppError> {
    let dir = app_handle.path().app_data_dir().map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    tokio::task::spawn_blocking(move || {
        crate::agency::learning::promotion_candidates(&crate::agency::learning::ObservationLogger::new(dir), &story_id)
    }).await.map_err(|e| AppError::from(format!("candidates join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_confirm_promotion(
    story_id: String,
    instinct_id: String,
    app_handle: AppHandle,
    skills: tauri::State<'_, crate::skills::SkillManager>,
) -> Result<crate::agency::learning::PromoteOutcome, AppError> {
    let dir = app_handle.path().app_data_dir().map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    let skills_dir = crate::skills::get_default_skills_dir();
    let outcome = tokio::task::spawn_blocking(move || {
        crate::agency::learning::confirm_promotion(
            &crate::agency::learning::ObservationLogger::new(dir), &story_id, &instinct_id, &skills_dir)
    }).await.map_err(|e| AppError::from(format!("confirm join error: {}", e)))??;
    // 注册进内存 registry（import_skill 会拷贝到 skills_dir 同名目录——物化已在原位，拷贝为幂等覆盖）
    let skill_dir = crate::skills::get_default_skills_dir().join(&outcome.skill_id);
    let skill = skills.import_skill(&skill_dir)?;
    // 观察：晋升事件
    let logger = crate::agency::learning::ObservationLogger::new(
        app_handle.path().app_data_dir().map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?);
    logger.log(&outcome.instinct.scope, "promotion", "user", serde_json::json!({
        "instinct_id": outcome.instinct.id, "skill_id": skill.manifest.id,
    }));
    Ok(outcome)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_reject_promotion(
    story_id: String,
    instinct_id: String,
    app_handle: AppHandle,
) -> Result<crate::agency::learning::Instinct, AppError> {
    let dir = app_handle.path().app_data_dir().map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    tokio::task::spawn_blocking(move || {
        crate::agency::learning::reject_promotion(
            &crate::agency::learning::ObservationLogger::new(dir), &story_id, &instinct_id)
    }).await.map_err(|e| AppError::from(format!("reject join error: {}", e)))?
}
```

`skills/mod.rs`：`fn get_default_skills_dir` 改 `pub fn`（现状为私有）。

`lib.rs` 启动修复（skills/mod.rs:740-744 附近，以实际为准）：

```rust
let mut skill_manager = crate::skills::SkillManager::new(Some(llm_service.clone()), Some(pool.clone()));
skill_manager.reload_skills(); // v0.30.0：恢复磁盘上的用户/学习技能（此前启动不加载）
app.manage(skill_manager);
```

（`reload_skills(&mut self)` 签名要求 mut 绑定；若 SkillManager::new 返回非 mut 或与 State 形态冲突，以最小改动适配并在报告说明。）

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`（新增 4）及全量。
Expected: PASS、无新警告。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/learning.rs src-tauri/src/agency/commands.rs src-tauri/src/skills/mod.rs src-tauri/src/lib.rs src-tauri/src/handlers.rs
git commit -m "feat(agency): promotion pipeline with skill materialization + startup skill reload"
```

---

### Task 5: 学习中心前端（AgencyLearning.tsx + 数据 IPC）

**Files:**
- Modify: `src-tauri/src/agency/commands.rs`（`agency_learning_overview`）
- Modify: `src-tauri/src/handlers.rs`（注册）
- Modify: `src-frontend/src/services/api/agency.ts`（学习中心类型与封装追加）
- Create: `src-frontend/src/pages/AgencyLearning.tsx`
- Create: `src-frontend/src/pages/__tests__/AgencyLearning.test.tsx`
- Modify: `src-frontend/src/types/index.ts`（ViewType 加 'agency-learning'）、`src-frontend/src/App.tsx`、`src-frontend/src/components/Sidebar.tsx`（诊断组）

**Interfaces:**
- Produces（后端）：`LearningOverview { instincts: Vec<Instinct>, candidates: Vec<Instinct>, recent_observations: Vec<Observation>, unanalyzed_count: usize }`；IPC `agency_learning_overview(story_id)`。
- Produces（前端）：`AgencyLearning` 页面（instinct 列表 + 置信度条 + 晋升卡片区 + 观察流 + 手动分析按钮）；`agency.ts` 追加 `Instinct/Observation/LearningOverview/AnalyzeOutcome/PromoteOutcome` 类型与 `getLearningOverview/analyzeLearning/confirmPromotion/rejectPromotion/instinctFeedback` 封装。

**后端 `agency_learning_overview`：**

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct LearningOverview {
    pub instincts: Vec<crate::agency::learning::Instinct>,
    pub candidates: Vec<crate::agency::learning::Instinct>,
    pub recent_observations: Vec<crate::agency::learning::Observation>,
    pub unanalyzed_count: usize,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_learning_overview(
    story_id: String,
    app_handle: AppHandle,
) -> Result<LearningOverview, AppError> {
    let dir = app_handle.path().app_data_dir().map_err(|e| AppError::from(format!("app_data_dir: {}", e)))?;
    tokio::task::spawn_blocking(move || -> Result<LearningOverview, AppError> {
        let logger = crate::agency::learning::ObservationLogger::new(dir);
        // 惰性周衰减（读取时生效，ECC 同参数）
        let _ = crate::agency::learning::apply_weekly_decay(&logger, &story_id);
        let instincts = crate::agency::learning::list_instincts(&logger, &story_id)?;
        let candidates = crate::agency::learning::promotion_candidates(&logger, &story_id)?;
        let recent_observations = logger.recent(&story_id, 20);
        let unanalyzed_count = logger.count_unanalyzed(&story_id);
        Ok(LearningOverview { instincts, candidates, recent_observations, unanalyzed_count })
    }).await.map_err(|e| AppError::from(format!("learning overview join error: {}", e)))?
}
```

**前端页面（完整代码）：**

`agency.ts` 追加：

```ts
export interface Instinct {
  id: string;
  trigger: string;
  action: string;
  confidence: number;
  evidence_count: number;
  scope: string;
  status: string;
  created_at: string;
  updated_at: string;
  evolved_from: string[];
}

export interface Observation {
  ts: string;
  story_id: string;
  kind: string;
  actor: string;
  payload: Record<string, unknown>;
}

export interface LearningOverview {
  instincts: Instinct[];
  candidates: Instinct[];
  recent_observations: Observation[];
  unanalyzed_count: number;
}

export interface AnalyzeOutcome {
  new_instincts: number;
  updated_instincts: number;
  analyzed: number;
}

export interface PromoteOutcome {
  instinct: Instinct;
  skill_id: string;
}

export function getLearningOverview(storyId: string) {
  return loggedInvoke<LearningOverview>('agency_learning_overview', { story_id: storyId });
}

export function analyzeLearning(storyId: string) {
  return loggedInvoke<AnalyzeOutcome>('agency_analyze_learning', { story_id: storyId });
}

export function confirmPromotion(storyId: string, instinctId: string) {
  return loggedInvoke<PromoteOutcome>('agency_confirm_promotion', { story_id: storyId, instinct_id: instinctId });
}

export function rejectPromotion(storyId: string, instinctId: string) {
  return loggedInvoke<Instinct>('agency_reject_promotion', { story_id: storyId, instinct_id: instinctId });
}

export function instinctFeedback(storyId: string, instinctId: string, accepted: boolean) {
  return loggedInvoke<Instinct>('agency_instinct_feedback', { story_id: storyId, instinct_id: instinctId, accepted });
}
```

`AgencyLearning.tsx`：

```tsx
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import {
  getLearningOverview, analyzeLearning, confirmPromotion, rejectPromotion, instinctFeedback,
} from '@/services/api/agency';
import type { Instinct } from '@/services/api/agency';

function ConfidenceBar({ value }: { value: number }) {
  const pct = Math.round(value * 100);
  const color = value >= 0.8 ? '#22c55e' : value >= 0.5 ? '#f59e0b' : '#9ca3af';
  return (
    <div className="h-2 w-24 rounded bg-gray-200">
      <div className="h-2 rounded" style={{ width: `${pct}%`, background: color }} />
    </div>
  );
}

export default function AgencyLearning() {
  const currentStory = useAppStore(s => s.currentStory);
  const storyId = currentStory?.id ?? '';
  const qc = useQueryClient();
  const [analyzing, setAnalyzing] = useState(false);
  const { data, isLoading, error } = useQuery({
    queryKey: ['agency-learning', storyId],
    queryFn: () => getLearningOverview(storyId),
    enabled: !!storyId,
    staleTime: 15_000,
  });
  const refresh = () => qc.invalidateQueries({ queryKey: ['agency-learning', storyId] });

  if (!currentStory) return <p className="p-6 text-gray-500">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载学习数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  const onAnalyze = async () => {
    setAnalyzing(true);
    try {
      await analyzeLearning(storyId);
      await refresh();
    } finally {
      setAnalyzing(false);
    }
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">学习中心 · {currentStory.title}</h1>
        <button
          onClick={onAnalyze}
          disabled={analyzing || data.unanalyzed_count < 2}
          className="rounded bg-indigo-600 px-3 py-1 text-sm text-white disabled:opacity-40"
        >
          {analyzing ? '分析中…' : `立即分析（${data.unanalyzed_count} 条未分析观察）`}
        </button>
      </div>

      {data.candidates.length > 0 && (
        <section>
          <h2 className="mb-2 font-medium">晋升提案（{data.candidates.length}）</h2>
          <div className="space-y-2">
            {data.candidates.map(c => (
              <div key={c.id} className="flex items-center justify-between rounded border border-amber-300 bg-amber-50 p-3">
                <div>
                  <div className="font-medium">{c.trigger}</div>
                  <div className="text-sm text-gray-600">{c.action}</div>
                  <div className="mt-1 flex items-center gap-2 text-xs text-gray-500">
                    <ConfidenceBar value={c.confidence} />
                    <span>{(c.confidence * 100).toFixed(0)}%</span>
                    <span>证据 {c.evidence_count}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={async () => { await confirmPromotion(storyId, c.id); await refresh(); }}
                    className="rounded bg-green-600 px-3 py-1 text-sm text-white"
                  >确认为技能</button>
                  <button
                    onClick={async () => { await rejectPromotion(storyId, c.id); await refresh(); }}
                    className="rounded border px-3 py-1 text-sm"
                  >拒绝</button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <section>
        <h2 className="mb-2 font-medium">已学模式（{data.instincts.length}）</h2>
        {data.instincts.length === 0 && <p className="text-sm text-gray-500">尚无模式——创作几章后点击"立即分析"。</p>}
        <div className="space-y-2">
          {data.instincts.map((i: Instinct) => (
            <div key={i.id} className="rounded border p-3">
              <div className="flex items-center justify-between">
                <div className="font-medium">{i.trigger}</div>
                <span className="text-xs text-gray-400">{i.status}{i.scope === 'global' ? ' · global' : ''}</span>
              </div>
              <div className="text-sm text-gray-600">{i.action}</div>
              <div className="mt-1 flex items-center gap-2 text-xs text-gray-500">
                <ConfidenceBar value={i.confidence} />
                <span>{(i.confidence * 100).toFixed(0)}%</span>
                <span>证据 {i.evidence_count}</span>
                <button onClick={async () => { await instinctFeedback(storyId, i.id, true); await refresh(); }} className="ml-2 underline">有用</button>
                <button onClick={async () => { await instinctFeedback(storyId, i.id, false); await refresh(); }} className="underline">不准</button>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-medium">最近观察</h2>
        <table className="w-full text-sm">
          <thead><tr className="text-left text-gray-500"><th>时间</th><th>类型</th><th>角色</th><th>摘要</th></tr></thead>
          <tbody>
            {data.recent_observations.slice().reverse().map((o, idx) => (
              <tr key={idx} className="border-t">
                <td className="text-gray-400">{o.ts.slice(5, 16)}</td>
                <td>{o.kind}</td>
                <td>{o.actor}</td>
                <td className="max-w-md truncate">{JSON.stringify(o.payload)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
```

测试（`pages/__tests__/AgencyLearning.test.tsx`，模式同 AgencyEval.test）：

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const confirmMock = vi.fn().mockResolvedValue({ instinct: {}, skill_id: 'learned.inst-x' });

vi.mock('@/services/api/agency', () => ({
  getLearningOverview: vi.fn().mockResolvedValue({
    instincts: [
      { id: 'inst-a', trigger: '当编辑审计连续两轮 revise', action: '修订前先复读角色卡', confidence: 0.5, evidence_count: 3, scope: 'story', status: 'pending', created_at: 't', updated_at: 't', evolved_from: [] },
    ],
    candidates: [
      { id: 'inst-x', trigger: '当用户频繁修改开头', action: '开头避免天气描写', confidence: 0.85, evidence_count: 6, scope: 'story', status: 'candidate', created_at: 't', updated_at: 't', evolved_from: [] },
    ],
    recent_observations: [{ ts: '2026-07-18T10:00:00+08:00', story_id: 's1', kind: 'gate', actor: 'editor_auditor', payload: { outcome: 'pass' } }],
    unanalyzed_count: 3,
  }),
  analyzeLearning: vi.fn().mockResolvedValue({ new_instincts: 1, updated_instincts: 0, analyzed: 3 }),
  confirmPromotion: (...args: unknown[]) => confirmMock(...args),
  rejectPromotion: vi.fn().mockResolvedValue({}),
  instinctFeedback: vi.fn().mockResolvedValue({}),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: any) => sel({ currentStory: { id: 's1', title: '学习书' } }),
}));

import AgencyLearning from '../AgencyLearning';

describe('AgencyLearning', () => {
  it('渲染晋升提案与模式列表，确认按钮触发 confirm', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={qc}><AgencyLearning /></QueryClientProvider>);
    expect(await screen.findByText('晋升提案（1）')).toBeInTheDocument();
    expect(await screen.findByText('当用户频繁修改开头')).toBeInTheDocument();
    expect(await screen.findByText('当编辑审计连续两轮 revise')).toBeInTheDocument();
    fireEvent.click(await screen.findByText('确认为技能'));
    await waitFor(() => expect(confirmMock).toHaveBeenCalledWith('s1', 'inst-x'));
  });
});
```

注册三处：ViewType 加 `| 'agency-learning'`；App.tsx `case 'agency-learning': return <AgencyLearning />;`（import 同步）；Sidebar 诊断组加 `{ id: 'agency-learning', label: '学习中心', icon: Brain, impact: 'warm' }`（Brain 以 lucide-react 现有版本为准，没有则用 GraduationCap/Lightbulb）。

- [ ] **Step 1: 后端 IPC + Rust 冒烟测试**（overview 聚合：种子 instinct 文件 + 观察，断言 candidates/计数正确）。
- [ ] **Step 2: 前端封装 + 页面 + 注册 + vitest**（`npx vitest run` 全绿 + `npm run type-check`）。
- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/agency/commands.rs src-tauri/src/handlers.rs src-frontend/src/
git commit -m "feat(agency): learning center page with promotion flow + feedback loop"
```

---

### Task 6: 代理工作室前端 + 事件补全

**Files:**
- Modify: `src-tauri/src/agency/coordinator.rs`（emit_activity 补全：genesis 五阶段、producer、editor done）
- Create: `src-frontend/src/pages/AgencyStudio.tsx`
- Modify: `src-frontend/src/services/api/agency.ts`（BoardItem 类型 + getRun/listBoard 封装）
- Modify: `src-frontend/src/types/index.ts`（ViewType 加 'agency-studio'）、`src-frontend/src/App.tsx`、`src-frontend/src/components/Sidebar.tsx`（创作组或诊断组——放创作组，label "代理工作室"，impact hot）
- Create: `src-frontend/src/pages/__tests__/AgencyStudio.test.tsx`

**Interfaces:**
- Consumes: 事件 `agency-run-progress`/`agency-agent-activity`/`agency-board-changed`（payload 见 Global Constraints 说明）；IPC `agency_get_run`/`agency_list_board`（已有）。
- Produces（后端）：emit_activity 补全点位——genesis：concept(LeadWriter)、assets(Producer)、writing(LeadWriter)、review(EditorAuditor start+done)、assembly(Producer)；batch：`tokio::join!` 汇合后 editor done；handle_gate 完成后（已有 editor start 处配对）。`AgencyRun`/`BoardItem` 已有 serde Serialize（前端直接镜像）。

**事件补全（后端，全部一行级）：**
- `run_genesis_inner`：concept 完成、assets 完成、writing 开始、review 开始（evaluate_gate 调用处）+ done、assembly 开始。
- `run_batch_inner`：`tokio::join!` 汇合后（gate 结果到手时）emit editor done。
- emit_activity 是 `fn(&self,...)`——genesis 路径 self 可用，直接加。

**前端页面结构（AgencyStudio.tsx）：**

```tsx
// 三状态卡：role 显示名 + 最近动作（来自 activity 事件流该 role 最新一条）+ 当前 run 状态（run-progress 最新）
// 黑板视图：agency_list_board(run_id) 按 zone 四栏（asset/draft/review/schedule），item 显示 key/summary/v版本/status
// 时间线：activity + progress 事件按时间倒序（最多 100 条），格式 [HH:mm:ss] role/phase action detail
// run 选择：当前 story 最近 run（agency 无 list_runs——用 agency_list_board 需要 run_id；
//   简化：监听事件捕获活跃 run_id；无事件时提示"暂无活动"）
```

数据流（hooks 内嵌页面）：

```tsx
const [activities, setActivities] = useState<ActivityEvent[]>([]);
const [progress, setProgress] = useState<ProgressEvent[]>([]);
const [activeRunId, setActiveRunId] = useState<string | null>(null);

useEffect(() => {
  let un1: (() => void) | undefined, un2: (() => void) | undefined, un3: (() => void) | undefined;
  (async () => {
    const { listen } = await import('@tauri-apps/api/event');
    un1 = await listen<ActivityEvent>('agency-agent-activity', e => {
      setActivities(prev => [...prev.slice(-99), { ...e.payload, at: Date.now() }]);
      setActiveRunId(e.payload.run_id);
    });
    un2 = await listen<ProgressEvent>('agency-run-progress', e => {
      setProgress(prev => [...prev.slice(-99), { ...e.payload, at: Date.now() }]);
      setActiveRunId(e.payload.run_id);
    });
    un3 = await listen('agency-board-changed', () => {
      qc.invalidateQueries({ queryKey: ['agency-board', activeRunId] });
    });
  })();
  return () => { un1?.(); un2?.(); un3?.(); };
}, [activeRunId]);

const { data: board } = useQuery({
  queryKey: ['agency-board', activeRunId],
  queryFn: () => listBoard(activeRunId!),
  enabled: !!activeRunId,
  refetchInterval: 10_000,
});
```

TS 类型（agency.ts 追加）：

```ts
export interface BoardItem {
  id: string;
  run_id: string;
  story_id: string;
  zone: 'asset' | 'draft' | 'review' | 'schedule';
  item_type: string;
  key: string;
  content: string;
  summary: string;
  version: number;
  producer: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface AgencyRun {
  id: string;
  story_id: string | null;
  premise: string;
  status: string;
  phase: string;
  result_json: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export function getRun(runId: string) {
  return loggedInvoke<AgencyRun | null>('agency_get_run', { run_id: runId });
}

export function listBoard(runId: string) {
  return loggedInvoke<BoardItem[]>('agency_list_board', { run_id: runId });
}
```

测试（渲染三状态卡骨架 + 空态文案，mock agency.ts 与 listen）：

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@/services/api/agency', () => ({
  listBoard: vi.fn().mockResolvedValue([]),
  getRun: vi.fn().mockResolvedValue(null),
}));
vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: any) => sel({ currentStory: { id: 's1', title: '工作室书' } }),
}));

import AgencyStudio from '../AgencyStudio';

describe('AgencyStudio', () => {
  it('渲染三角色状态卡与黑板空态', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={qc}><AgencyStudio /></QueryClientProvider>);
    expect(await screen.findByText('主创')).toBeInTheDocument();
    expect(await screen.findByText('管理')).toBeInTheDocument();
    expect(await screen.findByText('编辑审计')).toBeInTheDocument();
    expect(await screen.findByText(/暂无活动/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 1: 后端 emit_activity 补全 + 编译**（`cargo check` + 全量 `cargo test --lib`）。
- [ ] **Step 2: 前端封装 + 页面 + 注册 + vitest**。
- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs src-frontend/src/
git commit -m "feat(agency): studio page with live role cards, board view, activity timeline"
```

---

### Task 7: P4 转项小批次（eval CI step / checkpoint 对比 UI / story 级 token 聚合 / 追读力口径统一）

**Files:**
- Modify: `.github/workflows/build.yml`（rust-check 加 agency eval 专用 step）
- Modify: `src-frontend/src/pages/AgencyEval.tsx`（checkpoint 对比区块）
- Modify: `src-tauri/src/agency/commands.rs`（EvalOverview 加 `story_tokens` 字段）
- Modify: `src-tauri/src/agency/graders.rs`（追读力归一化改生产口径）
- Modify: `src-frontend/src/services/api/agency.ts`（StoryTokens 类型 + overview 字段）

**逐项规格：**

1. **eval CI step**（build.yml，插在 `Run Rust tests`（:98）之后）：

```yaml
      - name: Run agency eval scenarios
        if: matrix.os != 'windows-latest'
        working-directory: src-tauri
        run: cargo test --lib agency::eval_harness
```

（不加 continue-on-error——eval 场景失败即红，与 V092 基线失败的豁免隔离。）

2. **checkpoint 对比 UI**（AgencyEval.tsx 追加区块，`listCheckpoints`/`compareCheckpoints` 封装已存在）：

```tsx
function CheckpointCompare({ storyId }: { storyId: string }) {
  const { data: checkpoints } = useQuery({
    queryKey: ['agency-checkpoints', storyId],
    queryFn: () => listCheckpoints(storyId),
    enabled: !!storyId,
  });
  const [a, setA] = useState('');
  const [b, setB] = useState('');
  const { data: diff } = useQuery({
    queryKey: ['agency-checkpoint-diff', a, b],
    queryFn: () => compareCheckpoints(a, b),
    enabled: !!a && !!b && a !== b,
  });
  if (!checkpoints || checkpoints.length < 2) return null;
  return (
    <section>
      <h2 className="mb-2 font-medium">检查点对比</h2>
      <div className="flex gap-2">
        <select value={a} onChange={e => setA(e.target.value)} className="rounded border px-2 py-1 text-sm">
          <option value="">基准…</option>
          {checkpoints.map(c => <option key={c.id} value={c.id}>{c.milestone}{c.chapter_number != null ? ` · 第${c.chapter_number}章` : ''} · {c.created_at.slice(0, 16)}</option>)}
        </select>
        <select value={b} onChange={e => setB(e.target.value)} className="rounded border px-2 py-1 text-sm">
          <option value="">对比…</option>
          {checkpoints.map(c => <option key={c.id} value={c.id}>{c.milestone}{c.chapter_number != null ? ` · 第${c.chapter_number}章` : ''} · {c.created_at.slice(0, 16)}</option>)}
        </select>
      </div>
      {diff && (
        <div className="mt-2 grid grid-cols-4 gap-2 text-center text-sm">
          <div className="rounded border p-2"><div className="text-gray-500">字数</div><div>{diff.words_delta >= 0 ? '+' : ''}{diff.words_delta}</div></div>
          <div className="rounded border p-2"><div className="text-gray-500">章节</div><div>{diff.chapters_delta >= 0 ? '+' : ''}{diff.chapters_delta}</div></div>
          <div className="rounded border p-2"><div className="text-gray-500">tokens</div><div>{diff.tokens_delta >= 0 ? '+' : ''}{diff.tokens_delta}</div></div>
          <div className="rounded border p-2"><div className="text-gray-500">加权分</div><div>{diff.gate_weighted_delta >= 0 ? '+' : ''}{diff.gate_weighted_delta.toFixed(2)}</div></div>
        </div>
      )}
    </section>
  );
}
```

（在 AgencyEval 页面内调用 `<CheckpointCompare storyId={storyId} />`，import listCheckpoints/compareCheckpoints/useState。）

3. **story 级 token 聚合**（EvalOverview 加字段；llm_calls 全局段保留并已在 UI 标注"（全局）"）：

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoryTokens {
    pub total_tokens: i64,
    pub run_count: i64,
}

// EvalOverview 加：
pub story_tokens: StoryTokens,

// eval_overview 内（json1 已可用，bundled rusqlite 0.39）：
let story_tokens = conn.query_row(
    "SELECT COALESCE(SUM(t), 0), COUNT(*) FROM (
       SELECT run_id, MAX(CAST(json_extract(metrics_json, '$.tokens_used') AS INTEGER)) AS t
       FROM agency_checkpoints WHERE story_id = ?1 GROUP BY run_id
     )",
    rusqlite::params![story_id],
    |r| Ok(StoryTokens { total_tokens: r.get(0)?, run_count: r.get(1)? }),
).map_err(AppError::from)?;
```

前端：`StoryTokens` 类型 + overview 接口加 `story_tokens`；AgencyEval 在 token 用量区加一行"本故事累计（检查点）：{total} tokens / {runs} runs"。

4. **追读力口径统一**（graders.rs `reading_power_score_of`）：coolpoint/micropayoff 归一化改生产口径（reading_power/mod.rs:112-115 的 cap）：

```rust
// 生产口径（reading_power/mod.rs:112-115）：每命中 +0.1，coolpoint 上限 0.8，micropayoff 上限 0.4
let coolpoint = (features.coolpoint_count as f64 * 0.1).min(0.8);
let micropayoff = (features.micropayoff_count as f64 * 0.1).min(0.4);
hook_score * 0.4 + coolpoint * 0.3 + micropayoff * 0.3
```

**注意**：此改动会拉低现有 fixture 的 rule 分（pass_grade_content 原 ≈0.66 可能降至 ≈0.45，weighted ≈0.76 仍过 0.75 但余量变薄）——跑全量测试，若有测试翻 revise：优先增强 fixture（加爽点/微兑现命中），**不得放宽阈值或退回旧口径**；grader 单测的期望值同步修正并在报告记录。

- [ ] **Step 1: eval CI step + 追读力口径 + Rust 测试修正**（全量 cargo test 验证）。
- [ ] **Step 2: story_tokens 后端 + 前端类型 + checkpoint 对比区块 + vitest**。
- [ ] **Step 3: Commit**

```bash
git add .github/workflows/build.yml src-tauri/src/agency/ src-frontend/src/
git commit -m "feat(agency): eval ci gate, checkpoint compare ui, per-story tokens, reading-power parity"
```

---

### Task 8: 发布就绪（0.30.0 + 文档 + 全量验证）

**Files:**
- Modify: 版本四处（0.29.0 → **0.30.0**）+ 双 lockfile
- Modify: `ARCHITECTURE.md`（agency P5 小节：learning 双轨/instinct/晋升/学习中心/工作室）、`PROJECT_STATUS.md`、`docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（状态行 → **"P1-P5 已完成"**）、CHANGELOG（0.30.0 条目）

**CHANGELOG 条目：**

```markdown
## v0.30.0（2026-07-19）

### Agency P5：持续学习 + 代理可视化（框架收官）
- 持续学习双轨：观察层（observations.jsonl，10MB 轮转，防自观察）→ 后台 analyzer（Background 档）→ instinct（trigger/action/confidence 文件层）
- 置信度引擎：按证据初始化 + 采纳 +0.05 / 纠正 −0.1 / 周衰减 −0.02 / prune
- 晋升管线：≥0.8 且跨 story 复现 → 学习中心确认 → 物化为 skill.yaml 技能（重启自动 reload）
- 学习中心页：模式列表 + 置信度 + 晋升提案 + 观察流 + 手动分析
- 代理工作室页：三角色实时状态卡 + 黑板视图（事件驱动刷新）+ 活动时间线
- eval 场景纳入 CI 专用门禁 step；检查点对比 UI；story 级 token 聚合；追读力口径统一
```

- [ ] **Step 1: 版本与文档** → **Step 2: 全量验证**（cargo test --lib / vitest / type-check / build / guard）→ **Step 3: Commit**（`release: v0.30.0 agency continuous learning + studio (P5)`）。

**真机验收（用户执行，端到端学习闭环）：**
1. 连续创作 3+ 章（含至少一次修订）→ 学习中心"立即分析"→ 出现 instinct；
2. 第二个 story 再产出同模式观察并分析 → 晋升提案出现 → "确认为技能"→ 技能页可见 `learned.*` 技能；重启应用后技能仍在（reload 验证）；
3. 代理工作室：创世/续写期间三角色状态卡实时变化、黑板分区条目流入、时间线滚动；
4. 创作评估页：检查点对比两个里程碑的 delta。

---

## Self-Review（计划自审结论）

- **Spec coverage**：设计 P5 行（观察→instinct→晋升双轨、学习中心、代理工作室完整版）→ T1-T6 全覆盖；ECC 持续学习五行映射逐项对应（hook 观察→T1、analyzer→T2、置信度参数→T3（0.05/−0.1/−0.02 同 ECC）、promote/evolve→T4（人工确认门槛）、防自观察→T1 label 过滤 + analyzer label override）；前端完整可视化 §8 → T5/T6（评估仪表盘 P4 已交付）；P4 终审转项四项 → T7 全覆盖（eval CI/checkpoint UI/story tokens/追读力口径）。
- **Placeholder scan**：T6 页面结构为骨架级描述而非完整代码——该任务验收以"三卡/黑板/时间线渲染 + 事件接入"为准（vitest 锁定），具体 JSX 布局允许实现者按 AgencyEval 模式发挥；无 TBD/待填逻辑。
- **Type consistency**：`Observation/Instinct/AnalyzeOutcome/PromoteOutcome/LearningOverview`（T1-T5）前后一致且 TS 镜像；`ObservationLogger::new(app_dir)` 在 T1 定义、T2-T5 统一复用；`AgencyLlm::new(app, run_id, role, story_id)`（T1 签名变更）在 T2 commands 层按新签名调用；`materialize_as_skill` 的 skills_dir 参数与 `get_default_skills_dir()`（T4 改 pub）一致。
- **风险备案**：T1 的 AgencyLlm 签名变更是 P 内第三次——调用点少（run_role_loop/evaluate_gate_impl/concept/finalize/gate_runner/commands analyzer）已枚举；T7 追读力口径变更可能翻 fixture，已给"增强 fixture 不动阈值"的硬规则；lib.rs 的 reload_skills 是启动行为变更，已在 T4 要求报告适配方式。
