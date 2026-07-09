# StoryForge (草苔) v0.26.57 项目完成状态

> 最后更新: 2026-07-09（v0.26.57 自动划分章节、本地导出保存与提示词目录）
> GitHub: https://github.com/91zgaoge/StoryForge

---

## ✅ 最近完成功能

### v0.26.57 — 自动划分章节、本地导出保存与提示词目录（2026-07-09）

- 后台设置新增「划分章节方式」：`word_count` 按字数（上限留空默认 3000 字）、`plot` 按情节；场景保存空闲 30s 后仅对最新章自动切分。
- 导出结果走系统原生保存对话框，文本格式直接写 UTF-8，pdf/epub 复制后端临时文件；取消时不关闭导出弹窗。
- 提示词注册表新增「打开目录」按钮，直接打开 bundled prompts 资源目录；编辑器改为原生 textarea，避免 Monaco CDN 被 CSP 拦截。
- ✅ **验证**：`cargo test --lib` 769 passed；`npx vitest run` 292 passed；tsc / fmt / format:check 全绿。

### v0.26.56 — 网关契约测试串行化（2026-07-09）

- mock app_data_dir 写 config 契约加锁，消除并行污染。

### v0.26.55 — 幕后模型列表开启/关闭（2026-07-09）

- 模型卡片「开启/关闭」开关；仅轮询已启用模型；复用 v0.26.54 fail-closed。
- `is_promotable_user_model` 要求仍在网关注册表。

### v0.26.54 — 修复创作模型被粘性降级绕过（2026-07-09）

- 显式创作角色不受连续失败 demotion 拦截；粘性 Unhealthy 在 resolve 清一次再探。
- `set_active_model` / `save_settings` 调用 `clear_model_demotion`；`generate()` 再提升用 `is_promotable`。
- ✅ **验证**：gateway/health/commands 契约 6 passed；architecture_guard。

### v0.26.53 — 故事名取消单击回幕后（2026-07-09）

- 故事名仅双击改名；设置按钮为回幕后入口（禅模式保留）。
- ✅ **验证**：Header 单击不调 backstage；设置按钮可回幕后。

### v0.26.52 — 修复模型新增与默认创作模型即时生效（2026-07-09）

- 幕前 `gateway-status` 随 `model_config` 失效；状态栏含 Unknown。
- 创作角色允许 Unknown 置顶；同步 `active_llm_profile`。
- ✅ **验证**：Rust 4；useSyncStore 5；tsc/fmt/architecture_guard。

### v0.26.51 — 幕前故事名与章节名内联改名（2026-07-09）

- 故事：草苔/未命名展示 + 粘贴自动建故事；双击改名。
- 章节：`.chapter-header` + 顶栏状态统一双击改名；`update_scene` 持久化 title。
- ✅ **验证**：相关 vitest 30；tsc / format / architecture_guard。

### v0.26.50 — 修复打字触发后台运行与深度思考假超时（2026-07-09）

- AutoIngest 30s 防抖 + 后台信号量；contract-auto 静默；活动不同步拉高 isGenerating；超时看门狗弹诊断。
- ✅ **验证**：scene_service 6；contract gate 2。

### v0.26.49 — 修复续写与正文脱节（末句硬锚点）（2026-07-09）

- Call3/TimeSliced prompt 最末尾注入末 2 句硬锚点，覆盖开场大纲。
- ✅ **验证**：ending_anchor 3 passed。

### v0.26.48 — 修复自动更新：GitHub Releases + latest.json（2026-07-09）

- `createUpdaterArtifacts` + AppImage；CI 上传签名包；tag 后校验 `latest.json`。
- ✅ **验证**：updater 2 passed。

### v0.26.47 — CI 热修复：Rust fmt（2026-07-09）

- 修复 v0.26.46 rust-check 失败；无逻辑变更。

### v0.26.46 — 创世方法论全链路、题材 match-or-create 与拆书持久化（2026-07-09）

- Background 模板恢复方法论占位符；Genesis 分步注入 + step 推进；HDWB ID 统一。
- EnsureGenreProfile match-or-create；拆书 StoryArc/作者/伏笔 + 分块止血。
- ✅ **验证**：genesis/methodology/prompt 契约 20+ passed。

### v0.26.45 — Genesis 人物卡强制落地（姓名 + 欲望/阻力）（2026-07-09）

- ProtagonistCard 双重注入 + 三信号探针 + 软重试；零新 LLM。
- ✅ **验证**：narrative 61；protagonist_card 6。

### v0.26.44 — Genesis 首章质量：开篇骨架与提示词加厚（2026-07-09）

- quick_phase 四步；OpeningSkeletonStep（≤10s fail-open）；概念加厚；strategy 中文；四元组；占位角色去硬编码；纪律单源。
- ✅ **验证**：`narrative::genesis` 12 passed；extract_story_meta 2 passed。

### v0.26.43 — 修复底部状态栏 emoji 显示为方框（2026-07-09）

- 阶段文案去 emoji；StatusIcon 渲染；状态解析修复。
- ✅ **验证**：StatusIcon/BottomBar 相关 18+；vitest 全绿。

### v0.26.42 — 修复续写 Tab 提示可见但无幽灵文本（2026-07-09）

- 新续写清零 hideGhostUntil / postAcceptHideUntil；接受中不误解除。
- ✅ **验证**：RichTextEditor.duplicate 6 passed（+1）。

### v0.26.41 — 记忆统一读模型与 Finalize scene_id 根治（2026-07-09）

- Finalize 按 scene_id 直写；story_memory_facts VIEW + kg_entity_id；表不 DROP。
- ✅ **验证**：cargo 701；finalize 3；facade 7；vitest 261。

### v0.26.40 — 幕后资产闭环 P0–P3（2026-07-09）

- 侧栏影响徽章；SceneEditor 管线轨；KG→Bundle；MCP→设置扩展；MemoryFacade；prompt 覆盖率。
- ✅ **验证**：memory::facade 5；相关 vitest 15+。

### v0.26.39 — 幕后信息架构全面重排（2026-07-09）

- 侧栏五组分类 + 中文命名；数据洞察三 Tab；设置七 Tab；拆书设置就近；账号死链修复。
- ✅ **验证**：vitest 249；tsc/format 通过。

### v0.26.38 — 提示词面板修复与组合智能化（2026-07-09）

- 面板：textarea 替代 Monaco CDN；原生打开目录；导出覆盖/完整包。
- 运行时：Call 1 `methodology`/`contextual_injectors` 回灌 Call 3；场景组合预览。
- ✅ **验证**：cargo test 690；vitest 244；tsc/fmt/architecture_guard 通过。

### v0.26.37 — 修复幕前「保存中」常亮与字数不更新（2026-07-09）

- 幕前 `update_scene` 参数改为 `{ scene_id, updates }`；AI 追加后刷新字数并自动保存。
- ✅ **验证**：vitest 242；tsc/format 通过。

### v0.26.36 — 后台配置变更即时生效（超时/字体/主题热同步）（2026-07-09）

- `save_settings` 热重载 LLM + 广播 `app_settings`；幕前/幕后 Query 即时失效。
- `llm_first_chunk_timeout_secs` 接入适配器；TriShot 预算与 writer prompt 读真实配置。
- 字体/色调主题经 Tauri 事件跨窗口即时同步。
- ✅ **验证**：cargo test 685；vitest 240；fmt/tsc/architecture_guard 通过。

### v0.26.35 — 全面落地幕后工作室审计残留 R1–R11（2026-07-09）

对照 `docs/AUDIT_BACKSTAGE_STUDIO_v0.26.34.md` 残留项一次性关闭：

- **R1**：`list_stories` → `StoryListItem.scene_count`；Dashboard「场景」用真实场景数。
- **R2**：CreationPathGuide 快速创作 → `runCreationWorkflow`；导航统一 `appStore.currentView`。
- **R3**：后端 `apply_wizard_to_story`（去重 + KG）；前端单 IPC。
- **R4**：幕后监听 `genesis-warnings` + GenesisPanel 刷新。
- **R5/R6**：Pipeline/SceneEditor 场景序号语义标注。
- **R7–R11**：文风 Tab、UsageStats 启发式、伏笔 Kanban、角色→场景跳转、拆书转故事导航。
- ✅ **验证**：见 CHANGELOG / AGENTS 本版本门禁结果。

### v0.26.34 — 修复提示词导入参数并新增「打开本地目录」功能（2026-07-09）

- **修复批量导入静默失败**：`PromptsPanel.handleImportAll` 参数键由 `promptId` 修正为 `prompt_id`，与后端命令字段命名对齐。
- **新增「打开目录」功能**：后端新增 `get_prompts_directory` 命令；前端标题栏新增按钮，使用系统文件管理器打开当前 prompts 资源目录。
- **新增「刷新」按钮**：重新加载提示词列表与目录路径。
- **改善错误展示**：加载失败时页面显示具体错误信息。
- **导出/导入按钮归位**：移至页面标题栏，避免与重置操作混淆。
- ✅ **验证**：`cargo test --lib` 685 passed；`npx vitest run` 237 passed / 3 skipped；`cargo +nightly fmt -- --check`、`npx tsc --noEmit`、`architecture_guard.py`、`npm run format:check`、`npm run build` 均通过。

### v0.26.33 — 补齐阶段 2/3/4 具体缺口：KG/角色关系删除、前端解耦（2026-07-08）

- **知识图谱实体归档与关系删除 UI**（Stage 4）：后端新增 `archive_entity` / `delete_relation` 命令；实体详情面板与关系列表新增删除/归档按钮。
- **角色关系删除 UI**（Stage 2）：`useDeleteCharacterRelationship` hook + `Characters.tsx` 关系卡片删除按钮。
- **前端 `frontstage ↔ components` 解耦**（Stage 3）：新增 `hooks/contracts/useEditorConfig.ts`；`FrontstageApp` / `RichTextEditor` 不再直接 import `EditorSettings.tsx`；循环依赖数为 0。
- ✅ **验证**：`cargo test --lib` 684 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 234 passed / 3 skipped；`npx tsc --noEmit` / `architecture_guard.py` / `npm run format:check` 通过。

### v0.26.32 — 完成阶段一剩余项：L1 创作入口、仪表盘统计卡、memory/ingest 测试（2026-07-08）

- **L1 创作入口 UX 统一**：`CreationPathGuide` 卡片可点击；Dashboard “AI 创建故事”主按钮进入幕前 Genesis 流程。
- **仪表盘统计卡修正**：“章节”改为“场景”，新增“字数”统计卡，数据源对齐 `useStories`。
- **`memory/ingest` 测试补齐**：新增 5 条 happy/error 路径测试，不依赖 LLM。
- **新增文档**：`docs/plans/2026-07-08-storyforge-phase1-execution-plan.md` 记录与综合优化计划的对照及执行方案。
- ✅ **验证**：`cargo test --lib` 682 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 222 passed / 3 skipped；`npx tsc --noEmit` / `architecture_guard.py` / `npm run format:check` 通过。

### v0.26.31 — 修复幕前状态栏体验、策略解析鲁棒性与新数据库 schema（2026-07-08）

- **幕前顶部状态栏字数统计滞后**：章节加载后 `wordCount` 始终为 0，直到首次自动保存成功才更新；切章时 diff 基准也未重置。
  - `selectChapter` 加载正文后即时计算并设置当前章节字数。
  - `handleContentChange` 中字数变化时同步更新 `wordCount`。
  - 新增回归测试验证章节加载后立即显示非零字数。
- **顶部状态栏字体大小不可点击**：字号显示无点击响应。
  - `FrontstageHeader` 新增 `onOpenFontSettings` 回调，字号显示可点击。
  - 扩展 `show_backstage` 命令支持 `view` / `panel` 参数，点击后打开幕后通用设置并滚动到编辑器设置卡片。
- **底部状态栏后台任务图标 tofu**：emoji 图标在部分系统字体下显示为缺字符号。
  - 8 个活动类别图标全部替换为 `lucide-react` SVG 图标。
  - 新增回归测试验证图标渲染为 SVG。
- **策略选择 JSON 解析失败**：LLM 输出仍可能使用 `reasoning` 或缺失 `rationale`。
  - `SelectedStrategy.rationale` 增加 `#[serde(default, alias = "reasoning")]`。
  - 新增回归测试覆盖 `reasoning` 别名与缺失默认值。
- **新数据库 schema 列缺失**：v0.26.30 兜底修复未覆盖新库建表。
  - `create_tables` 中 `characters` / `scenes` / `world_buildings` / `kg_entities` 新增 `source` / `is_auto_generated` 列。
- ✅ **验证**：`cargo test --lib` 677 passed；`cargo +nightly fmt -- --check` 通过；`cargo clippy --lib` 通过；`npx vitest run` 213 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.30 — 热修复旧数据库缺失 source/is_auto_generated 列（2026-07-08）

- **问题**：部分旧数据库在 v0.26.28 迁移框架切换后，`characters` / `scenes` / `world_buildings` / `kg_entities` 表缺失 `source` / `is_auto_generated` 列，Genesis 与资产查询报 `no such column: source`。
- **修复**：
  - 新增 Rust migration `V103__ensure_source_columns`，幂等补回缺失列。
  - `init_db` 新增 `ensure_source_columns` 启动兜底修复。
  - 新增回归测试覆盖 `schema_migrations=102` 但列缺失场景。
- ✅ **验证**：`cargo test --lib` 674 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.29 — 热修复策略选择 JSON schema 不匹配（2026-07-08）

- **问题**：v0.26.28 将 prompts 外部化后，`strategy_selector.md` 模板字段与 `selector.rs` 的 `SelectedStrategy` schema 不一致，Genesis「选择创作策略」步骤报 `VALIDATION_FAILED: missing field rationale`。
- **修复**：
  - 重写 `resources/prompts/strategy/strategy_selector.md`，对齐 `rationale`/`genre_profile_id`/`methodology_id`/`style_dna_ids`/`skill_ids`/`workflow_id`/`parameters` 字段。
  - `selector.rs` 新增 `LegacyStrategyResponse` 兜底解析，兼容旧格式（`selected_strategy`/`reasoning`/`asset_combination`）。
  - 新增 `test_parse_strategy_response_legacy_schema` 单元测试。
- ✅ **验证**：`cargo test --lib` 673 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.28 — Phase 4 架构债务与工程体验（2026-07-07）

- **知识图谱手动 CRUD UI**：Graph 页图例面板新增「新建实体」按钮；实体详情面板新增「添加关系」按钮，支持从当前故事已有实体中按名称搜索并建立关系。
- **世界构建 AI 生成**：`WorldBuilding` 页新增「AI 生成」按钮与 `AiWorldBuildingModal`，基于当前故事调用 `generateWorldBuildingOptions` 一键生成世界观并回写。
- **角色 AI 扩展**：`Characters` 页新增「AI 扩展」按钮与 modal，基于当前世界观调用 `generateCharacterProfiles`，选择角色组后批量 `createCharacter`。
- **叙事分析图表**：`NarrativeAnalysis` 页新增 SVG `ReadingPowerChart` 折线/面积图，替代原有条形图展示追读力趋势。
- **策略选择移入 Quick Phase**：`genesis.rs` 中 `StrategySelectionStep` 从 `background_steps()` 前移至 `quick_phase_steps()`，位于 `ConceptGenerationStep` 之后、`FirstChapterGenerationStep` 之前；同步更新所有步骤的 `step_number`/`total_steps`/`progress_percent` 与前后端测试契约。
- **外部化 prompts**：`prompts/registry.rs` 中 95 个内置提示词迁移至 `resources/prompts/{category}/{id}.md`，运行时从 Tauri 资源目录加载；保留用户覆盖能力。
- **迁移脚本拆分**：`db/connection.rs` 中 2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件；`MigrationRunner` 扩展 `RustMigration` trait，统一排序、过滤、执行 SQL 与 Rust 迁移。

#### 下一 milestone 已识别项

- 无

- ✅ **验证**：`cargo test --lib` 672 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.27 — L4 诊断互链、文档与依赖解耦（2026-07-07）

- **诊断页互链**：GenesisPanel ↔ TracingPanel 双向跳转；Genesis 失败运行 → Logs 深链并预填 `session_id`。
- **用量统计 operation 分组**：全部 / bootstrap / smart_execute / 其他 标签（启发式分组）。
- **伏笔看板 UX**：`setup_scene_id` 场景下拉；可编辑 `target_start_scene` / `target_end_scene`。
- **循环依赖解耦**：前端 `components ↔ stores ↔ hooks ↔ frontstage` 解耦；Tauri `creative_engine ↔ llm`、`model_gateway ↔ router` 解耦。
- **文档补齐**：`USER_GUIDE.md` 补 L4 诊断页、修正过度承诺；元文档同步。
- ✅ **验证**：`cargo test --lib` 672 passed；`cargo +nightly fmt -- --check` 通过；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.26 — L2 资产补齐与领域层止血（2026-07-07）

- **角色编辑与关系 CRUD**：新增 `CharacterEditModal` / `CharacterRelationshipForm`；角色资料与关系 Tab 均可编辑/添加。
- **L2 创世溯源徽章**：世界观、角色、场景、KG 实体显示「创世」徽章；后端写入时标记 `source` / `is_auto_generated`。
- **Story System 合同播种状态卡**：Contracts Tab 显示 `MASTER_SETTING` + `CHAPTER_1` 合同状态；失败 run 显示错误摘要。
- **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开幕前窗口。
- **StorySystem.tsx 拆分**：8 个独立标签组件，主文件 125 行。
- **Repository 层 trait 化与拆分**：`db/repositories.rs` 拆分为模块文件；`creative_engine/context_builder.rs` 依赖 trait。
- ✅ **验证**：`cargo test --lib` 672 passed；`npx vitest run` 210 passed；`architecture_guard.py` 通过。

### v0.26.25 — Backstage Genesis 可观测性与测试基线（2026-07-07）

- **GenesisPanel 动态步骤**：对齐后端 Quick(2) + Background(6)，展示非致命 `errors[]`，支持 story/幕前跳转。
- **L1 创作路径引导**：Dashboard / Stories 新增 `CreationPathGuide`，消除三路径误判。
- **Wizard 重复建故事修复**：已有故事走 update 资产路径，ID 不变。
- **仪表盘统计卡可点击**：跳转 stories / characters / scenes。
- **测试基线**：`genesisSteps.ts` 18 单测；`model_gateway/executor`、`db/repositories`、`memory/ingest` 首批特征测试。
- ✅ **验证**：`cargo test --lib` 677 passed；`npx vitest run` 210 passed；`npx tsc --noEmit` / `architecture_guard.py` 通过。

### v0.26.24 — 修复续写重复、截断与跨内容复述（5 项根因）（2026-07-07）

对照 `creative_workflow.log` 2026-07-07 08:44–09:05 续写会话：

- **散布式句子块重复**：`trimInterspersedRepeatedBlocks`（Rust + TS golden 双跑）。
- **跨内容重叠复述**：`stripExistingOverlap`（尾部 3000 字比对，≥25 归一化字剥离）。
- **截断末句污染**：`trimDanglingTail`（极短末句裁剪）。
- **续写 8% 重试闸门**：TriShot anti-repeat 重试（对齐 Genesis）。
- **前端管线**：`sanitizeContinuationOutput` 全路径接入。
- ✅ **验证**：`cargo test --lib` 666 passed；`npx vitest run` 192 passed。

### v0.26.23 — 修复续写卡死与幽灵文本混乱（4 项根因）（2026-07-07）

对照 `creative_workflow.log` 2026-07-07 续写会话时间线，定位并修复 4 个根因：

- **Bug B（卡死主因）**：`auto_contract` 4 个 LLM 调用加入 `is_silent_background` 列表，后台补齐合同不再阻塞 `isAnyBackendActive`（原 6 分钟阻塞）。
- **Bug D（混乱主因）**：`handleSmartGeneration` 入口加重入守卫，存在未接受幽灵时先丢弃并提示。
- **Bug A**：`RichTextEditor` 新增 `bodyForceHideGhost` state 镜像 `force-hide-ghost` 类，消除 10s 渲染延迟。
- **Bug C**：续写 call3 超时上限从 120s 降至 60s，慢模型 fail-fast 回退到快模型。
- ✅ **验证**：`cargo test --lib` 655 passed；`npx vitest run` 183 passed；fmt/tsc 通过。

### v0.26.21 — 修复 Windows MSI 构建（迁移文件名重命名）（2026-07-07）

- 🎯 **背景**：v0.26.17 起将 `src/db/migrations/` 打包为 Tauri resource，但 24 个迁移文件名含中文/全角逗号/破折号且最长 102 字符，导致 WiX `light.exe` 从文件名生成 `File/@Id` 标识符时失败。v0.26.14/v0.26.16（resources 引入前）Windows MSI 曾成功，根因确凿。v0.26.20 尝试的 `wix.language: zh-CN` 无效（问题在标识符生成而非代码页）。
- 🎯 **修复**：将 24 个迁移文件重命名为 ASCII 短名（保留 `V###` 前缀与排序）。`schema_migrations` 按 version 跟踪，已应用迁移不受影响；`parse_filename` 仅解析 `V###` 前缀，无逻辑变更。
- ✅ **验证**：`cargo test --lib migrations` 8 passed；本地 `cargo tauri build`（macOS）通过；CI Windows MSI 待验证。

### v0.26.20 — 修复 v0.26.19 CI 格式检查失败与 Windows 打包（2026-07-06）

- 🎯 **修复**：v0.26.19 的 `ParallelWorldOutlineCharacterStep` doc 注释行超过 `max_width=100`，运行 `cargo +nightly fmt` 自动换行。仅注释格式变更。
- 🎯 **macOS 公证**：随 Apple Developer 协议续签已恢复成功。

### v0.26.19 — Genesis 创世流程全面审计与测试加固（2026-07-06）

- 🎯 **背景**：对照项目文档对「智能创作流程-创世」进行全面审计，分 Phase 1–4 执行修复、加固与测试补齐。
- 🎯 **Phase 1（P0 竞态与契约）**：
  - **Gap B**：`isFirstChapterReady` 路径在 `finalContent` 为空时不锁 `delivered`，避免编辑器永久空白。
  - **P0-2 角色世界观上下文**：`ParallelWorldOutlineCharacterStep` 中 character 提示词读取 `bundle.world_building` 恒为空（闭包捕获竞态），改为先 await world 拿真实 `world_concept` 再构造 character；提取 `world_concept_for_character_prompt` 纯函数 + 单测。
  - **P0-3 ChapterSwitch delivered 时序**：`selectChapter` 懒加载失败时不标记 `delivered`（`markDeliveredOnLoad` 仅在 `setContent` 成功后标记）。
- 🎯 **Phase 2（P1 架构对齐）**：
  - **后台错误可观测性**：`GenesisContext.errors` 共享错误集合 → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast 区分 warning/error。
  - **mutex 中毒锁加固**：`pipeline.rs` cancel flags 与 `model_gateway/executor.rs` registry 锁改用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒锁 + 单测。
  - **策略移入 quick phase**：经评估暂缓，记录为债务。
  - **文档/类型对齐**：`window/mod.rs` 与 `FrontstageEvent.ts` 注释重写，明确创世第一章 `ChapterSwitch` 不携正文。
- 🎯 **Phase 3（测试加固）**：
  - 8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 边界/契约测试。
  - 前端 Gap C 专用测试 + 状态机端点契约测试。
  - **跨层共享 trim golden fixture**：`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定跨层一致性。
- 🎯 **Phase 4（代码整洁）**：重命名 `*_future` → `*_gen`；去重 `AppConfig::load`；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent；评估不合并 `isGenesisSettingUpRef`。
- ✅ **验证**：`cargo test --lib` 655 passed（+10）；`npx vitest run` 183 passed（+17）；`npx tsc --noEmit` 零错误；fmt 通过。

### v0.26.18 — Genesis 第一章重复：竞态路径加固（2026-07-06）

- 🎯 **背景**：用户报告 v0.26.16 后新写小说第一章仍有内容重复。代码审查发现三个残留竞态缺口。
- 🎯 **修复**：
  - **Gap A**：ChapterSwitch `auto_accept=true` 但 content 为空时 `skipContent=true`（不从 DB 加载），不标记 `delivered`（让 smart_execute 投递）。
  - **Gap B**：`isFirstChapterReady` 路径仅在已 append 或编辑器已有内容时标记 `delivered`，避免空内容误锁。
  - **Gap C**：`selectChapter` 咽喉点新增 `delivered` + 编辑器已有内容守卫。
- ✅ **验证**：`npx vitest run` 167 passed（+1 Gap A 回归测试）；`npx tsc --noEmit` 零错误。

### v0.26.17 — Issue #4 启动加固：打包 SQL 迁移与 init_db 诊断增强（2026-07-06）

- 🎯 **背景**：v0.26.16 已修复 `init_db` 失败时的二级 panic（GatewayExecutor `state::<DbPool>()`），但 Windows 用户仍可能因 `init_db` 本身失败或 Release 缺 SQL 迁移而进入降级模式。
- 🎯 **修复**：
  - 打包 `src/db/migrations/` 到 `$RESOURCE/db/migrations/`。
  - `setup` 解析 bundled migrations 并传入 `init_db`。
  - `init_db` 启动前 `create_dir_all`；失败日志含 DB 路径与 migrations 目录。
  - 新增 `init_db_succeeds_on_fresh_directory` 回归测试。
- ✅ **验证**：`cargo test --lib init_db` 2 passed；`cargo check` 通过。

### v0.26.16 — 根治 Genesis 第一章重复、Issue #4 启动稳定性与代码格式修复（2026-07-06）

- 🎯 **症状**：v0.26.14 后 Genesis 第一章重复问题在部分模型/路径上仍偶发；部分 Windows 用户在应用数据目录不可写时遇到启动闪退/ panic。
- 🎯 **根因 1（重复）**：LLM 可能生成自身首尾重复的正文；前端 Genesis 自动接受流程使用布尔守卫，多处赋值导致状态机混乱，多路径并发下内容被叠加。
- 🎯 **根因 2（启动 panic）**：`init_db` 失败后 `setup` 仍构造 `GatewayExecutor`，其通过 `state::<DbPool>()` 读取未 manage 的 pool 导致启动 panic。
- 🎯 **修复**：
  - 生成侧验证闸门：自重复比例 ≥8% 时 anti-repeat 重试；prompt 新增「结构纪律」段。
  - 前端单写者状态机：`idle → generating → delivered`，阻塞外部投递与幽灵恢复。
  - `GatewayExecutor::new` 显式传 pool，`setup` 仅在 pool 可用时初始化网关。
  - 全局代码格式化，修复 CI `fmt`/`prettier` 检查失败。
- ✅ **验证**：`cargo test --lib` 637 passed / 0 failed / 2 ignored；`npx vitest run` 166 passed / 3 skipped；`npx tsc --noEmit` 零错误；`cargo +nightly fmt -- --check` 通过；`npm run format:check` 零差异；`python3 scripts/architecture_guard.py` 通过。

### v0.26.14 — 修复 Genesis 第一章模型输出自重复与降低幕前诊断日志压力（2026-07-05）

- 🎯 **症状**：v0.26.13 日志显示 `append_ai_done` 只触发一次、`append_text_check.occurrences=1`，但用户仍看到第一章「开头段落与结尾段落相同」的内容重复。
- 🎯 **根因**：前端没有追加两次；LLM 生成的正文自身存在首尾段落重复（模型级循环/自重复）。
- 🎯 **修复**：新增 `trimSelfRepetition` 工具，段落级检测「后半段 == 前半段」或「末段 == 首段」，字符级使用 KMP 最长 border 检测长尾重复；在 `appendAiContent` 入口及 `smart_execute.finalContent` 写入编辑器/幽灵文本前统一清理。同时降低 `RichTextEditor` 渲染诊断日志频率（前 20 帧 + 幽灵状态变化 + 200ms IPC 节流），缓解长时间写作后页面卡顿/崩溃。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 151 passed / 3 skipped；`npx playwright test` 36 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.13 — 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）（2026-07-05）

- 🎯 **症状**：v0.26.12 后日志显示 `append_ai_done` 只触发一次、`hasDuplicate: false`，但用户仍看到第一章「前一段分行、后一段挤成一大段」的虚假重复。
- 🎯 **根因**：数据层只写一次；`RichTextEditor` 的 `shouldShowGhostTree` 条件为 `!!(generatedText || isGenerating)`，当 `generatedText` 为空但 `isGenerating=true` 时会渲染空幽灵容器。该容器若残留旧内容或 React 复用 DOM 节点异常，就会导致「正文 + 幽灵文本」同框。
- 🎯 **修复**：`shouldShowGhostTree` 改为 `!!generatedText`；`FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)` 再清空 `generatedText` / 追加正文；增强 `frontstage:rich_editor_diag` 诊断字段；E2E 回归测试新增 `ghost-paragraph` 隐藏断言。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 148 passed / 3 skipped；`npx playwright test --project=chromium` 35 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.12 — 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值（2026-07-05）

- 🎯 **症状**：打开已有故事或新写小说后，幕前界面偶尔白屏崩溃，ErrorBoundary 显示 `Cannot read properties of null (reading 'length')`；订阅状态接口返回异常时日志出现 TypeError。
- 🎯 **根因**：`RichTextEditor`「角色名点击」effect 在初始化时直接访问 `characters.length`，而 `useCharacters` 的 `data` 可能为 `null`（React Query 默认值仅对 `undefined` 生效）；`useSubscription` 未对 `getSubscriptionStatus()` 返回 `null` 做空值防护。
- 🎯 **修复**：`RichTextEditor` 角色点击 effect 增加 `!characters || characters.length === 0` 守卫；`useSubscription` 使用 optional chaining 读取 `status?.tier`、`status?.status` 并回退默认值；新增 Playwright E2E 回归测试 `e2e/genesis-duplicate.spec.ts` 覆盖「已有故事 + 新写末世小说」完整流程。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 148 passed / 3 skipped；`npx playwright test --project=chromium` 35 passed / 5 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.11 — 修复 Genesis 第一章 store-editor 失步与崩溃隐患（2026-07-05）

- 🎯 **症状**：v0.26.10 后日志显示单次 `append_ai_done`，但用户仍看到第一章内容重复；写完后过一会儿页面可能崩溃。
- 🎯 **根因**：追加后 store 依赖 200ms onChange debounce 回写，当 `latestContentRef` 与编辑器 HTML 指纹相同时 `handleContentChange` 提前返回，store 长期为空，导致后续外部同步/章节切换出现视觉重复或状态漂移；`RichTextEditor.appendText` 空文档 setContent 未更新 `lastExternalContentRef`，content prop 到达后外部同步 effect 可能再次 setContent；开发模式下可能加载陈旧 `dist`。
- 🎯 **修复**：`appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`；`RichTextEditorRef` 新增 `getHTML()`；确认 `tauri.conf.json` `devUrl` 指向 dev server。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 147 passed / 3 skipped；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过。

### v0.26.9 — 根治 Genesis 第一章重复（DOM 竞态与追加去重）（2026-07-04）

- 🎯 **症状**：v0.26.8 后用户反馈第一章内容仍会重复显示，日志显示重复检测在 `appendAiContent` 与 `setGeneratedText` 路径上失效。
- 🎯 **根因**：所有重复检测与前缀去重都依赖 `editorRef.current.getText()`，而 TipTap DOM/Text 状态滞后于 React `content` prop。在 ChapterSwitch / pipeline-complete 刚加载正文后、`onChange` debounce 触发前，`getText()` 返回空/旧文本，导致已有正文被当成新内容追加或恢复为幽灵文本。
- 🎯 **修复**：`isTextAlreadyInEditor`、`handleRequestGeneration`、`handleSmartGeneration`、`appendAiContent` 统一改用 `latestContentRef.current` 作为内容基准；`appendAiContent` 追加后立即同步 `latestContentRef`；`RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 146 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.26.8 — 彻底修复 Genesis 第一章重复（竞态路径覆盖）（2026-07-04）

- 🎯 **症状**：v0.26.7 后，在 pipeline-complete 先加载 DB 正文、smart_execute 后返回 final_content 的竞态下，新小说第一章仍会重复显示。
- 🎯 **根因**：`genesisAutoAcceptedRef` 仅在 ChapterSwitch 自动加载正文时设置，无法覆盖 pipeline-complete 先完成的路径；后续 smart_execute 把 `final_content` 恢复为幽灵文本，与编辑器正文叠加。
- 🎯 **修复**：新增 `isTextDuplicate` 归一化去重工具；提取 `isTextAlreadyInEditor` helper；`pipeline-complete` 加载正文后标记 Genesis 已自动接受；`handleRequestGeneration` / `handleSmartGeneration` 设置 `generatedText` 前检测编辑器是否已包含该内容，已包含则跳过。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 138 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.26.7 — 修复 React #185 无限循环与 Genesis 第一章重复（2026-07-04）

- 🎯 **症状**：新写小说后过一会儿页面崩溃（React #185 Maximum update depth exceeded）；新小说第一章内容重复显示。
- 🎯 **根因**：`pipeline-complete` effect 依赖未 memo 的 `selectChapter`，每次渲染重复触发；Genesis 异步装配期间 `loadStories` 自动选择新 story 并把 DB 正文加载进编辑器，与 `generatedText` 幽灵文本叠加。
- 🎯 **修复**：关键回调全部 `useCallback`/ref 稳定化；`pipeline-complete` effect 增加单次处理守卫并改用 ref 读状态；新增 `isGenesisSettingUpRef` 禁止装配期间自动选择 story。
- ✅ **验证**：`cargo test --lib` 632 passed / 0 failed / 2 ignored；`npx vitest run` 138 passed / 3 skipped；`npx tsc --noEmit` 零错误。

### v0.23.49 — 推理模型思考链导致 JSON 提取出空对象修复（2026-06-26）

- 🎯 **症状**：用推理模型（如 MN-Oblivion-26B-UNCENSORED）创世时报 `missing field 'title' at line 1 column 2`，LLM 实际成功返回 5191 字符，失败在 JSON 提取阶段。
- 🎯 **根因**：推理模型在正文前输出 `önh...` / `<thinking>...</thinking>` 思考链，思考链里含花括号（如 "用 {} 格式表示"），`extract_first_json_object` 把第一个 `{}` 当成 JSON 对象提取出空对象，serde 找不到必填 `title`。
- 🎯 **修复**：新增 `strip_reasoning_blocks` 剥离配对思考链块；`extract_first_json_object` 跳过空对象 `{}` 继续向后扫描。
- ✅ **验证**：`cargo test --lib` 571 passed / 0 failed / 2 ignored

### v0.23.48 — JSON 提取用括号匹配修复 trailing characters 解析失败（2026-06-25）

- 🎯 **根因**：LLM 返回故事概念 JSON 后附带额外说明文本（含 `}`），`extract_and_sanitize_json` 用 `rfind('}')` 找 JSON 结尾会误提取过多内容 → serde_json "trailing characters" 错误。
- 🎯 **修复**：新增 `extract_first_json_object` 用括号匹配（跟踪 `{`/`}` 深度 + 跳过字符串字面量）精确提取第一个完整 JSON 对象。
- ✅ **验证**：`cargo test --lib` 568 passed / 0 failed / 2 ignored

### v0.23.47 — 调用模型前实时连接探测（5s），跳过失效死模型（2026-06-25）

- 🎯 **根因**：模型列表里可能存在已失效但健康状态仍为 Healthy 的死模型（本地 llama.cpp/MLX 服务已停止但缓存未更新），直接调用浪费 30-300s 直到 LLM 超时。
- 🎯 **修复**：`GatewayExecutor::generate` 候选循环中，每个候选模型在实际 LLM 调用前先执行 5s 超时实时探测；探测失败/超时则标记 `HealthStatus::Unhealthy`，跳到下一候选。

### v0.23.46 — AI 状态提示使用模型名称（2026-06-25）

- 🎯 `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`）。

### v0.23.45 — IngestPipeline LLM 调用静默化，根治正文后活动卡死与页面崩溃（2026-06-25）

- 🎯 **根因（日志确认）**：创世正文返回后，IngestPipeline 并发发起多个"记忆-内容分析"LLM 调用，`context_label` 未匹配 `is_silent_background` 静默列表，进度事件覆盖前端主活动状态（"准备上下文"卡住）。本地模型无法处理并发请求返回 `INTERNAL_ERROR`，大量错误事件涌入导致前端页面崩溃空白。
- 🎯 **修复**：将 IngestPipeline 的三个 `context_label` 加入 `is_silent_background` 静默列表。
- ✅ **验证**：`cargo check` 零错误

### v0.23.44 — AI 状态提示使用模型名称（2026-06-25）

- 🎯 `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称（格式：`准备上下文... · gemma4-e2b (OpenAI) (15s)`）。

### v0.23.43 — 前端诊断日志 + log_frontend_event 命令（2026-06-25）

- 🎯 新增 `log_frontend_event` Tauri 命令，前端关键路径可写入 `creative_workflow.log`。

### v0.23.42 — 根治创世卡在"最终输出"：BGP-4 自死锁修复（2026-06-25）

- 🎯 **根因（日志确认）**：`execute_trishot` 在 Call 3 成功返回后用 `spawn_blocking().await` 同步等待 BGP-4 `should_trigger` DB 查询，与 BGP-1/BGP-3 后台任务竞争 `std::sync::Mutex` 导致自死锁，`execute_trichot` 永不返回。
- 🎯 **修复**：BGP-4 改为 `tokio::spawn`（fire-and-forget）。
- ✅ **验证**：`cargo test --lib` **563 passed / 0 failed / 2 ignored**

### v0.23.40 — 参照现有诊断机制添加 WorkflowLogger 日志点（2026-06-25）

- 🎯 Bug A/B 诊断日志点接入 WorkflowLogger（`genesis.chapter_switch.sent`、`trishot.call3.done`、`trishot.bgp4` 等）。

### v0.23.37 — Genesis 活动清理（2026-06-25）

- 🎯 Genesis 成功路径补发 `smart-execute-progress` completed/error；`smart-execute-progress` 处理器把 timeout/error 映射为 failed。

### v0.23.36 — 创世正文清洗 + 后台作业不阻塞输入（2026-06-25）

- 🎯 **创世正文质量优化**：TriShot Call 3 的 `final_prompt` 追加 `NOVEL_OUTPUT_DISCIPLINE` 输出纪律段（禁元评论/markdown/小节标题/幕结束批注）+ 新增 `sanitize_novel_output` 后处理兜底（逐行去 markdown 符号→截断尾部元评论→剥离前导过渡语→去整行小节标题/批注）。7 个单元测试覆盖各场景。
- 🎯 **后台作业不阻塞输入**：Genesis 后台阶段 `pipeline-progress` 事件打 `metadata: {background: true}` 标记，前端 `useBackendActivityListener` 检测到后跳过注册 running activity，不禁用输入框。状态文案仍由 `novel-bootstrap-progress` 监听器独立更新。
- ✅ **验证**：`cargo test --lib` **563 passed / 0 failed / 2 ignored**（新增 7 个 sanitize 测试，零回归）；`npx tsc --noEmit` 零错误

### v0.23.35 — 采摘 Step1 JSON 解析容错（2026-06-23）

- 🩹 **Ingest Step1 `missing field entity_type`**：`memory/ingest.rs` 6 个反序列化结构体补 `#[serde(default)]`，LLM 返回 JSON 缺失字段时不再解析失败。

### v0.23.34 — 修复 select_candidates 中 std::sync::Mutex 自死锁（根因彻底查明）（2026-06-23）

- 🎯 **v0.23.31-33 全链路 15 个诊断标记精确定位**：自死锁发生在 `select_candidates` 内部
- 🔧 **自死锁根因**：第125行 `let health = health_registry.lock().ok()` 获取 MutexGuard，变量存活到函数末尾。第188行 `is_model_available` 再次 `lock()` 同一 `std::sync::Mutex`（不可重入）→ 线程永远等待自己释放 → 600s 超时
- 🔧 **修复**：`health` 锁移入嵌套块作用域，块结束时 MutexGuard 自动释放。后续 `is_model_available` 可安全重新锁定
- 🔧 **Call 1 为何不受影响**：Call 1 走 `select_fastest_profile`，不调 `select_candidates`
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.33 — 全链路 15 个精确定位诊断标记（2026-06-23）

- 🔍 从 `trishot.call3.start` 到 `llm.generate.start` 共 15 个 workflow_log 标记

### v0.23.30 — Genesis 全线阻塞点修复：genesis_default + select_candidates spawn_blocking + Chapter 保存 spawn_blocking（2026-06-23）

- 🏛️ **`GenerationMode::genesis_default()`**：显式化 Genesis 模式选择。Genesis 始终走 TriShot（需资产选择 + 快速出章），用户模式设置影响日常续写/改写
- 🔧 **`select_candidates` spawn_blocking**：`GatewayExecutor::generate` 中 `CapabilityStore::load_all()` 用 `spawn_blocking` 预加载，修复 Call 3 卡死
- 🔧 **Chapter 保存 spawn_blocking**：`FirstChapterGenerationStep` 中所有 `ChapterRepository` 操作移入 `spawn_blocking`
- 🔧 **Genesis 跳过 Call 2 精修器**：第 1 章 + 无已有内容时直接进 Call 3
- 🖥️ **前端显示 "[创世]"**：Genesis 期间底部栏显示创世状态而非"三击模式"
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.28 — select_candidates spawn_blocking + v0.23.29 — Chapter 保存 spawn_blocking（2026-06-23）

- 🔧 **Call 3 不再卡在 gateway 路由**：`select_candidates` 中 `CapabilityStore::load_all()` 用 `spawn_blocking` 包裹
- 🔧 **第一章内容写入不再卡在 DB**：ChapterRepository 操作移入 `spawn_blocking`
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.24 — setContent 内容比较 + v0.23.23 — RichTextEditor isExternalSyncRef（2026-06-23）

- 🔧 **从根源杜绝伪"保存中"**：`setContent` 内容未变化时不标记未保存
- 🔧 **从入口杜绝伪"保存中"**：编辑器外部 `setContent` 跳过 `onChange`
- ✅ **验证**：`npx tsc --noEmit` ✅ / `npx vitest run` ✅ 126 passed

### v0.23.22 — 诊断增强 + v0.23.25 — 信号竖条（2026-06-23）

- 🔍 `select_candidates` 慢查询标记（>100ms 输出工作流日志）
- 📊 模型状态指示器改为信号竖条组（3px 宽，4-16px 高，得分低→高排列）
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.19 — 根治 600s 超时：record_llm_call DB 写入不再阻塞 tokio worker（2026-06-22）

- 🎯 **根治概念 LLM 秒回但 pipeline 阻塞 600s**：v0.23.18 行级工作流日志定位卡点——概念生成 LLM 1.1s 完成，但 `record_llm_call` 同步 DB INSERT 卡住 600s 永不返回
- 🔧 **Fix 1 生产连接池加 `connection_timeout(5s)`**：`init_db` 的 `Pool::builder()` 补 `.connection_timeout(Duration::from_secs(5))`，防止 `pool.get()` 无限阻塞
- 🔧 **Fix 2 `record_llm_call` 改为 fire-and-forget `spawn_blocking`**：DB 写入提交到阻塞线程池立即返回，永不阻塞生成主流程
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.18 — 行级诊断：execute_generation Ok 分支 12+ 标记（2026-06-22）

- 🔍 **行级工作流日志**：`execute_generation` Ok 分支每步插入标记（`record_call.start` → `try_state` → `db_write` → `db_done` → `emit_completed.start` → `generate.return_ok`）
- 🧪 **5 个独立模块测试**：心跳 abort 不阻塞、阻塞 emit 由 5s 超时保护、TASK_START_TIMES Mutex 无死锁、pool.get 超时、record_llm_call 非阻塞
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.17 — 心跳阻塞 + 连接池超时双保险（2026-06-22）

- 🔧 `heartbeat_handle.await` 用 `tokio::time::timeout(5s)` 包裹；测试连接池补 `connection_timeout(10s)`
- 🔍 `record_llm_call` 内部添加 `try_state` / `db_write` / `db_done` 诊断标记
- ✅ **验证**：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### v0.23.16 — Genesis 快速阶段卡死修复 + E2E 集成测试（2026-06-22）

- 🔧 `story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化，防止 DB 锁/连接池满阻塞 tokio worker
- 🧪 新增 `scripts/test_trishot_e2e.py` 端到端集成测试：Gemma4-e2b 真实 LLM **73.2s 完成，1852 中文字**
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.15 — TriShot 管线 4 处缺陷修复（2026-06-22）

- 🔧 P0 预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色；P1 消息改名 `novel_bootstrap_first_chapter_ready`；P2 Call 1/2 预算守卫用 `total_start`、Call 3 超时 30-120s + 空内容检查
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.14 — 干净健康的模型池 + 统一身份 + 实时健康报告（2026-06-22）

- 🔧 模型池净化 L1-L4：启动归零清空 `llm_calls`、级联清理死模型、拒绝 disabled 设为活跃、健康报告数据源切换为实时探测快照
- 🔧 Genesis 两阶段：`quick_phase_steps()`（概念+第一章 TriShot）+ `background_steps()`（策略+世界观/大纲/角色）
- ✅ **验证**：`cargo test --lib` **551 passed / 0 failed / 2 ignored**

### v0.23.13 — 强制所有生成路径使用活跃模型（2026-06-22）

- 🎯 **彻底修复“当前模型是 A，实际调用 B”**：`LlmService::select_profile_for_request`、`GatewayExecutor::select_candidates`、`GatewayExecutor::select_fastest_profile` 全部优先返回/置顶用户当前设置的活跃模型
- 🧭 **Genesis 故事概念、TriShot Call 1、普通路由生成统一走活跃模型**：只要活跃模型健康（Healthy/Degraded），不再被 TTFB 阈值或三维打分绕开
- 🩹 **新增模型即时可用**：`create_model` 完成后立即刷新网关注册表并执行健康探测，探测通过即刻进入可用模型池
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`cargo +nightly fmt --check` 通过；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.12 — 彻底修复长超时：活跃模型优先 + 智能创作流程日志（2026-06-22）

- 🎯 **修复长超时根因**：模型网关现在强制把用户当前设置的活跃模型放到候选链首位，避免连接到历史/错误模型导致挂起
- 🧭 **`select_fastest_profile` 活跃模型兜底**：即使活跃模型没有算力档案，也优先使用它
- 📝 **新增 `WorkflowLogger`**：记录 TriShot Call 1/Call 3、LLM 调用起止、模型网关候选链与选择原因、错误等详细步骤
- 📋 **诊断卡片增强**：新增 `工作流日志路径` 与 `智能创作流程最近日志`，可直接查看后端执行轨迹
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.11 — 诊断提示词过滤探测/静默调用（2026-06-22）

- 🛡️ **过滤探测/静默调用**：`LlmService::execute_generation` 只在非静默调用时更新诊断提示词
- 🐛 **修复诊断提示词被 probe 覆盖**：避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖用户真正关心的生成提示词
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.10 — 模型网关优先使用当前活跃模型（2026-06-22）

- 🎯 **修复 AI 连到旧模型的问题**：`select_fastest_profile` 现在优先使用当前设置的活跃模型（只要健康且 TTFB 不比最快模型差太多）
- 🔗 **`select_candidates` 兜底活跃模型**：候选链中若不存在活跃模型，自动注入，保证用户设置的模型始终有机会被选中
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.9 — 运行时创作资产能力清单 + TriShot 路由增强（2026-06-22）

- 📚 **运行时创作资产能力清单**：应用启动后自动生成并刷新全部系统资产（methodology、genre_profile、style_dna、skill、beat_card、story_engine、pressure_relationship、workflow 等）的紧凑目录
- 🎯 **TriShot Call 1 可见全局资产**：`PromptSynthesizer` 的 prompt 中新增【系统可用创作资产目录】，让最快模型在选资产时知道可调用的系统级资产
- 🔀 **Call 3 资产透传**：TriShot Call 3 通过 `generate_for_task_with_tags` 把 Call 1 选中的资产 ID/标签透传给 `ModelGateway`
- 🧭 **ModelGateway 识别更多资产标签**：`methodology`、`beat_card`、`story_engine`、`pressure_relationship`、`style_dna`、`skill` 等标签会触发 `HeavyCreation`，优先使用创作能力强的模型
- 🐛 **修复 TriShot request_id 错误**：不再把 `gen_response.model` 当作 `request_id`
- 🛡️ **Call 1 预算守卫**：剩余时间不够完成 Call 1 + Call 3 时直接回退本地 `bundle_prompt`，避免前端长时间无响应
- ✅ **验证**：`cargo test --lib` **540 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.8 — AI 进度指示精细化 + 提示词诊断可靠性提升（2026-06-22）

- 🎯 **LLM 进度阶段具体化**：每个 LLM 调用都会显示连接模型 ID/提供商、组合提示词规模、等待模型回应、模型回应 token 数、解析结果，不再只显示“构思故事”
- 📊 **`LlmGeneratingProgress` 字段扩展**：新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- 🛡️ **提示词诊断兜底机制**：新增 `diagnostics::DiagnosticStore` Tauri State 与 `get_last_llm_prompt` 命令，解决大提示词事件可能丢失的问题
- 🩹 **修复诊断卡片“未捕获提示词”**：即使 `llm-prompt-sent` 事件未送达，诊断时也会主动通过命令读取完整提示词
- ✅ **验证**：`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.7 — 诊断信息增强 + 超时文案去硬编码（2026-06-22）

- 🩹 **修复诊断版本号硬编码**：`__STORYFORGE_VERSION__` 改为从 `package.json` 动态读取，不再显示 `0.16.0`
- 🩹 **修复超时文案硬编码**：`handleRequestGeneration` / `handleSmartGeneration` 现在从 `settings` 读取 `frontend_timeout_secs` / `smart_execute_total_timeout_secs`，错误提示与诊断卡片均显示实际配置值
- 📋 **诊断卡片新增 AI 生成模式**：显示 `settings.generation_mode`（`auto` / `time_sliced` / `fast` / `full` / `tri_shot`）
- 🤖 **诊断卡片新增当前模型信息**：模型 ID / 名称 / 提供商 / 端点
- 📝 **诊断卡片新增最后发给模型的提示词全文**：后端通过 `llm-prompt-sent` 事件广播，前端实时捕获并展示（上限 12000 字符）
- ✅ **验证**：`cargo check` 零错误；`npx tsc --noEmit` 零错误；`npm run format:check` 零差异

### v0.23.6 — 修复 macOS 启动崩溃：VectorStore State 初始化顺序（2026-06-22）

- 🐛 **修复启动 panic**：解决 `state() called before manage() for Arc<dyn VectorStore>` 导致的 macOS 启动崩溃
- 🔧 **根因**：`init_task_system_and_automation` 在 `app.manage(vector_store)` 之前通过 `app_handle.state()` 获取向量存储
- 🔧 **方案**：将 `LanceVectorStore` 创建与 `app.manage(vector_store)` 提前到依赖组件之前，异步 `init()` 保留原地
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`npm run format:check` 通过；`python3 scripts/architecture_guard.py` 通过

### v0.23.5 — CI 格式化修复（2026-06-21）

- 🎨 **Rust nightly fmt**：修复 import 顺序、函数参数折行、单行化等格式化差异
- 🎨 **前端 Prettier**：修复 `GeneralSettings.tsx` 类型断言单行化差异
- 📋 **无业务逻辑变更**：仅代码风格修复，使 GitHub Actions `rust-check` / `frontend-check` 通过

### v0.23.4 — 智能层闭环落地（2026-06-21）

- 🧠 **LLM JSON mode 原生支持**：新增 `llm::adapter::ResponseFormat::JsonObject`，OpenAI/Ollama 适配器分别附加 `response_format` / `format`，模型网关可透传
- ✍️ **Review/Refine Pipeline 结构化输出**：调用 JSON mode 并解析 `{ refined_content, change_summary, refinement_notes }`
- 💰 **MemoryPack 预算语义强类型化**：`MemoryBudget::for_task_type` 接收 `MemoryTaskType { Write, Plan, Review }`
- 📚 **拆书存储统一**：删除 `reference_characters` / `reference_scenes`，人物/场景数据全部汇入 `narrative_*` 表；迁移 `V100__拆书存储统一_删除_reference_表.sql`
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.3 — 测试基线修复 + 工程化（2026-06-21）

- 🐛 **MigrationRunner 交错执行**：`run_with_legacy` 按版本将 SQL 文件 migration 与 inline Rust migration 交错执行，避免高版本 SQL 文件跳过低版本 inline migrations
- 🗂️ **SING migration 版本上调**：`V095__意图图_SING_数据层.sql` → `V099__...`，确保其跑在所有 inline migrations 之后
- 🗂️ **`narrative_*` 表补 status 列**：`narrative_characters` / `narrative_scenes` / `narrative_world_buildings` 加入 `status TEXT NOT NULL DEFAULT 'active'`，新增 inline Migration 98 为已存在表补列
- 🔄 **ElementSource/ElementStatus round-trip 修复**：`domain/narrative_elements.rs` 新增 `as_str()` / `from_str()`（snake_case 英文）；`db/repositories_narrative.rs` 存储与解析统一使用英文键
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` **538 passed / 0 failed / 2 ignored**（新增 3 个测试，零回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.2 — 事件总线与状态同步治理（2026-06-21）

- 📡 **后端 `SyncEvent::ChapterCommitted`**：携带 `projection_status`，`SceneCommitService::apply_commit` 在 projections 完成后统一发射
- 🖥️ **前端 `content/isSaved` 迁移到 `frontstageStore`**：移除本地 `useState`，保留 `isSaved` + editor focus 双重保护
- 🧹 **清理遗留事件/hack**：删除所有 `backstage-data-refreshed` 废弃注释；`useWebViewRedrawFix` 改为 `FIXME` 标记
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 487 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`npx vitest run` 126 passed / 3 skipped

### v0.23.1 — 架构债务清偿：全局单例治理 + 模块依赖解耦（2026-06-21）

- 🗑️ **全局单例清零**：彻底移除 14 个全局 `static`/缓存，全部改为 Tauri State 注入或每次调用重新加载
- 🏗️ **domain 领域层扩展**：新增 `agent_context` / `agent_types` / `foreshadowing` / `search` / `write_time_bundle` / `asset_snapshot` / `continuity` / `adaptive` / `prompt_synthesis` / `agent_service` / `creative_engine` 等共享类型与端口
- 🔗 **模块循环依赖斩断**：`memory → agents`、`narrative → memory`、`narrative → creative_engine` 数据类型下沉到 `domain`；`agents ↔ creative_engine` 行为依赖通过 `CreativeEnginePort` / `AgentServicePort` 双向反转
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed / 48 failed（零新回归）；`npx tsc --noEmit` 零错误；`python3 scripts/architecture_guard.py` 通过

### v0.23.0 — TriShot 三击生成管线：关键路径压缩至最多 3 次 LLM（2026-06-21）

- 🎯 **TriShot 三击管线**：新增 `GenerationMode::TriShot`（三击），Call 1 最快模型选资产+合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成。质检/改写/入库/洞察全部下沉后台静默执行
- ⚡ **快速模型选取**：`GatewayExecutor::select_fastest_profile()` 按算力档案 TTFB 升序选最快可用模型，`LlmService::generate_with_fastest()` 捷径
- 🧩 **prompt_synthesis 模块**：`AssetManifest` 把 ~17 段资产打包为紧凑清单（4000 字符预算）+ `PromptSynthesizer` JSON 结构输出 + `PromptRefiner` 可选精修（预算守卫跳过）
- 🏎️ **PlanExecutor 快速路径**：TriShot 跳过 SING/PlanGenerator，`PlanStep::long_running` 跳过 90s 步超时
- 🤖 **BGP-2 智能改写**：`auto_rewrite_executor.rs` 按严重度分流——HIGH 自动改写+可撤销，LOW 仅建议
- 📡 **SyncEvent 扩展**：`ContentAutoRevised`（toast 通知）+ `RevisionSuggested`（审阅面板）
- 🖥️ **前端配置**：设置页面新增「三击模式」下拉选项
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed（新增 TriShot 19 测试全部通过，零回归）；`npx tsc --noEmit` 零错误

### v0.22.4 — 「异星球末世生存」智能创作流程优化 + 后台资产审计（2026-06-21）

- 🧩 **GenreResolver 题材解析**：精确/别名/子串/同义词/复合题材解析，解决自然语言题材词断链
- 🗺️ **意图图资产发现增强**：`AssetNode` tags + `discover_assets` 复合题材补充发现
- 🌉 **模型网关资产感知调度**：`asset_tags`/`discovered_asset_ids` 全链路透传，任务类别按标签校准
- ✍️ **TimeSliced 复合题材补强**：`secondary_genre_profile_strategy` 注入次要题材画像
- 📋 **后台资产全面审计**：新增 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md`，梳理全部 22 类创作资产、智能创作流程注入点、12 项断链/断环问题与 10 条修复建议
- 🗺️ **项目流程图技术文档**：新增 `docs/PROJECT_PROCESS_FLOWCHARTS_v0.22.4.md`，覆盖创世、拆书、智能创作主路径、79+ 提示词、43 个网文题材模板、40+ 创意资产、Story System 全子系统流程图
- 🏗️ **架构审计报告**：新增 `docs/BROOKS_LINT_ARCHITECTURE_AUDIT_v0.22.4.md`，模块依赖图 + 6 大 decay risks 诊断，Health Score 18/100
- ✅ **验证**：新增 targeted tests 39 passed；`cargo check` 零错误；`npx tsc` 零错误

### v0.22.3 — 钥匙串彻底移除 + 模型健康报告自动刷新（2026-06-21）

- 🔐 **钥匙串彻底移除**：删除 keyring crate、secure_storage 模块、store_api_keys_securely 配置项
- 🧹 **移除 ~260 行钥匙串读写逻辑**：load/save 中全部钥匙串访问已清除
- 📊 **模型健康报告自动刷新**：前端每 30 秒自动刷新，后端改为 async
- ⚡ **冗余 load 消除**：execute_writer 2→1 次、FirstChapterGenerationStep 3→1 次
- ✅ **零回归**：cargo check 零错误，425 passed，tsc 零错误

- GenreProfile 推荐资产种子：7 个题材写入推荐风格/方法论/技能
- 策略选择硬约束：体裁画像有推荐时跳过 LLM 直接使用
- 算力档案默认值修正：capability_score 未测试时默认 0.0

### v0.22.1 — 5 条建设性意见（2026-06-21）

- StrategySelector 题材推荐映射：7 种题材→风格推荐
- StyleDNA 句长偏差检测：>30% 偏差记录建议
- Inspector 方法论动态 prompt：5 种方法论全覆盖
- GenreProfile 推荐字段：4 新列 + Migration 96
- 算力档案质量分权重：HeavyCreation→quality80%

### v0.22.0 — 提示词与后台资产深度结合（2026-06-21）

- Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+题材画像+策略）
- Phase B：Inspector 全资产注入（体裁画像+角色状态+冲突+四元组）
- Phase C：意图感知调度接线（agent_type→intent 自动推导）
- Phase D：算力档案消费闭环（TTFB/TPS 参与候选排序）
- Phase E：资产→生成参数规则映射（asset_params.rs）

### v0.21.0 — 提示词全量可配置化（2026-06-21）

- 79 个提示词全部前端可编辑（21 个分类）
- 假接入修复：15 个 key 改为 resolve_prompt（含 DB 覆盖）
- 旁路接线：40+ 个硬编码提示词全部接入 registry
- 前端 Monaco 编辑器 + 批量导入导出

---

## 🔧 编译状态

| 检查项                                    | 状态                                                |
| ----------------------------------------- | --------------------------------------------------- |
| `cargo check`                             | ✅ 零错误                                           |
| `cargo test --lib`                        | ✅ 538 passed / 0 failed / 2 ignored                |
| `cargo test --lib intention_graph`        | ✅ 21/21                                            |
| `cargo test --lib adaptive::asset_params` | ✅ 3/3                                              |
| `cargo test --lib genre_resolver`         | ✅ 5/5                                              |
| `cargo test --lib selector`               | ✅ 6/6                                              |
| `cargo test --lib write_time_bundle`      | ✅ 13/13                                            |
| `cargo test --lib dispatcher`             | ✅ 5/5                                              |
| 真实模型测试（Gemma4-e2b）                | ✅ 6/6                                              |
| `npx tsc --noEmit`                        | ✅ 零错误                                           |
| `npx vitest run`                          | ✅ 126 passed / 3 skipped                           |
| `cargo +nightly fmt -- --check`           | ✅ 零差异                                           |
| `npm run format:check`                    | ✅ 零差异                                           |
| `python3 scripts/architecture_guard.py`   | ✅ 通过                                             |
| 后台资产审计                              | ✅ 完成，见 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md` |
| 已知测试失败                              | ✅ 无（V092 基线问题已在 v0.23.3 清零）             |

---

## 📊 提示词覆盖统计

| 类别                         | 数量   | 状态            |
| ---------------------------- | ------ | --------------- |
| Writer/Inspector/Commentator | 5      | ✅ 全部可覆盖   |
| Planner/Analyzer             | 4      | ✅ 全部可覆盖   |
| Pipeline（审稿/修稿/后处理） | 4      | ✅ v0.22.0 新增 |
| Audit（质量审计）            | 1      | ✅ v0.22.0 新增 |
| Intent（意图解析）           | 1      | ✅ v0.22.0 新增 |
| Deconstruction（拆书）       | 5      | ✅ v0.22.0 新增 |
| Creation（创世流程）         | 14     | ✅ v0.22.0 新增 |
| Strategy（策略选择）         | 1      | ✅ v0.22.0 新增 |
| Methodology（方法论）        | 19     | ✅ 全部可覆盖   |
| Skill（技能）                | 5      | ✅ 全部可覆盖   |
| Memory/Knowledge/Probe       | 7      | ✅ 全部可覆盖   |
| Narrative（叙事）            | 2      | ✅ 全部可覆盖   |
| World/Character（世界/角色） | 6      | ✅ 全部可覆盖   |
| System/Other                 | 5      | ✅ 全部可覆盖   |
| **总计**                     | **79** | ✅              |
