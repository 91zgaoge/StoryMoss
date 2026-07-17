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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specs_complete() {
        for role in AgentRole::all() {
            let spec = spec_for(role);
            assert!(spec.prompt_id.starts_with("agency_"));
            assert!(spec.max_turns >= 4);
            assert!(spec.max_output_tokens >= 1024);
        }
        assert_eq!(spec_for(AgentRole::LeadWriter).task_type, TaskType::CreativeWriting);
        assert_eq!(spec_for(AgentRole::Producer).task_type, TaskType::WorldBuilding);
        assert_eq!(spec_for(AgentRole::EditorAuditor).task_type, TaskType::Proofreading);
    }

    #[test]
    fn test_agency_prompts_loadable() {
        for role in AgentRole::all() {
            let id = spec_for(role).prompt_id;
            assert!(
                crate::prompts::registry::resolve_prompt_default(id).is_some(),
                "提示词应能被注册表加载: {}",
                id
            );
        }
    }
}
