#![allow(dead_code)]
//! Foreshadowing Service - 伏笔 / 线索 / 回报统一真源
//!
//! 作为 story_system 的一部分，集中管理 `foreshadowing_tracker` 表的所有读写，
//! 为 creative_engine、narrative、commands 等模块提供单一可信接口。

use std::collections::{HashMap, HashSet};

use chrono::Local;
use rusqlite::params;
use uuid::Uuid;

use crate::{
    db::DbPool,
    domain::foreshadowing::{
        ForeshadowingError, ForeshadowingProvider, ForeshadowingRecord, ForeshadowingService,
        ForeshadowingStatus, Payoff, PayoffLedgerItem, PayoffRecommendation, PayoffStatus,
        ScopeType, UrgencyLevel,
    },
};

/// 把基础设施错误统一包成领域内部错误，避免 `domain::ForeshadowingError` 依赖
/// `rusqlite`/`r2d2`。
fn into_internal<E: std::fmt::Display>(err: E) -> ForeshadowingError {
    ForeshadowingError::Internal(err.to_string())
}

/// 伏笔服务实现
pub struct ForeshadowingServiceImpl {
    pool: DbPool,
}

impl ForeshadowingServiceImpl {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn conn(
        &self,
    ) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, ForeshadowingError>
    {
        self.pool.get().map_err(into_internal)
    }

    /// 获取故事当前最大场景序号。
    pub fn current_scene_number(&self, story_id: &str) -> Result<i32, ForeshadowingError> {
        let conn = self.conn()?;
        let seq: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) FROM scenes WHERE story_id = ?1",
                [story_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(seq)
    }

    fn map_row(&self, row: &rusqlite::Row) -> Result<ForeshadowingRecord, rusqlite::Error> {
        let status_str: String = row.get(5)?;
        let status = status_str.parse().unwrap_or(ForeshadowingStatus::Setup);

        Ok(ForeshadowingRecord {
            id: row.get(0)?,
            story_id: row.get(1)?,
            content: row.get(2)?,
            setup_scene_id: row.get(3)?,
            payoff_scene_id: row.get(4)?,
            status,
            importance: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8)?,
            setup_event_id: row.get(9)?,
            payoff_event_id: row.get(10)?,
            risk_signals_score: row.get(11)?,
            target_start_scene: row.get(12)?,
            target_end_scene: row.get(13)?,
            evidence: row.get(14)?,
        })
    }

    fn scene_sequence_map(
        &self,
        conn: &rusqlite::Connection,
        scene_ids: &[String],
    ) -> Result<HashMap<String, i32>, ForeshadowingError> {
        let mut map = HashMap::new();
        if scene_ids.is_empty() {
            return Ok(map);
        }

        let placeholders = scene_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, sequence_number FROM scenes WHERE id IN ({})",
            placeholders
        );
        let mut stmt = conn.prepare(&sql).map_err(into_internal)?;
        let params: Vec<&dyn rusqlite::ToSql> = scene_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let sequences = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })
            .map_err(into_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_internal)?;
        for (sid, seq) in sequences {
            map.insert(sid, seq);
        }
        Ok(map)
    }

    fn build_ledger_items(
        &self,
        conn: &rusqlite::Connection,
        story_id: &str,
    ) -> Result<Vec<PayoffLedgerItem>, ForeshadowingError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status,
                        importance, created_at, resolved_at, target_start_scene,
                        target_end_scene, risk_signals, scope_type, ledger_key
                 FROM foreshadowing_tracker
                 WHERE story_id = ?1
                 ORDER BY importance DESC, created_at ASC",
            )
            .map_err(into_internal)?;

        let rows = stmt
            .query_map([story_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i32>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<i32>>(9)?,
                    row.get::<_, Option<i32>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, Option<String>>(13)?,
                ))
            })
            .map_err(into_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_internal)?;

        let scene_ids: Vec<String> = rows
            .iter()
            .filter_map(|r| r.2.clone())
            .chain(rows.iter().filter_map(|r| r.3.clone()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let sequence_map = self.scene_sequence_map(conn, &scene_ids)?;

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
                        .and_then(|sid| sequence_map.get(sid).copied());
                    let last_touched_scene = payoff_scene_id
                        .as_ref()
                        .and_then(|sid| sequence_map.get(sid).copied())
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
                    // 按字符数（非字节数）截取 title 预览，避免 &content[..30]
                    // 在中文字符中间切分导致 panic（byte index not a char
                    // boundary）。
                    let title = if content.chars().count() > 30 {
                        format!("{}...", content.chars().take(30).collect::<String>())
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
}

impl ForeshadowingProvider for ForeshadowingServiceImpl {
    fn list_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status,
                 importance, created_at, resolved_at, setup_event_id, payoff_event_id,
                 risk_signals_score, target_start_scene, target_end_scene, evidence
             FROM foreshadowing_tracker WHERE story_id = ?1
             ORDER BY importance DESC, created_at ASC",
            )
            .map_err(into_internal)?;

        let records = stmt
            .query_map([story_id], |row| self.map_row(row))
            .map_err(into_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_internal)?;
        Ok(records)
    }

    fn get_by_id(&self, id: &str) -> Result<Option<ForeshadowingRecord>, ForeshadowingError> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status,
                 importance, created_at, resolved_at, setup_event_id, payoff_event_id,
                 risk_signals_score, target_start_scene, target_end_scene, evidence
             FROM foreshadowing_tracker WHERE id = ?1",
            )
            .map_err(into_internal)?;

        let mut records: Vec<ForeshadowingRecord> = stmt
            .query_map([id], |row| self.map_row(row))
            .map_err(into_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_internal)?;
        Ok(records.pop())
    }

    fn get_unresolved(
        &self,
        story_id: &str,
    ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status,
                 importance, created_at, resolved_at, setup_event_id, payoff_event_id,
                 risk_signals_score, target_start_scene, target_end_scene, evidence
             FROM foreshadowing_tracker WHERE story_id = ?1 AND status = 'setup'
             ORDER BY importance DESC, created_at ASC",
            )
            .map_err(into_internal)?;

        let records = stmt
            .query_map([story_id], |row| self.map_row(row))
            .map_err(into_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(into_internal)?;
        Ok(records)
    }

    fn get_overdue(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
        let overdue_ids = {
            let conn = self.conn()?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, setup_scene_id, importance, target_end_scene
                     FROM foreshadowing_tracker
                     WHERE story_id = ?1 AND status = 'setup'",
                )
                .map_err(into_internal)?;

            let rows: Vec<_> = stmt
                .query_map([story_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, Option<i32>>(3)?,
                    ))
                })
                .map_err(into_internal)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(into_internal)?;

            let scene_ids: Vec<String> = rows
                .iter()
                .filter_map(|r| r.1.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let sequence_map = self.scene_sequence_map(&conn, &scene_ids)?;

            let mut ids = Vec::new();
            for (id, setup_scene_id, importance, target_end_scene) in rows {
                let first_seen_scene = setup_scene_id
                    .as_ref()
                    .and_then(|sid| sequence_map.get(sid).copied());

                let is_overdue = if let Some(target_end) = target_end_scene {
                    target_end < current_scene_number
                } else if let Some(first_seen) = first_seen_scene {
                    let threshold = match importance {
                        8..=10 => 5,
                        5..=7 => 10,
                        _ => 15,
                    };
                    current_scene_number - first_seen > threshold
                } else {
                    false
                };

                if is_overdue {
                    ids.push(id);
                }
            }
            ids
        };

        let mut overdue = Vec::new();
        for id in overdue_ids {
            if let Some(record) = self.get_by_id(&id)? {
                overdue.push(record);
            }
        }

        Ok(overdue)
    }

    fn get_writing_hints(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<String>, ForeshadowingError> {
        let unresolved = self.get_unresolved(story_id)?;
        let current = self.current_scene_number(story_id).unwrap_or(0);
        let hints: Vec<String> = unresolved
            .into_iter()
            .take(limit)
            .map(|r| {
                let importance_marker = match r.importance {
                    8..=10 => "【关键】",
                    5..=7 => "【重要】",
                    _ => "【次要】",
                };
                // v0.62.0 P2-C：把「计划回收窗口」与「实际回收」分离后，
                // 注入话术可以分档——临近窗口的明确禁止提前回收，
                // 已过窗口的要求尽快回收（对齐外部项目的最佳实践）。
                let window_note = match r.target_end_scene {
                    Some(end) if current > 0 && end > current && end - current <= 5 => {
                        format!("（计划第{end}章前后回收，请勿提前回收）")
                    }
                    Some(end) if current > 0 && end <= current => {
                        format!("（已到/超过计划回收点第{end}章，请尽快回收）")
                    }
                    _ => String::new(),
                };
                format!(
                    "{} 未回收伏笔: {}{}",
                    importance_marker, r.content, window_note
                )
            })
            .collect();
        Ok(hints)
    }

    fn detect_payoffs(&self, story_id: &str) -> Result<Vec<Payoff>, ForeshadowingError> {
        let records = self.list_by_story(story_id)?;
        Ok(records
            .into_iter()
            .map(|r| Payoff {
                foreshadowing_id: r.id,
                content: r.content,
                importance: r.importance,
                setup_scene_id: r.setup_scene_id,
                payoff_scene_id: r.payoff_scene_id,
                status: r.status,
            })
            .collect())
    }

    fn recommend_payoffs(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<PayoffRecommendation>, ForeshadowingError> {
        let conn = self.conn()?;
        let ledger = self.build_ledger_items(&conn, story_id)?;

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
            if !matches!(
                item.current_status,
                PayoffStatus::Setup | PayoffStatus::Hinted | PayoffStatus::PendingPayoff
            ) {
                continue;
            }

            if let Some(target_end) = item.target_end_scene {
                if target_end < current_scene_number {
                    continue;
                }
            }

            let mut recommended_scene = if let Some(target_start) = item.target_start_scene {
                if target_start >= current_scene_number {
                    target_start
                } else {
                    current_scene_number
                }
            } else {
                current_scene_number
            };

            if let Some(target_end) = item.target_end_scene {
                if target_end <= current_scene_number + 3 {
                    recommended_scene = target_end.min(recommended_scene);
                }
            }

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

    fn get_ledger(&self, story_id: &str) -> Result<Vec<PayoffLedgerItem>, ForeshadowingError> {
        let conn = self.conn()?;
        self.build_ledger_items(&conn, story_id)
    }
}

impl ForeshadowingService for ForeshadowingServiceImpl {
    fn create(
        &self,
        story_id: &str,
        content: &str,
        setup_scene_id: Option<&str>,
        importance: i32,
    ) -> Result<String, ForeshadowingError> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO foreshadowing_tracker (id, story_id, content, setup_scene_id, status, \
             importance, created_at)
             VALUES (?1, ?2, ?3, ?4, 'setup', ?5, ?6)",
            params![
                &id,
                story_id,
                content,
                setup_scene_id,
                importance.clamp(1, 10),
                now
            ],
        )
        .map_err(into_internal)?;

        Ok(id)
    }

    fn mark_payoff(
        &self,
        foreshadowing_id: &str,
        payoff_scene_id: Option<&str>,
    ) -> Result<(), ForeshadowingError> {
        let now = Local::now().to_rfc3339();
        let conn = self.conn()?;

        conn.execute(
            "UPDATE foreshadowing_tracker SET status = 'payoff', payoff_scene_id = ?2, \
             resolved_at = ?3 WHERE id = ?1",
            params![foreshadowing_id, payoff_scene_id, now],
        )
        .map_err(into_internal)?;

        Ok(())
    }

    fn abandon(&self, foreshadowing_id: &str) -> Result<(), ForeshadowingError> {
        let now = Local::now().to_rfc3339();
        let conn = self.conn()?;

        conn.execute(
            "UPDATE foreshadowing_tracker SET status = 'abandoned', resolved_at = ?2 WHERE id = ?1",
            params![foreshadowing_id, now],
        )
        .map_err(into_internal)?;

        Ok(())
    }

    fn update(
        &self,
        foreshadowing_id: &str,
        content: &str,
        importance: i32,
        setup_scene_id: Option<&str>,
    ) -> Result<(), ForeshadowingError> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE foreshadowing_tracker SET content = ?2, importance = ?3, setup_scene_id = ?4 \
             WHERE id = ?1",
            params![
                foreshadowing_id,
                content,
                importance.clamp(1, 10),
                setup_scene_id
            ],
        )
        .map_err(into_internal)?;

        Ok(())
    }

    fn delete(&self, foreshadowing_id: &str) -> Result<(), ForeshadowingError> {
        let conn = self.conn()?;

        conn.execute(
            "DELETE FROM foreshadowing_tracker WHERE id = ?1",
            params![foreshadowing_id],
        )
        .map_err(into_internal)?;

        Ok(())
    }

    fn update_ledger_fields(
        &self,
        foreshadowing_id: &str,
        target_start_scene: Option<i32>,
        target_end_scene: Option<i32>,
        risk_signals: Option<Vec<String>>,
        scope_type: Option<ScopeType>,
        ledger_key: Option<String>,
        setup_event_id: Option<String>,
        payoff_event_id: Option<String>,
        risk_signals_score: Option<f32>,
    ) -> Result<(), ForeshadowingError> {
        let conn = self.conn()?;

        if let Some(ts) = target_start_scene {
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_start_scene = ?1 WHERE id = ?2",
                rusqlite::params![ts, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(te) = target_end_scene {
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = ?1 WHERE id = ?2",
                rusqlite::params![te, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(ref rs) = risk_signals {
            let json = serde_json::to_string(rs).map_err(into_internal)?;
            conn.execute(
                "UPDATE foreshadowing_tracker SET risk_signals = ?1 WHERE id = ?2",
                rusqlite::params![json, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(ref st) = scope_type {
            conn.execute(
                "UPDATE foreshadowing_tracker SET scope_type = ?1 WHERE id = ?2",
                rusqlite::params![st.to_string(), foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(ref lk) = ledger_key {
            conn.execute(
                "UPDATE foreshadowing_tracker SET ledger_key = ?1 WHERE id = ?2",
                rusqlite::params![lk, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(ref seid) = setup_event_id {
            conn.execute(
                "UPDATE foreshadowing_tracker SET setup_event_id = ?1 WHERE id = ?2",
                rusqlite::params![seid, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(ref peid) = payoff_event_id {
            conn.execute(
                "UPDATE foreshadowing_tracker SET payoff_event_id = ?1 WHERE id = ?2",
                rusqlite::params![peid, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }
        if let Some(rss) = risk_signals_score {
            conn.execute(
                "UPDATE foreshadowing_tracker SET risk_signals_score = ?1 WHERE id = ?2",
                rusqlite::params![rss, foreshadowing_id],
            )
            .map_err(into_internal)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::RustMigration;

    fn in_memory_pool() -> DbPool {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(manager).unwrap();
        let mut conn = pool.get().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );
            CREATE TABLE stories (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT,
                status TEXT,
                word_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE scenes (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_id TEXT,
                title TEXT,
                content TEXT,
                sequence_number INTEGER NOT NULL,
                act_number INTEGER,
                position_in_act INTEGER,
                narrative_intensity REAL,
                narrative_sentiment REAL,
                narrative_event_types TEXT,
                confidence_score REAL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        // Minimal foreshadowing_tracker schema (V015 + V027 + V079 + V137).
        conn.execute_batch(
            "CREATE TABLE foreshadowing_tracker (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                content TEXT NOT NULL,
                setup_scene_id TEXT,
                payoff_scene_id TEXT,
                status TEXT NOT NULL DEFAULT 'setup',
                importance INTEGER,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                setup_event_id TEXT,
                payoff_event_id TEXT,
                risk_signals_score REAL DEFAULT 0.0,
                target_start_scene INTEGER,
                target_end_scene INTEGER,
                risk_signals TEXT,
                scope_type TEXT DEFAULT 'story',
                ledger_key TEXT,
                evidence TEXT,
                strength INTEGER DEFAULT 5,
                subtlety INTEGER DEFAULT 5,
                related_foreshadow_ids TEXT DEFAULT '[]'
            );",
        )
        .unwrap();
        pool
    }

    fn seed_story_and_scenes(pool: &DbPool, story_id: &str) {
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, 'Test', ?2, ?2)",
            rusqlite::params![story_id, now],
        )
        .unwrap();
        for (i, scene_id) in ["s1", "s2", "s3", "s4", "s5"].iter().enumerate() {
            conn.execute(
                "INSERT INTO scenes (id, story_id, title, sequence_number, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                rusqlite::params![
                    scene_id,
                    story_id,
                    format!("Scene {}", i + 1),
                    (i + 1) as i32,
                    now
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn service_create_and_list() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        let id = service
            .create("story-1", "神秘钥匙", Some("s1"), 8)
            .unwrap();
        let records = service.list_by_story("story-1").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, id);
        assert_eq!(records[0].content, "神秘钥匙");
        assert_eq!(records[0].importance, 8);
        assert!(matches!(records[0].status, ForeshadowingStatus::Setup));
    }

    #[test]
    fn service_unresolved_and_hints() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        service.create("story-1", "钥匙", Some("s1"), 9).unwrap();
        service.create("story-1", "信件", Some("s2"), 3).unwrap();

        let unresolved = service.get_unresolved("story-1").unwrap();
        assert_eq!(unresolved.len(), 2);

        let hints = service.get_writing_hints("story-1", 10).unwrap();
        assert_eq!(hints.len(), 2);
        assert!(hints[0].contains("关键"));
        assert!(hints[1].contains("次要"));
    }

    /// v0.62.0 P2-C 契约：注入话术按「计划回收窗口」分档——
    /// 临近窗口明确禁止提前回收；已过窗口要求尽快回收。
    #[test]
    fn service_hints_annotate_planned_payoff_window() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool.clone());
        let id = service
            .create("story-1", "玉佩的秘密", Some("s1"), 8)
            .unwrap();

        // 当前最新场 = 5；计划第 7 场回收 → 临近窗口
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = 7 WHERE id = ?1",
                rusqlite::params![&id],
            )
            .unwrap();
        }
        let near = service.get_writing_hints("story-1", 3).unwrap();
        assert!(
            near[0].contains("请勿提前回收"),
            "临近窗口应禁止提前回收: {near:?}"
        );

        // 计划第 4 场回收但已写到第 5 场 → 已过期
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = 4 WHERE id = ?1",
                rusqlite::params![&id],
            )
            .unwrap();
        }
        let overdue = service.get_writing_hints("story-1", 3).unwrap();
        assert!(
            overdue[0].contains("尽快回收"),
            "过期窗口应要求尽快回收: {overdue:?}"
        );

        // 无计划窗口时不加注（保持原格式）
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE foreshadowing_tracker SET target_end_scene = NULL WHERE id = ?1",
                rusqlite::params![&id],
            )
            .unwrap();
        }
        let plain = service.get_writing_hints("story-1", 3).unwrap();
        assert!(!plain[0].contains("回收）"), "无窗口不应加注: {plain:?}");
    }

    #[test]
    fn service_mark_payoff_and_abandon() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        let id = service.create("story-1", "钥匙", Some("s1"), 5).unwrap();
        service.mark_payoff(&id, Some("s3")).unwrap();

        let record = service.get_by_id(&id).unwrap().unwrap();
        assert!(matches!(record.status, ForeshadowingStatus::Payoff));
        assert_eq!(record.payoff_scene_id.as_deref(), Some("s3"));

        service.abandon(&id).unwrap();
        let record = service.get_by_id(&id).unwrap().unwrap();
        assert!(matches!(record.status, ForeshadowingStatus::Abandoned));
    }

    #[test]
    fn service_detect_overdue_by_scene() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        // High importance with first_seen at s1 and current at s7 => threshold
        // 5 => overdue.
        let high = service
            .create("story-1", "关键伏笔", Some("s1"), 9)
            .unwrap();
        // Low importance with first_seen at s1 and current at s7 => threshold
        // 15 => not overdue.
        let _low = service
            .create("story-1", "次要伏笔", Some("s1"), 3)
            .unwrap();
        // With target_end_scene = s4 and current = s7 => overdue.
        let target = service.create("story-1", "窗口伏笔", None, 5).unwrap();
        service
            .update_ledger_fields(&target, None, Some(4), None, None, None, None, None, None)
            .unwrap();

        let overdue = service.get_overdue("story-1", 7).unwrap();
        let overdue_ids: Vec<_> = overdue.iter().map(|r| r.id.clone()).collect();
        assert!(overdue_ids.contains(&high));
        assert!(overdue_ids.contains(&target));
        assert_eq!(overdue.len(), 2);
    }

    #[test]
    fn service_ledger_and_recommendations() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        let id = service
            .create("story-1", "神秘钥匙", Some("s1"), 8)
            .unwrap();
        service
            .update_ledger_fields(
                &id,
                Some(2),
                Some(4),
                None,
                Some(ScopeType::Story),
                Some("key-1".to_string()),
                None,
                None,
                None,
            )
            .unwrap();

        let ledger = service.get_ledger("story-1").unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].ledger_key, "key-1");
        assert_eq!(ledger[0].first_seen_scene, Some(1));

        let recs = service.recommend_payoffs("story-1", 3).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].foreshadowing_id, id);
    }

    #[test]
    fn service_importance_is_clamped() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        let id = service.create("story-1", "钥匙", None, 15).unwrap();
        let record = service.get_by_id(&id).unwrap().unwrap();
        assert_eq!(record.importance, 10);

        let id2 = service.create("story-1", "信件", None, 0).unwrap();
        let record2 = service.get_by_id(&id2).unwrap().unwrap();
        assert_eq!(record2.importance, 1);
    }

    #[test]
    fn service_reads_and_persists_unified_columns() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        let id = service
            .create("story-1", "神秘钥匙", Some("s1"), 8)
            .unwrap();
        service
            .update_ledger_fields(
                &id,
                None,
                None,
                None,
                None,
                None,
                Some("evt-setup-1".to_string()),
                Some("evt-payoff-1".to_string()),
                Some(0.75),
            )
            .unwrap();

        let record = service.get_by_id(&id).unwrap().unwrap();
        assert_eq!(record.setup_event_id.as_deref(), Some("evt-setup-1"));
        assert_eq!(record.payoff_event_id.as_deref(), Some("evt-payoff-1"));
        assert_eq!(record.risk_signals_score, Some(0.75));

        let listed = service.list_by_story("story-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].setup_event_id.as_deref(), Some("evt-setup-1"));

        let unresolved = service.get_unresolved("story-1").unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].risk_signals_score, Some(0.75));
    }

    #[test]
    fn service_ledger_title_multibyte_no_panic() {
        // 回归：伏笔 content 含多字节中文字符时，get_ledger 构造 title 预览不能
        // panic。 旧实现 &content[..30] 按**字节**切片，当 byte 30
        // 落在某个三字节中文字符内部 （如「指」bytes 29..32）时触发
        // "end byte index 30 is not a char boundary"
        // panic，直接炸垮续写 bundle 加载（文思活跃模式连续续写会读伏笔账本）。
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let service = ForeshadowingServiceImpl::new(pool);

        // 场景一：content 字节数 > 30 但字符数 < 30。
        // 「老王爷遗言'看草料'，指向草料车夹层中的血桉」共 22 字符、约 64
        // 字节， byte 30 恰在「指」（bytes 29..32）内部 -> 旧代码
        // panic，新代码不截断。
        let id_short = service
            .create(
                "story-1",
                "老王爷遗言'看草料'，指向草料车夹层中的血桉",
                Some("s1"),
                8,
            )
            .unwrap();
        let ledger = service.get_ledger("story-1").unwrap();
        let item_short = ledger.iter().find(|i| i.id == id_short).unwrap();
        // 字符数 < 30 -> title 为完整 content（不截断、不 panic）
        assert_eq!(
            item_short.title,
            "老王爷遗言'看草料'，指向草料车夹层中的血桉"
        );

        // 场景二：content 字符数 > 30（截断分支），验证按字符截取不 panic
        // 且补省略号。
        let long_content = "这是一段超过三十个字符的伏笔内容用于验证按字符数截取标题预览不会在中文字符中间切断导致panic";
        let id_long = service
            .create("story-1", long_content, Some("s2"), 5)
            .unwrap();
        let ledger = service.get_ledger("story-1").unwrap();
        let item_long = ledger.iter().find(|i| i.id == id_long).unwrap();
        assert!(item_long.title.ends_with("..."));
        // title 去掉省略号后应为前 30 个字符
        let prefix = &item_long.title[..item_long.title.len() - 3];
        assert_eq!(prefix.chars().count(), 30);
        assert_eq!(prefix, long_content.chars().take(30).collect::<String>());
    }
}
