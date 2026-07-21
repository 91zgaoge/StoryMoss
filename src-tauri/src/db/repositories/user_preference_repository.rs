use super::*;

// ==================== UserPreference Repository ====================

pub struct UserPreferenceRepository {
    pool: DbPool,
}

impl UserPreferenceRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn upsert(
        &self,
        story_id: &str,
        preference_type: &str,
        preference_key: &str,
        preference_value: &str,
        confidence: f32,
        evidence_count: i32,
    ) -> Result<UserPreference, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        // 先检查是否已存在
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 AND \
                 preference_key = ?3",
                params![story_id, preference_type, preference_key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            // 更新
            conn.execute(
                "UPDATE user_preferences SET preference_value = ?4, confidence = ?5, \
                 evidence_count = ?6, updated_at = ?7
                 WHERE id = ?1",
                params![&id, preference_value, confidence, evidence_count, now],
            )?;

            Ok(UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type.parse().unwrap_or(PreferenceType::Content),
                preference_key: preference_key.to_string(),
                preference_value: preference_value.to_string(),
                confidence,
                evidence_count,
                updated_at: Local::now(),
            })
        } else {
            // 创建
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO user_preferences (id, story_id, preference_type, preference_key, \
                 preference_value, confidence, evidence_count, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &id,
                    story_id,
                    preference_type,
                    preference_key,
                    preference_value,
                    confidence,
                    evidence_count,
                    now
                ],
            )?;

            Ok(UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type.parse().unwrap_or(PreferenceType::Content),
                preference_key: preference_key.to_string(),
                preference_value: preference_value.to_string(),
                confidence,
                evidence_count,
                updated_at: Local::now(),
            })
        }
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<UserPreference>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, \
             evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 ORDER BY confidence DESC",
        )?;

        let prefs = stmt
            .query_map([story_id], |row| {
                let updated_str: String = row.get::<_, Option<String>>(7)?.unwrap_or_default();
                Ok(UserPreference {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    preference_type: row
                        .get::<_, Option<String>>(2)?
                        .unwrap_or_default()
                        .parse()
                        .unwrap_or(PreferenceType::Content),
                    preference_key: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    preference_value: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    confidence: row.get::<_, Option<f32>>(5)?.unwrap_or_default(),
                    evidence_count: row.get::<_, Option<i32>>(6)?.unwrap_or_default(),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn get_by_type(
        &self,
        story_id: &str,
        pref_type: &str,
    ) -> Result<Vec<UserPreference>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, \
             evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 ORDER BY \
             confidence DESC",
        )?;

        let prefs = stmt
            .query_map(params![story_id, pref_type], |row| {
                let updated_str: String = row.get::<_, Option<String>>(7)?.unwrap_or_default();
                Ok(UserPreference {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    preference_type: row
                        .get::<_, Option<String>>(2)?
                        .unwrap_or_default()
                        .parse()
                        .unwrap_or(PreferenceType::Content),
                    preference_key: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    preference_value: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    confidence: row.get::<_, Option<f32>>(5)?.unwrap_or_default(),
                    evidence_count: row.get::<_, Option<i32>>(6)?.unwrap_or_default(),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM user_preferences WHERE id = ?1", params![id])
    }
}
