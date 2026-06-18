#![allow(dead_code)]
//! Agent Service - 智能代理服务
//!
//! 协调多个Agent完成复杂的创作任务
//! 支持任务分解、执行、结果整合
#![allow(unused_imports)]

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::OnceCell;

use super::{Agent, AgentContext, AgentResult};
use crate::{
    config::settings::AppConfig,
    error::AppError,
    events::{emit_generation_status, GenerationPhase},
    llm::service::LlmService,
    router::{Complexity, Priority, RoutingConstraint, RoutingRequest, TaskType},
    subscription::{SubscriptionService, SubscriptionTier},
};

// v0.9.7: Writer 提示词构建阶段的全局只读配置缓存，避免每次 build_writer_prompt
// 都重新从磁盘/数据库加载 AppConfig、GenreProfile、StyleDNA。
static WRITER_APP_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static WRITER_APP_CONFIG: OnceCell<Arc<AppConfig>> = OnceCell::const_new();
static WRITER_GENRE_PROFILES: OnceCell<Arc<HashMap<String, crate::db::GenreProfile>>> =
    OnceCell::const_new();
static WRITER_STYLE_DNAS: OnceCell<Arc<HashMap<String, crate::db::StyleDNA>>> =
    OnceCell::const_new();

fn writer_app_dir(app_handle: &AppHandle) -> &'static PathBuf {
    WRITER_APP_DIR.get_or_init(|| {
        app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
    })
}

async fn writer_app_config(app_handle: &AppHandle) -> Result<&'static Arc<AppConfig>, AppError> {
    WRITER_APP_CONFIG
        .get_or_try_init(|| async {
            let app_dir = writer_app_dir(app_handle).clone();
            let cfg = tokio::task::spawn_blocking(move || {
                AppConfig::load(&app_dir).map_err(|e| AppError::internal(e.to_string()))
            })
            .await
            .map_err(|e| AppError::internal(format!("AppConfig 加载任务失败: {}", e)))??;
            Ok::<_, AppError>(Arc::new(cfg))
        })
        .await
}

async fn writer_genre_profiles(
    app_handle: &AppHandle,
) -> Result<&'static Arc<HashMap<String, crate::db::GenreProfile>>, AppError> {
    WRITER_GENRE_PROFILES
        .get_or_try_init(|| async {
            let pool = app_handle.state::<crate::db::DbPool>().inner().clone();
            let profiles = tokio::task::spawn_blocking(move || {
                let repo = crate::db::GenreProfileRepository::new(pool);
                repo.get_all()
                    .map(|profiles| {
                        profiles
                            .into_iter()
                            .map(|p| (p.id.clone(), p))
                            .collect::<HashMap<String, crate::db::GenreProfile>>()
                    })
                    .map_err(|e| AppError::internal(format!("体裁画像加载失败: {}", e)))
            })
            .await
            .map_err(|e| AppError::internal(format!("GenreProfile 加载任务失败: {}", e)))??;
            Ok::<_, AppError>(Arc::new(profiles))
        })
        .await
}

async fn writer_style_dnas(
    app_handle: &AppHandle,
) -> Result<&'static Arc<HashMap<String, crate::db::StyleDNA>>, AppError> {
    WRITER_STYLE_DNAS
        .get_or_try_init(|| async {
            let pool = app_handle.state::<crate::db::DbPool>().inner().clone();
            let dnas = tokio::task::spawn_blocking(move || {
                let repo = crate::db::StyleDnaRepository::new(pool);
                repo.get_all()
                    .map(|dnas| {
                        dnas.into_iter()
                            .map(|d| (d.id.clone(), d))
                            .collect::<HashMap<String, crate::db::StyleDNA>>()
                    })
                    .map_err(|e| AppError::internal(format!("风格 DNA 加载失败: {}", e)))
            })
            .await
            .map_err(|e| AppError::internal(format!("StyleDNA 加载任务失败: {}", e)))??;
            Ok::<_, AppError>(Arc::new(dnas))
        })
        .await
}

/// Agent类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Writer,             // 写作助手
    Inspector,          // 质检员
    OutlinePlanner,     // 大纲规划师
    StyleMimic,         // 风格模仿师
    PlotAnalyzer,       // 情节分析师
    MemoryCompressor,   // 记忆压缩师
    Commentator,        // 古典评点家
    KnowledgeDistiller, // 知识蒸馏师
}

impl AgentType {
    pub fn name(&self) -> &'static str {
        match self {
            AgentType::Writer => "写作助手",
            AgentType::Inspector => "质检员",
            AgentType::OutlinePlanner => "大纲规划师",
            AgentType::StyleMimic => "风格模仿师",
            AgentType::PlotAnalyzer => "情节分析师",
            AgentType::MemoryCompressor => "记忆压缩师",
            AgentType::Commentator => "古典评点家",
            AgentType::KnowledgeDistiller => "知识蒸馏师",
        }
    }

    pub fn agent_id(&self) -> &'static str {
        match self {
            AgentType::Writer => "writer",
            AgentType::Inspector => "inspector",
            AgentType::OutlinePlanner => "outline_planner",
            AgentType::StyleMimic => "style_mimic",
            AgentType::PlotAnalyzer => "plot_analyzer",
            AgentType::MemoryCompressor => "memory_compressor",
            AgentType::Commentator => "commentator",
            AgentType::KnowledgeDistiller => "knowledge_distiller",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AgentType::Writer => "根据上下文生成或改写章节内容",
            AgentType::Inspector => "检查内容质量、逻辑连贯性、人物一致性",
            AgentType::OutlinePlanner => "设计故事大纲、章节结构",
            AgentType::StyleMimic => "分析并模仿特定文风",
            AgentType::PlotAnalyzer => "分析情节复杂度、检测漏洞",
            AgentType::MemoryCompressor => "将详细内容压缩为高层记忆摘要",
            AgentType::Commentator => "以金圣叹风格对小说段落进行实时文学点评",
            AgentType::KnowledgeDistiller => "将知识图谱蒸馏为高层故事摘要与世界观总结",
        }
    }
}

/// Agent任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub agent_type: AgentType,
    pub context: AgentContext,
    pub input: String,
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<SubscriptionTier>,
}

/// Agent执行事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub task_id: String,
    pub agent_type: String,
    pub stage: AgentStage,
    pub message: String,
    pub progress: f32, // 0.0 - 1.0
    /// 关联的 LLM request_id，供上层取消使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStage {
    Started,
    Thinking,
    Generating,
    Reviewing,
    Completed,
    Failed,
}

/// Writer 候选/生成前的预准备上下文，供多个候选共享以避免重复工作。
#[derive(Debug, Clone)]
pub struct WriterPreparedContext {
    pub prompt: String,
    pub max_tokens: Option<i32>,
    pub base_temperature: f32,
    pub tier: SubscriptionTier,
}

/// Agent服务
pub struct AgentService {
    app_handle: AppHandle,
    llm_service: LlmService,
}

impl AgentService {
    pub fn new(app_handle: AppHandle) -> Self {
        let llm_service = LlmService::new(app_handle.clone());

        Self {
            app_handle,
            llm_service,
        }
    }

    /// 获取 AppHandle 引用（用于上下文构建等场景）
    pub fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }

    /// 检查指定 request_id 是否已被请求取消
    pub fn is_cancelled(&self, request_id: &str) -> bool {
        self.llm_service.is_cancelled(request_id)
    }

    /// v0.17.1: 解析 prompt id（先查 DB override，否则回退到内置默认）。
    /// 该函数在没有 DbPool 的早期启动场景下也能工作。
    fn resolve_prompt(&self, prompt_id: &str) -> String {
        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            if let Ok(content) = crate::prompts::registry::resolve_prompt(pool.inner(), prompt_id) {
                return content;
            }
        }
        crate::prompts::registry::resolve_prompt_default(prompt_id).unwrap_or_default()
    }

    /// 从用户指令中提取明确的题材要求
    fn extract_genre_from_instruction(instruction: &str) -> Option<String> {
        let lower = instruction.to_lowercase();
        // 常见题材关键词映射（按匹配长度降序，避免"玄幻"匹配"都市玄幻"时丢失前缀）
        let genre_keywords: &[(&str, &str)] = &[
            ("都市玄幻", "都市玄幻"),
            ("都市修仙", "都市修仙"),
            ("都市异能", "都市异能"),
            ("都市言情", "都市言情"),
            ("都市", "都市"),
            ("玄幻", "玄幻"),
            ("仙侠", "仙侠"),
            ("武侠", "武侠"),
            ("科幻", "科幻"),
            ("悬疑", "悬疑"),
            ("推理", "推理"),
            ("恐怖", "恐怖"),
            ("灵异", "灵异"),
            ("末世", "末世"),
            ("末日", "末世"),
            ("穿越", "穿越"),
            ("重生", "重生"),
            ("古言", "古言"),
            ("古代言情", "古言"),
            ("现言", "现言"),
            ("现代言情", "现言"),
            ("耽美", "耽美"),
            ("同人", "同人"),
            ("轻小说", "轻小说"),
            ("历史", "历史"),
            ("军事", "军事"),
            ("游戏", "游戏"),
            ("体育", "体育"),
            ("洪荒", "洪荒"),
            ("西游", "西游"),
            ("封神", "封神"),
            ("诸天", "诸天"),
            ("无限流", "无限流"),
        ];
        for (keyword, genre) in genre_keywords {
            if lower.contains(&keyword.to_lowercase()) {
                return Some(genre.to_string());
            }
        }
        None
    }

    /// 执行Agent任务
    pub async fn execute_task(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let task_id = task.id.clone();
        let agent_type = task.agent_type;

        // 发送开始事件
        self.emit_event(
            &task_id,
            agent_type,
            AgentStage::Started,
            "开始执行任务",
            0.0,
        );

        let result = match agent_type {
            AgentType::Writer => self.execute_writer_raw(task).await,
            AgentType::Inspector => self.execute_inspector(task).await,
            AgentType::OutlinePlanner => self.execute_outline_planner(task).await,
            AgentType::StyleMimic => self.execute_style_mimic(task).await,
            AgentType::PlotAnalyzer => self.execute_plot_analyzer(task).await,
            AgentType::MemoryCompressor => self.execute_memory_compressor(task).await,
            AgentType::Commentator => self.execute_commentator(task).await,
            AgentType::KnowledgeDistiller => self.execute_knowledge_distiller(task).await,
        };

        match &result {
            Ok(_) => {
                self.emit_event(&task_id, agent_type, AgentStage::Completed, "任务完成", 1.0);
            }
            Err(e) => {
                self.emit_event(
                    &task_id,
                    agent_type,
                    AgentStage::Failed,
                    &format!("执行失败: {}", e),
                    0.0,
                );
            }
        }

        result
    }

    /// 获取Agent对应的聊天模型ID
    fn get_agent_chat_model_id(&self, agent_type: AgentType) -> Option<String> {
        self.get_agent_model_id(agent_type, |m| m.chat_model_id.as_ref())
    }

    /// 获取Agent对应的嵌入模型ID
    #[allow(dead_code)]
    fn get_agent_embedding_model_id(&self, agent_type: AgentType) -> Option<String> {
        self.get_agent_model_id(agent_type, |m| m.embedding_model_id.as_ref())
    }

    /// 获取Agent对应的多模态模型ID
    #[allow(dead_code)]
    fn get_agent_multimodal_model_id(&self, agent_type: AgentType) -> Option<String> {
        self.get_agent_model_id(agent_type, |m| m.multimodal_model_id.as_ref())
    }

    /// 获取模型配置的友好显示名称；若找不到则回退到模型ID
    fn get_model_display_name(&self, model_id: &str) -> String {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        if let Ok(config) = AppConfig::load(&app_dir) {
            if let Some(profile) = config.llm_profiles.get(model_id) {
                return profile.name.clone();
            }
            if let Some(profile) = config.embedding_profiles.get(model_id) {
                return profile.name.clone();
            }
        }

        model_id.to_string()
    }

    fn get_agent_model_id<F>(&self, agent_type: AgentType, extractor: F) -> Option<String>
    where
        F: FnOnce(&crate::config::AgentMapping) -> Option<&String>,
    {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let config = AppConfig::load(&app_dir).ok()?;
        let mapping = config.agent_mappings.get(agent_type.agent_id())?;
        let model_id = extractor(mapping)?;

        // 校验模型ID是否仍然存在于对应配置中，避免引用已删除模型
        let is_valid = config.llm_profiles.contains_key(model_id)
            || config.embedding_profiles.contains_key(model_id);
        if !is_valid {
            log::warn!(
                "[AgentService] Agent '{}' references removed model '{}', falling back to default",
                agent_type.agent_id(),
                model_id
            );
            return None;
        }

        Some(model_id.clone())
    }

    /// 获取 Agent 对应的生成参数（max_tokens, temperature）。
    /// 优先从 Agent 映射的 chat 模型 profile 读取；未设置则使用经验默认值。
    fn get_agent_llm_params(
        &self,
        agent_type: AgentType,
        default_max_tokens: i32,
        default_temperature: f32,
    ) -> (Option<i32>, Option<f32>) {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let Ok(config) = AppConfig::load(&app_dir) else {
            return (Some(default_max_tokens), Some(default_temperature));
        };

        let Some(mapping) = config.agent_mappings.get(agent_type.agent_id()) else {
            return (Some(default_max_tokens), Some(default_temperature));
        };

        let Some(model_id) = mapping.chat_model_id.as_ref() else {
            return (Some(default_max_tokens), Some(default_temperature));
        };

        let Some(profile) = config.llm_profiles.get(model_id) else {
            return (Some(default_max_tokens), Some(default_temperature));
        };

        let max_tokens = if profile.max_tokens > 0 {
            Some(profile.max_tokens)
        } else {
            Some(default_max_tokens)
        };
        let temperature = if profile.temperature >= 0.0 {
            Some(profile.temperature)
        } else {
            Some(default_temperature)
        };

        (max_tokens, temperature)
    }

    /// 获取当前用户 ID
    fn get_user_id(&self) -> String {
        let app_dir = match self.app_handle.path().app_data_dir() {
            Ok(d) => d,
            Err(_) => return "local".to_string(),
        };
        let machine_id_path = app_dir.join(".machine_id");
        if machine_id_path.exists() {
            std::fs::read_to_string(&machine_id_path)
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            "local".to_string()
        }
    }

    /// 获取当前用户的订阅层级（fallback 查询，优先使用 task.tier）
    fn get_user_tier(&self) -> SubscriptionTier {
        let user_id = self.get_user_id();
        if user_id.is_empty() || user_id == "local" {
            log::warn!("[AgentService] user_id is empty/local, defaulting to Free");
            return SubscriptionTier::Free;
        }

        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            let service = SubscriptionService::new(pool.inner().clone());
            match service.get_or_create_subscription(&user_id) {
                Ok(status) => match status.tier.parse() {
                    Ok(tier) => return tier,
                    Err(e) => log::warn!(
                        "[AgentService] Failed to parse tier '{}': {}, defaulting to Free",
                        status.tier,
                        e
                    ),
                },
                Err(e) => log::warn!("[AgentService] DB query failed: {}, defaulting to Free", e),
            }
        } else {
            log::warn!("[AgentService] DbPool not available, defaulting to Free");
        }
        SubscriptionTier::Free
    }

    /// 从 task 或 fallback 获取 tier
    fn resolve_tier(&self, task: &AgentTask) -> SubscriptionTier {
        task.tier.unwrap_or_else(|| self.get_user_tier())
    }

    /// 获取 Agent 映射配置
    fn get_agent_mapping(&self, agent_type: AgentType) -> Option<crate::config::AgentMapping> {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        AppConfig::load(&app_dir)
            .ok()?
            .agent_mappings
            .get(agent_type.agent_id())
            .cloned()
    }

    /// 借用 LlmService 引用（供 orchestrator 在 TimeSliced 路径直接调用，绕过
    /// execute_writer_raw）
    pub fn llm_service_ref(&self) -> &LlmService {
        &self.llm_service
    }

    /// 判断指定 Agent 实际会使用的目标模型是否为本地模型。
    /// 用于候选阶段并发策略与 Writer 全局并发限制。
    pub fn is_target_model_local(&self, agent_type: AgentType) -> bool {
        use crate::config::settings::{is_private_url, LlmProvider, ModelSource};

        let app_dir = match self.app_handle.path().app_data_dir() {
            Ok(d) => d,
            Err(_) => return false,
        };
        let config = match AppConfig::load(&app_dir) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let is_local_profile = |p: &crate::config::settings::LlmProfile| {
            p.is_local_model
                || p.model_source == ModelSource::Local
                || p.provider == LlmProvider::Ollama
                || p.api_base.as_deref().map(is_private_url).unwrap_or(false)
        };

        let model_id = if let Some(id) = self.get_agent_chat_model_id(agent_type) {
            id
        } else {
            let mapping = match self.get_agent_mapping(agent_type) {
                Some(m) => m,
                None => return false,
            };
            let tier = self.get_user_tier();
            let request = self.build_routing_request(agent_type, tier, Some(&mapping));
            match self.llm_service.select_profile_for_request(&request) {
                Ok(p) => return is_local_profile(&p),
                Err(_) => return false,
            };
        };

        config
            .llm_profiles
            .get(&model_id)
            .map(is_local_profile)
            .unwrap_or(false)
    }

    /// 根据 Agent 类型、订阅层级与映射配置构建路由请求
    fn build_routing_request(
        &self,
        agent_type: AgentType,
        tier: SubscriptionTier,
        mapping: Option<&crate::config::AgentMapping>,
    ) -> RoutingRequest {
        let budget_priority = match tier {
            SubscriptionTier::Free => Priority::High,
            _ => Priority::Low,
        };

        let mut request = match agent_type {
            AgentType::Writer => RoutingRequest {
                task: TaskType::CreativeWriting,
                complexity: Complexity::High,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::Inspector => RoutingRequest {
                task: TaskType::Editing,
                complexity: Complexity::Medium,
                budget_priority,
                speed_priority: Priority::Medium,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::OutlinePlanner => RoutingRequest {
                task: TaskType::WorldBuilding,
                complexity: Complexity::High,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::StyleMimic => RoutingRequest {
                task: TaskType::Analysis,
                complexity: Complexity::Medium,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::PlotAnalyzer => RoutingRequest {
                task: TaskType::Analysis,
                complexity: Complexity::High,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::MemoryCompressor => RoutingRequest {
                task: TaskType::Summarization,
                complexity: Complexity::Medium,
                budget_priority,
                speed_priority: Priority::Medium,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::Commentator => RoutingRequest {
                task: TaskType::CreativeWriting,
                complexity: Complexity::Medium,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
            AgentType::KnowledgeDistiller => RoutingRequest {
                task: TaskType::Summarization,
                complexity: Complexity::Medium,
                budget_priority,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            },
        };

        // 应用用户在 Agent 映射中配置的策略覆盖
        if let Some(m) = mapping {
            if let Some(t) = m.task_type.as_deref() {
                request.task = parse_task_type(t).unwrap_or(request.task);
            }
            if let Some(c) = m.complexity.as_deref() {
                request.complexity = parse_complexity(c).unwrap_or(request.complexity);
            }
            if let Some(p) = m.budget_priority.as_deref() {
                request.budget_priority = parse_priority(p).unwrap_or(request.budget_priority);
            }
            if let Some(p) = m.speed_priority.as_deref() {
                request.speed_priority = parse_priority(p).unwrap_or(request.speed_priority);
            }
            for c in &m.constraints {
                if let Some(constraint) = parse_constraint(c) {
                    request.constraints.push(constraint);
                }
            }
        }

        request
    }

    /// 为Agent生成内容，优先使用映射的模型
    /// 免费版限制 max_tokens 以控制成本与质量
    /// `request_id`: 上层传入的取消标识；为 None 时内部生成 UUID
    async fn generate_for_agent(
        &self,
        task: &AgentTask,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        tier: SubscriptionTier,
        request_id: Option<String>,
    ) -> Result<(String, crate::llm::GenerateResponse), AppError> {
        self.generate_for_agent_with_options(
            task,
            prompt,
            max_tokens,
            temperature,
            tier,
            request_id,
            None,
            None,
        )
        .await
    }

    /// 为Agent生成内容，支持超时与重试覆盖。
    async fn generate_for_agent_with_options(
        &self,
        task: &AgentTask,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        tier: SubscriptionTier,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
    ) -> Result<(String, crate::llm::GenerateResponse), AppError> {
        // v0.9.5: 协作式取消 —— 若上层已请求取消，直接返回，避免无效 LLM 调用
        if let Some(ref req_id) = request_id {
            if self.llm_service.is_cancelled(req_id) {
                return Err(AppError::internal("生成已取消".to_string()));
            }
        }

        let start_time = std::time::Instant::now();
        let effective_max = match tier {
            // v0.9.6: 免费版也需要足够长度生成完整场景，放宽到 2000 tokens
            SubscriptionTier::Free => max_tokens.map(|m| m.min(2000)).or(Some(2000)),
            _ => max_tokens,
        };
        let agent_type = task.agent_type;

        log::info!(
            "[AgentService] generate_for_agent_with_options: agent={:?}, story_id={}, prompt_len={}, max_tokens={:?}, temperature={:?}",
            agent_type,
            task.context.story.story_id,
            prompt.chars().count(),
            effective_max,
            temperature
        );

        // 发送准备调用LLM事件
        self.emit_event(
            &task.id,
            agent_type,
            AgentStage::Generating,
            "准备调用模型...",
            0.3,
        );

        // 发送获取模型配置事件
        self.emit_event(
            &task.id,
            agent_type,
            AgentStage::Generating,
            "正在获取模型配置...",
            0.32,
        );

        // v0.15.0: 网关介入 - 优先走网关路由，agent_mapping 作为偏好提示
        // 仅在 force_locked=true 时保留旧行为（绕过网关直连指定模型）
        let mapping = self.get_agent_mapping(agent_type);
        let force_locked = mapping.as_ref().map(|m| m.force_locked).unwrap_or(false);
        let (req_id, response) = if let Some(model_id) = self.get_agent_chat_model_id(agent_type) {
            if force_locked {
                // v0.15.0: force_locked - 保留旧行为，绕过网关
                let display_name = self.get_model_display_name(&model_id);
                self.emit_event(
                    &task.id,
                    agent_type,
                    AgentStage::Generating,
                    &format!("使用强制锁定模型 {} 生成...", display_name),
                    0.35,
                );
                let (rid, result) = self
                    .llm_service
                    .generate_with_profile_and_request_id(
                        &model_id,
                        prompt.clone(),
                        effective_max,
                        temperature,
                        None,
                        request_id.clone(),
                        timeout_seconds_override,
                        max_retries_override,
                    )
                    .await;
                (rid, result?)
            } else {
                // v0.15.0: 有偏好但未锁定 - 走网关，偏好模型作为提示
                self.emit_event(
                    &task.id,
                    agent_type,
                    AgentStage::Generating,
                    "正在通过模型网关智能路由（用户偏好模型作为提示）...",
                    0.32,
                );
                let routing_request =
                    self.build_routing_request(agent_type, tier, mapping.as_ref());
                self.emit_event(
                    &task.id,
                    agent_type,
                    AgentStage::Generating,
                    "通过模型网关路由并生成...",
                    0.35,
                );
                let (rid, result) = self
                    .llm_service
                    .generate_for_request_with_request_id(
                        routing_request,
                        prompt.clone(),
                        effective_max,
                        temperature,
                        None,
                        request_id.clone(),
                        timeout_seconds_override,
                        max_retries_override,
                    )
                    .await;
                (rid, result?)
            }
        } else {
            // v0.11.0: 无固定映射时采用自动路由策略，根据 Agent 类型选择模型
            self.emit_event(
                &task.id,
                agent_type,
                AgentStage::Generating,
                "正在根据任务策略路由模型...",
                0.32,
            );
            let mapping = self.get_agent_mapping(agent_type);
            let routing_request = self.build_routing_request(agent_type, tier, mapping.as_ref());
            self.emit_event(
                &task.id,
                agent_type,
                AgentStage::Generating,
                "通过模型网关路由并生成...",
                0.35,
            );
            let (rid, result) = self
                .llm_service
                .generate_for_request_with_request_id(
                    routing_request,
                    prompt.clone(),
                    effective_max,
                    temperature,
                    None,
                    request_id.clone(),
                    timeout_seconds_override,
                    max_retries_override,
                )
                .await;
            (rid, result?)
        };

        // 暴露 request_id 到前端事件
        self.emit_event_with_request_id(
            &task.id,
            agent_type,
            AgentStage::Generating,
            "模型生成中...",
            0.4,
            Some(req_id.clone()),
        );

        let duration_ms = start_time.elapsed().as_millis() as i32;

        // C2: 输出结构化 LLM 耗时指标。同步路径无法获得真实首 token 时间，
        // 以总耗时作为 TTFT 近似值并标注。
        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "phase": "llm_total",
                "elapsed_ms": duration_ms,
                "request_id": req_id,
                "task_id": task.id,
                "agent_type": agent_type.agent_id(),
                "model": response.model,
                "tokens_used": response.tokens_used,
            })
        );
        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "phase": "llm_ttft",
                "elapsed_ms": duration_ms,
                "request_id": req_id,
                "task_id": task.id,
                "details": "sync_path_approx",
            })
        );

        // 记录 AI 使用日志
        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            let service = SubscriptionService::new(pool.inner().clone());
            let user_id = self.get_user_id();
            let tier_str = match tier {
                SubscriptionTier::Free => "free",
                SubscriptionTier::Pro => "pro",
                SubscriptionTier::Enterprise => "enterprise",
            };
            let prompt_tokens = Some((prompt.chars().count() as i32) / 2);
            let _ = service.log_ai_usage(
                &user_id,
                Some(&task.context.story.story_id),
                None,
                agent_type.agent_id(),
                Some(&task.input),
                prompt_tokens,
                Some(response.tokens_used),
                Some(&response.model),
                Some(response.cost),
                Some(duration_ms),
                tier_str,
            );
        }

        Ok((req_id, response))
    }

    /// 原始 Writer 生成 — 只生成内容，不进入闭环。
    /// 完整链路：预检/补齐 → 构建 prompt → 生成策略 → LLM 生成 → 后处理。
    pub async fn execute_writer_raw(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let prepared = self.prepare_writer_context(&task).await?;
        self.execute_writer_prepared(task, prepared, None, None)
            .await
    }

    /// 预准备 Writer 上下文：执行预检、自动补齐、构建 prompt 与生成策略。
    /// 多个候选可共享同一份预准备结果，避免重复 LLM 调用与重复构建。
    ///
    /// 外层套 60 秒整体超时：若 SQLite
    /// 锁竞争或某同步步骤仍阻塞，超时后返回明确错误，
    /// 避免前端长期显示"系统正在处理中..."。
    pub async fn prepare_writer_context(
        &self,
        task: &AgentTask,
    ) -> Result<WriterPreparedContext, AppError> {
        let start = std::time::Instant::now();
        match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.prepare_writer_context_inner(task),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                log::error!(
                    "[AgentService] prepare_writer_context timed out after {}ms for story {}",
                    start.elapsed().as_millis(),
                    task.context.story.story_id
                );
                Err(AppError::internal(
                    "写作上下文准备超时（60秒），请检查数据库或模型配置后重试".to_string(),
                ))
            }
        }
    }

    async fn prepare_writer_context_inner(
        &self,
        task: &AgentTask,
    ) -> Result<WriterPreparedContext, AppError> {
        let start = std::time::Instant::now();
        log::info!(
            "[AgentService] prepare_writer_context start: story_id={}, chapter_number={}, agent_task_id={}",
            task.context.story.story_id,
            task.context.narrative.chapter_number,
            task.id
        );
        let pool = self.app_handle.state::<crate::db::DbPool>();
        let checker = crate::story_system::preflight::PreflightChecker::new();
        let preflight = checker
            .check(
                pool.inner(),
                &task.context.story.story_id,
                task.context.narrative.chapter_number as i32,
            )
            .await;
        if !preflight.ready {
            log::info!(
                "[AgentService] Preflight failed for story {} chapter {}, blocking_issues={:?}, \
                 attempting auto-fill",
                task.context.story.story_id,
                task.context.narrative.chapter_number,
                preflight.blocking_issues
            );

            let _ = self.app_handle.emit(
                "agent-stage-update",
                serde_json::json!({
                    "agent_type": "Writer",
                    "stage": "Started",
                    "message": "检测到写作前置条件缺失，正在自动补齐...",
                    "progress": 0.02,
                    "request_id": null,
                }),
            );

            let builder = crate::story_system::auto_contract::AutoContractBuilder::new(
                pool.inner().clone(),
                self.app_handle.clone(),
            );

            // v0.9.7: 将同步场景查询移入 spawn_blocking，避免阻塞 tokio worker
            let target_scene_id = {
                let pool = pool.inner().clone();
                let story_id = task.context.story.story_id.clone();
                let chapter_number = task.context.narrative.chapter_number as i32;
                tokio::task::spawn_blocking(move || {
                    let scene_repo = crate::db::repositories::SceneRepository::new(pool);
                    scene_repo
                        .get_by_story(&story_id)
                        .ok()
                        .and_then(|scenes| {
                            scenes
                                .into_iter()
                                .find(|s| s.sequence_number == chapter_number)
                        })
                        .map(|s| s.id)
                })
                .await
                .unwrap_or_else(|e| {
                    log::warn!("[AgentService] 目标场景查询任务失败: {}", e);
                    None
                })
            };

            match builder
                .auto_fill(
                    &task.context.story.story_id,
                    task.context.narrative.chapter_number as i32,
                    target_scene_id.as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if result.created_master
                        || result.created_chapter
                        || result.created_character
                        || result.created_scene
                        || result.created_outline
                    {
                        log::info!(
                            "[AgentService] Auto-fill completed: master={}, chapter={}, \
                             character={}, scene={}, outline={}",
                            result.created_master,
                            result.created_chapter,
                            result.created_character,
                            result.created_scene,
                            result.created_outline
                        );

                        let preflight_after = checker
                            .check(
                                pool.inner(),
                                &task.context.story.story_id,
                                task.context.narrative.chapter_number as i32,
                            )
                            .await;
                        if !preflight_after.ready {
                            return Err(AppError::preflight_failed(
                                "自动补齐后写作前检查仍发现阻塞性问题",
                                preflight_after.blocking_issues,
                            ));
                        }
                    } else {
                        log::warn!("[AgentService] Auto-fill did not create any items");
                        return Err(AppError::preflight_failed(
                            "写作前检查发现阻塞性问题",
                            preflight.blocking_issues,
                        ));
                    }
                }
                Err(e) => {
                    log::warn!("[AgentService] Auto-fill failed: {}", e);
                    return Err(AppError::preflight_failed(
                        "写作前检查发现阻塞性问题，自动补齐失败",
                        preflight.blocking_issues,
                    ));
                }
            }
        }

        let tier = self.resolve_tier(task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析写作上下文",
            0.1,
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "正在读取订阅配置...",
            0.12,
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "正在构建写作提示词...",
            0.15,
        );
        let prompt = self.build_writer_prompt(task, tier).await;
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "写作提示词构建完成",
            0.2,
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "准备生成内容...",
            0.25,
        );

        let user_temperature = self
            .llm_service
            .get_active_profile()
            .map(|p| p.temperature)
            .unwrap_or(0.8);

        let story_progress = task
            .parameters
            .get("story_progress")
            .and_then(|v| v.as_str());
        let scene_stage = task
            .parameters
            .get("current_scene_stage")
            .and_then(|v| v.as_str());

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "正在加载用户偏好...",
            0.28,
        );
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "正在查询用户反馈历史...",
            0.281,
        );
        let (max_tokens, temperature) = {
            let pool = self.app_handle.state::<crate::db::DbPool>();
            let generator =
                crate::creative_engine::adaptive::AdaptiveGenerator::new(pool.inner().clone());
            self.emit_event(
                &task.id,
                task.agent_type,
                AgentStage::Generating,
                "正在计算生成策略...",
                0.285,
            );
            match generator
                .build_strategy_with_context(
                    &task.context.story.story_id,
                    Some(user_temperature),
                    story_progress,
                    scene_stage,
                )
                .await
            {
                Ok(strategy) => {
                    log::info!(
                        "[AgentService] Adaptive strategy for story {}: progress={:?}, \
                         stage={:?}, base_temp={}, adjusted_temp={}, max_tokens={}",
                        task.context.story.story_id,
                        story_progress,
                        scene_stage,
                        user_temperature,
                        strategy.temperature,
                        strategy.max_tokens
                    );
                    self.emit_event(
                        &task.id,
                        task.agent_type,
                        AgentStage::Generating,
                        &format!(
                            "生成策略已构建: temperature={:.2}, max_tokens={}",
                            strategy.temperature, strategy.max_tokens
                        ),
                        0.3,
                    );
                    (Some(strategy.max_tokens), strategy.temperature)
                }
                Err(e) => {
                    log::warn!(
                        "[AgentService] Failed to build adaptive strategy: {}, using defaults",
                        e
                    );
                    self.emit_event(
                        &task.id,
                        task.agent_type,
                        AgentStage::Generating,
                        "使用默认生成策略",
                        0.3,
                    );
                    (Some(2000), user_temperature)
                }
            }
        };

        log::info!(
            "[AgentService] prepare_writer_context end: story_id={}, elapsed_ms={}, prompt_len={}",
            task.context.story.story_id,
            start.elapsed().as_millis(),
            prompt.chars().count()
        );

        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "phase": "prepare_writer_context",
                "elapsed_ms": start.elapsed().as_millis(),
                "story_id": task.context.story.story_id,
                "task_id": task.id,
                "prompt_len": prompt.chars().count(),
            })
        );

        Ok(WriterPreparedContext {
            prompt,
            max_tokens,
            base_temperature: temperature,
            tier,
        })
    }

    /// 使用已预准备的上下文执行 Writer 生成，跳过预检、补齐、prompt
    /// 构建与策略计算。 `timeout_override` 与 `retries_override`
    /// 用于候选阶段短超时/少重试。
    pub async fn execute_writer_prepared(
        &self,
        task: AgentTask,
        prepared: WriterPreparedContext,
        timeout_override: Option<u64>,
        retries_override: Option<u32>,
    ) -> Result<AgentResult, AppError> {
        let mut temperature = prepared.base_temperature;

        // v0.7.8: 支持 temperature override（用于候选生成多样性）
        if let Some(override_val) = task
            .parameters
            .get("temperature_override")
            .and_then(|v| v.as_f64())
        {
            temperature = override_val as f32;
            log::info!("[AgentService] Temperature override: {}", override_val);
        }

        // v0.11.8: 全局 Writer 并发限制 — 本地 1、远端 2，避免本地服务端被
        // 多个候选/任务同时压垮。
        let is_local = self.is_target_model_local(AgentType::Writer);
        let _writer_permit = self.llm_service.acquire_writer_permit(is_local).await?;

        let request_id = uuid::Uuid::new_v4().to_string();
        let (req_id, response) = self
            .generate_for_agent_with_options(
                &task,
                prepared.prompt,
                prepared.max_tokens,
                Some(temperature),
                prepared.tier,
                Some(request_id.clone()),
                timeout_override,
                retries_override,
            )
            .await?;

        if response.content.trim().is_empty() {
            log::error!(
                "[AgentService::execute_writer_prepared] LLM returned empty content. story_id={}, \
                 chapter_number={}, instruction_len={}",
                task.context.story.story_id,
                task.context.narrative.chapter_number,
                task.input.len()
            );
            return Err(AppError::internal("AI 返回了空内容，请检查模型配置或重试"));
        }
        let mut content = response.content;
        if let Some(ref current) = task.context.narrative.current_content {
            if !current.is_empty() && current != "无" {
                let current_trimmed = current.trim();
                let content_trimmed = content.trim();
                if content_trimmed.starts_with(current_trimmed) {
                    content = content_trimmed[current_trimmed.len()..]
                        .trim_start()
                        .to_string();
                    log::info!(
                        "[execute_writer_prepared] Removed duplicate prefix (len={}) from LLM \
                         output, remaining len={}",
                        current_trimmed.len(),
                        content.len()
                    );
                    if content.trim().is_empty() {
                        log::error!(
                            "[AgentService::execute_writer_prepared] Content became empty after \
                             prefix removal. story_id={}, chapter_number={}",
                            task.context.story.story_id,
                            task.context.narrative.chapter_number
                        );
                        return Err(AppError::internal(
                            "AI 返回的内容与已有文本完全重复，请重试",
                        ));
                    }
                }
            }
        }

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Reviewing,
            "检查生成质量",
            0.8,
        );

        let score = self.calculate_quality_score(&content);

        let mut suggestions = if score < 0.7 {
            vec!["建议：内容可能需要进一步润色".to_string()]
        } else {
            vec![]
        };

        // v0.9.7: 将同步场景查询与连续性检查整体移入 spawn_blocking
        let continuity_suggestions: Vec<String> = {
            let pool = self.app_handle.state::<crate::db::DbPool>().inner().clone();
            let story_id = task.context.story.story_id.clone();
            let chapter_number = task.context.narrative.chapter_number as i32;
            let content = content.clone();
            tokio::task::spawn_blocking(move || {
                let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
                let mut issues = Vec::new();
                if let Ok(scenes) = scene_repo.get_by_story(&story_id) {
                    if let Some(scene) = scenes
                        .iter()
                        .find(|s| s.sequence_number == chapter_number)
                    {
                        let continuity =
                            crate::creative_engine::continuity::ContinuityEngine::new(pool);
                        match continuity.check_scene_continuity(&story_id, &scene.id, &content) {
                            Ok(check) if !check.is_valid => {
                                for issue in check.issues {
                                    let msg = format!(
                                        "[{}] {}",
                                        match issue.severity {
                                            crate::creative_engine::continuity::Severity::Critical =>
                                                "严重",
                                            crate::creative_engine::continuity::Severity::Warning =>
                                                "警告",
                                            _ => "提示",
                                        },
                                        issue.message
                                    );
                                    issues.push(msg);
                                    log::warn!(
                                        "[ContinuityEngine] {:?}: {}",
                                        issue.issue_type,
                                        issue.message
                                    );
                                }
                            }
                            Ok(_) => {}
                            Err(e) => log::warn!("[ContinuityEngine] Check failed: {}", e),
                        }
                    }
                }
                issues
            })
            .await
            .unwrap_or_else(|e| {
                log::warn!("[AgentService] 连续性检查任务失败: {}", e);
                vec![]
            })
        };
        suggestions.extend(continuity_suggestions);

        // v0.7.8: 风格一致性快速检查（ fingerprint 存在时）
        if let Some(ref fingerprint) = task.context.style.style_fingerprint {
            let style_check =
                crate::creative_engine::style::fingerprint::StyleFingerprint::from_text(&content);
            let len_diff = (style_check.syntax.avg_sentence_length
                - fingerprint.syntax.avg_sentence_length)
                .abs();
            let len_deviation = if fingerprint.syntax.avg_sentence_length > 0.0 {
                len_diff / fingerprint.syntax.avg_sentence_length
            } else {
                0.0
            };

            if len_deviation > 0.3 {
                suggestions.push(format!(
                    "[风格] 句长偏离 {:.0}%：实际平均 {:.0} 字 vs 参考 {:.0} 字",
                    len_deviation * 100.0,
                    style_check.syntax.avg_sentence_length,
                    fingerprint.syntax.avg_sentence_length
                ));
                log::warn!(
                    "[StyleCheck] Sentence length drift: {:.0}%",
                    len_deviation * 100.0
                );
            }

            let four_char_diff = (style_check.vocabulary.four_char_density
                - fingerprint.vocabulary.four_char_density)
                .abs();
            if four_char_diff > 5.0 {
                suggestions.push(format!(
                    "[风格] 四字格密度偏离：实际 {:.1}% vs 参考 {:.1}%",
                    style_check.vocabulary.four_char_density,
                    fingerprint.vocabulary.four_char_density
                ));
            }
        }

        Ok(AgentResult {
            content,
            score: Some(score),
            suggestions,
            request_id: Some(req_id),
        })
    }

    /// 执行质检员
    async fn execute_inspector(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析内容质量",
            0.1,
        );

        let prompt = self.build_inspector_prompt(&task);

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "生成质检报告",
            0.4,
        );

        let (max_tokens, temperature) = self.get_agent_llm_params(AgentType::Inspector, 1500, 0.3);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        // 解析质检结果
        let (score, suggestions) = self.parse_inspection_result(&response.content);

        Ok(AgentResult {
            content: response.content,
            score: Some(score),
            suggestions,
            request_id: None,
        })
    }

    /// 执行大纲规划师
    async fn execute_outline_planner(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析故事需求",
            0.1,
        );

        let prompt = self.build_outline_prompt(&task);

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "设计故事大纲",
            0.3,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::OutlinePlanner, 3000, 0.9);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        Ok(AgentResult {
            content: response.content,
            score: Some(0.95),
            suggestions: vec![],
            request_id: None,
        })
    }

    /// 执行风格模仿师
    async fn execute_style_mimic(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析文风特征",
            0.1,
        );

        let prompt = self.build_style_prompt(&task);

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "模仿指定文风",
            0.4,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::StyleMimic, 2000, 0.85);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        Ok(AgentResult {
            content: response.content,
            score: Some(0.9),
            suggestions: vec![],
            request_id: None,
        })
    }

    /// 执行情节分析师
    async fn execute_plot_analyzer(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析情节结构",
            0.1,
        );

        let prompt = self.build_plot_prompt(&task);

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "生成分析报告",
            0.4,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::PlotAnalyzer, 2000, 0.4);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        let (score, suggestions) = self.parse_plot_analysis(&response.content);

        Ok(AgentResult {
            content: response.content,
            score: Some(score),
            suggestions,
            request_id: None,
        })
    }

    /// 执行古典评点家
    async fn execute_commentator(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "品读文本",
            0.1,
        );

        let ctx = &task.context;
        let prompt = format!(
            r#"你是一位中国古典小说评点家，风格类似金圣叹。请对以下小说段落进行简短点评。

【作品信息】
标题: {}
题材: {}

【待评段落】
{}

【点评要求】
1. 用古典文人评点的口吻，简洁有力，每段不超过60字
2. 可点评：文笔、结构、人物、伏笔、情感、节奏
3. 语气可带几分机锋，但不可刻薄伤人
4. 直接输出 JSON 数组，格式：[{{"paragraph_index": 0, "commentary": "...", "tone": "insightful"}}]
5. tone 可选：insightful / witty / emotional / critical
6. 如果没有值得点评之处， commentary 可为空字符串

请直接输出 JSON，不要添加 markdown 代码块标记。"#,
            ctx.story.story_title, ctx.story.genre, task.input
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "生成评点",
            0.4,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::Commentator, 2048, 0.85);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        Ok(AgentResult::simple(response.content))
    }

    /// 执行记忆压缩师
    async fn execute_memory_compressor(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析待压缩内容",
            0.1,
        );

        let ctx = &task.context;
        let target_ratio = task
            .parameters
            .get("target_ratio")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.25);
        let ratio_pct = (target_ratio * 100.0) as i32;

        let prompt = format!(
            r#"你是一位专业的文学记忆压缩师。请将以下小说相关内容压缩为简洁的高层摘要。

【作品信息】
标题: {}
题材: {}
文风: {}
节奏: {}

【待压缩内容】
{}

【压缩要求】
1. 保留核心情节、人物关系、关键伏笔
2. 删除细节描写、重复叙述、过渡段落
3. 输出长度控制在原文的 {}%
4. 使用第三人称客观叙述

请直接输出压缩后的摘要，不要添加解释。"#,
            ctx.story.story_title,
            ctx.story.genre,
            ctx.story.tone,
            ctx.story.pacing,
            ratio_pct,
            task.input
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "压缩内容",
            0.4,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::MemoryCompressor, 2048, 0.3);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        let original_len = task.input.chars().count();
        let compressed_len = response.content.chars().count();
        let compression_ratio = if original_len > 0 {
            compressed_len as f32 / original_len as f32
        } else {
            1.0
        };
        let score = (1.0 - compression_ratio).max(0.0).min(1.0);

        Ok(AgentResult {
            content: response.content,
            score: Some(score),
            suggestions: vec![format!("压缩率: {:.1}%", compression_ratio * 100.0)],
            request_id: None,
        })
    }

    /// 执行知识蒸馏师
    async fn execute_knowledge_distiller(&self, task: AgentTask) -> Result<AgentResult, AppError> {
        let tier = self.resolve_tier(&task);
        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Thinking,
            "分析知识图谱结构",
            0.1,
        );

        let ctx = &task.context;
        let prompt = format!(
            r#"你是一位专业的文学知识蒸馏师。请根据以下小说知识图谱，提炼出高层摘要。

【作品信息】
标题: {}
题材: {}
文风: {}
节奏: {}

【知识图谱】
{}

【蒸馏要求】
1. 世界观概述：提炼故事的宏观设定、核心规则、时代背景
2. 主要势力：总结故事中的重要组织、阵营、群体及其关系
3. 人物关系网：梳理核心角色之间的关系、立场、冲突
4. 核心情节线：提炼当前已展开的主要悬念、伏笔、目标
5. 输出条理清晰，使用Markdown格式，总长度控制在800字以内

请直接输出蒸馏后的摘要。"#,
            ctx.story.story_title, ctx.story.genre, ctx.story.tone, ctx.story.pacing, task.input
        );

        self.emit_event(
            &task.id,
            task.agent_type,
            AgentStage::Generating,
            "蒸馏知识图谱",
            0.4,
        );

        let (max_tokens, temperature) =
            self.get_agent_llm_params(AgentType::KnowledgeDistiller, 2048, 0.4);
        let (_, response) = self
            .generate_for_agent(&task, prompt, max_tokens, temperature, tier, None)
            .await?;

        Ok(AgentResult::with_score(response.content, 0.9))
    }

    // ==================== 提示词构建（模板化） ====================

    async fn build_writer_prompt(&self, task: &AgentTask, tier: SubscriptionTier) -> String {
        use std::collections::HashMap;

        use crate::prompts::TemplateEngine;

        let ctx = &task.context;
        let has_selection = ctx
            .narrative
            .selected_text
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let is_pro = tier != SubscriptionTier::Free;
        let at = task.agent_type;
        let tid = task.id.clone();

        // 辅助：emit + yield，确保前端及时收到事件
        let emit_and_yield = |msg: &str, prog: f32| {
            self.emit_event(&tid, at, AgentStage::Thinking, msg, prog);
        };

        emit_and_yield("正在读取写作策略配置...", 0.15);
        // v0.9.7: 使用全局只读缓存，避免每次调用都重新从磁盘加载 AppConfig
        let strategy = match writer_app_config(&self.app_handle).await {
            Ok(cfg) => Some(cfg.writing_strategy.clone()),
            Err(e) => {
                log::warn!("[build_writer_prompt] Failed to load app config: {}", e);
                None
            }
        };
        tokio::task::yield_now().await;

        emit_and_yield("正在准备模板变量...", 0.155);
        let mut vars = HashMap::new();
        vars.insert("story_title".to_string(), ctx.story.story_title.clone());
        let effective_genre = Self::extract_genre_from_instruction(&task.input)
            .unwrap_or_else(|| ctx.story.genre.clone());
        if effective_genre != ctx.story.genre {
            log::info!(
                "[build_writer_prompt] Genre override: instruction hints '{}' vs story genre \
                 '{}', using '{}'",
                effective_genre,
                ctx.story.genre,
                effective_genre
            );
        }
        vars.insert("genre".to_string(), effective_genre);
        vars.insert("tone".to_string(), ctx.story.tone.clone());
        vars.insert("pacing".to_string(), ctx.story.pacing.clone());
        vars.insert("characters".to_string(), ctx.format_characters());
        // W3-B1: MemoryPack 注入提示词 — 如果存在
        // memory_pack，使用结构化记忆替代传统前文摘要 v0.8.0: 优先使用
        // memory_context（含路由分数和理由），回退到 memory_pack
        let previous_chapters_text = if let Some(ref mem_ctx) = ctx.memory.memory {
            if !mem_ctx.injected_memories.is_empty() {
                Self::format_memory_context(mem_ctx)
            } else if let Some(ref pack) = ctx.memory.memory_pack {
                Self::format_memory_pack_for_prompt(pack)
            } else {
                ctx.format_previous_chapters()
            }
        } else if let Some(ref pack) = ctx.memory.memory_pack {
            Self::format_memory_pack_for_prompt(pack)
        } else {
            ctx.format_previous_chapters()
        };
        vars.insert("previous_chapters".to_string(), previous_chapters_text);
        // LitSeg E1: 注入叙事结构上下文
        vars.insert(
            "narrative_structure".to_string(),
            ctx.format_narrative_structure(),
        );
        vars.insert(
            "current_content".to_string(),
            ctx.narrative
                .current_content
                .clone()
                .unwrap_or_else(|| "无".to_string()),
        );
        vars.insert("instruction".to_string(), task.input.clone());
        vars.insert(
            "world_rules".to_string(),
            ctx.world.world_rules.clone().unwrap_or_default(),
        );
        vars.insert(
            "scene_structure".to_string(),
            ctx.world.scene_structure.clone().unwrap_or_default(),
        );
        vars.insert(
            "outline_context".to_string(),
            ctx.narrative.outline_context.clone().unwrap_or_default(),
        );
        vars.insert(
            "scene_beats".to_string(),
            ctx.world.scene_structure.clone().unwrap_or_default(),
        );
        vars.insert(
            "must_cover".to_string(),
            ctx.narrative.outline_context.clone().unwrap_or_default(),
        );
        vars.insert(
            "story_description".to_string(),
            ctx.story.description.clone().unwrap_or_default(),
        );
        tokio::task::yield_now().await;

        emit_and_yield("正在渲染系统提示词...", 0.16);
        // v0.17.1: 通过 PromptRegistry 读取，支持用户在前端覆盖
        let writer_system_tpl = self.resolve_prompt("writer_system");
        let mut system_prompt = TemplateEngine::render_with_conditions(&writer_system_tpl, &vars);
        tokio::task::yield_now().await;

        // 注入写作策略约束
        emit_and_yield("正在注入写作策略约束...", 0.165);
        if let Some(ref ws) = strategy {
            let mut strategy_lines = Vec::new();

            if ws.run_mode == "fast" {
                strategy_lines.push("运行模式：快速生成。允许较快的叙事推进，注重效率。");
            } else if ws.run_mode == "polish" {
                strategy_lines
                    .push("运行模式：精修生成。注重文字质量，每句都需斟酌，允许较慢的推进速度。");
            }

            if ws.conflict_level >= 80 {
                strategy_lines
                    .push("冲突强度：极高。每 500 字至少设置一次冲突或张力，保持高度紧张感。");
            } else if ws.conflict_level >= 60 {
                strategy_lines.push("冲突强度：高。保持频繁的冲突和对抗，推动情节快速展开。");
            } else if ws.conflict_level >= 40 {
                strategy_lines.push("冲突强度：中等。适度安排冲突，兼顾人物发展和情节推进。");
            } else if ws.conflict_level >= 20 {
                strategy_lines.push("冲突强度：低。以人物内心和情感为主，减少外部冲突。");
            } else {
                strategy_lines.push("冲突强度：极低。以平和、抒情、描写为主，避免剧烈冲突。");
            }

            if ws.pace == "fast" {
                strategy_lines
                    .push("叙事节奏：快。减少环境描写和冗余叙述，增加动作和对话，快速推进情节。");
            } else if ws.pace == "slow" {
                strategy_lines.push("叙事节奏：慢。允许细腻的环境描写和心理刻画，注重氛围营造。");
            } else {
                strategy_lines.push("叙事节奏：均衡。动作与描写交替，保持适度的推进速度。");
            }

            if ws.ai_freedom == "low" {
                strategy_lines.push(
                    "AI 自由度：低。严格遵循已有设定和大纲，不得偏离世界观或人物设定，\
                     不得擅自引入新元素。",
                );
            } else if ws.ai_freedom == "high" {
                strategy_lines.push(
                    "AI 自由度：高。在保持整体方向一致的前提下，允许创新情节发展和意外转折。",
                );
            } else {
                strategy_lines
                    .push("AI 自由度：中。遵循核心设定，但在细节和情节展开上有一定发挥空间。");
            }

            if !strategy_lines.is_empty() {
                system_prompt.push_str("\n\n【写作策略约束】\n");
                for line in strategy_lines {
                    system_prompt.push_str(line);
                    system_prompt.push('\n');
                }
            }
        }
        tokio::task::yield_now().await;

        // v0.10.0: 注入体裁画像专家策略（所有版本）
        emit_and_yield("正在加载体裁画像策略...", 0.168);
        if let Some(ref genre_profile_id) = ctx.story.genre_profile_id {
            // v0.9.7: 从全局只读缓存中查找体裁画像，避免每次重新查库
            let profile = writer_genre_profiles(&self.app_handle)
                .await
                .ok()
                .and_then(|profiles| profiles.get(genre_profile_id).cloned());

            if let Some(profile) = profile {
                let mut lines = vec![format!(
                    "你正在创作的是 '{}'（{}）题材。请严格遵循以下体裁专家策略：",
                    profile.genre_name, profile.canonical_name
                )];
                if let Some(tone) = &profile.core_tone {
                    lines.push(format!("核心基调：{}", tone));
                }
                if let Some(pacing) = &profile.pacing_strategy {
                    lines.push(format!("节奏策略：\n{}", pacing));
                }
                if let Some(anti_patterns_json) = &profile.anti_patterns_json {
                    if let Ok(list) = serde_json::from_str::<Vec<String>>(anti_patterns_json) {
                        if !list.is_empty() {
                            lines.push(format!("应避免的反套路：\n- {}", list.join("\n- ")));
                        }
                    }
                }
                if let Some(reference_tables) = &profile.reference_tables_json {
                    lines.push(format!("元素参考表：\n{}", reference_tables));
                }
                if let Some(typical_structure) = &profile.typical_structure_json {
                    lines.push(format!("典型结构参考：\n{}", typical_structure));
                }
                system_prompt.push_str("\n\n【体裁画像策略】\n");
                system_prompt.push_str(&lines.join("\n"));
            }
        }
        tokio::task::yield_now().await;

        // 注入创作方法论扩展（仅专业版）
        if is_pro {
            emit_and_yield("正在加载创作方法论...", 0.17);
            if let Some(ref method_id) = ctx.world.methodology_id {
                use crate::creative_engine::methodology::{
                    MethodologyConfig, MethodologyEngine, MethodologyType,
                };
                let method_type = match method_id.as_str() {
                    "snowflake" => Some(MethodologyType::Snowflake),
                    "scene_structure" | "scene_beat" => Some(MethodologyType::SceneStructure),
                    "hero_journey" => Some(MethodologyType::HeroJourney),
                    "character_depth" => Some(MethodologyType::CharacterDepth),
                    "world_building" => Some(MethodologyType::HighDensityWorldBuilding),
                    _ => None,
                };
                if let Some(mt) = method_type {
                    let config = MethodologyConfig {
                        methodology_type: mt,
                        is_active: true,
                        current_step: ctx.world.methodology_step.clone(),
                        custom_params: serde_json::json!({}),
                    };
                    let extension = MethodologyEngine::build_prompt_extension(&config);
                    if !extension.is_empty() {
                        system_prompt.push_str("\n\n【创作方法论约束】\n");
                        system_prompt.push_str(&extension);
                    }
                }
            }
            tokio::task::yield_now().await;

            // 注入风格（混合优先，单一 DNA 回退，仅专业版）
            // v0.9.3: 优先使用 StoryContextBuilder 预计算的扩展，避免每个候选重复查库
            emit_and_yield("正在加载风格 DNA...", 0.175);
            if let Some(ref extension) = ctx.style.style_dna_extension {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(extension);
            } else if let Some(ref blend) = ctx.style.style_blend {
                // v0.9.7: 从全局只读缓存中批量查找风格 DNA，避免为每个 component
                // 单独创建 Repository 并查询。
                use crate::creative_engine::style::dna::StyleDNA;

                let dnas: Vec<StyleDNA> = match writer_style_dnas(&self.app_handle).await {
                    Ok(cache) => blend
                        .components
                        .iter()
                        .filter_map(|comp| {
                            cache.get(&comp.dna_id).and_then(|db_dna| {
                                serde_json::from_str::<StyleDNA>(&db_dna.dna_json).ok()
                            })
                        })
                        .collect(),
                    Err(e) => {
                        log::warn!(
                            "[build_writer_prompt] Failed to load style DNA cache: {}",
                            e
                        );
                        vec![]
                    }
                };
                if !dnas.is_empty() {
                    let extension = blend.to_prompt_extension(&dnas);
                    if !extension.is_empty() {
                        system_prompt.push_str("\n\n");
                        system_prompt.push_str(&extension);
                    }
                }
            } else if let Some(ref style_id) = ctx.style.style_dna_id {
                use crate::creative_engine::style::dna::StyleDNA;

                if let Ok(cache) = writer_style_dnas(&self.app_handle).await {
                    if let Some(db_dna) = cache.get(style_id) {
                        if let Ok(dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                            let extension = dna.to_prompt_extension();
                            if !extension.is_empty() {
                                system_prompt.push_str("\n\n");
                                system_prompt.push_str(&extension);
                            }
                        }
                    }
                }
            }
            tokio::task::yield_now().await;

            // 注入写作风格详细设定（所有用户可用）
            emit_and_yield("正在加载写作风格设定...", 0.177);
            let mut style_detail_lines = Vec::new();
            if let Some(ref name) = ctx.style.writing_style_name {
                style_detail_lines.push(format!("风格名称: {}", name));
            }
            if let Some(ref desc) = ctx.style.writing_style_description {
                style_detail_lines.push(format!("风格描述: {}", desc));
            }
            if let Some(ref vocab) = ctx.style.writing_style_vocabulary_level {
                style_detail_lines.push(format!("词汇层级: {}", vocab));
            }
            if let Some(ref sentence) = ctx.style.writing_style_sentence_structure {
                style_detail_lines.push(format!("句式结构: {}", sentence));
            }
            if let Some(ref rules) = ctx.style.writing_style_custom_rules {
                if !rules.is_empty() {
                    style_detail_lines.push("自定义规则:".to_string());
                    for rule in rules.lines() {
                        if !rule.trim().is_empty() {
                            style_detail_lines.push(format!("- {}", rule.trim()));
                        }
                    }
                }
            }
            if !style_detail_lines.is_empty() {
                system_prompt.push_str("\n\n【写作风格约束】\n");
                for line in style_detail_lines {
                    system_prompt.push_str(&line);
                    system_prompt.push('\n');
                }
            }
            tokio::task::yield_now().await;

            // 注入作品简介
            if let Some(ref desc) = ctx.story.description {
                if !desc.is_empty() {
                    system_prompt.push_str("\n\n【作品简介】\n");
                    system_prompt.push_str(desc);
                    system_prompt.push('\n');
                }
            }
            tokio::task::yield_now().await;

            // 注入个性化偏好（自适应学习，仅专业版）
            // v0.9.3: 优先使用 StoryContextBuilder 预计算的扩展，避免每个候选重复查库
            emit_and_yield("正在加载个性化偏好...", 0.18);
            if let Some(ref extension) = ctx.story.personalizer_extension {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(extension);
            } else {
                use crate::{creative_engine::adaptive::PromptPersonalizer, db::DbPool};

                let pool = self.app_handle.state::<DbPool>().inner().clone();
                let story_id = ctx.story.story_id.clone();
                let extension = match PromptPersonalizer::new(pool)
                    .build_prompt_extension(&story_id)
                    .await
                {
                    Ok(ext) => Some(ext),
                    Err(e) => {
                        log::warn!(
                            "[build_writer_prompt] Failed to build personalizer extension: {}",
                            e
                        );
                        None
                    }
                };

                if let Some(extension) = extension {
                    if !extension.is_empty() {
                        system_prompt.push_str("\n\n");
                        system_prompt.push_str(&extension);
                    }
                }
            }
            tokio::task::yield_now().await;
        }

        // v0.7.8: 注入风格指纹（从参考文本提取的量化风格约束）
        // 优先级：style_fingerprint > style_blend > style_dna_id > current_content
        // 实时提取 fingerprint 提供基于实际文本的量化约束（句长分布、N-gram
        // 白名单、锚点片段）， 比 StyleDNA
        // 的定性描述更精确，用于续写时严格保持语言风格一致。
        emit_and_yield("正在提取风格指纹...", 0.176);
        let fingerprint_text = if let Some(ref fingerprint) = ctx.style.style_fingerprint {
            Some(fingerprint.to_prompt_section())
        } else if let Some(ref content) = ctx.narrative.current_content {
            // 如果没有预计算的 fingerprint，从当前内容实时提取
            // v0.7.8: 过滤掉 PlanContext 截断前缀 "...(前N字已省略)\n"
            let cleaned = content.trim().trim_start_matches("...").trim_start();
            // 去掉 "(前xxx字已省略)" 前缀行
            let cleaned = if cleaned.starts_with('(') && cleaned.contains("已省略)") {
                cleaned
                    .split_once('\n')
                    .map(|(_, rest)| rest)
                    .unwrap_or(cleaned)
                    .trim_start()
            } else {
                cleaned
            };
            if cleaned.len() > 100 && cleaned != "无" {
                let fingerprint =
                    crate::creative_engine::style::fingerprint::StyleFingerprint::from_text(
                        cleaned,
                    );
                let section = fingerprint.to_prompt_section();
                if !section.is_empty() {
                    Some(section)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ref section) = fingerprint_text {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(section);
            log::info!(
                "[build_writer_prompt] Injected style fingerprint for story {}",
                ctx.story.story_id
            );
        }
        tokio::task::yield_now().await;

        // 注入 Canonical State（叙事阶段、伏笔、角色状态、活跃冲突）
        emit_and_yield("正在构建叙事状态快照...", 0.185);
        let snapshot = {
            use tauri::Manager;

            use crate::{canonical_state::CanonicalStateManager, db::DbPool};

            // v0.9.7: CanonicalStateManager 内部为同步聚合，将其整体移入 spawn_blocking
            let pool = self.app_handle.state::<DbPool>().inner().clone();
            let story_id = ctx.story.story_id.clone();
            tokio::task::spawn_blocking(move || {
                let cs_manager = CanonicalStateManager::new(pool);
                cs_manager.get_snapshot_sync(&story_id)
            })
            .await
            .map_err(|e| log::warn!("[build_writer_prompt] Canonical state task failed: {}", e))
            .ok()
            .and_then(|res| res.ok())
        };
        emit_and_yield("正在读取故事与场景数据...", 0.187);
        tokio::task::yield_now().await;

        if let Some(snapshot) = snapshot {
            emit_and_yield("正在注入叙事阶段指导...", 0.188);
            system_prompt.push_str("\n\n【叙事阶段指导】\n");
            system_prompt.push_str(&snapshot.narrative_phase.writer_guidance());
            system_prompt.push('\n');
            tokio::task::yield_now().await;

            if !snapshot.story_context.active_conflicts.is_empty() {
                emit_and_yield("正在注入活跃冲突信息...", 0.189);
                system_prompt.push_str("\n【当前活跃冲突】\n");
                for conflict in &snapshot.story_context.active_conflicts {
                    system_prompt.push_str(&format!(
                        "- {}: 涉及 {}, 赌注: {}\n",
                        conflict.conflict_type,
                        conflict.parties.join(", "),
                        conflict.stakes
                    ));
                }
                tokio::task::yield_now().await;
            }

            if !snapshot.story_context.pending_payoffs.is_empty() {
                emit_and_yield("正在注入待回收伏笔...", 0.19);
                system_prompt.push_str("\n【待回收伏笔】\n");
                for payoff in &snapshot.story_context.pending_payoffs {
                    system_prompt.push_str(&format!(
                        "- {}（重要度: {}）\n",
                        payoff.content, payoff.importance
                    ));
                }
                tokio::task::yield_now().await;
            }

            if !snapshot.story_context.overdue_payoffs.is_empty() {
                emit_and_yield("正在注入逾期伏笔警告...", 0.191);
                system_prompt.push_str("\n【⚠️ 逾期伏笔——请在续写中优先回收】\n");
                for payoff in &snapshot.story_context.overdue_payoffs {
                    system_prompt.push_str(&format!(
                        "- {}（重要度: {}）\n",
                        payoff.content, payoff.importance
                    ));
                }
                tokio::task::yield_now().await;
            }

            if !snapshot.character_states.is_empty() {
                emit_and_yield("正在注入角色当前状态...", 0.192);
                system_prompt.push_str("\n【角色当前状态】\n");
                for cs in &snapshot.character_states {
                    let mut parts = vec![format!("{}:", cs.name)];
                    if let Some(ref loc) = cs.current_location {
                        parts.push(format!("位置: {}", loc));
                    }
                    if let Some(ref emo) = cs.current_emotion {
                        parts.push(format!("情绪: {}", emo));
                    }
                    if let Some(ref goal) = cs.active_goal {
                        parts.push(format!("目标: {}", goal));
                    }
                    if !cs.secrets_known.is_empty() {
                        parts.push(format!("已知秘密: {}", cs.secrets_known.join(", ")));
                    }
                    if !cs.secrets_unknown.is_empty() {
                        parts.push(format!("未知秘密: {}", cs.secrets_unknown.join(", ")));
                    }
                    parts.push(format!("弧光进度: {:.0}%", cs.arc_progress * 100.0));
                    system_prompt.push_str(&format!("- {}\n", parts.join(" ")));
                }
                tokio::task::yield_now().await;
            }
        }

        emit_and_yield("正在组装最终提示词...", 0.195);

        // v0.17.1: 注入智能后台预访谈推断出的中文叙事四元组（含桥段卡），
        // 让 Writer 在生成时同时考虑「主情绪 + 高压关系 + 冲突场 + 引擎 + 桥段卡」。
        // 数据来自 PlanExecutor::execute_writer 注入的 `narrative_quartet` 参数。
        if let Some(quartet) = task.parameters.get("narrative_quartet") {
            if let Some(section) = render_narrative_quartet_section(quartet) {
                system_prompt.push_str("\n\n");
                system_prompt.push_str(&section);
            }
        }

        // v0.17.1: Living Author Guard —— 在最终 prompt 组装后，扫描并替换在世作者
        // 姓名为「具备相同手工艺特征的写作风格」+ 手工艺滑块描述。
        // 目的：1) 不直接对在世作者点名模仿；2) 把模仿型描述拆解为可量化维度。
        let guard_outcome =
            crate::creative_engine::style::living_author_guard::sanitize_style_brief(
                &system_prompt,
            );
        system_prompt = guard_outcome.sanitized;
        if guard_outcome.require_craft_sliders {
            log::info!(
                "[build_writer_prompt] living_author_guard 命中 {} 位作者，已替换并追加手工艺滑块",
                guard_outcome.removed_authors.len()
            );
            system_prompt.push_str("\n\n");
            system_prompt.push_str(
                &crate::creative_engine::style::living_author_guard::render_craft_sliders(
                    &crate::creative_engine::style::living_author_guard::default_craft_sliders(),
                ),
            );
        }

        let user_prompt = if has_selection {
            vars.insert(
                "selected_text".to_string(),
                ctx.narrative.selected_text.clone().unwrap_or_default(),
            );
            // v0.17.1: 通过 PromptRegistry 读取
            let tpl = self.resolve_prompt("writer_rewrite");
            TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            // v0.17.1: 通过 PromptRegistry 读取
            let tpl = self.resolve_prompt("writer_continue");
            TemplateEngine::render_with_conditions(&tpl, &vars)
        };
        tokio::task::yield_now().await;

        emit_and_yield("写作提示词构建完成", 0.20);
        format!("{}\n\n{}", system_prompt, user_prompt)
    }

    fn build_inspector_prompt(&self, task: &AgentTask) -> String {
        use std::collections::HashMap;

        use crate::prompts::TemplateEngine;

        let ctx = &task.context;
        let mut vars = HashMap::new();
        vars.insert("story_title".to_string(), ctx.story.story_title.clone());
        vars.insert("genre".to_string(), ctx.story.genre.clone());
        vars.insert("characters".to_string(), ctx.format_characters());
        vars.insert("content".to_string(), task.input.clone());

        let inspector_tpl = self.resolve_prompt("inspector_system");
        let system_prompt = TemplateEngine::render_with_conditions(&inspector_tpl, &vars);

        format!("{}\n\n【待检查内容】\n{}", system_prompt, task.input)
    }

    fn build_outline_prompt(&self, task: &AgentTask) -> String {
        use std::collections::HashMap;

        use crate::prompts::TemplateEngine;

        let ctx = &task.context;
        let mut vars = HashMap::new();
        vars.insert("premise".to_string(), task.input.clone());
        vars.insert("characters".to_string(), ctx.format_characters());

        let outline_tpl = self.resolve_prompt("outline_planner");
        TemplateEngine::render_with_conditions(&outline_tpl, &vars)
    }

    fn build_style_prompt(&self, task: &AgentTask) -> String {
        format!(
            r#"【参考文风样例】
{}

【需要改写的文本】
{}

请模仿参考文风的语言特点（词汇选择、句式结构、修辞手法等），改写上述文本，保持原意但改变表达方式。"#,
            task.parameters
                .get("style_sample")
                .and_then(|v| v.as_str())
                .unwrap_or("无样例"),
            task.input
        )
    }

    fn build_plot_prompt(&self, task: &AgentTask) -> String {
        format!(
            r#"【故事内容】
{}

【分析要求】
1. 情节复杂度评估（简单/中等/复杂）
2. 主要情节线索梳理
3. 潜在的逻辑漏洞
4. 伏笔和回收情况
5. 高潮设置是否合理
6. 改进建议"#,
            task.input
        )
    }

    // ==================== 辅助方法 ====================

    /// 将 Agent 阶段映射为统一的生成阶段
    ///
    /// 仅对参与创作流程的 Agent/阶段做映射，避免无关任务产生过多统一事件。
    fn map_agent_stage_to_generation_phase(
        agent_type: AgentType,
        stage: AgentStage,
    ) -> Option<GenerationPhase> {
        match (agent_type, stage) {
            (_, AgentStage::Failed) => Some(GenerationPhase::Error),
            (AgentType::Writer, AgentStage::Started)
            | (AgentType::Writer, AgentStage::Thinking) => Some(GenerationPhase::PreparingContext),
            (AgentType::Writer, AgentStage::Generating) => {
                Some(GenerationPhase::GeneratingCandidates)
            }
            (AgentType::Writer, AgentStage::Reviewing) => Some(GenerationPhase::Inspecting),
            (AgentType::Writer, AgentStage::Completed) => Some(GenerationPhase::FinalOutput),
            (AgentType::Inspector, AgentStage::Started)
            | (AgentType::Inspector, AgentStage::Thinking)
            | (AgentType::Inspector, AgentStage::Generating)
            | (AgentType::Inspector, AgentStage::Reviewing) => Some(GenerationPhase::Inspecting),
            (AgentType::Inspector, AgentStage::Completed) => Some(GenerationPhase::FinalOutput),
            _ => None,
        }
    }

    fn emit_event(
        &self,
        task_id: &str,
        agent_type: AgentType,
        stage: AgentStage,
        message: &str,
        progress: f32,
    ) {
        self.emit_event_with_request_id(task_id, agent_type, stage, message, progress, None);
    }

    fn emit_event_with_request_id(
        &self,
        task_id: &str,
        agent_type: AgentType,
        stage: AgentStage,
        message: &str,
        progress: f32,
        request_id: Option<String>,
    ) {
        let event = AgentEvent {
            task_id: task_id.to_string(),
            agent_type: agent_type.name().to_string(),
            stage,
            message: message.to_string(),
            progress,
            request_id,
        };

        let _ = self
            .app_handle
            .emit(&format!("agent-event-{}", task_id), event.clone());
        // 同时发送全局事件，让前端不需要知道task_id也能监听
        let _ = self.app_handle.emit(
            "agent-stage-update",
            serde_json::json!({
                "agent_type": event.agent_type,
                "stage": format!("{:?}", stage),
                "message": event.message,
                "progress": event.progress,
                "request_id": event.request_id,
            }),
        );

        // C1: 同时发射统一生成状态事件，供新版前端聚合消费
        if let Some(phase) = Self::map_agent_stage_to_generation_phase(agent_type, stage) {
            let unified_message = format!("{}: {}", agent_type.name(), message);
            emit_generation_status(
                &self.app_handle,
                task_id,
                phase,
                progress,
                unified_message,
                event.request_id,
            );
        }
    }

    /// W3-B1: 将 MemoryPack 格式化为提示词可用的记忆上下文文本
    // v0.8.0: MemoryContext 格式化 — 支持带分数和理由的记忆注入
    fn format_memory_context(ctx: &super::MemoryContext) -> String {
        let mut parts = Vec::new();

        if !ctx.injected_memories.is_empty() {
            // Working Memory
            let working: Vec<_> = ctx
                .injected_memories
                .iter()
                .filter(|e| e.entry.layer == "working")
                .collect();
            if !working.is_empty() {
                parts.push("【工作记忆】".to_string());
                for scored in working {
                    let content_str = match &scored.entry.content {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let display = if content_str.chars().count() > 300 {
                        format!("{}...", content_str.chars().take(300).collect::<String>())
                    } else {
                        content_str
                    };
                    parts.push(format!(
                        "- [{}] {} (相关度: {:.0}%, 理由: {})",
                        scored.entry.source, display, scored.relevance_score, scored.reason
                    ));
                }
            }

            // Episodic Memory
            let episodic: Vec<_> = ctx
                .injected_memories
                .iter()
                .filter(|e| e.entry.layer == "episodic")
                .collect();
            if !episodic.is_empty() {
                parts.push(String::new());
                parts.push("【情景记忆】".to_string());
                for scored in episodic {
                    let content_str = match &scored.entry.content {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let display = if content_str.chars().count() > 200 {
                        format!("{}...", content_str.chars().take(200).collect::<String>())
                    } else {
                        content_str
                    };
                    parts.push(format!(
                        "- [{}] {} (相关度: {:.0}%, 理由: {})",
                        scored.entry.source, display, scored.relevance_score, scored.reason
                    ));
                }
            }

            // Semantic Memory
            let semantic: Vec<_> = ctx
                .injected_memories
                .iter()
                .filter(|e| e.entry.layer == "semantic")
                .collect();
            if !semantic.is_empty() {
                parts.push(String::new());
                parts.push("【语义记忆】".to_string());
                for scored in semantic.iter().take(20) {
                    let content_str = match &scored.entry.content {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let display = if content_str.chars().count() > 200 {
                        format!("{}...", content_str.chars().take(200).collect::<String>())
                    } else {
                        content_str
                    };
                    parts.push(format!(
                        "- [{}] {} (相关度: {:.0}%, 理由: {})",
                        scored.entry.source, display, scored.relevance_score, scored.reason
                    ));
                }
            }
        }

        // 记忆一致性报告（如果存在）
        if let Some(ref report) = ctx.consistency_report {
            if !report.conflicts.is_empty() {
                parts.push(String::new());
                parts.push(format!(
                    "【记忆一致性警告】评分: {:.0}%",
                    report.memory_score * 100.0
                ));
                for conflict in &report.conflicts {
                    parts.push(format!("- {}", conflict));
                }
            }
        }

        if parts.is_empty() {
            "暂无记忆".to_string()
        } else {
            parts.join("\n")
        }
    }

    fn format_memory_pack_for_prompt(pack: &crate::memory::orchestrator::MemoryPack) -> String {
        let mut parts = Vec::new();

        // Working Memory（工作记忆：近章摘要 + 当前大纲 + 前文）
        if !pack.working_memory.is_empty() {
            parts.push("【工作记忆】".to_string());
            for entry in &pack.working_memory {
                let content_str = match &entry.content {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                // 截断过长的条目，避免提示词爆炸
                let display = if content_str.chars().count() > 300 {
                    format!("{}...", content_str.chars().take(300).collect::<String>())
                } else {
                    content_str
                };
                parts.push(format!("- [{}] {}", entry.source, display));
            }
        }

        // Episodic Memory（情景记忆：状态变更、关系变化）
        if !pack.episodic_memory.is_empty() {
            parts.push(String::new());
            parts.push("【情景记忆】".to_string());
            for entry in &pack.episodic_memory {
                let content_str = match &entry.content {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let display = if content_str.chars().count() > 200 {
                    format!("{}...", content_str.chars().take(200).collect::<String>())
                } else {
                    content_str
                };
                parts.push(format!("- [{}] {}", entry.source, display));
            }
        }

        // Semantic Memory（语义记忆：长期事实、世界规则）
        let semantic_items: Vec<_> = pack
            .semantic_memory
            .iter()
            .filter(|item| item.category != "timeline") // timeline 太冗长，过滤
            .collect();
        if !semantic_items.is_empty() {
            parts.push(String::new());
            parts.push("【语义记忆】".to_string());
            for item in semantic_items.iter().take(20) {
                if let Some(ref value) = item.value {
                    let subject = item.subject.as_deref().unwrap_or("未知");
                    parts.push(format!("- {}（{}）: {}", subject, item.category, value));
                }
            }
        }

        // Active Constraints（活跃约束：世界规则、开放循环）
        if !pack.active_constraints.is_empty() {
            parts.push(String::new());
            parts.push("【活跃约束】".to_string());
            for constraint in &pack.active_constraints {
                if let Some(ref value) = constraint.value {
                    parts.push(format!("- {}: {}", constraint.category, value));
                }
            }
        }

        // Warnings（记忆冲突警告）
        if !pack.warnings.is_empty() {
            parts.push(String::new());
            parts.push("【记忆警告】".to_string());
            for warning in &pack.warnings {
                parts.push(format!(
                    "- {}（{} 项冲突，示例: {:?}）",
                    warning.warning_type,
                    warning.count,
                    warning
                        .sample
                        .iter()
                        .map(|s| s.value.clone().unwrap_or_default())
                        .collect::<Vec<_>>()
                ));
            }
        }

        // Stats（记忆统计，用于调试，不注入提示词）
        // 仅当存在异常时附加一行
        if pack.stats.conflicts > 0 || pack.stats.filtered > 50 {
            parts.push(String::new());
            parts.push(format!(
                "[记忆统计] 总计: {}, 工作: {}, 情景: {}, 语义: {}, 过滤: {}, 冲突: {}",
                pack.stats.total,
                pack.stats.working_total,
                pack.stats.episodic_total,
                pack.stats.semantic_total,
                pack.stats.filtered,
                pack.stats.conflicts
            ));
        }

        if parts.is_empty() {
            "暂无记忆".to_string()
        } else {
            parts.join("\n")
        }
    }

    fn calculate_quality_score(&self, content: &str) -> f32 {
        // 简单的启发式评分
        let length_score = (content.len() as f32 / 500.0).min(1.0); // 长度
        let sentence_count = content.split(['。', '！', '？']).count() as f32;
        let variety_score = (sentence_count / 5.0).min(1.0); // 句子多样性

        (length_score * 0.4 + variety_score * 0.6).min(1.0)
    }

    fn parse_inspection_result(&self, content: &str) -> (f32, Vec<String>) {
        // 尝试 JSON 结构化解析
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(score_val) = json.get("score").or_else(|| json.get("总体评分")) {
                let score = match score_val {
                    serde_json::Value::Number(n) => n.as_f64().unwrap_or(60.0) as f32 / 100.0,
                    serde_json::Value::String(s) => s.trim().parse::<f32>().unwrap_or(60.0) / 100.0,
                    _ => 0.6,
                }
                .clamp(0.0, 1.0);

                let suggestions = json
                    .get("suggestions")
                    .or_else(|| json.get("改进建议"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                return (score, suggestions);
            }
        }

        // 回退：正则提取分数 (0-100 或 0.0-1.0)
        let score = {
            let re = regex::Regex::new(r"(?:总体?)?评分[:：]\s*(\d+(?:\.\d+)?)").ok();
            let extracted = re
                .as_ref()
                .and_then(|r| r.captures(content))
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<f32>().ok());

            match extracted {
                Some(v) if v > 1.0 => (v / 100.0).clamp(0.0, 1.0), // 0-100 分制
                Some(v) => v.clamp(0.0, 1.0),                      // 0-1 分制
                None => {
                    // 关键词回退
                    if content.contains("90") || content.contains("优秀") {
                        0.9
                    } else if content.contains("80") || content.contains("良好") {
                        0.8
                    } else if content.contains("70") {
                        0.7
                    } else {
                        0.6
                    }
                }
            }
        };

        // 提取建议：支持 "1. xxx"、"- xxx"、"* xxx"、"建议：xxx" 等格式
        let suggestions: Vec<String> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| {
                !l.is_empty()
                    && (l
                        .starts_with(|c: char| c.is_ascii_digit() && l.chars().nth(1) == Some('.'))
                        || l.starts_with("-")
                        || l.starts_with("*")
                        || l.contains("建议")
                        || l.contains("改进")
                        || l.contains("问题"))
            })
            .map(|l| {
                // 清理前缀
                l.trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c == '.' || c == '-' || c == '*' || c == ' '
                })
                .trim()
                .to_string()
            })
            .filter(|l| !l.is_empty() && l.len() > 5)
            .collect();

        (score, suggestions)
    }

    fn parse_plot_analysis(&self, content: &str) -> (f32, Vec<String>) {
        let score = if content.contains("复杂") || content.contains("优秀") {
            0.85
        } else if content.contains("中等") {
            0.7
        } else {
            0.6
        };

        let suggestions = content
            .lines()
            .filter(|l| l.contains("漏洞") || l.contains("建议"))
            .map(|l| l.to_string())
            .collect();

        (score, suggestions)
    }
}

fn parse_task_type(s: &str) -> Option<TaskType> {
    match s.to_lowercase().as_str() {
        "creative_writing" => Some(TaskType::CreativeWriting),
        "editing" => Some(TaskType::Editing),
        "analysis" => Some(TaskType::Analysis),
        "dialogue" => Some(TaskType::Dialogue),
        "summarization" => Some(TaskType::Summarization),
        "brainstorming" => Some(TaskType::Brainstorming),
        "proofreading" => Some(TaskType::Proofreading),
        "world_building" => Some(TaskType::WorldBuilding),
        "vision" => Some(TaskType::Vision),
        "image_generation" => Some(TaskType::ImageGeneration),
        _ => None,
    }
}

fn parse_complexity(s: &str) -> Option<Complexity> {
    match s.to_lowercase().as_str() {
        "low" => Some(Complexity::Low),
        "medium" => Some(Complexity::Medium),
        "high" => Some(Complexity::High),
        "critical" => Some(Complexity::Critical),
        _ => None,
    }
}

fn parse_priority(s: &str) -> Option<Priority> {
    match s.to_lowercase().as_str() {
        "low" => Some(Priority::Low),
        "medium" => Some(Priority::Medium),
        "high" => Some(Priority::High),
        _ => None,
    }
}

fn parse_constraint(s: &str) -> Option<RoutingConstraint> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "local_only" => Some(RoutingConstraint::LocalOnly),
        "platform_only" => Some(RoutingConstraint::PlatformOnly),
        _ => {
            if let Some(rest) = lower.strip_prefix("min_quality:") {
                let q = match rest.trim() {
                    "low" => crate::config::settings::QualityTier::Low,
                    "medium" => crate::config::settings::QualityTier::Medium,
                    "high" => crate::config::settings::QualityTier::High,
                    "ultra" => crate::config::settings::QualityTier::Ultra,
                    _ => return None,
                };
                return Some(RoutingConstraint::MinQuality(q));
            }
            if let Some(rest) = lower.strip_prefix("min_context:") {
                if let Ok(ctx) = rest.trim().parse::<u32>() {
                    return Some(RoutingConstraint::MinContext(ctx));
                }
            }
            if let Some(rest) = lower.strip_prefix("requires:") {
                let cap = match rest.trim() {
                    "chat" => crate::config::settings::ModelCapability::Chat,
                    "completion" => crate::config::settings::ModelCapability::Completion,
                    "function_calling" => crate::config::settings::ModelCapability::FunctionCalling,
                    "json_mode" => crate::config::settings::ModelCapability::JsonMode,
                    "vision" => crate::config::settings::ModelCapability::Vision,
                    "long_context" => crate::config::settings::ModelCapability::LongContext,
                    _ => return None,
                };
                return Some(RoutingConstraint::Requires(cap));
            }
            None
        }
    }
}

impl Clone for AgentService {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            llm_service: LlmService::new(self.app_handle.clone()),
        }
    }
}

/// 获取所有可用的Agent类型
#[tauri::command]
pub fn get_available_agents() -> Vec<(AgentType, String, String)> {
    vec![
        (
            AgentType::Writer,
            AgentType::Writer.name().to_string(),
            AgentType::Writer.description().to_string(),
        ),
        (
            AgentType::Inspector,
            AgentType::Inspector.name().to_string(),
            AgentType::Inspector.description().to_string(),
        ),
        (
            AgentType::OutlinePlanner,
            AgentType::OutlinePlanner.name().to_string(),
            AgentType::OutlinePlanner.description().to_string(),
        ),
        (
            AgentType::StyleMimic,
            AgentType::StyleMimic.name().to_string(),
            AgentType::StyleMimic.description().to_string(),
        ),
        (
            AgentType::PlotAnalyzer,
            AgentType::PlotAnalyzer.name().to_string(),
            AgentType::PlotAnalyzer.description().to_string(),
        ),
    ]
}

/// 把 `narrative_quartet` JSON 渲染为 Writer prompt 注入片段（v0.17.1
/// 中文叙事增强）。
///
/// 该 JSON 结构由 `strategy::quartet_inference::serialize_quartet_for_prompt`
/// 生成， 包含主情绪 / 高压关系 / 冲突场 / 剧情引擎 /
/// 桥段卡五要素。本函数用中文标题 与重点标记把它渲染成 Writer
/// 可读的提示，避免重复传递大段 JSON。
fn render_narrative_quartet_section(quartet: &serde_json::Value) -> Option<String> {
    let obj = quartet.as_object()?;
    if obj.is_empty() {
        return None;
    }
    let mut s = String::new();
    s.push_str("【中文叙事四件套（智能后台推断，请在续写中体现）】\n");

    if let Some(payoff) = obj.get("emotional_payoff").and_then(|v| v.as_str()) {
        s.push_str(&format!("• 主情绪 / 读者爽点承诺：{}\n", payoff));
    }
    if let Some(arena) = obj.get("conflict_arena").and_then(|v| v.as_str()) {
        s.push_str(&format!(
            "• 冲突场（推荐安排关键场面发生于此）：{}\n",
            arena
        ));
    }

    if let Some(rel) = obj.get("pressure_relationship").and_then(|v| v.as_object()) {
        let name = rel.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let pressure = rel
            .get("pressure_source")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        s.push_str(&format!(
            "• 高压关系：{}（自带压力来源 — {}）\n",
            name, pressure
        ));
    }

    if let Some(engines) = obj.get("story_engines").and_then(|v| v.as_array()) {
        if !engines.is_empty() {
            s.push_str("• 剧情引擎（请正交组合这些叙事动力）：\n");
            for e in engines {
                let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let payoff = e.get("payoff").and_then(|v| v.as_str()).unwrap_or("");
                let best = e.get("best_payoff").and_then(|v| v.as_str()).unwrap_or("");
                let avoid = e.get("avoid").and_then(|v| v.as_str()).unwrap_or("");
                s.push_str(&format!(
                    "    - {}：{} | 最佳收束：{} | 反例：{}\n",
                    name, payoff, best, avoid
                ));
            }
        }
    }

    if let Some(cards) = obj.get("beat_cards").and_then(|v| v.as_array()) {
        if !cards.is_empty() {
            s.push_str("• 桥段卡（建议借用其叙事功能，但用本作设定重写）：\n");
            for c in cards {
                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let func = c.get("function").and_then(|v| v.as_str()).unwrap_or("");
                let remix = c.get("remix_hint").and_then(|v| v.as_str()).unwrap_or("");
                let avoid = c.get("avoid").and_then(|v| v.as_str()).unwrap_or("");
                s.push_str(&format!(
                    "    - {}：{} | 重构提示：{} | 反例：{}\n",
                    name, func, remix, avoid
                ));
            }
        }
    }

    s.push_str(
        "\n注意：以上四件套是「叙事方向参考」，不是模板。请用本作角色、场景、设定重新组织，\
         避免直接套用名称或情节链。\n",
    );

    Some(s)
}

#[cfg(test)]
mod narrative_quartet_render_tests {
    use super::*;

    #[test]
    fn empty_object_returns_none() {
        let v = serde_json::json!({});
        assert!(render_narrative_quartet_section(&v).is_none());
    }

    #[test]
    fn null_returns_none() {
        let v = serde_json::Value::Null;
        assert!(render_narrative_quartet_section(&v).is_none());
    }

    #[test]
    fn full_quartet_renders_with_chinese_labels() {
        let v = serde_json::json!({
            "emotional_payoff": "燃",
            "conflict_arena": "宗门大比",
            "pressure_relationship": {
                "name": "师徒宗门",
                "pressure_source": "等级忠诚背叛禁忌技艺"
            },
            "story_engines": [
                {
                    "name": "成长阶梯引擎",
                    "payoff": "升级可见",
                    "best_payoff": "前后差距",
                    "avoid": "抽象升级"
                }
            ],
            "beat_cards": [
                {
                    "name": "规则破解式",
                    "function": "理解规则漏洞反向获胜",
                    "remix_hint": "可叠加双层反转",
                    "avoid": "神预知"
                }
            ]
        });
        let out = render_narrative_quartet_section(&v).expect("expected section");
        assert!(out.contains("中文叙事四件套"));
        assert!(out.contains("主情绪"));
        assert!(out.contains("燃"));
        assert!(out.contains("宗门大比"));
        assert!(out.contains("师徒宗门"));
        assert!(out.contains("成长阶梯引擎"));
        assert!(out.contains("规则破解式"));
        // 强调"重写、不套用"原则
        assert!(out.contains("不是模板"));
    }
}
