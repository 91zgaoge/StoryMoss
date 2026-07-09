---
id: narrative_story_concept_generate
name: "创世-故事概念生成"
description: "Bootstrap 第1步：根据用户创意生成故事概念（标题/题材/冲突/主角/世界锚点）"
category: creation
version: 0.26.44
variables:
  - user_input
  - genre_profiles
---

你是一位资深小说编辑。请根据用户的创意，生成一个完整、可写的故事概念。

用户输入："{{user_input}}"

可选题材画像目录（仅供标准化映射，可单选或多选）：
{{genre_profiles}}

请用 JSON 格式回复：
{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字，必须点出核心冲突）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言、末世生存）",
  "genre_profile_ids": ["从上述目录中挑选最匹配的题材画像 id，可单选或多选"],
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）",
  "protagonist_name": "主角姓名（具体中文名，勿用「主角」）",
  "protagonist_desire": "主角此刻最想得到/保住的东西（一句话）",
  "protagonist_wound": "主角的旧伤或软肋（一句话，可空）",
  "core_conflict": "贯穿全书的核心冲突（谁与谁、争什么）",
  "world_one_liner": "世界规则一句话（读者开篇就能感到的设定锚点）",
  "survival_stakes": "若不行动会失去什么（末世/生存类必填；其他题材可写等价代价）"
}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材必须严格遵循用户输入中的要求
4. 题材要具体，不要笼统「小说」
5. 如果用户输入包含复合题材（如「异星球末世生存」），请尽量映射多个 genre_profile_ids
6. 如果目录中没有精确匹配，允许返回空数组，但必须在 genre 字段保留原始题材描述
7. 末世/生存类必须给出非空的 world_one_liner 与 survival_stakes
8. protagonist_name 必须是具体人名，禁止输出「主角」「男主」「女主」
9. 只输出 JSON，不要其他内容
