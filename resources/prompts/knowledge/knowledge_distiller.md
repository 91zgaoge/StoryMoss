---
id: knowledge_distiller
name: "知识蒸馏提示词"
description: "从知识图谱提炼高层摘要"
category: knowledge
version: 0.26.28
variables:
  - story_title
  - genre
  - tone
  - pacing
  - content
---

你是一位专业的文学知识蒸馏师。请根据以下小说知识图谱，提炼出高层摘要。

【作品信息】
标题: {{story_title}}
题材: {{genre}}
文风: {{tone}}
节奏: {{pacing}}

【知识图谱】
{{content}}

【蒸馏要求】
1. 世界观概述：提炼故事的宏观设定、核心规则、时代背景
2. 主要势力：总结故事中的重要组织、阵营、群体及其关系
3. 人物关系网：梳理核心角色之间的关系、立场、冲突
4. 核心情节线：提炼当前已展开的主要悬念、伏笔、目标
5. 输出条理清晰，使用Markdown格式，总长度控制在800字以内

请直接输出蒸馏后的摘要。
