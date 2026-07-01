#![allow(dead_code)]
//! Sync commands

use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;

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

/// 显示 backstage 窗口
#[tauri::command(rename_all = "snake_case")]
pub fn show_backstage(app: AppHandle, story_id: Option<String>) -> Result<(), AppError> {
    let window = if let Some(window) = app.get_webview_window("backstage") {
        window.show().map_err(AppError::from)?;
        window.set_focus().map_err(AppError::from)?;
        window
    } else {
        // 窗口可能被关闭，重新创建
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "backstage",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("草苔 - 幕后工作室")
        .inner_size(1200.0, 800.0)
        .center()
        .build()
        .map_err(AppError::from)?;
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
    if let Some(sid) = story_id {
        let _ = crate::window::WindowManager::send_to_backstage(
            &app,
            crate::window::BackstageEvent::NavigateTo {
                view: "stories".to_string(),
                highlight_story_id: Some(sid),
                open_panel: Some("overview".to_string()),
            },
        );
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_genesis_pipeline(session_id: String) -> Result<bool, AppError> {
    let cancelled = crate::narrative::pipeline::cancel_pipeline(&session_id);
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
