use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use super::*;
use crate::{error::AppError, router::TaskType};

fn task_type_for_category(category: &SkillCategory) -> TaskType {
    match category {
        SkillCategory::Analysis | SkillCategory::Plot => TaskType::Analysis,
        SkillCategory::Style => TaskType::Editing,
        SkillCategory::Export => TaskType::Summarization,
        SkillCategory::WorldBuilding | SkillCategory::Character => TaskType::WorldBuilding,
        SkillCategory::Writing | SkillCategory::Integration | SkillCategory::Custom => {
            TaskType::CreativeWriting
        }
    }
}

/// 将 skill_id 映射到 PromptRegistry 中的 prompt_id
fn skill_id_to_prompt_id(skill_id: &str) -> String {
    match skill_id {
        "builtin.style_enhancer" => "skill_style_enhancer",
        "builtin.plot_twist" => "skill_plot_twist",
        "builtin.text_formatter" => "skill_text_formatter",
        "builtin.character_voice" => "skill_character_voice",
        "builtin.emotion_pacing" => "skill_emotion_pacing",
        _ => skill_id,
    }
    .to_string()
}

#[derive(Clone)]
pub struct SkillExecutor {
    registry: Arc<Mutex<SkillRegistry>>,
    llm_service: Option<crate::llm::LlmService>,
    db_pool: Option<crate::db::DbPool>,
}

impl SkillExecutor {
    pub fn new(
        registry: Arc<Mutex<SkillRegistry>>,
        llm_service: Option<crate::llm::LlmService>,
    ) -> Self {
        Self {
            registry,
            llm_service,
            db_pool: None,
        }
    }

    /// 设置数据库连接池，用于读取提示词覆盖
    pub fn with_db_pool(mut self, pool: crate::db::DbPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Execute a skill
    pub async fn execute(
        &self,
        skill_id: &str,
        context: &AgentContext,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<SkillResult, AppError> {
        let start = Instant::now();

        let skill = self
            .registry
            .lock()
            .unwrap()
            .get(skill_id)
            .ok_or_else(|| "Skill not found".to_string())?;

        if !skill.is_enabled {
            return Err(AppError::internal("Skill is disabled"));
        }

        // Validate parameters and merge defaults / config / caller params
        self.validate_params(&skill.manifest, &params)?;
        let merged_params = self.merge_params(&skill.manifest, params);

        // Execute based on runtime
        let result = match &skill.runtime {
            SkillRuntime::Prompt(runtime) => {
                self.execute_prompt(runtime, &skill.manifest, context, merged_params)
                    .await
            }
            SkillRuntime::Mcp(runtime) => self.execute_mcp(runtime, context, merged_params).await,
            SkillRuntime::Native(runtime) => runtime
                .handler
                .execute(context, merged_params)
                .map_err(AppError::from),
        };

        let execution_time_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut r) => {
                r.execution_time_ms = execution_time_ms;
                Ok(r)
            }
            Err(e) => Ok(SkillResult {
                success: false,
                data: serde_json::Value::Null,
                error: Some(e.to_string()),
                execution_time_ms,
            }),
        }
    }

    /// Execute hooks for an event
    pub async fn execute_hooks(
        &self,
        event: HookEvent,
        context: &AgentContext,
        data: serde_json::Value,
    ) -> Vec<SkillResult> {
        let skills = self.registry.lock().unwrap().get_hook_handlers(&event);

        let mut results = Vec::new();

        for skill in skills {
            let params = HashMap::from([("event_data".to_string(), data.clone())]);

            match self.execute(&skill.manifest.id, context, params).await {
                Ok(result) => results.push(result),
                Err(e) => results.push(SkillResult {
                    success: false,
                    data: serde_json::Value::Null,
                    error: Some(e.to_string()),
                    execution_time_ms: 0,
                }),
            }
        }

        results
    }

    fn validate_params(
        &self,
        manifest: &SkillManifest,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<(), AppError> {
        for param in &manifest.parameters {
            if param.required && !params.contains_key(&param.name) {
                if param.default.is_none() {
                    return Err(AppError::internal(format!(
                        "Missing required parameter: {}",
                        param.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// 合并参数优先级：调用参数 > skill config > parameter default
    fn merge_params(
        &self,
        manifest: &SkillManifest,
        params: HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        let mut merged = HashMap::new();

        // 1. parameter defaults
        for param in &manifest.parameters {
            if let Some(default) = &param.default {
                merged.insert(param.name.clone(), default.clone());
            }
        }

        // 2. skill config (lower priority than explicit params)
        for (key, value) in &manifest.config {
            merged.insert(key.clone(), value.clone());
        }

        // 3. caller params (highest priority)
        for (key, value) in params {
            merged.insert(key, value);
        }

        merged
    }

    /// 从 skill config 解析 f64 配置项
    fn config_f64(&self, manifest: &SkillManifest, key: &str, fallback: f64) -> f64 {
        manifest
            .config
            .get(key)
            .and_then(|v| v.as_f64())
            .unwrap_or(fallback)
    }

    async fn execute_prompt(
        &self,
        runtime: &PromptRuntime,
        manifest: &SkillManifest,
        context: &AgentContext,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<SkillResult, AppError> {
        // v0.19.0: 从 PromptRegistry 读取提示词覆盖
        let (system_prompt, user_prompt_template) =
            self.resolve_skill_prompts(&manifest.id, runtime).await;

        // Build user prompt from template
        let mut user_prompt = user_prompt_template;

        // Simple template substitution
        for (key, value) in &params {
            let placeholder = format!("{{{}}}", key);
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            user_prompt = user_prompt.replace(&placeholder, &value_str);
        }

        // Add context info
        let context_info = format!(
            "Story: {}\nGenre: {}\nTone: {}\nChapter: {}\n",
            context.story.story_title,
            context.story.genre,
            context.story.tone,
            context.narrative.chapter_number
        );

        user_prompt = format!("{}\n\n{}", context_info, user_prompt);

        // Resolve generation parameters from skill config
        let max_tokens = self
            .config_f64(manifest, "max_tokens", 2000.0)
            .clamp(1.0, 16384.0) as i32;
        let temperature = self
            .config_f64(manifest, "temperature", 0.7)
            .clamp(0.0, 2.0) as f32;

        // Call LLM if service is available
        if let Some(ref llm) = self.llm_service {
            let full_prompt = if system_prompt.is_empty() {
                user_prompt
            } else {
                format!(
                    "[系统指令]\n{}\n\n[用户请求]\n{}",
                    system_prompt, user_prompt
                )
            };

            let task_type = task_type_for_category(&manifest.category);
            let response = llm
                .generate_for_task(
                    task_type,
                    full_prompt,
                    Some(max_tokens),
                    Some(temperature),
                    Some(&manifest.id),
                )
                .await?;

            Ok(SkillResult {
                success: true,
                data: serde_json::json!({
                    "content": response.content,
                    "model": response.model,
                    "tokens_used": response.tokens_used,
                }),
                error: None,
                execution_time_ms: 0,
            })
        } else {
            // Fallback: return the prompt for external LLM call
            Ok(SkillResult {
                success: true,
                data: serde_json::json!({
                    "system_prompt": system_prompt,
                    "user_prompt": user_prompt,
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                }),
                error: None,
                execution_time_ms: 0,
            })
        }
    }

    /// 根据 skill_id 映射到 prompt_id，从 PromptRegistry 读取覆盖
    async fn resolve_skill_prompts(
        &self,
        skill_id: &str,
        runtime: &PromptRuntime,
    ) -> (String, String) {
        let prompt_id = skill_id_to_prompt_id(skill_id);

        // 优先从数据库读取覆盖
        if let Some(ref pool) = self.db_pool {
            if let Ok(content) = crate::prompts::registry::resolve_prompt(pool, &prompt_id) {
                // 技能提示词在 registry 中是完整提示词（system + user 合并），
                // 但 builtin.rs 中 skills 使用分开的 system_prompt /
                // user_prompt_template
                // 这里保持简单：如果 registry 中有覆盖，将覆盖内容作为
                // system_prompt 并将 user_prompt_template
                // 保持原样（因为 registry 中的 skill 提示词是单条）
                // 实际上，我们需要更精细的区分...
                // 为了兼容，我们采用：registry 覆盖 system_prompt，保留原始
                // user_prompt_template
                return (content, runtime.user_prompt_template.clone());
            }
        }

        // 无数据库或读取失败，尝试默认解析
        if let Some(content) = crate::prompts::registry::resolve_prompt_default(&prompt_id) {
            return (content, runtime.user_prompt_template.clone());
        }

        // 无覆盖，使用原始 PromptRuntime
        (
            runtime.system_prompt.clone(),
            runtime.user_prompt_template.clone(),
        )
    }

    async fn execute_mcp(
        &self,
        runtime: &McpRuntime,
        _context: &AgentContext,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<SkillResult, AppError> {
        let mcp_config = crate::mcp::types::McpServerConfig {
            id: "skill-mcp".to_string(),
            name: runtime.server_config.command.clone(),
            command: runtime.server_config.command.clone(),
            args: runtime.server_config.args.clone(),
            env: runtime.server_config.env.clone(),
            timeout_seconds: 30,
        };
        let mut client = crate::mcp::McpClient::new(mcp_config);

        match client.connect().await {
            Ok(_) => {
                let tool_name = params
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                if tool_name.is_empty() {
                    let tools = client.get_tools();
                    return Ok(SkillResult {
                        success: true,
                        data: serde_json::json!({
                            "available_tools": tools,
                            "message": "Connected. Specify 'tool_name' and 'arguments' to call a tool.",
                        }),
                        error: None,
                        execution_time_ms: 0,
                    });
                }

                match client.call_tool(tool_name, arguments.clone()).await {
                    Ok(result) => {
                        let _ = client.disconnect().await;
                        Ok(SkillResult {
                            success: true,
                            data: result,
                            error: None,
                            execution_time_ms: 0,
                        })
                    }
                    Err(e) => {
                        let _ = client.disconnect().await;
                        Err(AppError::internal(format!("MCP tool call failed: {}", e)))
                    }
                }
            }
            Err(e) => Err(AppError::internal(format!("MCP connection failed: {}", e))),
        }
    }
}
