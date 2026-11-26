//! Model Gateway — 健康探测调度器
//!
//! v0.14.0: 负责在应用启动时执行全量探测，并按计划对 healthy/degraded
//! 模型进行轻量 ping。
//! v0.23.60: 重构为三轮探测架构——
//!   1. 启动全量探测（30s 超时，顺序）
//!   2. 快速保活（每 10s，30s 超时）→ 保持 HealthRegistry 新鲜
//!   3. 重试退避（指数退避 60→120→240→…→3600s）→ 不浪费资源在死模型上

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use tauri::AppHandle;
use tokio::time::interval;

use super::{executor::GatewayExecutor, types::ProbeResult};
use crate::error::AppError;

/// v0.30.50（issue #14）: 后台探测模式共享开关。
/// true = `always`（每 10s 保活 + 退避重试，默认）；
/// false = `on_demand`（闲置完全静默，仅在生成时由网关内联探测）。
/// 由 lib.rs 启动时按 AppConfig.health_probe_mode 初始化并 manage，
/// save_settings 保存后热更新，调度器每 tick 读取，无需重启生效。
pub type HealthProbeModeFlag = Arc<AtomicBool>;

/// 快速保活探测间隔
const KEEPALIVE_INTERVAL_SECS: u64 = 10;
/// 快速保活超时
///
/// v0.99.11: 5s → 30s。大本地模型（180B MoE）单次首字可 20–60s，串行闸门
/// 后面排队的探测 5s 必然超时，把「busy」误判成「down」。放宽后仍以 10s
/// 间隔轮询，超时不再制造 85 条/天的假故障。
const KEEPALIVE_TIMEOUT_SECS: u64 = 30;
/// 活跃模型的保活间隔：最近 2 分钟内有真实生成调用。
const KEEPALIVE_ACTIVE_INTERVAL_SECS: i64 = 10;
/// 闲置模型的保活间隔。
///
/// v0.99.26：真机 v0.99.25 一天 **15,584 次探测 vs 45 次真实生成**（其中
/// 10,413 次打在付费远端 deepseek-v4），tracing 日志 86% 是探测行，每小时
/// ~1,000 次探测即便凌晨无人使用也照跑。闲置模型不需要 10s 新鲜度：真正
/// 要用它时网关会按 `is_health_fresh`（15s）内联探测一次，成本远低于常驻
/// 轮询。
const KEEPALIVE_IDLE_INTERVAL_SECS: i64 = 300;
/// 「活跃模型」判定窗口：最近一次真实生成调用在此窗口内。
const KEEPALIVE_ACTIVE_WINDOW_SECS: i64 = 120;
/// 连续失败达到该值视为死模型：保活不再探测，交给退避重试。
const DEAD_MODEL_FAILURES: u32 = 3;
/// 网关信任缓存的保鲜期（<此值跳过内联探测）
pub const HEALTH_FRESHNESS_THRESHOLD_SECS: i64 = 15;
/// 重试退避初始间隔（秒）
const RETRY_BASE_INTERVAL_SECS: u64 = 30;
/// 重试退避最大间隔（秒）
const RETRY_MAX_INTERVAL_SECS: u64 = 3600;

/// 启动后台健康探测任务
pub fn spawn_health_probe_scheduler(
    app_handle: AppHandle,
    executor: GatewayExecutor,
    probe_mode: HealthProbeModeFlag,
) {
    tauri::async_runtime::spawn(async move {
        // 1. 启动时全量探测（顺序，每模型最多 30s）。
        // 两种模式下都保留：一次性探测为健康报告与首次生成提供基线，
        // on_demand 模式跳过的只是后续的周期性探测。
        run_full_probe(&executor).await;

        // v0.15.0: 启动 30s 后运行流式基准
        let bench_executor = executor.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            bench_executor.run_initial_benchmark().await;
        });

        // v0.23.60: 快速保活——每 10s 探测所有健康/降级模型（30s 超时），
        // 保持 HealthRegistry 新鲜，让网关路径跳过内联探测。
        let mut keepalive_interval = interval(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));
        keepalive_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // v0.23.60: 退避重试——死模型不再每 60s 浪费一次探测，
        // 而是指数退避直到 1h 上限。
        let mut retry_interval = interval(Duration::from_secs(RETRY_BASE_INTERVAL_SECS));
        retry_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut retry_backoffs: HashMap<String, u64> = HashMap::new(); // model_id → 退避秒数

        loop {
            tokio::select! {
                _ = keepalive_interval.tick() => {
                    // v0.30.50: on_demand 模式（flag=false）闲置完全静默。
                    // 健康缓存过期后，网关 generate() 会自动内联探测——
                    // 即 issue #14 承诺的「仅按需探测」。
                    if probe_mode.load(Ordering::Relaxed) {
                        run_keepalive_probe(&executor).await;
                    }
                }
                _ = retry_interval.tick() => {
                    if probe_mode.load(Ordering::Relaxed) {
                        run_retry_probe_with_backoff(&executor, &mut retry_backoffs).await;
                    }
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
        match executor.probe_model(&model_id).await {
            Ok(result) if result.success => {}
            Ok(result) => {
                log::warn!(
                    "[GatewayScheduler] 启动探测 {} 失败: {}",
                    model_id,
                    result.error.unwrap_or_else(|| "探测未通过".to_string())
                );
            }
            Err(e) => {
                log::warn!("[GatewayScheduler] 启动探测 {} 失败: {}", model_id, e);
            }
        }
    }
}

/// v0.23.60: 快速保活探测——仅探测健康/降级模型，30s 超时。
///
/// 保持 HealthRegistry 的 `last_checked_at` 新鲜度，让网关 generate()
/// 读缓存时 <15s 即可跳过内联探测。
///
/// v0.99.26：加入按模型的新鲜度门禁（`keepalive_due`）——闲置模型 5 分钟
/// 一次，活跃模型 10s 一次，死模型交给退避重试。此前每 tick 无条件探测
/// 所有健康/降级模型，真机单日探测 15,584 次（真实生成仅 45 次）。
async fn run_keepalive_probe(executor: &GatewayExecutor) {
    let health = match executor.health_registry().lock() {
        Ok(g) => g.all(),
        Err(_) => return,
    };
    // v0.26.54: 只保活仍在 enabled 注册表中的模型，避免禁用后仍打探测
    let enabled_ids: std::collections::HashSet<String> = executor
        .registry
        .lock()
        .map(|g| {
            g.enabled_generative_models()
                .into_iter()
                .map(|m| m.id.clone())
                .collect()
        })
        .unwrap_or_default();

    let now = chrono::Utc::now();
    let mut probed = 0u32;
    for snapshot in health {
        if !enabled_ids.contains(&snapshot.model_id) {
            continue;
        }
        // 失败计数与真实活动时间一次性取出（锁不跨 await）。
        let (failures, last_real_activity) = match executor.health_registry().lock() {
            Ok(g) => (
                g.consecutive_failures(&snapshot.model_id),
                g.last_real_activity(&snapshot.model_id),
            ),
            Err(_) => (0, None),
        };
        if !keepalive_due(
            &snapshot.status,
            failures,
            snapshot.last_checked_at.as_deref(),
            last_real_activity,
            now,
        ) {
            continue;
        }
        // 30s 超时的轻量探测
        let start = Instant::now();
        match tokio::time::timeout(
            Duration::from_secs(KEEPALIVE_TIMEOUT_SECS),
            executor.probe_model(&snapshot.model_id),
        )
        .await
        {
            Ok(Ok(result)) if result.success => {
                probed += 1;
                log::debug!(
                    "[GatewayScheduler] keepalive {} OK ({:.0}ms)",
                    snapshot.model_id,
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
            Ok(Ok(result)) => {
                log::warn!(
                    "[GatewayScheduler] keepalive {} 失败: {}",
                    snapshot.model_id,
                    result.error.unwrap_or_else(|| "探测未通过".to_string())
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
    if probed > 0 {
        log::debug!(
            "[GatewayScheduler] keepalive 轮完成: {} 个模型探测通过",
            probed
        );
    }
}

/// v0.99.26: 本次 tick 是否应为该模型执行保活探测（纯函数）。
///
/// - 只保活健康/降级模型（Unhealthy 由退避重试负责，Unknown 由启动探测负责）；
/// - 连续失败 ≥3 的死模型不再保活，避免「保活失败 → 退避 → 保活又打」双路探测；
/// - 活跃模型（最近 2 分钟有真实生成）10s 一次，闲置模型 5 分钟一次；
/// - `last_checked_at` 缺失（从未探测）时立即探测。
fn keepalive_due(
    status: &super::types::HealthStatus,
    consecutive_failures: u32,
    last_checked_at: Option<&str>,
    last_real_activity_at: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    if !matches!(
        status,
        super::types::HealthStatus::Healthy | super::types::HealthStatus::Degraded
    ) {
        return false;
    }
    if consecutive_failures >= DEAD_MODEL_FAILURES {
        return false;
    }
    let active = last_real_activity_at
        .map(|t| (now - t).num_seconds() <= KEEPALIVE_ACTIVE_WINDOW_SECS)
        .unwrap_or(false);
    let required_secs = if active {
        KEEPALIVE_ACTIVE_INTERVAL_SECS
    } else {
        KEEPALIVE_IDLE_INTERVAL_SECS
    };
    match last_checked_at.and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok()) {
        Some(ts) => (now - ts.with_timezone(&chrono::Utc)).num_seconds() >= required_secs,
        None => true,
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
    // v0.26.54: 禁用模型不进入退避重试
    let enabled_ids: std::collections::HashSet<String> = executor
        .registry
        .lock()
        .map(|g| {
            g.enabled_generative_models()
                .into_iter()
                .map(|m| m.id.clone())
                .collect()
        })
        .unwrap_or_default();

    let now = Instant::now();
    for snapshot in health {
        if !enabled_ids.contains(&snapshot.model_id) {
            backoffs.remove(&snapshot.model_id);
            continue;
        }
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
            let outcome = executor.probe_model(&snapshot.model_id).await;
            let succeeded = probe_succeeded(&outcome);
            if succeeded {
                // 探测成功，重置退避
                backoffs.remove(&snapshot.model_id);
                log::info!(
                    "[GatewayScheduler] 退避重试 {} 成功，重置退避",
                    snapshot.model_id
                );
            } else {
                let reason = match &outcome {
                    Ok(result) => result
                        .error
                        .clone()
                        .unwrap_or_else(|| "探测未通过".to_string()),
                    Err(e) => e.to_string(),
                };
                log::warn!(
                    "[GatewayScheduler] 退避重试 {} 仍失败: {}",
                    snapshot.model_id,
                    reason
                );
                // 退避翻倍（封顶 1h）
                if let Some(next) = next_retry_backoff(current_backoff, false) {
                    backoffs.insert(snapshot.model_id.clone(), next);
                }
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

/// v0.99.25: 探测是否成功。
///
/// `probe_model` 对「调用失败」也返回 `Ok(ProbeResult { success: false, ..
/// })`， 只有「模型不存在/已禁用」这类前置校验才返回 `Err`。调度器曾只判断
/// `Err`， 把失败当成功 → 每次重置退避 → 死模型被永久每 30s 探测
/// （真机 v0.99.14 Windows：三模型连续失败 60+ 次仍停在「退避 30s」）。
fn probe_succeeded(result: &Result<ProbeResult, AppError>) -> bool {
    matches!(result, Ok(probe) if probe.success)
}

/// v0.99.25: 失败后退避翻倍（封顶 1h）；成功返回 `None`（重置）。
fn next_retry_backoff(current_secs: u64, succeeded: bool) -> Option<u64> {
    if succeeded {
        None
    } else {
        Some((current_secs * 2).min(RETRY_MAX_INTERVAL_SECS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_gateway::types::HealthStatus;

    fn probe(success: bool) -> ProbeResult {
        ProbeResult {
            success,
            ttft_ms: 0,
            total_tokens: 0,
            duration_ms: 0,
            tps: 0.0,
            error: if success {
                None
            } else {
                Some("boom".to_string())
            },
        }
    }

    #[test]
    fn probe_result_with_success_false_counts_as_failure() {
        // 回归：probe_model 对调用失败返回 Ok(success=false)，不能当成功。
        assert!(!probe_succeeded(&Ok(probe(false))));
        assert!(probe_succeeded(&Ok(probe(true))));
        assert!(!probe_succeeded(&Err(AppError::Internal {
            message: "x".into()
        })));
    }

    #[test]
    fn retry_backoff_doubles_until_cap_and_resets_on_success() {
        assert_eq!(next_retry_backoff(30, false), Some(60));
        assert_eq!(next_retry_backoff(1920, false), Some(3600));
        assert_eq!(next_retry_backoff(3600, false), Some(3600));
        assert_eq!(next_retry_backoff(30, true), None);
    }

    fn ts_secs_ago(secs: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::seconds(secs)).to_rfc3339()
    }

    #[test]
    fn keepalive_skips_unhealthy_dead_and_recent_models() {
        let now = chrono::Utc::now();
        // Unhealthy 交给退避重试
        assert!(!keepalive_due(&HealthStatus::Unhealthy, 1, None, None, now));
        // 连续失败 ≥3 的死模型不再保活
        assert!(!keepalive_due(
            &HealthStatus::Healthy,
            3,
            Some(&ts_secs_ago(3600)),
            None,
            now
        ));
        // 从未探测过：立即探测
        assert!(keepalive_due(&HealthStatus::Healthy, 0, None, None, now));
    }

    #[test]
    fn keepalive_idle_models_probe_every_five_minutes() {
        let now = chrono::Utc::now();
        // 闲置（无真实活动）：30s 前探测过 → 不重复
        assert!(!keepalive_due(
            &HealthStatus::Healthy,
            0,
            Some(&ts_secs_ago(30)),
            None,
            now
        ));
        // 闲置 6 分钟 → 到期
        assert!(keepalive_due(
            &HealthStatus::Healthy,
            0,
            Some(&ts_secs_ago(360)),
            None,
            now
        ));
        // 真实活动在 2 分钟窗口外，按闲置节奏
        assert!(!keepalive_due(
            &HealthStatus::Healthy,
            0,
            Some(&ts_secs_ago(30)),
            Some(now - chrono::Duration::seconds(200)),
            now
        ));
    }

    #[test]
    fn keepalive_active_models_probe_every_ten_seconds() {
        let now = chrono::Utc::now();
        let active = Some(now - chrono::Duration::seconds(20));
        // 活跃模型刚探测过 → 跳过
        assert!(!keepalive_due(
            &HealthStatus::Healthy,
            0,
            Some(&ts_secs_ago(5)),
            active,
            now
        ));
        // 活跃模型 15s 前探测过 → 到期（真机训练期间保持新鲜度）
        assert!(keepalive_due(
            &HealthStatus::Healthy,
            0,
            Some(&ts_secs_ago(15)),
            active,
            now
        ));
        // Degraded 但未连续失败 3 次：仍按节奏保活
        assert!(keepalive_due(
            &HealthStatus::Degraded,
            2,
            Some(&ts_secs_ago(400)),
            None,
            now
        ));
    }
}
