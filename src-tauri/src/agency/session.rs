use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencySession {
    pub id: String,
    pub run_id: String,
    pub story_id: Option<String>,
    pub phase: String,
    pub snapshot_json: String,
    pub summary: Option<String>,
    pub kind: String, // auto | final
    pub created_at: String,
}

#[derive(Clone)]
pub struct SessionService {
    pool: DbPool,
}

impl SessionService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 机械提取快照（同步 fn；调用方负责 spawn_blocking/self.db）。
    /// 内容：黑板各分区 active 条目（key+summary+version）、最新 gate 判定、run 元数据。
    pub fn snapshot(&self, run_id: &str, phase: &str, kind: &str) -> Result<AgencySession, AppError> {
        let conn = self.pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
        let (story_id, premise): (Option<String>, String) = conn.query_row(
            "SELECT story_id, premise FROM agency_runs WHERE id = ?1",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(AppError::from)?;
        let mut stmt = conn.prepare(
            "SELECT zone, key, summary, version FROM agency_board_items
             WHERE run_id = ?1 AND status = 'active' ORDER BY zone, created_at, rowid")?;
        let mut board = serde_json::json!({"asset": [], "draft": [], "review": [], "schedule": []});
        let rows = stmt.query_map(params![run_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i32>(3)?))
        })?;
        for row in rows {
            let (zone, key, summary, version) = row.map_err(AppError::from)?;
            board[zone.as_str()].as_array_mut().unwrap().push(serde_json::json!({
                "key": key, "summary": summary, "version": version,
            }));
        }
        // 最新 gate 判定（审查区 item_type=gate 最新条）
        let verdict: Option<(String, String)> = conn.query_row(
            "SELECT content, summary FROM agency_board_items
             WHERE run_id = ?1 AND zone = 'review' AND item_type = 'gate'
             ORDER BY created_at DESC, rowid DESC LIMIT 1",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional().map_err(AppError::from)?;
        let snapshot_json = serde_json::json!({
            "premise": premise,
            "board": board,
            "latest_verdict": verdict.map(|(content, _)| {
                crate::agency::coordinator::parse_lenient::<serde_json::Value>(&content)
                    .unwrap_or_else(|| serde_json::json!({"raw": content.chars().take(200).collect::<String>()}))
            }),
        });
        let session = AgencySession {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.to_string(),
            story_id,
            phase: phase.to_string(),
            snapshot_json: snapshot_json.to_string(),
            summary: None,
            kind: kind.to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        };
        conn.execute(
            "INSERT INTO agency_sessions (id, run_id, story_id, phase, snapshot_json, summary, kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![session.id, session.run_id, session.story_id, session.phase,
                    session.snapshot_json, session.summary, session.kind, session.created_at],
        ).map_err(AppError::from)?;
        Ok(session)
    }

    /// 机械摘要文本（LLM 不可用时的兜底层，ECC 双层策略的底层）。
    pub fn mechanical_summary(&self, session: &AgencySession) -> String {
        let json: serde_json::Value = serde_json::from_str(&session.snapshot_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        let mut out = format!("阶段: {}\n", session.phase);
        for zone in ["asset", "draft", "review", "schedule"] {
            if let Some(items) = json["board"][zone].as_array() {
                if !items.is_empty() {
                    let keys: Vec<String> = items.iter()
                        .map(|i| format!("{}({})", i["key"].as_str().unwrap_or("?"), i["summary"].as_str().unwrap_or("")))
                        .collect();
                    out.push_str(&format!("{}: {}\n", zone, keys.join("、")));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::board::BlackboardService;
    use crate::agency::models::*;
    use crate::agency::repository::AgencyRepository;
    use crate::db::create_test_pool;

    fn seed(pool: &crate::db::DbPool, run_id: &str) {
        let repo = AgencyRepository::new(pool.clone());
        repo.create_run(&AgencyRun::new(run_id, "前提")).unwrap();
        repo.set_run_story(run_id, "s1").unwrap();
        let board = BlackboardService::new(pool.clone());
        board.write(run_id, "s1", AgentRole::Producer, BoardZone::Asset,
            "world", "世界观", "双星", "双星").unwrap();
        board.write(run_id, "s1", AgentRole::LeadWriter, BoardZone::Draft,
            "chapter", "第一章", "正文", "首章").unwrap();
        board.write(run_id, "s1", AgentRole::EditorAuditor, BoardZone::Review,
            "gate", "gate-第一章", r#"{"verdict":"pass","comments":"好"}"#, "gate:pass").unwrap();
    }

    #[test]
    fn test_snapshot_mechanical_extraction() {
        let pool = create_test_pool().unwrap();
        seed(&pool, "r1");
        let svc = SessionService::new(pool.clone());
        let session = svc.snapshot("r1", "writing", "auto").unwrap();
        assert_eq!(session.phase, "writing");
        assert_eq!(session.kind, "auto");
        let json: serde_json::Value = serde_json::from_str(&session.snapshot_json).unwrap();
        assert_eq!(json["board"]["asset"].as_array().unwrap().len(), 1);
        assert_eq!(json["board"]["draft"][0]["key"], "第一章");
        assert!(json["latest_verdict"]["comments"].as_str().is_some());
        // 已入库
        let repo = AgencyRepository::new(pool.clone());
        let loaded = repo.latest_session("r1").unwrap().unwrap();
        assert_eq!(loaded.id, session.id);
    }

    #[test]
    fn test_write_and_read_summary() {
        let pool = create_test_pool().unwrap();
        seed(&pool, "r1");
        let svc = SessionService::new(pool.clone());
        let session = svc.snapshot("r1", "assembly", "final").unwrap();
        let repo = AgencyRepository::new(pool.clone());
        repo.write_session_summary(&session.id, "五段摘要内容").unwrap();
        let loaded = repo.latest_session("r1").unwrap().unwrap();
        assert_eq!(loaded.summary.as_deref(), Some("五段摘要内容"));
    }

    #[test]
    fn test_mechanical_summary_text() {
        let pool = create_test_pool().unwrap();
        seed(&pool, "r1");
        let svc = SessionService::new(pool.clone());
        let session = svc.snapshot("r1", "writing", "auto").unwrap();
        let text = svc.mechanical_summary(&session);
        assert!(text.contains("世界观"));
        assert!(text.contains("第一章"));
    }

    #[test]
    fn test_latest_session_for_story() {
        let pool = create_test_pool().unwrap();
        seed(&pool, "r1");
        let svc = SessionService::new(pool.clone());
        svc.snapshot("r1", "writing", "auto").unwrap();
        let repo = AgencyRepository::new(pool.clone());
        let loaded = repo.latest_session_for_story("s1").unwrap().unwrap();
        assert_eq!(loaded.run_id, "r1");
        assert!(repo.latest_session_for_story("s2").unwrap().is_none());
    }
}
