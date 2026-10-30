use serde::{Deserialize, Serialize};

/// agency 角色：主创 / 管理 / 编辑审计 / 高频 agents 映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    LeadWriter,
    Producer,
    EditorAuditor,
    Writer,
    Inspector,
    OutlinePlanner,
    StyleMimic,
    Director,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::LeadWriter => "lead_writer",
            AgentRole::Producer => "producer",
            AgentRole::EditorAuditor => "editor_auditor",
            AgentRole::Writer => "writer",
            AgentRole::Inspector => "inspector",
            AgentRole::OutlinePlanner => "outline_planner",
            AgentRole::StyleMimic => "style_mimic",
            AgentRole::Director => "director",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lead_writer" => Some(AgentRole::LeadWriter),
            "producer" => Some(AgentRole::Producer),
            "editor_auditor" => Some(AgentRole::EditorAuditor),
            "writer" => Some(AgentRole::Writer),
            "inspector" => Some(AgentRole::Inspector),
            "outline_planner" => Some(AgentRole::OutlinePlanner),
            "style_mimic" => Some(AgentRole::StyleMimic),
            "director" => Some(AgentRole::Director),
            _ => None,
        }
    }

    pub fn all() -> [AgentRole; 8] {
        [
            AgentRole::LeadWriter,
            AgentRole::Producer,
            AgentRole::EditorAuditor,
            AgentRole::Writer,
            AgentRole::Inspector,
            AgentRole::OutlinePlanner,
            AgentRole::StyleMimic,
            AgentRole::Director,
        ]
    }

    /// 角色任务默认 run lane：审读类走 `review`，写入类走 `write`，
    /// 导演走 `inspect`。
    pub fn run_lane(&self) -> &'static str {
        match self {
            AgentRole::Director => "inspect",
            AgentRole::EditorAuditor | AgentRole::Inspector | AgentRole::StyleMimic => "review",
            AgentRole::LeadWriter
            | AgentRole::Producer
            | AgentRole::Writer
            | AgentRole::OutlinePlanner => "write",
        }
    }
}

/// 黑板分区：资产 / 草稿 / 审查 / 计划 / 导演
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardZone {
    Asset,
    Draft,
    Review,
    Schedule,
    Direction,
}

impl BoardZone {
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardZone::Asset => "asset",
            BoardZone::Draft => "draft",
            BoardZone::Review => "review",
            BoardZone::Schedule => "schedule",
            BoardZone::Direction => "direction",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "asset" => Some(BoardZone::Asset),
            "draft" => Some(BoardZone::Draft),
            "review" => Some(BoardZone::Review),
            "schedule" => Some(BoardZone::Schedule),
            "direction" => Some(BoardZone::Direction),
            _ => None,
        }
    }

    pub fn all() -> [BoardZone; 5] {
        [
            BoardZone::Asset,
            BoardZone::Draft,
            BoardZone::Review,
            BoardZone::Schedule,
            BoardZone::Direction,
        ]
    }

    /// 单一写入者原则：每个分区只有 owner 角色能直写（active），
    /// 其他角色的写入降级为提案（proposed），由协调器仲裁。
    pub fn owner(&self) -> AgentRole {
        match self {
            BoardZone::Asset => AgentRole::Producer,
            BoardZone::Draft => AgentRole::LeadWriter,
            BoardZone::Review => AgentRole::EditorAuditor,
            BoardZone::Schedule => AgentRole::Producer,
            BoardZone::Direction => AgentRole::Director,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyRun {
    pub id: String,
    pub story_id: Option<String>,
    pub premise: String,
    pub status: String,
    pub phase: String,
    /// 顶层 run 的并发车道：write | review | inspect | chat。
    /// V149 的唯一索引按 (story_id, lane) 判定活跃 run。
    pub lane: String,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub const OBSERVE_PREMISE: &str = "观察";

impl AgencyRun {
    pub fn new(id: impl Into<String>, premise: impl Into<String>) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            id: id.into(),
            story_id: None,
            premise: premise.into(),
            status: "pending".to_string(),
            phase: "concept".to_string(),
            lane: "write".to_string(),
            result_json: None,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn with_lane(mut self, lane: impl Into<String>) -> Self {
        self.lane = lane.into();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardItem {
    pub id: String,
    pub run_id: String,
    pub story_id: String,
    pub zone: BoardZone,
    pub item_type: String,
    pub key: String,
    pub content: String,
    pub summary: String,
    pub version: i32,
    pub producer: AgentRole,
    pub status: String, // active | proposed
    pub created_at: String,
    pub updated_at: String,
}

impl BoardItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        story_id: impl Into<String>,
        zone: BoardZone,
        item_type: impl Into<String>,
        key: impl Into<String>,
        content: impl Into<String>,
        summary: impl Into<String>,
        producer: AgentRole,
        status: impl Into<String>,
    ) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            story_id: story_id.into(),
            zone,
            item_type: item_type.into(),
            key: key.into(),
            content: content.into(),
            summary: summary.into(),
            version: 1,
            producer,
            status: status.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyMessage {
    pub id: String,
    pub run_id: String,
    pub from_role: AgentRole,
    pub to_role: AgentRole,
    pub msg_type: String, // proposal | note | alert
    pub payload: String,  // JSON
    pub created_at: String,
}

impl AgencyMessage {
    pub fn new(
        run_id: impl Into<String>,
        from_role: AgentRole,
        to_role: AgentRole,
        msg_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            from_role,
            to_role,
            msg_type: msg_type.into(),
            payload: payload.to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        }
    }
}

/// 代理活动日志条目：持久化 agency-agent-activity / agency-run-progress 事件。
/// 幕后代理工作室轮询拉取，不依赖 Tauri 实时事件到达隐藏窗口。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyActivityLogEntry {
    pub id: i64,
    pub run_id: String,
    /// 'activity' | 'progress'
    pub event_type: String,
    /// activity 事件的角色（lead_writer/producer/editor_auditor）
    pub role: Option<String>,
    /// activity 事件的动作（start/done）
    pub action: Option<String>,
    /// activity 事件的详情（概念/首章/深度资产/审查/装配）
    pub detail: Option<String>,
    /// progress 事件的阶段（concept/assets/writing/review/assembly）
    pub phase: Option<String>,
    /// progress 事件的状态（running/completed/failed）
    pub status: Option<String>,
    /// progress 事件的描述文案
    pub message: Option<String>,
    pub created_at: String,
}

/// 子 agent 任务进程：异步派发的最小可观测单位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgencyAgentTask {
    pub id: String,
    pub run_id: String,
    pub story_id: String,
    pub role: AgentRole,
    pub parent_task_id: Option<String>,
    pub task: String,
    pub status: String, /* queued | running | waiting | cancelling | completed | failed |
                         * cancelled | stale */
    pub phase: Option<String>,
    pub turn_count: i32,
    pub tool_calls: i32,
    pub last_tool: Option<String>,
    pub heartbeat_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub result_summary: Option<String>,
    pub meta_json: String,
    pub created_at: String,
}

impl AgencyAgentTask {
    pub fn new(
        run_id: impl Into<String>,
        story_id: impl Into<String>,
        role: AgentRole,
        task: impl Into<String>,
        parent_task_id: Option<String>,
        meta_json: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.into(),
            story_id: story_id.into(),
            role,
            parent_task_id,
            task: task.into(),
            status: "queued".to_string(),
            phase: None,
            turn_count: 0,
            tool_calls: 0,
            last_tool: None,
            heartbeat_at: None,
            started_at: None,
            finished_at: None,
            error: None,
            result_summary: None,
            meta_json: meta_json.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// 故事资源租约：写库提交点的互斥锁（章节号/场景/资产/草稿/质量门）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLease {
    pub id: String,
    pub story_id: String,
    pub resource: String,
    pub owner_run_id: Option<String>,
    pub owner_task_id: Option<String>,
    pub acquired_at: String,
    pub heartbeat_at: String,
    pub expires_at: String,
}

/// 剧组对话会话。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub story_id: String,
    pub title: String,
    pub target_role: String,
    pub summary: Option<String>,
    pub summary_upto_message_id: Option<String>,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 剧组对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String, // user | director | producer | editor_auditor | lead_writer | system
    pub content: String,
    pub status: String, /* pending | thinking | acting | waiting | done | failed | cancelled |
                         * degraded */
    pub run_key: Option<String>,
    pub task_id: Option<String>,
    pub task_ids_json: Option<String>,
    pub actions_json: Option<String>,
    pub meta_json: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

impl ChatMessage {
    pub fn new(
        session_id: impl Into<String>,
        role: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: role.into(),
            content: content.into(),
            status: "pending".to_string(),
            run_key: None,
            task_id: None,
            task_ids_json: None,
            actions_json: None,
            meta_json: None,
            error: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// 导演动作审计记录：调度/修复/确认的完整溯源。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorAction {
    pub id: String,
    pub story_id: String,
    pub session_id: Option<String>,
    pub message_id: Option<String>,
    pub run_id: Option<String>,
    pub action_type: String,
    pub target_role: Option<String>,
    pub target_id: Option<String>,
    pub params_json: String,
    pub status: String, // proposed | approved | executing | done | failed | rejected
    pub reason: Option<String>,
    pub result_summary: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
