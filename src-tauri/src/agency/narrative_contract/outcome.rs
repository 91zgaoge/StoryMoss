//! v0.80.1：结构化场结果（SceneOutcome）落库。
//!
//! 进度块不再只从正文末段推断，而是优先读最近 3
//! 章的「结果/状态变化/未决问题」。

use chrono::Local;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::types::{EmotionChain, SceneContract, StateDelta};
use crate::db::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneOutcome {
    pub scene_id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub outcome: String,
    pub state_delta_json: String,
    pub emotion_json: Option<String>,
    pub open_question: String,
    pub contract_misses_json: Option<String>,
    pub created_at: String,
}

impl SceneOutcome {
    pub fn from_contract(
        contract: &SceneContract,
        story_id: &str,
        scene_id: &str,
        chapter_number: i32,
    ) -> Self {
        let now = Local::now().to_rfc3339();
        Self {
            scene_id: scene_id.to_string(),
            story_id: story_id.to_string(),
            chapter_number,
            outcome: contract.outcome.clone(),
            state_delta_json: serde_json::to_string(&contract.state_delta)
                .unwrap_or_else(|_| "{}".to_string()),
            emotion_json: serde_json::to_string(&contract.emotion).ok(),
            open_question: contract.hook_out.clone(),
            contract_misses_json: None,
            created_at: now,
        }
    }
}

pub struct SceneOutcomeRepository {
    pool: DbPool,
}

impl SceneOutcomeRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn ensure_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scene_outcomes (
                scene_id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                state_delta_json TEXT NOT NULL,
                emotion_json TEXT,
                open_question TEXT NOT NULL,
                contract_misses_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_scene_outcomes_story
                ON scene_outcomes (story_id, chapter_number DESC);",
        )
    }

    pub fn upsert(&self, outcome: &SceneOutcome) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scene_outcomes
                (scene_id, story_id, chapter_number, outcome, state_delta_json, emotion_json,
                 open_question, contract_misses_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(scene_id) DO UPDATE SET
                story_id = excluded.story_id,
                chapter_number = excluded.chapter_number,
                outcome = excluded.outcome,
                state_delta_json = excluded.state_delta_json,
                emotion_json = excluded.emotion_json,
                open_question = excluded.open_question,
                contract_misses_json = excluded.contract_misses_json,
                updated_at = excluded.updated_at",
            rusqlite::params![
                outcome.scene_id,
                outcome.story_id,
                outcome.chapter_number,
                outcome.outcome,
                outcome.state_delta_json,
                outcome.emotion_json,
                outcome.open_question,
                outcome.contract_misses_json,
                outcome.created_at,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn list_for_story(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<SceneOutcome>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT scene_id, story_id, chapter_number, outcome, state_delta_json, emotion_json,
                    open_question, contract_misses_json, created_at
             FROM scene_outcomes
             WHERE story_id = ?1
             ORDER BY chapter_number DESC, updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![story_id, limit as i64], |row| {
            Ok(SceneOutcome {
                scene_id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_number: row.get(2)?,
                outcome: row.get(3)?,
                state_delta_json: row.get(4)?,
                emotion_json: row.get(5)?,
                open_question: row.get(6)?,
                contract_misses_json: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn latest_for_scene(
        &self,
        scene_id: &str,
    ) -> Result<Option<SceneOutcome>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        conn.query_row(
            "SELECT scene_id, story_id, chapter_number, outcome, state_delta_json, emotion_json,
                    open_question, contract_misses_json, created_at
             FROM scene_outcomes WHERE scene_id = ?1",
            rusqlite::params![scene_id],
            |row| {
                Ok(SceneOutcome {
                    scene_id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    outcome: row.get(3)?,
                    state_delta_json: row.get(4)?,
                    emotion_json: row.get(5)?,
                    open_question: row.get(6)?,
                    contract_misses_json: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )
        .optional()
    }
}

impl StateDelta {
    /// 供 UI/日志读取的紧凑描述。
    pub fn compact(&self) -> String {
        format!("{} 从「{}」到「{}」", self.axis.as_zh(), self.from, self.to)
    }
}

impl EmotionChain {
    pub fn compact(&self) -> String {
        format!(
            "{}→{}；行动={}；代价={}",
            self.from_emotion, self.to_emotion, self.action, self.cost
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agency::beat_card::ChangeKind, db::connection::create_test_pool};

    fn contract() -> SceneContract {
        SceneContract {
            index: 0,
            total: 1,
            pov: "沈砚".into(),
            place_time: "王府".into(),
            entry_state: "半本名册".into(),
            goal: "让王爷承认名册来历".into(),
            opposition: "王爷".into(),
            opposition_want: "把沈砚引到侧厅".into(),
            obstacle: "王爷烧名册".into(),
            tactic: "当众问军饷".into(),
            turn: "王爷烧名册".into(),
            outcome: "名册烧毁一半，证人被带走".into(),
            irreversible: true,
            state_delta: StateDelta {
                axis: ChangeKind::Relationship,
                from: "表面叔侄".into(),
                to: "公开敌对".into(),
                evidence_hint: "决裂".into(),
            },
            emotion: EmotionChain {
                trigger_event: "名册被烧".into(),
                appraisal: "证据被毁".into(),
                from_emotion: "恐惧".into(),
                to_emotion: "愤怒".into(),
                action: "抢火盆".into(),
                cost: "烫伤".into(),
            },
            information_gap: None,
            promise_op: None,
            hook_out: "证人说母亲也签过".into(),
            word_budget: 1500,
            must_not: vec![],
        }
    }

    #[test]
    fn outcome_roundtrip() {
        let pool = create_test_pool().unwrap();
        let repo = SceneOutcomeRepository::new(pool);
        let outcome = SceneOutcome::from_contract(&contract(), "story-1", "scene-1", 7);
        repo.upsert(&outcome).unwrap();
        let got = repo.latest_for_scene("scene-1").unwrap().unwrap();
        assert_eq!(got.chapter_number, 7);
        assert_eq!(got.outcome, "名册烧毁一半，证人被带走");
        assert!(got.state_delta_json.contains("表面叔侄"));
        let list = repo.list_for_story("story-1", 3).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn outcome_upsert_replaces_same_scene() {
        let pool = create_test_pool().unwrap();
        let repo = SceneOutcomeRepository::new(pool);
        let mut outcome = SceneOutcome::from_contract(&contract(), "story-1", "scene-1", 7);
        outcome.outcome = "新结果".into();
        repo.upsert(&outcome).unwrap();
        assert_eq!(
            repo.latest_for_scene("scene-1").unwrap().unwrap().outcome,
            "新结果"
        );
    }
}
