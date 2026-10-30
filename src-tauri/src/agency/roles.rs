use crate::{agency::models::AgentRole, router::TaskType};

pub mod director;
pub mod inspector;
pub mod outline_planner;
pub mod style_mimic;
pub mod writer;

/// 角色规格：运行时之上的配置（提示词 + 路由任务类型 + 熔断参数）。
#[derive(Debug, Clone, Copy)]
pub struct RoleSpec {
    pub role: AgentRole,
    pub prompt_id: &'static str,
    pub task_type: TaskType,
    pub max_turns: usize,
    pub max_output_tokens: i32,
    /// 上下文注入预算（字符）：ToolLoop 会话窗口超预算时保留头尾截断。
    pub context_budget_chars: usize,
}

pub fn spec_for(role: AgentRole) -> RoleSpec {
    match role {
        AgentRole::LeadWriter => RoleSpec {
            role,
            prompt_id: "agency_lead_writer_system",
            task_type: TaskType::CreativeWriting,
            max_turns: 10,
            max_output_tokens: 8192,
            context_budget_chars: 24_000,
        },
        AgentRole::Producer => RoleSpec {
            role,
            prompt_id: "agency_producer_system",
            task_type: TaskType::WorldBuilding,
            // v0.73.3: 12 → 16。创世资产至少 5 类（世界观/角色/关系/大纲/
            // 伏笔），叠加初始读取与模型自愈重试，12 轮容易在写完关键资产
            // 前耗尽；16 轮给「写完即 final」留出余量。
            max_turns: 16,
            max_output_tokens: 4096,
            context_budget_chars: 16_000,
        },
        AgentRole::EditorAuditor => RoleSpec {
            role,
            prompt_id: "agency_editor_auditor_system",
            task_type: TaskType::Proofreading,
            max_turns: 6,
            max_output_tokens: 2048,
            context_budget_chars: 10_000,
        },
        AgentRole::Writer => writer::spec(),
        AgentRole::Inspector => inspector::spec(),
        AgentRole::OutlinePlanner => outline_planner::spec(),
        AgentRole::StyleMimic => style_mimic::spec(),
        AgentRole::Director => director::spec(),
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
        assert_eq!(
            spec_for(AgentRole::LeadWriter).task_type,
            TaskType::CreativeWriting
        );
        assert_eq!(
            spec_for(AgentRole::Producer).task_type,
            TaskType::WorldBuilding
        );
        assert_eq!(
            spec_for(AgentRole::EditorAuditor).task_type,
            TaskType::Proofreading
        );
    }

    #[test]
    fn test_agency_prompts_loadable() {
        // 仅验证既有三角色的提示词文件已存在；新映射角色使用占位 ID，
        // 运行时回退到 default_role_prompt（见 coordinator.rs）。
        for role in [
            AgentRole::LeadWriter,
            AgentRole::Producer,
            AgentRole::EditorAuditor,
            AgentRole::Director,
        ] {
            let id = spec_for(role).prompt_id;
            assert!(
                crate::prompts::registry::resolve_prompt_default(id).is_some(),
                "提示词应能被注册表加载: {}",
                id
            );
        }
    }

    #[test]
    fn test_new_role_specs() {
        assert_eq!(spec_for(AgentRole::Writer).role, AgentRole::Writer);
        assert_eq!(
            spec_for(AgentRole::Writer).task_type,
            TaskType::CreativeWriting
        );
        assert_eq!(spec_for(AgentRole::Inspector).role, AgentRole::Inspector);
        assert_eq!(
            spec_for(AgentRole::Inspector).task_type,
            TaskType::Proofreading
        );
        assert_eq!(
            spec_for(AgentRole::OutlinePlanner).role,
            AgentRole::OutlinePlanner
        );
        assert_eq!(
            spec_for(AgentRole::OutlinePlanner).task_type,
            TaskType::WorldBuilding
        );
        assert_eq!(spec_for(AgentRole::StyleMimic).role, AgentRole::StyleMimic);
        assert_eq!(
            spec_for(AgentRole::StyleMimic).task_type,
            TaskType::Analysis
        );
        assert_eq!(spec_for(AgentRole::Director).role, AgentRole::Director);
        assert_eq!(spec_for(AgentRole::Director).task_type, TaskType::Analysis);
        assert_eq!(
            spec_for(AgentRole::Director).prompt_id,
            "agency_director_system"
        );
    }
}
