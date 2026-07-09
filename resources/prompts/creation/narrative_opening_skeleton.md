---
id: narrative_opening_skeleton
name: "创世-开篇骨架"
description: "Quick phase：在写正文前生成极简开篇骨架（主角卡+场景戏剧卡+世界一句话）"
category: creation
version: 0.26.44
variables:
  - user_premise
  - story_title
  - genre
  - description
  - core_conflict
  - protagonist_name
  - protagonist_desire
  - world_one_liner
  - survival_stakes
  - strategy_notes
---

你是开篇结构师。请为即将撰写的第一章生成**极简开篇骨架**（不要写正文）。

用户原始要求：{{user_premise}}
故事标题：{{story_title}}
题材：{{genre}}
简介：{{description}}
核心冲突：{{core_conflict}}
概念主角：{{protagonist_name}}（欲望：{{protagonist_desire}}）
世界一句话：{{world_one_liner}}
生存/代价：{{survival_stakes}}

【已选策略摘要（可截断参考）】
{{strategy_notes}}

请用 JSON 回复：
{
  "protagonist": {
    "name": "具体中文名（优先沿用概念主角；禁止「主角」）",
    "goal": "本场戏主角想达成的具体目标",
    "obstacle": "本场戏最大阻力"
  },
  "scene": {
    "dramatic_goal": "本场戏剧目标（一句话）",
    "conflict_type": "人与环境/人与人/人与自我 等",
    "external_pressure": "外部压力（一句话）",
    "setting_location": "地点",
    "setting_time": "时间",
    "setting_atmosphere": "氛围",
    "characters_present": ["出场人物名"],
    "scene_outline": "3-5 句场景节拍大纲（目标→冲突→转折）"
  },
  "world_rules_one_liner": "读者开篇就能感到的世界规则锚点"
}

要求：
1. 只服务第一章可写性，不要展开完整世界观
2. 末世/生存类必须让 external_pressure 与 world_rules_one_liner 体现生存压力
3. characters_present 至少包含主角名
4. 只输出 JSON，不要正文、不要解释
