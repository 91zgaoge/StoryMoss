#![allow(dead_code)]
use std::{collections::HashMap, fs, path::Path};

// W4-B3: SQLite-backed config storage
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};

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

/// 默认 LLM 请求超时（秒）。本地大模型生成长文本较慢，300 秒比 120
/// 秒更能避免误超时。
pub const DEFAULT_LLM_TIMEOUT_SECONDS: u64 = 300;

// ============================================================================
// Secure API Key Storage (cross-platform keychain)
// ============================================================================

pub mod secure_storage {
    use keyring::Entry;

    const SERVICE_NAME: &str = "storyforge";

    /// Store an API key in the OS keychain
    pub fn store_api_key(profile_id: &str, api_key: &str) -> Result<(), String> {
        if api_key.is_empty() {
            return Ok(());
        }
        let entry = Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Keyring entry creation failed: {}", e))?;
        entry
            .set_password(api_key)
            .map_err(|e| format!("Failed to store API key: {}", e))?;
        Ok(())
    }

    /// Retrieve an API key from the OS keychain
    pub fn get_api_key(profile_id: &str) -> Result<Option<String>, String> {
        let entry = Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Keyring entry creation failed: {}", e))?;
        match entry.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Failed to retrieve API key: {}", e)),
        }
    }

    /// Delete an API key from the OS keychain
    pub fn delete_api_key(profile_id: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Keyring entry creation failed: {}", e))?;
        entry
            .delete_credential()
            .map_err(|e| format!("Failed to delete API key: {}", e))?;
        Ok(())
    }
}

/// Agent 模型映射
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
    #[serde(default = "default_store_api_keys_securely")]
    pub store_api_keys_securely: bool,
    #[serde(default)]
    pub writing_strategy: WritingStrategy,
    /// OAuth 客户端配置
    #[serde(default)]
    pub auth_clients: Option<HashMap<String, crate::auth::OAuthClientConfig>>,
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

fn default_store_api_keys_securely() -> bool {
    true
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

/// LLM 模型配置档案
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
    #[serde(default)]
    pub capabilities: Vec<ModelCapability>,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Chat,
    Completion,
    FunctionCalling,
    JsonMode,
    Vision,
    LongContext,
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

        // 1. 语言模型 - Qwen3.5-27B-Uncensored-Q4_K_M
        let qwen35 = LlmProfile {
            id: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
            name: "Qwen 3.5 语言模型".to_string(),
            description: Some("本地语言模型，用于文本生成和对话".to_string()),
            provider: LlmProvider::Custom,
            model_source: ModelSource::Local,
            model: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
            api_key: "".to_string(),
            api_base: Some(env_or_default(
                "STORYFORGE_LLM_API_BASE",
                "http://localhost:11434/v1",
            )),
            max_tokens: 8192,
            temperature: 0.8,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: true,
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::Completion,
                ModelCapability::LongContext,
            ],
        };
        llm_profiles.insert(qwen35.id.clone(), qwen35);

        // 2. 多模态模型 - Gemma-4-31B-it-Q6_K
        let gemma4 = LlmProfile {
            id: "Gemma-4-31B-it-Q6_K".to_string(),
            name: "Gemma 4 多模态".to_string(),
            description: Some("本地多模态模型，支持图文理解".to_string()),
            provider: LlmProvider::Custom,
            model_source: ModelSource::Local,
            model: "Gemma-4-31B-it-Q6_K".to_string(),
            api_key: "".to_string(),
            api_base: Some(env_or_default(
                "STORYFORGE_VISION_API_BASE",
                "http://localhost:11435/v1",
            )),
            max_tokens: 8192,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: false,
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::Vision,
                ModelCapability::LongContext,
            ],
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
        agent_mappings.insert(
            "writer".to_string(),
            AgentMapping {
                agent_id: "writer".to_string(),
                agent_name: "写作助手".to_string(),
                chat_model_id: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
                embedding_model_id: None,
                multimodal_model_id: None,
                description: Some("负责章节生成、改写".to_string()),
            },
        );
        agent_mappings.insert(
            "inspector".to_string(),
            AgentMapping {
                agent_id: "inspector".to_string(),
                agent_name: "质检员".to_string(),
                chat_model_id: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
                embedding_model_id: None,
                multimodal_model_id: None,
                description: Some("负责内容质量检查".to_string()),
            },
        );
        agent_mappings.insert(
            "outline_planner".to_string(),
            AgentMapping {
                agent_id: "outline_planner".to_string(),
                agent_name: "大纲规划师".to_string(),
                chat_model_id: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
                embedding_model_id: None,
                multimodal_model_id: None,
                description: Some("负责故事大纲设计".to_string()),
            },
        );
        agent_mappings.insert(
            "style_mimic".to_string(),
            AgentMapping {
                agent_id: "style_mimic".to_string(),
                agent_name: "风格模仿师".to_string(),
                chat_model_id: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
                embedding_model_id: None,
                multimodal_model_id: None,
                description: Some("负责文风分析与模仿".to_string()),
            },
        );
        agent_mappings.insert(
            "plot_analyzer".to_string(),
            AgentMapping {
                agent_id: "plot_analyzer".to_string(),
                agent_name: "情节分析师".to_string(),
                chat_model_id: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
                embedding_model_id: None,
                multimodal_model_id: None,
                description: Some("负责情节复杂度分析".to_string()),
            },
        );

        Self {
            llm: LlmConfig {
                provider: "custom".to_string(),
                api_key: "".to_string(),
                model: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
                api_base: Some(env_or_default(
                    "STORYFORGE_LLM_API_BASE",
                    "http://localhost:11434/v1",
                )),
                max_tokens: 8192,
                temperature: 0.8,
            },
            llm_profiles,
            embedding_profiles,
            active_llm_profile: Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string()),
            active_embedding_profile: Some("bge-m3".to_string()),
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
            theme: default_theme(),
            language: default_language(),
            auto_save: default_auto_save(),
            auto_save_interval: default_auto_save_interval(),
            font_size: default_font_size(),
            line_height: default_line_height(),
            share_usage_data: default_share_usage_data(),
            store_api_keys_securely: default_store_api_keys_securely(),
            writing_strategy: WritingStrategy::default(),
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

        // 序列化前 strip API keys（它们由 keychain 管理）
        let mut temp = self.clone();
        for profile in temp.llm_profiles.values_mut() {
            profile.api_key.clear();
        }
        for profile in temp.embedding_profiles.values_mut() {
            profile.api_key.clear();
        }
        temp.llm.api_key.clear();

        let json = serde_json::to_string(&temp)?;
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

        // ================================================================
        // Migrate API keys from config.json to OS keychain (W1-B5)
        // ================================================================
        let mut keys_migrated = false;

        // Migrate LLM profile API keys
        for (id, profile) in config.llm_profiles.iter_mut() {
            if !profile.api_key.is_empty() {
                match secure_storage::store_api_key(id, &profile.api_key) {
                    Ok(()) => {
                        log::info!(
                            "[SecureStorage] Migrated API key for profile '{}' to OS keychain",
                            id
                        );
                        profile.api_key.clear();
                        keys_migrated = true;
                    }
                    Err(e) => {
                        log::warn!(
                            "[SecureStorage] Failed to migrate API key for profile '{}': {}",
                            id,
                            e
                        );
                    }
                }
            }
        }

        // Migrate embedding profile API keys
        for (id, profile) in config.embedding_profiles.iter_mut() {
            if !profile.api_key.is_empty() {
                match secure_storage::store_api_key(id, &profile.api_key) {
                    Ok(()) => {
                        log::info!(
                            "[SecureStorage] Migrated API key for embedding profile '{}' to OS \
                             keychain",
                            id
                        );
                        profile.api_key.clear();
                        keys_migrated = true;
                    }
                    Err(e) => {
                        log::warn!(
                            "[SecureStorage] Failed to migrate API key for embedding profile \
                             '{}': {}",
                            id,
                            e
                        );
                    }
                }
            }
        }

        // Migrate legacy LLM config API key
        if !config.llm.api_key.is_empty() {
            match secure_storage::store_api_key("legacy_llm", &config.llm.api_key) {
                Ok(()) => {
                    log::info!("[SecureStorage] Migrated legacy LLM API key to OS keychain");
                    config.llm.api_key.clear();
                    keys_migrated = true;
                }
                Err(e) => {
                    log::warn!(
                        "[SecureStorage] Failed to migrate legacy LLM API key: {}",
                        e
                    );
                }
            }
        }

        // Restore API keys from keychain into memory for runtime use
        for (id, profile) in config.llm_profiles.iter_mut() {
            match secure_storage::get_api_key(id) {
                Ok(Some(key)) => profile.api_key = key,
                Ok(None) => {}
                Err(e) => log::warn!(
                    "[SecureStorage] Failed to load API key for profile '{}': {}",
                    id,
                    e
                ),
            }
        }
        for (id, profile) in config.embedding_profiles.iter_mut() {
            match secure_storage::get_api_key(id) {
                Ok(Some(key)) => profile.api_key = key,
                Ok(None) => {}
                Err(e) => log::warn!(
                    "[SecureStorage] Failed to load API key for embedding profile '{}': {}",
                    id,
                    e
                ),
            }
        }
        match secure_storage::get_api_key("legacy_llm") {
            Ok(Some(key)) => config.llm.api_key = key,
            Ok(None) => {}
            Err(e) => log::warn!("[SecureStorage] Failed to load legacy LLM API key: {}", e),
        }

        if keys_migrated {
            let _ = config.save(config_dir);
        }

        // 自动补充真实本地模型（如果缺失）
        let mut needs_save = false;

        // 补充 Qwen3.5 语言模型
        if !config
            .llm_profiles
            .contains_key("Qwen3.5-27B-Uncensored-Q4_K_M")
        {
            let qwen35 = LlmProfile {
                id: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
                name: "Qwen 3.5 语言模型".to_string(),
                description: Some("本地语言模型，用于文本生成和对话".to_string()),
                provider: LlmProvider::Custom,
                model_source: ModelSource::Local,
                model: "Qwen3.5-27B-Uncensored-Q4_K_M".to_string(),
                api_key: "".to_string(),
                api_base: Some(env_or_default(
                    "STORYFORGE_LLM_API_BASE",
                    "http://localhost:11434/v1",
                )),
                max_tokens: 8192,
                temperature: 0.8,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
                is_default: config.llm_profiles.values().all(|p| !p.is_default),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Completion,
                    ModelCapability::LongContext,
                ],
            };
            config.llm_profiles.insert(qwen35.id.clone(), qwen35);
            if config.active_llm_profile.is_none() {
                config.active_llm_profile = Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string());
            }
            needs_save = true;
        }

        // 补充 Gemma-4 多模态模型
        if !config.llm_profiles.contains_key("Gemma-4-31B-it-Q6_K") {
            let gemma4 = LlmProfile {
                id: "Gemma-4-31B-it-Q6_K".to_string(),
                name: "Gemma 4 多模态".to_string(),
                description: Some("本地多模态模型，支持图文理解".to_string()),
                provider: LlmProvider::Custom,
                model_source: ModelSource::Local,
                model: "Gemma-4-31B-it-Q6_K".to_string(),
                api_key: "".to_string(),
                api_base: Some(env_or_default(
                    "STORYFORGE_VISION_API_BASE",
                    "http://localhost:11435/v1",
                )),
                max_tokens: 8192,
                temperature: 0.7,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
                is_default: false,
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Vision,
                    ModelCapability::LongContext,
                ],
            };
            config.llm_profiles.insert(gemma4.id.clone(), gemma4);
            needs_save = true;
        }

        // 补充 bge-m3 嵌入模型
        if !config.embedding_profiles.contains_key("bge-m3") {
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
                is_default: config.embedding_profiles.values().all(|p| !p.is_default),
            };
            config.embedding_profiles.insert(bge_m3.id.clone(), bge_m3);
            if config.active_embedding_profile.is_none() {
                config.active_embedding_profile = Some("bge-m3".to_string());
            }
            needs_save = true;
        }

        if needs_save {
            let _ = config.save(config_dir);
        }

        Ok(config)
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Persist all API keys to OS keychain first
        for (id, profile) in &self.llm_profiles {
            if !profile.api_key.is_empty() {
                if let Err(e) = secure_storage::store_api_key(id, &profile.api_key) {
                    log::warn!(
                        "[SecureStorage] Failed to store API key for profile '{}': {}",
                        id,
                        e
                    );
                }
            }
        }
        for (id, profile) in &self.embedding_profiles {
            if !profile.api_key.is_empty() {
                if let Err(e) = secure_storage::store_api_key(id, &profile.api_key) {
                    log::warn!(
                        "[SecureStorage] Failed to store API key for embedding profile '{}': {}",
                        id,
                        e
                    );
                }
            }
        }
        if !self.llm.api_key.is_empty() {
            if let Err(e) = secure_storage::store_api_key("legacy_llm", &self.llm.api_key) {
                log::warn!("[SecureStorage] Failed to store legacy LLM API key: {}", e);
            }
        }

        // 2. W4-B3: Save user-level config to SQLite
        self.save_to_db(config_dir)?;

        // 3. W4-B3: config.json 只保留启动级配置
        let bootstrap = BootstrapConfig::default();
        bootstrap.save(config_dir)?;

        Ok(())
    }

    /// 获取当前活跃的LLM配置
    pub fn get_active_llm_profile(&self) -> Option<&LlmProfile> {
        self.active_llm_profile
            .as_ref()
            .and_then(|id| self.llm_profiles.get(id))
            .or_else(|| self.llm_profiles.values().find(|p| p.is_default))
    }

    /// 获取当前活跃的嵌入模型配置
    pub fn get_active_embedding_profile(&self) -> Option<&EmbeddingProfile> {
        self.active_embedding_profile
            .as_ref()
            .and_then(|id| self.embedding_profiles.get(id))
            .or_else(|| self.embedding_profiles.values().find(|p| p.is_default))
    }

    /// 设置活跃的LLM配置
    pub fn set_active_llm_profile(&mut self, profile_id: &str) -> Result<(), String> {
        if self.llm_profiles.contains_key(profile_id) {
            self.active_llm_profile = Some(profile_id.to_string());
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", profile_id))
        }
    }

    /// 设置活跃的嵌入模型配置
    pub fn set_active_embedding_profile(&mut self, profile_id: &str) -> Result<(), String> {
        if self.embedding_profiles.contains_key(profile_id) {
            self.active_embedding_profile = Some(profile_id.to_string());
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", profile_id))
        }
    }

    /// 添加LLM配置
    pub fn add_llm_profile(&mut self, mut profile: LlmProfile) -> Result<(), String> {
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
    pub fn add_embedding_profile(&mut self, mut profile: EmbeddingProfile) -> Result<(), String> {
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
    pub fn remove_llm_profile(&mut self, profile_id: &str) -> Result<(), String> {
        if let Some(profile) = self.llm_profiles.get(profile_id) {
            if profile.is_default && self.llm_profiles.len() > 1 {
                return Err("Cannot delete the default profile".to_string());
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
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", profile_id))
        }
    }

    /// 删除嵌入模型配置
    pub fn remove_embedding_profile(&mut self, profile_id: &str) -> Result<(), String> {
        if let Some(profile) = self.embedding_profiles.get(profile_id) {
            if profile.is_default && self.embedding_profiles.len() > 1 {
                return Err("Cannot delete the default profile".to_string());
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
            Err(format!("Profile '{}' not found", profile_id))
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
            max_tokens: self.llm.max_tokens,
            temperature: self.llm.temperature,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: DEFAULT_LLM_TIMEOUT_SECONDS,
            is_default: true,
            capabilities: vec![ModelCapability::Chat, ModelCapability::Completion],
        };

        self.llm_profiles
            .insert(legacy_profile.id.clone(), legacy_profile);
        self.active_llm_profile = Some("legacy".to_string());
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
