//! 扩张债务：四项停滞指标的阈值判定与弹性配额文案。
//! 低于阈值零干扰；达标必填；超阈值 ≥2 措辞升级为"严重停滞"。

use rusqlite::params;

use super::{ExpansionDebt, RotationLedger};
use crate::db::connection::DbPool;

pub const CONFLICT_STAGNATION_THRESHOLD: u32 = 2;
pub const SCENE_STAGNATION_THRESHOLD: u32 = 3;
pub const CHARACTER_STAGNATION_THRESHOLD: u32 = 3;
pub const FORESHADOW_STAGNATION_THRESHOLD: u32 = 3;

/// 配额项（与 BeatPlan 字段的映射见 Task 5）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaItem {
    NewScene,
    CharacterMove,
    ConflictEscalation,
    ForeshadowMove,
}

impl ExpansionDebt {
    /// 按续写拍数计债务。`last==0` 且 `append_beats>0` 视为已停滞 append_beats
    /// （不再当旧书零干扰）。
    pub fn from_beats(beats: &crate::creative_engine::expansion::BeatCounters) -> Self {
        let gap = |last: i32, append: i32| -> u32 {
            if append <= 0 {
                0
            } else if last <= 0 {
                append.max(0) as u32
            } else {
                (append - last).max(0) as u32
            }
        };
        ExpansionDebt {
            conflict: gap(beats.last_conflict_beat, beats.append_beats),
            scene: gap(beats.last_location_beat, beats.append_beats),
            character: gap(beats.last_cast_refresh_beat, beats.append_beats),
            foreshadow: gap(beats.last_foreshadow_beat, beats.append_beats),
        }
    }

    pub fn compute(pool: &DbPool, story_id: &str, ledger: &RotationLedger) -> Result<Self, String> {
        let conn = pool.get().map_err(|e| e.to_string())?;
        let beats = super::read_beat_counters(&conn, story_id);
        if beats.append_beats > 0 {
            return Ok(Self::from_beats(&beats));
        }

        let current = ledger.current_sequence;
        let stagnation = |last: i32| -> u32 {
            if current == 0 || last == 0 {
                0
            } else {
                (current - last).max(0) as u32
            }
        };

        // 伏笔停滞：最近一次埋设/回收距当前的章数；表里无任何记录 →
        // 0（旧书零干扰）
        let foreshadow_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM foreshadowing_tracker WHERE story_id = ?1",
                params![story_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let foreshadow = if foreshadow_count == 0 || current == 0 {
            0
        } else {
            let last: Option<i32> = conn
                .query_row(
                    "SELECT MAX(s.sequence_number) FROM scenes s WHERE s.story_id = ?1 AND s.id IN ( \
                        SELECT setup_scene_id FROM foreshadowing_tracker WHERE story_id = ?1 AND setup_scene_id IS NOT NULL \
                        UNION \
                        SELECT payoff_scene_id FROM foreshadowing_tracker WHERE story_id = ?1 AND payoff_scene_id IS NOT NULL \
                    )",
                    params![story_id],
                    |r| r.get(0),
                )
                .unwrap_or(None);
            stagnation(last.unwrap_or(0))
        };

        Ok(ExpansionDebt {
            conflict: ledger.trailing_conflict_free,
            scene: stagnation(ledger.last_new_location_seq),
            character: stagnation(ledger.last_character_refresh_seq),
            foreshadow,
        })
    }

    /// 达到阈值的配额项（平稳期返回空 = 零干扰）
    pub fn triggered(&self) -> Vec<QuotaItem> {
        let mut items = Vec::new();
        if self.conflict >= CONFLICT_STAGNATION_THRESHOLD {
            items.push(QuotaItem::ConflictEscalation);
        }
        if self.scene >= SCENE_STAGNATION_THRESHOLD {
            items.push(QuotaItem::NewScene);
        }
        if self.character >= CHARACTER_STAGNATION_THRESHOLD {
            items.push(QuotaItem::CharacterMove);
        }
        if self.foreshadow >= FORESHADOW_STAGNATION_THRESHOLD {
            items.push(QuotaItem::ForeshadowMove);
        }
        items
    }

    /// 渲染硬性扩张任务段；无触发返回 None
    pub fn quota_text(&self) -> Option<String> {
        let items = self.triggered();
        if items.is_empty() {
            return None;
        }
        let emph = |debt: u32, threshold: u32| -> &'static str {
            if debt >= threshold + 2 {
                "严重停滞"
            } else {
                "停滞"
            }
        };
        let mut lines = vec!["【本章扩张任务（硬性要求，必须落实）】".to_string()];
        for item in items {
            let line = match item {
                QuotaItem::ConflictEscalation => format!(
                    "冲突已{} {} 章——本章必须选择一条活跃冲突线将其升级（加压、反转或代价显现），不得原地踏步、不得仅靠对话过渡。",
                    emph(self.conflict, CONFLICT_STAGNATION_THRESHOLD),
                    self.conflict
                ),
                QuotaItem::NewScene => format!(
                    "场景已{} {} 章——本章必须离开当前场景，开辟一个有叙事功能的新场景（给出名称与剧情关联），不得继续在原场景打转。",
                    emph(self.scene, SCENE_STAGNATION_THRESHOLD),
                    self.scene
                ),
                QuotaItem::CharacterMove => format!(
                    "角色已{} {} 章无更新——本章必须安排一名沉寂角色回归（推进其弧光）或引入一名有叙事功能的新角色。",
                    emph(self.character, CHARACTER_STAGNATION_THRESHOLD),
                    self.character
                ),
                QuotaItem::ForeshadowMove => format!(
                    "伏笔已{} {} 章无动静——本章必须埋设一条新伏笔，或推进/回收到期伏笔。",
                    emph(self.foreshadow, FORESHADOW_STAGNATION_THRESHOLD),
                    self.foreshadow
                ),
            };
            lines.push(line);
        }
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debt(c: u32, s: u32, ch: u32, f: u32) -> ExpansionDebt {
        ExpansionDebt {
            conflict: c,
            scene: s,
            character: ch,
            foreshadow: f,
        }
    }

    #[test]
    fn thresholds_trigger_expected_items() {
        // 全部低于阈值：零干扰
        assert!(debt(1, 2, 2, 2).triggered().is_empty());
        assert!(debt(1, 2, 2, 2).quota_text().is_none());
        // 恰好达标：触发
        assert_eq!(
            debt(2, 0, 0, 0).triggered(),
            vec![QuotaItem::ConflictEscalation]
        );
        assert_eq!(debt(0, 3, 0, 0).triggered(), vec![QuotaItem::NewScene]);
        assert_eq!(debt(0, 0, 3, 0).triggered(), vec![QuotaItem::CharacterMove]);
        assert_eq!(
            debt(0, 0, 0, 3).triggered(),
            vec![QuotaItem::ForeshadowMove]
        );
        // 多项同时触发
        assert_eq!(debt(2, 3, 0, 0).triggered().len(), 2);
    }

    #[test]
    fn beat_debt_triggers_conflict_after_two_appends_without_conflict() {
        let beats = crate::creative_engine::expansion::BeatCounters {
            append_beats: 2,
            last_conflict_beat: 0,
            last_cast_refresh_beat: 0,
            last_location_beat: 0,
            last_foreshadow_beat: 0,
        };
        let debt = ExpansionDebt::from_beats(&beats);
        assert!(debt.conflict >= 2);
        let items = debt.triggered();
        assert!(items.contains(&QuotaItem::ConflictEscalation));
    }

    #[test]
    fn quota_text_escalates_with_debt_depth() {
        let mild = debt(2, 0, 0, 0).quota_text().unwrap();
        assert!(mild.contains("冲突"));
        assert!(mild.contains("必须"));
        // 达标（debt = 阈值）不升级为"严重停滞"
        assert!(!mild.contains("严重停滞"));
        // 差值 1（debt = 阈值 + 1）仍不升级，off-by-one 护栏
        let borderline = debt(3, 0, 0, 0).quota_text().unwrap();
        assert!(borderline.contains("停滞"));
        assert!(!borderline.contains("严重停滞"));
        // 深度债务（超阈值 ≥2）措辞升级
        let deep = debt(4, 0, 0, 0).quota_text().unwrap();
        assert!(deep.contains("严重停滞"));
    }

    #[test]
    fn compute_reads_foreshadow_stagnation() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let req = crate::db::repositories::CreateStoryRequest {
            title: "债务测试".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let sid = repo.create(req).unwrap().id;
        let conn = pool.get().unwrap();
        // 5 个场景，第 2 章埋过一条伏笔
        // scenes.created_at/updated_at 为 NOT NULL，需显式提供
        for seq in 1..=5 {
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, setting_location, characters_present, character_conflicts, content, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, '章', '练功房', '[\"c1\"]', '[]', '正文', '2026-01-01', '2026-01-01')",
                rusqlite::params![format!("s{}", seq), sid, seq],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO foreshadowing_tracker (id, story_id, content, setup_scene_id, status, importance, created_at) \
             VALUES ('f1', ?1, '神秘令牌', 's2', 'setup', 5, datetime('now'))",
            rusqlite::params![sid],
        )
        .unwrap();
        drop(conn);

        let ledger = RotationLedger::load_sync(&pool, &sid).unwrap();
        let d = ExpansionDebt::compute(&pool, &sid, &ledger).unwrap();
        // 伏笔停滞 = 5 - 2 = 3，达阈值
        assert_eq!(d.foreshadow, 3);
        assert!(d.triggered().contains(&QuotaItem::ForeshadowMove));
        // 场景：5 章同一地点，last_new_location_seq=1 → 债务 4
        assert_eq!(d.scene, 4);
        // 冲突：尾部 5 章全无冲突 → 债务 5
        assert_eq!(d.conflict, 5);
    }

    #[test]
    fn no_foreshadow_rows_means_zero_debt() {
        // 旧书兼容：从未有伏笔记录 → 不加压（设计：旧书初始低干扰）
        let pool = crate::db::connection::create_test_pool().unwrap();
        let repo = crate::db::repositories::StoryRepository::new(pool.clone());
        let req = crate::db::repositories::CreateStoryRequest {
            title: "无伏笔".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let sid = repo.create(req).unwrap().id;
        let ledger = RotationLedger::default();
        let d = ExpansionDebt::compute(&pool, &sid, &ledger).unwrap();
        assert_eq!(d.foreshadow, 0);
    }
}
