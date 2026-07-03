#![allow(dead_code)]
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};

// W4-B3: SQLite-backed config storage
use crate::error::AppError;

/// 启动级配置 —— 保留在 config.json 中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<String>,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            log_level: default_log_level(),
        }
    }
}

impl BootstrapConfig {
    pub fn load(config_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = config_dir.join("config.json");
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            // 尝试解析为 BootstrapConfig；如果失败（旧格式完整 AppConfig），返回默认
            match serde_json::from_str::<BootstrapConfig>(&content) {
                Ok(cfg) => Ok(cfg),
                Err(e) => {
                    log::info!(
                        "[BootstrapConfig] config.json is in legacy format ({}), using defaults",
                        e
                    );
                    Ok(BootstrapConfig::default())
                }
            }
        } else {
            Ok(BootstrapConfig::default())
        }
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(config_dir)?;
        let path = config_dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

/// W4-B8: 从环境变量读取默认 API base URL，避免硬编码内网地址
fn env_or_default(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

/// 默认 LLM 请求超时（秒）。
/// v0.13.4: 从 300 秒降至 240 秒，确保后端能在前端 300 秒超时前返回结果，
/// 避免前端已超时但后端仍在重试的错配。需要更长时间的用户可在模型配置中
/// 手动调高单个 profile 的 timeout_seconds。
///
/// v0.24.8: 默认从 120 上调至 300 秒，适配本地大模型首次加载/慢生成场景。
pub const DEFAULT_LLM_TIMEOUT_SECONDS: u64 = 300;

/// 判断 URL 是否指向本地/局域网地址（localhost / 127.0.0.1 / 私有网段）。
pub fn is_private_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    if lower.contains("localhost") || lower.contains("127.0.0.1") || lower.contains("::1") {
        return true;
    }
    let host = lower
        .split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next());
    if let Some(host) = host {
        if host.starts_with("10.") || host.starts_with("192.168.") {
            return true;
        }
        if let Some(seg) = host.split('.').nth(1).and_then(|s| s.parse::<u8>().ok()) {
            if host.starts_with("172.") && (16..=31).contains(&seg) {
                return true;
            }
        }
    }
    false
}

/// Agent 显示名称（默认映射初始化用）
fn agent_display_name(agent_id: &str) -> String {
    match agent_id {
        "writer" => "写作助手".to_string(),
        "inspector" => "质检员".to_string(),
        "outline_planner" => "大纲规划师".to_string(),
        "style_mimic" => "风格模仿师".to_string(),
        "plot_analyzer" => "情节分析师".to_string(),
        _ => agent_id.to_string(),
    }
}

/// Agent 描述（默认映射初始化用）
fn agent_description(agent_id: &str) -> Option<String> {
    match agent_id {
        "writer" => Some("负责章节生成、改写".to_string()),
        "inspector" => Some("负责内容质量检查".to_string()),
        "outline_planner" => Some("负责故事大纲设计".to_string()),
        "style_mimic" => Some("负责文风分析与模仿".to_string()),
        "plot_analyzer" => Some("负责情节复杂度分析".to_string()),
        _ => None,
    }
}

/// 语言模型配置（向后兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMapping {
    pub agent_id: String,
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multimodal_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// v0.11.0: 任务类型覆盖（agent 默认策略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// v0.11.0: 任务复杂度覆盖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
    /// v0.11.0: 成本优先级覆盖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_priority: Option<String>,
    /// v0.11.0: 速度优先级覆盖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<String>,
    /// v0.11.0: 约束标签列表（如 "local_only", "min_quality:high"）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    /// v0.15.0: 强制锁定此模型，网关不会根据算力档案替换
    /// 默认 false（用户偏好，网关可根据算力覆盖）
    #[serde(default)]
    pub force_locked: bool,
}

/// 写作策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStrategy {
    pub run_mode: String,
    #[serde(default = "default_conflict_level")]
    pub conflict_level: i32,
    pub pace: String,
    pub ai_freedom: String,
}

fn default_conflict_level() -> i32 {
    50
}

impl Default for WritingStrategy {
    fn default() -> Self {
        Self {
            run_mode: "balanced".to_string(),
            conflict_level: 60,
            pace: "balanced".to_string(),
            ai_freedom: "medium".to_string(),
        }
    }
}

/// 用户级运行时配置。
///
/// 持久化策略（W4-B3）：
/// - 主源：SQLite `app_settings` 表的 `app_config` 行（`save_to_db` /
///   `load_from_db`）。
/// - 兼容：`load` 会回退读取旧版 `config.json` 并自动迁移到 SQLite；`save`
///   只写入 SQLite。
/// - `config.json` 仅保留启动级字段（`BootstrapConfig`：db_path /
///   log_level），不再存放用户级设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub llm: LlmConfig,
    #[serde(default)]
    pub llm_profiles: HashMap<String, LlmProfile>,
    #[serde(default)]
    pub embedding_profiles: HashMap<String, EmbeddingProfile>,
    #[serde(default)]
    pub active_llm_profile: Option<String>,
    #[serde(default)]
    pub active_embedding_profile: Option<String>,
    /// v0.23.66: 创作模型 — 正文生成、Writer、改写
    #[serde(default)]
    pub creative_model_id: Option<String>,
    /// v0.23.66: 工具模型 — Call 1 路由、探测、意图识别、JSON 提取
    #[serde(default)]
    pub tool_model_id: Option<String>,
    /// v0.23.66: 后台任务模型 — BGP 审计/入库/洞察、Genesis 后台流水线
    #[serde(default)]
    pub background_model_id: Option<String>,
    /// v0.23.66: 创世/首章温度 — 用于生成全新故事概念和第一章（推荐 1.0-1.3）
    #[serde(default)]
    pub creative_temperature: Option<f32>,
    /// v0.23.66: 续写温度 — 用于已有内容基础上的续写/改写（推荐 0.7-0.9）
    #[serde(default)]
    pub continuation_temperature: Option<f32>,
    /// v0.23.66: 工具温度 — 用于路由合成/Call1/JSON提取等精确任务（推荐
    /// 0.2-0.4）
    #[serde(default)]
    pub tool_temperature: Option<f32>,
    #[serde(default)]
    pub agent_mappings: HashMap<String, AgentMapping>,
    /// 拆书分析 LLM 并发数（默认 3，本地模型可调大）
    #[serde(default = "default_concurrency")]
    pub book_deconstruction_concurrency: usize,
    /// AgentOrchestrator 质检改写阈值（默认 0.75）
    #[serde(default = "default_rewrite_threshold")]
    pub rewrite_threshold: f32,
    /// AgentOrchestrator 最大反馈循环次数（默认 2）
    #[serde(default = "default_max_feedback_loops")]
    pub max_feedback_loops: u32,
    /// 风格评分在最终综合分中的权重（0-1，默认 0.5）
    #[serde(default = "default_style_weight")]
    pub style_weight: f32,
    /// 叙事评分在最终综合分中的权重（0-1，默认 0.5）
    #[serde(default = "default_narrative_weight")]
    pub narrative_weight: f32,
    /// 综合分数达到该阈值时跳过改写闭环，直接返回结果（默认 0.90）
    #[serde(default = "default_skip_rewrite_threshold")]
    pub skip_rewrite_threshold: f32,
    /// 是否在改写闭环中保留历史版本（默认 true）
    #[serde(default = "default_keep_revision_history")]
    pub keep_revision_history: bool,
    /// Genesis 向导第一章目标字数（默认 2000）
    #[serde(default = "default_genesis_first_chapter_word_count_target")]
    pub genesis_first_chapter_word_count_target: i32,
    /// 创作工作流 Inspector 通过阈值（默认 0.75）
    #[serde(default = "default_creation_workflow_review_threshold")]
    pub creation_workflow_review_threshold: f32,
    /// 创作工作流最大迭代次数（默认 2）
    #[serde(default = "default_creation_workflow_max_iterations")]
    pub creation_workflow_max_iterations: u32,
    /// 候选生成阶段单个远程候选的 LLM 超时（秒，默认 120）
    #[serde(default = "default_candidate_timeout_seconds")]
    pub candidate_timeout_seconds: u64,
    /// 候选生成阶段单个本地候选的 LLM 超时（秒，默认 60）
    #[serde(default = "default_candidate_timeout_local_seconds")]
    pub candidate_timeout_local_seconds: u64,
    /// 候选生成阶段单个候选的最大重试次数（默认 0）
    ///
    /// 候选阶段本身已生成多个版本，单个候选失败应快速跳过，避免超时叠加。
    #[serde(default = "default_candidate_max_retries")]
    pub candidate_max_retries: u32,
    /// 候选生成阶段候选数量（默认 1，远端模型可在 1–2 之间配置）
    #[serde(default = "default_candidate_count")]
    pub candidate_count: u32,
    /// 上下文构建时使用模型上下文窗口的比例（默认 0.8，保留 20% 给输出与开销）
    #[serde(default = "default_context_budget_ratio")]
    pub context_budget_ratio: f32,
    /// 本地模型 Writer 全局并发数（默认 1）
    #[serde(default = "default_writer_local_concurrency")]
    pub writer_local_concurrency: usize,
    /// 远端模型 Writer 全局并发数（默认 2）
    #[serde(default = "default_writer_remote_concurrency")]
    pub writer_remote_concurrency: usize,
    /// 本地模型是否在候选阶段串行生成以避免服务端排队（默认 false）
    ///
    /// 串行会导致候选 1 阻塞候选 2，一旦候选 1 挂起，整个阶段无进展。
    /// 默认并行，配合更短的本地超时（60s）避免排队影响。
    /// v0.11.8: 该字段已弃用，候选阶段始终并行；保留仅用于兼容旧配置。
    #[serde(default = "default_candidate_local_sequential")]
    pub candidate_local_sequential: bool,
    /// 通用 UI 设置
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: u64,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    /// 隐私设置
    #[serde(default = "default_share_usage_data")]
    pub share_usage_data: bool,
    #[serde(default)]
    pub writing_strategy: WritingStrategy,
    /// v0.14.3: AI 生成模式（auto/time_sliced/fast/full/tri_shot）
    /// - auto: 场景智能路由（续写 TimeSliced，重写 Full）
    /// - time_sliced: 强制分时模式（最快，单次 LLM）
    /// - fast: 强制 Fast 模式（单次 LLM + 风格技能）
    /// - full: 强制 Full 模式（Writer + Inspector + Rewrite 闭环）
    /// - tri_shot: v0.23 三击模式（弹性 2~3 次
    ///   LLM：合成→精修→生成，质检/改写下沉后台）
    #[serde(default = "default_generation_mode")]
    pub generation_mode: String,
    /// v0.23 TriShot BGP-2：后台自动改写的严重度阈值（"high" / "medium"）。
    /// 仅当审计问题严重度 ≥ 此阈值时后台 agent
    /// 自动改写并替换正文（写修订历史可撤销）；
    /// 低于阈值仅生成修订建议供用户审阅。默认
    /// "high"（保守，只自动改逻辑硬伤）。
    #[serde(default = "default_auto_rewrite_severity_threshold")]
    pub auto_rewrite_severity_threshold: String,
    /// v0.15.5: 超时配置（可从前端设置调整，无需重新编译）
    #[serde(default = "default_llm_connect_timeout")]
    pub llm_connect_timeout_secs: u64,
    #[serde(default = "default_smart_execute_timeout")]
    pub smart_execute_total_timeout_secs: u64,
    #[serde(default = "default_executor_step_timeout")]
    pub executor_step_timeout_secs: u64,
    #[serde(default = "default_frontend_timeout")]
    pub frontend_timeout_secs: u64,
    #[serde(default = "default_llm_first_chunk_timeout")]
    pub llm_first_chunk_timeout_secs: u64,
    /// v0.16.0: Writer 系统提示词覆盖（空则用默认）
    #[serde(default)]
    pub writer_system_prompt_override: String,
    /// v0.16.0: 模型探测提示词覆盖（空则用默认）
    #[serde(default)]
    pub probe_prompt_override: String,
    /// OAuth 客户端配置
    #[serde(default)]
    pub auth_clients: Option<HashMap<String, crate::auth::OAuthClientConfig>>,
}

fn default_generation_mode() -> String {
    "auto".to_string()
}

/// v0.23 TriShot BGP-2：后台自动改写严重度阈值默认值。
fn default_auto_rewrite_severity_threshold() -> String {
    "high".to_string()
}

fn default_llm_connect_timeout() -> u64 {
    60
}
fn default_smart_execute_timeout() -> u64 {
    600
}
fn default_executor_step_timeout() -> u64 {
    300
}
fn default_frontend_timeout() -> u64 {
    600
}
fn default_llm_first_chunk_timeout() -> u64 {
    180
}

fn default_rewrite_threshold() -> f32 {
    0.75
}

fn default_max_feedback_loops() -> u32 {
    2
}

fn default_concurrency() -> usize {
    3
}

fn default_style_weight() -> f32 {
    0.5
}

fn default_narrative_weight() -> f32 {
    0.5
}

fn default_skip_rewrite_threshold() -> f32 {
    0.90
}

fn default_keep_revision_history() -> bool {
    true
}

fn default_genesis_first_chapter_word_count_target() -> i32 {
    2000
}

fn default_creation_workflow_review_threshold() -> f32 {
    0.75
}

fn default_creation_workflow_max_iterations() -> u32 {
    2
}

fn default_candidate_timeout_seconds() -> u64 {
    180
}

fn default_candidate_timeout_local_seconds() -> u64 {
    120
}

fn default_candidate_max_retries() -> u32 {
    0
}

fn default_candidate_count() -> u32 {
    1
}

pub fn default_context_budget_ratio() -> f32 {
    0.8
}

fn default_writer_local_concurrency() -> usize {
    1
}

fn default_writer_remote_concurrency() -> usize {
    2
}

fn default_candidate_local_sequential() -> bool {
    false
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "zh-CN".to_string()
}

fn default_auto_save() -> bool {
    true
}

fn default_auto_save_interval() -> u64 {
    30
}

fn default_font_size() -> u32 {
    16
}

fn default_line_height() -> f32 {
    1.6
}

fn default_share_usage_data() -> bool {
    false
}

/// 语言模型配置（向后兼容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub api_base: Option<String>,
    pub max_tokens: i32,
    pub temperature: f32,
}

/// temperature 序列化/反序列化规范化：限制到2位小数，避免浮点精度噪声
mod temperature_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(temp: &f32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let normalized = ((temp * 100.0).round() / 100.0).clamp(0.0, 2.0);
        // 通过字符串 round-trip 确保序列化输出为干净的 2 位小数，
        // 避免 f32 -> f64 精度扩展导致 serde_json 输出 0.8899999856948853 这类噪声
        let s = format!("{:.2}", normalized);
        let clean: f64 = s.parse().unwrap();
        serializer.serialize_f64(clean)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let temp = f32::deserialize(deserializer)?;
        Ok(((temp * 100.0).round() / 100.0).clamp(0.0, 2.0))
    }
}

/// LLM 模型配置档案（chat / multimodal / image 统一）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: LlmProvider,
    /// 模型来源 — 决定配额和用量统计策略
    #[serde(default)]
    pub model_source: ModelSource,
    pub model: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    /// 是否本地/局域网模型（影响候选数、并发数、超时策略）。
    /// 用户可显式标记；未标记时由 provider / api_base / model_source 自动推断。
    #[serde(default)]
    pub is_local_model: bool,
    pub max_tokens: i32,
    #[serde(with = "temperature_serde")]
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    pub timeout_seconds: u64,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub kind: ModelKind,
    #[serde(default)]
    pub capabilities: Vec<ModelCapability>,
    /// 最大上下文长度（token），用于路由决策
    #[serde(default = "default_max_context_length")]
    pub max_context_length: u32,
    /// 质量等级
    #[serde(default)]
    pub quality_tier: QualityTier,
    /// 速度等级
    #[serde(default)]
    pub speed_tier: SpeedTier,
    /// 每 1K 输入 token 成本（可选，API 模型填写）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_1k_input: Option<f64>,
    /// 每 1K 输出 token 成本（可选，API 模型填写）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_1k_output: Option<f64>,
    /// 用户/系统标签，例如 ["fast", "local", "reasoning"]
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否支持 system prompt（部分本地模型或特定 API 不支持）
    #[serde(default = "default_true")]
    pub supports_system_prompt: bool,
    /// v0.23.61: 每模型独立系统提示词覆盖。
    /// 优先级：此字段 > AppConfig.writer_system_prompt_override >
    /// PromptRegistry "writer_system" 默认。 MN-Oblivion
    /// 等推理模型可能需要不同的 system prompt 来激活模型卡指令。
    #[serde(default)]
    pub system_prompt_override: Option<String>,
    /// 是否支持流式输出
    #[serde(default = "default_true")]
    pub supports_streaming: bool,
    /// 知识截止日期（可选，如 "2024-06"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_cutoff: Option<String>,
    /// Reasoning effort 等级（low / medium / high），仅 reasoning 模型有效
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl LlmProfile {
    /// v0.14.0: 根据 provider/model 名称关键字自动推断并补齐能力标签
    pub fn infer_capabilities(&mut self) -> bool {
        let model_lower = self.model.to_lowercase();
        let name_lower = self.name.to_lowercase();
        let combined = format!("{} {}", model_lower, name_lower);

        let mut changed = false;
        let mut add = |cap: ModelCapability| {
            if !self.capabilities.contains(&cap) {
                self.capabilities.push(cap);
                true
            } else {
                false
            }
        };

        // 所有现代模型默认支持 Chat / Completion / Streaming
        changed |= add(ModelCapability::Chat);
        changed |= add(ModelCapability::Completion);
        changed |= add(ModelCapability::Streaming);

        // Reasoning 模型关键字
        let reasoning_keywords = ["reasoning", "r1", "o1", "o3", "deepseek", "qwq", "think"];
        if reasoning_keywords.iter().any(|k| combined.contains(k)) {
            changed |= add(ModelCapability::Reasoning);
        }

        // Vision 模型关键字
        let vision_keywords = [
            "vision",
            "vl",
            "gemma-4",
            "llava",
            "qwen2.5-vl",
            "qwen-vl",
            "gpt-4o",
            "claude-3",
        ];
        if vision_keywords.iter().any(|k| combined.contains(k)) {
            changed |= add(ModelCapability::Vision);
            // 具备 Vision 时通常也支持多模态 Chat
            if self.kind == ModelKind::Chat {
                self.kind = ModelKind::Multimodal;
                changed = true;
            }
        }

        // LongContext 模型关键字
        let long_context_keywords = [
            "128k", "200k", "1m", "100k", "kimi", "claude-3", "gpt-4o", "qwen3",
        ];
        if long_context_keywords.iter().any(|k| combined.contains(k))
            || self.max_context_length >= 32768
        {
            changed |= add(ModelCapability::LongContext);
        }

        // JSON mode / Structured output（主流 API 模型）
        if matches!(
            self.provider,
            LlmProvider::OpenAI
                | LlmProvider::Anthropic
                | LlmProvider::DeepSeek
                | LlmProvider::Qwen
        ) {
            changed |= add(ModelCapability::JsonMode);
            changed |= add(ModelCapability::StructuredOutput);
        }

        // Tool use（主流 API 与部分本地模型）
        if matches!(
            self.provider,
            LlmProvider::OpenAI
                | LlmProvider::Anthropic
                | LlmProvider::DeepSeek
                | LlmProvider::Qwen
        ) || (self.provider == LlmProvider::Ollama && self.max_context_length >= 8192)
        {
            changed |= add(ModelCapability::ToolUse);
        }

        // FunctionCalling 兼容旧标签
        changed |= add(ModelCapability::FunctionCalling);

        changed
    }
}

fn default_true() -> bool {
    true
}

pub fn default_max_context_length() -> u32 {
    8192
}

/// 模型来源 — 决定配额和用量统计策略
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    /// 平台提供的模型（用量统计 + 配额检查）
    Platform,
    /// 本地模型（Ollama 等，无平台配额）
    Local,
    /// 用户自购的 API key（无平台配额）
    UserOwned,
}

impl Default for ModelSource {
    fn default() -> Self {
        ModelSource::Platform
    }
}

/// 支持的LLM提供商
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    OpenAI,
    Anthropic,
    Azure,
    Ollama,
    DeepSeek,
    Qwen,
    Custom,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::OpenAI => write!(f, "openai"),
            LlmProvider::Anthropic => write!(f, "anthropic"),
            LlmProvider::Azure => write!(f, "azure"),
            LlmProvider::Ollama => write!(f, "ollama"),
            LlmProvider::DeepSeek => write!(f, "deepseek"),
            LlmProvider::Qwen => write!(f, "qwen"),
            LlmProvider::Custom => write!(f, "custom"),
        }
    }
}

/// 模型能力
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Chat,
    Completion,
    FunctionCalling,
    JsonMode,
    Vision,
    LongContext,
    /// 推理链 / thinking / DeepSeek-R1 / o1 类
    Reasoning,
    /// 现代工具调用（兼容 FunctionCalling）
    ToolUse,
    /// 强制 JSON schema / response_format
    StructuredOutput,
    /// 支持流式输出
    Streaming,
    /// 明确标注为轻量快模型
    Fast,
    /// 文本嵌入（统一模型视图用）
    Embedding,
    /// 文生图
    ImageGeneration,
}

/// 模型种类 — 明确区分生成模型的用途
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Chat,
    Multimodal,
    Image,
}

impl Default for ModelKind {
    fn default() -> Self {
        ModelKind::Chat
    }
}

/// 模型角色 — 指定模型在网关调度中的默认用途
///
/// 用户可为不同任务类型指定不同模型：
/// - `Creative`：正文生成、Writer、改写 → 质量优先
/// - `Tool`：Call 1 路由合成、探测、意图识别、JSON 提取 → 速度优先
/// - `Background`：BGP 审计/入库/洞察、Genesis 后台流水线 → 不抢占前台模型
///
/// 未指定时网关自动分配：快模型→工具，闲置→后台，创作回退 active。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Creative,
    Tool,
    Background,
}

/// 质量等级 — 用于任务路由评分
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityTier {
    Low,
    Medium,
    High,
    Ultra,
}

impl Default for QualityTier {
    fn default() -> Self {
        QualityTier::Medium
    }
}

/// 速度等级 — 用于任务路由评分
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeedTier {
    Fast,
    Normal,
    Slow,
    VerySlow,
}

impl Default for SpeedTier {
    fn default() -> Self {
        SpeedTier::Normal
    }
}

/// 嵌入模型配置档案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider: EmbeddingProvider,
    pub model: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    pub dimensions: usize,
    pub max_input_tokens: usize,
    #[serde(default)]
    pub is_default: bool,
}

/// 支持的嵌入模型提供商
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    OpenAI,
    Azure,
    Ollama,
    Local, // 本地TF-IDF
    Custom,
}

impl std::fmt::Display for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingProvider::OpenAI => write!(f, "openai"),
            EmbeddingProvider::Azure => write!(f, "azure"),
            EmbeddingProvider::Ollama => write!(f, "ollama"),
            EmbeddingProvider::Local => write!(f, "local"),
            EmbeddingProvider::Custom => write!(f, "custom"),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut llm_profiles = HashMap::new();
        let mut embedding_profiles = HashMap::new();

        // 1. 语言模型占位（用户需在设置中替换为真实 endpoint）
        // 保留占位以避免首次启动无模型导致的空指针；标记 enabled=false 并在 UI 提示配置
        let qwen35 = LlmProfile {
            id: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
            name: "Qwen 3.5 语言模型（请检查配置）".to_string(),
            description: Some(
                "默认占位语言模型，请在模型管理中确认或替换为自己的本地模型".to_string(),
            ),
            provider: LlmProvider::Custom,
            model_source: ModelSource::Local,
            model: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
            api_key: "".to_string(),
            api_base: Some(env_or_default(
                "STORYFORGE_LLM_API_BASE",
                "http://localhost:11434/v1",
            )),
            is_local_model: false,
            max_tokens: 2500,
            temperature: 0.8,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: true,
            enabled: false,
            kind: ModelKind::Chat,
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::Completion,
                ModelCapability::LongContext,
            ],
            max_context_length: 8192,
            quality_tier: QualityTier::High,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec!["placeholder".to_string()],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        llm_profiles.insert(qwen35.id.clone(), qwen35);

        // 2. 多模态模型占位
        let gemma4 = LlmProfile {
            id: "Gemma-4-31B-it-Q6_K".to_string(),
            name: "Gemma 4 多模态（请检查配置）".to_string(),
            description: Some("默认占位多模态模型，请在模型管理中确认或替换".to_string()),
            provider: LlmProvider::Custom,
            model_source: ModelSource::Local,
            model: "Gemma-4-31B-it-Q6_K".to_string(),
            api_key: "".to_string(),
            api_base: Some(env_or_default(
                "STORYFORGE_VISION_API_BASE",
                "http://localhost:11435/v1",
            )),
            is_local_model: false,
            max_tokens: 2500,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: false,
            enabled: false,
            kind: ModelKind::Multimodal,
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::Vision,
                ModelCapability::LongContext,
            ],
            max_context_length: 8192,
            quality_tier: QualityTier::High,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec!["placeholder".to_string()],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        llm_profiles.insert(gemma4.id.clone(), gemma4);

        // 3. Embedding 嵌入模型 - bge-m3
        let bge_m3 = EmbeddingProfile {
            id: "bge-m3".to_string(),
            name: "BGE-M3 Embedding".to_string(),
            description: Some("文本嵌入模型，用于语义搜索和向量化".to_string()),
            provider: EmbeddingProvider::Custom,
            model: "bge-m3".to_string(),
            api_key: std::env::var("STORYFORGE_EMBEDDING_API_KEY").unwrap_or_default(),
            api_base: Some(env_or_default(
                "STORYFORGE_EMBEDDING_API_BASE",
                "http://localhost:11436/v1",
            )),
            dimensions: 1024,
            max_input_tokens: 8192,
            is_default: true,
        };
        embedding_profiles.insert(bge_m3.id.clone(), bge_m3);

        let mut agent_mappings = HashMap::new();
        // v0.11.0: Agent 默认不再硬编码指向具体模型，而是采用自动路由策略
        // 用户可在设置中为每个 Agent 指定固定/优先模型
        for agent_id in [
            "writer",
            "inspector",
            "outline_planner",
            "style_mimic",
            "plot_analyzer",
        ] {
            agent_mappings.insert(
                agent_id.to_string(),
                AgentMapping {
                    agent_id: agent_id.to_string(),
                    agent_name: agent_display_name(agent_id),
                    chat_model_id: None,
                    embedding_model_id: None,
                    multimodal_model_id: None,
                    description: agent_description(agent_id),
                    task_type: None,
                    complexity: None,
                    budget_priority: None,
                    speed_priority: None,
                    constraints: vec![],
                    force_locked: false,
                },
            );
        }

        Self {
            llm: LlmConfig {
                provider: "custom".to_string(),
                api_key: "".to_string(),
                model: "".to_string(),
                api_base: None,
                max_tokens: 2500,
                temperature: 0.8,
            },
            llm_profiles,
            embedding_profiles,
            active_llm_profile: None, /* v0.23.14: 不默认指向占位 profile，由
                                       * get_active_llm_profile 走 fallback */
            active_embedding_profile: None,
            creative_model_id: None,
            tool_model_id: None,
            background_model_id: None,
            creative_temperature: None,
            continuation_temperature: None,
            tool_temperature: None,
            agent_mappings,
            book_deconstruction_concurrency: 3,
            rewrite_threshold: 0.75,
            max_feedback_loops: 2,
            style_weight: 0.5,
            narrative_weight: 0.5,
            skip_rewrite_threshold: 0.90,
            keep_revision_history: true,
            genesis_first_chapter_word_count_target: 2000,
            creation_workflow_review_threshold: 0.75,
            creation_workflow_max_iterations: 2,
            candidate_timeout_seconds: default_candidate_timeout_seconds(),
            candidate_timeout_local_seconds: default_candidate_timeout_local_seconds(),
            candidate_max_retries: default_candidate_max_retries(),
            candidate_count: default_candidate_count(),
            context_budget_ratio: default_context_budget_ratio(),
            writer_local_concurrency: default_writer_local_concurrency(),
            writer_remote_concurrency: default_writer_remote_concurrency(),
            candidate_local_sequential: default_candidate_local_sequential(),
            theme: default_theme(),
            language: default_language(),
            auto_save: default_auto_save(),
            auto_save_interval: default_auto_save_interval(),
            font_size: default_font_size(),
            line_height: default_line_height(),
            share_usage_data: default_share_usage_data(),
            writing_strategy: WritingStrategy::default(),
            generation_mode: default_generation_mode(),
            auto_rewrite_severity_threshold: default_auto_rewrite_severity_threshold(),
            llm_connect_timeout_secs: default_llm_connect_timeout(),
            smart_execute_total_timeout_secs: default_smart_execute_timeout(),
            executor_step_timeout_secs: default_executor_step_timeout(),
            frontend_timeout_secs: default_frontend_timeout(),
            llm_first_chunk_timeout_secs: default_llm_first_chunk_timeout(),
            writer_system_prompt_override: String::new(),
            probe_prompt_override: String::new(),
            auth_clients: Default::default(),
        }
    }
}

impl AppConfig {
    /// 打开配置数据库（内部 helper）
    fn open_config_db(
        config_dir: &Path,
    ) -> Result<Pool<SqliteConnectionManager>, Box<dyn std::error::Error>> {
        let db_path = config_dir.join("cinema_ai.db");
        let manager = SqliteConnectionManager::file(&db_path)
            .with_init(|c| c.execute_batch("PRAGMA foreign_keys = ON;"));
        let pool = Pool::builder().max_size(1).build(manager)?;
        Ok(pool)
    }

    /// 确保 app_settings 表存在
    fn ensure_app_settings_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// 从 SQLite 加载配置（W4-B3）
    fn load_from_db(config_dir: &Path) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let pool = match Self::open_config_db(config_dir) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "[AppConfig] Cannot open DB: {}, falling back to config.json",
                    e
                );
                return Ok(None);
            }
        };
        let conn = pool.get()?;
        Self::ensure_app_settings_table(&conn)?;

        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'app_config'",
                [],
                |row| row.get(0),
            )
            .ok();

        match value {
            Some(json) => {
                let config: AppConfig = serde_json::from_str(&json)?;
                log::info!("[AppConfig] Loaded from SQLite");
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// 保存完整配置到 SQLite（W4-B3）
    fn save_to_db(&self, config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let pool = Self::open_config_db(config_dir)?;
        let conn = pool.get()?;
        Self::ensure_app_settings_table(&conn)?;

        let json = serde_json::to_string(self)?;
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["app_config", json, now],
        )?;
        log::info!("[AppConfig] Saved to SQLite");
        Ok(())
    }

    pub fn load(config_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // W4-B3: 优先从 SQLite 加载；如不存在则从 config.json 回退并自动迁移
        let mut config = match Self::load_from_db(config_dir)? {
            Some(cfg) => cfg,
            None => {
                let config_path = config_dir.join("config.json");
                let config = if config_path.exists() {
                    let content = fs::read_to_string(&config_path)?;
                    // 尝试解析完整 AppConfig；若失败（可能是新的 BootstrapConfig），使用默认
                    match serde_json::from_str::<AppConfig>(&content) {
                        Ok(mut cfg) => {
                            if cfg.llm_profiles.is_empty() {
                                cfg.migrate_legacy_config();
                            }
                            cfg
                        }
                        Err(e) => {
                            log::info!(
                                "[AppConfig] config.json is not legacy AppConfig ({}), using \
                                 defaults",
                                e
                            );
                            AppConfig::default()
                        }
                    }
                } else {
                    AppConfig::default()
                };
                // 首次迁移到 SQLite
                if let Err(e) = config.save_to_db(config_dir) {
                    log::warn!("[AppConfig] Auto-migration to SQLite failed: {}", e);
                }
                config
            }
        };

        // v0.22.3: API Key 直接存储在 SQLite 中，不再使用 macOS 钥匙串。
        // 钥匙串已彻底移除 —— 对本地模型用户（API Key 为空）无影响；
        // 使用云 API 的用户若之前密钥存在钥匙串中，需在设置中重新输入一次。

        // v0.11.0: 迁移旧配置字段，不再自动补充硬编码真实模型
        let mut needs_save = false;

        // 为已有 LLM profile 补齐路由相关字段（从旧版本迁移）
        for profile in config.llm_profiles.values_mut() {
            if profile.max_context_length == 0 {
                profile.max_context_length = 8192;
                needs_save = true;
            }
            // 根据 capabilities 推断 kind（旧版本只有 Chat/Multimodal 两种）
            if profile.kind == ModelKind::Chat
                && profile.capabilities.contains(&ModelCapability::Vision)
            {
                profile.kind = ModelKind::Multimodal;
                needs_save = true;
            }
            // v0.14.0: 自动推断并补齐现代能力标签
            if profile.infer_capabilities() {
                needs_save = true;
            }
        }

        // 清理指向不存在模型的 Agent 映射
        let valid_llm_ids: std::collections::HashSet<_> =
            config.llm_profiles.keys().cloned().collect();
        let valid_embedding_ids: std::collections::HashSet<_> =
            config.embedding_profiles.keys().cloned().collect();
        for mapping in config.agent_mappings.values_mut() {
            if mapping
                .chat_model_id
                .as_ref()
                .is_some_and(|id| !valid_llm_ids.contains(id))
            {
                mapping.chat_model_id = None;
                needs_save = true;
            }
            if mapping
                .embedding_model_id
                .as_ref()
                .is_some_and(|id| !valid_embedding_ids.contains(id))
            {
                mapping.embedding_model_id = None;
                needs_save = true;
            }
            if mapping
                .multimodal_model_id
                .as_ref()
                .is_some_and(|id| !valid_llm_ids.contains(id))
            {
                mapping.multimodal_model_id = None;
                needs_save = true;
            }
        }

        // 如果活跃模型已不存在，回退到剩余配置中的默认或第一个
        if config
            .active_llm_profile
            .as_ref()
            .is_some_and(|id| !config.llm_profiles.contains_key(id))
        {
            config.active_llm_profile = config
                .llm_profiles
                .values()
                .find(|p| p.is_default)
                .or_else(|| config.llm_profiles.values().next())
                .map(|p| p.id.clone());
            needs_save = true;
        }
        if config
            .active_embedding_profile
            .as_ref()
            .is_some_and(|id| !config.embedding_profiles.contains_key(id))
        {
            config.active_embedding_profile = config
                .embedding_profiles
                .values()
                .find(|p| p.is_default)
                .or_else(|| config.embedding_profiles.values().next())
                .map(|p| p.id.clone());
            needs_save = true;
        }

        if needs_save {
            let _ = config.save(config_dir);
        }

        Ok(config)
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // 1. W4-B3: Save user-level config to SQLite（含 API Key）
        self.save_to_db(config_dir)?;

        // 2. W4-B3: config.json 只保留启动级配置
        let bootstrap = BootstrapConfig::default();
        bootstrap.save(config_dir)?;

        Ok(())
    }

    /// 获取当前活跃的LLM配置
    ///
    /// 只返回 `enabled=true` 的模型。若用户显式指定的活跃模型被禁用，或默认模型
    /// 被禁用，均不回退到禁用模型，避免生成请求陷入长超时。
    pub fn get_active_llm_profile(&self) -> Option<&LlmProfile> {
        let explicit = self
            .active_llm_profile
            .as_ref()
            .and_then(|id| self.llm_profiles.get(id))
            .filter(|p| p.enabled);
        explicit.or_else(|| {
            self.llm_profiles
                .values()
                .find(|p| p.is_default && p.enabled)
        })
    }

    /// 获取当前活跃的嵌入模型配置
    pub fn get_active_embedding_profile(&self) -> Option<&EmbeddingProfile> {
        let explicit = self
            .active_embedding_profile
            .as_ref()
            .and_then(|id| self.embedding_profiles.get(id));
        explicit.or_else(|| self.embedding_profiles.values().find(|p| p.is_default))
    }

    /// 设置活跃的LLM配置
    pub fn set_active_llm_profile(&mut self, profile_id: &str) -> Result<(), AppError> {
        // v0.23.14: 拒绝将已禁用的模型设为活跃模型，防止 active 指针指向不可用模型
        // 后静默跳过、回落到候选链死模型导致超时。
        match self.llm_profiles.get(profile_id) {
            Some(profile) if profile.enabled => {
                self.active_llm_profile = Some(profile_id.to_string());
                Ok(())
            }
            Some(_) => Err(AppError::ValidationFailed {
                message: "无法将已禁用的模型设为活跃模型".to_string(),
                field: Some("profile_id".to_string()),
            }),
            None => Err(AppError::NotFound {
                resource: "llm_profile".to_string(),
                id: profile_id.to_string(),
            }),
        }
    }

    /// 设置活跃的嵌入模型配置
    pub fn set_active_embedding_profile(&mut self, profile_id: &str) -> Result<(), AppError> {
        if self.embedding_profiles.contains_key(profile_id) {
            self.active_embedding_profile = Some(profile_id.to_string());
            Ok(())
        } else {
            Err(AppError::NotFound {
                resource: "embedding_profile".to_string(),
                id: profile_id.to_string(),
            })
        }
    }

    // =========================================================================
    // v0.23.66: 模型角色分配 — 创作/工具/后台任务三层默认模型
    // =========================================================================

    /// 按角色获取当前指定的模型 profile（若未设置或模型不可用则返回 None）
    pub fn get_model_for_role(&self, role: ModelRole) -> Option<&LlmProfile> {
        let id = match role {
            ModelRole::Creative => self.creative_model_id.as_ref(),
            ModelRole::Tool => self.tool_model_id.as_ref(),
            ModelRole::Background => self.background_model_id.as_ref(),
        };
        id.and_then(|id| self.llm_profiles.get(id))
            .filter(|p| p.enabled)
    }

    /// 设置指定角色的默认模型
    pub fn set_model_for_role(&mut self, role: ModelRole, model_id: &str) -> Result<(), AppError> {
        match self.llm_profiles.get(model_id) {
            Some(profile) if profile.enabled => {
                let slot = match role {
                    ModelRole::Creative => &mut self.creative_model_id,
                    ModelRole::Tool => &mut self.tool_model_id,
                    ModelRole::Background => &mut self.background_model_id,
                };
                *slot = Some(model_id.to_string());
                Ok(())
            }
            Some(_) => Err(AppError::ValidationFailed {
                message: "无法将已禁用的模型设为角色模型".to_string(),
                field: Some("model_id".to_string()),
            }),
            None => Err(AppError::NotFound {
                resource: "llm_profile".to_string(),
                id: model_id.to_string(),
            }),
        }
    }

    /// 清除指定角色的模型分配（恢复自动分配）
    pub fn clear_model_for_role(&mut self, role: ModelRole) {
        let slot = match role {
            ModelRole::Creative => &mut self.creative_model_id,
            ModelRole::Tool => &mut self.tool_model_id,
            ModelRole::Background => &mut self.background_model_id,
        };
        *slot = None;
    }

    /// v0.23.66: 获取创意生成（创世/首章）的温度覆盖值
    pub fn creative_temperature(&self) -> Option<f32> {
        self.creative_temperature
    }

    /// v0.23.66: 获取续写温度覆盖值
    pub fn continuation_temperature(&self) -> Option<f32> {
        self.continuation_temperature
    }

    /// v0.23.66: 获取工具任务（路由/JSON提取）温度覆盖值
    pub fn tool_temperature(&self) -> Option<f32> {
        self.tool_temperature
    }

    /// 添加LLM配置
    pub fn add_llm_profile(&mut self, mut profile: LlmProfile) -> Result<(), AppError> {
        if profile.id.is_empty() {
            profile.id = format!("llm-{}", uuid::Uuid::new_v4());
        }

        // 如果设为默认，取消其他默认
        if profile.is_default {
            for p in self.llm_profiles.values_mut() {
                p.is_default = false;
            }
        }

        self.llm_profiles.insert(profile.id.clone(), profile);
        Ok(())
    }

    /// 添加嵌入模型配置
    pub fn add_embedding_profile(&mut self, mut profile: EmbeddingProfile) -> Result<(), AppError> {
        if profile.id.is_empty() {
            profile.id = format!("emb-{}", uuid::Uuid::new_v4());
        }

        // 如果设为默认，取消其他默认
        if profile.is_default {
            for p in self.embedding_profiles.values_mut() {
                p.is_default = false;
            }
        }

        self.embedding_profiles.insert(profile.id.clone(), profile);
        Ok(())
    }

    /// 删除LLM配置
    pub fn remove_llm_profile(&mut self, profile_id: &str) -> Result<(), AppError> {
        if let Some(profile) = self.llm_profiles.get(profile_id) {
            if profile.is_default && self.llm_profiles.len() > 1 {
                return Err(AppError::ValidationFailed {
                    message: "Cannot delete the default profile".to_string(),
                    field: Some("profile_id".to_string()),
                });
            }
            self.llm_profiles.remove(profile_id);

            // 如果删除的是当前活跃配置，重置
            if self.active_llm_profile.as_deref() == Some(profile_id) {
                self.active_llm_profile = self
                    .llm_profiles
                    .values()
                    .find(|p| p.is_default)
                    .map(|p| p.id.clone());
            }
            // v0.23.66: 清除该模型的所有角色分配
            if self.creative_model_id.as_deref() == Some(profile_id) {
                self.creative_model_id = None;
            }
            if self.tool_model_id.as_deref() == Some(profile_id) {
                self.tool_model_id = None;
            }
            if self.background_model_id.as_deref() == Some(profile_id) {
                self.background_model_id = None;
            }
            Ok(())
        } else {
            Err(AppError::NotFound {
                resource: "llm_profile".to_string(),
                id: profile_id.to_string(),
            })
        }
    }

    /// 删除嵌入模型配置
    pub fn remove_embedding_profile(&mut self, profile_id: &str) -> Result<(), AppError> {
        if let Some(profile) = self.embedding_profiles.get(profile_id) {
            if profile.is_default && self.embedding_profiles.len() > 1 {
                return Err(AppError::ValidationFailed {
                    message: "Cannot delete the default profile".to_string(),
                    field: Some("profile_id".to_string()),
                });
            }
            self.embedding_profiles.remove(profile_id);

            if self.active_embedding_profile.as_deref() == Some(profile_id) {
                self.active_embedding_profile = self
                    .embedding_profiles
                    .values()
                    .find(|p| p.is_default)
                    .map(|p| p.id.clone());
            }
            Ok(())
        } else {
            Err(AppError::NotFound {
                resource: "embedding_profile".to_string(),
                id: profile_id.to_string(),
            })
        }
    }

    /// 迁移旧版配置
    fn migrate_legacy_config(&mut self) {
        let provider = match self.llm.provider.as_str() {
            "anthropic" => LlmProvider::Anthropic,
            "ollama" => LlmProvider::Ollama,
            _ => LlmProvider::OpenAI,
        };
        let model_source = if provider == LlmProvider::Ollama {
            ModelSource::Local
        } else {
            ModelSource::UserOwned
        };
        let legacy_profile = LlmProfile {
            id: "legacy".to_string(),
            name: "Legacy Config".to_string(),
            description: Some("从旧版本迁移的配置".to_string()),
            provider,
            model_source,
            model: self.llm.model.clone(),
            api_key: self.llm.api_key.clone(),
            api_base: self.llm.api_base.clone(),
            is_local_model: false,
            max_tokens: self.llm.max_tokens,
            temperature: self.llm.temperature,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: true,
            enabled: true,
            kind: ModelKind::Chat,
            capabilities: vec![ModelCapability::Chat, ModelCapability::Completion],
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec!["legacy".to_string()],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        let _ = legacy_profile; // suppress unused warning in migration code

        self.llm_profiles
            .insert(legacy_profile.id.clone(), legacy_profile);
        self.active_llm_profile = Some("legacy".to_string());
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
