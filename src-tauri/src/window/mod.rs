#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use ts_rs::TS;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub frontstage_visible: bool,
    pub backstage_visible: bool,
}

/// 窗口管理器 - 管理幕前和幕后窗口
pub struct WindowManager;

impl WindowManager {
    /// 获取幕前窗口
    pub fn get_frontstage(app: &AppHandle) -> Option<WebviewWindow> {
        app.get_webview_window("frontstage")
    }

    /// 获取幕后窗口
    pub fn get_backstage(app: &AppHandle) -> Option<WebviewWindow> {
        app.get_webview_window("backstage")
    }

    /// 显示幕前窗口
    pub fn show_frontstage(app: &AppHandle) -> Result<(), AppError> {
        if let Some(window) = Self::get_frontstage(app) {
            window
                .show()
                .map_err(|e| AppError::internal(e.to_string()))?;
            window
                .set_focus()
                .map_err(|e| AppError::internal(e.to_string()))?;
            Ok(())
        } else {
            Err(AppError::internal("Frontstage window not found"))
        }
    }

    /// 隐藏幕前窗口
    pub fn hide_frontstage(app: &AppHandle) -> Result<(), AppError> {
        if let Some(window) = Self::get_frontstage(app) {
            window.hide().map_err(|e| AppError::internal(e.to_string()))
        } else {
            Err(AppError::internal("Frontstage window not found"))
        }
    }

    /// 切换幕前窗口显示状态
    pub fn toggle_frontstage(app: &AppHandle) -> Result<bool, AppError> {
        if let Some(window) = Self::get_frontstage(app) {
            let is_visible = window
                .is_visible()
                .map_err(|e| AppError::internal(e.to_string()))?;
            if is_visible {
                window
                    .hide()
                    .map_err(|e| AppError::internal(e.to_string()))?;
                Ok(false)
            } else {
                window
                    .show()
                    .map_err(|e| AppError::internal(e.to_string()))?;
                window
                    .set_focus()
                    .map_err(|e| AppError::internal(e.to_string()))?;
                Ok(true)
            }
        } else {
            Err(AppError::internal("Frontstage window not found"))
        }
    }

    /// 获取窗口状态
    pub fn get_window_state(app: &AppHandle) -> Result<WindowState, AppError> {
        let frontstage_visible = if let Some(window) = Self::get_frontstage(app) {
            window
                .is_visible()
                .map_err(|e| AppError::internal(e.to_string()))?
        } else {
            false
        };

        let backstage_visible = if let Some(window) = Self::get_backstage(app) {
            window
                .is_visible()
                .map_err(|e| AppError::internal(e.to_string()))?
        } else {
            false
        };

        Ok(WindowState {
            frontstage_visible,
            backstage_visible,
        })
    }

    /// 向幕前窗口发送内容更新
    pub fn send_to_frontstage(app: &AppHandle, event: FrontstageEvent) -> Result<(), AppError> {
        if let Some(window) = Self::get_frontstage(app) {
            window
                .emit("frontstage-update", event)
                .map_err(|e| AppError::internal(e.to_string()))
        } else {
            Err(AppError::internal("Frontstage window not found"))
        }
    }

    /// 向幕后窗口发送内容更新
    pub fn send_to_backstage(app: &AppHandle, event: BackstageEvent) -> Result<(), AppError> {
        if let Some(window) = Self::get_backstage(app) {
            window
                .emit("backstage-update", event)
                .map_err(|e| AppError::internal(e.to_string()))
        } else {
            Err(AppError::internal("Backstage window not found"))
        }
    }
}

/// 发送给幕前窗口的事件
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(rename_all = "camelCase")]
#[serde(tag = "type", content = "payload")]
pub enum FrontstageEvent {
    /// 更新正文内容（完全替换）
    ContentUpdate { text: String, chapter_id: String },
    /// 追加内容到正文末尾
    AppendContent { text: String, chapter_id: String },
    /// AI 生成段落预览
    AiPreview {
        text: String,
        insert_position: usize,
    },
    /// 章节切换
    ChapterSwitch {
        story_id: String,
        chapter_id: String,
        /// Phase 1: Scene 为主要叙事单元，携带 scene_id 供前端按场景加载
        scene_id: Option<String>,
        title: String,
        content: Option<String>,
        /// Phase 4 fix: false 时前端不自动将章节内容塞入编辑器，等
        /// generatedText + Tab 确认。 Genesis 创世时设为
        /// false，续写/正常切章保持 true。
        auto_accept: bool,
    },
    /// 保存状态更新
    SaveStatus {
        saved: bool,
        timestamp: Option<String>,
    },
    /// 数据刷新通知（幕后数据变更，幕前需重新加载）
    DataRefresh { entity: String },
}

/// 发送给幕后窗口的事件
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(rename_all = "camelCase")]
#[serde(tag = "type", content = "payload")]
pub enum BackstageEvent {
    /// 幕前内容变更
    ContentChanged { text: String, chapter_id: String },
    /// 幕前请求生成
    GenerationRequested { chapter_id: String, context: String },
    /// 幕前窗口关闭
    FrontstageClosed,
    /// 幕前窗口获得焦点
    FrontstageFocused,
    /// 数据刷新通知
    DataRefresh { entity: String },
    /// 导航到指定视图
    NavigateTo {
        view: String,
        highlight_story_id: Option<String>,
        open_panel: Option<String>,
    },
}

/// AI 提示位置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(rename_all = "camelCase")]
pub struct HintPosition {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

/// 窗口相关 Tauri 命令
#[tauri::command]
pub fn show_frontstage(app: AppHandle) -> Result<(), AppError> {
    WindowManager::show_frontstage(&app)
}

#[tauri::command]
pub fn hide_frontstage(app: AppHandle) -> Result<(), AppError> {
    WindowManager::hide_frontstage(&app)
}

#[tauri::command]
pub fn toggle_frontstage(app: AppHandle) -> Result<bool, AppError> {
    WindowManager::toggle_frontstage(&app)
}

#[tauri::command]
pub fn get_window_state(app: AppHandle) -> Result<WindowState, AppError> {
    WindowManager::get_window_state(&app)
}

#[tauri::command]
pub fn update_frontstage_content(
    app: AppHandle,
    text: String,
    chapter_id: String,
) -> Result<(), AppError> {
    let event = FrontstageEvent::ContentUpdate { text, chapter_id };
    WindowManager::send_to_frontstage(&app, event)
}

// =============================================================================
// Phase 1.1: TypeScript 绑定导出测试
// =============================================================================

#[cfg(test)]
mod ts_export_tests {
    use std::path::PathBuf;

    use super::*;

    /// 将窗口事件类型导出为 TypeScript 类型定义。
    #[test]
    fn export_window_event_types() {
        let export_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src-frontend/src/generated");

        std::fs::create_dir_all(&export_dir).expect("创建 generated 目录失败");

        FrontstageEvent::export_all_to(&export_dir).expect("导出 FrontstageEvent 失败");
        BackstageEvent::export_all_to(&export_dir).expect("导出 BackstageEvent 失败");
        HintPosition::export_all_to(&export_dir).expect("导出 HintPosition 失败");

        assert!(
            export_dir.join("FrontstageEvent.ts").exists(),
            "FrontstageEvent.ts 未生成"
        );
        assert!(
            export_dir.join("BackstageEvent.ts").exists(),
            "BackstageEvent.ts 未生成"
        );

        println!("✅ 窗口事件 TypeScript 绑定已导出到: {:?}", export_dir);
    }
}
