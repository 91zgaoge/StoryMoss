use rusqlite::Connection;

use crate::db::migrations::RustMigration;

/// V149：语料资产、导演剧组对话、lane/task/lease 并发底座。
///
/// 关键约束：V109 的「同一 story 只有一个 pending/running run」升级为
/// `(story_id, lane)` 部分唯一索引。`write` lane 仍单活跃，`review` /
/// `inspect` / `chat` 可以与之并行。索引替换必须和 lane 回填在同一个事务里，
/// 否则会留下「旧索引已删、新索引未建」的窗口。
pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        149
    }

    fn description(&self) -> &'static str {
        "corpus, director chat, lane/task/lease"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;

        // 1) agency_runs.lane（幂等）：存量行默认 write。
        if !has_column(&tx, "agency_runs", "lane")? {
            tx.execute(
                "ALTER TABLE agency_runs ADD COLUMN lane TEXT NOT NULL DEFAULT 'write'",
                [],
            )?;
        }

        // 2) 原子替换 V109 索引：旧索引只允许每 story 一个活跃 run，
        // 新索引允许每 story 每 lane 一个活跃 run。
        tx.execute(
            "DROP INDEX IF EXISTS idx_agency_runs_one_active_per_story",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_agency_runs_one_active_per_lane
             ON agency_runs(story_id, lane) WHERE status IN ('pending', 'running')",
            [],
        )?;

        // 3) 剧组任务注册表：子 agent 进程/心跳/父子任务。
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS agency_agent_tasks (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                story_id TEXT NOT NULL,
                role TEXT NOT NULL,
                parent_task_id TEXT,
                task TEXT NOT NULL,
                status TEXT NOT NULL,
                phase TEXT,
                turn_count INTEGER NOT NULL DEFAULT 0,
                tool_calls INTEGER NOT NULL DEFAULT 0,
                last_tool TEXT,
                heartbeat_at TEXT,
                started_at TEXT,
                finished_at TEXT,
                error TEXT,
                result_summary TEXT,
                meta_json TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_agency_tasks_story_status
                ON agency_agent_tasks(story_id, status, heartbeat_at);
            CREATE INDEX IF NOT EXISTS idx_agency_tasks_run
                ON agency_agent_tasks(run_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_agency_tasks_parent
                ON agency_agent_tasks(parent_task_id);

            CREATE TABLE IF NOT EXISTS story_resource_leases (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                resource TEXT NOT NULL,
                owner_run_id TEXT,
                owner_task_id TEXT,
                acquired_at TEXT NOT NULL,
                heartbeat_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                UNIQUE(story_id, resource),
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_story_leases_expiry
                ON story_resource_leases(expires_at);

            CREATE TABLE IF NOT EXISTS agency_chat_sessions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                title TEXT NOT NULL,
                target_role TEXT NOT NULL DEFAULT 'director',
                summary TEXT,
                summary_upto_message_id TEXT,
                archived INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_agency_chat_sessions_story
                ON agency_chat_sessions(story_id, updated_at);

            CREATE TABLE IF NOT EXISTS agency_chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                run_key TEXT,
                task_id TEXT,
                task_ids_json TEXT,
                actions_json TEXT,
                meta_json TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES agency_chat_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_agency_chat_messages_session
                ON agency_chat_messages(session_id, created_at);

            CREATE TABLE IF NOT EXISTS agency_director_actions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                session_id TEXT,
                message_id TEXT,
                run_id TEXT,
                action_type TEXT NOT NULL,
                target_role TEXT,
                target_id TEXT,
                params_json TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL,
                reason TEXT,
                result_summary TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_director_actions_story
                ON agency_director_actions(story_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_director_actions_status
                ON agency_director_actions(status, created_at);

            CREATE TABLE IF NOT EXISTS corpus_items (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                source_url TEXT,
                source_name TEXT,
                tags TEXT NOT NULL DEFAULT '[]',
                file_path TEXT,
                file_name TEXT,
                mime_type TEXT,
                file_size INTEGER,
                file_hash TEXT,
                status TEXT NOT NULL DEFAULT 'raw',
                pinned INTEGER NOT NULL DEFAULT 0,
                distilled_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_corpus_story_kind
                ON corpus_items(story_id, kind, created_at);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_corpus_story_hash
                ON corpus_items(story_id, file_hash) WHERE file_hash IS NOT NULL;

            CREATE TABLE IF NOT EXISTS corpus_links (
                id TEXT PRIMARY KEY,
                corpus_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                note TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (corpus_id) REFERENCES corpus_items(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_corpus_links_corpus
                ON corpus_links(corpus_id);
            CREATE INDEX IF NOT EXISTS idx_corpus_links_target
                ON corpus_links(target_type, target_id);",
        )?;

        tx.commit()?;
        Ok(())
    }
}

fn has_column(
    tx: &rusqlite::Transaction,
    table: &str,
    column: &str,
) -> Result<bool, rusqlite::Error> {
    let mut stmt = tx.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    fn seeded_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE stories (id TEXT PRIMARY KEY, title TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE agency_runs (
                id TEXT PRIMARY KEY,
                story_id TEXT,
                premise TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                phase TEXT NOT NULL DEFAULT 'concept',
                result_json TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE INDEX idx_agency_runs_one_active_per_story
             ON agency_runs(story_id) WHERE status IN ('pending', 'running')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_adds_lane_and_replaces_index_atomically() {
        let mut conn = seeded_conn();
        conn.execute(
            "INSERT INTO agency_runs (id, story_id, premise, status, created_at, updated_at)
             VALUES ('u1', 's1', '前提', 'running', 'now', 'now')",
            [],
        )
        .unwrap();

        Migration.apply(&mut conn).unwrap();

        let lane: String = conn
            .query_row("SELECT lane FROM agency_runs WHERE id = 'u1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(lane, "write");
        let old_index: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_agency_runs_one_active_per_story'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old_index, 0);
        let new_index: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_agency_runs_one_active_per_lane'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_index, 1);
    }

    #[test]
    fn test_idempotent() {
        let mut conn = seeded_conn();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
    }
}
