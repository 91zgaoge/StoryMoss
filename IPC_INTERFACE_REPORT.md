# StoryMoss 前后端 IPC 接口对比报告

> 生成时间: 2026-04-22
> 工具: Python 正则提取 + 手动校验

## 统计概览

| 类别 | 数量 |
|------|------|
| 后端命令总数 | 194 |
| 前端 invoke 调用总数 | 179 |
| **前后端对应** | **179** |
| 仅后端实现（前端未调用） | 15 |
| 仅前端调用（后端未实现） | **0** |

---

## 1. 前后端都有的命令 (179个) [OK]

所有前端调用的命令后端均有实现，核心接口对应良好。

主要调用分布:
- `services/tauri.ts` — 核心服务封装 (约80+个命令)
- `services/settings.ts` — 设置相关 (约10个命令)
- `hooks/` — 各业务 Hooks (约60+个命令)
- `components/` / `pages/` — 组件直接调用 (约20个命令)

---

## 2. 只有后端有的命令 (15个) [前端未调用]

| 命令 | 所在文件 | 分析 |
|------|---------|------|
| `agent_execute` | `agents/commands.rs` | 同步执行版本，前端使用 `agent_execute_stream` 流式版本 |
| `agent_get_status` | `agents/commands.rs` | Agent 状态查询，前端未使用 |
| `get_subscription_status` | `subscription/commands.rs` | 获取当前订阅状态（Free/Pro/Enterprise） |
| `dev_upgrade_subscription` | `subscription/commands.rs` | 模拟升级订阅（开发测试用） |
| `dev_downgrade_subscription` | `subscription/commands.rs` | 模拟降级订阅（开发测试用） |
| `get_available_agents` | `agents/service.rs` | 获取可用 Agent 列表，前端未使用 |
| `get_entity_relations` | `commands_v3.rs` | 知识图谱关系查询，前端未使用 |
| `get_story_entities` | `commands_v3.rs` | 知识图谱实体查询，前端未使用 |
| `get_task` | `task_system/commands.rs` | 获取单个任务，前端只使用 `list_tasks` |
| `init_llm` | `llm/commands.rs` | LLM 初始化命令，前端通过设置流程隐式触发 |
| `llm_generate` | `llm/commands.rs` | 同步生成，前端使用 `llm_generate_stream` 流式版本 |
| `llm_test_connection` | `llm/commands.rs` | LLM 连接测试，前端使用 `test_model_connection` |
| `notify_backstage_generation_requested` | `lib.rs` | 内部通知命令 |
| `notify_frontstage_content_changed` | `lib.rs` | 内部通知命令 |
| `open_update_settings` | `updater/mod.rs` | 打开更新设置，前端未使用 |
| `toggle_frontstage` | `window/mod.rs` | 切换窗口，前端使用 `show_frontstage` / `hide_frontstage` |
| `update_frontstage_content` | `window/mod.rs` | 更新窗口内容，内部命令 |

**结论**: 这15个命令大部分是内部命令、同步版本（前端改用流式版本）、或被更细粒度的命令替代。属于合理差异，无功能缺失风险。

---

## 3. 只有前端有的命令 (0个) [后端未实现]

**无。** 所有前端调用的命令后端均有实现。

---

## 4. 参数命名不匹配问题 (3处) [需要修复]

以下调用存在 **camelCase vs snake_case** 参数名不匹配问题。在 Tauri v2 中，如果前后端参数名不一致，会导致反序列化失败（参数值为空）。

### 4.1 `upload_book` — 高风险

**前端调用** (`src-frontend/src/hooks/useBookDeconstruction.ts:24`):
```typescript
const bookId: string = await invoke('upload_book', { filePath });
```

**后端签名** (`src-tauri/src/book_deconstruction/commands.rs:13`):
```rust
pub async fn upload_book(file_path: String, app_handle: AppHandle) -> Result<String, String> {
```

**问题**: 前端传 `filePath` (camelCase)，后端期望 `file_path` (snake_case)。
**修复建议**: 前端改为 `{ file_path: filePath }`。

---

### 4.2 `convert_book_to_story` — 高风险

**前端调用** (`src-frontend/src/hooks/useBookDeconstruction.ts:219`):
```typescript
const storyId: string = await invoke('convert_book_to_story', { bookId });
```

**后端签名** (`src-tauri/src/book_deconstruction/commands.rs:75`):
```rust
pub async fn convert_book_to_story(book_id: String, app_handle: AppHandle) -> Result<String, String> {
```

**问题**: 前端传 `bookId` (camelCase)，后端期望 `book_id` (snake_case)。
**修复建议**: 前端改为 `{ book_id: bookId }`。

---

### 4.3 `set_active_model` — 高风险

**前端调用** (`src-frontend/src/services/settings.ts:165`):
```typescript
return invoke('set_active_model', { modelType: type, modelId });
```

**后端签名** (`src-tauri/src/config/commands.rs:496`):
```rust
pub fn set_active_model(model_type: String, model_id: String, app_handle: AppHandle) -> Result<(), String> {
```

**问题**: 前端传 `modelType` / `modelId` (camelCase)，后端期望 `model_type` / `model_id` (snake_case)。
**修复建议**: 前端改为 `{ model_type: type, model_id: modelId }`。

---

## 总结

| 检查项 | 结果 |
|--------|------|
| 命令名称对应 | 179/179 完全对应 |
| 后端命令未使用 | 15个（均为内部/替代命令，合理） |
| 前端命令缺失后端 | 0个 |
| **参数名不匹配** | **3处，需要修复** |

**建议优先修复**: `upload_book`、`convert_book_to_story`、`set_active_model` 的参数命名问题，这会导致功能在 Tauri v2 运行时静默失败。

