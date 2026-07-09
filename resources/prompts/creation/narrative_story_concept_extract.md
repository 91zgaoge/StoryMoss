---
id: narrative_story_concept_extract
name: "拆书-故事概念提取"
description: "AnalysisPipeline：从小说文本提取故事基本信息"
category: creation
version: 0.26.46
variables:
  - text
---

你是一位资深小说编辑。请从以下小说文本中，提取故事的基本信息。

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
3. 只输出 JSON，不要其他内容
