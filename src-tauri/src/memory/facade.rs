//! MemoryFacade — 记忆/知识图谱读取的统一入口（P2 起步）。
//!
//! 热路径只做确定性 DB 读取与截断，**零 LLM**。
//! WriteTimeBundle 与 StoryContextBuilder 共用 `related_entity_summaries`，
//! 避免两处各自拼装 KG 摘要导致 top-N / 截断策略漂移。

use crate::{
    db::{DbPool, Entity, KnowledgeGraphRepository},
    domain::memory_pack::MemoryPack,
    error::AppError,
    memory::orchestrator::{MemoryOrchestrator, MemoryTaskType},
};

/// 相关设定描述默认截断长度（字符）。
pub const ENTITY_DESC_TRUNCATE: usize = 80;

/// 默认续写热路径注入的相关设定条数上限。
pub const DEFAULT_RELATED_ENTITY_LIMIT: usize = 5;

/// 记忆门面：KG 摘要与 MemoryPack 的薄封装。
pub struct MemoryFacade;

impl MemoryFacade {
    /// 加载故事未归档 KG 实体，按 `access_count` 降序取 top-`limit`，
    /// 每条格式为 `名称（类型）: 描述`（描述截断至 [`ENTITY_DESC_TRUNCATE`]
    /// 字符）。
    ///
    /// DB 失败时返回空 Vec（软降级，不阻断写作热路径）。
    pub fn related_entity_summaries(pool: &DbPool, story_id: &str, limit: usize) -> Vec<String> {
        let repo = KnowledgeGraphRepository::new(pool.clone());
        match repo.get_entities_by_story(story_id) {
            Ok(entities) => Self::summaries_from_entities(&entities, limit),
            Err(e) => {
                log::warn!(
                    "[MemoryFacade] related_entity_summaries failed story={}: {}",
                    story_id,
                    e
                );
                vec![]
            }
        }
    }

    /// 从已加载实体构建摘要（纯函数，便于单测）。
    pub fn summaries_from_entities(entities: &[Entity], limit: usize) -> Vec<String> {
        if limit == 0 {
            return vec![];
        }
        let mut scored: Vec<&Entity> = entities.iter().filter(|e| !e.is_archived).collect();
        scored.sort_by(|a, b| {
            b.access_count
                .cmp(&a.access_count)
                .then_with(|| a.name.cmp(&b.name))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|e| {
                let description = e
                    .attributes
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("无描述");
                format_entity_summary(&e.name, &e.entity_type.to_string(), description)
            })
            .collect()
    }

    /// MemoryPack 薄封装——委托 [`MemoryOrchestrator`]。
    pub fn build_memory_pack(
        pool: &DbPool,
        story_id: &str,
        chapter_number: i32,
        task_type: MemoryTaskType,
        outline: Option<&str>,
    ) -> Result<MemoryPack, AppError> {
        MemoryOrchestrator::new(pool.clone()).build_memory_pack(
            story_id,
            chapter_number,
            task_type,
            outline,
        )
    }
}

/// 单条相关设定摘要：`名称（类型）: 描述`，描述按字符截断。
pub fn format_entity_summary(name: &str, entity_type: &str, description: &str) -> String {
    format!(
        "{}（{}）: {}",
        name,
        entity_type,
        truncate_chars(description, ENTITY_DESC_TRUNCATE)
    )
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", chars.into_iter().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use serde_json::json;

    use super::*;

    fn make_entity(name: &str, entity_type: &str, description: &str, access_count: i32) -> Entity {
        Entity {
            id: format!("id-{name}"),
            story_id: "story-1".into(),
            name: name.into(),
            entity_type: entity_type
                .parse()
                .unwrap_or(crate::db::EntityType::Concept),
            attributes: json!({ "description": description }),
            embedding: None,
            first_seen: Local::now(),
            last_updated: Local::now(),
            confidence_score: None,
            access_count,
            last_accessed: None,
            is_archived: false,
            archived_at: None,
            source: None,
            is_auto_generated: None,
        }
    }

    #[test]
    fn empty_kg_returns_empty_summaries() {
        let summaries = MemoryFacade::summaries_from_entities(&[], 5);
        assert!(summaries.is_empty());
    }

    #[test]
    fn truncate_description_to_80_chars() {
        let long: String = "测".repeat(100);
        let line = format_entity_summary("玄铁剑", "Item", &long);
        // "玄铁剑（Item）: " + 80 chars + "…"
        let desc_part = line.split(": ").nth(1).expect("has desc");
        let body = desc_part.trim_end_matches('…');
        assert_eq!(body.chars().count(), 80);
        assert!(desc_part.ends_with('…'));
    }

    #[test]
    fn top_5_cap_respects_limit() {
        let entities: Vec<Entity> = (0..12)
            .map(|i| {
                make_entity(
                    &format!("实体{i}"),
                    "Concept",
                    &format!("描述{i}"),
                    i, // higher index = higher access
                )
            })
            .collect();
        let summaries = MemoryFacade::summaries_from_entities(&entities, 5);
        assert_eq!(summaries.len(), 5);
        // access_count 降序：实体11 … 实体7
        assert!(summaries[0].starts_with("实体11（"));
        assert!(summaries[4].starts_with("实体7（"));
    }

    #[test]
    fn archived_entities_excluded() {
        let mut archived = make_entity("已归档", "Location", "不该出现", 99);
        archived.is_archived = true;
        let live = make_entity("活实体", "Location", "应出现", 1);
        let summaries = MemoryFacade::summaries_from_entities(&[archived, live], 5);
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].contains("活实体"));
        assert!(!summaries[0].contains("已归档"));
    }

    #[test]
    fn short_description_not_truncated() {
        let line = format_entity_summary("小镇", "Location", "边境小镇");
        assert_eq!(line, "小镇（Location）: 边境小镇");
        assert!(!line.contains('…'));
    }
}
