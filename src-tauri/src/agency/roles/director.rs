use super::RoleSpec;
use crate::{agency::models::AgentRole, router::TaskType};

/// 导演 Agent：总控，负责巡检、调度、修复与汇报。
/// P0 使用单次规划调用（inspect → plan → dispatch → follow-up），
/// max_turns 预留给 P1 的导演工具循环。
pub fn spec() -> RoleSpec {
    RoleSpec {
        role: AgentRole::Director,
        prompt_id: "agency_director_system",
        task_type: TaskType::Analysis,
        max_turns: 8,
        max_output_tokens: 4096,
        context_budget_chars: 32_000,
    }
}
