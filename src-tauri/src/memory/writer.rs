//! Memory Writer — 创作完成后自动将新内容写入记忆系统
//!
//! v0.8.0: 简化版实现
//! - 生成内容摘要（前 200 字）
//! - 更新 scene_commits.summary_text
//! - 异步更新 memory_items（简化：直接创建 summary 条目）
//!
//! v0.11.x (C2): 增加全局 Semaphore 背压与 CancellationToken 取消传播。

use std::{collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::db::{repositories_story_system::SceneCommitRepository, DbPool, MemoryItemRepository};

/// 后台记忆写入任务全局并发限制（默认同时运行 2 个）。
pub static MEMORY_WRITER_SEMAPHORE: Lazy<Arc<Semaphore>> =
    Lazy::new(|| Arc::new(Semaphore::new(2)));

/// 用户取消生成时，向尚未完成的后台 ingest 任务传播取消信号。
/// key = 触发该后台任务的 generation request_id。
static MEMORY_INGEST_CANCEL_TOKENS: Lazy<std::sync::Mutex<HashMap<String, CancellationToken>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// 注册并返回一个与 request_id 关联的 CancellationToken。
/// 后台任务应使用 token.child_token()，以便用户取消时传播信号。
pub fn register_ingest_cancel_token(request_id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    let mut map = MEMORY_INGEST_CANCEL_TOKENS.lock().unwrap();
    map.insert(request_id.to_string(), token.clone());
    token
}

/// 取消并移除指定 request_id 对应的 ingest token。
pub fn cancel_ingest_token(request_id: &str) {
    let mut map = MEMORY_INGEST_CANCEL_TOKENS.lock().unwrap();
    if let Some(token) = map.remove(request_id) {
        token.cancel();
        log::info!(
            "[MemoryWriter] Cancel signal propagated to ingest task for request_id {}",
            request_id
        );
    }
}

/// 取出并移除指定 request_id 对应的 ingest token（任务完成时清理）。
pub fn take_ingest_cancel_token(request_id: &str) -> Option<CancellationToken> {
    MEMORY_INGEST_CANCEL_TOKENS
        .lock()
        .unwrap()
        .remove(request_id)
}

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
        self.write_with_cancel(story_id, chapter_number, content, None)
            .await
    }

    /// 支持取消令牌的 write 版本。
    pub async fn write_with_cancel(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<(), String> {
        if content.len() < 10 {
            log::warn!("[MemoryWriter] Content too short, skipping");
            return Ok(());
        }

        if let Some(token) = cancel {
            if token.is_cancelled() {
                log::info!(
                    "[MemoryWriter] Ingest cancelled before write for story {}",
                    story_id
                );
                return Ok(());
            }
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
        let db_start = std::time::Instant::now();
        self.update_scene_commit(story_id, chapter_number, &summary)?;
        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "phase": "db_write_scene_commit",
                "elapsed_ms": db_start.elapsed().as_millis(),
                "story_id": story_id,
                "chapter_number": chapter_number,
            })
        );

        if let Some(token) = cancel {
            if token.is_cancelled() {
                log::info!(
                    "[MemoryWriter] Ingest cancelled after scene_commit for story {}",
                    story_id
                );
                return Ok(());
            }
        }

        // 3. 创建 memory_item（working memory 摘要）
        let db_start = std::time::Instant::now();
        self.create_memory_item(story_id, chapter_number, &summary)?;
        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "phase": "db_write_memory_item",
                "elapsed_ms": db_start.elapsed().as_millis(),
                "story_id": story_id,
                "chapter_number": chapter_number,
            })
        );

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
        if let Some(pos) = prefix.rfind(|c| c == '。' || c == '！' || c == '？') {
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
            .max_by_key(|c| c.created_at.clone());

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_cancel_ingest_token_propagates_to_running_task() {
        let request_id = "req-cancel-001";
        let token = register_ingest_cancel_token(request_id);
        let child = token.child_token();

        let handle = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while !child.is_cancelled() {
                if start.elapsed() > Duration::from_secs(5) {
                    panic!("child token was not cancelled in time");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            true
        });

        // Simulate user cancelling the generation: the ingest task should observe it.
        std::thread::sleep(Duration::from_millis(20));
        cancel_ingest_token(request_id);

        assert!(handle.join().expect("thread panicked"));
        assert!(
            take_ingest_cancel_token(request_id).is_none(),
            "cancelled token should be removed from registry"
        );
    }
}
