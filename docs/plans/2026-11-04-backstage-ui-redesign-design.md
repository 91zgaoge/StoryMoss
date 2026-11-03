# 幕后工作室 UI 全面重设计（设计文档）

> 日期：2026-11-04 ｜ 状态：待批准 ｜ 实施目标版本：v0.81.0 起，分阶段发版
> 范围：幕后（index.html 应用）全部界面。不含幕前（frontstage/）、不含后端与 IPC。
> 前置调研：本仓库 docs/plans/2026-07-27-ui-redesign-design.md、2026-08-1x AI 原生组件系列、2026-08-13/14 墨纸机械演进。本文档在幕后范围内接续并取代其未完成部分。

## 0. 设计定调

- 产品类型：**密集型创作工作台**（不是营销页）。密度优先、动效克制。
- 模式：**重设计-演进（Preserve）**。保留：cinema 暗色基底、12 套传统色主题、film-grain 质感、现有 `--ai-*` 语义令牌、信息架构 5 组 23 项、全部页面与 view key。
- 拨盘：`DESIGN_VARIANCE 3 / MOTION_INTENSITY 3 / VISUAL_DENSITY 6`。
- 技术栈不动：Tailwind + tokens.css 自定义令牌 + 现有依赖（lucide-react 已深度使用，保留；不引入新组件库）。

## 1. 现状诊断（六类问题，均带实证）

### A. 双壳并行
- 全局 `Sidebar`（5 组 23 项）与 `StudioNavRail`（6 项图标栏）并存；只有 Dashboard 和 Settings 用 NavRail + 自带 h-14 顶栏（`pages/Dashboard.tsx:202`、`pages/Settings.tsx:153`），其余页面用 Sidebar。用户在两套壳之间获得两种导航心智。

### B. 布局会坏（变形/出窗/无滚动）
- 最小窗口 800×600 下必然溢出：Scenes 双栏下限 780px + Sidebar（`pages/Scenes.tsx:205,221`）；Foreshadowing、NarrativeAnalysis 固定 `grid-cols-4`（`Foreshadowing.tsx:721`、`NarrativeAnalysis.tsx:197`）；Stories 场景条固定 `grid-cols-3`（`Stories.tsx:156`）。
- 27 个页面中 11 个零断点。
- 全仓无 sticky 表头；仅 AiFilterTable 有横向滚动。
- ≥20 个手写弹窗，其中 9 个无 max-height/内滚（WorldBuilding×2、Dashboard、Characters、Stories×3、GenresTab、Skills）。
- `100vh` 魔法减法：`Logs.tsx:220`（calc(100vh-320px)）、`TracingPanel.tsx:236`。
- 手算宽度：`Stories.tsx:289`（100/scenes.length%）、ConfidenceIndicator 内联 width。

### C. 组件分裂（同一件事多种写法）
- `ui/Button.tsx` 存在但原始 `<button>` 占主导（AgencyStudio 22 处、Corpus 18 处…）；Button 还引用了未定义的 `cinema-100/300/50` token。
- 无共享 Input/Select/Textarea：91 个裸控件、同一串 class 复制 4 份（`Characters.tsx:157-199`）。
- 无共享 Modal：遮罩、内边距、限高全不统一。
- 卡片混用 Card / 手写 bg-cinema-800 面板 / ai 卡；圆角混用 rounded-lg×175、xl×86、full×83、2xl×4。
- 加载三范式（spinner span / 整页文字 / AiThinking）、空态两范式（EmptyHint / 裸 `<p>`）。
- UserMenu 整体浅色（stone/bg-white），嵌在暗色 Sidebar 内（`UserMenu.tsx:19-53`）。

### D. 层级与令牌漂移
- 内联 hex 约 43 处（NarrativeAnalysis SVG、story-system 状态三色、Toaster 底色 `main.tsx:66-68`、滚动条 `index.css:75-80`）。
- Tailwind 默认灰大规模直用（text-gray-400×396、text-red-400×105…），脱离 cinema/status 令牌。
- z-50 平铺 31 处，模态/Toast/连接条/更新条同层，层级靠渲染顺序。

### E. 状态缺失
- 无骨架屏；页面级无 ErrorBoundary（仅根级一个）；错误常是裸 `text-red-500` 段落。

### F. 文案冗余（38 条实证，附录 A）
- 「点击右上角按钮…」「还没有 X，添加一个吧」「此页面用于…」式教学句；60+ 字机制说明段；与 label 重复的 helper。
- 同时存在**必须原样保留**的关键文本：24 条错误提示、18 条危险确认、14 条输入约束、18 组状态词（附录 B）。

## 2. 设计原则（本次重设计的四条铁律）

1. **布局先于文字**：界面不清晰时，只允许改布局、分组、间距、层级；禁止用加字解决。
2. **文字三问**：任何新增文字必须依次通过：①是否提供界面表达不了的必要信息？②界面/字段本身是否已表达？③能否更短？三问有疑虑时先问用户，不默认加。
3. **关键文本冻结**：附录 B 的 label、错误提示、输入限制、危险确认，一句不少、原样保留；删除冗余句时不得连带删除其中的约束半句（如 Stories 大纲 placeholder 里「请确保角色与情节走向一致」）。
4. **防变形是验收项**：800×600 为最小验收尺寸；违反 §7 布局契约的代码不得合入。

## 3. 信息架构与导航

- **双壳保留（已拍板）**：Dashboard/Settings 的 StudioNavRail + h-14 顶栏保留，做视觉升级与全局 Sidebar 对齐（active 金条指示、图标 tooltip、同套间距与字重），不做合并。
- **导航重组（已获准，改动清单见 §3.1）**：分组由 5 组调整为 5 组新划分，条目 label 全部不变；view key、icon、impact 徽章、deprecated 映射不变。
- 两侧导航共同升级：
  - active 指示：左侧 2px 金色竖条 + 图标/文字金色（替代仅背景色）。
  - 窄态（<lg，w-20）补 tooltip 显示名称；组标题窄态显示为分隔点。
  - 当前故事卡压缩为两行（题名 + 字数/章数），溢出 truncate。

### 3.1 导航重组清单（唯一需要改 NAV_GROUPS 的地方）

| 现分组 | 条目 | 新分组 |
|---|---|---|
| 创作 | dashboard、stories、agency-studio | 创作（不变） |
| 创作工具 | skills、book-deconstruction | 并入「创作」 |
| 故事资产 | scenes、characters、corpus、world_building、foreshadowing、knowledge-graph、story-system | 故事资产（不变，7 项） |
| 诊断 | tasks、maintenance、cascade-center、logs、tracing | **运维**（新组，5 项） |
| 诊断 | usage-stats、narrative-analysis、agency-eval、agency-learning、intention-graph | **分析**（新组，5 项） |
| 系统 | settings | 系统（不变） |

- 结果：创作 5 ｜ 故事资产 7 ｜ 运维 5 ｜ 分析 5 ｜ 系统 1 = 23 项，无增删。
- 组名变化：创作工具（删）、诊断（拆分为运维/分析）。条目 label 一个不改。
- 连带更新：`Sidebar.tsx` NAV_GROUPS 与折叠默认（运维/分析默认折叠，同现诊断组）；`FrontstageHeader.test`/`Sidebar` 相关测试的分组断言；USER_GUIDE/ARCHITECTURE 中的分组描述。
- 每页统一 **PageHeader**：左「标题（+ 必要的计数值）」、右「主操作 + 次操作」；不再有各页自造的标题排法。PageHeader 不带介绍段。

## 4. 页面模板（四个，全站归一）

| 模板 | 结构 | 适用页 |
|---|---|---|
| T1 列表页 | PageHeader → 工具行（筛选/搜索/主操作）→ 内容网格/表格 → EmptyState | Stories、Characters、Foreshadowing、Skills、Tasks、Maintenance、CascadeCenter、Logs、AgencyEval |
| T2 主从页 | `grid lg:grid-cols-[minmax(280px,340px)_minmax(0,1fr)]`，<lg 单列堆叠；右栏自身滚动 | Corpus、Scenes、KnowledgeGraph（统计+图）、StorySystem |
| T3 工作台页 | `grid lg:grid-cols-[…_minmax(0,1fr)_…]` 三栏，全高 `min-h-0`，各栏独立滚动 | AgencyStudio、AgencyLearning、TracingPanel、IntentionGraphDiagnostics |
| T4 表单/设置页 | 单列 `max-w-3xl`，分区 Section（标题一行 + 内容），sticky 保存条（如需要） | Settings 13 面板、WorldBuilding、BookDeconstruction |

- 页面容器：滚动型页 `px-6 py-6 max-w-[1440px] mx-auto`；全高型页（T2/T3）`h-full min-h-0 p-4`。
- 断点回退：所有 `grid-cols-N` 必须写 `grid-cols-1 md:grid-cols-2 xl:grid-cols-N` 或等价；禁止无断点的固定列数。
- 具体修复：Scenes 改 `grid lg:grid-cols-[minmax(280px,340px)_minmax(0,1fr)]`；Foreshadowing/NarrativeAnalysis → `grid-cols-2 xl:grid-cols-4`；Stories 场景条 → `flex gap-2 overflow-x-auto`（横向滚动优于挤压）。

## 5. 组件系统（新建 8 个共享件 + 收敛 3 个旧件）

新建（`src-frontend/src/components/ui/`，全部走现有 token）：
1. `Modal.tsx`：遮罩统一、`max-h-[85dvh]` 内容区内滚、Esc 关闭、`danger` 变体（红色标题条，承载全部危险确认）；尺寸 sm/md/lg。
2. `PageHeader.tsx`：标题 + count + 右侧 actions 插槽。
3. `Field.tsx`：label 上置 + 控件 + 错误下置 + `hint` 插槽（hint 只放附录 B 类约束文本）。
4. `Input.tsx / Select.tsx / Textarea.tsx`：统一 class（bg-ai-field 系），替换 91 个裸控件中 Phase 覆盖到的部分。
5. `Table.tsx`：`overflow-x-auto` + sticky thead + 行 hover + 空/加载/错误三态插槽。先落地 Logs、UsageStats、AiRecordsTable 场景。
6. `EmptyState.tsx`：图标 + 一句话（≤20 字）+ 可选主操作按钮。替代 EmptyHint 与裸 `<p>`；空态不再出现教学段落。
7. `Skeleton.tsx`：三种形状（卡片格、表格行、详情栏）。
8. `ConfirmDialog.tsx`：基于 Modal danger，`confirm()` 的 18 处迁移过来，**文案逐字保留**（附录 B2）。

收敛：
- Button：7 变体 → 4（primary/secondary/ghost/danger）×3 尺寸；修复未定义 token；补 `active:scale-[0.98]`、`focus-visible:ring`；新代码一律用 Button。
- Toaster：`main.tsx` 硬编码 hex → status/ai token；icon 提示仅保留有语义的（📝⚠️ 类删除，改为语义色边）。
- Card/Panel：圆角契约（见 §6）后保留两者，手写面板逐步替换（仅在新触及的页面内替换，不做全仓盲扫）。

## 6. 视觉规范

- **z-index 契约**（写入 tokens.css 注释）：nav 40 ｜ launcher/连接条 45 ｜ overlay 50 ｜ modal 60 ｜ toast 70。31 处 z-50 按此归层。
- **圆角契约**：控件/输入 rounded-lg(8px)；卡片/面板 rounded-xl(12px)；full 仅头像、徽章、pill、开关。新代码不得出现 2xl 与混合。
- **颜色**：
  - 删除 43 处内联 hex → `--status-*` / cinema token（NarrativeAnalysis SVG、story-system 三色、Toaster、滚动条、UserMenu 暗色化）。
  - 本版不大规模替换 gray-*（量大风险高）；仅约定：新增代码禁止裸 gray/red/green，用 status/ai token。gray 收敛列为后续独立版本。
  - accent 唯一：cinema-gold（主题切换时跟随 backstageThemes 覆写）。
- **文字**：正文用现有 LXGW/Noto Serif 栈不动；数值/表格列 `tabular-nums`；标题引入 500/600 两档字重建立层级（现基本只有 400/700）；大标题 `tracking-tight`。
- **间距**：4/8 标尺；页面级 py-6、组件间 gap-4、组内 gap-2；禁止魔法数。
- **动效**：沿用现有 keyframes 与 `--transition-press`；新增动效仅限 hover/active/骨架 shimmer；全部尊重 prefers-reduced-motion（token 已有降级）。

## 7. 布局契约（防变形/出窗/无滚动，代码评审门）

1. flex/grid 子项默认 `min-w-0 min-h-0`；内容可收缩是默认，不是补丁。
2. 每页有且只有一个滚动容器：要么 main，要么页内明确 panel；禁止双滚。
3. 禁止 `100vh/calc(100vh-Npx)` 魔法减法；高度关系用 `h-full min-h-0` 链表达（Logs、TracingPanel 重写）。
4. 表格必须 `overflow-x-auto`；宽表 thead sticky。
5. 弹窗必须 max-h + 内滚；内容超出滚动，不撑破视口。
6. 文本溢出 `truncate`/`line-clamp` + `title` 原文；数字用 tabular-nums 不换行。
7. 任何 `grid-cols-N` 必须带 <md 回退；任何 min-w ≥ 320px 的并排栏在 <lg 堆叠。
8. 800×600 与 1400×900 双档截图验收（人工 + CDP 截图脚本 `scripts/cdp-inspect.js`）。

## 8. 文案处置

- **删除**：附录 A 全部 38 条（含空态教学句、机制说明段、重复副标题）。空态一律改写为「一句话 + 按钮」，如 Corpus 空态：删整段，仅留「还没有语料」+「导入语料」按钮。
- **保留**：附录 B 全部（B1 错误 24、B2 危险确认 18、B3 输入约束 14、B4 状态词 18 组），迁移到新组件时逐字复制，实施 PR 内附核对表。
- **新增文字预算**：每个页面新增可见文字 ≤ 2 处、每处 ≤ 20 字，且须通过三问；默认为零。
- WorkflowSettings.tsx:39 的技术路径说明（workflows/ 目录）：删面板内说明，改为空态按钮「打开工作流目录」（信息不丢失，文字更短）。

## 9. 实施计划（四阶段，每阶段独立发版可回滚）

| 阶段 | 内容 | 主要文件 | 验证 |
|---|---|---|---|
| P0 基建 | §5 全部共享组件 + Button/Toaster 收敛 + z-index/圆角契约 + §7 契约写入 CONTRIBUTING 注释 | components/ui/*、main.tsx、tokens.css、tailwind.config.js | tsc、vitest（新组件单测）、既有 657/48 基线不降 |
| P1 壳+核心页 | 导航重组（§3.1）+ 双壳视觉统一、Sidebar/NavRail 升级、PageHeader 接入、T1/T2 模板改造：Dashboard、Stories、Characters、WorldBuilding、Scenes、Foreshadowing + 对应附录 A 文案删除 | App.tsx、Sidebar、StudioNavRail、上述页面 | 800×600/1400×900 截图验收 + vitest + playwright |
| P2 数据与诊断页 | Corpus、KnowledgeGraph、Skills、Tasks、Logs、Tracing、Maintenance、CascadeCenter、叙事诊断、故事系统、拆书、Insights + Table/Skeleton 落地 + 对应文案 | 上述页面 | 同上 |
| P3 设置+工作台 | Settings 13 面板（T4 模板 + Field/Input 替换）、AgencyStudio/AgencyEval/AgencyLearning 密度对齐、UserMenu 暗色化、附录 A 设置区文案 | pages/settings/*、Agency* | 同上 + 设置页全部子 tab 遍历截图 |

- 每阶段按仓库规则：版本号统一 bump、CHANGELOG/AGENTS/PROJECT_STATUS/ROADMAP/ARCHITECTURE/TESTING/USER_GUIDE 同步、推 tag 触发 CI。
- 不做的事：不改 IPC/业务逻辑、不改 view key 与导航 label、不动 `--ai-*` 组件 API、不做浅色幕后、不换图标库、不盲扫替换 gray-*、不动幕前。

## 10. 验收清单

- [ ] 导航重组落地（创作5/资产7/运维5/分析5/系统1），条目 label 零改动；双壳视觉一致
- [ ] 800×600 下：Scenes/Foreshadowing/NarrativeAnalysis/Stories/Settings 模型管理无溢出、无挤压变形、均可滚动到底
- [ ] 所有弹窗限高内滚；Esc 可关；危险操作全部走 ConfirmDialog 且文案与附录 B2 逐字一致
- [ ] 所有表格 sticky 表头 + 横向滚动；Logs/Tracing 无 100vh 魔法数
- [ ] 加载一律 Skeleton、空态一律 EmptyState（≤一句话+按钮）、页面级 ErrorBoundary 生效
- [ ] 内联 hex 归零（§6 清单内）；z-index 分层生效；圆角符合契约
- [ ] 附录 A 38 条删除、附录 B 全部保留（PR 核对表）
- [ ] 新增可见文字总量 ≤ 本文预算；tsc/vitest/playwright 基线不降

## 附录 A：删除清单（38 条，file:line + 原文摘要）

Stories：1097「你还没有创建任何故事…」、1040「分步选择 AI 生成选项」；CreationPathGuide 26/35/39 三段功能介绍；Stories:105 placeholder 机制解释句（保留约束半句）。Dashboard:477「保存世界观、角色…」；NovelCreationWizard 226/271/380/412/447 教学与重复副标题。WorldBuilding 359/384/656/789/880；Characters 408/597/811；CharacterRelationshipForm:90。Corpus:418 空态整段；KnowledgeGraph 295/474/493/523；Foreshadowing:867；Tasks:637。story-system：AuditTab:118、AntiAiTab:90、ContractsTab:316。settings：GeneralSettings 563/584/637/711/919/1050-1052、PromptsPanel 461/564、UnifiedModelManager:232、AgentConfig:198、ModelHealthPanel 126/144、RouteSimulator:65、WorkflowSettings:39（改按钮）。组件：SceneEditor 480/516、SceneAuditPanel 50/57、VersionTimeline:569、EditorSettings:317、ExportDialog:324、UpgradeModal:160。诊断页：IntentionGraphDiagnostics:366 科普段、AgencyLearning:180 机制段、StorySystem:81、CascadeCenter:111、Settings:286、Login:81。AgencyStudio:980「暂无任务…」。（AgencyStudio:814 角色语气空态语保留，属产品语气。）

## 附录 B：必须原样保留的关键文本

### B1 错误提示（24 条代表，全量在实施时以 grep `toast.error|setError` 核对）
GenesisPanel 153「打开幕前失败」168「暂停失败」；FrontstageLauncher:53「无法打开幕前界面」；NovelCreationWizard 105/125/145/167/279「生成X失败，请重试/策略加载失败，请返回重试」；SceneEditor 182「请先生成大纲」207「没有可提升为定稿的内容」；CharacterStatePanel:78「更新失败: …」；SkillExecutionPanel:195「执行失败」；KnowledgeGraphView 375「无效的实体类型」389/416/431「创建实体/关系失败、归档实体失败」；BookUploadPanel 31「仅支持 txt、pdf、epub 格式」35「文件大小超过 100MB 限制」；Login:52「邀请码无效或已使用」；Scenes:55「请先选择一个故事」；Corpus:286「请先选择文件」；Dashboard:174「创建失败，请重试」；Stories 611/628；Maintenance 91/205/322；ModelModal 158/176；PromptsPanel 380/349；AgencyLearning:61、KnowledgeGraph:224、AntiAiTab:32。

### B2 危险操作确认（18 条，迁移 ConfirmDialog 时逐字保留）
Stories 565「确定要删除这个故事吗？此操作不可撤销。」579-581 无简介继续创作确认；Characters 335/84/297-302（含生死状态改判双确认）；WorldBuilding 567/582；Scenes:156；Foreshadowing:229；Corpus:233；Tasks:80；BookDeconstruction 70/101；VersionTimeline 388/398；GuidebookDistillationPanel 84/240/566；UnifiedModelManager:131；PromptsPanel:299；Skills:144；KnowledgeGraphView 422/437；GenresTab:39。

### B3 输入限制与校验（14 条）
StyleBlendPanel 45/46/186（5 个上限）；CharacterEditModal 137/143 必填星标+placeholder；Skills:547「必填」；Foreshadowing:667、Tasks:486、AntiAiTab:22「请输入…」；Login:103、UpgradeModal:112 邀请码必填；BookUploadPanel:75 格式与大小；GeneralSettings 611-618/637 字数上限含行为约束；UnifiedModelManager 491-493 建议范围与区间公式；RouteSimulator:105「0 表示不限制」；KnowledgeGraph:598「最多 5 条」；Stories:1261「300-3000字」。

### B4 状态语义词（18 组）
保存中.../保存；正在创建故事...；AI正在发挥创意... 等向导进度组；拆书全阶段进度组（含「此阶段较耗时」）；流式生成中.../打字机效果中.../AI 正在思考...；正在连接服务.../重试；Anti-AI 审查进度组；大纲/草稿生成成功、已提升为定稿；更新器全组（下载/重启安装）；Maintenance 裁决回执组；任务完成/失败；提炼/取消提炼；场景阶段域词（铺垫/上升/高潮/收尾；规划/大纲/起草/审校/定稿）；ErrorBoundary:63 兜底文案；Corpus 固定/已用于创作组；Login:171 法律声明。
