---
id: segment_summary
name: "段摘要（每 10 章）"
description: "把一段章节的逐章摘要压缩成 200-300 字段摘要，供长篇分层记忆金字塔使用"
category: memory
version: 0.61.0
variables:
  - start_chapter
  - end_chapter
  - chapter_summaries
---

你是长篇小说编辑。下面是第{{start_chapter}}-{{end_chapter}}章的逐章摘要。请把它们压缩成 200-300 字的**段摘要**，用于后续章节的一致性参照。

【要求】
- 保留：主线推进、人物状态与关系的关键变化、物品归属、尚未回收的伏笔与承诺、重要的世界观揭示
- 合并重复信息，不要逐章罗列流水账，不要评价文笔，不要剧透本段之后的内容
- 不要输出 JSON、不要标题、不要 markdown 围栏，只输出一段正文

【逐章摘要】
{{chapter_summaries}}
