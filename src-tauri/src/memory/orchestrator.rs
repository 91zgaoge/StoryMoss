//! Memory Orchestrator - 三层记忆编排器
//!
//! 参考 webnovel-writer 的 MemoryOrchestrator 设计：
//! - Working Memory: 本章大纲 + 近章摘要 + 主角状态导出
//! - Episodic Memory: 最近状态变更 + 关系变化 + 出场记录
//! - Semantic Memory: 长期语义事实（按优先级过滤）
//!
//! 预算分配：按任务类型 (write/plan/review) 分配各层条目上限

use std::collections::HashMap;

use crate::{
    db::{DbPool, MemoryItemRepository, SceneCommitRepository},
    domain::memory_pack::*,
    error::AppError,
};

/// 记忆包构建任务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTaskType {
    /// 写作/续写场景
    Write,
    /// 大纲/规划场景
    Plan,
    /// 审稿/分析场景
    Review,
}

impl MemoryTaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryTaskType::Write => "write",
            MemoryTaskType::Plan => "plan",
            MemoryTaskType::Review => "review",
        }
    }
}

/// 记忆类别优先级（数值越小优先级越高）
pub const MEMORY_PRIORITY: &[(&str, i32)] = &[
    ("world_rule", 0),
    ("character_state", 1),
    ("relationship", 2),
    ("story_fact", 3),
    ("open_loop", 4),
    ("reader_promise", 5),
    ("timeline", 6),
];

/// 记忆预算配置
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    pub working_max: usize,
    pub episodic_max: usize,
    pub semantic_max: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            working_max: 10,
            episodic_max: 15,
            semantic_max: 30,
        }
    }
}

impl MemoryBudget {
    pub fn for_task_type(task_type: MemoryTaskType) -> Self {
        match task_type {
            MemoryTaskType::Write => Self {
                working_max: 10,
                episodic_max: 15,
                semantic_max: 30,
            },
            MemoryTaskType::Plan => Self {
                working_max: 15,
                episodic_max: 10,
                semantic_max: 20,
            },
            MemoryTaskType::Review => Self {
                working_max: 5,
                episodic_max: 20,
                semantic_max: 25,
            },
        }
    }
}

/// 长篇阈值：超过该章数视为长篇，注入段摘要
pub const LONG_BOOK_THRESHOLD: i32 = 50;
/// 长篇注入的最近段摘要条数上限
const MAX_SEGMENT_MEMORIES: usize = 3;

/// 自适应章节摘要窗口（P1-C，v0.61.0）。
///
/// 替换写死的「近 3 章」：短篇给足近章细节，长篇收窄近章、改由段摘要/全书纲要
/// 提供确定性的远期参照（不再只靠向量检索概率召回）。
pub fn adaptive_summary_window(chapter_number: i32) -> usize {
    if chapter_number <= 15 {
        10
    } else if chapter_number <= LONG_BOOK_THRESHOLD {
        5
    } else {
        3
    }
}

/// 记忆编排器
pub struct MemoryOrchestrator {
    pool: DbPool,
}

impl MemoryOrchestrator {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 构建记忆包
    pub fn build_memory_pack(
        &self,
        story_id: &str,
        chapter_number: i32,
        task_type: MemoryTaskType,
        outline: Option<&str>,
    ) -> Result<MemoryPack, AppError> {
        let budget = MemoryBudget::for_task_type(task_type);

        // 1. 构建工作记忆
        let working = self.build_working_memory(story_id, chapter_number, outline)?;

        // 2. 构建情景记忆
        let episodic = self.build_episodic_memory(story_id, chapter_number)?;

        // 3. 获取语义记忆
        let repo = MemoryItemRepository::new(self.pool.clone());
        let active_items = repo.get_active_by_story(story_id)?;

        let conflicts = repo.get_conflicts(story_id)?;

        // 4. 按相关性过滤
        let filtered = self.filter_relevant(&active_items, chapter_number, outline);

        // 5. 应用预算
        let semantic_items = filtered
            .into_iter()
            .take(budget.semantic_max)
            .map(|item| MemoryItemDto {
                id: item.id.clone(),
                category: item.category.clone(),
                subject: item.subject.clone(),
                field: item.field.clone(),
                value: item.value.clone(),
                source_chapter: item.source_chapter,
                confidence: item.confidence,
            })
            .collect::<Vec<_>>();

        let working_items = working
            .into_iter()
            .take(budget.working_max)
            .collect::<Vec<_>>();

        let episodic_items = episodic
            .into_iter()
            .take(budget.episodic_max)
            .collect::<Vec<_>>();

        // 6. 提取活跃约束
        let active_constraints: Vec<MemoryItemDto> = semantic_items
            .iter()
            .filter(|item| item.category == "world_rule" || item.category == "open_loop")
            .cloned()
            .collect();

        // 7. 构建警告
        let mut warnings = Vec::new();
        if !conflicts.is_empty() {
            warnings.push(MemoryWarning {
                warning_type: "memory_conflict".to_string(),
                count: conflicts.len(),
                sample: conflicts
                    .into_iter()
                    .take(5)
                    .map(|item| MemoryItemDto {
                        id: item.id.clone(),
                        category: item.category,
                        subject: item.subject,
                        field: item.field,
                        value: item.value,
                        source_chapter: item.source_chapter,
                        confidence: item.confidence,
                    })
                    .collect(),
            });
        }

        let semantic_payload = semantic_items.clone();
        let injected = semantic_payload.len();
        let layered_total = working_items.len() + episodic_items.len() + injected;
        let conflicts = warnings.first().map(|w| w.count).unwrap_or(0);

        Ok(MemoryPack {
            working_memory: working_items.clone(),
            episodic_memory: episodic_items.clone(),
            semantic_memory: semantic_payload.clone(),
            long_term_facts: semantic_payload,
            active_constraints,
            recent_changes: Vec::new(), // 简化处理
            warnings,
            stats: MemoryStats {
                total: active_items.len(),
                working_total: working_items.len(),
                episodic_total: episodic_items.len(),
                semantic_total: semantic_items.len(),
                injected,
                layered_total_injected: layered_total,
                filtered: active_items.len().saturating_sub(semantic_items.len()),
                conflicts,
            },
        })
    }

    /// 构建记忆包（异步版本，避免在 async 运行时中阻塞 tokio worker 线程）
    pub async fn build_memory_pack_async(
        &self,
        story_id: &str,
        chapter_number: i32,
        task_type: MemoryTaskType,
        outline: Option<&str>,
    ) -> Result<MemoryPack, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        let outline = outline.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || {
            let orchestrator = MemoryOrchestrator::new(pool);
            orchestrator.build_memory_pack(&story_id, chapter_number, task_type, outline.as_deref())
        })
        .await
        .map_err(|e| AppError::internal(format!("memory pack task failed: {}", e)))?
    }

    fn build_working_memory(
        &self,
        story_id: &str,
        chapter_number: i32,
        outline: Option<&str>,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let mut result = Vec::new();

        // 添加大纲
        if let Some(outline_text) = outline {
            result.push(MemoryEntry {
                layer: "working".to_string(),
                source: "outline".to_string(),
                chapter: chapter_number,
                content: serde_json::json!(outline_text),
            });
        }

        // 添加近章摘要（窗口随书长自适应：10 / 5 / 3）
        let commit_repo = SceneCommitRepository::new(self.pool.clone());
        let recent_commits = commit_repo.get_by_story(story_id)?;

        let window = adaptive_summary_window(chapter_number);
        let recent_summaries: Vec<_> = recent_commits
            .into_iter()
            .filter(|c| c.chapter_number < chapter_number)
            .take(window)
            .collect();

        for commit in recent_summaries {
            if let Some(summary) = commit.summary_text {
                result.push(MemoryEntry {
                    layer: "working".to_string(),
                    source: "summary".to_string(),
                    chapter: commit.chapter_number,
                    content: serde_json::json!(summary),
                });
            }
        }

        // 长篇（P1-B/C）：注入最近段摘要与全书纲要，提供确定性远期参照。
        // 位置在近章摘要之后——预算截断时优先保留最贴近本章的细节。
        if chapter_number > LONG_BOOK_THRESHOLD {
            let segments =
                crate::story_system::segment_summary::load_segment_summaries(&self.pool, story_id);
            for segment in segments.into_iter().rev().take(MAX_SEGMENT_MEMORIES) {
                result.push(MemoryEntry {
                    layer: "working".to_string(),
                    source: "segment_summary".to_string(),
                    chapter: segment.end_chapter.unwrap_or(0),
                    content: serde_json::json!(format!(
                        "第{}-{}章：{}",
                        segment.start_chapter.unwrap_or(0),
                        segment.end_chapter.unwrap_or(0),
                        segment.summary
                    )),
                });
            }
        }
        if let Some(book) =
            crate::story_system::segment_summary::load_book_summary(&self.pool, story_id)
        {
            result.push(MemoryEntry {
                layer: "working".to_string(),
                source: "book_summary".to_string(),
                chapter: chapter_number,
                content: serde_json::json!(book.summary),
            });
        }

        Ok(result)
    }

    fn build_episodic_memory(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Vec<MemoryEntry>, AppError> {
        let mut result = Vec::new();

        // 1. 获取最近 3 个章节的场景提交记录（状态变更历史）
        let commit_repo = SceneCommitRepository::new(self.pool.clone());
        let commits = commit_repo.get_by_story(story_id)?;

        for commit in commits
            .into_iter()
            .filter(|c| c.chapter_number < chapter_number)
            .take(3)
        {
            // 解析 state_deltas_json 中的状态变更
            if let Ok(deltas) = serde_json::from_str::<serde_json::Value>(
                commit.state_deltas_json.as_deref().unwrap_or("{}"),
            ) {
                if let Some(changes) = deltas.as_object() {
                    for (key, value) in changes {
                        result.push(MemoryEntry {
                            layer: "episodic".to_string(),
                            source: format!("state_delta:ch{}", commit.chapter_number),
                            chapter: commit.chapter_number,
                            content: serde_json::json!({
                                "field": key,
                                "change": value,
                            }),
                        });
                    }
                }
            }

            // 添加提交摘要作为 episodic 记忆
            if let Some(ref summary) = commit.summary_text {
                result.push(MemoryEntry {
                    layer: "episodic".to_string(),
                    source: format!("commit_summary:ch{}", commit.chapter_number),
                    chapter: commit.chapter_number,
                    content: serde_json::json!(summary),
                });
            }
        }

        // 2. 获取角色出场记录（简化：最近活跃角色）
        let char_repo = crate::db::CharacterRepository::new(self.pool.clone());
        if let Ok(characters) = char_repo.get_by_story(story_id) {
            for character in characters.into_iter().take(5) {
                let state = format!(
                    "{}: 目标={}, 状态={}",
                    if character.name.is_empty() {
                        "Unknown"
                    } else {
                        &character.name
                    },
                    character.goals.as_deref().unwrap_or("N/A"),
                    // 使用 dynamic_traits 的第一个特征作为当前状态
                    character
                        .dynamic_traits
                        .first()
                        .map(|t| format!("{}({:.0}%)", t.trait_name, t.confidence * 100.0))
                        .unwrap_or_else(|| "Active".to_string())
                );
                result.push(MemoryEntry {
                    layer: "episodic".to_string(),
                    source: format!("character_state:{}", character.id),
                    chapter: chapter_number,
                    content: serde_json::json!(state),
                });
            }
        }

        Ok(result)
    }

    fn filter_relevant<'a>(
        &self,
        items: &'a [crate::db::MemoryItem],
        chapter_number: i32,
        outline: Option<&str>,
    ) -> Vec<&'a crate::db::MemoryItem> {
        let outline_text = outline.unwrap_or("");
        let source_window = 20;

        let priority_map: HashMap<&str, i32> =
            MEMORY_PRIORITY.iter().map(|(k, v)| (*k, *v)).collect();

        let mut filtered: Vec<&crate::db::MemoryItem> = items
            .iter()
            .filter(|item| {
                // 按大纲关键词匹配
                if let Some(ref subject) = item.subject {
                    if outline_text.contains(subject) {
                        return true;
                    }
                }
                if let Some(ref field) = item.field {
                    if outline_text.contains(field) {
                        return true;
                    }
                }
                // 按源章节窗口匹配
                if let Some(source_ch) = item.source_chapter {
                    if chapter_number - source_ch <= source_window {
                        return true;
                    }
                }
                false
            })
            .collect();

        // 如果没有匹配到，保留最近的
        if filtered.is_empty() {
            filtered = items.iter().collect();
        }

        // 按优先级和源章节排序
        filtered.sort_by(|a, b| {
            let pa = priority_map.get(a.category.as_str()).unwrap_or(&99);
            let pb = priority_map.get(b.category.as_str()).unwrap_or(&99);
            pa.cmp(pb).then_with(|| {
                b.source_chapter
                    .unwrap_or(0)
                    .cmp(&a.source_chapter.unwrap_or(0))
            })
        });

        filtered
    }
}

#[cfg(test)]
mod adaptive_window_tests {
    use super::*;

    #[test]
    fn adaptive_summary_window_shrinks_with_book_length() {
        // 短篇给足近章细节
        assert_eq!(adaptive_summary_window(1), 10);
        assert_eq!(adaptive_summary_window(15), 10);
        // 中篇收窄
        assert_eq!(adaptive_summary_window(16), 5);
        assert_eq!(adaptive_summary_window(50), 5);
        // 长篇最窄，靠段摘要/全书纲要补远期
        assert_eq!(adaptive_summary_window(51), 3);
        assert_eq!(adaptive_summary_window(500), 3);
    }
}
