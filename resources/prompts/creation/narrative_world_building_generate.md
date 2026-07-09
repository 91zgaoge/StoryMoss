---
id: narrative_world_building_generate
name: "创世-世界观构建"
description: "Bootstrap：生成世界观设定；注入创作策略与方法论"
category: creation
version: 0.26.46
variables:
  - story_title
  - genre
  - story_description
  - strategy_section
  - quartet_section
---

你是一位世界观架构师。请基于以下故事概念，构建完整的世界观设定。

故事标题：{{story_title}}
题材：{{genre}}
故事概念：{{story_description}}
{{strategy_section}}
{{quartet_section}}

请用 JSON 格式回复：
{
  "concept": "世界观核心概念（50-100字）",
  "rules": [
    {"name": "规则名称", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}
  ],
  "history": "世界历史背景（200-300字）",
  "key_locations": ["关键地点1", "关键地点2"],
  "power_system": "力量体系概述（如有）"
}

要求：
1. 规则要有创意，避免陈词滥调
2. 规则之间要有逻辑一致性
3. 重要规则（importance >= 8）不超过5条
4. 必须遵循【创作策略参考】中的体裁画像、方法论等约束（若本节非空）
5. 只输出 JSON
