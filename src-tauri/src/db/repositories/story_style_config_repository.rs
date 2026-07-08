use super::*;

// ==================== StoryStyleConfig Repository ====================

pub struct StoryStyleConfigRepository {
    pool: DbPool,
}

impl StoryStyleConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        name: &str,
        blend_json: &str,
    ) -> Result<StoryStyleConfig, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_style_configs (id, story_id, name, blend_json, is_active, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&id, story_id, name, blend_json, 1, &now, &now],
        )?;

        Ok(StoryStyleConfig {
            id,
            story_id: story_id.to_string(),
            name: name.to_string(),
            blend_json: blend_json.to_string(),
            is_active: true,
            created_at: Local::now(),
            updated_at: Local::now(),
        })
    }

    pub fn get_active_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<StoryStyleConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 AND is_active = 1 LIMIT 1",
        )?;

        let result = stmt
            .query_row([story_id], |row| {
                let is_active: i32 = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryStyleConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    blend_json: row.get(3)?,
                    is_active: is_active != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn get_all_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<StoryStyleConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 ORDER BY updated_at DESC",
        )?;

        let configs = stmt
            .query_map([story_id], |row| {
                let is_active: i32 = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryStyleConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    blend_json: row.get(3)?,
                    is_active: is_active != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(configs)
    }

    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        blend_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE story_style_configs SET
                name = COALESCE(?2, name),
                blend_json = COALESCE(?3, blend_json),
                updated_at = ?4
             WHERE id = ?1",
            params![id, name, blend_json, now],
        )
    }

    pub fn set_active(&self, story_id: &str, config_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        // 先取消该 story 下所有配置的 active 状态
        conn.execute(
            "UPDATE story_style_configs SET is_active = 0 WHERE story_id = ?1",
            params![story_id],
        )?;
        // 再设置指定配置为 active
        conn.execute(
            "UPDATE story_style_configs SET is_active = 1 WHERE id = ?1 AND story_id = ?2",
            params![config_id, story_id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_style_configs WHERE id = ?1", params![id])
    }
}
