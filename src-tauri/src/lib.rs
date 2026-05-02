#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

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
mod chat;
mod analytics;
mod skills;
mod mcp;
mod collab;
mod state;
mod router;
mod evolution;
mod embeddings;
mod utils;
mod window;
mod updater;
mod commands_v3;
mod intent;
mod creative_engine;
mod subscription;
mod book_deconstruction;
mod task_system;
mod canonical_state;
mod capabilities;
mod planner;
mod audit;
mod auth;
mod state_sync;

#[cfg(test)]
mod test_utils;

use tauri::{Manager, AppHandle, Emitter};

use db::{DbPool, init_db, StoryRepository, CharacterRepository, ChapterRepository, CreateStoryRequest, CreateCharacterRequest, CreateChapterRequest};
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

use collab::websocket::WebSocketServer;


static DB_POOL: Lazy<Mutex<Option<DbPool>>> = Lazy::new(|| Mutex::new(None));
static APP_CONFIG: Lazy<Mutex<Option<AppConfig>>> = Lazy::new(|| Mutex::new(None));
pub static SKILL_MANAGER: OnceCell<Mutex<SkillManager>> = OnceCell::new();

/// 自动 Ingest 冷却状态：chapter_id -> (content_hash, last_ingest_time)
/// 防止频繁保存导致重复 LLM 调用
static INGEST_COOLDOWN: Lazy<Mutex<HashMap<String, (u64, Instant)>>> = Lazy::new(|| Mutex::new(HashMap::new()));
const INGEST_COOLDOWN_SECONDS: u64 = 300; // 5 分钟冷却期

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
            
            log::info!("App directory: {:?}", app_dir);

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

            // Initialize embedding model
            let _ = embeddings::init_embedding_model();

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
            }

            // Initialize LanceDB vector store
            let vector_db_path = app_dir.join("vector_db").to_string_lossy().to_string();
            std::fs::create_dir_all(&vector_db_path).ok();

            tauri::async_runtime::spawn(async move {
                let mut vector_store = LanceVectorStore::new(vector_db_path);
                if let Err(e) = vector_store.init().await {
                    log::error!("Failed to initialize vector store: {}", e);
                } else {
                    let _ = VECTOR_STORE.set(vector_store);
                    log::info!("Vector store initialized successfully");
                }
            });

            // Start WebSocket server for collaborative editing
            if let Some(pool) = get_pool() {
                tauri::async_runtime::spawn(async move {
                    // Try different ports if 8765 is taken
                    let ports = [8765, 8766, 8767, 8768, 8769];
                    for port in ports {
                        let ws_server = WebSocketServer::with_pool(pool.clone());
                        match ws_server.start(port).await {
                            Ok(_) => {
                                log::info!("WebSocket server started on port {}", port);
                                break;
                            }
                            Err(e) => {
                                log::warn!("Failed to start WebSocket server on port {}: {}", port, e);
                            }
                        }
                    }
                });
            }

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check, check_model_status, chat_completion, get_state, list_stories, create_story, update_story, delete_story,
            get_story_characters, create_character, update_character, delete_character,
            get_story_chapters, get_chapter, create_chapter, update_chapter, delete_chapter,
            get_skills, get_skill, get_skills_by_category, import_skill, enable_skill, disable_skill, uninstall_skill, execute_skill, update_skill, format_text,
            connect_mcp_server, call_mcp_tool, disconnect_mcp_server, get_mcp_connections, list_mcp_tools, execute_mcp_tool,
            search_similar, text_search_vectors, hybrid_search_vectors, embed_chapter,
            export_story,
            // Window management commands
            window::show_frontstage,
            window::hide_frontstage,
            window::toggle_frontstage,
            window::get_window_state,
            window::update_frontstage_content,
            // Backstage communication commands
            notify_backstage_content_changed,
            notify_backstage_generation_requested,
            notify_frontstage_content_changed,
            notify_frontstage_data_refresh,
            show_backstage,
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
            // Intent commands
            parse_intent,
            execute_intent,
            record_feedback,
            // Smart orchestrator
            smart_execute,
            get_input_hint,
            // Agent commands
            agents::commands::agent_execute,
            agents::commands::agent_execute_stream,
            agents::commands::agent_cancel_task,
            agents::commands::writer_agent_execute,
            agents::commands::auto_write,
            agents::commands::auto_write_cancel,
            agents::commands::auto_revise,
            agents::commands::auto_revise_cancel,
            agents::service::get_available_agents,
            // Subscription commands
            subscription::commands::get_subscription_status,
            subscription::commands::get_quota_detail,
            subscription::commands::check_auto_write_quota,
            subscription::commands::check_auto_revise_quota,
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
            // Structured outline commands
            commands_v3::generate_scene_outline,
            commands_v3::generate_scene_draft,
            // Audit commands
            audit::commands::audit_scene,
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
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}

#[tauri::command]
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

#[tauri::command]
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
        .map_err(|e| e.to_string())?;

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

    let response = request.json(&body).send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, text));
    }

    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(data)
}

#[tauri::command]
async fn check_model_status(app_handle: AppHandle) -> Result<String, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = config::AppConfig::load(&app_dir).map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;

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

#[tauri::command]
fn get_state() -> Result<DashboardState, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let stories = StoryRepository::new(pool.clone()).get_all().map_err(|e| e.to_string())?;
    let chars_count: usize = stories.iter().map(|s| CharacterRepository::new(pool.clone()).get_by_story(&s.id).map(|c| c.len()).unwrap_or(0)).sum();
    Ok(DashboardState { current_story: stories.first().cloned(), stories_count: stories.len(), characters_count: chars_count, chapters_count: 0 })
}

#[tauri::command]
fn list_stories() -> Result<Vec<db::Story>, String> {
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).get_all().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_story(title: String, description: Option<String>, genre: Option<String>, app: AppHandle) -> Result<db::Story, String> {
    let story = StoryRepository::new(get_pool().ok_or("DB not initialized")?).create(CreateStoryRequest { title, description, genre, style_dna_id: None }).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_created(&app, &story.id, &story.title);
    Ok(story)
}

#[tauri::command]
fn update_story(
    id: String,
    title: Option<String>,
    description: Option<String>,
    tone: Option<String>,
    pacing: Option<String>,
    style_dna_id: Option<String>,
    methodology_id: Option<String>,
    methodology_step: Option<i32>,
    app: AppHandle,
) -> Result<(), String> {
    let req = db::UpdateStoryRequest { title: title.clone(), description: description.clone(), tone, pacing, style_dna_id, methodology_id, methodology_step };
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).update(&id, &req).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_updated(&app, &id, title.as_deref());
    Ok(())
}

#[tauri::command]
fn delete_story(id: String, app: AppHandle) -> Result<(), String> {
    StoryRepository::new(get_pool().ok_or("DB not initialized")?).delete(&id).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_story_deleted(&app, &id);
    Ok(())
}

#[tauri::command]
fn get_story_characters(story_id: String) -> Result<Vec<db::Character>, String> {
    CharacterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_story(&story_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_character(story_id: String, name: String, background: Option<String>, app: AppHandle) -> Result<db::Character, String> {
    let character = CharacterRepository::new(get_pool().ok_or("DB not initialized")?).create(CreateCharacterRequest { story_id: story_id.clone(), name: name.clone(), background, personality: None, goals: None, appearance: None, gender: None, age: None }).map_err(|e| e.to_string())?;

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
    Ok(character)
}

#[tauri::command]
fn update_character(id: String, name: Option<String>, background: Option<String>, personality: Option<String>, goals: Option<String>, app: AppHandle) -> Result<(), String> {
    let repo = CharacterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，确保同步事件携带正确的 story_id
    let story_id = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id).unwrap_or_default();
    repo.update(&id, name.clone(), background, personality, goals, None, None, None).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_character_updated(&app, &id, name.as_deref(), &story_id);
    Ok(())
}

#[tauri::command]
fn delete_character(id: String, app: AppHandle) -> Result<(), String> {
    let repo = CharacterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，删除后无法再获取
    let story_id = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id).unwrap_or_default();
    repo.delete(&id).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_character_deleted(&app, &id, &story_id);
    Ok(())
}

#[tauri::command]
fn get_story_chapters(story_id: String) -> Result<Vec<db::Chapter>, String> {
    db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_story(&story_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_chapter(id: String) -> Result<Option<db::Chapter>, String> {
    db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?).get_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_chapter(id: String, title: Option<String>, outline: Option<String>, content: Option<String>, word_count: Option<i32>, app: AppHandle) -> Result<(), String> {
    let repo = db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，确保同步事件携带正确的 story_id
    let story_id = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id).unwrap_or_default();
    let result = repo.update(&id, title.clone(), outline, content, word_count).map_err(|e| e.to_string());
    if result.is_ok() {
        let _ = window::WindowManager::send_to_frontstage(&app, window::FrontstageEvent::SaveStatus { saved: true, timestamp: Some(chrono::Local::now().to_rfc3339()) });
        let _ = crate::state_sync::StateSync::emit_chapter_updated(&app, &id, title.as_deref(), &story_id);
        // v5.1.1: 保存后自动触发 IngestPipeline，更新知识图谱
        if let Some(pool) = get_pool() {
            let chapter_id = id.clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                auto_ingest_chapter(pool, app_handle, chapter_id).await;
            });
        }
    }
    result.map(|_| ())
}

#[tauri::command]
fn delete_chapter(id: String, app: AppHandle) -> Result<(), String> {
    let repo = db::ChapterRepository::new(get_pool().ok_or("DB not initialized")?);
    // 先查询 story_id，删除后无法再获取
    let story_id = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id).unwrap_or_default();
    repo.delete(&id).map_err(|e| e.to_string())?;
    let _ = crate::state_sync::StateSync::emit_chapter_deleted(&app, &id, &story_id);
    Ok(())
}

/// 计算字符串的简单哈希（用于内容去重）
fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// 章节保存后自动触发 IngestPipeline，将内容分析并更新知识图谱
/// 内置 5 分钟冷却期 + 内容哈希去重，防止频繁保存导致重复 LLM 调用
async fn auto_ingest_chapter(pool: DbPool, app: AppHandle, chapter_id: String) {
    let repo = db::ChapterRepository::new(pool.clone());
    let chapter = match repo.get_by_id(&chapter_id).ok().flatten() {
        Some(c) => c,
        None => {
            log::warn!("[auto_ingest] Chapter {} not found, skipping ingest", chapter_id);
            return;
        }
    };
    let story_id = chapter.story_id.clone();
    let content_text = chapter.content.clone().unwrap_or_default();
    if content_text.len() < 20 {
        log::info!("[auto_ingest] Chapter {} content too short ({} chars), skipping ingest", chapter_id, content_text.len());
        return;
    }

    // 冷却期检查：内容未变化或 5 分钟内已 Ingest，则跳过
    {
        let mut cooldown = INGEST_COOLDOWN.lock().unwrap();
        let content_hash = hash_content(&content_text);
        let now = Instant::now();
        let should_skip = cooldown.get(&chapter_id).map(|(last_hash, last_time)| {
            let same_content = *last_hash == content_hash;
            let within_cooldown = now.duration_since(*last_time) < Duration::from_secs(INGEST_COOLDOWN_SECONDS);
            same_content || within_cooldown
        }).unwrap_or(false);

        if should_skip {
            log::info!("[auto_ingest] Chapter {} skipped (cooldown or unchanged content)", chapter_id);
            return;
        }

        // 记录本次 Ingest 的 hash 和时间
        cooldown.insert(chapter_id.clone(), (content_hash, now));
    }

    let llm_service = crate::llm::LlmService::new(app);
    let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service);
    let ingest_content = crate::memory::ingest::IngestContent {
        text: content_text,
        source: format!("chapter:{}:{}", story_id, chapter_id),
        story_id: story_id.clone(),
        scene_id: chapter.scene_id.clone(),
    };

    match pipeline.ingest(&ingest_content).await {
        Ok(result) => {
            let kg_repo = db::repositories_v3::KnowledgeGraphRepository::new(pool.clone());
            let entity_count = result.entities.len();
            let relation_count = result.relations.len();
            match kg_repo.save_entities_batch(&result.entities) {
                Ok(saved) => log::info!("[auto_ingest] Saved {}/{} entities for chapter {}", saved, entity_count, chapter_id),
                Err(e) => log::warn!("[auto_ingest] Failed to save entities: {}", e),
            }
            match kg_repo.save_relations_batch(&result.relations) {
                Ok(saved) => log::info!("[auto_ingest] Saved {}/{} relations for chapter {}", saved, relation_count, chapter_id),
                Err(e) => log::warn!("[auto_ingest] Failed to save relations: {}", e),
            }
        }
        Err(e) => {
            log::warn!("[auto_ingest] IngestPipeline failed for chapter {}: {}", chapter_id, e);
        }
    }
}

#[tauri::command]
fn create_chapter(story_id: String, chapter_number: i32, title: Option<String>, outline: Option<String>, content: Option<String>, app: AppHandle) -> Result<db::Chapter, String> {
    let repo = ChapterRepository::new(get_pool().ok_or("DB not initialized")?);

    // 如果该 chapter_number 已存在，直接返回已有章节（幂等）
    if let Ok(chapters) = repo.get_by_story(&story_id) {
        if let Some(existing) = chapters.into_iter().find(|c| c.chapter_number == chapter_number) {
            log::info!("[create_chapter] Chapter {} already exists for story {}, returning existing", chapter_number, story_id);
            return Ok(existing);
        }
    }

    let req = CreateChapterRequest { story_id: story_id.clone(), chapter_number, title: title.clone(), outline, content };
    let chapter = repo.create(req).map_err(|e| e.to_string())?;

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
    if let Some(ref scene_id) = chapter.scene_id {
        let _ = crate::state_sync::StateSync::emit_scene_updated(&app, &story_id, scene_id, title.as_deref());
    }
    // v5.1.1: 新建章节后自动触发 IngestPipeline（无论 AfterChapterSave hook 是否执行）
    if let Some(pool) = get_pool() {
        let chapter_id = chapter.id.clone();
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            auto_ingest_chapter(pool, app_handle, chapter_id).await;
        });
    }
    Ok(chapter)
}

#[tauri::command]
fn get_skills() -> Result<Vec<SkillInfo>, String> {
    let skills = SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.get_all_skills();
    Ok(skills.into_iter().map(SkillInfo::from).collect())
}

#[tauri::command]
fn get_skills_by_category(category: String) -> Result<Vec<SkillInfo>, String> {
    let cat = match category.as_str() {
        "writing" => SkillCategory::Writing, "analysis" => SkillCategory::Analysis,
        "character" => SkillCategory::Character, "world_building" => SkillCategory::WorldBuilding,
        "style" => SkillCategory::Style, "plot" => SkillCategory::Plot,
        "export" => SkillCategory::Export, "integration" => SkillCategory::Integration,
        _ => SkillCategory::Custom,
    };
    let skills = SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.get_skills_by_category(cat);
    Ok(skills.into_iter().map(SkillInfo::from).collect())
}

#[tauri::command]
fn import_skill(path: String) -> Result<SkillInfo, String> {
    let skill = SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.import_skill(std::path::Path::new(&path))?;
    Ok(SkillInfo::from(skill))
}

#[tauri::command]
fn enable_skill(skill_id: String) -> Result<(), String> {
    SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.enable_skill(&skill_id)
}

#[tauri::command]
fn disable_skill(skill_id: String) -> Result<(), String> {
    SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.disable_skill(&skill_id)
}

#[tauri::command]
fn uninstall_skill(skill_id: String) -> Result<(), String> {
    SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.uninstall_skill(&skill_id)
}

#[tauri::command]
fn get_skill(skill_id: String) -> Result<SkillInfo, String> {
    let skill = SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.get_skill(&skill_id);
    skill.map(SkillInfo::from).ok_or_else(|| "Skill not found".to_string())
}

#[tauri::command]
fn update_skill(skill_id: String, manifest: skills::SkillManifest) -> Result<(), String> {
    SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?.update_skill(&skill_id, manifest)
}

#[tauri::command]
async fn execute_skill(
    skill_id: String,
    params: HashMap<String, serde_json::Value>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
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
        }
    };

    // Execute skill
    let manager = {
        let guard = SKILL_MANAGER.get().ok_or("Skills not initialized")?.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    
    let result = manager.execute_skill(&skill_id, &context, params).await?;
    
    if !result.success {
        return Err(result.error.unwrap_or("Skill execution failed".to_string()));
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
        return Err("Skill did not produce a valid prompt".to_string());
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
#[tauri::command]
async fn format_text(content: String, app: AppHandle) -> Result<String, String> {
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
        .ok_or_else(|| "LLM returned empty content".to_string())
}

use tokio::sync::Mutex as TokioMutex;

static MCP_CONNECTIONS: Lazy<TokioMutex<HashMap<String, mcp::McpClient>>> =
    Lazy::new(|| TokioMutex::new(HashMap::new()));

#[tauri::command]
async fn connect_mcp_server(config: McpServerConfig) -> Result<Vec<mcp::McpTool>, String> {
    let mut client = McpClient::new(config.clone());
    client.connect().await.map_err(|e| e.to_string())?;
    let tools = client.get_tools().clone();
    let mut connections = MCP_CONNECTIONS.lock().await;
    connections.insert(config.id.clone(), client);
    log::info!("[MCP] Connected to server {} ({}), {} tools available", config.name, config.id, tools.len());
    Ok(tools)
}

#[tauri::command]
async fn call_mcp_tool(server_id: String, tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut connections = MCP_CONNECTIONS.lock().await;
    let client = connections.get_mut(&server_id)
        .ok_or_else(|| format!("MCP server {} not connected", server_id))?;
    client.call_tool(&tool_name, arguments).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn disconnect_mcp_server(server_id: String) -> Result<(), String> {
    let mut connections = MCP_CONNECTIONS.lock().await;
    if let Some(mut client) = connections.remove(&server_id) {
        client.disconnect().await;
        log::info!("[MCP] Disconnected from server {}", server_id);
    }
    Ok(())
}

#[tauri::command]
async fn get_mcp_connections() -> Result<Vec<serde_json::Value>, String> {
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

static VECTOR_STORE: OnceCell<LanceVectorStore> = OnceCell::new();

#[tauri::command]
async fn search_similar(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    use embeddings::embed_text;
    
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    
    // 生成查询向量
    let query_embedding = embed_text(&query).map_err(|e| e.to_string())?;
    
    store.search(&story_id, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn text_search_vectors(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    store.text_search(&story_id, &query, top_k.unwrap_or(5))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hybrid_search_vectors(story_id: String, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    use embeddings::embed_text;
    
    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;
    let query_embedding = embed_text(&query).map_err(|e| e.to_string())?;
    
    store.hybrid_search(&story_id, &query, query_embedding, top_k.unwrap_or(5))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn embed_chapter(chapter_id: String, content: String) -> Result<(), String> {
    use embeddings::embed_text;
    use vector::VectorRecord;

    let store = VECTOR_STORE.get().ok_or("Vector store not initialized")?;

    // 生成嵌入向量
    let embedding = embed_text(&content).map_err(|e| e.to_string())?;

    let record = VectorRecord {
        id: format!("{}", uuid::Uuid::new_v4()),
        story_id: String::new(), // 需要从chapter_id查询
        chapter_id,
        chapter_number: 0,
        text: content.chars().take(500).collect(),
        record_type: "chapter".to_string(),
        embedding,
    };

    store.add_record(record).await.map_err(|e| e.to_string())
}

// Intent Parser Command
#[tauri::command]
async fn parse_intent(user_input: String, app_handle: AppHandle) -> Result<intent::Intent, String> {
    let parser = intent::IntentParser::new(app_handle);
    parser.parse(&user_input).await
}

// Intent Executor Command
#[tauri::command]
async fn execute_intent(
    intent: intent::Intent,
    story_id: String,
    app_handle: AppHandle,
) -> Result<intent::IntentExecutionResult, String> {
    let executor = intent::IntentExecutor::new(app_handle);
    executor.execute(intent, story_id).await
}

/// 智能执行命令 - 新一代意图理解与执行入口
#[tauri::command]
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
    let current_content_preview = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| chapters.last().and_then(|c| c.content.clone()))
        .map(|content| {
            let preview: String = content.chars().take(2000).collect();
            if content.chars().count() > 2000 {
                format!("{}...", preview)
            } else {
                preview
            }
        });

    // 检测是否需要启动小说初始化工作流
    let is_bootstrap_intent = stories.is_empty()
        && is_novel_creation_intent(&user_input);

    if is_bootstrap_intent {
        log::info!("[smart_execute] Detected novel creation intent, starting NovelBootstrapWorkflow");
        let bootstrap = planner::bootstrap::NovelBootstrapWorkflow::new(app_handle.clone());
        match bootstrap.run(&user_input).await {
            Ok(session) => {
                if let Some(ref story_id) = session.story_id {
                    let _ = crate::state_sync::StateSync::emit_story_created(&app_handle, story_id, "新故事");
                }
                return Ok(planner::PlanExecutionResult {
                    success: true,
                    steps_completed: session.total_steps,
                    // 直接返回生成的小说正文开头，前端以 ghost text 形式展示
                    final_content: session.first_chapter_content,
                    messages: vec![
                        format!("story_created:{}", session.story_id.unwrap_or_default()),
                        "novel_bootstrap_completed".to_string(),
                    ],
                });
            }
            Err(e) => {
                log::error!("[smart_execute] NovelBootstrapWorkflow failed: {}", e);
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
        }).await;
        return Ok(result);
    }

    emit_progress("context_loaded", "故事上下文加载完成", 2, 5);

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

    Ok(result)
}

/// 检测用户输入是否包含"创建新小说"的意图
fn is_novel_creation_intent(user_input: &str) -> bool {
    let input = user_input.to_lowercase();
    let creation_keywords = [
        "写", "创作", "开始", "新建", "生成", "创建",
        "write", "create", "start", "generate", "begin",
        "novel", "story", "book", "小说", "故事", "书",
    ];
    creation_keywords.iter().any(|&kw| input.contains(kw))
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

#[tauri::command]
async fn record_feedback(request: RecordFeedbackRequest) -> Result<(), String> {
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
        _ => Err("Unknown feedback type".to_string()),
    };
    
    // 异步触发偏好挖掘，让自适应学习系统形成闭环
    if result.is_ok() {
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
    }
    
    result
}

/// 获取输入栏智能提示 — 由LLM根据当前故事上下文生成建议
#[tauri::command]
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

#[tauri::command]
async fn list_mcp_tools() -> Result<Vec<mcp::McpTool>, String> {
    let config = mcp::McpServerConfig {
        id: "builtin".to_string(),
        name: "Built-in Tools".to_string(),
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        timeout_seconds: 30,
    };

    let server = mcp::McpServer::new(config);
    Ok(server.get_tools())
}

#[tauri::command]
async fn execute_mcp_tool(tool_name: String, arguments: serde_json::Value) -> Result<serde_json::Value, String> {
    let config = mcp::McpServerConfig {
        id: "builtin".to_string(),
        name: "Built-in Tools".to_string(),
        command: String::new(),
        args: vec![],
        env: HashMap::new(),
        timeout_seconds: 30,
    };

    let server = mcp::McpServer::new(config);
    server.start().await.map_err(|e| e.to_string())?;

    let result = server.execute_tool(&tool_name, arguments).await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

#[derive(Debug, Deserialize)]
struct ExportOptions {
    story_id: String,
    format: String,
    include_metadata: Option<bool>,
    include_outline: Option<bool>,
    include_characters: Option<bool>,
}
#[tauri::command]
async fn export_story(options: ExportOptions, app_handle: tauri::AppHandle) -> Result<ExportResult, String> {
    let pool = get_pool().ok_or("Database not initialized")?;

    let story = StoryRepository::new(pool.clone())
        .get_by_id(&options.story_id)
        .map_err(|e| e.to_string())?
        .ok_or("Story not found")?;

    let chapters = ChapterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(|e| e.to_string())?;

    let characters = CharacterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(|e| e.to_string())?;

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

    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
    let output_path = export_dir.join(&filename);

    let config = ExportConfig {
        format,
        include_outline: options.include_outline.unwrap_or(true),
        include_metadata: options.include_metadata.unwrap_or(true),
        chapter_separator: "\n\n---\n\n".to_string(),
    };

    let exporter = StoryExporter::new();
    exporter.export_to_file(&story, &chapters, &characters, &config, &output_path)
        .map_err(|e| e.to_string())?;

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        content: std::fs::read_to_string(&output_path).unwrap_or_default(),
        format: options.format,
    })
}

// ===== 幕前/幕后通信命令 =====

/// 通知 backstage 内容已变更
#[tauri::command]
fn notify_backstage_content_changed(text: String, chapter_id: String, app: AppHandle) -> Result<(), String> {
    let event = window::BackstageEvent::ContentChanged { text, chapter_id };
    window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 backstage 请求生成内容
#[tauri::command]
fn notify_backstage_generation_requested(chapter_id: String, context: String, app: AppHandle) -> Result<(), String> {
    let event = window::BackstageEvent::GenerationRequested { chapter_id, context };
    window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 frontstage 内容已变更
#[tauri::command]
fn notify_frontstage_content_changed(text: String, chapter_id: String, app: AppHandle) -> Result<(), String> {
    let event = window::FrontstageEvent::ContentUpdate { text, chapter_id };
    window::WindowManager::send_to_frontstage(&app, event)
}

/// 通知 frontstage 数据已刷新（幕后创建/修改了故事、章节等）
#[tauri::command]
fn notify_frontstage_data_refresh(entity: String, app: AppHandle) -> Result<(), String> {
    let event = window::FrontstageEvent::DataRefresh { entity };
    window::WindowManager::send_to_frontstage(&app, event)
}

/// 显示 backstage 窗口
#[tauri::command]
fn show_backstage(app: AppHandle, story_id: Option<String>) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window("backstage") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        window
    };

    // v5.0.0 修复：强制 WebView2 重新创建渲染表面，防止 hide/show 后白屏
    // 方法：微调窗口大小再恢复，触发 WebView2 重绘
    if let Ok(size) = window.inner_size() {
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: size.width + 1,
            height: size.height,
        }));
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: size.width,
            height: size.height,
        }));
    }

    // 执行 JS 强制重排
    let _ = window.eval(r#"
        (function() {
            const body = document.body;
            if (body) {
                const originalDisplay = body.style.display;
                body.style.display = 'none';
                void body.offsetHeight;
                body.style.display = originalDisplay || '';
            }
            window.dispatchEvent(new Event('resize'));
        })();
    "#);

    // 延迟发射 backstage-shown 事件，确保前端监听器已就绪
    let app_handle = app.clone();
    let story_id_clone = story_id.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if let Some(window) = app_handle.get_webview_window("backstage") {
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
#[tauri::command]
async fn get_canonical_state(story_id: String) -> Result<canonical_state::CanonicalStateSnapshot, String> {
    let pool = get_pool().ok_or("Database not initialized")?;
    let manager = canonical_state::CanonicalStateManager::new(pool);
    manager.get_snapshot(&story_id).await
}

// ===== 模型驱动的智能编排命令 =====


