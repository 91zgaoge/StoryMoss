//! v0.34.0 弹性扩张：轮换账本 + 扩张债务 + 动态资产菜单。
//! 设计：docs/plans/2026-08-09-elastic-expansion-asset-fusion-design.md
//! 纯 Rust 零 LLM 成本；产出注入 beat_planner 模板与 WriteTimeBundle。

pub mod asset_menu;
pub mod debt;
pub mod ledger;

use serde::{Deserialize, Serialize};

/// 近 10 章场景使用账（按地点聚合）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneUsage {
    pub location: String,
    pub count: u32,
}

/// 主要角色沉寂账
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSilence {
    pub character_id: String,
    pub name: String,
    /// 距上次登场的章数（从未登场 = current_sequence）
    pub chapters_absent: u32,
}

/// 轮换账本原始事实（ledger.rs 聚合，debt.rs 消费）
#[derive(Debug, Clone, Default)]
pub struct RotationLedger {
    /// 当前最大 scene sequence_number（无场景 = 0）
    pub current_sequence: i32,
    /// 近 10 章场景使用次数（按次数降序）
    pub scene_usage: Vec<SceneUsage>,
    /// 角色沉寂账（按沉寂章数降序，仅含 absent > 0）
    pub character_silence: Vec<CharacterSilence>,
    /// 最近一次"新地点首次出现"的 sequence（无 = 0）
    pub last_new_location_seq: i32,
    /// 最近一次"新角色登场或沉寂≥3章角色回归"的 sequence（无 = 0）
    pub last_character_refresh_seq: i32,
    /// 尾部连续 character_conflicts 为空的场景数
    pub trailing_conflict_free: u32,
}

/// 四项扩张债务（连续停滞章数）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExpansionDebt {
    pub conflict: u32,
    pub scene: u32,
    pub character: u32,
    pub foreshadow: u32,
}

/// 资产选用历史条目（stories.asset_history_json）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetHistoryEntry {
    pub chapter: i32,
    pub ids: Vec<String>,
}

/// 续写拍计数（与资产历史同列共存）。旧书从未 Append 时全 0。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeatCounters {
    #[serde(default)]
    pub append_beats: i32,
    #[serde(default)]
    pub last_conflict_beat: i32,
    #[serde(default)]
    pub last_cast_refresh_beat: i32,
    #[serde(default)]
    pub last_location_beat: i32,
    #[serde(default)]
    pub last_foreshadow_beat: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetHistoryDocument {
    #[serde(default)]
    assets: Vec<AssetHistoryEntry>,
    #[serde(default)]
    beats: BeatCounters,
}

const ASSET_HISTORY_KEEP: usize = 10;

fn parse_history_document(raw: Option<String>) -> AssetHistoryDocument {
    let Some(s) = raw.filter(|s| !s.trim().is_empty()) else {
        return AssetHistoryDocument {
            assets: vec![],
            beats: BeatCounters::default(),
        };
    };
    if let Ok(doc) = serde_json::from_str::<AssetHistoryDocument>(&s) {
        return doc;
    }
    if let Ok(assets) = serde_json::from_str::<Vec<AssetHistoryEntry>>(&s) {
        return AssetHistoryDocument {
            assets,
            beats: BeatCounters::default(),
        };
    }
    AssetHistoryDocument {
        assets: vec![],
        beats: BeatCounters::default(),
    }
}

fn load_history_document(conn: &rusqlite::Connection, story_id: &str) -> AssetHistoryDocument {
    let raw: Option<String> = conn
        .query_row(
            "SELECT asset_history_json FROM stories WHERE id = ?1",
            rusqlite::params![story_id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    parse_history_document(raw)
}

fn save_history_document(
    conn: &rusqlite::Connection,
    story_id: &str,
    doc: &AssetHistoryDocument,
) -> Result<(), String> {
    let json = serde_json::to_string(doc).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE stories SET asset_history_json = ?1 WHERE id = ?2",
        rusqlite::params![json, story_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取资产选用历史（NULL/损坏 JSON → 空）。兼容旧数组与新 `{assets,beats}`
/// 对象。
pub fn read_asset_history(conn: &rusqlite::Connection, story_id: &str) -> Vec<AssetHistoryEntry> {
    load_history_document(conn, story_id).assets
}

pub fn read_beat_counters(conn: &rusqlite::Connection, story_id: &str) -> BeatCounters {
    load_history_document(conn, story_id).beats
}

/// 追加一条资产选用历史，只保留最近 10 条；不得抹掉既有 beats。
pub fn append_asset_history(
    pool: &crate::db::connection::DbPool,
    story_id: &str,
    chapter: i32,
    ids: &[String],
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let mut doc = load_history_document(&tx, story_id);
    doc.assets.push(AssetHistoryEntry {
        chapter,
        ids: ids.to_vec(),
    });
    if doc.assets.len() > ASSET_HISTORY_KEEP {
        doc.assets = doc.assets.split_off(doc.assets.len() - ASSET_HISTORY_KEEP);
    }
    save_history_document(&tx, story_id, &doc)?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn write_beat_counters(
    conn: &rusqlite::Connection,
    story_id: &str,
    beats: BeatCounters,
) -> Result<(), String> {
    let mut doc = load_history_document(conn, story_id);
    doc.beats = beats;
    save_history_document(conn, story_id, &doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_story(pool: &crate::db::connection::DbPool) -> String {
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let req = crate::db::repositories::CreateStoryRequest {
            title: "测试小说".to_string(),
            description: None,
            genre: Some("玄幻".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        repo.create(req).unwrap().id
    }

    #[test]
    fn asset_history_roundtrip_and_retention() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let sid = seed_story(&pool);
        // 无历史时为空
        {
            let conn = pool.get().unwrap();
            assert!(read_asset_history(&conn, &sid).is_empty());
        }
        // 追加 12 条，只保留最近 10 条
        for ch in 1..=12 {
            append_asset_history(&pool, &sid, ch, &[format!("beat_card.x{}", ch)]).unwrap();
        }
        let conn = pool.get().unwrap();
        let history = read_asset_history(&conn, &sid);
        assert_eq!(history.len(), 10);
        assert_eq!(history.first().unwrap().chapter, 3); // 最旧的 1、2 被淘汰
        assert_eq!(history.last().unwrap().chapter, 12);
        assert_eq!(
            history.last().unwrap().ids,
            vec!["beat_card.x12".to_string()]
        );
    }

    #[test]
    fn append_asset_history_preserves_beats() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let sid = seed_story(&pool);
        {
            let conn = pool.get().unwrap();
            write_beat_counters(
                &conn,
                &sid,
                BeatCounters {
                    append_beats: 3,
                    last_conflict_beat: 1,
                    ..BeatCounters::default()
                },
            )
            .unwrap();
        }
        append_asset_history(&pool, &sid, 1, &["beat_card.x".into()]).unwrap();
        let conn = pool.get().unwrap();
        let beats = read_beat_counters(&conn, &sid);
        assert_eq!(beats.append_beats, 3);
        assert_eq!(beats.last_conflict_beat, 1);
        assert_eq!(read_asset_history(&conn, &sid).len(), 1);
    }
}
