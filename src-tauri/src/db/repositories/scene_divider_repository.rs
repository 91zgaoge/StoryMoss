use super::*;

// ==================== SceneDividerNode Repository ====================

pub struct SceneDividerRepository {
    pool: DbPool,
}

impl SceneDividerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 为指定章节创建 divider
    pub fn create(
        &self,
        chapter_id: &str,
        position: i32,
        scene_id: &str,
        label: Option<&str>,
    ) -> Result<SceneDividerNode, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO scene_divider_nodes (id, chapter_id, position, scene_id, label, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                chapter_id,
                position,
                scene_id,
                label,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;
        Ok(SceneDividerNode {
            id,
            chapter_id: chapter_id.to_string(),
            position,
            scene_id: scene_id.to_string(),
            label: label.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    /// 获取章节下的所有 divider，按 position 排序
    pub fn get_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<SceneDividerNode>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, chapter_id, position, scene_id, label, created_at, updated_at
             FROM scene_divider_nodes WHERE chapter_id = ?1 ORDER BY position ASC",
        )?;
        let nodes = stmt
            .query_map([chapter_id], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(SceneDividerNode {
                    id: row.get(0)?,
                    chapter_id: row.get(1)?,
                    position: row.get(2)?,
                    scene_id: row.get(3)?,
                    label: row.get(4)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(nodes)
    }

    /// 批量设置章节的 divider（用于重排/重建 divider）
    pub fn set_dividers(
        &self,
        chapter_id: &str,
        dividers: &[(String, i32, Option<String>)], // (scene_id, position, label)
    ) -> Result<Vec<SceneDividerNode>, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM scene_divider_nodes WHERE chapter_id = ?1",
            [chapter_id],
        )?;
        let now = Local::now();
        let mut nodes = Vec::new();
        for (scene_id, position, label) in dividers {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO scene_divider_nodes (id, chapter_id, position, scene_id, label, \
                 created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    &id,
                    chapter_id,
                    position,
                    scene_id,
                    label,
                    now.to_rfc3339(),
                    now.to_rfc3339()
                ],
            )?;
            nodes.push(SceneDividerNode {
                id,
                chapter_id: chapter_id.to_string(),
                position: *position,
                scene_id: scene_id.clone(),
                label: label.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        tx.commit()?;
        Ok(nodes)
    }

    /// 删除单个 divider
    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM scene_divider_nodes WHERE id = ?1", [id])
    }

    /// 删除章节的所有 divider
    pub fn delete_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_divider_nodes WHERE chapter_id = ?1",
            [chapter_id],
        )
    }
}
