#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub system_prompt: String,
    pub user_prompt_template: String,
}

impl PromptTemplate {
    pub fn render(&self, variables: &[(String, String)]) -> String {
        let mut result = self.user_prompt_template.clone();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
}

pub struct PromptLibrary;

impl PromptLibrary {
    pub fn chapter_generation() -> PromptTemplate {
        PromptTemplate {
            name: "chapter_generation".to_string(),
            system_prompt: r#"You are a professional creative writing assistant specializing in Chinese fiction.
Your task is to write engaging, well-structured story chapters based on the provided outline.

Guidelines:
1. Write in Chinese (简体中文)
2. Maintain consistent character voices and personalities
3. Show, don't tell - use vivid descriptions and dialogue
4. Create atmosphere appropriate to the genre
5. End with a hook that makes readers want to continue"#.to_string(),
            user_prompt_template: r#"Please write Chapter {chapter_number} based on the following outline:

Outline:
{outline}

Story Context:
- Genre: {genre}
- Tone: {tone}
- Pacing: {pacing}

Requirements:
- Word count: approximately 1500-2000 Chinese characters
- Include both narrative and dialogue
- Advance the plot while developing characters

Write the chapter now:"#.to_string(),
        }
    }

    pub fn character_analysis() -> PromptTemplate {
        PromptTemplate {
            name: "character_analysis".to_string(),
            system_prompt: "You are a character development expert. Analyze character consistency \
                            and suggest trait updates based on their actions in the story."
                .to_string(),
            user_prompt_template: r#"Analyze the following character's behavior in this chapter:

Character: {character_name}
Background: {character_background}
Current Traits: {current_traits}

Chapter Content:
{chapter_content}

Please:
1. Identify any new personality traits revealed
2. Note any contradictions with established character
3. Suggest dynamic trait updates with confidence scores (0.0-1.0)

Respond in JSON format with an array of traits:"#
                .to_string(),
        }
    }

    pub fn plot_consistency_check() -> PromptTemplate {
        PromptTemplate {
            name: "plot_consistency".to_string(),
            system_prompt: "You are a story editor specializing in continuity and plot \
                            consistency."
                .to_string(),
            user_prompt_template: r#"Check this chapter for plot consistency:

New Chapter:
{chapter_content}

Previous Context:
{previous_chapters}

Story Bible:
{story_bible}

Identify any:
1. Timeline inconsistencies
2. Contradictions with previous events
3. Character behavior that conflicts with established traits
4. Unexplained plot developments"#
                .to_string(),
        }
    }

    /// LitSeg 叙事事件提取提示 — 从文本中提取推动情节发展的关键事件
    pub fn narrative_event_extraction() -> PromptTemplate {
        PromptTemplate {
            name: "narrative_event_extraction".to_string(),
            system_prompt:
                "你是一个专业的叙事分析专家。你的任务是从小说文本中提取推动情节发展的关键事件。\n\n\
分析标准：\n\
1. 「有效事件」= 真正推动情节发展的关键节点，不是过渡性描述\n\
2. 事件强度（0.0-1.0）反映对后续情节的影响程度\n\
3. 如果角色发生内在改变（信念、态度、关系本质），标记为角色弧光\n\
4. 伏笔埋设和回收是独立事件，即使在同一场景中\n\
5. 保持与已有事件链的因果一致性\n\n\
输出 JSON 格式的事件数组。"
                    .to_string(),
            user_prompt_template: "请分析以下文本，提取推动情节发展的关键事件。\n\n\
【角色列表】\n\
{characters}\n\n\
【已有事件链（前序）】\n\
{prior_events}\n\n\
【当前文本】\n\
{content}\n\n\
请输出 JSON 格式的事件数组，每个事件包含：\n\
- event_type: 事件类型（从以下选择）\n\
  - introduction: 开场/介绍\n\
  - turning_point: 转折点\n\
  - climax: 高潮\n\
  - resolution: 回落\n\
  - revelation: 揭示\n\
  - conflict_eruption: 冲突爆发\n\
  - character_arc: 角色弧光\n\
  - foreshadow_setup: 伏笔埋设\n\
  - foreshadow_payoff: 伏笔回收\n\
  - transition: 过渡\n\
- intensity: 事件强度（0.0-1.0）\n\
- sentiment: 情感极性（-1.0 到 +1.0）\n\
- description: 事件描述（20-50字）\n\
- involved_character_ids: 涉及的角色 ID 数组\n\
- conflict_types: 涉及的冲突类型数组\n\n\
只输出 JSON，不要其他文字。"
                .to_string(),
        }
    }

    /// LitSeg 叙事结构分析提示 — 基于事件强度分布推断幕结构
    pub fn narrative_structure_analysis() -> PromptTemplate {
        PromptTemplate {
            name: "narrative_structure_analysis".to_string(),
            system_prompt: "你是一个专业的叙事结构分析专家。你的任务是基于事件强度分布，推断故事的幕级结构。\n\n\
分析标准：\n\
1. 基于亚里士多德五幕结构：起（开端）→ 承（发展）→ 转（转折）→ 合（高潮/结局）\n\
2. 高潮点 = 事件强度达到局部最大值的位置\n\
3. 幕边界 = 事件强度发生显著突变的位置（变化 > 0.3）\n\
4. 每个事件标注其在幕中的位置和戏剧功能\n\n\
输出 JSON 格式的结构分析。".to_string(),
            user_prompt_template: "请基于以下事件强度时间线，分析故事的叙事结构。\n\n\
【事件时间线】\n\
{event_timeline}\n\n\
请输出 JSON 格式的分析结果：\n\
- acts: 幕数组，每个幕包含：\n\
  - act_number: 幕编号（1-4）\n\
  - act_type: 幕类型（introduction/development/turn/resolution）\n\
  - start_chapter: 起始章节\n\
  - end_chapter: 结束章节\n\
  - summary: 幕摘要\n\
- positions: 事件位置数组，每个包含：\n\
  - event_id: 事件 ID\n\
  - act_number: 所属幕编号\n\
  - position_in_act: 在幕中的相对位置（0.0-1.0）\n\
  - dramatic_function: 戏剧功能（prologue/rising_action/climax/falling_action/catastrophe/peripeteia/anagnorisis/transition）\n\
  - is_narrative_boundary: 是否在叙事边界上\n\n\
只输出 JSON，不要其他文字。".to_string(),
        }
    }
}
