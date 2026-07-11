# 拆书功能设计方案

> 日期: 2026-04-19
> 功能: 拆书功能 (Book Deconstruction)
> 位置: 幕后界面 (Backstage)

---

## 1. 功能概述

在幕后界面新增「拆书」独立页面。用户上传小说文件（txt/pdf/epub），系统后端解析文本、调用 LLM 进行深度分析，最终输出结构化拆解结果，保存为「参考素材库」中的条目，并可一键转为 StoryMoss 故事项目供创作使用。

## 2. 数据模型

### 2.1 参考小说主表 `reference_books`

```rust
pub struct ReferenceBook {
    pub id: String,              // 唯一ID (UUID)
    pub title: String,
    pub author: Option<String>,
    pub genre: String,           // 小说类型
    pub word_count: i64,
    pub file_format: String,     // txt/pdf/epub
    pub file_hash: String,       // 去重校验 (SHA256)
    pub file_path: Option<String>, // 原始文件存储路径
    
    // 拆书结果
    pub world_setting: String,   // 世界观设定 (JSON)
    pub plot_summary: String,    // 故事主线概要
    pub story_arc: String,       // 故事线 (JSON 结构化)
    pub analysis_status: AnalysisStatus, // 分析状态
    
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

pub enum AnalysisStatus {
    Pending,      // 等待分析
    Extracting,   // 文本提取中
    Analyzing,    // LLM 分析中
    Completed,    // 完成
    Failed,       // 失败
}
```

### 2.2 人物表 `reference_characters`

```rust
pub struct ReferenceCharacter {
    pub id: String,
    pub book_id: String,
    pub name: String,
    pub role_type: String,       // 主角 / 反派 / 配角 / 龙套
    pub personality: String,     // 性格分析
    pub appearance: Option<String>, // 外貌描写
    pub relationships: String,   // 人物关系 JSON
    pub key_scenes: Vec<String>, // 关键场景ID列表
    pub importance_score: f32,   // 重要度 0.0-1.0
}
```

### 2.3 章节/场景概要表 `reference_scenes`

```rust
pub struct ReferenceScene {
    pub id: String,
    pub book_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub summary: String,         // AI 生成的章节概要
    pub characters_present: Vec<String>, // 出场人物
    pub key_events: Vec<String>, // 关键事件
    pub conflict_type: Option<String>,
    pub emotional_tone: Option<String>, // 情感基调
}
```

### 2.4 文本块表 `reference_chunks`

用于分块分析和向量化存储：

```rust
pub struct ReferenceChunk {
    pub id: String,
    pub book_id: String,
    pub chunk_index: i32,        // 块序号
    pub content: String,         // 原始文本块
    pub word_count: i32,
    pub chunk_type: ChunkType,   // 章节/采样/摘要
}

pub enum ChunkType {
    FullChapter,   // 完整章节
    SampledBlock,  // 采样块（长篇）
    ExtractedText, // 纯提取文本
}
```

## 3. 分层分析策略

采用 **智能分块 + 增量归纳** 策略：

| 小说长度 | 策略 | LLM 调用方式 |
|----------|------|-------------|
| 短篇 (<10万字) | 全文一次性分析 | 单轮 comprehensive analysis |
| 中篇 (10-50万字) | 按章节分块 → 逐块分析 → 汇总 | 多轮 per-chunk + final synthesis |
| 长篇 (>50万字) | 均匀分块 + 关键章节采样 → 汇总 | 采样 + 分块 + synthesis |

### 3.1 分析 Pipeline

```
文本提取 → 章节结构识别 → 元信息识别 → 人物拆解 → 章节概要 → 故事线生成 → 向量化存储
```

**Step 1: 文本提取**
- txt: 直接读取，编码自动检测 (UTF-8/GBK)
- pdf: `pdf-extract` crate 提取纯文本
- epub: `epub` crate 解析章节结构

**Step 2: 章节结构识别**
- 正则匹配章节标题（第X章 / 第X回 / Chapter X）
- 若无明确章节，按 ~5000 字固定分块

**Step 3: 元信息识别**
- LLM prompt: "分析以下小说开头，提取标题、作者、类型、总字数估计"

**Step 4: 人物拆解（分块并行）**
- 每块 LLM prompt: "提取本章节出现的所有人物，分析其姓名、角色定位、性格特征、与其他人物的关系"
- 汇总轮: "基于以下各章节人物分析，生成完整的人物关系网络"

**Step 5: 章节概要（逐块）**
- 每块 LLM prompt: "总结本章内容概要、出场人物、关键事件、情感基调"

**Step 6: 故事线生成**
- LLM prompt: "基于以下章节概要，生成完整的故事主线、支线、高潮点、结局"

**Step 7: 向量化存储**
- 每个章节概要、人物分析、世界观段落 → embedding → VectorStore

## 4. 向量存储策略

复用现有 `VectorStore` / `LanceVectorStore`，新增 `RecordType::BookAnalysis`：

```rust
pub enum RecordType {
    ChapterSummary,
    KeyEvent,
    CharacterTrait,
    BookAnalysis,    // 新增
}
```

每条记录关联 `book_id` 和 `chunk_id`，供写作 Agent 的 `QueryPipeline` 检索学习。

## 5. 前端交互设计

### 5.1 页面结构

```
幕后 Sidebar → 点击「拆书」→ 进入 /book-deconstruction 页面
  ├── 左侧：已拆书籍列表（卡片网格 / 搜索过滤）
  └── 右侧：
      ├── 空状态：上传引导区（拖放 + 按钮）
      └── 详情态：拆书结果展示（标签页切换）
```

### 5.2 上传流程

```
点击「上传新书」→ 文件选择 / 拖放
  → 前端校验格式 (txt/pdf/epub) + 大小 (<100MB)
  → 上传至后端临时目录
  → 后端返回 book_id
  → 前端轮询 analysis_status
  → 显示分析进度（步骤指示器）
      Step 1/5: 文本提取
      Step 2/5: 结构识别
      Step 3/5: 人物拆解
      Step 4/5: 章节概要
      Step 5/5: 故事线生成
  → 完成后跳转详情页
```

### 5.3 详情页标签页

| 标签 | 内容 |
|------|------|
| 概览 | 封面、标题、作者、类型、字数、世界观摘要 |
| 人物 | 人物列表卡片 + 关系网络图 (ReactFlow) |
| 章节 | 章节大纲列表（可展开查看概要） |
| 故事线 | 时间线可视化（主线/支线/高潮点） |
| 原始文本 | 文本预览（只读） |

### 5.4 操作按钮

- **保存到素材库**: 确认保存（默认自动保存）
- **一键转为故事**: 调用 `NovelCreationAgent` 生成 StoryMoss 故事项目
- **重新分析**: 使用相同文件重新跑分析
- **删除**: 从素材库移除（同时清理向量记录）

## 6. 后端模块设计

```
src-tauri/src/
├── book_deconstruction/        # 新增拆书核心模块
│   ├── mod.rs                  # 模块导出
│   ├── parser.rs               # 文件解析器
│   │   ├── txt_parser
│   │   ├── pdf_parser
│   │   └── epub_parser
│   ├── analyzer.rs             # LLM 分析 orchestrator
│   │   ├── analyze_metadata
│   │   ├── analyze_characters
│   │   ├── analyze_chapters
│   │   └── synthesize_story_arc
│   ├── chunker.rs              # 文本分块策略
│   │   ├── split_by_chapters
│   │   └── split_by_size
│   ├── models.rs               # 数据模型
│   ├── repository.rs           # 数据库存取
│   ├── service.rs              # 业务逻辑层
│   └── commands.rs             # Tauri IPC 命令
```

### 6.1 文件解析器

```rust
pub trait BookParser {
    fn parse(&self, file_path: &Path) -> Result<ParsedBook, ParseError>;
}

pub struct ParsedBook {
    pub title: Option<String>,       // 从元数据提取
    pub author: Option<String>,
    pub chapters: Vec<Chapter>,
    pub raw_text: String,
    pub word_count: usize,
}

pub struct Chapter {
    pub title: Option<String>,
    pub content: String,
    pub word_count: usize,
}
```

### 6.2 Tauri IPC 命令

```rust
// 上传文件并开始分析
#[tauri::command]
async fn upload_book(
    file_path: String,
    app_handle: AppHandle,
) -> Result<String, String>; // 返回 book_id

// 查询分析状态
#[tauri::command]
async fn get_analysis_status(
    book_id: String,
) -> Result<AnalysisStatus, String>;

// 获取拆书结果
#[tauri::command]
async fn get_book_analysis(
    book_id: String,
) -> Result<BookAnalysisResult, String>;

// 获取已拆书籍列表
#[tauri::command]
async fn list_reference_books(
) -> Result<Vec<ReferenceBookSummary>, String>;

// 删除参考书籍
#[tauri::command]
async fn delete_reference_book(
    book_id: String,
) -> Result<(), String>;

// 一键转为故事项目
#[tauri::command]
async fn convert_to_story(
    book_id: String,
) -> Result<String, String>; // 返回 story_id
```

## 7. 前端模块设计

```
src-frontend/src/
├── pages/
│   └── BookDeconstruction.tsx          # 拆书主页面
├── components/book-deconstruction/
│   ├── BookUploadPanel.tsx             # 上传面板（拖放区）
│   ├── BookListGrid.tsx                # 书籍列表网格
│   ├── BookDetailView.tsx              # 详情容器（标签页）
│   ├── BookOverviewTab.tsx             # 概览标签
│   ├── CharacterNetworkTab.tsx         # 人物关系网 (ReactFlow)
│   ├── ChapterOutlineTab.tsx           # 章节大纲
│   ├── StoryArcTab.tsx                 # 故事线可视化
│   ├── AnalysisProgress.tsx            # 分析进度指示器
│   └── BookSearchFilter.tsx            # 搜索过滤
├── hooks/
│   ├── useBookUpload.ts                # 上传 + 轮询状态
│   ├── useBookAnalysis.ts              # 获取分析结果
│   ├── useReferenceBooks.ts            # 列表管理
│   └── useConvertToStory.ts            # 转为故事
└── types/
    └── book-deconstruction.ts          # 类型定义
```

## 8. 系统集成点

| 系统 | 集成方式 |
|------|----------|
| LLM Adapter | 复用 `src-tauri/src/llm/` 现有适配器，通过 `LlmService` 调用 |
| 向量存储 | 复用 `src-tauri/src/vector/` `LanceVectorStore`，新增 `BookAnalysis` 记录类型 |
| 数据库 | 新增表通过 `src-tauri/src/db/` 迁移创建，复用 `rusqlite` + `r2d2` |
| Agent 学习 | 写作 Agent 的 `QueryPipeline` 自动覆盖 `BookAnalysis` 向量记录 |
| 一键转故事 | 调用现有 `NovelCreationAgent` 或 `StoryRepository` 创建新项目 |
| 前端路由 | 新增 `/book-deconstruction` 路由，在 Sidebar 添加入口 |

## 9. 错误处理与边界

| 场景 | 处理方式 |
|------|----------|
| 文件 >100MB | 前端拦截，提示"文件过大，请压缩后上传" |
| 格式不支持 | 前端校验，仅允许 .txt/.pdf/.epub |
| PDF 扫描件/加密 | `pdf-extract` 返回空文本 → 标记 `AnalysisStatus::Failed`，提示"无法提取文本" |
| LLM 超时/失败 | 分块任务可重试 3 次，支持断点续拆（记录已完成步骤） |
| 重复上传 | `file_hash` 去重，提示"该书籍已存在，是否重新分析？" |
| 编码问题 | txt 自动检测 UTF-8/GBK/GB2312，失败时尝试其他编码 |
| 超长小说内存 | 流式读取大文件，避免一次性载入内存 |

## 10. 新增依赖

### Rust (src-tauri/Cargo.toml)
```toml
# PDF 解析
pdf-extract = "0.7"

# EPUB 解析
epub = "2.1"

# 编码检测（txt）
encoding = "0.2"

# 文件哈希
sha2 = "0.10"
```

### TypeScript (src-frontend/package.json)
```json
// 无需新增主要依赖，复用现有 ReactFlow、Tailwind、Zustand
```

## 11. 性能考虑

- **大文件处理**: 流式读取，逐章节处理，避免内存溢出
- **LLM 调用**: 分块任务可并行（使用 `tokio::spawn`），但需控制并发数（默认 3 个并发）
- **向量存储**: 批量写入 embedding，每批 100 条
- **前端**: 长章节列表使用虚拟滚动，人物关系图按需渲染

## 12. 测试策略

- **单元测试**: `chunker.rs` 分块逻辑、`parser.rs` 各格式解析
- **集成测试**: 端到端上传→分析→查询流程
- ** fixtures**: 准备短篇/中篇/长篇测试文件各 1 个

---

*设计确认后进入实现阶段。*
