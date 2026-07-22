# 🧪 StoryMoss 自动化测试环境 (v0.30.15)

本机已配置 Playwright 无头浏览器自动化测试环境，专为 AI 助手设计。

## 测试统计

### v0.30.15 变更说明

- 场景围绕故事大纲生成（创作原则加固）：根因 A 场景大纲生成 `generate_scene_outline` 复用故事级 outline_planner 提示词且不注入 story_outlines.content 致幻觉新角色"金敏秀"；根因 B writer（TimeSliced/TriShot）prompt 从不包含故事大纲致续写偏离。Fix A：新增场景级提示词 `scene_outline.md`（强制复用已登场角色、禁止发明新角色、围绕故事大纲节点）+ `generate_scene_outline` 注入故事大纲 + `build_outline_prompt` 分流；Fix B：`WriteTimeBundle` 加 `story_outline` 字段 + `to_prompt` 红线后插入权威段【故事大纲（必须围绕展开）】，一处覆盖两条 writer 路径，冲突时以故事大纲为准。
- 新增 4 个回归测试：WriteTimeBundle `to_prompt` story_outline 渲染 / 红线-故事大纲顺序不变量 / story_outline 缺失不渲染；registry `scene_outline` 提示词注册（含"禁止发明新角色"指令）。
- 全量基线：`cargo test --lib` 964 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.14 变更说明

- 续写返回风格增强模板修复（多步 plan 尾部非 writer 覆盖正文，第 5 次复发）：`execute_plan`（executor.rs:685-687）用最后产出 `content` 的步骤作为 `final_content`，force-correction（防线 2）只修正首步无法拦截尾部 style_enhancer/inspector，其模板/报告覆盖 writer 正文；新增防线 3 `PlanGenerator::sanitize_plan_for_prose_request` 在咽喉点（force-correction 之后）对所有 `is_prose_request` plan 净化（移除 builtin.style_enhancer/text_formatter/character_voice/emotion_pacing；续写塌缩单 writer；其余弹出尾部非 writer 保证末步 writer；空则补 writer），保留 [inspector, writer] Rule 9 流，非 prose（Audit）不净化。
- 新增 12 个 sanitize 回归测试（`sanitize_plan_for_prose_request`：inspector+style_enhancer 多步、style_enhancer 单步、writer+style_enhancer、inspector+writer Rewrite 保留、续写多步塌缩、续写单 writer 不变、Audit 不净化、outline+writer 保留、outline 单步补 writer、空 plan、无分类不净化、净化 writer 步带 instruction/current_content/story_id）。
- 全量基线：`cargo test --lib` 960 passed（+12）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550）。

### v0.30.13 变更说明

- 续写返回风格增强模板修复（SING 路径绕过 force-correction）：SING（IntentionGraphPlanner）路径直接返回 plan、绕过 `PlanGenerator::generate_plan` 内的防线 2，续写被 SING 路由到 `builtin.style_enhancer` 返回"请提供需要增强的原始文本"模板；提取 `PlanGenerator::force_correct_first_step_to_writer`（`pub(crate)`，封装 swap + understanding/purpose 标注）在 plan 执行咽喉点 `execute_with_context`（`execute_plan` 之前，所有 plan 来源 SING/PlanGenerator/fallback 必经）统一施加，幂等。
- 新增 4 个咽喉点 force-correction 回归测试（`force_correct_first_step_to_writer`：SING style_enhancer 修正、writer 幂等不改动、空 plan 不 panic、inspector 续写修正）。
- 全量基线：`cargo test --lib` 948 passed（+4）；fmt / architecture_guard 全绿；clippy 零新增（baseline 550 -> 549）。

### v0.30.12 变更说明

- planner force-correction 覆盖 inspector（修复续写误路由到 inspector 返回质检报告）：提取纯函数 `PlanGenerator::should_force_correct_to_writer`（可单测），将 inspector 纳入 swap-to-writer 列表，按 LLM 分类分流（续写/创世/无分类/审查+prose 强制 writer；纯 Audit(非prose)/Rewrite 保留 inspector）；提示词 Rule 9 澄清续写≠refine、Rule 21 加入 inspector 禁用。
- 新增 8 个 force-correction 回归测试（`should_force_correct_to_writer`：inspector 续写/审查/改写/创世/无分类分流）。
- 全量基线：`cargo test --lib` 944 passed（+8）；`npx vitest run` 305 passed；tsc / fmt / clippy / format 全绿。

### v0.30.11 变更说明

- 意图识别 LLM 化重构：用 `IntentParser::classify_writing_intent`（`src-tauri/src/intent.rs`）单次 LLM 调用产出 `WritingIntentClassification`，替换 6 处高风险朴素子串匹配点（`is_novel_creation_intent`、`find_template` 禁用、`from_instruction_and_context` 优先级 bug 修复 + hint 参数、force-correction 读 `is_prose_request`、`extract_genre` 否定+排序、intention_graph builder LLM 加固）；新增 `classify_intent` Tauri 命令与前端 `classifyIntent` IPC。
- 新增测试：`parse_classification_json` 解析单测覆盖合法/容错路径；`is_prose` 别名回归测试锁定 force-correction 路径读取 `is_prose_request`。
- 全量基线：`cargo test --lib` 936 passed（+7）；`npx vitest run` 305 passed（+8）；tsc / fmt / clippy / architecture_guard 全绿。

### v0.30.10 变更说明

- 续写返回风格增强模板修复（模板匹配误路由 + content 空兜底注入）：新增 5 个 Rust 单测（`inject_content_fallback` 5 场景：depends_on 注入 / content 已存在跳过 / 无 outputs 时用 current_content / 无 content 返回 false / 优先 outputs 胜过 current_content）；executor.rs 续写意图跳过模板匹配；mod.rs force-correction 扩展到 style_mimic/plot_analyzer/builtin；Rule 21 强化。
- 全量基线：`cargo test --lib` 929 passed（+5）；fmt / clippy 无新增告警。无前端变更。

### v0.30.9 变更说明

- 续写返回 Inspector 审查模板修复（inspector draft 空内容兜底注入）：新增 5 个 Rust 单测（`inject_inspector_draft_fallback` 5 场景：depends_on 注入 / draft 已存在跳过 / dep 不存在扫描兜底 / 无 content 返回 false / 空 content 跳过）；planner 提示词增加 inspector `draft={{step_N}}` JSON 示例与 Rule 9 强化。
- 全量基线：`cargo test --lib` 924 passed（+5）；fmt / clippy 无新增告警。无前端变更。

### v0.30.8 变更说明

- 全面修复 nullable 列读取（`Invalid column type Null` 系列）：新增 2 个 Rust 单测（world_buildings cultures/rules NULL 兜底 + 合法 JSON 解析）；系统性修复 8 个 repository 文件 31 处 nullable 列读取（scene/version/studio/writing_style/kg/user_pref），全部改为 `Option<String>` + 兜底；V112/V113 迁移回填所有 nullable JSON/TEXT 列。
- 全量基线：`cargo test --lib` 919 passed（+2）；fmt / architecture_guard 全绿。无前端变更。

### v0.30.7 变更说明

- 计划执行失败修复（LLM 在 depends_on 写入上下文名）：新增 2 个 Rust 单测（`topological_sort` 非 step_id 依赖跳过 + 混合依赖排序），验证 executor 依赖校验与 `topological_sort` 行为一致。
- 全量基线：`cargo test --lib` 917 passed（+2）；fmt / tsc / architecture_guard 全绿。无前端变更。

### v0.30.4 变更说明

- 幕前输入历史持久化（按故事隔离）：新增 2 个 vitest（持久化写入 + 重载后 ↑ 召回），覆盖 localStorage 读写与故事隔离。
- 全量基线：`npx vitest run` 297 passed（+2）；`npx tsc --noEmit` 通过；`prettier --check` 通过。无 Rust 变更。

### v0.30.0 变更说明

- Agency 多代理创作框架（创世 2.0）P1–P5 全部完成，测试体系包括：`src-tauri/src/agency/` 26+ 单元测试（coordinator / gate / graders / board / budget / session / learning / eval_harness 等）；eval harness JSON 场景随 `cargo test` 运行；CI 另设 `cargo test --lib agency::eval_harness` 专用门禁 step。
- 全量基线：`cargo test --lib` 877 passed；`npx vitest run` 295 passed。

### v0.26.59 变更说明

- 新增官网落地页组件测试（landing）：`DownloadButton` 平台检测与链接断言 8 tests；全量 landing 19 passed。
- 无 Rust/前端应用逻辑变更，全量基线保持 `cargo test --lib` 770 passed、`npx vitest run` 292 passed。

### v0.26.57 变更说明

- 新增 `chapter_splitter` 单元测试 7 passed（mode_parse、resolve_max_chars、word_count/plot 切分边界）。
- 新增 `export::assemble` 单元测试 8 passed（scenes 为真相源、孤儿场景、标题回退）。
- 新增 `prompts::registry` 测试：目录解析、场景组合预览。
- 前端新增 `useExport.test.ts` 4 passed（取消保存、文本/二进制处理、空内容拒绝）。
- 前端 `PromptsPanel.test.tsx` 5 passed（加载、展开编辑器、导入参数、打开目录、组合预览）。
- 全量基线：`cargo test --lib` 769 passed；`npx vitest run` 292 passed。

### v0.26.56 变更说明

- executor 写 AppConfig 契约测试加 `mock_app_config_lock`，并行 `--test-threads=8` 稳定。

### v0.26.55 变更说明

- 新增 `ModelCard.enabled.test.tsx`（开启/关闭开关契约）。
- Rust：`apply_disable_side_effects` / `disabled_model_excluded_from_gateway_registry` / `test_probe_model_rejects_missing_or_disabled` / `test_disabled_model_not_selected_after_registry_reload`。

### v0.26.54 变更说明

- `clear_demotion` / `demoted_degraded_creative_still_promoted` / `auto_clears_sticky_unhealthy_creative` / `user_sets_creative_x_overrides_demoted_y` / `sync_creative_to_active_llm` 契约通过。

### v0.26.53 变更说明

- `FrontstageHeader`：单击故事名不得调用 `onOpenBackstage`；设置按钮可回幕后；双击仍进编辑。

### v0.26.52 变更说明

- `include_in_gateway_status` / `is_promotable_user_model` / `sync_creative_to_active_llm` 契约 4 passed。
- `useSyncStore.bug.spec`：`model_config`/`app_settings` 失效含 `gateway-status` 5 passed。

### v0.26.51 变更说明

- `displayStoryTitle` / `displayChapterTitle` 展示契约；`FrontstageHeader` / `EditableChapterTitle` 双击改名交互 30 passed。

### v0.26.50 变更说明

- `story_system::scene_service`：AutoIngest 防抖窗口契约 6 passed（含 debounce=commit 同窗）。
- `useBackendActivityListener.contract`：contract-auto-progress 不得注册 running 2 passed。

### v0.26.49 变更说明

- `agents::orchestrator`：`last_n_sentences` / `build_ending_anchor` / 纪律后置序契约 3 passed。

### v0.26.48 变更说明

- `updater::tests`：下载进度累加 + 404 错误文案契约 2 passed。
- CI：`verify-updater-manifest` 在 tag 发布后校验 `latest.json`。

### v0.26.47 变更说明

- 无测试变更；`cargo +nightly fmt -- --check` 恢复通过。

### v0.26.46 变更说明

- `background_generate_templates_declare_strategy_section`：5 个 externalized 模板契约。
- `normalize_methodology_id` / `final_methodology_step_after_genesis` / Genesis strategy notes 注入契约。
- 拆书 chunker 与 StoryArc 持久化相关单测（见 commit 5a5c9b1）。

### v0.26.45 变更说明

- `narrative::protagonist_card`：merge/render/probe/soft_retry 契约 6 passed。
- Genesis first_scene 增加 `protagonist_card` 变量；Call3 尾注注入。

### v0.26.44 变更说明

- Genesis `quick_phase_steps` 契约更新为四步（含「铺设开篇骨架」）；新增 `parse_opening_skeleton` / `opening_skeleton_from_concept` 契约测试。
- `extract_story_meta_fallback` 覆盖加厚字段（protagonist_name / core_conflict / world_one_liner）。

### v0.26.43 变更说明

- StatusIcon / FrontstageBottomBar：emoji→Lucide + 状态解析修复（+4）。
- vitest 全绿。

### v0.26.42 变更说明

- RichTextEditor：接受后 30s 内新续写须显示幽灵段落（+1）。
- `RichTextEditor.duplicate.test.tsx` 6 passed。

### v0.26.41 变更说明

- finalize scene_id 直写单测 3；MemoryFacade unified facts 7；V104–V106 迁移。
- `cargo test --lib` 701 passed。

### v0.26.40 变更说明

- Sidebar impact badges / 诊断默认折叠；SceneEditor pipeline rail；PromptCoverageBar；MemoryFacade KG top-5。
- `cargo test --lib memory::facade` 5；相关 vitest 15+。

### v0.26.39 变更说明

- Sidebar 五组 IA + Insights 三 Tab vitest；`writing-stats` 重定向契约。
- vitest 249 passed（+5）。

### v0.26.38 变更说明

- PromptsPanel：展开编辑器 / 打开目录 / 组合预览 vitest；framework_guidance + preview_prompt_composition Rust 单测。
- `cargo test --lib` 690 passed（+5）；vitest 244 passed（+2）。

### v0.26.37 变更说明

- `updateSceneIpc` 契约测试；幕前自动保存参数形状锁定。

### v0.26.36 变更说明

- 配置热同步：`app_settings` sync、editor/theme Tauri 事件；vitest +3（useEditorConfig / useSyncStore）。

### v0.26.35 变更说明

- Dashboard `scene_count` 单测数据对齐；幕后导航统一 store；`apply_wizard_to_story` 为新增 IPC（跨层）。
- 前端 R7–R11 为 UI/导航改动，以 `tsc` + 既有 vitest（含 Dashboard）门禁为主。


| 套件                                | 数量     | 状态                           |
| ----------------------------------- | -------- | ------------------------------ |
| `cargo test --lib`                  | 690      | ✅ 0 failed / 2 ignored        |
| `cargo test --lib prompt_synthesis` | 19       | ✅（TriShot 三击管线全部通过） |
| `cargo test --lib narrative::genesis` | 12     | ✅（创世四步/骨架解析/重试闸门/payload 契约） |
| `cargo test --lib narrative::protagonist_card` | 6 | ✅（人物卡 merge/render/probe） |
| `npx tsc --noEmit`                  | 前端类型 | ✅                             |
| `cargo check`                       | —        | ✅ 零错误                      |
| `npm run format:check`              | 代码风格 | ✅ 零差异                      |

| 类型           | 数量      | 状态                                         |
| -------------- | --------- | -------------------------------------------- |
| Rust 单元测试  | 685       | ✅ 全部通过 (`cargo test --lib`)             |
| 前端单元测试   | 242       | ✅ 全部通过 (`vitest run`)                   |
| 前端构建测试   | —         | ✅ `npm run build` 通过                      |
| Tauri 构建测试 | —         | ✅ `cargo tauri build` 通过                  |
| Playwright E2E | 41 (36+5) | ✅ 行为驱动测试（CI 中 `continue-on-error`），其中 `genesis-duplicate.spec.ts` 验证自动接受后幽灵段落隐藏 |

### v0.26.24 新增测试

- **散布式句子块重复**：Rust `test_trim_self_repetition_interspersed_*` + TS `trimInterspersed*` 用例；golden fixture 新增 `interspersed_repeated_block` / `interspersed_short_sentence_unchanged`。
- **跨内容重叠剥离**：Rust `test_strip_existing_overlap_*`（6 条）；TS `stripExistingOverlap` / `sanitizeContinuationOutput` 用例。
- **截断末句裁剪**：Rust/TS `trimDanglingTail` 用例。

### v0.26.28 Phase 4 新增测试

- **策略选择移入 Quick Phase**：`genesis.rs` `quick_phase_steps` 为「概念 → 策略选择 → 铺设开篇骨架 → 撰写开篇」四步（v0.26.44）、`background_steps` 为 5 步的单元契约；步骤 `step_number`/`total_steps`/`progress_percent` 一致性覆盖。
- **Prompts 外部化**：`prompts/registry.rs` 运行时加载 `resources/prompts/**/*.md` 的集成测试；95 个提示词全部可解析且公开 API 保持不变的回归测试。
- **迁移脚本拆分**：`MigrationRunner` + `RustMigration` trait 对 70 个编号 Rust 迁移与 SQL 迁移统一排序、过滤、执行的测试；`schema_migrations` 版本语义保持不变的兼容性测试。
- **知识图谱手动 CRUD UI**：新建实体、添加关系交互的 Playwright 覆盖。
- **世界构建 AI 生成 / 角色 AI 扩展 / 叙事分析图表**：对应组件渲染、API 调用、数据回写的单元与集成测试。

### v0.26.27 Phase 3 新增测试

- **L4 诊断互链**：GenesisPanel → TracingPanel / Logs 跳转与预过滤行为覆盖；TracingPanel detail → GenesisPanel 回跳选择对应 run 覆盖。
- **UsageStats operation 分组**：按 `operation` 字段拆分的四标签页渲染与聚合逻辑测试。
- **Foreshadowing UX**：`setup_scene_id` 下拉绑定 `useScenes`、高级区 `target_start_scene` / `target_end_scene` 编辑交互测试。
- **前端循环依赖守卫**：`npx madge --circular src/main.tsx` 验证循环数为 0；新增 `types/editor.ts`、`stores/contracts/*` 的导入方向单测。
- **Tauri 循环依赖守卫**：`creative_engine ↔ llm` 已无互相 import；`model_gateway ↔ router` 仍存少量直接 import，静态检查标记后续迁移目标；`ports/` / `domain/` 共享 trait 的单元测试。

### v0.26.26 Phase 2 新增测试

- **角色编辑与关系 CRUD**：`CharacterEditModal` 与 `CharacterRelationshipForm` 的创建 / 更新 / 删除路径测试。
- **L2 创世溯源徽章**：`is_auto_generated` / `source` 字段在角色、场景、世界观、知识图谱等页面的显示规则测试。
- **Story System 合同播种状态**：MASTER_SETTING + CHAPTER_1 合同状态卡渲染；失败运行警告摘要测试。
- **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开 frontstage 的交互测试。
- **Repository trait 注入**：`creative_engine` 通过 `db/traits.rs` 调用 repository 的契约测试；`db/repositories/*.rs` 拆分后 re-export 一致性测试。

### v0.26.25 Phase 1 新增测试

- **GenesisPanel 步骤模型**：`src-frontend/src/utils/__tests__/genesisSteps.test.ts` 验证 Quick（3 步）+ Background（5 步）顺序、`steps_json.errors` 展示、story / 幕前跳转。
- **仪表盘统计卡**：点击跳转对应页面与口径一致性测试。
- **Stories Wizard 重复建故事**：已有故事 update 路径不重复创建的故事级测试。
- **后端特征测试**：
  - `model_gateway/executor.rs`：happy path + 模型降级 / 超时错误路径。
  - `db/repositories.rs`：创建 / 更新 / 删除 round-trip 与级联清理。
  - `memory/ingest.rs`：实体关系提取成功与字段缺失降级路径。

### v0.26.19 新增测试

- **Rust Genesis 契约测试**：`compute_trim_ratio`/`should_retry_self_repetition`/`select_first_chapter_content`/`build_first_chapter_chapter_switch` 纯函数边界与 payload 契约；`background_steps` 6 步固定顺序；`world_concept_for_character_prompt`；mutex 中毒恢复；`GenesisStepError` 严重度分级与累计；`genesis_runs` 状态流转。
- **跨层共享 trim golden fixture**：`tests/fixtures/trim_golden.json`（7 条用例），Rust `trim_self_repetition_matches_shared_golden_fixture` 与 TS `textCleanup.golden.test.ts` 双跑同一 fixture，锁定跨层一致性。
- **前端 Gap B/C + 状态机**：Gap B（空 finalContent 不锁 delivered）、P0-3（懒加载失败不锁 delivered）、Gap C（delivered + 编辑器有内容 → 跳过 setContent）、p4-4（重复入站也跳过）、状态机端点契约。

### 测试文件分布

**前端单元测试** (`src-frontend/src/**/*.test.{ts,tsx}`):

- `frontstage/hooks/`：useFrontstageWensi ×6、useFrontstagePanels ×8、useFrontstageEditor ×7、useFrontstageGeneration ×6
- `frontstage/components/`：HelpPanel ×3、ZenModeExit ×2、FrontstageHeader ×11、FrontstageSidebar ×3、FrontstageBottomBar ×3、FrontstageApp ×5
- `utils/`：cn ×5、format ×14、numberFormat ×19、settings ×4
- `hooks/`：useSettings ×4
- `services/`：settings ×4
- 其他：smoke ×1、useSyncStore.bug ×1、LlmProfileForm.bug ×1

**Rust 单元测试** (`src-tauri/src/**/*.rs` 内 `#[cfg(test)]`):

- `db/repositories_tests.rs`：18 例
- `db/cascade_tests.rs`：6 例
- `db/repositories_narrative.rs`：3 例（source/status round-trip、repository 读写 round-trip）
- `canonical_state/tests.rs`：8 例
- `task_system/tests.rs`：15 例
- `task_system/integration_tests.rs`：5 例
- `prompts/registry.rs`：提示词注册表测试（内置 prompt 解析、覆盖读取、分类枚举）
- `creative_engine/anti_ai/`：AntiAiRewriter 4 例、OpeningClarityGate 5 例、LivingAuthorGuard 6 例
- `utils/validation_tests.rs`：16 例
- `utils/style_align.rs`：3 例
- `utils/text.rs`：12 例（新增 `trim_self_repetition` 自重复清理测试）
- `utils/file.rs`：3 例
- `pipeline/executor.rs`：9 例
- `pipeline/refine.rs`：3 例
- `pipeline/review.rs`：3 例
- `story_system/scene_service.rs`：5 例
- `narrative/elements.rs`：8 例
- `creative_engine/style/dna.rs`：4 例
- `book_deconstruction/parser.rs`：3 例
- `config/settings_tests.rs`：13 例

**E2E 测试** (`e2e/*.spec.ts`):

- `storymoss.spec.ts`：12 例（数据持久化、页面加载、响应式）
- `frontstage-editing.spec.ts`：7 例（编辑器、自动保存、模式切换）
- `backstage-pages.spec.ts`：8 例（各页面加载与导航）
- `navigation.spec.ts`：4 例（URL 路由）
- `context-menu.spec.ts`：2 例（右键菜单）
- `example.spec.ts`：1 例（冒烟测试）
- `performance/tiptap-benchmark.spec.ts`：2 例（性能基准，默认跳过）

## ✅ 已安装组件

| 组件       | 版本          | 状态      |
| ---------- | ------------- | --------- |
| Bun        | 1.3.6         | ✅        |
| bunwv      | 0.0.5         | ✅ (备用) |
| Playwright | latest        | ✅        |
| Chromium   | 147.0.7727.15 | ✅        |

## 🚀 快速开始

### 1. 运行所有测试

```bash
npm test
# 或
npx playwright test
```

### 2. 截图所有页面

```bash
npm run screenshot
```

### 3. 快速截图幕前界面

```bash
npm run screenshot:front
```

### 4. 快速截图幕后界面

```bash
npm run screenshot:back
```

### 5. 交互式调试

```bash
npm run test:ui
```

## 📸 截图示例

测试环境已成功截图：

### 幕前界面 (Frontstage)

- 温暖纸张色调 (#f5f4ed)
- 简洁写作界面
- AI 续写功能入口

### 幕后界面 (Backstage)

- 深色影院主题
- 仪表盘统计
- 左侧导航菜单

截图保存在 `e2e/screenshots/` 目录。

## 🛠️ 测试脚本

### 使用 test-helper.js

```bash
# 显示帮助
node scripts/test-helper.js help

# 启动开发服务器
node scripts/test-helper.js start

# 运行测试
node scripts/test-helper.js test

# 截图
node scripts/test-helper.js screenshot

# 清理截图
node scripts/test-helper.js clean

# 查看报告
node scripts/test-helper.js report
```

### 使用 BrowserTestHelper 类

```typescript
import { BrowserTestHelper, runTest } from "./e2e/test-helper";

// 方式 1: 使用 runTest 包装器
runTest(async (helper) => {
  await helper.navigate("http://localhost:5173");
  await helper.screenshot("homepage");
  await helper.click("button");
  await helper.type('input[name="title"]', "测试标题");
  await helper.sleep(1000);
});

// 方式 2: 手动控制
const helper = new BrowserTestHelper();
await helper.start("chromium", false); // 启动有界面浏览器
await helper.navigate("http://localhost:5173");
await helper.screenshot("test");
await helper.stop();
```

## 📝 测试命令参考

### 导航

- `helper.navigate(url)` - 导航到 URL
- `helper.getTitle()` - 获取页面标题
- `helper.getUrl()` - 获取当前 URL

### 截图

- `helper.screenshot(name)` - 截图保存
- `helper.sleep(ms)` - 等待指定时间

### 交互

- `helper.click(selector)` - 点击元素
- `helper.clickText(text)` - 点击包含文本的元素
- `helper.type(selector, text)` - 输入文本
- `helper.clear(selector)` - 清除输入框
- `helper.press(key)` - 按下按键
- `helper.scroll(dx, dy)` - 滚动页面

### 等待

- `helper.waitFor(selector)` - 等待元素出现
- `helper.waitForText(text)` - 等待文本出现

### JavaScript

- `helper.eval(script)` - 执行 JS 代码
- `helper.getText(selector)` - 获取元素文本
- `helper.exists(selector)` - 检查元素是否存在

## 🎯 测试场景示例

### 测试版本管理功能

```typescript
test("版本时间线截图", async ({ page }) => {
  await page.goto("/index.html#/scenes");
  await page.waitForTimeout(3000);

  // 查找版本时间线组件
  const versionTimeline = page.locator('[data-testid="version-timeline"]');
  if (await versionTimeline.isVisible()) {
    await versionTimeline.screenshot({
      path: "e2e/screenshots/version-timeline.png",
    });
  }
});
```

### 测试响应式布局

```typescript
test("多分辨率测试", async ({ page }) => {
  const sizes = [
    { width: 1920, height: 1080, name: "desktop" },
    { width: 1366, height: 768, name: "laptop" },
    { width: 768, height: 1024, name: "tablet" },
  ];

  for (const size of sizes) {
    await page.setViewportSize(size);
    await page.goto("/frontstage.html");
    await page.screenshot({
      path: `e2e/screenshots/responsive_${size.name}.png`,
    });
  }
});
```

## 🔧 配置说明

### Playwright 配置 (playwright.config.ts)

```typescript
export default defineConfig({
  testDir: "./e2e",
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  use: {
    baseURL: "http://localhost:5173",
    screenshot: "only-on-failure",
    video: "on-first-retry",
  },
  webServer: {
    command: "cd src-frontend && npm run dev",
    url: "http://localhost:5173",
  },
});
```

## 📊 测试报告

运行测试后查看报告：

```bash
npm run test:report
```

报告位于 `playwright-report/` 目录。

## 🐛 故障排除

### 浏览器未安装

```bash
npx playwright install chromium
```

### 端口被占用

修改 `playwright.config.ts` 中的端口配置。

### 测试超时

增加 `timeout` 配置：

```typescript
timeout: 60000, // 60秒
```

## 📚 参考文档

- [Playwright 官方文档](https://playwright.dev/)
- [bunwv GitHub](https://github.com/NatiCha/bunwv)
- [StoryMoss 架构文档](./ARCHITECTURE.md)

---

_最后更新: 2026-07-22 - v0.30.15 场景围绕故事大纲生成，测试基线 964_
