//! Memory Orchestrator - 三层记忆编排器
//!
//! 参考 webnovel-writer 的 MemoryOrchestrator 设计：
//! - Working Memory: 本章大纲 + 近章摘要 + 主角状态导出
//! - Episodic Memory: 最近状态变更 + 关系变化 + 出场记录
//! - Semantic Memory: 长期语义事实（按优先级过滤）
//!
//! 预算分配：按任务类型 (write/plan/review) 分配各层条目上限

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::db::{DbPool, MemoryItemRepository, SceneCommitRepository};

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

/// 记忆包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPack {
    pub working_memory: Vec<MemoryEntry>,
    pub episodic_memory: Vec<MemoryEntry>,
    pub semantic_memory: Vec<MemoryItemDto>,
    pub long_term_facts: Vec<MemoryItemDto>,
    pub active_constraints: Vec<MemoryItemDto>,
    pub recent_changes: Vec<HashMap<String, serde_json::Value>>,
    pub warnings: Vec<MemoryWarning>,
    pub stats: MemoryStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub layer: String,
    pub source: String,
    pub chapter: i32,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItemDto {
    pub id: String,
    pub category: String,
    pub subject: Option<String>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub source_chapter: Option<i32>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWarning {
    pub warning_type: String,
    pub count: usize,
    pub sample: Vec<MemoryItemDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total: usize,
    pub working_total: usize,
    pub episodic_total: usize,
    pub semantic_total: usize,
    pub injected: usize,
    pub layered_total_injected: usize,
    pub filtered: usize,
    pub conflicts: usize,
}

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
    pub fn for_task_type(task_type: &str) -> Self {
        match task_type {
            "write" => Self {
                working_max: 10,
                episodic_max: 15,
                semantic_max: 30,
            },
            "plan" => Self {
                working_max: 15,
                episodic_max: 10,
                semantic_max: 20,
            },
            "review" => Self {
                working_max: 5,
                episodic_max: 20,
                semantic_max: 25,
            },
            _ => Self::default(),
        }
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
        task_type: &str,
        outline: Option<&str>,
    ) -> Result<MemoryPack, String> {
        let budget = MemoryBudget::for_task_type(task_type);

        // 1. 构建工作记忆
        let working = self.build_working_memory(story_id, chapter_number, outline)?;

        // 2. 构建情景记忆
        let episodic = self.build_episodic_memory(story_id, chapter_number)?;

        // 3. 获取语义记忆
        let repo = MemoryItemRepository::new(self.pool.clone());
        let active_items = repo
            .get_active_by_story(story_id)
            .map_err(|e| format!("获取记忆项失败: {}", e))?;

        let conflicts = repo
            .get_conflicts(story_id)
            .map_err(|e| format!("获取冲突项失败: {}", e))?;

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

    fn build_working_memory(
        &self,
        story_id: &str,
        chapter_number: i32,
        outline: Option<&str>,
    ) -> Result<Vec<MemoryEntry>, String> {
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

        // 添加近章摘要（最近3章）
        let commit_repo = SceneCommitRepository::new(self.pool.clone());
        let recent_commits = commit_repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取最近提交失败: {}", e))?;

        let recent_summaries: Vec<_> = recent_commits
            .into_iter()
            .filter(|c| c.chapter_number < chapter_number)
            .take(3)
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

        Ok(result)
    }

    fn build_episodic_memory(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<Vec<MemoryEntry>, String> {
        let mut result = Vec::new();

        // 1. 获取最近 3 个章节的场景提交记录（状态变更历史）
        let commit_repo = SceneCommitRepository::new(self.pool.clone());
        let commits = commit_repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取场景提交失败: {}", e))?;

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
