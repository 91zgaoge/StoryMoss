use super::*;

// ==================== Story Outline Repository ====================

pub struct StoryOutlineRepository {
    pool: DbPool,
}

impl StoryOutlineRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        content: &str,
        structure_json: Option<&str>,
        act_count: i32,
        total_scenes_estimate: Option<i32>,
    ) -> Result<StoryOutline, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, \
             total_scenes_estimate, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                content,
                structure_json,
                act_count,
                total_scenes_estimate,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(StoryOutline {
            id,
            story_id: story_id.to_string(),
            content: content.to_string(),
            structure_json: structure_json.map(|s| s.to_string()),
            act_count,
            total_scenes_estimate,
            created_at: now,
            updated_at: now,
            analyzed_structure_json: None,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<StoryOutline>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, content, structure_json, act_count, total_scenes_estimate, \
             analyzed_structure_json, created_at, updated_at
             FROM story_outlines WHERE story_id = ?1",
        )?;

        let outline = stmt
            .query_row([story_id], |row| {
                let created_str: String = row.get(7)?;
                let updated_str: String = row.get(8)?;

                Ok(StoryOutline {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    content: row.get(2)?,
                    structure_json: row.get(3)?,
                    act_count: row.get(4)?,
                    total_scenes_estimate: row.get(5)?,
                    analyzed_structure_json: row.get(6)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(outline)
    }

    pub fn update(
        &self,
        story_id: &str,
        content: Option<&str>,
        structure_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE story_outlines SET content = COALESCE(?2, content), structure_json = \
             COALESCE(?3, structure_json), updated_at = ?4 WHERE story_id = ?1",
            params![story_id, content, structure_json, now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_outlines WHERE story_id = ?1", [story_id])
    }

    /// 更新分析后的幕级结构 JSON
    pub fn update_analyzed_structure_json(
        &self,
        story_id: &str,
        analyzed_structure_json: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE story_outlines SET analyzed_structure_json = ?2, updated_at = ?3 WHERE story_id = ?1",
            params![story_id, analyzed_structure_json, now],
        )?;
        Ok(count)
    }
}
