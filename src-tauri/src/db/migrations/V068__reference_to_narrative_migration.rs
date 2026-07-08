use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        68
    }

    fn description(&self) -> &'static str {
        "reference to narrative migration"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let has_reference_characters: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND \
             name='reference_characters'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_reference_characters {
            conn.execute(
                "INSERT OR IGNORE INTO narrative_characters (
                id, story_id, name, role_type, personality, background, goals, appearance,
                gender, age, importance_score, source, source_ref_id, status, created_at, \
             updated_at
            )
            SELECT
                rc.id, rc.book_id, rc.name, rc.role_type, rc.personality, '', '', \
             rc.appearance,
                '', 0, COALESCE(rc.importance_score, 0.0), 'extracted', rc.book_id, \
             'reference', rc.created_at, rc.created_at
            FROM reference_characters rc
            LEFT JOIN narrative_characters nc ON nc.id = rc.id
            WHERE nc.id IS NULL
                AND EXISTS (SELECT 1 FROM stories s WHERE s.id = rc.book_id)",
                [],
            )?;
        }

        let has_reference_scenes: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='reference_scenes'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_reference_scenes {
            conn.execute(
                "INSERT OR IGNORE INTO narrative_scenes (
                id, story_id, sequence_number, title, summary, dramatic_goal, \
             external_pressure,
                conflict_type, characters_present, setting_location, setting_time, content,
                source, source_ref_id, status, created_at, updated_at
            )
            SELECT
                rs.id, rs.book_id, rs.sequence_number, rs.title, rs.summary, '', '',
                rs.conflict_type, rs.characters_present, '', '', NULL,
                'extracted', rs.book_id, 'reference', rs.created_at, rs.created_at
            FROM reference_scenes rs
            LEFT JOIN narrative_scenes ns ON ns.id = rs.id
            WHERE ns.id IS NULL
                AND EXISTS (SELECT 1 FROM stories s WHERE s.id = rs.book_id)",
                [],
            )?;
        }
        Ok(())
    }
}
