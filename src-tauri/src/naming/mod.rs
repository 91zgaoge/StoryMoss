//! 三层九维姓名编译法的 P0 校验核心。
//!
//! P0 先做「能拦什么」：重名、同用字、同姓扎堆、长度单一、模型陈词组合。
//! 候选池/完整账本/称呼图在 P1 接入；这里不增加 LLM 调用。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub mod repository;

pub use repository::{insert_in_tx, NameLedgerRecord, NameLedgerRepository};

/// P0 回滚开关：`STORYMOSS_NAMING=0` 时关闭姓名校验。
pub fn enabled() -> bool {
    std::env::var("STORYMOSS_NAMING")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

/// 姓名问题严重度：Hard 会阻止落库，Soft 只告警/提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSeverity {
    Hard,
    Soft,
}

/// 姓名问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameIssue {
    pub severity: NameSeverity,
    pub code: &'static str,
    pub message: String,
}

impl NameIssue {
    fn hard(code: &'static str, message: String) -> Self {
        Self {
            severity: NameSeverity::Hard,
            code,
            message,
        }
    }

    fn soft(code: &'static str, message: String) -> Self {
        Self {
            severity: NameSeverity::Soft,
            code,
            message,
        }
    }
}

/// 姓名账本条目（P1 完整落库用；P0 只做内存校验）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterName {
    pub character_id: String,
    pub formal: String,
    pub surname: String,
    pub generation: Option<char>,
    pub given: String,
    pub courtesy: Option<String>,
    pub art: Option<String>,
    pub childhood: Option<String>,
    pub nickname: Option<String>,
    pub aliases: Vec<String>,
    pub titles: Vec<String>,
    pub address_rules: Vec<AddressRule>,
    pub name_story: String,
}

/// 一句称呼规则：谁在什么场合怎么叫谁。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressRule {
    pub speaker: String,
    pub addressee: String,
    pub public_form: String,
    pub private_form: String,
    pub hostile_form: Option<String>,
    pub note: String,
}

/// 取名候选项（P1 作者编辑界面/取名编译器用）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameCandidate {
    pub formal: String,
    pub surname: String,
    pub given: String,
    pub domain: String,
    pub note: String,
}

const GIVEN_POOL: &[(&str, &str, &str)] = &[
    ("秀兰", "期望/日常", "1960s 常见的女性名，普通家庭"),
    ("建国", "时代", "1970s 男性名，时代感强"),
    ("雨桐", "纪念", "1990s 后常见，出生雨季"),
    ("子默", "经典", "偏书面，适合读书人家庭"),
    ("守拙", "经典", "字辈/书香家庭，藏锋"),
    ("阿禾", "日常/方言", "乡村小名，亲近"),
    ("小满", "节气/纪念", "节气名，乡野"),
    ("念慈", "纪念", "纪念长辈，女性偏多"),
    ("承志", "家族", "承接字辈与家族期待"),
    ("明远", "期望", "普通期望名，男女可用"),
    ("静秋", "时代", "年代感女性名"),
    ("志强", "期望", "1980s 男性常见名"),
    ("招娣", "日常/方言", "旧时代女性名，带时代伤痕"),
    ("石头", "日常", "乡土男性名，质朴"),
    ("若兰", "经典", "书香家庭女性名"),
];

/// 为给定本名生成候选（保留姓氏，换用不同语义域的给定名）。
/// 只用于作者选名/候选池；不直接改已有角色名。
pub fn suggest_candidates(formal: &str, count: usize) -> Vec<NameCandidate> {
    let surname = surname_of(formal);
    if surname.is_empty() {
        return Vec::new();
    }
    let existing = formal.trim();
    let mut out = Vec::new();
    for (given, domain, note) in GIVEN_POOL {
        let candidate = format!("{surname}{given}");
        if candidate == existing || is_model_cliche(&candidate) {
            continue;
        }
        out.push(NameCandidate {
            formal: candidate,
            surname: surname.clone(),
            given: given.to_string(),
            domain: domain.to_string(),
            note: note.to_string(),
        });
        if out.len() >= count.max(1) {
            break;
        }
    }
    out
}

/// 主要角色姓名校验。返回空表示通过；Hard 问题需阻止落库。
pub fn validate_names(names: &[&str]) -> Vec<NameIssue> {
    let mut issues = Vec::new();
    let cleaned: Vec<&str> = names
        .iter()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
        .collect();
    if cleaned.is_empty() {
        return issues;
    }

    // 1) 完全重名
    let mut seen = HashSet::new();
    for name in &cleaned {
        if !seen.insert(*name) {
            issues.push(NameIssue::hard(
                "duplicate_name",
                format!("姓名「{name}」在同一组角色里重复"),
            ));
        }
    }

    // 2) 同姓扎堆（非家族场景；P0 不区分家族）
    let mut surname_counts: HashMap<String, usize> = HashMap::new();
    for name in &cleaned {
        let surname = surname_of(name);
        *surname_counts.entry(surname).or_insert(0) += 1;
    }
    for (surname, count) in surname_counts {
        if count > 2 {
            issues.push(NameIssue::soft(
                "surname_overuse",
                format!("姓氏「{surname}」出现 {count} 次，若不是同族请拉开"),
            ));
        }
    }

    // 3) 主要角色共用给定名用字
    let mut given_counts: HashMap<char, usize> = HashMap::new();
    for name in &cleaned {
        for ch in given_chars(name) {
            *given_counts.entry(ch).or_insert(0) += 1;
        }
    }
    for (ch, count) in given_counts {
        if count > 1 {
            issues.push(NameIssue::hard(
                "duplicate_given_char",
                format!("给定名用字「{ch}」被 {count} 个角色共用"),
            ));
        }
    }

    // 4) 长度分布塌缩：全组都是二字全名，或全组同一个长度
    let lengths: Vec<usize> = cleaned.iter().map(|n| n.chars().count()).collect();
    let two_char = lengths.iter().filter(|&&n| n == 2).count();
    let ratio = two_char as f64 / cleaned.len() as f64;
    if ratio >= 0.8 {
        issues.push(NameIssue::soft(
            "two_char_collapse",
            format!(
                "{} 个角色里 {:.0}% 是二字全名，分布过于单一",
                cleaned.len(),
                ratio * 100.0
            ),
        ));
    }
    if lengths.iter().all(|n| *n == lengths[0]) {
        issues.push(NameIssue::soft(
            "uniform_length",
            format!("全部姓名都是 {} 字，缺少长度参差", lengths[0]),
        ));
    }

    // 5) 模型陈词组合：小说姓 + 冷色/夜色/墨色用字 + 二字全名
    for name in &cleaned {
        if is_model_cliche(name) {
            issues.push(NameIssue::soft(
                "model_cliche",
                format!("「{name}」是典型的模型姓名组合（二字 + 小说姓 + 冷色用字）"),
            ));
        }
    }

    issues
}

/// 是否有 Hard 问题。
pub fn has_hard_issue(names: &[&str]) -> bool {
    validate_names(names)
        .iter()
        .any(|i| i.severity == NameSeverity::Hard)
}

fn surname_of(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() >= 4 {
        // 复姓（欧阳/上官/诸葛/慕容…）粗判为前两字
        let first2: String = chars[..2].iter().collect();
        if COMPOUND_SURNAMES.contains(&first2.as_str()) {
            return first2;
        }
    }
    chars.first().map(|c| c.to_string()).unwrap_or_default()
}

fn given_chars(name: &str) -> Vec<char> {
    let surname = surname_of(name);
    let chars: Vec<char> = name.chars().collect();
    let surname_len = surname.chars().count();
    if surname_len >= chars.len() {
        return vec![];
    }
    chars[surname_len..].to_vec()
}

/// 二字全名 + 小说姓 + 冷色/夜色/墨色用字的组合，正是 DeepSeek/Qwen 的
/// 默认塌缩形态。不是按单字封杀（雪/月/夜本身是正常字），而是按组合扣分。
fn is_model_cliche(name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() != 2 {
        return false;
    }
    let surname = chars[0].to_string();
    let given = chars[1];
    NOVEL_SURNAME_CHARS.contains(&surname.as_str()) && CLICHE_GIVEN_CHARS.contains(&given)
}

const COMPOUND_SURNAMES: &[&str] = &[
    "欧阳", "上官", "诸葛", "慕容", "司徒", "司马", "尉迟", "长孙", "宇文", "皇甫", "令狐", "独孤",
    "南宫", "西门", "东方", "夏侯", "公孙", "端木", "呼延", "赫连",
];

/// 模型/网文最常见的「小说姓」小池。出现本身不是错，和二字冷色名组合才是。
const NOVEL_SURNAME_CHARS: &[&str] = &[
    "林", "苏", "顾", "沈", "陆", "江", "秦", "楚", "叶", "白", "温", "宋", "裴", "谢", "萧", "容",
    "傅", "晏", "黎", "霍", "厉", "靳", "岑", "闻", "祝", "阮", "卫", "尤", "连", "时", "席", "商",
];

/// 模型最爱的单字用字池。只参与「二字全名组合」扣分，不按单字封杀。
const CLICHE_GIVEN_CHARS: &[char] = &[
    '雪', '夜', '墨', '寒', '云', '月', '辰', '宇', '轩', '然', '若', '离', '歌', '汐', '璃', '殇',
    '陌', '泽', '川', '野', '风', '霜', '灵', '儿', '染', '玄', '尘', '影', '瑶', '琳', '萱', '涵',
    '梓', '曦', '澈', '彦', '煜', '宸', '洛', '熙', '瑾', '瑜', '辞', '归', '舟', '渡', '晏', '知',
    '念', '初', '南', '北', '故', '里', '昭', '景', '行', '止', '安', '宁', '远', '深', '微', '浅',
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cliche_combo_is_flagged() {
        let names = ["林雪", "沈夜", "苏墨", "顾寒", "叶云", "秦月"];
        let issues = validate_names(&names);
        assert!(issues.iter().any(|i| i.code == "model_cliche"));
        assert!(issues.iter().any(|i| i.code == "two_char_collapse"));
    }

    #[test]
    fn duplicate_given_char_is_hard() {
        let issues = validate_names(&["林雪", "沈雪"]);
        assert!(issues
            .iter()
            .any(|i| i.code == "duplicate_given_char" && i.severity == NameSeverity::Hard));
    }

    #[test]
    fn realistic_mix_passes() {
        let names = ["王秀兰", "李建国", "张伟", "陈雨桐", "周子默"];
        let issues = validate_names(&names);
        assert!(
            !issues.iter().any(|i| i.code == "model_cliche"),
            "issues={issues:?}"
        );
        assert!(!issues.iter().any(|i| i.severity == NameSeverity::Hard));
    }

    #[test]
    fn common_surnames_are_not_banned() {
        let issues = validate_names(&["王秀兰", "李建国", "张伟"]);
        assert!(!issues.iter().any(|i| i.code == "common_surname_banned"));
    }

    #[test]
    fn candidate_pool_keeps_surname_and_varies_given() {
        let candidates = suggest_candidates("沈砚", 3);
        assert_eq!(candidates.len(), 3);
        for c in &candidates {
            assert!(c.formal.starts_with("沈"));
            assert_ne!(c.formal, "沈砚");
            assert!(!c.given.is_empty());
        }
    }

    #[test]
    fn candidate_pool_keeps_compound_surname() {
        let candidates = suggest_candidates("欧阳修远", 2);
        assert_eq!(candidates.len(), 2);
        for c in &candidates {
            assert!(c.formal.starts_with("欧阳"));
            assert_ne!(c.formal, "欧阳修远");
        }
    }
}
