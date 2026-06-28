//! 统一 Prompt 模板系统
//!
//! 核心理念：每个叙事元素的 Prompt 都有两种模式 —— Generate（生成）和
//! Extract（提取）。 生成模式用于 Bootstrap（从零创造），
//! 提取模式用于拆书（从文本分析）。 两种模式共享相同的输出结构（JSON
//! Schema），确保结果可以直接写入统一的数据模型。

use crate::db::DbPool;

/// Prompt 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    Generate, // 正向：从零生成
    Extract,  // 逆向：从文本提取
}

impl PromptMode {
    fn verb(&self) -> &'static str {
        match self {
            PromptMode::Generate => "生成",
            PromptMode::Extract => "提取",
        }
    }
}

/// v0.21.0: 从 PromptRegistry 读取模板并渲染变量
///
/// 若 registry 不可用或 key 不存在，回退到提供的默认模板。
fn resolve_and_render(
    prompt_id: &str,
    default_template: &str,
    vars: &[(&str, &str)],
    pool: Option<&DbPool>,
) -> String {
    let template = if let Some(pool) = pool {
        crate::prompts::registry::resolve_prompt(pool, prompt_id)
            .unwrap_or_else(|_| default_template.to_string())
    } else {
        crate::prompts::registry::resolve_prompt_default(prompt_id)
            .unwrap_or_else(|| default_template.to_string())
    };

    let mut vars_map = std::collections::HashMap::new();
    for (k, v) in vars {
        vars_map.insert(k.to_string(), v.to_string());
    }
    crate::prompts::engine::TemplateEngine::render_with_conditions(&template, &vars_map)
}

// ==================== 故事概念 Prompt ====================

pub fn story_concept_prompt(
    mode: PromptMode,
    context: &str,
    available_profiles: Option<&[crate::db::GenreProfile]>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let profiles_json = available_profiles.map_or_else(
                || "[]".to_string(),
                |profiles| {
                    serde_json::to_string(
                        &profiles
                            .iter()
                            .map(|p| {
                                serde_json::json!({
                                    "id": p.id,
                                    "genre_name": p.genre_name,
                                    "canonical_name": p.canonical_name,
                                    "aliases": p.aliases_json.as_deref().unwrap_or("[]"),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                    .unwrap_or_else(|_| "[]".to_string())
                },
            );
            resolve_and_render(
                "narrative_story_concept_generate",
                r#"你是一位资深小说编辑。请根据用户的创意，生成一个完整的故事概念。

用户输入："{{user_input}}"

可选题材画像目录（仅供标准化映射，可单选或多选）：
{{genre_profiles}}

请用 JSON 格式回复：
{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言）",
  "genre_profile_ids": ["从上述目录中挑选最匹配的题材画像 id，可单选或多选"],
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）"
}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材必须严格遵循用户输入中的要求
4. 题材要具体，不要笼统"小说"
5. 如果用户输入包含复合题材（如"异星球末世生存"），请尽量映射多个 genre_profile_ids
6. 如果目录中没有精确匹配，允许返回空数组，但必须在 genre 字段保留原始题材描述
7. 只输出 JSON，不要其他内容"#,
                &[
                    ("user_input", &context.replace('"', "'")),
                    ("genre_profiles", &profiles_json),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_story_concept_extract",
            r#"你是一位资深小说编辑。请从以下小说文本中，提取故事的基本信息。

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "title": "小说标题（如无法确定则为null）",
  "description": "一句话简介（30-50字，如无法确定则为null）",
  "genre": "题材（如：玄幻、都市、穿越、科幻、武侠等）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "估计篇幅"
}

要求：
1. 基于文本内容推断，不要虚构
2. 如某信息文本中未体现，标记为null
3. 只输出 JSON，不要其他内容"#,
            &[("text", context)],
            pool,
        ),
    }
}

// ==================== 世界观 Prompt ====================

pub fn world_building_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    context: &str,
    strategy_context: Option<&str>,
    narrative_quartet: Option<&str>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let strategy_section = strategy_context
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【创作策略参考】\n{}\n", s))
                .unwrap_or_default();
            let quartet_section = narrative_quartet
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【中文叙事四件套】\n{}\n", s))
                .unwrap_or_default();
            resolve_and_render(
                "narrative_world_building_generate",
                &format!(
                    r#"你是一位世界观架构师。请为以下故事生成完整的世界观设定。

故事：《{{story_title}}》
题材：{{genre}}
简介：{{story_description}}{{{{strategy_section}}}}{{{{quartet_section}}}}

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
4. 必须遵循【创作策略参考】中的体裁画像、方法论等约束
5. 只输出 JSON"#
                ),
                &[
                    ("story_title", story_title),
                    ("genre", genre),
                    ("story_description", context),
                    ("strategy_section", &strategy_section),
                    ("quartet_section", &quartet_section),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_world_building_extract",
            r#"你是一位世界观分析专家。请从以下小说文本中，提取世界观设定。

故事：《{{title}}》
题材：{{genre}}

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "concept": "世界观核心概念（50-100字，基于文本推断）",
  "rules": [
    {"name": "规则名称", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}
  ],
  "history": "世界历史背景（基于文本推断，200-300字）",
  "key_locations": ["关键地点1", "关键地点2"],
  "power_system": "力量体系概述（如有）"
}

要求：
1. 基于文本内容推断，不要虚构
2. 规则从文本中的描写归纳总结
3. 只输出 JSON"#,
            &[("title", story_title), ("genre", genre), ("text", context)],
            pool,
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
    strategy_context: Option<&str>,
    narrative_quartet: Option<&str>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let strategy_section = strategy_context
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【创作策略参考】\n{}\n", s))
                .unwrap_or_default();
            let quartet_section = narrative_quartet
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【中文叙事四件套】\n{}\n", s))
                .unwrap_or_default();
            resolve_and_render(
                "narrative_character_generate",
                &format!(
                    r#"你是一位角色设计师。请为以下故事生成 3-5 个主要角色。

故事：《{{story_title}}》
题材：{{genre}}
世界观：{{world_concept}}
简介：{{outline_summary}}{{{{strategy_section}}}}{{{{quartet_section}}}}

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
2. 角色之间要有冲突和张力，可参考【中文叙事四件套】中的高压关系
3. 避免刻板印象
4. 命名多样性，禁用最常见单字姓，禁止单字名，姓氏不得重复
5. 角色应有鲜明外貌、性别、年龄
6. 必须遵循【创作策略参考】中的体裁画像、方法论等约束
7. 只输出 JSON"#
                ),
                &[
                    ("story_title", story_title),
                    ("genre", genre),
                    ("world_concept", world_concept),
                    ("outline_summary", context),
                    ("strategy_section", &strategy_section),
                    ("quartet_section", &quartet_section),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_character_extract",
            r#"你是一位角色分析专家。请从以下小说文本中，提取所有出现的人物角色。

故事：《{{title}}》
题材：{{genre}}

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "characters": [
    {
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
      "relationships": [{"target_name": "另一个角色名", "relation_type": "关系性质", "description": "关系描述"}]
    }
  ]
}

要求：
1. 只提取文本中实际出现或有明确描写的人物
2. 仅被提及但未出场，role_type 标记为"提及"
3. importance_score 根据重要性打分（1-10）
4. 只输出 JSON"#,
            &[("title", story_title), ("genre", genre), ("text", context)],
            pool,
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
    strategy_context: Option<&str>,
    narrative_quartet: Option<&str>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let strategy_section = strategy_context
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【创作策略参考】\n{}\n", s))
                .unwrap_or_default();
            let quartet_section = narrative_quartet
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【中文叙事四件套】\n{}\n", s))
                .unwrap_or_default();
            resolve_and_render(
                "narrative_scene_generate",
                &format!(
                    r#"你是一位大纲规划师。请为以下故事生成 8-12 个核心场景。

故事：《{{story_title}}》
题材：{{genre}}
角色：{{characters}}
简介：{{outline_summary}}{{{{strategy_section}}}}{{{{quartet_section}}}}

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
3. 冲突类型要多样，可参考【中文叙事四件套】中的剧情引擎与高压关系
4. 必须遵循【创作策略参考】中的场景结构方法论、体裁画像等约束
5. 只输出 JSON"#
                ),
                &[
                    ("story_title", story_title),
                    ("genre", genre),
                    ("characters", character_names),
                    ("outline_summary", context),
                    ("strategy_section", &strategy_section),
                    ("quartet_section", &quartet_section),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_scene_extract",
            r#"你是一位场景分析专家。请从以下小说文本中，提取所有场景/章节。

故事：《{{title}}》
题材：{{genre}}

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "scenes": [
    {
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
    }
  ]
}

要求：
1. 按文本顺序排列场景
2. 提取每个场景的核心冲突和情感基调
3. 列出场景中出场的所有人物
4. 只输出 JSON"#,
            &[("title", story_title), ("genre", genre), ("text", context)],
            pool,
        ),
    }
}

// ==================== 大纲 Prompt ====================

pub fn outline_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    context: &str,
    strategy_context: Option<&str>,
    narrative_quartet: Option<&str>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let strategy_section = strategy_context
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【创作策略参考】\n{}\n", s))
                .unwrap_or_default();
            let quartet_section = narrative_quartet
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【中文叙事四件套】\n{}\n", s))
                .unwrap_or_default();
            resolve_and_render(
                "narrative_outline_generate",
                &format!(
                    r#"你是一位资深故事架构师。请为以下故事生成一个完整的三幕式大纲。

故事：《{{story_title}}》
题材：{{genre}}
简介：{{world_summary}}{{{{strategy_section}}}}{{{{quartet_section}}}}

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
3. 场景数量要合理
4. 可参考【中文叙事四件套】中的剧情引擎、桥段卡、高压关系来设计情节点
5. 必须遵循【创作策略参考】中的方法论、体裁画像等约束
6. 只输出 JSON"#
                ),
                &[
                    ("story_title", story_title),
                    ("genre", genre),
                    ("world_summary", context),
                    ("strategy_section", &strategy_section),
                    ("quartet_section", &quartet_section),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_outline_extract",
            r#"你是一位故事结构分析专家。请从以下小说文本（或章节概要）中，提取故事的三幕式大纲结构。

故事：《{{title}}》
题材：{{genre}}

文本/概要：
{{text}}

请用 JSON 格式回复：
{
  "acts": [
    {
      "act_number": 1,
      "title": "第一幕标题（基于内容推断）",
      "summary": "本幕核心内容摘要（100字）",
      "key_plot_points": ["情节点1", "情节点2"],
      "estimated_scenes": 4
    }
  ],
  "total_scenes_estimate": 12
}

要求：
1. 基于文本内容推断故事结构
2. 如果文本不完整，只推断已读部分的结构
3. 只输出 JSON"#,
            &[("title", story_title), ("genre", genre), ("text", context)],
            pool,
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
    strategy_context: Option<&str>,
    narrative_quartet: Option<&str>,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => {
            let strategy_section = strategy_context
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【创作策略参考】\n{}\n", s))
                .unwrap_or_default();
            let quartet_section = narrative_quartet
                .filter(|s| !s.is_empty())
                .map(|s| format!("\n【中文叙事四件套】\n{}\n", s))
                .unwrap_or_default();
            resolve_and_render(
                "narrative_foreshadowing_generate",
                &format!(
                    r#"你是一位资深编剧。请根据以下故事概念和大纲，设计 3-5 个核心伏笔。

故事：《{{story_title}}》
题材：{{genre}}

故事大纲：
{{outline_summary}}{{{{strategy_section}}}}{{{{quartet_section}}}}

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
4. 第一个伏笔建议在第一章就埋下
5. 可参考【中文叙事四件套】中的剧情引擎、桥段卡来设计伏笔埋设方向
6. 只输出 JSON"#
                ),
                &[
                    ("story_title", story_title),
                    ("genre", genre),
                    ("outline_summary", outline_summary),
                    ("strategy_section", &strategy_section),
                    ("quartet_section", &quartet_section),
                ],
                pool,
            )
        }
        PromptMode::Extract => resolve_and_render(
            "narrative_foreshadowing_extract",
            r#"你是一位伏笔分析专家。请从以下小说文本中，提取所有伏笔（已埋设的暗示和线索）。

故事：《{{title}}》
题材：{{genre}}

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "foreshadowings": [
    {
      "content": "伏笔内容描述（基于文本中的具体描写）",
      "importance": 8,
      "target_act": 2,
      "hint_style": "暗示风格（如：环境隐喻、对话暗示、物品象征、预言梦境）",
      "setup_scene": "埋设伏笔的场景描述"
    }
  ]
}

要求：
1. 只提取文本中实际存在的暗示和线索
2. 区分已明确回收的伏笔和尚未回收的伏笔
3. importance 根据伏笔对整体故事的重要性打分
4. 只输出 JSON"#,
            &[("title", story_title), ("genre", genre), ("text", context)],
            pool,
        ),
    }
}

// ==================== 故事线/弧光 Prompt ====================

pub fn story_arc_prompt(
    mode: PromptMode,
    story_title: &str,
    context: &str,
    pool: Option<&DbPool>,
) -> String {
    match mode {
        PromptMode::Generate => resolve_and_render(
            "narrative_story_arc_generate",
            r#"你是一位故事结构专家。请为以下故事生成完整的故事线。

故事：《{{story_title}}》
简介：{{outline_summary}}

请用 JSON 格式回复：
{
  "main_arc": "主线故事（简要概括）",
  "sub_arcs": ["支线1", "支线2"],
  "climaxes": ["高潮点1", "高潮点2"],
  "turning_points": ["转折点1", "转折点2"]
}

要求：
1. 主线要清晰，有起承转合
2. 支线要与主线有机联系
3. 高潮点要分布在不同幕次
4. 只输出 JSON"#,
            &[("story_title", story_title), ("outline_summary", context)],
            pool,
        ),
        PromptMode::Extract => resolve_and_render(
            "narrative_story_arc_extract",
            r#"你是一位故事线分析专家。请从以下小说章节概要中，提取故事线结构。

故事：《{{title}}》

章节概要：
{{text}}

请用 JSON 格式回复：
{
  "main_arc": "主线故事（基于概要推断）",
  "sub_arcs": ["支线1", "支线2"],
  "climaxes": ["高潮点1", "高潮点2"],
  "turning_points": ["转折点1", "转折点2"]
}

要求：
1. 基于章节概要推断故事结构
2. 如果文本不完整，标注待补充
3. 只输出 JSON"#,
            &[("title", story_title), ("text", context)],
            pool,
        ),
    }
}

// ==================== 第一章正文 Prompt (v0.23.61) ====================

pub fn first_chapter_prompt(
    title: &str,
    genre: &str,
    tone: &str,
    pacing: &str,
    description: &str,
    themes: &str,
    strategy_notes: &str,
    narrative_quartet: &str,
    run_mode: &str,
    conflict_level: i32,
    pace: &str,
    ai_freedom: &str,
    user_premise: &str,
    word_count: u32,
    genre_tips: &str,
    pool: Option<&DbPool>,
) -> String {
    let vars: &[(&str, &str)] = &[
        ("story_title", title),
        ("genre", genre),
        ("tone", tone),
        ("pacing", pacing),
        ("description", description),
        ("themes", themes),
        ("strategy_notes", strategy_notes),
        ("narrative_quartet", narrative_quartet),
        ("run_mode", run_mode),
        ("conflict_level", &conflict_level.to_string()),
        ("pace", pace),
        ("ai_freedom", ai_freedom),
        ("user_premise", user_premise),
        ("word_count", &word_count.to_string()),
        ("genre_tips", genre_tips),
    ];
    resolve_and_render(
        "narrative_first_chapter_generate",
        "你是一名专业的小说作家。请根据故事设定撰写第一章开头，目标{{word_count}}字。",
        vars,
        pool,
    )
}

// ==================== 第一场景正文 Prompt (Phase 4) ====================

/// Phase 4: 场景优先创世——以 Scene 为单位生成正文，
/// 注入场景级戏剧结构变量（dramatic_goal, conflict_type, characters_present 等）。
pub fn first_scene_prompt(
    title: &str,
    genre: &str,
    tone: &str,
    pacing: &str,
    description: &str,
    themes: &str,
    dramatic_goal: &str,
    conflict_type: &str,
    external_pressure: &str,
    setting_location: &str,
    setting_time: &str,
    setting_atmosphere: &str,
    characters_present: &str,
    scene_outline: &str,
    strategy_notes: &str,
    narrative_quartet: &str,
    run_mode: &str,
    conflict_level: i32,
    pace: &str,
    ai_freedom: &str,
    user_premise: &str,
    word_count: u32,
    genre_tips: &str,
    pool: Option<&DbPool>,
) -> String {
    let vars: &[(&str, &str)] = &[
        ("story_title", title),
        ("genre", genre),
        ("tone", tone),
        ("pacing", pacing),
        ("description", description),
        ("themes", themes),
        ("dramatic_goal", dramatic_goal),
        ("conflict_type", conflict_type),
        ("external_pressure", external_pressure),
        ("setting_location", setting_location),
        ("setting_time", setting_time),
        ("setting_atmosphere", setting_atmosphere),
        ("characters_present", characters_present),
        ("scene_outline", scene_outline),
        ("strategy_notes", strategy_notes),
        ("narrative_quartet", narrative_quartet),
        ("run_mode", run_mode),
        ("conflict_level", &conflict_level.to_string()),
        ("pace", pace),
        ("ai_freedom", ai_freedom),
        ("user_premise", user_premise),
        ("word_count", &word_count.to_string()),
        ("genre_tips", genre_tips),
    ];
    resolve_and_render(
        "narrative_first_scene_generate",
        "你是一名专业的小说作家。请根据场景戏剧结构撰写正文，目标{{word_count}}字。",
        vars,
        pool,
    )
}

// ==================== 提示词框架目录 (v0.23.61) ====================

/// 生成紧凑的提示词框架目录 JSON，供 Call 1 最快模型选择创作框架。
pub fn build_prompt_framework_catalog() -> String {
    serde_json::json!({
        "methodologies": [
            {"id": "snowflake", "name": "雪花法", "steps": 10, "适合": "规划型作者"},
            {"id": "hero_journey", "name": "英雄之旅", "stages": 12, "适合": "史诗/奇幻/冒险"},
            {"id": "scene_structure", "name": "场景结构法", "适合": "电影化写作"},
            {"id": "character_depth", "name": "角色深度模型", "适合": "角色驱动型"},
            {"id": "hdwb", "name": "高密度世界构建", "phases": 4, "适合": "复杂世界观"}
        ],
        "quality_gates": [
            {"id": "pipeline_review", "用途": "深度审稿(5维评分)"},
            {"id": "audit_quality_inspector", "用途": "11维审计(后台静默)"},
            {"id": "mini_review_system", "用途": "轻量合同检查(默认)"}
        ],
        "contextual_injectors": [
            {"id": "writer_contract_constraints", "触发": "故事合同已设置时"},
            {"id": "writer_chase_debt", "触发": "有未回收伏笔时"},
            {"id": "writer_narrative_event_history", "触发": "已有前文内容时"}
        ]
    })
    .to_string()
}
