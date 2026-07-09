//! Model Gateway — 模型健康探测与注册表
//!
//! v0.14.0: 负责维护每个启用模型的实时健康快照，包括 TTFB、TPS、成功率。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::types::{HealthStatus, ModelHealthSnapshot, ProbeResult};
use crate::{config::settings::LlmProfile, db::DbPool};

/// 单个模型的历史健康记录
#[derive(Debug, Clone, Default)]
pub struct HealthRecord {
    pub snapshot: ModelHealthSnapshot,
    /// 最近若干次探测结果（用于计算平均 TTFB/TPS）
    pub recent_probes: Vec<ProbeResult>,
    /// v0.23.59: 连续失败次数（预探测失败 + 真实调用失败）。
    ///
    /// 真实调用失败（模型能说 "OK" 但无法生成正文）以前对调度器
    /// 完全不可见，活跃模型仍被 `+1000` 强制置顶。此计数让连续
    /// 失败的活跃模型降级，交给其他健康候选接管；成功 1 次即清零。
    pub consecutive_failures: u32,
}

/// 健康注册表
#[derive(Debug, Clone, Default)]
pub struct HealthRegistry {
    records: HashMap<String, HealthRecord>,
}

impl HealthRegistry {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// 获取单个模型健康快照
    pub fn get(&self, model_id: &str) -> Option<&ModelHealthSnapshot> {
        self.records.get(model_id).map(|r| &r.snapshot)
    }

    /// 获取所有模型健康快照
    pub fn all(&self) -> Vec<ModelHealthSnapshot> {
        self.records.values().map(|r| r.snapshot.clone()).collect()
    }

    /// 更新或插入模型健康快照
    pub fn update(&mut self, snapshot: ModelHealthSnapshot) {
        let record = self.records.entry(snapshot.model_id.clone()).or_default();
        record.snapshot = snapshot;
    }

    /// 获取所有 healthy / degraded 模型的 ID（用于路由候选过滤）
    pub fn usable_model_ids(&self) -> Vec<String> {
        self.records
            .values()
            .filter(|r| {
                matches!(
                    r.snapshot.status,
                    HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unknown
                )
            })
            .map(|r| r.snapshot.model_id.clone())
            .collect()
    }

    /// 从历史 llm_calls 聚合最近 24 小时成功率
    pub fn aggregate_success_rate_from_history(
        &mut self,
        model_id: &str,
        pool: &DbPool,
    ) -> Option<f64> {
        let conn = pool.get().ok()?;
        let since = chrono::Local::now() - chrono::Duration::hours(24);
        let since_str = since.to_rfc3339();

        let result = conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) \
             FROM llm_calls WHERE model_id = ?1 AND created_at > ?2",
            rusqlite::params![model_id, since_str],
            |row| {
                let total: i64 = row.get(0)?;
                let success: i64 = row.get(1)?;
                Ok((total, success))
            },
        );

        match result {
            Ok((total, success)) if total > 0 => {
                let rate = success as f64 / total as f64;
                if let Some(record) = self.records.get_mut(model_id) {
                    record.snapshot.success_rate_24h = Some(rate);
                }
                Some(rate)
            }
            _ => None,
        }
    }

    /// 根据探测结果更新模型健康状态
    /// v0.23.13: 健康状态严格由本次探测结果决定，不保留历史失败状态，
    /// 避免用户修复模型后仍然显示不可用。
    /// v0.23.59: 同步维护 `consecutive_failures`，成功清零、失败递增。
    pub fn apply_probe_result(
        &mut self,
        profile: &LlmProfile,
        result: &ProbeResult,
        pool: &DbPool,
    ) {
        let status = if result.success {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        };

        // 历史成功率仅用于展示，不再用来降级当前状态。
        let success_rate = self.aggregate_success_rate_from_history(&profile.id, pool);

        let record = self.records.entry(profile.id.clone()).or_default();
        // v0.23.59: 同步连续失败计数
        if result.success {
            record.consecutive_failures = 0;
        } else {
            record.consecutive_failures = record.consecutive_failures.saturating_add(1);
        }
        record.recent_probes.push(result.clone());
        // 保留最近 20 次
        if record.recent_probes.len() > 20 {
            record.recent_probes.remove(0);
        }

        let ttfb_values: Vec<u64> = record
            .recent_probes
            .iter()
            .filter(|p| p.success)
            .map(|p| p.ttft_ms)
            .collect();
        let tps_values: Vec<f64> = record
            .recent_probes
            .iter()
            .filter(|p| p.success && p.tps > 0.0)
            .map(|p| p.tps)
            .collect();

        let avg_ttfb = if !ttfb_values.is_empty() {
            Some(ttfb_values.iter().sum::<u64>() / ttfb_values.len() as u64)
        } else {
            None
        };
        let avg_tps = if !tps_values.is_empty() {
            Some(tps_values.iter().sum::<f64>() / tps_values.len() as f64)
        } else {
            None
        };

        record.snapshot = ModelHealthSnapshot {
            model_id: profile.id.clone(),
            model_name: profile.name.clone(),
            status,
            ttfb_ms: avg_ttfb,
            tps: avg_tps,
            success_rate_24h: success_rate,
            avg_latency_ms: None,
            last_error: result.error.clone(),
            last_checked_at: Some(chrono::Local::now().to_rfc3339()),
            enabled: profile.enabled,
            is_primary: false,
            is_fallback: false,
        };
    }

    /// v0.23.59: 记录一次真实调用失败（探测通过但生成失败）。
    ///
    /// 这是让"能说 OK 但无法生成正文"的模型对调度器可见的关键。
    /// 真实调用失败标记为 `Degraded`（而非 Unhealthy），保留在候选池
    /// 中（受 `-20` 分惩罚）但不再被强制置顶。
    pub fn record_failure(
        &mut self,
        model_id: &str,
        model_name: &str,
        status: HealthStatus,
        error: Option<String>,
    ) {
        let record = self.records.entry(model_id.to_string()).or_default();
        record.consecutive_failures = record.consecutive_failures.saturating_add(1);
        // 保留原 TTFB/TPS/success_rate 展示数据，仅更新状态与错误
        record.snapshot.status = status;
        record.snapshot.last_error = error;
        record.snapshot.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
        record.snapshot.model_id = model_id.to_string();
        if record.snapshot.model_name.is_empty() {
            record.snapshot.model_name = model_name.to_string();
        }
    }

    /// v0.23.59: 记录一次成功（预探测或真实调用），重置连续失败计数。
    ///
    /// 成功 1 次即清零，让用户修复模型后立即恢复强制置顶资格。
    pub fn record_success(&mut self, model_id: &str, model_name: &str) {
        let record = self.records.entry(model_id.to_string()).or_default();
        record.consecutive_failures = 0;
        record.snapshot.status = HealthStatus::Healthy;
        record.snapshot.last_error = None;
        record.snapshot.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
        record.snapshot.model_id = model_id.to_string();
        if record.snapshot.model_name.is_empty() {
            record.snapshot.model_name = model_name.to_string();
        }
    }

    /// v0.23.59: 查询模型连续失败次数（预探测 + 真实调用合计）。
    pub fn consecutive_failures(&self, model_id: &str) -> u32 {
        self.records
            .get(model_id)
            .map(|r| r.consecutive_failures)
            .unwrap_or(0)
    }

    /// v0.26.54: 用户显式重选模型时清除粘性降级。
    ///
    /// 连续失败阈值会让模型失去强制置顶资格；若用户在设置里再次指定该模型
    /// （创作/活跃），必须给一次重新探测机会，否则表现为「设了创作模型仍用
    /// 网关打分选出的本地模型」。
    ///
    /// - 清零 `consecutive_failures`
    /// - Unhealthy/Degraded → Unknown（允许 `is_promotable_user_model`，不假装
    ///   Healthy）
    /// - 无记录时 no-op
    pub fn clear_demotion(&mut self, model_id: &str) {
        let Some(record) = self.records.get_mut(model_id) else {
            return;
        };
        record.consecutive_failures = 0;
        if matches!(
            record.snapshot.status,
            HealthStatus::Unhealthy | HealthStatus::Degraded
        ) {
            record.snapshot.status = HealthStatus::Unknown;
            record.snapshot.last_error = None;
            record.snapshot.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    /// 移除指定模型的健康记录（删除模型时联动清除残留快照）
    pub fn purge(&mut self, model_id: &str) -> bool {
        self.records.remove(model_id).is_some()
    }

    /// 仅保留 valid_ids 中的模型健康记录（启动时清除 config 中已不存在的模型）
    pub fn retain(&mut self, valid_ids: &[String]) {
        self.records.retain(|k, _| valid_ids.iter().any(|v| v == k));
    }

    /// 从内存最近探测记录计算成功率（替代依赖 llm_calls 历史的聚合）
    pub fn probe_success_rate(&self, model_id: &str) -> Option<f64> {
        let record = self.records.get(model_id)?;
        if record.recent_probes.is_empty() {
            return None;
        }
        let total = record.recent_probes.len();
        let success = record.recent_probes.iter().filter(|p| p.success).count();
        Some(success as f64 / total as f64)
    }

    /// 返回内存中保留的探测次数（用于健康报告的 total_calls 字段）
    pub fn probe_count(&self, model_id: &str) -> usize {
        self.records
            .get(model_id)
            .map(|r| r.recent_probes.len())
            .unwrap_or(0)
    }
}

/// 探测引擎
#[derive(Clone)]
pub struct ProbeEngine {
    registry: Arc<Mutex<HealthRegistry>>,
}

impl ProbeEngine {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HealthRegistry::new())),
        }
    }

    pub fn registry(&self) -> Arc<Mutex<HealthRegistry>> {
        self.registry.clone()
    }

    /// 对单个模型执行轻量探测
    ///
    /// 实际调用由 executor 层注入，health 模块只负责记录与聚合。
    pub fn record_probe(&self, profile: &LlmProfile, result: &ProbeResult, pool: &DbPool) {
        if let Ok(mut reg) = self.registry.lock() {
            reg.apply_probe_result(profile, result, pool);
        }
    }
}

impl Default for ProbeEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_gateway::types::HealthStatus;

    fn make_snapshot(id: &str, status: HealthStatus) -> ModelHealthSnapshot {
        ModelHealthSnapshot {
            model_id: id.to_string(),
            model_name: format!("Model {}", id),
            status,
            ttfb_ms: Some(100),
            tps: Some(10.0),
            success_rate_24h: None,
            avg_latency_ms: None,
            last_error: None,
            last_checked_at: None,
            enabled: true,
            is_primary: false,
            is_fallback: false,
        }
    }

    #[test]
    fn test_purge_removes_entry() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Healthy));
        registry.update(make_snapshot("model-b", HealthStatus::Unhealthy));

        assert!(registry.get("model-a").is_some());
        assert!(registry.purge("model-a"));
        assert!(registry.get("model-a").is_none());
        assert!(registry.get("model-b").is_some());
    }

    #[test]
    fn test_purge_returns_false_for_missing() {
        let mut registry = HealthRegistry::new();
        assert!(!registry.purge("nonexistent"));
    }

    #[test]
    fn test_retain_keeps_only_valid_ids() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Healthy));
        registry.update(make_snapshot("model-b", HealthStatus::Healthy));
        registry.update(make_snapshot("model-c", HealthStatus::Unhealthy));

        registry.retain(&["model-a".to_string(), "model-c".to_string()]);

        assert!(registry.get("model-a").is_some());
        assert!(registry.get("model-b").is_none());
        assert!(registry.get("model-c").is_some());
    }

    #[test]
    fn test_retain_with_empty_ids_clears_all() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Healthy));
        registry.retain(&[]);
        assert!(registry.get("model-a").is_none());
        assert!(registry.all().is_empty());
    }

    #[test]
    fn test_probe_success_rate_none_when_no_probes() {
        let registry = HealthRegistry::new();
        // 没有 recent_probes 时返回 None
        assert_eq!(registry.probe_success_rate("model-a"), None);
    }

    #[test]
    fn test_probe_count_zero_when_missing() {
        let registry = HealthRegistry::new();
        assert_eq!(registry.probe_count("nonexistent"), 0);
    }

    #[test]
    fn test_probe_count_zero_when_no_probes() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Healthy));
        // 有快照但无探测记录时返回 0
        assert_eq!(registry.probe_count("model-a"), 0);
    }

    // v0.23.59: 连续失败计数与降级测试

    #[test]
    fn test_record_failure_increments_consecutive_count() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Healthy));

        registry.record_failure(
            "model-a",
            "Model A",
            HealthStatus::Degraded,
            Some("e1".into()),
        );
        assert_eq!(registry.consecutive_failures("model-a"), 1);
        assert_eq!(
            registry.get("model-a").unwrap().status,
            HealthStatus::Degraded
        );

        registry.record_failure(
            "model-a",
            "Model A",
            HealthStatus::Degraded,
            Some("e2".into()),
        );
        assert_eq!(registry.consecutive_failures("model-a"), 2);
    }

    #[test]
    fn test_record_success_resets_consecutive_count() {
        let mut registry = HealthRegistry::new();
        registry.update(make_snapshot("model-a", HealthStatus::Degraded));

        registry.record_failure(
            "model-a",
            "Model A",
            HealthStatus::Degraded,
            Some("e1".into()),
        );
        registry.record_failure(
            "model-a",
            "Model A",
            HealthStatus::Degraded,
            Some("e2".into()),
        );
        assert_eq!(registry.consecutive_failures("model-a"), 2);

        registry.record_success("model-a", "Model A");
        assert_eq!(registry.consecutive_failures("model-a"), 0);
        assert_eq!(
            registry.get("model-a").unwrap().status,
            HealthStatus::Healthy
        );
        assert!(registry.get("model-a").unwrap().last_error.is_none());
    }

    #[test]
    fn test_consecutive_failures_zero_for_missing_model() {
        let registry = HealthRegistry::new();
        assert_eq!(registry.consecutive_failures("nonexistent"), 0);
    }

    #[test]
    fn test_record_failure_creates_record_if_missing() {
        let mut registry = HealthRegistry::new();
        // 模型从未被探测过，直接记录失败（如 generate_with_fastest 探测失败）
        registry.record_failure(
            "model-new",
            "Model New",
            HealthStatus::Unhealthy,
            Some("probe timeout".into()),
        );
        assert_eq!(registry.consecutive_failures("model-new"), 1);
        assert_eq!(
            registry.get("model-new").unwrap().status,
            HealthStatus::Unhealthy
        );
        assert_eq!(registry.get("model-new").unwrap().model_name, "Model New");
    }

    #[test]
    fn test_clear_demotion_resets_failures_and_unhealthy_to_unknown() {
        let mut registry = HealthRegistry::new();
        registry.record_failure(
            "ds",
            "Deepseek",
            HealthStatus::Unhealthy,
            Some("timeout".into()),
        );
        registry.record_failure(
            "ds",
            "Deepseek",
            HealthStatus::Unhealthy,
            Some("timeout".into()),
        );
        assert_eq!(registry.consecutive_failures("ds"), 2);
        assert_eq!(registry.get("ds").unwrap().status, HealthStatus::Unhealthy);

        registry.clear_demotion("ds");
        assert_eq!(registry.consecutive_failures("ds"), 0);
        assert_eq!(registry.get("ds").unwrap().status, HealthStatus::Unknown);
        // no-op for missing
        registry.clear_demotion("missing");
    }
}
