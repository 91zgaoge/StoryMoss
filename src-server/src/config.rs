//! Server Configuration

use once_cell::sync::Lazy;
use std::env;

pub static CONFIG: Lazy<ServerConfig> = Lazy::new(|| ServerConfig::from_env());

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub frontend_url: String,

    // OAuth
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub wechat_client_id: Option<String>,
    pub wechat_client_secret: Option<String>,
    pub qq_client_id: Option<String>,
    pub qq_client_secret: Option<String>,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "storymoss-default-secret-change-me".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            frontend_url: env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string()),

            google_client_id: env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").ok(),
            github_client_id: env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").ok(),
            wechat_client_id: env::var("WECHAT_CLIENT_ID").ok(),
            wechat_client_secret: env::var("WECHAT_CLIENT_SECRET").ok(),
            qq_client_id: env::var("QQ_CLIENT_ID").ok(),
            qq_client_secret: env::var("QQ_CLIENT_SECRET").ok(),
        }
    }

    pub fn is_oauth_enabled(&self, provider: &str) -> bool {
        match provider {
            "google" => self.google_client_id.is_some() && self.google_client_secret.is_some(),
            "github" => self.github_client_id.is_some() && self.github_client_secret.is_some(),
            "wechat" => self.wechat_client_id.is_some() && self.wechat_client_secret.is_some(),
            "qq" => self.qq_client_id.is_some() && self.qq_client_secret.is_some(),
            _ => false,
        }
    }
}
