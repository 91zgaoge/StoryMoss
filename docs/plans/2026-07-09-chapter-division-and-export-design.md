# 自动划分章节与故事导出功能设计

## 背景与现状

StoryForge 当前采用 **Scene 为内容真相源** 的架构（Phase 1）：`scenes` 表存储实际正文，`chapters` 表作为章节容器，通过 `chapter_id` 关联多个 Scene。`ChapterRepository` 已提供创建、查询、更新、删除章节的接口。

**导出功能已存在**：`src-frontend/src/components/ExportDialog.tsx` 已支持 `txt`、`md`（markdown）、`pdf`、`epub`、`html`、`json` 六种格式，并在 `src/pages/Stories.tsx` 的故事卡片与详情区域提供「导出」按钮，后端由 `src-tauri/src/commands/export.rs` 的 `export_story` 命令实现。

因此本设计主要新增 **章节自动划分** 功能，并对导出入口做最小化可用性确认。

## 目标

1. 在后台「工作室配置」中新增章节划分策略选项：
   - **自动划分（按情节）**：由 LLM 分析故事节奏与情节转折点，自动决定章节边界。
   - **按字数划分**：用户填入单章字数上限，系统按字数将 Scene 组合或拆分为新章节。
   - 未填字数时默认使用**自动划分**。
2. 在后台「故事管理」中为每个故事提供「自动划分章节」操作入口。
3. 确认并保留现有的导出功能（txt/md/pdf/epub 等）。

## 方案对比

| 方案 | 核心思路 | 优点 | 缺点 |
|------|---------|------|------|
| A. 仅字数分组 | 按字数上限将相邻 Scene 合并成新章节 | 实现简单、速度快、无 LLM 成本 | 不考虑情节，可能切断高潮 |
| B. 仅 LLM 情节分析 | 将全文喂给 LLM，让其输出章节边界 | 智能化、贴合叙事结构 | 有 Token 成本、速度较慢、结果不稳定 |
| C. 混合策略（推荐） | 设置层提供 `mode` 与 `word_count_target`；字数模式用 A，自动模式用 B | 兼顾可控性与智能化，符合用户「或者」表述 | 实现量略大 |

**推荐方案 C**：在设置中暴露两种模式，由用户选择；未填字数时回退到自动模式。

## 数据模型

### 应用设置扩展

在 `AppSettings`（`src-frontend/src/types/llm.ts` 与后端对应结构）中新增：

```typescript
interface ChapterDivisionConfig {
  mode: 'auto' | 'word_count';
  word_count_target?: number | null;
}
```

后端在 `settings.rs` 的 `AppSettingsData` 中增加同名字段，并通过 `save_settings` / `get_settings` 持久化到 DB。

### 章节划分请求

```rust
struct DivideChaptersRequest {
    story_id: String,
    mode: ChapterDivisionMode,
    word_count_target: Option<usize>,
}

enum ChapterDivisionMode {
    Auto,
    WordCount,
}
```

## 架构设计

### 后端

新增模块 `src-tauri/src/chapter_division/`：

- `mod.rs`：公共接口 `divide_chapters(pool, request) -> Result<Vec<Chapter>, AppError>`。
- `word_count.rs`：字数模式实现。
  - 获取 story 下所有 scene，按 `sequence_number` 排序。
  - 贪婪分组：累加 scene 字数，接近目标时切分；单个 scene 超过目标时独立成章。
  - 返回 `(chapter_title, scene_ids)` 列表。
- `plot_based.rs`：情节模式实现。
  - 拼接所有 scene 内容，构造 LLM prompt，要求输出 JSON：`[{ "title": "章标题", "cut_after_scene_index": 2 }]`。
  - 使用现有 `LlmService` 调用模型，失败后回退到字数模式并提示用户。
- `persistence.rs`：事务化写入。
  - 删除旧 chapters（保留关联 scenes 的 content）。
  - 按结果创建新 chapters，更新 scenes 的 `chapter_id` 与 `sequence_number`。

新增 Tauri 命令 `divide_chapters` 并注册到 `handlers.rs`。

### 前端

1. **设置页**（`src/pages/settings/GeneralSettings.tsx`）：
   - 新增「章节划分策略」卡片。
   - 单选：`自动划分（按情节）` / `按字数划分`。
   - 字数输入框：仅在「按字数」时启用；为空时提示将使用自动划分。
   - 通过 `useSettingsContext` 保存到 `AppSettings.chapter_division`。

2. **故事管理页**（`src/pages/Stories.tsx`）：
   - 在故事卡片/详情操作区新增「自动划分章节」按钮。
   - 点击后弹出确认对话框，显示当前章节数与预估新章节数（字数模式可精确计算，自动模式由后端先预览）。
   - 调用 `divide_chapters` 命令，成功后刷新故事与章节列表。

3. **导出**：
   - 现有 `ExportDialog` 已覆盖 txt/md/pdf/epub/html/json，无需重复实现。
   - 如需提升发现性，可在故事卡片上保留现有「导出」按钮（已存在）。

## 数据流

```
用户选择划分模式 → 保存到 AppSettings
                ↓
故事管理页点击「自动划分章节」
                ↓
前端读取当前设置 mode/word_count_target
                ↓
调用 divide_chapters(story_id, mode, word_count_target?)
                ↓
后端：
  - WordCount：贪婪分组 scenes
  - Auto：LLM 分析 → 边界列表
                ↓
事务：删除旧 chapters → 创建新 chapters → 更新 scenes.chapter_id
                ↓
返回新章节列表 → 前端刷新
```

## 错误处理

- 故事无 scene：返回明确错误「当前故事没有可划分的内容」。
- LLM 调用失败（自动模式）：降级为字数模式（若设置过字数目标）或返回错误。
- 事务失败：回滚，保留原章节结构。
- 字数目标 <= 0：校验失败，提示填入正整数。

## 测试策略

- **Rust 单元测试**：
  - `word_count` 分组边界（空、单 scene、恰好目标、超过目标）。
  - `plot_based` prompt 解析与降级。
  - 事务化持久化（旧 chapter 删除、新 chapter 创建、scene 关联更新）。
- **前端单元测试**：
  - 设置卡片模式切换与字数输入显隐。
  - 「自动划分章节」按钮调用正确参数。

##  Risks & Mitigation

- **风险**：LLM 输出不稳定导致章节划分质量差。
  - **缓解**：提供字数模式作为稳定回退；自动模式输出需校验格式，失败时降级。
- **风险**：删除旧 chapters 后用户无法恢复。
  - **缓解**：操作前强制 confirm 对话框；未来可接入版本系统做快照（本次不实现）。
- **风险**：Scene 为真相源，旧 `chapters.content` 可能为空，划分后需确保新 chapter 能从 scenes 聚合到内容。
  - **缓解**：复用 `ChapterRepository::get_content` 逻辑，不依赖 `chapters.content`。

## 导出功能说明

导出当前故事全部章节内容的功能已存在于 `ExportDialog`，支持：

- `txt`：纯文本
- `md` / `markdown`：Markdown
- `pdf`：PDF 文档
- `epub`：EPUB 电子书
- `html`：网页格式
- `json`：数据备份

入口位于 `Stories.tsx` 的故事卡片与详情区域。本实现将确认该入口可用，不再重复开发导出后端。
