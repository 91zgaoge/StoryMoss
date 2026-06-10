//! Projection Writers - 投影写入器
//!
//! CHAPTER_COMMIT 被接受后，各 projection writer 负责更新对应的 read-model：
//! - StateProjectionWriter: 更新 protagonist_state, plot_threads
//! - IndexProjectionWriter: 更新实体出场、关系、状态变更
//! - SummaryProjectionWriter: 写入章节摘要
//! - MemoryProjectionWriter: 更新长期记忆
//! - VectorProjectionWriter: 更新向量索引

use serde::Deserialize;

use crate::{
    db::{DbPool, MemoryItemRepository, StorySummaryRepository},
    vector::lancedb_store::{LanceVectorStore, VectorRecord},
};

/// 投影写入器 trait
pub trait ProjectionWriter {
    fn name(&self) -> &'static str;
    fn apply(&self, story_id: &str, chapter_number: i32, commit_json: &str)
        -> Result<bool, String>;
}

/// 状态投影写入器
pub struct StateProjectionWriter {
    pool: DbPool,
}

impl StateProjectionWriter {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProjectionWriter for StateProjectionWriter {
    fn name(&self) -> &'static str {
        "state"
    }

    fn apply(
        &self,
        story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct CommitData {
            state_deltas_json: Option<String>,
        }

        let commit: CommitData =
            serde_json::from_str(commit_json).map_err(|e| format!("解析 commit 失败: {}", e))?;

        let deltas_str = match commit.state_deltas_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true), // 无状态变更
        };

        #[derive(Deserialize)]
        struct StateDelta {
            subject: String,
            field: String,
            old_value: Option<String>,
            new_value: String,
        }

        let deltas: Vec<StateDelta> = serde_json::from_str(&deltas_str)
            .map_err(|e| format!("解析 state_deltas 失败: {}", e))?;

        let repo = MemoryItemRepository::new(self.pool.clone());
        for delta in deltas {
            let value = format!(
                "{} -> {}",
                delta.old_value.unwrap_or_else(|| "(无)".to_string()),
                delta.new_value
            );
            repo.create(
                story_id,
                "state",
                Some(&delta.subject),
                Some(&delta.field),
                Some(&value),
                Some(chapter_number),
                0.95,
            )
            .map_err(|e| format!("写入 state 记忆失败: {}", e))?;
        }

        Ok(true)
    }
}

/// 索引投影写入器
pub struct IndexProjectionWriter {
    pool: DbPool,
}

impl IndexProjectionWriter {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProjectionWriter for IndexProjectionWriter {
    fn name(&self) -> &'static str {
        "index"
    }

    fn apply(
        &self,
        story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct CommitData {
            entity_deltas_json: Option<String>,
        }

        let commit: CommitData =
            serde_json::from_str(commit_json).map_err(|e| format!("解析 commit 失败: {}", e))?;

        let deltas_str = match commit.entity_deltas_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        #[derive(Deserialize)]
        struct EntityDelta {
            entity_id: String,
            entity_name: String,
            action: String, // create | update | delete | appear
            changes: Option<Vec<(String, String)>>,
        }

        let deltas: Vec<EntityDelta> = serde_json::from_str(&deltas_str)
            .map_err(|e| format!("解析 entity_deltas 失败: {}", e))?;

        let repo = MemoryItemRepository::new(self.pool.clone());
        for delta in deltas {
            let value = match &delta.changes {
                Some(changes) => changes
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", "),
                None => delta.action.clone(),
            };

            repo.create(
                story_id,
                "entity",
                Some(&delta.entity_name),
                Some(&delta.action),
                Some(&value),
                Some(chapter_number),
                0.9,
            )
            .map_err(|e| format!("写入 entity 索引失败: {}", e))?;
        }

        Ok(true)
    }
}

/// 摘要投影写入器
pub struct SummaryProjectionWriter {
    pool: DbPool,
}

impl SummaryProjectionWriter {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProjectionWriter for SummaryProjectionWriter {
    fn name(&self) -> &'static str {
        "summary"
    }

    fn apply(
        &self,
        story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct CommitData {
            summary_text: Option<String>,
        }

        let commit: CommitData =
            serde_json::from_str(commit_json).map_err(|e| format!("解析 commit 失败: {}", e))?;

        let summary = match commit.summary_text {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        let repo = StorySummaryRepository::new(self.pool.clone());
        let content = format!("第{}章摘要\n\n{}", chapter_number, summary);
        repo.create_summary(story_id, "chapter", &content)
            .map_err(|e| format!("写入摘要失败: {}", e))?;

        Ok(true)
    }
}

/// 记忆投影写入器
pub struct MemoryProjectionWriter {
    pool: DbPool,
}

impl MemoryProjectionWriter {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProjectionWriter for MemoryProjectionWriter {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn apply(
        &self,
        story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct CommitData {
            accepted_events_json: Option<String>,
        }

        let commit: CommitData =
            serde_json::from_str(commit_json).map_err(|e| format!("解析 commit 失败: {}", e))?;

        let events_str = match commit.accepted_events_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        #[derive(Deserialize)]
        struct StoryEvent {
            description: String,
            importance: Option<f32>,
        }

        let events: Vec<StoryEvent> =
            serde_json::from_str(&events_str).map_err(|e| format!("解析 events 失败: {}", e))?;

        let repo = MemoryItemRepository::new(self.pool.clone());
        for event in events {
            let confidence = event.importance.unwrap_or(0.85);
            repo.create(
                story_id,
                "event",
                None,
                Some("chapter_event"),
                Some(&event.description),
                Some(chapter_number),
                confidence,
            )
            .map_err(|e| format!("写入事件记忆失败: {}", e))?;
        }

        Ok(true)
    }
}

/// 向量投影写入器
pub struct VectorProjectionWriter {
    store: LanceVectorStore,
}

impl VectorProjectionWriter {
    pub fn new(store: LanceVectorStore) -> Self {
        Self { store }
    }
}

impl ProjectionWriter for VectorProjectionWriter {
    fn name(&self) -> &'static str {
        "vector"
    }

    fn apply(
        &self,
        _story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, String> {
        #[derive(Deserialize)]
        struct CommitData {
            summary_text: Option<String>,
        }

        let commit: CommitData =
            serde_json::from_str(commit_json).map_err(|e| format!("解析 commit 失败: {}", e))?;

        let summary = match commit.summary_text {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        let text = format!("第{}章: {}", chapter_number, summary);

        let _embedding = match crate::embeddings::embedding::embed_text(&text) {
            Ok(emb) => emb,
            Err(e) => {
                return Err(format!("生成嵌入向量失败: {}", e));
            }
        };

        // VectorProjectionWriter 需要在异步上下文中运行
        // 这里返回需要异步处理的标记，由调用方处理
        Err("VECTOR_ASYNC_REQUIRED".to_string())
    }
}

/// 获取所有投影写入器
pub fn get_projection_writers(pool: DbPool) -> Vec<Box<dyn ProjectionWriter>> {
    vec![
        Box::new(StateProjectionWriter::new(pool.clone())),
        Box::new(IndexProjectionWriter::new(pool.clone())),
        Box::new(SummaryProjectionWriter::new(pool.clone())),
        Box::new(MemoryProjectionWriter::new(pool.clone())),
    ]
}

/// 异步应用向量投影
pub async fn apply_vector_projection(
    store: &LanceVectorStore,
    story_id: &str,
    chapter_number: i32,
    summary_text: &str,
) -> Result<bool, String> {
    let text = format!("第{}章: {}", chapter_number, summary_text);

    let embedding = crate::embeddings::embedding::embed_text(&text)
        .map_err(|e| format!("生成嵌入向量失败: {}", e))?;

    let record = VectorRecord {
        id: format!("{}_ch{}", story_id, chapter_number),
        story_id: story_id.to_string(),
        chapter_id: String::new(),
        chapter_number,
        text,
        record_type: "chapter_summary".to_string(),
        metadata: None,
        embedding,
    };

    store
        .add_record(record)
        .await
        .map_err(|e| format!("写入向量索引失败: {}", e))?;

    Ok(true)
}
