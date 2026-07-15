# StoryForge 迁移后续改进设计

> 日期：2026-07-14
> 状态：已确认，待实施
> 关联功能：StoryForge → StoryMoss 数据自动迁移

## 背景

首次实现的自动迁移已在 `src-tauri/src/migration/storyforge.rs` 中完成，并在 setup 阶段于数据库初始化前执行。代码审查与风险评估后，发现以下三个可改进点：

1. 迁移反复失败会在每次启动时生成新的 timestamp 备份目录，可能无限堆积。
2. SQLite 合并未显式处理外键约束，若目标库启用了 foreign keys，子表可能先于父表插入而失败。
3. 数据检测只检查 `cinema_ai.db` 与 `config.json`，纯文件型旧数据可能被漏检。

## 目标

- 防止迁移失败导致备份无限增长。
- 确保外键约束开启时数据库合并仍能成功。
- 只要旧目录存在用户数据就触发迁移。

## 设计方案

### 1. 失败标记防止无限重试

新增常量：

```rust
const MIGRATION_FAILED_MARKER: &str = ".storyforge_migration_failed";
```

行为：

- `run_storyforge_migration` 返回 `Err` 时，立即写入失败标记。
- `migration_needed` 在检查 `.storyforge_migrated` 之后，继续检查 `.storyforge_migration_failed`；任一标记存在均视为不需要迁移。
- 失败标记与成功标记路径均通过 `migration_marker_path` 的父目录逻辑创建。
- 用户修复旧数据或磁盘问题后，手动删除失败标记即可让应用在下次启动时重试。

### 2. 合并数据库时禁用外键

在 `merge_sqlite_databases` 中：

- `BEGIN IMMEDIATE` 之前执行 `PRAGMA foreign_keys = OFF;`
- `COMMIT` 成功后执行 `PRAGMA foreign_keys = ON;`
- 若合并失败执行 `ROLLBACK`，在回滚后同样执行 `PRAGMA foreign_keys = ON;` 恢复默认

这样可避免因插入顺序导致的外键冲突，同时不影响应用启动后的数据库外键行为。

### 3. 非空旧目录即视为有数据

将 `has_storyforge_data` 从检测特定文件改为：

```rust
pub fn has_storyforge_data(app_handle: &AppHandle) -> bool {
    let Some(old) = storyforge_data_dir(app_handle) else {
        return false;
    };
    if !old.is_dir() {
        return false;
    }
    // 只要旧目录非空（包含任意文件或子目录）即认为有数据
    match old.read_dir() {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}
```

## 测试方案

- `migration_needed_returns_false_when_failed_marker_exists`：写入失败标记后断言 `migration_needed` 为 false。
- `merge_sqlite_with_foreign_keys_disabled`：构造含外键依赖的两个表，子表在旧库有数据、父表也有数据，验证合并成功。
- `has_storyforge_data_true_for_non_empty_dir`：空目录返回 false，含任意文件的目录返回 true。

## 相关文件

- `src-tauri/src/migration/storyforge.rs`
- `src-tauri/src/migration/tests.rs`
- `CHANGELOG.md`
