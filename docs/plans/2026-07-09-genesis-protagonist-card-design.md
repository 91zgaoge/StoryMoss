# Genesis 人物卡强制落地 — 设计文档

> **状态：** 已批准（2026-07-09）· **修订：** 补充「冲突与目标清晰」（同日）  
> **路线：** A — Character Card Enforcement  
> **约束：** 先创世后续写 · 延迟优先（p95 ≤90s）· 零新增 LLM  
> **双目标：** ① 人物辨识度 ② 冲突与目标清晰（开篇就知道要什么、阻力是什么）  
> **画布：** [genesis-protagonist-card-design.canvas.tsx](file:///Users/yuzaimu/.cursor/projects/Users-yuzaimu-projects-StoryMoss/canvases/genesis-protagonist-card-design.canvas.tsx)

## 1. 问题

v0.26.44 已加厚概念字段并插入开篇骨架，但首章 Writer 仍常出现两类空洞：

1. **人空**：泛称「主角」，叫不出名字  
2. **戏空**：开篇读完仍不知道主角**要什么**、**拦着他的是什么**

根因相同：字段/槽位存在，**落地纪律缺失**。不堆新 LLM，专治「有卡却写不出人与戏」。

## 2. 方案摘要

纯函数合并 `OpeningSkeleton` ∪ 加厚 `StoryMetaElement` → `ProtagonistCard`；双重注入（first_scene Critical 位 + TriShot Call3 尾注）；生成后规则探针验 **真名 + 欲望信号 + 阻力信号**；未达门槛且预算允许则一次软重试。探针失败 fail-open。

## 3. Schema

| 字段 | 合并优先级 | 强制 |
|------|------------|------|
| `name` | skeleton.name → meta.protagonist_name | 有有效名才建卡（过滤「主角/男主/女主」） |
| `desire` | skeleton.goal → meta.protagonist_desire → meta.survival_stakes | **有则必注入**（开篇目标轴） |
| `obstacle` | skeleton.obstacle → meta.survival_stakes / meta.core_conflict | **有则必注入**（开篇阻力轴） |
| `scene_goal` | skeleton.scene.dramatic_goal → meta.core_conflict | 有则注入（本场戏剧目标，补强「要什么」） |
| `wound` | meta.protagonist_wound | 可选 |

无有效 `name` → 不注入（不强塞泛称）。  
若有名但 desire 与 obstacle 皆空 → 仍注入姓名卡，并打 `thin_card` 警告日志（概念/骨架过薄，属上游问题）。

## 4. 渲染纪律（固定短段）

```
【开篇人物卡·必须落地】
姓名：{name}
本场欲望/目标：{desire 或 scene_goal}
本场阻力：{obstacle}
旧伤/软肋：{wound}          // 空则省略
纪律：
1. 开场前 3 段内必须出现姓名，禁止用「主角」指代
2. 开场必须让读者明白：主角此刻要什么，以及什么在拦他（用行动/选择体现，禁止空喊口号）
```

## 5. 注入与探针

**注入（同前）：**

- `{{protagonist_card}}` 置于 `narrative_first_scene_generate` 戏剧任务之前  
- Call3：`NOVEL_OUTPUT_DISCIPLINE` 之前追加同一渲染串  

**探针（规则，零 LLM）：**

| 信号 | 算法（保守） | 何时计分 |
|------|--------------|----------|
| `name_hit` | 正文含 `card.name`（归一化空白） | 有卡必测 |
| `generic_label_hit` | 「主角/男主/女主」出现次数 > 0（标题外） | 有卡必测 |
| `desire_hit` | desire/scene_goal 抽 ≥2 字内容词，正文命中 ≥1 | 仅当 desire 或 scene_goal 非空 |
| `obstacle_hit` | obstacle 抽 ≥2 字内容词，正文命中 ≥1 | 仅当 obstacle 非空 |

内容词：去掉「的/了/在/与/和」等停用字后按 2-gram/整词扫描；过短（<2 字）字段跳过该轴探针。

**软重试触发（共享「最多一次额外 Call3」，与 8% 自重复互斥）：**

- `!name_hit`，或  
- （desire/scene_goal 与 obstacle 均非空）且 `!desire_hit && !obstacle_hit`  

重试指令追加：必须出现姓名「{name}」；开场用行动体现「要 {desire}」与「被 {obstacle} 所阻」。

日志：`genesis.protagonist_card.{merged,probe,retry}`（probe 含四信号）。

## 6. 不变量

不新增 quick LLM；不热路径 quality_gate；不动 delivered 状态机与 8% 闸门；本切片不改续写 TimeSliced。

## 7. 验收

| 轴 | 门槛 |
|----|------|
| 真名命中率 | ≥ **80%**（有有效卡） |
| 欲望信号命中率 | ≥ **60%**（有 desire/scene_goal 时） |
| 阻力信号命中率 | ≥ **60%**（有 obstacle 时） |
| 盲测 | 「能否说出主角是谁、要什么、阻力是什么」胜率 ≥70% 或均分 ↑≥15%（N≥5） |
| quick p95 | ≤ **90s**（预期 Δ≈0） |
| 交付率 | 探针失败仍 **100%** fail-open |

## 8. 非目标 / 下一战役

- 非目标：扩骨架 JSON、voice_hint LLM、完整 Character 行进 prompt、续写路径、热路径质量门。  
- 下一战役：日常续写资产利用率（TimeSliced 压缩注入等）。

## 9. 批准记录

- 路径优先级：先创世后续写  
- 代价：延迟优先（选项 1）  
- 主目标：人物辨识度（选项 4）  
- **补充目标：冲突与目标清晰（选项 1 维度）** — 2026-07-09 用户追加  
- 路线：A  
- §1–§3：用户 OK；本修订为同路线增量，不改延迟/零 LLM 约束  
