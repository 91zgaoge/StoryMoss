//! SceneBeatCard：本拍硬任务纯 Rust 编译（0 LLM）。
//! 设计：docs/plans/2026-08-13-agency-only-continuation-design.md §5

use crate::{
    creative_engine::expansion::{debt::QuotaItem, ExpansionDebt, RotationLedger},
    db::{
        repositories::{
            CharacterRelationshipRepository, CharacterRepository, StoryOutlineRepository,
            StoryRepository,
        },
        Character, DbPool, SceneRepository,
    },
    error::AppError,
};

#[derive(Debug, Clone)]
pub struct CastMember {
    pub name: String,
    pub purpose: String,
}

#[derive(Debug, Clone)]
pub struct ConflictMove {
    pub action: String,
    pub parties: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EmotionBeat {
    pub summary: String,
}

/// 本拍必须改变的戏剧项（AI-drama-pound：每场至少改信息/关系/目标/风险/
/// 情绪之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Information,
    Relationship,
    Goal,
    Risk,
    Emotion,
}

impl ChangeKind {
    pub fn as_zh(self) -> &'static str {
        match self {
            Self::Information => "信息",
            Self::Relationship => "关系",
            Self::Goal => "目标",
            Self::Risk => "风险",
            Self::Emotion => "情绪",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDelta {
    pub kind: ChangeKind,
    pub summary: String,
}

impl Default for ChangeDelta {
    fn default() -> Self {
        Self {
            kind: ChangeKind::Information,
            summary: String::new(),
        }
    }
}

pub const CURRENT_SCENE_OUTLINE_MARK: &str = "【当前场大纲】";

#[derive(Debug, Clone)]
pub struct SceneBeatCard {
    pub cast: Vec<CastMember>,
    pub conflict_move: ConflictMove,
    pub emotion_beat: EmotionBeat,
    pub next_outline_node: String,
    pub expansion_quota: Vec<QuotaItem>,
    pub expansion_quota_text: Option<String>,
    pub setting_location: Option<String>,
    pub open_review_issues: Vec<String>,
    /// 近文已写成尸体/气绝的人。禁止再当行动主体，禁止重演其死亡。
    pub dead: Vec<String>,
    pub change_delta: ChangeDelta,
}

pub fn quota_text_for_beats(
    debt: &crate::creative_engine::expansion::ExpansionDebt,
) -> Option<String> {
    debt.quota_text().map(|s| {
        s.replace(
            "【本章扩张任务（硬性要求，必须落实）】",
            "【本拍扩张任务（硬性要求，必须落实）】",
        )
        .replace(" 章——本章", " 拍——本拍")
        .replace(" 章无更新——本章", " 拍无更新——本拍")
        .replace(" 章无动静——本章", " 拍无动静——本拍")
    })
}

impl SceneBeatCard {
    pub fn render_full(&self) -> String {
        let mut lines = vec!["【本章节拍任务】".to_string()];
        let cast = self
            .cast
            .iter()
            .map(|c| format!("{}（{}）", c.name, c.purpose))
            .collect::<Vec<_>>()
            .join("、");
        lines.push(format!(
            "阵容：{}",
            if cast.is_empty() {
                "待定".into()
            } else {
                cast
            }
        ));
        lines.push(format!("冲突：{}", self.conflict_move.action));
        lines.push(format!("情感：{}", self.emotion_beat.summary));
        lines.push(format!("推进：{}", self.next_outline_node));
        if !self.change_delta.summary.trim().is_empty() {
            lines.push(format!(
                "必须改变：{} — {}",
                self.change_delta.kind.as_zh(),
                self.change_delta.summary
            ));
        }
        if !self.dead.is_empty() {
            lines.push(format!(
                "已死（禁止再行动、禁止重演其死亡）：{}",
                self.dead.join("、")
            ));
        }
        if let Some(ref loc) = self.setting_location {
            if !loc.is_empty() {
                lines.push(format!("地点：{}", loc));
            }
        }
        if let Some(ref q) = self.expansion_quota_text {
            if !q.is_empty() {
                lines.push(q.clone());
            }
        }
        if !self.open_review_issues.is_empty() {
            lines.push("【待兑现审查】".into());
            for (i, issue) in self.open_review_issues.iter().take(2).enumerate() {
                lines.push(format!("{}. {}", i + 1, issue));
            }
        }
        lines.join("\n")
    }

    /// Append 写入 `scenes.outline_content` 的结构化当前场大纲（0 LLM）。
    pub fn render_scene_outline(&self) -> String {
        let mut lines = vec![CURRENT_SCENE_OUTLINE_MARK.to_string()];
        let cast = self
            .cast
            .iter()
            .map(|c| c.name.as_str())
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>()
            .join("、");
        lines.push(format!(
            "在场：{}",
            if cast.is_empty() {
                "待定".into()
            } else {
                cast
            }
        ));
        lines.push(format!("冲突：{}", self.conflict_move.action));
        lines.push(format!("情感：{}", self.emotion_beat.summary));
        // v0.64.9：通用兜底句（方法论模板）不落库——落了会被下一拍当计划采纳，
        // 章节方向永远停在模板上（真机逐章重演同一场面）。
        if !self.next_outline_node.trim().is_empty()
            && !crate::agency::prose_ground::is_generic_next_node(&self.next_outline_node)
        {
            lines.push(format!("下一拍：{}", self.next_outline_node));
        }
        if let Some(ref loc) = self.setting_location {
            if !loc.is_empty() {
                lines.push(format!("地点：{}", loc));
            }
        }
        lines.join("\n")
    }

    pub fn render_tail_summary(&self) -> String {
        format!(
            "【节拍摘要】上场：{}｜冲突：{}｜情感：{}｜推进：{}",
            self.cast
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join("、"),
            self.conflict_move
                .action
                .chars()
                .take(40)
                .collect::<String>(),
            self.emotion_beat
                .summary
                .chars()
                .take(40)
                .collect::<String>(),
            self.next_outline_node.chars().take(40).collect::<String>(),
        )
    }
}

/// 从 DB + 当前章末段编译本拍硬任务卡。缺槽用降级句，永不空卡。
pub fn compile_beat_card(
    pool: &DbPool,
    story_id: &str,
    current_content: &str,
) -> Result<SceneBeatCard, AppError> {
    compile_beat_card_located(pool, story_id, current_content, None)
}

pub fn compile_beat_card_located(
    pool: &DbPool,
    story_id: &str,
    current_content: &str,
    current_scene_location: Option<&str>,
) -> Result<SceneBeatCard, AppError> {
    let chars = CharacterRepository::new(pool.clone())
        .get_by_story(story_id)
        .map_err(AppError::from)?;
    let protagonist = chars.first().map(|c| c.name.as_str()).unwrap_or("主角");

    let tail = crate::agency::continue_assets::prior_tail_for_cast(current_content);
    let table_names: Vec<String> = chars.iter().map(|c| c.name.clone()).collect();
    let mut dead = crate::agency::continue_assets::dead_names_in_text(&table_names, &tail);
    // v0.64.7：近文窗口（章末 1500 字）滑过的死亡由持久化生死状态兜底——
    // 真机第 2 章写死的明成公主在第 10 章被当成活人上场，就是窗口丢的。
    for name in crate::story_system::life_status::dead_names(pool, story_id) {
        if !dead.iter().any(|d| d == &name) {
            dead.push(name);
        }
    }
    let mut cast: Vec<CastMember> = present_in_text(&chars, &tail)
        .into_iter()
        .filter(|c| !dead.iter().any(|d| d == &c.name))
        .collect();
    let ledger = RotationLedger::load_sync(pool, story_id).unwrap_or_default();
    let debt = ExpansionDebt::compute(pool, story_id, &ledger).unwrap_or_default();
    let expansion_quota = debt.triggered();
    let allow_character_move = expansion_quota.contains(&QuotaItem::CharacterMove);

    if chars.len() >= 3 && allow_character_move {
        if let Some(silent) = ledger
            .character_silence
            .iter()
            .find(|s| !cast.iter().any(|c| c.name == s.name) && !dead.iter().any(|d| d == &s.name))
        {
            cast.push(CastMember {
                name: silent.name.clone(),
                purpose: "沉寂回归，推进其弧光".into(),
            });
        }
    }
    let tensions = crate::agency::emotional_ledger::load_tensions(pool, story_id);
    if let Some(t) = tensions.iter().max_by(|a, b| {
        a.pressure
            .partial_cmp(&b.pressure)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        let src_in = !t.source_name.is_empty() && cast.iter().any(|c| c.name == t.source_name);
        let tgt_in = !t.target_name.is_empty() && cast.iter().any(|c| c.name == t.target_name);
        if src_in ^ tgt_in {
            let name = if src_in {
                &t.target_name
            } else {
                &t.source_name
            };
            if !name.is_empty()
                && !cast.iter().any(|c| c.name == *name)
                && !dead.iter().any(|d| d == name)
            {
                cast.push(CastMember {
                    name: name.clone(),
                    purpose: format!("张力对手（{}）", t.tension_type),
                });
            }
        }
    }
    if cast.is_empty() {
        let living = table_names
            .iter()
            .find(|n| !dead.iter().any(|d| d == *n))
            .map(|s| s.as_str())
            .unwrap_or(protagonist);
        cast.push(CastMember {
            name: living.to_string(),
            purpose: "本拍行动主体".into(),
        });
    }
    cast.truncate(8);
    let conflict_move = compile_conflict(&chars, &cast, pool, story_id, protagonist);
    let emotion_beat = compile_emotion(&chars, &cast, pool, story_id, protagonist);
    let next_outline_node = compile_next_node(pool, story_id, current_content);
    let change_delta = compile_change_delta(&conflict_move, &next_outline_node, &emotion_beat);
    let expansion_quota_text = quota_text_for_beats(&debt);
    let setting_location = current_scene_location
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let open_review_issues = crate::agency::continue_loop::load_open_review_issues(pool, story_id);

    Ok(SceneBeatCard {
        cast,
        conflict_move,
        emotion_beat,
        next_outline_node,
        expansion_quota,
        expansion_quota_text,
        setting_location,
        open_review_issues,
        dead,
        change_delta,
    })
}

fn present_in_text(chars: &[Character], text: &str) -> Vec<CastMember> {
    let names: Vec<String> = chars.iter().map(|c| c.name.clone()).collect();
    crate::agency::continue_assets::match_character_names(&names, text)
        .into_iter()
        .map(|name| CastMember {
            name,
            purpose: "可沉默".into(),
        })
        .collect()
}

pub(crate) fn compile_conflict(
    chars: &[Character],
    cast: &[CastMember],
    pool: &DbPool,
    story_id: &str,
    protagonist: &str,
) -> ConflictMove {
    let cast_names: Vec<&str> = cast.iter().map(|c| c.name.as_str()).collect();
    let rels = CharacterRelationshipRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    const HOSTILE: &[&str] = &[
        "仇", "敌", "对立", "背叛", "欺骗", "复仇", "恨", "enemy", "rival", "conflict",
    ];
    for r in &rels {
        let ty = r.relationship_type.to_lowercase();
        let bond = r.emotional_bond.as_deref().unwrap_or("");
        let hostile = HOSTILE.iter().any(|k| ty.contains(k) || bond.contains(k));
        if !hostile {
            continue;
        }
        let src = chars
            .iter()
            .find(|c| c.id == r.source_character_id)
            .map(|c| c.name.as_str())
            .unwrap_or(protagonist);
        let tgt = r.target_character_name.as_deref().unwrap_or("对方");
        if !cast_names.contains(&src) || !cast_names.contains(&tgt) {
            continue;
        }
        return ConflictMove {
            action: format!("加压：{src} 与 {tgt} 正面对峙，赌注未解，不得只靠对话过渡。"),
            parties: vec![src.to_string(), tgt.to_string()],
        };
    }
    let parties: Vec<String> = cast.iter().take(2).map(|c| c.name.clone()).collect();
    let names = if parties.is_empty() {
        protagonist.to_string()
    } else {
        parties.join("、")
    };
    ConflictMove {
        action: format!("{names} 必须在本拍与阻力正面对峙，不得只靠对话过渡。"),
        parties,
    }
}

pub(crate) fn compile_change_delta(
    conflict: &ConflictMove,
    next_node: &str,
    emotion: &EmotionBeat,
) -> ChangeDelta {
    let action = conflict.action.as_str();
    if action.contains("加压") || action.contains("对峙") || action.contains("赌注") {
        let kind = if conflict.parties.len() >= 2 || action.contains("对峙") {
            ChangeKind::Risk
        } else {
            ChangeKind::Relationship
        };
        return ChangeDelta {
            kind,
            summary: conflict.action.clone(),
        };
    }
    let node = next_node.trim();
    if !node.is_empty() {
        return ChangeDelta {
            kind: ChangeKind::Information,
            summary: node.to_string(),
        };
    }
    let emo = emotion.summary.trim();
    if !emo.is_empty() {
        return ChangeDelta {
            kind: ChangeKind::Emotion,
            summary: emo.to_string(),
        };
    }
    ChangeDelta::default()
}

fn compile_emotion(
    chars: &[Character],
    cast: &[CastMember],
    pool: &DbPool,
    story_id: &str,
    protagonist: &str,
) -> EmotionBeat {
    let cast_names: Vec<&str> = cast.iter().map(|c| c.name.as_str()).collect();
    for c in chars {
        if !cast_names.contains(&c.name.as_str()) && !cast_names.is_empty() {
            continue;
        }
        let wound = c.emotional_wound.as_deref().unwrap_or("");
        let need = c.emotional_need.as_deref().unwrap_or("");
        let core = c.emotional_core.as_deref().unwrap_or("");
        if wound.is_empty() && need.is_empty() && core.is_empty() {
            continue;
        }
        let mut bits = vec![];
        if !core.is_empty() {
            bits.push(format!("内核={}", core));
        }
        if !wound.is_empty() {
            bits.push(format!("伤口={}", wound));
        }
        if !need.is_empty() {
            bits.push(format!("需求={}", need));
        }
        return EmotionBeat {
            summary: format!("{}：{}", c.name, bits.join("，")),
        };
    }
    let rels = CharacterRelationshipRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    if let Some(r) = rels.iter().find(|r| {
        let a = r.emotional_bond.as_deref().unwrap_or("");
        let b = r.reverse_emotional_bond.as_deref().unwrap_or("");
        !a.is_empty() && !b.is_empty() && a != b
    }) {
        let src = chars
            .iter()
            .find(|c| c.id == r.source_character_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| protagonist.to_string());
        let tgt = r
            .target_character_name
            .clone()
            .unwrap_or_else(|| "对方".into());
        return EmotionBeat {
            summary: format!(
                "{}→{}：{} vs {}→{}：{}",
                src,
                tgt,
                r.emotional_bond.as_deref().unwrap_or("未明"),
                tgt,
                src,
                r.reverse_emotional_bond.as_deref().unwrap_or("未明"),
            ),
        };
    }
    EmotionBeat {
        summary: format!("本拍必须让 {protagonist} 的需求受阻并露出情绪代价。"),
    }
}

pub fn next_node_from_scene_outline(outline: &str) -> Option<String> {
    if !outline.contains(CURRENT_SCENE_OUTLINE_MARK) {
        return None;
    }
    for line in outline.lines() {
        if let Some(rest) = line.trim().strip_prefix("下一拍：") {
            let n = rest.trim();
            // v0.64.9：通用兜底句不是计划——它没有本故事的方向，采信它等于
            // 让每一拍都停在同一句模板上（真机第 11-13 章的「下一拍」全同）。
            if n.chars().count() >= 2 && !crate::agency::prose_ground::is_generic_next_node(n) {
                return Some(n.chars().take(200).collect());
            }
        }
    }
    None
}

pub(crate) fn compile_next_node(pool: &DbPool, story_id: &str, current_content: &str) -> String {
    let shot = crate::agency::continue_assets::prior_tail_for_cast(current_content);
    let names: Vec<String> = CharacterRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.name)
        .collect();
    let mentioned = crate::agency::continue_assets::match_character_names(&names, &shot);
    let mut dead = crate::agency::continue_assets::dead_names_in_text(&names, &shot);
    // v0.64.7：已写入库的死亡不因近文窗口滑过而失效（下一拍不得再派死人上场）
    for name in crate::story_system::life_status::dead_names(pool, story_id) {
        if !dead.iter().any(|d| d == &name) {
            dead.push(name);
        }
    }
    let living: Vec<String> = mentioned
        .into_iter()
        .filter(|n| !dead.iter().any(|d| d == n))
        .collect();
    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    if let Some(latest) = scenes.last() {
        if let Some(node) =
            next_node_from_scene_outline(latest.outline_content.as_deref().unwrap_or(""))
        {
            let key: String = node.chars().take(20).collect();
            let replay =
                crate::agency::continue_assets::node_replays_completed_climax(&node, &shot, &names);
            if !replay && (key.is_empty() || !shot.contains(&key)) {
                return node;
            }
        }
    }
    let methodology_id = StoryRepository::new(pool.clone())
        .get_by_id(story_id)
        .ok()
        .flatten()
        .and_then(|s| s.methodology_id);
    let methodology_id =
        crate::agency::prose_ground::resolve_methodology_id(methodology_id.as_deref());
    let method_fallback =
        crate::agency::prose_ground::methodology_next_node(methodology_id, &shot, &living);
    let raw_outline = StoryOutlineRepository::new(pool.clone())
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|o| o.content)
        .unwrap_or_default();
    let scene_prose = crate::agency::materialize::concat_story_prose(pool, story_id);
    let prose = if crate::agency::prose_ground::has_substantial_prose(&scene_prose) {
        scene_prose
    } else {
        current_content.to_string()
    };
    let outline = if raw_outline.trim().is_empty()
        || (crate::agency::prose_ground::has_substantial_prose(&prose)
            && !crate::agency::prose_ground::outline_is_grounded(&raw_outline, &prose, &names))
    {
        String::new()
    } else {
        raw_outline
    };
    if outline.trim().is_empty() {
        return method_fallback;
    }
    let mut recent: Vec<&str> = scenes
        .iter()
        .filter_map(|s| s.outline_content.as_deref())
        .collect();
    recent.reverse();
    recent.truncate(3);
    let covered = recent.join("");
    let candidates: Vec<&str> = outline
        .split(['\n', '。', '！', '？', ';', '；'])
        .map(str::trim)
        .filter(|s| s.chars().count() >= 8)
        .collect();
    for cand in &candidates {
        let key: String = cand.chars().take(20).collect();
        if key.is_empty() {
            continue;
        }
        if covered.contains(&key) || shot.contains(&key) {
            continue;
        }
        if crate::agency::continue_assets::node_replays_completed_climax(cand, &shot, &names) {
            continue;
        }
        if !living.is_empty() && !living.iter().any(|n| cand.contains(n.as_str())) {
            continue;
        }
        return cand.chars().take(200).collect();
    }
    method_fallback
}

fn last_n_sentences(text: &str, n: usize, max_chars: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || n == 0 {
        return None;
    }
    let delimiters: &[char] = &['。', '？', '！', '.', '?', '!'];
    let mut ends: Vec<usize> = Vec::new();
    for (i, c) in trimmed.char_indices() {
        if delimiters.contains(&c) {
            ends.push(i + c.len_utf8());
        }
    }
    let slice = if ends.is_empty() {
        let total = trimmed.chars().count();
        if total > max_chars {
            trimmed.chars().skip(total - max_chars).collect::<String>()
        } else {
            trimmed.to_string()
        }
    } else {
        let take = n.min(ends.len());
        let start_byte = if take >= ends.len() {
            0
        } else {
            ends[ends.len() - take - 1]
        };
        let rest = trimmed[start_byte..].trim_start();
        let total = rest.chars().count();
        if total > max_chars {
            rest.chars().skip(total - max_chars).collect::<String>()
        } else {
            rest.to_string()
        }
    };
    let out = slice.trim();
    if out.is_empty() {
        None
    } else {
        Some(out.to_string())
    }
}

/// 末句硬锚点：复制自 `agents::orchestrator::build_ending_anchor`，避免
/// coordinator 耦合编排器。有正文时返回非空；空正文返回空串。
pub fn ending_anchor(current_content: &str) -> String {
    let plain = crate::agency::continue_assets::strip_editor_markup(current_content);
    let Some(last) = last_n_sentences(&plain, 2, 280) else {
        return String::new();
    };
    let extra = if crate::agency::continue_assets::prose_has_completed_death(&plain) {
        "近文已有人气绝或成尸体。禁止重演行刺、刺入胸口、头脸崩裂。从末句动作之后写活人的反应与乱局。\n"
    } else {
        ""
    };
    format!(
        "【续写硬锚点】\n\
         正文已写到此处。必须从末句之后继续，禁止另起开篇，禁止重写醒来/失忆/初入场景，\
         禁止用换一种说法重复末两句里已经完成的动作（饮酒、递盏、天气、跪拜等）。\n\
         人物、地点、未决问题以节拍任务与状态网为准，但不得与末句已发生的事实打架。\n\
         {extra}\
         ——已有正文末句——\n\
         {last}\n\
         ——请紧接上句继续写——"
    )
}

/// 主创 user prompt：卡全文 → 人物锁 → 状态网 → Bundle → 指令 → 卡摘要 →
/// 状态摘要 → 末句锚点。
pub fn render_writer_user_prompt(
    bundle_prompt: &str,
    card: &SceneBeatCard,
    instruction: &str,
    current_content: &str,
    state: Option<&crate::agency::beat_state::BeatState>,
    lock: Option<&crate::agency::continue_director::DirectorLock>,
) -> String {
    let state_full = state
        .map(|s| format!("\n\n{}", s.render_full()))
        .unwrap_or_default();
    let state_tail = state
        .map(|s| format!("\n\n{}", s.render_tail_summary()))
        .unwrap_or_default();
    let lock_block = lock
        .map(|l| {
            let t = l.render();
            if t.is_empty() {
                String::new()
            } else {
                format!("\n\n{t}")
            }
        })
        .unwrap_or_default();
    let facts = if card.dead.is_empty() {
        String::new()
    } else {
        format!(
            "\n【已发生事实】{}已死。禁止再写一次被刺、气绝、头脸崩裂。从末句动作之后写活人的反应与乱局。",
            card.dead.join("、")
        )
    };
    format!(
        "{card_full}{lock_block}{state_full}\n\n{bundle}\n\n【本次创作指令】\n{instruction}\n\n\
         须在节拍任务硬约束内落实指令核心意图。\n\n{card_tail}{state_tail}\n\n{ending}{facts}",
        card_full = card.render_full(),
        bundle = bundle_prompt,
        instruction = instruction,
        card_tail = card.render_tail_summary(),
        ending = ending_anchor(current_content),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool,
        repositories::{CreateCharacterRequest, CreateStoryRequest, StoryRepository},
    };

    fn story_req(title: &str) -> CreateStoryRequest {
        CreateStoryRequest {
            title: title.into(),
            description: None,
            genre: Some("玄幻".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }
    }

    fn char_req(story_id: &str, name: &str) -> CreateCharacterRequest {
        CreateCharacterRequest {
            story_id: story_id.into(),
            name: name.into(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
            source: None,
            is_auto_generated: None,
            emotional_core: None,
            emotional_trigger: None,
            emotional_wound: None,
            emotional_need: None,
        }
    }

    fn seed_story_minimal(pool: &DbPool) -> String {
        let story = StoryRepository::new(pool.clone())
            .create(story_req("最小卡"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&story.id, "阿岩"))
            .unwrap();
        story.id
    }

    fn seed_three_chars_one_silent(pool: &DbPool) -> String {
        let story = StoryRepository::new(pool.clone())
            .create(story_req("三角色一沉寂"))
            .unwrap();
        let repo = CharacterRepository::new(pool.clone());
        let mut a = char_req(&story.id, "阿岩");
        a.emotional_wound = Some("被师门放逐".into());
        a.emotional_need = Some("讨回公道".into());
        let a = repo.create(a).unwrap();
        repo.create(char_req(&story.id, "林雪")).unwrap();
        let b = repo.create(char_req(&story.id, "顾长夜")).unwrap();
        CharacterRelationshipRepository::new(pool.clone())
            .create(
                &story.id,
                &a.id,
                &b.id,
                "仇敌",
                None,
                None,
                Some("恨"),
                Some(0.9),
                Some("戒备"),
                Some(0.6),
            )
            .unwrap();
        StoryOutlineRepository::new(pool.clone())
            .create(
                &story.id,
                "夜宴破裂。林雪当众质问阿岩。顾长夜抽身。",
                None,
                3,
                None,
            )
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        for seq in 1..=3 {
            let scene = scene_repo
                .create_in_tx(&tx, &story.id, seq, Some("章"))
                .unwrap();
            scene_repo
                .update_in_tx(
                    &tx,
                    &scene.id,
                    &crate::db::repositories::SceneUpdate {
                        setting_location: Some("雨巷".into()),
                        characters_present: Some(vec!["阿岩".into(), "顾长夜".into()]),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        tx.commit().unwrap();
        story.id
    }

    #[test]
    fn beat_card_does_not_teleport_silent_without_character_move() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        {
            let conn = pool.get().unwrap();
            crate::creative_engine::expansion::write_beat_counters(
                &conn,
                &sid,
                crate::creative_engine::expansion::BeatCounters {
                    append_beats: 1,
                    last_conflict_beat: 1,
                    last_cast_refresh_beat: 1,
                    last_location_beat: 1,
                    last_foreshadow_beat: 1,
                },
            )
            .unwrap();
        }
        let card = compile_beat_card(&pool, &sid, "阿岩站在雨里。顾长夜冷笑。").unwrap();
        assert!(
            !card.cast.iter().any(|c| c.name == "林雪"),
            "无 CharacterMove 不得传送林雪 cast={:?}",
            card.cast
        );
        assert!(!card.conflict_move.action.is_empty());
        assert!(!card.emotion_beat.summary.is_empty());
        assert!(!card.next_outline_node.is_empty());
    }

    #[test]
    fn beat_card_cast_includes_silent_when_character_move_quota() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        {
            let conn = pool.get().unwrap();
            crate::creative_engine::expansion::write_beat_counters(
                &conn,
                &sid,
                crate::creative_engine::expansion::BeatCounters {
                    append_beats: 3,
                    last_conflict_beat: 3,
                    last_cast_refresh_beat: 0,
                    last_location_beat: 3,
                    last_foreshadow_beat: 3,
                },
            )
            .unwrap();
        }
        let card = compile_beat_card(&pool, &sid, "阿岩站在雨里。顾长夜冷笑。").unwrap();
        assert!(
            card.cast.iter().any(|c| c.name == "林雪"),
            "CharacterMove 应将林雪列入 cast={:?}",
            card.cast
        );
    }

    #[test]
    fn beat_card_never_empty_slots() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        let card = compile_beat_card(&pool, &sid, "他走了。").unwrap();
        assert!(!card.conflict_move.action.is_empty());
        assert!(!card.emotion_beat.summary.is_empty());
        assert!(!card.next_outline_node.is_empty());
    }

    #[test]
    fn writer_prompt_order_is_card_then_body_then_summary_then_ending() {
        let card = SceneBeatCard {
            cast: vec![CastMember {
                name: "林雪".into(),
                purpose: "回归质问".into(),
            }],
            conflict_move: ConflictMove {
                action: "加压：当众揭穿".into(),
                parties: vec!["林雪".into(), "阿岩".into()],
            },
            emotion_beat: EmotionBeat {
                summary: "林雪伤口=被抛弃".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("夜宴".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta::default(),
        };
        let prompt = render_writer_user_prompt(
            "【红线】不可飞天",
            &card,
            "往下写",
            "他推开门。",
            None,
            None,
        );
        let i_card = prompt.find("【本章节拍任务】").unwrap();
        let i_sum = prompt.find("【节拍摘要】").unwrap();
        let i_end = prompt
            .find("必须从上述末句")
            .unwrap_or_else(|| prompt.find("末句").unwrap());
        assert!(i_card < i_sum);
        assert!(i_sum < i_end);
        assert!(prompt.contains("林雪"));
        assert!(!prompt.contains("最高优先级"));
    }

    #[test]
    fn ending_anchor_empty_when_no_content() {
        assert!(ending_anchor("").is_empty());
        assert!(ending_anchor("   ").is_empty());
    }

    #[test]
    fn ending_anchor_strips_html_tags() {
        let html = "<p>他推开门。</p><p>雨还在下。</p>";
        let a = ending_anchor(html);
        assert!(!a.contains("<p>"), "{a}");
        assert!(!a.contains("</p>"), "{a}");
        assert!(a.contains("雨还在下"), "{a}");
        assert!(a.contains("他推开门"), "{a}");
        assert!(!a.contains("最高优先级"), "{a}");
    }

    #[test]
    fn cast_present_only_from_chapter_tail() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(story_req("末段点名"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&story.id, "青梧甲"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&story.id, "客栈乙"))
            .unwrap();
        let opening = "青梧甲在雨里立誓。";
        let middle = "闲笔。".repeat(800);
        let ending = "客栈乙扣上匣子。";
        let card =
            compile_beat_card(&pool, &story.id, &format!("{opening}{middle}{ending}")).unwrap();
        let present: Vec<_> = card
            .cast
            .iter()
            .filter(|c| c.purpose.contains("可沉默") || c.purpose.contains("末段已在场"))
            .map(|c| c.name.as_str())
            .collect();
        assert!(
            present.contains(&"客栈乙"),
            "近文人物应标末段在场 cast={:?}",
            card.cast
        );
        assert!(
            !present.contains(&"青梧甲"),
            "开篇人物不得标成末段在场 cast={:?}",
            card.cast
        );
    }

    #[test]
    fn quota_text_for_beats_uses_pai_not_debug_enum() {
        let debt = crate::creative_engine::expansion::ExpansionDebt {
            conflict: 2,
            scene: 0,
            character: 0,
            foreshadow: 0,
        };
        let text = quota_text_for_beats(&debt).expect("quota");
        assert!(text.contains("本拍扩张任务"));
        assert!(text.contains("必须"));
        assert!(!text.contains("ConflictEscalation"));
        assert!(!text.contains("本章扩张任务"));
    }

    #[test]
    fn compile_next_node_does_not_rewind_to_first_sentence() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        StoryOutlineRepository::new(pool.clone())
            .create(&sid, "开篇灵堂。钟楼破阵。龙脉重封。", None, 3, None)
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo.create_in_tx(&tx, &sid, 1, Some("章")).unwrap();
        scene_repo
            .update_in_tx(
                &tx,
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    outline_content: Some(
                        "进度：开篇灵堂。\n进度：钟楼破阵。\n进度：龙脉重封。".into(),
                    ),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        let node = compile_next_node(&pool, &sid, "");
        assert!(!node.starts_with("开篇灵堂"), "rewound: {node}");
        assert!(
            node.contains("把当前冲突推进一步")
                || node.contains("不得原地复述")
                || node.contains("场景结构")
                || node.contains("不得另起开篇"),
            "covered 书纲应变空并回落方法论下一拍, got={node}"
        );
    }

    #[test]
    fn compile_next_node_skips_book_beats_without_present_cast() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        CharacterRepository::new(pool.clone())
            .create(char_req(&sid, "苏会山"))
            .unwrap();
        StoryOutlineRepository::new(pool.clone())
            .create(
                &sid,
                "三年后京城火起奉乾帝崩。苏会山在席间把酒盏捏出裂纹。",
                None,
                3,
                None,
            )
            .unwrap();
        let node = compile_next_node(&pool, &sid, "盖头轻晃。苏会山接过酒盏，一饮而尽。");
        assert!(
            node.contains("苏会山"),
            "下一节点须落在本拍在场者身上, got={node}"
        );
        assert!(
            !node.contains("奉乾帝"),
            "不得跳到与本拍无关的书纲, got={node}"
        );
    }

    #[test]
    fn compile_next_node_ignores_ungrounded_book_outline() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        CharacterRepository::new(pool.clone())
            .create(char_req(&sid, "苏会山"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&sid, "费迪南三世"))
            .unwrap();
        StoryOutlineRepository::new(pool.clone())
            .create(
                &sid,
                "第一卷·灰烬低语。费迪南三世为撑烟火节加征火药税。费迪南得知苏会山遇刺。",
                None,
                3,
                None,
            )
            .unwrap();
        let mut shot = "盖头轻晃。苏会山接过酒盏，一饮而尽。".to_string();
        while shot.chars().count() < 200 {
            shot.push_str("红毡未干。");
        }
        let node = compile_next_node(&pool, &sid, &shot);
        assert!(
            !node.contains("费迪南"),
            "未接地书大纲不得充当下一节点 got={node}"
        );
        assert!(
            node.contains("苏会山") || node.contains("反应") || node.contains("场景结构"),
            "应回落到本场方法论下一拍 got={node}"
        );
    }

    #[test]
    fn cast_is_shot_window_not_whole_near_prose() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(story_req("镜头在场"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&story.id, "曹元佩"))
            .unwrap();
        CharacterRepository::new(pool.clone())
            .create(char_req(&story.id, "苏会山"))
            .unwrap();
        let text = format!(
            "曹元佩站在廊下记那一步之差。{}苏会山饮尽杯中酒。",
            "雪落。".repeat(600)
        );
        let card = compile_beat_card(&pool, &story.id, &text).unwrap();
        let present: Vec<_> = card
            .cast
            .iter()
            .filter(|c| c.purpose.contains("可沉默") || c.purpose.contains("末段已在场"))
            .map(|c| c.name.as_str())
            .collect();
        assert!(present.contains(&"苏会山"), "cast={:?}", card.cast);
        assert!(
            !present.contains(&"曹元佩"),
            "章中人物不得算本拍必须在场 cast={:?}",
            card.cast
        );
        assert!(
            !card.cast.iter().any(|c| c.purpose.contains("补位上场")),
            "不得按角色表顺序补位 cast={:?}",
            card.cast
        );
    }

    #[test]
    fn ending_anchor_forbids_paraphrasing_last_actions() {
        let a = ending_anchor("苏会山一饮而尽。窗外风声渐紧。");
        assert!(a.contains("禁止用换一种说法"), "{a}");
        assert!(a.contains("一饮而尽"), "{a}");
    }

    #[test]
    fn compile_conflict_ignores_offstage_enemy() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        let chars = CharacterRepository::new(pool.clone())
            .get_by_story(&sid)
            .unwrap();
        let cast = vec![CastMember {
            name: "林雪".into(),
            purpose: "可沉默".into(),
        }];
        let mv = compile_conflict(&chars, &cast, &pool, &sid, "林雪");
        assert!(!mv.parties.iter().any(|p| p == "顾长夜"));
        assert!(!mv.action.contains("顾长夜"));
    }

    #[test]
    fn third_compile_after_two_idle_appends_has_conflict_quota_zh() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&sid)
            .unwrap();
        let scene_id = scenes.last().unwrap().id.clone();
        let card = compile_beat_card(&pool, &sid, "阿岩站在雨里。").unwrap();
        crate::agency::persist::persist_append_with_card(
            &pool,
            &scene_id,
            "阿岩站在雨里。",
            &"续写增量正文。".repeat(30),
            &card,
        )
        .unwrap();
        crate::agency::persist::persist_append_with_card(
            &pool,
            &scene_id,
            "阿岩站在雨里。",
            &"续写增量正文。".repeat(30),
            &card,
        )
        .unwrap();
        let card3 = compile_beat_card(&pool, &sid, "阿岩站在雨里。").unwrap();
        let text = card3.expansion_quota_text.clone().unwrap_or_default();
        let full = card3.render_full();
        assert!(
            text.contains("冲突") || full.contains("本拍扩张任务"),
            "quota missing: {full}"
        );
        assert!(!full.contains("ConflictEscalation"));
    }

    #[test]
    fn compile_next_node_prefers_current_scene_outline() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        StoryOutlineRepository::new(pool.clone())
            .create(
                &sid,
                "三年后京城火起奉乾帝崩。苏会山在席间把酒盏捏出裂纹。",
                None,
                3,
                None,
            )
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo.create_in_tx(&tx, &sid, 1, Some("章")).unwrap();
        scene_repo
            .update_in_tx(
                &tx,
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    outline_content: Some(
                        "【当前场大纲】\n在场：阿岩\n冲突：加压\n情感：怒\n下一拍：留在夜宴厅对质"
                            .into(),
                    ),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        let node = compile_next_node(&pool, &sid, "阿岩推开门。");
        assert!(
            node.contains("留在夜宴厅"),
            "应读当前场大纲下一拍, got={node}"
        );
        assert!(!node.contains("奉乾帝"), "不得改走书纲, got={node}");
    }

    #[test]
    fn beat_card_render_full_includes_open_review_issues() {
        let card = SceneBeatCard {
            cast: vec![CastMember {
                name: "林雪".into(),
                purpose: "质问".into(),
            }],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["林雪".into()],
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: None,
            open_review_issues: vec!["苏会山与曹元佩的冲突未兑现".into()],
            dead: vec![],
            change_delta: ChangeDelta {
                kind: ChangeKind::Risk,
                summary: "加压".into(),
            },
        };
        let full = card.render_full();
        assert!(full.contains("【待兑现审查】"));
        assert!(full.contains("苏会山"));
        assert!(
            full.contains("必须改变：风险 — 加压"),
            "节拍卡须写出本拍改变项 full={full}"
        );
        assert!(card.render_scene_outline().contains("【当前场大纲】"));
        assert!(card.render_scene_outline().contains("下一拍：夜宴破裂"));
    }

    #[test]
    fn change_delta_from_hostile_cast() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        let card = compile_beat_card(&pool, &sid, "阿岩站在雨里。顾长夜冷笑。").unwrap();
        assert_eq!(
            card.change_delta.kind,
            ChangeKind::Risk,
            "敌对双方在场须编译为风险 delta={:?}",
            card.change_delta
        );
        assert!(
            card.change_delta.summary.contains("加压")
                || card.change_delta.summary.contains("对峙"),
            "summary={}",
            card.change_delta.summary
        );
    }

    #[test]
    fn beat_card_render_includes_must_change() {
        let pool = create_test_pool().unwrap();
        let sid = seed_three_chars_one_silent(&pool);
        let card = compile_beat_card(&pool, &sid, "阿岩站在雨里。顾长夜冷笑。").unwrap();
        let full = card.render_full();
        assert!(
            full.contains("必须改变："),
            "render_full 须含必须改变行 full={full}"
        );
        assert!(full.contains(card.change_delta.kind.as_zh()));
    }

    #[test]
    fn compile_beat_card_injects_prior_run_revise_issues() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        let repo = crate::agency::repository::AgencyRepository::new(pool.clone());
        repo.create_run(&crate::agency::models::AgencyRun::new("old-r", "续写"))
            .unwrap();
        repo.set_run_story("old-r", &sid).unwrap();
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        let content = serde_json::json!({
            "outcome": "revise",
            "issues": ["苏会山与曹元佩的冲突未兑现"],
        })
        .to_string();
        board
            .write(
                "old-r",
                &sid,
                crate::agency::models::AgentRole::EditorAuditor,
                crate::agency::models::BoardZone::Review,
                "gate",
                "gate-ch2-r1",
                &content,
                "gate:revise 1 条问题",
            )
            .unwrap();
        let card = compile_beat_card(&pool, &sid, "阿岩走了。").unwrap();
        assert!(
            card.open_review_issues.iter().any(|i| i.contains("苏会山")),
            "got={:?}",
            card.open_review_issues
        );
        assert!(card.render_full().contains("【待兑现审查】"));
    }

    fn seed_wedding_cast(pool: &DbPool) -> String {
        let story = StoryRepository::new(pool.clone())
            .create(story_req("大婚礼成"))
            .unwrap();
        for name in ["苏会山", "明成公主", "苏亦铁", "曹元佩", "景亲王"] {
            CharacterRepository::new(pool.clone())
                .create(char_req(&story.id, name))
                .unwrap();
        }
        story.id
    }

    #[test]
    fn wedding_climax_dead_are_not_acting_cast_or_conflict() {
        let pool = create_test_pool().unwrap();
        let sid = seed_wedding_cast(&pool);
        let card = compile_beat_card(
            &pool,
            &sid,
            crate::agency::continue_assets::WEDDING_ASSASSINATION_TAIL,
        )
        .unwrap();
        let acting: Vec<_> = card.cast.iter().map(|c| c.name.as_str()).collect();
        assert!(card.dead.contains(&"苏会山".into()), "dead={:?}", card.dead);
        assert!(
            card.dead.contains(&"明成公主".into()),
            "dead={:?}",
            card.dead
        );
        assert!(!acting.contains(&"苏会山"), "cast={:?}", card.cast);
        assert!(!acting.contains(&"明成公主"), "cast={:?}", card.cast);
        assert!(acting.contains(&"苏亦铁"), "cast={:?}", card.cast);
        assert!(
            !card.conflict_move.parties.iter().any(|p| p == "苏会山"),
            "conflict={:?}",
            card.conflict_move
        );
        assert!(
            !card.conflict_move.parties.iter().any(|p| p == "明成公主"),
            "conflict={:?}",
            card.conflict_move
        );
        let prompt = render_writer_user_prompt(
            "",
            &card,
            "续写",
            crate::agency::continue_assets::WEDDING_ASSASSINATION_TAIL,
            None,
            None,
        );
        assert!(prompt.contains("已死"), "{prompt}");
        assert!(prompt.contains("禁止再写一次"), "{prompt}");
        assert!(prompt.contains("飞身扑上"), "{prompt}");
    }

    #[test]
    fn compile_next_node_skips_stab_already_written_in_prose() {
        let pool = create_test_pool().unwrap();
        let sid = seed_wedding_cast(&pool);
        StoryOutlineRepository::new(pool.clone())
            .create(
                &sid,
                "大婚之日明成公主于二拜高堂行刺苏会山。苏亦铁当众驳斥谋反。",
                None,
                3,
                None,
            )
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo.create_in_tx(&tx, &sid, 1, Some("章")).unwrap();
        scene_repo
            .update_in_tx(
                &tx,
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    outline_content: Some(
                        "【当前场大纲】\n在场：苏亦铁\n冲突：加压\n情感：怒\n下一拍：明成公主于二拜高堂行刺苏会山"
                            .into(),
                    ),
                    content: Some(crate::agency::continue_assets::WEDDING_ASSASSINATION_TAIL.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        let node = compile_next_node(
            &pool,
            &sid,
            crate::agency::continue_assets::WEDDING_ASSASSINATION_TAIL,
        );
        assert!(
            !node.contains("行刺"),
            "不得把已写完的行刺再当下一拍 got={node}"
        );
        assert!(
            node.contains("苏亦铁") || node.contains("反应") || node.contains("谋反"),
            "应从刺杀之后写活人反应 got={node}"
        );
    }

    #[test]
    fn ending_anchor_forbids_replaying_completed_deaths() {
        let a = ending_anchor(crate::agency::continue_assets::WEDDING_ASSASSINATION_TAIL);
        assert!(a.contains("禁止重演行刺"), "{a}");
        assert!(a.contains("飞身扑上"), "{a}");
    }

    /// v0.64.7 真机契约（《帝国的烟火》第 10 章）：死亡写上在第 2 章，
    /// 近文 1500 字窗口早已滑过；只要近文把她当活人写，旧逻辑就会把她
    /// 重新列入 cast。持久化生死状态必须压过局部窗口。
    #[test]
    fn persisted_dead_beats_local_window_for_cast_and_next_node() {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(story_req("死人不得复活"))
            .unwrap();
        let repo = CharacterRepository::new(pool.clone());
        repo.create(char_req(&story.id, "苏亦铁")).unwrap();
        repo.create(char_req(&story.id, "明成公主")).unwrap();
        crate::story_system::life_status::mark_dead(&pool, &story.id, "明成公主", Some(2)).unwrap();

        // 近文里她"活了"：走动、说话、抓手腕（真机第 10 章的形态）
        let content = "明成公主往前走了一步，抓住苏亦铁的手腕。\
                       苏亦铁没有动。明成公主笑了，说：把印给我。";
        let card = compile_beat_card(&pool, &story.id, content).unwrap();

        assert!(
            card.dead.iter().any(|d| d == "明成公主"),
            "持久化已死必须在 card.dead 里 dead={:?}",
            card.dead
        );
        assert!(
            !card.cast.iter().any(|c| c.name == "明成公主"),
            "已死角色不得进 cast cast={:?}",
            card.cast
        );
        assert!(
            card.cast.iter().any(|c| c.name == "苏亦铁"),
            "活着的在场者仍在 cast cast={:?}",
            card.cast
        );

        let node = compile_next_node(&pool, &story.id, content);
        assert!(
            !node.contains("明成公主"),
            "下一拍不得再派已死角色上场 node={node}"
        );
    }

    /// v0.64.9 真机契约：通用兜底「下一拍」不落库（否则逐拍滚动复制），
    /// 落库的兜底句也不再被当计划采纳。
    #[test]
    fn generic_next_node_is_neither_persisted_nor_adopted() {
        let pool = create_test_pool().unwrap();
        let sid = seed_story_minimal(&pool);
        let mut card = compile_beat_card(&pool, &sid, "阿岩站在雨里。").unwrap();
        card.next_outline_node =
            "按场景结构推进：目标→冲突→灾难或反应→困境→决定。只写本场阿岩，不得另起开篇。".into();
        let outline = card.render_scene_outline();
        assert!(outline.contains(CURRENT_SCENE_OUTLINE_MARK), "{outline}");
        assert!(!outline.contains("下一拍："), "兜底句不得落库: {outline}");
        assert!(
            next_node_from_scene_outline(&outline).is_none(),
            "兜底句不得被采纳: {outline}"
        );

        // 真实计划节点照常落库并采纳
        card.next_outline_node = "阿岩把断灯绳交给林雪，禁区的门开了".into();
        let outline = card.render_scene_outline();
        assert!(
            outline.contains("下一拍：阿岩把断灯绳交给林雪"),
            "{outline}"
        );
        assert_eq!(
            next_node_from_scene_outline(&outline).as_deref(),
            Some("阿岩把断灯绳交给林雪，禁区的门开了")
        );
    }
}
