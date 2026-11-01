# 批量续写接情感回路：`write_chapter` 的章节级情感弧设计

日期：2026-10-31
状态：已实施（v0.76.0）
决策来源：v0.75.0 已把情感回路接入单拍主路径 `write_beat_once`；批量续写 `write_chapter` 仍是一章一次 tool_loop，未接情感契约、情感弧、探针与写回。本设计只解决这一条链路的接通。
影响范围：`agency::coordinator::{run_continue_batch, run_batch_inner, write_chapter, handle_gate, assemble_next_chapter, evaluate_gate, evaluate_gate_impl, GateRunner}`、`agency::graders::run_rule_grader`、`emotion_circuit::{circuit, modulation, repo}`。
关联文档：`docs/plans/2026-10-31-story-emotion-circuit-design.md`（情感回路总设计）、`docs/plans/2026-08-13-agency-only-continuation-design.md`（Agency 唯一续写路径）、`CHANGELOG.md` v0.75.0。


**实施结果（v0.76.0）**：`write_chapter` 已返回 `BatchChapterArtifacts`；`run_batch_inner` 用 `last_written_plan` 做批内续接；`handle_gate`/`GateRunner`/`evaluate_gate_impl`/`run_rule_grader` 均接收章节计划；`assemble_next_chapter` 在有计划时回写 card 与 N 行 `scene_emotion_beats`/状态/张力/探针。新增 11 个 Rust 测试（章弧 1/2/3 幕、幕切分、批量提示词、批量落库、gate/revision 用同一计划、批内 previous plan 续接）。Rust **1946 passed / 10 ignored**；vitest 647 / 3；playwright 45 / 5；tsc/clippy/fmt/guard/IPC 全绿。**未关闭**：人类 A/B 与真实模型端到端批量情感目标达成率待复测。

---

## 0. 一页纸结论

**批量续写不是逐拍循环，不能照搬单拍路径。**

`write_chapter` 的形态是「一章 1500–2500 字，一次 tool_loop 写完」，不是 `write_beat_once` 的「一拍一次 complete」。因此批量接情感回路的正确粒度是**章节级情感弧**：

- 每章生成 **1–3 个 act（幕/段）**，每个 act 复用现有 `EmotionBeat`（事件→评估→情绪→行动/台词→代价→载体）。
- writer task 注入一个 ≤900 字的【本章情感回路】块；不新增 LLM 调用。
- gate 用**同一个章节计划**做确定性探针与编辑审查，而不是从 DB 读「尚未落库」的拍。
- 装配后把 N 个 act 写成 N 行 `scene_emotion_beats`，更新角色状态/张力/探针日志；不新增表。
- **只有作者设置了情感契约才启用**；没有契约时批量 writer task、装配行为、gate 行为与现在逐字/逐语义不变。
- 批内并发（gate(n) ∥ writer(n+1)）用**上一章已写出的 plan**作为下一章的起点，不依赖尚未 commit 的 DB 状态。

一句话：**把「逐拍情感因果链」压缩成「章节情感弧 + 每幕载体」，让批量续写也能被情感驱动，但保持批量的速度与结构。**

---

## 1. 现状与缺口（代码核对）

### 1.1 批量链路

```
run_continue_batch (6131)
  └─ run_batch_inner (6223)
       ├─ ensure_assets
       ├─ for chapter in start..start+count:
       │    ├─ write_chapter (5731)  ← 一次 tool_loop 写整章
       │    ├─ gate(n-1) 与 writer(n) 并发（6319-6371）
       │    ├─ spawn gate(n)（6373-6390）
       │    └─ pending_chapter = (章号, 草稿, 是否已修订)（6284, 6390）
       └─ 收尾 handle_gate（6393-6416）
```

### 1.2 具体缺口

| 缺口 | 代码事实 | 后果 |
|---|---|---|
| writer 没有情感计划 | `write_chapter` 只编译 `card`、生成章纲、`render_parts`、拼 writer task（5731-5832），没有任何 `EmotionBeat`/`select_next_beat` | 批量章节没有情感弧，writer 仍按普通大纲写 |
| writer 没有情感提示块 | writer task 三个分支（5811-5832）均无【本章情感回路】 | 即使作者设了契约，批量也不消费 |
| gate 拿不到章节计划 | `write_chapter` 只返回 `BoardItem`（5740）；`pending_chapter` 只存 `(i32, BoardItem, bool)`（6284）；`GateRunner::evaluate` 只接 `draft`（7405-7412） | gate 无法按计划评分情感因果/目标/张力 |
| gate 的 rule grader 从 DB 读拍 | `run_rule_grader`（`graders.rs:85`）用 `latest_beat_for_chapter` 读已落库的 `scene_emotion_beats` | 批量章在 gate 时尚未装配，读不到本拍计划，情感分退化为中性 0.5 或读到上一章 |
| 装配时 card 被丢弃 | `handle_gate` 调 `assemble_next_chapter(..., None)`（5990-6000） | 批量章不写 `characters_present` / `character_conflicts` / `setting_location` / 卡块大纲；这是既有缺口 |
| 没有情感写回 | `assemble_next_chapter` 只做 scene/ingest/checkpoint（6008-6127） | 批量章的情感链、角色状态、张力累积、探针日志都不落库；下一章压力场看不到 |
| revision 不知道情感弧 | `build_revision_task` 只拼接 gate issues（6665-6670） | 修订可能修掉情感因果，却不知道本章应该保留的 from→to 与载体 |
| 批内并发看不到上一章 | writer(n+1) 与 gate(n) 并发；gate(n) 未装配前，n 的状态不在 DB | 下一章的情感起点会退到 n-1，批内情感弧断裂 |

### 1.3 为什么「只加一块提示词」不够

1. gate 的 `emotion_score` 需要计划作为锚点；没有计划只能中性或读旧章。
2. revision 需要知道本章的 from→to 与每幕代价，否则会把情感线改平。
3. 不落库，下一章压力场读不到本章情感结果，批量越多章越断。
4. 不把 card 带到装配，scene 元数据继续缺失，轮换账本/债务也读不到。

---

## 2. 目标与非目标

### 2.1 目标

- 批量章节获得**章节级情感弧**：1–3 个 act，每个 act 有事件/评估/情绪变化/行动倾向/代价/载体。
- writer task 注入【本章情感回路】，与单拍路径同一套 `EmotionVector`/`Appraisal`/`CarrierPlan`/`EmotionProbe`。
- gate 使用同一个计划做确定性探针 + 编辑审查；revision 能看到情感弧。
- 装配后写回 `scene_emotion_beats` / `character_emotion_states` / `emotion_tensions` / `emotion_probe_log`。
- 没有情感契约时，批量 writer task、gate 行为、装配行为保持现状。
- **零新增 LLM 调用**；批量延迟增加目标 ≤5%。

### 2.2 非目标

- 不把 `run_continue_batch` 改写成逐拍循环；不改变「gate(n-1) ∥ writer(n)」的并发结构。
- 不新增数据表；复用 V150 七张表。
- 不在 P0/P1 让情感探针硬阻断批量章；只进 gate 规则分/审查问题/质量债。
- 不改变 Gate 顶层权重（0.2 code / 0.3 rule / 0.5 model）与阈值 0.75。
- 不做远程模型的激活级干预；白盒端口仍只服务本地权重模型。
- 不在本设计内重做 batch 的 `ensure_assets`、resume、会话快照。

---

## 3. 核心裁定

| 裁定 | 内容 | 理由 |
|---|---|---|
| D1 粒度 | 章节级 1–3 act，而不是一拍一次 | 批量是一章一次 tool_loop；逐拍会推翻现有结构与速度 |
| D2 类型 | act 复用 `EmotionBeat`；章节用 `ChapterEmotionPlan` 聚合 | 单拍/批量共享评估、载体、探针，不造第二套语义 |
| D3 开关 | 只有 `story_emotion_contracts` 存在才启用 | 与 v0.75.0 单拍路径一致；旧故事零变化 |
| D4 传参 | `BatchChapterArtifacts` 沿 `write_chapter → pending_chapter → handle_gate → assemble_next_chapter` 显式传递 | 不用全局状态；gate 并发时 plan 必须随 draft 一起 owned |
| D5 落库 | N 个 act 写 N 行 `scene_emotion_beats`（`beat_index=1..N`） | 复用现有表与探针；前端情感页签无需新 schema |
| D6 gate | 有 plan 用 plan；无 plan 回退 DB（单拍路径） | 不破坏单拍路径，也让批量 gate 有锚点 |
| D7 阻断 | P0/P1 advisory；P2 校准后再评估 | 批量一次写整章，误杀代价高于单拍 |
| D8 批内连续 | 下一章 writer 用上一章已写出的 plan 的 `to` 作为起点 | gate(n) 与 writer(n+1) 并发，不能依赖未 commit 的 DB 状态 |

---

## 4. 数据与类型

### 4.1 `ChapterEmotionPlan`

```rust
/// 一章的情感弧。1–3 个 act；act 复用单拍的 EmotionBeat。
#[derive(Debug, Clone)]
pub struct ChapterEmotionPlan {
    pub chapter_number: i32,
    pub contract: Option<EmotionContract>,
    /// 按章节顺序排列的 1–3 幕。
    pub beats: Vec<EmotionBeat>,
    /// 本章选中的主张力/债务（可为空）。
    pub tension: Option<EmotionTension>,
    pub debt: Option<EmotionDebt>,
    /// 章纲里的关键事件/节点（用于 writer/editor 对照）。
    pub plot_nodes: Vec<String>,
    /// 全章起点/终点（由 beats.first().from 与 beats.last().to 派生）。
    pub chapter_from: EmotionVector,
    pub chapter_to: EmotionVector,
}

impl ChapterEmotionPlan {
    pub fn peak_count(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

### 4.2 `BatchChapterArtifacts`

```rust
/// 批量一章从写作到装配需要携带的全部产物。
#[derive(Debug, Clone)]
pub struct BatchChapterArtifacts {
    pub draft: BoardItem,
    /// 本拍/本章编译出的节拍卡（用于 scene 元数据写回）。
    pub card: crate::agency::beat_card::SceneBeatCard,
    /// 有情感契约时才有。
    pub emotion: Option<ChapterEmotionPlan>,
    /// 写作后跑出的确定性探针（advisory）。
    pub emotion_probes: Vec<crate::emotion_circuit::EmotionProbe>,
}
```

### 4.3 持久化映射（复用 V150）

| 内存 | 表 | 写入时机 |
|---|---|---|
| `ChapterEmotionPlan.beats[i]` | `scene_emotion_beats`（`beat_index=i+1`） | 装配成功后 |
| `beats.last().to` | `character_emotion_states`（`source='batch'`） | 装配成功后 |
| `plan.tension.id` | `emotion_tensions`（pressure +0.05，accumulated +N，last_escalated=章） | 装配成功后 |
| `emotion_probes` | `emotion_probe_log`（`beat_index=1..N` 或 NULL） | 装配成功后 |
| 失败项 | `quality_debts`（kind=`continue_emotion`） | 探针有失败时 |

不新增表。`reader_resonance_reviews` 仍由手动/后台读者面板写，不在批量热路径。

---

## 5. 算法

### 5.1 章节情感弧规划 `plan_chapter_emotion`

输入：`pool/story_id/chapter_number/card/chapter_outline/contract/tensions/debts/states/cast/peak_used/previous_plan`。

输出：`Option<ChapterEmotionPlan>`。

步骤：

1. **无契约短路**：`contract` 为空 → `None`。批量 writer task 与现在逐字不变。
2. **选压力源**：复用单拍 `select_next_beat` 的评分：关系压力 × 债务逾期 × 情感位移；过滤到本拍 card 阵容内可执行的关系对。
3. **取幕事件**：
   - 从 `chapter_outline` 中解析「关键事件」列表（2–4 条）；没有则回退 `card.next_outline_node`。
   - 每条事件按关键词映射 `EventKind`：威胁/背叛/失去/重逢/真相/羞辱/帮助/牺牲；认不出时用张力类型映射（复用 `map_tension_type`）。
   - 上限 3 幕；只有 1 条事件时生成 2 幕（加压 → 后果）或 1 幕（章纲极短）。
4. **逐幕评估**：
   - 第 1 幕 `from` = `previous_plan.last().to`（有上一章 plan）或角色最新状态/初始向量。
   - 每幕 `appraisal = appraise_event(event, target, sensitivities)`；`to = emotion_from_appraisal`。
   - 若 `from.distance(&to) < 0.10`，换更强的事件（如 SecretReveal/Threat）或加大 appraisal 的 norm_violation。
   - `action_tendency`/`dialogue_policy`/`cost` 复用单拍函数；`carriers = select_carriers(to, defense, public, action_scene)`。
5. **强度预算**：
   - 默认第 1 幕 0.8，第 2 幕（若有）1.2，第 3 幕 0.8。
   - 若 `contract.peak_budget()` 只允许 1 次，则只允许一幕 1.2；其余 0.4/0.8。
   - 连续两幕不得都是 1.2。
6. **章弧校验**：`chapter_from.distance(&chapter_to) >= 0.20`；不足则把最后一幕换成更高压事件，或在章末加一个「余波」幕（只写后果，不再升级）。
7. **空输入回退**：章纲为空 → 1 幕，事件用 `card.next_outline_node`；仍为空 → `None`（不硬造情感）。

> 复用边界：第 1 幕可直接调用 `select_next_beat` 得到 beat，再把 `last.to` 作为第 2 幕的 `from`。不要复制一套评估逻辑。

### 5.2 章节分段与探针 `split_chapter_into_acts`

批量草稿是整章文本，探针需要按幕切分：

```
fn split_chapter_into_acts(text: &str, n: usize) -> Vec<String>
```

- n=1 或 `chars < 600`：整章一段。
- n=2：按段落累计字符 55% 处切。
- n≥3：按 30% / 65% 切；每段至少 200 字，否则合并到相邻段。
- 段落优先按空行/换行切；不按句号硬切，避免把对话拆散。

探针聚合：

| 探针 | 聚合规则 |
|---|---|
| `emotion_causality` | 至少 `ceil(n/2)` 段通过 → pass；否则 fail |
| `emotion_target` | 全章 `EmotionVector::from_text` 与 `plan.chapter_to` 的 cosine；阈值沿用单拍 |
| `emotion_delta` | `plan.chapter_from` 与全章实际向量的位移；<0.10 fail |
| `voice_consistency` | 任一段命中台词禁忌 → fail |
| `tension_payoff` | 任一段出现升级/揭示/修复/破裂标记 → pass |
| `melodrama` / `reader_restraint` / `carrier_sparsity` | 任一段命中 → 按单拍规则 fail/warn |
| 情感分 | `1.0 - Σ penalty`（high 0.25 / medium 0.12 / low 0.05），与单拍一致 |

### 5.3 批量写入提示词

新增 `render_batch_emotion_block(plan: &ChapterEmotionPlan) -> String`，≤900 字：

```text
【本章情感回路（批量续写；硬约束）】
本章情感弧：羞耻0.60/愤怒0.30 → 愤怒0.55/希望0.25；主张力：林雪→阿岩「未揭穿的欺骗」压力0.82（积压3拍，本章必须接近暴露或加深）
第1幕（开篇/加压）：
  事件：阿岩当众替林雪承认那封信。
  评估：林雪视为「他在替我承担，也在替我定义我是谁」；目标相关高、与底线冲突、对方可控、有观众。
  情绪：羞耻0.6+愤怒0.3 → 羞耻0.4+愤怒0.6。
  行动/台词：否认并冷嘲；目标=让他收回那句话；潜台词=你凭什么替我决定；绝不说=谢谢。
  代价：她当众推开他，坐实「不识好歹」。
  载体：动作（推回伞）/台词潜台词（冷嘲）/代价（他沉默离开）。
第2幕（转折/升级）：
  ...
第3幕（收束/余波）：
  ...
读者目标：先替林雪难堪，再对她的反击产生复杂同情；禁止旁白解释「她其实很感动」。
禁忌：不得直接写「她很痛苦」；不得让所有人轮流表态；不得用天气映心情。
强度：第1幕0.8 / 第2幕1.2 / 第3幕0.8。
```

放置位置：`writer_task` 的 `header → 章纲 → assets_ctx → 情感块 → 写作要求`。情感块靠近写作要求，保持高显著；assets 仍在其前，避免把稳定资产推到更后。

`render_batch_revision_block(plan)` 复用同一块，但只保留「本章弧 + 每幕必须保留的事件/代价/载体」，供 revision 使用，≤600 字。

### 5.4 gate 集成

1. `write_chapter` 返回 `BatchChapterArtifacts`，而不是 `BoardItem`。
2. `run_batch_inner` 的 `pending_chapter` 改为 `(i32, BatchChapterArtifacts, bool)`；`pending_gate` 闭包捕获 `Arc<ChapterEmotionPlan>`（有则 Some）。
3. `GateRunner` 增加 `emotion_plan: Option<Arc<ChapterEmotionPlan>>`；`evaluate` 透传给 `evaluate_gate_impl`。
4. `evaluate_gate_impl` 增加 `emotion_plan: Option<&ChapterEmotionPlan>`：
   - editor assets 追加 `render_batch_emotion_block(plan)`，让编审能对照「应该发生的情感弧」。
   - `run_rule_grader` 增加 `emotion_plan` 参数；有 plan 时用 `plan.beats` 做探针，无 plan 时保留现有 `latest_beat_for_chapter` 回退。
5. `handle_gate` 增加 `artifacts: BatchChapterArtifacts`：
   - revision task 追加 `render_batch_revision_block(plan)`。
   - 第二次 gate 继续传同一个 plan。
   - 最后 `assemble_next_chapter` 传 `Some(&artifacts.card)`、`artifacts.emotion.as_ref()`、`&artifacts.emotion_probes`。
6. 单拍路径 `run_continue_inner` 调 `assemble_next_chapter` 时新增参数传 `None`，行为不变。

### 5.5 装配与写回

`assemble_next_chapter` 增加参数：

```rust
card: Option<&SceneBeatCard>,
emotion: Option<&ChapterEmotionPlan>,
emotion_probes: &[EmotionProbe],
```

装配事务成功后：

```rust
if let Some(plan) = emotion {
    repo.persist_chapter_plan(story_id, &scene.id, chapter_number, plan, emotion_probes)?;
}
```

`persist_chapter_plan` 在**一个事务**内：

1. 为每个 act 插入 `scene_emotion_beats`（`beat_index=i+1`）。
2. 插入 `character_emotion_states`（source=`batch`，取最后一幕 `to`）。
3. `UPDATE emotion_tensions`：`pressure = MIN(1.0, pressure + 0.05)`，`accumulated_beats += N`，`last_escalated_chapter = chapter`。
4. 插入 `emotion_probe_log`（每个探针一行；`beat_index` 为对应幕或 NULL）。
5. 失败探针汇总写 `quality_debts`（kind=`continue_emotion`）。

失败只告警，不影响正文落库（沿用单拍原则）。

**card 回写**：只有 `emotion.is_some()` 时才把 `card` 传给 `assemble_next_chapter`。这样：

- 有情感契约：补齐 `characters_present` / `character_conflicts` / `setting_location` / 卡块大纲（修复既有批量缺口）。
- 无情感契约：保持现在的 `None` 行为，旧批量语义不变。

### 5.6 批内连续性：上一章 plan 作为下一章起点

`run_batch_inner` 的结构是「gate(n-1) ∥ writer(n)」，而 gate(n-1) 装配前，n-1 的状态不在 DB。为了让批内情感弧不断：

- `run_batch_inner` 维护 `last_written_plan: Option<ChapterEmotionPlan>`。
- 每轮 `write_chapter` 增加参数 `previous_plan: Option<&ChapterEmotionPlan>`。
- `plan_chapter_emotion` 用 `previous_plan.last().to` 作为第 1 幕的 `from`；用上一章 tension 的 pressure 作为初始压力（若同一张力）。
- writer(n) 完成后，把 `artifacts.emotion` 存入 `last_written_plan`，供 n+1 使用。
- revisor 改变第 n 章后，n+1 的 plan 可能略旧；P2 可在 `handle_gate` 完成后比较 `draft` 实际结尾与 plan 的 `to`，差异大时标记 n+1 的 plan 为「待重算」，不自动重写。

这是批量情感连续性的关键：**不依赖尚未 commit 的 DB 状态，也不新增同步点。**

---

## 6. 代码接入点

| 文件/位置 | 改动 |
|---|---|
| `src-tauri/src/emotion_circuit/circuit.rs` | 新增 `ChapterEmotionPlan`、`plan_chapter_emotion`、`split_chapter_into_acts`、`plan_from_outline` |
| `src-tauri/src/emotion_circuit/modulation.rs` | 新增 `render_batch_emotion_block`、`render_batch_revision_block` |
| `src-tauri/src/emotion_circuit/repo.rs` | 新增 `persist_chapter_plan`（单事务：N acts + state + tension + probes + debt） |
| `agency/coordinator.rs::write_chapter` (5731) | 返回 `BatchChapterArtifacts`；生成 plan；writer task 注入情感块；写完跑探针 |
| `agency/coordinator.rs::run_batch_inner` (6223) | `pending_chapter` 类型改为 artifacts；维护 `last_written_plan`；gate 闭包捕获 plan |
| `agency/coordinator.rs::handle_gate` (5888) | 接收 artifacts；revision/second gate 带 plan；装配传 card/plan/probes |
| `agency/coordinator.rs::assemble_next_chapter` (6008) | 新增 card/emotion/probes 参数；scene 提交后 `persist_chapter_plan` |
| `agency/coordinator.rs::GateRunner` (7394) | 新增 `emotion_plan: Option<Arc<ChapterEmotionPlan>>` |
| `agency/coordinator.rs::evaluate_gate` (6625) / `evaluate_gate_impl` (6982) | 透传 plan；editor assets 追加情感块 |
| `agency/graders.rs::run_rule_grader` (85) | 新增 `Option<&ChapterEmotionPlan>`；有 plan 用 plan.beats，无则 DB 回退 |
| `agency/coordinator.rs::build_revision_task` (6665) | 新增可选情感块；或新增 `build_revision_task_with_emotion` |
| `agency/emotion_commands.rs` / 前端 | 无新增命令；批量落库后现有情感页签自动可见 |
| `e2e/mock-tauri.ts` | 无新增命令；批量 run 已有 mock，若前端展示批量情感进度再加 |

---

## 7. 失败与降级

| 场景 | 行为 |
|---|---|
| 无情感契约 | `plan=None`；writer task / gate / 装配全部走旧路径 |
| 章纲为空 | 1 幕，用 `next_outline_node`；仍为空则 `None` |
| 章纲关键事件解析失败 | 按张力类型生成 1–2 幕；不阻断写作 |
| 情感块超长 | 先删「读者目标/禁忌」，保留弧与每幕事件/代价/载体 |
| 探针失败 | 只记录 + 质量债；gate 规则分扣分；不硬阻断（P0/P1） |
| editor 失败 | 沿用 `salvage_failed_gate`；若装配继续，仍写回 plan/probes |
| gate 修订 | revision task 带情感块；第二次 gate 用同一 plan；不重新规划 |
| 取消 | 未装配的章节不写回；已装配的按现有语义保留 |
| DB 写回失败 | 只告警，不影响正文落库 |
| 模型忽略情感块 | 探针检测；gate 扣分/审查问题；不自动改写 |

---

## 8. 测试与验收

### 8.1 单元测试

| 测试 | 预期 |
|---|---|
| `plan_chapter_emotion_no_contract_returns_none` | 无契约 → None |
| `plan_chapter_emotion_one_event_creates_two_acts` | 1 个关键事件 → 2 幕，弧位移 ≥0.20 |
| `plan_chapter_emotion_three_events_creates_three_acts` | 3 个事件 → 3 幕，峰值 ≤ budget |
| `plan_chapter_emotion_peak_budget_respected` | budget=1 时只有一幕 1.2 |
| `plan_chapter_emotion_uses_previous_plan_as_from` | 第一章 `to` 成为第二章 `from` |
| `split_chapter_into_acts_short_text_one_segment` | <600 字 → 1 段 |
| `split_chapter_into_acts_long_text_three_segments` | >1200 字 → 3 段，非空 |
| `render_batch_emotion_block_bounded` | ≤900 字，含弧/事件/代价/载体 |
| `render_batch_emotion_block_empty_without_plan` | 无 plan → 空串 |
| `persist_chapter_plan_writes_n_beats_and_state` | N 行 scene_emotion_beats + 1 行 state + tension 累积 |
| `run_rule_grader_uses_plan_for_batch` | 有 plan 时情感分来自 plan；无 plan 回退 DB/0.5 |
| `build_revision_task_includes_emotion_block` | revision task 含本章弧与每幕代价 |

### 8.2 集成测试

- `test_run_continue_batch_injects_emotion_block`：有契约 + 张力，`run_continue_batch(1 章)`，MockLlm 捕获 writer task，断言含【本章情感回路】；装配后 `scene_emotion_beats` 行数 = 幕数。
- `test_run_continue_batch_without_contract_is_unchanged`：无契约，writer task 与当前版本逐字相同；不写情感表。
- `test_batch_gate_emotion_score_uses_plan`：gate 记录 `emotion_score` 来自 plan，而不是中性 0.5。
- `test_batch_revision_keeps_emotion_arc`：gate 返回 revise，revision task 含情感块；第二次 gate 用同一 plan。
- `test_batch_scene_metadata_written_when_emotion_enabled`：有 plan 时 scene 的 `characters_present`/`conflicts`/`location` 被写入；无 plan 时保持 None。
- `test_batch_previous_plan_feeds_next_chapter`：两章批量，第二章 plan 的 `from` = 第一章 plan 的 `to`。

### 8.3 回归与门禁

- 现有 `cargo test --lib` **1935 passed / 10 ignored** 不得回退；预计新增 12–15 个 Rust 测试。
- `npx vitest run` 647 / 3 不变（本设计无前端改动）。
- `npx playwright test` 45 / 5 不变。
- `tsc / clippy / fmt / architecture_guard / IPC manifest` 全绿。
- 旧批量路径的 writer task 字节级契约：无契约时 `write_chapter` 的 `writer_task` 与基线相同。
- 延迟：有契约时 batch 单章 writer prompt 增加 ≤900 字，目标批量总耗时增加 ≤5%。

### 8.4 真机验收（实施后）

- 选一本已设置情感契约、已有张力账本的故事，跑 `run_continue_batch(count=2)`。
- 检查：两章 writer task 均含情感块；`scene_emotion_beats` 每章 2–3 行；第二章第一幕 `from` = 第一章末幕 `to`；gate 记录有情感分；情感页签能看到批量章。
- 未通过前不得宣称「批量续写已具备情感驱动」。

---

## 9. 分期

| 阶段 | 建议版本 | 交付 | 验收 |
|---|---|---|---|
| **P0：计划与提示词** | v0.75.1 | `ChapterEmotionPlan`/`plan_chapter_emotion`/`split_chapter_into_acts`；`write_chapter` 生成 plan 并注入 writer task；单元测试；不接 gate/写回 | 无契约批量 task 逐字不变；有契约含块；Rust +8 |
| **P1：gate 与写回** | v0.76.0 | `BatchChapterArtifacts` 贯穿；GateRunner/evaluate/rule grader 接 plan；revision 带情感块；`persist_chapter_plan`；card 写回；集成测试 | 批量章写回 N acts/state/tension；gate 情感分来自 plan；旧路径不变；Rust +12 |
| **P2：校准与界面** | v0.76.x | 批量章情感分趋势、批量进度「第 N 幕」、探针误报校准；评估是否将严重情感问题纳入阻断 | 真机 2 章批量通过；误报 ≤20%；不增加 >5% 延迟 |

---

## 10. 风险与开放问题

### 10.1 风险

| 风险 | 缓解 |
|---|---|
| 幕切分与 writer 实际结构不一致，探针误判 | 按段落比例切；advisory；P2 校准；不硬阻断 |
| 情感块让 tool_loop prompt 变长 | ≤900 字；只保留弧/每幕事件/代价/载体；超长先删读者目标 |
| gate 并发拿不到 plan | plan 随 `BatchChapterArtifacts` owned/Arc 传递，不读 DB |
| 批内 n+1 看不到 n 的状态 | `previous_plan` 内存传递；revision 后标记待重算 |
| card 回写改变旧批量行为 | 仅 `emotion.is_some()` 时传 card；无契约保持 None |
| 一章 3 幕但 writer 写成 5 个场景 | 探针按段聚合；不要求严格对齐；编辑审查看「是否兑现弧」 |
| 批量跑长章时探针成本 | 纯 Rust，O(字符数)；无 LLM；可忽略 |
| 与单拍路径语义漂移 | 共享 `EmotionBeat`/`appraise_event`/`select_carriers`/`run_emotion_probes`；不复制实现 |

### 10.2 开放问题（实施前拍板）

1. **幕数默认**：2 幕（加压→后果）还是 3 幕（加压→升级→余波）？建议按章纲关键事件数：1 条→2 幕，2 条→3 幕，0 条→1 幕。
2. **gate 情感分是否阻断**：建议 P0/P1 不阻断；P2 校准后仅「因果 fail + 张力 fail + 编辑 ≤2」三重条件阻断。
3. **card 回写范围**：有情感契约才传 card，还是所有批量章都传（顺便修复既有缺口）？建议先只在有情感契约时传，降低回归面。
4. **revision 后 n+1 是否重算**：建议 P2 做「差异大时标记待重算」，不在 P1 自动重写。
5. **批量章是否跑读者面板**：建议不自动跑；作者可在情感页签手动运行。

---

## 11. 不变量清单

1. 无 `story_emotion_contracts` → `plan=None`，批量 writer task / gate / 装配行为不变。
2. 批量仍是「一章一次 tool_loop」，不改成逐拍循环。
3. 不新增 LLM 调用；不新增表。
4. 情感块 ≤900 字；revision 块 ≤600 字。
5. gate 有 plan 用 plan，无 plan 回退 DB（单拍路径）。
6. 情感探针 P0/P1 只记录/进规则分，不硬阻断。
7. 装配写回失败不影响正文落库。
8. 批内 n+1 的起点来自 n 的 plan，不依赖未 commit 的 DB。
9. Gate 顶层权重与阈值不变。
10. 远程 API 不做激活级干预。

---

## 12. 附：批量 writer task 结构（有/无情感）

**无契约（现状，逐字不变）**

```text
续写第N章（1500-2500 字）。
【本章大纲（必须遵循的章节方向）】
...
【世界观、角色、故事大纲与前文】
...
写作要求：
- ...
完成后用 board_write 把完整正文写入 draft 区（item_type=chapter, key=第N章）。
```

**有契约（新增情感块）**

```text
续写第N章（1500-2500 字）。
【本章大纲（必须遵循的章节方向）】
...
【世界观、角色、故事大纲与前文】
...
【本章情感回路（批量续写；硬约束）】
...（≤900 字）
写作要求：
- 严格按照本章大纲的冲突和转折撰写，必须兑现【本章情感回路】的章节弧与每幕代价。
- ...
完成后用 board_write 把完整正文写入 draft 区（item_type=chapter, key=第N章）。
```

---

## 13. 参考

- `docs/plans/2026-10-31-story-emotion-circuit-design.md`
- `src-tauri/src/agency/coordinator.rs`：`run_continue_batch` / `run_batch_inner` / `write_chapter` / `handle_gate` / `assemble_next_chapter` / `evaluate_gate_impl`
- `src-tauri/src/agency/graders.rs::run_rule_grader`
- `src-tauri/src/emotion_circuit/{circuit,modulation,probe,repo}.rs`
- `src-tauri/src/db/migrations/V150__emotion_circuit.sql`
