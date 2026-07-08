#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 中性领域类型模块，必须在业务模块之前声明，供各层依赖。
mod domain;

// 基础设施 ports（trait 契约），必须在具体实现模块之前声明。
mod ports;

mod agents;
mod analytics;
mod anti_ai;
mod audit;
mod auth;
mod automation;
mod book_deconstruction;
mod canonical_state;
mod capabilities;
mod chat; // RESERVED: story-associated chat sessions (Phase 4)
mod collab; // RESERVED: collaborative editing WebSocket server (Phase 4)
mod config;
mod creation_commands;
mod creative_engine;
mod db;
mod diagnostics;
pub(crate) mod embeddings;
mod error;
mod error_recovery;
mod events;
mod export;
mod intent;
mod intention_graph;
mod knowledge_base;
mod llm;
mod logging;
mod mcp;
mod memory;
mod model_gateway;
mod narrative;
mod pipeline;
mod planner;
mod prompts;
mod reading_power;
mod revision_commands;
mod router;
mod scene_commands;
mod skills;
mod state_sync;
mod story_system;
mod strategy;
mod studio_commands;
mod subscription;
mod task_system;
mod telemetry;
mod tracing;
mod updater;
mod utils;
mod vector;
mod versions;
mod window;
mod workflow;
mod workflow_logger;
mod workspace;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;
#[macro_use]
mod commands;

use std::collections::HashMap;

use config::AppConfig;
use db::{init_db, DbPool};
use once_cell::sync::Lazy;
use serde::Deserialize;
use skills::SkillManager;
use tauri::{path::BaseDirectory, Manager};

// NOTE: Collab WebSocket server is reserved for future use (Phase 4)
// use collab::websocket::WebSocketServer;

/// 记录 AI 操作历史
pub(crate) fn record_ai_operation(pool: &DbPool, req: db::CreateAiOperationRequest) {
    let repo = db::AiOperationRepository::new(pool.clone());
    if let Err(e) = repo.create(req) {
        log::warn!("[AiOperation] Failed to record operation: {}", e);
    }
}

/// 优雅关闭：WAL checkpoint、保存向量索引、然后退出
fn graceful_shutdown(app_handle: &tauri::AppHandle) {
    log::info!("[Shutdown] Starting graceful shutdown...");

    // 1. SQLite WAL checkpoint — 确保所有数据已写入主数据库
    let pool_result: Result<DbPool, _> = app_handle
        .try_state::<DbPool>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| "No DB pool managed in Tauri state".to_string());
    match &pool_result {
        Ok(pool) => {
            if let Ok(conn) = pool.get() {
                match conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)") {
                    Ok(_) => log::info!("[Shutdown] WAL checkpoint completed"),
                    Err(e) => log::warn!("[Shutdown] WAL checkpoint failed: {}", e),
                }
            } else {
                log::warn!("[Shutdown] Failed to get DB connection for checkpoint");
            }
        }
        Err(e) => {
            log::warn!("[Shutdown] No DB pool available for checkpoint: {}", e);
        }
    }

    // 2. 保存待处理的向量索引
    if let Ok(pool) = &pool_result {
        if let Some(queue) = PendingVectorIndexQueue::from_app_handle(app_handle) {
            queue.save_to_db(pool);
        }
    }
    log::info!("[Shutdown] Pending vector indexes saved");

    // 3. 停止自动化服务
    if let Ok(automation) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app_handle.state::<crate::automation::service::AutomationService>()
    })) {
        automation.shutdown();
        log::info!("[Shutdown] Automation service stop requested");
    } else {
        log::warn!("[Shutdown] Automation service not available for shutdown");
    }

    // 4. 退出应用
    log::info!("[Shutdown] Exiting application");
    std::process::exit(0);
}

/// 种子内置数据：StyleDNA、导出模板、GenreProfiles
fn seed_builtin_data(pool: &DbPool, app_dir: &std::path::Path) {
    // Seed built-in StyleDNAs
    let style_repo = db::StyleDnaRepository::new(pool.clone());
    match style_repo.get_builtin() {
        Ok(existing) if existing.is_empty() => {
            log::info!("[StyleDNA] Seeding built-in styles...");
            for style in creative_engine::style::classic_styles::get_builtin_styles() {
                if let Ok(dna_json) = serde_json::to_string(&style) {
                    let _ = style_repo.create(
                        &style.meta.name,
                        style.meta.author.as_deref(),
                        &dna_json,
                        true,
                    );
                }
            }
            log::info!("[StyleDNA] Built-in styles seeded successfully");
        }
        Ok(_) => log::info!("[StyleDNA] Built-in styles already exist, skipping seed"),
        Err(e) => log::warn!("[StyleDNA] Failed to check existing styles: {}", e),
    }

    // Seed built-in export templates
    let template_repo = db::ExportTemplateRepository::new(pool.clone());
    match template_repo.seed_builtin_templates() {
        Ok(_) => log::info!("[ExportTemplates] Built-in templates seeded successfully"),
        Err(e) => {
            log::warn!("[ExportTemplates] Failed to seed built-in templates: {}", e)
        }
    }

    // Seed genre profiles
    let templates_dir = app_dir.join("templates");
    let user_genres_path = templates_dir.join("genres.json");
    let default_genres_json = include_str!("../../templates/genres.json");

    if !user_genres_path.exists() {
        let _ = std::fs::create_dir_all(&templates_dir);
        if let Err(e) = std::fs::write(&user_genres_path, default_genres_json) {
            log::warn!(
                "[GenreProfiles] Failed to copy default genres.json to app dir: {}",
                e
            );
        } else {
            log::info!(
                "[GenreProfiles] Copied default genres.json to {:?}",
                user_genres_path
            );
        }
    }

    if let Ok(json_str) = std::fs::read_to_string(&user_genres_path) {
        match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(genres_data) => {
                if let Some(profiles) = genres_data.get("profiles").and_then(|p| p.as_array()) {
                    let repo = db::GenreProfileRepository::new(pool.clone());
                    for profile in profiles {
                        let genre_name = profile
                            .get("genre_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let canonical_name = profile
                            .get("canonical_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if genre_name.is_empty() || canonical_name.is_empty() {
                            continue;
                        }
                        let aliases_json = profile.get("aliases").map(|v| v.to_string());
                        let core_tone = profile.get("core_tone").and_then(|v| v.as_str());
                        let pacing_strategy =
                            profile.get("pacing_strategy").and_then(|v| v.as_str());
                        let anti_patterns_json =
                            profile.get("anti_patterns").map(|v| v.to_string());
                        let reference_tables_json =
                            profile.get("reference_tables").and_then(|v| v.as_str());
                        let typical_structure_json =
                            profile.get("typical_structure").map(|v| v.to_string());

                        // 仅当不存在时才插入，避免覆盖用户自定义修改
                        match repo.get_by_name(genre_name) {
                            Ok(None) => {
                                let created = repo.create(
                                    genre_name,
                                    canonical_name,
                                    aliases_json.as_deref(),
                                    core_tone,
                                    pacing_strategy,
                                    anti_patterns_json.as_deref(),
                                    reference_tables_json,
                                    typical_structure_json.as_deref(),
                                );
                                // v0.17.0: 回填读者主情绪承诺
                                if let Ok(profile) = created {
                                    if let Some(promise) =
                                        crate::creative_engine::reader_promise::reader_promise_for(
                                            canonical_name,
                                        )
                                    {
                                        let _ = repo.set_reader_promise(&profile.id, Some(promise));
                                    }
                                }
                            }
                            Ok(Some(existing)) => {
                                // 对已有内置体裁，仅当 typical_structure_json 缺失时回填，
                                // 保证新字段能落地而不覆盖用户已有修改
                                if existing.typical_structure_json.is_none()
                                    || existing.typical_structure_json.as_deref() == Some("")
                                {
                                    let _ = repo.update(
                                        &existing.id,
                                        genre_name,
                                        canonical_name,
                                        aliases_json.as_deref(),
                                        core_tone,
                                        pacing_strategy,
                                        anti_patterns_json.as_deref(),
                                        reference_tables_json,
                                        typical_structure_json.as_deref(),
                                    );
                                }
                                // v0.17.0: 若 reader_promise 缺失则回填（不覆盖用户已设置的值）
                                if existing.reader_promise.is_none()
                                    || existing.reader_promise.as_deref() == Some("")
                                {
                                    if let Some(promise) =
                                        crate::creative_engine::reader_promise::reader_promise_for(
                                            canonical_name,
                                        )
                                    {
                                        let _ =
                                            repo.set_reader_promise(&existing.id, Some(promise));
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!(
                                    "[GenreProfiles] Failed to check existing genre '{}' : {}",
                                    genre_name,
                                    e
                                );
                            }
                        }
                    }
                    log::info!(
                        "[GenreProfiles] Seeded {} built-in genre profiles",
                        profiles.len()
                    );
                }
            }
            Err(e) => {
                log::warn!("[GenreProfiles] Failed to parse genres.json: {}", e);
            }
        }
    }
}

/// v0.22.2: 更新内置题材画像的推荐资产字段
/// v0.22.2: 更新内置题材画像的推荐资产字段（Phase F 种子数据）
fn seed_genre_recommendations(pool: &DbPool) {
    let mappings: &[(&str, &str, &str, &str)] = &[
        (
            "末世流",
            r#"["余华","海明威","鲁迅"]"#,
            "hero_journey",
            r#"["emotion_pacing","character_voice"]"#,
        ),
        (
            "科幻",
            r#"["海明威","余华","王小波"]"#,
            "hero_journey",
            r#"["style_enhancer","emotion_pacing"]"#,
        ),
        (
            "修仙",
            r#"["金庸","曹雪芹"]"#,
            "snowflake",
            r#"["style_enhancer","character_voice"]"#,
        ),
        (
            "都市",
            r#"["张爱玲","老舍","余华"]"#,
            "scene_structure",
            r#"["character_voice","emotion_pacing"]"#,
        ),
        (
            "悬疑",
            r#"["鲁迅","海明威"]"#,
            "scene_structure",
            r#"["emotion_pacing"]"#,
        ),
        (
            "历史",
            r#"["金庸","曹雪芹"]"#,
            "snowflake",
            r#"["style_enhancer"]"#,
        ),
    ];
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[GenreProfile] Cannot get connection for seed: {}", e);
            return;
        }
    };
    for (genre_name, styles, method, skills) in mappings {
        if let Err(e) = conn.execute(
            "UPDATE genre_profiles SET recommended_style_dna_ids=?1, recommended_methodology_id=?2, recommended_skill_ids=?3 WHERE genre_name=?4 AND recommended_style_dna_ids IS NULL",
            rusqlite::params![styles, method, skills, genre_name],
        ) {
            log::warn!("[GenreProfile] Failed to seed {}: {}", genre_name, e);
        }
    }
    log::info!(
        "[GenreProfile] Seeded recommendations for {} genres",
        mappings.len()
    );
}

/// 初始化任务系统和自动化服务
fn init_task_system_and_automation(
    app: &mut tauri::App,
    pool: &DbPool,
    app_handle: &tauri::AppHandle,
) {
    let task_service = task_system::service::TaskService::new(pool.clone(), app_handle.clone());
    let llm_service = llm::LlmService::new(app_handle.clone());
    let vector_store = app_handle
        .state::<std::sync::Arc<dyn ports::VectorStore>>()
        .inner()
        .clone();
    let executor = std::sync::Arc::new(
        book_deconstruction::executor::BookDeconstructionExecutor::new(
            pool.clone(),
            llm_service,
            app_handle.clone(),
            vector_store.clone(),
        ),
    );
    task_service.register_executor(executor);
    let cascade_executor = std::sync::Arc::new(
        creative_engine::cascade_rewriter::executor::CascadeRewriteExecutor::new(
            pool.clone(),
            app_handle.clone(),
        ),
    );
    task_service.register_executor(cascade_executor);
    let ai_gen_executor = std::sync::Arc::new(agents::executor::AiGenerationExecutor::new(
        pool.clone(),
        app_handle.clone(),
    ));
    task_service.register_executor(ai_gen_executor);
    let pipeline_executor = std::sync::Arc::new(pipeline::executor::PipelineReviewExecutor::new(
        pool.clone(),
        app_handle.clone(),
        vector_store.clone(),
    ));
    task_service.register_executor(pipeline_executor);
    if let Err(e) = task_service.bootstrap() {
        log::error!("Failed to bootstrap task system: {}", e);
    } else {
        log::info!("Task system bootstrapped successfully");
    }
    app.manage(task_service);

    // Initialize automation service
    let automation_service =
        automation::service::AutomationService::new(app_handle.clone(), pool.clone());
    let automation_service_clone = automation_service.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone.initialize().await {
            log::error!("Failed to initialize automation service: {}", e);
        } else {
            log::info!("Automation service initialized successfully");
        }
    });
    app.manage(automation_service);
}

/// 异步初始化 LanceDB 向量存储并处理积压索引
async fn init_vector_store_async(
    _vector_db_path: String,
    vector_store: std::sync::Arc<dyn ports::VectorStore>,
    pool: DbPool,
    pending_queue: PendingVectorIndexQueue,
) {
    if let Err(e) = vector_store.init().await {
        log::error!("Failed to initialize vector store: {}", e);
        return;
    }
    log::info!("Vector store initialized successfully");

    // 处理启动期间积压的章节索引请求
    let pending_ids = pending_queue.take();
    if !pending_ids.is_empty() {
        log::info!(
            "[PENDING_VECTOR] Processing {} queued chapter indexes",
            pending_ids.len()
        );
        for chapter_id in pending_ids {
            let repo = db::ChapterRepository::new(pool.clone());
            if let Ok(Some(chapter)) = repo.get_by_id(&chapter_id) {
                let story_id = chapter.story_id.clone();
                let content_text = chapter.content.clone().unwrap_or_default();
                if content_text.len() >= 20 {
                    match embeddings::embed_text_async(content_text.clone()).await {
                        Ok(embedding) => {
                            let record = vector::VectorRecord {
                                id: format!("chapter:{}", chapter_id),
                                story_id: story_id.clone(),
                                chapter_id: chapter_id.clone(),
                                chapter_number: chapter.chapter_number,
                                text: content_text.clone(),
                                record_type: "chapter".to_string(),
                                metadata: None,
                                embedding,
                            };
                            match vector_store.upsert(record).await {
                                Ok(_) => log::info!(
                                    "[PENDING_VECTOR] Indexed queued chapter {}",
                                    chapter_id
                                ),
                                Err(e) => log::warn!(
                                    "[PENDING_VECTOR] Failed to index queued chapter {}: {}",
                                    chapter_id,
                                    e
                                ),
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "[PENDING_VECTOR] Failed to generate embedding for queued chapter {}: {}",
                                chapter_id,
                                e
                            );
                        }
                    }
                }
            }
        }
    }
    if let Ok(conn) = pool.get() {
        let _ = conn.execute("DELETE FROM pending_vector_indexes", []);
    }
    pending_queue.remove_path_file();
}

/// 初始化工作流引擎、调度器和 DSL 加载器
fn init_workflow_engine(app: &mut tauri::App, app_handle: tauri::AppHandle, pool: Option<&DbPool>) {
    let (engine, restored_instance_ids) = if let Some(pool) = pool {
        workflow::WorkflowEngine::with_pool(pool.clone())
    } else {
        (workflow::WorkflowEngine::new(), vec![])
    };
    let scheduler = std::sync::Arc::new(workflow::WorkflowScheduler::new());
    // Register the standard writing workflow template
    if let Err(e) = engine.register_workflow(workflow::templates::standard_writing_workflow()) {
        log::warn!("Failed to register standard workflow: {}", e);
    }
    let engine_arc = std::sync::Arc::new(engine);
    scheduler.start_auto_drain(engine_arc.clone(), app_handle.clone());
    // v0.11.6-hotfix2: 不再在启动时自动恢复并执行之前未完成的 workflow 实例。
    // 恢复执行会在用户未输入任何指令时就进入后台 LLM 流程，导致输入框被禁用。
    if !restored_instance_ids.is_empty() {
        log::info!(
            "[WorkflowEngine] Found {} pending/running instances from previous session; \
             skipping auto-restore to avoid blocking UI on startup",
            restored_instance_ids.len()
        );
    }

    app.manage(engine_arc.clone());
    app.manage(scheduler);
    log::info!("Workflow engine and scheduler initialized");

    // Initialize WorkflowLoader (file system DSL watcher)
    let loader = workflow::WorkflowLoader::new(engine_arc);
    let builtin_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("workflows")));
    let user_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
        .join("workflows");
    if let Err(e) = loader.initialize(builtin_dir, user_dir) {
        log::warn!("[WorkflowLoader] Failed to initialize: {}", e);
    }
    app.manage(loader);
    log::info!("[WorkflowLoader] File system workflow watcher initialized");
}

/// 初始化窗口状态：隐藏 backstage，聚焦 frontstage，禁用 Windows 右键菜单
fn init_windows(app: &mut tauri::App) {
    log::info!("[StateSync] State synchronization service initialized");

    if let Some(backstage) = app.get_webview_window("backstage") {
        let _ = backstage.hide();
    }
    if let Some(frontstage) = app.get_webview_window("frontstage") {
        let _ = frontstage.set_focus();
    }

    #[cfg(target_os = "windows")]
    {
        for label in ["frontstage", "backstage"] {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.with_webview(|webview| {
                    let controller = webview.controller();
                    unsafe {
                        if let Ok(core) = controller.CoreWebView2() {
                            if let Ok(settings) = core.Settings() {
                                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                            }
                        }
                    }
                });
            }
        }
    }
}

/// 启动后台任务：能力进化自动触发
///
/// v0.11.5-hotfix: 默认禁用启动时自动触发能力进化。早期实现会在启动 30s 后
/// 无条件调用 LLM 分析所有能力执行记录，若模型未配置或响应慢，会让应用在
/// 用户未输入任何指令的情况下卡住 500s 以上。
fn spawn_background_tasks(_app_handle: tauri::AppHandle) {
    log::info!("[BackgroundTasks] Auto capability evolution disabled by default");
}

pub fn run() {
    let _app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                match window.label() {
                    "backstage" => {
                        // 关闭幕后窗口时，只隐藏它，不退出应用，也不影响幕前窗口
                        log::info!("Backstage close requested, hiding instead of exiting");
                        let _ = window.hide();
                    }
                    "frontstage" => {
                        // 优雅关闭: 检查数据库、保存向量索引、停止自动化服务
                        graceful_shutdown(&window.app_handle());
                    }
                    _ => {
                        // 其他窗口默认退出
                        log::info!(
                            "Window {} close requested, exiting application",
                            window.label()
                        );
                        graceful_shutdown(&window.app_handle());
                    }
                }
            }
        })
        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
            if let Err(e) = std::fs::create_dir_all(&app_dir) {
                log::error!(
                    "Failed to create app data directory {}: {}",
                    app_dir.display(),
                    e
                );
            }

            // 初始化结构化日志系统（必须在其他操作之前）
            let _log_guard = logging::init_logger(&app_dir);

            log::info!("App directory: {:?}", app_dir);

            let bundled_migrations = app
                .path()
                .resolve("db/migrations", BaseDirectory::Resource)
                .ok()
                .filter(|p| p.is_dir());
            match &bundled_migrations {
                Some(dir) => log::info!("[Setup] Bundled SQL migrations: {}", dir.display()),
                None => log::warn!(
                    "[Setup] Bundled SQL migrations not found; using inline migrations only"
                ),
            }

            // 设置 prompts 资源目录，供 PromptRegistry 在运行时读取内置提示词
            if let Some(prompts_dir) = app
                .path()
                .resolve("prompts", BaseDirectory::Resource)
                .ok()
                .filter(|p| p.is_dir())
            {
                log::info!("[Setup] Bundled prompts directory: {}", prompts_dir.display());
                crate::prompts::registry::set_prompts_resource_dir(prompts_dir);
            } else {
                log::warn!("[Setup] Bundled prompts directory not found; using fallback paths");
            }

            // 设置 panic hook 以便记录崩溃信息，辅助诊断窗口最大化等异常退出
            std::panic::set_hook(Box::new(|info| {
                let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                    *s
                } else if let Some(s) = info.payload().downcast_ref::<String>() {
                    s.as_str()
                } else {
                    "unknown panic"
                };
                let location = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_else(|| "unknown location".to_string());
                log::error!("APPLICATION PANIC: {} at {}", payload, location);
                eprintln!("APPLICATION PANIC: {} at {}", payload, location);
            }));

            // 初始化数据库（必须在加载 pending vector indexes 之前）
            let pool = match init_db(&app_dir, bundled_migrations.as_deref()) {
                Ok(pool) => {
                    log::info!("Database initialized successfully");
                    app.manage(pool.clone());
                    Some(pool)
                }
                Err(e) => {
                    log::error!(
                        "Failed to initialize database at {}: {}",
                        app_dir.join("cinema_ai.db").display(),
                        e
                    );
                    None
                }
            };

            // P2-19 修复: 初始化 pending vector indexes 队列，加载上次未处理的项并注入 State
            let pending_queue = PendingVectorIndexQueue::new(app_dir.join("pending_vector_indexes.json"));
            if let Some(ref pool) = pool {
                pending_queue.load_from_pool(pool);
                let loaded_count = pending_queue.queue.lock().map(|q| q.len()).unwrap_or(0);
                if loaded_count > 0 {
                    log::info!(
                        "Loaded {} pending vector indexes from previous session",
                        loaded_count
                    );
                }
            }
            app.manage(pending_queue.clone());

            // 初始化 LanceDB 向量存储并尽早注入 State，后续 task_system /
            // model_gateway 等组件会通过 app_handle.state() 获取它。
            let vector_db_path = app_dir.join("vector_db").to_string_lossy().to_string();
            std::fs::create_dir_all(&vector_db_path).ok();
            let vector_store: std::sync::Arc<dyn ports::VectorStore> =
                std::sync::Arc::new(vector::LanceVectorStore::new(vector_db_path.clone()));
            app.manage(vector_store.clone());

            // 初始化共享 LLM 服务并通过 Tauri State 注入，确保后续所有
            // LlmService::new(app_handle) 复用同一连接池与缓存。
            let llm_service = crate::llm::LlmService::new(app.handle().clone());
            app.manage(llm_service);

            // v0.23.8: 注入诊断数据存储，供前端超时/失败时获取最后 LLM 提示词全文
            app.manage(std::sync::Arc::new(crate::diagnostics::DiagnosticStore::new()));

            // v0.26.0: 注入生成链路 Trace 存储
            match crate::tracing::TraceStore::new(&app.handle().clone()) {
                Ok(trace_store) => {
                    app.manage(trace_store);
                    log::info!("[TraceStore] initialized");
                }
                Err(e) => {
                    log::warn!("[TraceStore] failed to initialize: {}", e);
                }
            }

            // v0.23.12: 注入智能创作流程详细日志记录器
            match crate::workflow_logger::WorkflowLogger::new(&app_dir) {
                Ok(logger) => {
                    app.manage(std::sync::Arc::new(logger));
                    log::info!("[WorkflowLogger] initialized at {}", app_dir.join("logs").join("creative_workflow.log").display());
                }
                Err(e) => {
                    log::warn!("[WorkflowLogger] failed to initialize: {}", e);
                }
            }

            // 初始化共享 SkillManager 并通过 Tauri State 注入。
            let skill_manager = SkillManager::new(
                Some(crate::llm::LlmService::new(app.handle().clone())),
                pool.clone(),
            );
            app.manage(skill_manager.clone());

            // 初始化共享 ChapterCommitDebouncer 并通过 Tauri State 注入。
            let debouncer = crate::story_system::ChapterCommitDebouncer::new();
            app.manage(debouncer);

            // 设置能力进化描述持久化路径
            let evolved_desc_path = app_dir.join("evolved_descriptions.json");
            crate::capabilities::set_evolved_descriptions_path(evolved_desc_path);

            if let Some(ref pool) = pool {
                seed_builtin_data(pool, &app_dir);

                // 注册可发现创作资产到全局 CapabilityRegistry
                let genre_repo = db::GenreProfileRepository::new(pool.clone());
                // v0.22.2: 种子题材推荐资产映射（Phase F）
                seed_genre_recommendations(&pool);
                let skills = skill_manager.get_all_skills();
                match strategy::load_all_assets(&genre_repo, &skills) {
                    Ok(assets) => {
                        let mut registry = capabilities::get_capability_registry();
                        registry.register_selectable_assets(&assets);
                        log::info!(
                            "[CapabilityRegistry] Registered {} selectable creative assets",
                            assets.len()
                        );

                        // v0.23.9: 生成运行时创作资产能力清单并注入 Tauri State，
                        // 供 PromptSynthesizer / ModelGateway 在每次启动后使用最新资产目录。
                        match crate::creative_engine::asset_capability_manifest::AssetCapabilityManifest::build_from(
                            &genre_repo, &skills,
                        ) {
                            Ok(manifest) => {
                                log::info!(
                                    "[AssetCapabilityManifest] Built index with {} assets (summary rendered on demand)",
                                    manifest.assets.len()
                                );
                                app.manage(std::sync::Arc::new(manifest));
                            }
                            Err(e) => {
                                log::warn!("[AssetCapabilityManifest] Failed to build manifest: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[CapabilityRegistry] Failed to load creative assets: {}", e);
                    }
                }
            }

            // 将内置 MCP 工具（filesystem/text_processing/web_search）自动注册到
            // CapabilityRegistry，接通审计报告 P0-3：此前内置 MCP 工具只注册了
            // handler（BUILTIN_MCP_SERVER），未注册进 CapabilityRegistry，
            // 导致 PlanGenerator 输出的 mcp.builtin.* 步骤被验证器丢弃。
            // setup 为同步上下文，使用 try_lock（Lazy 初始化已同步完成注册）。
            if let Ok(builtin_server) = BUILTIN_MCP_SERVER.try_lock() {
                let tools = builtin_server.get_tools();
                if !tools.is_empty() {
                    let mut registry = capabilities::get_capability_registry();
                    let mut count = 0;
                    for tool in &tools {
                        let cap = capabilities::Capability::from_mcp_tool("builtin", tool);
                        registry.register(cap);
                        count += 1;
                    }
                    log::info!(
                        "[CapabilityRegistry] Registered {} built-in MCP tools",
                        count
                    );
                }
            } else {
                log::warn!("[CapabilityRegistry] BUILTIN_MCP_SERVER busy, skip auto-register");
            }

            // v0.20.1: 初始化 SING 意图图——将 CapabilityRegistry 和
            // SelectableAsset 同步到意图-资产异构图。
            // 修复审计报告 P0-1：此前 AssetSyncEngine 从未被调用，导致 6 张意图图表
            // 永远为空，discover_server_level 返回空，generate_plan 静默回退。
            if let Some(ref pool) = pool {
                let ig_repo = intention_graph::graph::IntentionGraphRepository::new(
                    pool.clone(),
                );
                let sync_engine =
                    intention_graph::asset_sync::AssetSyncEngine::new(ig_repo.clone());

                // 收集已注册的 SelectableAsset（与上面 CapabilityRegistry 同源）
                let genre_repo = db::GenreProfileRepository::new(pool.clone());
                let skills = skill_manager.get_all_skills();
                let selectable_assets =
                    strategy::load_all_assets(&genre_repo, &skills).unwrap_or_default();

                match sync_engine.full_initialize(
                    &capabilities::get_capability_registry(),
                    &selectable_assets,
                ) {
                    Ok(stats) => {
                        log::info!(
                            "[IntentionGraph] 初始化完成: {} 能力, {} 可选资产, {} Agent, {} 系统命令, {} 资产边",
                            stats.capabilities,
                            stats.selectable_assets,
                            stats.agents,
                            stats.system_commands,
                            stats.asset_edges
                        );
                    }
                    Err(e) => {
                        log::warn!("[IntentionGraph] 资产同步失败（意图图路径将降级到 PlanGenerator）: {}", e);
                    }
                }

                // 预热内存缓存（加载所有意图/资产/边到内存加速查询）
                if let Err(e) = ig_repo.warm_up_cache() {
                    log::warn!("[IntentionGraph] warm_up_cache 失败: {}", e);
                }

                // 注册为 Tauri state，供 IntentionGraphPlanner::from_app_handle 复用
                // （复用同一 cache 实例，避免每次请求重建图缓存）
                app.manage(ig_repo);
            }

            // Initialize embedding model
            let _ = embeddings::init_embedding_model();
            let app_config = match config::AppConfig::load(&app_dir) {
                Ok(config) => {
                    embeddings::provider::init_global_provider(&config);
                    config
                }
                Err(e) => {
                    log::warn!("[Setup] 加载配置失败，使用默认嵌入: {}", e);
                    embeddings::provider::init_global_provider(&config::AppConfig::default());
                    config::AppConfig::default()
                }
            };

            // v0.14.0: 初始化模型网关执行器与健康探测调度器
            // Issue #4: init_db 失败时 pool 未 manage，必须跳过 GatewayExecutor，
            // 否则 state::<DbPool>() 会在启动时 panic（Windows 闪退）。
            if let Some(ref db_pool) = pool {
                let llm_service = app
                    .state::<crate::llm::service::LlmService>()
                    .inner()
                    .clone();
                let registry = crate::router::UnifiedModelRegistry::from_app_config(&app_config);
                let gateway_registry =
                    crate::model_gateway::registry::GatewayRegistry::new(registry);
                let gateway_executor = crate::model_gateway::executor::GatewayExecutor::new(
                    app.handle().clone(),
                    gateway_registry,
                    llm_service,
                    db_pool.clone(),
                );
                app.manage(gateway_executor.clone());
                crate::model_gateway::scheduler::spawn_health_probe_scheduler(
                    app.handle().clone(),
                    gateway_executor,
                );
                log::info!("[ModelGateway] 网关执行器与健康探测调度器已初始化");
            } else {
                log::warn!(
                    "[ModelGateway] No DB pool available, skipping gateway executor init \
                     (app will start in degraded mode — check logs for init_db error)"
                );
            }

            // v0.23.14: 启动归零 —— 清空 llm_calls 历史表，避免死模型污染健康报告。
            // 健康报告数据源已切换为 HealthRegistry 实时探测快照，不再依赖 llm_calls。
            if let Some(ref pool) = pool {
                let repo = crate::db::repositories_pipeline::LlmCallRepository::new(
                    pool.clone(),
                );
                match repo.delete_all() {
                    Ok(n) => {
                        log::info!("[Setup] 启动归零：清除 llm_calls {} 条历史记录", n)
                    }
                    Err(e) => log::warn!("[Setup] 启动归零失败: {}", e),
                }
                // 清除 HealthRegistry 中不在当前 config 的残留条目（防御性）
                if let Some(executor) =
                    app.try_state::<crate::model_gateway::executor::GatewayExecutor>()
                {
                    let valid_ids: Vec<String> = app_config
                        .llm_profiles
                        .keys()
                        .cloned()
                        .chain(app_config.embedding_profiles.keys().cloned())
                        .collect();
                    if let Ok(mut health) = executor.health_registry().lock() {
                        health.retain(&valid_ids);
                    }
                }
            }

            if let Some(ref pool) = pool {
                let app_handle = app.handle().clone();
                init_task_system_and_automation(app, pool, &app_handle);
            }

            // 异步初始化 LanceDB 向量存储（State 已在上方提前注入）
            if let Some(ref pool) = pool {
                let pool_clone = pool.clone();
                tauri::async_runtime::spawn(init_vector_store_async(
                    vector_db_path,
                    vector_store,
                    pool_clone,
                    pending_queue.clone(),
                ));
            } else {
                log::warn!("[Setup] No DB pool available, skipping vector store init");
            }

            // Initialize the neutral creative-engine port used by agents.
            if let Some(ref pool) = pool {
                let creative_engine: std::sync::Arc<dyn crate::domain::creative_engine::CreativeEnginePort> =
                    std::sync::Arc::new(crate::creative_engine::adapter::CreativeEngineAdapter::new(
                        pool.clone(),
                    ));
                app.manage(creative_engine);
                log::info!("[Setup] CreativeEnginePort 已注册为 Tauri managed state");
            }

            // NOTE: WebSocket server for collaborative editing is reserved for future use
            // (Phase 4) See docs/plans/ for collaboration feature roadmap.
            // if let Some(pool) = get_pool() {
            //     tauri::async_runtime::spawn(async move {
            //         let ports = [8765, 8766, 8767, 8768, 8769];
            //         for port in ports {
            //             let ws_server = WebSocketServer::with_pool(pool.clone());
            //             match ws_server.start(port).await {
            //                 Ok(_) => {
            //                     log::info!("WebSocket server started on port {}", port);
            //                     break;
            //                 }
            //                 Err(e) => {
            //                     log::warn!("Failed to start WebSocket server on port {}:
            // {}", port, e);                 }
            //             }
            //         }
            //     });
            // }

            init_workflow_engine(app, app.handle().clone(), pool.as_ref());

            init_windows(app);
            spawn_background_tasks(app.handle().clone());

            // v0.8.0: 启动记忆健康守护进程
            if let Some(ref pool) = pool {
                crate::memory::health_daemon::spawn_daemon(pool.clone(), app.handle().clone());
            }

            Ok(())
        })
        .invoke_handler(include!("handlers.rs"))
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessageItem {
    pub role: String,
    pub content: String,
}

use tokio::sync::Mutex as TokioMutex;

// GLOBAL: MCP 客户端连接池。
// SAFETY: TokioMutex 用于异步上下文。连接在运行时动态增删。
pub(crate) static MCP_CONNECTIONS: Lazy<TokioMutex<HashMap<String, mcp::McpClient>>> =
    Lazy::new(|| TokioMutex::new(HashMap::new()));

// GLOBAL: 内置 MCP Server 实例（W2-B8）。
// SAFETY: Lazy 初始化一次。运行时通过 TokioMutex 支持动态注册/注销工具。
pub(crate) static BUILTIN_MCP_SERVER: Lazy<TokioMutex<mcp::McpServer>> = Lazy::new(|| {
    let config = mcp::McpServerConfig {
        id: "builtin".to_string(),
        name: "Built-in Tools".to_string(),
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        timeout_seconds: 30,
    };
    TokioMutex::new(mcp::McpServer::new(config))
});

/// 向量存储初始化前积压的章节索引队列（P0-7 / P2-19）。
///
/// 启动时从 SQLite/JSON 恢复，初始化完成后消费并清空。
/// 丢失仅导致章节未建立向量索引，无数据损坏。
#[derive(Clone)]
struct PendingVectorIndexQueue {
    queue: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    path: std::path::PathBuf,
}

impl PendingVectorIndexQueue {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            queue: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            path,
        }
    }

    /// 优先从 Tauri State 获取共享实例；不存在则返回 None。
    fn from_app_handle(app_handle: &tauri::AppHandle) -> Option<Self> {
        app_handle.try_state::<Self>().map(|s| s.inner().clone())
    }

    /// 从 SQLite（优先）和旧 JSON 文件（迁移回退）加载积压项到队列。
    fn load_from_pool(&self, pool: &DbPool) {
        let mut result = Vec::new();

        if let Ok(conn) = pool.get() {
            if let Ok(mut stmt) =
                conn.prepare("SELECT chapter_id FROM pending_vector_indexes ORDER BY created_at")
            {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let id: Option<String> = row.get(0)?;
                    Ok(id)
                }) {
                    for row in rows {
                        if let Ok(Some(id)) = row {
                            result.push(id);
                        }
                    }
                }
            }
        }

        if result.is_empty() {
            if let Ok(content) = std::fs::read_to_string(&self.path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<String>>(&content) {
                    result = loaded;
                }
            }
        }

        if let Ok(mut q) = self.queue.lock() {
            *q = result;
        }
    }

    /// 取出并清空当前队列中的所有积压项。
    fn take(&self) -> Vec<String> {
        if let Ok(mut q) = self.queue.lock() {
            std::mem::take(&mut *q)
        } else {
            Vec::new()
        }
    }

    /// 将当前队列持久化到 SQLite，并删除旧 JSON 文件。
    fn save_to_db(&self, pool: &DbPool) {
        if let Ok(pending) = self.queue.lock() {
            let conn = pool.get();
            if let Ok(conn) = conn {
                let _ = conn.execute("DELETE FROM pending_vector_indexes", []);
                for chapter_id in pending.iter() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO pending_vector_indexes (chapter_id, created_at) \
                         VALUES (?1, ?2)",
                        rusqlite::params![chapter_id, now],
                    );
                }
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }

    /// 删除旧 JSON 持久化文件（向量索引消费完成后调用）。
    fn remove_path_file(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 检测用户输入是否包含"创建新小说"的意图
pub(crate) fn is_novel_creation_intent(user_input: &str) -> bool {
    let input = user_input.to_lowercase();
    let creation_signals = [
        "写一部",
        "写一本",
        "写一篇",
        "写个",
        "创作一部",
        "创作一本",
        "创作一篇",
        "创作个",
        "生成一部",
        "生成一本",
        "生成一篇",
        "新建",
        "创建",
        "新开",
        "write a",
        "write an",
        "create a",
        "create an",
        "start a",
        "start an",
        "novel",
        "story",
        "book",
    ];
    let has_creation_signal = creation_signals.iter().any(|&kw| input.contains(kw));
    if !has_creation_signal {
        return false;
    }
    // 排除明确的续写意图词
    let continuation_signals = ["续写", "接着写", "往下写", "后面", "接下来", "继续", "后续"];
    let has_continuation_signal = continuation_signals.iter().any(|&kw| input.contains(kw));
    // 如果同时包含创建信号和续写信号，优先判断为续写
    if has_continuation_signal {
        return false;
    }
    true
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct RecordFeedbackRequest {
    story_id: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    feedback_type: String,
    agent_type: Option<String>,
    original_ai_text: String,
    final_text: Option<String>,
    /// 最后发给模型的完整提示词（诊断用）
    original_prompt: Option<String>,
    /// AI 生成的原始文本（通常与 original_ai_text
    /// 相同，保留独立字段方便后续扩展）
    generated_content: Option<String>,
    /// 用户在 original_ai_text 基础上的后续编辑差异（modify 场景由前端提供）
    subsequent_edit_diff: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct LearningPoint {
    category: String,
    observation: String,
    impact: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ExportOptions {
    story_id: String,
    format: String,
    include_metadata: Option<bool>,
    include_outline: Option<bool>,
    include_characters: Option<bool>,
    template_id: Option<String>,
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn test_is_novel_creation_intent_chinese_creation() {
        assert!(is_novel_creation_intent("我想写一部武侠小说"));
        assert!(is_novel_creation_intent("帮我创建一本新书"));
        assert!(is_novel_creation_intent("生成一篇科幻小说"));
        assert!(is_novel_creation_intent("新建一个故事"));
    }

    #[test]
    fn test_is_novel_creation_intent_english_creation() {
        assert!(is_novel_creation_intent("I want to write a novel"));
        assert!(is_novel_creation_intent("create a story"));
        assert!(is_novel_creation_intent("start a book"));
    }

    #[test]
    fn test_is_novel_creation_intent_continuation() {
        // 包含续写关键词，应返回 false
        assert!(!is_novel_creation_intent("帮我续写这部小说"));
        assert!(!is_novel_creation_intent("接着写后面的内容"));
        assert!(!is_novel_creation_intent("继续往下写"));
    }

    #[test]
    fn test_is_novel_creation_intent_mixed_signals() {
        // 同时包含创建和续写信号，优先判断为续写
        assert!(!is_novel_creation_intent(
            "创建一本新书，然后续写后面的章节"
        ));
        assert!(!is_novel_creation_intent("写一部小说，接着写后续"));
    }

    #[test]
    fn test_is_novel_creation_intent_no_signal() {
        assert!(!is_novel_creation_intent("今天天气不错"));
        assert!(!is_novel_creation_intent("帮我修改这段文字"));
        assert!(!is_novel_creation_intent("保存文件"));
    }

    #[test]
    fn test_is_novel_creation_intent_case_insensitive() {
        assert!(is_novel_creation_intent("WRITE A NOVEL"));
        assert!(is_novel_creation_intent("Create A Story"));
    }
}
