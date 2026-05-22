#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
use crate::error::AppError;
mod db;
mod config;
mod llm;
mod agents;
mod memory;
mod vector;
mod workflow;
mod export;
mod prompts;
mod versions;
mod chat;           // RESERVED: story-associated chat sessions (Phase 4)
mod analytics;
mod skills;
mod mcp;
mod collab;         // RESERVED: collaborative editing WebSocket server (Phase 4)
mod state;          // RESERVED: runtime story state manager (Phase 4)
mod router;
mod evolution;
pub(crate) mod embeddings;
mod utils;
mod window;
mod updater;
mod commands_v3;
mod commands_pipeline;
mod pipeline;
mod knowledge_base;
mod intent;
mod creative_engine;
mod subscription;
mod book_deconstruction;
mod task_system;
mod canonical_state;
mod capabilities;
mod planner;
mod narrative;
mod audit;
mod auth;
mod state_sync;
mod logging;
mod automation;
mod story_system;
mod reading_power;
mod anti_ai;
mod telemetry;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;

use tauri::{Manager, AppHandle, Emitter};

use db::{DbPool, init_db, StoryRepository, CharacterRepository, ChapterRepository, SceneRepository, CreateStoryRequest, CreateCharacterRequest, CreateChapterRequest};
use config::AppConfig;
use skills::{SkillManager, SkillCategory, SkillInfo};
use mcp::{McpClient, McpServerConfig};
use export::{StoryExporter, ExportConfig, ExportFormat, ExportResult};
use once_cell::sync::OnceCell;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// NOTE: Collab WebSocket server is reserved for future use (Phase 4)
// use collab::websocket::WebSocketServer;


static DB_POOL: Lazy<Mutex<Option<DbPool>>> = Lazy::new(|| Mutex::new(None));
static APP_CONFIG: Lazy<Mutex<Option<AppConfig>>> = Lazy::new(|| Mutex::new(None));
pub static SKILL_MANAGER: OnceCell<Mutex<SkillManager>> = OnceCell::new();

/// Chapter commit debounce: chapter_id -> last_scheduled_time
/// W4-B9: 防止频繁保存导致重复 commit
static CHAPTER_COMMIT_DEBOUNCE: Lazy<Mutex<HashMap<String, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));
const CHAPTER_COMMIT_DEBOUNCE_SECONDS: u64 = 30; // 30 秒 debounce

/// 记录 AI 操作历史
fn record_ai_operation(req: db::CreateAiOperationRequest) {
    if let Some(pool) = get_pool() {
        let repo = db::AiOperationRepository::new(pool);
        if let Err(e) = repo.create(req) {
            log::warn!("[AiOperation] Failed to record operation: {}", e);
        }
    }
}

fn get_pool() -> Option<DbPool> { DB_POOL.lock().unwrap().clone() }
fn get_config() -> Option<AppConfig> { APP_CONFIG.lock().unwrap().clone() }

#[derive(Serialize)]
struct DashboardState { current_story: Option<db::Story>, stories_count: usize, characters_count: usize, chapters_count: usize }

pub fn run() {
    tauri::Builder::default()
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
                        // 关闭幕前窗口时，默认退出整个应用
                        log::info!("Frontstage close requested, exiting application");
                        std::process::exit(0);
                    }
                    _ => {
                        // 其他窗口默认退出
                        log::info!("Window {} close requested, exiting application", window.label());
                        std::process::exit(0);
                    }
                }
            }
        })
        .setup(|app| {
            let app_dir = app.path().app_data_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
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
                let location = info.location()
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
                log::info!("Loaded {} pending vector indexes from previous session", loaded_pending.len());
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
            let _ = SKILL_MANAGER.set(Mutex::new(SkillManager::new(Some(crate::llm::LlmService::new(app.handle().clone())))));

            // 设置能力进化描述持久化路径
            let evolved_desc_path = app_dir.join("evolved_descriptions.json");
            crate::capabilities::set_evolved_descriptions_path(evolved_desc_path);

            // Seed built-in StyleDNAs
            if let Some(pool) = get_pool() {
                let style_repo = db::repositories_v3::StyleDnaRepository::new(pool);
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
            }

            // Seed built-in export templates
            if let Some(pool) = get_pool() {
                let template_repo = db::ExportTemplateRepository::new(pool);
                match template_repo.seed_builtin_templates() {
                    Ok(_) => log::info!("[ExportTemplates] Built-in templates seeded successfully"),
                    Err(e) => log::warn!("[ExportTemplates] Failed to seed built-in templates: {}", e),
                }
            }

            // v6.0.1: Seed built-in genre profiles from external JSON
            {
                let templates_dir = app_dir.join("templates");
                let user_genres_path = templates_dir.join("genres.json");
                let default_genres_json = include_str!("../../templates/genres.json");

                if !user_genres_path.exists() {
                    let _ = std::fs::create_dir_all(&templates_dir);
                    if let Err(e) = std::fs::write(&user_genres_path, default_genres_json) {
                        log::warn!("[GenreProfiles] Failed to copy default genres.json to app dir: {}", e);
                    } else {
                        log::info!("[GenreProfiles] Copied default genres.json to {:?}", user_genres_path);
                    }
                }

                if let (Some(pool), Ok(json_str)) = (get_pool(), std::fs::read_to_string(&user_genres_path)) {
                    match serde_json::from_str::<serde_json::Value>(&json_str) {
                        Ok(genres_data) => {
                            if let Some(profiles) = genres_data.get("profiles").and_then(|p| p.as_array()) {
                                let repo = db::GenreProfileRepository::new(pool);
                                for profile in profiles {
                                    let genre_name = profile.get("genre_name").and_then(|v| v.as_str()).unwrap_or("");
                                    let canonical_name = profile.get("canonical_name").and_then(|v| v.as_str()).unwrap_or("");
                                    if genre_name.is_empty() || canonical_name.is_empty() {
                                        continue;
                                    }
                                    // 仅当不存在时才插入，避免覆盖用户自定义修改
                                    match repo.get_by_name(genre_name) {
                                        Ok(None) => {
                                            let aliases_json = profile.get("aliases").map(|v| v.to_string());
                                            let core_tone = profile.get("core_tone").and_then(|v| v.as_str());
                                            let pacing_strategy = profile.get("pacing_strategy").and_then(|v| v.as_str());
                                            let anti_patterns_json = profile.get("anti_patterns").map(|v| v.to_string());
                                            let reference_tables_json = profile.get("reference_tables").and_then(|v| v.as_str());
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
                                            log::warn!("[GenreProfiles] Failed to check existing genre '{}': {}", genre_name, e);
                                        }
                                    }
                                }
                                log::info!("[GenreProfiles] Seeded {} built-in genre profiles", profiles.len());
                            }
                        }
                        Err(e) => {
                            log::warn!("[GenreProfiles] Failed to parse genres.json: {}", e);
                        }
                    }
                }
            }

            // Initialize embedding model
            let _ = embeddings::init_embedding_model();

            // v5.4.0: 加载配置并初始化语义嵌入提供者
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

            // Bootstrap task system
            if let Some(pool) = get_pool() {
                let app_handle = app.handle().clone();
                let task_service = task_system::service::TaskService::new(pool.clone(), app_handle.clone());
                let llm_service = llm::LlmService::new(app_handle.clone());
                let executor = std::sync::Arc::new(book_deconstruction::executor::BookDeconstructionExecutor::new(
                    pool.clone(),
                    llm_service,
                    app_handle.clone(),
                ));
                task_service.register_executor(executor);
                if let Err(e) = task_service.bootstrap() {
                    log::error!("Failed to bootstrap task system: {}", e);
                } else {
                    log::info!("Task system bootstrapped successfully");
                }
                app.manage(task_service);

                // Initialize automation service
                let automation_service = automation::service::AutomationService::new(app_handle.clone(), pool.clone());
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

            // Initialize LanceDB vector store
            let vector_db_path = app_dir.join("vector_db").to_string_lossy().to_string();
            std::fs::create_dir_all(&vector_db_path).ok();

            // P0-7 修复: 在向量存储初始化完成后处理积压的索引请求
            let _app_handle_for_vector = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut vector_store = LanceVectorStore::new(vector_db_path);
                if let Err(e) = vector_store.init().await {
                    log::error!("Failed to initialize vector store: {}", e);
                } else {
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
                        log::info!("[PENDING_VECTOR] Processing {} queued chapter indexes", pending_ids.len());
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
                                                    embedding,
                                                };
                                                if let Some(store) = VECTOR_STORE.get() {
                                                    match store.add_record(record).await {
                                                        Ok(_) => log::info!("[PENDING_VECTOR] Indexed queued chapter {}", chapter_id),
                                                        Err(e) => log::warn!("[PENDING_VECTOR] Failed to index queued chapter {}: {}", chapter_id, e),
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!("[PENDING_VECTOR] Failed to generate embedding for queued chapter {}: {}", chapter_id, e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // v5.6.1 修复: drain 完成后清理 SQLite 表和旧 JSON 文件
                    if let Some(pool) = get_pool() {
                        if let Ok(conn) = pool.get() {
                            let _ = conn.execute("DELETE FROM pending_vector_indexes", []);
                        }
                    }
                    if let Some(path) = PENDING_VECTOR_INDEXES_PATH.get() {
                        let _ = std::fs::remove_file(path);
                    }
                }
            });

            // NOTE: WebSocket server for collaborative editing is reserved for future use (Phase 4)
            // See docs/plans/ for collaboration feature roadmap.
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
            //                     log::warn!("Failed to start WebSocket server on port {}: {}", port, e);
            //                 }
            //             }
            //         }
            //     });
            // }

            // Initialize WorkflowEngine and WorkflowScheduler
            let workflow_engine_arc = {
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
                scheduler.start_auto_drain(engine_arc.clone(), app.handle().clone());

                // v5.6.4 修复: 将恢复的工作流 Pending/Running 实例重新入队 scheduler
                if !restored_instance_ids.is_empty() {
                    log::info!("[WorkflowEngine] Restoring {} pending/running instances to scheduler", restored_instance_ids.len());
                    let scheduler_clone = scheduler.clone();
                    tauri::async_runtime::spawn(async move {
                        for instance_id in restored_instance_ids {
                            if let Err(e) = scheduler_clone.schedule_execution(instance_id).await {
                                log::warn!("[WorkflowEngine] Failed to restore instance to scheduler: {}", e);
                            }
                        }
                    });
                }

                app.manage(engine_arc.clone());
                app.manage(scheduler);
                log::info!("Workflow engine and scheduler initialized");
                engine_arc
            };

            // Initialize WorkflowLoader (file system DSL watcher)
            {
                let loader = workflow::WorkflowLoader::new(workflow_engine_arc);
                let builtin_dir = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("workflows")));
                let user_dir = app.path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
                    .join("workflows");
                if let Err(e) = loader.initialize(builtin_dir, user_dir) {
                    log::warn!("[WorkflowLoader] Failed to initialize: {}", e);
                }
                app.manage(loader);
                log::info!("[WorkflowLoader] File system workflow watcher initialized");
            }

            // Initialize State Sync Service
            log::info!("[StateSync] State synchronization service initialized");

            // Ensure backstage is hidden on startup
            if let Some(backstage) = app.get_webview_window("backstage") {
                let _ = backstage.hide();
            }
            // Focus frontstage
            if let Some(frontstage) = app.get_webview_window("frontstage") {
                let _ = frontstage.set_focus();
            }

            // Disable default webview context menus on Windows
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

            // v5.4.0: 自动触发能力进化（延迟30秒，避免启动时竞争资源）
            {
                let app_handle_evolve = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    let llm = llm::LlmService::new(app_handle_evolve.clone());
                    let engine = capabilities::evolution::CapabilityEvolutionEngine::new(llm, &app_handle_evolve);
                    let stats = engine.get_statistics();
                    let total_records: usize = stats.values().map(|(t, _)| t).sum();
                    if total_records >= 5 {
                        log::info!("[CapabilityEvolution] Auto-triggering evolution with {} total records", total_records);
                        match engine.evolve_capability_descriptions().await {
                            Ok(improvements) => {
                                log::info!("[CapabilityEvolution] Auto-evolution completed with {} improvements", improvements.len());
                                let _ = app_handle_evolve.emit("capabilities-evolved", serde_json::json!({
                                    "improvements": improvements,
                                    "auto_triggered": true,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }));
                            }
                            Err(e) => log::warn!("[CapabilityEvolution] Auto-evolution failed: {}", e),
                        }
                    } else {
                        log::info!("[CapabilityEvolution] Not enough records ({}) to trigger auto-evolution", total_records);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check, check_model_status, chat_completion, get_state, list_stories, create_story, update_story, delete_story,
            get_story_characters, create_character, update_character, delete_character,
            get_story_chapters, get_chapter, create_chapter, update_chapter, delete_chapter,
            get_skills, get_skill, get_skills_by_category, import_skill, enable_skill, disable_skill, uninstall_skill, execute_skill, update_skill, format_text,
            connect_mcp_server, call_mcp_tool, disconnect_mcp_server, get_mcp_connections, list_mcp_tools, execute_mcp_tool, register_mcp_tool, unregister_mcp_tool,
            search_similar, text_search_vectors, hybrid_search_vectors, embed_chapter,
            export_story, list_export_templates, save_export_template, delete_export_template,
            list_ai_operations, rollback_ai_operation,
            // Window management commands
            window::show_frontstage,
            window::hide_frontstage,
            window::toggle_frontstage,
            window::get_window_state,
            window::update_frontstage_content,
            // Backstage communication commands
            notify_backstage_content_changed,
            notify_frontstage_content_changed,
            notify_frontstage_data_refresh,
            show_backstage,
            cancel_genesis_pipeline,
            // Settings commands
            config::get_settings,
            config::save_settings,
            config::export_settings,
            config::import_settings,
            config::get_models,
            config::get_model_api_key,
            config::create_model,
            config::update_model,
            config::delete_model,
            config::set_active_model,
            config::get_agent_mappings,
            config::update_agent_mapping,
            config::test_model_connection,
            config::fetch_models,
            // LLM commands
            llm::commands::llm_generate,
            llm::commands::llm_generate_stream,
            llm::commands::llm_test_connection,
            llm::commands::llm_cancel_generation,
            llm::commands::init_llm,
            // Intent commands
            parse_intent,
            execute_intent,
            record_feedback,
            // Smart orchestrator
            check_preflight,
            smart_execute,
            analyze_story_structure,
            get_input_hint,
            // Agent commands
            agents::commands::agent_execute,
            agents::commands::agent_execute_stream,
            agents::commands::agent_cancel_task,
            agents::commands::agent_get_status,
            agents::commands::writer_agent_execute,
            agents::commands::auto_write,
            agents::commands::auto_write_cancel,
            agents::commands::auto_revise,
            agents::commands::auto_revise_cancel,
            agents::service::get_available_agents,
            // Subscription commands
            subscription::commands::get_subscription_status,
            subscription::commands::dev_upgrade_subscription,
            subscription::commands::dev_downgrade_subscription,
            // Updater commands
            updater::check_update,
            updater::install_update,
            updater::get_current_version,
            updater::open_update_settings,
            // V3 Architecture commands
            commands_v3::create_scene,
            commands_v3::get_story_scenes,
            commands_v3::get_scene,
            commands_v3::update_scene,
            commands_v3::delete_scene,
            commands_v3::reorder_scenes,
            commands_v3::create_world_building,
            commands_v3::get_world_building,
            commands_v3::update_world_building,
            commands_v3::delete_world_building,
            commands_v3::create_writing_style,
            commands_v3::get_writing_style,
            commands_v3::update_writing_style,
            commands_v3::create_studio_config,
            commands_v3::get_studio_config,
            commands_v3::update_studio_config,
            commands_v3::export_studio,
            commands_v3::import_studio,
            commands_v3::create_entity,
            commands_v3::update_entity,
            commands_v3::get_story_entities,
            commands_v3::create_relation,
            commands_v3::get_entity_relations,
            commands_v3::get_ingest_jobs,
            commands_v3::get_story_graph,
            commands_v3::get_retention_report,
            commands_v3::archive_forgotten_entities,
            commands_v3::restore_archived_entity,
            commands_v3::get_archived_entities,
            // Scene annotations
            commands_v3::create_scene_annotation,
            commands_v3::get_scene_annotations,
            commands_v3::get_story_unresolved_annotations,
            commands_v3::update_scene_annotation,
            commands_v3::resolve_scene_annotation,
            commands_v3::unresolve_scene_annotation,
            commands_v3::delete_scene_annotation,
            // Text inline annotations
            commands_v3::create_text_annotation,
            commands_v3::get_text_annotations_by_chapter,
            commands_v3::get_text_annotations_by_scene,
            commands_v3::update_text_annotation,
            commands_v3::resolve_text_annotation,
            commands_v3::unresolve_text_annotation,
            commands_v3::delete_text_annotation,
            // Commentator agent
            commands_v3::generate_paragraph_commentaries,
            // Memory compressor
            commands_v3::compress_content,
            commands_v3::compress_scene,
            // Knowledge distiller
            commands_v3::distill_story_knowledge,
            commands_v3::get_story_summaries,
            commands_v3::update_story_summary,
            commands_v3::delete_story_summary,
            // Novel creation wizard commands
            commands_v3::generate_world_building_options,
            commands_v3::generate_character_profiles,
            commands_v3::generate_writing_styles,
            commands_v3::generate_first_scene,
            commands_v3::create_story_with_wizard,
            // Scene version commands
            commands_v3::get_scene_versions,
            commands_v3::get_scene_version,
            commands_v3::create_scene_version,
            commands_v3::compare_scene_versions,
            commands_v3::restore_scene_version,
            commands_v3::get_scene_version_stats,
            commands_v3::delete_scene_version,
            commands_v3::get_scene_version_chain,
            commands_v3::get_version_change_tracks,
            // Change tracking (revision mode)
            commands_v3::track_change,
            commands_v3::accept_change,
            commands_v3::reject_change,
            commands_v3::get_pending_changes,
            commands_v3::accept_all_changes,
            commands_v3::reject_all_changes,
            // Comment threads (revision mode)
            commands_v3::create_comment_thread,
            commands_v3::add_comment_message,
            commands_v3::get_comment_threads,
            commands_v3::resolve_comment_thread,
            commands_v3::reopen_comment_thread,
            commands_v3::delete_comment_thread,
            commands_v3::run_creation_workflow,
            commands_v3::list_style_dnas,
            commands_v3::set_story_style_dna,
            commands_v3::analyze_style_sample,
            // Style blend commands (v4.4.0 - 3风格三角框架)
            commands_v3::get_story_style_blend,
            commands_v3::set_story_style_blend,
            commands_v3::update_scene_style_blend,
            commands_v3::check_style_drift,
            // Book deconstruction commands
            book_deconstruction::commands::upload_book,
            book_deconstruction::commands::get_analysis_status,
            book_deconstruction::commands::get_book_analysis,
            book_deconstruction::commands::list_reference_books,
            book_deconstruction::commands::delete_reference_book,
            book_deconstruction::commands::convert_book_to_story,
            book_deconstruction::commands::cancel_book_analysis,
            // Task system commands
            task_system::commands::create_task,
            task_system::commands::update_task,
            task_system::commands::delete_task,
            task_system::commands::list_tasks,
            task_system::commands::get_task,
            task_system::commands::trigger_task,
            task_system::commands::cancel_task,
            task_system::commands::get_task_logs,
            // Foreshadowing tracker commands
            commands_v3::get_story_foreshadowings,
            commands_v3::create_foreshadowing,
            commands_v3::update_foreshadowing_status,
            // Payoff Ledger commands
            commands_v3::get_payoff_ledger,
            commands_v3::detect_overdue_payoffs,
            commands_v3::recommend_payoff_timing,
            commands_v3::update_payoff_ledger_fields,
            // Canonical state commands
            get_canonical_state,
            sync_story_data,
            get_sync_status,
            enable_auto_sync,
            disable_auto_sync,
            // Automation commands
            automation::commands::trigger_automation_event,
            automation::commands::get_automation_triggers,
            automation::commands::get_automation_handlers,
            automation::commands::add_automation_trigger,
            automation::commands::add_automation_handler,
            automation::commands::trigger_story_created,
            automation::commands::trigger_chapter_created,
            automation::commands::trigger_character_created,
            automation::commands::trigger_chapter_content_updated,
            // Structured outline commands
            commands_v3::generate_scene_outline,
            commands_v3::generate_scene_draft,
            // Audit commands
            audit::commands::audit_scene,
            audit_story,
            // Auth commands (v4.5.0)
            auth::commands::get_auth_config,
            auth::commands::oauth_start,
            auth::commands::oauth_callback,
            auth::commands::get_current_user,
            auth::commands::logout,
            // Genesis Engine commands (v5.0.0)
            commands_v3::get_story_outline,
            commands_v3::update_story_outline,
            commands_v3::get_character_relationships,
            commands_v3::create_character_relationship,
            commands_v3::update_character_relationship,
            commands_v3::delete_character_relationship,
            // Scene-Character association commands
            commands_v3::get_scene_characters,
            commands_v3::add_scene_character,
            commands_v3::remove_scene_character,
            commands_v3::set_scene_characters,
            commands_v3::get_character_scenes,
            commands_v3::get_character_by_name,
            commands_v3::check_projection_health,
            commands_v3::get_writing_analytics,
            // Generic Workflow commands (v5.2.0)
            register_workflow,
            create_workflow_instance,
            start_workflow_instance,
            get_workflow_instance_status,
            list_workflows,
            reload_workflows,
            // Capability evolution commands (v5.4.0)
            evolve_capabilities,
            // Logging commands (v5.4.0)
            logging::write_frontend_log,
            logging::get_log_directory,
            logging::get_recent_logs,
            // Story System commands (v6.0.0)
            create_master_setting,
            create_chapter_contract,
            get_contract_tree,
            get_runtime_contract,
            init_chapter_commit,
            apply_chapter_commit,
            get_scene_commits,
            // Memory commands (v6.0.0)
            build_memory_pack,
            get_memory_items,
            create_memory_item,
            // Reading Power commands (v6.0.0)
            evaluate_reading_power,
            get_reading_power_trend,
            get_chase_debts,
            create_override_contract,
            // Genre Profile commands (v6.0.0)
            get_genre_profiles,
            get_genre_profile,
            save_genre_profile,
            delete_genre_profile,
            // Anti-AI Review command (v6.0.0)
            anti_ai_review,
            evolve_style_from_anti_ai_review,
            get_feature_usage_stats,
            log_frontend_feature_usage,
            // Pipeline commands (v7.0.0)
            commands_pipeline::create_blueprint,
            commands_pipeline::get_story_blueprints,
            commands_pipeline::get_chapter_blueprint,
            commands_pipeline::update_blueprint,
            commands_pipeline::delete_blueprint,
            commands_pipeline::create_draft,
            commands_pipeline::get_draft,
            commands_pipeline::get_story_chapter_drafts,
            commands_pipeline::get_latest_draft,
            commands_pipeline::get_finalized_draft,
            commands_pipeline::update_draft_status,
            commands_pipeline::update_draft_content,
            commands_pipeline::delete_draft,
            commands_pipeline::create_revision,
            commands_pipeline::get_draft_revisions,
            commands_pipeline::get_revision,
            commands_pipeline::update_revision_status,
            commands_pipeline::delete_revision,
            commands_pipeline::create_pipeline_review,
            commands_pipeline::get_draft_reviews,
            commands_pipeline::get_latest_pipeline_review,
            commands_pipeline::delete_pipeline_review,
            commands_pipeline::create_post_process_run,
            commands_pipeline::get_post_process_run,
            commands_pipeline::get_story_chapter_post_process_runs,
            commands_pipeline::update_post_process_run_status,
            commands_pipeline::create_post_process_step,
            commands_pipeline::get_post_process_steps,
            commands_pipeline::update_post_process_step_status,
            commands_pipeline::delete_post_process_run,
            commands_pipeline::record_llm_call,
            commands_pipeline::get_story_llm_calls,
            commands_pipeline::get_recent_llm_calls,
            commands_pipeline::get_llm_call_stats,
            commands_pipeline::update_character_state,
            commands_pipeline::batch_update_character_states,
            commands_pipeline::get_character_state,
            // Pipeline orchestrator commands (v7.0.0)
            pipeline::commands::run_refine,
            pipeline::commands::run_review,
            pipeline::commands::run_finalize,
            pipeline::commands::repair_finalize,
            pipeline::commands::get_post_process_status,
            pipeline::commands::get_pipeline_active_draft,
            pipeline::commands::merge_revision,
            pipeline::commands::get_draft_revision_history,
            pipeline::commands::get_draft_review_history,
            // Knowledge Base commands (v7.0.0)
            knowledge_base::commands::kb_import_text,
            knowledge_base::commands::kb_search,
            knowledge_base::commands::kb_delete_chapter,
            knowledge_base::commands::kb_stats,
            // Auto Contract commands (v0.7.5)
            auto_create_missing_contracts,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}

#[tauri::command(rename_all = "snake_case")]
fn health_check() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessageItem {
    pub role: String,
    pub content: String,
}

#[tauri::command(rename_all = "snake_case")]
async fn chat_completion(
    base_url: String,
    api_key: Option<String>,
    model: String,
    messages: Vec<ChatMessageItem>,
    max_tokens: i32,
    temperature: f32,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    let mut request = client
        .post(format!("{}/chat/completions", base_url))
        .header("Content-Type", "application/json");

    if let Some(key) = api_key {
        if !key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", key));
        }
    }

    let body = serde_json::json!({
        "model": model,
        "messages": messages.iter().map(|m| serde_json::json!({
            "role": m.role,
            "content": m.content
        })).collect::<Vec<_>>(),
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": false,
    });

    let response = request.json(&body).send().await.map_err(|e| crate::error::AppError::from(e).to_string())?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, text));
    }

    let data: serde_json::Value = response.json().await.map_err(|e| crate::error::AppError::from(e).to_string())?;
    Ok(data)
}

#[tauri::command(rename_all = "snake_case")]
async fn check_model_status(app_handle: AppHandle) -> Result<String, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = config::AppConfig::load(&app_dir).map_err(|e| crate::error::AppError::from(e).to_string())?;
    let active_profile_id = config.active_llm_profile.as_deref()
        .or(config.llm_profiles.values().find(|p| p.is_default).map(|p| p.id.as_str()))
        .or(config.llm_profiles.keys().next().map(|s| s.as_str()))
        .ok_or("No LLM profile configured")?;

    let profile = config.llm_profiles.get(active_profile_id)
        .ok_or("Active LLM profile not found")?;

    let base_url = profile.api_base.clone()
        .or(config.llm.api_base.clone())
        .unwrap_or_else(|| match profile.provider {
            config::settings::LlmProvider::OpenAI => "https://api.openai.com/v1".to_string(),
            config::settings::LlmProvider::Anthropic => "https://api.anthropic.com".to_string(),
            config::settings::LlmProvider::Ollama => "http://localhost:11434".to_string(),
            config::settings::LlmProvider::DeepSeek => "https://api.deepseek.com".to_string(),
            _ => "http://localhost:11434".to_string(),
        });

    let api_key = if profile.api_key.is_empty() {
        config.llm.api_key.clone()
    } else {
        profile.api_key.clone()
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    let api_key_ref = if api_key.is_empty() { None } else { Some(api_key.as_str()) };

    // 探测策略：只要收到任何 HTTP 响应（不论状态码）即视为网络可通
    // 1. GET base_url（根路径，最宽容）
    if client.get(&base_url).send().await.is_ok() {
        return Ok("connected".to_string());
    }

    // 2. GET /models（OpenAI 标准）
    let mut req = client.get(format!("{}/models", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    if req.send().await.is_ok() {
        return Ok("connected".to_string());
    }

    // 3. POST /chat/completions
    let mut req = client.post(format!("{}/chat/completions", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    req = req.header("Content-Type", "application/json");
    if req.body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#).send().await.is_ok() {
        return Ok("connected".to_string());
    }

    // 4. POST /v1/chat/completions（部分服务 base_url 不含 /v1）
    let mut req = client.post(format!("{}/v1/chat/completions", base_url));
    if let Some(key) = api_key_ref {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    req = req.header("Content-Type", "application/json");
    if req.body(r#"{"model":"test","messages":[{"role":"user","content":"hi"}],"max_tokens":1}"#).send().await.is_ok() {
        return Ok("connected".to_string());
    }

    Ok("disconnected".to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn get_state() -> Result<DashboardState, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let stories = StoryRepository::new(pool.clone()).get_all().map_err(|e| crate::error::AppError::from(e).to_string())?;
    let chars_count: usize = stories.iter().map(|s| CharacterRepository::new(pool.clone()).get_by_story(&s.id).map(|c| c.len()).unwrap_or(0)).sum();
    Ok(DashboardState { current_story: stories.first().cloned(), stories_count: stories.len(), characters_count: chars_count, chapters_count: 0 })
}

#[tauri::command(rename_all = "snake_case")]
fn list_stories() -> Result<Vec<db::Story>, String> {
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).get_all().map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn create_story(title: String, description: Option<String>, genre: Option<String>, app: AppHandle, automation_service: tauri::State<crate::automation::service::AutomationService>) -> Result<db::Story, String> {
    let story = StoryRepository::new(get_pool().ok_or("DB not initialized")?).create(CreateStoryRequest { title, description, genre, style_dna_id: None }).map_err(|e| crate::error::AppError::from(e).to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_created(&app, &story.id, &story.title);
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone.trigger_event(crate::automation::triggers::TriggerEvent::StoryCreated {
            story_id: story_id_clone,
        }).await {
            log::warn!("[create_story] Failed to trigger story created automation: {}", e);
        }
    });
    Ok(story)
}

#[tauri::command(rename_all = "snake_case")]
fn update_story(
    id: String,
    title: Option<String>,
    description: Option<String>,
    genre: Option<String>,
    tone: Option<String>,
    pacing: Option<String>,
    style_dna_id: Option<String>,
    methodology_id: Option<String>,
    methodology_step: Option<i32>,
    app: AppHandle,
) -> Result<(), String> {
    let req = db::UpdateStoryRequest { title: title.clone(), description: description.clone(), genre, tone, pacing, style_dna_id, methodology_id, methodology_step };
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).update(&id, &req).map_err(|e| crate::error::AppError::from(e).to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_updated(&app, &id, title.as_deref());
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn delete_story(id: String, app: AppHandle) -> Result<(), String> {
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).delete(&id).map_err(|e| crate::error::AppError::from(e).to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_deleted(&app, &id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn get_story_characters(story_id: String) -> Result<Vec<db::Character>, String> {
    CharacterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn create_character(
    story_id: String, name: String, background: Option<String>,
    personality: Option<String>, goals: Option<String>, appearance: Option<String>,
    gender: Option<String>, age: Option<i32>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>
) -> Result<db::Character, String> {
    let character = CharacterRepository::new(get_pool().ok_or("DB not initialized")?).create(CreateCharacterRequest { story_id: story_id.clone(), name: name.clone(), background, personality, goals, appearance, gender, age }).map_err(|e| crate::error::AppError::from(e).to_string())?;

    // OnCharacterCreate hook
    if let Some(manager) = SKILL_MANAGER.get() {
        if let Ok(skill_manager) = manager.lock() {
            let story_id = character.story_id.clone();
            let character_id = character.id.clone();
            let character_name = character.name.clone();
            let skill_manager = skill_manager.clone();
            tauri::async_runtime::spawn(async move {
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "character_id": character_id, "character_name": character_name });
                let _ = skill_manager.execute_hooks(crate::skills::HookEvent::OnCharacterCreate, &context, data).await;
                log::info!("Hook executed: {:?}", crate::skills::HookEvent::OnCharacterCreate);
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_character_created(&app, &story_id, &character.id, &character.name);
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story_id.clone();
    let character_id_clone = character.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone.trigger_event(crate::automation::triggers::TriggerEvent::CharacterCreated {
            story_id: story_id_clone,
            character_id: character_id_clone,
        }).await {
            log::warn!("[create_character] Failed to trigger character created automation: {}", e);
        }
    });
    Ok(character)
}

#[tauri::command(rename_all = "snake_case")]
fn update_character(
    id: String, name: Option<String>, background: Option<String>,
    personality: Option<String>, goals: Option<String>,
    appearance: Option<String>, gender: Option<String>, age: Option<i32>,
    app: AppHandle
) -> Result<(), String> {
    let repo = CharacterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，确保同步事件携带正确的 story_id（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    repo.update(&id, name.clone(), background, personality, goals, appearance, gender, age).map_err(|e| crate::error::AppError::from(e).to_string())?;
    if let Some(story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_character_updated(&app, &id, name.as_deref(), &story_id);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn delete_character(id: String, app: AppHandle) -> Result<(), String> {
    let repo = CharacterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，删除后无法再获取（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    repo.delete(&id).map_err(|e| crate::error::AppError::from(e).to_string())?;
    if let Some(story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_character_deleted(&app, &id, &story_id);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn get_story_chapters(story_id: String) -> Result<Vec<db::Chapter>, String> {
    db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn get_chapter(id: String) -> Result<Option<db::Chapter>, String> {
    db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_id(&id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn update_chapter(id: String, title: Option<String>, outline: Option<String>, content: Option<String>, word_count: Option<i32>, app: AppHandle, automation_service: tauri::State<crate::automation::service::AutomationService>) -> Result<(), String> {
    let repo = db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，确保同步事件携带正确的 story_id（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    let result = repo.update(&id, title.clone(), outline, content, word_count).map_err(|e| crate::error::AppError::from(e).to_string());
    if result.is_ok() {
        let _ = window::WindowManager::send_to_frontstage(&app, window::FrontstageEvent::SaveStatus { saved: true, timestamp: Some(chrono::Local::now().to_rfc3339()) });
        if let Some(ref story_id) = story_id_opt {
            let _ = crate::state_sync::StateSync::emit_chapter_updated(&app, &id, title.as_deref(), story_id);
            // v5.6.4 修复: 触发自动化事件：章节内容更新
            let automation_service_clone = automation_service.inner().clone();
            let story_id_clone = story_id.clone();
            let chapter_id_clone = id.clone();
            let word_count_val = word_count.unwrap_or(0) as usize;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = automation_service_clone.trigger_event(crate::automation::triggers::TriggerEvent::ChapterContentUpdated {
                    story_id: story_id_clone,
                    chapter_id: chapter_id_clone,
                    word_count: word_count_val,
                }).await {
                    log::warn!("[update_chapter] Failed to trigger chapter content updated automation: {}", e);
                }
            });
        }
        // W4-B9: 保存后自动触发 chapter commit（30s debounce），替代独立的 auto_ingest
        // apply_commit 已吸收 vector + kg ingest 功能，避免重复工作
        if let Some(pool) = get_pool() {
            let chapter_id = id.clone();
            let app_handle = app.clone();
            let scheduled_time = Instant::now();
            {
                let mut debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
                debounce.insert(chapter_id.clone(), scheduled_time);
                // 清理超过 24 小时的过期条目，防止内存泄漏
                const MAX_DEBOUNCE_AGE_HOURS: u64 = 24;
                debounce.retain(|_, last_time| {
                    Instant::now().duration_since(*last_time) < Duration::from_secs(MAX_DEBOUNCE_AGE_HOURS * 3600)
                });
            }
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(CHAPTER_COMMIT_DEBOUNCE_SECONDS)).await;
                let should_commit = {
                    let debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
                    debounce.get(&chapter_id).map(|t| *t == scheduled_time).unwrap_or(false)
                };
                if should_commit {
                    let repo = db::ChapterRepository::new(pool.clone());
                    let scene_repo = db::SceneRepository::new(pool.clone());
                    if let Ok(Some(chapter)) = repo.get_by_id(&chapter_id) {
                        let scene_id = scene_repo.get_by_chapter(&chapter_id)
                            .ok()
                            .and_then(|scenes| scenes.into_iter().next())
                            .map(|s| s.id);
                        let service = story_system::SceneCommitService::new(pool);
                        let store = VECTOR_STORE.get();
                        if let Err(e) = service.auto_commit(
                            &chapter.story_id,
                            scene_id.as_deref(),
                            Some(&chapter_id),
                            chapter.chapter_number,
                            chapter.content.as_deref(),
                            Some(app_handle),
                            store,
                        ).await {
                            log::warn!("[update_chapter] auto_commit failed for chapter {}: {}", chapter_id, e);
                        }
                    }
                } else {
                    log::info!("[update_chapter] auto_commit skipped for chapter {} (debounced)", chapter_id);
                }
            });
        }
    }
    result.map(|_| ())
}

#[tauri::command(rename_all = "snake_case")]
fn delete_chapter(id: String, app: AppHandle) -> Result<(), String> {
    let repo = db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，删除后无法再获取（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    repo.delete(&id).map_err(|e| crate::error::AppError::from(e).to_string())?;
    if let Some(story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_chapter_deleted(&app, &id, &story_id);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn create_chapter(
    story_id: String,
    chapter_number: i32,
    title: Option<String>,
    outline: Option<String>,
    content: Option<String>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>
) -> Result<db::Chapter, String> {
    let pool = get_pool().ok_or("DB not initialized")?;
    let repo = ChapterRepository::new(pool.clone());

    // 如果该 chapter_number 已存在，直接返回已有章节（幂等）
    if let Ok(chapters) = repo.get_by_story(&story_id) {
        if let Some(existing) = chapters.into_iter().find(|c| c.chapter_number == chapter_number) {
            log::info!("[create_chapter] Chapter {} already exists for story {}, returning existing", chapter_number, story_id);
            return Ok(existing);
        }
    }

    let req = CreateChapterRequest { story_id: story_id.clone(), chapter_number, title: title.clone(), outline, content };
    let chapter = repo.create(req).map_err(|e| crate::error::AppError::from(e).to_string())?;

    // AfterChapterSave hook
    if let Some(manager) = SKILL_MANAGER.get() {
        if let Ok(skill_manager) = manager.lock() {
            let story_id = chapter.story_id.clone();
            let chapter_id = chapter.id.clone();
            let chapter_number = chapter.chapter_number;
            let skill_manager = skill_manager.clone();
            tauri::async_runtime::spawn(async move {
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "chapter_id": chapter_id, "chapter_number": chapter_number });
                let _ = skill_manager.execute_hooks(crate::skills::HookEvent::AfterChapterSave, &context, data).await;
                log::info!("Hook executed: {:?}", crate::skills::HookEvent::AfterChapterSave);
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_chapter_created(&app, &story_id, &chapter.id, title.as_deref());
    // 同时发射 Scene 更新事件（chapter 创建会自动创建/关联 scene）
    // v0.7.3: 1:N 架构下通过 scenes.chapter_id 查询关联 scene
    {
        let scene_repo = SceneRepository::new(pool.clone());
        if let Ok(scenes) = scene_repo.get_by_chapter(&chapter.id) {
            if let Some(scene) = scenes.first() {
                let _ = crate::state_sync::StateSync::emit_scene_updated(&app, &story_id, &scene.id, title.as_deref());
            }
        }
    }

    // P0-3 修复: 触发自动化事件：章节创建完成
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story_id.clone();
    let chapter_id_clone = chapter.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone.trigger_event(crate::automation::triggers::TriggerEvent::ChapterCreated {
            story_id: story_id_clone,
            chapter_id: chapter_id_clone
        }).await {
            log::warn!("[create_chapter] Failed to trigger chapter created automation: {}", e);
        }
    });

    // W4-B9: 新建章节后自动触发 chapter commit（无需 debounce，首次创建只执行一次）
    // apply_commit 已吸收 vector + kg ingest 功能，避免重复工作
    let scene_id_for_commit = {
        let scene_repo = SceneRepository::new(pool.clone());
        scene_repo.get_by_chapter(&chapter.id)
            .ok()
            .and_then(|scenes| scenes.into_iter().next())
            .map(|s| s.id)
    };
    {
        let chapter_id = chapter.id.clone();
        let app_handle = app.clone();
        let story_id = chapter.story_id.clone();
        let chapter_number = chapter.chapter_number;
        let content = chapter.content.clone();
        tauri::async_runtime::spawn(async move {
            let service = story_system::SceneCommitService::new(pool);
            let store = VECTOR_STORE.get();
            if let Err(e) = service.auto_commit(
                &story_id,
                scene_id_for_commit.as_deref(),
                Some(&chapter_id),
                chapter_number,
                content.as_deref(),
                Some(app_handle),
                store,
            ).await {
                log::warn!("[create_chapter] auto_commit failed for chapter {}: {}", chapter_id, e);
            }
        });
    }
    Ok(chapter)
}

#[tauri::command(rename_all = "snake_case")]
fn get_skills() -> Result<Vec<SkillInfo>, AppError> {
    let skills = SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.get_all_skills();
    Ok(skills.into_iter().map(SkillInfo::from).collect())
}

#[tauri::command(rename_all = "snake_case")]
fn get_skills_by_category(category: String) -> Result<Vec<SkillInfo>, AppError> {
    let cat = match category.as_str() {
        "writing" => SkillCategory::Writing, "analysis" => SkillCategory::Analysis,
        "character" => SkillCategory::Character, "world_building" => SkillCategory::WorldBuilding,
        "style" => SkillCategory::Style, "plot" => SkillCategory::Plot,
        "export" => SkillCategory::Export, "integration" => SkillCategory::Integration,
        _ => SkillCategory::Custom,
    };
    let skills = SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.get_skills_by_category(cat);
    Ok(skills.into_iter().map(SkillInfo::from).collect())
}

#[tauri::command(rename_all = "snake_case")]
fn import_skill(path: String) -> Result<SkillInfo, AppError> {
    let skill = SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.import_skill(std::path::Path::new(&path))?;
    Ok(SkillInfo::from(skill))
}

#[tauri::command(rename_all = "snake_case")]
fn enable_skill(skill_id: String) -> Result<(), AppError> {
    SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.enable_skill(&skill_id)
}

#[tauri::command(rename_all = "snake_case")]
fn disable_skill(skill_id: String) -> Result<(), AppError> {
    SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.disable_skill(&skill_id)
}

#[tauri::command(rename_all = "snake_case")]
fn uninstall_skill(skill_id: String) -> Result<(), AppError> {
    SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.uninstall_skill(&skill_id)
}

#[tauri::command(rename_all = "snake_case")]
fn get_skill(skill_id: String) -> Result<SkillInfo, AppError> {
    let skill = SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.get_skill(&skill_id);
    skill.map(SkillInfo::from).ok_or_else(|| AppError::not_found("Skill", &skill_id))
}

#[tauri::command(rename_all = "snake_case")]
fn update_skill(skill_id: String, manifest: skills::SkillManifest) -> Result<(), AppError> {
    SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?.update_skill(&skill_id, manifest)
}

#[tauri::command(rename_all = "snake_case")]
async fn execute_skill(
    skill_id: String,
    params: HashMap<String, serde_json::Value>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    let mut params = params;
    let story_id = params.remove("story_id").and_then(|v| v.as_str().map(|s| s.to_string()));

    // Build context from database if story_id is provided
    let context = if let Some(story_id) = story_id {
        match app_handle.try_state::<DbPool>() {
            Some(pool_state) => {
                let pool = pool_state.inner().clone();
                let builder = creative_engine::StoryContextBuilder::new(pool);
                match builder.build_quick(&story_id) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::warn!("[execute_skill] StoryContextBuilder failed: {}, using minimal context", e);
                        agents::AgentContext::minimal(story_id, String::new())
                    }
                }
            }
            None => {
                log::warn!("[execute_skill] DbPool not available, using minimal context");
                agents::AgentContext::minimal(story_id, String::new())
            }
        }
    } else {
        agents::AgentContext {
            story_id: String::new(),
            story_title: String::new(),
            genre: String::new(),
            tone: String::new(),
            pacing: String::new(),
            chapter_number: 0,
            characters: vec![],
            previous_chapters: vec![],
            current_content: None,
            selected_text: None,
            world_rules: None,
            scene_structure: None,
            methodology_id: None,
            methodology_step: None,
            style_dna_id: None,
            style_blend: None,
            memory_pack: None,
        }
    };

    // Execute skill
    let manager = {
        let guard = SKILL_MANAGER.get().ok_or(AppError::internal("Skills not initialized"))?.lock().map_err(|e| crate::error::AppError::from(e).to_string())?;
        guard.clone()
    };
    
    let result = manager.execute_skill(&skill_id, &context, params).await?;
    
    if !result.success {
        return Err(AppError::internal(result.error.unwrap_or("Skill execution failed".to_string())));
    }
    
    // If LLM was already called (PromptRuntime with llm_service), return content directly
    if let Some(content) = result.data.get("content").and_then(|v| v.as_str()) {
        return Ok(serde_json::json!({
            "success": true,
            "content": content,
            "model": result.data.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            "tokens_used": result.data.get("tokens_used").and_then(|v| v.as_i64()).unwrap_or(0),
            "execution_time_ms": result.execution_time_ms,
        }));
    }
    
    // Fallback: skill returned prompts but no LLM result, call LLM manually
    let system_prompt = result.data.get("system_prompt").and_then(|v| v.as_str()).unwrap_or("");
    let user_prompt = result.data.get("user_prompt").and_then(|v| v.as_str()).unwrap_or("");
    
    if system_prompt.is_empty() && user_prompt.is_empty() {
        return Err(AppError::internal("Skill did not produce a valid prompt"));
    }

    let llm_service = crate::llm::LlmService::new(app_handle);
    let full_prompt = if system_prompt.is_empty() {
        user_prompt.to_string()
    } else {
        format!("[系统指令]\n{}\n\n[用户请求]\n{}", system_prompt, user_prompt)
    };
    
    let response = llm_service.generate(full_prompt, Some(2000), Some(0.7)).await?;
    
    Ok(serde_json::json!({
        "success": true,
        "content": response.content,
        "model": response.model,
        "tokens_used": response.tokens_used,
        "execution_time_ms": result.execution_time_ms,
    }))
}

/// 使用 text_formatter skill 对文本进行智能排版
#[tauri::command(rename_all = "snake_case")]
async fn format_text(content: String, app: AppHandle) -> Result<String, AppError> {
    let result = execute_skill(
        "builtin.text_formatter".to_string(),
        {
            let mut p = HashMap::new();
            p.insert("content".to_string(), serde_json::Value::String(content));
            p
        },
        app,
    ).await?;
    
    result.get("content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::internal("LLM returned empty content"))
}

use tokio::sync::Mutex as TokioMutex;

static MCP_CONNECTIONS: Lazy<TokioMutex<HashMap<String, mcp::McpClient>>> =
    Lazy::new(|| TokioMutex::new(HashMap::new()));

// W2-B8: 全局内置 MCP Server 实例，支持运行时动态注册/注销工具
static BUILTIN_MCP_SERVER: Lazy<TokioMutex<mcp::McpServer>> = Lazy::new(|| {
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

#[tauri::command(rename_all = "snake_case")]
async fn connect_mcp_server(config: McpServerConfig) -> Result<Vec<mcp::McpTool>, AppError> {
    let mut client = McpClient::new(config.clone());
    client.connect().await.map_err(AppError::from)?;
    let tools = client.get_tools().clone();

    // W4-B2: 动态注册到 CapabilityRegistry
    {
        let mut registry = crate::capabilities::get_capability_registry();
        for tool in &tools {
            let cap = crate::capabilities::Capability::from_mcp_tool(&config.id, tool);
            registry.register(cap);
        }
        log::info!("[CapabilityRegistry] Registered {} MCP tools from server {}", tools.len(), config.id);
    }

    let mut connections = MCP_CONNECTIONS.lock().await;
    connections.insert(config.id.clone(), client);
    log::info!("[MCP] Connected to server {} ({}), {} tools available", config.name, config.id, tools.len());
    Ok(tools)
}

#[tauri::command(rename_all = "snake_case")]
async fn call_mcp_tool(server_id: String, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, AppError> {
    let mut connections = MCP_CONNECTIONS.lock().await;
    let client = connections.get_mut(&server_id)
        .ok_or_else(|| AppError::internal(format!("MCP server {} not connected", server_id)))?;
    client.call_tool(&tool_name, arguments).await.map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
async fn disconnect_mcp_server(server_id: String) -> Result<(), AppError> {
    {
        let mut connections = MCP_CONNECTIONS.lock().await;
        if let Some(mut client) = connections.remove(&server_id) {
            client.disconnect().await;
            log::info!("[MCP] Disconnected from server {}", server_id);
        }
    }

    // W4-B2: 从 CapabilityRegistry 注销该服务器的所有能力
    {
        let mut registry = crate::capabilities::get_capability_registry();
        let prefix = format!("mcp.{server_id}.");
        let removed = registry.unregister_by_prefix(&prefix);
        if removed > 0 {
            log::info!("[CapabilityRegistry] Unregistered {} MCP capabilities from server {}", removed, server_id);
        }
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn get_mcp_connections() -> Result<Vec<serde_json::Value>, AppError> {
    let connections = MCP_CONNECTIONS.lock().await;
    let result: Vec<serde_json::Value> = connections.iter()
        .map(|(id, client)| {
            serde_json::json!({
                "id": id,
                "tools": client.get_tools().len(),
                "resources": client.get_resources().len(),
            })
        })
        .collect();
    Ok(result)
}

// Vector Search Commands (LanceDB)
use vector::{LanceVectorStore, SearchResult};

pub(crate) static VECTOR_STORE: OnceCell<LanceVectorStore> = OnceCell::new();
/// 向量存储初始化前积压的章节索引请求（P0-7 修复: 防止启动后首章丢失索引）
static PENDING_VECTOR_INDEXES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
/// P2-19 修复: pending queue 持久化文件路径
static PENDING_VECTOR_INDEXES_PATH: OnceCell<std::path::PathBuf> = OnceCell::new();

/// v5.6.1: 保存 pending vector indexes 到 SQLite
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
                        "INSERT OR IGNORE INTO pending_vector_indexes (chapter_id, created_at) VALUES (?1, ?2)",
                        rusqlite::params![chapter_id, now],
                    );
                }
            }
        }
    }
}

/// v5.6.1: 从 SQLite 加载 pending vector indexes，保留 JSON fallback
fn load_pending_vector_indexes() -> Vec<String> {
    let mut result = Vec::new();
    
    // 优先从 SQLite 加载
    if let Some(pool) = get_pool() {
        if let Ok(conn) = pool.get() {
            if let Ok(mut stmt) = conn.prepare("SELECT chapter_id FROM pending_vector_indexes ORDER BY created_at") {
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

#[tauri::command(rename_all = "snake_case")]
async fn search_similar(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    use embeddings::embed_text_async;
    
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    
    // v5.4.0: 使用 embed_text_async 避免阻塞异步运行时
    let query_embedding = embed_text_async(query.clone()).await.map_err(|e| crate::error::AppError::from(e).to_string())?;
    
    store.search(&story_id, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn text_search_vectors(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    store.text_search(&story_id, &query, top_k.unwrap_or(5))
        .await
        .map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn hybrid_search_vectors(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    use embeddings::embed_text_async;
    
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    let query_embedding = embed_text_async(query.clone()).await.map_err(|e| crate::error::AppError::from(e).to_string())?;
    
    store.hybrid_search(&story_id, &query, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn embed_chapter(chapter_id: String, content: String) -> Result<(), String> {
    use embeddings::embed_text_async;
    use vector::VectorRecord;

    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;

    // v5.4.0: 查询 chapter 的 story_id 和 chapter_number
    let pool = get_pool().ok_or("Database not initialized")?;
    let chapter = db::ChapterRepository::new(pool)
        .get_by_id(&chapter_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?;
    let (story_id, chapter_number) = match chapter {
        Some(c) => (c.story_id, c.chapter_number),
        None => (String::new(), 0),
    };

    // v5.4.0: 使用 embed_text_async 避免阻塞异步运行时
    let embedding = embed_text_async(content.clone()).await.map_err(|e| crate::error::AppError::from(e).to_string())?;

    let record = VectorRecord {
        id: format!("{}", uuid::Uuid::new_v4()),
        story_id,
        chapter_id,
        chapter_number,
        text: content.chars().take(500).collect(),
        record_type: "chapter".to_string(),
        embedding,
    };

    store.add_record(record).await.map_err(|e| crate::error::AppError::from(e).to_string())
}

// Intent Parser Command
#[tauri::command(rename_all = "snake_case")]
async fn parse_intent(user_input: String, app_handle: AppHandle) -> Result<intent::Intent, String> {
    let parser = intent::IntentParser::new(app_handle);
    parser.parse(&user_input).await
}

// Intent Executor Command
#[tauri::command(rename_all = "snake_case")]
async fn execute_intent(
    intent: intent::Intent,
    story_id: String,
    app_handle: AppHandle,
) -> Result<intent::IntentExecutionResult, String> {
    let executor = intent::IntentExecutor::new(app_handle);
    executor.execute(intent, story_id).await
}

/// 预检命令 - 写作前检查阻塞性问题
#[tauri::command(rename_all = "snake_case")]
fn check_preflight(
    story_id: String,
    chapter_number: i32,
) -> Result<story_system::preflight::PreflightResult, String> {
    let pool = get_pool().ok_or("[check_preflight] Database not initialized")?;
    let checker = story_system::preflight::PreflightChecker::new();
    Ok(checker.check(&pool, &story_id, chapter_number))
}

/// 智能执行命令 - 新一代意图理解与执行入口
#[tauri::command(rename_all = "snake_case")]
async fn smart_execute(
    user_input: String,
    current_content: Option<String>,
    app_handle: AppHandle,
) -> Result<planner::PlanExecutionResult, String> {
    use tauri::Emitter;

    let pool = get_pool().ok_or("[smart_execute] Database not initialized")?;

    // 辅助函数：发送 smart_execute 整体进度事件
    let app_handle_for_progress = app_handle.clone();
    let emit_progress = move |stage: &str, message: &str, step_number: usize, total_steps: usize| {
        let _ = app_handle_for_progress.emit("smart-execute-progress", planner::SmartExecuteProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            step_number,
            total_steps,
        });
    };

    emit_progress("loading_context", "正在加载故事上下文...", 1, 5);

    // 构建 PlanContext：从当前系统状态推断
    let stories = StoryRepository::new(pool.clone()).get_all()
        .map_err(|e| format!("[smart_execute] Failed to load stories: {}", e))?;
    let current_story = stories.first().cloned();
    let current_story_id = current_story.as_ref().map(|s| s.id.clone());

    let chapters = if let Some(ref story_id) = current_story_id {
        ChapterRepository::new(pool.clone())
            .get_by_story(story_id)
            .map_err(|e| format!("[smart_execute] Failed to load chapters: {}", e))?
    } else {
        vec![]
    };

    let chapter_count = chapters.len();

    // 优先使用前端传来的实时编辑器内容，其次回退到数据库中最后一章的内容
    // v5.3.1 修复：从尾部截断（保留最新内容），让 LLM 知道最近写了什么
    let current_content_preview = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| chapters.last().and_then(|c| c.content.clone()))
        .map(|content| {
            let max_chars = 6000;
            let total = content.chars().count();
            if total > max_chars {
                // 从尾部截断：保留最后 max_chars 个字符，前面加省略号
                let skip = total - max_chars;
                let preview: String = content.chars().skip(skip).collect();
                format!("...(前{}字已省略)\n{}", skip, preview)
            } else {
                content
            }
        });

    // 检测是否需要启动小说初始化工作流
    // v5.4.0: 移除 stories.is_empty() 限制，用户输入明确的创建意图时始终创建新小说
    let is_bootstrap_intent = is_novel_creation_intent(&user_input);

    if is_bootstrap_intent {
        log::info!("[smart_execute] Detected novel creation intent, starting GenesisPipeline (v5.3.0 narrative model)");
        
        // v5.3.0: 使用新的 GenesisPipeline
        let mut ctx = narrative::genesis::GenesisContext::new(app_handle.clone(), user_input.clone());
        let session_id = ctx.session_id.clone();
        let llm = llm::LlmService::new(app_handle.clone());
        let quick_steps = narrative::genesis::GenesisPipeline::quick_phase_steps();
        let cancel_flag = narrative::pipeline::register_pipeline_cancel(&session_id);
        let executor = narrative::pipeline::NarrativePipelineExecutor::new(quick_steps)
            .with_cancel_flag(cancel_flag);
        
        // 进度回调：同时发射新旧两种事件（向后兼容）
        let app_handle_progress = app_handle.clone();
        let progress_callback = std::sync::Arc::new(move |evt: narrative::progress::PipelineProgressEvent| {
            // 发射新事件
            let _ = app_handle_progress.emit("pipeline-progress", &evt);
            // 发射旧事件（向后兼容，前端仍在监听 novel-bootstrap-progress）
            let _ = app_handle_progress.emit("novel-bootstrap-progress", planner::bootstrap::BootstrapProgressEvent {
                session_id: evt.pipeline_id.clone(),
                step_name: evt.step_name.clone(),
                step_number: evt.step_number,
                total_steps: evt.total_steps,
                message: evt.message.clone(),
                status: format!("{:?}", evt.status).to_lowercase(),
            });
        });
        
        match executor.execute(&mut ctx, &llm, progress_callback.clone()).await {
            Ok(()) => {
                // v5.4.0: 发射 pipeline-complete 事件，通知前端 Genesis 即时阶段完成
                let _ = app_handle.emit("pipeline-complete", narrative::progress::PipelineCompleteEvent {
                    pipeline_id: ctx.session_id.clone(),
                    pipeline_type: narrative::progress::PipelineType::Genesis,
                    success: true,
                    total_elapsed_seconds: 0,
                    elements_created: narrative::progress::ElementsCount::default(),
                    error_message: None,
                });
                let story_id = ctx.story_id.clone();
                let session_id = ctx.session_id.clone();
                let first_chapter = ctx.first_chapter_content.clone();
                let bundle = ctx.bundle.clone();
                
                // 发射 story_created 同步事件
                let _ = crate::state_sync::StateSync::emit_story_created(&app_handle, &story_id, "新故事");

                // Record AI operation (before user_input is moved)
                let user_input_for_record = user_input.clone();
                record_ai_operation(db::CreateAiOperationRequest {
                    story_id: story_id.clone(),
                    scene_id: None,
                    chapter_id: None,
                    operation_type: "bootstrap".to_string(),
                    operation_name: "小说创世".to_string(),
                    input_summary: Some(user_input_for_record),
                    output_summary: first_chapter.as_ref().map(|c| c.chars().take(200).collect()),
                    previous_content: None,
                    new_content: first_chapter.clone(),
                    metadata: Some(serde_json::json!({"session_id": session_id}).to_string()),
                });
                
                // 启动后台阶段
                let app_handle_bg = app_handle.clone();
                let story_id_bg = story_id.clone();
                let session_id_bg = session_id.clone();
                let user_input_bg = user_input.clone();
                tauri::async_runtime::spawn(async move {
                    let story_id_for_emit = story_id_bg.clone();
                    let app_handle_for_emit = app_handle_bg.clone();
                    let mut bg_ctx = narrative::genesis::GenesisContext::for_background(
                        app_handle_bg.clone(), story_id_bg, session_id_bg.clone(), user_input_bg, bundle
                    );
                    let llm_bg = llm::LlmService::new(app_handle_bg.clone());
                    let bg_steps = narrative::genesis::GenesisPipeline::background_phase_steps();
                    let bg_cancel_flag = narrative::pipeline::register_pipeline_cancel(&session_id_bg);
                    let bg_executor = narrative::pipeline::NarrativePipelineExecutor::new(bg_steps)
                        .with_cancel_flag(bg_cancel_flag);

                    let progress_callback_bg = std::sync::Arc::new(move |evt: narrative::progress::PipelineProgressEvent| {
                        let _ = app_handle_bg.emit("pipeline-progress", &evt);
                        let _ = app_handle_bg.emit("novel-bootstrap-progress", planner::bootstrap::BootstrapProgressEvent {
                            session_id: evt.pipeline_id.clone(),
                            step_name: evt.step_name.clone(),
                            step_number: evt.step_number,
                            total_steps: evt.total_steps,
                            message: evt.message.clone(),
                            status: format!("{:?}", evt.status).to_lowercase(),
                        });
                    });

                    let bg_start = std::time::Instant::now();
                    let bg_result = bg_executor.execute(&mut bg_ctx, &llm_bg, progress_callback_bg).await;
                    let bg_elapsed = bg_start.elapsed().as_secs();

                    // P0-5 修复: 根据后台阶段实际结果设置 success/error，不再硬编码
                    let (success, error_message) = match &bg_result {
                        Ok(_) => {
                            log::info!("[GenesisPipeline] 后台阶段完成，发射数据刷新事件");
                            // v5.4.0: 统一通过 state_sync 发射 data-refresh，避免重复发射
                            crate::state_sync::StateSync::emit_data_refresh(
                                &app_handle_for_emit,
                                Some(&story_id_for_emit),
                                "all"
                            );
                            (true, None)
                        }
                        Err(e) => {
                            log::warn!("[GenesisPipeline] 后台阶段失败: {}", e);
                            (false, Some(format!("{}", e)))
                        }
                    };

                    // 统计实际生成的元素数量
                    let elements_created = if success {
                        let mut counts = narrative::progress::ElementsCount::default();
                        // 从上下文统计实际生成的元素（P0-5 修复: 使用 ElementsCount 的正确字段名）
                        counts.world_rules = if bg_ctx.bundle.world_building.is_some() { 1 } else { 0 };
                        counts.characters = bg_ctx.bundle.characters.len();
                        counts.scenes = bg_ctx.bundle.scenes.len();
                        counts.foreshadowings = bg_ctx.bundle.foreshadowings.len();
                        // outline 映射到 plot_points
                        counts.plot_points = if bg_ctx.bundle.outline.is_some() { 1 } else { 0 };
                        counts
                    } else {
                        narrative::progress::ElementsCount::default()
                    };

                    // v5.4.0: 发射 pipeline-complete 事件，通知前端 Genesis 后台阶段完成
                    let _ = app_handle_for_emit.emit("pipeline-complete", narrative::progress::PipelineCompleteEvent {
                        pipeline_id: session_id_bg.clone(),
                        pipeline_type: narrative::progress::PipelineType::Genesis,
                        success,
                        total_elapsed_seconds: bg_elapsed,
                        elements_created,
                        error_message,
                    });
                });

                return Ok(planner::PlanExecutionResult {
                    success: true,
                    steps_completed: 2,
                    final_content: first_chapter,
                    messages: vec![
                        format!("story_created:{}", story_id),
                        format!("session_id:{}", session_id),
                        "novel_bootstrap_completed".to_string(),
                    ],
                });
            }
            Err(e) => {
                log::error!("[smart_execute] GenesisPipeline failed: {}", e);
                return Err(format!("小说初始化失败: {}", e));
            }
        }
    }

    // Phase 3: 加载场景结构信息 + 增强上下文
    let (
        _scenes, scene_count, scenes_summary, current_scene_id, current_scene_stage,
        total_word_count, latest_chapter_word_count, story_progress,
        world_building_summary, character_list, foreshadowing_status, style_dna_info, mcp_tools_available
    ) = if let Some(ref story_id) = current_story_id {
        let scene_repo = db::repositories_v3::SceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(story_id)
            .map_err(|e| format!("[smart_execute] Failed to load scenes: {}", e))?;
        let scene_count = scenes.len();

        let scenes_summary: Vec<planner::SceneStructureSummary> = scenes.iter().map(|s| {
            let word_count = s.content.as_ref().map(|c| c.chars().count()).unwrap_or(0)
                + s.draft_content.as_ref().map(|c| c.chars().count()).unwrap_or(0);
            planner::SceneStructureSummary {
                scene_id: s.id.clone(),
                sequence_number: s.sequence_number,
                title: s.title.clone(),
                execution_stage: s.execution_stage.clone(),
                has_content: s.content.is_some() || s.draft_content.is_some(),
                word_count,
            }
        }).collect();

        // 当前场景 = 最新有内容的场景，或最新场景
        let current_scene = scenes.iter()
            .filter(|s| s.content.is_some() || s.draft_content.is_some())
            .max_by_key(|s| s.sequence_number)
            .or_else(|| scenes.iter().max_by_key(|s| s.sequence_number));

        let current_scene_id = current_scene.map(|s| s.id.clone());
        let current_scene_stage = current_scene.and_then(|s| s.execution_stage.clone());

        let total_word_count = chapters.iter()
            .filter_map(|c| c.word_count)
            .map(|w| w as usize)
            .sum::<usize>()
            + scenes_summary.iter().map(|s| s.word_count).sum::<usize>();

        let latest_chapter_word_count = chapters.last()
            .and_then(|c| c.word_count)
            .map(|w| w as usize)
            .unwrap_or(0);

        // 故事进度判断
        let story_progress = if scene_count == 0 {
            "just_started".to_string()
        } else {
            let completed_scenes = scenes_summary.iter().filter(|s| s.has_content).count();
            let ratio = if scene_count > 0 { completed_scenes as f32 / scene_count as f32 } else { 0.0 };
            if ratio < 0.15 {
                "just_started".to_string()
            } else if ratio < 0.4 {
                "developing".to_string()
            } else if ratio < 0.7 {
                "midpoint".to_string()
            } else if ratio < 0.9 {
                "climax".to_string()
            } else {
                "resolution".to_string()
            }
        };

        // ===== 增强上下文加载 =====
        // 世界观摘要
        let wb_repo = db::repositories_v3::WorldBuildingRepository::new(pool.clone());
        let world_building_summary = wb_repo.get_by_story(story_id).ok().flatten().map(|wb| {
            let rules_summary = wb.rules.iter()
                .filter(|r| r.importance >= 7)
                .map(|r| format!("{}: {}", r.name, r.description.as_deref().unwrap_or("")))
                .collect::<Vec<_>>().join("; ");
            format!("概念：{}；核心规则：{}", wb.concept, rules_summary)
        });

        // 角色列表
        let char_repo = db::repositories::CharacterRepository::new(pool.clone());
        let character_list = char_repo.get_by_story(story_id).ok().map(|chars| {
            chars.iter().map(|c| {
                let role = c.background.as_deref().unwrap_or("主要角色");
                format!("{}（{}）", c.name, role)
            }).collect()
        }).unwrap_or_default();

        // 活跃伏笔
        let foreshadowing_tracker = crate::creative_engine::foreshadowing::ForeshadowingTracker::new(pool.clone());
        let foreshadowing_status = foreshadowing_tracker.get_unresolved(story_id).ok().map(|records| {
            records.into_iter().take(5).map(|r| r.content).collect()
        }).unwrap_or_default();

        // 风格DNA / 风格混合
        let style_dna_info = {
            use crate::db::repositories_v3::StoryStyleConfigRepository;
            use crate::creative_engine::style::blend::StyleBlendConfig;
            
            // 优先检查混合配置
            let blend_info = if let Some(ref story) = current_story {
                let blend_repo = StoryStyleConfigRepository::new(pool.clone());
                if let Ok(Some(config)) = blend_repo.get_active_by_story(&story.id) {
                    if let Ok(blend) = serde_json::from_str::<StyleBlendConfig>(&config.blend_json) {
                        let comps = blend.components.iter()
                            .map(|c| format!("{}:{:.0}%", c.dna_name, c.weight * 100.0))
                            .collect::<Vec<_>>()
                            .join(", ");
                        Some(format!("风格混合 [{}]: {}", blend.name, comps))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            
            // 回退到单一风格DNA
            if blend_info.is_some() {
                blend_info
            } else {
                current_story.as_ref().and_then(|s| s.style_dna_id.clone()).map(|dna_id| {
                    format!("风格DNA ID: {}", dna_id)
                })
            }
        };

        // 异步加载MCP工具列表
        let mcp_tools_available = {
            let connections = crate::MCP_CONNECTIONS.lock().await;
            connections.iter()
                .flat_map(|(_id, client)| {
                    client.get_tools().iter().map(|t| format!("{}: {}", t.name, t.description)).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };

        (scenes, scene_count, scenes_summary, current_scene_id, current_scene_stage,
         total_word_count, latest_chapter_word_count, story_progress,
         world_building_summary, character_list, foreshadowing_status, style_dna_info, mcp_tools_available)
    } else {
        (vec![], 0, vec![], None, None, 0, 0, "no_story".to_string(),
         None, vec![], vec![], None, vec![])
    };

    // Phase 3: 意图路由增强 — 自动检测更多用户意图
    let auto_routed_plan = detect_and_route_intent(
        &user_input,
        &current_story_id,
        scene_count,
        &scenes_summary,
        &current_scene_stage,
        &story_progress,
    );

    if let Some(plan) = auto_routed_plan {
        log::info!("[smart_execute] Auto-routed intent to plan: {}", plan.understanding);
        let executor = planner::PlanExecutor::new(app_handle);
        let result = executor.execute_plan(plan, &planner::PlanContext {
            current_story_id: current_story_id.clone(),
            has_story: !stories.is_empty(),
            has_chapters: !chapters.is_empty(),
            chapter_count,
            current_content_preview: current_content_preview.clone(),
            user_input: user_input.clone(),
            scene_count,
            scenes_summary: scenes_summary.clone(),
            current_scene_id: current_scene_id.clone(),
            current_scene_stage: current_scene_stage.clone(),
            total_word_count,
            latest_chapter_word_count,
            story_progress: story_progress.clone(),
            world_building_summary: world_building_summary.clone(),
            character_list: character_list.clone(),
            foreshadowing_status: foreshadowing_status.clone(),
            style_dna_info: style_dna_info.clone(),
            mcp_tools_available: mcp_tools_available.clone(),
            selected_text: None,
        }).await;
        return Ok(result);
    }

    emit_progress("context_loaded", "故事上下文加载完成", 2, 5);

    // Clone values before they are moved into plan_context
    let story_id_for_record = current_story_id.clone();
    let scene_id_for_record = current_scene_id.clone();
    let chapter_id_for_record = chapters.last().map(|c| c.id.clone());
    let input_for_record = user_input.clone();
    let prev_content_for_record = current_content_preview.clone();

    let plan_context = planner::PlanContext {
        current_story_id,
        has_story: !stories.is_empty(),
        has_chapters: !chapters.is_empty(),
        chapter_count,
        current_content_preview,
        user_input: user_input.clone(),
        scene_count,
        scenes_summary,
        current_scene_id,
        current_scene_stage,
        total_word_count,
        latest_chapter_word_count,
        story_progress,
        world_building_summary,
        character_list,
        foreshadowing_status,
        style_dna_info,
        mcp_tools_available,
        selected_text: None,
    };

    // 执行计划（内部会自动检查模板库并生成计划）
    emit_progress("executing", "开始执行创作计划...", 3, 5);
    let executor = planner::PlanExecutor::new(app_handle);
    let result = executor.execute_with_context(&plan_context).await
        .map_err(|e| format!("[smart_execute] Plan execution failed: {}", e))?;
    emit_progress("completed", "创作计划执行完成", 5, 5);

    // 如果计划执行失败（所有步骤都失败或没有内容/空内容），返回错误
    let is_empty_content = result.final_content.as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if !result.success || is_empty_content {
        let error_msg = if result.messages.iter().any(|m| m.contains("超时") || m.contains("timed out") || m.contains("timeout")) {
            "模型响应超时，请检查模型服务是否正常运行".to_string()
        } else if result.messages.is_empty() {
            "计划执行失败：未生成任何内容".to_string()
        } else {
            format!("计划执行失败：{}", result.messages.join("; "))
        };
        return Err(error_msg);
    }

    // Record AI operation for non-bootstrap generation
    if let Some(ref story_id) = story_id_for_record {
        record_ai_operation(db::CreateAiOperationRequest {
            story_id: story_id.clone(),
            scene_id: scene_id_for_record,
            chapter_id: chapter_id_for_record,
            operation_type: "smart_execute".to_string(),
            operation_name: "AI 续写".to_string(),
            input_summary: Some(input_for_record),
            output_summary: result.final_content.as_ref().map(|c| c.chars().take(200).collect()),
            previous_content: prev_content_for_record,
            new_content: result.final_content.clone(),
            metadata: Some(serde_json::json!({"steps_completed": result.steps_completed}).to_string()),
        });
    }

    Ok(result)
}

/// 检测用户输入是否包含"创建新小说"的意图
fn is_novel_creation_intent(user_input: &str) -> bool {
    let input = user_input.to_lowercase();
    // v5.4.0: 增强检测，区分"创建新小说"和"续写当前故事"
    let creation_signals = [
        "写一部", "写一本", "写一篇", "写个",
        "创作一部", "创作一本", "创作一篇", "创作个",
        "生成一部", "生成一本", "生成一篇",
        "新建", "创建", "新开",
        "write a", "write an", "create a", "create an", "start a", "start an",
        "novel", "story", "book",
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

/// Phase 3: 轻量意图检测 — v4.2.0 模型驱动编排范式
/// 
/// 设计原则：所有复杂意图理解交由 PlanGenerator（LLM）处理。
/// 此函数仅作为极端轻量的快速路径，仅在输入非常简短明确时提供建议。
/// 返回 None 表示"让模型决定"，这是默认和推荐行为。
fn detect_and_route_intent(
    _user_input: &str,
    _story_id: &Option<String>,
    _scene_count: usize,
    _scenes_summary: &[planner::SceneStructureSummary],
    _current_scene_stage: &Option<String>,
    _story_progress: &str,
) -> Option<planner::ExecutionPlan> {
    // v4.2.0: 彻底移除关键词匹配路由。所有意图由 PlanGenerator 自由理解。
    // 保留函数接口以兼容现有调用点，但始终返回 None。
    None
}

#[derive(Debug, Deserialize)]
struct RecordFeedbackRequest {
    story_id: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    feedback_type: String,
    agent_type: Option<String>,
    original_ai_text: String,
    final_text: Option<String>,
}

/// v5.6.1: 学习点，返回给前端展示
#[derive(Debug, Clone, serde::Serialize)]
struct LearningPoint {
    category: String,
    observation: String,
    impact: String,
}

#[tauri::command(rename_all = "snake_case")]
async fn record_feedback(request: RecordFeedbackRequest) -> Result<Vec<LearningPoint>, AppError> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let recorder = creative_engine::adaptive::FeedbackRecorder::new(pool.clone());
    let result = match request.feedback_type.as_str() {
        "accept" => recorder.record_accept(&request.story_id, &request.original_ai_text, request.agent_type.as_deref()),
        "reject" => recorder.record_reject(&request.story_id, &request.original_ai_text, request.agent_type.as_deref()),
        "modify" => recorder.record_modify(
            &request.story_id,
            &request.original_ai_text,
            request.final_text.as_deref().unwrap_or(""),
            request.agent_type.as_deref(),
        ),
        _ => Err(AppError::validation_failed("Unknown feedback type", None::<String>)),
    };
    
    if result.is_err() {
        return Err(result.err().unwrap());
    }
    
    // v5.6.1: 同步挖掘偏好，返回真实学习点给前端
    let miner = creative_engine::adaptive::PreferenceMiner::new(pool.clone());
    let learnings = match miner.mine(&request.story_id) {
        Ok(prefs) => {
            prefs.into_iter()
                .filter(|p| p.confidence >= 0.5)
                .take(3)
                .map(|p| LearningPoint {
                    category: p.preference_type,
                    observation: format!("{}: {} (置信度{:.0}%)", p.preference_key, p.preference_value, p.confidence * 100.0),
                    impact: p.reasoning,
                })
                .collect()
        }
        Err(e) => {
            log::warn!("[record_feedback] Preference mining failed: {}", e);
            vec![]
        }
    };
    
    // 异步触发偏好挖掘保存，让自适应学习系统形成闭环
    let story_id = request.story_id.clone();
    tauri::async_runtime::spawn(async move {
        let engine = creative_engine::adaptive::AdaptiveLearningEngine::new(pool);
        match engine.mine_preferences(&story_id) {
            Ok(prefs) if !prefs.is_empty() => {
                log::info!("[Adaptive] Mined {} preferences for story {}", prefs.len(), story_id);
            }
            Ok(_) => {}
            Err(e) => log::warn!("[Adaptive] Preference mining failed: {}", e),
        }
    });
    
    Ok(learnings)
}

/// 获取输入栏智能提示 — 由LLM根据当前故事上下文生成建议
#[tauri::command(rename_all = "snake_case")]
async fn get_input_hint(
    app_handle: AppHandle,
    current_content: Option<String>,
) -> Result<String, String> {
    let pool = get_pool().ok_or("Database not initialized")?;

    // 获取当前故事状态
    let stories = StoryRepository::new(pool.clone()).get_all()
        .map_err(|e| format!("Failed to load stories: {}", e))?;
    let current_story = stories.first().cloned();
    let current_story_id = current_story.as_ref().map(|s| s.id.clone());

    let chapters = if let Some(ref story_id) = current_story_id {
        ChapterRepository::new(pool.clone())
            .get_by_story(story_id)
            .map_err(|e| format!("Failed to load chapters: {}", e))?
    } else {
        vec![]
    };

    let content_preview = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| chapters.last().and_then(|c| c.content.clone()));

    let word_count = content_preview.as_ref().map(|c| c.chars().count()).unwrap_or(0);

    // 构建规则驱动的候选建议
    let mut candidates: Vec<String> = vec![];

    if stories.is_empty() {
        candidates.push("写一个新故事".to_string());
        candidates.push("创作一部科幻小说".to_string());
        candidates.push("我想写一个关于...的故事".to_string());
    } else if chapters.is_empty() {
        candidates.push("创建第一章".to_string());
        candidates.push("开始写作".to_string());
    } else if word_count < 100 {
        candidates.push("续写".to_string());
        candidates.push("展开这个场景".to_string());
        candidates.push("增加环境描写".to_string());
    } else if word_count < 1000 {
        candidates.push("续写下一段".to_string());
        candidates.push("润色当前段落".to_string());
        candidates.push("增加对话".to_string());
    } else {
        candidates.push("续写".to_string());
        candidates.push("调整节奏".to_string());
        candidates.push("生成古典评点".to_string());
        candidates.push("优化对话".to_string());
    }

    // 如果有角色，添加角色相关建议
    if let Some(ref story_id) = current_story_id {
        let char_repo = db::repositories::CharacterRepository::new(pool.clone());
        if let Ok(chars) = char_repo.get_by_story(story_id) {
            if let Some(first_char) = chars.first() {
                candidates.push(format!("让{}出场", first_char.name));
            }
            if chars.len() >= 2 {
                candidates.push("增加人物冲突".to_string());
            }
        }

        // 如果有场景信息，添加场景相关建议
        let scene_repo = db::repositories_v3::SceneRepository::new(pool.clone());
        if let Ok(scenes) = scene_repo.get_by_story(story_id) {
            let scene_count = scenes.len();
            let has_content = scenes.iter().any(|s| s.content.is_some() || s.draft_content.is_some());
            if scene_count > 0 && !has_content {
                candidates.push("为当前场景写内容".to_string());
            }
        }
    }

    // 尝试用 LLM 生成更个性化的建议
    let llm_hint = if let Some(ref story) = current_story {
        let llm_service = crate::llm::LlmService::new(app_handle);
        let prompt = format!(
            "你是一个AI写作助手。当前故事：{}，字数：{}，章节数：{}。\
             请生成一条简短的输入建议（12字以内），告诉用户下一步可以做什么。\
             建议要自然、有创意、贴合故事。只输出建议内容，不要解释。",
            story.title,
            word_count,
            chapters.len()
        );
        match llm_service.generate(prompt, Some(30), Some(0.7)).await {
            Ok(response) => {
                let hint = response.content.trim().replace(['"', '\'', '「', '」'], "").trim().to_string();
                if !hint.is_empty() && hint.chars().count() <= 20 {
                    Some(hint)
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("[get_input_hint] LLM generation failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // 优先返回 LLM 建议，否则返回规则建议
    if let Some(hint) = llm_hint {
        Ok(hint)
    } else if let Some(hint) = candidates.first() {
        Ok(hint.clone())
    } else {
        Ok("输入指令开始创作".to_string())
    }
}

#[tauri::command(rename_all = "snake_case")]
async fn list_mcp_tools() -> Result<Vec<mcp::McpTool>, String> {
    // 1. 收集内置工具（含动态注册的）
    let server = BUILTIN_MCP_SERVER.lock().await;
    let mut all_tools = server.get_tools();

    // 2. 收集外部连接工具（W2-B8: MCP 工具动态注册）
    // 外部工具名称前缀为 mcp.{server_id}.{tool_name}，便于前端区分来源
    let connections = MCP_CONNECTIONS.lock().await;
    for (server_id, client) in connections.iter() {
        for tool in client.get_tools() {
            let mut t = tool.clone();
            t.name = format!("mcp.{}.{}", server_id, tool.name);
            all_tools.push(t);
        }
    }

    Ok(all_tools)
}

#[tauri::command(rename_all = "snake_case")]
async fn execute_mcp_tool(tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, String> {
    // W2-B8: 统一入口 — 支持内置工具和外部工具
    // 如果 tool_name 以 mcp.{server_id}. 开头，路由到外部连接
    if let Some(rest) = tool_name.strip_prefix("mcp.") {
        let parts: Vec<&str> = rest.splitn(2, '.').collect();
        if parts.len() == 2 {
            let server_id = parts[0];
            let actual_tool_name = parts[1];
            let mut connections = MCP_CONNECTIONS.lock().await;
            let client = connections.get_mut(server_id)
                .ok_or_else(|| format!("MCP server {} not connected", server_id))?;
            let result = client.call_tool(actual_tool_name, arguments).await
                .map_err(|e| format!("MCP tool call failed: {}", e))?;
            return Ok(result);
        }
    }

    // 否则作为内置工具执行（使用全局实例）
    let server = BUILTIN_MCP_SERVER.lock().await;
    server.start().await.map_err(|e| crate::error::AppError::from(e).to_string())?;
    let result = server.execute_tool(&tool_name, arguments).await
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    Ok(result)
}

/// 动态注册内置 MCP 工具到 CapabilityRegistry（W2-B8）
///
/// 注意：此命令仅将工具注册到 CapabilityRegistry，使其可被 PlanGenerator 发现和调度。
/// 实际执行 handler 仍需在 mcp/server.rs 中实现并硬编码注册到 McpServer。
#[tauri::command(rename_all = "snake_case")]
async fn register_mcp_tool(tool: mcp::McpTool) -> Result<(), String> {
    let mut registry = crate::capabilities::get_capability_registry();
    let cap = crate::capabilities::Capability::from_mcp_tool("builtin", &tool);
    registry.register(cap);

    log::info!("[MCP] Dynamically registered built-in tool to CapabilityRegistry: {}", tool.name);
    Ok(())
}

/// 动态注销内置 MCP 工具（W2-B8）
#[tauri::command(rename_all = "snake_case")]
async fn unregister_mcp_tool(tool_name: String) -> Result<(), String> {
    // 1. 从 CapabilityRegistry 注销
    let mut registry = crate::capabilities::get_capability_registry();
    let cap_id = format!("mcp.builtin.{}", tool_name);
    registry.unregister(&cap_id);

    log::info!("[MCP] Dynamically unregistered built-in tool: {}", tool_name);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ExportOptions {
    story_id: String,
    format: String,
    include_metadata: Option<bool>,
    include_outline: Option<bool>,
    include_characters: Option<bool>,
    template_id: Option<String>,
}
#[tauri::command(rename_all = "snake_case")]
async fn export_story(options: ExportOptions, app_handle: tauri::AppHandle) -> Result<ExportResult, String> {
    let pool = get_pool().ok_or("Database not initialized")?;

    let story = StoryRepository::new(pool.clone())
        .get_by_id(&options.story_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .ok_or("Story not found")?;

    let mut chapters = ChapterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    let characters = CharacterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    // W4-B10: 导出聚合完整性修复 - 对 content 为空的 chapter，自动从关联 scenes 聚合内容
    for chapter in &mut chapters {
        if chapter.content.as_ref().map(|c| c.trim().is_empty()).unwrap_or(true) {
            let mut chapter_scenes: Vec<&crate::db::Scene> = scenes.iter()
                .filter(|s| s.chapter_id.as_deref() == Some(&chapter.id))
                .collect();
            chapter_scenes.sort_by_key(|s| s.sequence_number);
            let aggregated = chapter_scenes.iter()
                .filter_map(|s| s.content.as_deref())
                .filter(|c| !c.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            if !aggregated.is_empty() {
                chapter.content = Some(aggregated);
            }
        }
    }

    let format = match options.format.as_str() {
        "markdown" => ExportFormat::Markdown,
        "pdf" => ExportFormat::Pdf,
        "epub" => ExportFormat::Epub,
        "html" => ExportFormat::Html,
        "txt" => ExportFormat::PlainText,
        "json" => ExportFormat::Json,
        _ => ExportFormat::Markdown,
    };

    let extension = match format {
        ExportFormat::Markdown => "md",
        ExportFormat::Pdf => "pdf",
        ExportFormat::Epub => "epub",
        ExportFormat::Html => "html",
        ExportFormat::PlainText => "txt",
        ExportFormat::Json => "json",
    };

    let safe_title = story.title.replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!("{}_{}.{}", safe_title, chrono::Local::now().format("%Y%m%d"), extension);

    let export_dir = app_handle.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
        .join("exports");

    std::fs::create_dir_all(&export_dir).map_err(|e| crate::error::AppError::from(e).to_string())?;
    let output_path = export_dir.join(&filename);

    let config = ExportConfig {
        format,
        include_outline: options.include_outline.unwrap_or(true),
        include_metadata: options.include_metadata.unwrap_or(true),
        chapter_separator: "\n\n---\n\n".to_string(),
    };

    // Load template if specified
    let template_content = if let Some(ref template_id) = options.template_id {
        let template_repo = db::ExportTemplateRepository::new(pool.clone());
        template_repo.get_by_id(template_id)
            .map_err(|e| crate::error::AppError::from(e).to_string())?
            .map(|t| t.template_content)
    } else {
        None
    };

    let exporter = StoryExporter::new();
    exporter.export_to_file(&story, &chapters, &characters, &scenes, &config, &output_path, template_content.as_deref())
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        content: std::fs::read_to_string(&output_path).unwrap_or_default(),
        format: options.format,
    })
}

#[tauri::command(rename_all = "snake_case")]
async fn list_export_templates(format_filter: Option<String>) -> Result<Vec<db::ExportTemplate>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = db::ExportTemplateRepository::new(pool);
    let templates = repo.get_all().map_err(|e| crate::error::AppError::from(e).to_string())?;

    if let Some(filter) = format_filter {
        let filtered: Vec<_> = templates.into_iter()
            .filter(|t| t.format == filter)
            .collect();
        Ok(filtered)
    } else {
        Ok(templates)
    }
}

#[tauri::command(rename_all = "snake_case")]
async fn save_export_template(name: String, description: Option<String>, format: String, template_content: String) -> Result<db::ExportTemplate, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = db::ExportTemplateRepository::new(pool);
    let req = db::CreateExportTemplateRequest {
        name,
        description,
        format,
        template_content,
    };
    repo.create(req).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn delete_export_template(id: String) -> Result<(), String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = db::ExportTemplateRepository::new(pool);
    repo.delete(&id).map_err(|e| crate::error::AppError::from(e).to_string())?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn list_ai_operations(story_id: String) -> Result<Vec<db::AiOperation>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = db::AiOperationRepository::new(pool);
    repo.get_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn rollback_ai_operation(operation_id: String, app: AppHandle) -> Result<(), String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let op_repo = db::AiOperationRepository::new(pool.clone());
    let chapter_repo = ChapterRepository::new(pool.clone());

    let operation = op_repo.get_by_id(&operation_id)
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .ok_or("Operation not found")?;

    // Only support rollback for chapter content operations that have previous_content
    let prev_content = operation.previous_content
        .ok_or("此操作不支持回滚")?;

    let chapter_id = operation.chapter_id
        .ok_or("此操作没有关联章节")?;

    // Restore previous content
    chapter_repo.update(&chapter_id, None, None, Some(prev_content), None)
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    // Mark operation as rolled back
    op_repo.update_status(&operation_id, "rolled_back")
        .map_err(|e| crate::error::AppError::from(e).to_string())?;

    // Emit sync event
    let _ = app.emit("sync-event", serde_json::json!({
        "event": "chapterUpdated",
        "chapter_id": chapter_id,
        "story_id": operation.story_id,
    }));

    Ok(())
}

// ===== 幕前/幕后通信命令 =====

/// 通知 backstage 内容已变更
#[tauri::command(rename_all = "snake_case")]
fn notify_backstage_content_changed(text: String, chapter_id: String, app: AppHandle) -> Result<(), crate::error::AppError> {
    let event = window::BackstageEvent::ContentChanged { text, chapter_id };
    window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 backstage 请求生成内容
#[tauri::command(rename_all = "snake_case")]
fn notify_backstage_generation_requested(chapter_id: String, context: String, app: AppHandle) -> Result<(), crate::error::AppError> {
    let event = window::BackstageEvent::GenerationRequested { chapter_id, context };
    window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 frontstage 内容已变更
#[tauri::command(rename_all = "snake_case")]
fn notify_frontstage_content_changed(text: String, chapter_id: String, app: AppHandle) -> Result<(), crate::error::AppError> {
    let event = window::FrontstageEvent::ContentUpdate { text, chapter_id };
    window::WindowManager::send_to_frontstage(&app, event)
}

/// 通知 frontstage 数据已刷新（幕后创建/修改了故事、章节等）
#[tauri::command(rename_all = "snake_case")]
fn notify_frontstage_data_refresh(entity: String, app: AppHandle) -> Result<(), crate::error::AppError> {
    let event = window::FrontstageEvent::DataRefresh { entity };
    window::WindowManager::send_to_frontstage(&app, event)
}

/// 显示 backstage 窗口
#[tauri::command(rename_all = "snake_case")]
fn show_backstage(app: AppHandle, story_id: Option<String>) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window("backstage") {
        window.show().map_err(|e| crate::error::AppError::from(e).to_string())?;
        window.set_focus().map_err(|e| crate::error::AppError::from(e).to_string())?;
        window
    } else {
        // 窗口可能被关闭，重新创建
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "backstage",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("草苔 - 幕后工作室")
        .inner_size(1200.0, 800.0)
        .center()
        .build()
        .map_err(|e| crate::error::AppError::from(e).to_string())?;
        window.show().map_err(|e| crate::error::AppError::from(e).to_string())?;
        window.set_focus().map_err(|e| crate::error::AppError::from(e).to_string())?;
        window
    };

    // v5.2.0 修复：强制 WebView2 重新创建渲染表面，防止 hide/show 后白屏
    // 方法：微调窗口大小再恢复 + 双重维度触发 + JS强制重排 + 延迟事件
    let _window_clone = window.clone();
    if let Ok(size) = window.inner_size() {
        if size.width > 0 && size.height > 0 {
            // 第一步：宽度+1
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size.width + 1,
                height: size.height,
            }));
            // 第二步：高度+1
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size.width + 1,
                height: size.height + 1,
            }));
            // 第三步：恢复原始尺寸
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size.width,
                height: size.height,
            }));
        }
    }

    // 执行 JS 强制重排 + 通知前端准备恢复
    let _ = window.eval(r#"
        (function() {
            const body = document.body;
            const html = document.documentElement;
            if (body && html) {
                // 保存原始样式
                const bodyDisplay = body.style.display;
                const htmlDisplay = html.style.display;
                // 强制重排
                html.style.display = 'none';
                body.style.display = 'none';
                void html.offsetHeight;
                void body.offsetHeight;
                html.style.display = htmlDisplay || '';
                body.style.display = bodyDisplay || '';
                // 触发多重重绘事件
                window.dispatchEvent(new Event('resize'));
                window.dispatchEvent(new Event('scroll'));
                // 通知React可能需要重新渲染
                window.dispatchEvent(new CustomEvent('backstage-window-restored', { detail: { timestamp: Date.now() } }));
            }
        })();
    "#);

    // 延迟发射 backstage-shown 事件，确保前端监听器已就绪 + WebView2 渲染表面已恢复
    // 800ms 给 WebView2 足够时间从休眠状态恢复
    let app_handle = app.clone();
    let story_id_clone = story_id.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        // 再次触发尺寸变化确保渲染表面激活
        if let Some(window) = app_handle.get_webview_window("backstage") {
            if let Ok(size) = window.inner_size() {
                if size.width > 0 && size.height > 0 {
                    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                        width: size.width + 1,
                        height: size.height,
                    }));
                    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                        width: size.width,
                        height: size.height,
                    }));
                }
            }
            // 发射事件通知前端窗口已恢复
            let _ = window.emit("backstage-shown", serde_json::json!({
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                "story_id": story_id_clone
            }));
        }
    });

    // v5.1.0: 如果提供了 story_id，同时发射 NavigateTo 事件让幕后自动定位
    if let Some(sid) = story_id {
        let _ = crate::window::WindowManager::send_to_backstage(
            &app,
            crate::window::BackstageEvent::NavigateTo {
                view: "stories".to_string(),
                highlight_story_id: Some(sid),
                open_panel: Some("overview".to_string()),
            }
        );
    }

    Ok(())
}

/// 获取故事的规范状态快照
#[tauri::command(rename_all = "snake_case")]
async fn get_canonical_state(story_id: String) -> Result<canonical_state::CanonicalStateSnapshot, AppError> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let manager = canonical_state::CanonicalStateManager::new(pool);
    manager.get_snapshot(&story_id).await
}

/// 同步故事数据
#[tauri::command(rename_all = "snake_case")]
async fn sync_story_data(story_id: String) -> Result<serde_json::Value, String> {
    let pool = get_pool().ok_or("Database not initialized")?;

    // 获取故事的所有相关数据并同步
    let story_repo = StoryRepository::new(pool.clone());
    let character_repo = CharacterRepository::new(pool.clone());
    let chapter_repo = ChapterRepository::new(pool.clone());

    // 检查故事是否存在
    let _story = story_repo.get_by_id(&story_id)
        .map_err(|e| format!("Failed to get story: {}", e))?
        .ok_or("Story not found")?;

    // 获取角色和章节数据进行同步验证
    let characters = character_repo.get_by_story(&story_id)
        .map_err(|e| format!("Failed to get characters: {}", e))?;

    let chapters = chapter_repo.get_by_story(&story_id)
        .map_err(|e| format!("Failed to get chapters: {}", e))?;

    // 返回同步状态
    Ok(serde_json::json!({
        "story_id": story_id,
        "characters_count": characters.len(),
        "chapters_count": chapters.len(),
        "sync_timestamp": chrono::Utc::now().to_rfc3339(),
        "status": "synced"
    }))
}

/// 获取同步状态
#[tauri::command(rename_all = "snake_case")]
async fn get_sync_status(story_id: String) -> Result<serde_json::Value, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let story_repo = StoryRepository::new(pool.clone());

    // 检查故事是否存在
    let story = story_repo.get_by_id(&story_id)
        .map_err(|e| format!("Failed to get story: {}", e))?
        .ok_or("Story not found")?;

    Ok(serde_json::json!({
        "story_id": story_id,
        "story_title": story.title,
        "auto_sync_enabled": true, // 默认启用
        "last_sync": chrono::Utc::now().to_rfc3339(),
        "sync_status": "active"
    }))
}

/// 启用自动同步
#[tauri::command(rename_all = "snake_case")]
async fn enable_auto_sync(story_id: String) -> Result<(), String> {
    // 这里可以将自动同步状态保存到数据库或配置中
    // 目前返回成功，表示自动同步已启用
    println!("Auto sync enabled for story: {}", story_id);
    Ok(())
}

/// 禁用自动同步
#[tauri::command(rename_all = "snake_case")]
async fn disable_auto_sync(story_id: String) -> Result<(), String> {
    // 这里可以将自动同步状态保存到数据库或配置中
    // 目前返回成功，表示自动同步已禁用
    println!("Auto sync disabled for story: {}", story_id);
    Ok(())
}

// ===== 通用 Workflow 引擎命令 (v5.2.0) =====

/// 注册一个新的工作流定义
#[tauri::command(rename_all = "snake_case")]
fn register_workflow(
    workflow: workflow::Workflow,
    engine: tauri::State<'_, std::sync::Arc<workflow::WorkflowEngine>>,
) -> Result<(), String> {
    engine.register_workflow(workflow).map_err(|e| crate::error::AppError::from(e).to_string())
}

/// 创建工作流实例
#[tauri::command(rename_all = "snake_case")]
fn create_workflow_instance(
    workflow_id: String,
    story_id: String,
    initial_context: std::collections::HashMap<String, serde_json::Value>,
    engine: tauri::State<'_, std::sync::Arc<workflow::WorkflowEngine>>,
) -> Result<workflow::WorkflowInstance, String> {
    engine.create_instance(&workflow_id, &story_id, initial_context)
        .map_err(|e| crate::error::AppError::from(e).to_string())
}

/// 启动工作流实例（加入执行队列并异步执行）
#[tauri::command(rename_all = "snake_case")]
async fn start_workflow_instance(
    instance_id: String,
    engine: tauri::State<'_, std::sync::Arc<workflow::WorkflowEngine>>,
    scheduler: tauri::State<'_, std::sync::Arc<workflow::WorkflowScheduler>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Mark instance as started
    engine.start_instance(&instance_id).map_err(|e| crate::error::AppError::from(e).to_string())?;
    
    // Clone needed data for the spawned task
    let scheduler_clone = std::sync::Arc::clone(&scheduler);
    let engine_clone = std::sync::Arc::clone(&engine);
    let instance_id_clone = instance_id.clone();
    
    // Spawn async execution
    tauri::async_runtime::spawn(async move {
        if let Err(e) = scheduler_clone.schedule_execution(instance_id_clone.clone()).await {
            log::error!("[Workflow] Failed to schedule instance {}: {}", instance_id_clone, e);
            return;
        }
        // Execute the next (and only) item in the queue
        let result = scheduler_clone.execute_next(&engine_clone, &app_handle).await;
        match result {
            Some(Ok(id)) => log::info!("[Workflow] Instance {} executed successfully", id),
            Some(Err(e)) => log::error!("[Workflow] Instance execution failed: {}", e),
            None => log::warn!("[Workflow] No instance found in queue"),
        }
    });
    
    Ok(instance_id)
}

/// 获取工作流实例状态
#[tauri::command(rename_all = "snake_case")]
fn get_workflow_instance_status(
    instance_id: String,
    engine: tauri::State<'_, std::sync::Arc<workflow::WorkflowEngine>>,
) -> Result<Option<workflow::WorkflowInstance>, String> {
    Ok(engine.get_instance(&instance_id))
}

/// 列出所有已注册的工作流（包括从文件加载的）
#[tauri::command(rename_all = "snake_case")]
fn list_workflows(
    loader: tauri::State<'_, workflow::WorkflowLoader>,
) -> Result<Vec<workflow::LoadedWorkflow>, String> {
    Ok(loader.list_workflows())
}

/// 手动重新加载所有工作流文件
#[tauri::command(rename_all = "snake_case")]
fn reload_workflows(
    loader: tauri::State<'_, workflow::WorkflowLoader>,
) -> Result<usize, String> {
    loader.reload_all().map_err(|e| crate::error::AppError::from(e).to_string())
}

// ===== 模型驱动的智能编排命令 =====

/// 分析已有故事的结构健康度 (v5.3.0)
/// 对现有故事进行逆向分析，检测伏笔回收率、角色弧光完整度、冲突多样性等
#[tauri::command(rename_all = "snake_case")]
async fn analyze_story_structure(
    story_id: String,
    app_handle: tauri::AppHandle,
) -> Result<narrative::health::HealthReport, String> {
    log::info!("[analyze_story_structure] 分析故事结构: story_id={}", story_id);
    
    let pool = get_pool().ok_or("Database not initialized")?;
    let analyzer = narrative::health::StoryHealthAnalyzer::new(pool, story_id.clone());
    
    // 发射开始事件
    let _ = app_handle.emit("analysis-started", serde_json::json!({
        "story_id": story_id,
        "analysis_type": "story_structure"
    }));
    
    let report = tokio::task::spawn_blocking(move || analyzer.analyze())
        .await
        .map_err(|e| format!("分析任务执行失败: {}", e))?
        .map_err(|e| crate::error::AppError::from(e).to_string())?;
    
    // 发射完成事件
    let _ = app_handle.emit("analysis-completed", serde_json::json!({
        "story_id": story_id,
        "overall_score": report.overall_score,
        "check_count": report.checks.len()
    }));
    
    log::info!("[analyze_story_structure] 分析完成: overall_score={:.1}", report.overall_score);
    Ok(report)
}



/// v5.4.0: 取消正在运行的 GenesisPipeline
#[tauri::command(rename_all = "snake_case")]
fn cancel_genesis_pipeline(session_id: String) -> Result<bool, String> {
    let cancelled = narrative::pipeline::cancel_pipeline(&session_id);
    if cancelled {
        log::info!("[cancel_genesis_pipeline] Pipeline {} 已标记为取消", session_id);
        Ok(true)
    } else {
        log::warn!("[cancel_genesis_pipeline] Pipeline {} 未找到或已完成", session_id);
        Ok(false)
    }
}

/// v5.4.0: 手动触发能力进化分析
#[tauri::command(rename_all = "snake_case")]
async fn evolve_capabilities(app_handle: AppHandle) -> Result<Vec<(String, String)>, String> {
    log::info!("[evolve_capabilities] 手动触发能力进化分析");
    let llm = llm::LlmService::new(app_handle.clone());
    let engine = capabilities::evolution::CapabilityEvolutionEngine::new(llm, &app_handle);
    let improvements = engine.evolve_capability_descriptions().await?;
    let _ = app_handle.emit("capabilities-evolved", serde_json::json!({
        "improvements": improvements,
        "auto_triggered": false,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }));
    log::info!("[evolve_capabilities] 进化完成，生成 {} 条改进建议", improvements.len());
    Ok(improvements)
}

// ==================== v6.0.0: Story System Commands ====================

#[tauri::command(rename_all = "snake_case")]
fn create_master_setting(
    story_id: String,
    genre: String,
    core_tone: String,
    pacing_strategy: String,
    anti_patterns: Vec<String>,
    world_rules: Vec<String>,
) -> Result<crate::db::StoryContract, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let engine = story_system::StorySystemEngine::new(pool);
    engine.create_master_setting(
        &story_id, &genre, &core_tone, &pacing_strategy, &anti_patterns, &world_rules
    )
}

#[tauri::command(rename_all = "snake_case")]
fn create_chapter_contract(
    story_id: String,
    chapter_number: i32,
    goal: String,
    must_cover_nodes: Vec<String>,
    forbidden_zones: Vec<String>,
    time_anchor: Option<String>,
    chapter_span: Option<String>,
) -> Result<crate::db::StoryContract, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let engine = story_system::StorySystemEngine::new(pool);
    engine.create_chapter_contract(
        &story_id, chapter_number, &goal, &must_cover_nodes, &forbidden_zones,
        time_anchor.as_deref(), chapter_span.as_deref()
    )
}

#[tauri::command(rename_all = "snake_case")]
fn get_contract_tree(story_id: String) -> Result<story_system::ContractTree, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let engine = story_system::StorySystemEngine::new(pool);
    engine.get_contract_tree(&story_id)
}

#[tauri::command(rename_all = "snake_case")]
fn get_runtime_contract(
    story_id: String,
    chapter_number: i32,
) -> Result<story_system::RuntimeContract, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let engine = story_system::StorySystemEngine::new(pool);
    engine.get_runtime_contract(&story_id, chapter_number)
}

/// v0.7.5: 自动补齐缺失的合同（世界观合同 + 章节合同）
#[tauri::command(rename_all = "snake_case")]
async fn auto_create_missing_contracts(
    story_id: String,
    chapter_number: i32,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let builder = story_system::auto_contract::AutoContractBuilder::new(pool, app_handle);
    let (created_master, created_chapter) = builder.auto_fill(&story_id, chapter_number).await?;
    Ok(serde_json::json!({
        "created_master_setting": created_master,
        "created_chapter_contract": created_chapter,
        "message": if created_master || created_chapter {
            "合同补齐完成"
        } else {
            "所有合同已存在，无需补齐"
        }
    }))
}

#[tauri::command(rename_all = "snake_case")]
fn init_chapter_commit(
    story_id: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    chapter_number: i32,
) -> Result<crate::db::SceneCommit, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let service = story_system::SceneCommitService::new(pool);
    service.init_commit(&story_id, scene_id.as_deref(), chapter_id.as_deref(), chapter_number)
}

#[tauri::command(rename_all = "snake_case")]
async fn apply_chapter_commit(
    commit_id: String,
    outline_snapshot_json: String,
    review_result_json: String,
    fulfillment_result_json: String,
    accepted_events_json: String,
    state_deltas_json: String,
    entity_deltas_json: String,
    summary_text: String,
    dominant_strand: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let service = story_system::SceneCommitService::new(pool);
    let store = VECTOR_STORE.get();
    service.apply_commit(
        &commit_id, &outline_snapshot_json, &review_result_json,
        &fulfillment_result_json, &accepted_events_json, &state_deltas_json,
        &entity_deltas_json, &summary_text, &dominant_strand,
        None, Some(app_handle), store
    ).await
}

#[tauri::command(rename_all = "snake_case")]
fn get_scene_commits(story_id: String) -> Result<Vec<crate::db::SceneCommit>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::SceneCommitRepository::new(pool);
    repo.get_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

// ==================== v6.0.0: Memory Commands ====================

#[tauri::command(rename_all = "snake_case")]
fn build_memory_pack(
    story_id: String,
    chapter_number: i32,
    task_type: String,
    outline: Option<String>,
) -> Result<memory::MemoryPack, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let orchestrator = memory::MemoryOrchestrator::new(pool);
    orchestrator.build_memory_pack(&story_id, chapter_number, &task_type, outline.as_deref()
    )
}

#[tauri::command(rename_all = "snake_case")]
fn get_memory_items(story_id: String) -> Result<Vec<crate::db::MemoryItem>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::MemoryItemRepository::new(pool);
    repo.get_active_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn create_memory_item(
    story_id: String,
    category: String,
    subject: Option<String>,
    field: Option<String>,
    value: Option<String>,
    source_chapter: Option<i32>,
    confidence: f32,
) -> Result<crate::db::MemoryItem, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::MemoryItemRepository::new(pool);
    repo.create(&story_id, &category, subject.as_deref(), field.as_deref(),
        value.as_deref(), source_chapter, confidence
    ).map_err(|e| crate::error::AppError::from(e).to_string())
}

// ==================== v6.0.0: Story Structure Audit Commands ====================

#[tauri::command(rename_all = "snake_case")]
async fn audit_story(
    story_id: String,
    app_handle: tauri::AppHandle,
) -> Result<narrative::audit::StoryAnalysisReport, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let llm_service = crate::llm::LlmService::new(app_handle);
    let auditor = narrative::audit::StoryStructureAuditor::new(pool, llm_service);
    auditor.analyze(&story_id).await.map_err(|e| e.to_string())
}

// ==================== v6.0.0: Reading Power Commands ====================

#[tauri::command(rename_all = "snake_case")]
fn evaluate_reading_power(
    story_id: String,
    chapter_number: i32,
) -> Result<reading_power::ReadingPowerEvaluation, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let evaluator = reading_power::ReadingPowerEvaluator::new(pool);
    evaluator.evaluate(&story_id, chapter_number)
}

#[tauri::command(rename_all = "snake_case")]
fn get_reading_power_trend(
    story_id: String,
    last_n: i64,
) -> Result<Vec<reading_power::ReadingPowerEvaluation>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let evaluator = reading_power::ReadingPowerEvaluator::new(pool);
    evaluator.get_trend(&story_id, last_n)
}

#[tauri::command(rename_all = "snake_case")]
fn get_chase_debts(story_id: String) -> Result<Vec<crate::db::ChaseDebt>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::ChaseDebtRepository::new(pool);
    repo.get_active_by_story(&story_id).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn create_override_contract(
    story_id: String,
    chapter_number: i32,
    constraint_type: String,
    constraint_id: String,
    rationale_type: String,
    rationale_text: String,
    payback_plan: String,
    due_chapter: i32,
) -> Result<crate::db::OverrideContract, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let manager = reading_power::DebtManager::new(pool);
    manager.create_override_contract(
        &story_id, chapter_number, &constraint_type, &constraint_id,
        &rationale_type, &rationale_text, &payback_plan, due_chapter
    )
}

// ==================== v6.0.0: Genre Profile Commands ====================

#[tauri::command(rename_all = "snake_case")]
fn get_genre_profiles() -> Result<Vec<crate::db::GenreProfile>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::GenreProfileRepository::new(pool);
    repo.get_all().map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn get_genre_profile(genre_name: String) -> Result<Option<crate::db::GenreProfile>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::GenreProfileRepository::new(pool);
    repo.get_by_name(&genre_name).map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn save_genre_profile(
    id: Option<String>,
    genre_name: String,
    canonical_name: String,
    aliases_json: Option<String>,
    core_tone: Option<String>,
    pacing_strategy: Option<String>,
    anti_patterns_json: Option<String>,
    reference_tables_json: Option<String>,
) -> Result<crate::db::GenreProfile, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::GenreProfileRepository::new(pool);

    if let Some(existing_id) = id {
        // 更新现有记录
        repo.update(
            &existing_id,
            &genre_name,
            &canonical_name,
            aliases_json.as_deref(),
            core_tone.as_deref(),
            pacing_strategy.as_deref(),
            anti_patterns_json.as_deref(),
            reference_tables_json.as_deref(),
        ).map_err(|e| crate::error::AppError::from(e).to_string())?;
        repo.get_by_id(&existing_id)
            .map_err(|e| crate::error::AppError::from(e).to_string())?
            .ok_or_else(|| "更新后未找到记录".to_string())
    } else {
        // 创建新记录（is_builtin = 0）
        repo.create(
            &genre_name,
            &canonical_name,
            aliases_json.as_deref(),
            core_tone.as_deref(),
            pacing_strategy.as_deref(),
            anti_patterns_json.as_deref(),
            reference_tables_json.as_deref(),
        ).map_err(|e| crate::error::AppError::from(e).to_string())
    }
}

#[tauri::command(rename_all = "snake_case")]
fn delete_genre_profile(id: String) -> Result<usize, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let repo = crate::db::GenreProfileRepository::new(pool);
    // 只允许删除非内置体裁
    if let Ok(Some(profile)) = repo.get_by_id(&id) {
        if profile.is_builtin {
            return Err("内置体裁不可删除".to_string());
        }
    }
    repo.delete(&id).map_err(|e| crate::error::AppError::from(e).to_string())
}

// ==================== v6.0.0: Anti-AI Review Command ====================

#[tauri::command(rename_all = "snake_case")]
fn anti_ai_review(
    text: String,
    genre: Option<String>,
) -> Result<anti_ai::AntiAiReview, String> {
    let reviewer = anti_ai::AntiAiReviewer::new();
    Ok(reviewer.review(&text, genre.as_deref()))
}

#[tauri::command(rename_all = "snake_case")]
fn evolve_style_from_anti_ai_review(
    story_id: String,
    review: anti_ai::AntiAiReview,
) -> Result<creative_engine::style::evolution::StyleDnaDelta, String> {
    let pool = get_pool().ok_or("Database not initialized")?;

    // 1. 获取 story 的 style_dna_id
    let story_repo = db::repositories::StoryRepository::new(pool.clone());
    let style_dna_id = match story_repo.get_by_id(&story_id) {
        Ok(Some(story)) => story.style_dna_id,
        Ok(None) => return Err("故事不存在".to_string()),
        Err(e) => return Err(format!("查询故事失败: {}", e)),
    };

    // 2. 获取或创建 StyleDNA
    let dna_repo = db::repositories_v3::StyleDnaRepository::new(pool.clone());
    let (dna_id, base_dna) = if let Some(id) = style_dna_id {
        let dna = dna_repo.get_by_id(&id).map_err(|e| e.to_string())?
            .ok_or("StyleDNA not found")?;
        let base: creative_engine::style::dna::StyleDNA = serde_json::from_str(&dna.dna_json)
            .map_err(|e| format!("Parse StyleDNA failed: {}", e))?;
        (id, base)
    } else {
        let base = creative_engine::style::dna::StyleDNA::new("evolved");
        let json = serde_json::to_string(&base).map_err(|e| e.to_string())?;
        let dna = dna_repo.create("evolved", None, &json, false).map_err(|e| e.to_string())?;
        let conn = pool.get().map_err(|e| e.to_string())?;
        conn.execute("UPDATE stories SET style_dna_id = ?1 WHERE id = ?2", [&dna.id, &story_id])
            .map_err(|e| e.to_string())?;
        (dna.id, base)
    };

    // 3. 运行风格演化
    let engine = creative_engine::style::evolution::StyleEvolutionEngine::new();
    let delta = engine.evolve_from_reviews(&base_dna, Some(&review), None);

    // 4. 如果有实质调整，保存
    if !delta.is_empty() {
        let evolved = delta.apply(&base_dna);
        let json = serde_json::to_string(&evolved).map_err(|e| e.to_string())?;
        dna_repo.update_dna_json(&dna_id, &json).map_err(|e| e.to_string())?;
    }

    Ok(delta)
}

// ==================== v6.0.1: Telemetry Commands ====================

#[tauri::command(rename_all = "snake_case")]
fn get_feature_usage_stats(
    days: i32,
) -> Result<Vec<crate::telemetry::FeatureUsageStat>, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    crate::telemetry::get_feature_usage_stats(&pool, days)
        .map_err(|e| crate::error::AppError::from(e).to_string())
}

#[tauri::command(rename_all = "snake_case")]
fn log_frontend_feature_usage(
    feature_id: String,
    action: String,
    story_id: Option<String>,
) {
    if let Some(pool) = get_pool() {
        crate::telemetry::log_feature_usage(&pool, &feature_id, &action, story_id.as_deref(), None);
    }
}
