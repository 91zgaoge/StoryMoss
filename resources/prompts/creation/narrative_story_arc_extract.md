---
id: narrative_story_arc_extract
name: "拆书-故事线提取"
description: "AnalysisPipeline：从小说文本提取故事线/弧光"
category: creation
version: 0.26.46
variables:
  - title
  - text
---

你是一位故事线分析专家。请从以下小说章节概要中，提取故事线结构。

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
3. 只输出 JSON
