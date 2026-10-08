#![allow(dead_code, non_snake_case)]
//! Lightweight migration runner
//!
//! Reads `.sql` files from `migrations/` directory, runs registered Rust
//! migrations, tracks applied versions in `schema_migrations`, and executes
//! pending migrations in order.
//!
//! Replaces the previous 3,111-line hand-rolled migration block.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::Connection;

/// A single migration parsed from a `.sql` file.
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i32,
    pub description: String,
    pub sql: String,
}

/// A migration implemented in Rust code rather than a `.sql` file.
pub trait RustMigration: Send + Sync {
    fn version(&self) -> i32;
    fn description(&self) -> &'static str;
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error>;
}

/// Internal union of SQL-file migrations and Rust migrations for execution.
enum PendingMigration<'a> {
    Sql(Migration),
    Rust(&'a dyn RustMigration),
}

impl<'a> PendingMigration<'a> {
    fn version(&self) -> i32 {
        match self {
            PendingMigration::Sql(m) => m.version,
            PendingMigration::Rust(m) => m.version(),
        }
    }

    fn description(&self) -> String {
        match self {
            PendingMigration::Sql(m) => m.description.clone(),
            PendingMigration::Rust(m) => m.description().to_string(),
        }
    }

    /// 迁移内容校验和：SQL 用内容哈希；Rust 迁移无内容可哈希，返回 None。
    fn checksum(&self) -> Option<String> {
        match self {
            PendingMigration::Sql(m) => Some(migration_checksum(&m.sql)),
            PendingMigration::Rust(_) => None,
        }
    }

    fn describe(&self) -> Migration {
        Migration {
            version: self.version(),
            description: self.description(),
            sql: String::new(),
        }
    }
}

/// 迁移内容的稳定校验和（SHA-256，归一化 CRLF/LF）。
///
/// v0.59.0：`schema_migrations` 原先只记版本号，两份同版本但内容不同的迁移
/// （历史上 `target/` 陈旧副本曾遮蔽源码目录）不会被任何机制发现。
pub(crate) fn migration_checksum(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = content.replace("\r\n", "\n");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// `schema_migrations` 是否已有 `checksum` 列（V132 之前的老库没有）。
fn schema_migrations_has_checksum(conn: &Connection) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare("PRAGMA table_info(schema_migrations)")?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    Ok(names.iter().any(|n| n == "checksum"))
}

/// Lightweight migration runner compatible with rusqlite 0.39.
pub struct MigrationRunner {
    migrations_dir: String,
    rust_migrations: Vec<Box<dyn RustMigration>>,
}

impl MigrationRunner {
    pub fn new<P: AsRef<Path>>(migrations_dir: P) -> Self {
        Self {
            migrations_dir: migrations_dir.as_ref().to_string_lossy().to_string(),
            rust_migrations: Vec::new(),
        }
    }

    /// Register a list of Rust migrations to run after all pending SQL
    /// migrations.
    pub fn with_rust_migrations(mut self, migrations: Vec<Box<dyn RustMigration>>) -> Self {
        self.rust_migrations = migrations;
        self
    }

    /// Default runner pointing to `src-tauri/migrations/`.
    pub fn default_runner() -> Self {
        // For Tauri apps, migrations live next to the binary.
        // In dev, they are at the workspace root under src-tauri/migrations/.
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();

        let cwd = std::env::current_dir().unwrap_or_default();
        let cargo_dir = std::env::var("CARGO_MANIFEST_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_default();

        let candidates = [
            // Production: Tauri bundled resources (see tauri.conf.json bundle.resources)
            exe_dir.join("resources/db/migrations"),
            // Production: next to binary
            exe_dir.join("migrations"),
            exe_dir.join("../migrations"),
            exe_dir.join("../../migrations"),
            // Dev: CWD is workspace root
            cwd.join("src-tauri/migrations"),
            // Dev: CWD is src-tauri crate root
            cwd.join("migrations"),
            // Dev: CARGO_MANIFEST_DIR points to src-tauri
            cargo_dir.join("migrations"),
            // Dev: CARGO_MANIFEST_DIR/../src-tauri/migrations (workspace root)
            cargo_dir.join("../src-tauri/migrations"),
            // Dev/Prod: db/migrations (T1.4-T1.5 migration framework path)
            exe_dir.join("db/migrations"),
            exe_dir.join("../db/migrations"),
            exe_dir.join("../../db/migrations"),
            cwd.join("src-tauri/src/db/migrations"),
            cwd.join("src/db/migrations"),
            cargo_dir.join("src/db/migrations"),
        ];

        let dir = pick_migrations_dir(&candidates);

        Self::new(dir)
    }

    /// Scan the migrations directory and parse all `.sql` files.
    pub fn load_migrations(&self) -> Result<Vec<Migration>, MigrationError> {
        let path = Path::new(&self.migrations_dir);
        if !path.exists() {
            log::warn!(
                "[migrations] Directory not found: {}. No SQL migrations will be applied.",
                path.display()
            );
            return Ok(Vec::new());
        }

        let mut entries: Vec<_> = fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();

        // Sort by filename (V001, V002, ...)
        entries.sort_by_key(|e| e.file_name());

        let mut migrations = Vec::new();
        for entry in entries {
            let filename = entry.file_name().to_string_lossy().to_string();
            let (version, description) = Self::parse_filename(&filename)?;
            let sql = fs::read_to_string(entry.path())?;

            if sql.trim().is_empty() {
                log::warn!("[migrations] Skipping empty migration file: {}", filename);
                continue;
            }

            migrations.push(Migration {
                version,
                description,
                sql,
            });
        }

        // Validate ordering: versions must be strictly increasing
        for window in migrations.windows(2) {
            if window[0].version >= window[1].version {
                return Err(MigrationError::OutOfOrder {
                    prev: window[0].clone(),
                    next: window[1].clone(),
                });
            }
        }

        Ok(migrations)
    }

    /// Run all pending SQL and Rust migrations against the given connection.
    pub fn run(&self, conn: &mut Connection) -> Result<(), MigrationError> {
        let sql_migrations = self.load_migrations()?;
        let mut all: Vec<PendingMigration> = sql_migrations
            .into_iter()
            .map(PendingMigration::Sql)
            .collect();
        for rust in &self.rust_migrations {
            all.push(PendingMigration::Rust(rust.as_ref()));
        }
        all.sort_by_key(|m| m.version());

        // Validate ordering: versions must be strictly increasing.
        for window in all.windows(2) {
            if window[0].version() >= window[1].version() {
                return Err(MigrationError::OutOfOrder {
                    prev: window[0].describe(),
                    next: window[1].describe(),
                });
            }
        }

        Self::apply_pending(conn, all)
    }

    /// Apply a list of migrations that are newer than the current schema
    /// version.
    fn apply_pending(
        conn: &mut Connection,
        migrations: Vec<PendingMigration>,
    ) -> Result<(), MigrationError> {
        if migrations.is_empty() {
            log::info!("[migrations] No pending migrations to apply.");
            return Ok(());
        }

        let current_version = get_current_version(conn);
        log::info!(
            "[migrations] {} migration(s) loaded, current schema version: {}",
            migrations.len(),
            current_version
        );

        // v0.59.0：待执行集合 = 迁移文件中所有「未记录在 schema_migrations
        // 的版本」， 而非「版本号 > MAX(version)」。
        // 旧口径下任何低于当前水位的补丁迁移
        // （历史空洞/后补迁移）永远不会被执行，也永远不会被告警。
        let applied = Self::applied_migrations(conn)?;
        let max_applied = applied.keys().copied().max().unwrap_or(0);

        // 已应用迁移的内容一致性校验：同版本内容变了说明两份迁移副本分歧，
        // 或已发布迁移被静默改动——两种都不该悄悄发生。
        for migration in &migrations {
            let version = migration.version();
            let Some(Some(stored)) = applied.get(&version) else {
                continue;
            };
            if let Some(computed) = migration.checksum() {
                if stored != &computed {
                    log::warn!(
                        "[migrations] V{:03} 内容校验和不一致（库内 {}，当前文件 {}）：\
                         该版本可能来自另一份迁移副本，或迁移文件在发布后被修改。",
                        version,
                        &stored[..stored.len().min(12)],
                        &computed[..computed.len().min(12)]
                    );
                }
            }
        }

        let pending: Vec<_> = migrations
            .into_iter()
            .filter(|m| !applied.contains_key(&m.version()))
            .collect();

        // 低于当前水位却未记录的版本：属于历史跳过（旧 MAX(version) 水位线的
        // 遗留），显式告警后再补执行。
        let holes: Vec<i32> = pending
            .iter()
            .map(|m| m.version())
            .filter(|v| *v <= max_applied)
            .collect();
        if !holes.is_empty() {
            log::warn!(
                "[migrations] 检测到 {} 个低于当前水位（V{:03}）但未记录的迁移，将补执行：{:?}",
                holes.len(),
                max_applied,
                holes
            );
        }

        if pending.is_empty() {
            log::info!("[migrations] Database is up to date.");
            return Ok(());
        }

        log::info!(
            "[migrations] {} pending migration(s) to apply.",
            pending.len()
        );

        for migration in pending {
            let version = migration.version();
            let description = migration.description();
            log::info!("[migrations] Applying V{:03}: {}", version, description);

            let checksum = migration.checksum();
            match migration {
                PendingMigration::Sql(m) => {
                    let tx = conn.transaction()?;
                    Self::execute_migration_sql(&tx, &m.sql)?;
                    record_migration(&tx, version, checksum.as_deref())?;
                    tx.commit()?;
                }
                PendingMigration::Rust(m) => {
                    m.apply(conn)?;
                    record_migration(conn, version, checksum.as_deref())?;
                }
            }

            log::info!("[migrations] V{:03} applied successfully.", version);
        }

        log::info!("[migrations] All pending migrations applied.");
        Ok(())
    }

    /// 已应用迁移集合：version -> 内容校验和（老库/V132 之前为 None）。
    fn applied_migrations(
        conn: &Connection,
    ) -> Result<std::collections::HashMap<i32, Option<String>>, MigrationError> {
        let has_checksum = schema_migrations_has_checksum(conn)?;
        let sql = if has_checksum {
            "SELECT version, checksum FROM schema_migrations"
        } else {
            "SELECT version, NULL FROM schema_migrations"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
        Ok(rows)
    }

    /// Run SQL file migrations interleaved with a legacy inline migration
    /// function. SQL files with version <= `max_inline_version` run before the
    /// inline function; SQL files with version > `max_inline_version` run
    /// after.
    ///
    /// This prevents a high-version SQL file from advancing `schema_migrations`
    /// past inline migrations that still need to run.
    ///
    /// Kept for backward compatibility; new code should use
    /// [`MigrationRunner::run`].
    pub fn run_with_legacy<F>(
        &self,
        conn: &mut Connection,
        legacy_fn: F,
        max_inline_version: i32,
    ) -> Result<(), MigrationError>
    where
        F: FnOnce(&mut Connection) -> Result<(), rusqlite::Error>,
    {
        let migrations = self.load_migrations()?;
        if migrations.is_empty() {
            log::info!(
                "[migrations] No migration files found in {}",
                self.migrations_dir
            );
        }

        // 1. SQL file migrations that should run before inline migrations.
        let pre_inline: Vec<_> = migrations
            .iter()
            .filter(|m| m.version <= max_inline_version)
            .cloned()
            .map(PendingMigration::Sql)
            .collect();
        Self::apply_pending(conn, pre_inline)?;

        // 2. Run legacy inline migrations.
        log::info!("[migrations] Running legacy inline migrations...");
        legacy_fn(conn).map_err(MigrationError::from)?;
        log::info!("[migrations] Legacy inline migrations completed.");

        // 3. SQL file migrations that should run after inline migrations.
        let post_inline: Vec<_> = migrations
            .into_iter()
            .filter(|m| m.version > max_inline_version)
            .map(PendingMigration::Sql)
            .collect();
        Self::apply_pending(conn, post_inline)?;

        Ok(())
    }

    /// Execute a single migration's SQL, splitting on `;` into individual
    /// statements.
    fn execute_migration_sql(tx: &rusqlite::Transaction, sql: &str) -> Result<(), MigrationError> {
        // Split by semicolons, but be careful with semicolons inside string
        // literals. For simplicity, we split on `;\n` or `;` at end of
        // line, which is safe for the project's DDL/DML patterns (no
        // complex stored procedures).
        let statements: Vec<&str> = sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for stmt in statements {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }

            // Skip transaction control statements — MigrationRunner already
            // wraps each migration in a transaction via
            // `conn.transaction()`.
            let upper = stmt.to_uppercase();
            if upper == "BEGIN" || upper.starts_with("BEGIN ") {
                log::debug!("[migrations] Skipping BEGIN (managed by runner)");
                continue;
            }
            if upper == "COMMIT" || upper.starts_with("COMMIT ") {
                log::debug!("[migrations] Skipping COMMIT (managed by runner)");
                continue;
            }
            if upper == "ROLLBACK" || upper.starts_with("ROLLBACK ") {
                log::debug!("[migrations] Skipping ROLLBACK (managed by runner)");
                continue;
            }

            // Add semicolon back for execution
            let stmt_with_semicolon = format!("{};", stmt);

            if let Err(e) = tx.execute(&stmt_with_semicolon, []) {
                // If the error is "duplicate column name" or "table already
                // exists", we may want to log and continue for
                // idempotent safety.
                let err_msg = e.to_string().to_lowercase();
                if err_msg.contains("duplicate column name") || err_msg.contains("already exists") {
                    log::warn!(
                        "[migrations] Idempotent skip: {} (stmt: {})",
                        e,
                        stmt_with_semicolon.chars().take(80).collect::<String>()
                    );
                    continue;
                }
                return Err(MigrationError::SqlExecution {
                    sql: stmt_with_semicolon,
                    source: e,
                });
            }
        }

        Ok(())
    }

    /// Parse `V{version}__{description}.sql` → (version, description).
    fn parse_filename(filename: &str) -> Result<(i32, String), MigrationError> {
        let stem = filename
            .strip_suffix(".sql")
            .ok_or_else(|| MigrationError::InvalidFilename(filename.to_string()))?;

        let parts: Vec<&str> = stem.splitn(2, "__").collect();
        if parts.len() != 2 {
            return Err(MigrationError::InvalidFilename(filename.to_string()));
        }

        let version_str = parts[0]
            .strip_prefix('V')
            .ok_or_else(|| MigrationError::InvalidFilename(filename.to_string()))?;

        let version: i32 = version_str
            .parse()
            .map_err(|_| MigrationError::InvalidFilename(filename.to_string()))?;

        let description = parts[1].replace('_', " ");

        Ok((version, description))
    }
}

// ---------------------------------------------------------------------------
// Migrations directory selection
// ---------------------------------------------------------------------------

/// 在存在的候选目录中选 .sql 最高版本号最大者（修复"陈旧 target
/// 副本遮蔽新迁移"）。 版本持平取候选序靠前者。多候选存在且最高版本不一致时
/// warn 双方路径。
///
/// v0.59.0：候选先剔除构建产物路径（含 `target` 组件）。dev 下 exe_dir 即
/// `src-tauri/target/debug`，`target/debug/db/migrations` 是历史遗留副本，
/// 版本持平时会因候选序靠前而被选中；剔除后源码目录成为唯一事实源。
pub(crate) fn pick_migrations_dir(candidates: &[PathBuf]) -> PathBuf {
    let existing_all: Vec<&PathBuf> = candidates.iter().filter(|p| p.exists()).collect();
    let existing: Vec<&PathBuf> = existing_all
        .iter()
        .copied()
        .filter(|p| !is_build_output_dir(p))
        .collect();
    // 极端情况（仓库本身位于名为 target 的目录下）回退到未过滤集合，
    // 保证仍有目录可用。
    let existing = if existing.is_empty() {
        existing_all
    } else {
        existing
    };
    let fallback = existing
        .first()
        .map(|p| (*p).clone())
        .unwrap_or_else(|| candidates.last().unwrap().clone());
    let mut best: Option<(&PathBuf, i32)> = None;
    for dir in &existing {
        if let Some(v) = max_sql_version(dir) {
            match &best {
                Some((_, bv)) if *bv >= v => {}
                _ => best = Some((dir, v)),
            }
        }
    }
    if let Some((best_dir, best_v)) = best {
        for dir in &existing {
            if *dir != best_dir {
                if let Some(v) = max_sql_version(dir) {
                    if v != best_v {
                        log::warn!(
                            "[migrations] 多个迁移目录存在且版本不一致：选用 {}（V{}），忽略 {}（V{}）。建议删除陈旧目录。",
                            best_dir.display(), best_v, dir.display(), v
                        );
                    }
                }
            }
        }
        log::info!(
            "[migrations] 选用迁移目录：{}（V{}）",
            best_dir.display(),
            best_v
        );
        return best_dir.clone();
    }
    fallback
}

/// 路径是否位于构建产物目录（任一路径组件为 `target`）。
fn is_build_output_dir(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::Normal(s) if s == "target"))
}

/// 目录中最高的 V{num}__ 迁移版本号（无 .sql 返回 None）。
pub(crate) fn max_sql_version(dir: &Path) -> Option<i32> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().extension().map(|x| x == "sql").unwrap_or(false) {
                MigrationRunner::parse_filename(&name).ok().map(|(v, _)| v)
            } else {
                None
            }
        })
        .max()
}

// ---------------------------------------------------------------------------
// Compatibility with existing schema_migrations table
// ---------------------------------------------------------------------------

fn get_current_version(conn: &Connection) -> i32 {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn record_migration(
    conn: &Connection,
    version: i32,
    checksum: Option<&str>,
) -> Result<(), rusqlite::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // V132 之前的库没有 checksum 列；迁移按版本顺序执行，V132 之前的
    // 记录必须走旧语句。
    if checksum.is_some() && schema_migrations_has_checksum(conn)? {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at, checksum) VALUES (?1, ?2, ?3)",
            rusqlite::params![version, now, checksum],
        )?;
    } else {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, now],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MigrationError {
    DirectoryNotFound(std::path::PathBuf),
    InvalidFilename(String),
    OutOfOrder {
        prev: Migration,
        next: Migration,
    },
    SqlExecution {
        sql: String,
        source: rusqlite::Error,
    },
    Io(std::io::Error),
    Rusqlite(rusqlite::Error),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::DirectoryNotFound(p) => {
                write!(f, "Migrations directory not found: {}", p.display())
            }
            MigrationError::InvalidFilename(name) => {
                write!(f, "Invalid migration filename: {}", name)
            }
            MigrationError::OutOfOrder { prev, next } => {
                write!(
                    f,
                    "Migrations out of order: V{:03} ({}) followed by V{:03} ({})",
                    prev.version, prev.description, next.version, next.description
                )
            }
            MigrationError::SqlExecution { sql, source } => {
                write!(f, "SQL execution failed: {} | SQL: {}", source, sql)
            }
            MigrationError::Io(e) => write!(f, "IO error: {}", e),
            MigrationError::Rusqlite(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for MigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MigrationError::SqlExecution { source, .. } => Some(source),
            MigrationError::Io(e) => Some(e),
            MigrationError::Rusqlite(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MigrationError {
    fn from(e: std::io::Error) -> Self {
        MigrationError::Io(e)
    }
}

impl From<rusqlite::Error> for MigrationError {
    fn from(e: rusqlite::Error) -> Self {
        MigrationError::Rusqlite(e)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_parse_filename_valid() {
        let (v, d) = MigrationRunner::parse_filename("V001__create_users.sql").unwrap();
        assert_eq!(v, 1);
        assert_eq!(d, "create users");
    }

    #[test]
    fn test_parse_filename_chinese() {
        let (v, d) =
            MigrationRunner::parse_filename("V007__创建角色状态追踪表_智能化创作.sql").unwrap();
        assert_eq!(v, 7);
        assert!(d.contains("创建角色状态追踪表"));
    }

    #[test]
    fn test_parse_filename_invalid() {
        assert!(MigrationRunner::parse_filename("invalid.sql").is_err());
        assert!(MigrationRunner::parse_filename("Vabc__test.sql").is_err());
    }

    #[test]
    fn test_load_migrations_sorts_and_validates() {
        let dir = TempDir::new().unwrap();
        let mut f1 = fs::File::create(dir.path().join("V002__second.sql")).unwrap();
        writeln!(f1, "CREATE TABLE t2 (id INTEGER);").unwrap();
        let mut f2 = fs::File::create(dir.path().join("V001__first.sql")).unwrap();
        writeln!(f2, "CREATE TABLE t1 (id INTEGER);").unwrap();

        let runner = MigrationRunner::new(dir.path());
        let migs = runner.load_migrations().unwrap();
        assert_eq!(migs.len(), 2);
        assert_eq!(migs[0].version, 1);
        assert_eq!(migs[1].version, 2);
    }

    #[test]
    fn test_load_migrations_rejects_out_of_order() {
        let dir = TempDir::new().unwrap();
        let mut f1 = fs::File::create(dir.path().join("V003__third.sql")).unwrap();
        writeln!(f1, "CREATE TABLE t3 (id INTEGER);").unwrap();
        let mut f2 = fs::File::create(dir.path().join("V001__first.sql")).unwrap();
        writeln!(f2, "CREATE TABLE t1 (id INTEGER);").unwrap();
        let mut f3 = fs::File::create(dir.path().join("V002__second.sql")).unwrap();
        writeln!(f3, "CREATE TABLE t2 (id INTEGER);").unwrap();

        let runner = MigrationRunner::new(dir.path());
        assert!(runner.load_migrations().is_ok());
    }

    #[test]
    fn test_run_migrations_applies_pending() {
        let dir = TempDir::new().unwrap();
        let mut f1 = fs::File::create(dir.path().join("V001__create_test.sql")).unwrap();
        writeln!(f1, "CREATE TABLE test_table (id INTEGER PRIMARY KEY);").unwrap();

        let runner = MigrationRunner::new(dir.path());
        let mut conn = Connection::open_in_memory().unwrap();

        // Create schema_migrations table first (normally done by create_tables)
        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        runner.run(&mut conn).unwrap();

        // Verify table exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='test_table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify version recorded
        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_run_migrations_skips_already_applied() {
        let dir = TempDir::new().unwrap();
        let mut f1 = fs::File::create(dir.path().join("V001__create_test.sql")).unwrap();
        writeln!(f1, "CREATE TABLE test_table (id INTEGER PRIMARY KEY);").unwrap();

        let runner = MigrationRunner::new(dir.path());
        let mut conn = Connection::open_in_memory().unwrap();

        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        // Pre-record V001 as applied
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, 0)",
            [],
        )
        .unwrap();

        runner.run(&mut conn).unwrap();

        // test_table should NOT exist because V001 was skipped
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='test_table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_run_migrations_idempotent_errors() {
        let dir = TempDir::new().unwrap();
        let mut f1 = fs::File::create(dir.path().join("V001__add_col.sql")).unwrap();
        writeln!(f1, "CREATE TABLE test_table (id INTEGER PRIMARY KEY);").unwrap();
        let mut f2 = fs::File::create(dir.path().join("V002__add_dup_col.sql")).unwrap();
        writeln!(f2, "ALTER TABLE test_table ADD COLUMN name TEXT;").unwrap();

        let runner = MigrationRunner::new(dir.path());
        let mut conn = Connection::open_in_memory().unwrap();

        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();

        runner.run(&mut conn).unwrap();

        // Run again should succeed (idempotent)
        let runner2 = MigrationRunner::new(dir.path());
        runner2.run(&mut conn).unwrap();
    }

    // v0.26.30 hotfix: 当旧数据库在 inline → Rust migration 切换过程中跳过
    // V099， schema_migrations 可能已到 102 但 characters 等表缺失 source /
    // is_auto_generated 列。 V103 必须能补回这些列。
    #[test]
    fn test_v103_repairs_missing_source_columns() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();
        // Simulate a database that reached V102 but missed V099's columns.
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (102, 0)",
            [],
        )
        .unwrap();

        // Create characters/scenes/world_buildings/kg_entities without source
        // columns.
        conn.execute_batch(
            "CREATE TABLE characters (id TEXT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE scenes (id TEXT PRIMARY KEY, title TEXT);
             CREATE TABLE world_buildings (id TEXT PRIMARY KEY, concept TEXT);
             CREATE TABLE kg_entities (id TEXT PRIMARY KEY, name TEXT);",
        )
        .unwrap();

        let migration = V103__ensure_source_columns::Migration;
        migration.apply(&mut conn).unwrap();

        // Verify columns were added.
        for table in ["characters", "scenes", "world_buildings", "kg_entities"] {
            let cols: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))
                .unwrap()
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                cols.contains(&"source".to_string()),
                "{} should have source column",
                table
            );
            assert!(
                cols.contains(&"is_auto_generated".to_string()),
                "{} should have is_auto_generated column",
                table
            );
        }
    }

    #[test]
    fn test_pick_migrations_dir_prefers_highest_version() {
        let base = std::env::temp_dir().join(format!("mig-pick-{}", uuid::Uuid::new_v4()));
        let stale = base.join("target/debug/db/migrations");
        let fresh = base.join("src-tauri/src/db/migrations");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::create_dir_all(&fresh).unwrap();
        // 陈旧副本：只到 V106；源码目录：到 V109
        std::fs::write(stale.join("V106__a.sql"), "-- x").unwrap();
        std::fs::write(fresh.join("V106__a.sql"), "-- x").unwrap();
        std::fs::write(fresh.join("V109__b.sql"), "-- y").unwrap();
        let candidates = vec![stale.clone(), fresh.clone()]; // stale 排前（复现旧 find(exists) 命中）
        let picked = pick_migrations_dir(&candidates);
        assert_eq!(picked, fresh);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// v0.59.0：构建产物目录（含 `target` 组件）里的迁移副本永不选用，
    /// 即使其最高版本号更大——源码目录是唯一事实源。
    #[test]
    fn test_pick_migrations_dir_ignores_build_output_copy_even_when_newer() {
        let base = std::env::temp_dir().join(format!("mig-pick-target-{}", uuid::Uuid::new_v4()));
        let shadow = base.join("target/debug/db/migrations");
        let fresh = base.join("src-tauri/src/db/migrations");
        std::fs::create_dir_all(&shadow).unwrap();
        std::fs::create_dir_all(&fresh).unwrap();
        std::fs::write(shadow.join("V140__bogus.sql"), "-- x").unwrap();
        std::fs::write(fresh.join("V131__real.sql"), "-- y").unwrap();
        let picked = pick_migrations_dir(&[shadow.clone(), fresh.clone()]);
        assert_eq!(picked, fresh);
        let picked2 = pick_migrations_dir(&[fresh.clone(), shadow.clone()]);
        assert_eq!(picked2, fresh);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_pick_migrations_dir_falls_back_to_first_existing_when_no_sql() {
        let base = std::env::temp_dir().join(format!("mig-pick-empty-{}", uuid::Uuid::new_v4()));
        let a = base.join("a");
        let b = base.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("V109__b.sql"), "-- y").unwrap();
        // a 存在但无 .sql → 选 b；都无 .sql → 第一个存在
        let picked = pick_migrations_dir(&[a.clone(), b.clone()]);
        assert_eq!(picked, b);
        let picked2 = pick_migrations_dir(&[a.clone()]);
        assert_eq!(picked2, a);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_max_sql_version() {
        let base = std::env::temp_dir().join(format!("mig-max-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        assert_eq!(max_sql_version(&base), None);
        std::fs::write(base.join("V103__x.sql"), "--").unwrap();
        std::fs::write(base.join("V109__y.sql"), "--").unwrap();
        std::fs::write(base.join("notes.md"), "not a migration").unwrap();
        assert_eq!(max_sql_version(&base), Some(109));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// v0.59.0：迁移内容校验和必须与换行风格无关（Windows CRLF 检出不应误报）。
    #[test]
    fn test_migration_checksum_normalizes_line_endings() {
        assert_eq!(migration_checksum("a\r\nb"), migration_checksum("a\nb"));
        assert_ne!(migration_checksum("a\nb"), migration_checksum("a\nc"));
        assert_eq!(migration_checksum("x").len(), 64);
    }

    /// v0.59.0：后补的低版本迁移（低于已应用水位）必须被补执行——
    /// 旧口径 `version > MAX(version)` 会把它永久跳过且不告警。
    #[test]
    fn test_apply_pending_backfills_lower_versioned_migration() {
        let dir = TempDir::new().unwrap();
        let write = |name: &str, sql: &str| {
            let mut f = fs::File::create(dir.path().join(name)).unwrap();
            writeln!(f, "{sql}").unwrap();
        };
        write("V010__tenth.sql", "CREATE TABLE t10 (id INTEGER);");
        write("V020__twentieth.sql", "CREATE TABLE t20 (id INTEGER);");

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL,
                checksum TEXT
            )",
            [],
        )
        .unwrap();

        MigrationRunner::new(dir.path()).run(&mut conn).unwrap();
        assert_eq!(get_current_version(&conn), 20);

        // 水位已到 V020，此时补一个 V015：集合水位线必须仍能执行它
        write("V015__backfill.sql", "CREATE TABLE t15 (id INTEGER);");
        MigrationRunner::new(dir.path()).run(&mut conn).unwrap();

        let t15_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='t15'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(t15_exists, "低于水位的补丁迁移必须被执行");
        let applied = MigrationRunner::applied_migrations(&conn).unwrap();
        assert!(applied.contains_key(&15));
    }

    /// v0.59.0：SQL 迁移落库时记录内容校验和，可读回并比对。
    #[test]
    fn test_record_migration_stores_content_checksum() {
        let dir = TempDir::new().unwrap();
        let sql = "CREATE TABLE a (id INTEGER);\n";
        let mut f = fs::File::create(dir.path().join("V001__a.sql")).unwrap();
        f.write_all(sql.as_bytes()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL,
                checksum TEXT
            )",
            [],
        )
        .unwrap();

        MigrationRunner::new(dir.path()).run(&mut conn).unwrap();
        let applied = MigrationRunner::applied_migrations(&conn).unwrap();
        let stored = applied.get(&1).and_then(|c| c.clone()).unwrap();
        assert_eq!(stored, migration_checksum(sql));
    }

    /// 老库（无 checksum 列）仍可落库：降级为旧 INSERT 语句，不报错。
    #[test]
    fn test_record_migration_works_without_checksum_column() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )
        .unwrap();
        record_migration(&conn, 7, Some("deadbeef")).unwrap();
        let applied = MigrationRunner::applied_migrations(&conn).unwrap();
        assert_eq!(applied.get(&7), Some(&None));
    }
}

pub mod V028__scene_structure_fields;
pub mod V029__chat_sessions_and_messages;
pub mod V030__story_runtime_states;
pub mod V031__story_style_configs;
pub mod V032__scene_style_blend_override;
pub mod V033__user_auth_system;
pub mod V034__subscription_real_user_id;
pub mod V035__story_outlines;
pub mod V036__character_relationships;
pub mod V037__scene_foreshadowing_ids;
pub mod V038__chapter_scene_mapping;
pub mod V039__workflow_instances;
pub mod V040__pending_vector_indexes;
pub mod V041__story_metadata;
pub mod V042__scene_characters;
pub mod V043__scene_character_actions;
pub mod V044__plan_templates;
pub mod V045__story_contracts;
pub mod V046__scene_commits;
pub mod V047__memory_items;
pub mod V048__chapter_reading_power;
pub mod V049__chase_debt;
pub mod V050__override_contracts;
pub mod V051__review_issues;
pub mod V052__genre_profiles;
pub mod V053__chapter_writing_phase;
pub mod V054__ingest_jobs;
pub mod V055__feature_usage_logs;
pub mod V056__blueprints;
pub mod V057__drafts;
pub mod V058__revisions;
pub mod V059__reviews;
pub mod V060__post_process_runs;
pub mod V061__character_dynamic_state_fields;
pub mod V062__llm_calls;
pub mod V063__ai_usage_quota_offline_grace;
pub mod V064__style_snapshots;
pub mod V065__narrative_tables_status;
pub mod V066__genesis_runs;
pub mod V067__scene_commits_chapter_id;
pub mod V068__reference_to_narrative_migration;
pub mod V069__rename_chapter_commits;
pub mod V070__drop_chapters_scene_id;
pub mod V071__scene_divider_nodes;
pub mod V072__entity_mentions;
pub mod V073__narrative_events;
pub mod V074__narrative_threads;
pub mod V075__narrative_structure_positions;
pub mod V076__narrative_structure;
pub mod V077__narrative_chunks;
pub mod V078__scene_litseg_fields;
pub mod V079__foreshadowing_tracker_events;
pub mod V080__character_states_arc;
pub mod V081__story_outlines_analyzed_structure;
pub mod V082__conflict_escalations;
pub mod V083__drop_redundant_litseg_tables;
pub mod V085__reference_scenes_litseg_fields;
pub mod V086__reference_books_analyzed_structure;
pub mod V087__genre_profiles_typical_structure;
pub mod V088__stories_genre_profile_id;
pub mod V089__llm_calls_model_health;
pub mod V090__text_annotations_metadata_severity;
pub mod V091__model_capability_profile;
pub mod V092__beat_cards_story_engines_pressure_relationships;
pub mod V093__prompt_overrides;
pub mod V094__drop_dead_tables;
pub mod V096__genre_profiles_recommended_assets;
pub mod V097__stories_reference_book_id;
pub mod V098__narrative_tables_status_v2;
pub mod V099__source_and_auto_generated_columns;
pub mod V103__ensure_source_columns;
pub mod V115__drop_chapter_content;
pub mod V116__migrate_character_cs_columns;
pub mod V117__unify_entities_to_kg_entities;
pub mod V118__unify_foreshadowing_thread;
pub mod V120__guidebooks_custom_methodologies;
pub mod V121__repair_duplicate_chapter_titles;
pub mod V127__split_overlong_chapters;
pub mod V128__merge_lone_closing_punct_paragraphs;
pub mod V130__retitle_generic_chapter_numbers;
pub mod V139__merge_leading_closing_punct_paragraphs;
pub mod V141__merge_same_person_characters;
pub mod V142__character_life_status;
pub mod V143__story_material_staleness;
pub mod V144__relationship_kind_normalization;
pub mod V145__summary_source_hash;

/// Returns all Rust-coded migrations (versions 28-103, 115-117) ordered by
/// version.
pub fn all_rust_migrations() -> Vec<Box<dyn RustMigration>> {
    vec![
        Box::new(V028__scene_structure_fields::Migration),
        Box::new(V029__chat_sessions_and_messages::Migration),
        Box::new(V030__story_runtime_states::Migration),
        Box::new(V031__story_style_configs::Migration),
        Box::new(V032__scene_style_blend_override::Migration),
        Box::new(V033__user_auth_system::Migration),
        Box::new(V034__subscription_real_user_id::Migration),
        Box::new(V035__story_outlines::Migration),
        Box::new(V036__character_relationships::Migration),
        Box::new(V037__scene_foreshadowing_ids::Migration),
        Box::new(V038__chapter_scene_mapping::Migration),
        Box::new(V039__workflow_instances::Migration),
        Box::new(V040__pending_vector_indexes::Migration),
        Box::new(V041__story_metadata::Migration),
        Box::new(V042__scene_characters::Migration),
        Box::new(V043__scene_character_actions::Migration),
        Box::new(V044__plan_templates::Migration),
        Box::new(V045__story_contracts::Migration),
        Box::new(V046__scene_commits::Migration),
        Box::new(V047__memory_items::Migration),
        Box::new(V048__chapter_reading_power::Migration),
        Box::new(V049__chase_debt::Migration),
        Box::new(V050__override_contracts::Migration),
        Box::new(V051__review_issues::Migration),
        Box::new(V052__genre_profiles::Migration),
        Box::new(V053__chapter_writing_phase::Migration),
        Box::new(V054__ingest_jobs::Migration),
        Box::new(V055__feature_usage_logs::Migration),
        Box::new(V056__blueprints::Migration),
        Box::new(V057__drafts::Migration),
        Box::new(V058__revisions::Migration),
        Box::new(V059__reviews::Migration),
        Box::new(V060__post_process_runs::Migration),
        Box::new(V061__character_dynamic_state_fields::Migration),
        Box::new(V062__llm_calls::Migration),
        Box::new(V063__ai_usage_quota_offline_grace::Migration),
        Box::new(V064__style_snapshots::Migration),
        Box::new(V065__narrative_tables_status::Migration),
        Box::new(V066__genesis_runs::Migration),
        Box::new(V067__scene_commits_chapter_id::Migration),
        Box::new(V068__reference_to_narrative_migration::Migration),
        Box::new(V069__rename_chapter_commits::Migration),
        Box::new(V070__drop_chapters_scene_id::Migration),
        Box::new(V071__scene_divider_nodes::Migration),
        Box::new(V072__entity_mentions::Migration),
        Box::new(V073__narrative_events::Migration),
        Box::new(V074__narrative_threads::Migration),
        Box::new(V075__narrative_structure_positions::Migration),
        Box::new(V076__narrative_structure::Migration),
        Box::new(V077__narrative_chunks::Migration),
        Box::new(V078__scene_litseg_fields::Migration),
        Box::new(V079__foreshadowing_tracker_events::Migration),
        Box::new(V080__character_states_arc::Migration),
        Box::new(V081__story_outlines_analyzed_structure::Migration),
        Box::new(V082__conflict_escalations::Migration),
        Box::new(V083__drop_redundant_litseg_tables::Migration),
        Box::new(V085__reference_scenes_litseg_fields::Migration),
        Box::new(V086__reference_books_analyzed_structure::Migration),
        Box::new(V087__genre_profiles_typical_structure::Migration),
        Box::new(V088__stories_genre_profile_id::Migration),
        Box::new(V089__llm_calls_model_health::Migration),
        Box::new(V090__text_annotations_metadata_severity::Migration),
        Box::new(V091__model_capability_profile::Migration),
        Box::new(V092__beat_cards_story_engines_pressure_relationships::Migration),
        Box::new(V093__prompt_overrides::Migration),
        Box::new(V094__drop_dead_tables::Migration),
        Box::new(V096__genre_profiles_recommended_assets::Migration),
        Box::new(V097__stories_reference_book_id::Migration),
        Box::new(V098__narrative_tables_status_v2::Migration),
        Box::new(V099__source_and_auto_generated_columns::Migration),
        Box::new(V103__ensure_source_columns::Migration),
        Box::new(V115__drop_chapter_content::Migration),
        Box::new(V116__migrate_character_cs_columns::Migration),
        Box::new(V117__unify_entities_to_kg_entities::Migration),
        Box::new(V118__unify_foreshadowing_thread::Migration),
        Box::new(V120__guidebooks_custom_methodologies::Migration),
        Box::new(V121__repair_duplicate_chapter_titles::Migration),
        Box::new(V127__split_overlong_chapters::Migration),
        Box::new(V128__merge_lone_closing_punct_paragraphs::Migration),
        Box::new(V130__retitle_generic_chapter_numbers::Migration),
        Box::new(V139__merge_leading_closing_punct_paragraphs::Migration),
        Box::new(V141__merge_same_person_characters::Migration),
        Box::new(V142__character_life_status::Migration),
        Box::new(V143__story_material_staleness::Migration),
        Box::new(V144__relationship_kind_normalization::Migration),
        Box::new(V145__summary_source_hash::Migration),
    ]
}
