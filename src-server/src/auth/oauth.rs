//! OAuth2 Flow — Server-side
//!
//! 参考 oauth2-rs crate 实现 Authorization Code + PKCE 流程

use super::{OAuthProvider, OAuthUserInfo};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenUrl,
};
use serde_json::Value;

use crate::config::CONFIG;

/// 构建OAuth客户端
pub fn build_oauth_client(
    provider: OAuthProvider,
) -> Result<(BasicClient, String, String), String> {
    let (client_id, client_secret, redirect_url) = get_provider_config(provider)?;

    let (auth_url, token_url) = get_provider_urls(provider);

    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| e.to_string())?);

    // 生成 PKCE
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let state = CsrfToken::new_random().secret().clone();

    let auth_url = client
        .authorize_url(|| CsrfToken::new(state.clone()))
        .set_pkce_challenge(pkce_challenge)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url()
        .0;

    Ok((client, auth_url.to_string(), pkce_verifier.secret().clone()))
}

/// 用code交换token并获取用户信息
pub async fn exchange_code_and_get_user(
    provider: OAuthProvider,
    code: &str,
    pkce_verifier: &str,
) -> Result<OAuthUserInfo, String> {
    let (client_id, client_secret, redirect_url) = get_provider_config(provider)?;

    let (_, token_url) = get_provider_urls(provider);

    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new("http://dummy".to_string()).unwrap(), // not used for token exchange
        Some(token_url),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| e.to_string())?);

    let verifier = PkceCodeVerifier::new(pkce_verifier.to_string());

    let token_result = client
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .set_pkce_verifier(verifier)
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let access_token = token_result.access_token().secret().clone();
    let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());
    let expires_at = token_result.expires_in().map(|d| {
        chrono::Utc::now() + chrono::Duration::from_std(d).unwrap_or(chrono::Duration::seconds(3600))
    });

    // 获取用户资料
    let profile = match provider {
        OAuthProvider::Google => fetch_google_user_info(&access_token).await?,
        OAuthProvider::Github => fetch_github_user_info(&access_token).await?,
        _ => return Err(format!("Provider {:?} not yet implemented", provider)),
    };

    Ok(OAuthUserInfo {
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

fn get_provider_config(provider: OAuthProvider) -> Result<(String, String, String), String> {
    match provider {
        OAuthProvider::Google => {
            let id = CONFIG.google_client_id.clone().ok_or("Google client ID not configured")?;
            let secret = CONFIG.google_client_secret.clone().ok_or("Google client secret not configured")?;
            let redirect = format!("{}/api/auth/google/callback", get_server_base_url());
            Ok((id, secret, redirect))
        }
        OAuthProvider::Github => {
            let id = CONFIG.github_client_id.clone().ok_or("GitHub client ID not configured")?;
            let secret = CONFIG.github_client_secret.clone().ok_or("GitHub client secret not configured")?;
            let redirect = format!("{}/api/auth/github/callback", get_server_base_url());
            Ok((id, secret, redirect))
        }
        _ => Err(format!("Provider {:?} not configured", provider)),
    }
}

fn get_provider_urls(provider: OAuthProvider) -> (AuthUrl, TokenUrl) {
    match provider {
        OAuthProvider::Google => (
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap(),
        ),
        OAuthProvider::Github => (
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
            TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap(),
        ),
        OAuthProvider::Wechat => (
            AuthUrl::new("https://open.weixin.qq.com/connect/qrconnect".to_string()).unwrap(),
            TokenUrl::new("https://api.weixin.qq.com/sns/oauth2/access_token".to_string()).unwrap(),
        ),
        OAuthProvider::Qq => (
            AuthUrl::new("https://graph.qq.com/oauth2.0/authorize".to_string()).unwrap(),
            TokenUrl::new("https://graph.qq.com/oauth2.0/token".to_string()).unwrap(),
        ),
    }
}

fn get_server_base_url() -> String {
    format!("http://{}:{}", CONFIG.server_host, CONFIG.server_port)
}

async fn fetch_google_user_info(access_token: &str) -> Result<OAuthUserInfo, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Google user: {}", e))?;

    let data: Value = response.json().await.map_err(|e| e.to_string())?;

    Ok(OAuthUserInfo {
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

async fn fetch_github_user_info(access_token: &str) -> Result<OAuthUserInfo, String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {}", access_token))
        .header("User-Agent", "StoryMoss-Server/4.5.0")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch GitHub user: {}", e))?;

    let data: Value = response.json().await.map_err(|e| e.to_string())?;

    let user_id = data["id"].as_i64().map(|id| id.to_string()).unwrap_or_default();
    let name = data["name"].as_str().or_else(|| data["login"].as_str()).map(|s| s.to_string());
    let avatar = data["avatar_url"].as_str().map(|s| s.to_string());

    // Get email
    let email = if let Some(email) = data["email"].as_str() {
        Some(email.to_string())
    } else {
        fetch_github_email(access_token).await.ok()
    };

    Ok(OAuthUserInfo {
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

async fn fetch_github_email(access_token: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("token {}", access_token))
        .header("User-Agent", "StoryMoss-Server/4.5.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let emails: Vec<Value> = response.json().await.map_err(|e| e.to_string())?;

    for entry in emails {
        if entry.get("primary").and_then(|v| v.as_bool()).unwrap_or(false) {
            if let Some(email) = entry["email"].as_str() {
                return Ok(email.to_string());
            }
        }
    }

    if let Some(first) = emails.first() {
        if let Some(email) = first["email"].as_str() {
            return Ok(email.to_string());
        }
    }

    Err("No email found".to_string())
}
