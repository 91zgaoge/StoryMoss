# 续写叙事质量 + 多作品切换 实施方案

- 日期：2026-11-02
- 状态：**P0 + M3/M5 增量已实施（v0.80.0），P1/P2 剩余项待继续**。
- 设计依据：[2026-11-02-narrative-contract-design.md](./2026-11-02-narrative-contract-design.md)

### 实施状态（2026-11-02）

| 里程碑 | 状态 | 已交付 | 未交付 |
|---|---|---|---|
| M0 基线与开关 | 🟡 部分 | prompt/契约快照、指标日志骨架、`STORYMOSS_NARRATIVE_CONTRACT` / `STORYMOSS_NAMING` 环境回滚开关 | 设置页灰度开关、golden harness 扩展、正式指标表 |
| M1 多作品切换 | ✅ P0 | active story + seq、StorySwitchController、生成隔离、跨故事懒加载修复、后端/前端测试、多故事切换 E2E | 后台/创世分离的完整 UI |
| M2 NCC 最小闭环 | ✅ P0 | SceneContract 编译/场简报/契约核对/gate 硬失败/定向修订/情感最小链/8 条 system | 读者面板联动 |
| M3 章契约与批量统一 | 🟡 P0+ | `ChapterContract`/确定性章契约/1–3 场投影/批量 writer 注入章契约+场简报/legacy methodology/quartet 接线/进度块改实际正文末段 | LLM 章契约生成与校验、`SceneOutcome` 结构化落库、legacy 追读力/参考场景全量映射 |
| M4 姓名编译器 | 🟡 P0 | `naming` 校验（重名/同用字/同姓/长度塌缩/陈词）、创世逐组校验、反模式提示词修正 | NameLedger/称呼图/DB 落库/候选池、旧故事反建账本 |
| M5 度量、灰度、发布 | 🟡 部分 | 版本/文档同步、全量测试门禁、多故事切换 E2E | 人类 A/B、设置页正式灰度开关、真机样章 |

- 目标版本：建议 M1 `v0.78.1`，M2 `v0.79.0`，M3 `v0.80.0`，M4 `v0.81.0`，M5 `v0.82.0`（以实际发布为准）
- 实施原则：**先修用户可见 bug，再上契约最小闭环，最后做命名与全量灰度。** 不一次性重写 writer 路径。

---

## 0. 里程碑总览

| 里程碑 | 目标 | 依赖 | 退出标准 | 建议版本 |
|---|---|---|---|---|
| M0 基线与开关 | 可度量、可回滚、可灰度 | 无 | 指标能落库；flags 能关；golden harness 能跑 | v0.78.x |
| M1 多作品切换 | 幕后选中即切换，生成中也不拒绝；不串稿、不丢稿 | M0 | §2.7 全部通过 | v0.78.1 |
| M2 NCC 最小闭环 | 每拍有场契约；情感最小链默认；门禁核契约 | M0 | §3.7 全部通过 | v0.79.0 |
| M3 章契约与批量统一 | 章契约接地、批量不再丢卡、资产映射 | M2 | §4.7 全部通过 | v0.80.0 |
| M4 姓名编译器 | 新故事强制命名账本；旧故事只读兼容 | M0 | §5.7 全部通过 | v0.81.0 |
| M5 度量、灰度、发布 | 真机样章、人类 A/B、文档与回滚 | M2/M3/M4 | §6.6 全部通过 | v0.82.0 |

**并行建议：** M1 与 M2 可并行（前端 vs 后端/提示词）；M4 依赖 M0，可与 M3 并行，但不要和 M2 的 prompt 大改同时合并，避免 golden 快照冲突。

---

## 1. M0：基线与开关

### 1.1 目标

在改行为前，先建立「能看见、能关掉、能回滚」的基础设施。没有这一步，后面无法判断质量是变好还是只是变了。

### 1.2 任务

| ID | 任务 | 文件/位置 | 验收 |
|---|---|---|---|
| M0-1 | 新增叙事质量配置开关 | `src-tauri/src/config/settings.rs` 或新 `agency/narrative_contract/config.rs` | `narrative_contract_enabled`、`contract_gate_hard_fail`、`naming_compiler_enabled`、`story_switch_v2` 可读；默认值明确 |
| M0-2 | 新增切换/契约指标埋点 | `src-tauri/src/agency/learning.rs`、`frontstageLogger` / `logToBackend` | 切换耗时、取消/分离、契约完整率、通用节点率、硬失败率可查询 |
| M0-3 | 扩展 golden harness | `src-tauri/src/agency/golden_harness.rs`、`src-tauri/src/agency/serial_harness.rs` | 至少 3 个题材 × 2 章的固定输入；可断言契约字段与 gate 结果 |
| M0-4 | 建立 prompt 快照测试 | `src-tauri/src/agency/tests.rs`、`src-tauri/src/prompts/assembly.rs` 测试 | 稳定区逐字节不变；场简报/情感链块有快照 |
| M0-5 | 文档与回滚骨架 | `docs/plans/`、`docs/archive/LESSONS_LEARNED.md` | 回滚步骤可执行；flags 说明清楚 |

### 1.3 开关策略

- `narrative_contract_enabled`：默认 **新故事 true / 旧故事 compat**；关闭时走现有路径。
- `contract_gate_hard_fail`：默认 true；灰度期可降为 advisory，但必须记录。
- `naming_compiler_enabled`：默认 **新故事 true / 旧故事 false**。
- `story_switch_v2`：默认 true；仅在极端回归时可临时回退旧行为。
- 所有 flag 必须能在设置/环境变量中读取，不允许硬编码分支散落。

---

## 2. M1：多作品切换（P0，先修用户可见 bug）

### 2.1 目标

用户在幕后点击任何故事（卡片、打开、侧栏），幕前都必须切换到该故事；生成中也不静默拒绝。切换前保存旧故事，切换后不串稿、不丢稿、可恢复。

### 2.2 任务 1.1：后端 active story 真源

**改动文件：**

- `src-tauri/src/commands/sync.rs`
- `src-tauri/src/handlers.rs`
- `src-tauri/src/state_sync/events.rs`
- `src-tauri/src/state_sync/service.rs`
- 新增/复用持久化：`src-tauri/src/db/repositories/`（app state / settings 均可）

**步骤：**

1. 新增 `set_active_story(story_id: String)`：
   - 校验故事存在；
   - 持久化 `active_story_id`；
   - 生成持久化单调 `seq`（跨重启不回退）；
   - 发出 `story-selected`，payload 增加 `seq`、`origin`。
2. 新增 `get_active_story()`：返回 `{ story_id, title, seq, updated_at }`；故事不存在时返回 null。
3. `open_story_in_frontstage` 与 `notify_frontstage_story_selected` 统一调用 `set_active_story`；保留旧命令名做兼容包装。
4. handler 注册新命令。

**测试：**

- Rust 单测：set/get 往返；seq 单调；故事删除后 get 返回 null；事件 payload 含 seq/origin。
- 旧命令仍可调用（兼容）。

**验收：**

- 后端是唯一真源；幕前挂载能读到上次选中故事。

---

### 2.3 任务 1.2：幕后入口统一

**改动文件：**

- `src-frontend/src/pages/Stories.tsx:469`
- `src-frontend/src/components/Sidebar.tsx:174`
- `src-frontend/src/components/FrontstageLauncher.tsx:42`

**步骤：**

1. 卡片点击、打开按钮、侧栏打开全部走同一个 `set_active_story` 命令。
2. 同一故事 300ms 内重复触发去重。
3. 幕后选中状态保持乐观更新；失败时回滚并 toast。
4. 不弹二次确认；切换后的「已停止/后台继续」由幕前 toast 说明。

**测试：**

- Vitest：点击卡片调用一次命令；重复点击去重；失败回滚。
- E2E：卡片点击和打开按钮行为一致。

**验收：**

- 所有入口最终都让幕前显示目标故事；不存在“只有打开按钮有效”的旁路。

---

### 2.4 任务 1.3：幕前 StorySwitchController

**新增文件：**

- `src-frontend/src/frontstage/storySwitch/types.ts`
- `src-frontend/src/frontstage/storySwitch/controller.ts`
- `src-frontend/src/frontstage/storySwitch/useStorySwitch.ts`

**改动文件：**

- `src-frontend/src/frontstage/FrontstageApp.tsx:633`（删除生成中直接 return 的守卫）
- `src-frontend/src/frontstage/FrontstageApp.tsx:3108`（`selectStory` 改为受控入口或薄封装）
- `src-frontend/src/hooks/useSyncStore.ts:176`（传 `seq`/`origin`）

**步骤：**

1. 实现状态机：`Idle → Flushing → Disposing → Loading → Applying → Active`。
2. `switchStory({storyId, seq, origin})`：
   - 同故事：只重新定位最新章节，不 flush、不清空；
   - 先 `await flushSceneSaveRef.current()`；失败中止；
   - 调 `disposeGenerationForSwitch`；
   - 加载 story bundle（stories + chapters + scenes + latest chapter）；
   - `seq` 过期丢弃；
   - 原子应用并清理旧状态。
3. `switchSeqRef` 保证最后写入胜出；旧 `selectStory` 的异步返回不应用。
4. 删除 `FrontstageApp.tsx:635-638` 的 toast/return；`isGenerating` 只参与 `disposeGenerationForSwitch`。

**测试：**

- Vitest：
  - 生成中调用 `onStorySelected` 触发 `switchStory`，不再 toast-return；
  - flush 失败时 `currentStory` 不变；
  - 快速 A→B→C 只应用 C；
  - 同故事点击不 flush。
- 原有 `selectStory` 测试保持通过。

**验收：**

- 生成中点击另一部小说，幕前切换到目标故事。
- 旧故事的异步加载结果不会覆盖新故事。

---

### 2.5 任务 1.4：在途生成隔离

**改动文件：**

- `src-frontend/src/frontstage/FrontstageApp.tsx`（生成入口、事件回调、`handleCancelGeneration`）
- `src-frontend/src/services/api/stream.ts`（如需 request_id）
- `src-tauri/src/agency/commands.rs:263`（`agency_cancel_run` 已有，按 run_id 定向取消）
- `src-tauri/src/llm/commands.rs:102`（`llm_cancel_generation` 已有）

**步骤：**

1. 增加 `generationEpochRef` 与 `activeRunRef`（story_id/run_id/request_id）。
2. 所有异步回调应用前检查：
   - `eventStoryId === currentStoryRef.current?.id`；
   - `generationEpoch` 未过期。
3. `disposeGenerationForSwitch`：
   - 前台单拍/流式：递增 epoch、abort 前端流、按 `run_id` 调 `agency_cancel_run` 或按 `request_id` 调 `llm_cancel_generation`；等待 ≤2s。
   - 后台批量/创世：标记 detached，不取消；幕前过滤其事件；toast「旧作品仍在后台写作」。
   - 目标故事正是当前创世目标：等待创世完成再切换。
4. **不无条件调用 `agent_cancel_all_tasks`**；只有拿不到 run_id/request_id 时才回退全局取消并提示。

**测试：**

- 单测：旧 epoch 的 `ContentUpdate`/`AppendContent` 被忽略。
- E2E：A 生成中切 B，A 的完成结果不进入 B；A 内容完整。
- 后台批量分离：切走后旧故事继续写入，新故事不受影响。

**验收：**

- 跨故事写入 = 0；后台任务状态可见。

---

### 2.6 任务 1.5：状态清理与恢复

**改动文件：**

- `FrontstageApp.tsx`
- `src-frontend/src/frontstage/store/frontstageStore.ts`
- `src-frontend/src/frontstage/hooks/useScenePersistence.ts`

**步骤：**

1. `applyStoryBundle` + `resetStoryScopedUiState` 统一清理设计 §6.4.5 清单：
   - `generatedText`、`hideGhostUntil`、`postAcceptLock`、`genesisDeliveryRef`、`isGenesisSettingUpRef`、`smartExecuteInFlightRef`、打字机、流式缓冲、`recentlyAppendedRef`、`inputHistory`、`aiHints`、`saveError`、字数基准、`lazyLoadingChapterIdsRef`、`chapterSwitchTimestampRef`。
2. `latestContentRef` 在加载新内容前置空，避免 `flushSceneSave` 写错场景。
3. 挂载/聚焦：
   - `get_active_story()` 有值 → 直接加载；
   - 无值 → `stories[0]`；
   - `frontstageFocused`/`visibilitychange` 时比对并恢复。
4. 目标故事无章节 → 空态；被删除 → 清选择 + 提示。

**测试：**

- 单测：清理函数覆盖清单逐项断言。
- E2E：隐藏时选择 B，聚焦后显示 B；无章节/被删除 story 的边界。

**验收：**

- 切换后旧状态残留 = 0；事件丢失恢复率 100%。

---

### 2.7 任务 1.6：测试与验收门

**测试文件：**

- `src-frontend/src/frontstage/__tests__/storySwitch.test.tsx`（新增）
- `src-frontend/src/frontstage/__tests__/FrontstageApp.test.tsx`
- `e2e/` 新增多故事切换用例
- Rust：`src-tauri/src/commands/sync.rs` 测试 / `state_sync` 测试

**必须通过：**

1. 生成中点击 B → 幕前 B 最新章。
2. A→B→C 快速点击 → 最终 C。
3. 幕前隐藏选 B → 聚焦恢复 B。
4. 目标无章节 → 空态，不残留 A。
5. 目标被删除 → 提示，不冒充。
6. autosave 防抖中切换 → A 最后输入已落库。
7. 旧生成结果不进入 B。
8. 同故事点击只定位最新章。

**验收指标：**

| 指标 | 目标 |
|---|---|
| 目标存在时切换成功率 | 100% |
| 跨故事写入 | 0 |
| 数据丢失 | 0 |
| 旧状态残留 | 0 |
| 壳反馈 P95 | ≤ 500ms |
| 正文可见 P95 | ≤ 1.5s |

---

## 3. M2：NCC 最小闭环（P0，叙事质量）

### 3.1 目标

不先做完整章契约生成，而是先把「每拍有场契约、情感有最小因果链、门禁核契约」跑通。这一阶段不新增每拍 LLM 调用，主要靠确定性编译。

### 3.2 任务 2.1：新增 narrative_contract 模块

**新增目录：**

```
src-tauri/src/agency/narrative_contract/
  mod.rs
  types.rs        // SceneContract / StateDelta / EmotionChain / ContractVerification
  compile.rs      // 从 SceneBeatCard + 状态 + 冲突 + 情感编译
  validate.rs     // 具体性/完整性校验
  render.rs       // 场简报渲染
  verify.rs       // 契约核对
```

**步骤：**

1. 定义 `SceneContract` 全部字段（设计 §4.3）。
2. `StateDelta` 五轴：信息/关系/目标/风险/情绪；`from != to`。
3. `EmotionChain` 复用 `emotion_circuit` 的 `EmotionVector`/`OCC`/`Carrier`/`DialoguePolicy`。
4. 所有类型 `Serialize/Deserialize`，便于落库与测试。

**测试：**

- 序列化往返；
- 校验器拒绝空目标/无具名阻力/无状态变化/情绪无代价。

---

### 3.3 任务 2.2：确定性场契约编译

**改动文件：**

- `src-tauri/src/agency/beat_card.rs`（保留 SceneBeatCard 作为素材，不直接删）
- `src-tauri/src/agency/narrative_contract/compile.rs`

**步骤：**

1. 输入：`SceneBeatCard`、当前章大纲、上一场 `SceneOutcome`、角色目标/关系张力/活跃冲突/伏笔。
2. 输出 `SceneContract`：
   - `goal`：从章纲/下一节点 + 角色目标合成具体目标；
   - `opposition`：优先敌对关系双方；否则从目标冲突/张力/秘密推导具名对手；实在没有则报资产不足，禁止泛化「阻力」；
   - `stakes`：从角色需求/关系/目标推导具体损失；
   - `turn`：从冲突阶梯 + 章节转折推导事件；
   - `outcome`：不可逆结果或新问题；
   - `state_delta`：冲突阶段决定轴，`from/to` 必须具体；
   - `emotion`：最小链（见 3.4）。
3. 去掉 `methodology_next_node` 通用句作为生产路径；通用句只允许作为「资产不足」提示，不进入契约。

**测试：**

- 给定三类故事 fixture（古装/现代/玄幻），断言契约字段非空、具体、无通用句；
- 无敌对关系时能从目标冲突生成具名对手；仍不能则返回 `contract_incomplete` 而非泛化句。

**验收：**

- 生产样本中通用 `next_node` 占比 = 0。

---

### 3.4 任务 2.3：场简报渲染与 writer prompt

**改动文件：**

- `src-tauri/src/agency/narrative_contract/render.rs`
- `src-tauri/src/agency/beat_card.rs:925`（`render_writer_user_prompt_with_emotion`）
- `src-tauri/src/prompts/assembly.rs:83`（`CONTINUE_BEAT_SYSTEM`）
- `src-tauri/src/agency/continue_assets.rs:819`（资产顺序/预算）

**步骤：**

1. 新增 `render_scene_brief(&SceneContract) -> String`，≤300 字，包含：
   - 场次/场前状态/目标/阻力/阻力目标/手段/转折/必须结果/情感链/禁止/钩子/字数。
2. writer prompt 顺序调整为：稳定资产 → 章契约（M3 前可为空）→ 角色状态 → 场简报 → 指令 → 手法 → 末句锚点。
3. `CONTINUE_BEAT_SYSTEM` 改为设计 §5.1 的 8 条；保留错误/正确示例，删除重复长规则。
4. 资产预算：场简报不受 `ASSET_CHAR_BUDGET` 压缩；资产块只保留本场相关事实。
5. 保留 `writer_system.md` / `agency_lead_writer_system.md` 给 legacy/批量路径，不强行合并。

**测试：**

- Prompt 快照：稳定区逐字节不变；场简报在末句锚点之前；
- 字数预算：6000 字资产 + 300 字简报 + 锚点仍可控；
- 无契约时不注入空块（旧故事逐字不变，flag off）。

**验收：**

- writer 最后看到的完整指令是场简报，不是四行节拍卡。

---

### 3.5 任务 2.4：情感最小链默认启用

**改动文件：**

- `src-tauri/src/agency/coordinator.rs:8012`（`load_emotion_plan`）
- `src-tauri/src/agency/coordinator.rs:8069`（`load_chapter_emotion_plan`）
- `src-tauri/src/agency/beat_card.rs:627`（`compile_emotion`）
- `src-tauri/src/emotion_circuit/modulation.rs:11`
- `src-tauri/src/agency/graders.rs:120`

**步骤：**

1. `load_emotion_plan`：无契约/张力/债务/状态时，不再直接 None；用角色 wound/need/core + 本场目标 + 关系推导最小 `EmotionBeat`。
2. `load_chapter_emotion_plan`：去掉“只有契约存在才 Some”的硬门槛；无完整契约时生成最小章情感弧（1–2 幕）。
3. `compile_emotion`：不再返回静态角色档案；改为最小因果链摘要；无法生成时返回空并由契约校验报 `emotion_incomplete`。
4. `render_emotion_block` 嵌入场简报，不再单独成块（避免重复）。
5. `run_rule_grader`：有最小链时按链检查；完全没有情感资产时才允许中性，并在契约标记。
6. 保留完整 OCC/载体/读者面板作为增强路径。

**测试：**

- 无契约故事：仍生成最小链；
- 有完整契约：行为不变；
- 情感探针失败进入硬失败（当 `contract_gate_hard_fail=true`）；
- 旧故事 flag off 时 prompt 逐字不变。

**验收：**

- 新故事情感因果链覆盖率 100%；旧故事兼容模式 ≥90%。

---

### 3.6 任务 2.5：契约核对与门禁

**改动文件：**

- `src-tauri/src/agency/narrative_contract/verify.rs`
- `src-tauri/src/agency/coordinator.rs:7329`（`evaluate_gate_impl`）
- `src-tauri/src/agency/graders.rs`
- `src-tauri/src/agency/gate.rs`

**步骤：**

1. `ContractVerifier` 逐字段返回 `Met/Partial/Miss + evidence`。
2. 硬失败：无目标/无具名阻力/无转折/无状态变化/章末无不可逆/情感无触发或代价/连续性矛盾。
3. 门禁：
   - 任一硬失败 → `RevisionRequired`；
   - `stall`/`emotion`/`contract`/`hook` ≤2.0 → 触发修订；
   - 其余软问题记录质量债。
4. 保留现有 code grader / rule grader / weighted 分数用于报告，不让总分掩盖硬失败。
5. 增加 `contract_misses` 落库，供下一场契约消费。

**测试：**

- 硬失败样例逐项触发修订；
- 软问题不阻断；
- dimension_scores 低分触发修订；
- 旧故事无契约时 gate 行为不变（flag off）。

**验收：**

- 结构硬失败检出率 100%；stall 维度不再“只落库”。

---

### 3.7 任务 2.6：定向修订

**改动文件：**

- `src-tauri/src/agency/coordinator.rs:6996`（`build_revision_task`）
- `src-tauri/src/agency/coordinator.rs:6040`（`handle_gate` 修订轮）

**步骤：**

1. 新增 `build_contract_revision_task(contract, failures, draft)`：
   - 只改失败场/段；
   - 每个失败字段给契约要求、原文证据、目标状态；
   - 保留已达标项；不得新增角色/改变已发生事实。
2. 修订最多 1 轮；第二轮仍硬失败 → 质量债 + 下一场 `must_not`，不静默放行。
3. 幕前 Append：确定性硬失败同步修订（短 prompt）；软问题后台。

**测试：**

- 修订任务包含字段/证据/目标；
- 修订后二次核对通过率；
- 第二轮失败落质量债。

---

### 3.8 任务 2.7：M2 验收门

| 指标 | 目标 |
|---|---|
| 场契约字段完整率 | ≥95% |
| 通用 next_node 占比 | 0 |
| 每场状态变化率 | 100% |
| 每章不可逆结果率 | 100% |
| 情感因果链覆盖率 | 100%（最小链） |
| 硬失败检出率 | 100% |
| 旧故事 prompt 回归 | flag off 时逐字不变 |
| 单章 token 成本 | ≤ +20% |

---

## 4. M3：章契约与批量统一

### 4.1 目标

把「拍级契约」升级为「章契约 → 场契约」；批量续写不再丢节拍卡；把 legacy 结构资产映射进契约层。

### 4.2 任务 3.1：章契约生成与校验

**新增/改动：**

- `narrative_contract/chapter.rs`
- `narrative_contract/validate.rs`
- `coordinator.rs:4728`（`generate_chapter_outline` 改造或并存）

**步骤：**

1. 每章 1 次 LLM 调用，输入：上一章末 3000 字、故事大纲、章纲、角色目标/伤口/需求、关系张力、活跃冲突、伏笔/债务、最近 3 场结果。
2. 输出严格 JSON：`dramatic_question/want/opposition/opposition_plan/stakes/turning_point/irreversible_outcome/emotional_arc/promise_ops/scenes`。
3. 确定性校验：具体性、具名反对者、赌注、转折是事件、不可逆、场景数 1–3、每场状态变化、情感链。
4. 失败 → 修复一次（只修字段）→ 仍失败 → 最小契约；最小契约仍空泛 → 报资产不足，不写通用句。
5. 章契约章内不变，放稳定前缀区；场契约每拍变化。

**测试：**

- 3 题材 fixture 的章契约 JSON 校验；
- 修复路径；
- 最小契约不出现通用句；
- prompt cache：章契约在章内逐字节不变。

---

### 4.3 任务 3.2：场契约投影

**新增/改动：**

- `narrative_contract/project.rs`

**步骤：**

1. 章契约 → 1–3 场契约，纯 Rust。
2. 第 N 场 `entry_state` = 第 N-1 场 `outcome`。
3. 最后一场 `outcome` = 章 `irreversible_outcome`。
4. 场字数按章目标均分 ±20%。

**测试：**

- 1/2/3 场投影；
- 因果链连续性；
- 最后一场结果一致。

---

### 4.4 任务 3.3：批量续写统一

**改动文件：**

- `src-tauri/src/agency/coordinator.rs:5803`（`write_chapter`）
- `src-tauri/src/agency/coordinator.rs:5919`（writer task）
- `src-tauri/src/agency/coordinator.rs:8388`（`inject_batch_emotion_block`）

**步骤：**

1. `write_chapter` 不再只给「章纲 + 资产 + 有起伏」；注入章契约 + 本场简报（按 1–3 场顺序写）。
2. 批量 writer 改为直接 `complete()` + 契约；tool_loop 仅在失败时兜底（D5）。
3. 保留 `board_write` 交付契约（如果直接 complete 不改黑板，需在装配层写 draft；二选一，优先直接 complete + 内部写 draft）。
4. 每章 gate 使用本章契约；修订带契约/证据。

**测试：**

- 批量 writer prompt 含章契约/场简报；
- 批量章节 gate 能核对契约；
- tool_loop 兜底不回归。

---

### 4.5 任务 3.4：legacy 资产映射

**改动文件：**

- `src-tauri/src/creative_engine/write_time_bundle.rs`
- `src-tauri/src/agency/continue_assets.rs`
- `src-tauri/src/agency/narrative_contract/render.rs`

**映射表：**

| 资产 | 去向 |
|---|---|
| `methodology_extension` | 章契约/场契约的 scene engine；不再只靠 `to_prompt()` |
| `writer_beat_plan` | 场契约 schema |
| `writer_reading_power_goal` / `writer_chase_debt` | 章契约 `promise_ops` + `hook_out` |
| `writer_contract_constraints` | 稳定前缀硬约束层 |
| `narrative_quartet` | 章契约节奏/幕结构（可选） |
| `style_dna_extension` / `human_voice` | 文风层（现状保留） |
| `reference_scene_fewshots` | 条件 few-shot（仅风格漂移） |
| `outline_replan` | 章契约修复入口 |
| `continuity_blocks` / 检索 | `entry_state` 事实层 |

**步骤：**

1. 逐项确认「加载了但没消费」的资产，映射到契约或明确标记 legacy。
2. 不删除用户可能覆盖的 prompt；保留兼容。
3. 加测试：每个资产在契约渲染中有对应片段，或明确标注不适用。

---

### 4.6 任务 3.5：回流与存储

**迁移：**

- 新增 `V152__narrative_contract_state.sql`：
  - `scenes.scene_contract_json TEXT`（本场契约快照）
  - `scenes.scene_outcome_json TEXT`（本场结果/状态变化）
  - `scenes.chapter_contract_json TEXT`（章契约快照，可空）
  - 或复用 `outline_content` 结构化块作为 P0，迁移在 P1 落库。**推荐新列，避免继续往 outline 文本里塞。**

**步骤：**

1. 落库当前契约与结果；失败 best-effort 不阻断。
2. `progress_lines_from_parts` 改为最近 3 场 `SceneOutcome`，不再读 `outline_content` 前 200 字。
3. `next_node` 改为下一场契约；章内场次用完 → 下一章契约。
4. 承诺账本记录埋设/推进/兑现。

**测试：**

- 迁移幂等；
- 旧数据 null 兼容；
- 进度块显示结果而非计划。

---

### 4.7 M3 验收门

| 指标 | 目标 |
|---|---|
| 章契约生成成功率 | ≥95%（可校验） |
| 章契约修复后合格率 | ≥80% |
| 批量 writer 含契约率 | 100% |
| 进度块为结果而非计划 | 100% |
| 旧故事兼容 | flag off 时无行为变化 |
| 迁移可回滚 | 新列可空，旧代码忽略 |

---

## 5. M4：姓名编译器

### 5.1 目标

新故事强制使用命名账本；主要角色姓名通过分布/音形/语义/陈词校验；旧故事只读兼容，不自动改名。

### 5.2 任务 4.1：命名模块与数据资产

**新增目录：**

```
src-tauri/src/naming/
  mod.rs
  world.rs       // NameWorld / Era / Region / SocialClass
  family.rs      // FamilyLineage / 字辈
  scorer.rs      // 音形/语义/分布/陈词评分
  ledger.rs      // NameLedger / CharacterName
  address.rs     // AddressRule / 称呼图
  cliche.rs      // 加权陈词表
```

**新增资源：**

```
resources/naming/surnames.csv
resources/naming/given_chars.csv
resources/naming/cliche_weights.csv
resources/naming/famous_names.txt
resources/naming/generation_poems.csv
resources/naming/address_forms.csv
```

**步骤：**

1. 数据表先做小规模种子（每个题材至少 100 姓、500 字、50 条陈词组合），后续可扩充。
2. `NameWorld`/`FamilyLineage`/`CharacterName`/`AddressRule` 类型化。
3. 评分器实现设计 §5.5.3 的权重与硬约束。
4. 陈词按组合/聚集扣分，不按单字封杀。

---

### 5.3 任务 4.2：创世角色生成接入

**改动文件：**

- `resources/prompts/creation/novel_creation_character_roster.md`
- `src-tauri/src/story_system/auto_contract.rs:868`
- `src-tauri/src/agents/novel_creation.rs:163`（解析与校验）

**步骤：**

1. 删除「禁止常见姓/禁止单字名」；改为 `NamingBrief` + 候选池。
2. LLM 只填性格/背景/情感/取名故事，从候选里选；不得自由新增规范名。
3. `parse_character_roster_response` 增加：
   - 重名/同音；
   - 主要角色共用字；
   - 长度配额；
   - 陈词组合；
   - 时代/阶层/性别 fit。
4. 校验失败 → 重新候选（一次）→ 仍失败报错，不落库。

**测试：**

- 3 题材生成 5 角色：无重复字、无陈词、长度分布落在配额带；
- 旧提示词覆盖兼容；
- 用户手写名登记不报错。

---

### 5.4 任务 4.3：姓名账本与称呼图

**迁移：**

- 新增 `V153__character_names.sql`：
  - `character_names`：character_id/story_id/formal/surname/generation/given/courtesy/art/childhood/nickname/aliases_json/titles_json/taboo_json/address_rules_json/name_story/domains_json/phonetic_json
  - 可选 `story_name_worlds`：story_id/world_json
  - 旧角色按需回填（lazy），不强制全量迁移。

**步骤：**

1. `CharacterRepository::create/update` 接入账本：新角色必须有账本条目或先走命名编译器。
2. `character_aliases` 存字/号/小名/绰号/化名；`NameLedger.canonical` 为规范名真源。
3. `render_one_card`/场简报增加「本场称呼」；`SceneContract` cast 增加 `address_in_scene`。
4. `ContractVerifier` 检查：不得新编姓名、不得用错称呼、不得让死人说话、不得同场混淆同音名。
5. `prose_ground` 使用账本别名替代纯启发式 `aliases_for`。

**测试：**

- 称呼规则覆盖率 100%（主要角色在场时）；
- 续写新编姓名 = 0；
- 旧故事反建账本后不改名、不破坏别名。

---

### 5.5 任务 4.4：旧故事兼容

**步骤：**

1. 旧故事从 `characters` + `character_aliases` 反向构建账本。
2. 不自动改名；陈词名只给建议。
3. 作者确认改名 → 走现有 `record_aliases`，旧名登记别名。
4. 新增角色走新编译器。

**测试：**

- 旧故事加载不报错；
- 改名后旧名可解析；
- 不破坏 v0.73.14 的改名去重。

---

### 5.6 任务 4.5：命名验收

| 指标 | 目标 |
|---|---|
| 主要角色给定名共用字 | 0 |
| 主要角色非家族同姓 | ≤2 人 |
| 主要角色陈词组合命中 | 0 |
| 长度分布 | 落在题材配额带 |
| 同声调轮廓占比 | ≤30% |
| 同部首占比 | ≤2 人 |
| 单一语义域占比 | ≤30% |
| 称呼规则覆盖率 | 100% |
| 续写新编姓名率 | 0 |
| 人类评价「像真人/像这个时代」 | ≥4/5 |

---

## 6. M5：度量、灰度、发布

### 6.1 指标落库

- 契约：完整率、硬失败率、修订成功率、通用节点率、状态变化率、情感链覆盖率。
- 切换：成功率、取消/分离次数、耗时、跨故事写入（应为 0）。
- 命名：共用字、陈词、长度分布、称呼覆盖率。
- 成本：单章 token、P95 延迟、重复率。

### 6.2 灰度策略

1. **开发/内测**：M1 + M2 默认开；M3 章契约只对新故事开；M4 命名只对新故事开。
2. **旧故事**：compat 模式；M2 最小链可用，M3/M4 默认关。
3. **回滚**：
   - `narrative_contract_enabled=false` → 旧 writer 路径；
   - `contract_gate_hard_fail=false` → 硬失败降 advisory；
   - `naming_compiler_enabled=false` → 旧取名提示词；
   - `story_switch_v2=false` → 旧切换行为（仅紧急）。
4. **数据回滚**：新列可空；旧代码忽略；不删旧 prompt；不自动改名。

### 6.3 人类 A/B

- 30 章 / 3 题材，盲评：
  - 「想继续读」
  - 「情节合理」
  - 「人物动人」
  - 「姓名像真人/这个时代」
- 与现状对比，目标相对提升 ≥20% / ≥20% / ≥15% / ≥4/5。
- 若未达标：先修契约/命名，不加更多提示词。

### 6.4 文档与发布

按 AGENTS.md 强制规则：

- `README.md`
- `CHANGELOG.md`
- `AGENTS.md`
- `PROJECT_STATUS.md`
- `ROADMAP.md`
- `ARCHITECTURE.md`
- `TESTING.md`
- `docs/USER_GUIDE.md`
- 版本号统一：`Cargo.toml` / `tauri.conf.json` / `package.json` / tag。
- 推送后由 GitHub Actions 全平台构建；本地不主动 `cargo tauri build`，除非用户明确要求。

---

## 7. 依赖图与实施顺序

```
M0 基线与开关
 ├─► M1 多作品切换（可独立先发）
 ├─► M2 NCC 最小闭环
 │     └─► M3 章契约与批量统一
 │           └─► M5 度量/灰度/发布
 └─► M4 姓名编译器（可与 M3 并行）
       └─► M5
```

**建议合并顺序：**

1. M1（用户可见 bug，风险低，先发）。
2. M2（叙事质量最小闭环，必须带 flag）。
3. M4（命名，独立资产，可与 M3 并行）。
4. M3（章契约/批量/资产映射，改动面最大，最后合）。
5. M5（灰度与 A/B，不阻塞功能合并，但阻塞“可上线”声明）。

---

## 8. 测试矩阵

| 层 | 范围 | 命令/文件 |
|---|---|---|
| Rust 单测 | 契约类型/校验/投影/情感链/命名评分/active story | `cargo test --lib` |
| Rust 集成 | `write_beat_once` prompt 快照、`write_chapter`、gate、迁移 | `src-tauri/src/agency/tests.rs`、golden harness |
| 前端单测 | StorySwitchController、FrontstageApp 清理、命名弹窗 | `npx vitest run` |
| E2E | 多故事切换、生成中切换、隐藏恢复、无章节/删除边界 | `npx playwright test` |
| 静态 | tsc / clippy / fmt / guard | `npx tsc --noEmit`、`cargo clippy`、`cargo +nightly-2026-10-05 fmt -- --check`、`npm run format:check`、`python3 scripts/architecture_guard.py` |
| 人工 | 30 章盲评、姓名盲评 | 评审表 |

**每个里程碑必须新增：**

- 至少 1 个失败路径测试；
- 至少 1 个旧故事兼容测试；
- 至少 1 个 prompt/契约快照测试。

---

## 9. 验收门（上线前必须全部满足）

1. M1 多作品切换：§2.7 全绿；E2E 生成中切换通过。
2. M2：契约完整率 ≥95%；通用节点 0；硬失败检出 100%；旧故事 flag off 逐字不变。
3. M3：章契约/批量/回流通过；迁移幂等；进度块为结果。
4. M4：命名指标达标；旧故事不改名；续写新编姓名 0。
5. M5：人类 A/B 达到目标；成本/延迟在预算内；文档/版本/CI 全绿。
6. 未跑通设计验收探针，不得宣称“已修复”。

---

## 10. 回滚方案

| 层 | 回滚 |
|---|---|
| 前端切换 | `story_switch_v2=false` 恢复旧行为（仅紧急）；代码保留新路径 |
| 契约渲染 | `narrative_contract_enabled=false` 走旧 writer prompt |
| 契约门禁 | `contract_gate_hard_fail=false` 降为 advisory，不阻断 |
| 章契约 | 关闭章契约生成，回退拍级契约/旧章纲 |
| 命名 | `naming_compiler_enabled=false` 恢复旧取名提示词；已生成账本保留只读 |
| 数据库 | 新列可空；不回滚迁移；旧代码忽略新列 |
| Prompt | 用户覆盖保留；新 prompt ID 不被覆盖；删除新 ID 不影响旧 ID |

---

## 11. 风险与应对

| 风险 | 应对 |
|---|---|
| M1 删除守卫后串稿 | `story_id + generationEpoch` 双过滤；E2E 断言 |
| M2 契约过严导致修订风暴 | 灰度期硬失败限 1 轮；软问题不阻断；指标监控 |
| M2 情感最小链变成苦情戏 | 情感只在合法候选中选压力最大事件；外部目标/赌注硬约束 |
| M3 章契约 LLM 失败 | 修复一次 + 最小契约 + 资产不足报错；不写通用句 |
| M3 资产映射破坏旧 prompt | 新契约层叠加，不删除旧资产；flag off 逐字回归 |
| M4 命名规则误伤用户手写名 | 只拦硬冲突；手写名登记不强制改名 |
| M4 生僻字/可读性 | 可读性硬约束；生僻字只用于有理由的配角 |
| M5 人类 A/B 不达标 | 先修契约/命名，不加规则；不宣称已修复 |

---

## 12. 实施纪律

按 AGENTS.md：

1. 修改符号前跑 impact analysis，报告 blast radius；HIGH/CRITICAL 先告警。
2. 提交前跑 `detect_changes()`，确认只影响预期符号/流程。
3. 每次修改后：
   - `cargo check`
   - `cargo test --lib`
   - `cargo clippy`
   - `cargo +nightly-2026-10-05 fmt -- --check`
   - `npx tsc --noEmit`
   - `npx vitest run`
   - `npx playwright test`（涉及前端/切换时）
   - `npm run format:check`
   - `python3 scripts/architecture_guard.py`
4. 不本地执行 `cargo tauri build`，除非用户明确要求「构建/打包」。
5. 每个里程碑单独提交/打 tag；不 force push 已有 tag。
6. 每个里程碑更新 README/CHANGELOG/AGENTS/PROJECT_STATUS/ROADMAP/ARCHITECTURE/TESTING/USER_GUIDE。
7. 不用 find-and-replace 改名；用 call-graph 感知的 rename。
8. 未跑通验收探针，不宣称完成。

---

## 13. 批准后的第一步

建议从 **M1-1.1 后端 active story 真源** 开始：

1. 新增 `set_active_story/get_active_story` 与 `seq`；
2. 加 Rust 单测；
3. 再改幕后入口与幕前 controller；
4. 最后做 E2E 生成中切换。

M1 独立于 NCC，可先发 `v0.78.1`；M2 随后以 flag 灰度进入 `v0.79.0`。
