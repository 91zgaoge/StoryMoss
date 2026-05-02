# StoryForge Agent 指南

> 本文件包含 AI 助手需要了解的项目背景、编码风格和工具配置

## 🧠 永久记忆：自动化测试助手

本项目已配置 **Playwright + Chromium** 无头浏览器自动化测试环境，专为 AI 助手设计。

### 快速启动测试

```bash
# 一键截图所有页面
npm run screenshot

# 截图幕前界面
npm run screenshot:front

# 截图幕后界面
npm run screenshot:back

# 运行完整测试
npm test
```

### 测试助手 API

文件位置：`e2e/test-helper.ts`

```typescript
import { runTest } from './e2e/test-helper';

runTest(async (helper) => {
  // 导航
  await helper.navigate('http://localhost:5173');
  
  // 截图
  await helper.screenshot('homepage');
  
  // 交互
  await helper.click('button');
  await helper.type('input[name="title"]', '测试标题');
  await helper.press('Enter');
  
  // 等待
  await helper.waitFor('.success-message');
  await helper.sleep(1000);
  
  // 执行 JS
  const title = await helper.eval<string>('document.title');
});
```

### 已配置的测试环境

| 组件 | 版本 | 路径 |
|------|------|------|
| Playwright | latest | `e2e/` |
| Chromium | 147.0.7727.15 | `C:\Users\admin\AppData\Local\ms-playwright\chromium-1217` |
| bunwv | 0.0.5 | 全局安装 (备用) |

### 测试文件位置

- 测试代码：`e2e/*.spec.ts`
- 测试截图：`e2e/screenshots/`
- 测试报告：`playwright-report/`
- 配置：`playwright.config.ts`

---

## 📋 项目背景

**StoryForge (草苔)** - AI 辅助小说创作桌面应用

- **版本**: v5.1.1
- **GitHub**: https://github.com/91zgaoge/StoryForge
- **技术栈**: Tauri 2.4 + Rust 1.94 + React 18 + TypeScript 5.8 + SQLite + Vitest

### 双界面架构

| 界面 | 用途 | URL |
|------|------|-----|
| 幕前 (Frontstage) | 沉浸式写作 | `/frontstage.html` |
| 幕后 (Backstage) | 工作室管理 | `/index.html` |

---

## 🎨 编码风格

### Rust 后端

- 使用 `snake_case` 命名
- 错误处理使用 `Result<T, E>`
- 异步函数使用 `async/await`
- 数据库使用 `rusqlite` + `r2d2` 连接池

### TypeScript 前端

- 使用 `camelCase` 命名
- 组件使用函数式组件 + Hooks
- 状态管理使用 Zustand
- API 调用使用 TanStack Query

### 提交信息格式

```
<type>: <subject>

<body>

type:
  feat: 新功能
  fix: 修复
  docs: 文档
  style: 格式
  refactor: 重构
  test: 测试
  chore: 构建
```

---

## 🔧 开发命令

```bash
# 启动前端开发服务器
cd src-frontend && npm run dev

# 启动 Tauri 应用
cd src-tauri && cargo tauri dev

# 构建生产版本
cd src-tauri && cargo tauri build

# 运行测试
npm test
```

---

## 📚 重要文档

- [ARCHITECTURE.md](./ARCHITECTURE.md) - 架构设计
- [TESTING.md](./TESTING.md) - 测试文档
- [CHANGELOG.md](./CHANGELOG.md) - 更新日志
- [ROADMAP.md](./ROADMAP.md) - 开发路线

---

### 最近完成的功能

- **v5.1.0 幕前幕后自动关联对齐** (2026-05-01) — 从"各自为战"到"自动联动"的全面升级
  - **Phase 1.1 Chapter↔Scene 双向映射**: `chapters.scene_id` + `scenes.chapter_id` 外键关联，ChapterRepository 自动创建/关联 Scene
  - **Phase 1.2 统一实时状态中心**: 后端 `state_sync` 模块 + 16 种 `SyncEvent`，所有数据修改命令自动发射同步事件
  - **Phase 1.3 Bootstrap 自动加载**: `smartExecute` 返回后前端自动加载新故事并切换到第一章，Bootstrap 完成后双重 `ChapterSwitch` 保险
  - **Phase 1.4 幕前→幕后快速跳转**: `Ctrl+Shift+B` 快捷键 + 标题栏点击，幕后自动定位当前故事
  - **Phase 2.2 自适应学习闭环修复**: `record_feedback` 成功后异步触发 `mine_preferences`，偏好挖掘自动激活
  - **Phase 2.4 AgentOrchestrator 闭环接入**: `execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 质检改写生效
  - **Phase 3.1 Zustand↔TanStack Query 同步**: `App.tsx` 监听 `currentStory` 变化，自动刷新关联数据缓存
  - **Phase 3.2 窗口通信事件标准化**: `DataRefresh` 统一由 `useSyncStore` 处理，消除重复刷新
  - **编译**: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v5.0.0 创世引擎：一键创世，万物关联** (2026-04-30) — 从"一键生成开头"到"一键生成完整小说世界"
  - **故事大纲自动生成**: `story_outlines` 表 + LLM 生成 3 幕结构大纲（标题/摘要/情节点/预估场景数）
  - **角色完整性格小传**: `characters` 表新增 appearance/gender/age，bootstrap 完整填充 personality/goals/appearance
  - **角色关系图谱**: `character_relationships` 表 + 前端"关系"标签页，展示角色间朋友/敌人/恋人/师徒等关联
  - **伏笔自动埋设**: Bootstrap 基于大纲识别 3-5 个核心伏笔，自动关联第一章场景
  - **知识图谱自动构建**: 创世时为角色/场景/伏笔自动创建 KG 实体和关系
  - **前后台智能联动**: Bootstrap 完成后自动发送 `NavigateTo` 事件，幕后切换到 Stories 并高亮新故事
  - **故事概览面板**: Stories.tsx 新增"概览"视图，展示大纲/角色/场景/伏笔总览
  - **7步创世工作流**: 构思故事 → 撰写开篇 → 构建世界 → 生成大纲 → 塑造角色 → 铺设场景 → 埋设伏笔 → 编织关联
  - **Migration 34/35/36**: story_outlines / characters增强+relationships / scenes.foreshadowing_ids
  - **Bug 修复 v3（热修复）**: 后台窗口白屏 + 后台卡片显示修复
    - **白屏根因**: WebView2 窗口 `hide()` 后重新 `show()` 时渲染表面可能丢失；JS 强制重排不够可靠
    - **白屏修复 v3**: `show_backstage` 命令**微调窗口大小再恢复**（`width+1` → `width`），强制 WebView2 重新创建渲染表面；配合 JS 强制重排；延迟 300ms 发射 `backstage-shown` 事件确保前端监听器就绪
    - **卡片不显示根因 v3**: (1) Bootstrap 完成时 backstage 被隐藏，事件丢失; (2) `DataLoader` 与 `App.tsx` 同时加载 stories 造成**竞态条件**; (3) Bootstrap LLM 调用失败时错误被 `log::warn` 吞掉，前端完全不可见
    - **卡片修复 v3**: `DataLoader` **移除 stories 查询**，完全由 `App.tsx` 控制数据加载，消除竞态 → `App.tsx` 引入 `useQueryClient`，`handleWindowShown` 中主动 `invalidateQueries` 强制刷新所有页面数据 → `bootstrap.rs` LLM 调用失败时发射 `novel-bootstrap-error` 事件到前端，让错误可见
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.5.0 进程提示栏超时深度修复：消灭"系统仍在处理中"黑洞** (2026-04-30) — 从"不知道在等什么"到"每一步都可见"
  - **根因定位**: `build_writer_prompt` 是同步函数，内部包含大量数据库查询 + `block_on` 调用，但**零事件输出**。用户在 0.15→0.20 之间等待 5-30 秒无反馈，前端 fallback timer 超时退出
  - **async 化**: `build_writer_prompt` → `async fn`，移除危险的 `tauri::async_runtime::block_on`（在 Tauri 异步运行时中可能导致死锁/线程阻塞）
  - **密集事件**: 在 `build_writer_prompt` 内部插入 15+ 个新事件（0.150→0.195），覆盖：策略配置→模板变量→系统提示词渲染→策略约束注入→方法论→风格 DNA→个性化偏好→叙事状态快照（故事/场景/冲突/伏笔/角色）→最终组装
  - **`tokio::task::yield_now().await`**: 每个子步骤之间 yield，确保事件循环有机会将 IPC 事件发送到前端
  - **AdaptiveGenerator 细分**: 0.281"查询用户反馈历史"、0.285"计算生成策略"
  - **前端图标映射**: 补充"读取/渲染/准备/查询/计算"→Brain，"注入/组装"→Cog
  - **状态栏宽度**: `generation-status-text` max-width 200px→600px + `flex-shrink: 0`；最终移到输入框 pill 下方独立行 `generation-status-row`，占满 900px 宽度
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.4.0 3风格三角框架：通用风格混合系统** (2026-04-28) — 从单一风格到多风格融合的创作革命
  - **通用风格混合系统**: `StyleBlendConfig` 支持任意 2-5 个 StyleDNA 按权重组合，主导/辅助角色自动分配，权重实时归一化
  - **3风格三角创作框架**: 新增普鲁斯特（意识流/长句/内心独白70%）+ 马尔克斯（魔幻现实/全知视角）内置风格，与现有海明威形成完整三角
  - **混合风格 Prompt 注入**: 主导风格完整注入，辅助风格仅注入关键差异维度；融合规则明确"主导定基调，辅助渗精神"
  - **防漂移自检清单**: 5项检查（句长/对话比/比喻密度/内心独白/情感外露），加权平均目标 ± 容差，总体匹配度评分
  - **章节级风格控制**: `scenes.style_blend_override` 支持每章独立配置，前端 Stories.tsx 双标签页（单一风格/风格混合）
  - **数据层**: Migration 30/31 新增 `story_style_configs` 表 + `scenes` 覆盖字段；4 个新 IPC 命令
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

- **v4.3.0 智能交互创作流程深度优化** (2026-04-27) — 从"能创作"到"懂创作"的全面升级
  - **一键创作体验升级**: Bootstrap前端实时显示5步进度（构思→世界观→角色→场景→撰写）；创建完成后自动切换新故事并加载第一章；Chapter/Scene双轨同步确保前端零延迟加载
  - **模型驱动编排全面落地**: 彻底移除`detect_and_route_intent`关键词匹配，所有用户输入交由PlanGenerator自由理解；PlanContext增强注入世界观摘要、角色列表、活跃伏笔、风格DNA、MCP可用工具
  - **设定修改智能响应**: 新增`update_character`/`update_world_building`/`update_scene`能力，LLM解析用户修改意图自动更新后台设定；场景修改自动标记`needs_rewrite`，续写时自动重写受影响内容
  - **MCP与技能自动化**: CapabilityRegistry注册MCP工具，PlanGenerator知道何时调用外部工具；内置技能（style_enhancer/character_voice/emotion_pacing）可由模型自主编排
  - **PlanGenerator Prompt进化**: 新增技能调用指南、设定修改指南、MCP工具使用指南、伏笔处理指南（Rule 12-18）
  - 编译: `cargo check` 零错误零警告，`cargo test` 183/183，`npm run build` 通过
  - 新增测试: planner/bootstrap 7个（JSON提取/概念序列化）、planner/executor 4个（参数解析）、planner/mod 4个（PlanContext/PlanStep）
  - 修复: bootstrap.rs 编译警告、第一章 prompt 增强（注入题材/基调/简介）

- **v4.2.0 智能交互设计重构 V2：模型驱动的编排范式** (2026-04-23) — 从程序式编排转向模型式编排
  - **核心理念**: 人类只定义能力能做什么（自然语言描述），模型负责编排（什么时候用、怎么用、按什么顺序）。移除所有关键词匹配、意图分类枚举、if/else 分支判断用户意图。
  - **CapabilityRegistry（能力自描述系统）**: Agent 和 Skill 用自然语言描述自己（`description` / `when_to_use` / `input_description` / `output_description`），模型阅读描述自主选择。人类不再写死 Agent 映射规则。
  - **PlanGenerator（模型计划生成器）**: 取代 IntentParser + IntentExecutor。LLM 接收系统状态 + 用户输入 + 能力清单，自主输出执行计划（自由文本理解 + 步骤列表 + 参数 + 依赖关系）。
  - **PlanExecutor（计划执行引擎）**: Dumb executor，忠实执行 LLM 生成的计划。按顺序执行步骤、传递输出、处理失败。所有决策已在计划中。
  - **PromptEvolver（提示词进化器）**: LLM 根据故事上下文（题材、叙事阶段、用户偏好）自由改写整个 prompt。不是模板变量替换，而是真正的"进化"。
  - **AiLearningIndicator（记忆显性化）**: 前端组件，每次 AI 交互后展示"系统学到了什么"。让"越写越懂"对用户可见。
  - **CapabilityEvolutionEngine（能力进化反馈环）**: 记录能力调用结果，长期优化能力描述准确性。
  - **PlanTemplateLibrary（计划模板学习）**: 记录成功执行计划，类似请求复用或微调。
  - **移除的程序式规则**: IntentType 枚举（11 类预设分类）、前端正则关键词检测、IntentExecutor.map_agents 写死映射、`if (!currentStory)` 强制报错流程。
  - **前端简化**: `handleSmartGeneration` / `handleRequestGeneration` 统一走 `smart_execute`，用户任何输入都交给模型决定。
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.1.0 幕前界面深度重构：化整为零，萤火随行** (2026-04-22) — P0+P1+P2 全流程体验重构
  - **设计理念**: 从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。"创作者不应在工具中花费精力标注自己的创作"——移除所有显式注释/评论创建 UI。
  - **P0 核心重构 (4 项)**:
    - 顶栏精简: 44px 细线设计，小说标题（点击进入幕后）、字数统计、字号调节、🔥 文思三态切换（off·/passive✨/active🔥）、禅模式。移除汉堡菜单、订阅徽章、"开启文思"按钮、"AI 续写"按钮、主行动按钮。
    - 底栏删除: 彻底删除底部聊天工具栏（chat input、模型状态点、WenSiPanel 嵌入、Slash textarea 菜单）。AI 结果以幽灵文本（ghost text）内联呈现，Tab 接受/Esc 拒绝。
    - 侧边栏精简: 5 按钮→2 按钮：修（修订模式）/ 批（生成古典评点）/ 幕（幕后）。移除注释和评论显式 UI。
    - 键盘快捷键: `Ctrl+Enter` / `Cmd+Enter` 全局触发续写，`Ctrl+Space` 循环文思模式，`F11` 禅模式。
  - **P1 萤火系统 (3 项)**:
    - 幽灵文本: 编辑器末尾灰色斜体段落（`opacity: 0.35`），附带萤火操作栏（Tab 接受 / Esc 拒绝）。
    - 右边缘萤火: `smartGhostText` 从右侧淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流。
    - 空态引导: 编辑器无内容时居中显示诗意提示"开始写下第一句话，文思将随你而行"。
  - **P2 体验优化 (4 项)**:
    - 内联 `/` 命令菜单: 8 命令（续写/润色/古风/场景/自动续写/审校/评点/排版），光标处触发，方向键导航，回车执行，Esc 关闭，自动删除 `/` 字符。
    - WenSiPanel 浮动化: 从底栏嵌入改为 FrontstageApp 右下角浮动卡片，通过 `/` 菜单高级命令触发。
    - 修订横幅精简: 从多行可展开缩减为 32px 单行，变更列表可滚动，默认折叠。
    - 古典评点保留: AI 生成的段落评点（金圣叹式朱批）保留为内联段落，朱红色 `oklch(55% 0.18 25)`，霞鹜文楷字体，左边框红色，※ 前缀，缩进 3em。
  - **移除（设计决策）**:
    - 显式注释系统: sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单项、所有相关 hooks（`useTextAnnotations`、`useCommentThreads`）。
    - 原因: AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现。
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.0.1 全面代码审计与空实现修复** (2026-04-22) — Phase A+B
  - **Phase A: 代码审计与 P0 修复 (15+ 项)**:
    - 综合审计: 扫描 40+ 模块，输出 `CODE_AUDIT_REPORT_V4.md`（5 严重/17 参数/9 空实现）
    - IPC: 统一 17 处 camelCase→snake_case 参数名，修复 Tauri v2 反序列化静默失败
    - 空实现补全: `analytics` 真实统计、`agents/commands` 真实状态、`skills/executor` 真实 MCP 调用、`export/import_from_text` 正则解析、`workflow/scheduler` 执行日志、`evolution/updater` manifest CRUD、`mcp/server` 缺失 `.await`
    - 前端修复: `settings.ts` 移除硬编码密钥、`useCollaboration.ts` WebSocket 真实发送、`useStreamingGeneration.ts` 移除 mock、`textAnalyzer.ts` 增量分析
    - UI: 聊天工具栏从 absolute 改为正常流、编辑器 padding 优化
    - 类型统一: `skills/mod.rs` 移除重复 `McpServerConfig`
  - **Phase B: 内存模块 SQLite 持久化 (3 模块)**:
    - Migration 26/27/28: `chat_sessions`/`chat_messages`、`story_runtime_states`、`collab_sessions`/`collab_participants`
    - `chat/mod.rs`: `ChatManager` 改为 `DbPool` 持久化
    - `state/manager.rs`: `StoryStateManager` 改为 `DbPool` 持久化
    - `collab/mod.rs` + `websocket.rs`: `CollabManager` 持久化 + 完整消息处理闭环（Join/Leave/Operation/Cursor/Participants）
  - 编译: `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过

- **v4.0.0 借鉴 AI-Novel-Writing-Assistant 全面优化** (2026-04-22) — Phase 1+2+3 共 9 项新功能
  - **Phase 1: P0 核心能力 (3 项)**:
    - Canonical State: 新增规范状态系统，统一聚合 StoryContextBuilder/character_states/foreshadowing/KG 等分散状态，AI 续写时准确知道"当前处于故事哪个阶段"
    - Payoff Ledger: 升级 ForeshadowingTracker 为伏笔账本，新增时间窗口追踪(target_start/target_end)、逾期检测、风险信号、回收时机智能推荐
    - Execution Panel: 新增章节执行面板，智能推荐下一步行动（"处理逾期伏笔"/"续写"/"运行审校"），集成到 Scenes.tsx 和 FrontstageApp
  - **Phase 2: P1 质量与控制 (3 项)**:
    - Narrative Phase Detection: 增强叙事阶段检测（逾期伏笔→ConflictActive、高置信度长内容→Climax、主要伏笔回收→Resolution），注入 Writer prompt
    - Structured Outline: Scene 模型新增 execution_stage/outline_content/draft_content，SceneEditor 重写为 6 标签页（规划/大纲/起草/审校/定稿/批注）
    - Audit System: 新增统一审计模块，整合 ContinuityEngine/StyleChecker/QualityChecker/PayoffLedger，五维评分（连续性/人物/风格/节奏/伏笔），支持 light/full 审计
  - **Phase 3: P2 体验优化 (3 项)**:
    - Novel Creation Wizard: 新增 5 步小说创建向导（创意→世界观→角色→文风→首个场景），每步提供 AI 生成选项
    - Enhanced Streaming: StreamOutput 组件增强（Markdown 渲染、实时字数、停止按钮、打字机效果），接入 FrontstageApp/WenSiPanel/CreationWizard
    - Strategy Configuration: Settings 新增写作策略配置（运行模式/冲突强度/叙事节奏/AI 自由度），动态注入 Writer prompt
  - 编译: `cargo check` 零错误，`cargo test` 160/160，`npm run build` 通过

- **v3.7.1 智能化创作系统 5 阶段重构深度修复** (2026-04-22) — Phase A+B+C 共 15 项修复
  - **Phase A: P0 核心断裂修复 (5 项)**:
    - QueryPipeline: `graph_expansion` 内容分词后逐 token 匹配实体，修复图谱扩展永不命中的 bug
    - QueryPipeline: `budget_control` 修复内层 break 只跳出内层循环的预算泄漏 bug
    - ContinuityEngine: `check_world_rules` 修复检查方向——从"检测规则描述片段"改为"提取禁止条款后检测"
    - ContinuityEngine: `get_character_states` 效率优化（O(N×M)→O(N+M)），`check_character_locations` 增强跨场景位置检测
    - PreferenceMiner: `record_feedback` 成功后异步触发 `mine_preferences`，自适应学习闭环激活
    - StyleChecker: 接入 `AgentOrchestrator` 闭环，Writer→Inspector→StyleChecker→Writer 风格校验生效
    - Ingestion: 实现真正的内容保存（Chapter 创建/更新）+ 简化知识图谱实体提取，工作流闭环完成
  - **Phase B: P1 功能补全 (6 项)**:
    - 方法论: Migration 22 添加 `methodology_id`/`methodology_step`，Settings 页面新增创作方法论配置
    - 创作模式: `CreationWorkflowEngine` 按 `CreationMode` 分支（AI全自动/AI初稿+精修/人工初稿+润色）
    - 进度反馈: 前端 `useWorkflowProgress` Hook + Stories.tsx 进度弹窗（阶段名称+百分比+指示器）
    - Orchestrator 事件: 前端监听 `orchestrator-step` 实时状态（生成→质检→改写），Settings 暴露阈值/循环数配置
    - AdaptiveGenerator: `calculate_temperature` 累加而非覆盖，pacing/style 偏好微调生效
    - 反馈记录: AiSuggestionNode + WenSiPanel 接入 `record_feedback`，覆盖内联建议/自动续写/自动修改
  - **Phase C: P2 优化 (4 项)**:
    - StyleAnalyzer: 新增 `analyze_with_llm` + `analyze_style_sample` IPC，Stories.tsx 新增"从文本生成风格"
    - QualityChecker: 新增 `check_with_llm`，Review 阶段优先 LLM 评估、回退规则评估
    - PhaseWorkflow: 硬编码阶段逻辑迁移到配置驱动，`PhaseWorkflow` 配置系统激活
    - 增量 Context: 每阶段完成后关键产出回注 `AgentContext`（Conception→world_rules, Outlining→scene_structure）
  - 编译: `cargo check` 零错误，`cargo test` 145/145，`npm run build` 通过

- **v3.6.1 全面功能审计与深度修复** (2026-04-22) — P0+P1+P2 共 30 项修复
  - **P0 紧急修复 (10 项)**:
    - DB: Migration 21 补全 scenes/kg_relations `confidence_score` 缺失列，消除运行时崩溃
    - IPC: 统一 25 处 camelCase→snake_case 参数名，修复 Tauri v2 反序列化失败
    - 场景: `create_scene` 后端扩展参数，前端传参不再静默丢弃
    - Orchestrator: 修复 Rewrite 事件错误携带初稿分数的 bug (`writer_result.score` → `rewrite_result.score`)
    - 技能: `execute_skill` 注入真实 `StoryContext`，`SkillExecutor` 实现真正 LLM 调用
    - 自适应学习: FrontstageApp accept/reject 接入 `record_feedback`，FeedbackRecorder 数据源激活
    - 审计: `LlmService::generate` 完成后调用 `log_ai_usage`，AI 调用日志写入数据库
    - 配额: auto_write/auto_revise 错误处理识别配额关键字，触发 Toast 提示
  - **P1 功能补全 (8 项)**:
    - ContinuityEngine: 补全 timeline + character_emotion + relationship 检查，5/5 全部实现
    - 一键创作: `CreationWorkflowEngine` 每阶段发射 `workflow-progress` 事件 + QualityReport 填充
    - SceneRepository: 新增 5 个单元测试（create/get/update/delete/reorder），Rust 测试 139→144
    - hooks/index.ts: 补全 `useCommentThreads` 等 6 个 Hook 导出
    - 类型: `ChangeTrack.scene_id` 改为 `string | undefined`，与后端 `Option<String>` 对齐
    - 评论: RichTextEditor 已解决评论支持「重新打开」
    - 变更追踪: 修订模式增加单条 change 独立接受/拒绝按钮
    - 清理: 移除弃用 `check_ai_quota` IPC 注册
  - **P2 优化 (6 项)**:
    - 概念统一: Sidebar `chapter_count` 显示从"场景"改为"章"
    - 滑块: SceneEditor 置信度 `step` 从 0.05 改为 0.1
    - 拆书转故事: 人物 background 合并 personality + appearance，场景 summary 保存为 content
    - 伏笔看板: 幕后新增 Foreshadowing 页面，支持 setup/payoff/abandoned 状态管理
    - 技能 Hook: 6 个关键业务点（create_chapter/character/scene、AI write、world_building update）激活 Hook 调用
    - 孤儿表: 评估 `world_rules`/`settings`/`character_states`，保留兼容
  - 编译: `cargo check` 零错误，`cargo test` 144/144，`npm run build` 通过

- **v3.5.2 全功能落地：剩余 7 项修复完成** (2026-04-22)
  - #17 auto_revise 取消/进度事件：后台任务模式 + 4 阶段进度 + 取消支持
  - #20 confidence_score：Scene 类型补全 + SceneEditor 置信度滑块
  - #16 MCP 持久连接：全局连接池 + disconnect/get_connections + DuckDuckGo 真实搜索
  - #19 一键创作按钮：Stories 页面入口 + run_creation_workflow 调用
  - #18 StyleDNA UI：stories 表 style_dna_id + 前端选择模态框 + 创作注入
  - #15 技能系统补全：execute_skill 异步 LLM 调用 + 2 个缺失技能（角色声音/情感节奏）
  - #14 意图引擎接入：RichTextEditor 聊天栏 parseIntent → 路由 → executeIntent
  - 139 Rust tests + 前端构建全部通过，版本号统一 3.5.2

- **v3.5.1 全面功能审计与修复** (2026-04-22) — 13 项关键修复
  - 自动修改: 结果应用到编辑器 + 保存到数据库
  - 拆书: 书名/作者持久化、convert_to_story story_id 修复、store_embeddings、进度 100%、心跳闪烁修复
  - 场景模型: scene_versions 表生产环境补建、conflict_type 列索引修复、版本快照全字段检测
  - AI 核心: AgentOrchestrator 闭环集成、ContinuityEngine/ForeshadowingTracker 写作流集成、AdaptiveGenerator 动态参数应用、auto_write Ingest 触发
  - Inspector: JSON 结构化输出 + 三层解析增强
  - LLM: 取消机制实现、useLlmStream 真实流式
  - StyleDNA: 内置风格自动种子化、CreationWorkflowEngine 暴露命令
  - 测试: Rust 139 全部通过，前端构建通过，已推送 GitHub

- **v3.5.0 拆书体验升级** (2026-04-21) — 进度提示 + 取消支持
  - 后端: `BookAnalyzer` 5 步 Pipeline 每个子步骤发送详细进度，人物/章节逐块汇报
  - 前端: `AnalysisProgress` 8 步骤指示器 + 百分比 + 块处理信息，告别"只见转圈"
  - 取消: `TaskExecutionContext.is_cancelled()` + analyzer 循环检查 + `cancel_book_analysis` IPC
  - 数据库: `reference_books` 新增 `task_id` 字段 + Migration 18
  - 测试: Rust 139 全部通过，前端构建通过

- **v3.4.0 智能化创作系统** (2026-04-18) — 5 阶段重构
  - Phase 1 地基重构: `StoryContextBuilder` 真实 DB 上下文, `QueryPipeline` 四阶段检索, `ContinuityEngine`, `ForeshadowingTracker` — 27 tests ✅
  - Phase 2 方法论注入: 雪花法/场景节拍/英雄之旅/人物深度 + `MethodologyEngine` + `AgentOrchestrator`(Writer→Inspector 闭环) — 34 tests ✅
  - Phase 3 风格深度化: `StyleDNA` 六维模型, `StyleAnalyzer`, `StyleChecker`, 10 经典作家 DNA, `StyleDnaRepository` — 45 tests ✅
  - Phase 4 自适应学习: `FeedbackRecorder`, `PreferenceMiner`(5 维启发式), `AdaptiveGenerator`(动态 temperature/top-p), `PromptPersonalizer` — 54 tests ✅
  - Phase 5 工作流闭环: `CreationWorkflowEngine`(7 阶段), `QualityChecker`(4 维评估) — 63 tests ✅
  - 版本号统一 3.3.0→3.4.0，Logo 生成全平台图标包

- **Freemium 付费系统** (2026-04-18)
  - 后端: `subscriptions`/`ai_usage_quota`/`ai_usage_logs` 表 + `SubscriptionService` + Tauri IPC 命令
  - 前端: `useSubscription` Hook + `SubscriptionStatus` 指示器 + `UpgradePanel` 付费引导 + 配额用尽提示
  - 策略: "分析免费，修改收费" — 免费用户看提示，Pro 用户享内联改写 + 风格 DNA + 方法论
  - Agent 分层: 免费版 max_tokens 1000 + 简化 prompt；专业版完整能力
  - 优化: 原子扣减 / 成功后扣费 / session 冷却 / 离线缓存 / 防抖修复 — 9 项

- **幕前排版与 AI 续写优化** (2026-04-17)
  - 段落间距收紧 + 首行缩进 2em，底部栏 padding-bottom 增至 10rem
  - 自动续写：接受 AI 生成后自动触发下一轮续写
  - Zen 模式绝对纯净：隐藏所有 AI UI 元素

- **TaskService 全局共享修复 + 集成测试建设** (2026-04-19)
  - 关键 Bug: `TaskService` 未全局共享 → 每个 command 新建实例 → `BookDeconstructionExecutor` 丢失 → 拆书功能不可用
  - 修复: `TaskService<R: Runtime>` 泛型化 + 手动 `Clone` + `app.manage(task_service)` + `State<'_, TaskService>`
  - 缓存修复: `useSetActiveModel` `invalidateQueries({ queryKey: ['settings'] })`
  - 单元测试: Rust 71 新增（settings 16 + task_system 13 + repositories 14 + validation 20）+ 前端 21 新增
  - 集成测试: Rust 5 新增（executor registry 共享、任务生命周期、调度器、无执行器失败、拆书去重）
  - 测试总计: Rust 139 + 前端 21 = 160 tests 全部通过

- **拆书功能 + 任务系统 + 向量化存储** (2026-04-19)
  - 后端: `book_deconstruction` 模块 — parser/chunker/analyzer/repository/service/commands
  - 前端: `BookDeconstruction` 页面 + 6 个子组件 + `useBookDeconstruction` Hooks
  - 任务系统: `task_system` 模块 — models/repository/scheduler/heartbeat/executor/service/commands (8 IPC 命令)
  - 拆书改为 `BookDeconstructionExecutor` 任务执行，心跳保活 + 进度推送
  - 向量化: 场景/人物 embedding 自动生成并入库 LanceVectorStore
  - 数据库: 5 张新表 (tasks + task_logs + reference_books + reference_characters + reference_scenes) + 9 个索引 + Migration 16/17

- **拆书功能** (2026-04-19)
  - 后端: `book_deconstruction` 模块 — parser/chunker/analyzer/repository/service/commands
  - 前端: `BookDeconstruction` 页面 + 6 个子组件 + `useBookDeconstruction` Hooks
  - 支持 txt/pdf/epub 解析，三层 LLM 分块分析策略，生成小说类型/世界观/人物/章节/故事线
  - 一键转为故事项目，参考素材库独立存储，向量化接口预留
  - 新增 3 张数据库表 + 4 个索引 + Migration 16，6 个单元测试

- **任务系统 + 拆书改任务 + 向量化存储** (2026-04-19)
  - 后端: `task_system` 模块 — models/repository/scheduler/heartbeat/executor/service/commands (8 IPC 命令)
  - 前端: `Tasks` 页面 + `useTasks` Hooks，状态分组/心跳指示器/进度条/执行日志
  - tokio::time 调度器支持 once/daily/weekly/cron，每任务互斥锁防重叠，心跳检测60秒扫描
  - 拆书分析改为 `BookDeconstructionExecutor` 任务执行，每步分析后心跳保活
  - 向量化存储接入 LanceVectorStore：场景/人物 embedding 自动生成并入库
  - 新增 2 张数据库表 (tasks + task_logs) + 5 个索引 + Migration 17

### 编译状态

- `cargo check` ✅ | 警告: 0
- `cargo check --release` ✅ | 警告: 0
- `npm run build` ✅
- `cargo test` ✅ 193/193

---

## [v5.1.0] - 幕前幕后自动关联对齐

### 核心升级
- **Chapter↔Scene 双向映射**: 自动关联，幕前切换章节同步切换场景
- **统一实时状态中心**: 所有数据修改自动同步，前后台零延迟对齐
- **Bootstrap 自动加载**: 创世完成后幕前自动加载新故事并切换到第一章
- **AgentOrchestrator 闭环**: Writer 生成后自动质检→风格检查→改写

### 技术细节
- Migration 37: `chapters.scene_id` + `scenes.chapter_id`
- 后端 `state_sync` 模块: `SyncEvent` 枚举 + `StateSync` 发射器
- 前端 `useSyncStore` Hook: 监听 `sync-event`，自动 `invalidateQueries`
- `show_backstage` 接收 `story_id` 参数，自动导航定位

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ 193/193
- `npm run build` ✅

---

## [v5.0.0] - 创世引擎：一键创世，万物关联

### 核心升级
- **一键生成完整小说世界**: 输入"写一部都市玄幻小说"，自动生成故事概念 + 第一章正文 + 完整大纲 + 角色性格小传 + 场景规划 + 伏笔埋设
- **7步创世工作流**: 构思 → 开篇 → 世界 → 大纲 → 角色 → 场景 → 伏笔 → 关联
- **自动幕后卡片创建**: 所有生成内容自动在幕后对应栏目创建卡片

### 新增功能
- **故事大纲系统**: `story_outlines` 表 + 3幕结构自动生成 + 前端概览面板
- **角色系统增强**: appearance/gender/age 字段 + 角色关系图谱
- **伏笔自动生成**: Bootstrap 自动埋设 3-5 个核心伏笔
- **知识图谱自动构建**: 角色/场景/伏笔自动创建 KG 实体
- **前后台智能联动**: 完成后自动导航到 Stories 并高亮新故事

### Bug 修复
- **后台窗口白屏修复**: 隐藏后重新显示时出现空白/白屏
- **后台卡片显示修复**: Bootstrap 后大纲/角色/场景/伏笔卡片不显示
- **根因**: (1) 后台隐藏时无法接收事件 (2) 前后台独立 Zustand store 未同步 (3) 页面未监听 DataRefresh (4) 无自动加载
- **修复**: App.tsx 自动加载 + FrontstageApp 通知同步 + 页面监听 backstage-data-refreshed 并 invalidate queries

### 数据库迁移
- Migration 34: `story_outlines` 表
- Migration 35: `characters` 增强 + `character_relationships` 表
- Migration 36: `scenes.foreshadowing_ids`

### 编译状态
- `cargo check` ✅ 零错误
- `cargo test` ✅ 193/193
- `npm run build` ✅
- `cargo tauri build` ✅ — Windows `.exe` (36MB) + `.msi` (14MB) + `-setup.exe` (10MB) 已生成

---

*最后更新: 2026-05-01 - v5.1.1 设计-实现对齐全面修复*

- **Bug Fix v4** (commit `70a8851`): 根因定位 — `TanStack Query` `['stories']` 缓存未被 invalidates
  - `index.html` 添加加载指示器，防止 React 挂载前被误认为白屏
  - `App.tsx` `DataRefresh` 事件处理添加 `invalidateQueries(['stories'])`，Bootstrap 完成后自动刷新故事列表
  - `App.tsx` `handleWindowShown` 添加 `invalidateQueries(['stories'])`，窗口重新可见时刷新数据
  - 编译: `cargo check` 零错误，`cargo test` 193/193，`npm run build` 通过

---

### 🏗️ 永久构建规则（用户强制要求）

> **每次修改代码后，先推送到 GitHub，触发 GitHub Actions 全平台构建。**
> **推送完成后，在本地执行构建并打包生成 Windows `.exe` / `.msi` 安装包。**
> **每次推送到 GitHub，都必须逐条更新 GitHub 项目的 `README.md` 文件内容。包括但不限于：功能列表、版本号、截图、应用图标、安装说明、使用指南等所有相关信息。**
> **Git tag、Cargo.toml、tauri.conf.json、package.json 中的版本号必须保持统一。**

> **⚠️ README.md 更新检查清单（推送前必做）：**
> - [ ] 版本号是否与当前 tag 一致
> - [ ] 功能列表是否包含本次新增/修改的功能
> - [ ] 截图是否更新为最新 UI（幕前 + 幕后）
> - [ ] 应用图标/Logo 是否为最新版本
> - [ ] 安装说明是否需要调整
> - [ ] 使用指南是否反映最新交互方式
> - [ ] CHANGELOG 链接是否有效

> **⚠️ 代码更新后必做（永久记住）：**
> - [ ] **重新构建 Windows `.exe`** — 任何前端代码（JS/CSS/TSX）或后端代码（Rust）修改后，必须执行 `cargo tauri build` 重新生成安装包，并复制产物到项目根目录
> - [ ] 验证 `StoryForge.exe`、`StoryForge_latest.exe`、`.msi`、`-setup.exe` 修改时间是否最新

> **🧠 AI 创作工具交互设计原则（永久记住）：**
> - **智能判断用户意图，主动调整状态** — 不要像传统软件一样弹出对话框让用户手工操作。例如：用户输入"写一篇小说"但无章节时，应**自动创建第一章**而非提示"请先选择章节"；文思模式非 active 时应**自动切换**而非提示用户按键。
> - **减少用户操作步骤** — AI 工具的核心价值是智能代理，用户给出意图后，工具应自动完成所有必要的配置和准备工作。
> - **避免非智能的传统软件式交互** — 不要用 toast/dialog/alert 来要求用户做本应由 AI 自动完成的事情。错误提示只用于真正无法自动处理的情况（如网络断开、API 密钥缺失）。

> **🌿 「越写越懂」核心理念（永久记住）：**
> StoryForge 不是简单的文本生成器，而是一个**理解用户意图并智能化调用全套创作工具**的 AI 导演式创作系统。
> - **用户输入 = 意图，不是命令** — 用户的每一句话都应被模型理解意图，模型自主决定：是续写？润色？调用技能？调用 MCP 工具搜索资料？还是调整故事结构？
> - **模型主动调用技能和 MCP** — 当用户说"写一个关于赛博朋克的打斗场景"，模型不仅生成文字，还应自动：调用世界观技能补充设定、调用风格 DNA 匹配文风、调用 MCP 搜索赛博朋克相关资料。
> - **越写越懂 = 上下文深度理解** — 随着写作进行，模型持续学习：角色关系图谱、伏笔回收状态、叙事阶段检测、用户偏好反馈。每一次输入都让模型对故事的理解更深一层。
> - **幕前是导演椅，不是打字机** — 用户坐在幕前，像在导演椅上发号施令。AI 负责调度所有创作资源（知识图谱、技能工坊、MCP 外部工具、StyleDNA、方法论引擎），用户只需表达意图。
> - **自适应进化** — 系统持续记忆用户习惯，智能修改技能提示词来改进技能效果，修改 Agent Bot 写作助手的提示词来改进写作风格，动态调优以更好地满足用户需求。这不是静态配置，而是持续学习的智能体。

**本地构建脚本**: `scripts/build-local.ps1`
```powershell
# 推送前必执行：生成本地 Windows 安装包
.\scripts\build-local.ps1 -Windows
```
```powershell
# Windows 本地构建
.\scripts\build-local.ps1

# 或指定平台
.\scripts\build-local.ps1 -Windows
.\scripts\build-local.ps1 -All
```

**构建产物位置**（执行 `cargo tauri build` 后）：
```
target/x86_64-pc-windows-msvc/release/
├── storyforge.exe                          ← 30MB+，可直接运行
└── bundle/
    ├── msi/StoryForge_3.4.0_x64_en-US.msi  ← MSI安装包
    └── nsis/StoryForge_3.4.0_x64-setup.exe ← NSIS安装程序
```
> 为方便取用，每次构建后应将产物复制到项目根目录：`StoryForge.exe` 和 `StoryForge_3.4.0_x64-setup.exe`

**现实限制**:
- Windows 主机 ✅ 可本地构建 Windows (.msi/.exe)
- Linux 主机 ⚠️ 需 WSL 或 Linux 虚拟机
- macOS 主机 ❌ 无法在 Windows 上本地构建（需 macOS + Xcode）
- 跨平台完整构建 → 交由 GitHub Actions (`ubuntu-latest` / `windows-latest` / `macos-latest`)

---

## 🏛️ Spec-Kit 集成 (Spec-Driven Development)

本项目已集成 **GitHub Spec-Kit**，使用 Spec-Driven Development (SDD) 方法论管理功能开发。

### Spec-Kit 技能命令

在 Kimi Code 中使用以下 `/skill:` 命令：

| 命令 | 用途 | 阶段 |
|------|------|------|
| `/skill:speckit-constitution` | 查看/更新项目宪法 |  anytime |
| `/skill:speckit-specify` | 创建功能规格说明 | Phase 1 |
| `/skill:speckit-plan` | 生成技术实现计划 | Phase 2 |
| `/skill:speckit-tasks` | 分解为可执行任务 | Phase 3 |
| `/skill:speckit-implement` | 执行实现 | Phase 4 |
| `/skill:speckit-clarify` | 澄清需求模糊点 | Optional |
| `/skill:speckit-analyze` | 跨工件一致性检查 | Optional |
| `/skill:speckit-checklist` | 生成质量检查清单 | Optional |

### 文件结构

```
.specify/
├── memory/
│   └── constitution.md      # 项目宪法
├── templates/
│   ├── constitution-template.md
│   ├── spec-template.md
│   ├── plan-template.md
│   ├── tasks-template.md
│   └── checklist-template.md
├── scripts/
│   └── powershell/          # PowerShell 工作流脚本
│       ├── check-prerequisites.ps1
│       ├── create-new-feature.ps1
│       └── setup-plan.ps1
├── workflows/
│   └── speckit/
│       └── workflow.yml     # 完整 SDD 工作流定义
├── init-options.json
└── integration.json

.kimi/
└── skills/                  # Kimi Code 技能文件
    ├── speckit-constitution/SKILL.md
    ├── speckit-specify/SKILL.md
    ├── speckit-plan/SKILL.md
    ├── speckit-tasks/SKILL.md
    ├── speckit-implement/SKILL.md
    └── ...

specs/                       # 功能规格目录（按功能分支组织）
└── NNN-feature-name/
    ├── spec.md              # 功能规格
    ├── plan.md              # 实现计划
    ├── tasks.md             # 任务列表
    ├── checklists/
    │   └── requirements.md  # 质量检查清单
    ├── research.md          # 技术研究 (可选)
    ├── data-model.md        # 数据模型 (可选)
    └── contracts/           # 接口契约 (可选)
```

### 快速开始一个新功能

```powershell
# 1. 创建新功能分支和规格目录
.specify/scripts/powershell/create-new-feature.ps1 '功能描述'

# 2. 在 Kimi Code 中执行
/skill:speckit-specify 功能描述...
/skill:speckit-plan
/skill:speckit-tasks
/skill:speckit-implement
```

### 配置

- **AI 助手**: kimi (Kimi Code CLI)
- **脚本类型**: PowerShell (ps)
- **分支编号**: sequential (001, 002, ...)
- **项目宪法**: `.specify/memory/constitution.md`

---

*最后更新: 2026-04-17 - Spec-Kit 集成完成，项目宪法已建立，版本号统一为 3.3.0*
