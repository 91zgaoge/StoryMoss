//! 人物别称仓库（V140）。
//!
//! 中文小说里同一人物有多个称呼：姓+称号（苏世子）、称号+名（景亲王曹元寿）、
//! 字、号、 官职、小名。此前 `characters` 只有 name
//! 且按名精确匹配建行，于是每个新称呼都会长出
//! 一个幻影人物。本仓库负责登记与解析「称呼 → 人物」。
//!
//! 幂等：同 (story, alias) 重复登记只更新归属（`ON CONFLICT DO UPDATE`）。

use chrono::Local;
use rusqlite::{params, OptionalExtension};

use crate::db::DbPool;

pub struct CharacterAliasRepository {
    pool: DbPool,
}

impl CharacterAliasRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 登记别称。同一故事内同一称呼已存在时更新归属（后到的解析结果覆盖）。
    /// 返回是否发生写入。
    pub fn upsert(
        &self,
        story_id: &str,
        character_id: &str,
        alias: &str,
        source: &str,
    ) -> Result<bool, rusqlite::Error> {
        let alias = alias.trim();
        if alias.is_empty() {
            return Ok(false);
        }
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let n = conn.execute(
            "INSERT INTO character_aliases (id, story_id, character_id, alias, source, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(story_id, alias) DO UPDATE SET character_id = excluded.character_id \
             WHERE character_aliases.character_id <> excluded.character_id",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                character_id,
                alias,
                source,
                Local::now().to_rfc3339()
            ],
        )?;
        Ok(n > 0)
    }

    /// 按称呼解析人物 id。精确匹配别称表（name 本身不存别名表，由调用方先查
    /// name）。
    pub fn resolve(&self, story_id: &str, alias: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.query_row(
            "SELECT character_id FROM character_aliases WHERE story_id = ?1 AND alias = ?2",
            params![story_id, alias.trim()],
            |r| r.get(0),
        )
        .optional()
    }

    /// 该故事全部别称（按登记时间升序，便于「先登记的为准」）。
    pub fn list_by_story(&self, story_id: &str) -> Result<Vec<(String, String)>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT alias, character_id FROM character_aliases WHERE story_id = ?1 \
             ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map([story_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 某角色的全部别称。
    pub fn aliases_of(
        &self,
        story_id: &str,
        character_id: &str,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT alias FROM character_aliases WHERE story_id = ?1 AND character_id = ?2 \
             ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![story_id, character_id], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
