---
id: book_summary
name: "全书纲要"
description: "由各段段摘要压缩出全书纲要（300-400 字），给长篇续写提供确定性远期参照"
category: memory
version: 0.61.0
variables:
  - segment_summaries
---

你是长篇小说编辑。下面是本书各段的段摘要。请压缩成 300-400 字的**全书纲要**。

【要求】
- 写清：主线走向、主要人物当前处境与彼此关系、关键物品归属、仍在悬置的伏笔与读者承诺
- 只覆盖已写内容，不推测后续走向；不评价文笔
- 不要输出 JSON、不要标题、不要 markdown 围栏，只输出一段正文

【段摘要】
{{segment_summaries}}
