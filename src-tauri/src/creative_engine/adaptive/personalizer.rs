//! 个性化提示词系统
//!
//! 根据用户偏好构建个性化 system prompt 扩展，
//! 注入到 Writer 的提示词中，实现"越写越懂"。

use super::generator::{AdaptiveGenerator, GenerationStrategy};
use crate::{
    db::{repositories::UserPreferenceRepository, DbPool},
    error::AppError,
};

/// 个性化提示词构建器
pub struct PromptPersonalizer {
    pool: DbPool,
}

impl PromptPersonalizer {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 为故事构建个性化提示词扩展
    pub fn build_prompt_extension(&self, story_id: &str) -> Result<String, AppError> {
        let mut parts = Vec::new();

        // 1. 获取用户偏好
        let pref_repo = UserPreferenceRepository::new(self.pool.clone());
        let prefs = pref_repo.get_by_story(story_id).map_err(AppError::from)?;

        // 2. 过滤高置信度偏好
        let high_confidence: Vec<_> = prefs.into_iter().filter(|p| p.confidence >= 0.6).collect();

        if high_confidence.is_empty() {
            return Ok(String::new());
        }

        parts.push("\n【个性化偏好】".to_string());
        parts.push("根据你之前的写作习惯，系统了解到以下偏好：".to_string());

        // 3. 按类型分组
        let mut dialogue_prefs = Vec::new();
        let mut content_prefs = Vec::new();
        let mut pacing_prefs = Vec::new();
        let mut style_prefs = Vec::new();

        for pref in &high_confidence {
            match pref.preference_type.to_string().as_str() {
                "dialogue" => dialogue_prefs.push(pref),
                "content" => content_prefs.push(pref),
                "pacing" => pacing_prefs.push(pref),
                "style" => style_prefs.push(pref),
                _ => {}
            }
        }

        // 4. 生成各组提示词
        if !dialogue_prefs.is_empty() {
            parts.push("\n对话偏好：".to_string());
            for pref in &dialogue_prefs {
                let desc = self.describe_dialogue_preference(pref);
                parts.push(format!(
                    "- {}（置信度: {:.0}%）",
                    desc,
                    pref.confidence * 100.0
                ));
            }
        }

        if !content_prefs.is_empty() {
            parts.push("\n内容偏好：".to_string());
            for pref in &content_prefs {
                let desc = self.describe_content_preference(pref);
                parts.push(format!(
                    "- {}（置信度: {:.0}%）",
                    desc,
                    pref.confidence * 100.0
                ));
            }
        }

        if !pacing_prefs.is_empty() {
            parts.push("\n节奏偏好：".to_string());
            for pref in &pacing_prefs {
                let desc = self.describe_pacing_preference(pref);
                parts.push(format!(
                    "- {}（置信度: {:.0}%）",
                    desc,
                    pref.confidence * 100.0
                ));
            }
        }

        if !style_prefs.is_empty() {
            parts.push("\n整体风格：".to_string());
            for pref in &style_prefs {
                let desc = self.describe_style_preference(pref);
                parts.push(format!(
                    "- {}（置信度: {:.0}%）",
                    desc,
                    pref.confidence * 100.0
                ));
            }
        }

        // 5. 添加生成策略调整
        let generator = AdaptiveGenerator::new(self.pool.clone());
        if let Ok(strategy) = generator.build_strategy(story_id, None) {
            let strategy_prompt = AdaptiveGenerator::strategy_to_prompt(&strategy);
            if !strategy_prompt.is_empty() {
                parts.push(strategy_prompt);
            }
        }

        parts.push("\n请在续写时自动应用以上偏好，无需额外说明。".to_string());

        Ok(parts.join("\n"))
    }

    fn describe_dialogue_preference(&self, pref: &crate::db::models::UserPreference) -> String {
        match pref.preference_key.as_str() {
            "dialogue_ratio" => match pref.preference_value.as_str() {
                "prefer_more_dialogue" => "偏好更多对话".to_string(),
                "prefer_less_dialogue" => "偏好减少对话".to_string(),
                _ => pref.preference_value.clone(),
            },
            _ => pref.preference_value.clone(),
        }
    }

    fn describe_content_preference(&self, pref: &crate::db::models::UserPreference) -> String {
        match pref.preference_key.as_str() {
            "description_ratio" => match pref.preference_value.as_str() {
                "prefer_more_description" => "偏好更多环境描写".to_string(),
                "prefer_less_description" => "偏好减少环境描写".to_string(),
                _ => pref.preference_value.clone(),
            },
            "interior_monologue" => match pref.preference_value.as_str() {
                "prefer_more_interior_monologue" => "偏好更多内心独白".to_string(),
                "prefer_less_interior_monologue" => "偏好减少内心独白".to_string(),
                _ => pref.preference_value.clone(),
            },
            _ => pref.preference_value.clone(),
        }
    }

    fn describe_pacing_preference(&self, pref: &crate::db::models::UserPreference) -> String {
        match pref.preference_key.as_str() {
            "sentence_length" => match pref.preference_value.as_str() {
                "prefer_slower_pacing" => "偏好慢节奏（长句）".to_string(),
                "prefer_faster_pacing" => "偏好快节奏（短句）".to_string(),
                _ => pref.preference_value.clone(),
            },
            _ => pref.preference_value.clone(),
        }
    }

    fn describe_style_preference(&self, pref: &crate::db::models::UserPreference) -> String {
        match pref.preference_key.as_str() {
            "overall_satisfaction" => match pref.preference_value.as_str() {
                "needs_improvement" => "近期需要调整生成策略".to_string(),
                "high_satisfaction" => "当前风格匹配良好".to_string(),
                _ => pref.preference_value.clone(),
            },
            _ => pref.preference_value.clone(),
        }
    }

    /// 获取简短的状态摘要（用于调试/监控）
    pub fn get_preference_summary(&self, story_id: &str) -> Result<String, AppError> {
        let pref_repo = UserPreferenceRepository::new(self.pool.clone());
        let prefs = pref_repo.get_by_story(story_id).map_err(AppError::from)?;

        let total = prefs.len();
        let high_conf = prefs.iter().filter(|p| p.confidence >= 0.6).count();

        Ok(format!(
            "故事 {} 的偏好统计: 共 {} 条偏好，其中 {} 条高置信度(>=60%)",
            story_id, total, high_conf
        ))
    }
}

/// 个性化提示词结果
#[derive(Debug, Clone)]
pub struct PersonalizedPrompt {
    pub extension: String,
    pub strategy: GenerationStrategy,
    pub preference_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe_preferences() {
        let p = PromptPersonalizer::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );

        let pref = crate::db::models::UserPreference {
            id: "1".to_string(),
            story_id: "s1".to_string(),
            preference_type: crate::db::models::PreferenceType::Dialogue,
            preference_key: "dialogue_ratio".to_string(),
            preference_value: "prefer_more_dialogue".to_string(),
            confidence: 0.8,
            evidence_count: 10,
            updated_at: chrono::Local::now(),
        };

        assert_eq!(p.describe_dialogue_preference(&pref), "偏好更多对话");
    }
}
