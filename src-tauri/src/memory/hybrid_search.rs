//! 混合搜索 - Phase 1.3
//!
//! 结合 BM25 文本搜索和向量相似度搜索
//! 使用 RRF (Reciprocal Rank Fusion) 融合排序

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    db::models::Entity,
    embeddings::embedding::embed_text_async,
    vector::lancedb_store::{LanceVectorStore, SearchResult as VectorSearchResult},
};

/// 混合搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub id: String,
    pub content: String,
    pub bm25_score: f32,
    pub vector_score: f32,
    pub hybrid_score: f32,
    pub source_type: SourceType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Scene,
    Entity,
    Memory,
    Note,
}

/// 混合搜索配置
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// BM25 权重
    pub bm25_weight: f32,
    /// 向量搜索权重
    pub vector_weight: f32,
    /// RRF 融合参数 k
    pub rrf_k: f32,
    /// 每路搜索返回数量
    pub top_k_per_route: usize,
    /// 最终结果数量
    pub final_top_k: usize,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            bm25_weight: 0.4,
            vector_weight: 0.6,
            rrf_k: 60.0,
            top_k_per_route: 20,
            final_top_k: 10,
        }
    }
}

/// BM25 搜索引擎
pub struct Bm25Search {
    // 倒排索引: token -> [(doc_id, tf), ...]
    inverted_index: HashMap<String, Vec<(String, f32)>>,
    // 文档长度
    doc_lengths: HashMap<String, usize>,
    // 平均文档长度
    avg_doc_length: f32,
    // 总文档数
    total_docs: usize,
}

impl Bm25Search {
    pub fn new() -> Self {
        Self {
            inverted_index: HashMap::new(),
            doc_lengths: HashMap::new(),
            avg_doc_length: 0.0,
            total_docs: 0,
        }
    }

    /// 添加文档到索引
    pub fn add_document(&mut self, doc_id: &str, content: &str) {
        let tokens = self.tokenize(content);
        let doc_length = tokens.len();

        // 统计词频
        let mut tf_map: HashMap<String, usize> = HashMap::new();
        for token in tokens {
            *tf_map.entry(token).or_insert(0) += 1;
        }

        // 更新倒排索引
        for (token, freq) in tf_map {
            let tf = freq as f32 / doc_length as f32;
            self.inverted_index
                .entry(token)
                .or_default()
                .push((doc_id.to_string(), tf));
        }

        // 更新文档长度
        self.doc_lengths.insert(doc_id.to_string(), doc_length);
        self.total_docs += 1;

        // 重新计算平均文档长度
        let total_length: usize = self.doc_lengths.values().sum();
        self.avg_doc_length = total_length as f32 / self.total_docs as f32;
    }

    /// 搜索
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f32)> {
        let tokens = self.tokenize(query);
        let mut scores: HashMap<String, f32> = HashMap::new();

        for token in tokens {
            if let Some(postings) = self.inverted_index.get(&token) {
                // 计算 IDF
                let idf = self.compute_idf(postings.len());

                for (doc_id, tf) in postings {
                    // 获取文档长度
                    let doc_length = self.doc_lengths.get(doc_id).copied().unwrap_or(0) as f32;

                    // BM25 公式
                    let score = self.bm25_score(*tf, idf, doc_length);
                    *scores.entry(doc_id.clone()).or_insert(0.0) += score;
                }
            }
        }

        // 排序并返回 Top-K
        let mut results: Vec<(String, f32)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.into_iter().take(top_k).collect()
    }

    /// CJK 二元组分词
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = vec![];
        let chars: Vec<char> = text.to_lowercase().chars().collect();

        // 二元组分词
        for i in 0..chars.len().saturating_sub(1) {
            let bigram: String = chars[i..=i + 1].iter().collect();
            tokens.push(bigram);
        }

        // 如果只有一个字符
        if chars.len() == 1 {
            tokens.push(chars[0].to_string());
        }

        tokens
    }

    /// 计算 IDF
    fn compute_idf(&self, doc_freq: usize) -> f32 {
        let n = self.total_docs as f32;
        let df = doc_freq as f32;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// BM25 评分
    fn bm25_score(&self, tf: f32, idf: f32, doc_length: f32) -> f32 {
        let k1 = 1.5; // 词频饱和参数
        let b = 0.75; // 长度归一化参数

        let norm_tf =
            tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * doc_length / self.avg_doc_length.max(1.0)));

        idf * norm_tf
    }
}

/// 混合搜索引擎
pub struct HybridSearch {
    bm25: Bm25Search,
    config: HybridSearchConfig,
}

impl HybridSearch {
    pub fn new(config: HybridSearchConfig) -> Self {
        Self {
            bm25: Bm25Search::new(),
            config,
        }
    }

    /// 添加文档
    pub fn add_document(&mut self, doc_id: &str, content: &str) {
        self.bm25.add_document(doc_id, content);
    }

    /// 执行混合搜索
    pub async fn search(
        &self,
        query: &str,
        story_id: &str,
        vector_store: &LanceVectorStore,
    ) -> Result<Vec<HybridSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // 1. BM25 搜索
        let bm25_results = self.bm25.search(query, self.config.top_k_per_route);

        // 2. 向量搜索
        let query_embedding = embed_text_async(query.to_string())
            .await
            .map_err(|e| format!("嵌入失败: {}", e))?;
        let vector_results = vector_store
            .search(story_id, query_embedding, self.config.top_k_per_route)
            .await?;

        // 3. RRF 融合排序
        let fused_results = self.rrf_fusion(&bm25_results, &vector_results);

        // 4. 构建最终结果
        let mut results = vec![];
        for (doc_id, hybrid_score, bm25_score, vector_score) in
            fused_results.iter().take(self.config.final_top_k)
        {
            // 从向量结果或 BM25 结果中获取内容
            let content = self.find_content(doc_id, &bm25_results, &vector_results);

            results.push(HybridSearchResult {
                id: doc_id.clone(),
                content,
                bm25_score: *bm25_score,
                vector_score: *vector_score,
                hybrid_score: *hybrid_score,
                source_type: SourceType::Scene, // 默认类型，实际需要根据文档ID判断
                metadata: HashMap::new(),
            });
        }

        Ok(results)
    }

    /// RRF (Reciprocal Rank Fusion) 融合排序
    fn rrf_fusion(
        &self,
        bm25_results: &[(String, f32)],
        vector_results: &[VectorSearchResult],
    ) -> Vec<(String, f32, f32, f32)> {
        let mut rrf_scores: HashMap<String, (f32, f32, f32)> = HashMap::new(); // doc_id -> (rrf_score, bm25_score, vector_score)
        let k = self.config.rrf_k;

        // 处理 BM25 结果
        for (rank, (doc_id, score)) in bm25_results.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);
            let entry = rrf_scores.entry(doc_id.clone()).or_insert((0.0, 0.0, 0.0));
            entry.0 += rrf_score * self.config.bm25_weight;
            entry.1 = *score;
        }

        // 处理向量结果
        for (rank, result) in vector_results.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);
            let entry = rrf_scores
                .entry(result.id.clone())
                .or_insert((0.0, 0.0, 0.0));
            entry.0 += rrf_score * self.config.vector_weight;
            entry.2 = result.score;
        }

        // 转换为向量并排序
        let mut results: Vec<(String, f32, f32, f32)> = rrf_scores
            .into_iter()
            .map(|(doc_id, (rrf, bm25, vector))| (doc_id, rrf, bm25, vector))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results
    }

    fn find_content(
        &self,
        doc_id: &str,
        _bm25_results: &[(String, f32)],
        vector_results: &[VectorSearchResult],
    ) -> String {
        // 尝试从向量结果中查找
        if let Some(result) = vector_results.iter().find(|r| r.id == doc_id) {
            return result.text.clone();
        }

        // 否则返回 doc_id 作为占位符
        doc_id.to_string()
    }
}

/// 实体混合搜索器
pub struct EntityHybridSearch;

impl EntityHybridSearch {
    /// 搜索实体（结合名称匹配和向量相似度）
    pub async fn search_entities(
        query: &str,
        entities: &[Entity],
        top_k: usize,
    ) -> Result<Vec<(Entity, f32)>, Box<dyn std::error::Error + Send + Sync>> {
        // 1. 名称匹配（BM25简化版）
        let name_scores: HashMap<String, f32> = entities
            .iter()
            .map(|e| {
                let score = Self::name_match_score(query, &e.name);
                (e.id.clone(), score)
            })
            .collect();

        // 2. 向量相似度（如果查询可以嵌入）
        let query_embedding: Option<Vec<f32>> = embed_text_async(query.to_string()).await.ok();

        // 3. 融合排序
        let mut combined_scores: Vec<(Entity, f32)> = entities
            .iter()
            .map(|e| {
                let name_score = name_scores.get(&e.id).copied().unwrap_or(0.0);

                let vector_score = if let Some(ref query_emb) = query_embedding {
                    if let Some(ref entity_emb) = e.embedding {
                        Self::cosine_similarity(query_emb, entity_emb)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                // 加权融合
                let combined = name_score * 0.3 + vector_score * 0.7;
                (e.clone(), combined)
            })
            .collect();

        // 排序并返回 Top-K
        combined_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(combined_scores.into_iter().take(top_k).collect())
    }

    /// 名称匹配分数
    fn name_match_score(query: &str, name: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let name_lower = name.to_lowercase();

        if name_lower == query_lower {
            return 1.0;
        }

        if name_lower.contains(&query_lower) {
            return 0.8;
        }

        if query_lower.contains(&name_lower) {
            return 0.6;
        }

        // 计算字符重叠度
        let query_chars: std::collections::HashSet<char> = query_lower.chars().collect();
        let name_chars: std::collections::HashSet<char> = name_lower.chars().collect();

        let intersection: std::collections::HashSet<_> =
            query_chars.intersection(&name_chars).collect();

        if !query_chars.is_empty() {
            intersection.len() as f32 / query_chars.len() as f32 * 0.5
        } else {
            0.0
        }
    }

    /// 余弦相似度
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot_product / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_search() {
        let mut bm25 = Bm25Search::new();

        bm25.add_document("doc1", "这是一个测试文档");
        bm25.add_document("doc2", "这是另一个文档用于测试");
        bm25.add_document("doc3", "完全不同的内容");

        let results = bm25.search("测试文档", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "doc1"); // 最相关的应该是 doc1
    }

    #[test]
    fn test_name_match_score() {
        assert_eq!(EntityHybridSearch::name_match_score("张三", "张三"), 1.0);
        assert_eq!(EntityHybridSearch::name_match_score("张三", "张三丰"), 0.8);
        assert!(EntityHybridSearch::name_match_score("张三", "李四") < 0.5);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((EntityHybridSearch::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((EntityHybridSearch::cosine_similarity(&a, &c)).abs() < 0.001);
    }
}
