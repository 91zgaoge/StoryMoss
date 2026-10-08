#![allow(dead_code)]
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
    error::AppError,
    vector::lancedb_store::{LanceVectorStore, VectorRecord},
};

/// 投影写入器 trait
pub trait ProjectionWriter {
    fn name(&self) -> &'static str;
    fn apply(
        &self,
        story_id: &str,
        chapter_number: i32,
        commit_json: &str,
    ) -> Result<bool, AppError>;
}

// ==================== 声明式投影路由表（P3-F，v0.64.0） ====================
//
// 参考 webnovel-writer 的「事件 → writer」声明式路由：把「哪类提交产物激活
// 哪些投影」从隐式的注册顺序变成**纯数据表**——可单测（覆盖性/无孤儿）、
// 可审计（commit 日志打印路由摘要）、可扩展（新增 writer 必须同时进表，
// 测试会拦住漏配）。
//
// 语义边界（重要）：路由表决定「谁被触发、以什么顺序、状态键是什么」；
// 单个 writer 内部的「产物为空则 skipped」判定保持不变（各 writer 自持），
// 因此本表是行为保持的重构，不改变既有投影结果。

/// 提交产物：一次 CHAPTER_COMMIT 中可被投影到 read-model 的部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitArtifact {
    StateDeltas,
    EntityDeltas,
    AcceptedEvents,
    SummaryText,
    /// 章节正文本身（KG 提取的触发条件，不在 commit_json 内）
    ChapterContent,
}

impl CommitArtifact {
    pub const ALL: [CommitArtifact; 5] = [
        CommitArtifact::StateDeltas,
        CommitArtifact::EntityDeltas,
        CommitArtifact::AcceptedEvents,
        CommitArtifact::SummaryText,
        CommitArtifact::ChapterContent,
    ];

    /// commit_json 中的字段名（ChapterContent 无字段，标注为调用方参数）。
    pub fn key(self) -> &'static str {
        match self {
            CommitArtifact::StateDeltas => "state_deltas_json",
            CommitArtifact::EntityDeltas => "entity_deltas_json",
            CommitArtifact::AcceptedEvents => "accepted_events_json",
            CommitArtifact::SummaryText => "summary_text",
            CommitArtifact::ChapterContent => "(chapter_content)",
        }
    }

    /// 该产物在本次提交中是否有实质内容（空数组/空对象视为无）。
    pub fn has_content(self, commit_json: &str, chapter_content: Option<&str>) -> bool {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            state_deltas_json: Option<String>,
            #[serde(default)]
            entity_deltas_json: Option<String>,
            #[serde(default)]
            accepted_events_json: Option<String>,
            #[serde(default)]
            summary_text: Option<String>,
        }
        if self == CommitArtifact::ChapterContent {
            // 与 commit_service 中 KG 提取的准入一致（按字节长度判阈值）
            return chapter_content
                .map(|c| c.trim().len() >= 20)
                .unwrap_or(false);
        }
        let Ok(probe) = serde_json::from_str::<Probe>(commit_json) else {
            return false;
        };
        let raw = match self {
            CommitArtifact::StateDeltas => probe.state_deltas_json,
            CommitArtifact::EntityDeltas => probe.entity_deltas_json,
            CommitArtifact::AcceptedEvents => probe.accepted_events_json,
            CommitArtifact::SummaryText => probe.summary_text,
            CommitArtifact::ChapterContent => None,
        };
        let Some(value) = raw else { return false };
        let trimmed = value.trim();
        !trimmed.is_empty() && trimmed != "[]" && trimmed != "{}" && trimmed != "null"
    }
}

/// 投影写入器类别（与 trait 实现一一对应；`deferred` 表示异步执行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectionWriterKind {
    State,
    Index,
    Summary,
    Memory,
    Vector,
    Kg,
}

impl ProjectionWriterKind {
    pub const ALL: [ProjectionWriterKind; 6] = [
        ProjectionWriterKind::State,
        ProjectionWriterKind::Index,
        ProjectionWriterKind::Summary,
        ProjectionWriterKind::Memory,
        ProjectionWriterKind::Vector,
        ProjectionWriterKind::Kg,
    ];

    /// 与 `ProjectionWriter::name()` 一致的稳定键（也是 projection_status
    /// 的键）。
    pub fn name(self) -> &'static str {
        match self {
            ProjectionWriterKind::State => "state",
            ProjectionWriterKind::Index => "index",
            ProjectionWriterKind::Summary => "summary",
            ProjectionWriterKind::Memory => "memory",
            ProjectionWriterKind::Vector => "vector",
            ProjectionWriterKind::Kg => "kg",
        }
    }

    /// 异步（deferred）执行：与同步 writer 一起声明，但由调用方在后续阶段执行。
    pub fn deferred(self) -> bool {
        matches!(
            self,
            ProjectionWriterKind::Vector | ProjectionWriterKind::Kg
        )
    }
}

/// 一条路由：产物 → 被激活的 writer（含异步；顺序即执行顺序）。
pub struct ProjectionRoute {
    pub artifact: CommitArtifact,
    pub writers: &'static [ProjectionWriterKind],
}

/// 声明式路由表（唯一权威；`get_projection_writers` 与状态键都从它派生）。
pub const PROJECTION_ROUTES: &[ProjectionRoute] = &[
    ProjectionRoute {
        artifact: CommitArtifact::StateDeltas,
        writers: &[ProjectionWriterKind::State],
    },
    ProjectionRoute {
        artifact: CommitArtifact::EntityDeltas,
        writers: &[ProjectionWriterKind::Index],
    },
    ProjectionRoute {
        artifact: CommitArtifact::AcceptedEvents,
        writers: &[ProjectionWriterKind::Memory],
    },
    ProjectionRoute {
        artifact: CommitArtifact::SummaryText,
        writers: &[ProjectionWriterKind::Summary, ProjectionWriterKind::Vector],
    },
    ProjectionRoute {
        artifact: CommitArtifact::ChapterContent,
        writers: &[ProjectionWriterKind::Kg],
    },
];

/// 同步 writer 的执行顺序（按表去重，过滤异步）。
pub fn sync_projection_kinds() -> Vec<ProjectionWriterKind> {
    let mut out: Vec<ProjectionWriterKind> = Vec::new();
    for route in PROJECTION_ROUTES {
        for kind in route.writers {
            if !kind.deferred() && !out.contains(kind) {
                out.push(*kind);
            }
        }
    }
    out
}

/// 异步 writer（由调用方在后续阶段执行）。
pub fn deferred_projection_kinds() -> Vec<ProjectionWriterKind> {
    let mut out: Vec<ProjectionWriterKind> = Vec::new();
    for route in PROJECTION_ROUTES {
        for kind in route.writers {
            if kind.deferred() && !out.contains(kind) {
                out.push(*kind);
            }
        }
    }
    out
}

/// projection_status 的全部键（含异步），用于 commit 记录与前端展示。
pub fn projection_status_keys() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for route in PROJECTION_ROUTES {
        for kind in route.writers {
            if !out.contains(&kind.name()) {
                out.push(kind.name());
            }
        }
    }
    out
}

/// 路由摘要（写进 commit 日志，便于事后审计「这次提交触发了什么」）。
pub fn route_summary(commit_json: &str, chapter_content: Option<&str>) -> String {
    PROJECTION_ROUTES
        .iter()
        .map(|route| {
            let flag = if route.artifact.has_content(commit_json, chapter_content) {
                "on"
            } else {
                "off"
            };
            let writers: Vec<&str> = route.writers.iter().map(|k| k.name()).collect();
            format!("{}:{flag}→[{}]", route.artifact.key(), writers.join(","))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 按类别构造同步 writer（新增类别必须在此接线，测试会校验覆盖性）。
fn build_sync_writer(
    kind: ProjectionWriterKind,
    pool: DbPool,
) -> Option<Box<dyn ProjectionWriter>> {
    match kind {
        ProjectionWriterKind::State => Some(Box::new(StateProjectionWriter::new(pool))),
        ProjectionWriterKind::Index => Some(Box::new(IndexProjectionWriter::new(pool))),
        ProjectionWriterKind::Summary => Some(Box::new(SummaryProjectionWriter::new(pool))),
        ProjectionWriterKind::Memory => Some(Box::new(MemoryProjectionWriter::new(pool))),
        ProjectionWriterKind::Vector | ProjectionWriterKind::Kg => None, // 异步，不在同步链
    }
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
    ) -> Result<bool, AppError> {
        #[derive(Deserialize)]
        struct CommitData {
            state_deltas_json: Option<String>,
        }

        let commit: CommitData = serde_json::from_str(commit_json)?;

        let deltas_str = match commit.state_deltas_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true), // 无状态变更
        };

        let repo = MemoryItemRepository::new(self.pool.clone());
        for delta in normalize_delta_items(&deltas_str, &repo, story_id)? {
            repo.create_with_kg_entity(
                story_id,
                "state",
                Some(&delta.subject),
                Some(&delta.field),
                Some(&delta.value),
                Some(chapter_number),
                0.95,
                delta.kg_entity_id.as_deref(),
            )?;
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
    ) -> Result<bool, AppError> {
        #[derive(Deserialize)]
        struct CommitData {
            entity_deltas_json: Option<String>,
        }

        let commit: CommitData = serde_json::from_str(commit_json)?;

        let deltas_str = match commit.entity_deltas_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        let repo = MemoryItemRepository::new(self.pool.clone());
        for delta in normalize_delta_items(&deltas_str, &repo, story_id)? {
            repo.create_with_kg_entity(
                story_id,
                "entity",
                Some(&delta.subject),
                Some(&delta.field),
                Some(&delta.value),
                Some(chapter_number),
                0.9,
                delta.kg_entity_id.as_deref(),
            )?;
        }

        Ok(true)
    }
}

/// 归一后的 delta 项：任何形态都落成 (subject, field, value[, kg_entity_id])。
#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDelta {
    subject: String,
    field: String,
    value: String,
    kg_entity_id: Option<String>,
}

/// 把提交产物里的 delta 项归一。
///
/// v0.64.9 真机事故：`state` / `index` 两个 writer 从上线起就一直报
/// `missing field subject` / `missing field entity_id`（真机 projection 状态里
/// 两条 error），因为它们的反序列化目标是**没有生产者**的历史形态；而
/// `auto_commit` 实际写的是 KG 视图：
/// - `state_deltas_json`：`[{id, name, entity_type, attributes}]`
/// - `entity_deltas_json`：`[{id, source_id, target_id, relation_type,
///   strength}]`
/// 结果状态类记忆一条都没落进 `memory_items`（`category='state'` 计数为 0）。
///
/// 现在按字段识别形态，四种都收：
/// 1. 键值形态 `{subject, field, old_value, new_value}`（历史形态，保留兼容）
/// 2. 实体事件形态 `{entity_id, entity_name, action, changes}`（历史形态）
/// 3. KG 关系视图 `{source_id, target_id, relation_type, strength}` →
///    名字解析后落库
/// 4. KG 实体视图 `{id, name, entity_type, attributes}` → 属性压成一行摘要
fn normalize_delta_items(
    deltas_str: &str,
    repo: &MemoryItemRepository,
    story_id: &str,
) -> Result<Vec<NormalizedDelta>, AppError> {
    let items: Vec<serde_json::Value> = serde_json::from_str(deltas_str)?;
    let mut out = Vec::new();
    for item in items {
        if let Some(d) = normalize_one(&item, repo, story_id) {
            out.push(d);
        }
    }
    Ok(out)
}

fn normalize_one(
    item: &serde_json::Value,
    repo: &MemoryItemRepository,
    story_id: &str,
) -> Option<NormalizedDelta> {
    let s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let name_of = |id: &str| {
        repo.lookup_kg_entity_name_by_id(story_id, id)
            .ok()
            .flatten()
            .unwrap_or_else(|| id.to_string())
    };

    // 1) 键值形态
    if let (Some(subject), Some(field)) = (s("subject"), s("field")) {
        let new_value = s("new_value").unwrap_or_default();
        let value = match s("old_value").filter(|v| !v.is_empty()) {
            Some(old) => format!("{old} -> {new_value}"),
            None => new_value,
        };
        return Some(NormalizedDelta {
            subject,
            field,
            value,
            kg_entity_id: None,
        });
    }

    // 2) 实体事件形态
    if let Some(entity_name) = s("entity_name") {
        let action = s("action").unwrap_or_else(|| "update".into());
        let value = item
            .get("changes")
            .and_then(|v| v.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|r| match r.as_array().filter(|a| a.len() >= 2) {
                        Some(kv) => format!(
                            "{}: {}",
                            kv[0].as_str().unwrap_or(""),
                            kv[1].as_str().unwrap_or("")
                        ),
                        None => String::new(),
                    })
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
                    .join("；")
            })
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| action.clone());
        let kg_entity_id = s("entity_id").filter(|x| !x.is_empty()).or_else(|| {
            repo.lookup_kg_entity_id_by_name(story_id, &entity_name)
                .ok()
                .flatten()
        });
        return Some(NormalizedDelta {
            subject: entity_name,
            field: action,
            value,
            kg_entity_id,
        });
    }

    // 3) KG 关系视图（真机 entity_deltas_json 的实际形态）
    if let (Some(source_id), Some(target_id)) = (s("source_id"), s("target_id")) {
        let source = name_of(&source_id);
        let target = name_of(&target_id);
        let relation = s("relation_type").unwrap_or_else(|| "关系".into());
        let strength = item.get("strength").and_then(|v| v.as_f64());
        let value = match strength {
            Some(x) => format!("→ {target}（强度 {x:.2}）"),
            None => format!("→ {target}"),
        };
        return Some(NormalizedDelta {
            subject: source,
            field: relation,
            value,
            kg_entity_id: Some(source_id),
        });
    }

    // 4) KG 实体视图（真机 state_deltas_json 的实际形态）
    if let Some(name) = s("name") {
        let entity_type = s("entity_type").unwrap_or_else(|| "entity".into());
        let value = item
            .get("attributes")
            .map(compact_attributes)
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| entity_type.clone());
        return Some(NormalizedDelta {
            subject: name,
            field: entity_type,
            value,
            kg_entity_id: s("id"),
        });
    }

    None
}

/// 属性 JSON 压成一行「k=v；k=v」（跳过空值，按需截断）。
fn compact_attributes(attrs: &serde_json::Value) -> String {
    let Some(map) = attrs.as_object() else {
        return attrs.to_string();
    };
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in map {
        let text = match v {
            serde_json::Value::Null => continue,
            serde_json::Value::String(s) if s.trim().is_empty() => continue,
            serde_json::Value::String(s) => s.trim().to_string(),
            other => other.to_string(),
        };
        parts.push(format!("{k}={text}"));
    }
    let joined = parts.join("；");
    if joined.chars().count() > 800 {
        joined.chars().take(800).collect::<String>()
    } else {
        joined
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
    ) -> Result<bool, AppError> {
        #[derive(Deserialize)]
        struct CommitData {
            summary_text: Option<String>,
        }

        let commit: CommitData = serde_json::from_str(commit_json)?;

        let summary = match commit.summary_text {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        let repo = StorySummaryRepository::new(self.pool.clone());
        let content = format!("第{}章摘要\n\n{}", chapter_number, summary);
        repo.create_summary(story_id, "chapter", &content)?;

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
    ) -> Result<bool, AppError> {
        #[derive(Deserialize)]
        struct CommitData {
            accepted_events_json: Option<String>,
        }

        let commit: CommitData = serde_json::from_str(commit_json)?;

        let events_str = match commit.accepted_events_json {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        #[derive(Deserialize)]
        struct StoryEvent {
            description: String,
            importance: Option<f32>,
        }

        let events: Vec<StoryEvent> = serde_json::from_str(&events_str)?;

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
            )?;
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
    ) -> Result<bool, AppError> {
        #[derive(Deserialize)]
        struct CommitData {
            summary_text: Option<String>,
        }

        let commit: CommitData = serde_json::from_str(commit_json)?;

        let summary = match commit.summary_text {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(true),
        };

        let text = format!("第{}章: {}", chapter_number, summary);

        let _embedding = crate::embeddings::embedding::embed_text(&text)?;

        // VectorProjectionWriter 需要在异步上下文中运行
        // 这里返回需要异步处理的标记，由调用方处理
        Err(AppError::from("VECTOR_ASYNC_REQUIRED"))
    }
}

/// 获取所有投影写入器
pub fn get_projection_writers(pool: DbPool) -> Vec<Box<dyn ProjectionWriter>> {
    // 顺序与组成由声明式路由表派生（P3-F）：新增 writer 只需进表 + 接线，
    // 漏配会被 `projection_routing` 测试拦住。
    let mut writers: Vec<Box<dyn ProjectionWriter>> = Vec::new();
    for kind in sync_projection_kinds() {
        match build_sync_writer(kind, pool.clone()) {
            Some(writer) => writers.push(writer),
            None => log::warn!(
                "[ProjectionWriter] 路由表声明了同步 writer 「{}」但没有构造器接线",
                kind.name()
            ),
        }
    }
    writers
}

/// 异步应用向量投影
pub async fn apply_vector_projection(
    store: &LanceVectorStore,
    story_id: &str,
    chapter_number: i32,
    summary_text: &str,
) -> Result<bool, AppError> {
    let text = format!("第{}章: {}", chapter_number, summary_text);

    let embedding = crate::embeddings::embedding::embed_text_async(text.clone()).await?;

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

    store.add_record(record).await?;

    Ok(true)
}

#[cfg(test)]
mod projection_routing_tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn routing_table_covers_every_artifact() {
        for artifact in CommitArtifact::ALL {
            assert!(
                PROJECTION_ROUTES.iter().any(|r| r.artifact == artifact),
                "产物 {:?} 没有路由（新增产物必须进表）",
                artifact
            );
        }
    }

    #[test]
    fn sync_kinds_are_constructible_and_match_writer_names() {
        let pool = create_test_pool().unwrap();
        let kinds = sync_projection_kinds();
        assert_eq!(
            kinds,
            vec![
                ProjectionWriterKind::State,
                ProjectionWriterKind::Index,
                ProjectionWriterKind::Memory,
                ProjectionWriterKind::Summary,
            ],
            "同步执行顺序由表决定：state → index → memory → summary"
        );
        for kind in &kinds {
            assert!(
                build_sync_writer(*kind, pool.clone()).is_some(),
                "路由表声明了同步 writer「{}」但没有构造器",
                kind.name()
            );
        }
        // 注册顺序与 name() 一致（审计依赖 name 作为状态键）
        let writers = get_projection_writers(pool);
        let names: Vec<&str> = writers.iter().map(|w| w.name()).collect();
        assert_eq!(names, vec!["state", "index", "memory", "summary"]);
    }

    #[test]
    fn status_keys_include_deferred_writers() {
        let keys = projection_status_keys();
        for expected in ["state", "index", "summary", "memory", "vector", "kg"] {
            assert!(keys.contains(&expected), "状态键缺 {expected}: {keys:?}");
        }
        assert_eq!(keys.len(), 6, "状态键不应重复: {keys:?}");
        // 每个类别都必须出现在某条路由里（无孤儿 writer）
        for kind in ProjectionWriterKind::ALL {
            assert!(
                PROJECTION_ROUTES.iter().any(|r| r.writers.contains(&kind)),
                "writer「{}」未出现在任何路由中",
                kind.name()
            );
        }
    }

    #[test]
    fn deferred_kinds_are_vector_and_kg() {
        assert_eq!(
            deferred_projection_kinds(),
            vec![ProjectionWriterKind::Vector, ProjectionWriterKind::Kg]
        );
    }

    #[test]
    fn artifact_has_content_treats_empty_payload_as_absent() {
        let empty = serde_json::json!({
            "state_deltas_json": "[]",
            "entity_deltas_json": "{}",
            "accepted_events_json": "",
            "summary_text": "第3章：他们分道扬镳。",
        })
        .to_string();
        assert!(!CommitArtifact::StateDeltas.has_content(&empty, None));
        assert!(!CommitArtifact::EntityDeltas.has_content(&empty, None));
        assert!(!CommitArtifact::AcceptedEvents.has_content(&empty, None));
        assert!(CommitArtifact::SummaryText.has_content(&empty, None));
        assert!(!CommitArtifact::ChapterContent.has_content(&empty, None));
        assert!(!CommitArtifact::ChapterContent.has_content(&empty, Some("太短")));
        assert!(CommitArtifact::ChapterContent
            .has_content(&empty, Some("这一章足够长，可以触发知识图谱提取了。")));
    }

    #[test]
    fn route_summary_reports_on_off_per_artifact() {
        let commit = serde_json::json!({
            "state_deltas_json": "[{\"id\":\"c1\"}]",
            "entity_deltas_json": "[]",
            "accepted_events_json": "[]",
            "summary_text": "第3章：雨夜对峙。",
        })
        .to_string();
        let summary = route_summary(&commit, None);
        assert!(
            summary.contains("state_deltas_json:on→[state]"),
            "{summary}"
        );
        assert!(
            summary.contains("entity_deltas_json:off→[index]"),
            "{summary}"
        );
        assert!(
            summary.contains("summary_text:on→[summary,vector]"),
            "{summary}"
        );
        assert!(summary.contains("(chapter_content):off→[kg]"), "{summary}");
    }

    /// v0.64.9 真机契约：`state` / `index` 必须吃得下 `auto_commit` 实际写入的
    /// KG 视图（此前报 missing field subject /
    /// entity_id，状态类记忆一条不落）。
    #[test]
    fn state_and_index_writers_accept_real_kg_delta_shapes() {
        let pool = create_test_pool().unwrap();
        let story_id = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "投影形态".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id;
        let (lady, man) = {
            let repo = crate::db::CharacterRepository::new(pool.clone());
            let a = repo
                .create(crate::db::CreateCharacterRequest {
                    story_id: story_id.clone(),
                    name: "明成公主".into(),
                    ..Default::default()
                })
                .unwrap();
            let b = repo
                .create(crate::db::CreateCharacterRequest {
                    story_id: story_id.clone(),
                    name: "苏亦铁".into(),
                    ..Default::default()
                })
                .unwrap();
            (a.id, b.id)
        };

        // 真机形态：state_deltas = KG 实体视图；entity_deltas = KG 关系视图
        let commit = serde_json::json!({
            "state_deltas_json": serde_json::json!([{
                "id": lady,
                "name": "明成公主",
                "entity_type": "Character",
                "attributes": {"status": "Dead", "location": "棺中", "mood": null},
            }]).to_string(),
            "entity_deltas_json": serde_json::json!([{
                "id": "rel-1",
                "source_id": lady,
                "target_id": man,
                "relation_type": "敌对",
                "strength": 0.9,
            }]).to_string(),
        })
        .to_string();

        let state = StateProjectionWriter::new(pool.clone())
            .apply(&story_id, 2, &commit)
            .expect("state writer 必须吃得下 KG 实体视图");
        assert!(state);
        let index = IndexProjectionWriter::new(pool.clone())
            .apply(&story_id, 2, &commit)
            .expect("index writer 必须吃得下 KG 关系视图");
        assert!(index);

        let conn = pool.get().unwrap();
        let state_rows: Vec<(String, String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT COALESCE(subject,''), COALESCE(field,''), COALESCE(value,'') \
                     FROM memory_items WHERE story_id = ?1 AND category = 'state'",
                )
                .unwrap();
            let rows = stmt.query_map([&story_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)));
            rows.unwrap().filter_map(Result::ok).collect()
        };
        assert_eq!(state_rows.len(), 1, "{state_rows:?}");
        assert_eq!(state_rows[0].0, "明成公主");
        assert!(state_rows[0].2.contains("status=Dead"), "{state_rows:?}");
        assert!(
            !state_rows[0].2.contains("mood"),
            "空属性不得落库: {:?}",
            state_rows[0]
        );

        let index_rows: Vec<(String, String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT COALESCE(subject,''), COALESCE(field,''), COALESCE(value,'') \
                     FROM memory_items WHERE story_id = ?1 AND category = 'entity'",
                )
                .unwrap();
            let rows = stmt.query_map([&story_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)));
            rows.unwrap().filter_map(Result::ok).collect()
        };
        assert_eq!(index_rows.len(), 1, "{index_rows:?}");
        assert_eq!(index_rows[0].0, "明成公主");
        assert_eq!(index_rows[0].1, "敌对");
        assert!(index_rows[0].2.contains("苏亦铁"), "{index_rows:?}");
    }

    #[test]
    fn legacy_key_value_delta_shape_still_supported() {
        let pool = create_test_pool().unwrap();
        let story_id = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "历史形态".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id;
        let commit = serde_json::json!({
            "state_deltas_json": "[{\"subject\":\"苏亦铁\",\"field\":\"location\",\"old_value\":\"大堂\",\"new_value\":\"古道\"}]",
        })
        .to_string();
        StateProjectionWriter::new(pool.clone())
            .apply(&story_id, 3, &commit)
            .expect("键值形态继续兼容");
        let conn = pool.get().unwrap();
        let value: String = conn
            .query_row(
                "SELECT value FROM memory_items WHERE story_id = ?1 AND category = 'state'",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, "大堂 -> 古道");
    }
}
