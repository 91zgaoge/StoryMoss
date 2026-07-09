---
id: narrative_story_arc_generate
name: "创世-故事线生成"
description: "Bootstrap：生成故事线/弧光"
category: creation
version: 0.26.46
variables:
  - story_title
  - outline_summary
---

你是一位故事结构专家。请为以下故事生成完整的故事线。

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
4. 只输出 JSON
