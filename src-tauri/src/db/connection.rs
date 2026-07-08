use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;

use crate::db::migrations::{all_rust_migrations, MigrationRunner};

pub type DbPool = Pool<SqliteConnectionManager>;

#[cfg(test)]
pub fn create_test_pool() -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager = SqliteConnectionManager::memory().with_init(|c| {
        c.execute_batch(
            "PRAGMA foreign_keys = ON; \
             PRAGMA busy_timeout = 5000;",
        )
    });
    let pool = Pool::builder()
        .max_size(10)
        // v0.23.17: connection_timeout 防止 pool.get() 在连接池耗尽时无限阻塞，
        // 导致 tokio worker 线程被卡死、tokio::time::timeout 无法触发。
        .connection_timeout(std::time::Duration::from_secs(10))
        .build(manager)?;

    let mut conn = pool.get()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\n            version INTEGER PRIMARY \
         KEY,\n            applied_at INTEGER NOT NULL\n        )",
        [],
    )?;

    create_tables(&mut conn)?;
    MigrationRunner::default_runner()
        .with_rust_migrations(all_rust_migrations())
        .run(&mut conn)?;

    // 测试环境：创建 scene_versions 表（被 change_tracks/comment_threads 外键引用）
    conn.execute(
        "CREATE TABLE IF NOT EXISTS scene_versions (
            id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            chapter_id TEXT,
            content TEXT,
            word_count INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
        )",
        [],
    )?;

    Ok(pool)
}

fn get_current_version(conn: &rusqlite::Connection) -> i32 {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

fn record_migration(conn: &rusqlite::Connection, version: i32) -> Result<(), rusqlite::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![version, now],
    )?;
    Ok(())
}

pub fn init_db(
    app_dir: &Path,
    migrations_dir: Option<&Path>,
) -> Result<DbPool, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(app_dir).map_err(|e| {
        format!(
            "Failed to create app data directory {}: {}",
            app_dir.display(),
            e
        )
    })?;

    let db_path = app_dir.join("cinema_ai.db");
    log::info!(
        "[init_db] Opening database at {} (migrations_dir={})",
        db_path.display(),
        migrations_dir
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "auto-detect".to_string())
    );

    let manager = SqliteConnectionManager::file(&db_path).with_init(|c| {
        c.execute_batch(
            "PRAGMA foreign_keys = ON; \
             PRAGMA journal_mode = WAL; \
             PRAGMA busy_timeout = 5000; \
             PRAGMA synchronous = NORMAL;",
        )
    });
    let pool = Pool::builder()
        // v0.23.20: 扩容到 50，缓冲 auto_commit/ingest/projection writers 并发占用。
        // 根因：auto_commit 的 run_kg_ingest 做 LLM 调用（30-90s）期间持有连接，
        // 叠加 projection writers + 用户自动保存，20 连接易耗尽。
        .max_size(50)
        // v0.23.19: connection_timeout 防止 pool.get() 在连接池耗尽时无限阻塞，
        // 导致 tokio worker 线程被卡死、tokio::time::timeout 无法触发。
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(manager)?;

    // Initialize tables
    let mut conn = pool.get()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\n            version INTEGER PRIMARY \
         KEY,\n            applied_at INTEGER NOT NULL\n        )",
        [],
    )?;

    create_tables(&mut conn)?;
    let runner = match migrations_dir {
        Some(dir) => MigrationRunner::new(dir),
        None => MigrationRunner::default_runner(),
    };
    runner
        .with_rust_migrations(all_rust_migrations())
        .run(&mut conn)?;

    // v0.26.30 hotfix: 兜底修复部分旧数据库在 inline migration → Rust migration
    // 切换过程中可能跳过 V099，导致 characters/scenes/world_buildings/kg_entities
    // 表缺失 source / is_auto_generated 列。该函数幂等，可多次执行。
    ensure_source_columns(&mut conn)?;

    log::info!("[init_db] Database initialized at {}", db_path.display());
    Ok(pool)
}

/// 确保关键资产表包含 source / is_auto_generated 列。
/// 用于修复 v0.26.28 迁移框架切换时可能遗漏的 schema 升级。
fn ensure_source_columns(conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let tables = ["characters", "scenes", "world_buildings", "kg_entities"];
    let mut modified = false;
    for table in tables {
        let cols: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({})", table))?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !cols.contains(&"source".to_string()) {
            log::info!("[init_db] Adding missing column {}.source", table);
            conn.execute(
                &format!(
                    "ALTER TABLE {} ADD COLUMN source TEXT DEFAULT 'user_created'",
                    table
                ),
                [],
            )?;
            modified = true;
        }
        if !cols.contains(&"is_auto_generated".to_string()) {
            log::info!(
                "[init_db] Adding missing column {}.is_auto_generated",
                table
            );
            conn.execute(
                &format!(
                    "ALTER TABLE {} ADD COLUMN is_auto_generated INTEGER NOT NULL DEFAULT 0",
                    table
                ),
                [],
            )?;
            modified = true;
        }
    }
    if modified {
        log::info!("[init_db] Source columns repaired successfully.");
    }
    Ok(())
}

fn create_tables(conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let current_version = get_current_version(conn);

    conn.execute_batch(
        r#"
        -- Stories table
        CREATE TABLE IF NOT EXISTS stories (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            genre TEXT,
            tone TEXT,
            pacing TEXT,
            style_dna_id TEXT,
            reference_book_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- Characters table
        CREATE TABLE IF NOT EXISTS characters (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            name TEXT NOT NULL,
            background TEXT,
            personality TEXT,
            goals TEXT,
            dynamic_traits TEXT, -- JSON array
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- Chapters table (保留用于向后兼容，新功能使用scenes表)
        CREATE TABLE IF NOT EXISTS chapters (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            chapter_number INTEGER NOT NULL,
            title TEXT,
            outline TEXT,
            content TEXT,
            word_count INTEGER,
            model_used TEXT,
            cost REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
            UNIQUE(story_id, chapter_number)
        );

        -- Create indexes
        CREATE INDEX IF NOT EXISTS idx_characters_story ON characters(story_id);
        CREATE INDEX IF NOT EXISTS idx_chapters_story ON chapters(story_id);
        CREATE INDEX IF NOT EXISTS idx_chapters_number ON chapters(story_id, chapter_number);
        "#,
    )?;
    // Migration 17: 创建任务表和任务日志表
    if current_version < 1 {
        let task_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if task_tables.is_empty() {
            conn.execute(
                "CREATE TABLE tasks (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    task_type TEXT NOT NULL DEFAULT 'custom',
                    schedule_type TEXT NOT NULL DEFAULT 'once',
                    cron_pattern TEXT,
                    payload TEXT,
                    status TEXT NOT NULL DEFAULT 'pending',
                    progress INTEGER NOT NULL DEFAULT 0,
                    result TEXT,
                    error_message TEXT,
                    max_retries INTEGER NOT NULL DEFAULT 3,
                    retry_count INTEGER NOT NULL DEFAULT 0,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    last_run_at TEXT,
                    next_run_at TEXT,
                    last_heartbeat_at TEXT,
                    heartbeat_timeout_seconds INTEGER NOT NULL DEFAULT 300,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_tasks_status ON tasks(status)", [])?;
            conn.execute("CREATE INDEX idx_tasks_type ON tasks(task_type)", [])?;
            conn.execute("CREATE INDEX idx_tasks_enabled ON tasks(enabled)", [])?;
            conn.execute("CREATE INDEX idx_tasks_next_run ON tasks(next_run_at)", [])?;
            conn.execute(
                "CREATE TABLE task_logs (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    log_level TEXT NOT NULL,
                    message TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_task_logs_task ON task_logs(task_id)", [])?;
        }
        record_migration(conn, 1)?;
    }

    // Migration 28: 创建协作会话表（协同编辑持久化)
    if current_version < 2 {
        let collab_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='collab_sessions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if collab_tables.is_empty() {
            conn.execute(
                "CREATE TABLE collab_sessions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    chapter_id TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE TABLE collab_participants (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    user_name TEXT NOT NULL,
                    cursor_line INTEGER,
                    cursor_column INTEGER,
                    joined_at TEXT NOT NULL,
                    FOREIGN KEY (session_id) REFERENCES collab_sessions(id) ON DELETE CASCADE,
                    UNIQUE(session_id, user_id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_collab_sessions_story ON collab_sessions(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_collab_participants_session ON collab_participants(session_id)",
                [],
            )?;
        }
        record_migration(conn, 2)?;
    }

    // Migration 29: 创建小说初始化会话追踪表
    if current_version < 3 {
        let bootstrap_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
                 name='novel_bootstrap_sessions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if bootstrap_tables.is_empty() {
            conn.execute(
                "CREATE TABLE novel_bootstrap_sessions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT,
                    status TEXT NOT NULL DEFAULT 'in_progress',
                    current_step TEXT NOT NULL DEFAULT 'concept',
                    steps_completed INTEGER DEFAULT 0,
                    total_steps INTEGER DEFAULT 5,
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    completed_at TEXT
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_bootstrap_story ON novel_bootstrap_sessions(story_id)",
                [],
            )?;
        }
        record_migration(conn, 3)?;
    }

    // Migration 39: 创建导出模板表
    if current_version < 5 {
        let export_template_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='export_templates'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if export_template_tables.is_empty() {
            conn.execute(
                "CREATE TABLE export_templates (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    format TEXT NOT NULL,
                    template_content TEXT NOT NULL,
                    is_builtin INTEGER NOT NULL DEFAULT 0,
                    is_user_created INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_export_templates_format ON export_templates(format)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_export_templates_builtin ON export_templates(is_builtin)",
                [],
            )?;
        }
        record_migration(conn, 5)?;
    }

    // Migration 40: 创建 AI 操作历史表
    if current_version < 6 {
        let ai_op_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ai_operations'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if ai_op_tables.is_empty() {
            conn.execute(
                "CREATE TABLE ai_operations (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_id TEXT,
                    operation_type TEXT NOT NULL,
                    operation_name TEXT NOT NULL,
                    input_summary TEXT,
                    output_summary TEXT,
                    previous_content TEXT,
                    new_content TEXT,
                    metadata TEXT,
                    status TEXT NOT NULL DEFAULT 'success',
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ai_operations_story ON ai_operations(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ai_operations_scene ON ai_operations(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ai_operations_chapter ON ai_operations(chapter_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ai_operations_type ON ai_operations(operation_type)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ai_operations_created ON ai_operations(created_at)",
                [],
            )?;
        }
        record_migration(conn, 6)?;
    }

    // Migration 38: 统一叙事元素表
    if current_version < 4 {
        let narrative_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_characters'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if narrative_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_characters (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    role_type TEXT,
                    personality TEXT,
                    background TEXT,
                    goals TEXT,
                    appearance TEXT,
                    gender TEXT,
                    age INTEGER,
                    importance_score REAL,
                    source TEXT NOT NULL DEFAULT 'user_created',
                    source_ref_id TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_narrative_chars_story ON \
                 narrative_characters(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_narrative_chars_source ON \
                 narrative_characters(source)",
                [],
            )?;

            conn.execute(
                "CREATE TABLE narrative_scenes (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    sequence_number INTEGER NOT NULL,
                    title TEXT,
                    summary TEXT,
                    dramatic_goal TEXT,
                    external_pressure TEXT,
                    conflict_type TEXT,
                    characters_present TEXT,
                    setting_location TEXT,
                    setting_time TEXT,
                    content TEXT,
                    key_events TEXT,
                    emotional_tone TEXT,
                    narrative_intensity REAL DEFAULT 0.0,
                    narrative_sentiment REAL DEFAULT 0.0,
                    narrative_event_types TEXT DEFAULT '[]',
                    act_number INTEGER DEFAULT 1,
                    position_in_act REAL DEFAULT 0.0,
                    source TEXT NOT NULL DEFAULT 'user_created',
                    source_ref_id TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_narrative_scenes_story ON \
                 narrative_scenes(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_narrative_scenes_source ON \
                 narrative_scenes(source)",
                [],
            )?;

            conn.execute(
                "CREATE TABLE narrative_world_buildings (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL UNIQUE,
                    concept TEXT NOT NULL,
                    rules TEXT,
                    history TEXT,
                    key_locations TEXT,
                    power_system TEXT,
                    source TEXT NOT NULL DEFAULT 'user_created',
                    source_ref_id TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_narrative_wb_story ON \
                 narrative_world_buildings(story_id)",
                [],
            )?;
        }
        record_migration(conn, 4)?;
    }

    conn.execute_batch(
        r#"
        -- ==================== V3 新表结构 ====================

        -- 场景表（取代章节表成为主要叙事单元）
        CREATE TABLE IF NOT EXISTS scenes (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            sequence_number INTEGER NOT NULL,
            title TEXT,
            dramatic_goal TEXT,             -- 戏剧目标：这个场景要完成什么
            external_pressure TEXT,         -- 外部压迫：环境/反派/事件对角色的压迫
            conflict_type TEXT,             -- 冲突类型
            characters_present TEXT,        -- JSON: [character_id, ...]
            character_conflicts TEXT,       -- JSON: [{a, b, nature, stakes}, ...]
            setting_location TEXT,
            setting_time TEXT,
            setting_atmosphere TEXT,
            content TEXT,
            previous_scene_id TEXT,
            next_scene_id TEXT,
            chapter_id TEXT,                -- 1:N Chapter↔Scene 关联
            model_used TEXT,
            cost REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
            FOREIGN KEY (previous_scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
            FOREIGN KEY (next_scene_id) REFERENCES scenes(id) ON DELETE SET NULL,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
            UNIQUE(story_id, sequence_number)
        );

        -- 世界观表
        CREATE TABLE IF NOT EXISTS world_buildings (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL UNIQUE,
            concept TEXT NOT NULL,          -- 宏观世界观概念
            rules TEXT,                     -- JSON: 世界规则列表
            history TEXT,
            cultures TEXT,                  -- JSON: 文化设定
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 世界规则表
        CREATE TABLE IF NOT EXISTS world_rules (
            id TEXT PRIMARY KEY,
            world_building_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            rule_type TEXT,                 -- magic/technology/social/...
            importance INTEGER,             -- 1-10
            created_at TEXT NOT NULL,
            FOREIGN KEY (world_building_id) REFERENCES world_buildings(id) ON DELETE CASCADE
        );

        -- 场景设置表（故事中的具体地点/时间设置）
        CREATE TABLE IF NOT EXISTS settings (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            location_type TEXT,             -- city/building/nature/...
            sensory_details TEXT,           -- JSON: 感官细节
            significance TEXT,              -- 在故事中的重要性
            created_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 文字风格表
        CREATE TABLE IF NOT EXISTS writing_styles (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL UNIQUE,
            name TEXT,
            description TEXT,
            tone TEXT,
            pacing TEXT,
            vocabulary_level TEXT,
            sentence_structure TEXT,
            custom_rules TEXT,              -- JSON: 自定义规则
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 知识图谱实体表
        CREATE TABLE IF NOT EXISTS kg_entities (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            name TEXT NOT NULL,
            entity_type TEXT NOT NULL,      -- character/location/item/concept/event/organization
            attributes TEXT,                -- JSON
            embedding BLOB,                 -- 向量嵌入（可选）
            first_seen TEXT NOT NULL,
            last_updated TEXT NOT NULL,
            confidence_score REAL,          -- 置信度 (0-1)
            access_count INTEGER DEFAULT 0, -- 访问计数（遗忘曲线）
            last_accessed TEXT,             -- 最后访问时间
            is_archived INTEGER DEFAULT 0,  -- 归档状态
            archived_at TEXT,               -- 归档时间
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 知识图谱关系表
        CREATE TABLE IF NOT EXISTS kg_relations (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation_type TEXT NOT NULL,
            strength REAL NOT NULL,         -- 0-1
            evidence TEXT,                  -- JSON: 场景ID列表
            first_seen TEXT NOT NULL,
            confidence_score REAL,          -- 置信度 (0-1)
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
            FOREIGN KEY (source_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
            FOREIGN KEY (target_id) REFERENCES kg_entities(id) ON DELETE CASCADE
        );

        -- 工作室配置表（存储每部小说的独立配置）
        CREATE TABLE IF NOT EXISTS studio_configs (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL UNIQUE,
            pen_name TEXT,
            llm_config TEXT,                -- JSON: LLM配置
            ui_config TEXT,                 -- JSON: UI配置
            agent_bots TEXT,                -- JSON: Agent Bot配置
            frontstage_theme TEXT,          -- CSS内容
            backstage_theme TEXT,           -- CSS内容
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 场景版本历史表
        CREATE TABLE IF NOT EXISTS scene_versions (
            id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            version_number INTEGER NOT NULL,
            title TEXT,
            content TEXT,
            dramatic_goal TEXT,
            external_pressure TEXT,
            conflict_type TEXT,
            characters_present TEXT,
            character_conflicts TEXT,
            setting_location TEXT,
            setting_time TEXT,
            setting_atmosphere TEXT,
            word_count INTEGER,
            change_summary TEXT NOT NULL,
            created_by TEXT NOT NULL,
            model_used TEXT,
            confidence_score REAL,
            previous_version_id TEXT,
            superseded_by TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
            FOREIGN KEY (previous_version_id) REFERENCES scene_versions(id) ON DELETE SET NULL,
            FOREIGN KEY (superseded_by) REFERENCES scene_versions(id) ON DELETE SET NULL
        );
        CREATE INDEX IF NOT EXISTS idx_scene_versions_scene ON scene_versions(scene_id);

        -- 场景批注表
        CREATE TABLE IF NOT EXISTS scene_annotations (
            id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            story_id TEXT NOT NULL,
            content TEXT NOT NULL,
            annotation_type TEXT NOT NULL DEFAULT 'note',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            resolved_at TEXT,
            FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 文本内联批注表（TipTap range comments）
        CREATE TABLE IF NOT EXISTS text_annotations (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            scene_id TEXT,
            chapter_id TEXT,
            content TEXT NOT NULL,
            annotation_type TEXT NOT NULL DEFAULT 'note',
            from_pos INTEGER NOT NULL,
            to_pos INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            resolved_at TEXT,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 故事摘要表（知识蒸馏、剧情总结等）
        CREATE TABLE IF NOT EXISTS story_summaries (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL,
            summary_type TEXT NOT NULL DEFAULT 'knowledge_distillation',
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
        );

        -- 变更追踪表（修订模式）
        CREATE TABLE IF NOT EXISTS change_tracks (
            id TEXT PRIMARY KEY,
            scene_id TEXT,
            chapter_id TEXT,
            version_id TEXT,
            author_id TEXT NOT NULL,
            author_name TEXT,
            change_type TEXT NOT NULL,
            from_pos INTEGER NOT NULL,
            to_pos INTEGER NOT NULL,
            content TEXT,
            status TEXT NOT NULL DEFAULT 'Pending',
            created_at TEXT NOT NULL,
            resolved_at TEXT,
            FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
            FOREIGN KEY (version_id) REFERENCES scene_versions(id) ON DELETE CASCADE
        );

        -- 评论线程表
        CREATE TABLE IF NOT EXISTS comment_threads (
            id TEXT PRIMARY KEY,
            scene_id TEXT,
            chapter_id TEXT,
            version_id TEXT,
            anchor_type TEXT NOT NULL,
            from_pos INTEGER,
            to_pos INTEGER,
            selected_text TEXT,
            status TEXT NOT NULL DEFAULT 'Open',
            created_at TEXT NOT NULL,
            resolved_at TEXT,
            FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
            FOREIGN KEY (version_id) REFERENCES scene_versions(id) ON DELETE CASCADE
        );

        -- 评论消息表
        CREATE TABLE IF NOT EXISTS comment_messages (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            author_name TEXT,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (thread_id) REFERENCES comment_threads(id) ON DELETE CASCADE
        );

        -- 创建索引
        CREATE INDEX IF NOT EXISTS idx_change_tracks_scene ON change_tracks(scene_id);
        CREATE INDEX IF NOT EXISTS idx_change_tracks_chapter ON change_tracks(chapter_id);
        CREATE INDEX IF NOT EXISTS idx_change_tracks_status ON change_tracks(status);
        CREATE INDEX IF NOT EXISTS idx_comment_threads_scene ON comment_threads(scene_id);
        CREATE INDEX IF NOT EXISTS idx_comment_threads_chapter ON comment_threads(chapter_id);
        CREATE INDEX IF NOT EXISTS idx_comment_messages_thread ON comment_messages(thread_id);
        CREATE INDEX IF NOT EXISTS idx_scenes_story ON scenes(story_id);
        CREATE INDEX IF NOT EXISTS idx_scenes_sequence ON scenes(story_id, sequence_number);
        CREATE INDEX IF NOT EXISTS idx_scenes_prev ON scenes(previous_scene_id);
        CREATE INDEX IF NOT EXISTS idx_scenes_next ON scenes(next_scene_id);
        
        CREATE INDEX IF NOT EXISTS idx_world_buildings_story ON world_buildings(story_id);
        CREATE INDEX IF NOT EXISTS idx_world_rules_wb ON world_rules(world_building_id);
        CREATE INDEX IF NOT EXISTS idx_settings_story ON settings(story_id);
        CREATE INDEX IF NOT EXISTS idx_writing_styles_story ON writing_styles(story_id);
        
        CREATE INDEX IF NOT EXISTS idx_kg_entities_story ON kg_entities(story_id);
        CREATE INDEX IF NOT EXISTS idx_kg_entities_type ON kg_entities(entity_type);
        CREATE INDEX IF NOT EXISTS idx_kg_relations_story ON kg_relations(story_id);
        CREATE INDEX IF NOT EXISTS idx_kg_relations_source ON kg_relations(source_id);
        CREATE INDEX IF NOT EXISTS idx_kg_relations_target ON kg_relations(target_id);
        CREATE INDEX IF NOT EXISTS idx_kg_relations_type ON kg_relations(relation_type);
        
        CREATE INDEX IF NOT EXISTS idx_studio_configs_story ON studio_configs(story_id);
        CREATE INDEX IF NOT EXISTS idx_scene_annotations_scene ON scene_annotations(scene_id);
        CREATE INDEX IF NOT EXISTS idx_scene_annotations_story ON scene_annotations(story_id);
        CREATE INDEX IF NOT EXISTS idx_scene_annotations_resolved ON scene_annotations(resolved_at);
        CREATE INDEX IF NOT EXISTS idx_text_annotations_story ON text_annotations(story_id);
        CREATE INDEX IF NOT EXISTS idx_text_annotations_scene ON text_annotations(scene_id);
        CREATE INDEX IF NOT EXISTS idx_text_annotations_chapter ON text_annotations(chapter_id);
        CREATE INDEX IF NOT EXISTS idx_text_annotations_resolved ON text_annotations(resolved_at);
        CREATE INDEX IF NOT EXISTS idx_story_summaries_story ON story_summaries(story_id);
        CREATE INDEX IF NOT EXISTS idx_story_summaries_type ON story_summaries(story_id, summary_type);

        -- 参考小说表（拆书功能）
        CREATE TABLE IF NOT EXISTS reference_books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT,
            genre TEXT,
            word_count INTEGER,
            file_format TEXT,
            file_hash TEXT UNIQUE,
            file_path TEXT,
            world_setting TEXT,
            plot_summary TEXT,
            story_arc TEXT,
            analysis_status TEXT NOT NULL DEFAULT 'pending',
            analysis_progress INTEGER DEFAULT 0,
            analysis_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- 参考人物表
        CREATE TABLE IF NOT EXISTS reference_characters (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            name TEXT NOT NULL,
            role_type TEXT,
            personality TEXT,
            appearance TEXT,
            relationships TEXT,
            key_scenes TEXT,
            importance_score REAL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (book_id) REFERENCES reference_books(id) ON DELETE CASCADE
        );

        -- 参考场景/章节表
        CREATE TABLE IF NOT EXISTS reference_scenes (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            sequence_number INTEGER NOT NULL,
            title TEXT,
            summary TEXT,
            characters_present TEXT,
            key_events TEXT,
            conflict_type TEXT,
            emotional_tone TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (book_id) REFERENCES reference_books(id) ON DELETE CASCADE
        );

        -- 拆书功能索引
        CREATE INDEX IF NOT EXISTS idx_ref_books_hash ON reference_books(file_hash);
        CREATE INDEX IF NOT EXISTS idx_ref_books_status ON reference_books(analysis_status);
        CREATE INDEX IF NOT EXISTS idx_ref_characters_book ON reference_characters(book_id);
        CREATE INDEX IF NOT EXISTS idx_ref_scenes_book ON reference_scenes(book_id);
        "#
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Error as SqliteError;

    use super::*;

    #[test]
    fn test_foreign_key_constraints_enabled() {
        let pool = create_test_pool().expect("Failed to create test pool");
        let conn = pool.get().expect("Failed to get connection");

        // 验证外键约束是否启用
        let foreign_keys_enabled: i32 = conn
            .prepare("PRAGMA foreign_keys")
            .expect("Failed to prepare PRAGMA statement")
            .query_row([], |row| row.get(0))
            .expect("Failed to query foreign_keys pragma");

        assert_eq!(
            foreign_keys_enabled, 1,
            "Foreign key constraints should be enabled"
        );
    }

    #[test]
    fn test_foreign_key_constraint_violation() {
        let pool = create_test_pool().expect("Failed to create test pool");
        let conn = pool.get().expect("Failed to get connection");

        // create_test_pool() already runs migrations via MigrationRunner

        // 尝试插入一个引用不存在故事的章节，应该失败
        let result = conn.execute(
            "INSERT INTO chapters (id, story_id, title, content, chapter_number, created_at, updated_at)
             VALUES ('test-chapter', 'non-existent-story', 'Test Chapter', 'Test content', 1, 0, 0)",
            []
        );

        // 应该因为外键约束而失败
        match result {
            Err(SqliteError::SqliteFailure(err, _)) => {
                // SQLITE_CONSTRAINT_FOREIGNKEY = 787
                assert_eq!(err.code, rusqlite::ErrorCode::ConstraintViolation);
            }
            _ => panic!(
                "Expected foreign key constraint violation, but operation succeeded or failed \
                 with different error"
            ),
        }
    }

    #[test]
    fn test_cascade_delete_behavior() {
        let pool = create_test_pool().expect("Failed to create test pool");
        let conn = pool.get().expect("Failed to get connection");

        // create_test_pool() already runs migrations via MigrationRunner

        // 创建一个测试故事
        conn.execute(
            "INSERT INTO stories (id, title, description, created_at, updated_at)
             VALUES ('test-story', 'Test Story', 'A test story', 0, 0)",
            [],
        )
        .expect("Failed to insert test story");

        // 创建一个测试章节
        conn.execute(
            "INSERT INTO chapters (id, story_id, title, content, chapter_number, created_at, \
             updated_at)
             VALUES ('test-chapter', 'test-story', 'Test Chapter', 'Test content', 1, 0, 0)",
            [],
        )
        .expect("Failed to insert test chapter");

        // 验证章节存在
        let chapter_count: i32 = conn
            .prepare("SELECT COUNT(*) FROM chapters WHERE story_id = 'test-story'")
            .expect("Failed to prepare count statement")
            .query_row([], |row| row.get(0))
            .expect("Failed to count chapters");
        assert_eq!(
            chapter_count, 1,
            "Chapter should exist before story deletion"
        );

        // 删除故事
        conn.execute("DELETE FROM stories WHERE id = 'test-story'", [])
            .expect("Failed to delete story");

        // 验证章节也被级联删除
        let chapter_count_after: i32 = conn
            .prepare("SELECT COUNT(*) FROM chapters WHERE story_id = 'test-story'")
            .expect("Failed to prepare count statement")
            .query_row([], |row| row.get(0))
            .expect("Failed to count chapters after deletion");
        assert_eq!(
            chapter_count_after, 0,
            "Chapter should be cascade deleted when story is deleted"
        );
    }

    #[test]
    fn test_comprehensive_cascade_delete() {
        let pool = create_test_pool().expect("Failed to create test pool");
        let conn = pool.get().expect("Failed to get connection");

        // create_test_pool() already runs migrations via MigrationRunner

        // 创建测试故事
        conn.execute(
            "INSERT INTO stories (id, title, description, created_at, updated_at)
             VALUES ('cascade-story', 'Cascade Test Story', 'Testing cascade deletes', \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test story");

        // 创建测试角色
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, created_at, updated_at)
             VALUES ('cascade-char1', 'cascade-story', 'Test Character 1', 'First test character', \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test character 1");

        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, created_at, updated_at)
             VALUES ('cascade-char2', 'cascade-story', 'Test Character 2', 'Second test \
             character', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test character 2");

        // 创建测试场景
        conn.execute(
            "INSERT INTO scenes (id, story_id, title, content, sequence_number, created_at, \
             updated_at)
             VALUES ('cascade-scene', 'cascade-story', 'Test Scene', 'Test scene content', 1, \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test scene");

        // 创建角色关系
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, created_at)
             VALUES ('cascade-rel', 'cascade-story', 'cascade-char1', 'cascade-char2', 'friend', \
             '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert character relationship");

        // 创建场景角色关联
        conn.execute(
            "INSERT INTO scene_characters (id, scene_id, character_id, created_at)
             VALUES ('cascade-sc1', 'cascade-scene', 'cascade-char1', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert scene character 1");

        conn.execute(
            "INSERT INTO scene_characters (id, scene_id, character_id, created_at)
             VALUES ('cascade-sc2', 'cascade-scene', 'cascade-char2', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert scene character 2");

        // 创建场景角色动作
        conn.execute(
            "INSERT INTO scene_character_actions (id, scene_id, character_id, action_type, \
             content, created_at)
             VALUES ('cascade-action', 'cascade-scene', 'cascade-char1', 'dialogue', 'Hello \
             world!', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert scene character action");

        // 创建叙事角色（如果表存在）
        let _ = conn.execute(
            "INSERT INTO narrative_characters (id, story_id, name, description, created_at)
             VALUES ('cascade-nchar', 'cascade-story', 'Narrative Character', 'Test narrative \
             character', '2024-01-01T00:00:00Z')",
            [],
        );

        // 创建叙事场景（如果表存在）
        let _ = conn.execute(
            "INSERT INTO narrative_scenes (id, story_id, title, content, created_at)
             VALUES ('cascade-nscene', 'cascade-story', 'Narrative Scene', 'Test narrative scene', \
             '2024-01-01T00:00:00Z')",
            [],
        );

        // 验证所有数据都存在
        let story_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM stories WHERE id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let char_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let scene_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scenes WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let rel_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let sc_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_characters WHERE scene_id = 'cascade-scene'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let action_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_character_actions WHERE scene_id = 'cascade-scene'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(story_count, 1, "Story should exist");
        assert_eq!(char_count, 2, "Characters should exist");
        assert_eq!(scene_count, 1, "Scene should exist");
        assert_eq!(rel_count, 1, "Character relationship should exist");
        assert_eq!(sc_count, 2, "Scene characters should exist");
        assert_eq!(action_count, 1, "Scene character action should exist");

        // 删除故事，触发级联删除
        conn.execute("DELETE FROM stories WHERE id = 'cascade-story'", [])
            .expect("Failed to delete story");

        // 验证所有相关数据都被级联删除
        let story_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM stories WHERE id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let char_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let scene_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scenes WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let rel_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id = 'cascade-story'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let sc_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_characters WHERE scene_id = 'cascade-scene'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let action_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_character_actions WHERE scene_id = 'cascade-scene'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(story_count_after, 0, "Story should be deleted");
        assert_eq!(char_count_after, 0, "Characters should be cascade deleted");
        assert_eq!(scene_count_after, 0, "Scenes should be cascade deleted");
        assert_eq!(
            rel_count_after, 0,
            "Character relationships should be cascade deleted"
        );
        assert_eq!(
            sc_count_after, 0,
            "Scene characters should be cascade deleted"
        );
        assert_eq!(
            action_count_after, 0,
            "Scene character actions should be cascade deleted"
        );

        // 验证叙事表也被级联删除（如果存在）
        let nchar_count_after: Result<i32, _> = conn.query_row(
            "SELECT COUNT(*) FROM narrative_characters WHERE story_id = 'cascade-story'",
            [],
            |row| row.get(0),
        );
        let nscene_count_after: Result<i32, _> = conn.query_row(
            "SELECT COUNT(*) FROM narrative_scenes WHERE story_id = 'cascade-story'",
            [],
            |row| row.get(0),
        );

        if let Ok(count) = nchar_count_after {
            assert_eq!(count, 0, "Narrative characters should be cascade deleted");
        }
        if let Ok(count) = nscene_count_after {
            assert_eq!(count, 0, "Narrative scenes should be cascade deleted");
        }
    }

    #[test]
    fn test_character_cascade_delete() {
        let pool = create_test_pool().expect("Failed to create test pool");
        let conn = pool.get().expect("Failed to get connection");

        // create_test_pool() already runs migrations via MigrationRunner

        // 创建测试故事
        conn.execute(
            "INSERT INTO stories (id, title, description, created_at, updated_at)
             VALUES ('char-cascade-story', 'Character Cascade Test', 'Testing character cascade \
             deletes', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test story");

        // 创建测试角色
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, created_at, updated_at)
             VALUES ('char-cascade-1', 'char-cascade-story', 'Character 1', 'First character', \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert character 1");

        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, created_at, updated_at)
             VALUES ('char-cascade-2', 'char-cascade-story', 'Character 2', 'Second character', \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert character 2");

        // 创建测试场景
        conn.execute(
            "INSERT INTO scenes (id, story_id, title, content, sequence_number, created_at, \
             updated_at)
             VALUES ('char-cascade-scene', 'char-cascade-story', 'Test Scene', 'Test scene', 1, \
             '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert test scene");

        // 创建角色关系
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, created_at)
             VALUES ('char-cascade-rel', 'char-cascade-story', 'char-cascade-1', 'char-cascade-2', \
             'friend', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert character relationship");

        // 创建场景角色关联
        conn.execute(
            "INSERT INTO scene_characters (id, scene_id, character_id, created_at)
             VALUES ('char-cascade-sc', 'char-cascade-scene', 'char-cascade-1', \
             '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert scene character");

        // 创建场景角色动作
        conn.execute(
            "INSERT INTO scene_character_actions (id, scene_id, character_id, action_type, \
             content, created_at)
             VALUES ('char-cascade-action', 'char-cascade-scene', 'char-cascade-1', 'dialogue', \
             'Test dialogue', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("Failed to insert scene character action");

        // 验证数据存在
        let rel_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE source_character_id = \
                 'char-cascade-1' OR target_character_id = 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let sc_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_characters WHERE character_id = 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let action_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_character_actions WHERE character_id = \
                 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(rel_count, 1, "Character relationship should exist");
        assert_eq!(sc_count, 1, "Scene character should exist");
        assert_eq!(action_count, 1, "Scene character action should exist");

        // 删除角色，触发级联删除
        conn.execute("DELETE FROM characters WHERE id = 'char-cascade-1'", [])
            .expect("Failed to delete character");

        // 验证相关数据被级联删除
        let rel_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE source_character_id = \
                 'char-cascade-1' OR target_character_id = 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let sc_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_characters WHERE character_id = 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let action_count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scene_character_actions WHERE character_id = \
                 'char-cascade-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(
            rel_count_after, 0,
            "Character relationships should be cascade deleted"
        );
        assert_eq!(
            sc_count_after, 0,
            "Scene characters should be cascade deleted"
        );
        assert_eq!(
            action_count_after, 0,
            "Scene character actions should be cascade deleted"
        );

        // 验证其他角色和数据仍然存在
        let char2_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE id = 'char-cascade-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let scene_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM scenes WHERE id = 'char-cascade-scene'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(char2_count, 1, "Other characters should remain");
        assert_eq!(scene_count, 1, "Scenes should remain");
    }

    /// Issue #4 contract: when init_db fails, setup must not construct
    /// GatewayExecutor (pool is not managed; calling state::<DbPool>()
    /// would panic on startup).
    #[test]
    fn issue_4_init_db_failure_returns_err_on_unwritable_app_dir() {
        let dir = std::env::temp_dir().join(format!("storyforge_issue4_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dir).expect("metadata").permissions();
            perms.set_mode(0o444);
            std::fs::set_permissions(&dir, perms).expect("set read-only");
        }

        #[cfg(windows)]
        {
            let mut cmd = std::process::Command::new("attrib");
            cmd.args(["+R", &dir.to_string_lossy()]);
            let status = cmd.status().expect("attrib +R");
            assert!(status.success(), "failed to mark temp dir read-only");
        }

        let result = init_db(&dir, None);
        assert!(
            result.is_err(),
            "init_db should fail when app data directory is not writable"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fresh app data directory should initialize successfully
    /// (Windows/macOS/Linux).
    #[test]
    fn init_db_succeeds_on_fresh_directory() {
        let dir =
            std::env::temp_dir().join(format!("storyforge_fresh_init_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let pool = init_db(&dir, None).expect("fresh init_db should succeed");
        assert!(pool.get().is_ok());

        let db_file = dir.join("cinema_ai.db");
        assert!(db_file.exists(), "cinema_ai.db should be created");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
