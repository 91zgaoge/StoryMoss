#![allow(dead_code)]
//! 两步思维链Ingest流程
//!
//! 基于llm_wiki方法论：
//! Step 1: 分析 - 使用LLM深入分析内容
//! Step 2: 生成 - 基于分析结果生成结构化知识

use std::{collections::HashMap, str::FromStr};

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::{
    db::{
        models::{Entity, EntityType, Relation, RelationType},
        DbPool,
    },
    embeddings::{embed_entity, EntityEmbeddingRequest},
    llm::LlmService,
    narrative::event::{EventType, NarrativeEvent},
    router::TaskType,
};

/// Ingest作业记录
#[derive(Debug, Clone, Serialize)]
pub struct IngestJob {
    pub id: String,
    pub story_id: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Ingest管道
pub struct IngestPipeline {
    llm_service: LlmService,
    pool: Option<DbPool>,
    app_handle: Option<tauri::AppHandle>,
}

/// 待Ingest的内容
#[derive(Debug, Clone)]
pub struct IngestContent {
    pub text: String,
    pub source: String,
    pub story_id: String,
    pub scene_id: Option<String>,
}

/// Step 1: 内容分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysis {
    /// 识别出的实体
    #[serde(default)]
    pub entities: Vec<AnalyzedEntity>,
    /// 实体间的关系
    #[serde(default)]
    pub relationships: Vec<AnalyzedRelation>,
    /// 关键事件
    #[serde(default)]
    pub events: Vec<AnalyzedEvent>,
    /// 情感分析
    pub sentiment: SentimentAnalysis,
    /// 伏笔和照应
    #[serde(default)]
    pub foreshadowing: Vec<Foreshadowing>,
    /// 主题标签
    #[serde(default)]
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedEntity {
    pub name: String,
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub mentions: Vec<serde_json::Value>,
    pub attributes: serde_json::Value,
}

fn default_entity_type() -> String {
    "Unknown".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedRelation {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default = "default_strength")]
    pub strength: f32,
}

fn default_strength() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedEvent {
    pub description: String,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default = "default_importance")]
    pub importance: i32,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub consequence: String,
}

fn default_importance() -> i32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub overall: String,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    #[serde(default)]
    pub arc: Vec<SentimentPoint>,
}

fn default_intensity() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentPoint {
    #[serde(default)]
    pub position: f32,
    pub sentiment: String,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Foreshadowing {
    pub content: String,
    #[serde(default = "default_fore_type")]
    pub type_: String,
    #[serde(default)]
    pub related_to: Vec<String>,
}

fn default_fore_type() -> String {
    "setup".to_string()
}

/// Step 2: 生成的知识
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedKnowledge {
    pub entities: Vec<EntityProfile>,
    pub relations: Vec<RelationProfile>,
    pub events: Vec<EventProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityProfile {
    pub name: String,
    pub entity_type: EntityType,
    pub description: String,
    pub attributes: serde_json::Value,
    pub importance: i32, // 1-10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationProfile {
    pub source: String,
    pub target: String,
    pub relation_type: RelationType,
    pub description: String,
    pub strength: f32,
    pub evolution: String, // 关系如何发展
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProfile {
    pub title: String,
    pub description: String,
    pub importance: i32,
    pub impact: String,
}

/// 从 LLM 响应中提取 JSON，处理 markdown 代码块、前导文字等常见格式问题
fn extract_json(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    // 尝试提取 ```json ... ``` 或 ``` ... ``` 中的内容
    if let Some(start) = trimmed.find("```") {
        let after_start = &trimmed[start + 3..];
        let code_start = if after_start.starts_with("json") || after_start.starts_with("JSON") {
            after_start[4..].trim_start()
        } else {
            after_start.trim_start()
        };
        if let Some(end) = code_start.find("```") {
            // 围栏内的内容仍可能有尾部多余文本，用括号匹配精确提取
            let inner = code_start[..end].trim();
            return Ok(crate::narrative::extract_and_sanitize_json(inner)?);
        }
    }
    // fallback: 用括号匹配提取第一个完整 JSON 对象
    // （不使用 rfind('}')，因为 LLM 可能在 JSON 后输出包含 } 的额外文本）
    crate::narrative::extract_and_sanitize_json(trimmed)
}

impl IngestPipeline {
    pub fn new(llm_service: LlmService) -> Self {
        Self {
            llm_service,
            pool: None,
            app_handle: None,
        }
    }

    pub fn with_pool(mut self, pool: DbPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_app_handle(mut self, app_handle: tauri::AppHandle) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    pub fn pool(&self) -> Option<&DbPool> {
        self.pool.as_ref()
    }

    /// 执行两步思维链Ingest，自动追踪作业状态
    pub async fn ingest(
        &self,
        content: &IngestContent,
    ) -> Result<IngestResult, Box<dyn std::error::Error>> {
        self.ingest_with_cancel(content, None).await
    }

    /// 支持取消令牌的 ingest 版本。
    pub async fn ingest_with_cancel(
        &self,
        content: &IngestContent,
        cancel: Option<&CancellationToken>,
    ) -> Result<IngestResult, Box<dyn std::error::Error>> {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                log::info!(
                    "[IngestPipeline] Ingest cancelled before start for story {}",
                    content.story_id
                );
                return Err("ingest cancelled".into());
            }
        }

        let job_id = uuid::Uuid::new_v4().to_string();
        let resource_type = content
            .scene_id
            .as_ref()
            .map(|_| "scene".to_string())
            .unwrap_or_else(|| "chapter".to_string());

        // 插入 pending 记录
        if let Err(e) = self.create_job(
            &job_id,
            &content.story_id,
            &resource_type,
            content.scene_id.as_deref(),
        ) {
            log::warn!("[IngestPipeline] Failed to create job record: {}", e);
        }

        let result = self.run_ingest(content, cancel).await;

        match &result {
            Ok(_) => {
                if let Err(e) = self.complete_job(&job_id) {
                    log::warn!("[IngestPipeline] Failed to complete job record: {}", e);
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                if let Err(e) = self.fail_job(&job_id, &error_msg) {
                    log::warn!("[IngestPipeline] Failed to fail job record: {}", e);
                }
            }
        }

        result
    }

    async fn run_ingest(
        &self,
        content: &IngestContent,
        cancel: Option<&CancellationToken>,
    ) -> Result<IngestResult, Box<dyn std::error::Error>> {
        // Step 1: 分析阶段 — 实体、关系、情感、伏笔、主题
        let analysis = self.analyze_content(content, cancel).await?;

        // Step 1b: 将分析出的伏笔持久化到 foreshadowing_tracker，
        // 接通"续写中隐含埋设伏笔 → 自动追踪 → 后续续写提醒回收"闭环。
        if let Err(e) = self.persist_foreshadowings(&analysis.foreshadowing, content) {
            log::warn!("[Ingest] 保存伏笔失败: {}", e);
        }

        // Step 1c: 将角色实体的位置/情绪状态写入 character_states，
        // 接通审计报告 P0-2：此前 character_states 表运行时为空，
        // 导致 build_writer_prompt 的【角色当前状态】段永远为空。
        if let Err(e) = self.persist_character_states(&analysis.entities, content) {
            log::warn!("[Ingest] 保存角色状态失败: {}", e);
        }

        // Step 2: 生成阶段 — 结构化知识档案
        let knowledge = self.generate_knowledge(&analysis, cancel).await?;

        // Step 3: LitSeg 叙事事件提取 — 从文本中提取推动情节的关键事件
        let mut narrative_events = self
            .extract_narrative_events(content, &analysis, cancel)
            .await
            .unwrap_or_default();

        // Step 3b: 构建事件因果链并保存到数据库
        Self::build_event_chain(&mut narrative_events);
        if let Err(e) = self.save_narrative_events(&narrative_events) {
            log::warn!("[Ingest] 保存叙事事件失败: {}", e);
        }

        // 转换为数据库模型
        let new_entities = self.convert_entities(&knowledge.entities, content);

        // 实体链接：查询已有实体，按名称去重，返回 name -> id 映射
        let (entities, name_to_id) = self.link_entities(new_entities, &content.story_id).await?;

        // 转换关系，使用链接后的实体ID
        let relations = self.convert_relations(&knowledge.relations, content, &name_to_id);

        Ok(IngestResult {
            analysis,
            knowledge,
            entities,
            relations,
            narrative_events,
        })
    }

    fn create_job(
        &self,
        job_id: &str,
        story_id: &str,
        resource_type: &str,
        resource_id: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO ingest_jobs (id, story_id, resource_type, resource_id, status, \
             created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![job_id, story_id, resource_type, resource_id, "pending", now],
        )?;
        self.emit_job_updated(story_id);
        Ok(())
    }

    fn complete_job(&self, job_id: &str) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE ingest_jobs SET status = ?1, completed_at = ?2 WHERE id = ?3",
            rusqlite::params!["completed", now, job_id],
        )?;
        if let Ok(story_id) = conn.query_row(
            "SELECT story_id FROM ingest_jobs WHERE id = ?1",
            [job_id],
            |row| row.get::<_, String>(0),
        ) {
            self.emit_job_updated(&story_id);
        }
        Ok(())
    }

    fn fail_job(&self, job_id: &str, error_message: &str) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE ingest_jobs SET status = ?1, error_message = ?2, completed_at = ?3 WHERE id = \
             ?4",
            rusqlite::params!["failed", error_message, now, job_id],
        )?;
        if let Ok(story_id) = conn.query_row(
            "SELECT story_id FROM ingest_jobs WHERE id = ?1",
            [job_id],
            |row| row.get::<_, String>(0),
        ) {
            self.emit_job_updated(&story_id);
        }
        Ok(())
    }

    fn emit_job_updated(&self, story_id: &str) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(
                "ingest-job-updated",
                serde_json::json!({ "story_id": story_id }),
            );
        }
    }

    /// 查询最近 N 条 ingest 作业
    pub fn get_recent_jobs(
        story_id: &str,
        limit: i32,
        pool: &DbPool,
    ) -> Result<Vec<IngestJob>, rusqlite::Error> {
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, resource_type, resource_id, status, error_message, created_at, \
             completed_at
             FROM ingest_jobs WHERE story_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![story_id, limit], |row| {
            Ok(IngestJob {
                id: row.get(0)?,
                story_id: row.get(1)?,
                resource_type: row.get(2)?,
                resource_id: row.get(3)?,
                status: row.get(4)?,
                error_message: row.get(5)?,
                created_at: row.get(6)?,
                completed_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// Step 1: 使用LLM分析内容
    async fn analyze_content(
        &self,
        content: &IngestContent,
        cancel: Option<&CancellationToken>,
    ) -> Result<ContentAnalysis, Box<dyn std::error::Error>> {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err("ingest cancelled before analyze_content".into());
            }
        }

        // v0.21.0: 优先从 PromptRegistry 读取覆盖
        let base_prompt = if let Some(pool) = self.pool.as_ref() {
            let tpl = crate::prompts::registry::resolve_prompt(pool, "memory_content_analysis")
                .ok()
                .or_else(|| crate::prompts::registry::resolve_prompt_default("memory_content_analysis"))
                .unwrap_or_else(|| r#"你是一位专业的小说分析师。请深入分析以下小说内容，提取结构化信息。

【内容】
{}

【来源】
{}

【输出要求】
请按以下JSON格式输出分析结果。必须严格遵循格式，不要添加任何注释或说明文字：
{{
  "entities": [
    {{
      "name": "实体名称（必须是文本中明确出现的名字，禁止编造）",
      "entity_type": "Character", // 可选值: Character(人物), Location(地点), Item(物品), Organization(组织), Concept(概念), Event(事件)
      "mentions": ["文本中出现该实体的具体片段，引用原文1-2句"],
      "attributes": {{"key": "value"}}
    }}
  ],
  "relationships": [
    {{
      "source": "源实体名称",
      "target": "目标实体名称",
      "relation_type": "关系类型（如: 朋友/敌人/家人/师徒/上下级/爱慕/仇恨/竞争）",
      "evidence": "支持该关系的原文引用",
      "strength": 0.8 // 0.0-1.0 的浮点数
    }}
  ],
  "events": [
    {{
      "description": "事件描述（30字以内）",
      "participants": ["参与者1", "参与者2"],
      "importance": 8, // 1-10 的整数
      "trigger": "触发原因",
      "consequence": "后果影响"
    }}
  ],
  "sentiment": {{
    "overall": "positive", // 可选值: positive/negative/neutral
    "intensity": 0.7, // 0.0-1.0 的浮点数
    "arc": [{{"position": 0.5, "sentiment": "tense", "intensity": 0.8}}]
  }},
  "foreshadowing": [
    {{
      "content": "伏笔内容",
      "type_": "setup", // 可选值: setup(埋下)/payoff(回收)
      "related_to": ["相关内容"]
    }}
  ],
  "themes": ["主题1", "主题2"]
}}

【Few-shot示例】
输入: "林枫站在青云山顶，望着远处的云海。他握紧手中的长剑，心中暗暗发誓要找到杀害师父的凶手。"
输出: {{
  "entities": [
    {{"name": "林枫", "entity_type": "Character", "mentions": ["林枫站在青云山顶"], "attributes": {{"location": "青云山顶", "mood": "愤怒/决心"}}}},
    {{"name": "青云山", "entity_type": "Location", "mentions": ["林枫站在青云山顶"], "attributes": {{}}}} ,
    {{"name": "长剑", "entity_type": "Item", "mentions": ["握紧手中的长剑"], "attributes": {{}}}}
  ],
  "relationships": [
    {{"source": "林枫", "target": "师父", "relation_type": "师徒", "evidence": "要找到杀害师父的凶手", "strength": 0.9}}
  ],
  "events": [
    {{"description": "林枫在青云山顶发誓复仇", "participants": ["林枫"], "importance": 9, "trigger": "师父被杀", "consequence": "林枫决心复仇"}}
  ],
  "sentiment": {{"overall": "negative", "intensity": 0.8, "arc": [{{"position": 0.5, "sentiment": "determined", "intensity": 0.9}}]}},
  "foreshadowing": [{{"content": "要找到杀害师父的凶手", "type_": "setup", "related_to": ["复仇主线"]}}],
  "themes": ["复仇", "成长"]
}}

【重要规则】
1. 实体名称必须是文本中明确出现的名字，禁止编造或推断未命名的实体
2. 实体类型必须严格使用: Character/Location/Item/Organization/Concept/Event 之一
3. 关系必须有明确的原文证据支持，禁止臆测
4. 事件重要性评估: 1(轻微提及) 到 10(核心转折)
5. 确保输出是合法的JSON，不要添加markdown代码块标记
6. 如果文本中没有足够信息，返回空数组即可，不要编造"#.to_string());
            let mut vars = std::collections::HashMap::new();
            vars.insert("content".to_string(), content.text.clone());
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            format!(
                r#"你是一位专业的小说分析师。请深入分析以下小说内容，提取结构化信息。

【内容】
{}

【来源】
{}

【输出要求】
请按以下JSON格式输出分析结果。必须严格遵循格式，不要添加任何注释或说明文字：
{{
  "entities": [
    {{
      "name": "实体名称（必须是文本中明确出现的名字，禁止编造）",
      "entity_type": "Character", // 可选值: Character(人物), Location(地点), Item(物品), Organization(组织), Concept(概念), Event(事件)
      "mentions": ["文本中出现该实体的具体片段，引用原文1-2句"],
      "attributes": {{"key": "value"}}
    }}
  ],
  "relationships": [
    {{
      "source": "源实体名称",
      "target": "目标实体名称",
      "relation_type": "关系类型（如: 朋友/敌人/家人/师徒/上下级/爱慕/仇恨/竞争）",
      "evidence": "支持该关系的原文引用",
      "strength": 0.8 // 0.0-1.0 的浮点数
    }}
  ],
  "events": [
    {{
      "description": "事件描述（30字以内）",
      "participants": ["参与者1", "参与者2"],
      "importance": 8, // 1-10 的整数
      "trigger": "触发原因",
      "consequence": "后果影响"
    }}
  ],
  "sentiment": {{
    "overall": "positive", // 可选值: positive/negative/neutral
    "intensity": 0.7, // 0.0-1.0 的浮点数
    "arc": [{{"position": 0.5, "sentiment": "tense", "intensity": 0.8}}]
  }},
  "foreshadowing": [
    {{
      "content": "伏笔内容",
      "type_": "setup", // 可选值: setup(埋下)/payoff(回收)
      "related_to": ["相关内容"]
    }}
  ],
  "themes": ["主题1", "主题2"]
}}

【Few-shot示例】
输入: "林枫站在青云山顶，望着远处的云海。他握紧手中的长剑，心中暗暗发誓要找到杀害师父的凶手。"
输出: {{
  "entities": [
    {{"name": "林枫", "entity_type": "Character", "mentions": ["林枫站在青云山顶"], "attributes": {{"location": "青云山顶", "mood": "愤怒/决心"}}}},
    {{"name": "青云山", "entity_type": "Location", "mentions": ["林枫站在青云山顶"], "attributes": {{}}}} ,
    {{"name": "长剑", "entity_type": "Item", "mentions": ["握紧手中的长剑"], "attributes": {{}}}}
  ],
  "relationships": [
    {{"source": "林枫", "target": "师父", "relation_type": "师徒", "evidence": "要找到杀害师父的凶手", "strength": 0.9}}
  ],
  "events": [
    {{"description": "林枫在青云山顶发誓复仇", "participants": ["林枫"], "importance": 9, "trigger": "师父被杀", "consequence": "林枫决心复仇"}}
  ],
  "sentiment": {{"overall": "negative", "intensity": 0.8, "arc": [{{"position": 0.5, "sentiment": "determined", "intensity": 0.9}}]}},
  "foreshadowing": [{{"content": "要找到杀害师父的凶手", "type_": "setup", "related_to": ["复仇主线"]}}],
  "themes": ["复仇", "成长"]
}}

【重要规则】
1. 实体名称必须是文本中明确出现的名字，禁止编造或推断未命名的实体
2. 实体类型必须严格使用: Character/Location/Item/Organization/Concept/Event 之一
3. 关系必须有明确的原文证据支持，禁止臆测
4. 事件重要性评估: 1(轻微提及) 到 10(核心转折)
5. 确保输出是合法的JSON，不要添加markdown代码块标记
6. 如果文本中没有足够信息，返回空数组即可，不要编造"#,
                content.text, content.source
            )
        };
        let mut last_error: Option<String> = None;
        for attempt in 0..3 {
            if let Some(token) = cancel {
                if token.is_cancelled() {
                    return Err("ingest cancelled during analyze_content".into());
                }
            }

            let mut prompt = base_prompt.clone();
            if let Some(ref err) = last_error {
                prompt.push_str(&format!(
                    "\n\n【修正要求】之前输出存在问题: \
                     {}。请务必修正后，仅输出合法的JSON对象，不要添加任何markdown代码块标记。",
                    err
                ));
            }
            let response = self
                .llm_service
                .generate_for_task(
                    TaskType::Analysis,
                    prompt,
                    None,
                    None,
                    Some("记忆-内容分析"),
                )
                .await?;
            let json_str = match extract_json(&response.content) {
                Ok(s) => s,
                Err(e) => {
                    last_error = Some(format!("JSON提取失败: {}", e));
                    log::warn!(
                        "[Ingest Step1] Attempt {}: JSON extraction failed: {}. Preview: {}",
                        attempt + 1,
                        e,
                        &response.content.chars().take(200).collect::<String>()
                    );
                    continue;
                }
            };
            match serde_json::from_str::<ContentAnalysis>(&json_str) {
                Ok(analysis) => {
                    if let Err(e) = validate_content_analysis(&analysis) {
                        last_error = Some(e);
                        log::warn!(
                            "[Ingest Step1] Attempt {}: validation failed: {}",
                            attempt + 1,
                            last_error.as_ref().unwrap()
                        );
                        continue;
                    }
                    return Ok(analysis);
                }
                Err(e) => {
                    last_error = Some(format!("JSON解析失败: {}", e));
                    log::warn!(
                        "[Ingest Step1] Attempt {}: JSON parse error: {}. Preview: {}",
                        attempt + 1,
                        e,
                        &json_str.chars().take(300).collect::<String>()
                    );
                }
            }
        }

        Err(format!(
            "[Ingest Step1] 3次尝试均失败. 最后错误: {}",
            last_error.unwrap_or_default()
        )
        .into())
    }

    /// Step 2: 基于分析生成结构化知识
    async fn generate_knowledge(
        &self,
        analysis: &ContentAnalysis,
        cancel: Option<&CancellationToken>,
    ) -> Result<GeneratedKnowledge, Box<dyn std::error::Error>> {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err("ingest cancelled before generate_knowledge".into());
            }
        }

        let analysis_json = serde_json::to_string_pretty(analysis)?;

        let base_prompt = format!(
            r#"基于以下内容分析结果，生成知识库条目：

【分析结果】
{}

请生成详细的知识档案，按以下JSON格式输出：
{{
  "entities": [
    {{
      "name": "实体名称",
      "entity_type": "Character/Location/Item/Organization/Concept/Event",
      "description": "详细描述",
      "attributes": {{}},
      "importance": 8
    }}
  ],
  "relations": [
    {{
      "source": "源实体",
      "target": "目标实体",
      "relation_type": "Friend/Enemy/Family/...",
      "description": "关系描述",
      "strength": 0.8,
      "evolution": "关系发展趋势"
    }}
  ],
  "events": [
    {{
      "title": "事件标题",
      "description": "详细描述",
      "importance": 9,
      "impact": "对故事的影响"
    }}
  ]
}}

注意：
1. 为每个实体生成完整档案
2. 计算关系强度（0-1）
3. 评估事件重要性（1-10）
4. 确保JSON格式正确"#,
            analysis_json
        );

        let mut last_error: Option<String> = None;
        for attempt in 0..3 {
            if let Some(token) = cancel {
                if token.is_cancelled() {
                    return Err("ingest cancelled during generate_knowledge".into());
                }
            }

            let mut prompt = base_prompt.clone();
            if let Some(ref err) = last_error {
                prompt.push_str(&format!(
                    "\n\n【修正要求】之前输出存在问题: \
                     {}。请务必修正后，仅输出合法的JSON对象，不要添加任何markdown代码块标记。",
                    err
                ));
            }
            let response = self
                .llm_service
                .generate_for_task(
                    TaskType::WorldBuilding,
                    prompt,
                    None,
                    None,
                    Some("记忆-生成知识"),
                )
                .await?;
            let json_str = match extract_json(&response.content) {
                Ok(s) => s,
                Err(e) => {
                    last_error = Some(format!("JSON提取失败: {}", e));
                    log::warn!(
                        "[Ingest Step2] Attempt {}: JSON extraction failed: {}. Preview: {}",
                        attempt + 1,
                        e,
                        &response.content.chars().take(200).collect::<String>()
                    );
                    continue;
                }
            };
            match serde_json::from_str::<GeneratedKnowledge>(&json_str) {
                Ok(knowledge) => {
                    if let Err(e) = validate_generated_knowledge(&knowledge) {
                        last_error = Some(e);
                        log::warn!(
                            "[Ingest Step2] Attempt {}: validation failed: {}",
                            attempt + 1,
                            last_error.as_ref().unwrap()
                        );
                        continue;
                    }
                    return Ok(knowledge);
                }
                Err(e) => {
                    last_error = Some(format!("JSON解析失败: {}", e));
                    log::warn!(
                        "[Ingest Step2] Attempt {}: JSON parse error: {}. Preview: {}",
                        attempt + 1,
                        e,
                        &json_str.chars().take(300).collect::<String>()
                    );
                }
            }
        }

        Err(format!(
            "[Ingest Step2] 3次尝试均失败. 最后错误: {}",
            last_error.unwrap_or_default()
        )
        .into())
    }

    // ==================== LitSeg Step 3: 叙事事件提取 ====================

    /// Step 3: LitSeg 叙事事件提取 — 从文本中提取推动情节发展的关键事件
    async fn extract_narrative_events(
        &self,
        content: &IngestContent,
        analysis: &ContentAnalysis,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<NarrativeEvent>, Box<dyn std::error::Error>> {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err("ingest cancelled before extract_narrative_events".into());
            }
        }

        // 构建提示词
        let characters: Vec<String> = analysis
            .entities
            .iter()
            .filter(|e| e.entity_type == "Character")
            .map(|e| e.name.clone())
            .collect();

        // v0.19.0: 从 PromptRegistry 读取提示词模板
        let prompt_text = if let Some(ref pool) = self.pool {
            if let Ok(tpl) =
                crate::prompts::registry::resolve_prompt(pool, "narrative_event_extraction")
            {
                let mut result = tpl;
                result = result.replace("{{characters}}", &characters.join(", "));
                result = result.replace("{{prior_events}}", "无");
                result = result.replace("{{content}}", &content.text);
                result
            } else {
                Self::fallback_narrative_event_extraction_prompt(&characters, &content.text)
            }
        } else {
            Self::fallback_narrative_event_extraction_prompt(&characters, &content.text)
        };

        let mut last_error: Option<String> = None;
        for _attempt in 0..3 {
            if let Some(token) = cancel {
                if token.is_cancelled() {
                    return Err("ingest cancelled during extract_narrative_events".into());
                }
            }

            let mut current_prompt = prompt_text.clone();
            if let Some(ref err) = last_error {
                current_prompt.push_str(&format!(
                    "\n\n【修正要求】之前输出存在问题: {}。请务必修正后，仅输出合法的JSON数组，不要添加任何markdown代码块标记。",
                    err
                ));
            }

            let response = self
                .llm_service
                .generate_for_task(
                    TaskType::Analysis,
                    current_prompt,
                    None,
                    None,
                    Some("记忆-叙事事件提取"),
                )
                .await?;
            let json_str = match crate::narrative::extract_and_sanitize_json(&response.content) {
                Ok(s) => s,
                Err(e) => {
                    last_error = Some(format!("JSON提取失败: {}", e));
                    continue;
                }
            };

            match serde_json::from_str::<Vec<NarrativeEventExtraction>>(&json_str) {
                Ok(extractions) => {
                    let events = self.convert_to_narrative_events(extractions, content);
                    return Ok(events);
                }
                Err(e) => {
                    last_error = Some(format!("JSON解析失败: {}", e));
                    continue;
                }
            }
        }

        Err(format!(
            "[Ingest Step3] 3次尝试均失败. 最后错误: {}",
            last_error.unwrap_or_default()
        )
        .into())
    }

    /// v0.19.0: 叙事事件提取的 fallback 提示词（当 PromptRegistry 不可用时）
    fn fallback_narrative_event_extraction_prompt(characters: &[String], content: &str) -> String {
        format!(
            "你是一个专业的叙事分析专家。你的任务是从小说文本中提取推动情节发展的关键事件。\n\n\
            分析标准：\n\
            1. 「有效事件」= 真正推动情节发展的关键节点，不是过渡性描述\n\
            2. 事件强度（0.0-1.0）反映对后续情节的影响程度\n\
            3. 如果角色发生内在改变（信念、态度、关系本质），标记为角色弧光\n\
            4. 伏笔埋设和回收是独立事件，即使在同一场景中\n\
            5. 保持与已有事件链的因果一致性\n\n\
            输出 JSON 格式的事件数组。\n\n\
            【角色列表】\n{}\n\n\
            【已有事件链（前序）】\n无\n\n\
            【当前文本】\n{}\n\n\
            请输出 JSON 格式的事件数组，每个事件包含：\n\
            - event_type: 事件类型（introduction/turning_point/climax/resolution/revelation/conflict_eruption/character_arc/foreshadow_setup/foreshadow_payoff/transition）\n\
            - intensity: 事件强度（0.0-1.0）\n\
            - sentiment: 情感极性（-1.0 到 +1.0）\n\
            - description: 事件描述（20-50字）\n\
            - involved_character_ids: 涉及的角色 ID 数组\n\
            - conflict_types: 涉及的冲突类型数组\n\n\
            只输出 JSON，不要其他文字。",
            characters.join(", "),
            content
        )
    }

    /// 将 LLM 提取结果转换为 NarrativeEvent
    fn convert_to_narrative_events(
        &self,
        extractions: Vec<NarrativeEventExtraction>,
        content: &IngestContent,
    ) -> Vec<NarrativeEvent> {
        extractions
            .into_iter()
            .enumerate()
            .map(|(idx, ext)| NarrativeEvent {
                id: format!("nev_{}_{}", content.story_id, idx),
                story_id: content.story_id.clone(),
                chapter_number: 0, // 由调用方填充
                scene_id: content.scene_id.clone(),
                event_type: ext.event_type,
                intensity: ext.intensity.clamp(0.0, 1.0),
                sentiment: ext.sentiment.clamp(-1.0, 1.0),
                description: ext.description,
                involved_character_ids: ext.involved_character_ids,
                conflict_types: ext
                    .conflict_types
                    .iter()
                    .filter_map(|c| crate::db::ConflictType::from_str(c).ok())
                    .collect(),
                preceding_event_id: None,
                following_event_id: None,
                act_number: 1,
                position_in_act: 1,
                created_at: Local::now(),
            })
            .collect()
    }

    /// 构建事件因果链 — 基于事件类型和描述推断前后关系
    pub fn build_event_chain(events: &mut [NarrativeEvent]) {
        if events.len() < 2 {
            return;
        }

        // 按事件类型排序：introduction -> foreshadow_setup -> conflict_eruption ->
        // turning_point -> climax -> resolution -> foreshadow_payoff
        let type_order = |et: &EventType| match et {
            EventType::Introduction => 0,
            EventType::ForeshadowSetup => 1,
            EventType::Transition => 2,
            EventType::ConflictEruption => 3,
            EventType::CharacterArc => 4,
            EventType::TurningPoint => 5,
            EventType::Revelation => 6,
            EventType::Climax => 7,
            EventType::Resolution => 8,
            EventType::ForeshadowPayoff => 9,
        };

        // 按类型顺序排序（稳定排序保留原始顺序）
        events.sort_by_key(|e| type_order(&e.event_type));

        // 建立因果链
        for i in 0..events.len() - 1 {
            let next_id = events[i + 1].id.clone();
            events[i].following_event_id = Some(next_id);
            let prev_id = events[i].id.clone();
            events[i + 1].preceding_event_id = Some(prev_id);
        }
    }

    /// 保存叙事事件到数据库 — 融合后：直接更新 scenes 表的 narrative 字段
    fn save_narrative_events(&self, events: &[NarrativeEvent]) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        for event in events {
            let scene_id = match &event.scene_id {
                Some(id) => id,
                None => continue,
            };

            let event_types_json = serde_json::to_string(&[event.event_type.to_string()])
                .unwrap_or_else(|_| "[]".to_string());
            let result = conn.execute(
                "UPDATE scenes SET
                    narrative_intensity = ?2,
                    narrative_sentiment = ?3,
                    narrative_event_types = ?4,
                    narrative_preceding_scene_id = ?5,
                    narrative_following_scene_id = ?6,
                    act_number = ?7,
                    position_in_act = ?8
                 WHERE id = ?1",
                params![
                    scene_id,
                    event.intensity,
                    event.sentiment,
                    event_types_json,
                    event.preceding_event_id.as_ref().map(|s| s.as_str()),
                    event.following_event_id.as_ref().map(|s| s.as_str()),
                    event.act_number,
                    event.position_in_act,
                ],
            );

            if let Err(e) = result {
                log::warn!(
                    "[Ingest] 更新场景叙事字段失败 (scene_id={}): {}",
                    scene_id,
                    e
                );
            }
        }
        Ok(())
    }

    /// 将分析阶段提取的伏笔持久化到 foreshadowing_tracker。
    ///
    /// 接通审计报告 P0-1：续写中隐含埋设的伏笔此前只在内存中分析，
    /// 从不写入 foreshadowing_tracker，导致回收账本不完整、
    /// 后续续写的"待回收/逾期伏笔"提示会漏报。
    ///
    /// 仅注册 type_=setup 的伏笔（status=setup）。
    /// payoff 类型表示此处回收了既有伏笔，需按 related_to 模糊匹配既有记录
    /// 再 mark_payoff，但匹配不可靠，故暂不自动回收，留给用户/审计确认。
    /// 按 (story_id, content) 去重，防止同一伏笔重复登记。
    fn persist_foreshadowings(
        &self,
        foreshadowings: &[Foreshadowing],
        content: &IngestContent,
    ) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        if foreshadowings.is_empty() {
            return Ok(());
        }
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut registered = 0usize;
        for fs in foreshadowings {
            // 仅追踪"埋下"型伏笔
            if fs.type_ != "setup" {
                continue;
            }
            let trimmed = fs.content.trim();
            if trimmed.is_empty() {
                continue;
            }

            // 去重：同一 story 下已有相同 content 的 setup 伏笔则跳过
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM foreshadowing_tracker \
                     WHERE story_id = ?1 AND content = ?2 AND status = 'setup'",
                    params![&content.story_id, trimmed],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists {
                continue;
            }

            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Local::now().to_rfc3339();
            // importance 默认 5（中等），无 LLM 显式重要性评分时保守取值
            let importance = 5_i32;
            conn.execute(
                "INSERT INTO foreshadowing_tracker \
                 (id, story_id, content, setup_scene_id, status, importance, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 'setup', ?5, ?6)",
                params![
                    &id,
                    &content.story_id,
                    trimmed,
                    content.scene_id.as_ref().map(|s| s.as_str()),
                    importance,
                    now,
                ],
            )?;
            registered += 1;
        }

        if registered > 0 {
            log::info!(
                "[Ingest] 自动登记 {} 条伏笔到 foreshadowing_tracker (story_id={})",
                registered,
                content.story_id
            );
        }
        Ok(())
    }

    /// 将分析阶段提取的 Character 实体属性写入 character_states 表。
    ///
    /// 接通审计报告 P0-2：此前 character_states
    /// 表运行时为空（有人读、无人写）， 导致 build_writer_prompt
    /// 的【角色当前状态】段永远为空， 续写 LLM 看不到角色当前的位置/情绪/
    /// 目标。
    ///
    /// 从 AnalyzedEntity.attributes（serde_json::Value 对象）中提取
    /// location/mood/emotion/goal 等字段，按角色名匹配 characters 表得到
    /// character_id， 再 INSERT OR REPLACE 到 character_states。
    fn persist_character_states(
        &self,
        entities: &[AnalyzedEntity],
        content: &IngestContent,
    ) -> Result<(), rusqlite::Error> {
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let characters: Vec<AnalyzedEntity> = entities
            .iter()
            .filter(|e| e.entity_type.eq_ignore_ascii_case("Character"))
            .cloned()
            .collect();
        if characters.is_empty() {
            return Ok(());
        }
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut updated = 0usize;
        let now = chrono::Local::now().to_rfc3339();
        for ch in &characters {
            let name = ch.name.trim();
            if name.is_empty() {
                continue;
            }
            // 按名称匹配 characters 表，得到 character_id
            let character_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2",
                    params![&content.story_id, name],
                    |row| row.get(0),
                )
                .ok();
            let Some(character_id) = character_id else {
                continue; // 实体名未在 characters 表登记，跳过
            };

            // 从 attributes JSON 提取位置/情绪/目标
            let attrs = &ch.attributes;
            let location = json_str_field(attrs, &["location", "current_location", "位置"]);
            let emotion = json_str_field(attrs, &["mood", "emotion", "current_emotion", "情绪"]);
            let goal = json_str_field(attrs, &["goal", "active_goal", "目标"]);

            // INSERT OR REPLACE：保留既有 secrets/arc_progress（COALESCE 回填）
            conn.execute(
                "INSERT OR REPLACE INTO character_states (
                    id, story_id, character_id, current_location, current_emotion,
                    active_goal, secrets_known, secrets_unknown, arc_progress, last_updated
                ) VALUES (
                    COALESCE((SELECT id FROM character_states WHERE character_id = ?1), ?2),
                    ?3, ?1, ?4, ?5, ?6,
                    COALESCE((SELECT secrets_known FROM character_states WHERE character_id = ?1), '[]'),
                    COALESCE((SELECT secrets_unknown FROM character_states WHERE character_id = ?1), '[]'),
                    COALESCE((SELECT arc_progress FROM character_states WHERE character_id = ?1), 0.0),
                    ?7
                )",
                params![
                    &character_id,
                    uuid::Uuid::new_v4().to_string(),
                    &content.story_id,
                    location,
                    emotion,
                    goal,
                    now,
                ],
            )?;
            updated += 1;
        }

        if updated > 0 {
            log::info!(
                "[Ingest] 更新 {} 个角色状态到 character_states (story_id={})",
                updated,
                content.story_id
            );
        }
        Ok(())
    }

    /// 转换为数据库实体模型，并生成嵌入向量
    fn convert_entities(&self, profiles: &[EntityProfile], content: &IngestContent) -> Vec<Entity> {
        profiles
            .iter()
            .map(|profile| {
                // 生成实体嵌入
                let embedding = self.generate_entity_embedding(profile);

                Entity {
                    id: uuid::Uuid::new_v4().to_string(),
                    story_id: content.story_id.clone(),
                    name: profile.name.clone(),
                    entity_type: profile.entity_type.clone(),
                    attributes: profile.attributes.clone(),
                    embedding,
                    first_seen: Local::now(),
                    last_updated: Local::now(),
                    confidence_score: None,
                    access_count: 0,
                    last_accessed: None,
                    is_archived: false,
                    archived_at: None,
                }
            })
            .collect()
    }

    /// 为单个实体生成嵌入向量
    fn generate_entity_embedding(&self, profile: &EntityProfile) -> Option<Vec<f32>> {
        // 将 serde_json::Value 转换为 HashMap
        let attributes: HashMap<String, serde_json::Value> = match &profile.attributes {
            serde_json::Value::Object(map) => {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }
            _ => HashMap::new(),
        };

        let request = EntityEmbeddingRequest {
            entity_id: profile.name.clone(), // 临时ID
            name: profile.name.clone(),
            description: Some(profile.description.clone()),
            entity_type: profile.entity_type.to_string(),
            attributes,
        };

        match embed_entity(&request) {
            Ok(embedding) => {
                log::debug!("Generated embedding for entity: {}", profile.name);
                Some(embedding)
            }
            Err(e) => {
                log::warn!(
                    "Failed to generate embedding for entity {}: {}",
                    profile.name,
                    e
                );
                None
            }
        }
    }

    /// 转换为数据库关系模型，使用实体链接后的ID
    fn convert_relations(
        &self,
        profiles: &[RelationProfile],
        content: &IngestContent,
        name_to_id: &std::collections::HashMap<String, String>,
    ) -> Vec<Relation> {
        profiles
            .iter()
            .filter_map(|profile| {
                let source_id = name_to_id.get(&profile.source)?;
                let target_id = name_to_id.get(&profile.target)?;
                Some(Relation {
                    id: uuid::Uuid::new_v4().to_string(),
                    story_id: content.story_id.clone(),
                    source_id: source_id.clone(),
                    target_id: target_id.clone(),
                    relation_type: profile.relation_type.clone(),
                    strength: profile.strength,
                    evidence: vec![content.source.clone()],
                    first_seen: Local::now(),
                    confidence_score: None,
                })
            })
            .collect()
    }

    /// 查询 story_id 下的现有实体（用于实体链接去重）
    async fn load_existing_entities(
        &self,
        story_id: &str,
    ) -> Result<Vec<Entity>, Box<dyn std::error::Error>> {
        let Some(pool) = &self.pool else {
            return Ok(vec![]);
        };
        let pool = pool.clone();
        let story_id = story_id.to_string();

        let entities = tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
                 last_updated FROM kg_entities WHERE story_id = ?1",
            )?;
            let rows = stmt.query_map([&story_id], |row| {
                let entity_type_str: String = row.get(3)?;
                let entity_type =
                    EntityType::from_str(&entity_type_str).unwrap_or(EntityType::Character);
                let attributes_str: String = row.get(4).unwrap_or_default();
                let attributes =
                    serde_json::from_str(&attributes_str).unwrap_or(serde_json::Value::Null);
                let embedding_bytes: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_bytes.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
                        .collect()
                });

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: row.get(6)?,
                    last_updated: row.get(7)?,
                    confidence_score: None,
                    access_count: 0,
                    last_accessed: None,
                    is_archived: false,
                    archived_at: None,
                })
            })?;
            let result: Result<Vec<_>, _> = rows.collect();
            result
        })
        .await
        .map_err(|e| format!("spawn_blocking failed: {}", e))??;

        Ok(entities)
    }

    /// 实体链接：按名称去重，合并属性，返回 (去重后实体列表, name -> final_id
    /// 映射)
    async fn link_entities(
        &self,
        new_entities: Vec<Entity>,
        story_id: &str,
    ) -> Result<(Vec<Entity>, std::collections::HashMap<String, String>), Box<dyn std::error::Error>>
    {
        let existing = self.load_existing_entities(story_id).await?;

        let mut final_entities = Vec::new();
        let mut name_to_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for new_entity in new_entities {
            if let Some(existing_entity) = existing.iter().find(|e| e.name == new_entity.name) {
                // 命中已有实体：复用ID，合并属性，保留最新嵌入
                name_to_id.insert(new_entity.name.clone(), existing_entity.id.clone());
                let merged_attrs =
                    merge_json_objects(&existing_entity.attributes, &new_entity.attributes);
                final_entities.push(Entity {
                    id: existing_entity.id.clone(),
                    story_id: new_entity.story_id,
                    name: new_entity.name,
                    entity_type: new_entity.entity_type,
                    attributes: merged_attrs,
                    embedding: new_entity
                        .embedding
                        .or_else(|| existing_entity.embedding.clone()),
                    first_seen: existing_entity.first_seen,
                    last_updated: Local::now(),
                    confidence_score: new_entity
                        .confidence_score
                        .or(existing_entity.confidence_score),
                    access_count: existing_entity.access_count,
                    last_accessed: existing_entity.last_accessed,
                    is_archived: existing_entity.is_archived,
                    archived_at: existing_entity.archived_at,
                });
            } else {
                // 新实体
                name_to_id.insert(new_entity.name.clone(), new_entity.id.clone());
                final_entities.push(new_entity);
            }
        }

        Ok((final_entities, name_to_id))
    }
}

/// 合并两个 JSON 对象，overlay 覆盖 base 中的同名字段
fn merge_json_objects(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            let mut result = base_map.clone();
            for (k, v) in overlay_map.iter() {
                result.insert(k.clone(), v.clone());
            }
            serde_json::Value::Object(result)
        }
        _ => overlay.clone(),
    }
}

/// 从 JSON 对象中按候选键名提取第一个非空字符串值。
/// 用于从 AnalyzedEntity.attributes 提取 location/mood/goal 等字段，
/// 兼容 LLM 返回的不同键名拼写。
fn json_str_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let obj = value.as_object()?;
    for key in keys {
        if let Some(v) = obj.get(*key) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// 验证 ContentAnalysis 的结构有效性
fn validate_content_analysis(analysis: &ContentAnalysis) -> Result<(), String> {
    let valid_types = [
        "Character",
        "Location",
        "Item",
        "Organization",
        "Concept",
        "Event",
    ];
    for (i, entity) in analysis.entities.iter().enumerate() {
        if entity.name.trim().is_empty() {
            return Err(format!("Entity[{}]: name is empty", i));
        }
        if !valid_types.contains(&entity.entity_type.as_str()) {
            return Err(format!(
                "Entity[{}]: invalid entity_type '{}', expected one of {:?}",
                i, entity.entity_type, valid_types
            ));
        }
    }
    for (i, rel) in analysis.relationships.iter().enumerate() {
        if rel.source.trim().is_empty() || rel.target.trim().is_empty() {
            return Err(format!("Relation[{}]: source or target is empty", i));
        }
        if rel.relation_type.trim().is_empty() {
            return Err(format!("Relation[{}]: relation_type is empty", i));
        }
        if rel.strength < 0.0 || rel.strength > 1.0 {
            return Err(format!(
                "Relation[{}]: strength {} out of range [0,1]",
                i, rel.strength
            ));
        }
    }
    for (i, event) in analysis.events.iter().enumerate() {
        if event.description.trim().is_empty() {
            return Err(format!("Event[{}]: description is empty", i));
        }
        if event.importance < 1 || event.importance > 10 {
            return Err(format!(
                "Event[{}]: importance {} out of range [1,10]",
                i, event.importance
            ));
        }
    }
    Ok(())
}

/// 验证 GeneratedKnowledge 的结构有效性
fn validate_generated_knowledge(knowledge: &GeneratedKnowledge) -> Result<(), String> {
    for (i, entity) in knowledge.entities.iter().enumerate() {
        if entity.name.trim().is_empty() {
            return Err(format!("EntityProfile[{}]: name is empty", i));
        }
        if entity.importance < 1 || entity.importance > 10 {
            return Err(format!(
                "EntityProfile[{}]: importance {} out of range [1,10]",
                i, entity.importance
            ));
        }
    }
    for (i, rel) in knowledge.relations.iter().enumerate() {
        if rel.source.trim().is_empty() || rel.target.trim().is_empty() {
            return Err(format!("RelationProfile[{}]: source or target is empty", i));
        }
        if rel.strength < 0.0 || rel.strength > 1.0 {
            return Err(format!(
                "RelationProfile[{}]: strength {} out of range [0,1]",
                i, rel.strength
            ));
        }
    }
    for (i, event) in knowledge.events.iter().enumerate() {
        if event.title.trim().is_empty() {
            return Err(format!("EventProfile[{}]: title is empty", i));
        }
        if event.importance < 1 || event.importance > 10 {
            return Err(format!(
                "EventProfile[{}]: importance {} out of range [1,10]",
                i, event.importance
            ));
        }
    }
    Ok(())
}

/// LitSeg 叙事事件提取 — LLM 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEventExtraction {
    pub event_type: EventType,
    pub intensity: f32,
    pub sentiment: f32,
    pub description: String,
    pub involved_character_ids: Vec<String>,
    pub conflict_types: Vec<String>,
}

/// Ingest结果
#[derive(Debug, Clone)]
pub struct IngestResult {
    pub analysis: ContentAnalysis,
    pub knowledge: GeneratedKnowledge,
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    /// LitSeg: 叙事事件提取结果
    pub narrative_events: Vec<NarrativeEvent>,
}

/// Ingest批次（用于批量处理）
pub struct IngestBatch {
    pub contents: Vec<IngestContent>,
}

impl IngestBatch {
    pub fn new() -> Self {
        Self { contents: vec![] }
    }

    pub fn add(&mut self, content: IngestContent) {
        self.contents.push(content);
    }

    pub async fn process(
        &self,
        pipeline: &IngestPipeline,
    ) -> Vec<Result<IngestResult, Box<dyn std::error::Error>>> {
        use futures::future::join_all;

        let futures: Vec<_> = self
            .contents
            .iter()
            .map(|content| pipeline.ingest(content))
            .collect();

        join_all(futures).await
    }
}

impl Default for IngestBatch {
    fn default() -> Self {
        Self::new()
    }
}
