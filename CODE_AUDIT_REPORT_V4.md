# StoryMoss v4.0.0 代码全面审查报告

> 审查日期: 2026-04-22
> 审查范围: 后端 Rust (`src-tauri/src`) + 前端 TypeScript (`src-frontend/src`)
> 审查重点: 空实现 / 占位符 / 前后端对应 / 参数命名匹配

---

## 一、编译与测试状态

| 检查项 | 状态 |
|--------|------|
| `cargo check` | ✅ 通过，0 警告 |
| `cargo test` | ✅ 160/160 通过 |
| `npm test` (Vitest) | ✅ 21/21 通过 |
| `npm run build` | ✅ 通过 |

**结论**: 项目编译和单元测试全部通过，但测试覆盖的是已实现的功能，未覆盖大量占位符和空实现代码。

---

## 二、后端 Rust 空实现 / 占位符问题

### 🔴 P0 高优先级（功能虚假/数据丢失）

#### 1. `analytics/mod.rs` — 写作分析数据完全硬编码
- **位置**: `src-tauri/src/analytics/mod.rs:44-50`
- **问题**: `writing_streak` 固定为 `current_streak: 1, longest_streak: 7`；`productivity_score` 固定为 `80.0`；`avg_words_per_day` 固定为 `1500.0`
- **影响**: 用户看到的写作统计是虚假数据，核心分析功能未实现
- **建议**: 基于实际章节创建时间和字数计算真实统计

#### 2. `chat/mod.rs` — 聊天系统纯内存，无持久化
- **位置**: `src-tauri/src/chat/mod.rs`
- **问题**: `ChatManager` 使用 `HashMap<String, ChatSession>` 纯内存存储，`create_session`/`add_message`/`delete_session` 在应用重启后全部丢失
- **影响**: 聊天会话数据无法持久保存
- **建议**: 添加 `chat_sessions` / `chat_messages` 数据库表及 Repository 层

#### 3. `collab/mod.rs` — 协作编辑 OT 未实现
- **位置**: `src-tauri/src/collab/mod.rs:117-123`
- **问题**: `apply_operation` 仅将操作 `push` 到 `operations` 数组，**没有调用 OT 转换引擎**实际应用编辑
- **影响**: 多人协作编辑功能不可用
- **建议**: 集成 OT 算法，将操作转换为实际文本变更

#### 4. `collab/websocket.rs` — WebSocket 消息处理为空
- **位置**: `src-tauri/src/collab/websocket.rs:195-201`
- **问题**: 
  - `CollabMessage::Operation` 分支只有 `// TODO: Get session from context`
  - `CollabMessage::Cursor` 分支只有 `// Broadcast cursor position`
- **影响**: 协作编辑的操作广播和光标同步完全未实现
- **建议**: 实现操作广播逻辑和光标位置转发

#### 5. `state/manager.rs` — 故事状态纯内存，无持久化
- **位置**: `src-tauri/src/state/manager.rs`
- **问题**: `StoryStateManager` 使用 `HashMap` 纯内存存储所有故事状态、角色状态、章节进度。应用重启后全部丢失
- **影响**: 故事进度、角色弧线、世界状态等运行时数据无法保存
- **建议**: 将 `story_states` 表持久化到 SQLite，或至少在应用关闭时序列化到 JSON

---

### 🟡 P1 中优先级（功能缺失/降级）

#### 6. `export/mod.rs` — 文本导入未解析内容
- **位置**: `src-tauri/src/export/mod.rs:90-102`
- **问题**: `import_from_text` 接收 `_content` 参数但完全未解析，直接返回空 `vec![]` 和仅含标题的 `CreateStoryRequest`
- **影响**: 从文本导入故事功能不可用
- **建议**: 实现文本分块、章节识别、标题提取逻辑

#### 7. `skills/executor.rs` — MCP 技能未实际执行
- **位置**: `src-tauri/src/skills/executor.rs:185-203`
- **问题**: `execute_mcp` 未实际连接 MCP 服务器执行工具调用，仅将 `server_config` 序列化为 JSON 后返回
- **影响**: MCP 类型的技能无法真实执行
- **建议**: 调用 `McpClient` 或 `McpServer::execute_tool` 执行真实工具调用

#### 8. `mcp/server.rs` — WebSearchTool handle 为模拟实现
- **位置**: `src-tauri/src/mcp/server.rs:91-111`
- **问题**: `WebSearchTool::handle` 硬编码返回模拟搜索结果。虽然同文件的 `execute_tool` 方法（行242-266）已实现了真实的 DuckDuckGo 搜索，但 `handle_tool_call`（行200-213）调用的是 `handle` 而非 `execute_tool`
- **影响**: 网页搜索返回虚假结果
- **建议**: 将 `handle_tool_call` 中的 `web_search` 路由到 `execute_tool`

#### 9. `workflow/scheduler.rs` — 工作流调度为空
- **位置**: `src-tauri/src/workflow/scheduler.rs:12-19`
- **问题**: `schedule_execution` 函数体只有注释 `// Placeholder for actual scheduling logic` 和 `Ok(())`
- **影响**: 工作流调度功能不可用
- **建议**: 集成任务系统（TaskSystem）或 tokio 调度器实现实际调度

#### 10. `evolution/updater.rs` — 技能更新未应用
- **位置**: `src-tauri/src/evolution/updater.rs:154-160`
- **问题**: `apply_update` 函数体只有注释 `// Implementation would update the actual skill data` 和 `Ok(())`
- **影响**: 进化系统分析出的技能更新建议无法应用到实际技能
- **建议**: 实现 `SkillManifest` 的字段更新和持久化

#### 11. `agents/commands.rs` — Agent 状态查询硬编码
- **位置**: `src-tauri/src/agents/commands.rs:223-226`
- **问题**: `agent_get_status` 无论传入什么 `task_id` 都返回硬编码字符串 `"running"`
- **影响**: 无法获取 Agent 任务的真实执行状态
- **建议**: 从 `TASK_HANDLES` 中查询任务是否存在，返回 `running`/`completed`/`not_found`

---

### 🟢 P2 低优先级（设计如此或可接受）

#### 12. `updater/mod.rs` — 打开更新设置为空
- **位置**: `src-tauri/src/updater/mod.rs:112-117`
- **问题**: `open_update_settings` 只有 `Ok(())`
- **备注**: 注释说明"具体的设置界面由前端实现"，属于合理占位

#### 13. `canonical_state/manager.rs` — update_story_context 为空
- **位置**: `src-tauri/src/canonical_state/manager.rs:127-136`
- **问题**: `update_story_context` 为空实现
- **备注**: 注释说明"context is aggregated in real-time"，这是有意的设计决策，非 bug

---

## 三、前端 TypeScript 问题

### 🔴 P0 高优先级 — 前后端参数命名不匹配（camelCase vs snake_case）

> **关键背景**: Tauri v2 的 `invoke` 不会自动将 camelCase 转换为 snake_case。前端传的参数键名必须与后端命令的参数名完全一致，否则后端会收到空值/默认值。

| # | 前端文件 | 调用位置 | 命令 | 前端传的键名 | 后端期望的键名 |
|---|---------|---------|------|-------------|--------------|
| 1 | `services/tauri.ts` | 行49 | `get_story_chapters` | `storyId` | `story_id` |
| 2 | `services/tauri.ts` | 行68 | `get_skill` | `skillId` | `skill_id` |
| 3 | `services/tauri.ts` | 行78 | `enable_skill` | `skillId` | `skill_id` |
| 4 | `services/tauri.ts` | 行81 | `disable_skill` | `skillId` | `skill_id` |
| 5 | `services/tauri.ts` | 行84 | `uninstall_skill` | `skillId` | `skill_id` |
| 6 | `services/tauri.ts` | 行87 | `update_skill` | `skillId` | `skill_id` |
| 7 | `services/tauri.ts` | 行134 | `embed_chapter` | `chapterId` | `chapter_id` |
| 8 | `services/settings.ts` | 行165 | `set_active_model` | `modelType`, `modelId` | `model_type`, `model_id` |
| 9 | `services/settings.ts` | 行221 | `test_model_connection` | `modelId` | `model_id` |
| 10 | `hooks/useBookDeconstruction.ts` | 行24 | `upload_book` | `filePath` | `file_path` |
| 11 | `hooks/useBookDeconstruction.ts` | 行157 | `get_analysis_status` | `bookId` | `book_id` |
| 12 | `hooks/useBookDeconstruction.ts` | 行180 | `get_book_analysis` | `bookId` | `book_id` |
| 13 | `hooks/useBookDeconstruction.ts` | 行206 | `delete_reference_book` | `bookId` | `book_id` |
| 14 | `hooks/useBookDeconstruction.ts` | 行219 | `convert_book_to_story` | `bookId` | `book_id` |
| 15 | `hooks/useBookDeconstruction.ts` | 行232 | `cancel_book_analysis` | `bookId` | `book_id` |
| 16 | `frontstage/FrontstageApp.tsx` | 行241 | `update_chapter` | `wordCount` | `word_count` |
| 17 | `frontstage/FrontstageApp.tsx` | 行254 | `notify_backstage_content_changed` | `chapterId` | `chapter_id` |

**影响**: 这 17 处调用中，后端会接收到 `undefined` 或空字符串，导致：
- 拆书功能可能无法获取正确的书籍 ID
- 模型设置可能无法保存激活模型
- 章节字数无法正确更新
- 幕后内容变更通知可能丢失章节 ID

---

### 🟡 P1 中优先级

#### 18. `services/settings.ts` — 浏览器 Fallback 硬编码敏感信息
- **位置**: `src-frontend/src/services/settings.ts:16-66`
- **问题**: `BROWSER_FALLBACK_MODELS` 包含内部 IP `10.62.239.13` 和明文 API key `76e0e2bc...`
- **影响**: 安全风险，且在不同环境无法运行
- **建议**: 改为从环境变量读取，或移除真实配置使用占位符

#### 19. `services/modelService.ts` — 模拟流式而非真实 SSE
- **位置**: `src-frontend/src/services/modelService.ts:118-124`
- **问题**: `chat()` 在 `options.stream=true` 时一次性推送完整内容，并非真实流式传输
- **影响**: 用户体验上的"伪流式"，前端无法显示逐字输出效果
- **建议**: 接入真实 SSE 流式接口

#### 20. `hooks/useCollaboration.ts` — 协同编辑发送函数为空
- **位置**: `src-frontend/src/hooks/useCollaboration.ts:138-146`
- **问题**: `sendOperation()` 和 `sendCursorPosition()` 只有 `console.log`，没有实际发送逻辑。更严重的是，`connect()` 中创建的 `WebSocket` 实例 `ws` 没有保存到 ref 或 state
- **影响**: 协同编辑功能完全不可用
- **建议**: 使用 `useRef` 保存 ws 实例，在 send 方法中调用 `ws.send()`

#### 21. `frontstage/hooks/useStreamingGeneration.ts` — Mock 函数留在生产代码
- **位置**: `src-frontend/src/frontstage/hooks/useStreamingGeneration.ts:201-230`
- **问题**: `mockStreamGeneration()` 函数使用硬编码中文文本随机返回，明确标注"用于测试"
- **影响**: 生产代码中包含测试替身
- **建议**: 移除或移动到 `__tests__` 目录

#### 22. `frontstage/ai-perception/textAnalyzer.ts` — 增量分析未实现
- **位置**: `src-frontend/src/frontstage/ai-perception/textAnalyzer.ts:439-441`
- **问题**: `analyzeRecent()` 注释明确写"TODO: 实现真正的增量分析"，当前直接回退到全量分析
- **影响**: 文本分析性能优化未生效
- **建议**: 实现基于文本差异的增量分析

---

## 四、前后端 IPC 接口对应关系

| 指标 | 数值 |
|------|------|
| 后端命令总数 | 194 |
| 前端 invoke 调用 | 179 |
| 前后端命令对应 | 179 / 179 ✅ |
| 前端调用但后端缺失 | 0 ❌ |
| 后端实现但前端未调用 | 15 ⚠️（合理：被替代/内部通知/预留） |
| **参数名不匹配** | **17 处 🔴** |

**结论**: 接口命名对应度 100%，但参数命名存在系统性不匹配问题。

---

## 五、全局性问题

### 1. `#![allow(dead_code)]` 泛滥
超过 20 个 `.rs` 文件顶部带有 `#![allow(dead_code)]` 或 `#[allow(unused_imports)]`：

| 模块 | 指令 |
|------|------|
| `agents/*.rs` | `#![allow(dead_code)]` |
| `analytics/mod.rs` | `#![allow(dead_code)]` |
| `chat/mod.rs` | `#![allow(dead_code)]` |
| `collab/*.rs` | `#![allow(dead_code)]` |
| `config/*.rs` | `#![allow(dead_code)]` |
| `embeddings/*.rs` | `#![allow(dead_code)]` |
| `evolution/*.rs` | `#![allow(dead_code)]` |
| `export/mod.rs` | `#![allow(dead_code)]` |
| `llm/*.rs` | `#![allow(dead_code)]` |
| `memory/mod.rs` | `#![allow(dead_code)]` |
| `mcp/*.rs` | `#![allow(dead_code)]` |
| `prompts/mod.rs` | `#![allow(dead_code)]` |
| `router/*.rs` | `#![allow(dead_code)]` |
| `skills/*.rs` | `#![allow(dead_code)]` |
| `state/*.rs` | `#![allow(dead_code)]` |
| `utils/*.rs` | `#![allow(dead_code)]` |
| `vector/*.rs` | `#![allow(dead_code)]` |
| `versions/*.rs` | `#![allow(dead_code)]` |
| `lib.rs` | `#![allow(dead_code)]` |

**影响**: 这些指令掩盖了真实的代码质量问题，使得未使用的变量、未调用的函数、未完成的功能无法被编译器警告。

### 2. 纯内存模块导致数据丢失
以下核心模块完全依赖内存存储，应用重启后数据全部丢失：
- `chat/mod.rs` — 聊天会话
- `collab/mod.rs` — 协作会话
- `state/manager.rs` — 故事运行状态

---

## 六、完善与优化计划

### Phase A: P0 紧急修复（1-2 天）

**目标**: 修复会导致功能失效或数据错误的问题

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| A1 | 修复 17 处 camelCase→snake_case 参数命名不匹配 | 6 个前端文件 | 小 |
| A2 | 修复 `analytics/mod.rs` 硬编码统计（改为基于真实数据计算） | `src-tauri/src/analytics/mod.rs` | 中 |
| A3 | 修复 `agents/commands.rs` `agent_get_status` 真实状态查询 | `src-tauri/src/agents/commands.rs` | 小 |
| A4 | 修复 `mcp/server.rs` `handle_tool_call` 路由到 `execute_tool` | `src-tauri/src/mcp/server.rs` | 小 |
| A5 | 移除 `services/settings.ts` 硬编码 API key | `src-frontend/src/services/settings.ts` | 小 |

### Phase B: P1 功能补全（3-5 天）

**目标**: 补全核心功能的缺失实现

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| B1 | 为 `chat/mod.rs` 添加数据库持久化表 + Repository | 新增 `chat_sessions`/`chat_messages` 表 | 中 |
| B2 | 为 `state/manager.rs` 添加 SQLite 持久化或序列化 | `src-tauri/src/state/manager.rs` | 中 |
| B3 | 实现 `export/mod.rs` `import_from_text` 文本解析 | `src-tauri/src/export/mod.rs` | 中 |
| B4 | 实现 `skills/executor.rs` `execute_mcp` 真实 MCP 调用 | `src-tauri/src/skills/executor.rs` | 中 |
| B5 | 实现 `workflow/scheduler.rs` `schedule_execution` 调度逻辑 | `src-tauri/src/workflow/scheduler.rs` | 中 |
| B6 | 实现 `evolution/updater.rs` `apply_update` 技能更新应用 | `src-tauri/src/evolution/updater.rs` | 小 |
| B7 | 修复 `hooks/useCollaboration.ts` WebSocket 发送逻辑 | `src-frontend/src/hooks/useCollaboration.ts` | 小 |
| B8 | 移除 `useStreamingGeneration.ts` `mockStreamGeneration` | `src-frontend/src/frontstage/hooks/useStreamingGeneration.ts` | 小 |
| B9 | 实现 `textAnalyzer.ts` `analyzeRecent` 增量分析 | `src-frontend/src/frontstage/ai-perception/textAnalyzer.ts` | 中 |

### Phase C: P2 架构优化（5-7 天）

**目标**: 提升代码质量，消除技术债务

| # | 任务 | 说明 |
|---|------|------|
| C1 | 逐步移除 `#![allow(dead_code)]` | 逐个模块清理，每次清理后确保 `cargo check` 0 警告 |
| C2 | 为纯内存模块添加持久化层 | `chat`/`collab`/`state` 添加数据库表 |
| C3 | 统一前后端参数命名规范 | 建立前端调用规范文档，所有新代码强制 snake_case |
| C4 | 为占位符实现添加单元测试 | `analytics`, `export`, `workflow` 等模块补充测试 |
| C5 | 添加 IPC 参数名静态检查 | 考虑编写脚本或类型检查，防止 camelCase 混入 |

---

## 七、风险等级汇总

| 等级 | 数量 | 代表问题 |
|------|------|---------|
| 🔴 高 | 5+17 | 数据硬编码、参数不匹配、数据丢失 |
| 🟡 中 | 9 | 功能空实现、Mock 数据、协作不可用 |
| 🟢 低 | 2 | 设计占位、已注释说明 |

---

## 八、建议的实施顺序

```
Day 1: Phase A1（参数命名修复）— 影响面最大，最容易修复
Day 1: Phase A2-A5 — 核心功能数据修复
Day 2-3: Phase B1-B4 — 持久化与功能补全
Day 4-5: Phase B5-B9 — 剩余空实现补全
Day 6-10: Phase C — 代码质量与架构优化
```

**每次修改后必须执行**:
```bash
cd src-tauri && cargo test      # 160 tests
cd src-frontend && npm test     # 21 tests
cd src-frontend && npm run build # 前端构建
cd src-tauri && cargo check     # Rust 检查
```

---

*报告生成完毕，待审批后实施。*
