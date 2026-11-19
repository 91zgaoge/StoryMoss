# 续写训练长程记忆缺失分析报告

- 日期：2026-11-19
- 代码基线：v0.99.1（报告中的 P0+P1 已落地：StoryBrief/前情提要全量注入、批次账本、伪大纲移除、预算保护、报告记忆行）
- 实机数据：本机 `~/Library/Application Support/com.storymoss.app/cinema_ai.db`，最近两份《阴脉先生》10 章训练报告（2026-11-17 17:53、2026-11-18 12:55）
- 结论一句话：**用户判断成立。当前管线把“上一章结尾原文”当成了唯一的硬承接物，而前 10 章真正需要的“逐章故事簿 + 未结线索 + 人物当前态 + 设定白名单 + 本批滚动账本”从未作为一个整体进入 writer/planner 的提示词。仅补一份“故事大纲”也不够，必须补的是一层可核验的长程记忆。**

---

## 0. 结论（BLUF）

1. **“上一章结尾 1400 字”只解决“从哪句话之后落笔”，不解决“这个故事已经走到哪、哪些线还开着、哪些设定不能碰”。**
   实机报告里编审自己的总结就是：“AI 在文笔模仿上有一定天赋，但**缺乏长程记忆与逻辑连贯性**，导致剧情严重幻觉与原地踏步。”

2. **资产重建（v0.94.0）确实生成过“故事大纲”和 10 条逐章梗概，但它们在管线里被三层截断/丢弃：**
   - 故事大纲渲染上限 1200 字，且只保留“核心冲突 + 3 个转折点 + 2 个其他块”；转折点没有章号、没有因果顺序。
   - 10 条逐章梗概写进了 `scenes.outline_content`，但 writer 的“进度块”只读**最近 3 章正文的最后 220 字**，不读梗概；planner 也只读最近 3 条梗概。
   - 一旦第 11 章装配完成并写入 `scene_outcomes`，“进度块”会切换为**只显示最近 3 条 outcome**，前 10 章上下文从提示词里彻底消失。

3. **比“没有大纲”更糟的是“错误的大纲”。** 11-18 那次训练资产重建失败（故事大纲—/角色 0/梗概 0），随后通用 `ensure_story_outline` 用“整本书正文的头 600 字 + 尾 1800 字”生成了一份 1794 字的文本，其中 1554 字是**第 134 章单章回顾**，后面接着“下一拍规划”。渲染时 `strip_outline_planning` 在 1572 字处切断，再按 1200 字上限截断——writer 看到的“故事大纲（本场景必须围绕此大纲展开，禁止偏离）”实际上是**第 134 章前 1200 字的单章回顾**。它会把模型牢牢钉在旧现场，而不是拉向 10 章主线。

4. **两次实机结果构成对照：**
   - 资产重建失败（11-18）：总分 0.40，承接 3.1，情节 3.3；编审指出 14–17、20 章整段复制，自创“引魂蛆/骨刺/阴脉起手式”。
   - 资产重建成功（11-17）：总分 0.24，承接 1.2，情节 2.3；10 条逐章梗概都在库里，writer 依然没看到。
   - **两种状态都漂移，说明问题不是“资产重建成功与否”单点，而是“长程记忆没有进入提示词”这个结构性缺口。**

5. **最小可行修复（P0）不是再写一份更长的摘要，而是：**
   - 把**全部前文章节梗概**（不是最近 3 章）作为固定块注入 writer 和 planner；
   - 把**未结线索 / 人物当前态 / 设定白名单**从重建结果中结构化保留；
   - 给本批已生成章节加一个**滚动账本**，第 n 章必须看到第 11..n-1 章“发生了什么、推进了哪条线”；
   - 禁止 `ensure_story_outline` 在训练沙盒里用“头 600 + 尾 1800”生成规划文档来冒充故事大纲。
   - 以上四项都做，才是用户说的“故事大纲 + 上一章结尾一起给 LLM”的完整形态。

---

## 1. 问题定义：三种不同的“漂移”

| 症状 | 实机表现 | 真正缺的信息 |
|---|---|---|
| **局部承接失败** | 第 11 章没接住上一章最后一句/最后一个动作 | 上一章结尾原文（已有 1400 字锚点，基本解决） |
| **长程记忆失败** | 第 142 章埋的“赵开来赠送福摩萨夜总会道场、去见邵卫江”，第 11 章完全不提；模型留在医院自造“闭口燕”危机 | 前 10 章逐章事件 + 未结线索 + 人物当前态 |
| **设定/人物越界** | 自创“引魂蛆/骨刺/阴脉起手式”；把第一人称“我”叫成“杨先生”；凭空出现“陈先生/何小满” | 设定白名单 + 权威人物名册 + 已发生事实的边界 |

这三类失败对应三种不同的记忆物，不能用一个“故事大纲”笼统替代。

---

## 2. 现状：writer / planner 实际能看到什么

### 2.1 首章（沙盒第 11 章）实际可见

| 信息 | 当前来源 | 规模 | 进入 writer？ | 进入 planner？ |
|---|---|---|---|---|
| 原书前 10 章正文 | 灌入 sandbox `scenes` | 每章 2.2k–2.5k 字 | 否（只灌库） | 否 |
| 上一章结尾 | `build_continuity_anchor` | 900/1400 字 | 是，放在指令最后 | **通常被 3200 字 directive 上限截掉** |
| 文风锚点 | `build_style_anchor`（仅在 DNA 失败或上轮文笔低分时） | 2 章 × 600 字 | 是 | 可能被截断 |
| 故事大纲 | `StoryOutlineRepository` → `condense_story_outline_stable` | ≤1200 字 | 是（稳定区） | 是（≤1200 字） |
| 逐章梗概 | `scenes.outline_content`（v0.99.0 起有确定性兜底） | 10 条 × 72–83 字 | **否** | 仅最近 3 条（每条 200 字） |
| 进度 | `progress_lines_from_parts` | 最近 3 章正文末 220 字，或最近 3 条 outcome | 是（但只 3 章） | 否 |
| 人物卡 | 资产重建 8 人（成功时） | 每人若干字段 | 仅被 beat card 准入者 | 仅章节变量 |
| 设定白名单 | 不存在 | — | — | — |
| 未结线索 | 不存在（只有 next_node） | — | — | — |

### 2.2 第 12–20 章实际可见

- writer 的“上一章”由 `PriorChapterContext` 提供（v0.98.7），只包含**上一章正文 + 上一章大纲**；
- “进度块”在 `scene_outcomes` 出现后切换为**最近 3 条 outcome**，前 10 章上下文不再出现；
- `generate_chapter_outline` 的 `progress` 只取最近 3 个 scene 的 `outline_content`（200 字/条）；
- `compile_next_node` 只用最近 3 条 scene outline 判断“下一拍”；
- 训练指令从第 12 章起明确要求“承接沙盒上一章生成的正文，不重复从这段原文起笔”，等于主动切断与原文锚点的关系；
- 结果是：**第 12 章之后，模型对“前 10 章”的记忆几乎归零，只剩最近 1–3 章的尾巴。**

### 2.3 关键代码路径

| 环节 | 位置 | 关键事实 |
|---|---|---|
| 训练指令组装 | `src-tauri/src/task_system/continuation_training.rs:522-576` | 硬承接锚点 900/1400 字放在最后；style anchor 在其前 |
| 硬承接锚点 | `src-tauri/src/task_system/continuation_training.rs:227-241` | 只取最后一个 context 章的末尾 |
| 训练红线 | `src-tauri/src/task_system/continuation_training.rs:62-67` | 要求“只沿用前文真实原文出现过的设定”，但 writer 看不到前文，红线不可执行 |
| 资产重建 prompt | `src-tauri/src/agency/training_assets.rs:160-194` | 每章采样上限 2600 字；要求输出 `chapter_outlines` |
| 逐章梗概落库 | `src-tauri/src/agency/training_assets.rs:753-789` | 写 `scenes.outline_content`；v0.99.0 加首尾窗口兜底 |
| 通用大纲兜底 | `src-tauri/src/agency/coordinator.rs:4956-5105` | `slice_prior_prose` = 头 600 + 尾 1800；用场景方法论 prompt 生成“规划下一拍” |
| 故事大纲渲染 | `src-tauri/src/agency/continue_assets.rs:388-452` | 只保留 core + 3 转折 + 2 其他；总上限 1200 字 |
| 进度块 | `src-tauri/src/agency/coordinator.rs:10132-10186` | outcomes 优先；否则最近 3 章正文末 220 字 |
| 章纲规划 | `src-tauri/src/agency/coordinator.rs:5267-5470` | directive 只取前 3200 字；progress 只取最近 3 条 200 字梗概 |
| directive 上限 | `src-tauri/src/agency/coordinator.rs:10449` | `OUTLINE_DIRECTIVE_CHAR_CAP = 3200` |
| 批次账本 | 无 | `PriorChapterContext` 只带上一章 |
| 资产预算 | `src-tauri/src/agency/continue_assets.rs:16, 763-791` | 总资产块 6000 字；动态块从尾部先被压缩/清空 |
| 下一节点 | `src-tauri/src/agency/beat_card.rs:725-798` | 用最近 3 条 scene outline 从全局大纲里挑一句 |
| 结构化 outcome | `src-tauri/src/agency/narrative_contract/outcome.rs:26-46` | `from_contract` 是“计划”，不是“实际结果” |

---

## 3. 实机证据

### 3.1 11-18 12:12 训练（资产重建失败，最能说明“污染”）

任务日志（`task_logs`）：

```text
12:12:38 已取 20 章：上下文 10 章（原书第 134–143 章）+ 真实后续基准 10 章（第 144–153 章）
12:12:51 进度 35% · 正在重建世界观/大纲/角色/故事线…
12:16:31 资产重建完成：世界观✓｜故事大纲—｜角色0｜关系0｜章节梗概0
12:16:31 模型未返回故事大纲
12:16:31 模型未返回逐章梗概
12:16:31 资产重建未登记任何角色
12:16:31 已注入上一章结尾原文作为硬承接锚点（1400 字，上一轮承接分偏低=true）
12:53:19 进度 72% · 正在取真实后续章并逐章对比…
```

数据库里对应沙盒 `bd8c0cf3…`：

- `scenes` 1–10 的 `outline_content` 全部为空；
- `story_outlines` 有一行 1794 字，创建于 12:17:53（资产重建之后、续写开始之后），内容是：
  - “## 一、已有正文归纳（第134章）”——第 134 章饭局/杨晓雯/出警/医院/小护士的回顾；
  - “## 二、下一拍规划（按方法论）”——从“### 场景一：走廊截人”开始。
- `strip_outline_planning` 命中“### 场景一”，在 1572 字处切断；`condense_story_outline_impl` 再按 1200 字截断。
- 因此 writer 的稳定区“【故事大纲（本场景必须围绕此大纲展开，禁止偏离）】” = **第 134 章前 1200 字回顾**，不是 134–143 的 10 章弧。

这次报告结果：总分 **0.40**，承接 **3.1**，情节 **3.3**；编审总结：

> “AI 陷入严重的‘剧情幻觉’与‘原地踏步’循环，第14-17章及第20章出现整段复制上一章结尾的严重错误，完全丧失推进能力……世界观自相矛盾，AI 强行植入‘引魂蛆/骨刺/阴脉起手式’等生硬设定。”

### 3.2 11-17 17:53 训练（资产重建成功，最能说明“有数据但没进 prompt”）

任务日志：

```text
资产重建完成：世界观✓｜故事大纲✓｜角色8｜关系5｜章节梗概10
```

数据库：

- `story_outlines` 只有 **269 字**：1 条核心冲突 + 5 条转折点 + 1 条下一节点；转折点**没有章号**。
- `scenes` 1–10 的 `outline_content` 都有 72–83 字的逐章梗概。

其中第 142 章（scene 9）梗概明确写着：

> “周成向赵开来汇报，获准介入承包，并获赠**福摩萨夜总会**道场线索。”

而第 142 章正文中“邵卫江/福摩萨”出现在第 1892/2325 字（约 81% 处），**不在 writer 进度块所取的“末 220 字”窗口内**。结果是：

- 该训练生成的沙盒第 11 章正文完全没有出现“邵卫江/福摩萨/夜总会”（`grep` 计数 0）；
- 模型留在医院，自造“闭口燕”对话与冲突；
- 生成正文还把第一人称主角称作“杨先生”（原文是“我”，且本章 134 章已经有真实人名“周成”）。

报告结果：总分 **0.24**，承接 **1.2**，情节 **2.3**。编审总结：

> “AI 在文笔模仿上有一定天赋（尤其第19章），但**缺乏长程记忆与逻辑连贯性**，导致剧情严重幻觉与原地踏步。”

**这组对照说明：**
- 资产重建失败时，模型看到的是一份“单章回顾”冒充大纲——比没有更糟；
- 资产重建成功时，10 条梗概已经在库里，但 writer 的进度块只看最近 3 章正文尾巴，planner 只看最近 3 条梗概，**第 142 章的关键铺垫在写第 11 章时仍可能完全不可见**；
- 因此，用户提出的“对前 10 章写故事大纲并和上一章结尾一起给 LLM”是正确方向，但必须落实到**逐章、全量、结构化、进入 writer 与 planner** 这四个约束上。

---

## 4. 根因清单

### R1（致命）长程记忆只存在于数据库，没有进入提示词

10 章原文灌进 `scenes` 后，除了风格采样和一次性资产重建，之后再也没有以“前情”的形式回到 writer。`progress_lines_from_parts` 只取最近 3 章；`generate_chapter_outline` 只取最近 3 条梗概；`PriorChapterContext` 只带上一章。**“前 10 章”在 prompt 里没有稳定位置。**

### R2（致命）“故事大纲”是全局提纲，不是逐章故事簿

`condense_story_outline_impl` 的设计目标是“稳定前缀里的一段抽象方向”，它天然会丢掉章号、时间顺序和事件因果。11-17 的 269 字大纲只有 5 条无章号转折；11-18 的 1200 字大纲只是一个单章回顾。**它无法回答“第 135 章发生了什么、第 139 章留下了什么未结事”。**

### R3（高）逐章梗概写进 `scenes.outline_content` 后基本不被消费

v0.99.0 已经能在模型漏返回时用“原文首尾 70 字窗口”兜底写梗概（`deterministic_chapter_summaries`），但：
- writer 的进度块优先读**正文末 220 字**，正文 ≥40 字就不会读梗概；
- planner 只读最近 3 条梗概（`coordinator.rs:5384-5406`）；
- `next_node` 只拿最近 3 条梗概去全局大纲里挑句子（`beat_card.rs:792-797`）。

10 条梗概里通常只有 2–3 条在任意时刻可见，且会随章节推进被滚掉。

### R4（致命）`scene_outcomes` 一出现，进度块就把上下文 10 章整体替换掉

`progress_lines_from_parts` 的逻辑是：

```rust
if !parts.outcomes.is_empty() {
    return parts.outcomes.iter().take(3)...;   // 只返回最近 3 条
}
// 否则才回退到 scenes 正文末 220 字
```

`load_continue_context_parts` 只加载 `list_for_story(story_id, 3)`。第 11 章装配后，outcomes 非空，**前 10 章上下文不再出现在进度块**；第 12 章起模型只剩最近 3 条“计划型 outcome”。这不是“3 章窗口”，而是“3 条计划摘要”。

### R5（高）章纲规划看不到承接锚点，也看不到完整前情

`generate_chapter_outline` 把训练指令作为 `directive`，只取前 3200 字（`OUTLINE_DIRECTIVE_CHAR_CAP`）。而训练指令的顺序是：

```text
base（~150 字）→ 名册 → 红线（~700 字）→ 历史结论 → 维度修正 → 文风锚点（~1200 字）→ 承接锚点（~1400 字）→ 最后提醒
```

在多数运行里，**承接锚点在 3200 字之外**，planner 根本看不到“上一章最后一句是什么”。writer 能看到，但 planner 已经先按“没有锚点”的进度规划了本章；writer 收到冲突的“章纲 vs 锚点”，更容易跟着章纲跑。

### R6（致命，且会造成“比没有更糟”）资产重建 all-or-nothing，失败后 `ensure_story_outline` 用头/尾窗口生成“规划文档”冒充故事大纲

- 资产重建是两个 8192-token 的巨型 JSON 调用；10 章 + 世界观 + 8 角色 + 关系 + 逐章梗概，很容易被截断/CoT-only（11-18 两次尝试、3 分 40 秒，只 salvage 出世界观）。
- `persist_training_bundle` 在 `bundle` 无效时不会落任何逐章梗概。
- 随后 `ensure_assets` 发现 `story_outlines` 为空，调用 `ensure_story_outline`：
  - `prose = concat_story_prose`（10 章约 24k 字）；
  - `excerpt = slice_prior_prose(prose)` = 头 600 + 尾 1800，**中间 8 章不可见**；
  - system prompt 是 `methodology_scene_structure`（场景级“目标→冲突→灾难”）；
  - user prompt 明确要求“归纳故事大纲，并**规划下一拍如何推进**”。
- 产出的就是 11-18 那份“第 134 章回顾 + 下一拍规划”。它被写进 `story_outlines`，然后以“本场景必须围绕此大纲展开”的强措辞进入 writer。**这是直接导致后续 9 章钉在旧现场、无法推进的污染源。**

### R7（高）批次内没有滚动账本；gate∥writer 并发只补了 n-1 的正文

- v0.98.7 的 `PriorChapterContext` 只带上一章正文/大纲，解决了“writer 读到 n-2”的 bug，但没有解决“第 n 章不知道第 11..n-1 章整体发生了什么”。
- `gate(n-1) ∥ writer(n)` 并发时，DB 里最新的已装配 scene 是 n-2；进度块、outcome、planner 的最近 3 条都因此**再滞后一章**。
- 结果：第 12–20 章的记忆窗口实际是“n-1 正文 + n-4..n-2 的尾巴”，不是“前 10 章 + 本批已写”。

### R8（高）设定白名单不存在，红线不可执行

`TRAINING_REDLINES` 第 1、5 条要求“只沿用前文真实原文已经出现的术法、门派、器物与专有名词”，但：
- writer 只看到上一章 1400 字，无法知道 10 章里已经出现过什么；
- 没有一份“已确立名词表”可对照；
- 名册（`load_canonical_roster`）只取 `source='training_assets'` 的角色；11-18 资产重建 0 角色 → 名册为空，红线等于没有执行依据。
- 11-18 里还出现了 `characters` 表被 `ingest`/`user_created` 写入“陈先生/蔡长庚/陈砚舟”的情况，说明“沙盒名册”本身也会被后续链路污染。

### R9（中高）资产预算和上限优先挤掉“前文/进度”

- `ASSET_CHAR_BUDGET = 6000`；`OUTLINE_CHAR_CAP = 1200`；`CHAPTER_OUTLINE_CHAR_CAP = 800`。
- 稳定区（红线/大纲/世界观/风格/方法论/叙事策略/Logline/本章大纲/下一节点/人物卡/名册）本身就可能接近或超过 6000；`apply_asset_budget` 从尾部开始压缩动态块，顺序是 arcs → tensions → prior → progress → goals → …
- 也就是说，**最需要保住的“前文”和“进度”排在压缩链的前面**（在 arcs/tensions 之后最先被削）。在 11-18 这种风格锚点 1200 + 承接锚点 1400 的训练里，资产块很容易把 `【前文】` 的头段削掉。

### R10（中）评估闭环只看单章对照，不检查长程弧与线索回收

- 逐章编审是“生成章 vs 对应真实章”，没有“这 10 章是否形成一条连贯的推进线”的确定性检查；
- 跨章总评看的是每章开头/结尾 220 字 + 逐章评语，能描述漂移，但不能定位“哪条线在哪一章丢了”；
- 报告没有“记忆使用情况”字段（10 条梗概是否进入 prompt、账本是否注入、白名单命中/违反次数），所以下一轮修正只能继续给抽象要求（“严格承接”“回归原作逻辑”），无法针对记忆层修复。

---

## 5. 目标设计：三层记忆

### 5.1 三层结构

```text
① BackstoryBrief（前情提要，整批固定，stable）
   - 总脉络（100–200 字）
   - 逐章事件（每章 100–150 字：地点/在场/事件/结果/未决）
   - 未结线索（id/描述/首次出现章/最近推进章/状态/下一预期动作）
   - 人物当前态（姓名/身份/目标/情绪/知道什么/不知道什么/持有物）
   - 设定白名单（术法/门派/器物/地名/组织，一行一个，必须来自前文）

② BatchLedger（本批续写账本，逐章追加，dynamic）
   - 第 11..n-1 章：实际结果（1–2 句）+ 推进/新增/回收的线索 + 状态变化
   - 只写“已发生”，不写计划；禁止把章纲原文贴进去

③ SceneAnchor（上一章结尾现场，dynamic）
   - 现有 900/1400 字原文锚点保留，只用于对齐落笔点
```

> 注意：不要把三者合并成一段“故事大纲”。故事大纲负责“方向”，BackstoryBrief 负责“事实与状态”，BatchLedger 负责“本批已发生”。三者混在一起会重新变成 R2。

### 5.2 BackstoryBrief 的建议 schema

```json
{
  "arc_summary": "从第134章赵开来饭局到第143章医院截护士，主角周成从立足金城到接下木磨山承包与夜总会道场，核心冲突是地仙会/三理教/公家承包权。",
  "chapters": [
    {
      "chapter": 134,
      "location": "金城·饭馆",
      "cast": ["我", "赵开来", "张宝山"],
      "events": "赵开来点破主角借刀杀人的来意；张宝山来电请主角看青年旅社斗殴案。",
      "result": "主角接下特聘顾问的案子，随张宝山去武警医院。",
      "open_thread": "旅社第七名年轻女人在逃；小护士身份可疑。"
    }
  ],
  "threads": [
    {
      "id": "t-fomosa",
      "description": "赵开来承诺赠送福摩萨夜总会道场，需去见邵卫江。",
      "introduced_chapter": 142,
      "last_advanced_chapter": 142,
      "status": "open",
      "next_expected": "主角应在近期去福摩萨夜总会见邵卫江。"
    }
  ],
  "cast_states": [
    {
      "name": "我（周成）",
      "role": "主角/阴脉先生",
      "location": "武警医院走廊",
      "goal": "查清小护士身份，处理燕行团伙。",
      "knows": ["燕行七人", "小护士换药异常"],
      "does_not_know": ["第七名燕子的去向"],
      "possessions": ["烟", "桃木法剑"]
    }
  ],
  "canon_terms": [
    {"term": "阴脉先生", "type": "身份", "note": "主角行医身份"},
    {"term": "三理教", "type": "组织", "note": "已退出金城，甩卖资产"},
    {"term": "福摩萨夜总会", "type": "地点", "note": "赵开来承诺的道场线索"}
  ]
}
```

渲染为中文块时，控制在 `BACKSTORY_CHAR_CAP`（建议 2600，10 章场景）以内，**优先保逐章事件和未结线索，其次人物态，最后 whitelist**。

### 5.3 Prompt 注入协议（遵守 v0.74.3 稳定前缀契约）

```text
【稳定区】
红线
【前情提要（原书第134–143章，本批固定）】   ← 新，protected，≤2600
【故事大纲（全局方向）】                     ← 现有，≤1200
【世界观设定】                               ← 现有，原文顺序
【风格 DNA / 文风锚点】
【Logline】
【本章大纲】                                 ← 每章变，但已属“准稳定”
【下一节点】                                 ← 动态开始
【人物卡 / 名册 / 关系 / 伏笔】
【续写账本（沙盒第11..n-1章）】              ← 新，dynamic，≤800
【已推进进度】                               ← 现有，改为“全量前情摘要 + 本批账本”
【上一章结尾现场（仅对齐落笔点）】            ← 现有，dynamic
【最后提醒】
```

关键点：
- **前情提要与全局大纲都不要注入 `next_node`**（next_node 每章变，会破坏前缀缓存）。
- **账本必须放在动态区**，每章更新；不能混进 stable。
- **上一章结尾锚点保留在最后**（recency），但不要再承担“长程记忆”的职责。

### 5.4 构建策略（可靠性优先）

现状是“一次巨型 JSON 调用，all-or-nothing”。建议拆成两阶段：

**阶段 A：逐章摘要（可并行、可缓存）**

- 输入：每章正文（长章头 1300 + 尾 1300，短章全文）。
- 输出：`{chapter, location, cast[], events, result, open_thread, turning_point}`。
- 每章一次小调用（JSON mode，max_tokens 800–1200），并发 2–4。
- 失败/不可解析 → 用确定性兜底：
  - 正文开头 70 字 + 结尾 70 字（已有 `deterministic_chapter_summaries`）；
  - 或抽取“对话/动作转折句”（可选，P1）。
- 缓存键：`server_id + book_ref + chapter_index + prompt_version`（复用 V161 缓存思路）；同一本书反复训练不重复摘要。

**阶段 B：聚合 brief**

- 输入：10 条结构化摘要（不是 26k 原文）。
- 输出：`arc_summary + threads + cast_states + canon_terms`。
- 一次小调用（JSON mode，max_tokens 2000–3000）；失败 → 用逐章摘要确定性拼装：`arc_summary = 首章 result + 末章 result`，threads 取每章 `open_thread` 去重，cast 取高频人名，canon_terms 从原文高频专名提取（可用现有 `prose_ground` 的名单/名词启发式）。
- **绝不接受“散文/规划文档”作为 brief**：解析失败就不写，宁可只用确定性拼装。

**禁止项（针对 R6）：**
- 训练沙盒在执行 `run_continue_batch` 前必须已有一份**确定性全局大纲**（可由 brief 生成，如“本批必须推进以下未结线索：…”），使 `ensure_assets` 的 `story_outlines` 判定为非空，从而跳过 `ensure_story_outline`。
- 或者给 `run_continue_batch_*` 加 `assets_seeded: bool`（训练路径 true），`ensure_assets` 在 true 时跳过 `ensure_story_outline` / `hot_path_extract_from_prose`。
- 即便走 `ensure_story_outline`，也要：
  - 用 `agency_problem_outline`（故事级）而不是场景方法论；
  - 输入用**逐章摘要**而不是 `slice_prior_prose` 的头 600 + 尾 1800；
  - 输出禁止包含“下一拍规划/场景一/目标→冲突→灾难”等规划标记；
  - 落库前用 `strip_outline_planning` + 长度/章号覆盖校验，不通过就不写。

### 5.5 Planner / Writer / Gate 的契约

- **Planner**（`generate_chapter_outline`）：
  - 新增 `backstory` 变量（完整 brief）；
  - 新增 `prior_tail` 变量（上一章最后 800–1400 字），不再依赖 `directive` 的 3200 字截断；
  - 输出章纲必须显式“承接 brief 中的哪条未结线索 / 推进哪个 thread id”；
  - 章纲长度 200–400 字，`CHAPTER_OUTLINE_CHAR_CAP=800` 保留。
- **Writer**：
  - `assets_ctx` 含完整 brief + 账本；
  - `writer_task` 增加硬约束：“本章必须推进 brief 中至少一条 open thread；若本章引入新专名，必须来自 canon_terms，否则不允许作为设定使用”；
  - 保留“禁止复制锚点”的最后提醒。
- **Gate / Editor**：
  - 除现有维度外，增加确定性探针：
    - `canon_probe`：正文中的新专名（2–4 字、非停用词）不在 canon_terms 且不在 context prose → 记录；若被写成规则/体系 → blocking。
    - `location_probe`：本章开头地点不在上一章结尾地点/brief 允许范围，且正文没有过渡动作 → 记录。
    - `cast_probe`：有名有姓的新角色不在 roster/brief → 记录；若给对白/行动 → blocking。
    - `thread_probe`：本章未推进任何 brief open thread，且不是“收束/过场”章 → 记录。
  - 探针结果写进 gate comments / 训练报告，驱动下一轮。

### 5.6 本批滚动账本（BatchLedger）

在 `run_batch_inner` 里维护：

```rust
struct LedgerEntry {
    chapter: i32,
    result: String,        // 1–2 句“实际发生了什么”
    advanced_threads: Vec<String>,
    new_threads: Vec<String>,
    state_delta: String,
}
```

- 第 n 章 writer 接收 `ledger[0..n-1]`（渲染 ≤800 字）。
- gate∥writer 并发时，把 `pending_chapter` 的 n-1 **已装配 artifact** 先入账本，再启动 n；这样 n 看到的是 n-1 的实际结果，不是计划。
- `result` 的来源（按优先级）：
  1. 第 n 章最终装配后的**确定性摘取**：最后 2–3 句 + 章纲的 `outcome` 字段；
  2. 一次小 LLM 摘要（200–400 tokens，可与下一章 writer 并行）；
  3. 若都失败：用章纲 + `SceneOutcome.outcome`，但标注“计划”而不是“实际”。
- 账本只写“已发生”，不写“下一拍”，避免变成第二个 planner。

### 5.7 报告与观测

训练报告新增“记忆使用”段：

```text
- 前情提要：10 章逐章梗概 ✓（xxx 字，全部进入 writer/planner）
- 未结线索：N 条（本章推进 M 条）
- 人物当前态：N 人（本章新登场 K 人，其中白名单外 0 人）
- 设定白名单：N 项（本章新增专名 0 个；违规 0 个）
- 本批账本：第 11..n-1 章（xxx 字）
- 资产重建失败兜底：否 / 是（确定性逐章摘要）
```

这样下一轮的“改进项”才有可核验的落点，而不是继续写“严格承接”。

---

## 6. 分阶段落地

### P0（1–2 天，最小闭环，先止血）

1. **全量前情摘要进入 writer**
   - 在 `render_continue_assets` 新增 `backstory_lines`：读取 `parts.scenes` 中所有**当前章之前**的 scene：
     - 优先 `outline_content`（去掉“本章梗概：”前缀）；
     - 其次正文末 160 字；
     - 每章一行 `第N章：…`；总上限 2400 字。
   - 放进稳定区（红线之后、故事大纲之前），标记 `protected=true`，确保 `apply_asset_budget` 不裁它。
   - 这样即使不改资产重建，也能让 11-18 那种“0 梗概”场景至少有正文兜底；11-17 那种“10 梗概”场景直接受益。

2. **进度块改为“全量摘要 + 最近 3 章正文尾 + 最近 3 条 outcome”**
   - `progress_lines_from_parts` 不再 `return` outcomes；改为拼接：
     - 所有 prior chapter 的 `outline_content`（或正文尾）；
     - 最近 3 条 outcome；
     - 总上限 1200–1600 字。
   - **不要**让 outcomes 把上下文 10 章整体替换掉。

3. **Planner 看到 brief + 锚点**
   - `generate_chapter_outline` 新增 `backstory` 变量（来自同一 brief）；
   - 新增 `prior_tail` 变量（上一章末 800–1400 字），不再依赖 `directive` 的 3200 字；
   - `directive` 上限可维持 3200，但把 brief/锚点从 directive 中剥离，单独注入。

4. **训练路径禁止 `ensure_story_outline` 污染**
   - 训练 executor 在 `run_continue_batch` 前写入确定性全局大纲（由逐章摘要拼装）；
   - 或给 `run_continue_batch_*` 加 `assets_seeded=true`，`ensure_assets` 跳过 `ensure_story_outline`；
   - `ensure_story_outline` 自身改为“用逐章摘要/全量摘要”，并拒绝规划标记。

5. **预算**
   - `BACKSTORY_CHAR_CAP = 2600`（按 `context_n` 缩放，30 章可 4000）；
   - 将 `backstory` 放在 `render_continue_assets` 的 protected 区最前，不参与 6000 预算竞争；
   - 或者训练路径把 `ASSET_CHAR_BUDGET` 提升到 8000，但仅限训练。

6. **测试**
   - `render_continue_assets` 10 章 fixture：断言包含全部 10 个章号；
   - planner prompt：断言包含 brief 和上一章末段；
   - 第 12 章 writer prompt：断言包含第 11 章账本/摘要；
   - truth 章绝不进入 brief（复用 v0.93.16 的基准章泄漏测试口径）。

### P1（3–5 天，结构化故事簿 + 账本 + 探针）

1. 逐章摘要 pass（5.4 阶段 A），缓存摘要（新表或 V161 扩展）。
2. `RawBundle` 增加 `open_threads`、`canon_terms`、`cast_states`，落库后生成 `BackstoryBrief`（新表 V162 `story_briefs`，或 `continuation_training_briefs`）。
3. `BatchLedger` 在批内滚动，gate∥writer 并发时用 pending n-1 artifact 入账。
4. 确定性探针（canon/location/cast/thread）接入 gate 和报告。
5. 报告新增“记忆使用”段；训练页可展示 brief 预览（便于用户直接看“模型到底看到了什么”）。

### P2（泛化）

- 把 `StoryBriefService` 从训练沙盒推广到普通续写：从 `scenes.outline_content` + 角色/关系/伏笔表构建 brief；对长篇连载尤其有价值。
- 允许用户在幕后编辑/校正 brief（类似角色卡/世界观），把它变成一等创作资产。
- A/B 实验：同一本书、同一区间、固定 seed/温度，比较 “tail only / +brief / +brief+ledger” 三组。

---

## 7. 验收标准

### 7.1 单元/集成（必须进 CI）

| 用例 | 断言 |
|---|---|
| `backstory_brief_contains_all_context_chapters` | 10 章 fixture，brief 中有 134..143 全部章号，顺序正确，无 truth 章 |
| `writer_prompt_contains_full_backstory` | `render_continue_assets` 输出包含 10 条“第N章：…”，且该块位于故事大纲之前 |
| `planner_prompt_contains_backstory_and_anchor` | `generate_chapter_outline` 的 prompt 含 brief + 上一章末段 |
| `batch_ledger_reaches_next_chapter` | 第 12 章 writer prompt 含第 11 章账本；gate∥writer 并发下仍含 |
| `asset_rebuild_failure_does_not_write_planning_outline` | 空 bundle + prose 兜底时，`story_outlines` 不含“下一拍规划/场景一/目标→冲突→灾难” |
| `backstory_survives_asset_budget` | 动态块塞满 6000 时，backstory 仍在输出中且完整 |
| `truth_chapters_never_leak_into_brief` | 故意把 truth 章传入，brief/asset prompt 均不包含其内容 |
| `canon_probe_flags_new_system_term` | “引魂蛆”作为规则写入正文 → probe 报警 |
| `thread_probe_detects_no_advance` | 连续两章未推进任何 open thread → 报告标记 |

### 7.2 真机 A/B（同一本书、同一区间）

建议用《阴脉先生》134–143 上下文、144–153 基准，跑三轮：

| 组 | 配置 | 期望 |
|---|---|---|
| A | 现状（仅上一章锚点） | 承接 ~3，情节 ~3 |
| B | A + 全量逐章梗概进入 writer/planner | 承接 ≥5，情节 ≥5 |
| C | B + brief（线索/人物态/白名单）+ 账本 | 承接 ≥6，情节 ≥6；新体系名词 0；相邻章复制 0 |

验收指标：
- 加权总分 ≥0.60；
- 承接 ≥6.0、情节 ≥6.0；
- 10 章全部有可用正文（非空、非重复、非拒绝话术）；
- 报告中“记忆使用”段显示 10/10 章摘要进入 prompt；
- 编审逐章“承接”列不再出现“完全无视上一章/场景跳跃”；
- `canon` 违规 0（引魂蛆/骨刺类新体系名词 0）。

### 7.3 反例/回归

- 资产重建失败注入：训练仍完成；brief 由确定性摘要拼装；`story_outlines` 不被污染；
- 预算压力注入：动态块超大时 backstory 不被裁；
- 第 1 章与第 12 章：第 1 章用原书末章锚点；第 12 章用沙盒第 11 章账本 + 原书 brief；两者不混淆；
- 训练沙盒清理/保留 3 个：brief 随沙盒删除级联，不泄漏到下一轮。

---

## 8. 风险与反模式

1. **不要把“故事大纲”写成又一篇散文。** 必须是结构化字段 + 章号 + 状态；否则会重演 11-18 的“单章回顾冒充大纲”。
2. **不要用 `next_node` 参与 brief 的稳定区。** 它每章变，会打断前缀缓存（v0.74.3 教训）。
3. **不要让 brief 自己成为幻觉源。** 所有人物/专名必须用 `name_in_prose`/context prose 接地；摘要里的名册外人名一律丢弃。
4. **不要把 truth 章泄漏进 brief。** brief 只能由 context 章构建；这条要单测锁死。
5. **不要把账本写成第二个 planner。** 账本只写“已发生”，不写“下一拍”；否则模型会照着账本再演一遍。
6. **不要只改 prompt 不改预算。** 6000 字资产预算会把新增的 brief/账本从尾部挤掉；必须给 brief 独立 protected 额度。
7. **不要忽略 gate∥writer 并发。** 第 n 章的账本必须包含 n-1 的**实际装配结果**，不能用 DB 的 n-2 或 pending 的“计划”。
8. **不要把 `ensure_story_outline` 当作训练兜底。** 它是为“空书/无大纲”设计的场景级兜底，不适合“有 10 章正文但没有大纲”的训练沙盒；训练路径必须显式跳过或替换。

---

## 9. 附录：关键代码索引

| 主题 | 文件:行 |
|---|---|
| 硬承接锚点 | `src-tauri/src/task_system/continuation_training.rs:227-241` |
| 训练红线 | `src-tauri/src/task_system/continuation_training.rs:62-67` |
| 训练指令组装 | `src-tauri/src/task_system/continuation_training.rs:522-576` |
| 资产重建 prompt | `src-tauri/src/agency/training_assets.rs:160-194` |
| 逐章梗概落库 | `src-tauri/src/agency/training_assets.rs:753-789` |
| 确定性首尾梗概 | `src-tauri/src/agency/training_assets.rs:541-560` |
| 大纲兜底（污染源） | `src-tauri/src/agency/coordinator.rs:4956-5105` |
| 大纲压缩 | `src-tauri/src/agency/continue_assets.rs:388-452` |
| 规划标记清洗 | `src-tauri/src/agency/continue_assets.rs:352-375` |
| 资产渲染顺序 | `src-tauri/src/agency/continue_assets.rs:1085-1105` |
| 资产预算 | `src-tauri/src/agency/continue_assets.rs:763-791` |
| 进度块（3 章/outcomes） | `src-tauri/src/agency/coordinator.rs:10132-10186` |
| outcomes 只取 3 | `src-tauri/src/agency/coordinator.rs:9796-9798` |
| 章纲规划（3200 cap） | `src-tauri/src/agency/coordinator.rs:5267-5470` |
| directive 上限 | `src-tauri/src/agency/coordinator.rs:10449` |
| writer 任务组装 | `src-tauri/src/agency/coordinator.rs:6687-6905` |
| 在途上一章 | `src-tauri/src/agency/coordinator.rs:8183-8229` |
| 下一节点编译 | `src-tauri/src/agency/beat_card.rs:725-798` |
| outcome 计划语义 | `src-tauri/src/agency/narrative_contract/outcome.rs:26-46` |
| 场景正文拼接 | `src-tauri/src/agency/materialize.rs:26-42` |

---

## 10. 一句话建议

**先把“10 条逐章梗概 + 未结线索 + 人物态 + 设定白名单”做成一个 protected 的 `前情提要` 块，同时进入 planner 和 writer；再用批内账本让第 12–20 章看到第 11..n-1 章实际发生了什么；最后把 `ensure_story_outline` 的“头 600 + 尾 1800 规划文档”从训练路径彻底移除。** 这三件事做完，才配得上“故事大纲 + 上一章结尾一起给 LLM”，也才有希望把承接从 1.2–3.1 拉到 6 以上。
