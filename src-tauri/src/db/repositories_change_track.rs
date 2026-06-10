#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json;
use uuid::Uuid;

use super::{ChangeStatus, ChangeTrack, ChangeType, DbPool};

pub struct ChangeTrackRepository {
    pool: DbPool,
}

impl ChangeTrackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, track: &ChangeTrack) -> Result<ChangeTrack, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO change_tracks (id, scene_id, chapter_id, version_id, author_id, \
             author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &track.id,
                &track.scene_id,
                &track.chapter_id,
                &track.version_id,
                &track.author_id,
                &track.author_name,
                format!("{:?}", track.change_type),
                track.from_pos,
                track.to_pos,
                &track.content,
                format!("{:?}", track.status),
                track.created_at.to_rfc3339(),
                track.resolved_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(track.clone())
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE id = ?1",
        )?;

        let result = stmt.query_row([id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        });

        match result {
            Ok(track) => Ok(Some(track)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_by_scene(&self, scene_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE scene_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([scene_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_pending_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE scene_id = ?1 AND status = 'Pending' ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([scene_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_pending_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE chapter_id = ?1 AND status = 'Pending' ORDER BY created_at \
             DESC",
        )?;

        let rows = stmt.query_map([chapter_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_by_version(&self, version_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE version_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([version_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn update_status(&self, id: &str, status: ChangeStatus) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let resolved = match status {
            ChangeStatus::Pending => None,
            _ => Some(Local::now().to_rfc3339()),
        };
        conn.execute(
            "UPDATE change_tracks SET status = ?2, resolved_at = ?3 WHERE id = ?1",
            params![id, format!("{:?}", status), resolved],
        )
    }

    pub fn accept_all_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE scene_id = ?1 \
             AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn reject_all_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE scene_id = ?1 \
             AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn accept_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE chapter_id = ?1 \
             AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn reject_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE chapter_id = ?1 \
             AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM change_tracks WHERE scene_id = ?1",
            params![scene_id],
        )
    }
}
