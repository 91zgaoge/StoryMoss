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
            // v0.59.3：带内容更新即视为「作者手写/已确认」→ 标记 source=user_created，
            // 后续机器路径（materialize / ingest）不得再覆盖或追加（见 V134）。
            "UPDATE story_outlines SET content = COALESCE(?2, content), structure_json = \
             COALESCE(?3, structure_json), \
             source = CASE WHEN ?2 IS NOT NULL THEN 'user_created' ELSE source END, \
             updated_at = ?4 WHERE story_id = ?1",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_test_pool, CreateStoryRequest, StoryRepository};

    fn story(pool: &crate::db::DbPool, _id: &str) -> String {
        StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "大纲来源测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id
    }

    fn source_of(pool: &crate::db::DbPool, story_id: &str) -> Option<String> {
        let conn = pool.get().unwrap();
        conn.query_row(
            "SELECT source FROM story_outlines WHERE story_id = ?1",
            params![story_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
    }

    /// v0.59.3：用户带内容保存 = 手写/已确认 → 打上 user_created，
    /// 机器路径（创世 materialize / ingest）此后不得覆盖或追加。
    #[test]
    fn update_with_content_marks_user_created() {
        let pool = create_test_pool().unwrap();
        let sid = story(&pool, "s1");
        let repo = StoryOutlineRepository::new(pool.clone());
        repo.create(&sid, "机器初稿", None, 3, None).unwrap();
        repo.update(&sid, Some("作者改写后的三幕结构"), None)
            .unwrap();
        assert_eq!(source_of(&pool, &sid).as_deref(), Some("user_created"));
    }

    /// 只改 structure_json（content=None）不改来源标记——否则机器侧更新
    /// 结构时会把机器大纲误标成手写，永久冻结后续精炼。
    #[test]
    fn update_without_content_keeps_source() {
        let pool = create_test_pool().unwrap();
        let sid = story(&pool, "s1");
        let repo = StoryOutlineRepository::new(pool.clone());
        repo.create(&sid, "机器初稿", None, 3, None).unwrap();
        repo.update(&sid, None, Some("{\"act1\":\"x\"}")).unwrap();
        let src = source_of(&pool, &sid);
        assert!(
            src.is_none() || src.as_deref() == Some("unknown"),
            "无内容更新不得把来源改成 user_created：{src:?}"
        );
    }
}
