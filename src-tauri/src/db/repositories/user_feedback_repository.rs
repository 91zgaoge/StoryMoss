use super::*;

// ==================== UserFeedback Repository ====================

pub struct UserFeedbackRepository {
    pool: DbPool,
}

impl UserFeedbackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        feedback_type: &str,
        agent_type: Option<&str>,
        original_ai_text: &str,
        final_text: &str,
        ai_score: Option<f32>,
        user_satisfaction: Option<i32>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<UserFeedbackLog, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO user_feedback_log (id, story_id, scene_id, chapter_id, feedback_type, \
             agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_id,
                feedback_type,
                agent_type,
                original_ai_text,
                final_text,
                ai_score,
                user_satisfaction,
                metadata.map(|m| m.to_string()),
                now
            ],
        )?;

        Ok(UserFeedbackLog {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            feedback_type: feedback_type.parse().unwrap_or(FeedbackType::Accept),
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_ai_text.to_string(),
            final_text: final_text.to_string(),
            ai_score,
            user_satisfaction,
            metadata: metadata.cloned(),
            created_at: Local::now(),
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<UserFeedbackLog>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let sql = if let Some(lim) = limit {
            format!(
                "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
                 original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
                 FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC LIMIT {}",
                lim
            )
        } else {
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
             original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC"
                .to_string()
        };
        let mut stmt = conn.prepare(&sql)?;

        let logs = stmt
            .query_map([story_id], |row| {
                let meta_str: Option<String> = row.get(10)?;
                let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
                let created_str: String = row.get(11)?;
                Ok(UserFeedbackLog {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    feedback_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(FeedbackType::Accept),
                    agent_type: row.get(5)?,
                    original_ai_text: row.get(6)?,
                    final_text: row.get(7)?,
                    ai_score: row.get(8)?,
                    user_satisfaction: row.get(9)?,
                    metadata: meta,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_recent(
        &self,
        story_id: &str,
        days: i64,
    ) -> Result<Vec<UserFeedbackLog>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let cutoff = (Local::now() - chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
             original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 AND created_at >= ?2 ORDER BY created_at \
             DESC",
        )?;

        let logs = stmt
            .query_map(params![story_id, cutoff], |row| {
                let meta_str: Option<String> = row.get(10)?;
                let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
                let created_str: String = row.get(11)?;
                Ok(UserFeedbackLog {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    feedback_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(FeedbackType::Accept),
                    agent_type: row.get(5)?,
                    original_ai_text: row.get(6)?,
                    final_text: row.get(7)?,
                    ai_score: row.get(8)?,
                    user_satisfaction: row.get(9)?,
                    metadata: meta,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_stats(&self, story_id: &str) -> Result<FeedbackStats, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT feedback_type, COUNT(*) FROM user_feedback_log WHERE story_id = ?1 GROUP BY \
             feedback_type",
        )?;

        let mut accept = 0;
        let mut reject = 0;
        let mut modify = 0;

        let rows = stmt.query_map([story_id], |row| {
            let ft: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((ft, count))
        })?;

        for row in rows {
            let (ft, count) = row?;
            match ft.as_str() {
                "accept" => accept = count,
                "reject" => reject = count,
                "modify" => modify = count,
                _ => {}
            }
        }

        Ok(FeedbackStats {
            accept,
            reject,
            modify,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FeedbackStats {
    pub accept: i64,
    pub reject: i64,
    pub modify: i64,
}
