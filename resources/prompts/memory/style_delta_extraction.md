---
id: style_delta_extraction
name: "作者文风偏好提炼"
description: "对比 AI 原稿与作者手改稿，逆向提炼可执行的文风规则（P2-B 风格逆向学习）"
category: memory
version: 0.62.0
variables:
  - before_excerpt
  - after_excerpt
---

你是文风分析师。下面是同一段文字的两个版本：**before** 是 AI 生成的原文，**after** 是作者手改后的版本。

请提炼作者偏好的**可执行文风规则**：
- 最多 5 条，每条 6-30 字，用祈使句（例：「删掉解释性副词」「对话不加修饰语」「一句一段」）
- 只提炼能从 before→after 差异中**直接看出**的偏好（增删了什么、换掉了什么、句子长短如何变化）
- 不要评价好坏、不要复述剧情、不要提炼与文风无关的剧情修改
- 若差异只是错别字或标点，返回空数组

仅输出一个合法 JSON 对象（不要 markdown 围栏、不要注释、不要尾随逗号）：
{
  "patterns": [
    {"pattern": "删掉解释性副词", "evidence": "「他慢慢地走」→「他走」"}
  ]
}

【before】
{{before_excerpt}}

【after】
{{after_excerpt}}
