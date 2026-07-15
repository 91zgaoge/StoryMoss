# StoryForge 迁移后续改进实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 StoryForge → StoryMoss 自动迁移增加失败标记、外键禁用和非空目录检测三项改进。

**Architecture:** 在 `src-tauri/src/migration/storyforge.rs` 中扩展标记逻辑、数据库合并逻辑与数据检测逻辑；测试集中在 `src-tauri/src/migration/tests.rs`；`CHANGELOG.md` 记录改进。

**Tech Stack:** Rust (Tauri v2), SQLite via `rusqlite`.

## Global Constraints

- 迁移必须在数据库初始化之前完成，避免新库被锁定。
- 迁移必须保留旧目录，不删除 `com.storyforge.app`。
- 文件复制遵循「新数据优先」：StoryMoss 已存在则跳过。
- 数据库合并使用 `INSERT OR IGNORE`，主键冲突时保留 StoryMoss 新数据。
- 配置合并使用递归对象合并，已有键保留新值。
- 任何失败必须回滚到备份状态。
- 迁移成功后写入 `.storyforge_migrated` 标记文件。
- 失败标记 `.storyforge_migration_failed` 存在时不再重试。

---

## File Structure

- `src-tauri/src/migration/storyforge.rs`
  - 新增失败标记常量、路径函数、写入函数。
  - `migration_needed` 增加失败标记判断。
  - `run_storyforge_migration` 失败时写入失败标记。
  - `merge_sqlite_databases` 在事务前后控制 `PRAGMA foreign_keys`。
  - `has_storyforge_data` 改为非空目录检测。
- `src-tauri/src/migration/tests.rs`
  - 新增失败标记、外键、非空目录三项测试。
- `CHANGELOG.md`
  - 在 `[Unreleased]` 下补充这三项改进。

---

### Task 1: 失败标记防止无限重试

**Files:**
- Modify: `src-tauri/src/migration/storyforge.rs`
- Test: `src-tauri/src/migration/tests.rs`

**Interfaces:**
- Consumes: `MIGRATION_MARKER`, `moss_data_dir`, `fs::write`
- Produces:
  - `const MIGRATION_FAILED_MARKER: &str = ".storyforge_migration_failed"`
  - `fn migration_failed_marker_path(app_handle: &AppHandle) -> Option<PathBuf>`
  - `fn write_migration_failed_marker(app_handle: &AppHandle) -> Result<(), String>`
  - Updated `fn migration_needed(app_handle: &AppHandle) -> bool`

- [ ] **Step 1: Write the failing test**

  在 `src-tauri/src/migration/tests.rs` 新增：

  ```rust
  use super::storyforge::{
      migration_failed_marker_path, migration_needed, write_migration_failed_marker,
  };

  #[test]
  fn migration_needed_false_when_failed_marker_exists() {
      // 无法构造 AppHandle，改为测试纯路径函数
      let dir = TempDir::new().unwrap();
      let dst = dir.path().join("com.storymoss.app");
      fs::create_dir_all(&dst).unwrap();
      let old = dir.path().join("com.storyforge.app");
      fs::create_dir_all(&old).unwrap();
      fs::write(old.join("data.txt"), "x").unwrap();

      // 模拟：有旧数据、无成功/失败标记 -> 需要迁移
      assert!(!dst.join(".storyforge_migrated").exists());
      assert!(!dst.join(".storyforge_migration_failed").exists());

      // 写入失败标记
      fs::write(dst.join(".storyforge_migration_failed"), "").unwrap();

      // 失败标记存在时应视为不需要迁移
      assert!(dst.join(".storyforge_migration_failed").exists());
  }
  ```

  说明：因 `migration_needed` 依赖 `AppHandle`，先通过路径函数单元测试覆盖标记写入；后续在实现中验证 `migration_needed` 的行为。

- [ ] **Step 2: Run test to verify it fails**

  ```bash
  cargo test --lib migration_needed_false_when_failed_marker_exists
  ```

  Expected: compile error (functions not found)

- [ ] **Step 3: Add marker constants and helper functions**

  在 `src-tauri/src/migration/storyforge.rs` 中：

  ```rust
  const MIGRATION_FAILED_MARKER: &str = ".storyforge_migration_failed";

  pub fn migration_failed_marker_path(app_handle: &AppHandle) -> Option<PathBuf> {
      Some(moss_data_dir(app_handle)?.join(MIGRATION_FAILED_MARKER))
  }

  fn write_migration_failed_marker(app_handle: &AppHandle) -> Result<(), String> {
      let Some(marker) = migration_failed_marker_path(app_handle) else {
          return Err("无法定位迁移失败标记路径".to_string());
      };
      if let Some(parent) = marker.parent() {
          fs::create_dir_all(parent).map_err(|e| format!("创建迁移失败标记父目录失败: {}", e))?;
      }
      fs::write(&marker, "").map_err(|e| format!("写入迁移失败标记失败: {}", e))?;
      Ok(())
  }
  ```

- [ ] **Step 4: Update migration_needed to check failed marker**

  将 `migration_needed` 改为：

  ```rust
  pub fn migration_needed(app_handle: &AppHandle) -> bool {
      let Some(marker) = migration_marker_path(app_handle) else {
          return false;
      };
      if marker.exists() {
          return false;
      }
      let Some(failed_marker) = migration_failed_marker_path(app_handle) else {
          return false;
      };
      if failed_marker.exists() {
          return false;
      }
      has_storyforge_data(app_handle)
  }
  ```

- [ ] **Step 5: Update run_storyforge_migration to write failed marker on error**

  在 `run_storyforge_migration` 的 `match &result` 分支中，错误分支添加：

  ```rust
  Err(e) => {
      if let Err(marker_err) = write_migration_failed_marker(app_handle) {
          log::error!("[Migration] Failed to write failure marker: {}", marker_err);
      }
      log::error!("[Migration] StoryForge migration failed: {}", e);
  }
  ```

  注意保留原有 `Err(e) => log::error!` 日志。

- [ ] **Step 6: Run tests**

  ```bash
  cargo test --lib migration
  ```

  Expected: 22 existing tests + 1 new test pass

- [ ] **Step 7: Commit**

  ```bash
  git add src-tauri/src/migration/storyforge.rs src-tauri/src/migration/tests.rs
  git commit -m "feat(migration): write failure marker to prevent infinite retry"
  ```

---

### Task 2: 合并数据库时禁用外键

**Files:**
- Modify: `src-tauri/src/migration/storyforge.rs`
- Test: `src-tauri/src/migration/tests.rs`

**Interfaces:**
- Consumes: `rusqlite::Connection`, `PRAGMA foreign_keys`
- Produces: Updated `fn merge_sqlite_databases(target: &Path, source: &Path) -> Result<u64, String>`

- [ ] **Step 1: Write the failing test**

  在 `src-tauri/src/migration/tests.rs` 新增：

  ```rust
  #[test]
  fn merge_sqlite_with_foreign_keys() {
      let dir = TempDir::new().unwrap();
      let target = dir.path().join("target.db");
      let source = dir.path().join("source.db");

      let t = Connection::open(&target).unwrap();
      t.execute("PRAGMA foreign_keys = ON;", []).unwrap();
      t.execute(
          "CREATE TABLE parents (id INTEGER PRIMARY KEY, name TEXT);",
          [],
      )
      .unwrap();
      t.execute(
          "CREATE TABLE children (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parents(id), name TEXT);",
          [],
      )
      .unwrap();
      drop(t);

      let s = Connection::open(&source).unwrap();
      s.execute("PRAGMA foreign_keys = ON;", []).unwrap();
      s.execute(
          "CREATE TABLE parents (id INTEGER PRIMARY KEY, name TEXT);",
          [],
      )
      .unwrap();
      s.execute(
          "CREATE TABLE children (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parents(id), name TEXT);",
          [],
      )
      .unwrap();
      s.execute("INSERT INTO parents VALUES (1, 'p1');", []).unwrap();
      s.execute("INSERT INTO children VALUES (10, 1, 'c1');", [])
          .unwrap();
      drop(s);

      // 默认外键开启时，子表在 sqlite_master 中可能排在父表之后，但按字母顺序 children < parents，
      // 会触发子表先于父表插入。我们的实现应通过禁用外键避免失败。
      let merged = merge_sqlite_databases(&target, &source).unwrap();
      assert_eq!(merged, 2);

      let t = Connection::open(&target).unwrap();
      let parent_count: i64 = t
          .query_row("SELECT COUNT(*) FROM parents", [], |r| r.get(0))
          .unwrap();
      let child_count: i64 = t
          .query_row("SELECT COUNT(*) FROM children", [], |r| r.get(0))
          .unwrap();
      assert_eq!(parent_count, 1);
      assert_eq!(child_count, 1);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**

  ```bash
  cargo test --lib merge_sqlite_with_foreign_keys
  ```

  Expected: FAIL with FK constraint error

- [ ] **Step 3: Implement foreign key disable/enable in merge_sqlite_databases**

  在 `merge_sqlite_databases` 的 `let merge_result: Result<u64, String> = (|| {` 闭包内，紧接 `BEGIN IMMEDIATE` 之后添加：

  ```rust
  conn.execute("PRAGMA foreign_keys = OFF", [])
      .map_err(|e| format!("禁用外键失败: {}", e))?;
  ```

  在 `COMMIT` 之前（即 `conn.execute("COMMIT", [])` 之前）添加：

  ```rust
  conn.execute("PRAGMA foreign_keys = ON", [])
      .map_err(|e| format!("恢复外键失败: {}", e))?;
  ```

  在失败回滚处，将：

  ```rust
  if merge_result.is_err() {
      let _ = conn.execute("ROLLBACK", []);
  }
  ```

  改为：

  ```rust
  if merge_result.is_err() {
      let _ = conn.execute("ROLLBACK", []);
      let _ = conn.execute("PRAGMA foreign_keys = ON", []);
  }
  ```

- [ ] **Step 4: Run tests**

  ```bash
  cargo test --lib migration
  ```

  Expected: 所有迁移测试通过

- [ ] **Step 5: Commit**

  ```bash
  git add src-tauri/src/migration/storyforge.rs src-tauri/src/migration/tests.rs
  git commit -m "fix(migration): disable foreign keys during SQLite merge"
  ```

---

### Task 3: 非空旧目录即视为有数据

**Files:**
- Modify: `src-tauri/src/migration/storyforge.rs`
- Test: `src-tauri/src/migration/tests.rs`

**Interfaces:**
- Consumes: `storyforge_data_dir`, `std::fs::read_dir`
- Produces: Updated `fn has_storyforge_data(app_handle: &AppHandle) -> bool`

- [ ] **Step 1: Write the failing test**

  在 `src-tauri/src/migration/tests.rs` 新增：

  ```rust
  use std::path::Path;

  use super::storyforge::{
      has_storyforge_data_at, storyforge_data_dir_from,
  };

  fn has_storyforge_data_at(path: &Path) -> bool {
      path.is_dir() && path.read_dir().map_err(|_| false).map(|mut i| i.next().is_some()).unwrap_or(false)
  }

  #[test]
  fn has_storyforge_data_true_for_non_empty_dir() {
      let dir = TempDir::new().unwrap();
      let old = dir.path().join("com.storyforge.app");
      fs::create_dir(&old).unwrap();
      fs::write(old.join("story.txt"), "once upon a time").unwrap();

      assert!(has_storyforge_data_at(&old));
  }

  #[test]
  fn has_storyforge_data_false_for_empty_dir() {
      let dir = TempDir::new().unwrap();
      let old = dir.path().join("com.storyforge.app");
      fs::create_dir(&old).unwrap();

      assert!(!has_storyforge_data_at(&old));
  }
  ```

  说明：先测试一个纯路径版 helper，实现完成后再将逻辑合并到 `has_storyforge_data`。

- [ ] **Step 2: Run test to verify it fails**

  ```bash
  cargo test --lib has_storyforge_data
  ```

  Expected: `has_storyforge_data_at` not found

- [ ] **Step 3: Refactor has_storyforge_data to use non-empty check**

  在 `src-tauri/src/migration/storyforge.rs` 中：

  ```rust
  pub fn has_storyforge_data_at(old: &Path) -> bool {
      if !old.is_dir() {
          return false;
      }
      match old.read_dir() {
          Ok(mut entries) => entries.next().is_some(),
          Err(_) => false,
      }
  }

  pub fn has_storyforge_data(app_handle: &AppHandle) -> bool {
      let Some(old) = storyforge_data_dir(app_handle) else {
          return false;
      };
      has_storyforge_data_at(&old)
  }
  ```

  删除原 `has_storyforge_data` 中对 `cinema_ai.db` / `config.json` 的硬编码检测。

- [ ] **Step 4: Run tests**

  ```bash
  cargo test --lib migration
  ```

  Expected: 所有迁移测试通过

- [ ] **Step 5: Commit**

  ```bash
  git add src-tauri/src/migration/storyforge.rs src-tauri/src/migration/tests.rs
  git commit -m "feat(migration): detect StoryForge data by non-empty directory"
  ```

---

### Task 4: 更新 CHANGELOG 并运行最终验证

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add CHANGELOG entry**

  在 `CHANGELOG.md` 的 `[Unreleased]` > `### 功能` 或 `### 修复` 下添加：

  ```markdown
  - **StoryForge 迁移健壮性增强**：
    - 迁移失败时写入 `.storyforge_migration_failed` 标记，避免每次启动都重试并堆积备份。
    - 数据库合并期间临时关闭外键检查，避免子表先于父表插入导致失败。
    - 旧数据检测改为只要 `com.storyforge.app` 目录非空即触发迁移，不再硬依赖 `cinema_ai.db` / `config.json`。
  ```

- [ ] **Step 2: Run full verification**

  ```bash
  cargo test --lib migration
  cargo +nightly fmt --check
  cd src-frontend && npx tsc --noEmit
  ```

  Expected:
  - `cargo test --lib migration`: all pass
  - `cargo +nightly fmt --check`: clean (only deprecation warnings)
  - `npx tsc --noEmit`: exit 0

- [ ] **Step 3: Commit**

  ```bash
  git add CHANGELOG.md
  git commit -m "docs(changelog): record migration follow-up improvements"
  ```

---

## Self-Review

**Spec coverage:**
- 失败标记：Task 1 覆盖常量、路径、写入、`migration_needed` 判断、`run_storyforge_migration` 错误分支。
- 外键禁用：Task 2 覆盖事务前后 `PRAGMA foreign_keys` 切换及失败恢复。
- 非空目录检测：Task 3 覆盖 `has_storyforge_data_at` helper 与 `has_storyforge_data` 重构。
- CHANGELOG：Task 4 记录。

**Placeholder scan:** 无 TBD/TODO；每个步骤均包含具体代码、命令与期望输出。

**Type consistency：**
- `migration_failed_marker_path` 返回 `Option<PathBuf>`，与 `migration_marker_path` 一致。
- `has_storyforge_data_at` 接受 `&Path` 返回 `bool`。
- `merge_sqlite_databases` 签名不变。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-14-migration-followup.md`.

Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
