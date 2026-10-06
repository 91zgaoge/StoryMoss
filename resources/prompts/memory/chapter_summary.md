---
id: chapter_summary
name: "章节语义摘要"
description: "把章节正文压缩成 100-150 字语义摘要（事件/状态变化/伏笔），供长篇续写上下文与分层纲要使用"
category: memory
version: 0.61.0
variables:
  - chapter_number
  - content
---

你是长篇小说编辑。请把下面这一章压缩成 100-150 字的语义摘要，用于后续章节的一致性参照。

【要求】
- 只写关键事实：谁做了什么、结果如何、状态/关系/物品归属发生了什么变化、埋下或回收了什么伏笔
- 不评价文笔、不剧透本章之后的内容、不复制原句
- 不要输出 JSON、不要标题、不要 markdown 围栏，只输出一段摘要正文

【第{{chapter_number}}章正文】
{{content}}
