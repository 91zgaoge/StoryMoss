//! P1-5（v0.69.0）：作者裁决回灌每拍手法池。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md——Sherpa 用行为数据
//! 训练散文引擎；StoryMoss 的正确对位是「把作者裁决变成每拍决策的权重」：
//!
//! 1. 每拍写作时把**用了哪条手法**落库（`beat_technique_log`，V147）；
//! 2. 作者的 Tab 接受 / Esc 放弃按**时间窗**归因到最近一拍的手法上——幕前反馈
//!    不带 scene_id（且 chapter_id 语义不稳定），取「反馈时刻之前最近一次落库的
//!    拍」且间隔 ≤ `TECHNIQUE_ATTRIBUTION_WINDOW_SECS`，并把反馈限制为
//!    `agent_type = 'writer'`（手法只注入主续写路径）；
//! 3. 统计反哺 `human_voice::choose_technique_index`（优选已被接受、避开已被
//!    证明不佳），无数据时照旧基准轮换。
//!
//! 纪律：只影响提示词里的一条写作手段，不参与质量门限；统计可重置（重置 =
//! 清空归因基底，后续重新积累——不改任何正文/资产）。

use chrono::{DateTime, FixedOffset};
use rusqlite::params;

use crate::{
    db::DbPool,
    story_system::human_voice::{technique_label, TechniqueStat, TECHNIQUE_POOL},
};

/// 反馈归因窗口：反馈时刻与最近一拍落库时刻的最大间隔（秒）。
pub const TECHNIQUE_ATTRIBUTION_WINDOW_SECS: i64 = 3600;

/// 落一行「第 beat_no 拍用了 technique_id」。
pub fn record_beat_technique(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: i32,
    beat_no: i32,
    technique_id: &str,
) -> Result<(), String> {
    if story_id.trim().is_empty() || technique_id.trim().is_empty() {
        return Ok(());
    }
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO beat_technique_log \
         (id, story_id, scene_id, chapter_number, beat_no, technique_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            scene_id,
            chapter_number,
            beat_no,
            technique_id,
            chrono::Local::now().to_rfc3339(),
        ],
    )
    .map_err(|e| format!("手法落库失败: {e}"))?;
    Ok(())
}

/// 最近 `n` 拍用过的手法 id（新→旧；用于避免连续重复同一手法）。
pub fn recent_technique_ids(
    pool: &DbPool,
    story_id: &str,
    n: usize,
) -> Result<Vec<String>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT technique_id FROM beat_technique_log WHERE story_id = ?1 \
             ORDER BY created_at DESC, rowid DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![story_id, n as i64], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn parse_time(raw: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(raw).ok()
}

/// 作者裁决统计（按时间窗归因；返回全部手法池条目，含零样本）。
pub fn technique_stats(pool: &DbPool, story_id: &str) -> Result<Vec<TechniqueStat>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let beats: Vec<(String, DateTime<FixedOffset>)> = {
        let mut stmt = conn
            .prepare(
                "SELECT technique_id, created_at FROM beat_technique_log \
                 WHERE story_id = ?1 ORDER BY created_at ASC, rowid ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([story_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok)
            .filter_map(|(id, ts)| parse_time(&ts).map(|t| (id, t)))
            .collect()
    };
    // 反馈只取主续写路径（手法只注入这里）：accept / reject。
    let feedback: Vec<(String, DateTime<FixedOffset>)> = {
        let mut stmt = conn
            .prepare(
                "SELECT feedback_type, created_at FROM user_feedback_log \
                 WHERE story_id = ?1 AND feedback_type IN ('accept','reject') \
                 AND (agent_type IS NULL OR agent_type = 'writer') \
                 ORDER BY created_at ASC, rowid ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([story_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok)
            .filter_map(|(kind, ts)| parse_time(&ts).map(|t| (kind, t)))
            .collect()
    };

    let mut accepts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut rejects: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (kind, at) in &feedback {
        // 归因：反馈时刻之前最近一拍，且间隔在窗口内。
        let attributed = beats
            .iter()
            .filter(|(_, beat_at)| beat_at <= at)
            .filter(|(_, beat_at)| {
                (*at - *beat_at).num_seconds() <= TECHNIQUE_ATTRIBUTION_WINDOW_SECS
            })
            .last()
            .map(|(id, _)| id.as_str());
        if let Some(technique_id) = attributed {
            if kind == "accept" {
                *accepts.entry(technique_id).or_insert(0) += 1;
            } else {
                *rejects.entry(technique_id).or_insert(0) += 1;
            }
        }
    }

    Ok(TECHNIQUE_POOL
        .iter()
        .map(|t| TechniqueStat {
            technique_id: t.id.to_string(),
            label: technique_label(t.id),
            accepts: accepts.get(t.id).copied().unwrap_or(0),
            rejects: rejects.get(t.id).copied().unwrap_or(0),
        })
        .collect())
}

/// 重置统计：清空该故事的归因基底（后续重新积累）。返回删除的行数。
pub fn reset_technique_stats(pool: &DbPool, story_id: &str) -> Result<usize, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM beat_technique_log WHERE story_id = ?1",
        params![story_id],
    )
    .map_err(|e| format!("重置手法统计失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "手法回灌".into(),
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

    fn insert_beat(pool: &DbPool, story_id: &str, technique_id: &str, created_at: &str) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO beat_technique_log \
             (id, story_id, scene_id, chapter_number, beat_no, technique_id, created_at) \
             VALUES (?1, ?2, 'sc1', 1, 1, ?3, ?4)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                technique_id,
                created_at
            ],
        )
        .unwrap();
    }

    fn insert_feedback(pool: &DbPool, story_id: &str, kind: &str, agent: Option<&str>, at: &str) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO user_feedback_log \
             (id, story_id, scene_id, chapter_id, feedback_type, agent_type, original_ai_text, \
              final_text, metadata, created_at) \
             VALUES (?1, ?2, NULL, NULL, ?3, ?4, '', '', NULL, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), story_id, kind, agent, at],
        )
        .unwrap();
    }

    #[test]
    fn record_writes_row_and_recent_returns_newest_first() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        record_beat_technique(&pool, &story_id, Some("sc1"), 1, 1, "late-reveal").unwrap();
        record_beat_technique(&pool, &story_id, Some("sc1"), 1, 2, "plain-naming").unwrap();
        let recent = recent_technique_ids(&pool, &story_id, 3).unwrap();
        assert_eq!(
            recent,
            vec!["plain-naming".to_string(), "late-reveal".to_string()]
        );
        assert_eq!(recent_technique_ids(&pool, &story_id, 1).unwrap().len(), 1);
    }

    #[test]
    fn stats_attribute_feedback_by_time_window() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        // 拍 1 在 10:00 落库（late-reveal），拍 2 在 10:10（plain-naming）。
        insert_beat(&pool, &story_id, "late-reveal", "2026-10-14T10:00:00+08:00");
        insert_beat(
            &pool,
            &story_id,
            "plain-naming",
            "2026-10-14T10:10:00+08:00",
        );
        // 10:11 接受 → 归因拍 2；10:12 放弃 → 归因拍 2；10:05 接受 → 归因拍 1。
        insert_feedback(
            &pool,
            &story_id,
            "accept",
            Some("writer"),
            "2026-10-14T10:11:00+08:00",
        );
        insert_feedback(
            &pool,
            &story_id,
            "reject",
            Some("writer"),
            "2026-10-14T10:12:00+08:00",
        );
        insert_feedback(
            &pool,
            &story_id,
            "accept",
            Some("writer"),
            "2026-10-14T10:05:00+08:00",
        );
        // 超出窗口（11:30，最近一拍 10:10 差 80 分钟）→ 不归因。
        insert_feedback(
            &pool,
            &story_id,
            "accept",
            Some("writer"),
            "2026-10-14T11:30:00+08:00",
        );
        // 非 writer 路径（内联建议/自动续写）→ 不归因。
        insert_feedback(
            &pool,
            &story_id,
            "accept",
            Some("inline_suggestion"),
            "2026-10-14T10:11:30+08:00",
        );

        let stats = technique_stats(&pool, &story_id).unwrap();
        let find = |id: &str| stats.iter().find(|s| s.technique_id == id).unwrap();
        assert_eq!(stats.len(), TECHNIQUE_POOL.len());
        assert_eq!(find("late-reveal").accepts, 1);
        assert_eq!(find("plain-naming").accepts, 1);
        assert_eq!(find("plain-naming").rejects, 1);
        assert_eq!(find("plain-naming").sample(), 2);
        assert_eq!(find("plain-naming").quality(), 0.0);
        assert_eq!(find("late-reveal").quality(), 1.0);
        // 无关手法零样本。
        assert_eq!(find("time-shift").sample(), 0);
        assert_eq!(find("time-shift").quality(), 0.0);
    }

    #[test]
    fn reset_clears_attribution_base() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        insert_beat(&pool, &story_id, "late-reveal", "2026-10-14T10:00:00+08:00");
        insert_feedback(
            &pool,
            &story_id,
            "accept",
            Some("writer"),
            "2026-10-14T10:01:00+08:00",
        );
        assert_eq!(technique_stats(&pool, &story_id).unwrap()[0].accepts, 1);
        let deleted = reset_technique_stats(&pool, &story_id).unwrap();
        assert_eq!(deleted, 1);
        let stats = technique_stats(&pool, &story_id).unwrap();
        assert!(stats.iter().all(|s| s.sample() == 0), "重置后应全部零样本");
        assert!(recent_technique_ids(&pool, &story_id, 3)
            .unwrap()
            .is_empty());
    }
}
