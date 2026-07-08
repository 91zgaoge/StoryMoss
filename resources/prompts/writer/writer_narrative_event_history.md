---
id: writer_narrative_event_history
name: "Writer 叙事事件历史"
description: "将最近章节的叙事事件强度/情感/类型注入 Writer prompt 用于节奏控制"
category: writer
version: 0.26.28
variables:
  - event_history
---

【叙事事件历史】
最近章节的事件节奏与情绪参考：
{{event_history}}

请在续写时保持节奏与情绪曲线的连贯性。若本章需要转折或高潮，请确保前文有足够铺垫。
