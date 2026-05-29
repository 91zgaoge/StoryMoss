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
        let suggestions = Vec::new();

        let sentences: Vec<&str> = text
            .split(|c| c == '。' || c == '！' || c == '？')
            .collect();

        // 检查句式多样性
        let mut short_sentences = 0;
        let mut long_sentences = 0;
        for s in &sentences {
            let len = s.chars().count();
            if len > 0 && len < 10 {
                short_sentences += 1;
            }
            if len > 50 {
                long_sentences += 1;
            }
        }

        let short_ratio = short_sentences as f64 / sentences.len().max(1) as f64;
        let long_ratio = long_sentences as f64 / sentences.len().max(1) as f64;

        if short_ratio > 0.5 {
            issues.push(ReviewIssue {
                dimension: "语法".to_string(),
                severity: "medium".to_string(),
                description: "短句过多，节奏过于碎片化".to_string(),
                example: sentences
                    .iter()
                    .find(|s| s.chars().count() < 10)
                    .unwrap_or(&"")
                    .to_string(),
                suggestion: "适当使用复合句，增强句子间的逻辑关联".to_string(),
            });
        }

        if long_ratio > 0.3 {
            issues.push(ReviewIssue {
                dimension: "语法".to_string(),
                severity: "medium".to_string(),
                description: "长句过多，阅读负担重".to_string(),
                example: sentences
                    .iter()
                    .find(|s| s.chars().count() > 50)
                    .unwrap_or(&"")
                    .to_string(),
                suggestion: "拆分超长句，用节奏变化调节阅读体验".to_string(),
            });
        }

        // 检查被动语态倾向（中文中的"被"字句）
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

        let score = if short_ratio > 0.5 || long_ratio > 0.3 {
            0.6
        } else if passive_ratio > 0.15 {
            0.75
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

        // 检查流水账倾向（段落长度过于均匀）
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
                suggestion: "打破均匀节奏，用长短段落制造呼吸感".to_string(),
            });
        }

        // 检查叙事密度（每百字的动作/描写词比例）
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
                severity: "high".to_string(),
                description: "叙事密度过低，缺乏感官描写和动作细节".to_string(),
                example: text.chars().take(50).collect(),
                suggestion: "增加五感描写和具体动作，让读者身临其境".to_string(),
            });
            suggestions.push("每段至少包含一个感官细节或具体动作".to_string());
        }

        let score = if sensory_density < 1.0 && action_density < 1.0 {
            0.4
        } else if uniform_ratio > 0.7 {
            0.6
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

    fn check_emotion(&self, text: &str) -> DimensionResult {
        let mut issues = Vec::new();
        let mut flagged = Vec::new();
        let mut suggestions = Vec::new();

        // 情感标签词检测
        let emotion_labels = vec![
            "他很生气",
            "她很高兴",
            "非常愤怒",
            "极其开心",
            "感到悲伤",
            "十分恐惧",
            "无比激动",
            "深感欣慰",
            "由衷地",
            "发自内心地",
            "情不自禁地",
        ];

        let mut label_count = 0;
        for label in &emotion_labels {
            if text.contains(label) {
                label_count += 1;
                if label_count <= 2 {
                    flagged.push(FlaggedPassage {
                        text: label.to_string(),
                        dimension: "情感".to_string(),
                        reason: "情感标签化，直接告诉读者情绪而非展示".to_string(),
                        position: text.find(label).unwrap_or(0),
                    });
                }
            }
        }

        if label_count > 0 {
            issues.push(ReviewIssue {
                dimension: "情感".to_string(),
                severity: if label_count > 3 { "high" } else { "medium" }.to_string(),
                description: format!("检测到 {} 处情感标签化表达", label_count),
                example: emotion_labels
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                suggestion: "用动作、神态、环境反应来暗示情绪，而非直接标签".to_string(),
            });
            suggestions.push("遵循'展示而非告知'原则描写情感".to_string());
        }

        // 检查情感细腻度（内心独白 vs 外部描写）
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

        let score = if label_count > 3 {
            0.3
        } else if label_count > 0 {
            0.6
        } else if inner_ratio > 3.0 {
            0.75
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

        // 提取对话内容（简化处理：引号内的内容）
        let mut dialogues = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut in_quote = false;
        let mut current_quote = String::new();

        for c in &chars {
            if *c == '\u{201C}' || *c == '\u{201D}' || *c == '\u{2018}' || *c == '\u{2019}' {
                if in_quote {
                    dialogues.push(current_quote.clone());
                    current_quote.clear();
                }
                in_quote = !in_quote;
            } else if in_quote {
                current_quote.push(*c);
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

        // 检查对话标签单调
        let dialogue_tags = vec!["他说", "她说", "说道", "回答", "问道"];
        let mut tag_count = 0;
        for tag in &dialogue_tags {
            tag_count += text.matches(tag).count();
        }

        let tag_ratio = tag_count as f64 / dialogues.len().max(1) as f64;
        if tag_ratio > 0.8 {
            issues.push(ReviewIssue {
                dimension: "对话".to_string(),
                severity: "low".to_string(),
                description: "对话标签单调，缺乏变化".to_string(),
                example: "他说".to_string(),
                suggestion: "用动作标签替代部分'说'，如'他揉了揉眉心''她放下茶杯'".to_string(),
            });
        }

        let score = if exposition_ratio > 0.5 {
            0.4
        } else if exposition_ratio > 0.3 {
            0.6
        } else if tag_ratio > 0.8 {
            0.75
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
