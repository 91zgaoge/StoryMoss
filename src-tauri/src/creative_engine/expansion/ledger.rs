//! 轮换账本：场景使用 + 角色沉寂的聚合（零 LLM 成本，纯 SQL/内存计算）。

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use super::{CharacterSilence, RotationLedger, SceneUsage};
use crate::db::connection::DbPool;

/// 使用账统计窗口（近 N 章）
const USAGE_WINDOW: i32 = 10;
/// 渲染时最多列出的场景/角色条数
const RENDER_CAP: usize = 8;

impl RotationLedger {
    pub fn load_sync(pool: &DbPool, story_id: &str) -> Result<Self, String> {
        let conn = pool.get().map_err(|e| e.to_string())?;

        let current: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sequence_number), 0) FROM scenes WHERE story_id = ?1",
                params![story_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if current == 0 {
            return Ok(Self::default());
        }

        // 全量场景（seq, location, characters_present,
        // has_conflict），按章升序。 章节量级百级，行数据小，
        // 内存聚合即可。
        let mut stmt = conn
            .prepare(
                "SELECT sequence_number, COALESCE(setting_location, ''), \
                        COALESCE(characters_present, '[]'), COALESCE(character_conflicts, '[]') \
                 FROM scenes WHERE story_id = ?1 ORDER BY sequence_number ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i32, String, Vec<String>, bool)> = stmt
            .query_map(params![story_id], |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|(seq, loc, present_json, conflicts_json)| {
                let present: Vec<String> = serde_json::from_str(&present_json).unwrap_or_default();
                let conflicts: serde_json::Value =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();
                let has_conflict = conflicts.as_array().map(|a| !a.is_empty()).unwrap_or(false);
                (seq, loc, present, has_conflict)
            })
            .collect();

        // ① 近 USAGE_WINDOW 章场景使用次数（空地点名跳过）
        let mut usage_map: HashMap<String, u32> = HashMap::new();
        for (seq, loc, _, _) in &rows {
            if *seq > current - USAGE_WINDOW && !loc.is_empty() {
                *usage_map.entry(loc.clone()).or_insert(0) += 1;
            }
        }
        let mut scene_usage: Vec<SceneUsage> = usage_map
            .into_iter()
            .map(|(location, count)| SceneUsage { location, count })
            .collect();
        scene_usage.sort_by(|a, b| b.count.cmp(&a.count).then(a.location.cmp(&b.location)));
        scene_usage.truncate(RENDER_CAP);

        // ② 最近一次新地点首次出现的 sequence
        let mut seen_locations: HashSet<&str> = HashSet::new();
        let mut last_new_location_seq = 0;
        for (seq, loc, _, _) in &rows {
            if !loc.is_empty() && seen_locations.insert(loc.as_str()) {
                last_new_location_seq = *seq;
            }
        }

        // ③ 最近一次角色更新（新角色登场，或沉寂 ≥3 章角色回归）的 sequence
        // characters_present 既可能是 id 也可能是名；last_seen 的 key 用 name。
        let mut id_to_name: HashMap<String, String> = HashMap::new();
        {
            let mut char_stmt = conn
                .prepare("SELECT id, name FROM characters WHERE story_id = ?1")
                .map_err(|e| e.to_string())?;
            let rows = char_stmt
                .query_map(params![story_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            for row in rows.filter_map(|r| r.ok()) {
                id_to_name.insert(row.0, row.1);
            }
        }
        let present_name = |item: &str| -> String {
            id_to_name
                .get(item)
                .cloned()
                .unwrap_or_else(|| item.to_string())
        };

        let mut last_character_refresh_seq = 0;
        let mut last_seen: HashMap<String, i32> = HashMap::new();
        for (seq, _, present, _) in &rows {
            for item in present {
                let name = present_name(item);
                match last_seen.get(&name) {
                    None => last_character_refresh_seq = *seq, // 新角色
                    Some(prev) if seq - prev >= 3 => last_character_refresh_seq = *seq, // 回归
                    _ => {}
                }
                last_seen.insert(name, *seq);
            }
        }

        // ④ 角色沉寂账（characters 表全量，按沉寂降序）
        let mut character_silence: Vec<CharacterSilence> = id_to_name
            .iter()
            .map(|(cid, name)| {
                let last = last_seen.get(name).copied().unwrap_or(0);
                let absent = if last == 0 { current } else { current - last };
                CharacterSilence {
                    character_id: cid.clone(),
                    name: name.clone(),
                    chapters_absent: absent.max(0) as u32,
                }
            })
            .filter(|c| c.chapters_absent > 0)
            .collect();
        character_silence.sort_by(|a, b| b.chapters_absent.cmp(&a.chapters_absent));
        character_silence.truncate(RENDER_CAP);

        // ⑤ 尾部连续无冲突场景数
        let trailing_conflict_free =
            rows.iter().rev().take_while(|(_, _, _, has)| !has).count() as u32;

        Ok(RotationLedger {
            current_sequence: current,
            scene_usage,
            character_silence,
            last_new_location_seq,
            last_character_refresh_seq,
            trailing_conflict_free,
        })
    }

    /// 渲染为 prompt 注入段；无任何数据时返回 None（整段省略）
    pub fn render_for_prompt(&self) -> Option<String> {
        if self.current_sequence == 0
            || (self.scene_usage.is_empty() && self.character_silence.is_empty())
        {
            return None;
        }
        let mut out = String::from("【场景与角色轮换账本（调度参考）】\n");
        if !self.scene_usage.is_empty() {
            let usage = self
                .scene_usage
                .iter()
                .map(|u| format!("{}×{}", u.location, u.count))
                .collect::<Vec<_>>()
                .join("、");
            out.push_str(&format!(
                "近 10 章场景使用：{}（优先选择使用次数少或未曾使用过的场景）\n",
                usage
            ));
        }
        if !self.character_silence.is_empty() {
            let silence = self
                .character_silence
                .iter()
                .map(|c| format!("{}（{} 章未登场）", c.name, c.chapters_absent))
                .collect::<Vec<_>>()
                .join("、");
            out.push_str(&format!(
                "角色沉寂：{}（沉寂 ≥3 章的角色建议安排回归或推进其弧光）",
                silence
            ));
        }
        Some(out.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::db::connection::DbPool;

    fn seed_story_with_scenes(pool: &DbPool) -> (String, String, String) {
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let req = crate::db::repositories::CreateStoryRequest {
            title: "账本测试".to_string(),
            description: None,
            genre: Some("玄幻".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let sid = repo.create(req).unwrap().id;
        let conn = pool.get().unwrap();
        // 两个角色：阿岩（每章都在）、林雪（只在第 1 章出现）
        // characters.created_at/updated_at 为 NOT NULL，需显式提供
        conn.execute(
            "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
             VALUES ('c1', ?1, '阿岩', '2026-01-01', '2026-01-01')",
            params![sid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
             VALUES ('c2', ?1, '林雪', '2026-01-01', '2026-01-01')",
            params![sid],
        )
        .unwrap();
        // 5 章：第 1-3 章在练功房，第 4 章开辟山道（新地点），第 5 章回练功房
        // 第 1 章林雪登场（c1+c2），第 2-5 章只有阿岩（c1）
        // 第 4、5 章 character_conflicts 为空（尾部连续 2 章无冲突）
        // scenes.created_at/updated_at 为 NOT NULL，需显式提供
        let scenes = [
            (
                1,
                "练功房",
                "[\"c1\",\"c2\"]",
                "[{\"a\":\"c1\",\"b\":\"c2\",\"nature\":\"师徒\",\"stakes\":\"传承\"}]",
            ),
            (
                2,
                "练功房",
                "[\"c1\"]",
                "[{\"a\":\"c1\",\"b\":\"x\",\"nature\":\"敌对\",\"stakes\":\"生死\"}]",
            ),
            (
                3,
                "练功房",
                "[\"c1\"]",
                "[{\"a\":\"c1\",\"b\":\"x\",\"nature\":\"敌对\",\"stakes\":\"生死\"}]",
            ),
            (4, "山道", "[\"c1\"]", "[]"),
            (5, "练功房", "[\"c1\"]", "[]"),
        ];
        for (seq, loc, present, conflicts) in scenes {
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, setting_location, characters_present, character_conflicts, content, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, '章', ?4, ?5, ?6, '正文', '2026-01-01', '2026-01-01')",
                params![format!("s{}", seq), sid, seq, loc, present, conflicts],
            )
            .unwrap();
        }
        drop(conn);
        (sid, "c1".to_string(), "c2".to_string())
    }

    #[test]
    fn ledger_aggregates_scene_usage_and_character_silence() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let (sid, _c1, c2) = seed_story_with_scenes(&pool);
        let ledger = RotationLedger::load_sync(&pool, &sid).unwrap();

        assert_eq!(ledger.current_sequence, 5);
        // 近 10 章使用账：练功房 4 次、山道 1 次，降序
        assert_eq!(
            ledger.scene_usage,
            vec![
                SceneUsage {
                    location: "练功房".into(),
                    count: 4
                },
                SceneUsage {
                    location: "山道".into(),
                    count: 1
                },
            ]
        );
        // 林雪第 1 章登场后沉寂 4 章；阿岩不沉寂
        assert_eq!(ledger.character_silence.len(), 1);
        assert_eq!(ledger.character_silence[0].character_id, c2);
        assert_eq!(ledger.character_silence[0].chapters_absent, 4);
        // 最近一次新地点 = 第 4 章（山道首次出现）
        assert_eq!(ledger.last_new_location_seq, 4);
        // 最近一次角色更新 = 第 1 章（林雪此后再未回归/无新角色）
        assert_eq!(ledger.last_character_refresh_seq, 1);
        // 尾部连续 2 章无冲突
        assert_eq!(ledger.trailing_conflict_free, 2);
    }

    #[test]
    fn ledger_matches_character_present_by_name() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let sid = repo
            .create(crate::db::repositories::CreateStoryRequest {
                title: "按名匹配出场".to_string(),
                description: None,
                genre: Some("玄幻".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id;
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
             VALUES ('c-lin', ?1, '林雪', '2026-01-01', '2026-01-01')",
            params![sid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, setting_location, characters_present, character_conflicts, content, created_at, updated_at) \
             VALUES ('sc1', ?1, 1, '章', '客栈', '[\"林雪\"]', '[]', '正文', '2026-01-01', '2026-01-01')",
            params![sid],
        )
        .unwrap();
        drop(conn);
        let ledger = RotationLedger::load_sync(&pool, &sid).unwrap();
        let lin = ledger.character_silence.iter().find(|c| c.name == "林雪");
        assert!(
            lin.is_none() || lin.unwrap().chapters_absent == 0,
            "林雪在场，last_seen 应按名匹配而非 id，不得被当成从未登场"
        );
        assert_eq!(
            ledger.last_character_refresh_seq, 1,
            "按名匹配时应把林雪第 1 章登场记为角色更新"
        );
    }

    #[test]
    fn empty_story_yields_default_ledger_and_no_prompt_text() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let req = crate::db::repositories::CreateStoryRequest {
            title: "空书".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let sid = repo.create(req).unwrap().id;
        let ledger = RotationLedger::load_sync(&pool, &sid).unwrap();
        assert_eq!(ledger.current_sequence, 0);
        assert!(ledger.render_for_prompt().is_none());
    }

    #[test]
    fn render_for_prompt_lists_usage_and_silence() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let (sid, _, _) = seed_story_with_scenes(&pool);
        let ledger = RotationLedger::load_sync(&pool, &sid).unwrap();
        let text = ledger.render_for_prompt().unwrap();
        assert!(text.contains("练功房×4"));
        assert!(text.contains("山道×1"));
        assert!(text.contains("林雪"));
        assert!(text.contains("4 章未登场"));
    }
}
