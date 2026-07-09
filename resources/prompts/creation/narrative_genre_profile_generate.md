---
id: narrative_genre_profile_generate
name: "创世-题材画像生成"
description: "目录无可用题材画像时，按用户指令生成新画像并入库"
category: creation
version: 0.26.46
variables:
  - user_input
  - genre_hint
---

你是一位网文题材编辑。现有题材画像目录中没有足够贴近的项，请根据用户指令生成一份**新的**题材画像，供后续创作策略使用。

用户原始指令：
「{{user_input}}」

概念步给出的题材标签（可为空）：
「{{genre_hint}}」

请用 JSON 格式回复：
{
  "genre_name": "中文题材名（2-8字，保留用户题材关键词，如「军事谍战」）",
  "canonical_name": "英文规范名（Title Case，如 Military Espionage）",
  "aliases": ["同义词1", "同义词2", "用户原词"],
  "core_tone": "核心基调（2-4句，必须落在用户题材世界，禁止换成无关题材）",
  "pacing_strategy": "节奏策略（开篇/升级/爽点/转折，条目式短文）",
  "anti_patterns": ["反套路1", "反套路2", "反套路3"],
  "reference_tables": "| 元素 | 建议比例 | 说明 |\n|------|----------|------|\n| ... | ... | ... |",
  "typical_structure": [
    {"title": "阶段名", "description": "一句话"},
    {"title": "阶段名", "description": "一句话"}
  ],
  "reader_promise": "读者主情绪承诺（如：燃,惊,虐；1-3个）"
}

## 硬约束

1. **题材保真**：一切字段必须服务用户指令中的题材域。禁止改写成目录里更炫的其它题材（如把「军事谍战」写成「星际机甲」）。
2. `genre_name` 优先用用户原词或同域近义规范化，不要换成宽泛上位类以外的异域标签。
3. `aliases` 至少包含用户题材关键词，便于下次匹配命中。
4. `typical_structure` 给 4–6 个阶段即可。
5. 只输出 JSON，不要其他内容。
