# StoryForge (草苔) v0.26.14 架构文档

> 本文档反映 v0.26.14 最新架构状态（2026-07-05）
> **v0.26.14 稳定性补丁**：修复 Genesis 新小说第一章模型输出自重复并降低幕前诊断日志压力。日志证实 v0.26.13 数据层与渲染层均未重复追加内容，用户看到的「首尾段落相同」来自 LLM 生成的正文自身循环。新增 `trimSelfRepetition` 工具，在 `appendAiContent` 与 `smart_execute.finalContent` 进入编辑器/幽灵文本前做段落级与 KMP border 级自重复清理；同时 `RichTextEditor` 的 `frontstage:rich_editor_diag` 渲染日志从每帧记录改为前 20 帧 + 幽灵状态变化 + 200ms IPC 节流，减少长时间写作或文思活跃模式下的 IPC 与日志开销。
> **v0.26.13 稳定性补丁**：修复 Genesis 新小说第一章渲染层视觉重复。数据层已确保只追加一次，但 `RichTextEditor` 的幽灵树条件 `!!(generatedText || isGenerating)` 会在 `generatedText` 为空但 `isGenerating=true` 时渲染空幽灵容器；该容器若残留旧内容或 React 复用 DOM 节点异常，就会造成「正文 + 幽灵文本」同框的虚假重复。改为 `!!generatedText` 后幽灵树只在有实际幽灵文本时渲染；`FrontstageApp` Genesis 自动接受路径先 `setIsGenerating(false)` 确保幽灵树立即卸载；诊断日志增加 `isGenerating`、`isHidingGhost`、`bodyHidingGhost`、`generatedTextLen`。
> **v0.26.12 稳定性补丁**：修复 `RichTextEditor` 角色点击 effect 在 `characters` 为 `null` 时访问 `.length` 导致的幕前白屏崩溃；加固 `useSubscription` 对 `null` 订阅状态的兼容；新增 Playwright E2E 回归测试覆盖「已有故事 + 新写末世小说」完整流程；`frontstage/main.tsx` 与 `ErrorBoundary` 增强崩溃诊断输出。
> **v0.26.11 稳定性补丁**：修复 Genesis 自动接受第一章后 store-editor 失步问题。`appendAiContent` 追加后立即用 `editorRef.getHTML()` 同步 store 与 `latestContentRef`；`RichTextEditor.appendText` 空文档分支标记外部同步并更新 `lastExternalContentRef`，防止 content prop 被外部同步 effect 再次 setContent；`RichTextEditorRef` 新增 `getHTML()`。同时确认 `tauri.conf.json` `devUrl` 指向 dev server，避免开发模式加载陈旧 dist 崩溃。
> **v0.26.9 稳定性补丁**：在 v0.26.8 基础上进一步根治 Genesis 新小说第一章内容重复问题。重复检测与前缀去重统一改用 `latestContentRef.current`（React state 同步快照），避免 TipTap DOM 滞后导致已有正文被再次追加或恢复为幽灵文本；`RichTextEditor` 幽灵文本直接包含检测剥离 HTML 标签，覆盖 ContentUpdate/AppendContent 路径。
> **v0.26.8 稳定性补丁**：在 v0.26.7 基础上彻底修复 Genesis 新小说第一章内容重复问题。新增 `isTextDuplicate` 归一化去重工具与 `isTextAlreadyInEditor` helper，覆盖 pipeline-complete / ChapterSwitch / smart_execute 等多条竞态路径，确保 DB 正文不会与幽灵文本叠加。
> **v0.26.7 稳定性补丁**：修复 `FrontstageApp` pipeline-complete effect 无限循环（React #185）与 Genesis 新小说第一章内容重复问题，核心回调全部稳定化。
> **v0.26.0 重大变更**：数据飞轮 + Harness 可观测性 + 子代理协作。`WorkspaceService` 初始化 `.storyforge/` 工作空间并自动 Git 提交；`PreferencePairExporter` 把用户接受/拒绝反馈导出为 RLHF 成对数据；`TraceStore` 为每次生成请求建立 `trace_id`，在 `GatewayRequest` / `GenerateRequest` / `LlmGeneratingProgress` 全链路透传，前端新增「生成链路」面板；`Subagent` trait 与 `ContinuityAgent` / `StyleAgent` / `WorldAgent` 提供异步协作审查。
> **v0.25.0 重大变更**：Context Rot 显式防御 + 四级错误分类与恢复。`ContextPrioritizer` 按 Critical/High/Normal/Background 排序系统提示词并双重锚定关键约束；`ErrorSeverity` 把错误分为 Fatal/Retry/Degraded/UserAction，后端支持指数退避重试与降级回退，前端 `AgentInterruptionModal` 显式中断 Fatal/UserAction 错误。
> **v0.23.74 重大变更**：场景优先架构迁移完成——`scenes.content` 为唯一叙事真相源，`chapters.content` 为只读聚合投影。创世提示词场景化，幕前编辑器纯正文无缝拼接。

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
│                        StoryForge (草苔) v0.23.13                          │
│              Tauri 2.4 + React 18 + TypeScript 5.8 + Vite 6              │
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
│  │                      Backend (Rust) - v0.23.13 Core                │   │
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
│  │  │  • Scenes*  │  │  • 实体向量  │  │  • 工作室配置           │ │   │
│  │  │  • Chapters │  │  • 语义搜索  │  │                         │ │   │
│  │  │  • Characters│ │             │  │  *Scene 为内容真相源     │ │   │
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

### 🛡️ 上下文健康与错误恢复 (v0.25.0)

v0.25.0 在原有智能创作链路之上增加了两道防御性基础设施：

#### 1. Context Rot 显式防御

长系统提示词容易出现 "Lost in the Middle"——模型对中间段落的约束记忆衰减。`ContextPrioritizer` 把系统提示词拆分为带优先级的 `ContextChunk`：

- **Critical**：合同红线、在世作者保护、反 AI 陈词滥调等绝对不能违反的约束
- **High**：角色核心、伏笔、四元组等强信号上下文
- **Normal**：写作风格、方法论、世界设定等常规上下文
- **Background**：历史参考、弱关联记忆等辅助信息

排序后 Critical 信息同时出现在提示词开头和结尾的轻量摘要中，实现双重锚定。`ContextHealthMetrics` 记录各类别的 token 数与预算使用率，供诊断卡片实时查看。

#### 2. 四级错误分类与恢复

后端错误统一注入 `ErrorSeverity`：

| Severity       | 含义         | 典型场景              | 恢复策略                                    |
| -------------- | ------------ | --------------------- | ------------------------------------------- |
| **Fatal**      | 无法自动恢复 | 未知内部 panic        | 记录并终止，前端弹 `AgentInterruptionModal` |
| **Retry**      | 瞬态错误     | 模型连接超时、DB 锁定 | 指数退避重试，或切换候选模型                |
| **Degraded**   | 可降级继续   | 上下文缺失            | 执行简化路径并提示用户                      |
| **UserAction** | 需要用户处理 | 订阅、校验、模型禁用  | 前端弹 `AgentInterruptionModal` 引导设置    |

`error_recovery.rs` 提供 `retry_with_backoff` 与 `with_degraded_fallback`；`GatewayExecutor` 与 `smart_execute` 的 DB 加载已接入重试。前端 `errorHandler.ts` 按 `severity` 兜底推荐动作，`AgentInterruptionModal` 对 Fatal/UserAction 直接中断当前流程。

### 🏗️ 分层架构 (v0.23.6)

v0.23.6 在 v0.19.0 分层基础上进一步完成 **全局单例清零**（14 个 `static`/缓存改为 Tauri State 注入）与 **模块循环依赖斩断**，并新增 **PromptSynthesis 提示词合成层** 支撑 TriShot 三击生成管线。当前核心调用链为：

```
Frontend (React)
      │ invoke / listen
      ▼
Tauri Command Layer (commands/*.rs)   ← 薄层：参数校验 + EmitSync
      │
      ├─ DTO (db/dto.rs)              ← 请求/响应序列化对象
      │
      ├─ Domain Service                 ← 业务编排（story_system/*_service.rs）
      │
      ▼
Repository Layer (db/repositories*.rs) ← 数据访问
      │
      ▼
SQLite / LanceDB / File System
```

#### Command 层 (`src-tauri/src/commands/`)

- 按领域拆分为 20 个文件：`story.rs`、`chapter.rs`、`scene_commands.rs`、`character.rs`、`memory.rs`、`story_system.rs`、`pipeline.rs` 等
- 统一使用 `State<'_, DbPool>` 注入，返回 `Result<T, AppError>`
- 通过 `commands/utils.rs` 的 `EmitSync` trait 在变更后发射 `sync-event`

#### DTO 层 (`src-tauri/src/db/dto.rs`)

- v0.9.0 新增：将 18+ 个请求/响应结构体从 `models.rs` 迁出
- 包括：`CreateSceneRequest`、`UpdateSceneRequest`、`CreateStoryRequest`、`UpdateStoryRequest`、`CreateCharacterRequest`、`CreateChapterRequest`、`CreateAiOperationRequest`、`StudioExportRequest` 等
- 原则：贫血模型（`models.rs`）只保留数据库实体，DTO 只承担序列化/反序列化职责

#### 领域服务层 (`src-tauri/src/story_system/`)

- `chapter_service.rs`：章节变更后的伏笔检测、自动化触发（v0.23.74: `ChapterCommitDebouncer` 已由 `SceneCommitDebouncer` 接替）
- `scene_service.rs`：场景内容变更后的 KG Ingest、向量索引、world_building 刷新、**SceneCommitDebouncer**（30s 防抖 auto_commit）
- `commit_service.rs`：`SceneCommitService` — 场景级 commit，驱动 5 个 Projection Writer
- `mod.rs`：`StorySystemEngine`、ContractTree / RuntimeContract

#### Repository 层 (`src-tauri/src/db/repositories*.rs`)

- `repositories.rs`：Story / Scene / Chapter / Character 通用仓库
- `repositories_story_system.rs`：StoryContract / SceneCommit / MemoryItem / ReadingPower 等专用仓库
- `repositories_narrative.rs` / `repositories_pipeline.rs` / `repositories_export.rs`：各垂直域仓库

#### PromptRegistry 层 (`src-tauri/src/prompts/`)

- v0.19.0 新增：统一 LLM 提示词注册表，所有硬编码提示词提取为可配置项
- `registry.rs`：35+ 内置 prompt，15 个 `PromptCategory` 分类，支持 `resolve_prompt()` 运行时读取覆盖
- `commands.rs`：IPC 命令（`list_prompt_entries`、`save_prompt_override`、`reset_prompt_override`、`reset_all_prompt_overrides`、`resolve_prompt_content`）
- 消费者：Writer/Inspector/Commentator/Planner/Analyzer/Probe/Memory/Knowledge/Skill/Methodology 等全部模块

---

### 🗄️ 数据库与迁移框架 (v0.19.0)

#### 数据层组件

| 组件                        | 路径                             | 职责                                             |
| --------------------------- | -------------------------------- | ------------------------------------------------ |
| `connection.rs`             | `src-tauri/src/db/connection.rs` | `DbPool`、初始 Schema、`init_db()`、遗留内联迁移 |
| `migrations.rs`             | `src-tauri/src/db/migrations.rs` | 自定义 `MigrationRunner`，扫描 `.sql` 文件       |
| `migrations/V007~V027*.sql` | `src-tauri/src/db/migrations/`   | 21 个版本化 SQL 迁移                             |
| `dto.rs`                    | `src-tauri/src/db/dto.rs`        | 请求/响应 DTO                                    |
| `models.rs`                 | `src-tauri/src/db/models.rs`     | 数据库实体模型                                   |
| `traits.rs`                 | `src-tauri/src/db/traits.rs`     | 仓库 trait 抽象                                  |

#### MigrationRunner 设计

- 因项目使用 rusqlite 0.39，**自定义实现** `MigrationRunner`（未使用 refinery 默认 rusqlite 特性）
- 扫描 `V{version}__{description}.sql` 文件名，按版本排序
- 通过 `schema_migrations` 表追踪已应用版本
- 每个迁移在独立事务中执行；自动忽略 SQL 中的 `BEGIN/COMMIT/ROLLBACK`
- 对 `duplicate column name` / `already exists` 等错误幂等跳过
- `run_with_legacy()` 先跑 SQL 迁移，再跑遗留 Rust inline 迁移，保证平滑升级

---

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

| 特性     | 章节 (Chapter) | 场景 (Scene)    |
| -------- | -------------- | --------------- |
| 驱动方式 | 时间/长度驱动  | 戏剧冲突驱动    |
| 结构     | 线性序列       | 网络化关联      |
| AI 理解  | 文本内容       | 戏剧目标 + 冲突 |
| reorder  | 简单排序       | 依赖关系维护    |

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
│  SCENE_COMMIT (写后真源)                 │
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

**领域服务下沉 (v0.9.0)**

- `story_system/chapter_service.rs`：`ChapterService` 统一处理 `on_chapter_updated` / `on_chapter_created`
  - `ChapterCommitDebouncer`：30 秒 debounce 后调用 `SceneCommitService::auto_commit`
  - `PayoffDetector`：检测逾期伏笔并发射 `PayoffOverdue` 事件
  - `AutomationTrigger`：触发 `ChapterContentUpdated` / `ChapterCreated` 自动化事件
- `story_system/scene_service.rs`：`SceneService` 统一处理 `on_scene_updated` / `on_scene_created` / `on_scene_deleted`
  - `SceneIngestor`：后台 KG Ingest + 向量索引更新
  - `SceneAutomationTrigger`：触发 `SceneContentUpdated` / `SceneCreated`
- 原 `commands/chapter.rs` 与 `scene_commands.rs` 只保留薄封装：参数校验 → 调用 Service → 发射同步事件

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

### ⏱️ 分时介入架构 (v0.13.0)

**解决的核心矛盾**：AI 长篇小说创作中"质量与速度不可兼得"——强化专业资产介入导致生成过慢，放松则质量低劣。

**根因诊断（B + E）**：

- **B（资产被错误同步化）**：合同、伏笔、Inspector、记忆各有最佳发力时机，却全压在"Writer 一次 LLM 调用"那个点上
- **E（写与审被错误耦合）**：写（快）和审（深）被焊死在一条同步链路，用户全程干等

**第一性原理**：把大灾难变成即时可见的小债务。蚂蚁搬家，不积巨石。

**三条时间线**：

```
┌─────────────────────────────────────────────────────────────┐
│  时间线 1：写作时刻（热路径，< 15s，用户等待）                │
│  QuickPreflightChecker → WriteTimeBundle（红线突出+题材自适应）│
│  → generate_for_task 直连 LLM → 立即返回正文                  │
│  代码：execute_time_sliced (agents/orchestrator.rs)          │
└─────────────────────────────────────────────────────────────┘
          │ 正文已返回，spawn 后台（不阻塞）
          ▼
┌─────────────────────────────────────────────────────────────┐
│  时间线 2：审计时刻（温路径，30-90s，后台异步）                │
│  AuditExecutor → Inspector 7 维审计（memory 优先）            │
│  → create_annotation_with_meta（type=ai_audit）              │
│  → emit SyncEvent::AnnotationCreated → 前端自动渲染标注       │
│  代码：task_system/audit_executor.rs                          │
└─────────────────────────────────────────────────────────────┘
          │ 每 5 段条件触发
          ▼
┌─────────────────────────────────────────────────────────────┐
│  时间线 3：洞察时刻（冷路径，分钟级，跨章节深度）              │
│  InsightExecutor → 追读力趋势 + 债务汇总 + annotation 盘点    │
│  → 整体健康度评分 → story_summaries → NarrativeAnalysis 页    │
│  代码：task_system/insight_executor.rs                        │
└─────────────────────────────────────────────────────────────┘
```

**GenerationMode 三值**：

- `Fast`：Ghost Text 等实时补全（原有，不变）
- `TimeSliced`：**默认**，走三时间线（普通生成/auto_write/auto_revise）
- `Full`：同步审计+Rewrite 闭环（向导/Genesis/Planner/Workflow）

**Phase 0 实测验证**（qwen3.6-35b，3 场景 A/B 盲测）：

- 最小约束 vs 全量资产平均质量差距 **7.9%**（< 30% 阈值）→ 架构成立
- prompt 长 160% 仅耗时多 7% → 证实"慢在同步链路而非 Writer 本身"
- 三条实证改进：红线突出注入、题材自适应 bundle、memory 维度优先审计

**前端可见物**：

- 顶栏 **DebtIndicator**（债务指示器）：未处理 annotation 计数，超阈值红色警告
- 编辑器内 **TextAnnotationMark**：ai_audit 类型按 severity 动态着色（high=红/medium=琥珀/low=蓝）
- 叙事分析页 **深度洞察 section**：健康度仪表盘 + 追读力趋势柱状图

**设计文档**：[`docs/plans/2026-06-14-time-sliced-intervention-design.md`](./docs/plans/2026-06-14-time-sliced-intervention-design.md)

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

- 解析 `scene_commits.projection_status_json`
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
StoryForge/
├── src-frontend/                    # 前端代码 (React 18 + TypeScript 5.8 + Vite 6)
│   ├── src/
│   │   ├── main.tsx                # 幕后入口
│   │   ├── App.tsx                 # 幕后主应用：路由 + 全局事件监听
│   │   │
│   │   ├── frontstage/             # 幕前界面
│   │   │   ├── main.tsx            # 幕前入口
│   │   │   ├── FrontstageApp.tsx
│   │   │   ├── components/
│   │   │   │   ├── ReaderWriter.tsx
│   │   │   │   ├── RichTextEditor.tsx
│   │   │   │   ├── CharacterPeekCard.tsx
│   │   │   │   ├── IngestHealthIndicator.tsx
│   │   │   │   └── ...
│   │   │   └── styles/
│   │   │
│   │   ├── pages/                  # 幕后页面
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Stories.tsx
│   │   │   ├── Characters.tsx
│   │   │   ├── Scenes.tsx
│   │   │   ├── WorldBuilding.tsx
│   │   │   ├── KnowledgeGraph.tsx
│   │   │   ├── Skills.tsx
│   │   │   ├── Mcp.tsx
│   │   │   ├── BookDeconstruction.tsx
│   │   │   ├── Tasks.tsx
│   │   │   ├── Foreshadowing.tsx
│   │   │   ├── NarrativeAnalysis.tsx
│   │   │   ├── StorySystem.tsx
│   │   │   ├── UsageStats.tsx
│   │   │   ├── WritingStats.tsx
│   │   │   └── Settings.tsx
│   │   │
│   │   ├── components/             # 共享组件
│   │   │   ├── Sidebar.tsx
│   │   │   ├── StoryTimeline.tsx
│   │   │   ├── SceneEditor.tsx
│   │   │   ├── DataLoader.tsx
│   │   │   ├── ErrorBoundary.tsx
│   │   │   └── ...
│   │   │
│   │   ├── services/               # IPC API 层 (v0.9.0 拆分)
│   │   │   ├── tauri.ts           # 兼容入口：barrel re-export
│   │   │   └── api/
│   │   │       ├── index.ts       # barrel export
│   │   │       ├── core.ts        # loggedInvoke
│   │   │       ├── stories.ts
│   │   │       ├── storySystem.ts
│   │   │       ├── skills.ts
│   │   │       ├── settings.ts
│   │   │       ├── intent.ts
│   │   │       ├── annotations.ts
│   │   │       ├── knowledge.ts
│   │   │       ├── memory.ts
│   │   │       ├── pipeline.ts
│   │   │       ├── quality.ts
│   │   │       ├── genesis.ts
│   │   │       ├── stream.ts
│   │   │       ├── subscription.ts
│   │   │       ├── writing.ts
│   │   │       └── wizard.ts
│   │   │
│   │   ├── hooks/                  # 自定义 Hooks
│   │   │   ├── useSyncStore.ts    # 统一 sync-event 监听
│   │   │   ├── useScenes.ts
│   │   │   ├── useWorldBuilding.ts
│   │   │   ├── useWorkflowNodes.ts
│   │   │   ├── useUpdater.ts
│   │   │   └── ...
│   │   │
│   │   ├── stores/                 # Zustand 全局状态
│   │   │   └── appStore.ts
│   │   │
│   │   ├── generated/              # ts-rs 自动生成类型
│   │   │   └── SyncEvent.ts
│   │   │
│   │   ├── types/                  # 前端类型定义
│   │   │   └── index.ts
│   │   │
│   │   └── utils/                  # 工具函数
│   │       └── logger.ts
│   │
│   ├── index.html
│   ├── frontstage.html
│   └── package.json
│
├── src-tauri/                       # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs                 # 可执行入口
│   │   ├── lib.rs                  # crate 根：模块声明 + 全局单例 + run()
│   │   ├── handlers.rs             # generate_handler![] 宏命令注册表
│   │   │
│   │   ├── commands/               # Tauri Command 层（按领域拆分，v0.7.9+）
│   │   │   ├── mod.rs
│   │   │   ├── utils.rs           # EmitSync trait
│   │   │   ├── core.rs
│   │   │   ├── story.rs
│   │   │   ├── chapter.rs
│   │   │   ├── character.rs
│   │   │   ├── story_system.rs
│   │   │   ├── memory.rs
│   │   │   ├── pipeline.rs
│   │   │   ├── skill.rs
│   │   │   ├── mcp.rs
│   │   │   ├── intent.rs
│   │   │   ├── export.rs
│   │   │   ├── anti_ai.rs
│   │   │   ├── audit.rs
│   │   │   ├── reading_power.rs
│   │   │   ├── vector.rs
│   │   │   ├── workflow.rs
│   │   │   ├── sync.rs
│   │   │   ├── ai_op.rs
│   │   │   └── genre.rs
│   │   │
│   │   ├── scene_commands.rs       # 场景命令（v0.7.9 拆分，顶层保留）
│   │   ├── creation_commands.rs    # 创世/创作命令（v0.7.9 拆分）
│   │   ├── revision_commands.rs    # 修订命令（v0.7.9 拆分）
│   │   └── studio_commands.rs      # 工作室命令（v0.7.9 拆分）
│   │   │
│   │   ├── db/                     # 数据层 (v0.9.0 DTO + MigrationRunner)
│   │   │   ├── mod.rs
│   │   │   ├── connection.rs      # DbPool、init_db、遗留内联迁移
│   │   │   ├── migrations.rs      # 自定义 MigrationRunner
│   │   │   ├── migrations/        # V007 ~ V027 .sql 文件
│   │   │   ├── models.rs          # 数据库实体
│   │   │   ├── dto.rs             # 🆕 请求/响应 DTO (v0.9.0)
│   │   │   ├── repositories.rs
│   │   │   ├── repositories_story_system.rs
│   │   │   ├── repositories_narrative.rs
│   │   │   ├── repositories_pipeline.rs
│   │   │   ├── repositories_export.rs
│   │   │   ├── traits.rs
│   │   │   ├── repositories_tests.rs
│   │   │   └── cascade_tests.rs
│   │   │
│   │   ├── story_system/           # 合同驱动故事系统
│   │   │   ├── mod.rs             # StorySystemEngine / SceneCommitService
│   │   │   ├── chapter_service.rs # 🆕 章节领域服务 (v0.9.0)
│   │   │   ├── scene_service.rs   # 🆕 场景领域服务 (v0.9.0)
│   │   │   ├── auto_contract.rs
│   │   │   ├── contract_builder.rs
│   │   │   ├── preflight.rs
│   │   │   └── projection_writers.rs
│   │   │
│   │   ├── state_sync/             # 前后端状态同步
│   │   │   ├── mod.rs
│   │   │   ├── events.rs          # SyncEvent (ts-rs)
│   │   │   └── service.rs
│   │   │
│   │   ├── agents/                 # Agent 系统
│   │   ├── memory/                 # 四层记忆系统
│   │   ├── pipeline/               # Pipeline 审校
│   │   ├── book_deconstruction/    # 拆书
│   │   ├── task_system/            # 任务调度
│   │   ├── creative_engine/        # 创意引擎 / 风格 / 连续性
│   │   ├── narrative/              # 叙事元素与管线
│   │   ├── llm/                    # LLM 适配器
│   │   ├── vector/                 # LanceDB 向量存储
│   │   ├── embeddings/             # Embedding 服务
│   │   ├── knowledge_base/         # 知识库
│   │   ├── skills/                 # 技能系统
│   │   ├── mcp/                    # MCP 工具
│   │   ├── automation/             # 自动化事件
│   │   ├── workflow/               # 工作流引擎
│   │   ├── planner/                # 计划生成与执行
│   │   ├── config/                 # 配置管理
│   │   ├── export/                 # 导出
│   │   ├── updater/                # 自动更新
│   │   ├── telemetry/              # 使用统计
│   │   ├── versions/               # 版本管理
│   │   ├── anti_ai/                # Anti-AI 审查
│   │   ├── reading_power/          # 追读力评估
│   │   ├── canonical_state/        # 规范状态
│   │   ├── utils/                  # 工具
│   │   └── tests/                  # 集成测试
│   │
│   └── Cargo.toml
│
├── e2e/                             # Playwright E2E 测试
├── docs/                            # 文档
│   ├── USER_GUIDE.md
│   ├── product-screenshots/
│   └── ...
├── scripts/                         # 工具脚本
└── README.md
```

---

## 数据流

### 场景创建流程 (v0.9.0 分层)

```
用户点击"新建场景" → StoryTimeline
→ invoke('create_scene', { story_id, sequence_number, title })
→ commands/scene_commands.rs 参数校验
→ db::CreateSceneRequest (dto.rs) 反序列化
→ SceneRepository::create()
→ SQLite
→ StateSync::emit_scene_created() 发射 sync-event
→ 前端 useSyncStore 失效 ['scenes'] 查询
→ StoryTimeline 自动刷新列表
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

### SCENE_COMMIT 投影流程 (v6.0.0 → v0.9.0)

```
场景保存
→ scene_service.rs / chapter_service.rs 领域编排
  → SceneCommitService::auto_commit()
    → StateProjectionWriter     → memory_items (category="state")
    → IndexProjectionWriter     → memory_items (category="entity")
    → SummaryProjectionWriter   → story_summaries
    → MemoryProjectionWriter    → memory_items (category="event")
    → VectorProjectionWriter    → LanceDB VectorRecord
  → projection_status_json 记录各 Writer 状态
→ 发射 sync-event: DataRefresh + IngestionCompleted
```

> **v0.7.3 变更**：`ChapterCommitService` 重命名为 `SceneCommitService`，`chapter_commits` 表重命名为 `scene_commits`（Migration 70），提交粒度从 Chapter 对齐到 Scene。  
> **v0.9.0 变更**：`SceneCommitService::auto_commit` 的调用从 Command 层下沉到 `story_system/chapter_service.rs` 的 `ChapterCommitDebouncer`，触发逻辑与 HTTP/IPC 层解耦。

---

## 前端 IPC 与状态同步 (v0.9.0)

### API 服务层 (`src-frontend/src/services/api/`)

- v0.9.0 将原 `services/tauri.ts`（1,340 行上帝文件）拆分为 17 个按域子模块
- `core.ts` 仅保留 `loggedInvoke<T>`：参数脱敏 + 耗时日志 + 统一错误抛出
- 历史导入 `import { ... } from '@/services/tauri'` 仍通过 3 行 barrel 兼容
- 新增 `services/api/index.ts` barrel export，未来新模块推荐 `import { ... } from '@/services/api'`

### TanStack Query + Zustand 协作

- `stores/appStore.ts`（Zustand）：保存 `currentStory`、`stories[]`、UI 状态
- `hooks/useSyncStore.ts`：监听 Rust 发射的 `sync-event`，根据事件类型精确失效 TanStack Query 缓存
- `App.tsx`：`currentStory` 变化时批量 `cancelQueries` + `invalidateQueries`，刷新关联数据

### 前后台通信

- Rust `state_sync/events.rs` 定义 `SyncEvent`（`#[derive(TS)]` 自动生成到 `src-frontend/src/generated/SyncEvent.ts`）
- 16+ 种事件覆盖：StoryCreated / StoryDeleted / CharacterUpdated / SceneCreated / ChapterUpdated / WorldBuildingUpdated / StyleDnaUpdated / TaskUpdated / AnnotationCreated / PayoffLedgerUpdated / DataRefresh 等
- 幕前/幕后通过 Tauri 事件（`backstage-update`、`backstage-shown`）联动，逐步替代旧的 DOM CustomEvent

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
# 前端开发服务器（推荐单独启动）
cd src-frontend && npm run dev
# 默认监听 http://127.0.0.1:5173/

# Tauri 开发模式（会自动启动前端并打开桌面窗口）
cd src-tauri && cargo tauri dev

# 生产构建
cd src-tauri && cargo tauri build
```

### 测试

```bash
# Rust 单元测试
cd src-tauri && cargo test

# 前端类型检查
cd src-frontend && npx tsc --noEmit

# Playwright E2E
npm test
```

### 数据库迁移

- 启动时 `init_db()` 自动调用 `MigrationRunner::run_with_legacy()`
- SQL 迁移文件位置：`src-tauri/src/db/migrations/V###__*.sql`
- 开发环境如需重置，可删除应用数据目录下的 SQLite 文件后重新启动

---

## 相关文档

- [V3 架构计划](docs/plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计文档
- [功能清单](docs/FEATURES.md) - 完整功能列表
- [更新日志](CHANGELOG.md) - 版本变更记录
