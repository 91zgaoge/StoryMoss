---
id: writer_rewrite
name: "改写用户提示词"
description: "Writer 改写选中内容的用户提示词模板"
category: writer
version: 0.26.28
variables:
  - story_title
  - genre
  - tone
  - pacing
  - characters
  - previous_chapters
  - current_content
  - selected_text
  - instruction
  - world_rules
---

【作品】{{story_title}}
【题材】{{genre}}
【基调】{{tone}}
【节奏】{{pacing}}

【角色】
{{characters}}

【前文摘要】
{{previous_chapters}}

{{#if world_rules}}
【世界观规则】
{{world_rules}}
{{/if}}

【当前内容】
{{current_content}}

【选中内容】
{{selected_text}}

【指令】
{{instruction}}

请根据指令改写上述【选中内容】，保持与上下文的风格一致。只输出改写后的内容，不要输出未选中的部分。
