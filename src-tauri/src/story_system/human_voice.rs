#![allow(dead_code)]
//! 人类文笔基线（v0.65.0，方法来源：sepia / StoryScope）。
//!
//! 与 `prose_lint` 的分工：`prose_lint` 抓「绝不能出现」的硬伤（注入术语泄漏、
//! 章尾预告腔）；本模块抓「机器写多了、人写少了」的**分布偏移**——它不判对错，
//! 只把文笔与人类基线的距离量化成两条清单，交给编辑器裁决。
//!
//! ## 为什么只放这几条规则
//!
//! sepia 汇总了 16 篇句法量测研究，结论是**只有「句长离散度」跨语言、跨模型
//! 世代方向一致**（人类段内句长方差更大）；平均句长、标点密度、段落长度、
//! 段落数在不同语料上方向互相矛盾，因此一律不进规则。中文侧的校准来自
//! HC3-Chinese 语料（朱君輝 CCL 2023）的实测差：
//!
//! | 特征 | 人类 | ChatGPT | 本模块 |
//! |---|---|---|---|
//! | 句长标准差（字） | 15.15 | 12.84 | `flat_rhythm` 连续近等长句 |
//! | 語氣詞密度 | 0.016 | 0.003 | `mood_particle_absent`（对话里一个没有） |
//! | 連詞密度 | 0.013 | 0.036 | `connective_stack`（一句堆三个以上） |
//! | 单音节词占比 | 0.483 | 0.379 | `disyllabic_padding`（进行/加以+双音节动词） |
//!
//! 情绪模式来自 StoryScope：机器 81% 的场景以**身体反应**承载情绪（心口一紧、
//! 背脊发凉），人类只有 38%，人类更常用「行为」与「平直命名」（「她害怕」是
//! 人类句式，模型几乎不写）。所以本模块把「一具身化而无平直命名」记为**缺失**，
//! 而不是把平直命名记为缺陷——这与旧 `anti_ai` 的「情感标签化」判定方向相反，
//! 旧判定已按本模块校准。
//!
//! ## 过度纠正也是指纹
//!
//! sepia 的校准原则：**瞄准人类分布带，而不是走到 AI 的反面**。均匀短句是
//! 与均匀长句同构的缺陷；把一个「不是…而是」当机器腔去改，反而制造新指纹
//! （中文人类语料 30% 的文章含有该形态，只有聚集才是信号）。本模块全部规则
//! 都是 advisory（供裁决），且带白名单：单项形态不报。

use serde::{Deserialize, Serialize};

/// 段落内「连续近等长句」的判定宽度：相邻句长比落在 [1/1.33, 1.33] 视为同长。
const FLAT_RATIO: f64 = 1.33;
/// 参与节奏判定的最短句（字）；过短句属对话/动作节拍，不参与。
const FLAT_MIN_SENTENCE_CHARS: usize = 10;
/// 连续几句近等长即报。
const FLAT_RUN_LEN: usize = 3;
/// 段落长度变异系数低于此值、且段落数够多 → 段落过于均匀。
const PARAGRAPH_CV_FLOOR: f64 = 0.30;
const PARAGRAPH_MIN_COUNT: usize = 6;
/// 短段（≤此字数）视为「一句话成段」的人类参差。
const SHORT_PARAGRAPH_CHARS: usize = 40;

/// 花式对话标签（轮换它们是「机器优雅」；重复「说/道」才是人类常态）。
const FANCY_SPEECH_TAGS: &[&str] = &[
    "低语",
    "咕哝",
    "嗤笑",
    "嘟囔",
    "呢喃",
    "咆哮",
    "嘶吼",
    "怒吼",
    "冷笑",
    "沉声",
    "喃喃",
    "嘀咕",
    "呵斥",
    "讥讽",
    "揶揄",
    "哂笑",
    "轻哼",
    "轻声道",
    "冷冷道",
    "缓缓道",
];
/// 平直对话标签。
const PLAIN_SPEECH_TAGS: &[&str] = &["说道", "问道", "答道", "回道", "回答"];

/// 统计平直对话标签。
///
/// 不能直接数「说」「道」二字：中文里「知道／难道／味道／街道／说明」都含这些
/// 字，会把非对话文本算成标签，进而让「花式标签轮换而几乎不用说」这条判反
/// （真机探针实测：AI 样本里一句「她知道」把 plain_tag_count 抬到 1，规则整条
/// 失效）。这里只认确定是标签的形态：
/// - 双字形态（说道/问道/答道/回道/回答）；
/// - 「说」紧跟句读或引号（他说。／他说：／他说「）；
/// - 「道」紧跟冒号或引号（他道：／他道「）——不认「知道。」这类。
fn count_plain_speech_tags(text: &str) -> usize {
    let mut count = PLAIN_SPEECH_TAGS
        .iter()
        .map(|t| text.matches(t).count())
        .sum::<usize>();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let Some(next) = chars.get(i + 1) else {
            continue;
        };
        match c {
            '说' if matches!(next, '。' | '，' | '：' | '！' | '？' | '」' | '”') => {
                count += 1
            }
            '道' if matches!(next, '：' | '「' | '”') => count += 1,
            _ => {}
        }
    }
    count
}

/// 连词（跨句/跨子句堆叠会显著抬高 AI 度）。
const CONNECTIVES: &[&str] = &[
    "以及", "并且", "同时", "此外", "因此", "然而", "而且", "进而", "从而", "加上", "况且", "何况",
    "并且",
];
/// 双音节填充（单音节动词即可）。左词 + 右动词形态。
const DISYLLABIC_VERB_STEMS: &[&str] = &["进行", "加以", "予以", "作出", "做出"];
const DISYLLABIC_VERBS: &[&str] = &[
    "讨论", "思考", "分析", "观察", "了解", "说明", "处理", "决定", "选择", "判断", "评估", "调整",
    "改变", "改进", "安排", "准备", "解释", "描述", "确认", "检查", "尝试", "努力", "回应", "反应",
    "回答", "接受", "拒绝", "保证", "承诺", "表达",
];

/// 具身化情绪（AI 主导模式：心口一紧、背脊发凉）。
const EMBODIED_EMOTION: &[&str] = &[
    "心里一紧",
    "心头一紧",
    "心口一紧",
    "心里一沉",
    "心头一沉",
    "心里咯噔",
    "背脊一凉",
    "背脊发凉",
    "脊背发凉",
    "后背发凉",
    "背后发凉",
    "后背一僵",
    "喉咙发紧",
    "喉头发紧",
    "嗓子发紧",
    "眼眶一热",
    "眼眶发热",
    "眼眶发酸",
    "胸口一闷",
    "胸中一闷",
    "胸口发紧",
    "指尖发凉",
    "手脚冰凉",
    "指尖一颤",
    "呼吸一滞",
    "呼吸滞",
    "血液凝固",
    "心跳漏",
    "胃里一阵",
    "胃里翻涌",
    "头皮发麻",
    "汗毛竖起",
    "身体一僵",
    "浑身一震",
    "浑身一僵",
    "手一抖",
    "瞳孔一缩",
    "心头一震",
    "心中一震",
    "脑子嗡",
];
/// 平直命名情绪（人类句式：「她害怕」）。前缀必须是人称/姓名的近距离搭配。
const EMOTION_WORDS: &[&str] = &[
    "害怕", "恐惧", "愤怒", "生气", "悲伤", "难过", "高兴", "开心", "痛苦", "焦虑", "紧张", "羞耻",
    "愧疚", "绝望", "失望", "委屈", "嫉妒", "厌烦", "不安", "惭愧", "心慌", "心疼", "心酸", "庆幸",
    "恨", "爱",
];
/// 语气词（人类对话里 5 倍于机器）。
const MOOD_PARTICLES: &[char] = &[
    '啊', '吧', '呢', '嘛', '啦', '呀', '哦', '唉', '咦', '嗯', '哈', '嘿', '喔', '咯', '诶', '嘞',
];
/// 抽象包装（"a [abstract noun] of [noun]" 的中文形态）。
const ABSTRACT_WRAPPERS: &[&str] = &[
    "的化身",
    "的象征",
    "的缩影",
    "的写照",
    "的注脚",
    "的结晶",
    "明证",
    "无声的",
    "无形的",
    "莫名的",
    "难以言喻",
    "不可名状",
    "说不清道不明",
];
const ABSTRACT_QUANTIFIERS: &[&str] = &["一股", "一种", "一丝", "一抹", "一缕", "一份", "一阵"];
/// 抽象名词（跟在量词后即为包装句）。
const ABSTRACT_NOUNS: &[&str] = &[
    "暖意", "寒意", "感觉", "情绪", "氛围", "气韵", "光芒", "力量", "情绪", "感动", "失落", "孤独",
    "温柔", "柔软", "坚定", "决绝", "悲恸", "喜悦",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceFinding {
    /// 稳定规则 id
    pub rule: String,
    /// excess（机器过量）| deficit（人类标志缺失）
    pub kind: String,
    /// 人类可读说明（含实测依据方向）
    pub detail: String,
    /// 命中片段
    pub excerpt: String,
}

impl VoiceFinding {
    fn new(rule: &str, kind: &str, detail: &str, excerpt: &str) -> Self {
        Self {
            rule: rule.to_string(),
            kind: kind.to_string(),
            detail: detail.to_string(),
            excerpt: excerpt.chars().take(40).collect(),
        }
    }
}

/// 量化指标：落日志与审计，不做通过/不通过判定。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VoiceMetrics {
    pub sentence_count: usize,
    /// 段内句长标准差（字），加权前的最长段
    pub sentence_len_sd: f64,
    /// 最长「近等长句」连串长度
    pub flat_run_max: usize,
    pub paragraph_count: usize,
    /// 段落长度变异系数
    pub paragraph_cv: f64,
    pub embodied_count: usize,
    pub plain_emotion_count: usize,
    pub mood_particle_count: usize,
    pub fancy_tag_distinct: usize,
    pub plain_tag_count: usize,
    pub connective_stack_count: usize,
    pub disyllabic_padding_count: usize,
    pub abstract_wrapper_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HumanVoiceReport {
    pub findings: Vec<VoiceFinding>,
    pub metrics: VoiceMetrics,
}

impl HumanVoiceReport {
    pub fn excess(&self) -> Vec<&VoiceFinding> {
        self.findings
            .iter()
            .filter(|f| f.kind == "excess")
            .collect()
    }

    pub fn deficit(&self) -> Vec<&VoiceFinding> {
        self.findings
            .iter()
            .filter(|f| f.kind == "deficit")
            .collect()
    }
}

/// 去标点后的句长（字）。
fn sentence_len(sentence: &str) -> usize {
    sentence
        .chars()
        .filter(|c| !"。！？…；：、，,.;:!?\"'“”‘’「」『』（）()—— \t".contains(*c))
        .count()
}

/// 句末切分（含引号收尾：…。」作为一句的结尾）。
fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut pending_close = false;
    for ch in text.chars() {
        if ch == '\n' {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
                current.clear();
            }
            pending_close = false;
            continue;
        }
        if matches!(ch, '。' | '！' | '？' | '…' | '；') {
            current.push(ch);
            pending_close = true;
            continue;
        }
        if pending_close && matches!(ch, '」' | '』' | '"' | '”' | '’') {
            current.push(ch);
            continue;
        }
        if pending_close {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
            pending_close = false;
        }
        current.push(ch);
    }
    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn paragraphs(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// 提取引号内的对话文本（中文引号与直角引号）。
fn quoted_segments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '“' | '「' | '『' => {
                depth += 1;
                if depth == 1 {
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            '”' | '」' | '』' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if !current.trim().is_empty() {
                            out.push(current.trim().to_string());
                        }
                        current.clear();
                    } else {
                        current.push(ch);
                    }
                }
            }
            _ => {
                if depth > 0 {
                    current.push(ch);
                }
            }
        }
    }
    out
}

/// 窗口内是否有人称代词/停顿（用于判定「平直命名」而非转述）。
///
/// `at` 是 `find` 给出的**字节**偏移，取它之前 8 个字符做窗口。
fn has_pronoun_prefix(text: &str, at: usize) -> bool {
    let window: String = text[..at].chars().rev().take(8).collect();
    ["他", "她", "我", "你", "它", "们", "人"]
        .iter()
        .any(|p| window.contains(p))
}

/// 统计「段内连续近等长句」的最长连串。
fn longest_flat_run(sentences: &[String]) -> usize {
    let lens: Vec<usize> = sentences.iter().map(|s| sentence_len(s)).collect();
    let mut best = 0usize;
    let mut run = 1usize;
    for i in 1..lens.len() {
        let (a, b) = (lens[i - 1], lens[i]);
        let eligible = a >= FLAT_MIN_SENTENCE_CHARS && b >= FLAT_MIN_SENTENCE_CHARS;
        let close = if a == 0 || b == 0 {
            false
        } else {
            let ratio = a as f64 / b as f64;
            ratio <= FLAT_RATIO && ratio >= 1.0 / FLAT_RATIO
        };
        if eligible && close {
            run += 1;
            best = best.max(run);
        } else {
            run = 1;
        }
    }
    best
}

/// 入口：产出人类文笔基线报告（纯函数，0 LLM）。
pub fn analyze_human_voice(text: &str) -> HumanVoiceReport {
    let mut findings: Vec<VoiceFinding> = Vec::new();
    let mut metrics = VoiceMetrics::default();
    if text.trim().is_empty() {
        return HumanVoiceReport { findings, metrics };
    }

    let paras = paragraphs(text);
    let sentences = split_sentences(text);
    metrics.sentence_count = sentences.len();
    metrics.paragraph_count = paras.len();

    // ── 节奏：段内连续近等长句（唯一跨语言方向一致的句法信号）──
    let mut flat_worst: (usize, String) = (0, String::new());
    let mut sd_max = 0.0f64;
    for para in &paras {
        let sents = split_sentences(para);
        if sents.len() < FLAT_RUN_LEN {
            continue;
        }
        let lens: Vec<f64> = sents.iter().map(|s| sentence_len(s) as f64).collect();
        let mean = lens.iter().sum::<f64>() / lens.len() as f64;
        if mean > 0.0 {
            let var = lens.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lens.len() as f64;
            sd_max = sd_max.max(var.sqrt());
        }
        let run = longest_flat_run(&sents);
        if run > flat_worst.0 {
            flat_worst = (run, sents.first().cloned().unwrap_or_default());
        }
    }
    metrics.sentence_len_sd = (sd_max * 100.0).round() / 100.0;
    metrics.flat_run_max = flat_worst.0;
    if flat_worst.0 >= FLAT_RUN_LEN {
        findings.push(VoiceFinding::new(
            "flat-rhythm",
            "excess",
            &format!(
                "连续 {} 句长度趋同（人类段内句长方差显著大于机器；拆一句或并两句，只搬字不加字）",
                flat_worst.0
            ),
            &flat_worst.1,
        ));
    }

    // ── 段落参差：均匀段落（人类包含一句话成段）──
    if paras.len() >= PARAGRAPH_MIN_COUNT {
        let lens: Vec<f64> = paras.iter().map(|p| p.chars().count() as f64).collect();
        let mean = lens.iter().sum::<f64>() / lens.len() as f64;
        if mean > 0.0 {
            let var = lens.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lens.len() as f64;
            let cv = var.sqrt() / mean;
            metrics.paragraph_cv = (cv * 100.0).round() / 100.0;
            let has_short = paras
                .iter()
                .any(|p| p.chars().count() <= SHORT_PARAGRAPH_CHARS);
            if cv < PARAGRAPH_CV_FLOOR && !has_short {
                findings.push(VoiceFinding::new(
                    "uniform-paragraphs",
                    "excess",
                    &format!(
                        "{} 个段落长度过于均匀且无短段（变异系数 {:.2}）；人类常有一句话成段",
                        paras.len(),
                        cv
                    ),
                    "",
                ));
            }
        }
    }

    // ── 情绪模式：具身化独大而零平直命名 ──
    let mut embodied_count = 0usize;
    for marker in EMBODIED_EMOTION {
        embodied_count += text.matches(marker).count();
    }
    let mut plain_emotion_count = 0usize;
    for word in EMOTION_WORDS {
        let mut from = 0usize;
        while let Some(rel) = text[from..].find(word) {
            let at = from + rel;
            if has_pronoun_prefix(text, at) {
                plain_emotion_count += 1;
            }
            from = at + word.len();
            if from >= text.len() {
                break;
            }
        }
    }
    metrics.embodied_count = embodied_count;
    metrics.plain_emotion_count = plain_emotion_count;
    if embodied_count >= 3 && plain_emotion_count == 0 {
        findings.push(VoiceFinding::new(
            "embodied-only",
            "deficit",
            &format!(
                "{} 处身体反应式情绪、0 处平直命名；人类情感四模式混用，行为优先、平直命名次之，具身化只留峰值",
                embodied_count
            ),
            "",
        ));
    }

    // ── 语气词：对话里一个都没有 ──
    let dialogues = quoted_segments(text);
    let mood_particle_count: usize = text.chars().filter(|c| MOOD_PARTICLES.contains(c)).count();
    metrics.mood_particle_count = mood_particle_count;
    let dialogue_chars: usize = dialogues.iter().map(|d| d.chars().count()).sum();
    if dialogue_chars >= 60 && mood_particle_count == 0 {
        findings.push(VoiceFinding::new(
            "mood-particle-absent",
            "deficit",
            "全章对话无一处语气词（啊/吧/呢/嘛/啦…）；中文人类语料语气词密度是机器的 5 倍",
            dialogues.first().map(String::as_str).unwrap_or_default(),
        ));
    }

    // ── 对话标签：花式轮换而几乎不重复「说/道」──
    let mut fancy: Vec<&str> = Vec::new();
    for tag in FANCY_SPEECH_TAGS {
        if text.contains(tag) {
            fancy.push(tag);
        }
    }
    let plain_tag_count: usize = count_plain_speech_tags(text);
    metrics.fancy_tag_distinct = fancy.len();
    metrics.plain_tag_count = plain_tag_count;
    if fancy.len() >= 3 && plain_tag_count == 0 {
        findings.push(VoiceFinding::new(
            "fancy-speech-tags",
            "excess",
            &format!(
                "轮换使用 {} 种花式对话标签且无一处「说/道」；重复「说」是人类常态，轮换标签是机器优雅",
                fancy.len()
            ),
            &fancy.join("、"),
        ));
    }

    // ── 连词堆叠：一句里三个以上 ──
    let mut stack_count = 0usize;
    let mut stack_excerpt = String::new();
    for sentence in &sentences {
        let mut hits = CONNECTIVES
            .iter()
            .filter(|c| sentence.contains(**c))
            .count();
        // 裸「和」需 4 次以上才算堆叠（避免人名/词语误伤）
        if sentence.matches('和').count() >= 4 {
            hits += 1;
        }
        if hits >= 3 {
            stack_count += 1;
            if stack_excerpt.is_empty() {
                stack_excerpt = sentence.clone();
            }
        }
    }
    metrics.connective_stack_count = stack_count;
    if stack_count > 0 {
        findings.push(VoiceFinding::new(
            "connective-stack",
            "excess",
            &format!("{stack_count} 句堆叠三个以上连词；中文以意合（并置）为常，机器连词密度是人类 2.8 倍"),
            &stack_excerpt,
        ));
    }

    // ── 双音节填充 ──
    let mut padding = 0usize;
    let mut padding_excerpt = String::new();
    // 「进行了思考／进行一番讨论／进行一下评估」都是同一形态，中间可插入状语。
    const PADDING_INFIXES: &[&str] = &["", "了", "一次", "一下", "一番"];
    for stem in DISYLLABIC_VERB_STEMS {
        for verb in DISYLLABIC_VERBS {
            for infix in PADDING_INFIXES {
                let pattern = format!("{stem}{infix}{verb}");
                if let Some(pos) = text.find(&pattern) {
                    padding += 1;
                    if padding_excerpt.is_empty() {
                        padding_excerpt = text[pos..].chars().take(12).collect();
                    }
                }
            }
        }
    }
    metrics.disyllabic_padding_count = padding;
    if padding > 0 {
        findings.push(VoiceFinding::new(
            "disyllabic-padding",
            "excess",
            &format!(
                "{padding} 处「进行/加以/予以+双音节动词」；中文单音节动词占比人类 0.483 对机器 0.379，用单动词即可"
            ),
            &padding_excerpt,
        ));
    }

    // ── 抽象包装 ──
    let mut wrapper = 0usize;
    let mut wrapper_excerpt = String::new();
    for phrase in ABSTRACT_WRAPPERS {
        if let Some(pos) = text.find(phrase) {
            wrapper += 1;
            if wrapper_excerpt.is_empty() {
                wrapper_excerpt = text[pos.saturating_sub(6)..].chars().take(14).collect();
            }
        }
    }
    for quantifier in ABSTRACT_QUANTIFIERS {
        for noun in ABSTRACT_NOUNS {
            let pattern = format!("{quantifier}{noun}");
            if text.contains(&pattern) {
                wrapper += 1;
                if wrapper_excerpt.is_empty() {
                    wrapper_excerpt = pattern;
                }
            }
        }
    }
    metrics.abstract_wrapper_count = wrapper;
    if wrapper >= 3 {
        findings.push(VoiceFinding::new(
            "abstract-wrapper",
            "excess",
            &format!(
                "{wrapper} 处「一股…之感 / 无声的 / 的化身」类抽象包装；写得出名字的实物与动作替代它"
            ),
            &wrapper_excerpt,
        ));
    }

    HumanVoiceReport { findings, metrics }
}

/// 固定的写作准则（每次注入；全部是可逐条核查的动作，不是形容词）。
pub fn render_guidance_rules() -> String {
    [
        "【人类文笔基线（逐条落实；与节拍任务冲突时以节拍任务为先）】",
        "1. 情绪四模式混用：以「行为」为主（她做了什么），其次平直命名（「她害怕」「他恨她」——这是人类句式，不羞于直说），具身化（心口一紧、背脊发凉）只留给本章至多一个峰值；也允许情绪含混不明说。",
        "2. 句长参差：禁止连续三句以上长度相近；该长则长，该短则短；允许一句话成段。",
        "3. 不解释主题：叙述者不总结「这意味着什么」，不写「她终于明白……」，不写格言式收尾；让事件自己说话。",
        "4. 对话用「说/道」即可，重复不丢人；不必轮换低语/咕哝/嗤笑；对话里允许啊/吧/呢/嘛等语气词。",
        "5. 感官适度：一段最多两种感官，不堆三感；天气不映心情（除非本拍就是情感峰值）。",
        "6. 具体压倒抽象：写得出名字的实物、动作、数目，替代「说不清的感觉」类包装。",
        "7. 不写全知总结：不用「殊不知」「无人知晓」「命运的齿轮」类预告腔收束。",
    ]
    .join("\n")
}

/// 本章手法池（按章号轮换，一次只给一条）。
///
/// 依据 sepia 的「Select, don't accumulate」：人类文本的多样性来自**每篇选
/// 3–5 个手法**，不是把全部规则堆上去——堆满会变成新的机器指纹（过度纠正）。
/// 逐章轮换让全书在多个人类手法之间散射，而不是每章同一个模子。
pub const TECHNIQUE_POOL: &[&str] = &[
    "本章手法｜迟一拍揭示：把本场最要紧的信息放到段落最后一句，而不是开头先交代。",
    "本章手法｜无用细节：写一个不服务情节的具体细节（灯油的价钱、旧疤、招牌上的错字），不解释它。",
    "本章手法｜配角不互识：让两个在场者只通过第三人认识，或干脆互不搭理；不是所有人都彼此有戏。",
    "本章手法｜离题回忆：让某人走神想起与主线无关的旧事（二十字以内），立即收回，不解释关联。",
    "本章手法｜时间错位：插入一句回望或预叙，打散线性；不超两行。",
    "本章手法｜不体面的举动：给主角一个不高尚、不必要但真实的小动作（抠桌面、抢话、说谎）。",
    "本章手法｜平直命名：至少一次用直白句说出情绪或关系，不比喻、不身体反应。",
    "本章手法｜收束早一拍：本段在「还想再写一句」的地方停下，把余味留给空白。",
];

/// 取本章手法（章号轮换；章号缺失时用 0）。
pub fn technique_for_chapter(chapter_number: i64) -> &'static str {
    let idx = chapter_number.rem_euclid(TECHNIQUE_POOL.len() as i64) as usize;
    TECHNIQUE_POOL[idx]
}

/// 按「已写正文字数」轮换手法。
///
/// 每拍写作提示词的组装点拿不到章号（`render_writer_user_prompt` 的参数从
/// 各处来），但正文长度是天然进度标尺：每约 800 字换一个手法，同一章的不同
/// 拍之间自然散射，跨章也不会停在同一手法上；且完全确定、可测。
pub fn technique_for_progress(prose_chars: usize) -> &'static str {
    let idx = (prose_chars / 800) % TECHNIQUE_POOL.len();
    TECHNIQUE_POOL[idx]
}

/// 渲染注入块：固定准则 + 本章手法（约 500 字，逐拍注入）。
pub fn render_guidance_block(chapter_number: i64) -> String {
    format!(
        "{}\n{}",
        render_guidance_rules(),
        technique_for_chapter(chapter_number)
    )
}

/// 渲染注入块（按已写正文字数轮换手法）。
pub fn render_guidance_block_for_progress(prose_chars: usize) -> String {
    format!(
        "{}\n{}",
        render_guidance_rules(),
        technique_for_progress(prose_chars)
    )
}

/// 渲染给编辑器的审查块（无发现时返回 None，不产生空块）。
pub fn render_audit_block(report: &HumanVoiceReport, max: usize) -> Option<String> {
    if report.findings.is_empty() {
        return None;
    }
    let lines: Vec<String> = report
        .findings
        .iter()
        .take(max)
        .map(|f| {
            let tag = if f.kind == "deficit" {
                "缺失"
            } else {
                "过量"
            };
            if f.excerpt.is_empty() {
                format!("- [{tag}] {}", f.detail)
            } else {
                format!("- [{tag}] {}｜命中：{}", f.detail, f.excerpt)
            }
        })
        .collect();
    Some(format!(
        "【人类文笔基线（机器检出，供裁决；逐条判断是否值得改，过度纠正本身是新的指纹）】\n{}",
        lines.join("\n")
    ))
}

/// 落库用的紧凑摘要（每条一行，最多 6 条）。
pub fn summarize_findings(report: &HumanVoiceReport) -> Vec<String> {
    report
        .findings
        .iter()
        .take(6)
        .map(|f| format!("[文笔基线/{}] {}｜{}", f.kind, f.rule, f.detail))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_rhythm_flags_uniform_sentence_run_but_not_varied_prose() {
        // 三句长度趋同（每句 10 字以上、彼此相差不到三分之一）→ 报
        let flat =
            "他推开门走了进去，屋里没人。她抬头看了他一眼，没有作声。桌上摆着两杯还温着的茶。";
        let report = analyze_human_voice(flat);
        assert!(
            report.findings.iter().any(|f| f.rule == "flat-rhythm"),
            "{:?}",
            report.findings
        );

        // 长短短参差 → 不报
        let varied =
            "他推开门。里面很暗，只有窗台上一盏灯亮着，灯芯烧得歪歪扭扭，像随时要灭。她抬头。";
        let varied_report = analyze_human_voice(varied);
        assert!(
            !varied_report
                .findings
                .iter()
                .any(|f| f.rule == "flat-rhythm"),
            "{:?}",
            varied_report.findings
        );
    }

    #[test]
    fn embodied_only_is_a_deficit_not_plain_naming() {
        // 全是身体反应、零平直命名 → deficit
        let embodied = "她心里一紧。他背脊发凉。她喉咙发紧。";
        let report = analyze_human_voice(embodied);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "embodied-only" && f.kind == "deficit"),
            "{:?}",
            report.findings
        );

        // 平直命名情绪（人类句式）不得被记成缺陷
        let plain = "她害怕。他恨她。她心里一紧。";
        let plain_report = analyze_human_voice(plain);
        assert!(
            !plain_report
                .findings
                .iter()
                .any(|f| f.rule == "embodied-only"),
            "平直命名不应触发: {:?}",
            plain_report.findings
        );
        assert!(plain_report.metrics.plain_emotion_count >= 2);
    }

    #[test]
    fn mood_particle_absence_only_with_dialogue() {
        let no_dialogue = "他推开门走了出去，雨声很大，他把领口立起来。";
        let report = analyze_human_voice(no_dialogue);
        assert!(!report
            .findings
            .iter()
            .any(|f| f.rule == "mood-particle-absent"));

        let flat_dialogue =
            "「你来了。」他说。「我知道了。」她回答。「请坐，别站着。」他指了指椅子。\
「这茶是今早新沏的，你尝尝温度。」她把盏推过去。「路上还顺利。」他说。「还行，只是雨大。」\
「那就先歇一阵。」「城外堵了半日。」「你先喝。」「好。」";
        let dialogue_report = analyze_human_voice(flat_dialogue);
        assert!(
            dialogue_report
                .findings
                .iter()
                .any(|f| f.rule == "mood-particle-absent"),
            "{:?}",
            dialogue_report.findings
        );

        let human_dialogue =
            "「你来了啊。」他说。「嗯，我知道呢。」她回答。「请坐，别站着。」他指了指椅子。\
「这茶是今早新沏的，你尝尝温度。」她把盏推过去。「路上还顺利。」他说。「还行，只是雨大。」\
「那就先歇一阵。」「城外堵了半日。」「你先喝。」「好。」";
        let human_report = analyze_human_voice(human_dialogue);
        assert!(
            !human_report
                .findings
                .iter()
                .any(|f| f.rule == "mood-particle-absent"),
            "{:?}",
            human_report.findings
        );
    }

    #[test]
    fn fancy_tags_flagged_only_when_plain_said_is_absent() {
        let fancy_only = "「不行，」他低语。「随你，」她嗤笑。「也罢，」他咕哝。";
        let report = analyze_human_voice(fancy_only);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "fancy-speech-tags"),
            "{:?}",
            report.findings
        );

        let with_plain = "「不行，」他低语。「随你，」她嗤笑。「也罢，」他咕哝。他说道。";
        let plain_report = analyze_human_voice(with_plain);
        assert!(
            !plain_report
                .findings
                .iter()
                .any(|f| f.rule == "fancy-speech-tags"),
            "{:?}",
            plain_report.findings
        );
    }

    #[test]
    fn connective_stack_and_disyllabic_padding_are_detected() {
        let text = "他今天进行讨论了方案，并且因此同时加上以及后来又改了主意。";
        let report = analyze_human_voice(text);
        assert!(
            report.findings.iter().any(|f| f.rule == "connective-stack"),
            "{:?}",
            report.findings
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "disyllabic-padding"),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn abstract_wrapper_needs_cluster() {
        let single = "她有一种感觉。";
        assert!(!analyze_human_voice(single)
            .findings
            .iter()
            .any(|f| f.rule == "abstract-wrapper"));

        let clustered = "她有一种感觉。那是一股寒意。他是沉默的象征，无声的。";
        assert!(analyze_human_voice(clustered)
            .findings
            .iter()
            .any(|f| f.rule == "abstract-wrapper"));
    }

    #[test]
    fn clean_literary_text_produces_no_findings() {
        let text = "他把刀收回鞘里。\n\
                    雨还在下。屋檐上的水一滴一滴砸在青石板上，溅起来又落回去。\n\
                    「走吧。」她说。\n\
                    「再等等吧。」他盯着远处，没动。\n\
                    她没有再劝。风把灯笼吹得晃了一下，又稳住了。";
        let report = analyze_human_voice(text);
        assert!(
            report.findings.is_empty(),
            "文学性文本不应误报: {:?}",
            report.findings
        );
    }

    #[test]
    fn technique_rotates_by_chapter_and_guidance_is_compact() {
        let a = technique_for_chapter(1);
        let b = technique_for_chapter(2);
        assert_ne!(a, b);
        assert_eq!(technique_for_chapter(1 + TECHNIQUE_POOL.len() as i64), a);
        // 负数章号不得 panic（rem_euclid）
        let _ = technique_for_chapter(-3);

        let block = render_guidance_block(5);
        assert!(block.contains("人类文笔基线"));
        assert!(block.contains("本章手法"));
        assert!(
            block.chars().count() < 700,
            "注入块过长: {}",
            block.chars().count()
        );
    }

    #[test]
    fn audit_block_is_bounded_and_empty_when_clean() {
        assert!(render_audit_block(&HumanVoiceReport::default(), 6).is_none());
        let report = analyze_human_voice(
            "他推开门走了进去，屋里没人。她抬头看了他一眼，没有作声。桌上摆着两杯还温着的茶。",
        );
        let block = render_audit_block(&report, 6).expect("应有块");
        assert!(block.contains("人类文笔基线"));
        let summary = summarize_findings(&report);
        assert!(!summary.is_empty() && summary.len() <= 6);
    }
}

#[cfg(test)]
mod machine_probe {
    use super::*;

    /// 机器腔验收样本（按本项目真机事故记录构造的症状集合）：
    /// 具身化情绪密集、连词堆叠、语气词缺失、花式对话标签轮换、抽象包装、
    /// 句长趋同、章尾预告腔。
    const AI_FLAVORED: &str = "宴厅里的空气仿佛凝固了。\n\
林雪心里一紧。她感到一股难以言喻的寒意，背脊发凉，喉咙发紧。\
「你错了。」她低语。「我没有错。」他嗤笑。「你必须明白。」她嘟囔。\
「这是命定的。」他咕哝。\n\
他感到一种无形的压力。那是一种宿命的重量，无声的，莫名的。\
他进行了一番思考，并且因此同时加上后来终于做出了决定。\n\
她知道，这一切都只是开始。殊不知，命运的齿轮已经开始转动。";

    /// 人类腔验收样本（平直命名 + 语气词 + 长短参差 + 重复「说」+ 具体实物）。
    const HUMAN_FLAVORED: &str =
        "雨停了。屋檐上最后一滴水砸在青石板上，啪的一声，溅起来，又落回去。\n\
「你来了啊。」他说。\n\
「嗯，路上堵了半日呢。」她说。\n\
「先喝口茶吧。」他把盏推过去，「今早新沏的，三分钱一两。」\n\
她害怕。她自己也知道这害怕没什么道理——院墙外头是条死巷，\
除了收夜香的老周，半夜不会有人经过。可她还是把门闩上了，两遍。\n\
他看着她闩门，没笑她，只是把灯芯剪短了一截。屋子里暗下来，\
两个人坐着，谁也没再说话。";

    /// 设计验收探针：机器腔样本必须被检出，人类腔样本不得误报，且注入块
    /// 与手法轮换按设计工作。
    ///
    /// 库里有真实章节时会一并分析（副本，不碰原库）：
    /// ```bash
    /// cargo test --lib human_voice::machine_probe -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn voice_baseline_acceptance_probe() {
        // 1) 机器腔：至少命中 3 条（灵敏度）
        let ai = analyze_human_voice(AI_FLAVORED);
        let ai_rules: Vec<&str> = ai.findings.iter().map(|f| f.rule.as_str()).collect();
        println!("── 机器腔样本命中 {} 条：{:?}", ai_rules.len(), ai_rules);
        println!("   metrics={:?}", ai.metrics);
        assert!(
            ai_rules.len() >= 3,
            "机器腔样本命中不足（灵敏度不足）: {ai_rules:?}"
        );

        // 2) 人类腔：零误报（特异性）
        let human = analyze_human_voice(HUMAN_FLAVORED);
        let human_rules: Vec<&str> = human.findings.iter().map(|f| f.rule.as_str()).collect();
        println!(
            "── 人类腔样本命中 {} 条：{:?}",
            human_rules.len(),
            human_rules
        );
        println!("   metrics={:?}", human.metrics);
        assert!(
            human_rules.is_empty(),
            "人类腔样本被误报（过度纠正风险）: {human_rules:?}"
        );

        // 3) 注入块与手法轮换
        let block = render_guidance_block(5);
        println!(
            "── 写作准则块（第 5 章，{} 字）:\n{block}",
            block.chars().count()
        );
        println!(
            "── 轮换：progress 0/800/1600 → {} / {} / {}",
            technique_for_progress(0),
            technique_for_progress(800),
            technique_for_progress(1600)
        );

        // 4) 有真机库时附跑（副本；无故事数据则跳过）
        let src = std::env::var("STORYMOSS_DB").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/Library/Application Support/com.storymoss.app/cinema_ai.db")
        });
        let src_path = std::path::PathBuf::from(&src);
        if !src_path.exists() {
            println!("── 跳过真机库（不存在）: {src}");
            return;
        }
        let tmp = std::env::temp_dir().join(format!(
            "storymoss-voice-probe-{}",
            chrono::Local::now().timestamp_millis()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        for suffix in ["", "-wal", "-shm"] {
            let from = std::path::PathBuf::from(format!("{src}{suffix}"));
            if from.exists() {
                std::fs::copy(&from, tmp.join(format!("cinema_ai.db{suffix}"))).unwrap();
            }
        }
        let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库");
        let conn = pool.get().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT s.content FROM scenes s \
                 WHERE s.content IS NOT NULL AND length(s.content) > 1200 \
                 ORDER BY s.sequence_number DESC LIMIT 6",
            )
            .expect("查询 scenes");
        let texts: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        if texts.is_empty() {
            println!("── 真机库无章节正文（跳过）");
        }
        for (i, text) in texts.iter().enumerate() {
            let report = analyze_human_voice(text);
            let rules: Vec<&str> = report.findings.iter().map(|f| f.rule.as_str()).collect();
            println!(
                "── 真机样章 {}（{} 字）命中 {} 条：{:?}",
                i + 1,
                text.chars().count(),
                rules.len(),
                rules
            );
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
