//! Settings management commands for Tauri

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager};

use super::settings::*;
use crate::error::AppError;

/// 模型类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    Chat,
    Embedding,
    Multimodal,
    Image,
}

/// 通用模型配置（前端传来的）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub model_type: ModelType,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub dimensions: Option<usize>,
    pub capabilities: Option<Vec<String>>,
    pub is_default: Option<bool>,
    pub enabled: Option<bool>,
    // v0.11.0 路由元数据
    pub max_context_length: Option<u32>,
    pub quality_tier: Option<String>,
    pub speed_tier: Option<String>,
    pub cost_per_1k_input: Option<f64>,
    pub cost_per_1k_output: Option<f64>,
    pub tags: Option<Vec<String>>,
    // v0.14.0 模型网关元数据
    pub supports_system_prompt: Option<bool>,
    pub supports_streaming: Option<bool>,
    pub knowledge_cutoff: Option<String>,
    pub reasoning_effort: Option<String>,
    /// v0.23.61: 每模型独立系统提示词覆盖
    #[serde(default)]
    pub system_prompt_override: Option<String>,
}

// ============================================================================
// 辅助函数 — 消除 Chat/Multimodal 重复逻辑
// ============================================================================

/// 解析提供商字符串为 LlmProvider
fn parse_llm_provider(provider: &str) -> LlmProvider {
    match provider {
        "anthropic" => LlmProvider::Anthropic,
        "azure" => LlmProvider::Azure,
        "ollama" => LlmProvider::Ollama,
        "deepseek" => LlmProvider::DeepSeek,
        "qwen" => LlmProvider::Qwen,
        "custom" => LlmProvider::Custom,
        _ => LlmProvider::OpenAI,
    }
}

/// 解析能力字符串列表为 ModelCapability 列表
fn parse_capabilities(caps: Vec<String>) -> Vec<ModelCapability> {
    caps.into_iter()
        .filter_map(|c| match c.as_str() {
            "chat" => Some(ModelCapability::Chat),
            "completion" => Some(ModelCapability::Completion),
            "function_calling" => Some(ModelCapability::FunctionCalling),
            "json_mode" => Some(ModelCapability::JsonMode),
            "vision" => Some(ModelCapability::Vision),
            "long_context" => Some(ModelCapability::LongContext),
            "reasoning" => Some(ModelCapability::Reasoning),
            "tool_use" => Some(ModelCapability::ToolUse),
            "structured_output" => Some(ModelCapability::StructuredOutput),
            "streaming" => Some(ModelCapability::Streaming),
            "fast" => Some(ModelCapability::Fast),
            "image_generation" => Some(ModelCapability::ImageGeneration),
            _ => None,
        })
        .collect()
}

/// 规范化 temperature：限制到2位小数，避免浮点精度噪声
fn normalize_temperature(temp: f32) -> f32 {
    ((temp * 100.0).round() / 100.0).clamp(0.0, 2.0)
}

/// 解析质量等级字符串
fn parse_quality_tier(tier: Option<&str>) -> QualityTier {
    match tier {
        Some("low") => QualityTier::Low,
        Some("medium") => QualityTier::Medium,
        Some("high") => QualityTier::High,
        Some("ultra") => QualityTier::Ultra,
        _ => QualityTier::Medium,
    }
}

/// 解析速度等级字符串
fn parse_speed_tier(tier: Option<&str>) -> SpeedTier {
    match tier {
        Some("fast") => SpeedTier::Fast,
        Some("normal") => SpeedTier::Normal,
        Some("slow") => SpeedTier::Slow,
        Some("very_slow") => SpeedTier::VerySlow,
        _ => SpeedTier::Normal,
    }
}

/// 构建 LlmProfile（Chat 和 Multimodal 共用）
fn build_llm_profile(
    model_id: String,
    config: &ModelConfigInput,
    force_vision: bool,
) -> LlmProfile {
    let mut capabilities = config
        .capabilities
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| match c.as_str() {
            "chat" => Some(ModelCapability::Chat),
            "completion" => Some(ModelCapability::Completion),
            "function_calling" => Some(ModelCapability::FunctionCalling),
            "json_mode" => Some(ModelCapability::JsonMode),
            "vision" => Some(ModelCapability::Vision),
            "long_context" => Some(ModelCapability::LongContext),
            "reasoning" => Some(ModelCapability::Reasoning),
            "tool_use" => Some(ModelCapability::ToolUse),
            "structured_output" => Some(ModelCapability::StructuredOutput),
            "streaming" => Some(ModelCapability::Streaming),
            "fast" => Some(ModelCapability::Fast),
            "image_generation" => Some(ModelCapability::ImageGeneration),
            _ => None,
        })
        .collect::<Vec<_>>();

    if force_vision && !capabilities.contains(&ModelCapability::Vision) {
        capabilities.push(ModelCapability::Vision);
    }

    // 确保 chat capability 始终存在
    if !capabilities.contains(&ModelCapability::Chat) {
        capabilities.push(ModelCapability::Chat);
    }

    let provider = parse_llm_provider(&config.provider);
    let model_source = match config.provider.as_str() {
        "ollama" | "local" => ModelSource::Local,
        _ => ModelSource::Platform,
    };

    // 根据类型推断 kind
    let kind = match config.model_type {
        ModelType::Multimodal => ModelKind::Multimodal,
        ModelType::Image => ModelKind::Image,
        _ => ModelKind::Chat,
    };

    LlmProfile {
        id: model_id,
        name: config.name.clone(),
        description: config.description.clone(),
        provider,
        model: config.model.clone(),
        api_key: config.api_key.clone().unwrap_or_default(),
        api_base: config.api_base.clone(),
        is_local_model: false,
        max_tokens: config.max_tokens.unwrap_or(2000),
        temperature: normalize_temperature(config.temperature.unwrap_or(0.7)),
        top_p: config.top_p.map(|v| v.clamp(0.0, 1.0)),
        frequency_penalty: config.frequency_penalty.map(|v| v.clamp(-2.0, 2.0)),
        presence_penalty: config.presence_penalty.map(|v| v.clamp(-2.0, 2.0)),
        timeout_seconds: super::settings::DEFAULT_LLM_TIMEOUT_SECONDS,
        is_default: config.is_default.unwrap_or(false),
        enabled: config.enabled.unwrap_or(true),
        kind,
        capabilities,
        max_context_length: config.max_context_length.unwrap_or(8192),
        quality_tier: parse_quality_tier(config.quality_tier.as_deref()),
        speed_tier: parse_speed_tier(config.speed_tier.as_deref()),
        cost_per_1k_input: config.cost_per_1k_input,
        cost_per_1k_output: config.cost_per_1k_output,
        tags: config.tags.clone().unwrap_or_default(),
        model_source,
        supports_system_prompt: config.supports_system_prompt.unwrap_or(true),
        supports_streaming: config.supports_streaming.unwrap_or(true),
        knowledge_cutoff: config.knowledge_cutoff.clone(),
        reasoning_effort: config.reasoning_effort.clone(),
        system_prompt_override: config.system_prompt_override.clone(),
    }
}

/// 应用设置导出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsExport {
    pub version: String,
    pub exported_at: String,
    pub settings: AppSettingsData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettingsData {
    #[serde(default)]
    pub models: HashMap<String, Vec<serde_json::Value>>,
    #[serde(default)]
    pub active_models: HashMap<String, String>,
    #[serde(default)]
    pub agent_mappings: Vec<AgentMapping>,
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub privacy: PrivacySettings,
    /// 拆书分析 LLM 并发数
    #[serde(default = "default_concurrency")]
    pub book_deconstruction_concurrency: usize,
    /// AgentOrchestrator 质检改写阈值
    #[serde(default = "default_rewrite_threshold")]
    pub rewrite_threshold: f32,
    /// AgentOrchestrator 最大反馈循环次数
    #[serde(default = "default_max_feedback_loops")]
    pub max_feedback_loops: u32,
    #[serde(default)]
    pub writing_strategy: super::settings::WritingStrategy,

    // v0.17.1: 补齐 v0.16.0 引入但未在 IPC 暴露的字段，
    // 修复「超时设置一改就弹出 保存设置失败: undefined」的根因。
    // 全部带 #[serde(default)]，前端只需发送 patch 即可。
    #[serde(default)]
    pub style_weight: Option<f32>,
    #[serde(default)]
    pub narrative_weight: Option<f32>,
    #[serde(default)]
    pub skip_rewrite_threshold: Option<f32>,
    #[serde(default)]
    pub keep_revision_history: Option<bool>,
    #[serde(default)]
    pub context_budget_ratio: Option<f32>,
    #[serde(default)]
    pub generation_mode: Option<String>,
    #[serde(default)]
    pub auto_rewrite_severity_threshold: Option<String>,
    #[serde(default)]
    pub llm_connect_timeout_secs: Option<u64>,
    #[serde(default)]
    pub smart_execute_total_timeout_secs: Option<u64>,
    #[serde(default)]
    pub executor_step_timeout_secs: Option<u64>,
    #[serde(default)]
    pub frontend_timeout_secs: Option<u64>,
    #[serde(default)]
    pub llm_first_chunk_timeout_secs: Option<u64>,
    #[serde(default)]
    pub writer_system_prompt_override: Option<String>,
    #[serde(default)]
    pub probe_prompt_override: Option<String>,
}

fn default_concurrency() -> usize {
    3
}

fn default_rewrite_threshold() -> f32 {
    0.75
}

fn default_max_feedback_loops() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralSettings {
    pub theme: String,
    pub language: String,
    pub auto_save: bool,
    pub auto_save_interval: u64,
    pub font_size: u32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivacySettings {
    pub share_usage_data: bool,
}

/// 获取应用设置
#[command]
pub fn get_settings(app_handle: AppHandle) -> Result<AppSettingsData, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // v0.11.0: 统一模型池 — 所有生成模型与嵌入模型合并到一个列表，前端按 type 分组
    let mut all_models: Vec<serde_json::Value> = vec![];

    for p in config.llm_profiles.values() {
        let model_type = match p.kind {
            ModelKind::Chat => "chat",
            ModelKind::Multimodal => "multimodal",
            ModelKind::Image => "image",
        };
        all_models.push(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "provider": p.provider,
            "model_source": p.model_source,
            "model": p.model,
            "type": model_type,
            "temperature": format!("{:.2}", p.temperature).parse::<f64>().unwrap(),
            "max_tokens": p.max_tokens,
            "timeout_seconds": p.timeout_seconds,
            "is_default": p.is_default,
            "enabled": p.enabled,
            "capabilities": p.capabilities,
            "max_context_length": p.max_context_length,
            "quality_tier": p.quality_tier,
            "speed_tier": p.speed_tier,
            "cost_per_1k_input": p.cost_per_1k_input,
            "cost_per_1k_output": p.cost_per_1k_output,
            "tags": p.tags,
            "api_key": if p.api_key.is_empty() { None } else { Some("***") },
            "api_base": p.api_base,
        }));
    }

    for p in config.embedding_profiles.values() {
        all_models.push(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "provider": p.provider,
            "model_source": super::settings::ModelSource::Platform,
            "model": p.model,
            "type": "embedding",
            "dimensions": p.dimensions,
            "max_input_tokens": p.max_input_tokens,
            "is_default": p.is_default,
            "enabled": true,
            "api_key": if p.api_key.is_empty() { None } else { Some("***") },
            "api_base": p.api_base,
        }));
    }

    // 按类型分组，保持前端兼容
    let mut models: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    models.insert(
        "chat".to_string(),
        all_models
            .iter()
            .filter(|m| m["type"] == "chat")
            .cloned()
            .collect(),
    );
    models.insert(
        "multimodal".to_string(),
        all_models
            .iter()
            .filter(|m| m["type"] == "multimodal")
            .cloned()
            .collect(),
    );
    models.insert(
        "image".to_string(),
        all_models
            .iter()
            .filter(|m| m["type"] == "image")
            .cloned()
            .collect(),
    );
    models.insert(
        "embedding".to_string(),
        all_models
            .iter()
            .filter(|m| m["type"] == "embedding")
            .cloned()
            .collect(),
    );

    // v0.11.2: multimodal 复用 active_llm_profile，不应硬编码为空；
    // image 类型当前未独立实现，保持空字符串。
    // v0.23.66: 新增三个模型角色字段
    let active_llm_profile = config.active_llm_profile.clone().unwrap_or_default();
    let active_models = vec![
        ("chat".to_string(), active_llm_profile.clone()),
        (
            "embedding".to_string(),
            config.active_embedding_profile.unwrap_or_default(),
        ),
        ("multimodal".to_string(), active_llm_profile),
        ("image".to_string(), String::new()),
        (
            "creative".to_string(),
            config.creative_model_id.clone().unwrap_or_default(),
        ),
        (
            "tool".to_string(),
            config.tool_model_id.clone().unwrap_or_default(),
        ),
        (
            "background".to_string(),
            config.background_model_id.clone().unwrap_or_default(),
        ),
    ]
    .into_iter()
    .collect();

    let agent_mappings: Vec<AgentMapping> = config.agent_mappings.values().cloned().collect();

    Ok(AppSettingsData {
        models,
        active_models,
        agent_mappings,
        general: GeneralSettings {
            theme: if config.theme.is_empty() {
                "dark".to_string()
            } else {
                config.theme.clone()
            },
            language: if config.language.is_empty() {
                "zh-CN".to_string()
            } else {
                config.language.clone()
            },
            auto_save: config.auto_save,
            auto_save_interval: config.auto_save_interval,
            font_size: config.font_size,
            line_height: config.line_height,
        },
        privacy: PrivacySettings {
            share_usage_data: config.share_usage_data,
        },
        book_deconstruction_concurrency: config.book_deconstruction_concurrency,
        rewrite_threshold: config.rewrite_threshold,
        max_feedback_loops: config.max_feedback_loops,
        writing_strategy: config.writing_strategy.clone(),

        // v0.17.1: 暴露 v0.16.0 引入的高级字段，供前端编辑
        style_weight: Some(config.style_weight),
        narrative_weight: Some(config.narrative_weight),
        skip_rewrite_threshold: Some(config.skip_rewrite_threshold),
        keep_revision_history: Some(config.keep_revision_history),
        context_budget_ratio: Some(config.context_budget_ratio),
        generation_mode: Some(config.generation_mode.clone()),
        llm_connect_timeout_secs: Some(config.llm_connect_timeout_secs),
        smart_execute_total_timeout_secs: Some(config.smart_execute_total_timeout_secs),
        executor_step_timeout_secs: Some(config.executor_step_timeout_secs),
        frontend_timeout_secs: Some(config.frontend_timeout_secs),
        llm_first_chunk_timeout_secs: Some(config.llm_first_chunk_timeout_secs),
        writer_system_prompt_override: Some(config.writer_system_prompt_override.clone()),
        probe_prompt_override: Some(config.probe_prompt_override.clone()),
        auto_rewrite_severity_threshold: Some(config.auto_rewrite_severity_threshold.clone()),
    })
}

/// 保存设置
#[command]
pub fn save_settings(settings: AppSettingsData, app_handle: AppHandle) -> Result<(), AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let mut config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // 保存活跃配置
    if let Some(chat_id) = settings.active_models.get("chat") {
        if !chat_id.is_empty() {
            config.active_llm_profile = Some(chat_id.clone());
        }
    }
    if let Some(emb_id) = settings.active_models.get("embedding") {
        if !emb_id.is_empty() {
            config.active_embedding_profile = Some(emb_id.clone());
        }
    }
    // v0.23.66: 保存模型角色分配
    if let Some(creative_id) = settings.active_models.get("creative") {
        config.creative_model_id = if creative_id.is_empty() {
            None
        } else {
            Some(creative_id.clone())
        };
    }
    if let Some(tool_id) = settings.active_models.get("tool") {
        config.tool_model_id = if tool_id.is_empty() {
            None
        } else {
            Some(tool_id.clone())
        };
    }
    if let Some(bg_id) = settings.active_models.get("background") {
        config.background_model_id = if bg_id.is_empty() {
            None
        } else {
            Some(bg_id.clone())
        };
    }

    // 保存 Agent 映射
    for mapping in settings.agent_mappings {
        config
            .agent_mappings
            .insert(mapping.agent_id.clone(), mapping);
    }

    // 保存拆书并发数
    config.book_deconstruction_concurrency =
        settings.book_deconstruction_concurrency.max(1).min(100);
    // 保存 AgentOrchestrator 配置
    config.rewrite_threshold = settings.rewrite_threshold.clamp(0.0, 1.0);
    config.max_feedback_loops = settings.max_feedback_loops.max(1).min(10);
    // 保存写作策略
    config.writing_strategy = settings.writing_strategy;
    // 保存通用与隐私设置
    config.theme = settings.general.theme;
    config.language = settings.general.language;
    config.auto_save = settings.general.auto_save;
    config.auto_save_interval = settings.general.auto_save_interval;
    config.font_size = settings.general.font_size;
    config.line_height = settings.general.line_height;
    config.share_usage_data = settings.privacy.share_usage_data;

    // v0.17.1: 保存 v0.16.0 高级字段（None 表示前端未触及，保持原值）。
    // 修复「超时设置一改就弹出 保存设置失败: undefined 且数字不变」的根因。
    if let Some(v) = settings.style_weight {
        config.style_weight = v.clamp(0.0, 1.0);
    }
    if let Some(v) = settings.narrative_weight {
        config.narrative_weight = v.clamp(0.0, 1.0);
    }
    if let Some(v) = settings.skip_rewrite_threshold {
        config.skip_rewrite_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = settings.keep_revision_history {
        config.keep_revision_history = v;
    }
    if let Some(v) = settings.context_budget_ratio {
        config.context_budget_ratio = v.clamp(0.1, 1.0);
    }
    if let Some(v) = settings.generation_mode {
        // 仅接受白名单值，避免脏数据
        if matches!(
            v.as_str(),
            "auto" | "time_sliced" | "fast" | "full" | "tri_shot"
        ) {
            config.generation_mode = v;
        }
    }
    if let Some(v) = settings.auto_rewrite_severity_threshold {
        // 仅接受白名单值（与 generation_mode 策略一致）
        if matches!(v.as_str(), "high" | "medium" | "low") {
            config.auto_rewrite_severity_threshold = v;
        }
    }
    if let Some(v) = settings.llm_connect_timeout_secs {
        config.llm_connect_timeout_secs = v.clamp(5, 300);
    }
    if let Some(v) = settings.smart_execute_total_timeout_secs {
        config.smart_execute_total_timeout_secs = v.clamp(30, 600);
    }
    if let Some(v) = settings.executor_step_timeout_secs {
        config.executor_step_timeout_secs = v.clamp(10, 300);
    }
    if let Some(v) = settings.frontend_timeout_secs {
        config.frontend_timeout_secs = v.clamp(30, 600);
    }
    if let Some(v) = settings.llm_first_chunk_timeout_secs {
        config.llm_first_chunk_timeout_secs = v.clamp(5, 300);
    }
    if let Some(v) = settings.writer_system_prompt_override {
        config.writer_system_prompt_override = v;
    }
    if let Some(v) = settings.probe_prompt_override {
        config.probe_prompt_override = v;
    }

    config.save(&app_dir).map_err(AppError::from)
}

/// 导出设置
#[command]
pub fn export_settings(app_handle: AppHandle) -> Result<AppSettingsExport, AppError> {
    let _app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let settings = get_settings(app_handle)?;

    Ok(AppSettingsExport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        settings,
    })
}

/// 导入设置
#[command]
pub fn import_settings(data: AppSettingsExport, app_handle: AppHandle) -> Result<(), AppError> {
    // 验证版本兼容性
    let current_version = env!("CARGO_PKG_VERSION");
    let import_version = &data.version;

    // 简单版本检查（主版本号必须相同）
    let current_major = current_version.split('.').next().unwrap_or("0");
    let import_major = import_version.split('.').next().unwrap_or("0");

    if current_major != import_major {
        return Err(AppError::internal(format!(
            "版本不兼容: 当前版本 {}，导入版本 {}",
            current_version, import_version
        )));
    }

    save_settings(data.settings, app_handle)?;
    Ok(())
}

/// 获取所有模型
#[command]
pub fn get_models(app_handle: AppHandle) -> Result<Vec<serde_json::Value>, AppError> {
    let settings = get_settings(app_handle)?;
    let mut all_models: Vec<serde_json::Value> = vec![];

    for (model_type, models) in settings.models {
        for mut model in models {
            if let Some(obj) = model.as_object_mut() {
                obj.insert("type".to_string(), serde_json::json!(model_type));
            }
            all_models.push(model);
        }
    }

    Ok(all_models)
}

/// 创建模型配置
#[command]
pub fn create_model(
    config: ModelConfigInput,
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let mut app_config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    let model_id = config
        .id
        .clone()
        .unwrap_or_else(|| format!("model-{}", uuid::Uuid::new_v4()));
    let model_name = config.name.clone();
    let model_type_str = format!("{:?}", config.model_type);

    match config.model_type {
        ModelType::Chat => {
            let profile = build_llm_profile(model_id.clone(), &config, false);
            app_config
                .add_llm_profile(profile)
                .map_err(AppError::from)?;
        }
        ModelType::Embedding => {
            let provider = match config.provider.as_str() {
                "azure" => EmbeddingProvider::Azure,
                "ollama" => EmbeddingProvider::Ollama,
                "local" => EmbeddingProvider::Local,
                "custom" => EmbeddingProvider::Custom,
                _ => EmbeddingProvider::OpenAI,
            };

            let profile = EmbeddingProfile {
                id: model_id.clone(),
                name: config.name,
                description: config.description,
                provider,
                model: config.model,
                api_key: config.api_key.unwrap_or_default(),
                api_base: config.api_base,
                dimensions: config.dimensions.unwrap_or(1536),
                max_input_tokens: 8192,
                is_default: config.is_default.unwrap_or(false),
            };

            app_config
                .add_embedding_profile(profile)
                .map_err(AppError::from)?;
        }
        ModelType::Multimodal => {
            let profile = build_llm_profile(model_id.clone(), &config, true);
            app_config
                .add_llm_profile(profile)
                .map_err(AppError::from)?;
        }
        ModelType::Image => {
            // TODO: 实现图像生成模型
            return Err(AppError::internal("图像生成模型暂未实现"));
        }
    }

    app_config.save(&app_dir).map_err(AppError::from)?;

    // v0.11.2: 刷新 LLM 服务内存配置，避免新增/修改模型后仍使用旧适配器或旧活跃模型
    crate::llm::LlmService::new(app_handle.clone()).reload_config();
    log::info!(
        "[create_model] reloaded LLM service config for {}",
        model_id
    );

    // v0.23.13: 新模型加入后立即刷新网关注册表并执行即时健康探测，
    // 探测通过则立刻进入可用模型池，避免调度范围遗漏。
    if let Some(executor) =
        app_handle.try_state::<crate::model_gateway::executor::GatewayExecutor>()
    {
        let executor = executor.inner().clone();
        executor.refresh_registry();
        let probe_model_id = model_id.clone();
        tauri::async_runtime::spawn(async move {
            match executor.probe_model(&probe_model_id).await {
                Ok(result) => {
                    log::info!(
                        "[create_model] 新增模型 {} 即时健康检查完成: success={}",
                        probe_model_id,
                        result.success
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[create_model] 新增模型 {} 即时健康检查失败: {}",
                        probe_model_id,
                        e
                    );
                }
            }
        });
    }

    // 通知 frontstage 刷新模型列表
    let _ = crate::window::WindowManager::send_to_frontstage(
        &app_handle,
        crate::window::FrontstageEvent::DataRefresh {
            entity: "model_config".to_string(),
        },
    );
    crate::state_sync::StateSync::emit_data_refresh(&app_handle, None, "model_config");

    Ok(serde_json::json!({
        "id": model_id,
        "name": model_name,
        "type": model_type_str,
    }))
}

/// 更新模型配置（直接修改，避免 delete+create 导致数据丢失）
#[command]
pub fn update_model(
    id: String,
    config: ModelConfigInput,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let mut app_config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // 确定模型当前在哪个集合中
    let in_llm = app_config.llm_profiles.contains_key(&id);
    let in_embedding = app_config.embedding_profiles.contains_key(&id);

    if !in_llm && !in_embedding {
        return Err(AppError::internal(format!("Model '{}' not found", id)));
    }

    match config.model_type {
        ModelType::Chat | ModelType::Multimodal => {
            // 先处理默认标记（避免与后续 get_mut 冲突）
            if let Some(is_def) = config.is_default {
                if is_def {
                    for p in app_config.llm_profiles.values_mut() {
                        p.is_default = false;
                    }
                }
            }

            let profile = app_config
                .llm_profiles
                .get_mut(&id)
                .ok_or_else(|| format!("Chat/Multimodal model '{}' not found", id))?;

            profile.name = config.name;
            if let Some(desc) = config.description {
                profile.description = if desc.is_empty() { None } else { Some(desc) };
            }
            profile.provider = parse_llm_provider(&config.provider);
            profile.model = config.model;

            // API Key: 前端传了值就更新（包括空字符串表示清空）；None 表示未修改，保留旧值
            if let Some(key) = config.api_key {
                profile.api_key = key;
            }

            if let Some(base) = config.api_base {
                profile.api_base = if base.is_empty() { None } else { Some(base) };
            }
            if let Some(temp) = config.temperature {
                profile.temperature = normalize_temperature(temp);
            }
            if let Some(max_tok) = config.max_tokens {
                profile.max_tokens = max_tok;
            }
            if let Some(caps) = config.capabilities {
                profile.capabilities = parse_capabilities(caps);
            }
            if let Some(is_def) = config.is_default {
                profile.is_default = is_def;
            }
        }
        ModelType::Embedding => {
            // 先处理默认标记（避免与后续 get_mut 冲突）
            if let Some(is_def) = config.is_default {
                if is_def {
                    for p in app_config.embedding_profiles.values_mut() {
                        p.is_default = false;
                    }
                }
            }

            let profile = app_config
                .embedding_profiles
                .get_mut(&id)
                .ok_or_else(|| format!("Embedding model '{}' not found", id))?;

            profile.name = config.name;
            if let Some(desc) = config.description {
                profile.description = if desc.is_empty() { None } else { Some(desc) };
            }
            profile.provider = match config.provider.as_str() {
                "azure" => EmbeddingProvider::Azure,
                "ollama" => EmbeddingProvider::Ollama,
                "local" => EmbeddingProvider::Local,
                "custom" => EmbeddingProvider::Custom,
                _ => EmbeddingProvider::OpenAI,
            };
            profile.model = config.model;

            // API Key: 前端传了值就更新；None 表示未修改，保留旧值
            if let Some(key) = config.api_key {
                profile.api_key = key;
            }

            if let Some(base) = config.api_base {
                profile.api_base = if base.is_empty() { None } else { Some(base) };
            }
            if let Some(dims) = config.dimensions {
                profile.dimensions = dims;
            }
            if let Some(is_def) = config.is_default {
                profile.is_default = is_def;
            }
        }
        ModelType::Image => {
            return Err(AppError::internal("图像生成模型暂未实现"));
        }
    }

    app_config.save(&app_dir).map_err(AppError::from)?;

    // v0.11.2: 刷新 LLM 服务内存配置，确保 api_base/api_key 等变更立即生效
    crate::llm::LlmService::new(app_handle.clone()).reload_config();
    log::info!("[update_model] reloaded LLM service config for {}", id);

    // v0.23.14: 刷新网关注册表（使用新 endpoint/api_key）+ 清除旧健康快照 +
    // 重新探测 防止修改 endpoint 后网关仍用旧连接信息、健康状态仍基于旧
    // endpoint
    if let Some(executor) =
        app_handle.try_state::<crate::model_gateway::executor::GatewayExecutor>()
    {
        let executor = executor.inner().clone();
        executor.refresh_registry();
        // 清除旧健康快照（旧 endpoint 的探测结果不再有效）
        if let Ok(mut health) = executor.health_registry().lock() {
            health.purge(&id);
        }
        // 重新探测，让新 endpoint 立即进入可用池
        let probe_model_id = id.clone();
        tauri::async_runtime::spawn(async move {
            match executor.probe_model(&probe_model_id).await {
                Ok(result) => {
                    log::info!(
                        "[update_model] 模型 {} 修改后重新探测完成: success={}",
                        probe_model_id,
                        result.success
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[update_model] 模型 {} 修改后重新探测失败: {}",
                        probe_model_id,
                        e
                    );
                }
            }
        });
    }

    // 通知 frontstage 刷新模型列表
    let _ = crate::window::WindowManager::send_to_frontstage(
        &app_handle,
        crate::window::FrontstageEvent::DataRefresh {
            entity: "model_config".to_string(),
        },
    );
    crate::state_sync::StateSync::emit_data_refresh(&app_handle, None, "model_config");

    Ok(())
}

/// 获取模型真实 API Key（编辑时明文显示用，不随列表批量暴露）
#[command]
pub fn get_model_api_key(
    model_id: String,
    app_handle: AppHandle,
) -> Result<Option<String>, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    if let Some(p) = config.llm_profiles.get(&model_id) {
        return Ok(if p.api_key.is_empty() {
            None
        } else {
            Some(p.api_key.clone())
        });
    }
    if let Some(p) = config.embedding_profiles.get(&model_id) {
        return Ok(if p.api_key.is_empty() {
            None
        } else {
            Some(p.api_key.clone())
        });
    }

    Err(AppError::internal(format!(
        "Model '{}' not found",
        model_id
    )))
}

/// 删除模型配置
#[command]
pub fn delete_model(id: String, app_handle: AppHandle) -> Result<(), AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let mut config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // v0.11.2: 删除成功后必须持久化，不能依赖条件变量；避免"toast 成功但刷新后仍在"
    let mut changed;

    // 尝试删除LLM配置
    if config.llm_profiles.contains_key(&id) {
        config.remove_llm_profile(&id).map_err(AppError::from)?;
        changed = true;
    }
    // 尝试删除Embedding配置
    else if config.embedding_profiles.contains_key(&id) {
        config
            .remove_embedding_profile(&id)
            .map_err(AppError::from)?;
        changed = true;
    } else {
        return Err(AppError::internal(format!("Model '{}' not found", id)));
    }

    // 清理 Agent 映射中引用该模型的字段，避免已删除模型仍被使用
    for mapping in config.agent_mappings.values_mut() {
        if mapping.chat_model_id.as_ref() == Some(&id) {
            mapping.chat_model_id = None;
            changed = true;
        }
        if mapping.embedding_model_id.as_ref() == Some(&id) {
            mapping.embedding_model_id = None;
            changed = true;
        }
        if mapping.multimodal_model_id.as_ref() == Some(&id) {
            mapping.multimodal_model_id = None;
            changed = true;
        }
    }

    // 如果当前活跃模型就是被删除的模型，重置为剩余配置中第一个启用的模型
    // v0.23.14: 增加 enabled 过滤，避免重置到 disabled 模型
    if config.active_llm_profile.as_ref() == Some(&id) {
        config.active_llm_profile = config
            .llm_profiles
            .values()
            .find(|p| p.enabled)
            .map(|p| p.id.clone());
        changed = true;
    }
    if config.active_embedding_profile.as_ref() == Some(&id) {
        config.active_embedding_profile = config.embedding_profiles.keys().next().cloned();
        changed = true;
    }

    log::info!(
        "[delete_model] removed model {}, changed={}, saving...",
        id,
        changed
    );

    // v0.11.2: 只要走到这里说明删除成功，必须保存配置，不再受 changed 条件限制
    config.save(&app_dir).map_err(AppError::from)?;

    // v0.11.2: 删除模型后立即刷新 LLM 服务内存配置，避免后续生成仍使用已删除模型
    crate::llm::LlmService::new(app_handle.clone()).reload_config();
    log::info!(
        "[delete_model] reloaded LLM service config after removing {}",
        id
    );

    // v0.23.14: 级联清理 —— 彻底清除死模型残留，防止候选链/健康报告污染
    // 1. 刷新网关注册表（从最新 config 重建，移除已删除模型）+ 清除健康快照
    if let Some(executor) =
        app_handle.try_state::<crate::model_gateway::executor::GatewayExecutor>()
    {
        executor.refresh_registry();
        if let Ok(mut health) = executor.health_registry().lock() {
            health.purge(&id);
        }
        log::info!(
            "[delete_model] purged gateway registry + health snapshot for {}",
            id
        );
    }
    // 2. 清除 llm_calls 表中该模型的历史调用记录
    if let Some(pool) = app_handle.try_state::<crate::db::DbPool>() {
        let repo = crate::db::repositories_pipeline::LlmCallRepository::new(pool.inner().clone());
        match repo.delete_by_model_id(&id) {
            Ok(n) => log::info!("[delete_model] purged {} llm_calls for {}", n, id),
            Err(e) => log::warn!("[delete_model] failed to purge llm_calls: {}", e),
        }
    }

    // v0.11.2: 通知 frontstage 刷新模型状态，保持多窗口/多组件数据一致
    let _ = crate::window::WindowManager::send_to_frontstage(
        &app_handle,
        crate::window::FrontstageEvent::DataRefresh {
            entity: "model_config".to_string(),
        },
    );

    Ok(())
}

/// 设置活跃模型（v0.23.66: 支持 role="creative"/"tool"/"background" 角色参数）
#[command]
pub fn set_active_model(
    model_type: String,
    model_id: String,
    role: Option<String>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let mut config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // v0.23.66: 按角色设置对应字段
    if let Some(ref role_str) = role {
        let model_role = match role_str.as_str() {
            "creative" => crate::config::settings::ModelRole::Creative,
            "tool" => crate::config::settings::ModelRole::Tool,
            "background" => crate::config::settings::ModelRole::Background,
            _ => {
                return Err(AppError::internal(format!(
                    "Unknown model role: {}",
                    role_str
                )))
            }
        };
        config
            .set_model_for_role(model_role, &model_id)
            .map_err(AppError::from)?;
    } else {
        match model_type.as_str() {
            "chat" => config
                .set_active_llm_profile(&model_id)
                .map_err(AppError::from)?,
            "multimodal" => config
                .set_active_llm_profile(&model_id)
                .map_err(AppError::from)?,
            "embedding" => config
                .set_active_embedding_profile(&model_id)
                .map_err(AppError::from)?,
            _ => {
                return Err(AppError::internal(format!(
                    "Unknown model type: {}",
                    model_type
                )))
            }
        }
    }

    config.save(&app_dir).map_err(AppError::from)?;

    // v0.11.2: 刷新 LLM 服务内存配置，确保后续生成请求立即使用新活跃模型
    crate::llm::LlmService::new(app_handle.clone()).reload_config();
    log::info!(
        "[set_active_model] reloaded LLM service config for {}",
        model_id
    );

    // v0.23.14: 刷新网关注册表，确保候选链立即反映新的活跃模型
    if let Some(executor) =
        app_handle.try_state::<crate::model_gateway::executor::GatewayExecutor>()
    {
        executor.refresh_registry();
    }

    // 通知幕前窗口刷新模型状态
    let _ = crate::window::WindowManager::send_to_frontstage(
        &app_handle,
        crate::window::FrontstageEvent::DataRefresh {
            entity: "model_config".to_string(),
        },
    );

    // v0.11.2: 同时通过 sync-event 通知所有窗口（包括 backstage）刷新模型配置
    crate::state_sync::StateSync::emit_data_refresh(&app_handle, None, "model_config");

    Ok(())
}

/// 获取Agent模型映射
#[command]
pub fn get_agent_mappings(app_handle: AppHandle) -> Result<Vec<AgentMapping>, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;
    Ok(config.agent_mappings.values().cloned().collect())
}

/// 更新Agent模型映射
#[command]
pub fn update_agent_mapping(mapping: AgentMapping, app_handle: AppHandle) -> Result<(), AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;

    let mut config = AppConfig::load(&app_dir).map_err(AppError::from)?;
    config
        .agent_mappings
        .insert(mapping.agent_id.clone(), mapping);
    config.save(&app_dir).map_err(AppError::from)?;
    Ok(())
}

/// 单步探测结果
#[derive(Debug, Serialize)]
struct ConnectionStep {
    name: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 测试模型连接，返回带 `steps` 字段的详细探测结果
#[command]
pub async fn test_model_connection(
    model_id: String,
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    // 在 LLM profiles 和 embedding profiles 中查找真实配置
    let mut found_profile: Option<(String, Option<String>)> = None;
    for p in config.llm_profiles.values() {
        if p.id == model_id {
            found_profile = Some((
                p.api_base.clone().unwrap_or_default(),
                Some(p.api_key.clone()),
            ));
            break;
        }
    }
    if found_profile.is_none() {
        for p in config.embedding_profiles.values() {
            if p.id == model_id {
                found_profile = Some((
                    p.api_base.clone().unwrap_or_default(),
                    Some(p.api_key.clone()),
                ));
                break;
            }
        }
    }

    let (api_base, api_key) =
        found_profile.ok_or_else(|| format!("Model '{}' not found", model_id))?;
    if api_base.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "latency": 0,
            "error": "未配置 API Base",
            "steps": [],
        }));
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(AppError::from)?;

    let api_key_ref = api_key.as_deref();
    let mut steps: Vec<ConnectionStep> = Vec::new();
    let mut connected = false;

    // 1. GET base_url（根路径）
    let step1 = match client.get(&api_base).send().await {
        Ok(_) => {
            connected = true;
            ConnectionStep {
                name: "GET root".to_string(),
                success: true,
                error: None,
            }
        }
        Err(e) => ConnectionStep {
            name: "GET root".to_string(),
            success: false,
            error: Some(e.to_string()),
        },
    };
    steps.push(step1);

    // 2. GET /models
    if !connected {
        let mut req = client.get(format!("{}/models", api_base));
        if let Some(key) = api_key_ref {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }
        let step2 = match req.send().await {
            Ok(_) => {
                connected = true;
                ConnectionStep {
                    name: "GET /models".to_string(),
                    success: true,
                    error: None,
                }
            }
            Err(e) => ConnectionStep {
                name: "GET /models".to_string(),
                success: false,
                error: Some(e.to_string()),
            },
        };
        steps.push(step2);
    }

    // 3. POST /chat/completions
    if !connected {
        let mut req = client.post(format!("{}/chat/completions", api_base));
        if let Some(key) = api_key_ref {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }
        req = req.header("Content-Type", "application/json");
        let step3 = match req
            .body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#)
            .send()
            .await
        {
            Ok(_) => {
                connected = true;
                ConnectionStep {
                    name: "POST /chat/completions".to_string(),
                    success: true,
                    error: None,
                }
            }
            Err(e) => ConnectionStep {
                name: "POST /chat/completions".to_string(),
                success: false,
                error: Some(e.to_string()),
            },
        };
        steps.push(step3);
    }

    // 4. POST /v1/chat/completions
    if !connected {
        let mut req = client.post(format!("{}/v1/chat/completions", api_base));
        if let Some(key) = api_key_ref {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }
        req = req.header("Content-Type", "application/json");
        let step4 = match req
            .body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#)
            .send()
            .await
        {
            Ok(_) => {
                connected = true;
                ConnectionStep {
                    name: "POST /v1/chat/completions".to_string(),
                    success: true,
                    error: None,
                }
            }
            Err(e) => ConnectionStep {
                name: "POST /v1/chat/completions".to_string(),
                success: false,
                error: Some(e.to_string()),
            },
        };
        steps.push(step4);
    }

    if connected {
        let latency = start.elapsed().as_millis() as u64;
        Ok(serde_json::json!({
            "success": true,
            "latency": latency,
            "steps": steps,
        }))
    } else {
        Ok(serde_json::json!({
            "success": false,
            "latency": 0,
            "error": "无法连接到模型服务".to_string(),
            "steps": steps,
        }))
    }
}

/// 从 API 地址获取可用模型列表
#[command]
pub async fn fetch_models(
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<String>, AppError> {
    if base_url.is_empty() {
        return Err(AppError::internal("API Base 地址不能为空"));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(AppError::from)?;

    let urls = if base_url.ends_with("/v1") {
        vec![format!("{}/models", base_url)]
    } else {
        vec![
            format!("{}/v1/models", base_url),
            format!("{}/models", base_url),
        ]
    };

    for url in urls {
        let mut req = client.get(&url);
        if let Some(ref key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        let ids: Vec<String> = data
                            .get("data")
                            .and_then(|d| d.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|m| {
                                        m.get("id")
                                            .and_then(|id| id.as_str())
                                            .map(|s| s.to_string())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !ids.is_empty() {
                            return Ok(ids);
                        }
                    }
                    Err(_) => continue,
                }
            }
            _ => continue,
        }
    }

    Err(AppError::internal(
        "无法从该 API 地址获取模型列表，请检查地址和密钥是否正确",
    ))
}
