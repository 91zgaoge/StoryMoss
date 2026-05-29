#![allow(dead_code)]

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{
    Blueprint, CreateBlueprintRequest, DbPool, Draft, DraftSource, DraftStatus, LlmCall,
    PipelineReview, PostProcessRun, PostProcessStatus, PostProcessStep, RecordLlmCallRequest,
    ReviewDimension, ReviewIssueItem, Revision, RevisionStatus, RevisionType, StepStatus,
    UpdateBlueprintRequest,
};

// ==================== Blueprint Repository ====================

pub struct BlueprintRepository {
    pool: DbPool,
}

impl BlueprintRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateBlueprintRequest) -> Result<Blueprint, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let key_events = req
            .key_events
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let characters = req
            .characters
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO blueprints (id, story_id, chapter_number, title, role, purpose, \
             key_events, characters, suspense_hook, user_guidance, notes, knowledge_query_hint, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &id,
                &req.story_id,
                req.chapter_number,
                req.title,
                req.role,
                req.purpose,
                &key_events,
                &characters,
                req.suspense_hook,
                req.user_guidance,
                None::<String>,
                req.knowledge_query_hint,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Blueprint {
            id,
            story_id: req.story_id,
            chapter_number: req.chapter_number,
            title: req.title,
            role: req.role,
            purpose: req.purpose,
            key_events,
            characters,
            suspense_hook: req.suspense_hook,
            user_guidance: req.user_guidance,
            notes: None,
            notes_updated_at: None,
            knowledge_query_hint: req.knowledge_query_hint,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Blueprint>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, role, purpose, key_events, characters, \
             suspense_hook, user_guidance, notes, notes_updated_at, knowledge_query_hint, \
             created_at, updated_at
             FROM blueprints WHERE story_id = ?1 ORDER BY chapter_number",
        )?;

        let blueprints = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(13)?;
                let updated_str: String = row.get(14)?;
                Ok(Blueprint {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    title: row.get(3)?,
                    role: row.get(4)?,
                    purpose: row.get(5)?,
                    key_events: row.get(6)?,
                    characters: row.get(7)?,
                    suspense_hook: row.get(8)?,
                    user_guidance: row.get(9)?,
                    notes: row.get(10)?,
                    notes_updated_at: row.get(11)?,
                    knowledge_query_hint: row.get(12)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(blueprints)
    }

    pub fn get_by_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<Blueprint>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, role, purpose, key_events, characters, \
             suspense_hook, user_guidance, notes, notes_updated_at, knowledge_query_hint, \
             created_at, updated_at
             FROM blueprints WHERE story_id = ?1 AND chapter_number = ?2",
        )?;

        let blueprint = stmt
            .query_row([story_id, chapter_number.to_string().as_str()], |row| {
                let created_str: String = row.get(13)?;
                let updated_str: String = row.get(14)?;
                Ok(Blueprint {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    title: row.get(3)?,
                    role: row.get(4)?,
                    purpose: row.get(5)?,
                    key_events: row.get(6)?,
                    characters: row.get(7)?,
                    suspense_hook: row.get(8)?,
                    user_guidance: row.get(9)?,
                    notes: row.get(10)?,
                    notes_updated_at: row.get(11)?,
                    knowledge_query_hint: row.get(12)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(blueprint)
    }

    pub fn update(&self, id: &str, req: UpdateBlueprintRequest) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let key_events = req
            .key_events
            .map(|v| serde_json::to_string(&v).unwrap_or_default());
        let characters = req
            .characters
            .map(|v| serde_json::to_string(&v).unwrap_or_default());
        let notes_updated_at = if req.notes.is_some() {
            Some(now.clone())
        } else {
            None::<String>
        };

        let count = conn.execute(
            "UPDATE blueprints SET
                title = COALESCE(?2, title),
                role = COALESCE(?3, role),
                purpose = COALESCE(?4, purpose),
                key_events = COALESCE(?5, key_events),
                characters = COALESCE(?6, characters),
                suspense_hook = COALESCE(?7, suspense_hook),
                user_guidance = COALESCE(?8, user_guidance),
                notes = COALESCE(?9, notes),
                notes_updated_at = COALESCE(?10, notes_updated_at),
                knowledge_query_hint = COALESCE(?11, knowledge_query_hint),
                updated_at = ?12
             WHERE id = ?1",
            params![
                id,
                req.title,
                req.role,
                req.purpose,
                key_events,
                characters,
                req.suspense_hook,
                req.user_guidance,
                req.notes,
                notes_updated_at,
                req.knowledge_query_hint,
                now
            ],
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM blueprints WHERE id = ?1", [id])?;
        Ok(count)
    }
}

// ==================== Draft Repository ====================

pub struct DraftRepository {
    pool: DbPool,
}

impl DraftRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        chapter_number: i32,
        version: i32,
        status: DraftStatus,
        source: DraftSource,
        content: &str,
        word_count: i32,
        model_used: Option<&str>,
        cost: Option<f64>,
        metadata: Option<&str>,
    ) -> Result<Draft, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO drafts (id, story_id, chapter_number, version, status, source, content, \
             word_count, model_used, cost, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &id,
                story_id,
                chapter_number,
                version,
                status.to_string(),
                source.to_string(),
                content,
                word_count,
                model_used,
                cost,
                metadata,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Draft {
            id,
            story_id: story_id.to_string(),
            chapter_number,
            version,
            status,
            source,
            content: content.to_string(),
            word_count,
            model_used: model_used.map(|s| s.to_string()),
            cost,
            metadata: metadata.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Draft>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, version, status, source, content, word_count, \
             model_used, cost, metadata, created_at, updated_at
             FROM drafts WHERE id = ?1",
        )?;

        let draft = stmt
            .query_row([id], |row| {
                let status_str: String = row.get(4)?;
                let source_str: String = row.get(5)?;
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Draft {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    version: row.get(3)?,
                    status: status_str.parse().unwrap_or(DraftStatus::Draft),
                    source: source_str.parse().unwrap_or(DraftSource::Write),
                    content: row.get(6)?,
                    word_count: row.get(7)?,
                    model_used: row.get(8)?,
                    cost: row.get(9)?,
                    metadata: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(draft)
    }

    pub fn get_by_story_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Vec<Draft>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, version, status, source, content, word_count, \
             model_used, cost, metadata, created_at, updated_at
             FROM drafts WHERE story_id = ?1 AND chapter_number = ?2 ORDER BY version DESC",
        )?;

        let drafts = stmt
            .query_map([story_id, chapter_number.to_string().as_str()], |row| {
                let status_str: String = row.get(4)?;
                let source_str: String = row.get(5)?;
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Draft {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    version: row.get(3)?,
                    status: status_str.parse().unwrap_or(DraftStatus::Draft),
                    source: source_str.parse().unwrap_or(DraftSource::Write),
                    content: row.get(6)?,
                    word_count: row.get(7)?,
                    model_used: row.get(8)?,
                    cost: row.get(9)?,
                    metadata: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(drafts)
    }

    pub fn get_latest_by_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<Draft>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, version, status, source, content, word_count, \
             model_used, cost, metadata, created_at, updated_at
             FROM drafts WHERE story_id = ?1 AND chapter_number = ?2 ORDER BY version DESC LIMIT 1",
        )?;

        let draft = stmt
            .query_row([story_id, chapter_number.to_string().as_str()], |row| {
                let status_str: String = row.get(4)?;
                let source_str: String = row.get(5)?;
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Draft {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    version: row.get(3)?,
                    status: status_str.parse().unwrap_or(DraftStatus::Draft),
                    source: source_str.parse().unwrap_or(DraftSource::Write),
                    content: row.get(6)?,
                    word_count: row.get(7)?,
                    model_used: row.get(8)?,
                    cost: row.get(9)?,
                    metadata: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(draft)
    }

    pub fn get_finalized_by_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<Draft>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, version, status, source, content, word_count, \
             model_used, cost, metadata, created_at, updated_at
             FROM drafts WHERE story_id = ?1 AND chapter_number = ?2 AND status = 'finalized' \
             ORDER BY version DESC LIMIT 1",
        )?;

        let draft = stmt
            .query_row([story_id, chapter_number.to_string().as_str()], |row| {
                let status_str: String = row.get(4)?;
                let source_str: String = row.get(5)?;
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Draft {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    version: row.get(3)?,
                    status: status_str.parse().unwrap_or(DraftStatus::Finalized),
                    source: source_str.parse().unwrap_or(DraftSource::Write),
                    content: row.get(6)?,
                    word_count: row.get(7)?,
                    model_used: row.get(8)?,
                    cost: row.get(9)?,
                    metadata: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(draft)
    }

    pub fn update_status(&self, id: &str, status: DraftStatus) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE drafts SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status.to_string(), now],
        )?;
        Ok(count)
    }

    pub fn update_content(
        &self,
        id: &str,
        content: &str,
        word_count: i32,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE drafts SET content = ?2, word_count = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, content, word_count, now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM drafts WHERE id = ?1", [id])?;
        Ok(count)
    }

    pub fn delete_by_story_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute(
            "DELETE FROM drafts WHERE story_id = ?1 AND chapter_number = ?2",
            [story_id, chapter_number.to_string().as_str()],
        )?;
        Ok(count)
    }
}

// ==================== Revision Repository ====================

pub struct RevisionRepository {
    pool: DbPool,
}

impl RevisionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        draft_id: &str,
        revision_index: i32,
        revision_type: RevisionType,
        user_prompt: Option<&str>,
        original_content: &str,
        revised_content: &str,
        word_count: i32,
        change_summary: Option<&str>,
        model_used: Option<&str>,
        cost: Option<f64>,
        metadata: Option<&str>,
    ) -> Result<Revision, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO revisions (id, story_id, draft_id, revision_index, revision_type, \
             status, user_prompt, original_content, revised_content, word_count, change_summary, \
             model_used, cost, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                &id,
                story_id,
                draft_id,
                revision_index,
                revision_type.to_string(),
                "pending",
                user_prompt,
                original_content,
                revised_content,
                word_count,
                change_summary,
                model_used,
                cost,
                metadata,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Revision {
            id,
            story_id: story_id.to_string(),
            draft_id: draft_id.to_string(),
            revision_index,
            revision_type,
            status: RevisionStatus::Pending,
            user_prompt: user_prompt.map(|s| s.to_string()),
            original_content: original_content.to_string(),
            revised_content: revised_content.to_string(),
            word_count,
            change_summary: change_summary.map(|s| s.to_string()),
            model_used: model_used.map(|s| s.to_string()),
            cost,
            metadata: metadata.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_draft(&self, draft_id: &str) -> Result<Vec<Revision>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, revision_index, revision_type, status, user_prompt, \
             original_content, revised_content, word_count, change_summary, model_used, cost, \
             metadata, created_at, updated_at
             FROM revisions WHERE draft_id = ?1 ORDER BY revision_index",
        )?;

        let revisions = stmt
            .query_map([draft_id], |row| {
                let type_str: String = row.get(4)?;
                let status_str: String = row.get(5)?;
                let created_str: String = row.get(14)?;
                let updated_str: String = row.get(15)?;
                Ok(Revision {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    revision_index: row.get(3)?,
                    revision_type: type_str.parse().unwrap_or(RevisionType::Refine),
                    status: status_str.parse().unwrap_or(RevisionStatus::Pending),
                    user_prompt: row.get(6)?,
                    original_content: row.get(7)?,
                    revised_content: row.get(8)?,
                    word_count: row.get(9)?,
                    change_summary: row.get(10)?,
                    model_used: row.get(11)?,
                    cost: row.get(12)?,
                    metadata: row.get(13)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(revisions)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Revision>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, revision_index, revision_type, status, user_prompt, \
             original_content, revised_content, word_count, change_summary, model_used, cost, \
             metadata, created_at, updated_at
             FROM revisions WHERE id = ?1",
        )?;

        let revision = stmt
            .query_row([id], |row| {
                let type_str: String = row.get(4)?;
                let status_str: String = row.get(5)?;
                let created_str: String = row.get(14)?;
                let updated_str: String = row.get(15)?;
                Ok(Revision {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    revision_index: row.get(3)?,
                    revision_type: type_str.parse().unwrap_or(RevisionType::Refine),
                    status: status_str.parse().unwrap_or(RevisionStatus::Pending),
                    user_prompt: row.get(6)?,
                    original_content: row.get(7)?,
                    revised_content: row.get(8)?,
                    word_count: row.get(9)?,
                    change_summary: row.get(10)?,
                    model_used: row.get(11)?,
                    cost: row.get(12)?,
                    metadata: row.get(13)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(revision)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: RevisionStatus,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE revisions SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status.to_string(), now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM revisions WHERE id = ?1", [id])?;
        Ok(count)
    }
}

// ==================== Pipeline Review Repository ====================

pub struct PipelineReviewRepository {
    pool: DbPool,
}

impl PipelineReviewRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        draft_id: &str,
        review_index: i32,
        content: &str,
        dimensions: Option<&[ReviewDimension]>,
        issues: Option<&[ReviewIssueItem]>,
        overall_score: Option<f32>,
        review_focus: Option<&str>,
        model_used: Option<&str>,
        cost: Option<f64>,
        metadata: Option<&str>,
    ) -> Result<PipelineReview, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let dimensions_json = dimensions.map(|d| serde_json::to_string(d).unwrap_or_default());
        let issues_json = issues.map(|i| serde_json::to_string(i).unwrap_or_default());

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO reviews (id, story_id, draft_id, review_index, content, dimensions, \
             issues, overall_score, review_focus, model_used, cost, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &id,
                story_id,
                draft_id,
                review_index,
                content,
                dimensions_json,
                issues_json,
                overall_score,
                review_focus,
                model_used,
                cost,
                metadata,
                now.to_rfc3339()
            ],
        )?;

        Ok(PipelineReview {
            id,
            story_id: story_id.to_string(),
            draft_id: draft_id.to_string(),
            review_index,
            content: content.to_string(),
            dimensions: dimensions_json,
            issues: issues_json,
            overall_score,
            review_focus: review_focus.map(|s| s.to_string()),
            model_used: model_used.map(|s| s.to_string()),
            cost,
            metadata: metadata.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub fn get_by_draft(&self, draft_id: &str) -> Result<Vec<PipelineReview>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, review_index, content, dimensions, issues, \
             overall_score, review_focus, model_used, cost, metadata, created_at
             FROM reviews WHERE draft_id = ?1 ORDER BY review_index",
        )?;

        let reviews = stmt
            .query_map([draft_id], |row| {
                let created_str: String = row.get(12)?;
                Ok(PipelineReview {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    review_index: row.get(3)?,
                    content: row.get(4)?,
                    dimensions: row.get(5)?,
                    issues: row.get(6)?,
                    overall_score: row.get(7)?,
                    review_focus: row.get(8)?,
                    model_used: row.get(9)?,
                    cost: row.get(10)?,
                    metadata: row.get(11)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reviews)
    }

    pub fn get_latest_by_draft(
        &self,
        draft_id: &str,
    ) -> Result<Option<PipelineReview>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, review_index, content, dimensions, issues, \
             overall_score, review_focus, model_used, cost, metadata, created_at
             FROM reviews WHERE draft_id = ?1 ORDER BY review_index DESC LIMIT 1",
        )?;

        let review = stmt
            .query_row([draft_id], |row| {
                let created_str: String = row.get(12)?;
                Ok(PipelineReview {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    review_index: row.get(3)?,
                    content: row.get(4)?,
                    dimensions: row.get(5)?,
                    issues: row.get(6)?,
                    overall_score: row.get(7)?,
                    review_focus: row.get(8)?,
                    model_used: row.get(9)?,
                    cost: row.get(10)?,
                    metadata: row.get(11)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(review)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM reviews WHERE id = ?1", [id])?;
        Ok(count)
    }
}

// ==================== Post Process Repository ====================

pub struct PostProcessRepository {
    pool: DbPool,
}

impl PostProcessRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ---- Run operations ----

    pub fn create_run(
        &self,
        story_id: &str,
        chapter_number: i32,
        source_label: &str,
        scope: Option<&str>,
    ) -> Result<PostProcessRun, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO post_process_runs (id, story_id, chapter_number, source_label, scope, \
             status, started_at, completed_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &id,
                story_id,
                chapter_number,
                source_label,
                scope,
                "running",
                now.to_rfc3339(),
                None::<String>,
                None::<String>
            ],
        )?;

        Ok(PostProcessRun {
            id,
            story_id: story_id.to_string(),
            chapter_number,
            source_label: source_label.to_string(),
            scope: scope.map(|s| s.to_string()),
            status: PostProcessStatus::Running,
            started_at: now,
            completed_at: None,
            error_message: None,
        })
    }

    pub fn get_run_by_id(&self, id: &str) -> Result<Option<PostProcessRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, source_label, scope, status, started_at, \
             completed_at, error_message
             FROM post_process_runs WHERE id = ?1",
        )?;

        let run = stmt
            .query_row([id], |row| {
                let status_str: String = row.get(5)?;
                let started_str: String = row.get(6)?;
                let completed_str: Option<String> = row.get(7)?;
                Ok(PostProcessRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    source_label: row.get(3)?,
                    scope: row.get(4)?,
                    status: status_str.parse().unwrap_or(PostProcessStatus::Running),
                    started_at: started_str.parse().unwrap_or_else(|_| Local::now()),
                    completed_at: completed_str.and_then(|s| s.parse().ok()),
                    error_message: row.get(8)?,
                })
            })
            .optional()?;

        Ok(run)
    }

    pub fn get_runs_by_story_chapter(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Vec<PostProcessRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, source_label, scope, status, started_at, \
             completed_at, error_message
             FROM post_process_runs WHERE story_id = ?1 AND chapter_number = ?2 ORDER BY \
             started_at DESC",
        )?;

        let runs = stmt
            .query_map([story_id, chapter_number.to_string().as_str()], |row| {
                let status_str: String = row.get(5)?;
                let started_str: String = row.get(6)?;
                let completed_str: Option<String> = row.get(7)?;
                Ok(PostProcessRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    source_label: row.get(3)?,
                    scope: row.get(4)?,
                    status: status_str.parse().unwrap_or(PostProcessStatus::Running),
                    started_at: started_str.parse().unwrap_or_else(|_| Local::now()),
                    completed_at: completed_str.and_then(|s| s.parse().ok()),
                    error_message: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(runs)
    }

    pub fn update_run_status(
        &self,
        id: &str,
        status: PostProcessStatus,
        error_message: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let completed_at = match status {
            PostProcessStatus::Completed
            | PostProcessStatus::Failed
            | PostProcessStatus::Partial => Some(now.clone()),
            _ => None::<String>,
        };

        let count = conn.execute(
            "UPDATE post_process_runs SET status = ?2, error_message = ?3, completed_at = ?4 \
             WHERE id = ?1",
            params![id, status.to_string(), error_message, completed_at],
        )?;
        Ok(count)
    }

    // ---- Step operations ----

    pub fn create_step(
        &self,
        run_id: &str,
        step_key: &str,
        step_label: &str,
        critical: bool,
    ) -> Result<PostProcessStep, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO post_process_steps (id, run_id, step_key, step_label, status, critical, \
             log_output, error_message, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                run_id,
                step_key,
                step_label,
                "pending",
                if critical { 1 } else { 0 },
                None::<String>,
                None::<String>,
                None::<String>,
                None::<String>
            ],
        )?;

        Ok(PostProcessStep {
            id,
            run_id: run_id.to_string(),
            step_key: step_key.to_string(),
            step_label: step_label.to_string(),
            status: StepStatus::Pending,
            critical,
            log_output: None,
            error_message: None,
            started_at: None,
            completed_at: None,
        })
    }

    pub fn get_steps_by_run(&self, run_id: &str) -> Result<Vec<PostProcessStep>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, run_id, step_key, step_label, status, critical, log_output, \
             error_message, started_at, completed_at
             FROM post_process_steps WHERE run_id = ?1 ORDER BY rowid",
        )?;

        let steps = stmt
            .query_map([run_id], |row| {
                let status_str: String = row.get(4)?;
                let critical_i: i32 = row.get(5)?;
                let started_str: Option<String> = row.get(8)?;
                let completed_str: Option<String> = row.get(9)?;
                Ok(PostProcessStep {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    step_key: row.get(2)?,
                    step_label: row.get(3)?,
                    status: status_str.parse().unwrap_or(StepStatus::Pending),
                    critical: critical_i != 0,
                    log_output: row.get(6)?,
                    error_message: row.get(7)?,
                    started_at: started_str.and_then(|s| s.parse().ok()),
                    completed_at: completed_str.and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(steps)
    }

    pub fn update_step_status(
        &self,
        id: &str,
        status: StepStatus,
        log_output: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let (started_at, completed_at): (Option<String>, Option<String>) = match status {
            StepStatus::Running => (Some(now.clone()), None),
            StepStatus::Success | StepStatus::Failed | StepStatus::Skipped => {
                let existing_started: Option<String> = conn
                    .query_row(
                        "SELECT started_at FROM post_process_steps WHERE id = ?1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()?;
                (existing_started, Some(now.clone()))
            }
            _ => (None, None),
        };

        let count = conn.execute(
            "UPDATE post_process_steps SET status = ?2, log_output = COALESCE(?3, log_output), \
             error_message = COALESCE(?4, error_message), started_at = COALESCE(?5, started_at), \
             completed_at = COALESCE(?6, completed_at) WHERE id = ?1",
            params![
                id,
                status.to_string(),
                log_output,
                error_message,
                started_at,
                completed_at
            ],
        )?;
        Ok(count)
    }

    pub fn delete_run(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM post_process_runs WHERE id = ?1", [id])?;
        Ok(count)
    }
}

// ==================== LLM Call Repository ====================

pub struct LlmCallRepository {
    pool: DbPool,
}

impl LlmCallRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        req: RecordLlmCallRequest,
        total_tokens: i32,
        duration_ms: i32,
        prompt_preview: Option<&str>,
        metadata: Option<&str>,
    ) -> Result<LlmCall, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        conn.execute(
            "INSERT INTO llm_calls (id, story_id, draft_id, model_id, model_name, purpose, \
             prompt_tokens, completion_tokens, total_tokens, duration_ms, success, error_message, \
             prompt_preview, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                &id,
                req.story_id,
                req.draft_id,
                req.model_id,
                req.model_name,
                req.purpose,
                req.prompt_tokens,
                req.completion_tokens,
                total_tokens,
                duration_ms,
                if req.success { 1 } else { 0 },
                req.error_message,
                prompt_preview,
                metadata,
                now.to_rfc3339()
            ],
        )?;

        Ok(LlmCall {
            id,
            story_id: req.story_id,
            draft_id: req.draft_id,
            revision_id: None,
            model_id: req.model_id,
            model_name: req.model_name,
            purpose: req.purpose,
            prompt_tokens: req.prompt_tokens,
            completion_tokens: req.completion_tokens,
            total_tokens,
            duration_ms,
            success: req.success,
            error_message: req.error_message,
            prompt_preview: prompt_preview.map(|s| s.to_string()),
            metadata: metadata.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
        limit: i64,
    ) -> Result<Vec<LlmCall>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, revision_id, model_id, model_name, purpose, \
             prompt_tokens, completion_tokens, total_tokens, duration_ms, success, error_message, \
             prompt_preview, metadata, created_at
             FROM llm_calls WHERE story_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let calls = stmt
            .query_map([story_id, limit.to_string().as_str()], |row| {
                let created_str: String = row.get(15)?;
                Ok(LlmCall {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    revision_id: row.get(3)?,
                    model_id: row.get(4)?,
                    model_name: row.get(5)?,
                    purpose: row.get(6)?,
                    prompt_tokens: row.get(7)?,
                    completion_tokens: row.get(8)?,
                    total_tokens: row.get(9)?,
                    duration_ms: row.get(10)?,
                    success: row.get::<_, i32>(11)? != 0,
                    error_message: row.get(12)?,
                    prompt_preview: row.get(13)?,
                    metadata: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    pub fn get_recent(&self, limit: i64) -> Result<Vec<LlmCall>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, story_id, draft_id, revision_id, model_id, model_name, purpose, \
             prompt_tokens, completion_tokens, total_tokens, duration_ms, success, error_message, \
             prompt_preview, metadata, created_at
             FROM llm_calls ORDER BY created_at DESC LIMIT ?1",
        )?;

        let calls = stmt
            .query_map([limit.to_string().as_str()], |row| {
                let created_str: String = row.get(15)?;
                Ok(LlmCall {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    draft_id: row.get(2)?,
                    revision_id: row.get(3)?,
                    model_id: row.get(4)?,
                    model_name: row.get(5)?,
                    purpose: row.get(6)?,
                    prompt_tokens: row.get(7)?,
                    completion_tokens: row.get(8)?,
                    total_tokens: row.get(9)?,
                    duration_ms: row.get(10)?,
                    success: row.get::<_, i32>(11)? != 0,
                    error_message: row.get(12)?,
                    prompt_preview: row.get(13)?,
                    metadata: row.get(14)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    pub fn get_stats_by_story(&self, story_id: &str) -> Result<(i64, i64, f64), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let (count, total_tokens, total_cost): (i64, i64, f64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(total_tokens), 0), COALESCE(SUM(cost), 0.0) FROM \
             llm_calls WHERE story_id = ?1",
            [story_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        Ok((count, total_tokens, total_cost))
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM llm_calls WHERE id = ?1", [id])?;
        Ok(count)
    }
}
