---
id: cascade_conflict_scan
name: "改稿级联冲突扫描"
description: "改稿级联（P0-T4）：上游章正文被修改后，对照下游章节摘要/节选，找出直接矛盾并按 JSON 输出"
category: audit
version: 0.60.0
variables:
  - source_chapter
  - source_excerpt
  - downstream
---

你是长篇小说的连续性审校。上游第{{source_chapter}}章正文刚被作者修改，下面是修改后的正文节选，以及若干下游章节的摘要（或节选）。请找出下游内容与修改后正文之间**有直接证据的**矛盾。

【修改后的第{{source_chapter}}章节选】
{{source_excerpt}}

【下游章节】
{{downstream}}

请重点核对：
- 事实矛盾：下游陈述与修改后正文互斥（如某人已死/未死、某事件发生时间）
- 物品归属：物品在谁手里、是否已遗失/损毁
- 角色知情：下游角色表现出知道了修改后正文里他还不知道的事（或反之）
- 人物状态：位置、身份、关系、伤病等
- 时间线：先后顺序被打乱

只报告能在文本中找到直接证据的矛盾；不要臆测、不要提改进文笔类意见。若没有矛盾，返回空数组。

仅输出一个合法 JSON 对象（不要 markdown 代码围栏、不要注释、不要尾随逗号）：
{
  "conflicts": [
    {
      "target_chapter": 7,
      "severity": "warning",
      "description": "一句话说明矛盾",
      "source_evidence": "修改后正文中的相关句子（逐字引用）",
      "target_evidence": "下游章节中的相关句子（逐字引用）",
      "suggestion": "最小改法建议"
    }
  ]
}

severity 取值：critical（硬伤，读者必然发现）/ warning（大概率矛盾）/ info（可疑但需人工确认）。
