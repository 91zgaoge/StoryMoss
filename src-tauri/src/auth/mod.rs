//! Authentication Module — 多账号OAuth登录系统
//!
//! 支持 Google / GitHub OAuth2 登录
//! 微信/QQ 预留框架（二期补充）

use serde::{Deserialize, Serialize};

pub mod commands;
pub mod oauth;
pub mod session;

/// OAuth提供商
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    Github,
    Wechat,
    Qq,
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthProvider::Google => write!(f, "google"),
            OAuthProvider::Github => write!(f, "github"),
            OAuthProvider::Wechat => write!(f, "wechat"),
            OAuthProvider::Qq => write!(f, "qq"),
        }
    }
}

impl std::str::FromStr for OAuthProvider {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "google" => Ok(OAuthProvider::Google),
            "github" => Ok(OAuthProvider::Github),
            "wechat" => Ok(OAuthProvider::Wechat),
            "qq" => Ok(OAuthProvider::Qq),
            _ => Err(format!("Unknown OAuth provider: {}", s)),
        }
    }
}

/// OAuth客户端配置（存储在AppConfig中）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientConfig {
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

/// 认证配置（前端可见部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_enabled: bool,
    pub github_enabled: bool,
    pub wechat_enabled: bool,
    pub qq_enabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            google_enabled: false,
            github_enabled: false,
            wechat_enabled: false,
            qq_enabled: false,
        }
    }
}
