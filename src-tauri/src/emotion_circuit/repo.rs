//! 情感回路的 SQLite 仓储。所有方法同步；调用方负责 `spawn_blocking`。

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{
    probe::{EmotionProbe, ProbeSeverity},
    reader::ReaderReview,
    types::{
        CharacterEmotionState, EmotionBeat, EmotionContract, EmotionDebt, EmotionProbeLog,
        EmotionTension, ReaderResonanceReview, SceneEmotionBeat,
    },
};
use crate::db::DbPool;

pub struct EmotionCircuitRepository {
    pool: DbPool,
}

impl EmotionCircuitRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn conn(
        &self,
    ) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, rusqlite::Error> {
        self.pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
    }

    pub fn get_contract(&self, story_id: &str) -> Result<Option<EmotionContract>, rusqlite::Error> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT story_id, core_question, reader_promise, palette_json, ending_residue, \
             forbidden_json, peak_budget, version FROM story_emotion_contracts WHERE story_id = ?1",
            [story_id],
            |row| {
                let palette: String = row.get(3)?;
                let forbidden: String = row.get(5)?;
                Ok(EmotionContract {
                    story_id: row.get(0)?,
                    core_question: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    reader_promise: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    palette: serde_json::from_str(&palette).unwrap_or_default(),
                    ending_residue: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    forbidden: serde_json::from_str(&forbidden).unwrap_or_default(),
                    peak_budget: row.get::<_, i64>(6)?.clamp(0, 9) as u8,
                    version: row.get(7)?,
                })
            },
        )
        .optional()
    }

    pub fn upsert_contract(&self, c: &EmotionContract) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO story_emotion_contracts \
             (story_id, core_question, reader_promise, palette_json, ending_residue, forbidden_json, \
              peak_budget, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9) \
             ON CONFLICT(story_id) DO UPDATE SET \
              core_question=excluded.core_question, reader_promise=excluded.reader_promise, \
              palette_json=excluded.palette_json, ending_residue=excluded.ending_residue, \
              forbidden_json=excluded.forbidden_json, peak_budget=excluded.peak_budget, \
              version=excluded.version, updated_at=excluded.updated_at",
            params![
                c.story_id,
                c.core_question,
                c.reader_promise,
                json(&c.palette)?,
                c.ending_residue,
                json(&c.forbidden)?,
                c.peak_budget as i64,
                c.version.max(1),
                now,
            ],
        )?;
        Ok(())
    }

    /// 每个角色最新一条状态。
    pub fn latest_states(
        &self,
        story_id: &str,
    ) -> Result<Vec<CharacterEmotionState>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, character_id, scene_id, chapter_number, beat_index, \
             vector_json, intensity, active_goal, defense, cause_event, source, created_at \
             FROM character_emotion_states WHERE story_id = ?1 \
             ORDER BY COALESCE(chapter_number,0) ASC, COALESCE(beat_index,0) ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([story_id], map_state_row)?;
        let mut latest: std::collections::HashMap<String, CharacterEmotionState> =
            std::collections::HashMap::new();
        for row in rows {
            let state = row?;
            latest.insert(state.character_id.clone(), state);
        }
        let mut out: Vec<CharacterEmotionState> = latest.into_values().collect();
        out.sort_by(|a, b| a.character_id.cmp(&b.character_id));
        for s in &mut out {
            s.character_name = self
                .character_name(story_id, &s.character_id)
                .unwrap_or_default()
                .unwrap_or_default();
        }
        Ok(out)
    }

    pub fn latest_state(
        &self,
        story_id: &str,
        character_id: &str,
    ) -> Result<Option<CharacterEmotionState>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut state = conn
            .query_row(
                "SELECT id, story_id, character_id, scene_id, chapter_number, beat_index, \
                 vector_json, intensity, active_goal, defense, cause_event, source, created_at \
                 FROM character_emotion_states WHERE story_id = ?1 AND character_id = ?2 \
                 ORDER BY COALESCE(chapter_number,0) DESC, COALESCE(beat_index,0) DESC, \
                 created_at DESC LIMIT 1",
                params![story_id, character_id],
                map_state_row,
            )
            .optional()?;
        if let Some(ref mut s) = state {
            s.character_name = self
                .character_name(story_id, character_id)
                .unwrap_or_default()
                .unwrap_or_default();
        }
        Ok(state)
    }

    pub fn insert_state(&self, s: &CharacterEmotionState) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO character_emotion_states \
             (id, story_id, character_id, scene_id, chapter_number, beat_index, vector_json, \
              intensity, active_goal, defense, cause_event, source, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                if s.id.is_empty() {
                    Uuid::new_v4().to_string()
                } else {
                    s.id.clone()
                },
                s.story_id,
                s.character_id,
                s.scene_id,
                s.chapter_number,
                s.beat_index,
                json(&s.vector)?,
                s.intensity.clamp(0.0, 1.0),
                s.active_goal,
                s.defense,
                s.cause_event,
                s.source,
                if s.created_at.is_empty() {
                    Local::now().to_rfc3339()
                } else {
                    s.created_at.clone()
                },
            ],
        )?;
        Ok(())
    }

    /// 首次使用时从角色关系播种张力账本（只播种一次，不改既有账本）。
    /// 这样已有情感关系的老故事无需手工录入也能进入情感回路。
    pub fn seed_tensions_from_relationships(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<usize, rusqlite::Error> {
        let existing = self.list_tensions(story_id)?;
        if !existing.is_empty() {
            return Ok(0);
        }
        let rels = crate::db::repositories::CharacterRelationshipRepository::new(self.pool.clone())
            .get_by_story(story_id)?;
        let chars = crate::db::repositories::CharacterRepository::new(self.pool.clone())
            .get_by_story(story_id)
            .unwrap_or_default();
        let now = Local::now().to_rfc3339();
        let mut rows: Vec<EmotionTension> = Vec::new();
        for r in rels {
            let bond = r.emotional_bond.as_deref().unwrap_or("");
            let reverse = r.reverse_emotional_bond.as_deref().unwrap_or("");
            if bond.trim().is_empty() && reverse.trim().is_empty() {
                continue;
            }
            let source_name = chars
                .iter()
                .find(|c| c.id == r.source_character_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let target_name = r.target_character_name.clone().unwrap_or_default();
            if source_name.is_empty() || target_name.is_empty() {
                continue;
            }
            let tension = crate::agency::emotional_ledger::compute_interpersonal_tension(
                bond,
                r.emotional_intensity.unwrap_or(0.5),
                reverse,
                r.reverse_emotional_intensity.unwrap_or(0.5),
            );
            rows.push(EmotionTension {
                id: format!("rel:{}:{}", r.source_character_id, r.target_character_id),
                story_id: story_id.to_string(),
                source_character_id: r.source_character_id.clone(),
                target_character_id: Some(r.target_character_id.clone()),
                source_name,
                target_name,
                tension_type: tension.tension_type,
                bond: (!bond.trim().is_empty()).then(|| bond.to_string()),
                reverse_bond: (!reverse.trim().is_empty()).then(|| reverse.to_string()),
                pressure: tension.pressure.clamp(0.0, 1.0),
                accumulated_beats: 0,
                due_chapter: Some(chapter_number + 3),
                status: "open".into(),
                last_escalated_chapter: None,
                updated_at: now.clone(),
            });
        }
        rows.sort_by(|a, b| {
            b.pressure
                .partial_cmp(&a.pressure)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows.truncate(8);
        for t in &rows {
            self.upsert_tension(t)?;
        }
        Ok(rows.len())
    }

    pub fn list_tensions(&self, story_id: &str) -> Result<Vec<EmotionTension>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.story_id, t.source_character_id, t.target_character_id, \
             COALESCE((SELECT name FROM kg_entities WHERE id = t.source_character_id), \
                      (SELECT name FROM characters WHERE id = t.source_character_id), '') AS source_name, \
             COALESCE((SELECT name FROM kg_entities WHERE id = t.target_character_id), \
                      (SELECT name FROM characters WHERE id = t.target_character_id), '') AS target_name, \
             t.tension_type, t.bond, t.reverse_bond, t.pressure, t.accumulated_beats, \
             t.due_chapter, t.status, t.last_escalated_chapter, t.updated_at \
             FROM emotion_tensions t WHERE t.story_id = ?1 \
             ORDER BY t.pressure DESC, t.accumulated_beats DESC",
        )?;
        let rows = stmt.query_map([story_id], |row| {
            Ok(EmotionTension {
                id: row.get(0)?,
                story_id: row.get(1)?,
                source_character_id: row.get(2)?,
                target_character_id: row.get(3)?,
                source_name: row.get(4)?,
                target_name: row.get(5)?,
                tension_type: row.get(6)?,
                bond: row.get(7)?,
                reverse_bond: row.get(8)?,
                pressure: row.get::<_, f64>(9)? as f32,
                accumulated_beats: row.get::<_, i64>(10)?.max(0) as u32,
                due_chapter: row.get(11)?,
                status: row.get(12)?,
                last_escalated_chapter: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_tension(&self, t: &EmotionTension) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO emotion_tensions \
             (id, story_id, source_character_id, target_character_id, tension_type, bond, \
              reverse_bond, pressure, accumulated_beats, due_chapter, status, \
              last_escalated_chapter, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT(id) DO UPDATE SET tension_type=excluded.tension_type, bond=excluded.bond, \
              reverse_bond=excluded.reverse_bond, pressure=excluded.pressure, \
              accumulated_beats=excluded.accumulated_beats, due_chapter=excluded.due_chapter, \
              status=excluded.status, last_escalated_chapter=excluded.last_escalated_chapter, \
              updated_at=excluded.updated_at",
            params![
                t.id,
                t.story_id,
                t.source_character_id,
                t.target_character_id,
                t.tension_type,
                t.bond,
                t.reverse_bond,
                t.pressure.clamp(0.0, 1.0),
                t.accumulated_beats as i64,
                t.due_chapter,
                t.status,
                t.last_escalated_chapter,
                if t.updated_at.is_empty() {
                    Local::now().to_rfc3339()
                } else {
                    t.updated_at.clone()
                },
            ],
        )?;
        Ok(())
    }

    pub fn list_debts(&self, story_id: &str) -> Result<Vec<EmotionDebt>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, kind, source_ref, description, pressure, due_chapter, status, \
             created_at FROM emotion_debts WHERE story_id = ?1 \
             ORDER BY pressure DESC, COALESCE(due_chapter, 9999) ASC",
        )?;
        let rows = stmt.query_map([story_id], |row| {
            Ok(EmotionDebt {
                id: row.get(0)?,
                story_id: row.get(1)?,
                kind: row.get(2)?,
                source_ref: row.get(3)?,
                description: row.get(4)?,
                pressure: row.get::<_, f64>(5)? as f32,
                due_chapter: row.get(6)?,
                status: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_debt(&self, d: &EmotionDebt) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO emotion_debts \
             (id, story_id, kind, source_ref, description, pressure, due_chapter, status, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, source_ref=excluded.source_ref, \
              description=excluded.description, pressure=excluded.pressure, \
              due_chapter=excluded.due_chapter, status=excluded.status",
            params![
                d.id,
                d.story_id,
                d.kind,
                d.source_ref,
                d.description,
                d.pressure.clamp(0.0, 1.0),
                d.due_chapter,
                d.status,
                if d.created_at.is_empty() { Local::now().to_rfc3339() } else { d.created_at.clone() },
            ],
        )?;
        Ok(())
    }

    pub fn resolve_debt(
        &self,
        story_id: &str,
        debt_id: &str,
        status: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE emotion_debts SET status = ?1 WHERE story_id = ?2 AND id = ?3",
            params![status, story_id, debt_id],
        )?;
        Ok(())
    }

    pub fn insert_scene_beat(&self, b: &SceneEmotionBeat) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO scene_emotion_beats \
             (id, story_id, scene_id, beat_index, from_json, to_json, trigger_event, appraisal_json, \
              action_tendency, dialogue_policy_json, cost, carrier_weights_json, reader_target_json, \
              scale, event_kind, source_name, target_name, tension_id, debt_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                if b.id.is_empty() { Uuid::new_v4().to_string() } else { b.id.clone() },
                b.story_id,
                b.scene_id,
                b.beat_index,
                json(&b.from)?,
                json(&b.to)?,
                b.trigger_event,
                json(&b.appraisal)?,
                serde_json::to_string(&b.action_tendency)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                b.dialogue_policy.as_ref().map(json_owned).transpose()?,
                b.cost,
                json(&b.carriers)?,
                json(&b.reader_target)?,
                b.scale,
                b.event_kind.id(),
                b.source_name,
                b.target_name,
                b.tension_id,
                b.debt_id,
                if b.created_at.is_empty() { Local::now().to_rfc3339() } else { b.created_at.clone() },
            ],
        )?;
        Ok(())
    }

    pub fn list_scene_beats(
        &self,
        story_id: &str,
        scene_id: &str,
    ) -> Result<Vec<SceneEmotionBeat>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, beat_index, from_json, to_json, trigger_event, \
             appraisal_json, action_tendency, dialogue_policy_json, cost, carrier_weights_json, \
             reader_target_json, scale, event_kind, source_name, target_name, tension_id, debt_id, \
             created_at FROM scene_emotion_beats \
             WHERE story_id = ?1 AND scene_id = ?2 ORDER BY beat_index ASC",
        )?;
        let rows = stmt.query_map(params![story_id, scene_id], |row| {
            let from_json: String = row.get(4)?;
            let to_json: String = row.get(5)?;
            let appraisal_json: String = row.get(7)?;
            let action: String = row.get(8)?;
            let dialogue: Option<String> = row.get(9)?;
            let carriers_json: String = row.get(11)?;
            let reader_json: String = row.get(12)?;
            Ok(SceneEmotionBeat {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                beat_index: row.get(3)?,
                from: serde_json::from_str(&from_json).unwrap_or_default(),
                to: serde_json::from_str(&to_json).unwrap_or_default(),
                trigger_event: row.get(6)?,
                appraisal: serde_json::from_str(&appraisal_json).unwrap_or_default(),
                action_tendency: serde_json::from_str(&action).unwrap_or_default(),
                dialogue_policy: dialogue.and_then(|s| serde_json::from_str(&s).ok()),
                cost: row.get(10)?,
                carriers: serde_json::from_str(&carriers_json).unwrap_or_default(),
                reader_target: serde_json::from_str(&reader_json).unwrap_or_default(),
                scale: row.get::<_, f64>(13)? as f32,
                event_kind: row
                    .get::<_, Option<String>>(14)?
                    .map(|s| crate::emotion_circuit::appraisal::EventKind::from_id(&s))
                    .unwrap_or_default(),
                source_name: row.get::<_, Option<String>>(15)?.unwrap_or_default(),
                target_name: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
                tension_id: row.get(17)?,
                debt_id: row.get(18)?,
                created_at: row.get(19)?,
            })
        })?;
        rows.collect()
    }

    /// 某章最近一拍（用于 Gate 情感维度与读者面板）。
    pub fn latest_beat_for_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<SceneEmotionBeat>, rusqlite::Error> {
        let conn = self.conn()?;
        let scene_id: Option<String> = conn
            .query_row(
                "SELECT id FROM scenes WHERE story_id = ?1 AND sequence_number = ?2 \
                 ORDER BY updated_at DESC LIMIT 1",
                params![story_id, chapter_number],
                |row| row.get(0),
            )
            .optional()?;
        let Some(scene_id) = scene_id else {
            return Ok(None);
        };
        let mut beats = self.list_scene_beats(story_id, &scene_id)?;
        Ok(beats.pop())
    }

    pub fn next_beat_index(&self, story_id: &str, scene_id: &str) -> Result<i32, rusqlite::Error> {
        let conn = self.conn()?;
        let next: Option<i64> = conn.query_row(
            "SELECT MAX(beat_index) FROM scene_emotion_beats WHERE story_id = ?1 AND scene_id = ?2",
            params![story_id, scene_id],
            |row| row.get(0),
        )?;
        Ok(next.unwrap_or(0) as i32 + 1)
    }

    pub fn peak_used(&self, story_id: &str, chapter_number: i32) -> Result<u32, rusqlite::Error> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM scene_emotion_beats b JOIN scenes s ON s.id = b.scene_id \
             WHERE b.story_id = ?1 AND s.sequence_number = ?2 AND b.scale >= 1.15",
            params![story_id, chapter_number],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn insert_probe_logs(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        beat_index: Option<i32>,
        probes: &[EmotionProbe],
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        let now = Local::now().to_rfc3339();
        for p in probes {
            conn.execute(
                "INSERT INTO emotion_probe_log \
                 (id, story_id, scene_id, beat_index, probe, severity, passed, detail_json, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    story_id,
                    scene_id,
                    beat_index,
                    p.code,
                    p.severity.id(),
                    p.passed as i32,
                    json(&p.detail)?,
                    now,
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_probe_logs(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
    ) -> Result<Vec<EmotionProbeLog>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, beat_index, probe, severity, passed, detail_json, \
             created_at FROM emotion_probe_log WHERE story_id = ?1 \
             AND (?2 IS NULL OR scene_id = ?2) ORDER BY created_at DESC LIMIT 200",
        )?;
        let rows = stmt.query_map(params![story_id, scene_id], |row| {
            let detail: String = row.get(7)?;
            Ok(EmotionProbeLog {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                beat_index: row.get(3)?,
                probe: row.get(4)?,
                severity: row.get(5)?,
                passed: row.get::<_, i64>(6)? != 0,
                detail: serde_json::from_str(&detail).unwrap_or_default(),
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn insert_reader_review(&self, r: &ReaderResonanceReview) -> Result<(), rusqlite::Error> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO reader_resonance_reviews \
             (id, story_id, scene_id, chapter_number, persona, scores_json, verdict, evidence_json, \
              created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                if r.id.is_empty() { Uuid::new_v4().to_string() } else { r.id.clone() },
                r.story_id,
                r.scene_id,
                r.chapter_number,
                r.persona,
                json(&r.scores)?,
                r.verdict,
                r.evidence.as_ref().map(json_owned).transpose()?,
                if r.created_at.is_empty() { Local::now().to_rfc3339() } else { r.created_at.clone() },
            ],
        )?;
        Ok(())
    }

    pub fn list_reader_reviews(
        &self,
        story_id: &str,
        chapter_number: Option<i32>,
    ) -> Result<Vec<ReaderResonanceReview>, rusqlite::Error> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_number, persona, scores_json, verdict, \
             evidence_json, created_at FROM reader_resonance_reviews WHERE story_id = ?1 \
             AND (?2 IS NULL OR chapter_number = ?2) ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![story_id, chapter_number], |row| {
            let scores: String = row.get(5)?;
            let evidence: Option<String> = row.get(7)?;
            Ok(ReaderResonanceReview {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                chapter_number: row.get(3)?,
                persona: row.get(4)?,
                scores: serde_json::from_str(&scores).unwrap_or_default(),
                verdict: row.get(6)?,
                evidence: evidence.and_then(|s| serde_json::from_str(&s).ok()),
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    /// 本拍落库后的状态更新：场景情感链 + 主角情绪状态 + 张力累积。
    pub fn persist_after_beat(
        &self,
        story_id: &str,
        scene_id: &str,
        chapter_number: i32,
        beat: &EmotionBeat,
    ) -> Result<(), rusqlite::Error> {
        let beat_index = self.next_beat_index(story_id, scene_id)?;
        let scene_beat = SceneEmotionBeat {
            id: Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
            beat_index,
            from: beat.from.clone(),
            to: beat.to.clone(),
            trigger_event: beat.trigger_event.clone(),
            appraisal: beat.appraisal.clone(),
            action_tendency: beat.action_tendency,
            dialogue_policy: beat.dialogue_policy.clone(),
            cost: beat.cost.clone(),
            carriers: beat.carriers.clone(),
            reader_target: beat.reader_target.clone(),
            scale: beat.scale,
            event_kind: beat.event_kind,
            source_name: beat.source_name.clone(),
            target_name: beat.target_name.clone(),
            tension_id: beat.tension_id.clone(),
            debt_id: beat.debt_id.clone(),
            created_at: Local::now().to_rfc3339(),
        };
        self.insert_scene_beat(&scene_beat)?;
        let Some(character_id) = self.find_character_id(story_id, &beat.source_name)? else {
            return Ok(());
        };
        let state = CharacterEmotionState {
            id: Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            character_id,
            character_name: beat.source_name.clone(),
            scene_id: Some(scene_id.to_string()),
            chapter_number: Some(chapter_number),
            beat_index: Some(beat_index),
            vector: beat.to.clone(),
            intensity: beat.to.dominant().1.clamp(0.0, 1.0),
            active_goal: None,
            defense: None,
            cause_event: Some(beat.trigger_event.clone()),
            source: "commit".into(),
            created_at: Local::now().to_rfc3339(),
        };
        self.insert_state(&state)?;
        if let Some(ref tid) = beat.tension_id {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE emotion_tensions SET pressure = MIN(1.0, pressure + 0.05), \
                 accumulated_beats = accumulated_beats + 1, last_escalated_chapter = ?1, \
                 updated_at = ?2 WHERE story_id = ?3 AND id = ?4",
                params![chapter_number, Local::now().to_rfc3339(), story_id, tid],
            )?;
        }
        Ok(())
    }

    pub fn find_character_id(
        &self,
        story_id: &str,
        name: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn()?;
        let id: Option<String> = conn
            .query_row(
                "SELECT id FROM kg_entities WHERE story_id = ?1 AND name = ?2 \
                 AND entity_type = 'Character' AND is_archived = 0 LIMIT 1",
                params![story_id, name],
                |row| row.get(0),
            )
            .optional()?;
        if id.is_some() {
            return Ok(id);
        }
        conn.query_row(
            "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2 LIMIT 1",
            params![story_id, name],
            |row| row.get(0),
        )
        .optional()
    }

    fn character_name(
        &self,
        story_id: &str,
        character_id: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT name FROM kg_entities WHERE id = ?1 AND story_id = ?2 UNION ALL \
             SELECT name FROM characters WHERE id = ?1 AND story_id = ?2 LIMIT 1",
            params![character_id, story_id],
            |row| row.get(0),
        )
        .optional()
    }
}

fn map_state_row(row: &rusqlite::Row<'_>) -> Result<CharacterEmotionState, rusqlite::Error> {
    let vector_json: String = row.get(6)?;
    Ok(CharacterEmotionState {
        id: row.get(0)?,
        story_id: row.get(1)?,
        character_id: row.get(2)?,
        character_name: String::new(),
        scene_id: row.get(3)?,
        chapter_number: row.get(4)?,
        beat_index: row.get(5)?,
        vector: serde_json::from_str(&vector_json).unwrap_or_default(),
        intensity: row.get::<_, f64>(7)? as f32,
        active_goal: row.get(8)?,
        defense: row.get(9)?,
        cause_event: row.get(10)?,
        source: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn json<T: serde::Serialize>(v: &T) -> Result<String, rusqlite::Error> {
    serde_json::to_string(v).map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
}

fn json_owned<T: serde::Serialize>(v: &T) -> Result<String, rusqlite::Error> {
    json(v)
}

/// 把读者评审落库（供后台任务调用）。
pub fn reader_review_to_row(
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: Option<i32>,
    review: &ReaderReview,
) -> ReaderResonanceReview {
    ReaderResonanceReview {
        id: Uuid::new_v4().to_string(),
        story_id: story_id.to_string(),
        scene_id: scene_id.map(str::to_string),
        chapter_number,
        persona: review.persona.clone(),
        scores: review.scores.clone(),
        verdict: review.verdict.clone(),
        evidence: Some(
            serde_json::json!({
                "moved_moment": review.moved_moment,
                "flat_moment": review.flat_moment,
            })
            .to_string(),
        ),
        created_at: Local::now().to_rfc3339(),
    }
}

/// 供外部构造探针日志时使用。
pub fn probe_severity_id(severity: ProbeSeverity) -> &'static str {
    severity.id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn setup() -> (DbPool, String, String) {
        let pool = create_test_pool().unwrap();
        let story = crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "情感回路测试".into(),
                description: None,
                genre: Some("言情".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let character = crate::db::repositories::CharacterRepository::new(pool.clone())
            .create(crate::db::dto::CreateCharacterRequest {
                story_id: story.id.clone(),
                name: "林雪".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: None,
                is_auto_generated: None,
                emotional_core: Some("表面冷漠".into()),
                emotional_trigger: None,
                emotional_wound: Some("被父亲当众否认".into()),
                emotional_need: Some("被看见".into()),
            })
            .unwrap();
        (pool, story.id, character.id)
    }

    #[test]
    fn contract_round_trip() {
        let (pool, sid, _) = setup();
        let repo = EmotionCircuitRepository::new(pool.clone());
        let c = EmotionContract {
            story_id: sid.clone(),
            core_question: "能否被爱而不失去自我".into(),
            reader_promise: "看一个人学会为自己活".into(),
            palette: vec!["隐忍的愤怒".into()],
            ending_residue: "苦尽甘来".into(),
            forbidden: vec!["失忆".into()],
            peak_budget: 1,
            version: 1,
        };
        repo.upsert_contract(&c).unwrap();
        let read = repo.get_contract(&sid).unwrap().unwrap();
        assert_eq!(read.core_question, c.core_question);
        assert_eq!(read.palette, c.palette);
    }

    #[test]
    fn state_round_trip_and_latest() {
        let (pool, sid, cid) = setup();
        let repo = EmotionCircuitRepository::new(pool.clone());
        let state = CharacterEmotionState {
            story_id: sid.clone(),
            character_id: cid.clone(),
            vector: crate::emotion_circuit::vector::EmotionVector::from_pairs(&[("shame", 0.8)]),
            intensity: 0.8,
            source: "ingest".into(),
            ..Default::default()
        };
        repo.insert_state(&state).unwrap();
        let states = repo.latest_states(&sid).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].character_name, "林雪");
        assert!(states[0].vector.shame > 0.5);
    }

    #[test]
    fn tension_upsert_and_persist_after_beat() {
        let (pool, story_id, cid) = setup();
        let scene = {
            let repo = crate::db::repositories::SceneRepository::new(pool.clone());
            let mut conn = pool.get().unwrap();
            let tx = conn.transaction().unwrap();
            let scene = repo.create_in_tx(&tx, &story_id, 1, Some("第1章")).unwrap();
            tx.commit().unwrap();
            scene
        };
        let repo = EmotionCircuitRepository::new(pool.clone());
        repo.upsert_tension(&EmotionTension {
            id: "t1".into(),
            story_id: story_id.clone(),
            source_character_id: cid.clone(),
            target_character_id: None,
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            tension_type: "未揭穿的欺骗".into(),
            pressure: 0.7,
            accumulated_beats: 1,
            due_chapter: Some(2),
            status: "open".into(),
            ..Default::default()
        })
        .unwrap();
        let beat = EmotionBeat {
            from: crate::emotion_circuit::vector::EmotionVector::from_pairs(&[("shame", 0.6)]),
            to: crate::emotion_circuit::vector::EmotionVector::from_pairs(&[("anger", 0.7)]),
            trigger_event: "他当众替她承认".into(),
            cost: "她推开他".into(),
            tension_id: Some("t1".into()),
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            ..Default::default()
        };
        repo.persist_after_beat(&story_id, &scene.id, 1, &beat)
            .unwrap();
        let beats = repo.list_scene_beats(&story_id, &scene.id).unwrap();
        assert_eq!(beats.len(), 1);
        assert_eq!(beats[0].beat_index, 1);
        let tensions = repo.list_tensions(&story_id).unwrap();
        assert_eq!(tensions[0].accumulated_beats, 2);
    }

    #[test]
    fn seeds_tensions_from_relationships_once() {
        let (pool, sid, cid) = setup();
        let other = crate::db::repositories::CharacterRepository::new(pool.clone())
            .create(crate::db::dto::CreateCharacterRequest {
                story_id: sid.clone(),
                name: "阿岩".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: None,
                is_auto_generated: None,
                emotional_core: None,
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        crate::db::repositories::CharacterRelationshipRepository::new(pool.clone())
            .create(
                &sid,
                &cid,
                &other.id,
                "师徒",
                None,
                None,
                Some("欺骗"),
                Some(0.9),
                Some("崇拜"),
                Some(0.6),
            )
            .unwrap();
        let repo = EmotionCircuitRepository::new(pool.clone());
        let n = repo.seed_tensions_from_relationships(&sid, 3).unwrap();
        assert_eq!(n, 1);
        let tensions = repo.list_tensions(&sid).unwrap();
        assert_eq!(tensions.len(), 1);
        assert!(tensions[0].tension_type.contains("欺骗"));
        assert_eq!(tensions[0].due_chapter, Some(6));
        assert_eq!(repo.seed_tensions_from_relationships(&sid, 4).unwrap(), 0);
    }

    #[test]
    fn probe_logs_round_trip() {
        let (pool, sid, _) = setup();
        let repo = EmotionCircuitRepository::new(pool.clone());
        let probes = vec![EmotionProbe {
            code: "emotion_causality".into(),
            severity: ProbeSeverity::High,
            passed: false,
            detail: "缺少动作".into(),
        }];
        repo.insert_probe_logs(&sid, Some("sc1"), Some(1), &probes)
            .unwrap();
        let logs = repo.list_probe_logs(&sid, Some("sc1")).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].probe, "emotion_causality");
    }
}
