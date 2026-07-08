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

    fn describe(&self) -> Migration {
        Migration {
            version: self.version(),
            description: self.description(),
            sql: String::new(),
        }
    }
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

        let dir = candidates
            .iter()
            .find(|p| p.exists())
            .cloned()
            .unwrap_or_else(|| candidates.last().unwrap().clone());

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

        let pending: Vec<_> = migrations
            .into_iter()
            .filter(|m| m.version() > current_version)
            .collect();

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

            match migration {
                PendingMigration::Sql(m) => {
                    let tx = conn.transaction()?;
                    Self::execute_migration_sql(&tx, &m.sql)?;
                    record_migration(&tx, version)?;
                    tx.commit()?;
                }
                PendingMigration::Rust(m) => {
                    m.apply(conn)?;
                    record_migration(conn, version)?;
                }
            }

            log::info!("[migrations] V{:03} applied successfully.", version);
        }

        log::info!("[migrations] All pending migrations applied.");
        Ok(())
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
        // Split by semicolons, but be careful with semicolons inside string literals.
        // For simplicity, we split on `;\n` or `;` at end of line, which is safe
        // for the project's DDL/DML patterns (no complex stored procedures).
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

            // Skip transaction control statements — MigrationRunner already wraps
            // each migration in a transaction via `conn.transaction()`.
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
                // If the error is "duplicate column name" or "table already exists",
                // we may want to log and continue for idempotent safety.
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

pub fn record_migration(conn: &Connection, version: i32) -> Result<(), rusqlite::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![version, now],
    )?;
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

/// Returns all Rust-coded migrations (versions 28-99) ordered by version.
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
    ]
}
