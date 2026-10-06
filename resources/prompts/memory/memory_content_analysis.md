---
id: memory_content_analysis
name: "小说内容结构化分析"
description: "IngestPipeline：深入分析小说内容，提取结构化信息与写作资产（角色画像/关系/世界观/场景大纲/故事增量）"
category: memory
version: 0.37.0
variables:
  - content
---

你是一位专业的小说分析师。请深入分析以下小说内容，提取结构化信息与写作资产，用于沉淀到角色档案、角色关系、世界观设定、场景大纲与故事大纲。

小说内容：
{{content}}

【输出要求】
仅输出一个合法 JSON 对象：不要 markdown 代码块围栏（```），不要注释，不要任何额外说明文字。结构如下（无依据的字段省略即可，不要编造）：
{
  "entities": [
    {
      "name": "实体名称（必须是文本中明确出现的名字，禁止编造）",
      "entity_type": "Character",
      "mentions": ["文本中出现该实体的具体片段，引用原文1-2句"],
      "attributes": {"location": "当前位置", "mood": "当前情绪", "goal": "当前目标"},
      "role_type": "主角/配角/反派/导师/盟友等（仅 Character）",
      "personality": "性格画像（仅 Character，文本有依据时填写）",
      "background": "出身/经历（仅 Character）",
      "goals": "目标动机（仅 Character）",
      "fears": "恐惧/弱点（仅 Character）",
      "appearance": "外貌特征（仅 Character）",
      "gender": "性别（仅 Character，明确时填写）",
      "age": 25,
      "emotional_core": "情感内核：角色的主导情感倾向（仅 Character）",
      "emotional_trigger": "情感触发：什么引发强烈情感反应（仅 Character）",
      "emotional_wound": "情感创伤：驱动行为的过往创伤（仅 Character）",
      "emotional_need": "情感需求：角色内心真正渴望什么（仅 Character）",
      "importance_score": 0.9
    }
  ],
  "relationships": [
    {
      "source": "源角色名称",
      "target": "目标角色名称",
      "relation_type": "关系类型（如: 朋友/敌人/家人/师徒/上下级/爱慕/仇恨/竞争）",
      "evidence": "支持该关系的原文引用",
      "strength": 0.8,
      "description": "关系描述",
      "dynamic": "关系动态/当前状态（如: 面和心不和/渐行渐远）",
      "emotional_bond": "source 对 target 的情感纽带",
      "emotional_intensity": 0.9,
      "reverse_emotional_bond": "target 对 source 的情感纽带",
      "reverse_emotional_intensity": 0.7
    }
  ],
  "events": [
    {
      "description": "事件描述（30字以内）",
      "participants": ["参与者1", "参与者2"],
      "importance": 8,
      "trigger": "触发原因",
      "consequence": "后果影响"
    }
  ],
  "sentiment": {
    "overall": "positive",
    "intensity": 0.7,
    "arc": [{"position": 0.5, "sentiment": "tense", "intensity": 0.8}]
  },
  "foreshadowing": [
    {
      "content": "伏笔内容",
      "type_": "setup",
      "related_to": ["相关内容"]
    }
  ],
  "themes": ["主题1", "主题2"],
  "world_building": {
    "concept": "本段内容揭示的世界观概念增量",
    "rules": [
      {"name": "规则名", "description": "规则描述", "rule_type": "magic/technology/social/physical/biological/historical/cultural/custom 之一", "importance": 7}
    ],
    "history": "历史背景增量",
    "cultures": [
      {"name": "文化名", "description": "描述", "customs": ["习俗1"], "values": ["价值观1"]}
    ]
  },
  "scene_outline": {
    "dramatic_goal": "本场景的戏剧目标",
    "key_events": ["关键事件1", "关键事件2"],
    "conflict_type": "冲突类型",
    "setting_location": "场景地点",
    "setting_time": "场景时间",
    "atmosphere": "氛围",
    "characters_present": ["出场角色名"],
    "emotional_tone": "情感基调"
  },
  "story_delta": {
    "core_conflict": "本段内容揭示或推进的故事核心冲突",
    "turning_points": ["本段出现的情节转折点"]
  },
  "knowledge_updates": [
    {
      "character": "获知信息的角色名（必须是文本中明确出现的角色）",
      "fact": "该角色在本次内容中获知的事实（一句话，可含关键人名/物名）",
      "evidence": "支持该信息流的原文引用"
    }
  ],
  "timeline_events": [
    {
      "objective_fact": "客观发生了什么（世界真相，作者侧视角，不与角色认知混淆）",
      "reader_knowledge": "读者此刻知道或以为的事实（可省略）",
      "reveal_status": "hidden",
      "participants": ["相关角色名"],
      "evidence": "原文依据"
    }
  ],
  "item_holdings": [
    {
      "item": "物品名（必须是文本中明确出现的物品）",
      "holder": "当前持有者角色名（lost/destroy 时可省略）",
      "action": "acquire",
      "evidence": "原文依据"
    }
  ]
}

【字段说明】
- entity_type 必须严格使用: Character(人物)/Location(地点)/Item(物品)/Organization(组织)/Concept(概念)/Event(事件) 之一
- age 为整数（未知则省略）；strength/emotional_intensity/reverse_emotional_intensity/importance_score 为 0.0-1.0 浮点数；importance（事件）为 1-10 整数
- sentiment.overall 可选值: positive/negative/neutral；foreshadowing.type_ 可选值: setup(埋下)/payoff(回收)
- world_building / scene_outline / story_delta 仅在有实质增量时输出，没有则整个字段省略

【Few-shot示例】
输入: "林枫站在青云山顶，望着远处的云海。他握紧手中的长剑，心中暗暗发誓要找到杀害师父的凶手。"
输出: {
  "entities": [
    {"name": "林枫", "entity_type": "Character", "mentions": ["林枫站在青云山顶"],
     "attributes": {"location": "青云山顶", "mood": "悲愤/决心"},
     "role_type": "主角", "personality": "坚毅重情", "goals": "为师父复仇",
     "emotional_core": "压抑的悲愤", "emotional_trigger": "提及师父之死",
     "emotional_wound": "师父被杀", "emotional_need": "为恩师讨回公道",
     "importance_score": 0.95},
    {"name": "青云山", "entity_type": "Location", "mentions": ["林枫站在青云山顶"], "attributes": {}},
    {"name": "长剑", "entity_type": "Item", "mentions": ["握紧手中的长剑"], "attributes": {}}
  ],
  "relationships": [
    {"source": "林枫", "target": "师父", "relation_type": "师徒", "evidence": "要找到杀害师父的凶手",
     "strength": 0.9, "description": "师父已遇害，林枫誓报此仇", "dynamic": "阴阳两隔",
     "emotional_bond": "敬爱与孺慕", "emotional_intensity": 0.9,
     "reverse_emotional_bond": "舐犊之情", "reverse_emotional_intensity": 0.8}
  ],
  "events": [
    {"description": "林枫在青云山顶发誓复仇", "participants": ["林枫"], "importance": 9, "trigger": "师父被杀", "consequence": "林枫决心复仇"}
  ],
  "sentiment": {"overall": "negative", "intensity": 0.8, "arc": [{"position": 0.5, "sentiment": "determined", "intensity": 0.9}]},
  "foreshadowing": [{"content": "要找到杀害师父的凶手", "type_": "setup", "related_to": ["复仇主线"]}],
  "themes": ["复仇", "成长"],
  "scene_outline": {
    "dramatic_goal": "林枫立誓复仇",
    "key_events": ["林枫登青云山顶远眺", "握紧长剑暗中立誓"],
    "conflict_type": "内心冲突",
    "setting_location": "青云山顶",
    "setting_time": "不明",
    "atmosphere": "苍凉肃杀",
    "characters_present": ["林枫"],
    "emotional_tone": "悲愤决绝"
  },
  "story_delta": {"core_conflict": "林枫与杀师凶手之间的血仇", "turning_points": ["林枫正式踏上复仇之路"]}
}

【重要规则】
1. 实体名称必须是文本中明确出现的名字，禁止编造或推断未命名的实体
2. 角色画像字段（role_type/personality/background/goals/fears/appearance/gender/age/4个 emotional_*）仅对 entity_type=Character 填写，且必须有文本依据，无依据则省略
3. 关系必须有明确的原文证据支持，禁止臆测
4. 只输出纯 JSON：不要 markdown 代码块围栏，不要注释，不要尾随逗号
5. 如果文本中没有足够信息，对应字段返回空数组或整个省略，不要编造
6. knowledge_updates 仅在文本明确写出「某角色获知/被告知/发现某信息」时输出；角色原本就知道的事不算
7. timeline_events 仅在出现值得跨章追踪的秘密/真相/揭示时输出：objective_fact 写客观事实（作者侧），reader_knowledge 写读者此刻的认知，两者不得混写；reveal_status 默认 hidden，只有文本明确向读者揭示时才写 partial/revealed
8. item_holdings 只登记跨章影响行动边界的关键物品（信物/武器/钥匙/凭证等），一次性道具不登记；action 取 acquire/transfer/lose/destroy
