//! 角色生死状态的**数据层**：列读写与存量回填（SQL 归 db，文本判定归 utils）。
//!
//! v0.64.7 真机事故：《帝国的烟火》第 2 章明成公主被一拳打死（「登时气绝」
//! 「公主尸身移西院」），自动续写到第 10 章又让她走路、说话、夺印——同一场景
//! 里她的尸体还停在门板上。根因：生死只由「章末 1500 字」窗口临时推断，
//! 没有任何持久化。
//!
//! 这里把死亡落成两处事实：
//! - `characters.life_status` / `characters.death_chapter`（V142，权威列）
//! - `character_states.physical_state` 打「已死（第 N 章）」标记，随角色卡
//!   进提示词（角色卡渲染只认 `physical_state`，注入路径无需改结构体）
//!
//! 策略层（何时刷新、如何注入、作者改判）在 `story_system::life_status`。

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::utils::death_text::{dead_names_in_text, strip_editor_markup};

/// 死亡标记词。渲染进角色卡「身体：」一行，兼作幂等判定与 UI 识别。
pub const DEAD_TAG: &str = "已死";

/// 「已死（第2章）」/「已死」（章次未知）。
pub fn dead_marker(chapter: Option<i32>) -> String {
    match chapter {
        Some(n) if n > 0 => format!("{DEAD_TAG}（第{n}章）"),
        _ => DEAD_TAG.to_string(),
    }
}

pub fn has_dead_marker(text: &str) -> bool {
    text.trim_start().starts_with(DEAD_TAG)
}

/// 把死亡标记并进现有身体状态：已有标记则原样保留（幂等），
/// 否则标记在前、原文在后。
pub fn annotate_physical_state(existing: Option<&str>, chapter: Option<i32>) -> String {
    let marker = format!("{}，不得作为活人行动", dead_marker(chapter));
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        Some(rest) if has_dead_marker(rest) => rest.to_string(),
        Some(rest) => format!("{marker}；原状态：{rest}"),
        None => marker,
    }
}

/// 去掉标记前缀，保留其余状态文本（改判存活时用）。
pub fn strip_dead_marker(text: &str) -> String {
    let t = text.trim();
    if !has_dead_marker(t) {
        return t.to_string();
    }
    // 「已死（第2章），不得作为活人行动；原状态：X」→ X
    if let Some(idx) = t.find("原状态：") {
        return t[idx + "原状态：".len()..].trim().to_string();
    }
    String::new()
}

/// 已死角色：name → death_chapter（可能未知）。
pub fn dead_marker_map_on(
    conn: &Connection,
    story_id: &str,
) -> Result<HashMap<String, Option<i32>>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT name, death_chapter FROM characters \
         WHERE story_id = ?1 AND life_status = 'dead'",
    )?;
    let rows = stmt.query_map([story_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<i32>>(1)?))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
}

pub fn alive_names_on(conn: &Connection, story_id: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT name FROM characters WHERE story_id = ?1 \
         AND COALESCE(life_status, 'alive') != 'dead'",
    )?;
    let rows = stmt.query_map([story_id], |r| r.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
}

/// 标记身故（单调：已死不再改；death_chapter 先到先得）。
/// 返回 true = 本次新标记。
pub fn mark_dead_on(
    conn: &Connection,
    story_id: &str,
    name: &str,
    chapter: Option<i32>,
) -> Result<bool, rusqlite::Error> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(false);
    }
    let now = chrono::Local::now().to_rfc3339();
    let updated = conn.execute(
        "UPDATE characters SET life_status = 'dead', \
         death_chapter = COALESCE(death_chapter, ?3), updated_at = ?4 \
         WHERE story_id = ?1 AND name = ?2 AND COALESCE(life_status, 'alive') != 'dead'",
        params![story_id, name, chapter, now],
    )?;
    if updated == 0 {
        return Ok(false);
    }

    // 角色卡路径：把标记并进 physical_state（已死 → 后续状态写回不得清掉）
    let char_id: Option<String> = conn
        .query_row(
            "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2",
            params![story_id, name],
            |r| r.get(0),
        )
        .ok();
    if let Some(char_id) = char_id {
        let existing: Option<String> = conn
            .query_row(
                "SELECT physical_state FROM character_states WHERE character_id = ?1",
                params![char_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        let annotated = annotate_physical_state(existing.as_deref(), chapter);
        let count = conn.execute(
            "UPDATE character_states SET physical_state = ?2, last_updated = ?3 \
             WHERE character_id = ?1",
            params![char_id, annotated, now],
        )?;
        if count == 0 {
            conn.execute(
                "INSERT INTO character_states (id, story_id, character_id, physical_state, \
                 last_updated, updated_at_chapter) \
                 SELECT ?1, ?2, e.id, ?3, ?4, ?5 FROM kg_entities e \
                 WHERE e.id = ?6 AND e.entity_type = 'Character'",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    story_id,
                    annotated,
                    now,
                    chapter,
                    char_id
                ],
            )?;
        }
        // kg 镜像（非权威；权威是 characters 列，读取路径见
        // get_story_characters）
        let _ = conn.execute(
            "UPDATE kg_entities SET attributes = json_set(COALESCE(attributes, '{}'), \
             '$.life_status', 'dead', '$.status', 'Dead'), last_updated = ?2 WHERE id = ?1",
            params![char_id, now],
        );
    }
    Ok(true)
}

/// 作者改判存活（假死/诈死情节）：清列 + 去掉标记。
pub fn revive_on(conn: &Connection, story_id: &str, name: &str) -> Result<usize, rusqlite::Error> {
    let now = chrono::Local::now().to_rfc3339();
    let count = conn.execute(
        "UPDATE characters SET life_status = 'alive', death_chapter = NULL, updated_at = ?3 \
         WHERE story_id = ?1 AND name = ?2 AND life_status = 'dead'",
        params![story_id, name, now],
    )?;
    if count == 0 {
        return Ok(0);
    }
    let char_id: Option<String> = conn
        .query_row(
            "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2",
            params![story_id, name],
            |r| r.get(0),
        )
        .ok();
    if let Some(char_id) = char_id {
        let existing: Option<String> = conn
            .query_row(
                "SELECT physical_state FROM character_states WHERE character_id = ?1",
                params![char_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(existing) = existing {
            let stripped = strip_dead_marker(&existing);
            let value: Option<String> = if stripped.is_empty() {
                None
            } else {
                Some(stripped)
            };
            let _ = conn.execute(
                "UPDATE character_states SET physical_state = ?2, last_updated = ?3 \
                 WHERE character_id = ?1",
                params![char_id, value, now],
            );
        }
        let _ = conn.execute(
            "UPDATE kg_entities SET attributes = json_set(COALESCE(attributes, '{}'), \
             '$.life_status', 'alive'), last_updated = ?2 WHERE id = ?1",
            params![char_id, now],
        );
    }
    Ok(count)
}

/// 存量回填（V142 迁移用）：按章序扫描正文，先命中者即死亡章；
/// 再并入 KG 抽取已判 `status=Dead` 的角色（章次未知）。
///
/// 幂等：已死的角色不再重复处理。
pub fn backfill_story(conn: &Connection, story_id: &str) -> Result<Vec<String>, rusqlite::Error> {
    let names = alive_names_on(conn, story_id)?;
    if names.is_empty() {
        return Ok(Vec::new());
    }

    // 章序正文（scenes.content 挂在该章上；无章挂载的按 sequence_number 兜底）
    let chapters: Vec<(i32, String)> = {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(c.chapter_number, s.sequence_number) AS n, \
             COALESCE(s.content, '') AS content \
             FROM scenes s LEFT JOIN chapters c ON c.id = s.chapter_id \
             WHERE s.story_id = ?1 AND COALESCE(s.content, '') != '' ORDER BY n ASC",
        )?;
        let rows = stmt.query_map([story_id], |r| {
            Ok((r.get::<_, i64>(0)? as i32, r.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut marked = Vec::new();
    let mut still_alive = names;
    for (chapter, raw) in &chapters {
        if still_alive.is_empty() {
            break;
        }
        let text = strip_editor_markup(raw);
        let hits = dead_names_in_text(&still_alive, &text);
        for name in hits {
            if mark_dead_on(conn, story_id, &name, Some(*chapter))? {
                still_alive.retain(|n| n != &name);
                marked.push(name);
            }
        }
    }

    // KG 抽取的死亡信号（正文扫描漏掉时兜底；章次未知）
    let kg_dead: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT name, COALESCE(attributes, '{}') FROM kg_entities \
             WHERE story_id = ?1 AND entity_type = 'Character'",
        )?;
        let rows = stmt.query_map([story_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.filter_map(Result::ok)
            .filter(|(_, attrs)| kg_attributes_say_dead(attrs))
            .map(|(name, _)| name)
            .collect()
    };
    for name in kg_dead {
        if still_alive.iter().any(|n| n == &name) && mark_dead_on(conn, story_id, &name, None)? {
            still_alive.retain(|n| n != &name);
            marked.push(name);
        }
    }

    Ok(marked)
}

/// KG 实体属性是否明确判死（只认 status 字段，避免 mood 里的
/// 「遇刺前」这类表述误判）。
fn kg_attributes_say_dead(attrs_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(attrs_json) else {
        return false;
    };
    let Some(status) = value.get("status").and_then(|v| v.as_str()) else {
        return false;
    };
    let s = status.trim().to_ascii_lowercase();
    s == "dead" || s == "已死" || s == "死亡"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool, repositories::CharacterRepository, CreateCharacterRequest,
        CreateStoryRequest, DbPool,
    };

    fn seed_story(pool: &DbPool) -> String {
        crate::db::StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "生死数据层".into(),
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

    fn seed_character(pool: &DbPool, story_id: &str, name: &str) -> String {
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: name.to_string(),
                ..Default::default()
            })
            .unwrap()
            .id
    }

    #[test]
    fn mark_dead_is_monotone_and_keeps_first_chapter() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        let conn = pool.get().unwrap();

        assert!(mark_dead_on(&conn, &story_id, "明成公主", Some(2)).unwrap());
        assert!(
            !mark_dead_on(&conn, &story_id, "明成公主", Some(9)).unwrap(),
            "二次标记不得重复计数"
        );
        assert_eq!(
            dead_marker_map_on(&conn, &story_id)
                .unwrap()
                .get("明成公主"),
            Some(&Some(2)),
            "死亡章先到先得"
        );
    }

    #[test]
    fn annotate_and_strip_roundtrip() {
        let once = annotate_physical_state(None, Some(2));
        assert_eq!(annotate_physical_state(Some(&once), Some(2)), once);
        let with_state = annotate_physical_state(Some("重伤"), Some(4));
        assert!(with_state.contains("原状态：重伤"), "{with_state}");
        assert_eq!(strip_dead_marker(&with_state), "重伤");
    }

    #[test]
    fn backfill_scans_chapters_in_order_and_uses_kg_signal() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        seed_character(&pool, &story_id, "无名氏");

        let scene_repo = crate::db::SceneRepository::new(pool.clone());
        let scene = scene_repo.create(&story_id, 2, None).unwrap();
        scene_repo
            .update(
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some("明成公主的尸体躺在原处。".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE kg_entities SET attributes = '{\"status\":\"Dead\"}' \
             WHERE story_id = ?1 AND name = '无名氏'",
            [&story_id],
        )
        .unwrap();

        let marked = backfill_story(&conn, &story_id).unwrap();
        assert!(
            marked.contains(&"明成公主".to_string()),
            "marked={marked:?}"
        );
        assert!(marked.contains(&"无名氏".to_string()), "marked={marked:?}");
        let dead = dead_marker_map_on(&conn, &story_id).unwrap();
        assert_eq!(dead.get("明成公主"), Some(&Some(2)));
        assert_eq!(dead.get("无名氏"), Some(&None), "KG 兜底无章次");
        assert!(backfill_story(&conn, &story_id).unwrap().is_empty(), "幂等");
    }

    #[test]
    fn revive_clears_marker_and_allows_second_death() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let char_id = seed_character(&pool, &story_id, "明成公主");
        let conn = pool.get().unwrap();

        mark_dead_on(&conn, &story_id, "明成公主", Some(2)).unwrap();
        assert_eq!(revive_on(&conn, &story_id, "明成公主").unwrap(), 1);
        let state = CharacterRepository::new(pool.clone())
            .get_character_state(&char_id)
            .unwrap()
            .unwrap();
        assert_eq!(state.physical_state, None, "改判后标记应清掉");

        mark_dead_on(&conn, &story_id, "明成公主", Some(7)).unwrap();
        assert_eq!(
            dead_marker_map_on(&conn, &story_id)
                .unwrap()
                .get("明成公主"),
            Some(&Some(7))
        );
    }
}
