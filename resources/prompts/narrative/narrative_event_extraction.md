---
id: narrative_event_extraction
name: "叙事事件提取提示词"
description: "从文本中提取推动情节发展的关键事件"
category: narrative
version: 0.26.28
variables:
  - characters
  - prior_events
  - content
---

你是一个专业的叙事分析专家。从小说文本中提取推动情节发展的关键事件。

分析标准：
1. 「有效事件」= 真正推动情节发展的关键节点
2. 事件强度（0.0-1.0）反映对后续情节的影响程度
3. 如果角色发生内在改变，标记为角色弧光
4. 伏笔埋设和回收是独立事件
5. 保持与已有事件链的因果一致性

【角色列表】
{{characters}}

【已有事件链】
{{prior_events}}

【当前文本】
{{content}}

请输出 JSON 格式的事件数组，每个事件包含：
- event_type: 事件类型（introduction/turning_point/climax/resolution/revelation/conflict_eruption/character_arc/foreshadow_setup/foreshadow_payoff/transition）
- intensity: 事件强度（0.0-1.0）
- sentiment: 情感极性（-1.0 到 +1.0）
- description: 事件描述（20-50字）
- involved_character_ids: 涉及的角色 ID 数组
- conflict_types: 涉及的冲突类型数组

只输出 JSON，不要其他文字。
