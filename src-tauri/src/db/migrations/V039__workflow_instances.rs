use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        39
    }

    fn description(&self) -> &'static str {
        "workflow instances"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let workflow_instance_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='workflow_instances'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if workflow_instance_tables.is_empty() {
            conn.execute(
                "CREATE TABLE workflow_instances (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                story_id TEXT NOT NULL,
                status TEXT NOT NULL,
                instance_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_workflow ON workflow_instances(workflow_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_story ON workflow_instances(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_status ON workflow_instances(status)",
                [],
            )?;
        }
        Ok(())
    }
}
