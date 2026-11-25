//! v0.99.15：文风基线 / 逐章漂移 / AI 腔探针 / 原文语体样本。
//!
//! 真机复盘（2026-11-24 两轮 10 章训练）：`style_dna_applied=true` 且
//! 上一轮 style 均分 6.8，未触发「文风锚点」，writer 只看到 StyleDNA 的
//! 六维指标（句长/词类清单）。10 章批量里每章只喂上一章生成的正文，风格
//! 逐章向冷硬 noir 漂移，原著的碎嘴自嘲与市井诙谐被稀释到 151–152 章
//! 几乎消失。指标文字无法复刻语感，必须在**每一章**都给原文语体样本，
//! 并把「句长/对白占比/第一人称/口语词/比喻」落成可测的基线。
//!
//! 本模块不做 LLM 调用：确定性统计 + 原文段落抽样，供训练执行器注入
//! writer/修订 prompt，并在报告中输出漂移证据。

use serde::{Deserialize, Serialize};

/// 语体画像（1–10 分维度之外的确定性基线）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleProfile {
    pub chars: usize,
    pub sentences: usize,
    pub avg_sentence_len: f64,
    pub short_ratio: f64,
    pub dialogue_ratio: f64,
    pub first_person_per_k: f64,
    pub colloquial_per_k: f64,
    pub metaphor_per_k: f64,
}

pub const COLLOQUIAL_MARKERS: &[&str] = &[
    "呗",
    "啥",
    "咋",
    "甭",
    "咱",
    "俺",
    "老子",
    "爷们",
    "哎",
    "哟",
    "嘿",
    "嘛",
    "咯",
    "喽",
    "嘞",
    "特么",
    "他娘的",
    "狗日的",
    "玩意儿",
    "家伙",
    "折腾",
    "瞎",
    "扯",
    "整",
    "闹",
    "攒",
    "揍",
    "混",
    "赔",
    "赚",
    "坑",
    "骗",
    "事儿",
    "得嘞",
    "成嘞",
    "好嘞",
];

pub const METAPHOR_MARKERS: &[&str] = &["像", "仿佛", "好似", "犹如", "似的", "般"];

/// AI 腔高频指纹（真机编审反复点名的项）。
pub const AI_ISM_MARKERS: &[(&str, &str)] = &[
    ("后槽牙", "后槽牙"),
    ("喉头", "喉头"),
    ("头皮", "头皮"),
    ("脊背", "脊背"),
    ("发麻", "身体反应"),
    ("发凉", "身体反应"),
    ("酥麻", "身体反应"),
    ("钻心", "身体反应"),
    ("一紧", "身体反应"),
    ("不由得", "书面转折"),
    ("下意识", "书面转折"),
    ("仿佛", "比喻"),
    ("似乎", "书面转折"),
];

pub fn profile(plain: &str) -> StyleProfile {
    let text = plain.trim();
    let chars = text.chars().count();
    if chars == 0 {
        return StyleProfile::default();
    }
    let sentences = split_sentences(text);
    let sentence_count = sentences.len().max(1);
    let sentence_chars: usize = sentences
        .iter()
        .map(|s| s.chars().filter(|c| is_chinese_or_digit(*c)).count())
        .sum();
    let avg_sentence_len = sentence_chars as f64 / sentence_count as f64;
    let short = sentences
        .iter()
        .filter(|s| s.chars().filter(|c| is_chinese_or_digit(*c)).count() <= 8)
        .count();
    let dialogue_chars = dialogue_span_chars(text);
    let first_person = count_markers(text, &["我", "咱", "俺"]);
    let colloquial = count_markers(text, COLLOQUIAL_MARKERS);
    let metaphor = count_markers(text, METAPHOR_MARKERS);
    StyleProfile {
        chars,
        sentences: sentence_count,
        avg_sentence_len,
        short_ratio: short as f64 / sentence_count as f64,
        dialogue_ratio: dialogue_chars as f64 / chars as f64,
        first_person_per_k: first_person as f64 / chars as f64 * 1000.0,
        colloquial_per_k: colloquial as f64 / chars as f64 * 1000.0,
        metaphor_per_k: metaphor as f64 / chars as f64 * 1000.0,
    }
}

fn is_chinese_or_digit(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c) || c.is_ascii_digit()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if matches!(c, '。' | '！' | '？' | '；' | '\n' | '\r') {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            current.clear();
        } else {
            current.push(c);
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

fn count_markers(text: &str, markers: &[&str]) -> usize {
    markers.iter().map(|m| text.matches(m).count()).sum()
}

fn dialogue_span_chars(text: &str) -> usize {
    let pairs = [('“', '”'), ('「', '」'), ('『', '』')];
    let chars: Vec<char> = text.chars().collect();
    let mut total = 0usize;
    for (open, close) in pairs {
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != open {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != close {
                j += 1;
            }
            if j > i + 1 {
                total += j - i - 1;
            }
            i = j + 1;
        }
    }
    total
}

/// 报告用基线一行。
pub fn baseline_line(p: &StyleProfile) -> String {
    if p.chars == 0 {
        return "原文语体基线：不可用（上下文为空）".to_string();
    }
    format!(
        "原文语体基线：平均句长 {:.1} 字；对白占比 {:.0}%；第一人称 {:.1}/千字；口语词 {:.1}/千字；比喻 {:.1}/千字；短句占比 {:.0}%。",
        p.avg_sentence_len,
        p.dialogue_ratio * 100.0,
        p.first_person_per_k,
        p.colloquial_per_k,
        p.metaphor_per_k,
        p.short_ratio * 100.0
    )
}

/// 逐章漂移探针：只列偏离基线 ±25% 的指标；全在阈值内时返回一句「正常」。
pub fn drift_report(baseline: &StyleProfile, generated: &[(i32, String)]) -> String {
    if baseline.chars == 0 {
        return "文风漂移探针：无基线（上下文为空）".to_string();
    }
    let mut flagged: Vec<String> = Vec::new();
    for (number, text) in generated {
        if text.trim().is_empty() {
            continue;
        }
        let p = profile(text);
        let mut items: Vec<String> = Vec::new();
        if deviation(p.avg_sentence_len, baseline.avg_sentence_len, 0.25) {
            items.push(format!(
                "句长 {:.1}（基线 {:.1}）",
                p.avg_sentence_len, baseline.avg_sentence_len
            ));
        }
        let dialogue_delta = p.dialogue_ratio - baseline.dialogue_ratio;
        if dialogue_delta.abs() > 0.12 {
            items.push(format!(
                "对白 {:.0}%（基线 {:.0}%）",
                p.dialogue_ratio * 100.0,
                baseline.dialogue_ratio * 100.0
            ));
        }
        if deviation(p.colloquial_per_k, baseline.colloquial_per_k, 0.30) {
            items.push(format!(
                "口语词 {:.1}/千字（基线 {:.1}）",
                p.colloquial_per_k, baseline.colloquial_per_k
            ));
        }
        if deviation(p.first_person_per_k, baseline.first_person_per_k, 0.30) {
            items.push(format!(
                "第一人称 {:.1}/千字（基线 {:.1}）",
                p.first_person_per_k, baseline.first_person_per_k
            ));
        }
        if p.metaphor_per_k > baseline.metaphor_per_k * 2.0 && p.metaphor_per_k >= 1.0 {
            items.push(format!(
                "比喻密度 {:.1}/千字（基线 {:.1}）",
                p.metaphor_per_k, baseline.metaphor_per_k
            ));
        }
        if !items.is_empty() {
            flagged.push(format!("第{number}章 {}", items.join("、")));
        }
    }
    if flagged.is_empty() {
        "文风漂移探针：各章句长/对白/第一人称/口语词/比喻均在基线 ±25% 内".to_string()
    } else {
        crate::agency::continue_assets::truncate_chars(
            &format!("文风漂移探针：{}", flagged.join("｜")),
            520,
        )
    }
}

fn deviation(value: f64, baseline: f64, ratio: f64) -> bool {
    if baseline <= f64::EPSILON {
        return value > f64::EPSILON;
    }
    ((value - baseline) / baseline).abs() > ratio
}

/// 严重文风漂移 → gate 强制修订问题。
///
/// 真机 2026-11-24 报告：144–153 章句长 9.5–13.3（原文 19.2）、口语词
/// 1.1–1.4/千字（原文 3.3）；149 章 editor 直接判 style 5「冷硬 noir」。
/// 只有 editor 分数触发不够（多数章 style 7 仍通过），这里用确定性指标
/// 兜底：句长偏短 + 口语词不足（或比喻密度超基线 2.5 倍）时，即使 editor
/// 放行也要求修订一次。
pub fn severe_drift_issue(baseline: &StyleProfile, plain: &str) -> Option<String> {
    if baseline.chars == 0 || plain.trim().is_empty() {
        return None;
    }
    let p = profile(plain);
    let mut flags: Vec<String> = Vec::new();
    let short =
        baseline.avg_sentence_len > 0.0 && p.avg_sentence_len < baseline.avg_sentence_len * 0.65;
    let colloquial =
        baseline.colloquial_per_k > 0.5 && p.colloquial_per_k < baseline.colloquial_per_k * 0.6;
    let low_dialogue = p.dialogue_ratio + 0.20 < baseline.dialogue_ratio;
    let metaphor = baseline.metaphor_per_k > 0.3
        && p.metaphor_per_k > baseline.metaphor_per_k * 2.5
        && p.metaphor_per_k > 1.5;
    if short {
        flags.push(format!(
            "平均句长 {:.1} 字（原文 {:.1}）",
            p.avg_sentence_len, baseline.avg_sentence_len
        ));
    }
    if colloquial {
        flags.push(format!(
            "口语词 {:.1}/千字（原文 {:.1}）",
            p.colloquial_per_k, baseline.colloquial_per_k
        ));
    }
    if low_dialogue {
        flags.push(format!(
            "对白占比 {:.0}%（原文 {:.0}%）",
            p.dialogue_ratio * 100.0,
            baseline.dialogue_ratio * 100.0
        ));
    }
    if metaphor {
        flags.push(format!(
            "比喻密度 {:.1}/千字（原文 {:.1}）",
            p.metaphor_per_k, baseline.metaphor_per_k
        ));
    }
    if flags.len() < 2 && !metaphor {
        return None;
    }
    Some(format!(
        "文风严重偏离原文基线：{}。必须按【原文语体样本与基线】恢复原文句长节奏、市井口语与自嘲，禁止冷硬 noir 化与短句堆叠。",
        flags.join("；")
    ))
}

/// AI 腔探针：统计高频指纹，指出重复最多的三项。
pub fn ai_ism_report(generated: &[(i32, String)]) -> String {
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    let mut body_reaction = 0usize;
    let mut not_but = 0usize;
    let mut metaphor = 0usize;
    for (_, text) in generated {
        if text.trim().is_empty() {
            continue;
        }
        for (marker, label) in AI_ISM_MARKERS {
            let n = text.matches(marker).count();
            if n > 0 {
                *counts.entry(*label).or_insert(0) += n;
            }
            if *label == "身体反应" {
                body_reaction += n;
            }
            if *label == "比喻" {
                metaphor += n;
            }
        }
        not_but += count_not_but(text);
    }
    let profile_all = profile(
        &generated
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut parts: Vec<String> = Vec::new();
    if let Some((label, n)) = counts
        .iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| **n >= 3)
    {
        parts.push(format!("{label}×{n}"));
    }
    if body_reaction >= 3 {
        parts.push(format!("身体反应×{body_reaction}"));
    }
    if not_but >= 2 {
        parts.push(format!("不是…而是×{not_but}"));
    }
    if metaphor >= 3 {
        parts.push(format!(
            "比喻×{metaphor}（{:.1}/千字）",
            profile_all.metaphor_per_k
        ));
    }
    if parts.is_empty() {
        "AI 腔探针：未检出高频 AI 腔".to_string()
    } else {
        format!("AI 腔探针：{}", parts.join("｜"))
    }
}

fn count_not_but(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut count = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '不' && i + 1 < chars.len() && chars[i + 1] == '是' {
            let mut j = i + 2;
            while j < chars.len() && j < i + 46 {
                if chars[j] == '而' && j + 1 < chars.len() && chars[j + 1] == '是' {
                    count += 1;
                    i = j + 2;
                    break;
                }
                j += 1;
            }
            if j >= chars.len() || j >= i + 46 {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    count
}

/// 原文语体样本：从上下文章抽 3 段（compact 2 段），优先对白密集、第一人称
/// 碎嘴、口语词多的段落；每段 ≤420 字。返回空串表示无法抽取。
pub fn build_exemplar(context: &[(usize, String)], compact: bool) -> String {
    let baseline = profile(
        &context
            .iter()
            .map(|(_, content)| crate::export::content::content_to_plain_text(content))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut voice: std::collections::BTreeMap<usize, (String, i32)> =
        std::collections::BTreeMap::new();
    let mut narrative: std::collections::BTreeMap<usize, (String, i32)> =
        std::collections::BTreeMap::new();
    for (number, content) in context {
        let plain = crate::utils::text::TextUtils::strip_leading_chapter_heading(
            &crate::export::content::content_to_plain_text(content),
        );
        for passage in candidate_passages(&plain) {
            let voice_score = passage_score(&passage);
            let entry = voice
                .entry(*number)
                .or_insert((passage.clone(), voice_score));
            if voice_score > entry.1 {
                *entry = (passage.clone(), voice_score);
            }
            let n_score = narrative_score(&passage, &baseline);
            if n_score > 0 {
                let entry = narrative
                    .entry(*number)
                    .or_insert((passage.clone(), n_score));
                if n_score > entry.1 {
                    *entry = (passage, n_score);
                }
            }
        }
    }
    if voice.is_empty() && narrative.is_empty() {
        return String::new();
    }
    let mut chapters: Vec<usize> = voice
        .keys()
        .chain(narrative.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    chapters.sort_unstable();
    let picks = if compact || chapters.len() < 3 {
        if chapters.len() >= 2 {
            vec![chapters[0], chapters[chapters.len() - 1]]
        } else {
            chapters
        }
    } else {
        vec![
            chapters[0],
            chapters[chapters.len() / 2],
            chapters[chapters.len() - 1],
        ]
    };
    let mut picked: Vec<(&str, usize, String)> = Vec::new();
    for number in &picks {
        if let Some((passage, _)) = voice.get(number) {
            picked.push(("口语", *number, passage.clone()));
        }
    }
    // compact 只留 1 段口语，把第 2 个名额留给叙述样本；否则截断会把
    // 叙述样本挤掉，短句问题仍在。
    if compact && picked.len() > 1 {
        picked.truncate(1);
    }
    // 叙述样本只取 1 段（compact 也 1 段），展示原文长句与连接词节奏；
    // 口语样本不能替代叙述节奏，否则模型只学到短句对话，句长持续偏短。
    if let Some(number) = picks
        .iter()
        .rev()
        .find(|number| narrative.contains_key(number))
        .copied()
    {
        if let Some((passage, _)) = narrative.get(&number) {
            picked.push(("叙述", number, passage.clone()));
        }
    }
    if picked.len() > if compact { 2 } else { 4 } {
        picked.truncate(if compact { 2 } else { 4 });
    }
    let cap = if compact { 900 } else { 1800 };
    let mut out = String::new();
    out.push_str(&format!(
        "【原文语体样本与基线（逐章对照；模仿其句长、口语颗粒、第一人称碎嘴、市井物象与自嘲；禁止改成冷硬侦探腔/书面总结腔）】\n{}\n",
        baseline_line(&baseline)
    ));
    out.push_str(
        "写作硬约束：本章句长/对白/第一人称/口语词/比喻应落在原文基线 ±25% 内；\
         每章至少一处主角自嘲或市井调侃；禁止整章冷硬侦探腔、格言式旁白与书面总结腔；\
         禁止用「后槽牙/喉头一紧/发麻/发凉/酥麻/钻心」等身体反应替代动作与对白；\
         「不是……而是……」每章最多一次；比喻每 500 字最多一处。\
         叙述样本用于校准长句与连接词节奏，口语样本用于校准市井对白；两类都要模仿，不能只写短句。\n",
    );
    for (kind, number, passage) in picked.iter() {
        let passage: String = passage.chars().take(420).collect();
        out.push_str(&format!("[{kind}样本·第{number}章] {passage}\n"));
    }
    crate::agency::continue_assets::truncate_chars(&out, cap)
}

/// 叙述样本评分：优先「平均句长不低于原文基线、对白占比低」的段落。
/// 真机生成句长 9.5–13.3（原文 19.2），只抽口语/对白样本会继续强化短句。
fn narrative_score(passage: &str, baseline: &StyleProfile) -> i32 {
    let p = profile(passage);
    if p.sentences < 2 || p.dialogue_ratio > 0.25 {
        return 0;
    }
    let mut score = 0i32;
    if baseline.avg_sentence_len > 0.0 && p.avg_sentence_len >= baseline.avg_sentence_len {
        score += 4;
    } else if baseline.avg_sentence_len > 0.0
        && p.avg_sentence_len >= baseline.avg_sentence_len * 0.85
    {
        score += 2;
    }
    if p.chars >= 150 {
        score += 1;
    }
    score
}

fn candidate_passages(plain: &str) -> Vec<String> {
    let sentences = split_sentences(plain);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut count = 0usize;
    for sentence in sentences {
        let s = sentence.trim();
        if s.chars().count() < 4 {
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(s);
        count += 1;
        let len = current.chars().count();
        if len >= 120 && count >= 2 {
            out.push(current.clone());
            current.clear();
            count = 0;
        }
        if len >= 420 {
            current.clear();
            count = 0;
        }
    }
    if !current.trim().is_empty() && current.chars().count() >= 80 {
        out.push(current);
    }
    out
}

fn passage_score(passage: &str) -> i32 {
    let mut score = 0i32;
    if passage.contains('“') || passage.contains('「') || passage.contains('『') {
        score += 3;
    }
    let colloquial = count_markers(passage, COLLOQUIAL_MARKERS);
    if colloquial >= 3 {
        score += 3;
    } else if colloquial >= 1 {
        score += 1;
    }
    let first_person = passage.matches('我').count();
    if first_person >= 3 {
        score += 2;
    }
    const INTERIOR: [&str; 8] = [
        "我心里",
        "我寻思",
        "我琢磨",
        "我合计",
        "我想着",
        "我暗",
        "我骂",
        "我盘算",
    ];
    if INTERIOR.iter().any(|m| passage.contains(m)) {
        score += 2;
    }
    let len = passage.chars().count();
    if (120..=360).contains(&len) {
        score += 1;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_detects_dialogue_and_colloquial() {
        let text = "我说：“咋整？甭跟我扯。”我心里琢磨，这事儿得整明白。";
        let p = profile(text);
        assert!(p.dialogue_ratio > 0.2);
        assert!(p.colloquial_per_k > 0.0);
        assert!(p.first_person_per_k > 0.0);
    }

    #[test]
    fn drift_report_flags_shorter_sentences_and_missing_dialogue() {
        let baseline = profile(
            "我 说 道 ： “ 你 这 是 干 啥 ？ 咱 们 得 整 明 白 。 ” 我 心 里 琢 磨 着 ， 这 事 儿 不 能 瞎 扯 。",
        );
        let generated = vec![(
            144,
            "风从巷口灌进来。他没有回头。灯灭了。门关上。远处传来脚步。".to_string(),
        )];
        let line = drift_report(&baseline, &generated);
        assert!(line.contains("第144章"));
        assert!(line.contains("对白") || line.contains("句长"));
    }

    #[test]
    fn severe_drift_issue_flags_short_sentences_and_low_colloquial() {
        let baseline = profile(
            &"我蹲在门槛上，叼着半根烟，心里琢磨着这买卖咋整，嘴上跟老曹扯两句啥，甭管值不值当。"
                .repeat(4),
        );
        let draft = "他推门。门轴响了。灯灭了。风进来。街上没人。";
        let issue = severe_drift_issue(&baseline, draft).expect("应判严重漂移");
        assert!(issue.contains("句长") || issue.contains("口语词"));
        assert!(issue.contains("原文语体样本"));
        let normal =
            "我蹲在门槛上，叼着半根烟，看着院里那棵老槐树的影子一点一点挪到墙根，心里琢磨着这买卖咋整，嘴上还得跟老曹扯两句啥，甭管值不值当。";
        assert!(severe_drift_issue(&baseline, normal).is_none());
    }

    #[test]
    fn compact_exemplar_keeps_both_voice_and_narrative_samples() {
        let context = vec![
            (
                134,
                "我蹲在门槛上，叼着半根烟：“咋整？甭跟老子扯那些没用的。”我心里琢磨，这事儿得整个明白。".repeat(4),
            ),
            (
                135,
                "我蹲在门槛上，看着院里那棵老槐树的影子一点一点挪到墙根，心里琢磨着这趟买卖到底值不值当，嘴上还得跟老曹扯两句闲篇。".repeat(4),
            ),
        ];
        let exemplar = build_exemplar(&context, true);
        assert!(exemplar.contains("口语样本"), "{exemplar}");
        assert!(
            exemplar.contains("叙述样本"),
            "compact 必须保留叙述样本: {exemplar}"
        );
    }

    #[test]
    fn ai_ism_report_counts_not_but_and_body_reactions() {
        let generated = vec![(
            144,
            "不是他不想，而是他不能。后槽牙咬了一下，喉头一紧，头皮发麻，脊背发凉。不是风，而是雨。".to_string(),
        )];
        let line = ai_ism_report(&generated);
        assert!(line.contains("不是…而是"));
        assert!(line.contains("身体反应") || line.contains("后槽牙"));
    }

    #[test]
    fn exemplar_prefers_colloquial_dialogue_passages() {
        let context = vec![
            (
                134,
                "我走进院子。风很大。院墙很旧。屋檐上落着灰。".repeat(4),
            ),
            (
                135,
                "我蹲在门槛上，叼着半根烟：“咋整？甭跟老子扯那些没用的。”我心里琢磨，这事儿得整个明白。".repeat(3),
            ),
            (
                136,
                "夜里有雨。灯很暗。街上没人。".repeat(4),
            ),
        ];
        let exemplar = build_exemplar(&context, false);
        assert!(exemplar.contains("原文语体基线"));
        assert!(exemplar.contains("咋整") || exemplar.contains("甭跟"));
        assert!(exemplar.contains("写作硬约束"));
    }
}
