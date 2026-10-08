---
id: writer_rewrite
name: "改写用户提示词"
description: "Writer 改写选中内容的用户提示词模板"
category: writer
version: 0.65.0
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

改写纪律（删优于加）：
- 能删就删、能换就换，加字是最后手段：删掉与场景已暗示重复的句子、删掉「一股说不清的感觉」类包装、删掉副词化填充（由衷地/情不自禁地）；改完不得比原文更长（除非指令明确要求扩写）。
- 不得把平直命名改成身体反应（「她害怕」是正常的人类句式）；不得把「说/道」改成低语/咕哝/嗤笑等花式标签。
- 不得把改写写得更华丽、更抒情、更「文艺」——改写的目标是更准、更干净，不是更浓。
- 保留原文的具体细节（名字、物件、数目）；不要用更笼统的说法替换它们。
