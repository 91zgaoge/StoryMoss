//! 统一 Prompt 模板系统
//!
//! 核心理念：每个叙事元素的 Prompt 都有两种模式 —— Generate（生成）和
//! Extract（提取）。 生成模式用于 Bootstrap（从零创造），
//! 提取模式用于拆书（从文本分析）。 两种模式共享相同的输出结构（JSON
//! Schema），确保结果可以直接写入统一的数据模型。

/// Prompt 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    Generate, // 正向：从零生成
    Extract,  // 逆向：从文本提取
}

impl PromptMode {
    pub fn verb(&self) -> &'static str {
        match self {
            PromptMode::Generate => "生成",
            PromptMode::Extract => "提取",
        }
    }
}

// ==================== 故事概念 Prompt ====================

pub fn story_concept_prompt(mode: PromptMode, context: &str) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位资深小说编辑。请根据用户的创意，{}一个完整的故事概念。

用户输入："{}"

请用 JSON 格式回复：
{{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）"
}}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材必须严格遵循用户输入中的要求。如果用户明确提到了具体题材（如"都市玄幻"、"科幻"、"古言"等），则必须使用，不得擅自更改或替换为其他题材
4. 题材要具体，不要笼统"小说"
5. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            context.replace('"', "'")
        ),
        PromptMode::Extract => format!(
            r#"你是一位资深小说编辑。请从以下小说文本中，{}故事的基本信息。

文本片段：
{}

请用 JSON 格式回复：
{{
  "title": "小说标题（如无法确定则为null）",
  "description": "一句话简介（30-50字，如无法确定则为null）",
  "genre": "题材（如：玄幻、都市、穿越、科幻、武侠等）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "估计篇幅"
}}

要求：
1. 基于文本内容推断，不要虚构
2. 如某信息文本中未体现，标记为null
3. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            context
        ),
    }
}

// ==================== 世界观 Prompt ====================

pub fn world_building_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    context: &str,
) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位世界观架构师。请为以下故事{}完整的世界观设定。

故事：《{}》
题材：{}
简介：{}

请用 JSON 格式回复：
{{
  "concept": "世界观核心概念（50-100字）",
  "rules": [
    {{"name": "规则名称", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}}
  ],
  "history": "世界历史背景（200-300字）",
  "key_locations": ["关键地点1", "关键地点2"],
  "power_system": "力量体系概述（如有）"
}}

要求：
1. 规则要有创意，避免陈词滥调
2. 规则之间要有逻辑一致性
3. 重要规则（importance >= 8）不超过5条
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
        PromptMode::Extract => format!(
            r#"你是一位世界观分析专家。请从以下小说文本中，{}世界观设定。

故事：《{}》
题材：{}

文本片段：
{}

请用 JSON 格式回复：
{{
  "concept": "世界观核心概念（50-100字，基于文本推断）",
  "rules": [
    {{"name": "规则名称", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}}
  ],
  "history": "世界历史背景（基于文本推断，200-300字）",
  "key_locations": ["关键地点1", "关键地点2"],
  "power_system": "力量体系概述（如有）"
}}

要求：
1. 基于文本内容推断，不要虚构文本中不存在的信息
2. 规则从文本中的描写归纳总结
3. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
    }
}

// ==================== 角色 Prompt ====================

pub fn character_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    world_concept: &str,
    context: &str,
) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位角色设计师。请为以下故事{} 3-5 个主要角色。

故事：《{}》
题材：{}
世界观：{}
简介：{}

请用 JSON 格式回复：
{{
  "characters": [
    {{
      "name": "角色姓名",
      "role_type": "角色定位（主角/反派/导师/盟友/爱情线）",
      "personality": "性格特征（50字）",
      "background": "背景故事（100字）",
      "goals": "核心目标",
      "fears": "深层恐惧",
      "appearance": "外貌特征（50字）",
      "gender": "男/女/其他",
      "age": 25,
      "importance_score": 9,
      "relationships": [{{"target_name": "另一个角色名", "relation_type": "关系性质", "description": "关系描述"}}]
    }}
  ]
}}

要求：
1. 主角要有鲜明的性格弧光空间
2. 角色之间要有冲突和张力
3. 避免刻板印象
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            world_concept,
            context
        ),
        PromptMode::Extract => format!(
            r#"你是一位角色分析专家。请从以下小说文本中，{}所有出现的人物角色。

故事：《{}》
题材：{}

文本片段：
{}

请用 JSON 格式回复：
{{
  "characters": [
    {{
      "name": "人物姓名",
      "role_type": "角色定位（主角/反派/配角/龙套/提及）",
      "personality": "性格特征（基于文本描写）",
      "background": "背景故事（基于文本推断）",
      "goals": "核心目标（如有）",
      "fears": "深层恐惧（如有）",
      "appearance": "外貌描写（如有）",
      "gender": "男/女/其他",
      "age": 25,
      "importance_score": 7,
      "relationships": [{{"target_name": "另一个角色名", "relation_type": "关系性质", "description": "关系描述"}}]
    }}
  ]
}}

要求：
1. 只提取文本中实际出现或有明确描写的人物
2. 如某人仅被提及但未出场，role_type 标记为"提及"
3. importance_score 根据人物在文本中的重要性打分（1-10）
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
    }
}

// ==================== 场景 Prompt ====================

pub fn scene_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    character_names: &str,
    context: &str,
) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位大纲规划师。请为以下故事{} 8-12 个核心场景。

故事：《{}》
题材：{}
角色：{}
简介：{}

请用 JSON 格式回复：
{{
  "scenes": [
    {{
      "sequence_number": 1,
      "title": "场景标题",
      "summary": "场景内容摘要（100字）",
      "dramatic_goal": "本场景的戏剧目标",
      "external_pressure": "外部压力/阻碍",
      "conflict_type": "man_vs_man|man_vs_self|man_vs_society|man_vs_nature|man_vs_technology|man_vs_fate|man_vs_supernatural|man_vs_time|man_vs_morality|man_vs_identity|faction_vs_faction",
      "setting_location": "地点",
      "setting_time": "时间",
      "characters_present": ["角色名1", "角色名2"]
    }}
  ]
}}

要求：
1. 场景之间要有因果关系
2. 每个场景都要推动情节或揭示人物
3. 冲突类型要多样
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            character_names,
            context
        ),
        PromptMode::Extract => format!(
            r#"你是一位场景分析专家。请从以下小说文本中，{}所有场景/章节。

故事：《{}》
题材：{}

文本片段：
{}

请用 JSON 格式回复：
{{
  "scenes": [
    {{
      "sequence_number": 1,
      "title": "场景标题（如有）",
      "summary": "场景内容概要（100-200字）",
      "dramatic_goal": "本场景的戏剧目标（基于内容推断）",
      "external_pressure": "外部压力/阻碍（如有）",
      "conflict_type": "man_vs_man|man_vs_self|...",
      "setting_location": "地点",
      "setting_time": "时间",
      "characters_present": ["角色名1", "角色名2"],
      "key_events": ["关键事件1", "关键事件2"],
      "emotional_tone": "情感基调（如：紧张/温馨/悲伤/激昂）"
    }}
  ]
}}

要求：
1. 按文本顺序排列场景
2. 提取每个场景的核心冲突和情感基调
3. 列出场景中出场的所有人物
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
    }
}

// ==================== 大纲 Prompt ====================

pub fn outline_prompt(mode: PromptMode, story_title: &str, genre: &str, context: &str) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位资深故事架构师。请为以下故事{}一个完整的三幕式大纲。

故事：《{}》
题材：{}
简介：{}

请用 JSON 格式回复：
{{
  "acts": [
    {{
      "act_number": 1,
      "title": "第一幕标题",
      "summary": "本幕核心内容摘要（100字）",
      "key_plot_points": ["情节点1", "情节点2", "情节点3"],
      "estimated_scenes": 4
    }}
  ],
  "total_scenes_estimate": 12
}}

要求：
1. 严格三幕结构（起-承-转-合）
2. 每幕包含3-5个关键情节点
3. 场景数量要合理（第一幕3-5场，第二幕6-10场，第三幕3-5场）
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
        PromptMode::Extract => format!(
            r#"你是一位故事结构分析专家。请从以下小说文本（或章节概要）中，{}故事的三幕式大纲结构。

故事：《{}》
题材：{}

文本/概要：
{}

请用 JSON 格式回复：
{{
  "acts": [
    {{
      "act_number": 1,
      "title": "第一幕标题（基于内容推断）",
      "summary": "本幕核心内容摘要（100字）",
      "key_plot_points": ["情节点1", "情节点2"],
      "estimated_scenes": 4
    }}
  ],
  "total_scenes_estimate": 12
}}

要求：
1. 基于文本内容推断故事结构
2. 如果文本不完整（只有部分章节），只推断已读部分的结构
3. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
    }
}

// ==================== 伏笔 Prompt ====================

pub fn foreshadowing_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    outline_summary: &str,
    context: &str,
) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位资深编剧。请根据以下故事概念和大纲，{} 3-5 个核心伏笔。

故事：《{}》
题材：{}

故事大纲：
{}

请用 JSON 格式回复：
{{
  "foreshadowings": [
    {{
      "content": "伏笔内容描述",
      "importance": 8,
      "target_act": 2,
      "hint_style": "暗示风格（如：环境隐喻、对话暗示、物品象征、预言梦境）"
    }}
  ]
}}

要求：
1. 伏笔要贯穿多个幕次，具有回收价值
2. importance 1-10，核心伏笔不低于7
3. hint_style 要多样化
4. 第一个伏笔建议在第一章（第一幕）就埋下
5. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            outline_summary
        ),
        PromptMode::Extract => format!(
            r#"你是一位伏笔分析专家。请从以下小说文本中，{}所有伏笔（已埋设的暗示和线索）。

故事：《{}》
题材：{}

文本片段：
{}

请用 JSON 格式回复：
{{
  "foreshadowings": [
    {{
      "content": "伏笔内容描述（基于文本中的具体描写）",
      "importance": 8,
      "target_act": 2,
      "hint_style": "暗示风格（如：环境隐喻、对话暗示、物品象征、预言梦境）",
      "setup_scene": "埋设伏笔的场景描述"
    }}
  ]
}}

要求：
1. 只提取文本中实际存在的暗示和线索
2. 区分"已明确回收的伏笔"和"尚未回收的伏笔"
3. importance 根据伏笔对整体故事的重要性打分
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            genre,
            context
        ),
    }
}

// ==================== 故事线/弧光 Prompt ====================

pub fn story_arc_prompt(mode: PromptMode, story_title: &str, context: &str) -> String {
    match mode {
        PromptMode::Generate => format!(
            r#"你是一位故事结构专家。请为以下故事{}完整的故事线。

故事：《{}》
简介：{}

请用 JSON 格式回复：
{{
  "main_arc": "主线故事（简要概括）",
  "sub_arcs": ["支线1", "支线2"],
  "climaxes": ["高潮点1", "高潮点2"],
  "turning_points": ["转折点1", "转折点2"]
}}

要求：
1. 主线要清晰，有起承转合
2. 支线要与主线有机联系
3. 高潮点要分布在不同幕次
4. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            context
        ),
        PromptMode::Extract => format!(
            r#"你是一位故事线分析专家。请从以下小说章节概要中，{}故事线结构。

故事：《{}》

章节概要：
{}

请用 JSON 格式回复：
{{
  "main_arc": "主线故事（基于概要推断）",
  "sub_arcs": ["支线1", "支线2"],
  "climaxes": ["高潮点1", "高潮点2"],
  "turning_points": ["转折点1", "转折点2"]
}}

要求：
1. 基于章节概要推断故事结构
2. 如果文本不完整，标注"待补充"
3. 只输出 JSON，不要其他内容"#,
            mode.verb(),
            story_title,
            context
        ),
    }
}
