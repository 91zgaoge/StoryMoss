# Genesis 智能创作流程审计与优化设计

> 状态：设计待审批  
> 版本：v0.26.19（计划）  
> 关联审计：PROJECT_STATUS.md、CHANGELOG.md、docs/archive/LESSONS_LEARNED.md

## 1. 背景与目标

### 1.1 背景

Genesis（智能创作流程-创世）是 StoryForge 的核心 onboarding 功能：用户在幕前输入一句话前提，系统在后台生成世界观、大纲、角色、场景、知识图谱、合同等叙事资产，并在 quick phase（30–60s）返回第一章正文。

v0.26.7 至 v0.26.18 期间，Genesis 第一章重复/空白问题经历了多轮补丁修复。本次审计在直接读码（非运行时复现）基础上，识别出 4 个 P0 契约违反/用户可感知缺陷、5 个 P1 架构债务/可观测性缺失、6 个 P2 技术债。

### 1.2 目标

- 修复全部 P0 缺陷，确保 Genesis 首章不空白、不重复、角色与世界观关联、仪表盘可展示运行记录。
- 在 P0 稳定前，不引入会改变用户可感知延迟的架构变更（策略选择不移入 quick phase）。
- 补齐关键测试覆盖，建立跨层共享 fixture。
- 更新文档与注释，消除 auto-accept/Tap 路径漂移。

## 2. 审计验证结论

| 编号 | 严重度 | 标题 | 验证结果 |
|------|--------|------|----------|
| P0-1 | P0 | `handleSmartGeneration` 空内容锁死状态机 | **已确认** |
| P0-2 | P0 | 角色生成缺失世界观上下文 | **已确认** |
| P0-3 | P0 | `ChapterSwitch` 在 `selectChapter` 完成前标记 delivered | **已确认** |
| P0-4 | P0 | `genesis_runs` 表完全未接入 | **已确认** |
| P1-1 | P1 | quick phase 顺序倒置（策略选择后置） | **已确认**，但暂缓 |
| P1-2 | P1 | 后台步骤错误静默吞掉 | **已确认** |
| P1-3 | P1 | mutex `.unwrap()` 中毒锁 panic 风险 | **已确认** |
| P1-4 | P1 | `ChapterSwitch` payload 与前端文档/注释漂移 | **已确认** |
| P1-5 | P1 | 测试覆盖严重不对称 | **已确认** |
| P2-x | P2 | 命名、重复加载、`appendAiContent`、双状态源等 | **已确认** |

## 3. 关键决策

### 决策 1：P0-2 采用顺序执行修复，而非 tokio channel

- `ParallelWorldOutlineCharacterStep` 自 v0.23.71 起因信号量约束与自死锁规避，内部已是串行 `.await`；「并行」仅为命名残留。
- `outline` 不依赖 `world`，但 `character_prompt` 需要 `world.concept`。
- 采用 `world → outline → character(world)` 全串行顺序，blast radius 最小，不改变 step 名称、进度事件与现有测试。

### 决策 2：P1-1 策略选择移入 quick phase 暂缓

- ROADMAP.md:171、CHANGELOG.md:894、AGENTS_HISTORY.md:153 多处承诺 quick phase 30–60s 返回首章。
- 移入策略选择（~10–20s LLM）会让延迟进入 60–90s 区间，跨越用户可感知阈值，违反既有文档契约。
- 首章通过 `build_strategy_notes` 的 genre-profile fallback 已获得题材级约束，质量退化是 bounded 的。
- 本次优化目标为「稳定性 > 质量」；待 P0 全部修复、`genesis_runs` 接入后，用真实运行数据量化首章质量退化，再决定是否迁移。

## 4. 分阶段执行计划

### Phase 1 — P0 关键正确性修复

| 任务 | 文件 | 验收标准 |
|------|------|----------|
| 修复 `handleSmartGeneration` Gap B | `src-frontend/src/frontstage/FrontstageApp.tsx:3608-3625` | 空 `finalContent` 时不标记 `delivered`，保持 `generating` 态；新增 vitest 覆盖 |
| 修复角色生成世界观上下文 | `src-tauri/src/narrative/genesis.rs:1500-1646` | `character_prompt` 拿到 `world.concept`；`background_steps_include_contract_seeding` 仍通过 |
| 修复 `ChapterSwitch` delivered 时序 | `src-frontend/src/frontstage/FrontstageApp.tsx:1555-1601` | `delivered` 仅在 `selectChapter` 成功且编辑器非空后标记；新增 vitest |
| 接入 `genesis_runs` 表 | `genesis.rs`、`commands/orchestrator.rs`、`db/repositories.rs` | quick/background 阶段写入状态、错误、资产数量；仪表盘展示运行记录；新增 Rust 测试 |

### Phase 2 — P1 架构对齐（不含策略迁移）

| 任务 | 文件 | 验收标准 |
|------|------|----------|
| 错误可观测性 | `genesis.rs`、`orchestrator.rs` | 所有 `let _ =` 失败收集到 `GenesisContext::errors`，写入 `genesis_runs.errors_json`；超过阈值 toast 提示 |
| mutex 中毒锁加固 | `narrative/pipeline.rs:27,40`、`model_gateway/executor.rs:65` | 改用 `unwrap_or_else(\|e\| e.into_inner())`；新增中毒恢复测试 |
| 文档/注释对齐 | `FrontstageEvent.ts`、`genesis.rs:1147-1188`、`USER_GUIDE.md` | 明确 Genesis 首章走 `smart_execute.final_content` auto-accept；`ChapterSwitch` 不携带正文 |
| 记录 P1-1 债务 | `ROADMAP.md`、`PROJECT_STATUS.md` | 明确策略迁移为「待量化后决策」债务 |

### Phase 3 — P1 测试加固

| 任务 | 文件 | 验收标准 |
|------|------|----------|
| Rust Genesis 测试 | `src-tauri/src/narrative/genesis.rs` | quick_phase_steps 顺序、8% 自重复重试闸门、`ChapterSwitch` payload 形状、JSON fallback |
| 前端测试 | `FrontstageApp.genesis-duplicate.test.tsx` | Gap B、Gap C 专用测试；`genesisDeliveryRef` 状态机断言 |
| 跨层共享 fixture | 新增 `tests/fixtures/trim_golden.json` | Rust `trim_self_repetition` 与 TS `trimSelfRepetition` 同输入同输出 |
| 降低测试 brittleness | vitest / playwright | `waitFor` 替代 `setTimeout`；`expect.poll` 替代 `waitForTimeout` |

### Phase 4 — P2 技术债清理

| 任务 | 文件 | 验收标准 |
|------|------|----------|
| 命名与注释 | `genesis.rs` | 消除 `*_future` 误导命名；更新 `ParallelWorldOutlineCharacterStep` 注释 |
| 去重 `AppConfig::load` | `genesis.rs:697,703` | 仅加载一次 |
| `appendAiContent` skip 路径 | `FrontstageApp.tsx:3115` | 仅成功 append 时调用 `markAccepted` |
| `selectChapter` Gap C | `FrontstageApp.tsx:2146-2172` | 重复时也跳过 `setContent` |
| 双状态源评估 | `FrontstageApp.tsx` | 明确 `isGenesisSettingUpRef` 与 `genesisDeliveryRef` 职责边界或合并 |

## 5. 验收标准（整体）

- `cargo check` 零错误
- `cargo test --lib` 全绿，且 Genesis 相关测试 ≥ 6 个
- `npx tsc --noEmit` 零错误
- `npx vitest run` 全绿，Gap B/C 各 ≥ 1 个测试
- `npx playwright test` 全绿（Genesis E2E）
- `cargo +nightly fmt -- --check` 通过
- `npm run format:check` 通过
- 手动 `cargo tauri dev` 验证：首章不空白、不重复、仪表盘有 Genesis 运行记录

## 6. 风险与回滚

| 风险 | 缓解措施 |
|------|----------|
| P0-2 顺序重构影响 narrative bundle | 保留旧路径函数，必要时可回滚；step 名称与进度事件不变 |
| `genesis_runs` 写入引入新失败路径 | 所有写入操作包裹错误处理，失败不影响主流程 |
| 测试 brittleness 改造引入新不稳定 | 逐步替换，先在前端 vitest 中验证 |

## 7. 分支与 PR 策略

- Phase 1 单独分支：`fix/genesis-p0-correctness`
- Phase 2 单独分支：`feat/genesis-p1-alignment`
- Phase 3 单独分支：`test/genesis-coverage`
- Phase 4 单独分支：`refactor/genesis-p2-cleanup`
- 每 Phase CI 全绿后合并，每合并一次更新版本号与相关文档（遵循 AGENTS.md 强制构建规则）。
