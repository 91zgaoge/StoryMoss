//! SING 意图图核心数据模型
//!
//! 定义意图节点、资产节点、执行图、边类型等核心结构。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

// ==================== 意图节点 ====================

/// 意图类型：原子意图（不可再分）、复合意图（多步组合）、合成意图（动态生成）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    Atomic,    // 原子意图：单一动词-宾语，如 "generate prose"
    Compound,  // 复合意图：多步组合，如 "write chapter → inspect → revise"
    Synthetic, // 合成意图：运行时动态发现，如 "enhance style based on user feedback"
}

impl std::fmt::Display for IntentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentType::Atomic => write!(f, "atomic"),
            IntentType::Compound => write!(f, "compound"),
            IntentType::Synthetic => write!(f, "synthetic"),
        }
    }
}

impl std::str::FromStr for IntentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "atomic" => Ok(IntentType::Atomic),
            "compound" => Ok(IntentType::Compound),
            "synthetic" => Ok(IntentType::Synthetic),
            _ => Err(format!("Unknown intent type: {}", s)),
        }
    }
}

/// 意图节点：全局归一化的动词-宾语短语
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionNode {
    pub id: String,
    pub intent_type: IntentType,
    pub verb: String,        // 动作动词
    pub object: String,      // 宾语对象
    pub description: String, // 自然语言描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>, // 语义嵌入向量
    pub frequency: i32,      // 出现频率（PPR 权重）
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl IntentionNode {
    /// 从动词-宾语创建原子意图
    pub fn atomic(verb: &str, object: &str, description: &str) -> Self {
        let id = format!("{}_{}", verb.to_lowercase(), object.to_lowercase());
        Self {
            id,
            intent_type: IntentType::Atomic,
            verb: verb.to_lowercase(),
            object: object.to_lowercase(),
            description: description.to_string(),
            embedding: None,
            frequency: 1,
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    /// 从 LLM 输出解析合成意图
    pub fn synthetic_from_llm(verb: &str, object: &str, description: &str) -> Self {
        let id = format!(
            "syn_{}_{}_{}",
            verb.to_lowercase(),
            object.to_lowercase(),
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        Self {
            id,
            intent_type: IntentType::Synthetic,
            verb: verb.to_lowercase(),
            object: object.to_lowercase(),
            description: description.to_string(),
            embedding: None,
            frequency: 1,
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    /// 意图的规范化文本表示（用于嵌入和匹配）
    pub fn canonical_text(&self) -> String {
        format!("{} {}", self.verb, self.object)
    }

    /// 增加频率计数
    pub fn increment_frequency(&mut self) {
        self.frequency += 1;
        self.updated_at = Local::now();
    }
}

// ==================== 资产节点 ====================

/// 资产类型：StoryForge 可调用的所有能力
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Skill,            // 内置技能（style_enhancer, character_voice 等）
    Methodology,      // 创作方法论（雪花法、英雄之旅等）
    StyleDna,         // 风格 DNA
    GenreProfile,     // 体裁画像
    McpTool,          // MCP 外部工具
    Agent,            // Agent（writer, inspector 等）
    SystemCommand,    // 系统命令（create_chapter, update_character 等）
    BeatCard,         // 节拍卡
    StoryEngine,      // 故事引擎
    PressureRelation, // 压力关系
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Skill => write!(f, "skill"),
            AssetType::Methodology => write!(f, "methodology"),
            AssetType::StyleDna => write!(f, "style_dna"),
            AssetType::GenreProfile => write!(f, "genre_profile"),
            AssetType::McpTool => write!(f, "mcp_tool"),
            AssetType::Agent => write!(f, "agent"),
            AssetType::SystemCommand => write!(f, "system_command"),
            AssetType::BeatCard => write!(f, "beat_card"),
            AssetType::StoryEngine => write!(f, "story_engine"),
            AssetType::PressureRelation => write!(f, "pressure_relation"),
        }
    }
}

impl std::str::FromStr for AssetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skill" => Ok(AssetType::Skill),
            "methodology" => Ok(AssetType::Methodology),
            "style_dna" => Ok(AssetType::StyleDna),
            "genre_profile" => Ok(AssetType::GenreProfile),
            "mcp_tool" => Ok(AssetType::McpTool),
            "agent" => Ok(AssetType::Agent),
            "system_command" => Ok(AssetType::SystemCommand),
            "beat_card" => Ok(AssetType::BeatCard),
            "story_engine" => Ok(AssetType::StoryEngine),
            "pressure_relation" => Ok(AssetType::PressureRelation),
            _ => Err(format!("Unknown asset type: {}", s)),
        }
    }
}

/// 资产节点：StoryForge 可调用的能力单元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetNode {
    pub id: String,
    pub asset_type: AssetType,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>, // 关联到 CapabilityRegistry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>, // 参数、约束、标签等
    pub frequency: i32,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl AssetNode {
    pub fn new(
        asset_type: AssetType,
        name: &str,
        description: &str,
        capability_id: Option<&str>,
    ) -> Self {
        let id = format!("{}_{}", asset_type, name.to_lowercase().replace(' ', "_"));
        Self {
            id,
            asset_type,
            name: name.to_string(),
            description: description.to_string(),
            embedding: None,
            capability_id: capability_id.map(|s| s.to_string()),
            metadata: None,
            frequency: 1,
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    /// 资产的规范化文本表示（用于嵌入和匹配）
    pub fn canonical_text(&self) -> String {
        format!("{}: {}", self.name, self.description)
    }

    /// 从 metadata JSON 中提取 tags（Phase 2：意图图资产发现增强）
    pub fn tags(&self) -> Vec<String> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("tags"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ==================== 边类型 ====================

/// 意图-资产边类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentionAssetEdgeType {
    HasIntention, // 资产拥有此意图（asset → intention）
    TriggeredBy,  // 意图触发此资产（intention → asset）
    Recommended,  // 推荐关联（基于协同过滤或历史）
}

impl std::fmt::Display for IntentionAssetEdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentionAssetEdgeType::HasIntention => write!(f, "has_intention"),
            IntentionAssetEdgeType::TriggeredBy => write!(f, "triggered_by"),
            IntentionAssetEdgeType::Recommended => write!(f, "recommended"),
        }
    }
}

impl std::str::FromStr for IntentionAssetEdgeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "has_intention" => Ok(IntentionAssetEdgeType::HasIntention),
            "triggered_by" => Ok(IntentionAssetEdgeType::TriggeredBy),
            "recommended" => Ok(IntentionAssetEdgeType::Recommended),
            _ => Err(format!("Unknown intention-asset edge type: {}", s)),
        }
    }
}

/// 资产-资产边类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetAssetEdgeType {
    ToolNext,    // 工具链：执行 A 后通常执行 B
    ToolCooccur, // 工具共现：A 和 B 经常一起使用
    DependsOn,   // 依赖：B 依赖 A 的输出
    Complements, // 互补：A 和 B 功能互补
}

impl std::fmt::Display for AssetAssetEdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetAssetEdgeType::ToolNext => write!(f, "tool_next"),
            AssetAssetEdgeType::ToolCooccur => write!(f, "tool_cooccur"),
            AssetAssetEdgeType::DependsOn => write!(f, "depends_on"),
            AssetAssetEdgeType::Complements => write!(f, "complements"),
        }
    }
}

impl std::str::FromStr for AssetAssetEdgeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tool_next" => Ok(AssetAssetEdgeType::ToolNext),
            "tool_cooccur" => Ok(AssetAssetEdgeType::ToolCooccur),
            "depends_on" => Ok(AssetAssetEdgeType::DependsOn),
            "complements" => Ok(AssetAssetEdgeType::Complements),
            _ => Err(format!("Unknown asset-asset edge type: {}", s)),
        }
    }
}

/// 意图-资产边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionAssetEdge {
    pub id: Option<i64>,
    pub intention_id: String,
    pub asset_id: String,
    pub edge_type: IntentionAssetEdgeType,
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub cooccurrence_count: i32,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

/// 资产-资产边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAssetEdge {
    pub id: Option<i64>,
    pub source_asset_id: String,
    pub target_asset_id: String,
    pub edge_type: AssetAssetEdgeType,
    pub weight: f64,
    pub cooccurrence_count: i32,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

// ==================== 执行图 ====================

/// 执行图状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionGraphStatus {
    Building,  // 正在构建图
    Executing, // 正在执行
    Completed, // 完成
    Failed,    // 失败
    Cancelled, // 取消
}

impl std::fmt::Display for ExecutionGraphStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionGraphStatus::Building => write!(f, "building"),
            ExecutionGraphStatus::Executing => write!(f, "executing"),
            ExecutionGraphStatus::Completed => write!(f, "completed"),
            ExecutionGraphStatus::Failed => write!(f, "failed"),
            ExecutionGraphStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for ExecutionGraphStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "building" => Ok(ExecutionGraphStatus::Building),
            "executing" => Ok(ExecutionGraphStatus::Executing),
            "completed" => Ok(ExecutionGraphStatus::Completed),
            "failed" => Ok(ExecutionGraphStatus::Failed),
            "cancelled" => Ok(ExecutionGraphStatus::Cancelled),
            _ => Err(format!("Unknown execution graph status: {}", s)),
        }
    }
}

/// 执行图：运行时动态构建的意图-资产执行实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    pub id: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_id: Option<String>,
    pub user_input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_intention_id: Option<String>,
    pub status: ExecutionGraphStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_json: Option<String>,
    pub created_at: DateTime<Local>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Local>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<i64>,
}

/// 执行节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionNodeStatus {
    Discovered, // 已发现，待执行
    Pending,    // 等待依赖
    Running,    // 执行中
    Completed,  // 完成
    Failed,     // 失败
    Skipped,    // 跳过
}

impl std::fmt::Display for ExecutionNodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionNodeStatus::Discovered => write!(f, "discovered"),
            ExecutionNodeStatus::Pending => write!(f, "pending"),
            ExecutionNodeStatus::Running => write!(f, "running"),
            ExecutionNodeStatus::Completed => write!(f, "completed"),
            ExecutionNodeStatus::Failed => write!(f, "failed"),
            ExecutionNodeStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for ExecutionNodeStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discovered" => Ok(ExecutionNodeStatus::Discovered),
            "pending" => Ok(ExecutionNodeStatus::Pending),
            "running" => Ok(ExecutionNodeStatus::Running),
            "completed" => Ok(ExecutionNodeStatus::Completed),
            "failed" => Ok(ExecutionNodeStatus::Failed),
            "skipped" => Ok(ExecutionNodeStatus::Skipped),
            _ => Err(format!("Unknown execution node status: {}", s)),
        }
    }
}

/// 发现来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
    Synthesis,       // 意图合成
    Ppr,             // PPR 图传播
    Semantic,        // 语义相似度
    OutputHeuristic, // 输出启发式
    LlmAssisted,     // LLM 辅助发现
}

impl std::fmt::Display for DiscoverySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoverySource::Synthesis => write!(f, "synthesis"),
            DiscoverySource::Ppr => write!(f, "ppr"),
            DiscoverySource::Semantic => write!(f, "semantic"),
            DiscoverySource::OutputHeuristic => write!(f, "output_heuristic"),
            DiscoverySource::LlmAssisted => write!(f, "llm_assisted"),
        }
    }
}

impl std::str::FromStr for DiscoverySource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "synthesis" => Ok(DiscoverySource::Synthesis),
            "ppr" => Ok(DiscoverySource::Ppr),
            "semantic" => Ok(DiscoverySource::Semantic),
            "output_heuristic" => Ok(DiscoverySource::OutputHeuristic),
            "llm_assisted" => Ok(DiscoverySource::LlmAssisted),
            _ => Err(format!("Unknown discovery source: {}", s)),
        }
    }
}

/// 执行节点：运行时动态发现的意图或资产节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: String,
    pub graph_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intention_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    pub status: ExecutionNodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>, // execution_node IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Value>,
    pub discovered_from: DiscoverySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<i64>,
    pub created_at: DateTime<Local>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Local>>,
}

// ==================== 发现结果 ====================

/// 资产发现结果（带评分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDiscoveryResult {
    pub asset: AssetNode,
    pub score: f64,          // 综合评分（0-1）
    pub semantic_score: f64, // 语义相似度
    pub intent_score: f64,   // 意图匹配度
    pub ppr_score: f64,      // 图传播分数
    pub collab_score: f64,   // 协同过滤分数
    pub reason: String,      // 为什么推荐此资产
}

/// 意图合成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSynthesisResult {
    pub root_intention: IntentionNode,
    pub sub_intentions: Vec<IntentionNode>,
    pub confidence: f64,              // 合成置信度
    pub chain_expansion: Vec<String>, // 链式扩展的中间意图
}

// ==================== 图统计 ====================

/// 意图图统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatistics {
    pub intention_count: i64,
    pub asset_count: i64,
    pub intention_asset_edge_count: i64,
    pub asset_asset_edge_count: i64,
    pub execution_graph_count: i64,
    pub execution_node_count: i64,
    pub top_intentions: Vec<(String, i32)>, // (intent_id, frequency)
    pub top_assets: Vec<(String, i32)>,     // (asset_id, frequency)
}

// ==================== 嵌入工具 ====================

/// 将浮点向量序列化为 JSON 字符串（用于数据库存储）
pub fn serialize_embedding(embedding: &[f32]) -> String {
    serde_json::to_string(embedding).unwrap_or_default()
}

/// 从 JSON 字符串反序列化浮点向量
pub fn deserialize_embedding(json: &str) -> Option<Vec<f32>> {
    serde_json::from_str(json).ok()
}

/// 计算余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
