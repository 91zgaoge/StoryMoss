<p align="center">
  <img src="docs/images/logo.png" alt="StoryForge 草苔" width="120" />
</p>

# StoryForge (草苔) — AI 辅助小说创作系统

> 🌿 越写越懂的 AI 小说创作桌面应用
>
> 专为小说作者打造的创作工作台：幕后管理故事/角色/场景/世界观，幕前沉浸式写作，AI 在需要时随行辅助。

[![Version](https://img.shields.io/badge/version-v0.26.47-gold)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-ISC-blue.svg)](./LICENSE)

**最新动态**：v0.26.47 CI 热修复——`cargo +nightly fmt` 格式化 v0.26.46 方法论/Genesis 代码，恢复 rust-check 全绿。

**上一版 v0.26.46** 创世方法论全链路注入——修复 background 模板断链，按步骤注入雪花/HDWB 并推进 `methodology_step`；题材画像 match-or-create；拆书 StoryArc/作者/伏笔落库与分块超时止血。

**上一版 v0.26.44** Genesis 首章质量——开篇骨架步 + 概念加厚 + 策略中文化 + 四元组接入。

**上一版 v0.26.43** 修复底部「准备上下文」前图标显示为方框——改用 Lucide SVG，不再依赖 WebView emoji 字体。

**上一版 v0.26.42** 修复续写「有 Tab 提示、无幽灵文本、确认后无追加」——接受后 30s 渲染锁未在新续写时清零。

**上一版 v0.26.41** 清偿两项架构债——管线 Finalize 按 `scene_id` 直写编辑场景；`kg_entities`/`memory_items` 统一读模型（VIEW + 链接列，物理表不 DROP）。

**上一版 v0.26.40** 幕后资产闭环 P0–P3——侧栏生成影响徽章、KG 摘要进默认续写、MCP 降级设置扩展、SceneEditor 管线统一、MemoryFacade、生成链路覆盖率。

**上一版 v0.26.39** 幕后信息架构全面重排——侧栏五组分类、数据洞察合并、设置七 Tab 重组。

**上一版 v0.26.37** 修复幕前「保存中」常亮与顶部字数不随正文增长——根因是 `update_scene` IPC 参数形状错误，以及 AI 追加路径未刷新字数/未调度自动保存。

**上一版 v0.26.34** 修复后台提示词页面批量导入参数错误（`promptId` → `prompt_id`），新增「打开目录」按钮以在系统文件管理器中打开当前 prompts 资源目录，并新增「刷新」按钮重新加载提示词列表。

**上一版 v0.26.33** 补齐综合优化计划阶段 2/3/4 的具体 UI/解耦缺口——知识图谱实体可归档、关系可删除；角色关系卡片新增删除按钮；前端 `frontstage` 与 `components/EditorSettings` 解耦。

**上一版 v0.26.30** 热修复旧数据库在 v0.26.28 迁移框架切换后可能缺失 `characters.scenes.world_buildings.kg_entities` 表的 `source` / `is_auto_generated` 列的问题——新增 V103 迁移与 `init_db` 启动兜底修复，确保 Genesis 与资产查询不再报 `no such column: source`。

**上一版 v0.26.29** 热修复 prompts 外部化后的策略选择 JSON schema 不匹配——`selector.rs` 新增 `LegacyStrategyResponse` 兜底解析，修复 Genesis「选择创作策略」步骤 `VALIDATION_FAILED: missing field rationale` 错误。

**上一版 v0.26.26** L2 资产补齐与领域层止血——角色编辑/关系 CRUD、L2 创世溯源徽章、Story System 合同播种状态卡、StorySystem.tsx 拆分、Repository 层 trait 化与拆分。

**上一版 v0.26.25** Backstage Genesis 可观测性与测试基线——GenesisPanel 动态步骤/非致命 errors/L1 创作路径引导/Wizard 重复建故事修复/仪表盘统计卡可点击。

- **v0.26.19 Phase 1（P0 竞态与契约）**：修复 Gap B（空 finalContent 不锁 delivered）、P0-2（角色世界观上下文闭包捕获竞态——character 提示词读取 `bundle.world_building` 恒为空，改为先 await world 拿真实 `world_concept`）、P0-3（ChapterSwitch delivered 时序——懒加载失败不标记 delivered）。
- **v0.26.19 Phase 2（P1 架构对齐）**：后台错误可观测性（`GenesisContext.errors` → `genesis_runs.steps_json` + `genesis-warnings` 事件 → 前端 toast）；mutex 中毒锁加固；`window/mod.rs` 与 `FrontstageEvent.ts` 注释对齐 auto-accept 真实路径。
- **v0.26.19 Phase 3（测试加固）**：8% 重试闸门 + ChapterSwitch payload 提取纯函数 + 契约测试；前端 Gap C + 状态机端点测试；**跨层共享 trim golden fixture**（Rust + TS 双跑锁定 `trim_self_repetition` 跨层一致性）。
- **v0.26.19 Phase 4（代码整洁）**：`*_future` → `*_gen` 重命名；`AppConfig::load` 去重；`appendAiContent` skip 路径不 `markAccepted`；Gap C 重复入站也跳过 setContent。

**上一版 v0.26.18**：加固 Genesis 第一章重复的三个残留竞态缺口：


- **根治「新写小说时第一章内容重复」**：替代 v0.26.7–v0.26.14 的散布补丁，从生成侧与前端的两个独立根因进行结构性修复。
  - **生成侧验证闸门**：`genesis.rs` 检测 LLM 输出自重复比例，≥8% 时用更强 anti-repeat 指令重试；prompt 模板新增「结构纪律」段，明确禁止首尾回环与整章重复。
  - **前端单写者状态机**：`FrontstageApp` 将 `genesisAutoAcceptedRef` 布尔替换为 `idle → generating → delivered` 三态状态机，`generating` 态阻塞外部内容投递，`delivered` 态阻塞幽灵文本恢复，消除多路径并发导致的重复叠加。
  - `textCleanup` 与 Rust `trim_self_repetition` 统一 KMP 最长 border 检测，覆盖全部写入路径。

- **修复 Issue #4：应用启动 panic/Windows 闪退**：当应用数据目录不可写导致 `init_db` 失败时，`GatewayExecutor` 不再通过 `state::<DbPool>()` 读取未 manage 的 pool，改为由 `setup` 显式传入 pool 并仅在 pool 可用时初始化网关，避免启动时 panic。

- **清理 CI 格式检查失败**：已全局运行 `cargo +nightly fmt` 与 `prettier --write`，`cargo +nightly fmt -- --check` 与 `npm run format:check` 均已通过。

**上一版**：v0.26.13 修复 Genesis 第一章渲染层视觉重复（幽灵容器残留）；v0.26.12 修复角色列表为空/未加载时的幕前崩溃与订阅状态空值；v0.26.11 修复 Genesis 第一章 store-editor 失步与崩溃隐患：

- **修复「新写小说时第一章内容重复」深层根因**：v0.26.10 已确保数据层面只追加一次，但追加后 store 依赖 200ms onChange debounce 回写，当 `latestContentRef` 与编辑器 HTML 指纹相同时 `handleContentChange` 会提前返回，导致 store 长期为空，后续外部同步/章节切换可能引发视觉重复或内容抖动。v0.26.11 在 `appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被外部同步 effect 再次 setContent。
- **修复「写完后过会儿页面崩溃」**：确认 `src-tauri/tauri.conf.json` 的 `devUrl` 指向 `http://localhost:5173`，避免 `cargo tauri dev` 加载陈旧 `dist` 中的旧代码触发崩溃；同时通过 store-editor 同步消除状态漂移引发的渲染异常。

> **上一版**：v0.26.9 将重复检测基准从 `editorRef.getText()` 改为 `latestContentRef.current`，覆盖 DOM 滞后竞态；v0.26.8 修复 pipeline-complete 先加载 DB 正文、smart_execute 后返回 final_content 的竞态重复；v0.26.7 修复 React #185 页面崩溃与 ChapterSwitch 路径下的重复。

> **上一版**：v0.24.9 修复 TipTap 渲染错误边界与接受后 30s 禁止外部 setContent，进一步根治内容重复问题。v0.23.74 完成场景优先架构迁移——`scenes.content` 为唯一叙事真相源。

> **上一版**：v0.22.4 异星球末世生存复合题材创作流程优化（GenreResolver 题材解析 + 意图图资产发现 + 模型网关资产标签调度 + TimeSliced 次要题材补强）。v0.22.0 TimeSliced 全资产注入 + Inspector 全资产注入 + 意图调度接线 + 算力档案消费 + 资产→生成参数规则映射。

> 🐛 v0.13.0 引入**分时介入架构**，解开 AI 长篇小说创作中「质量与速度不可兼得」的根本矛盾：

> 🌿 第一性原理：**把大灾难变成即时可见的小债务。** 蚂蚁搬家，不积巨石。

AI 写长篇小说，强化专业资产（合同/伏笔/Inspector/记忆）就慢，放松就崩。v0.13.0 把"写"和"审"解耦成三条独立时间线：

1. **写作时刻（< 15s 秒出正文）**：`WriteTimeBundle` 只带最小约束（合同红线 + 角色核心 + 场景大纲 + 题材反模式），直连 LLM 单轮生成，立即返回。不跑 Preflight 补合同、不跑 Inspector、不跑 Rewrite。
2. **审计时刻（后台 30-90s）**：正文返回后，`AuditExecutor` 在后台异步跑 7 维 Inspector，发现的问题以 **inline 标注**（ai_audit 类型，按 severity 红黄蓝着色）回流到编辑器。用户当场处理小债，不让它滚成大灾难。
3. **洞察时刻（每 5 段深度报告）**：`InsightExecutor` 汇总追读力趋势 + 追读债务 + 标注盘点，产出整体健康度报告，在「叙事分析」页呈现。

顶栏新增**债务指示器**，实时显示未处理标注数，超阈值红色警告。

**Phase 0 实测验证**（qwen3.6-35b，3 场景 A/B 盲测）：最小约束 vs 全量资产平均质量差距仅 **7.9%**（< 30% 阈值），且会被后台审计追平。证实"慢的根源不是资产量，而是同步链路堆叠的 Inspector/Rewrite"。

设计文档见 [`docs/plans/2026-06-14-time-sliced-intervention-design.md`](./docs/plans/2026-06-14-time-sliced-intervention-design.md)，验收清单见 [`docs/time-sliced-architecture-qa-checklist.md`](./docs/time-sliced-architecture-qa-checklist.md)，完整变更见 [`CHANGELOG.md`](./CHANGELOG.md)。

<details>
<summary>📦 v0.12.0 性能重构（点击展开）</summary>

v0.12.0 针对「智能创作无处不在的卡顿、生成无输出」进行系统性性能重构：

1. **后端生成链路止血**：本地/局域网模型默认单候选 + 全局并发限流，候选总超时硬上限 90s；LLM 连接/生成超时拆分；上下文准备、SQLite 高频路径 spawn_blocking 化；全局 Mutex 替换为 OnceLock/RwLock。
2. **前端响应与大数据量优化**：生成状态精确显示 + 可靠取消；移除高频心跳；场景/章节分页加载；sync-event 批量失效；文思分析移入 Web Worker；RichTextEditor HTML 序列化节流。
3. **架构级重构**：统一 `generation-status` 事件；知识图谱 viewport 裁剪 + LOD；Agent 编排结构化 trace；后台 Ingest 任务 Semaphore + 取消令牌；引入 `tiktoken-rs` 真实 tokenizer 与上下文预算。

修复计划见 [`PERFORMANCE_FIX_PLAN.md`](./PERFORMANCE_FIX_PLAN.md)，阶段验证报告见 `QA-Stage1-report.md`、`QA-Stage2-report.md`、`QA-Stage3-report.md`。

</details>

---

## 📖 用户指南

> 以下基于当前版本实际界面截图整理，持续更新。完整图文版见 [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md)。

### 一、产品概览

**草苔 StoryForge** 将创作流程分为两大空间：

| 空间                   | 作用                                  | 适合场景                 |
| ---------------------- | ------------------------------------- | ------------------------ |
| **幕后（Backstage）**  | 管理故事、角色、场景、世界观、AI 配置 | 规划、整理素材、配置模型 |
| **幕前（Frontstage）** | 沉浸式写作界面，专注正文创作          | 码字、与 AI 对话续写     |

核心思路：幕后把创作要素结构化管好，幕前让你专注写字，AI 在需要时介入，不打断心流。

---

### 二、幕前写作界面

![幕前写作](docs/product-screenshots/00_frontstage.png)

极简、全屏的写作环境，唯一目的就是让你专注码字。

#### 顶部状态栏

| 元素     | 作用                               |
| -------- | ---------------------------------- |
| **草苔** | 返回幕后                           |
| **字数** | 当前章节字数 / 总字数              |
| **18px** | 当前字号，点击可调                 |
| **色调** | 暖赭 / 冷青 / 琥珀 / 靛紫 四种配色 |
| **设置** | 打开设置 / 幕后工作室              |
| **温**   | 文思模式切换                       |

#### 中间编辑区

- 点击"开始写作…"即可输入。
- 支持富文本格式。
- 自动保存。

#### 底部 AI 输入栏

- 输入任意指令，例如"帮我续写下一段""把这段改得更紧张""加入一个意外转折"。
- 按回车或点击纸飞机发送。

#### 文思模式

点击右上角 **温** 切换 AI 介入程度：

- **被动**：只在发指令时响应。
- **主动**：适时给出萤火提示（下一句建议、情节提醒）。

---

### 三、全局导航

左侧边栏是所有功能的入口，任何页面都可以一键切换。

![仪表盘](docs/product-screenshots/01_dashboard.png)

| 按钮             | 作用                         |
| ---------------- | ---------------------------- |
| **开幕前写作**   | 快速打开「幕前写作」窗口     |
| **仪表盘**       | 回到首页，查看统计与快捷入口 |
| **故事**         | 管理所有故事项目             |
| **角色**         | 管理登场角色与关系           |
| **世界构建**     | 设定世界观、势力、规则       |
| **场景**         | 管理场景（情节单元）         |
| **知识图谱**     | 可视化角色/地点/事件关系     |
| **技能**         | 配置 AI 辅助技能             |
| **MCP**          | 连接外部模型/工具            |
| **拆书**         | 分析参考书籍结构             |
| **任务**         | 查看后台 AI 任务队列         |
| **伏笔看板**     | 追踪伏笔埋设与回收           |
| **叙事分析**     | 诊断故事节奏与结构           |
| **Story System** | 高级契约与版本管理           |
| **用量统计**     | AI 调用与 Token 消耗         |
| **写作统计**     | 字数、时长、写作习惯         |
| **设置**         | 模型、账号、通用偏好         |

---

### 四、仪表盘 — 创作起点

![仪表盘](docs/product-screenshots/01_dashboard.png)

打开应用后首先进入这里。核心元素：

- **快捷创建**：
  - **AI 创建故事** —— 输入一句话创意，AI 生成故事框架（含大纲、角色、场景）。
  - **手动创建** —— 自己填写标题、简介、类型，从零开始。
- **统计卡片**：故事数 / 角色数 / 场景数，点击可跳转。
- **GENESIS 运行记录**：显示 AI 自动生成任务的运行历史。
- **开始创作引导**：没有故事时，下方会出现"开始你的创作之旅"，提供 AI/手动两种创建入口。

**典型路径**：打开应用 → 仪表盘 → AI 创建故事 → 输入创意 → 进入「故事」页继续完善。

---

### 五、故事 — 作品管理中心

![故事页](docs/product-screenshots/02_stories.png)

"故事"是创作的顶层容器。一本小说、一个短篇，都是一个故事。

首次使用时页面为空，需要先创建故事。有数据后：

- 故事卡片/列表展示标题、类型、进度、最近编辑时间。
- **打开** / **编辑** / **删除** / **导出** 等操作。

选择一个故事后，左侧底部会显示"当前编辑"，角色、场景、世界观等页面自动切换到该故事的数据。

---

### 六、角色 — 人物资料库

![角色页](docs/product-screenshots/03_characters.png)

管理系统化的人物设定：

- **基本信息**：姓名、性别、年龄、外貌。
- **性格与背景**：性格标签、核心驱动力、出身、目标。
- **关系网络**：与其他角色的关系可视化。
- **AI 生成角色**：输入一句话，AI 扩展成完整人设。

这让 AI 在续写时严格遵循人设，避免"角色崩坏"。

---

### 七、场景 — 情节单元

![场景页](docs/product-screenshots/04_scenes.png)

"场景"是故事的最小情节单位，类似"一场戏"。

- 场景卡片：标题、所属章节、出场角色、地点、状态。
- **新增 / 编辑 / AI 扩写 / 排序**。
- 把"写一章"拆成"写几场戏"，降低创作心理压力。

---

### 八、世界构建 — 设定资料库

![世界构建](docs/product-screenshots/05_world_building.png)

存放世界观、势力、地理、规则等背景设定。支持分类浏览、AI 生成世界观、关联角色/场景。

保证奇幻/科幻/架空作品的设定不自相矛盾，防止 AI "吃书"。

---

### 九、知识图谱 — 关系可视化

![知识图谱](docs/product-screenshots/06_knowledge-graph.png)

把角色、地点、事件、势力变成一张可交互网络图：

- 拖拽节点、缩放画布。
- 点击节点查看详情。
- 筛选显示某类节点。
- **手动 CRUD**：图例面板可新建实体，实体详情面板可添加关系。

直观发现"谁太久没出场""哪条线索忘了回收"。

---

### 十、技能工坊 — AI 辅助技能

![技能页](docs/product-screenshots/07_skills.png)

管理和配置可复用的 AI 技能模板：

- **导入技能**：导入别人分享的技能配置。
- **分类筛选**：全部 / 写作 / 分析 / 角色 / 情节 / 风格 / 世界观 / 导出 / 集成 / 自定义。
- **技能卡片**：名称、描述、适用场景、启用开关。

在幕前写作时，可随时调用已启用的技能（如"续写""润色""生成大纲"）。

---

### 十一、MCP — 外部工具连接

![MCP](docs/product-screenshots/08_mcp.png)

MCP（Model Context Protocol）让草苔连接外部模型或数据源，扩展 AI 能力。例如连接专门的"古文润色"模型或私有知识库。

---

### 十二、拆书 — 学习经典结构

![拆书](docs/product-screenshots/09_book-deconstruction.png)

上传参考小说，AI 自动分析：

- 整体结构（三幕式、英雄之旅等）
- 章节节奏与高潮分布
- 角色出场频率
- 核心主题

把"凭感觉写"变成"有参照地写"。

---

### 十三、任务 — 后台作业队列

![任务页](docs/product-screenshots/10_tasks.png)

当 AI 执行批量操作（批量润色、整书生成）时，会在这里显示进度。

- **状态筛选**：全部 / 执行中 / 等待中 / 已完成 / 失败。
- **新建任务**：手动发起后台 AI 任务。

你可以关闭界面去做别的事，回来在任务页查看结果。

---

### 十四、伏笔看板 — 线索回收

![伏笔看板](docs/product-screenshots/11_foreshadowing.png)

管理伏笔的全生命周期：

- **已埋下 / 已回收 / 待回收 / 废弃** 四态看板。
- 创建伏笔时填写描述、预期回收章节、重要性。
- 关联到具体场景。

防止"开头精彩、结尾烂尾"，确保每条线索都有交代。

---

### 十五、叙事分析 — 结构诊断

![叙事分析](docs/product-screenshots/12_narrative-analysis.png)

用 AI 诊断故事的叙事健康度：

- 节奏曲线（每章紧张度变化）
- 角色戏份分布
- 情节密度（对话/动作/描写比例）
- AI 诊断建议

像给小说做体检，发现结构问题再针对性修改。

---

### 十六、Story System — 高级契约系统

![Story System](docs/product-screenshots/13_story-system.png)

高级用户功能：

- **契约树**：定义 AI 必须遵守的规则（如"主角不能死""保持第三人称"）。
- **版本记录**：类似 Git 的提交历史，可回溯故事版本。
- **运行时规则**：控制 AI 生成的行为边界。

让 AI 在长篇幅创作中保持高度一致性。

---

### 十七、用量统计与写作统计

![用量统计](docs/product-screenshots/14_usage-stats.png)

**用量统计**：AI 调用次数、Token 消耗、按模型/功能拆分。适合关注 API 成本的用户。

![写作统计](docs/product-screenshots/15_writing-stats.png)

**写作统计**：每日字数、活跃时段、连续创作天数、平均写作速度。帮助你建立稳定输出节奏。

---

### 十八、设置 — 模型与偏好

![设置页](docs/product-screenshots/16_settings.png)

配置 AI 模型和应用偏好：

- **模型管理**：添加、删除、测试 LLM 连接（聊天/嵌入/多模态/图像）。
- **Agent 配置**：为不同 AI Agent 分配模型。
- **创作方法论**：选择雪花法、英雄之旅等创作框架。
- **工作流**：配置自动化流程。
- **通用设置**：主题、语言、自动保存、字号、行高。
- **数据统计**：查看本地功能使用统计。
- **账号与登录**：管理账号和订阅。

**首次使用建议**：进入 **模型管理** → **添加聊天模型** → 填写 API 地址和 Key → 测试连接 → 完成后即可在幕前调用 AI。

#### 提示词注册表（v0.19.0 新增）

进入 **设置 → 提示词** 可查看和编辑全部 35+ 个 AI 提示词：

- **分类浏览**：15 个分类折叠面板（写作核心 / 审校 / 评点 / 规划 / 分析 / 世界观 / 角色 / 叙事 / 方法论 / 技能 / 记忆 / 知识 / 探测 / 系统 / 其他）
- **实时搜索**：按提示词 ID、名称、描述或内容关键词搜索
- **编辑覆盖**：点击任意提示词展开编辑器，修改后保存即可覆盖默认提示词
- **默认值预览**：已覆盖的提示词显示内置默认值（只读），方便对比修改
- **批量重置**：一键恢复所有提示词到默认状态
- **模板变量**：提示词中 `{{variable}}` 形式的变量会自动高亮

所有修改即时生效，下次 AI 调用自动使用新提示词。

---

### 十九、快速上手

第一次使用草苔，建议按以下顺序：

1. 打开应用 → 看到仪表盘。
2. 点击 **AI 创建故事** → 输入创意一句话 → 等待 AI 生成框架。
3. 进入「故事」页 → 确认新建的故事。
4. 进入「角色」页 → 添加 2-3 个核心角色。
5. 进入「场景」页 → 创建第一章的关键场景。
6. 点击左侧 **开幕前写作** → 在幕前界面写第一章。
7. 卡壳时用底部 AI 输入栏求助。
8. 返回幕后「叙事分析」查看结构诊断。

---

### 二十、常见状态

- **顶部红色提示条"无法连接到本地服务"**：表示前端未连上后端。请等待几秒后点击"重试"，或重启应用。
- **左下角"登录"**：未登录状态，点击可登录账号。
- **右上角更新通知**：有新版本时弹出，可选择安装或忽略。

---

## 🚀 安装与运行

### 下载预构建版本

releases 页面提供 Windows / macOS 安装包，下载后直接安装即可。

### 从源码运行

需要安装 [Node.js](https://nodejs.org/)（推荐 20 LTS）和 [Rust](https://rustup.rs/)。
仓库通过 `rust-toolchain.toml` 固定 Rust 版本为 **1.95.0**，`rustup` 会自动下载对应工具链。

```bash
# 1. 克隆仓库
git clone https://github.com/91zgaoge/StoryForge.git
cd StoryForge

# 2. 安装前端依赖
cd src-frontend && npm install

# 3. 安装 Tauri CLI 并运行桌面应用
cd ..
npm install -g @tauri-apps/cli
cargo tauri dev
```

> **注意**：`Cargo.lock` 已纳入版本控制。如需升级依赖，请在本地验证 `cargo clippy` / `cargo test` 通过后再提交。

### 仅运行前端（开发调试）

```bash
cd src-frontend
npm run dev
```

然后在浏览器打开 `http://localhost:5173/`。

---

## 🏗️ 技术栈

- **前端**：React 18 + TypeScript 5.8 + Vite 6 + Tailwind CSS 3
- **桌面框架**：Tauri 2.4（Rust 后端 + Web 前端）
- **编辑器**：TipTap / ProseMirror
- **状态管理**：Zustand + TanStack Query
- **知识图谱**：ReactFlow
- **向量存储**：LanceDB + SQLite
- **LLM 适配**：OpenAI / Anthropic / Ollama / 自定义本地 API
- **提示词注册表**（v0.19.0）：35+ 内置提示词统一注册表，15 分类，前端完整管理，运行时覆盖生效
- **分时介入架构**（v0.13.0）：三条时间线（写作/审计/洞察）解耦，解开质量与速度矛盾
- **Context Rot 显式防御**（v0.25.0）：`ContextPrioritizer` 按 Critical/High/Normal/Background 排序系统提示词，并在结尾双重锚定关键约束，缓解长上下文中的 "Lost in the Middle"
- **四级错误分类与恢复**（v0.25.0）：`ErrorSeverity` Fatal/Retry/Degraded/UserAction + 指数退避重试 + 降级回退 + `AgentInterruptionModal` 显式中断 UI

---

## 📚 更多文档

| 文档                                                                                                                     | 说明                                 |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------ |
| [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md)                                                                             | 完整用户指南（含全部截图与详细说明） |
| [`CHANGELOG.md`](./CHANGELOG.md)                                                                                         | 版本更新日志                         |
| [`ARCHITECTURE.md`](./ARCHITECTURE.md)                                                                                   | 系统架构设计（含分时介入架构章节）   |
| [`docs/plans/2026-06-14-time-sliced-intervention-design.md`](./docs/plans/2026-06-14-time-sliced-intervention-design.md) | 分时架构设计文档（Phase 0 已验证）   |
| [`docs/time-sliced-architecture-qa-checklist.md`](./docs/time-sliced-architecture-qa-checklist.md)                       | 分时架构 QA 验收清单                 |
| [`AGENTS.md`](./AGENTS.md)                                                                                               | 开发代理指南                         |

---

## 📸 截图清单

所有界面截图均由 CDP 自动截取，保存在 [`docs/product-screenshots/`](./docs/product-screenshots/)：

| 文件名                       | 页面         |
| ---------------------------- | ------------ |
| `00_frontstage.png`          | 幕前写作     |
| `01_dashboard.png`           | 仪表盘       |
| `02_stories.png`             | 故事         |
| `03_characters.png`          | 角色         |
| `04_scenes.png`              | 场景         |
| `05_world_building.png`      | 世界构建     |
| `06_knowledge-graph.png`     | 知识图谱     |
| `07_skills.png`              | 技能工坊     |
| `08_mcp.png`                 | MCP          |
| `09_book-deconstruction.png` | 拆书         |
| `10_tasks.png`               | 任务         |
| `11_foreshadowing.png`       | 伏笔看板     |
| `12_narrative-analysis.png`  | 叙事分析     |
| `13_story-system.png`        | Story System |
| `14_usage-stats.png`         | 用量统计     |
| `15_writing-stats.png`       | 写作统计     |
| `16_settings.png`            | 设置         |

---

## 🤝 参与贡献

欢迎通过 Issue 和 Pull Request 参与项目。大型改动建议先阅读 [`AGENTS.md`](./AGENTS.md) 和 [`ARCHITECTURE.md`](./ARCHITECTURE.md)。

---

<p align="center">
  Made with 🌿 by StoryForge Team
</p>
