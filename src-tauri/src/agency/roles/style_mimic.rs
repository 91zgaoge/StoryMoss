use super::RoleSpec;
use crate::{agency::models::AgentRole, router::TaskType};

/// 高频 StyleMimic Agent 映射为 agency role（风格分析/提取，只读）。
///
/// 注：`agency_style_mimic_system` 自 v0.59.0 起有 bundled 资产
/// （resources/prompts/agency/agency_style_mimic_system.md）；无资产时回退到
/// `default_role_prompt`（见 coordinator）。不在 Agency 主流程。
pub fn spec() -> RoleSpec {
    RoleSpec {
        role: AgentRole::StyleMimic,
        prompt_id: "agency_style_mimic_system",
        task_type: TaskType::Analysis,
        max_turns: 6,
        max_output_tokens: 2048,
        context_budget_chars: 10_000,
    }
}
