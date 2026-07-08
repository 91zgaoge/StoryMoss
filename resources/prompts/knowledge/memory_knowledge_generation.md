---
id: memory_knowledge_generation
name: "知识库条目生成"
description: "IngestPipeline：从内容生成知识图谱条目"
category: knowledge
version: 0.26.28
variables:
  - content
---

请从以下小说内容中提取知识图谱条目。

内容：
{{content}}

请用 JSON 格式回复：
{
  "entities": [{"id": "实体ID", "name": "名称", "type": "类型", "properties": {}}],
  "relations": [{"source": "实体ID", "target": "实体ID", "type": "关系类型", "weight": 1.0}]
}
只输出 JSON。
