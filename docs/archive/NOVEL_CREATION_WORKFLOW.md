# StoryForge (草苔) 智能化小说创作全流程指南

> **版本**: v3.1.2  
> **目标**: 基于 StoryForge 全功能体系，描绘从灵感萌发到作品出版的完整智能化创作流程，识别当前缺失环节并提出优化方案，确保项目愿景的全面实现。

---

## 一、创作哲学与系统总览

StoryForge 的核心理念是**"越写越懂的创作系统"**。它不是一个简单的文本编辑器，而是一个以**场景化叙事**为骨骼、**增强记忆系统**为血脉、**多智能体协作**为大脑的文学创作操作系统。

### 1.1 双界面工作流

```
┌─────────────────────────────────────────────────────────────┐
│                    创作生命周期全景图                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   [灵感] ──► [配置] ──► [建构] ──► [写作] ──► [打磨] ──► [出版]  │
│      │        │         │         │         │        │     │
│      ▼        ▼         ▼         ▼         ▼        ▼     │
│   AI向导    幕后设置   幕后管理   幕前沉浸   双界协同  多格式导出 │
│            模型/技能  场景/角色  TipTap编辑器 版本/记忆 PDF/EPUB  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、全流程详解：从零到一创作一部科幻小说

以下以创作一部名为**《星际牧歌》**的太空歌剧科幻小说为例，演示如何调用 StoryForge 的每一个功能模块。

---

### Phase 1: 工作室搭建 —— 幕后配置中心

**使用界面**: Backstage (幕后) → 设置页  
**核心功能**: 工作室配置系统 + LLM 模型管理 + 技能系统 + MCP 集成

#### 1.1 初始化创作环境

打开 StoryForge，应用默认进入**幕前 (Frontstage)** 的欢迎界面。点击左侧金色羽毛笔按钮，切换到**幕后 (Backstage)**。

进入**「设置」**页：
1. **模型配置**: 系统自动加载默认的三模型配置
   - **Qwen 3.5 语言模型** (`chat`) → 负责日常对话与文本生成
   - **Gemma 4 多模态** (`multimodal`) → 负责场景画面理解与视觉辅助
   - **BGE-M3 Embedding** (`embedding`) → 负责语义检索与记忆向量化
   
   连接状态指示灯显示绿色，表示本地模型已就绪。

2. **Agent 映射** (规划中): 为不同创作阶段配置专属模型
   - 世界观构建 → 绑定长上下文模型
   - 文风模仿 → 绑定高创造力模型
   - 情节分析 → 绑定逻辑推理模型

3. **技能系统激活**: 进入「技能」页，启用以下内置技能 (Skills)
   - `novel-creation` — 小说创建向导
   - `scene-generator` — 场景生成器
   - `style-mimic` — 文风模仿器
   - `plot-analyzer` — 情节分析器
   - `character-evolution` — 角色成长追踪

4. **MCP 服务接入**: 进入「MCP」页，连接外部服务
   - 连接图像生成 MCP Server (为小说生成概念图)
   - 连接文献检索 MCP Server (为科幻设定查找科学资料)
   - 连接语音合成 MCP Server (将场景对白转为音频试听)

**当前缺失**: Agent 映射功能仅展示 UI，尚未实现模型路由逻辑。建议完善 `AgentModelMapping` 的持久化与调用链路由。

---

### Phase 2: 新世界诞生 —— AI 引导式创作

**使用界面**: Backstage → 新建故事 / 仪表盘  
**核心功能**: NovelCreationAgent + NovelCreationWizard + 首个场景自动生成

#### 2.1 启动小说创建向导

在仪表盘点击**「新建故事」**，触发 `NovelCreationWizard`：

**Step 1: 类型选择**
- AI 生成 3 个类型卡片：「硬科幻太空歌剧」、「赛博朋克近未来」、「后启示录星际流浪」
- 用户单击选择「硬科幻太空歌剧」，双击可编辑提示词

**Step 2: 世界观构建**
- 调用 `WorldBuilding Agent` (Memory Layer 4)
- AI 基于类型生成宏观世界观概念：星际联邦政治体制、曲速引擎技术设定、 alien 种族社会结构
- 用户通过卡片式选择确认核心设定，AI 将其存入 `world_buildings` 表

**Step 3: 角色谱生成**
- 调用 `Character Agent`，基于世界观生成主角配置
- 输出：沈牧歌 (星际牧羊人/基因改造者)、K-7 (觉醒的 AI 牧羊犬)、联邦稽查官 (反派)
- 角色信息存入 `characters` 表，并与故事建立关联

**Step 4: 文风设定**
- 调用 `WritingStyle Agent`
- 生成 3 种文风选项：「刘慈欣式硬核哲思」、「海因莱因式社会寓言」、「莱姆式诗意冷峻」
- 选择后，文风参数（句式长度、修辞偏好、节奏特征）被编码为 `writing_style` JSON，绑定到工作室配置

#### 2.2 自动生成首个场景

向导完成后，系统调用 `SceneGenerator`：
- AI 基于已确认的世界观、角色、文风，生成第一个场景的正文草案
- 同时生成该场景的**戏剧结构**：
  - 戏剧目标: 展示主角沈牧歌在边缘星区的孤独日常，并暗示基因改造的秘密
  - 外部压迫: 联邦稽查船正在逼近
  - 冲突类型: `ManVsSociety` (个体与体制的对抗)
  - 角色冲突: 沈牧歌 vs K-7 (关于是否逃离的道德分歧)

**当前缺失**: 创建向导生成的内容目前未自动触发 `IngestPipeline` 进入记忆系统。建议向导完成后自动执行一次全量 Ingest，确保新世界知识即时进入知识图谱。

---

### Phase 3: 幕前沉浸式写作 —— 文学诞生的主舞台

**使用界面**: Frontstage (幕前)  
**核心功能**: TipTap 编辑器 + AI 流式续写 + 角色卡片弹窗 + 禅模式

#### 3.1 进入幕前界面

点击左侧「幕前」按钮，切换到 Frontstage。

**界面特征**:
- 背景为 `oklch(96.5% 0.008 95)` 暖纸张色
- 正文字体为「霞鹜文楷 LXGW WenKai」，字号 18px，行距 1.8
- 顶部动态状态栏显示：当前字数 1,247 / 字体大小 18px / 保存状态「已保存」
- 底部 LLM 对话栏默认隐藏，鼠标悬停底部区域时优雅浮现

#### 3.2 人机协作式流式创作

**场景**: 用户正在写沈牧歌发现稽查船的场景。

1. **触发 AI 续写**: 按下 `Ctrl+Space`，底部状态灯由绿转黄（呼吸动画），表示正在请求模型
2. **流式输出**: `StreamingText` 组件接收 `stream_generate` 的 SSE 流
   - AI 生成文字以淡灰色 (`#87867f`)、14px 斜体，在用户光标位置逐字浮现
   - 右侧有 Terracotta 色的闪烁光标，伴随微弱光晕脉冲
3. **采纳生成**: 按 `Tab` 键，AI 文字「转正」—— 字号变大、颜色变深，融入正文
4. **拒绝生成**: 按 `Esc` 键，AI 文字淡出消失
5. **对话追问**: 对生成的内容不满意，在底部对话栏输入：「让 K-7 的语气更悲观一些」，按 Enter 发送。Shift+Enter 换行。AI 重新生成该段落。

#### 3.3 角色卡片实时查阅

写作中提到角色「K-7」时，使用快捷键（或悬停）调出**角色卡片弹窗**：
- 显示角色头像（首字母 + 紫绒色背景）
- 显示性格特征、动机、当前情绪状态
- 点击「关系图谱」可查看 K-7 与沈牧歌的关系强度 (0.85)

#### 3.4 禅模式专注写作

按 `F11` 进入禅模式：
- 界面全屏，隐藏所有 UI 元素
- 仅保留光标和纸张
- 再次按 `F11` 退出

**当前缺失**: 
- 角色卡片弹窗目前为设计概念，前端组件尚未完全接入后端角色数据。建议实现 `CharacterCardOverlay` 与 `characters` 表的实时联动。
- `StreamingText` 的墨滴扩散动画效果目前为 CSS 占位，尚未实现粒子系统。建议引入轻量级 Canvas 粒子库增强视觉体验。

---

### Phase 4: 创作者-AI 协作控制层 —— 意图驱动的智能编排

**使用界面**: 幕前底部 LLM 对话栏 / 幕后全局 AI 助手栏  
**核心功能**: 意图解析引擎 + Agent 调度器 (`workflow::scheduler`) + 多智能体协作 + 执行反馈闭环

StoryForge 不是被动等待指令的工具，而是一个**能够理解创作者意图并主动调度智能体执行**的协作系统。人类作者通过输入栏用自然语言表达需求，AI 接收后解析意图、调度 Agent、执行操作、反馈结果——形成完整的干预闭环。

#### 4.1 统一意图输入栏

**幕前输入栏**: 底部 LLM 对话栏（已存在）
- 默认状态隐藏，鼠标悬停底部浮现
- 支持自然语言指令、斜杠命令 (`/ingest`、`/analyze`)、`@Agent` 提及
- 显示当前激活的 Agent 头像与状态指示灯

**幕后输入栏** (规划中): Backstage 顶部固定的全局 AI 助手栏
- 类似 VS Code Command Palette 的增强版
- 支持跨页面操作：「帮我把 Scene 3 移到 Scene 5 后面」
- 支持多轮对话上下文保持

#### 4.2 意图解析引擎 (Intent Parser)

当用户在输入栏发送一条指令，系统首先调用 `Intent Parser`（由 LLM 担任）：

```
用户输入: "把 Scene 2 改得更紧张一些，让雷诺的威胁感更强，但不要改变 K-7 的悲观语气"
```

`Intent Parser` 解析输出结构化意图：
```json
{
  "intent_type": "text_rewrite",
  "target": { "type": "scene", "id": "scene_2" },
  "constraints": [
    "增强紧张感",
    "强化雷诺的威胁感",
    "保持 K-7 的悲观语气不变"
  ],
  "required_agents": ["writer", "style_mimic"],
  "execution_mode": "serial",
  "feedback_type": "diff_preview"
}
```

支持的意图类型覆盖全创作生命周期：
- `text_generate` — 文本续写/扩展
- `text_rewrite` — 文本改写/润色
- `plot_suggest` — 情节建议/反转设计
- `character_check` — 角色一致性检查
- `world_consistency` — 世界设定一致性检查
- `style_shift` — 文风切换/模仿
- `memory_ingest` — 知识摄取
- `visual_generate` — 图像/概念图生成 (通过 MCP)
- `scene_reorder` — 场景结构调整
- `outline_expand` — 大纲扩展

#### 4.3 Agent 调度器编排执行

`workflow::scheduler` 根据解析出的意图，执行 Agent 编排：

**示例 1: 单 Agent 直接执行**
```
用户: "继续写下去"
意图: text_generate
调度: Writer Agent → 直接调用 stream_generate → 流式输出到幕前
```

**示例 2: 多 Agent 串行协作**
```
用户: "帮我想一个 Scene 3 的反转，要符合沈牧歌的性格"
意图: plot_suggest
调度:
  Step 1: Memory Agent 执行 Query Pipeline，提取「沈牧歌性格」相关记忆
  Step 2: Plot Analyzer 分析现有情节张力与伏笔回收可能性
  Step 3: Outline Planner 生成 3 个反转方案卡片
  Step 4: Character Agent 审核各方案是否符合角色性格
  Step 5: 将筛选后的方案以卡片形式呈现给用户
```

**示例 3: 多 Agent 并行执行 + 聚合**
```
用户: "全面检查一下当前小说的问题"
意图: comprehensive_review
并行调度:
  - Inspector Agent → 检查语法、逻辑、设定冲突
  - Character Agent → 检查角色成长弧线断裂
  - Plot Analyzer → 检查情节节奏与伏笔回收
  - Style Agent → 检查文风一致性
聚合: 将所有结果汇总到「世界健康度报告」弹窗
```

**示例 4: 带 MCP 扩展的复合执行**
```
用户: "为 Scene 2 生成一张红砂风暴的概念图，然后帮我把画面描述写进场景里"
意图: visual_enhance
调度:
  Step 1: 通过 MCP Client 调用图像生成 Server，prompt 来自 Scene 2 正文
  Step 2: 图像返回后，Writer Agent 基于图像内容生成环境描写段落
  Step 3: 将生成段落以 Diff 预览形式展示，用户确认后插入 Scene 2
```

**上下文注入机制**: 每个 Agent 执行前，`workflow::scheduler` 自动调用 `memory::query`：
- 提取与当前任务最相关的记忆片段（通过 Hybrid Search）
- 将结果注入 Agent 的 System Prompt，确保 Agent "记得" 故事设定
- 如果 Agent 映射已配置，根据任务类型路由到不同的 LLM 模型（如创意任务走 Gemma，逻辑检查走 Qwen）

#### 4.4 执行反馈与人工干预

Agent 执行完成后，系统根据意图的 `feedback_type` 以不同形式反馈：

**直接修改 (direct_apply)**
- 适用于流式续写、小段改写
- AI 文字直接以淡灰色流式出现在光标位置
- 用户通过 `Tab` (采纳) / `Esc` (拒绝) 进行即时干预

**建议卡片 (suggestion_card)**
- 适用于情节建议、角色设定变更
- 在界面右侧浮现卡片，展示 AI 生成的 3 个选项
- 用户单击选择，双击编辑，右键忽略

**Diff 预览 (diff_preview)**
- 适用于对已有文本的改写、结构调整
- 弹出 DiffViewer，高亮增删改内容
- 用户点击「应用变更」或「放弃」

**系统通知 (system_notice)**
- 适用于异步任务完成通知（如 `/ingest` 执行完毕）
- 右下角 Toast 通知：「Scene 1-3 记忆摄取完成，新增 12 个实体」

**可视化高亮 (visual_highlight)**
- 适用于一致性检查、角色分析
- 在正文中高亮问题段落，悬浮显示 AI 批注
- 点击批注可直接接受修复建议

**撤回与重做**
- 所有通过 AI 协作控制层产生的修改，都自动记录到 `ai_operation_log`
- 用户可随时按 `Ctrl+Z` 撤回上一次 AI 操作，或从「操作历史」中选择性回滚

**当前缺失**: 
- **意图解析引擎尚未实现**：目前所有输入都直接走单一 `chat` 路由，没有结构化的意图分类与 Agent 调度。建议增加 `intent.rs` 模块，基于 function calling 实现意图解析。
- **Agent 编排 DSL 缺失**：`workflow::scheduler` 目前框架已存在但缺少可视化/声明式的工作流定义能力。建议支持 JSON/YAML 格式的工作流脚本，允许用户自定义 Agent 协作链。
- **幕后全局 AI 助手栏缺失**：Backstage 缺少统一的命令入口，所有操作依赖用户在各页面间跳转。建议在 Backstage 顶部增加常驻的 AI Command Bar。
- **Diff 预览层未与 AI 生成打通**：目前 `DiffViewer` 仅用于版本对比，未用于审阅 AI 改写结果。建议在 Writer Agent 改写流程中强制插入 Diff 预览步骤，避免 AI 直接覆盖用户文本。
- **操作历史与回滚缺失**：AI 生成的修改目前未纳入 undo/redo 栈。建议建立 `ai_operation_log` 表，记录每次 AI 操作的原始状态、生成结果、应用状态，支持按操作粒度回滚。

---

### Phase 5: 场景化叙事构建 —— 戏剧冲突驱动

**使用界面**: Backstage → 场景 / 故事时间线  
**核心功能**: Scene System + StoryTimeline + ConflictType 枚举 + 场景拖拽排序

#### 4.1 场景结构设计

回到 Backstage，进入**「场景」**页。用户已写好的第一个场景作为 Scene 1 存在。现在需要规划后续场景。

点击「添加场景」，输入场景元数据：
```
标题: 红砂风暴
序列号: 2
戏剧目标: 展现主角为保护羊群不惜违抗联邦法规
外部压迫: 红砂风暴即将来临，联邦稽查船已登陆
冲突类型: ManVsNature + ManVsSociety
在场角色: [沈牧歌, K-7, 稽查官雷诺]
角色冲突: 沈牧歌 vs 雷诺 (关于基因改造合法性的意识形态冲突)
```

#### 4.2 可视化时间线调整

在 `StoryTimeline` 组件中：
- 垂直时间线展示所有场景卡片
- 用户拖拽 Scene 3 到 Scene 2 之前
- 系统自动重新计算所有场景的 `sequence_number`，并更新 `previous_scene_id` 和 `next_scene_id` 链

#### 4.3 场景版本控制

对 Scene 2 的内容进行了大改后，点击「保存版本」。
- 系统调用 `SceneVersionRepository`，将当前内容快照保存
- `VersionTimeline` 显示垂直版本历史
- 用户可以对比任意两个版本的差异 (`DiffViewer`)
- 发现改差了，点击「恢复到此版本」
- `ConfidenceIndicator` 用圆形进度条显示当前版本与上一版本的相似度 (78%)

**当前缺失**: 
- 场景版本系统目前保存的是完整文本快照，未实现增量 Diff 存储，长期可能导致数据库膨胀。建议引入基于 `diff-match-patch` 的增量版本存储。
- 场景编辑器与幕前编辑器的内容同步是单向的（从幕前到场景），尚未实现双向实时同步。建议建立 `FrontstageEvent::ContentUpdate` ↔ `BackstageEvent::ContentChanged` 的 WebSocket 级实时桥接。

---

### Phase 6: 增强记忆系统 —— 让 AI 真正读懂你的故事

**使用界面**: Backstage → 知识图谱 / 技能 → 记忆助手  
**核心功能**: Ingest Pipeline + Knowledge Graph + Vector Store + Multi-Agent Sessions

#### 9.1 自动知识摄取 (Ingest)

用户完成 Scene 1 和 Scene 2 的写作后，点击技能栏的「记忆摄取」技能。

`IngestPipeline` 执行两步思维链：

**Step 1: 分析阶段**
- LLM 读取 Scene 1-2 的正文
- 提取实体: `沈牧歌` (人物)、`K-7` (AI)、`边缘星区` (地点)、`基因改造` (概念)、`联邦稽查船` (组织)
- 提取关系: `沈牧歌 --基因改造--> 边缘星区` (strength: 0.92)
- 提取事件: `稽查船逼近` (伏笔)、`红砂风暴预警` (环境)
- 提取情感基调: `孤独中带着倔强` (scene 1)、`紧张对峙` (scene 2)

**Step 2: 生成阶段**
- 将提取结果结构化存入 `kg_entities` 和 `kg_relations`
- 同时通过 `CJK Bigram Tokenizer` 对场景正文分词，生成向量嵌入 (Embedding)，存入 LanceDB

#### 9.2 知识图谱浏览

进入「知识图谱」页（规划中）：
- 可视化网络图展示实体节点
- 节点大小代表实体出现频次
- 连线粗细代表关系强度
- 点击「基因改造」节点，右侧滑出详情面板：
  - 定义、首次出现场景、相关角色、相关概念
  - 记忆保留系统提示：「该概念在 3 个场景中出现，优先级：高」

#### 7.3 记忆查询与辅助

用户在幕前写 Scene 3 时提到「沈牧歌的基因锁」：

`Query Pipeline` 被自动触发：
1. **分词搜索**: CJK Bigram 对「基因锁」分词
2. **向量相似度搜索**: 在 LanceDB 中查找语义相近的片段（如「基因改造」「基因序列」）
3. **图谱扩展**: 从 `kg_entities` 找到「基因改造」节点，扩展其 1-hop 邻居（沈牧歌、边缘星区、联邦法规）
4. **预算控制**: 根据上下文窗口限制，筛选最相关的 5 个记忆片段
5. **上下文组装**: 将筛选结果注入到当前请求的 System Prompt 中

AI 因此在续写 Scene 3 时，**自动记住了** Scene 1 埋下的基因改造伏笔，以及联邦法规的设定，生成连贯的剧情推进。

#### 6.4 多智能体会话 (Multi-Agent Sessions)

用户陷入创作瓶颈，激活「情节助手」`Plot Agent`：
- 该 Agent 拥有独立的 Wiki/Memory Session
- 它只读取与「情节推进」相关的记忆（忽略文风描写）
- 提出建议：「基于现有伏笔，Scene 3 可以设计为：红砂风暴中，稽查官雷诺发现沈牧歌的羊群其实是人类基因库的一部分……」

同理可召唤：
- `WorldBuilding Agent` — 解答设定漏洞
- `Character Agent` — 分析角色动机一致性
- `Style Agent` — 检查文风偏离度
- `Memory Agent` — 检索遗忘的设定细节

**当前缺失**: 
- LanceDB 向量存储目前为内存模式，应用重启后向量数据丢失。建议实现 LanceDB 的磁盘持久化存储，并在启动时自动重建索引。
- 知识图谱可视化页面目前尚未实现前端组件。建议基于 `d3-force` 或 `vis-network` 实现交互式图谱浏览器。
- Ingest Pipeline 目前为手动触发，建议配置为「每保存一个场景后自动异步触发」或支持批量夜间摄取。

---

### Phase 7: 角色成长与世界演化

**使用界面**: Backstage → 角色 / 场景  
**核心功能**: Character Evolution + Evolution Analyzer/Reviewer/Updater + 混合搜索

#### 9.1 角色成长追踪

在「角色」页查看 `沈牧歌` 的卡片：
- 基础信息栏：年龄、职业、外貌
- **成长弧线栏**：系统自动生成的角色弧线图
  - Scene 1: 孤独 (情绪值: -0.6)
  - Scene 2: 紧张但坚定 (情绪值: +0.2)
  - Scene 5: 觉醒反抗 (情绪值: +0.8)
- `CharacterEvolution` 技能分析各场景中角色的台词与行为，标记成长转折点

#### 9.2 世界设定一致性检查

写完 10 个场景后，运行 `Evolution Analyzer`：
- 扫描全文中所有关于「曲速引擎」的描述
- 发现 Scene 3 说「曲速引擎需要 3 天预热」，但 Scene 8 说「瞬间启动」
- `Evolution Reviewer` 标记这是一个**设定冲突**
- 用户点击冲突项，直接跳转到 Scene 3 和 Scene 8 的对比视图

#### 7.3 混合搜索定位问题

使用 Backstage 顶部的全局搜索框，输入「基因改造合法吗？」
- `Hybrid Search Engine` 同时执行：
  - **BM25 文本搜索**: 找到提到「基因改造」的 8 个场景
  - **向量语义搜索**: 找到讨论「法律/合法性/违规」的 3 个场景
  - **RRF 融合排序**: 将结果融合，最相关的场景排在前面
- 搜索结果展示片段摘要 + 跳转链接

**当前缺失**: 
- Character Evolution 的成长弧线目前依赖手动标记，建议引入 `evolution::analyzer` 的自动情感分析流水线，基于角色台词自动计算情绪值。
- 设定冲突检测目前未与前端 UI 打通，建议在「仪表盘」增加「世界健康度」看板，集中展示所有设定冲突与未回收伏笔。

---

### Phase 8: 版本管理与协作

**使用界面**: Backstage → 场景版本 / 协作 (规划中)  
**核心功能**: SceneVersionService + RetentionManager + Collab (OT/WebSocket)

#### 9.1 记忆保留与清理

小说写到 50 个场景，知识图谱已积累 300+ 实体。

`RetentionManager` 每周运行一次：
- 根据**遗忘曲线**计算每个记忆片段的保留概率
- 五级优先级分类：
  - P0 (核心设定): 永恒保留
  - P1 (主要角色): 衰减极慢
  - P2 (次要地点): 6 个月无引用则降级
  - P3 (一次性道具): 3 个月无引用则归档
  - P4 (废弃设定): 建议删除
- 生成「记忆保留报告」，提示用户：「边缘星区的辐射值设定已 45 天未引用，建议回顾或删除」

#### 9.2 多人协作编辑 (基础层已就绪)

如果未来开启协作功能：
- `collab::ot` 模块提供操作转换算法
- `collab::websocket` 提供实时通信通道
- 两位作者可以同时编辑同一个场景，系统保证最终一致性
- 评论批注系统（v3.2 计划）支持在段落上添加批注与回复

**当前缺失**: 
- Collab 模块的基础算法层已实现，但尚未接入前端编辑器。建议优先实现 TipTap 的 Yjs 适配或自研 OT 客户端适配器。
- 评论批注系统完全缺失，这是协作功能的前置依赖。建议增加 `annotations` 数据表和前端批注 UI。

---

### Phase 9: 导出与发布 —— 作品成型

**使用界面**: Backstage → 故事 → 导出  
**核心功能**: Export System (PDF/EPUB/Markdown) + Studio Manager (ZIP导入导出)

#### 9.1 多格式导出

小说完稿后，进入「故事」页选择《星际牧歌》：
1. **PDF 导出**: 
   - 使用内置模板，Cinzel 字体作为章节标题，Crimson Pro 作为正文
   - 包含自动生成的目录、页眉（小说名）、页脚（页码）
2. **EPUB 导出**: 
   - 生成标准 EPUB 3.0，支持电子书阅读器
   - 每一场景作为一章，保留元数据（场景标题、时间戳）
3. **Markdown 导出**: 
   - 纯文本格式，适合发布到网络小说平台或 GitHub

#### 9.2 工作室打包

用户想将《星际牧歌》的全部创作资料（正文、角色设定、知识图谱、版本历史、LLM 配置、文风参数）打包分享给合著者。

使用 `StudioManager`：
- 点击「导出工作室」
- 生成 `.storyforge` 文件（本质为 ZIP）
- 文件中包含：
  - `story.db` — SQLite 数据库
  - `vectors/` — LanceDB 向量文件
  - `config.json` — 工作室配置
  - `skills/` — 自定义技能脚本

接收方使用「导入工作室」，选择性导入需要的模块。

**当前缺失**: 
- PDF/EPUB 导出目前依赖基础模板，尚未支持自定义导出模板（v3.3 计划）。建议引入 Handlebars 或 Tera 模板引擎，允许用户自定义导出样式。
- 导出系统未集成图像生成结果（如 MCP 生成的概念图）。建议将 MCP 产出的多媒体文件纳入导出包。

---

## 三、缺失环节分析与优化建议

基于上述全流程梳理，以下环节存在缺失或待完善，按优先级排序：

### 🔴 P0 — 阻塞核心体验

| 缺失项 | 影响 | 优化方案 |
|--------|------|---------|
| **LanceDB 持久化** | 应用重启后向量数据丢失，AI "失忆" | 在 `vector/lancedb_store.rs` 中配置磁盘持久化路径，启动时自动加载已有索引 |
| **Ingest 自动触发** | 新写内容不进入记忆，AI 辅助脱节 | 在场景保存/幕前自动保存后异步调用 `IngestPipeline`，并提供后台进度指示 |
| **幕前 ↔ 幕后双向同步** | 两边内容不一致，用户体验割裂 | 实现基于 Tauri Event 的实时内容同步桥，`FrontstageEvent` ↔ `BackstageEvent` |
| **意图解析引擎** | 创作者输入无法被结构化理解，AI 协作沦为简单聊天 | 增加 `intent.rs` 模块，基于 LLM function calling 实现意图分类与参数提取 |

### 🟠 P1 — 严重影响创作效率

| 缺失项 | 影响 | 优化方案 |
|--------|------|---------|
| **知识图谱可视化** | 用户无法直观浏览故事世界 | 基于 `d3-force` 实现交互式 KG 浏览器，支持缩放、筛选、实体详情弹窗 |
| **角色卡片弹窗** | 写作时查阅角色信息不便 | 实现 `CharacterCardOverlay` 组件，接入 `characters` 表，支持 hover/快捷键触发 |
| **Agent 映射生效** | 设置页 Agent 配置是摆设 | 完善 `config/commands.rs` 中的路由逻辑，使不同 Agent 调用不同模型配置 |
| **世界健康度看板** | 设定冲突、伏笔遗漏难以发现 | 在仪表盘增加 `WorldHealthDashboard`，整合 `Evolution Analyzer` 和未回收伏笔检测 |
| **Diff 预览层与 AI 生成打通** | AI 改写直接覆盖原文，用户无法审阅风险 | 在 Writer Agent 改写流程中强制插入 Diff 预览步骤，支持「应用」/「放弃」/「编辑」 |
| **幕后全局 AI 助手栏** | Backstage 缺少统一命令入口，操作效率低下 | 在 Backstage 顶部增加常驻 AI Command Bar，支持自然语言跨页面操作 |

### 🟡 P2 — 体验增强

| 缺失项 | 影响 | 优化方案 |
|--------|------|---------|
| **增量版本存储** | 数据库随版本数线性膨胀 | 引入 `diff-match-patch` 库，仅存储版本间差异 |
| **自定义导出模板** | 导出格式千篇一律 | 引入 Tera 模板引擎，允许用户上传自定义 PDF/EPUB 模板 |
| **流式生成粒子动效** | AI 生成缺少 "文思泉涌" 的视觉感染力 | 增加轻量 Canvas 粒子效果，模拟墨滴扩散 |
| **自动更新 nightly 发布页** | 用户无法直接下载最新构建 | CI 已配置，待验证发布产物是否完整包含图标更新 |
| **Agent 编排 DSL** | 高级用户无法自定义 Agent 协作链 | 支持 JSON/YAML 格式的工作流脚本，允许声明式定义多 Agent 串并行流程 |
| **AI 操作历史与回滚** | AI 生成的修改无法按操作粒度撤回 | 建立 `ai_operation_log` 表，记录原始状态、生成结果、应用状态，支持选择性回滚 |

---

## 四、功能覆盖度检查表

为确保本文档使用了 StoryForge 的**所有已实现功能**，以下逐一核对：

### 核心架构
- [x] 幕前 (Frontstage) 沉浸式写作界面
- [x] 幕后 (Backstage) 专业工作室
- [x] Tauri Bridge IPC 通信
- [x] 双窗口切换 (`FrontstageLauncher`)

### 创作者-AI 协作控制层
- [x] 统一意图输入栏（幕前底部 LLM 对话栏）
- [ ] 意图解析引擎（结构化意图分类与参数提取）
- [x] Agent 调度器框架 (`workflow::scheduler`)
- [ ] Agent 编排 DSL（声明式多 Agent 协作链）
- [x] 流式续写反馈（`StreamingText` / `Tab` 采纳 / `Esc` 拒绝）
- [ ] Diff 预览层审阅 AI 改写结果
- [ ] 幕后全局 AI 助手栏（跨页面自然语言命令）
- [ ] AI 操作历史与回滚 (`ai_operation_log`)

### 场景化叙事
- [x] Scene 模型（戏剧目标、外部压迫、冲突类型、角色冲突）
- [x] StoryTimeline 可视化时间线
- [x] SceneEditor 三标签页编辑
- [x] 场景拖拽排序与依赖维护
- [x] 6 种 ConflictType 枚举应用

### 增强记忆系统
- [x] Ingest Pipeline（两步思维链）
- [x] Knowledge Graph（带权实体关系）
- [x] Vector Store（LanceDB + CJK Bigram）
- [x] Query Pipeline（四阶段检索）
- [x] Multi-Agent Sessions（世界观/人物/文风/情节/场景/记忆助手）

### AI 生成与辅助
- [x] NovelCreationAgent
- [x] NovelCreationWizard 卡片式选择 UI
- [x] 首个场景自动生成
- [x] stream_generate 流式生成
- [x] `StreamingText` 双状态编辑器
- [x] AI 提示接受/拒绝/重新生成交互
- [x] Writer Agent（文本改写/续写）
- [x] Plot Analyzer / Outline Planner（情节分析与规划）
- [x] Style Mimic Agent（文风模仿）
- [x] Inspector Agent（审查与一致性检查）
- [ ] 多 Agent 并行协作执行（Comprehensive Review）

### 角色与世界
- [x] Character 数据模型与管理
- [x] WorldBuilding 表与 Agent
- [x] 角色关系与冲突追踪
- [x] CharacterEvolution 技能

### 版本与演化
- [x] SceneVersionRepository / Service
- [x] VersionTimeline / DiffViewer
- [x] ConfidenceIndicator
- [x] Evolution Analyzer / Reviewer / Updater
- [x] RetentionManager（遗忘曲线 + 五级优先级）

### 搜索与路由
- [x] BM25 Search（CJK 二元组）
- [x] Hybrid Search（RRF 融合）
- [x] Entity Hybrid Search
- [x] LLM Router（cost/model/router）

### 技能与 MCP
- [x] Skills 系统（Loader/Executor/Registry/Builtin）
- [x] MCP Client / Server / Transport
- [x] MCP 服务连接管理 UI

### 配置与导出
- [x] StudioConfig（每部小说独立配置）
- [x] LLM 模型配置（chat/embedding/multimodal/image）
- [x] 本地三模型集成（Gemma / Qwen3.5 / bge-m3）
- [x] 模型连接状态检测
- [x] StudioManager ZIP 导入导出
- [x] PDF / EPUB / Markdown 导出
- [x] `.storyforge` 打包格式

### 界面与设计
- [x] OKLCH 颜色系统
- [x] LXGW WenKai 字体
- [x] Cinema 暗色主题（幕后）
- [x] Parchment 暖色主题（幕前）
- [x] 电影感设计系统（glass / gold lines / film grain）
- [x] TipTap 富文本编辑器
- [x] 禅模式 (Zen Mode)

---

## 五、结语：从工具到伙伴

StoryForge v3.1.2 已经具备了**完整的小说智能化创作基础设施**。从世界观构建到流式写作，从场景化叙事到增强记忆，从版本控制到多格式导出，每一个环节都有相应的功能模块支撑。

当前项目距离「完美」只差最后几步：
1. **让意图解析真正落地**（结构化理解创作者指令，调度正确的 Agent 执行）
2. **让记忆真正持久化**（LanceDB 磁盘存储）
3. **让图谱真正可视化**（前端 KG 浏览器）
4. **让双界面真正融为一体**（实时同步桥接）
5. **让 Agent 映射真正生效**（模型路由落地）

当这些 P0/P1 问题解决后，StoryForge 将不再是单纯的「写作工具」，而会成为一位**真正读过你所有作品、记得每一个伏笔、懂得你文风偏好的 AI 创作伙伴**。

> 🌿 **草苔虽小，越写越懂。**
