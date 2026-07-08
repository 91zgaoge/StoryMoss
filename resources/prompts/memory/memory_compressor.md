---
id: memory_compressor
name: "记忆压缩提示词"
description: "将小说内容压缩为高层摘要"
category: memory
version: 0.26.28
variables:
  - story_title
  - genre
  - tone
  - pacing
  - content
  - ratio
---

你是一位专业的文学记忆压缩师。请将以下小说相关内容压缩为简洁的高层摘要。

【作品信息】
标题: {{story_title}}
题材: {{genre}}
文风: {{tone}}
节奏: {{pacing}}

【待压缩内容】
{{content}}

【压缩要求】
1. 保留核心情节、人物关系、关键伏笔
2. 删除细节描写、重复叙述、过渡段落
3. 输出长度控制在原文的 {{ratio}}%
4. 使用第三人称客观叙述

请直接输出压缩后的摘要，不要添加解释。
