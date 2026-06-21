# Story System × Genesis 智能创作流程贯通设计

> **版本**: v0.22.4 后续
> **日期**: 2026-06-21
> **状态**: 设计稿待实现
> **目标**: 把 Story System（合同驱动体系、追读力、提交链、投影）从"后台孤岛"逐步接入 Genesis 及后续智能创作主路径

---

## 1. 背景与问题

当前 Genesis 智能创作流程产出大量故事级资产（世界观、角色、大纲、场景、伏笔、知识图谱、第一章正文），但这些产出：

1. **没有写入 Story System 的合同真源**（`story_contracts`）。
2. **没有生成运行时合同约束**，Writer / Inspector / Review 无法加载。
3. **追读力（`chapter_reading_power` / `chase_debt`）与提交链（`scene_commits`）只在后台分析/任务系统中使用**，未在"生成前→生成→提交"主路径中形成闭环。
4. **体裁画像、Style DNA、方法论等创作资产**虽已注入 Writer prompt，但与 Story System 的合同/审计维度是平行宇宙，缺少统一叙事。

结果是：创世成功后，后台有一堆互不相干的子系统，用户感受不到"合同约束驱动创作"。

---

## 2. 设计原则

1. **不推倒重来**：在现有 `GenesisPipeline`、`AgentOrchestrator`、`StorySystemEngine` 上缝合，不重构核心架构。
2. **写后真源优先**：先把 Genesis 产出写成合同，再让后续环节消费；避免 Genesis 内部循环依赖。
3. **失败不阻塞创作**：合同播种、追读力计算失败只记 warning，不影响用户主流程。
4. **逐步放量**：Phase A 只播种 → Phase B 加载约束 → Phase C 闭环反馈。
5. **用户可覆盖**：所有新增 prompt 注入段都走 `PromptRegistry`，不新增硬编码提示词。

---

## 3. 总体路线图

| 阶段 | 目标 | 主要文件 | 用户可感知变化 |
|---|---|---|---|
| **A. 合同真源建立** | Genesis 完成后自动写入 `MASTER_SETTING` + `CHAPTER_1` 合同 | `narrative/genesis.rs`, `story_system/contract_builder.rs` | 无直接感知，但后续 Writer 开始受约束 |
| **B. 合同约束回流** | Writer / Inspector / Review / Refine 加载运行时合同并注入 prompt | `agents/service.rs`, `pipeline/review.rs`, `pipeline/refine.rs`, `creative_engine/write_time_bundle.rs` | 续写更贴合大纲；审稿能指出"违背合同" |
| **C. 追读力与提交链闭环** | 章节生成时考虑追读力债务，提交后更新投影/追读力/债务 | `story_system/chapter_service.rs`, `reading_power/`, `agents/orchestrator.rs` | 生成内容自动埋钩、控制节奏；后台产生追读力报告 |

---

## 4. Phase A：合同真源建立

### 4.1 范围

只在 `GenesisPipeline` 末尾增加一个收尾步骤 `ContractSeedingStep`，在**所有后台生成步骤完成后**统一创建初始合同。不修改现有并行生成逻辑。

### 4.2 设计

#### 新增步骤

```rust
// narrative/genesis.rs
struct ContractSeedingStep;
```

加入 `GenesisPipeline::first_chapter_and_background_steps()` 的末尾：

```rust
pub fn first_chapter_and_background_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
    vec![
        Box::new(FirstChapterGenerationStep),
        Box::new(ParallelWorldOutlineCharacterStep),
        Box::new(SceneGenerationStep),
        Box::new(ForeshadowingGenerationStep),
        Box::new(KnowledgeGraphGenerationStep),
        Box::new(ContractSeedingStep), // 新增
    ]
}
```

#### 数据映射

| Story System 合同字段 | Genesis 数据源 | 说明 |
|---|---|---|
| `MASTER_SETTING.genre` | `StoryMeta.genre` | 故事题材 |
| `MASTER_SETTING.core_tone` | `GenreProfile.core_tone` → fallback `StoryMeta.tone` | 核心基调 |
| `MASTER_SETTING.pacing_strategy` | `GenreProfile.pacing_strategy` → fallback `StoryMeta.pacing` | 节奏策略 |
| `MASTER_SETTING.anti_patterns` | `GenreProfile.anti_patterns_json` | 应避免的反套路 |
| `MASTER_SETTING.world_rules` | `WorldBuildingElement.rules[].name + description` | 世界规则 |
| `CHAPTER_1.goal` | 第一章 outline 或 FirstChapterGenerationStep 的生成目标 | 本章戏剧目标 |
| `CHAPTER_1.must_cover_nodes` | 首个 scene 标题 + 首个伏笔 content + 第一章必须出现的角色名 | 必须覆盖的情节点 |
| `CHAPTER_1.forbidden_zones` | `GenreProfile.anti_patterns` + 世界观中标注的"不可打破规则" | 禁止触碰的设定 |
| `CHAPTER_1.time_anchor` | 首个 scene 的 `setting_time` | 时间锚点 |
| `CHAPTER_1.chapter_span` | 首个 scene 的 `setting_location` | 空间跨度 |

#### 实现细节

1. `ContractSeedingStep` 从 `ctx.bundle` 读取最终状态（已写入 DB 的也可从 DB 重读）。
2. 调用 `StorySystemEngine::create_master_setting` 和 `create_chapter_contract`。
3. 若 `selected_strategy.genre_profile_id` 为空，则尝试用 `StoryMeta.genre` 按名称查 `genre_profiles` 表补全。
4. 所有字段优先用 `GenreProfile` 数据，因为那是体裁画像的权威来源。

#### 错误处理

```rust
// 伪代码
if let Err(e) = seed_contracts(ctx).await {
    log::warn!("[GenesisPipeline] Contract seeding failed (non-blocking): {}", e);
}
Ok(())
```

**合同播种失败不中断 Genesis 流程**。

#### 测试

- 新增 `contract_seeding_tests` 模块：
  - 验证 `MASTER_SETTING` 写入 `story_contracts` 且 JSON 可反序列化。
  - 验证 `CHAPTER_1` 写入且 `chapter_number == 1`。
  - 验证 GenreProfile 为空时回退到 StoryMeta 字段。
- 使用内存 SQLite + 完整迁移（待 V092 测试基线修复后回归）。

---

## 5. Phase B：合同约束回流

### 5.1 范围

让 Writer、Inspector、Review、Refine 在运行时能加载 `RuntimeContract`，并把合同约束作为 prompt 上下文注入。

### 5.2 设计

#### 5.2.1 运行时合同加载

在 `AgentContext` / `WriteTimeBundle` 中增加：

```rust
pub struct AgentContext {
    // ... 现有字段
    pub runtime_contract: Option<RuntimeContract>,
}

pub struct WriteTimeBundle {
    // ... 现有字段
    pub contract_constraints: Option<String>,
}
```

`StoryContextBuilder::build` 和 `WriteTimeBundle::load` 中调用：

```rust
let engine = StorySystemEngine::new(pool.clone());
let runtime_contract = engine.get_runtime_contract(story_id, chapter_number).ok();
```

#### 5.2.2 Prompt 注入点

**Writer (`agents/service.rs::build_writer_prompt`)**

在 genre_profile / methodology 之后追加：

```
【故事合同约束】
- 核心基调：{core_tone}
- 节奏策略：{pacing_strategy}
- 不可违反的世界规则：
  - {rule1}
  - {rule2}
- 本章目标：{chapter_goal}
- 本章必须覆盖：{must_cover_nodes}
- 本章禁止区域：{forbidden_zones}

重要：续写内容必须遵守上述合同。如需打破规则，必须先在剧情中给出足够铺垫，并在【违背合同说明】中解释。
```

**Inspector (`agents/service.rs::build_inspector_prompt`)**

新增审计维度：

```
【合同合规检查】
请检查待检查内容是否违反以下合同：
1. 是否违背核心基调？
2. 是否违背节奏策略？
3. 是否违反世界规则？
4. 是否遗漏本章必须覆盖的情节点？
5. 是否进入禁止区域？
```

**TimeSliced Writer (`creative_engine/write_time_bundle.rs`)**

把合同约束渲染为 `contract_constraints` section，放在 `genre_profile_strategy` 之后注入。

**Review / Refine (`pipeline/review.rs`, `pipeline/refine.rs`)**

在审稿/修稿 prompt 中追加：

```
【审稿合同标准】
以故事合同（MASTER_SETTING + CHAPTER_N）为基准，判断稿件是否存在：
- 设定冲突
- 节奏偏离
- 情节点遗漏
- 反套路触碰
```

#### 5.2.3 与创作资产的统一

体裁画像、方法论、Style DNA、叙事四元组 already 注入 Writer。合同约束是对这些资产的"执行层封装"：

- `MASTER_SETTING` 把 genre_profile / world_rules 固化成不可违反的约束。
- `CHAPTER_N` 把 outline / scene / foreshadowing 转化为本章必须履行的义务。

#### 5.2.4 测试

- 单元测试：验证 `build_writer_prompt` 输出包含 "故事合同约束" 关键字。
- 单元测试：验证 `build_inspector_prompt` 输出包含 "合同合规检查"。
- 集成测试：构造一份明显违反世界规则的稿件，验证 Inspector 能指出违规。

---

## 6. Phase C：追读力与提交链闭环

### 6.1 范围

在章节生成前计算当前追读力债务并注入生成目标；在章节提交（commit）后更新追读力、追读力债务、投影状态。

### 6.2 设计

#### 6.2.1 生成前：追读力债务注入

在 `AgentOrchestrator` / `PlanExecutor::execute_writer` 中，加载当前故事的 active `chase_debt`：

```rust
let debt_repo = ChaseDebtRepository::new(pool.clone());
let overdue_debts = debt_repo.get_overdue_by_story(story_id, current_chapter)?;
let active_debts = debt_repo.get_active_by_story(story_id)?;
```

把债务信息渲染为 prompt 段落：

```
【追读力债务】
当前有 {N} 条待偿还的追读力债务，需在后续章节中兑现：
- 债务类型：{type}，到期章节：{due_chapter}，当前金额：{amount}，来源：{source_chapter}

请在续写中优先安排以下元素以偿还债务：
{payback_plan}
```

如果债务金额高 / 已逾期，可以提升本章的"钩子强度"要求。

#### 6.2.2 生成时：埋钩与节奏控制

在 Writer prompt 中增加可选的"追读力目标"：

```
【本章追读力目标】
- 结尾需留下的悬念类型：{hook_type}
- 钩子强度：{hook_strength}
- 需埋设/回收的伏笔：{foreshadowing_list}
- 爽点/情绪微 payoff 数量：{micropayoff_count}
```

这些数据可以从：
- `chapter_reading_power` 的历史记录
- `chase_debt` 的偿还计划
- `foreshadowing_tracker` 的待回收伏笔

综合计算得出。

#### 6.2.3 提交后：追读力评估与投影更新

复用 `SceneCommitService.apply_commit` 的已有基础设施：

1. 在 `apply_commit` 中，若 `chapter_content` 存在，调用 `ReadingPowerEvaluator::evaluate`。
2. 把结果写入 `chapter_reading_power` 表。
3. 根据评估结果生成/更新 `chase_debt`：
   - 若本章产生新的未兑现承诺 → 新增债务。
   - 若本章偿还了既有债务 → 标记为 `paid`。
4. 更新 `scene_commits.projection_status_json`。

#### 6.2.4 自动化触发

当前 `update_chapter` 有 30s debounce 后触发 `auto_commit`。Phase C 保持这个触发点，但在 `auto_commit` 之前插入：

```rust
// 在 update_chapter 的 debounce 回调中
let evaluator = ReadingPowerEvaluator::new(pool.clone());
let evaluation = evaluator.evaluate(story_id, chapter_number, &content).await?;
let rp_repo = ChapterReadingPowerRepository::new(pool.clone());
rp_repo.create_or_update(...).await?;

let debt_service = ChaseDebtService::new(pool.clone());
debt_service.reconcile(story_id, chapter_number, &evaluation).await?;

scene_commit_service.auto_commit(...).await?;
```

#### 6.2.5 测试

- 单元测试：验证 `ReadingPowerEvaluator` 对含明显 cliffhanger 的文本给出高 hook_strength。
- 单元测试：验证偿还债务后 `chase_debt.status == 'paid'`。
- 集成测试：完整走一章生成→更新→提交，验证 `chapter_reading_power` 和 `chase_debt` 有记录。

---

## 7. 跨阶段事项

### 7.1 数据库迁移

Phase A/B/C 均复用已有表：
- `story_contracts`（Migration 47）
- `chapter_reading_power`（Migration 50）
- `chase_debt`（Migration 51）
- `override_contracts`（Migration 52）
- `scene_commits`（已存在）

**不新增表**。但 Phase C 可能需要在 `chase_debt` 中补充 `source_scene_id` 字段，若需要追踪到具体场景，可在后续单独评估。

### 7.2 向后兼容

- 老故事没有合同：Writer/Inspector 遇到 `RuntimeContract` 缺失时静默跳过，不报错。
- 老故事没有追读力记录：债务计算为空，不强制注入追读力目标。
- 所有新增 prompt 段都通过 `PromptRegistry` 渲染，用户可关闭或覆盖。

### 7.3 性能

- `StorySystemEngine::get_runtime_contract` 是同步 DB 查询，放在 `StoryContextBuilder` 中注意不能阻塞异步 runtime；建议用 `tokio::task::spawn_blocking` 包裹。
- `ReadingPowerEvaluator` 若调用 LLM，需放在后台任务中，不影响保存响应速度。

### 7.4 PromptRegistry 条目

新增/复用以下 prompt key（全部用户可覆盖）：

| Prompt ID | 用途 | 注入位置 |
|---|---|---|
| `writer_contract_constraints` | Writer 合同约束段 | `agents/service.rs` |
| `inspector_contract_compliance` | Inspector 合同合规检查 | `agents/service.rs` |
| `write_time_bundle_contract` | TimeSliced 合同约束 | `write_time_bundle.rs` |
| `review_contract_criteria` | 审稿合同标准 | `pipeline/review.rs` |
| `refine_contract_criteria` | 修稿合同标准 | `pipeline/refine.rs` |
| `writer_chase_debt` | 追读力债务段 | `agents/service.rs` |
| `writer_reading_power_goal` | 本章追读力目标 | `agents/service.rs` |

> **注意**：这些 prompt 只注册默认内容，不硬编码在 Rust 代码中。

---

## 8. 成功指标

| 指标 | 验证方式 |
|---|---|
| Genesis 完成后 `story_contracts` 有 `MASTER_SETTING` 和 `CHAPTER_1` | DB 查询 / 单元测试 |
| Writer prompt 包含合同约束段 | 字符串断言 |
| Inspector 能指出明显违背世界规则的内容 | 集成测试 |
| 章节提交后 `chapter_reading_power` 有记录 | DB 查询 / 单元测试 |
| 追读力债务随章节更新被创建/偿还 | 单元测试 |
| `cargo check` 零错误 | CI |
| 新增/修改的测试通过 | CI |

---

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 合同约束过强导致生成内容僵化 | 中 | Prompt 中强调"合同是参考，可违背但需说明"；保留 override_contracts 机制 |
| Phase A 播种失败导致后续阶段无合同可用 | 低 | 失败不阻塞 Genesis；后续阶段对缺失合同静默跳过 |
| 追读力计算调用 LLM 增加延迟 | 中 | 放在后台 debounce 任务中；提供同步启发式 fallback |
| Prompt 过长 | 中 | 合同约束只取核心字段；追读力债务只取 top 3 |
| 测试基线（V092）schema 问题阻塞回归 | 高 | 先在当前基线跑 targeted tests；全量回归待 schema 修复后补 |

---

## 10. 实现顺序建议

1. **Phase A**：`ContractSeedingStep` + 数据映射 + 单元测试。
2. **Phase B 先行**：`Writer` 加载 `RuntimeContract` 并注入 prompt（收益最大）。
3. **Phase B 后行**：`Inspector` / `Review` / `Refine` 合同约束。
4. **Phase C**：`ReadingPowerEvaluator` 接入 `update_chapter` debounce + 债务偿还逻辑。

每个阶段结束后都做一次 `cargo check` + targeted tests + 人工 prompt 抽查。

---

*设计完成。下一步：通过 `writing-plans` skill 生成 Phase A 的详细实现计划。*
