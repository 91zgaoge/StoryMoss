use super::RoleSpec;
use crate::{agency::models::AgentRole, router::TaskType};

/// 高频 Writer Agent 映射为 agency role。
///
/// 注：`agency_writer_system` 自 v0.59.0 起有 bundled 资产
/// （resources/prompts/agency/agency_writer_system.md），用户可在提示词页覆盖；
/// 之前无资产时回退到 `default_role_prompt` 的一句泛化提示。本角色不在
/// Agency genesis/continue 主流程（仅 LeadWriter/Producer/EditorAuditor），
/// 属 文思/planner 子系统映射。
pub fn spec() -> RoleSpec {
    RoleSpec {
        role: AgentRole::Writer,
        prompt_id: "agency_writer_system",
        task_type: TaskType::CreativeWriting,
        max_turns: 10,
        max_output_tokens: 8192,
        context_budget_chars: 24_000,
    }
}
