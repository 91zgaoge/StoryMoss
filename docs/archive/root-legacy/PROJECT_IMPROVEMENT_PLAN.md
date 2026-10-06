# StoryMoss (草苔) 智能化创作系统优化计划书

> **版本**: v3.3.0 → v4.0 智能化创作核心升级
> **日期**: 2026-04-18
> **核心目标**: 从"功能堆砌"走向"系统智能"，真正实现"越写越懂"的创作伙伴
> **铁律**: 幕前幕后两分开 —— 幕前只创作不管理，幕后全功能不干扰

---

## 一、诊断篇：智能化创作目标 vs 当前现实

### 1.1 项目愿景回顾

StoryMoss 的核心承诺是**"越写越懂"的智能化创作系统**：
- **智能化**: 系统通过持续阅读用户创作内容，构建故事记忆，在每次创作时提供精准的上下文支持
- **创作**: 围绕小说创作全流程（构思→大纲→场景→写作→迭代），提供Agent辅助
- **越写越懂**: 每写一章，系统对故事的理解就深一层，下一次辅助就更精准

### 1.2 当前系统六大结构性缺陷

#### 缺陷一：Agent 各自为战，无协作编排（"散兵游勇"）

| 现状 | 理想 |
|------|------|
| 8 个 Agent 类型定义清晰，但 `AgentService` 只是简单 `switch-case` 分发 | Agent 按创作阶段编排成工作流，自动协作 |
| Writer 写作后不会自动触发 Inspector 质检 | Writer → Inspector → 反馈循环，质检结果驱动改写 |
| `OutlinePlannerAgent` 和 `AgentService::OutlinePlanner` 是两套独立实现 | 统一 Agent 运行时，一套实现多处复用 |
| `IntentExecutor` 串行执行只是"把 A 的输出传给 B"，无语义转换 | Agent 间有协议化接口，支持结果聚合、分歧解决 |
| `character_agent`/`world_building_agent` 映射到 `Inspector` fallback | 每个创作维度有专职 Agent，不混用 |

**关键代码证据**:
```rust
// intent.rs:308 - 上下文构建完全硬编码，不从数据库读取真实故事状态
fn build_context(story_id: &str, _intent: &Intent) -> AgentContext {
    AgentContext {
        story_id: story_id.to_string(),
        story_title: "未命名作品".to_string(),  // ❌ 不查数据库
        genre: "小说".to_string(),               // ❌ 不查故事设定
        tone: "中性".to_string(),                // ❌ 不查风格配置
        characters: vec![],                      // ❌ 不查角色列表
        previous_chapters: vec![],               // ❌ 不查章节摘要
        // ...
    }
}
```

#### 缺陷二：记忆系统与创作流程断裂（"有库不用"）

| 模块 | 已实现 | 未使用 |
|------|--------|--------|
| `IngestPipeline` | 两步思维链分析内容，提取实体/关系/事件/情感/伏笔 | 无自动触发机制，用户需手动调用 |
| `QueryPipeline` | 四阶段检索（分词→图谱扩展→预算控制→上下文组装） | **没有任何 Agent 在写作时调用它** |
| `MultiAgentSessionManager` | 5 种独立会话（世界观/人物/文风/场景/情节） | 会话结果不自动同步到创作上下文 |
| `KnowledgeGraph` | 实体-关系图可视化 | 写作时不自动查询相关实体注入提示词 |
| `VectorStore` | 语义搜索接口 | LanceDB 被注释掉，当前内存存储（重启丢失） |

**创作飞轮断裂**: 写作 → ❌不自动Ingest → ❌知识不更新 → ❌下次写作无记忆 → 每次都从零开始

#### 缺陷三：写作风格停留在"排版皮肤"（"表里不一"）

| 层面 | 现状 | 应有 |
|------|------|------|
| 视觉 | `writingStyles.ts` 定义 5 种字体/颜色/间距组合 | ✅ 已实现 |
| 创作 | `StyleMimic` Agent 只是"模仿参考文风样例" | ❌ 需要风格规范系统 |
| 深度 | 没有风格 DNA 解析：句式结构、修辞偏好、情感密度 | 风格 = 词汇库 + 句法模板 + 节奏模式 + 视角规范 |

当前 `"古典深沉"` 风格只改了字体为 `"Noto Serif SC"`，但 Writer 生成内容时**没有任何陀思妥耶夫斯基式的创作约束**（长句、哲学对话、心理剖析）。

#### 缺陷四：场景/角色/世界观设定与创作脱节（"设定是设定，写作是写作"）

**NovelCreationWizard** 流程：
```
用户输入题材 → 生成世界观选项 → 选择世界观 → 生成角色谱 → 选择角色 → 生成文风 → 生成首场景 → ✅ 完成
```

问题：完成后这些设定去了哪里？
- 世界观规则（如"灵力体系：炼气→筑基→金丹"）→ **不注入 Writer system prompt**
- 角色关系网（如"A 是 B 的杀父仇人但不知情"）→ **不用于人物一致性检查**
- 场景结构（dramatic_goal/external_pressure/conflict_type）→ **续写时不参考**
- 写作风格（tone/pacing/vocabulary_level）→ **仅作为提示词中的一行文本**

`CharacterInfo` 只有 `name`/`personality`/`role`，**没有角色弧光、当前状态、目标动机、秘密**。

#### 缺陷五：Agent 提示词即兴拼凑，无创作方法论（"野路子"）

| 应有 | 现状 |
|------|------|
| 雪花写作法（10 步从一句话到完整小说） | ❌ 未实现 |
| 英雄之旅 12 阶段 | ❌ 未实现 |
| 三幕式结构 + 节拍表 | `OutlinePlanner` 提到"起承转合"但无具体方法论 |
| 场景结构（目标-冲突-灾难-反应-困境-决定） | ❌ 未实现 |
| 人物深度模型（目标/动机/冲突/秘密/弧光） | ❌ 未实现 |
| 风格 DNA 编码（海明威=短句+省略+冰山理论） | ❌ 未实现 |

`PromptManager` 只有两个占位模板，**整个系统的提示词都是临时格式化的字符串**，散落在各 Agent 文件中。

#### 缺陷六："越写越懂"是伪命题（"无记忆学习闭环"）

真正的"越写越懂"需要：
1. ✅ 内容存储（SQLite 存储章节）
2. ❌ 自动分析（Ingest 不自动触发）
3. ❌ 知识更新（分析结果不反馈到创作上下文）
4. ❌ 自适应生成（不跟踪用户接受/拒绝模式）
5. ❌ 精准检索（QueryPipeline 不被调用）

当前系统：**每次 AI 辅助都是冷启动**，对用户故事的理解深度在第 1 章和第 50 章没有区别。

---

## 二、蓝图篇：StoryMoss 智能化创作系统架构

### 2.1 核心设计理念

```
┌─────────────────────────────────────────────────────────────┐
│                   StoryMoss 智能化创作飞轮                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌──────────┐    自动分析    ┌──────────┐    知识注入    ┌──────────┐
│   │  用户写作  │ ────────────→ │  记忆系统  │ ────────────→ │  Agent   │
│   │ (幕前编辑器)│              │(Ingest+KG) │              │  辅助创作  │
│   └──────────┘                └──────────┘                └──────────┘
│        ↑                                                      │
│        │                   越写越懂闭环                          │
│        └────────────────── 接受/反馈 ───────────────────────────┘
│                                                             │
│   关键机制:                                                 │
│   • 每保存一章 → 自动 Ingest → 更新知识图谱                  │
│   • Writer 写作前 → 自动 Query → 注入相关设定/角色/伏笔        │
│   • 用户接受/拒绝 → 记录偏好 → 调整生成策略                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 幕前幕后职责边界（铁律）

> **所有新增功能必须首先回答：这是创作行为还是管理行为？**

| 维度 | 幕前 (Frontstage) | 幕后 (Backstage) |
|------|-------------------|------------------|
| **核心使命** | 沉浸式写作 | 创作资源管理与工作流编排 |
| **设计主题** | 暖色纸张质感 (#f5f4ed) | 深色电影感 (cinema-950) |
| **界面原则** | 极简、无干扰、接近阅读体验 | 功能完整、信息密集、专业管理 |
| **Agent 交互** | 只呈现结果（续写文本、轻量提示） | 完整 Agent 工作流配置与执行监控 |
| **场景结构** | 只读显示当前场景的戏剧目标（侧栏） | 编辑 dramatic_goal / conflict_type |
| **角色信息** | 点击角色名弹出轻量卡片（只读） | 完整角色编辑、弧光设计、关系管理 |
| **风格系统** | 切换预设风格（影响排版+AI生成） | 风格DNA编辑、风格解析、作家库管理 |
| **知识图谱** | 不显示 | 完整可视化、实体编辑、关系管理 |
| **伏笔追踪** | 写作时轻量提醒（如"此处有未回收伏笔"） | 完整伏笔看板、setup/payoff 管理 |
| **创作方法论** | 不暴露方法论选择，系统自动应用 | 方法论选择、阶段配置、工作流编排 |
| **记忆系统** | 无感知（自动查询自动注入） | Ingest 状态监控、记忆健康、归档管理 |

**同步机制**：
- 幕后设计的场景结构 → 通过事件同步到幕前侧栏（只读）
- 幕后设定的风格DNA → 同步到幕前影响AI生成（幕前只切换风格预设）
- 幕后管理的角色状态 → 同步到幕前用于角色卡片弹窗
- 幕前写作的内容 → 保存后自动触发 Ingest，幕后更新知识图谱

### 2.3 五大子系统重构蓝图

#### 子系统 A：创作方法论引擎（Methodology Engine）— 幕后

将经典创作方法论编码为系统级提示词规范，在幕后配置，幕前无感知自动应用：

```
prompts/methodologies/
├── snowflake/
│   ├── step_01_one_sentence_story.md   # 一句话概括
│   ├── step_02_one_paragraph_expansion.md
│   ├── ...
│   └── step_10_scene_drafting.md
├── hero_journey/
│   ├── act_01_departure.md             # 平凡世界→冒险召唤→拒绝→导师
│   ├── act_02_initiation.md
│   └── act_03_return.md
├── save_the_cat/
│   ├── beat_01_opening_image.md
│   ├── beat_02_theme_stated.md
│   └── ...
└── scene_structure/
    ├── goal_conflict_disaster.md       # 场景上半：目标-冲突-灾难
    ├── reaction_dilemma_decision.md    # 续接：反应-困境-决定
    └── sequel_variations.md
```

**Agent 提示词模板化**：
```rust
pub struct AgentPromptTemplate {
    pub methodology_id: Option<String>,     // 绑定的创作方法论
    pub system_prompt: PromptBlock,         // 系统角色定义
    pub context_injection: Vec<ContextSource>, // 自动注入的上下文源
    pub output_schema: JsonSchema,          // 结构化输出约束
    pub quality_checklist: Vec<String>,     // 自检清单
}
```

**幕后 UI**: 用户在幕后选择"本次创作使用雪花写作法"，幕前 Writer 自动按雪花法生成，幕前不显示任何方法论UI。

#### 子系统 B：Agent 协作编排系统（Agent Orchestrator）— 幕后编排，幕前呈现

定义创作阶段工作流，在幕后配置和监控，幕前只呈现最终结果：

```rust
pub enum CreationPhase {
    Conception,      // 构思阶段：用户灵感 → OutlinePlanner → 故事种子
    Outlining,       // 大纲阶段：故事种子 → 三幕式/雪花法 → 完整大纲
    SceneDesign,     // 场景设计：大纲章节 → SceneDesigner → 场景结构
    Writing,         // 写作阶段：场景结构 + 记忆查询 → Writer → 初稿
    Review,          // 审校阶段：初稿 → Inspector + ConsistencyChecker → 问题列表
    Iteration,       // 迭代阶段：问题列表 → Writer(改写) → 终稿
    Ingestion,       // 记忆阶段：终稿 → IngestPipeline → 知识图谱更新
}

pub struct PhaseWorkflow {
    pub phase: CreationPhase,
    pub required_agents: Vec<AgentSlot>,    // 岗位定义
    pub context_sources: Vec<ContextSource>, // 该阶段自动查询的上下文
    pub transitions: Vec<PhaseTransition>,   // 阶段转移条件
}
```

**幕后 UI**: 工作流编排面板、Agent 执行监控、阶段切换控制
**幕前 UI**: 只显示 AI 续写结果、轻量接受/拒绝按钮、极简进度指示

**关键工作流示例——写作阶段**：
```
1. 用户在幕前触发续写（或自动续写）
2. 【系统自动上下文构建】（幕前无感知）
   a. 查询知识图谱：当前场景涉及的角色、地点、物品实体
   b. 查询向量记忆：与当前主题相关的历史场景
   c. 查询角色状态：各角色在本章之前的最新状态/动机
   d. 查询伏笔追踪：未回收的伏笔列表
3. 【Agent 协作】（幕后执行）
   a. Writer 生成初稿（注入全部上下文）
   b. Inspector 质检（检查人物一致性、逻辑漏洞）
   c. 如有问题 → 返回 Writer 改写；如无问题 → 输出
4. 【幕前用户交互】
   a. 流式显示 AI 生成内容（幕前）
   b. 用户 Tab 接受 / Esc 拒绝 / 手动修改
   c. 记录用户反馈到偏好模型（幕后）
5. 【记忆更新】（保存后幕后自动触发）
   a. IngestPipeline 分析新内容
   b. 更新实体状态、关系演变、新伏笔
   c. 更新向量存储
```

#### 子系统 C：深度风格系统（Style DNA）— 幕后编辑，幕前切换

从"排版皮肤"升级为"创作基因"：

```rust
pub struct StyleDNA {
    pub meta: StyleMeta,                    // 名称、作者、描述
    pub vocabulary: VocabularyProfile,      // 词汇偏好（抽象/具象、古风/现代、密度）
    pub syntax: SyntaxProfile,              // 句法特征（平均句长、从句复杂度、节奏模式）
    pub rhetoric: RhetoricProfile,          // 修辞偏好（比喻频率、通感、排比、反讽）
    pub perspective: PerspectiveProfile,    // 视角规范（POV 距离、内心独白比例、全知程度）
    pub emotion: EmotionProfile,            // 情感表达（外露/克制、情感词汇密度）
    pub dialogue: DialogueProfile,          // 对话风格（简洁/冗长、潜台词比例、方言特征）
}
```

**幕后 UI**: `StyleDnaEditor.tsx` — 风格解析器（上传5000字样例→生成DNA）、作家库管理、风格验证
**幕前 UI**: 风格切换按钮（同现有的5种风格切换），但切换的不仅是字体颜色，还包括AI生成策略

风格注入写作流程：
- 用户在幕后创建 `"张爱玲风格 DNA"` → 保存到作家库
- 用户在幕前选择 `"张爱玲风格"`（同现有交互）
- 幕后自动加载 StyleDNA → 注入 Writer system prompt
- 生成后幕后 StyleChecker 验证 → 不达标则幕后自动改写
- 幕前用户始终只看到最终生成结果

#### 子系统 D：角色-世界观-场景 联动引擎（Story Continuity Engine）— 幕后管理，幕前受益

```rust
pub struct ContinuityEngine {
    pub character_tracker: CharacterTracker,    // 角色状态追踪
    pub world_consistency: WorldConsistencyChecker, // 世界观一致性检查
    pub foreshadowing_tracker: ForeshadowingTracker, // 伏笔追踪
    pub timeline_manager: TimelineManager,      // 时间线管理
}

pub struct CharacterState {
    pub profile: CharacterProfile,          // 基础设定
    pub current_state: CharacterMoment,     // 当前状态（位置、情绪、目标）
    pub arc_progress: f32,                  // 弧光进度（0-1）
    pub relationships: Vec<RelationshipState>, // 动态关系状态
    pub secrets_known: Vec<String>,         // 已知的秘密
    pub secrets_unknown: Vec<String>,       // 尚不知情的秘密
    pub appearance_count: HashMap<String, usize>, // 各场景出场次数
}
```

**幕后 UI**: 
- `CharacterArcView.tsx` — 角色弧光可视化、关系网编辑、秘密管理
- `ForeshadowingBoard.tsx` — 伏笔看板、setup/payoff 追踪
- `SceneDesigner.tsx` — 场景结构设计（dramatic_goal / external_pressure / conflict_type）
- `WorldBuildingEditor.tsx` — 世界观规则编辑、一致性检查

**幕前 UI**:
- 侧栏显示当前场景的戏剧目标（只读）
- 点击角色名弹出轻量卡片（显示当前状态，只读）
- 写作时如有伏笔可在此处回收，轻量提示"此处可回收伏笔'神秘项链'"
- **绝不暴露复杂的管理界面**

**联动示例**：
- 幕后设定：张三目前不知道李四是杀父仇人（秘密 #7）
- 幕后设定：张三此刻应该在监狱（根据第3章时间线）
- 用户在幕前写"张三走进了李四的办公室"
- 幕后 ContinuityEngine 检测一致性冲突 → 通过 AI 提示在幕前显示："提示：根据时间线，张三目前应在监狱。是否调整？"
- 幕前用户只看到一个轻量提示气泡，不改写作界面

#### 子系统 E：自适应记忆系统（Adaptive Memory）— 幕后透明运行

实现真正的"越写越懂"，幕前完全无感知：

```
记忆层级（幕后管理，幕前无感知）：
Layer 1: 原始文本（章节全文）— 数据库存储
Layer 2: 场景摘要（每场景 100 字摘要 + 关键词）— 自动提取
Layer 3: 实体知识（角色/地点/物品/组织的当前状态）— Ingest 生成
Layer 4: 情节记忆（事件链、伏笔状态、未解悬念）— 自动追踪
Layer 5: 风格记忆（用户修改模式、偏好句型、高频词汇）— 偏好挖掘
```

**幕后 UI**: 记忆健康面板、Ingest 状态监控、记忆压缩管理、用户偏好统计
**幕前 UI**: 完全无感知 —— 系统自动查询、自动注入、自动调整

**自适应机制**（幕后运行）：
- 用户频繁删除 AI 生成的环境描写 → 记录偏好 `"减少环境描写"` → 下次生成降低环境描写权重
- 用户经常手动添加对话 → 记录偏好 `"增加对话比例"` → 调整 Writer prompt
- 某角色名字经常被 AI 写错 → 记录 `"强化角色名记忆"` → 在 prompt 中显式列出

---

## 三、实施篇：分阶段路线图

### 第一阶段：地基重构（4-6 周）— 让系统"知道"自己在写什么

**目标**：解决 Agent 上下文空洞问题，实现"写作时系统自动查询故事设定"

| 周次 | 任务 | 关键产出 | 验收标准 | 界面归属 |
|------|------|---------|---------|---------|
| W1 | **AgentContext 真实化** | `StoryContextBuilder` 模块 | Writer 写作时 prompt 包含真实角色列表、世界观规则 | 幕后 |
| W1-2 | **知识查询自动化** | `QueryPipeline` 接入 `WriterAgent` | 每次续写前自动查询 KG + Vector，注入前3相关实体 | 幕后运行，幕前无感知 |
| W2 | **Ingest 自动触发** | 保存章节后自动 Ingest | 新保存的章节 5 秒内出现在知识图谱中 | 幕后 |
| W2-3 | **角色状态追踪** | `CharacterTracker` 初版 | 能检测"角色此刻不应该在这里"类一致性错误 | 幕后 |
| W3 | **伏笔追踪系统** | `ForeshadowingTracker` | 能列出"未回收伏笔"并在写作时提示 | 幕后管理 + 幕前轻量提示 |
| W3-4 | **Prompt 模板化** | `PromptTemplateEngine` + 5 个核心模板 | Agent 提示词从硬编码字符串改为模板渲染 | 幕后 |

**技术要点**：
- `StoryContextBuilder` 从数据库读取：故事设定、角色列表、章节摘要、最新场景
- `QueryPipeline` 接入：Writer 写作前自动 `query(current_paragraph, story_id)`
- Ingest 触发器：`save_chapter` command 成功后 `tokio::spawn` 后台执行
- 伏笔追踪：Ingest 结果中的 `foreshadowing` 存入 `foreshadowing_tracker` 表

### 第二阶段：方法论注入（4-5 周）— 让 Agent "懂"创作

**目标**：将经典创作方法论编码为系统能力

| 周次 | 任务 | 关键产出 | 验收标准 | 界面归属 |
|------|------|---------|---------|---------|
| W4 | **雪花写作法** | `SnowflakeMethodology` | 支持从 1 句话 → 1 段 → 角色表 → 场景表 → 全文 | 幕后向导 |
| W4-5 | **场景结构规范** | `SceneStructureMethodology` | 每个场景必须包含：目标-冲突-灾难-反应-困境-决定 | 幕后场景设计器 |
| W5 | **英雄之旅模板** | `HeroJourneyMethodology` | 大纲自动标注 12 阶段位置 | 幕后大纲视图 |
| W5-6 | **人物深度模型** | `CharacterDepthModel` | 角色卡包含：目标/动机/冲突/秘密/弧光/转变时刻 | 幕后角色管理 |
| W6-7 | **Agent 协作协议** | `AgentOrchestrator` v1 | 支持 Writer→Inspector→Writer 闭环 | 幕后工作流面板 |

**雪花写作法集成流程（幕后向导）**：
```
用户在幕后启动雪花向导：
Step 1: 输入一句话故事 → 系统保存"故事种子"
Step 2: 扩展为一段（5句：设定+3灾难+结局）
Step 3: 角色概要（每个角色：名字+目标+动机+冲突+顿悟）
Step 4: 扩展每句话为一段（形成 5 段故事摘要）
Step 5: 角色详细表（完整角色小传）→ 保存到角色管理
Step 6: 扩展 5 段为完整故事梗概
Step 7: 制作场景表（每个场景：POV+目标+冲突+挫折）→ 保存到场景管理
Step 8: 扩展场景为章节大纲
Step 9-10: 逐场景写作（切换到幕前）
```

### 第三阶段：风格深度化（3-4 周）— 从"看起来像"到"写得像"

**目标**：建立 Style DNA 系统，让 AI 真正模仿风格而非只是换字体

| 周次 | 任务 | 关键产出 | 验收标准 | 界面归属 |
|------|------|---------|---------|---------|
| W7 | **Style DNA 模型** | `StyleDNA` 结构定义 | 可描述任意风格的量化特征 | 幕后 |
| W7-8 | **风格解析器** | `StyleAnalyzer` | 输入 5000 字样例，输出 StyleDNA JSON | 幕后 |
| W8 | **风格注入** | Writer 支持 StyleDNA prompt | 同一段情节，切换风格后输出明显不同 | 幕后配置，幕前切换 |
| W8-9 | **风格验证** | `StyleChecker` | 生成内容经 StyleChecker 评分，不达标自动改写 | 幕后 |
| W9 | **经典风格库** | 内置 10 位作家风格 DNA | 用户可在幕后一键选择"海明威/张爱玲/金庸/村上春树..." | 幕后作家库 |

**金庸 StyleDNA 示例**：
```json
{
  "vocabulary": {
    "density": "high",
    "preference": ["武侠术语", "古典诗词", "色彩词汇", "动词前置"]
  },
  "syntax": {
    "avg_sentence_length": 35,
    "clause_complexity": "high",
    "rhythm_pattern": "四字格+长短交替"
  },
  "rhetoric": {
    "metaphor_density": 0.08,
    "preference": ["武打比喻", "自然意象", "对仗"]
  },
  "dialogue": {
    "style": "古典白话",
    "subtext_ratio": 0.3,
    "signature": "说话前先动作描写"
  }
}
```

### 第四阶段：自适应学习（3-4 周）— 实现"越写越懂"

**目标**：建立用户偏好模型，让系统随着使用变得更懂用户

| 周次 | 任务 | 关键产出 | 验收标准 | 界面归属 |
|------|------|---------|---------|---------|
| W10 | **反馈记录** | `UserFeedbackLog` | 记录每次用户接受/拒绝/修改 AI 建议 | 幕后 |
| W10-11 | **偏好挖掘** | `PreferenceMiner` | 从反馈日志提取稳定偏好（如"偏好对话>描写"） | 幕后 |
| W11 | **生成策略调整** | `AdaptiveGenerator` | 根据偏好动态调整 temperature/top-p/prompt 权重 | 幕后 |
| W11-12 | **记忆压缩优化** | `SmartMemoryCompressor` | 自动判断哪些记忆需要保留/丢弃/升级 | 幕后 |
| W12 | **个性化提示词** | `PersonalizedPrompt` | 每个用户有独立的系统提示词调优 | 幕后 |

### 第五阶段：工作流闭环（4-5 周）— 一键完成创作全流程

**目标**：实现从构思到成稿的完整自动化工作流

| 周次 | 任务 | 关键产出 | 验收标准 | 界面归属 |
|------|------|---------|---------|---------|
| W13 | **Workflow 引擎** | `CreationWorkflowEngine` | 支持在幕后可视化编排创作阶段 | 幕后 |
| W13-14 | **一键创作** | `OneClickNovel` | 幕后输入一句话 → 自动完成雪花 10 步 → 输出完整小说 | 幕后向导 |
| W14-15 | **人机协作模式** | `CollaborativeMode` | 支持"AI 草稿+人修改"和"人草稿+AI 润色"两种模式 | 幕后配置，幕前执行 |
| W15 | **质量评估** | `NovelQualityReport` | 生成后在幕后自动评估：结构完整性、人物一致性、风格统一度 | 幕后报告 |

---

## 四、技术实现要点

### 4.1 核心架构调整（严格区分幕前幕后）

```
src-tauri/src/
├── creative_engine/          # 创作引擎核心（幕后运行）
│   ├── mod.rs
│   ├── context_builder.rs    # StoryContextBuilder
│   ├── continuity.rs         # ContinuityEngine
│   ├── methodology/          # 创作方法论
│   │   ├── mod.rs
│   │   ├── snowflake.rs
│   │   ├── hero_journey.rs
│   │   ├── scene_structure.rs
│   │   └── character_depth.rs
│   └── style/                # 深度风格系统
│       ├── mod.rs
│       ├── dna.rs
│       ├── analyzer.rs
│       └── checker.rs
├── agent_orchestrator/       # Agent 编排（幕后运行）
│   ├── mod.rs
│   ├── workflow.rs           # 阶段工作流定义
│   ├── transitions.rs        # 阶段转移逻辑
│   └── feedback_loop.rs      # 反馈闭环
├── memory/                   # 记忆系统（幕后透明运行）
│   ├── mod.rs
│   ├── ingest.rs             # 现有，需加自动触发
│   ├── query.rs              # 现有，需加创作接入
│   ├── adaptive.rs           # 新增：自适应学习
│   └── foreshadowing.rs      # 新增：伏笔追踪
├── prompts/                  # 提示词系统（幕后管理）
│   ├── mod.rs
│   ├── engine.rs             # 模板渲染引擎
│   ├── templates/            # 模板库
│   │   ├── writer/
│   │   ├── inspector/
│   │   └── planner/
│   └── methodologies/        # 方法论提示词
│       ├── snowflake/
│       ├── hero_journey/
│       └── scene_structure/
└── agents/                   # Agent 层（幕后运行）
    ├── mod.rs
    ├── runtime.rs            # 统一 Agent 运行时
    ├── context.rs            # AgentContext（真实化）
    ├── roles/                # 各 Agent 实现
    │   ├── writer.rs
    │   ├── inspector.rs
    │   ├── planner.rs
    │   └── ...
    └── collaboration.rs      # Agent 协作协议

src-frontend/src/
├── frontstage/               # 幕前：只创作，不管理
│   ├── components/
│   │   ├── RichTextEditor.tsx     # 核心：编辑器（现有，增强AI上下文）
│   │   ├── AiSuggestionBubble.tsx # AI 续写气泡（现有）
│   │   ├── AiHintOverlay.tsx      # 文思泉涌提示（现有）
│   │   ├── EditorContextMenu.tsx  # 右键菜单（现有）
│   │   ├── CharacterCardPopup.tsx # 角色卡片（只读轻量弹窗）
│   │   ├── FrontstageToolbar.tsx  # 底部工具栏（现有）
│   │   └── SceneOutline.tsx       # 场景导航（侧栏，只读显示戏剧目标）
│   ├── hooks/
│   │   └── useCreativeContext.ts  # 创作上下文（自动接收幕后同步）
│   └── store/
│       └── frontstageStore.ts     # 幕前状态（极简）
├── pages/                    # 幕后：全功能管理
│   ├── Dashboard.tsx
│   ├── Stories.tsx
│   ├── Scenes.tsx              # 场景管理 + 场景结构设计器
│   ├── Characters.tsx          # 角色管理 + 角色弧光视图
│   ├── KnowledgeGraph.tsx      # 知识图谱 + 实体编辑
│   ├── Settings.tsx            # 设置中心 + 风格DNA编辑器
│   ├── Skills.tsx
│   └── Mcp.tsx
├── creative/                 # 新增：幕后创作中心
│   ├── NovelCreationFlow.tsx   # 雪花法/英雄之旅向导（幕后）
│   ├── SceneDesigner.tsx       # 场景结构设计器（幕后）
│   ├── StyleDnaEditor.tsx      # 风格 DNA 编辑器（幕后）
│   ├── CharacterArcView.tsx    # 角色弧光可视化（幕后）
│   ├── ForeshadowingBoard.tsx  # 伏笔看板（幕后）
│   └── WorkflowPanel.tsx       # 创作工作流编排面板（幕后）
└── stores/
    └── creativeStore.ts        # 创作状态管理（幕后）
```

### 4.2 数据库 Schema 扩展

```sql
-- 角色状态追踪
CREATE TABLE character_states (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    current_location TEXT,
    current_emotion TEXT,
    active_goal TEXT,
    secrets_known TEXT,      -- JSON array
    secrets_unknown TEXT,    -- JSON array
    arc_progress REAL,       -- 0.0 - 1.0
    last_updated TEXT,
    FOREIGN KEY (character_id) REFERENCES characters(id)
);

-- 伏笔追踪
CREATE TABLE foreshadowing_tracker (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    content TEXT NOT NULL,       -- 伏笔内容
    setup_scene_id TEXT,         -- 设置场景
    payoff_scene_id TEXT,        -- 回收场景（可为空）
    status TEXT NOT NULL,        -- setup / payoff / abandoned
    importance INTEGER,          -- 1-10
    created_at TEXT,
    resolved_at TEXT
);

-- 用户偏好
CREATE TABLE user_preferences (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    preference_type TEXT,        -- style / content / structure
    preference_key TEXT,
    preference_value TEXT,
    confidence REAL,             -- 0.0 - 1.0
    evidence_count INTEGER,      -- 支持该偏好的证据数
    updated_at TEXT
);

-- 风格 DNA
CREATE TABLE style_dnas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    author TEXT,
    dna_json TEXT NOT NULL,      -- StyleDNA JSON
    is_builtin BOOLEAN,
    is_user_created BOOLEAN,
    created_at TEXT
);
```

### 4.3 幕前幕后同步协议

```rust
/// 幕后 → 幕前：场景结构同步
pub struct SceneStructureSync {
    pub scene_id: String,
    pub dramatic_goal: String,       // 幕前侧栏只读显示
    pub external_pressure: String,
    pub conflict_type: String,
    pub characters_present: Vec<String>, // 影响角色卡片弹窗
}

/// 幕后 → 幕前：风格切换同步
pub struct StyleSync {
    pub style_id: String,
    pub font_family: String,         // 幕前排版变量
    pub font_size: i32,
    pub dna_summary: String,         // 注入 Writer prompt 的风格摘要
}

/// 幕后 → 幕前：一致性提示
pub struct ConsistencyHint {
    pub hint_type: String,           // "character_location" / "foreshadowing" / "timeline"
    pub message: String,             // 轻量提示文本（12px Olive Gray）
    pub severity: String,            // "info" / "warning"
}

/// 幕前 → 幕后：内容保存触发 Ingest
pub struct ContentSavedEvent {
    pub chapter_id: String,
    pub content: String,
    pub story_id: String,
}

/// 幕前 → 幕后：用户反馈
pub struct UserFeedbackEvent {
    pub chapter_id: String,
    pub feedback_type: String,       // "accept" / "reject" / "modify"
    pub original_ai_text: String,
    pub final_text: String,
}
```

---

## 五、验收标准与 KPI

### 5.1 阶段性验收

| 阶段 | 核心验收测试 |
|------|-------------|
| 第一阶段 | 在幕后设定世界观和角色，切换到幕前写作，Writer 续写时自动包含世界观规则和角色列表 |
| 第二阶段 | 在幕后使用雪花法向导从一句话生成完整大纲，大纲中每章自动标注三幕式位置 |
| 第三阶段 | 在幕后上传金庸5000字样例生成 StyleDNA，在幕前选择该风格写同一段情节，输出明显不同 |
| 第四阶段 | 连续在幕前写作 10 章，第 10 章的 AI 建议相比第 1 章更符合用户偏好（通过盲测验证） |
| 第五阶段 | 在幕后输入"一个关于时间旅行者的爱情故事"，一键生成完整小说初稿，幕前可读 |

### 5.2 量化 KPI

| 指标 | 当前 | 第一阶段 | 第二阶段 | 第三阶段 | 第四阶段 |
|------|------|---------|---------|---------|---------|
| Agent 上下文完整度 | 10%（硬编码） | 80%（真实数据） | 90% | 95% | 98% |
| 记忆自动触发率 | 0% | 100% | 100% | 100% | 100% |
| 人物一致性错误检出 | 0 | 基础检测 | 增强检测 | 自动预防 | 零错误 |
| 风格可量化区分度 | N/A | N/A | N/A | 可区分 5+ 风格 | 可区分 10+ 风格 |
| 用户偏好学习准确率 | N/A | N/A | N/A | 60% | 80% |
| 一键创作完整度 | N/A | N/A | N/A | N/A | 可生成可读初稿 |

### 5.3 幕前幕后边界验收

| 检查项 | 验收标准 |
|--------|---------|
| 幕前无管理型 UI | 幕前界面不出现任何"编辑"、"配置"、"管理"类复杂面板 |
| 幕前侧栏信息密度 | 侧栏只显示：场景列表、当前场景戏剧目标（1行）、字数统计 |
| 幕前 AI 提示形式 | 只接受：流式文本、轻量气泡提示（12px Olive Gray）、Tab/Esc 交互 |
| 幕后功能完整性 | 所有创作方法论、Agent 配置、风格 DNA 编辑、伏笔管理均在幕后可用 |
| 同步实时性 | 幕后修改场景结构 → 3 秒内同步到幕前侧栏 |

---

## 六、风险与建议

### 6.1 风险

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| LLM 提示词过长 | 上下文窗口溢出 | 智能裁剪 + 分层记忆（只注入最相关的） |
| 自动生成质量不稳定 | 用户信任度下降 | 每步都有 Inspector 质检 + 用户确认机制 |
| 创作方法论过于西化 | 中文创作水土不服 | 同时引入中国传统评点法、章回体结构 |
| 幕前功能膨胀 | 违背两分开原则 | 每个新功能必须经过"创作vs管理"审查 |
| 数据库存储膨胀 | 性能下降 | 记忆压缩 + 归档策略 |

### 6.2 实施建议

1. **MVP 优先**：第一阶段先做通"Writer 自动查 KG"这一个闭环，其他逐步扩展
2. **A/B 测试**：新功能默认关闭，用户可切换新旧模式对比
3. **保留人工干预**：每个自动化步骤都有"暂停/修改/跳过"选项
4. **使用 Spec-Kit**：每个子系统按 `.specify/` 流程执行，确保需求不漂移
5. **持续评估**：每两周做一次创作测试（让系统写同一段情节，评估进步）
6. **两分开审查**：每增加一个前端组件，必须明确回答"这是幕前还是幕后？是否符合极简/管理边界？"

---

## 七、修正声明

本计划书第一版在前端架构部分错误地将 `SceneDesigner`、`StyleDnaEditor`、`CharacterArcView`、`ForeshadowingBoard` 等管理型功能置于幕前，严重违反了"幕前幕后两分开"铁律。现已全部调整至幕后，幕前仅保留沉浸式写作和极简AI辅助呈现。

**幕前铁律 checklist**：
- [x] 不出现任何"编辑"、"配置"、"管理"类面板
- [x] 不暴露创作方法论选择UI（系统自动应用）
- [x] 不暴露风格DNA编辑（只提供风格切换）
- [x] 不暴露角色关系网编辑（只提供角色卡片只读弹窗）
- [x] 不暴露伏笔管理看板（只提供轻量写作时提示）
- [x] 所有复杂管理功能均置于幕后深色电影感界面

---

*本计划书以"智能化创作"为北极星指标，以"幕前幕后两分开"为架构铁律，所有工程决策服务于"让系统真正理解用户的故事，并在幕前以极简方式提供精准辅助"这一目标。*
