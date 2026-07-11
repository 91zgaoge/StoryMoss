# StoryMoss 完善与优化计划

## Context

基于对 StoryMoss 代码库的深度审计，项目在功能层面极其丰富（AI 辅助长篇小说创作的完整链路），但积累了显著的技术债务。核心问题包括：前端内存泄漏、后端命令层单体化（3445 行的 story_commands.rs）、双前端系统并存、全局状态管理粗糙、状态同步缺口、构建工具链缺失。本计划旨在分阶段、有依赖地清理债务，提升稳定性、可维护性和开发效率。

**项目规模参考**：
- Rust 后端：~45 模块，197 个 Tauri 命令，263 个测试函数
- 前端：42 组件、24 页面、42 个 hooks、5 个服务
- 构建产物：双窗口（backstage + frontstage），无代码分割

---

## Phase 1: 安全与稳定性修复（P0）

**目标**：消除已知稳定性风险，清理死代码

### 1.1 修复 FrontstageApp.tsx 内存泄漏
**文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`
**问题**：Tauri 事件监听器注册后未保存 `unlisten` 返回值，组件卸载时无法清理
**实施**：
- 为每个 `listen()` 调用保存 `UnlistenFn`
- 在 `useEffect` cleanup 中调用所有 `unlisten()`
- 对 async setup 使用 ref 追踪 mounted 状态，防止组件卸载后仍调用 setState
- 验证：通过 React DevTools Profiler 反复挂载/卸载 FrontstageApp，确认内存不再增长

### 1.2 删除旧前端死代码
**文件**：`src/main.js`、`src/views.js`，以及 `src/` 目录下所有旧前端资源
**问题**：约 2400 行已死代码，构建流程不走这些文件，但会导致开发者混淆
**实施**：
- 确认 `src-frontend/dist` 是唯一前端产物（tauri.conf.json 验证）
- 删除 `src/main.js`、`src/views.js`
- 检查 `src/` 目录是否还有其他被引用的文件（HTML 模板、CSS 等）
- 如果 `src/` 目录下无其他有用文件，整目录删除
- 验证：执行 `npm run build` + `cargo build` 确认无构建错误

### 1.3 清理 lib.rs 空白行与死代码
**文件**：`src-tauri/src/lib.rs`
**问题**：524-771 行约 240 行纯空白行；`detect_and_route_intent` 是完全的死代码（始终返回 None）
**实施**：
- 删除所有连续多余空白行
- 删除 `detect_and_route_intent` 函数及其所有调用点
- 删除 `is_novel_creation_intent` 函数（硬编码关键词匹配，无调用点或可被替换）
- 验证：`cargo check` 通过

---

## Phase 2: 后端命令层拆分（P1）

**目标**：拆分 story_commands.rs 单体文件，建立清晰的命令层边界
**依赖**：Phase 1 完成（确保无死代码干扰重构）

### 2.1 拆分 story_commands.rs
**文件**：`src-tauri/src/story_commands.rs`（3445 行）
**拆分策略**（按领域）：

| 新文件 | 来源命令 | 行数估计 |
|--------|----------|----------|
| `scene_commands.rs` | scene CRUD, scene versions, scene annotations | ~800 |
| `worldbuilding_commands.rs` | world building, world rules, settings | ~300 |
| `style_commands.rs` | writing styles, style DNA, style blend | ~400 |
| `foreshadowing_commands.rs` | foreshadowing tracker, payoff ledger | ~350 |
| `kg_commands.rs` | knowledge graph entities/relations | ~300 |
| `character_commands.rs` | character states, relationships | ~250 |
| `comment_commands.rs` | comment threads, change tracking | ~300 |
| `studio_commands.rs` | studio config, import/export | ~200 |
| `analytics_commands.rs` | writing analytics, reading power | ~250 |

**实施**：
- 逐领域提取，保持命令签名不变
- 更新 `handlers.rs` 的 import 列表
- 每提取一个文件，运行 `cargo check` 验证
- 保留原始文件直到全部拆分完成，最后删除

### 2.2 统一错误处理模式
**范围**：`src-tauri/src/commands/` 目录下的 legacy 文件
**问题**：两套错误模式并存：`Result<T, String>`（旧）vs `Result<T, AppError>`（新）
**实施**：
- 将 `commands/story.rs`、`commands/chapter.rs`、`commands/character.rs`、`commands/export.rs`、`commands/memory.rs` 等文件从 `Result<T, String>` 迁移到 `Result<T, AppError>`
- 统一使用 `map_err(AppError::from)` 替代 `.map_err(|e| e.to_string())`
- 更新前端对应的 error handling（如果前端依赖了特定错误格式）
- 验证：编译通过 + 前端错误提示正常

### 2.3 标准化状态注入模式
**范围**：所有仍使用 `get_pool()` 全局状态的命令文件
**问题**：约 50 处 `get_pool()` 调用仍在 legacy commands 中使用
**实施**：
- 将 `commands/story.rs`、`commands/chapter.rs`、`commands/character.rs`、`commands/memory.rs`、`commands/export.rs` 等改为 `State<'_, DbPool>` 注入模式
- 删除 `lib.rs` 中 `get_pool()` 函数（前提是所有调用点已迁移）
- 将 `APP_CONFIG` 全局变量替换为 Tauri State 注入
- 验证：编译通过 + 命令正常执行

---

## Phase 3: 状态同步与前端优化（P1-P2）

**目标**：消除状态同步缺口，优化前端性能和架构
**依赖**：Phase 2 完成（命令层稳定后处理同步逻辑）

### 3.1 补齐状态同步事件
**范围**：所有修改数据但不发射 state sync 的命令
**问题**：审计发现 20+ 个命令修改数据后前端不会自动刷新
**实施**：
- 在 `commands/skill.rs`、`commands/export.rs`、`commands/story_system.rs`、`commands/intent.rs` 等文件的数据修改命令末尾添加 `StateSync::emit_data_refresh()` 调用
- 在拆分后的 `comment_commands.rs`、`character_commands.rs` 等文件中统一添加事件发射
- 建立 checklist：每个 command 文件逐一检查 mutation 是否配对了 sync event
- 验证：从前端触发操作，确认另一窗口/标签页自动刷新

### 3.2 优化 Query 缓存刷新策略
**文件**：`src-frontend/src/App.tsx`、`src-frontend/src/hooks/useSyncStore.ts`
**问题**：`currentStory` 变化时顺序触发 8 次 `invalidateQueries`，造成查询雪崩
**实施**：
- 使用 `queryClient.cancelQueries()` 取消过时的 inflight 请求
- 将多个 `invalidateQueries` 调用合并为单个 `invalidateQueries({ queryKey: ['story-data', storyId] })`，利用 TanStack Query 的 query key hierarchy
- 或在 `useSyncStore` 中使用 debounce 批量处理 invalidate
- 验证：Network/Performance tab 中故事切换时请求数量减少

### 3.3 移除 DOM Hack（forceRedraw）
**文件**：`src-frontend/src/App.tsx`
**问题**：`opacity: 0.99` 抖动 hack 用于强制 WebView 重绘
**实施**：
- 记录当前 Tauri/WebKit 版本和具体复现条件
- 尝试升级 Tauri 到最新 patch 版本，看是否已修复
- 若必须保留，将 hack 封装为可复用的 `useWebViewRedrawFix()` hook，集中管理
- 验证：窗口恢复/切换时内容正确渲染

---

## Phase 4: 构建工具链与代码质量（P2）

**目标**：建立一致的代码风格和质量门禁

### 4.1 添加 Rust 格式化与 lint 配置
**实施**：
- 创建 `rustfmt.toml`：设置 `max_width = 100`、`edition = "2021"`、`use_small_heuristics = "Default"`
- 创建 `.clippy.toml` 或在 `Cargo.toml` 中配置 clippy lint 级别
- 在 CI 中添加 `cargo fmt --check` 和 `cargo clippy -- -D warnings` 步骤
- 一次性运行 `cargo fmt` 格式化整个代码库（会产生大量 diff，建议单独 PR）
- 逐步修复 clippy warnings（不要一次全改，分文件处理）

### 4.2 添加前端格式化配置
**实施**：
- 创建 `.prettierrc`：配置 printWidth、tabWidth、trailingComma、arrowParens
- 创建 `eslint.config.mjs`（ESLint v9 flat config）替代旧的命令行配置
- 在 `package.json` 中添加 `format` 和 `format:check` 脚本
- 运行 Prettier 格式化整个 `src-frontend/src/`（单独 PR）

### 4.3 配置 Vite 代码分割
**文件**：`src-frontend/vite.config.ts`
**问题**：无 manualChunks，所有依赖打包到单一 chunk
**实施**：
- 添加 `manualChunks` 配置：
  - `react-vendor`: react, react-dom
  - `editor-vendor`: @tiptap/*, @monaco-editor/react
  - `ui-vendor`: framer-motion, reactflow, lucide-react
  - `data-vendor`: @tanstack/react-query, zustand
- 验证：构建后 `dist/assets/` 下出现多个 chunk 文件，单个 chunk < 500KB

---

## Phase 5: 数据层与架构深化（P2-P3）

**目标**：清理数据模型分裂，提升数据库层质量
**依赖**：Phase 2 完成（命令层拆分后数据层变更风险降低）

### 5.1 引入迁移版本控制
**文件**：`src-tauri/src/db/connection.rs`
**问题**：53 个手工条件判断迁移，无版本号表，每次启动全量检查
**实施**：
- 创建 `schema_migrations` 表：`version INTEGER PRIMARY KEY, applied_at TEXT`
- 将现有迁移逻辑编号（1-53），记录当前已应用的版本
- 新迁移遵循：插入 schema_migrations → 执行 ALTER/CREATE
- 长期目标：评估引入 `rusqlite_migration` crate 替代手工管理
- 验证：新数据库正确初始化，旧数据库正确迁移

### 5.2 清理 v2/v3 模型分裂
**范围**：`db/models.rs` vs `db/models_v3.rs`，`db/repositories.rs` vs `db/repositories_v3.rs`
**问题**：两套模型和仓储并存，`chapters` 表与 `scenes` 表双轨运行
**实施**：
- 分析 `chapters` 表是否仍有活跃使用（除了向后兼容外）
- 若 `scenes` 已完全取代 `chapters`，将 chapters 相关代码标记为 deprecated，逐步迁移
- 合并 `models.rs` + `models_v3.rs` → `models.rs`（单一套件）
- 合并 `repositories.rs` + `repositories_v3.rs` → `repositories.rs`
- 验证：所有命令正常执行，无编译错误

### 5.3 向量存储统一
**文件**：`src-tauri/src/vector/mod.rs`
**问题**：简陋的词频向量 fallback 与 LanceDB 并存，维度不一致、全量扫描
**实施**：
- 评估 LanceDB 是否已稳定可用（确认初始化逻辑）
- 若 LanceDB 可用，删除 `VectorStore` fallback 实现
- 统一所有向量操作走 `LanceVectorStore`
- 验证：向量搜索功能正常

---

## Phase 6: 架构改进（P3）

**目标**：解决深层架构问题
**依赖**：Phase 2-5 完成（基础稳定后再做架构变动）

### 6.1 AgentContext 拆分
**文件**：`src-tauri/src/agents/mod.rs`
**问题**：20+ 字段的 God Context，每次构建都全量聚合
**实施**：
- 拆分为嵌套结构：
  - `StoryContext`: story_id, title, genre, tone, pacing
  - `NarrativeContext`: chapter_number, characters, previous_chapters
  - `StyleContext`: style_dna_id, style_blend, style_fingerprint
  - `WorldContext`: world_rules, scene_structure
  - `MemoryContext`: memory_pack, memory_context
- 使用 Builder 模式按需加载
- 验证：Agent 执行正常，context 构建不丢失信息

### 6.2 提取命令层公共逻辑
**文件**：拆分后的命令文件
**问题**：每个 mutation command 末尾重复编写 `StateSync::emit_data_refresh()`
**实施**：
- 创建 `CommandContext` 或 middleware 机制
- 或使用过程宏 `#[tauri::command]` + 自定义 derive 自动注入 sync
- 先以 helper 函数简化（`fn emit_on_success<T>(result: Result<T>, app: &AppHandle, story_id: &str, data_type: &str)`）

### 6.3 优雅关闭
**文件**：`src-tauri/src/lib.rs`
**问题**：`std::process::exit(0)` 在窗口关闭时直接退出
**实施**：
- 添加应用关闭生命周期钩子
- 在关闭前：完成 SQLite WAL checkpoint、持久化 pending vector indexes、停止 automation service
- 使用 Tauri 的 `RunEvent::ExitRequested` 处理退出逻辑
- 验证：应用关闭后数据库文件一致，无 WAL 残留

---

## 验证策略

### 每阶段通用验证
- `cargo check` / `cargo build` 无错误
- `cargo test` 测试通过（关注新增/修改模块的测试）
- `npm run type-check` 无 TypeScript 错误
- `npm run build` 前端构建成功

### 端到端验证
- 启动应用，创建新故事
- 添加角色、场景、世界观
- 切换 backstage ↔ frontstage，确认数据同步
- 触发 AI 生成，确认流式输出正常
- 关闭并重新打开应用，确认数据持久化
- 运行 Playwright E2E 测试：`npx playwright test`

---

## 执行顺序与依赖

```
Phase 1: 安全与稳定性
  ├── 1.1 修复内存泄漏
  ├── 1.2 删除旧前端
  └── 1.3 清理 lib.rs
       ↓
Phase 2: 后端命令层拆分
  ├── 2.1 拆分 story_commands.rs
  ├── 2.2 统一错误处理
  └── 2.3 标准化状态注入
       ↓
Phase 3: 状态同步与前端优化
  ├── 3.1 补齐状态同步事件
  ├── 3.2 优化 Query 缓存
  └── 3.3 移除 DOM Hack
       ↓
Phase 4: 构建工具链（可与 Phase 3 并行）
  ├── 4.1 Rust fmt/clippy
  ├── 4.2 Prettier/ESLint
  └── 4.3 Vite 代码分割
       ↓
Phase 5: 数据层深化
  ├── 5.1 迁移版本控制
  ├── 5.2 清理 v2/v3 分裂
  └── 5.3 向量存储统一
       ↓
Phase 6: 架构改进
  ├── 6.1 AgentContext 拆分
  ├── 6.2 命令层公共逻辑提取
  └── 6.3 优雅关闭
```

**可并行**：Phase 4（工具链）与 Phase 3（前端优化）可并行执行，Phase 1 与 Phase 4 的早期部分也可并行。

---

## 风险与回滚策略

| 风险 | 缓解措施 |
|------|----------|
| story_commands.rs 拆分引入回归 | 每次只拆分一个领域文件，保持命令签名不变，充分测试后删除原代码 |
| 错误处理统一破坏前端错误提示 | 保持 AppError 的 to_string 输出与原 String 错误一致 |
| 格式化全库导致 git blame 混乱 | 使用 `.git-blame-ignore-revs` 忽略格式化 commit |
| 状态注入迁移导致命令不可用 | 逐个文件迁移，每次迁移后手动测试该文件的所有命令 |
| 删除旧前端误删有用文件 | 删除前搜索所有引用，构建验证 |

---

## 成功标准

1. **稳定性**：FrontstageApp 反复挂载/卸载无内存增长（通过 DevTools Heap 快照验证）
2. **可维护性**：最大单个 Rust 文件 < 800 行（当前 story_commands.rs 3445 行）
3. **一致性**：所有 mutation command 发射对应的状态同步事件
4. **性能**：Vite 构建产物单个 chunk < 500KB，故事切换时并发 IPC 请求 < 5 个
5. **质量**：CI 中 `cargo fmt --check`、`cargo clippy -- -D warnings`、`eslint` 全部通过
