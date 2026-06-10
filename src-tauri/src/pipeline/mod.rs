pub mod commands;
pub mod executor;
pub mod finalize;
pub mod post_process;
pub mod refine;
pub mod review;
pub mod style_analysis;
pub mod types;

pub use finalize::finalize_draft;
pub use post_process::{build_finalize_steps, run_post_process_step};
pub use refine::refine_draft;
pub use review::review_draft;

use crate::{
    db::{
        DbPool, DraftRepository, DraftStatus, PipelineReviewRepository, PostProcessRepository,
        RevisionRepository, RevisionStatus,
    },
    error::AppError,
};

/// 管线编排器 — 提供高级管线操作
pub struct PipelineOrchestrator {
    pool: DbPool,
}

impl PipelineOrchestrator {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 获取指定章节当前活跃的管线草稿（最新非归档）
    pub fn get_active_draft(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<crate::db::Draft>, AppError> {
        let repo = DraftRepository::new(self.pool.clone());
        repo.get_latest_by_chapter(story_id, chapter_number)
            .map(|d| d.filter(|draft| draft.status != DraftStatus::Archived))
            .map_err(AppError::from)
    }

    /// 获取指定章节已定稿的草稿
    pub fn get_finalized_draft(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Option<crate::db::Draft>, AppError> {
        let repo = DraftRepository::new(self.pool.clone());
        repo.get_finalized_by_chapter(story_id, chapter_number)
            .map_err(AppError::from)
    }

    /// 获取草稿的审稿报告列表
    pub fn get_draft_review_history(
        &self,
        draft_id: &str,
    ) -> Result<Vec<crate::db::PipelineReview>, AppError> {
        let repo = PipelineReviewRepository::new(self.pool.clone());
        repo.get_by_draft(draft_id).map_err(AppError::from)
    }

    /// 获取草稿的修稿历史
    pub fn get_draft_revision_history(
        &self,
        draft_id: &str,
    ) -> Result<Vec<crate::db::Revision>, AppError> {
        let repo = RevisionRepository::new(self.pool.clone());
        repo.get_by_draft(draft_id).map_err(AppError::from)
    }

    /// 废弃指定草稿的所有后续版本（当用户选择回退时）
    pub fn discard_subsequent_versions(
        &self,
        story_id: &str,
        chapter_number: i32,
        keep_version: i32,
    ) -> Result<usize, AppError> {
        let repo = DraftRepository::new(self.pool.clone());
        let drafts = repo.get_by_story_chapter(story_id, chapter_number)?;

        let mut discarded = 0;
        for draft in drafts {
            if draft.version > keep_version {
                if let Ok(count) = repo.delete(&draft.id) {
                    discarded += count;
                }
            }
        }
        Ok(discarded)
    }

    /// 标记修稿为已合并（用户接受修稿结果）
    pub fn merge_revision(&self, revision_id: &str) -> Result<usize, AppError> {
        let repo = RevisionRepository::new(self.pool.clone());
        repo.update_status(revision_id, RevisionStatus::Merged)
            .map_err(AppError::from)
    }

    /// 获取后处理运行状态（含步骤详情）
    pub fn get_post_process_status(
        &self,
        run_id: &str,
    ) -> Result<Option<PostProcessRunWithSteps>, AppError> {
        let run_repo = PostProcessRepository::new(self.pool.clone());
        let run = run_repo.get_run_by_id(run_id)?;
        match run {
            Some(r) => {
                let steps = run_repo.get_steps_by_run(run_id)?;
                Ok(Some(PostProcessRunWithSteps { run: r, steps }))
            }
            None => Ok(None),
        }
    }
}

/// 后处理运行状态（含步骤详情）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PostProcessRunWithSteps {
    pub run: crate::db::PostProcessRun,
    pub steps: Vec<crate::db::PostProcessStep>,
}
