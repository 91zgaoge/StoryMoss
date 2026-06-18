//! Opening Clarity Gate — 开篇清晰度门骨架（v0.17.1）
//!
//! 设计目的：在场景/章节开篇的前 200 字内，检查是否同时满足核心要素：
//! 1. 题材一致性（与 GenreProfile.canonical_name 的 reader_promise 对齐）
//! 2. 危险（danger）/ 羞辱（humiliation）/ 失去（loss）/
//!    谜题（puzzle）至少出现一项
//! 3. 物理锚点（physical anchor）：具体可视化的人/地/物
//!
//! v0.17.1 阶段说明：
//! - 这是骨架实现：所有 public API 已就绪，但仅做轻量启发式检测，
//!   不接入主创作流程。
//! - v0.17.2 将把它接入 [`crate::task_system::audit_executor`] 的 11 维评估，
//!   并允许 GenreProfile 自定义钩子。
//!
//! 不接入生产：本模块当前不被任何业务路径调用。

use serde::{Deserialize, Serialize};

/// 开篇要素类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpeningElement {
    Danger,
    Humiliation,
    Loss,
    Puzzle,
    PhysicalAnchor,
    GenreSignal,
}

/// 开篇清晰度评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningClarityReport {
    /// 是否通过门控（至少 4/6 要素命中且必含 PhysicalAnchor）
    pub passed: bool,
    /// 命中的要素列表
    pub hits: Vec<OpeningElement>,
    /// 缺失的要素列表
    pub misses: Vec<OpeningElement>,
    /// 综合得分 0-100
    pub score: f64,
    /// 简短理由（前端展示）
    pub rationale: String,
}

/// 开篇清晰度门
pub struct OpeningClarityGate {
    /// 评估窗口：从开篇起检查多少字符（中文按 char 计数）
    pub window_chars: usize,
}

impl Default for OpeningClarityGate {
    fn default() -> Self {
        Self { window_chars: 200 }
    }
}

impl OpeningClarityGate {
    pub fn new(window_chars: usize) -> Self {
        Self { window_chars }
    }

    /// 评估开篇清晰度。骨架阶段使用关键词启发式，不调用 LLM。
    ///
    /// `genre_canonical` 是 GenreProfile 的
    /// canonical_name（例如「都市·赘婿·扮猪吃虎」）， 当 None 时跳过
    /// GenreSignal 检查。
    pub fn evaluate(
        &self,
        opening_text: &str,
        genre_canonical: Option<&str>,
    ) -> OpeningClarityReport {
        let snippet: String = opening_text.chars().take(self.window_chars).collect();

        let mut hits: Vec<OpeningElement> = Vec::new();
        let mut misses: Vec<OpeningElement> = Vec::new();

        // 1. Danger
        if contains_any(
            &snippet,
            &[
                "杀", "死", "刀", "血", "敌", "危险", "追", "袭", "炸", "枪", "炮", "毒", "陷阱",
            ],
        ) {
            hits.push(OpeningElement::Danger);
        } else {
            misses.push(OpeningElement::Danger);
        }

        // 2. Humiliation
        if contains_any(
            &snippet,
            &[
                "嘲",
                "辱",
                "讽",
                "看不起",
                "瞧不起",
                "废物",
                "丢脸",
                "羞",
                "贱",
            ],
        ) {
            hits.push(OpeningElement::Humiliation);
        } else {
            misses.push(OpeningElement::Humiliation);
        }

        // 3. Loss
        if contains_any(
            &snippet,
            &[
                "失去", "夺走", "破碎", "倒塌", "死了", "走了", "再也", "丢", "逝", "不复",
            ],
        ) {
            hits.push(OpeningElement::Loss);
        } else {
            misses.push(OpeningElement::Loss);
        }

        // 4. Puzzle
        if contains_any(
            &snippet,
            &[
                "为何",
                "为什么",
                "怎么会",
                "诡异",
                "异常",
                "疑",
                "秘密",
                "谜",
                "诡谲",
                "不对劲",
            ],
        ) {
            hits.push(OpeningElement::Puzzle);
        } else {
            misses.push(OpeningElement::Puzzle);
        }

        // 5. PhysicalAnchor —
        //    至少有一个具体名词锚定（粗略：包含「：」「。」之外的句号且字数≥40）
        if snippet.chars().count() >= 40 {
            hits.push(OpeningElement::PhysicalAnchor);
        } else {
            misses.push(OpeningElement::PhysicalAnchor);
        }

        // 6. GenreSignal
        if let Some(canonical) = genre_canonical {
            if signal_for_genre(canonical, &snippet) {
                hits.push(OpeningElement::GenreSignal);
            } else {
                misses.push(OpeningElement::GenreSignal);
            }
        }

        let total = hits.len() + misses.len();
        let score = if total == 0 {
            0.0
        } else {
            (hits.len() as f64) * 100.0 / (total as f64)
        };

        let must_have_anchor = hits.contains(&OpeningElement::PhysicalAnchor);
        let passed = must_have_anchor && hits.len() >= 4;

        let rationale = if passed {
            format!("开篇命中 {}/{} 要素，物理锚点存在，达标", hits.len(), total)
        } else if !must_have_anchor {
            "开篇缺少物理锚点（具体可视化场景），读者难以入戏".to_string()
        } else {
            format!(
                "开篇仅命中 {}/{} 要素，建议增加危险/羞辱/失去/谜题之一",
                hits.len(),
                total
            )
        };

        OpeningClarityReport {
            passed,
            hits,
            misses,
            score,
            rationale,
        }
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

/// 题材专属信号：为不同 GenreProfile 提供差异化检测词。
fn signal_for_genre(canonical: &str, snippet: &str) -> bool {
    if canonical.contains("赘婿") || canonical.contains("扮猪吃虎") {
        return contains_any(snippet, &["岳", "妻", "婿", "豪门", "千金", "倒插门"]);
    }
    if canonical.contains("修真") || canonical.contains("玄幻") {
        return contains_any(
            snippet,
            &["灵", "气", "真", "丹", "宗", "派", "剑", "诀", "境"],
        );
    }
    if canonical.contains("末世") || canonical.contains("丧尸") {
        return contains_any(snippet, &["丧尸", "尸", "末日", "感染", "病毒", "废墟"]);
    }
    if canonical.contains("悬疑") || canonical.contains("推理") {
        return contains_any(snippet, &["案", "尸", "凶", "嫌", "证", "侦"]);
    }
    if canonical.contains("校园") || canonical.contains("青春") {
        return contains_any(snippet, &["教室", "操场", "同学", "校", "课", "考"]);
    }
    // 默认：题材未识别时不扣分
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_fails_gate() {
        let gate = OpeningClarityGate::default();
        let report = gate.evaluate("", None);
        assert!(!report.passed);
        assert!(report.misses.contains(&OpeningElement::PhysicalAnchor));
    }

    #[test]
    fn rich_opening_passes_without_genre() {
        let gate = OpeningClarityGate::default();
        // 含 危险/羞辱/谜题 + 长度 ≥ 40 字符
        let text = concat!(
            "刀光在他喉前停住，血珠滚落在青砖上。",
            "众人嘲笑：连这点疼都受不住的废物。",
            "他低头，看见手心那枚诡异的符纹——为何会在这里？",
        );
        let report = gate.evaluate(text, None);
        assert!(report.score > 50.0);
        assert!(report.hits.contains(&OpeningElement::PhysicalAnchor));
    }

    #[test]
    fn genre_signal_detected_when_match() {
        let gate = OpeningClarityGate::default();
        let text = "他被赶出豪门，岳父冷笑：废物赘婿。这刀刃就贴在他的喉头。";
        let report = gate.evaluate(text, Some("都市·赘婿·扮猪吃虎"));
        assert!(report.hits.contains(&OpeningElement::GenreSignal));
    }

    #[test]
    fn genre_signal_missed_when_off_topic() {
        let gate = OpeningClarityGate::default();
        let text = "春日的午后，阳光穿过窗帘洒在书桌上。";
        let report = gate.evaluate(text, Some("修真·玄幻"));
        assert!(report.misses.contains(&OpeningElement::GenreSignal));
    }

    #[test]
    fn anchor_required_for_pass() {
        let gate = OpeningClarityGate::default();
        // 短到无法形成物理锚点
        let text = "杀。";
        let report = gate.evaluate(text, None);
        assert!(!report.passed);
        assert!(report.misses.contains(&OpeningElement::PhysicalAnchor));
    }
}
