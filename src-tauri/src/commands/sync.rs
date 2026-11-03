#![allow(dead_code)]
//! Sync commands

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{db::DbPool, error::AppError};

/// v0.79.0：跨窗口「当前选中故事」的持久化真源。
///
/// 幕后点击故事后写入 `app_settings`，幕前挂载/聚焦时读取；事件里的 `seq`
/// 用于丢弃乱序/过期选择。旧调用方仍可只发事件（seq=0）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStorySelection {
    pub story_id: String,
    pub title: Option<String>,
    pub seq: i64,
    pub updated_at: String,
}

const ACTIVE_STORY_ID_KEY: &str = "active_story_id";
const ACTIVE_STORY_SEQ_KEY: &str = "active_story_seq";
const ACTIVE_STORY_TITLE_KEY: &str = "active_story_title";

fn ensure_app_settings_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn read_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn clear_active_story(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM app_settings WHERE key IN (?1, ?2, ?3)",
        rusqlite::params![
            ACTIVE_STORY_ID_KEY,
            ACTIVE_STORY_SEQ_KEY,
            ACTIVE_STORY_TITLE_KEY
        ],
    )?;
    Ok(())
}

/// 写入选中故事并返回递增后的 seq。使用 IMMEDIATE 事务，避免幕后快速连点
/// 两个故事时 seq 相同。
fn set_active_story_in_db(
    pool: &DbPool,
    story_id: &str,
    title: Option<&str>,
    origin: &str,
) -> Result<ActiveStorySelection, AppError> {
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .get_by_id(story_id)?
        .ok_or_else(|| AppError::NotFound {
            resource: "story".into(),
            id: story_id.to_string(),
        })?;
    let title = title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(Some(story.title));

    let mut conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("active story db pool: {e}"),
    })?;
    ensure_app_settings_table(&conn)?;
    let tx = conn.transaction()?;
    let prev_seq = read_setting(&tx, ACTIVE_STORY_SEQ_KEY)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let seq = prev_seq.saturating_add(1);
    let now = chrono::Local::now().to_rfc3339();
    let mut stmt = tx.prepare(
        "INSERT OR REPLACE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
    )?;
    stmt.execute(rusqlite::params![ACTIVE_STORY_ID_KEY, story_id, now])?;
    stmt.execute(rusqlite::params![
        ACTIVE_STORY_SEQ_KEY,
        seq.to_string(),
        now
    ])?;
    stmt.execute(rusqlite::params![
        ACTIVE_STORY_TITLE_KEY,
        title.clone().unwrap_or_default(),
        now
    ])?;
    drop(stmt);
    tx.commit()?;

    log::info!(
        "[active_story] selected story={} seq={} origin={}",
        story_id,
        seq,
        origin
    );
    Ok(ActiveStorySelection {
        story_id: story_id.to_string(),
        title,
        seq,
        updated_at: now,
    })
}

fn get_active_story_from_db(pool: &DbPool) -> Result<Option<ActiveStorySelection>, AppError> {
    let conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("active story db pool: {e}"),
    })?;
    ensure_app_settings_table(&conn)?;
    let Some(story_id) = read_setting(&conn, ACTIVE_STORY_ID_KEY)? else {
        return Ok(None);
    };
    let Some(story) =
        crate::db::repositories::StoryRepository::new(pool.clone()).get_by_id(&story_id)?
    else {
        // 目标故事已删除：不要让幕前恢复到一个不存在的故事。
        clear_active_story(&conn)?;
        return Ok(None);
    };
    let seq = read_setting(&conn, ACTIVE_STORY_SEQ_KEY)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let title = read_setting(&conn, ACTIVE_STORY_TITLE_KEY)?
        .filter(|s| !s.trim().is_empty())
        .or(Some(story.title));
    let updated_at = conn
        .query_row(
            "SELECT updated_at FROM app_settings WHERE key = ?1",
            rusqlite::params![ACTIVE_STORY_ID_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_default();
    Ok(Some(ActiveStorySelection {
        story_id,
        title,
        seq,
        updated_at,
    }))
}

// ===== 幕前/幕后通信命令 =====

/// 通知 backstage 内容已变更
#[tauri::command(rename_all = "snake_case")]
pub fn notify_backstage_content_changed(
    text: String,
    chapter_id: String,
    app: AppHandle,
) -> Result<(), AppError> {
    let event = crate::window::BackstageEvent::ContentChanged { text, chapter_id };
    crate::window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 backstage 请求生成内容
#[tauri::command(rename_all = "snake_case")]
pub fn notify_backstage_generation_requested(
    chapter_id: String,
    context: String,
    app: AppHandle,
) -> Result<(), AppError> {
    let event = crate::window::BackstageEvent::GenerationRequested {
        chapter_id,
        context,
    };
    crate::window::WindowManager::send_to_backstage(&app, event)
}

/// 通知 frontstage 内容已变更
#[tauri::command(rename_all = "snake_case")]
pub fn notify_frontstage_content_changed(
    text: String,
    chapter_id: String,
    app: AppHandle,
) -> Result<(), AppError> {
    let event = crate::window::FrontstageEvent::ContentUpdate { text, chapter_id };
    crate::window::WindowManager::send_to_frontstage(&app, event)
}

/// 通知 frontstage 数据已刷新（幕后创建/修改了故事、章节等）
#[tauri::command(rename_all = "snake_case")]
pub fn notify_frontstage_data_refresh(entity: String, app: AppHandle) -> Result<(), AppError> {
    let event = crate::window::FrontstageEvent::DataRefresh { entity };
    crate::window::WindowManager::send_to_frontstage(&app, event)
}

/// v0.79.0：写入/读取「当前选中故事」真源。
#[tauri::command(rename_all = "snake_case")]
pub fn set_active_story(
    story_id: String,
    title: Option<String>,
    origin: Option<String>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<ActiveStorySelection, AppError> {
    let origin = origin.as_deref().unwrap_or("backstage_card");
    let selection = set_active_story_in_db(&pool, &story_id, title.as_deref(), origin)?;
    crate::state_sync::service::StateSync::emit_story_selected_with_meta(
        &app,
        &selection.story_id,
        selection.title.as_deref(),
        selection.seq,
        origin,
    );
    Ok(selection)
}

/// 幕前挂载/聚焦时恢复上次选中的故事。
#[tauri::command(rename_all = "snake_case")]
pub fn get_active_story(pool: State<'_, DbPool>) -> Result<Option<ActiveStorySelection>, AppError> {
    get_active_story_from_db(&pool)
}

/// 幕后故事卡片「打开」：显示幕前窗口并广播 storySelected，
/// 幕前收到后选中该故事（selectStory 定位到最新章节）。
#[tauri::command(rename_all = "snake_case")]
pub fn open_story_in_frontstage(
    story_id: String,
    title: Option<String>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    crate::window::WindowManager::show_frontstage(&app)?;
    let selection = set_active_story_in_db(&pool, &story_id, title.as_deref(), "backstage_open")?;
    crate::state_sync::service::StateSync::emit_story_selected_with_meta(
        &app,
        &selection.story_id,
        selection.title.as_deref(),
        selection.seq,
        "backstage_open",
    );
    Ok(())
}

/// v0.73.9: 幕后点击故事卡片（不打开窗口）时同步选中到幕前。
///
/// v0.79.0：先持久化 active_story（seq 单调），再广播事件；幕前据此恢复，
/// 不再依赖一次性事件是否被监听。
#[tauri::command(rename_all = "snake_case")]
pub fn notify_frontstage_story_selected(
    story_id: String,
    title: Option<String>,
    origin: Option<String>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let origin = origin.as_deref().unwrap_or("backstage_card");
    let selection = set_active_story_in_db(&pool, &story_id, title.as_deref(), origin)?;
    crate::state_sync::service::StateSync::emit_story_selected_with_meta(
        &app,
        &selection.story_id,
        selection.title.as_deref(),
        selection.seq,
        origin,
    );
    Ok(())
}

/// 显示 backstage 窗口
#[tauri::command(rename_all = "snake_case")]
pub fn show_backstage(
    app: AppHandle,
    story_id: Option<String>,
    view: Option<String>,
    panel: Option<String>,
) -> Result<(), AppError> {
    let window = if let Some(window) = app.get_webview_window("backstage") {
        window.show().map_err(AppError::from)?;
        window.set_focus().map_err(AppError::from)?;
        window
    } else {
        // 窗口可能被关闭，重新创建。优先复用 tauri.conf.json 的 backstage
        // 配置（与启动时 from_config 完全一致），不要再手写一份 builder——
        // v0.80.8 前这里漏了 url/尺寸等配置项，Windows/WebView2 重建后出现
        // 空白页（幕前第二次点设置白屏）。配置缺失时保留兜底。
        log::warn!("[show_backstage] backstage window missing, recreating from config");
        let backstage_config = app
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == "backstage")
            .cloned();
        let window = if let Some(config) = backstage_config {
            tauri::WebviewWindowBuilder::from_config(&app, &config)
                .map_err(AppError::from)?
                .build()
                .map_err(AppError::from)?
        } else {
            tauri::WebviewWindowBuilder::new(
                &app,
                "backstage",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("草苔 - 幕后工作室")
            .inner_size(1200.0, 800.0)
            .center()
            .build()
            .map_err(AppError::from)?
        };
        window.show().map_err(AppError::from)?;
        window.set_focus().map_err(AppError::from)?;
        window
    };
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

    // 延迟发射 backstage-shown 事件，确保前端监听器已就绪 + WebView2
    // 渲染表面已恢复 800ms 给 WebView2 足够时间从休眠状态恢复
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
            let _ = window.emit(
                "backstage-shown",
                serde_json::json!({
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis(),
                    "story_id": story_id_clone
                }),
            );
        }
    });
    // 导航到指定视图；若未提供则默认进入 stories/overview
    let target_view = view.unwrap_or_else(|| "stories".to_string());
    let _ = crate::window::WindowManager::send_to_backstage(
        &app,
        crate::window::BackstageEvent::NavigateTo {
            view: target_view,
            highlight_story_id: story_id.clone(),
            open_panel: panel.clone().or_else(|| {
                if story_id.is_some() {
                    Some("overview".to_string())
                } else {
                    None
                }
            }),
        },
    );

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_genesis_pipeline(session_id: String) -> Result<bool, AppError> {
    let cancelled = crate::narrative::pipeline::cancel_pipeline(&session_id);
    // 桥接 agency 创世：T7 起 smart_execute 创世分支的 session_id 即 agency
    // run_id， 置取消 flag 后 run 在下一 check_cancel 停并落
    // cancelled（生成中前端尚未拿到 session_id 时为空串，跳过）。
    if !session_id.is_empty() {
        crate::agency::coordinator::cancel_agency_run(&session_id);
    }
    if cancelled {
        log::info!(
            "[cancel_genesis_pipeline] Pipeline {} 已标记为取消",
            session_id
        );
        Ok(true)
    } else {
        log::warn!(
            "[cancel_genesis_pipeline] Pipeline {} 未找到或已完成",
            session_id
        );
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connection::create_test_pool, dto::CreateStoryRequest, DbPool};

    fn story_req(title: &str) -> CreateStoryRequest {
        CreateStoryRequest {
            title: title.into(),
            description: None,
            genre: Some("玄幻".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }
    }

    fn seed_story(pool: &DbPool, title: &str) -> String {
        crate::db::repositories::StoryRepository::new(pool.clone())
            .create(story_req(title))
            .unwrap()
            .id
    }

    #[test]
    fn active_story_set_get_roundtrip() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story(&pool, "甲");
        let first = set_active_story_in_db(&pool, &sid, Some("甲"), "test").unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(first.title.as_deref(), Some("甲"));

        let got = get_active_story_from_db(&pool).unwrap().unwrap();
        assert_eq!(got.story_id, sid);
        assert_eq!(got.seq, 1);
        assert_eq!(got.title.as_deref(), Some("甲"));
    }

    #[test]
    fn active_story_seq_increases() {
        let pool = create_test_pool().unwrap();
        let a = seed_story(&pool, "甲");
        let b = seed_story(&pool, "乙");
        set_active_story_in_db(&pool, &a, None, "test").unwrap();
        let second = set_active_story_in_db(&pool, &b, None, "test").unwrap();
        assert_eq!(second.seq, 2);
        assert_eq!(second.story_id, b);
    }

    #[test]
    fn active_story_rejects_missing_story() {
        let pool = create_test_pool().unwrap();
        let err = set_active_story_in_db(&pool, "missing", None, "test").unwrap_err();
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[test]
    fn active_story_clears_after_story_deleted() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story(&pool, "甲");
        set_active_story_in_db(&pool, &sid, Some("甲"), "test").unwrap();
        crate::db::repositories::StoryRepository::new(pool.clone())
            .delete(&sid)
            .unwrap();
        assert!(get_active_story_from_db(&pool).unwrap().is_none());
    }
}
