//! 资产落库：把黑板资产区的条目物化到应用资产表（characters/world_buildings/
//! story_outlines/foreshadowing_tracker/character_relationships）。 character
//! 条目 content 须为 JSON（{"name","background","personality","goals"} +
//! 可选 emotional_core/trigger/wound/need）； relationship 条目 content 为
//! SeedRelationship JSON 数组（兼容单对象）； world/outline 条目 content
//! 为纯文本； foreshadowing 条目 content 为纯文本或 JSON（数组/对象，
//! 经 normalize_foreshadowing 逐条归一化）。item_type 做别名归一化
//! （worldbuilding/world_building→world，story_outline→outline，
//! emotional_relationship/bond→relationship），兼容本地
//! 模型变体。解析失败的条目跳过并 log::warn!。

use rusqlite::params;

use crate::{
    agency::{
        models::BoardItem,
        prose_ground::{has_substantial_prose, name_in_prose, outline_is_grounded},
    },
    db::DbPool,
};

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

fn load_story_prose(conn: &rusqlite::Connection, story_id: &str) -> String {
    let mut stmt = match conn.prepare(
        "SELECT COALESCE(content, '') FROM scenes WHERE story_id = ?1 ORDER BY sequence_number",
    ) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let rows = match stmt.query_map(params![story_id], |r| r.get::<_, String>(0)) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    let mut out = String::new();
    for row in rows.flatten() {
        out.push_str(&row);
        out.push('\n');
    }
    out
}

fn character_name_from_item(item: &BoardItem) -> Option<String> {
    let parsed = crate::agency::coordinator::parse_lenient::<serde_json::Value>(&item.content)?;
    let name = parsed.get("name").and_then(|x| x.as_str()).unwrap_or("");
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn load_registered_names(conn: &rusqlite::Connection, story_id: &str) -> Vec<String> {
    let mut stmt = match conn.prepare("SELECT name FROM characters WHERE story_id = ?1") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(params![story_id], |r| r.get::<_, String>(0)) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    rows.flatten().collect()
}

pub(crate) fn concat_story_prose(pool: &DbPool, story_id: &str) -> String {
    let Ok(conn) = pool.get() else {
        return String::new();
    };
    load_story_prose(&conn, story_id)
}

/// 按角色名查 characters 表 id（relationship 落库时解析 source/target）。
fn find_character_id_by_name(
    conn: &rusqlite::Connection,
    story_id: &str,
    name: &str,
) -> Option<String> {
    conn.query_row(
        "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2 LIMIT 1",
        params![story_id, name],
        |r| r.get(0),
    )
    .ok()
}

fn find_existing_relationship_id(
    conn: &rusqlite::Connection,
    story_id: &str,
    source_id: &str,
    target_id: &str,
) -> Option<String> {
    conn.query_row(
        "SELECT id FROM character_relationships WHERE story_id = ?1 AND (\
         (source_character_id = ?2 AND target_character_id = ?3) OR \
         (source_character_id = ?3 AND target_character_id = ?2)) LIMIT 1",
        params![story_id, source_id, target_id],
        |r| r.get(0),
    )
    .ok()
}

/// 缺则建、有则改（含反向同一对）。管理 Agent 物化关系的唯一入口。
pub(crate) fn upsert_seed_relationship(
    conn: &rusqlite::Connection,
    story_id: &str,
    rel: &crate::agency::coordinator::SeedRelationship,
) -> usize {
    let source_id = find_character_id_by_name(conn, story_id, rel.source.trim());
    let target_id = find_character_id_by_name(conn, story_id, rel.target.trim());
    let (Some(sid), Some(tid)) = (source_id, target_id) else {
        log::warn!(
            "materialize: 关系 {} -> {} 找不到角色，跳过",
            rel.source,
            rel.target
        );
        return 0;
    };
    let ty = if rel.relationship_type.trim().is_empty() {
        "关系"
    } else {
        rel.relationship_type.trim()
    };
    let desc = if rel.description.trim().is_empty() {
        None
    } else {
        Some(rel.description.trim())
    };
    let bond = if rel.emotional_bond.trim().is_empty() {
        None
    } else {
        Some(rel.emotional_bond.trim())
    };
    let rev = if rel.reverse_emotional_bond.trim().is_empty() {
        None
    } else {
        Some(rel.reverse_emotional_bond.trim())
    };
    if let Some(id) = find_existing_relationship_id(conn, story_id, &sid, &tid) {
        match conn.execute(
            "UPDATE character_relationships SET relationship_type = ?2, description = ?3, \
             emotional_bond = ?4, emotional_intensity = ?5, reverse_emotional_bond = ?6, \
             reverse_emotional_intensity = ?7 WHERE id = ?1",
            params![
                id,
                ty,
                desc,
                bond,
                rel.emotional_intensity,
                rev,
                rel.reverse_emotional_intensity
            ],
        ) {
            Ok(_) => 1,
            Err(e) => {
                log::warn!("materialize: 更新关系失败: {}", e);
                0
            }
        }
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let ts = now();
        match conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, description, emotional_bond, \
             emotional_intensity, reverse_emotional_bond, reverse_emotional_intensity, \
             created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                id,
                story_id,
                sid,
                tid,
                ty,
                desc,
                bond,
                rel.emotional_intensity,
                rev,
                rel.reverse_emotional_intensity,
                ts
            ],
        ) {
            Ok(n) => n,
            Err(e) => {
                log::warn!("materialize: 写入关系失败: {}", e);
                0
            }
        }
    }
}

pub(crate) fn parse_seed_relationships(
    content: &str,
) -> Vec<crate::agency::coordinator::SeedRelationship> {
    use crate::agency::coordinator::SeedRelationship;
    match crate::agency::coordinator::parse_lenient::<Vec<SeedRelationship>>(content) {
        Some(v) => v,
        None => crate::agency::coordinator::parse_lenient::<SeedRelationship>(content)
            .into_iter()
            .collect(),
    }
}

pub fn materialize_assets(pool: &DbPool, story_id: &str, items: &[BoardItem]) -> usize {
    let mut count = 0usize;
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("materialize_assets: pool 获取失败: {}", e);
            return 0;
        }
    };
    let prose = load_story_prose(&conn, story_id);
    let gate = has_substantial_prose(&prose);
    let mut candidates: Vec<String> = load_registered_names(&conn, story_id);
    if gate {
        for item in items.iter().filter(|i| i.status == "active") {
            let nt = match item.item_type.as_str() {
                "worldbuilding" | "world_building" => "world",
                "story_outline" => "outline",
                "emotional_relationship" | "bond" => "relationship",
                other => other,
            };
            if nt == "character" {
                if let Some(n) = character_name_from_item(item) {
                    if !candidates.iter().any(|c| c == &n) {
                        candidates.push(n);
                    }
                }
            }
        }
    }
    for item in items.iter().filter(|i| i.status == "active") {
        // item_type 别名归一化：兼容本地模型经 board_write 写入的变体
        let normalized_type = match item.item_type.as_str() {
            "worldbuilding" | "world_building" => "world",
            "story_outline" => "outline",
            "emotional_relationship" | "bond" => "relationship",
            other => other,
        };
        match normalized_type {
            "character" => {
                let parsed =
                    crate::agency::coordinator::parse_lenient::<serde_json::Value>(&item.content);
                let (
                    name,
                    background,
                    personality,
                    goals,
                    emo_core,
                    emo_trigger,
                    emo_wound,
                    emo_need,
                ) = match parsed.as_ref() {
                    Some(v) => (
                        v.get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("background")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("personality")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("goals")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("emotional_core")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("emotional_trigger")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("emotional_wound")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        v.get("emotional_need")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    ),
                    None => {
                        log::warn!("materialize: 角色条目 {} 非 JSON，跳过", item.key);
                        continue;
                    }
                };
                if name.is_empty() {
                    log::warn!("materialize: 角色条目 {} 缺 name，跳过", item.key);
                    continue;
                }
                if gate && !name_in_prose(&name, &prose) {
                    log::warn!("materialize: 角色「{}」未出现在已有正文，跳过落库", name);
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                // story_id+name upsert：已存在同名角色时刷新字段（创世重跑/资产
                // 补齐场景），否则插入新行。情感属性同步写入 characters 表。
                let updated = conn.execute(
                    "UPDATE characters SET background = ?3, personality = ?4, goals = ?5, \
                     emotional_core = ?6, emotional_trigger = ?7, emotional_wound = ?8, emotional_need = ?9, \
                     updated_at = ?10
                     WHERE story_id = ?1 AND name = ?2",
                    params![story_id, name, background, personality, goals,
                            emo_core, emo_trigger, emo_wound, emo_need, ts],
                );
                match updated {
                    Ok(n) if n > 0 => count += 1,
                    Ok(_) => match conn.execute(
                        "INSERT INTO characters (id, story_id, name, background, personality, goals, \
                         emotional_core, emotional_trigger, emotional_wound, emotional_need, \
                         source, is_auto_generated, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'agency', 1, ?11, ?12)",
                        params![id, story_id, name, background, personality, goals,
                                emo_core, emo_trigger, emo_wound, emo_need, ts, ts],
                    ) {
                        Ok(n) => count += n,
                        Err(e) => log::warn!("materialize: 插入角色失败: {}", e),
                    },
                    Err(e) => log::warn!("materialize: 更新角色失败: {}", e),
                }
            }
            "world" => {
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO world_buildings (id, story_id, concept, rules, source, is_auto_generated, created_at, updated_at)
                     VALUES (?1, ?2, ?3, '[]', 'agency', 1, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET concept = excluded.concept, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入世界观失败: {}", e),
                }
            }
            "outline" => {
                if gate && !outline_is_grounded(&item.content, &prose, &candidates) {
                    log::warn!("materialize: 大纲未接地（含正文未出现的角色名），跳过落库");
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                let ts = now();
                let result = conn.execute(
                    "INSERT INTO story_outlines (id, story_id, content, act_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 3, ?4, ?5)
                     ON CONFLICT(story_id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
                    params![id, story_id, item.content, ts, ts],
                );
                match result {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("materialize: 写入大纲失败: {}", e),
                }
            }
            "foreshadowing" => {
                // content 可能是纯文本（单条），也可能是 JSON 数组/对象
                // （本地模型经 board_write
                // 写入的变体），统一归一化为若干条文本。
                // 先试严格 JSON（覆盖数组形态），再退回 parse_lenient（花括号
                // 截取只覆盖对象形态），最后按纯文本处理。
                let trimmed = item.content.trim();
                let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
                    .ok()
                    .or_else(|| {
                        crate::agency::coordinator::parse_lenient::<serde_json::Value>(trimmed)
                    });
                let texts: Vec<String> = match parsed {
                    Some(serde_json::Value::Array(arr)) => arr
                        .iter()
                        .map(crate::agency::coordinator::normalize_foreshadowing)
                        .collect(),
                    Some(v) => vec![crate::agency::coordinator::normalize_foreshadowing(&v)],
                    None => vec![item.content.clone()],
                };
                for text in texts {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    let id = uuid::Uuid::new_v4().to_string();
                    let ts = now();
                    // story_id+content 去重：同一伏笔重复物化时跳过
                    match conn.execute(
                        "INSERT INTO foreshadowing_tracker (id, story_id, content, setup_scene_id, status, importance, created_at)
                         SELECT ?1, ?2, ?3, NULL, 'setup', 5, ?4
                         WHERE NOT EXISTS (SELECT 1 FROM foreshadowing_tracker WHERE story_id = ?2 AND content = ?3)",
                        params![id, story_id, text, ts],
                    ) {
                        Ok(n) => count += n,
                        Err(e) => log::warn!("materialize: 写入伏笔失败: {}", e),
                    }
                }
            }
            "relationship" => continue,
            _ => {}
        }
    }
    for item in items.iter().filter(|i| i.status == "active") {
        let normalized_type = match item.item_type.as_str() {
            "emotional_relationship" | "bond" => "relationship",
            other => other,
        };
        if normalized_type != "relationship" {
            continue;
        }
        let rels = parse_seed_relationships(&item.content);
        if rels.is_empty() {
            log::warn!("materialize: 关系条目 {} 非 JSON，跳过", item.key);
            continue;
        }
        for rel in rels {
            count += upsert_seed_relationship(&conn, story_id, &rel);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::models::*,
        db::{create_test_pool, dto::CreateStoryRequest, repositories::StoryRepository},
    };

    fn story(pool: &crate::db::DbPool, id: &str) {
        let s = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "测试书".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        // create 生成自己的 id；测试统一改用其返回值
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE stories SET id = ?1 WHERE id = ?2",
            rusqlite::params![id, s.id],
        )
        .unwrap();
    }

    fn item(item_type: &str, key: &str, content: &str) -> BoardItem {
        BoardItem::new(
            "r1",
            "s1",
            BoardZone::Asset,
            item_type,
            key,
            content,
            "摘要",
            AgentRole::Producer,
            "active",
        )
    }

    #[test]
    fn test_materialize_character_json() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item(
            "character",
            "主角",
            r#"{"name":"阿苔","background":"拾荒者","personality":"坚韧","goals":"找到星环"}"#,
        )];
        let n = materialize_assets(&pool, "s1", &items);
        assert_eq!(n, 1);
        let conn = pool.get().unwrap();
        let name: String = conn
            .query_row("SELECT name FROM characters WHERE story_id='s1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "阿苔");
        // 重复物化（story_id+name upsert）：刷新字段，仍一行
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        // upsert 应刷新字段而非跳过
        let items2 = vec![item(
            "character",
            "主角",
            r#"{"name":"阿苔","background":"拾荒者出身·已刷新","personality":"坚韧","goals":"找到星环"}"#,
        )];
        assert_eq!(materialize_assets(&pool, "s1", &items2), 1);
        let bg: String = conn
            .query_row(
                "SELECT background FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(bg.contains("已刷新"));
    }

    fn seed_su_prose(pool: &crate::db::DbPool, story_id: &str) {
        use crate::db::repositories::SceneRepository;
        let scene = SceneRepository::new(pool.clone())
            .create(story_id, 1, Some("第一章"))
            .unwrap();
        let mut prose = "知启纪元八百四十七年。大奉帝国西北边陲重镇，黑崎州城。\
第二代镇北王苏会山端坐大堂。大少爷苏亦铁红装肃立。"
            .to_string();
        while prose.chars().count() < 200 {
            prose.push_str("镇北王府大堂里红毡铺地，黑卫军肃立。");
        }
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE scenes SET content = ?1 WHERE id = ?2",
            rusqlite::params![prose, scene.id],
        )
        .unwrap();
    }

    #[test]
    fn test_materialize_drops_names_absent_from_prose() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        seed_su_prose(&pool, "s1");
        let items = vec![
            item(
                "character",
                "苏",
                r#"{"name":"苏会山","background":"镇北王","personality":"刚","goals":"护边"}"#,
            ),
            item(
                "character",
                "费",
                r#"{"name":"费迪南三世","background":"皇帝","personality":"疑","goals":"烟火节"}"#,
            ),
        ];
        let n = materialize_assets(&pool, "s1", &items);
        assert_eq!(n, 1);
        let conn = pool.get().unwrap();
        let names: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM characters WHERE story_id='s1' ORDER BY name")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert_eq!(names, vec!["苏会山".to_string()]);
    }

    #[test]
    fn test_materialize_without_prose_keeps_genesis_names() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![
            item(
                "character",
                "苏",
                r#"{"name":"苏会山","background":"镇北王","personality":"刚","goals":"护边"}"#,
            ),
            item(
                "character",
                "费",
                r#"{"name":"费迪南三世","background":"皇帝","personality":"疑","goals":"烟火节"}"#,
            ),
        ];
        assert_eq!(materialize_assets(&pool, "s1", &items), 2);
    }

    #[test]
    fn test_materialize_skips_ungrounded_outline_when_prose_exists() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        seed_su_prose(&pool, "s1");
        let items = vec![
            item(
                "character",
                "费",
                r#"{"name":"费迪南三世","background":"皇帝","personality":"疑","goals":"烟火节"}"#,
            ),
            item(
                "outline",
                "大纲",
                "第一卷·灰烬低语。费迪南三世为撑烟火节加征火药税。",
            ),
        ];
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
        let conn = pool.get().unwrap();
        let outlines: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outlines, 0);
    }

    #[test]
    fn test_materialize_world_upsert_idempotent() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("world", "世界观", "双星废土，磁力风暴")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        // 再次执行不报错（upsert），仍一行
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_materialize_skips_non_json_character() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("character", "主角", "自由文本不是 JSON")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_materialize_outline() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item("outline", "第一卷", "第一卷大纲：起承转合……")];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(content.contains("起承转合"));
    }

    #[test]
    fn test_materialize_foreshadowing_text_and_json_array() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![
            item("foreshadowing", "伏笔1", "妹妹的项链（第三卷回收）"),
            item(
                "foreshadowing",
                "伏笔清单",
                r#"["身世之谜",{"description":"星环秘密"},{"text":"远古文字"}]"#,
            ),
        ];
        assert_eq!(materialize_assets(&pool, "s1", &items), 4);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM foreshadowing_tracker WHERE story_id='s1' AND status='setup'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 4);
        // 重复物化按 story_id+content 去重
        assert_eq!(materialize_assets(&pool, "s1", &items), 0);
    }

    #[test]
    fn test_materialize_item_type_alias() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![
            item("worldbuilding", "世界观", "双星废土"),
            item("story_outline", "大纲", "整本书大纲"),
        ];
        assert_eq!(materialize_assets(&pool, "s1", &items), 2);
        let conn = pool.get().unwrap();
        let world: String = conn
            .query_row(
                "SELECT concept FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(world, "双星废土");
        let outline: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outline, "整本书大纲");
    }

    #[test]
    fn test_materialize_character_with_emotional_attrs() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![item(
            "character",
            "主角",
            r#"{"name":"阿苔","background":"拾荒者","personality":"坚韧","goals":"找到星环",
               "emotional_core":"压抑的愤怒","emotional_trigger":"被背叛时暴怒",
               "emotional_wound":"目睹母亲惨死","emotional_need":"被认可"}"#,
        )];
        assert_eq!(materialize_assets(&pool, "s1", &items), 1);
        let conn = pool.get().unwrap();
        let core: String = conn
            .query_row(
                "SELECT emotional_core FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(core, "压抑的愤怒");
        let wound: String = conn
            .query_row(
                "SELECT emotional_wound FROM characters WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(wound, "目睹母亲惨死");
    }

    #[test]
    fn test_materialize_relationship_with_emotional_bond() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![
            item(
                "character",
                "甲",
                r#"{"name":"甲","background":"","personality":"","goals":""}"#,
            ),
            item(
                "character",
                "乙",
                r#"{"name":"乙","background":"","personality":"","goals":""}"#,
            ),
            item(
                "relationship",
                "关系",
                r#"[{"source":"甲","target":"乙","relationship_type":"师徒",
               "emotional_bond":"欺骗","emotional_intensity":0.9,
               "reverse_emotional_bond":"崇拜","reverse_emotional_intensity":0.7,
               "description":"面和心不和"}]"#,
            ),
        ];
        materialize_assets(&pool, "s1", &items);
        let conn = pool.get().unwrap();
        let bond: String = conn
            .query_row(
                "SELECT emotional_bond FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bond, "欺骗");
        let rev_bond: String = conn
            .query_row(
                "SELECT reverse_emotional_bond FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rev_bond, "崇拜");
    }

    #[test]
    fn test_materialize_relationship_after_characters_even_if_listed_first() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let items = vec![
            item(
                "relationship",
                "关系",
                r#"{"source":"甲","target":"乙","relationship_type":"夫妻",
               "emotional_bond":"爱","emotional_intensity":0.8,
               "reverse_emotional_bond":"爱","reverse_emotional_intensity":0.8,
               "description":"并坐"}"#,
            ),
            item(
                "character",
                "甲",
                r#"{"name":"甲","background":"","personality":"","goals":""}"#,
            ),
            item(
                "character",
                "乙",
                r#"{"name":"乙","background":"","personality":"","goals":""}"#,
            ),
        ];
        let n = materialize_assets(&pool, "s1", &items);
        assert!(n >= 3, "角色+关系都应落库 n={n}");
        let conn = pool.get().unwrap();
        let ty: String = conn
            .query_row(
                "SELECT relationship_type FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ty, "夫妻");
    }

    #[test]
    fn test_materialize_relationship_updates_existing_pair() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let chars = vec![
            item(
                "character",
                "甲",
                r#"{"name":"甲","background":"","personality":"","goals":""}"#,
            ),
            item(
                "character",
                "乙",
                r#"{"name":"乙","background":"","personality":"","goals":""}"#,
            ),
        ];
        materialize_assets(&pool, "s1", &chars);
        let first = vec![item(
            "relationship",
            "关系",
            r#"{"source":"甲","target":"乙","relationship_type":"侄子",
               "emotional_bond":"怜","emotional_intensity":0.4,
               "reverse_emotional_bond":"","reverse_emotional_intensity":0.5,
               "description":"脏"}"#,
        )];
        assert_eq!(materialize_assets(&pool, "s1", &first), 1);
        let second = vec![item(
            "relationship",
            "关系",
            r#"{"source":"乙","target":"甲","relationship_type":"父子",
               "emotional_bond":"悲愤","emotional_intensity":0.9,
               "reverse_emotional_bond":"庇护","reverse_emotional_intensity":0.8,
               "description":"近文锁定"}"#,
        )];
        assert_eq!(materialize_assets(&pool, "s1", &second), 1);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "同一对只保留一行");
        let (ty, bond, desc): (String, String, String) = conn
            .query_row(
                "SELECT relationship_type, emotional_bond, description FROM character_relationships WHERE story_id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(ty, "父子");
        assert_eq!(bond, "悲愤");
        assert!(desc.contains("近文锁定"), "desc={desc}");
    }
}
