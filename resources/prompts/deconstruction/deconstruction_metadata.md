---
id: deconstruction_metadata
name: "拆书-元数据提取"
description: "从小说开头提取基本信息（标题/作者/题材/字数）"
category: deconstruction
version: 0.26.28
variables:
  - text
---

请分析以下小说开头，提取基本信息。只输出 JSON。
{
  "title": "书名",
  "author": "作者（如能识别）",
  "genre": "题材",
  "language": "语言",
  "estimated_length": "预估总字数"
}

小说开头：
{{text}}
