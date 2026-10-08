//! Anti-AI 五维审查系统
//!
//! 检测 AI 生成文本的典型特征，输出五维评分和改进建议：
//! - 词汇维度 (Vocabulary)
//! - 语法维度 (Syntax)
//! - 叙事维度 (Narrative)
//! - 情感维度 (Emotion)
//! - 对话维度 (Dialogue)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// v0.17.1: 改写闸骨架（不接入生产）
pub mod rewriter;

/// 五维审查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiAiReview {
    pub overall_score: f64,
    pub dimensions: Vec<DimensionScore>,
    pub issues: Vec<ReviewIssue>,
    pub suggestions: Vec<String>,
    pub flagged_passages: Vec<FlaggedPassage>,
}

/// 单维度评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub name: String,
    pub score: f64,
    pub weight: f64,
    pub description: String,
}

/// 审查发现的问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub dimension: String,
    pub severity: String, // high | medium | low
    pub description: String,
    pub example: String,
    pub suggestion: String,
}

/// 被标记的段落
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlaggedPassage {
    pub text: String,
    pub dimension: String,
    pub reason: String,
    pub position: usize,
}

/// Anti-AI 审查器
pub struct AntiAiReviewer;

impl AntiAiReviewer {
    pub fn new() -> Self {
        Self
    }

    /// 执行五维审查
    pub fn review(&self, text: &str, genre: Option<&str>) -> AntiAiReview {
        let mut issues = Vec::new();
        let mut flagged = Vec::new();
        let mut suggestions = Vec::new();

        // 1. 词汇维度
        let vocab_result = self.check_vocabulary(text);
        issues.extend(vocab_result.issues);
        flagged.extend(vocab_result.flagged);
        suggestions.extend(vocab_result.suggestions);
        let vocab_score = vocab_result.score;

        // 2. 语法维度
        let syntax_result = self.check_syntax(text);
        issues.extend(syntax_result.issues);
        flagged.extend(syntax_result.flagged);
        suggestions.extend(syntax_result.suggestions);
        let syntax_score = syntax_result.score;

        // 3. 叙事维度
        let narrative_result = self.check_narrative(text, genre);
        issues.extend(narrative_result.issues);
        flagged.extend(narrative_result.flagged);
        suggestions.extend(narrative_result.suggestions);
        let narrative_score = narrative_result.score;

        // 4. 情感维度
        let emotion_result = self.check_emotion(text);
        issues.extend(emotion_result.issues);
        flagged.extend(emotion_result.flagged);
        suggestions.extend(emotion_result.suggestions);
        let emotion_score = emotion_result.score;

        // 5. 对话维度
        let dialogue_result = self.check_dialogue(text);
        issues.extend(dialogue_result.issues);
        flagged.extend(dialogue_result.flagged);
        suggestions.extend(dialogue_result.suggestions);
        let dialogue_score = dialogue_result.score;

        let dimensions = vec![
            DimensionScore {
                name: "词汇".to_string(),
                score: vocab_score,
                weight: 0.2,
                description: "词汇丰富度、AI 常用词检测".to_string(),
            },
            DimensionScore {
                name: "语法".to_string(),
                score: syntax_score,
                weight: 0.2,
                description: "句式多样性、修辞手法".to_string(),
            },
            DimensionScore {
                name: "叙事".to_string(),
                score: narrative_score,
                weight: 0.25,
                description: "叙事节奏、细节密度、视角一致性".to_string(),
            },
            DimensionScore {
                name: "情感".to_string(),
                score: emotion_score,
                weight: 0.2,
                description: "情感表达细腻度、避免标签化".to_string(),
            },
            DimensionScore {
                name: "对话".to_string(),
                score: dialogue_score,
                weight: 0.15,
                description: "对话自然度、角色个性".to_string(),
            },
        ];

        let overall_score: f64 = dimensions.iter().map(|d| d.score * d.weight).sum();

        AntiAiReview {
            overall_score: overall_score.max(0.0).min(1.0),
            dimensions,
            issues,
            suggestions,
            flagged_passages: flagged,
        }
    }

    // ==================== 词汇维度 ====================

    fn check_vocabulary(&self, text: &str) -> DimensionResult {
        let mut issues = Vec::new();
        let flagged = Vec::new();
        let mut suggestions = Vec::new();

        let ai_cliches = vec![
            "不言而喻",
            "显而易见",
            "毫无疑问",
            "众所周知",
            "不可否认",
            "值得一提的是",
            "从某种意义上说",
            "总的来说",
            "归根结底",
            "总而言之",
            "突然之间",
            "刹那间",
            "说时迟那时快",
            "嘴角微微上扬",
            "眼中闪过一丝",
            "心中涌起一股",
            // v0.17.1 中文叙事增强：补充 AI 高频提领句 / 综述句 / 廉价转折
            "关键在于",
            "值得注意的是",
            "综上所述",
            "让我们",
            "在某种程度上",
            "与此同时",
            "这一切的背后",
        ];

        let text_lower = text.to_lowercase();
        let mut cliche_count = 0;

        for cliche in &ai_cliches {
            if text_lower.contains(cliche) {
                cliche_count += 1;
                if cliche_count <= 3 {
                    issues.push(ReviewIssue {
                        dimension: "词汇".to_string(),
                        severity: "medium".to_string(),
                        description: format!("检测到 AI 高频 cliché: {}", cliche),
                        example: cliche.to_string(),
                        suggestion: "替换为更具体、更具画面感的描写".to_string(),
                    });
                }
            }
        }

        if cliche_count > 3 {
            issues.push(ReviewIssue {
                dimension: "词汇".to_string(),
                severity: "high".to_string(),
                description: format!("检测到 {} 处 AI 高频 cliché，词汇同质化严重", cliche_count),
                example: ai_cliches.join(", "),
                suggestion: "大量替换陈词滥调，使用角色视角的独特表达".to_string(),
            });
            suggestions.push("建立个人禁用词表，避免 AI 高频用语".to_string());
        }

        // 检查重复用词
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut word_freq: HashMap<String, usize> = HashMap::new();
        for word in &words {
            let w = word.trim_matches(|c: char| !c.is_alphanumeric());
            if w.len() > 1 {
                *word_freq.entry(w.to_lowercase()).or_insert(0) += 1;
            }
        }

        let mut repeated_words = Vec::new();
        for (word, count) in &word_freq {
            if *count > words.len() / 50 && word.len() >= 2 {
                repeated_words.push(word.clone());
            }
        }

        if repeated_words.len() >= 3 {
            issues.push(ReviewIssue {
                dimension: "词汇".to_string(),
                severity: "medium".to_string(),
                description: format!("高频重复用词: {}", repeated_words.join(", ")),
                example: repeated_words.first().cloned().unwrap_or_default(),
                suggestion: "使用同义词替换，增加词汇多样性".to_string(),
            });
        }

        let score = if cliche_count > 5 {
            0.3
        } else if cliche_count > 2 {
            0.5
        } else if cliche_count > 0 {
            0.7
        } else if !repeated_words.is_empty() {
            0.8
        } else {
            0.95
        };

        DimensionResult {
            score,
            issues,
            flagged,
            suggestions,
        }
    }

    // ==================== 语法维度 ====================

    fn check_syntax(&self, text: &str) -> DimensionResult {
        let mut issues = Vec::new();
        let flagged = Vec::new();
        let mut suggestions = Vec::new();

        // v0.65.0 校准（sepia 汇总 16 篇句法研究）：平均句长不是信号——同语料换
        // 计数单位（词/字）方向就翻转；短句/
        // 长句的绝对占比同样随语料与体裁矛盾。
        // **唯一跨语言、跨模型世代方向一致的是「句长离散度」**：人类段内句长
        // 方差更大。所以此处不再按 ≤10 字 / >50 字的占比打分，改为「连续近等长
        // 句」检查（与 human_voice::flat-rhythm 同一形态）。
        let report = crate::story_system::human_voice::analyze_human_voice(text);
        if report.metrics.flat_run_max >= 3 {
            issues.push(ReviewIssue {
                dimension: "语法".to_string(),
                severity: "medium".to_string(),
                description: format!(
                    "连续 {} 句长度趋同，句式节奏平板（人类段内句长方差显著大于机器）",
                    report.metrics.flat_run_max
                ),
                example: String::new(),
                suggestion: "拆一句、并两句或删一个子句来打破连串；只搬字，不加字".to_string(),
            });
        }

        // 被动句式偏多（低置信度提示，保留）
        let sentences: Vec<&str> = text
            .split(|c| c == '。' || c == '！' || c == '？')
            .filter(|s| !s.trim().is_empty())
            .collect();
        let passive_count = text.matches('被').count();
        let passive_ratio = passive_count as f64 / sentences.len().max(1) as f64;
        if passive_ratio > 0.15 {
            issues.push(ReviewIssue {
                dimension: "语法".to_string(),
                severity: "low".to_string(),
                description: "被动句式偏多，叙事缺乏主动感".to_string(),
                example: "他被一阵风吹得东倒西歪".to_string(),
                suggestion: "将被动句改为主动句，增强画面冲击力".to_string(),
            });
        }

        let score = if report.metrics.flat_run_max >= 3 {
            0.65
        } else if passive_ratio > 0.15 {
            0.8
        } else {
            0.9
        };

        DimensionResult {
            score,
            issues,
            flagged,
            suggestions,
        }
    }

    // ==================== 叙事维度 ====================

    fn check_narrative(&self, text: &str, _genre: Option<&str>) -> DimensionResult {
        let mut issues = Vec::new();
        let flagged = Vec::new();
        let mut suggestions = Vec::new();

        let paragraphs: Vec<&str> = text.split('\n').filter(|s| !s.trim().is_empty()).collect();

        // 段落长度过于均匀（sepia discourse-pass §3：同篇内长度一致是信号，
        // 段落数/平均段长本身不是——跨语料方向矛盾，故只看一致性）
        let mut lengths: Vec<usize> = paragraphs.iter().map(|p| p.chars().count()).collect();
        let uniform_ratio = if lengths.len() >= 3 {
            lengths.sort();
            let median = lengths[lengths.len() / 2];
            let uniform_count = lengths
                .iter()
                .filter(|l| {
                    let diff = if **l > median {
                        **l - median
                    } else {
                        median - **l
                    };
                    diff < median / 5
                })
                .count();
            uniform_count as f64 / lengths.len() as f64
        } else {
            0.0
        };

        if uniform_ratio > 0.7 {
            issues.push(ReviewIssue {
                dimension: "叙事".to_string(),
                severity: "medium".to_string(),
                description: "段落长度过于均匀，有流水账倾向".to_string(),
                example: paragraphs
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                suggestion: "打破均匀节奏，用长短段落制造呼吸感；允许一句话成段".to_string(),
            });
        }

        // 感官/动作密度：v0.65.0 降级为 advisory——sepia 实测机器感官密度反而
        // **高于**人类（3.93 对 3.66），低密度不是 AI
        // 指纹，只是画面感弱的提示。
        let sensory_words = vec!["看", "听", "闻", "摸", "感", "视", "见", "触", "嗅", "尝"];
        let action_words = vec!["走", "跑", "跳", "打", "抓", "挥", "冲", "退", "闪", "跃"];

        let sensory_count: usize = sensory_words.iter().map(|w| text.matches(w).count()).sum();
        let action_count: usize = action_words.iter().map(|w| text.matches(w).count()).sum();

        let text_len = text.chars().count().max(1);
        let sensory_density = sensory_count as f64 * 100.0 / text_len as f64;
        let action_density = action_count as f64 * 100.0 / text_len as f64;

        if sensory_density < 1.0 && action_density < 1.0 {
            issues.push(ReviewIssue {
                dimension: "叙事".to_string(),
                severity: "low".to_string(),
                description: "感官与动作密度偏低，画面感可能不足（仅作提示，非 AI 指纹）"
                    .to_string(),
                example: text.chars().take(50).collect(),
                suggestion: "补一处具体动作或实物细节即可，不必堆砌五感".to_string(),
            });
        }

        let score = if uniform_ratio > 0.7 {
            0.65
        } else if sensory_density < 1.0 && action_density < 1.0 {
            0.8
        } else {
            0.85
        };

        DimensionResult {
            score,
            issues,
            flagged,
            suggestions,
        }
    }

    // ==================== 情感维度 ====================

    /// v0.65.0 校准（sepia / StoryScope 实测）：
    ///
    /// 旧版把「情感标签化」（他很生气、她很高兴）当缺陷、要求一律改成动作与
    /// 神态——这与实测方向**相反**：人类 29% 的场景用显式标签命名情绪，机器只有
    /// 8%（模型几乎不写「她害怕」）；而机器 81% 的场景用身体感受承载情绪
    /// （心口一紧、背脊发凉），人类只有 38%。即「每段情绪都写成身体反应」才是
    /// 机器指纹，直说情绪是人类常态。
    ///
    /// 现在只报两类：①具身化独大而零平直命名（缺模式）；②廉价副词化强调
    /// （由衷地／情不自禁地／无比……），后者是真正的填充词。
    fn check_emotion(&self, text: &str) -> DimensionResult {
        let mut issues = Vec::new();
        let mut flagged = Vec::new();
        let mut suggestions = Vec::new();

        let report = crate::story_system::human_voice::analyze_human_voice(text);
        let embodied_count = report.metrics.embodied_count;
        let plain_count = report.metrics.plain_emotion_count;

        if embodied_count >= 3 && plain_count == 0 {
            issues.push(ReviewIssue {
                dimension: "情感".to_string(),
                severity: "medium".to_string(),
                description: format!(
                    "情绪全靠身体反应承载（{embodied_count} 处），无一处平直命名；机器 81% 的场景如此，人类只有 38%"
                ),
                example: "心口一紧 / 背脊发凉 / 喉咙发紧".to_string(),
                suggestion:
                    "四模式混用：以行为为主，平直命名次之（「她害怕」是正常的人类句式），具身化只留峰值"
                        .to_string(),
            });
            suggestions.push("把多数身体反应换成她做了什么，或直接说出情绪".to_string());
        }

        // 廉价副词化强调（真正的填充词，非「直说情绪」）
        let padding = vec![
            "由衷地",
            "发自内心地",
            "情不自禁地",
            "无比激动",
            "十分恐惧",
            "极其开心",
            "深感欣慰",
            "不禁感到",
            "忍不住感到",
        ];
        let mut padding_count = 0;
        for phrase in &padding {
            if let Some(pos) = text.find(phrase) {
                padding_count += 1;
                if padding_count <= 2 {
                    flagged.push(FlaggedPassage {
                        text: phrase.to_string(),
                        dimension: "情感".to_string(),
                        reason: "副词化情绪填充，删掉后句子更干净".to_string(),
                        position: pos,
                    });
                }
            }
        }
        if padding_count > 0 {
            issues.push(ReviewIssue {
                dimension: "情感".to_string(),
                severity: "low".to_string(),
                description: format!("{padding_count} 处副词化情绪填充（由衷地/情不自禁地…）"),
                example: padding.first().map(|s| s.to_string()).unwrap_or_default(),
                suggestion: "删掉副词，保留情绪本身".to_string(),
            });
        }

        // 内心独白占比（低置信度提示，保留）
        let inner_monologue_markers = vec!["想", "觉得", "认为", "感到", "感觉"];
        let inner_count: usize = inner_monologue_markers
            .iter()
            .map(|w| text.matches(w).count())
            .sum();

        let text_len = text.chars().count().max(1);
        let inner_ratio = inner_count as f64 * 100.0 / text_len as f64;

        if inner_ratio > 3.0 {
            issues.push(ReviewIssue {
                dimension: "情感".to_string(),
                severity: "low".to_string(),
                description: "内心独白占比偏高，可能削弱画面感".to_string(),
                example: "他想，这样做是对的".to_string(),
                suggestion: "将部分内心活动转化为动作或对话".to_string(),
            });
        }

        let score = if embodied_count >= 3 && plain_count == 0 {
            0.6
        } else if padding_count > 0 || inner_ratio > 3.0 {
            0.8
        } else {
            0.9
        };

        DimensionResult {
            score,
            issues,
            flagged,
            suggestions,
        }
    }

    // ==================== 对话维度 ====================

    fn check_dialogue(&self, text: &str) -> DimensionResult {
        let mut issues = Vec::new();
        let flagged = Vec::new();
        let suggestions = Vec::new();

        // 提取对话内容。v0.65.0：原先只认弯引号（“ ” ‘ ’），中文直角引号
        // （「」『』）里的对话整块漏检，导致本维度在「」体例的正文上直接失效；
        // 现按深度配对两种引号（『』可嵌套在「」内）。
        let mut dialogues = Vec::new();
        let mut depth = 0usize;
        let mut current_quote = String::new();

        for c in text.chars() {
            match c {
                '“' | '「' | '『' => {
                    depth += 1;
                    if depth == 1 {
                        current_quote.clear();
                    } else {
                        current_quote.push(c);
                    }
                }
                '”' | '」' | '』' => {
                    if depth > 0 {
                        depth -= 1;
                        if depth == 0 {
                            let trimmed = current_quote.trim().to_string();
                            if !trimmed.is_empty() {
                                dialogues.push(trimmed);
                            }
                            current_quote.clear();
                        } else {
                            current_quote.push(c);
                        }
                    }
                }
                _ => {
                    if depth > 0 {
                        current_quote.push(c);
                    }
                }
            }
        }

        if dialogues.is_empty() {
            // 无对话，不评分
            return DimensionResult {
                score: 1.0,
                issues: Vec::new(),
                flagged: Vec::new(),
                suggestions: Vec::new(),
            };
        }

        // 检查说明性对话
        let exposition_markers = vec!["你知道吗", "其实", "简单来说", "换句话说", "所谓"];
        let mut exposition_count = 0;

        for dialogue in &dialogues {
            for marker in &exposition_markers {
                if dialogue.contains(marker) {
                    exposition_count += 1;
                    break;
                }
            }
        }

        let exposition_ratio = exposition_count as f64 / dialogues.len() as f64;
        if exposition_ratio > 0.3 {
            issues.push(ReviewIssue {
                dimension: "对话".to_string(),
                severity: "medium".to_string(),
                description: "说明性对话过多，角色像解说员".to_string(),
                example: dialogues.first().cloned().unwrap_or_default(),
                suggestion: "将背景信息拆散到动作和场景中，而非借角色之口说明".to_string(),
            });
        }

        // 对话标签：v0.65.0 校准——旧版把「标签单调（几乎全是说/道）」当缺陷并
        // 建议换成动作标签，方向相反：重复「说」是人类常态，**轮换低语/咕哝/
        // 嗤笑 才是机器优雅**（sepia style-pass
        // §4）。现在只在「花式标签轮换且几乎不用 说/道」时提示。
        let plain_tags = vec!["说道", "问道", "回答", "说", "道"];
        let plain_tag_count: usize = plain_tags.iter().map(|t| text.matches(t).count()).sum();
        let report_tags = crate::story_system::human_voice::analyze_human_voice(text);
        let fancy_tag_count = report_tags.metrics.fancy_tag_distinct;

        if fancy_tag_count >= 3 && plain_tag_count == 0 {
            issues.push(ReviewIssue {
                dimension: "对话".to_string(),
                severity: "low".to_string(),
                description: format!(
                    "轮换使用 {fancy_tag_count} 种花式对话标签（低语/咕哝/嗤笑…）且无一处「说/道」"
                ),
                example: "他低语／她嗤笑／他咕哝".to_string(),
                suggestion: "对话标签重复「说」即可，不必轮换；花式标签留给关键处".to_string(),
            });
        }

        // 语气词缺失：中文人类语料的语气词密度是机器的 5 倍（sepia zh.md §1），
        // 对话里一个「啊/吧/呢/嘛」都没有是中文最强的机器指纹之一。仅作提示——
        // 冷硬角色或正式场合确实可以不用，由编辑器裁决。
        let dialogue_chars: usize = dialogues.iter().map(|d| d.chars().count()).sum();
        if dialogue_chars >= 60 && report_tags.metrics.mood_particle_count == 0 {
            issues.push(ReviewIssue {
                dimension: "对话".to_string(),
                severity: "low".to_string(),
                description: "对话无一处语气词（啊/吧/呢/嘛/啦），书面腔偏重".to_string(),
                example: dialogues.first().cloned().unwrap_or_default(),
                suggestion: "在符合角色口吻的对话里放一两个语气词，中文口语里这是常态".to_string(),
            });
        }

        let score = if exposition_ratio > 0.5 {
            0.4
        } else if exposition_ratio > 0.3 {
            0.6
        } else if (fancy_tag_count >= 3 && plain_tag_count == 0)
            || (dialogue_chars >= 60 && report_tags.metrics.mood_particle_count == 0)
        {
            0.8
        } else {
            0.9
        };

        DimensionResult {
            score,
            issues,
            flagged,
            suggestions,
        }
    }
}

/// 单维度检查结果
struct DimensionResult {
    score: f64,
    issues: Vec<ReviewIssue>,
    flagged: Vec<FlaggedPassage>,
    suggestions: Vec<String>,
}

impl Default for AntiAiReviewer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v0.65.0 校准契约：人类更常**平直命名**情绪（29% vs 8%），「她害怕」不是
    /// 缺陷；缺陷是「每个情绪都写成身体反应、零平直命名」。
    #[test]
    fn plain_emotion_naming_is_not_flagged_but_embodied_only_is() {
        let reviewer = AntiAiReviewer::new();

        let plain = "她害怕。他恨她。她坐在门槛上，没有动。";
        let plain_issues = reviewer.check_emotion(plain).issues;
        assert!(
            !plain_issues
                .iter()
                .any(|i| i.description.contains("身体反应")),
            "平直命名不应被判缺陷: {plain_issues:?}"
        );

        let embodied = "她心里一紧。他背脊发凉。她喉咙发紧。";
        let embodied_issues = reviewer.check_emotion(embodied).issues;
        assert!(
            embodied_issues
                .iter()
                .any(|i| i.description.contains("身体反应")),
            "{embodied_issues:?}"
        );
    }

    /// v0.65.0 校准契约：重复「说/道」是人类常态，轮换花式标签才是机器优雅。
    #[test]
    fn repeated_plain_speech_tag_is_not_flagged_but_fancy_rotation_is() {
        let reviewer = AntiAiReviewer::new();

        let repeated = "「你来了。」他说。「我知道了。」他说。「坐。」他说。\
「这茶是今早新沏的，你尝尝温度。」他说。「路上还顺利。」他说。「还行，只是雨大。」他说。";
        let repeated_issues = reviewer.check_dialogue(repeated).issues;
        assert!(
            !repeated_issues
                .iter()
                .any(|i| i.description.contains("花式对话标签")),
            "重复「说」不应被判缺陷: {repeated_issues:?}"
        );

        let fancy = "「你来了。」他低语。「我明白。」她嗤笑。「坐。」他咕哝。\
「这茶是今早新沏的，你尝尝温度。」她嘟囔。「路上还顺利。」他呢喃。「还行，只是雨大。」她嘶吼。";
        let fancy_issues = reviewer.check_dialogue(fancy).issues;
        assert!(
            fancy_issues
                .iter()
                .any(|i| i.description.contains("花式对话标签")),
            "{fancy_issues:?}"
        );
    }

    /// 语法维度按「句长离散度」而非平均句长判定（sepia 汇总 16 篇研究的唯一
    /// 方向一致指标）：连续近等长句要报，长短参差不得报。
    #[test]
    fn sentence_rhythm_uses_dispersion_not_mean_length() {
        let reviewer = AntiAiReviewer::new();

        let flat =
            "他推开门走了进去，屋里没人。她抬头看了他一眼，没有作声。桌上摆着两杯还温着的茶。";
        assert!(
            reviewer
                .check_syntax(flat)
                .issues
                .iter()
                .any(|i| i.description.contains("长度趋同")),
            "连续近等长句应报"
        );

        let varied =
            "他推开门。里面很暗，只有窗台上一盏灯亮着，灯芯烧得歪歪扭扭，像随时要灭。她抬头。";
        assert!(
            !reviewer
                .check_syntax(varied)
                .issues
                .iter()
                .any(|i| i.description.contains("长度趋同")),
            "长短参差不应报"
        );
    }
}
