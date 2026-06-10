#![allow(unused_imports)]
//! LitSeg 叙事感知分段与检索 — Repository 层 (深度融合后)
//!
//! 操作 narrative_structure_positions / conflict_escalations / narrative_chunks
//! 表。 注意: narrative_events / narrative_threads / narrative_structure
//! 已合并到现有表。

use chrono::Local;
use rusqlite::params;

use super::{ConflictEscalation, DbPool, NarrativeChunk, NarrativeStructurePosition};

// ==================== Narrative Structure Position Repository
// ====================

pub struct NarrativeStructurePositionRepository {
    pool: DbPool,
}

impl NarrativeStructurePositionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn insert(&self, row: &NarrativeStructurePosition) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let id = format!("{}_{}", row.story_id, row.event_id);

        conn.execute(
            "INSERT INTO narrative_structure_positions (
                id, story_id, event_id, act_number, act_type, position_in_act,
                dramatic_function, is_narrative_boundary, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                row.story_id,
                row.event_id,
                row.act_number,
                row.act_type,
                row.position_in_act,
                row.dramatic_function,
                row.is_narrative_boundary as i32,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<NarrativeStructurePosition>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, event_id, act_number, act_type, position_in_act,
                dramatic_function, is_narrative_boundary, created_at
             FROM narrative_structure_positions
             WHERE story_id = ?1
             ORDER BY act_number ASC, position_in_act ASC",
        )?;
        let rows = stmt.query_map(params![story_id], |row| {
            let is_boundary: i32 = row.get(7)?;
            Ok(NarrativeStructurePosition {
                id: row.get(0)?,
                story_id: row.get(1)?,
                event_id: row.get(2)?,
                act_number: row.get(3)?,
                act_type: row.get(4)?,
                position_in_act: row.get(5)?,
                dramatic_function: row.get(6)?,
                is_narrative_boundary: is_boundary != 0,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_boundaries(
        &self,
        story_id: &str,
    ) -> Result<Vec<NarrativeStructurePosition>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, event_id, act_number, act_type, position_in_act,
                dramatic_function, is_narrative_boundary, created_at
             FROM narrative_structure_positions
             WHERE story_id = ?1 AND is_narrative_boundary = 1
             ORDER BY act_number ASC, position_in_act ASC",
        )?;
        let rows = stmt.query_map(params![story_id], |row| {
            let is_boundary: i32 = row.get(7)?;
            Ok(NarrativeStructurePosition {
                id: row.get(0)?,
                story_id: row.get(1)?,
                event_id: row.get(2)?,
                act_number: row.get(3)?,
                act_type: row.get(4)?,
                position_in_act: row.get(5)?,
                dramatic_function: row.get(6)?,
                is_narrative_boundary: is_boundary != 0,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM narrative_structure_positions WHERE story_id = ?1",
            params![story_id],
        )?;
        Ok(())
    }
}

// ==================== Conflict Escalation Repository ====================

pub struct ConflictEscalationRepository {
    pool: DbPool,
}

impl ConflictEscalationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn insert(&self, row: &ConflictEscalation) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        conn.execute(
            "INSERT INTO conflict_escalations (
                id, story_id, conflict_type, party_a_ids, party_b_ids,
                intensity_timeline_json, current_intensity, is_escalated, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id,
                row.story_id,
                row.conflict_type,
                row.party_a_ids,
                row.party_b_ids,
                row.intensity_timeline_json,
                row.current_intensity,
                row.is_escalated as i32,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<ConflictEscalation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, conflict_type, party_a_ids, party_b_ids,
                intensity_timeline_json, current_intensity, is_escalated, created_at
             FROM conflict_escalations
             WHERE story_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![story_id], |row| {
            let is_escalated: i32 = row.get(7)?;
            Ok(ConflictEscalation {
                id: row.get(0)?,
                story_id: row.get(1)?,
                conflict_type: row.get(2)?,
                party_a_ids: row.get(3)?,
                party_b_ids: row.get(4)?,
                intensity_timeline_json: row.get(5)?,
                current_intensity: row.get(6)?,
                is_escalated: is_escalated != 0,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM conflict_escalations WHERE story_id = ?1",
            params![story_id],
        )?;
        Ok(())
    }
}

// ==================== Narrative Chunk Repository ====================

pub struct NarrativeChunkRepository {
    pool: DbPool,
}

impl NarrativeChunkRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn insert(&self, row: &NarrativeChunk) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        conn.execute(
            "INSERT INTO narrative_chunks (
                id, story_id, chapter_range_start, chapter_range_end, scene_ids, event_ids,
                text, chunk_type, is_boundary_start, is_boundary_end, thread_ids, vector_id, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.id,
                row.story_id,
                row.chapter_range_start,
                row.chapter_range_end,
                row.scene_ids,
                row.event_ids,
                row.text,
                row.chunk_type,
                row.is_boundary_start as i32,
                row.is_boundary_end as i32,
                row.thread_ids,
                row.vector_id,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<NarrativeChunk>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_range_start, chapter_range_end, scene_ids, event_ids,
                text, chunk_type, is_boundary_start, is_boundary_end, thread_ids, vector_id, created_at
             FROM narrative_chunks
             WHERE story_id = ?1
             ORDER BY chapter_range_start ASC",
        )?;
        let rows = stmt.query_map(params![story_id], |row| {
            let is_start: i32 = row.get(8)?;
            let is_end: i32 = row.get(9)?;
            Ok(NarrativeChunk {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_range_start: row.get(2)?,
                chapter_range_end: row.get(3)?,
                scene_ids: row.get(4)?,
                event_ids: row.get(5)?,
                text: row.get(6)?,
                chunk_type: row.get(7)?,
                is_boundary_start: is_start != 0,
                is_boundary_end: is_end != 0,
                thread_ids: row.get(10)?,
                vector_id: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_by_chunk_type(
        &self,
        story_id: &str,
        chunk_type: &str,
    ) -> Result<Vec<NarrativeChunk>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_range_start, chapter_range_end, scene_ids, event_ids,
                text, chunk_type, is_boundary_start, is_boundary_end, thread_ids, vector_id, created_at
             FROM narrative_chunks
             WHERE story_id = ?1 AND chunk_type = ?2
             ORDER BY chapter_range_start ASC",
        )?;
        let rows = stmt.query_map(params![story_id, chunk_type], |row| {
            let is_start: i32 = row.get(8)?;
            let is_end: i32 = row.get(9)?;
            Ok(NarrativeChunk {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_range_start: row.get(2)?,
                chapter_range_end: row.get(3)?,
                scene_ids: row.get(4)?,
                event_ids: row.get(5)?,
                text: row.get(6)?,
                chunk_type: row.get(7)?,
                is_boundary_start: is_start != 0,
                is_boundary_end: is_end != 0,
                thread_ids: row.get(10)?,
                vector_id: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_boundary_chunks(
        &self,
        story_id: &str,
    ) -> Result<Vec<NarrativeChunk>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_range_start, chapter_range_end, scene_ids, event_ids,
                text, chunk_type, is_boundary_start, is_boundary_end, thread_ids, vector_id, created_at
             FROM narrative_chunks
             WHERE story_id = ?1 AND (is_boundary_start = 1 OR is_boundary_end = 1)
             ORDER BY chapter_range_start ASC",
        )?;
        let rows = stmt.query_map(params![story_id], |row| {
            let is_start: i32 = row.get(8)?;
            let is_end: i32 = row.get(9)?;
            Ok(NarrativeChunk {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_range_start: row.get(2)?,
                chapter_range_end: row.get(3)?,
                scene_ids: row.get(4)?,
                event_ids: row.get(5)?,
                text: row.get(6)?,
                chunk_type: row.get(7)?,
                is_boundary_start: is_start != 0,
                is_boundary_end: is_end != 0,
                thread_ids: row.get(10)?,
                vector_id: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM narrative_chunks WHERE story_id = ?1",
            params![story_id],
        )?;
        Ok(())
    }
}
