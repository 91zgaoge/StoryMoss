use super::RoleSpec;
use crate::{agency::models::AgentRole, router::TaskType};

/// 高频 Inspector Agent 映射为 agency role（只读审查）。
///
/// 注：`agency_inspector_system` 自 v0.59.0 起有 bundled 资产
/// （resources/prompts/agency/agency_inspector_system.md）；无资产时回退到
/// `default_role_prompt`（见 coordinator）。不在 Agency 主流程。
pub fn spec() -> RoleSpec {
    RoleSpec {
        role: AgentRole::Inspector,
        prompt_id: "agency_inspector_system",
        task_type: TaskType::Proofreading,
        max_turns: 6,
        max_output_tokens: 2048,
        context_budget_chars: 10_000,
    }
}
