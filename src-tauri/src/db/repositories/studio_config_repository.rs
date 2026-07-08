use super::*;

// ==================== StudioConfig Repository ====================

pub struct StudioConfigRepository {
    pool: DbPool,
}

impl StudioConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建默认配置 (兼容旧接口)
    pub fn create(&self, story_id: &str) -> Result<StudioConfig, rusqlite::Error> {
        self.create_default(story_id, "新建工作室")
    }

    pub fn create_default(
        &self,
        story_id: &str,
        title: &str,
    ) -> Result<StudioConfig, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let llm_config = LlmStudioConfig {
            default_provider: "openai".to_string(),
            default_model: "gpt-4".to_string(),
            generation_temperature: 0.7,
            max_tokens: 4096,
            profiles: vec![],
        };

        let ui_config = UiStudioConfig {
            frontstage_font_size: 18,
            frontstage_font_family: "Noto Serif SC".to_string(),
            frontstage_line_height: 1.8,
            frontstage_paper_color: "#f5f4ed".to_string(),
            frontstage_text_color: "#2c2c2c".to_string(),
            backstage_theme: "dark".to_string(),
            backstage_accent_color: "#6366f1".to_string(),
        };

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO studio_configs (id, story_id, pen_name, llm_config, ui_config, 
             agent_bots, frontstage_theme, backstage_theme, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                title,
                serde_json::to_string(&llm_config).unwrap(),
                serde_json::to_string(&ui_config).unwrap(),
                "[]",
                "paper",
                "dark",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(StudioConfig {
            id,
            story_id: story_id.to_string(),
            pen_name: Some(title.to_string()),
            llm_config,
            ui_config,
            agent_bots: vec![],
            frontstage_theme: Some("paper".to_string()),
            backstage_theme: Some("dark".to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<StudioConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, pen_name, llm_config, ui_config, agent_bots, 
                    frontstage_theme, backstage_theme, created_at, updated_at 
             FROM studio_configs WHERE story_id = ?1",
        )?;

        let config = stmt
            .query_row([story_id], |row| {
                let llm_json: String = row.get(3)?;
                let llm_config: LlmStudioConfig =
                    serde_json::from_str(&llm_json).unwrap_or_default();

                let ui_json: String = row.get(4)?;
                let ui_config: UiStudioConfig = serde_json::from_str(&ui_json).unwrap_or_default();

                let bots_json: String = row.get(5)?;
                let agent_bots: Vec<AgentBotConfig> =
                    serde_json::from_str(&bots_json).unwrap_or_default();

                let created_str: String = row.get(8)?;
                let updated_str: String = row.get(9)?;

                Ok(StudioConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    pen_name: row.get(2)?,
                    llm_config,
                    ui_config,
                    agent_bots,
                    frontstage_theme: row.get(6)?,
                    backstage_theme: row.get(7)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(config)
    }

    /// 更新配置 (兼容旧接口)
    pub fn update(
        &self,
        id: &str,
        _pen_name: Option<&str>,
        llm_config: Option<&LlmStudioConfig>,
        ui_config: Option<&UiStudioConfig>,
        agent_bots: Option<&[AgentBotConfig]>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE studio_configs SET 
                llm_config = COALESCE(?2, llm_config),
                ui_config = COALESCE(?3, ui_config),
                agent_bots = COALESCE(?4, agent_bots),
                updated_at = ?5
             WHERE id = ?1",
            params![
                id,
                llm_config.map(|c| serde_json::to_string(c).unwrap()),
                ui_config.map(|c| serde_json::to_string(c).unwrap()),
                agent_bots.map(|b| serde_json::to_string(&b.to_vec()).unwrap()),
                now
            ],
        )?;
        Ok(count)
    }

    /// 更新主题
    pub fn update_themes(
        &self,
        id: &str,
        frontstage_theme: Option<&str>,
        backstage_theme: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE studio_configs SET 
                frontstage_theme = COALESCE(?2, frontstage_theme),
                backstage_theme = COALESCE(?3, backstage_theme),
                updated_at = ?4
             WHERE id = ?1",
            params![id, frontstage_theme, backstage_theme, now],
        )?;
        Ok(count)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StudioConfigUpdate {
    pub pen_name: Option<String>,
    pub llm_config: Option<LlmStudioConfig>,
    pub ui_config: Option<UiStudioConfig>,
    pub agent_bots: Option<Vec<AgentBotConfig>>,
    pub frontstage_theme: Option<String>,
    pub backstage_theme: Option<String>,
}
