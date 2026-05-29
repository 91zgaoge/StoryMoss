use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingAnalytics {
    pub story_id: String,
    pub total_words: i64,
    pub total_scenes: i32,
    pub writing_streak: WritingStreak,
    pub productivity_score: f32,
    pub avg_words_per_day: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStreak {
    pub current_streak: i32,
    pub longest_streak: i32,
    pub last_writing_date: Option<NaiveDate>,
}

pub struct AnalyticsEngine;

impl AnalyticsEngine {
    pub fn new() -> Self {
        Self
    }

    /// 基于 Scene 架构的写作分析
    /// 从 content 实时计算中文字数，以 updated_at 作为写作日期
    pub fn analyze_writing_data(
        &self,
        story_id: &str,
        scenes: &[crate::db::models::Scene],
    ) -> WritingAnalytics {
        let total_words: i64 = scenes
            .iter()
            .map(|s| {
                crate::utils::text::TextUtils::chinese_word_count(
                    s.content.as_deref().unwrap_or(""),
                ) as i64
            })
            .sum();
        let total_scenes = scenes.len() as i32;

        // Calculate writing streak from scene updated_at dates
        let mut dates: Vec<NaiveDate> = scenes.iter().map(|s| s.updated_at.date_naive()).collect();
        dates.sort_unstable();
        dates.dedup();
        dates.reverse();

        let (current_streak, longest_streak, last_writing_date) = if dates.is_empty() {
            (0, 0, None)
        } else {
            let today = Utc::now().date_naive();
            let mut longest = 0;
            let mut streak = 0;
            let mut prev_date = today.succ_opt().unwrap_or(today);

            for &date in &dates {
                if prev_date.succ_opt() == Some(date) || prev_date == date {
                    streak += 1;
                } else {
                    longest = longest.max(streak);
                    streak = 1;
                }
                prev_date = date;
            }
            longest = longest.max(streak);

            // Current streak: count backwards from today
            let mut check_date = today;
            let mut curr_streak = 0;
            let date_set: std::collections::HashSet<_> = dates.iter().cloned().collect();
            while date_set.contains(&check_date) {
                curr_streak += 1;
                check_date = check_date.pred_opt().unwrap_or(check_date);
            }
            // If no writing today, check yesterday
            if curr_streak == 0 {
                check_date = today.pred_opt().unwrap_or(today);
                while date_set.contains(&check_date) {
                    curr_streak += 1;
                    check_date = check_date.pred_opt().unwrap_or(check_date);
                }
            }

            (curr_streak, longest, Some(dates[0]))
        };

        let writing_days = dates.len().max(1) as i32;
        let avg_words_per_day = if writing_days > 0 {
            (total_words as f32) / (writing_days as f32)
        } else {
            0.0
        };

        // Productivity score: combination of consistency and output
        let consistency_factor = (current_streak as f32).min(30.0) / 30.0;
        let output_factor = (avg_words_per_day / 2000.0).min(1.0);
        let productivity_score = (consistency_factor * 50.0 + output_factor * 50.0).min(100.0);

        WritingAnalytics {
            story_id: story_id.to_string(),
            total_words,
            total_scenes,
            writing_streak: WritingStreak {
                current_streak,
                longest_streak,
                last_writing_date,
            },
            productivity_score,
            avg_words_per_day,
        }
    }
}
