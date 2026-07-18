use serde::{Deserialize, Serialize};

/// 三角色：主创 / 管理 / 编辑审计
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    LeadWriter,
    Producer,
    EditorAuditor,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::LeadWriter => "lead_writer",
            AgentRole::Producer => "producer",
            AgentRole::EditorAuditor => "editor_auditor",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lead_writer" => Some(AgentRole::LeadWriter),
            "producer" => Some(AgentRole::Producer),
            "editor_auditor" => Some(AgentRole::EditorAuditor),
            _ => None,
        }
    }

    pub fn all() -> [AgentRole; 3] {
        [
            AgentRole::LeadWriter,
            AgentRole::Producer,
            AgentRole::EditorAuditor,
        ]
    }
}

/// 黑板分区：资产 / 草稿 / 审查 / 调度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardZone {
    Asset,
    Draft,
    Review,
    Schedule,
}

impl BoardZone {
    pub fn as_str(&self) -> &'static str {
        match self {
            BoardZone::Asset => "asset",
            BoardZone::Draft => "draft",
            BoardZone::Review => "review",
            BoardZone::Schedule => "schedule",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "asset" => Some(BoardZone::Asset),
            "draft" => Some(BoardZone::Draft),
            "review" => Some(BoardZone::Review),
            "schedule" => Some(BoardZone::Schedule),
            _ => None,
        }
    }

    pub fn all() -> [BoardZone; 4] {
        [
            BoardZone::Asset,
            BoardZone::Draft,
            BoardZone::Review,
            BoardZone::Schedule,
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
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AgencyRun {
    pub fn new(id: impl Into<String>, premise: impl Into<String>) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            id: id.into(),
            story_id: None,
            premise: premise.into(),
            status: "pending".to_string(),
            phase: "concept".to_string(),
            result_json: None,
            error_message: None,
            created_at: now.clone(),
            updated_at: now,
        }
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
