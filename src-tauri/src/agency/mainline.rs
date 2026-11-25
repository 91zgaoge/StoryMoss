//! v0.99.15：续写训练的「原著主线投影」。
//!
//! 真机复盘（2026-11-24 两轮 10 章训练）显示：资产重建的故事大纲
//! `【下一节点】` 只由上下文（134–143 章）归纳，指向「就地审问伪装护士」；
//! 真机 writer 严格按这条硬大纲写，把原书第 144 章的「福摩萨夜总会正面冲突
//! 邵卫江」推迟到「明日」，随后另起老曹—法林寺—三理教平行支线，10 章都
//! 没有回到原书主线。上一轮报告里的「真实后续节点」只有 1–2 行，且是**上一
//! 轮**编审的转述，优先级远低于故事大纲/章纲，无法纠正世界线。
//!
//! 本模块把**本轮真实后续章**（truth_hints 模式；blind 模式不构建）变成逐章
//! 硬任务：
//! - `goal`：该真实章已发生的主线事件（由真实章结构化摘要派生）；
//! - `key_terms`：该章真实出场的人物/地点/关键物（用于确定性落地探针）；
//! - 注入章纲规划与 writer 的「本章原著主线任务」，并作为编审 gate 的核对项；
//! - 生成后做确定性探针，报告逐章主线是否落地。
//!
//! 只用于训练沙盒；普通续写不构建、不注入（builder 默认为 None）。

use crate::agency::story_brief::{BriefChapter, PROTAGONIST_ALIASES};

/// 单章原著主线任务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainlineBeat {
    pub chapter_number: i32,
    /// 原书该章标题（如「法不可亵，术不可显」）。
    pub title: String,
    /// 该章已发生的主线事件/结果（≤200 字）。
    pub goal: String,
    /// 该章最大转折（必须写出机制，不得只写结果；≤80 字）。
    pub key_turn: String,
    /// 真实章关键对白语感（可化用；truth_hints 教学信号，≤3 条/40 字）。
    pub voice_cues: Vec<String>,
    /// 该章真实出场的主要人物（已排除主角占位名；用于节拍卡对齐）。
    pub cast: Vec<String>,
    /// 该章主要地点（摘要里的第一个地点段；用于节拍卡对齐）。
    pub location: String,
    /// 真实出场的人物/地点/关键物（用于提示；已排除主角占位名）。
    pub key_terms: Vec<String>,
    /// 硬落地核对要素：人物/地点等必须出现（或等价别名）的高精度项。
    /// 比 `key_terms` 少，避免把摘要里的普通器物当硬指标导致误判。
    pub must_terms: Vec<String>,
}

/// 一批真实后续章的主线投影。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MainlineProjection {
    /// 原书主线走势（章标题链，≤400 字）。
    pub arc: String,
    pub beats: Vec<MainlineBeat>,
}

/// 确定性落地探针结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MainlineCoverage {
    /// 报告用一行（逐章 ✓/✗ 与缺失要素）。
    pub line: String,
    /// 未落地章节与缺失要素。
    pub misses: Vec<(i32, Vec<String>)>,
}

impl MainlineProjection {
    pub fn is_empty(&self) -> bool {
        self.beats.is_empty()
    }

    pub fn beat_for(&self, chapter_number: i32) -> Option<&MainlineBeat> {
        self.beats
            .iter()
            .find(|beat| beat.chapter_number == chapter_number)
    }

    /// 给 writer 的稳定块（整批固定，放在资产区尾部）：一次看到本批主线。
    pub fn render_assets_block(&self, compact: bool) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut lines = String::new();
        for beat in &self.beats {
            let goal_cap = if compact { 80 } else { 180 };
            let goal: String = beat.goal.chars().take(goal_cap).collect();
            let terms = beat
                .key_terms
                .iter()
                .take(if compact { 4 } else { 6 })
                .cloned()
                .collect::<Vec<_>>()
                .join("、");
            if terms.is_empty() {
                lines.push_str(&format!("- 第{}章：{}\n", beat.chapter_number, goal));
            } else {
                lines.push_str(&format!(
                    "- 第{}章：{}｜关键：{}\n",
                    beat.chapter_number, goal, terms
                ));
            }
        }
        let arc = if self.arc.trim().is_empty() {
            String::new()
        } else {
            format!("\n【主线弧（升级方向）】{}", self.arc.trim())
        };
        let block = format!(
            "【原著主线投影（本批续写必须按原书主线推进；细节可自由发挥，但下列逐章主线事件/地点/关键人物不得推迟、替换或用电话/转述替代）】\n{lines}{arc}\n\
             【硬约束】每章先落地本章主线任务，再处理其他支线；不得把本章主线推到后续章节；\
             不得另开与原书主线平行的长期支线；若本章主线与本地线索冲突，以本章主线为准，并把本地线索降为支线并行处理。"
        );
        if compact {
            crate::agency::continue_assets::truncate_chars(&block, 900)
        } else {
            crate::agency::continue_assets::truncate_chars(&block, 1600)
        }
    }

    /// 写进沙盒故事大纲（gate/planner 都读它）。
    pub fn render_outline_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut out = String::from(
            "【原著主线投影（本批续写必须按此原书主线推进；细节可自由发挥，主线事件/地点/关键人物不可替换或推迟）】\n",
        );
        for beat in &self.beats {
            let goal: String = beat.goal.chars().take(200).collect();
            let terms = beat
                .key_terms
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join("、");
            if terms.is_empty() {
                out.push_str(&format!("- 第{}章：{}\n", beat.chapter_number, goal));
            } else {
                out.push_str(&format!(
                    "- 第{}章：{}｜关键：{}\n",
                    beat.chapter_number, goal, terms
                ));
            }
        }
        if !self.arc.trim().is_empty() {
            out.push_str(&format!("【主线弧】{}\n", self.arc.trim()));
        }
        out
    }

    /// writer / 章纲规划的「本章原著主线任务」硬块。
    pub fn render_beat_block(&self, chapter_number: i32) -> Option<String> {
        let beat = self.beat_for(chapter_number)?;
        let terms = beat
            .key_terms
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        let mut block = format!(
            "【本章原著主线任务（最高优先级；必须落地）】\n第{}章：{}\n",
            beat.chapter_number, beat.goal
        );
        if !beat.location.trim().is_empty() {
            block.push_str(&format!("地点：{}\n", beat.location));
        }
        if !terms.is_empty() {
            block.push_str(&format!("关键人物/地点：{terms}\n"));
        }
        if !beat.key_turn.trim().is_empty() {
            block.push_str(&format!(
                "【本章关键转折（必须写出机制，不得只写结果）】{}\n",
                beat.key_turn.trim()
            ));
        }
        if !beat.voice_cues.is_empty() {
            block.push_str("【本章关键对白语感（化用其口气与句式；不要整段照抄旁白）】\n");
            for cue in &beat.voice_cues {
                block.push_str(&format!("- 「{}」\n", cue));
            }
        }
        block.push_str(
            "要求：本章的核心冲突/目的地/关键人物必须与上述主线事件一致；不得把该事件推迟到后续章节，\
             不得用电话、短信、转述、跟踪或事后听说替代正面场面；可在细节与支线上自由发挥。\
             若与本章大纲/本地线索冲突，以本任务为准并调整章纲。",
        );
        Some(block)
    }

    /// 编审 gate 的核对块（含确定性缺失要素，帮助 editor 直接判 revise）。
    /// 返回 `(提示块, 缺失要素)`；缺失要素同时供 gate 硬拦截使用。
    pub fn render_gate_block(
        &self,
        chapter_number: i32,
        draft_plain: &str,
    ) -> Option<(String, Vec<String>)> {
        let beat = self.beat_for(chapter_number)?;
        let terms: &[String] = if beat.must_terms.is_empty() {
            &beat.key_terms
        } else {
            &beat.must_terms
        };
        let missing: Vec<String> = terms
            .iter()
            .filter(|term| !term_matches(draft_plain, term))
            .cloned()
            .collect();
        let terms_text = if terms.is_empty() {
            "（无）".to_string()
        } else {
            terms.join("、")
        };
        let check = if missing.is_empty() {
            "确定性核对：主要人物/地点均在正文出现。".to_string()
        } else {
            format!(
                "确定性核对：以下主线要素未在正文出现：{}。",
                missing.join("、")
            )
        };
        let block = format!(
            "【本章原著主线任务（编辑核对用）】\n第{}章：{}\n关键人物/地点：{terms_text}\n{check}\n\
             裁决要求：若本章主线事件被推迟/替换/用转述替代，或上述必需人物/地点缺失且无等价替代，\
             判 revise 并在 blocking_issues 写明具体修正；若正文以等价方式落地，不扣分。",
            beat.chapter_number, beat.goal
        );
        Some((block, missing))
    }

    /// 生成后确定性探针：逐章是否至少命中一个主线关键要素。
    pub fn coverage(&self, generated: &[(i32, String)]) -> MainlineCoverage {
        if self.is_empty() {
            return MainlineCoverage {
                line: "未启用（blind 或未构建主线投影）".to_string(),
                misses: Vec::new(),
            };
        }
        let mut parts = Vec::new();
        let mut misses = Vec::new();
        for (number, text) in generated {
            let Some(beat) = self.beat_for(*number) else {
                continue;
            };
            if text.trim().is_empty() {
                parts.push(format!("第{number}章 ✗（空稿）"));
                misses.push((*number, beat.key_terms.clone()));
                continue;
            }
            if beat.key_terms.is_empty() {
                parts.push(format!("第{number}章 —（无关键要素可核）"));
                continue;
            }
            let terms: &[String] = if beat.must_terms.is_empty() {
                &beat.key_terms
            } else {
                &beat.must_terms
            };
            let hits: Vec<&str> = terms
                .iter()
                .filter(|term| term_matches(text, term))
                .map(|term| term.as_str())
                .collect();
            if hits.is_empty() {
                parts.push(format!(
                    "第{number}章 ✗（缺 {}）",
                    terms.iter().take(4).cloned().collect::<Vec<_>>().join("/")
                ));
                misses.push((*number, terms.to_vec()));
            } else {
                parts.push(format!(
                    "第{number}章 ✓ {}/{}（{}）",
                    hits.len(),
                    terms.len(),
                    hits.iter().take(3).copied().collect::<Vec<_>>().join("/")
                ));
            }
        }
        let line = if parts.is_empty() {
            "无逐章主线任务".to_string()
        } else {
            parts.join("｜")
        };
        MainlineCoverage { line, misses }
    }
}

/// 从真实后续章的**结构化摘要**构建投影。
///
/// 摘要来自 `training_assets::summarize_chapters`（逐章小 prompt，单章失败会
/// 退化到确定性摘要）。`context_prose` 用于过滤摘要里偶尔出现的幻觉专名：
/// 真实章正文里没有的术语不进 key_terms。
pub fn from_briefs(
    truths: &[(usize, String)],
    briefs: &[BriefChapter],
    protagonist: Option<&str>,
) -> MainlineProjection {
    let mut beats = Vec::with_capacity(truths.len());
    for (number, content) in truths {
        let chapter = *number as i32;
        let plain = crate::export::content::content_to_plain_text(content);
        let brief = briefs.iter().find(|b| b.chapter == chapter);
        let title = crate::utils::chapter_title::source_chapter_title(&plain)
            .and_then(|raw| crate::utils::chapter_title::strip_chapter_prefix(&raw))
            .unwrap_or_default();
        let goal = brief
            .map(|b| beat_goal(b, &plain))
            .filter(|g| !g.trim().is_empty())
            .unwrap_or_else(|| deterministic_goal(&plain));
        let terms = brief
            .map(|b| collect_terms(b, &plain, protagonist))
            .unwrap_or_else(|| {
                let key_terms = deterministic_key_terms(&plain, protagonist);
                let must_terms = key_terms.iter().take(2).cloned().collect();
                BeatTerms {
                    key_terms,
                    must_terms,
                    cast: Vec::new(),
                    location: String::new(),
                }
            });
        let key_turn = brief
            .map(|b| b.turning_point.trim().chars().take(80).collect::<String>())
            .unwrap_or_default();
        let voice_cues = extract_voice_cues(&plain, &terms.cast);
        beats.push(MainlineBeat {
            chapter_number: chapter,
            title,
            goal,
            key_turn,
            voice_cues,
            cast: terms.cast,
            location: terms.location,
            key_terms: terms.key_terms,
            must_terms: terms.must_terms,
        });
    }
    let arc = render_arc(&beats);
    MainlineProjection { arc, beats }
}

/// 从真实章抽 2–3 条「关键对白语感」。
///
/// truth_hints 训练的教学信号：真机 editor 反复点名「陆尘音亮枪的『妖孽，
/// 看我法宝！』、黄玄然『欲成大事，当行阳谋』」等关键台词没有进入 writer，
/// 新角色只能靠摘要里的性格标签，声音不像作者。抽取真实章引号短句（≤40
/// 字、含！/？、含人物名或我/你优先），供 writer 化用口气与句式，不整段
/// 照抄旁白。
fn extract_voice_cues(plain: &str, cast: &[String]) -> Vec<String> {
    let chars: Vec<char> = plain.chars().collect();
    let pairs = [('“', '”'), ('「', '」'), ('『', '』')];
    let mut candidates: Vec<(String, i32, usize)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (open, close) in pairs {
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != open {
                i += 1;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < chars.len() && chars[j] != close {
                j += 1;
            }
            if j > start {
                let token: String = chars[start..j].iter().collect();
                let token = token.trim().to_string();
                i = j + 1;
                let len = token.chars().count();
                if len < 4 || len > 40 {
                    continue;
                }
                const GENERIC: [&str; 8] = [
                    "不知道",
                    "什么",
                    "不是",
                    "没有",
                    "这个",
                    "那个",
                    "怎么回事",
                    "嗯",
                ];
                if GENERIC.iter().any(|g| token == *g) {
                    continue;
                }
                if token.contains("第") && token.contains("章") {
                    continue;
                }
                let mut score = 0i32;
                if token.contains('！') || token.contains('？') {
                    score += 2;
                }
                if cast.iter().any(|name| token.contains(name.as_str())) {
                    score += 2;
                }
                if token.contains('我') || token.contains('你') {
                    score += 1;
                }
                if score == 0 {
                    score += 1;
                }
                let key: String = token.chars().take(12).collect();
                if seen.insert(key) {
                    candidates.push((token, score, start));
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.0.chars().count().cmp(&b.0.chars().count()))
            .then(a.2.cmp(&b.2))
    });
    candidates
        .into_iter()
        .map(|(token, _, _)| token.chars().take(40).collect())
        .take(3)
        .collect()
}

fn beat_goal(brief: &BriefChapter, plain: &str) -> String {
    // 摘要本身已经包含「谁在哪做了什么/结果」；result/turning_point 再拼
    // 会产生「…真本事。；周成未表演…；邵卫江逼…」的重复长句，挤占
    // 章纲/writer 的注意力。只有摘要过短时才补 result/turning_point。
    let summary = brief.summary.trim();
    if summary.chars().count() >= 60 {
        return summary.chars().take(200).collect();
    }
    let mut parts: Vec<String> = Vec::new();
    for text in [&brief.summary, &brief.result, &brief.turning_point] {
        let t = text.trim();
        if !t.is_empty() && !parts.iter().any(|p| p.contains(t)) {
            parts.push(t.to_string());
        }
    }
    let joined = parts.join("；");
    if joined.chars().count() > 200 {
        joined.chars().take(200).collect()
    } else if joined.trim().is_empty() {
        deterministic_goal(plain)
    } else {
        joined
    }
}

fn deterministic_goal(plain: &str) -> String {
    let summary = crate::agency::training_assets::deterministic_chapter_summary(0, plain);
    summary.chars().take(200).collect()
}

struct BeatTerms {
    key_terms: Vec<String>,
    must_terms: Vec<String>,
    cast: Vec<String>,
    location: String,
}

fn collect_terms(brief: &BriefChapter, plain: &str, protagonist: Option<&str>) -> BeatTerms {
    let mut key_terms: Vec<String> = Vec::new();
    let mut cast_terms: Vec<String> = Vec::new();
    for name in &brief.cast {
        add_term(&mut key_terms, name, plain, protagonist);
        add_term(&mut cast_terms, name, plain, protagonist);
    }
    let location = brief
        .location
        .split(['、', '，', ',', '及', '/'])
        .map(str::trim)
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
        .to_string();
    if !location.is_empty() {
        add_term(&mut key_terms, &location, plain, protagonist);
    }
    for term in &brief.key_terms {
        add_term(&mut key_terms, term, plain, protagonist);
    }
    key_terms.truncate(6);
    // 硬核对取「真实章里出现次数最多的前两个人物 + 第一个地点」：
    // cast 顺序常把只出现一次的次要人物排在前面（小护士/路少），全要或按
    // 顺序取都会误判。出现频次更接近「本章主线人物」。
    let mut ranked: Vec<(String, usize)> = cast_terms
        .iter()
        .map(|term| {
            let count = plain.matches(term.as_str()).count();
            (term.clone(), count)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let cast: Vec<String> = ranked.iter().map(|(term, _)| term.clone()).collect();
    let mut must_terms: Vec<String> = ranked
        .iter()
        .take(2)
        .map(|(term, _)| term.clone())
        .collect();
    if !location.is_empty() {
        let mut location_only = Vec::new();
        add_term(&mut location_only, &location, plain, protagonist);
        if let Some(term) = location_only.into_iter().next() {
            if !must_terms.iter().any(|existing| existing == &term) {
                must_terms.push(term);
            }
        }
    }
    must_terms.truncate(3);
    if must_terms.is_empty() {
        must_terms.extend(key_terms.iter().take(2).cloned());
    }
    BeatTerms {
        key_terms,
        must_terms,
        cast,
        location,
    }
}

fn add_term(out: &mut Vec<String>, term: &str, plain: &str, protagonist: Option<&str>) {
    let term = term.trim();
    if term.chars().count() < 2 || !plain.contains(term) {
        return;
    }
    if is_protagonist_term(term, protagonist) || is_generic_term(term) {
        return;
    }
    if !out.iter().any(|existing| existing == term) {
        out.push(term.to_string());
    }
}

fn deterministic_key_terms(plain: &str, protagonist: Option<&str>) -> Vec<String> {
    // 摘要 LLM 失败时的兜底：只保留「真实章正文反复出现且不是主角/通用词」
    // 的 2–4 字窗口。宁可少报，不可把普通短语当主线要素。
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let chars: Vec<char> = plain.chars().collect();
    for n in 2..=4 {
        if chars.len() < n {
            continue;
        }
        for i in 0..=chars.len() - n {
            let term: String = chars[i..i + n].iter().collect();
            if !term.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
                continue;
            }
            if term.chars().any(|c| FUNCTION_CHARS.contains(c)) {
                continue;
            }
            if is_generic_term(&term) {
                continue;
            }
            if let Some(last) = term.chars().last() {
                if TERMINAL_LOCATIVE_CHARS.contains(last) {
                    continue;
                }
            }
            *counts.entry(term).or_insert(0) += 1;
        }
    }
    let mut candidates: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .collect();
    // 长串优先，去掉被更长串包含的碎片。
    candidates.sort_by(|a, b| {
        b.0.chars()
            .count()
            .cmp(&a.0.chars().count())
            .then(b.1.cmp(&a.1))
    });
    let mut kept: Vec<(String, usize)> = Vec::new();
    for item in candidates {
        if kept.iter().any(|(term, _)| term.contains(item.0.as_str())) {
            continue;
        }
        if is_protagonist_term(&item.0, protagonist) {
            continue;
        }
        kept.push(item);
    }
    kept.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    kept.into_iter().map(|(term, _)| term).take(6).collect()
}

const FUNCTION_CHARS: &str = "你我您咱俺他她它的地得了着过把被给让这那哪个们吗呢吧啊呀哦嗯\
                              是有像就又也都还再很太更最没别声缕";
const TERMINAL_LOCATIVE_CHARS: &str = "上下里外边前后";

/// 主角的占位名与权威姓名都不能当「主线必须出现」的要素：第一人称叙事里
/// 「我」永远在，拿它做探针会恒真。
fn is_protagonist_term(term: &str, protagonist: Option<&str>) -> bool {
    let term = term.trim();
    if PROTAGONIST_ALIASES.contains(&term) {
        return true;
    }
    if let Some(name) = protagonist.map(str::trim).filter(|n| !n.is_empty()) {
        if term == name {
            return true;
        }
    }
    false
}

/// 主线要素匹配：全名命中，或去掉通用中心语后的别名命中。
///
/// 真机 `福摩萨夜总会` 常被写成「福摩萨」、`大波浪卷女人` 常被写成
/// 「大波浪卷」；只做全等 `contains` 会把已落地的章误判为缺失，进而
/// 触发不必要的强制修订。
fn term_matches(text: &str, term: &str) -> bool {
    let term = term.trim();
    if term.is_empty() {
        return true;
    }
    if text.contains(term) {
        return true;
    }
    let chars: Vec<char> = term.chars().collect();
    if chars.len() >= 4 {
        let prefix: String = chars[..chars.len() - 2].iter().collect();
        if prefix.chars().count() >= 2 && text.contains(&prefix) {
            return true;
        }
    }
    const GENERIC_SUFFIXES: [&str; 13] = [
        "夜总会",
        "医院",
        "病房",
        "包厢",
        "道观",
        "寺庙",
        "客栈",
        "码头",
        "广场",
        "胡同",
        "巷子",
        "女人",
        "男人",
    ];
    for suffix in GENERIC_SUFFIXES {
        if let Some(stripped) = term.strip_suffix(suffix) {
            if stripped.chars().count() >= 2 && text.contains(stripped) {
                return true;
            }
        }
    }
    // v0.99.18：地点同义词。真机第 146 章主线地点是「夜总会」，正文却写
    // 「吧台/门厅/珠帘门/场子」——没出现「夜总会」三个字，确定性探针
    // 判缺失并强制修订，而 editor 已判 pass。场景类型等价词视为命中。
    const VENUE_SYNONYMS: [(&str, &[&str]); 8] = [
        (
            "夜总会",
            &[
                "包厢", "吧台", "门厅", "场子", "会所", "大厅", "夜场", "舞厅",
            ],
        ),
        (
            "医院",
            &["病房", "走廊", "诊室", "急诊", "护士站", "住院部", "手术室"],
        ),
        ("包厢", &["包间", "雅间", "里屋", "厅里", "座上"]),
        ("道观", &["观里", "三清殿", "大殿", "庙里", "殿里", "山门"]),
        ("寺庙", &["寺里", "庙里", "大殿", "佛堂", "山门"]),
        ("客栈", &["店里", "楼上", "房间", "客店", "店堂"]),
        ("码头", &["江边", "岸边", "渡口", "船坞"]),
        ("胡同", &["巷子", "巷口", "胡同口", "巷里"]),
    ];
    for (venue, synonyms) in VENUE_SYNONYMS {
        if term == venue || term.ends_with(venue) {
            if synonyms.iter().any(|synonym| text.contains(*synonym)) {
                return true;
            }
        }
    }
    false
}

fn is_generic_term(term: &str) -> bool {
    const STOPWORDS: [&str; 57] = [
        "时候", "已经", "自己", "什么", "一个", "这个", "那个", "里面", "外面", "的话", "就是",
        "可是", "然后", "因为", "所以", "不是", "没有", "他们", "我们", "起来", "出来", "过去",
        "现在", "心里", "脸上", "身上", "手里", "眼睛", "声音", "地方", "东西", "事情", "感觉",
        "知道", "看着", "听见", "突然", "马上", "一下", "一点", "一声", "一句", "一边", "有些",
        "有点", "似乎", "仿佛", "果然", "居然", "竟然", "不由", "缓缓", "慢慢", "轻轻", "微微",
        "可以", "不能",
    ];
    STOPWORDS.iter().any(|word| term.contains(word))
}

fn render_arc(beats: &[MainlineBeat]) -> String {
    let labels: Vec<String> = beats
        .iter()
        .map(|beat| {
            if beat.title.trim().is_empty() {
                format!("第{}章", beat.chapter_number)
            } else {
                format!(
                    "第{}章 {}",
                    beat.chapter_number,
                    beat.title.chars().take(18).collect::<String>()
                )
            }
        })
        .collect();
    crate::agency::continue_assets::truncate_chars(&labels.join(" → "), 400)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(n: usize, text: &str) -> (usize, String) {
        (n, text.to_string())
    }

    fn brief(n: i32, summary: &str, cast: &[&str], location: &str) -> BriefChapter {
        BriefChapter {
            chapter: n,
            summary: summary.to_string(),
            location: location.to_string(),
            cast: cast.iter().map(|s| s.to_string()).collect(),
            result: format!("第{n}章结果"),
            ..Default::default()
        }
    }

    #[test]
    fn projection_builds_goal_and_key_terms_from_truth_briefs() {
        let truths = vec![
            chapter(
                144,
                "第144章 法不可亵，术不可显。我逛到了福摩萨夜总会，邵卫江在。",
            ),
            chapter(
                145,
                "第145章 叫人畏我惧我。邵卫江的人动手，我把汪必成按在墙上。",
            ),
        ];
        let briefs = vec![
            brief(
                144,
                "周成去福摩萨夜总会正面撞上邵卫江。",
                &["周成", "邵卫江"],
                "福摩萨夜总会",
            ),
            brief(
                145,
                "周成在福摩萨立威，汪必成服软。",
                &["周成", "邵卫江", "汪必成"],
                "福摩萨",
            ),
        ];
        let p = from_briefs(&truths, &briefs, Some("周成"));
        assert_eq!(p.beats.len(), 2);
        assert!(p.beats[0].goal.contains("福摩萨夜总会"));
        assert!(p.beats[0].key_terms.iter().any(|t| t == "邵卫江"));
        assert!(
            !p.beats[0].key_terms.iter().any(|t| t == "周成"),
            "主角不能当探针要素"
        );
        assert!(p.arc.contains("第144章"));
        // 章标题已经是「第一百四十四章 法不可亵」，arc 只加一次章号。
        assert!(
            !p.arc.contains("第144章 第一百四十四章"),
            "arc 不得重复章号: {}",
            p.arc
        );
        assert!(p.beats[0].cast.iter().any(|n| n == "邵卫江"));
        assert_eq!(p.beats[0].location, "福摩萨夜总会");
        assert!(
            p.beats[0].must_terms.iter().any(|t| t == "邵卫江"),
            "主线硬核对必须含主要人物"
        );
    }

    #[test]
    fn beat_block_forbids_deferring_mainline() {
        let p = MainlineProjection {
            arc: String::new(),
            beats: vec![MainlineBeat {
                chapter_number: 144,
                title: "法不可亵".into(),
                goal: "周成去福摩萨夜总会正面冲撞邵卫江。".into(),
                key_turn: String::new(),
                voice_cues: vec!["法不可亵，术不可显".into()],
                cast: vec!["邵卫江".into()],
                location: "福摩萨夜总会".into(),
                key_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
                must_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
            }],
        };
        let block = p.render_beat_block(144).unwrap();
        assert!(block.contains("不得把该事件推迟到后续章节"));
        assert!(block.contains("不得用电话、短信、转述"));
        assert!(block.contains("福摩萨夜总会"));
        assert!(p.render_beat_block(145).is_none());
    }

    #[test]
    fn coverage_flags_missing_mainline_terms() {
        let p = MainlineProjection {
            arc: String::new(),
            beats: vec![
                MainlineBeat {
                    chapter_number: 144,
                    title: String::new(),
                    goal: String::new(),
                    key_turn: String::new(),
                    voice_cues: vec![],
                    cast: vec!["邵卫江".into()],
                    location: "福摩萨夜总会".into(),
                    key_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
                    must_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
                },
                MainlineBeat {
                    chapter_number: 145,
                    title: String::new(),
                    goal: String::new(),
                    key_turn: String::new(),
                    voice_cues: vec![],
                    cast: vec!["汪必成".into()],
                    location: "夜总会".into(),
                    key_terms: vec!["汪必成".into()],
                    must_terms: vec!["汪必成".into()],
                },
            ],
        };
        let generated = vec![
            (144, "我在医院走廊审问护士。".to_string()),
            (145, "汪必成在福摩萨夜总会等我。".to_string()),
        ];
        let coverage = p.coverage(&generated);
        assert_eq!(coverage.misses.len(), 1);
        assert_eq!(coverage.misses[0].0, 144);
        assert!(coverage.line.contains("第144章 ✗"));
        assert!(coverage.line.contains("第145章 ✓"));
    }

    #[test]
    fn gate_block_reports_missing_terms_against_draft() {
        let p = MainlineProjection {
            arc: String::new(),
            beats: vec![MainlineBeat {
                chapter_number: 144,
                title: String::new(),
                goal: "周成去福摩萨夜总会冲撞邵卫江。".into(),
                key_turn: String::new(),
                voice_cues: vec![],
                cast: vec!["邵卫江".into()],
                location: "福摩萨夜总会".into(),
                key_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
                must_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
            }],
        };
        let (block, missing) = p.render_gate_block(144, "我在医院走廊审问护士。").unwrap();
        assert!(block.contains("福摩萨夜总会"));
        assert!(block.contains("邵卫江"));
        assert!(block.contains("未在正文出现"));
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn term_matches_accepts_alias_without_generic_suffix() {
        // 真机生成常把「福摩萨夜总会」写成「福摩萨」、把「大波浪卷女人」
        // 写成「大波浪卷」；不能因此判主线未落地。
        assert!(term_matches("我在福摩萨门口撞见邵卫江", "福摩萨夜总会"));
        assert!(term_matches("大波浪卷脸都白了", "大波浪卷女人"));
        assert!(term_matches("邵卫江就在里头", "邵卫江"));
        assert!(!term_matches("我在医院走廊审问护士", "邵卫江"));
    }

    #[test]
    fn term_matches_accepts_venue_synonyms() {
        // 真机第 146 章：地点「夜总会」未出现，正文写「吧台/门厅/珠帘门」。
        assert!(term_matches(
            "我把珠帘挑开，走进门厅，吧台上摆着两只骰盅",
            "夜总会"
        ));
        assert!(term_matches("病房走廊里护士站还亮着灯", "医院"));
        assert!(term_matches("观里三清殿的香火没断", "道观"));
        assert!(!term_matches("大河村诊室里只剩一盏灯", "夜总会"));
    }

    #[test]
    fn voice_cues_extract_key_dialogue_from_truth() {
        let text = "第148章 看我法宝。陆尘音失笑道：“妖孽，看我法宝！”张宝山道：“你他妈疯了？”";
        let cues = extract_voice_cues(text, &["陆尘音".into(), "张宝山".into()]);
        assert!(
            cues.iter().any(|cue| cue.contains("妖孽，看我法宝")),
            "关键台词应进入 voice_cues: {cues:?}"
        );
        assert!(cues.len() <= 3);
    }

    #[test]
    fn deterministic_fallback_terms_skip_protagonist_and_fragments() {
        let text = "医院走廊里，我拦住护士。福摩萨夜总会灯火通明。邵卫江坐在里头，邵卫江很有面子。";
        let terms = deterministic_key_terms(text, Some("周成"));
        assert!(!terms.iter().any(|t| t == "周成"));
        assert!(!terms.iter().any(|t| t == "我拦"));
    }
}
