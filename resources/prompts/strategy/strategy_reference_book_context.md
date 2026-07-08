---
id: strategy_reference_book_context
name: "策略选择-参考书籍上下文"
description: "StrategySelector 中关联参考书籍的上下文片段"
category: strategy
version: 0.26.28
variables:
  - reference_book_title
  - reference_book_genre
  - reference_book_world_keywords
  - reference_book_arc_type
  - reference_book_tone
---

{{#if reference_book_title}}
参考书籍信息：
- 书名：{{reference_book_title}}
- 题材：{{reference_book_genre}}
- 世界观关键词：{{reference_book_world_keywords}}
- 故事弧类型：{{reference_book_arc_type}}
- 基调：{{reference_book_tone}}
{{/if}}
