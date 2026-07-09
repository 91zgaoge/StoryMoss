//! MemoryFacade — 记忆/知识图谱读取的统一入口（P2 起步）。
//!
//! 热路径只做确定性 DB 读取与截断，**零 LLM**。
//! WriteTimeBundle 与 StoryContextBuilder 共用 `related_entity_summaries`，
//! 避免两处各自拼装 KG 摘要导致 top-N / 截断策略漂移。
//!
//! `list_unified_facts` 经 `story_memory_facts` VIEW（V105+）投影
//! `kg_entities` ∪ `memory_items`，不破坏任一物理表。

use serde::{Deserialize, Serialize};

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

/// 统一记忆事实默认条数上限。
pub const DEFAULT_UNIFIED_FACTS_LIMIT: usize = 100;

/// 统一读模型中的单条事实（来自 `story_memory_facts` VIEW）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMemoryFact {
    pub id: String,
    pub story_id: String,
    /// `kg_entity` | `memory_item`
    pub record_kind: String,
    pub category: String,
    pub subject: Option<String>,
    pub field: Option<String>,
    pub value: String,
    pub source_chapter: Option<i32>,
    pub confidence: f32,
    pub status: String,
    pub updated_at: String,
    pub kg_entity_id: Option<String>,
    pub memory_item_id: Option<String>,
}

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

    /// 统一列出 `kg_entities` + `memory_items` 事实（经 VIEW 或 UNION 回退）。
    ///
    /// 默认仅 `status = 'active'`；`limit == 0` 返回空。DB/VIEW
    /// 失败时软降级为空。
    pub fn list_unified_facts(
        pool: &DbPool,
        story_id: &str,
        limit: usize,
    ) -> Vec<UnifiedMemoryFact> {
        if limit == 0 {
            return vec![];
        }
        match Self::query_unified_facts(pool, story_id, limit) {
            Ok(facts) => facts,
            Err(e) => {
                log::warn!(
                    "[MemoryFacade] list_unified_facts failed story={}: {}",
                    story_id,
                    e
                );
                vec![]
            }
        }
    }

    fn query_unified_facts(
        pool: &DbPool,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<UnifiedMemoryFact>, rusqlite::Error> {
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let view_exists: bool = conn
            .prepare(
                "SELECT 1 FROM sqlite_master WHERE type='view' AND name='story_memory_facts' \
                 LIMIT 1",
            )
            .and_then(|mut stmt| stmt.exists([]))
            .unwrap_or(false);

        let sql = if view_exists {
            "SELECT id, story_id, record_kind, category, subject, field, value, source_chapter, \
             confidence, status, updated_at, kg_entity_id, memory_item_id \
             FROM story_memory_facts \
             WHERE story_id = ?1 AND status = 'active' \
             ORDER BY record_kind, category, updated_at DESC \
             LIMIT ?2"
        } else {
            // Pre-V105 fallback: inline UNION matching the VIEW contract.
            "SELECT id, story_id, record_kind, category, subject, field, value, source_chapter, \
             confidence, status, updated_at, kg_entity_id, memory_item_id FROM ( \
               SELECT id, story_id, 'kg_entity' AS record_kind, entity_type AS category, \
                 name AS subject, NULL AS field, \
                 COALESCE(json_extract(attributes, '$.description'), '') AS value, \
                 NULL AS source_chapter, COALESCE(confidence_score, 1.0) AS confidence, \
                 CASE WHEN is_archived = 1 THEN 'archived' ELSE 'active' END AS status, \
                 last_updated AS updated_at, id AS kg_entity_id, NULL AS memory_item_id \
               FROM kg_entities \
               UNION ALL \
               SELECT id, story_id, 'memory_item' AS record_kind, category, subject, field, \
                 COALESCE(value, ''), source_chapter, confidence, status, updated_at, \
                 NULL AS kg_entity_id, id AS memory_item_id \
               FROM memory_items \
             ) \
             WHERE story_id = ?1 AND status = 'active' \
             ORDER BY record_kind, category, updated_at DESC \
             LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql)?;
        let facts = stmt
            .query_map(rusqlite::params![story_id, limit as i64], |row| {
                Ok(UnifiedMemoryFact {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    record_kind: row.get(2)?,
                    category: row.get(3)?,
                    subject: row.get(4)?,
                    field: row.get(5)?,
                    value: row.get(6)?,
                    source_chapter: row.get(7)?,
                    confidence: row.get(8)?,
                    status: row.get(9)?,
                    updated_at: row.get(10)?,
                    kg_entity_id: row.get(11)?,
                    memory_item_id: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(facts)
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
    use crate::db::{create_test_pool, KnowledgeGraphRepository, MemoryItemRepository};

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

    fn seed_story(pool: &DbPool, story_id: &str) {
        let conn = pool.get().expect("pool");
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO stories (id, title, created_at, updated_at) VALUES (?1, ?2, \
             ?3, ?3)",
            rusqlite::params![story_id, "统一记忆测试", now],
        )
        .expect("insert story");
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

    #[test]
    fn list_unified_facts_includes_kg_and_memory_items() {
        let pool = create_test_pool().expect("test pool");
        let story_id = "story-unified-facts";
        seed_story(&pool, story_id);

        let kg = KnowledgeGraphRepository::new(pool.clone());
        let attrs = json!({ "description": "传说中的神兵" });
        kg.create_entity(story_id, "赤霄剑", "Item", &attrs, None)
            .expect("create kg entity");

        let mi = MemoryItemRepository::new(pool.clone());
        mi.create(
            story_id,
            "event",
            Some("赤霄剑现身"),
            Some("chapter_event"),
            Some("主角在废墟中发现赤霄剑"),
            Some(1),
            0.9,
        )
        .expect("create memory item");

        let facts = MemoryFacade::list_unified_facts(&pool, story_id, 50);
        assert!(
            facts
                .iter()
                .any(|f| f.record_kind == "kg_entity" && f.subject.as_deref() == Some("赤霄剑")),
            "expected kg_entity fact, got: {:?}",
            facts
        );
        assert!(
            facts
                .iter()
                .any(|f| f.record_kind == "memory_item" && f.category == "event"),
            "expected memory_item fact, got: {:?}",
            facts
        );
    }

    #[test]
    fn story_memory_facts_view_is_idempotent() {
        let pool = create_test_pool().expect("test pool");
        let conn = pool.get().expect("conn");
        // Re-apply VIEW DDL (same as V105) must not fail.
        conn.execute_batch(
            "CREATE VIEW IF NOT EXISTS story_memory_facts AS
             SELECT
               id, story_id, 'kg_entity' AS record_kind, entity_type AS category,
               name AS subject, NULL AS field,
               COALESCE(json_extract(attributes, '$.description'), '') AS value,
               NULL AS source_chapter, COALESCE(confidence_score, 1.0) AS confidence,
               CASE WHEN is_archived = 1 THEN 'archived' ELSE 'active' END AS status,
               last_updated AS updated_at, id AS kg_entity_id, NULL AS memory_item_id
             FROM kg_entities
             UNION ALL
             SELECT
               id, story_id, 'memory_item' AS record_kind, category, subject, field,
               COALESCE(value, ''), source_chapter, confidence, status, updated_at,
               NULL AS kg_entity_id, id AS memory_item_id
             FROM memory_items;",
        )
        .expect("idempotent CREATE VIEW IF NOT EXISTS");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='view' AND \
                 name='story_memory_facts'",
                [],
                |row| row.get(0),
            )
            .expect("count view");
        assert_eq!(count, 1);
    }
}
