# 创世方法论全面应用 — 设计决策

> **日期：** 2026-07-09  
> **依据审计：** `docs/audits/2026-07-09-methodology-in-genesis-audit.md`  
> **实施计划：** `docs/superpowers/plans/2026-07-09-methodology-in-genesis-remediation.md`  
> **目标：** 使已选创作方法论在创世 quick + background 全链路可观测、可验证地生效，并打通步进到续写。

---

## 1. 问题陈述

当前（inspected）：

- **首章路径已注入**方法论全文（`build_strategy_notes` → `first_scene` → TriShot + WriteTimeBundle）。
- **Background 五步代码传 notes，外部化模板无占位符 → 静默丢弃**（P0）。
- **`methodology_step` 创世恒 None** → 雪花/HDWB 永远 step1/seed（P1/P2）。
- **ID 分裂** `world_building` vs `high_density_world_building`（P1）。
- **OpeningSkeleton 800 字截断**常挤掉方法论（P1）。
- 文档仍写「策略在后台」——已过时。

「全面应用」≠ 把 10 步雪花全文塞进每一步；而是：

1. **每一条会调 LLM 的创世步骤都能看到与本步相关的方法论约束**；
2. **选择 → 落库 → 注入 → 可观测**闭环无断链；
3. **步进在创世结束时落到合理起点**，续写可继续推进。

---

## 2. 成功标准（可证伪）

| ID | 标准 | 验证方式 |
|----|------|----------|
| S1 | Background 任一步（world/outline/character/scene/foreshadow）渲染 prompt 含「创作策略参考」且含已选方法论正文关键词 | 契约测试 + 手工创世日志 |
| S2 | `resolve_methodology_prompt("world_building")` 与 `"high_density_world_building"` 均返回 HDWB 正文 | 单测 |
| S3 | OpeningSkeleton 在画像很长时，prompt 仍含「应遵循的方法论」或方法论短摘要 ≥120 字 | 单测 |
| S4 | StrategySelection 后 `stories.methodology_id` 非空且 `methodology_step = 1` | 单测 / DB |
| S5 | 题材命中有 `recommended_methodology_id` 时，selector 预填该 id（LLM 可覆盖） | 单测 |
| S6 | 雪花创世结束后 `methodology_step` 反映「已完成到哪一步」映射表（见 §4），续写读到同值 | 单测 |
| S7 | 文档不再声称「策略在 background / 首章无方法论」 | 文档 diff |

**非目标（本方案不做）：**

- 不把 StrategySelection 再挪回 background。
- 不在 quick phase 为方法论再加一轮独立 LLM（延迟预算）。
- 不强制用户只能用某一种方法论。
- 不重写 MethodologyEngine trait 体系。

---

## 3. 架构原则

### 3.1 单一展开入口

继续以 `build_strategy_notes` + `resolve_methodology_prompt` 为 Genesis 展开入口；禁止在各 Step 内各自硬编码方法论正文。

增强为：

```text
build_strategy_notes(ctx, genre)           // 完整 notes（首章 / background）
build_methodology_brief(ctx)               // 方法论短摘要（骨架优先保留）
build_strategy_notes_for_step(ctx, step)   // Phase C：按步注入对应 phase/step prompt
```

### 3.2 模板变量契约

凡 `*_prompt(..., strategy_context, narrative_quartet, ...)` 传入的键，外部化 `.md` **必须**声明并使用同名变量。用测试锁定，防止再次外部化回归。

### 3.3 ID 规范

- **Canonical：** `high_density_world_building`
- **Alias（读写兼容）：** `world_building`、`hdwb` → 全部 normalize 到 canonical 再 resolve / 落库
- 设置页展示仍可用短名，但写入 DB 用 canonical

### 3.4 注入剂量

| 步骤 | 剂量 | 理由 |
|------|------|------|
| OpeningSkeleton | 方法论 brief（≤400 字）+ 画像 brief（≤400 字） | 避免 800 截断误伤 |
| FirstChapter | 全文 notes + 四元组 | 正文质量主战场 |
| World | HDWB seed/expansion 或通用 notes | 世界构建最吃方法论 |
| Outline | 雪花 step2–4 或 hero_journey 结构段 + notes | 结构步 |
| Character | character_depth 优先叠加 + notes | 人物步 |
| Scene | scene_structure 优先叠加 + notes | 场景步 |
| Foreshadow | 通用 notes（可短） | 伏笔不需全文十步 |

Phase A 先统一「全文 notes 进 background」；Phase C 再按步替换为「步进专用 prompt」。

---

## 4. 分阶段步进映射（Phase C，需门控）

创世结束时写入 `methodology_step`，表示「创世已推进到的步骤」，供续写接着用。

### 4.1 雪花法（`snowflake`）

| 创世步骤 | 注入的 snowflake step | 结束后 step 落库 |
|----------|----------------------|------------------|
| StrategySelection | — | 1 |
| OpeningSkeleton / FirstChapter | step1（一句话）已由 concept 覆盖；首章用 step1+结构纪律 | 1 |
| Outline | step2（一段话）+ step4（结构）摘要 | 4 |
| Character | step3（角色） | 4（取 max） |
| Scene | step8（场景列表）摘要 | 8 |
| 创世全部完成后 | — | **max(已完成)=8**；续写从 9（或用户设置） |

简化落地（推荐 v1）：创世完成后统一写 `methodology_step = 8`（场景列表已做过），续写默认读 step9。若担心过激，可写 `4`（仅大纲级）。

**本方案 v1 决策：创世完成后 snowflake → `methodology_step = 4`（大纲级完成）；Scene 步注入 step8 文本但不把落库推到 8，避免续写跳步。** 可在 CHANGELOG 写明，后续用配置项升级。

### 4.2 HDWB（`high_density_world_building`）

| 创世步骤 | 注入 phase | 结束后 step |
|----------|------------|-------------|
| World | seed（1） | 1 |
| Outline / Character 并行后 | expansion（2）可追加到 outline/character notes | 2 |
| 创世完成 | — | **2** |

### 4.3 无步进的方法论

`hero_journey` / `scene_structure` / `character_depth`：全文注入相关步骤；`methodology_step` 保持 `1`。

---

## 5. 分阶段交付

| Phase | 内容 | 建议版本 | 依赖 |
|-------|------|----------|------|
| **A** | P0 模板断链 + 变量契约测试 + 可观测日志 | 下一 patch | 无 |
| **B** | P1 ID 统一、骨架截断、推荐预填、step=1 落库 | 同或下一 patch | A |
| **C** | P2 分步注入映射 + 创世结束 step 推进 | 独立小版本 | A+B + 产品确认 §4 |
| **D** | 文档/技能/死代码清理、TriShot 去重 | 随 A–C | — |

---

## 6. 风险与回滚

| 风险 | 缓解 |
|------|------|
| Background prompt 变长 → 慢/截断 | notes 对 foreshadow 可截断 1200 字；先 A 全文，有问题再截 |
| 步进映射过激导致续写错乱 | C 默认保守（snowflake→4，HDWB→2）；配置可关 |
| Alias 规范化改写用户旧 DB | 读时 normalize，写时写 canonical；旧 `world_building` 仍可读 |
| 模板改了用户有 override | 提示词面板「重置」；契约测内置模板 |

回滚：Phase A 仅改 5 个 md + 测试，可单 commit revert。

---

## 7. 明确不在本方案

- 题材画像 match-or-create（并行线，见未完成的 EnsureGenreProfile）。
- 拆书方法论注入。
- 把 52 个 StyleDNA 全文塞进 Genesis（仅 P2 可选 brief）。
