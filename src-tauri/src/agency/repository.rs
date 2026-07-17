use rusqlite::{params, OptionalExtension};

use crate::agency::models::*;
use crate::db::DbPool;

pub struct AgencyRepository {
    pool: DbPool,
}

impl Clone for AgencyRepository {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

fn pool_err(e: r2d2::Error) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(e.to_string())
}

impl AgencyRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ---- runs ----

    pub fn create_run(&self, run: &AgencyRun) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_runs (id, story_id, premise, status, phase, result_json, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![run.id, run.story_id, run.premise, run.status, run.phase,
                    run.result_json, run.error_message, run.created_at, run.updated_at],
        )?;
        Ok(())
    }

    pub fn set_run_story(&self, run_id: &str, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET story_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![run_id, story_id, now()],
        )?;
        Ok(())
    }

    pub fn update_run_phase(&self, run_id: &str, status: &str, phase: &str) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET status = ?2, phase = ?3, updated_at = ?4 WHERE id = ?1",
            params![run_id, status, phase, now()],
        )?;
        Ok(())
    }

    /// 终态守护：cancelled/completed/failed 后不再允许覆盖（取消竞态防护）。
    pub fn finish_run(
        &self,
        run_id: &str,
        status: &str,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_runs SET status = ?2, result_json = ?3, error_message = ?4, updated_at = ?5
             WHERE id = ?1 AND status NOT IN ('cancelled', 'completed', 'failed')",
            params![run_id, status, result_json, error_message, now()],
        )?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<AgencyRun>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.query_row(
            "SELECT id, story_id, premise, status, phase, result_json, error_message, created_at, updated_at
             FROM agency_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok(AgencyRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    premise: row.get(2)?,
                    status: row.get(3)?,
                    phase: row.get(4)?,
                    result_json: row.get(5)?,
                    error_message: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        ).optional()
    }

    // ---- board items ----

    pub fn insert_item(&self, item: &BoardItem) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_board_items
             (id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![item.id, item.run_id, item.story_id, item.zone.as_str(), item.item_type,
                    item.key, item.content, item.summary, item.version, item.producer.as_str(),
                    item.status, item.created_at, item.updated_at],
        )?;
        Ok(())
    }

    /// 版本乐观锁修订。返回 None 表示版本冲突。
    pub fn revise_item(
        &self,
        item_id: &str,
        new_content: &str,
        new_summary: &str,
        expected_version: i32,
    ) -> Result<Option<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let changed = conn.execute(
            "UPDATE agency_board_items
             SET content = ?2, summary = ?3, version = version + 1, updated_at = ?4
             WHERE id = ?1 AND version = ?5",
            params![item_id, new_content, new_summary, now(), expected_version],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        drop(conn);
        self.get_item(item_id)
    }

    pub fn get_item(&self, item_id: &str) -> Result<Option<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.query_row(
            "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
             FROM agency_board_items WHERE id = ?1",
            params![item_id],
            map_board_item,
        ).optional()
    }

    pub fn list_items(&self, run_id: &str, zone: Option<BoardZone>) -> Result<Vec<BoardItem>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let items = match zone {
            Some(z) => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                     FROM agency_board_items WHERE run_id = ?1 AND zone = ?2 ORDER BY created_at ASC, rowid ASC",
                )?;
                let rows = stmt.query_map(params![run_id, z.as_str()], map_board_item)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, story_id, zone, item_type, key, content, summary, version, producer, status, created_at, updated_at
                     FROM agency_board_items WHERE run_id = ?1 ORDER BY created_at ASC, rowid ASC",
                )?;
                let rows = stmt.query_map(params![run_id], map_board_item)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(items)
    }

    pub fn promote_item(&self, item_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_board_items SET status = 'active', updated_at = ?2 WHERE id = ?1",
            params![item_id, now()],
        )
    }

    // ---- messages ----

    pub fn insert_message(&self, msg: &AgencyMessage) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_messages (id, run_id, from_role, to_role, msg_type, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg.id, msg.run_id, msg.from_role.as_str(), msg.to_role.as_str(),
                    msg.msg_type, msg.payload, msg.created_at],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, run_id: &str, to_role: Option<AgentRole>) -> Result<Vec<AgencyMessage>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let msgs = match to_role {
            Some(role) => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, from_role, to_role, msg_type, payload, created_at
                     FROM agency_messages WHERE run_id = ?1 AND to_role = ?2 ORDER BY created_at ASC, rowid ASC",
                )?;
                let rows = stmt.query_map(params![run_id, role.as_str()], map_message)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, run_id, from_role, to_role, msg_type, payload, created_at
                     FROM agency_messages WHERE run_id = ?1 ORDER BY created_at ASC, rowid ASC",
                )?;
                let rows = stmt.query_map(params![run_id], map_message)?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };
        Ok(msgs)
    }
}

fn map_board_item(row: &rusqlite::Row) -> Result<BoardItem, rusqlite::Error> {
    let zone_str: String = row.get(3)?;
    let producer_str: String = row.get(9)?;
    Ok(BoardItem {
        id: row.get(0)?,
        run_id: row.get(1)?,
        story_id: row.get(2)?,
        zone: BoardZone::from_str(&zone_str).unwrap_or(BoardZone::Asset),
        item_type: row.get(4)?,
        key: row.get(5)?,
        content: row.get(6)?,
        summary: row.get(7)?,
        version: row.get(8)?,
        producer: AgentRole::from_str(&producer_str).unwrap_or(AgentRole::Producer),
        status: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_message(row: &rusqlite::Row) -> Result<AgencyMessage, rusqlite::Error> {
    let from_str: String = row.get(2)?;
    let to_str: String = row.get(3)?;
    Ok(AgencyMessage {
        id: row.get(0)?,
        run_id: row.get(1)?,
        from_role: AgentRole::from_str(&from_str).unwrap_or(AgentRole::Producer),
        to_role: AgentRole::from_str(&to_str).unwrap_or(AgentRole::LeadWriter),
        msg_type: row.get(4)?,
        payload: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn repo() -> (AgencyRepository, DbPool) {
        let pool = create_test_pool().unwrap();
        (AgencyRepository::new(pool.clone()), pool)
    }

    fn sample_run() -> AgencyRun {
        AgencyRun::new("run-1", "一个关于星海拾荒者的故事")
    }

    #[test]
    fn test_create_and_get_run() {
        let (repo, _) = repo();
        let run = sample_run();
        repo.create_run(&run).unwrap();
        let loaded = repo.get_run("run-1").unwrap().expect("run should exist");
        assert_eq!(loaded.premise, "一个关于星海拾荒者的故事");
        assert_eq!(loaded.status, "pending");
        assert_eq!(loaded.phase, "concept");
    }

    #[test]
    fn test_run_phase_and_finish() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        repo.update_run_phase("run-1", "running", "assets").unwrap();
        let r = repo.get_run("run-1").unwrap().unwrap();
        assert_eq!(r.status, "running");
        assert_eq!(r.phase, "assets");
        repo.finish_run("run-1", "completed", Some("{\"ok\":true}"), None).unwrap();
        let r = repo.get_run("run-1").unwrap().unwrap();
        assert_eq!(r.status, "completed");
        assert_eq!(r.result_json.as_deref(), Some("{\"ok\":true}"));
    }

    #[test]
    fn test_insert_and_list_board_items() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Asset, "world", "世界观",
            "内容：双星系统", "双星系统，废土文明", AgentRole::Producer, "active",
        );
        repo.insert_item(&item).unwrap();
        let items = repo.list_items("run-1", Some(BoardZone::Asset)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].version, 1);
        assert_eq!(items[0].producer, AgentRole::Producer);
        let all = repo.list_items("run-1", None).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_revise_item_optimistic_lock() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Draft, "chapter", "第一章",
            "旧稿", "旧摘要", AgentRole::LeadWriter, "active",
        );
        repo.insert_item(&item).unwrap();
        // 版本匹配 → 成功
        let revised = repo.revise_item(&item.id, "新稿", "新摘要", 1).unwrap();
        assert!(revised.is_some());
        let revised = revised.unwrap();
        assert_eq!(revised.version, 2);
        assert_eq!(revised.content, "新稿");
        // 版本不匹配 → None（冲突）
        let conflict = repo.revise_item(&item.id, "并发写", "x", 1).unwrap();
        assert!(conflict.is_none());
    }

    #[test]
    fn test_promote_item() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let item = BoardItem::new(
            "run-1", "story-1", BoardZone::Draft, "chapter", "第一章",
            "提案稿", "提案", AgentRole::Producer, "proposed",
        );
        repo.insert_item(&item).unwrap();
        repo.promote_item(&item.id).unwrap();
        let loaded = repo.get_item(&item.id).unwrap().unwrap();
        assert_eq!(loaded.status, "active");
    }

    #[test]
    fn test_messages() {
        let (repo, _) = repo();
        repo.create_run(&sample_run()).unwrap();
        let msg = AgencyMessage::new(
            "run-1", AgentRole::EditorAuditor, AgentRole::LeadWriter,
            "proposal", serde_json::json!({"text":"建议加强冲突"}),
        );
        repo.insert_message(&msg).unwrap();
        let inbox = repo.list_messages("run-1", Some(AgentRole::LeadWriter)).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].msg_type, "proposal");
        assert!(inbox[0].payload.contains("建议加强冲突"));
        let all = repo.list_messages("run-1", None).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_role_zone_ownership() {
        assert_eq!(BoardZone::Asset.owner(), AgentRole::Producer);
        assert_eq!(BoardZone::Draft.owner(), AgentRole::LeadWriter);
        assert_eq!(BoardZone::Review.owner(), AgentRole::EditorAuditor);
        assert_eq!(BoardZone::Schedule.owner(), AgentRole::Producer);
        assert_eq!(AgentRole::from_str("lead_writer"), Some(AgentRole::LeadWriter));
        assert_eq!(BoardZone::from_str("review"), Some(BoardZone::Review));
        assert_eq!(AgentRole::from_str("nope"), None);
    }
}
