use super::*;

// ==================== StyleDNA Repository ====================

pub struct StyleDnaRepository {
    pool: DbPool,
}

impl StyleDnaRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        name: &str,
        author: Option<&str>,
        dna_json: &str,
        is_builtin: bool,
    ) -> Result<StyleDNA, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO style_dnas (id, name, author, dna_json, is_builtin, is_user_created, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                name,
                author,
                dna_json,
                is_builtin as i32,
                !is_builtin as i32,
                now
            ],
        )?;

        Ok(StyleDNA {
            id,
            name: name.to_string(),
            author: author.map(|s| s.to_string()),
            dna_json: dna_json.to_string(),
            is_builtin,
            is_user_created: !is_builtin,
            created_at: Local::now(),
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE id = ?1",
        )?;

        let result = stmt
            .query_row([id], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    /// 批量按 ID 查询 StyleDNA，将多次单条查询合并为一次 SQL IN 查询。
    pub fn get_many_by_ids(&self, ids: &[String]) -> Result<Vec<StyleDNA>, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE id IN ({}) ORDER BY name ASC",
            placeholders
        );

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(&sql)?;

        let dnas = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn get_all(&self) -> Result<Vec<StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas ORDER BY is_builtin DESC, name ASC",
        )?;

        let dnas = stmt
            .query_map([], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn get_builtin(&self) -> Result<Vec<StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE is_builtin = 1 ORDER BY name ASC",
        )?;

        let dnas = stmt
            .query_map([], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM style_dnas WHERE id = ?1 AND is_builtin = 0",
            params![id],
        )
    }

    pub fn update_dna_json(&self, id: &str, dna_json: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE style_dnas SET dna_json = ?2 WHERE id = ?1",
            params![id, dna_json],
        )
    }
}
