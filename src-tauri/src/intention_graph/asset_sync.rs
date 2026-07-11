//! 意图图资产同步引擎
//!
//! 将 StoryMoss 现有的 CapabilityRegistry、SelectableAsset、Agent、Skill 等
//! 系统资产同步到意图图存储层，构建初始的意图-资产异构图。

use std::collections::HashMap;

use super::{graph::IntentionGraphRepository, models::*};
use crate::{
    capabilities::{Capability, CapabilityRegistry, CapabilitySource},
    error::AppError,
    strategy::SelectableAsset,
};

/// 资产同步引擎
/// 负责将系统现有资产一次性/增量同步到意图图数据库
pub struct AssetSyncEngine {
    repo: IntentionGraphRepository,
}

impl AssetSyncEngine {
    pub fn new(repo: IntentionGraphRepository) -> Self {
        Self { repo }
    }

    /// 全量同步：从 CapabilityRegistry 同步所有能力为资产节点
    pub fn sync_capabilities(&self, registry: &CapabilityRegistry) -> Result<usize, AppError> {
        let mut count = 0;
        for cap in registry.get_all() {
            let asset = capability_to_asset_node(cap);
            self.repo.create_asset(&asset)?;
            count += 1;

            // 同步能力的 when_to_use 作为意图节点，并建立边
            self.sync_capability_intentions(cap)?;
        }
        log::info!("[AssetSyncEngine] Synced {} capabilities", count);
        Ok(count)
    }

    /// 从 SelectableAsset 同步（方法论、风格 DNA、体裁画像等）
    pub fn sync_selectable_assets(&self, assets: &[SelectableAsset]) -> Result<usize, AppError> {
        let mut count = 0;
        for asset in assets {
            let node = selectable_asset_to_asset_node(asset);
            self.repo.create_asset(&node)?;
            count += 1;

            // 为每个资产创建默认意图关联
            self.create_default_intention_edges(&node)?;
        }
        log::info!("[AssetSyncEngine] Synced {} selectable assets", count);
        Ok(count)
    }

    /// 同步内置 Agent 为资产节点
    pub fn sync_builtin_agents(&self) -> Result<usize, AppError> {
        let agents = builtin_agents();
        let mut count = 0;
        for agent in agents {
            self.repo.create_asset(&agent)?;
            count += 1;

            // Agent 的默认意图边
            self.create_agent_intention_edges(&agent)?;
        }
        log::info!("[AssetSyncEngine] Synced {} builtin agents", count);
        Ok(count)
    }

    /// 同步系统命令为资产节点
    pub fn sync_system_commands(&self) -> Result<usize, AppError> {
        let commands = builtin_system_commands();
        let mut count = 0;
        for cmd in commands {
            self.repo.create_asset(&cmd)?;
            count += 1;
        }
        log::info!("[AssetSyncEngine] Synced {} system commands", count);
        Ok(count)
    }

    /// 构建资产-资产共现边（基于预设的创作流程模式）
    pub fn build_default_asset_edges(&self) -> Result<usize, AppError> {
        let mut count = 0;

        // 预设的创作流程边：writer -> inspector -> style_checker -> rewriter
        let workflow_edges = vec![
            (
                "agent_writer",
                "agent_inspector",
                AssetAssetEdgeType::ToolNext,
                0.9,
            ),
            (
                "agent_inspector",
                "skill_style_enhancer",
                AssetAssetEdgeType::ToolNext,
                0.7,
            ),
            (
                "skill_style_enhancer",
                "agent_rewriter",
                AssetAssetEdgeType::ToolNext,
                0.6,
            ),
            (
                "agent_writer",
                "skill_character_voice",
                AssetAssetEdgeType::ToolCooccur,
                0.5,
            ),
            (
                "agent_writer",
                "skill_emotion_pacing",
                AssetAssetEdgeType::ToolCooccur,
                0.5,
            ),
        ];

        for (source, target, edge_type, weight) in workflow_edges {
            // 容错：跳过尚未注册的资产（避免 FK 约束失败）
            // 部分技能资产仅在 SkillManager 加载后才存在
            if self.repo.get_asset(source)?.is_none() {
                log::debug!(
                    "[AssetSyncEngine] Skip edge: source '{}' not registered",
                    source
                );
                continue;
            }
            if self.repo.get_asset(target)?.is_none() {
                log::debug!(
                    "[AssetSyncEngine] Skip edge: target '{}' not registered",
                    target
                );
                continue;
            }
            let edge = AssetAssetEdge {
                id: None,
                source_asset_id: source.to_string(),
                target_asset_id: target.to_string(),
                edge_type,
                weight,
                cooccurrence_count: 1,
                created_at: chrono::Local::now(),
                updated_at: chrono::Local::now(),
            };
            self.repo.create_asset_asset_edge(&edge)?;
            count += 1;
        }

        log::info!("[AssetSyncEngine] Built {} default asset edges", count);
        Ok(count)
    }

    /// 完整初始化：同步所有资产并构建默认边
    pub fn full_initialize(
        &self,
        registry: &CapabilityRegistry,
        selectable_assets: &[SelectableAsset],
    ) -> Result<SyncStats, AppError> {
        let mut stats = SyncStats::default();

        stats.capabilities = self.sync_capabilities(registry)?;
        stats.selectable_assets = self.sync_selectable_assets(selectable_assets)?;
        stats.agents = self.sync_builtin_agents()?;
        stats.system_commands = self.sync_system_commands()?;
        stats.asset_edges = self.build_default_asset_edges()?;

        log::info!(
            "[AssetSyncEngine] Full initialization complete: {:?}",
            stats
        );
        Ok(stats)
    }

    // ------------------------------------------------------------------
    // 内部辅助
    // ------------------------------------------------------------------

    fn sync_capability_intentions(&self, cap: &Capability) -> Result<(), AppError> {
        // 将 capability 的 when_to_use 描述解析为意图关键词
        let intents = extract_intentions_from_description(&cap.when_to_use);

        for intent_text in intents {
            let parts: Vec<&str> = intent_text.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let mut intention = IntentionNode::atomic(parts[0], parts[1], &cap.when_to_use);
                // v0.20.1: 为意图节点生成嵌入，使语义相似度匹配生效
                intention.embedding = generate_embedding(&intention.description);
                self.repo.create_intention(&intention)?;

                // 建立意图 -> 资产边
                let edge = IntentionAssetEdge {
                    id: None,
                    intention_id: intention.id.clone(),
                    asset_id: cap.id.clone(),
                    edge_type: IntentionAssetEdgeType::TriggeredBy,
                    weight: 0.8,
                    reason: Some(format!("Derived from capability '{}' when_to_use", cap.id)),
                    cooccurrence_count: 1,
                    created_at: chrono::Local::now(),
                    updated_at: chrono::Local::now(),
                };
                self.repo.create_intention_asset_edge(&edge)?;
            }
        }

        Ok(())
    }

    fn create_default_intention_edges(&self, asset: &AssetNode) -> Result<(), AppError> {
        // 根据资产类型创建默认意图关联
        let default_intents = match asset.asset_type {
            AssetType::Skill => vec!["apply skill", "enhance content"],
            AssetType::Methodology => vec!["plan structure", "guide creation"],
            AssetType::StyleDna => vec!["enhance style", "apply style dna"],
            AssetType::GenreProfile => vec!["match genre", "apply genre"],
            AssetType::Agent => vec!["generate prose", "inspect quality", "revise content"],
            AssetType::SystemCommand => vec!["execute command", "manage data"],
            AssetType::McpTool => vec!["external search", "fetch data"],
            AssetType::BeatCard => vec!["plan structure", "design plot"],
            AssetType::StoryEngine => vec!["drive narrative", "generate plot"],
            AssetType::PressureRelation => vec!["create conflict", "build tension"],
        };

        for intent_text in default_intents {
            let parts: Vec<&str> = intent_text.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let mut intention = IntentionNode::atomic(parts[0], parts[1], &asset.description);
                // v0.20.1: 为意图节点生成嵌入
                intention.embedding = generate_embedding(&intention.description);
                // 使用 UPSERT，避免重复创建
                let _ = self.repo.create_intention(&intention);

                let edge = IntentionAssetEdge {
                    id: None,
                    intention_id: intention.id.clone(),
                    asset_id: asset.id.clone(),
                    edge_type: IntentionAssetEdgeType::TriggeredBy,
                    weight: 0.6,
                    reason: Some(format!("Default intent for {:?} asset", asset.asset_type)),
                    cooccurrence_count: 1,
                    created_at: chrono::Local::now(),
                    updated_at: chrono::Local::now(),
                };
                let _ = self.repo.create_intention_asset_edge(&edge);
            }
        }

        Ok(())
    }

    fn create_agent_intention_edges(&self, agent: &AssetNode) -> Result<(), AppError> {
        // Agent 特定的意图映射
        let agent_intents: HashMap<&str, Vec<&str>> = HashMap::from([
            (
                "agent_writer",
                vec!["generate prose", "write content", "create chapter"],
            ),
            (
                "agent_inspector",
                vec!["inspect quality", "check continuity", "audit content"],
            ),
            (
                "agent_rewriter",
                vec!["revise content", "rewrite text", "polish prose"],
            ),
            (
                "agent_style_checker",
                vec!["check style", "enhance style", "apply style dna"],
            ),
        ]);

        if let Some(intents) = agent_intents.get(agent.id.as_str()) {
            for intent_text in intents {
                let parts: Vec<&str> = intent_text.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let mut intention =
                        IntentionNode::atomic(parts[0], parts[1], &agent.description);
                    // v0.20.1: 为意图节点生成嵌入
                    intention.embedding = generate_embedding(&intention.description);
                    let _ = self.repo.create_intention(&intention);

                    let edge = IntentionAssetEdge {
                        id: None,
                        intention_id: intention.id.clone(),
                        asset_id: agent.id.clone(),
                        edge_type: IntentionAssetEdgeType::TriggeredBy,
                        weight: 0.9,
                        reason: Some(format!("Agent '{}' core intention", agent.name)),
                        cooccurrence_count: 1,
                        created_at: chrono::Local::now(),
                        updated_at: chrono::Local::now(),
                    };
                    let _ = self.repo.create_intention_asset_edge(&edge);
                }
            }
        }

        Ok(())
    }
}

/// 同步统计
#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    pub capabilities: usize,
    pub selectable_assets: usize,
    pub agents: usize,
    pub system_commands: usize,
    pub asset_edges: usize,
}

// ------------------------------------------------------------------
// 转换函数
// ------------------------------------------------------------------

fn capability_to_asset_node(cap: &Capability) -> AssetNode {
    let asset_type = match cap.source_type {
        CapabilitySource::Agent => AssetType::Agent,
        CapabilitySource::Skill => AssetType::Skill,
        CapabilitySource::McpTool => AssetType::McpTool,
        CapabilitySource::SystemCommand => AssetType::SystemCommand,
        CapabilitySource::Methodology => AssetType::Methodology,
        CapabilitySource::GenreProfile => AssetType::GenreProfile,
        CapabilitySource::StyleDna => AssetType::StyleDna,
        CapabilitySource::Workflow => AssetType::SystemCommand,
    };

    let mut tags: Vec<String> = vec![
        format!("{:?}", cap.source_type).to_lowercase(),
        cap.name.to_lowercase().replace(' ', "_"),
    ];
    tags.extend(
        cap.name
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string()),
    );
    tags.sort();
    tags.dedup();

    let mut metadata = HashMap::new();
    metadata.insert(
        "parameters".to_string(),
        serde_json::to_value(&cap.parameters).unwrap_or_default(),
    );
    metadata.insert(
        "source_type".to_string(),
        serde_json::json!(format!("{:?}", cap.source_type)),
    );
    metadata.insert(
        "tags".to_string(),
        serde_json::to_value(tags).unwrap_or_default(),
    );

    let mut node = AssetNode::new(asset_type, &cap.name, &cap.description, Some(&cap.id));
    node.metadata = Some(serde_json::to_value(metadata).unwrap_or_default());
    // v0.20.1: 生成语义嵌入，使 discover_tool_level 的描述匹配走余弦相似度
    // 而非回退到 Jaccard 词重叠（修复审计报告 P1-4）
    node.embedding = generate_embedding(&cap.description);
    node
}

fn selectable_asset_to_asset_node(asset: &SelectableAsset) -> AssetNode {
    let asset_type = match asset.kind {
        crate::strategy::AssetKind::Methodology => AssetType::Methodology,
        crate::strategy::AssetKind::GenreProfile => AssetType::GenreProfile,
        crate::strategy::AssetKind::StyleDna => AssetType::StyleDna,
        crate::strategy::AssetKind::Workflow => AssetType::SystemCommand,
        crate::strategy::AssetKind::Agent => AssetType::Agent,
        crate::strategy::AssetKind::Skill => AssetType::Skill,
        crate::strategy::AssetKind::SystemCommand => AssetType::SystemCommand,
        crate::strategy::AssetKind::McpTool => AssetType::McpTool,
        crate::strategy::AssetKind::BeatCard => AssetType::BeatCard,
        crate::strategy::AssetKind::StoryEngine => AssetType::StoryEngine,
        crate::strategy::AssetKind::PressureRelationship => AssetType::PressureRelation,
    };

    let mut tags: Vec<String> = vec![
        format!("{:?}", asset.kind).to_lowercase(),
        asset.name.to_lowercase().replace(' ', "_"),
    ];
    // 对 GenreProfile，把 aliases 和 canonical_name 也作为 tags，便于复合题材发现
    if asset.kind == crate::strategy::AssetKind::GenreProfile {
        if let Some(aliases) = asset.payload.get("aliases").and_then(|v| v.as_array()) {
            for alias in aliases {
                if let Some(s) = alias.as_str() {
                    tags.push(s.to_lowercase());
                }
            }
        }
        if let Some(canonical) = asset.payload.get("canonical_name").and_then(|v| v.as_str()) {
            tags.push(canonical.to_lowercase());
        }
    }
    tags.extend(
        asset
            .name
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string()),
    );
    tags.sort();
    tags.dedup();

    let mut node = AssetNode::new(asset_type, &asset.name, &asset.description, Some(&asset.id));
    node.metadata = Some(serde_json::json!({
        "when_to_use": asset.when_to_use,
        "input_description": asset.input_description,
        "output_description": asset.output_description,
        "payload": asset.payload,
        "tags": tags,
    }));
    // v0.20.1: 生成语义嵌入（修复审计报告 P1-4）
    node.embedding = generate_embedding(&asset.description);
    node
}

/// 内置 Agent 资产定义
fn builtin_agents() -> Vec<AssetNode> {
    let mut agents = vec![
        AssetNode::new(
            AssetType::Agent,
            "writer",
            "生成小说正文内容，根据上下文和风格要求创作高质量 prose",
            Some("agent_writer"),
        ),
        AssetNode::new(
            AssetType::Agent,
            "inspector",
            "审校生成内容，检查连续性、人物一致性、风格合规性",
            Some("agent_inspector"),
        ),
        AssetNode::new(
            AssetType::Agent,
            "rewriter",
            "根据审校反馈重写和润色内容，提升质量",
            Some("agent_rewriter"),
        ),
        AssetNode::new(
            AssetType::Agent,
            "style_checker",
            "检查文本是否符合目标 Style DNA 的量化指标",
            Some("agent_style_checker"),
        ),
    ];
    // v0.20.1: 为内置 Agent 生成语义嵌入
    for agent in &mut agents {
        agent.embedding = generate_embedding(&agent.description);
        agent.metadata = Some(serde_json::json!({
            "tags": ["agent", agent.name.to_lowercase()]
        }));
    }
    agents
}

/// 内置系统命令资产定义
fn builtin_system_commands() -> Vec<AssetNode> {
    let mut commands = vec![
        AssetNode::new(
            AssetType::SystemCommand,
            "create_chapter",
            "创建新章节并关联到当前故事",
            Some("cmd_create_chapter"),
        ),
        AssetNode::new(
            AssetType::SystemCommand,
            "update_character",
            "更新角色信息并同步到知识图谱",
            Some("cmd_update_character"),
        ),
        AssetNode::new(
            AssetType::SystemCommand,
            "update_scene",
            "更新场景内容并标记需要重写",
            Some("cmd_update_scene"),
        ),
        AssetNode::new(
            AssetType::SystemCommand,
            "ingest_content",
            "将内容提取到知识图谱和向量索引",
            Some("cmd_ingest_content"),
        ),
    ];
    // v0.20.1: 为内置系统命令生成语义嵌入
    for cmd in &mut commands {
        cmd.embedding = generate_embedding(&cmd.description);
        cmd.metadata = Some(serde_json::json!({
            "tags": ["system_command", cmd.name.to_lowercase()]
        }));
    }
    commands
}

/// 从描述文本中提取意图关键词（简单启发式）
fn extract_intentions_from_description(description: &str) -> Vec<String> {
    let mut intentions = Vec::new();
    let normalized = description.to_lowercase();

    // 常见动词映射
    let verb_hints = [
        ("write", "generate prose"),
        ("generate", "generate prose"),
        ("create", "generate prose"),
        ("inspect", "inspect quality"),
        ("check", "inspect quality"),
        ("audit", "inspect quality"),
        ("revise", "revise content"),
        ("rewrite", "revise content"),
        ("polish", "enhance style"),
        ("enhance", "enhance style"),
        ("style", "enhance style"),
        ("plan", "plan structure"),
        ("outline", "plan structure"),
        ("structure", "plan structure"),
        ("character", "manage character"),
        ("world", "manage world building"),
        ("search", "external search"),
        ("fetch", "fetch data"),
    ];

    for (hint, intent) in verb_hints {
        if normalized.contains(hint) {
            intentions.push(intent.to_string());
        }
    }

    // 去重
    intentions.sort();
    intentions.dedup();

    intentions
}

/// 为文本生成语义嵌入向量
///
/// v0.20.1: 修复审计报告 P1-4——此前 AssetSyncEngine 不生成嵌入，
/// 导致 discover_tool_level 的描述匹配退化为 Jaccard 词重叠。
///
/// 优先使用全局嵌入 provider（Ollama/OpenAI），失败时 graceful fallback
/// 到 None（discover_tool_level 会回退到文本 Jaccard 匹配）。
///
/// 注意：embed_text 内部访问全局 OnceLock，未初始化时会 panic（unwrap on
/// None）。 先检查 OnceLock 是否已初始化，避免 panic；再用 catch_unwind 兜底。
fn generate_embedding(text: &str) -> Option<Vec<f32>> {
    // 快速检查：若 EMBEDDING_CACHE 全局状态未初始化，直接返回 None
    // 避免 embed_text 内部的 .unwrap() panic
    if !crate::embeddings::is_embedding_initialized() {
        return None;
    }

    let result = std::panic::catch_unwind(|| crate::embeddings::embed_text(text));
    match result {
        Ok(Ok(emb)) => Some(emb),
        Ok(Err(e)) => {
            log::debug!(
                "[AssetSyncEngine] 嵌入生成失败（降级为 None，描述匹配将回退到文本相似度）: {}",
                e
            );
            None
        }
        Err(_) => {
            log::debug!("[AssetSyncEngine] 嵌入生成 panic（全局状态未初始化），降级为 None");
            None
        }
    }
}
