# 情感回路（Story Emotion Circuit）：让情感成为情节发动机

日期：2026-10-31
状态：已实施（P0–P3，v0.75.0）
决策来源：用户提出「AI 创作小说最大的缺点是没有和人类读者的情感共鸣」，要求融合 EmotionCircuits-LLM（arXiv:2510.11328）的机制发现与本项目架构，强化情感对情节进程、故事线、戏剧冲突、人物对白/性格/行为的根本驱动。
影响范围：Agency 续写主路径、角色/关系资产、章纲与卷纲、质量门、资产回流、幕后代理工作室前端。
关联文档：`docs/superpowers/plans/2026-08-09-emotional-attrs-relationships.md`、`docs/plans/2026-08-13-agency-only-continuation-design.md`、`docs/plans/2026-08-15-continue-quality-closure-design.md`、`docs/plans/2026-10-30-corpus-director-crew-chat-design.md`。
参考实现：https://github.com/Aurora-cx/EmotionCircuits-LLM（GPL-3.0，本设计只借鉴方法论，不复制代码）。

**实施结果（v0.75.0）**：新增 `src-tauri/src/emotion_circuit/` 与 V150 七张表；`write_beat_once` 接入压力场选拍与【本拍情感回路】；章纲情感目标、ingest 回流、编辑第 9 维、rule grader 情感分、代理工作室「情感」页签与 5 个 IPC 命令均已落地。Rust **1935 passed / 10 ignored**（+45）；vitest **647 passed / 3 skipped**（+1）；playwright **45 passed / 5 skipped**；tsc / clippy / fmt / guard / IPC manifest 全绿。**未关闭**：人类 A/B 尚未跑；读者面板只做代理指标；批量续写 `write_chapter` 尚未接；真实模型端到端情感目标达成率待复测。

---

## 0. 一页纸结论

**论文给我们的不是「更会写情绪词」，而是三条可迁移的控制论原则：**

1. **情绪是可分离的方向，不是表面标签。** 在六种情绪条件下，模型内部存在跨语境稳定的情绪方向；把情绪指令从提示词里拿掉，仅沿这些方向调制激活，仍能诱导出对应情绪（steering 成功率 91.22%）。
2. **情绪由少数关键组件局部实现。** 每层只有少量 MLP 神经元与注意力头对情绪方向有决定性贡献；消融它们情绪显著下降，增强它们情绪显著上升，随机干预几乎无效。
3. **全局稀疏电路的调制优于单一全局向量。** 按逐层因果重要性分配稀疏预算（论文总预算 560 = 10 × 56 个子层，MLP/Attention ≈ 70/30），直接调制电路达到 99.65% 的情绪表达准确率，高于 prompting 与 steering，且文本更自然。

**映射到小说创作：**

| 论文机制 | 小说层的对应物 |
|---|---|
| 情绪方向向量 | `EmotionVector`：角色/场景/读者的情绪状态，独立于情节内容 |
| 局部神经元/注意力头 | 8 类**表达载体**（动作、台词潜台词、选择、代价、感官、内心、沉默、节奏） |
| 子层因果重要性 | 故事位置的情感预算（铺垫/升级/兑现/余波），不是每拍都峰值 |
| 全局稀疏电路 | 每拍只选 2–3 个载体 + 一条最高压的关系张力，不做「全都要」 |
| 情绪差分向量 + scale | 本拍 `from → to` 的情感增量与强度档位（0.4/0.8/1.2） |
| 消融/增强验证 | 反事实探针：抽掉一个载体，情感是否塌；放大一个载体，目标情感是否上升 |
| 99.65% 控制准确率 | 目标情感达成率、情感因果率、角色声音一致率、读者共鸣预测 |

**本设计的核心裁定：情感必须上游于情节，而不是情节的形容词。**

当前链路是「大纲 → 冲突 → 情感行 → 正文」，情感是第四顺位的装饰；本设计改为「情感契约 → 角色评估 → 关系压力 → 选择能制造情感转折的事件 → 正文」。情感先决定「谁在这一拍必须做一个什么选择、付出什么代价」，情节节点再被这个选择逼出来。这样情感才会成为情节进程、故事线、冲突与对白的发动机。

**不做什么：**

- 不复制 EmotionCircuits-LLM 的 GPL 代码，不做远程闭源模型的激活钩子（无法拿到 hidden states）。
- 不把「情感」做成每拍都必须嚎啕大哭的强度竞赛；峰值预算每章 1–2 次。
- 不用一个 LLM 判官替代作者；判官只做辅助信号，作者裁决优先。
- 不在 P0 引入额外热路径 LLM 调用；P0 全部是纯 Rust 确定性计算。

---

## 1. 问题：为什么 AI 小说「有情绪词，没有情感驱动力」

### 1.1 用户命题

AI 小说最大的缺点不是文笔、不是设定，而是**没有和人类读者的情感共鸣**。读者不会被「她很悲伤」打动，只会被「她想要的东西、她为它付出的代价、她在代价面前的選擇」打动。情感共鸣是**因果体验**，不是词汇体验。

### 1.2 代码证据：情感目前是静态声明

对 v0.74.3 的核对结论（不是会话推测）：

| 现状 | 代码事实 | 后果 |
|---|---|---|
| 节拍卡只有一行静态「情感：」 | `src-tauri/src/agency/beat_card.rs:233`；`compile_emotion` 取第一个有情感四元组的在场角色，或第一条不对称关系，或兜底句（`:627-691`） | 同一角色连续多拍的情感行完全相同；没有 `from → to`，没有强度，没有触发事件 |
| 「必须改变」几乎不选情感 | `compile_change_delta` 先选冲突，再选下一节点，最后才回落到 `ChangeKind::Emotion`（`:580-625`） | `Emotion` 实际不可达；情感从来不是本拍的主变化 |
| 情感账本是静态的 | `accumulated_chapters` 恒为 0、`stage` 恒为 `Brewing`（`emotional_ledger.rs:49-55,132`；加载 `:186-224`） | 张力不累积、不逾期、不兑现；「积压 0 章」永远不变 |
| 情感块处于预算收缩的末段 | 张力/弧光各截断 400 字、非 protected、位于资产块末尾（`continue_assets.rs:939-948,989-1006`）；预算从最后一个非保护段往前收缩（`:738-766`） | 上下文紧张时，伏笔、弧光、张力会先于前文/关系被截断，情感信息最先残缺 |
| L1 角色卡没有情感字段 | `render_one_card_l1` 只渲染身份/身体/精神/位置（`continue_assets.rs:553-572`） | 因指令/大纲被准入、但不在场的角色，情感信息全丢 |
| 状态网没有情感 | `BeatState` 只有在场、地点、未决（`beat_state.rs:17-24`） | 探针不检查情感；写后不更新情感状态 |
| 探针不检查情感 | `probe_increment_ex` 检查阵容、冲突升级、地点、死亡、身份/亲缘、知识边界（`beat_state.rs:145-179,265-353`） | 「情感没落地」不会被发现 |
| 质检门没有情感维度 | 编辑审查八维：continuity/style/contract/ai_tone/hook/repetition/stall/reversal（`resources/prompts/agency/agency_editor_auditor_system.md:13-41`）；Gate 权重 0.2/0.3/0.5，阈值 0.75（`agency/gate.rs:20-32`） | 情感质量不影响是否通过 |
| 系统提示词只教「怎么写情绪」 | `CONTINUE_BEAT_SYSTEM` 第 7 条只管四模式混用、句长、标签（`prompts/assembly.rs:83-104`） | 没教「这一拍的情感为什么必须发生」 |
| 章纲没有情感目标 | `scene_outline.md` 只要求「地点、时间、氛围」（`:48-53`） | 规划层不产出情感转折，正文层只能自由发挥 |
| 资产回流只存静态画像 | `memory_content_analysis.md` 抽取四元组、双向 bond、sentiment（`:41-59,72-75`）；`asset_bridge` 只填不改（`asset_bridge.rs:77-108,222-226`） | 每章实际发生了什么情感变化，没有变成下一章的输入 |

### 1.3 三个断裂

**断裂一：评估断裂。** 事件发生了，但角色为什么因此产生这种情绪，没有模型。模型只能根据「她是悲伤的人」自由发挥，情绪与事件之间没有可检查的因果。

**断裂二：传导断裂。** 情绪产生了，但情绪如何改变目标、选择、台词、行为，没有约束。情绪停在描写层，没有变成行动势能。

**断裂三：回收断裂。** 行动产生了后果，但后果如何改变关系、债务、下一拍的压力，没有持久化。情感不累积，读者感受不到「越来越」。

### 1.4 本设计的裁定

| 项 | 决定 |
|---|---|
| 情感的地位 | 情感是**上游约束**：先定情感转折，再选能制造该转折的事件；不是正文写完再润色 |
| 情感的最小单位 | 不是「章」，不是「场景」，是**一拍**：一次事件 → 一次评估 → 一次情绪变化 → 一个选择/台词 → 一个代价 |
| 情感的表达 | 稀疏：每拍 2–3 个载体，其余留白；峰值每章 1–2 次 |
| 情感的验证 | 确定性探针优先，LLM 判官只做章节级辅助；作者裁决最终优先 |
| 情感的持久化 | 角色状态、关系张力、情感债务按拍/章追加，可回溯、可压缩、可作者覆盖 |
| 与现有文笔规则的关系 | `human_voice`/`anti_ai` 管「怎么写才像人」；本设计管「为什么这一拍必须这样写」 |

---

## 2. EmotionCircuits-LLM：精华与迁移边界

### 2.1 论文做了什么

论文《Do LLMs “Feel”? Emotion Circuits Discovery and Control》（arXiv:2510.11328）的七阶段流程：

1. **SEV 数据集**：8 个日常领域 × 20 个中性场景 × 3 种结果极性（正/中/负），共 480 条事件描述；同一场景的参与者、时间、语境相同，只改结果；明确禁止出现「happy/sad」等情绪词。这样情绪差异只能来自事件语义。
2. **情绪方向提取**：对每种情绪的成功生成，取最后一个 token 的残差流；在同一 scenario–event 组内减去六种情绪的均值（视为中性基线），去掉共享语义，得到每种情绪的层间质心；再减全局均值并做单位归一化，得到 attention 方向与 MLP 方向。层 9 开始出现情绪簇，层 12 前后 anger/disgust、sadness/fear 相邻，深层稳定。
3. **方向验证**：把 MLP 方向注入层 11–20 的最后一个 token，steering 成功率 91.22%；prompting 在 SEV 上 98.85%、测试集 98.96%。
4. **局部组件识别**：MLP 神经元用「写向量与情绪方向的点积 × 激活强度」解析分解；注意力头用逐头置零后的投影下降做因果排序。消融 top-k 后情绪分数陡降后平台，随机消融几乎无效。
5. **增强验证**：用组内差分向量（去掉共享语义）注入 top-k 神经元/头，情绪分数陡升后饱和；随机增强无效。说明这些组件不仅必要，而且充分。
6. **逐层重要性**：以层 21–25 的 attention/MLP 方向符号对齐平均作为参考基，逐个子层注入小扰动，测最终隐状态沿参考基的位移；归一化得到每个子层对最终情绪的因果贡献。深层子层余弦相似度普遍 >0.90。
7. **全局电路装配与调制**：预算 = 10 × 56 个子层 = 560；70% 给 MLP、30% 给 attention；每层先给最低配额（MLP 4、attention 2），剩余按重要性分配；每层内选 top 组件。直接调制全局电路，情绪表达准确率 99.65%；surprise 的 steering 只有 67.71%，电路调制达到 100%。跨情绪神经元重叠低、注意力头重叠中等，说明「MLP 情绪特异子电路 + attention 共享传播通路」的双重结构。

### 2.2 五个第一性原理

**原理 1：方向性。** 情绪是状态空间里的方向，不是词表里的标签。两个文本可以都没有「悲伤」二字，但情感方向相反。

**原理 2：稀疏性。** 决定性组件很少。把情绪写成「每个角色都心口一紧、背脊发凉、喉咙发紧」，等价于随机干预所有组件：噪声大、效果差、读者疲劳。

**原理 3：因果性。** 相关性不够，必须做消融/增强。一个情感细节是否有效，不看它写得多美，看抽掉它情感是否塌、放大它情感是否升。

**原理 4：层次性。** 早期层提供情绪线索，后期层实现情绪表达；不同子层的因果权重不同。故事也一样：铺垫期埋情感线索，升级期加压，兑现期才允许峰值；不能每章都在高潮。

**原理 5：电路优于单向量。** 单一「写得更悲伤」的全局指令，效果不如同时调制「角色状态 + 关系张力 + 本拍事件 + 载体配比」。情绪是系统属性，不是一句形容词。

### 2.3 不能直接照搬的部分

| 边界 | 现实 | 本设计的处理 |
|---|---|---|
| 黑盒模型 | StoryMoss 主要用远程 OpenAI 兼容 API，拿不到 hidden states，无法注册 forward hook | 不做激活干预；把「电路」实现在**叙事可观测层**：状态、关系、事件、载体、探针 |
| 英文/六情绪/短文本 | 论文只验证英文与 Ekman 六情绪，生成最长 150 token | 扩展到中文、12 维情绪、长篇；用中文语料校准，不宣称与论文等价 |
| 模型特异性 | 电路是 Llama-3.2-3B/Qwen2.5-7B 的，换模型需重提取 | 叙事层与模型无关；P3 可选为本地 HF 模型做白盒适配 |
| GPL-3.0 | 仓库代码 GPL；StoryMoss 是 ISC | 不复制代码；只借鉴方法论并独立实现；文档注明出处 |
| 99.65% | 是短文本情绪分类准确率，不是「感动读者」 | 本设计区分「目标情感达成」与「读者共鸣」；后者用读者面板 + 人类 A/B，不混为一谈 |

### 2.4 机制 → 叙事映射总表

| 论文机制 | 论文做法 | 叙事层对应 | StoryMoss 落点 |
|---|---|---|---|
| 情绪方向 | 组内减均值、单位化 | `EmotionVector`（12 维，强度独立） | `emotion_circuit::vector` |
| 局部组件 | top-k 神经元/头 | 8 类表达载体，每拍选 2–3 | `emotion_circuit::carriers` |
| 消融 | 置零 top-k | 反事实：抽掉载体，情感是否塌 | `emotion_circuit::probe` |
| 增强 | 注入差分向量 | 反事实：放大载体，目标情感是否升 | 同上 + A/B 草稿 |
| 逐层重要性 | 56 子层因果权重 | 故事位置预算（铺垫/升级/兑现/余波） | `EmotionBeat.scale` + 章级峰值预算 |
| 全局电路 | 560 稀疏预算 | 每拍 2–3 载体 + 一条最高压张力 | `EmotionCircuit` |
| 调制 | 电路 + 差分向量 + scale | 提示词调制块：从→到、载体、禁忌 | `emotion_circuit::modulation` |
| 控制准确率 | 99.65% | 目标情感达成率/因果率/声音一致率/共鸣预测 | 探针 + 读者面板 + 人类 A/B |

---

## 3. 理论内核：情感作为因果发动机

### 3.1 情感因果链（Emotional Causal Chain, ECC）

每一拍必须实例化一条完整的因果链，缺任何一环都视为「情感未落地」：

```
E_t   事件（谁做了什么/发生了什么）
 ↓
A_t   评估（这件事对角色目标意味着什么：相关性、一致性、归因、确定性、可控性、规范性）
 ↓
F_t   情绪（from → to 的向量变化 + 强度）
 ↓
G_t   目标变化（角色现在更想要什么/更怕什么）
 ↓
D_t   选择/行动/台词（行动倾向 + 防御机制 + 台词策略）
 ↓
C_t   代价/后果（失去什么、暴露什么、伤害谁、不可逆什么）
 ↓
F_{t+1} 新情绪（后果被评估后的状态）
 ↓
E_{t+1} 下一个事件（由新情绪和债务逼出来）
```

这条链是**双向**的：情感由事件评估产生，又反过来选择下一个事件。情节不是情感的容器，情节是情感选择的后果。

**写作学对应：**

- 情节 = 角色在情感压力下做出的一系列选择。
- 冲突 = 两个人的情感目标在同一事件上不可兼容。
- 对白 = 角色为调节情绪/说服/隐瞒/伤害对方而采取的语言策略。
- 人物弧光 = 评估偏差的重复 → 危机 → 重新评估 → 行动倾向改变。
- 主题 = 故事对「核心情感问题」的最终回答，体现在最后一次选择上。

### 3.2 三个闭环

**闭环 A：角色心理回路（Character Psyche Loop）**

```
伤口/需求 → 评估偏差 → 触发事件 → 情绪 → 防御 → 行动/台词 → 后果 → 伤口被强化或松动
```

- 伤口决定「什么是威胁」；需求决定「什么值得追求」；评估偏差决定「同一事件为什么他反应过度/过度迟钝」；防御决定「情绪如何被隐藏/表达」。
- 每一拍至少让一个角色的伤口被触碰，或让一个需求被推进/阻断。
- 弧光不是「从悲伤到快乐」，而是「同一触发事件，评估变了，行动选择变了」。

**闭环 B：关系张力回路（Relationship Tension Loop）**

```
A 的需求 vs B 的需求 → 未说出口的真话 → 事件迫使表态 → 张力升级/揭示/修复/破裂 → 信任账本变化 → 下一次事件的赌注变化
```

- 每条关系有两个方向的情感向量、一个信任账本、一个未说出口的真话、一个情感债务。
- 张力 = 方向差异 × 强度 × 赌注 × 积压时间 × 未满足需求。
- 每拍只能选一条最高压的关系线推进；推进方式必须是升级/揭示/修复/破裂之一，不能换说法复述上一次对峙（复用现有 `ConflictStage` 阶梯）。
- 情感债务逾期必须兑现或付出「拖延的代价」；不能无限拖。

**闭环 C：读者参与回路（Reader Engagement Loop）**

```
承诺 → 期待 → 部分兑现 → 升级 → 大兑现 → 新承诺
```

- 故事契约给出「读者最初为什么在乎」；每章给一次部分兑现；每卷给一次大兑现；兑现后必须开新承诺。
- 读者必须比角色多知道一点（信息差），或比角色少知道一点（悬念），但不能全程同步。
- 每章末尾的钩子不是「有人推门进来」，而是「读者在乎的人面临一个读者在乎的问题」。

### 3.3 读者共鸣的五个条件

「感动」不是悲伤的同义词。一个场景要让人类读者产生共鸣，至少满足五个条件中的三个，且第 1、2 条必须满足：

1. **可理解的具体欲望**：读者知道角色这一刻最想要什么，而且这个欲望是具体的、可想象的（不是「想要幸福」）。
2. **可见的代价**：角色为这个欲望付出了读者能看见的东西（尊严、安全、关系、机会、自我认同）。
3. **正当性**：读者理解角色为什么这么做，即使不赞同；或者读者认为角色受到了不公正的对待。
4. **意料之外、情理之中**：选择既让读者意外，又能在前文找到依据。
5. **叙述克制**：叙述者不替读者哭，不解释「这一刻多么感人」，把情感留给动作、台词、留白。

这五条同时是**写作准则**、**编辑审查维度**和**读者面板评分维度**。

### 3.4 与现有表达层规则的分工

| 层 | 现有模块 | 负责 | 本设计的关系 |
|---|---|---|---|
| 驱动层 | 本设计 `emotion_circuit` | 为什么这一拍必须发生、角色为什么这样选 | 新增，位于最上游 |
| 结构层 | `beat_card`/`beat_state`/`continue_assets` | 这一拍必须完成什么、谁在场、什么不能做 | 扩展字段，不改骨架 |
| 表达层 | `human_voice`/`anti_ai`/`prose_lint` | 怎么写才像人、怎么避免 AI 腔 | 复用，不重复造规则 |
| 评审层 | editor/gate/graders | 是否达标 | 增加情感维度与探针 |
| 回流层 | ingest/asset_bridge | 写后资产更新 | 增加动态情感状态与债务 |

**关键边界：** 表达层规则不能替代驱动层。把「情绪四模式混用」写得再好，如果这一拍没有情感因果，读者仍然不会在乎。反过来，驱动层成立时，表达层规则是放大器；驱动层不成立时，表达层规则只是修饰。

---

## 4. 五层领域模型

### 4.1 L0 故事情感契约（Story Emotional Contract）

每部故事一份，作者可编辑，AI 可建议但不可覆盖作者确认版。

| 字段 | 含义 | 示例 |
|---|---|---|
| `core_question` | 全书要回答的情感问题 | 「一个人能否在不失去自我的情况下被爱？」 |
| `reader_promise` | 读者最初被承诺的情感体验 | 「看一个被家族当成工具的人，如何学会为自己活」 |
| `palette` | 主情绪/次情绪/复杂情绪/禁止情绪 | 主：隐忍的愤怒；次：羞耻、渴望；禁：廉价煽情 |
| `ending_residue` | 读完最后一页应留下的情绪余味 | 「苦尽甘来，但代价不可撤销」 |
| `forbidden` | 禁止的情感捷径 | 失忆、误会、绝症、天降救兵、旁白解释 |
| `peak_budget` | 每章允许的情感峰值次数 | 默认 1，最高 2 |
| `genre_profile` | 题材情感契约（言情/悬疑/历史/玄幻） | 由 `reader_promise` 与题材配置生成 |

**与现有资产的承接：** `genre_profiles.reader_promise`（`creative_engine/reader_promise.rs:9-58`）与 `stories.strategy_json.emotional_payoff`（V119）已经是 L0 的雏形；本设计不另起炉灶，而是把它们提升为可编辑、可审查、可被探针引用的显式契约，并补上 `core_question/ending_residue/peak_budget/forbidden`。

**作用：** 所有角色、关系、节拍的情感选择都必须能回答「这与故事契约的关系是什么」。不能回答的，就是无关情感，删掉。

### 4.2 L1 角色情感回路（Character Emotional Circuit）

在现有四元组（`emotional_core/trigger/wound/need`，`characters` 表，`db/models.rs:1418-1424`）之上，补三层：

**静态层（作者可编辑，AI 不可覆盖）：**

| 字段 | 含义 |
|---|---|
| `wound` | 过去发生了什么，让他形成今天的评估偏差 |
| `need` | 他真正需要但不愿承认的东西 |
| `core` | 他用来保护自己的主导情绪/信念 |
| `trigger` | 什么事件会绕过理性直接点燃他 |
| `defense` | 他如何隐藏/表达情绪（攻击、沉默、讨好、理智化、幽默、逃避） |
| `lie` | 他相信的关于自己的假话（「我不需要任何人」） |
| `truth` | 故事要让他学会的真话（「被需要不等于软弱」） |
| `moral_line` | 他绝不会跨过的线；跨过就是弧光转折 |
| `voice_signature` | 用词、句长、正式度、口头禅、回避话题、说谎方式 |
| `display_rules` | 哪些情绪可以外露，哪些必须藏（文化/身份/性别/职业） |
| `appraisal_bias` | 他最容易把中性事件误读成什么（威胁/轻视/抛弃/施舍） |

**动态层（每拍/每章更新，作者可覆盖）：**

| 字段 | 含义 |
|---|---|
| `current_vector` | 当前 12 维情绪向量 |
| `intensity` | 当前主导情绪强度 0–1 |
| `active_goal` | 当前最想要的具体东西 |
| `stress` | 压力/疲惫/恐惧的累积 |
| `trust_in` | 对每个关键关系的信任账本 |
| `secrets` | 他知道但没说的事 |
| `last_appraisal` | 最近一次评估（事件 → 解释） |

**弧光状态机：**

```
Brewing（酝酿） → Escalating（升级） → Crisis（危机） → Transforming（转变） → Resolved（收束）
```

与现有 `ArcStage` 同名，但这次**真的推进**：由每章的情感债务和评估偏差触发，不再恒为 `Brewing`。

### 4.3 L2 关系情感回路（Relationship Emotional Circuit）

在现有双向 `emotional_bond/emotional_intensity/reverse_*`（`character_relationships`，`db/models.rs:404-423`）之上，补：

| 字段 | 含义 |
|---|---|
| `trust` | 信任账本 0–1（可被事件增减） |
| `unspoken_truth` | 一方知道但没说的真话 |
| `debt` | 一方欠另一方的（救命、背叛、牺牲、承诺） |
| `tension_type` | 未揭穿的欺骗、对抗、单方面执念、毁灭倾向、复仇、嫉妒、愧疚 |
| `pressure` | 当前张力 0–1（由方向差异、强度、赌注、积压时间合成） |
| `due_chapter` | 该张力最迟应在第几章兑现 |
| `last_escalated` | 上次升级的章/拍，用于防复述 |
| `repair_window` | 修复窗口（错过就要付更大代价） |

**关系不是一条边，是两个方向。** A 对 B 的爱 0.9、B 对 A 的恨 0.8，这不是「矛盾数据」，这是故事。现有 `load_tensions` 已经能识别这种不对称（`emotional_ledger.rs:178-203`），但压力不累积、不逾期。本设计把它变成真正的账本。

### 4.4 L3 节拍情感回路（Beat Emotional Circuit）

这是每一次续写的最小单位。用 `EmotionBeat` 替换现在的一行 `情感：`。

```rust
pub struct EmotionBeat {
    /// 本拍开始时的情绪状态（主导情绪 + 强度）
    pub from: EmotionVector,
    /// 本拍结束时的情绪状态
    pub to: EmotionVector,
    /// 触发事件：谁做了什么/发生了什么
    pub trigger_event: String,
    /// 评估：角色如何解释这件事（OCC 维度）
    pub appraisal: Appraisal,
    /// 行动倾向：战斗/逃跑/冻结/讨好/接近/退缩/坦白/隐瞒/惩罚/牺牲/求助
    pub action_tendency: ActionTendency,
    /// 台词策略：本拍对话要达成什么、用什么手段、藏什么
    pub dialogue_policy: Option<DialoguePolicy>,
    /// 代价：角色为这个选择失去什么
    pub cost: String,
    /// 本拍结束后谁的状态变了
    pub state_changes: Vec<StateChange>,
    /// 载体配比：只选 2–3 项
    pub carriers: CarrierPlan,
    /// 读者目标：读者应该产生什么感受、为什么
    pub reader_target: ReaderTarget,
    /// 强度档位：0.4 克制 / 0.8 标准 / 1.2 峰值（每章峰值 ≤2）
    pub scale: f32,
}
```

**硬约束：**

1. `from != to`：本拍必须发生情感变化；变化可以是「加深」「转向」「压抑」「爆发」「麻木」，但不能是「保持不变」。
2. `trigger_event` 必须是一个具体事件，不能是「她想起了往事」这种无事件。
3. `action_tendency` 必须能被拍出来（可见动作/可听台词），不能是「内心挣扎」。
4. `cost` 必须具体：失去什么、暴露什么、伤害谁、不可逆什么。
5. `carriers` 最多 3 项，至少 2 项；不允许八项全选。
6. `scale=1.2` 每章最多 2 拍；连续两拍不得都是 1.2。

### 4.5 L4 表达载体（Prose Carriers）

论文的「局部组件」在小说层的对应物。每类载体都有确定性检测特征和提示词写法。

| 载体 | 定义 | 适合的情绪 | 检测特征 |
|---|---|---|---|
| `action` | 角色做了什么可见动作 | 愤怒、恐惧、爱、愧疚 | 动词密度、及物动作、身体移动 |
| `dialogue_subtext` | 台词表面说 A、实际要 B | 羞耻、爱、嫉妒、愧疚 | 问句/命令/回避/转移话题/言外之意 |
| `choice` | 角色在两种代价之间做选择 | 全部 | 选择连词、取舍、放弃、坚持 |
| `consequence` | 选择造成的不可逆后果 | 悲伤、恐惧、希望 | 结果句、损失、关系变化 |
| `sensory` | 具体感官细节（不是情绪形容词） | 恐惧、怀旧、亲密 | 五感词、具体物件、温度/气味/声音 |
| `interiority` | 内心活动（限量） | 羞耻、犹豫、自我欺骗 | 想法动词、反问、内心独白 |
| `silence` | 不说话、停顿、留白 | 悲伤、压抑、愧疚 | 省略号、短句成段、动作中断 |
| `rhythm` | 句长/段落节奏变化 | 紧张、崩溃、释然 | 句长方差、短句连发、段落长度 |

**稀疏分配原则：**

- 每拍只选 2–3 项，其中至少一项是 `action` 或 `choice`（情感必须落到行动）。
- `interiority` 每章最多 2 次主载体；`sensory` 不得连续两拍做主载体；`silence` 只在情绪压制/峰值后使用。
- 同一角色连续两拍不得使用完全相同的载体组合。

### 4.6 L5 读者共鸣模型（Reader Resonance Model）

不是「读者真的哭了」，而是「在一个可解释的读者模型里，这个场景是否有条件让读者产生共鸣」。它是**预测**，不是**测量**。

**读者状态：**

```rust
pub struct ReaderState {
    pub identification: f32, // 认同：我理解这个角色想要什么
    pub sympathy: f32,       // 同情：他值得更好的/他受了不公正
    pub investment: f32,     // 投入：我在乎结果
    pub tension: f32,        // 紧张：结果不确定
    pub curiosity: f32,      // 好奇：我想知道为什么/接下来
    pub trust: f32,          // 信任：作者不会骗我/不会廉价煽情
}
```

**四类读者人格（章节级后台评审，不阻塞正文）：**

| 人格 | 关注点 | 会被什么劝退 |
|---|---|---|
| 类型读者 | 爽点、节奏、关系张力 | 拖沓、说教、主角憋屈太久 |
| 人物读者 | 角色一致性、情感真实 | OOC、工具人、强行误会 |
| 文学读者 | 语言、留白、主题 | 直白解释、陈词滥调、情绪过载 |
| 挑剔读者 | 逻辑、因果、伏笔 | 巧合、降智、未兑现承诺 |

**「感动」判定四问（同时用于提示词和判官）：**

1. 读者是否知道角色这一刻最想要什么、最怕什么？
2. 角色为这个想要付出了什么可见代价？
3. 这个选择是否既出人意料、又能在前文找到依据？
4. 叙述是否克制（没有替读者哭、没有解释主题）？

四问中 1、2 必须为「是」，3、4 至少一个为「是」，否则不认定为「感动场景」。

### 4.7 Rust 数据结构（核心）

```rust
/// 12 维情绪向量：六种基本情绪 + 六种社会情绪。
/// 权重和归一化为 1；强度单独存储，避免「强度」被向量长度重复表达。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmotionVector {
    pub joy: f32,
    pub sadness: f32,
    pub fear: f32,
    pub anger: f32,
    pub disgust: f32,
    pub surprise: f32,
    pub love: f32,
    pub trust: f32,
    pub guilt: f32,
    pub shame: f32,
    pub pride: f32,
    pub hope: f32,
}

impl EmotionVector {
    pub fn normalized(&self) -> Self;
    pub fn dominant(&self) -> (EmotionKind, f32);
    pub fn valence(&self) -> f32;   // 由正负情绪加权
    pub fn arousal(&self) -> f32;   // 由高唤醒情绪加权
    pub fn distance(&self, other: &Self) -> f32; // 1 - cosine
    pub fn delta(&self, other: &Self) -> Self;   // 调制用的差分向量
    pub fn blend(&self, other: &Self, t: f32) -> Self;
}

/// OCC 评估维度。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appraisal {
    pub relevance: f32,      // 与我目标的相关性 0-1
    pub congruence: f32,     // 与目标一致/不一致 -1..1
    pub agency: Agency,      // 自己/他人/环境
    pub certainty: f32,      // 确定性 0-1
    pub control: f32,        // 可控性 0-1
    pub norm_violation: f32, // 违背我的底线/规范 0-1
    pub attachment: f32,     // 对方对我的重要性 0-1
}

pub enum Agency { SelfAgent, Other(String), Circumstance }

pub enum ActionTendency {
    Approach, Withdraw, Attack, Freeze, Appease, Confess,
    Conceal, Help, Sacrifice, SeekContact, Punish, Betray,
}

pub struct DialoguePolicy {
    pub objective: String,   // 这场对话我想要对方做什么
    pub tactic: String,      // 我用什么手段（质问/示弱/转移/诱惑/威胁/沉默）
    pub subtext: String,     // 我真正想说但没说的话
    pub forbidden: Vec<String>, // 这个角色绝不会说的话
}

pub struct CarrierPlan {
    pub action: f32,
    pub dialogue_subtext: f32,
    pub choice: f32,
    pub consequence: f32,
    pub sensory: f32,
    pub interiority: f32,
    pub silence: f32,
    pub rhythm: f32,
}

pub struct ReaderTarget {
    pub primary: ReaderEmotion, // 心疼/愤怒/不安/释然/敬畏/羞耻/希望
    pub secondary: Option<ReaderEmotion>,
    pub why: String,
    pub restraint: String, // 叙述必须克制的地方
}
```

**序列化策略：** 向量、评估、载体配比全部存 JSON 文本列，避免 12×N 的宽表；查询用 story_id/chapter 索引，不解析 JSON。与现有 `characters.attributes`、`story_contracts` 的 JSON 风格一致。

---

## 5. 核心算法

### 5.1 情绪向量与中文情绪空间

**两层提取：**

1. **确定性词典层（0 LLM）**：中文情绪词表 + 程度副词 + 否定 + 转折 + 行动动词 + 身体反应词。按句切分，计算 12 维分布；`valence/arousal` 由维度加权。输出 `confidence`；低置信度不参与门禁。
2. **LLM 结构化层（P1 起，按需）**：只对「情感转折候选拍」做 JSON 精修，输出 `from/to`、`trigger`、`appraisal`、`action_tendency`、`cost`。优先复用章纲/后台已有调用，结果按内容哈希缓存；P0 不调用。

**为什么要 12 维而不是 6 维：** Ekman 六情绪覆盖不了中文小说最重要的「爱、信任、愧疚、羞耻、自尊、希望」。论文自己也在 Limitations 里承认六情绪是限制。12 维是「六基本 + 六社会」，仍然可解释、可评测，不引入黑箱 embedding。

**校准：** 用 `resources/voice_corpus/` 的中文公版小说 + 项目内人类样本，校准每维词表的 Precision/Recall；只采用方向一致的词，方向矛盾的不入规则（沿用 v0.65.0 人类文笔基线的方法论）。

### 5.2 评估引擎（OCC）

情感不是事件的直接函数，是**评估**的函数。同一事件，不同角色、不同关系、不同目标，产生不同情绪。

**规则表（节选，完整表见附录 C）：**

| 事件类型 | 评估条件 | 情绪 | 行动倾向 |
|---|---|---|---|
| 目标受阻 | 他人可控、违背规范 | anger | attack / punish |
| 目标受阻 | 自己可控、违背自己的标准 | guilt / shame | confess / withdraw / appease |
| 目标受阻 | 环境不可控、确定 | sadness | withdraw / seek_contact |
| 威胁 | 高相关、低可控、不确定 | fear | freeze / escape / appease |
| 失去重要对象 | 高依恋、不可逆 | sadness + love | seek_contact / sacrifice |
| 被重要对象接纳 | 高依恋、目标一致 | joy + love + trust | approach / help |
| 被重要对象背叛 | 高依恋、他人可控、违背规范 | anger + shame + disgust | attack / betray / conceal |
| 公开暴露弱点 | 高规范、低控制、有观众 | shame | conceal / appease / attack |
| 达成被认可的目标 | 高相关、自己可控、有观众 | pride + joy | approach / help |
| 未来仍有可能 | 高相关、部分可控 | hope | approach / persist |

**实现方式：**

```rust
pub fn appraise(
    event: &StoryEvent,
    character: &CharacterEmotionalCircuit,
    relationship: Option<&RelationshipEmotionalCircuit>,
    contract: &StoryEmotionalContract,
) -> Appraisal;
```

P0 只用纯 Rust 规则给出 `Appraisal` 与候选情绪，保证零新增调用、可回归；P1 起才在「规则置信度低」或「复杂情绪混合」时，复用章纲/后台现有调用做精修，不新增续写热路径调用。

**反标签规则：** 如果正文只写「她很愤怒」而没有触发事件、评估、行动倾向、代价中的至少两项，探针直接判「情感空转」。

### 5.3 情感压力场与下一拍选择（emotion-first planning）

**现状：** `compile_conflict` 从敌对关系里选一对，`compile_emotion` 选一个静态画像，`compile_next_node` 从大纲里找下一个未写过的句子。三者各选各的，情感不决定情节。

**新算法：情感压力优先的下一拍选择。**

```
输入：故事契约、角色状态、关系张力、情感债务、故事大纲、章节大纲、当前正文
输出：EmotionBeat + 事件节点 + 阵容 + 冲突

1. 构建压力场：
   - 节点压力 = 角色需求受阻程度 × 伤口被触碰程度 × 当前情绪强度
   - 边压力   = 关系方向差异 × 赌注 × 积压拍数 × 逾期程度
   - 债务压力 = 逾期情感债务 × 不可逆程度
2. 选出最高压的「角色-关系-债务」组合（只选一个主组合）。
3. 生成候选事件：
   - 从章节大纲的未完成节点中取候选；
   - 每个候选事件必须能让主组合的两个角色同时做出评估；
   - 如果没有候选能同时施压，允许「大纲内的新事件」，但必须服务于大纲节点。
4. 对每个候选事件做评估：
   - 主角色 appraisal → emotion delta → action tendency → cost
   - 对手 appraisal → emotion delta → action tendency → cost
   - 计算情感转折幅度 |to - from|、关系压力变化、读者投入变化
   - 惩罚与最近 3 拍重复的事件/冲突形式
5. 选取得分最高且能推进大纲的候选；若全部低于阈值：
   - 优先「升级现有张力」（不换事件，换代价）；
   - 其次「兑现逾期债务」；
   - 最后才允许「新事件」，且必须标记为「外部事件驱动」，下一拍必须回到角色情感。
6. 编译 EmotionBeat：from/to、trigger、appraisal、action_tendency、dialogue_policy、cost、carriers、reader_target、scale。
```

**评分函数（可调，P0 先保守）：**

```
score = 0.30 * emotion_delta
      + 0.25 * relationship_pressure
      + 0.15 * debt_due
      + 0.15 * reader_investment
      + 0.10 * plot_advance
      + 0.05 * freshness
      - 0.30 * repetition_penalty
```

**关键约束：** 情感不能推翻大纲。情感压力只决定「在大纲允许的候选里选哪一个、以什么代价发生」，不能凭空发明与大纲冲突的情节。这是「情感驱动」而不是「情感失控」。

### 5.4 冲突引擎：两种正当性的对撞

好的戏剧冲突不是「好人打坏人」，是**两个人从各自的伤口和需求出发，都认为自己是对的**。

```rust
pub struct Conflict {
    pub kind: ConflictKind,          // 资源/价值/忠诚/身份/秘密/牺牲
    pub a_goal: String,
    pub b_goal: String,
    pub a_justification: String,     // 从 A 的伤口/需求看，他为什么正当
    pub b_justification: String,
    pub incompatible_because: String,
    pub stakes: String,              // 输的人失去什么
    pub escalation_ladder: [&'static str; 4], // 加压/升级/结账/余波
}
```

**生成规则：**

1. 从 `character_emotional_circuit` 取两个角色的 `active_goal` 与 `moral_line`。
2. 判断目标是否在同一事件上不可兼容（资源稀缺、价值对立、忠诚分裂、身份冲突、秘密保护、牺牲选择）。
3. 为双方各写一句「正当性」：不是「他坏」，是「他如果不这样，就会再次经历他的伤口」。
4. 设置赌注：输的人失去什么（不是抽象的「失败」，是具体的失去）。
5. 复用现有 `ConflictStage` 阶梯，防止连拍复述。

**反模式：** 为了冲突而让角色降智、强行误会、突然变坏。探针检查「冲突是否有前文依据」「双方是否都有正当性」；没有依据的冲突判为「廉价冲突」。

### 5.5 稀疏载体分配

论文的稀疏电路对应到写作：**每拍只动 2–3 个载体**。

**分配算法：**

```
输入：EmotionBeat（from/to、action_tendency、defense、relationship、scene）
输出：CarrierPlan（8 维权重，非零项 ≤3）

1. 基础映射：情绪 → 首选载体
   - anger → action / dialogue_subtext / choice
   - fear → sensory / action / silence
   - sadness → silence / consequence / sensory
   - shame → conceal（dialogue_subtext / interiority）/ silence
   - love → action（照顾）/ dialogue_subtext / choice
   - guilt → consequence / action（补偿）/ confession
   - hope → choice / action / dialogue_subtext
2. 防御修正：
   - 攻击型防御 → 提高 action/dialogue_subtext
   - 回避型防御 → 提高 silence/interiority
   - 讨好型防御 → 提高 dialogue_subtext/action（服务）
   - 理智化防御 → 提高 dialogue_subtext/interiority（分析）
3. 场景修正：
   - 公共场合 → 降低 interiority，提高 dialogue_subtext/silence
   - 私密场合 → 允许 interiority/sensory
   - 动作场 → 提高 action/consequence/rhythm
4. 稀疏化：取权重最高的 3 项，重新归一化；若最高项 <0.2，补一个 action/choice。
5. 连续性检查：与最近 2 拍的主载体组合不得完全相同；interiority 每章 ≤2 次主载体。
```

**提示词只写选中的载体，不写未选中的。** 未选中的载体不是「禁止」，而是「本拍不需要重点写」；这样模型有留白，读者有想象空间。

### 5.6 调制器：把回路编译成提示词

调制器输出一个 ≤600 字的 `【本拍情感回路】` 块，插在节拍卡之后、状态网之前（动态区，不破坏稳定前缀缓存）。

**调制强度三档：**

| 档位 | scale | 用途 | 约束 |
|---|---|---|---|
| 克制 | 0.4 | 日常推进、暗流、伏笔 | 只写载体，不做情绪命名；读者自己感觉 |
| 标准 | 0.8 | 大多数冲突/关系推进 | 允许一次平直命名，允许一次具身化 |
| 峰值 | 1.2 | 每章 1–2 次的情感兑现 | 允许短句、留白、动作中断；仍禁止解释主题 |

**峰值预算：** 每章最多 2 拍 `scale=1.2`；连续两拍不得都是 1.2；峰值后必须有一拍「余波」（scale≤0.4），否则读者疲劳、情感贬值。

**前缀缓存：** 故事契约、角色静态画像属于稳定区，放在 bundle 前部；当前情绪、关系压力、本拍因果链属于动态区，放在 `next_node` 之后。遵守 v0.74.3 的 stable/dynamic 拆分契约（`continue_assets.rs:986-1006`）。

### 5.7 确定性探针（0 LLM）

| 探针 | 问题 | 判定 | 严重度 |
|---|---|---|---|
| `emotion_causality` | 情绪是否有事件、评估、行动、代价支撑？ | 四要素缺 ≥2 → fail | high |
| `emotion_target` | 正文实际情绪分布是否接近 `to`？ | cosine < 0.35 → fail；0.35–0.5 → warn | medium |
| `emotion_delta` | 本拍是否真的发生变化？ | `distance(from, actual) < 0.1` → fail | high |
| `voice_consistency` | 台词是否符合角色 `voice_signature`？ | 特征距离 > 阈值或判官否定 → warn/fail | high |
| `tension_payoff` | 本拍是否推进了选定张力？ | 未升级/揭示/修复/破裂 → fail | high |
| `melodrama` | 是否情绪过载、无代价、连续具身化？ | 命中 ≥2 条 → fail | medium |
| `carrier_sparsity` | 是否八项全写、每段都在抒情？ | 主载体 >3 或 interiority 超预算 → warn | low |
| `reader_restraint` | 是否替读者哭、解释主题？ | 命中「她终于明白/这一刻她懂了/泪水说明了一切」→ warn | medium |

**关键原则：**

- P0 全部 advisory，只记录不阻塞；连续 2 周真实章节校准后，P1 才把 high 级纳入规则分。
- 探针必须能引用原文证据（句号级），不能只给「情感不足」这种无法行动的结论。
- 探针阈值用人类/机器对照样本校准，不用拍脑袋；校准报告进 `docs/audits/`。

### 5.8 读者面板与「感动」判定

**章节级后台评审**（不阻塞正文上屏）：

1. 取本章正文 + 故事契约 + 角色卡 + 本章情感目标。
2. 四个读者人格各输出 JSON：

```json
{
  "persona": "character_reader",
  "scores": {
    "identification": 0.8,
    "sympathy": 0.7,
    "investment": 0.75,
    "tension": 0.6,
    "curiosity": 0.65,
    "trust": 0.8,
    "moved": 0.62
  },
  "moved_moment": {"quote": "……", "why": "她明知道会被赶出去，还是把伞留在了门口"},
  "flat_moment": {"quote": "……", "why": "这段只是解释她很难过，没有选择"},
  "verdict": "promising | flat | unearned | melodramatic"
}
```

3. 聚合：取四人格中位数；`moved` 中位数 <0.5 或两个以上人格判 `unearned` → 本章情感预警。
4. 只报不改：结果进质量债与「创作评估」页，不自动改稿；作者可点「按此重写这一拍」。

**成本控制：** 读者面板每章一次，4 个短 JSON 调用，可走后台模型；不进入续写热路径。P2 才接入。

### 5.9 反事实验证（叙事版 ablation / enhancement）

论文用消融/增强证明因果，小说层用**反事实草稿**：

- **消融：** 把本拍的主载体（如「代价」）从提示词里删掉，重写一版；如果读者面板 `moved` 显著下降，说明该载体是必要的。
- **增强：** 把主载体权重从 0.3 提到 0.5，重写一版；如果 `moved` 上升且 `melodrama` 不升，说明增强有效。
- **随机对照：** 换一个无关载体（如「感官」）做同样操作；如果效果不明显，说明不是所有载体都等价。

**用途：** 校准载体映射表、验证探针、给作者提供「这一拍为什么有效」的解释。不作为每拍常规流程，只在 P2 的 A/B harness 里跑。

---

## 6. 与现有架构的融合

### 6.1 模块与依赖方向

新增顶层领域模块：

```
src-tauri/src/emotion_circuit/
├── mod.rs           // 公共 API、错误类型、版本
├── vector.rs        // EmotionVector、EmotionKind、valence/arousal
├── appraisal.rs     // OCC 评估、行动倾向、规则表
├── state.rs         // CharacterEmotionState、RelationshipTension、EmotionDebt
├── circuit.rs       // 压力场、下一拍选择、冲突引擎
├── carriers.rs      // 载体映射、稀疏分配
├── modulation.rs    // 提示词调制块编译
├── probe.rs         // 确定性探针
├── reader.rs        // 读者面板数据结构与聚合
├── repo.rs          // DB 读写（依赖 db）
└── tests.rs         // 领域单测 + 契约测试
```

**依赖方向：**

```
emotion_circuit (纯领域，只依赖 db/domain/serde)
   ↑                         ↑
agency (消费)          story_system (消费)
   ↑                         ↑
commands / handlers (IPC)
```

- `emotion_circuit` **不得**依赖 `agency`；避免现有 `agents → agency` 循环引用扩大（见 `docs/audits/2026-10-06-project-review-v0.58.0.md:82`）。
- LLM 判官通过 trait 端口注入（`ReaderJudge`），生产实现在 `agency`，测试用 mock。
- 所有纯函数（vector/appraisal/carriers/probe）必须可在 `create_test_pool()` 之外单测，不碰网络。

### 6.2 V150 迁移

最新迁移是 V149（`db/migrations/mod.rs:1160,1255`）。本设计新增 **V150**，全部 additive，无破坏性变更。

| 表 | 用途 | 写入方 | 读取方 |
|---|---|---|---|
| `story_emotion_contracts` | 故事情感契约 | 作者编辑 / 创世建议 | 续写、章纲、评审 |
| `character_emotion_states` | 角色动态情绪状态（按场景追加） | ingest、commit、作者 | 续写、探针、前端 |
| `scene_emotion_beats` | 每拍情感因果链 | 续写编译、commit | 下一拍、评审、前端 |
| `emotion_tensions` | 关系张力账本 | ingest、commit、作者 | 压力场、续写 |
| `emotion_debts` | 情感债务（未兑现承诺/未修复裂痕） | commit、评审 | 压力场、续写 |
| `reader_resonance_reviews` | 读者面板结果 | 后台评审 | 创作评估、质量债 |
| `emotion_probe_log` | 探针结果 | 续写、评审 | 质量债、校准 |

SQL 草案见附录 F。迁移遵循现有约定：

- SQL 纯 DDL 放 `V150__emotion_circuit.sql`；需要回填的逻辑用 Rust migration。
- 所有列可空/有默认值；旧故事不迁移也能跑。
- `character_emotion_states` 按 `(story_id, chapter_number)` 索引；只保留最近 N 章 + 摘要，旧的压缩进 `story_summaries`，防止长篇膨胀。
- 作者手工编辑的静态画像（`characters.emotional_*`）永不被自动覆盖；动态状态可被 ingest 更新，但 `source='author'` 的行优先。

### 6.3 续写主路径改造（`write_beat_once`）

现有顺序（`agency/coordinator.rs:4908`）：

```
compile_beat_card → load context → generate chapter outline → admit → director lock
→ continuity → retrieval → render assets → compile BeatState → render prompt
→ LLM write → sanitize → probes → persist
```

改造后：

```
1. 载入故事情感契约 + 角色情感回路 + 关系张力 + 情感债务
2. emotion_circuit::circuit::select_next_beat(...)
   → 输出 EmotionBeat（from/to、trigger、appraisal、action、cost、carriers、reader_target、scale）
3. compile_beat_card 用 EmotionBeat 替换/扩展静态「情感：」行
   - conflict 由情感压力场的最高压关系决定，而不是先按 hostile 关键词选
   - next_node 必须与事件候选一致
4. compile_beat_state 增加每个在场角色的情绪快照与张力快照
5. render_assets 在动态区插入【本拍情感回路】块（≤600 字）
6. render_writer_user_prompt 保持 stable bundle 在前、动态块在后
7. LLM write
8. probe_increment_ex 增加 emotion_causality / target / delta / voice / tension / melodrama
9. 失败时：优先重写「缺失的因果环」，而不是简单重试全文
10. commit 时写入 scene_emotion_beats + character_emotion_states + 更新 tension/debt
```

**失败重试策略：**

- `emotion_causality` fail → 在重试指令中补「本拍必须写出：事件 → 评估 → 行动 → 代价」。
- `voice_consistency` fail → 附上该角色「绝不会说的话」与「本拍台词目标/潜台词」。
- `tension_payoff` fail → 附上「本拍必须升级/揭示/修复/破裂的具体张力」。
- `melodrama` fail → 降低 scale 一档，要求留白与克制。
- 最多重试 1 次；仍失败则落稿但记质量债（P0/P1），不阻塞作者。

### 6.4 章纲/卷纲：emotion-first planning

**现状：** 章纲只规划戏剧目标、关键事件、出场角色、场景设定、与大纲衔接（`scene_outline.md:48-53`），没有情感目标。

**改造：** 在 `scene_outline.md` 输出要求中新增（无情感契约时整块不出现，保证旧故事逐字不变）：

```markdown
6. **情感转折**：
   - 本场开始：谁带着什么情绪/强度/未说出口的话进入？
   - 本场触发：什么事件触碰了谁的伤口/需求？
   - 本场结束：谁的情绪/关系/目标发生了什么不可逆变化？
7. **读者目标**：读者在本场应产生什么感受？为什么？叙述必须克制在哪里？
8. **情感债务**：本场兑现哪笔旧债、新开哪笔债？
```

**卷纲**（`story_system/volume_plan.rs`）新增：

- `emotional_arc`：本卷主角/关系的情感弧光阶段（Brewing → Escalating → Crisis → Transforming → Resolved）。
- `payoff_plan`：本卷必须兑现的情感债务/关系张力，按章分配。
- `peak_budget`：本卷允许的峰值章（通常 1–2 章）。

**原则：** 章纲先定「情感转折」，再定「什么事件能制造这个转折」。关键事件必须服务于情感转折，而不是反过来。

### 6.5 质检门

**编辑审查新增第 9 维 `emotion`：**

```
9. 情感回路：本场是否有清晰的「事件→评估→情绪变化→行动/台词→代价」？
   情感是否由事件引起，而不是旁白宣布？
   角色的反应是否符合其伤口/需求/防御，而不是作者需要？
   是否克制（没有替读者哭、没有解释主题）？
   是否出现情感过载（每段都在抒情、每个情绪都具身化）？
```

**`dimension_scores` 增加 `emotion: 1-5`。** 编辑输出的总 `score` 仍按现有契约解析（`ModelGraderReport::from_verdict` 把 1–5 映射到 0–1），`emotion` 维度先用于审查展示与质量债；P1 才评估是否把 `model_score` 改为 `0.8 × overall + 0.2 × emotion_normalized`。Gate 顶层权重保持 `0.2 code / 0.3 rule / 0.5 model`、阈值 0.75 不变（`agency/gate.rs:20-32`），避免破坏 Gate v2 契约与回归测试。

**规则分（rule grader）内部调整：**

```
rule = 0.40 * contract
     + 0.30 * reading_power
     + 0.30 * emotion_rule_score
```

`emotion_rule_score` 由确定性探针合成（因果率、目标达成、张力兑现、声音一致、melodrama 反向）。P0 只记录不参与；P1 以 0.2 权重试运行；P2 校准后到 0.3。

**阻断策略：**

- P0/P1：情感探针只进质量债，不阻断。
- P2：仅当「`emotion_causality` fail + `tension_payoff` fail + 编辑第 9 维 ≤2」同时成立时，才阻断；单点失败不阻断，避免误杀。
- 作者手动「定稿」永远可以覆盖门禁（现有产品原则）。

### 6.6 资产回流

**现有 ingest** 抽取静态四元组、双向 bond、sentiment（`memory_content_analysis.md:41-59,72-75`），但只填不改（`asset_bridge.rs:77-108`），且 `character_states.current_emotion` 与 V116 字段存在「双写分裂」（见 `db/migrations/V014`、`V116`、`memory/ingest.rs:1677-1752`）。

**改造：**

1. `memory_content_analysis.md` 增加 `emotion_delta`：

```json
{
  "emotion_delta": {
    "character": "林雪",
    "from": {"shame": 0.6, "anger": 0.3},
    "to": {"shame": 0.4, "anger": 0.6},
    "trigger_event": "阿岩当众替她承认了那封信",
    "appraisal": {"relevance": 0.9, "congruence": -0.4, "agency": "other:阿岩", "certainty": 0.8, "control": 0.2, "norm_violation": 0.1, "attachment": 0.9},
    "action_tendency": "conceal",
    "cost": "她欠了他一个无法还的人情",
    "reader_effect": "心疼 + 不安"
  }
}
```

2. `asset_bridge` 新增 `sync_emotion_delta`：
   - 写 `character_emotion_states`（append-only）。
   - 更新 `emotion_tensions` 的 pressure/accumulated/last_escalated。
   - 新开/结算 `emotion_debts`。
   - 绝不覆盖 `source='author'` 的静态画像与作者锁定状态。
3. **一次 ingest 调用完成**，不新增 LLM 调用；复用现有 Step1/Step2 结果。失败静默，不影响正文落库。
4. 修复 `current_emotion` 双写分裂：P1 先双写并给每行标 `source`（ingest/commit/author），读取时以 `source` 优先级合并；P2 再把 `character_emotion_states` 收敛为唯一动态真源，`character_states.current_emotion` 降级为投影视图，避免两个 writer 互相覆盖。此项列为 P1/P2 的独立子任务，不与情感主链同时上线。

### 6.7 提示词资产

| 资产 | 类型 | 用途 | 是否用户可覆盖 |
|---|---|---|---|
| `resources/prompts/agency/agency_emotion_circuit_system.md` | system | 情感回路总则（因果链、稀疏、克制、禁忌） | 是 |
| `resources/prompts/agency/agency_emotion_judge.md` | system | 读者面板四人格 + 感动四问 | 是 |
| `resources/prompts/agency/agency_emotion_probe_fix.md` | system | 探针失败后的定向重写指令 | 是 |
| `resources/prompts/planner/scene_outline.md` | planner | 新增情感转折/读者目标/债务 | 是（现有资产） |
| `resources/prompts/agency/agency_editor_auditor_system.md` | system | 新增第 9 维 emotion | 是（现有资产） |
| `resources/prompts/memory/memory_content_analysis.md` | pipeline | 新增 emotion_delta | 是（现有资产） |
| `CONTINUE_BEAT_SYSTEM`（内联） | system | 主续写路径，暂不改为可覆盖；追加一条「情感因果」clause | 否（保持现状） |

**为什么主 system 不改成可覆盖：** 现有契约测试锁定 `CONTINUE_BEAT_SYSTEM` 的行数与关键句（`prompts/assembly.rs:307-527`），且用户覆盖主 system 会带来不可控的回归。P2 可评估迁移到 registry，但不在 P0/P1 做。

### 6.8 前端与 IPC

**幕后代理工作室（Agency Studio）：**

- 右栏新增「情感」页签（与现场/黑板/时间线并列）：
  - 故事情感契约卡（可编辑 core_question / reader_promise / palette / ending_residue / forbidden）。
  - 当前章情感曲线（每拍 from→to，强度，载体）。
  - 关系张力账本（压力、积压拍数、due_chapter、状态）。
  - 逾期情感债务（可「兑现」「延期并付代价」「忽略」）。
  - 读者面板结果（四人格、moved 分数、引用证据）。
- 现有「现场」增加「本拍情感回路」折叠块：事件→评估→情绪→行动→代价。

**创作评估（AgencyEval）：**

- 情感目标达成率趋势。
- 情感因果率、张力按时兑现率、声音一致率。
- 读者 panel `moved` 中位数。
- 探针误报/漏报统计（用作者后续修改作为弱标签）。

**角色页：**

- 角色编辑弹窗补四元组编辑（现在只有关系表单能编辑 bond；`CharacterEditModal.tsx` 没有 `emotional_*`）。
- 新增「情感回路」只读卡：伤口/需求/防御/谎言/真话/声音签名/当前状态/弧光阶段。
- 关系表单已有双向 bond + 强度；增加 trust/debt 的只读展示与「手动修正」。

**幕前：**

- 不增加打断式 UI。仅在文思面板/底部状态栏提供可选「情感曲线」轻提示（默认关闭）。
- 不显示分数，不显示「AI 觉得这里该哭」；作者写作时不需要被评审干扰。

**IPC 命令：**

| 命令 | 用途 |
|---|---|
| `get_story_emotion_contract` | 读取契约 |
| `update_story_emotion_contract` | 作者保存契约 |
| `get_emotion_circuit_snapshot` | 当前章/拍的情感回路快照 |
| `preview_next_emotion_beat` | 预览下一拍情感选择（不落库） |
| `list_emotion_tensions` | 张力账本 |
| `resolve_emotion_debt` | 兑现/延期/忽略 |
| `run_reader_panel` | 手动触发章节读者面板 |
| `get_reader_resonance_reviews` | 读取历史结果 |

按现有约定：命令加 `#[tauri::command(rename_all = "snake_case")]`，注册到 `handlers.rs`，前端 `services/api/agency.ts` 加 `loggedInvoke` 包装，`e2e/mock-tauri.ts` 补 handler，`scripts/verify-ipc-manifest.py` 通过。

---

## 7. 提示词与契约

### 7.1 【本拍情感回路】块（示例）

插在节拍卡之后、状态网之前，≤600 字：

```text
【本拍情感回路（硬约束：情感必须由事件引起，并改变下一步行动）】
故事情感契约：一个人能否在不失去自我的情况下被爱？
本拍因果链：
  事件：阿岩当众替林雪承认了那封信是她写的。
  评估：林雪把这件事视为「他在替我承担，也在替我定义我是谁」——与她「不被任何人可怜」的底线冲突；对方可控，有观众，确定性高。
  情绪：羞耻 0.6 + 愤怒 0.3 → 羞耻 0.4 + 愤怒 0.6（转向愤怒，但愤怒是防御）。
  行动倾向： conceal（先否认，再反击）。
  台词：目标=让他收回那句话；手段=冷嘲；潜台词=「你凭什么替我决定」；绝不说=「谢谢」。
  代价：她当众推开他，坐实了「不识好歹」，也伤了他。
  新状态：她欠他一个无法还的人情；关系张力 +0.2。
关系张力：林雪 → 阿岩「未揭穿的感激」压力 0.7，积压 3 拍，本拍必须接近暴露或加深。
载体配比（只选 3 项，其余留白）：
  1. 动作（必须）：她把他递来的伞推回去，伞骨磕在门槛上。
  2. 台词潜台词（必须）：一句冷嘲，表面拒绝，实际在确认他会不会走。
  3. 代价/后果（必须）：他沉默离开；旁人开始议论。
读者目标：先替林雪难堪，再对她的反击产生复杂同情；禁止旁白解释「她其实很感动」。
禁忌：不得直接写「她很痛苦」；不得让所有人轮流表态；不得用天气映心情；不得让阿岩说出「我都是为了你」。
强度：0.8（标准）。
```

**这块的四个作用：**

1. 给模型一个**可执行的因果链**，而不是「写得更悲伤」。
2. 给模型**稀疏载体**，防止八项全写。
3. 给模型**台词策略**，让对白有目标、有潜台词、有禁忌。
4. 给探针**可检查的锚点**（事件、评估、行动、代价、载体）。

### 7.2 角色声音/防御卡（注入角色卡）

```text
【林雪｜情感回路】
伤口：幼年被父亲当众否认「你不是我的女儿」。
需求：被看见，但不被可怜。
防御：攻击型——先推开，再偷偷确认对方有没有走。
谎言：「我不需要任何人。」
真话：「我需要有人留下来，但不要替我决定。」
底线：不欠人情；欠了就必须还。
声音：短句，少形容词；被逼急时反问；从不说「谢谢」，说「知道了」。
情绪外露规则：愤怒可以露，羞耻绝不露；越羞耻越冷。
本拍台词策略：目标=让他收回那句话；手段=冷嘲；潜台词=「你凭什么替我决定」；绝不说=「谢谢」。
```

**探针检查：**

- 台词是否出现「绝不说」的词？出现即 voice fail。
- 句长/问句比例/正式度是否偏离基线？偏离 warn。
- 行动是否符合防御（攻击型在羞耻时先推开）？不符合且无前文依据 → fail。

### 7.3 编辑审查第 9 维（追加到现有 rubric）

```text
9. 情感回路：
   - 本场是否有清晰的「事件→评估→情绪变化→行动/台词→代价」？缺环即问题。
   - 情感是否由事件引起，而不是旁白宣布（「她很痛苦」）？
   - 角色反应是否符合其伤口/需求/防御，而不是作者需要？
   - 是否克制：没有替读者哭、没有解释主题、没有每段抒情？
   - 是否过载：连续具身化、每个情绪都写成身体反应、所有人轮流表态？
   - 是否廉价：强行误会、天降救兵、无代价的和解、突然变坏？
   评分 1-5：5=情感因果完整且克制；3=有情绪但因果不足；1=情绪空转/过载。
```

### 7.4 章纲情感目标（追加到 `scene_outline.md`）

```text
6. **情感转折**：
   - 开始：谁带着什么情绪/强度/未说出口的话进入本场？
   - 触发：什么事件触碰了谁的伤口/需求？
   - 结束：谁的情绪/关系/目标发生了什么不可逆变化？
7. **读者目标**：读者应产生什么感受？为什么？叙述必须克制在哪里？
8. **情感债务**：本场兑现哪笔旧债、新开哪笔债？
```

无情感契约的旧故事，这三块不出现，提示词逐字不变（用 mustache 条件块，与现有 `volume` 块同模式）。

---

## 8. 分期实施

| 阶段 | 建议版本 | 交付 | 不做 | 验收 |
|---|---|---|---|---|
| **P0 地基** | v0.75.0 | `emotion_circuit` 领域模块；12 维向量；OCC 评估；V150 迁移；`SceneBeatCard` 扩展；`【本拍情感回路】` 块；确定性探针（advisory）；单测/契约测试/Golden fixtures | 不改 Gate 阻断；不加读者面板；不做压力场选拍；不改章纲 | 现有 1890/646/45 全绿；新增 Rust 测试 ≥30；真机一章能看到块且不崩；探针在机器腔样本命中 ≥3、人类样本误报 ≤1 |
| **P1 闭环** | v0.76.0 | 情感压力场选下一拍；张力账本持久化与逾期；章纲情感目标；ingest `emotion_delta` 回流；L1 角色卡补最小情感；编辑第 9 维（advisory） | 不阻断；不做读者面板；不做载体反事实 | 情感因果率 ≥80%；张力按时兑现 ≥70%；旧故事提示词逐字不变；无新增热路径 LLM 调用 |
| **P2 调制** | v0.77.0 | 稀疏载体分配；读者面板四人格；Gate 规则分情感权重 0.3；A/B harness；前端情感页签/曲线/债务 | 不自动改稿；不替代作者；不做白盒激活 | 目标情感达成 ≥80%；读者 `moved` 中位数 ≥0.65；Gate 能拦下「情感空转」样本且不误杀人类样本；真机复跑 |
| **P3 自适应** | v0.78+ | 作者偏好校准；题材情感契约；跨语言；可选本地 HF 模型白盒适配（情绪方向/局部组件/电路调制，直接复用论文方法） | 不对远程 API 做白盒；不强制本地模型 | 人类 A/B ≥60% 偏好；作者 Tab 接受率提升；白盒适配仅本地模型可选 |

**每阶段的停止条件：**

- 任一阶段若探针误报率 >20%，停止进入下一阶段，先校准。
- 任一阶段若现有测试回退，停止，先修回归。
- 任一阶段若真机续写耗时增加 >15%，回退该阶段的非必要 LLM 调用。

---

## 9. 验收与度量

### 9.1 回归基线

以 v0.74.3 为基线：

- `cargo test --lib`：1890 passed / 10 ignored。
- `npx vitest run`：646 passed / 3 skipped。
- `npx playwright test`：45 passed / 5 skipped。
- `npx tsc --noEmit`、`cargo clippy`、`cargo +nightly-2026-10-05 fmt`、`python3 scripts/architecture_guard.py`、`verify-ipc-manifest.py` 全绿。

**硬性要求：** 任何阶段都不得以「新增测试数」掩盖旧测试回退；旧测试失败必须先修。

### 9.2 新增指标

| 指标 | 定义 | P1 目标 | P2 目标 | 备注 |
|---|---|---|---|---|
| 情感因果率 | 四要素（事件/评估/行动/代价）齐全的拍 / 总拍 | ≥80% | ≥90% | 确定性探针 |
| 目标情感达成 | 正文实际情绪向量与 `to` 的 cosine ≥0.5 的拍占比 | ≥70% | ≥80% | 词典 + 判官 |
| 情感变化率 | `distance(from, actual) ≥0.1` 的拍占比 | ≥85% | ≥90% | 防止「情感原地」 |
| 张力按时兑现 | due_chapter 前兑现/升级的张力 / 到期张力 | ≥70% | ≥80% | 账本 |
| 声音一致率 | 台词通过 voice probe 的角色拍 / 总角色拍 | ≥85% | ≥90% | 特征 + 判官 |
| 读者 `moved` | 四人格中位数 | 观测 | ≥0.65 | 章节级 |
| 过载率 | melodrama fail 的拍 / 总拍 | ≤10% | ≤5% | 反向指标 |
| 作者接受率 | 作者保留的情感探针建议 / 总建议 | 观测 | ≥50% | 弱标签 |

### 9.3 Golden fixtures

新增 `tests/fixtures/emotion/`：

| fixture | 用途 | 期望 |
|---|---|---|
| `flat_ai.txt` | 情绪标签堆砌、无事件/行动 | 因果率 fail、melodrama fail |
| `human_restrained.txt` | 人类公版小说节选，情感克制 | 因果率 pass、误报 ≤1 |
| `melodrama.txt` | 每段抒情、连续具身化、旁白解释 | melodrama fail、restraint fail |
| `subtle_turn.txt` | 情感转向但不明说 | target 达标、因果率 pass |
| `voice_break.txt` | 角色说了「绝不说」的话 | voice fail |
| `tension_stall.txt` | 同一对峙换说法复述 | tension fail |
| `earned_payoff.txt` | 有代价、有前文依据、克制 | `moved` ≥0.7（判官） |

**门禁：** fixture 测试进 `cargo test --lib`，不依赖网络；LLM 判官 fixture 用 mock，真机判官只在手动探针跑。

### 9.4 人类 A/B 协议

「感动人类读者」不能只靠代理指标。P2 起建立小规模人类评审：

- 样本：同故事、同大纲、同一拍的两个版本（有情感回路 vs 无）。
- 评审：≥20 名目标读者，盲评。
- 问题：
  1. 你更想继续读哪一版？（偏好）
  2. 哪一版让你更在乎角色？（投入）
  3. 哪一版有让你停下来回味的地方？（共鸣）
  4. 哪一版更假/更煽情？（反向）
- 成功标准：偏好 ≥60%，投入 ≥60%，共鸣 ≥40%，更假 ≤20%。
- 记录：`docs/audits/YYYY-MM-DD-emotion-ab-report.md`；未达标不宣称「已能感动读者」。

### 9.5 不可宣称的内容

在人类 A/B 达标前，文档、README、官网、提交信息**不得**写：

- 「已实现与人类读者的情感共鸣」
- 「AI 写的小说能感动人」
- 「情感回路达到论文级 99.65% 控制」
- 「已解决 AI 小说最大的缺点」

只能说：

- 「引入了情感因果链与稀疏载体调制，代理指标 X/Y 改善」
- 「论文方法在叙事层的可观测映射，尚未完成人类读者验证」

---

## 10. 风险、降级与开放问题

### 10.1 风险

| 风险 | 表现 | 缓解 |
|---|---|---|
| 情感机械化 | 每拍都按模板填「事件→评估→情绪→代价」，读起来像表格 | 载体稀疏、留白、峰值预算、读者 panel 反「模板感」、作者可关闭回路块 |
| 过度煽情 | 每章都在哭，读者疲劳 | scale 三档、每章峰值 ≤2、峰值后必须余波、melodrama 探针 |
| 判官偏好显性情绪 | LLM 判官给「哭得很惨」高分 | 判官 rubric 奖励克制与因果，不奖励强度；人类 A/B 校准 |
| 提示词膨胀/延迟 | 上下文变长、首字节变慢 | 块 ≤600 字；稳定区前缀缓存；P0 零新增 LLM；P2 读者 panel 后台化 |
| 黑盒模型不遵从 | 远程模型忽略情感块 | 探针检测；定向重写；降级为「载体指令」；不阻断落稿 |
| 数据模型膨胀 | 每拍一行，长篇几万行 | 按章聚合 + 最近 N 章明细 + 旧章压缩；索引 story_id/chapter |
| 作者被剥夺控制 | AI 自动改情感状态 | 作者锁定优先；所有状态可编辑；自动状态标 `source`；一键回滚 |
| 文化偏差 | 中文的羞耻/面子/关系逻辑与英文六情绪不同 | 12 维 + 中文语料校准；题材契约；不直接套 Ekman |
| GPL 污染 | 复制论文代码导致许可证冲突 | 不复制；独立实现；文档与代码注释注明「方法借鉴，非代码移植」 |
| 指标欺骗 | 探针通过但读者无感 | 代理指标只做筛选；人类 A/B 是最终门；不宣称未验证结论 |

### 10.2 降级策略

- `emotion_circuit` 数据缺失 → 回退现有静态情感行，逐字不变。
- 情感块超预算 → 先删「读者目标」和「禁忌」，保留因果链与载体；仍超则整块降级为一行「本拍情感：from→to」。
- 探针失败 → 只记录质量债，不阻塞（P0/P1）；P2 仅在三重条件同时成立时阻断。
- 读者 panel 失败/超时 → 静默跳过，不影响正文。
- ingest 失败 → 不写动态状态，不影响正文落库（沿用现有原则）。

### 10.3 开放问题（评审需拍板）

1. **Gate 何时阻断？** 建议 P2 校准后，仅「因果 fail + 张力 fail + 编辑 ≤2」三重条件阻断；是否更激进？
2. **12 维是否够？** 是否加入「嫉妒/孤独/敬畏」等；建议先用 12 维，JSON 可扩展。
3. **峰值预算谁定？** 默认每章 1–2 次；是否按题材（言情可 2，悬疑可 1）？
4. **情感契约谁写？** 创世 AI 建议 + 作者确认；是否允许作者完全手写、AI 不参与？
5. **读者 panel 成本？** 每章 4 次短调用，是否可接受；是否改成每 3 章一次？
6. **白盒适配优先级？** P3 是否值得为本地 Llama/Qwen 用户做激活级情感调制？还是永远只做叙事层？
7. **主 system 是否迁移 registry？** `CONTINUE_BEAT_SYSTEM` 目前不可覆盖；是否在 P2 迁移，让高级用户可调？
8. **旧故事兼容？** 无情感契约的旧故事是否自动生成契约，还是保持逐字不变直到作者启用？

---

## 11. 附录

### A. 论文机制 → 叙事映射（速查）

| 论文 | 叙事 |
|---|---|
| 六情绪 SEV | 12 维中文情绪空间 |
| 组内减均值 | 事件事实与情感评估分离 |
| 单位方向 | `EmotionVector` 归一化 + 独立强度 |
| top-k 神经元 | 2–3 个表达载体 |
| 消融/增强 | 反事实草稿 / 探针 |
| 子层重要性 | 故事位置预算 |
| 560 稀疏电路 | 每拍稀疏载体 + 一条主张力 |
| 差分向量 + scale | `from→to` + 0.4/0.8/1.2 |
| 99.65% | 目标情感达成率 + 读者 panel + 人类 A/B |

### B. 12 维情绪与中文词表示例

| 维度 | 词表示例 | 高唤醒 | 正/负 |
|---|---|---|---|
| joy | 高兴、欢喜、雀跃、欣慰 | 中 | 正 |
| sadness | 难过、失落、悲恸、怅然 | 低 | 负 |
| fear | 害怕、恐惧、不安、发冷 | 高 | 负 |
| anger | 愤怒、恼火、恨、暴怒 | 高 | 负 |
| disgust | 厌恶、恶心、鄙夷、嫌弃 | 中 | 负 |
| surprise | 惊讶、错愕、震惊、意外 | 高 | 中性 |
| love | 爱、心疼、眷恋、舍不得 | 中 | 正 |
| trust | 信任、依赖、安心、托付 | 低 | 正 |
| guilt | 愧疚、内疚、亏欠、悔 | 中 | 负 |
| shame | 羞耻、难堪、丢脸、无地自容 | 高 | 负 |
| pride | 骄傲、自尊、体面、扬眉 | 中 | 正 |
| hope | 希望、盼、期待、侥幸 | 中 | 正 |

### C. OCC 评估规则表（节选）

| 事件 | relevance | congruence | agency | certainty | control | norm | attachment | 情绪 |
|---|---|---|---|---|---|---|---|---|
| 被当众否认 | 高 | 负 | 他人 | 高 | 低 | 高 | 高 | shame + anger |
| 被重要的人背叛 | 高 | 负 | 他人 | 高 | 低 | 高 | 高 | anger + disgust + shame |
| 失去亲人 | 高 | 负 | 环境 | 高 | 低 | — | 高 | sadness + love |
| 被无条件接纳 | 高 | 正 | 他人 | 高 | — | — | 高 | joy + love + trust |
| 自己造成伤害 | 高 | 负 | 自己 | 高 | 高 | 高 | 高 | guilt |
| 目标受威胁但可努力 | 高 | 负 | 环境 | 低 | 中 | — | — | fear + hope |
| 达成被认可的目标 | 高 | 正 | 自己 | 高 | 高 | — | — | pride + joy |
| 未来仍有可能 | 高 | 正 | 混合 | 低 | 中 | — | — | hope |

### D. 载体表（情绪 → 首选载体）

| 情绪 | 首选 | 次选 | 避免 |
|---|---|---|---|
| anger | action / dialogue_subtext | choice / consequence | interiority（除非压抑） |
| fear | sensory / action | silence / choice | 长篇解释 |
| sadness | silence / consequence | sensory / action | 连续具身化 |
| shame | conceal（dialogue_subtext / silence） | interiority（限量） | 当众独白 |
| love | action（照顾）/ dialogue_subtext | choice / sensory | 直接告白（除非峰值） |
| guilt | consequence / action（补偿） | dialogue_subtext / silence | 自我辩解 |
| hope | choice / action | dialogue_subtext / sensory | 空喊口号 |
| pride | action / dialogue_subtext | choice | 旁白夸赞 |
| disgust | dialogue_subtext / action（推开） | sensory | 长篇生理描写 |
| surprise | rhythm / silence | action / dialogue_subtext | 解释「她很惊讶」 |

### E. 文件清单（计划新增/修改）

**新增：**

- `src-tauri/src/emotion_circuit/{mod,vector,appraisal,state,circuit,carriers,modulation,probe,reader,repo,tests}.rs`
- `src-tauri/src/db/migrations/V150__emotion_circuit.sql`
- `src-tauri/src/db/migrations/V150__emotion_circuit_backfill.rs`（如需回填；否则只用 SQL）
- `resources/prompts/agency/agency_emotion_circuit_system.md`
- `resources/prompts/agency/agency_emotion_judge.md`
- `resources/prompts/agency/agency_emotion_probe_fix.md`
- `tests/fixtures/emotion/*.txt`
- `docs/audits/2026-11-XX-emotion-probe-calibration.md`（实施后）
- `docs/audits/2026-11-XX-emotion-ab-report.md`（P2 后）

**修改：**

- `src-tauri/src/agency/beat_card.rs`（`EmotionBeat` 扩展、`compile_emotion` → `compile_emotion_beat`、冲突由压力场选）
- `src-tauri/src/agency/beat_state.rs`（状态网增加情绪/张力快照、探针接线）
- `src-tauri/src/agency/continue_assets.rs`（动态区情感块、L1 卡补最小情感、预算保护）
- `src-tauri/src/agency/continue_director.rs`（人物锁增加情绪/防御/台词禁忌摘要）
- `src-tauri/src/agency/coordinator.rs`（`write_beat_once` 接线、章纲注入、Gate 第 9 维、commit 写回）
- `src-tauri/src/agency/gate.rs` / `graders.rs`（规则分内部权重）
- `src-tauri/src/memory/ingest.rs` / `asset_bridge.rs`（`emotion_delta` 回流）
- `resources/prompts/planner/scene_outline.md`
- `resources/prompts/agency/agency_editor_auditor_system.md`
- `resources/prompts/memory/memory_content_analysis.md`
- `src-tauri/src/prompts/assembly.rs`（`CONTINUE_BEAT_SYSTEM` 追加一条情感因果 clause，不改变行数契约或同步更新契约）
- `src-tauri/src/handlers.rs`（IPC）
- `src-frontend/src/services/api/agency.ts`（IPC 包装）
- `src-frontend/src/pages/AgencyStudio.tsx`（情感页签）
- `src-frontend/src/pages/AgencyEval.tsx`（指标）
- `src-frontend/src/components/CharacterEditModal.tsx`（四元组编辑）
- `e2e/mock-tauri.ts`（新命令 mock）
- `AGENTS.md` / `CHANGELOG.md` / `README.md` / `PROJECT_STATUS.md` / `ROADMAP.md` / `ARCHITECTURE.md` / `TESTING.md` / `docs/USER_GUIDE.md`（按发版规则同步）

### F. V150 SQL 草案

```sql
-- 故事情感契约
CREATE TABLE IF NOT EXISTS story_emotion_contracts (
  story_id TEXT PRIMARY KEY,
  core_question TEXT,
  reader_promise TEXT,
  palette_json TEXT NOT NULL DEFAULT '{}',
  ending_residue TEXT,
  forbidden_json TEXT NOT NULL DEFAULT '[]',
  peak_budget INTEGER NOT NULL DEFAULT 1,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- 角色动态情绪状态（append-only）
CREATE TABLE IF NOT EXISTS character_emotion_states (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  character_id TEXT NOT NULL,
  scene_id TEXT,
  chapter_number INTEGER,
  beat_index INTEGER,
  vector_json TEXT NOT NULL,
  intensity REAL NOT NULL DEFAULT 0.5,
  active_goal TEXT,
  defense TEXT,
  cause_event TEXT,
  source TEXT NOT NULL DEFAULT 'ingest', -- ingest | commit | author
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_character_emotion_states_story
  ON character_emotion_states(story_id, chapter_number);

-- 每拍情感因果链
CREATE TABLE IF NOT EXISTS scene_emotion_beats (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT NOT NULL,
  beat_index INTEGER NOT NULL,
  from_json TEXT NOT NULL,
  to_json TEXT NOT NULL,
  trigger_event TEXT,
  appraisal_json TEXT,
  action_tendency TEXT,
  dialogue_policy_json TEXT,
  cost TEXT,
  carrier_weights_json TEXT NOT NULL,
  reader_target_json TEXT,
  scale REAL NOT NULL DEFAULT 0.8,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_emotion_beats_scene
  ON scene_emotion_beats(scene_id, beat_index);

-- 关系张力账本
CREATE TABLE IF NOT EXISTS emotion_tensions (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  source_character_id TEXT NOT NULL,
  target_character_id TEXT,
  tension_type TEXT NOT NULL,
  bond TEXT,
  reverse_bond TEXT,
  pressure REAL NOT NULL DEFAULT 0.5,
  accumulated_beats INTEGER NOT NULL DEFAULT 0,
  due_chapter INTEGER,
  status TEXT NOT NULL DEFAULT 'open', -- open | escalating | paid | dormant
  last_escalated_chapter INTEGER,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_emotion_tensions_story
  ON emotion_tensions(story_id, status, due_chapter);

-- 情感债务
CREATE TABLE IF NOT EXISTS emotion_debts (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  kind TEXT NOT NULL, -- promise | rift | sacrifice | secret | unpaid_kindness
  source_ref TEXT,
  description TEXT,
  pressure REAL NOT NULL,
  due_chapter INTEGER,
  status TEXT NOT NULL DEFAULT 'open', -- open | paid | deferred | ignored
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_emotion_debts_story
  ON emotion_debts(story_id, status, due_chapter);

-- 读者面板
CREATE TABLE IF NOT EXISTS reader_resonance_reviews (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT,
  chapter_number INTEGER,
  persona TEXT NOT NULL,
  scores_json TEXT NOT NULL,
  verdict TEXT NOT NULL,
  evidence_json TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reader_resonance_story
  ON reader_resonance_reviews(story_id, chapter_number);

-- 探针日志
CREATE TABLE IF NOT EXISTS emotion_probe_log (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT,
  beat_index INTEGER,
  probe TEXT NOT NULL,
  severity TEXT NOT NULL, -- low | medium | high
  passed INTEGER NOT NULL,
  detail_json TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_emotion_probe_story
  ON emotion_probe_log(story_id, scene_id, probe);
```

### G. 参考文献

1. Wang, C., Zhang, Y., Yu, R., et al. *Do LLMs “Feel”? Emotion Circuits Discovery and Control.* arXiv:2510.11328, 2025. https://arxiv.org/abs/2510.11328
2. EmotionCircuits-LLM 代码与预提取电路：https://github.com/Aurora-cx/EmotionCircuits-LLM（GPL-3.0）
3. Ekman, P. *An Argument for Basic Emotions.* Cognition and Emotion, 1992.
4. Ortony, A., Clore, G. L., & Collins, A. *The Cognitive Structure of Emotions.* Cambridge University Press, 1988.（OCC 评估模型）
5. McKee, R. *Story: Substance, Structure, Style, and the Principles of Screenwriting.* 1997.（压力下的选择、场景价值转折）
6. 项目内文档：`docs/superpowers/plans/2026-08-09-emotional-attrs-relationships.md`、`docs/plans/2026-08-13-agency-only-continuation-design.md`、`docs/plans/2026-08-15-continue-quality-closure-design.md`、`docs/audits/2026-10-06-project-review-v0.58.0.md`、`docs/audits/2026-10-14-human-voice-corpus-calibration.md`。

---

## 12. 评审检查清单

评审这份设计时，只需回答：

- [ ] 是否同意「情感必须上游于情节」这一裁定？如果不同意，本设计的大部分结构不成立。
- [ ] 是否同意 P0 不引入额外热路径 LLM 调用、不阻断落稿？
- [ ] 是否同意 Gate 顶层权重保持 0.2/0.3/0.5，只在内部增加情感维度？
- [ ] 是否同意 12 维情绪空间（六基本 + 六社会）？
- [ ] 是否同意每拍 2–3 个载体、每章 1–2 次峰值的稀疏原则？
- [ ] 是否同意 V150 的 7 张新表与 append-only 策略？
- [ ] 是否同意 P2 才做读者面板与阻断，P3 才考虑白盒适配？
- [ ] 是否接受「人类 A/B 达标前不宣称能感动读者」的验收纪律？
