# StoryForge v5.1.x 设计-实现对齐实施计划

> **基于**: AUDIT_GAP_REPORT_v5.1.md
> **目标**: 消灭 P0 差距，补齐 P1 差距，全面达到 v5.1.0 设计目标
> **预计工期**: 2-3 天（按顺序执行）

---

## 一、实施原则

1. **先 P0 后 P1 再 P2** — 功能正确性优先于完整度
2. **最小侵入** — 尽量复用已有基础设施，不引入新架构
3. **编译即通过** — 每阶段结束后 `cargo check` + `cargo test` + `npm run build` 必须全绿
4. **文档同步** — 修复一处，同步更新 AGENTS.md / CHANGELOG

---

## 二、Phase 1: P0 紧急修复（Day 1）

### 任务 1.1: 修复 `FrontstageToolbar` story_id 传递
**文件**: `src-frontend/src/frontstage/components/FrontstageToolbar.tsx`
**修改**: 
- 为组件添加 `storyId?: string` prop
- `handleToggleBackstage` 中传递 `story_id: storyId || null`
- 在 `FrontstageApp.tsx` 中使用 `<FrontstageToolbar storyId={currentStory?.id} />`
**验收**: 点击"幕后"按钮，幕后自动高亮当前故事
**工期**: 15 分钟

---

### 任务 1.2: 修复 `state_sync` 空 story_id
**文件**: `src-tauri/src/lib.rs`
**修改**:
- `update_character` 前查询 `character` 所属 `story_id`，传递给 `emit_character_updated`
- `delete_character` 同理
- `update_chapter` 前查询 `chapter` 所属 `story_id`，传递给 `emit_chapter_updated`
- `delete_chapter` 同理
**验收**: `useSyncStore` 收到这些事件后，`storyId` 不为空，能精准刷新缓存
**工期**: 30 分钟

---

### 任务 1.3: `update_chapter` 保存后自动触发 Ingest
**文件**: `src-tauri/src/lib.rs`
**修改**:
- 在 `update_chapter` 成功后，参考 `create_chapter` 的 `AfterChapterSave` hook 逻辑，异步触发 skill hooks
- **更优方案**: 直接在 `update_chapter` 和 `create_chapter` 成功后，统一调用 `IngestPipeline::ingest()`，不依赖 skill hook 的"概率性"触发
**验收**: 
- 修改章节内容后保存，5 秒内知识图谱中的实体/关系被更新
- `cargo test` 通过
**工期**: 45 分钟

---

### 任务 1.4: 固化 `create_chapter` 的 Ingest 触发
**文件**: `src-tauri/src/lib.rs`
**修改**:
- 在 `AfterChapterSave` hook 之后，**硬编码**触发 `IngestPipeline::ingest()`（或调用一个统一的 `auto_ingest_chapter` 辅助函数）
- 确保无论 skills 配置如何，Ingest **必定执行**
**验收**: 
- 新建章节后，知识图谱必定更新
- 单元测试验证 ingest 被调用
**工期**: 30 分钟

---

### 任务 1.5: 实现 `WorkflowScheduler::schedule_execution`
**文件**: `src-tauri/src/workflow/scheduler.rs`, `src-tauri/src/workflow/mod.rs`
**修改**:
- `schedule_execution` 将工作流实例加入内存队列（`VecDeque`）
- `WorkflowEngine` 在 `lib.rs` setup 中启动一个后台 tokio task，定期从队列取出实例并执行
- 复用 `TaskService` 的 executor registry，或直接在 workflow 模块内实现简单的串行执行器
**验收**:
- 调用 `schedule_execution` 后，工作流实例的节点状态从 `Pending` → `Running` → `Completed`
- 新增 workflow 集成测试
**工期**: 90 分钟

---

## 三、Phase 2: P1 功能补全（Day 2）

### 任务 2.1: 扩展 `PromptLibrary`
**文件**: `src-tauri/src/prompts/engine.rs`, 新建 `src-tauri/src/prompts/library.rs`
**修改**:
- 新建 `library.rs`，集中定义所有 Agent 的系统提示词模板:
  - `inspector_system_template()`
  - `outline_planner_system_template()`
  - `style_checker_system_template()`
  - `commentator_system_template()`
- 将现有硬编码提示词迁移到模板
**验收**:
- 所有 Agent 的 `system_prompt` 都通过 `PromptLibrary` 获取
- `cargo test` 通过
**工期**: 60 分钟

---

### 任务 2.2: 创建方法论提示词模板目录结构
**文件**: 新建 `src-tauri/src/prompts/methodologies/`
**修改**:
- 新建目录结构:
  ```
  src-tauri/src/prompts/methodologies/
  ├── snowflake.rs      # 雪花法 10 步提示词
  ├── hero_journey.rs   # 英雄之旅 12 阶段
  ├── scene_structure.rs # 场景结构（目标-冲突-灾难-反应-困境-决定）
  └── mod.rs            # 统一导出
  ```
- 每个方法论提供 `get_step_prompt(step: usize) -> &'static str`
- 在 `MethodologyEngine` 中接入（如已存在）
**验收**:
- 编译通过，方法论模板可通过代码引用
- 不强制要求所有 Agent 立即接入（Phase 3 完成）
**工期**: 45 分钟

---

### 任务 2.3: 移除 `FrontstageToolbar` 显式"AI 续写"按钮
**文件**: `src-frontend/src/frontstage/components/FrontstageToolbar.tsx`
**修改**:
- 删除 `toolbar-center` 中的 "AI 续写" 按钮
- `onRequestGeneration` prop 可保留用于 `/` 菜单调用
**验收**:
- 工具栏不再显示"AI 续写"按钮
- `Ctrl+Enter` 全局续写仍然可用
**工期**: 15 分钟

---

## 四、Phase 3: P2 优化与文档同步（Day 2-3）

### 任务 3.1: 修正 AGENTS.md / CHANGELOG SyncEvent 数量
**文件**: `AGENTS.md`, `CHANGELOG.md`
**修改**:
- 将 "18 种 `SyncEvent`" 改为 "16 种"
- 列出完整的事件类型清单
**工期**: 10 分钟

---

### 任务 3.2: 为 `QueryPipeline` 降级添加前端事件
**文件**: `src-tauri/src/agents/commands.rs`, `src-frontend/src/frontstage/FrontstageApp.tsx`
**修改**:
- `build_with_query` 降级时，除了 `log::warn!`，发射一个 `context-degraded` 事件到前端
- 前端可选显示轻量提示（如 "正在使用基础上下文"）
**工期**: 20 分钟

---

### 任务 3.3: `task_system` 调度器增强超时/重试
**文件**: `src-tauri/src/task_system/scheduler.rs`
**修改**:
- `spawn_interval` 中的 callback 包装超时逻辑（使用 `tokio::time::timeout`）
- 失败时记录日志，可选重试 3 次
**工期**: 30 分钟

---

### 任务 3.4: 更新版本号与构建
**文件**: `Cargo.toml`, `package.json`, `tauri.conf.json`, `AGENTS.md`
**修改**:
- 版本号统一更新为 `5.1.1`
- 执行 `cargo check`, `cargo test`, `npm run build`
- 本地构建 Windows `.exe` / `.msi`
**工期**: 60 分钟

---

## 五、验收检查清单

### Phase 1 验收（P0）
- [ ] 点击幕前工具栏"幕后"按钮 → 幕后自动定位并高亮当前故事
- [ ] 修改章节内容并保存 → 5 秒内知识图谱更新（可通过知识图谱页面验证）
- [ ] 新建章节 → 知识图谱自动更新
- [ ] 后台 workflow 实例可提交到调度器并实际执行（节点状态变化）
- [ ] `cargo test` 193/193 通过

### Phase 2 验收（P1）
- [ ] `PromptLibrary` 包含 writer + inspector + planner + style_checker 模板
- [ ] `prompts/methodologies/` 目录结构存在，包含 snowflake / hero_journey / scene_structure
- [ ] 幕前工具栏无显式"AI 续写"按钮
- [ ] `npm run build` 通过

### Phase 3 验收（P2）
- [ ] 文档中 SyncEvent 数量准确（16 种）
- [ ] 版本号统一为 5.1.1
- [ ] `StoryForge_5.1.1_x64-setup.exe` 和 `.msi` 已生成并复制到根目录

---

## 六、风险与回滚

| 风险 | 缓解措施 |
|------|----------|
| Ingest 自动触发导致性能问题 | 使用 `tokio::spawn` 后台异步执行，不阻塞 save 响应 |
| Workflow 调度器实现引入死锁 | 复用已有 `TaskScheduler` 模式，单线程串行执行节点 |
| 提示词模板迁移导致 Agent 行为变化 | 保持原有硬编码字符串作为 fallback，新模板逐步切换 |

---

*本计划经批准后，将按 Phase 顺序逐任务实施，每阶段提交一次 commit。*
