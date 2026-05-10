//! Subscription Tauri Commands

use super::{SubscriptionService, SubscriptionStatus, QuotaCheckResult, QuotaDetail};
use tauri::{command, AppHandle, Manager};
use crate::db::DbPool;

/// 获取当前用户订阅状态
#[command]
pub fn get_subscription_status(app_handle: AppHandle) -> Result<SubscriptionStatus, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    // 当前使用 device/machine id 作为 user_id，后续可接入真实用户系统
    let user_id = get_user_id(&app_handle);
    service.get_or_create_subscription(&user_id)
}

/// 检查 AI 使用配额（向后兼容，所有功能已免费）
#[command]
pub fn check_ai_quota(app_handle: AppHandle) -> Result<QuotaCheckResult, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    service.check_ai_quota(&user_id)
}

/// 获取 V2 配额详情（按功能区分）
#[command]
pub fn get_quota_detail(app_handle: AppHandle) -> Result<QuotaDetail, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    service.get_quota_detail(&user_id)
}

/// 检查自动续写配额
#[command(rename_all = "snake_case")]
pub fn check_auto_write_quota(app_handle: AppHandle, requested_chars: i32) -> Result<QuotaCheckResult, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    service.check_auto_write_quota(&user_id, requested_chars)
}

/// 检查自动修改配额
#[command(rename_all = "snake_case")]
pub fn check_auto_revise_quota(app_handle: AppHandle, requested_chars: i32) -> Result<QuotaCheckResult, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    service.check_auto_revise_quota(&user_id, requested_chars)
}

/// 获取当前用户 ID（基于设备标识，后续可接入真实用户认证）
fn get_user_id(app_handle: &AppHandle) -> String {
    // 优先从 app_data_dir 生成稳定的设备标识
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_default();
    
    let machine_id_path = app_dir.join(".machine_id");
    if machine_id_path.exists() {
        std::fs::read_to_string(&machine_id_path).unwrap_or_default().trim().to_string()
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = std::fs::create_dir_all(&app_dir);
        let _ = std::fs::write(&machine_id_path, &id);
        id
    }
}

/// 模拟升级订阅（开发测试用）
#[command]
pub fn dev_upgrade_subscription(tier: String, app_handle: AppHandle) -> Result<SubscriptionStatus, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    
    let expires_days = if tier == "pro" { Some(30) } else { None };
    service.upgrade_subscription(&user_id, &tier, expires_days)
}

/// 模拟降级订阅（开发测试用）
#[command]
pub fn dev_downgrade_subscription(app_handle: AppHandle) -> Result<SubscriptionStatus, String> {
    let pool = app_handle.state::<DbPool>();
    let service = SubscriptionService::new(pool.inner().clone());
    let user_id = get_user_id(&app_handle);
    service.upgrade_subscription(&user_id, "free", None)
}
