//! 偏好对导出器
//!
//! 将每次用户反馈追加写入 `.storyforge/feedback/preference_pairs.jsonl`，
//! 为后续 RLHF / 共同进化提供可训练数据。

use std::{fs::OpenOptions, io::Write, path::PathBuf};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{creative_engine::adaptive::FeedbackEvent, error::AppError};

const FEEDBACK_DIR: &str = "feedback";
const PREFERENCE_PAIRS_FILE: &str = "preference_pairs.jsonl";

#[derive(Debug, Serialize)]
pub struct PreferencePairRecord {
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub feedback_type: String,
    pub agent_type: Option<String>,
    pub original_prompt: Option<String>,
    pub generated_content: Option<String>,
    pub final_text: String,
    pub subsequent_edit_diff: Option<String>,
    pub ai_score: Option<f32>,
    pub user_satisfaction: Option<i32>,
    pub recorded_at: String,
}

pub struct PreferencePairExporter;

impl PreferencePairExporter {
    /// 将反馈事件追加为 `.storyforge/feedback/preference_pairs.jsonl` 的一行
    pub fn export(app: &AppHandle, event: &FeedbackEvent) -> Result<(), AppError> {
        let app_dir = app.path().app_data_dir().map_err(|e| AppError::Internal {
            message: format!("无法获取应用数据目录: {}", e),
        })?;
        let feedback_dir = app_dir
            .join("stories")
            .join(&event.story_id)
            .join(".storyforge")
            .join(FEEDBACK_DIR);
        std::fs::create_dir_all(&feedback_dir).map_err(|e| AppError::Internal {
            message: format!("创建 feedback 目录失败: {}", e),
        })?;

        let file_path = feedback_dir.join(PREFERENCE_PAIRS_FILE);
        let record = PreferencePairRecord {
            story_id: event.story_id.clone(),
            scene_id: event.scene_id.clone(),
            chapter_id: event.chapter_id.clone(),
            feedback_type: event.feedback_type.as_str().to_string(),
            agent_type: event.agent_type.clone(),
            original_prompt: event.original_prompt.clone(),
            generated_content: event.generated_content.clone(),
            final_text: event.final_text.clone(),
            subsequent_edit_diff: event.subsequent_edit_diff.clone(),
            ai_score: event.ai_score,
            user_satisfaction: event.user_satisfaction,
            recorded_at: Utc::now().to_rfc3339(),
        };
        let line = serde_json::to_string(&record).map_err(|e| AppError::Internal {
            message: format!("序列化偏好对失败: {}", e),
        })?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| AppError::Internal {
                message: format!("打开 preference_pairs.jsonl 失败: {}", e),
            })?;
        writeln!(file, "{}", line).map_err(|e| AppError::Internal {
            message: format!("写入 preference_pairs.jsonl 失败: {}", e),
        })?;

        log::info!(
            "[PreferencePairExporter] appended feedback to {}",
            file_path.display()
        );
        Ok(())
    }

    /// 返回指定故事的偏好对文件路径
    pub fn get_path(app: &AppHandle, story_id: &str) -> Result<PathBuf, AppError> {
        let app_dir = app.path().app_data_dir().map_err(|e| AppError::Internal {
            message: format!("无法获取应用数据目录: {}", e),
        })?;
        Ok(app_dir
            .join("stories")
            .join(story_id)
            .join(".storyforge")
            .join(FEEDBACK_DIR)
            .join(PREFERENCE_PAIRS_FILE))
    }
}
