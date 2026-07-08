use super::*;

// ==================== 文本内联批注 Repository ====================

pub struct TextAnnotationRepository {
    pool: DbPool,
}

impl TextAnnotationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_annotation(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        content: &str,
        annotation_type: &str,
        from_pos: i32,
        to_pos: i32,
    ) -> Result<TextAnnotation, rusqlite::Error> {
        self.create_annotation_with_meta(
            story_id,
            scene_id,
            chapter_id,
            content,
            annotation_type,
            from_pos,
            to_pos,
            None,
            "medium",
        )
    }

    /// 创建带 metadata 和 severity 的批注（用于 ai_audit 类型）。
    pub fn create_annotation_with_meta(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        content: &str,
        annotation_type: &str,
        from_pos: i32,
        to_pos: i32,
        metadata: Option<&str>,
        severity: &str,
    ) -> Result<TextAnnotation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO text_annotations (id, story_id, scene_id, chapter_id, content, \
             annotation_type, from_pos, to_pos, created_at, updated_at, metadata, severity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_id,
                content,
                annotation_type,
                from_pos,
                to_pos,
                now.to_rfc3339(),
                now.to_rfc3339(),
                metadata,
                severity
            ],
        )?;

        Ok(TextAnnotation {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            content: content.to_string(),
            annotation_type: annotation_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
            })?,
            from_pos,
            to_pos,
            created_at: now,
            updated_at: now,
            resolved_at: None,
            metadata: metadata.map(|s| s.to_string()),
            severity: severity.to_string(),
        })
    }

    pub fn get_annotations_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at, metadata, severity
             FROM text_annotations WHERE chapter_id = ?1 AND resolved_at IS NULL ORDER BY from_pos \
             ASC",
        )?;
        let rows = stmt.query([chapter_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at, metadata, severity
             FROM text_annotations WHERE scene_id = ?1 AND resolved_at IS NULL ORDER BY from_pos \
             ASC",
        )?;
        let rows = stmt.query([scene_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at, metadata, severity
             FROM text_annotations WHERE story_id = ?1 AND resolved_at IS NULL ORDER BY created_at \
             DESC",
        )?;
        let rows = stmt.query([story_id])?;
        Self::map_annotations(rows)
    }

    fn map_annotations(
        mut rows: rusqlite::Rows<'_>,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let mut annotations = Vec::new();
        while let Some(row) = rows.next()? {
            let type_str: String = row.get(5)?;
            let annotation_type = type_str.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
            })?;
            let created_str: String = row.get(8)?;
            let updated_str: String = row.get(9)?;
            let resolved_str: Option<String> = row.get(10)?;
            let metadata: Option<String> = row.get(11).unwrap_or(None);
            let severity: String = row.get(12).unwrap_or_else(|_| "medium".to_string());

            annotations.push(TextAnnotation {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                chapter_id: row.get(3)?,
                content: row.get(4)?,
                annotation_type,
                from_pos: row.get(6)?,
                to_pos: row.get(7)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
                metadata,
                severity,
            });
        }
        Ok(annotations)
    }

    pub fn update_annotation(
        &self,
        annotation_id: &str,
        content: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, content, now],
        )
    }

    pub fn resolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, now, now],
        )
    }

    pub fn unresolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![annotation_id, now],
        )
    }

    pub fn delete_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM text_annotations WHERE id = ?1",
            params![annotation_id],
        )
    }
}
