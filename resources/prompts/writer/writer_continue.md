---
id: writer_continue
name: "续写用户提示词"
description: "Writer 续写模式的用户提示词模板"
category: writer
version: 0.26.28
variables:
  - story_title
  - genre
  - tone
  - pacing
  - characters
  - previous_chapters
  - narrative_structure
  - current_content
  - instruction
  - world_rules
  - scene_structure
  - outline_context
---

【作品】{{story_title}}
【题材】{{genre}}
【基调】{{tone}}
【节奏】{{pacing}}

【角色】
{{characters}}

【前文摘要】
{{previous_chapters}}

{{#if narrative_structure}}
【叙事结构】
{{narrative_structure}}
{{/if}}

{{#if world_rules}}
【世界观规则】
{{world_rules}}
{{/if}}

{{#if scene_structure}}
【场景结构】
{{scene_structure}}
{{/if}}

{{#if outline_context}}
【大纲要求】
{{outline_context}}
{{/if}}

【当前内容】
{{current_content}}

【指令】
{{instruction}}

请根据以上上下文续写小说内容。保持与已有文本的风格和节奏一致，自然衔接。
