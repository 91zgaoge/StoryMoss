//! Reading Power Evaluator - 追读力评估器
//!
//! 评估章节的 hook、爽点、微兑现、债务等指标

use once_cell::sync::Lazy;
use regex::Regex;

/// 从章节内容中提取的追读力特征
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFeatures {
    pub hook_type: Option<String>,
    pub hook_signals: Vec<String>,
    pub coolpoint_patterns: Vec<String>,
    pub micropayoffs: Vec<String>,
    pub hard_violations: Vec<String>,
    pub soft_suggestions: Vec<String>,
    pub is_transition: bool,
}

/// 基于规则的内容特征提取器（面向中文网文）
pub struct ContentFeatureExtractor;

impl ContentFeatureExtractor {
    /// 从章节正文中提取追读力特征
    pub fn extract(content: &str) -> ContentFeatures {
        let tail = Self::tail(content, 200);
        let (hook_type, hook_signals) = Self::detect_hook(&tail, content);
        let coolpoint_patterns = Self::detect_coolpoints(content);
        let micropayoffs = Self::detect_micropayoffs(content);
        let is_transition = Self::detect_transition(content);

        ContentFeatures {
            hook_type,
            hook_signals,
            coolpoint_patterns,
            micropayoffs,
            hard_violations: Vec::new(),
            soft_suggestions: Vec::new(),
            is_transition,
        }
    }

    fn tail(content: &str, n: usize) -> String {
        content
            .chars()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn detect_hook(tail: &str, content: &str) -> (Option<String>, Vec<String>) {
        if content.is_empty() {
            return (None, Vec::new());
        }

        // 统一全角标点，便于规则匹配
        let tail = tail
            .replace('？', "?")
            .replace('！', "!")
            .replace('…', "...");

        let mut signals = Vec::new();

        let cliffhanger_markers = ["?", "...", "!"];
        let cliffhanger_words = ["没想到", "竟然", "难道", "果然", "突然", "下一秒"];
        let mystery_words = ["谜团", "真相", "秘密", "是谁", "为什么"];
        let emotional_words = ["心痛", "愤怒", "狂喜", "绝望", "不甘"];
        let action_words = ["杀", "逃", "追", "战", "出手", "动手"];

        for m in cliffhanger_markers {
            if tail.contains(m) {
                signals.push(format!("punctuation:{}", m));
            }
        }
        for w in cliffhanger_words {
            if tail.contains(w) {
                signals.push(format!("word:{}", w));
            }
        }

        let is_cliffhanger = !signals.is_empty();

        let mut mystery_signals = Vec::new();
        for w in mystery_words {
            if tail.contains(w) {
                mystery_signals.push(format!("mystery:{}", w));
            }
        }

        let mut emotional_signals = Vec::new();
        for w in emotional_words {
            if tail.contains(w) {
                emotional_signals.push(format!("emotion:{}", w));
            }
        }

        let mut action_signals = Vec::new();
        for w in action_words {
            if tail.contains(w) {
                action_signals.push(format!("action:{}", w));
            }
        }

        if is_cliffhanger {
            signals.extend(mystery_signals);
            signals.extend(emotional_signals);
            signals.extend(action_signals);
            return (Some("cliffhanger".to_string()), signals);
        }

        if !mystery_signals.is_empty() {
            signals = mystery_signals;
            signals.extend(emotional_signals);
            signals.extend(action_signals);
            return (Some("mystery".to_string()), signals);
        }

        if !emotional_signals.is_empty() {
            signals = emotional_signals;
            signals.extend(action_signals);
            return (Some("emotional".to_string()), signals);
        }

        if !action_signals.is_empty() {
            return (Some("action".to_string()), action_signals);
        }

        (Some("weak".to_string()), Vec::new())
    }

    fn detect_coolpoints(content: &str) -> Vec<String> {
        let mut patterns = Vec::new();

        let face_slapping = [
            "打脸",
            "嘲讽",
            "废物",
            "瞧不起",
            "看不起",
            "反转",
            "震惊",
            "全场寂静",
            "鸦雀无声",
        ];
        if face_slapping.iter().any(|w| content.contains(w)) {
            patterns.push("打脸".to_string());
        }

        static IDENTITY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"没想到.{0,15}是").unwrap());
        if content.contains("身份")
            || content.contains("真实身份")
            || content.contains("原来你是")
            || content.contains("竟然是")
            || IDENTITY_RE.is_match(content)
        {
            patterns.push("身份揭示".to_string());
        }

        let numeric = ["突破", "升级", "暴涨", "飙升"];
        let numeric_breakthrough = content.contains("突破") && content.contains("层");
        if numeric.iter().any(|w| content.contains(w)) || numeric_breakthrough {
            patterns.push("数值突破".to_string());
        }

        let showing_off = ["淡然", "不屑", "轻松", "碾压", "秒杀"];
        if showing_off.iter().any(|w| content.contains(w)) {
            patterns.push("装逼".to_string());
        }

        patterns
    }

    fn detect_micropayoffs(content: &str) -> Vec<String> {
        let payoff_words = ["承诺", "答应", "约定", "果然", "兑现", "如约", "没忘"];
        payoff_words
            .iter()
            .filter(|w| content.contains(*w))
            .map(|w| w.to_string())
            .collect()
    }

    fn detect_transition(content: &str) -> bool {
        if content.chars().count() < 500 {
            return true;
        }
        let transition_words = ["过渡", "转场", "翌日", "几天后", "与此同时"];
        transition_words.iter().any(|w| content.contains(w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_is_weak_and_transition() {
        let features = ContentFeatureExtractor::extract("");
        assert_eq!(features.hook_type, None);
        assert!(features.hook_signals.is_empty());
        assert!(features.coolpoint_patterns.is_empty());
        assert!(features.micropayoffs.is_empty());
        assert!(features.is_transition);
    }

    #[test]
    fn detects_cliffhanger_with_question() {
        let text = "主角走到门前，心中满是疑惑。这扇门后面到底藏着什么？";
        let features = ContentFeatureExtractor::extract(text);
        assert_eq!(features.hook_type, Some("cliffhanger".to_string()));
        assert!(features.hook_signals.iter().any(|s| s.contains("?")));
    }

    #[test]
    fn detects_mystery_hook() {
        let text = "他望着那封信，心中的谜团越来越深，真相若隐若现。";
        let features = ContentFeatureExtractor::extract(text);
        assert_eq!(features.hook_type, Some("mystery".to_string()));
    }

    #[test]
    fn detects_emotional_hook() {
        let text = "她咬紧牙关，眼中燃烧着愤怒的火焰，绝不甘就此认输。";
        let features = ContentFeatureExtractor::extract(text);
        assert_eq!(features.hook_type, Some("emotional".to_string()));
    }

    #[test]
    fn detects_action_hook() {
        let text = "敌人已经追到身后，他猛然回身，准备出手。";
        let features = ContentFeatureExtractor::extract(text);
        assert_eq!(features.hook_type, Some("action".to_string()));
    }

    #[test]
    fn detects_face_slap_coolpoint() {
        let text = "众人原本满脸嘲讽，认定他是个废物，结果下一秒全场寂静。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features.coolpoint_patterns.contains(&"打脸".to_string()));
    }

    #[test]
    fn detects_identity_reveal() {
        let text = "没想到站在眼前的竟然是他多年未见的好友。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features
            .coolpoint_patterns
            .contains(&"身份揭示".to_string()));
    }

    #[test]
    fn detects_numeric_breakthrough() {
        let text = "他一声低喝，修为再次突破，直接冲破第三层瓶颈。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features
            .coolpoint_patterns
            .contains(&"数值突破".to_string()));
    }

    #[test]
    fn detects_showing_off() {
        let text = "他淡然一笑，轻松躲过所有攻击，反手将对手秒杀。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features.coolpoint_patterns.contains(&"装逼".to_string()));
    }

    #[test]
    fn detects_micropayoffs() {
        let text = "他果然没有忘记当初的承诺，如约而至，兑现了约定。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features.micropayoffs.contains(&"承诺".to_string()));
        assert!(features.micropayoffs.contains(&"果然".to_string()));
        assert!(features.micropayoffs.contains(&"如约".to_string()));
        assert!(features.micropayoffs.contains(&"兑现".to_string()));
        assert!(features.micropayoffs.contains(&"约定".to_string()));
    }

    #[test]
    fn detects_transition_by_length() {
        let text = "翌日，阳光洒落。"; // short
        let features = ContentFeatureExtractor::extract(text);
        assert!(features.is_transition);
    }

    #[test]
    fn detects_transition_by_keyword() {
        let long_text = "a".repeat(600);
        let text = format!("{}几天后，风云再起。", long_text);
        let features = ContentFeatureExtractor::extract(&text);
        assert!(features.is_transition);
    }

    #[test]
    fn non_transition_long_content() {
        let text = "a".repeat(1000);
        let features = ContentFeatureExtractor::extract(&text);
        assert!(!features.is_transition);
    }

    #[test]
    fn multiple_coolpoints_reported() {
        let text = "他淡然出手，全场震惊，身份揭示的那一刻，众人鸦雀无声。";
        let features = ContentFeatureExtractor::extract(text);
        assert!(features.coolpoint_patterns.contains(&"打脸".to_string()));
        assert!(features.coolpoint_patterns.contains(&"装逼".to_string()));
        assert!(features
            .coolpoint_patterns
            .contains(&"身份揭示".to_string()));
    }
}
