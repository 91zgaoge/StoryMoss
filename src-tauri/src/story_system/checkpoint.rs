#![allow(dead_code)]
//! 故事检查点与「某章时点」回溯（P3-C，v0.63.0）。
//!
//! - **检查点**：每 10 章（与段摘要同频）写一份连续性资产快照 （角色状态 /
//!   物品归属 / 未回收伏笔 / 段摘要 / 终局指南针）， 供事故恢复与回溯比对。
//! - **时间旅行查询**：给定章节 N，返回「截至第 N 章」的确定性视图——
//!   角色已知信息来自 `character_knowledge_log`（append-only），
//!   世界真相是否已揭示来自 `story_timeline_events.reveal_chapter`，
//!   物品归属是当前账本（表本身非追加式，结果中明确标注 current_only）。

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoryCheckpoint {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterKnowledgeAsOf {
    pub name: String,
    /// 截至该章已知（来自知情流水）
    pub known: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineEventAsOf {
    pub objective_fact: String,
    pub reveal_status: String,
    pub reveal_chapter: Option<i32>,
    /// 截至该章是否已向读者揭示
    pub revealed_by_now: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HoldingAsOf {
    pub item: String,
    pub holder: Option<String>,
    pub status: String,
    /// 物品账本非追加式，只能给当前值（调用方应据此提示）
    pub current_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsOfView {
    pub chapter_number: i32,
    pub character_knowledge: Vec<CharacterKnowledgeAsOf>,
    pub timeline_events: Vec<TimelineEventAsOf>,
    pub holdings: Vec<HoldingAsOf>,
    /// 不晚于该章的最新检查点章号（若有）
    pub latest_checkpoint_chapter: Option<i32>,
}

/// 写入/覆盖某章检查点（幂等：同 story+chapter 只保留一份）。
pub fn write_checkpoint(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
) -> Result<(), rusqlite::Error> {
    let snapshot = build_snapshot(pool, story_id, chapter_number);
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO story_checkpoints (id, story_id, chapter_number, snapshot_json, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(story_id, chapter_number) DO UPDATE SET \
          snapshot_json = excluded.snapshot_json, created_at = excluded.created_at",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            chapter_number,
            snapshot,
            now
        ],
    )?;
    Ok(())
}

/// 组装快照 JSON（角色状态 / 物品归属 / 未回收伏笔 / 段摘要 / 指南针）。
fn build_snapshot(pool: &DbPool, story_id: &str, chapter_number: i32) -> String {
    let Ok(conn) = pool.get() else {
        return "{}".to_string();
    };

    let mut character_states: Vec<serde_json::Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT c.name, cs.current_location, cs.current_emotion, cs.active_goal, \
                COALESCE(cs.secrets_known, '[]'), COALESCE(cs.secrets_unknown, '[]') \
         FROM characters c LEFT JOIN character_states cs ON cs.character_id = c.id \
         WHERE c.story_id = ?1 ORDER BY c.name LIMIT 50",
    ) {
        if let Ok(rows) = stmt.query_map(params![story_id], |row| {
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "location": row.get::<_, Option<String>>(1)?,
                "emotion": row.get::<_, Option<String>>(2)?,
                "goal": row.get::<_, Option<String>>(3)?,
                "secrets_known": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(4)?).unwrap_or(serde_json::json!([])),
                "secrets_unknown": serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(5)?).unwrap_or(serde_json::json!([])),
            }))
        }) {
            character_states = rows.flatten().collect();
        }
    }

    let holdings: Vec<serde_json::Value> =
        crate::memory::continuity::load_item_holdings(pool, story_id)
            .into_iter()
            .map(|h| {
                serde_json::json!({
                    "item": h.item_name,
                    "holder": h.holder_name,
                    "status": h.status,
                })
            })
            .collect();

    let mut foreshadowings: Vec<String> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT content FROM foreshadowing_tracker \
         WHERE story_id = ?1 AND status = 'setup' ORDER BY importance DESC LIMIT 20",
    ) {
        if let Ok(rows) = stmt.query_map(params![story_id], |row| row.get::<_, String>(0)) {
            foreshadowings = rows.flatten().collect();
        }
    }

    let segments: Vec<serde_json::Value> =
        crate::story_system::segment_summary::load_segment_summaries(pool, story_id)
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "index": s.segment_index,
                    "range": [s.start_chapter, s.end_chapter],
                    "summary": s.summary,
                })
            })
            .collect();
    let book_summary =
        crate::story_system::segment_summary::load_book_summary(pool, story_id).map(|s| s.summary);

    serde_json::json!({
        "chapter_number": chapter_number,
        "character_states": character_states,
        "holdings": holdings,
        "unresolved_foreshadowings": foreshadowings,
        "segments": segments,
        "book_summary": book_summary,
        "captured_at": Local::now().to_rfc3339(),
    })
    .to_string()
}

/// 列出检查点（章号降序）。
pub fn load_checkpoints(pool: &DbPool, story_id: &str, limit: i64) -> Vec<StoryCheckpoint> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT id, story_id, chapter_number, snapshot_json, created_at \
         FROM story_checkpoints WHERE story_id = ?1 ORDER BY chapter_number DESC LIMIT ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, limit], |row| {
        Ok(StoryCheckpoint {
            id: row.get(0)?,
            story_id: row.get(1)?,
            chapter_number: row.get(2)?,
            snapshot_json: row.get(3)?,
            created_at: row.get(4)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 「截至第 N 章」的确定性视图。
pub fn query_as_of(pool: &DbPool, story_id: &str, chapter_number: i32) -> AsOfView {
    let mut view = AsOfView {
        chapter_number,
        character_knowledge: Vec::new(),
        timeline_events: Vec::new(),
        holdings: Vec::new(),
        latest_checkpoint_chapter: None,
    };
    let Ok(conn) = pool.get() else {
        return view;
    };

    // 1) 角色已知（append-only 知情流水，取 chapter_number <= N 或未标章号的）
    if let Ok(mut stmt) = conn.prepare(
        "SELECT c.name, kl.fact FROM character_knowledge_log kl \
         JOIN characters c ON c.id = kl.character_id \
         WHERE kl.story_id = ?1 AND kl.change_type = 'learned' \
           AND (kl.chapter_number IS NULL OR kl.chapter_number <= ?2) \
         ORDER BY c.name, kl.chapter_number, kl.created_at",
    ) {
        if let Ok(rows) = stmt.query_map(params![story_id, chapter_number], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            let mut grouped: Vec<CharacterKnowledgeAsOf> = Vec::new();
            for (name, fact) in rows.flatten() {
                match grouped.iter_mut().find(|g| g.name == name) {
                    Some(entry) => {
                        if !entry.known.contains(&fact) {
                            entry.known.push(fact);
                        }
                    }
                    None => grouped.push(CharacterKnowledgeAsOf {
                        name,
                        known: vec![fact],
                    }),
                }
            }
            view.character_knowledge = grouped;
        }
    }

    // 2) 世界真相是否截至该章已揭示
    if let Ok(mut stmt) = conn.prepare(
        "SELECT objective_fact, reveal_status, reveal_chapter FROM story_timeline_events \
         WHERE story_id = ?1 ORDER BY sequence_number ASC LIMIT 100",
    ) {
        if let Ok(rows) = stmt.query_map(params![story_id], |row| {
            let fact: String = row.get(0)?;
            let status: String = row.get(1)?;
            let reveal_chapter: Option<i32> = row.get(2)?;
            Ok(TimelineEventAsOf {
                objective_fact: fact,
                revealed_by_now: reveal_chapter.map(|c| c <= chapter_number).unwrap_or(false),
                reveal_status: status,
                reveal_chapter,
            })
        }) {
            view.timeline_events = rows.flatten().collect();
        }
    }

    // 3) 物品归属（当前值，标注 current_only）
    view.holdings = crate::memory::continuity::load_item_holdings(pool, story_id)
        .into_iter()
        .map(|h| HoldingAsOf {
            item: h.item_name,
            holder: h.holder_name,
            status: h.status,
            current_only: true,
        })
        .collect();

    // 4) 不晚于该章的最新检查点
    view.latest_checkpoint_chapter = conn
        .query_row(
            "SELECT MAX(chapter_number) FROM story_checkpoints \
             WHERE story_id = ?1 AND chapter_number <= ?2",
            params![story_id, chapter_number],
            |row| row.get::<_, Option<i32>>(0),
        )
        .unwrap_or(None);

    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::connection::create_test_pool,
        memory::continuity::{self, KnowledgeUpdate, TimelineEventDelta},
    };

    fn seed_story_with_character(pool: &DbPool) -> (String, String) {
        let story_id = uuid::Uuid::new_v4().to_string();
        let character_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '回溯', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
             VALUES (?1, ?2, '徐棠', ?3, ?3)",
            params![&character_id, &story_id, &now],
        )
        .unwrap();
        (story_id, character_id)
    }

    fn learn(pool: &DbPool, story_id: &str, chapter: i32, fact: &str) {
        // 直接写知情流水（模拟第 chapter 章发生的信息流）
        let conn = pool.get().unwrap();
        let character_id: String = conn
            .query_row(
                "SELECT id FROM characters WHERE story_id = ?1 LIMIT 1",
                params![story_id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO character_knowledge_log \
             (id, story_id, character_id, fact, change_type, chapter_number, created_at) \
             VALUES (?1, ?2, ?3, ?4, 'learned', ?5, ?6)",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                character_id,
                fact,
                chapter,
                Local::now().to_rfc3339()
            ],
        )
        .unwrap();
    }

    #[test]
    fn as_of_filters_knowledge_and_reveals_by_chapter() {
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_with_character(&pool);
        learn(&pool, &story_id, 3, "这封信是哥哥寄的");
        learn(&pool, &story_id, 9, "将军知道密道");
        continuity::persist_timeline_events(
            &pool,
            &story_id,
            None,
            Some(4),
            &[TimelineEventDelta {
                objective_fact: "密道入口在佛堂".into(),
                reader_knowledge: "读者已知密道存在".into(),
                reveal_status: "revealed".into(),
                participants: vec![],
                evidence: String::new(),
            }],
        )
        .unwrap();

        // 第 5 章时点：只应看到第 3 章的知识，密道真相已揭示（第 4 章）
        let view = query_as_of(&pool, &story_id, 5);
        assert_eq!(view.character_knowledge.len(), 1);
        assert_eq!(view.character_knowledge[0].known, vec!["这封信是哥哥寄的"]);
        assert!(view.timeline_events[0].revealed_by_now);

        // 第 2 章时点：什么都没有
        let early = query_as_of(&pool, &story_id, 2);
        assert!(early.character_knowledge.is_empty());
        assert!(!early.timeline_events[0].revealed_by_now);

        // 第 10 章时点：两条知识都在
        let late = query_as_of(&pool, &story_id, 10);
        assert_eq!(late.character_knowledge[0].known.len(), 2);
    }

    #[test]
    fn checkpoint_upsert_and_lookup_by_chapter() {
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_with_character(&pool);
        continuity::persist_item_holdings(
            &pool,
            &story_id,
            None,
            Some(1),
            &[continuity::ItemHoldingDelta {
                item: "羊脂玉佩".into(),
                holder: "徐棠".into(),
                action: "acquire".into(),
                evidence: String::new(),
            }],
        )
        .unwrap();
        write_checkpoint(&pool, &story_id, 10).unwrap();
        write_checkpoint(&pool, &story_id, 10).unwrap();
        write_checkpoint(&pool, &story_id, 20).unwrap();

        let checkpoints = load_checkpoints(&pool, &story_id, 10);
        assert_eq!(checkpoints.len(), 2, "同章应幂等: {checkpoints:?}");
        let snapshot: serde_json::Value =
            serde_json::from_str(&checkpoints[0].snapshot_json).unwrap();
        assert_eq!(snapshot["chapter_number"], 20);
        assert!(snapshot["holdings"].as_array().unwrap().len() == 1);

        let view = query_as_of(&pool, &story_id, 15);
        assert_eq!(view.latest_checkpoint_chapter, Some(10));
    }

    #[test]
    fn as_of_marks_holdings_as_current_only() {
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_with_character(&pool);
        continuity::persist_item_holdings(
            &pool,
            &story_id,
            None,
            Some(2),
            &[continuity::ItemHoldingDelta {
                item: "玉佩".into(),
                holder: "徐棠".into(),
                action: "acquire".into(),
                evidence: String::new(),
            }],
        )
        .unwrap();
        let view = query_as_of(&pool, &story_id, 1);
        assert_eq!(view.holdings.len(), 1);
        assert!(view.holdings[0].current_only, "物品账本非追加式须标注");
    }

    #[test]
    fn knowledge_log_persist_feeds_time_travel() {
        // 端到端：ingest 落库的知情流水 → 回溯查询可读
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_with_character(&pool);
        continuity::persist_knowledge_updates(
            &pool,
            &story_id,
            None,
            Some(7),
            &[KnowledgeUpdate {
                character: "徐棠".into(),
                fact: "父亲当年并未叛逃".into(),
                evidence: "她翻到那页日记".into(),
            }],
        )
        .unwrap();
        let view = query_as_of(&pool, &story_id, 8);
        assert_eq!(view.character_knowledge.len(), 1);
        assert!(view.character_knowledge[0].known[0].contains("并未叛逃"));
        let before = query_as_of(&pool, &story_id, 6);
        assert!(before.character_knowledge.is_empty());
    }
}
