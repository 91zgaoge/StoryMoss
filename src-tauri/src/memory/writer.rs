//! Memory Writer — 创作完成后自动将新内容写入记忆系统
//!
//! v0.8.0: 简化版实现
//! - 生成内容摘要（前 200 字）
//! - 更新 scene_commits.summary_text
//! - 异步更新 memory_items（简化：直接创建 summary 条目）

use crate::db::{repositories_story_system::SceneCommitRepository, DbPool, MemoryItemRepository};

pub struct MemoryWriter {
    pool: DbPool,
}

impl MemoryWriter {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 定稿/续写完成后自动更新记忆
    /// 1. 生成摘要
    /// 2. 更新 scene_commits
    /// 3. 创建 memory_items（简化版）
    pub async fn write(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: &str,
    ) -> Result<(), String> {
        if content.len() < 10 {
            log::warn!("[MemoryWriter] Content too short, skipping");
            return Ok(());
        }

        // 1. 生成摘要（前 200 字，不截断句子）
        let summary = Self::extract_summary(content, 200);
        log::info!(
            "[MemoryWriter] Writing memory for story {} chapter {}: {} chars",
            story_id,
            chapter_number,
            summary.chars().count()
        );

        // 2. 更新 scene_commits
        self.update_scene_commit(story_id, chapter_number, &summary)?;

        // 3. 创建 memory_item（working memory 摘要）
        self.create_memory_item(story_id, chapter_number, &summary)?;

        Ok(())
    }

    /// 提取内容摘要：取前 N 字，不截断句子
    fn extract_summary(content: &str, max_chars: usize) -> String {
        let trimmed = content.trim();
        if trimmed.chars().count() <= max_chars {
            return trimmed.to_string();
        }

        // 取前 max_chars 字，然后找到最后一个句号/逗号截断
        let prefix: String = trimmed.chars().take(max_chars).collect();
        if let Some(pos) = prefix.rfind(['。', '！', '？']) {
            prefix[..=pos].to_string()
        } else if let Some(pos) = prefix.rfind('，') {
            prefix[..=pos].to_string()
        } else {
            prefix
        }
    }

    /// 更新 scene_commits：找到该 chapter_number 的最新 commit，更新
    /// summary_text
    fn update_scene_commit(
        &self,
        story_id: &str,
        chapter_number: i32,
        summary: &str,
    ) -> Result<(), String> {
        let repo = SceneCommitRepository::new(self.pool.clone());
        let commits = repo
            .get_by_story(story_id)
            .map_err(|e| format!("Failed to get commits: {}", e))?;

        // 找到该 chapter_number 的最新 commit
        let target = commits
            .into_iter()
            .filter(|c| c.chapter_number == chapter_number)
            .max_by_key(|c| c.created_at);

        if let Some(commit) = target {
            repo.update_commit(
                &commit.id,
                &commit.status,
                commit.outline_snapshot_json.as_deref(),
                commit.review_result_json.as_deref(),
                commit.fulfillment_result_json.as_deref(),
                commit.accepted_events_json.as_deref(),
                commit.state_deltas_json.as_deref(),
                commit.entity_deltas_json.as_deref(),
                Some(summary),
                commit.dominant_strand.as_deref(),
                commit.projection_status_json.as_deref(),
            )
            .map_err(|e| format!("Failed to update commit: {}", e))?;
            log::info!("[MemoryWriter] Updated scene_commit {} summary", commit.id);
        } else {
            log::warn!(
                "[MemoryWriter] No scene_commit found for story {} chapter {}, creating new one",
                story_id,
                chapter_number
            );
            // 创建新的 commit
            let _ = repo
                .create(
                    story_id,
                    None, // scene_id
                    None, // chapter_id
                    chapter_number,
                    "draft",
                )
                .map_err(|e| format!("Failed to create commit: {}", e))?;
        }

        Ok(())
    }

    /// 创建 memory_item（简化版：只创建 summary 类型条目）
    fn create_memory_item(
        &self,
        story_id: &str,
        chapter_number: i32,
        summary: &str,
    ) -> Result<(), String> {
        let repo = MemoryItemRepository::new(self.pool.clone());
        repo.create(
            story_id,
            "summary",
            Some(&format!("第{}章", chapter_number)),
            Some("summary"),
            Some(summary),
            Some(chapter_number),
            0.9,
        )
        .map_err(|e| format!("Failed to create memory item: {}", e))?;

        log::info!(
            "[MemoryWriter] Created memory_item for chapter {}",
            chapter_number
        );
        Ok(())
    }
}
