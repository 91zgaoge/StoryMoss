//! Novel Bootstrap Workflow - 小说自动初始化工作流 (Genesis Engine v5.0.0)
//!
//! 当用户输入"写一篇XX小说"且无现有故事时，自动完成：
//! 1. 生成故事概念（标题、简介、题材）
//! 2. 生成第一章正文（用户立即可见）
//! 3. 生成世界观设定 + 故事大纲
//! 4. 生成角色、场景、伏笔、知识图谱

use crate::agents::service::{AgentService, AgentTask, AgentType};
use crate::db::{DbPool, CreateStoryRequest, CreateCharacterRequest};
use crate::db::repositories::{StoryRepository, CharacterRepository};
use crate::db::repositories_v3::{WorldBuildingRepository, SceneRepository, StoryOutlineRepository, CharacterRelationshipRepository, KnowledgeGraphRepository};
use crate::db::repositories_v3::SceneUpdate;
use crate::db::models_v3::{WorldRule, RuleType, ConflictType};
use crate::creative_engine::foreshadowing::ForeshadowingTracker;
use crate::llm::{LlmService, service::PipelineContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

/// 小说初始化会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapSession {
    pub id: String,
    pub status: BootstrapStatus,
    pub current_step: String,
    pub steps_completed: usize,
    pub total_steps: usize,
    pub story_id: Option<String>,
    pub error_message: Option<String>,
    /// 生成的小说正文开头内容（直接返回给前端展示）
    pub first_chapter_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapStatus {
    InProgress,
    Completed,
    Failed,
}

/// Bootstrap 进度事件（推送到前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapProgressEvent {
    pub session_id: String,
    pub step_name: String,
    pub step_number: usize,
    pub total_steps: usize,
    pub message: String,
}

/// 小说初始化工作流
pub struct NovelBootstrapWorkflow {
    app_handle: AppHandle,
    llm_service: LlmService,
    pool: DbPool,
}

impl NovelBootstrapWorkflow {
    pub fn new(app_handle: AppHandle) -> Self {
        let pool = app_handle.state::<DbPool>().inner().clone();
        let llm_service = LlmService::new(app_handle.clone());
        Self { app_handle, llm_service, pool }
    }

    /// v5.2.2 重构：两阶段Bootstrap — 先快速返回正文，后台异步完善世界观/角色/场景
    /// 
    /// 即时阶段（2-3分钟）：生成概念 + 第一章正文 → 立即返回，用户可以开始写作
    /// 后台阶段（5-8分钟，异步）：世界观 → 大纲 → 角色 → 场景 → 伏笔 → 知识图谱
    pub async fn run(&self, user_premise: &str) -> Result<BootstrapSession, String> {
        // 阶段1：即时返回
        let (session, story_concept, story_id) = self.run_quick_phase(user_premise).await?;

        // 阶段2：后台异步完成剩余工作
        let app_handle = self.app_handle.clone();
        let session_id = session.id.clone();
        let story_concept_clone = story_concept.clone();
        tauri::async_runtime::spawn(async move {
            Self::run_background_phase(app_handle, session_id, story_id, story_concept_clone).await;
        });

        Ok(session)
    }

    /// 即时阶段：生成故事概念 + 第一章正文，立即返回给用户
    /// 用户只需等待2-3分钟即可看到正文并开始写作
    pub async fn run_quick_phase(&self, user_premise: &str) -> Result<(BootstrapSession, StoryConcept, String), String> {
        let overall_start = std::time::Instant::now();
        let session_id = Uuid::new_v4().to_string();
        let total_steps = 2; // 用户可见只有2步：概念 + 正文

        // 创建持久化会话记录
        self.create_session(&session_id, total_steps).map_err(|e| format!("Failed to create session: {}", e))?;

        // Step 1: 生成故事概念
        let step1_start = std::time::Instant::now();
        self.update_session(&session_id, "concept", 0, None).ok();
        self.emit_progress(&session_id, "构思故事", 1, total_steps, "正在构思故事概念...");
        self.emit_progress(&session_id, "构思故事", 1, total_steps, "正在调用AI生成故事概念...");
        let story_concept = match self.generate_story_concept(user_premise).await {
            Ok(c) => c,
            Err(e) => {
                log::error!("[NovelBootstrapWorkflow] generate_story_concept FAILED after {:?}: {}", step1_start.elapsed(), e);
                self.fail_session(&session_id, &format!("故事概念生成失败: {}", e)).ok();
                return Err(e);
            }
        };
        let step1_elapsed = step1_start.elapsed().as_secs();
        log::info!("[NovelBootstrapWorkflow] generate_story_concept SUCCESS in {}s, title='{}'", step1_elapsed, story_concept.title);
        self.emit_progress(&session_id, "构思故事", 1, total_steps, &format!("故事概念已生成：《{}》（耗时{}秒）", story_concept.title, step1_elapsed));

        // 创建 Story 记录
        self.emit_progress(&session_id, "构思故事", 1, total_steps, "正在保存故事...");
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = match story_repo.create(CreateStoryRequest {
            title: story_concept.title.clone(),
            description: Some(story_concept.description.clone()),
            genre: Some(story_concept.genre.clone()),
            style_dna_id: None,
        }) {
            Ok(s) => s,
            Err(e) => {
                self.fail_session(&session_id, &format!("创建故事失败: {}", e)).ok();
                return Err(e.to_string());
            }
        };
        let story_id = story.id.clone();
        self.update_session(&session_id, "concept", 1, Some(&story_id)).ok();
        self.emit_progress(&session_id, "构思故事", 1, total_steps, &format!("故事《{}》已创建", story.title));

        // Step 2: 生成第一章正文（用户立即可见）
        let step2_start = std::time::Instant::now();
        self.update_session(&session_id, "first_chapter", 1, Some(&story_id)).ok();
        self.emit_progress(&session_id, "撰写开篇", 2, total_steps, "正在准备写作上下文...");
        self.emit_progress(&session_id, "撰写开篇", 2, total_steps, "正在构建写作指令...");
        self.emit_progress(&session_id, "撰写开篇", 2, total_steps, "正在调用AI撰写第一章（1500-2500字，可能需要1-3分钟）...");
        // 传入空的世界观/角色/场景 — StoryContextBuilder 能优雅处理空数据
        let empty_world = WorldBuildingResult { concept: story_concept.description.clone(), rules: vec![] };
        let (first_chapter_content, chapter_id) = match self.generate_first_chapter(&story_id, &session_id, &story_concept, &empty_world, &[], &[]).await {
            Ok(c) => c,
            Err(e) => {
                log::error!("[NovelBootstrapWorkflow] generate_first_chapter FAILED after {:?}: {}", step2_start.elapsed(), e);
                self.fail_session(&session_id, &format!("第一章生成失败: {}", e)).ok();
                return Err(e);
            }
        };
        let step2_elapsed = step2_start.elapsed().as_secs();
        let content_len = first_chapter_content.chars().count();
        log::info!("[NovelBootstrapWorkflow] generate_first_chapter SUCCESS in {}s, {} chars", step2_elapsed, content_len);
        self.emit_progress(&session_id, "撰写开篇", 2, total_steps, &format!("第一章已完成！{}字（耗时{}秒）", content_len, step2_elapsed));
        self.update_session(&session_id, "first_chapter", 2, Some(&story_id)).ok();

        let overall_elapsed = overall_start.elapsed().as_secs();
        log::info!("[NovelBootstrapWorkflow] run_quick_phase TOTAL {}s (concept:{}s + chapter:{}s)", overall_elapsed, step1_elapsed, step2_elapsed);

        // 发送 ChapterSwitch 事件让前端自动切换到新故事（用户可以立刻开始写作）
        let _ = crate::window::WindowManager::send_to_frontstage(
            &self.app_handle,
            crate::window::FrontstageEvent::ChapterSwitch {
                story_id: story_id.clone(),
                chapter_id: chapter_id.clone(),
                title: "第一章".to_string(),
            }
        );
        // 同时发送 DataRefresh 事件让前端刷新故事列表
        let _ = crate::window::WindowManager::send_to_frontstage(
            &self.app_handle,
            crate::window::FrontstageEvent::DataRefresh { entity: "stories".to_string() }
        );

        let session = BootstrapSession {
            id: session_id,
            status: BootstrapStatus::Completed,
            current_step: "first_chapter_ready".to_string(),
            steps_completed: 2,
            total_steps: 2,
            story_id: Some(story_id.clone()),
            error_message: None,
            first_chapter_content: Some(first_chapter_content),
        };

        Ok((session, story_concept, story_id))
    }

    /// 后台阶段：异步完成世界观/大纲/角色/场景/伏笔/知识图谱
    /// 此方法应在 tokio::spawn 中调用，不阻塞前端响应
    pub async fn run_background_phase(
        app_handle: AppHandle,
        session_id: String,
        story_id: String,
        story_concept: StoryConcept,
    ) {
        let workflow = NovelBootstrapWorkflow::new(app_handle.clone());
        let total_steps = 6; // 后台6步

        log::info!(
            "[NovelBootstrapWorkflow] 后台阶段开始 story_id={} session_id={}",
            story_id, session_id
        );

        // 通知前端：后台完善已开始
        workflow.emit_progress(&session_id, "后台完善", 0, total_steps, "后台正在完善小说世界，您可以先开始写作...");

        // Step 3: 生成世界观
        workflow.emit_progress(&session_id, "后台完善", 1, total_steps, "正在生成世界观设定...");
        let world = match workflow.generate_world_building(&story_id, &session_id, &story_concept).await {
            Ok(w) => w,
            Err(e) => {
                log::warn!("[NovelBootstrapWorkflow] 世界观生成失败 for story {}: {}", story_id, e);
                let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                    "step": "world_building", "story_id": story_id, "error": e
                }));
                WorldBuildingResult { concept: story_concept.description.clone(), rules: vec![] }
            }
        };
        workflow.emit_progress(&session_id, "后台完善", 1, total_steps, "世界观设定已生成");

        // Step 4: 生成故事大纲
        workflow.emit_progress(&session_id, "后台完善", 2, total_steps, "正在生成故事大纲（三幕结构）...");
        let outline = match workflow.generate_story_outline(&story_id, &session_id, &story_concept).await {
            Ok(o) => o,
            Err(e) => {
                log::warn!("[NovelBootstrapWorkflow] 故事大纲生成失败 for story {}: {}", story_id, e);
                let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                    "step": "story_outline", "story_id": story_id, "error": e
                }));
                StoryOutlineData { acts: vec![] }
            }
        };
        workflow.emit_progress(&session_id, "后台完善", 2, total_steps, "故事大纲已生成");

        // Step 5: 生成角色
        workflow.emit_progress(&session_id, "后台完善", 3, total_steps, "正在生成角色（3-5个主要角色）...");
        let characters = match workflow.generate_characters(&story_id, &session_id, &story_concept, &world).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[NovelBootstrapWorkflow] 角色生成失败 for story {}: {}", story_id, e);
                let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                    "step": "characters", "story_id": story_id, "error": e
                }));
                vec![]
            }
        };
        workflow.emit_progress(&session_id, "后台完善", 3, total_steps, &format!("已生成 {} 个角色", characters.len()));

        // Step 6: 生成场景
        workflow.emit_progress(&session_id, "后台完善", 4, total_steps, "正在生成场景大纲（8-12个核心场景）...");
        let scenes = match workflow.generate_scene_outline(&story_id, &session_id, &story_concept, &characters).await {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[NovelBootstrapWorkflow] 场景大纲生成失败 for story {}: {}", story_id, e);
                let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                    "step": "scenes", "story_id": story_id, "error": e
                }));
                vec![]
            }
        };
        workflow.emit_progress(&session_id, "后台完善", 4, total_steps, &format!("已生成 {} 个场景", scenes.len()));

        // 获取第一个场景ID用于伏笔关联
        let first_scene_id = scenes.first().map(|s| s.id.as_str());

        // Step 7: 埋设伏笔
        workflow.emit_progress(&session_id, "后台完善", 5, total_steps, "正在埋设伏笔（3-5处核心伏笔）...");
        let foreshadowings = match workflow.generate_foreshadowing(&story_id, &session_id, &story_concept, &outline, first_scene_id).await {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[NovelBootstrapWorkflow] 伏笔生成失败 for story {}: {}", story_id, e);
                let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                    "step": "foreshadowing", "story_id": story_id, "error": e
                }));
                vec![]
            }
        };
        workflow.emit_progress(&session_id, "后台完善", 5, total_steps, &format!("已埋设 {} 处伏笔", foreshadowings.len()));

        // Step 8: 构建知识图谱
        workflow.emit_progress(&session_id, "后台完善", 6, total_steps, "正在构建知识图谱...");
        if let Err(e) = workflow.create_genesis_knowledge_graph(&story_id, &characters, &scenes, &foreshadowings).await {
            log::warn!("[NovelBootstrapWorkflow] 知识图谱构建失败 for story {}: {}", story_id, e);
            let _ = app_handle.emit("novel-bootstrap-error", serde_json::json!({
                "step": "knowledge_graph", "story_id": story_id, "error": e
            }));
        } else {
            workflow.emit_progress(&session_id, "后台完善", 6, total_steps, "知识图谱已构建");
        }

        workflow.complete_session(&session_id, &story_id).ok();
        workflow.emit_progress(&session_id, "后台完善", 6, total_steps, "创世完成！所有卡片已生成");

        log::info!(
            "[NovelBootstrapWorkflow] 后台阶段完成 story_id={} session_id={}",
            story_id, session_id
        );

        // 通知前端刷新所有数据（让幕后世界观/角色/场景/大纲/伏笔卡片自动出现）
        let _ = crate::window::WindowManager::send_to_frontstage(
            &app_handle,
            crate::window::FrontstageEvent::DataRefresh { entity: "all".to_string() }
        );
        let _ = crate::window::WindowManager::send_to_backstage(
            &app_handle,
            crate::window::BackstageEvent::DataRefresh { entity: "world_building".to_string() }
        );
        // 自动导航到幕后 Stories 页面并高亮新故事
        let _ = crate::window::WindowManager::send_to_backstage(
            &app_handle,
            crate::window::BackstageEvent::NavigateTo {
                view: "stories".to_string(),
                highlight_story_id: Some(story_id.clone()),
                open_panel: Some("overview".to_string()),
            }
        );
        // Bootstrap 全部完成后再次发送 ChapterSwitch 到幕前，确保自动加载
        let _ = crate::window::WindowManager::send_to_frontstage(
            &app_handle,
            crate::window::FrontstageEvent::ChapterSwitch {
                story_id: story_id.clone(),
                chapter_id: String::new(), // 后台阶段不知道chapter_id，由前端自己处理
                title: "第一章".to_string(),
            }
        );
    }

    // ==================== Step 1: 故事概念 ====================

    async fn generate_story_concept(&self, user_premise: &str) -> Result<StoryConcept, String> {
        let prompt = format!(
            r#"你是一位资深小说编辑。请根据用户的创意，生成一个完整的故事概念。

用户输入："{}"

请用 JSON 格式回复：
{{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）"
}}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材要具体，不要笼统"小说"
4. 只输出 JSON，不要其他内容"#,
            user_premise.replace('"', "'")
        );

        // 故事概念JSON比较短，512 tokens足够，减少等待时间
        let ctx = PipelineContext {
            step_name: "构思故事".to_string(),
            step_number: 1,
            total_steps: 2,
            action: "生成故事概念".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(512), Some(0.7), Some("生成故事概念"), Some(ctx)).await?;
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let concept: StoryConcept = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse story concept: {}. JSON: {}", e, json_str))?;

        Ok(concept)
    }

    // ==================== Step 2: 世界观 ====================

    async fn generate_world_building(&self, story_id: &str, session_id: &str, concept: &StoryConcept) -> Result<WorldBuildingResult, String> {
        self.emit_progress(session_id, "构建世界", 3, 4, "正在构建世界观核心概念...");
        let prompt = format!(
            r#"你是一位世界观架构师。请为以下故事构建完整的世界观设定。

故事：《{}》
题材：{}
简介：{}

请用 JSON 格式回复：
{{
  "concept": "世界观核心概念（50-100字）",
  "rules": [
    {{"name": "规则名称", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}}
  ],
  "history": "世界历史背景（200-300字）",
  "key_locations": ["关键地点1", "关键地点2"],
  "power_system": "力量体系概述（如有）"
}}

要求：
1. 规则要有创意，避免陈词滥调
2. 规则之间要有逻辑一致性
3. 重要规则（importance >= 8）不超过5条
4. 只输出 JSON，不要其他内容"#,
            concept.title, concept.genre, concept.description
        );

        self.emit_progress(session_id, "构建世界", 3, 4, "正在调用AI生成世界观设定...");
        let ctx = PipelineContext {
            step_name: "构建世界".to_string(),
            step_number: 1,
            total_steps: 6,
            action: "生成世界观设定".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(2048), Some(0.6), Some("生成世界观设定"), Some(ctx)).await?;
        self.emit_progress(session_id, "构建世界", 3, 4, "AI世界观设定已生成，正在解析...");
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let wb_data: WorldBuildingData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse world building: {}. JSON: {}", e, json_str))?;

        // 存入数据库 —— WorldBuildingRepository::create 接收 (story_id, concept)
        let repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = repo.create(story_id, &wb_data.concept).map_err(|e| e.to_string())?;

        // 更新世界观规则和历史文化
        let rules: Vec<WorldRule> = wb_data.rules.iter().map(|r| WorldRule {
            id: Uuid::new_v4().to_string(),
            name: r.name.clone(),
            description: Some(r.description.clone()),
            rule_type: match r.rule_type.as_str() {
                "physical" => RuleType::Physical,
                "magic" => RuleType::Magic,
                "social" => RuleType::Social,
                "historical" => RuleType::Historical,
                "technology" => RuleType::Technology,
                "biological" => RuleType::Biological,
                "cultural" => RuleType::Cultural,
                _ => RuleType::Custom,
            },
            importance: r.importance,
        }).collect();

        let _ = repo.update(
            &world_building.id,
            None,
            Some(&rules),
            Some(&wb_data.history),
            None,
        );

        Ok(WorldBuildingResult {
            concept: wb_data.concept,
            rules: wb_data.rules,
        })
    }

    // ==================== Step 3: 故事大纲 ====================

    async fn generate_story_outline(&self, story_id: &str, session_id: &str, concept: &StoryConcept) -> Result<StoryOutlineData, String> {
        let prompt = format!(
            r#"你是一位资深故事架构师。请为以下故事设计一个完整的三幕式大纲。

故事：《{}》
题材：{}
简介：{}

请用 JSON 格式回复：
{{
  "acts": [
    {{
      "act_number": 1,
      "title": "第一幕标题",
      "summary": "本幕核心内容摘要（100字）",
      "key_plot_points": ["情节点1", "情节点2", "情节点3"],
      "estimated_scenes": 4
    }}
  ]
}}

要求：
1. 严格三幕结构（起-承-转-合）
2. 每幕包含3-5个关键情节点
3. 场景数量要合理（第一幕3-5场，第二幕6-10场，第三幕3-5场）
4. 只输出 JSON，不要其他内容"#,
            concept.title, concept.genre, concept.description
        );

        self.emit_progress(session_id, "构建世界", 3, 4, "正在调用AI设计故事大纲...");
        let ctx = PipelineContext {
            step_name: "故事大纲".to_string(),
            step_number: 2,
            total_steps: 6,
            action: "生成故事大纲".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(2048), Some(0.6), Some("生成故事大纲"), Some(ctx)).await?;
        self.emit_progress(session_id, "构建世界", 3, 4, "故事大纲已生成，正在解析...");
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let outline_data: StoryOutlineData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse story outline: {}. JSON: {}", e, json_str))?;

        // 保存到数据库
        let repo = StoryOutlineRepository::new(self.pool.clone());
        let total_scenes: i32 = outline_data.acts.iter().map(|a| a.estimated_scenes).sum();
        let structure_json = serde_json::to_string(&outline_data.acts)
            .map_err(|e| format!("Failed to serialize outline: {}", e))?;
        let content_summary = outline_data.acts.iter()
            .map(|a| format!("第{}幕 {}：{}", a.act_number, a.title, a.summary))
            .collect::<Vec<_>>()
            .join("\n\n");

        let _outline = repo.create(
            story_id,
            &content_summary,
            Some(&structure_json),
            outline_data.acts.len() as i32,
            Some(total_scenes),
        ).map_err(|e| e.to_string())?;

        Ok(outline_data)
    }

    // ==================== Step 4: 角色 ====================

    async fn generate_characters(&self, story_id: &str, session_id: &str, concept: &StoryConcept, world: &WorldBuildingResult) -> Result<Vec<GeneratedCharacter>, String> {
        let prompt = format!(
            r#"你是一位角色设计师。请为以下故事设计 3-5 个主要角色。

故事：《{}》
题材：{}
简介：{}
世界观：{}
核心规则：{}

请用 JSON 格式回复：
{{
  "characters": [
    {{
      "name": "角色姓名",
      "role": "角色定位（主角/反派/导师/盟友/爱情线）",
      "personality": "性格特征（50字）",
      "background": "背景故事（100字）",
      "goals": "核心目标",
      "fears": "深层恐惧",
      "appearance": "外貌特征（50字）",
      "gender": "男/女/其他",
      "age": 25,
      "relationships": [{{"target": "另一个角色名", "nature": "关系性质"}}]
    }}
  ]
}}

要求：
1. 主角要有鲜明的性格弧光空间
2. 角色之间要有冲突和张力
3. 避免刻板印象
4. 只输出 JSON，不要其他内容"#,
            concept.title, concept.genre, concept.description,
            world.concept,
            world.rules.iter().map(|r| format!("{}: {}", r.name, r.description)).collect::<Vec<_>>().join("; ")
        );

        self.emit_progress(session_id, "塑造世界", 4, 4, "正在调用AI设计角色...");
        let ctx = PipelineContext {
            step_name: "塑造角色".to_string(),
            step_number: 3,
            total_steps: 6,
            action: "生成角色".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(3000), Some(0.7), Some("生成角色"), Some(ctx)).await?;
        self.emit_progress(session_id, "塑造世界", 4, 4, "角色设计已生成，正在解析...");
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let char_data: CharacterData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse characters: {}. JSON: {}", e, json_str))?;

        // 存入数据库
        let repo = CharacterRepository::new(self.pool.clone());
        let rel_repo = CharacterRelationshipRepository::new(self.pool.clone());
        let mut generated = Vec::new();
        let mut name_to_id: HashMap<String, String> = HashMap::new();

        for c in &char_data.characters {
            let background = format!("{}", c.background);
            let character = repo.create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: c.name.clone(),
                background: Some(background),
                personality: Some(c.personality.clone()),
                goals: Some(c.goals.clone()),
                appearance: Some(c.appearance.clone()),
                gender: Some(c.gender.clone()),
                age: Some(c.age),
            }).map_err(|e| e.to_string())?;

            name_to_id.insert(c.name.clone(), character.id.clone());

            generated.push(GeneratedCharacter {
                id: character.id,
                name: c.name.clone(),
                role: c.role.clone(),
                personality: c.personality.clone(),
                background: c.background.clone(),
                goals: c.goals.clone(),
                fears: c.fears.clone(),
                appearance: c.appearance.clone(),
                gender: c.gender.clone(),
                age: c.age,
            });
        }

        // 创建角色关系
        for c in &char_data.characters {
            if let Some(source_id) = name_to_id.get(&c.name) {
                for rel in &c.relationships {
                    if let Some(target_id) = name_to_id.get(&rel.target) {
                        let _ = rel_repo.create(
                            story_id,
                            source_id,
                            target_id,
                            &rel.nature,
                            None,
                            None,
                        );
                    }
                }
            }
        }

        Ok(generated)
    }

    // ==================== Step 5: 场景大纲 ====================

    async fn generate_scene_outline(&self, story_id: &str, session_id: &str, concept: &StoryConcept, characters: &[GeneratedCharacter]) -> Result<Vec<GeneratedScene>, String> {
        let character_names = characters.iter().map(|c| format!("{}({})", c.name, c.role)).collect::<Vec<_>>().join(", ");

        let prompt = format!(
            r#"你是一位大纲规划师。请为以下故事设计 8-12 个核心场景。

故事：《{}》
题材：{}
简介：{}
角色：{}

请用 JSON 格式回复：
{{
  "scenes": [
    {{
      "sequence_number": 1,
      "title": "场景标题",
      "dramatic_goal": "本场景的戏剧目标（角色想达成什么）",
      "external_pressure": "外部压力/阻碍",
      "conflict_type": "man_vs_man|man_vs_self|man_vs_society|man_vs_nature|man_vs_technology|man_vs_fate|man_vs_supernatural|man_vs_time|man_vs_morality|man_vs_identity|faction_vs_faction",
      "setting_location": "地点",
      "setting_time": "时间",
      "characters_present": ["角色名1", "角色名2"],
      "summary": "场景内容摘要（100字）"
    }}
  ]
}}

要求：
1. 场景之间要有因果关系
2. 每个场景都要推动情节或揭示人物
3. 冲突类型要多样
4. 只输出 JSON，不要其他内容"#,
            concept.title, concept.genre, concept.description, character_names
        );

        self.emit_progress(session_id, "塑造世界", 4, 4, "正在调用AI设计场景...");
        let ctx = PipelineContext {
            step_name: "场景规划".to_string(),
            step_number: 4,
            total_steps: 6,
            action: "生成场景大纲".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(3000), Some(0.6), Some("生成场景大纲"), Some(ctx)).await?;
        self.emit_progress(session_id, "塑造世界", 4, 4, "场景设计已生成，正在解析...");
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let scene_data: SceneData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse scenes: {}. JSON: {}", e, json_str))?;

        // 存入数据库 —— SceneRepository::create(story_id, sequence_number, title)
        let repo = SceneRepository::new(self.pool.clone());
        let mut generated = Vec::new();
        for s in scene_data.scenes {
            let scene = repo.create(
                story_id,
                s.sequence_number,
                Some(&s.title),
            ).map_err(|e| e.to_string())?;

            // 使用 SceneUpdate 更新额外字段
            let updates = SceneUpdate {
                title: Some(s.title.clone()),
                dramatic_goal: Some(s.dramatic_goal.clone()),
                external_pressure: Some(s.external_pressure.clone()),
                conflict_type: Some(match s.conflict_type.as_str() {
                    "man_vs_man" => ConflictType::ManVsMan,
                    "man_vs_self" => ConflictType::ManVsSelf,
                    "man_vs_society" => ConflictType::ManVsSociety,
                    "man_vs_nature" => ConflictType::ManVsNature,
                    "man_vs_technology" => ConflictType::ManVsTechnology,
                    "man_vs_fate" => ConflictType::ManVsFate,
                    "man_vs_supernatural" => ConflictType::ManVsSupernatural,
                    "man_vs_time" => ConflictType::ManVsTime,
                    "man_vs_morality" => ConflictType::ManVsMorality,
                    "man_vs_identity" => ConflictType::ManVsIdentity,
                    "faction_vs_faction" => ConflictType::FactionVsFaction,
                    _ => ConflictType::ManVsMan,
                }),
                characters_present: Some(s.characters_present.clone()),
                character_conflicts: None,
                setting_location: Some(s.setting_location.clone()),
                setting_time: Some(s.setting_time.clone()),
                setting_atmosphere: None,
                content: None,
                previous_scene_id: None,
                next_scene_id: None,
                confidence_score: Some(0.8),
                execution_stage: Some("planning".to_string()),
                outline_content: Some(s.summary.clone()),
                draft_content: None,
                style_blend_override: None,
                foreshadowing_ids: None,
            };
            let _ = repo.update(&scene.id, &updates);

            generated.push(GeneratedScene {
                id: scene.id.clone(),
                sequence_number: s.sequence_number,
                title: s.title,
                summary: s.summary,
            });
        }

        Ok(generated)
    }

    // ==================== Step 6: 第一章 ====================

    async fn generate_first_chapter(&self, story_id: &str, session_id: &str, concept: &StoryConcept, world: &WorldBuildingResult, characters: &[GeneratedCharacter], scenes: &[GeneratedScene]) -> Result<(String, String), String> {
        self.emit_progress(session_id, "撰写开篇", 2, 4, "正在加载故事上下文和角色信息...");
        // 构建完整的 AgentContext
        let builder = crate::creative_engine::context_builder::StoryContextBuilder::new(self.pool.clone());
        let agent_context = builder.build(story_id, Some(1), None, None)?;

        // 构建丰富的写作指令 — 即使世界观/角色尚未生成，也注入故事概念的全部信息
        let character_info = if characters.is_empty() {
            format!("【待生成】题材为{}，请根据题材和简介创造合适的主角", concept.genre)
        } else {
            characters.iter().map(|c| format!("{}({}): {}", c.name, c.role, c.personality)).collect::<Vec<_>>().join("; ")
        };
        let scene_info = if scenes.is_empty() {
            "【待生成】请根据故事概念自行设计开篇场景".to_string()
        } else {
            scenes.first().map(|s| format!("{} - {}", s.title, s.summary)).unwrap_or_default()
        };

        let service = AgentService::new(self.app_handle.clone());
        let task = AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: AgentType::Writer,
            context: agent_context,
            input: format!(
                "请撰写《{}》的第一章开头（1500-2500字）。\n\n【故事概念】\n题材：{}\n基调：{}\n节奏：{}\n简介：{}\n主题：{}\n预计篇幅：{}\n\n这是故事的开篇，需要：\n1. 迅速建立世界观和氛围（紧扣题材和基调）\n2. 引入主角，展示其性格和目标\n3. 埋下至少一个伏笔\n4. 在第一幕结尾制造一个冲突或悬念\n\n世界观核心：{}\n核心角色：{}\n第一章场景：{}",
                concept.title,
                concept.genre,
                concept.tone,
                concept.pacing,
                concept.description,
                concept.themes.join(", "),
                concept.target_length,
                world.concept,
                character_info,
                scene_info
            ),
            parameters: HashMap::new(),
            tier: None,
        };

        self.emit_progress(session_id, "撰写开篇", 2, 4, "AI写作完成，正在保存章节...");
        let result = service.execute_task(task).await?;
        self.emit_progress(session_id, "撰写开篇", 2, 4, "章节内容已生成，正在存入数据库...");

        // 保存到第一个场景的 content 字段
        if let Some(first_scene) = scenes.first() {
            let scene_repo = SceneRepository::new(self.pool.clone());
            let updates = SceneUpdate {
                title: None,
                dramatic_goal: None,
                external_pressure: None,
                conflict_type: None,
                characters_present: None,
                character_conflicts: None,
                setting_location: None,
                setting_time: None,
                setting_atmosphere: None,
                content: Some(result.content.clone()),
                previous_scene_id: None,
                next_scene_id: None,
                confidence_score: None,
                execution_stage: None,
                outline_content: None,
                draft_content: None,
                style_blend_override: None,
                foreshadowing_ids: None,
            };
            let _ = scene_repo.update(&first_scene.id, &updates);
        }

        // 同时创建 Chapter 记录，确保前端可以加载
        let chapter_repo = crate::db::repositories::ChapterRepository::new(self.pool.clone());
        let chapter = chapter_repo.create(crate::db::CreateChapterRequest {
            story_id: story_id.to_string(),
            chapter_number: 1,
            title: Some("第一章".to_string()),
            outline: None,
            content: Some(result.content.clone()),
        }).map_err(|e| e.to_string())?;

        Ok((result.content, chapter.id))
    }

    // ==================== Step 7: 伏笔 ====================

    async fn generate_foreshadowing(
        &self,
        story_id: &str,
        _session_id: &str,
        concept: &StoryConcept,
        outline: &StoryOutlineData,
        first_scene_id: Option<&str>,
    ) -> Result<Vec<GeneratedForeshadowing>, String> {
        let outline_summary = if outline.acts.is_empty() {
            "暂无大纲".to_string()
        } else {
            outline.acts.iter()
                .map(|a| format!("第{}幕 {}：{}", a.act_number, a.title, a.summary))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt = format!(
            r#"你是一位资深编剧。请根据以下故事概念和大纲，设计3-5个核心伏笔。

故事：《{}》
题材：{}
简介：{}

故事大纲：
{}

请用 JSON 格式回复：
{{
  "foreshadowings": [
    {{
      "content": "伏笔内容描述",
      "importance": 8,
      "target_act": 2,
      "hint_style": "暗示风格（如：环境隐喻、对话暗示、物品象征、预言梦境）"
    }}
  ]
}}

要求：
1. 伏笔要贯穿多个幕次，具有回收价值
2. importance 1-10，核心伏笔不低于7
3. hint_style 要多样化
4. 第一个伏笔建议在第一章（第一幕）就埋下
5. 只输出 JSON，不要其他内容"#,
            concept.title, concept.genre, concept.description, outline_summary
        );

        let ctx = PipelineContext {
            step_name: "埋设伏笔".to_string(),
            step_number: 5,
            total_steps: 6,
            action: "生成伏笔".to_string(),
        };
        let response = self.llm_service.generate_with_context_and_pipeline(prompt, Some(1024), Some(0.7), Some("生成伏笔"), Some(ctx)).await?;
        let content = response.content.trim();
        let json_str = Self::extract_json(content)?;
        let fw_data: ForeshadowingData = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse foreshadowings: {}. JSON: {}", e, json_str))?;

        let tracker = ForeshadowingTracker::new(self.pool.clone());
        let mut generated = Vec::new();

        for (idx, fw) in fw_data.foreshadowings.into_iter().enumerate() {
            let setup_scene = if idx == 0 { first_scene_id } else { None };
            let _id = tracker.add_foreshadowing(
                story_id,
                &fw.content,
                setup_scene,
                fw.importance,
            ).map_err(|e| format!("保存伏笔失败: {}", e))?;

            generated.push(fw);
        }

        Ok(generated)
    }

    // ==================== Step 8: 知识图谱 ====================

    async fn create_genesis_knowledge_graph(
        &self,
        story_id: &str,
        characters: &[GeneratedCharacter],
        scenes: &[GeneratedScene],
        foreshadowings: &[GeneratedForeshadowing],
    ) -> Result<(), String> {
        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
        let mut entity_id_map: HashMap<String, String> = HashMap::new();

        // 创建角色实体
        for c in characters {
            let attrs = serde_json::json!({
                "role": c.role,
                "personality": c.personality,
            });
            let entity = kg_repo.create_entity(
                story_id,
                &c.name,
                "Character",
                &attrs,
                None,
            ).map_err(|e| e.to_string())?;
            entity_id_map.insert(format!("char:{}", c.id), entity.id);
        }

        // 创建场景实体
        for s in scenes {
            let attrs = serde_json::json!({
                "sequence_number": s.sequence_number,
                "summary": s.summary,
            });
            let entity = kg_repo.create_entity(
                story_id,
                &s.title,
                "Event",
                &attrs,
                None,
            ).map_err(|e| e.to_string())?;
            entity_id_map.insert(format!("scene:{}", s.id), entity.id);
        }

        // 创建伏笔实体
        for (idx, f) in foreshadowings.iter().enumerate() {
            let attrs = serde_json::json!({
                "importance": f.importance,
                "target_act": f.target_act,
                "hint_style": f.hint_style,
            });
            let entity = kg_repo.create_entity(
                story_id,
                &format!("伏笔{}", idx + 1),
                "PlotDevice",
                &attrs,
                None,
            ).map_err(|e| e.to_string())?;
            entity_id_map.insert(format!("fw:{}", idx), entity.id);
        }

        // 创建关系：角色 -> 场景 (participates_in)
        for c in characters {
            for s in scenes {
                let scene_text = format!("{} {}", s.title, s.summary);
                if scene_text.contains(&c.name) {
                    if let (Some(char_entity), Some(scene_entity)) = (
                        entity_id_map.get(&format!("char:{}", c.id)),
                        entity_id_map.get(&format!("scene:{}", s.id)),
                    ) {
                        let _ = kg_repo.create_relation(
                            story_id,
                            char_entity,
                            scene_entity,
                            "ParticipatesIn",
                            0.7,
                        );
                    }
                }
            }
        }

        // 创建关系：伏笔 -> 第一个场景 (set_up_in)
        if let Some(first_scene) = scenes.first() {
            if let Some(scene_entity) = entity_id_map.get(&format!("scene:{}", first_scene.id)) {
                for idx in 0..foreshadowings.len() {
                    if let Some(fw_entity) = entity_id_map.get(&format!("fw:{}", idx)) {
                        let _ = kg_repo.create_relation(
                            story_id,
                            fw_entity,
                            scene_entity,
                            "SetUpIn",
                            0.9,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    // ==================== 辅助方法 ====================

    fn emit_progress(&self, session_id: &str, step_name: &str, step_number: usize, total_steps: usize, message: &str) {
        let event = BootstrapProgressEvent {
            session_id: session_id.to_string(),
            step_name: step_name.to_string(),
            step_number,
            total_steps,
            message: message.to_string(),
        };
        let _ = self.app_handle.emit("novel-bootstrap-progress", event);
    }

    // ==================== 会话持久化 ====================

    fn create_session(&self, session_id: &str, total_steps: usize) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO novel_bootstrap_sessions (id, status, current_step, steps_completed, total_steps, created_at)
             VALUES (?1, 'in_progress', 'concept', 0, ?2, ?3)",
            rusqlite::params![session_id, total_steps as i32, chrono::Local::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn update_session(&self, session_id: &str, current_step: &str, steps_completed: usize, story_id: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE novel_bootstrap_sessions
             SET current_step = ?2, steps_completed = ?3, story_id = ?4
             WHERE id = ?1",
            rusqlite::params![session_id, current_step, steps_completed as i32, story_id],
        )?;
        Ok(())
    }

    fn complete_session(&self, session_id: &str, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE novel_bootstrap_sessions
             SET status = 'completed', current_step = 'completed', steps_completed = total_steps,
                 story_id = ?2, completed_at = ?3
             WHERE id = ?1",
            rusqlite::params![session_id, story_id, chrono::Local::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn fail_session(&self, session_id: &str, error_message: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE novel_bootstrap_sessions
             SET status = 'failed', error_message = ?2
             WHERE id = ?1",
            rusqlite::params![session_id, error_message],
        )?;
        Ok(())
    }

    fn extract_json(content: &str) -> Result<&str, String> {
        if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            Ok(&content[start..=end])
        } else {
            Err("No JSON object found in response".to_string())
        }
    }
}

// ==================== 数据结构 ====================

#[derive(Debug, Clone, Deserialize)]
pub struct StoryConcept {
    title: String,
    description: String,
    genre: String,
    tone: String,
    pacing: String,
    #[serde(default)]
    themes: Vec<String>,
    #[serde(default)]
    target_length: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WorldBuildingData {
    concept: String,
    rules: Vec<WorldRuleData>,
    history: String,
    #[serde(default)]
    key_locations: Vec<String>,
    #[serde(default)]
    power_system: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WorldRuleData {
    name: String,
    description: String,
    rule_type: String,
    importance: i32,
}

#[derive(Debug, Clone)]
struct WorldBuildingResult {
    concept: String,
    rules: Vec<WorldRuleData>,
}

#[derive(Debug, Clone, Deserialize)]
struct CharacterData {
    characters: Vec<CharacterDetail>,
}

#[derive(Debug, Clone, Deserialize)]
struct CharacterDetail {
    name: String,
    role: String,
    personality: String,
    background: String,
    goals: String,
    fears: String,
    appearance: String,
    gender: String,
    age: i32,
    #[serde(default)]
    relationships: Vec<RelationshipData>,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationshipData {
    target: String,
    nature: String,
}

#[derive(Debug, Clone)]
struct GeneratedCharacter {
    id: String,
    name: String,
    role: String,
    personality: String,
    background: String,
    goals: String,
    fears: String,
    appearance: String,
    gender: String,
    age: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneData {
    scenes: Vec<SceneDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SceneDetail {
    sequence_number: i32,
    title: String,
    dramatic_goal: String,
    external_pressure: String,
    conflict_type: String,
    setting_location: String,
    setting_time: String,
    characters_present: Vec<String>,
    summary: String,
}

#[derive(Debug, Clone)]
struct GeneratedScene {
    id: String,
    sequence_number: i32,
    title: String,
    summary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StoryOutlineData {
    acts: Vec<StoryOutlineAct>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoryOutlineAct {
    act_number: i32,
    title: String,
    summary: String,
    key_plot_points: Vec<String>,
    estimated_scenes: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct ForeshadowingData {
    foreshadowings: Vec<GeneratedForeshadowing>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeneratedForeshadowing {
    content: String,
    importance: i32,
    target_act: i32,
    hint_style: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"title": "测试", "genre": "科幻"}"#;
        let result = NovelBootstrapWorkflow::extract_json(input).unwrap();
        assert_eq!(result, r#"{"title": "测试", "genre": "科幻"}"#);
    }

    #[test]
    fn test_extract_json_with_markdown() {
        let input = "这里有一些解释\n```json\n{\"title\": \"测试\"}\n```\n更多解释";
        let result = NovelBootstrapWorkflow::extract_json(input).unwrap();
        assert_eq!(result, r#"{"title": "测试"}"#);
    }

    #[test]
    fn test_extract_json_with_prefix_text() {
        let input = "好的，这是你的JSON:\n{\"name\": \"value\"}\n希望这有帮助";
        let result = NovelBootstrapWorkflow::extract_json(input).unwrap();
        assert_eq!(result, r#"{"name": "value"}"#);
    }

    #[test]
    fn test_extract_json_no_json() {
        let input = "这里没有JSON对象";
        assert!(NovelBootstrapWorkflow::extract_json(input).is_err());
    }

    #[test]
    fn test_extract_json_nested() {
        let input = r#"{"outer": {"inner": "value"}}"#;
        let result = NovelBootstrapWorkflow::extract_json(input).unwrap();
        assert_eq!(result, r#"{"outer": {"inner": "value"}}"#);
    }

    #[test]
    fn test_story_concept_deserialization() {
        let json = r#"{
            "title": "都市仙尊",
            "description": "一个现代都市中的修仙故事",
            "genre": "都市玄幻",
            "tone": "热血",
            "pacing": "快节奏",
            "themes": ["复仇", "成长"],
            "target_length": "长篇100万字"
        }"#;
        let concept: StoryConcept = serde_json::from_str(json).unwrap();
        assert_eq!(concept.title, "都市仙尊");
        assert_eq!(concept.genre, "都市玄幻");
        assert_eq!(concept.themes.len(), 2);
    }

    #[test]
    fn test_story_concept_deserialization_defaults() {
        let json = r#"{"title": "极简", "description": "测试", "genre": "测试", "tone": "轻松", "pacing": "慢热"}"#;
        let concept: StoryConcept = serde_json::from_str(json).unwrap();
        assert!(concept.themes.is_empty());
        assert!(concept.target_length.is_empty());
    }
}
