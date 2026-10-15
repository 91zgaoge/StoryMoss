---
id: chapter_commit_digest
name: "章节提交摘要 + 快评（合并调用）"
description: "SceneCommitService::auto_commit 后台一次调用同时产出章节语义摘要与合同快评（v0.73.0：合并原 chapter_summary + mini_review_system 两次调用）"
category: memory
version: 0.73.0
variables:
  - chapter_number
  - content
  - genre
  - core_tone
  - pacing_strategy
  - world_rules
  - chapter_goal
  - must_cover_nodes
  - forbidden_zones
---

你是长篇小说编辑。请阅读下方章节正文，**一次完成两件事**并以 JSON 输出。

【任务一 · 语义摘要】压成 100-150 字，供后续章节的一致性参照：只写关键事实——谁做了什么、结果如何、状态/关系/物品归属发生了什么变化、埋下或回收了什么伏笔；不评价、不剧透、不复制原句。

【任务二 · 合同快评】对照世界观规则、本章目标、必须覆盖节点与禁忌区快速评估：score 0.0-1.0（1.0 为完全满足合同要求）、dimensions 各维度分与一句话点评、review_summary 一两句总评、issues 列出具体问题。

【作品信息】
题材: {{genre}}
基调: {{core_tone}}
节奏策略: {{pacing_strategy}}

【世界观规则】
{{world_rules}}

【本章目标】
{{chapter_goal}}

【必须覆盖节点】
{{must_cover_nodes}}

【禁忌区】
{{forbidden_zones}}

【第{{chapter_number}}章正文】
{{content}}

严格以 JSON 输出，不要 markdown 代码块、不要任何额外说明：
{
  "summary": "100-150 字的语义摘要",
  "score": 0.82,
  "dimensions": [
    {"name": "合同目标达成", "score": 0.9, "comment": "覆盖了主要必须节点"},
    {"name": "世界规则一致", "score": 0.8, "comment": "未发现明显违反"},
    {"name": "叙事连贯性", "score": 0.85, "comment": "情节推进自然"},
    {"name": "基调一致", "score": 0.8, "comment": "符合整体基调"}
  ],
  "review_summary": "总体评价，1-2 句话",
  "issues": ["具体问题1", "具体问题2"]
}
