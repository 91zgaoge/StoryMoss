//! 自动化相关的 IPC 命令

use std::collections::HashMap;

use tauri::{AppHandle, State, Wry};

use crate::automation::{
    handlers::AutomationHandler,
    service::AutomationService,
    triggers::{AutomationTrigger, TriggerEvent},
};

/// 触发自动化事件
#[tauri::command]
pub async fn trigger_automation_event(
    _app_handle: AppHandle<Wry>,
    automation_service: State<'_, AutomationService>,
    event: TriggerEvent,
) -> Result<(), String> {
    log::debug!("[IPC] trigger_automation_event: {:?}", event);

    automation_service.trigger_event(event).await?;
    Ok(())
}

/// 获取所有自动化触发器
#[tauri::command]
pub async fn get_automation_triggers(
    automation_service: State<'_, AutomationService>,
) -> Result<HashMap<String, AutomationTrigger>, String> {
    log::debug!("[IPC] get_automation_triggers");

    let triggers = automation_service.get_triggers().await;
    Ok(triggers)
}

/// 获取所有自动化处理器
#[tauri::command]
pub async fn get_automation_handlers(
    automation_service: State<'_, AutomationService>,
) -> Result<HashMap<String, AutomationHandler>, String> {
    log::debug!("[IPC] get_automation_handlers");

    let handlers = automation_service.get_handlers().await;
    Ok(handlers)
}

/// 添加自动化触发器
#[tauri::command]
pub async fn add_automation_trigger(
    automation_service: State<'_, AutomationService>,
    trigger: AutomationTrigger,
) -> Result<(), String> {
    log::debug!("[IPC] add_automation_trigger: {}", trigger.name);

    automation_service.add_trigger(trigger).await?;
    Ok(())
}

/// 添加自动化处理器
#[tauri::command]
pub async fn add_automation_handler(
    automation_service: State<'_, AutomationService>,
    handler: AutomationHandler,
) -> Result<(), String> {
    log::debug!("[IPC] add_automation_handler: {}", handler.name);

    automation_service.add_handler(handler).await?;
    Ok(())
}

/// 手动触发故事创建事件（用于测试）
#[tauri::command]
pub async fn trigger_story_created(
    automation_service: State<'_, AutomationService>,
    story_id: String,
) -> Result<(), String> {
    log::debug!("[IPC] trigger_story_created: {}", story_id);

    let event = TriggerEvent::StoryCreated { story_id };
    automation_service.trigger_event(event).await?;
    Ok(())
}

/// 手动触发章节创建事件（用于测试）
#[tauri::command]
pub async fn trigger_chapter_created(
    automation_service: State<'_, AutomationService>,
    story_id: String,
    chapter_id: String,
) -> Result<(), String> {
    log::debug!(
        "[IPC] trigger_chapter_created: {} -> {}",
        story_id,
        chapter_id
    );

    let event = TriggerEvent::ChapterCreated {
        story_id,
        chapter_id,
    };
    automation_service.trigger_event(event).await?;
    Ok(())
}

/// 手动触发角色创建事件（用于测试）
#[tauri::command]
pub async fn trigger_character_created(
    automation_service: State<'_, AutomationService>,
    story_id: String,
    character_id: String,
) -> Result<(), String> {
    log::debug!(
        "[IPC] trigger_character_created: {} -> {}",
        story_id,
        character_id
    );

    let event = TriggerEvent::CharacterCreated {
        story_id,
        character_id,
    };
    automation_service.trigger_event(event).await?;
    Ok(())
}

/// 手动触发章节内容更新事件（用于测试）
#[tauri::command]
pub async fn trigger_chapter_content_updated(
    automation_service: State<'_, AutomationService>,
    story_id: String,
    chapter_id: String,
    word_count: usize,
) -> Result<(), String> {
    log::debug!(
        "[IPC] trigger_chapter_content_updated: {} -> {} ({}字)",
        story_id,
        chapter_id,
        word_count
    );

    let event = TriggerEvent::ChapterContentUpdated {
        story_id,
        chapter_id,
        word_count,
    };
    automation_service.trigger_event(event).await?;
    Ok(())
}
