//! 读者共鸣模型：四类读者人格 + 「感动」四问。
//!
//! 这是**预测**，不是测量。结果只做质量债与作者参考，不自动改稿。

use serde::{Deserialize, Serialize};

use super::types::{EmotionBeat, EmotionContract};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderScores {
    #[serde(default)]
    pub identification: f32,
    #[serde(default)]
    pub sympathy: f32,
    #[serde(default)]
    pub investment: f32,
    #[serde(default)]
    pub tension: f32,
    #[serde(default)]
    pub curiosity: f32,
    #[serde(default)]
    pub trust: f32,
    #[serde(default)]
    pub moved: f32,
}

impl ReaderScores {
    pub fn engagement(&self) -> f32 {
        (0.30 * self.identification
            + 0.25 * self.sympathy
            + 0.20 * self.tension
            + 0.15 * self.curiosity
            + 0.10 * self.trust)
            .clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderReview {
    pub persona: String,
    #[serde(default)]
    pub scores: ReaderScores,
    #[serde(default)]
    pub moved_moment: Option<String>,
    #[serde(default)]
    pub flat_moment: Option<String>,
    #[serde(default)]
    pub verdict: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderAggregate {
    pub reviews: usize,
    pub scores: ReaderScores,
    pub verdict: String,
    pub moved_moments: Vec<String>,
    pub flat_moments: Vec<String>,
}

pub fn aggregate_reviews(reviews: &[ReaderReview]) -> ReaderAggregate {
    if reviews.is_empty() {
        return ReaderAggregate::default();
    }
    let median = |f: fn(&ReaderScores) -> f32| -> f32 {
        let mut values: Vec<f32> = reviews.iter().map(|r| f(&r.scores)).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = values.len() / 2;
        if values.len() % 2 == 0 {
            (values[mid - 1] + values[mid]) / 2.0
        } else {
            values[mid]
        }
    };
    let scores = ReaderScores {
        identification: median(|s| s.identification),
        sympathy: median(|s| s.sympathy),
        investment: median(|s| s.investment),
        tension: median(|s| s.tension),
        curiosity: median(|s| s.curiosity),
        trust: median(|s| s.trust),
        moved: median(|s| s.moved),
    };
    let unearned = reviews
        .iter()
        .filter(|r| r.verdict == "unearned" || r.verdict == "melodramatic")
        .count();
    let verdict = if unearned >= 2 {
        "unearned".to_string()
    } else if scores.moved >= 0.70 {
        "moved".to_string()
    } else if scores.moved >= 0.50 {
        "promising".to_string()
    } else {
        "flat".to_string()
    };
    ReaderAggregate {
        reviews: reviews.len(),
        scores,
        verdict,
        moved_moments: reviews
            .iter()
            .filter_map(|r| r.moved_moment.clone())
            .take(3)
            .collect(),
        flat_moments: reviews
            .iter()
            .filter_map(|r| r.flat_moment.clone())
            .take(3)
            .collect(),
    }
}

pub const READER_PERSONAS: [&str; 4] = ["genre", "character", "literary", "skeptical"];

pub fn build_reader_panel_prompt(
    persona: &str,
    text: &str,
    beat: &EmotionBeat,
    contract: Option<&EmotionContract>,
) -> String {
    let contract_text = contract
        .map(|c| {
            format!(
                "核心情感问题：{}\n读者承诺：{}\n结局余味：{}\n禁止：{}",
                c.core_question,
                c.reader_promise,
                c.ending_residue,
                c.forbidden.join("、")
            )
        })
        .unwrap_or_else(|| "（未设置情感契约）".to_string());
    format!(
        "你是读者共鸣评审。你现在的读者人格是：{persona}（genre=类型读者；character=人物读者；literary=文学读者；skeptical=挑剔读者）。只输出一个 JSON 对象：\n\
         {{\"persona\":\"{persona}\",\"scores\":{{\"identification\":0-1,\"sympathy\":0-1,\"investment\":0-1,\"tension\":0-1,\"curiosity\":0-1,\"trust\":0-1,\"moved\":0-1}},\"moved_moment\":\"引用原文，没有则空\",\"flat_moment\":\"引用原文，没有则空\",\"verdict\":\"promising|flat|unearned|melodramatic\"}}\n\n\
         【感动四问】1. 读者是否知道角色最想要什么、最怕什么？2. 角色为这个想要付出了什么可见代价？3. 这个选择是否既出人意料、又能在前文找到依据？4. 叙述是否克制（没有替读者哭、没有解释主题）？1、2 必须为是；3、4 至少一个为是。\n\n\
         【本拍情感目标】读者感受：{}；为什么：{}\n【故事契约】{}\n\n【章节正文】\n{}",
        beat.reader_target.primary,
        beat.reader_target.why,
        contract_text,
        text.chars().take(6000).collect::<String>()
    )
}

pub fn parse_reader_review(raw: &str) -> Option<ReaderReview> {
    if let Ok(v) = serde_json::from_str::<ReaderReview>(raw.trim()) {
        return Some(v);
    }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<ReaderReview>(&raw[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(persona: &str, moved: f32, verdict: &str) -> ReaderReview {
        ReaderReview {
            persona: persona.into(),
            scores: ReaderScores {
                identification: 0.8,
                sympathy: 0.7,
                investment: 0.7,
                tension: 0.6,
                curiosity: 0.6,
                trust: 0.8,
                moved,
            },
            moved_moment: Some("她把伞留在门口".into()),
            flat_moment: None,
            verdict: verdict.into(),
        }
    }

    #[test]
    fn aggregate_uses_median_moved() {
        let reviews = vec![
            review("a", 0.4, "flat"),
            review("b", 0.7, "promising"),
            review("c", 0.9, "moved"),
        ];
        let agg = aggregate_reviews(&reviews);
        assert!((agg.scores.moved - 0.7).abs() < 1e-5);
        assert_eq!(agg.verdict, "moved");
    }

    #[test]
    fn two_unearned_reviews_override_high_scores() {
        let reviews = vec![
            review("a", 0.9, "unearned"),
            review("b", 0.9, "melodramatic"),
            review("c", 0.9, "moved"),
        ];
        let agg = aggregate_reviews(&reviews);
        assert_eq!(agg.verdict, "unearned");
    }

    #[test]
    fn parse_review_accepts_fenced_json() {
        let raw = "```json\n{\"persona\":\"character\",\"scores\":{\"moved\":0.8},\"verdict\":\"promising\"}\n```";
        let v = parse_reader_review(raw).unwrap();
        assert_eq!(v.persona, "character");
        assert!((v.scores.moved - 0.8).abs() < 1e-5);
    }
}
