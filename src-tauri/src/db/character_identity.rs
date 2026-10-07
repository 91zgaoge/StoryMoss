//! 人物身份归一：「称呼 → 人物」的解析与合并（v0.64.6）。
//!
//! 中文小说里同一人物有多个称呼：姓+称号（苏世子）、称号+名（景亲王曹元寿）、
//! 字、号、 官职、小名。此前 `characters` 只有 name 且 ingest
//! 按名精确匹配建行，于是每个新称呼
//! 都会长出一个幻影人物行（真机《帝国的烟火》：`苏世子` 与 `景亲王` /
//! `景亲王曹元寿` 各占一行，关系表与提示词里出现"两个同一个人"）。
//!
//! 本模块提供三件事：
//! 1. `resolve_character_id`：建行前解析——精确名 → 别称表 →
//!    称号形态（`same_person`）；
//! 2. `record_aliases`：登记别称，并在别称与某个既有角色行同名时把那一行**合并*
//!    *进来 （自愈：LLM 一旦给出「苏世子 = 苏亦铁」，幻影行自动并回本人）；
//! 3. `merge_characters`：真正改线——把 from
//!    的所有引用（状态/关系/场景关联/行为/知情 流水/物品持有）转到
//!    to，去重关系与场景关联，改写正文里的名字 token，删除 from 行。

use rusqlite::{params, Connection, OptionalExtension};

/// 合并结果（供日志与测试断言）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MergeReport {
    pub states_moved: usize,
    pub relations_moved: usize,
    pub relation_dups_dropped: usize,
    pub scene_links_moved: usize,
    pub scene_link_dups_dropped: usize,
    pub actions_moved: usize,
    pub knowledge_moved: usize,
    pub holdings_moved: usize,
    pub name_tokens_rewritten: usize,
    pub aliases_moved: usize,
}

impl MergeReport {
    pub fn any_change(&self) -> bool {
        self.states_moved
            + self.relations_moved
            + self.relation_dups_dropped
            + self.scene_links_moved
            + self.scene_link_dups_dropped
            + self.actions_moved
            + self.knowledge_moved
            + self.holdings_moved
            + self.name_tokens_rewritten
            + self.aliases_moved
            > 0
    }
}

fn character_name(conn: &Connection, id: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row("SELECT name FROM characters WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
}

/// 建行前解析：精确名 → 别称表 → 称号形态（`same_person` 变体）。
///
/// 返回命中的既有角色 id。`same_person`
/// 变体只在唯一命中时采纳——同名候选多于一个时
/// 宁可新建也不猜（避免把两个真的不同角色并到一起）。
pub fn resolve_character_id(
    conn: &Connection,
    story_id: &str,
    name: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2 LIMIT 1",
            params![story_id, name],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(Some(id));
    }
    if let Some(id) = conn
        .query_row(
            "SELECT character_id FROM character_aliases WHERE story_id = ?1 AND alias = ?2",
            params![story_id, name],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(Some(id));
    }

    let mut stmt = conn.prepare("SELECT id, name FROM characters WHERE story_id = ?1")?;
    let rows = stmt
        .query_map([story_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let hits: Vec<String> = rows
        .into_iter()
        .filter(|(_, existing)| crate::agency::continue_director::same_person(existing, name))
        .map(|(id, _)| id)
        .collect();
    Ok(if hits.len() == 1 {
        Some(hits[0].clone())
    } else {
        None
    })
}

/// 登记别称；若别称与某个既有角色行同名，把那一行合并进 `canonical_id`。
///
/// 这是幻影人物的**自愈路径**：抽取一旦给出「苏世子 = 苏亦铁」，此前误建的
/// `苏世子` 行会被并回 `苏亦铁`。
pub fn record_aliases(
    conn: &Connection,
    story_id: &str,
    canonical_id: &str,
    aliases: &[String],
    source: &str,
) -> Result<Vec<MergeReport>, rusqlite::Error> {
    let mut reports = Vec::new();
    for alias in aliases {
        let alias = alias.trim();
        if alias.is_empty() {
            continue;
        }
        if let Some(canonical_name) = character_name(conn, canonical_id)? {
            if crate::agency::continue_director::same_person(&canonical_name, alias) {
                // 称号形态本来就等价：只登记，不建行也不合并
            }
        }
        // 同名行 → 合并进来（改线 + 删除幻影行）
        if let Some(phantom_id) = conn
            .query_row(
                "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2 LIMIT 1",
                params![story_id, alias],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            if phantom_id != canonical_id {
                let report = merge_characters(conn, story_id, &phantom_id, canonical_id)?;
                reports.push(report);
            }
        }
        // 别称被别的角色占用时不抢归属（先到先得），
        // 但同名行已并进来的情况已处理
        conn.execute(
            "INSERT INTO character_aliases (id, story_id, character_id, alias, source, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(story_id, alias) DO NOTHING",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                canonical_id,
                alias,
                source,
                chrono::Local::now().to_rfc3339()
            ],
        )?;
    }
    Ok(reports)
}

/// 把 `from_id` 合并进 `to_id`：改线所有引用、去重关系与场景关联、改写正文名字
/// token、 搬迁别称，最后删除 `from` 行。调用方负责事务（本函数只发语句）。
pub fn merge_characters(
    conn: &Connection,
    story_id: &str,
    from_id: &str,
    to_id: &str,
) -> Result<MergeReport, rusqlite::Error> {
    let mut report = MergeReport::default();
    if from_id == to_id {
        return Ok(report);
    }
    let Some(from_name) = character_name(conn, from_id)? else {
        return Ok(report);
    };
    let Some(to_name) = character_name(conn, to_id)? else {
        return Ok(report);
    };

    // 1) 状态：同一角色只保留最新一条
    report.states_moved += conn.execute(
        "UPDATE character_states SET character_id = ?3 WHERE story_id = ?1 AND character_id = ?2",
        params![story_id, from_id, to_id],
    )?;
    dedup_character_states(conn, story_id, to_id)?;

    // 2) 关系：先删与 to 已成对的重复行（保留 to 的行），再改线
    for (a_col, b_col) in [
        ("source_character_id", "target_character_id"),
        ("target_character_id", "source_character_id"),
    ] {
        report.relation_dups_dropped += conn.execute(
            &format!(
                "DELETE FROM character_relationships \
                 WHERE story_id = ?1 AND {a_col} = ?2 \
                   AND EXISTS (SELECT 1 FROM character_relationships r2 \
                                WHERE r2.story_id = ?1 AND r2.{a_col} = ?3 \
                                  AND r2.{b_col} = character_relationships.{b_col})",
            ),
            params![story_id, from_id, to_id],
        )?;
    }
    report.relations_moved += conn.execute(
        "UPDATE character_relationships SET source_character_id = ?3 \
         WHERE story_id = ?1 AND source_character_id = ?2",
        params![story_id, from_id, to_id],
    )?;
    report.relations_moved += conn.execute(
        "UPDATE character_relationships SET target_character_id = ?3 \
         WHERE story_id = ?1 AND target_character_id = ?2",
        params![story_id, from_id, to_id],
    )?;

    // 3) 场景关联：UNIQUE(scene_id, character_id)
    report.scene_link_dups_dropped += conn.execute(
        "DELETE FROM scene_characters WHERE character_id = ?1 AND scene_id IN \
         (SELECT scene_id FROM scene_characters WHERE character_id = ?2)",
        params![from_id, to_id],
    )?;
    report.scene_links_moved += conn.execute(
        "UPDATE scene_characters SET character_id = ?2 WHERE character_id = ?1",
        params![from_id, to_id],
    )?;

    // 4) 行为 / 知情流水 / 物品持有
    report.actions_moved += conn.execute(
        "UPDATE scene_character_actions SET character_id = ?2 WHERE character_id = ?1",
        params![from_id, to_id],
    )?;
    report.knowledge_moved += conn.execute(
        "UPDATE character_knowledge_log SET character_id = ?2 WHERE character_id = ?1",
        params![from_id, to_id],
    )?;
    report.holdings_moved += conn.execute(
        "UPDATE item_holdings SET holder_character_id = ?2, holder_name = ?3 \
         WHERE holder_character_id = ?1",
        params![from_id, to_id, to_name],
    )?;

    // 5) 正文/版本里的名字 token（JSON 数组或对象里的带引号名字）。 scenes 有
    //    story_id；scene_versions 没有，按 scene_id 归属过滤。
    for col in ["characters_present", "character_conflicts"] {
        report.name_tokens_rewritten += conn.execute(
            &format!(
                "UPDATE scenes SET {col} = REPLACE({col}, ?3, ?4) \
                 WHERE story_id = ?1 AND {col} LIKE ?2"
            ),
            params![
                story_id,
                format!("%\"{from_name}\"%"),
                format!("\"{from_name}\""),
                format!("\"{to_name}\"")
            ],
        )?;
        report.name_tokens_rewritten += conn.execute(
            &format!(
                "UPDATE scene_versions SET {col} = REPLACE({col}, ?3, ?4) \
                 WHERE {col} LIKE ?2 AND scene_id IN (SELECT id FROM scenes WHERE story_id = ?1)"
            ),
            params![
                story_id,
                format!("%\"{from_name}\"%"),
                format!("\"{from_name}\""),
                format!("\"{to_name}\"")
            ],
        )?;
    }

    // 6) 别称搬迁（UNIQUE(story_id, alias)：冲突时丢弃 from 侧那条）
    report.aliases_moved += conn.execute(
        "DELETE FROM character_aliases WHERE character_id = ?1 AND alias IN \
         (SELECT alias FROM character_aliases WHERE character_id = ?2)",
        params![from_id, to_id],
    )?;
    report.aliases_moved += conn.execute(
        "UPDATE character_aliases SET character_id = ?2 WHERE character_id = ?1",
        params![from_id, to_id],
    )?;
    // 把被并掉的称呼本身登记成别称（此后同称呼不会再建行）
    conn.execute(
        "INSERT INTO character_aliases (id, story_id, character_id, alias, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, 'merge', ?5) \
         ON CONFLICT(story_id, alias) DO NOTHING",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            to_id,
            from_name,
            chrono::Local::now().to_rfc3339()
        ],
    )?;

    // 7) 先把被并行独有的资料补到保留行的空字段，再删除幻影行（避免合并丢信息）
    conn.execute(
        "UPDATE characters SET \
           background = COALESCE(NULLIF(TRIM(background), ''), (SELECT NULLIF(TRIM(background), '') FROM characters WHERE id = ?1)), \
           personality = COALESCE(NULLIF(TRIM(personality), ''), (SELECT NULLIF(TRIM(personality), '') FROM characters WHERE id = ?1)), \
           goals = COALESCE(NULLIF(TRIM(goals), ''), (SELECT NULLIF(TRIM(goals), '') FROM characters WHERE id = ?1)), \
           appearance = COALESCE(NULLIF(TRIM(appearance), ''), (SELECT NULLIF(TRIM(appearance), '') FROM characters WHERE id = ?1)), \
           gender = COALESCE(NULLIF(TRIM(gender), ''), (SELECT NULLIF(TRIM(gender), '') FROM characters WHERE id = ?1)), \
           age = COALESCE(age, (SELECT age FROM characters WHERE id = ?1)), \
           emotional_core = COALESCE(NULLIF(TRIM(emotional_core), ''), (SELECT NULLIF(TRIM(emotional_core), '') FROM characters WHERE id = ?1)), \
           emotional_trigger = COALESCE(NULLIF(TRIM(emotional_trigger), ''), (SELECT NULLIF(TRIM(emotional_trigger), '') FROM characters WHERE id = ?1)), \
           emotional_wound = COALESCE(NULLIF(TRIM(emotional_wound), ''), (SELECT NULLIF(TRIM(emotional_wound), '') FROM characters WHERE id = ?1)), \
           emotional_need = COALESCE(NULLIF(TRIM(emotional_need), ''), (SELECT NULLIF(TRIM(emotional_need), '') FROM characters WHERE id = ?1)), \
           updated_at = ?3 \
         WHERE id = ?2",
        params![from_id, to_id, chrono::Local::now().to_rfc3339()],
    )?;
    conn.execute("DELETE FROM characters WHERE id = ?1", [from_id])?;
    log::info!(
        "[character_identity] 合并人物「{}」→「{}」：状态 {}、关系 {}（去重 {}）、场景关联 {}（去重 {}）、行为 {}、知情 {}、持有 {}、正文名字 {} 处",
        from_name,
        to_name,
        report.states_moved,
        report.relations_moved,
        report.relation_dups_dropped,
        report.scene_links_moved,
        report.scene_link_dups_dropped,
        report.actions_moved,
        report.knowledge_moved,
        report.holdings_moved,
        report.name_tokens_rewritten
    );
    Ok(report)
}

/// 同一角色多条状态时保留最新一条（`updated_at_chapter` 大者优先，其次
/// `last_updated`）。
fn dedup_character_states(
    conn: &Connection,
    story_id: &str,
    character_id: &str,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM character_states WHERE story_id = ?1 AND character_id = ?2 AND id NOT IN \
         (SELECT id FROM character_states WHERE story_id = ?1 AND character_id = ?2 \
          ORDER BY COALESCE(updated_at_chapter, 0) DESC, COALESCE(last_updated, '') DESC, id \
          LIMIT 1)",
        params![story_id, character_id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        create_test_pool, repositories::CharacterRelationshipRepository, CreateStoryRequest,
        DbPool, StoryRepository,
    };

    fn seed(pool: &DbPool) -> String {
        StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "人物身份归一".to_string(),
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

    fn add_character(conn: &Connection, story: &str, name: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, source, is_auto_generated, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'ingest', 1, ?4, ?4)",
            params![id, story, name, now],
        )
        .unwrap();
        id
    }

    /// 真机形态：`景亲王` 与 `景亲王曹元寿` 是一个人的两种称呼 →
    /// 称号形态直接可解析。
    #[test]
    fn resolve_by_title_form() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        let story = seed(&pool);
        let jing = add_character(&conn, &story, "景亲王曹元寿");

        assert_eq!(
            resolve_character_id(&conn, &story, "景亲王")
                .unwrap()
                .as_deref(),
            Some(jing.as_str()),
            "称号形态应解析到同一人"
        );
    }

    /// 真机形态：`苏世子` 是称呼不是新人物——靠别称表解析。
    #[test]
    fn resolve_by_alias_and_absorb_phantom() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        let story = seed(&pool);
        let su = add_character(&conn, &story, "苏亦铁");
        let phantom = add_character(&conn, &story, "苏世子");
        // 幻影行上还挂着一层关系，合并必须把它带走
        let other = add_character(&conn, &story, "景亲王");
        CharacterRelationshipRepository::new(pool.clone())
            .create(
                &story,
                &phantom,
                &other,
                "上下级",
                Some("世子与亲王"),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();

        let reports =
            record_aliases(&conn, &story, &su, &["苏世子".to_string()], "ingest").unwrap();
        assert_eq!(reports.len(), 1, "同名幻影行必须被合并");
        assert!(reports[0].relations_moved >= 1, "关系要改线到本人");

        assert_eq!(
            resolve_character_id(&conn, &story, "苏世子")
                .unwrap()
                .as_deref(),
            Some(su.as_str())
        );
        let phantom_left: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE id = ?1",
                [&phantom],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(phantom_left, 0, "幻影行应被删除");
        let rel: String = conn
            .query_row(
                "SELECT source_character_id FROM character_relationships WHERE story_id = ?1",
                [&story],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rel, su, "关系应指向本人");
    }

    /// 没有任何依据时不得乱并：精确名与别称表都不命中 → 返回
    /// None（调用方新建）。
    #[test]
    fn resolve_returns_none_for_unknown() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        let story = seed(&pool);
        add_character(&conn, &story, "苏亦铁");
        assert!(resolve_character_id(&conn, &story, "苏世子")
            .unwrap()
            .is_none());
        assert!(resolve_character_id(&conn, &story, "苏亦俭")
            .unwrap()
            .is_none());
    }

    /// 合并要处理 UNIQUE 约束：场景关联与关系都不能留下重复行。
    #[test]
    fn merge_dedups_scene_links_and_relations() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        let story = seed(&pool);
        let a = add_character(&conn, &story, "景亲王");
        let b = add_character(&conn, &story, "景亲王曹元寿");
        let c = add_character(&conn, &story, "曹元佩");
        let scene_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, characters_present, \
             character_conflicts, execution_stage, created_at, updated_at) \
             VALUES (?1, ?2, 1, ?3, '[]', 'drafting', '2026-01-01T00:00:00+08:00', '2026-01-01T00:00:00+08:00')",
            params![scene_id, story, r#"["景亲王曹元寿","曹元佩"]"#.to_string()],
        )
        .unwrap();
        for cid in [&a, &b] {
            conn.execute(
                "INSERT INTO scene_characters (id, scene_id, character_id, created_at) \
                 VALUES (?1, ?2, ?3, '2026-01-01T00:00:00+08:00')",
                params![uuid::Uuid::new_v4().to_string(), scene_id, cid],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO character_relationships (id, story_id, source_character_id, \
                 target_character_id, relationship_type, created_at) \
                 VALUES (?1, ?2, ?3, ?4, '家人', '2026-01-01T00:00:00+08:00')",
                params![uuid::Uuid::new_v4().to_string(), story, cid, c],
            )
            .unwrap();
        }

        let report = merge_characters(&conn, &story, &b, &a).unwrap();
        assert_eq!(report.scene_link_dups_dropped, 1, "场景关联去重");
        assert_eq!(report.relation_dups_dropped, 1, "关系去重");
        assert_eq!(report.name_tokens_rewritten, 1, "正文名字 token 改写");

        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_characters WHERE scene_id = ?1",
                [&scene_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
        let rels: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id = ?1",
                [&story],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rels, 1);
        let present: String = conn
            .query_row(
                "SELECT characters_present FROM scenes WHERE id = ?1",
                [&scene_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            present.contains("景亲王") && !present.contains("景亲王曹元寿"),
            "{present}"
        );
    }
}
