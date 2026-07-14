# StoryForge → StoryMoss 数据自动迁移设计

## 背景

产品从 StoryForge 更名为 StoryMoss 后，应用 bundle identifier 从 `com.storyforge.app` 变为 `com.storymoss.app`，导致老用户升级后无法访问原有数据。本方案在 StoryMoss 首次启动时自动检测旧数据，经用户确认后导入全部配置与数据。

## 目标

- 自动检测旧版 `com.storyforge.app` 数据目录。
- 首次启动时弹出确认弹窗，用户点击后执行迁移。
- 迁移内容包括 `app_data_dir` 下的全部文件与目录（数据库、配置、stories、exports、vector_db、workflows、templates、logs 等）。
- 对 `cinema_ai.db` 和 `config.json` 执行合并：StoryMoss 已存在的数据优先保留，旧数据补充缺失内容。
- 迁移完成后保留旧目录，不删除。
- 写入迁移标记，避免重复弹窗。

## 旧数据目录定位

| 平台 | 旧目录路径 |
|------|-----------|
| macOS | `~/Library/Application Support/com.storyforge.app` |
| Windows | `%LOCALAPPDATA%\com.storyforge.app` |
| Linux | `~/.local/share/com.storyforge.app` |

实现方式：使用 Tauri `app.path().app_data_dir()` 获取当前 `com.storymoss.app` 目录，再替换末级目录名得到旧目录路径。

## 检测时机

在 `src-tauri/src/lib.rs` 的 `setup` 钩子中：

1. 创建 `app_data_dir`。
2. 初始化日志。
3. **执行迁移检测**（在数据库初始化之前，避免锁定新库）。
4. 若满足迁移条件，通过 Tauri 事件通知前端。
5. 前端显示确认弹窗，等待用户操作。

检测条件：
- 旧目录存在。
- 旧目录下存在 `cinema_ai.db` 或 `config.json` 等核心文件。
- 新目录下不存在 `.storyforge_migrated` 标记文件。

## 后端迁移模块

新增 `src-tauri/src/migration/mod.rs` 与 `src-tauri/src/migration/storyforge.rs`。

### 暴露命令

- `check_storyforge_migration()` → `{ needed: bool, source_path: string }`
- `migrate_storyforge_data()` → `{ success: bool, message: string }`

### 迁移步骤

1. **备份**：将当前 `com.storymoss.app` 重命名为 `com.storymoss.app.bak.<timestamp>`。
2. **文件复制**：递归复制旧目录到新目录，已存在文件跳过（新数据优先）。
3. **数据库合并**：
   - 在新库上 `ATTACH DATABASE old_db AS old`。
   - 读取 `sqlite_master` 获取所有用户表。
   - 对每个表执行 `INSERT OR IGNORE INTO table SELECT * FROM old.table`。
   - 处理 `sqlite_sequence`（`UPDATE OR IGNORE`）。
   - `DETACH old`。
   - 删除旧库附加的 `-wal` / `-shm`（已复制到新目录，合并后不再需要）。
4. **配置合并**：读取新旧 `config.json`，递归浅合并，已有键保留新值。
5. **标记**：在新目录创建 `.storyforge_migrated` 空文件。
6. **清理**：迁移成功后删除备份目录；失败时恢复备份。

### 错误处理

- 任何步骤失败立即中止，回滚到备份状态。
- 返回可读的失败原因。
- 前端弹窗展示失败信息，用户可选择跳过或重试。

## 前端弹窗

新增 `src-frontend/src/components/StoryForgeMigrationDialog.tsx`。

- 在 `App.tsx` 顶层监听 `storyforge-migration-prompt` 事件。
- 弹窗文案：
  > 检测到旧版 StoryForge 数据
  > 是否将配置、故事和数据库全部导入到 StoryMoss？
  > 导入后原 StoryForge 数据仍会保留。
- 按钮：「立即导入」「跳过」。
- 无论导入还是跳过，都写入迁移标记，避免下次启动再弹。

## 测试方案

1. **单元测试**：构造临时旧目录与新目录，验证：
   - 文件复制跳过已存在文件。
   - 数据库 `INSERT OR IGNORE` 合并结果正确。
   - 配置合并保留新值。
   - 失败回滚恢复原始新目录。
2. **集成测试**：使用 Tauri `mock_app` 验证命令注册与事件发射。
3. **手动测试**：
   - 在测试机上保留旧 `com.storyforge.app` 目录，安装 StoryMoss，确认弹窗出现。
   - 点击导入后，确认数据完整。
   - 再次启动，确认不再弹窗。

## 启动时序与重启策略

由于 Tauri 的 `setup` 钩子在前端 WebView 可用之前同步执行，迁移提示只能在 `setup` 中通过事件发出，而实际的文件复制、数据库合并等迁移操作必须等到前端弹窗渲染并由用户确认后才能执行。此时 `setup` 早已结束，数据库也已在 `init_db` 中初始化并可能被后续服务使用。

如果在这种情况下直接完成迁移，应用将面对以下问题：

- 数据库连接池、向量存储、任务系统、模型网关等组件已经在旧（空或部分）数据上初始化。
- 迁移写入的新数据对这些已启动的服务不可见，可能导致缓存不一致、WAL 锁定或操作的是过期数据。

因此，我们在迁移成功并写入 `.storyforge_migrated` 标记后，让后端在 `MigrationResult` 中返回 `needs_restart: true`，前端弹出成功提示并显示「立即重启」按钮。用户点击后调用 `process` 插件的 `relaunch()` 重启应用。再次启动时，`setup` 检测到 `.storyforge_migrated` 标记已存在，跳过弹窗，直接使用迁移后的数据执行 `init_db` 和后续初始化流程。

这一设计将“检测/提示”与“迁移/生效”分离：

1. 第一次启动：检测旧数据 → 发出提示 → 用户确认 → 执行迁移 → 写入标记 → 重启。
2. 第二次启动：检测到标记 → 正常初始化数据库 → 应用在迁移后的数据上运行。

## 相关文件变更

- 新增：`src-tauri/src/migration/mod.rs`
- 新增：`src-tauri/src/migration/storyforge.rs`
- 新增：`src-frontend/src/components/StoryForgeMigrationDialog.tsx`
- 修改：`src-tauri/src/lib.rs`（setup 中调用检测、注册 `tauri-plugin-process`）
- 修改：`src-tauri/src/handlers.rs`（注册命令）
- 修改：`src-tauri/Cargo.toml`（添加 `tauri-plugin-process`）
- 修改：`src-tauri/capabilities/main-capability.json`（添加 `process:default` 权限）
- 修改：`src-frontend/src/App.tsx`（监听事件并渲染弹窗）
- 修改：`src-frontend/package.json`（添加 `@tauri-apps/plugin-process`）
- 修改：`CHANGELOG.md`
