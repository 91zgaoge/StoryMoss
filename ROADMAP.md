# StoryForge (草苔) 开发路线图

> 最后更新: 2026-07-09（v0.26.38 提示词面板与组合智能化；v0.26.37 幕前保存/字数）

## ✅ v0.26.x 已实施完成

### 📝 v0.26.38 — 提示词面板与组合智能化 ✅ (2026-07-09)

- [x] 面板 Loading / 打开目录 / 导出修复
- [x] FrameworkSelections methodology + contextual_injectors 回灌 Call 3
- [x] 场景组合预览（preview_prompt_composition）
- [ ] 热路径按 quality_gate 条件审校（仍仅日志，不增 LLM）
- [ ] 前端孤儿 IPC：`auto_write_cancel` / `auto_revise_cancel` / `get_canonical_state`（已 allowlist）

### 📝 v0.26.37 — 幕前保存与字数 ✅ (2026-07-09)

- [x] 修复 `update_scene` IPC 参数形状（「保存中」常亮）
- [x] `appendAiContent` 后刷新字数并调度自动保存

### 📝 v0.26.36 — 后台配置即时生效 ✅ (2026-07-09)

- [x] `save_settings` → `reload_config` + `app_settings` sync
- [x] `llm_first_chunk_timeout_secs` 接入适配器
- [x] 字体/主题跨窗口 Tauri 事件同步
- [x] TriShot 预算 / writer prompt 读真实配置

### 📝 v0.26.35 — 幕后工作室审计残留 R1–R11 ✅ (2026-07-09)

- [x] R1 Dashboard `scene_count` 真实口径
- [x] R2 CreationPathGuide 快速创作 → `runCreationWorkflow`
- [x] R3 `apply_wizard_to_story` 去重 + KG
- [x] R4 幕后 `genesis-warnings`
- [x] R5/R6 场景序号语义标注
- [x] R7 世界构建文风 Tab
- [x] R8 UsageStats 启发式加强
- [x] R9 伏笔三列 Kanban
- [x] R10 角色→场景跳转
- [x] R11 拆书转故事后导航到场景

### 📝 v0.26.28 Phase 4 — 架构债务与工程体验 ✅ (2026-07-07)

- [x] **外部化 prompts**：`prompts/registry.rs` 中 95 个内置提示词迁移至 `resources/prompts/{category}/{id}.md`，运行时从 Tauri 资源目录加载，保留用户覆盖能力。
- [x] **迁移脚本拆分**：`db/connection.rs` 中 ~2,650 行 inline `run_migrations` 拆分为 `src/db/migrations/V028__*.rs` … `V099__*.rs` 共 70 个编号 Rust 迁移文件；`MigrationRunner` 新增 `RustMigration` trait 统一执行 SQL 与 Rust 迁移。
- [x] **知识图谱手动 CRUD UI**：Graph 页支持新建实体与添加关系。
- [x] **世界构建 AI 生成**：`WorldBuilding` 页新增「AI 生成」按钮，调用 `generateWorldBuildingOptions` 一键生成世界观。
- [x] **角色 AI 扩展**：`Characters` 页新增「AI 扩展」按钮，批量生成并创建角色。
- [x] **叙事分析图表**：`NarrativeAnalysis` 页新增 SVG 折线/面积图展示追读力趋势。
- [x] **策略选择移入 Quick Phase**：`genesis.rs` 中 `StrategySelectionStep` 前移至 `quick_phase_steps()`，`quick_phase_steps` 变为 3 步，`background_steps` 变为 5 步；同步更新步骤编号、进度百分比与测试契约。
- [x] **元文档同步**：`README.md`、`AGENTS.md`、`ARCHITECTURE.md`、`TESTING.md`、`CHANGELOG.md`、`PROJECT_STATUS.md` 版本与内容同步。

### 📝 v0.26.27 Phase 3 — L4 诊断互链、文档与依赖解耦 ✅ (2026-07-07)

- [x] **TracingPanel ↔ GenesisPanel 互链**：Genesis 运行记录可跳转对应生成链路；链路详情可跳转对应 Genesis 运行。
- [x] **Logs 深链**：失败 Genesis 运行一键跳转日志页并预填 `session_id`。
- [x] **UsageStats 按 operation 分组**：全部 / bootstrap / smart_execute / 其他 四标签页。
- [x] **Foreshadowing UX 改进**：`setup_scene_id` 改为场景下拉选择；Ledger 字段在可折叠高级区编辑。
- [x] **前端循环依赖解耦**：`components ↔ stores ↔ hooks ↔ frontstage` 分层清晰化，新增 `types/editor.ts`、`stores/contracts/*`；`appStore.ts` 不再从 `components/EditorSettings.tsx` import；`hooks/contracts/*` 仍待补齐。
- [x] **Tauri 循环依赖解耦**：`creative_engine ↔ llm` 已无互相 import；`model_gateway ↔ router` 仍存少量直接 import，后续继续向 `ports/` / `domain/` 迁移共享 trait。
- [x] **用户文档更新**：`docs/USER_GUIDE.md` 补全 L4 诊断页说明，修正过度承诺，与 v0.26.27 实现一致。
- [x] **元文档同步**：`AGENTS.md`、`ROADMAP.md`、`TESTING.md`、`ARCHITECTURE.md`、`README.md` 版本与内容同步。

### 📝 v0.26.26 Phase 2 — L2 资产补齐与领域层止血 ✅ (2026-07-07)

- [x] **角色页编辑 + 关系 CRUD**：`Characters.tsx` 支持编辑 Genesis 产出角色；新增关系增删改 UI。
- [x] **L2 创世溯源徽章**：Genesis 产出的资产（角色、场景、世界观等）显示「创世」来源标识，手动创建不显示。
- [x] **Story System 合同播种状态卡**：展示 MASTER_SETTING + CHAPTER_1 合同播种状态；失败运行显示警告摘要。
- [x] **Scenes 续写跳转幕前**：`ExecutionPanel` 主行动打开幕前写作界面。
- [x] **拆分 StorySystem.tsx**：标签页拆分为独立组件；原文件 < 200 行，只做 tab 路由。
- [x] **注入 repository traits 到 creative_engine**：`creative_engine/context_builder.rs` 通过 `db/traits.rs` 抽象依赖，领域代码不再直接 `use crate::db::repositories::*`。
- [x] **拆分 db/repositories.rs**：新建 `db/repositories/*.rs`，每个 repo 独立文件，原文件仅做 re-export。

### 📝 v0.26.25 Phase 1 — 可观测性与测试基线 ✅ (2026-07-07)

- [x] **重构 GenesisPanel 步骤模型**：步骤与后端 Quick（3 步）+ Background（5 步）对齐；展示 `steps_json.errors`；支持跳转到 story / 幕前。
- [x] **统一 L1 创作入口 UX**：`Dashboard.tsx`、`Stories.tsx` 与新增 `CreationPathGuide.tsx` 共同引导用户区分三条创作路径。
- [x] **修复 Stories Wizard 重复建故事**：已有故事走 update 路径，避免重复创建。
- [x] **仪表盘统计卡修正**：标签与口径一致；点击卡片可跳转对应页面。
- [x] **高频后端模块首批特征测试**：为 `model_gateway/executor.rs`、`db/repositories.rs`、`memory/ingest.rs` 各补 happy path + 错误路径测试。

### 📝 v0.26.19 Genesis 流程审计与 Phase 2 优化 ✅ (2026-07-06)

**Phase 1 — P0 关键正确性修复**
- [x] `handleSmartGeneration` Gap B 对齐：空 `finalContent` 不锁 `delivered`（与 `handleRequestGeneration` 一致）
- [x] 角色生成世界观上下文修复：`character_future` 不再读取空 `bundle.world_building`，改为 await `world_future` 后用真实 `world_concept` 构造提示词
- [x] `genesis_runs` 表接入：记录创世运行状态机（pending → quick_done → completed/failed）+ story_id + 错误累计
- [x] 新增 `GenesisRunRepository::set_story_id_and_status` / `update_steps_json`

**Phase 2 — P1 架构对齐**
- [x] 后台错误可观测性：`GenesisContext.errors` 共享 `Arc<Mutex<Vec<GenesisStepError>>>`，收集 world update / character relations / scene update / KG relations / contract seeding 的非致命错误，写入 `genesis_runs.steps_json`，发射 `genesis-warnings` 事件供前端 toast
- [x] mutex 中毒锁加固：`PIPELINE_CANCEL_FLAGS` 与 `GatewayExecutor::registry` 改用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒锁，新增 `lock_cancel_flags_recovers_from_poison` 测试
- [x] 文档/注释对齐：`genesis.rs` ChapterSwitch 注释、`window/mod.rs` `auto_accept` 文档、`USER_GUIDE.md` 创世章节更新为 auto-accept 真实路径
- [x] 策略移入 quick_phase 暂缓，记为本节已知债务

**Phase 3 — 测试加固**
- [x] Rust Genesis 契约测试：`compute_trim_ratio`/`should_retry_self_repetition`/`select_first_chapter_content`/`build_first_chapter_chapter_switch` 纯函数边界 + payload 契约；`background_steps` 6 步固定顺序
- [x] 前端 Gap B/C 专用测试 + 状态机端点契约（idle → delivered 可观测效果）
- [x] 跨层共享 trim golden fixture：`tests/fixtures/trim_golden.json`，Rust + TS 双跑锁定 `trim_self_repetition`/`trimSelfRepetition` 跨层一致性
- [x] 降低测试 brittleness：新 Gap C 测试用 `waitFor` 轮询替代固定 `setTimeout`

**Phase 4 — 代码整洁**
- [x] 重命名 `*_future` → `*_gen`（澄清顺序 await，非 tokio::join! 并行）+ 更新 `ParallelWorldOutlineCharacterStep` 注释（标注 world/outline 可并行化延迟债务）
- [x] 去重 `AppConfig::load`（`FirstChapterGenerationStep` 内连续两次合并为单次）
- [x] `appendAiContent` skip 路径不 `markAccepted`（移入实际追加成功的 else 分支）
- [x] `selectChapter` Gap C 重复入站也跳过 setContent（移除 `!isTextAlreadyInEditor` 条件）
- [x] 评估合并 `isGenesisSettingUpRef` → `genesisDeliveryRef`：不合并（覆盖窗口不同，前者覆盖续写 story_created bootstrap，后者仅创世 generating 态）

### 📝 v0.26.18 Genesis 第一章重复竞态加固 ✅ (2026-07-06)

- [x] Gap A：ChapterSwitch auto_accept=true 但 content 为空时 skipContent=true，不标记 delivered
- [x] Gap B：isFirstChapterReady 路径仅在已 append 或编辑器已有内容时标记 delivered
- [x] Gap C：selectChapter 咽喉点新增 delivered + 编辑器已有内容守卫
- [x] 新增 Gap A 回归测试

### 📝 v0.26.17 Issue #4 启动加固：打包 SQL 迁移 ✅ (2026-07-06)

- [x] `tauri.conf.json` 打包 `src/db/migrations/` 到 `$RESOURCE/db/migrations/`
- [x] `setup` 从 Resource 解析 bundled migrations 并传入 `init_db`
- [x] `init_db` 启动前 `create_dir_all`；失败日志含 DB 路径
- [x] 新增 `init_db_succeeds_on_fresh_directory` 回归测试

### 📝 v0.26.16 Genesis 第一章重复根治 + Issue #4 启动稳定性修复 ✅ (2026-07-06)

- [x] 生成侧验证闸门：`genesis.rs` 检测 LLM 自重复比例，≥8% 时 anti-repeat 重试
- [x] Prompt 模板新增「结构纪律」段，禁止首尾回环与整章重复
- [x] 前端单写者状态机：`idle → generating → delivered` 三态替换布尔守卫
- [x] `generating` 态阻塞 `onChapterUpdated` 与 `loadStories` 自动选择
- [x] `delivered` 态阻塞 `setGeneratedText` 幽灵文本恢复
- [x] `textCleanup` 提升为 `src-frontend/src/utils` 共享工具
- [x] Rust `trim_self_repetition` 对齐前端 KMP 最长 border 检测
- [x] Issue #4：`GatewayExecutor::new` 显式接收 `pool`，`setup` 仅在 pool 可用时初始化网关
- [x] 新增不可写应用目录回归测试
- [x] 修复 CI `cargo +nightly fmt -- --check` 与 `npm run format:check` 失败

## ✅ v0.23.x 已实施完成

### 📝 v0.23.74 场景优先架构迁移——Scene 成为唯一叙事真相源 ✅ (2026-06-28)

- [x] Phase 1: 消灭内容双写 — `scenes.content` 为唯一真相源，`chapters.content` 不再直接写入
- [x] Phase 2: 前端编辑器切到 Scene — store `sceneId` 主键，`update_scene` 自动保存
- [x] Phase 3: Commit 触发点迁移 — `SceneCommitDebouncer` 接替 `ChapterCommitDebouncer`
- [x] Phase 4: 创世提示词场景化 — `narrative_first_scene_generate`（14 场景变量），`SceneOutline` 扩展
- [x] 幕前纯正文 — 移除 `SceneDividerNode`，章内容无缝聚合
- [x] `SceneUpdated` 事件新增 `content_changed` 字段

### 📝 v0.23.66 模型角色分配 × 后台并发根治 ✅ (2026-06-28)

- [x] 模型角色分配：创作/工具/后台三层默认模型 + 网关按角色智能调度 + 前端「模型角色分配」卡片
- [x] 后台并发过载根治：`ParallelWorldOutlineCharacterStep` `tokio::join!` 3 路 → 串行 + `BACKGROUND_LLM_SEMAPHORE` 全覆盖

### 📝 v0.23.63 系统提示词可配置 + 第一章注册表化 + 框架级智能路由 ✅ (2026-06-27)

- [x] Gap 1: 第一章正文指令从硬编码 `format!()` 迁移到 PromptRegistry `narrative_first_chapter_generate`（15 个模板变量）
- [x] Gap 2: `LlmProfile.system_prompt_override` → `GenerateRequest.system_prompt` → OpenAI/Anthropic adapter 去硬编码英文
- [x] Gap 3: 新增 `FrameworkSelections` + `build_prompt_framework_catalog()`，Call 1 最快模型自主选择方法论/质量门/注入器
- [x] 前端 ModelModal 新增「系统提示词覆盖」多行文本框

### 📝 v0.23.60 网关探测异步化 + 调度退避 + 并发限流 ✅ (2026-06-27)

- [x] 后台 keepalive 每 10s 刷新缓存 → `is_health_fresh()` 跳过内联 5s 探测，0ms 延迟
- [x] 死模型指数退避 30→60→120→…→3600s
- [x] `BACKGROUND_LLM_SEMAPHORE(1)` 后台 LLM 串行化
- [x] `execute_trishot` → `orchestrator.generate` → genesis DB 保存全线 `log::warn!` 诊断

### 📝 v0.23.59 全面修复并强化模型网关调度 ✅ (2026-06-27)

- [x] `generate_for_request_with_context_and_pipeline` 路由到网关（单点覆盖概念生成 + 5 后台 pipeline）
- [x] `generate_with_fastest` 加 5s 探测 + 回退网关候选链
- [x] 活跃模型连续失败 ≥2 次降级，3 个强制置顶点跳过
- [x] TimeSliced 写作策略从 AppConfig 读取用户配置

### 📝 v0.23.49 推理模型思考链导致 JSON 提取出空对象修复 ✅ (2026-06-26)

- [x] 新增 `strip_reasoning_blocks` 剥离 `önh...` / `<thinking>...</thinking>` 思考链块
- [x] `extract_first_json_object` 跳过空对象 `{}` 继续向后扫描
- [x] 根因：推理模型思考链里的花括号会被 `find('{')` 误当成 JSON 对象，提取出空 `{}` → serde "missing field 'title'"

### 📝 v0.23.48 JSON 提取用括号匹配修复 trailing characters 解析失败 ✅ (2026-06-25)

- [x] 新增 `extract_first_json_object` 用括号匹配精确提取第一个完整 JSON 对象
- [x] 根因：`rfind('}')` 在 JSON 后附带含 `}` 文本时会误提取过多内容

### 📝 v0.23.47 调用模型前实时连接探测 + JSON 尾部多余文本容错 ✅ (2026-06-25)

- [x] 候选模型在实际 LLM 调用前先执行 5s 超时实时探测，探测失败标记 Unhealthy 跳到下一候选
- [x] 三处 WorkflowLogger 日志点：`pre_call_probe.ok` / `pre_call_probe.fail` / `pre_call_probe.timeout`

### 📝 v0.23.46 AI 状态提示使用模型名称 ✅ (2026-06-25)

- [x] `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称

### 📝 v0.23.45 IngestPipeline LLM 调用静默化，根治正文后活动卡死与页面崩溃 ✅ (2026-06-25)

- [x] 将 IngestPipeline 的三个 `context_label`（`"记忆-内容分析"`、`"记忆-生成知识"`、`"记忆-叙事事件提取"`）加入 `is_silent_background` 静默列表
- [x] 根因：创世正文返回后 IngestPipeline 并发发起多个 LLM 调用未静默，进度事件覆盖前端主活动导致卡死，本地模型并发崩溃导致页面空白

### 📝 v0.23.44 AI 状态提示使用模型名称 ✅ (2026-06-25)

- [x] `generation-status` 和 `llm-generating-progress` 心跳事件状态文案追加模型名称

### 📝 v0.23.43 前端诊断日志 + log_frontend_event 命令 ✅ (2026-06-25)

- [x] 新增 `log_frontend_event` Tauri 命令，前端可写入 WorkflowLogger

### 📝 v0.23.42 根治创世卡在"最终输出"：BGP-4 自死锁修复 ✅ (2026-06-25)

- [x] BGP-4 从 `spawn_blocking().await` 改为 `tokio::spawn`（fire-and-forget）
- [x] 根因：BGP-4 同步等待 DB 查询与 BGP-1/BGP-3 竞争 `std::sync::Mutex` 自死锁

### 📝 v0.23.40 参照现有诊断机制添加 WorkflowLogger 日志点 ✅ (2026-06-25)

- [x] Bug A 日志点：`genesis.first_chapter.generated`、`genesis.chapter_switch.sent`、`genesis.final_content`
- [x] Bug B 日志点：`smart_execute.start`、`trishot.call3.done`、`trishot.bgp4.start`/`bgp4.done`
- [x] 前端 `[DEBUG-dup]` / `[DEBUG-act]` console.warn 诊断日志

### 📝 v0.23.37 Genesis 活动清理 + 前端正文重复修复尝试 ✅ (2026-06-25)

- [x] Genesis 成功路径补发 `smart-execute-progress` completed/error 事件
- [x] `smart-execute-progress` 处理器把 timeout/error 映射为 failed

### 📝 v0.23.36 创世正文清洗 + 后台作业不阻塞输入 ✅ (2026-06-25)

- [x] TriShot Call 3 追加 `NOVEL_OUTPUT_DISCIPLINE` 输出纪律段（禁元评论/markdown/小节标题/幕结束批注）
- [x] 新增 `sanitize_novel_output` 后处理兜底（逐行去 markdown→截断尾部元评论→剥离前导过渡语→去整行小节标题/批注）
- [x] 7 个单元测试覆盖各场景（前导剥离/尾部截断/markdown清洗/幕结束/小节标题/纯净正文不误伤/空输入）
- [x] Genesis 后台阶段事件打 `metadata: {background: true}` 标记，前端跳过注册 running activity，输入框不再被禁用

### 🩹 v0.23.35 采摘 Step1 JSON 解析容错 ✅ (2026-06-23)

- [x] `memory/ingest.rs` 6 个反序列化结构体补 `#[serde(default)]`，修复 `missing field entity_type`

### 🏛️ v0.23.34 select_candidates Mutex 自死锁修复 ✅ (2026-06-23)

- [x] 全链路 15 个诊断标记精确定位自死锁位置
- [x] 根因：`health_registry.lock()` MutexGuard 不释放，`is_model_available` 再次 lock → std::sync::Mutex 不可重入 → 自死锁
- [x] 修复：health 锁移入嵌套块作用域，块结束时自动释放
- [x] Call 1 走 select_fastest_profile 不受影响，Call 3 走 select_candidates 此前必死锁
- [x] 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### 🚑 v0.23.19 根治 600s 超时：record_llm_call DB 写入不再阻塞 tokio worker ✅ (2026-06-22)

- [x] 生产连接池 `init_db` 补 `.connection_timeout(5s)`，防止 `pool.get()` 无限阻塞
- [x] `record_llm_call` 改为 fire-and-forget `spawn_blocking`，DB 写入提交到阻塞线程池立即返回
- [x] 工作流日志新增 `llm.record_call.spawn` phase 标记提交点
- [x] 验证：`cargo test --lib` **556 passed / 0 failed / 2 ignored**

### 🔬 v0.23.18 行级诊断：execute_generation Ok 分支 12+ 标记 ✅ (2026-06-22)

- [x] `execute_generation` Ok 分支每步前后插入工作流日志标记（`record_call.start` → `try_state` → `db_write` → `db_done` → `emit_completed.start` → `generate.return_ok`）
- [x] 新增 5 个独立模块测试（心跳 abort、阻塞 emit、Mutex 死锁、pool 超时、record 非阻塞）

### 🛡️ v0.23.17 心跳阻塞 + 连接池超时双保险 ✅ (2026-06-22)

- [x] `heartbeat_handle.await` 用 `tokio::time::timeout(5s)` 包裹
- [x] 测试连接池补 `.connection_timeout(10s)`
- [x] `record_llm_call` 内部添加诊断标记

### 🔧 v0.23.16 Genesis 快速阶段卡死修复 + E2E 集成测试 ✅ (2026-06-22)

- [x] `story_repo.create()` 改用 `tokio::task::spawn_blocking` 异步化
- [x] 新增 `scripts/test_trishot_e2e.py` E2E 集成测试（73.2s 完成，1852 中文字）

### 🔧 v0.23.15 TriShot 管线 4 处缺陷修复 ✅ (2026-06-22)

- [x] P0: 预检失败时调 `AutoContractBuilder::auto_fill` 补齐角色后重试
- [x] P1: `novel_bootstrap_background_started` → `novel_bootstrap_first_chapter_ready`
- [x] P2: Call 1/2 预算守卫用 `total_start` 计算已耗时间；Call 3 超时 30-120s + 空内容检查

### 🏗️ v0.23.14 干净健康的模型池 + 两阶段 Genesis ✅ (2026-06-22)

- [x] 启动归零清空 `llm_calls` + 过滤 `HealthRegistry` 残留；删除/更新模型级联清理
- [x] Genesis 拆分为 `quick_phase_steps()`（概念+第一章 TriShot）+ `background_steps()`（世界观/大纲/角色）

### 🔒 v0.23.13 强制所有生成路径使用活跃模型 ✅ (2026-06-22)

- [x] `LlmService::select_profile_for_request` 无条件优先返回 `active_llm_profile`
- [x] `GatewayExecutor::select_candidates` 将健康活跃模型强制置顶为 primary
- [x] `GatewayExecutor::select_fastest_profile` 健康活跃模型无条件优先，不再受 TTFB 阈值限制
- [x] Genesis 故事概念、TriShot Call 1、普通路由生成全部走用户当前设置的活跃模型
- [x] 新增模型保存后即时刷新注册表并执行健康探测

### 🎯 TriShot 三击生成管线 ✅ (v0.23.0)

- [x] GenerationMode::TriShot 三击模式（与 Fast/TimeSliced/Full 并存）
- [x] prompt_synthesis 模块（manifest + synthesizer + refiner）
- [x] GatewayExecutor::select_fastest_profile + generate_with_fastest
- [x] PlanExecutor TriShot 快速路径（跳过计划生成 LLM）
- [x] PlanStep::long_running 跳过 90s 步超时
- [x] execute_trishot 完整管线（Call 1 → Call 2 → Call 3 + 预算守卫）
- [x] BGP-2 auto_rewrite_executor（HIGH 自动改写 / LOW 建议）
- [x] SyncEvent::ContentAutoRevised / RevisionSuggested
- [x] 前端「三击模式」配置选项
- [x] BGP-3 后台 IngestPipeline（补 smart_execute 路径缺口）
- [x] BGP-1/BGP-4 后台审计+洞察链式 spawn
- [x] silent_background 白名单扩展（4 个新标签）

### 🧩 v0.23.4 智能层闭环落地 ✅ (2026-06-21)

- [x] LLM JSON mode 原生支持（`ResponseFormat::JsonObject`）
- [x] OpenAI/Ollama 适配器结构化输出接线
- [x] Review/Refine Pipeline 解析 `refinement_notes`
- [x] `MemoryBudget::for_task_type` 强类型化预算参数
- [x] 拆书存储统一：`reference_characters` / `reference_scenes` 删除，汇入 `narrative_*` 表
- [x] 迁移 `V100__拆书存储统一_删除_reference_表.sql`

### 🎨 v0.23.5 CI 格式化修复 ✅ (2026-06-21)

- [x] Rust nightly `cargo fmt` 格式化差异清零
- [x] 前端 Prettier 格式化差异清零
- [x] GitHub Actions `rust-check` / `frontend-check` 通过

### 🐛 v0.23.6 修复 macOS 启动崩溃 ✅ (2026-06-22)

- [x] 修复 `state() called before manage() for Arc<dyn VectorStore>` 启动 panic
- [x] `LanceVectorStore` 创建与 `app.manage` 提前到依赖组件之前
- [x] 全平台 CI 构建通过，生成 `.dmg` / `.deb` / `.msi`

### 📋 v0.23.7 诊断信息增强 ✅ (2026-06-22)

- [x] 修复诊断卡片版本号硬编码为 `0.16.0`
- [x] 修复前端/后端超时文案硬编码 `200s` / `180s`
- [x] 诊断信息新增 AI 生成模式、当前模型 ID/名称/提供商/端点
- [x] 诊断信息新增最后调用模型与最后发给 LLM 的提示词全文
- [x] 后端 `LlmService` 发射 `llm-prompt-sent` 事件供前端诊断捕获

### 🚀 v0.23.8 AI 进度指示精细化 ✅ (2026-06-22)

- [x] `LlmGeneratingProgress` 新增 `model_id`、`provider`、`prompt_chars`、`prompt_tokens`、`response_tokens`
- [x] 进度文案具体化：连接模型、组合提示词、等待回应、模型回应 token 数、解析结果
- [x] 新增 `diagnostics::DiagnosticStore` 与 `get_last_llm_prompt` 命令
- [x] 解决大提示词事件丢失导致诊断“未捕获”的问题

### 📚 v0.23.9 运行时创作资产能力清单 ✅ (2026-06-22)

- [x] 应用启动时自动生成并刷新全部系统创作资产目录
- [x] `AssetCapabilityManifest` 注入 Tauri State
- [x] TriShot Call 1 prompt 注入【系统可用创作资产目录】
- [x] TriShot Call 3 透传 `selected_asset_ids` / `asset_tags` 给 ModelGateway
- [x] ModelGateway dispatcher 识别 methodology/beat_card/story_engine/pressure_relationship/style_dna/skill 等标签
- [x] 修复 TriShot `request_id` 错误赋值、Call 1 无预算守卫

### 🎯 v0.23.10 模型网关优先使用当前活跃模型 ✅ (2026-06-22)

- [x] `select_fastest_profile` 优先使用当前 `active profile`（健康且 TTFB 不比最快模型差太多）
- [x] `select_candidates` 保证活跃模型始终出现在候选链中

### 🛡️ v0.23.11 诊断提示词过滤探测/静默调用 ✅ (2026-06-22)

- [x] 静默/探测调用不再更新 `DiagnosticStore` 和 `llm-prompt-sent` 事件
- [x] 避免 `model_gateway_probe` 的 `Respond with exactly the word OK.` 覆盖诊断提示词

### 🐛📝 v0.23.12 活跃模型优先 + 智能创作流程日志 ✅ (2026-06-22)

- [x] `GatewayExecutor::generate` 强制把当前活跃模型放到候选链首位
- [x] `select_fastest_profile` 无算力档案时也优先使用活跃模型
- [x] 新增 `WorkflowLogger`，记录 TriShot/LLM/ModelGateway 各阶段到 `logs/creative_workflow.log`
- [x] 诊断卡片显示工作流日志路径与最近日志

## ✅ v0.22.x 已实施完成

### 🧩 「异星球末世生存」复合题材创作流程优化 ✅ (v0.22.4)

- [x] GenreResolver 题材解析服务
- [x] GenreProfile 中文别名扩展
- [x] StrategySelector / build_selected_strategy / story_concept_prompt 接入 GenreResolver
- [x] AssetNode tags 与资产同步标签注入
- [x] IntentionGraphPlanner 复合题材资产补充发现
- [x] GatewayRequest asset_tags / discovered_asset_ids 透传
- [x] TaskClassifier / GatewayExecutor 资产标签感知调度
- [x] WriteTimeBundle secondary_genre_profile_strategy 复合题材续写补强

### 🔐 钥匙串彻底移除 + 模型健康报告自动刷新 ✅ (v0.22.3)

- [x] 移除 keyring crate（全平台依赖）
- [x] 移除 secure_storage 模块
- [x] API Key 改为直接存 SQLite
- [x] 模型健康报告每 30 秒自动刷新
- [x] AppConfig.load() 热路径冗余调用消除
- [x] Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+体裁画像+写作策略）
- [x] Phase B：Inspector 全资产注入（体裁画像+角色状态+活跃冲突+四元组+方法论）
- [x] Phase C：意图感知调度接线（agent_type→intent 自动推导，activate classify_by_intention）
- [x] Phase D：算力档案消费闭环（CapabilityProfile TTFB/TPS 参与候选排序）
- [x] Phase E：资产→生成参数规则映射（asset_params.rs）
- [x] Phase F：GenreProfile 推荐资产字段（Migration 96 + 4 新列 + 种子数据 7 题材）

### 提示词全量可配置化 ✅

- [x] 79 个提示词全部纳入 PromptRegistry（21 个分类）
- [x] 前端 Monaco 编辑器 + 批量导入/导出
- [x] 40+ 个原硬编码提示词全部接入 registry
- [x] 15 个假接入 key 修复为真实 DB 覆盖

### SING 意图图集成 ✅

- [x] Migration 95：6 张意图图表
- [x] 意图合成流水线（LLM 增强 + 规则回退）
- [x] PPR 分层发现
- [x] 动态 ReAct 执行
- [x] IntentionGraphPlanner × PlanExecutor 集成
- [x] 前端诊断面板（IntentionGraphDiagnostics）

### v0.20.x 基础设施 ✅

- [x] Phase 1-5: SING 数据层/离线合成/分层发现/PlanGenerator重构/动态ReAct
- [x] Phase 6: 模型网关意图感知集成
- [x] Phase 7: 前端意图图诊断面板
- [x] P0 断环修复: 资产同步/意图分类/执行图持久化/LLM合成/PPR传播
- [x] 真实模型测试（Gemma4-e2b, 6/6）
- [x] Multi-Agent Sessions（6种助手类型）

### Phase 4: AI 智能生成 ✅

**状态**: 完整实现

- [x] NovelCreationAgent
- [x] NovelCreationWizard 组件
- [x] 卡片式选择 UI
- [x] 首个场景自动生成

### Phase 5: 工作室配置系统 ✅

**状态**: 完整实现

- [x] StudioConfig 模型
- [x] StudioManager（导入/导出）
- [x] ZIP 格式支持
- [x] 默认主题配置

### Phase 6: 场景版本系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] SceneVersionRepository（版本CRUD）
- [x] SceneVersionService（比较、恢复、统计）
- [x] VersionTimeline 组件（垂直时间线）
- [x] DiffViewer 组件（差异对比）
- [x] ConfidenceIndicator 组件（置信度可视化）
- [x] 版本链管理（supersession）

### Phase 7: 混合搜索系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] BM25 Search（CJK二元组分词）
- [x] Hybrid Search（RRF融合排序）
- [x] Entity Hybrid Search（名称+向量）
- [x] 可配置权重和参数

### Phase 8: 记忆保留系统 ✅ (v3.1.0)

**状态**: 完整实现

- [x] RetentionManager（遗忘曲线计算）
- [x] 五级优先级分类
- [x] 遗忘时间预测
- [x] 保留报告生成
- [x] 上下文窗口优化

### Phase 9: 幕前界面重构与本地模型 ✅ (v3.1.1)

**状态**: 完整实现

- [x] 精简侧边栏（仅保留"幕后"按钮）
- [x] OKLCH 颜色系统重构（去除 AI 感模板色）
- [x] LXGW WenKai 字体替换（去除 Crimson/Inter）
- [x] Blockquote 与微交互重设计（Waza 原则）
- [x] 顶部动态状态栏
- [x] 底部 LLM 对话栏（悬停显示、模型状态灯、去除模式切换图标）
- [x] 流式对话交互（Enter 发送 / Shift+Enter 换行）
- [x] 本地三模型配置（Gemma / Qwen3.5 / bge-m3）
- [x] Tauri Windows 构建与打包（MSI + NSIS）
- [x] GitHub Actions CI 图标修复（macOS / Ubuntu）

---

### Phase 10: 设计-实现对齐修复 ✅ (v5.6.0)

**状态**: 全部完成

- [x] Scene 删除外键清理（chapters.scene_id → NULL）
- [x] Wizard 同步事件（story_created + data_refresh）
- [x] Character relationships 真实查询（character_relationships 表 JOIN）
- [x] Collab 文档 OT 重建（operations apply 重建内容）
- [x] Workflow EdgeCondition 条件求值（8 种运算符）
- [x] Task 心跳超时指数退避重试
- [x] Outline/Foreshadowing/Payoff 修改后同步事件
- [x] Cache 对称失效（sceneUpdated↔chapters、chapterDeleted↔scenes）
- [x] Workflow 节点 300s 超时
- [x] INGEST_COOLDOWN 24h 过期清理
- [x] FrontstageApp 真实 feedback（移除 mock learnings）
- [x] WritingStyle 更新同步事件
- [x] Workflow 并发守卫与重试幂等性
- [x] Pending vector SQLite 持久化
- [x] Task 执行 300s 超时

### Phase 11: 提示词全面可配置化 ✅ (v0.19.0)

**状态**: 全部完成

- [x] 35+ 内置提示词注册表（`prompts/registry.rs`）
- [x] 15 个 `PromptCategory` 分类体系
- [x] 雪花法 10 步提示词注入注册表
- [x] 5 个内置技能提示词映射（`skill_id_to_prompt_id`）
- [x] Memory / Knowledge / MultiAgent 模块接入注册表
- [x] 前端 PromptsPanel 重写（分类 + 搜索 + 批量重置 + 默认值预览）
- [x] GeneralSettings 精简为「提示词注册表」链接卡片
- [x] `reset_all_prompt_overrides` 批量重置 IPC
- [x] 运行时覆盖生效（`resolve_prompt()` 优先查 DB）

---

## 📊 v0.19.0 项目状态

| 模块             | 完成度   | 说明                                                                                                    |
| ---------------- | -------- | ------------------------------------------------------------------------------------------------------- |
| 场景化叙事系统   | 100%     | Scene 模型、StoryTimeline、SceneEditor                                                                  |
| 增强记忆系统     | 100%     | Ingest/Query Pipeline、Knowledge Graph、LanceDB 语义搜索、Pending Vector SQLite 持久化                  |
| AI 智能生成      | 100%     | NovelCreationAgent、Bootstrap 两阶段、创建向导、真实自适应学习反馈                                      |
| 工作室配置       | 100%     | 导入/导出、主题系统                                                                                     |
| 混合搜索         | 100%     | BM25 + Vector RRF融合 + 语义嵌入                                                                        |
| 场景版本         | 100%     | 版本历史、对比、恢复                                                                                    |
| 记忆保留         | 100%     | 遗忘曲线、优先级管理                                                                                    |
| 幕前界面         | 100%     | 精简侧边栏、幽灵文本、`/` 菜单                                                                          |
| 幕前幕后自动关联 | 100%     | Chapter↔Scene 双向映射、state_sync、实时同步、Cache 对称失效完整、writingStyle/storySelected 缓存精确化 |
| 后台自动化       | 100%     | Workflow 持久化、能力进化反馈环、向量索引闭环（Chapter + Scene）、Workflow 幂等性                       |
| 本地模型配置     | 100%     | 三模型集成                                                                                              |
| 提示词可配置化   | 100%     | 35+ 提示词注册表、15 分类、前端完整管理面板、运行时覆盖生效                                             |
| Tauri 构建       | 100%     | MSI + NSIS 安装包                                                                                       |
| 设计-实现对齐    | 100%     | v5.6.4 Tauri IPC rename_all 修复                                                                        |
| **整体 v0.19.0** | **100%** | 核心功能全部完成                                                                                        |

---

## 🚀 编译状态

```bash
$ cd src-frontend && npm run build
    vite v6.4.2 building for production...
    ✓ 2156 modules transformed.
    dist/                     655.75 kB │ gzip: 216.60 kB
```

```bash
$ cd src-tauri && cargo tauri build
    Finished release profile [optimized] target(s) in 8m 04s
       Built application at: target/release/storyforge
    Finished 3 bundles at:
        target/release/bundle/dmg/StoryForge_0.23.6_aarch64.dmg
        target/release/bundle/deb/storyforge_0.23.6_amd64.deb
        target/release/bundle/msi/StoryForge_0.23.6_x64_en-US.msi
```

```bash
$ cd src-tauri && cargo test --lib
    running 538 tests
    test result: ok. 538 passed; 0 failed; 2 ignored
```

✅ **编译成功** | ✅ **测试全绿** | ✅ **打包成功**

---

## 🆕 v3.1.1 新增依赖

| 依赖                          | 版本    | 用途             |
| ----------------------------- | ------- | ---------------- |
| @tiptap/react                 | ^3.22.3 | 幕前富文本编辑器 |
| @tiptap/starter-kit           | ^3.22.3 | TipTap 基础扩展  |
| @tiptap/extension-placeholder | ^3.22.3 | 占位符扩展       |

---

## 📋 后续路线图

### v3.2.x 进行中

- [x] LLM 真实 SSE 流式输出
- [x] Anthropic 适配器
- [x] Ollama 适配器
- [x] 实体嵌入持久化修复

#### 向量存储增强

- [x] SQLite 向量存储持久化（已替代 JSON-memory fallback）
- [ ] LanceDB 持久化存储（ blocked：Arrow 依赖与当前工具链冲突）
- [x] 实体向量持久化（`kg_entities.embedding` BLOB 读写修复）
- [x] 实体向量自动更新（属性变更时重新生成嵌入）
- [x] 语义搜索优化
- [ ] 向量索引性能优化

#### 知识图谱可视化

- [x] 实体关系图谱可视化
- [x] 交互式图谱浏览（双击聚焦、搜索筛选、类型过滤）
- [x] 实体详情弹窗
- [x] 关系强度可视化

#### 记忆系统增强

- [x] 自动归档系统（一键归档 + 恢复 + 已归档浏览）
- [x] 创建向导自动 Ingest
- [x] 实体嵌入持久化
- [x] 知识蒸馏
- [x] 记忆压缩

#### 协作功能

- [x] 评论和批注系统
- [x] 修订模式
- [x] 变更追踪

### v3.3.0 (中期计划)

#### 云端同步

- [ ] 用户账户系统
- [ ] 云存储集成
- [ ] 多设备同步

#### 协作写作增强

- [ ] 实时协作场景编辑
- [ ] 评论和批注系统
- [ ] 修订模式

#### 插件市场

- [ ] Skills 分享平台
- [ ] 主题市场
- [ ] Agent 模板市场

#### 导出增强

- [ ] 自定义导出模板
- [ ] 批量导出
- [ ] 自动发布集成

### v4.0.0 (长期计划)

#### 技术架构升级

- [ ] WebAssembly 前端 (Leptos)
- [ ] 自研小模型部署
- [ ] 边缘计算支持

#### 多人实时协作

- [ ] OT 算法完整实现
- [ ] 实时光标同步
- [ ] 冲突解决机制

#### 移动端支持

- [ ] iOS 应用
- [ ] Android 应用
- [ ] 响应式 Web 版本

#### 发布平台集成

- [ ] 起点中文网集成
- [ ] 晋江文学城集成
- [ ] 自出版平台 (Amazon KDP)

---

## 📈 历史版本

### v0.23.13 (2026-06-22)

- [x] 强制 Genesis / TriShot / 普通路由生成统一使用用户设置的活跃模型
- [x] `select_profile_for_request`、`select_candidates`、`select_fastest_profile` 全部优先活跃模型
- [x] 新增模型保存后即时健康探测并刷新网关注册表

### v0.23.12 (2026-06-22)

- [x] 活跃模型强制优先，修复连接错误模型导致的长超时
- [x] 新增 WorkflowLogger 记录 TriShot/LLM/ModelGateway 详细执行步骤

### v0.23.11 (2026-06-22)

- [x] 诊断提示词过滤探测/静默调用，避免被 probe prompt 覆盖

### v0.23.10 (2026-06-22)

- [x] `select_fastest_profile` 优先使用当前活跃模型，避免连到旧模型
- [x] `select_candidates` 候选链兜底活跃模型

### v0.23.9 (2026-06-22)

- [x] 运行时创作资产能力清单：启动时刷新全部系统资产并注入 TriShot/ModelGateway
- [x] TriShot Call 1 可见全局资产，Call 3 透传选中资产给模型网关
- [x] 修复 TriShot request_id 错误与 Call 1 预算守卫

### v0.23.8 (2026-06-22)

- [x] AI 进度指示精细化：连接模型、组合提示词、等待回应、模型回应、解析结果
- [x] 新增 `DiagnosticStore` 与 `get_last_llm_prompt` 命令，提升提示词诊断可靠性

### v0.23.7 (2026-06-22)

- [x] 诊断卡片版本号改为从 `package.json` 动态读取
- [x] 超时文案去硬编码，读取用户实际设置
- [x] 诊断信息新增 AI 生成模式、当前模型、最后 LLM 提示词

### v0.23.6 (2026-06-22)

- [x] 修复 macOS 启动崩溃（VectorStore State 初始化顺序）
- [x] 全平台 CI 构建通过（`.dmg` / `.deb` / `.msi`）

### v0.23.5 (2026-06-21)

- [x] CI 格式化修复（Rust nightly fmt + 前端 Prettier）
- [x] `rust-check` / `frontend-check` 通过

### v0.23.4 (2026-06-21)

- [x] LLM JSON mode 原生支持（OpenAI/Ollama）
- [x] Review/Refine Pipeline 结构化输出
- [x] MemoryPack 预算参数强类型化
- [x] 拆书存储统一，删除 `reference_characters` / `reference_scenes`

### v0.23.3 (2026-06-21)

- [x] MigrationRunner 交错执行修复
- [x] V092 测试基线 48 个失败清零
- [x] `narrative_*` 表 `status` 列补齐

### v0.23.2 (2026-06-21)

- [x] `SyncEvent::ChapterCommitted`
- [x] 前端编辑器状态收敛到 `frontstageStore`

### v0.23.1 (2026-06-21)

- [x] 全局单例清零（14 个）
- [x] 模块循环依赖斩断

### v0.23.0 (2026-06-21)

- [x] TriShot 三击生成管线
- [x] prompt_synthesis 模块
- [x] BGP-2 智能改写
- [x] 前端「三击模式」配置

### v3.1.1 (2026-04-13)

- [x] 幕前界面重构（Waza 设计原则）
- [x] OKLCH 颜色系统 / LXGW WenKai 字体
- [x] 本地三模型配置
- [x] Tauri Windows 构建打包
- [x] GitHub Actions CI 跨平台修复

### v3.1.0 (2025-04-13)

- [x] 混合搜索
- [x] 场景版本管理
- [x] 记忆保留曲线

### v3.0.0 (2025-04-12)

- [x] 场景化叙事架构
- [x] 增强记忆系统
- [x] AI 智能生成
- [x] 工作室配置

### v2.0.x (已完成)

- [x] 双界面架构 (幕前/幕后)
- [x] 技能系统
- [x] MCP 支持
- [x] 状态管理
- [x] 模型路由
- [x] 进化算法
- [x] 导出功能 (PDF/EPUB)

### v1.x (已完成)

- [x] 基础架构
- [x] LLM 集成
- [x] 数据库设计
- [x] 前端界面

---

## 🎯 优先级说明

| 优先级 | 说明               |
| ------ | ------------------ |
| P0     | 核心功能，必须完成 |
| P1     | 重要功能，影响体验 |
| P2     | 增强功能，锦上添花 |
| P3     | 未来规划，长期目标 |

---

## 📚 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计
- [CHANGELOG](CHANGELOG.md) - 版本变更记录
- [PROJECT_STATUS](PROJECT_STATUS.md) - 详细项目状态
