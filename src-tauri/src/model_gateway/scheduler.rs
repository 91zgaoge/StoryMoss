//! Model Gateway — 健康探测调度器
//!
//! v0.14.0: 负责在应用启动时执行全量探测，并按计划对 healthy/degraded
//! 模型进行轻量 ping。
//! v0.23.60: 重构为三轮探测架构——
//!   1. 启动全量探测（30s 超时，顺序）
//!   2. 快速保活（每 10s，5s 超时）→ 保持 HealthRegistry 新鲜
//!   3. 重试退避（指数退避 60→120→240→…→3600s）→ 不浪费资源在死模型上

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use tauri::AppHandle;
use tokio::time::interval;

use super::executor::GatewayExecutor;

/// 快速保活探测间隔
const KEEPALIVE_INTERVAL_SECS: u64 = 10;
/// 快速保活超时
const KEEPALIVE_TIMEOUT_SECS: u64 = 5;
/// 网关信任缓存的保鲜期（<此值跳过内联探测）
pub const HEALTH_FRESHNESS_THRESHOLD_SECS: i64 = 15;
/// 重试退避初始间隔（秒）
const RETRY_BASE_INTERVAL_SECS: u64 = 30;
/// 重试退避最大间隔（秒）
const RETRY_MAX_INTERVAL_SECS: u64 = 3600;

/// 启动后台健康探测任务
pub fn spawn_health_probe_scheduler(app_handle: AppHandle, executor: GatewayExecutor) {
    tauri::async_runtime::spawn(async move {
        // 1. 启动时全量探测（顺序，每模型最多 30s）
        run_full_probe(&executor).await;

        // v0.15.0: 启动 30s 后运行流式基准
        let bench_executor = executor.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            bench_executor.run_initial_benchmark().await;
        });

        // v0.23.60: 快速保活——每 10s 探测所有健康/降级模型（5s 超时），
        // 保持 HealthRegistry 新鲜，让网关路径跳过内联探测。
        let mut keepalive_interval = interval(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));

        // v0.23.60: 退避重试——死模型不再每 60s 浪费一次探测，
        // 而是指数退避直到 1h 上限。
        let mut retry_interval = interval(Duration::from_secs(RETRY_BASE_INTERVAL_SECS));
        let mut retry_backoffs: HashMap<String, u64> = HashMap::new(); // model_id → 退避秒数

        loop {
            tokio::select! {
                _ = keepalive_interval.tick() => {
                    run_keepalive_probe(&executor).await;
                }
                _ = retry_interval.tick() => {
                    run_retry_probe_with_backoff(&executor, &mut retry_backoffs).await;
                }
            }
        }
    });
}

async fn run_full_probe(executor: &GatewayExecutor) {
    let models: Vec<String> = executor
        .registry
        .lock()
        .map(|g| {
            g.enabled_generative_models()
                .into_iter()
                .map(|m| m.id.clone())
                .collect()
        })
        .unwrap_or_default();

    log::info!(
        "[GatewayScheduler] 启动时全量探测 {} 个模型（每模型最多 30s）",
        models.len()
    );
    for model_id in models {
        if let Err(e) = executor.probe_model(&model_id).await {
            log::warn!("[GatewayScheduler] 启动探测 {} 失败: {}", model_id, e);
        }
    }
}

/// v0.23.60: 快速保活探测——仅探测健康/降级模型，5s 超时。
///
/// 保持 HealthRegistry 的 `last_checked_at` 在 10s 以内，
/// 网关 generate() 读缓存时 <15s 即可跳过内联探测。
async fn run_keepalive_probe(executor: &GatewayExecutor) {
    let health = match executor.health_registry().lock() {
        Ok(g) => g.all(),
        Err(_) => return,
    };

    let mut probed = 0u32;
    for snapshot in health {
        if matches!(
            snapshot.status,
            super::types::HealthStatus::Healthy | super::types::HealthStatus::Degraded
        ) {
            // 5s 超时的轻量探测
            let start = Instant::now();
            match tokio::time::timeout(
                Duration::from_secs(KEEPALIVE_TIMEOUT_SECS),
                executor.probe_model(&snapshot.model_id),
            )
            .await
            {
                Ok(Ok(_)) => {
                    probed += 1;
                    log::debug!(
                        "[GatewayScheduler] keepalive {} OK ({:.0}ms)",
                        snapshot.model_id,
                        start.elapsed().as_secs_f64() * 1000.0
                    );
                }
                Ok(Err(e)) => {
                    log::warn!(
                        "[GatewayScheduler] keepalive {} 失败: {}",
                        snapshot.model_id,
                        e
                    );
                }
                Err(_) => {
                    log::warn!(
                        "[GatewayScheduler] keepalive {} 超时 ({}s)",
                        snapshot.model_id,
                        KEEPALIVE_TIMEOUT_SECS
                    );
                }
            }
        }
    }
    if probed > 0 {
        log::debug!(
            "[GatewayScheduler] keepalive 轮完成: {} 个模型探测通过",
            probed
        );
    }
}

/// v0.23.60: 指数退避重试。
///
/// 死模型（连续失败 >=3 次）按退避间隔重试，不再每 30s 浪费一次探测。
/// 退避序列: 30s → 60s → 120s → 240s → 480s → 960s → … → 3600s
async fn run_retry_probe_with_backoff(
    executor: &GatewayExecutor,
    backoffs: &mut HashMap<String, u64>,
) {
    let health = match executor.health_registry().lock() {
        Ok(g) => g.all(),
        Err(_) => return,
    };

    let now = Instant::now();
    for snapshot in health {
        let failures = {
            executor
                .health_registry()
                .lock()
                .ok()
                .map(|g| g.consecutive_failures(&snapshot.model_id))
                .unwrap_or(0)
        };

        // 只有连续失败 >=3 的模型才退避
        if failures >= 3 {
            let current_backoff = backoffs
                .get(&snapshot.model_id)
                .copied()
                .unwrap_or(RETRY_BASE_INTERVAL_SECS);

            // 简单退避：上次探测距今 < backoff 秒则跳过
            let last_checked_secs_ago = snapshot
                .last_checked_at
                .as_ref()
                .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
                .map(|dt| {
                    (chrono::Utc::now() - dt.with_timezone(&chrono::Utc))
                        .num_seconds()
                        .max(0) as u64
                })
                .unwrap_or(u64::MAX);

            if last_checked_secs_ago < current_backoff {
                continue;
            }

            // 到了重试时间，执行探测
            log::info!(
                "[GatewayScheduler] 退避重试 {} (连续失败 {} 次, 退避 {}s)",
                snapshot.model_id,
                failures,
                current_backoff
            );
            if let Err(e) = executor.probe_model(&snapshot.model_id).await {
                log::warn!(
                    "[GatewayScheduler] 退避重试 {} 仍失败: {}",
                    snapshot.model_id,
                    e
                );
                // 退避翻倍
                let next = (current_backoff * 2).min(RETRY_MAX_INTERVAL_SECS);
                backoffs.insert(snapshot.model_id.clone(), next);
            } else {
                // 探测成功，重置退避
                backoffs.remove(&snapshot.model_id);
                log::info!(
                    "[GatewayScheduler] 退避重试 {} 成功，重置退避",
                    snapshot.model_id
                );
            }
        } else {
            // 非死模型：正常重试（Degraded / Unknown）
            if matches!(
                snapshot.status,
                super::types::HealthStatus::Degraded
                    | super::types::HealthStatus::Unhealthy
                    | super::types::HealthStatus::Unknown
            ) {
                let _ = executor.probe_model(&snapshot.model_id).await;
                // 重置退避
                backoffs.remove(&snapshot.model_id);
            }
        }
    }
}
