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
    // 生产代码中仅 Extract 有构造点（analysis 拆书管线）；Generate 仍被本文件
    // 多个 prompt 函数的 match 臂及其保真测试引用，删除将牵动存活代码，故保留。
    #[allow(dead_code)]
    Generate, // 正向：从零生成
    Extract, // 逆向：从文本提取
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
                r#"你是一位资深小说编辑。请根据用户的创意，生成一个完整、可写的故事概念。

用户输入（最高优先级，题材与世界域以此为准）：
「{{user_input}}」

题材画像目录（**仅用于填写 genre_profile_ids 的 id 映射**；不是选题菜单，禁止用目录里的其它题材替换用户题材）：
{{genre_profiles}}

请用 JSON 格式回复：
{
  "title": "故事标题（在用户题材域内取名；可有吸引力，但不得换题材）",
  "description": "一句话简介（30-50字，必须点出核心冲突，且冲突发生在用户题材世界内）",
  "genre": "题材标签（必须保留用户输入中的题材关键词；可略作规范化，禁止换成目录中的其它题材名）",
  "genre_profile_ids": ["仅从上述目录选最贴近的 id；无贴近项则 []"],
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）",
  "protagonist_name": "主角姓名（具体中文名，勿用「主角」）",
  "protagonist_desire": "主角此刻最想得到/保住的东西（一句话）",
  "protagonist_wound": "主角的旧伤或软肋（一句话，可空）",
  "core_conflict": "贯穿全书的核心冲突（谁与谁、争什么；必须落在用户题材世界）",
  "world_one_liner": "世界规则一句话（必须属于用户题材世界，禁止换成无关世界观）",
  "survival_stakes": "若不行动会失去什么（末世/生存类必填；其他题材写等价代价）"
}

## 硬约束（违反任一条即视为失败输出）

1. **题材保真**：用户输入已给出题材/类型时，`genre`、`description`、`core_conflict`、`world_one_liner` 必须留在同一题材域。禁止为了「更好看/更燃/更具体」改换题材域。
2. **反例（禁止）**：用户要「军事谍战」→ 禁止改成「星际机甲 / 科幻 / 宇宙失忆间谍」；用户要「都市奇幻」→ 禁止改成「修仙 / 末世」；用户要「古言」→ 禁止改成「现代都市」。
3. **目录不是菜单**：`genre_profiles` 只用于选 `genre_profile_ids`。即使目录里有更炫的标签（如「星际机甲」），只要用户没写，就不得写入 `genre` 或世界设定。
4. **无精确画像时**：`genre_profile_ids` 填 `[]` 即可；后续步骤会匹配现有目录或按指令生成新画像入库。`genre` 仍须用用户原词或同域近义（如「军事谍战」可写「军事谍战」或「军事/谍战」，不可写「星际机甲」）。
5. **「具体」的含义**：在用户题材内部把冲突、人物、场景写清楚；不是换成另一个更具体的题材标签。
6. **标题**：可在用户题材域内起名；禁止用标题暗示另一题材世界。
7. 复合题材（如「异星球末世生存」）可映射多个 `genre_profile_ids`，但仍不得引入用户未提及的第三域。
8. 末世/生存类必须给出非空的 `world_one_liner` 与 `survival_stakes`。
9. `protagonist_name` 必须是具体人名，禁止「主角」「男主」「女主」。
10. 只输出 JSON，不要其他内容。"#,
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
  "author": "作者姓名（文本中可识别则填写，否则为null）",
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
                    ("scenes", context),
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

#[cfg(test)]
mod concept_prompt_fidelity_tests {
    use chrono::Local;

    use super::*;
    use crate::db::GenreProfile;

    fn sample_profiles() -> Vec<GenreProfile> {
        let now = Local::now();
        vec![
            GenreProfile {
                id: "military-id".into(),
                genre_name: "军事".into(),
                canonical_name: "Military".into(),
                aliases_json: Some(r#"["military"]"#.into()),
                core_tone: None,
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: now,
            },
            GenreProfile {
                id: "mecha-id".into(),
                genre_name: "星际机甲".into(),
                canonical_name: "Mecha / Stellar Warfare".into(),
                aliases_json: Some(r#"["mecha"]"#.into()),
                core_tone: None,
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: now,
            },
        ]
    }

    #[test]
    fn concept_generate_prompt_locks_genre_fidelity_against_menu_drift() {
        let prompt = story_concept_prompt(
            PromptMode::Generate,
            "写一部军事谍战的长篇小说",
            Some(&sample_profiles()),
            None,
        );

        assert!(prompt.contains("军事谍战"));
        assert!(
            prompt.contains("题材保真") || prompt.contains("硬约束"),
            "must state hard fidelity constraints"
        );
        assert!(
            prompt.contains("星际机甲"),
            "must include explicit anti-example naming 星际机甲"
        );
        assert!(
            prompt.contains("不是选题菜单") || prompt.contains("仅用于填写 genre_profile_ids"),
            "must demote genre_profiles from menu to id-map"
        );
        assert!(
            prompt.contains("不是换成另一个更具体的题材标签"),
            "must redefine「具体」to in-domain specificity"
        );
        assert!(
            !prompt.contains("标题要有吸引力，避免俗套"),
            "old title-rewrite incentive must be removed"
        );
        assert!(
            !prompt.contains("可选题材画像目录"),
            "must not call profiles a selectable menu"
        );
        assert!(
            prompt.contains("匹配现有目录或按指令生成新画像"),
            "must describe match-or-create follow-up"
        );
    }

    #[test]
    fn background_generate_templates_declare_strategy_section() {
        // narrative_outline_generate 已随 P4 清理（T6 遗留孤儿种子模板，唯一
        // Rust 消费方 outline_prompt 已删），不在校验之列
        let ids = [
            "narrative_world_building_generate",
            "narrative_character_generate",
            "narrative_scene_generate",
            "narrative_foreshadowing_generate",
        ];
        for id in ids {
            let body = crate::prompts::registry::resolve_prompt_default(id)
                .unwrap_or_else(|| panic!("missing builtin {id}"));
            assert!(
                body.contains("{{strategy_section}}"),
                "{id} must include {{{{strategy_section}}}}"
            );
            assert!(
                body.contains("{{quartet_section}}"),
                "{id} must include {{{{quartet_section}}}}"
            );
        }
    }

    #[test]
    fn world_building_prompt_includes_strategy_when_provided() {
        let prompt = world_building_prompt(
            PromptMode::Generate,
            "荒星",
            "末世",
            "求生",
            Some("应遵循的方法论：hero_journey\n英雄之旅十二阶段"),
            Some(r#"{"run_mode":"文戏"}"#),
            None,
        );
        assert!(
            prompt.contains("创作策略参考") || prompt.contains("应遵循的方法论"),
            "strategy must appear in rendered world prompt"
        );
        assert!(prompt.contains("hero_journey") || prompt.contains("英雄"));
    }
}
