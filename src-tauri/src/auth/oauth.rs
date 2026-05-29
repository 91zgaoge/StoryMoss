//! OAuth2 流程实现
//!
//! 参考开源 oauth2-rs crate 的 PKCE + Authorization Code 流程
//! Google: https://developers.google.com/identity/protocols/oauth2/native-app
//! GitHub: https://docs.github.com/en/developers/apps/building-oauth-apps/authorizing-oauth-apps

use std::{collections::HashMap, sync::Mutex};

use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse, TokenUrl,
};
use once_cell::sync::Lazy;

use super::OAuthProvider;
use crate::error::AppError;

/// 存储正在进行的OAuth流程状态（state -> PKCE verifier映射）
static OAUTH_STATE_STORE: Lazy<Mutex<HashMap<String, OAuthState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct OAuthState {
    pub provider: OAuthProvider,
    pub pkce_verifier: String,
    #[allow(dead_code)]
    pub redirect_port: u16,
}

/// 用户资料（从OAuth provider获取）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthUserProfile {
    pub provider: String,
    pub provider_account_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Local>>,
}

/// 开始OAuth流程，返回授权URL和state
pub fn start_oauth_flow(
    provider: OAuthProvider,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<(String, String, u16), AppError> {
    let (auth_url, token_url, _revoke_url) = get_provider_urls(provider);

    // 构建 OAuth client（oauth2 v5 类型状态模式）
    let mut client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url);

    if let Some(secret) = client_secret {
        client = client.set_client_secret(ClientSecret::new(secret.to_string()));
    }

    // 生成 PKCE 挑战
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // 生成 CSRF state
    let (auth_url_obj, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    let state = csrf_token.secret().clone();

    // 找到可用端口
    let port = find_available_port()?;

    // 存储state
    let oauth_state = OAuthState {
        provider,
        pkce_verifier: pkce_verifier.secret().clone(),
        redirect_port: port,
    };

    {
        let mut store = OAUTH_STATE_STORE.lock().unwrap();
        store.insert(state.clone(), oauth_state);
    }

    let auth_url_str = auth_url_obj.to_string();

    Ok((auth_url_str, state, port))
}

/// 验证回调参数，用code交换token，获取用户资料
pub async fn handle_oauth_callback(
    state: &str,
    code: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<OAuthUserProfile, AppError> {
    // 查找并移除state
    let oauth_state = {
        let mut store = OAUTH_STATE_STORE.lock().unwrap();
        store
            .remove(state)
            .ok_or("Invalid or expired OAuth state")?
    };

    let provider = oauth_state.provider;

    let (auth_url, token_url, _revoke_url) = get_provider_urls(provider);

    // 构建 OAuth client
    let mut client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url);

    if let Some(secret) = client_secret {
        client = client.set_client_secret(ClientSecret::new(secret.to_string()));
    }

    // 用code交换token
    let pkce_verifier = PkceCodeVerifier::new(oauth_state.pkce_verifier);

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let token_result = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let access_token = token_result.access_token().secret().clone();
    let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());
    let expires_at = token_result.expires_in().map(|d| {
        chrono::Local::now()
            + chrono::Duration::from_std(d).unwrap_or(chrono::Duration::seconds(3600))
    });

    // 获取用户资料
    let profile = match provider {
        OAuthProvider::Google => fetch_google_user_info(&access_token).await?,
        OAuthProvider::Github => fetch_github_user_info(&access_token).await?,
        _ => {
            return Err(AppError::internal(format!(
                "Provider {:?} not yet implemented",
                provider
            )))
        }
    };

    Ok(OAuthUserProfile {
        provider: provider.to_string(),
        provider_account_id: profile.provider_account_id,
        email: profile.email,
        display_name: profile.display_name,
        avatar_url: profile.avatar_url,
        access_token,
        refresh_token,
        expires_at,
    })
}

/// 获取各Provider的OAuth URL
fn get_provider_urls(
    provider: OAuthProvider,
) -> (AuthUrl, TokenUrl, Option<oauth2::RevocationUrl>) {
    match provider {
        OAuthProvider::Google => (
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap(),
            Some(
                oauth2::RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())
                    .unwrap(),
            ),
        ),
        OAuthProvider::Github => (
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
            TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap(),
            None,
        ),
        OAuthProvider::Wechat => (
            AuthUrl::new("https://open.weixin.qq.com/connect/qrconnect".to_string()).unwrap(),
            TokenUrl::new("https://api.weixin.qq.com/sns/oauth2/access_token".to_string()).unwrap(),
            None,
        ),
        OAuthProvider::Qq => (
            AuthUrl::new("https://graph.qq.com/oauth2.0/authorize".to_string()).unwrap(),
            TokenUrl::new("https://graph.qq.com/oauth2.0/token".to_string()).unwrap(),
            None,
        ),
    }
}

/// 获取Google用户信息
async fn fetch_google_user_info(access_token: &str) -> Result<OAuthUserProfile, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Google user info: {}", e))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("Google API error: {}", text)));
    }

    let data: serde_json::Value = response.json().await.map_err(AppError::from)?;

    Ok(OAuthUserProfile {
        provider: "google".to_string(),
        provider_account_id: data["id"].as_str().unwrap_or("").to_string(),
        email: data["email"].as_str().map(|s| s.to_string()),
        display_name: data["name"].as_str().map(|s| s.to_string()),
        avatar_url: data["picture"].as_str().map(|s| s.to_string()),
        access_token: access_token.to_string(),
        refresh_token: None,
        expires_at: None,
    })
}

/// 获取GitHub用户信息
async fn fetch_github_user_info(access_token: &str) -> Result<OAuthUserProfile, AppError> {
    let client = reqwest::Client::new();

    // 获取用户基本信息
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {}", access_token))
        .header("User-Agent", "StoryForge/4.5.0")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch GitHub user info: {}", e))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!("GitHub API error: {}", text)));
    }

    let data: serde_json::Value = response.json().await.map_err(AppError::from)?;

    let user_id = data["id"]
        .as_i64()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let name = data["name"]
        .as_str()
        .or_else(|| data["login"].as_str())
        .map(|s| s.to_string());
    let avatar = data["avatar_url"].as_str().map(|s| s.to_string());

    // 获取用户邮箱（GitHub可能不返回public email）
    let email = if let Some(email) = data["email"].as_str() {
        Some(email.to_string())
    } else {
        // 尝试从邮箱API获取
        fetch_github_email(access_token).await.ok()
    };

    Ok(OAuthUserProfile {
        provider: "github".to_string(),
        provider_account_id: user_id,
        email,
        display_name: name,
        avatar_url: avatar,
        access_token: access_token.to_string(),
        refresh_token: None,
        expires_at: None,
    })
}

async fn fetch_github_email(access_token: &str) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("token {}", access_token))
        .header("User-Agent", "StoryForge/4.5.0")
        .send()
        .await
        .map_err(AppError::from)?;

    let emails: Vec<serde_json::Value> = response.json().await.map_err(AppError::from)?;

    // 找到primary邮箱
    for email_entry in &emails {
        if email_entry
            .get("primary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Some(email) = email_entry["email"].as_str() {
                return Ok(email.to_string());
            }
        }
    }

    // fallback到第一个邮箱
    if let Some(first) = emails.first() {
        if let Some(email) = first["email"].as_str() {
            return Ok(email.to_string());
        }
    }

    Err(AppError::internal("No email found"))
}

/// 查找可用端口
fn find_available_port() -> Result<u16, AppError> {
    for port in 8765..=9000 {
        if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return Ok(port);
        }
    }
    Err(AppError::internal("No available port found"))
}
