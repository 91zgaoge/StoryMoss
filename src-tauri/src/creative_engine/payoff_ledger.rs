//! Payoff Ledger - 伏笔账本系统
//!
//! 扩展 ForeshadowingTracker，提供时间窗口追踪、逾期检测、风险信号、
//! 回收时机推荐等高级功能。

use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

/// 伏笔作用域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    Story, // 全故事级伏笔
    Arc,   // 故事弧级伏笔
    Scene, // 单场景级伏笔
}

impl std::fmt::Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Story => write!(f, "story"),
            ScopeType::Arc => write!(f, "arc"),
            ScopeType::Scene => write!(f, "scene"),
        }
    }
}

impl std::str::FromStr for ScopeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "story" => Ok(ScopeType::Story),
            "arc" => Ok(ScopeType::Arc),
            "scene" => Ok(ScopeType::Scene),
            _ => Err(format!("Unknown scope type: {}", s)),
        }
    }
}

/// 伏笔账本状态（比 DB 层更丰富的状态机）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoffStatus {
    Setup,         // 已设置，尚未有进一步暗示
    Hinted,        // 已有暗示/呼应
    PendingPayoff, // 临近回收窗口
    PaidOff,       // 已回收
    Failed,        // 已放弃/失效
    Overdue,       // 已逾期
}

impl std::fmt::Display for PayoffStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayoffStatus::Setup => write!(f, "setup"),
            PayoffStatus::Hinted => write!(f, "hinted"),
            PayoffStatus::PendingPayoff => write!(f, "pending_payoff"),
            PayoffStatus::PaidOff => write!(f, "paid_off"),
            PayoffStatus::Failed => write!(f, "failed"),
            PayoffStatus::Overdue => write!(f, "overdue"),
        }
    }
}

/// 伏笔账本条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoffLedgerItem {
    pub id: String,
    pub ledger_key: String,
    pub title: String,
    pub summary: String,
    pub scope_type: ScopeType,
    pub current_status: PayoffStatus,
    pub target_start_scene: Option<i32>,
    pub target_end_scene: Option<i32>,
    pub first_seen_scene: Option<i32>,
    pub last_touched_scene: Option<i32>,
    pub confidence: f32,
    pub risk_signals: Vec<String>,
    pub importance: i32,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// 回收时机推荐
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoffRecommendation {
    pub foreshadowing_id: String,
    pub ledger_key: String,
    pub title: String,
    pub recommended_scene: i32,
    pub urgency: UrgencyLevel,
    pub reason: String,
    pub importance: i32,
}

/// 紧急程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrgencyLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for UrgencyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrgencyLevel::Low => write!(f, "low"),
            UrgencyLevel::Medium => write!(f, "medium"),
            UrgencyLevel::High => write!(f, "high"),
            UrgencyLevel::Critical => write!(f, "critical"),
        }
    }
}

/// 伏笔账本
pub struct PayoffLedger {
    pool: DbPool,
}

impl PayoffLedger {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 获取故事的完整伏笔账本
    pub fn get_ledger(&self, story_id: &str) -> Result<Vec<PayoffLedgerItem>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status,
                        importance, created_at, resolved_at, target_start_scene,
                        target_end_scene, risk_signals, scope_type, ledger_key
                 FROM foreshadowing_tracker
                 WHERE story_id = ?1
                 ORDER BY importance DESC, created_at ASC",
            )
            .map_err(|e| format!("准备查询失败: {}", e))?;

        let rows = stmt
            .query_map([story_id], |row| {
                let id: String = row.get(0)?;
                let content: String = row.get(2)?;
                let setup_scene_id: Option<String> = row.get(3)?;
                let payoff_scene_id: Option<String> = row.get(4)?;
                let status_str: String = row.get(5)?;
                let importance: i32 = row.get(6)?;
                let created_at: String = row.get(7)?;
                let resolved_at: Option<String> = row.get(8)?;
                let target_start_scene: Option<i32> = row.get(9)?;
                let target_end_scene: Option<i32> = row.get(10)?;
                let risk_signals_raw: Option<String> = row.get(11)?;
                let scope_type_str: String = row.get(12)?;
                let ledger_key_opt: Option<String> = row.get(13)?;

                Ok((
                    id,
                    content,
                    setup_scene_id,
                    payoff_scene_id,
                    status_str,
                    importance,
                    created_at,
                    resolved_at,
                    target_start_scene,
                    target_end_scene,
                    risk_signals_raw,
                    scope_type_str,
                    ledger_key_opt,
                ))
            })
            .map_err(|e| format!("查询失败: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("映射失败: {}", e))?;

        // 批量查询场景序号（避免 N+1）
        let scene_ids: Vec<String> = rows
            .iter()
            .filter_map(|r| r.2.clone())
            .chain(rows.iter().filter_map(|r| r.3.clone()))
            .collect::<std::collections::HashSet<String>>()
            .into_iter()
            .collect();

        let mut scene_sequence_map: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        if !scene_ids.is_empty() {
            let placeholders = scene_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT id, sequence_number FROM scenes WHERE id IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&sql).map_err(AppError::from)?;
            let params: Vec<&dyn rusqlite::ToSql> = scene_ids
                .iter()
                .map(|id| id as &dyn rusqlite::ToSql)
                .collect();
            let sequences = stmt
                .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                    let sid: String = row.get(0)?;
                    let seq: i32 = row.get(1)?;
                    Ok((sid, seq))
                })
                .map_err(AppError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)?;
            for (sid, seq) in sequences {
                scene_sequence_map.insert(sid, seq);
            }
        }

        let items: Vec<PayoffLedgerItem> = rows
            .into_iter()
            .map(
                |(
                    id,
                    content,
                    setup_scene_id,
                    payoff_scene_id,
                    status_str,
                    importance,
                    created_at,
                    resolved_at,
                    target_start_scene,
                    target_end_scene,
                    risk_signals_raw,
                    scope_type_str,
                    ledger_key_opt,
                )| {
                    let first_seen_scene = setup_scene_id
                        .as_ref()
                        .and_then(|sid| scene_sequence_map.get(sid).copied());
                    let last_touched_scene = payoff_scene_id
                        .as_ref()
                        .and_then(|sid| scene_sequence_map.get(sid).copied())
                        .or(first_seen_scene);

                    let scope_type = scope_type_str.parse().unwrap_or(ScopeType::Story);
                    let current_status = match status_str.as_str() {
                        "setup" => PayoffStatus::Setup,
                        "payoff" => PayoffStatus::PaidOff,
                        "abandoned" => PayoffStatus::Failed,
                        _ => PayoffStatus::Setup,
                    };

                    let risk_signals: Vec<String> = risk_signals_raw
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();

                    let ledger_key = ledger_key_opt.unwrap_or_else(|| id.clone());
                    let title = if content.len() > 30 {
                        format!("{}...", &content[..30])
                    } else {
                        content.clone()
                    };

                    PayoffLedgerItem {
                        id: id.clone(),
                        ledger_key,
                        title,
                        summary: content,
                        scope_type,
                        current_status,
                        target_start_scene,
                        target_end_scene,
                        first_seen_scene,
                        last_touched_scene,
                        confidence: (importance as f32 / 10.0).clamp(0.0, 1.0),
                        risk_signals,
                        importance,
                        created_at,
                        resolved_at,
                    }
                },
            )
            .collect();

        Ok(items)
    }

    /// 检测逾期伏笔
    pub fn detect_overdue(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<PayoffLedgerItem>, AppError> {
        let ledger = self.get_ledger(story_id)?;

        let mut overdue_items = Vec::new();
        for mut item in ledger {
            // 只检查 setup / hinted / pending_payoff 状态的伏笔
            let is_active = matches!(
                item.current_status,
                PayoffStatus::Setup | PayoffStatus::Hinted | PayoffStatus::PendingPayoff
            );
            if !is_active {
                continue;
            }

            let is_overdue = if let Some(target_end) = item.target_end_scene {
                // 如果 target_end_scene 不为空且已过当前场景
                target_end < current_scene_number
            } else if let Some(first_seen) = item.first_seen_scene {
                // 如果 target_end_scene 为空但已设置超过 10 个场景仍未回收
                current_scene_number - first_seen > 10
            } else {
                // 无法判断，不标记逾期
                false
            };

            if is_overdue {
                item.current_status = PayoffStatus::Overdue;
                overdue_items.push(item);
            }
        }

        // 按重要性排序
        overdue_items.sort_by(|a, b| b.importance.cmp(&a.importance));
        Ok(overdue_items)
    }

    /// 推荐回收时机
    pub fn recommend_payoff_timing(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<PayoffRecommendation>, AppError> {
        let ledger = self.get_ledger(story_id)?;

        // 估算故事总场景数（用于判断 narrative_phase）
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;
        let total_scenes: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) FROM scenes WHERE story_id = ?1",
                [story_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let climax_threshold = if total_scenes > 0 {
            (total_scenes as f32 * 0.75) as i32
        } else {
            i32::MAX
        };
        let is_climax_phase = current_scene_number >= climax_threshold;

        let mut recommendations = Vec::new();
        for item in ledger {
            // 只考虑活跃的 setup 状态伏笔（视为 pending_payoff）
            if !matches!(
                item.current_status,
                PayoffStatus::Setup | PayoffStatus::Hinted | PayoffStatus::PendingPayoff
            ) {
                continue;
            }

            // 跳过已逾期的（由 detect_overdue 处理）
            if let Some(target_end) = item.target_end_scene {
                if target_end < current_scene_number {
                    continue;
                }
            }

            // 计算推荐场景
            let recommended_scene = if let Some(target_start) = item.target_start_scene {
                if target_start >= current_scene_number {
                    target_start
                } else {
                    current_scene_number
                }
            } else {
                current_scene_number
            };

            // 如果 target_end 存在且很近，推荐在窗口内
            let recommended_scene = if let Some(target_end) = item.target_end_scene {
                if target_end <= current_scene_number + 3 {
                    target_end.min(recommended_scene)
                } else {
                    recommended_scene
                }
            } else {
                recommended_scene
            };

            // 紧急度计算
            let urgency = if is_climax_phase && item.importance >= 7 {
                UrgencyLevel::Critical
            } else if is_climax_phase && item.importance >= 5 {
                UrgencyLevel::High
            } else if item.target_end_scene.is_some()
                && item.target_end_scene.unwrap() <= current_scene_number + 2
            {
                UrgencyLevel::High
            } else if item.importance >= 8 {
                UrgencyLevel::Medium
            } else {
                UrgencyLevel::Low
            };

            let reason = if is_climax_phase && item.importance >= 7 {
                format!(
                    "当前处于高潮阶段（场景 {}+），重要伏笔（{}/10）建议尽快回收",
                    climax_threshold, item.importance
                )
            } else if item.target_end_scene.is_some() {
                format!(
                    "目标回收窗口将在场景 {} 结束",
                    item.target_end_scene.unwrap()
                )
            } else {
                format!(
                    "建议在当前或接下来 3 个场景内兑现（场景 {}–{}）",
                    recommended_scene,
                    (recommended_scene + 3).min(total_scenes)
                )
            };

            recommendations.push(PayoffRecommendation {
                foreshadowing_id: item.id,
                ledger_key: item.ledger_key,
                title: item.title,
                recommended_scene,
                urgency,
                reason,
                importance: item.importance,
            });
        }

        // 按紧急度 + 重要性排序
        recommendations.sort_by(|a, b| {
            let urgency_order = |u: &UrgencyLevel| match u {
                UrgencyLevel::Critical => 0,
                UrgencyLevel::High => 1,
                UrgencyLevel::Medium => 2,
                UrgencyLevel::Low => 3,
            };
            let ord = urgency_order(&a.urgency).cmp(&urgency_order(&b.urgency));
            if ord == std::cmp::Ordering::Equal {
                b.importance.cmp(&a.importance)
            } else {
                ord
            }
        });

        Ok(recommendations)
    }

    /// 更新伏笔的账本字段（供未来 UI 调用）
    pub fn update_ledger_fields(
        &self,
        foreshadowing_id: &str,
        target_start_scene: Option<i32>,
        target_end_scene: Option<i32>,
        risk_signals: Option<Vec<String>>,
        scope_type: Option<ScopeType>,
        ledger_key: Option<String>,
    ) -> Result<(), AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        if let Some(ts) = target_start_scene {
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_start_scene = ?1 WHERE id = ?2",
                rusqlite::params![ts, foreshadowing_id],
            )
            .map_err(|e| format!("更新账本字段失败: {}", e))?;
        }
        if let Some(te) = target_end_scene {
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = ?1 WHERE id = ?2",
                rusqlite::params![te, foreshadowing_id],
            )
            .map_err(|e| format!("更新账本字段失败: {}", e))?;
        }
        if let Some(ref rs) = risk_signals {
            let json = serde_json::to_string(rs).map_err(AppError::from)?;
            conn.execute(
                "UPDATE foreshadowing_tracker SET risk_signals = ?1 WHERE id = ?2",
                rusqlite::params![json, foreshadowing_id],
            )
            .map_err(|e| format!("更新账本字段失败: {}", e))?;
        }
        if let Some(ref st) = scope_type {
            conn.execute(
                "UPDATE foreshadowing_tracker SET scope_type = ?1 WHERE id = ?2",
                rusqlite::params![st.to_string(), foreshadowing_id],
            )
            .map_err(|e| format!("更新账本字段失败: {}", e))?;
        }
        if let Some(ref lk) = ledger_key {
            conn.execute(
                "UPDATE foreshadowing_tracker SET ledger_key = ?1 WHERE id = ?2",
                rusqlite::params![lk, foreshadowing_id],
            )
            .map_err(|e| format!("更新账本字段失败: {}", e))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_type_display() {
        assert_eq!(ScopeType::Story.to_string(), "story");
        assert_eq!(ScopeType::Arc.to_string(), "arc");
        assert_eq!(ScopeType::Scene.to_string(), "scene");
    }

    #[test]
    fn test_payoff_status_display() {
        assert_eq!(PayoffStatus::Setup.to_string(), "setup");
        assert_eq!(PayoffStatus::Overdue.to_string(), "overdue");
    }

    #[test]
    fn test_urgency_level_order() {
        let mut levels = vec![
            UrgencyLevel::Low,
            UrgencyLevel::Critical,
            UrgencyLevel::Medium,
            UrgencyLevel::High,
        ];
        levels.sort_by(|a, b| {
            let order = |u: &UrgencyLevel| match u {
                UrgencyLevel::Critical => 0,
                UrgencyLevel::High => 1,
                UrgencyLevel::Medium => 2,
                UrgencyLevel::Low => 3,
            };
            order(a).cmp(&order(b))
        });
        assert!(matches!(levels[0], UrgencyLevel::Critical));
        assert!(matches!(levels[3], UrgencyLevel::Low));
    }
}
