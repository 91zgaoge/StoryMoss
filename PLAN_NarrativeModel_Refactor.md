# StoryForge 创世-拆书同构架构重构计划

## 背景与洞察

用户的深刻洞察：**Bootstrap（生成小说）和 BookDeconstruction（拆书）是两个可逆的过程**。

- **Bootstrap（正向/创世）**：概念 → 世界观 → 大纲 → 角色 → 场景 → 伏笔 → 正文
- **拆书（逆向/分解）**：正文 → 元信息 → 世界观 → 人物 → 章节概要 → 故事线

**结构要素相同，工作流程可正向逆向。**

---

## 一、现状分析：同构性诊断

### 1.1 结构要素映射表

| 结构要素 | Bootstrap（生产表） | 拆书（参考表） | 同构度 |
|---------|-------------------|--------------|-------|
| 故事元信息 | `stories` (title, genre, description) | `reference_books` (title, genre, author) | **高** |
| 世界观 | `world_buildings` (concept, rules, history, cultures) | `reference_books.world_setting` (纯文本) | **中** — 参考表缺失结构化规则 |
| 角色 | `characters` (name, personality, goals, appearance, gender, age) | `reference_characters` (name, personality, appearance, role_type) | **高** — 字段几乎一致 |
| 场景 | `scenes` (title, summary, conflict_type, characters_present, dramatic_goal) | `reference_scenes` (title, summary, conflict_type, characters_present) | **高** — 字段几乎一致 |
| 大纲 | `story_outlines` (acts, structure_json, total_scenes_estimate) | `reference_books.story_arc` (纯文本JSON) | **中** — 参考表用文本存储 |
| 角色关系 | `character_relationships` (关系表) | `reference_characters.relationships` (JSON字符串) | **低** — 存储方式完全不同 |
| 伏笔 | `foreshadowing_tracker` (独立表) | **无** | **低** — 拆书不提取伏笔 |
| 知识图谱 | `kg_entities` + `kg_relations` | **无** | **低** — 拆书不构建KG |
| 正文 | `chapters.content` | 源文件保留 | **中** — 存储方式不同 |

### 1.2 流程对比

**Bootstrap 流程（`planner/bootstrap.rs`）**：
```
run_quick_phase():
  1. generate_story_concept() → LLM → StoryConcept
  2. create Story record → DB
  3. generate_first_chapter() → AgentService → Chapter.content
  
run_background_phase():
  4. generate_world_building() → LLM → WorldBuilding
  5. generate_story_outline() → LLM → StoryOutline
  6. generate_characters() → LLM → Character[]
  7. generate_scene_outline() → LLM → Scene[]
  8. generate_foreshadowing() → LLM → Foreshadowing[]
  9. create_genesis_knowledge_graph() → KG
```

**拆书流程（`book_deconstruction/analyzer.rs`）**：
```
analyze():
  1. extract_metadata() → LLM → ExtractedMetadata
  2. extract_world_setting() → LLM → String
  3. extract_characters() → LLM (逐块并行) → ReferenceCharacter[]
  4. extract_scene_summaries() → LLM (逐块并行) → ReferenceScene[]
  5. extract_story_arc() → LLM → ExtractedStoryArc
  6. build_book_result() → ReferenceBook
```

### 1.3 问题诊断

| 问题 | 严重程度 | 描述 |
|------|---------|------|
| **数据模型分裂** | 🔴 P0 | 生产表和参考表结构不同，无法共享查询/分析逻辑 |
| **流程重复实现** | 🔴 P0 | 两个模块独立实现相似的LLM调用、进度报告、错误处理 |
| **Prompt 不共享** | 🟡 P1 | 世界观/角色/场景/大纲的prompt在两个模块中独立维护 |
| **进度系统分裂** | 🟡 P1 | `novel-bootstrap-progress` vs `book-analysis-progress` |
| **拆书结果不完整** | 🟡 P1 | 拆书不提取伏笔、不构建知识图谱、世界观非结构化 |
| **无双向桥接** | 🟡 P1 | 拆书→故事（已有但独立实现），故事→拆书分析（无） |
| **存储冗余** | 🟢 P2 | 角色关系一个用关系表、一个用JSON字段 |

---

## 二、架构愿景：StoryForge 叙事元素模型（Narrative Element Model）

### 2.1 核心设计理念

**"无论正向生成还是逆向分析，操作的叙事元素是同一套抽象。"**

```
┌─────────────────────────────────────────────────────────────┐
│                  NarrativeElement 抽象层                      │
├─────────────┬─────────────┬─────────────┬─────────────────┤
│  StoryMeta  │ WorldBuilding│  Character  │     Scene       │
│  (元信息)    │  (世界观)    │   (角色)     │    (场景)       │
├─────────────┼─────────────┼─────────────┼─────────────────┤
│   Outline   │Relationship │Foreshadowing│KnowledgeGraph   │
│  (大纲)      │  (关系)      │   (伏笔)     │   (知识图谱)     │
└─────────────┴─────────────┴─────────────┴─────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌──────────────────────────┐    ┌──────────────────────────┐
│   GenesisPipeline        │    │   AnalysisPipeline       │
│   (正向/创世)             │    │   (逆向/分析)             │
│                          │    │                          │
│  输入: 用户概念            │    │  输入: 小说正文            │
│  输出: NarrativeElement[] │    │  输出: NarrativeElement[] │
│                          │    │                          │
│  LLM Prompt: "生成..."    │    │  LLM Prompt: "提取..."    │
└──────────────────────────┘    └──────────────────────────┘
```

### 2.2 统一数据模型

```rust
// === 核心抽象 ===

/// 叙事元素 — 故事的任何一个结构化组成部分
pub trait NarrativeElement {
    fn element_type(&self) -> ElementType;
    fn story_id(&self) -> &str;
    fn to_json(&self) -> serde_json::Value;
}

pub enum ElementType {
    StoryMeta,      // 故事元信息
    WorldBuilding,  // 世界观
    Character,      // 角色
    Scene,          // 场景
    Outline,        // 大纲
    Relationship,   // 角色关系
    Foreshadowing,  // 伏笔
    PlotPoint,      // 情节点
}

// === 具体实现（替换现有模型）===

/// 统一的角色模型 — 生产表和参考表共用
pub struct CharacterElement {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub role_type: String,       // 主角/反派/导师/盟友...
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub fears: String,
    pub appearance: String,
    pub gender: String,
    pub age: i32,
    pub relationships: Vec<CharacterRelationship>,
    pub importance_score: f32,   // 1-10
    pub source: ElementSource,   // 区分来源
}

/// 元素来源 — 标识这个数据是怎么来的
pub enum ElementSource {
    Generated,      // AI生成（Bootstrap）
    Extracted,      // 从文本提取（拆书）
    UserCreated,    // 用户手动创建
    Imported,       // 从外部导入
}

/// 统一的场景模型
pub struct SceneElement {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,
    pub title: String,
    pub summary: String,
    pub dramatic_goal: String,
    pub external_pressure: String,
    pub conflict_type: ConflictType,
    pub characters_present: Vec<String>,
    pub setting_location: String,
    pub setting_time: String,
    pub content: Option<String>,  // 正文内容（可选）
    pub source: ElementSource,
}

/// 统一的世界观模型
pub struct WorldBuildingElement {
    pub id: String,
    pub story_id: String,
    pub concept: String,
    pub rules: Vec<WorldRule>,
    pub history: String,
    pub key_locations: Vec<String>,
    pub power_system: String,
    pub source: ElementSource,
}
```

### 2.3 统一存储层

**方案：单层表 + source 字段区分**

不再维护两套表（`characters` vs `reference_characters`），而是使用统一的表，通过 `source` 字段区分数据是怎么来的。

```sql
-- 统一角色表（替代 characters + reference_characters）
CREATE TABLE narrative_characters (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    name TEXT NOT NULL,
    role_type TEXT,
    personality TEXT,
    background TEXT,
    goals TEXT,
    appearance TEXT,
    gender TEXT,
    age INTEGER,
    importance_score REAL,
    source TEXT NOT NULL DEFAULT 'user_created',  -- 'generated' | 'extracted' | 'user_created' | 'imported'
    source_ref_id TEXT,  -- 如果来源是拆书，关联 reference_book_id
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

-- 统一的场景表（替代 scenes + reference_scenes）
CREATE TABLE narrative_scenes (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    title TEXT,
    summary TEXT,
    dramatic_goal TEXT,
    external_pressure TEXT,
    conflict_type TEXT,
    characters_present TEXT,  -- JSON
    setting_location TEXT,
    setting_time TEXT,
    content TEXT,
    source TEXT NOT NULL DEFAULT 'user_created',
    source_ref_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
```

**向后兼容策略**：保留现有表，通过视图或同步机制过渡。

---

## 三、架构优化计划

### Phase 1: 统一叙事元素模型（数据层重构）

**目标**：消除生产表和参考表的数据模型分裂。

#### 1.1 定义核心抽象 trait

- 新建 `src-tauri/src/narrative/` 模块
- 定义 `NarrativeElement` trait 和 `ElementType` / `ElementSource` 枚举
- 定义统一的数据结构：`CharacterElement`, `SceneElement`, `WorldBuildingElement`, `OutlineElement`, `ForeshadowingElement`

#### 1.2 统一存储层

- **Migration**：新建 `narrative_characters`, `narrative_scenes`, `narrative_world_buildings` 等统一表
- **Repository**：新建统一的 Repository，支持按 `source` 过滤查询
- **兼容层**：现有 Repository 改为读写兼容视图，逐步迁移

#### 1.3 双向转换器

- `Converter::from_bootstrap(elements) -> DB records`
- `Converter::from_analysis(elements) -> DB records`
- `Converter::to_production(elements) -> 写入生产表`
- `Converter::to_reference(elements) -> 写入参考表`

**工作量**：~3天 | **风险**：数据迁移 | **测试**：需要全面的数据一致性测试

---

### Phase 2: 抽象 NarrativePipeline（流程层重构）

**目标**：提取 Bootstrap 和拆书的共同流程模式，形成可复用的 Pipeline 框架。

#### 2.1 Pipeline 抽象接口

```rust
/// 叙事流水线 — 可正向（生成）可逆向（分析）
pub trait NarrativePipeline {
    type Input;
    type Output;
    type Context;  // 进度上下文
    
    fn steps(&self) -> Vec<Box<dyn PipelineStep<Self::Context>>>;
    
    async fn execute(
        &self,
        input: Self::Input,
        progress_callback: Box<dyn Fn(PipelineProgressEvent) + Send>,
    ) -> Result<Self::Output, PipelineError>;
}

/// 单个处理步骤
#[async_trait]
pub trait PipelineStep<Context> {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    
    async fn execute(
        &self,
        ctx: &mut Context,
        llm: &LlmService,
    ) -> Result<(), StepError>;
}
```

#### 2.2 GenesisPipeline（正向）

```rust
pub struct GenesisPipeline {
    steps: Vec<Box<dyn PipelineStep<GenesisContext>>>,
}

impl GenesisPipeline {
    pub fn new() -> Self {
        Self {
            steps: vec![
                Box::new(ConceptGenerationStep),
                Box::new(WorldBuildingGenerationStep),
                Box::new(OutlineGenerationStep),
                Box::new(CharacterGenerationStep),
                Box::new(SceneGenerationStep),
                Box::new(ForeshadowingGenerationStep),
                Box::new(KnowledgeGraphGenerationStep),
            ],
        }
    }
}
```

#### 2.3 AnalysisPipeline（逆向）

```rust
pub struct AnalysisPipeline {
    steps: Vec<Box<dyn PipelineStep<AnalysisContext>>>,
}

impl AnalysisPipeline {
    pub fn new() -> Self {
        Self {
            steps: vec![
                Box::new(MetadataExtractionStep),
                Box::new(WorldBuildingExtractionStep),
                Box::new(CharacterExtractionStep),
                Box::new(SceneExtractionStep),
                Box::new(StoryArcExtractionStep),
                Box::new(ForeshadowingExtractionStep),  // 新增：提取伏笔
                Box::new(KnowledgeGraphExtractionStep),  // 新增：从文本构建KG
            ],
        }
    }
}
```

#### 2.4 统一 Prompt 模板系统

每个 PipelineStep 共享同一套 Prompt 模板，只是方向不同：

```rust
pub mod prompts {
    /// 世界观 Prompt — 可生成也可提取
    pub fn world_building_prompt(mode: PromptMode, context: &str) -> String {
        match mode {
            PromptMode::Generate => format!("请为以下故事生成世界观...\n{}", context),
            PromptMode::Extract => format!("请从以下文本中提取世界观设定...\n{}", context),
        }
    }
    
    /// 角色 Prompt
    pub fn character_prompt(mode: PromptMode, context: &str) -> String { ... }
    
    /// 场景 Prompt
    pub fn scene_prompt(mode: PromptMode, context: &str) -> String { ... }
}
```

**工作量**：~5天 | **风险**：Pipeline 抽象过度设计 | **测试**：需要验证两个 Pipeline 都能正确运行

---

### Phase 3: 统一进度与事件系统

**目标**：消除 `novel-bootstrap-progress` 和 `book-analysis-progress` 两套进度系统。

#### 3.1 统一进度事件

```rust
/// 统一的流水线进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineProgressEvent {
    pub pipeline_id: String,           // 流水线ID
    pub pipeline_type: PipelineType,   // Genesis | Analysis
    pub step_name: String,
    pub step_number: usize,
    pub total_steps: usize,
    pub status: StepStatus,            // Running | Completed | Failed
    pub message: String,
    pub progress_percent: i32,         // 0-100
    pub elapsed_seconds: u64,
    pub metadata: Option<serde_json::Value>,
}
```

#### 3.2 前端统一进度组件

```tsx
// 统一的进度显示组件
interface PipelineProgressProps {
  pipelineId: string;
  pipelineType: 'genesis' | 'analysis';
  steps: PipelineStep[];
  currentStep: number;
  message: string;
}

// 同时用于 Bootstrap 和拆书
<PipelineProgress 
  pipelineId={sessionId}
  pipelineType="genesis"
  ...
/>
```

**工作量**：~2天 | **风险**：前端事件监听改动 | **测试**：手动测试进度显示

---

### Phase 4: 拆书功能增强（逆向完整性）

**目标**：让拆书能够提取和生产表同样完整的结构要素。

#### 4.1 新增提取步骤

- **伏笔提取**：从文本中识别已埋设的伏笔和回收点
- **知识图谱构建**：从文本中提取实体和关系，构建KG
- **结构化世界观**：提取规则体系（而非纯文本描述）

#### 4.2 拆书→故事项目 一键转换优化

利用统一的 NarrativeElement 模型，转换过程变为：
```
ReferenceBook (拆书结果)
  → CharacterElement[] / SceneElement[] / WorldBuildingElement[]
  → 直接写入 narrative_characters / narrative_scenes / narrative_world_buildings
  → 自动创建 Story 记录
```

不再需要独立的 "convert_to_story" 逻辑。

**工作量**：~3天 | **风险**：LLM 提取质量 | **测试**：用多本小说测试提取质量

---

### Phase 5: 故事项目 → 拆书分析（新增正向分析）

**目标**：让用户可以对已有的故事项目进行"分析"，验证结构完整性。

#### 5.1 已有故事的结构分析

```rust
/// 对已有故事进行结构分析
pub async fn analyze_existing_story(story_id: &str) -> Result<AnalysisReport, Error> {
    // 1. 读取故事的所有元素（角色、场景、世界观、大纲）
    // 2. 分析结构完整性
    // 3. 检测潜在问题（伏笔未回收、角色弧光不完整、场景冲突类型单一）
    // 4. 生成分析报告
}
```

#### 5.2 应用场景

- 写作中途"体检"：分析当前故事结构是否健康
- 导出前检查：确保所有伏笔都有回收计划
- 对标分析：将自己的故事和参考书的结构对比

**工作量**：~4天 | **风险**：分析质量取决于LLM | **测试**：用已有故事测试

---

## 四、实施路线图

```
Week 1-2: Phase 1 — 统一叙事元素模型（数据层）
  ├─ Day 1-2: 定义核心 trait 和数据结构
  ├─ Day 3-4: 新建统一表 + Migration
  └─ Day 5-7: 兼容层 + 转换器 + 测试

Week 3-4: Phase 2 — 抽象 NarrativePipeline（流程层）
  ├─ Day 1-3: Pipeline trait + 通用基础设施
  ├─ Day 4-6: GenesisPipeline 重构
  ├─ Day 7-9: AnalysisPipeline 重构
  └─ Day 10: 统一 Prompt 模板系统

Week 5: Phase 3 — 统一进度与事件系统
  ├─ Day 1-2: 后端统一进度事件
  └─ Day 3-5: 前端统一进度组件

Week 6: Phase 4 — 拆书功能增强
  ├─ Day 1-2: 新增提取步骤（伏笔、KG）
  └─ Day 3-5: 拆书→故事转换优化

Week 7-8: Phase 5 — 故事→分析（可选）
  └─ 根据用户反馈决定是否实施
```

---

## 五、方案选择

### 选项 A：渐进式重构（推荐）

**策略**：不一次性推翻现有架构，而是：
1. 先新建 `narrative/` 模块和统一模型
2. 在新功能中使用新模型
3. 逐步将旧代码迁移到新模型
4. 保留旧表作为兼容视图，最终 deprecate

**优点**：
- 风险可控，每阶段都可独立测试
- 不影响现有功能
- 可以按需实施（先做Phase 1+2，Phase 3-5后续再说）

**缺点**：
- 过渡期存在两套代码
- 完成时间较长

### 选项 B：大爆炸式重构

**策略**：一次性重写 Bootstrap 和拆书模块，直接基于新架构。

**优点**：
- 架构最干净
- 没有过渡期负担

**缺点**：
- 风险极高，193个测试需要全部重写
- 开发周期长（4-6周）
- 可能导致回归问题

---

## 六、立即可以实施的小改进（不依赖大重构）

在等待大重构批准期间，以下改进可以立即实施：

1. **拆书提取结构化世界观**：修改 `extract_world_setting` 的 prompt，让它输出和 Bootstrap 相同的 JSON 结构（concept, rules, history, key_locations, power_system）
2. **拆书提取伏笔**：新增 `extract_foreshadowing` 步骤
3. **拆书构建知识图谱**：新增 `extract_knowledge_graph` 步骤
4. **统一进度消息格式**：将 Bootstrap 和拆书的进度事件统一为相同的格式

---

## 七、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 数据迁移失败 | 高 | 保留旧表，新表通过 migration + 数据同步建立 |
| Pipeline 抽象过度 | 中 | 保持 trait 简单，不追求过度抽象 |
| LLM 提取质量下降 | 中 | 保留现有拆书作为 fallback |
| 开发周期过长 | 中 | 分 Phase 实施，每 Phase 可独立交付 |
| 测试覆盖不足 | 高 | 每个 Phase 新增单元测试 + 集成测试 |

---

## 八、预期收益

1. **代码复用率提升**：Prompt 模板、进度系统、错误处理复用
2. **维护成本降低**：同一套模型，一处修改全局生效
3. **新功能开发加速**：新增"结构分析"、"对标比较"等功能更容易
4. **用户体验统一**：Bootstrap 和拆书的进度显示、交互方式一致
5. **架构美感**：可逆性体现在代码结构中，成为项目的设计亮点
