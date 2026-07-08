use super::*;

// ==================== Scene Version Repository (新增) ====================

pub struct SceneVersionRepository {
    pool: DbPool,
}

impl SceneVersionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建场景版本快照
    pub fn create_version(
        &self,
        scene: &Scene,
        change_summary: &str,
        created_by: CreatorType,
        model_used: Option<&str>,
        confidence_score: Option<f32>,
    ) -> Result<SceneVersion, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        // 获取当前版本号
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let version_number: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM scene_versions WHERE scene_id = ?1",
            [&scene.id],
            |row| row.get(0),
        )?;

        // 获取上一个版本ID
        let previous_version_id: Option<String> = conn
            .query_row(
                "SELECT id FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC \
                 LIMIT 1",
                [&scene.id],
                |row| row.get(0),
            )
            .ok();

        let word_count = scene.content.as_ref().map(|c| c.len() as i32).unwrap_or(0);

        conn.execute(
            "INSERT INTO scene_versions (id, scene_id, version_number, title, content, \
             dramatic_goal, 
             external_pressure, conflict_type, characters_present, character_conflicts,
             setting_location, setting_time, setting_atmosphere, word_count, change_summary,
             created_by, model_used, confidence_score, previous_version_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
             ?18, ?19, ?20)",
            params![
                &id,
                &scene.id,
                version_number,
                scene.title,
                scene.content,
                scene.dramatic_goal,
                scene.external_pressure,
                scene.conflict_type.as_ref().map(|c| c.to_string()),
                serde_json::to_string(&scene.characters_present).unwrap(),
                serde_json::to_string(&scene.character_conflicts).unwrap(),
                scene.setting_location,
                scene.setting_time,
                scene.setting_atmosphere,
                word_count,
                change_summary,
                created_by.to_string(),
                model_used,
                confidence_score,
                previous_version_id,
                now.to_rfc3339()
            ],
        )?;

        // 标记上一个版本为被取代
        if let Some(prev_id) = &previous_version_id {
            conn.execute(
                "UPDATE scene_versions SET superseded_by = ?1 WHERE id = ?2",
                params![&id, prev_id],
            )?;
        }

        let version = SceneVersion {
            id,
            scene_id: scene.id.clone(),
            version_number,
            title: scene.title.clone(),
            content: scene.content.clone(),
            dramatic_goal: scene.dramatic_goal.clone(),
            external_pressure: scene.external_pressure.clone(),
            conflict_type: scene.conflict_type.clone(),
            characters_present: scene.characters_present.clone(),
            character_conflicts: scene.character_conflicts.clone(),
            setting_location: scene.setting_location.clone(),
            setting_time: scene.setting_time.clone(),
            setting_atmosphere: scene.setting_atmosphere.clone(),
            word_count,
            change_summary: change_summary.to_string(),
            created_by,
            model_used: model_used.map(|s| s.to_string()),
            confidence_score,
            previous_version_id,
            superseded_by: None,
            created_at: now,
        };

        Ok(version)
    }

    /// 获取场景的所有版本
    pub fn get_versions(&self, scene_id: &str) -> Result<Vec<SceneVersion>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, \
             external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, \
             setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, \
             confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC",
        )?;

        let versions = stmt
            .query_map([scene_id], |row| {
                let conflict_type_str: Option<String> = row.get(7)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row.get(8)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(9)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_by_str: String = row.get(15)?;
                let created_by = created_by_str.parse().unwrap_or(CreatorType::System);

                let created_str: String = row.get(20)?;

                Ok(SceneVersion {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    version_number: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    dramatic_goal: row.get(5)?,
                    external_pressure: row.get(6)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(10)?,
                    setting_time: row.get(11)?,
                    setting_atmosphere: row.get(12)?,
                    word_count: row.get(13)?,
                    change_summary: row.get(14)?,
                    created_by,
                    model_used: row.get(16)?,
                    confidence_score: row.get(17)?,
                    previous_version_id: row.get(18)?,
                    superseded_by: row.get(19)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(versions)
    }

    /// 获取特定版本
    pub fn get_version(&self, version_id: &str) -> Result<Option<SceneVersion>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, \
             external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, \
             setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, \
             confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE id = ?1",
        )?;

        let version = stmt
            .query_row([version_id], |row| {
                let conflict_type_str: Option<String> = row.get(7)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row.get(8)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(9)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_by_str: String = row.get(15)?;
                let created_by = created_by_str.parse().unwrap_or(CreatorType::System);

                let created_str: String = row.get(20)?;

                Ok(SceneVersion {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    version_number: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    dramatic_goal: row.get(5)?,
                    external_pressure: row.get(6)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(10)?,
                    setting_time: row.get(11)?,
                    setting_atmosphere: row.get(12)?,
                    word_count: row.get(13)?,
                    change_summary: row.get(14)?,
                    created_by,
                    model_used: row.get(16)?,
                    confidence_score: row.get(17)?,
                    previous_version_id: row.get(18)?,
                    superseded_by: row.get(19)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(version)
    }

    /// 删除版本
    pub fn delete_version(&self, version_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM scene_versions WHERE id = ?1", [version_id])?;
        Ok(count)
    }

    /// 获取场景版本数量
    pub fn get_version_count(&self, scene_id: &str) -> Result<i32, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM scene_versions WHERE scene_id = ?1",
            [scene_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
