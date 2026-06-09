//! Core commands

use tauri::{AppHandle, Manager, State};

use crate::{db::DbPool, error::AppError, ChatMessageItem};

#[tauri::command(rename_all = "snake_case")]
pub fn health_check(_pool: State<'_, DbPool>) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn chat_completion(
    _pool: State<'_, DbPool>,
    base_url: String,
    api_key: Option<String>,
    model: String,
    messages: Vec<ChatMessageItem>,
    max_tokens: i32,
    temperature: f32,
) -> Result<serde_json::Value, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(AppError::from)?;

    let mut request = client
        .post(format!("{}/chat/completions", base_url))
        .header("Content-Type", "application/json");

    if let Some(ref key) = api_key {
        if !key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", key));
        }
    }

    let body = serde_json::json!({
        "model": model,
        "messages": messages.iter().map(|m| serde_json::json!({
            "role": m.role,
            "content": m.content
        })).collect::<Vec<_>>(),
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": false,
    });

    let mut response = request.json(&body).send().await.map_err(AppError::from)?;

    // Ollama 等本地服务的 OpenAI 兼容 API 使用 /v1/chat/completions
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let mut fallback = client
            .post(format!("{}/v1/chat/completions", base_url))
            .header("Content-Type", "application/json");
        if let Some(ref key) = api_key {
            if !key.is_empty() {
                fallback = fallback.header("Authorization", format!("Bearer {}", key));
            }
        }
        response = fallback.json(&body).send().await.map_err(AppError::from)?;
    }

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("HTTP {}: {}", status, text)));
    }

    let data: serde_json::Value = response.json().await.map_err(AppError::from)?;
    Ok(data)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn check_model_status(
    _pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("Failed to get app dir: {}", e)))?;
    let config = crate::config::AppConfig::load(&app_dir).map_err(AppError::from)?;
    let active_profile_id = config
        .active_llm_profile
        .as_deref()
        .or(config
            .llm_profiles
            .values()
            .find(|p| p.is_default)
            .map(|p| p.id.as_str()))
        .or(config.llm_profiles.keys().next().map(|s| s.as_str()))
        .ok_or("No LLM profile configured")?;

    let profile = config
        .llm_profiles
        .get(active_profile_id)
        .ok_or("Active LLM profile not found")?;

    let base_url = profile
        .api_base
        .clone()
        .or(config.llm.api_base.clone())
        .unwrap_or_else(|| match profile.provider {
            crate::config::settings::LlmProvider::OpenAI => "https://api.openai.com/v1".to_string(),
            crate::config::settings::LlmProvider::Anthropic => {
                "https://api.anthropic.com".to_string()
            }
            crate::config::settings::LlmProvider::Ollama => "http://localhost:11434".to_string(),
            crate::config::settings::LlmProvider::DeepSeek => {
                "https://api.deepseek.com".to_string()
            }
            _ => "http://localhost:11434".to_string(),
        });

    let api_key = if profile.api_key.is_empty() {
        config.llm.api_key.clone()
    } else {
        profile.api_key.clone()
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(AppError::from)?;

    let api_key_ref = if api_key.is_empty() {
        None
    } else {
        Some(api_key.as_str())
    };

    // 探测策略：只要收到任何 HTTP 响应（不论状态码）即视为网络可通
    // 1. GET base_url（根路径，最宽容）
    if client.get(&base_url).send().await.is_ok() {
        return Ok("connected".to_string());
    }

    // 2. GET /models（OpenAI 标准）
    let mut req = client.get(format!("{}/models", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    if req.send().await.is_ok() {
        return Ok("connected".to_string());
    }

    // 3. POST /chat/completions
    let mut req = client.post(format!("{}/chat/completions", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    req = req.header("Content-Type", "application/json");
    if req
        .body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#)
        .send()
        .await
        .is_ok()
    {
        return Ok("connected".to_string());
    }

    // 4. POST /v1/chat/completions（部分服务 base_url 不含 /v1）
    let mut req = client.post(format!("{}/v1/chat/completions", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    req = req.header("Content-Type", "application/json");
    if req
        .body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#)
        .send()
        .await
        .is_ok()
    {
        return Ok("connected".to_string());
    }

    Ok("disconnected".to_string())
}
