# 续写叙事质量设计：从「资产倾倒」到「叙事契约」

- 日期：2026-11-02
- 状态：**已实施 P0 + M3/M5 增量（v0.80.1）**。实施方案见 [2026-11-02-narrative-quality-implementation-plan.md](./2026-11-02-narrative-quality-implementation-plan.md)，交付记录见 [CHANGELOG v0.80.1](../../CHANGELOG.md)。
- 范围：Agency 续写主路径（幕前 Append、幕后 NextChapter、批量续写）。改写/审稿仍走现有 planner，不在本文范围。
- 关联：`docs/plans/2026-08-13-agency-only-continuation-design.md`

---

## 0. 结论先行

当前续写的主要矛盾不是「提示词不够长」，而是四件事同时缺失：

1. **写作前没有契约。** writer 拿到的是故事大纲、章纲、节拍卡、角色卡、状态网、情感块、检索块的并置；但没有一份「这一场谁要什么、谁挡着、失败失去什么、必须发生什么不可逆变化」的可执行契约。
2. **计划源分裂。** 章纲、拍节点、冲突、情感是四套独立生成物，彼此不校验。`next_node` 甚至是故事大纲的字符串匹配，匹配失败就回退到通用模板。
3. **结构资产只在旧路径生效。** 方法论、节拍规划、追读力目标、追读力债务、故事合同约束、风格 DNA、叙事四元组、参考场景等大量资产，在 legacy writer/planner 路径有注入，但 Agency 主路径没有消费。
4. **质检是事后总分，不是契约核对。** 编辑审计的 9 个维度只落库，不参与门禁判定；幕前 Append 的质量门是后台审计，不阻断、不修订；失败不会改变下一拍。

此外，**人物命名存在独立的分布塌缩**：现有创世提示词禁止最常见姓氏和单字名，模型于是退回一小撮「小说姓 + 冷色/夜色/墨色用字」，同一本书里反复出现林雪、沈夜、苏墨、顾寒这类组合。姓名不是文笔问题，是资产分布问题；命名系统见 §5.5。

**设计主张：叙事契约编译器（Narrative Contract Compiler，NCC）。**

> 先用现有资产编译出「章契约」与「场契约」，再让 writer 写「这一场」，最后用同一份契约逐项核对，并把结果回流到下一场。

这不是推翻 v0.75 的情感回路；恰恰相反，它把情感回路从「可选调制器」提升为章/场契约的主驱动（见 §4.3.1）。情感回路已经解决了「情绪怎么变成因果链」的问题，剩下要解决的是「它什么时候必须启用、由谁来决定事件、失败了谁来拦」。

提示词只保留可执行的最小集合，不再堆规则。预期结果是：每一拍都有具体目标、具体阻力、具体赌注、明确转折、不可逆结果和情感因果链；不再出现「按场景结构推进」这类通用节点；幕前 Append 也能在返回前拦住结构性空转。

---

## 1. 症状与代码证据

> 以下为代码核对结论，不是对某一部作品的逐句归因。四类症状都能在这些结构缺口里找到可验证的解释。

### 1.1 情节莫名其妙：事件之间没有「因为—所以」

**证据一：章纲和拍节点是两套独立计划。**

- 章纲由 `generate_chapter_outline`（`src-tauri/src/agency/coordinator.rs:4728`）用 `scene_outline` 提示词生成，输入是故事大纲、世界观、前 3 章 `outline_content` 的前 200 字，**不是上一章正文**。
- 拍节点由 `compile_next_node`（`src-tauri/src/agency/beat_card.rs:710`）生成：优先读当前场景卡块的「下一拍」，否则从故事大纲的句子里找第一个「近文未覆盖、且包含在场活人姓名」的句子，再否则走方法论兜底。它**不读章纲**。
- 两者可以同时出现在 writer prompt 里，但互不校验：章纲说本章要「当众烧名册」，拍卡说「按场景结构推进」；模型只能自己猜哪个算数。

**证据二：大纲接地失败会整段丢弃。**

- `compile_next_node` 在 `outline_is_grounded` 失败时把故事大纲置空，直接返回 `methodology_next_node` 的通用句（`src-tauri/src/agency/beat_card.rs:768`、`:774`）。
- 通用句长这样：「按场景结构推进：目标→冲突→灾难或反应→困境→决定。只写本场甲、乙，不得另起开篇。」（`src-tauri/src/agency/prose_ground.rs:60`）
- 这句话没有本故事的方向。它不是「下一节点」，只是「请按结构写」的元指令。`is_generic_next_node` 也明确把它列为兜底（`prose_ground.rs:50`）。

**证据三：章纲没有因果校验。**

`scene_outline.md` 要求「承接上一节点、引出下一节点」，但输出是自由 Markdown，没有校验每条关键事件是否由前一条引起、是否改变状态、是否与正文事实冲突。`prose_ground` 只校验故事大纲是否接地，不校验章纲。

**结论：** 「莫名其妙」不是文笔问题，是事件之间缺少前置条件、动机和结果。模型被要求「推进到下一节点」，但系统没有告诉它「下一节点为什么必须由上一节点导致」。

---

### 1.2 无戏剧冲突：冲突是静态关系标签，不是目标对撞

**证据一：冲突只从「敌对关系 + 双方在场」生成。**

- `compile_conflict`（`src-tauri/src/agency/beat_card.rs:475`）只扫描 `character_relationships` 中带仇/敌/对立/背叛/欺骗/复仇/恨等关键词的边。
- 双方都必须出现在近文 1500 字阵容（`PRIOR_CAST_CHAR_CAP`）里；对手不在场，这条关系就被跳过。
- 找不到敌对关系时，退化为：「甲、乙必须在本拍与阻力正面对峙，不得只靠对话过渡。」（`beat_card.rs:560` 附近）
- 它没有从「目标不相容、资源稀缺、秘密、期限、道德困境」生成冲突，也没有「反对者想要什么、他的计划是什么」。

**证据二：冲突没有赌注和失败代价。**

`ConflictMove` 只有 `action`、`parties`、`stage` 三个字段（`beat_card.rs:20` 附近）。`ConflictStage` 提供「加压 → 升级 → 结账 → 余波」的推进阶梯，这是好的骨架，但每一级只说「对峙要付出代价 / 要出现不可逆结果」，没有具体到「失败会失去什么」。

**证据三：「必须改变」可以靠再威胁一次满足。**

`compile_change_delta`（`beat_card.rs:580`）在冲突行时直接把冲突动作复述为 `ChangeDelta.summary`，轴选 `Risk` 或 `Relationship`。因此「必须改变：风险 — 加压：甲乙正面对峙」并不要求状态从 A 变到 B，只要求再对峙一次。

**结论：** 冲突被降维成「关系表里有一条敌对边」。没有敌对边、对手不在场、或者作者没填关系时，模型只能写泛化的「顶撞/对峙」。

---

### 1.3 无情感共鸣：情感是角色档案，不是事件因果

**证据一：单拍情感块是静态标签。**

`compile_emotion`（`src-tauri/src/agency/beat_card.rs:627`）优先返回上场角色的「内核/伤口/需求」，例如：

> 情感：沈砚：内核=被抛弃的恐惧，伤口=母亲改嫁，需求=被认可

这是人物小传，不是本拍事件。writer 看到它，只能把「伤口」写成形容词或身体反应，无法知道「什么事件触碰了它、角色因此做了什么、付出了什么代价」。

**证据二：真正的情感因果链默认关闭。**

- `load_emotion_plan`（`src-tauri/src/agency/coordinator.rs:8012`）只有在作者设置了情感契约，或已有张力/债务/状态时才返回计划；否则整块不注入。
- 没有情感计划时，`run_rule_grader` 的情感分固定为 `0.5` 中性（`src-tauri/src/agency/graders.rs:120` 附近）。
- 编辑审计的第 9 维 emotion 在「草稿区没有情感回路资产」时要求「按通用标准轻判，不得据此单独判 blocking」（`resources/prompts/agency/agency_editor_auditor_system.md`）。

也就是说：**没有配置情感契约的故事，系统不会要求情感，也不会因此扣分。**

**证据三：张力账本只给一句方向。**

`render_tensions_for_prompt`（`src-tauri/src/agency/emotional_ledger.rs:227`）会写「本节须让至少一条张力推进或释放」，但只有 `suggested_action` 一句（如「本节应让对抗升级为直接冲突」），没有具体事件、选择、代价。

**结论：** 情感共鸣不是「多写身体反应」能解决的。缺的是「事件 → 评估 → 情绪变化 → 行动/台词 → 代价」这条链；这条链在 `emotion_circuit` 里已经实现，但主路径默认不走。

---

### 1.4 故事进展缓慢：允许不推进的结构

**证据一：下一节点可以是通用句。**

如上，`compile_next_node` 可以返回「按场景结构推进…」。通用句不落库，但每拍都会重新生成；writer 每拍都收到「按结构推进」，没有具体方向。故事自然停在原地。

**证据二：进度块不是「已完成事件」。**

`progress_lines_from_parts`（`src-tauri/src/agency/coordinator.rs:8195`）只取前 3 章 `outline_content` 的前 200 字。`outline_content` 里混有卡块（在场/冲突/情感/下一拍）和章纲前缀，不是干净的「上一章发生了什么、结果是什么」。

**证据三：预算收缩优先牺牲冲突与目标。**

`render_continue_assets` 的资产预算 `ASSET_CHAR_BUDGET = 6000`（`continue_assets.rs:16`），收缩从变动区尾部开始（`continue_assets.rs:761`）。当前顺序是：

```
关系 → 冲突 → 目标 → 进度 → 前文 → 张力 → 弧光 → 伏笔
```

预算不足时，先被截断的是伏笔、弧光、张力、前文，再往前是目标、冲突。节拍卡本身不受这个预算约束，但它只有 4–5 行（`beat_card.rs:216`）。

**证据四：门禁不裁决「原地踏步」。**

编辑审计有 `stall` 维度，但 `dimension_scores` 只被写入 `editor_agreements` 表（`src-tauri/src/agency/coordinator.rs:7664` 附近），不参与 `evaluate_gate_impl` 的判定。判定只看 `verdict`、`blocking_issues`、规则复检 High+ 和加权总分（`coordinator.rs:7591` 起）。

**证据五：Append 不修订。**

幕前 Append 的注释写得很清楚：「增量直接合并进既有场景，不建新章、不跑同步质量门（editor 质检后台 spawn）」（`coordinator.rs:3735`）。后台质检只发事件/toast，不改正文（`spawn_editor_qc`，`coordinator.rs:2998`）。

**结论：** 故事可以「合法地」慢：下一节点是通用句、变化轴可以重复、stall 不扣分、Append 不修订。

---

### 1.5 大量结构技巧没有真正进入写作流程

这张表是本次审计最重要的发现之一：**资产存在，加载也存在，但 Agency 主路径没有消费。**

| 资产 | 实现/加载位置 | Agency 主路径是否消费 | 说明 |
|---|---|---|---|
| `methodology_scene_structure` | `resources/prompts/methodology/methodology_scene_structure.md` | ❌ | 只在 `generate_chapter_outline` 的旧分支/legacy writer 中作为方法论提示，未进 writer 主 prompt |
| `methodology_extension` | `write_time_bundle.rs:270` 加载，`to_prompt()` `:877` 渲染 | ❌ | `render_continue_assets` 只读 bundle 的少数字段，不读方法论扩展 |
| `writer_beat_plan` | `planner/executor.rs:1738` | ❌ | 只服务旧 beat_planner，Agency 续写不用 |
| `writer_reading_power_goal` | `agents/service.rs:2081` | ❌ | legacy writer prompt 注入，Agency 不用 |
| `writer_chase_debt` | `agents/service.rs:2037` | ❌ | 同上 |
| `writer_contract_constraints` | `agents/service.rs:2005` | ❌ | 同上 |
| `narrative_quartet` | `write_time_bundle` 字段，`to_prompt()` `:861` | ❌ | Agency 不消费 |
| `style_dna_summary` / `style_dna_extension` | `write_time_bundle` 字段，`to_prompt()` `:856` | ❌ | Agency 只有 `style_learning` 块，不读 Style DNA |
| `reference_scene_fewshots` | `write_time_bundle.rs:217` 加载，`to_prompt()` `:915` | ❌ | Agency 不消费 |
| `genre_profile_strategy` / `genre_antipatterns` / `writing_strategy_constraints` | `write_time_bundle` 字段 | ❌ | Agency 不消费 |
| `chase_debt_text` | `write_time_bundle` 字段 | ❌ | Agency 不消费 |
| `runtime_contract` | `write_time_bundle` 字段 | ⚠️ 仅红线 | Agency 只取 `contract_redlines`，不读完整合同 |
| `writer_system.md`（58 行文笔/结构总则） | `resources/prompts/writer/writer_system.md` | ❌ | 单拍主路径用 `CONTINUE_BEAT_SYSTEM`，不是这份 |
| `agency_writer_system.md` | `resources/prompts/agency/agency_writer_system.md` | ❌ | `AgentRole::Writer` 使用，不是主路径的 `LeadWriter` |
| `agency_lead_writer_system.md` | `resources/prompts/agency/agency_lead_writer_system.md` | ⚠️ 仅批量 | 批量 `write_chapter` 走 tool_loop 时用；单拍不用 |
| `skill_plot_twist` / `skill_emotion_pacing` | `skills/executor.rs` | ❌ | 只在技能执行器/planner，不在续写主路径 |
| `outline_replan` | `agency/outline_replan.rs` | ⚠️ 未接写作前 | 章纲重规划存在，但没有作为「计划不合格再规划」的入口 |

**单拍主路径实际使用的系统提示词**是 `CONTINUE_BEAT_SYSTEM`（`src-tauri/src/prompts/assembly.rs:83`），共 24 行，重点是「不要重复、不要发明人、兑现必须改变、人类文笔基线」。它没有「目标→阻力→转折→结果」的场景引擎，也没有「冲突必须有赌注」「情感必须有代价」「本场结果必须成为下一场前提」。

**批量主路径**用 `write_chapter`（`coordinator.rs:5803`）：章纲 + 资产 + 一句「必须有起伏、有转折、有精彩的冲突」+ 可选情感块。**SceneBeatCard 的冲突/情感/必须改变没有进入批量 writer task**；`render_parts` 只把 `card.next_outline_node` 和地点传进资产块（`coordinator.rs:8338` 附近）。

**结论：** 「结构技巧没有真正用到写作流程」不是感觉，是代码事实。大量能力停留在 `WriteTimeBundle::to_prompt()` 和 legacy writer 路径，Agency 续写只消费了其中一个子集。

---

## 2. 根因归纳

| 编号 | 根因 | 对应症状 |
|---|---|---|
| R1 | 只有资产，没有契约：writer 知道很多事实，但不知道这场戏必须完成什么 | 全部 |
| R2 | 计划源分裂：章纲、拍节点、冲突、情感各自生成，无共同事实基座 | 情节莫名其妙、进展缓慢 |
| R3 | 冲突/情感是静态标签：关系敌对 ≠ 目标对撞；伤口 ≠ 事件因果 | 无冲突、无情感 |
| R4 | 质检是总分不是契约：维度分不裁决，证据不回流 | 进展缓慢、质量不稳定 |
| R5 | 主路径闭环缺失：Append 后台审计不修订，失败不影响下一拍 | 全部 |
| R6 | 资产只在 legacy 路径生效：Agency 编译器没有消费方法论/节拍/追读力/风格资产 | 结构技巧没用上 |

---

## 3. 设计目标与非目标

### 3.1 目标

- **G1：每一拍都是一场戏。** 有明确 POV、目标、阻力、手段、转折、结果；不是「继续写一段」。
- **G2：每一章有一个戏剧问题和一个不可逆转折。** 读者能一句话说出「这一章在追什么」。
- **G3：冲突必须具体。** 具体的人/势力、具体的不相容目标、具体的失败代价；禁止用「众人反对」「命运」「气氛」代替阻力。
- **G4：情感必须有因果链。** 事件 → 评估 → 情绪变化 → 行动/台词 → 代价；禁止只写情绪标签或身体反应。
- **G5：计划、写作、质检、回流共用一份契约。** 不再出现「章纲说 A、拍卡说 B、编辑按 C 打分」。
- **G6：复用现有资产，不增加规则堆叠。** 把已经实现的方法论/节拍/追读力/情感/风格资产编译进契约，而不是再写一套新提示词。
- **G7：姓名像真人，而不是像模型生成的笔名。** 姓氏、长度、用字、音形、语义域按真实分布采样；主要角色有姓名账本和称呼图。

### 3.2 非目标

- 不追求「每场都是高潮」。反应场、静场、关系场都可以，但必须有内在变化。
- 不改写/审稿的 planner 路径。
- 不做整本自动重写；只修当前场/当前章。
- 不引入新的模型供应商或外部服务。
- 不做与叙事质量无关的 UI 改造。
- 不在本文做任务拆分与排期。

---

## 4. 总体设计：叙事契约编译器

### 4.1 数据流

```
DB / 正文 / 资产 / 黑板
        │
        ▼
[0] Grounding：事实快照（已发生事实、角色状态、关系、知识边界、伏笔、承诺）
        │
        ▼
[1] ChapterContract：本章戏剧问题 / 主角欲望 / 反对者 / 赌注 / 转折 / 不可逆结果 / 情感弧
        │  1 次 LLM（每章），确定性校验，失败修复一次
        ▼
[2] SceneContract × 1–3：场目标 / 阻力 / 手段 / 转折 / 结果 / 状态变化 / 情感链 / 钩子
        │  纯 Rust 投影；上一场结果 = 下一场前提
        ▼
[3] Prompt Render：[稳定资产] → [章契约] → [角色状态] → [本场简报] → [指令] → [手法] → [末句锚点]
        │
        ▼
[4] Writer：写「这一场」，不是「续写一段」
        │
        ▼
[5] ContractVerifier：逐字段核对 + 原文证据；确定性探针 + 编辑审计
        │
        ▼
[6] Feedback：状态变化 / 承诺账本 / 下一场前提 / 失败项进下一场契约
        │
        └──────────────► 回到 [2] 或下一章 [1]
```

### 4.2 ChapterContract（章契约）

章契约回答「这一章读者在追什么」。它不是章纲的替代品，而是章纲的**可验证子集**。

```rust
struct ChapterContract {
    chapter_number: i32,

    // 戏剧问题：读者这一章想知道答案的问题
    dramatic_question: String,      // 例：沈砚能不能在宴席结束前拿到名册？
    protagonist: String,            // 本章主视角
    want: String,                   // 具体、可执行、有对象
    opposition: String,             // 具体的人/势力，有自己的目标
    opposition_plan: String,        // 反对者打算怎么阻止
    stakes: String,                 // 失败会失去什么（具体到人/物/身份/机会）
    deadline: Option<String>,       // 本章内的时限（可选，有则必须用）

    // 结构
    turning_point: String,          // 本章中段/后段的转折事件
    irreversible_outcome: String,   // 章末不可逆结果（或新问题）
    target_progress: ProgressDelta, // 本章必须推进到的故事进度

    // 情感
    emotional_arc: ChapterEmotionArc, // 从…到…，触发事件，代价
    relationship_focus: Option<String>, // 本章主要推进/破裂/修复的关系

    // 承诺
    promise_ops: Vec<PromiseOp>,    // 埋设/推进/兑现的伏笔/追读力债务
    must_not: Vec<String>,          // 本章禁止（例如：不能死人、不能揭示真相）

    scenes: Vec<SceneContract>,     // 1–3 场
}
```

**字段约束：**

- `dramatic_question` 必须是「谁 + 想要什么 + 什么阻碍 + 截止条件」的问句，不能是「本章讲述……的故事」。
- `want` 必须包含动词和具体对象；禁止「成长」「变强」「守护一切」这类不可验证目标。
- `opposition` 必须是具名角色/势力；不能是「命运」「众人」「环境」。
- `stakes` 必须写出具体损失；不能只写「失败」「危险」。
- `turning_point` 必须是一个事件，不是情绪；`irreversible_outcome` 必须改变状态或产生新问题。
- `scenes.len()` 为 1–3；最后一场的结果必须等于或导致 `irreversible_outcome`。

### 4.3 SceneContract（场契约）

场契约回答「这一场具体怎么变」。它是 writer 唯一需要执行的叙事指令。

```rust
struct SceneContract {
    index: usize,
    total: usize,
    pov: String,
    place_time: String,

    // 场前状态
    entry_state: StateSnapshot,     // 谁知道什么、谁在哪、关系如何、目标是什么

    // 目标—阻力—手段—转折—结果
    goal: String,                   // 本场 POV 想要的具体结果
    opposition: String,             // 谁/什么阻止
    opposition_want: String,        // 反对者这一场想要什么
    obstacle: String,               // 具体障碍（不是「很难」）
    tactic: String,                 // POV 打算怎么做
    turn: String,                   // 什么事件让计划失效/改变方向
    outcome: String,                // 本场结果（成功/失败/部分成功，但必须改变状态）
    irreversible: bool,             // 是否不可逆
    state_delta: StateDelta,        // 信息/关系/目标/风险/情绪，从 A 到 B

    // 情感因果
    emotion: EmotionChain,          // 事件→评估→情绪→行动→代价

    // 信息与承诺
    information_gap: Option<String>, // 谁知道/不知道什么
    promise_op: Option<PromiseOp>,   // 本场埋/推/收哪条伏笔或债务
    hook_out: String,                // 场末钩子（问题/新威胁/决定）

    // 写作边界
    word_budget: usize,
    must_not: Vec<String>,
}
```

```rust
struct StateDelta {
    axis: ChangeAxis,               // Information | Relationship | Goal | Risk | Emotion
    from: String,
    to: String,
    evidence_hint: String,          // 正文里应该出现什么，便于验证
}

struct EmotionChain {
    trigger_event: String,          // 具体事件
    appraisal: String,              // 相关性/一致性/归因/确定性/可控性/违背底线
    from: EmotionVector,            // 复用 emotion_circuit
    to: EmotionVector,
    action_tendency: String,        // 情绪推出的行动
    cost: String,                   // 行动/选择的代价
    carriers: Vec<Carrier>,         // 只选 2–3 个表达载体
    dialogue_policy: Option<DialoguePolicy>,
}
```

**场契约的硬规则：**

1. 每场必须至少改变一个轴；`from != to`。
2. 每章至少一场 `irreversible = true`；章末场必须不可逆或产生新问题。
3. 上一场的 `outcome` 必须是下一场的 `entry_state` 的一部分；不能跳过。
4. `turn` 必须是事件，不是「她意识到」「气氛变了」。
5. `emotion.trigger_event` 必须与 `turn` 或 `goal` 相关；`cost` 不能为空。
6. 不允许「过渡场」「总结场」「全员表态场」：如果本场没有状态变化，就不写。

### 4.3.1 与 v0.75 情感回路的关系：不另起一套，升级为主驱动

v0.75.0 的情感回路不是被本设计替换，而是本设计的核心素材。它已经把最难的部分做完了：12 维情绪向量、OCC 评估、行动倾向、8 类表达载体、压力场、情感债务、确定性探针、读者面板。问题在于它当前的**位置**：它是一个「拍级调制器」，不是「章/场契约的驱动源」。

现状的三个结构性限制：

1. **默认不启用。** `load_emotion_plan` 只有在作者设置情感契约，或已有张力/债务/状态时才返回计划（`src-tauri/src/agency/coordinator.rs:8012`）；`load_chapter_emotion_plan` 只有契约存在才返回 Some（`coordinator.rs:8069` 附近）。没有配置的故事，writer 只拿到 `compile_emotion` 的静态角色档案（`beat_card.rs:627`），不是事件因果链。
2. **下游调制，不是上游驱动。** 先由 `compile_conflict` / `compile_next_node` 选出冲突与节点，再由情感计划选择「怎么读这一拍」；`override_conflict_from_emotion` 只有在关系对已在本拍阵容时才替换冲突。情感不能决定「这一场必须发生什么」，只能决定「已选中的事件带什么情绪」。
3. **不参与硬判定。** 没有计划时 rule grader 情感分固定 0.5；编辑审计第 9 维要求「无情感资产时轻判」；批量路径只把情感块插进 task，模型可以忽略而不被拦截。

NCC 的改法：把情感回路从「附加块」提升为「契约的一等字段」，并默认启用最小链。

| v0.75 现有能力 | 在 NCC 中的位置 |
|---|---|
| `EmotionVector` / `EmotionContract.core_question` | `ChapterContract.emotional_arc` 的起点/终点与本章情感问题 |
| `OCC appraisal`（相关性/一致性/归因/确定性/可控性/违背底线） | `SceneContract.emotion.appraisal` |
| `ActionTendency` | `SceneContract.tactic` 的候选之一；情绪推出的行动必须落到本场手段 |
| `Carrier`（8 类） | `SceneContract.emotion.carriers`，仍只选 2–3 项 |
| `DialoguePolicy` | `SceneContract.emotion.dialogue_policy` |
| `EmotionTension` / `EmotionDebt` | `ChapterContract.relationship_focus` / `promise_ops` |
| `ReaderPanel` / `reader_target` | 验收指标与编辑审计的读者目标核对 |
| `EmotionProbe` | `ContractVerifier` 的确定性硬检查（因果、位移、张力兑现、melodrama、克制、载体稀疏） |
| `render_emotion_block` | 场简报的「情感链」段（更短、嵌入契约，不单独成块） |

具体规则：

- **情感先选事件，事件再选情绪。** 章契约先确定「本章要触碰谁的伤口/需求」，再从候选事件中选一个能造成最大压力、且与外部目标冲突的事件；不是先写事件再贴情绪。
- **情绪必须改变行动。** `action_tendency` 必须成为本场 `tactic` 的一部分；`cost` 必须成为 `state_delta` 的一部分。做不到，就说明这个情绪只是装饰。
- **最小链默认启用。** 没有作者契约时，从角色的 `emotional_wound/need/core` + 当前关系张力 + 本场目标确定性推导：触发事件 = 本场转折，评估 = 目标受阻/推进，行动 = 角色惯用防御，代价 = 赌注的一部分。没有情感资产的故事，跳过情感约束，但仍保留目标/阻力/结果契约。
- **完整回路仍是可选增强。** 作者配置了情感契约、张力、债务时，使用完整的 OCC/载体/读者面板；没有配置时，不阻塞创作，但也不退回「情感 = 角色档案」。
- **门禁不再中性。** 有情感计划时，情感探针失败是硬失败；没有计划但有最小链时，检查「触发事件/行动/代价」三件事；两者都没有时，才允许中性，并在契约里明确标记「本场无情感约束」。

需要避免的偏差：情感驱动不等于苦情戏。情感只在**情节合法的候选中**选择压力最大的事件；外部目标、赌注、因果链仍由章契约约束。不能让「为了让她哭」而让反派降智、让事件失去因果，或让每场都变成情绪戏。

### 4.4 契约生成与校验

#### 4.4.1 Grounding：事实优先于意图

优先级必须写死：

```
已发生正文事实 > 角色当前状态/知识边界/物品归属 > 章纲意图 > 故事大纲意图 > 方法论模板
```

- 章契约从**上一章正文的实际结尾**生成，不是从 `outline_content` 前 200 字生成。
- 故事大纲只作为方向约束；如果大纲里的角色/事件与正文冲突，不能像现在这样整段丢弃大纲，而要**保留方向、替换事实**：把大纲中未接地的名字/事件替换为正文中已存在的角色/事件。
- 章纲生成后必须做接地校验：章纲点名的角色必须在正文/角色表中存在；章纲的关键事件必须能从上一章结尾或未兑现承诺推出。校验失败 → 修复一次（`outline_replan` 可复用）→ 仍失败则降级为「最小契约」。
- **最小契约不是通用句。** 它必须仍然包含：具名 POV、具名反对者、具体赌注、一个状态变化。可以从 `character_goals`、`active_conflicts`、`tension`、`foreshadowing` 确定性推导；如果连这些都推导不出，宁可返回「资产不足，无法规划本场」并让用户补资产，也不要写通用句。

#### 4.4.2 章契约生成

- 每章 1 次 LLM 调用，输入：上一章末 3000 字、故事大纲、章纲、角色目标/伤口/需求、关系张力、活跃冲突、伏笔/债务、最近 3 场结果。
- 输出严格 JSON（复用 `writer_beat_plan.md` 的字段精神：goal / conflict_escalation / new_elements / character_moves / foreshadowing_ops / target_words）。
- 校验器（纯 Rust）检查：字段非空、具体性、具名反对者、赌注、转折是事件、不可逆结果、场景数 1–3、每场状态变化、情感链完整。
- 校验失败 → 一次修复调用（只修不合格字段，不重生成整章）→ 仍失败 → 最小契约。
- 章契约在章内**不变**，可以放在稳定前缀区，保护 prefix cache。

#### 4.4.3 场契约投影

- 章契约 → 场契约用纯 Rust 投影，不额外调用 LLM。
- 第 1 场从当前状态和章契约的 `want` 开始；第 N 场的 `entry_state` = 第 N-1 场的 `outcome`。
- 如果章契约只有 1 场，就把整章的转折/结果压到这一场；如果 3 场，按「目标场 → 反应场 → 决定场」或「加压 → 升级 → 结账」分配。
- 场契约的 `word_budget` 按总目标字数均分，允许 ±20%。

### 4.5 渲染：writer 只看到「一页简报」

当前 prompt 的问题不是信息少，而是**信息多但优先级不清**。设计后的渲染顺序：

```
[稳定前缀]
  世界观红线
  故事大纲（不注入下一节点）
  世界观设定
  Logline
  ChapterContract（章内不变）
[变动区]
  角色卡/名册/关系（只取本场相关）
  当前状态/知识边界/物品
  上一场结果
  SceneContract（本场简报，最高显著度）
  用户指令（编译进契约，不裸追加）
  本章手法（人类文笔基线，一条）
[末句锚点]
  已有正文末两句 + 硬锚点
```

**关键变化：**

- `SceneContract` 放在变动区末尾、末句锚点之前，是 writer 最后看到的完整指令。
- 章契约放在稳定区末尾，章内不变，保护 prefix cache。
- 资产块只保留本场相关事实，不再把全量角色卡/名册/关系/进度都塞进去。
- 用户指令不再裸追加；先编译成「本场指令」，与契约冲突时以契约为先，保留指令核心意图。

### 4.6 验证：用同一份契约核对

`ContractVerifier` 不是再打一个 1–5 总分，而是逐字段返回：

```rust
struct FieldResult {
    field: String,          // goal / opposition / turn / outcome / state_delta / emotion / continuity / hook
    status: Met | Partial | Miss,
    evidence: String,       // 必须引用草稿原文
    note: String,
}

struct ContractVerification {
    fields: Vec<FieldResult>,
    hard_failures: Vec<String>,   // 结构性失败
    soft_issues: Vec<String>,     // 文风/节奏/重复
}
```

**硬失败（必须修订）：**

- 本场没有具体目标，或目标与契约不符。
- 没有具名阻力，或阻力没有自己的目标。
- 没有转折事件，或转折只是情绪/气氛。
- 没有状态变化，或 `from == to`。
- 章末没有不可逆结果/新问题。
- 情感只有标签/身体反应，没有触发事件或代价。
- 与已死角色、知识边界、物品归属矛盾。

**软问题（记录/建议，不单独阻断）：**

- AI 腔、句长、对话标签、重复、节奏、文采。
- 钩子强度不足。
- 视角/语域轻微漂移。

**门禁判定：**

```
任何 hard_failure → RevisionRequired（定向修订）
否则 soft 分数低于阈值 → 记录质量债/建议，不阻断
否则 → Passed
```

编辑审计现有的 9 维评分保留，但**必须参与判定**：`stall`、`emotion`、`contract`、`hook` 任一 ≤ 2.0，即使 `verdict=pass` 也触发一次定向修订。这样 dimension_scores 不再只是落库数据。

### 4.7 修订：只改失败的场/段

当前 `build_revision_task` 只给一句「审查阻断问题：A；B；C」（`coordinator.rs:6996`），模型只能整章重写。设计改为：

```rust
fn build_contract_revision_task(
    contract: &SceneContract,
    failures: &[FieldResult],
    draft: &str,
) -> String
```

内容包含：

- 只改失败场/失败段落，保留已达标项。
- 每个失败字段给出：契约要求、草稿证据、必须变成什么。
- 修订后必须重新满足 `state_delta` 和 `emotion.cost`。
- 不得重写开头/结尾，不得增加新角色，不得改变已发生事实。

修订最多 1 轮；第 2 轮仍 hard failure → 标记质量债，并把失败项写入下一场契约的 `must_not` / `entry_state`，不再静默放行。

### 4.8 回流：下一场从「结果」开始，不从「计划」开始

每场落库时持久化 `SceneOutcome`：

```rust
struct SceneOutcome {
    scene_index: usize,
    outcome: String,
    state_delta: StateDelta,
    emotion_end: EmotionVector,
    open_question: String,
    promise_ops: Vec<PromiseOp>,
    contract_misses: Vec<String>,
}
```

- `progress_lines` 改为「最近 3 场结果」：`第N章第2场：沈砚拿到半本名册，但被王爷认出；关系从戒备→公开敌对`。
- `next_node` 不再从故事大纲里字符串匹配，而是下一场 `SceneContract`；章内场次用完后，生成下一章契约。
- 承诺账本记录「埋设/推进/兑现」；下一章契约必须消费至少一条旧承诺，或明确说明为什么暂不兑现。

---

## 5. 提示词设计

### 5.1 系统提示词：8 条，不堆规则

单拍主路径的 `CONTINUE_BEAT_SYSTEM` 应替换为下面 8 条（保留现有的人类文笔基线和 Wrong/Right 示例，但压缩）：

```
你是小说主创。你写的是「一场戏」，不是「一段续写」。只输出正文。

1. 每场必须有：谁想要什么 → 谁/什么阻止 → 他怎么做 → 什么事件让计划失效 → 结果改变了什么。
2. 每场至少改变信息/关系/目标/风险/情绪之一；本章至少一次不可逆结果。没有变化就不要写。
3. 冲突必须是具体的人或势力与具体目标对撞；禁止用「众人反对」「命运」「气氛」代替阻力。
4. 情感必须由事件触发，落到行动或选择，并产生代价；禁止只写情绪标签或身体反应。
5. 本场结果必须成为下一场的前提；禁止靠巧合、天降信息、突然转变推进。
6. 不写过渡场、总结场、全员表态场；不解释主题，不写「她终于明白」。
7. 连续性：人设/世界观/伏笔/知识边界以资产为准；已死角色退场。
8. 文笔：情绪四模式混用，行为为主；句长参差；对话标签重复「说」即可。
```

这 8 条与现有 `CONTINUE_BEAT_SYSTEM` 的重合部分（不重复、不发明人、兑现必须改变、文笔）保留；新增的是 1–5 的「场景引擎」。不要把 `writer_system.md` 的 58 行全部塞进 system；长规则会稀释契约。

### 5.2 本场简报：writer 真正执行的东西

```markdown
【本场简报｜第 2 场 / 共 3 场】

场前状态：沈砚手里只有半本名册；王爷已知她在查账；两人表面仍以叔侄相称。

本场目标：沈砚要在宴席散场前让王爷当众承认名册的来历。
阻力：王爷要保住名册，同时不能让宾客知道账目有问题。
阻力目标：把沈砚的注意力引到侧厅，拖到散场。
手段：沈砚先敬酒，再当众问起三年前那笔军饷。
转折：王爷不接话，反而把名册扔进火盆——沈砚必须选“救名册”还是“救证人”。
必须结果：名册烧毁一半，证人被王爷带走；沈砚与王爷的关系从“表面叔侄”变为“公开敌对”。不可逆。
情感链：事件=名册被烧；评估=失去证据、被当众压住；情绪=恐惧→愤怒；行动=徒手抢火盆；代价=烫伤，且证人被带走。
禁止：不要让王爷突然良心发现；不要让第三人替沈砚抢回名册；不要写全员震惊。
结尾钩子：证人被带走前说“你母亲也签过”。
字数：1200–1600。
```

这份简报只有 200–300 字，但它比当前「阵容/冲突/情感/推进」四行更能决定一场戏。

### 5.3 选择，而不是堆叠

设计原则：**每场只给一个契约 + 一个简报 + 一条手法。**

- 不再同时注入 `writer_system.md`、`agency_writer_system.md`、`CONTINUE_BEAT_SYSTEM`、节拍卡、情感块、状态网、人物锁、人类文笔基线、末句锚点、事实块——这些内容会互相竞争注意力。
- 人类文笔基线只保留当前轮换的 1 条手法 + 7 条固定准则的精简版；不要每拍把全部手法列出。
- 参考场景 few-shot 只在检测到风格漂移时注入 1–2 段，不作为常驻上下文。
- 检索块只注入「与本场契约相关」的 1–2 段，不注入整章。

### 5.4 现有提示词资产的去向

| 现有资产 | 新位置 |
|---|---|
| `methodology_scene_structure` | 系统提示词第 1 条 + 场契约的 goal/obstacle/turn/outcome |
| `writer_beat_plan` | 章契约/场契约的字段 schema（goal、conflict_escalation、new_elements、character_moves、foreshadowing_ops、target_words） |
| `writer_reading_power_goal` / `writer_chase_debt` | 章契约的 `promise_ops` + 场契约的 `hook_out` |
| `writer_contract_constraints` | 稳定前缀的「硬约束」层 |
| `narrative_quartet` | 章契约的节奏/幕结构（可选） |
| `style_dna_extension` / `human_voice` | 文风层（保留现状，不再扩规则） |
| `reference_scene_fewshots` | 条件注入的 few-shot（仅风格漂移时） |
| `outline_replan` | 章契约修复入口 |
| `emotion_circuit` | 场契约的 `EmotionChain`（一等字段，不再是可选附加块）；默认启用最小链，完整 OCC/载体/读者面板仍可作者配置 |
| `continuity_blocks` / 检索 | `entry_state` 事实层，不作为指令 |
| `gate` / `graders` | 契约核对 + 现有 code/style 检查 |

---

## 5.5 人物命名系统：三层九维姓名编译法

> 命名不是「让模型想几个好听的名字」，而是一次小型世界构建。模型擅长写意义，不擅长采样真实姓名分布；把取名交给自由生成，必然塌缩到同一小撮「小说感」用字。

### 5.5.1 为什么现在的名字会塌缩

当前创世提示词里有三条互相放大的规则（`resources/prompts/creation/novel_creation_character_roster.md`，`src-tauri/src/story_system/auto_contract.rs:868` 重复了同一段）：

```
禁止使用林、陈、王、李、张、刘等最常见单字姓；禁止单字名；
同一组角色姓氏不得重复
```

这三条看似在「去套路」，实际效果相反：

1. **把真实姓氏分布拿掉了。** 王、李、张、刘、陈是真实世界里最常见的姓；禁止它们，等于把「普通人」赶出故事，只留下林、苏、顾、沈、陆、江、秦、楚、叶、白、温、宋、裴、谢、萧这一小撮「小说姓」。模型于是把「小说姓」当成正确答案。
2. **把合法的二字全名误杀了。** 李白、杜甫、林雪、沈夜都是真实存在的姓名形态。禁止单字名，等于强制所有名字变成三字「优雅名」，反而更像网文模板。
3. **没有生成后的校验。** `parse_character_roster_response`（`src-tauri/src/agents/novel_creation.rs:163`）只解析 JSON，不检查重名、共用字、部首、声调、时代感、陈词用字。模型重复用「雪/夜/墨/寒/云/月/辰/宇/轩/然/若/离/歌」也不会被拦。

**结论：** 不是模型「不会取名」，是提示词把真实分布删掉、又没给替代分布。新命名法必须提供**正面分布 + 姓名账本 + 硬校验**，而不是又一份黑名单。

### 5.5.2 三层九维

| 层 | 维度 | 要决定什么 | 示例 |
|---|---|---|---|
| 世界层 | 时代 | 命名制度、避讳、常用字、是否有字/号 | 1980 年代 vs 明代 vs 架空 |
| | 地域 | 姓氏分布、方言小名、移民/少数民族 | 江南、闽南、西北、边镇 |
| | 阶层 | 教育、职业、家族资源、识字率 | 士绅、商户、佃农、军户 |
| 家族层 | 姓氏 | 家族姓氏、聚居、同姓比例、复姓比例 | 周家、李家、欧阳家 |
| | 辈分 | 字辈诗、兄弟姐妹共用字、排行 | 「守正承先志」 |
| | 亲缘 | 父系/母系、入赘、收养、改姓、随母姓 | 随母姓、逃难改姓 |
| 个人层 | 音形 | 声调轮廓、声母/韵母、部首、字形复杂度 | 避免全员氵/木/王 |
| | 语义 | 期望、纪念、经典、宗教、时代、方言 | 建国、念慈、守拙、阿禾 |
| | 称呼 | 本名、字、号、小名、绰号、化名、职称 | 公开称呼 vs 私下称呼 |

**术语先统一：**

- **二字全名**：姓 + 单字名（李白、林雪）。
- **三字全名**：单姓 + 双字名（张伟明），或复姓 + 单字名（欧阳策）。
- **四字全名**：复姓 + 双字名（欧阳修远）。

模型化的典型不是「二字」本身，而是**二字全名 + 冷色/夜色/墨色用字 + 小说姓**的高度同质组合。真实姓名需要长度、姓氏、语义、阶层都有分布。

### 5.5.3 生成流程：先有世界和家族，再有个人的名字

```
NameWorld（时代/地域/阶层/语域/避讳/复姓比例）
   │
   ▼
FamilyLineage × N（姓氏 + 字辈诗 + 阶层/地域 + 命名风格）
   │
   ▼
CharacterSlot × M（角色功能/性别/年龄/阶层/地域/家族）
   │
   ▼
候选名生成（模板 + 受控字池；LLM 只写取名动机/典故，不自由起名）
   │
   ▼
NameScorer（音形/语义/分布/时代/阶层/陈词/重名/名人）
   │
   ▼
配额选择（长度/姓氏/用字/部首/声调/语义域）
   │
   ▼
社会称呼图（本名/字/号/小名/绰号/化名/职称/禁忌称呼/使用规则）
   │
   ▼
NameLedger 落库 → 场简报只注入本场称呼表
```

**关键原则：不要让 LLM 从零发明姓名。** 让编译器从受控字池和分布里生成候选，LLM 只负责两件事：

1. 为候选名写一句「取名故事」（谁取的、为什么、人物自己喜不喜欢）；
2. 在候选名里选一个最符合人物/家族/时代的，并说明理由。

这样既保留语义和人情味，又不会回到模型默认的那一小撮字。

**Naming Brief（给 LLM 的输入，不是自由起名）：**

```markdown
【命名任务】
时代：1990 年代江南小城
阶层：国营厂职工家庭
家族：周姓，字辈「守正承先志」
角色：男，32 岁，厂办干事，性格谨慎，有未说出口的野心

【编译器候选（已通过硬约束，不要新增候选）】
1. 周守拙（字慎之）：父亲按族谱取名，希望他藏锋；他嫌「拙」字晦气。
2. 周守正（无字）：堂兄，名字取「守正不阿」，与主角形成对照。
3. 周承志（无字）：同族晚辈，名字承字辈，代表家族期待。

请选一个，并写一句「取名故事」（谁取的、为什么、人物自己喜不喜欢）。
不要新增候选名，不要改姓，不要用「林/苏/顾/沈/墨/雪/夜」类小说默认字。
```

**名字要有来历。** 例：周守拙的父亲是小学教员，按族谱「守正承先」给他取名「守拙」，希望他别学自己投机；他后来偷偷把「拙」改成「卓」，被父亲打了一顿。一个名字就能带出父子关系、时代气质和人物欲望。这比「沈墨，性格冷漠」有效得多。

**核心数据结构（设计级）：**

```rust
struct NameWorld {
    era: Era,                       // 年代/朝代/架空纪元
    region: Region,                 // 地域/方言区
    register: LanguageRegister,     // 文言/白话/方言/现代/译名
    class_spectrum: Vec<SocialClass>,
    compound_surname_ratio: f32,    // 复姓配额
    taboo: Vec<String>,             // 避讳/禁用词
    foreign_influence: f32,         // 音译/外来名比例
}

struct FamilyLineage {
    surname: String,
    generation_poem: Option<String>,// 字辈诗
    class: SocialClass,
    region: Region,
    style: NameStyle,               // 质朴/书香/商贾/军户/宗教
    members: Vec<String>,
}

struct NameScorer {
    // 归一化后加权；权重可按题材配置
    era_fit: f32,                   // 0.25
    class_fit: f32,                 // 0.20
    phonetic_distinctness: f32,     // 0.15
    orthographic_distinctness: f32, // 0.15
    semantic_fit: f32,              // 0.10
    family_consistency: f32,        // 0.10
    rarity_fit: f32,                // 0.05
    cliche_penalty: f32,            // 加权扣分
    famous_name_penalty: f32,       // 完全同名重罚
    homophone_penalty: f32,         // 与现有角色同/近音重罚
}
```

选择时不是「取最高分」，而是**先过硬约束，再在合格候选里按分数和配额选**。这样不会为了分数牺牲可读性、家族一致性或题材分布。

### 5.5.4 六条硬约束（直接对抗模型塌缩）

**C1：姓氏按真实分布采样，不按「小说感」挑选。**

- 现代/现实题材：从真实姓氏频率表采样，前 100 常见姓必须占主要角色的一定比例（建议 40–70%），复姓占比 ≤ 3%。
- 历史题材：按朝代/地域的姓氏分布采样；复姓可以有，但主要角色中不超过 15–20%，避免全员上官/慕容/欧阳。
- 架空/玄幻：可以自造姓氏，但必须有构词规则（单字姓 + 部族/地域词根），且复姓/异域姓氏总量不超过 25%。
- 同一家族可以同姓；非家族主要角色同姓不超过 2 人。

**C2：名字长度按题材配额，不禁止二字全名。**

| 题材 | 二字全名 | 三字全名 | 四字全名 |
|---|---|---|---|
| 现代/现实 | 20–30% | 65–75% | ≤ 5%（复姓） |
| 历史/年代 | 10–20% | 60–70% | 15–25%（复姓/字辈） |
| 玄幻/仙侠 | 15–25% | 55–65% | 15–25% |
| 科幻/未来 | 15–30% | 55–65% | 5–15%（含代号/音译） |

配额是分布，不是每本书必须完全命中；关键是**不要整本书一个长度**。

**C3：主要角色不得共用名字用字。**

- 主要角色（主角、反派、核心配角）的给定名用字不得重复；全角色表同一用字最多出现 2 次（字辈字除外，但字辈字只用于同族）。
- 不得出现「林雪 / 沈雪 / 苏雪」这类同字换姓。
- 同一家族兄弟姐妹可以共用辈分字，这是真实感来源，不是重复。

**C4：音形必须拉开距离。**

- 声调轮廓：同一轮廓（如全去声、全阳平）在主要角色中不超过 30%。
- 部首：主要角色给定名同一部首不超过 2 人（防止全员氵/木/王/日/月）。
- 声母/韵母：避免主要角色姓名首字同声母、末字同韵母扎堆。
- 同音/近音检查：用不带声调拼音比对；避免「张伟/章伟」「李娜/黎娜」这类对话中分不清的名字。
- 可读性：不用生僻到 TTS/配音读不出的字；不用需要注音的生字来「显独特」。

**C5：语义域配额，防止全员冷色/诗意。**

把给定名的语义分成六个域：

```
期望（健康/长寿/富贵/平安/光耀）
纪念（出生地/事件/祖先/恩人）
时代（建国/援朝/跃进/改革/开放/奥运）
经典（诗经/楚辞/论语/史书/诗词典故）
宗教/信仰（佛/道/民间信仰）
日常/方言（阿禾/二丫/小满/石头/招娣）
```

主要角色中，任何单一语义域不超过 30%；「冷色/夜色/墨色/云月雪霜」这类模型偏好域必须单独统计并压到 20% 以下。不要所有名字都有深意——普通人名、时代名、方言小名本身就是真实感。

**C6：陈词与名人检查。**

- 建立加权「模型陈词表」，不是按单字封杀（雪/月/夜本身是正常字），而是按**组合与聚集**扣分：林雪、沈夜、苏墨、顾寒、叶云、秦月、白汐、萧离、慕容雪、上官墨这类组合进入高扣分。
- 主要角色陈词命中 = 0；全角色表陈词命中率 ≤ 10%。
- 避免与著名历史/文学人物完全同名（李白、杜甫、苏轼、鲁迅、林黛玉等）；如剧情需要，必须显式标记「同名梗」并给出叙事理由。
- 避免名字与常见词/贬义词/品牌同音到出戏。

### 5.5.5 社会称呼图：名字不是只有一个

真实人物在不同关系里有不同叫法。模型常犯的错是所有人对所有角色都用全名。命名系统必须为每个主要角色生成一张称呼表：

```rust
struct CharacterName {
    character_id: String,
    formal: String,              // 本名
    surname: String,
    generation: Option<char>,    // 辈分字
    given: String,
    courtesy: Option<String>,    // 字（历史/古风）
    art: Option<String>,         // 号
    childhood: Option<String>,   // 小名/乳名
    nickname: Option<String>,    // 绰号/江湖名
    aliases: Vec<String>,        // 化名/代号
    titles: Vec<String>,         // 职称/爵位/尊称
    taboo_address: Vec<String>,  // 禁忌称呼（叫了会翻脸）
    address_rules: Vec<AddressRule>,
    name_story: String,          // 谁取的、为什么、人物自己喜不喜欢
}

struct AddressRule {
    speaker: String,
    addressee: String,
    public_form: String,         // 公开场合怎么叫
    private_form: String,        // 私下怎么叫
    hostile_form: Option<String>,// 敌对/威胁时怎么叫
    note: String,                // 关系潜台词
}
```

场简报只注入本场相关的 3–6 条，例如：

| 说话人 | 对象 | 公开 | 私下 | 潜台词 |
|---|---|---|---|---|
| 王爷 | 沈砚 | 沈姑娘 | 砚儿 | 表面叔侄，实为监视 |
| 沈砚 | 王爷 | 叔父 | 老狐狸（心里） | 从不出口 |
| 丫鬟 | 沈砚 | 小姐 | 姑娘 | 亲近但守规矩 |

这比在角色卡上写「姓名：沈砚」有用得多：称呼本身就是关系、阶层和潜台词。

### 5.5.6 与 NCC / 现有代码的接线

| 现有位置 | 改动方向 |
|---|---|
| `novel_creation_character_roster` / `auto_contract.rs` 的取名段 | 删除「禁止常见姓/禁止单字名」；改为由姓名编译器提供候选池，LLM 只填性格/背景/情感与取名故事 |
| `src-tauri/src/agents/novel_creation.rs:163` 解析 | 增加姓名账本校验：重名、共用字、长度配额、陈词、同音、时代/阶层 |
| `CharacterRepository::create` / `update` | 新角色必须从 `NameLedger` 取规范名；没有槽位则先走命名编译器 |
| `character_aliases` / `character_identity` | 存字/号/小名/绰号/化名；`NameLedger.canonical` 是规范名真源 |
| `continue_assets::aliases_for` | 保留为兜底；称呼表优先于「阿末字/末两字」启发式 |
| `render_one_card` / 场简报 | 增加「本场称呼」一行，不再只给全名 |
| `SceneContract` cast | 增加 `address_in_scene`（本场谁怎么叫谁） |
| `ContractVerifier` | 检查：不得新编姓名、不得用错称呼、不得让死人说话、不得同场混淆同音名 |
| `asset_refresh` / `ingest` | 新姓名必须能通过账本解析；未登记的新规范名不得进大纲/角色表 |
| `prose_ground` | 用账本别名替代纯启发式别名做「姓名是否在正文中出现」的判定 |

**数据资产（新增 `resources/naming/`）：**

- `surnames.csv`：姓氏、频率、时代/地域标签、复姓标记；
- `given_chars.csv`：字、拼音、声调、频率、语义域、性别倾向、时代标签；
- `cliche_weights.csv`：模型陈词组合权重（按组合/聚集扣分，不按单字封杀）；
- `famous_names.txt`：著名历史/文学人物完全同名表；
- `generation_poems.csv`：常见字辈诗/字辈字；
- `address_forms.csv`：亲属、职称、爵位、江湖称呼模板。

**生成时机：** 创世时生成整个主要角色命名账本；新增角色时增量生成；续写每拍只读账本 + 本场称呼表，**不再让 writer 临时起名**。这直接消灭「续写突然冒出一个新名字」的模型习惯。

### 5.5.7 兼容性与验收

**兼容性：**

- 新故事：强制走姓名编译器；主要角色必须通过校验才能落库。
- 旧故事：从现有 `characters` + `character_aliases` 反向构建账本；**不自动改名**（避免破坏已写正文和 v0.73.14 的改名去重逻辑）；只对新增角色生效。
- 旧故事的「陈词名」只给建议，不强制；作者确认后才改名，并走现有 `record_aliases` 记录旧名。
- 用户手写角色名：允许保留，但必须登记进账本并给出别名/称呼规则；校验器只拦截「与现有角色重名/同音/共用字」这类硬冲突。

**验收指标：**

| 指标 | 目标 | 测量 |
|---|---|---|
| 主要角色给定名共用字 | 0 | 账本校验 |
| 主要角色非家族同姓 | ≤ 2 人 | 账本校验 |
| 主要角色陈词组合命中 | 0 | 加权陈词表 |
| 二字/三字/四字全名分布 | 落在题材配额带内 | 账本统计 |
| 主要角色同声调轮廓占比 | ≤ 30% | 音形签名 |
| 主要角色同部首占比 | ≤ 2 人 | 音形签名 |
| 单一语义域占比 | ≤ 30% | 语义域统计 |
| 本场称呼规则覆盖率 | 100% | 场简报/契约核对 |
| 续写新编姓名率 | 0 | `ContractVerifier` |
| 人类评价「像真人/像这个时代」 | ≥ 4/5 | 30 角色盲评 |

**反例（模型默认池）：**

> 林雪、沈夜、苏墨、顾寒、叶云、秦月、白汐、萧离

问题不是这些字不能用，而是**同一本书里它们同时出现**：全是二字全名、全是冷色/夜色/墨色/云月雪霜、全是小说姓。这是分布塌缩，不是单个名字的错。

**正例（按时代/阶层分层）：**

> 现代：王秀兰（1960s）、李建国（1970s）、张伟（1980s）、陈雨桐（1990s）、周子默（2000s）
> 历史：周守拙（字慎之，士绅家）、沈令仪（闺名，商户女）、李二（佃户）、欧阳策（寒门，复姓）
> 架空：石敢当（民间名）、阿禾（小名）、庾守一（道门辈分名）、贺兰石（部族地名转姓）

这些名字不追求「好听」，而追求**可归因**：时代、地域、阶层、家族、取名人都能解释它。名字有来历，人物就有生活。

## 6. 多作品切换一致性：选中即切换

> 这是与叙事质量并列的独立问题：用户在幕后点另一部小说，幕前必须切换到那部小说，不能因为「正在生成」就静默留在旧作品。修复不是删掉一个守卫，而是把「选择—保存—取消/隔离—加载—应用」做成一个可取消、可恢复、可测试的状态机。

### 6.1 症状与复现

- **复现 A（用户报告）**：幕前正在续写作品 A → 幕后故事列表点击作品 B → 幕前仍显示 A 的章节，只弹「正在生成，完成后请再切换故事」。
- **复现 B**：幕前未生成时点击 B，通常能切换；但快速点 A→B→C 或幕前窗口隐藏/重载时，可能停在 B、或仍显示 A 的正文。
- **复现 C**：切换成功后，旧作品的幽灵正文/生成结果仍可能追加到新作品编辑器，造成串稿。

### 6.2 根因（代码核对）

1. **显式守卫直接拒绝切换。** `FrontstageApp.tsx:633` 的 `onStorySelected` 在 `isGeneratingRef.current || isGenesisSettingUpRef.current` 为真时执行 `toast('正在生成，完成后请再切换故事')` 并 `return`（`FrontstageApp.tsx:635-638`）。这是「写作中不切换」的直接原因。
2. **选择状态双源。** 幕后用 `useAppStore.currentStory`（`src-frontend/src/stores/appStore.ts:105`），幕前用局部 `useState<Story | null>`（`FrontstageApp.tsx:250`）；两者只靠一次性的 `story-selected` 事件同步，没有持久化的 `active_story_id`。事件被守卫吞掉、窗口未就绪或监听尚未注册时，两边永久分叉。
3. **切换前没有 flush。** `selectStory`（`FrontstageApp.tsx:3108`）先 `setCurrentStory(story)` 再拉章节；没有 `await flushSceneSave()`（`useScenePersistence.ts:217`）。旧故事最后 200ms 的输入、防抖保存可能丢失。
4. **在途生成没有按故事隔离。** 当前守卫只是「不切换」，不是取消。若简单删除守卫，旧生成的 `ContentUpdate` / `AppendContent` / `ChapterSwitch` / `smart_execute` 结果仍可能写进新故事编辑器。`handleCancelGeneration`（`FrontstageApp.tsx:4368`）调用的是全局 `agent_cancel_all_tasks`，不是「只取消当前故事的前台生成」。
5. **事件没有序号与去重。** `useSyncStore` 的 `onStorySelected`（`src-frontend/src/hooks/useSyncStore.ts:176`）只传 `story_id/title`。快速 A→B→C 会并发执行多个 `selectStory`，后返回的旧请求可能覆盖新选择。
6. **切换后的状态清理不完整。** `selectStory` 设置了 chapters/scenes/content，但 `generatedText`、幽灵锁、`postAcceptLock`、打字机、流式缓冲、输入历史、AI hints、字数基准等没有统一清理；旧故事残留会污染新故事。
7. **焦点/可见性没有补偿。** 幕前窗口隐藏、重载或事件丢失后，重新聚焦不会重新读取「当前选中故事」，只能靠用户再点一次。

### 6.3 行为契约（必须满足）

| 编号 | 契约 |
|---|---|
| B1 | **选中即切换。** 幕后任何「选中故事」入口（卡片点击、打开按钮、侧栏）都必须让幕前最终显示该故事；生成中也不得静默忽略。 |
| B2 | **先保存后切换。** 切换前 flush 旧故事当前场景；flush 失败则中止切换并明确报错，不能丢稿。 |
| B3 | **在途生成必须有明确处置。** 前台单拍/流式生成 → 取消；后台批量/创世 → 可继续在旧故事后台完成，但幕前不得把其事件应用到新故事。 |
| B4 | **原子应用。** 新故事的 story/chapters/scenes/currentChapter/currentScene/content 必须一次性应用；旧故事的生成态、幽灵锁、流式缓冲、选择 ref 必须清空。 |
| B5 | **最后写入胜出。** selection seq 单调；过期异步结果丢弃；快速连点最终停在最后一次选择。 |
| B6 | **可恢复。** 幕前隐藏/重载/事件丢失后，重新聚焦或挂载时读取持久化的 active story 并切换。 |
| B7 | **可回退。** 目标故事被删除、无章节或加载失败时，保持旧故事或进入安全空态，并给明确提示；不能显示旧故事内容冒充新故事。 |

### 6.4 设计：StorySwitchController

#### 6.4.1 状态机

```
Idle
  │ 收到 selection（事件/用户操作）
  ▼
Flushing（flush 旧故事）
  │ 失败 → Abort（保留旧故事 + 错误提示）
  ▼
Disposing（取消/分离在途生成）
  │ 失败 → Abort 或 Detach（按生成类型）
  ▼
Loading（拉 stories/chapters/scenes）
  │ 失败 → SafeFallback（旧故事或空态）
  ▼
Applying（原子替换 + 清理旧状态）
  │ seq 过期 → 丢弃
  ▼
Active
```

#### 6.4.2 选择序号与持久化

新增一个轻量的「当前选中故事」真源：

- 后端命令 `set_active_story(story_id)`：持久化 `active_story_id`（可复用现有 app state/settings 表，不新增复杂 schema），并发出 `story-selected` 事件，带持久化的单调递增 `seq`（或 `updated_at` 毫秒，跨重启不回退）与 `origin`。
- 后端命令 `get_active_story()`：供幕前挂载/聚焦时恢复。
- 事件 payload：`{ story_id, title, seq, origin: "backstage_card" | "backstage_open" | "restore" }`。
- 幕后 `handleSelectStory`（`src-frontend/src/pages/Stories.tsx:469`）与 `open_story_in_frontstage`（`src-tauri/src/commands/sync.rs:56`、`:72`）统一走 `set_active_story`，不再各自发裸事件。
- 幕前 `useSyncStore.onStorySelected` 把 `seq` 传给 `StorySwitchController`；`seq <= lastAppliedSeq` 直接丢弃；同一故事 300ms 内重复事件去重。

**`isGenerating` 只决定「怎么处置在途生成」，不决定「是否切换」。** 这是本次修复与旧行为的分水岭：旧代码把生成态当成了切换的否决票；新设计把它当成状态机里的一个分支。

#### 6.4.3 切换流程

```ts
async function switchStory(req: { storyId: string; seq: number; origin: string }) {
  if (req.storyId === currentStoryRef.current?.id) {
    await selectLatestChapterWithoutClearing(); // 同一故事只重新定位最新章节
    return;
  }

  const switchSeq = ++switchSeqRef.current;
  setSwitching(true);

  // 1) 先保存旧故事
  try {
    await flushSceneSaveRef.current();
  } catch (e) {
    if (switchSeq === switchSeqRef.current) {
      toast.error('当前章节保存失败，已取消切换');
      setSwitching(false);
    }
    return;
  }

  // 2) 处置在途生成
  const oldStoryId = currentStoryRef.current?.id;
  await disposeGenerationForSwitch(oldStoryId, req.storyId);

  // 3) 加载新故事
  let bundle;
  try {
    bundle = await loadStoryBundle(req.storyId); // stories + chapters + scenes + latest chapter
  } catch (e) {
    if (switchSeq === switchSeqRef.current) {
      toast.error('目标作品加载失败，已保留当前作品');
      setSwitching(false);
    }
    return;
  }

  // 4) 过期丢弃
  if (switchSeq !== switchSeqRef.current) return;

  // 5) 原子应用 + 清理旧状态
  applyStoryBundle(bundle);
  resetStoryScopedUiState();
  setSwitching(false);
}
```

`disposeGenerationForSwitch` 的规则：

| 当前生成类型 | 处置 | 理由 |
|---|---|---|
| 前台单拍/流式续写 | **取消**：`generationEpoch++`，abort 前端流，调用 `llm_cancel_generation`（有 request_id）或 `agency_cancel_run`（有 run_id）；等待 ≤2s 后继续 | 结果只能写进旧场景，不能跨故事追加 |
| 后台批量续写 | **分离**：不取消，标记 `detached`，让其继续写旧故事；幕前用 `storyEpoch` 过滤其事件 | 长任务取消代价高，且它本来就有自己的故事落库 |
| 创世装配中 | 若目标是正在创世的新故事 → 等待创世完成再切换；否则分离/取消旧创世 | 避免把未完成的故事当目标 |
| 无在途生成 | 直接进入加载 | — |

**关键：不要用全局 `llm_cancel_all_generations` 处理普通切换。** 它会把后台批量任务一起停掉；优先按 `run_id` / `request_id` 定向取消，只有拿不到 id 时才回退全局取消并提示「所有后台生成已停止」。

#### 6.4.4 事件隔离

所有异步回调在应用前检查两件事：

```ts
if (eventStoryId !== currentStoryRef.current?.id) return; // 不跨故事
if (eventStoryId === currentStoryRef.current?.id && generationEpoch !== currentEpoch) return; // 旧生成
```

覆盖：`ContentUpdate`、`AppendContent`、`ChapterSwitch`、`pipeline-complete`、`smart_execute` final、流式 chunk、`scene-created/updated`、自动分章事件。`ChapterSwitch` 里已有的 story 分支（`FrontstageApp.tsx:2222`）应复用同一个 `switchStory`，不要维护第二套切故事逻辑。

#### 6.4.5 状态清理清单

切换应用时必须清空/重置：

- `currentStory`、`chapters`、`scenes`、`currentChapter`、`currentScene`
- `useFrontstageStore.content`、`sceneId`、`chapterId`、`sceneTitle`、`storyTitle`、`isSaved`
- `latestContentRef`、`editorRef` 内容、`generatedText`、`hideGhostUntil`、`postAcceptLock`
- `genesisDeliveryRef`、`isGenesisSettingUpRef`、`smartExecuteInFlightRef`
- `typewriterFrameRef`、流式缓冲、`recentlyAppendedRef`
- `inputHistory`（按故事重新加载）、`aiHints`、`saveError`、字数基准
- `lazyLoadingChapterIdsRef`、`chapterSwitchTimestampRef`、`postChapterSwitch` 静默期
- `switchSeqRef` 仅在应用成功后更新；失败不更新，允许重试

#### 6.4.6 幕前恢复与焦点补偿

- **挂载时**：先 `get_active_story()`；有值则直接加载该故事，不再走「默认第一个故事」。无值才回退 `stories[0]`。
- **窗口聚焦/可见性变化**：`frontstageFocused` / `visibilitychange` 时重新读取 `active_story_id`；若与当前不同且没有在途 switch，执行 `switchStory({origin:'restore'})`。
- **窗口未加载时**：事件可以在幕前启动后再补一次恢复，不依赖实时监听是否已注册。
- **故事被删除**：`active_story_id` 指向不存在的故事时，清空选择并进入空态，提示「该作品已删除」。

### 6.5 边界情况

| 场景 | 期望行为 |
|---|---|
| 同一故事点击 | 只重新定位最新章节；不 flush、不清空编辑器、不取消生成 |
| 目标故事无章节 | 切换成功，清空编辑器，显示空态，不保留旧正文 |
| 目标故事被删除 | 保留旧故事或空态 + 明确提示；不显示旧正文冒充 |
| 快速 A→B→C | 只有 C 应用；A/B 的异步加载结果丢弃 |
| 切换时 autosave 失败 | 中止切换，保留旧故事，显示「保存失败，重试」 |
| 切换时创世装配中 | 目标是新故事则等待；否则分离/取消旧创世 |
| 切换时后台批量运行 | 默认分离，旧故事后台继续；幕前不接收其事件 |
| 幕前窗口隐藏 | 选中事件持久化；聚焦时恢复 |
| 切换后旧生成完成 | 只更新旧故事；若用户已切回旧故事，再按正常事件刷新 |
| 卡片点击 + 打开按钮同时触发 | 同故事 300ms 去重，只执行一次 |

### 6.6 测试与验收

**组件/单测：**

- `onStorySelected` 在 `isGenerating=true` 时不再 toast-return，而是进入 switch 流程。
- 切换前 `flushSceneSave` 被 await；flush 失败时 `currentStory` 不变。
- 切换时调用定向取消（有 run_id/request_id），不无条件调用全局取消。
- 过期 `selectStory` 结果不应用（seq 测试）。
- 同一故事点击不触发 flush/取消。
- 切换后旧 `generatedText`、幽灵锁、流式缓冲全部清空。

**E2E：**

1. 两部作品 A/B；A 正在续写；幕后点击 B；幕前显示 B 的最新章；A 的生成结果不进入 B；A 的正文已保存。
2. A→B→C 快速点击；最终停在 C。
3. 幕前隐藏时选择 B；聚焦后显示 B。
4. 目标故事无章节；编辑器为空态，不残留 A 的正文。
5. 目标故事被删除；给出提示，不显示 A 正文冒充 B。
6. 自动保存防抖中切换；A 的最后输入已落库。

**验收指标：**

| 指标 | 目标 |
|---|---|
| 目标存在时切换成功率 | 100%（含生成中） |
| 跨故事写入/串稿 | 0 |
| 切换导致的数据丢失 | 0 |
| 切换后旧状态残留 | 0（清单逐项断言） |
| 切换 UI 反馈 P95 | ≤ 500ms 显示目标故事壳；≤ 1.5s 显示正文 |
| 事件丢失后的恢复率 | 100%（聚焦/挂载恢复） |
| 快速连点最终选择正确率 | 100% |

### 6.7 本节决策

切换相关的 4 个决策（D10–D13）并入 §11 总表，避免与全局决策表重复。

## 7. 质检与门禁设计

### 7.1 结构硬失败与软问题

见 §4.6。硬失败必须修订；软问题只记录。这样既不让「文笔小瑕疵」拖慢创作，也不让「没有冲突/没有变化/没有情感因果」蒙混过关。

### 7.2 让 dimension_scores 真正参与判定

当前 `dimension_scores` 只落库（`coordinator.rs:7664` 附近）。设计改为：

- `stall`、`emotion`、`contract`、`hook` 任一 ≤ 2.0 → 触发定向修订。
- 其他维度 ≤ 2.0 → 记录质量债，不阻断。
- 总分仍用于报告与趋势，不作为唯一放行依据。

### 7.3 修订任务

见 §4.7。修订必须带契约、证据、目标状态，不再是一串模糊问题。

### 7.4 幕前 Append 的闭环边界

幕前 Append 不能为了质量把等待时间拉长到不可接受。设计边界：

- **同步（<100ms）：** 确定性契约探针（具名阻力是否出现、计划事件关键词、状态变化关键词、情感触发/代价关键词、重复率）。硬失败 → 一次快速定向修订（同一模型，短 prompt，只改失败段）。
- **异步（后台）：** 编辑审计的软问题、文风、钩子强度；结果进质量债/下一场 `must_not`，不阻断本次上屏。
- **跨拍：** 本场 hard failure 若未修好，写入下一场 `entry_state` 与 `must_not`，下一场必须处理，不能遗忘。

这样既保留幕前的流畅，又不让结构问题无限累积。

---

## 8. 兼容性与灰度

### 8.1 默认策略

- **新故事：** 默认启用 NCC（章契约 + 场契约 + 契约核对）。
- **旧故事：** 默认启用「兼容模式」：能生成契约就生成；资产不足时用最小契约（具名 POV/反对者/赌注/状态变化），不写通用句。完整情感回路仍可按作者配置启用。
- **已有提示词覆盖：** `prompts` 注册表中的用户覆盖继续生效；NCC 使用同一批 prompt id，不另起一套不可见的提示词。

### 8.2 无破坏性迁移

- 不新增必填 DB 列；契约可从现有资产 + 正文即时编译。
- `SceneOutcome` 可以先写进现有 `scenes.outline_content` 的结构化块，或新增 JSON 列；具体存储方案在实施阶段定。
- 旧故事不强制回填历史契约；从下一次续写开始生效。

### 8.3 前缀缓存

- 稳定区（红线/故事大纲/世界观/Logline/章契约）章内不变。
- 场契约每拍变化，放在变动区末尾。
- 末句锚点仍在最后，保持现有「从末句继续」的硬约束。
- 不在稳定区注入下一节点、当前地点、在场角色等每拍变化的字段。

### 8.4 成本与延迟

- 章契约：每章 1 次 LLM 调用（可用后台档/低成本档）。
- 场契约投影：0 次 LLM。
- 契约核对：复用编辑审计调用；确定性探针 0 次 LLM。
- 幕前 Append：默认 0 次额外 LLM（只跑确定性探针）；硬失败才触发一次短修订。
- 批量续写：章契约 1 次/章；场契约 0 次；核对复用 gate。总体 token 增长目标 ≤ 20%。

---

## 9. 验收指标

| 指标 | 现状/基线 | 目标 | 测量方式 |
|---|---|---|---|
| 通用 `next_node` 占比 | 可回退到「按场景结构推进」 | 生产 0% | 日志/契约来源标记 |
| 场契约字段完整率 | 无此概念 | ≥ 95% | 契约校验器 |
| 每场状态变化率 | 无强制 | 100% | 契约核对 + 探针 |
| 每章不可逆结果率 | 无强制 | 100% | 契约核对 |
| 情感回路启用率 | 仅配置情感契约/张力/债务的故事 | 新故事 100%；旧故事兼容模式 ≥ 90% | 契约来源标记 |
| 情感因果链覆盖率 | 仅配置情感契约的故事 | 100%（最小链） | 契约核对 |
| 主要角色给定名共用字 | 无校验 | 0 | 姓名账本 |
| 主要角色陈词组合命中 | 无校验 | 0 | 加权陈词表 |
| 姓名长度分布 | 模型塌缩 | 落在题材配额带内 | 姓名账本 |
| 本场称呼规则覆盖率 | 无 | 100% | 场简报/契约核对 |
| 续写新编姓名率 | 可新编 | 0 | `ContractVerifier` |
| 多作品切换成功率 | 生成中直接拒绝 | 100%（目标存在时） | E2E/事件日志 |
| 跨故事串稿 | 可能发生 | 0 | E2E/场景归属断言 |
| 切换导致的数据丢失 | 未保证 | 0 | flush 前后对比 |
| 切换 UI 反馈 P95 | 未定义 | ≤ 500ms 壳 / ≤ 1.5s 正文 | 前端埋点 |
| `stall` 场占比 | 不参与判定 | ≤ 20% | 编辑审计 + 探针 |
| 硬失败修订成功率 | 无 | ≥ 60% | 修订前后对比 |
| 人类 A/B「想继续读」 | 待测 | 相对提升 ≥ 20% | 30 章/3 题材盲测 |
| 人类 A/B「情节合理」 | 待测 | 相对提升 ≥ 20% | 同上 |
| 人类 A/B「人物动人」 | 待测 | 相对提升 ≥ 15% | 同上 |
| 重复率 | 8% 重试闸门 | 不高于现状 | code grader |
| 单章 token 成本 | 现状 | ≤ +20% | budget 统计 |
| 幕前 Append P95 延迟 | 现状 | ≤ +10% | 前端埋点 |

> 人类 A/B 的百分比是设计目标，不是承诺；基线需在实施前用 10–20 章样章测定。

### 9.1 先行探针（设计验收，不是实施）

在实施前，用 3 个题材各 3 章做「离线契约演练」：

1. 用现有资产 + 正文生成章契约/场契约（不写正文），人工判断契约是否具体、可执行、无通用句。
2. 用同一契约让模型写一场，人工按硬失败清单核对。
3. 对比当前路径同章：是否仍有「无冲突/无变化/无情感因果/原地踏步」。
4. 如果契约本身就不能生成具体内容，先修资产/计划源，不要先改 writer 提示词。

---

## 10. 风险与取舍

| 风险 | 表现 | 缓解 |
|---|---|---|
| 公式化 | 每场都变成「目标→冲突→灾难」，读起来像模板 | 允许反应场/静场，但必须有内在变化；场类型轮换；契约只规定最低要求，不规定写法 |
| 过度规划 | 契约写死，模型没有即兴空间 | 契约是「最低完成条件」，不是剧本；允许一个不服务情节但真实的细节；允许在契约内改变手段 |
| 计划质量差 | 章契约生成空泛，校验也拦不住 | 具体性校验 + 修复一次 + 最小契约；最小契约仍空泛则报资产不足，不写通用句 |
| 延迟/成本 | 多一次章契约调用，幕前变慢 | 章契约用后台档/低成本档；场契约 0 LLM；Append 只跑确定性探针；缓存章契约 |
| 旧故事不兼容 | 没有情感字段/关系表，契约生成失败 | 兼容模式 + 最小契约；从正文/目标/张力确定性推导；不强制回填 |
| 模型不遵从 | 长契约被忽略 | 一页简报 ≤ 300 字；契约放末尾高显著位置；证据化核对；硬失败定向修订 |
| 提示词覆盖冲突 | 用户自定义 prompt 与新编译器打架 | 同一 prompt id；新编译器只增加「契约层」，不替换用户可编辑的 writer 提示词；冲突时以契约为先并提示 |
| 质检过严 | 每章都修订，拖慢创作 | 只硬失败阻断；软问题记录；修订 1 轮；Append 只同步确定性硬失败 |
| 情感驱动过度 | 为情绪服务牺牲情节，变成苦情戏/全员崩溃 | 情感只在情节合法候选中选择压力最大的事件；外部目标/赌注/因果链由章契约硬约束；每场仍必须改变信息/关系/目标/风险/情绪之一，不能只改变情绪 |
| 命名过度设计 | 为独特而生僻，TTS 读不出、读者记不住 | 可读性硬约束；生僻字只用于有叙事理由的配角；主要角色必须在常用字/次常用字带内 |
| 命名校验误伤 | 用户手写的真实姓名被规则拦下 | 校验只拦硬冲突（重名/同音/同字/时代错置）；用户手写名登记进账本并给出称呼规则，不强制改名 |
| 切换取消长任务 | 作者正在批量续写，切换导致进度丢失 | 前台单拍取消；后台批量/创世分离并继续写旧故事；toast 明确「已停止/后台继续」 |
| 后台任务跨故事污染 | 旧故事流式事件进入新故事编辑器 | 所有异步回调带 `story_id` + `storyEpoch` 双过滤；切换时递增 epoch |

---

## 11. 待批准的设计决策

以下 5 项需要作者/产品确认；批准后另出实施方案。

| 编号 | 决策 | 推荐 | 理由 |
|---|---|---|---|
| D1 | 新故事是否默认启用 NCC | **是** | 新故事没有历史包袱；默认关闭等于继续产出旧质量问题 |
| D2 | 旧故事是否默认启用 | **兼容模式默认启用** | 有资产就生成契约，不足用最小契约；不强制回填 |
| D3 | 幕前 Append 的修订边界 | **确定性硬失败同步修订；软问题后台** | 保证流畅，同时不让结构问题累积 |
| D4 | 章契约的事实优先级 | **正文事实 > 章纲 > 故事大纲** | 避免「大纲说有、正文没写」的凭空事件；同时保留大纲方向 |
| D5 | 批量续写是否保留 tool_loop | **改为直接 complete + 契约；tool_loop 仅兜底** | 当前批量 writer 拿不到 SceneBeatCard 的冲突/情感/变化；tool_loop 还会让模型先读资产、稀释写作注意力 |
| D6 | legacy 提示词资产是否删除 | **不删除，标记 legacy；逐步映射到契约层** | 用户可能已覆盖；删除会破坏兼容性与可回退性 |
| D7 | 情感回路是否默认启用 | **默认启用最小链；完整 OCC/载体/读者面板按作者配置增强** | v0.75 的因果链是正确方向，但当前默认关闭导致多数故事退化为静态角色档案；最小链不增加作者配置成本 |
| D8 | 新故事是否强制走姓名编译器 | **是** | 当前提示词的「禁止常见姓/禁止单字名」正是塌缩源头；只有强制账本 + 校验才能根治 |
| D9 | 旧故事是否自动改名 | **否** | 自动改名会破坏已写正文、别名与既有关系；旧故事只反向建账本，新增角色生效，改名需作者确认 |
| D10 | 切换时前台生成取消还是分离 | **前台单拍/流式取消；后台批量/创世分离** | 前台结果只能写旧场景，跨故事必须隔离；长任务取消代价高，分离并让其写自己的故事更稳 |
| D11 | active story 持久化位置 | **后端 `set_active_story/get_active_story`** | localStorage 无法覆盖窗口未加载/多窗口一致性；后端是单一真源 |
| D12 | 同一故事点击行为 | **重新定位最新章节，不清空编辑器** | 用户只是想回到最新进度，不应触发 flush/取消/清空 |
| D13 | 切换是否弹确认 | **不弹；toast 说明「已停止/后台继续」** | 切换本身是明确意图，弹确认会打断；用可撤销的 toast 和后台状态提示即可 |

---

## 12. 附录：代码证据索引

| 结论 | 代码位置 |
|---|---|
| 单拍 writer 使用 `CONTINUE_BEAT_SYSTEM` | `src-tauri/src/prompts/assembly.rs:83`、`:191` |
| LeadWriter 批量使用 `agency_lead_writer_system` | `src-tauri/src/agency/roles.rs:23` |
| 单拍 writer prompt 组装顺序 | `src-tauri/src/agency/beat_card.rs:925` |
| 节拍卡只有阵容/冲突/情感/推进/必须改变 | `src-tauri/src/agency/beat_card.rs:216` |
| 冲突只从敌对关系 + 双方在场生成 | `src-tauri/src/agency/beat_card.rs:475` |
| 情感是静态角色档案 | `src-tauri/src/agency/beat_card.rs:627` |
| 下一节点可回退通用句 | `src-tauri/src/agency/beat_card.rs:710`、`:774`；`src-tauri/src/agency/prose_ground.rs:60` |
| 章纲不读上一章正文 | `src-tauri/src/agency/coordinator.rs:4728`、`:4800` 附近 |
| 章纲/拍节点互不校验 | `coordinator.rs:4728`；`beat_card.rs:710` |
| 资产渲染只消费 bundle 子集 | `src-tauri/src/agency/continue_assets.rs:819` |
| 资产预算 6000 字、从变动区尾部收缩 | `continue_assets.rs:16`、`:761`、`:1015` |
| 进度块只读 outline_content 前 200 字 | `coordinator.rs:8195` |
| 批量 writer task 不含节拍卡冲突/情感/变化 | `coordinator.rs:5803`、`:5919`、`:8338` |
| Append 不跑同步质量门、后台质检不修订 | `coordinator.rs:3735`、`:2998` |
| 质量门阈值与加权 | `src-tauri/src/agency/gate.rs:20` |
| 规则分 = 合同 0.4 + 追读 0.3 + 情感 0.3 | `src-tauri/src/agency/graders.rs:170` |
| 无情感计划时情感分 0.5 | `src-tauri/src/agency/graders.rs:120` 附近 |
| 编辑审计 9 维 rubric | `resources/prompts/agency/agency_editor_auditor_system.md` |
| dimension_scores 只落库 | `coordinator.rs:7664` 附近 |
| 修订任务只有问题列表 | `coordinator.rs:6996`、`:7004` |
| `methodology_extension` 加载但不进 Agency 资产 | `src-tauri/src/creative_engine/write_time_bundle.rs:270`、`:877` |
| `writer_beat_plan` 只在 planner 使用 | `src-tauri/src/planner/executor.rs:1738` |
| 取名提示词禁止常见姓/禁止单字名 | `resources/prompts/creation/novel_creation_character_roster.md`；`src-tauri/src/story_system/auto_contract.rs:868` |
| 角色谱解析只校验 JSON，不校验姓名分布 | `src-tauri/src/agents/novel_creation.rs:163` |
| 现有别名启发式（阿末字/末两字） | `src-tauri/src/agency/continue_assets.rs:41` |
| 角色别称/改名去重基础设施 | `src-tauri/src/db/character_identity.rs:115`、`src-tauri/src/db/repositories/character_alias_repository.rs` |
| 生成中拒绝切换的显式守卫 | `src-frontend/src/frontstage/FrontstageApp.tsx:633-638` |
| 幕前故事选择是局部 state，幕后是 appStore | `FrontstageApp.tsx:250`、`src-frontend/src/stores/appStore.ts:105` |
| `selectStory` 未先 flush 旧故事 | `FrontstageApp.tsx:3108` |
| `flushSceneSave` 能力已存在 | `src-frontend/src/frontstage/hooks/useScenePersistence.ts:217` |
| `story-selected` 事件无 seq/去重 | `src-frontend/src/hooks/useSyncStore.ts:176` |
| 幕后卡片点击同步事件 | `src-frontend/src/pages/Stories.tsx:469`；`src-tauri/src/commands/sync.rs:72` |
| `open_story_in_frontstage` 广播 | `src-tauri/src/commands/sync.rs:56`、`src-tauri/src/state_sync/service.rs:104` |
| `ChapterSwitch` 已有跨故事分支 | `FrontstageApp.tsx:2222` |
| 取消当前生成入口 | `FrontstageApp.tsx:4368`；`src-tauri/src/agency/commands.rs:263` |
| `writer_reading_power_goal` / `writer_chase_debt` / `writer_contract_constraints` 只在 legacy writer 使用 | `src-tauri/src/agents/service.rs:2005`、`:2037`、`:2081` |
| `style_dna` / `narrative_quartet` / `reference_scene_fewshots` 由 `to_prompt()` 渲染 | `write_time_bundle.rs:856`–`:917` |
| 场景结构方法论存在 | `resources/prompts/methodology/methodology_scene_structure.md` |
| 情感回路默认需契约/张力/债务/状态 | `coordinator.rs:8012` |
| 批量情感弧只在契约存在时返回 | `coordinator.rs:8069` |
| 情感覆盖冲突仅限本拍阵容内的关系对 | `coordinator.rs:8120` |
| 情感因果链渲染已实现 | `src-tauri/src/emotion_circuit/modulation.rs:11` |
| 章纲提示词含情感转折（仅启用情感契约时） | `resources/prompts/planner/scene_outline.md` |

---

## 13. 批准后的下一步

本文只做设计。实施方案已拆到 [2026-11-02-narrative-quality-implementation-plan.md](./2026-11-02-narrative-quality-implementation-plan.md)，至少包含：

- 契约数据模型与存储位置；
- 章契约生成/校验/修复的调用点；
- 场契约投影算法；
- prompt 渲染顺序与预算；
- 契约核对器与 gate 集成；
- Append 同步/异步边界；
- 灰度开关、回滚方案；
- 测试矩阵（单测、golden harness、真机样章、人类 A/B）。

在批准前，不建议先改 writer 提示词或再加规则——那只会把「没有契约」的问题藏得更深。
