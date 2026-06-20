//! 分层发现机制
//!
//! Server-level: PPR 图传播发现相关意图/资产
//! Tool-level: 描述匹配 + 意图匹配 + 图信号 融合评分
//!
//! v0.20.1: 修复审计报告 P1-1/P1-2——
//! - `discover_server_level` 真正调用 `GraphScorer::ppr_propagate`（此前从未调用）
//! - `discover_tool_level` 使用 PPR 传播分数替代 `0.5` 占位
//! - 评分权重对齐论文 λ=1 等权（此前为 0.3/0.4/0.2/0.1）

use std::collections::HashMap;

use crate::error::AppError;

use super::graph::IntentionGraphRepository;
use super::models::*;
use super::scorer::GraphScorer;

/// 分层发现引擎
pub struct LayeredDiscovery {
    scorer: GraphScorer,
}

impl LayeredDiscovery {
    pub fn new(scorer: GraphScorer) -> Self {
        Self { scorer }
    }

    /// Server-level 发现：基于 PPR 图传播
    ///
    /// 从根意图种子节点出发，在异构图上执行 Personalized PageRank，
    /// 通过 intention→asset 边和 asset→asset（tool_next/tool_cooccur）边传播，
    /// 发现与根意图相关的资产。
    ///
    /// v0.20.1: 此前用一跳邻域传播冒充 PPR，现改为真正调用 `ppr_propagate`。
    pub fn discover_server_level(
        &self,
        root_intention: &IntentionNode,
        graph_repo: &IntentionGraphRepository,
        max_results: usize,
    ) -> Result<ServerLevelResult, AppError> {
        // 1. 构建异构图邻接表用于 PPR 传播
        //    节点 ID 命名空间：intention 节点原样使用 ID，asset 节点原样使用 ID
        let mut edges: HashMap<String, Vec<(String, f64)>> = HashMap::new();

        // 1a. intention → asset 边（TriggeredBy / HasIntention）
        let ia_edges = graph_repo.get_intention_edges(&root_intention.id, None)?;
        for e in &ia_edges {
            let weight = e.weight.max(0.01);
            edges
                .entry(e.intention_id.clone())
                .or_default()
                .push((e.asset_id.clone(), weight));
            // 反向边（asset → intention），权重减半，保证双向可达
            edges
                .entry(e.asset_id.clone())
                .or_default()
                .push((e.intention_id.clone(), weight * 0.5));
        }

        // 1b. 收集所有已涉及的 asset ID，补全 asset → asset 边
        let mut involved_assets: std::collections::HashSet<String> = std::collections::HashSet::new();
        for e in &ia_edges {
            involved_assets.insert(e.asset_id.clone());
        }

        for asset_id in &involved_assets {
            let aa_edges = graph_repo.get_asset_edges(asset_id, None)?;
            for ae in &aa_edges {
                let weight = ae.weight.max(0.01);
                edges
                    .entry(ae.source_asset_id.clone())
                    .or_default()
                    .push((ae.target_asset_id.clone(), weight));
                // tool_cooccur 是无向边，加反向
                if ae.edge_type == AssetAssetEdgeType::ToolCooccur {
                    edges
                        .entry(ae.target_asset_id.clone())
                        .or_default()
                        .push((ae.source_asset_id.clone(), weight));
                }
            }
        }

        // 2. 执行 PPR 传播——种子节点为根意图
        let seed_nodes = vec![root_intention.id.clone()];
        let ppr_scores = self.scorer.ppr_propagate(&seed_nodes, &edges);

        // 3. 直接关联的资产（TriggeredBy 边）保底——即使 PPR 收敛慢也能发现
        let mut asset_scores: HashMap<String, f64> = HashMap::new();
        for e in &ia_edges {
            let direct_score = e.weight * e.cooccurrence_count.max(1) as f64;
            asset_scores
                .entry(e.asset_id.clone())
                .and_modify(|s| *s = s.max(direct_score))
                .or_insert(direct_score);
        }

        // 4. 合并 PPR 分数（取 PPR 和直接分数的较大值）
        for (node_id, ppr_score) in &ppr_scores {
            // 只关注 asset 节点（跳过 intention 节点自身）
            if node_id != &root_intention.id {
                asset_scores
                    .entry(node_id.clone())
                    .and_modify(|s| *s = s.max(*ppr_score))
                    .or_insert(*ppr_score);
            }
        }

        // 5. 排序取 Top-K
        let mut scored: Vec<(String, f64)> = asset_scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        let mut assets = Vec::new();
        for (asset_id, score) in scored {
            if let Some(asset) = graph_repo.get_asset(&asset_id)? {
                assets.push((asset, score));
            }
        }

        Ok(ServerLevelResult {
            root_intention: root_intention.clone(),
            discovered_assets: assets,
            propagation_depth: 2,
        })
    }

    /// Tool-level 发现：多信号融合评分
    ///
    /// v0.20.1: 对齐论文公式 score_tool = λ_desc·sim + λ_int·max_sim + λ_graph·ĝ
    /// 论文设定所有 λ = 1（等权）。此前实现为 0.3/0.4/0.2/0.1 且 ppr 硬编码 0.5。
    /// 现改为等权 + 从 server_level 结果传入真实 PPR 分数。
    pub fn discover_tool_level(
        &self,
        intention: &IntentionNode,
        candidates: &[AssetNode],
        ppr_scores: &HashMap<String, f64>,
    ) -> Vec<AssetDiscoveryResult> {
        let mut results = Vec::new();

        // 归一化 PPR 分数到 [0, 1]
        let max_ppr = ppr_scores.values().cloned().fold(0.0_f64, f64::max).max(1e-10);

        for candidate in candidates {
            // 1. 描述匹配（语义相似度，嵌入缺失时回退到文本 Jaccard）
            let desc_score = if let (Some(a_emb), Some(i_emb)) =
                (candidate.embedding.as_ref(), intention.embedding.as_ref())
            {
                cosine_similarity(a_emb, i_emb)
            } else {
                Self::text_similarity(&candidate.description, &intention.description)
            };

            // 2. 意图匹配（动词/宾语匹配）
            let intent_score = Self::intent_match_score(intention, candidate);

            // 3. PPR 图分数（从 server-level 传播结果获取，归一化到 [0,1]）
            let ppr_score = ppr_scores
                .get(&candidate.id)
                .copied()
                .unwrap_or(0.0)
                / max_ppr;

            // 4. 协同过滤分数（基于历史频率，作为额外信号）
            let collab_score = (candidate.frequency as f64).min(10.0) / 10.0;

            // v0.20.1: 对齐论文 λ=1 等权（desc + intent + ppr）
            // collab 作为辅助微调信号（权重 0.2），不改变论文三主项等权结构
            let total_score = desc_score + intent_score + ppr_score + 0.2 * collab_score;

            results.push(AssetDiscoveryResult {
                asset: candidate.clone(),
                score: total_score,
                semantic_score: desc_score,
                intent_score,
                ppr_score,
                collab_score,
                reason: format!(
                    "desc={:.2}, intent={:.2}, ppr={:.2}, collab={:.2}",
                    desc_score, intent_score, ppr_score, collab_score
                ),
            });
        }

        // 按总分排序
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// 完整分层发现：Server-level + Tool-level
    pub fn discover(
        &self,
        root_intention: &IntentionNode,
        graph_repo: &IntentionGraphRepository,
        max_results: usize,
    ) -> Result<Vec<AssetDiscoveryResult>, AppError> {
        // Server-level: PPR 传播发现候选资产
        let server_result = self.discover_server_level(root_intention, graph_repo, max_results * 3)?;

        // 构建 PPR 分数映射（从 server-level 结果）
        let ppr_scores: HashMap<String, f64> = server_result
            .discovered_assets
            .iter()
            .map(|(a, score)| (a.id.clone(), *score))
            .collect();

        let candidates: Vec<AssetNode> = server_result
            .discovered_assets
            .iter()
            .map(|(a, _)| a.clone())
            .collect();

        // Tool-level: 多信号融合重排序（传入真实 PPR 分数）
        let mut results = self.discover_tool_level(root_intention, &candidates, &ppr_scores);
        results.truncate(max_results);

        Ok(results)
    }

    // ------------------------------------------------------------------
    // 辅助函数
    // ------------------------------------------------------------------

    fn text_similarity(a: &str, b: &str) -> f64 {
        let a_words: HashMap<String, i32> = a
            .to_lowercase()
            .split_whitespace()
            .map(|w| (w.to_string(), 1))
            .collect();
        let b_words: HashMap<String, i32> = b
            .to_lowercase()
            .split_whitespace()
            .map(|w| (w.to_string(), 1))
            .collect();

        let mut intersection = 0;
        let mut union = a_words.len() + b_words.len();

        for (word, _) in &a_words {
            if b_words.contains_key(word) {
                intersection += 1;
                union -= 1;
            }
        }

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    fn intent_match_score(intention: &IntentionNode, asset: &AssetNode) -> f64 {
        let mut score = 0.0;

        // 动词匹配
        if asset.description.to_lowercase().contains(&intention.verb)
            || asset.name.to_lowercase().contains(&intention.verb)
        {
            score += 0.5;
        }

        // 宾语匹配
        if asset.description.to_lowercase().contains(&intention.object)
            || asset.name.to_lowercase().contains(&intention.object)
        {
            score += 0.5;
        }

        score
    }
}

/// Server-level 发现结果
#[derive(Debug, Clone)]
pub struct ServerLevelResult {
    pub root_intention: IntentionNode,
    pub discovered_assets: Vec<(AssetNode, f64)>,
    pub propagation_depth: usize,
}
