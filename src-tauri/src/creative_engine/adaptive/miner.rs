//! 偏好挖掘引擎
//!
//! 从用户反馈日志中提取稳定偏好。
//! 基于启发式规则分析反馈模式，推断用户偏好。
//!
//! 挖掘维度：
//! - 内容偏好：对话比例、描写比例、叙事节奏
//! - 风格偏好：文风倾向、词汇选择
//! - 结构偏好：段落长度、场景切换频率
//! - 对话偏好：对话长度、对话标签风格

use super::feedback::FeedbackRecorder;
use crate::{
    db::{
        repositories::{UserFeedbackRepository, UserPreferenceRepository},
        DbPool,
    },
    error::AppError,
};

/// 挖掘出的偏好
#[derive(Debug, Clone)]
pub struct MinedPreference {
    pub preference_type: String,
    pub preference_key: String,
    pub preference_value: String,
    pub confidence: f32,
    pub evidence_count: i32,
    pub reasoning: String,
}

/// 偏好挖掘引擎
pub struct PreferenceMiner {
    pool: DbPool,
}

impl PreferenceMiner {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 挖掘故事的所有偏好
    pub fn mine(&self, story_id: &str) -> Result<Vec<MinedPreference>, AppError> {
        let mut preferences = Vec::new();

        // 1. 获取反馈日志
        let feedback_repo = UserFeedbackRepository::new(self.pool.clone());
        let logs = feedback_repo
            .get_by_story(story_id, Some(100))
            .map_err(AppError::from)?;

        if logs.is_empty() {
            return Ok(preferences);
        }

        // 2. 挖掘各维度偏好
        preferences.extend(self.mine_dialogue_preference(&logs));
        preferences.extend(self.mine_description_preference(&logs));
        preferences.extend(self.mine_pacing_preference(&logs));
        preferences.extend(self.mine_style_preference(&logs));
        preferences.extend(self.mine_narrative_preference(&logs));

        // 3. 保存到数据库
        let pref_repo = UserPreferenceRepository::new(self.pool.clone());
        for pref in &preferences {
            if pref.confidence >= 0.6 {
                let _ = pref_repo.upsert(
                    story_id,
                    &pref.preference_type,
                    &pref.preference_key,
                    &pref.preference_value,
                    pref.confidence,
                    pref.evidence_count,
                );
            }
        }

        Ok(preferences)
    }

    /// 挖掘对话偏好
    fn mine_dialogue_preference(
        &self,
        logs: &[crate::db::models::UserFeedbackLog],
    ) -> Vec<MinedPreference> {
        let mut preferences = Vec::new();

        // 计算接受/拒绝/修改中对话的比例差异
        let mut accept_dialogue_ratio = 0.0f32;
        let mut accept_count = 0;
        let mut reject_dialogue_ratio = 0.0f32;
        let mut reject_count = 0;

        for log in logs {
            let ratio = estimate_dialogue_ratio(&log.original_ai_text);
            match log.feedback_type {
                crate::db::models::FeedbackType::Accept => {
                    accept_dialogue_ratio += ratio;
                    accept_count += 1;
                }
                crate::db::models::FeedbackType::Reject => {
                    reject_dialogue_ratio += ratio;
                    reject_count += 1;
                }
                crate::db::models::FeedbackType::Modify => {
                    // 修改的情况：比较原文和修改后的对话比例变化
                    let original_ratio = estimate_dialogue_ratio(&log.original_ai_text);
                    let final_ratio = estimate_dialogue_ratio(&log.final_text);
                    if final_ratio > original_ratio {
                        accept_dialogue_ratio += final_ratio;
                        accept_count += 1;
                    } else {
                        reject_dialogue_ratio += original_ratio;
                        reject_count += 1;
                    }
                }
            }
        }

        if accept_count > 0 && reject_count > 0 {
            let avg_accept = accept_dialogue_ratio / accept_count as f32;
            let avg_reject = reject_dialogue_ratio / reject_count as f32;
            let diff = avg_accept - avg_reject;

            if diff.abs() > 0.1 {
                let (pref_value, reasoning) = if diff > 0.0 {
                    (
                        "prefer_more_dialogue".to_string(),
                        format!(
                            "用户接受的内容平均对话比例 {:.0}%，拒绝的内容平均 {:.0}%",
                            avg_accept * 100.0,
                            avg_reject * 100.0
                        ),
                    )
                } else {
                    (
                        "prefer_less_dialogue".to_string(),
                        format!(
                            "用户拒绝的内容平均对话比例 {:.0}%，接受的内容平均 {:.0}%",
                            avg_reject * 100.0,
                            avg_accept * 100.0
                        ),
                    )
                };

                preferences.push(MinedPreference {
                    preference_type: "dialogue".to_string(),
                    preference_key: "dialogue_ratio".to_string(),
                    preference_value: pref_value,
                    confidence: diff.abs().min(1.0),
                    evidence_count: accept_count + reject_count,
                    reasoning,
                });
            }
        }

        preferences
    }

    /// 挖掘描写偏好
    fn mine_description_preference(
        &self,
        logs: &[crate::db::models::UserFeedbackLog],
    ) -> Vec<MinedPreference> {
        let mut preferences = Vec::new();

        let mut accept_desc = 0.0f32;
        let mut accept_count = 0;
        let mut reject_desc = 0.0f32;
        let mut reject_count = 0;

        for log in logs {
            let ratio = estimate_description_ratio(&log.original_ai_text);
            match log.feedback_type {
                crate::db::models::FeedbackType::Accept => {
                    accept_desc += ratio;
                    accept_count += 1;
                }
                crate::db::models::FeedbackType::Reject => {
                    reject_desc += ratio;
                    reject_count += 1;
                }
                _ => {}
            }
        }

        if accept_count > 0 && reject_count > 0 {
            let avg_accept = accept_desc / accept_count as f32;
            let avg_reject = reject_desc / reject_count as f32;
            let diff = avg_accept - avg_reject;

            if diff.abs() > 0.1 {
                let (pref_value, reasoning) = if diff > 0.0 {
                    (
                        "prefer_more_description".to_string(),
                        format!(
                            "接受的内容环境描写更丰富（{:.0}% vs {:.0}%）",
                            avg_accept * 100.0,
                            avg_reject * 100.0
                        ),
                    )
                } else {
                    (
                        "prefer_less_description".to_string(),
                        format!(
                            "拒绝的内容环境描写过多（{:.0}% vs {:.0}%）",
                            avg_reject * 100.0,
                            avg_accept * 100.0
                        ),
                    )
                };

                preferences.push(MinedPreference {
                    preference_type: "content".to_string(),
                    preference_key: "description_ratio".to_string(),
                    preference_value: pref_value,
                    confidence: diff.abs().min(1.0),
                    evidence_count: accept_count + reject_count,
                    reasoning,
                });
            }
        }

        preferences
    }

    /// 挖掘节奏偏好
    fn mine_pacing_preference(
        &self,
        logs: &[crate::db::models::UserFeedbackLog],
    ) -> Vec<MinedPreference> {
        let mut preferences = Vec::new();

        let mut accept_sentence_len = 0.0f32;
        let mut accept_count = 0;
        let mut reject_sentence_len = 0.0f32;
        let mut reject_count = 0;

        for log in logs {
            let avg_len = estimate_avg_sentence_length(&log.original_ai_text);
            match log.feedback_type {
                crate::db::models::FeedbackType::Accept => {
                    accept_sentence_len += avg_len;
                    accept_count += 1;
                }
                crate::db::models::FeedbackType::Reject => {
                    reject_sentence_len += avg_len;
                    reject_count += 1;
                }
                _ => {}
            }
        }

        if accept_count > 0 && reject_count > 0 {
            let avg_accept = accept_sentence_len / accept_count as f32;
            let avg_reject = reject_sentence_len / reject_count as f32;
            let diff = avg_accept - avg_reject;

            if diff.abs() > 5.0 {
                let (pref_value, reasoning) = if diff > 0.0 {
                    (
                        "prefer_slower_pacing".to_string(),
                        format!(
                            "接受的内容平均句长 {:.0} 字，拒绝的 {:.0} 字",
                            avg_accept, avg_reject
                        ),
                    )
                } else {
                    (
                        "prefer_faster_pacing".to_string(),
                        format!(
                            "拒绝的内容平均句长 {:.0} 字，接受的 {:.0} 字",
                            avg_reject, avg_accept
                        ),
                    )
                };

                preferences.push(MinedPreference {
                    preference_type: "pacing".to_string(),
                    preference_key: "sentence_length".to_string(),
                    preference_value: pref_value,
                    confidence: (diff.abs() / 30.0).min(1.0),
                    evidence_count: accept_count + reject_count,
                    reasoning,
                });
            }
        }

        preferences
    }

    /// 挖掘风格偏好（基于接受/拒绝的统计）
    fn mine_style_preference(
        &self,
        logs: &[crate::db::models::UserFeedbackLog],
    ) -> Vec<MinedPreference> {
        let mut preferences = Vec::new();

        let stats = FeedbackRecorder::new(self.pool.clone())
            .get_stats(&logs.first().map(|l| l.story_id.clone()).unwrap_or_default());
        if let Ok(stats) = stats {
            let total = stats.accept + stats.reject + stats.modify;
            if total > 5 {
                let accept_rate = stats.accept as f32 / total as f32;
                let reject_rate = stats.reject as f32 / total as f32;

                if reject_rate > 0.4 {
                    preferences.push(MinedPreference {
                        preference_type: "style".to_string(),
                        preference_key: "overall_satisfaction".to_string(),
                        preference_value: "needs_improvement".to_string(),
                        confidence: reject_rate.min(1.0),
                        evidence_count: total as i32,
                        reasoning: format!(
                            "拒绝率 {:.0}% 较高，系统需要调整生成策略",
                            reject_rate * 100.0
                        ),
                    });
                } else if accept_rate > 0.7 {
                    preferences.push(MinedPreference {
                        preference_type: "style".to_string(),
                        preference_key: "overall_satisfaction".to_string(),
                        preference_value: "high_satisfaction".to_string(),
                        confidence: accept_rate.min(1.0),
                        evidence_count: total as i32,
                        reasoning: format!(
                            "接受率 {:.0}% 较高，当前策略匹配用户偏好",
                            accept_rate * 100.0
                        ),
                    });
                }
            }
        }

        preferences
    }

    /// 挖掘叙事偏好
    fn mine_narrative_preference(
        &self,
        logs: &[crate::db::models::UserFeedbackLog],
    ) -> Vec<MinedPreference> {
        let mut preferences = Vec::new();

        // 检查用户修改的内容中是否经常添加/删除内心独白
        let mut added_interior = 0;
        let mut removed_interior = 0;
        let mut total_modify = 0;

        for log in logs {
            if log.feedback_type == crate::db::models::FeedbackType::Modify {
                let original_interior = count_interior_monologue(&log.original_ai_text);
                let final_interior = count_interior_monologue(&log.final_text);
                if final_interior > original_interior {
                    added_interior += 1;
                } else if final_interior < original_interior {
                    removed_interior += 1;
                }
                total_modify += 1;
            }
        }

        if total_modify >= 3 {
            let diff: i32 = added_interior - removed_interior;
            if diff.abs() >= 2 {
                let (pref_value, reasoning) = if diff > 0 {
                    (
                        "prefer_more_interior_monologue".to_string(),
                        format!(
                            "用户修改时 {} 次增加内心独白，{} 次减少",
                            added_interior, removed_interior
                        ),
                    )
                } else {
                    (
                        "prefer_less_interior_monologue".to_string(),
                        format!(
                            "用户修改时 {} 次减少内心独白，{} 次增加",
                            removed_interior, added_interior
                        ),
                    )
                };

                preferences.push(MinedPreference {
                    preference_type: "content".to_string(),
                    preference_key: "interior_monologue".to_string(),
                    preference_value: pref_value,
                    confidence: (diff.abs() as f32 / total_modify as f32).min(1.0),
                    evidence_count: total_modify,
                    reasoning,
                });
            }
        }

        preferences
    }
}

// ==================== 文本分析辅助函数 ====================

/// 估算文本中的对话比例
fn estimate_dialogue_ratio(text: &str) -> f32 {
    let dialogue_markers = ['"', '「', '『'];
    let total_chars = text.chars().count();
    if total_chars == 0 {
        return 0.0;
    }

    let dialogue_chars = text
        .chars()
        .filter(|&c| dialogue_markers.contains(&c))
        .count()
        / 2;
    (dialogue_chars as f32 / total_chars as f32).min(1.0)
}

/// 估算文本中的环境描写比例（简化：基于自然意象词汇密度）
fn estimate_description_ratio(text: &str) -> f32 {
    let desc_markers = [
        "风", "雨", "雪", "月", "阳", "云", "山", "水", "花", "树", "天", "地", "光", "影", "色",
        "香", "声", "冷", "热",
    ];
    let total_chars = text.chars().count();
    if total_chars == 0 {
        return 0.0;
    }

    let desc_count: usize = desc_markers.iter().map(|&m| text.matches(m).count()).sum();
    (desc_count as f32 * 2.0 / total_chars as f32).min(1.0)
}

/// 估算平均句长
fn estimate_avg_sentence_length(text: &str) -> f32 {
    let sentences: Vec<&str> = text
        .split(['。', '！', '？', '.', '!', '?'])
        .filter(|s| !s.trim().is_empty())
        .collect();
    if sentences.is_empty() {
        return 0.0;
    }

    let total_len: usize = sentences.iter().map(|s| s.chars().count()).sum();
    total_len as f32 / sentences.len() as f32
}

/// 估算内心独白数量
fn count_interior_monologue(text: &str) -> usize {
    let markers = ["心想", "暗想", "觉得", "感到", "想道", "暗忖"];
    markers.iter().map(|&m| text.matches(m).count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_dialogue_ratio() {
        let text = "「你好。」他说。「再见。」她答。";
        let ratio = estimate_dialogue_ratio(text);
        assert!(ratio > 0.0);
    }

    #[test]
    fn test_estimate_avg_sentence_length() {
        let text = "今天天气很好。我们去公园吧。";
        let len = estimate_avg_sentence_length(text);
        assert!(len > 0.0);
    }

    #[test]
    fn test_count_interior_monologue() {
        let text = "他心想，这不可能。她暗想，也许吧。";
        let count = count_interior_monologue(text);
        assert_eq!(count, 2);
    }
}
