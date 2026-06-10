//! 场景结构 (Scene Structure) 提示词模板
//!
//! 基于 Dwight V. Swain 的场景-续接 (Scene-Sequel) 模型。
//! 每个场景 = 目标-冲突-灾难，每个续接 = 反应-困境-决定。

/// 获取场景结构第 N 步的系统提示词
pub fn get_structure_prompt(step: usize) -> &'static str {
    match step {
        1 => SCENE_GOAL_CONFLICT_DISASTER,
        2 => SEQUEL_REACTION_DILEMMA_DECISION,
        3 => SCENE_SEQUEL_VARIATIONS,
        _ => SCENE_GOAL_CONFLICT_DISASTER,
    }
}

pub const SCENE_GOAL_CONFLICT_DISASTER: &str = r#"【场景结构：目标-冲突-灾难】
请按以下结构设计一个场景：

1. 目标 (Goal)
   - 这个场景中，POV角色想要什么？
   - 目标必须具体、可衡量、有时限
   - 目标必须与整体情节相关联

2. 冲突 (Conflict)
   - 什么力量阻碍了角色达成目标？
   - 冲突可以来自：对手、环境、自身、时间
   - 冲突必须升级，不能原地踏步

3. 灾难 (Disaster)
   - 场景结尾发生了什么，让情况更糟？
   - 灾难可以是：目标失败、发现更可怕的事实、胜利的代价
   - 必须让读者想知道"接下来怎么办？"

场景上下文：{{scene_context}}"#;

pub const SEQUEL_REACTION_DILEMMA_DECISION: &str = r#"【续接结构：反应-困境-决定】
上一场景以灾难结尾，现在写续接：

1. 反应 (Reaction)
   - 角色的即时情感反应（震惊、愤怒、悲伤、恐惧）
   - 生理反应（颤抖、心跳、呼吸）
   - 不要立即理性分析，先让情感流淌

2. 困境 (Dilemma)
   - 角色面临的选择：A 或 B？
   - 每个选项的代价和收益
   - 时间压力：必须尽快决定
   - 展示角色的价值观和性格

3. 决定 (Decision)
   - 角色做出了什么选择？
   - 这个决定如何设定下一个场景的目标？
   - 决定的代价是什么？

上一场景灾难：{{previous_disaster}}"#;

pub const SCENE_SEQUEL_VARIATIONS: &str = r#"【场景-续接变体】
根据叙事需要，场景-续接结构可以灵活变化：

变体 A：快速场景（动作场景）
- 目标 → 冲突 → 灾难（快节奏，跳过续接）
- 适用：追逐、打斗、紧急事件

变体 B：长续接（内心戏）
- 反应（详细心理描写）→ 困境（深度思考）→ 决定（艰难抉择）
- 适用：转折点、角色成长时刻

变体 C：连续场景（多POV）
- 场景A（POV1）灾难 → 场景B（POV2）目标
- 适用：平行叙事、群像剧

变体 D：无灾难场景（虚假胜利）
- 目标 → 冲突 → 看似成功（但埋下更大隐患）
- 适用：反转前奏、喜剧节奏

当前叙事需求：{{narrative_need}}"#;
