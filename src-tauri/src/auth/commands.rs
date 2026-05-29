//! Tauri IPC Commands — 认证相关

use tauri::{AppHandle, Manager, State};

use super::{oauth, session, AuthConfig, OAuthProvider};
use crate::{
    db::{DbPool, UserRepository},
    error::AppError,
};

/// 获取当前认证配置（前端用，不含密钥）
#[tauri::command]
pub fn get_auth_config(app_handle: AppHandle) -> Result<AuthConfig, AppError> {
    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;

    let config = crate::config::AppConfig::load(&app_dir).map_err(AppError::from)?;

    // 检查各provider是否配置了client_id
    let google_enabled = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get("google"))
        .map(|c| !c.client_id.is_empty())
        .unwrap_or(false);

    let github_enabled = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get("github"))
        .map(|c| !c.client_id.is_empty())
        .unwrap_or(false);

    let wechat_enabled = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get("wechat"))
        .map(|c| !c.client_id.is_empty())
        .unwrap_or(false);

    let qq_enabled = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get("qq"))
        .map(|c| !c.client_id.is_empty())
        .unwrap_or(false);

    Ok(AuthConfig {
        google_enabled,
        github_enabled,
        wechat_enabled,
        qq_enabled,
    })
}

/// 开始OAuth登录流程
#[tauri::command]
pub fn oauth_start(provider: String, app_handle: AppHandle) -> Result<serde_json::Value, AppError> {
    let provider = provider.parse::<OAuthProvider>().map_err(AppError::from)?;

    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;

    let config = crate::config::AppConfig::load(&app_dir).map_err(AppError::from)?;

    let client_config = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get(&provider.to_string()))
        .ok_or(format!("OAuth client not configured for {}", provider))?;

    let (auth_url, state, port) = oauth::start_oauth_flow(
        provider,
        &client_config.client_id,
        client_config.client_secret.as_deref(),
    )?;

    Ok(serde_json::json!({
        "auth_url": auth_url,
        "state": state,
        "redirect_port": port,
    }))
}

/// OAuth回调处理（桌面端通过本地HTTP服务器接收回调后调用）
#[tauri::command]
pub async fn oauth_callback(
    provider: String,
    code: String,
    state: String,
    app_handle: AppHandle,
) -> Result<crate::db::UserInfo, AppError> {
    let provider = provider.parse::<OAuthProvider>().map_err(AppError::from)?;

    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;

    let config = crate::config::AppConfig::load(&app_dir).map_err(AppError::from)?;

    let client_config = config
        .auth_clients
        .as_ref()
        .and_then(|c| c.get(&provider.to_string()))
        .ok_or(format!("OAuth client not configured for {}", provider))?;

    // 用code交换token并获取用户资料
    let profile = oauth::handle_oauth_callback(
        &state,
        &code,
        &client_config.client_id,
        client_config.client_secret.as_deref(),
    )
    .await?;

    // 获取数据库连接池
    let pool = app_handle.state::<DbPool>().inner().clone();

    let user_repo = UserRepository::new(pool);

    // 查找或创建用户
    let user = match user_repo
        .find_by_oauth(&profile.provider, &profile.provider_account_id)
        .map_err(AppError::from)?
    {
        Some(existing_user) => existing_user,
        None => {
            // 创建新用户
            let new_user = user_repo
                .create_user(
                    profile.email.clone(),
                    profile.display_name.clone(),
                    profile.avatar_url.clone(),
                )
                .map_err(AppError::from)?;

            // 创建OAuth账号关联
            user_repo
                .create_oauth_account(
                    &new_user.id,
                    &profile.provider,
                    &profile.provider_account_id,
                    Some(profile.access_token),
                    profile.refresh_token,
                    profile.expires_at,
                )
                .map_err(AppError::from)?;

            new_user
        }
    };

    // 创建session
    let token = session::create_token(&user.id)?;
    let expires_at = chrono::Local::now() + chrono::Duration::days(7);
    user_repo
        .create_session(&user.id, &token, expires_at)
        .map_err(AppError::from)?;

    Ok(user_repo.to_user_info(&user))
}

/// 获取当前登录用户
#[tauri::command]
pub fn get_current_user(_pool: State<'_, DbPool>) -> Result<Option<crate::db::UserInfo>, AppError> {
    // 桌面端简化实现：从内存/session存储中获取
    // 实际应用中可以通过前端传递token来验证
    // 这里返回None表示未实现完整的session持久化检查
    Ok(None)
}

/// 注销登录
#[tauri::command]
pub fn logout(token: String, pool: State<'_, DbPool>) -> Result<(), AppError> {
    let user_repo = UserRepository::new(pool.inner().clone());
    user_repo.delete_session(&token).map_err(AppError::from)?;
    Ok(())
}
