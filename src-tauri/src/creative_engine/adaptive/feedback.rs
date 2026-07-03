#![allow(dead_code)]
//! 用户反馈记录器
//!
//! 记录每次用户对 AI 建议的反馈：接受/拒绝/修改。
//! 这是自适应学习的数据来源。

use crate::{
    db::{repositories::UserFeedbackRepository, DbPool},
    error::AppError,
};

/// 反馈事件
#[derive(Debug, Clone)]
pub struct FeedbackEvent {
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub feedback_type: FeedbackType,
    pub agent_type: Option<String>,
    pub original_ai_text: String,
    pub final_text: String,
    pub ai_score: Option<f32>,
    pub user_satisfaction: Option<i32>,
    pub original_prompt: Option<String>,
    pub generated_content: Option<String>,
    pub subsequent_edit_diff: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackType {
    Accept,
    Reject,
    Modify,
}

impl FeedbackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackType::Accept => "accept",
            FeedbackType::Reject => "reject",
            FeedbackType::Modify => "modify",
        }
    }
}

impl From<crate::db::models::FeedbackType> for FeedbackType {
    fn from(ft: crate::db::models::FeedbackType) -> Self {
        match ft {
            crate::db::models::FeedbackType::Accept => FeedbackType::Accept,
            crate::db::models::FeedbackType::Reject => FeedbackType::Reject,
            crate::db::models::FeedbackType::Modify => FeedbackType::Modify,
        }
    }
}

impl From<FeedbackType> for crate::db::models::FeedbackType {
    fn from(ft: FeedbackType) -> Self {
        match ft {
            FeedbackType::Accept => crate::db::models::FeedbackType::Accept,
            FeedbackType::Reject => crate::db::models::FeedbackType::Reject,
            FeedbackType::Modify => crate::db::models::FeedbackType::Modify,
        }
    }
}

/// 反馈记录器
pub struct FeedbackRecorder {
    pool: DbPool,
}

impl FeedbackRecorder {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 记录一条反馈
    pub fn record(&self, event: FeedbackEvent) -> Result<(), AppError> {
        let repo = UserFeedbackRepository::new(self.pool.clone());
        let metadata = serde_json::json!({
            "original_prompt": event.original_prompt,
            "generated_content": event.generated_content,
            "subsequent_edit_diff": event.subsequent_edit_diff,
        });
        repo.create(
            &event.story_id,
            event.scene_id.as_deref(),
            event.chapter_id.as_deref(),
            event.feedback_type.as_str(),
            event.agent_type.as_deref(),
            &event.original_ai_text,
            &event.final_text,
            event.ai_score,
            event.user_satisfaction,
            Some(&metadata),
        )
        .map_err(|e| format!("记录反馈失败: {}", e))?;

        Ok(())
    }

    /// 快捷方法：记录接受
    pub fn record_accept(
        &self,
        story_id: &str,
        original_text: &str,
        agent_type: Option<&str>,
    ) -> Result<(), AppError> {
        self.record(FeedbackEvent {
            story_id: story_id.to_string(),
            scene_id: None,
            chapter_id: None,
            feedback_type: FeedbackType::Accept,
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_text.to_string(),
            final_text: original_text.to_string(),
            ai_score: None,
            user_satisfaction: None,
            original_prompt: None,
            generated_content: None,
            subsequent_edit_diff: None,
        })
    }

    /// 快捷方法：记录拒绝
    pub fn record_reject(
        &self,
        story_id: &str,
        original_text: &str,
        agent_type: Option<&str>,
    ) -> Result<(), AppError> {
        self.record(FeedbackEvent {
            story_id: story_id.to_string(),
            scene_id: None,
            chapter_id: None,
            feedback_type: FeedbackType::Reject,
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_text.to_string(),
            final_text: String::new(),
            ai_score: None,
            user_satisfaction: None,
            original_prompt: None,
            generated_content: None,
            subsequent_edit_diff: None,
        })
    }

    /// 快捷方法：记录修改
    pub fn record_modify(
        &self,
        story_id: &str,
        original_text: &str,
        final_text: &str,
        agent_type: Option<&str>,
    ) -> Result<(), AppError> {
        self.record(FeedbackEvent {
            story_id: story_id.to_string(),
            scene_id: None,
            chapter_id: None,
            feedback_type: FeedbackType::Modify,
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_text.to_string(),
            final_text: final_text.to_string(),
            ai_score: None,
            user_satisfaction: None,
            original_prompt: None,
            generated_content: None,
            subsequent_edit_diff: None,
        })
    }

    /// 获取故事的反馈统计
    pub fn get_stats(&self, story_id: &str) -> Result<FeedbackStats, AppError> {
        let repo = UserFeedbackRepository::new(self.pool.clone());
        let db_stats = repo.get_stats(story_id).map_err(AppError::from)?;
        Ok(FeedbackStats {
            accept: db_stats.accept,
            reject: db_stats.reject,
            modify: db_stats.modify,
        })
    }

    /// 获取最近 N 天的反馈
    pub fn get_recent(&self, story_id: &str, days: i64) -> Result<Vec<FeedbackEvent>, AppError> {
        let repo = UserFeedbackRepository::new(self.pool.clone());
        let logs = repo.get_recent(story_id, days).map_err(AppError::from)?;
        Ok(logs
            .into_iter()
            .map(|l| {
                let (original_prompt, generated_content, subsequent_edit_diff) = l
                    .metadata
                    .as_ref()
                    .map(|m| {
                        (
                            m.get("original_prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            m.get("generated_content")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            m.get("subsequent_edit_diff")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    })
                    .unwrap_or((None, None, None));
                FeedbackEvent {
                    story_id: l.story_id,
                    scene_id: l.scene_id,
                    chapter_id: l.chapter_id,
                    feedback_type: l.feedback_type.into(),
                    agent_type: l.agent_type,
                    original_ai_text: l.original_ai_text,
                    final_text: l.final_text,
                    ai_score: l.ai_score,
                    user_satisfaction: l.user_satisfaction,
                    original_prompt,
                    generated_content,
                    subsequent_edit_diff,
                }
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct FeedbackStats {
    pub accept: i64,
    pub reject: i64,
    pub modify: i64,
}

impl From<crate::db::repositories::FeedbackStats> for FeedbackStats {
    fn from(s: crate::db::repositories::FeedbackStats) -> Self {
        Self {
            accept: s.accept,
            reject: s.reject,
            modify: s.modify,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_type_as_str() {
        assert_eq!(FeedbackType::Accept.as_str(), "accept");
        assert_eq!(FeedbackType::Reject.as_str(), "reject");
        assert_eq!(FeedbackType::Modify.as_str(), "modify");
    }

    #[test]
    fn test_feedback_type_roundtrip() {
        let db_type = crate::db::models::FeedbackType::Accept;
        let local_type: FeedbackType = db_type.into();
        let back: crate::db::models::FeedbackType = local_type.into();
        assert_eq!(back, crate::db::models::FeedbackType::Accept);
    }
}
