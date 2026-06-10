#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json;
use uuid::Uuid;

use super::{
    AnchorType, CommentMessage, CommentThread, CommentThreadWithMessages, DbPool, ThreadStatus,
};

// ==================== CommentThread Repository (评论线程) ====================

pub struct CommentThreadRepository {
    pool: DbPool,
}

impl CommentThreadRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_thread(&self, thread: &CommentThread) -> Result<CommentThread, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_threads (id, scene_id, chapter_id, version_id, anchor_type, \
             from_pos, to_pos, selected_text, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &thread.id,
                &thread.scene_id,
                &thread.chapter_id,
                &thread.version_id,
                format!("{:?}", thread.anchor_type),
                thread.from_pos,
                thread.to_pos,
                &thread.selected_text,
                format!("{:?}", thread.status),
                thread.created_at.to_rfc3339(),
                thread.resolved_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(thread.clone())
    }

    pub fn add_message(&self, message: &CommentMessage) -> Result<CommentMessage, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_messages (id, thread_id, author_id, author_name, content, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &message.id,
                &message.thread_id,
                &message.author_id,
                &message.author_name,
                &message.content,
                message.created_at.to_rfc3339(),
            ],
        )?;
        Ok(message.clone())
    }

    fn parse_thread(&self, row: &rusqlite::Row) -> Result<CommentThread, rusqlite::Error> {
        let created_str: String = row.get(9)?;
        let resolved_str: Option<String> = row.get(10)?;
        Ok(CommentThread {
            id: row.get(0)?,
            scene_id: row.get(1)?,
            chapter_id: row.get(2)?,
            version_id: row.get(3)?,
            anchor_type: match row.get::<_, String>(4)?.as_str() {
                "SceneLevel" => AnchorType::SceneLevel,
                _ => AnchorType::TextRange,
            },
            from_pos: row.get(5)?,
            to_pos: row.get(6)?,
            selected_text: row.get(7)?,
            status: match row.get::<_, String>(8)?.as_str() {
                "Resolved" => ThreadStatus::Resolved,
                _ => ThreadStatus::Open,
            },
            created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            resolved_at: resolved_str.and_then(|s| s.parse().ok()),
        })
    }

    fn parse_message(&self, row: &rusqlite::Row) -> Result<CommentMessage, rusqlite::Error> {
        let created_str: String = row.get(5)?;
        Ok(CommentMessage {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            author_id: row.get(2)?,
            author_name: row.get(3)?,
            content: row.get(4)?,
            created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
        })
    }

    pub fn get_threads_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, \
             selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE chapter_id = ?1 ORDER BY created_at DESC",
        )?;

        let threads: Vec<CommentThread> = stmt
            .query_map([chapter_id], |row| self.parse_thread(row))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_threads_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, \
             selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE scene_id = ?1 ORDER BY created_at DESC",
        )?;

        let threads: Vec<CommentThread> = stmt
            .query_map([scene_id], |row| self.parse_thread(row))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_messages(&self, thread_id: &str) -> Result<Vec<CommentMessage>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, author_id, author_name, content, created_at
             FROM comment_messages WHERE thread_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([thread_id], |row| self.parse_message(row))?;
        rows.collect()
    }

    pub fn resolve_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE comment_threads SET status = 'Resolved', resolved_at = ?2 WHERE id = ?1",
            params![thread_id, now],
        )
    }

    pub fn reopen_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE comment_threads SET status = 'Open', resolved_at = NULL WHERE id = ?1",
            params![thread_id],
        )
    }

    pub fn delete_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM comment_threads WHERE id = ?1",
            params![thread_id],
        )
    }
}
