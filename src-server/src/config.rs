//! Server Configuration

use std::env;

use once_cell::sync::Lazy;

pub static CONFIG: Lazy<ServerConfig> = Lazy::new(|| ServerConfig::from_env());

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub frontend_url: String,
    /// 对外 base URL（OAuth redirect_uri 用），需与 provider 后台注册的
    /// callback 前缀一致
    pub server_base_url: Option<String>,

    // Dev
    pub dev_upgrade_enabled: bool,

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

/// 公开的示例/缺省 JWT 密钥：任何人都能用它们伪造 token，运行时出现即拒绝启动。
const INSECURE_JWT_SECRETS: &[&str] = &[
    "storymoss-default-secret-change-me",
    "change-me-in-production",
    "change-me",
    "secret",
    "your-super-secret-jwt-key-min-32-chars",
];

/// JWT 密钥最小长度（与 .env.example / SERVER_DEPLOYMENT.md 的 32 字符要求一致）。
const MIN_JWT_SECRET_LEN: usize = 32;

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            // v0.59.0：不再提供缺省密钥。缺省值等于公开常量，线上漏配 env
            // 时任意人可自签 token 冒充任意用户（含 admin）。
            jwt_secret: parse_jwt_secret(),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            server_base_url: env::var("SERVER_BASE_URL")
                .ok()
                .map(|u| u.trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty()),

            // v0.59.0：缺省改为 false——自助升级权益只应在显式开启的
            // 开发环境生效，裸跑二进制不应是升级后门。
            dev_upgrade_enabled: env::var("DEV_UPGRADE_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),

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

/// 纯函数校验：便于单测覆盖「缺省值/过短」两类拒绝路径。
fn validate_jwt_secret(raw: &str) -> Result<String, String> {
    let secret = raw.trim();
    if secret.is_empty() {
        return Err("JWT_SECRET 为空；生成：openssl rand -hex 32".to_string());
    }
    let lowered = secret.to_ascii_lowercase();
    if INSECURE_JWT_SECRETS.contains(&lowered.as_str()) {
        return Err(
            "JWT_SECRET 使用了公开的示例/缺省值，任何人都能伪造 token；请改为随机强密钥（openssl rand -hex 32）"
                .to_string(),
        );
    }
    if secret.chars().count() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "JWT_SECRET 过短（当前 {} 字符，至少需要 {}）；生成：openssl rand -hex 32",
            secret.chars().count(),
            MIN_JWT_SECRET_LEN
        ));
    }
    Ok(secret.to_string())
}

fn parse_jwt_secret() -> String {
    let raw = env::var("JWT_SECRET")
        .unwrap_or_else(|_| panic!("JWT_SECRET must be set；生成：openssl rand -hex 32"));
    validate_jwt_secret(&raw).unwrap_or_else(|e| panic!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_known_insecure_defaults() {
        for value in [
            "storymoss-default-secret-change-me",
            "change-me-in-production",
            "your-super-secret-jwt-key-min-32-chars",
            "CHANGE-ME-IN-PRODUCTION",
        ] {
            assert!(
                validate_jwt_secret(value).is_err(),
                "公开示例值必须被拒绝：{value}"
            );
        }
    }

    #[test]
    fn rejects_short_or_empty_secret() {
        assert!(validate_jwt_secret("").is_err());
        assert!(validate_jwt_secret("   ").is_err());
        assert!(validate_jwt_secret("short-secret").is_err());
    }

    #[test]
    fn accepts_strong_secret_and_trims_whitespace() {
        let strong = "9f2c1d4b7a6e8f0c3b5d2a1e4f7c8b9d";
        assert_eq!(validate_jwt_secret(strong).unwrap(), strong);
        assert_eq!(
            validate_jwt_secret(&format!("  {strong}  ")).unwrap(),
            strong
        );
    }
}
