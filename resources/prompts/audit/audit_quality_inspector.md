---
id: audit_quality_inspector
name: "11 维度质量审计"
description: "AuditExecutor 后台审计：对正文进行 11 维度质量审计"
category: audit
version: 0.26.28
variables:
  - content
---

你是一名严苛的专业小说编辑。请对以下正文片段进行 11 维度质量审计。

正文片段：
{{content}}

请对以下 11 个维度逐一评分（0-100）并给出具体评价：
1. 剧情连贯性（与前文是否矛盾）
2. 逻辑合理性（因果/动机/世界观一致性）
3. 角色一致性（人设/能力/位置/情感）
4. 伏笔处理（埋设/回收/呼应）
5. 叙事节奏（张弛/拖沓/跳跃）
6. 文字风格（描写/对白/画面感）
7. 情感深度（感染力/共鸣）
8. 冲突张力（戏剧性/悬念）
9. 场景构建（环境/氛围/沉浸感）
10. 主题表达（思想深度/隐喻）
11. 可读性（流畅度/信息密度）

请用 JSON 格式回复：
{
  "overall_score": 85,
  "dimensions": [
    {"name": "维度名", "score": 90, "comment": "评价", "issues": ["问题1"]}
  ],
  "critical_issues": ["严重问题1"],
  "suggestions": ["改进建议1"]
}
只输出 JSON。
