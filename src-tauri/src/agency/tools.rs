use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use rusqlite::OptionalExtension;

use crate::{
    agency::{board::BlackboardService, models::*},
    creative_engine::adapter::CreativeEngineAdapter,
    db::DbPool,
    domain::{
        asset_snapshot::{ActiveConflict, AssetSnapshot, CharacterStateSnapshot},
        creative_engine::CreativeEnginePort,
    },
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
    fn usage_guidance(&self) -> Option<&'static str> {
        None
    }
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

fn format_character_states(states: &[CharacterStateSnapshot]) -> String {
    if states.is_empty() {
        return "无".to_string();
    }
    states
        .iter()
        .map(|s| {
            format!(
                "- {} | 位置: {} | 情绪: {} | 目标: {}",
                s.name,
                s.current_location.as_deref().unwrap_or("无"),
                s.current_emotion.as_deref().unwrap_or("无"),
                s.active_goal.as_deref().unwrap_or("无")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_active_conflicts(conflicts: &[ActiveConflict]) -> String {
    if conflicts.is_empty() {
        return "无".to_string();
    }
    conflicts
        .iter()
        .map(|c| {
            format!(
                "- 类型: {} | 参与方: {} | 赌注: {}",
                c.conflict_type,
                c.parties.join(", "),
                c.stakes
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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

    /// 当前角色白名单的原生 ToolSpec（JSON Schema parameters）。
    pub fn tool_specs_for_role(&self, role: AgentRole) -> Vec<crate::llm::adapter::ToolSpec> {
        use crate::llm::adapter::{informal_args_to_json_schema, ToolSpec};
        let mut out = Vec::new();
        if let Some(allowed) = self.whitelists.get(&role) {
            let mut names: Vec<&String> = allowed.iter().collect();
            names.sort();
            for name in names {
                if let Some(tool) = self.tools.get(name) {
                    let mut description = tool.description().to_string();
                    if let Some(usage) = tool.usage_guidance() {
                        description.push(' ');
                        description.push_str(usage);
                    }
                    out.push(ToolSpec {
                        name: tool.name().to_string(),
                        description,
                        parameters: informal_args_to_json_schema(&tool.args_schema()),
                    });
                }
            }
        }
        out
    }

    /// 注入系统提示词的工具目录：名 + 一行。JSON Schema 走原生 `tools[]`。
    pub fn catalog_for_role(&self, role: AgentRole) -> String {
        let mut out = String::from("可用工具：\n");
        if let Some(allowed) = self.whitelists.get(&role) {
            let mut names: Vec<&String> = allowed.iter().collect();
            names.sort();
            for name in names {
                if let Some(tool) = self.tools.get(name) {
                    let mut line = format!("- {}: {}", tool.name(), tool.description());
                    if let Some(usage) = tool.usage_guidance() {
                        line.push(' ');
                        line.push_str(usage);
                    }
                    out.push_str(&line);
                    out.push('\n');
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
        registry.register(Arc::new(AssetReadTool));
        registry.register(Arc::new(CreativeContextTool));
        for role in AgentRole::all() {
            registry.allow(role, "board_read");
            registry.allow(role, "story_info");
            registry.allow(role, "asset_query");
            registry.allow(role, "asset_read");
            registry.allow(role, "creative_context");
        }
        // 编辑审计只读（审查结论经 ToolLoop final 由协调器落审查区）
        registry.allow(AgentRole::LeadWriter, "board_write");
        registry.allow(AgentRole::Producer, "board_write");
        registry.allow(AgentRole::LeadWriter, "board_revise");
        // 高频 agents 映射为 agency role 后的工具白名单
        registry.allow(AgentRole::Writer, "board_write");
        registry.allow(AgentRole::Writer, "board_revise");
        registry.allow(AgentRole::OutlinePlanner, "board_write");
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
        "读取黑板分区目录（key+摘要+版本）；detail=catalog|summary|full，key 精确读时 summary 档取前 500 字"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"zone": "asset|draft|review|schedule（可选，缺省读全部）", "key": "可选，精确读取某条目", "detail": "catalog|summary|full（默认 catalog；key 精确读默认 full）"})
    }
    fn usage_guidance(&self) -> Option<&'static str> {
        Some("资产已注入时不要轮询 board_read 拉全文；只需补读遗漏 key")
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
                    // 三档 detail：summary 只取前 500
                    // 字符；full（含默认）取全文
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
                    if items.is_empty() {
                        return Ok("（空）\n".into());
                    }
                    // 单分区目录同样受 token 预算约束：组装单分区快照走
                    // to_catalog_tokens 逐行截断（与全量目录同一预算口径）
                    let mut snap = crate::agency::board::BoardSnapshot {
                        assets: vec![],
                        drafts: vec![],
                        reviews: vec![],
                        schedules: vec![],
                    };
                    match z {
                        BoardZone::Asset => snap.assets = items,
                        BoardZone::Draft => snap.drafts = items,
                        BoardZone::Review => snap.reviews = items,
                        BoardZone::Schedule => snap.schedules = items,
                    }
                    Ok(snap.to_catalog_tokens(500, "cl100k"))
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
    fn usage_guidance(&self) -> Option<&'static str> {
        Some("正文写入 draft 区，勿覆盖 user_created 资产。写角色后必须再写 item_type=relationship（缺则建、有则改）")
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let zone_str = args.get("zone").and_then(|v| v.as_str()).unwrap_or("");
        // 非法 zone 收编：本地模型常把 item_type 误填进 zone（如
        // "character"）。若该字符串是已知 item_type（大小写不敏感），收编为
        // asset 区并在返回文本注明；否则维持 validation_failed 让模型自愈。
        const KNOWN_ITEM_TYPES: [&str; 5] = [
            "character",
            "world",
            "outline",
            "foreshadowing",
            "worldbuilding",
        ];
        let (zone, coerced_note) = match BoardZone::from_str(zone_str) {
            Some(z) => (z, None),
            None if KNOWN_ITEM_TYPES
                .iter()
                .any(|t| t.eq_ignore_ascii_case(zone_str)) =>
            {
                (
                    BoardZone::Asset,
                    Some(format!("（zone '{}' 已收编为 asset）", zone_str)),
                )
            }
            None => {
                return Err(AppError::validation_failed(
                    format!("非法 zone: {}", zone_str),
                    None::<String>,
                ))
            }
        };
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
                "已写入 [{}/{}] status={} id={}{}",
                item.zone.as_str(),
                item.key,
                item.status,
                item.id,
                coerced_note.as_deref().unwrap_or("")
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
        "读取当前故事的基本信息（标题/类型/简介）与已有正文开篇摘录"
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
        let pool_prose = ctx.pool.clone();
        let sid_prose = ctx.story_id.clone();
        let info = tokio::task::spawn_blocking(move || -> Result<Option<(String, String, String)>, AppError> {
            let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let info = conn.query_row(
                "SELECT title, COALESCE(genre, ''), COALESCE(description, '') FROM stories WHERE id = ?1",
                rusqlite::params![story_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            ).optional().map_err(AppError::from)?;
            Ok(info)
        }).await
            .map_err(|e| AppError::from(format!("story_info join error: {}", e)))??;
        let prose = tokio::task::spawn_blocking(move || {
            crate::agency::materialize::concat_story_prose(&pool_prose, &sid_prose)
        })
        .await
        .unwrap_or_default();

        match info {
            Some((title, genre, desc)) => {
                let context = CreativeContextTool.load_context(ctx, 1).await?;

                let mut out = format!("标题: {}\n类型: {}\n简介: {}", title, genre, desc);
                if crate::agency::prose_ground::has_substantial_prose(&prose) {
                    let plain = crate::agency::continue_assets::strip_editor_markup(&prose);
                    let excerpt: String = plain
                        .chars()
                        .take(crate::agency::prose_ground::STORY_INFO_PROSE_CHARS)
                        .collect();
                    out.push_str("\n\n已有正文开篇：\n");
                    out.push_str(&excerpt);
                    out.push_str("\n禁止发明下列正文未出现的姓名。");
                }
                out.push_str("\n\n创作上下文：");
                out.push_str(&format!(
                    "\n叙事阶段: {}",
                    context
                        .asset_snapshot
                        .narrative_phase_guidance
                        .as_deref()
                        .unwrap_or("无")
                ));
                out.push_str(&format!(
                    "\n风格 DNA: {}",
                    context
                        .asset_snapshot
                        .style_dna_summary
                        .as_deref()
                        .unwrap_or("无")
                ));
                if context.foreshadowing_hints.is_empty() {
                    out.push_str("\n伏笔提示: 无");
                } else {
                    out.push_str("\n伏笔提示:\n");
                    out.push_str(
                        &context
                            .foreshadowing_hints
                            .iter()
                            .map(|h| format!("- {}", h))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
                Ok(out)
            }
            None => Ok("（故事尚未创建）".to_string()),
        }
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
    fn usage_guidance(&self) -> Option<&'static str> {
        Some("按 kind 查询，不要倾倒全表")
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
        let story_id = ctx.story_id.clone();

        match kind.as_str() {
            "characters" => {
                let context = CreativeContextTool.load_context(ctx, 1).await?;
                let formatted = format_character_states(&context.asset_snapshot.character_states);
                Ok(if formatted == "无" {
                    "（资产库无角色）".to_string()
                } else {
                    formatted
                })
            }
            "world" => {
                let context = CreativeContextTool.load_context(ctx, 1).await?;
                let has_context = context.asset_snapshot.narrative_phase_guidance.is_some()
                    || !context.asset_snapshot.active_conflicts.is_empty();
                if has_context {
                    let mut parts = Vec::new();
                    if let Some(ref guidance) = context.asset_snapshot.narrative_phase_guidance {
                        parts.push(format!("叙事阶段指引: {}", guidance));
                    }
                    if !context.asset_snapshot.active_conflicts.is_empty() {
                        parts.push(format!(
                            "活跃冲突:\n{}",
                            format_active_conflicts(&context.asset_snapshot.active_conflicts)
                        ));
                    }
                    Ok(parts.join("\n\n"))
                } else {
                    let pool = ctx.pool.clone();
                    tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                        let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
                        Ok(conn
                            .query_row(
                                "SELECT concept, COALESCE(history,'') FROM world_buildings WHERE story_id = ?1",
                                rusqlite::params![story_id],
                                |r| Ok(format!("概念: {}\n历史: {}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                            )
                            .optional()?
                            .unwrap_or_else(|| "（资产库无世界观）".to_string()))
                    })
                    .await
                    .map_err(|e| AppError::from(format!("asset_query join error: {}", e)))?
                }
            }
            "outline" => {
                let pool = ctx.pool.clone();
                tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                    let conn = pool
                        .get()
                        .map_err(|e| AppError::from(format!("pool: {}", e)))?;
                    Ok(conn
                        .query_row(
                            "SELECT content FROM story_outlines WHERE story_id = ?1",
                            rusqlite::params![story_id],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()?
                        .unwrap_or_else(|| "（资产库无大纲）".to_string()))
                })
                .await
                .map_err(|e| AppError::from(format!("asset_query join error: {}", e)))?
            }
            "scenes" => {
                let pool = ctx.pool.clone();
                tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                    let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
                    let mut stmt = conn.prepare(
                        "SELECT sequence_number, COALESCE(title,''), substr(COALESCE(content,''),1,200)
                         FROM scenes WHERE story_id = ?1 ORDER BY sequence_number DESC LIMIT 5")?;
                    let mut rows: Vec<String> = stmt.query_map(rusqlite::params![story_id], |r| {
                        Ok(format!("- 第{}场 {}: {}…", r.get::<_, i32>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                    })?.collect::<Result<_, _>>()?;
                    rows.reverse(); // 恢复时间序
                    Ok(if rows.is_empty() { "（尚无场景）".to_string() } else { rows.join("\n") })
                })
                .await
                .map_err(|e| AppError::from(format!("asset_query join error: {}", e)))?
            }
            other => Ok(format!(
                "非法 kind: {}，可选 characters|world|outline|scenes",
                other
            )),
        }
    }
}

pub struct AssetReadTool;

#[async_trait::async_trait]
impl AgentTool for AssetReadTool {
    fn name(&self) -> &'static str {
        "asset_read"
    }
    fn description(&self) -> &'static str {
        "按名读取一张生产资产全卡：character / world / outline"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"kind": "character|world|outline", "name": "角色名（kind=character 必填）"})
    }
    fn usage_guidance(&self) -> Option<&'static str> {
        Some("一次只读一张；不要倾倒全表")
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let kind = args
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("character")
            .trim()
            .to_ascii_lowercase();
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        match kind.as_str() {
            "character" | "characters" => {
                if name.is_empty() {
                    return Ok("（asset_read 需要 name）".to_string());
                }
                let pool = ctx.pool.clone();
                let story_id = ctx.story_id.clone();
                tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                    let chars = crate::db::repositories::CharacterRepository::new(pool)
                        .get_by_story(&story_id)
                        .map_err(|e| AppError::from(format!("asset_read: {e}")))?;
                    match chars.into_iter().find(|c| c.name == name) {
                        Some(c) => Ok(render_db_character_full_card(&c)),
                        None => Ok("（无此角色或不属于本故事）".to_string()),
                    }
                })
                .await
                .map_err(|e| AppError::from(format!("asset_read join error: {}", e)))?
            }
            "world" => {
                AssetQueryTool
                    .execute(ctx, serde_json::json!({"kind": "world"}))
                    .await
            }
            "outline" => {
                AssetQueryTool
                    .execute(ctx, serde_json::json!({"kind": "outline"}))
                    .await
            }
            other => Ok(format!(
                "非法 kind: {}，可选 character|world|outline",
                other
            )),
        }
    }
}

fn render_db_character_full_card(c: &crate::db::Character) -> String {
    crate::agency::continue_assets::render_one_card(
        &crate::domain::write_time_bundle::CoreCharacter {
            name: c.name.clone(),
            identity: c.background.clone(),
            physical_state: None,
            mental_state: None,
            location: None,
            personality: c.personality.clone(),
            emotional_core: c.emotional_core.clone(),
            emotional_trigger: c.emotional_trigger.clone(),
            emotional_wound: c.emotional_wound.clone(),
            emotional_need: c.emotional_need.clone(),
        },
    )
}

/// 从 `CreativeContextTool` 提取的结构化创作上下文。
/// `StoryInfoTool` / `AssetQueryTool` 通过它复用同一套引擎加载逻辑。
pub struct CreativeContext {
    pub asset_snapshot: AssetSnapshot,
    pub foreshadowing_hints: Vec<String>,
    pub bundle_prompt: String,
}

pub struct CreativeContextTool;

impl CreativeContextTool {
    /// 加载当前故事的创作上下文，供本工具及其他工具复用。
    pub async fn load_context(
        &self,
        ctx: &ToolContext,
        chapter_number: i32,
    ) -> Result<CreativeContext, AppError> {
        let pool = ctx.pool.clone();
        let story_id = ctx.story_id.clone();
        tokio::task::spawn_blocking(move || -> Result<CreativeContext, AppError> {
            let engine =
                CreativeEngineAdapter::new(pool, std::sync::Arc::new(crate::ports::NoOpLlmPort));
            let asset_snapshot = engine.load_asset_snapshot(&story_id, None);
            let foreshadowing_hints = engine.get_foreshadowing_hints(&story_id, 10)?;
            let bundle = engine.load_write_time_bundle(&story_id, chapter_number, None, None)?;
            let bundle_prompt = bundle.to_prompt();
            Ok(CreativeContext {
                asset_snapshot,
                foreshadowing_hints,
                bundle_prompt,
            })
        })
        .await
        .map_err(|e| AppError::from(format!("load_context join error: {}", e)))?
    }
}

#[async_trait::async_trait]
impl AgentTool for CreativeContextTool {
    fn name(&self) -> &'static str {
        "creative_context"
    }
    fn description(&self) -> &'static str {
        "加载当前故事的创作上下文（资产快照、伏笔提示、续写约束）"
    }
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({"chapter_number": "可选，默认 1"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: serde_json::Value,
    ) -> Result<String, AppError> {
        let chapter_number = args
            .get("chapter_number")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;
        let context = self.load_context(ctx, chapter_number).await?;

        let preview_chars: String = context.bundle_prompt.chars().take(2000).collect();
        let preview = if context.bundle_prompt.chars().count() > 2000 {
            format!("{}...(已截断)", preview_chars)
        } else {
            preview_chars
        };

        let mut sections = Vec::new();
        sections.push(format!(
            "叙事阶段指引: {}",
            context
                .asset_snapshot
                .narrative_phase_guidance
                .as_deref()
                .unwrap_or("无")
        ));
        sections.push(format!(
            "风格 DNA: {}",
            context
                .asset_snapshot
                .style_dna_summary
                .as_deref()
                .unwrap_or("无")
        ));
        sections.push(format!(
            "待回收伏笔:\n{}",
            if context.asset_snapshot.pending_foreshadowings.is_empty() {
                "无".to_string()
            } else {
                context
                    .asset_snapshot
                    .pending_foreshadowings
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        ));
        sections.push(format!(
            "逾期伏笔:\n{}",
            if context.asset_snapshot.overdue_foreshadowings.is_empty() {
                "无".to_string()
            } else {
                context
                    .asset_snapshot
                    .overdue_foreshadowings
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        ));
        sections.push(format!(
            "角色状态:\n{}",
            format_character_states(&context.asset_snapshot.character_states)
        ));
        sections.push(format!(
            "活跃冲突:\n{}",
            format_active_conflicts(&context.asset_snapshot.active_conflicts)
        ));
        sections.push(format!(
            "伏笔提示:\n{}",
            if context.foreshadowing_hints.is_empty() {
                "无".to_string()
            } else {
                context
                    .foreshadowing_hints
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        ));
        sections.push(format!("写作时间束提示:\n{}", preview));
        Ok(sections.join("\n\n"))
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
        // 编辑审计角色不允许 board_write（其审查经 ToolLoop final +
        // 协调器落审查区， P1 白名单收紧到只读 + story_info）
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

    #[tokio::test]
    async fn test_story_info_includes_prose_excerpt() {
        use crate::db::repositories::SceneRepository;

        let pool = create_test_pool().unwrap();
        let created = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "帝国的烟火".into(),
                description: Some("空简介不得当情节".into()),
                genre: Some("历史".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let scene = SceneRepository::new(pool.clone())
            .create(&created.id, 1, Some("第一章"))
            .unwrap();
        let mut prose = "知启纪元八百四十七年。第二代镇北王苏会山端坐大堂。".to_string();
        while prose.chars().count() < 200 {
            prose.push_str("红毡铺地。");
        }
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE scenes SET content = ?1 WHERE id = ?2",
                rusqlite::params![prose, scene.id],
            )
            .unwrap();
        }
        let registry = ToolRegistry::agency_default();
        let mut context = ctx(pool, AgentRole::Producer);
        context.story_id = created.id;
        let tool = registry
            .get_for_role(AgentRole::Producer, "story_info")
            .unwrap();
        let info = tool.execute(&context, serde_json::json!({})).await.unwrap();
        assert!(info.contains("苏会山"), "须附开篇摘录: {}", info);
        assert!(
            info.contains("禁止发明") && info.contains("正文未出现"),
            "须禁止按书名发明姓名: {}",
            info
        );
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
    async fn test_board_write_item_type_zone_coerced_to_asset() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool, AgentRole::Producer);
        let write = registry
            .get_for_role(AgentRole::Producer, "board_write")
            .unwrap();
        // 模型把 item_type 误填进 zone：收编为 asset 并注明
        let out = write
            .execute(
                &context,
                serde_json::json!({
                    "zone": "character", "item_type": "character", "key": "阿苔",
                    "content": "星环拾荒者", "summary": "阿苔"
                }),
            )
            .await
            .unwrap();
        assert!(
            out.contains("（zone 'character' 已收编为 asset）"),
            "结果应含收编提示: {}",
            out
        );
        let items = context.board.list_zone("r1", BoardZone::Asset).unwrap();
        assert!(
            items
                .iter()
                .any(|i| i.key == "阿苔" && i.item_type == "character"),
            "角色卡应写入 asset 区: {:?}",
            items.iter().map(|i| &i.key).collect::<Vec<_>>()
        );
        // 大小写不敏感
        let out2 = write
            .execute(
                &context,
                serde_json::json!({
                    "zone": "World", "item_type": "world", "key": "世界观",
                    "content": "双星废土", "summary": "双星"
                }),
            )
            .await
            .unwrap();
        assert!(out2.contains("（zone 'World' 已收编为 asset）"));
    }

    #[tokio::test]
    async fn test_board_write_nonsense_zone_still_rejected() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let registry = ToolRegistry::agency_default();
        let context = ctx(pool, AgentRole::Producer);
        let write = registry
            .get_for_role(AgentRole::Producer, "board_write")
            .unwrap();
        let err = write
            .execute(
                &context,
                serde_json::json!({"zone": "nonsense", "item_type": "note", "key": "x", "content": "y"}),
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("非法 zone"),
            "应为非法 zone: {}",
            err
        );
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
        // 版本冲突 → 错误回显（工具 Ok 但内容提示冲突，或
        // Err——以实现为准断言其一）
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

    #[test]
    fn tool_specs_for_role_producer_is_json_schema() {
        let registry = ToolRegistry::agency_default();
        let specs = registry.tool_specs_for_role(AgentRole::Producer);
        assert!(!specs.is_empty());
        for spec in &specs {
            assert_eq!(spec.parameters["type"], "object");
            assert!(spec.parameters.get("properties").is_some());
        }
        assert!(specs.iter().any(|s| s.name == "board_read"));
        assert!(specs.iter().any(|s| s.name == "story_info"));
    }

    #[test]
    fn test_new_role_tool_whitelists() {
        let registry = ToolRegistry::agency_default();
        // Writer: 读写 + creative_context
        assert!(registry
            .get_for_role(AgentRole::Writer, "board_write")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Writer, "board_revise")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Writer, "creative_context")
            .is_some());
        // Inspector: 只读
        assert!(registry
            .get_for_role(AgentRole::Inspector, "board_read")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Inspector, "story_info")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Inspector, "asset_query")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Inspector, "creative_context")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::Inspector, "board_write")
            .is_none());
        // OutlinePlanner: 读写 + 查询
        assert!(registry
            .get_for_role(AgentRole::OutlinePlanner, "board_write")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::OutlinePlanner, "board_read")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::OutlinePlanner, "story_info")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::OutlinePlanner, "asset_query")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::OutlinePlanner, "creative_context")
            .is_some());
        // StyleMimic: 只读 + creative_context
        assert!(registry
            .get_for_role(AgentRole::StyleMimic, "board_read")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::StyleMimic, "creative_context")
            .is_some());
        assert!(registry
            .get_for_role(AgentRole::StyleMimic, "board_write")
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

    #[tokio::test]
    async fn test_story_info_includes_creative_context() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "创作上下文信息".into(),
                description: Some("测试".into()),
                genre: Some("科幻".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let registry = ToolRegistry::agency_default();
        let mut context = ctx(pool, AgentRole::LeadWriter);
        context.story_id = story.id.clone();
        let tool = registry
            .get_for_role(AgentRole::LeadWriter, "story_info")
            .unwrap();
        let out = tool.execute(&context, serde_json::json!({})).await.unwrap();
        assert!(
            out.contains("创作上下文："),
            "应包含创作上下文标题: {}",
            out
        );
        assert!(out.contains("叙事阶段:"), "应包含叙事阶段: {}", out);
        assert!(out.contains("伏笔提示"), "应包含伏笔提示: {}", out);
    }

    #[tokio::test]
    async fn test_asset_query_characters_snapshot_format() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "资产书2".into(),
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
            conn.execute(
                "INSERT INTO character_states (id, story_id, character_id, current_location, current_emotion, active_goal, secrets_known, secrets_unknown, arc_progress, last_updated)
                 VALUES ('cs1', ?1, 'c1', '星环废墟', '疲惫', '找到水源', '[]', '[]', 0.1, '2026-01-01')",
                rusqlite::params![story.id],
            ).unwrap();
        }
        let registry = ToolRegistry::agency_default();
        let mut context = ctx(pool, AgentRole::LeadWriter);
        context.story_id = story.id.clone();
        let tool = registry
            .get_for_role(AgentRole::LeadWriter, "asset_query")
            .unwrap();
        let out = tool
            .execute(&context, serde_json::json!({"kind": "characters"}))
            .await
            .unwrap();
        assert!(
            out.contains("阿苔 | 位置: 星环废墟 | 情绪: 疲惫 | 目标: 找到水源"),
            "应返回快照格式角色状态: {}",
            out
        );
    }

    #[tokio::test]
    async fn test_creative_context_tool_registered_and_whitelisted() {
        let registry = ToolRegistry::agency_default();
        assert!(
            registry.tools.contains_key("creative_context"),
            "creative_context 应已注册"
        );
        for role in AgentRole::all() {
            assert!(
                registry.get_for_role(role, "creative_context").is_some(),
                "{:?} 应可调用 creative_context",
                role
            );
        }
    }

    #[tokio::test]
    async fn test_creative_context_tool_returns_sections() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "创作上下文测试".into(),
                description: Some("测试故事".into()),
                genre: Some("科幻".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let registry = ToolRegistry::agency_default();
        let mut context = ctx(pool, AgentRole::Producer);
        context.story_id = story.id.clone();
        let tool = registry
            .get_for_role(AgentRole::Producer, "creative_context")
            .unwrap();
        let out = tool
            .execute(&context, serde_json::json!({"chapter_number": 1}))
            .await
            .unwrap();
        assert!(out.contains("叙事阶段指引:"), "应包含叙事阶段指引: {}", out);
        assert!(out.contains("风格 DNA:"), "应包含风格 DNA: {}", out);
        assert!(out.contains("角色状态:"), "应包含角色状态: {}", out);
        assert!(
            out.contains("写作时间束提示:"),
            "应包含写作时间束提示: {}",
            out
        );
    }

    #[test]
    fn catalog_for_role_is_name_and_one_line() {
        let reg = ToolRegistry::agency_default();
        let cat = reg.catalog_for_role(crate::agency::models::AgentRole::EditorAuditor);
        assert!(cat.contains("board_read"));
        assert!(cat.contains("asset_read"));
        assert!(!cat.contains("参数:"), "schema 不得进目录: {cat}");
        assert!(!cat.contains("用法:"), "用法并进同一行，不另起标签: {cat}");
        assert!(
            !cat.contains("\"type\":\"object\""),
            "目录不得倾倒 JSON schema: {cat}"
        );
    }

    #[test]
    fn catalog_includes_usage_for_read_write_query() {
        let reg = ToolRegistry::agency_default();
        let writer = reg.catalog_for_role(crate::agency::models::AgentRole::LeadWriter);
        assert!(writer.contains("资产已注入时不要轮询 board_read 拉全文"));
        assert!(writer.contains("正文写入 draft 区，勿覆盖 user_created 资产"));
        assert!(
            writer.contains("写角色后必须再写 item_type=relationship"),
            "{writer}"
        );
        let producer = reg.catalog_for_role(crate::agency::models::AgentRole::Producer);
        assert!(producer.contains("写角色后必须再写 item_type=relationship"));
        assert!(writer.contains("按 kind 查询，不要倾倒全表"));
        assert!(writer.contains("一次只读一张"));
        let editor = reg.catalog_for_role(crate::agency::models::AgentRole::EditorAuditor);
        assert!(editor.contains("资产已注入时不要轮询 board_read 拉全文"));
        assert!(!editor.contains("正文写入 draft 区，勿覆盖 user_created 资产"));
    }

    #[tokio::test]
    async fn asset_read_returns_full_card_and_refuses_unknown_name() {
        use crate::db::{dto::CreateCharacterRequest, repositories::CharacterRepository};
        let pool = create_test_pool().unwrap();
        seed_run(&pool);
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "读卡".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story.id.clone(),
                name: "苏会山".into(),
                background: Some("藩王".into()),
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: None,
                is_auto_generated: None,
                emotional_core: Some("权欲".into()),
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        let registry = ToolRegistry::agency_default();
        let mut context = ctx(pool, AgentRole::Producer);
        context.story_id = story.id;
        let tool = registry
            .get_for_role(AgentRole::Producer, "asset_read")
            .unwrap();
        let ok = tool
            .execute(
                &context,
                serde_json::json!({"kind": "character", "name": "苏会山"}),
            )
            .await
            .unwrap();
        assert!(ok.contains("姓名：苏会山"), "{ok}");
        assert!(ok.contains("情感内核：权欲"), "{ok}");
        let miss = tool
            .execute(
                &context,
                serde_json::json!({"kind": "character", "name": "金敏秀"}),
            )
            .await
            .unwrap();
        assert!(miss.contains("无此角色"), "{miss}");
        assert!(registry
            .get_for_role(AgentRole::EditorAuditor, "asset_read")
            .is_some());
    }
}
