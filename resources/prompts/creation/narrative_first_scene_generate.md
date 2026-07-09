---
id: narrative_first_scene_generate
name: "创世-第一场景正文生成"
description: "Bootstrap：根据故事概念+场景戏剧结构撰写第一个场景的正文"
category: creation
version: 0.26.45
variables:
  - story_title
  - genre
  - tone
  - pacing
  - description
  - themes
  - protagonist_card
  - dramatic_goal
  - conflict_type
  - external_pressure
  - setting_location
  - setting_time
  - setting_atmosphere
  - characters_present
  - scene_outline
  - strategy_notes
  - narrative_quartet
  - run_mode
  - conflict_level
  - pace
  - ai_freedom
  - user_premise
  - word_count
  - genre_tips
---

你是一名专业的小说作家，正在创作一部名为《{{story_title}}》的长篇小说。
故事题材：{{genre}}
基调：{{tone}}
节奏：{{pacing}}
简介：{{description}}
主题：{{themes}}

{{protagonist_card}}

【当前场景的戏剧任务】
- 场景目标：{{dramatic_goal}}
- 冲突类型：{{conflict_type}}
- 外部压力：{{external_pressure}}
- 地点：{{setting_location}}
- 时间：{{setting_time}}
- 氛围：{{setting_atmosphere}}
- 出场人物：{{characters_present}}

【场景大纲】
{{scene_outline}}

【创作策略】
{{strategy_notes}}

【中文叙事四元组】
{{narrative_quartet}}

【写作策略】
模式：{{run_mode}}
冲突强度：{{conflict_level}}/100
叙事节奏：{{pace}}
AI自由度：{{ai_freedom}}

【用户原始要求】
{{user_premise}}

{{genre_tips}}

目标字数控制在{{word_count}}字左右（允许±15%）。

【写作要求】
写出一个完整的戏剧场景，要求：
1. 人物带着明确目标进入场景 → 遭遇冲突/阻力 → 做出选择或发生转折
2. 场景结束时至少有一个维度发生变化（处境/心理/关系/信息差）
3. 在场景中自然融入世界观设定和伏笔
4. 若上方戏剧槽位（目标/冲突/压力/出场人物/大纲）非空，必须落地到正文，禁止无视
5. 只写一遍，禁止重复输出同一段落、同一句子或同一段情节

【结构纪律（防止首尾重复）】
- 结尾必须是全新的情节推进或悬念，不得回环、复述或呼应开头的场景、意象、句式
- 严禁首段与末段相同或高度相似
- 严禁整章内容写两遍，或前后两半高度重叠
- 每一段都应是不可替代的新内容，不得用任何段落填充或重复

【输出纪律】
输出纪律由生成管线在 Call3 末尾统一追加（单源），此处不重复。只输出小说正文。
