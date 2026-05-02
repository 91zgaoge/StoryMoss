<p align="center">
  <img src="docs/images/logo.png" alt="StoryForge 草苔" width="120" />
</p>

# StoryForge (草苔) v5.1.1 - AI 导演式小说创作系统

> 🌿 越写越懂的 AI 小说创作系统 — Tauri + Rust + React 驱动的桌面写作软件
>
> 专为小说作者打造的**导演式创作工作台**：知识图谱可视化、伏笔追踪与回收、StyleDNA 风格引擎、多人协同编辑、7 阶段全自动创作工作流。让 AI 成为你的创作搭档，越写越懂你。
>
> **v5.1.0 最新更新**：幕前幕后自动关联对齐 — 从"各自为战"到"自动联动"。Chapter↔Scene 双向映射、统一实时状态中心（所有数据修改自动同步）、Bootstrap 完成后幕前自动加载并切换第一章、Ctrl+Shift+B 一键回幕后并定位当前故事、AgentOrchestrator 闭环接入（Writer→Inspector→StyleChecker→Writer 自动质检改写）、自适应学习闭环激活、Zustand↔TanStack Query 状态同步、窗口通信事件标准化。
>
> **v5.0.0 更新**：创世引擎 — 一键创世，万物关联。输入一句话，系统自动在幕后所有对应栏目中创建完整关联卡片。新增故事大纲自动生成、角色完整性格小传入库、角色关系图谱、伏笔自动埋设、知识图谱自动构建、前后台智能联动导航。
>
> **v4.3.0 最新更新**：智能交互创作流程深度优化 — 从"能创作"到"懂创作"。输入"写一稿都市玄幻小说"等创建意图后，**系统直接生成小说正文开头并以 ghost text 展示**，同时在后台自动完成 5 步初始化（故事概念→世界观→角色→场景→第一章），无需等待即可看到正文；PlanGenerator全面模型化，彻底移除关键词匹配，LLM自由理解意图，新增规则确保"写小说"类意图**强制调用 Writer 生成正文**而非返回大纲分析；新增设定修改智能响应（角色/世界观/场景自动更新并标记重写）；CapabilityRegistry扩展MCP工具和查询能力；PlanContext注入世界观、角色、伏笔、风格DNA等完整上下文，续写时真正"记得"故事的一切。
>
> **v4.2.0** (2026-04-23): 智能交互设计重构 V2 — 模型驱动的编排范式。从程序式编排转向模型式编排：LLM自主理解用户意图、生成执行计划、系统只负责执行。移除所有关键词匹配和if/else分支，真正实现"越写越懂"。新增能力自描述系统、计划生成器、提示词进化器、记忆显性化组件。
>
> **v4.1.0** (2026-04-22): 幕前界面深度重构 — 化整为零，萤火随行。从 20+ 可见 UI 元素缩减至 <5 持久元素，AI 功能以萤火暗示形式按需浮现。44px 极简顶栏、底栏删除、内联 `/` 命令菜单、幽灵文本、三态文思模式。

## 🎭 独具特色的双界面设计

StoryForge 独创**"幕前 - 幕后"**双界面架构，让创作与阅读完美融合：

### 🎬 幕前 (Frontstage) - 沉浸式阅读写作

**设计理念**：像阅读一本精美小说一样写作

- **OKLCH 暖色纸张** - 感知均匀的色彩系统，`oklch(96.5% 0.008 95)` 暖色调背景，护眼舒适
- **霞鹜文楷正文字体** - 采用 LXGW WenKai 作为正文字体，中文排版优雅，去除通用字体的"AI 感"
- **大字号阅读体验** - 18px 正文字号，1.8 倍行距，久写不累
- **44px 极简顶栏** - 小说标题（点击进入幕后）、章节信息、字数/总字数/字号、🔥 文思三态切换、禅模式按钮，无汉堡菜单无订阅徽章
- **F1 帮助面板** - 按 `F1` 弹出浮动快捷键指南面板，分类展示写作/模式/操作提示，动画入场，禅模式自动隐藏
- **AI 萤火随行** - `Ctrl+Space` 循环三态文思模式：`off·`（关闭）/ `passive✨`（被动暗示）/ `active🔥`（主动续写）
- **幽灵文本续写** - `Ctrl+Enter` 触发 AI 续写，结果以灰色斜体幽灵文本呈现于编辑器末尾，`Tab` 接受 / `Esc` 拒绝。生成等待期间显示三点起伏动画指示器
- **智能状态调整** - AI 自动判断用户意图：无章节时自动创建第一章、文思模式非 active 时自动切换、无需用户手工配置
- **一键创建小说并直出正文** - 输入"写一稿都市玄幻小说"等意图，系统启动**创世引擎**完整流程：故事概念 → 第一章正文 → 世界观构建 → 故事大纲自动生成 → 角色创建（含完整性格/目标/外貌/年龄/性别） → 场景设计 → 伏笔自动埋设 → 知识图谱自动构建。后台自动在幕后所有对应栏目中创建完整关联卡片，同时直接将生成的第一章正文以 ghost text 展示在编辑器中，Tab 接受即可开始写作
- **内联 `/` 指令输入框** - 编辑器内输入 `/` 触发浮动输入框，可直接输入任意指令（如"续写"、"润色"、"写一篇武侠小说吧"）。回车发送，Esc 取消，再按 `/` 输出字符。用户输入经过**意图引擎**解析，模型自主决定调用续写、润色、技能、MCP 搜索或结构调整。
- **右边缘萤火暗示** - 创作建议从编辑区右边缘淡入（0.8s）→ 停留 → 淡出（1.2s），不打扰写作流
- **空态诗意引导** - 编辑器无内容时居中显示"开始写下第一句话，文思将随你而行"
- **精简侧边栏 Dock** - 3 按钮：修（修订模式）/ 批（生成古典评点）/ 幕（进入幕后）
- **修订模式与变更追踪** - 32px 单行横幅，变更列表可滚动折叠，支持逐条接受/拒绝
- **古典评点生成** - AI 模拟金圣叹风格生成朱红色段落评点（`LXGW WenKai` 字体、左边框、`※` 前缀），内联插入于段落之间
- **禅模式** - `F11` 快捷键进入绝对纯净全屏，隐藏顶栏/侧边栏/所有萤火提示
- **右键上下文菜单** - 修订模式、生成古典评点、全选、复制、剪切、粘贴
- **后台设置同步** - 写作风格、字体设置在后台统一管理

![幕前界面预览](docs/images/frontstage-preview.png)

### 🔧 幕后 (Backstage) - 全能创作工作室

**设计理念**：专业作家的数字工作台

- **故事管理** - 多故事、多场景结构化组织，故事概览面板聚合大纲/角色/场景/伏笔摘要
- **角色管理** - 角色卡片含完整性格/目标/外貌/年龄/性别，关系图谱可视化，性格追踪
- **场景化叙事** - 以场景为单位的戏剧冲突驱动
- **场景编辑器** - 三标签页设计（基础信息 / 戏剧结构 / 内容编辑）
- **伏笔看板** - 伏笔全生命周期管理：setup / payoff / abandoned 状态追踪，逾期检测与回收推荐
- **知识图谱可视化** - 基于 ReactFlow 的交互式力导向图谱，支持搜索、筛选、实体编辑，bootstrap 期间自动构建
- **记忆健康与自动归档** - 基于艾宾浩斯遗忘曲线的实体保留分析，一键归档遗忘内容
- **版本控制** - 场景历史自动快照，行级 diff 对比，随时回溯
- **技能系统** - AI 技能插件工坊，支持导入/启用/禁用/执行
- **MCP 外部服务器** - 连接外部 MCP 服务器，扩展工具生态
- **数据导出** - 支持 PDF、EPUB、Markdown 等多种格式
- **模型映射与路由** - 为不同 Agent 独立配置 LLM 模型
- **意图引擎** - 聊天栏自动解析用户意图并调度对应 Agent 执行
- **创作方法论引擎** - 雪花法 / 场景节拍 / 英雄之旅 / 人物深度，自动注入创作约束
- **StyleDNA 系统** - 六维定量风格模型，10 种经典作家 DNA，实时风格匹配
- **自适应学习系统** - 记录用户反馈、挖掘偏好、动态调节生成参数
- **创作工作流引擎** - 7 阶段全自动工作流（构思→大纲→场景→写作→审阅→迭代→入库）
- **创世引擎** (v5.0.0) - 输入一句话，自动生成完整小说世界：大纲、角色、场景、伏笔，并在幕后自动创建卡片
- **多账号 OAuth 登录** (v4.5.0) - 支持 Google / GitHub 登录，可选登录、本地优先，微信/QQ 预留框架
- **云端主站** (v4.5.0) - Linux 服务端（Actix-web + PostgreSQL + Docker），落地页 / Web 登录 / 用户后台

![幕后界面预览](docs/images/backstage-preview.png)

### 🔄 双窗口无缝协作

| 功能 | 幕前 | 幕后 |
|------|------|------|
| 阅读写作 | ✅ 沉浸式体验 | - |
| 故事管理 | - | ✅ 完整功能 |
| 场景管理 | ✅ 快速切换 / 版本历史 | ✅ 详细编辑 / 故事线拖拽 |
| AI 续写 | ✅ 流式生成 / 自动续写 | ✅ 参数调节 / 方法论约束 |
| 角色查看 | ✅ 卡片式预览 | ✅ 完整编辑 / 关系图谱 / StyleDNA |
| 知识图谱 | - | ✅ 可视化 / 编辑 / 归档 |
| 技能执行 | ✅ 快捷执行 | ✅ 技能工坊管理 |
| 文本批注 | - | ✅ 场景级批注 |
| 创作方法论 | - | ✅ 雪花法 / 节拍表 / 英雄之旅 / 人物深度 |
| StyleDNA | - | ✅ 风格选择 / 相似度分析 |
| 工作流引擎 | - | ✅ 一键全自动创作 |
| 自适应学习 | - | ✅ 反馈记录 / 偏好挖掘 |

**快捷键对照**：
- `Ctrl+Enter` / `Cmd+Enter` - 触发 AI 续写（active 模式下）
- `Ctrl+Space` - 循环文思三态模式（off → passive → active）
- `F11` - 禅模式切换（绝对纯净，隐藏所有 UI）
- `Tab` - 接受 AI 幽灵文本建议
- `Esc` - 拒绝 AI 幽灵文本建议
- `/` - 内联命令菜单（8 命令）

---

## ✨ v4.0.0 核心新特性
- **Canonical State** — 规范状态系统，统一聚合故事/角色/伏笔/知识图谱状态，AI 续写时准确知道"当前处于哪个叙事阶段"
- **Payoff Ledger** — 伏笔账本，时间窗口追踪 + 逾期检测 + 智能回收推荐，防止"挖坑不填"
- **Execution Panel** — 章节执行面板，智能推荐下一步行动（处理伏笔/续写/审校）
- **Audit System** — 五维审计（连续性/人物/风格/节奏/伏笔），light/full 模式 + 智能升降级
- **Creation Wizard** — 5 步小说创建向导（创意→世界观→角色→文风→场景）
- **Structured Outline** — Scene 分 stage 编辑（规划/大纲/起草/审校/定稿）
- **Enhanced Streaming** — Markdown 渲染 + 实时字数 + 打字机效果

### 🔧 v4.0.1 修复与优化
- **代码审计** — 扫描 40+ 模块，修复 17 处 IPC 参数不匹配、9 项空实现
- **SQLite 持久化** — `ChatManager` / `StoryStateManager` / `CollabManager` 从内存 HashMap 迁移到数据库
- **协作编辑** — WebSocket 完整消息处理（Operation/Cursor/Leave/Participants）
- **UI 优化** — 聊天工具栏布局改进、编辑器 padding 调整

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

### 🧠 智能化创作系统（5 阶段重构）

**Phase 1 - 地基重构：真实上下文**
- `StoryContextBuilder` — 从真实数据库构建丰富的 Agent 上下文（世界观、角色、场景结构）
- `QueryPipeline` — 四阶段知识检索（CJK 分词搜索 → 知识图谱扩展 → 预算控制 → 上下文组装）
- `ContinuityEngine` + `ForeshadowingTracker` — 连续性追踪与伏笔回收系统

**Phase 2 - 方法论注入**
- 创作方法论引擎：`MethodologyEngine` 自动将方法论约束注入 Writer 系统提示词
- 四种经典方法论：雪花法（10 步）· 场景节拍表（6 节拍）· 英雄之旅（12 阶段）· 人物深度模型（6 维度）
- `AgentOrchestrator` — Writer→Inspector→Writer 质量反馈循环（可配置阈值与最大循环数）

**Phase 3 - 风格深度化**
- `StyleDNA` 六维定量模型：词汇/句法/修辞/视角/情感/对白
- 10 种内置经典作家 DNA：金庸、张爱玲、海明威、村上春树、莫言、古典散文、现代极简、黑色侦探、武侠诗意、浪漫主义
- 实时风格相似度计算与提示词注入

**Phase 4 - 自适应学习**
- `FeedbackRecorder` — 记录用户对 AI 生成内容的接受/拒绝/修改行为
- `PreferenceMiner` — 五维度启发式偏好挖掘（主题/风格/节奏/视角/结构）
- `AdaptiveGenerator` — 动态调节温度（temperature）、top-p、提示词权重
- `PromptPersonalizer` — 将用户偏好自动注入系统提示词

**Phase 5 - 工作流闭环**
- `CreationWorkflowEngine` — 7 阶段全自动工作流：构思 → 大纲 → 场景设计 → 写作 → 审阅 → 迭代 → 入库
- 3 种创作模式：一键全自动 / AI 初稿 + 人工精修 / 人工初稿 + AI 润色
- `QualityChecker` — 四维质量评估（结构/人物/风格/情节）

### 🎨 品牌焕新

- 全新 Logo：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型
- `cargo tauri icon logo.png` 生成全平台图标包（Windows / macOS / iOS / Android）

### 🚀 v4.3.0 智能交互创作流程深度优化

**一键创作体验升级**
- Bootstrap前端进度实时可见 — 构思→世界观→角色→场景→撰写，5步进度在状态栏清晰展示
- 创建完成后自动加载 — 新故事自动切换，第一章内容直接呈现于编辑器
- Chapter/Scene双轨同步 — Bootstrap生成第一章时同时创建Chapter记录，前端零延迟加载

**模型驱动编排全面落地**
- 彻底移除关键词匹配 — `detect_and_route_intent` 已清空，所有用户输入交由PlanGenerator自由理解
- PlanContext上下文增强 — 注入世界观摘要、角色列表、活跃伏笔、风格DNA、MCP可用工具
- PlanGenerator Prompt进化 — 新增技能调用指南、设定修改指南、MCP工具使用指南、伏笔处理指南

**设定修改智能响应**
- `update_character` — LLM解析用户修改意图，自动更新角色属性（姓名/性格/背景/目标）
- `update_world_building` — 智能修改世界观规则、历史背景、力量体系
- `update_scene` — 调整场景结构、戏剧目标、地点时间等属性
- 影响检测 — 设定修改后自动标记受影响场景为 `needs_rewrite`，续写时自动重写

**MCP与技能自动化**
- CapabilityRegistry注册MCP工具 — PlanGenerator知道何时调用外部工具搜索资料
- 内置技能自动调用 — `builtin.style_enhancer` / `character_voice` / `emotion_pacing` 可由模型自主编排

### 🏗️ 架构与质量

- **183 项 Rust 测试全部通过**（新增 15 项 planner 模块测试：bootstrap JSON 提取/概念序列化、executor 参数解析、mod PlanContext/PlanStep）
- `cargo check` 零错误零警告
- `npm run build` 通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 5.0.0

---

## 📊 项目状态概览

**当前版本**: v5.1.1  
**最后更新**: 2026-04-30  
**GitHub**: https://github.com/91zgaoge/StoryForge  
**整体完成度**: 100%

> 🍃 品牌图标：「草苔」立方体标志 —— 融合自然叶脉纹理的几何立方体造型，象征创作的结构化生长与文学的立体纵深

| 模块 | 状态 | 完成度 |
|------|------|--------|
| 核心架构 | ✅ 稳定 | 100% |
| 场景化系统 | ✅ 完成 | 100% |
| 记忆系统 | ✅ 完成 | 100% |
| AI 生成 | ✅ 完成 | 100% |
| 知识图谱可视化 | ✅ 完成 | 100% |
| 工作室配置 | ✅ 完成 | 100% |
| 双界面设计 | ✅ 完成 | 100% |
| LLM 集成 / 流式输出 | ✅ 完成 | 100% |
| 本地模型配置 | ✅ 完成 | 100% |
| Agent 系统 / 意图引擎 | ✅ 完成 | 100% |
| 技能系统 / MCP | ✅ 完成 | 100% |
| 版本控制 / 修订模式 | ✅ 完成 | 100% |
| 文本批注 | ✅ 完成 | 100% |
| 前端界面 | ✅ 完成 | 100% |
| 桌面构建打包 | ✅ 完成 | 100% |
| 创作方法论引擎 | ✅ 完成 | 100% |
| StyleDNA 系统 | ✅ 完成 | 100% |
| 自适应学习系统 | ✅ 完成 | 100% |
| 创作工作流引擎 | ✅ 完成 | 100% |
| 拆书功能 | ✅ 完成 | 100% |
| 任务系统 | ✅ 完成 | 100% |
| 测试覆盖 | ✅ 完成 | 193 tests |
| 创世引擎 | ✅ 完成 | 100% |

---

## 🗂️ 项目结构

```
v2-rust/
├── src-frontend/                 # 前端代码 (React + TypeScript)
│   ├── src/
│   │   ├── main.tsx             # 幕后入口
│   │   ├── App.tsx              # 幕后主应用
│   │   ├── frontstage/          # 幕前界面
│   │   │   ├── FrontstageApp.tsx
│   │   │   ├── components/
│   │   │   │   ├── RichTextEditor.tsx    # TipTap 富文本编辑器
│   │   │   │   ├── EditorContextMenu.tsx # 右键上下文菜单
│   │   │   │   ├── AiSuggestionBubble.tsx # AI 氛围提示
│   │   │   │   ├── CharacterCardPopup.tsx # 角色卡片弹窗
│   │   │   │   └── ChapterOutline.tsx
│   │   │   └── styles/frontstage.css
│   │   ├── pages/               # 幕后页面
│   │   │   ├── Dashboard.tsx    # 仪表盘
│   │   │   ├── Stories.tsx      # 故事库
│   │   │   ├── Characters.tsx   # 角色管理
│   │   │   ├── Scenes.tsx       # 场景管理
│   │   │   ├── Chapters.tsx     # 章回管理
│   │   │   ├── KnowledgeGraph.tsx # 知识图谱
│   │   │   ├── Skills.tsx       # 技能工坊
│   │   │   ├── Mcp.tsx          # MCP 服务器配置
│   │   │   └── Settings.tsx     # 设置中心
│   │   ├── components/          # 共享组件
│   │   │   ├── StoryTimeline.tsx    # 故事线视图
│   │   │   ├── SceneEditor.tsx      # 场景编辑器
│   │   │   ├── VersionTimeline.tsx  # 版本历史
│   │   │   ├── DiffViewer.tsx       # 版本对比
│   │   │   ├── VectorSearch.tsx     # 向量搜索
│   │   │   ├── NovelCreationWizard.tsx # 创建向导
│   │   │   └── ExportDialog.tsx
│   │   └── hooks/               # 自定义 Hooks
│   │       ├── useScenes.ts
│   │       ├── useIntent.ts         # 意图解析
│   │       ├── useChangeTracking.ts # 修订追踪
│   │       ├── useTextAnnotations.ts # 文本批注
│   │       ├── useCommentThreads.ts  # 评论线程
│   │       ├── useSceneVersions.ts   # 版本管理
│   │       ├── useMcpTools.ts        # MCP 工具
│   │       ├── useVectorSearch.ts    # 向量搜索
│   │       └── useStudioConfig.ts
│   ├── index.html               # 幕后 HTML
│   ├── frontstage.html          # 幕前 HTML
│   └── package.json
│
├── src-tauri/                   # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs              # 应用入口
│   │   ├── lib.rs               # 库入口
│   │   ├── commands.rs          # Tauri 命令
│   │   ├── commands_v3.rs       # V3 命令集
│   │   ├── intent.rs            # 意图解析引擎
│   │   ├── db/                  # 数据库层
│   │   │   ├── models_v3.rs
│   │   │   └── repositories_v3.rs
│   │   ├── agents/              # Agent 系统
│   │   │   ├── service.rs
 │   │   │   ├── orchestrator.rs  # Writer→Inspector 质量闭环
│   │   │   ├── commentator.rs   # 古典评点家
│   │   │   ├── memory_compressor.rs
│   │   │   └── novel_creation.rs
 │   │   ├── creative_engine/     # 智能化创作引擎 (v3.4.0)
 │   │   │   ├── mod.rs
 │   │   │   ├── context_builder.rs    # 真实 DB 上下文构建
 │   │   │   ├── continuity.rs         # 连续性追踪
 │   │   │   ├── foreshadowing.rs      # 伏笔回收系统
 │   │   │   ├── methodology/          # 创作方法论引擎
 │   │   │   │   ├── mod.rs
 │   │   │   │   ├── snowflake.rs
 │   │   │   │   ├── scene_structure.rs
 │   │   │   │   ├── hero_journey.rs
 │   │   │   │   └── character_depth.rs
 │   │   │   ├── style/                # StyleDNA 系统
 │   │   │   │   ├── mod.rs
 │   │   │   │   ├── dna.rs
 │   │   │   │   └── classic_styles.rs
 │   │   │   ├── adaptive/             # 自适应学习系统
 │   │   │   │   ├── mod.rs
 │   │   │   │   ├── feedback.rs
 │   │   │   │   ├── miner.rs
 │   │   │   │   ├── generator.rs
 │   │   │   │   └── personalizer.rs
 │   │   │   └── workflow/             # 工作流引擎
 │   │   │       ├── mod.rs
 │   │   │       ├── engine.rs
 │   │   │       └── quality.rs
│   │   ├── memory/              # 记忆系统
│   │   │   ├── tokenizer.rs
│   │   │   ├── ingest.rs
│   │   │   ├── query.rs
│   │   │   ├── hybrid_search.rs
│   │   │   └── multi_agent.rs
│   │   ├── llm/                 # LLM 适配器
│   │   │   ├── adapter.rs
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   └── ollama.rs
│   │   ├── collab/              # 协作编辑
│   │   │   └── websocket.rs
│   │   └── config/              # 配置管理
│   │       └── studio_manager.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── docs/                        # 文档
├── README.md
├── CHANGELOG.md
├── ARCHITECTURE.md
└── AGENTS.md
```

---

## 🎨 前端双界面架构

### 技术栈
- **React 18** - UI 框架
- **Vite 6** - 构建工具，支持多入口
- **TypeScript 5.8** - 类型安全
- **Tailwind CSS 3** - 原子化样式
- **TipTap** - ProseMirror 富文本编辑器
- **Zustand** - 轻量状态管理
- **TanStack Query** - 服务端状态管理
- **Tauri 2.4** - 桌面应用框架
- **@dnd-kit** - 拖拽排序
- **ReactFlow** - 知识图谱可视化

### 核心组件

#### RichTextEditor - 幕前富文本编辑器
集成 TipTap 编辑器，极简沉浸式写作体验：
- **幽灵文本** - AI 续写结果以灰色斜体段落内联呈现，附带萤火操作栏
- **内联 `/` 命令菜单** - 9 命令（续写/润色/古风/场景/自动续写/审校/评点/排版/自由指令），光标处触发
- **修订模式** - trackInsert / trackDelete 可视化标记，32px 紧凑横幅
- **古典评点段落** - 金圣叹式朱批内联插入（红色 `LXGW WenKai` 字体）
- **空态引导** - 无内容时显示诗意提示
- **右键上下文菜单** - 修订模式、生成评点、全选/复制/剪切/粘贴

#### StoryTimeline - 故事线视图
可视化场景序列，支持拖拽重新排序：
- 场景卡片展示戏剧目标、冲突类型
- 拖拽手柄调整场景顺序
- 点击选择场景进行编辑

#### SceneEditor - 场景编辑器
三标签页场景编辑：
- **基础信息** - 标题、场景设置、在场角色、记忆压缩
- **戏剧结构** - 戏剧目标、外部压迫、冲突类型（11 种）
- **内容编辑** - 富文本编辑器、场景批注、版本历史

#### KnowledgeGraphView - 知识图谱可视化
基于 ReactFlow 的交互式力导向图谱：
- 节点按实体类型着色，关系边按强度显示粗细
- 实时搜索与类型筛选，双击节点聚焦居中
- 右侧详情面板支持实体就地编辑
- 记忆健康面板与自动归档建议

#### NovelCreationWizard - 创建向导
引导式小说创建流程：
- 类型输入（灰色提示词）
- 世界观 / 角色谱 / 文风卡片式选择
- 完成自动 Ingest 到知识图谱

---

## ✅ 功能实现详情

### 0. 创世引擎 v5.0.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 故事大纲自动生成 | ✅ | Bootstrap 流程自动产出结构化故事大纲 |
| 角色完整性格小传 | ✅ | 含 personality/goals/appearance/age/gender 的完整角色卡片 |
| 角色关系图谱 | ✅ | 自动构建角色间关系并可视化展示 |
| 伏笔自动埋设 | ✅ | 智能分析故事结构并自动创建伏笔卡片 |
| 知识图谱自动构建 | ✅ | Bootstrap 完成后自动提取实体关系入库 |
| 前后台智能联动 | ✅ | 幕前创建后幕后自动刷新并展示全部卡片 |
| 空白修复 | ✅ | 修复后台隐藏后重新显示白屏问题 |

### 1. 智能化创作系统 v3.4.0 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| `StoryContextBuilder` | ✅ | 从真实数据库构建丰富的 Agent 上下文 |
| `QueryPipeline` | ✅ | 四阶段知识检索（CJK→图谱→预算→组装） |
| `ContinuityEngine` | ✅ | 章节连续性追踪与伏笔回收 |
| `ForeshadowingTracker` | ✅ | 伏笔埋设与回收追踪 |
| `MethodologyEngine` | ✅ | 雪花法/节拍表/英雄之旅/人物深度 |
| `AgentOrchestrator` | ✅ | Writer→Inspector→Writer 质量闭环 |
| `StyleDNA` | ✅ | 六维定量风格模型，10 种经典作家 DNA |
| `StyleAnalyzer` | ✅ | 从文本提取风格指纹 |
| `StyleChecker` | ✅ | 对比文本与目标 DNA 相似度 |
| `FeedbackRecorder` | ✅ | 记录接受/拒绝/修改行为 |
| `PreferenceMiner` | ✅ | 五维度启发式偏好挖掘 |
| `AdaptiveGenerator` | ✅ | 动态调节 temperature/top-p/权重 |
| `PromptPersonalizer` | ✅ | 将用户偏好注入系统提示词 |
| `CreationWorkflowEngine` | ✅ | 7 阶段全自动工作流 |
| `QualityChecker` | ✅ | 四维质量评估（结构/人物/风格/情节） |

### 2. 场景化叙事系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| Scene 模型 | ✅ | 戏剧目标、外部压迫、冲突类型 |
| SceneRepository | ✅ | CRUD + 重新排序 |
| 故事线视图 | ✅ | 拖拽排序、场景卡片 |
| 场景编辑器 | ✅ | 三标签页 + 批注 + 版本历史 |
| 冲突类型枚举 | ✅ | 11 种标准冲突类型 |
| 记忆压缩 | ✅ | MemoryCompressorAgent 集成 |

### 3. 记忆系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| CJK 分词器 | ✅ | 二元组分词，中日韩支持 |
| Ingest 管线 | ✅ | 两步思维链：分析→生成 |
| 知识图谱 | ✅ | 实体/关系带强度评分，ReactFlow 可视化 |
| 查询检索 | ✅ | 四阶段检索管线 |
| 多助手会话 | ✅ | 6 种助手类型独立会话 |
| 混合搜索 | ✅ | BM25 + 向量融合 (RRF) |
| FTS5 全文索引 | ✅ | SQLite 原生全文加速 |
| 场景版本 | ✅ | 版本历史、比较、恢复、版本链 |
| 记忆保留 | ✅ | 遗忘曲线、优先级管理、自动归档 |
| 向量持久化 | ✅ | SQLite + LanceDB 混合存储 |

### 4. AI 智能生成 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| NovelCreationAgent | ✅ | 世界观/角色/文风/首个场景生成 |
| 创建向导 | ✅ | 4 步引导流程，自动 Ingest |
| 卡片式 UI | ✅ | 单击选择，双击编辑 |
| 古典评点家 | ✅ | 金圣叹风格段落点评 |
| 意图引擎 | ✅ | 11 种意图解析 + Agent 调度 |
| 真实 SSE 流式 | ✅ | OpenAI / Anthropic / Ollama 全适配 |

### 5. 协作与批注系统 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| 修订模式 | ✅ | trackInsert / trackDelete，逐条接受/拒绝 |
| 文本内联批注 | ✅ | note/todo/warning/idea，高亮锚定 |
| 评论线程 | ✅ | 多轮回复、解决/重开、删除 |
| 场景批注 | ✅ | 场景级批注/待办/警告 |
| WebSocket 协作 | ✅ | 协作编辑服务端 |

### 6. 工作室配置与扩展 (100% ✅)

| 功能 | 状态 | 说明 |
|------|------|------|
| StudioConfig 模型 | ✅ | 每部小说独立配置 |
| ZIP 导出/导入 | ✅ | `.storyforge` 格式，选择性导入 |
| 技能系统 | ✅ | 内置 5+ 技能，支持导入/禁用/执行 |
| MCP 服务器 | ✅ | 外部服务器配置与工具调用 |
| 模型映射 | ✅ | Agent → LLM 独立路由 |
| 默认主题 | ✅ | 幕前暖色 / 幕后暗色 |

### 7. 本地模型与构建

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 本地模型配置 | 100% | Gemma / Qwen / bge-m3 |
| LLM 集成 | 100% | OpenAI/Anthropic/Ollama/本地 API |
| Agent 系统 | 100% | 6 种 Agent + 模型路由 |
| 技能系统 | 100% | 内置技能 + 扩展支持 |
| 向量检索 | 100% | TF-IDF + BM25 + 语义 + 混合 |
| 导出功能 | 100% | PDF/EPUB/Markdown |
| Tauri 打包 | 100% | MSI + NSIS 安装包 |

---

## 📅 更新历史

### v5.1.1 (2026-05-01) - 设计-实现对齐全面修复
- **P0 修复**: `update_chapter` / `create_chapter` 保存后自动触发 IngestPipeline，知识图谱实时更新
- **P0 修复**: `state_sync` character/chapter update/delete 事件携带正确 `story_id`
- **P0 修复**: `FrontstageToolbar` 传递 `story_id` 到 `show_backstage`
- **P0 修复**: `WorkflowScheduler::schedule_execution` 实现队列机制
- **P1 补全**: `PromptLibrary` 新增 StyleChecker + Commentator 模板
- **P1 补全**: `prompts/methodologies/` 雪花法/英雄之旅/场景结构模板

### v5.1.0 (2026-05-01) - 幕前幕后自动关联对齐

> **核心理念**：从"各自为战"到"自动联动"。所有数据修改自动同步，前后台零延迟对齐。

**幕前幕后自动关联**
- **Chapter↔Scene 双向映射** — Migration 37 建立双向外键关联，ChapterRepository 自动查找/创建关联 Scene
- **统一实时状态中心** — 后端 `state_sync` 模块定义 16 种 `SyncEvent`，所有数据修改命令完成后自动发射同步事件
- **前端 useSyncStore Hook** — 监听 `sync-event` 频道，根据事件类型自动 `invalidateQueries` / `removeQueries`
- **Bootstrap 完成后幕前自动加载** — `smartExecute` 返回后检测 `story_created:` 消息，自动加载新故事并切换到第一章；Bootstrap 完成后双重 `ChapterSwitch` 保险
- **幕前→幕后快速跳转** — `Ctrl+Shift+B` 快捷键，标题栏点击，幕后自动定位当前故事并高亮
- **AgentOrchestrator 闭环接入** — `execute_writer` 集成 `AgentOrchestrator::execute_write_with_inspection`，Writer→Inspector→StyleChecker→Writer 自动质检改写生效
- **自适应学习闭环激活** — `record_feedback` 成功后异步触发 `mine_preferences`，偏好挖掘自动运行
- **Zustand↔TanStack Query 同步** — `App.tsx` 监听 `currentStory` 变化，自动刷新关联数据缓存
- **窗口通信事件标准化** — `DataRefresh` 统一由 `useSyncStore` 处理，消除 `backstage-update` 和 `handleWindowShown` 中的重复刷新

**编译与测试**
- `cargo check`：零错误
- `cargo test`：193/193 全部通过
- `npm run build`：通过

### v5.0.0 (2026-04-30) - 创世引擎：一键创世，万物关联

> **核心理念**：输入一句话，系统自动在幕后所有对应栏目中创建完整关联卡片。从"能创建"到"自动构建完整世界"。

**创世引擎全流程**
- **故事概念生成** — 解析用户意图，生成故事标题、类型、基调、简介
- **第一章直出正文** — 同步生成第一章正文并以 ghost text 展示在幕前编辑器
- **世界观自动构建** — 生成世界观规则、历史背景、力量体系、地理环境
- **故事大纲自动生成** — 产出完整的故事结构、主线/支线、关键转折点
- **角色完整性格小传** — 每个角色含 personality（性格）、goals（目标）、appearance（外貌）、age（年龄）、gender（性别）的完整卡片
- **角色关系图谱** — 自动分析角色间关系并构建可视化图谱
- **场景自动设计** — 按大纲生成场景卡片，含戏剧目标、冲突类型、场景设置
- **伏笔自动埋设** — 智能识别故事中的伏笔点并创建追踪卡片
- **知识图谱自动构建** — 从所有生成内容中提取实体和关系，自动入库并可视化
- **前后台智能联动** — 幕前创建完成后，幕后自动刷新并展示所有生成的卡片，无需手动切换

**关键 Bug 修复**
- **修复后台白屏** — 修复后台窗口隐藏后重新显示时出现空白/白屏的问题
- **修复卡片不显示** — 修复 Bootstrap 完成后幕后不显示生成的角色、场景、伏笔、故事大纲卡片的问题

**编译与测试**
- `cargo check`：零错误零警告
- `cargo test`：193/193 全部通过
- `npm run build`：通过
- 版本号统一：Cargo.toml / package.json / tauri.conf.json → 5.0.0

### v4.2.0 (2026-04-23) - 智能交互设计重构 V2：模型驱动的编排范式

> **核心理念**：从"程序式编排"转向"模型式编排"。人类只定义能力能做什么，模型负责决定什么时候用、怎么用、按什么顺序。真正实现"越写越懂"。

**架构变革**
- **CapabilityRegistry（能力自描述系统）**：Agent 和 Skill 用自然语言描述自己的能力（`description` / `when_to_use` / `input_description` / `output_description`），模型阅读这些描述自主选择需要的能力。人类不再写死 Agent 映射规则。
- **PlanGenerator（模型计划生成器）**：取代旧的 IntentParser + IntentExecutor。LLM 接收系统状态 + 用户输入 + 能力清单，自主输出执行计划（自由文本理解 + 步骤列表 + 参数 + 依赖关系）。没有预设分类标签，没有关键词匹配。
- **PlanExecutor（计划执行引擎）**：Dumb executor，只做一件事——忠实地执行 LLM 生成的计划。按顺序执行步骤、将前一步输出传给后一步、失败时按备选方案处理。所有决策已在计划中。
- **PromptEvolver（提示词进化器）**：每次调用 Agent/Skill 前，LLM 根据当前故事上下文（题材、叙事阶段、用户偏好）自由改写整个 prompt。不是模板变量替换，而是真正的"进化"。
- **AiLearningIndicator（记忆显性化）**：前端组件，每次 AI 交互后展示"系统学到了什么"（如"已记录：你偏好快节奏打斗"）。让"越写越懂"对用户可见。
- **CapabilityEvolutionEngine（能力进化反馈环）**：记录每次能力调用的执行结果，长期优化能力描述的准确性。
- **PlanTemplateLibrary（计划模板学习）**：记录成功的执行计划，类似请求来时直接复用或微调。

**移除的程序式规则**
- 移除 `IntentType` 枚举（11 类预设分类）
- 移除前端正则关键词检测（`/写|创作|生成|续写/`）
- 移除 `IntentExecutor.map_agents` 写死映射
- 移除 `FrontstageApp` 中 `if (!currentStory) { toast.error(...) }` 强制流程
- 移除所有基于关键词的意图分支判断

**前端简化**
- `handleSmartGeneration` / `handleRequestGeneration` 统一走 `smart_execute`
- 用户任何输入都交给模型决定如何处理
- 新增 `smartExecute` IPC 命令

**编译与测试**
- `cargo check`：零错误零警告
- `cargo test`：160/160 全部通过
- `npm run build`：通过

### v4.1.0 (2026-04-22) - 幕前界面深度重构：化整为零，萤火随行

> **设计理念**：从 20+ 可见 UI 元素缩减至 <5 持久元素。AI 功能以萤火暗示（firefly hints）形式按需浮现，用完即隐。

**P0 核心重构**
- 顶栏精简为 44px 细线：小说标题/章节信息/字数/字号/文思三态/禅模式。快捷键提示改为 F1 浮动面板
- 底栏删除：彻底删除底部聊天工具栏。AI 结果以幽灵文本内联呈现，Tab 接受 / Esc 拒绝
- 侧边栏精简：5 按钮 → 3 按钮（修/批/幕）
- 键盘快捷键：`Ctrl+Enter` 续写，`Ctrl+Space` 循环文思，`F11` 禅模式

**P1 萤火系统**
- 幽灵文本：编辑器末尾灰色斜体段落 + 萤火操作栏
- 右边缘萤火：`smartGhostText` 淡入（0.8s）→ 停留 → 淡出（1.2s）
- 空态引导：编辑器无内容时居中诗意提示

**P2 体验优化**
- 内联 `/` 指令输入框：输入 `/` 后弹出浮动输入框，直接输入任意指令
- WenSiPanel 浮动化：从底栏嵌入改为右下角浮动卡片，新增"自由指令"标签页
- 修订横幅精简：多行可展开 → 32px 单行
- 古典评点保留：金圣叹式朱批内联段落，红色 `oklch(55% 0.18 25)`

**🗑️ 移除（设计决策）**
- 显式注释/评论系统：sidebar "注"按钮、注释/评论面板、选中文本弹窗创建按钮、右键菜单项、相关 hooks
- 原因：AI 写作工具不需要创作者标注自己的作品；AI 反馈应以幽灵文本或古典评点形式自然呈现

**📊 统计**
- `cargo check` 零错误零警告，`cargo test` 160/160，`npm run build` 通过
- 修改文件：前端 8 个，删除代码约 800 行

### v4.0.1 (2026-04-22) - 全面代码审计与空实现修复

**Phase A - 代码审计与 P0 修复**
- 综合代码审计：扫描 40+ 模块，识别 5 严重 / 17 参数 / 9 空实现
- IPC 参数统一：17 处 camelCase→snake_case（Tauri v2 反序列化修复）
- 空实现补全：`analytics` 真实统计、`agents/commands` 真实状态、`skills/executor` 真实 MCP 调用、`export/import_from_text` 正则解析章节、`workflow/scheduler` 执行日志、`evolution/updater` manifest CRUD、`mcp/server` 缺失 `.await`
- 前端修复：移除硬编码 API keys、WebSocket 真实发送、移除 mock 流式、文本分析器增量分析
- UI 优化：聊天工具栏从 absolute 改为正常流、编辑器 padding 优化
- 类型统一：移除重复 `McpServerConfig`

**Phase B - 内存模块 SQLite 持久化**
- Migration 26/27/28：`chat_sessions` + `chat_messages`、`story_runtime_states`、`collab_sessions` + `collab_participants`
- `ChatManager` / `StoryStateManager` / `CollabManager`：内存 HashMap → DbPool
- `WebSocketServer`：完整消息处理闭环（Operation/Cursor/Leave/Participants）

### v3.4.0 (2026-04-18) - 智能化创作系统（5 阶段重构）

- **Phase 1 地基重构** - `StoryContextBuilder` 真实 DB 上下文, `QueryPipeline` 四阶段检索, `ContinuityEngine`, `ForeshadowingTracker` (27 tests)
- **Phase 2 方法论注入** - 雪花法/场景节拍/英雄之旅/人物深度 + `MethodologyEngine` + `AgentOrchestrator` Writer→Inspector 闭环 (34 tests)
- **Phase 3 风格深度化** - `StyleDNA` 六维模型, 10 经典作家 DNA, `StyleAnalyzer`, `StyleChecker` (45 tests)
- **Phase 4 自适应学习** - `FeedbackRecorder`, `PreferenceMiner`, `AdaptiveGenerator`, `PromptPersonalizer` (54 tests)
- **Phase 5 工作流闭环** - `CreationWorkflowEngine` 7 阶段工作流, `QualityChecker` 四维评估, 3 种创作模式 (63 tests)
- **品牌焕新** - `logo.png` 立方体标志生成全平台图标包
- **版本统一** - Cargo.toml / package.json / tauri.conf.json → 3.4.0

### v3.3.0 (2026-04-15) - 功能断层修复与架构清理

- **幕前右键菜单修复** - Tailwind utilities 补充、事件捕获修复、WebView2 默认菜单禁用、暖色 UI 重构
- **MCP 外部服务器连接** - 配置卡片 + 工具调用
- **技能导入** - 本地文件选择器导入
- **Agent 流式执行与取消** - 实时进度 + 可中断
- **知识图谱实体就地编辑** - 节点属性增删改
- **版本系统增强** - 版本链视图 + diff 元信息
- **LLM 调用路径决策** - 明确 HTTP 直连为官方路径
- **Rust Warnings 降噪** - `cargo check` 0 警告

### v3.2.0 (2026-04-14) - 意图引擎与 Agent 调度 + 知识图谱可视化 + 修订模式

- **知识图谱可视化** - ReactFlow 力导向图谱，搜索/筛选/实体编辑
- **记忆健康与自动归档** - 艾宾浩斯遗忘曲线 + 一键归档
- **意图解析引擎** - 11 种意图类型，聊天栏自动调度 Agent
- **Agent 模型映射** - 按 Agent 类型路由到不同 LLM
- **真实 SSE 流式输出** - OpenAI/Anthropic/Ollama 全适配
- **文本内联批注 + 评论线程** - 选中文本批注与讨论
- **修订模式与变更追踪** - trackInsert/trackDelete + 接受/拒绝
- **古典评点家 Agent** - 金圣叹风格文学点评
- **小说创建向导后端连通** - 4 步引导 + 自动 Ingest
- **记忆压缩师集成** - 场景内容智能压缩
- **SQLite FTS5 全文索引** - BM25 + 向量混合搜索

### v3.1.2 (2026-04-13) - 设置页增强、浏览器开发环境修复与全新应用图标

- **全新羽毛笔品牌图标**
- **模型连接状态指示灯** - 实时检测延迟
- **设置页编辑模型模态框修复** - `custom` 提供商兼容
- **浏览器开发环境兼容** - Vite dev server 模型回退

### v3.1.1 (2026-04-13) - 幕前 Waza 设计重构与 CI 修复

- **幕前界面重构** - OKLCH 颜色系统、LXGW WenKai 字体、精简侧边栏
- **底部 LLM 对话栏** - 悬停显示、流式对话、模型状态灯
- **本地三模型配置** - Gemma-4 / Qwen3.5 / bge-m3
- **Tauri 构建与 CI 修复** - MSI/NSIS 安装包、GitHub Actions Nightly Release

### v3.1.0 (2026-04-13) - 智能记忆与版本管理

- **混合搜索** - BM25 + Vector RRF 融合
- **场景版本历史** - 快照、diff、恢复、统计
- **记忆保留曲线** - 优先级分级、自动归档建议

### v3.0.0 (2026-04-12) - 重大架构调整

- **场景化叙事架构** - Scene 取代 Chapter
- **增强记忆系统** - CJK 分词、Ingest 管线、知识图谱
- **AI 智能生成** - 引导式小说创建
- **工作室配置** - 导入/导出功能

---

## 🚀 快速开始

### 环境要求
- Rust 1.95+
- Node.js 18+ (前端开发)
- SQLite 3

### 开发模式

**快速启动（Windows PowerShell）**:
```powershell
# 一键启动前端和后端
.\start-dev.ps1
```

**手动启动**:
```bash
# 1. 克隆项目
cd v2-rust

# 2. 安装依赖
cd src-frontend && npm install && cd ..

# 3. 终端 1 - 启动前端开发服务器
cd src-frontend && npm run dev

# 4. 终端 2 - 启动 Tauri 应用
cd src-tauri && cargo tauri dev

# 5. 构建发布版本（Windows）
cd src-tauri && cargo tauri build

# 构建产物
# target/release/storyforge.exe          - 独立可执行文件
# target/release/bundle/msi/*.msi        - MSI 安装包
# target/release/bundle/nsis/*-setup.exe - NSIS 安装包
```

**双界面入口**:
- 幕前界面: http://localhost:5173/frontstage.html
- 幕后界面: http://localhost:5173/index.html
- Tauri 应用会自动打开两个窗口，幕前在前，幕后在后

**故障排除**: 参考 [TROUBLESHOOTING.md](TROUBLESHOOTING.md)

### 配置说明

配置文件位置：`~/.config/storyforge/config.json`

```json
{
  "llm": {
    "provider": "openai",
    "api_key": "your-api-key",
    "model": "gpt-4",
    "max_tokens": 4096,
    "temperature": 0.7
  }
}
```

---

## 🛣️ 路线图 (Roadmap)

### 已完成 (v3.4.0) ✅
- [x] **场景化叙事** - 场景取代章节，戏剧冲突驱动
- [x] **记忆系统** - 基于 llm_wiki 的知识图谱
- [x] **AI 智能生成** - 引导式小说创建
- [x] **工作室配置** - 导入/导出功能
- [x] **混合搜索** - BM25 + Vector RRF
- [x] **场景版本历史** - 快照、diff、版本链
- [x] **记忆保留曲线** - 自动归档
- [x] **幕前界面重构** - Waza / OKLCH / LXGW WenKai
- [x] **本地模型配置** - Gemma / Qwen / bge-m3
- [x] **Tauri 构建打包** - MSI / NSIS
- [x] **GitHub Actions CI** - Nightly Release
- [x] **知识图谱可视化** - ReactFlow 交互图谱
- [x] **意图引擎** - Agent 调度
- [x] **修订模式** - 变更追踪
- [x] **文本批注 / 评论线程** - 内联协作
- [x] **MCP 外部服务器** - 工具扩展
- [x] **技能工坊** - 导入/执行/管理
- [x] **创作方法论引擎** - 雪花法/节拍表/英雄之旅/人物深度
- [x] **StyleDNA 系统** - 六维风格模型 + 10 经典作家 DNA
- [x] **自适应学习系统** - 反馈→偏好→生成→个性化
- [x] **创作工作流引擎** - 7 阶段全自动闭环
- [x] **拆书功能** - txt/pdf/epub 解析，LLM 分析，一键转故事
- [x] **任务系统** - once/daily/weekly/cron 调度，心跳检测，防重叠执行
- [x] **向量化存储** - 场景/人物 embedding 自动生成并入库

### 已完成 (v5.0.0) ✅
- [x] **创世引擎** - 一键创世：概念→章节→世界观→大纲→角色→场景→伏笔→知识图谱，前后台智能联动

### 已完成 (v3.5.x ~ v4.0.1) ✅
- [x] 前端 UI 接入新方法引擎（方法论选择、StyleDNA 面板、工作流启动）
- [x] 性能优化（大数据量场景）
- [x] 导出模板自定义
- [x] 全面代码审计与空实现修复
- [x] 内存模块 SQLite 持久化

### 短期计划 (v4.1.x)
- [ ] 云端同步
- [ ] 协作写作增强（多人实时编辑 OT 完整实现）
- [ ] 插件市场

### 中期计划 (v4.2.0)
- [ ] WebAssembly 前端
- [ ] 自研小模型
- [ ] 移动端适配

### 长期计划 (v5.0.0)
- [ ] 发布平台集成
- [ ] AI 全自动长篇小说生成

---

## 📚 相关文档

- [架构设计](ARCHITECTURE.md) - 详细架构说明
- [功能清单](docs/FEATURES.md) - 完整功能列表
- [更新日志](CHANGELOG.md) - 版本变更记录
- [项目状态](PROJECT_STATUS.md) - 开发进度
- [Agent 指南](AGENTS.md) - AI 助手开发指南
- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计

---

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

**StoryForge (草苔)** - 让创作更智能 🌿
