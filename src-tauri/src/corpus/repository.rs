use rusqlite::{params, OptionalExtension};

use crate::{
    corpus::models::{CorpusDetail, CorpusDistillation, CorpusLink, CorpusSummary, CorpusUpdate},
    db::DbPool,
    error::AppError,
};

pub struct CorpusRepository {
    pool: DbPool,
}

impl CorpusRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn list(
        &self,
        story_id: &str,
        kind: Option<&str>,
        status: Option<&str>,
        query: Option<&str>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<CorpusSummary>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let like = query.map(|q| format!("%{}%", q));
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, kind, title, summary, source_url, source_name,
                        tags, status, pinned, created_at, updated_at
                 FROM corpus_items
                 WHERE story_id = ?1
                   AND (?2 IS NULL OR kind = ?2)
                   AND (?3 IS NULL OR status = ?3)
                   AND (?4 IS NULL OR title LIKE ?4 OR summary LIKE ?4
                        OR content LIKE ?4 OR tags LIKE ?4)
                 ORDER BY created_at DESC
                 LIMIT ?5 OFFSET ?6",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(
                params![story_id, kind, status, like, limit.max(0), offset.max(0)],
                map_summary,
            )
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn get(&self, id: &str) -> Result<Option<CorpusDetail>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let detail = conn
            .query_row(
                "SELECT id, story_id, kind, title, summary, content, source_url, source_name,
                        tags, status, pinned, file_name, mime_type, file_size,
                        distilled_json, created_at, updated_at
                 FROM corpus_items WHERE id = ?1",
                params![id],
                map_detail,
            )
            .optional()
            .map_err(AppError::from)?;
        let Some(mut detail) = detail else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare(
                "SELECT id, corpus_id, target_type, target_id, note, created_at
                 FROM corpus_links WHERE corpus_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(AppError::from)?;
        let links = stmt
            .query_map(params![id], |row| {
                Ok(CorpusLink {
                    id: row.get(0)?,
                    corpus_id: row.get(1)?,
                    target_type: row.get(2)?,
                    target_id: row.get(3)?,
                    note: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(AppError::from)?;
        detail.links = links
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(Some(detail))
    }

    pub fn create(
        &self,
        story_id: &str,
        kind: &str,
        title: &str,
        content: &str,
        summary: &str,
        source_url: Option<&str>,
        source_name: Option<&str>,
        tags: Vec<String>,
    ) -> Result<String, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO corpus_items
             (id, story_id, kind, title, summary, content, source_url, source_name, tags,
              status, pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'raw', 0, ?10, ?11)",
            params![
                id,
                story_id,
                kind,
                title,
                summary,
                content,
                source_url,
                source_name,
                serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string()),
                now,
                now,
            ],
        )
        .map_err(AppError::from)?;
        Ok(id)
    }

    pub fn create_file_item(
        &self,
        story_id: &str,
        kind: &str,
        title: &str,
        content: &str,
        summary: &str,
        file_path: &str,
        file_name: &str,
        mime_type: &str,
        file_size: i64,
        file_hash: &str,
        tags: Vec<String>,
    ) -> Result<String, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO corpus_items
             (id, story_id, kind, title, summary, content, tags, file_path, file_name,
              mime_type, file_size, file_hash, status, pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'raw', 0, ?13, ?14)",
            params![
                id,
                story_id,
                kind,
                title,
                summary,
                content,
                serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string()),
                file_path,
                file_name,
                mime_type,
                file_size,
                file_hash,
                now,
                now,
            ],
        )
        .map_err(AppError::from)?;
        Ok(id)
    }

    pub fn update(&self, id: &str, update: &CorpusUpdate) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let tags = update
            .tags
            .as_ref()
            .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".to_string()));
        let pinned = update.pinned.map(|p| if p { 1i64 } else { 0i64 });
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE corpus_items SET
                kind = COALESCE(?2, kind),
                title = COALESCE(?3, title),
                summary = COALESCE(?4, summary),
                content = COALESCE(?5, content),
                source_url = COALESCE(?6, source_url),
                source_name = COALESCE(?7, source_name),
                tags = COALESCE(?8, tags),
                status = COALESCE(?9, status),
                pinned = COALESCE(?10, pinned),
                updated_at = ?11
             WHERE id = ?1",
            params![
                id,
                update.kind,
                update.title,
                update.summary,
                update.content,
                update.source_url,
                update.source_name,
                tags,
                update.status,
                pinned,
                now,
            ],
        )
        .map_err(AppError::from)
    }

    pub fn delete(&self, id: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute("DELETE FROM corpus_items WHERE id = ?1", params![id])
            .map_err(AppError::from)
    }

    pub fn set_status(&self, id: &str, status: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "UPDATE corpus_items SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(AppError::from)
    }

    pub fn set_distilled(
        &self,
        id: &str,
        distilled_json: &str,
        model: &str,
    ) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE corpus_items SET distilled_json = ?2, status = 'distilled',
                                    updated_at = ?3
             WHERE id = ?1",
            params![id, distilled_json, now],
        )
        .map_err(AppError::from)?;
        let _ = model;
        Ok(1)
    }

    pub fn insert_link(
        &self,
        corpus_id: &str,
        target_type: &str,
        target_id: &str,
        note: Option<&str>,
    ) -> Result<String, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO corpus_links (id, corpus_id, target_type, target_id, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                corpus_id,
                target_type,
                target_id,
                note,
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .map_err(AppError::from)?;
        Ok(id)
    }

    pub fn delete_link(&self, link_id: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute("DELETE FROM corpus_links WHERE id = ?1", params![link_id])
            .map_err(AppError::from)
    }

    pub fn find_by_hash(
        &self,
        story_id: &str,
        file_hash: &str,
    ) -> Result<Option<String>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let id: Option<String> = conn
            .query_row(
                "SELECT id FROM corpus_items WHERE story_id = ?1 AND file_hash = ?2 LIMIT 1",
                params![story_id, file_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)?;
        Ok(id)
    }

    pub fn get_file_path(&self, id: &str) -> Result<Option<String>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.query_row(
            "SELECT file_path FROM corpus_items WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)
        .map(|v| v.flatten())
    }

    pub fn list_file_paths_for_story(&self, story_id: &str) -> Result<Vec<String>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT file_path FROM corpus_items
                 WHERE story_id = ?1 AND file_path IS NOT NULL",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![story_id], |row| row.get::<_, String>(0))
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }
}

fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_else(|_| {
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn map_summary(row: &rusqlite::Row<'_>) -> Result<CorpusSummary, rusqlite::Error> {
    let tags: String = row.get(7)?;
    Ok(CorpusSummary {
        id: row.get(0)?,
        story_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        source_url: row.get(5)?,
        source_name: row.get(6)?,
        tags: parse_tags(&tags),
        status: row.get(8)?,
        pinned: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn map_detail(row: &rusqlite::Row<'_>) -> Result<CorpusDetail, rusqlite::Error> {
    let tags: String = row.get(8)?;
    let distilled_json: Option<String> = row.get(14)?;
    let distilled = distilled_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<CorpusDistillation>(s).ok());
    Ok(CorpusDetail {
        id: row.get(0)?,
        story_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        content: row.get(5)?,
        source_url: row.get(6)?,
        source_name: row.get(7)?,
        tags: parse_tags(&tags),
        status: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        file_name: row.get(11)?,
        mime_type: row.get(12)?,
        file_size: row.get(13)?,
        distilled,
        links: Vec::new(),
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn repo() -> CorpusRepository {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        CorpusRepository::new(pool)
    }

    #[test]
    fn test_create_list_get_update_delete() {
        let repo = repo();
        let id = repo
            .create(
                "story-1",
                "news",
                "老街",
                "正文",
                "摘要",
                None,
                Some("城市观察"),
                vec!["社会".into()],
            )
            .unwrap();
        let list = repo
            .list("story-1", Some("news"), None, Some("老街"), 10, 0)
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tags, vec!["社会"]);
        let detail = repo.get(&id).unwrap().unwrap();
        assert_eq!(detail.content, "正文");

        let update = CorpusUpdate {
            title: Some("新标题".into()),
            pinned: Some(true),
            ..Default::default()
        };
        repo.update(&id, &update).unwrap();
        let detail = repo.get(&id).unwrap().unwrap();
        assert_eq!(detail.title, "新标题");
        assert!(detail.pinned);

        repo.delete(&id).unwrap();
        assert!(repo.get(&id).unwrap().is_none());
    }

    #[test]
    fn test_link_and_file_paths() {
        let repo = repo();
        let id = repo
            .create(
                "story-1",
                "concept",
                "概念",
                "正文",
                "摘要",
                None,
                None,
                vec![],
            )
            .unwrap();
        let link = repo
            .insert_link(&id, "character", "c1", Some("主角原型"))
            .unwrap();
        assert_eq!(repo.get(&id).unwrap().unwrap().links.len(), 1);
        repo.delete_link(&link).unwrap();
        assert!(repo.get(&id).unwrap().unwrap().links.is_empty());
    }
}
