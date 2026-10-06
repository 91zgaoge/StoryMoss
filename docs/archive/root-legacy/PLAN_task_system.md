# StoryMoss 任务系统 + 拆书改进 + 向量化存储 实施计划

> 参考 memoh-X 项目 (`github.com/91zgaoge/memoh-X`) 的 `internal/schedule`、`internal/heartbeat`、`internal/automation/cron_pool` 源代码设计。

---

## 一、需求总览

| # | 需求 | 说明 |
|---|------|------|
| 1 | **任务系统** | 幕后界面新增「任务」页面。支持一次性/定时任务（每天/每周/cron表达式），用于需要子agent单独完成的无需中途中止或协同的工作 |
| 2 | **拆书改为任务** | 每次拆书分析创建为一个任务实例，通过任务系统执行。任务有心跳机制，可被检测感应 |
| 3 | **向量化存储** | 拆书分析结果接入 LanceVectorStore，生成 embedding 存入向量库 |
| 4 | **构建脚本修复** | 解决 `scripts/build-local.ps1` 第37行中文 `&&` 在 PowerShell 中的解析错误 |

---

## 二、架构设计

### 2.1 数据模型

```sql
-- 任务表（参考 memoh schedule + heartbeat 融合设计）
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    task_type TEXT NOT NULL,        -- 'book_deconstruction' | 'custom' | ...
    schedule_type TEXT NOT NULL,    -- 'once' | 'daily' | 'weekly' | 'cron'
    cron_pattern TEXT,              -- cron表达式（schedule_type='cron'时）
    payload TEXT,                   -- JSON: 任务参数
    status TEXT NOT NULL DEFAULT 'pending',  -- pending/running/completed/failed/cancelled
    progress INTEGER DEFAULT 0,     -- 0-100
    result TEXT,                    -- JSON: 执行结果
    error_message TEXT,
    max_retries INTEGER DEFAULT 3,
    retry_count INTEGER DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,      -- 定时任务是否启用
    last_run_at TEXT,
    next_run_at TEXT,               -- 定时任务下次执行时间
    last_heartbeat_at TEXT,         -- 心跳时间戳
    heartbeat_timeout_seconds INTEGER DEFAULT 300, -- 默认5分钟超时
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_type ON tasks(task_type);
CREATE INDEX idx_tasks_enabled ON tasks(enabled);
CREATE INDEX idx_tasks_next_run ON tasks(next_run_at);

-- 任务执行日志表
CREATE TABLE task_logs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    log_level TEXT NOT NULL,        -- info | warn | error
    message TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);
CREATE INDEX idx_task_logs_task ON task_logs(task_id);
```

### 2.2 后端模块结构

```
src-tauri/src/
├── task_system/
│   ├── mod.rs           -- 模块导出
│   ├── models.rs        -- Task, TaskCreateRequest, TaskUpdateRequest 等类型
│   ├── repository.rs    -- TaskRepository CRUD + 批量日志写入
│   ├── scheduler.rs     -- TaskScheduler: 基于 tokio::time 的定时调度器
│   ├── heartbeat.rs     -- HeartbeatMonitor: 心跳检测引擎
│   ├── executor.rs      -- TaskExecutor: 任务执行trait + 拆书执行器
│   ├── service.rs       -- TaskService: 业务层整合
│   └── commands.rs      -- Tauri IPC commands (7个)
```

### 2.3 心跳机制设计（参考 memoh heartbeat engine）

- **任务执行时**：每 30 秒更新 `last_heartbeat_at` 字段
- **心跳检测器**：每 60 秒扫描所有 `status='running'` 的任务
  - 若 `now - last_heartbeat_at > heartbeat_timeout_seconds` → 标记为 `failed`，记录错误日志
  - 若 `retry_count < max_retries` → 自动重试（ reschedule）
- **前端检测**：轮询(5s) + Tauri 事件监听(`task-heartbeat`, `task-status-changed`)

### 2.4 调度器设计（参考 memoh CronPool，简化版）

使用 **tokio::time** 而非引入 `robfig/cron` crate（减少依赖）：

- `once` 任务：立即执行或指定时间执行（一次性）
- `daily` 任务：`@every 86400s`，下次运行时间 = 上次 + 1天
- `weekly` 任务：`@every 604800s`
- `cron` 任务：用简单解析器分解为秒级定时器

每个注册任务有独立 `tokio::task::JoinHandle`，支持取消。同一任务互斥锁防止重叠执行。

---

## 三、分阶段实施计划

### Phase 1: 任务系统核心（后端 + 前端页面）

**后端 (Rust)**
1. **Migration 17**: 在 `db/connection.rs` 中添加 `tasks` 表和 `task_logs` 表创建
2. **models.rs**: 定义 `Task`, `TaskStatus`, `ScheduleType`, `TaskCreateRequest`, `TaskUpdateRequest`, `TaskLog`
3. **repository.rs**: `TaskRepository` — create/get/list/update/delete/create_log
4. **scheduler.rs**: `TaskScheduler` — 
   - `register(task)` 根据 schedule_type 创建 tokio 定时器
   - `unregister(task_id)` 取消定时器
   - `run_now(task_id)` 立即执行任务
   - 内部维护 `HashMap<String, JoinHandle>` 和 `HashMap<String, Arc<Mutex>>`
5. **heartbeat.rs**: `HeartbeatMonitor` — 
   - `start()` 启动检测循环（每60秒）
   - `check_all()` 扫描 running 任务，检测超时
   - `record_heartbeat(task_id)` 更新心跳时间
6. **executor.rs**: `TaskExecutor` trait + `BookDeconstructionExecutor`
   - trait: `async fn execute(&self, task: &Task) -> Result<TaskResult, Error>`
   - 拆书执行器：将现有 `BookAnalyzer::analyze()` 包装为任务执行
7. **service.rs**: `TaskService` — 整合 Repository + Scheduler + Heartbeat + Executor
   - `create_task()`, `update_task()`, `delete_task()`, `list_tasks()`
   - `trigger_task()`, `cancel_task()`, `get_task_logs()`
   - `bootstrap()` 启动时加载所有 enabled 任务
8. **commands.rs**: 7个 IPC 命令
   - `create_task`, `update_task`, `delete_task`, `list_tasks`, `get_task`, `trigger_task`, `get_task_logs`

**前端 (React/TS)**
1. 扩展 `ViewType` 增加 `'tasks'`
2. `src/pages/Tasks.tsx` — 任务列表页面（参考 memoh schedules 页面设计）
   - 分组：运行中 / 已完成 / 失败 / 定时任务
   - 每行：开关(enable/disable)、名称、类型、状态、进度条、心跳指示器、删除
   - 空状态引导
3. `src/hooks/useTasks.ts` — TanStack Query hooks
   - `useTasks()`, `useCreateTask()`, `useUpdateTask()`, `useDeleteTask()`, `useTaskLogs()`
4. `Sidebar.tsx` + `App.tsx` — 追加导航入口（icon: `ListChecks`）

### Phase 2: 拆书改为任务执行 + 心跳

**后端修改**
1. 修改 `BookDeconstructionService::upload_and_analyze()`:
   - 不再直接 spawn 分析，而是创建一个 `task_type='book_deconstruction'` 的任务
   - 返回 task_id 给前端
2. 修改 `BookDeconstructionExecutor`:
   - 每完成一个分析步骤，调用 `record_heartbeat(task_id)`
   - 每完成一步，通过 `app_handle.emit("task-progress", ...)` 推送进度
   - 分析完成后更新 task status = completed，写入 result
3. 保留原有进度事件 `book-analysis-progress` 以兼容前端（或改为 task-progress 统一通道）

**前端修改**
1. `BookDeconstruction.tsx`:
   - 上传后显示「任务已创建」状态，通过任务状态轮询替代直接轮询 book status
   - 进度条来源从 `book-analysis-progress` 改为 `task-progress`
   - 心跳指示器：显示最后心跳时间，超时变红

### Phase 3: 向量化存储

**后端**
1. 复用现有 `LanceVectorStore` (`src-tauri/src/vector/lancedb_store.rs`)
2. 复用现有 `EmbeddingProvider` (`src-tauri/src/embeddings/`)
3. 在 `BookDeconstructionExecutor` 分析完成后：
   - 对 `reference_scenes` 的每条 summary 生成 embedding
   - 对 `reference_characters` 的 personality/description 生成 embedding
   - 调用 `LanceVectorStore::upsert()` 存入（record_type = 'reference_scene' / 'reference_character'）
4. `store_embeddings()` 接口从预留变为实际实现

### Phase 4: 构建脚本修复

- `scripts/build-local.ps1` 第37行：
  ```powershell
  # 修复前
  Write-Host "   请手动在 WSL 中运行: cd $tauriDir && cargo tauri build" -ForegroundColor DarkGray
  # 修复后
  Write-Host "   请手动在 WSL 中运行: cd $tauriDir ; cargo tauri build" -ForegroundColor DarkGray
  ```
- 同时检查 `.github/workflows/build.yml` 是否有类似问题

---

## 四、接口契约

### Tauri IPC Commands

```rust
// task_system/commands.rs
#[tauri::command]
async fn create_task(
    name: String,
    description: Option<String>,
    task_type: String,
    schedule_type: String,
    cron_pattern: Option<String>,
    payload: Option<String>,
    state: State<'_, AppState>,
) -> Result<Task, String>

#[tauri::command]
async fn update_task(id: String, ...)

#[tauri::command]
async fn delete_task(id: String, ...)

#[tauri::command]
async fn list_tasks(
    status_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, String>

#[tauri::command]
async fn get_task(id: String, ...) -> Result<Task, String>

#[tauri::command]
async fn trigger_task(id: String, ...) -> Result<(), String>

#[tauri::command]
async fn get_task_logs(task_id: String, ...) -> Result<Vec<TaskLog>, String>
```

### Tauri Events (前端监听)

```
task-created       { task_id, name }
task-status-changed { task_id, status, progress }
task-progress      { task_id, step, progress, message }
task-heartbeat     { task_id, timestamp }
task-failed        { task_id, error, retry_count }
```

---

## 五、文件变更清单

### 新增文件
| 文件 | 说明 |
|------|------|
| `src-tauri/src/task_system/mod.rs` | 模块入口 |
| `src-tauri/src/task_system/models.rs` | 数据模型 |
| `src-tauri/src/task_system/repository.rs` | 数据库操作 |
| `src-tauri/src/task_system/scheduler.rs` | tokio定时调度 |
| `src-tauri/src/task_system/heartbeat.rs` | 心跳检测 |
| `src-tauri/src/task_system/executor.rs` | 任务执行器 |
| `src-tauri/src/task_system/service.rs` | 业务服务 |
| `src-tauri/src/task_system/commands.rs` | IPC命令 |
| `src-frontend/src/pages/Tasks.tsx` | 任务页面 |
| `src-frontend/src/hooks/useTasks.ts` | 前端hooks |

### 修改文件
| 文件 | 修改内容 |
|------|----------|
| `src-tauri/src/db/connection.rs` | Migration 17: tasks + task_logs 表 |
| `src-tauri/src/lib.rs` | 注册 task_system 模块和命令 |
| `src-tauri/src/book_deconstruction/service.rs` | upload_and_analyze 改为创建任务 |
| `src-tauri/src/book_deconstruction/analyzer.rs` | 接入心跳记录 + 进度推送 |
| `src-tauri/src/book_deconstruction/mod.rs` | 导出 store_embeddings 实现 |
| `src-frontend/src/types/index.ts` | ViewType += 'tasks' |
| `src-frontend/src/App.tsx` | 渲染 Tasks 页面 |
| `src-frontend/src/components/Sidebar.tsx` | 导航追加「任务」 |
| `scripts/build-local.ps1` | `&&` → `;` |

---

## 六、测试策略

1. **单元测试** (Rust)
   - `TaskRepository`: CRUD + 日志写入
   - `TaskScheduler`: 注册/取消/立即执行
   - `HeartbeatMonitor`: 超时检测 + 重试
2. **集成测试**
   - 创建一次性任务 → 执行 → 完成
   - 创建定时任务 → 等待触发 → 验证
   - 模拟心跳中断 → 检测失败 → 自动重试
   - 拆书流程：上传 → 创建任务 → 任务执行 → 心跳正常 → 完成
3. **前端测试**
   - 任务列表渲染
   - 创建/删除/触发交互
   - 心跳指示器状态变化
4. **E2E**
   - 上传小说 → 作为任务执行 → 完成后可查看分析结果

---

## 七、依赖评估

| 依赖 | 是否需要新增 | 说明 |
|------|------------|------|
| `tokio` | ❌ 已有 | 用于定时器和并发 |
| `chrono` | ❌ 已有 | 时间处理 |
| `serde_json` | ❌ 已有 | JSON序列化 |
| `uuid` | ❌ 已有 | ID生成 |
| `cron` crate | ❌ 不需要 | 用 tokio::time 简化实现 |
| `lancedb_store` | ❌ 已有 | 向量存储 |
| `embeddings` | ❌ 已有 | embedding生成 |

**无需新增任何 Rust 依赖。**

---

## 八、预计工作量

| Phase | 预计时间 | 复杂度 |
|-------|---------|--------|
| Phase 1: 任务系统核心 | 3-4h | 高 |
| Phase 2: 拆书接入任务 | 1.5h | 中 |
| Phase 3: 向量化存储 | 1h | 低 |
| Phase 4: 构建脚本修复 | 10min | 极低 |
| 测试 + 构建 + 推送 | 1-2h | 中 |
| **总计** | **~7-8h** | — |

---

## 九、风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 定时任务在应用关闭后丢失 | SQLite 持久化 + Bootstrap 重启时恢复 |
| 心跳检测误报（任务正常但心跳延迟） | 设置合理超时(5min) + 前端展示最后心跳时间 |
| 大量定时任务性能问题 | 限制最大并发数(10)，超出的任务排队 |
| 拆书改为任务后前端状态同步问题 | 保留原有事件名兼容 + 新增统一 task-progress 通道 |
