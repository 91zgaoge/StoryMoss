#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
pub(crate) mod embeddings;
mod error;
mod evolution;
mod export;
mod intent;
mod knowledge_base;
mod llm;
mod logging;
mod mcp;
mod memory;
mod narrative;
mod pipeline;
mod planner;
mod prompts;
mod reading_power;
mod revision_commands;
mod router;
mod scene_commands;
mod skills;
mod state; // RESERVED: runtime story state manager (Phase 4)
mod state_sync;
mod story_system;
mod studio_commands;
mod subscription;
mod task_system;
mod telemetry;
mod updater;
mod utils;
mod vector;
mod versions;
mod window;
mod workflow;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;
#[macro_use]
mod commands;

use std::{collections::HashMap, sync::Mutex, time::Instant};

use config::AppConfig;
use db::{init_db, DbPool};
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;
use skills::SkillManager;
use tauri::{Emitter, Manager};

// NOTE: Collab WebSocket server is reserved for future use (Phase 4)
// use collab::websocket::WebSocketServer;

// GLOBAL: DB_POOL — 数据库连接池全局访问点。
// SAFETY: 仅在 setup() 中初始化一次，之后只读访问。Mutex 用于 Lazy
// 初始化，不是频繁锁竞争。 NOTE: 理想情况下应通过 Tauri State
// 注入，但当前大量模块直接调用 get_pool()，保留为过渡期全局。
static DB_POOL: Lazy<Mutex<Option<DbPool>>> = Lazy::new(|| Mutex::new(None));

// GLOBAL: APP_CONFIG — 应用配置全局访问点。
// SAFETY: 在 setup() 中加载后极少变更。reload_config() 会写，但频率极低。
// NOTE: 应逐步迁移到按模块配置注入。
static APP_CONFIG: Lazy<Mutex<Option<AppConfig>>> = Lazy::new(|| Mutex::new(None));

// GLOBAL: SKILL_MANAGER — 技能管理器全局单例。
// SAFETY: OnceCell 保证仅初始化一次。SkillManager 内部使用 Mutex 保护状态。
pub static SKILL_MANAGER: OnceCell<Mutex<SkillManager>> = OnceCell::new();

// GLOBAL: Chapter commit debounce 状态。
// W4-B9: 防止频繁保存导致重复 commit。
// SAFETY: 纯内存状态，丢失无数据损坏风险。每次启动重置。
pub(crate) static CHAPTER_COMMIT_DEBOUNCE: Lazy<Mutex<HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
pub(crate) const CHAPTER_COMMIT_DEBOUNCE_SECONDS: u64 = 30; // 30 秒 debounce

/// 记录 AI 操作历史
pub(crate) fn record_ai_operation(req: db::CreateAiOperationRequest) {
    if let Some(pool) = get_pool() {
        let repo = db::AiOperationRepository::new(pool);
        if let Err(e) = repo.create(req) {
            log::warn!("[AiOperation] Failed to record operation: {}", e);
        }
    }
}

pub(crate) fn get_pool() -> Option<DbPool> {
    DB_POOL.lock().unwrap().clone()
}
/// 优雅关闭：WAL checkpoint、保存向量索引、然后退出
fn graceful_shutdown(app_handle: &tauri::AppHandle) {
    log::info!("[Shutdown] Starting graceful shutdown...");

    // 1. SQLite WAL checkpoint — 确保所有数据已写入主数据库
    if let Some(pool) = get_pool() {
        if let Ok(conn) = pool.get() {
            match conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)") {
                Ok(_) => log::info!("[Shutdown] WAL checkpoint completed"),
                Err(e) => log::warn!("[Shutdown] WAL checkpoint failed: {}", e),
            }
        } else {
            log::warn!("[Shutdown] Failed to get DB connection for checkpoint");
        }
    } else {
        log::warn!("[Shutdown] No DB pool available for checkpoint");
    }

    // 2. 保存待处理的向量索引
    save_pending_vector_indexes();
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
                        // 仅当不存在时才插入，避免覆盖用户自定义修改
                        match repo.get_by_name(genre_name) {
                            Ok(None) => {
                                let aliases_json = profile.get("aliases").map(|v| v.to_string());
                                let core_tone = profile.get("core_tone").and_then(|v| v.as_str());
                                let pacing_strategy =
                                    profile.get("pacing_strategy").and_then(|v| v.as_str());
                                let anti_patterns_json =
                                    profile.get("anti_patterns").map(|v| v.to_string());
                                let reference_tables_json =
                                    profile.get("reference_tables").and_then(|v| v.as_str());
                                let _ = repo.create(
                                    genre_name,
                                    canonical_name,
                                    aliases_json.as_deref(),
                                    core_tone,
                                    pacing_strategy,
                                    anti_patterns_json.as_deref(),
                                    reference_tables_json,
                                );
                            }
                            Ok(Some(_)) => {
                                // 已存在，跳过
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

/// 初始化任务系统和自动化服务
fn init_task_system_and_automation(
    app: &mut tauri::App,
    pool: &DbPool,
    app_handle: &tauri::AppHandle,
) {
    let task_service = task_system::service::TaskService::new(pool.clone(), app_handle.clone());
    let llm_service = llm::LlmService::new(app_handle.clone());
    let executor = std::sync::Arc::new(
        book_deconstruction::executor::BookDeconstructionExecutor::new(
            pool.clone(),
            llm_service,
            app_handle.clone(),
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
async fn init_vector_store_async(vector_db_path: String) {
    let mut vector_store = LanceVectorStore::new(vector_db_path);
    if let Err(e) = vector_store.init().await {
        log::error!("Failed to initialize vector store: {}", e);
        return;
    }
    let _ = VECTOR_STORE.set(vector_store);
    log::info!("Vector store initialized successfully");

    // 处理启动期间积压的章节索引请求
    let pending_ids: Vec<String> = {
        if let Ok(mut pending) = PENDING_VECTOR_INDEXES.lock() {
            std::mem::take(&mut *pending)
        } else {
            Vec::new()
        }
    };
    if !pending_ids.is_empty() {
        log::info!(
            "[PENDING_VECTOR] Processing {} queued chapter indexes",
            pending_ids.len()
        );
        for chapter_id in pending_ids {
            if let Some(pool) = get_pool() {
                let repo = db::ChapterRepository::new(pool);
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
                                if let Some(store) = VECTOR_STORE.get() {
                                    match store.add_record(record).await {
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
    }
    if let Some(pool) = get_pool() {
        if let Ok(conn) = pool.get() {
            let _ = conn.execute("DELETE FROM pending_vector_indexes", []);
        }
    }
    if let Some(path) = PENDING_VECTOR_INDEXES_PATH.get() {
        let _ = std::fs::remove_file(path);
    }
}

/// 初始化工作流引擎、调度器和 DSL 加载器
fn init_workflow_engine(app: &mut tauri::App, app_handle: tauri::AppHandle) {
    let (engine, restored_instance_ids) = if let Some(pool) = get_pool() {
        workflow::WorkflowEngine::with_pool(pool)
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
    if !restored_instance_ids.is_empty() {
        log::info!(
            "[WorkflowEngine] Restoring {} pending/running instances to scheduler",
            restored_instance_ids.len()
        );
        let scheduler_clone = scheduler.clone();
        tauri::async_runtime::spawn(async move {
            for instance_id in restored_instance_ids {
                if let Err(e) = scheduler_clone.schedule_execution(instance_id).await {
                    log::warn!(
                        "[WorkflowEngine] Failed to restore instance to scheduler: {}",
                        e
                    );
                }
            }
        });
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
fn spawn_background_tasks(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let llm = llm::LlmService::new(app_handle.clone());
        let engine = capabilities::evolution::CapabilityEvolutionEngine::new(llm, &app_handle);
        let stats = engine.get_statistics();
        let total_records: usize = stats.values().map(|(t, _)| t).sum();
        if total_records >= 5 {
            log::info!(
                "[CapabilityEvolution] Auto-triggering evolution with {} total records",
                total_records
            );
            match engine.evolve_capability_descriptions().await {
                Ok(improvements) => {
                    log::info!(
                        "[CapabilityEvolution] Auto-evolution completed with {} improvements",
                        improvements.len()
                    );
                    let _ = app_handle.emit(
                        "capabilities-evolved",
                        serde_json::json!({
                            "improvements": improvements,
                            "auto_triggered": true,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }
                Err(e) => {
                    log::warn!("[CapabilityEvolution] Auto-evolution failed: {}", e)
                }
            }
        } else {
            log::info!(
                "[CapabilityEvolution] Not enough records ({}) to trigger auto-evolution",
                total_records
            );
        }
    });
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
            std::fs::create_dir_all(&app_dir).ok();

            // 初始化结构化日志系统（必须在其他操作之前）
            let _log_guard = logging::init_logger(&app_dir);

            log::info!("App directory: {:?}", app_dir);

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

            // P2-19 修复: 设置 pending vector indexes 持久化路径，并加载上次未处理的队列
            let pending_path = app_dir.join("pending_vector_indexes.json");
            let _ = PENDING_VECTOR_INDEXES_PATH.set(pending_path.clone());
            let loaded_pending = load_pending_vector_indexes();
            if !loaded_pending.is_empty() {
                log::info!(
                    "Loaded {} pending vector indexes from previous session",
                    loaded_pending.len()
                );
                if let Ok(mut pending) = PENDING_VECTOR_INDEXES.lock() {
                    *pending = loaded_pending;
                }
            }

            match init_db(&app_dir) {
                Ok(pool) => {
                    log::info!("Database initialized successfully");
                    app.manage(pool.clone());
                    *DB_POOL.lock().unwrap() = Some(pool);
                }
                Err(e) => {
                    log::error!("Failed to initialize database: {}", e);
                }
            }
            let _ = SKILL_MANAGER.set(Mutex::new(SkillManager::new(Some(
                crate::llm::LlmService::new(app.handle().clone()),
            ))));

            // 设置能力进化描述持久化路径
            let evolved_desc_path = app_dir.join("evolved_descriptions.json");
            crate::capabilities::set_evolved_descriptions_path(evolved_desc_path);

            if let Some(pool) = get_pool() {
                seed_builtin_data(&pool, &app_dir);
            }

            // Initialize embedding model
            let _ = embeddings::init_embedding_model();
            match config::AppConfig::load(&app_dir) {
                Ok(config) => {
                    embeddings::provider::init_global_provider(&config);
                    *APP_CONFIG.lock().unwrap() = Some(config);
                }
                Err(e) => {
                    log::warn!("[Setup] 加载配置失败，使用默认嵌入: {}", e);
                    embeddings::provider::init_global_provider(&config::AppConfig::default());
                }
            }

            if let Some(pool) = get_pool() {
                let app_handle = app.handle().clone();
                init_task_system_and_automation(app, &pool, &app_handle);
            }

            // Initialize LanceDB vector store (async background task)
            let vector_db_path = app_dir.join("vector_db").to_string_lossy().to_string();
            std::fs::create_dir_all(&vector_db_path).ok();
            tauri::async_runtime::spawn(init_vector_store_async(vector_db_path));

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

            init_workflow_engine(app, app.handle().clone());

            init_windows(app);
            spawn_background_tasks(app.handle().clone());

            // v0.8.0: 启动记忆健康守护进程
            if let Some(pool) = get_pool() {
                crate::memory::health_daemon::spawn_daemon(pool, app.handle().clone());
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

// Vector Search Commands (LanceDB)
use vector::LanceVectorStore;

// GLOBAL: LanceDB 向量存储单例。
// SAFETY: OnceCell 保证仅初始化一次。在异步任务中完成 init() 后设置。
// NOTE: 向量存储有持久化文件，重启后数据不丢失，但句柄需要重新打开。
pub(crate) static VECTOR_STORE: OnceCell<LanceVectorStore> = OnceCell::new();

// GLOBAL: 向量存储初始化前积压的章节索引请求（P0-7 修复）。
// SAFETY: 纯内存队列，启动时从 SQLite/JSON 恢复，初始化完成后消费并清空。
// 丢失仅导致章节未建立向量索引，无数据损坏。
static PENDING_VECTOR_INDEXES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

// GLOBAL: pending queue 持久化文件路径（P2-19 修复）。
// SAFETY: OnceCell 在 setup() 中设置一次。仅用于优雅关闭时的后备保存。
static PENDING_VECTOR_INDEXES_PATH: OnceCell<std::path::PathBuf> = OnceCell::new();

fn save_pending_vector_indexes() {
    if let Ok(pending) = PENDING_VECTOR_INDEXES.lock() {
        if let Some(pool) = get_pool() {
            let conn = pool.get();
            if let Ok(conn) = conn {
                // 清空后重新插入
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
    }
}

fn load_pending_vector_indexes() -> Vec<String> {
    let mut result = Vec::new();

    // 优先从 SQLite 加载
    if let Some(pool) = get_pool() {
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
    }

    // Fallback: 从旧 JSON 文件加载（迁移用）
    if result.is_empty() {
        if let Some(path) = PENDING_VECTOR_INDEXES_PATH.get() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<String>>(&content) {
                    result = loaded;
                }
            }
        }
    }

    result
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
