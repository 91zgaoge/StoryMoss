//! StoryMoss Server HTTP 客户端（订阅权威源 + 桌面 OAuth 中转）

use reqwest::Client;
use serde::Deserialize;

use crate::error::AppError;

const DEFAULT_BASE_URL: &str = "https://storymoss.top";

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteSubscription {
    pub tier: String,
    pub status: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DesktopLogin {
    pub token: String,
    pub user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct PollResponse {
    token: String,
    user: PollUser,
}

#[derive(Deserialize)]
struct PollUser {
    id: String,
    email: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

pub struct ServerClient {
    base_url: String,
    client: Client,
}

impl ServerClient {
    pub fn new() -> Self {
        let base_url =
            std::env::var("STORYMOSS_SERVER_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub fn desktop_auth_url(&self, provider: &str, dstate: &str, invite: Option<&str>) -> String {
        let mut url = format!(
            "{}/api/auth/{}/start?client=desktop&dstate={}",
            self.base_url, provider, dstate
        );
        if let Some(code) = invite.filter(|c| !c.trim().is_empty()) {
            url.push_str(&format!("&invite={}", urlencoding::encode(code.trim())));
        }
        url
    }

    /// Ok(None) = pending；Err = 过期或网络失败；403 + `{"error": code}` =
    /// 登录失败 （以 `AUTH_FAILED:<code>` 前缀上抛，前端按 code
    /// 映射文案并终止轮询）
    pub async fn desktop_poll(&self, dstate: &str) -> Result<Option<DesktopLogin>, AppError> {
        let resp = self
            .client
            .get(format!("{}/api/auth/desktop-poll", self.base_url))
            .query(&[("dstate", dstate)])
            .send()
            .await
            .map_err(|e| format!("desktop poll failed: {}", e))?;
        match resp.status().as_u16() {
            202 => Ok(None),
            200 => {
                let body: PollResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("bad poll body: {}", e))?;
                Ok(Some(DesktopLogin {
                    token: body.token,
                    user_id: body.user.id,
                    email: body.user.email,
                    display_name: body.user.display_name,
                    avatar_url: body.user.avatar_url,
                }))
            }
            403 => {
                let code = resp
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|v| {
                        v.get("error")
                            .and_then(|e| e.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                Err(format!("AUTH_FAILED:{}", code).into())
            }
            s => Err(format!("desktop poll status {}", s).into()),
        }
    }

    pub async fn get_subscription(&self, token: &str) -> Result<RemoteSubscription, AppError> {
        let resp = self
            .client
            .get(format!("{}/api/subscription/me", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| format!("get subscription failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("subscription status {}", resp.status()).into());
        }
        resp.json()
            .await
            .map_err(|e| format!("bad subscription body: {}", e).into())
    }

    pub async fn dev_upgrade(
        &self,
        token: &str,
        tier: &str,
    ) -> Result<RemoteSubscription, AppError> {
        let resp = self
            .client
            .post(format!("{}/api/subscription/dev-upgrade", self.base_url))
            .bearer_auth(token)
            .json(&serde_json::json!({"tier": tier}))
            .send()
            .await
            .map_err(|e| format!("dev upgrade failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("dev upgrade status {}", resp.status()).into());
        }
        resp.json()
            .await
            .map_err(|e| format!("bad upgrade body: {}", e).into())
    }

    /// v0.89.0: 上传日志压缩包（multipart）。返回 (accepted, duplicate)——
    /// 服务端对同 sha256 幂等返回 409，视为 duplicate=true 不算错误。
    pub async fn upload_log_package(
        &self,
        token: &str,
        bytes: Vec<u8>,
        filename: &str,
        sha256: &str,
    ) -> Result<(bool, bool), AppError> {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str("application/zip")
            .map_err(|e| AppError::internal(format!("mime: {e}")))?;
        let form = reqwest::multipart::Form::new()
            .text("sha256", sha256.to_string())
            .part("file", part);
        let resp = self
            .client
            .post(format!("{}/api/logs/upload", self.base_url))
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("上传日志包失败: {e}")))?;
        match resp.status().as_u16() {
            200 | 201 => Ok((true, false)),
            409 => Ok((false, true)),
            401 => Err(AppError::validation_failed(
                "登录已过期，请重新登录后再上传日志",
                None::<String>,
            )),
            s => {
                let body = resp.text().await.unwrap_or_default();
                Err(AppError::internal(format!(
                    "日志上传失败（HTTP {s}）: {}",
                    &body[..body.floor_char_boundary(body.len().min(200))]
                )))
            }
        }
    }

    pub async fn logout(&self, token: &str) -> Result<(), AppError> {
        let _ = self
            .client
            .post(format!("{}/api/auth/logout", self.base_url))
            .bearer_auth(token)
            .send()
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_auth_url_format() {
        let c = ServerClient::new();
        assert_eq!(
            c.desktop_auth_url("google", "abc", None),
            "https://storymoss.top/api/auth/google/start?client=desktop&dstate=abc"
        );
        assert_eq!(
            c.desktop_auth_url("google", "abc", Some("BETA 1")),
            "https://storymoss.top/api/auth/google/start?client=desktop&dstate=abc&invite=BETA%201"
        );
    }

    #[test]
    fn remote_subscription_parses_server_json() {
        let s: RemoteSubscription = serde_json::from_str(
            r#"{"user_id":"u","tier":"pro","status":"active","expires_at":null}"#,
        )
        .unwrap();
        assert_eq!(s.tier, "pro");
        assert_eq!(s.expires_at, None);
    }
}
