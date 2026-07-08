use super::*;

// ==================== 场景-角色关联 Repository ====================

pub struct SceneCharacterRepository {
    pool: DbPool,
}

impl SceneCharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 添加角色到场景
    pub fn add_character_to_scene(
        &self,
        scene_id: &str,
        character_id: &str,
    ) -> Result<SceneCharacter, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 检查是否已存在
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM scene_characters WHERE scene_id = ?1 AND character_id = ?2",
                [scene_id, character_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                Some("Character already in scene".to_string()),
            ));
        }

        conn.execute(
            "INSERT INTO scene_characters (id, scene_id, character_id, created_at) VALUES (?1, \
             ?2, ?3, ?4)",
            params![&id, scene_id, character_id, now.to_rfc3339()],
        )?;

        // 获取角色名称
        let character_name: Option<String> = conn
            .query_row(
                "SELECT name FROM characters WHERE id = ?1",
                [character_id],
                |row| row.get(0),
            )
            .ok();

        Ok(SceneCharacter {
            id,
            scene_id: scene_id.to_string(),
            character_id: character_id.to_string(),
            character_name,
            created_at: now,
        })
    }

    /// 从场景移除角色
    pub fn remove_character_from_scene(
        &self,
        scene_id: &str,
        character_id: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1 AND character_id = ?2",
            [scene_id, character_id],
        )
    }

    /// 获取场景中的所有角色
    pub fn get_characters_in_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<SceneCharacter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sc.id, sc.scene_id, sc.character_id, c.name, sc.created_at
             FROM scene_characters sc
             LEFT JOIN characters c ON sc.character_id = c.id
             WHERE sc.scene_id = ?1
             ORDER BY sc.created_at",
        )?;

        let scene_characters = stmt
            .query_map([scene_id], |row| {
                let created_str: String = row.get(4)?;
                Ok(SceneCharacter {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    character_id: row.get(2)?,
                    character_name: row.get(3)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scene_characters)
    }

    /// 获取角色参与的所有场景
    pub fn get_scenes_for_character(
        &self,
        character_id: &str,
    ) -> Result<Vec<SceneCharacter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sc.id, sc.scene_id, sc.character_id, c.name, sc.created_at
             FROM scene_characters sc
             LEFT JOIN characters c ON sc.character_id = c.id
             WHERE sc.character_id = ?1
             ORDER BY sc.created_at",
        )?;

        let scene_characters = stmt
            .query_map([character_id], |row| {
                let created_str: String = row.get(4)?;
                Ok(SceneCharacter {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    character_id: row.get(2)?,
                    character_name: row.get(3)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scene_characters)
    }

    /// 批量设置场景中的角色
    pub fn set_scene_characters(
        &self,
        scene_id: &str,
        character_ids: &[String],
    ) -> Result<Vec<SceneCharacter>, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 先清除现有关联
        tx.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1",
            [scene_id],
        )?;

        let mut result = Vec::new();
        let now = Local::now();

        // 添加新关联
        for character_id in character_ids {
            let id = Uuid::new_v4().to_string();

            tx.execute(
                "INSERT INTO scene_characters (id, scene_id, character_id, created_at) VALUES \
                 (?1, ?2, ?3, ?4)",
                params![&id, scene_id, character_id, now.to_rfc3339()],
            )?;

            // 获取角色名称
            let character_name: Option<String> = tx
                .query_row(
                    "SELECT name FROM characters WHERE id = ?1",
                    [character_id],
                    |row| row.get(0),
                )
                .ok();

            result.push(SceneCharacter {
                id,
                scene_id: scene_id.to_string(),
                character_id: character_id.clone(),
                character_name,
                created_at: now,
            });
        }

        tx.commit()?;
        Ok(result)
    }

    /// 删除场景的所有角色关联
    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1",
            [scene_id],
        )
    }

    /// 删除角色的所有场景关联
    pub fn delete_by_character(&self, character_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE character_id = ?1",
            [character_id],
        )
    }
}
