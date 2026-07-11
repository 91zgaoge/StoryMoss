# StoryMoss 修订模式与变更追踪架构计划

> **版本**: v3.3.0 规划
> **状态**: Phase 1/2/3 已完成，性能优化待续
> **目标**: 实现专业级修订模式（Track Changes）+ 评论批注线程 + 版本差异追踪

---

## 1. 现状分析

### 1.1 已有基础

| 模块 | 已有能力 | 复用价值 |
|------|---------|---------|
| `versions/` | SceneVersion 完整版本链、DiffViewer、版本恢复 | **核心基础** — 修订模式基于版本差异 |
| `collab/ot.rs` | TextOperation（insert/delete/retain）| **变更追踪底层** — OT 操作可直接转化为 track changes |
| `collab/websocket.rs` | CollabMessage、WebSocket 会话 | **实时协作通道** — 评论线程可复用消息通道 |
| `TextAnnotation` | 文本内联批注（from_pos/to_pos）| **评论锚点** — 扩展为带版本的线程评论 |
| `SceneAnnotation` | 场景级批注 | **场景评论** — 与修订模式评论合并 |

### 1.2 核心缺失

1. **变更粒度追踪**: 版本系统只能比较两个完整版本，无法追踪单次编辑中的增删改
2. **评论线程**: 现有批注是单条笔记，没有回复/解决/关联版本的能力
3. **TipTap 修订标记**: 编辑器缺少插入/删除标记的渲染与交互
4. **修订模式状态机**: 没有"开启修订模式"的状态管理

---

## 2. 架构设计

### 2.1 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│                    修订模式 (Revision Mode)                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│  │   变更追踪    │◄──►│   评论线程    │◄──►│   版本快照    │    │
│  │  (Track)    │    │  (Thread)   │    │  (Version)  │    │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘    │
│         │                   │                   │           │
│         └───────────────────┼───────────────────┘           │
│                             ▼                               │
│                  ┌─────────────────────┐                    │
│                  │    TipTap 编辑器层    │                    │
│                  │  • insert/delete    │                    │
│                  │  • comment mark     │                    │
│                  │  • suggestion mode  │                    │
│                  └─────────────────────┘                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 数据模型

#### `ChangeTrack` — 单条编辑操作记录
```rust
pub struct ChangeTrack {
    pub id: String,
    pub scene_id: String,           // 关联场景
    pub version_id: String,         // 所属版本（或生成该变更的版本）
    pub author_id: String,          // "user" | "ai" | 用户ID
    pub author_name: String,
    
    pub change_type: ChangeType,    // Insert | Delete | Format
    pub from_pos: i32,              // 文本起始位置
    pub to_pos: i32,                // 文本结束位置
    pub content: Option<String>,    // 插入/删除的文本内容
    
    pub status: ChangeStatus,       // Pending | Accepted | Rejected
    pub created_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

pub enum ChangeType {
    Insert,
    Delete,
    Format,
}

pub enum ChangeStatus {
    Pending,    // 待审核
    Accepted,   // 已接受
    Rejected,   // 已拒绝（恢复原文）
}
```

#### `CommentThread` — 评论线程
```rust
pub struct CommentThread {
    pub id: String,
    pub scene_id: String,
    pub version_id: String,         // 锚定版本（解决版本漂移）
    
    pub anchor_type: AnchorType,    // TextRange | SceneLevel
    pub from_pos: Option<i32>,
    pub to_pos: Option<i32>,
    pub selected_text: Option<String>, // 锚定时选中的原文
    
    pub status: ThreadStatus,       // Open | Resolved
    pub created_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

pub struct CommentMessage {
    pub id: String,
    pub thread_id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub created_at: DateTime<Local>,
}
```

### 2.3 与现有系统的关系

| 现有系统 | 关系 | 设计决策 |
|---------|------|---------|
| `SceneVersion` | 修订模式的"保存点" | 每次保存场景时，若处于修订模式，则同时生成 `ChangeTrack` 记录（通过版本 Diff） |
| `TextAnnotation` | 评论锚点的实现参考 | `CommentThread` 复用 `from_pos/to_pos` 定位逻辑，但增加版本锚定和线程回复 |
| `collab/ot.rs` | 变更追踪的底层操作来源 | 前端 TipTap 的每一步编辑可映射为 OT `TextOperation`，后端将其聚合为 `ChangeTrack` |
| `DiffViewer` | 接受/拒绝变更的展示组件 | 扩展为支持粒度到单条 `ChangeTrack` 的接受/拒绝 |

---

## 3. TipTap 编辑器层设计

### 3.1 新增 Mark Extensions

#### `trackInsert` — 插入标记
- 渲染: `span` + 蓝色下划线 + 淡蓝色背景
- 属性: `changeId`, `authorId`
- 交互: 悬停显示作者和接受/拒绝按钮

#### `trackDelete` — 删除标记
- 渲染: `span` + 红色删除线 + 红色背景
- 属性: `changeId`, `authorId`, `originalText`
- 交互: 悬停显示恢复按钮

#### `commentAnchor` — 评论锚点
- 渲染: `span` + 黄色高亮背景
- 属性: `threadId`
- 交互: 点击打开右侧评论线程面板

### 3.2 编辑器行为

```typescript
// 修订模式开启时
editor.setEditable(true, { trackChanges: true });

// 每次输入触发变更追踪
editor.on('transaction', ({ transaction }) => {
  if (!isRevisionMode) return;
  
  transaction.steps.forEach(step => {
    if (step instanceof ReplaceStep) {
      // 检测插入或删除
      const change = mapStepToChangeTrack(step, currentUser);
      sendChangeTrack(change);
    }
  });
});
```

---

## 4. 后端 API 设计

### 4.1 变更追踪命令

```rust
// 创建/更新变更记录（由编辑器前端在修订模式下实时推送）
#[command]
pub async fn track_change(
    scene_id: String,
    change_type: String,
    from_pos: i32,
    to_pos: i32,
    content: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<ChangeTrack, String>;

// 接受变更
#[command]
pub async fn accept_change(change_id: String, pool: State<'_, DbPool>) -> Result<(), String>;

// 拒绝变更
#[command]
pub async fn reject_change(change_id: String, pool: State<'_, DbPool>) -> Result<(), String>;

// 获取场景的待审变更列表
#[command]
pub async fn get_pending_changes(scene_id: String, pool: State<'_, DbPool>) -> Result<Vec<ChangeTrack>, String>;

// 一键接受/拒绝全部
#[command]
pub async fn accept_all_changes(scene_id: String, pool: State<'_, DbPool>) -> Result<usize, String>;
```

### 4.2 评论线程命令

```rust
// 创建评论线程
#[command]
pub async fn create_comment_thread(
    scene_id: String,
    version_id: String,
    from_pos: Option<i32>,
    to_pos: Option<i32>,
    selected_text: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<CommentThread, String>;

// 添加评论消息
#[command]
pub async fn add_comment(
    thread_id: String,
    content: String,
    pool: State<'_, DbPool>,
) -> Result<CommentMessage, String>;

// 获取场景的评论线程
#[command]
pub async fn get_scene_comment_threads(scene_id: String, pool: State<'_, DbPool>) -> Result<Vec<CommentThreadWithMessages>, String>;

// 解决/重开线程
#[command]
pub async fn resolve_comment_thread(thread_id: String, pool: State<'_, DbPool>) -> Result<(), String>;
```

---

## 5. 前端组件设计

### 5.1 新增组件

| 组件 | 路径 | 职责 |
|------|------|------|
| `TrackChangesToolbar` | `frontstage/components/TrackChangesToolbar.tsx` | 修订模式开关、接受/拒绝全部 |
| `CommentThreadPanel` | `frontstage/components/CommentThreadPanel.tsx` | 右侧评论线程列表 |
| `CommentBubble` | `frontstage/components/CommentBubble.tsx` | 编辑器内浮动添加评论按钮 |
| `ChangeTrackMark` | `frontstage/extensions/TrackChanges.ts` | TipTap Mark 扩展 |

### 5.2 状态管理

```typescript
// frontstage/stores/revisionStore.ts
interface RevisionState {
  isRevisionMode: boolean;
  pendingChanges: ChangeTrack[];
  commentThreads: CommentThread[];
  activeThreadId: string | null;
  
  toggleRevisionMode: () => void;
  acceptChange: (id: string) => void;
  rejectChange: (id: string) => void;
  setActiveThread: (id: string | null) => void;
}
```

---

## 6. 数据库 Schema 扩展

```sql
-- 变更追踪表
CREATE TABLE change_tracks (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL,
    version_id TEXT,
    author_id TEXT NOT NULL,
    author_name TEXT,
    change_type TEXT NOT NULL,
    from_pos INTEGER NOT NULL,
    to_pos INTEGER NOT NULL,
    content TEXT,
    status TEXT NOT NULL DEFAULT 'Pending',
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES scene_versions(id)
);

CREATE INDEX idx_change_tracks_scene ON change_tracks(scene_id);
CREATE INDEX idx_change_tracks_status ON change_tracks(status);

-- 评论线程表
CREATE TABLE comment_threads (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    anchor_type TEXT NOT NULL,
    from_pos INTEGER,
    to_pos INTEGER,
    selected_text TEXT,
    status TEXT NOT NULL DEFAULT 'Open',
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE
);

-- 评论消息表
CREATE TABLE comment_messages (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    author_id TEXT NOT NULL,
    author_name TEXT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (thread_id) REFERENCES comment_threads(id) ON DELETE CASCADE
);

CREATE INDEX idx_comment_threads_scene ON comment_threads(scene_id);
CREATE INDEX idx_comment_messages_thread ON comment_messages(thread_id);
```

---

## 7. 实施阶段

### Phase 1: 变更追踪核心（2 周）
- [x] 创建 `ChangeTrack` 数据模型 + 数据库表
- [x] 创建 `TrackChanges` TipTap Mark 扩展（insert/delete）
- [x] 实现 `track_change` / `accept_change` / `reject_change` 命令
- [x] 在 `RichTextEditor` 中集成修订模式开关

### Phase 2: 评论线程（1.5 周）
- [x] 创建 `CommentThread` / `CommentMessage` 模型 + 表
- [x] 创建 `CommentAnchor` TipTap Mark 扩展
- [x] 实现评论线程后端 API
- [x] 实现 `CommentThreadPanel` 右侧边栏
- [ ] 将现有 `TextAnnotation` 数据迁移到 `CommentThread`（可选）

### Phase 3: 版本集成与优化（1 周）
- [x] 在保存场景版本时自动生成 `ChangeTrack`（通过 Diff）
- [x] 在 `VersionTimeline` 中展示该版本的变更统计
- [x] 接受/拒绝变更后自动创建新版本快照
- [ ] 解决大规模文档下的性能优化（虚拟渲染变更标记）

---

## 8. 关键技术决策

### 8.1 位置映射策略
由于 TipTap HTML 文档与纯文本位置存在差异，采用 **版本锚定 + 文本指纹** 策略：
- `from_pos/to_pos` 存储纯文本偏移（兼容现有 `TextAnnotation`）
- 同时存储 `selected_text`（选中时的文本指纹）
- 重新加载时，优先用文本指纹在文档中搜索匹配，若失败则回退到位置偏移

### 8.2 变更与版本的关系
- **实时编辑** → 前端实时生成 `ChangeTrack`，每 30 秒自动保存为临时记录
- **显式保存** → 调用 `save_scene` 时，基于当前内容与上一版本的 Diff 生成正式的 `ChangeTrack`
- 这样既能追踪实时编辑，又保证版本系统的完整性

### 8.3 评论与批注的合并
- `SceneAnnotation`（场景级批注）保留，用于 TODO / 待办标记
- `TextAnnotation`（内联批注）逐步迁移到 `CommentThread`
- 最终统一为：**评论线程 = 带回复的内联批注**，场景批注 = 不带锚点的特殊线程

---

## 9. 风险与备选方案

| 风险 | 影响 | 备选方案 |
|------|------|---------|
| TipTap Mark 过多导致性能下降 | 高 | 使用 Decoration 替代 Mark，按需渲染可见区域内的变更 |
| 长文档位置偏移漂移 | 中 | 引入段落级锚定（paragraph_id + offset_in_paragraph）替代全局文本偏移 |
| OT 操作与 TipTap Step 映射复杂 | 中 | 不直接复用 `collab/ot.rs`，而是基于 `editor.getHTML()` 的 Diff 生成 `ChangeTrack` |

---

*最后更新: 2026-04-14*
*作者: Kimi Code CLI*
