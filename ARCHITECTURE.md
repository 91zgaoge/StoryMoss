# StoryForge (草苔) v6.0.0 架构文档

> 本文档反映 v6.0.0 最新架构状态

## 架构理念

StoryForge 采用创新的**剧院式双界面架构 + 场景化叙事 + 增强记忆系统**：

- **幕前 (Frontstage)**: 沉浸式写作界面，如同登台演出
- **幕后 (Backstage)**: 专业工作室，如同后台准备
- **场景 (Scene)**: 戏剧冲突驱动的叙事单位，取代传统章节
- **记忆 (Memory)**: 基于 llm_wiki 的知识图谱，真正的"越写越懂"

---

## 系统架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        StoryForge (草苔) v3.0                             │
│                     Tauri + React + Rust + SQLite                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────┐        ┌─────────────────────────┐         │
│  │     🎭 幕前 Frontstage   │        │     🎬 幕后 Backstage    │         │
│  │    (沉浸式写作界面)      │        │    (专业工作室)          │         │
│  ├─────────────────────────┤        ├─────────────────────────┤         │
│  │                         │        │                         │         │
│  │  • 极简阅读写作界面      │◄──────►│  • 故事/场景/角色管理     │         │
│  │  • TipTap 富文本编辑器   │        │  • LLM 模型配置中心       │         │
│  │  • 场景大纲侧边栏        │        │  • 技能系统               │         │
│  │  • AI 续写辅助          │        │  • 知识图谱浏览          │         │
│  │  • 写作风格切换          │        │  • 工作室配置管理        │         │
│  │  • 禅模式全屏           │        │  • 数据导出/分析          │         │
│  │  • 角色卡片弹窗          │        │                         │         │
│  │                         │        │                         │         │
│  │  暖色调 (#f5f4ed)        │        │  深色主题 (Cinema)       │         │
│  │  Claude 阅读体验设计     │        │  电影感专业界面          │         │
│  │                         │        │                         │         │
│  └──────────┬──────────────┘        └──────────┬──────────────┘         │
│             │                                   │                        │
│             └───────────────┬───────────────────┘                        │
│                             ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Tauri Bridge (IPC)                            │   │
│  │           Commands + Events + Window Management                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                             │                                          │
│  ┌──────────────────────────┴──────────────────────────────────────┐   │
│  │                      Backend (Rust) - v3.0 Core                   │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  🎪 SCENE SYSTEM (场景化叙事)                                     │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  • Scene: 戏剧目标、外部压迫、冲突类型、角色冲突         │   │   │
│  │  │  • StoryTimeline: 可视化场景序列、拖拽排序              │   │   │
│  │  │  • SceneGenerator: AI 场景生成建议                      │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  │  🧠 MEMORY SYSTEM (增强记忆系统)                                  │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  Layer 4: Multi-Agent Sessions (世界观/人物/文风助手)    │   │   │
│  │  │  Layer 3: Knowledge Graph (带权实体关系图谱)             │   │   │
│  │  │  Layer 2: Vector Store (CJK分词语义检索)                 │   │   │
│  │  │  Layer 1: Raw Sources (场景正文、角色设定)               │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │   │
│  │  │   Agents    │  │   Skills    │  │      LLM Adapter        │ │   │
│  │  │  ├─ Writer  │  │  ├─ Loader  │  │  ├─ OpenAI             │ │   │
│  │  │  ├─ NovelCreation│ ├─ Executor│ │  ├─ Anthropic         │ │   │
│  │  │  ├─ Planner │  │  ├─ Registry│  │  ├─ Ollama (本地)      │ │   │
│  │  │  ├─ Style   │  │  └─ Builtin │  │  └─ Azure/DeepSeek...  │ │   │
│  │  │  └─ Plot    │  │             │  │                         │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘ │   │
│  │                                                                  │   │
│  │  📦 STUDIO SYSTEM (工作室配置)                                    │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │  • StudioConfig: 每部小说独立配置                        │   │   │
│  │  │  • Import/Export: ZIP格式导入导出                        │   │   │
│  │  │  • Theme System: 幕前暖色/幕后暗色默认主题              │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                             │                                          │
│  ┌──────────────────────────┴──────────────────────────────────────┐   │
│  │                      Data Layer                                   │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │   │
│  │  │   SQLite    │  │  LanceDB    │  │    File System          │ │   │
│  │  │  (r2d2池)   │  │  (向量检索)  │  │  • 技能库               │ │   │
│  │  │  • Stories  │  │  • 场景嵌入  │  │  • 导出文件             │ │   │
│  │  │  • Scenes   │  │  • 实体向量  │  │  • 工作室配置           │ │   │
│  │  │  • Characters│ │  • 语义搜索  │  │                         │ │   │
│  │  │  • KG Entities│ └─────────────┘  └─────────────────────────┘ │   │
│  │  │  • KG Relations                                                 │
│  │  │  • WorldBuilding                                                │
│  │  └─────────────┘                                                  │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 核心系统详解

### 🎪 场景化叙事系统 (Scene System)

#### 场景模型
```rust
pub struct Scene {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,      // 场景序号
    pub title: String,
    
    // 戏剧结构
    pub dramatic_goal: String,      // 戏剧目标
    pub external_pressure: String,  // 外部压迫
    pub conflict_type: ConflictType, // 冲突类型
    
    // 角色参与
    pub characters_present: Vec<String>,
    pub character_conflicts: Vec<CharacterConflict>,
    
    // 内容
    pub content: String,
    pub setting: Setting,
    
    // 关联
    pub previous_scene_id: Option<String>,
    pub next_scene_id: Option<String>,
}

pub enum ConflictType {
    ManVsMan,        // 人与人
    ManVsSelf,       // 人与自我
    ManVsSociety,    // 人与社会
    ManVsNature,     // 人与自然
    ManVsTechnology, // 人与科技
    ManVsFate,       // 人与命运
}
```

#### 场景 vs 章节

| 特性 | 章节 (Chapter) | 场景 (Scene) |
|------|----------------|--------------|
| 驱动方式 | 时间/长度驱动 | 戏剧冲突驱动 |
| 结构 | 线性序列 | 网络化关联 |
| AI 理解 | 文本内容 | 戏剧目标 + 冲突 |
|  reorder | 简单排序 | 依赖关系维护 |

---

### 🧠 增强记忆系统 (Memory System)

基于 [karpathy/llm_wiki](https://github.com/karpathy/llm_wiki) 方法论实现。

#### 四层架构

```
┌─────────────────────────────────────────┐
│  Layer 4: Multi-Agent Sessions          │
│  - WorldBuilding Agent (世界观助手)      │
│  - Character Agent (人物助手)            │
│  - WritingStyle Agent (文风助手)         │
│  - Plot Agent (情节助手)                 │
│  - Scene Agent (场景助手)                │
│  - Memory Agent (记忆助手)               │
├─────────────────────────────────────────┤
│  Layer 3: Knowledge Graph               │
│  - Entity (实体)                        │
│  - Relation (关系，带 strength 0-1)      │
│  - 关系强度动态计算                      │
├─────────────────────────────────────────┤
│  Layer 2: Vector Store                  │
│  - CJK Bigram Tokenizer                 │
│  - 语义检索                              │
│  - 相似度搜索                            │
├─────────────────────────────────────────┤
│  Layer 1: Raw Sources                   │
│  - 场景正文                              │
│  - 角色设定                              │
│  - 世界设定                              │
└─────────────────────────────────────────┘
```

#### 两步思维链 Ingest

```rust
impl IngestPipeline {
    pub async fn ingest(&self, content: &IngestContent) -> Result<(), Error> {
        // Step 1: 分析阶段
        let analysis = self.analyze_content(content).await?;
        // 提取：实体、关系、事件、情感、伏笔
        
        // Step 2: 生成阶段
        let knowledge = self.generate_knowledge(&analysis).await?;
        // 生成：实体档案、关系强度、事件重要性
        
        // 保存
        self.save_to_graph(&knowledge).await?;
        self.save_to_vector_store(&knowledge).await?;
        
        Ok(())
    }
}
```

#### 四阶段查询检索

```rust
impl QueryPipeline {
    pub async fn query(&self, query: &str) -> Result<QueryResult, Error> {
        // Stage 1: CJK二元组分词搜索
        let search_results = self.token_search(query).await?;
        
        // Stage 2: 图谱扩展（基于关系强度）
        let graph_expansion = self.graph_expansion(&search_results).await?;
        
        // Stage 3: 预算控制（4K-1M tokens可配）
        let selected = self.budget_control(
            &search_results, 
            &graph_expansion
        ).await?;
        
        // Stage 4: 带引用编号的上下文组装
        let context = self.assemble_context(&selected).await?;
        
        Ok(QueryResult { context, citations })
    }
}
```

---

### 🤖 AI 智能生成系统

#### NovelCreationAgent

```rust
pub struct NovelCreationAgent {
    llm_adapter: Arc<dyn LlmAdapter>,
}

impl NovelCreationAgent {
    /// 根据用户输入生成世界观选项（3个）
    async fn generate_world_building_options(
        &self,
        user_input: &str,
    ) -> Result<Vec<WorldBuilding>, Error>;
    
    /// 根据世界观生成角色谱选项
    async fn generate_character_profiles(
        &self,
        world_building: &WorldBuilding,
    ) -> Result<Vec<Vec<CharacterProfile>>, Error>;
    
    /// 生成文字风格选项
    async fn generate_writing_styles(
        &self,
        genre: &str,
        world_building: &WorldBuilding,
    ) -> Result<Vec<WritingStyle>, Error>;
    
    /// 生成首个场景
    async fn generate_first_scene(
        &self,
        story_context: &StoryContext,
    ) -> Result<Scene, Error>;
}
```

#### 引导式创建流程

```
用户输入类型 → AI生成世界观选项 → 用户选择/编辑 → 
AI生成角色谱选项 → 用户选择/编辑 → 
AI生成文字风格选项 → 用户选择/编辑 → 
AI生成首个场景 → 开始创作
```

---

### 📦 工作室配置系统

#### 配置架构

```
~/.config/storyforge/
├── config.json              # 全局配置
└── studios/
    └── {story_id}/
        ├── studio.json          # 工作室主配置
        ├── llm_config.json      # LLM配置
        ├── ui_config.json       # 界面配置
        ├── agent_bots.json      # Agent配置
        └── ...
```

#### 导入/导出

```rust
pub struct StudioManager;

impl StudioManager {
    /// 导出工作室配置到 .storyforge ZIP
    pub async fn export_studio(
        &self,
        story_id: &str,
        output_path: &Path,
    ) -> Result<()>;
    
    /// 从 .storyforge ZIP 导入工作室配置
    pub async fn import_studio(
        &self,
        import_path: &Path,
        options: ImportOptions,
    ) -> Result<ImportResult>;
}
```

---

### 📜 场景版本系统 (Phase 3.x)

**版本管理架构**

```
┌─────────────────────────────────────────┐
│         Scene Version System            │
├─────────────────────────────────────────┤
│                                         │
│  SceneVersionRepository                 │
│  ├─ create_version()     # 创建快照     │
│  ├─ get_versions()       # 获取历史     │
│  ├─ get_version()        # 获取特定版本 │
│  └─ delete_version()     # 删除版本     │
│                                         │
│  SceneVersionService                    │
│  ├─ compare_versions()   # 版本对比     │
│  ├─ restore_version()    # 恢复版本     │
│  ├─ get_version_chain()  # 版本链       │
│  └─ get_version_stats()  # 统计信息     │
│                                         │
│  VersionTimeline (React)                │
│  ├─ VersionCard          # 版本卡片     │
│  ├─ DiffViewer           # 差异查看     │
│  └─ ConfidenceIndicator  # 置信度指示   │
│                                         │
└─────────────────────────────────────────┘
```

**版本模型**
```rust
pub struct SceneVersion {
    pub id: String,
    pub scene_id: String,
    pub version_number: i32,        // 版本号 (v1, v2, ...)
    
    // 内容快照
    pub title: Option<String>,
    pub content: Option<String>,
    pub dramatic_goal: Option<String>,
    pub conflict_type: Option<ConflictType>,
    
    // 版本元数据
    pub word_count: i32,
    pub change_summary: String,
    pub created_by: CreatorType,    // user/ai/system
    pub confidence_score: Option<f32>,
    
    // 版本链
    pub previous_version_id: Option<String>,
    pub superseded_by: Option<String>,
}
```

### 🔍 混合搜索系统 (Phase 1.3)

**RRF 融合排序**
```
┌─────────────────────────────────────────┐
│          Hybrid Search                  │
├─────────────────────────────────────────┤
│                                         │
│  Query: "主角与反派的冲突"               │
│                                         │
│  ┌─────────────┐    ┌─────────────┐    │
│  │ BM25 Search │    │Vector Search│    │
│  │  (CJK分词)  │    │(余弦相似度) │    │
│  └──────┬──────┘    └──────┬──────┘    │
│         │                   │           │
│         ▼                   ▼           │
│  ┌─────────────────────────────────┐   │
│  │    RRF Fusion (k=60)            │   │
│  │    score = Σ(1/(k+r))           │   │
│  └─────────────────────────────────┘   │
│                    │                    │
│                    ▼                    │
│         ┌──────────────────┐            │
│         │  Hybrid Results  │            │
│         └──────────────────┘            │
│                                         │
└─────────────────────────────────────────┘
```

### 🧠 记忆保留系统 (Phase 1.4)

**艾宾浩斯遗忘曲线**
```
R(t) = R₀ × e^(-λt) + Σ(强化奖励)

其中:
- R₀: 初始置信度
- λ: 衰减率 (架构级 0.01, 默认 0.05, 瞬态 0.1)
- t: 距离上次访问的天数
- Σ(强化): 每次访问增加的奖励
```

**优先级分级**
```rust
pub enum PriorityLevel {
    Critical,    // > 0.8  - 必须保留
    High,        // 0.6-0.8 - 优先保留
    Medium,      // 0.4-0.6 - 正常保留
    Low,         // 0.2-0.4 - 可压缩
    Forgotten,   // < 0.2  - 可归档
}
```

---

### 🏛️ Story System 合同驱动体系 (v6.0.0)

**四级合同架构**
```
┌─────────────────────────────────────────┐
│           Story System                  │
├─────────────────────────────────────────┤
│                                         │
│  MASTER_SETTING (故事级全局设定)          │
│  └─ Volume (卷级设定)                    │
│     └─ Chapter (章节级设定与预期)         │
│        └─ Review (审阅与修订合同)         │
│                                         │
│  CHAPTER_COMMIT (写后真源)               │
│  ├─ state_deltas_json                   │
│  ├─ entity_deltas_json                  │
│  ├─ accepted_events_json                │
│  └─ projection_status_json              │
│                                         │
│  5 Projection Writers                   │
│  ├─ StateProjectionWriter               │
│  ├─ IndexProjectionWriter               │
│  ├─ SummaryProjectionWriter             │
│  ├─ MemoryProjectionWriter              │
│  └─ VectorProjectionWriter              │
│                                         │
│  ContractTree / RuntimeContract         │
│  └─ 动态合并上层合同 → 运行时约束         │
│                                         │
└─────────────────────────────────────────┘
```

**防幻觉三定律**
1. 合同即法律 — 所有生成内容受合同约束
2. 设定即物理 — 世界观设定如物理定律般不可违背
3. 发明需识别 — 新实体必须被明确识别并记录

---

### 🧠 三层记忆编排器 (v6.0.0)

**MemoryOrchestrator 架构**
```
┌─────────────────────────────────────────┐
│      MemoryOrchestrator                 │
├─────────────────────────────────────────┤
│                                         │
│  Working Memory (50% budget for write)  │
│  ├─ 最近 5 章正文摘要                    │
│  ├─ 活跃角色（出场 > 3 次）               │
│  └─ 开放伏笔（未回收）                    │
│                                         │
│  Episodic Memory (30% budget)           │
│  ├─ state_changes 时间线                 │
│  └─ relationships 演变                   │
│                                         │
│  Semantic Memory (20% budget)           │
│  ├─ 长期事实（按优先级过滤）              │
│  │   Critical > High > Medium > Low     │
│  └─ 源章节窗口过滤（最近 30 章）          │
│                                         │
│  MemoryPack 组装                        │
│  └─ 按任务类型分配预算权重               │
│      write: 50/30/20                    │
│      plan:  20/30/50                    │
│      review: 30/40/30                   │
│                                         │
└─────────────────────────────────────────┘
```

---

### 📈 追读力评估系统 (v6.0.0)

**ReadingPowerEvaluator 五维评估**
```rust
pub struct ReadingPowerEvaluation {
    pub overall_score: f32,        // 0-100
    pub hook_count: i32,           // 悬念/冲突/转折
    pub coolpoint_count: i32,      // 打脸/收获/揭秘
    pub micropayoff_count: i32,    // 小承诺兑现
    pub debt_count: i32,           // 未兑现承诺
    pub trend: Vec<f32>,           // 最近 N 章趋势
}
```

**DebtManager 债务追踪**
- 创建债务 → 逾期计息（每日 5%）→ 覆盖合同跳过 → 兑现销账

---

### 📚 体裁模板库 (v6.0.0)

**GenreProfile 外部化**
- 启动时优先读取 `{app_data_dir}/templates/genres.json`
- 内置 37 个网文体裁模板，支持自定义编辑
- 模板五要素：核心基调、节奏策略、反模式清单、参考数据表、典型结构

---

### 🔍 Anti-AI 五维审查 (v6.0.0)

**AntiAiReviewer 架构**
- 词汇维度：Cliché 检测 + 重复用词
- 语法维度：句式多样性 + 被动语态
- 叙事维度：段落均匀度 + 感官密度
- 情感维度：标签化检测 + 展示 vs 告知
- 对话维度：说明性对话 + 标签单调性
- 输出：overall_score + issues + suggestions + flagged_passages

---

### 📊 可观测性系统 (v6.0.0)

**Ingest 作业追踪**
- `ingest_jobs` 表：pending → running → completed/failed
- `ingest-job-updated` Tauri 事件推送状态变更
- 幕前顶栏 🧠 图标实时显示最近 Ingest 健康状态

**功能使用度量**
- `feature_usage_logs` 表：feature_id / action / story_id / metadata
- `telemetry/mod.rs`：本地 SQLite 记录，零网络传输
- Settings 页面「数据统计」标签：30 天柱状图

**Projection 健康检查**
- 解析 `chapter_commits.projection_status_json`
- 逐 Writer 展示成功/失败状态与错误原因

---

### 🔒 类型安全基座 (v6.0.0)

**ts-rs 自动生成**
- Rust `SyncEvent` / `FrontstageEvent` / `BackstageEvent` 添加 `#[derive(TS)]`
- 编译时生成 TypeScript 绑定到 `src-frontend/src/generated/`

**前端穷尽匹配**
```typescript
function assertUnreachable(x: never): never {
  throw new Error(`Unhandled case: ${x}`);
}
// default case 中使用，新增 variant 时编译失败
```

**IPC 一致性检查**
- `scripts/verify-ipc-manifest.py` 解析 `generate_handler![]` 与前端 `loggedInvoke`
- 前端调用未注册命令时报 ERROR

---

## 目录结构

```
v2-rust/
├── src-frontend/                 # 前端代码
│   ├── src/
│   │   ├── main.tsx             # 幕后入口
│   │   ├── App.tsx              # 幕后主应用
│   │   │
│   │   ├── frontstage/          # 幕前界面
│   │   │   ├── main.tsx         # 幕前入口
│   │   │   ├── FrontstageApp.tsx
│   │   │   ├── components/
│   │   │   │   ├── ReaderWriter.tsx
│   │   │   │   ├── RichTextEditor.tsx
│   │   │   │   ├── CharacterPeekCard.tsx     # 🆕 角色悬浮卡片 (v6.0.0)
│   │   │   │   ├── IngestHealthIndicator.tsx # 🆕 Ingest 健康指示器 (v6.0.0)
│   │   │   │   └── ...
│   │   │   └── styles/
│   │   │
│   │   ├── pages/               # 幕后页面
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Stories.tsx
│   │   │   ├── Characters.tsx
│   │   │   ├── Scenes.tsx           # 🆕 场景管理
│   │   │   └── Settings.tsx
│   │   │
│   │   ├── components/          # 共享组件
│   │   │   ├── StoryTimeline.tsx    # 🆕 故事线视图
│   │   │   ├── SceneEditor.tsx      # 🆕 场景编辑器
│   │   │   ├── NovelCreationWizard.tsx # 🆕 创建向导
│   │   │   └── ...
│   │   │
│   │   ├── hooks/               # 自定义 Hooks
│   │   │   ├── useScenes.ts         # 🆕 场景管理
│   │   │   ├── useWorldBuilding.ts  # 🆕 世界构建
│   │   │   └── useStudioConfig.ts   # 🆕 工作室配置
│   │   │
│   │   └── types/
│   │       └── v3.ts                # 🆕 V3类型定义
│   │
│   ├── index.html
│   ├── frontstage.html
│   └── package.json
│
├── src-tauri/                   # Tauri后端
│   ├── src/
│   │   ├── main.rs              # 入口
│   │   ├── lib.rs               # 库入口
│   │   ├── commands.rs          # 基础命令
│   │   ├── commands_v3.rs       # 🆕 V3命令集
│   │   │
│   │   ├── db/                  # 数据库层
│   │   │   ├── connection.rs
│   │   │   ├── models_v3.rs     # 🆕 V3数据模型
│   │   │   └── repositories_v3.rs # 🆕 V3存储层
│   │   │
│   │   ├── agents/              # Agent系统
│   │   │   ├── mod.rs
│   │   │   ├── writer.rs
│   │   │   └── novel_creation.rs # 🆕 小说创建Agent
│   │   │
│   │   ├── memory/              # 🆕 记忆系统
│   │   │   ├── mod.rs
│   │   │   ├── tokenizer.rs     # CJK分词器
│   │   │   ├── ingest.rs        # Ingest管线
│   │   │   ├── query.rs         # 查询检索管线
│   │   │   ├── multi_agent.rs   # 多助手会话
│   │   │   ├── hybrid_search.rs # 🆕 混合搜索 (Phase 1.3)
│   │   │   ├── retention.rs     # 🆕 记忆保留 (Phase 1.4)
│   │   │   └── orchestrator.rs  # 🆕 三层记忆编排器 (v6.0.0)
│   │   │
│   │   ├── story_system/        # 🆕 Story System 合同驱动 (v6.0.0)
│   │   │   ├── mod.rs
│   │   │   └── projection_writers.rs
│   │   │
│   │   ├── reading_power/       # 🆕 追读力评估系统 (v6.0.0)
│   │   │   └── mod.rs
│   │   │
│   │   ├── anti_ai/             # 🆕 Anti-AI 五维审查 (v6.0.0)
│   │   │   └── mod.rs
│   │   │
│   │   ├── telemetry/           # 🆕 功能使用度量 (v6.0.0)
│   │   │   └── mod.rs
│   │   │
│   │   ├── versions/            # 🆕 版本管理 (Phase 3.x)
│   │   │   ├── mod.rs
│   │   │   └── service.rs       # 版本服务
│   │   │
│   │   ├── config/              # 配置管理
│   │   │   └── studio_manager.rs # 🆕 工作室管理
│   │   │
│   │   └── ...
│   │
│   └── Cargo.toml
│
├── docs/                        # 文档
└── README.md
```

---

## 数据流

### 场景创建流程
```
用户点击"新建场景" → StoryTimeline 
→ invoke('create_scene') 
→ SceneRepository::create()
→ SQLite → 返回场景ID
→ StoryTimeline 更新列表
```

### AI 场景生成流程
```
用户请求生成 → SceneGeneratorAgent
→ QueryPipeline::query() 获取上下文
→ LLM Adapter 生成场景建议
→ 返回 3 个 SceneProposal
→ 用户选择 → SceneRepository::create()
```

### 记忆 Ingest 流程
```
场景保存 → IngestPipeline::ingest()
→ Step 1: analyze_content() 提取实体关系
→ Step 2: generate_knowledge() 生成知识
→ KnowledgeGraph::save_entities()
→ KnowledgeGraph::save_relations()
→ VectorStore::store()
→ ingest_jobs 表更新状态 (completed/failed)
→ 发射 ingest-job-updated 事件
→ 幕前顶栏 🧠 图标更新
```

### CHAPTER_COMMIT 投影流程 (v6.0.0)
```
章节保存 → ChapterCommitService::init_commit()
→ ChapterCommitService::apply_commit()
→ StateProjectionWriter     → memory_items (category="state")
→ IndexProjectionWriter     → memory_items (category="entity")
→ SummaryProjectionWriter   → story_summaries
→ MemoryProjectionWriter    → memory_items (category="event")
→ VectorProjectionWriter    → LanceDB VectorRecord
→ projection_status_json 记录各 Writer 状态
```

---

## 性能优化

### 前端
- **懒加载**: 幕前/幕后代码分割
- **虚拟列表**: 故事线长列表优化
- **防抖**: 自动保存 2 秒延迟
- **增量更新**: 精确触发重新渲染

### 后端
- **连接池**: r2d2 SQLite 连接复用
- **异步**: Tokio 运行时处理 I/O
- **缓存**: 向量索引内存缓存
- **批量处理**: Ingest 批量写入

---

## 安全考虑

1. **API Key**: 本地存储，界面显示为 `***`
2. **文件访问**: Tauri 能力限制
3. **SQL 注入**: 参数化查询
4. **XSS**: TipTap 内容转义
5. **CORS**: 仅允许本地请求

---

## 开发指南

### 启动开发服务器
```bash
# 前端开发
npm run dev

# 后端开发
cargo tauri dev

# 完整开发
.\start-dev.ps1
```

### 数据库迁移
```bash
# 数据库重置（开发环境）
cd src-tauri && cargo run --bin migrate
```

---

## 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计文档
- [功能清单](docs/FEATURES.md) - 完整功能列表
- [更新日志](CHANGELOG.md) - 版本变更记录
