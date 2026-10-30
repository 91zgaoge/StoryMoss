# 语料资产 · 导演 Agent · 幕后剧组对话 — 设计文档

日期：2026-10-30
状态：已获用户批准（2026-10-30 对话确认）；并发模型按「lane + task + lease + 异步派发」修订
建议版本：v0.74.0
影响范围：幕后工作室（语料页、代理工作室页）、agency 后端、故事资产、SQLite 迁移 V149

---

## 0. 一页纸结论

这次做三件事，但它们共用同一套底座：

1. **语料资产**：在幕后「故事资产」组新增「语料」页。语料是作者找来的新闻、故事、概念、灵感、图片、链接和随手笔记，按故事隔离；支持导入、检索、AI 提炼、转为角色/世界观/伏笔/场景草稿，以及交给导演使用。
2. **导演 Agent**：在现有 管理（Producer）、编审（EditorAuditor）、主创（LeadWriter）之上增加 **导演（Director）** 作为主 agent。导演不直接写正文，职责是巡检剧组、定位子 agent 错误、按最小干预原则调度修复，并向作者报告。
3. **幕后剧组对话**：把代理工作室从「只读观测页」升级为「对话 + 现场」工作台。页面中央是导演对话区，左侧是剧组花名册，右侧是现场（任务进程、黑板、时间线）。用户可以直接和导演说话，也可以 `@管理`、`@编审`、`@主创` 直达子 agent。

三项能力的技术底座是同一个 **剧组任务注册表（Agency Task Registry）**：现有 agent 都是同一 Tauri 进程里的 async 任务，没有 PID、没有进程表。我们新增 `agency_agent_tasks` 表，记录每次角色执行的开始、心跳、结束、错误和父子关系，让导演能「看得见」子 agent，也让用户能看见「谁在干活、卡在哪」。

**最重要的并发约束**：V109 的「同一故事只能有一个 `pending/running` run」是故事写入的一致性护栏，不是模型连接锁。本设计把它升级为 **lane + task + lease**：

- 顶层 run 按 `lane` 区分：`write`（创世/续写/批量/资产刷新）、`review`（独立复审）、`inspect`（导演巡检）、`chat`（剧组对话）。同一 story 同一 lane 同时只能有一个活跃 run；不同 lane 可以并行。
- 子 agent 不再各自开 run，而是写 `agency_agent_tasks`，由调度器 `tokio::spawn` 异步执行；`dispatch` 立即返回 `task_id`，导演通过事件回执继续对话，不阻塞等待。
- 真正写库的提交点用 `story_resource_leases` 串行化：章节号、场景追加、资产物化、草稿、质量门各自一把租约；读操作不加锁。
- 一句话：**异步的是任务与调度，串行的是提交点**。不是把所有进程拉成一条线。

剧组对话与巡检不创建 `write` run，只用 `chat:<session_id>` 作为任务/活动分组键；因此它们不会占用创世/续写的写入位。

---

## 1. 背景与目标

### 1.1 用户诉求

- 故事资产里有场景、角色、世界构建、伏笔，但缺少「作者找来的外部素材」这一层。素材散落在文件夹、浏览器收藏和笔记里，无法进入创作流程。
- 幕前已经有「用户 ↔ 主创」的指令式交流，但幕后没有「用户 ↔ 管理/编审」的对话入口。代理工作室目前只能看，不能问、不能调度。
- 代理工作室有三个 agent，但没有一个对它们负责的主 agent。出错时用户只能看着失败状态，无法让系统自检、重试或修复。

### 1.2 代码现状（设计依据）

| 事实 | 位置 | 对设计的影响 |
|---|---|---|
| 幕后路由是 Zustand `currentView` + `App.tsx` switch | `src-frontend/src/App.tsx:318`、`src-frontend/src/types/index.ts:299` | 新增语料页要加 `ViewType`、`App` 分支、Sidebar 项 |
| 资产页模板：story-scoped 查询 + TanStack Query | `src-frontend/src/pages/Scenes.tsx`、`hooks/useScenes.ts` | 语料按 `['corpus', storyId]` 缓存，删故事时清理 |
| 迁移当前最高 V148，SQL 文件自动扫描 | `src-tauri/src/db/migrations/mod.rs:114` | 新迁移用 V149 |
| 代理角色是 `AgentRole` 枚举，7 个值 | `src-tauri/src/agency/models.rs:6` | 新增 Director 要同步 `as_str/from_str/all/spec_for/context_label/budget` |
| 黑板 4 分区，单一写入者 | `src-tauri/src/agency/models.rs:58`、`board.rs:102` | 新增「导演」分区，owner = Director |
| 角色执行走 `ToolLoop`，有工具白名单与轮次上限 | `src-tauri/src/agency/tool_loop.rs`、`tools.rs:180` | 导演巡检测试工具、子 agent 派发复用同一循环 |
| 现有 run 生命周期与活动日志 | `V107`、`V129`、`coordinator.rs:1526` | 任务注册表补充「turn 级心跳」，不改现有事件契约 |
| V109 每故事仅一个进行中 run | `V109__agency_runs_active_unique.sql` | 升级为 `(story_id, lane)` 唯一索引；write 仍单活跃，review/inspect/chat 可并行 |
| 并发仍有非模型闸门 | `budget.rs:19` 全局 3、`budget.rs:34` 每角色 1、`concurrency.rs:8` 后台 1 | 导演预留 permit；角色许可可配；后台任务与用户任务分队列 |
| `chat_sessions/chat_messages` 已建但未暴露 | `V029`、`src-tauri/src/chat/mod.rs:1` | 不复用旧表；新建 agency 专用会话表，避免语义混淆 |
| 文件导入先例：拆书/指导书 | `book_deconstruction/service.rs:57`、`guidebook_distillation` | 复用「选择文件 → 校验 → 复制到 app_data → 落库」 |
| 图片没有现成预览方案 | 未启用 asset protocol | P0 用命令返回 data URL；限制大小、懒加载 |
| 单次 AI 调用入口 | `LlmService::generate_for_task_with_system_prompt`、`generate_for_task` | 语料提炼、导演规划复用 |

### 1.3 设计原则

1. **导演是编排层，不是第二个写手**。导演只做巡检、判断、调度、报告；正文和资产仍由 管理/编审/主创 执行。
2. **先看后动，最小干预**。能重试不重写，能补缺不推翻，能提案不覆盖作者手写内容。
3. **作者主权**。覆盖手写资产、删除、批量续写、取消运行必须二次确认；只读巡检与新增型任务可直接执行。
4. **可追溯**。每次调度/修复都落 `agency_director_actions`，聊天里能看到动作、对象、原因、结果。
5. **不阻塞创作，也不把进程拉直**。对话不占用 `write` lane；子 agent 任务异步派发、事件回执、按角色限流；只有写库提交点用资源租约串行化。
6. **渐进增强**。P0 先把闭环跑通（lane + 任务注册表 + 异步派发、语料 CRUD/提炼/转资产、导演对话/巡检/重试、三栏工作台），P1 再做多 write 并行（资源租约 + 原子章号）、语料进上下文、流式 token、自动巡检。

---

## 2. 总体架构

```
┌──────────────────────────── 幕后工作室（React） ────────────────────────────┐
│  Sidebar: 代理工作室 / 语料（新） / 场景 / 角色 / …                          │
│                                                                              │
│  AgencyStudio（改造）                         Corpus（新页面）                │
│  ┌──────────┬──────────────────┬──────────┐   ┌──────────────────────────┐  │
│  │ 剧组花名册│ 剧组对话          │ 现场      │   │ 语料列表 / 详情 / 导入    │  │
│  │ 导演      │ 消息流 + 输入条   │ 任务进程  │   │ AI 提炼 / 转为资产 /      │  │
│  │ 管理      │ 动作轨迹 + 审批卡 │ 黑板/时间线│   │ 交给导演                  │  │
│  │ 编审      │                  │          │   └──────────────────────────┘  │
│  │ 主创      │                  │          │                                 │
│  └──────────┴──────────────────┴──────────┘                                 │
└───────────────────────────────┬──────────────────────────────────────────────┘
                                │ IPC
┌───────────────────────────────▼──────────────────────────────────────────────┐
│ Rust: agency/director.rs（巡检/计划/执行/报告）                               │
│       agency/chat.rs（会话/消息/路由）                                        │
│       agency/crew.rs（lane/任务注册表/调度器/心跳/卡住判定/资源租约）          │
│       agency/tools.rs（新增 crew_status / corpus_search / corpus_read）        │
│       corpus/*（导入/解析/提炼/转资产草稿）                                    │
├──────────────────────────────────────────────────────────────────────────────┤
│ SQLite V149: corpus_items / corpus_links /                                    │
│              agency_chat_sessions / agency_chat_messages /                    │
│              agency_agent_tasks / agency_director_actions /                    │
│              story_resource_leases；agency_runs + lane                         │
│ 现有: agency_runs / agency_board_items / agency_activity_log / …              │
└──────────────────────────────────────────────────────────────────────────────┘
```

三个新增面共享的边界：

- **故事隔离**：所有新表都有 `story_id`，FK `stories(id) ON DELETE CASCADE`（任务表用 `run_id`/`context_key` 关联，合成键不建 FK）。
- **模型路由**：导演用 `TaskType::Analysis` + `context_label = agency_director`，默认走 Tool 档；子 agent 沿用各自档位（主创 Creative、管理 Tool、编审 Background）。
- **事件**：新增 `agency-chat-update`、`agency-crew-update`；现有 `agency-agent-activity`、`agency-run-progress`、`agency-board-changed` 保持不变。
- **测试契约**：新增命令必须进 `handlers.rs` 的 Agency 段；`tests/ipc_registry_contract_test.rs` 增加导演/语料段断言；`scripts/verify-ipc-manifest.py` 必须全绿。

### 2.1 并发模型：lane + task + lease

| 层 | 单位 | 并发规则 | 冲突处理 |
|---|---|---|---|
| **Lane** | `agency_runs.lane` | 同一 story 同一 lane 同时只能有一个 `pending/running` run；`write` 串行，`review`/`inspect`/`chat` 可并行 | 结构化 `ResourceBusy { owner, retry_after }` |
| **Task** | `agency_agent_tasks` | 子 agent 异步 `tokio::spawn`；按角色限流（Director 1 / Writer 1 / Producer 2 / Editor 2）；`dispatch` 立即返回 `task_id` | 重试、取消、交给导演 |
| **Lease** | `story_resource_leases` | 写库提交点互斥：`chapter_sequence` / `scene:<id>` / `assets:<group>` / `draft:<key>` / `gate:<chapter>`；读不加锁 | 等待、过期回收、导演裁决 |

执行流（异步，不是线性）：

```
用户消息
  → 创建 Director turn task（inspect → plan）
  → dispatch 动作：创建子 agent task，立即返回 task_id
  → 子 agent tasks 并发执行（受角色许可/全局模型闸门约束）
  → 每个 task 心跳 + agency-crew-update 事件
  → 需要写库的任务在提交前 acquire resource lease
  → 全部任务结束（或部分失败）→ Director follow-up message 汇总
```

导演的聊天回合**不等待子 agent 跑完**：它先回复「已派发，3 个任务在跑」，后续用事件驱动的 follow-up 消息汇报结果。这样用户可以在任务运行时继续对话，导演也能在别的任务完成后插入新的调度。

P0 保留「一个 story 一个 `write` lane」作为写入事务边界；`review`/`inspect`/`chat` 已经并行。P1 用资源租约 + 原子章号把 `write` 也拆成可并行的章节任务。

---

## 3. 语料资产（Corpus）

### 3.1 定位与边界

语料是**作者提供的原料**，不是 AI 生成的故事资产。它可以是：

| kind | 说明 | 典型来源 |
|---|---|---|
| `news` | 新闻、报道、真实事件 | 网页、剪报、截图 |
| `story` | 短故事、传说、案例 | 文档、粘贴 |
| `concept` | 概念、理论、设定点子 | 笔记、论文摘要 |
| `inspiration` | 灵感、画面、对白、片段 | 随手记 |
| `image` | 图片、照片、参考图 | 本地文件 |
| `link` | 网页链接 + 摘录 | URL |
| `note` | 其他笔记 | 手动输入 |

**不做的事（P0）**：不做全网搜索、不做 OCR/视觉理解、不做自动版权判断。图片先作为「参考素材」保存和展示；是否让视觉模型读图，P2 再评估。

### 3.2 数据模型

```sql
CREATE TABLE corpus_items (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,                    -- news/story/concept/inspiration/image/link/note
  title TEXT NOT NULL,
  summary TEXT NOT NULL DEFAULT '',      -- 列表摘要（用户可改，导入时自动取首段）
  content TEXT NOT NULL DEFAULT '',      -- 原文/笔记/网页正文
  source_url TEXT,
  source_name TEXT,
  tags TEXT NOT NULL DEFAULT '[]',       -- JSON string[]
  file_path TEXT,                        -- app_data/corpus/<story_id>/<id>.<ext>
  file_name TEXT,
  mime_type TEXT,
  file_size INTEGER,
  file_hash TEXT,                         -- sha256，同故事内去重
  status TEXT NOT NULL DEFAULT 'raw',    -- raw | distilled | used | archived
  pinned INTEGER NOT NULL DEFAULT 0,     -- 是否固定进续写上下文（P1）
  distilled_json TEXT,                    -- CorpusDistillation
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_corpus_story_kind ON corpus_items(story_id, kind, created_at);
CREATE UNIQUE INDEX idx_corpus_story_hash
  ON corpus_items(story_id, file_hash) WHERE file_hash IS NOT NULL;

CREATE TABLE corpus_links (
  id TEXT PRIMARY KEY,
  corpus_id TEXT NOT NULL REFERENCES corpus_items(id) ON DELETE CASCADE,
  target_type TEXT NOT NULL,   -- character | world | outline | foreshadowing | scene | chapter
  target_id TEXT NOT NULL,
  note TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_corpus_links_corpus ON corpus_links(corpus_id);
CREATE INDEX idx_corpus_links_target ON corpus_links(target_type, target_id);
```

`CorpusDistillation`（`distilled_json` 的结构）：

```json
{
  "version": 1,
  "model": "…",
  "generated_at": "…",
  "facts": ["可核实的核心事实"],
  "conflicts": ["可直接使用的冲突/张力"],
  "archetypes": ["人物原型/关系"],
  "world_hooks": ["世界观钩子/规则"],
  "adaptation_angles": ["可改编方向"],
  "cautions": ["与现有设定可能冲突之处"]
}
```

**为什么单独建表，不复用 `reference_books`**：拆书是「整本书 → 结构化拆解」，语料是「碎片素材 → 创作原料」，生命周期、检索方式、UI 都不同。复用一个表会让两种语义互相污染。

### 3.3 导入管线

| 入口 | 行为 |
|---|---|
| 粘贴文本 | 直接 `corpus_create`；自动取首段生成 summary |
| 粘贴链接 | 保存 URL；后端 best-effort 抓取 `<title>` 和正文（`reqwest` 已有，HTML 去标签，最多 50k 字符）；失败则保留 URL + 用户摘录 |
| 导入文件 | `corpus_import_file`：校验扩展名/大小 → SHA-256 去重 → 复制到 `app_data_dir/corpus/<story_id>/<uuid>.<ext>` → txt/md/pdf/epub 复用 `book_deconstruction::parser` 提取文本 → 落库 |
| 导入图片 | 同上，但只保存文件与元数据；不解析内容；详情页按需读取预览 |

限制：文本文件 20 MB；图片 10 MB；支持 `txt/md/pdf/epub/png/jpg/jpeg/webp/gif`。超限或格式不支持返回结构化错误。

**图片预览**：P0 用命令 `corpus_read_media(id)` 读取文件并返回 `{ mime, data_url }`（base64，仅详情页懒加载，单张上限 10 MB）。这样不改 CSP、不启用 asset protocol，风险最低。P1 如需列表缩略图，再评估 scoped asset protocol。

### 3.4 AI 提炼

命令 `corpus_distill(id)`：

1. 读取语料 `title + content`（截断 12k 字符）与该故事上下文（标题、题材、基调、logline、世界观摘要，约 2k 字符）。
2. 调 `LlmService::generate_for_task_with_system_prompt(TaskType::Analysis, context_label="corpus_distill")`。
3. 要求严格 JSON，按 `CorpusDistillation` 解析；解析失败时用宽松提取（复用项目现有的 JSON 容错思路）并保留原文。
4. 写回 `distilled_json`，`status = distilled`。
5. 返回提炼结果；失败不破坏原语料，只返回错误。

提炼结果不是「故事资产」，不会自动写入角色/世界观表；它只是作者决策的辅助。

### 3.5 转为资产（草稿，不直接落库）

命令 `corpus_promote(id, target_type)`，`target_type` ∈ `character | world | foreshadowing | outline | scene`：

- 用提炼结果 + 原文，生成**目标类型的草稿 JSON**（不是最终资产）。
- 前端把草稿预填到现有编辑入口：
  - 角色 → `CharacterEditModal` / 新建角色弹窗
  - 世界观 → `WorldBuilding` 编辑区
  - 伏笔 → 伏笔创建表单
  - 大纲/场景 → 章纲/场景编辑入口
- 作者确认保存后，前端调用 `corpus_link(corpus_id, target_type, target_id)` 建立溯源，`status = used`。

**为什么不在后端直接写资产**：现有资产写入有大量不变量（KG 双写、旧名别称、级联、审计），直接写会绕开这些保护。走「草稿 → 现有编辑入口 → 用户确认」最安全，也符合作者主权。

### 3.6 进入创作上下文

P0：

- 所有 agency 角色获得只读工具 `corpus_search`、`corpus_read`。导演/管理/编审/主创都可以在对话或任务中读取语料。
- 语料详情页的「交给导演」会打开代理工作室对话，预填：
  `用这条语料《{title}》丰富{目标}：{summary}。先给方案，不要直接覆盖已有人工设定。`
- 聊天命令接受 `corpus_ids: string[]`，导演简报里带语料摘要，派发子 agent 时把 `corpus_ids` 传入任务。

P1：

- `pinned=1` 的语料进入续写上下文：在 `continue_assets.rs` 增加「作者提供的语料」段，最多 3 条、总预算 1200 字符，位置在世界观红线之后、角色卡之前；测试钉住预算和截断。
- 语料语义检索（向量库）替代 LIKE 搜索。

### 3.7 页面与交互

```
┌──────────────────────────────────────────────────────────────────────┐
│ 语料库 · 《长夜将明》         [搜索标题/内容/标签]      [+ 导入语料]   │
│ 12 条 · 4 条待提炼 · 3 条已用于创作                                   │
│ [全部] [新闻] [故事] [概念] [灵感] [图片] [链接] [笔记]               │
├──────────────────────────────────────────────────────────────────────┤
│ ┌────────────┐ ┌────────────┐ ┌────────────┐                        │
│ │ [图片]     │ │ 新闻       │ │ 概念       │                        │
│ │ 老城区雨夜 │ │ 城市更新…  │ │ 时间循环…  │                        │
│ │ #氛围 #场景│ │ #社会 #冲突│ │ #结构      │                        │
│ │ 已提炼  📌 │ │ 待提炼     │ │ 已用于创作 │                        │
│ └────────────┘ └────────────┘ └────────────┘                        │
│                                                                      │
│ 右侧详情抽屉（选中卡片后出现）：                                      │
│ ┌───────────────────────────────┬──────────────────────────────────┐ │
│ │ 原文 / 图片预览                │ AI 提炼                          │ │
│ │ …                             │ 核心事实 · 可用冲突 · 人物原型    │ │
│ │                               │ 世界观钩子 · 改编建议             │ │
│ │                               │ [重新提炼]                       │ │
│ │                               │ [转为角色][世界观][伏笔][场景]    │ │
│ │                               │ [交给导演]                       │ │
│ └───────────────────────────────┴──────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

- 列表默认卡片网格；图片卡用图片预览（懒加载）或图标占位；支持切换紧凑列表。
- 筛选：kind chips + 状态（待提炼/已提炼/已用/归档）+ 搜索（标题/摘要/内容/标签）。
- 导入弹窗：四个 Tab（文本/链接/文件/图片）；文件与图片走 Tauri dialog；「导入后自动提炼」默认勾选。
- 详情：原文与 AI 提炼两栏；图片可放大；「转为资产」生成草稿并跳到现有编辑入口；「交给导演」跳转代理工作室。
- 空态：解释语料是什么，给三个示例入口（粘贴新闻、导入图片、记一条灵感）。

### 3.8 命令清单（语料）

| 命令 | 参数 | 返回 |
|---|---|---|
| `corpus_list` | `story_id, kind?, status?, query?, limit?, offset?` | `Vec<CorpusSummary>` |
| `corpus_get` | `id` | `CorpusDetail`（含 links、distilled） |
| `corpus_create` | `story_id, kind, title, content?, summary?, source_url?, source_name?, tags?` | `CorpusDetail` |
| `corpus_update` | `id, updates` | `CorpusDetail` |
| `corpus_delete` | `id` | `()`（同时删物理文件） |
| `corpus_import_file` | `story_id, kind, file_path, title?, tags?` | `CorpusDetail` |
| `corpus_read_media` | `id` | `{ mime, data_url }` |
| `corpus_distill` | `id` | `CorpusDistillation` |
| `corpus_promote` | `id, target_type` | `CorpusPromotionDraft`（目标类型草稿） |
| `corpus_link` | `corpus_id, target_type, target_id, note?` | `()` |
| `corpus_unlink` | `link_id` | `()` |
| `corpus_search`（内部工具） | `story_id, query, limit?` | 摘要列表 |

---

## 4. 导演 Agent（Director）

### 4.1 角色定义

| 项 | 值 |
|---|---|
| `AgentRole::Director` | `as_str = "director"` |
| 显示名 | 导演 |
| system prompt | `resources/prompts/agency/agency_director_system.md`（`id: agency_director_system`） |
| `TaskType` | `Analysis` |
| 默认模型档 | `Tool`（`context_label = agency_director`；可在模型映射里改） |
| `RoleSpec` | `max_turns = 8`、`max_output_tokens = 4096`、`context_budget_chars = 32_000`（`max_turns` 预留给 P1 的导演工具循环；P0 用单次规划调用） |
| 黑板分区 | 新增 `direction`，owner = Director |
| 工具权限 | 只读巡检工具 + 受控动作目录（见 4.5）；**不直接获得** `board_write` / `board_revise` / 正文写入 |

导演不是 OS 进程，也不是第二个 `ToolLoop` 写手。它是 **inspect → plan → dispatch → follow-up** 的异步编排循环：

```
用户消息 / 巡检触发
        │
        ▼
① 巡检 CrewInspector.inspect(story_id)
   run 状态、任务心跳、lane/租约、黑板缺口、审查阻塞、质量债、语料引用
        │
        ▼
② 计划 DirectorPlanner.plan(brief)
   LLM 输出严格 JSON：reply + actions[] + needs_confirmation
        │
        ▼
③ 派发 DirectorExecutor.execute(actions)
   dispatch / retry / resume / materialize / recheck_gate / escalate
   创建 task 后立即返回 task_ids；需要确认的动作先挂起，返回审批卡
        │
        ▼
④ 即时回复 + follow-up
   先发「已派发，N 个任务在跑」；任务完成/失败事件到达后，
   由 TaskScheduler 回调追加 follow-up 消息 + agency_director_actions
```

### 4.2 与现有三角色的关系

| 角色 | 定位 | 导演能对它做什么 |
|---|---|---|
| 管理（Producer） | 资产、计划、世界观、章纲 | 派发补齐/重写资产、物化黑板、生成章纲；覆盖手写内容需确认 |
| 编审（EditorAuditor） | 质量门、审查、问题清单 | 派发复审、重跑 gate、查看阻塞证据；不能代替作者做最终裁决 |
| 主创（LeadWriter） | 正文、续写、修订 | 派发续写/修订；批量续写、覆盖正文必须确认 |
| 写手/检查/大纲/风格 | 映射执行单元 | 不是导演的直属子 agent；由管理/编审/主创内部使用，不单独出现在花名册 |

花名册固定 4 张卡：导演、管理、编审、主创。执行单元在「现场」里以任务行出现，避免 UI 变成 8 个平级 agent。

### 4.3 巡检快照（CrewInspector）

`inspect(story_id) -> CrewSnapshot`：

```rust
struct CrewSnapshot {
    story: StoryBrief,                 // 标题/题材/基调/logline/章数/字数
    current_run: Option<RunBrief>,     // 最近 run：status/phase/error/result
    tasks: Vec<TaskBrief>,             // 最近 20 条任务（含子 agent）
    activity: Vec<ActivityBrief>,      // 最近 30 条角色活动
    board: BoardBrief,                 // 各分区条目数、缺失资产、proposed 提案
    review: ReviewBrief,               // 最新 gate 裁决、blocking 问题、质量债
    corpus: CorpusBrief,               // 语料条数、待提炼、已引用
    alerts: Vec<CrewAlert>,            // 见 4.4
}
```

**告警类型**（`CrewAlert`）：

| 类型 | 触发条件 | 严重度 |
|---|---|---|
| `StaleTask` | `running` 且 `heartbeat_at` > 300s | high |
| `SlowTask` | `running` 且 `heartbeat_at` > 90s | warn |
| `FailedTask` | `failed` 且未处理 | high |
| `FailedRun` | run `failed` | high |
| `MissingAsset` | 黑板/DB 缺世界观、大纲、关系 | warn |
| `GateBlocked` | 最新审查有 blocking 且未处理 | high |
| `EmptyDraft` | 草稿区 active 条目为空/过短 | warn |
| `BudgetExhausted` | run 预算耗尽或熔断 | high |
| `ParseCircuitBreak` | 连续解析失败熔断 | high |
| `OrphanTask` | 任务 `running` 但所属 run 已终态 | warn |
| `ResourceLeaseExpired` | 租约 `expires_at` 已过且 owner 未释放 | warn/high |
| `CorpusConflict` | 提炼 cautions 与现有设定冲突（P1） | warn |

巡检是只读的、确定性的，不调用 LLM。只有把快照交给 DirectorPlanner 时才调用模型。

### 4.4 计划输出（DirectorPlan）

严格 JSON：

```json
{
  "reply": "给作者的中文回复，先结论后动作",
  "intent": "inspect | dispatch | repair | chat | corpus | none",
  "actions": [
    {
      "id": "a1",
      "type": "dispatch",
      "role": "producer",
      "instruction": "补齐世界观缺失的势力设定；不要覆盖已有手写内容",
      "corpus_ids": ["…"],
      "depends_on_action": null,
      "reason": "黑板 asset 区缺 world/relationship"
    },
    {
      "id": "a2",
      "type": "dispatch",
      "role": "editor_auditor",
      "instruction": "复审第 12 章",
      "depends_on_action": "a1",
      "reason": "管理补完资产后再审"
    },
    {
      "type": "repair",
      "action": "retry_task",
      "target_id": "task-…",
      "reason": "编审任务心跳超时"
    }
  ],
  "needs_confirmation": false,
  "confirmation": null
}
```

解析失败时回退：只返回巡检摘要 + 一条「导演没听懂，请换个说法」的回复，不执行任何动作。**计划失败不能产生副作用**。

### 4.5 动作目录与确认策略

| action | 参数 | 行为 | 确认 |
|---|---|---|---|
| `dispatch` | `role`, `instruction`, `corpus_ids?`, `depends_on_action?` | 创建子 agent task，异步执行，立即返回 `task_id`；`depends_on_action` 由执行器映射为父 task_id，父任务完成后才开始；导演不等待 | 管理/编审的「补齐/复审」可直接；主创续写/修订需确认；批量需确认 |
| `retry_task` | `task_id` | 用原任务参数重跑失败任务 | 可直接 |
| `cancel_task` | `task_id` | 协作式取消（下一轮生效） | 需确认 |
| `cancel_run` | `run_id` | 取消当前 run | 需确认 |
| `resume_run` | `run_id` | 复用现有 `resume_prepare`/`resume_run` | 需确认 |
| `materialize_assets` | `run_id` | 把黑板资产落库 | 可直接（幂等） |
| `ensure_assets` | `story_id` | 补齐缺失资产（可能调 LLM） | 需确认 |
| `recheck_gate` | `run_id` / `draft_key` | 重跑质量门 | 需确认（会调编审） |
| `record_debt` | `story_id`, `note` | 登记质量债并关闭告警 | 可直接 |
| `escalate` | `message` | 不自动修复，交给作者 | 可直接（只发消息） |

**确认卡**：需要确认的动作先写 `agency_director_actions.status = proposed`，聊天里渲染 `AiApprovalCard`（或专用审批卡）显示「将做什么 / 影响谁 / 为什么」。作者确认后调用 `agency_director_execute(action_ids: Vec<String>)`；取消则调用 `agency_director_reject(action_ids)` 标记 `rejected`。

**硬性限制**：

- 单轮最多 3 个 action；`dispatch` **异步并发**，按角色限流（Director 1 / Writer 1 / Producer 2 / Editor 2），不阻塞导演回合。
- 任务之间不共享可变状态；需要写库的动作在提交前 acquire `story_resource_leases`，拿不到就等待/重试，不直接改库。
- 导演不能直接调用 `board_write`/`board_revise`；只能通过子 agent 或幂等的 `materialize` 修复。
- 任何覆盖作者手写资产的指令必须带 `overwrite=false` 语义；子 agent 的提示词明确「只补空，不覆盖」。
- 修复失败两次后停止自动尝试，转为 `escalate` + 质量债。

### 4.6 修复策略矩阵

| 症状 | 首选动作 | 失败回退 |
|---|---|---|
| 子 agent 心跳超时/卡住 | `cancel_task` → `retry_task` | `escalate` + 记录告警 |
| 子 agent 失败（解析/模型错误） | `retry_task` | 换模型重试一次 → `escalate` |
| 资源租约被占用 | 等待 owner 完成 / 选择另一资源 | 过期回收 → 重试 → `escalate` |
| run 失败但黑板有草稿 | `materialize_assets` / `resume_run` | `escalate` |
| 缺世界观/大纲/关系 | `dispatch producer 补齐（只补空）` | `ensure_assets` |
| 审查 blocking | `dispatch editor 复审` / `recheck_gate` | `record_debt` + 作者裁决 |
| 草稿过短/空 | `dispatch lead_writer 修订`（需确认） | `escalate` |
| 预算耗尽 | 停止派发，`escalate` | — |
| 语料与设定冲突 | `escalate`，给出冲突点 | — |

### 4.7 失败与降级

- **LLM 不可用**：导演只返回巡检摘要与告警，不派发；聊天消息标记 `degraded`。
- **计划 JSON 解析失败**：不执行动作；回复「我先看了一遍现场……」+ 摘要。
- **子 agent 超时**：任务标记 `failed`，导演报告「已派发但超时」，给出重试按钮。
- **资源租约冲突**：任务进入 `waiting`，写 `agency-crew-update`；租约过期或 owner 结束后自动重试一次，仍失败交给导演裁决。
- **进程退出**：任务表残留 `running`；启动时 `reap_stale_tasks` 标记 `stale`，巡检产生 `StaleTask` 告警。
- **事件丢失**：聊天消息与任务状态都在 DB，前端 3s 轮询 + 事件双通道；页面重开可恢复。

### 4.8 提示词与模型路由

新增 `resources/prompts/agency/agency_director_system.md`：

- frontmatter：`id: agency_director_system`、`category: system`、`variables: [premise]`（只声明 `premise`，动态简报放在 user prompt，满足 `prompts/registry.rs` 的 fail-closed 占位符测试）。
- 正文包含：身份、职责、三子 agent 名册、巡检优先、最小干预、作者主权、动作目录、JSON 输出 schema、中文风格、禁止事项（不写正文、不覆盖手写、不伪造事实、不越权）。
- 用户 prompt 由 `DirectorBrief` 渲染：用户消息、最近 8 轮对话、CrewSnapshot 摘要、动作目录、语料摘要（如有）、当前时间/章节。

模型路由：

- `AgentRole::Director` → `AgencyLlm::context_label = "agency_director"`。
- `llm/service.rs::derive_model_role_from_label` 增加 `agency_director => Tool`（默认）。
- `AgencyBudget` 增加 `director_sem`（容量 1），`acquire(Director)` 用它，避免导演与编审抢同一槽位；Director turn 在 run 预算之外，另用全局 `DIRECTOR_LLM_SEM`（容量 1）。
- `agency_eval_overview` 的 purpose 聚合增加 `agency_director`。

---

## 5. 幕后剧组对话

### 5.1 会话模型

```sql
CREATE TABLE agency_chat_sessions (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  target_role TEXT NOT NULL DEFAULT 'director',
  summary TEXT,
  summary_upto_message_id TEXT,
  archived INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_agency_chat_sessions_story ON agency_chat_sessions(story_id, updated_at);

CREATE TABLE agency_chat_messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES agency_chat_sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL,          -- user | director | producer | editor_auditor | lead_writer | system
  content TEXT NOT NULL,
  status TEXT NOT NULL,        -- pending | thinking | acting | waiting | done | failed | cancelled | degraded
  run_key TEXT,                -- chat:<session_id> 任务分组键（不是 agency_runs 行）
  task_id TEXT,                -- 本轮导演 turn 的 task id
  task_ids_json TEXT,          -- 派发的子 agent task ids（JSON 数组）
  actions_json TEXT,           -- DirectorPlan.actions / DirectorAction 结果
  meta_json TEXT,              -- 语料引用、模型、tokens 等
  error TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_agency_chat_messages_session ON agency_chat_messages(session_id, created_at);
```

**为什么用 `chat:<session_id>` 作为任务分组键**：聊天/巡检不是 `write` run，不应占用故事的写入位；但它们仍需要把任务、活动日志、黑板写入归到同一个上下文里。`chat:<session_id>` 只是 `agency_agent_tasks.run_id` / `agency_activity_log.run_id` 的字符串分组键，不写 `agency_runs` 行，也不受 lane 索引约束。子 agent 如果产生写库动作，仍然要 acquire 资源租约；租约是提交点的锁，不是聊天上下文的锁。

### 5.2 消息状态与事件

消息状态机：

```
pending → thinking → acting → waiting（资源租约排队）→ done
                    ├→ failed
                    └→ cancelled
thinking/acting → degraded（模型不可用，返回巡检摘要）
```

`acting` 表示子 agent 任务已派发、尚未全部结束；完成后由 Director follow-up 消息把状态推进到 `done`。同一个会话里可以有多条 `acting` 消息，因为它们对应不同的任务组。

事件：

| 事件 | payload | 用途 |
|---|---|---|
| `agency-chat-update` | `{session_id, message_id, role, status, content?, stage?, detail?, task_ids?}` | 消息新增/状态/阶段更新 |
| `agency-crew-update` | `{story_id, task}` | 任务开始/心跳/结束/`waiting`，现场面板即时刷新 |
| `agency-lease-update` | `{story_id, resource, owner_task_id, status}` | 租约 acquire/release/expire |
| `agency-director-action` | `{action_id, status, summary}` | 审批卡/动作执行结果 |

P0 不做 token 级流式：导演的「已派发」即时回复和任务完成后的 follow-up 各是一条独立消息，前端用 `AiStreamingText` 做本地逐字呈现；阶段（检视/计划/派发/观察/修复/报告）通过 `agency-chat-update.stage` 实时显示。P1 再评估把 ToolLoop 的 token 流接入。

### 5.3 路由：导演 / 子 agent 直达

| 输入方式 | 路由 | 行为 |
|---|---|---|
| 默认输入 | Director | 走 inspect → plan → dispatch；dispatch 立即返回 task_ids，任务异步执行 |
| 点击「管理」卡 / `@管理` | Producer | 创建 Producer task，异步执行；回复以管理身份落消息 |
| 点击「编审」卡 / `@编审` | EditorAuditor | 同上 |
| 点击「主创」卡 / `@主创` | LeadWriter | 同上；写入正文类指令仍弹确认 |
| `/巡检` | Director | 只巡检 + 摘要，不调 LLM 规划，不创建任务 |
| `/修复` | Director | 巡检 → 只派发修复类 task |
| `/续写` | Director → LeadWriter | 先确认，再创建 LeadWriter task |
| `/语料` | Director | 从语料库选择/引用后进入规划 |

子 agent 直达是「跳过导演的判断，但仍在剧组现场留痕」：任务注册表照常记录，导演能看到并接管后续修复。所有直达任务也返回 `task_id`，聊天消息进入 `acting`，完成事件到达后追加角色自己的 follow-up 消息。

### 5.4 页面布局（三栏）

```
┌────────────────────────────────────────────────────────────────────────────┐
│ 代理工作室 · 《长夜将明》      [run 选择]   ● 导演在线    [巡检] [取消运行]  │
├──────────────┬────────────────────────────────────────────┬────────────────┤
│ 剧组          │ 对话                                        │ 现场            │
│ ┌──────────┐ │ ┌ 导演 ─────────────────────────────────┐  │ [现场][黑板][时间线]│
│ │ 导演 ●   │ │ │ 我在。管理正在补世界观，编审待命。    │  │ ● 管理 执行中    │
│ │ 总控     │ │ │ 建议先让编审复审第 12 章。           │  │   补齐势力设定   │
│ └──────────┘ │ └───────────────────────────────────────┘  │   心跳 3s        │
│ ┌──────────┐ │ ┌ 你 ───────────────────────────────────┐  │ ● 编审 待命      │
│ │ 管理 ●   │ │ │ 让编审复审第 12 章，有问题直接修。    │  │ ○ 主创 待命      │
│ └──────────┘ │ └───────────────────────────────────────┘  │                  │
│ ┌──────────┐ │ ┌ 导演 · 正在调度 ──────────────────────┐  │ 告警 2           │
│ │ 编审 ○   │ │ │ ✓ 检视剧组  ✓ 派发编审  ● 观察中     │  │ 编审心跳延迟 42s │
│ └──────────┘ │ │ 编审：发现 3 处视角越界…              │  │ [重试][交给导演] │
│ ┌──────────┐ │ └───────────────────────────────────────┘  │                  │
│ │ 主创 ○   │ │ ┌───────────────────────────────────────┐  │ 资产缺口 2       │
│ └──────────┘ │ │ 对导演说，或 @管理/@编审/@主创…        │  │ 审查阻塞 1       │
│ [新会话]     │ │ [/巡检][/修复][/续写][/语料]      [发送]│  │ [去处理]         │
│              │ └───────────────────────────────────────┘  │                  │
└──────────────┴────────────────────────────────────────────┴────────────────┘
```

要点：

- **对话是页面主视觉**，不是抽屉/弹窗；打开代理工作室第一眼就能说话。
- 左侧花名册常驻，点击卡片即切换对话对象；导演卡在顶部，金色描边表示主控。
- 右侧「现场」是原来黑板/时间线的升级：任务进程置顶，黑板保留 5 分区（导演/资产/草稿/审查/计划），时间线保留。
- 顶部 run 选择器保留；「巡检」按钮一键让导演看现场。
- 窄屏：右栏移到对话下方，花名册变成横向 chips；对话始终可见。

### 5.5 空态与快捷指令

无消息时显示「剧组已就位」空态 + 四个快捷指令：

- 巡检全书状态
- 让管理补齐资产
- 让编审复审最新一章
- 让主创续写下一拍

点击快捷指令等同于发送一条用户消息。空态下方显示「最近一次运行」摘要（run 状态/阶段/时间）。

### 5.6 与语料的联动

- 语料详情「交给导演」→ 切到代理工作室，预填消息并附带 `corpus_ids`。
- 导演简报带语料摘要；派发管理时把 `corpus_ids` 写进任务，管理可用 `corpus_read` 读取全文。
- 聊天消息下方显示「引用语料：老城区雨夜 · 城市更新」的小卡片，可点击回到语料详情。
- 语料页「已用于创作」状态在 `corpus_link` 后更新，聊天/资产页都能看到来源。

---

## 6. 任务注册表与进程检视

### 6.1 为什么需要

现有可观测性只有：

- `agency_runs`：粗粒度 run 状态/阶段。
- `agency_activity_log`：角色 start/done 和 run 进度。
- 内存取消 flag / 在途 LLM 请求表（模块私有，无只读 API）。

看不到：某个子 agent 正在跑第几轮、最后一次心跳、用了哪个工具、失败原因、父子任务关系。导演要「检视子 agent 进程、修复错误」，必须先有这张表。

### 6.2 表结构

**6.2.1 `agency_runs.lane` 与索引替换**

```sql
ALTER TABLE agency_runs ADD COLUMN lane TEXT NOT NULL DEFAULT 'write';
DROP INDEX IF EXISTS idx_agency_runs_one_active_per_story;
CREATE UNIQUE INDEX idx_agency_runs_one_active_per_lane
ON agency_runs(story_id, lane) WHERE status IN ('pending', 'running');
```

- `write`：创世/续写/批量/资产刷新。P0 保持同 story 只有一条活跃 write run。
- `review`：独立复审/质量门，可与 write 并行。
- `inspect`：导演巡检，可与 write/review 并行。
- `chat`：剧组对话。P0 的聊天不建 run，任务用 `chat:<session_id>` 作为 `run_id` 字符串；若 P1 需要 run 级取消/快照，再建 `lane=chat` 的 run。

存量代码映射（实现时按此改 `AgencyRun::new` / 各 run 创建点）：

| 创建点 | lane |
|---|---|
| `run_genesis_inner` | `write`（story_id 先 NULL，V149 索引豁免） |
| `run_continue_inner` / `run_batch_inner` / `resume_prepare` / `run_asset_refresh` | `write` |
| `run_role_task`：Producer / LeadWriter / Writer / OutlinePlanner | `write` |
| `run_role_task`：EditorAuditor / Inspector | `review` |
| `run_role_task`：Director | `inspect` |
| `observe::find_or_create_observe_run` | `inspect`（status 仍 `idle`，不进 lane 索引） |
| Director 巡检 / 聊天 turn / 子 agent task | 不建 run，`run_id = chat:<session_id>` 或当前 write run id |

**6.2.2 任务表**

```sql
CREATE TABLE agency_agent_tasks (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,           -- 真实 run_id 或 chat:<session_id>
  story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  role TEXT NOT NULL,             -- director/producer/editor_auditor/lead_writer/...
  parent_task_id TEXT,
  task TEXT NOT NULL,             -- 任务指令（截断 2000 字）
  status TEXT NOT NULL,           -- queued | running | waiting | cancelling | completed | failed | cancelled | stale
  phase TEXT,
  turn_count INTEGER NOT NULL DEFAULT 0,
  tool_calls INTEGER NOT NULL DEFAULT 0,
  last_tool TEXT,
  heartbeat_at TEXT,
  started_at TEXT,
  finished_at TEXT,
  error TEXT,
  result_summary TEXT,
  meta_json TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_agency_tasks_story_status ON agency_agent_tasks(story_id, status, heartbeat_at);
CREATE INDEX idx_agency_tasks_run ON agency_agent_tasks(run_id, created_at);
CREATE INDEX idx_agency_tasks_parent ON agency_agent_tasks(parent_task_id);

CREATE TABLE agency_director_actions (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  session_id TEXT,
  message_id TEXT,
  run_id TEXT,
  action_type TEXT NOT NULL,       -- dispatch/retry_task/.../escalate
  target_role TEXT,
  target_id TEXT,
  params_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL,            -- proposed | approved | executing | done | failed | rejected
  reason TEXT,
  result_summary TEXT,
  error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_director_actions_story ON agency_director_actions(story_id, created_at);

CREATE TABLE story_resource_leases (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  resource TEXT NOT NULL,          -- chapter_sequence / scene:<id> / assets:<group> / draft:<key> / gate:<chapter>
  owner_run_id TEXT,
  owner_task_id TEXT,
  acquired_at TEXT NOT NULL,
  heartbeat_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  UNIQUE(story_id, resource)
);
CREATE INDEX idx_story_leases_expiry ON story_resource_leases(expires_at);
```

**6.2.3 调度器与异步派发**

- `TaskScheduler`：按角色限流（Director 1 / Writer 1 / Producer 2 / Editor 2），每个 story 再限总并发（默认 4）。`dispatch` 只做 `spawn` + 任务落库，立即返回 `task_id`。
- 任务完成/失败/进入 `waiting` 都发 `agency-crew-update`；导演的 follow-up 由 `TaskScheduler` 的完成回调触发，不阻塞原聊天回合。
- 写任务在提交前 `LeaseManager::acquire(story_id, resource, run_id, task_id, ttl)`；拿不到 → `status=waiting`，事件通知，租约释放后重试一次。
- `DIRECTOR_LLM_SEM` 单独 1 个许可，保证子 agent 跑满时导演的巡检/汇报仍能拿到模型；`AGENCY_GLOBAL_LLM_SEM` 从固定 3 改为可配置（默认 4），并保留至少 1 个给导演。`AgencyBudget` 另加 `director_sem`，供 `run_role_task(Director)` 使用；Director turn 走全局 `DIRECTOR_LLM_SEM`。
- 后台 ingest/审计继续走 `BACKGROUND_LLM_SEMAPHORE`，与用户发起的剧组任务分队列；不要让他们抢同一个槽位。

### 6.3 埋点与心跳

在 `run_role_loop` / `run_sub_role` 外包一层 `TaskReporter`：

```
创建任务行（running, heartbeat_at=now）
  → 每次 LLM 调用前 heartbeat("llm")
  → 每次 LLM 调用后 heartbeat("llm:done")
  → 工具循环结束：completed/failed/cancelled + turn_count/tool_calls/result_summary
```

实现方式：

- 新增 `ReportingLlm` 包装 `BudgetedLlm`，在 `complete()` 前后更新心跳；不侵入 `ToolLoop`。
- `ToolLoop` 增加可选的 `with_cancel(Arc<AtomicBool>)`，每轮开始检查，支持协作式取消。
- `run_role_loop` 增加 `TaskReporter` 参数（或新增 `run_role_loop_reported` 包装，避免改所有既有调用点）。
- 子 agent 统一走 `TaskScheduler::dispatch`：任务落库 → `tokio::spawn` → `TaskReporter` → 完成回调；不再走 `run_role_task` 的「一任务一 run」路径。
- 单次 LLM 调用阶段（concept_pack、writer_first_chapter、gate verdict）不强制进任务表；它们仍通过 `agency_activity_log` 和 run phase 可见。P1 再逐步补齐。

心跳写入用 `spawn_blocking` fire-and-forget；**任务结束写库要 await**（吸取 v0.73.11 手法落库竞态的教训），确保测试和进程退出前状态落盘。

### 6.4 卡住/失败判定

- `running` + `heartbeat_at` > 90s：`SlowTask`（warn）。
- `running` + `heartbeat_at` > 300s：`StaleTask`（high）。
- `running` 但所属 run 已 `completed/failed/cancelled`：`OrphanTask`（warn）。
- `failed` 且没有后续重试：`FailedTask`（high）。
- 启动时 `reap_stale_tasks(threshold=300s)` 把超过阈值的 `running` 标记 `stale`，并写一条 `agency-crew-update`（无 app handle 时只写库）。
- 阈值可配置（设置页/常量），默认值参考单次 LLM 最长 155s 的真实日志。

### 6.5 取消

- `agency_crew_cancel_task(task_id)`：设置内存 cancel token + DB `status=cancelling`；`ToolLoop` 下一轮检查到后退出并写 `cancelled`。
- 在途 LLM 请求不保证立刻中断；ToolLoop 在下一轮开始前退出。UI 显示「正在停止…」。
- `cancel_run` 沿用现有 `cancel_agency_run`，并级联取消该 run 下所有 running 任务。

### 6.6 开机清理

- `reap_stale_tasks` 在 `lib.rs` 启动任务里调用（与现有 activity log 剪枝并列）。
- 不删除任务历史；保留 30 天，与 `agency_activity_log` 一致。

---

## 7. 数据迁移与清理

### 7.1 V149 迁移

新增 **Rust 迁移** `src-tauri/src/db/migrations/V149__corpus_and_director.rs`（在 `db/migrations/mod.rs` 声明并加入 `all_rust_migrations()`）。不用 SQL 文件的原因：SQL runner 逐条执行、会跳过 `BEGIN/COMMIT`，无法保证「加 lane → 删旧索引 → 建新索引」原子完成；Rust 迁移可以 `conn.transaction()` 一次提交。

- `corpus_items`
- `corpus_links`
- `agency_chat_sessions`
- `agency_chat_messages`
- `agency_agent_tasks`
- `agency_director_actions`
- `story_resource_leases`
- `agency_runs` 增加 `lane`；删除 `idx_agency_runs_one_active_per_story`，建立 `idx_agency_runs_one_active_per_lane(story_id, lane)`。

所有 `story_id` 外键 CASCADE；`run_id` 不建外键（聊天任务用合成键）。迁移前先把存量 `pending/running` run 的 `lane` 回填为 `write`；`ALTER TABLE ... ADD COLUMN lane TEXT NOT NULL DEFAULT 'write'` 会自动完成这一步。索引替换必须放在同一迁移里，避免出现「旧索引已删、新索引未建」的窗口。

### 7.2 故事删除

`StoryRepository::delete` 前：

- **故事删除顺序**：先查询该故事所有语料 `file_path`，执行 `StoryRepository::delete`（FK CASCADE 清行）；DB 删除成功后再删除物理文件（`app_data_dir/corpus/<story_id>/` 整个目录）。这样 DB 删除失败不会提前丢文件。
- 依赖 FK CASCADE 删除 `corpus_items`、`corpus_links`、`agency_chat_*`、`agency_agent_tasks`、`agency_director_actions`、`story_resource_leases`。
- `agency_chat_messages` 通过 session CASCADE。
- 把 `corpus_items`、`agency_chat_sessions`、`agency_agent_tasks`、`story_resource_leases` 加入 `story_repository.rs` 的 orphan 检查数组。

### 7.3 前端缓存

- `['corpus', storyId]` 在切故事/删故事时 invalidate/remove（`App.tsx`、`useSyncStore.ts`）。
- `['agency-chat', storyId]`、`['agency-crew', storyId]` 同理。
- 删除故事后清空 `agencyChatStore` 与 `agencyCrewStore` 的当前会话。

---

## 8. 后端命令与事件

### 8.1 命令（导演/剧组）

| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `agency_chat_send` | `story_id, session_id?, message, target_role?, corpus_ids?` | `{session_id, message_id, turn_task_id}` | 立即返回；导演 turn 与子 agent 任务全部异步，子 task_ids 通过 `agency-chat-update` 到达 |
| `agency_chat_sessions` | `story_id` | `Vec<ChatSessionSummary>` | 会话列表 |
| `agency_chat_messages` | `session_id, limit?, before?` | `Vec<ChatMessage>` | 历史消息 |
| `agency_chat_new_session` | `story_id, title?` | `ChatSessionSummary` | 新建会话 |
| `agency_chat_delete_session` | `session_id` | `()` | 删除会话 |
| `agency_crew_status` | `story_id` | `CrewSnapshot` | 巡检快照（只读，含 lane/lease） |
| `agency_crew_dispatch` | `story_id, role, instruction, run_key?, corpus_ids?` | `{task_id}` | 手动派发子 agent task（异步） |
| `agency_crew_cancel_task` | `task_id` | `()` | 协作式取消 |
| `agency_crew_retry_task` | `task_id` | `{task_id}` | 重试失败任务 |
| `agency_crew_release_lease` | `story_id, resource` | `()` | 导演裁决后强制释放过期/孤儿租约（需确认） |
| `agency_director_execute` | `action_ids: Vec<String>` | `{dispatched_task_ids}` | 执行已确认动作，异步派发 |
| `agency_director_reject` | `action_ids: Vec<String>` | `()` | 拒绝动作 |
| `agency_director_patrol` | `story_id` | `CrewSnapshot` | 手动巡检（不调 LLM，不建 run） |

新增命令全部注册到 `handlers.rs` 的 Agency 段，并在 `tests/ipc_registry_contract_test.rs` 加断言。

### 8.2 事件

| 事件 | 方向 | 说明 |
|---|---|---|
| `agency-chat-update` | Rust → 前端 | 消息状态/阶段/内容更新（含 `task_ids`） |
| `agency-crew-update` | Rust → 前端 | 任务创建/心跳/结束/`waiting` |
| `agency-lease-update` | Rust → 前端 | 资源租约 acquire/release/expire |
| `agency-director-action` | Rust → 前端 | 动作审批/执行结果 |
| `agency-agent-activity` | 既有 | 角色 start/done，继续保留 |
| `agency-run-progress` | 既有 | run 阶段，继续保留 |
| `agency-board-changed` | 既有 | 黑板变化，继续保留 |

前端监听统一放在 `App.tsx` 常驻 effect，写入 `agencyChatStore` / `agencyCrewStore`，不要放在页面组件内（页面卸载会丢事件，这是 v0.70 已修过的坑）。

---

## 9. 前端改动

### 9.1 路由与侧栏

- `ViewType` 增加 `'corpus'`。
- `App.tsx`：
  - import `Corpus`，`renderView` 增加 `case 'corpus'`。
  - 故事切换/窗口恢复时 invalidate `['corpus', storyId]`、`['agency-chat', storyId]`、`['agency-crew', storyId]`。
  - 常驻监听三个新事件。
- `Sidebar.tsx`：故事资产组增加 `{ id: 'corpus', label: '语料', icon: Library, impact: 'warm' }`（图标可用 `Library` / `Files`）。
- `App.tsx` 的 `NavigateTo` 已接受 `ViewType`，语料页可被幕前/其他页面跳转。

### 9.2 新页面与组件

| 文件 | 作用 |
|---|---|
| `pages/Corpus.tsx` | 语料列表/筛选/搜索/详情/导入 |
| `components/corpus/CorpusCard.tsx` | 卡片（文本/图片/链接） |
| `components/corpus/CorpusDetail.tsx` | 原文 + AI 提炼 + 转资产 + 交给导演 |
| `components/corpus/CorpusImportDialog.tsx` | 文本/链接/文件/图片四 Tab |
| `components/corpus/CorpusPromoteDialog.tsx` | 目标类型草稿预览/跳转现有编辑入口 |
| `hooks/useCorpus.ts` | TanStack Query：list/get/create/update/delete/import/distill/promote/link |
| `pages/AgencyStudio.tsx` | 改造为三栏工作台 |
| `components/agency/CrewRoster.tsx` | 4 张角色卡 + 状态 + 对话入口 |
| `components/agency/CrewChat.tsx` | 消息流 + 输入条 + 快捷指令 + 审批卡 |
| `components/agency/CrewChatMessage.tsx` | 用户/导演/子 agent 消息、阶段轨迹、语料引用 |
| `components/agency/CrewLivePanel.tsx` | 任务进程 + 告警 + 操作 |
| `components/agency/CrewBoard.tsx` | 5 分区黑板 |
| `components/agency/CrewTimeline.tsx` | 现有时间线抽取 |
| `stores/agencyChatStore.ts` | 当前会话、消息、阶段、草稿输入、pending 语料 |
| `stores/agencyCrewStore.ts` | 任务快照、告警、轮询/事件合并 |
| `services/api/corpus.ts` | 语料命令封装 |
| `services/api/agency.ts` | 增加 chat/crew/director 命令与类型 |

### 9.3 UI 复用

- 输入条：`AiPromptBar`（受控；支持 `/` 命令和 `@` 数据源）。
- 流式呈现：`AiStreamingText`（导演回复本地逐字）；`AiThinking`（阶段轨迹）。
- 审批：`AiApprovalCard` 或专用 `DirectorApprovalCard`（动作摘要、影响范围、确认/取消）。
- 任务行：`AiTaskRows`（现场任务列表）；`AiToolChips`（工具/动作 chips）。
- 黑板：现有 `AiContextCards`；5 分区横向滚动或 2 行网格。
- 空态：`EmptyHint`。
- 令牌：只用 `--ai-*` 与 `--cinema-*` 语义令牌，不写死颜色；图标用 `lucide-react`。
- 动效：阶段切换用现有 `ai-fade-up`/`ai-pop-in`；遵守 `prefers-reduced-motion`。

### 9.4 交互细节

- 输入框 placeholder：「对导演说，或 @管理/@编审/@主创 直接下指令…」。
- 发送后立即插入用户消息；导演消息先以 `thinking` 气泡出现，显示阶段文案：
  - 「正在检视剧组…」
  - 「正在判断…」
  - 「正在派发：管理」
  - 「正在观察：编审」
  - 「正在整理报告…」
- 阶段轨迹用 `AiThinking` 可折叠展示；完成后折叠为「3 个动作 · 12s」。
- 动作卡：`派发 管理 · 补齐世界观`、`重试 编审 · 第 12 章复审`；点击展开参数与结果。
- 审批卡：标题、动作列表、影响范围、预计耗时、[确认执行] [取消]。
- 错误：消息内联错误 + 「重试」/「让导演处理」。
- 会话切换：顶部下拉；支持新建/删除；最近会话按更新时间排序。
- 轮询：聊天消息 5s（事件补充实时）；任务现场 3s；黑板 10s（保持现状）。

---

## 10. 兼容性与风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| V109 索引替换 | 迁移窗口内旧索引已删、新索引未建 | 同一迁移内先 `ALTER` 回填 `lane`，再 `DROP` + `CREATE`；事务包裹 |
| 导演/子 agent 抢模型 | 续写变慢/超时 | `director_sem` 预留 1 个；`AGENCY_GLOBAL_LLM_SEM` 可配置默认 4；子 agent 按角色限流 |
| 资源租约死锁/过期 | 写任务卡住 | 租约带 TTL + heartbeat；过期回收；导演可裁决释放；重试一次 |
| 后台任务抢槽位 | 收尾/审计延迟 | 用户任务与 `BACKGROUND_LLM_SEMAPHORE` 分队列；用户任务优先 |
| 长 LLM 调用（155s） | 心跳误判卡住 | 阈值 300s；心跳在调用前后写；UI 显示「等待模型」而非「卡住」 |
| 事件丢失/窗口隐藏 | 聊天状态停在 thinking | 消息/任务落库 + 轮询 + 页面重开恢复 |
| 图片太大 | 内存/传输 | 10 MB 限制；详情懒加载；列表不加载原图 |
| 覆盖作者手写资产 | 数据丢失 | 导演无直接写权限；子 agent 提示词「只补空」；覆盖类动作强制确认 |
| 中文 prompt 变量残留 | registry fail-closed 测试红 | director prompt 只声明 `premise`；动态内容放 user prompt |
| 旧数据兼容 | 迁移失败 | V149 只建新表；不修复旧表；幂等 `IF NOT EXISTS` |
| 任务表膨胀 | DB 变大 | 30 天剪枝；仅记录最近任务；`result_summary` 截断 |
| Windows 路径/文件名 | 文件导入失败 | 使用 UUID 文件名；`PathBuf` 拼接；不使用中文路径做 key |
| 测试竞态 | CI flaky | 任务结束 await 落库；测试用固定时间/阈值；不依赖事件顺序 |

---

## 11. 分阶段实施与验收

### P0：闭环（建议 v0.74.0）

**并发底座（必须最先做）**
- V149：`agency_runs.lane` + `(story_id, lane)` 部分唯一索引，替换 V109 的单 story 索引。
- `agency_agent_tasks` + `TaskReporter` + `TaskScheduler`：子 agent 异步派发、心跳、状态机、取消、开机清理。
- `story_resource_leases` + `LeaseManager`：先用于 `materialize_assets`、章节序号、门禁写入；P0 不开放多 write 并行，但表与接口就位。
- `run_role_task` 保留给独立外部任务；导演/聊天/子 agent 一律走 `TaskScheduler`，不再「一任务一 run」。
- 全局闸门：新增 `director_sem`；`AGENCY_GLOBAL_LLM_SEM` 可配置（默认 4）；用户任务与后台 ingest 分队列。

**语料**
- V149 迁移 + `CorpusRepository` + 命令注册。
- 语料页：列表/筛选/搜索/详情/导入（文本、链接、文件、图片）/删除。
- AI 提炼 + 转为资产草稿（预填现有编辑入口）+ `corpus_link`。
- 文件存储/清理、故事删除级联、orphan 检查。
- agency 角色只读工具 `corpus_search`/`corpus_read`。

**导演 + 对话**
- `AgentRole::Director` 全链路（枚举/spec/prompt/context_label/budget/前端角色名/黑板 direction 分区）。
- `crew.rs` 巡检快照 + 告警 + lane/lease 视图；`director.rs` 计划/派发/报告；动作目录与确认策略。
- `agency_chat_*` 会话/消息/命令；`chat:<session>` 任务分组键（不建 write run）。
- 导演派发异步：`agency_chat_send` 立即返回 `turn_task_id`；子任务 `task_ids` 通过 `agency-chat-update` 到达；完成回调追加 follow-up 消息。
- 代理工作室三栏改造：花名册 + 对话 + 现场（任务/告警/黑板/时间线）。
- 子 agent 直达（@/卡片切换）；快捷指令；审批卡；重试/取消。
- 事件监听提升到 `App.tsx`；stores。

**验收**
- 新建语料（文本/图片）→ 列表可见 → 提炼 → 转为角色草稿 → 保存后 links 有记录 → 删故事后语料与文件消失。
- 代理工作室打开即有对话；发「巡检」返回现场摘要；发「让管理补齐世界观」**立即返回 `turn_task_id`**，随后 `agency-chat-update` 带子任务 `task_ids`，管理任务（queued→running→done）事件到达后追加 follow-up，并更新黑板。
- 发「让编审复审第 12 章」时，如果当前有 write run 在跑，编审 task 仍能执行（review lane/任务不与 write 互斥）；现场同时显示 write 与 review 两条任务。
- 注入失败任务（测试或手动改 DB status）→ 现场出现告警 → 重试成功 → 告警消失。
- 资源租约冲突：模拟两个 `materialize_assets` 同时提交 → 第二个进入 `waiting`，第一个释放后自动重试成功。
- 覆盖类动作弹审批卡；取消不执行；确认后异步派发并落 `agency_director_actions`。
- 刷新/重开页面，会话与消息恢复；切故事不串数据。
- 既有功能回归：创世、续写、观察、评估页、学习页不回归；V109→lane 索引迁移后旧 run 仍可查询。
- Rust 单测：lane 索引/旧索引替换、corpus 仓储/迁移/级联、director plan 解析、动作确认策略、任务状态机/卡住判定/异步派发、租约 acquire/expire/释放、聊天持久化、IPC 契约。
- 前端 vitest：语料页筛选/详情/导入、聊天 store 合并、`acting → done` 异步消息、角色路由、审批卡、空态快捷指令。
- Playwright：代理工作室发消息立即返回、语料导入文本、切换到语料页。

### P1：增强

- **多 write 并行**：`story_resource_leases` 全面启用 + 章节号原子分配（事务内 `MAX+1` 或 `story_sequences`），允许两章/两个写任务同时跑，只在提交点串行。
- 后台任务与用户任务分队列；`AGENCY_GLOBAL_LLM_SEM`、角色许可进设置页。
- `pinned` 语料进入续写上下文（1200 字预算 + 测试）。
- 聊天 token 级流式（`llm_generate_stream` 或 ToolLoop delta）。
- 会话滚动摘要（超过 12 轮自动压缩）。
- 导演自动巡检：run 失败后自动生成告警与建议（不自动执行）。
- 语料语义检索（向量库）。
- 任务表按角色/状态筛选，历史任务页；租约历史视图。

### P2：进阶

- 图片视觉理解/OCR（接视觉模型）。
- 导演定时巡检与自动修复（可配置开关、白名单动作）。
- 语料与知识图谱自动关联、冲突检测。
- 多会话并行（同一故事多个剧组会话，需要重新评估预算/信号量）。

---

## 12. 已确认事项（2026-10-30）

1. **导演默认模型档**：已确认 `Tool`（快、便宜）；需要更强判断时可在模型映射里改 `Creative`。
2. **语料作用域**：已确认按故事隔离；全局语料库 + 关联表放 P1。
3. **图片预览**：已确认 P0 用命令返回 data URL（不改 CSP）；图片量大再上 scoped asset protocol。
4. **导演自主度**：已确认「只读巡检 + 重试/物化/登记质量债可直接执行；覆盖/取消/续写/批量必须确认」。
5. **版本节奏**：已确认为 P0 一个版本（v0.74.0）；P1 另开版本。
6. **并发模型**：已确认改为 **lane + task + lease + 异步派发**。P0 保留「一个 story 一个 write lane」作为写入事务边界，`review`/`inspect`/`chat` 并行；P1 用资源租约 + 原子章号开放多 write 并行。
7. **V109 处理**：不直接删除，升级为 `(story_id, lane)` 部分唯一索引；`write` 仍单活跃，避免重复章节号/资产覆盖。

---

## 附录 A：新增/修改文件清单（预估）

**Rust 新增**
- `src-tauri/src/db/migrations/V149__corpus_and_director.rs`（Rust 迁移，事务内完成 lane 回填 + 索引替换 + 建表）
- `src-tauri/src/corpus/mod.rs`
- `src-tauri/src/corpus/repository.rs`
- `src-tauri/src/corpus/service.rs`
- `src-tauri/src/corpus/commands.rs`
- `src-tauri/src/agency/director.rs`
- `src-tauri/src/agency/chat.rs`
- `src-tauri/src/agency/crew.rs`（巡检/告警）
- `src-tauri/src/agency/scheduler.rs`（异步派发/角色限流/完成回调）
- `src-tauri/src/agency/lease.rs`（资源租约 acquire/heartbeat/expire/release）
- `src-tauri/src/agency/roles/director.rs`
- `resources/prompts/agency/agency_director_system.md`

**Rust 修改**
- `agency/models.rs`（Director、BoardZone::Direction、AgencyRun.lane、Task/Lease/Chat/DirectorAction 结构）
- `agency/roles.rs`（spec_for + 测试）
- `agency/coordinator.rs`（context_label、run_sub_role、run lane 映射、任务埋点接入）
- `agency/budget.rs`（director_sem、全局闸门可配置）
- `agency/tools.rs`（crew_status/corpus_search/corpus_read 白名单）
- `agency/repository.rs`（lane、任务/会话/动作/租约 CRUD，或拆到新模块）
- `agency/commands.rs`（chat/crew/director 命令）
- `agency/observe.rs`（observe run lane=inspect）
- `llm/service.rs`（agency_director 路由）
- `handlers.rs`（命令注册）
- `db/repositories/story_repository.rs`（语料文件清理 + orphan）
- `db/migrations/mod.rs`（若 V149 用 Rust 迁移则注册；SQL 不需要）
- `lib.rs`（模块声明、reap_stale_tasks/reap_expired_leases 启动任务）
- `prompts/registry.rs`（Director prompt 测试覆盖）

**前端新增**
- `src-frontend/src/pages/Corpus.tsx`
- `src-frontend/src/components/corpus/*`
- `src-frontend/src/components/agency/*`
- `src-frontend/src/hooks/useCorpus.ts`
- `src-frontend/src/services/api/corpus.ts`
- `src-frontend/src/stores/agencyChatStore.ts`
- `src-frontend/src/stores/agencyCrewStore.ts`

**前端修改**
- `types/index.ts`（ViewType + 语料/聊天类型）
- `App.tsx`（路由、事件、缓存）
- `components/Sidebar.tsx`（语料入口）
- `pages/AgencyStudio.tsx`（三栏改造）
- `stores/agencyActivityStore.ts`（AGENCY_ROLES 加导演）
- `services/api/agency.ts`（chat/crew 类型与封装）

**测试**
- `src-tauri/src/corpus/tests.rs` 或 `#[cfg(test)]` 模块
- `src-tauri/src/agency/tests.rs`（导演计划/动作/任务状态/异步派发/lane/租约）
- `src-tauri/src/agency/repository.rs`（lane 索引替换、任务状态机、租约过期）
- `src-tauri/src/tests/ipc_registry_contract_test.rs`（新命令注册）
- `src-frontend/src/pages/__tests__/Corpus.test.tsx`
- `src-frontend/src/components/agency/__tests__/*.test.tsx`
- `src-frontend/src/stores/__tests__/agencyChatStore.test.ts`
- `e2e/agency-studio.spec.ts`、`e2e/corpus.spec.ts`

**文档**
- `README.md`、`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`（按项目强制规则）

---

## 附录 B：关键不变量（实现时必须钉住）

1. 聊天/巡检不得创建 `write` run；`chat:<session_id>` 只用于活动日志/任务/黑板分组。
2. 顶层 run 按 `lane` 唯一：同 story 同 lane 仅一个 `pending/running`；`write` 仍单活跃，`review`/`inspect`/`chat` 可并行。
3. 子 agent 必须异步派发：`dispatch` 立即返回 `task_id`，不得在聊天回合里 `await` 子 agent 完成；完成靠事件回执 + follow-up 消息。
4. 写库提交点必须 acquire `story_resource_leases`；读操作不加锁。租约带 TTL + heartbeat，过期可回收。
5. 导演不得直接调用 `board_write`/`board_revise`；覆盖作者内容必须确认。
6. 任务结束写库必须 await；心跳可 fire-and-forget。
7. 事件监听必须在 `App.tsx` 常驻；页面内监听会丢事件。
8. 语料文件路径必须在 `app_data_dir/corpus/<story_id>/` 下；删除故事必须清目录。
9. 所有新命令必须进 `handlers.rs` + IPC 契约测试；前端调用必须能被 `verify-ipc-manifest.py` 找到。
10. 所有新 UI 只用 `--ai-*` 语义令牌；新增提示词必须通过 `prompts/registry.rs` 的 fail-closed 占位符测试。
11. V149 必须在同一迁移里原子替换 V109 索引（先回填 `lane`，再删旧建新），不允许出现无索引窗口。
12. 导演的规划/汇报调用必须先拿 `DIRECTOR_LLM_SEM` permit（子 agent 跑满时不得饿死导演）；`run_role_task(Director)` 走 `AgencyBudget.director_sem`。
