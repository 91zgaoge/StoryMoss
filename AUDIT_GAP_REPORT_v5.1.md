# StoryForge v5.1.0 全面检视：设计目标 vs 代码实现差距报告

> **检查日期**: 2026-05-01
> **检查范围**: 幕前幕后自动关联、后台自动化、智能化创作飞轮
> **检查方法**: 静态代码审计 + 文档对比 + 关键路径追踪

---

## 一、执行摘要

v5.1.0 在**基础设施层面**已建立了较为完整的框架：`state_sync` 模块、`AgentOrchestrator` 闭环、`TaskService` 调度、`QueryPipeline` 上下文注入、`useSyncStore` 前端Hook 等核心组件均已落地。但存在**11 项关键差距**，其中 **5 项 P0（影响功能正确性）**、**4 项 P1（影响完整度）**、**2 项 P2（文档/精度）**。

最大的结构性缺口是：**"保存后自动 Ingest" 这一智能化创作飞轮的核心环节未闭合**——`update_chapter` 完全缺失 Ingest 触发，`create_chapter` 的 Hook 路径也不保证 Ingest 执行。这导致 PROJECT_IMPROVEMENT_PLAN 中提出的"每写一章 → 自动分析 → 更新知识图谱 → 下次写作注入记忆"的飞轮在最关键的环节断裂。

---

## 二、P0 关键差距（功能断裂）

### 差距 1: `update_chapter` 保存后完全不触发 Ingest ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/lib.rs:670` `update_chapter` 命令 |
| **设计目标** | PROJECT_IMPROVEMENT_PLAN Phase 1: "保存章节后自动 Ingest，5 秒内出现在知识图谱" |
| **现状** | `update_chapter` 仅更新数据库 + 发射 `chapter_updated` sync 事件 + 发送 SaveStatus 到幕前。**零 Ingest 触发。** |
| **影响** | 用户在幕前写作、修改内容后，知识图谱永不更新。系统"越写越懂"是伪命题。 |
| **根因** | `create_chapter` 有 `AfterChapterSave` skill hook（`lib.rs:701-715`），但 `update_chapter` 完全未添加。 |

**代码证据**:
```rust
// lib.rs:670 — update_chapter 完全没有 ingest 或 hook 调用
fn update_chapter(id: String, title: Option<String>, outline: Option<String>, 
                  content: Option<String>, word_count: Option<i32>, app: AppHandle) 
                  -> Result<(), String> {
    let result = db::ChapterRepository::new(...).update(&id, ...).map_err(...);
    if result.is_ok() {
        let _ = window::WindowManager::send_to_frontstage(...SaveStatus...);
        let _ = crate::state_sync::StateSync::emit_chapter_updated(&app, &id, ...);
    }
    result.map(|_| ())
}
```

---

### 差距 2: `WorkflowScheduler::schedule_execution` 空实现 ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/workflow/scheduler.rs:12-19` |
| **设计目标** | AGENTS.md / ARCHITECTURE.md: Workflow 引擎支持创作阶段编排 |
| **现状** | `schedule_execution` 只是 `log::info!` 记录请求，**不将工作流实例加入任何执行队列**。 |
| **影响** | 所有通过 Workflow 系统编排的自动化流程（包括 `CreationWorkflowEngine` 理论上依赖的底层调度）无法真正运行。 |
| **根因** | 开发者明确标注 `"For now, we just log the request"` — 这是一个待实现的占位符。 |

**代码证据**:
```rust
pub async fn schedule_execution(&self, instance_id: String) 
    -> Result<(), Box<dyn std::error::Error>> {
    log::info!("[WorkflowScheduler] Queuing workflow instance {} for execution", instance_id);
    // In production, this would enqueue the instance to a task queue
    // and let a worker pool pick it up. For now, we just log the request.
    Ok(())
}
```

---

### 差距 3: `FrontstageToolbar` "幕后"按钮不传递 `story_id` ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-frontend/src/frontstage/components/FrontstageToolbar.tsx:28-33` |
| **设计目标** | v5.1.0 CHANGELOG: "幕前→幕后快速跳转，幕后自动定位当前故事" |
| **现状** | `handleToggleBackstage` 调用 `invoke('show_backstage')` **完全不传 `story_id`**。 |
| **影响** | 用户点击侧边栏/工具栏的"幕后"按钮时，幕后不会自动定位到当前故事，需要手动查找。 |
| **对比** | `FrontstageApp.tsx:546` 的 `Ctrl+Shift+B` 和标题栏点击**正确传递了** `story_id`。 |

**代码证据**:
```typescript
// FrontstageToolbar.tsx — ❌ 缺少 story_id
const handleToggleBackstage = async () => {
    await invoke('show_backstage');  // 无参数！
};

// FrontstageApp.tsx — ✅ 正确传递
await invoke('show_backstage', { story_id: currentStory?.id || null });
```

---

### 差距 4: `state_sync` character/chapter update/delete 事件携带空 `story_id` ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/lib.rs:641-648, 674-682` 等 |
| **设计目标** | `useSyncStore` 根据 `storyId` invalidate 对应故事的缓存 |
| **现状** | `emit_character_updated`, `emit_character_deleted`, `emit_chapter_updated`, `emit_chapter_deleted` 传入 `String::new()`（空字符串）作为 `story_id`。 |
| **影响** | `useSyncStore` 收到这些事件后，`storyId` 为空 → 无法精准刷新对应故事的 `characters`/`chapters` 缓存 → 幕后数据可能 stale。 |
| **根因** | 代码注释承认 `"character_id 需要查 story_id，这里简化处理"`，但未修复。 |

**代码证据**:
```rust
// lib.rs 中多处:
let _ = crate::state_sync::StateSync::emit_character_updated(&app, &id, name.as_deref());
// 内部实现:
payload: SyncPayload::CharacterUpdated { character_id, name, story_id: String::new() }
```

---

### 差距 5: `create_chapter` 的 `AfterChapterSave` Hook 不保证 Ingest 执行 ❌

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/lib.rs:701-715` |
| **设计目标** | 保存章节后**必须**触发 IngestPipeline，更新知识图谱 |
| **现状** | `create_chapter` 调用 `skill_manager.execute_hooks(AfterChapterSave, ...)`，但 hook 的执行内容取决于当前注册的技能。**没有技能保证会调用 `IngestPipeline`**。 |
| **影响** | 即使 `create_chapter`，知识图谱更新也是"概率性"的——取决于 skills 配置。 |
| **根因** | Ingest 应该作为数据层 save 的**副作用**硬编码触发，而不是依赖可选的 skill hook。 |

---

## 三、P1 重要差距（功能不完整）

### 差距 6: `prompts/` 目录缺少方法论模板子系统 ⚠️

| 项 | 详情 |
|---|---|
| **位置** | `src-tauri/src/prompts/` |
| **设计目标** | PROJECT_IMPROVEMENT_PLAN 子系统 A: `prompts/methodologies/snowflake/`, `hero_journey/`, `scene_structure/` |
| **现状** | 只有 `engine.rs`、`evolver.rs`、`mod.rs` 三个文件。`PromptLibrary` 只有 `writer_system_template()` 一个模板。 |
| **影响** | 方法论注入（雪花法、英雄之旅、场景节拍）没有模板化基础设施。Agent 提示词仍然是硬编码字符串。 |

---

### 差距 7: `PromptLibrary` 模板覆盖不全 ⚠️

| 项 | 详情 |
|---|---|
| **设计目标** | `PromptLibrary` 应提供 writer、inspector、planner、methodology 等全套模板 |
| **现状** | 仅实现了 `writer_system_template()`。Inspector、StyleChecker、OutlinePlanner 等 Agent 仍使用硬编码提示词。 |
| **影响** | 提示词管理分散，难以统一维护和优化。 |

---

### 差距 8: `FrontstageToolbar` 仍显式显示"AI 续写"按钮 ⚠️

| 项 | 详情 |
|---|---|
| **设计目标** | v4.1.0 P0: "移除'AI 续写'按钮"，改为 `Ctrl+Enter` 全局触发 + 幽灵文本呈现 |
| **现状** | `FrontstageToolbar.tsx:52-59` 仍然有一个显式的 "AI 续写" 按钮。 |
| **影响** | 与 v4.1.0 "化整为零，萤火随行"的设计理念冲突。 |

---

### 差距 9: 文档与实际 SyncEvent 数量不一致 ⚠️

| 项 | 详情 |
|---|---|
| **文档声称** | AGENTS.md / CHANGELOG: "18 种 `SyncEvent`" |
| **实际** | `state_sync/events.rs` 中定义了 **16 种**: Story(4) + Character(3) + Scene(4) + Chapter(3) + WorldBuilding(1) + DataRefresh(1) = 16 |
| **影响** | 文档精度问题。实际 16 种已覆盖主要数据类型，功能不受影响。 |

---

## 四、P2 优化差距（锦上添花）

### 差距 10: `QueryPipeline` 错误降级时无监控 ⚠️

`build_with_query` 失败时降级到 `AgentContext::minimal()`，只有 `log::warn!` 记录，前端和用户完全无感知。建议增加指标或事件上报。

### 差距 11: `task_system` 定时任务执行器缺少超时/重试机制 ⚠️

`TaskScheduler::spawn_interval` 中的 callback 是 `Fn() + Send + 'static`，但不支持 `async`，也没有任务超时和失败重试逻辑。

---

## 五、已验证正确实现的清单（对齐部分）

| 功能 | 状态 | 验证位置 |
|------|------|----------|
| `state_sync` 模块 + 事件发射 | ✅ | `state_sync/`, `lib.rs` |
| `useSyncStore` Hook + 自动刷新 | ✅ | `src/hooks/useSyncStore.ts` |
| `show_backstage` 接受 `story_id` | ✅ | `window/mod.rs` |
| `AgentOrchestrator` Writer→Inspector→StyleChecker 闭环 | ✅ | `agents/orchestrator.rs` |
| `ChapterRepository` 自动创建/关联 Scene | ✅ | `db/repositories.rs:242-312` |
| `build_with_query` 注入 Writer 上下文 | ✅ | `agents/commands.rs:893` |
| `TaskService` 初始化 + bootstrap | ✅ | `lib.rs:144-156` |
| `McpClient` 真实 StdioTransport 实现 | ✅ | `mcp/client.rs` |
| `prompts/engine.rs` TemplateEngine | ✅ | `prompts/engine.rs` |
| `App.tsx` Zustand↔TanStack Query 同步 | ✅ | `App.tsx:35-46` |
| `FrontstageApp.tsx` ghost text + `/` 菜单 | ✅ | `FrontstageApp.tsx` |
| `record_feedback` 前端调用 | ✅ | `services/tauri.ts:218` |

---

## 六、差距影响矩阵

| 差距 | 幕前幕后关联 | 后台自动化 | 智能化飞轮 | 优先级 |
|------|:----------:|:--------:|:--------:|:------:|
| 1. update_chapter 不触发 Ingest | — | — | 🔴 断裂 | **P0** |
| 2. WorkflowScheduler 空实现 | — | 🔴 断裂 | — | **P0** |
| 3. FrontstageToolbar 不传 story_id | 🔴 断裂 | — | — | **P0** |
| 4. state_sync 空 story_id | 🔴 断裂 | — | — | **P0** |
| 5. create_chapter Hook 不保证 Ingest | — | — | 🟡 不稳 | **P0** |
| 6. 缺少方法论模板目录 | — | 🟡 不完整 | 🟡 不完整 | P1 |
| 7. PromptLibrary 覆盖不全 | — | 🟡 不完整 | — | P1 |
| 8. FrontstageToolbar 显式 AI 按钮 | 🟡 不一致 | — | — | P1 |
| 9. SyncEvent 数量文档错误 | — | — | — | P2 |
| 10. QueryPipeline 降级无感知 | — | — | 🟡 | P2 |
| 11. Task 调度缺少超时/重试 | — | 🟡 | — | P2 |
