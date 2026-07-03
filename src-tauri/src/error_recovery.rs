//! 四级错误恢复基础设施
//!
//! v0.25.0: 根据 AppError.severity() 对错误进行分类恢复：
//! - Retry    : 自动重试 + 指数退避（LLM 连接/生成、DB 锁定、网络离线）
//! - Degraded : 降级到简化路径（上下文不可用、部分资源缺失）
//! - UserAction: 抛出给用户处理（订阅、校验、预检）
//! - Fatal    : 记录后终止，返回原始错误

use std::future::Future;

use crate::error::{AppError, ErrorSeverity};

/// 恢复动作结果
#[derive(Debug, Clone)]
pub enum RecoveryOutcome<T> {
    /// 首次成功
    Success(T),
    /// 重试后成功，携带重试次数
    RetriedSuccess(T, u32),
    /// 降级后成功，携带降级说明
    DegradedSuccess(T, String),
    /// 恢复失败，携带最终错误
    Failed(AppError),
}

impl<T> RecoveryOutcome<T> {
    /// 转换为 Result，RetriedSuccess / DegradedSuccess 都算成功
    pub fn into_result(self) -> Result<T, AppError> {
        match self {
            RecoveryOutcome::Success(v) => Ok(v),
            RecoveryOutcome::RetriedSuccess(v, _) => Ok(v),
            RecoveryOutcome::DegradedSuccess(v, _) => Ok(v),
            RecoveryOutcome::Failed(e) => Err(e),
        }
    }
}

/// 对可重试错误执行指数退避重试。
///
/// 仅当错误 severity 为 Retry 时才重试；其它 severity 立即返回失败。
/// 每次退避时间 = min(base_delay_ms * 2^(attempt-1), max_delay_ms)。
pub async fn retry_with_backoff<F, Fut, T>(
    operation: F,
    max_retries: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
    context: &str,
) -> RecoveryOutcome<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    let mut last_error = None;
    for attempt in 0..=max_retries {
        match operation().await {
            Ok(value) => {
                if attempt == 0 {
                    return RecoveryOutcome::Success(value);
                }
                return RecoveryOutcome::RetriedSuccess(value, attempt);
            }
            Err(e) => {
                let severity = e.severity();
                if severity != ErrorSeverity::Retry || attempt == max_retries {
                    last_error = Some(e);
                    break;
                }
                let delay = base_delay_ms.saturating_mul(1 << attempt).min(max_delay_ms);
                log::warn!(
                    "[error_recovery] {} 第 {} 次尝试失败（severity={:?}），{}ms 后重试: {}",
                    context,
                    attempt + 1,
                    severity,
                    delay,
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                last_error = Some(e);
            }
        }
    }

    RecoveryOutcome::Failed(
        last_error.unwrap_or_else(|| AppError::internal(format!("{} 重试后仍失败", context))),
    )
}

/// 对可能降级的错误执行降级回退。
///
/// 主操作失败时：
/// - 若错误 severity 为 Degraded，执行 fallback 降级路径；
/// - 否则直接返回失败。
pub async fn with_degraded_fallback<F, Fut, G, Gfut, T>(
    primary: F,
    fallback: G,
    context: &str,
) -> RecoveryOutcome<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
    G: Fn() -> Gfut,
    Gfut: Future<Output = Result<T, AppError>>,
{
    match primary().await {
        Ok(value) => RecoveryOutcome::Success(value),
        Err(e) => {
            if e.severity() == ErrorSeverity::Degraded {
                log::warn!(
                    "[error_recovery] {} 主路径失败且 severity=Degraded，尝试降级路径: {}",
                    context,
                    e
                );
                match fallback().await {
                    Ok(value) => RecoveryOutcome::DegradedSuccess(value, e.message()),
                    Err(e2) => RecoveryOutcome::Failed(e2),
                }
            } else {
                RecoveryOutcome::Failed(e)
            }
        }
    }
}

/// 快速判断错误是否需要用户显式介入。
pub fn is_user_action(err: &AppError) -> bool {
    err.severity() == ErrorSeverity::UserAction
}

/// 快速判断错误是否为致命错误。
pub fn is_fatal(err: &AppError) -> bool {
    err.severity() == ErrorSeverity::Fatal
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let outcome = retry_with_backoff(|| async { Ok(42) }, 3, 10, 100, "test").await;
        assert!(matches!(outcome, RecoveryOutcome::Success(42)));
    }

    #[tokio::test]
    async fn test_retry_eventually_success() {
        let counter = Arc::new(Mutex::new(0u32));
        let outcome = retry_with_backoff(
            {
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    async move {
                        let mut c = counter.lock().unwrap();
                        *c += 1;
                        let value = *c;
                        drop(c);
                        if value < 3 {
                            Err(AppError::llm_connection_timeout(10))
                        } else {
                            Ok(42)
                        }
                    }
                }
            },
            3,
            5,
            50,
            "test",
        )
        .await;
        assert!(matches!(outcome, RecoveryOutcome::RetriedSuccess(42, 2)));
    }

    #[tokio::test]
    async fn test_retry_non_retryable_immediately_fails() {
        let outcome: RecoveryOutcome<i32> = retry_with_backoff(
            || async { Err(AppError::internal("fatal".to_string())) },
            3,
            5,
            50,
            "test",
        )
        .await;
        assert!(matches!(outcome, RecoveryOutcome::Failed(_)));
    }

    #[tokio::test]
    async fn test_degraded_fallback_used() {
        let outcome = with_degraded_fallback(
            || async { Err(AppError::context_unavailable("characters", "no chars")) },
            || async { Ok(42) },
            "test",
        )
        .await;
        assert!(matches!(outcome, RecoveryOutcome::DegradedSuccess(42, _)));
    }

    #[tokio::test]
    async fn test_degraded_fallback_skipped_for_non_degraded() {
        let outcome = with_degraded_fallback(
            || async { Err(AppError::internal("fatal".to_string())) },
            || async { Ok(42) },
            "test",
        )
        .await;
        assert!(matches!(outcome, RecoveryOutcome::Failed(_)));
    }
}
