# Changelog

All notable changes to StoryForge (草苔) project will be documented in this file.

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
