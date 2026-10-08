//! 续写导演锁：一人一号、近文亲缘、一拍一事。
//! 设计：docs/plans/2026-08-27-continue-director-lock-design.md

use serde::Deserialize;

const TITLE_TOKENS: &[&str] = &[
    "镇北王",
    // 封号/身份（含古代小说常见称谓：称人不说名时靠这些词识别）
    "亲王",
    "郡王",
    "王爷",
    "国公",
    "侯爷",
    "伯爷",
    "太子",
    "世子",
    "殿下",
    "公主",
    "郡主",
    "王妃",
    "驸马",
    "娘娘",
    "皇后",
    "太后",
    "贵妃",
    "皇上",
    "皇帝",
    "陛下",
    "钦差",
    // 官称
    "大人",
    "将军",
    "都统",
    "大帅",
    "军师",
    "尚书",
    "侍郎",
    "中书令",
    "参军",
    "参事",
    "知县",
    "知府",
    "巡抚",
    // 民间称谓
    "公子",
    "少爷",
    "小姐",
    "老爷",
    "夫人",
    "掌柜",
    "掌门",
    "长老",
    "帮主",
    "教主",
    "神医",
    "太医",
    "道长",
    "方丈",
    "先生",
    "王",
];

const PURE_TITLES: &[&str] = &[
    "亲王", "郡王", "王爷", "国公", "侯爷", "伯爷", "太子", "世子", "殿下", "公主", "郡主", "王妃",
    "驸马", "娘娘", "皇后", "太后", "贵妃", "皇上", "皇帝", "陛下", "钦差", "大人", "将军", "公子",
    "少爷", "小姐", "老爷", "夫人", "王",
];

const KIN_INVERSION_WORDS: &[&str] = &["侄子", "侄女", "姑姑", "叔父"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifeStatus {
    Living,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityLock {
    pub canonical: String,
    pub aliases: Vec<String>,
    pub status: LifeStatus,
    pub kin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectorLock {
    pub identities: Vec<IdentityLock>,
    pub beat_move: String,
    pub forbidden: Vec<String>,
    /// 本拍在场（含死者）两端都在锁内的关系，喂给主创。
    pub relations: Vec<String>,
    /// 被关系不变量（`sanitize_relations`）拦下的原始行：只用于质量债与排查，
    /// 不渲染进提示词。
    pub relations_dropped: Vec<String>,
}

impl DirectorLock {
    pub fn render(&self) -> String {
        if self.identities.is_empty() && self.beat_move.is_empty() {
            return String::new();
        }
        let mut lines = vec!["【本拍人物锁】".to_string()];
        for id in &self.identities {
            let mut names = vec![id.canonical.clone()];
            for a in &id.aliases {
                if a != &id.canonical {
                    names.push(a.clone());
                }
            }
            let joined = names.join("＝");
            let life = match id.status {
                LifeStatus::Dead => "已死",
                LifeStatus::Living => "活人",
            };
            let kin = id.kin.as_deref().unwrap_or("");
            if kin.is_empty() {
                lines.push(format!("{joined}。{life}。禁止拆成两人。"));
            } else {
                lines.push(format!("{joined}。{life}。{kin}。禁止拆成两人。"));
            }
        }
        if !self.beat_move.is_empty() {
            lines.push(format!("【本拍只写】{}", self.beat_move));
        }
        if !self.relations.is_empty() {
            lines.push("【本拍人物关系】".into());
            for r in &self.relations {
                lines.push(r.clone());
            }
            lines.push("言行必须符合上列关系，禁止把父子写成叔侄、把妻子写成姑姑。".into());
        }
        if !self.forbidden.is_empty() {
            lines.push(format!("禁：{}", self.forbidden.join("；")));
        }
        lines.join("\n")
    }

    pub fn canonical_of(&self, name: &str) -> Option<&str> {
        let n = name.trim();
        self.identities.iter().find_map(|id| {
            if id.canonical == n || id.aliases.iter().any(|a| a == n) {
                Some(id.canonical.as_str())
            } else {
                None
            }
        })
    }
}

pub fn prefix_has_title(prefix: &str) -> bool {
    !prefix.is_empty() && TITLE_TOKENS.iter().any(|t| prefix.contains(t))
}

pub fn is_pure_title(name: &str) -> bool {
    PURE_TITLES.iter().any(|t| *t == name)
}

pub fn same_person(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    if short.chars().count() < 2 || is_pure_title(short) {
        return false;
    }
    if !long.ends_with(short) {
        // 称号在前的形态：`景亲王` ≡ `景亲王曹元寿`（中文封号+名，与
        // 「称号+本名」的 `镇北王苏会山`
        // 互为镜像；真机同一个人被拆成两行的主因）
        return same_person_title_first(short, long);
    }
    let prefix_len = long.len() - short.len();
    let prefix = &long[..prefix_len];
    prefix_has_title(prefix)
}

/// 称号在前的形态：`base`（如 `景亲王`）以称号结尾，`longer` 是 `base` +
/// 像人名的后缀 （如 `曹元寿`）。后缀必须是 2–4
/// 字、不含称号词、且不以「之/的/与/和/及」开头——
/// 避免把「明成公主之母」这类短语当成同一个人。
fn same_person_title_first(base: &str, longer: &str) -> bool {
    let Some(suffix) = longer.strip_prefix(base) else {
        return false;
    };
    let n = suffix.chars().count();
    if !(2..=4).contains(&n) {
        return false;
    }
    if suffix.starts_with(['之', '的', '与', '和', '及']) {
        return false;
    }
    if TITLE_TOKENS.iter().any(|t| suffix.contains(t)) {
        return false;
    }
    TITLE_TOKENS.iter().any(|t| base.ends_with(t))
}

#[derive(Debug, Clone)]
pub struct IdentityCluster {
    pub canonical: String,
    pub members: Vec<String>,
    pub aliases: Vec<String>,
}

fn compounds_in_text(name: &str, text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let nchars: Vec<char> = name.chars().collect();
    if nchars.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + nchars.len() <= chars.len() {
        if chars[i..i + nchars.len()] == nchars[..] {
            let max_back = i.min(6);
            for back in (1..=max_back).rev() {
                let prefix: String = chars[i - back..i].iter().collect();
                if prefix_has_title(&prefix) {
                    out.push(format!("{prefix}{name}"));
                    break;
                }
            }
            i += nchars.len();
        } else {
            i += 1;
        }
    }
    out
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (pa, pb) = (find(parent, a), find(parent, b));
    if pa != pb {
        parent[pb] = pa;
    }
}

fn find(parent: &mut [usize], x: usize) -> usize {
    if parent[x] != x {
        parent[x] = find(parent, parent[x]);
    }
    parent[x]
}

pub fn merge_identity_clusters(table_names: &[String], tail: &str) -> Vec<IdentityCluster> {
    let names: Vec<String> = table_names
        .iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return vec![];
    }
    let n = names.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            if same_person(&names[i], &names[j]) {
                union(&mut parent, i, j);
            }
            let glued = format!("{}{}", names[i], names[j]);
            let glued_rev = format!("{}{}", names[j], names[i]);
            if tail.contains(&glued) || tail.contains(&glued_rev) {
                union(&mut parent, i, j);
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut root_at: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let r = find(&mut parent, i);
        if let Some(idx) = root_at[r] {
            groups[idx].push(i);
        } else {
            root_at[r] = Some(groups.len());
            groups.push(vec![i]);
        }
    }
    let mut clusters = Vec::new();
    for g in groups {
        let members: Vec<String> = g.iter().map(|&i| names[i].clone()).collect();
        let mut aliases = members.clone();
        for m in &members {
            for c in compounds_in_text(m, tail) {
                if !aliases.iter().any(|a| a == &c) {
                    aliases.push(c);
                }
            }
        }
        let canonical = pick_canonical(&members, tail);
        aliases.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
        aliases.dedup();
        clusters.push(IdentityCluster {
            canonical,
            members,
            aliases,
        });
    }
    clusters
}

fn pick_canonical(members: &[String], tail: &str) -> String {
    let candidates: Vec<&String> = members.iter().filter(|m| !is_pure_title(m)).collect();
    let pool = if candidates.is_empty() {
        members.iter().collect::<Vec<_>>()
    } else {
        candidates
    };
    pool.iter()
        .max_by(|a, b| {
            let ca = tail.matches(a.as_str()).count();
            let cb = tail.matches(b.as_str()).count();
            ca.cmp(&cb)
                .then_with(|| a.chars().count().cmp(&b.chars().count()))
        })
        .map(|s| (*s).clone())
        .unwrap_or_else(|| members[0].clone())
}

pub fn collapse_to_canonical(names: &[String], clusters: &[IdentityCluster]) -> Vec<String> {
    let mut out = Vec::new();
    for n in names {
        let canon = clusters
            .iter()
            .find(|c| c.members.iter().any(|m| m == n) || c.aliases.iter().any(|a| a == n))
            .map(|c| c.canonical.clone())
            .unwrap_or_else(|| n.clone());
        if !out.iter().any(|x| x == &canon) {
            out.push(canon);
        }
    }
    out
}

fn last_sentence(tail: &str) -> String {
    tail.split(['。', '！', '？', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("")
        .chars()
        .take(40)
        .collect()
}

fn kin_spouse(a: &str, b: &str, tail: &str) -> bool {
    let patterns = [
        format!("{a}与{b}一并坐下"),
        format!("{b}与{a}一并坐下"),
        format!("{a}、{b}夫妇"),
        format!("{b}、{a}夫妇"),
        format!("{a}与{b}夫妇"),
        format!("{b}与{a}夫妇"),
    ];
    patterns.iter().any(|p| tail.contains(p))
}

/// `kin` 里是否明确把 `other` 写成配偶。
///
/// `kin` 由多段用 `；`
/// 拼接，必须**逐段**判断「同段既出现对方姓名、又有配偶信号」。
/// v0.64.1 真机事故：整串 `contains("配偶")` 让一处配偶信号扩散成与全部在场者的
/// 夫妻边——《帝国的烟火》里曹元佩与苏家满门、王府大执事都被写成夫妻，并被落库。
fn kin_pairs_with_spouse(kin: &str, other: &str) -> bool {
    kin.split(['；', ';']).any(|seg| {
        seg.contains(other)
            && (seg.contains("并坐")
                || seg.contains("配偶")
                || seg.contains("夫妻")
                || seg.contains("之妻")
                || seg.contains("之夫"))
    })
}

pub fn compile_director_lock_rust(
    clusters: &[IdentityCluster],
    dead: &[String],
    tail: &str,
    table_rels: &[(String, String, String)],
) -> DirectorLock {
    let dead_canons: Vec<String> = dead
        .iter()
        .map(|d| {
            clusters
                .iter()
                .find(|c| {
                    c.members.iter().any(|m| m == d)
                        || c.aliases.iter().any(|a| a == d)
                        || c.canonical == *d
                })
                .map(|c| c.canonical.clone())
                .unwrap_or_else(|| d.clone())
        })
        .collect();
    let plunge_sent = tail
        .split(['。', '！', '？', '\n'])
        .filter(|s| s.contains("飞身扑上") || s.contains("扑向") || s.contains("扑上"))
        .last()
        .unwrap_or("");
    let plunger = clusters.iter().find_map(|c| {
        if dead_canons.iter().any(|d| d == &c.canonical) {
            return None;
        }
        let hit = std::iter::once(&c.canonical)
            .chain(c.aliases.iter())
            .chain(c.members.iter())
            .any(|n| plunge_sent.contains(n.as_str()));
        if hit {
            Some(c.canonical.clone())
        } else {
            None
        }
    });
    let father_from_word = tail.contains("父亲") || tail.contains("父王") || tail.contains("爹");
    let father_from_plunge_corpse = !plunge_sent.is_empty()
        && plunge_sent.contains("尸体")
        && dead_canons.iter().any(|d| plunge_sent.contains(d.as_str()));
    let father_pair = if father_from_word || father_from_plunge_corpse {
        dead_canons.first().cloned().zip(
            plunger
                .clone()
                .filter(|p| !dead_canons.iter().any(|d| d == p)),
        )
    } else {
        None
    };

    let mut identities = Vec::new();
    for c in clusters {
        let status = if dead_canons.iter().any(|d| d == &c.canonical) {
            LifeStatus::Dead
        } else {
            LifeStatus::Living
        };
        let mut kin_bits = Vec::new();
        if let Some((parent, child)) = &father_pair {
            if c.canonical == *parent {
                kin_bits.push(format!("{child}之父"));
            }
            if c.canonical == *child {
                kin_bits.push(format!("{parent}之子"));
            }
        }
        for other in clusters {
            if other.canonical == c.canonical {
                continue;
            }
            if kin_spouse(&c.canonical, &other.canonical, tail)
                || c.members
                    .iter()
                    .any(|m| other.members.iter().any(|o| kin_spouse(m, o, tail)))
            {
                kin_bits.push(format!("与{}并坐", other.canonical));
            }
        }
        for (src, tgt, ty) in table_rels {
            let src_c = clusters
                .iter()
                .find(|cl| cl.members.iter().any(|m| m == src) || cl.canonical == *src)
                .map(|cl| cl.canonical.as_str())
                .unwrap_or(src.as_str());
            let tgt_c = clusters
                .iter()
                .find(|cl| cl.members.iter().any(|m| m == tgt) || cl.canonical == *tgt)
                .map(|cl| cl.canonical.as_str())
                .unwrap_or(tgt.as_str());
            if src_c != c.canonical && tgt_c != c.canonical {
                continue;
            }
            let dirty_kin = ty.contains("侄") || ty.contains("姑") || ty.contains("叔");
            if dirty_kin && father_pair.is_some() {
                continue;
            }
            if !ty.trim().is_empty() {
                let other = if src_c == c.canonical { tgt_c } else { src_c };
                let line = format!("{other}（表：{ty}）");
                if !kin_bits.iter().any(|k| k.contains(other)) {
                    kin_bits.push(line);
                }
            }
        }
        kin_bits.dedup();
        identities.push(IdentityLock {
            canonical: c.canonical.clone(),
            aliases: c.aliases.clone(),
            status,
            kin: if kin_bits.is_empty() {
                None
            } else {
                Some(kin_bits.join("；"))
            },
        });
    }

    let dead_names: Vec<&str> = identities
        .iter()
        .filter(|i| i.status == LifeStatus::Dead)
        .map(|i| i.canonical.as_str())
        .collect();
    let last = last_sentence(tail);
    let mut beat_move = if dead_names.is_empty() {
        format!("从「{last}」之后写下一拍，禁止点名式每人一段。")
    } else {
        format!(
            "{}已死。从「{last}」之后写下一拍，禁止点名式每人一段。",
            dead_names.join("、")
        )
    };
    if beat_move.chars().count() > 120 {
        beat_move = beat_move.chars().take(120).collect();
    }

    let mut forbidden = vec!["禁止把同一人的两个称呼写成两个身体".to_string()];
    if !dead_names.is_empty() {
        forbidden.push("禁止重演行刺或死亡".into());
    }
    if let Some((parent, child)) = &father_pair {
        forbidden.push(format!("禁止把{child}写成任何人的侄子；{parent}是其父"));
    }

    let raw_relations = appearing_relation_lines(&identities, table_rels, father_pair.as_ref());
    let (relations, relations_dropped) = sanitize_relations(&raw_relations);
    if !relations_dropped.is_empty() {
        log::warn!(
            "continue director: 关系不变量拦下 {} 行：{:?}",
            relations_dropped.len(),
            relations_dropped
        );
    }

    DirectorLock {
        identities,
        beat_move,
        forbidden,
        relations,
        relations_dropped,
    }
}

/// 解析人物锁关系行 `甲 — 乙：类型…`
fn parse_lock_relation(line: &str) -> Option<(String, String, String)> {
    let (left, right) = line.split_once(" — ")?;
    let (right, ty) = right.split_once('：').or_else(|| right.split_once(':'))?;
    let a = left.trim();
    let b = right.trim();
    if a.is_empty() || b.is_empty() || ty.trim().is_empty() {
        return None;
    }
    Some((a.to_string(), b.to_string(), ty.trim().to_string()))
}

/// 关系类型的主体部分（去掉 `（近文并坐）`、`／仇敌`、`。禁止写成叔侄`
/// 等后缀）。
fn rel_head(ty: &str) -> &str {
    ty.split(['。', '（', '(', '／', '/', '|', '｜', '，', ','])
        .next()
        .unwrap_or("")
        .trim()
}

fn rel_is_spouse(ty: &str) -> bool {
    // v0.64.11：先走归一分类（认得出「夫妻（名分）／仇敌」这类复合写法），
    // 关键词判定保留为兜底
    if crate::db::relation_kind::RelationClass::classify(ty, None).spouse {
        return true;
    }
    let h = rel_head(ty);
    h.starts_with("夫妻") || h.contains("配偶") || h.contains("之妻") || h.contains("之夫")
}

fn rel_is_kin(ty: &str) -> bool {
    const KIN_CHARS: &[char] = &[
        '父', '母', '子', '女', '兄', '弟', '姐', '妹', '祖', '孙', '叔', '姑', '侄', '甥', '舅',
        '姨', '媳', '婿', '亲', '家', '伦',
    ];
    let h = rel_head(ty);
    if crate::db::relation_kind::RelationClass::classify(h, None).kin {
        return true;
    }
    !rel_is_spouse(h) && h.chars().any(|c| KIN_CHARS.contains(&c))
}

/// 关系不变量自检：把「不可能同时成立」的关系行挡在提示词之外。
///
/// 1. 同一对人物既有血亲/亲属行又有夫妻行 → 丢弃夫妻行（真机事故形态：
///    父子、兄妹被「一并坐下」这条启发式写成夫妻）；
/// 2. 单人被写成 ≥3 人的配偶 → 丢弃其全部夫妻行（启发式发散的形状；
///    继室这类合法双配偶不触发）。
///
/// 取舍是「宁缺勿错」：表亲通婚这类既像血亲又像夫妻的组合会被一并拦下并记债，
/// 由人复核，而不是把矛盾喂给模型。返回 `(保留行,
/// 丢弃原因)`，丢弃原因只进质量债。
pub fn sanitize_relations(relations: &[String]) -> (Vec<String>, Vec<String>) {
    use std::collections::{BTreeMap, BTreeSet};

    let parsed: Vec<Option<(String, String, String)>> = relations
        .iter()
        .map(|line| parse_lock_relation(line))
        .collect();

    let mut kin_pair: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut spouse_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in parsed.iter().flatten() {
        let (a, b, ty) = p;
        if rel_is_kin(ty) {
            kin_pair
                .entry(pair_key(a, b))
                .or_insert_with(|| rel_head(ty).to_string());
        }
        if rel_is_spouse(ty) {
            spouse_of.entry(a.clone()).or_default().insert(b.clone());
            spouse_of.entry(b.clone()).or_default().insert(a.clone());
        }
    }
    drop_conflicting_spouse_lines(relations, &parsed, &kin_pair, &spouse_of)
}

/// bundle 情感关系行的同规则过滤。
///
/// 同一份脏数据有两条注入路径：人物锁的【本拍人物关系】，以及直接来自关系表的
/// 【角色情感关系】。后者只按「两端都准入」过滤，不吃锁的不变量，因此这里再跑
/// 一遍同样的判定；血亲证据同时取自锁（含近文推导出的父子）与 bundle 自身，
/// 避免只清一半。姓名先归一到规范名（`景亲王曹元寿` 与 `景亲王` 同对）。
pub fn sanitize_bundle_relations(
    lines: &[String],
    lock: &DirectorLock,
) -> (Vec<String>, Vec<String>) {
    use std::collections::{BTreeMap, BTreeSet};

    let canon = |name: &str| -> String {
        lock.canonical_of(name)
            .map(str::to_string)
            .unwrap_or_else(|| name.to_string())
    };
    let parsed: Vec<Option<(String, String, String)>> = lines
        .iter()
        .map(|line| parse_rel_triple(line).map(|(a, b, ty)| (canon(&a), canon(&b), ty)))
        .collect();

    let mut kin_pair: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut spouse_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for l in &lock.relations {
        if let Some((a, b, ty)) = parse_lock_relation(l) {
            if rel_is_kin(&ty) {
                kin_pair
                    .entry(pair_key(&a, &b))
                    .or_insert_with(|| rel_head(&ty).to_string());
            }
        }
    }
    for p in parsed.iter().flatten() {
        let (a, b, ty) = p;
        if rel_is_kin(ty) {
            kin_pair
                .entry(pair_key(a, b))
                .or_insert_with(|| rel_head(ty).to_string());
        }
        if rel_is_spouse(ty) {
            spouse_of.entry(a.clone()).or_default().insert(b.clone());
            spouse_of.entry(b.clone()).or_default().insert(a.clone());
        }
    }
    drop_conflicting_spouse_lines(lines, &parsed, &kin_pair, &spouse_of)
}

/// 两条注入路径共用的判定：血亲对上的夫妻行、以及单人 ≥3 人的夫妻行整批拦下。
fn drop_conflicting_spouse_lines(
    lines: &[String],
    parsed: &[Option<(String, String, String)>],
    kin_pair: &std::collections::BTreeMap<(String, String), String>,
    spouse_of: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
) -> (Vec<String>, Vec<String>) {
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (line, p) in lines.iter().zip(parsed.iter()) {
        let Some((a, b, ty)) = p else {
            kept.push(line.clone());
            continue;
        };
        if !rel_is_spouse(ty) {
            kept.push(line.clone());
            continue;
        }
        if let Some(kin_ty) = kin_pair.get(&pair_key(a, b)) {
            dropped.push(format!("{a} — {b}：夫妻 与同对的「{kin_ty}」冲突"));
            continue;
        }
        let pair_refs: [&String; 2] = [a, b];
        let crowded = pair_refs
            .iter()
            .filter_map(|who| spouse_of.get(*who).map(|set| (who, set.len())))
            .find(|(_, n)| *n >= 3);
        if let Some((who, n)) = crowded {
            dropped.push(format!(
                "{a} — {b}：夫妻 超量（{who} 同时被写成 {n} 人配偶）"
            ));
            continue;
        }
        kept.push(line.clone());
    }
    (kept, dropped)
}

/// 解析 bundle 关系行 `■ 甲 -> 乙：社会关系=同僚 ｜ …`
pub fn parse_rel_triple(line: &str) -> Option<(String, String, String)> {
    let rest = line.trim().trim_start_matches('■').trim();
    let (left, right) = rest.split_once(" -> ")?;
    let src = left.trim().to_string();
    let (tgt_part, after) = right.split_once('：').or_else(|| right.split_once(':'))?;
    let tgt = tgt_part.trim().to_string();
    if src.is_empty() || tgt.is_empty() {
        return None;
    }
    let ty = after
        .split("社会关系=")
        .nth(1)
        .map(|s| {
            s.split(['｜', '|', ' '])
                .next()
                .unwrap_or(s)
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| after.chars().take(12).collect());
    Some((src, tgt, ty))
}

fn pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn in_lock(identities: &[IdentityLock], name: &str) -> Option<String> {
    identities.iter().find_map(|id| {
        if id.canonical == name || id.aliases.iter().any(|a| a == name) {
            Some(id.canonical.clone())
        } else {
            None
        }
    })
}

pub fn appearing_relation_lines(
    identities: &[IdentityLock],
    table_rels: &[(String, String, String)],
    father_pair: Option<&(String, String)>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    if let Some((parent, child)) = father_pair {
        let k = pair_key(parent, child);
        seen.push(k);
        out.push(format!("{parent} — {child}：父子。禁止写成叔侄。"));
    }
    for id in identities {
        let Some(kin) = id.kin.as_deref() else {
            continue;
        };
        for other in identities {
            if other.canonical == id.canonical {
                continue;
            }
            if !kin_pairs_with_spouse(kin, &other.canonical) {
                continue;
            }
            let k = pair_key(&id.canonical, &other.canonical);
            if seen.iter().any(|s| s == &k) {
                continue;
            }
            seen.push(k);
            out.push(format!(
                "{} — {}：夫妻（近文并坐）。禁止写成姑侄。",
                id.canonical, other.canonical
            ));
        }
    }
    for (src, tgt, ty) in table_rels {
        let Some(sc) = in_lock(identities, src) else {
            continue;
        };
        let Some(tc) = in_lock(identities, tgt) else {
            continue;
        };
        if sc == tc {
            continue;
        }
        let dirty = ty.contains("侄") || ty.contains("姑") || ty.contains("叔");
        let k = pair_key(&sc, &tc);
        if seen.iter().any(|s| s == &k) {
            continue;
        }
        seen.push(k);
        if dirty && father_pair.is_some() {
            continue;
        }
        out.push(format!("{sc} — {tc}：{ty}"));
    }
    out
}

/// 近文锁定的亲缘，用于补进 `character_relationships`（缺则建，脏叔侄则改）。
pub fn inferred_kin_edges(lock: &DirectorLock) -> Vec<(String, String, String)> {
    let mut edges = Vec::new();
    for line in &lock.relations {
        if line.contains("父子") {
            if let Some((a, rest)) = line.split_once(" — ") {
                if let Some((b, _)) = rest.split_once('：') {
                    edges.push((a.trim().to_string(), b.trim().to_string(), "父子".into()));
                }
            }
        } else if line.contains("夫妻") {
            if let Some((a, rest)) = line.split_once(" — ") {
                if let Some((b, _)) = rest.split_once('：') {
                    edges.push((a.trim().to_string(), b.trim().to_string(), "夫妻".into()));
                }
            }
        }
    }
    edges
}

/// 近文已锁父子时，表里的叔侄/姑侄不得再喂给主创。
pub fn table_rel_contradicts_lock(src: &str, tgt: &str, ty: &str, lock: &DirectorLock) -> bool {
    let dirty = ty.contains("侄") || ty.contains("姑") || ty.contains("叔");
    if !dirty {
        return false;
    }
    let both = in_lock(&lock.identities, src).is_some() && in_lock(&lock.identities, tgt).is_some();
    if !both {
        return false;
    }
    lock.relations.iter().any(|r| r.contains("父子"))
        || lock.identities.iter().any(|i| {
            i.kin
                .as_deref()
                .is_some_and(|k| k.contains("之父") || k.contains("之子") || k.contains("之女"))
        })
}

pub fn filter_table_rel_lines_for_lock(lines: &[String], lock: &DirectorLock) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            parse_rel_triple(line)
                .map(|(src, tgt, ty)| !table_rel_contradicts_lock(&src, &tgt, &ty, lock))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

#[derive(Deserialize)]
struct DirectorJson {
    identities: Option<Vec<DirectorJsonId>>,
}

#[derive(Deserialize)]
struct DirectorJsonId {
    canonical: String,
    aliases: Option<Vec<String>>,
    kin: Option<String>,
    status: Option<String>,
}

/// 管理档 JSON 只增补别名/亲缘；不得拆簇、不得把死人标成活人。
pub fn merge_director_json(rust: DirectorLock, raw: &str) -> DirectorLock {
    let Ok(parsed) = serde_json::from_str::<DirectorJson>(raw) else {
        return rust;
    };
    let Some(ids) = parsed.identities else {
        return rust;
    };
    let mut out = rust;
    for j in ids {
        let Some(slot) = out
            .identities
            .iter_mut()
            .find(|i| i.canonical == j.canonical)
        else {
            continue;
        };
        if let Some(aliases) = j.aliases {
            for a in aliases {
                let a = a.trim().to_string();
                if a.chars().count() >= 2 && !slot.aliases.iter().any(|x| x == &a) {
                    slot.aliases.push(a);
                }
            }
        }
        if let Some(kin) = j.kin {
            if !kin.trim().is_empty() {
                match &slot.kin {
                    Some(old) if old.contains(kin.trim()) => {}
                    Some(old) => slot.kin = Some(format!("{old}；{}", kin.trim())),
                    None => slot.kin = Some(kin.trim().to_string()),
                }
            }
        }
        if let Some(st) = j.status {
            if st.contains("死") {
                slot.status = LifeStatus::Dead;
            }
            // living 不得覆盖 rust 的 dead
        }
    }
    out
}

fn is_char_boundary_name_start(text: &str, byte_idx: usize) -> bool {
    byte_idx == 0 || text.is_char_boundary(byte_idx)
}

/// 最长别名优先：被更长称呼盖住的短名不算另一次点名。
/// ≥2 个不同称呼各自独立出现 → 拆人（含「琬公主曹元佩抱着曹元佩的衣角」）。
pub fn subject_split_gaps(increment: &str, lock: &DirectorLock) -> Vec<String> {
    let mut gaps = Vec::new();
    for id in &lock.identities {
        let harvested = compounds_in_text(&id.canonical, increment);
        let mut names: Vec<String> = std::iter::once(id.canonical.clone())
            .chain(id.aliases.iter().cloned())
            .chain(harvested)
            .filter(|s| s.chars().count() >= 2)
            .collect();
        names.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
        names.dedup();
        let spans = independent_name_spans(increment, &names);
        let mut distinct = Vec::new();
        for (_, _, n) in &spans {
            if !distinct.iter().any(|x| x == n) {
                distinct.push(n.clone());
            }
        }
        if distinct.len() >= 2 {
            gaps.push(format!("同一人拆成两个身体：{}", id.canonical));
        }
    }
    gaps
}

fn independent_name_spans(increment: &str, names: &[String]) -> Vec<(usize, usize, String)> {
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    for name in names {
        let mut start = 0;
        while let Some(rel) = increment[start..].find(name.as_str()) {
            let at = start + rel;
            if is_char_boundary_name_start(increment, at) {
                let end = at + name.len();
                let covered = spans.iter().any(|(s, e, _)| at >= *s && end <= *e);
                if !covered {
                    spans.push((at, end, name.clone()));
                }
            }
            start = at + name.len().max(1);
            if start >= increment.len() {
                break;
            }
        }
    }
    spans
}

pub fn kin_inversion_gaps(increment: &str, lock: &DirectorLock) -> Vec<String> {
    let parent_child = lock.identities.iter().any(|i| {
        i.kin.as_deref().is_some_and(|k| {
            k.contains("之父") || k.contains("之子") || k.contains("之女") || k.contains("父亲")
        })
    });
    if !parent_child {
        return vec![];
    }
    if KIN_INVERSION_WORDS.iter().any(|w| increment.contains(w)) {
        vec!["亲缘与人物锁相反".into()]
    } else {
        vec![]
    }
}

const DEAD_AGENCY_MARKERS: &[&str] = &[
    "眼睛", "眼神", "目光", "锁定", "审视", "观察", "娇羞", "看着", "冷酷",
];

const CORPSE_WINDOW: &[&str] = &["尸体", "残骸", "头骨", "血雾"];

/// 锁里已死之人不得再当活人行动（眼睛锁定、审视）。点名尸体本身不算。
pub fn dead_acting_gaps(increment: &str, lock: &DirectorLock) -> Vec<String> {
    let mut gaps = Vec::new();
    for id in &lock.identities {
        if id.status != LifeStatus::Dead {
            continue;
        }
        let harvested = compounds_in_text(&id.canonical, increment);
        let mut names: Vec<String> = std::iter::once(id.canonical.clone())
            .chain(id.aliases.iter().cloned())
            .chain(harvested)
            .filter(|s| s.chars().count() >= 2)
            .collect();
        names.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
        names.dedup();
        let spans = independent_name_spans(increment, &names);
        for (_, end, _) in &spans {
            let after: String = increment[*end..].chars().take(80).collect();
            if CORPSE_WINDOW.iter().any(|m| after.contains(m)) {
                continue;
            }
            if DEAD_AGENCY_MARKERS.iter().any(|m| after.contains(m)) {
                gaps.push(format!("{}已死仍在行动", id.canonical));
                break;
            }
        }
    }
    gaps
}

pub fn lock_from_continue_inputs(
    table_names: &[String],
    extra_names: &[String],
    dead: &[String],
    tail: &str,
    table_rels: &[(String, String, String)],
) -> DirectorLock {
    let mut names = table_names.to_vec();
    for n in extra_names.iter().chain(dead.iter()) {
        if !n.trim().is_empty() && !names.iter().any(|x| x == n) {
            names.push(n.clone());
        }
    }
    let clusters = merge_identity_clusters(&names, tail);
    compile_director_lock_rust(&clusters, dead, tail, table_rels)
}

pub fn collapse_names(names: &[String], lock: &DirectorLock) -> Vec<String> {
    let mut out = Vec::new();
    for n in names {
        let canon = lock.canonical_of(n).unwrap_or(n.as_str()).to_string();
        if !out.iter().any(|x| x == &canon) {
            out.push(canon);
        }
    }
    out
}

/// 阵容改规范名；用途改为亲缘或「可沉默」。
pub fn rewrite_card_cast(card: &mut crate::agency::beat_card::SceneBeatCard, lock: &DirectorLock) {
    use crate::agency::beat_card::CastMember;
    let mut seen = Vec::new();
    let mut next = Vec::new();
    for m in &card.cast {
        let name = lock
            .canonical_of(&m.name)
            .unwrap_or(m.name.as_str())
            .to_string();
        if seen.iter().any(|s| s == &name) {
            continue;
        }
        seen.push(name.clone());
        let purpose = lock
            .identities
            .iter()
            .find(|i| i.canonical == name)
            .and_then(|i| i.kin.clone())
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| "可沉默".into());
        next.push(CastMember { name, purpose });
    }
    card.cast = next;
    card.dead = collapse_names(&card.dead, lock);
    card.conflict_move.parties = collapse_names(&card.conflict_move.parties, lock);
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEDDING_TAIL: &str = "\
进到王府大堂，论级别，苏会山虽贵为镇北王，与景亲王同为王爷，但景亲王是皇上弟弟、皇家贵胄，又是御使钦差，苏会山谨遵礼法，仍请景亲王坐了首席主宾位。苏会山与曹元佩一并坐下，就等新人进门。\
明成公主从船上下来。新郎苏亦铁轻掀轿帘。苏亦铁接过红绸带，进入大堂。\
吉时已到。公主左手射出数道红烟，右手短刃深深扎进了苏会山的胸口。苏会山一拳击中公主，将其打得横飞而出，七窍喷血，抽搐几下，登时气绝。\
苏会山整个头脸皮肉猛然鼓胀崩裂开来。苏亦铁从震惊中清醒过来，看到苏会山的尸体惨状，悲愤裂目，飞身扑上。";

    const FAILED_CONTINUE: &str = "\
苏亦铁的动作带着撕裂般的痛楚。他伸出的手几乎要触碰到那破碎的残骸。\
「父亲！」苏亦铁的喉咙里发出了野兽般的嘶吼。\
曹元佩则彻底僵住了，她紧紧抱住自己的手臂。\
琬公主曹元佩则发出了一声微弱的、压抑的呜咽，她颤抖着伸出手。\
琬公主曹元佩则蜷缩在角落。\
曹元佩看着苏亦铁，眼中有对眼前这个被命运推向深渊的侄子的怜惜。";

    /// 真机 v0.56.0：《帝国的烟火》从「飞身扑上」续写。拆人改成抱衣角，
    /// 死人还在用眼睛锁定。
    const REAL_DEVICE_CONTINUE: &str = "\
苏亦铁的动作猛地停滞在半空，他扑倒在苏会山冰冷的尸体旁，胸腔剧烈起伏。\
景亲王那藏在人群后的身影眼神深沉地扫过现场。\
礼仪主持见状，立刻上前一步，试图稳定局面。\
苏福贵则迅速地从人群中抽身而出。\
琬公主曹元佩，原本因为惊吓而僵立在原地，此刻也紧紧抱着曹元佩的衣角，脸色苍白。\
而明成公主，她被击飞在地，身体的僵硬并未消退，只是那双眼睛，却如同淬了冰的刀锋，死死地锁定在苏亦铁身上。\
她没有发出任何声音，只是静静地看着这一切的发生。\
苏亦铁没有理会景亲王的呵斥，目光最终落在了明成公主身上。他看到了她眼中那份更加深沉的冷酷。";

    #[test]
    fn same_person_title_plus_given_name() {
        assert!(same_person("琬公主曹元佩", "曹元佩"));
        assert!(same_person("镇北王苏会山", "苏会山"));
    }

    #[test]
    fn same_person_does_not_merge_unrelated() {
        assert!(!same_person("明成公主", "曹元佩"));
    }

    /// 称号在前的形态：`景亲王` ≡ `景亲王曹元寿`（真机同一人被拆成两行）。
    /// 后缀必须像人名：短语（明成公主之母）与含称号词的后缀都不算同一人。
    #[test]
    fn same_person_title_first_form() {
        assert!(same_person("景亲王", "景亲王曹元寿"));
        assert!(same_person("苏世子", "苏世子苏亦铁"));
        assert!(!same_person("明成公主", "明成公主之母"));
        assert!(!same_person("苏会山", "苏会山之父"));
        assert!(!same_person("景亲王", "景亲王钦差"));
        assert!(!same_person("苏亦铁", "苏亦铁某人某"));
    }

    #[test]
    fn same_person_does_not_merge_name_without_title_prefix() {
        assert!(!same_person("苏亦铁", "亦铁"));
    }

    #[test]
    fn two_table_rows_admit_one_canonical() {
        let table = vec!["琬公主".into(), "曹元佩".into(), "苏亦铁".into()];
        let tail = format!("{WEDDING_TAIL}琬公主曹元佩随苏会山并坐。");
        let clusters = merge_identity_clusters(&table, &tail);
        let admitted = collapse_to_canonical(&["琬公主".into(), "曹元佩".into()], &clusters);
        assert_eq!(admitted, vec!["曹元佩".to_string()]);
        let cao = clusters.iter().find(|c| c.canonical == "曹元佩").unwrap();
        assert!(
            cao.aliases.iter().any(|a| a.contains("琬公主"))
                || cao.members.iter().any(|m| m == "琬公主"),
            "aliases={:?}",
            cao.aliases
        );
    }

    #[test]
    fn rust_lock_spouse_from_sat_together() {
        let table = vec![
            "苏会山".into(),
            "曹元佩".into(),
            "苏亦铁".into(),
            "明成公主".into(),
        ];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(
            &clusters,
            &["苏会山".into(), "明成公主".into()],
            WEDDING_TAIL,
            &[],
        );
        let cao = lock
            .identities
            .iter()
            .find(|i| i.canonical == "曹元佩")
            .unwrap();
        let kin = cao.kin.as_deref().unwrap_or("");
        assert!(
            kin.contains("苏会山")
                && (kin.contains("并坐") || kin.contains("妻") || kin.contains("配偶")),
            "kin={kin}"
        );
    }

    #[test]
    fn rust_lock_father_from_plunge() {
        let table = vec!["苏会山".into(), "曹元佩".into(), "苏亦铁".into()];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(&clusters, &["苏会山".into()], WEDDING_TAIL, &[]);
        let son = lock
            .identities
            .iter()
            .find(|i| i.canonical == "苏亦铁")
            .unwrap();
        assert!(
            son.kin.as_deref().unwrap_or("").contains("苏会山")
                && son.kin.as_deref().unwrap_or("").contains("子"),
            "kin={:?}",
            son.kin
        );
        let dad = lock
            .identities
            .iter()
            .find(|i| i.canonical == "苏会山")
            .unwrap();
        assert_eq!(dad.status, LifeStatus::Dead);
        assert!(lock.beat_move.contains("苏会山已死") || lock.beat_move.contains("已死"));
        assert!(lock.beat_move.contains("禁止点名式每人一段"));
        let rendered = lock.render();
        assert!(rendered.contains("【本拍人物关系】"), "render={rendered}");
        assert!(
            lock.relations.iter().any(|r| r.contains("父子")),
            "relations={:?}",
            lock.relations
        );
        assert!(
            lock.relations
                .iter()
                .any(|r| r.contains("夫妻") || r.contains("并坐")),
            "relations={:?}",
            lock.relations
        );
        let edges = inferred_kin_edges(&lock);
        assert!(
            edges.iter().any(|(a, b, t)| t == "父子"
                && ((a == "苏会山" && b == "苏亦铁") || (a == "苏亦铁" && b == "苏会山"))),
            "edges={edges:?}"
        );
        assert!(edges.iter().any(|(_, _, t)| t == "夫妻"), "edges={edges:?}");
    }

    #[test]
    fn appearing_rels_drop_nephew_when_prose_says_father() {
        let table = vec!["苏会山".into(), "曹元佩".into(), "苏亦铁".into()];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(
            &clusters,
            &["苏会山".into()],
            WEDDING_TAIL,
            &[("曹元佩".into(), "苏亦铁".into(), "侄子".into())],
        );
        assert!(
            !lock.relations.iter().any(|r| r.contains("侄子")),
            "relations={:?}",
            lock.relations
        );
        assert!(lock.relations.iter().any(|r| r.contains("父子")));
        let cao = lock
            .identities
            .iter()
            .find(|i| i.canonical == "曹元佩")
            .unwrap();
        assert!(
            !cao.kin.as_deref().unwrap_or("").contains("侄子"),
            "kin={:?}",
            cao.kin
        );
        let raw = "■ 曹元佩 -> 苏亦铁：社会关系=侄子 ｜ 情感=怜[0.5]";
        assert!(table_rel_contradicts_lock(
            "曹元佩",
            "苏亦铁",
            "侄子",
            &lock
        ));
        let kept = filter_table_rel_lines_for_lock(&[raw.into()], &lock);
        assert!(kept.is_empty(), "kept={kept:?}");
    }

    #[test]
    fn appearing_rels_keep_table_edge_when_both_present() {
        let ids = vec![
            IdentityLock {
                canonical: "景亲王".into(),
                aliases: vec![],
                status: LifeStatus::Living,
                kin: None,
            },
            IdentityLock {
                canonical: "苏会山".into(),
                aliases: vec![],
                status: LifeStatus::Dead,
                kin: None,
            },
        ];
        let lines = appearing_relation_lines(
            &ids,
            &[("景亲王".into(), "苏会山".into(), "钦差与藩王".into())],
            None,
        );
        assert!(
            lines
                .iter()
                .any(|r| r.contains("景亲王") && r.contains("苏会山") && r.contains("钦差")),
            "{lines:?}"
        );
    }

    #[test]
    fn probe_rejects_cao_split_bodies() {
        let table = vec!["曹元佩".into(), "琬公主".into(), "苏亦铁".into()];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(&clusters, &["苏会山".into()], WEDDING_TAIL, &[]);
        let gaps = subject_split_gaps(FAILED_CONTINUE, &lock);
        assert!(
            gaps.iter()
                .any(|g| g.contains("曹元佩") && g.contains("拆成两个")),
            "gaps={gaps:?}"
        );
    }

    #[test]
    fn probe_rejects_hugging_own_clothes_as_two_people() {
        let table = vec![
            "曹元佩".into(),
            "琬公主".into(),
            "苏亦铁".into(),
            "明成公主".into(),
            "苏会山".into(),
        ];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(
            &clusters,
            &["苏会山".into(), "明成公主".into()],
            WEDDING_TAIL,
            &[],
        );
        let gaps = subject_split_gaps(REAL_DEVICE_CONTINUE, &lock);
        assert!(
            gaps.iter()
                .any(|g| g.contains("曹元佩") && g.contains("拆成两个")),
            "gaps={gaps:?}"
        );
    }

    #[test]
    fn probe_rejects_dead_princess_living_gaze() {
        let table = vec![
            "曹元佩".into(),
            "苏亦铁".into(),
            "明成公主".into(),
            "苏会山".into(),
        ];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(
            &clusters,
            &["苏会山".into(), "明成公主".into()],
            WEDDING_TAIL,
            &[],
        );
        let gaps = dead_acting_gaps(REAL_DEVICE_CONTINUE, &lock);
        assert!(
            gaps.iter()
                .any(|g| g.contains("明成公主") && g.contains("已死")),
            "gaps={gaps:?} status={:?}",
            lock.identities
                .iter()
                .map(|i| (&i.canonical, &i.status))
                .collect::<Vec<_>>()
        );
        assert!(
            !gaps.iter().any(|g| g.contains("苏会山")),
            "尸体旁点名苏会山不应算活人行动 gaps={gaps:?}"
        );
    }

    #[test]
    fn probe_rejects_nephew_when_lock_is_father() {
        let table = vec!["苏会山".into(), "曹元佩".into(), "苏亦铁".into()];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(&clusters, &["苏会山".into()], WEDDING_TAIL, &[]);
        let gaps = kin_inversion_gaps(FAILED_CONTINUE, &lock);
        assert!(
            gaps.iter().any(|g| g.contains("亲缘")),
            "gaps={gaps:?} lock_kin={:?}",
            lock.identities.iter().map(|i| &i.kin).collect::<Vec<_>>()
        );
    }

    #[test]
    fn merge_json_cannot_revive_dead() {
        let rust = DirectorLock {
            identities: vec![IdentityLock {
                canonical: "苏会山".into(),
                aliases: vec!["镇北王".into()],
                status: LifeStatus::Dead,
                kin: None,
            }],
            beat_move: "写后果".into(),
            forbidden: vec![],
            relations: vec![],
            relations_dropped: vec![],
        };
        let merged = merge_director_json(
            rust,
            r#"{"identities":[{"canonical":"苏会山","status":"living","aliases":["镇北王苏会山"]}]}"#,
        );
        assert_eq!(merged.identities[0].status, LifeStatus::Dead);
        assert!(merged.identities[0]
            .aliases
            .iter()
            .any(|a| a == "镇北王苏会山"));
    }

    #[test]
    fn merge_json_ignores_unknown_canonical() {
        let rust = DirectorLock {
            identities: vec![IdentityLock {
                canonical: "曹元佩".into(),
                aliases: vec![],
                status: LifeStatus::Living,
                kin: None,
            }],
            ..DirectorLock::default()
        };
        let merged = merge_director_json(
            rust,
            r#"{"identities":[{"canonical":"金敏秀","kin":"路人"}]}"#,
        );
        assert_eq!(merged.identities.len(), 1);
        assert_eq!(merged.identities[0].canonical, "曹元佩");
    }

    /// v0.64.1 真机事故：《帝国的烟火》一处「苏会山与曹元佩一并坐下」把
    /// 曹元佩与全部在场者写成夫妻，随提示词喂给主创并落库。
    #[test]
    fn spouse_inference_stays_on_the_named_pair() {
        let table = vec![
            "苏会山".into(),
            "曹元佩".into(),
            "苏亦铁".into(),
            "苏福贵".into(),
            "苏宜茹".into(),
            "江顾然".into(),
        ];
        let clusters = merge_identity_clusters(&table, WEDDING_TAIL);
        let lock = compile_director_lock_rust(&clusters, &["苏会山".into()], WEDDING_TAIL, &[]);
        let spouses: Vec<String> = lock
            .relations
            .iter()
            .filter(|r| r.contains("夫妻"))
            .cloned()
            .collect();
        assert_eq!(spouses.len(), 1, "只认「一并坐下」点名的两人：{spouses:?}");
        assert!(spouses[0].contains("苏会山") && spouses[0].contains("曹元佩"));
        for other in ["苏亦铁", "苏福贵", "苏宜茹", "江顾然"] {
            assert!(
                !lock
                    .relations
                    .iter()
                    .any(|r| r.contains("夫妻") && r.contains(other)),
                "{other} 不得被写成夫妻 relations={:?}",
                lock.relations
            );
        }
        assert!(lock.relations_dropped.is_empty(), "正常输入不该有丢弃");
        // 内部标记不得进入任何渲染面
        assert!(!lock.render().contains("配偶向"), "{}", lock.render());
        assert!(lock
            .identities
            .iter()
            .filter_map(|i| i.kin.as_deref())
            .all(|k| !k.contains("配偶向")));
    }

    #[test]
    fn sanitize_drops_spouse_on_kin_pair() {
        let rels = vec![
            "苏会山 — 苏亦铁：父子。禁止写成叔侄。".to_string(),
            "苏会山 — 苏亦铁：夫妻（近文并坐）。禁止写成姑侄。".to_string(),
            "苏会山 — 曹元佩：夫妻（近文并坐）。禁止写成姑侄。".to_string(),
            "苏亦铁 — 江顾然：同门好友".to_string(),
        ];
        let (kept, dropped) = sanitize_relations(&rels);
        assert!(
            kept.iter().any(|l| l.contains("父子")),
            "父子行须保留 {kept:?}"
        );
        assert!(
            kept.iter()
                .any(|l| l.contains("曹元佩") && l.contains("夫妻")),
            "无冲突的夫妻行须保留 {kept:?}"
        );
        assert!(kept.iter().any(|l| l.contains("同门好友")));
        assert_eq!(dropped.len(), 1, "dropped={dropped:?}");
        assert!(dropped[0].contains("苏亦铁") && dropped[0].contains("父子"));
    }

    #[test]
    fn sanitize_drops_person_with_three_spouses() {
        let rels = vec![
            "曹元佩 — 苏会山：夫妻（近文并坐）。".to_string(),
            "曹元佩 — 苏福贵：夫妻（近文并坐）。".to_string(),
            "曹元佩 — 苏宜茹：夫妻（近文并坐）。".to_string(),
        ];
        let (kept, dropped) = sanitize_relations(&rels);
        assert!(kept.is_empty(), "超量夫妻须整批拦下 kept={kept:?}");
        assert_eq!(dropped.len(), 3, "dropped={dropped:?}");
    }

    #[test]
    fn sanitize_keeps_two_spouses_and_non_kin_lines() {
        let rels = vec![
            // 继室/续弦这类合法的双配偶不得被误伤
            "苏会山 — 曹元佩：夫妻".to_string(),
            "苏会山 — 元配王氏：夫妻".to_string(),
            "景亲王 — 苏会山：政敌/清算者".to_string(),
            "江顾然 — 裴立言：师徒".to_string(),
            "苏福贵 — 苏会山：上下级".to_string(),
        ];
        let (kept, dropped) = sanitize_relations(&rels);
        assert_eq!(kept.len(), rels.len(), "dropped={dropped:?}");
        assert!(dropped.is_empty());
    }

    /// 关系表那条注入路径（【角色情感关系】）同样要过不变量。
    #[test]
    fn bundle_relations_drop_poisoned_spouse_rows() {
        let lock = DirectorLock {
            identities: vec![IdentityLock {
                canonical: "曹元佩".into(),
                aliases: vec!["琬公主曹元佩".into()],
                status: LifeStatus::Living,
                kin: None,
            }],
            relations: vec!["苏会山 — 苏亦铁：父子。禁止写成叔侄。".into()],
            ..DirectorLock::default()
        };
        let lines = vec![
            // 父子对上的夫妻行：与锁里的父子冲突
            "■ 苏会山 -> 苏亦铁：社会关系=夫妻 ｜ 情感=父亲的期许与严教[0.8]".to_string(),
            // 一人三配偶：真机事故里曹元佩与苏家满门成夫妻的形状
            "■ 曹元佩 -> 苏会山：社会关系=夫妻 ｜ 情感=政治联姻[0.6]".to_string(),
            "■ 曹元佩 -> 苏福贵：社会关系=夫妻 ｜ 情感=近文锁定[0.5]".to_string(),
            "■ 曹元佩 -> 苏宜茹：社会关系=夫妻 ｜ 情感=近文锁定[0.5]".to_string(),
            // 正常行必须原样保留
            "■ 苏亦铁 -> 江顾然：社会关系=同门好友 ｜ 情感=同窗情谊[0.7]".to_string(),
        ];
        let (kept, dropped) = sanitize_bundle_relations(&lines, &lock);
        assert!(
            kept.iter().any(|l| l.contains("同门好友")),
            "正常行须保留 kept={kept:?}"
        );
        assert!(!kept.iter().any(|l| l.contains("夫妻")), "kept={kept:?}");
        assert_eq!(dropped.len(), 4, "dropped={dropped:?}");
    }

    /// 别名归一到规范名：`景亲王曹元寿 -> 曹元佩=家人` 能挡住 `景亲王 —
    /// 曹元佩：夫妻`。
    #[test]
    fn bundle_relations_canonicalize_alias_pairs() {
        let lock = DirectorLock {
            identities: vec![IdentityLock {
                canonical: "景亲王".into(),
                aliases: vec!["景亲王曹元寿".into()],
                status: LifeStatus::Living,
                kin: None,
            }],
            ..DirectorLock::default()
        };
        let lines = vec![
            "■ 景亲王曹元寿 -> 曹元佩：社会关系=家人 ｜ 情感=同胞手足之情[0.8]".to_string(),
            "■ 景亲王 -> 曹元佩：社会关系=夫妻 ｜ 情感=同胞亲情[0.85]".to_string(),
        ];
        let (kept, dropped) = sanitize_bundle_relations(&lines, &lock);
        assert!(kept.iter().any(|l| l.contains("家人")), "kept={kept:?}");
        assert!(!kept.iter().any(|l| l.contains("夫妻")), "kept={kept:?}");
        assert_eq!(dropped.len(), 1, "dropped={dropped:?}");
    }
}
