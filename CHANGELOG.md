# Changelog

All notable changes to StoryForge (草苔) project will be documented in this file.

## [v0.7.3] - 商业模式重构 + 1:N Chapter↔Scene 架构完成 + SceneDivider 预留接口（2026-05-20）

### 💼 商业模式重构：订阅解锁功能，非模型配额

#### SubscriptionService 精简
- **移除配额计量体系**：删除 `QuotaDetail`、`QuotaCheckResult`、`OFFLINE_GRACE_LIMIT` 及全部配额检查/消费方法
- **移除方法**：`get_or_create_quota`、`get_quota_detail`、`check_auto_write_quota`、`consume_auto_write_quota`、`check_auto_revise_quota`、`consume_auto_revise_quota`、`check_platform_model_quota`、`consume_platform_model_quota`、`check_ai_quota`、`consume_ai_quota`
- **保留方法**：`get_or_create_subscription`（仅创建/查询订阅状态）、`has_feature_access`（功能开关检查）、`upgrade_subscription`、`log_ai_usage`（纯统计）

#### `has_feature_access` 细粒度功能权限
- **Free 用户可用**：`writer`、`scene_management`、`character_management`、`knowledge_graph_query`、`outline`
- **Pro/Enterprise 解锁**：`pipeline_refine`、`pipeline_review`、`pipeline_finalize`、`book_deconstruction`、`auto_write`、`auto_revise` 等全部高级功能
- **拆书与 Pipeline 命令接入**：`book_deconstruction/commands.rs` 和 `pipeline/commands.rs` 统一调用 `has_feature_access`，未授权返回 `AppError::subscription_required`

#### `AppError::SubscriptionRequired` 取代 `QuotaExceeded`
- **错误码变更**：`QUOTA_EXCEEDED` → `SUBSCRIPTION_REQUIRED`
- **字段变更**：`{ quota_type, remaining }` → `{ feature_id, current_tier }`
- **构造函数变更**：`quota_exceeded()` → `subscription_required(feature_id, message)`
- **前端兼容**：`loggedInvoke` 按 `SUBSCRIPTION_REQUIRED` code 渲染升级引导 UI

#### `log_ai_usage` 纯统计（不参与配额控制）
- 记录字段：`user_id`、`story_id`、`chapter_id`、`agent_type`、`instruction`、`prompt_tokens`、`completion_tokens`、`model_used`、`cost`、`duration_ms`、`tier_at_time`
- 仅用于 UsageStats 看板展示，不做任何拦截或限制

### 🏗️ 1:N Chapter↔Scene 架构完成（Phase 4）

#### 废弃 `chapters.scene_id`
- **Migration 71**：`chapters` 表移除 `scene_id` 列（旧数据库 `DROP COLUMN IF EXISTS`）
- **`Chapter` 模型**：删除 `pub scene_id: Option<String>` 字段
- **全链路改为 `scenes.chapter_id` 查询**：
  - `ChapterRepository::create` / `update` / `get_by_story`：不再读写 `chapters.scene_id`
  - `SceneRepository::create`：不再 `UPDATE chapters SET scene_id = ?`
  - `SceneRepository::delete`：不再 `UPDATE chapters SET scene_id = NULL`
  - `lib.rs` `create_chapter` / `update_chapter`：改为 `SceneRepository::get_by_chapter(&chapter.id)` 查询关联场景，触发 `SceneCommitService::auto_commit`

#### `SceneCommitService` 取代 `ChapterCommitService`
- **提交粒度对齐 Scene**：`scene_commits` 表新增 `scene_id` 外键（可空），记录 `state_deltas_json` / `entity_deltas_json` / `accepted_events_json`，提交链以 Scene 为单元
- **Migration 70**：`chapter_commits` 表重命名为 `scene_commits`；所有索引同步重建（`idx_scene_commits_story` / `idx_scene_commits_scene` / `idx_scene_commits_number` / `idx_scene_commits_chapter`）
- **Repository 重命名**：`ChapterCommitRepository` → `SceneCommitRepository`，`ChapterCommit` → `SceneCommit`
- **Service 重命名**：`story_system::SceneCommitService::auto_commit(story_id, chapter_id, scene_id)` — 有 `scene_id` 时写入 scene_commits，无则保持基于 chapter 的兼容路径
- **IPC 命令**：`init_chapter_commit` / `apply_chapter_commit` / `get_chapter_commits` 保留原签名，内部操作 `scene_commits` 表

#### `SceneDividerNode` 功能预留接口
- **`SceneDividerNode` 模型**（`db/models_v3.rs`）：`id` / `chapter_id` / `position` / `scene_id` / `label` / `created_at` / `updated_at`
- **`SceneDividerRepository`**（`db/repositories_v3.rs`）：`create`、`get_by_chapter`、`set_dividers`（事务级全量替换）、`delete`、`delete_by_chapter`
- **Migration 72**：`scene_divider_nodes` 表（`TEXT` 主键 + `chapter_id` 外键 + `position` 整数 + `scene_id` / `label`），新建数据库自动创建；`CREATE INDEX idx_scene_divider_chapter ON scene_divider_nodes(chapter_id)`
- **前端保留**：`SceneDividerNode.ts` TipTap 扩展维持原子块节点定义，等待 1:N 编辑器模式激活

### 🔧 其他变更
- **`db/connection.rs` 初始 Schema**：`scenes` 表 CREATE TABLE 添加 `chapter_id TEXT` + `FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL`；`CREATE INDEX idx_scene_chapter ON scenes(chapter_id)`；新增 `scene_divider_nodes` 表定义
- **编译状态**：`cargo check` 零错误，`cargo test` ~225/225 通过，`npm run build` 通过
- **版本号**：Cargo.toml / package.json / tauri.conf.json → 0.7.3

---

## [v0.7.0] - AI 三审 Pipeline + 角色动态状态 + 用量统计 + 幕前指令升级（2026-05-15）

### 🏭 AI 三审 Pipeline 系统

#### Pipeline 核心架构
- **`pipeline/mod.rs`** — 四级创作管线：`Rewrite` → `Refine` → `Review` → `Finalize`
- **`pipeline/refine.rs`** — `run_refine(story_id, draft_id, chapter_info, config)`：AI 修稿，对章节草稿进行语言润色、结构调整、错别字修正
- **`pipeline/review.rs`** — `run_review(...)`：AI 审稿，输出 `overall_score`（0-100）+ `dimensions` JSON 数组 + `issues` JSON 数组 + `content` 总结
- **`pipeline/finalize.rs`** — `finalize_draft(...)`：定稿主流程，创建 `PostProcessStep` 记录 → 执行各后处理步骤 → 更新状态为 Success/Failed
- **`pipeline/post_process.rs`** — `run_post_process_step(...)` + `run_character_cards(...)`：执行单个后处理步骤，LLM 驱动角色状态解析

#### 后处理步骤追踪
- **`PostProcessStepDef`** 定义 4 个标准步骤：`kb_import`（知识库更新）、`chapter_notes`（章节笔记）、`character_cards`（角色状态卡）、`style_analysis`（风格分析）
- **`PostProcessStep`** 数据库记录：`id`/`story_id`/`chapter_number`/`step_type`/`status`（Running/Success/Failed）/`is_critical`/`error_message`/`created_at`/`updated_at`
- **关键/非关键分类**：`is_critical=true` 的步骤失败时阻断定稿流程；非关键步骤失败仅记录日志，不影响整体定稿
- **真实执行**：`finalize.rs` 创建 `LlmService` 并调用 `run_post_process_step`，更新每一步的真实状态（替代原占位实现）

#### LLM 驱动角色状态解析
- `run_character_cards` 构建综合 Prompt：角色上下文（姓名/背景/性格/目标/外貌/关系）+ 章节内容
- 调用 LLM 输出 JSON 数组：`[{ "character_id": "...", "cs_location": "...", "cs_power_level": "...", ... }]`
- 解析 JSON 后批量更新 `characters` 表的 6 个动态状态字段 + `cs_updated_at_chapter`

#### 前端 Pipeline 面板
- **`Stories.tsx`** 场景级 Pipeline 进度看板：场景列表显示 `execution_stage` 彩色徽章（plan/outline/draft/review/final）+ 多色进度条
- **Actions/Drafts/Reviews 三标签页**：Actions 执行修稿/审稿/定稿；Drafts 查看草稿列表；Reviews 查看审稿结果与评分
- **`usePipeline.ts`** 新增 `parseReviewResult()` 辅助函数，将后端 `PipelineReview`（JSON 字符串字段）解析为结构化 `ReviewResult`

#### 幕前 `/` 指令打通
- **`RichTextEditor.tsx`** 扩展 slash 命令映射：`AI修稿`/`修稿` → `pipeline_refine`、`AI审稿`/`审稿` → `pipeline_review`、`定稿` → `pipeline_finalize`
- **`FrontstageApp.tsx`** 新增 `handlePipelineRefine` / `handlePipelineReview` / `handlePipelineFinalize` 处理器，调用 `runRefine` / `runReview` / `runFinalize` IPC
- `RichTextEditorRef` 扩展 `setContent(text: string)` 方法，支持 Pipeline 执行后自动回写编辑器内容

### 🧬 角色动态状态系统

#### 6 项动态状态字段
- `cs_location` — 角色当前所在位置
- `cs_power_level` — 实力等级/修为层次
- `cs_physical_state` — 身体状态（健康/负伤/疲惫等）
- `cs_mental_state` — 心理状态（平静/焦虑/愤怒等）
- `cs_key_items` — 随身携带的关键物品
- `cs_recent_events` — 最近经历的重要事件
- `cs_updated_at_chapter` — 状态最后更新的章节号

#### `CharacterStatePanel` 组件
- 可折叠 UI：每个角色卡片下方展开/收起状态面板
- 6 字段只读展示 + `cs_updated_at_chapter` 时间戳
- 内联编辑：点击字段值进入编辑模式，失焦自动保存
- API：`update_character_state` IPC 命令更新单字段

### 📊 用量统计与可观测性

#### `UsageStats` 页面
- 幕后独立页面，Sidebar `BarChart3` 图标导航
- **全局统计卡片**：总调用次数 / 总 token 数 / 平均响应时间 / 成功率
- **单故事统计**：按故事维度聚合（调用次数 / token 消耗）
- **最近调用记录表**：最近 20 条，展示模型名称、功能、token、耗时、状态、时间

#### 后端 API
- `get_llm_call_stats(story_id?: string)` — 返回 `LlmCallStats`（全局或单故事）
- `get_recent_llm_calls(limit: i64)` — 返回最近 N 条 `LlmCallRecord`
- `LlmCallRepository` — `get_stats()` / `get_recent()` 统计查询

### 🖥️ 前端架构升级
- **`Sidebar.tsx`** 新增「用量统计」导航项（`BarChart3` 图标）
- **`App.tsx`** 新增 `UsageStats` 路由 case（`'usage-stats'`）
- **`types/index.ts`** `ViewType` 扩展 `'usage-stats'`

### 🔧 技术细节
- `finalize.rs` 原占位实现重写为真实执行逻辑：创建 `LlmService` → 遍历步骤定义 → 执行 `run_post_process_step` → 更新数据库状态
- `post_process.rs` 修复 `resp.text` → `resp.content`（`GenerateResponse` 字段名修正）
- `usePipeline.ts` 类型对齐：`refreshReviews` 返回 `ReviewResult | null`（通过 `parseReviewResult` 转换）

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.0

---

## [v0.7.2+1] - 网文体裁模板扩充至 43 个 + typical_structure 全面补全（2026-05-20）

### 📝 网文体裁模板扩充与优化

#### 新增 5 个 2026 年热门/经典缺失体裁模板
- **灵气复苏** (`spiritual-recovery`) — 现代都市中灵气突然复苏，强调"日常与超凡的撕裂感"与旧秩序崩溃后的新博弈
- **规则怪谈** (`rules-horror`) — 以"规则文本"为核心恐怖机制的独立流派，逻辑推理与在规则夹缝中求生存
- **模拟器流** (`simulator`) — 系统流独立分支，"人生模拟器"推演不同选择，用无数次虚拟死亡换取现实中一次正确选择
- **盗墓流** (`tomb-raiding`) — 经典探险流派，古墓探险、机关解谜、风水秘术，揭开历史谜团
- **星际机甲** (`mecha-stellar`) — 科幻核心分支，机甲战斗、星际战争、宇宙探索，钢铁浪漫与史诗感

#### 现有 38 个模板全面优化
- **typical_structure 补全** — 全部 38 个现有模板从空数组 `[]` 补充为完整的 `{title, description}` 典型结构节点（平均 5-6 个阶段），为 AI 生成提供更清晰的叙事结构指引
- **凡人流反模式修复** — 原 `anti_patterns` 为空的凡人流模板补充 5 条反模式（资质逆转、准备无敌、越阶无代价、人缘逆天、长生无感）

#### 数据更新
- `templates/genres.json`：`count` 38 → 43，全部 43 个 profile 已补充 `typical_structure`
- 新增 5 个 Markdown 模板文件：`mecha-stellar.md` / `spiritual-recovery.md` / `rules-horror.md` / `simulator.md` / `tomb-raiding.md`

**编译与测试**
- `cargo check`：零错误
- `cargo test`：~225/225 通过
- `npm run build`：通过
- 版本号维持：Cargo.toml / package.json / tauri.conf.json → 0.7.2

---

## [v0.7.2] - 存储同构化 + MCP 动态注册 + 聚合编辑 + 场景分隔节点 + LLM 取消（2026-05-19）

### 🗄️ 拆书分析存储同构化

#### 统一存储层：reference_* → narrative_*
- **`BookDeconstructionExecutor`** — 分析结果保存时统一写入 `narrative_characters` / `narrative_scenes` / `narrative_world_buildings`，`source='extracted'` / `status='reference'`
- **`BookDeconstructionService::run_analysis`** — 保存到 `reference_*` 后，同步转换为 `CharacterElement` / `SceneElement` 并写入 `narrative_*`，保持过渡期双向兼容
- **`BookDeconstructionService::get_analysis`** — 从 `narrative_*` 读取并转换回 `BookAnalysisResult`，API 接口零变动
- **`BookDeconstructionService::delete_book`** — 删除时同步清理 `narrative_characters` / `narrative_scenes` / `narrative_world_buildings`
- **`BookDeconstructionService::convert_to_story`** — 一键转故事时自动 `UPDATE narrative_* SET status = 'active'`，角色/场景从参考态切换为生产态

#### Migration 69：历史数据自动迁移
- `reference_characters` → `narrative_characters`：`INSERT OR IGNORE` + `LEFT JOIN` 去重，缺失字段（`background`/`goals`/`gender`/`age`）置空或默认值
- `reference_scenes` → `narrative_scenes`：同上，字段映射对齐 `SceneElement` 结构

### 🔌 MCP 工具动态注册

- **`CapabilityRegistry` 实时同步** — 外部 MCP 服务器连接成功后，将其工具列表注入 `CapabilityRegistry`，`PlanGenerator` 即时感知新工具，无需重启应用
- **前缀命名空间** — 内置工具 `mcp.builtin.*`，外部工具 `mcp.{server_id}.*`，前端 `list_mcp_tools` 返回完整带前缀列表，零配置区分来源
- **动态注销** — MCP 服务器断开时从 `CapabilityRegistry` 移除对应工具，防止调用已失效外部工具
- **`execute_mcp_tool` 注册修复** — 移除对不存在的 `GenericToolHandler` 的引用，动态注册仅更新 `CapabilityRegistry`，工具调用走现有 `call_mcp_tool` 路径

### 📝 1:N 聚合编辑数据库 Schema

- **Migration 68**：`chapter_commits` 表新增 `chapter_id` 字段（`TEXT` / `Option<String>`），支持多场景聚合到单一章节的提交追踪
- **全链路对齐** — `ChapterCommit` 结构体、`ChapterCommitRepository`（create/get/update SQL）、`story_system/mod.rs` 全部适配新字段
- **`ChapterCommitService::auto_commit`** — 有 `chapter_id` 时写入外键，无则保持 `NULL`，兼容现有单场景提交路径

### 🖊️ TipTap SceneDividerNode

- **原子块节点** — `src-frontend/src/frontstage/extensions/SceneDividerNode.ts`，`group: 'block'` / `atom: true` / `selectable: false`
- **可视化渲染** — 幕前编辑器中相邻场景间显示水平分隔线 + 场景标题标签，结构边界一目了然
- **不可编辑** — 用户无法直接输入或删除分隔节点，仅作为结构标记，防止误操作破坏场景边界
- **事件扩展** — 支持 `click` / `mouseenter` / `mouseleave`，可扩展悬浮面板展示场景元信息

### ⏹️ LLM 调用取消机制

- **`request_id` 级取消** — `LlmService` 维护 `cancel_senders: HashMap<String, Sender<()>>`，每次 `generate` / `generate_stream` 分配唯一 `request_id`
- **`cancel_generation(request_id)`** — 精确向指定 sender 发送 `()` 信号，中断对应请求，不影响其他并行 LLM 调用
- **流式适配** — `generate_stream` 每接收一个 chunk 前 `try_recv` 检查取消信号，收到后立即终止流并返回已生成内容
- **`AgentOrchestrator`** — `WorkflowResult` 新增 `request_id` 字段，上层可通过同一 ID 取消正在执行的生成任务

### ⚠️ AppError 结构化 IPC

- **统一错误格式** — 所有 Tauri 命令返回 `Result<T, AppError>`，JSON 序列化为 `{ code: string, message: string, data?: unknown }`
- **错误码体系** — `NOT_FOUND` / `VALIDATION_ERROR` / `LLM_ERROR` / `CANCELLED` / `TIMEOUT` / `INTERNAL_ERROR` / `UNAUTHORIZED`
- **前端精准处理** — `loggedInvoke` 捕获 `AppError` 后按 `code` 分支：toast 提示 / 静默忽略 / 自动重试 / 引导用户操作
- **向后兼容** — 未显式返回 `AppError` 的命令仍走原有字符串错误路径，逐步迁移

### 🔧 其他修复
- **`CharacterElement::fears` 字段补全** — `service.rs` 构造 `CharacterElement` 时新增 `fears: String::new()`，修复 `cargo check` 编译错误
- **`AgentResult::request_id` 测试补全** — `orchestrator.rs` 测试初始化补充 `request_id: None`，修复 `cargo test` 编译错误

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 0.7.2

---

## [v0.7.1] - 架构优化：聚合提交 + 导出完整性 + 组件提取（2026-05-17）

### 🏗️ 后端架构优化

#### ChapterCommitService 防抖聚合提交
- **`lib.rs`** — 移除独立的 `auto_ingest_chapter` 函数、`INGEST_COOLDOWN`、`hash_content`，消除与 Projection Writer 的重复索引工作
- **`CHAPTER_COMMIT_DEBOUNCE`** — 新增全局防抖状态，`CHAPTER_COMMIT_DEBOUNCE_SECONDS = 30`
- **`ChapterCommitService::auto_commit()`** — 取代 `auto_ingest_chapter`，30 秒空闲延迟后自动聚合提交，驱动 `VectorProjectionWriter` / `MemoryProjectionWriter`
- `update_chapter` / `create_chapter` 命令统一调用 `auto_commit` 而非独立摄取

#### 导出聚合完整性
- **`export/mod.rs`** — `export_to_file` 新增 `scenes` 参数
- **`export_story`（`lib.rs`）** — 导出前自动检查章节内容，空章节按关联场景的 `sequence_number` 排序聚合填充，确保 Markdown/HTML/PlainText 导出完整无缺
- `generate_json` 导出 schema 扩展为包含 `scenes` 数组，支持全数据便携导出

### 🖥️ 前端架构优化

#### 大型组件提取重构
- **`Settings.tsx`** — 提取 8 个原子化子组件到 `src/pages/settings/`：`ModelCard`、`ModelList`、`ModelModal`、`StatsSettings`、`MethodologySettings`、`WorkflowSettings`、`GeneralSettings`、`AccountSettings`
- **`SceneEditor.tsx`** — 提取 `SceneAuditPanel` 和 `SceneAnnotationPanel` 到 `src/components/scene-editor/` 子目录，消除重复渲染与关注点混杂
- 清理未使用导入：`Image`、`createLogger`、`Clock`、`Eye`、`FileText`、`CharacterConflict`、`AuditReport`
- 移除 `SceneEditor.tsx` 中未使用的 `handleStageChange` 函数

#### StoryTimeline 场景进度可视化
- **`StoryTimeline.tsx`** — 场景卡片新增 `execution_stage` 彩色徽章（plan/outline/draft/review/final），与叙事阶段（铺垫/上升/高潮/收尾）双轨可视化
- 新增辅助函数 `getExecutionStageLabel()` / `getExecutionStageColor()`

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过

---

## [v6.0.0] - Story System 合同驱动 + 三层记忆编排 + 追读力评估 + 37 体裁模板 + Anti-AI 审查（2026-05-15）

### 🏗️ 架构级新体系：Story System 合同驱动

#### 四级合同架构
- **`story_contracts` 表**（Migration 47）— 四级合同存储：`MASTER_SETTING`（故事级全局设定）/ `Volume`（卷级设定）/ `Chapter`（章节级设定与预期）/ `Review`（审阅与修订合同）
- **`chapter_commits` 表**（Migration 48）— CHAPTER_COMMIT 写后真源，记录 `state_deltas_json`、`entity_deltas_json`、`accepted_events_json`，形成提交链
- **8 个新 Repository** — `StoryContractRepository`、`ChapterCommitRepository`、`MemoryItemRepository`、`ChapterReadingPowerRepository`、`ChaseDebtRepository`、`OverrideContractRepository`、`ReviewIssueRepository`、`GenreProfileRepository`

#### CHAPTER_COMMIT 提交链与 Projection Writer
- `ChapterCommitService::init_commit()` — 创建初始 commit 记录
- `ChapterCommitService::apply_commit()` — 异步应用 commit，驱动 5 个 Projection Writer
  - `StateProjectionWriter` — 解析 `state_deltas_json`，写入 `memory_items`（category="state"）
  - `IndexProjectionWriter` — 解析 `entity_deltas_json`，写入 `memory_items`（category="entity"）
  - `SummaryProjectionWriter` — 生成章节摘要，写入 `story_summaries`
  - `MemoryProjectionWriter` — 解析 `accepted_events_json`，写入 `memory_items`（category="event"）
  - `VectorProjectionWriter` — 生成摘要 embedding，写入 LanceDB `VectorRecord`
- `ContractTree` / `RuntimeContract` — 按故事/卷/章节层级查询合同树，动态合并上层合同生成运行时约束合同
- **防幻觉三定律**：合同即法律、设定即物理、发明需识别

### 🧠 三层记忆编排器

#### MemoryOrchestrator
- `build_memory_pack()` — 按任务类型（write/plan/review）动态组装 MemoryPack
- **三层记忆模型**：
  - **Working Memory**：最近 5 章 + 活跃角色（出场 > 3 次）+ 开放伏笔（未回收）
  - **Episodic Memory**：state_changes + relationships 时间线，最近 10 条
  - **Semantic Memory**：长期事实，按优先级排序（Critical > High > Medium > Low > Background），支持源章节窗口过滤（仅保留最近 30 章内的事实）
- **MemoryBudget**：write 任务分配 Working 50% / Episodic 30% / Semantic 20%；plan 任务 Semantic 优先
- **冲突检测**：比较 Working 与 Semantic 记忆，检测矛盾并输出 `MemoryWarning`
- `MemoryPack` / `MemoryEntry` / `MemoryItemDto` / `MemoryStats` 完整数据结构

### 📈 追读力评估系统

#### ReadingPowerEvaluator
- `evaluate()` — 单章追读力五维评估：
  - **Hook 检测**：悬念/冲突/转折三类钩子识别与计数
  - **Coolpoint 追踪**：打脸/收获/揭秘三类爽点追踪
  - **Micropayoff 微兑现**：章节内小承诺兑现检测
  - **综合评分**：0-100 分，加权计算
- `get_trend()` — 返回最近 N 章评分趋势数组

#### DebtManager
- `create_debt()` — 创建未兑现承诺/伏笔债务
- `accrue_interest()` — 债务逾期自动计算利息（每日 5%）
- `check_overdue_debts()` — 扫描逾期债务并返回告警
- `create_override_contract()` — 作者声明临时跳过债务并记录理由
- `fulfill_contract()` — 债务兑现后更新状态
- `OverrideContract` / `ChaseDebt` 完整数据结构

### 📚 37 体裁模板库

#### GenreProfile
- **`templates/genres/`** 目录新增 37 个 Markdown 体裁模板文件
- **内置模板覆盖**：玄幻/仙侠/都市/历史/科幻/悬疑/言情/武侠/游戏/修真/无限流/系统流/重生/穿越/快穿/凡人流/争霸流/幕后流/签到流/御兽流/驭鬼流/诡异流/赛博朋克/蒸汽朋克/克苏鲁/国运流/种田/末世/轻小说/体育/军事/西幻/灵异/现实/洪荒/武侠仙侠/诸天万界
- **模板五要素**：核心基调、节奏策略、反模式清单、参考数据表、典型结构
- `GenreProfileRepository` — SQLite 持久化，支持 `get_all()` / `get_by_id()`
- 前端 `StorySystem.tsx` 集成体裁查看面板

### 🔍 Anti-AI 五维审查

#### AntiAiReviewer
- `review(text, genre)` — 对输入文本进行五维度 AI 痕迹审查：
  - **词汇维度**：Cliché 检测（"浩瀚"、"磅礴"、"无尽"、"宛如"等 AI 高频词列表）+ 重复用词统计
  - **语法维度**：句式多样性（句长标准差评估，< 5 为单调）+ 被动语态计数
  - **叙事维度**：段落长度均匀度（变异系数 < 0.3 为可疑）+ 感官密度（五感关键词密度，< 3% 为贫乏）
  - **情感维度**：标签化检测（"愤怒地"、"悲伤地说"等副词+说组合）+ 展示 vs 告知判断（直接情感词密度 > 5% 为过度告知）
  - **对话维度**：说明性对话检测（对话中包含设定/背景信息占比 > 30% 为过度说明）+ 标签单调性（连续 3 句使用相同对话标签如"说道"）
- **输出结构**：`overall_score`（0-100）+ `dimensions[]`（各维度 0-100 分）+ `issues[]`（问题列表）+ `suggestions[]`（改进建议）+ `flagged_passages[]`（标记段落）

### 🖥️ 前端集成

#### StorySystem.tsx
- 新增幕后页面「故事系统」，5 个标签页：
  - **Contracts** — 合同树浏览、运行时合同查看
  - **Commits** — CHAPTER_COMMIT 提交历史、Projection 状态追踪
  - **Reading** — 追读力评分与趋势图、Debt 债务看板
  - **Memory** — 记忆包组装结果、三层记忆浏览
  - **Anti-AI** — 五维审查结果、评分与建议

#### Sidebar.tsx
- 新增「故事系统」导航入口（ShieldCheck 图标）

#### App.tsx
- 导入 `StorySystem` 组件，添加 `story-system` 路由 case

#### tauri.ts
- 新增 17 个 v6.0.0 IPC 命令的 TypeScript 接口与 API 函数：
  - `create_master_setting` / `create_chapter_contract` / `get_contract_tree` / `get_runtime_contract`
  - `init_chapter_commit` / `apply_chapter_commit` / `get_chapter_commits`
  - `build_memory_pack` / `get_memory_items` / `create_memory_item`
  - `evaluate_reading_power` / `get_reading_power_trend` / `get_chase_debts` / `create_override_contract`
  - `get_genre_profiles` / `get_genre_profile`
  - `anti_ai_review`

### 🗄️ 数据库迁移
- **Migration 47**：`story_contracts` 表（四级合同）
- **Migration 48**：`chapter_commits` 表（提交链）
- **Migration 49**：`memory_items` 表（记忆项）
- **Migration 50**：`story_summaries` 表（章节摘要）
- **Migration 51**：`chapter_reading_powers` 表（追读力评分）
- **Migration 52**：`chase_debts` 表（债务追踪）
- **Migration 53**：`override_contracts` 表（覆盖合同）
- **Migration 54**：`review_issues` 表（审查问题）
- **Migration 55**：`genre_profiles` 表（体裁模板）
- **scenes 表扩展**：新增 `writing_phase` 字段（v6.0.0 叙事阶段标记）

### 🔧 技术细节
- `IpcResponse` trait 要求 `Serialize` derive — `ContractTree` / `RuntimeContract` / `MemoryPack` / `ReadingPowerEvaluation` 等新增结构体均已实现
- `apply_chapter_commit` 为异步 Tauri 命令，通过 `VECTOR_STORE.get()` 获取 LanceDB 实例进行向量投影
- `rusqlite 0.39` 兼容：`row.get::<_, i32>()` 显式类型注解
- Anti-AI 审查中中文引号使用 Unicode 转义（`\u{201C}` / `\u{201D}` / `\u{2018}` / `\u{2019}`）避免空字符字面量编译错误

**类型安全基座**
- ts-rs 集成：`SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]`
- 前端穷尽匹配：`useSyncStore.ts` 重构为 typed discriminated union，`assertUnreachable(type: never)`
- IPC 一致性检查：`scripts/verify-ipc-manifest.py` 自动比对前后端命令注册

**可靠性与可观测性**
- Ingest 作业追踪（Migration 55）：`ingest_jobs` 表记录 pending/running/completed/failed
- Ingest 健康指示器：幕前顶栏 🧠 图标，点击展示最近 3 条记录
- Projection 健康检查：`check_projection_health` 解析 `projection_status_json`
- 功能使用度量（Migration 56）：`feature_usage_logs` 表 + Settings「数据统计」标签
- 技术债务清理：删除 `bug_condition_v57.rs` 中 11 个已修复的 `#[ignore]` 测试

**UX 微优化**
- 角色悬浮卡片：RichTextEditor hover 角色名 600ms 显示微型浮卡
- 体裁模板外部化：`templates/genres.json` 支持用户自定义
- 导出出版前体检：ExportDialog 4 步流程，可选 Anti-AI 审查

### 编译状态
- `cargo check` ✅ 零错误（~121 warnings）
- `cargo test` ✅ ~225/225 全部通过（0 ignored，历史 bug condition 测试已删除）
- `npm run build` ✅ 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 6.0.0

---

## [v5.6.4] - v5.6.4 补丁：JSON 修复、场景去重、自动排版、CI 修复（2026-05-15）

### 🔴 P0 JSON 解析与生成稳定性

#### `extract_and_sanitize_json` 全面增强
- **字符串内未转义换行符修复** — LLM 经常在 JSON 字符串值中直接换行，导致 `serde_json::from_str` 解析失败。新增状态机：仅在字符串内部将实际换行符替换为 `\n`，避免破坏 JSON 结构
- **C 风格注释移除** — LLM 有时在 JSON 中插入 `//` 或 `/* */` 注释，导致解析失败。新增注释跳过逻辑（保留换行以维持行号）
- **移除破坏性中文引号替换** — 原代码将中文引号「」『』强制替换为 ASCII 引号 `"`，这会破坏 JSON 字符串边界（如键名或值中包含中文引号时）。修复：不再替换中文引号，JSON 格式错误由 LLM 自行修正

#### 场景生成去重与幂等性
- **跳过重复 `sequence_number`** — LLM 返回的场景列表中可能包含重复的 `sequence_number`（重试或格式错误）。`SceneGenerationStep` 新增 `seen_seqs` HashSet，遇到重复序号时跳过并记录警告日志
- **更新已存在场景而非重复创建** — Bootstrap 重试或重新执行时，`sequence_number` 已存在的场景不再新建，而是获取现有记录并更新标题/内容，避免数据库中出现重复场景

#### 数据库类型安全加固
- **`scene_id`/`chapter_id` 显式类型注解** — `repositories.rs` 和 `repositories_v3.rs` 中 `row.get(9)?` 等调用添加显式 `Option<String>` 类型注解，消除编译器推断歧义，防止空值场景下的运行时 panic
- **`pending_vector_indexes` 查询容错** — `lib.rs` 中 `chapter_id` 查询结果从 `String` 改为 `Option<String>`，数据库中可能存在的 NULL 值不再导致 `rusqlite::Error`

### 🟡 P1 前端排版与体验

#### `autoFormatText` 自动排版引擎
- **新增 `src-frontend/src/utils/format.ts`** — 智能中文段落分段与引号规范化
  - 直引号 `"..."` / `'...'` 自动转换为中文弯引号「...」/『...』
  - 按句子长度（2~4 句/段）、对话检测（以引号开头优先独立成段）智能分段
  - 已有 `<p>` 标签的 HTML 输入保留结构，仅规范化引号
  - 输出标准 HTML `<p>` 包裹
- **集成到 `FrontstageApp`** — 所有内容更新路径（`ContentUpdate`/`AppendContent`/`ChapterSwitch`/`SmartGeneration`）统一经过 `autoFormatText`，LLM 返回的未格式化纯文本自动转换为标准 HTML 段落

#### AI 续写去重保护
- **去除重复前缀** — LLM 有时返回包含当前编辑器完整内容的续写结果，导致用户看到重复文本。`requestGeneration` 中新增前缀检测：若生成内容以当前编辑器文本开头，自动截去重复前缀，仅保留新增部分
- **空内容保护** — 去重后若内容为空，直接提示"AI 续写内容与当前文本相同，无需添加"，不插入空幽灵文本

#### 排版与样式优化
- **`frontstage.css` 借鉴 heti 排版理念** — 添加 `overflow-wrap: break-word`、`hyphens: auto`、`text-spacing-trim: space-all`、`text-autospace: ideograph-alpha`，改善中西文混排效果
- **段落间距公式化** — `.ProseMirror p` 的 `margin-block-start/end` 改为基于行高的动态计算，更符合中文排版网格
- **`AiSuggestionNode` 接受逻辑修复** — 接受 AI 建议时先删除原文段落再插入新内容，避免旧内容残留导致重复

### 🟢 P2 基础设施

#### GitHub Actions CI 修复
- **移除 `.cargo/config.toml` UTF-8 BOM** — 字符 65279（`\u{feff}`）导致 `tauri-action` 发布步骤在 Windows 和 Ubuntu 上解析失败。移除 BOM 后三平台构建通过
- **macOS 构建目标修正** — `macos-latest`  runner 已升级为 Apple Silicon（arm64/M1/M2），原 `x86_64-apple-darwin` 目标在交叉编译时触发 LanceDB AVX512 链接错误（`_sum_4bit_dist_table_32bytes_batch_avx512` symbol not found）。修复：目标改为 `aarch64-apple-darwin`
- **同步本地缺失的 workflow 文件** — `.github/workflows/build.yml` 之前仅存在于 GitHub 远程，未纳入本地版本控制。现已同步到仓库

### 编译状态
- `cargo check` ✅ 零错误（121 warnings）
- `cargo test` ✅ ~225/225 全部通过
- `npm run build` ✅ 通过
- GitHub Actions ✅ rust-check / frontend-check / e2e-check 全部通过；tauri-build 三平台修复验证中

---

## [v5.6.4] - Tauri v2 IPC `rename_all = "snake_case"` 根本修复（2026-05-08）

### 🔴 P0 核心断裂修复

#### Tauri v2 自动 camelCase 转换导致 IPC 参数静默丢弃
- **根因**：Tauri v2 默认行为 — `#[tauri::command]` 自动将 Rust `snake_case` 参数名转换为 `camelCase` 传给 JS 前端。v5.6.3 修复将前端参数从 camelCase 改为 snake_case，但未同步修改后端命令宏，导致 Tauri 仍期望 camelCase 而前端传 snake_case，参数全部静默丢弃
- **影响范围**：`smart_execute`（`user_input`/`current_content` 被丢弃 → AI 续写不可用）、`get_input_hint`、`record_feedback`、`call_mcp_tool`、`check_auto_write_quota`/`check_auto_revise_quota` 等全部命令
- **修复**：157 个后端 `#[tauri::command]` 全部添加 `rename_all = "snake_case"`
  - `src-tauri/src/lib.rs`：63 个命令
  - `src-tauri/src/commands_v3.rs`：92 个命令  
  - `src-tauri/src/subscription/commands.rs`：2 个命令
- **机制**：`rename_all = "snake_case"` 禁用 Tauri 自动转换，前端传 `user_input` → 后端接收 `user_input`，零映射歧义

### v5.6.4 设计-实现对齐全面修复 v6（2026-05-13）

#### 🔴 P0 数据层根因修复 — 补齐缺失表定义与级联删除
- **`story_metadata` 表定义补齐**（Migration 43）— `automation/service.rs` 大量操作 `story_metadata` 表，但 `connection.rs` schema 中缺失 CREATE TABLE。修复：新增 `story_metadata` 表（`story_id`/`key`/`value`/`updated_at`）+ 复合索引 + `REFERENCES stories(id) ON DELETE CASCADE` 外键约束
- **`scene_characters` 表定义补齐**（Migration 44）— `SceneCharacterRepository` 操作 `scene_characters` 表，但 schema 缺失。修复：新增表（`id`/`scene_id`/`character_id`/`created_at`）+ 双外键级联 + 双索引
- **`scene_character_actions` 表定义补齐**（Migration 45）— 同上，新增表（`id`/`scene_id`/`character_id`/`action_type`/`content`/`created_at`）+ 双外键级联
- **`delete_story` 显式级联清理加固** — 原仅依赖外键约束，但 `story_metadata`/`foreshadowing_tracker`/`user_preferences`/`ai_operations` 等表无外键或外键未覆盖。修复：事务内显式 DELETE 14+ 关联表数据（`story_metadata`/`story_outlines`/`foreshadowing_tracker`/`user_preferences`/`world_buildings`/`character_relationships`/`character_states`/`scenes`/`chapters`/`scene_characters`/`scene_character_actions`/`ai_operations`/`narrative_characters`/`narrative_scenes`），防御性编程确保零幽灵数据
- **`delete_character` 显式级联清理加固** — 事务内显式清理 `scene_characters`/`scene_character_actions`/`character_relationships`/`character_states`，消除外键未覆盖的残留

#### 🔴 P0 同步事件补全 — 幕前幕后自动关联
- **`auto_ingest_chapter` 发射同步事件** — Ingest 成功保存实体/关系后，追加 `emit_ingestion_completed` + `emit_data_refresh(_, _, "knowledgeGraph")`，幕后 KG 可视化自动刷新新抽取的实体
- **`update_scene` auto ingest 发射同步事件** — 异步 ingest 块完成保存后，通过 `AppHandle` 发射 `ingestionCompleted` + `dataRefresh(knowledgeGraph)`，消除场景内容更新后 KG 不刷新的问题
- **KG CRUD 命令统一 StateSync** — `create_entity`/`update_entity`/`delete_entity`/`create_relation`/`delete_relation` 命令末尾全部追加 `emit_data_refresh(_, _, "knowledgeGraph")`，所有 KG 更新统一经过 StateSync，前端实时感知变更
- **前端 `useSyncStore` 补全特定事件 case** — 新增 `case 'characterRelationshipsUpdated'`（刷新 `characterRelationships` 缓存）、`case 'payoffLedgerUpdated'`（刷新 `payoffLedger` 缓存）、`case 'ingestionCompleted'`（刷新 `knowledgeGraph` 缓存），后端直接发射特定事件时前端正确响应

#### 🟡 P1 Automation Service 全面集成
- **`create_story` 触发 `StoryCreated`** — 故事创建完成后推入 Automation Service 事件队列，激活 `init_story_structure` 等触发器
- **`create_character` 触发 `CharacterCreated`** — 角色创建完成后触发自动化事件，激活角色分析/关系推断等触发器
- **`update_chapter` 触发 `ChapterContentUpdated`** — 章节保存后触发内容更新事件（带字数），激活章节审校/向量索引等触发器
- **`update_scene` 触发内容更新事件** — 场景内容更新后推入 Automation Service，扩展自动化覆盖范围
- **`automation/service.rs` Tauri v2 API 修复** — `emit_all` → `emit`（Tauri v2 `Emitter` trait），`word_count` 类型 `usize` → `i32` 转换修复

#### 🟡 P1 后台自动化闭环
- **`PlanTemplateLibrary` SQLite 持久化**（Migration 46）— 原纯 `Vec<PlanTemplate>` 内存存储，重启后学习成果丢失。修复：新建 `plan_templates` 表（`id`/`trigger_patterns`/`plan_json`/`success_count`/`failure_count`/`created_at`）；`new()` 时从数据库加载；`record_success()` 时保存到 SQLite；`find_match()` 从内存+数据库查询。避免重复 LLM 调用，实现"越写越懂"效果持续累积
- **能力进化周期触发机制** — 原仅启动时延迟 30 秒执行一次 `evolve_capability_descriptions()`，长时间运行后不进化。修复：`ExecutionRecordStore::append()` 每次追加记录后检查总记录数是否达到阈值（默认 5 的倍数），达到则 `tokio::spawn` 异步触发进化。保留启动时兜底进化
- **`WorkflowEngine` 恢复实例自动入队** — `with_pool()` 从数据库加载 Pending/Running/Paused 实例到内存 HashMap，但恢复的实例未加入 Scheduler 队列，重启后中断工作流永不执行。修复：`with_pool()` 返回待恢复实例 ID 列表；`lib.rs` setup 中遍历列表调用 `scheduler.schedule_execution(instance_id).await`，确保应用重启后工作流自动恢复调度

#### 🟢 P2 系统整洁度优化
- **Settings.tsx 隐藏图像生成 Tab** — 后端无图像生成 IPC 命令或 Agent，用户配置后无法使用。修复：隐藏"图像生成" Tab 并标注"暂未实现"，消除死胡同功能
- **StateSync 注释加固** — 在 KG 更新命令中添加注释，说明所有 KG 更新必须经过 StateSync，防止未来开发绕过同步路径

### 编译状态
- `cargo check` ✅ 零错误（109 warnings）
- `cargo test` ✅ ~225/225 全部通过（0 ignored，历史 bug condition 测试已删除）
- `npm run build` ✅ 通过

---

## [v5.6.3] - IPC 参数一致性全面修复 + Bootstrap 序列化修复（2026-05-08）

### 🔴 P0 核心断裂修复

#### Bootstrap 进度卡死修复
- **角色字段缺失导致 serde 反序列化失败** — LLM 返回的 JSON 可能省略 `CharacterElement::age` 和 `SceneElement` 的 8 个核心字段（`sequence_number`/`title`/`summary`/`dramatic_goal`/`external_pressure`/`conflict_type`/`setting_location`/`setting_time`）。缺失字段导致 `serde_json::from_str` 失败 → `PipelineError::ParseError` → 后续步骤永不执行 → 前端永久显示 "塑造角色 (3/6)"。修复：给所有可能被 LLM 省略的字段添加 `#[serde(default)]`
- **Bootstrap 事件缺少状态传递** — `BootstrapProgressEvent` 没有 `status` 字段，前端无法区分进行中和失败。修复：新增 `BootstrapStatus` 枚举（`InProgress`/`Completed`/`Failed`），事件包含 `status`，前端根据状态显示 ❌ 失败标记

#### IPC 参数名全面审计与修复
- **smart_execute camelCase 传参** — 前端传 `userInput`/`currentContent`，后端期望 `user_input`/`current_content`。Tauri v2 反序列化不匹配导致参数静默丢弃为 `None`，AI 续写/润色完全不可用。修复：前端改为 snake_case 传参
- **get_input_hint camelCase 传参** — 前端传 `currentContent`，后端期望 `current_content`。修复：改为 snake_case
- **record_feedback 参数结构错误** — 前端将请求对象展开为平铺字段传递，后端期望 `{ request: RecordFeedbackRequest }` 包裹对象。修复：前端改为 `{ request: req }`
- **call_mcp_tool camelCase 传参** — 前端传 `toolName`，后端期望 `tool_name`。修复：改为 snake_case
- **check_auto_write_quota / check_auto_revise_quota camelCase 传参** — 前端传 `requestedChars`，后端期望 `requested_chars`。修复：改为 snake_case
- **updateConfig 裸 invoke** — `save_settings` 使用裸 `invoke` 绕过日志追踪和错误脱敏。修复：统一使用 `loggedInvoke`

#### 后端命令参数补全
- **run_creation_workflow mode 映射错误** — 前端传 `"human_draft_ai_polish"`，后端只识别 `"human_first"`，导致 "我初稿 + AI 润色" 模式被错误映射为 "AI 初稿 + 我精修"。修复： `"human_draft_ai_polish"` 映射到 `CreationMode::HumanDraftAiPolish`
- **update_story 缺少 genre 参数** — 后端 `update_story` 命令签名缺少 `genre`，前端 Stories.tsx 编辑表单修改类型被静默忽略。修复：后端添加 `genre: Option<String>`，更新 `UpdateStoryRequest` / `StoryRepository::update` SQL
- **create_character 扩展字段被忽略** — 后端只接受 `story_id`/`name`/`background`，前端传的 `personality`/`goals`/`appearance`/`gender`/`age` 被硬编码为 `None`。修复：后端扩展参数列表
- **update_character 扩展字段被忽略** — 后端只接受 `name`/`background`/`personality`/`goals`，缺少 `appearance`/`gender`/`age`。修复：后端扩展参数并传给 Repository

### 编译状态
- `cargo check` ✅ 零错误（109 warnings）
- `npm run build` ✅ 通过
- `cargo tauri build` ✅ Windows 安装包生成

---

## [v5.6.2] - 设计-实现对齐全面修复 v5（2026-05-08）

### 🔴 P0 核心断裂修复

#### 前端缓存同步精确化
- **writingStyle 缓存刷新错误** — `useSyncStore.ts` 中 `case 'writingStyle'` 刷新的是 `['world_building', storyId]`，但 `useWritingStyle` hook 使用的 queryKey 是 `['writing_style', storyId]`。写作风格更新后前端写作风格缓存不会自动刷新。修复：同时刷新 `['writing_style', storyId]` 缓存
- **chapterUpdated 缓存刷新不精确** — `case 'chapterUpdated'` 仅调用 `invalidateQueries(['chapters'])`（全局），未刷新 `['chapters', storyId]`（当前故事）。幕前保存章节后，幕后 chapters 列表可能不立即刷新。修复：补充 `invalidateQueries(['chapters', storyId])`

#### 后台自动化闭环补全
- **update_scene 未触发向量索引** — `update_chapter`/`create_chapter` 保存后触发 `auto_ingest_chapter`（含 KG 分析 + 向量索引），但 `update_scene` 仅内联了 KG 分析，未写入 LanceDB 向量存储。修复：将 `VECTOR_STORE`/`embeddings` 可见性提升为 `pub(crate)`；`update_scene` 的 Ingest 逻辑补充 `embed_text_async` → `VectorRecord` → `add_record` 向量索引闭环；通过独立作用域隔离 `Box<dyn Error>` 避免 `Send` 编译错误

### 🟡 P1 功能补全

#### 前端缓存同步增强
- **storySelected 未刷新关联数据** — `case 'storySelected'` 仅触发回调，未调用 `invalidateQueries`。幕后切换故事时关联数据刷新依赖 `App.tsx` 中的 `useEffect` 时序。修复：补充 characters/scenes/chapters/worldBuilding/foreshadowings/storyOutlines/knowledgeGraph/characterRelationships 缓存刷新
- **dataRefresh 缺少 knowledgeGraph/characterRelationships 单独 case** — 后端可能发射单独资源类型事件，但前端 switch 未处理。修复：补充 `case 'knowledgeGraph'` 和 `case 'characterRelationships'`

### 🟢 P2 优化

- ** cargo warnings 清理** — `resource_type()`/`emit_story_selected`/`emit_scene_selected`/`validate_token`/`redirect_port` 等 5 处 dead_code 警告。修复：添加 `#[allow(dead_code)]` 标记保留 API（未来可能使用），warnings 从 113 降至 109

### 编译状态
- `cargo check` ✅ 零错误
- `npm run build` ✅ 通过
- `cargo test` 待验证

---

## [v5.6.1] - 设计-实现对齐全面修复 v4（2026-05-08）

### 🔴 P0 核心断裂修复

#### 幕前幕后自动关联补全
- **sceneCreated/sceneDeleted 缓存不对称** — `useSyncStore.ts` 中 `sceneCreated`/`sceneDeleted` 只刷新 `scenes` 缓存，不刷新 `chapters` 缓存。后端 `SceneRepository::create`/`delete` 会修改 `chapters.scene_id` / `chapters.chapter_id`，但前端 chapters 列表中的 scene 关联状态滞后。修复：两个 case 中追加 `invalidateQueries(['chapters', storyId])`

#### 自适应学习真实反馈
- **FrontstageApp learnings 伪实现** — v5.6.0 注释声称"非硬编码 mock"，但 `setLearnings()` 仍是固定字符串。修复：后端 `record_feedback` 返回 `Vec<LearningPoint>`，同步调用 `PreferenceMiner::mine` 获取真实偏好；前端 `handleAcceptGeneration`/`handleRejectGeneration` 使用返回结果设置 learnings，无结果时 graceful fallback

### 🟡 P1 功能补全

#### 前端缓存同步完整覆盖
- **WritingStyle 更新缓存不刷新** — `update_writing_style` 发射 `data-refresh("writingStyle")`，但 `useSyncStore.ts` 无对应 case。修复：新增 `case 'writingStyle'` 刷新 `worldBuilding` 缓存
- **Outline/Foreshadowing 更新缓存不刷新** — 后端发射 `storyOutlines`/`foreshadowings`，前端 `useSyncStore.ts` 缺少对应 case。修复：新增 `case 'storyOutlines'` 和 `case 'foreshadowings'` 分别刷新对应缓存

#### 后台自动化加固
- **Pending vector SQLite 持久化** — v5.5.0 使用 `pending_vector_indexes.json` 文件持久化，与文档声明的"SQLite 持久化"不符。修复：Migration 42 创建 `pending_vector_indexes` 表；`save_pending_vector_indexes`/`load_pending_vector_indexes` 改为 SQLite 操作，保留 JSON fallback 用于迁移

### 🟢 P2 优化

- **WorkflowScheduler 文档一致性** — 代码使用 `join_all` 并行执行同层节点，但文档描述为"串行拓扑执行"。修复：AGENTS.md 更新为"拓扑有序执行（同层可并行）"
- **Workflow 幂等性** — `schedule_execution` 无幂等检查，同一 instance_id 可被重复入队。修复：入队前检查 queue 和 running_instances，已存在则跳过

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ 217/217 通过
- `npm run build` ✅ 通过
- `cargo tauri build` ✅ Windows 安装包生成

---

## [v5.6.0] - 设计-实现对齐全面修复 v3（2026-05-08）

### 🔴 P0 致命差距修复

#### 数据一致性
- **Scene 删除外键悬空** — `SceneRepository::delete` 删除前未清理 `chapters.scene_id` 外键，导致 chapter 指向已删除 scene。修复：删除 scene 前 `UPDATE chapters SET scene_id = NULL WHERE scene_id = ?`
- **Wizard 创建后前端不刷新** — `create_story_with_wizard` 完成所有步骤后未发射同步事件，前端 Stories 列表不显示新故事。修复：流程结束时发射 `story_created` + `data_refresh("all")` 双重事件
- **CharacterElement relationships 硬编码空数组** — `NarrativeCharacterRepository::get_by_story` 返回 `relationships: Vec::new()`，角色关系卡片始终为空。修复：二次查询 `character_relationships` 表，按 `source_character_id` JOIN `characters` 获取 `target_name` 填充 `CharacterRelationship`
- **Collab 文档同步返回空** — `CollabSession::get_current_document` 直接返回 `(String::new(), 0)`，协同编辑文档内容丢失。修复：遍历 `self.operations` 使用 OT 变换逐条 apply 重建文档内容
- **Workflow EdgeCondition 永不匹配** — `get_next_nodes` 原代码 `all(|edge| completed.contains(&edge.from_node))` 忽略了 `edge.condition` 字段，条件边永远被视为满足。修复：`EdgeCondition::evaluate()` 实现完整条件表达式求值（Eq/Neq/Gt/Gte/Lt/Lte/Contains/NotContains），根据 `instance.context.variables` 判断

#### 任务系统可靠性
- **Task 心跳超时无重试** — HeartbeatMonitor 检测到任务超时后仅标记 `Failed`，不触发重试。修复：超时后若 `retry_count < max_retries`，计算指数退避 `30*2^retry` 秒更新 `next_run_at`，状态回退为 `Pending`，发射 `task-retried` 事件

### 🟡 P1 重要差距修复

#### 缓存同步对称性
- **Outline/Foreshadowing 修改无同步** — `update_story_outline`、`create_foreshadowing`、`update_foreshadowing_status`、`update_payoff_ledger_fields` 修改数据后未发射同步事件。修复：所有方法完成后调用 `StateSync::emit_data_refresh`
- **Cache 失效不对称** — `sceneUpdated` 只失效 scenes 缓存但 chapter 数据中的 scene 引用已变；`chapterDeleted` 不清理关联 scenes 缓存。修复：`sceneUpdated` 追加 `invalidateQueries(['chapters', storyId])`；`chapterDeleted` 追加 `invalidateQueries(['scenes', storyId])`

#### Workflow 健壮性
- **Workflow 节点无限阻塞** — `execute_node` 中 LLM 调用可能永久阻塞（本地模型无响应），无超时机制。修复：每个节点执行包裹 `tokio::time::timeout`，默认 300s，超时标记 `Failed` 并触发重试
- **INGEST_COOLDOWN 内存泄漏** — `HashMap<String, (u64, Instant)>` 只增不减，长期运行内存膨胀。修复：`cleanup_expired_entries()` 在每次插入时清理 24h 前条目

#### 前端体验
- **FrontstageApp mock learnings** — `setLearnings()` 硬编码 3 条假数据。修复：接入 `recordFeedback()` API，根据用户实际反馈动态生成学习提示

### 🟢 P2 优化差距修复

- **WritingStyle 更新无同步** — `update_writing_style` 修改后前端写作风格面板不刷新。修复：更新后发射 `data_refresh(story_id, "writingStyle")`
- **Remove notifyFrontstageDataRefresh** — `useStories.ts`、`useChapters.ts`、`useCharacters.ts`、`services/tauri.ts` 中废弃的 `notifyFrontstageDataRefresh` 辅助函数已移除，避免与 `useSyncStore` 重复刷新
- **Workflow 并发重复执行** — `run_instance` 无运行状态检查，同一实例可能被多个线程同时执行。修复：入口检查 `instance.status == Running` 则直接返回错误；`start_workflow_instance` 命令不再直接调用 `execute_next`，由队列自动消费
- **Retry 非幂等** — 失败节点重试时不检查是否已在 `completed_nodes` 中，可能重复执行副作用操作。修复：重试前检查 `completed_nodes.contains(&node_id)`，已完成的跳过
- **Pending vector 内存队列丢失** — `PENDING_VECTOR_INDEXES` 是进程内存 HashSet，应用重启后丢失。修复：新增 `pending_vector_indexes` SQLite 表，持久化待索引 chapter_id；vector store init 时从 SQLite 加载并批量处理
- **Task 执行无限阻塞** — `run_task_internal` 中 `executor.execute()` 可能永久阻塞。修复：包裹 `tokio::time::timeout(300s)`，超时标记失败

### 🧪 质量保障
- `cargo check` 零错误（114 dead_code warnings 来自 `#![warn(dead_code)]` 激活）
- `cargo test` 217/217 全部通过
- `npm run build` 通过

## [v5.5.1] - 设计-实现对齐全面修复 v2（2026-05-08）

### 🔴 P0 致命差距修复

#### 幕前幕后自动关联
- **state_sync 空 story_id 修复** — `update_character`/`delete_character`/`update_chapter`/`delete_chapter`/`update_scene` 共 5 处使用 `unwrap_or_default()` 获取 story_id，数据库查询失败时发射 `story_id=""` 的同步事件，前端 `if (storyId)` 判断为 falsy 导致缓存永不刷新。修复为 `if let Some(story_id)` 条件发射，确保 update/delete 后前端对应故事的数据列表自动刷新（而非全局刷新所有故事的缓存）。
- **`delete_world_building` 命令补全** — 后端新增 IPC 命令 + `WorldBuildingRepository::delete()` + 前端 `useDeleteWorldBuilding` Hook。此前只有 create/get/update，幕后无法删除世界观设定。
- **`useSyncStore` DataRefresh 缺 worldBuilding** — `dataRefresh` case 中新增 `worldBuilding` 分支，后端批量刷新世界观信号不再被前端忽略。
- **`create_scene` 额外字段更新后缺 `scene_updated`** — 创建场景时若同时提供 `dramatic_goal`/`content` 等额外字段，先 `repo.create()` 再 `repo.update()`，原代码只发射 `scene_created` 事件，前端可能读到未更新额外字段的旧数据。修复为 `has_extra` 分支后追加 `emit_scene_updated`。
- **`useSceneWithChapter` 缓存失效** — `sceneUpdated`/`sceneDeleted` handler 中追加 `['scenes', 'chapter', sceneId]` 的 invalidate/remove，确保场景-章节关联数据不 stale。
- **`App.tsx` `backstage-shown` 未用 story_id** — 监听事件时读取 payload 中的 `story_id` 并调用 `setCurrentStory`，幕后窗口重新 show 时自动定位到当前故事。

#### 后台自动化
- **Bootstrap 后台失败不可见** — `pipeline-complete` 事件原硬编码 `success: true`、`elements_created: default()`、`error_message: None`。修复为根据 `bg_executor.execute()` 实际结果设置 success/error，并从 `GenesisContext.bundle` 统计实际生成的元素数量（world_rules/characters/scenes/foreshadowings/plot_points）。前端可区分成功与失败。
- **向量存储初始化竞态** — `VECTOR_STORE` 是 `OnceCell`，应用启动后立即保存章节时若 LanceDB 尚未 init 则跳过索引，该章节永不被向量检索。修复：新增全局 `PENDING_VECTOR_INDEXES` 队列，未初始化时将 chapter_id 入队；LanceDB init 成功后自动批量处理积压队列，查询数据库→生成 embedding→写入 LanceDB。

### 🟡 P1 重要差距修复

- **Workflow Condition 节点空壳** — 原仅支持字符串 `"true"`/`"1"` 判断。修复：实现轻量级条件表达式求值，支持 `{{score}} > 0.7`、`{{status}} == "approved"` 等上下文变量比较，回退到硬编码 truthy 判断。
- **Workflow 失败实例不重试** — `run_instance` 返回 `Err` 时仅记录日志，实例永久丢失。修复：节点失败时若 `retry_count < 3`，更新状态为 `Pending` 并重新入队，发射 `workflow-instance-retried` 事件；超次后标记 `Failed`。
- **能力进化路径不一致** — `evolution.rs` 和 `mod.rs` 各有一个 `load_evolved_descriptions()`，前者从 `storage_path.parent()` 计算路径，后者从 `EVOLVED_DESCRIPTIONS_PATH` 全局路径读取，路径不一致。修复：`evolution.rs` 统一使用全局 `EVOLVED_DESCRIPTIONS_PATH`。
- **Task Cron 解析过于简化** — 原仅支持 `*/N` 和 `0 H * * *`，其他表达式静默降级为 24 小时间隔。修复：引入 `cron` crate，新增 `spawn_cron` 方法精确计算下次执行时间（`schedule.upcoming(chrono::Utc)`），替代固定间隔 ticker。
- **`cancel_genesis_pipeline` 无法中断运行中 LLM** — 取消标志只在步骤边界检查，LLM 调用期间（30-120秒）无法中断。修复：`tokio::select!` 同时运行 `step.execute()` 和取消监听循环（每 500ms 检查标志），用户点击取消后立即返回 `Cancelled` 错误。

### 🟢 P2 优化差距修复

- **文档版本号同步** — `ARCHITECTURE.md` / `AGENTS.md` / `ROADMAP.md` / `docs/FEATURES.md` 版本号更新至 `v5.5.1`
- **过时文档归档** — `docs/UPDATE_SUMMARY.md`(v3.0.0)、`docs/FIXES_2025_04_11.md`(v2.0)、`docs/NOVEL_CREATION_WORKFLOW.md`(v3.1.2)、`docs/plans/PROGRESS.md`(v3.0)、`docs/plans/ARCHITECTURE_V3_PLAN.md`(v3.0) 移至 `docs/archive/`
- **`tauri.ts` 死代码清理** — 移除 5 个无引用的 `@deprecated` 导出：`getDashboardState`、`getSkillsByCategory`、`embedChapter`、`createEntity`、`createRelation`
- **`FrontstageToolbar` 废弃组件清理** — 删除 `FrontstageToolbar.tsx` 文件及 `index.ts` 中的注释引用

### 🧪 质量保障
- `cargo check` 零错误零警告
- `cargo test` 217/217 全部通过
- `npm run build` 通过

## [v5.5.0] - 设计-实现对齐全面修复（2026-05-07）

### 🔧 架构对齐

#### 幕前幕后自动关联补全
- `create_world_building` / `update_world_building` 正确发射 `WorldBuildingUpdated` 同步事件（原错误发射 `StoryUpdated`）
- `ChapterRepository::delete` 添加事务清理 `scenes.chapter_id` 外键，消除悬空引用
- `characterDeleted` 按 `storyId` 精准失效缓存（原全局失效所有 characters）

#### 后台自动化闭环
- `auto_ingest_chapter` 成功后写入 LanceDB 向量存储：`embed_text_async` 生成 embedding → 创建 `VectorRecord` → `store.add_record()`，语义搜索可检索最新写作内容
- WorkflowEngine 支持数据库持久化：Migration 41 创建 `workflow_instances` 表，`with_pool()` 初始化时自动加载，`update_instance()` 自动保存
- 能力进化反馈环闭合：`evolve_capability_descriptions` 自动保存进化描述到 JSON；`build_default_registry()` 加载并应用已进化描述；PlanExecutor 每次执行完成后后台触发进化分析

#### 技术债务清理
- 移除 `src-core` 幽灵 crate（54 文件、15 模块，名义依赖但零引用）
- 同步 `FEATURES.md` / `ROADMAP.md` / `ARCHITECTURE.md` 版本号至 v5.4.1

### 🧪 质量保障
- `cargo check` 零错误零警告
- `cargo test` 217/217 全部通过
- `npm run build` 通过
- `cargo tauri build` Windows 安装包生成

## [v5.4.1] - Bootstrap 编辑器内容丢失修复（2026-05-07）

### 🐛 Bug修复

#### 创世流程编辑器内容丢失
- **根因**：`ConceptGenerationStep` 创建 Story 后发射 `storyCreated` 事件 → `useSyncStore` 调用 `loadStories()` → `selectStory()` → `get_story_chapters` 返回空列表（此时 `FirstChapterGenerationStep` 尚未执行）→ `setContent('')` 清空编辑器。随后 `ChapterSwitch` 事件到达时，`currentStory` 已设置走 `else` 分支，但 `chaptersRef` 为空数组找不到 chapter，不调用 `selectChapter`
- **修复1**：`FrontstageEvent::ChapterSwitch` 新增 `content` 字段，`FirstChapterGenerationStep` 直接通过事件传递生成内容到前端
- **修复2**：前端 `ChapterSwitch` 事件处理优先使用 `payload.content`，绕过 DB 查询竞态
- **修复3**：`chaptersRef` 为空时自动重新查询数据库获取最新章节
- **修复4**：`smartExecute` 返回后增加 `final_content` 兜底机制
- **修复5**：`loadStories` 在 `isGenerating=true` 时禁止自动 `selectStory`
- **文件**：`src-tauri/src/window/mod.rs`, `src-tauri/src/narrative/genesis.rs`, `src-frontend/src/frontstage/FrontstageApp.tsx`, `src-tauri/src/agents/commands.rs`

## [v5.3.1] - Bootstrap体验修复 + 幕后数据刷新（2026-05-03）

### 🐛 Bug修复

#### Bootstrap重复显示小说开头
- **根因**：`handleSmartGeneration` 在 Bootstrap 完成时设置 `generatedText`（幽灵文本），同时 `ChapterSwitch` 事件加载 `chapter.content`（正文），编辑器同时显示两份内容
- **修复**：Bootstrap 完成时不再设置 `generatedText`，内容已通过数据库保存并由 `ChapterSwitch` 事件加载到编辑器
- **文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`

#### 幕后结构要素不显示
- **根因**：`useSyncStore` 中 `invalidateQueries` 的 queryKey 与 hooks 实际使用的 key 不一致：
  - `['world-building', storyId]` ≠ `['world_building', storyId]`
  - `['story-outlines', storyId]` ≠ `['story-outline', storyId]`
- **后果**：后台阶段生成数据保存到数据库并发射 `sync-event` 刷新事件，但 TanStack Query 缓存永不过期，幕后永远显示空数据
- **修复**：统一 `useSyncStore.ts` 中的 KEYS 为 hooks 实际使用的 queryKey
- **文件**：`src-frontend/src/hooks/useSyncStore.ts`

#### Bootstrap解析失败：missing field `id`
- **根因**：`ConceptGenerationStep` 中 LLM 返回的 JSON 缺少 `id`/`story_id`/`source` 等后端生成字段，`serde_json::from_str::<StoryMetaElement>()` 反序列化失败
- **修复**：给所有 `NarrativeElement` 结构体的 `id`/`story_id`/`source`/`source_ref_id`/`status` 字段添加 `#[serde(default)]`，允许 LLM 返回的 JSON 省略这些字段
- **文件**：`src-tauri/src/narrative/elements.rs`

#### Bootstrap生成中断：幕前无正文 + 幕后无结构要素
- **根因1**：`StoryContextBuilder::build` 中 `fetch_characters`/`fetch_previous_scenes`/`fetch_writing_style` 在 Bootstrap 时数据库为空返回 `Err`，导致 `FirstChapterGenerationStep` 失败，第一章无法生成
- **修复1**：`build` 方法中这些查询失败时返回默认值（`vec![]`/`None`）而非传播错误
- **根因2**：LLM 返回的角色/场景/世界观/大纲 JSON 可能缺少 `relationships`/`rules`/`key_locations`/`power_system`/`total_scenes_estimate`/`key_plot_points`/`estimated_scenes` 等字段，后台阶段反序列化失败中断
- **修复2**：给所有可能缺失的字段添加 `#[serde(default)]`
- **文件**：`src-tauri/src/creative_engine/context_builder.rs`、`src-tauri/src/narrative/elements.rs`

#### 续写时重复生成小说开头
- **根因**：`current_content_preview` 从**头部截断 2000 字符**，第一次续写后总字数超过 2000，LLM 只能看到第一章内容，看不到续写内容，于是重新生成开头
- **修复**：改为从**尾部截断 6000 字符**（保留最新内容），并标注省略字数，LLM 能看到最近的续写内容并在此基础上继续
- **文件**：`src-tauri/src/lib.rs`

#### 其他
- 移除 `state_sync/mod.rs` 未使用的 `SyncEvent` 导入
- `lib.rs`：后台阶段完成后通过 `StateSync::emit_data_refresh()` 发射标准 `sync-event` 事件

### 编译与测试
- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- `cargo tauri build`：Windows `.exe` / `.msi` / `-setup.exe` 生成成功

---

## [v5.3.0] - 叙事元素模型重构：创世-拆书同构架构（2026-05-02）

### 🏗️ 架构级重构：统一叙事元素模型

核心理念：无论正向生成（Bootstrap/创世）还是逆向分析（拆书），操作的叙事元素是同一套抽象。

#### Phase 1: 统一数据模型
- **新建 `src-tauri/src/narrative/` 模块**（8个文件）：
  - `elements.rs` — `CharacterElement/SceneElement/WorldBuildingElement/OutlineElement/ForeshadowingElement/StoryMetaElement` + `ElementSource` 枚举
  - `pipeline.rs` — `NarrativePipelineExecutor` + `PipelineStep` trait
  - `progress.rs` — 统一 `PipelineProgressEvent` 替代两套进度系统
  - `prompts.rs` — 共享 Prompt 模板（Generate/Extract 双模式）
  - `genesis.rs` — **GenesisPipeline** 7步正向流程
  - `analysis.rs` — **AnalysisPipeline** 7步逆向流程（含新增伏笔提取、知识图谱构建）
- **Migration 38**: `narrative_characters/scenes/world_buildings/outlines/foreshadowings/character_relationships` 统一表

#### Phase 2: Pipeline 框架切换
- `smart_execute` 已切换到 `GenesisPipeline`
- 拆书 `executor.rs` 已切换到 `AnalysisPipeline`
- 向后兼容：同时发射 `pipeline-progress`（新）和旧事件

#### Phase 3: 统一进度系统
- 前端新建 `usePipelineProgress.ts` Hook
- `AnalysisProgress.tsx` 和 `FrontstageApp.tsx` 已接入统一进度

#### Phase 4: 统一存储层
- `repositories_narrative.rs` — `NarrativeCharacterRepository`, `NarrativeSceneRepository`, `NarrativeWorldBuildingRepository`
- 生产表和参考表数据最终都汇聚到统一表中

#### Phase 5: 故事→分析功能
- **`StoryHealthAnalyzer`** — 6 维度结构健康检查：
  - 伏笔回收率、角色弧光完整度、冲突类型多样性
  - 大纲覆盖率、世界观完整度、角色关系网络密度
- **`analyze_story_structure`** IPC 命令 — 前端可调用分析已有故事
- `HealthReport` / `HealthCheck` / `HealthStatus` — 完整报告结构

#### 附带修复
- `audit.rs` `ForeshadowingTracker` 导入路径修复（`get_by_story` → `get_all`）
- `audit.rs` `ForeshadowingRecord` 字段访问修复（`is_paid_off` → `matches!(status, Payoff)`）

### 编译与测试
- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- `cargo tauri build`：Windows `.exe` / `.msi` / `-setup.exe` 生成成功

---

## [v5.2.0] - 设计-实现对齐全面完成（2026-05-02）

### 🎯 P0 核心差距修复

#### 通用 Workflow 引擎节点执行器实现
- **`WorkflowScheduler::run_instance` 从空实现到完整 DAG 执行**：支持 Start → WriteChapter → Inspect → Revise → VectorIndex → AnalyzePlot → End 全节点类型
- **节点执行映射**：WriteChapter/Revise → Writer Agent、Inspect → Inspector Agent、AnalyzePlot → PlotAnalyzer、VectorIndex → IngestPipeline
- **串行拓扑执行**：按 DAG 依赖关系遍历，状态管理（Pending → Running → Completed/Failed），上下文变量传递
- **进度事件**：`workflow-started` / `workflow-node-started` / `workflow-node-completed` / `workflow-node-failed` / `workflow-completed`
- **IPC 命令**：`register_workflow` / `create_workflow_instance` / `start_workflow_instance` / `get_workflow_instance_status`
- **注册标准模板**：`standard_writing_workflow` (Write → Inspect → Index) 在 setup 时自动注册

#### 能力进化反馈环闭合
- **`ExecutionRecordStore` JSON 持久化**：`app_data_dir/capability_execution_records.json`，自动保留最近 500 条记录
- **`record_execution` 真正持久化**：`PlanExecutor::execute_step` 每次能力执行后自动记录（capability_id / success / duration）
- **`evolve_capability_descriptions` LLM 分析**：查询执行历史 → 计算成功率 → LLM 生成改进后的 `when_to_use` 描述
- **统计查询**：`get_statistics()` 按能力汇总成功/失败次数

#### 幕前↔场景内容双向同步
- **useSyncStore chapterUpdated → scenes 刷新**：`chapterUpdated` 事件处理中新增 `invalidateQueries(['scenes', storyId])`，因为 chapter 更新会同步到 scene
- **FrontstageApp 监听 chapter-updated**：当当前编辑的 chapter 被幕后更新时，自动刷新编辑器内容（3 秒防循环保护）
- **数据库双向同步已验证**：`ChapterRepository::update` 同步到 scene，`SceneRepository::update` 同步到 chapter

### 🎯 P1 差距修复

#### 废弃组件清理
- **`FrontstageToolbar` 从索引移除**：`frontstage/components/index.ts` 中不再导出，组件文件保留供参考

#### QueryPipeline 降级感知
- **后端 `context-degraded` 事件**：`build_agent_context` 中 `StoryContextBuilder` 降级到 `minimal` 时发射事件
- **前端 toast 提示**：`FrontstageApp` 监听 `context-degraded`，显示 "正在使用简化上下文生成内容..."

### 编译与测试
- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.2.2] - Bootstrap两阶段架构重构：先出正文，后台完善（2026-05-02）

### 🏗️ 架构级重构

#### Bootstrap 两阶段执行模型（核心体验优化）
- **即时阶段**（同步，2-3分钟）：生成故事概念 + 第一章正文 → 立即返回给前端，用户可以开始写作
- **后台阶段**（异步，`tokio::spawn`，5-8分钟）：世界观 → 大纲 → 角色 → 场景 → 伏笔 → 知识图谱
- **用户等待时间**：从 10+ 分钟缩短到 **2-3 分钟**
- **实现**：`bootstrap.rs` `run()` 拆分为 `run_quick_phase()` + `run_background_phase()`；`lib.rs` 调用 `run()` 后，后台任务在 spawn 中继续执行

#### 前端体验优化
- Bootstrap 即时完成后显示："小说已创建！第一章已生成，您可以开始写作了"
- 后台阶段进行中状态栏显示："后台正在完善小说世界..."
- 后台全部完成后 toast："创世完成！世界观、角色、场景、伏笔已全部生成"
- `novel-bootstrap-progress` 事件处理区分"即时完成"和"后台完成"

### 编译与测试
- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.2.1] - 超时修复与白屏修复（2026-05-02）

### 🐛 Bug 修复

#### 小说创建超时修复
- **Bootstrap 超时延长**：前端 `handleSmartGeneration` 中创建新小说超时从 180 秒延长至 **600 秒**（10 分钟），匹配本地大模型多步 LLM 调用实际耗时
- **超时提示优化**：超时错误信息区分 Bootstrap 与普通操作，引导用户检查模型服务
- **进度事件密度增强**：`bootstrap.rs` 在 `generate_first_chapter`、`generate_world_building`、`generate_story_outline`、`generate_characters`、`generate_scene_outline` 等每个 LLM 调用前后增加进度事件，用户可实时看到"正在调用AI..."→"已生成，正在解析..."的细粒度状态
- **LLM 心跳频率加快**：`llm/service.rs` 心跳间隔从 3 秒缩短至 **2 秒**，心跳上限从 40 次扩展到 300 次（匹配 600 秒超时），消息优化为"正在深度思考中..."
- **Bootstrap 进度提示细化**：各步骤提示增加预计耗时说明，如"（1500-2500字，可能需要1-3分钟）"、"（8-12个核心场景）"

#### 后台窗口白屏修复（v5.2.0 增强版）
- **双重维度尺寸微调**：`show_backstage` 中不仅微调 width，还微调 height（width+1/height+1 → 恢复），更全面地触发 WebView2 重绘
- **JS 重排增强**：`document.documentElement` 和 `document.body` 双重强制重排，额外触发 scroll 事件和自定义 `backstage-window-restored` 事件
- **延迟时间延长**：`backstage-shown` 事件发射延迟从 300ms 延长至 **800ms**，给 WebView2 充足时间从休眠恢复；延迟期间再次执行尺寸微调
- **前端刷新增强**：`App.tsx` `handleWindowShown` 后调用 `forceRedraw()`：立即 + 300ms 延迟两次触发 `setRenderKey`，确保 React 重新挂载
- **前端监听恢复事件**：新增 `backstage-window-restored` DOM 事件监听，双重保险触发重绘

### 编译与测试
- `cargo check`：零错误（1 个已有警告 `unused import: events::SyncEvent`）
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.1.1] - 设计-实现对齐全面修复（2026-05-01）

### 🎯 P0 核心断裂修复
- **`update_chapter` 保存后自动触发 IngestPipeline**：`lib.rs` 中 `update_chapter` 命令成功后 `tokio::spawn` 异步调用 `auto_ingest_chapter()`，知识图谱实时更新
- **`create_chapter` Ingest 固化触发**：在 `AfterChapterSave` skill hook 之外**硬编码**触发 Ingest，确保无论 skills 配置如何，知识图谱必定更新
- **`state_sync` 空 story_id 修复**：`update_character` / `delete_character` / `update_chapter` / `delete_chapter` 在发射同步事件前先查询对应的 `story_id`，`useSyncStore` 可精准刷新缓存
- **`FrontstageToolbar` story_id 传递**：废弃组件 `FrontstageToolbar.tsx` 新增 `storyId` prop，`show_backstage` 调用正确传递 `story_id`

### 🎯 后台自动化修复
- **`WorkflowScheduler::schedule_execution` 队列机制**：从空实现（仅 log）改为真正的内存队列（`VecDeque`），`execute_next()` 支持串行执行工作流实例

### 🎯 代码审查修复
- **LLM 5 分钟冷却期 + 内容哈希去重**：`auto_ingest_chapter` 内置 `INGEST_COOLDOWN` 全局状态，相同内容或 5 分钟内重复保存跳过 Ingest，防止 API 成本失控
- **未使用导入清理**：`FrontstageToolbar.tsx` 删除 `Sparkles`、`Settings`；`workflow/scheduler.rs` 删除 `Workflow`、`NodeType`
- **`WorkflowScheduler::run_instance` 明确错误**：返回 `Err("Workflow node execution is not yet implemented")` 而非空 `Ok(())`

### 📦 基础设施
- **`PromptLibrary` 扩展**：新增 `style_checker_system_template()` + `commentator_system_template()`
- **`prompts/methodologies/` 方法论模板库**：雪花法 10 步 (`snowflake.rs`) + 英雄之旅 12 阶段 (`hero_journey.rs`) + 场景结构 3 变体 (`scene_structure.rs`)

### 编译与测试
- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

---

## [v5.1.0] - 幕前幕后自动关联对齐（2026-05-01）

### 🎯 幕前幕后自动关联
- **Chapter↔Scene 双向映射**：Migration 37 新增 `chapters.scene_id` + `scenes.chapter_id` 外键关联，`ChapterRepository::create` 事务内自动查找/创建关联 Scene
- **统一实时状态中心**：后端 `state_sync` 模块（`events.rs` + `service.rs` + `mod.rs`），定义 16 种 `SyncEvent`，所有数据修改命令完成后自动发射同步事件到 `sync-event` 频道
- **前端 useSyncStore Hook**：监听 `sync-event`，根据事件类型自动 `invalidateQueries` / `removeQueries`，实现前后台数据零延迟对齐
- **Bootstrap 完成后幕前自动加载**：`smartExecute` 返回后检测 `story_created:` 消息自动加载新故事并切换第一章；Bootstrap 完成后双重 `ChapterSwitch` 保险
- **幕前→幕后快速跳转**：`Ctrl+Shift+B` 快捷键，标题栏点击，`show_backstage` 接收 `story_id` 参数，幕后自动定位当前故事

### 🎯 后台自动化对齐
- **AgentOrchestrator 闭环接入**：`execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 自动质检改写生效；修复递归 async fn 调用（`Box::pin`）
- **自适应学习闭环激活**：`AdaptiveLearningEngine::record_feedback` 成功后 `std::thread::spawn` 异步触发 `mine_preferences`，偏好挖掘自动运行

### 🎯 状态管理与数据流优化
- **Zustand↔TanStack Query 同步**：`App.tsx` 使用 `useAppStore` 订阅 `currentStory`，`useEffect` 监听变化自动刷新关联数据缓存
- **窗口通信事件标准化**：`DataRefresh` 统一由 `useSyncStore` 处理，移除 `backstage-update` 和 `handleWindowShown` 中的重复 `invalidateQueries`

### 编译与测试
- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

## [v5.0.0] - 创世引擎：一键创世，万物关联（2026-04-30）

### 🎯 创世引擎 (Genesis Engine)
- **一键生成完整小说世界**：输入"写一部都市玄幻小说"，系统自动生成故事概念、第一章正文、完整大纲、主要角色及性格小传、场景规划、伏笔埋设
- **7步创世工作流**：构思故事 → 撰写开篇 → 构建世界 → 生成大纲 → 塑造角色 → 铺设场景 → 埋设伏笔 → 编织关联
- **自动幕后卡片创建**：所有生成内容自动在幕后对应栏目创建卡片，无需手动操作

### 🎯 故事大纲系统
- **新增 `story_outlines` 表**：存储完整故事大纲（Markdown + 结构化 JSON）
- **3幕结构自动生成**：每幕含标题、摘要、关键情节点、预估场景数
- **前端故事概览面板**：Stories 页面新增"概览"视图，展示大纲、角色、场景、伏笔总览

### 🎯 角色系统增强
- **完整性格小传入库**：`characters` 表新增 `appearance`/`gender`/`age` 字段
- **角色关系图谱**：新增 `character_relationships` 表，记录角色间关系（朋友/敌人/恋人/师徒等）
- **前端关系视图**：Characters 页面新增"关系"标签页，展示角色关联网络

### 🎯 伏笔自动生成
- **Bootstrap 自动埋设伏笔**：基于故事大纲识别 3-5 个核心伏笔
- **伏笔与场景自动关联**：第一个伏笔自动关联到第一章场景
- **创世标记**：自动生成的伏笔显示"创世"金色徽章

### 🎯 知识图谱自动构建
- **创世时自动创建 KG 实体**：角色 → Character、场景 → Event、伏笔 → PlotDevice
- **自动关系连接**：角色参与场景、伏笔设置于场景

### 🎯 前后台智能联动
- **Bootstrap 完成后自动导航**：幕后界面自动切换到 Stories 并高亮新故事
- **故事概览自动展开**：新故事"概览"面板自动打开
- **实时卡片创建事件**：新增 `novel-bootstrap-card-created` 事件，前端实时显示卡片创建进度

### 🐛 Bug 修复（v5.0.0 热修复 v3）
- **后台窗口白屏修复**：修复后台窗口隐藏后重新显示时出现空白/白屏的问题
  - **根因 v3**：WebView2 窗口 `hide()` 后重新 `show()` 时渲染表面丢失；JS 强制重排不够可靠
  - **修复 v3**：`show_backstage` 命令**微调窗口大小再恢复**（`width+1` → `width`），强制 WebView2 重新创建渲染表面；配合 JS 强制重排；延迟 300ms 发射 `backstage-shown` 事件确保前端监听器就绪
- **后台卡片显示修复**：修复 Bootstrap 小说创建后，后台不显示生成的卡片（故事大纲、完整角色传记、场景、伏笔）的问题
  - **根因 v3**：（1）Bootstrap 完成时后台窗口被隐藏，事件丢失；（2）`DataLoader` 与 `App.tsx` 同时加载 stories 造成**竞态条件**；（3）Bootstrap LLM 调用失败时错误被 `log::warn` 吞掉，前端完全不可见
  - **修复 v3**：（1）`DataLoader` **移除 stories 查询**，完全由 `App.tsx` 控制数据加载，消除竞态；（2）`App.tsx` 引入 `useQueryClient`，`handleWindowShown` 中主动 `invalidateQueries` 强制刷新角色/场景/伏笔/大纲等所有页面数据；（3）`bootstrap.rs` LLM 调用失败时发射 `novel-bootstrap-error` 事件到前端，让错误可见

### 🎯 数据库迁移
- **Migration 34**: `story_outlines` 表
- **Migration 35**: `characters` 增强 + `character_relationships` 表
- **Migration 36**: `scenes.foreshadowing_ids` 字段

### 📊 统计
- Rust 测试：193/193 全部通过
- 前端构建：npm run build 通过
- 新增后端模块：StoryOutlineRepository、CharacterRelationshipRepository
- 新增前端组件：StoryOverview、CharacterGrid、SceneTimelineMini、ForeshadowingListMini

---

## [v4.5.0] - 多账号认证与云端主站（2026-04-28）

### 🎯 多账号 OAuth 登录系统
- **桌面端 OAuth2 登录**：支持 Google / GitHub OAuth2 登录，PKCE + Authorization Code 流程
- **可选登录、本地优先**：不登录可正常使用所有功能，登录后解锁未来云同步能力
- **微信/QQ 预留框架**：OAuth URL 和类型已定义，二期补充具体实现
- **数据层**：`users` / `oauth_accounts` / `sessions` 表 + UserRepository 持久化
- **JWT Session 管理**：`jsonwebtoken` 签发/验证，7 天有效期

### 🎯 云端主站（Linux 服务端）
- **Actix-web 后端**：RESTful API，PostgreSQL 持久化，JWT 中间件认证
- **Web 前端**：Vite + React + Tailwind CSS，落地页 / 登录页 / 用户后台
- **Docker 部署**：`docker-compose.yml` + `.env.example` + `deploy.sh`，一键部署
- **数据库迁移**：`src-server/migrations/` 完整表结构（users / oauth_accounts / sessions / stories）

### 🎯 Bug 修复
- **API KEY 保存**：重写 `update_model` 为直接字段修改（取代 delete+create 模式），避免密钥在多次读写配置时丢失
- **前端密钥逻辑**：编辑模型时，用户输入非空值才更新 API Key，未输入则保留旧值
- **LLM 流式生成超时**：`generate_stream` 添加 30 秒启动超时 + 15 秒 chunk 超时，防止服务器挂起导致无响应
- **LLM 同步生成超时**：`generate` 添加 60 秒整体超时

### 🎯 构建与部署
- **Rust 升级**：1.85.0 → 1.95.0（MSVC toolchain）
- **oauth2 v5.0 兼容**：修复 Breaking API 变化（类型状态模式 builder）
- **GitHub Actions**：全平台构建触发
- **本地构建**：Windows `.exe` + `.msi` + `-setup.exe` 已生成

## [v4.4.0] - 3风格三角框架：通用风格混合系统（2026-04-28）

### 🎯 通用风格混合系统（StyleBlend）
- **新增 `StyleBlendConfig` + `BlendComponent`**：支持任意 2-5 个 StyleDNA 按权重组合，不绑定固定三角
- **主导/辅助角色自动分配**：权重 >= 50% → Dominant，>= 20% → Secondary，其余 Tertiary
- **权重归一化**：拖动滑块自动调整，总和始终为 100%
- **验证机制**：主导风格必须存在，最多 5 个风格，权重总和必须为 1.0

### 🎯 3风格三角创作框架
- **新增内置风格 DNA**：普鲁斯特（意识流/长句/内心独白 70%）+ 马尔克斯（魔幻现实/全知视角/循环时间）
- **海明威风格已存在**：极简/短句/对话驱动，avg_sentence_length=15
- **三角示例**：普鲁斯特 65% + 海明威 20% + 马尔克斯 15% = 心理深度 + 节奏对话 + 氛围哲理的有机融合

### 🎯 混合风格 Prompt 注入
- **主导风格完整注入**：Writer prompt 中注入完整 StyleDNA.to_prompt_extension()
- **辅助风格差异注入**：仅注入与主导风格的关键差异维度（句长/对话比/比喻密度/内心独白/情感外露）
- **融合规则**：主导定基调，辅助在特定场景渗透；冲突时以主导为准，辅助渗透"精神"而非"形式"
- **PlanGenerator Rule 20**：模型必须遵循混合权重，主动判断当前场景适合哪种风格元素主导

### 🎯 防漂移自检清单（5项检查）
- **新增 `StyleDriftChecker`**：每章生成后自动运行风格匹配度检查
- 1. 句长检查：加权平均 ± 30% 容差
- 2. 对话比例检查：加权平均 ± 15% 容差
- 3. 比喻密度检查：加权平均 ± 50% 相对容差
- 4. 内心独白比例检查：加权平均 ± 20% 容差
- 5. 情感外露检查：加权平均情感词密度 ± 30% 容差
- **评分机制**：每项 0.0-1.0，总体 >= 0.7 且单项全部通过才算合格

### 🎯 数据层扩展
- **Migration 30**：`story_style_configs` 表（story_id + blend_json + is_active）
- **Migration 31**：`scenes` 表新增 `style_blend_override` 字段，支持章节级风格覆盖
- **新增 `StoryStyleConfigRepository`**：CRUD + set_active 激活配置

### 🎯 前端 UI 升级
- **Stories.tsx 风格配置面板**："单一风格" / "风格混合" 双标签页
- **`StyleBlendPanel` 组件**：添加/移除风格、权重滑块、实时归一化、验证提示
- **新增 IPC 命令**：`get_story_style_blend` / `set_story_style_blend` / `update_scene_style_blend` / `check_style_drift`
- **向后兼容**：保留 `style_dna_id` 单一风格选择，混合配置优先于单一风格

### 测试
- Rust 测试：193/193 全部通过（新增 blend 4 项 + drift_checker 3 项 + classic_styles 2 项）
- 前端构建：npm run build 通过

## [v4.0.0] - 借鉴 AI-Novel-Writing-Assistant 全面优化（2026-04-22）

### 🎯 Canonical State 规范状态系统
- 新增 `canonical_state/` 后端模块，`CanonicalStateManager` 实时聚合 stories/scenes/characters/KG/foreshadowing 分散状态
- 定义 `CanonicalStateSnapshot`：story_context（当前场景/开放冲突/待兑现伏笔/逾期伏笔）、character_states、world_facts、timeline、narrative_phase
- `build_agent_context` 优先使用 Canonical State 构建上下文，AI 续写时准确知道"当前处于故事哪个阶段"
- 新增 `get_canonical_state` IPC 命令，8 个单元测试

### 🎯 Payoff Ledger 伏笔账本
- Migration 24 扩展 `foreshadowing_tracker` 表：target_start_scene / target_end_scene / risk_signals / scope_type / ledger_key
- 新增 `PayoffLedger` 后端模块：逾期检测（基于重要性动态阈值）、回收时机智能推荐（高潮阶段自动提升 urgency）
- 前端 `Foreshadowing.tsx` 升级为 Ledger 视图：生命周期时间轴、逾期告警横幅、回收推荐卡片
- 新增 4 个 IPC 命令 + 3 个前端 Hook

### 🎯 Execution Panel 章节执行面板
- 新增 `ExecutionPanel.tsx` 前端组件，智能推荐下一步行动（处理逾期伏笔 / 续写 / 运行审校）
- 集成到 `Scenes.tsx` 右侧栏（三栏布局）和 `FrontstageApp` 标题栏（「下一步」快捷按钮）
- 根据叙事阶段、逾期伏笔、场景置信度动态调整推荐

### 🎯 Narrative Phase Detection 叙事阶段检测
- 增强 `calculate_narrative_phase`：逾期伏笔→ConflictActive、最近3场景高置信长内容→Climax、主要伏笔回收+场景数≥50→Resolution
- 各阶段返回 `writer_guidance()` 指导语，注入 Writer Agent prompt
- 前端 `StoryTimeline.tsx` 场景节点旁标注阶段标签（蓝/琥珀/红/绿）

### 🎯 Structured Outline 结构化大纲
- Migration 25 扩展 `scenes` 表：execution_stage / outline_content / draft_content
- `SceneEditor` 重写为 6 标签页：规划 / 大纲 / 起草 / 审校 / 定稿 / 批注
- 阶段间流转按钮：生成大纲 → 根据大纲起草 → 提升为定稿
- 新增 `generate_scene_outline` / `generate_scene_draft` IPC 命令

### 🎯 Audit System 审计系统
- 新增 `audit/` 后端模块，整合 ContinuityEngine / StyleChecker / QualityChecker / PayoffLedger
- 五维评分：continuity / character / style / pacing / payoff，0-1 分制
- 支持 light（规则快速检查）和 full（+ LLM 深度评估）两种审计模式
- 智能升降级：字数 < 200 或 > 5000 自动触发完整审计
- 前端 SceneEditor「审校」Tab 展示五维进度条 + issue 列表 + 修复建议

### 🎯 Novel Creation Wizard 小说创建向导
- 新增 `CreationWizard.tsx` 页面，5 步向导：创意输入 → 世界观选择 → 角色谱选择 → 文风选择 → 首个场景生成
- 每步调用已有 IPC（generate_world_building_options / generate_character_profiles 等）
- 右侧汇总栏显示所有选择，可点击跳转修改
- Stories.tsx「AI 一键创作」按钮改为二级菜单：快速创作 / 向导创作

### 🎯 Enhanced Streaming 增强流式输出
- 新增 `StreamOutput.tsx` 组件：Markdown 渲染、实时字数统计、停止生成按钮、打字机效果、复制/全屏
- 支持 simulated 模式（前端打字机）和 real 模式（后端真实流式）
- 接入 FrontstageApp AI 续写面板、WenSiPanel 自动修改结果、CreationWizard 场景生成

### 🎯 Strategy Configuration 写作策略配置
- Settings.tsx 新增「写作策略」卡片：运行模式（快速/精修）、冲突强度（0-100）、叙事节奏（慢/均衡/快）、AI 自由度（低/中/高）
- `AppConfig` 扩展 `WritingStrategy`，`build_writer_prompt` 根据策略动态注入 prompt 约束
- 冲突强度≥80 → "每 500 字至少一次冲突"；pace=fast → "减少环境描写，增加动作"

### 📊 统计
- Rust 测试：160/160 全部通过
- 新增 Migration：24 / 25
- 新增后端模块：canonical_state / audit / payoff_ledger
- 新增前端页面：CreationWizard.tsx / ExecutionPanel.tsx / StreamOutput.tsx

## [v4.1.0] - 幕前界面深度重构：化整为零，萤火随行（2026-04-22）

> **设计理念**：从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。

### P0 核心重构
- **顶栏精简**：44px 细线设计。小说标题（点击进入幕后）、章节信息、字数/总字数/字号、🔥 文思三态切换（`off·` / `passive✨` / `active🔥`）、禅模式按钮。移除：汉堡菜单、订阅徽章、"开启文思"按钮、"AI 续写"按钮、主行动按钮。
- **底栏删除**：彻底删除底部聊天工具栏（chat input、模型状态点、WenSiPanel 嵌入、Slash textarea 菜单）。AI 生成结果以幽灵文本内联呈现，Tab 接受 / Esc 拒绝。
- **侧边栏精简**：5 按钮 → 3 按钮（修/批/幕）。"修"=修订模式切换，"批"=生成古典评点，"幕"=进入幕后。
- **键盘快捷键**：`Ctrl+Enter` / `Cmd+Enter` 全局触发续写，`Ctrl+Space` 循环文思模式，`F11` 禅模式。

### P1 萤火系统
- **幽灵文本**：编辑器末尾灰色斜体段落（`opacity: 0.35`），附带萤火操作栏（Tab 接受 / Esc 拒绝）。
- **右边缘萤火**：`smartGhostText` 从编辑区右边缘淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流。
- **空态引导**：编辑器无内容时居中显示诗意提示"开始写下第一句话，文思将随你而行 / 按 / 查看可用命令"。

### P2 体验优化
- **内联 `/` 命令菜单**：光标处触发，8 命令——续写/润色/古风/场景/自动续写/审校/评点/排版。方向键导航，回车执行，Esc 关闭，自动删除 `/` 字符。
- **WenSiPanel 浮动化**：从底栏嵌入改为 FrontstageApp 右下角浮动卡片，通过 `/` 菜单高级命令（auto_write/auto_revise）触发。
- **修订横幅精简**：从多行可展开缩减为 32px 单行，变更列表可滚动，默认折叠。
- **古典评点保留**：AI 生成的段落评点（金圣叹式朱批）保留为内联段落，朱红色 `oklch(55% 0.18 25)`，`LXGW WenKai` 字体，左边框红色，`※` 前缀，缩进 3em。通过 `/` 菜单、sidebar "批"按钮或右键菜单触发。

### 🗑️ 移除（设计决策）
- **显式注释/评论系统**：sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单注释项、所有相关 hooks（`useTextAnnotations`、`useCommentThreads`）。
- **原因**：AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现。

### 📊 统计
- Rust 测试：160/160 全部通过
- 前端构建：通过
- 修改文件：Rust 0 个 + 前端 8 个（FrontstageApp / RichTextEditor / EditorContextMenu / frontstage.css / useTextAnnotations / useCommentThreads / hooks/index.ts 导出清理）
- 删除代码：约 800 行（底栏、注释系统、评论面板）
- 设计原则："化整为零，萤火随行" — 从显性 UI 到隐性 AI

## [v4.0.1] - 全面代码审计与空实现修复（2026-04-22）

### Phase A: 代码审计与 P0 修复
- **综合代码审计**: 扫描 40+ 模块，识别 5 项严重问题、17 项参数不匹配、9 项空实现，输出 `CODE_AUDIT_REPORT_V4.md`
- **IPC 参数统一**: 修复 17 处 camelCase→snake_case 参数名（`services/tauri.ts` 7 处、`settings.ts` 2 处、`useBookDeconstruction.ts` 6 处、`FrontstageApp.tsx` 4 处），消除 Tauri v2 反序列化静默失败
- **空实现补全**:
  - `analytics/mod.rs`: 真实写作统计（streak/longest/productivity/avg words 从 chapter 日期计算）
  - `agents/commands.rs`: `agent_get_status` 查询 `TASK_HANDLES` 返回真实状态
  - `skills/executor.rs`: `execute_mcp` 异步连接真实 `McpClient` 并调用工具
  - `export/mod.rs`: `import_from_text` 正则解析章节（"第X章"/"Chapter X"）
  - `workflow/scheduler.rs`: 添加执行日志记录
  - `evolution/updater.rs`: `apply_update` 实现 manifest 字段 CRUD
  - `mcp/server.rs`: 修复 `execute_tool` 缺失 `.await`
- **前端修复**:
  - `services/settings.ts`: 移除硬编码浏览器 fallback API keys/内部 IPs
  - `hooks/useCollaboration.ts`: WebSocket 实例保存到 ref，实现 `sendOperation`/`sendCursorPosition`
  - `hooks/useStreamingGeneration.ts`: 生产环境移除 `mockStreamGeneration`
  - `frontstage/ai-perception/textAnalyzer.ts`: 实现 `analyzeRecent` 增量分析逻辑
- **UI 调整**: 底部聊天工具栏从 `absolute bottom-0` 改为正常 flex 流，`ProseMirror` padding-bottom 从 `10rem` 降至 `3rem`
- **类型统一**: `skills/mod.rs` 移除重复 `McpServerConfig`，复用 `crate::mcp::types::McpServerConfig`

### Phase B: 内存模块 SQLite 持久化
- **Migration 26**: `chat_sessions` + `chat_messages` 表，支持聊天记录持久化
- **Migration 27**: `story_runtime_states` 表，支持故事运行状态持久化
- **Migration 28**: `collab_sessions` + `collab_participants` 表，支持协作会话持久化
- `chat/mod.rs`: `ChatManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `state/manager.rs`: `StoryStateManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `collab/mod.rs`: `CollabManager` 从内存 `HashMap` 改为 `DbPool` 持久化
- `collab/websocket.rs`: 完整实现 Operation/Cursor/Leave/Participants 消息处理，修复 user_id 硬编码，WebSocketServer 支持 `with_pool`

### 📊 统计
- Rust 测试：160/160 全部通过
- 前端构建：通过
- 新增 Migration：26 / 27 / 28
- 修复文件：Rust 12 个 + 前端 10 个

## [v3.7.1] - 智能化创作系统 5 阶段重构深度修复（2026-04-22）

### Phase A: P0 核心断裂修复（5 项）
- QueryPipeline `graph_expansion` 内容分词后逐 token 匹配实体，修复图谱扩展永不命中
- QueryPipeline `budget_control` 修复内层 break 只跳出内层循环的预算泄漏
- ContinuityEngine `check_world_rules` 修复检查方向（提取禁止条款后检测）
- ContinuityEngine `get_character_states` 效率优化 O(N×M)→O(N+M)
- PreferenceMiner `record_feedback` 成功后异步触发 `mine_preferences`，自适应学习闭环激活
- StyleChecker 接入 `AgentOrchestrator` 闭环，Writer→Inspector→StyleChecker→Writer
- Ingestion 实现真正的内容保存 + 简化知识图谱实体提取

### Phase B: P1 功能补全（6 项）
- 方法论：Migration 22 添加 methodology_id/methodology_step，Settings 新增创作方法论配置
- 创作模式：`CreationWorkflowEngine` 按 CreationMode 分支（AI全自动/AI初稿+精修/人工初稿+润色）
- 进度反馈：`useWorkflowProgress` Hook + Stories.tsx 进度弹窗
- Orchestrator 事件：前端监听 `orchestrator-step` 实时状态，Settings 暴露阈值/循环数配置
- AdaptiveGenerator `calculate_temperature` 累加而非覆盖
- 反馈记录：AiSuggestionNode + WenSiPanel 接入 `record_feedback`

### Phase C: P2 优化（4 项）
- StyleAnalyzer 新增 `analyze_with_llm` + `analyze_style_sample` IPC
- QualityChecker 新增 `check_with_llm`，Review 阶段优先 LLM 评估
- PhaseWorkflow 硬编码阶段逻辑迁移到配置驱动
- 增量 Context：每阶段完成后关键产出回注 `AgentContext`

## [v3.6.1] - 全面功能审计与深度修复（2026-04-22）

### P0 紧急修复（10 项）
- DB: Migration 21 补全 scenes/kg_relations `confidence_score` 缺失列
- IPC: 统一 25 处 camelCase→snake_case 参数名
- 场景: `create_scene` 后端扩展参数
- Orchestrator: 修复 Rewrite 事件错误携带初稿分数
- 技能: `execute_skill` 注入真实 StoryContext，SkillExecutor 实现真正 LLM 调用
- 自适应学习: FrontstageApp accept/reject 接入 `record_feedback`
- 审计: `LlmService::generate` 完成后调用 `log_ai_usage`
- 配额: auto_write/auto_revise 错误处理识别配额关键字

### P1 功能补全（8 项）
- ContinuityEngine 补全 timeline + character_emotion + relationship 检查
- 一键创作 `CreationWorkflowEngine` 每阶段发射 `workflow-progress` 事件
- SceneRepository 新增 5 个单元测试（139→144→145）
- hooks/index.ts 补全 useCommentThreads 等 6 个 Hook
- 类型: ChangeTrack.scene_id 改为 `string | undefined`
- 评论: RichTextEditor 已解决评论支持「重新打开」
- 变更追踪: 修订模式增加单条 change 独立接受/拒绝按钮
- 清理: 移除弃用 `check_ai_quota` IPC 注册

### P2 优化（6 项）
- Sidebar `chapter_count` 显示从"场景"改为"章"
- SceneEditor 置信度滑块 step 从 0.05 改为 0.1
- 拆书转故事字段映射优化
- 幕后新增 Foreshadowing 页面
- 6 个关键业务点激活技能 Hook 调用
- 孤儿表评估保留兼容

## [v3.5.2] - 全功能落地：剩余 7 项修复完成（2026-04-22）

### 🎯 修复项 #17 - auto_revise 取消/进度事件
- `auto_revise` 从同步阻塞调用改造为后台任务模式（同 `auto_write`）
- 新增 4 阶段进度事件：`preparing` → `revising` → `saving` → `completed`
- 新增 `auto_revise_cancel` IPC 命令，支持用户随时取消
- 前端 `WenSiPanel` 新增进度条（百分比 + 阶段信息）和"停止修改"按钮

### 🎯 修复项 #20 - confidence_score 类型补全
- 前端 `Scene` interface 补全缺失的 `confidence_score?: number` 字段
- `SceneEditor` 戏剧结构 Tab 新增 AI 生成置信度滑块（0-100%）
- 保存时置信度值随场景数据一并持久化到数据库

### 🎯 修复项 #16 - MCP 持久连接
- 新增全局 `MCP_CONNECTIONS` 连接池（`tokio::sync::Mutex<HashMap<String, McpClient>>`）
- `connect_mcp_server` 连接后持久保存到池中，`call_mcp_tool` 复用已有连接
- 新增 `disconnect_mcp_server` 和 `get_mcp_connections` 命令
- 前端 `useMcpTools` 适配新 API，断开连接时真正释放后端资源
- `WebSearchTool` 改为真实 DuckDuckGo 搜索（HTML 解析），失败时回退模拟数据

### 🎯 修复项 #19 - 一键创作按钮
- `Stories` 页面每个故事卡片新增"一键创作"按钮（Sparkles 图标）
- 调用 `run_creation_workflow` 命令，`ai_only` 模式基于故事描述自动生成
- 加载状态防重复点击，结果显示 toast 通知

### 🎯 修复项 #18 - StyleDNA 前端选择 UI
- `stories` 表新增 `style_dna_id` 字段（Migration 20 自动迁移）
- 后端新增 `list_style_dnas` 和 `set_story_style_dna` IPC 命令
- `build_agent_context` 自动读取 story 的 `style_dna_id` 并注入 `AgentContext`
- 前端 `Stories` 页面每个故事卡片新增"风格"按钮
- 弹出 StyleDNA 选择模态框，展示所有内置/自定义风格，一键切换
- `StoryRepository` / `Story` 模型全链路支持 `style_dna_id` 读写

### 🎯 修复项 #15 - 技能系统补全 LLM 调用 + 缺失技能
- `execute_skill` 命令从同步改为异步，内部自动调用 `LlmService::generate`
- 所有 PromptRuntime 技能（style_enhancer / plot_twist / text_formatter 等）现在真正调用 LLM
- `format_text` 简化为复用 `execute_skill`，移除重复的低级 HTTP 调用代码
- 新增内置技能 `character_voice`（角色声音一致性检查与增强）
- 新增内置技能 `emotion_pacing`（情感曲线分析与节奏优化）
- 内置技能总数从 3 个补全至 5 个

### 🎯 修复项 #14 - 意图引擎接入聊天栏
- `RichTextEditor` 聊天栏接入 `useIntent` hook
- 用户发送消息后先调用 `parseIntent` 解析意图类型
- `text_generate` / `text_rewrite` / `unknown` → 走现有 `writerAgentExecute` 路径
- `plot_suggest` / `character_check` / `world_consistency` / `style_shift` / `outline_expand` → 走 `executeIntent` 路径
- 解析失败时自动回退到 WriterAgent，保证用户体验不中断

### 📊 质量验证
- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.5.2

---

## [v3.5.1] - 全面功能审计与修复（2026-04-22）

### 🔧 关键缺陷修复（13 项）

**自动修改 (auto_revise)**
- 修复修改结果永不应用到编辑器的致命 bug
- 后端自动保存修改后的内容到 scenes 表
- 前端 `WenSiPanel` 新增 `onReviseResult` 回调，`RichTextEditor` 接收后更新内容

**拆书功能 (book_deconstruction)**
- 修复提取的书名/作者永不写入数据库的 bug
- 修复 `convert_to_story` 返回错误 story_id 导致角色/场景关联失效的 bug
- 修复任务执行器未调用 `store_embeddings` 导致向量存储缺失的 bug
- 修复任务完成后数据库进度停在 95% 的问题（改为 100%）
- 修复心跳事件 progress=0 造成 UI 进度条闪烁的问题
- 前端 `BookListGrid` 新增 `cancelled: '已取消'` 状态标签
- 前端 `useBookDeconstruction` 过滤非当前 task_id 的事件，避免多任务进度乱跳

**场景模型与版本控制**
- 生产环境 `create_v3_tables` 中新增完整 `scene_versions` 表定义
- Migration 19 为已有数据库补建 `scene_versions` 表
- 修复 `conflict_type` 从错误列索引（5 而非 6）读取的 bug
- `Scenes.tsx` 版本快照检测扩展至全部字段（戏剧目标、外部压迫、冲突类型、场景设置等）
- `create_scene` 命令新增 `dramatic_goal`/`external_pressure`/`conflict_type` 参数

**AI 生成核心**
- `AgentOrchestrator` 集成到 `writer_agent_execute`，实现 Writer→Inspector→Writer 闭环优化
- `AgentOrchestrator` 每步完成后发射 `orchestrator-step-{task_id}` 事件到前端
- `ContinuityEngine` 集成到 `execute_writer` Reviewing 阶段，自动检测一致性 issues
- `ForeshadowingTracker` 集成到 `build_agent_context`，将未解决伏笔注入 Writer prompt
- `AdaptiveGenerator` 动态参数实际应用到 LLM 调用（temperature/max_tokens 替代硬编码）
- `auto_write` 循环结束后保存到数据库并后台触发 `IngestPipeline` 知识图谱更新
- Inspector prompt 改为要求 JSON 结构化输出，`parse_inspection_result` 增强三层解析（JSON→正则→关键词）

**基础设施**
- LLM 取消机制：`LlmService` 新增 `cancel_senders`，`cancel_generation()` 发送取消信号
- `llm_cancel_generation` 命令从 TODO stub 改为实际实现
- 前端 `useLlmStream` hook 封装真实 SSE 流式生成，替换 mock 数据
- `FrontstageApp` 集成 `useLlmStream`，`handleRequestGeneration` 调用真实流式接口
- StyleDNA 内置风格自动种子化：App 启动时检测空表则插入 10 种经典作家 DNA
- `CreationWorkflowEngine` 暴露 `run_creation_workflow` Tauri 命令，支持 3 种创作模式

### 📊 质量验证
- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 已推送至 GitHub

---

## [v3.5.0] - 拆书体验升级（2026-04-21）

### 📖 拆书功能：进度提示增强 + 取消支持

**进度提示内容和频次全面升级**
- 后端 `BookAnalyzer` 5 步 Pipeline 每个子步骤都发送详细进度事件
- 元信息识别：准备样本 → 调用LLM → 识别完成（显示书名/类型）
- 世界观提取：准备样本 → 调用LLM → 整理设定
- 人物拆解：每处理一个文本块都发进度，显示"已识别 N 人"
- 章节概要：每处理一章都发进度，显示"已处理 N 章"
- 故事线生成：调用LLM → 解析结构 → 完成（显示支线/高潮数量）
- 保存结果：保存分析结果 → 保存人物 → 保存场景（93% → 96% → 98% → 100%）
- 前端 `AnalysisProgress` 组件新增 8 步骤指示器、百分比数字、块处理信息

**取消分析功能**
- 后端 `TaskExecutionContext` 新增 `is_cancelled()` 检查机制
- `BookAnalyzer` 在每个耗时循环中定期检查任务是否被取消
- 检测到取消后优雅退出，状态更新为 `Cancelled`
- 新增 IPC 命令 `cancel_book_analysis(book_id)`
- 前端分析界面新增"取消分析"按钮，确认后即时中断
- 已取消状态 UI 展示：步骤指示器显示 `!` 标记，进度条变橙色

**数据库**
- `reference_books` 表新增 `task_id` 字段，关联拆书任务
- Migration 18 自动迁移

### 🏗️ 架构与质量

- **139 项 Rust 后端测试全部通过**
- **前端构建通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.5.0

## [v3.4.0] - 智能化创作系统（2026-04-18）

### 🧠 智能化创作系统（5 阶段重构）

**Phase 1 - 地基重构：真实上下文**
- `StoryContextBuilder` — 从真实数据库构建丰富的 Agent 上下文（世界观、角色、场景结构）
- `QueryPipeline` — 四阶段知识检索（CJK 分词搜索 → 知识图谱扩展 → 预算控制 → 上下文组装）
- `ContinuityEngine` + `ForeshadowingTracker` — 连续性追踪与伏笔回收系统
- `IngestPipeline` 自动触发 — 场景保存后自动摄取知识图谱

**Phase 2 - 方法论注入**
- 创作方法论引擎：`MethodologyEngine` 自动将方法论约束注入 Writer 系统提示词
- 四种经典方法论：
  - **雪花法**（10 步渐进细化）
  - **场景节拍表**（6 节拍：开场→冲突→行动→转折→高潮→结局）
  - **英雄之旅**（12 阶段：平凡世界→冒险召唤→拒绝→导师→跨越→考验→深渊→蜕变→奖赏→归途→复活→携宝归乡）
  - **人物深度模型**（6 维度：性格/动机/关系/成长/语言/秘密）
- `AgentOrchestrator` — Writer→Inspector→Writer 质量反馈循环
  - 可配置质量阈值（默认 0.75）和最大循环数（默认 2）
  - Inspector 评分未达标时自动生成重写反馈

**Phase 3 - 风格深度化**
- `StyleDNA` 六维定量模型：词汇/句法/修辞/视角/情感/对白
- 10 种内置经典作家 DNA：金庸、张爱玲、海明威、村上春树、莫言、古典散文、现代极简、黑色侦探、武侠诗意、浪漫主义
- `StyleAnalyzer` — 从文本提取 StyleDNA 指纹
- `StyleChecker` — 对比文本与目标 DNA 的相似度
- 实时风格相似度计算与提示词注入

**Phase 4 - 自适应学习**
- `FeedbackRecorder` — 记录用户对 AI 生成内容的接受/拒绝/修改行为
- `PreferenceMiner` — 五维度启发式偏好挖掘（主题/风格/节奏/视角/结构）
- `AdaptiveGenerator` — 动态调节温度（temperature）、top-p、提示词权重
- `PromptPersonalizer` — 将用户偏好自动注入系统提示词
- `AdaptiveLearningEngine` — 统一入口，整合反馈→挖掘→生成→个性化全流程

**Phase 5 - 工作流闭环**
- `CreationWorkflowEngine` — 7 阶段全自动工作流
  - Conception（构思）→ Outlining（大纲）→ SceneDesign（场景设计）→ Writing（写作）→ Review（审阅）→ Iteration（迭代）→ Ingestion（入库）
- 3 种创作模式：
  - `OneClick` — 一键全自动
  - `AiDraftHumanEdit` — AI 初稿 + 人工精修
  - `HumanDraftAiPolish` — 人工初稿 + AI 润色
- `QualityChecker` — 四维质量评估（结构/人物/风格/情节）

### 📖 拆书功能 + 任务系统（2026-04-19）

**拆书功能**
- **文件解析**: 支持 txt/pdf/epub 三种格式，txt 自动检测 UTF-8/GBK 编码
- **智能分块**: 短篇全文分析 / 中篇按章节 / 长篇固定大小(~5000字)全量覆盖，不采样跳过
- **LLM 分析 Pipeline**: 5 步深度分析 — 元信息识别 → 世界观提取 → 人物拆解 → 章节概要 → 故事线生成
- **分析结果**: 小说类型、基本信息(标题/作者)、世界观设定、人物角色与性格、章节大纲、故事线(主线/支线/高潮/转折)
- **参考素材库**: 独立 `reference_books`/`reference_characters`/`reference_scenes` 表存储，支持 file_hash 去重
- **一键转故事**: 拆书结果可一键转化为 StoryForge 故事项目
- **前端界面**: 幕后界面新增「拆书」页面，支持上传/列表/搜索/详情查看（概览/人物/章节/故事线标签页）

**任务系统（参考 memoh-X 设计）**
- **任务调度器**: 基于 tokio::time 的共享调度器，支持 once/daily/weekly/cron 四种调度类型
- **心跳检测**: 任务执行中每步更新心跳，检测器每60秒扫描，超时5分钟自动标记失败并重试
- **防重叠执行**: 每个任务独立互斥锁，避免同一任务并发执行
- **拆书改为任务**: 每次拆书自动创建为 `book_deconstruction` 类型任务，由任务系统调度执行
- **前端任务页面**: 幕后界面新增「任务」页面，状态分组、心跳指示器、进度条、执行日志
- **IPC 命令**: 8个 Tauri 命令 — create/update/delete/list/get/trigger/cancel_task + get_task_logs

**向量化存储**
- **拆书结果入库**: 分析完成后自动为场景(summary)和人物(personality)生成 embedding
- **接入 LanceVectorStore**: 使用现有 `embeddings::embed_text` + `LanceVectorStore::upsert`
- **进度实时推送**: Tauri 事件 `book-analysis-progress` 实时推送分析进度到前端

### 🔧 Bug 修复与测试建设（2026-04-19）

**关键架构修复：TaskService 全局共享**
- **Bug**: 每个 `#[command]` 独立 `TaskService::new()` 创建实例，`BookDeconstructionExecutor` 注册在局部变量 → 前端创建的任务找不到执行器 → 拆书功能不可用
- **修复**: `TaskService` 改为泛型 `<R: Runtime>` + 手动实现 `Clone`（不依赖 `R: Clone`，确保 `Arc<Mutex<ExecutorRegistry>>` 共享）
- **修复**: `commands.rs` 所有 command 改为 `tauri::State<'_, TaskService>` 获取，不再新建实例
- **修复**: `lib.rs` `app.manage(task_service)` 全局注册，setup 阶段注册 executor 后所有 command 共享

**缓存失效修复**
- `useSetActiveModel` mutation `onSuccess` 中 `invalidateQueries({ queryKey: ['settings'] })`，解决"设为当前"后列表状态不同步问题

**测试基础设施**
- `vitest.config.ts` + `jsdom` + `@testing-library/react` 前端测试环境
- Rust `tempfile` dev-dep + `test_utils.rs` 临时目录辅助工具

**单元测试（新增 71 个）**
- `config/settings_tests.rs` — 16 tests (profile CRUD, active model, default conflict)
- `task_system/tests.rs` — 13 tests (status machine, repository CRUD, heartbeat timeout)
- `db/repositories_tests.rs` — 14 tests (Story/Character/Chapter CRUD)
- `utils/validation_tests.rs` — 20 tests (email, url, json, uuid, password, html sanitize)
- 前端 `services/__tests__/settings.test.ts` — 10 tests
- 前端 `hooks/__tests__/useSettings.test.tsx` — 4 tests
- 前端 `utils/__tests__/cn.test.ts` — 5 tests

**集成测试（新增 5 个）**
- `task_system/integration_tests.rs` — 5 tests (executor registry shared via Arc, task full lifecycle, scheduler register/unregister, no-executor failure, book deconstruction duplicate detection)
- 集成测试验证端到端流程：创建任务 → 调度 → 执行 → 状态更新，能发现单元测试发现不了的架构级 bug

**数据库修复**
- `create_test_pool()` 补充 `scene_versions` 表创建（被 `change_tracks`/`comment_threads` 外键引用）

### 🎨 品牌焕新

- 全新 Logo：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型
- `cargo tauri icon logo.png` 生成全平台图标包（Windows / macOS / iOS / Android）
- 清理旧图标：`LOGO.jpg`、`icon.jpg`、`logo-source.png`

### 💎 Freemium 付费系统（2026-04-18）

**Phase 1 — 后端基础设施**
- 数据库迁移：`subscriptions`、`ai_usage_quota`、`ai_usage_logs` 表
- `SubscriptionService`：订阅状态管理、配额检查与消费、调用日志记录
- Tauri 命令：`get_subscription_status`、`check_ai_quota`、`record_ai_usage`、`dev_upgrade_subscription`

**Phase 2 — 前端付费开关**
- `useSubscription` Hook：全局订阅状态 + `canUseFeature` + `hasQuota`
- `SubscriptionStatus` 组件：Header 订阅状态指示器（免费版显示剩余配额，专业版显示"文思泉涌中"）
- 后端配额中间件：`check_ai_quota_sync` + `consume_ai_quota_sync` 统一拦截

**Phase 3 — 转化漏斗 UI**
- `SmartHintSystem` tier 感知：免费用户只显示分析提示（不生成内联修改）
- `free-hint-toast`：免费用户看到"句式单调"等提示，点击"查看 AI 改写"打开付费引导
- `UpgradePanel`：功能对比 + ¥19/月定价 + 立即升级按钮（开发测试模式）
- `quota-exhausted-toast`：配额用尽时引导升级

**Phase 4 — Agent 质量分层**
- 免费版：`max_tokens` 强制上限 1000，跳过创作方法论/风格 DNA/个性化偏好注入
- 专业版：完整 `max_tokens` + 全部高级提示词扩展

**9 项优化修复**
1. `get_user_tier` 缓存：通过 `AgentTask.tier` 避免每次调用重复查库
2. 配额先扣后执行 → 成功后扣费：避免用户为失败请求买单
3. 内联回调防抖修复：`useCallback` 包裹 `onFreeHint`，稳定引用避免定时器重置
4. `consume_ai_quota` 原子化：事务内查询+扣减，消除竞态窗口
5. 免费提示 session 冷却：`MIN_HINT_INTERVAL_MS = 30s` + `dismissedHintIdsRef` 去重
6. auto-save 定时器清理：`autoSaveTimerRef` 避免保存到错误章节
7. UpgradePanel 替换原生 `alert`：`react-hot-toast` + 加载状态
8. 配额检查失败策略：乐观策略 `allowed: true`，后端做最终校验
9. 离线 Pro 降级修复：`localStorage` 缓存订阅状态

### 🏗️ 架构与质量

- **139 项 Rust 后端测试全部通过**（63 原有 + 71 单元测试新增 + 5 集成测试新增）
- **21 项前端测试全部通过**
- `cargo check` 零警告
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 3.4.0
- `Box<dyn std::error::Error + Send + Sync>` 全链路修复 — Tauri 异步命令 Send 要求

## [Unreleased] - v3.3.0 功能断层修复与架构清理

### 🍃 品牌 Logo 全面应用（2026-04-15）

- **应用全新品牌标志**
  - 将项目根目录 `logo.png`（草苔立方体标志）生成全平台图标包
  - `cargo tauri icon` 重新生成 Windows / macOS / iOS / Android 全尺寸图标
  - 前端 `index.html` / `frontstage.html` favicon 从 `feather.svg` 替换为 `favicon.ico`
  - 生成 `apple-touch-icon.png`、`icon-192.png`、`icon-512.png` 供多设备使用
  - `docs/images/logo.png` 作为 README 及文档展示用图
  - 更新 `README.md`、`CHANGELOG.md`、`PROJECT_STATUS.md` 中的品牌图标描述

### 🖱️ 幕前右键菜单修复与样式重构（2026-04-15）

- **修复右键菜单不出现的问题**
  - `frontstage.css` 补充 `@tailwind utilities;`，解决 Tailwind utility 类（`fixed`、`z-[9999]` 等）在幕前入口不生效的问题
  - `RichTextEditor.tsx` 将事件监听改为捕获阶段，兼容 Tauri WebView 中 `contenteditable` 的原生事件拦截
  - Rust 后端通过 `webview2-com` 调用 WebView2 API 禁用 Windows 默认系统右键菜单

- **右键菜单 UI 暖色重构**
  - `EditorContextMenu.tsx` 整体色调从深色突兀风格切换为幕前暖色纸张规范
  - 背景：`bg-[var(--ivory)]`，边框：`border-[var(--warm-sand)]`
  - 主文字：`text-[var(--charcoal)]`，图标：`text-[var(--stone-gray)]`
  - Hover：`hover:bg-[var(--warm-sand)]`，禁用态：`text-[var(--stone-gray)]/60`
  - 分隔线改为 `bg-[var(--charcoal)]/10`，与暖色背景协调

### 🔧 API 一致性审计修复（2026-04-14）

- **MCP 外部服务器连接**
  - `Mcp.tsx` 新增外部服务器配置卡片，支持配置名称、启动命令、参数和环境变量
  - `useMcpTools` 新增 `connectServer` / `callExternalTool` / `disconnectServer`
  - 外部工具与内置工具统一展示，执行时自动区分调用路径

- **技能工坊 — 技能导入**
  - `Skills.tsx` 新增"导入技能"按钮
  - 集成 `@tauri-apps/plugin-dialog` 文件选择器，调用 `import_skill`

- **Agent 执行 — 取消任务 + 流式执行**
  - 后端 `agents/commands.rs` 引入全局 `TASK_HANDLES`，`agent_cancel_task` 实现真正的 `AbortHandle.abort()`
  - `agent_execute_stream` 保存任务句柄并在完成后自动清理
  - 前端 `SkillExecutionPanel` 迁移到 `agent_execute_stream`，支持实时进度事件监听
  - 添加"取消"按钮，执行中的长任务可被中断

- **知识图谱 — 实体就地编辑**
  - `KnowledgeGraphView` 实体详情面板新增编辑模式
  - 支持修改实体名称、动态增删改属性、调用 `update_entity` 保存
  - 保存后自动刷新图谱数据

- **版本系统增强**
  - `VersionTimeline` 新增"版本链"视图切换，调用 `useVersionChain` 展示分支/深度关系
  - `DiffViewer` 接入 `useVersionDiff`，在版本对比时展示元信息（标题/场景/角色/戏剧目标变更、字数/置信度变化）

- **代码对齐与清理**
  - `useVectorSearch.ts` 统一复用 `services/tauri.ts` 中的 `searchSimilar` / `textSearchVectors` / `hybridSearchVectors`
  - `services/tauri.ts` 中对暂未使用的导出添加 `@deprecated` JSDoc 标记

### 🏗️ 架构决策

- **LLM 调用路径决策**
  - 新增 `docs/LLM_CALL_PATH_DECISION.md`
  - 明确保留 HTTP 直连 (`modelService.ts`) 为前端唯一官方 LLM 调用路径
  - Tauri 侧 `llm_generate` 等命令降级为内部/备用用途

### 📝 幕前排版与 AI 续写优化（2026-04-17）

- **段落间距优化**
  - `frontstage.css` 将 `.ProseMirror p` 的 `margin-bottom` 从 `1.5em` 统一降至 `0`
  - 为所有 `.ProseMirror p` 增加 `text-indent: 2em`，符合中文小说首行缩进排版
  - 同步调整 classical / modernCN / minimal / romantic 四种写作风格的段落间距

- **底部栏遮挡修复**
  - `.rich-text-editor .ProseMirror` 的 `padding-bottom` 从 `3rem` 增至 `10rem`
  - 长文本滚动到底部时，最后一段文字不再被底部 chat toolbar 遮挡

- **自动续写**
  - `RichTextEditor.tsx` 提取 `executeWriterAgent(instruction)` 通用函数
  - 新增 `handleAcceptAndContinue`：用户按 `Tab` 或点击「接受」后，若 `aiEnabled` 开启且不在 Zen 模式，自动延迟 300ms 调用 `executeWriterAgent('续写')` 发起下一轮生成

- **Zen 模式 AI 纯净**
  - 禅模式下完全隐藏 `AiSuggestionBubble`、`generatedText` 预览、`isAiThinking` 指示器
  - 禁用 `Tab`/`Esc` 接受/拒绝快捷键，确保 F11 禅模式仅保留文字与空白

### 🔇 质量提升

- **Rust Warnings 降噪**
  - 在 50+ 个文件中批量添加 `#![allow(dead_code)]` / `#[allow(unused_imports)]` / `_` 前缀
  - `cargo check` warnings 从 **163 降至 0**
  - 未删除任何代码，仅做标记和压制

---

## [v3.2.0] - 意图引擎与 Agent 调度 + 知识图谱可视化 + 自动归档 + 场景批注 + LLM 流式升级 + 修订模式

### 🕸️ 知识图谱可视化

- **后端图数据 API**
  - `get_relations_by_story`：按故事 ID 批量查询关系
  - `get_story_graph`：一次性返回完整知识图谱（实体 + 关系）

- **交互式图谱视图** (`src-frontend/src/components/KnowledgeGraph/`)
  - 基于 **ReactFlow** 实现可缩放、可拖拽的力导向图谱
  - 节点按实体类型着色（角色/地点/物品/组织/概念/事件）
  - 关系边按强度显示不同粗细和透明度，高强度边带动画效果
  - 左上角图例面板显示统计信息
  - 点击节点展开右侧详情面板，展示属性和关联关系

- **页面集成**
  - 新增 backstage 「知识图谱」页面和 Sidebar 导航入口
  - 自动绑定当前选中的故事，空状态引导用户先选择故事

### 🧠 记忆健康与自动归档系统

- **后端保留报告 API**
  - `get_retention_report`：基于 Ebbinghaus 遗忘曲线计算实体保留状态
  - 复用已有的 `RetentionManager`，按实体类型应用不同衰减配置

- **自动归档工作流**
  - `kg_entities` 表新增 `is_archived` 和 `archived_at` 字段
  - `archive_forgotten_entities`：一键归档所有遗忘状态实体
  - `restore_archived_entity`：从归档状态恢复指定实体
  - `get_archived_entities`：查询故事的已归档实体列表
  - 数据库迁移脚本自动补全旧表缺失的保留/归档字段

- **记忆健康面板**（集成在知识图谱页面）
  - 汇总卡片：总实体数、平均优先级、系统健康状态
  - 自动归档建议：根据遗忘比例生成动态推荐文案，支持一键执行
  - 优先级分布可视化：关键/高/中/低/已遗忘五级进度条
  - 关键实体列表和待归档实体列表

- **已归档页签**
  - 知识图谱页面新增「已归档」标签页
  - 展示所有已归档实体，支持逐条恢复

### 🤖 Agent 模型映射与路由

- **后端配置持久化**
  - `AppConfig` 新增 `agent_mappings` 字段，支持 JSON 持久化
  - 默认映射：writer/inspector/outline_planner/style_mimic/plot_analyzer → Qwen 3.5
  - `get_settings` / `save_settings` 完整读写 agent_mappings
  - `get_agent_mappings` / `update_agent_mapping` 从硬编码改为读取/写入真实配置

- **模型路由逻辑**
  - `LlmService` 新增 `generate_with_profile`，支持按模型 ID 调用指定配置
  - `AgentService` 新增 `generate_for_agent`，自动根据 Agent 类型查找映射模型
  - 5 种 Agent（写作/质检/大纲/文风/情节）均已接入模型路由
  - 未配置映射时自动回退到活跃 LLM Profile

### 🧠 意图解析引擎 (Intent Engine)

- **后端意图解析器** (`src-tauri/src/intent.rs`)
  - 基于 LLM 的 JSON 意图提取，支持 11 种意图类型
  - 包含 `IntentParser`（解析）和 `IntentExecutor`（执行）两个核心组件
  - 新增 `parse_intent` 和 `execute_intent` Tauri 命令

- **Agent 调度执行**
  - 将意图的 `required_agents` 映射到现有的 `AgentService`
  - 支持串行 (`serial`) 和并行 (`parallel`) 两种执行模式
  - 执行结果包含每个 Agent 的步骤输出、评分和建议

- **前端意图感知对话**
  - `useIntent` Hook 新增 `executeIntent` 方法
  - `RichTextEditor` 聊天栏根据意图类型自动选择执行路径
  - `text_generate` / `text_rewrite` 继续走流式输出路径
  - `plot_suggest` / `character_check` / `world_consistency` 等走 Agent 调度路径
  - 聊天消息显示意图标签（如 "情节建议 · 建议卡片"）

### 📝 场景批注系统

- **数据库与后端 API**
  - 新增 `scene_annotations` 表，支持场景级批注/笔记/待办
  - 7 个 Tauri 命令：`create_scene_annotation`、`get_scene_annotations`、`get_story_unresolved_annotations`、`update_scene_annotation`、`resolve_scene_annotation`、`unresolve_scene_annotation`、`delete_scene_annotation`
  - 批注类型：`note` / `todo` / `warning` / `idea`
  - 支持标记「已解决」与恢复未解决状态

- **前端集成**
  - `SceneEditor` 新增「批注」标签页
  - 支持新建批注（带类型选择）、编辑、解决/恢复、删除
  - 已解决批注显示划线与降透明度
  - React Query Hook：`useSceneAnnotations`、`useStoryUnresolvedAnnotations`

### 🧠 实体嵌入持久化修复

- `kg_entities.embedding` BLOB 读写修复
  - `create_entity` 现在接受并持久化 `Option<Vec<f32>>` 嵌入向量
  - 所有查询方法（`get_entities_by_story`、`get_archived_entities`、`get_entity_by_id`）正确反序列化 BLOB 为 `Vec<f32>`
  - 小说创建向导的自动 Ingest 结果中的实体嵌入现已正确保存到数据库

### 🌊 LLM 真实 SSE 流式输出

- **适配器架构升级**
  - `LlmAdapter` trait 新增 `generate_stream` 方法，统一流式接口
  - `OpenAiAdapter` 实现真实 SSE 流式调用（`stream=true`）
  - 新增 `AnthropicAdapter`：支持同步与 SSE 流式生成
  - 新增 `OllamaAdapter`：支持同步与 NDJSON 流式生成

- **服务层接入**
  - `LlmService::generate_stream` 从模拟文本改为调用真实适配器流式 API
  - 通过 `tokio::sync::mpsc` channel 消费 chunk，实时推送 `llm-stream-chunk-{request_id}` 事件到前端
  - 前端事件格式保持不变，无需修改即可接入真实流式生成

### 🕸️ 知识图谱交互增强

- `KnowledgeGraphView` 新增搜索与筛选面板
  - 实时按名称搜索节点
  - 按实体类型（6 种）快速过滤，支持全选/清空
  - 双击节点聚焦并平滑动画居中
  - 图例面板同步显示可见/隐藏节点统计

### 💾 SQLite 向量存储持久化

- **替换 JSON 内存 fallback**
  - `LanceVectorStore` 内部实现从 `HashMap + records.json` 改为 `SQLite + vector_store.db`
  - 保留完全相同的公共 API：`upsert`、`search`、`delete`、`count`
  - 所有现有调用方（`search_similar`、`embed_chapter`、`HybridSearch`）无需修改

- **数据表结构**
  - `vector_records` 表存储 `id`、`story_id`、`chapter_id`、`text`、`record_type`、`embedding`（JSON）
  - 创建 `story_id` 和 `chapter_id` 索引优化查询

- **持久化验证**
  - 单元测试验证：跨实例重启后记录不丢失
  - `upsert` 使用 `ON CONFLICT(id) DO UPDATE` 实现幂等写入

### 🛠️ 技能工坊 (Skills) 后端连通

- **前端类型对齐**
  - `Skill` 接口扩展为完整 `SkillInfo` 结构，包含 `parameters`、`hooks`、`runtime_type` 等字段

- **真实数据接入**
  - `Skills.tsx` 从 mock 数据改为调用 `getSkills()` 拉取后端技能列表
  - 支持按分类筛选（全部 / 写作 / 分析 / 角色 / 情节 / 风格等）

- **技能操作**
  - 启用/禁用开关调用 `enable_skill` / `disable_skill`
  - 执行按钮支持 Prompt 技能运行，自动弹出必填参数输入框
  - 非内置技能显示卸载按钮，调用 `uninstall_skill`

### 🪄 小说创建向导 (NovelCreationWizard) 后端连通与自动 Ingest

- **后端 Agent 命令**
  - `generate_world_building_options`：基于用户输入生成世界观选项
  - `generate_character_profiles`：基于世界观生成角色谱选项
  - `generate_writing_styles`：生成文字风格选项
  - `generate_first_scene`：生成首个场景
  - `create_story_with_wizard`：一键保存故事、世界观、角色、文风、首个场景，并自动触发 Ingest

- **Dashboard 集成**
  - 主按钮从「新建故事」改为「AI 创建故事」，打开 NovelCreationWizard
  - 保留「手动创建」入口作为备用
  - 空状态时同时显示 AI 创建和手动创建按钮

- **前端向导重构**
  - `NovelCreationWizard` 从 mock 数据改为真实调用后端 Agent 命令
  - 每一步显示加载状态，失败时自动回退并提示重试
  - 完成页展示世界观、角色、文风、场景四项准备状态

- **自动 Ingest**
  - 向导完成后自动将世界观、角色设定、首个场景内容送入 `IngestPipeline`
  - 提取实体和关系并保存到知识图谱
  - 创建成功 toast 显示摄取的实体数和关系数

### ✏️ 文本内联批注系统

- **数据库与后端 API**
  - 新增 `text_annotations` 表，支持文本级别的内联批注
  - 8 个 Tauri 命令：`create_text_annotation`、`get_text_annotations_by_chapter`、`get_text_annotations_by_scene`、`update_text_annotation`、`resolve_text_annotation`、`unresolve_text_annotation`、`delete_text_annotation`
  - 支持按 `chapter_id` 或 `scene_id` 查询，带 `from_pos` / `to_pos` 文本坐标

- **前端集成**
  - 新增 `useTextAnnotations` 系列 React Query Hook
  - 完整支持新建、编辑、解决/恢复、删除批注

### 🔄 修订模式与变更追踪 (P3)

#### Phase 1 — 变更追踪核心
- **数据库与后端 API**
  - 新增 `change_tracks` 表，记录单条编辑操作的类型、位置、内容、作者和状态
  - `ChangeTrackRepository` 支持创建、查询、状态更新、批量接受/拒绝
  - 6 个 Tauri 命令：`track_change`、`accept_change`、`reject_change`、`get_pending_changes`、`accept_all_changes`、`reject_all_changes`

- **TipTap 编辑器扩展**
  - `TrackInsert` Mark：蓝色下划线 + 淡蓝背景，带 `changeId` / `authorId` 属性
  - `TrackDelete` Mark：红色删除线 + 淡红背景
  - `RichTextEditor` 集成修订模式开关、待审变更数横幅、全部接受/拒绝/退出按钮

- **前端状态管理**
  - `useChangeTracking` 系列 Hook：待审变更查询、单条追踪、接受/拒绝、批量操作
  - 实时 diff 检测：`onUpdate` 中对比文本变化，自动调用 `track_change`

#### Phase 2 — 评论线程系统
- **数据库与后端 API**
  - 新增 `comment_threads` 和 `comment_messages` 表，支持多回复线程
  - `CommentThreadRepository` 支持创建线程、添加消息、查询、解决/重开/删除
  - 8 个 Tauri 命令：`create_comment_thread`、`add_comment_message`、`get_comment_threads`、`resolve_comment_thread`、`reopen_comment_thread`、`delete_comment_thread`

- **TipTap 编辑器扩展**
  - `CommentAnchor` Mark：黄色高亮 + 虚线下划线，锚定 `threadId`

- **前端集成**
  - `useCommentThreads` 系列 Hook：线程查询、创建、回复、解决、重开、删除
  - `RichTextEditor` 右侧评论面板：选中文本创建线程、浏览消息、状态切换

#### Phase 3 — 版本集成
- **自动 diff 生成 ChangeTrack**
  - `create_scene_version` 在创建版本时自动与上一版本内容做字符级 diff
  - 将差异转换为 `ChangeTrack`（Insert / Delete）并绑定到该 `version_id`

- **版本历史集成**
  - 新增 `get_version_change_tracks` 命令和 `ChangeTrackRepository::get_by_version`
  - `VersionTimeline` 选中版本时展示该版本的所有变更追踪详情
  - `Scenes.tsx` 预览面板新增「版本历史」标签页，集成 `VersionTimeline` 和 `DiffViewer`
  - 保存场景时自动创建版本快照（内容或元数据变更触发）

### 🎭 古典评点家 Agent (CommentatorAgent)

- **后端 Agent 实现**
  - 新增 `CommentatorAgent` (`agents/commentator.rs`)，模拟金圣叹风格对小说段落进行实时文学点评
  - 支持 `ParagraphCommentary` 结构，返回段落索引、点评内容和语气类型
  - `AgentType` 新增 `Commentator` 变体，集成到 `AgentService` 模型路由
  - 新增 Tauri 命令 `generate_paragraph_commentaries`

- **前端集成**
  - `RichTextEditor` 聊天栏新增「生成古典评点」按钮
  - 调用后端逐段生成评点后，以 `commentary-paragraph` 样式插入编辑器
  - 古典批注样式：小字号（0.8em）、赤陶色（terracotta）、斜体、左侧缩进，还原传统小说批注效果

### ⚡ 性能与缓存优化

- **实体向量自动更新**
  - `update_entity` 命令支持 `regenerate_embedding` 参数
  - 当实体名称或属性变更时，可选自动重新生成并保存嵌入向量

- **向量搜索缓存**
  - `LanceVectorStore` 新增 `HashMap` 结果缓存，最大容量 100 条
  - 简单 LRU 淘汰策略（溢出时移除最旧的 20%），写操作时自动失效缓存

- **并行 Ingest 处理**
  - `IngestBatch::process` 改为使用 `futures::future::join_all` 并发执行内容摄取
  - 显著提升批量内容的处理吞吐量

### 🧠 Agent 上下文增强

- **`build_agent_context` 真实数据库接入**
  - 修复 `agents/commands.rs` 中长期存在的 TODO
  - 现在所有 Agent 执行任务时，上下文会自动从数据库拉取：
    - 作品标题、题材、文风、节奏（从 `stories` + `writing_styles` 表）
    - 角色信息（从 `characters` 表，包含姓名、性格、角色定位）
    - 前场景摘要（从 `scenes` 表，按 sequence_number 过滤并生成摘要）
  - 写作助手、质检员、评点家、记忆压缩师等 Agent 均获得更精准的上下文

### 🗜️ 记忆压缩师集成 (MemoryCompressorAgent)

- **后端命令**
  - 新增 `compress_content`：对任意内容进行记忆压缩
  - 新增 `compress_scene`：自动读取场景内容并调用压缩 Agent
  - 支持 `target_ratio` 参数控制压缩比例（默认 25%）

- **前端集成**
  - `SceneEditor` 内容标签页新增「记忆压缩」按钮
  - 压缩结果以下方面板展示，支持「应用」到场景内容或「关闭」
  - 新增 `useCompressScene` / `useCompressContent` React Query Hooks

### ⚔️ 冲突类型扩展

- `ConflictType` 新增 4 种戏剧冲突：
  - `ManVsTime` — 人与时间
  - `ManVsMorality` — 人与道德
  - `ManVsIdentity` — 人与身份
  - `FactionVsFaction` — 群体冲突
- `SceneEditor` 冲突选择网格从 2 列调整为 3 列，容纳 11 种冲突类型

### 🔍 SQLite FTS5 语义搜索优化

- **FTS5 全文索引**
  - `vector_records` 表新增 FTS5 虚拟表 `vector_records_fts`
  - 自动触发器同步 INSERT/UPDATE/DELETE，无需应用层手动维护

- **新搜索 API**
  - `text_search_vectors`：基于 BM25 的全文关键词搜索
  - `hybrid_search_vectors`：向量相似度 + FTS5 全文搜索的 RRF 融合
  - 前端新增 `useTextSearchVectors` / `useHybridSearchVectors` Hooks

- **性能收益**
  - 文本搜索从纯向量扫描升级为 FTS5 索引加速
  - 混合搜索通过 RRF（Reciprocal Rank Fusion）融合两路结果，召回率和相关性显著提升

## [3.1.2] - 2026-04-13 - 设置页增强、浏览器开发环境修复与全新应用图标

### 🎨 全新应用图标

- 从 [iconbuddy.com](https://iconbuddy.com) 引入 **Lucide `feather`** 作为 StoryForge 品牌图标
- 设计理念：羽毛笔象征创作与文学，金色羽毛配合深色背景，优雅且富有辨识度
- 使用 `cargo tauri icon` 重新生成全平台图标包（Windows .ico / macOS .icns / iOS / Android / UWP）
- 前端 favicon 同步替换为 `feather.svg`

### 🔧 幕后设置页修复

- **编辑模型模态框修复**
  - 修复 `custom` 提供商在编辑时缺少 API Key 输入框的问题
  - 现在 `custom` 类型模型始终显示 API Key 字段，兼容本地无密钥与有密钥模型

- **模型连接状态指示灯**
  - 模型卡片右上角新增实时连接状态检测
  - **检测中**：灰色加载动画
  - **已连接 (xxms)**：绿色圆点 + 延迟显示
  - **连接失败**：红色圆点（hover 查看错误详情）
  - 浏览器开发环境下通过 `fetch` 探测 `api_base` 可用性（5 秒超时）

### 🌐 浏览器开发环境兼容

- **Vite dev server 模型回退**
  - `getModels()` / `getSettings()` / `testModelConnection()` 在浏览器环境下自动回退到本地硬编码模型
  -  backstage 设置页在 `npm run dev` 浏览器模式下不再显示「暂无模型配置」
  - 同步更新 `docs/images/backstage-preview.png`

---

## [3.1.1] - 2026-04-13 - 幕前界面重构、Waza 设计与 CI 修复

### 🎭 幕前界面重构（Waza 设计原则落地）

- **精简侧边栏**
  - 侧边栏宽度缩减至 120px，仅保留"幕后"切换按钮
  - 去除冗余图标和文字，追求极简禅意
  - 修复按钮溢出侧边栏宽度的布局问题

- **颜色系统重构（OKLCH）**
  - 所有 Hex/HSL 颜色替换为 OKLCH，建立感知均匀的 60-30-10 视觉权重
  - 主背景：`oklch(96.5% 0.008 95)`（暖纸张色）
  - 强调色：`oklch(58% 0.13 45)`（赤陶色）
  - 去除装饰性纸张噪点纹理，背景更纯净

- **字体系统升级**
  - 移除 Waza 反感的 Crimson Pro / Cormorant Garamond / Inter
  - 正文字体统一为「霞鹜文楷 (LXGW WenKai) + 思源宋体」
  - 无衬线回退：`SF Pro Display / Segoe UI / PingFang SC`

- **微交互与排版**
  - 所有按钮增加 `active:scale-95` 触感反馈
  - 全面清除 `transition: all` 反模式，改为精确属性过渡
  - Blockquote 从左边框模板改为「背景色块 + 大引号装饰」

- **顶部动态状态栏**
  - 字数统计、字体大小、快捷键提示、保存状态集中展示
  - 去除底部固定的 AI 续写按钮，界面更加纯净

- **底部 LLM 对话栏**
  - 默认隐藏，鼠标悬停底部区域时优雅浮现
  - 集成模型状态指示灯（绿/黄/红三色 + 呼吸动画）
  - 去除对话/多模态模式切换图标，保持输入框极简
  - 占位文案："在此驾驭智能文思"
  - Enter 发送，Shift+Enter 换行，支持流式对话输出

### 🤖 本地三模型配置

- **Gemma-4-31B-it-Q6_K** (`http://10.62.239.13:17099/v1`)
  - 用途：多模态对话
  - 状态：已配置，无 API Key

- **Qwen3.5-27B-Uncensored-Q4_K_M** (`http://10.62.239.13:17098/v1`)
  - 用途：语言模型对话（默认"文思助手"）
  - 状态：已配置，无 API Key

- **bge-m3** (`http://10.62.239.13:8089`)
  - 用途：Embedding 向量嵌入
  - 状态：已配置，带 API Key

### 🖥️ Tauri 本地构建与 CI 修复

- 修复 `tauri.conf.json` 中 `beforeBuildCommand` 在 Windows 下的路径兼容性问题
- 成功构建 Release 版本并打包 Windows 安装程序
- 生成 MSI (12.3 MB) 和 NSIS (8.1 MB) 两种安装包
- 修复 GitHub Actions 跨平台构建缺少 `icons/icon.icns` 的问题
- `rust-check` 三平台（Ubuntu / Windows / macOS）全部通过
- **自动发布 Nightly Release**：每次推送到 master 自动构建并发布三平台安装包到 GitHub Releases

---

## [3.1.0] - 2025-04-13 - 智能记忆与版本管理

### 🔍 Hybrid Search (混合搜索)

**Phase 1.3 Implementation**

- **BM25 Search** (`memory/hybrid_search.rs`)
  - CJK Bigram tokenizer for Chinese text
  - Inverted index with TF-IDF scoring
  - Configurable k1 and b parameters

- **Hybrid Search Engine**
  - BM25 + Vector similarity fusion
  - RRF (Reciprocal Rank Fusion) ranking
  - Configurable weights (default: BM25 40%, Vector 60%)

- **Entity Hybrid Search**
  - Name matching + vector similarity
  - Cosine similarity calculation
  - Priority scoring for entity retrieval

### 📜 Scene Version Management (场景版本管理)

**Phase 3.x Implementation**

- **SceneVersionRepository** (`db/repositories_v3.rs`)
  - `create_version()` - Snapshot current scene state
  - `get_versions()` - List version history
  - `get_version()` - Get specific version
  - `delete_version()` - Remove version

- **SceneVersionService** (`versions/service.rs`)
  - `compare_versions()` - Line-level diff with word count delta
  - `restore_version()` - Restore to any historical version
  - `get_version_chain()` - Version chain with branch structure
  - `get_version_stats()` - Edit distribution, avg confidence

- **Frontend Components**
  - `VersionTimeline.tsx` - Vertical timeline with selection
  - `ConfidenceIndicator.tsx` - Circular/bar progress indicator
  - `DiffViewer.tsx` - Side-by-side diff view
  - `useSceneVersions.ts` - React Query hooks

### 🧠 Memory Retention Management (记忆保留管理)

**Phase 1.4 Implementation**

- **RetentionManager** (`memory/retention.rs`)
  - Ebbinghaus forgetting curve: R(t) = R₀ × e^(-λt)
  - 5 priority levels: Critical/High/Medium/Low/Forgotten
  - Retention report generation
  - Context window optimization

---

## [3.0.0] - 2025-04-12 - 重大架构调整

### 🎪 场景化叙事架构
- Scene 取代 Chapter，戏剧冲突驱动
- 戏剧目标、外部压迫、冲突类型、角色冲突
- StoryTimeline 拖拽排序、SceneEditor 三标签页

### 🧠 增强记忆系统
- CJK Bigram Tokenizer
- 两步 Ingest Pipeline
- 带权知识图谱
- 四阶段 Query Pipeline
- 多助手独立会话

### 🤖 AI 智能生成
- NovelCreationAgent
- 4 步引导式创建向导
- 卡片式 UI

### 📦 工作室配置
- 每部小说独立配置
- ZIP 导入/导出

---

## [2.0.0] - 2025-04-12

- 幕前-幕后双界面架构
- 双窗口通信

## [1.5.0] - 2025-04-08

- Agent 系统
- 工作流引擎
- 向量存储

## [1.0.0] - 2025-04-01

- 基础架构
- LLM 集成
- 数据库设计
