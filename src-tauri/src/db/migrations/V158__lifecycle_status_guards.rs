use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        158
    }

    fn description(&self) -> &'static str {
        "lifecycle status guards: DB triggers reject illegal status values"
    }

    /// v0.96.0：把 `lifecycle` 模块的合法状态词汇下沉为 SQLite 触发器。
    /// 应用层已用 UPDATE ... WHERE status IN (...) 做终态守卫；触发器是最后
    /// 一道防线：任何 raw UPDATE 写入未声明状态都会 ABORT，而不是静默落库
    /// 让 UI 读到“未知状态”。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // 先清理历史未知状态，避免触发器存在后旧行永远无法被修复。
        conn.execute(
            "UPDATE tasks SET status = 'failed' \
             WHERE status NOT IN ('pending','running','completed','failed','cancelled')",
            [],
        )?;
        conn.execute(
            "UPDATE agency_runs SET status = 'failed' \
             WHERE status NOT IN ('pending','running','completed','failed','cancelled','idle','observing')",
            [],
        )?;
        conn.execute(
            "UPDATE agency_agent_tasks SET status = 'failed' \
             WHERE status NOT IN ('queued','running','waiting','cancelling','completed','failed','cancelled','stale')",
            [],
        )?;
        conn.execute(
            "UPDATE agency_chat_messages SET status = 'failed' \
             WHERE status NOT IN ('pending','thinking','acting','waiting','done','failed','cancelled','degraded')",
            [],
        )?;
        conn.execute(
            "UPDATE agency_director_actions SET status = 'failed' \
             WHERE status NOT IN ('proposed','approved','executing','done','failed','rejected')",
            [],
        )?;
        conn.execute(
            "UPDATE editor_gate_records SET outcome = 'failed' \
             WHERE outcome NOT IN ('pass','revise','failed')",
            [],
        )?;

        // 触发器语法：INSERT 与 UPDATE OF <col> 各一条。SQLite 没有
        // ADD CONSTRAINT，触发器是唯一不改表结构就能加 CHECK 的方式。
        for (table, column, allowed) in [
            ("tasks", "status", "('pending','running','completed','failed','cancelled')"),
            (
                "agency_runs",
                "status",
                "('pending','running','completed','failed','cancelled','idle','observing')",
            ),
            (
                "agency_agent_tasks",
                "status",
                "('queued','running','waiting','cancelling','completed','failed','cancelled','stale')",
            ),
            (
                "agency_chat_messages",
                "status",
                "('pending','thinking','acting','waiting','done','failed','cancelled','degraded')",
            ),
            (
                "agency_director_actions",
                "status",
                "('proposed','approved','executing','done','failed','rejected')",
            ),
            ("editor_gate_records", "outcome", "('pass','revise','failed')"),
        ] {
            conn.execute(
                &format!(
                    "CREATE TRIGGER IF NOT EXISTS trg_{table}_{column}_insert \
                     BEFORE INSERT ON {table} \
                     FOR EACH ROW WHEN NEW.{column} NOT IN {allowed} \
                     BEGIN SELECT RAISE(ABORT, 'invalid {table}.{column}'); END"
                ),
                [],
            )?;
            conn.execute(
                &format!(
                    "CREATE TRIGGER IF NOT EXISTS trg_{table}_{column}_update \
                     BEFORE UPDATE OF {column} ON {table} \
                     FOR EACH ROW WHEN NEW.{column} NOT IN {allowed} \
                     BEGIN SELECT RAISE(ABORT, 'invalid {table}.{column}'); END"
                ),
                [],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_minimal_schema(conn: &mut Connection) {
        conn.execute_batch(
            "CREATE TABLE tasks (id TEXT PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE agency_runs (id TEXT PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE agency_agent_tasks (id TEXT PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE agency_chat_messages (id TEXT PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE agency_director_actions (id TEXT PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE editor_gate_records (id TEXT PRIMARY KEY, outcome TEXT NOT NULL);",
        )
        .unwrap();
    }

    #[test]
    fn rejects_unknown_task_status_on_insert_and_update() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_minimal_schema(&mut conn);
        Migration.apply(&mut conn).unwrap();

        let insert = conn.execute("INSERT INTO tasks (id, status) VALUES ('t1', 'bogus')", []);
        assert!(insert.is_err(), "未知任务状态必须在 INSERT 被拒绝");

        conn.execute(
            "INSERT INTO tasks (id, status) VALUES ('t2', 'pending')",
            [],
        )
        .unwrap();
        let update = conn.execute("UPDATE tasks SET status = 'bogus' WHERE id = 't2'", []);
        assert!(update.is_err(), "未知任务状态必须在 UPDATE 被拒绝");
        let status: String = conn
            .query_row("SELECT status FROM tasks WHERE id = 't2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn cleans_history_and_accepts_declared_states() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_minimal_schema(&mut conn);
        conn.execute(
            "INSERT INTO agency_runs (id, status) VALUES ('r1', 'mystery')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agency_chat_messages (id, status) VALUES ('m1', 'thinking')",
            [],
        )
        .unwrap();
        Migration.apply(&mut conn).unwrap();

        let run: String = conn
            .query_row("SELECT status FROM agency_runs WHERE id = 'r1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(run, "failed");
        conn.execute(
            "UPDATE agency_chat_messages SET status = 'done' WHERE id = 'm1'",
            [],
        )
        .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT status FROM agency_chat_messages WHERE id = 'm1'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "done"
        );
    }

    #[test]
    fn valid_status_insert_path_uses_params() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_minimal_schema(&mut conn);
        Migration.apply(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO agency_agent_tasks (id, status) VALUES (?1, ?2)",
            params!["a1", "waiting"],
        )
        .unwrap();
    }
}
