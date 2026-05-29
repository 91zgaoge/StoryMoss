#![allow(dead_code)]

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{
    ChapterReadingPower, ChaseDebt, DbPool, GenreProfile, MemoryItem, OverrideContract,
    ReviewIssue, SceneCommit, StoryContract,
};

// ==================== StoryContract Repository ====================

pub struct StoryContractRepository {
    pool: DbPool,
}

impl StoryContractRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        contract_type: &str,
        contract_json: &str,
    ) -> Result<StoryContract, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO story_contracts (id, story_id, contract_type, contract_json, version, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                story_id,
                contract_type,
                contract_json,
                1,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(StoryContract {
            id,
            story_id: story_id.to_string(),
            contract_type: contract_type.to_string(),
            contract_json: contract_json.to_string(),
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<StoryContract>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, contract_type, contract_json, version, created_at, updated_at \
             FROM story_contracts WHERE story_id = ?1 ORDER BY contract_type, version DESC",
        )?;

        let contracts = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryContract {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    contract_type: row.get(2)?,
                    contract_json: row.get(3)?,
                    version: row.get(4)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(contracts)
    }

    pub fn get_by_type(
        &self,
        story_id: &str,
        contract_type: &str,
    ) -> Result<Option<StoryContract>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, contract_type, contract_json, version, created_at, updated_at \
             FROM story_contracts WHERE story_id = ?1 AND contract_type = ?2 ORDER BY version \
             DESC LIMIT 1",
        )?;

        let contract = stmt
            .query_row([story_id, contract_type], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryContract {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    contract_type: row.get(2)?,
                    contract_json: row.get(3)?,
                    version: row.get(4)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(contract)
    }

    pub fn update(&self, id: &str, contract_json: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let now = Local::now().to_rfc3339();

        conn.execute(
            "UPDATE story_contracts SET contract_json = ?2, version = version + 1, updated_at = \
             ?3 WHERE id = ?1",
            params![id, contract_json, now],
        )
    }
}

// ==================== SceneCommit Repository ====================

pub struct SceneCommitRepository {
    pool: DbPool,
}

impl SceneCommitRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        chapter_number: i32,
        status: &str,
    ) -> Result<SceneCommit, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO scene_commits (id, story_id, scene_id, chapter_id, chapter_number, \
             status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_id,
                chapter_number,
                status,
                now.to_rfc3339()
            ],
        )?;

        Ok(SceneCommit {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            chapter_number,
            status: status.to_string(),
            outline_snapshot_json: None,
            review_result_json: None,
            fulfillment_result_json: None,
            accepted_events_json: None,
            state_deltas_json: None,
            entity_deltas_json: None,
            summary_text: None,
            dominant_strand: None,
            projection_status_json: None,
            created_at: now,
        })
    }

    pub fn update_commit(
        &self,
        id: &str,
        status: &str,
        outline_snapshot_json: Option<&str>,
        review_result_json: Option<&str>,
        fulfillment_result_json: Option<&str>,
        accepted_events_json: Option<&str>,
        state_deltas_json: Option<&str>,
        entity_deltas_json: Option<&str>,
        summary_text: Option<&str>,
        dominant_strand: Option<&str>,
        projection_status_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "UPDATE scene_commits SET status = ?2, outline_snapshot_json = ?3, review_result_json \
             = ?4, fulfillment_result_json = ?5, accepted_events_json = ?6, state_deltas_json = \
             ?7, entity_deltas_json = ?8, summary_text = ?9, dominant_strand = ?10, \
             projection_status_json = ?11 WHERE id = ?1",
            params![
                id,
                status,
                outline_snapshot_json,
                review_result_json,
                fulfillment_result_json,
                accepted_events_json,
                state_deltas_json,
                entity_deltas_json,
                summary_text,
                dominant_strand,
                projection_status_json
            ],
        )
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<SceneCommit>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, chapter_number, status, \
             outline_snapshot_json, review_result_json, fulfillment_result_json, \
             accepted_events_json, state_deltas_json, entity_deltas_json, summary_text, \
             dominant_strand, projection_status_json, created_at FROM scene_commits WHERE \
             story_id = ?1 ORDER BY chapter_number DESC",
        )?;

        let commits = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(15)?;
                Ok(SceneCommit {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    chapter_number: row.get(4)?,
                    status: row.get(5)?,
                    outline_snapshot_json: row.get(6)?,
                    review_result_json: row.get(7)?,
                    fulfillment_result_json: row.get(8)?,
                    accepted_events_json: row.get(9)?,
                    state_deltas_json: row.get(10)?,
                    entity_deltas_json: row.get(11)?,
                    summary_text: row.get(12)?,
                    dominant_strand: row.get(13)?,
                    projection_status_json: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(commits)
    }

    pub fn get_latest(&self, story_id: &str) -> Result<Option<SceneCommit>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, chapter_number, status, \
             outline_snapshot_json, review_result_json, fulfillment_result_json, \
             accepted_events_json, state_deltas_json, entity_deltas_json, summary_text, \
             dominant_strand, projection_status_json, created_at FROM scene_commits WHERE \
             story_id = ?1 ORDER BY chapter_number DESC LIMIT 1",
        )?;

        let commit = stmt
            .query_row([story_id], |row| {
                let created_str: String = row.get(15)?;
                Ok(SceneCommit {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    chapter_number: row.get(4)?,
                    status: row.get(5)?,
                    outline_snapshot_json: row.get(6)?,
                    review_result_json: row.get(7)?,
                    fulfillment_result_json: row.get(8)?,
                    accepted_events_json: row.get(9)?,
                    state_deltas_json: row.get(10)?,
                    entity_deltas_json: row.get(11)?,
                    summary_text: row.get(12)?,
                    dominant_strand: row.get(13)?,
                    projection_status_json: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(commit)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<SceneCommit>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, chapter_number, status, \
             outline_snapshot_json, review_result_json, fulfillment_result_json, \
             accepted_events_json, state_deltas_json, entity_deltas_json, summary_text, \
             dominant_strand, projection_status_json, created_at FROM scene_commits WHERE id = ?1",
        )?;

        let commit = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(15)?;
                Ok(SceneCommit {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    chapter_number: row.get(4)?,
                    status: row.get(5)?,
                    outline_snapshot_json: row.get(6)?,
                    review_result_json: row.get(7)?,
                    fulfillment_result_json: row.get(8)?,
                    accepted_events_json: row.get(9)?,
                    state_deltas_json: row.get(10)?,
                    entity_deltas_json: row.get(11)?,
                    summary_text: row.get(12)?,
                    dominant_strand: row.get(13)?,
                    projection_status_json: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(commit)
    }

    pub fn get_by_chapter_id(&self, chapter_id: &str) -> Result<Vec<SceneCommit>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, chapter_number, status, \
             outline_snapshot_json, review_result_json, fulfillment_result_json, \
             accepted_events_json, state_deltas_json, entity_deltas_json, summary_text, \
             dominant_strand, projection_status_json, created_at FROM scene_commits WHERE \
             chapter_id = ?1 ORDER BY created_at DESC",
        )?;

        let commits = stmt
            .query_map([chapter_id], |row| {
                let created_str: String = row.get(15)?;
                Ok(SceneCommit {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    chapter_number: row.get(4)?,
                    status: row.get(5)?,
                    outline_snapshot_json: row.get(6)?,
                    review_result_json: row.get(7)?,
                    fulfillment_result_json: row.get(8)?,
                    accepted_events_json: row.get(9)?,
                    state_deltas_json: row.get(10)?,
                    entity_deltas_json: row.get(11)?,
                    summary_text: row.get(12)?,
                    dominant_strand: row.get(13)?,
                    projection_status_json: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(commits)
    }

    pub fn update_projection_status(
        &self,
        id: &str,
        status_json: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "UPDATE scene_commits SET projection_status_json = ?2 WHERE id = ?1",
            params![id, status_json],
        )
    }
}

// ==================== MemoryItem Repository ====================

pub struct MemoryItemRepository {
    pool: DbPool,
}

impl MemoryItemRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        category: &str,
        subject: Option<&str>,
        field: Option<&str>,
        value: Option<&str>,
        source_chapter: Option<i32>,
        confidence: f32,
    ) -> Result<MemoryItem, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO memory_items (id, story_id, category, subject, field, value, \
             source_chapter, confidence, status, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
             ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                category,
                subject,
                field,
                value,
                source_chapter,
                confidence,
                "active",
                now.to_rfc3339()
            ],
        )?;

        Ok(MemoryItem {
            id,
            story_id: story_id.to_string(),
            category: category.to_string(),
            subject: subject.map(|s| s.to_string()),
            field: field.map(|s| s.to_string()),
            value: value.map(|s| s.to_string()),
            source_chapter,
            confidence,
            status: "active".to_string(),
            updated_at: now,
        })
    }

    pub fn get_active_by_story(&self, story_id: &str) -> Result<Vec<MemoryItem>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, category, subject, field, value, source_chapter, confidence, \
             status, updated_at FROM memory_items WHERE story_id = ?1 AND status = 'active' ORDER \
             BY category, source_chapter DESC",
        )?;

        let items = stmt
            .query_map([story_id], |row| {
                let updated_str: String = row.get(9)?;
                Ok(MemoryItem {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    category: row.get(2)?,
                    subject: row.get(3)?,
                    field: row.get(4)?,
                    value: row.get(5)?,
                    source_chapter: row.get(6)?,
                    confidence: row.get(7)?,
                    status: row.get(8)?,
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn get_conflicts(&self, story_id: &str) -> Result<Vec<MemoryItem>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, category, subject, field, value, source_chapter, confidence, \
             status, updated_at FROM memory_items WHERE story_id = ?1 AND status = 'conflicting'",
        )?;

        let items = stmt
            .query_map([story_id], |row| {
                let updated_str: String = row.get(9)?;
                Ok(MemoryItem {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    category: row.get(2)?,
                    subject: row.get(3)?,
                    field: row.get(4)?,
                    value: row.get(5)?,
                    source_chapter: row.get(6)?,
                    confidence: row.get(7)?,
                    status: row.get(8)?,
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn update_status(&self, id: &str, status: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let now = Local::now().to_rfc3339();

        conn.execute(
            "UPDATE memory_items SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status, now],
        )
    }
}

// ==================== ChapterReadingPower Repository ====================

pub struct ChapterReadingPowerRepository {
    pool: DbPool,
}

impl ChapterReadingPowerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn save(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_number: i32,
        hook_type: Option<&str>,
        hook_strength: &str,
        coolpoint_patterns_json: Option<&str>,
        micropayoffs_json: Option<&str>,
        is_transition: bool,
    ) -> Result<ChapterReadingPower, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO chapter_reading_power (id, story_id, scene_id, \
             chapter_number, hook_type, hook_strength, coolpoint_patterns_json, \
             micropayoffs_json, is_transition, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
             ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_number,
                hook_type,
                hook_strength,
                coolpoint_patterns_json,
                micropayoffs_json,
                if is_transition { 1 } else { 0 },
                now.to_rfc3339()
            ],
        )?;

        Ok(ChapterReadingPower {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_number,
            hook_type: hook_type.map(|s| s.to_string()),
            hook_strength: hook_strength.to_string(),
            coolpoint_patterns_json: coolpoint_patterns_json.map(|s| s.to_string()),
            micropayoffs_json: micropayoffs_json.map(|s| s.to_string()),
            hard_violations_json: None,
            soft_suggestions_json: None,
            is_transition,
            override_count: 0,
            debt_balance: 0.0,
            created_at: now,
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
        limit: i64,
    ) -> Result<Vec<ChapterReadingPower>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_number, hook_type, hook_strength, \
             coolpoint_patterns_json, micropayoffs_json, hard_violations_json, \
             soft_suggestions_json, is_transition, override_count, debt_balance, created_at FROM \
             chapter_reading_power WHERE story_id = ?1 ORDER BY chapter_number DESC LIMIT ?2",
        )?;

        let items = stmt
            .query_map([story_id, limit.to_string().as_str()], |row| {
                let created_str: String = row.get(13)?;
                Ok(ChapterReadingPower {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_number: row.get(3)?,
                    hook_type: row.get(4)?,
                    hook_strength: row.get(5)?,
                    coolpoint_patterns_json: row.get(6)?,
                    micropayoffs_json: row.get(7)?,
                    hard_violations_json: row.get(8)?,
                    soft_suggestions_json: row.get(9)?,
                    is_transition: row.get::<_, i32>(10)? != 0,
                    override_count: row.get(11)?,
                    debt_balance: row.get(12)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }
}

// ==================== ChaseDebt Repository ====================

pub struct ChaseDebtRepository {
    pool: DbPool,
}

impl ChaseDebtRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        debt_type: &str,
        original_amount: f64,
        interest_rate: f64,
        source_chapter: i32,
        due_chapter: i32,
    ) -> Result<ChaseDebt, rusqlite::Error> {
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO chase_debt (story_id, debt_type, original_amount, current_amount, \
             interest_rate, source_chapter, due_chapter, status, created_at) VALUES (?1, ?2, ?3, \
             ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                story_id,
                debt_type,
                original_amount,
                interest_rate,
                source_chapter,
                due_chapter,
                "active",
                now.to_rfc3339()
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(ChaseDebt {
            id,
            story_id: story_id.to_string(),
            debt_type: debt_type.to_string(),
            original_amount,
            current_amount: original_amount,
            interest_rate,
            source_chapter,
            due_chapter,
            override_contract_id: None,
            status: "active".to_string(),
            created_at: now,
        })
    }

    pub fn get_active_by_story(&self, story_id: &str) -> Result<Vec<ChaseDebt>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, debt_type, original_amount, current_amount, interest_rate, \
             source_chapter, due_chapter, override_contract_id, status, created_at FROM \
             chase_debt WHERE story_id = ?1 AND status = 'active' ORDER BY due_chapter ASC",
        )?;

        let debts = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(10)?;
                Ok(ChaseDebt {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    debt_type: row.get(2)?,
                    original_amount: row.get(3)?,
                    current_amount: row.get(4)?,
                    interest_rate: row.get(5)?,
                    source_chapter: row.get(6)?,
                    due_chapter: row.get(7)?,
                    override_contract_id: row.get(8)?,
                    status: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(debts)
    }

    pub fn get_overdue(
        &self,
        story_id: &str,
        current_chapter: i32,
    ) -> Result<Vec<ChaseDebt>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, debt_type, original_amount, current_amount, interest_rate, \
             source_chapter, due_chapter, override_contract_id, status, created_at FROM \
             chase_debt WHERE story_id = ?1 AND status = 'active' AND due_chapter < ?2 ORDER BY \
             due_chapter ASC",
        )?;

        let debts = stmt
            .query_map([story_id, &current_chapter.to_string()], |row| {
                let created_str: String = row.get(10)?;
                Ok(ChaseDebt {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    debt_type: row.get(2)?,
                    original_amount: row.get(3)?,
                    current_amount: row.get(4)?,
                    interest_rate: row.get(5)?,
                    source_chapter: row.get(6)?,
                    due_chapter: row.get(7)?,
                    override_contract_id: row.get(8)?,
                    status: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(debts)
    }

    pub fn apply_interest(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "UPDATE chase_debt SET current_amount = current_amount * (1 + interest_rate) WHERE \
             story_id = ?1 AND status = 'active'",
            [story_id],
        )
    }

    pub fn mark_paid(&self, id: i64) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute("UPDATE chase_debt SET status = 'paid' WHERE id = ?1", [id])
    }
}

// ==================== OverrideContract Repository ====================

pub struct OverrideContractRepository {
    pool: DbPool,
}

impl OverrideContractRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        chapter_number: i32,
        constraint_type: &str,
        constraint_id: &str,
        rationale_type: &str,
        rationale_text: &str,
        payback_plan: &str,
        due_chapter: i32,
    ) -> Result<OverrideContract, rusqlite::Error> {
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO override_contracts (story_id, chapter_number, constraint_type, \
             constraint_id, rationale_type, rationale_text, payback_plan, due_chapter, status, \
             created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                story_id,
                chapter_number,
                constraint_type,
                constraint_id,
                rationale_type,
                rationale_text,
                payback_plan,
                due_chapter,
                "pending",
                now.to_rfc3339()
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(OverrideContract {
            id,
            story_id: story_id.to_string(),
            chapter_number,
            constraint_type: constraint_type.to_string(),
            constraint_id: constraint_id.to_string(),
            rationale_type: rationale_type.to_string(),
            rationale_text: rationale_text.to_string(),
            payback_plan: payback_plan.to_string(),
            due_chapter,
            status: "pending".to_string(),
            fulfilled_at: None,
            created_at: now,
        })
    }

    pub fn get_pending_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<OverrideContract>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, constraint_type, constraint_id, rationale_type, \
             rationale_text, payback_plan, due_chapter, status, fulfilled_at, created_at FROM \
             override_contracts WHERE story_id = ?1 AND status = 'pending' ORDER BY due_chapter \
             ASC",
        )?;

        let contracts = stmt
            .query_map([story_id], |row| {
                let fulfilled_str: Option<String> = row.get(10)?;
                let created_str: String = row.get(11)?;
                Ok(OverrideContract {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    constraint_type: row.get(3)?,
                    constraint_id: row.get(4)?,
                    rationale_type: row.get(5)?,
                    rationale_text: row.get(6)?,
                    payback_plan: row.get(7)?,
                    due_chapter: row.get(8)?,
                    status: row.get(9)?,
                    fulfilled_at: fulfilled_str.and_then(|s| s.parse().ok()),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(contracts)
    }

    pub fn mark_fulfilled(&self, id: i64) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let now = Local::now().to_rfc3339();

        conn.execute(
            "UPDATE override_contracts SET status = 'fulfilled', fulfilled_at = ?2 WHERE id = ?1",
            params![id, now],
        )
    }
}

// ==================== ReviewIssue Repository ====================

pub struct ReviewIssueRepository {
    pool: DbPool,
}

impl ReviewIssueRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_number: i32,
        severity: &str,
        category: &str,
        location: Option<&str>,
        description: &str,
        evidence: Option<&str>,
        fix_hint: Option<&str>,
        blocking: bool,
    ) -> Result<ReviewIssue, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO review_issues (id, story_id, scene_id, chapter_number, severity, \
             category, location, description, evidence, fix_hint, blocking, resolved, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_number,
                severity,
                category,
                location,
                description,
                evidence,
                fix_hint,
                if blocking { 1 } else { 0 },
                0,
                now.to_rfc3339()
            ],
        )?;

        Ok(ReviewIssue {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_number,
            severity: severity.to_string(),
            category: category.to_string(),
            location: location.map(|s| s.to_string()),
            description: description.to_string(),
            evidence: evidence.map(|s| s.to_string()),
            fix_hint: fix_hint.map(|s| s.to_string()),
            blocking,
            resolved: false,
            created_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<ReviewIssue>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_number, severity, category, location, \
             description, evidence, fix_hint, blocking, resolved, created_at FROM review_issues \
             WHERE story_id = ?1 ORDER BY chapter_number DESC, CASE severity WHEN 'critical' THEN \
             0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END",
        )?;

        let issues = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(12)?;
                Ok(ReviewIssue {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_number: row.get(3)?,
                    severity: row.get(4)?,
                    category: row.get(5)?,
                    location: row.get(6)?,
                    description: row.get(7)?,
                    evidence: row.get(8)?,
                    fix_hint: row.get(9)?,
                    blocking: row.get::<_, i32>(10)? != 0,
                    resolved: row.get::<_, i32>(11)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    pub fn get_blocking(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Vec<ReviewIssue>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_number, severity, category, location, \
             description, evidence, fix_hint, blocking, resolved, created_at FROM review_issues \
             WHERE story_id = ?1 AND chapter_number = ?2 AND blocking = 1 AND resolved = 0",
        )?;

        let issues = stmt
            .query_map([story_id, &chapter_number.to_string()], |row| {
                let created_str: String = row.get(12)?;
                Ok(ReviewIssue {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_number: row.get(3)?,
                    severity: row.get(4)?,
                    category: row.get(5)?,
                    location: row.get(6)?,
                    description: row.get(7)?,
                    evidence: row.get(8)?,
                    fix_hint: row.get(9)?,
                    blocking: row.get::<_, i32>(10)? != 0,
                    resolved: row.get::<_, i32>(11)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    pub fn mark_resolved(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute("UPDATE review_issues SET resolved = 1 WHERE id = ?1", [id])
    }
}

// ==================== GenreProfile Repository ====================

pub struct GenreProfileRepository {
    pool: DbPool,
}

impl GenreProfileRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        genre_name: &str,
        canonical_name: &str,
        aliases_json: Option<&str>,
        core_tone: Option<&str>,
        pacing_strategy: Option<&str>,
        anti_patterns_json: Option<&str>,
        reference_tables_json: Option<&str>,
    ) -> Result<GenreProfile, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO genre_profiles (id, genre_name, canonical_name, aliases_json, core_tone, \
             pacing_strategy, anti_patterns_json, reference_tables_json, is_builtin, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                genre_name,
                canonical_name,
                aliases_json,
                core_tone,
                pacing_strategy,
                anti_patterns_json,
                reference_tables_json,
                1,
                now.to_rfc3339()
            ],
        )?;

        Ok(GenreProfile {
            id,
            genre_name: genre_name.to_string(),
            canonical_name: canonical_name.to_string(),
            aliases_json: aliases_json.map(|s| s.to_string()),
            core_tone: core_tone.map(|s| s.to_string()),
            pacing_strategy: pacing_strategy.map(|s| s.to_string()),
            anti_patterns_json: anti_patterns_json.map(|s| s.to_string()),
            reference_tables_json: reference_tables_json.map(|s| s.to_string()),
            is_builtin: true,
            created_at: now,
        })
    }

    pub fn get_all(&self) -> Result<Vec<GenreProfile>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, genre_name, canonical_name, aliases_json, core_tone, pacing_strategy, \
             anti_patterns_json, reference_tables_json, is_builtin, created_at FROM \
             genre_profiles ORDER BY genre_name",
        )?;

        let profiles = stmt
            .query_map([], |row| {
                let created_str: String = row.get(9)?;
                Ok(GenreProfile {
                    id: row.get(0)?,
                    genre_name: row.get(1)?,
                    canonical_name: row.get(2)?,
                    aliases_json: row.get(3)?,
                    core_tone: row.get(4)?,
                    pacing_strategy: row.get(5)?,
                    anti_patterns_json: row.get(6)?,
                    reference_tables_json: row.get(7)?,
                    is_builtin: row.get::<_, i32>(8)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }

    pub fn get_by_name(&self, genre_name: &str) -> Result<Option<GenreProfile>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, genre_name, canonical_name, aliases_json, core_tone, pacing_strategy, \
             anti_patterns_json, reference_tables_json, is_builtin, created_at FROM \
             genre_profiles WHERE genre_name = ?1 OR canonical_name = ?1 LIMIT 1",
        )?;

        let profile = stmt
            .query_row([genre_name], |row| {
                let created_str: String = row.get(9)?;
                Ok(GenreProfile {
                    id: row.get(0)?,
                    genre_name: row.get(1)?,
                    canonical_name: row.get(2)?,
                    aliases_json: row.get(3)?,
                    core_tone: row.get(4)?,
                    pacing_strategy: row.get(5)?,
                    anti_patterns_json: row.get(6)?,
                    reference_tables_json: row.get(7)?,
                    is_builtin: row.get::<_, i32>(8)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(profile)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<GenreProfile>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, genre_name, canonical_name, aliases_json, core_tone, pacing_strategy, \
             anti_patterns_json, reference_tables_json, is_builtin, created_at FROM \
             genre_profiles WHERE id = ?1 LIMIT 1",
        )?;

        let profile = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(9)?;
                Ok(GenreProfile {
                    id: row.get(0)?,
                    genre_name: row.get(1)?,
                    canonical_name: row.get(2)?,
                    aliases_json: row.get(3)?,
                    core_tone: row.get(4)?,
                    pacing_strategy: row.get(5)?,
                    anti_patterns_json: row.get(6)?,
                    reference_tables_json: row.get(7)?,
                    is_builtin: row.get::<_, i32>(8)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(profile)
    }

    pub fn update(
        &self,
        id: &str,
        genre_name: &str,
        canonical_name: &str,
        aliases_json: Option<&str>,
        core_tone: Option<&str>,
        pacing_strategy: Option<&str>,
        anti_patterns_json: Option<&str>,
        reference_tables_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "UPDATE genre_profiles SET genre_name = ?2, canonical_name = ?3, aliases_json = ?4, \
             core_tone = ?5, pacing_strategy = ?6, anti_patterns_json = ?7, reference_tables_json \
             = ?8 WHERE id = ?1",
            params![
                id,
                genre_name,
                canonical_name,
                aliases_json,
                core_tone,
                pacing_strategy,
                anti_patterns_json,
                reference_tables_json
            ],
        )
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute("DELETE FROM genre_profiles WHERE id = ?1", [id])
    }
}
