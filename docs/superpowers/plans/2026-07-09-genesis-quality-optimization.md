# Genesis 首章质量：完善与优化实施方案

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让「新写一部末世生存的长篇小说」类创世指令产出的第一章，在人物目标、冲突节拍、世界观锚点上显著可感知地优于现状，同时不破坏 quick phase 用户可感知延迟契约（目标 p95 ≤ 90s，理想 ≤ 60s）。

**Architecture:** 在现有 `概念 → 策略 → 开篇` quick_phase 与后台世界/大纲/角色/场景之间，插入**轻量「开篇骨架」同步步**（主角卡 + 场景戏剧卡 + 世界一句话规则），使 `narrative_first_scene_generate` 的戏剧槽位不再为空；并行加厚 concept / 中文化 strategy / 合并重复纪律 / 接通四元组。不把完整 BG 世界构建前移（延迟不可接受）。

**Tech Stack:** Rust GenesisPipeline (`narrative/genesis.rs`)、PromptRegistry (`resources/prompts/**`)、TriShot (`agents/orchestrator.rs`)、`infer_narrative_quartet`、vitest/cargo 契约测试、`creative_workflow.log` 对照。

**Audit basis:** [创世提示词审计画布](genesis-prompt-audit.canvas.tsx)（2026-07-09）；样本指令「新写一部末世生存的长篇小说」。

**Status note:** `sf-genesis-campaign` 中「策略仍在后台」已过时——v0.26.28 已将 `StrategySelectionStep` 移入 quick_phase。本方案**不再重复该战役**，主攻审计发现的时序倒置与提示词空心。

---

## 0. 问题陈述与成功标准

### 0.1 根因（已审计，非猜测）

| 优先级 | 根因 | 证据 |
|--------|------|------|
| P0 | **时序倒置**：开篇写在世界/角色/场景/合同之前 | BG 步骤在 `final_content` 返回后才跑；`dramatic_*` / `scene_outline` / 合同为空 |
| P0 | **概念提示过薄** | `narrative_story_concept_generate.md` 仅 7 字段，无冲突/主角/世界规则 |
| P1 | **四元组未接入创世** | Genesis 不调 `infer_narrative_quartet`；模板槽位常空 |
| P1 | **纪律段三重重复、创作指导少** | first_scene + writer_system + `NOVEL_OUTPUT_DISCIPLINE` |
| P1 | **占位角色硬编码** | QuickPreflight「主角 / 在异星末世中生存」 |
| P2 | strategy_selector 英文；quality_gate 永不热路径；.md/fallback 漂移 | 审计画布「质量诊断」 |

### 0.2 成功标准（可证伪）

- **S1（质量）**：同指令 A/B 盲测（现状 vs 本方案），人工或 LLM 评分器在「冲突清晰度 / 人物动机 / 末世锚点 / 可读性」四维平均提升 ≥15%，或盲测胜率 ≥70%（N≥5 题材样本，含末世）。
- **S2（延迟）**：quick_phase 墙钟 p95 ≤ **90s**（文档已写 30-90s）；若本地基线已常 >60s，**禁止**再叠加重 LLM 步而不加预算守卫。理想目标仍 ≤60s。
- **S3（不回归）**：`cargo test --lib narrative::genesis` 全绿；`genesis-duplicate` E2E 幽灵隐藏；无新 Genesis 重复/空交付。
- **S4（可观测）**：`creative_workflow.log` 记录骨架步耗时、槽位填充率（非空 dramatic_goal / characters_present 比例）。

### 0.3 非目标（本方案不做）

- 把完整 `ParallelWorldOutlineCharacterStep` 整段移入 quick（延迟爆炸）。
- 热路径同步跑 Inspector / Rewrite / quality_gate LLM（违反分时介入不变量）。
- 物理合并记忆表 / 大改 TriShot 拓扑。

### 0.4 用户契约门禁（R10）

若实施方案使 quick_phase **稳定**超过文档承诺的上限，必须先征得用户授权并更新 `USER_GUIDE` / `ROADMAP` 延迟文案。默认策略：**骨架步用最快模型 + 硬超时（建议 8–12s）+ 失败则空槽 fallback（不阻塞开篇）**。

---

## 1. 文件与职责地图

| 文件 | 职责 |
|------|------|
| `src-tauri/src/narrative/genesis.rs` | 新增 `OpeningSkeletonStep`；调整 `quick_phase_steps`；开篇步读取骨架 |
| `src-tauri/src/narrative/prompts.rs` | `opening_skeleton_prompt` / 加厚 concept 变量；四元组注入 helper |
| `resources/prompts/creation/narrative_story_concept_generate.md` | 加厚 JSON schema |
| `resources/prompts/creation/narrative_opening_skeleton.md` | **新建**骨架提示词 |
| `resources/prompts/creation/narrative_first_scene_generate.md` | 合并纪律；强调「槽位非空时必须落地」 |
| `resources/prompts/strategy/strategy_selector.md` | 中文化 + 可选 quartet 字段 |
| `resources/prompts/writer/writer_system.md` | 去掉与 Call3 尾部重复的纪律（或改为引用单源） |
| `src-tauri/src/agents/orchestrator.rs` | 单源 `NOVEL_OUTPUT_DISCIPLINE`；创世 preflight 用骨架角色替代硬编码 |
| `src-tauri/src/strategy/quartet_inference.rs` | Genesis 路径调用 |
| `src-tauri/src/narrative/genesis.rs` 测试 + 可选 golden | 契约：步骤顺序、骨架 JSON 解析、槽位填充 |
| `ROADMAP.md` / docs of record | 登记战役与延迟承诺 |

---

## 2. 分阶段实施

```mermaid
flowchart LR
  P0[Phase 0 基线量化] --> P1[Phase 1 提示词加厚]
  P1 --> P2[Phase 2 开篇骨架步]
  P2 --> P3[Phase 3 四元组+预检]
  P3 --> P4[Phase 4 纪律合并+观测]
  P4 --> P5[Phase 5 A/B 与发布]
```

---

### Phase 0 — 基线量化（gate：数据，不改行为）

**目的：** 知道当前 quick_phase 真实耗时与首章空槽率，避免在已超时基线上再叠 LLM。

- [ ] **Step 0.1** 从 `creative_workflow.log` 抽取近 N 次创世：`smart_execute.bootstrap.enter` → `genesis.final_content` 耗时；记录 Call3 `content_len`。
- [ ] **Step 0.2** 对同一次运行，检查开篇前 `first_scene` 渲染日志或临时诊断：`dramatic_goal` / `scene_outline` / `characters_present` / `narrative_quartet` 是否为空（可加一次性 `log::info!` 诊断，不改提示词）。
- [ ] **Step 0.3** 写一页基线笔记到 `docs/plans/2026-07-09-genesis-quality-baseline.md`：p50/p95 延迟、空槽率、样本文本摘录。

**Gate 0：**
- 若 p95 已 ≥ 90s → **禁止** Phase 2 同步加 LLM；改走「骨架与概念合并为单次 LLM」或「骨架纯规则/模板无 LLM」。
- 若空槽率 < 30%（意外已填充）→ 重新审计，勿盲目加骨架步。

---

### Phase 1 — 提示词加厚（低延迟、高杠杆）

**目的：** 不增加 LLM 次数，提高概念与策略信息密度。

#### Task 1.1 — 加厚 `narrative_story_concept_generate`

**Files:**
- Modify: `resources/prompts/creation/narrative_story_concept_generate.md`
- Modify: `src-tauri/src/narrative/prompts.rs`（fallback 对齐）
- Modify: `ConceptGenerationStep` 解析字段（向后兼容：缺字段不失败）

- [ ] **Step 1** 扩展 JSON schema（建议字段，均可选除 title/genre 外）：
  - `protagonist_name`, `protagonist_desire`, `protagonist_wound`
  - `core_conflict`, `world_one_liner`, `survival_stakes`（末世等）
  - `genre_profile_ids: string[]`
  - 保留原有 title/description/genre/tone/pacing/themes/target_length
- [ ] **Step 2** 在模板中声明 `{{genre_profiles}}`（与代码已传变量对齐），要求模型从列表选 id。
- [ ] **Step 3** 解析写入 `GenesisContext` / concept 结构（新增字段存 JSON 扩展或 struct 可选字段）。
- [ ] **Step 4** 单测：旧 JSON 仍可解析；新 JSON 字段可读出。
- [ ] **Step 5** Commit: `feat: thicken genesis concept prompt schema`

#### Task 1.2 — 中文化 `strategy_selector` + 输出约束

**Files:**
- Modify: `resources/prompts/strategy/strategy_selector.md`

- [ ] **Step 1** 将角色说明与 Rules 改为中文；保留 JSON key 英文（解析稳定）。
- [ ] **Step 2** 明确：必须选与 `genre` / 用户输入匹配的 `genre_profile_id`；末世类优先 `apocalyptic`。
- [ ] **Step 3** 契约测试：fixture 含「末世生存」时解析结果含 apocalyptic（已有测试可扩展）。
- [ ] **Step 4** Commit: `docs/feat: localize strategy_selector prompt`

**Gate 1：** 本地跑 1 次创世，概念 JSON 含冲突/主角字段；策略仍返回合法 JSON；quick 延迟变化 ≤ +5s（无新 LLM）。

---

### Phase 2 — 开篇骨架步（核心结构修复）

**目的：** 在策略之后、开篇之前，用**一次**最快模型调用产出可填槽的骨架；失败则降级为空（开篇仍可跑）。

#### Task 2.1 — 提示词与纯函数

**Files:**
- Create: `resources/prompts/creation/narrative_opening_skeleton.md`
- Create/Modify: `prompts.rs` → `opening_skeleton_prompt(...)`
- Test: `genesis.rs` `#[cfg(test)]` 解析纯函数

骨架 JSON 建议最小集：

```json
{
  "protagonist": { "name": "", "goal": "", "obstacle": "" },
  "scene": {
    "dramatic_goal": "",
    "conflict_type": "",
    "external_pressure": "",
    "setting_location": "",
    "setting_time": "",
    "setting_atmosphere": "",
    "characters_present": [],
    "scene_outline": ""
  },
  "world_rules_one_liner": ""
}
```

输入：`user_premise` + concept 摘要 + `strategy_notes`（截断）+ 可选 genre tips。

- [ ] **Step 1** 写失败测试：`parse_opening_skeleton` 合法 JSON → Ok；残缺 → Err/默认。
- [ ] **Step 2** 实现解析纯函数 + `.md` 模板。
- [ ] **Step 3** Commit: `feat: opening skeleton prompt + parser`

#### Task 2.2 — Pipeline 步骤 + 预算守卫

**Files:**
- Modify: `genesis.rs` — `OpeningSkeletonStep`；`quick_phase_steps` 变为 4 步
- Modify: 进度百分比 / UI 步骤名（「铺设开篇骨架」）
- Modify: `FirstChapterGenerationStep` 优先用 `ctx.opening_skeleton` 填 `first_scene_prompt` 变量

- [ ] **Step 1** 契约测试：锁定 `quick_phase_steps` 顺序为 Concept → Strategy → **Skeleton** → FirstChapter；`background_steps` 不变。
- [ ] **Step 2** 实现 `OpeningSkeletonStep`：
  - `TaskType::Analysis` 或最快路由；`timeout` 硬上限 **10s**（可配置）
  - 失败 / 超时：`ctx.opening_skeleton = None`，记 `GenesisContext.errors`，**不 fail pipeline**
  - 成功：写入 ctx；可选轻量 upsert 占位 Character（替换「主角」硬编码）
- [ ] **Step 3** `FirstChapterGenerationStep`：若 skeleton 存在，覆盖 dramatic_* / outline / characters_present / world one-liner 进 strategy_notes 或专用段。
- [ ] **Step 4** 日志：`genesis.opening_skeleton.done`（duration_ms, filled_slots）。
- [ ] **Step 5** 本地创世 3 次，确认槽位非空且 p95 仍 ≤ Gate 0 基线 + 15s。
- [ ] **Step 6** Commit: `feat: genesis OpeningSkeletonStep before first chapter`

**Gate 2：**
- 槽位填充率（dramatic_goal 非空）≥ 80%（成功路径）。
- 骨架失败时开篇仍交付（降级路径单测）。
- S2：若超 90s → 砍骨架 LLM，改为**从加厚 concept 字段规则映射**填槽（零额外 LLM），并更新 ROADMAP。

---

### Phase 3 — 四元组 + Preflight 去硬编码

#### Task 3.1 — Genesis 接入 `infer_narrative_quartet`

**Files:**
- Modify: `StrategySelectionStep` 或 Skeleton 之后：用 genre_profile + concept 调 `infer_narrative_quartet`
- Modify: `first_scene_prompt` 的 `narrative_quartet` 渲染

- [ ] **Step 1** 单测：末世 genre → 非空 quartet 字符串。
- [ ] **Step 2** 接入 GenesisContext；开篇模板可见。
- [ ] **Step 3** Commit: `feat: inject narrative quartet into genesis first chapter`

#### Task 3.2 — 替换 TriShot 占位角色

**Files:**
- Modify: `agents/orchestrator.rs` QuickPreflight 占位逻辑

- [ ] **Step 1** 若 ctx/story 已有骨架主角名与目标，用其创建 placeholder，禁止写死「异星末世」。
- [ ] **Step 2** 单测或集成断言。
- [ ] **Step 3** Commit: `fix: genesis preflight uses skeleton protagonist`

**Gate 3：** 开篇 prompt 诊断日志中 `narrative_quartet` 非空；无「在异星末世中生存」硬编码（除非用户原文如此）。

---

### Phase 4 — 纪律合并与观测

#### Task 4.1 — 单源输出纪律

**Files:**
- Modify: `narrative_first_scene_generate.md` — 保留结构纪律，输出纪律改为短引用或删除重复
- Modify: `writer_system.md` — 删除与 `NOVEL_OUTPUT_DISCIPLINE` 重复的条目
- Keep: `orchestrator.rs` `NOVEL_OUTPUT_DISCIPLINE` 为 Call3 唯一强制尾注

- [ ] **Step 1** 改模板；确认 Call3 仍追加常量。
- [ ] **Step 2** Commit: `refactor: single-source novel output discipline`

#### Task 4.2 — 观测字段

- [ ] **Step 1** `genesis_runs.steps_json` 或 workflow log 增加 skeleton 耗时与 filled_slots。
- [ ] **Step 2** Commit: `chore: genesis skeleton observability`

**Gate 4：** 提示词总字数（开篇 instruction）不因重复纪律膨胀；日志可查填充率。

---

### Phase 5 — A/B 验证与发布

- [ ] **Step 1** 固定 5 条指令（含末世、古言、科幻、悬疑、复合「异星球末世」），A=发布前 tag，B=本方案分支。
- [ ] **Step 2** 盲测打分表写入 `docs/plans/2026-07-09-genesis-quality-ab.md`。
- [ ] **Step 3** 若 S1 未达标：迭代骨架提示词（非再加 LLM 步）；若 S2 破坏：启用零 LLM 映射降级为默认。
- [ ] **Step 4** 全量验证：`cargo test --lib`、`tsc`、`vitest`、`architecture_guard`、相关 E2E。
- [ ] **Step 5** 走 `sf-change-control`：bump 版本、docs of record、tag、推送、**监控 CI 至全绿**、本地 `cargo tauri build`。

---

## 3. 延迟预算（设计约束）

| 步骤 | 预算建议 | 超时行为 |
|------|----------|----------|
| Concept | 现有 | 现有重试 |
| Strategy | 现有（已在 quick） | 现有 |
| **Skeleton** | **≤10s** 最快模型 | 跳过，空槽 |
| FirstChapter TriShot | 现有（Call3≤120s genesis） | 现有 |
| **Quick 总计** | **p95 ≤90s** | 超则降级骨架为规则映射 |

并发注意：骨架与策略**不要**再 `join!` 抢 `BACKGROUND_LLM_SEMAPHORE` 同池过载；骨架用独立短超时即可。

---

## 4. 风险与围栏

| 风险 | 缓解 |
|------|------|
| 多一次 LLM 拖垮 60s 体感 | 硬超时 + 失败跳过；Gate 0 基线 |
| 骨架胡编与后 BG 世界矛盾 | 骨架极简；BG 仍可覆盖 DB；首章只要求「可写」 |
| 解析失败导致创世中断 | 骨架步永不 fail pipeline |
| 回潮 Genesis 重复 | 不改 delivered 状态机；不改 8% 闸门 |
| 文档承诺 30-60s vs 代码 30-90s | 发布时统一文案；超 90s 需 R10 |

---

## 5. 建议落地顺序（版本切片）

| 版本切片 | 内容 | 预估 |
|----------|------|------|
| **v0.26.44** | Phase 1（concept 加厚 + strategy 中文） | 0.5–1 天 |
| **v0.26.45** | Phase 2（OpeningSkeletonStep） | 1–2 天 |
| **v0.26.46** | Phase 3–4（四元组、preflight、纪律、观测） | 1 天 |
| **随后** | Phase 5 A/B；不达标则调 prompt 不叠步 | 0.5–1 天 |

---

## 6. 决策请求（实施前需用户确认）

1. **延迟：** 接受 quick_phase p95 目标 **90s**（与代码注释一致），还是必须死守 **60s**（则 Phase 2 默认走「零 LLM 从 concept 映射」）？
2. **骨架形态：** 独立 LLM 步（质量更好）vs 合并进 concept 单次调用（延迟更稳）？
3. **是否授权** 在 USER_GUIDE 中写明「创世先铺开篇骨架再写正文」？

确认后按 Phase 0 → 1 → 2 执行；未确认前不改 pipeline 顺序。
