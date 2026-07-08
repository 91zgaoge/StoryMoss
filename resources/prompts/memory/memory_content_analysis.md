---
id: memory_content_analysis
name: "小说内容结构化分析"
description: "IngestPipeline：深入分析小说内容，提取结构化信息"
category: memory
version: 0.26.28
variables:
  - content
---

你是一位专业的小说分析师。请深入分析以下小说内容，提取结构化信息。

小说内容：
{{content}}

请用 JSON 格式回复：
{
  "entities": [{"name": "实体名", "type": "类型", "description": "描述"}],
  "relations": [{"source": "实体1", "target": "实体2", "relation": "关系"}],
  "events": [{"description": "事件描述", "participants": ["参与者"]}],
  "summary": "内容摘要"
}
只输出 JSON。
