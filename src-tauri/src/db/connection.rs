use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;

use crate::db::migrations::MigrationRunner;

pub type DbPool = Pool<SqliteConnectionManager>;

/// 最高 inline migration 版本号。新增 SQL 文件 migration
/// 时，版本号必须大于此值， 否则会被 `MigrationRunner::run_with_legacy`
/// 提前应用，导致低版本 inline migrations 被跳过。
const MAX_INLINE_MIGRATION_VERSION: i32 = 98;

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
    MigrationRunner::default_runner().run_with_legacy(
        &mut conn,
        run_migrations,
        MAX_INLINE_MIGRATION_VERSION,
    )?;

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
    runner.run_with_legacy(
        &mut conn,
        run_migrations,
        MAX_INLINE_MIGRATION_VERSION,
    )?;

    log::info!("[init_db] Database initialized at {}", db_path.display());
    Ok(pool)
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

/// 数据库迁移
fn run_migrations(conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let current_version = get_current_version(conn);

    // Migration 25: 为 scenes 表添加结构化大纲字段
    if current_version < 28 {
        let scene_columns_m25: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m25.iter().any(|c| c == "execution_stage") {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN execution_stage TEXT DEFAULT 'drafting'",
                [],
            )?;
        }
        if !scene_columns_m25.iter().any(|c| c == "outline_content") {
            conn.execute("ALTER TABLE scenes ADD COLUMN outline_content TEXT", [])?;
        }
        if !scene_columns_m25.iter().any(|c| c == "draft_content") {
            conn.execute("ALTER TABLE scenes ADD COLUMN draft_content TEXT", [])?;
        }
        record_migration(conn, 28)?;
    }

    // Migration 26: 创建聊天会话和消息表（持久化聊天)
    if current_version < 29 {
        let chat_session_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='chat_sessions'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chat_session_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chat_sessions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    context TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chat_sessions_story ON chat_sessions(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE chat_messages (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chat_messages_session ON chat_messages(session_id)",
                [],
            )?;
        }
        record_migration(conn, 29)?;
    }

    // Migration 27: 创建故事运行状态表（持久化状态)
    if current_version < 30 {
        let story_state_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_runtime_states'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_state_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_runtime_states (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL UNIQUE,
                    state_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_runtime_states_story ON story_runtime_states(story_id)",
                [],
            )?;
        }
        record_migration(conn, 30)?;
    }

    // Migration 30: 创建故事风格混合配置表
    if current_version < 31 {
        let story_style_config_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_style_configs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_style_config_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_style_configs (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    name TEXT NOT NULL DEFAULT '默认混合',
                    blend_json TEXT NOT NULL,
                    is_active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_style_configs_story ON story_style_configs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_style_configs_active ON story_style_configs(story_id, \
                 is_active)",
                [],
            )?;
        }
        record_migration(conn, 31)?;
    }

    // Migration 31: 为 scenes 表添加风格混合覆盖字段
    if current_version < 32 {
        let scene_columns_m31: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m31
            .iter()
            .any(|c| c == "style_blend_override")
        {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN style_blend_override TEXT",
                [],
            )?;
        }
        record_migration(conn, 32)?;
    }

    // Migration 32: 用户认证系统
    if current_version < 33 {
        let auth_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if auth_tables.is_empty() {
            conn.execute(
                "CREATE TABLE users (
                    id TEXT PRIMARY KEY,
                    email TEXT UNIQUE,
                    display_name TEXT,
                    avatar_url TEXT,
                    is_local_user INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE TABLE oauth_accounts (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    provider TEXT NOT NULL,
                    provider_account_id TEXT NOT NULL,
                    access_token TEXT,
                    refresh_token TEXT,
                    expires_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(provider, provider_account_id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_oauth_accounts_user ON oauth_accounts(user_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_oauth_accounts_provider ON oauth_accounts(provider, \
                 provider_account_id)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    token TEXT NOT NULL UNIQUE,
                    expires_at TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_sessions_token ON sessions(token)", [])?;
            conn.execute("CREATE INDEX idx_sessions_user ON sessions(user_id)", [])?;
        }
        record_migration(conn, 33)?;
    }

    // Migration 33: subscriptions 表添加 real_user_id
    if current_version < 34 {
        let sub_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(subscriptions)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !sub_columns.iter().any(|c| c == "real_user_id") {
            conn.execute(
                "ALTER TABLE subscriptions ADD COLUMN real_user_id TEXT REFERENCES users(id)",
                [],
            )?;
        }
        record_migration(conn, 34)?;
    }

    // Migration 34: 创建故事大纲表
    if current_version < 35 {
        let outline_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='story_outlines'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if outline_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_outlines (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL UNIQUE,
                    content TEXT NOT NULL,
                    structure_json TEXT,
                    act_count INTEGER DEFAULT 3,
                    total_scenes_estimate INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_outlines_story ON story_outlines(story_id)",
                [],
            )?;
        }
        record_migration(conn, 35)?;
    }

    // Migration 35: characters 表增强 + character_relationships 表
    if current_version < 36 {
        let char_columns_m35: Vec<String> = conn
            .prepare("PRAGMA table_info(characters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !char_columns_m35.iter().any(|c| c == "appearance") {
            conn.execute("ALTER TABLE characters ADD COLUMN appearance TEXT", [])?;
        }
        if !char_columns_m35.iter().any(|c| c == "gender") {
            conn.execute("ALTER TABLE characters ADD COLUMN gender TEXT", [])?;
        }
        if !char_columns_m35.iter().any(|c| c == "age") {
            conn.execute("ALTER TABLE characters ADD COLUMN age INTEGER", [])?;
        }

        let rel_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
                 name='character_relationships'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if rel_tables.is_empty() {
            conn.execute(
                "CREATE TABLE character_relationships (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    source_character_id TEXT NOT NULL,
                    target_character_id TEXT NOT NULL,
                    relationship_type TEXT NOT NULL,
                    description TEXT,
                    dynamic TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (source_character_id) REFERENCES characters(id) ON DELETE CASCADE,
                    FOREIGN KEY (target_character_id) REFERENCES characters(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_char_rel_story ON character_relationships(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_char_rel_source ON character_relationships(source_character_id)",
                [],
            )?;
        }
        record_migration(conn, 36)?;
    }

    // Migration 36: scenes 表新增 foreshadowing_ids
    if current_version < 37 {
        let scene_columns_m36: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m36.iter().any(|c| c == "foreshadowing_ids") {
            conn.execute("ALTER TABLE scenes ADD COLUMN foreshadowing_ids TEXT", [])?;
        }
        record_migration(conn, 37)?;
    }

    // Migration 37: Chapter↔Scene 双轨映射
    if current_version < 38 {
        let chapter_columns_m37: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !chapter_columns_m37.iter().any(|c| c == "scene_id") {
            conn.execute(
                "ALTER TABLE chapters ADD COLUMN scene_id TEXT REFERENCES scenes(id) ON DELETE \
                 SET NULL",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_chapters_scene ON chapters(scene_id)",
                [],
            )?;
        }

        let scene_columns_m37: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m37.iter().any(|c| c == "chapter_id") {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN chapter_id TEXT REFERENCES chapters(id) ON DELETE \
                 SET NULL",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scenes_chapter ON scenes(chapter_id)",
                [],
            )?;
        }
        record_migration(conn, 38)?;
    }

    // Migration 41: 创建 Workflow 实例持久化表
    if current_version < 39 {
        let workflow_instance_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='workflow_instances'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if workflow_instance_tables.is_empty() {
            conn.execute(
                "CREATE TABLE workflow_instances (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL,
                    story_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    instance_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_workflow ON workflow_instances(workflow_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_story ON workflow_instances(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_workflow_instances_status ON workflow_instances(status)",
                [],
            )?;
        }
        record_migration(conn, 39)?;
    }

    // Migration 42: 创建 Pending Vector Indexes 表
    if current_version < 40 {
        let pending_vector_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
                 name='pending_vector_indexes'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if pending_vector_tables.is_empty() {
            conn.execute(
                "CREATE TABLE pending_vector_indexes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    chapter_id TEXT NOT NULL UNIQUE,
                    created_at INTEGER NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_pending_vector_chapter ON pending_vector_indexes(chapter_id)",
                [],
            )?;
        }
        record_migration(conn, 40)?;
    }

    // Migration 43: 创建 story_metadata 表
    if current_version < 41 {
        let story_metadata_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='story_metadata'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_metadata_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_metadata (
                    story_id TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (story_id, key),
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_metadata_story ON story_metadata(story_id)",
                [],
            )?;
        }
        record_migration(conn, 41)?;
    }

    // Migration 44: 创建 scene_characters 表
    if current_version < 42 {
        let scene_characters_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='scene_characters'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_characters_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_characters (
                    id TEXT PRIMARY KEY,
                    scene_id TEXT NOT NULL,
                    character_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE,
                    UNIQUE(scene_id, character_id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_characters_scene ON scene_characters(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_characters_character ON scene_characters(character_id)",
                [],
            )?;
        }
        record_migration(conn, 42)?;
    }

    // Migration 45: 创建 scene_character_actions 表
    if current_version < 43 {
        let scene_character_actions_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
                 name='scene_character_actions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_character_actions_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_character_actions (
                    id TEXT PRIMARY KEY,
                    scene_id TEXT NOT NULL,
                    character_id TEXT NOT NULL,
                    action_type TEXT,
                    content TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_character_actions_scene ON \
                 scene_character_actions(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_character_actions_character ON \
                 scene_character_actions(character_id)",
                [],
            )?;
        }
        record_migration(conn, 43)?;
    }

    // Migration 46: 创建 plan_templates 表
    if current_version < 44 {
        let plan_templates_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='plan_templates'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if plan_templates_tables.is_empty() {
            conn.execute(
                "CREATE TABLE plan_templates (
                    id TEXT PRIMARY KEY,
                    trigger_patterns TEXT NOT NULL,
                    plan_json TEXT NOT NULL,
                    success_count INTEGER NOT NULL DEFAULT 0,
                    failure_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_plan_templates_patterns ON plan_templates(trigger_patterns)",
                [],
            )?;
        }

        // ==================== Story System 合同驱动体系 ====================
        record_migration(conn, 44)?;
    }

    // Migration 47: 创建 story_contracts 表 — 合同真源
    if current_version < 45 {
        let story_contract_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_contracts'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_contract_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_contracts (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    contract_type TEXT NOT NULL,
                    contract_json TEXT NOT NULL,
                    version INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_contracts_story ON story_contracts(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_contracts_type ON story_contracts(story_id, contract_type)",
                [],
            )?;
        }
        record_migration(conn, 45)?;
    }

    // Migration 48: 创建 scene_commits 表 — Scene 提交链
    if current_version < 46 {
        let scene_commit_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='scene_commits'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_commit_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_commits (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_id TEXT REFERENCES chapters(id) ON DELETE SET NULL,
                    chapter_number INTEGER NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    outline_snapshot_json TEXT,
                    review_result_json TEXT,
                    fulfillment_result_json TEXT,
                    accepted_events_json TEXT,
                    state_deltas_json TEXT,
                    entity_deltas_json TEXT,
                    summary_text TEXT,
                    dominant_strand TEXT,
                    projection_status_json TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_commits_story ON scene_commits(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_commits_scene ON scene_commits(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX idx_scene_commits_number ON scene_commits(story_id, \
                 chapter_number)",
                [],
            )?;
        }
        record_migration(conn, 46)?;
    }

    // Migration 49: 创建 memory_items 表 — 长期记忆
    if current_version < 47 {
        let memory_item_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='memory_items'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if memory_item_tables.is_empty() {
            conn.execute(
                "CREATE TABLE memory_items (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    category TEXT NOT NULL,
                    subject TEXT,
                    field TEXT,
                    value TEXT,
                    source_chapter INTEGER,
                    confidence REAL NOT NULL DEFAULT 1.0,
                    status TEXT NOT NULL DEFAULT 'active',
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_story ON memory_items(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_category ON memory_items(story_id, category)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_status ON memory_items(story_id, status)",
                [],
            )?;
        }
        record_migration(conn, 47)?;
    }

    // Migration 50: 创建 chapter_reading_power 表 — 追读力
    if current_version < 48 {
        let reading_power_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
                 name='chapter_reading_power'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if reading_power_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chapter_reading_power (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_number INTEGER NOT NULL,
                    hook_type TEXT,
                    hook_strength TEXT DEFAULT 'medium',
                    coolpoint_patterns_json TEXT,
                    micropayoffs_json TEXT,
                    hard_violations_json TEXT,
                    soft_suggestions_json TEXT,
                    is_transition INTEGER NOT NULL DEFAULT 0,
                    override_count INTEGER NOT NULL DEFAULT 0,
                    debt_balance REAL NOT NULL DEFAULT 0.0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_reading_power_story ON chapter_reading_power(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX idx_reading_power_chapter ON chapter_reading_power(story_id, \
                 chapter_number)",
                [],
            )?;
        }
        record_migration(conn, 48)?;
    }

    // Migration 51: 创建 chase_debt 表 — 追读力债务
    if current_version < 49 {
        let chase_debt_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='chase_debt'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chase_debt_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chase_debt (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    story_id TEXT NOT NULL,
                    debt_type TEXT NOT NULL,
                    original_amount REAL NOT NULL DEFAULT 1.0,
                    current_amount REAL NOT NULL DEFAULT 1.0,
                    interest_rate REAL NOT NULL DEFAULT 0.1,
                    source_chapter INTEGER NOT NULL,
                    due_chapter INTEGER NOT NULL,
                    override_contract_id INTEGER,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chase_debt_story ON chase_debt(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chase_debt_status ON chase_debt(story_id, status)",
                [],
            )?;
        }
        record_migration(conn, 49)?;
    }

    // Migration 52: 创建 override_contracts 表 — 违背约束合约
    if current_version < 50 {
        let override_contract_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='override_contracts'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if override_contract_tables.is_empty() {
            conn.execute(
                "CREATE TABLE override_contracts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    story_id TEXT NOT NULL,
                    chapter_number INTEGER NOT NULL,
                    constraint_type TEXT NOT NULL,
                    constraint_id TEXT NOT NULL,
                    rationale_type TEXT NOT NULL,
                    rationale_text TEXT NOT NULL,
                    payback_plan TEXT NOT NULL,
                    due_chapter INTEGER NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    fulfilled_at TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_override_contracts_story ON override_contracts(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_override_contracts_status ON override_contracts(story_id, \
                 status)",
                [],
            )?;
        }
        record_migration(conn, 50)?;
    }

    // Migration 53: 创建 review_issues 表 — 结构化审查问题
    if current_version < 51 {
        let review_issue_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='review_issues'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if review_issue_tables.is_empty() {
            conn.execute(
                "CREATE TABLE review_issues (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_number INTEGER NOT NULL,
                    severity TEXT NOT NULL,
                    category TEXT NOT NULL,
                    location TEXT,
                    description TEXT NOT NULL,
                    evidence TEXT,
                    fix_hint TEXT,
                    blocking INTEGER NOT NULL DEFAULT 0,
                    resolved INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_story ON review_issues(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_severity ON review_issues(story_id, severity)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_blocking ON review_issues(story_id, blocking)",
                [],
            )?;
        }
        record_migration(conn, 51)?;
    }

    // Migration 54: 创建 genre_profiles 表 — 体裁画像
    if current_version < 52 {
        let genre_profile_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='genre_profiles'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if genre_profile_tables.is_empty() {
            conn.execute(
                "CREATE TABLE genre_profiles (
                    id TEXT PRIMARY KEY,
                    genre_name TEXT NOT NULL UNIQUE,
                    canonical_name TEXT NOT NULL,
                    aliases_json TEXT,
                    core_tone TEXT,
                    pacing_strategy TEXT,
                    anti_patterns_json TEXT,
                    reference_tables_json TEXT,
                    is_builtin INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genre_profiles_canonical ON genre_profiles(canonical_name)",
                [],
            )?;
        }
        record_migration(conn, 52)?;
    }

    // Migration 55: 为 chapters 表添加 writing_phase 字段 — 写作流程状态机
    if current_version < 53 {
        let chapter_columns_m55: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !chapter_columns_m55.iter().any(|c| c == "writing_phase") {
            conn.execute(
                "ALTER TABLE chapters ADD COLUMN writing_phase TEXT DEFAULT 'planning'",
                [],
            )?;
        }
        record_migration(conn, 53)?;
    }

    // Migration 56: 创建 ingest_jobs 表 — Ingest 作业追踪
    if current_version < 54 {
        let ingest_jobs_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ingest_jobs'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if ingest_jobs_tables.is_empty() {
            conn.execute(
                "CREATE TABLE ingest_jobs (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    resource_type TEXT NOT NULL,
                    resource_id TEXT,
                    status TEXT NOT NULL,
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    completed_at TEXT
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ingest_jobs_story ON ingest_jobs(story_id, created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ingest_jobs_status ON ingest_jobs(story_id, status)",
                [],
            )?;
        }
        record_migration(conn, 54)?;
    }

    // Migration 57: 创建 feature_usage_logs 表 — 功能使用度量
    if current_version < 55 {
        let feature_usage_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='feature_usage_logs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if feature_usage_tables.is_empty() {
            conn.execute(
                "CREATE TABLE feature_usage_logs (
                    id TEXT PRIMARY KEY,
                    feature_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    story_id TEXT,
                    metadata TEXT,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_feature_usage_feature ON feature_usage_logs(feature_id, \
                 created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_feature_usage_story ON feature_usage_logs(story_id, created_at)",
                [],
            )?;
        }

        // ==================== Pipeline 管线体系（基于 Vela
        // 学习借鉴）====================
        record_migration(conn, 55)?;
    }

    // Migration 58: 创建 blueprints 表 — 章节蓝图/细纲
    if current_version < 56 {
        let blueprint_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='blueprints'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if blueprint_tables.is_empty() {
            conn.execute(
                "CREATE TABLE blueprints (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    chapter_number INTEGER NOT NULL,
                    title TEXT,
                    role TEXT,
                    purpose TEXT,
                    key_events TEXT,
                    characters TEXT,
                    suspense_hook TEXT,
                    user_guidance TEXT,
                    notes TEXT,
                    notes_updated_at TEXT,
                    knowledge_query_hint TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    UNIQUE(story_id, chapter_number)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_blueprints_story ON blueprints(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_blueprints_chapter ON blueprints(story_id, chapter_number)",
                [],
            )?;
        }
        record_migration(conn, 56)?;
    }

    // Migration 59: 创建 drafts 表 — 草稿版本管理
    if current_version < 57 {
        let draft_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='drafts'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if draft_tables.is_empty() {
            conn.execute(
                "CREATE TABLE drafts (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    chapter_number INTEGER NOT NULL,
                    version INTEGER NOT NULL DEFAULT 1,
                    status TEXT NOT NULL DEFAULT 'draft',
                    source TEXT NOT NULL DEFAULT 'write',
                    content TEXT NOT NULL DEFAULT '',
                    word_count INTEGER NOT NULL DEFAULT 0,
                    model_used TEXT,
                    cost REAL,
                    metadata TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    UNIQUE(story_id, chapter_number, version)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_drafts_story_chapter ON drafts(story_id, chapter_number)",
                [],
            )?;
            conn.execute("CREATE INDEX idx_drafts_status ON drafts(status)", [])?;
            conn.execute(
                "CREATE INDEX idx_drafts_finalized ON drafts(story_id, chapter_number, status)",
                [],
            )?;
        }
        record_migration(conn, 57)?;
    }

    // Migration 60: 创建 revisions 表 — 修稿记录
    if current_version < 58 {
        let revision_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='revisions'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if revision_tables.is_empty() {
            conn.execute(
                "CREATE TABLE revisions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    draft_id TEXT NOT NULL,
                    revision_index INTEGER NOT NULL,
                    revision_type TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    user_prompt TEXT,
                    original_content TEXT NOT NULL,
                    revised_content TEXT NOT NULL,
                    word_count INTEGER NOT NULL DEFAULT 0,
                    change_summary TEXT,
                    model_used TEXT,
                    cost REAL,
                    metadata TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    FOREIGN KEY (draft_id) REFERENCES drafts(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_revisions_draft ON revisions(draft_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_revisions_story ON revisions(story_id)",
                [],
            )?;
        }
        record_migration(conn, 58)?;
    }

    // 不同，review_issues 是 Anti-AI 审查问题）
    if current_version < 59 {
        let review_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='reviews'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if review_tables.is_empty() {
            conn.execute(
                "CREATE TABLE reviews (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    draft_id TEXT NOT NULL,
                    review_index INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    dimensions TEXT,
                    issues TEXT,
                    overall_score REAL,
                    review_focus TEXT,
                    model_used TEXT,
                    cost REAL,
                    metadata TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                    FOREIGN KEY (draft_id) REFERENCES drafts(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_reviews_draft ON reviews(draft_id)", [])?;
            conn.execute("CREATE INDEX idx_reviews_story ON reviews(story_id)", [])?;
        }
        record_migration(conn, 59)?;
    }

    // 后处理管线持久化
    if current_version < 60 {
        let post_process_run_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='post_process_runs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if post_process_run_tables.is_empty() {
            conn.execute(
                "CREATE TABLE post_process_runs (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    chapter_number INTEGER NOT NULL,
                    source_label TEXT NOT NULL,
                    scope TEXT,
                    status TEXT NOT NULL DEFAULT 'running',
                    started_at TEXT NOT NULL,
                    completed_at TEXT,
                    error_message TEXT,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_runs_story ON post_process_runs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_runs_chapter ON post_process_runs(story_id, \
                 chapter_number)",
                [],
            )?;
        }

        let post_process_step_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='post_process_steps'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if post_process_step_tables.is_empty() {
            conn.execute(
                "CREATE TABLE post_process_steps (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    step_key TEXT NOT NULL,
                    step_label TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    critical INTEGER NOT NULL DEFAULT 0,
                    log_output TEXT,
                    error_message TEXT,
                    started_at TEXT,
                    completed_at TEXT,
                    FOREIGN KEY (run_id) REFERENCES post_process_runs(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_steps_run ON post_process_steps(run_id)",
                [],
            )?;
        }
        record_migration(conn, 60)?;
    }

    // Migration 63: 扩展 characters 表 — 添加 cs_* 动态状态字段
    if current_version < 61 {
        let character_columns_m63: Vec<String> = conn
            .prepare("PRAGMA table_info(characters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !character_columns_m63.iter().any(|c| c == "cs_location") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_location TEXT", [])?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_power_level") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_power_level TEXT", [])?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_physical_state")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_physical_state TEXT",
                [],
            )?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_mental_state") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_mental_state TEXT", [])?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_key_items") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_key_items TEXT", [])?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_recent_events")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_recent_events TEXT",
                [],
            )?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_updated_at_chapter")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_updated_at_chapter INTEGER",
                [],
            )?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_json") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_json TEXT", [])?;
        }
        record_migration(conn, 61)?;
    }

    // Migration 64: 创建 llm_calls 表 — LLM 用量统计与审计
    if current_version < 62 {
        let llm_call_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='llm_calls'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if llm_call_tables.is_empty() {
            conn.execute(
                "CREATE TABLE llm_calls (
                    id TEXT PRIMARY KEY,
                    story_id TEXT,
                    draft_id TEXT,
                    revision_id TEXT,
                    model_id TEXT NOT NULL,
                    model_name TEXT,
                    purpose TEXT NOT NULL,
                    prompt_tokens INTEGER NOT NULL DEFAULT 0,
                    completion_tokens INTEGER NOT NULL DEFAULT 0,
                    total_tokens INTEGER NOT NULL DEFAULT 0,
                    duration_ms INTEGER NOT NULL DEFAULT 0,
                    success INTEGER NOT NULL DEFAULT 1,
                    error_message TEXT,
                    prompt_preview TEXT,
                    metadata TEXT,
                    created_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_time ON llm_calls(created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_story ON llm_calls(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_purpose ON llm_calls(purpose)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_model ON llm_calls(model_id)",
                [],
            )?;
        }
        record_migration(conn, 62)?;
    }

    // Migration 65: AI 使用配额表添加 offline_grace_used 字段 (W1-B6: 离线配额快照)
    if current_version < 63 {
        // 注：配额系统已移除 (A1)，此迁移保留以兼容旧数据库，但跳过无表的情况
        let quota_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ai_usage_quota'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !quota_tables.is_empty() {
            let quota_v3_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(ai_usage_quota)")?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !quota_v3_columns.iter().any(|c| c == "offline_grace_used") {
                conn.execute(
                    "ALTER TABLE ai_usage_quota ADD COLUMN offline_grace_used INTEGER NOT NULL \
                     DEFAULT 0",
                    [],
                )?;
            }
        }
        record_migration(conn, 63)?;
    }

    // Migration 66: 创建 style_snapshots 表 — StyleDNA 六维向量存储 (W3-B7)
    if current_version < 64 {
        let snapshot_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='style_snapshots'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if snapshot_tables.is_empty() {
            conn.execute(
                "CREATE TABLE style_snapshots (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    chapter_number INTEGER,
                    scene_number INTEGER,
                    sentence_length REAL NOT NULL,
                    dialogue_ratio REAL NOT NULL,
                    metaphor_density REAL NOT NULL,
                    inner_monologue_ratio REAL NOT NULL,
                    emotion_density REAL NOT NULL,
                    rhythm_score REAL NOT NULL,
                    computed_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_style_snapshots_story ON style_snapshots(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_style_snapshots_story_chapter ON style_snapshots(story_id, \
                 chapter_number)",
                [],
            )?;
        }
        record_migration(conn, 64)?;
    }

    // (W3-B2/W3-B3)
    if current_version < 65 {
        for table in [
            "narrative_characters",
            "narrative_scenes",
            "narrative_world_buildings",
        ] {
            let columns: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !columns.iter().any(|c| c == "status") {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
                        table
                    ),
                    [],
                )?;
            }
        }
        record_migration(conn, 65)?;
    }

    // W2-B9: genesis_runs 表 — GenesisRun 状态机持久化
    if current_version < 66 {
        let genesis_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='genesis_runs'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if genesis_tables.is_empty() {
            conn.execute(
                "CREATE TABLE genesis_runs (
                    id TEXT PRIMARY KEY,
                    story_id TEXT,
                    session_id TEXT NOT NULL,
                    premise TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    current_step TEXT,
                    current_step_number INTEGER NOT NULL DEFAULT 0,
                    total_steps INTEGER NOT NULL DEFAULT 7,
                    steps_json TEXT NOT NULL DEFAULT '{}',
                    error_message TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_story ON genesis_runs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_session ON genesis_runs(session_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_status ON genesis_runs(status)",
                [],
            )?;
        }
        record_migration(conn, 66)?;
    }

    // 1:N Chapter↔Scene 聚合提交 (W2-B8)
    if current_version < 67 {
        let scene_commit_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='scene_commits'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;
        let table_name = if scene_commit_exists {
            "scene_commits"
        } else {
            "chapter_commits"
        };

        let cc_columns_m68: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({})", table_name))?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !cc_columns_m68.iter().any(|c| c == "chapter_id") {
            conn.execute(
                &format!(
                    "ALTER TABLE {} ADD COLUMN chapter_id TEXT REFERENCES chapters(id) ON DELETE \
                     SET NULL",
                    table_name
                ),
                [],
            )?;
            conn.execute(
                &format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_chapter ON {}(chapter_id)",
                    table_name.replace("_commits", "_commits"),
                    table_name
                ),
                [],
            )?;
        }
        record_migration(conn, 67)?;
    }

    // narrative_* 统一表 (W3-B3)
    if current_version < 68 {
        let has_reference_characters: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND \
                 name='reference_characters'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_reference_characters {
            conn.execute(
                "INSERT OR IGNORE INTO narrative_characters (
                    id, story_id, name, role_type, personality, background, goals, appearance,
                    gender, age, importance_score, source, source_ref_id, status, created_at, \
                 updated_at
                )
                SELECT
                    rc.id, rc.book_id, rc.name, rc.role_type, rc.personality, '', '', \
                 rc.appearance,
                    '', 0, COALESCE(rc.importance_score, 0.0), 'extracted', rc.book_id, \
                 'reference', rc.created_at, rc.created_at
                FROM reference_characters rc
                LEFT JOIN narrative_characters nc ON nc.id = rc.id
                WHERE nc.id IS NULL
                    AND EXISTS (SELECT 1 FROM stories s WHERE s.id = rc.book_id)",
                [],
            )?;
        }

        let has_reference_scenes: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='reference_scenes'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_reference_scenes {
            conn.execute(
                "INSERT OR IGNORE INTO narrative_scenes (
                    id, story_id, sequence_number, title, summary, dramatic_goal, \
                 external_pressure,
                    conflict_type, characters_present, setting_location, setting_time, content,
                    source, source_ref_id, status, created_at, updated_at
                )
                SELECT
                    rs.id, rs.book_id, rs.sequence_number, rs.title, rs.summary, '', '',
                    rs.conflict_type, rs.characters_present, '', '', NULL,
                    'extracted', rs.book_id, 'reference', rs.created_at, rs.created_at
                FROM reference_scenes rs
                LEFT JOIN narrative_scenes ns ON ns.id = rs.id
                WHERE ns.id IS NULL
                    AND EXISTS (SELECT 1 FROM stories s WHERE s.id = rs.book_id)",
                [],
            )?;
        }
        record_migration(conn, 68)?;
    }

    // Migration 70: chapter_commits 重命名为 scene_commits
    if current_version < 69 {
        let has_old_table: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chapter_commits'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_old_table {
            // 如果 Migration 48 已经创建了空的 scene_commits（旧数据库升级场景），先删除它
            let has_new_table: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND \
                     name='scene_commits'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0)
                > 0;
            if has_new_table {
                conn.execute("DROP TABLE scene_commits", [])?;
            }
            conn.execute("ALTER TABLE chapter_commits RENAME TO scene_commits", [])?;
            // SQLite RENAME TABLE 会自动更新大部分索引引用，
            // 但含有旧表名的索引名需要删除后重建
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_story", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_scene", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_number", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_chapter", [])?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_story ON scene_commits(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_scene ON scene_commits(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_scene_commits_number ON \
                 scene_commits(story_id, chapter_number)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_chapter ON scene_commits(chapter_id)",
                [],
            )?;
        }
        record_migration(conn, 69)?;
    }

    // Migration 71: 废弃 chapters.scene_id，完成 1:N 架构语义对齐
    if current_version < 70 {
        let chapter_columns_m71: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chapter_columns_m71.iter().any(|c| c == "scene_id") {
            // 必须先删除索引，再删除列（SQLite DROP COLUMN 不能删除有索引的列）
            // 使用显式事务包裹，避免 SQLite schema 缓存导致 drop column
            // 时仍能看到已删除的索引
            let tx = conn.transaction()?;
            tx.execute("DROP INDEX IF EXISTS idx_chapters_scene", [])?;
            tx.execute("ALTER TABLE chapters DROP COLUMN scene_id", [])?;
            tx.commit()?;
        }
        record_migration(conn, 70)?;
    }

    // Migration 72: 创建 scene_divider_nodes 表
    if current_version < 71 {
        let divider_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='scene_divider_nodes'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if divider_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_divider_nodes (
                    id TEXT PRIMARY KEY,
                    chapter_id TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    scene_id TEXT NOT NULL,
                    label TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    UNIQUE(chapter_id, position)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_divider_chapter ON scene_divider_nodes(chapter_id)",
                [],
            )?;
        }
        record_migration(conn, 71)?;
    }

    // Migration 73: 创建 entity_mentions 表 — Cascade Rewriter 实体引用索引 (D1)
    if current_version < 72 {
        let mention_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='entity_mentions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if mention_tables.is_empty() {
            conn.execute(
                "CREATE TABLE entity_mentions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    entity_type TEXT NOT NULL,
                    start_pos INTEGER NOT NULL,
                    end_pos INTEGER NOT NULL,
                    mention_text TEXT NOT NULL,
                    confidence REAL NOT NULL DEFAULT 1.0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_entity ON entity_mentions(entity_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_scene ON entity_mentions(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_story ON entity_mentions(story_id)",
                [],
            )?;
        }
        record_migration(conn, 72)?;
    }

    // Migration 74: 创建 narrative_events 表 — LitSeg 叙事事件模型 (E1)
    if current_version < 73 {
        let event_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_events'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if event_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_events (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    chapter_number INTEGER NOT NULL,
                    scene_id TEXT,
                    event_type TEXT NOT NULL,
                    intensity REAL NOT NULL DEFAULT 0.5,
                    sentiment REAL NOT NULL DEFAULT 0.0,
                    description TEXT NOT NULL,
                    involved_character_ids TEXT NOT NULL DEFAULT '[]',
                    conflict_types TEXT NOT NULL DEFAULT '[]',
                    preceding_event_id TEXT,
                    following_event_id TEXT,
                    act_number INTEGER NOT NULL DEFAULT 1,
                    position_in_act INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id),
                    FOREIGN KEY (preceding_event_id) REFERENCES narrative_events(id),
                    FOREIGN KEY (following_event_id) REFERENCES narrative_events(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_story ON narrative_events(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_chapter ON narrative_events(story_id, \
                 chapter_number)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_type ON narrative_events(event_type)",
                [],
            )?;
        }
        record_migration(conn, 73)?;
    }

    // Migration 75: 创建 narrative_threads 表 — LitSeg 叙事线索追踪 (E1)
    if current_version < 74 {
        let thread_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_threads'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if thread_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_threads (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    thread_type TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    thread_data TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_threads_story ON narrative_threads(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_threads_type ON narrative_threads(thread_type)",
                [],
            )?;
        }
        record_migration(conn, 74)?;
    }

    // (E1)
    if current_version < 75 {
        let position_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_structure_positions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if position_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_structure_positions (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    event_id TEXT NOT NULL REFERENCES narrative_events(id),
                    act_number INTEGER NOT NULL,
                    act_type TEXT NOT NULL,
                    position_in_act REAL NOT NULL,
                    dramatic_function TEXT NOT NULL,
                    is_narrative_boundary INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id),
                    FOREIGN KEY (event_id) REFERENCES narrative_events(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_positions_story ON \
                 narrative_structure_positions(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_positions_boundary ON \
                 narrative_structure_positions(is_narrative_boundary)",
                [],
            )?;
        }
        record_migration(conn, 75)?;
    }

    // Migration 77: 创建 narrative_structure 表 — LitSeg 叙事结构幕级划分 (E1)
    if current_version < 76 {
        let structure_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_structure'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if structure_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_structure (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    act_number INTEGER NOT NULL,
                    act_type TEXT NOT NULL,
                    start_chapter INTEGER NOT NULL,
                    end_chapter INTEGER NOT NULL,
                    summary TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_story ON narrative_structure(story_id)",
                [],
            )?;
        }
        record_migration(conn, 76)?;
    }

    // Migration 78: 创建 narrative_chunks 表 — LitSeg 叙事感知分段 (E1)
    if current_version < 77 {
        let chunk_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_chunks'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chunk_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_chunks (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    chapter_range_start INTEGER NOT NULL,
                    chapter_range_end INTEGER NOT NULL,
                    scene_ids TEXT NOT NULL DEFAULT '[]',
                    event_ids TEXT NOT NULL DEFAULT '[]',
                    text TEXT NOT NULL,
                    chunk_type TEXT NOT NULL,
                    is_boundary_start INTEGER NOT NULL DEFAULT 0,
                    is_boundary_end INTEGER NOT NULL DEFAULT 0,
                    thread_ids TEXT NOT NULL DEFAULT '[]',
                    vector_id TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_story ON narrative_chunks(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_type ON narrative_chunks(chunk_type)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_boundary ON \
                 narrative_chunks(is_boundary_start, is_boundary_end)",
                [],
            )?;
        }
        record_migration(conn, 77)?;
    }

    // Migration 79: 增强 scenes 表 — 添加 LitSeg 叙事分析字段
    if current_version < 78 {
        let scene_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_cols.contains(&"narrative_intensity".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_intensity REAL DEFAULT 0.5",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_sentiment".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_sentiment REAL DEFAULT 0.0",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_event_types".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_event_types TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_preceding_scene_id".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_preceding_scene_id TEXT",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_following_scene_id".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_following_scene_id TEXT",
                [],
            )?;
        }
        if !scene_cols.contains(&"act_number".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN act_number INTEGER DEFAULT 1",
                [],
            )?;
        }
        if !scene_cols.contains(&"position_in_act".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN position_in_act INTEGER DEFAULT 1",
                [],
            )?;
        }
        record_migration(conn, 78)?;
    }

    // Migration 80: 增强 foreshadowing_tracker 表 — 添加 LitSeg 事件关联
    if current_version < 79 {
        let fs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(foreshadowing_tracker)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !fs_cols.contains(&"setup_event_id".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN setup_event_id TEXT",
                [],
            )?;
        }
        if !fs_cols.contains(&"payoff_event_id".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN payoff_event_id TEXT",
                [],
            )?;
        }
        if !fs_cols.contains(&"risk_signals_score".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN risk_signals_score REAL DEFAULT 0.0",
                [],
            )?;
        }
        record_migration(conn, 79)?;
    }

    // Migration 81: 增强 character_states 表 — 添加弧光追踪
    if current_version < 80 {
        let cs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(character_states)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !cs_cols.contains(&"state_transitions_json".to_string()) {
            conn.execute(
                "ALTER TABLE character_states ADD COLUMN state_transitions_json TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !cs_cols.contains(&"arc_type".to_string()) {
            conn.execute(
                "ALTER TABLE character_states ADD COLUMN arc_type TEXT DEFAULT 'positive'",
                [],
            )?;
        }
        record_migration(conn, 80)?;
    }

    // Migration 82: 增强 story_outlines 表 — 添加分析后的结构
    if current_version < 81 {
        let so_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(story_outlines)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !so_cols.contains(&"analyzed_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE story_outlines ADD COLUMN analyzed_structure_json TEXT",
                [],
            )?;
        }
        record_migration(conn, 81)?;
    }

    // Migration 83: 新建 conflict_escalations 表
    if current_version < 82 {
        let ce_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='conflict_escalations'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if ce_tables.is_empty() {
            conn.execute(
                "CREATE TABLE conflict_escalations (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL REFERENCES stories(id),
                    conflict_type TEXT NOT NULL,
                    party_a_ids TEXT NOT NULL DEFAULT '[]',
                    party_b_ids TEXT NOT NULL DEFAULT '[]',
                    intensity_timeline_json TEXT NOT NULL DEFAULT '[]',
                    current_intensity REAL NOT NULL DEFAULT 0.0,
                    is_escalated INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (story_id) REFERENCES stories(id)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_conflict_escalations_story ON conflict_escalations(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_conflict_escalations_type ON conflict_escalations(conflict_type)",
                [],
            )?;
        }
        record_migration(conn, 82)?;
    }

    // Migration 84: 删除冗余的 LitSeg 表（数据已迁移到增强后的现有表）
    if current_version < 83 {
        // 删除 narrative_events 表（功能已合并到 scenes 表）
        conn.execute("DROP TABLE IF EXISTS narrative_events", [])?;
        // 删除 narrative_threads 表（功能已拆分到
        // foreshadowing_tracker/character_states/conflict_escalations）
        conn.execute("DROP TABLE IF EXISTS narrative_threads", [])?;
        // 删除 narrative_structure 表（功能已合并到
        // story_outlines.analyzed_structure_json）
        conn.execute("DROP TABLE IF EXISTS narrative_structure", [])?;
        // 清理相关索引
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_story", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_chapter", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_type", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_threads_story", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_threads_type", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_structure_story", [])?;
        record_migration(conn, 83)?;
    }

    // Migration 85: 增强 reference_scenes 表 — 添加 LitSeg 叙事分析字段
    if current_version < 85 {
        let rs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(reference_scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !rs_cols.contains(&"narrative_intensity".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_intensity REAL",
                [],
            )?;
        }
        if !rs_cols.contains(&"narrative_sentiment".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_sentiment REAL",
                [],
            )?;
        }
        if !rs_cols.contains(&"narrative_event_types".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_event_types TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !rs_cols.contains(&"act_number".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN act_number INTEGER DEFAULT 1",
                [],
            )?;
        }
        if !rs_cols.contains(&"position_in_act".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN position_in_act REAL DEFAULT 0.0",
                [],
            )?;
        }
        record_migration(conn, 85)?;
    }

    // Migration 86: 增强 reference_books 表 — 添加 LitSeg 分析后的叙事结构
    if current_version < 86 {
        let rb_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(reference_books)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !rb_cols.contains(&"analyzed_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE reference_books ADD COLUMN analyzed_structure_json TEXT",
                [],
            )?;
        }
        record_migration(conn, 86)?;
    }

    // Migration 87: 扩展 genre_profiles 表 — 添加 typical_structure_json 字段
    if current_version < 87 {
        let genre_profile_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(genre_profiles)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !genre_profile_cols.contains(&"typical_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE genre_profiles ADD COLUMN typical_structure_json TEXT",
                [],
            )?;
        }
        record_migration(conn, 87)?;
    }

    // Migration 88: 扩展 stories 表 — 添加 genre_profile_id 字段
    if current_version < 88 {
        let story_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(stories)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !story_cols.contains(&"genre_profile_id".to_string()) {
            conn.execute("ALTER TABLE stories ADD COLUMN genre_profile_id TEXT", [])?;
        }
        record_migration(conn, 88)?;
    }

    // v0.11.0: 为 llm_calls 表补充模型健康与反馈闭环字段
    if current_version < 89 {
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(llm_calls)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !columns.iter().any(|c| c == "task_type") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN task_type TEXT", [])?;
        }
        if !columns.iter().any(|c| c == "quality_score") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN quality_score REAL", [])?;
        }
        if !columns.iter().any(|c| c == "latency_ms") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN latency_ms INTEGER", [])?;
        }
        if !columns.iter().any(|c| c == "route_decision") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN route_decision TEXT", [])?;
        }
        if !columns.iter().any(|c| c == "audit_feedback") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN audit_feedback TEXT", [])?;
        }
        record_migration(conn, 89)?;
    }

    // Migration 90: 为 text_annotations 添加 metadata 和 severity 列
    // 支持分时架构时间线 2 的异步审计 annotation（ai_audit 类型）
    if current_version < 90 {
        let ta_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(text_annotations)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !ta_columns.iter().any(|c| c == "metadata") {
            conn.execute("ALTER TABLE text_annotations ADD COLUMN metadata TEXT", [])?;
        }
        if !ta_columns.iter().any(|c| c == "severity") {
            conn.execute(
                "ALTER TABLE text_annotations ADD COLUMN severity TEXT NOT NULL DEFAULT 'medium'",
                [],
            )?;
        }
        record_migration(conn, 90)?;
    }

    // Migration 91: v0.15.0 ModelGateway 算力档案持久化表
    // 网关升级为智能调度器，存储每个模型的流式基准实测数据，
    // 跨应用启动保留，用于任务复杂度路由（LightTool→快模型，
    // HeavyCreation→推理大模型）
    if current_version < 91 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_capability_profile (
                model_id              TEXT PRIMARY KEY,
                short_ttfb_ms_p50     INTEGER,
                short_ttfb_ms_p95     INTEGER,
                long_ttfb_ms_p50      INTEGER,
                long_ttfb_ms_p95      INTEGER,
                sustained_tps         REAL,
                short_output_tps      REAL,
                success_rate_24h      REAL,
                last_full_benchmark_at INTEGER,
                last_health_probe_at  INTEGER,
                benchmark_sample_count INTEGER NOT NULL DEFAULT 0,
                status                TEXT NOT NULL DEFAULT 'unknown',
                status_reason         TEXT,
                capability_score      REAL,
                speed_score           REAL,
                quality_score         REAL,
                created_at            INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at            INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_capability_status
                ON model_capability_profile(status);
            CREATE INDEX IF NOT EXISTS idx_capability_score
                ON model_capability_profile(capability_score);
            ",
        )?;
        record_migration(conn, 91)?;
    }

    // Migration 92: v0.17.0 中文叙事增强 —— 桥段卡 / 剧情引擎 / 高压关系
    // 三类创作资产持久化 + genre_profiles.reader_promise 字段。
    // 设计：与既有 methodology / genre_profile / style_dna 同构，统一接入
    // strategy::AssetKind 与 StrategySelector 路由。所有内置数据由代码侧
    // 注入（builtin = 1），允许用户在 UI 中追加自定义条目（builtin = 0）。
    if current_version < 92 {
        // genre_profiles 新增 reader_promise 字段（爽 / 甜 / 虐 / 恨 / 惊 /
        // 燃 / 怕 / 痛 / 治愈 等读者主情绪承诺，用于注入 Writer prompt）
        let gp_cols_92: Vec<String> = conn
            .prepare("PRAGMA table_info(genre_profiles)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if !gp_cols_92.iter().any(|c| c == "reader_promise") {
            conn.execute(
                "ALTER TABLE genre_profiles ADD COLUMN reader_promise TEXT",
                [],
            )?;
        }

        // 桥段卡表（beat_cards）—— 经典叙事功能模板，可被 outline / writer 引用
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS beat_cards (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL UNIQUE,
                category        TEXT NOT NULL,
                function        TEXT NOT NULL,
                when_to_use     TEXT NOT NULL,
                remix_hint      TEXT,
                avoid           TEXT,
                tags_json       TEXT,
                is_builtin      INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_beat_cards_category
                ON beat_cards(category);
            CREATE INDEX IF NOT EXISTS idx_beat_cards_builtin
                ON beat_cards(is_builtin);
            ",
        )?;

        // 剧情引擎表（story_engines）—— 21 种正交叙事引擎，可组合 2-4 个
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS story_engines (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL UNIQUE,
                payoff          TEXT NOT NULL,
                best_payoff     TEXT,
                avoid           TEXT,
                pairs_well_with TEXT,
                tags_json       TEXT,
                is_builtin      INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_story_engines_builtin
                ON story_engines(is_builtin);
            ",
        )?;

        // 高压关系表（pressure_relationships）—— 13 种角色对位关系
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pressure_relationships (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL UNIQUE,
                pressure_source TEXT NOT NULL,
                works_with      TEXT,
                tags_json       TEXT,
                is_builtin      INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_pressure_relationships_builtin
                ON pressure_relationships(is_builtin);
            ",
        )?;

        record_migration(conn, 92)?;
    }

    // Migration 93: v0.17.1 提示词注册表 —— 用户对内置提示词的覆盖
    // 目的：把分散在 prompts/engine.rs / llm/prompt.rs /
    // task_system/audit_executor.rs 的硬编码 prompt
    // 全部抽取到一个可在前端编辑的注册表。 表结构：prompt_id 是 PromptRegistry
    // 中的稳定 ID（如 "writer_system"）， overridden_content
    // 是用户自定义版本；为空则回退到内置默认值。
    if current_version < 93 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prompt_overrides (
                prompt_id           TEXT PRIMARY KEY,
                overridden_content  TEXT NOT NULL,
                updated_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_prompt_overrides_updated
                ON prompt_overrides(updated_at);
            ",
        )?;
        record_migration(conn, 93)?;
    }

    // Migration 94: 删除从未使用的死表
    // beat_cards / story_engines / pressure_relationships 由 Migration 92 创建，
    // 但全仓无任何 INSERT 或 SELECT——资产数据只存在于内存的 builtin_*() 函数。
    // 审计报告 P2-3：删除死 schema，避免维护空表与混淆。
    // 若未来需要用户自定义条目，可重新创建并实现完整 Repository + IPC。
    if current_version < 94 {
        conn.execute_batch(
            "DROP TABLE IF EXISTS beat_cards;
             DROP TABLE IF EXISTS story_engines;
             DROP TABLE IF EXISTS pressure_relationships;",
        )?;
        record_migration(conn, 94)?;
    }

    // v0.22.1 Migration 96: genre_profiles 添加推荐资产字段（Phase F）
    if current_version < 96 {
        // 检查表是否存在（迁移 52 可能因版本跳跃未执行）
        let table_exists: bool = conn
            .prepare(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='genre_profiles'",
            )?
            .query_row([], |row| row.get::<_, i32>(0))
            .map(|count| count > 0)
            .unwrap_or(false);

        if table_exists {
            let genre_cols: Vec<String> = conn
                .prepare("PRAGMA table_info(genre_profiles)")?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !genre_cols.contains(&"recommended_style_dna_ids".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_style_dna_ids TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"recommended_methodology_id".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_methodology_id TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"recommended_skill_ids".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_skill_ids TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"min_quality_tier".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN min_quality_tier TEXT DEFAULT 'low'",
                    [],
                )?;
            }
        }
        record_migration(conn, 96)?;
    }

    // Migration 97: 扩展 stories 表 — 添加 reference_book_id 字段（拆书功能关联）
    if current_version < 97 {
        let story_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(stories)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !story_cols.contains(&"reference_book_id".to_string()) {
            conn.execute("ALTER TABLE stories ADD COLUMN reference_book_id TEXT", [])?;
        }
        record_migration(conn, 97)?;
    }

    // Migration 98: 为 narrative_* 表补 status 列（与 repository/query.rs 对齐）
    if current_version < 98 {
        for table in [
            "narrative_characters",
            "narrative_scenes",
            "narrative_world_buildings",
        ] {
            let cols: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !cols.contains(&"status".to_string()) {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
                        table
                    ),
                    [],
                )?;
            }
        }
        record_migration(conn, 98)?;
    }

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

    /// Fresh app data directory should initialize successfully (Windows/macOS/Linux).
    #[test]
    fn init_db_succeeds_on_fresh_directory() {
        let dir = std::env::temp_dir().join(format!(
            "storyforge_fresh_init_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let pool = init_db(&dir, None).expect("fresh init_db should succeed");
        assert!(pool.get().is_ok());

        let db_file = dir.join("cinema_ai.db");
        assert!(db_file.exists(), "cinema_ai.db should be created");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
