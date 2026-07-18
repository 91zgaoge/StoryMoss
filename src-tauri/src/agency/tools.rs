use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use rusqlite::OptionalExtension;

use crate::{
    agency::{board::BlackboardService, models::*},
    db::DbPool,
    error::AppError,
};

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
    async fn execute(&self, ctx: &ToolContext, args: serde_json::Value)
        -> Result<String, AppError>;
}

impl ToolContext {
    pub fn task_type(&self) -> crate::router::TaskType {
        crate::agency::roles::spec_for(self.role).task_type
    }

    pub fn max_output_tokens(&self) -> i32 {
        crate::agency::roles::spec_for(self.role).max_output_tokens
    }

    /// 当前角色的上下文注入预算（字符），ToolLoop 会话窗口截断用。
    pub fn max_context_chars(&self) -> usize {
        crate::agency::roles::spec_for(self.role).context_budget_chars
    }
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
        self.whitelists
            .entry(role)
            .or_default()
            .insert(tool_name.to_string());
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
        registry.register(Arc::new(BoardReviseTool));
        registry.register(Arc::new(StoryInfoTool));
        registry.register(Arc::new(AssetQueryTool));
        for role in AgentRole::all() {
            registry.allow(role, "board_read");
            registry.allow(role, "story_info");
            registry.allow(role, "asset_query");
        }
        // 编辑审计只读（审查结论经 ToolLoop final 由协调器落审查区）
        registry.allow(AgentRole::LeadWriter, "board_write");
        registry.allow(AgentRole::Producer, "board_write");
        registry.allow(AgentRole::LeadWriter, "board_revise");
        registry
    }
}

// ---- 内置工具 ----

pub struct BoardReadTool;

#[async_trait::async_trait]
impl AgentTool for BoardReadTool {
    fn name(&self) -> &'static str {
        "board_read"
    }
    fn description(&self) -> &'static str {
        "读取黑板分区目录（key+摘要+版本）；需要全文时给出 key"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"zone": "asset|draft|review|schedule（可选，缺省读全部）", "key": "可选，精确读取某条目", "detail": "catalog|summary|full（默认 catalog；key 精确读默认 full）"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let pool = ctx.pool.clone();
        let run_id = ctx.run_id.clone();
        let zone = args.get("zone").and_then(|v| v.as_str()).map(String::from);
        let key = args.get("key").and_then(|v| v.as_str()).map(String::from);
        let detail = args
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        tokio::task::spawn_blocking(move || -> Result<String, AppError> {
            let board = BlackboardService::new(pool);
            // zone 非空但非法时回显错误让模型自愈（不再静默读全部）
            let zone = match zone.as_deref() {
                Some(z) => match BoardZone::from_str(z) {
                    Some(parsed) => Some(parsed),
                    None => {
                        return Ok(format!(
                            "非法 zone: {}，可选 asset|draft|review|schedule",
                            z
                        ))
                    }
                },
                None => None,
            };
            if let Some(k) = key {
                let items = board.list_zone_filtered(&run_id, zone)?;
                if let Some(item) = items.into_iter().find(|i| i.key == k) {
                    // 三档 detail：summary 只取前 500 字符；full（含默认）取全文
                    let body = match detail.as_str() {
                        "summary" => format!(
                            "{}…(summary 档，detail=full 取全文)",
                            item.content.chars().take(500).collect::<String>()
                        ),
                        _ => item.content.clone(),
                    };
                    return Ok(format!(
                        "[{}/{}] v{}\n{}",
                        item.zone.as_str(),
                        item.key,
                        item.version,
                        body
                    ));
                }
                return Ok(format!("未找到 key={} 的条目", k));
            }
            match zone {
                Some(z) => {
                    let items = board.list_zone(&run_id, z)?;
                    let mut out = String::new();
                    for item in items {
                        out.push_str(&format!(
                            "- [{}/{}] {} (v{}, {})\n",
                            item.zone.as_str(),
                            item.key,
                            item.summary,
                            item.version,
                            item.status
                        ));
                    }
                    if out.is_empty() {
                        out = "（空）\n".into();
                    }
                    Ok(out)
                }
                None => Ok(board.snapshot(&run_id)?.to_catalog_tokens(500, "cl100k")),
            }
        })
        .await
        .map_err(|e| AppError::from(format!("board_read join error: {}", e)))?
    }
}

pub struct BoardWriteTool;

#[async_trait::async_trait]
impl AgentTool for BoardWriteTool {
    fn name(&self) -> &'static str {
        "board_write"
    }
    fn description(&self) -> &'static str {
        "写入黑板条目（非本角色分区自动降级为提案）"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"zone": "asset|draft|review|schedule", "item_type": "条目类型", "key": "条目标识", "content": "全文", "summary": "一句话摘要（≤80字）"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let zone_str = args.get("zone").and_then(|v| v.as_str()).unwrap_or("");
        let zone = BoardZone::from_str(zone_str).ok_or_else(|| {
            AppError::validation_failed(format!("非法 zone: {}", zone_str), None::<String>)
        })?;
        let item_type = args
            .get("item_type")
            .and_then(|v| v.as_str())
            .unwrap_or("note")
            .to_string();
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation_failed("board_write 缺少 key", None::<String>))?
            .to_string();
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let board = ctx.board.clone();
        let run_id = ctx.run_id.clone();
        let story_id = ctx.story_id.clone();
        let role = ctx.role;
        tokio::task::spawn_blocking(move || {
            board.write(
                &run_id, &story_id, role, zone, &item_type, &key, &content, &summary,
            )
        })
        .await
        .map_err(|e| AppError::from(format!("board_write join error: {}", e)))?
        .map(|item| {
            format!(
                "已写入 [{}/{}] status={} id={}",
                item.zone.as_str(),
                item.key,
                item.status,
                item.id
            )
        })
    }
}

pub struct BoardReviseTool;

#[async_trait::async_trait]
impl AgentTool for BoardReviseTool {
    fn name(&self) -> &'static str {
        "board_revise"
    }
    fn description(&self) -> &'static str {
        "修订自己分区的既有条目（版本乐观锁；用于按审查意见修订草稿）"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"item_id": "条目 id", "expected_version": "当前版本号（整数）", "content": "修订后全文", "summary": "一句话摘要"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let item_id = args
            .get("item_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::validation_failed("board_revise 缺少 item_id", None::<String>)
            })?
            .to_string();
        let expected_version = args
            .get("expected_version")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                AppError::validation_failed("board_revise 缺少 expected_version", None::<String>)
            })? as i32;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let board = ctx.board.clone();
        let role = ctx.role;
        tokio::task::spawn_blocking(move || {
            board.revise(&item_id, role, &content, &summary, expected_version)
        })
        .await
        .map_err(|e| AppError::from(format!("board_revise join error: {}", e)))?
        .map(|item| {
            format!(
                "已修订 [{}/{}] 到 v{}",
                item.zone.as_str(),
                item.key,
                item.version
            )
        })
    }
}

pub struct StoryInfoTool;

#[async_trait::async_trait]
impl AgentTool for StoryInfoTool {
    fn name(&self) -> &'static str {
        "story_info"
    }
    fn description(&self) -> &'static str {
        "读取当前故事的基本信息（标题/类型/简介）"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        _args: serde_json::Value,
    ) -> Result<String, AppError> {
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

pub struct AssetQueryTool;

#[async_trait::async_trait]
impl AgentTool for AssetQueryTool {
    fn name(&self) -> &'static str {
        "asset_query"
    }
    fn description(&self) -> &'static str {
        "查询故事资产库：characters 角色卡 / world 世界观 / outline 大纲 / scenes 最近场景摘要"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"kind": "characters|world|outline|scenes"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let kind = args
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pool = ctx.pool.clone();
        let story_id = ctx.story_id.clone();
        tokio::task::spawn_blocking(move || -> Result<String, AppError> {
            let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let out = match kind.as_str() {
                "characters" => {
                    let mut stmt = conn.prepare(
                        "SELECT name, COALESCE(personality,''), COALESCE(goals,''), COALESCE(background,'')
                         FROM characters WHERE story_id = ?1 ORDER BY created_at LIMIT 20")?;
                    let rows = stmt.query_map(rusqlite::params![story_id], |r| {
                        Ok(format!("- {}｜性格:{}｜目标:{}｜背景:{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
                    })?;
                    let list: Vec<String> = rows.collect::<Result<_, _>>()?;
                    if list.is_empty() { "（资产库无角色）".to_string() } else { list.join("\n") }
                }
                "world" => {
                    conn.query_row(
                        "SELECT concept, COALESCE(history,'') FROM world_buildings WHERE story_id = ?1",
                        rusqlite::params![story_id],
                        |r| Ok(format!("概念: {}\n历史: {}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    ).optional()?.unwrap_or_else(|| "（资产库无世界观）".to_string())
                }
                "outline" => {
                    conn.query_row(
                        "SELECT content FROM story_outlines WHERE story_id = ?1",
                        rusqlite::params![story_id],
                        |r| r.get::<_, String>(0),
                    ).optional()?.unwrap_or_else(|| "（资产库无大纲）".to_string())
                }
                "scenes" => {
                    let mut stmt = conn.prepare(
                        "SELECT sequence_number, COALESCE(title,''), substr(COALESCE(content,''),1,200)
                         FROM scenes WHERE story_id = ?1 ORDER BY sequence_number DESC LIMIT 5")?;
                    let mut rows: Vec<String> = stmt.query_map(rusqlite::params![story_id], |r| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::{board::BlackboardService, repository::AgencyRepository},
        db::{create_test_pool, dto::CreateStoryRequest, repositories::StoryRepository},
    };

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
            .create_run(&AgencyRun::new("r1", "前提"))
            .unwrap();
    }

    #[tokio::test]
    async fn test_board_write_then_read() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool, AgentRole::Producer);
        let write = registry
            .get_for_role(AgentRole::Producer, "board_write")
            .unwrap();
        let out = write
            .execute(
                &context,
                serde_json::json!({
                    "zone": "asset", "item_type": "world", "key": "世界观",
                    "content": "双星废土，磁力风暴", "summary": "双星废土"
                }),
            )
            .await
            .unwrap();
        assert!(out.contains("active"));
        let read = registry
            .get_for_role(AgentRole::Producer, "board_read")
            .unwrap();
        let catalog = read
            .execute(&context, serde_json::json!({"zone": "asset"}))
            .await
            .unwrap();
        assert!(catalog.contains("世界观") || catalog.contains("双星废土"));
    }

    #[tokio::test]
    async fn test_whitelist_enforcement() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        // 编辑审计角色不允许 board_write（其审查经 ToolLoop final + 协调器落审查区，
        // P1 白名单收紧到只读 + story_info）
        assert!(registry
            .get_for_role(AgentRole::EditorAuditor, "board_write")
            .is_none());
        assert!(registry
            .get_for_role(AgentRole::EditorAuditor, "board_read")
            .is_some());
        // 未注册工具名 → None
        assert!(registry
            .get_for_role(AgentRole::Producer, "delete_story")
            .is_none());
    }

    #[tokio::test]
    async fn test_story_info() {
        let pool = create_test_pool().unwrap();
        StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "星海拾荒者".into(),
                description: Some("废土与星环".into()),
                genre: Some("科幻".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let registry = ToolRegistry::agency_default();
        let story = StoryRepository::new(pool.clone());
        // 找到刚创建的 story id
        let created = story.get_all().unwrap();
        let sid = created[0].id.clone();
        let mut context = ctx(pool, AgentRole::LeadWriter);
        context.story_id = sid;
        let tool = registry
            .get_for_role(AgentRole::LeadWriter, "story_info")
            .unwrap();
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

    #[tokio::test]
    async fn test_board_read_summary_detail() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool, AgentRole::Producer);
        let long = "长".repeat(1000);
        context
            .board
            .write(
                "r1",
                "s1",
                AgentRole::Producer,
                BoardZone::Asset,
                "world",
                "世界观",
                &long,
                "长文本",
            )
            .unwrap();
        let read = registry
            .get_for_role(AgentRole::Producer, "board_read")
            .unwrap();
        let summary = read
            .execute(
                &context,
                serde_json::json!({"zone": "asset", "key": "世界观", "detail": "summary"}),
            )
            .await
            .unwrap();
        assert!(
            summary.chars().count() < 700,
            "summary 档应截断: {}",
            summary.len()
        );
        let full = read
            .execute(
                &context,
                serde_json::json!({"zone": "asset", "key": "世界观", "detail": "full"}),
            )
            .await
            .unwrap();
        assert!(full.chars().count() >= 1000);
    }

    #[tokio::test]
    async fn test_board_revise_tool() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool.clone(), AgentRole::LeadWriter);
        // 先由 owner 写入 draft
        let draft = context
            .board
            .write(
                "r1",
                "s1",
                AgentRole::LeadWriter,
                BoardZone::Draft,
                "chapter",
                "第一章",
                "初稿",
                "初稿",
            )
            .unwrap();
        let revise = registry
            .get_for_role(AgentRole::LeadWriter, "board_revise")
            .expect("LeadWriter 应有 board_revise");
        let out = revise
            .execute(
                &context,
                serde_json::json!({
                    "item_id": draft.id, "expected_version": 1,
                    "content": "修订稿", "summary": "修订稿"
                }),
            )
            .await
            .unwrap();
        assert!(out.contains("v2") || out.contains("version=2"));
        let item = context.board.repo().get_item(&draft.id).unwrap().unwrap();
        assert_eq!(item.content, "修订稿");
        assert_eq!(item.version, 2);
        // 版本冲突 → 错误回显（工具 Ok 但内容提示冲突，或 Err——以实现为准断言其一）
        let conflict = revise
            .execute(
                &context,
                serde_json::json!({
                    "item_id": draft.id, "expected_version": 1,
                    "content": "并发", "summary": "x"
                }),
            )
            .await;
        assert!(conflict.is_err() || conflict.unwrap().contains("冲突"));
    }

    #[tokio::test]
    async fn test_board_revise_whitelist() {
        let registry = ToolRegistry::agency_default();
        assert!(registry
            .get_for_role(AgentRole::Producer, "board_revise")
            .is_none());
        assert!(registry
            .get_for_role(AgentRole::EditorAuditor, "board_revise")
            .is_none());
    }

    #[tokio::test]
    async fn test_asset_query_tool() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "资产书".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                 VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
                rusqlite::params![story.id],
            ).unwrap();
        }
        let registry = ToolRegistry::agency_default();
        // 三角色白名单均可读
        for role in AgentRole::all() {
            assert!(
                registry.get_for_role(role, "asset_query").is_some(),
                "{:?} 应可读 asset_query",
                role
            );
        }
        let mut context = ctx(pool, AgentRole::LeadWriter);
        context.story_id = story.id.clone();
        let tool = registry
            .get_for_role(AgentRole::LeadWriter, "asset_query")
            .unwrap();
        let out = tool
            .execute(&context, serde_json::json!({"kind": "characters"}))
            .await
            .unwrap();
        assert!(out.contains("阿苔"));
        let empty = tool
            .execute(&context, serde_json::json!({"kind": "outline"}))
            .await
            .unwrap();
        assert!(empty.contains("无大纲"));
        let bad = tool
            .execute(&context, serde_json::json!({"kind": "nope"}))
            .await
            .unwrap();
        assert!(bad.contains("非法 kind"));
    }
}
