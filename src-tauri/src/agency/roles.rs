use crate::agency::models::AgentRole;
use crate::router::TaskType;

/// 角色规格：三角色 = 运行时之上的配置（提示词 + 路由任务类型 + 熔断参数）。
#[derive(Debug, Clone, Copy)]
pub struct RoleSpec {
    pub role: AgentRole,
    pub prompt_id: &'static str,
    pub task_type: TaskType,
    pub max_turns: usize,
    pub max_output_tokens: i32,
}

pub fn spec_for(role: AgentRole) -> RoleSpec {
    match role {
        AgentRole::LeadWriter => RoleSpec {
            role,
            prompt_id: "agency_lead_writer_system",
            task_type: TaskType::CreativeWriting,
            max_turns: 10,
            max_output_tokens: 8192,
        },
        AgentRole::Producer => RoleSpec {
            role,
            prompt_id: "agency_producer_system",
            task_type: TaskType::WorldBuilding,
            max_turns: 12,
            max_output_tokens: 4096,
        },
        AgentRole::EditorAuditor => RoleSpec {
            role,
            prompt_id: "agency_editor_auditor_system",
            task_type: TaskType::Proofreading,
            max_turns: 6,
            max_output_tokens: 2048,
        },
    }
}
