//! JWT Session 管理

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const JWT_SECRET_ENV: &str = "STORYFORGE_JWT_SECRET";
const DEFAULT_SECRET: &str = "storyforge-default-jwt-secret-change-in-production";
const TOKEN_EXPIRY_SECONDS: i64 = 7 * 24 * 3600; // 7天

/// JWT Claims
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,    // user_id
    exp: usize,     // expiration time
    iat: usize,     // issued at
    jti: String,    // token id
}

/// 生成JWT token
pub fn create_token(user_id: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    let exp = now + TOKEN_EXPIRY_SECONDS as usize;
    let jti = uuid::Uuid::new_v4().to_string();

    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        iat: now,
        jti,
    };

    let secret = get_jwt_secret();
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Failed to create JWT: {}", e))?;

    Ok(token)
}

/// 验证JWT token，返回user_id
#[allow(dead_code)]
pub fn validate_token(token: &str) -> Result<String, String> {
    let secret = get_jwt_secret();
    let validation = Validation::default();

    let decoded = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("Invalid token: {}", e))?;

    Ok(decoded.claims.sub)
}

/// 获取JWT密钥（优先从环境变量，fallback到默认值）
fn get_jwt_secret() -> String {
    std::env::var(JWT_SECRET_ENV).unwrap_or_else(|_| DEFAULT_SECRET.to_string())
}
