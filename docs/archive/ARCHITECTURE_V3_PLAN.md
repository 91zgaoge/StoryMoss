# StoryForge (草苔) v3.0 重大架构调整计划

## 概述

基于用户需求，本项目将进行以下四个核心方向的重大架构调整：

1. **小说独立配置系统** - 每部小说拥有独立的幕后工作室配置
2. **AI智能生成小说要素** - 引导式AI生成世界观、角色谱、文字风格
3. **场景化叙事架构** - 用"场景"取代"章节"驱动故事发展
4. **增强记忆系统** - 学习llm_wiki方法论，实现真正的"越写越懂"

---

## 一、小说独立配置系统

### 1.1 配置架构设计

每部小说将拥有独立的幕后工作室配置，存储在 `~/.config/storyforge/studios/{story_id}/` 目录下：

```
studios/
├── {story_id}/
│   ├── studio.json          # 工作室主配置
│   ├── llm_config.json      # LLM配置
│   ├── ui_config.json       # 界面样式配置
│   ├── agent_bots.json      # Agent Bot配置
│   ├── frontstage_theme.css # 幕前界面自定义CSS
│   └── backstage_theme.css  # 幕后界面自定义CSS
```

### 1.2 数据模型变更

#### 新增 `StudioConfig` 结构
```rust
pub struct StudioConfig {
    pub story_id: String,
    pub story_metadata: StoryMetadata,
    pub author_config: AuthorConfig,
    pub world_building: WorldBuilding,
    pub character_profiles: Vec<CharacterProfile>,
    pub writing_style: WritingStyle,
    pub scenes: Vec<Scene>,
    pub llm_config: LlmStudioConfig,
    pub ui_config: UiStudioConfig,
    pub agent_bots: Vec<AgentBotConfig>,
}

pub struct StoryMetadata {
    pub title: String,
    pub pen_name: String,
    pub genre: String,
    pub description: String,
}

pub struct WorldBuilding {
    pub concept: String,           // 宏观世界观概念
    pub rules: Vec<WorldRule>,     // 世界规则
    pub settings: Vec<Setting>,    // 多场景设置
    pub history: String,           // 历史背景
    pub cultures: Vec<Culture>,    // 文化设定
}

pub struct CharacterProfile {
    pub id: String,
    pub name: String,
    pub personality: String,       // 角色人格
    pub background: String,
    pub goals: String,
    pub relationships: Vec<Relationship>,
    pub voice_style: String,       // 语言风格
}

pub struct WritingStyle {
    pub name: String,
    pub description: String,
    pub tone: String,
    pub pacing: String,
    pub vocabulary_level: String,
    pub sentence_structure: String,
    pub custom_rules: Vec<String>,
}
```

### 1.3 配置导入/导出系统

#### 导出功能
- 完整导出：导出所有配置到 `.storyforge` 文件（ZIP格式）
- 选择性导出：用户可选择要导出的配置模块

#### 导入功能
- 完整导入：从 `.storyforge` 文件导入所有配置
- 选择性导入：用户可选择要导入的配置模块（勾选式）
- 冲突处理：检测到同名小说时提示用户选择覆盖或重命名

---

## 二、AI智能生成小说要素

### 2.1 新建小说流程重构

#### 新的新建小说界面
1. 用户点击"新建小说"
2. 进入引导式创建流程
3. 在编辑器中显示灰色提示词："小说类型：玄幻...商战...或随便定"
4. 用户输入小说类型/主题
5. AI根据输入逐步生成要素

### 2.2 分步生成流程

```
用户输入类型 → AI生成世界观选项 → 用户选择/编辑 → 
AI生成角色谱选项 → 用户选择/编辑 → 
AI生成文字风格选项 → 用户选择/编辑 → 
AI生成首个场景 → 开始创作
```

### 2.3 卡片式UI设计

每个AI生成要素以卡片形式呈现：
- **世界观卡片**：展示世界观名称、简介、关键特征
- **角色卡片**：展示角色头像（AI生成）、姓名、人格标签
- **风格卡片**：展示风格名称、描述、示例文本

交互：
- 单击选择（变为选中状态）
- 双击进入编辑模式（可修改内容）
- 右键菜单：重新生成、复制、删除

### 2.4 生成Agent设计

```rust
pub struct NovelCreationAgent {
    llm_adapter: Arc<dyn LlmAdapter>,
}

impl NovelCreationAgent {
    /// 根据用户输入生成世界观选项
    async fn generate_world_building_options(
        &self,
        user_input: &str,
    ) -> Result<Vec<WorldBuilding>, Error> {
        // 使用LLM生成3个世界观选项
    }
    
    /// 根据世界观生成角色谱选项
    async fn generate_character_profiles(
        &self,
        world_building: &WorldBuilding,
    ) -> Result<Vec<Vec<CharacterProfile>>, Error> {
        // 为每个世界观生成3组角色配置
    }
    
    /// 根据类型和世界观生成文字风格
    async fn generate_writing_styles(
        &self,
        genre: &str,
        world_building: &WorldBuilding,
    ) -> Result<Vec<WritingStyle>, Error> {
        // 生成3种文字风格选项
    }
}
```

---

## 三、场景化叙事架构

### 3.1 核心概念变更

| 旧概念 | 新概念 | 说明 |
|--------|--------|------|
| 章节 (Chapter) | 场景 (Scene) | 场景是戏剧冲突的容器 |
| 章节号 | 场景序列 | 场景按故事线顺序排列 |
| 大纲 | 场景目标 | 每个场景有明确的戏剧目标 |

### 3.2 场景数据模型

```rust
pub struct Scene {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,      // 场景序号
    pub title: String,
    
    // 戏剧结构
    pub dramatic_goal: String,      // 戏剧目标：这个场景要完成什么
    pub external_pressure: String,  // 外部压迫：环境/反派/事件对角色的压迫
    pub conflict_type: ConflictType, // 冲突类型
    
    // 角色参与
    pub characters_present: Vec<String>, // 在场角色ID
    pub character_conflicts: Vec<CharacterConflict>, // 角色间冲突
    
    // 内容
    pub content: String,            // 场景正文
    
    // 场景设置
    pub setting: Setting,           // 场景发生的地点/时间
    
    // 关联
    pub previous_scene_id: Option<String>,
    pub next_scene_id: Option<String>,
    
    // 元数据
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

pub enum ConflictType {
    ManVsMan,        // 人与人
    ManVsSelf,       // 人与自我
    ManVsSociety,    // 人与社会
    ManVsNature,     // 人与自然
    ManVsTechnology, // 人与科技
    ManVsFate,       // 人与命运
}

pub struct CharacterConflict {
    pub character_a_id: String,
    pub character_b_id: String,
    pub conflict_nature: String,    // 冲突本质：利益/价值观/情感/...
    pub stakes: String,             // 利害关系
}

pub struct Setting {
    pub location: String,
    pub time: String,
    pub atmosphere: String,         // 氛围
    pub sensory_details: Vec<String>, // 感官细节
}
```

### 3.3 AI场景生成Agent

```rust
pub struct SceneGeneratorAgent {
    llm_adapter: Arc<dyn LlmAdapter>,
    memory_system: Arc<MemorySystem>,
}

impl SceneGeneratorAgent {
    /// 生成下一个场景建议
    async fn generate_next_scene_proposal(
        &self,
        story_id: &str,
        current_scene_id: Option<&str>,
    ) -> Result<SceneProposal, Error> {
        // 1. 查询记忆系统获取故事上下文
        let context = self.memory_system.query_story_context(story_id).await?;
        
        // 2. 分析当前故事线状态
        let story_state = self.analyze_story_progression(&context).await?;
        
        // 3. 生成场景建议（3个选项）
        let proposals = self.llm_adapter.generate_scene_options(
            &context,
            &story_state,
        ).await?;
        
        Ok(proposals)
    }
    
    /// 分析故事线发展状态
    async fn analyze_story_progression(
        &self,
        context: &StoryContext,
    ) -> Result<StoryState, Error> {
        // 分析角色弧、情节推进、冲突升级等
    }
}
```

### 3.4 场景编辑界面

新的场景管理界面：
- **故事线视图**：时间线形式展示场景序列
- **场景卡片**：展示戏剧目标、冲突类型、参与角色
- **快速添加**：一键生成下一个场景建议
- **拖拽排序**：调整场景顺序

---

## 四、增强记忆系统（学习llm_wiki）

### 4.1 系统架构

基于llm_wiki方法论的记忆系统四层架构：

```
┌─────────────────────────────────────────────────────────────┐
│                     Memory System                            │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: Multi-Agent Sessions                               │
│  - 世界观助手、人物助手、文风助手独立会话                      │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: Knowledge Graph                                    │
│  - 实体关系图谱，带加权关系强度                                │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Vector Store                                       │
│  - 场景向量、实体向量、语义检索                                │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Raw Sources                                        │
│  - 小说正文、角色设定、世界设定                                │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 两步思维链Ingest流程

```rust
pub struct IngestPipeline {
    llm_adapter: Arc<dyn LlmAdapter>,
    knowledge_graph: Arc<KnowledgeGraph>,
    vector_store: Arc<VectorStore>,
}

impl IngestPipeline {
    /// 两步思维链：分析 + 生成
    pub async fn ingest(&self, content: &IngestContent) -> Result<(), Error> {
        // Step 1: 分析阶段 - 使用LLM深入分析内容
        let analysis = self.analyze_content(content).await?;
        
        // Step 2: 生成阶段 - 基于分析结果生成知识
        let knowledge = self.generate_knowledge(&analysis).await?;
        
        // 保存到知识图谱和向量存储
        self.save_to_graph(&knowledge).await?;
        self.save_to_vector_store(&knowledge).await?;
        
        Ok(())
    }
    
    async fn analyze_content(&self, content: &IngestContent) -> Result<ContentAnalysis, Error> {
        // LLM分析：实体识别、关系提取、事件抽取、情感分析
        let prompt = format!(
            r#"请深入分析以下小说内容：

{}

请提取：
1. 出现的所有实体（人物、地点、物品、概念）
2. 实体之间的关系
3. 关键事件及其影响
4. 情感变化和氛围
5. 伏笔和照应

以结构化JSON格式输出。"#,
            content.text
        );
        
        let analysis_json = self.llm_adapter.generate(&prompt).await?;
        let analysis: ContentAnalysis = serde_json::from_str(&analysis_json)?;
        
        Ok(analysis)
    }
    
    async fn generate_knowledge(&self, analysis: &ContentAnalysis) -> Result<Knowledge, Error> {
        // 基于分析结果生成结构化知识
        let prompt = format!(
            r#"基于以下分析结果，生成知识库条目：

{}

请生成：
1. 每个实体的详细档案
2. 关系强度评分（0-1）
3. 事件的重要性评分
4. 相关标签和分类

以结构化JSON格式输出。"#,
            serde_json::to_string(analysis)?
        );
        
        let knowledge_json = self.llm_adapter.generate(&prompt).await?;
        let knowledge: Knowledge = serde_json::from_str(&knowledge_json)?;
        
        Ok(knowledge)
    }
}
```

### 4.3 知识图谱（带关系强度）

```rust
pub struct KnowledgeGraph {
    entities: HashMap<String, Entity>,
    relations: Vec<Relation>,
    storage: Arc<dyn GraphStorage>,
}

pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub attributes: HashMap<String, String>,
    pub embedding: Vec<f32>,
    pub first_seen: DateTime<Local>,
    pub last_updated: DateTime<Local>,
}

pub enum EntityType {
    Character,      // 角色
    Location,       // 地点
    Item,           // 物品
    Organization,   // 组织
    Concept,        // 概念
    Event,          // 事件
}

pub struct Relation {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: RelationType,
    pub strength: f32,              // 关系强度 0-1
    pub evidence: Vec<String>,      // 证据引用（场景ID列表）
    pub first_seen: DateTime<Local>,
}

pub enum RelationType {
    Friend, Enemy, Family, Lover,  // 人际关系
    LocatedAt, BelongsTo, Uses,    // 物品关系
    PartOf, Leads, MemberOf,       // 组织关系
    Causes, Enables, Prevents,     // 因果关系
    SimilarTo, OppositeOf,         // 语义关系
}

impl KnowledgeGraph {
    /// 计算关系强度的加权算法
    pub fn calculate_relation_strength(&self, relation: &Relation) -> f32 {
        let base_strength = relation.strength;
        
        // 考虑证据数量
        let evidence_weight = (relation.evidence.len() as f32 * 0.1).min(0.3);
        
        // 考虑关系时间衰减
        let time_factor = self.calculate_time_decay(&relation.first_seen);
        
        // 综合计算
        let final_strength = (base_strength * 0.6 + evidence_weight * 0.4) * time_factor;
        
        final_strength.clamp(0.0, 1.0)
    }
    
    /// 基于关系强度决定引用优先级
    pub fn get_related_entities_by_priority(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Vec<(Entity, f32)> {
        self.relations
            .iter()
            .filter(|r| {
                (r.source_id == entity_id || r.target_id == entity_id)
                    && r.strength >= min_strength
            })
            .map(|r| {
                let related_id = if r.source_id == entity_id {
                    &r.target_id
                } else {
                    &r.source_id
                };
                let entity = self.entities.get(related_id).cloned().unwrap();
                let priority = self.calculate_relation_strength(r);
                (entity, priority)
            })
            .sorted_by(|a, b| b.1.partial_cmp(&a.1).unwrap())
            .collect()
    }
}
```

### 4.4 四阶段查询检索管线

```rust
pub struct QueryPipeline {
    tokenizer: CJKTokenizer,
    knowledge_graph: Arc<KnowledgeGraph>,
    vector_store: Arc<VectorStore>,
    budget_config: BudgetConfig,
}

pub struct BudgetConfig {
    pub total_budget: usize,        // 总token预算 (4K-1M可配)
    pub search_budget_pct: f32,     // 60%
    pub graph_budget_pct: f32,      // 20%
    pub context_budget_pct: f32,    // 5%
    pub assembly_budget_pct: f32,   // 15%
}

impl QueryPipeline {
    /// 四阶段查询检索
    pub async fn query(&self, query: &str) -> Result<QueryResult, Error> {
        // Stage 1: CJK二元组分词搜索
        let search_results = self.token_search(query).await?;
        
        // Stage 2: 图谱扩展
        let graph_expansion = self.graph_expansion(&search_results).await?;
        
        // Stage 3: 预算控制
        let selected = self.budget_control(
            &search_results,
            &graph_expansion,
        ).await?;
        
        // Stage 4: 带引用编号的上下文组装
        let context = self.assemble_context(&selected).await?;
        
        Ok(QueryResult {
            context,
            citations: selected.iter().map(|s| s.citation()).collect(),
        })
    }
    
    /// Stage 1: CJK二元组分词搜索
    async fn token_search(&self, query: &str) -> Result<Vec<SearchResult>, Error> {
        // 对查询进行CJK二元组分词
        let tokens = self.tokenizer.tokenize(query);
        
        // 在向量存储中进行多token搜索
        let results = self.vector_store
            .search_with_tokens(&tokens, 50)
            .await?;
        
        Ok(results)
    }
    
    /// Stage 2: 图谱扩展
    async fn graph_expansion(
        &self,
        search_results: &[SearchResult],
    ) -> Result<Vec<GraphResult>, Error> {
        let mut expanded = Vec::new();
        
        for result in search_results {
            // 找到结果相关的实体
            if let Some(entity) = self.knowledge_graph.find_entity_by_content(&result.content) {
                // 扩展相关实体（基于关系强度）
                let related = self.knowledge_graph
                    .get_related_entities_by_priority(&entity.id, 0.3);
                
                for (rel_entity, strength) in related {
                    expanded.push(GraphResult {
                        entity: rel_entity,
                        relation_strength: strength,
                        source: result.clone(),
                    });
                }
            }
        }
        
        // 去重并按关系强度排序
        expanded.sort_by(|a, b| b.relation_strength.partial_cmp(&a.relation_strength).unwrap());
        expanded.dedup_by(|a, b| a.entity.id == b.entity.id);
        
        Ok(expanded)
    }
    
    /// Stage 3: 预算控制
    async fn budget_control(
        &self,
        search_results: &[SearchResult],
        graph_expansion: &[GraphResult],
    ) -> Result<Vec<SelectedContext>, Error> {
        let total_budget = self.budget_config.total_budget;
        let search_budget = (total_budget as f32 * self.budget_config.search_budget_pct) as usize;
        let graph_budget = (total_budget as f32 * self.budget_config.graph_budget_pct) as usize;
        
        let mut selected = Vec::new();
        let mut used_budget = 0;
        
        // 优先选择搜索结果的Top-K
        for result in search_results.iter().take(10) {
            let cost = result.content.len();
            if used_budget + cost > search_budget {
                break;
            }
            selected.push(SelectedContext::from_search(result));
            used_budget += cost;
        }
        
        // 然后选择图谱扩展结果
        for graph_result in graph_expansion {
            let cost = graph_result.entity.description.len();
            if used_budget + cost > search_budget + graph_budget {
                break;
            }
            selected.push(SelectedContext::from_graph(graph_result));
            used_budget += cost;
        }
        
        Ok(selected)
    }
    
    /// Stage 4: 带引用编号的上下文组装
    async fn assemble_context(
        &self,
        selected: &[SelectedContext],
    ) -> Result<String, Error> {
        let mut context_parts = Vec::new();
        
        for (idx, item) in selected.iter().enumerate() {
            let citation_num = idx + 1;
            context_parts.push(format!(
                "[{}] {}\n",
                citation_num,
                item.content
            ));
        }
        
        Ok(context_parts.join("\n"))
    }
}
```

### 4.5 多助手独立会话

```rust
pub struct MultiAgentSessionManager {
    sessions: HashMap<AgentType, AgentSession>,
    memory_system: Arc<MemorySystem>,
}

pub enum AgentType {
    WorldBuilding,      // 世界观助手
    Character,          // 人物助手
    WritingStyle,       // 文风助手
    Plot,               // 情节助手
    Research,           // 研究助手
}

pub struct AgentSession {
    pub agent_type: AgentType,
    pub messages: Vec<Message>,
    pub used_wiki_pages: Vec<String>, // 标注使用了哪些Wiki页面
    pub created_at: DateTime<Local>,
}

impl MultiAgentSessionManager {
    /// 发送消息到特定助手
    pub async fn chat(
        &mut self,
        agent_type: AgentType,
        message: &str,
    ) -> Result<String, Error> {
        let session = self.sessions.get_mut(&agent_type).unwrap();
        
        // 查询记忆系统获取相关上下文
        let context = self.memory_system.query_for_agent(&agent_type, message).await?;
        
        // 构建完整提示
        let full_prompt = format!(
            "{system_prompt}\n\n相关背景知识：\n{context}\n\n用户：{message}\n\n助手：",
            system_prompt = self.get_system_prompt(&agent_type),
            context = context,
            message = message
        );
        
        // 调用LLM
        let response = self.llm_adapter.generate(&full_prompt).await?;
        
        // 记录使用的Wiki页面
        let used_pages = self.extract_wiki_references(&response);
        session.used_wiki_pages.extend(used_pages);
        
        // 保存对话
        session.messages.push(Message::user(message.to_string()));
        session.messages.push(Message::assistant(response.clone()));
        
        Ok(response)
    }
    
    /// 保存对话结果到Wiki
    pub async fn save_chat_to_wiki(
        &self,
        agent_type: AgentType,
        title: &str,
    ) -> Result<(), Error> {
        let session = self.sessions.get(&agent_type).unwrap();
        
        // 使用Ingest流程处理对话内容
        let content = IngestContent {
            text: format!("{}\n\n使用的参考资料：{:?}", 
                session.messages.iter().map(|m| m.to_string()).collect::<Vec<_>>().join("\n"),
                session.used_wiki_pages
            ),
            source: format!("chat:{:?}", agent_type),
            timestamp: Local::now(),
        };
        
        self.memory_system.ingest(&content).await?;
        
        Ok(())
    }
}
```

---

## 五、数据库Schema变更

### 5.1 新表结构

```sql
-- 场景表（替换章节表）
CREATE TABLE scenes (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    title TEXT,
    dramatic_goal TEXT,             -- 戏剧目标
    external_pressure TEXT,         -- 外部压迫
    conflict_type TEXT,             -- 冲突类型
    characters_present TEXT,        -- JSON: [character_id, ...]
    character_conflicts TEXT,       -- JSON: [{a, b, nature, stakes}, ...]
    setting_location TEXT,
    setting_time TEXT,
    setting_atmosphere TEXT,
    content TEXT,
    previous_scene_id TEXT,
    next_scene_id TEXT,
    model_used TEXT,
    cost REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
    FOREIGN KEY (previous_scene_id) REFERENCES scenes(id),
    FOREIGN KEY (next_scene_id) REFERENCES scenes(id),
    UNIQUE(story_id, sequence_number)
);

-- 世界观表
CREATE TABLE world_buildings (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL UNIQUE,
    concept TEXT NOT NULL,          -- 宏观世界观概念
    rules TEXT,                     -- JSON: 世界规则列表
    history TEXT,
    cultures TEXT,                  -- JSON: 文化设定
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

-- 世界规则表
CREATE TABLE world_rules (
    id TEXT PRIMARY KEY,
    world_building_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    rule_type TEXT,                 -- magic/technology/social/...
    importance INTEGER,             -- 1-10
    created_at TEXT NOT NULL,
    FOREIGN KEY (world_building_id) REFERENCES world_buildings(id) ON DELETE CASCADE
);

-- 场景设置表
CREATE TABLE settings (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    location_type TEXT,             -- city/building/nature/...
    sensory_details TEXT,           -- JSON: 感官细节
    significance TEXT,              -- 在故事中的重要性
    created_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

-- 文字风格表
CREATE TABLE writing_styles (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL UNIQUE,
    name TEXT,
    description TEXT,
    tone TEXT,
    pacing TEXT,
    vocabulary_level TEXT,
    sentence_structure TEXT,
    custom_rules TEXT,              -- JSON: 自定义规则
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

-- 知识图谱实体表
CREATE TABLE kg_entities (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL,      -- character/location/item/...
    attributes TEXT,                -- JSON
    embedding BLOB,                 -- 向量嵌入
    first_seen TEXT NOT NULL,
    last_updated TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

-- 知识图谱关系表
CREATE TABLE kg_relations (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    strength REAL NOT NULL,         -- 0-1
    evidence TEXT,                  -- JSON: 场景ID列表
    first_seen TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
    FOREIGN KEY (source_id) REFERENCES kg_entities(id),
    FOREIGN KEY (target_id) REFERENCES kg_entities(id)
);

-- 创建索引
CREATE INDEX idx_scenes_story ON scenes(story_id);
CREATE INDEX idx_scenes_sequence ON scenes(story_id, sequence_number);
CREATE INDEX idx_kg_entities_story ON kg_entities(story_id);
CREATE INDEX idx_kg_entities_type ON kg_entities(entity_type);
CREATE INDEX idx_kg_relations_story ON kg_relations(story_id);
CREATE INDEX idx_kg_relations_source ON kg_relations(source_id);
CREATE INDEX idx_kg_relations_target ON kg_relations(target_id);
```

---

## 六、实施计划

### Phase 1: 基础架构重构（2-3周）

1. **数据库迁移**
   - 创建新表结构
   - 编写数据迁移脚本（章节→场景）
   - 更新Repository层

2. **配置系统实现**
   - 实现StudioConfig数据模型
   - 实现配置导入/导出功能
   - 配置文件系统存储

3. **类型定义更新**
   - 更新Rust模型
   - 更新TypeScript类型
   - 更新API接口定义

### Phase 2: 场景化架构（2-3周）

1. **场景管理实现**
   - SceneRepository CRUD
   - 场景序列管理
   - 场景关联维护

2. **场景编辑界面**
   - 故事线视图组件
   - 场景卡片组件
   - 场景编辑器

3. **场景生成Agent**
   - SceneGeneratorAgent实现
   - 场景建议API
   - 场景导入流程

### Phase 3: AI智能生成（2周）

1. **引导式创建流程**
   - 新建小说向导组件
   - 类型输入界面（灰色提示词）
   - 分步生成流程

2. **卡片式UI**
   - 世界观光卡组件
   - 角色卡片组件
   - 风格卡片组件
   - 双击编辑功能

3. **生成Agent实现**
   - NovelCreationAgent
   - 世界观生成Prompt
   - 角色谱生成Prompt
   - 文风生成Prompt

### Phase 4: 记忆系统（3-4周）

1. **知识图谱系统**
   - Entity/Relation模型
   - 图谱存储实现
   - 关系强度计算

2. **Ingest管线**
   - 两步思维链实现
   - 内容分析Agent
   - 知识生成Agent

3. **查询检索管线**
   - CJK分词器
   - 四阶段检索实现
   - 预算控制

4. **多助手会话**
   - 会话管理器
   - Wiki引用跟踪
   - 保存到Wiki功能

### Phase 5: 集成测试与优化（1-2周）

1. **集成测试**
   - 端到端测试
   - 数据迁移验证
   - 性能测试

2. **UI/UX优化**
   - 交互细节优化
   - 加载状态处理
   - 错误处理

3. **文档更新**
   - 架构文档更新
   - API文档更新
   - 用户指南

---

## 七、技术要点

### 7.1 CJK分词实现

```rust
pub struct CJKTokenizer;

impl CJKTokenizer {
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        
        for window in chars.windows(2) {
            let token: String = window.iter().collect();
            // 只保留CJK字符的二元组
            if self.is_cjk(window[0]) && self.is_cjk(window[1]) {
                tokens.push(token);
            }
        }
        
        tokens
    }
    
    fn is_cjk(&self, c: char) -> bool {
        matches!(c as u32,
            0x4E00..=0x9FFF |    // CJK Unified Ideographs
            0x3040..=0x309F |    // Hiragana
            0x30A0..=0x30FF |    // Katakana
            0xAC00..=0xD7AF      // Hangul
        )
    }
}
```

### 7.2 向量存储集成

使用LanceDB或纯Rust实现的向量存储：

```rust
pub struct VectorStore {
    db: Arc<lancedb::Connection>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

impl VectorStore {
    pub async fn store_scene(&self, scene: &Scene) -> Result<()> {
        let embedding = self.embedding_provider.embed(&scene.content).await?;
        
        let table = self.db.open_table("scenes").execute().await?;
        table.add(vec![SceneVectorRecord {
            id: scene.id.clone(),
            content: scene.content.clone(),
            embedding,
            metadata: json!({
                "story_id": scene.story_id,
                "sequence": scene.sequence_number,
            }),
        }]).execute().await?;
        
        Ok(())
    }
}
```

### 7.3 配置导出格式

`.storyforge`文件结构（ZIP格式）：

```
story.studioforge/
├── manifest.json          # 元数据
├── studio.json            # 工作室配置
├── world_building.json    # 世界观
├── characters.json        # 角色谱
├── writing_style.json     # 文字风格
├── scenes.json            # 场景数据
├── llm_config.json        # LLM配置
├── ui_config.json         # UI配置
└── agent_bots.json        # Agent配置
```

---

## 八、风险评估

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| 数据迁移复杂 | 高 | 高 | 充分测试迁移脚本，保留原数据备份 |
| LLM调用成本高 | 中 | 中 | 实现本地模型支持，增加缓存机制 |
| 性能问题 | 中 | 中 | 异步处理，增量更新，性能测试 |
| 用户体验变化大 | 中 | 中 | 提供切换选项，渐进式引导 |

---

## 九、依赖项

### 新增Rust依赖

```toml
[dependencies]
# 知识图谱
petgraph = "0.6"

# CJK分词
jieba-rs = "0.6"

# 向量存储（可选，也可用纯Rust实现）
lancedb = "0.4"

# ZIP文件处理（配置导出）
zip = "0.6"

# 正则表达式（分词）
regex = "1.10"
```

---

## 十、总结

本次架构调整将使StoryForge从一个传统的小说写作工具转变为一个真正智能的、以场景驱动创作的AI辅助写作平台。核心亮点包括：

1. **独立工作室**：每部小说拥有完整的独立配置
2. **引导式创作**：AI引导用户从无到有构建小说世界
3. **场景驱动**：以戏剧冲突为核心驱动故事发展
4. **深度记忆**：真正理解故事内容，实现"越写越懂"

整体实施预计需要 **10-14周** 完成。
