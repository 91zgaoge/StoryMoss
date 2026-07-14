# StoryForge → StoryMoss 数据自动迁移设计

> 实现状态：已完成（自动迁移，无弹窗）。原始设计包含前端确认弹窗，后因 Tauri setup 阶段无法可靠地等待前端响应、且迁移必须在数据库初始化之前完成，改为在 setup 中自动执行迁移。

## 背景

产品从 StoryForge 更名为 StoryMoss 后，应用 bundle identifier 从 `com.storyforge.app` 变为 `com.storymoss.app`，导致老用户升级后无法访问原有数据。本方案在 StoryMoss 首次启动时自动检测旧数据并导入全部配置与数据。

## 目标

- 自动检测旧版 `com.storyforge.app` 数据目录。
- 首次启动时**自动**导入全部配置与数据，无需用户确认、无需重启。
- 迁移内容包括 `app_data_dir` 下的全部文件与目录（数据库、配置、stories、exports、vector_db、workflows、templates、logs 等）。
- 对 `cinema_ai.db` 和 `config.json` 执行合并：StoryMoss 已存在的数据优先保留，旧数据补充缺失内容。
- 迁移完成后保留旧目录，不删除。
- 写入迁移标记，避免重复执行。

## 旧数据目录定位

| 平台 | 旧目录路径 |
|------|-----------|
| macOS | `~/Library/Application Support/com.storyforge.app` |
| Windows | `%LOCALAPPDATA%\com.storyforge.app` |
| Linux | `~/.local/share/com.storyforge.app` |

实现方式：使用 Tauri `app.path().app_data_dir()` 获取当前 `com.storymoss.app` 目录，再替换末级目录名得到旧目录路径。

## 检测与执行时机

在 `src-tauri/src/lib.rs` 的 `setup` 钩子中：

1. 创建 `app_data_dir`。
2. 初始化日志。
3. **执行自动迁移**（在数据库初始化之前，避免锁定新库）。
   - 若检测到旧数据且未写入标记，则直接调用 `run_storyforge_migration`。
   - 迁移失败记录日志，但不阻塞启动，让用户在空/新数据上继续。
4. 初始化数据库等后续服务。

检测条件：
- 旧目录存在。
- 旧目录下存在 `cinema_ai.db` 或 `config.json` 等核心文件。
- 新目录下不存在 `.storyforge_migrated` 标记文件。

## 后端迁移模块

新增 `src-tauri/src/migration/mod.rs` 与 `src-tauri/src/migration/storyforge.rs`。

### 暴露接口

- `run_storyforge_migration(app_handle: &AppHandle) -> Result<MigrationResult, String>`：同步执行完整迁移，供 setup 调用。
- `check_storyforge_migration(app_handle: AppHandle) -> Result<MigrationStatus, String>`：查询是否需要迁移，保留作诊断用途（当前无前端调用）。

### 迁移步骤

1. **备份**：将当前 `com.storymoss.app` 目录复制到 `com.storymoss.app.bak.<timestamp>`（复制式备份，不移动原目录）。
2. **文件复制**：递归复制旧目录到新目录，已存在文件跳过（新数据优先）。
3. **数据库合并**：
   - 在新库上 `ATTACH DATABASE old_db AS legacy`。
   - 读取 `legacy.sqlite_master` 获取所有用户表。
   - 跳过目标库不存在的表。
   - 对每个共有的表，按**共有列**执行 `INSERT OR IGNORE INTO table (<common_cols>) SELECT <common_cols> FROM legacy.table`，避免旧库缺少新列导致失败。
   - 处理 `sqlite_sequence`（`INSERT OR IGNORE`）。
   - `DETACH legacy`。
4. **配置合并**：读取新旧 `config.json`，递归合并对象，已有键保留新值；目标配置格式无效时返回错误。
5. **标记**：在新目录创建 `.storyforge_migrated` 空文件。
6. **清理**：迁移成功后删除备份目录；失败时用备份恢复或清理部分创建的内容。

### 错误处理

- 任何步骤失败立即中止，回滚到备份状态。
- 失败仅记录日志，不阻塞应用启动（避免首次启动卡死）。

## 前端

无专门 UI。迁移在后台 setup 阶段自动完成，用户下次启动即可在 StoryMoss 中看到原有数据。

## 测试方案

1. **单元测试**：构造临时旧目录与新目录，验证：
   - 文件复制跳过已存在文件。
   - 数据库 `INSERT OR IGNORE` 合并结果正确。
   - 数据库按共有列合并，跳过目标库不存在的表。
   - 配置合并保留新值，且 malformed 目标配置会报错。
   - 失败回滚恢复原始新目录。
2. **手动测试**：
   - 在测试机上保留旧 `com.storyforge.app` 目录，安装 StoryMoss，确认数据自动导入。
   - 再次启动，确认不再执行迁移。

## 启动时序

迁移全部在 `setup` 中同步完成：

1. 第一次启动：检测旧数据 → 自动执行迁移 → 写入标记 → 继续初始化数据库 → 应用在迁移后的数据上运行。
2. 第二次启动：检测到标记 → 正常初始化数据库。

## 相关文件变更

- 新增：`src-tauri/src/migration/mod.rs`
- 新增：`src-tauri/src/migration/storyforge.rs`
- 修改：`src-tauri/src/lib.rs`（setup 中调用自动迁移）
- 修改：`src-tauri/src/handlers.rs`（保留诊断命令）
- 修改：`CHANGELOG.md`
