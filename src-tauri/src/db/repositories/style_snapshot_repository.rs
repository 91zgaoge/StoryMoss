use super::*;

// ==================== StyleSnapshot Repository (W3-B7) ====================

pub struct StyleSnapshotRepository {
    pool: DbPool,
}

impl StyleSnapshotRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        chapter_number: Option<i32>,
        scene_number: Option<i32>,
        metrics: &crate::creative_engine::style::metrics::StyleMetrics,
    ) -> Result<StyleSnapshot, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO style_snapshots
             (id, story_id, chapter_number, scene_number, sentence_length, dialogue_ratio,
              metaphor_density, inner_monologue_ratio, emotion_density, rhythm_score, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                story_id,
                chapter_number,
                scene_number,
                metrics.sentence_length as f64,
                metrics.dialogue_ratio as f64,
                metrics.metaphor_density as f64,
                metrics.inner_monologue_ratio as f64,
                metrics.emotion_density as f64,
                metrics.rhythm_score as f64,
                now,
            ],
        )?;

        Ok(StyleSnapshot {
            id,
            story_id: story_id.to_string(),
            chapter_number,
            scene_number,
            sentence_length: metrics.sentence_length as f64,
            dialogue_ratio: metrics.dialogue_ratio as f64,
            metaphor_density: metrics.metaphor_density as f64,
            inner_monologue_ratio: metrics.inner_monologue_ratio as f64,
            emotion_density: metrics.emotion_density as f64,
            rhythm_score: metrics.rhythm_score as f64,
            computed_at: Local::now(),
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<StyleSnapshot>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, scene_number,
                    sentence_length, dialogue_ratio, metaphor_density,
                    inner_monologue_ratio, emotion_density, rhythm_score, computed_at
             FROM style_snapshots WHERE story_id = ?1 ORDER BY computed_at DESC",
        )?;

        let snapshots = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(10)?;
                Ok(StyleSnapshot {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    scene_number: row.get(3)?,
                    sentence_length: row.get(4)?,
                    dialogue_ratio: row.get(5)?,
                    metaphor_density: row.get(6)?,
                    inner_monologue_ratio: row.get(7)?,
                    emotion_density: row.get(8)?,
                    rhythm_score: row.get(9)?,
                    computed_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    pub fn get_latest_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<StyleSnapshot>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, scene_number,
                    sentence_length, dialogue_ratio, metaphor_density,
                    inner_monologue_ratio, emotion_density, rhythm_score, computed_at
             FROM style_snapshots WHERE story_id = ?1 ORDER BY computed_at DESC LIMIT 1",
        )?;

        let result = stmt
            .query_row([story_id], |row| {
                let created_str: String = row.get(10)?;
                Ok(StyleSnapshot {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    scene_number: row.get(3)?,
                    sentence_length: row.get(4)?,
                    dialogue_ratio: row.get(5)?,
                    metaphor_density: row.get(6)?,
                    inner_monologue_ratio: row.get(7)?,
                    emotion_density: row.get(8)?,
                    rhythm_score: row.get(9)?,
                    computed_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM style_snapshots WHERE story_id = ?1",
            params![story_id],
        )
    }
}
