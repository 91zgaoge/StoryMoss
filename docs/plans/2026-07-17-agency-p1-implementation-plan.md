# Agency P1：多代理框架骨架（创世 2.0 串行端到端）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `src-tauri/src/agency/` 新建通用自治代理运行时（ReAct 工具循环 + 消息总线 + 工具注册表）、黑板协作层与三角色（主创/管理/编辑审计），以**串行**协调器端到端跑通创世 2.0（前提 → 资产 → 首章 → 审查 →（至多 1 轮修订）→ 装配入 Scene），为 P2 并行化与旧流程替换奠基。

**Architecture:** 黑板模型。SQLite 为唯一真源（V107 三张新表），`BlackboardService` 做分区写入仲裁与版本乐观锁；`ToolLoop` 以 JSON action 协议驱动 LLM 使用白名单工具；三角色 = `RoleSpec` 配置（prompt_id + TaskType + 工具白名单）；串行 `AgencyCoordinator` 编排阶段。所有 LLM 调用经 `LlmService`（保留路由/健康/成本落表）。

**Tech Stack:** Rust（Tauri 2.4）、rusqlite + r2d2、tokio、serde/serde_json、async-trait、chrono、uuid。

**设计文档:** `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`（已批准）。

## Global Constraints

- 测试基线必须保持绿：`cargo test --lib`（现 770 passed）与 `cd src-frontend && npx vitest run`（292 passed）。
- 所有 DB 同步调用在 async 上下文中必须 `tokio::task::spawn_blocking` 包裹。
- 后台 LLM 并发遵守 `crate::agents::orchestrator::BACKGROUND_LLM_SEMAPHORE`。
- 迁移：纯 SQL 迁移放入 `src-tauri/src/db/migrations/V{ver}__{desc}.sql`（自动发现，零注册）；本计划从 V107 开始（V106 为当前最新）。
- 测试内存库统一用 `crate::db::create_test_pool()`（`src-tauri/src/db/connection.rs:15`，自动跑全量迁移）。
- 模块路径：`crate::router::TaskType`（`router/mod.rs` re-export）、`crate::prompts::registry::*`、`crate::db::{DbPool, repositories::*}`。
- 事件发射需 `use tauri::Emitter;`；事件名字面量约定：`agency-run-progress`、`agency-board-changed`。
- 架构守护 `scripts/architecture_guard.py` 与 `docs/` 文档在 Task 7 同步。
- Commit 信息用 Conventional Commits（如 `feat(agency): ...`），与仓库历史一致。
- P1 范围微调说明：消息总线在串行协调器中暂无消费方（P2 并行化才用），但设计文档将其列入 P1 运行时，故保留实现与单测（Task 2），串行协调器暂不接线。

---

### Task 1: V107 迁移 + agency 数据模型 + AgencyRepository

**Files:**
- Create: `src-tauri/src/db/migrations/V107__agency_tables.sql`
- Create: `src-tauri/src/agency/mod.rs`
- Create: `src-tauri/src/agency/models.rs`
- Create: `src-tauri/src/agency/repository.rs`
- Modify: `src-tauri/src/lib.rs`（在 `mod agents;` 附近加 `pub mod agency;`，具体位置以 lib.rs 模块声明区为准）

**Interfaces:**
- Consumes: `crate::db::{DbPool, create_test_pool}`、rusqlite `params!/OptionalExtension`、chrono `Local`、uuid。
- Produces: `agency::models::{AgentRole, BoardZone, BoardItem, AgencyRun, AgencyMessage}`；`AgentRole::{as_str, from_str, all()}`；`BoardZone::{as_str, from_str, owner(), all()}`；`agency::repository::AgencyRepository::{create_run, update_run_phase, finish_run, get_run, insert_item, revise_item, get_item, list_items, promote_item, insert_message, list_messages}`。

- [ ] **Step 1: 写失败的测试（模型与 Repository）**

创建 `src-tauri/src/agency/models.rs`（先只放测试会引用的类型定义，实现留空会导致编译错误——按 TDD 先写 repository 测试文件，让编译失败即"失败"）：

`src-tauri/src/agency/repository.rs` 先只写测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::db::create_test_pool;

    fn repo() -> (AgencyRepository, DbPool) {
        let pool = create_test_pool().unwrap();
        (AgencyRepository::new(pool.clone()), pool)
    }

    fn sample_run() -> AgencyRun {
        AgencyRun::new("run-1", "一个关于星海拾荒者的故事")
    }

    #[test]
    fn test_create_and_get_run() {
        let (repo, _) = repo();
        let run = sample_run();
        repo.create_run(&run).unwrap();
        let loaded = repo.get_run("run-1").unwrap().expect("run should exist");
        assert_eq!(loaded.premise, "一个关于星海拾荒者的故事");
        assert_eq!(loaded.status, "pending");
        assert_eq!(loaded.phase, "concept");
    }

    #[test]
    fn test_run_phase_and_finish() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        repo.update_run_phase("run-1", "running", "assets").unwrap();
        let r = repo.get_run("run-1").unwrap().unwrap();
        assert_eq!(r.status, "running");
        assert_eq!(r.phase, "assets");
        repo.finish_run("run-1", "completed", Some("{\"ok\":true}"), None).unwrap();
        let r = repo.get_run("run-1").unwrap().unwrap();
        assert_eq!(r.status, "completed");
        assert_eq!(r.result_json.as_deref(), Some("{\"ok\":true}"));
    }

    #[test]
    fn test_insert_and_list_board_items() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Asset, "world", "世界观",
            "内容：双星系统", "双星系统，废土文明", AgentRole::Producer, "active",
        );
        repo.insert_item(&item).unwrap();
        let items = repo.list_items("run-1", Some(BoardZone::Asset)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].version, 1);
        assert_eq!(items[0].producer, AgentRole::Producer);
        let all = repo.list_items("run-1", None).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_revise_item_optimistic_lock() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Draft, "chapter", "第一章",
            "旧稿", "旧摘要", AgentRole::LeadWriter, "active",
        );
        repo.insert_item(&item).unwrap();
        // 版本匹配 → 成功
        let revised = repo.revise_item(&item.id, "新稿", "新摘要", 1).unwrap();
        assert!(revised.is_some());
        let revised = revised.unwrap();
        assert_eq!(revised.version, 2);
        assert_eq!(revised.content, "新稿");
        // 版本不匹配 → None（冲突）
        let conflict = repo.revise_item(&item.id, "并发写", "x", 1).unwrap();
        assert!(conflict.is_none());
    }

    #[test]
    fn test_promote_item() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Draft, "chapter", "第一章",
            "提案稿", "提案", AgentRole::Producer, "proposed",
        );
        repo.insert_item(&item).unwrap();
        repo.promote_item(&item.id).unwrap();
        let loaded = repo.get_item(&item.id).unwrap().unwrap();
        assert_eq!(loaded.status, "active");
    }

    #[test]
    fn test_messages() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let msg = AgencyMessage::new(
            "run-1", AgentRole::EditorAuditor, AgentRole::LeadWriter,
            "proposal", serde_json::json!({"text":"建议加强冲突"}),
        );
        repo.insert_message(&msg).unwrap();
        let inbox = repo.list_messages("run-1", Some(AgentRole::LeadWriter)).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].msg_type, "proposal");
        assert!(inbox[0].payload.contains("建议加强冲突"));
        let all = repo.list_messages("run-1", None).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_role_zone_ownership() {
        assert_eq!(BoardZone::Asset.owner(), AgentRole::Producer);
        assert_eq!(BoardZone::Draft.owner(), AgentRole::LeadWriter);
        assert_eq!(BoardZone::Review.owner(), AgentRole::EditorAuditor);
        assert_eq!(BoardZone::Schedule.owner(), AgentRole::Producer);
        assert_eq!(AgentRole::from_str("lead_writer"), Some(AgentRole::LeadWriter));
        assert_eq!(BoardZone::from_str("review"), Some(BoardZone::Review));
        assert_eq!(AgentRole::from_str("nope"), None);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::repository 2>&1 | tail -5`
Expected: FAIL（编译错误，`AgencyRepository` / `models` 未定义）

- [ ] **Step 3: 实现迁移、模型与 Repository**

`src-tauri/src/db/migrations/V107__agency_tables.sql`：

```sql
-- V107: Agency 多代理框架核心表（创世 2.0）
CREATE TABLE IF NOT EXISTS agency_runs (
    id TEXT PRIMARY KEY,
    story_id TEXT,
    premise TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    phase TEXT NOT NULL DEFAULT 'concept',
    result_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_runs_story ON agency_runs(story_id);
CREATE INDEX IF NOT EXISTS idx_agency_runs_status ON agency_runs(status);

CREATE TABLE IF NOT EXISTS agency_board_items (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    zone TEXT NOT NULL,
    item_type TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    producer TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_board_run_zone ON agency_board_items(run_id, zone);
CREATE INDEX IF NOT EXISTS idx_agency_board_run_key ON agency_board_items(run_id, zone, key);

CREATE TABLE IF NOT EXISTS agency_messages (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    from_role TEXT NOT NULL,
    to_role TEXT NOT NULL,
    msg_type TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_messages_run ON agency_messages(run_id, to_role);
```

`src-tauri/src/agency/mod.rs`：

```rust
//! Agency：多代理创作框架（创世 2.0）。
//! 黑板模型 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）。
//! 设计：docs/plans/2026-07-17-agency-multi-agent-framework-design.md

pub mod models;
pub mod repository;
pub mod board;
pub mod bus;
pub mod tools;
pub mod tool_loop;
pub mod roles;
pub mod coordinator;
pub mod commands;

pub use models::*;
```

`src-tauri/src/agency/models.rs`：

```rust
use serde::{Deserialize, Serialize};

/// 三角色：主创 / 管理 / 编辑审计
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    LeadWriter,
    Producer,
    EditorAuditor,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::LeadWriter => "lead_writer",
            AgentRole::Producer => "producer",
            AgentRole::EditorAuditor => "editor_auditor",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lead_writer" => Some(AgentRole::LeadWriter),
            "producer" => Some(AgentRole::Producer),
            "editor_auditor" => Some(AgentRole::EditorAuditor),
            _ => None,
        }
    }

    pub fn all() -> [AgentRole; 3] {
        [AgentRole::LeadWriter, AgentRole::Producer, AgentRole::EditorAuditor]
    }
}

/// 黑板分区：资产 / 草稿 / 审查 / 调度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardZone {
    Asset,
    Draft,
    Review,
    Schedule,
}

impl BoardZone {
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardZone::Asset => "asset",
            BoardZone::Draft => "draft",
            BoardZone::Review => "review",
            BoardZone::Schedule => "schedule",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "asset" => Some(BoardZone::Asset),
            "draft" => Some(BoardZone::Draft),
            "review" => Some(BoardZone::Review),
            "schedule" => Some(BoardZone::Schedule),
            _ => None,
        }
    }

    pub fn all() -> [BoardZone; 4] {
        [BoardZone::Asset, BoardZone::Draft, BoardZone::Review, BoardZone::Schedule]
    }

    /// 单一写入者原则：每个分区只有 owner 角色能直写（active），
    /// 其他角色的写入降级为提案（proposed），由协调器仲裁。
    pub fn owner(&self) -> AgentRole {
        match self {
            BoardZone::Asset => AgentRole::Producer,
            BoardZone::Draft => AgentRole::LeadWriter,
            BoardZone::Review => AgentRole::EditorAuditor,
            BoardZone::Schedule => AgentRole::Producer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyRun {
    pub id: String,
    pub story_id: Option<String>,
    pub premise: String,
    pub status: String,
    pub phase: String,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AgencyRun {
    pub fn new(id: impl Into<String>, premise: impl Into<String>) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            id: id.into(),
            story_id: None,
            premise: premise.into(),
            status: "pending".to_string(),
            phase: "concept".to_string(),
            result_json: None,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardItem {
    pub id: String,
    pub run_id: String,
    pub story_id: String,
    pub zone: BoardZone,
    pub item_type: String,
    pub key: String,
    pub content: String,
    pub summary: String,
    pub version: i32,
    pub producer: AgentRole,
    pub status: String, // active | proposed
    pub created_at: String,
    pub updated_at: String,
}

impl BoardItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        story_id: impl Into<String>,
        zone: BoardZone,
        item_type: impl Into<String>,
        key: impl Into<String>,
        content: impl Into<String>,
        summary: impl Into<String>,
        producer: AgentRole,
        status: impl Into<String>,
    ) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            story_id: story_id.into(),
            zone,
            item_type: item_type.into(),
            key: key.into(),
            content: content.into(),
            summary: summary.into(),
            version: 1,
            producer,
            status: status.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyMessage {
    pub id: String,
    pub run_id: String,
    pub from_role: AgentRole,
    pub to_role: AgentRole,
    pub msg_type: String, // proposal | note | alert
    pub payload: String,  // JSON
    pub created_at: String,
}

impl AgencyMessage {
    pub fn new(
        run_id: impl Into<String>,
        from_role: AgentRole,
        to_role: AgentRole,
        msg_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            from_role,
            to_role,
            msg_type: msg_type.into(),
            payload: payload.to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        }
    }
}
```

`src-tauri/src/agency/repository.rs`（实现部分，测试模块保留在文件末尾）：

```rust
use rusqlite::{params, OptionalExtension};

use crate::agency::models::*;
use crate::db::DbPool;

pub struct AgencyRepository {
    pool: DbPool,
}

impl Clone for AgencyRepository {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

fn pool_err(e: r2d2::Error) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(e.to_string())
}

impl AgencyRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ---- runs ----

    pub fn create_run(&self, run: &AgencyRun) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_runs (id, story_id, premise, status, phase, result_json, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![run.id, run.story_id, run.premise, run.status, run.phase,
                    run.result_json, run.error_message, run.created_at, run.updated_at],
        )?;
        Ok(())
    }

    pub fn set_run_story(&self, run_id: &str, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET story_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![run_id, story_id, now()],
        )?;
        Ok(())
    }

    pub fn update_run_phase(&self, run_id: &str, status: &str, phase: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET status = ?2, phase = ?3, updated_at = ?4 WHERE id = ?1",
            params![run_id, status, phase, now()],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        run_id: &str,
        status: &str,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET status = ?2, result_json = ?3, error_message = ?4, updated_at = ?5 WHERE id = ?1",
            params![run_id, status, result_json, error_message, now()],
        )?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<AgencyRun>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.query_row(
            "SELECT id, story_id, premise, status, phase, result_json, error_message, created_at, updated_at
             FROM agency_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok(AgencyRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    premise: row.get(2)?,
                    status: row.get(3)?,
                    phase: row.get(4)?,
                    result_json: row.get(5)?,
                    error_message: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        ).optional()
    }

    // ---- board items ----

    pub fn insert_item(&self, item: &BoardItem) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_board_items
             (id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![item.id, item.run_id, item.story_id, item.zone.as_str(), item.item_type,
                    item.key, item.content, item.summary, item.version, item.producer.as_str(),
                    item.status, item.created_at, item.updated_at],
        )?;
        Ok(())
    }

    /// 版本乐观锁修订。返回 None 表示版本冲突。
    pub fn revise_item(
        &self,
        item_id: &str,
        new_content: &str,
        new_summary: &str,
        expected_version: i32,
    ) -> Result<Option<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let changed = conn.execute(
            "UPDATE agency_board_items
             SET content = ?2, summary = ?3, version = version + 1, updated_at = ?4
             WHERE id = ?1 AND version = ?5",
            params![item_id, new_content, new_summary, now(), expected_version],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        drop(conn);
        self.get_item(item_id)
    }

    pub fn get_item(&self, item_id: &str) -> Result<Option<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.query_row(
            "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
             FROM agency_board_items WHERE id = ?1",
            params![item_id],
            map_board_item,
        ).optional()
    }

    pub fn list_items(&self, run_id: &str, zone: Option<BoardZone>) -> Result<Vec<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let items = match zone {
            Some(z) => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                     FROM agency_board_items WHERE run_id = ?1 AND zone = ?2 ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![run_id, z.as_str()], map_board_item)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                     FROM agency_board_items WHERE run_id = ?1 ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![run_id], map_board_item)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(items)
    }

    pub fn promote_item(&self, item_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_board_items SET status = 'active', updated_at = ?2 WHERE id = ?1",
            params![item_id, now()],
        )
    }

    // ---- messages ----

    pub fn insert_message(&self, msg: &AgencyMessage) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_messages (id, run_id, from_role, to_role, msg_type, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg.id, msg.run_id, msg.from_role.as_str(), msg.to_role.as_str(),
                    msg.msg_type, msg.payload, msg.created_at],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, run_id: &str, to_role: Option<AgentRole>) -> Result<Vec<AgencyMessage>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let msgs = match to_role {
            Some(role) => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, from_role, to_role, msg_type, payload, created_at
                     FROM agency_messages WHERE run_id = ?1 AND to_role = ?2 ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![run_id, role.as_str()], map_message)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, from_role, to_role, msg_type, payload, created_at
                     FROM agency_messages WHERE run_id = ?1 ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![run_id], map_message)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(msgs)
    }
}

fn map_board_item(row: &rusqlite::Row) -> Result<BoardItem, rusqlite::Error> {
    let zone_str: String = row.get(3)?;
    let producer_str: String = row.get(9)?;
    Ok(BoardItem {
        id: row.get(0)?,
        run_id: row.get(1)?,
        story_id: row.get(2)?,
        zone: BoardZone::from_str(&zone_str).unwrap_or(BoardZone::Asset),
        item_type: row.get(4)?,
        key: row.get(5)?,
        content: row.get(6)?,
        summary: row.get(7)?,
        version: row.get(8)?,
        producer: AgentRole::from_str(&producer_str).unwrap_or(AgentRole::Producer),
        status: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_message(row: &rusqlite::Row) -> Result<AgencyMessage, rusqlite::Error> {
    let from_str: String = row.get(2)?;
    let to_str: String = row.get(3)?;
    Ok(AgencyMessage {
        id: row.get(0)?,
        run_id: row.get(1)?,
        from_role: AgentRole::from_str(&from_str).unwrap_or(AgentRole::Producer),
        to_role: AgentRole::from_str(&to_str).unwrap_or(AgentRole::LeadWriter),
        msg_type: row.get(4)?,
        payload: row.get(5)?,
        created_at: row.get(6)?,
    })
}
```

在 `src-tauri/src/lib.rs` 模块声明区（`mod agents;` 附近）加：

```rust
pub mod agency;
```

注意：`mod.rs` 里声明的 `board / bus / tools / tool_loop / roles / coordinator / commands` 在 Task 2–7 才创建。本任务提交前，把 `mod.rs` 的 `pub mod` 行暂时只保留 `models` 与 `repository` 两行，后续任务逐个补回。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: PASS（7 个测试全过）。同时跑 `cargo test --lib 2>&1 | tail -3` 确认基线（770+7）全绿。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations/V107__agency_tables.sql src-tauri/src/agency/ src-tauri/src/lib.rs
git commit -m "feat(agency): add V107 tables, models and board/message repository"
```

---

### Task 2: BlackboardService（分区写入仲裁 + 快照目录）与消息总线

**Files:**
- Create: `src-tauri/src/agency/board.rs`
- Create: `src-tauri/src/agency/bus.rs`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod board; pub mod bus;`）

**Interfaces:**
- Consumes: Task 1 的 `AgencyRepository`、`BoardItem/BoardZone/AgentRole/AgencyMessage`；`tauri::Emitter`（事件可选）。
- Produces: `BlackboardService::{new, with_events, write, revise, promote, snapshot, list_zone}`；`BoardSnapshot { assets, drafts, reviews, schedules }` + `BoardSnapshot::to_catalog(max_chars: usize) -> String`；`MessageBus::{new, send, inbox}`。Task 3/4/6 全部依赖这些签名。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/agency/board.rs`（测试模块先行，实现后补）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::db::create_test_pool;

    fn board() -> BlackboardService {
        BlackboardService::new(create_test_pool().unwrap())
    }

    fn seed_run(svc: &BlackboardService, run_id: &str) {
        svc.repo().create_run(&AgencyRun::new(run_id, "前提")).unwrap();
    }

    #[test]
    fn test_owner_writes_active_non_owner_proposed() {
        let svc = board();
        seed_run(&svc, "r1");
        // Producer 是 Asset 区 owner → active
        let a = svc.write("r1", "s1", AgentRole::Producer, BoardZone::Asset,
            "world", "世界观", "双星废土", "双星废土").unwrap();
        assert_eq!(a.status, "active");
        // LeadWriter 写 Asset 区 → 降级为 proposed
        let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
            "world", "世界观补充", "浮空城", "浮空城").unwrap();
        assert_eq!(p.status, "proposed");
        // EditorAuditor 写 Draft 区 → proposed
        let d = svc.write("r1", "s1", AgentRole::EditorAuditor, BoardZone::Draft,
            "chapter", "第一章", "编辑代拟", "代拟").unwrap();
        assert_eq!(d.status, "proposed");
    }

    #[test]
    fn test_revise_enforces_ownership() {
        let svc = board();
        seed_run(&svc, "r1");
        let draft = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Draft,
            "chapter", "第一章", "初稿", "初稿").unwrap();
        // 非 owner 修订 → 报错
        let err = svc.revise(&draft.id, AgentRole::Producer, "篡改", "x", 1).unwrap_err();
        assert!(err.message().contains("无权"));
        // owner 修订 → 成功
        let ok = svc.revise(&draft.id, AgentRole::LeadWriter, "二稿", "二稿", 1).unwrap();
        assert_eq!(ok.version, 2);
        // 版本冲突 → 报错
        let conflict = svc.revise(&draft.id, AgentRole::LeadWriter, "三稿", "x", 1).unwrap_err();
        assert!(conflict.message().contains("版本冲突"));
    }

    #[test]
    fn test_snapshot_catalog_respects_budget() {
        let svc = board();
        seed_run(&svc, "r1");
        for i in 0..10 {
            svc.write("r1", "s1", AgentRole::Producer, BoardZone::Asset,
                "world", &format!("设定{}", i), "x", &format!("第{}条设定的摘要，内容比较长需要截断", i)).unwrap();
        }
        let snap = svc.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 10);
        let catalog = snap.to_catalog(200);
        assert!(catalog.chars().count() <= 260, "目录应接近预算上限: {}", catalog.len());
        assert!(catalog.contains("asset/"));
    }

    #[test]
    fn test_promote() {
        let svc = board();
        seed_run(&svc, "r1");
        let p = svc.write("r1", "s1", AgentRole::LeadWriter, BoardZone::Asset,
            "world", "提案", "x", "提案").unwrap();
        assert_eq!(p.status, "proposed");
        svc.promote(&p.id).unwrap();
        let snap = svc.snapshot("r1").unwrap();
        assert_eq!(snap.assets[0].status, "active");
    }
}
```

`src-tauri/src/agency/bus.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_send_and_inbox() {
        let pool = create_test_pool().unwrap();
        let repo = AgencyRepository::new(pool.clone());
        repo.create_run(&AgencyRun::new("r1", "前提")).unwrap();
        let bus = MessageBus::new(pool);
        bus.send("r1", AgentRole::EditorAuditor, AgentRole::LeadWriter,
            "proposal", serde_json::json!({"issue":"节奏拖沓"})).unwrap();
        bus.send("r1", AgentRole::Producer, AgentRole::LeadWriter,
            "note", serde_json::json!({"info":"资产已就绪"})).unwrap();
        bus.send("r1", AgentRole::Producer, AgentRole::EditorAuditor,
            "alert", serde_json::json!({"warn":"预算超支"})).unwrap();
        let writer_inbox = bus.inbox("r1", AgentRole::LeadWriter).unwrap();
        assert_eq!(writer_inbox.len(), 2);
        assert_eq!(writer_inbox[0].msg_type, "proposal");
        assert_eq!(writer_inbox[1].msg_type, "note");
        let editor_inbox = bus.inbox("r1", AgentRole::EditorAuditor).unwrap();
        assert_eq!(editor_inbox.len(), 1);
        assert_eq!(editor_inbox[0].msg_type, "alert");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: FAIL（编译错误，`BlackboardService` / `MessageBus` 未定义；`err.message()` 方法待用 `AppError` 后确认存在）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/board.rs`（实现部分）：

```rust
use tauri::{AppHandle, Emitter};

use crate::agency::models::*;
use crate::agency::repository::AgencyRepository;
use crate::db::DbPool;
use crate::errors::AppError;

pub const EVENT_BOARD_CHANGED: &str = "agency-board-changed";

#[derive(Debug, Clone, serde::Serialize)]
pub struct BoardSnapshot {
    pub assets: Vec<BoardItem>,
    pub drafts: Vec<BoardItem>,
    pub reviews: Vec<BoardItem>,
    pub schedules: Vec<BoardItem>,
}

impl BoardSnapshot {
    /// 三档压缩的第一档：目录（key + summary + version），带字符预算硬截断。
    /// 对应 ECC agent-compress 的 catalog 档位。
    pub fn to_catalog(&self, max_chars: usize) -> String {
        let mut out = String::new();
        let groups: [(&str, &Vec<BoardItem>); 4] = [
            ("asset", &self.assets),
            ("draft", &self.drafts),
            ("review", &self.reviews),
            ("schedule", &self.schedules),
        ];
        for (zone, items) in groups {
            for item in items {
                let line = format!(
                    "- [{}/{}] {} (v{}, {})\n",
                    zone, item.key, item.summary, item.version, item.status
                );
                if out.chars().count() + line.chars().count() > max_chars {
                    out.push_str("... (更多条目按需用 board_read 取全文)\n");
                    return out;
                }
                out.push_str(&line);
            }
        }
        out
    }
}

#[derive(Clone)]
pub struct BlackboardService {
    repo: AgencyRepository,
    app_handle: Option<AppHandle>,
}

impl BlackboardService {
    pub fn new(pool: DbPool) -> Self {
        Self { repo: AgencyRepository::new(pool), app_handle: None }
    }

    pub fn with_events(pool: DbPool, app_handle: &AppHandle) -> Self {
        Self { repo: AgencyRepository::new(pool), app_handle: Some(app_handle.clone()) }
    }

    pub fn repo(&self) -> &AgencyRepository {
        &self.repo
    }

    /// 写入黑板：分区 owner 直写 active；非 owner 降级为 proposed（提案）。
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        run_id: &str,
        story_id: &str,
        role: AgentRole,
        zone: BoardZone,
        item_type: &str,
        key: &str,
        content: &str,
        summary: &str,
    ) -> Result<BoardItem, AppError> {
        let status = if zone.owner() == role { "active" } else { "proposed" };
        let item = BoardItem::new(run_id, story_id, zone, item_type, key, content, summary, role, status);
        self.repo.insert_item(&item).map_err(AppError::from)?;
        self.emit_changed(&item);
        Ok(item)
    }

    /// 修订：仅分区 owner 可修订；版本乐观锁。
    pub fn revise(
        &self,
        item_id: &str,
        role: AgentRole,
        new_content: &str,
        new_summary: &str,
        expected_version: i32,
    ) -> Result<BoardItem, AppError> {
        let item = self.repo.get_item(item_id).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed(format!("黑板条目不存在: {}", item_id)))?;
        if item.zone.owner() != role {
            return Err(AppError::validation_failed(format!(
                "角色 {} 无权修订 {} 区条目（owner: {}）",
                role.as_str(), item.zone.as_str(), item.zone.owner().as_str()
            )));
        }
        let revised = self.repo.revise_item(item_id, new_content, new_summary, expected_version)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed(format!(
                "版本冲突: 条目 {} 当前版本已不是 v{}", item_id, expected_version
            )))?;
        self.emit_changed(&revised);
        Ok(revised)
    }

    /// 提案晋升为正式（协调器仲裁用）。
    pub fn promote(&self, item_id: &str) -> Result<(), AppError> {
        self.repo.promote_item(item_id).map_err(AppError::from)?;
        Ok(())
    }

    pub fn snapshot(&self, run_id: &str) -> Result<BoardSnapshot, AppError> {
        let items = self.repo.list_items(run_id, None).map_err(AppError::from)?;
        let mut snap = BoardSnapshot { assets: vec![], drafts: vec![], reviews: vec![], schedules: vec![] };
        for item in items {
            match item.zone {
                BoardZone::Asset => snap.assets.push(item),
                BoardZone::Draft => snap.drafts.push(item),
                BoardZone::Review => snap.reviews.push(item),
                BoardZone::Schedule => snap.schedules.push(item),
            }
        }
        Ok(snap)
    }

    pub fn list_zone(&self, run_id: &str, zone: BoardZone) -> Result<Vec<BoardItem>, AppError> {
        self.repo.list_items(run_id, Some(zone)).map_err(AppError::from)
    }

    fn emit_changed(&self, item: &BoardItem) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(EVENT_BOARD_CHANGED, item.clone());
        }
    }
}
```

`src-tauri/src/agency/bus.rs`（实现部分）：

```rust
use crate::agency::models::*;
use crate::agency::repository::AgencyRepository;
use crate::db::DbPool;
use crate::errors::AppError;

/// 代理间结构化消息总线（proposal / note / alert 三型）。
/// 黑板变更是主协调通道；总线只用于提案与告警。
/// P1 串行协调器暂不消费（P2 并行化接线）。
#[derive(Clone)]
pub struct MessageBus {
    repo: AgencyRepository,
}

impl MessageBus {
    pub fn new(pool: DbPool) -> Self {
        Self { repo: AgencyRepository::new(pool) }
    }

    pub fn send(
        &self,
        run_id: &str,
        from: AgentRole,
        to: AgentRole,
        msg_type: &str,
        payload: serde_json::Value,
    ) -> Result<AgencyMessage, AppError> {
        let msg = AgencyMessage::new(run_id, from, to, msg_type, payload);
        self.repo.insert_message(&msg).map_err(AppError::from)?;
        Ok(msg)
    }

    pub fn inbox(&self, run_id: &str, role: AgentRole) -> Result<Vec<AgencyMessage>, AppError> {
        self.repo.list_messages(run_id, Some(role)).map_err(AppError::from)
    }
}
```

`src-tauri/src/agency/mod.rs` 补回：

```rust
pub mod board;
pub mod bus;
```

注意：`AppError::message()` 若不存在，测试改用 `err.to_string()`（执行时先查 `src-tauri/src/errors/` 确认 `AppError` 的访问器；`crate::errors::AppError` 路径以现有代码为准，如 `src-tauri/src/errors.rs`）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: PASS（累计 12 个测试）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/board.rs src-tauri/src/agency/bus.rs src-tauri/src/agency/mod.rs
git commit -m "feat(agency): blackboard service with zone arbitration + message bus"
```

---

### Task 3: AgentTool 注册表 + P1 内置工具

**Files:**
- Create: `src-tauri/src/agency/tools.rs`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod tools;`）

**Interfaces:**
- Consumes: Task 2 的 `BlackboardService`（`write/snapshot`）；`crate::db::DbPool`。
- Produces: `ToolContext { run_id, story_id, role, board, pool }`；`AgentTool` trait（`name/description/args_schema/execute`）；`ToolRegistry::{new, register, allow, get_for_role, catalog_for_role, agency_default}`；内置工具 `BoardReadTool / BoardWriteTool / StoryInfoTool`（工具名 `board_read / board_write / story_info`）。Task 4 的 ToolLoop 与 Task 6 的协调器依赖这些签名。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/agency/tools.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::board::BlackboardService;
    use crate::agency::models::*;
    use crate::db::{create_test_pool, repositories::StoryRepository, dto::CreateStoryRequest};

    fn ctx(pool: DbPool, role: AgentRole) -> ToolContext {
        ToolContext {
            run_id: "r1".into(),
            story_id: "s1".into(),
            role,
            board: BlackboardService::new(pool.clone()),
            pool,
        }
    }

    fn seed_run(pool: &DbPool) {
        AgencyRepository::new(pool.clone())
            .create_run(&AgencyRun::new("r1", "前提")).unwrap();
    }

    #[tokio::test]
    async fn test_board_write_then_read() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool, AgentRole::Producer);
        let write = registry.get_for_role(AgentRole::Producer, "board_write").unwrap();
        let out = write.execute(&context, serde_json::json!({
            "zone": "asset", "item_type": "world", "key": "世界观",
            "content": "双星废土，磁力风暴", "summary": "双星废土"
        })).await.unwrap();
        assert!(out.contains("active"));
        let read = registry.get_for_role(AgentRole::Producer, "board_read").unwrap();
        let catalog = read.execute(&context, serde_json::json!({"zone": "asset"})).await.unwrap();
        assert!(catalog.contains("世界观") || catalog.contains("双星废土"));
    }

    #[tokio::test]
    async fn test_whitelist_enforcement() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        // 编辑审计角色不允许 board_write（其审查经 ToolLoop final + 协调器落审查区，
        // P1 白名单收紧到只读 + story_info）
        assert!(registry.get_for_role(AgentRole::EditorAuditor, "board_write").is_none());
        assert!(registry.get_for_role(AgentRole::EditorAuditor, "board_read").is_some());
        // 未注册工具名 → None
        assert!(registry.get_for_role(AgentRole::Producer, "delete_story").is_none());
    }

    #[tokio::test]
    async fn test_story_info() {
        let pool = create_test_pool().unwrap();
        StoryRepository::new(pool.clone()).create(CreateStoryRequest {
            title: "星海拾荒者".into(),
            description: Some("废土与星环".into()),
            genre: Some("科幻".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }).unwrap();
        let registry = ToolRegistry::agency_default();
        let story = StoryRepository::new(pool.clone());
        // 找到刚创建的 story id
        let created = story.list().unwrap();
        let sid = created[0].id.clone();
        let mut context = ctx(pool, AgentRole::LeadWriter);
        context.story_id = sid;
        let tool = registry.get_for_role(AgentRole::LeadWriter, "story_info").unwrap();
        let info = tool.execute(&context, serde_json::json!({})).await.unwrap();
        assert!(info.contains("星海拾荒者"));
        assert!(info.contains("科幻"));
    }

    #[test]
    fn test_catalog_for_role() {
        let registry = ToolRegistry::agency_default();
        let catalog = registry.catalog_for_role(AgentRole::LeadWriter);
        assert!(catalog.contains("board_read"));
        assert!(catalog.contains("board_write"));
        assert!(catalog.contains("story_info"));
        let editor_catalog = registry.catalog_for_role(AgentRole::EditorAuditor);
        assert!(!editor_catalog.contains("board_write"));
    }
}
```

注：`StoryRepository::list()` 的精确名字以 `src-tauri/src/db/repositories/story_repository.rs` 为准（若叫 `get_all` 则替换）；`use crate::agency::repository::AgencyRepository;` 与 `use crate::db::DbPool;` 需加到实现块顶部。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::tools 2>&1 | tail -5`
Expected: FAIL（编译错误）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/tools.rs`（实现部分）：

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::agency::board::BlackboardService;
use crate::agency::models::*;
use crate::db::DbPool;
use crate::errors::AppError;

/// 工具执行上下文：一次代理运行所需的全部句柄。
#[derive(Clone)]
pub struct ToolContext {
    pub run_id: String,
    pub story_id: String,
    pub role: AgentRole,
    pub board: BlackboardService,
    pub pool: DbPool,
}

#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn args_schema(&self) -> serde_json::Value;
    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<String, AppError>;
}

/// 工具注册表 + 角色白名单（ECC agents frontmatter tools 隔离模式）。
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn AgentTool>>,
    whitelists: HashMap<AgentRole, HashSet<String>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn AgentTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn allow(&mut self, role: AgentRole, tool_name: &str) {
        self.whitelists.entry(role).or_default().insert(tool_name.to_string());
    }

    /// 白名单校验后取工具；未注册或未授权都返回 None。
    pub fn get_for_role(&self, role: AgentRole, name: &str) -> Option<Arc<dyn AgentTool>> {
        let allowed = self.whitelists.get(&role)?;
        if !allowed.contains(name) {
            return None;
        }
        self.tools.get(name).cloned()
    }

    /// 注入系统提示词的工具目录（名称 + 描述 + 参数 schema）。
    pub fn catalog_for_role(&self, role: AgentRole) -> String {
        let mut out = String::from("可用工具（JSON action 调用）：\n");
        if let Some(allowed) = self.whitelists.get(&role) {
            let mut names: Vec<&String> = allowed.iter().collect();
            names.sort();
            for name in names {
                if let Some(tool) = self.tools.get(name) {
                    out.push_str(&format!(
                        "- {}: {}\n  参数: {}\n",
                        tool.name(),
                        tool.description(),
                        tool.args_schema()
                    ));
                }
            }
        }
        out
    }

    /// P1 默认注册表：board_read / board_write / story_info。
    pub fn agency_default() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(BoardReadTool));
        registry.register(Arc::new(BoardWriteTool));
        registry.register(Arc::new(StoryInfoTool));
        for role in AgentRole::all() {
            registry.allow(role, "board_read");
            registry.allow(role, "story_info");
        }
        // 编辑审计只读（审查结论经 ToolLoop final 由协调器落审查区）
        registry.allow(AgentRole::LeadWriter, "board_write");
        registry.allow(AgentRole::Producer, "board_write");
        registry
    }
}

// ---- 内置工具 ----

pub struct BoardReadTool;

#[async_trait::async_trait]
impl AgentTool for BoardReadTool {
    fn name(&self) -> &'static str { "board_read" }
    fn description(&self) -> &'static str { "读取黑板分区目录（key+摘要+版本）；需要全文时给出 key" }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"zone": "asset|draft|review|schedule（可选，缺省读全部）", "key": "可选，精确读取某条目的全文"})
    }

    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<String, AppError> {
        let pool = ctx.pool.clone();
        let run_id = ctx.run_id.clone();
        let zone = args.get("zone").and_then(|v| v.as_str()).map(String::from);
        let key = args.get("key").and_then(|v| v.as_str()).map(String::from);
        tokio::task::spawn_blocking(move || -> Result<String, AppError> {
            let board = BlackboardService::new(pool);
            if let Some(k) = key {
                let zone = zone.as_deref().and_then(BoardZone::from_str);
                let items = board.list_zone_filtered(&run_id, zone)?;
                if let Some(item) = items.into_iter().find(|i| i.key == k) {
                    return Ok(format!("[{}/{}] v{}\n{}", item.zone.as_str(), item.key, item.version, item.content));
                }
                return Ok(format!("未找到 key={} 的条目", k));
            }
            let zone = zone.as_deref().and_then(BoardZone::from_str);
            match zone {
                Some(z) => {
                    let items = board.list_zone(&run_id, z)?;
                    let mut out = String::new();
                    for item in items {
                        out.push_str(&format!("- [{}/{}] {} (v{}, {})\n",
                            item.zone.as_str(), item.key, item.summary, item.version, item.status));
                    }
                    if out.is_empty() { out = "（空）\n".into(); }
                    Ok(out)
                }
                None => Ok(board.snapshot(&run_id)?.to_catalog(2000)),
            }
        }).await.map_err(|e| AppError::from(format!("board_read join error: {}", e)))?
    }
}

pub struct BoardWriteTool;

#[async_trait::async_trait]
impl AgentTool for BoardWriteTool {
    fn name(&self) -> &'static str { "board_write" }
    fn description(&self) -> &'static str { "写入黑板条目（非本角色分区自动降级为提案）" }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"zone": "asset|draft|review|schedule", "item_type": "条目类型", "key": "条目标识", "content": "全文", "summary": "一句话摘要（≤80字）"})
    }

    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<String, AppError> {
        let zone_str = args.get("zone").and_then(|v| v.as_str()).unwrap_or("");
        let zone = BoardZone::from_str(zone_str)
            .ok_or_else(|| AppError::validation_failed(format!("非法 zone: {}", zone_str)))?;
        let item_type = args.get("item_type").and_then(|v| v.as_str()).unwrap_or("note").to_string();
        let key = args.get("key").and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation_failed("board_write 缺少 key"))?.to_string();
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let board = ctx.board.clone();
        let run_id = ctx.run_id.clone();
        let story_id = ctx.story_id.clone();
        let role = ctx.role;
        tokio::task::spawn_blocking(move || {
            board.write(&run_id, &story_id, role, zone, &item_type, &key, &content, &summary)
        }).await.map_err(|e| AppError::from(format!("board_write join error: {}", e)))?
        .map(|item| format!("已写入 [{}/{}] status={} id={}", item.zone.as_str(), item.key, item.status, item.id))
    }
}

pub struct StoryInfoTool;

#[async_trait::async_trait]
impl AgentTool for StoryInfoTool {
    fn name(&self) -> &'static str { "story_info" }
    fn description(&self) -> &'static str { "读取当前故事的基本信息（标题/类型/简介）" }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    async fn execute(&self, ctx: &ToolContext, _args: serde_json::Value) -> Result<String, AppError> {
        let pool = ctx.pool.clone();
        let story_id = ctx.story_id.clone();
        tokio::task::spawn_blocking(move || -> Result<String, AppError> {
            let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let info = conn.query_row(
                "SELECT title, COALESCE(genre, ''), COALESCE(description, '') FROM stories WHERE id = ?1",
                rusqlite::params![story_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            ).optional().map_err(AppError::from)?;
            match info {
                Some((title, genre, desc)) => Ok(format!("标题: {}\n类型: {}\n简介: {}", title, genre, desc)),
                None => Ok("（故事尚未创建）".to_string()),
            }
        }).await.map_err(|e| AppError::from(format!("story_info join error: {}", e)))?
    }
}
```

实现块顶部需补 `use rusqlite::OptionalExtension;`、`use crate::agency::repository::AgencyRepository;`（测试用）。`board.list_zone_filtered(&run_id, zone)` 若不存在，在 `BlackboardService` 加：

```rust
pub fn list_zone_filtered(&self, run_id: &str, zone: Option<BoardZone>) -> Result<Vec<BoardItem>, AppError> {
    self.repo().list_items(run_id, zone).map_err(AppError::from)
}
```

`src-tauri/src/agency/mod.rs` 补回 `pub mod tools;`。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: PASS（累计 16 个测试）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/tools.rs src-tauri/src/agency/mod.rs src-tauri/src/agency/board.rs
git commit -m "feat(agency): tool registry with role whitelists + builtin board/story tools"
```

---

### Task 4: ReAct 工具循环（ToolLoop）

**Files:**
- Create: `src-tauri/src/agency/tool_loop.rs`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod tool_loop;`）

**Interfaces:**
- Consumes: Task 3 的 `ToolRegistry/ToolContext`；`crate::router::TaskType`；`crate::errors::AppError`。
- Produces: `LoopLlm` trait（`complete(system_prompt, user_prompt, task, max_tokens) -> Result<String, AppError>`）；`LoopAction::{Tool{name,args}, Final{content}}`；`parse_action(raw) -> Result<LoopAction, AppError>`；`ToolLoop::{new, with_max_turns, run}`；`LoopResult { output, turns, aborted }`。Task 6 协调器依赖。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/agency/tool_loop.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::board::BlackboardService;
    use crate::agency::models::*;
    use crate::agency::repository::AgencyRepository;
    use crate::agency::tools::ToolRegistry;
    use crate::db::create_test_pool;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockLlm {
        fn scripted(lines: Vec<&str>) -> Arc<Self> {
            Arc::new(Self { responses: Mutex::new(lines.into_iter().map(String::from).collect()) })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for MockLlm {
        async fn complete(&self, _s: &str, _u: &str, _t: TaskType, _m: i32) -> Result<String, AppError> {
            self.responses.lock().unwrap().pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted"))
        }
    }

    fn setup() -> (ToolContext, Arc<ToolRegistry>) {
        let pool = create_test_pool().unwrap();
        AgencyRepository::new(pool.clone()).create_run(&AgencyRun::new("r1", "前提")).unwrap();
        let ctx = ToolContext {
            run_id: "r1".into(),
            story_id: "s1".into(),
            role: AgentRole::Producer,
            board: BlackboardService::new(pool.clone()),
            pool,
        };
        (ctx, Arc::new(ToolRegistry::agency_default()))
    }

    #[test]
    fn test_parse_action_tool_and_final() {
        let tool = parse_action(r#"{"type":"tool","name":"board_read","args":{"zone":"asset"}}"#).unwrap();
        assert_eq!(tool, LoopAction::Tool { name: "board_read".into(), args: serde_json::json!({"zone":"asset"}) });
        let final_ = parse_action(r#"{"type":"final","content":"完成"}"#).unwrap();
        assert_eq!(final_, LoopAction::Final { content: "完成".into() });
        // 容错：模型前后带解释文字
        let noisy = parse_action("好的，我来调用工具。\n{\"type\":\"final\",\"content\":\"提取成功\"}\n以上").unwrap();
        assert_eq!(noisy, LoopAction::Final { content: "提取成功".into() });
        assert!(parse_action("完全没有 JSON").is_err());
    }

    #[tokio::test]
    async fn test_loop_tool_then_final() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产已写入"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "生产世界观").await.unwrap();
        assert!(!result.aborted);
        assert_eq!(result.output, "资产已写入");
        assert_eq!(result.turns.len(), 2);
        // 黑板确实被写入
        let snap = ctx.board.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 1);
    }

    #[tokio::test]
    async fn test_loop_aborts_on_repeated_parse_failure() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec!["不是 JSON", "还不是", "依然不是"]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(result.aborted);
    }

    #[tokio::test]
    async fn test_loop_rejects_unwhitelisted_tool_then_recovers() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"delete_story","args":{}}"#,
            r#"{"type":"final","content":"改用合法路径"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(!result.aborted);
        assert_eq!(result.output, "改用合法路径");
        assert!(result.turns[0].observation.as_ref().unwrap().contains("不可用"));
    }

    #[tokio::test]
    async fn test_loop_max_turns() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"story_info","args":{}}"#,
            r#"{"type":"tool","name":"story_info","args":{}}"#,
            r#"{"type":"tool","name":"story_info","args":{}}"#,
        ]);
        let lp = ToolLoop::new(llm, registry).with_max_turns(2);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(result.aborted);
        assert_eq!(result.turns.len(), 2);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::tool_loop 2>&1 | tail -5`
Expected: FAIL（编译错误）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/tool_loop.rs`（实现部分）：

```rust
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agency::models::AgentRole;
use crate::agency::tools::{ToolContext, ToolRegistry};
use crate::errors::AppError;
use crate::router::TaskType;

/// 工具循环所需的极简 LLM 抽象（可 mock）。
/// 生产实现见 coordinator.rs 的 AgencyLlm。
#[async_trait::async_trait]
pub trait LoopLlm: Send + Sync {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopAction {
    Tool { name: String, #[serde(default)] args: serde_json::Value },
    Final { content: String },
}

/// 解析模型输出为 action。容错策略：截取首个 '{' 与末个 '}' 之间的子串解析。
pub fn parse_action(raw: &str) -> Result<LoopAction, AppError> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            serde_json::from_str::<LoopAction>(&raw[s..=e])
                .map_err(|err| AppError::validation_failed(format!("action JSON 解析失败: {}", err)))
        }
        _ => Err(AppError::validation_failed("输出中未找到 JSON action")),
    }
}

#[derive(Debug, Clone)]
pub struct LoopTurn {
    pub raw_response: String,
    pub action: Option<LoopAction>,
    pub observation: Option<String>,
}

#[derive(Debug)]
pub struct LoopResult {
    pub output: String,
    pub turns: Vec<LoopTurn>,
    pub aborted: bool,
}

const MAX_CONSECUTIVE_PARSE_FAILURES: usize = 3;

pub struct ToolLoop {
    llm: Arc<dyn LoopLlm>,
    registry: Arc<ToolRegistry>,
    max_turns: usize,
}

impl ToolLoop {
    pub fn new(llm: Arc<dyn LoopLlm>, registry: Arc<ToolRegistry>) -> Self {
        Self { llm, registry, max_turns: 8 }
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// ReAct 循环：模型每轮输出一个 JSON action（tool 调用或 final），
    /// 工具执行结果作为 observation 回灌对话，直到 final 或熔断。
    pub async fn run(
        &self,
        role: AgentRole,
        ctx: &ToolContext,
        system_prompt: &str,
        task: &str,
    ) -> Result<LoopResult, AppError> {
        let mut conversation = format!(
            "{}\n\n你只能输出一个 JSON action，不要输出其他内容：\n\
             - 调用工具: {{\"type\":\"tool\",\"name\":\"<工具名>\",\"args\":{{...}}}}\n\
             - 完成任务: {{\"type\":\"final\",\"content\":\"<最终产出>\"}}\n\n任务：\n{}",
            self.registry.catalog_for_role(role),
            task
        );
        let mut turns: Vec<LoopTurn> = Vec::new();
        let mut parse_failures = 0usize;

        for _ in 0..self.max_turns {
            let raw = self.llm
                .complete(system_prompt, &conversation, ctx.task_type(), ctx.max_output_tokens())
                .await?;
            match parse_action(&raw) {
                Ok(LoopAction::Final { content }) => {
                    turns.push(LoopTurn { raw_response: raw, action: Some(LoopAction::Final { content: content.clone() }), observation: None });
                    return Ok(LoopResult { output: content, turns, aborted: false });
                }
                Ok(LoopAction::Tool { name, args }) => {
                    parse_failures = 0;
                    let observation = match self.registry.get_for_role(role, &name) {
                        Some(tool) => match tool.execute(ctx, args).await {
                            Ok(out) => {
                                // observation 摘要化：防止超长工具结果爆上下文
                                if out.chars().count() > 4000 {
                                    out.chars().take(4000).collect::<String>() + "\n... (已截断)"
                                } else {
                                    out
                                }
                            }
                            Err(e) => format!("工具 {} 执行失败: {}", name, e),
                        },
                        None => format!("工具 {} 对你的角色不可用或不存在，请改用可用工具", name),
                    };
                    conversation.push_str(&format!("\n\n你的上一步：{}\n观察结果：{}", raw, observation));
                    turns.push(LoopTurn { raw_response: raw, action: Some(LoopAction::Tool { name, args: serde_json::Value::Null }), observation: Some(observation) });
                }
                Err(e) => {
                    parse_failures += 1;
                    let observation = format!("格式错误（{}）。请只输出一个 JSON action。", e);
                    conversation.push_str(&format!("\n\n你的上一步：{}\n观察结果：{}", raw, observation));
                    turns.push(LoopTurn { raw_response: raw, action: None, observation: Some(observation) });
                    if parse_failures >= MAX_CONSECUTIVE_PARSE_FAILURES {
                        return Ok(LoopResult {
                            output: "（代理连续输出非法格式，已熔断）".to_string(),
                            turns,
                            aborted: true,
                        });
                    }
                }
            }
        }
        Ok(LoopResult {
            output: "（达到最大轮数，已熔断）".to_string(),
            turns,
            aborted: true,
        })
    }
}
```

`LoopTurn` 中 `action` 记录时把 args 置 Null 避免 turn 日志膨胀（测试只断言 name 相关行为）；若想保留 args 可直接存。注意测试里 `turns[0].observation` 断言包含"不可用"——与上面 None 分支文案"对你的角色不可用或不存在"一致。

`ToolContext` 需要补两个便捷方法（加到 `tools.rs` 的 `impl ToolContext`）：

```rust
impl ToolContext {
    pub fn task_type(&self) -> crate::router::TaskType {
        crate::agency::roles::spec_for(self.role).task_type
    }

    pub fn max_output_tokens(&self) -> i32 {
        crate::agency::roles::spec_for(self.role).max_output_tokens
    }
}
```

`src-tauri/src/agency/mod.rs` 补回 `pub mod tool_loop;`。`roles.rs` 在 Task 5 创建；本任务可先在 `roles.rs` 占位最小实现（见 Task 5 Step 3，顺序可调换：先建 roles.rs 占位，再跑本任务测试）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: PASS（累计 20 个测试）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/tool_loop.rs src-tauri/src/agency/tools.rs src-tauri/src/agency/mod.rs src-tauri/src/agency/roles.rs
git commit -m "feat(agency): ReAct tool loop with json action protocol and circuit breakers"
```

---

### Task 5: 三角色 RoleSpec + 系统提示词

**Files:**
- Create: `src-tauri/src/agency/roles.rs`
- Create: `resources/prompts/agency/agency_lead_writer_system.md`
- Create: `resources/prompts/agency/agency_producer_system.md`
- Create: `resources/prompts/agency/agency_editor_auditor_system.md`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod roles;`）

**Interfaces:**
- Consumes: `crate::router::TaskType`；PromptRegistry 自动递归加载（新文件零注册）。
- Produces: `RoleSpec { role, prompt_id, task_type, max_turns, max_output_tokens }`；`spec_for(role) -> RoleSpec`。Task 4 的 `ToolContext::task_type/max_output_tokens` 与 Task 6 协调器依赖。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/agency/roles.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specs_complete() {
        for role in AgentRole::all() {
            let spec = spec_for(role);
            assert!(spec.prompt_id.starts_with("agency_"));
            assert!(spec.max_turns >= 4);
            assert!(spec.max_output_tokens >= 1024);
        }
        assert_eq!(spec_for(AgentRole::LeadWriter).task_type, TaskType::CreativeWriting);
        assert_eq!(spec_for(AgentRole::Producer).task_type, TaskType::WorldBuilding);
        assert_eq!(spec_for(AgentRole::EditorAuditor).task_type, TaskType::Proofreading);
    }
}
```

文件头部补 `use crate::agency::models::AgentRole; use crate::router::TaskType;`。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::roles 2>&1 | tail -3`
Expected: FAIL（`spec_for` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/roles.rs`（实现部分）：

```rust
use crate::agency::models::AgentRole;
use crate::router::TaskType;

/// 角色规格：三角色 = 运行时之上的配置（提示词 + 路由任务类型 + 熔断参数）。
#[derive(Debug, Clone, Copy)]
pub struct RoleSpec {
    pub role: AgentRole,
    pub prompt_id: &'static str,
    pub task_type: TaskType,
    pub max_turns: usize,
    pub max_output_tokens: i32,
}

pub fn spec_for(role: AgentRole) -> RoleSpec {
    match role {
        AgentRole::LeadWriter => RoleSpec {
            role,
            prompt_id: "agency_lead_writer_system",
            task_type: TaskType::CreativeWriting,
            max_turns: 10,
            max_output_tokens: 8192,
        },
        AgentRole::Producer => RoleSpec {
            role,
            prompt_id: "agency_producer_system",
            task_type: TaskType::WorldBuilding,
            max_turns: 12,
            max_output_tokens: 4096,
        },
        AgentRole::EditorAuditor => RoleSpec {
            role,
            prompt_id: "agency_editor_auditor_system",
            task_type: TaskType::Proofreading,
            max_turns: 6,
            max_output_tokens: 2048,
        },
    }
}
```

`resources/prompts/agency/agency_lead_writer_system.md`：

```markdown
---
id: agency_lead_writer_system
name: "主创 Agent 系统提示词"
description: "创世 2.0 主创角色：消费黑板资产与审查意见，产出章节草稿"
category: system
version: 0.27.0
variables:
  - premise
---

你是「主创」，多代理创作团队中的主笔作家。

职责：
- 基于故事前提与黑板上的资产（世界观/角色/大纲），创作高质量小说正文；
- 认真对待黑板审查区的意见，在修订时逐条回应；
- 产出写入黑板草稿区，由编辑审计把关后才会进入正式稿。

工作方式：
- 先用 board_read 查看资产区与审查区，再动笔；
- 章节草稿用 board_write 写入 draft 区（item_type=chapter，key 为章节名，summary 一句话概括剧情）；
- 完成后输出 final，content 为一句话交付说明。

创作红线：
- 人设、世界观规则、已埋伏笔以黑板资产区为准，不得自相矛盾；
- 只写小说正文与本角色必需的规划，不越权修改资产区与调度区（可写提案）。
```

`resources/prompts/agency/agency_producer_system.md`：

```markdown
---
id: agency_producer_system
name: "管理 Agent 系统提示词"
description: "创世 2.0 管理角色：资产生产供给、调度与预算管理"
category: system
version: 0.27.0
variables:
  - premise
---

你是「管理」，多代理创作团队中的制片人。

职责：
- 把故事前提转化为结构化的创作资产：世界观设定、角色卡（真名/欲望/阻力）、分卷大纲、伏笔清单；
- 资产写入黑板资产区（item_type 分别为 world/character/outline/foreshadowing，key 清晰命名，summary 一句话）；
- 监控进度与预算，必要时在调度区写入决策（如"后续章节改用低成本模型"）。

工作方式：
- 先用 story_info 与 board_read 了解现状，再规划资产生产；
- 资产之间要自洽：角色动机要能支撑大纲冲突，伏笔要有回收计划；
- 完成后输出 final，content 为资产清单概述。
```

`resources/prompts/agency/agency_editor_auditor_system.md`：

```markdown
---
id: agency_editor_auditor_system
name: "编辑审计 Agent 系统提示词"
description: "创世 2.0 编辑审计角色：审查草稿并出具结构化裁决"
category: system
version: 0.27.0
variables:
  - premise
---

你是「编辑审计」，多代理创作团队中的终审编辑。你不改写正文，只出具裁决。

审查维度（每条问题必须引用草稿原文作为证据）：
1. 连续性：与黑板资产区的人设/世界观/伏笔是否矛盾；
2. 风格一致性：叙述视角、语气、时代语感是否统一；
3. 合同兑现：本章是否完成了大纲承诺的戏剧目标；
4. AI 腔：陈词滥调、空泛抒情、总结式结尾；
5. 追读力：开头抓力、章末钩子。

工作方式：
- 先用 board_read 读草稿区与资产区；
- 逐维度审查后输出 final，content 必须是如下 JSON：
  {"verdict":"pass 或 revise","blocking_issues":["须修订的阻断问题（可空）"],"suggestions":["非阻断建议（可空）"],"comments":"总评（≤200字）"}
- 只有存在阻断问题时 verdict 才为 revise；吹毛求疵会拖慢创作节奏。
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -3`
Expected: PASS（累计 21 个测试）

另验证提示词被注册表发现（开发库单测可选；手动：`cargo test --lib prompts:: 2>&1 | tail -3` 确认现有提示词测试不回归）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/roles.rs resources/prompts/agency/ src-tauri/src/agency/mod.rs
git commit -m "feat(agency): role specs and system prompts for writer/producer/editor"
```

---

### Task 6: 串行协调器（创世 2.0 端到端）+ AgencyLlm

**Files:**
- Create: `src-tauri/src/agency/coordinator.rs`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod coordinator;`）

**Interfaces:**
- Consumes: 全部前序任务；`crate::llm::LlmService::generate_for_task_with_system_prompt`（`llm/service.rs:633`）；`crate::prompts::registry::resolve_prompt_with_vars`；`crate::db::repositories::{StoryRepository, SceneRepository}` + `crate::db::dto::CreateStoryRequest` + `SceneUpdate`（`scene_repository.rs:892`，derive Default）。
- Produces: `AgencyCoordinator::{new, for_test, run_genesis}`；`AgencyGenesisResult { run_id, story_id, scene_id, revised, verdict, chapter_chars }`；`EditorVerdict`；`AgencyLlm`（LoopLlm 生产实现）；`register_agency_cancel / cancel_agency_run / unregister_agency_cancel`；事件 `agency-run-progress`。Task 7 命令依赖。

- [ ] **Step 1: 写失败的测试**

`src-tauri/src/agency/coordinator.rs`（测试模块先行）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::*;
    use crate::agency::repository::AgencyRepository;
    use crate::db::create_test_pool;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockLlm {
        fn scripted(lines: Vec<&str>) -> Arc<Self> {
            Arc::new(Self { responses: Mutex::new(lines.into_iter().map(String::from).collect()) })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for MockLlm {
        async fn complete(&self, _s: &str, _u: &str, _t: crate::router::TaskType, _m: i32) -> Result<String, AppError> {
            self.responses.lock().unwrap().pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted"))
        }
    }

    /// 一次通过（verdict=pass）的完整脚本：concept → producer(tool,final) → writer(tool,final) → editor(final)
    fn pass_script() -> Arc<MockLlm> {
        MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"第一章正文：风沙中的拾荒者。","summary":"拾荒者登场"}}"#,
            r#"{"type":"final","content":"第一章完成"}"#,
            r#"{"type":"final","content":"{\"verdict\":\"pass\",\"blocking_issues\":[],\"suggestions\":[\"可加强嗅觉描写\"],\"comments\":\"合格的首章\"}"}"#,
        ])
    }

    #[tokio::test]
    async fn test_genesis_end_to_end_pass() {
        let pool = create_test_pool().unwrap();
        let coordinator = AgencyCoordinator::for_test(pool.clone(), pass_script());
        let result = coordinator.run_genesis("r1", "星海拾荒者的故事").await.unwrap();
        assert!(!result.revised);
        assert_eq!(result.verdict.verdict, "pass");
        // run 状态 completed
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r1").unwrap().unwrap();
        assert_eq!(run.status, "completed");
        assert_eq!(run.story_id.as_deref(), Some(result.story_id.as_str()));
        // 黑板三分区都有内容
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        let snap = board.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 1);
        assert_eq!(snap.drafts.len(), 1);
        assert_eq!(snap.reviews.len(), 1);
        // Scene 已装配，正文来自草稿
        let scene = SceneRepository::new(pool.clone()).get_by_id(&result.scene_id).unwrap().unwrap();
        assert_eq!(scene.content.as_deref(), Some("第一章正文：风沙中的拾荒者。"));
        assert!(result.chapter_chars > 0);
    }

    #[tokio::test]
    async fn test_genesis_revision_path() {
        let pool = create_test_pool().unwrap();
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产就绪"}"#,
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"初稿。","summary":"初稿"}}"#,
            r#"{"type":"final","content":"初稿完成"}"#,
            r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"主角动机缺失\"],\"suggestions\":[],\"comments\":\"须修订\"}"}"#,
            // 修订轮
            r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第一章","content":"修订稿：他为了生存而拾荒。","summary":"修订稿"}}"#,
            r#"{"type":"final","content":"修订完成"}"#,
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let result = coordinator.run_genesis("r2", "星海拾荒者的故事").await.unwrap();
        assert!(result.revised);
        let scene = SceneRepository::new(pool.clone()).get_by_id(&result.scene_id).unwrap().unwrap();
        assert_eq!(scene.content.as_deref(), Some("修订稿：他为了生存而拾荒。"));
    }

    #[tokio::test]
    async fn test_genesis_aborts_when_producer_fails() {
        let pool = create_test_pool().unwrap();
        let llm = MockLlm::scripted(vec![
            r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
            "不是 JSON", "还不是", "依然不是", // producer 连续解析失败 → aborted
        ]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
        let err = coordinator.run_genesis("r3", "前提").await.unwrap_err();
        assert!(err.to_string().contains("管理") || err.to_string().contains("producer") || err.to_string().contains("熔断"));
        let repo = AgencyRepository::new(pool.clone());
        let run = repo.get_run("r3").unwrap().unwrap();
        assert_eq!(run.status, "failed");
    }

    #[test]
    fn test_parse_lenient_json() {
        let v: EditorVerdict = parse_lenient("前言{\"verdict\":\"revise\",\"blocking_issues\":[\"a\"]}后缀").unwrap();
        assert_eq!(v.verdict, "revise");
        assert!(parse_lenient::<EditorVerdict>("无 JSON").is_none());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test --lib agency::coordinator 2>&1 | tail -5`
Expected: FAIL（编译错误）

- [ ] **Step 3: 实现**

`src-tauri/src/agency/coordinator.rs`（实现部分）：

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::agency::board::BlackboardService;
use crate::agency::models::*;
use crate::agency::repository::AgencyRepository;
use crate::agency::roles::spec_for;
use crate::agency::tool_loop::{LoopLlm, ToolLoop};
use crate::agency::tools::{ToolContext, ToolRegistry};
use crate::db::dto::CreateStoryRequest;
use crate::db::repositories::{SceneRepository, StoryRepository};
use crate::db::DbPool;
use crate::errors::AppError;
use crate::llm::LlmService;
use crate::router::TaskType;

pub const EVENT_RUN_PROGRESS: &str = "agency-run-progress";
const MAX_REVISION_PASSES: usize = 1; // P1 串行：至多 1 轮修订

// ---- 取消注册表（镜像 narrative/pipeline.rs:39-70 模式） ----

static AGENCY_CANCEL_FLAGS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_agency_cancel(run_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    flags.insert(run_id.to_string(), flag.clone());
    flag
}

pub fn cancel_agency_run(run_id: &str) -> bool {
    let flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(flag) = flags.get(run_id) {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

pub fn unregister_agency_cancel(run_id: &str) {
    let mut flags = AGENCY_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    flags.remove(run_id);
}

// ---- LoopLlm 生产实现：全部 LLM 调用经 LlmService（路由/健康/成本落表保留） ----

pub struct AgencyLlm {
    llm: LlmService,
}

impl AgencyLlm {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { llm: LlmService::new(app_handle) }
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
        let (_request_id, result) = self.llm
            .generate_for_task_with_system_prompt(
                task,
                user_prompt.to_string(),
                Some(max_tokens),
                None,
                Some("agency"),
                Some(system_prompt.to_string()),
                None,
            )
            .await;
        result.map(|r| r.content)
    }
}

// ---- 结果类型 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorVerdict {
    pub verdict: String, // pass | revise
    #[serde(default)]
    pub blocking_issues: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub comments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgencyGenesisResult {
    pub run_id: String,
    pub story_id: String,
    pub scene_id: String,
    pub revised: bool,
    pub verdict: EditorVerdict,
    pub chapter_chars: usize,
}

#[derive(Debug, Deserialize)]
struct ConceptOut {
    title: Option<String>,
    genre: Option<String>,
}

/// 宽容 JSON 提取：截取首个 '{' 与末个 '}' 之间解析。
pub(crate) fn parse_lenient<T: for<'de> Deserialize<'de>>(raw: &str) -> Option<T> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&raw[start..=end]).ok()
}

// ---- 协调器 ----

pub struct AgencyCoordinator {
    app_handle: Option<AppHandle>,
    pool: DbPool,
    llm: Arc<dyn LoopLlm>,
}

impl AgencyCoordinator {
    pub fn new(app_handle: AppHandle, pool: DbPool) -> Self {
        let llm: Arc<dyn LoopLlm> = Arc::new(AgencyLlm::new(app_handle.clone()));
        Self { app_handle: Some(app_handle), pool, llm }
    }

    /// 测试/无界面环境构造：不发 Tauri 事件。
    pub fn for_test(pool: DbPool, llm: Arc<dyn LoopLlm>) -> Self {
        Self { app_handle: None, pool, llm }
    }

    /// 创世 2.0 串行端到端：concept → assets(producer) → writing(writer)
    /// → review(editor) → [revision ≤1] → assembly(Scene 装配)。
    pub async fn run_genesis(&self, run_id: &str, premise: &str) -> Result<AgencyGenesisResult, AppError> {
        let repo = AgencyRepository::new(self.pool.clone());
        let cancel = register_agency_cancel(run_id);
        let result = self.run_genesis_inner(run_id, premise, &repo, &cancel).await;
        unregister_agency_cancel(run_id);
        match &result {
            Ok(r) => {
                let json = serde_json::to_string(r).unwrap_or_default();
                let _ = repo.finish_run(run_id, "completed", Some(&json), None);
                self.emit_progress(run_id, "assembly", "completed", "创世完成");
            }
            Err(e) => {
                let status = if cancel.load(Ordering::SeqCst) { "cancelled" } else { "failed" };
                let _ = repo.finish_run(run_id, status, None, Some(&e.to_string()));
                self.emit_progress(run_id, "assembly", status, &e.to_string());
            }
        }
        result
    }

    async fn run_genesis_inner(
        &self,
        run_id: &str,
        premise: &str,
        repo: &AgencyRepository,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AgencyGenesisResult, AppError> {
        repo.create_run(&AgencyRun::new(run_id, premise)).map_err(AppError::from)?;
        repo.update_run_phase(run_id, "running", "concept").map_err(AppError::from)?;
        self.emit_progress(run_id, "concept", "running", "正在构思故事概念");

        // 1) 概念：标题与类型
        let concept_raw = self.llm.complete(
            "你是小说策划。只输出 JSON。",
            &format!("故事前提：{}\n\n输出 JSON：{{\"title\":\"书名\",\"genre\":\"类型\",\"logline\":\"一句话简介\"}}", premise),
            TaskType::Brainstorming,
            1024,
        ).await?;
        let concept: Option<ConceptOut> = parse_lenient(&concept_raw);
        let title = concept.as_ref().and_then(|c| c.title.clone())
            .unwrap_or_else(|| premise.chars().take(12).collect::<String>());
        let genre = concept.as_ref().and_then(|c| c.genre.clone());

        // 2) 建故事
        let pool = self.pool.clone();
        let title_c = title.clone();
        let genre_c = genre.clone();
        let premise_c = premise.to_string();
        let story = tokio::task::spawn_blocking(move || {
            StoryRepository::new(pool).create(CreateStoryRequest {
                title: title_c,
                description: Some(premise_c),
                genre: genre_c,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
        }).await.map_err(|e| AppError::from(format!("create story join error: {}", e)))?
            .map_err(AppError::from)?;
        let story_id = story.id.clone();
        repo.set_run_story(run_id, &story_id).map_err(AppError::from)?;
        self.check_cancel(cancel)?;

        // 3) 管理：资产生产
        repo.update_run_phase(run_id, "running", "assets").map_err(AppError::from)?;
        self.emit_progress(run_id, "assets", "running", "管理 Agent 正在生产创作资产");
        let board = self.board();
        let registry = Arc::new(ToolRegistry::agency_default());
        let producer_out = self.run_role(
            AgentRole::Producer, &board, &registry, run_id, &story_id, premise,
            "请为本故事生产创世资产：世界观、至少 2 张角色卡（真名/欲望/阻力）、第一卷大纲、伏笔清单。逐条写入资产区。",
        ).await.map_err(|e| AppError::from(format!("管理 Agent 阶段失败: {}", e)))?;
        if producer_out.aborted {
            return Err(AppError::from("管理 Agent 被熔断，资产生产未完成"));
        }
        self.check_cancel(cancel)?;

        // 4) 主创：首章写作
        repo.update_run_phase(run_id, "running", "writing").map_err(AppError::from)?;
        self.emit_progress(run_id, "writing", "running", "主创 Agent 正在写作第一章");
        let writer_out = self.run_role(
            AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
            "基于资产区创作第一章正文（1500-2500 字）。先用 board_read 读资产，再用 board_write 把完整正文写入 draft 区（item_type=chapter, key=第一章）。",
        ).await.map_err(|e| AppError::from(format!("主创 Agent 阶段失败: {}", e)))?;
        if writer_out.aborted {
            return Err(AppError::from("主创 Agent 被熔断，首章未完成"));
        }
        let mut draft = self.latest_draft(&board, run_id)?;
        self.check_cancel(cancel)?;

        // 5) 编辑审计 + 至多 1 轮修订
        let mut revised = false;
        let verdict = loop {
            repo.update_run_phase(run_id, "running", "review").map_err(AppError::from)?;
            self.emit_progress(run_id, "review", "running", "编辑审计 Agent 正在审查草稿");
            let editor_out = self.run_role(
                AgentRole::EditorAuditor, &board, &registry, run_id, &story_id, premise,
                &format!("审查 draft 区的最新章节草稿（当前版本：{}）。按系统提示词出具裁决 JSON。", draft.key),
            ).await.map_err(|e| AppError::from(format!("编辑审计 Agent 阶段失败: {}", e)))?;
            let verdict: EditorVerdict = parse_lenient(&editor_out.output).unwrap_or(EditorVerdict {
                verdict: "pass".to_string(),
                blocking_issues: vec![],
                suggestions: vec![],
                comments: format!("（裁决解析失败，默认放行）原文：{}", editor_out.output.chars().take(200).collect::<String>()),
            });
            // 裁决落审查区（编辑审计为审查区 owner，active）
            let summary = format!("{}：{}", verdict.verdict, verdict.comments.chars().take(60).collect::<String>());
            board.write(run_id, &story_id, AgentRole::EditorAuditor, BoardZone::Review,
                "verdict", &format!("{}-v{}", draft.key, draft.version),
                &editor_out.output, &summary)?;
            if verdict.verdict == "revise" && !verdict.blocking_issues.is_empty() && !revised && revised_pass_allowed() {
                revised = true;
                repo.update_run_phase(run_id, "running", "revision").map_err(AppError::from)?;
                self.emit_progress(run_id, "revision", "running", "主创 Agent 正在按审查意见修订");
                let issues = verdict.blocking_issues.join("；");
                let revise_out = self.run_role(
                    AgentRole::LeadWriter, &board, &registry, run_id, &story_id, premise,
                    &format!("修订「{}」。审查阻断问题：{}。先 board_read 读草稿与资产，再把修订后的完整正文用 board_write 写入 draft 区（同 key）。", draft.key, issues),
                ).await.map_err(|e| AppError::from(format!("修订阶段失败: {}", e)))?;
                if revise_out.aborted {
                    return Err(AppError::from("主创 Agent 修订轮被熔断"));
                }
                draft = self.latest_draft(&board, run_id)?;
                self.check_cancel(cancel)?;
                continue; // 修订后再审一次（P1 第二轮无论结果都放行）
            }
            break verdict;
        };

        // 6) 装配：草稿 → Scene 真源（统一输出装配器 P1 形态）
        repo.update_run_phase(run_id, "running", "assembly").map_err(AppError::from)?;
        self.emit_progress(run_id, "assembly", "running", "正在装配正式稿");
        let pool = self.pool.clone();
        let sid = story_id.clone();
        let content = draft.content.clone();
        let scene = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
            let repo = SceneRepository::new(pool);
            let scene = repo.create(&sid, 1, Some("第一章")).map_err(AppError::from)?;
            repo.update(&scene.id, &crate::db::repositories::SceneUpdate {
                content: Some(content),
                ..Default::default()
            }).map_err(AppError::from)?;
            Ok(scene)
        }).await.map_err(|e| AppError::from(format!("scene assembly join error: {}", e)))??;

        Ok(AgencyGenesisResult {
            run_id: run_id.to_string(),
            story_id,
            scene_id: scene.id,
            revised,
            verdict,
            chapter_chars: draft.content.chars().count(),
        })
    }

    async fn run_role(
        &self,
        role: AgentRole,
        board: &BlackboardService,
        registry: &Arc<ToolRegistry>,
        run_id: &str,
        story_id: &str,
        premise: &str,
        task: &str,
    ) -> Result<crate::agency::tool_loop::LoopResult, AppError> {
        let spec = spec_for(role);
        let system_prompt = self.resolve_role_prompt(spec.prompt_id, premise);
        let ctx = ToolContext {
            run_id: run_id.to_string(),
            story_id: story_id.to_string(),
            role,
            board: board.clone(),
            pool: self.pool.clone(),
        };
        ToolLoop::new(self.llm.clone(), registry.clone())
            .with_max_turns(spec.max_turns)
            .run(role, &ctx, &system_prompt, task)
            .await
    }

    fn latest_draft(&self, board: &BlackboardService, run_id: &str) -> Result<BoardItem, AppError> {
        let drafts = board.list_zone(run_id, BoardZone::Draft)?;
        drafts.into_iter().last()
            .filter(|d| !d.content.is_empty())
            .ok_or_else(|| AppError::from("草稿区为空：主创未产出正文"))
    }

    fn board(&self) -> BlackboardService {
        match &self.app_handle {
            Some(app) => BlackboardService::with_events(self.pool.clone(), app),
            None => BlackboardService::new(self.pool.clone()),
        }
    }

    /// 角色系统提示词：优先 PromptRegistry（支持用户覆盖），注册表不可用时回退内置短提示。
    fn resolve_role_prompt(&self, prompt_id: &str, premise: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("premise".to_string(), premise.to_string());
        let pool = self.pool.clone();
        let pid = prompt_id.to_string();
        let resolved = crate::prompts::registry::resolve_prompt_with_vars(&pool, &pid, &vars);
        resolved.unwrap_or_else(|_| format!("{}\n\n当前故事前提：{}", default_role_prompt(&pid), premise))
    }

    fn check_cancel(&self, cancel: &Arc<AtomicBool>) -> Result<(), AppError> {
        if cancel.load(Ordering::SeqCst) {
            Err(AppError::from("创世已取消"))
        } else {
            Ok(())
        }
    }

    fn emit_progress(&self, run_id: &str, phase: &str, status: &str, message: &str) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(EVENT_RUN_PROGRESS, serde_json::json!({
                "run_id": run_id,
                "phase": phase,
                "status": status,
                "message": message,
            }));
        }
    }
}

fn revised_pass_allowed() -> bool {
    true // P1 常量：MAX_REVISION_PASSES=1，循环条件里 revised 已为 true 时不再进入
}

fn default_role_prompt(prompt_id: &str) -> &'static str {
    match prompt_id {
        "agency_lead_writer_system" => "你是「主创」：基于黑板资产创作小说正文，草稿写入 draft 区。",
        "agency_producer_system" => "你是「管理」：生产世界观/角色/大纲/伏笔资产，写入 asset 区。",
        "agency_editor_auditor_system" => "你是「编辑审计」：审查草稿，输出裁决 JSON（verdict/blocking_issues/suggestions/comments）。",
        _ => "你是创作团队的一员。",
    }
}

// 保留常量可见性，避免 dead_code 警告
#[allow(dead_code)]
const _: usize = MAX_REVISION_PASSES;
```

`src-tauri/src/agency/mod.rs` 补回 `pub mod coordinator;`。

注意：
- `BlackboardService::with_events` 的签名按 Task 2 修正后版本（`pub fn with_events(pool: DbPool, app_handle: &AppHandle) -> Self`，去掉泛型 R）。
- `crate::db::repositories::SceneUpdate` 的导出路径以 `repositories/mod.rs` 为准；`crate::errors::AppError` 路径以现有 errors 模块为准。
- `resolve_prompt_with_vars(pool, ...)` 是同步 DB 读取，此处为极短查询不包 spawn_blocking，与现有调用方一致（`prompts/registry.rs` 的调用方均直接调用）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test --lib agency:: 2>&1 | tail -5`
Expected: PASS（累计 25 个测试）。再跑 `cargo test --lib 2>&1 | tail -3` 确认整体基线全绿。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/coordinator.rs src-tauri/src/agency/mod.rs src-tauri/src/agency/board.rs
git commit -m "feat(agency): serial genesis coordinator end-to-end with editor verdict gate"
```

---

### Task 7: IPC 命令注册 + 全量验证 + 文档同步

**Files:**
- Create: `src-tauri/src/agency/commands.rs`
- Modify: `src-tauri/src/agency/mod.rs`（补回 `pub mod commands;`）
- Modify: `src-tauri/src/handlers.rs`（新增 agency 分组 4 行）
- Modify: `ARCHITECTURE.md`（新增 agency 模块小节）、`AGENTS.md`（模块清单）、`PROJECT_STATUS.md`（当前迭代）

**Interfaces:**
- Consumes: Task 6 全部公开 API；`crate::llm::LlmService::cancel_all_generations`。
- Produces: Tauri 命令 `agency_start_genesis / agency_get_run / agency_list_board / agency_cancel_run`。

- [ ] **Step 1: 实现命令**

`src-tauri/src/agency/commands.rs`：

```rust
use tauri::{AppHandle, State};

use crate::agency::board::BlackboardService;
use crate::agency::coordinator::{cancel_agency_run, AgencyCoordinator};
use crate::agency::models::{AgencyRun, BoardItem};
use crate::agency::repository::AgencyRepository;
use crate::db::DbPool;
use crate::errors::AppError;

/// 启动创世 2.0：立即返回 run_id，进度经 `agency-run-progress` 事件推送。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_start_genesis(
    premise: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator.run_genesis(&rid, &premise).await {
            log::error!("agency genesis run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_run(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<AgencyRun>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        AgencyRepository::new(pool).get_run(&run_id).map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_get_run join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_list_board(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<BoardItem>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        BlackboardService::new(pool).repo().list_items(&run_id, None).map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_list_board join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_cancel_run(
    run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    cancel_agency_run(&run_id);
    crate::llm::LlmService::new(app_handle).cancel_all_generations();
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        let repo = AgencyRepository::new(pool);
        if let Ok(Some(run)) = repo.get_run(&run_id) {
            if run.status == "running" || run.status == "pending" {
                let _ = repo.finish_run(&run_id, "cancelled", None, Some("用户取消"));
            }
        }
    })
    .await
    .map_err(|e| AppError::from(format!("agency_cancel_run join error: {}", e)))?;
    Ok(())
}
```

`src-tauri/src/agency/mod.rs` 补回 `pub mod commands;`。

`src-tauri/src/handlers.rs` 在 `// Audit commands` 分组后新增：

```rust
    // Agency (Genesis 2.0) commands
    agency::commands::agency_start_genesis, agency::commands::agency_get_run,
    agency::commands::agency_list_board, agency::commands::agency_cancel_run,
```

- [ ] **Step 2: 编译与全量测试**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`
Expected: PASS（770 + 25 agency 新增，全绿）

Run: `cd src-frontend && npx vitest run 2>&1 | tail -3`
Expected: 292 passed（前端零改动，保持绿）

Run: `python scripts/architecture_guard.py 2>&1 | tail -5`
Expected: 通过；若守护脚本有模块白名单/依赖规则，按其报错把 `agency` 加入允许清单（agency 允许依赖 db/llm/router/prompts/errors，不允许反向依赖）。

- [ ] **Step 3: 手动端到端验收（真实模型）**

1. 确保已配置可用模型（设置页激活 profile）。
2. `cargo tauri dev` 启动应用。
3. 触发创世：前端控制台执行
   ```js
   const { invoke } = window.__TAURI__.core;
   const runId = await invoke('agency_start_genesis', { premise: '一个在双星废土上拾荒的少女，意外挖出一艘能说话的星舰' });
   ```
   （若 `window.__TAURI__` 不可用，改用 `src-frontend/src/` 里现有的 invoke 封装在任意页面临时调用，或加一个临时按钮，验收后移除。）
4. 轮询 `await invoke('agency_get_run', { runId })` 直至 `status === 'completed'`。
5. 验收标准（P1 出口）：
   - `agency_get_run` 返回 `status=completed`、`story_id` 非空；
   - `agency_list_board` 返回资产区 ≥3 条（世界观/角色/大纲）、草稿区 ≥1 条、审查区 ≥1 条；
   - 应用中打开新故事，第一章场景正文存在且与黑板草稿一致；
   - 全程在 UI 可见新故事出现（`story-created` 现有机制不受影响）。

- [ ] **Step 4: 文档同步**

- `ARCHITECTURE.md`：在模块章节新增 `agency/` 小节（职责：多代理创作框架创世 2.0；依赖：db/llm/router/prompts；被依赖：无）。
- `AGENTS.md`：模块清单加 `src-tauri/src/agency/`（黑板/工具循环/三角色/协调器）；提示词目录加 `resources/prompts/agency/`。
- `PROJECT_STATUS.md`：当前迭代记录"P1 多代理框架骨架完成（串行端到端）"。
- `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`：实现状态行更新为"P1 已完成"。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/agency/commands.rs src-tauri/src/agency/mod.rs src-tauri/src/handlers.rs ARCHITECTURE.md AGENTS.md PROJECT_STATUS.md docs/plans/
git commit -m "feat(agency): IPC commands, docs sync; P1 serial genesis framework complete"
```

---

## Self-Review（计划自审结论）

- **Spec coverage**：设计 P1 范围（runtime/blackboard/三角色/串行协调/单测）→ Task 1–7 全覆盖。消息总线（Task 2）在 P1 串行协调器中暂无消费方（P2 接线），已在 Global Constraints 声明。P2–P5（并行化/代币优化+记忆/验证循环/持续学习/前端可视化）按计划边界另行制定。
- **Placeholder scan**：各任务代码完整；三处明确标注"以现有代码为准"的路径确认点（`AppError::message` 访问器、`StoryRepository::list` 名字、`SceneUpdate` 导出路径）——这些是接口命名核实点，不是占位符；执行者可在 10 秒内 grep 确认。
- **Type consistency**：`ToolContext` 五字段（run_id/story_id/role/board/pool）在 Task 3/4/6 一致；`LoopLlm::complete` 四参签名在 Task 4 定义、Task 6 生产实现与 mock 一致；`spec_for` 字段（prompt_id/task_type/max_turns/max_output_tokens）在 Task 4 便捷方法与 Task 5 定义一致；`parse_lenient` 在 Task 6 定义并自测；`BlackboardService::with_events` 在 Task 6 备注中修正为非泛型签名。
