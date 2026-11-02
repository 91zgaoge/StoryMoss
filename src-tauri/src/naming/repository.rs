//! v0.80.1：姓名账本（NameLedger）DB 仓库。
//!
//! P0 只在角色创建/迁移时落一条「本名 + 默认称呼规则」；完整字/号/绰号、
//! 候选池与作者编辑在后续接入。称呼图已从账本进入 writer 资产块。

use chrono::Local;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::AddressRule;
use crate::db::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameLedgerRecord {
    pub character_id: String,
    pub story_id: String,
    pub formal: String,
    pub surname: String,
    pub generation: Option<String>,
    pub given: String,
    pub courtesy: Option<String>,
    pub art: Option<String>,
    pub childhood: Option<String>,
    pub nickname: Option<String>,
    pub aliases_json: String,
    pub titles_json: String,
    pub taboo_json: String,
    pub address_rules_json: String,
    pub name_story: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 复姓表（与 `agency/continue_assets` 的别名启发式保持独立；账本以完整
/// 复姓为姓、其余为名）。
const COMPOUND_SURNAMES: &[&str] = &[
    "欧阳", "上官", "诸葛", "慕容", "司徒", "司马", "尉迟", "长孙", "宇文", "皇甫", "令狐", "独孤",
    "南宫", "西门", "东方", "夏侯", "公孙", "端木", "呼延", "赫连",
];

fn split_formal_name(formal: &str) -> (String, String) {
    let formal = formal.trim();
    if formal.is_empty() {
        return (String::new(), String::new());
    }
    for compound in COMPOUND_SURNAMES {
        if formal.starts_with(compound) && formal.chars().count() > compound.chars().count() {
            let given: String = formal.chars().skip(compound.chars().count()).collect();
            return (compound.to_string(), given);
        }
    }
    let surname = formal
        .chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let given: String = formal.chars().skip(1).collect();
    (surname, given)
}

impl NameLedgerRecord {
    pub fn from_character(story_id: &str, character_id: &str, formal: &str) -> Self {
        let formal = formal.trim().to_string();
        let (surname, given) = split_formal_name(&formal);
        let rules = vec![AddressRule {
            speaker: "众人".to_string(),
            addressee: formal.clone(),
            public_form: formal.clone(),
            private_form: formal.clone(),
            hostile_form: None,
            note: "默认称呼（作者可补充公开/私下/敌对叫法）".to_string(),
        }];
        let now = Local::now().to_rfc3339();
        Self {
            character_id: character_id.to_string(),
            story_id: story_id.to_string(),
            formal,
            surname,
            generation: None,
            given,
            courtesy: None,
            art: None,
            childhood: None,
            nickname: None,
            aliases_json: "[]".to_string(),
            titles_json: "[]".to_string(),
            taboo_json: "[]".to_string(),
            address_rules_json: serde_json::to_string(&rules).unwrap_or_else(|_| "[]".to_string()),
            name_story: String::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

pub fn ensure_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS character_names (
            character_id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            formal TEXT NOT NULL,
            surname TEXT,
            generation TEXT,
            given TEXT,
            courtesy TEXT,
            art TEXT,
            childhood TEXT,
            nickname TEXT,
            aliases_json TEXT NOT NULL DEFAULT '[]',
            titles_json TEXT NOT NULL DEFAULT '[]',
            taboo_json TEXT NOT NULL DEFAULT '[]',
            address_rules_json TEXT NOT NULL DEFAULT '[]',
            name_story TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_character_names_story
            ON character_names (story_id);",
    )
}

fn upsert_sql(tx: &rusqlite::Connection, record: &NameLedgerRecord) -> Result<(), rusqlite::Error> {
    let now = Local::now().to_rfc3339();
    tx.execute(
        "INSERT INTO character_names
            (character_id, story_id, formal, surname, generation, given, courtesy, art, childhood,
             nickname, aliases_json, titles_json, taboo_json, address_rules_json, name_story,
             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(character_id) DO UPDATE SET
            story_id = excluded.story_id,
            formal = excluded.formal,
            surname = excluded.surname,
            generation = excluded.generation,
            given = excluded.given,
            courtesy = excluded.courtesy,
            art = excluded.art,
            childhood = excluded.childhood,
            nickname = excluded.nickname,
            aliases_json = excluded.aliases_json,
            titles_json = excluded.titles_json,
            taboo_json = excluded.taboo_json,
            address_rules_json = excluded.address_rules_json,
            name_story = excluded.name_story,
            updated_at = excluded.updated_at",
        rusqlite::params![
            record.character_id,
            record.story_id,
            record.formal,
            record.surname,
            record.generation,
            record.given,
            record.courtesy,
            record.art,
            record.childhood,
            record.nickname,
            record.aliases_json,
            record.titles_json,
            record.taboo_json,
            record.address_rules_json,
            record.name_story,
            record.created_at,
            now,
        ],
    )?;
    Ok(())
}

/// 在角色创建事务内落账（与 characters/kg_entities 同事务）。
pub fn insert_in_tx(
    tx: &rusqlite::Transaction,
    record: &NameLedgerRecord,
) -> Result<(), rusqlite::Error> {
    ensure_table(tx)?;
    upsert_sql(tx, record)
}

fn backfill_from_characters(
    conn: &rusqlite::Connection,
    story_id: &str,
) -> Result<(), rusqlite::Error> {
    let mut stmt = match conn.prepare("SELECT id, name FROM characters WHERE story_id = ?1") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(()), // 测试/旧库无 characters：跳过
    };
    let Ok(rows) = stmt.query_map(rusqlite::params![story_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return Ok(());
    };
    for row in rows.flatten() {
        let (character_id, name) = row;
        let record = NameLedgerRecord::from_character(story_id, &character_id, &name);
        upsert_sql(conn, &record)?;
    }
    Ok(())
}

pub struct NameLedgerRepository {
    pool: DbPool,
}

impl NameLedgerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn upsert(&self, record: &NameLedgerRecord) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        ensure_table(&conn)?;
        upsert_sql(&conn, record)
    }

    pub fn list_for_story(&self, story_id: &str) -> Result<Vec<NameLedgerRecord>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        ensure_table(&conn)?;
        // 老故事按需回填：账本为空时从 characters 表读取本名。失败/表缺失
        // 不阻断续写，只是本场暂时没有称呼块。
        let existing: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_names WHERE story_id = ?1",
                rusqlite::params![story_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if existing == 0 {
            let _ = backfill_from_characters(&conn, story_id);
        }

        let mut stmt = conn.prepare(
            "SELECT character_id, story_id, formal, surname, generation, given, courtesy, art,
                    childhood, nickname, aliases_json, titles_json, taboo_json, address_rules_json,
                    name_story, created_at, updated_at
             FROM character_names WHERE story_id = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![story_id], |row| {
            Ok(NameLedgerRecord {
                character_id: row.get(0)?,
                story_id: row.get(1)?,
                formal: row.get(2)?,
                surname: row.get(3)?,
                generation: row.get(4)?,
                given: row.get(5)?,
                courtesy: row.get(6)?,
                art: row.get(7)?,
                childhood: row.get(8)?,
                nickname: row.get(9)?,
                aliases_json: row.get(10)?,
                titles_json: row.get(11)?,
                taboo_json: row.get(12)?,
                address_rules_json: row.get(13)?,
                name_story: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_by_character(
        &self,
        character_id: &str,
    ) -> Result<Option<NameLedgerRecord>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        ensure_table(&conn)?;
        conn.query_row(
            "SELECT character_id, story_id, formal, surname, generation, given, courtesy, art,
                    childhood, nickname, aliases_json, titles_json, taboo_json, address_rules_json,
                    name_story, created_at, updated_at
             FROM character_names WHERE character_id = ?1",
            rusqlite::params![character_id],
            |row| {
                Ok(NameLedgerRecord {
                    character_id: row.get(0)?,
                    story_id: row.get(1)?,
                    formal: row.get(2)?,
                    surname: row.get(3)?,
                    generation: row.get(4)?,
                    given: row.get(5)?,
                    courtesy: row.get(6)?,
                    art: row.get(7)?,
                    childhood: row.get(8)?,
                    nickname: row.get(9)?,
                    aliases_json: row.get(10)?,
                    titles_json: row.get(11)?,
                    taboo_json: row.get(12)?,
                    address_rules_json: row.get(13)?,
                    name_story: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                })
            },
        )
        .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn ledger_roundtrip() {
        let pool = create_test_pool().unwrap();
        let repo = NameLedgerRepository::new(pool);
        let rec = NameLedgerRecord::from_character("story-1", "char-1", "沈砚");
        repo.upsert(&rec).unwrap();
        let got = repo.get_by_character("char-1").unwrap().unwrap();
        assert_eq!(got.formal, "沈砚");
        assert_eq!(got.surname, "沈");
        assert_eq!(got.given, "砚");
        assert!(got.address_rules_json.contains("默认称呼"));
        let list = repo.list_for_story("story-1").unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn compound_surname_is_not_split_at_first_char() {
        let rec = NameLedgerRecord::from_character("story-1", "char-1", "欧阳修远");
        assert_eq!(rec.surname, "欧阳");
        assert_eq!(rec.given, "修远");
        let rec2 = NameLedgerRecord::from_character("story-1", "char-2", "上官婉儿");
        assert_eq!(rec2.surname, "上官");
        assert_eq!(rec2.given, "婉儿");
    }

    #[test]
    fn ledger_upsert_replaces_same_character() {
        let pool = create_test_pool().unwrap();
        let repo = NameLedgerRepository::new(pool);
        let mut rec = NameLedgerRecord::from_character("story-1", "char-1", "沈砚");
        rec.formal = "沈砚之".into();
        repo.upsert(&rec).unwrap();
        assert_eq!(
            repo.get_by_character("char-1").unwrap().unwrap().formal,
            "沈砚之"
        );
    }
}
