//! 四阶段查询检索管线
//!
//! Stage 1: CJK二元组分词搜索
//! Stage 2: 图谱扩展
//! Stage 3: 预算控制
//! Stage 4: 带引用编号的上下文组装

use rusqlite::params;

use super::tokenizer::CJKTokenizer;
use crate::{db::models::Entity, embeddings::embedding::embed_text_async};

/// 查询管道
pub struct QueryPipeline {
    tokenizer: CJKTokenizer,
    budget_config: BudgetConfig,
}

/// 预算配置
#[derive(Debug, Clone)]
pub struct BudgetConfig {
    /// 总token预算 (4K-1M可配)
    pub total_budget: usize,
    /// 搜索预算比例 (60%)
    pub search_budget_pct: f32,
    /// 图谱预算比例 (20%)
    pub graph_budget_pct: f32,
    /// 上下文预算比例 (5%)
    pub context_budget_pct: f32,
    /// 组装预算比例 (15%)
    pub assembly_budget_pct: f32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            total_budget: 4096,
            search_budget_pct: 0.60,
            graph_budget_pct: 0.20,
            context_budget_pct: 0.05,
            assembly_budget_pct: 0.15,
        }
    }
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub content: String,
    pub score: f32,
    pub source_type: SourceType,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum SourceType {
    Scene,
    Entity,
    Memory,
}

/// 图谱扩展结果
#[derive(Debug, Clone)]
pub struct GraphResult {
    pub entity: Entity,
    pub relation_strength: f32,
    pub related_entities: Vec<(Entity, f32)>,
}

/// 选中的上下文
#[derive(Debug, Clone)]
pub struct SelectedContext {
    pub content: String,
    pub source: String,
    pub citation_number: usize,
    pub relevance_score: f32,
}

/// 查询结果
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub context: String,
    pub citations: Vec<Citation>,
    pub total_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct Citation {
    pub number: usize,
    pub source: String,
    pub preview: String,
}

impl QueryPipeline {
    pub fn new(budget_config: BudgetConfig) -> Self {
        Self {
            tokenizer: CJKTokenizer::new(),
            budget_config,
        }
    }

    /// 四阶段查询检索（融合语义搜索）
    pub async fn query(
        &self,
        query: &str,
        story_id: &str,
        vector_store: &dyn VectorStore,
        knowledge_graph: &dyn KnowledgeGraph,
    ) -> Result<QueryResult, Box<dyn std::error::Error + Send + Sync>> {
        // Stage 1a: CJK二元组分词搜索
        let token_results = self.token_search(query, story_id, vector_store).await?;

        // Stage 1b: 语义向量搜索（与 token 搜索并行执行）
        let semantic_results = self.semantic_search(query, story_id, vector_store).await?;

        // Stage 1c: 融合两种搜索结果
        let fused_results = Self::fuse_results(token_results, semantic_results);

        // Stage 2: 图谱扩展
        let graph_expansion = self
            .graph_expansion(&fused_results, knowledge_graph)
            .await?;

        // Stage 3: 预算控制
        let selected = self.budget_control(&fused_results, &graph_expansion)?;

        // Stage 4: 带引用编号的上下文组装
        let result = self.assemble_context(&selected)?;

        Ok(result)
    }

    /// Stage 1a: CJK二元组分词搜索
    async fn token_search(
        &self,
        query: &str,
        story_id: &str,
        vector_store: &dyn VectorStore,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // 对查询进行CJK二元组分词
        let tokens = self.tokenizer.tokenize(query);

        // 在向量存储中进行多token搜索
        let mut all_results = vec![];

        for token in tokens {
            let results = vector_store.search_with_token(story_id, &token, 10).await?;
            all_results.extend(results);
        }

        // 去重并按分数排序
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.dedup_by(|a, b| a.id == b.id);

        // 返回Top 50
        Ok(all_results.into_iter().take(50).collect())
    }

    /// Stage 1b: 语义向量搜索
    ///
    /// 将查询文本编码为 embedding，通过向量相似度检索语义相关的内容。
    /// 若 embedding 生成失败或 vector_store 不支持语义搜索，则返回空结果（
    /// graceful fallback）。
    async fn semantic_search(
        &self,
        query: &str,
        story_id: &str,
        vector_store: &dyn VectorStore,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // 生成查询文本的 embedding（优先语义模型，fallback FNV-1a）
        let embedding = match embed_text_async(query.to_string()).await {
            Ok(emb) => emb,
            Err(e) => {
                log::warn!(
                    "[QueryPipeline] 查询 embedding 生成失败，跳过语义搜索: {}",
                    e
                );
                return Ok(vec![]);
            }
        };

        // 执行向量相似度搜索
        match vector_store
            .search_with_embedding(story_id, embedding, 30)
            .await
        {
            Ok(results) => Ok(results),
            Err(e) => {
                log::warn!("[QueryPipeline] 语义搜索失败，返回空结果: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Stage 1c: 融合 token 搜索与语义搜索结果
    ///
    /// 策略：
    /// - Token 搜索权重 0.4，语义搜索权重 0.6（语义召回更精准）
    /// - 对同一文档取加权最高分
    /// - 按融合分数降序排列，取 Top 50
    fn fuse_results(
        token_results: Vec<SearchResult>,
        semantic_results: Vec<SearchResult>,
    ) -> Vec<SearchResult> {
        use std::collections::HashMap;

        const TOKEN_WEIGHT: f32 = 0.4;
        const SEMANTIC_WEIGHT: f32 = 0.6;

        let mut merged: HashMap<String, (SearchResult, f32, f32)> = HashMap::new();

        // 收集 token 结果
        for r in token_results {
            let entry = merged
                .entry(r.id.clone())
                .or_insert_with(|| (r.clone(), 0.0, 0.0));
            entry.1 = r.score.max(entry.1);
        }

        // 收集语义结果
        for r in semantic_results {
            let entry = merged
                .entry(r.id.clone())
                .or_insert_with(|| (r.clone(), 0.0, 0.0));
            entry.2 = r.score.max(entry.2);
        }

        // 计算加权融合分数并组装最终列表
        let mut fused: Vec<SearchResult> = merged
            .into_iter()
            .map(|(_id, (mut result, token_score, semantic_score))| {
                // 加权融合：若只有一侧有结果，另一侧权重减半分配
                let fused_score = if token_score > 0.0 && semantic_score > 0.0 {
                    token_score * TOKEN_WEIGHT + semantic_score * SEMANTIC_WEIGHT
                } else if token_score > 0.0 {
                    token_score * (TOKEN_WEIGHT + SEMANTIC_WEIGHT * 0.5)
                } else {
                    semantic_score * (TOKEN_WEIGHT * 0.5 + SEMANTIC_WEIGHT)
                };
                result.score = fused_score;
                result
            })
            .collect();

        // 按融合分数降序排列
        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        fused.truncate(50);
        fused
    }

    /// Stage 2: 图谱扩展
    async fn graph_expansion(
        &self,
        search_results: &[SearchResult],
        knowledge_graph: &dyn KnowledgeGraph,
    ) -> Result<Vec<GraphResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut expanded = vec![];
        let mut processed_entities = std::collections::HashSet::new();

        for result in search_results {
            // 收集所有可能匹配的候选词：分词 + metadata 中的实体名
            let mut candidate_names = vec![];

            // 1. 对 content 分词后逐 token 尝试匹配
            let tokens = self.tokenizer.tokenize(&result.content);
            candidate_names.extend(tokens);

            // 2. 从 metadata 中提取已知的实体引用
            if let Some(entities) = result.metadata.get("entities").and_then(|v| v.as_array()) {
                for e in entities {
                    if let Some(name) = e.as_str() {
                        candidate_names.push(name.to_string());
                    }
                }
            }
            // 也尝试 metadata.name 字段
            if let Some(name) = result.metadata.get("name").and_then(|v| v.as_str()) {
                candidate_names.push(name.to_string());
            }

            // 去重
            let mut seen = std::collections::HashSet::new();
            candidate_names.retain(|n| seen.insert(n.clone()));

            for name in &candidate_names {
                if let Ok(entity) = knowledge_graph.find_entity_by_name(name).await {
                    if processed_entities.insert(entity.id.clone()) {
                        // 获取相关实体（基于关系强度）
                        let related = knowledge_graph
                            .get_related_entities(&entity.id, 0.3)
                            .await?;

                        // 计算加权分数
                        let related_with_scores: Vec<(Entity, f32)> = related
                            .into_iter()
                            .map(|(e, strength)| {
                                let weighted_score = strength * 0.8 + result.score * 0.2;
                                (e, weighted_score)
                            })
                            .collect();

                        expanded.push(GraphResult {
                            entity,
                            relation_strength: result.score,
                            related_entities: related_with_scores,
                        });
                    }
                }
            }
        }

        // 按关系强度排序
        expanded.sort_by(|a, b| {
            let a_score =
                a.relation_strength + a.related_entities.iter().map(|(_, s)| s).sum::<f32>();
            let b_score =
                b.relation_strength + b.related_entities.iter().map(|(_, s)| s).sum::<f32>();
            b_score.partial_cmp(&a_score).unwrap()
        });

        Ok(expanded)
    }

    /// Stage 3: 预算控制
    fn budget_control(
        &self,
        search_results: &[SearchResult],
        graph_expansion: &[GraphResult],
    ) -> Result<Vec<SelectedContext>, Box<dyn std::error::Error + Send + Sync>> {
        let total_budget = self.budget_config.total_budget;
        let search_budget = (total_budget as f32 * self.budget_config.search_budget_pct) as usize;
        let graph_budget = (total_budget as f32 * self.budget_config.graph_budget_pct) as usize;

        let mut selected = vec![];
        let mut used_budget = 0;
        let mut citation_counter = 1;

        // 优先选择搜索结果的Top-K
        for result in search_results.iter().take(10) {
            let cost = result.content.len();
            if used_budget + cost > search_budget {
                break;
            }

            selected.push(SelectedContext {
                content: result.content.clone(),
                source: format!("{:?}", result.source_type),
                citation_number: citation_counter,
                relevance_score: result.score,
            });

            used_budget += cost;
            citation_counter += 1;
        }

        // 然后选择图谱扩展结果
        let mut budget_exceeded = false;
        for graph_result in graph_expansion {
            if budget_exceeded {
                break;
            }

            // 添加主实体
            let entity_desc = format!(
                "{}: {}",
                graph_result.entity.name,
                graph_result
                    .entity
                    .attributes
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("无描述")
            );
            let cost = entity_desc.len();

            if used_budget + cost <= search_budget + graph_budget {
                selected.push(SelectedContext {
                    content: entity_desc,
                    source: format!("Entity: {}", graph_result.entity.name),
                    citation_number: citation_counter,
                    relevance_score: graph_result.relation_strength,
                });
                used_budget += cost;
                citation_counter += 1;
            } else {
                break;
            }

            // 添加相关实体（预算允许的情况下）
            for (related, score) in &graph_result.related_entities {
                let related_desc = format!(
                    "{}: {}",
                    related.name,
                    related
                        .attributes
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("无描述")
                );
                let cost = related_desc.len();

                if used_budget + cost <= search_budget + graph_budget {
                    selected.push(SelectedContext {
                        content: related_desc,
                        source: format!("Related: {}", related.name),
                        citation_number: citation_counter,
                        relevance_score: *score,
                    });
                    used_budget += cost;
                    citation_counter += 1;
                } else {
                    budget_exceeded = true;
                    break;
                }
            }
        }

        Ok(selected)
    }

    /// Stage 4: 带引用编号的上下文组装
    fn assemble_context(
        &self,
        selected: &[SelectedContext],
    ) -> Result<QueryResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut context_parts = vec![];
        let mut citations = vec![];
        let mut total_tokens = 0;

        for item in selected {
            let part = format!("[{}] {}\n", item.citation_number, item.content);
            total_tokens += part.len();

            context_parts.push(part);

            citations.push(Citation {
                number: item.citation_number,
                source: item.source.clone(),
                preview: item.content.chars().take(50).collect::<String>() + "...",
            });
        }

        Ok(QueryResult {
            context: context_parts.join("\n"),
            citations,
            total_tokens,
        })
    }
}

/// 向量存储接口（用于查询）
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    async fn search_with_token(
        &self,
        story_id: &str,
        token: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>>;

    /// 使用 embedding 向量进行语义搜索
    async fn search_with_embedding(
        &self,
        story_id: &str,
        embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>>;
}

/// 基于数据库文本搜索的 VectorStore 适配器
///
/// 使用 SQLite LIKE 查询在场景内容和实体名称/描述中搜索匹配的 token。
/// 作为 LanceVectorStore 的轻量级替代，无需 embedding 即可工作。
pub struct DbVectorStore {
    pool: crate::db::DbPool,
}

impl DbVectorStore {
    pub fn new(pool: crate::db::DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl VectorStore for DbVectorStore {
    async fn search_with_token(
        &self,
        story_id: &str,
        token: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();
        let like_pattern = format!("%{}%", token);

        // 1. 搜索场景内容
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("DB pool error: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, content FROM scenes WHERE story_id = ?1 AND content LIKE ?2 LIMIT ?3",
        )?;
        let scene_rows = stmt.query_map(params![story_id, &like_pattern, limit as i32], |row| {
            let id: String = row.get(0)?;
            let content: String = row.get(1)?;
            Ok((id, content))
        })?;
        for row in scene_rows {
            let (id, content) = row?;
            results.push(SearchResult {
                id,
                content: content.chars().take(300).collect::<String>(),
                score: 0.8,
                source_type: SourceType::Scene,
                metadata: serde_json::json!({}),
            });
        }

        // 2. 搜索实体名称
        let mut stmt = conn.prepare(
            "SELECT id, name, attributes FROM kg_entities WHERE story_id = ?1 AND name LIKE ?2 \
             AND is_archived = 0 LIMIT ?3",
        )?;
        let entity_rows =
            stmt.query_map(params![story_id, &like_pattern, limit as i32], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let attrs_json: String = row.get(2)?;
                Ok((id, name, attrs_json))
            })?;
        for row in entity_rows {
            let (id, name, attrs_json) = row?;
            let attrs: serde_json::Value = serde_json::from_str(&attrs_json).unwrap_or_default();
            let description = attrs
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content = if description.is_empty() {
                name.clone()
            } else {
                format!("{}: {}", name, description)
            };
            results.push(SearchResult {
                id,
                content,
                score: 0.9,
                source_type: SourceType::Entity,
                metadata: attrs,
            });
        }

        // 3. 搜索 narrative_characters（active + reference 双状态）
        let mut stmt = conn.prepare(
            "SELECT id, name, personality, background, goals, status
             FROM narrative_characters
             WHERE story_id = ?1 AND (name LIKE ?2 OR personality LIKE ?2 OR background LIKE ?2 OR \
             goals LIKE ?2)
             LIMIT ?3",
        )?;
        let char_rows = stmt.query_map(params![story_id, &like_pattern, limit as i32], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let personality: String = row.get(2)?;
            let background: String = row.get(3)?;
            let goals: String = row.get(4)?;
            let status: String = row.get(5)?;
            Ok((id, name, personality, background, goals, status))
        })?;
        for row in char_rows {
            let (id, name, personality, background, goals, status) = row?;
            let content = format!(
                "角色: {}\n性格: {}\n背景: {}\n目标: {}",
                name, personality, background, goals
            );
            results.push(SearchResult {
                id: format!("nchar_{}", id),
                content: content.chars().take(300).collect::<String>(),
                score: if status == "active" { 0.85 } else { 0.75 },
                source_type: SourceType::Entity,
                metadata: serde_json::json!({"type": "narrative_character", "name": name, "status": status}),
            });
        }

        // 4. 搜索 narrative_scenes（active + reference 双状态）
        let mut stmt = conn.prepare(
            "SELECT id, title, summary, content, status
             FROM narrative_scenes
             WHERE story_id = ?1 AND (title LIKE ?2 OR summary LIKE ?2 OR content LIKE ?2)
             LIMIT ?3",
        )?;
        let scene_rows = stmt.query_map(params![story_id, &like_pattern, limit as i32], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let summary: String = row.get(2)?;
            let content: Option<String> = row.get(3)?;
            let status: String = row.get(4)?;
            Ok((id, title, summary, content, status))
        })?;
        for row in scene_rows {
            let (id, title, summary, content, status) = row?;
            let text = content.unwrap_or(summary);
            let display = format!("场景: {}\n{}", title, text);
            results.push(SearchResult {
                id: format!("nscene_{}", id),
                content: display.chars().take(300).collect::<String>(),
                score: if status == "active" { 0.82 } else { 0.72 },
                source_type: SourceType::Scene,
                metadata: serde_json::json!({"type": "narrative_scene", "title": title, "status": status}),
            });
        }

        // 5. 搜索 narrative_world_buildings（active + reference 双状态）
        let mut stmt = conn.prepare(
            "SELECT id, concept, rules, history, power_system, status
             FROM narrative_world_buildings
             WHERE story_id = ?1 AND (concept LIKE ?2 OR rules LIKE ?2 OR history LIKE ?2 OR \
             power_system LIKE ?2)
             LIMIT ?3",
        )?;
        let wb_rows = stmt.query_map(params![story_id, &like_pattern, limit as i32], |row| {
            let id: String = row.get(0)?;
            let concept: String = row.get(1)?;
            let rules: String = row.get(2)?;
            let history: String = row.get(3)?;
            let power_system: String = row.get(4)?;
            let status: String = row.get(5)?;
            Ok((id, concept, rules, history, power_system, status))
        })?;
        for row in wb_rows {
            let (id, concept, rules, history, power_system, status) = row?;
            let content = format!(
                "世界观: {}\n规则: {}\n历史: {}\n力量体系: {}",
                concept, rules, history, power_system
            );
            results.push(SearchResult {
                id: format!("nwb_{}", id),
                content: content.chars().take(300).collect::<String>(),
                score: if status == "active" { 0.88 } else { 0.78 },
                source_type: SourceType::Memory,
                metadata: serde_json::json!({"type": "narrative_world_building", "status": status}),
            });
        }

        // 去重并截断
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.dedup_by(|a, b| a.id == b.id);
        results.truncate(limit);

        Ok(results)
    }

    async fn search_with_embedding(
        &self,
        _story_id: &str,
        _embedding: Vec<f32>,
        _limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // DbVectorStore 不支持语义嵌入搜索，返回空结果
        Ok(vec![])
    }
}

/// 知识图谱接口（用于查询）
#[async_trait::async_trait]
pub trait KnowledgeGraph: Send + Sync {
    async fn find_entity_by_name(
        &self,
        name: &str,
    ) -> Result<Entity, Box<dyn std::error::Error + Send + Sync>>;

    async fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(Entity, f32)>, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(id: &str, score: f32) -> SearchResult {
        SearchResult {
            id: id.to_string(),
            content: format!("Content of {}", id),
            score,
            source_type: SourceType::Scene,
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn test_fuse_results_both_sides() {
        // Token 和语义都有同一文档 → 加权融合
        let token = vec![make_result("doc1", 0.8), make_result("doc2", 0.6)];
        let semantic = vec![make_result("doc1", 0.95), make_result("doc3", 0.9)];

        let fused = QueryPipeline::fuse_results(token, semantic);

        assert_eq!(fused.len(), 3);
        assert_eq!(fused[0].id, "doc1"); // 两边都有，分数最高
                                         // doc1 score = 0.8*0.4 + 0.95*0.6 = 0.32 + 0.57 = 0.89
        assert!(
            (fused[0].score - 0.89).abs() < 0.01,
            "doc1 score should be ~0.89, got {}",
            fused[0].score
        );
        // doc3 score = 0.9*(0.2 + 0.6) = 0.72 (只有语义，token 权重折半)
        assert_eq!(fused[1].id, "doc3");
        // doc2 score = 0.6*(0.4 + 0.3) = 0.42 (只有 token，语义权重折半)
        assert_eq!(fused[2].id, "doc2");
    }

    #[test]
    fn test_fuse_results_token_only() {
        let token = vec![make_result("a", 0.9), make_result("b", 0.7)];
        let semantic = vec![];

        let fused = QueryPipeline::fuse_results(token, semantic);

        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].id, "a");
        // 0.9 * (0.4 + 0.3) = 0.63
        assert!(
            (fused[0].score - 0.63).abs() < 0.01,
            "got {}",
            fused[0].score
        );
    }

    #[test]
    fn test_fuse_results_semantic_only() {
        let token = vec![];
        let semantic = vec![make_result("x", 0.85), make_result("y", 0.75)];

        let fused = QueryPipeline::fuse_results(token, semantic);

        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].id, "x");
        // 0.85 * (0.2 + 0.6) = 0.68
        assert!(
            (fused[0].score - 0.68).abs() < 0.01,
            "got {}",
            fused[0].score
        );
    }

    #[test]
    fn test_fuse_results_deduplicates() {
        let token = vec![make_result("dup", 0.5), make_result("dup", 0.8)];
        let semantic = vec![make_result("dup", 0.9), make_result("dup", 0.6)];

        let fused = QueryPipeline::fuse_results(token, semantic);

        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].id, "dup");
        // token max = 0.8, semantic max = 0.9 → 0.8*0.4 + 0.9*0.6 = 0.86
        assert!(
            (fused[0].score - 0.86).abs() < 0.01,
            "got {}",
            fused[0].score
        );
    }

    #[test]
    fn test_fuse_results_empty() {
        let fused = QueryPipeline::fuse_results(vec![], vec![]);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_fuse_results_truncates_to_50() {
        let token: Vec<_> = (0..60)
            .map(|i| make_result(&format!("t{}", i), 0.5))
            .collect();
        let semantic: Vec<_> = (0..60)
            .map(|i| make_result(&format!("s{}", i), 0.6))
            .collect();

        let fused = QueryPipeline::fuse_results(token, semantic);

        assert_eq!(fused.len(), 50);
    }
}
