//! OCC 评估引擎：事件 → 评估 → 情绪 → 行动倾向。
//!
//! P0 全部纯规则，零 LLM。事件先被归入有限的 `EventKind`，再按角色伤口/
//! 需求/内核形成的敏感度给出 `Appraisal`，最后合成为 `EmotionVector`。

use serde::{Deserialize, Serialize};

use super::{
    types::DialoguePolicy,
    vector::{EmotionKind, EmotionVector},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Betrayal,
    Loss,
    Threat,
    Rejection,
    Help,
    Gift,
    Achievement,
    Humiliation,
    Separation,
    Reunion,
    SecretReveal,
    Sacrifice,
    Neutral,
}

impl EventKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Betrayal => "betrayal",
            Self::Loss => "loss",
            Self::Threat => "threat",
            Self::Rejection => "rejection",
            Self::Help => "help",
            Self::Gift => "gift",
            Self::Achievement => "achievement",
            Self::Humiliation => "humiliation",
            Self::Separation => "separation",
            Self::Reunion => "reunion",
            Self::SecretReveal => "secret_reveal",
            Self::Sacrifice => "sacrifice",
            Self::Neutral => "neutral",
        }
    }

    pub fn from_id(id: &str) -> Self {
        match id.trim().to_ascii_lowercase().as_str() {
            "betrayal" => Self::Betrayal,
            "loss" => Self::Loss,
            "threat" => Self::Threat,
            "rejection" => Self::Rejection,
            "help" => Self::Help,
            "gift" => Self::Gift,
            "achievement" => Self::Achievement,
            "humiliation" => Self::Humiliation,
            "separation" => Self::Separation,
            "reunion" => Self::Reunion,
            "secret_reveal" => Self::SecretReveal,
            "sacrifice" => Self::Sacrifice,
            _ => Self::Neutral,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Betrayal => "被背叛",
            Self::Loss => "失去",
            Self::Threat => "受威胁",
            Self::Rejection => "被拒绝",
            Self::Help => "被帮助",
            Self::Gift => "收到馈赠",
            Self::Achievement => "达成目标",
            Self::Humiliation => "被羞辱",
            Self::Separation => "分离",
            Self::Reunion => "重逢",
            Self::SecretReveal => "真相暴露",
            Self::Sacrifice => "牺牲",
            Self::Neutral => "中性事件",
        }
    }

    pub fn congruence(self) -> f32 {
        match self {
            Self::Betrayal => -0.9,
            Self::Loss => -1.0,
            Self::Threat => -0.8,
            Self::Rejection => -0.9,
            Self::Help => 0.8,
            Self::Gift => 0.7,
            Self::Achievement => 0.9,
            Self::Humiliation => -0.8,
            Self::Separation => -0.7,
            Self::Reunion => 0.8,
            Self::SecretReveal => -0.2,
            Self::Sacrifice => -0.1,
            Self::Neutral => 0.0,
        }
    }

    pub fn certainty(self) -> f32 {
        match self {
            Self::Betrayal => 0.9,
            Self::Loss => 0.95,
            Self::Threat => 0.5,
            Self::Rejection => 0.8,
            Self::Help => 0.7,
            Self::Gift => 0.8,
            Self::Achievement => 0.9,
            Self::Humiliation => 0.9,
            Self::Separation => 0.9,
            Self::Reunion => 0.9,
            Self::SecretReveal => 0.8,
            Self::Sacrifice => 0.9,
            Self::Neutral => 0.5,
        }
    }

    pub fn control(self) -> f32 {
        match self {
            Self::Betrayal => 0.2,
            Self::Loss => 0.1,
            Self::Threat => 0.4,
            Self::Rejection => 0.2,
            Self::Help => 0.7,
            Self::Gift => 0.8,
            Self::Achievement => 0.9,
            Self::Humiliation => 0.2,
            Self::Separation => 0.1,
            Self::Reunion => 0.5,
            Self::SecretReveal => 0.3,
            Self::Sacrifice => 0.6,
            Self::Neutral => 0.5,
        }
    }

    pub fn norm_violation(self) -> f32 {
        match self {
            Self::Betrayal => 0.9,
            Self::Threat => 0.7,
            Self::Humiliation => 0.8,
            Self::Rejection => 0.5,
            _ => 0.0,
        }
    }

    fn default_agency(self, other: Option<&str>) -> Agency {
        match self {
            Self::Achievement | Self::Sacrifice => Agency::SelfAgent,
            Self::Loss | Self::Separation => Agency::Circumstance,
            Self::Threat | Self::Neutral => Agency::Circumstance,
            _ => match other {
                Some(name) if !name.trim().is_empty() => Agency::Other(name.to_string()),
                _ => Agency::Circumstance,
            },
        }
    }

    fn base_emotions(self) -> Vec<(EmotionKind, f32)> {
        match self {
            Self::Betrayal => vec![
                (EmotionKind::Anger, 0.45),
                (EmotionKind::Disgust, 0.25),
                (EmotionKind::Shame, 0.30),
            ],
            Self::Loss => vec![(EmotionKind::Sadness, 0.70), (EmotionKind::Love, 0.30)],
            Self::Threat => vec![(EmotionKind::Fear, 0.75), (EmotionKind::Anger, 0.25)],
            Self::Rejection => vec![
                (EmotionKind::Shame, 0.45),
                (EmotionKind::Sadness, 0.35),
                (EmotionKind::Anger, 0.20),
            ],
            Self::Help => vec![
                (EmotionKind::Joy, 0.35),
                (EmotionKind::Trust, 0.35),
                (EmotionKind::Love, 0.30),
            ],
            Self::Gift => vec![
                (EmotionKind::Joy, 0.50),
                (EmotionKind::Trust, 0.30),
                (EmotionKind::Surprise, 0.20),
            ],
            Self::Achievement => vec![
                (EmotionKind::Pride, 0.50),
                (EmotionKind::Joy, 0.35),
                (EmotionKind::Hope, 0.15),
            ],
            Self::Humiliation => vec![
                (EmotionKind::Shame, 0.55),
                (EmotionKind::Anger, 0.30),
                (EmotionKind::Disgust, 0.15),
            ],
            Self::Separation => vec![(EmotionKind::Sadness, 0.65), (EmotionKind::Love, 0.35)],
            Self::Reunion => vec![
                (EmotionKind::Joy, 0.40),
                (EmotionKind::Love, 0.35),
                (EmotionKind::Trust, 0.25),
            ],
            Self::SecretReveal => vec![
                (EmotionKind::Surprise, 0.50),
                (EmotionKind::Fear, 0.30),
                (EmotionKind::Anger, 0.20),
            ],
            Self::Sacrifice => vec![
                (EmotionKind::Love, 0.45),
                (EmotionKind::Sadness, 0.30),
                (EmotionKind::Pride, 0.25),
            ],
            Self::Neutral => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agency {
    SelfAgent,
    Other(String),
    Circumstance,
}

impl Agency {
    pub fn label(&self) -> String {
        match self {
            Self::SelfAgent => "自己造成".into(),
            Self::Other(name) => format!("{name}造成"),
            Self::Circumstance => "环境造成".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appraisal {
    pub relevance: f32,
    pub congruence: f32,
    pub agency: Agency,
    pub certainty: f32,
    pub control: f32,
    pub norm_violation: f32,
    pub attachment: f32,
}

impl Default for Appraisal {
    fn default() -> Self {
        Self {
            relevance: 0.5,
            congruence: 0.0,
            agency: Agency::Circumstance,
            certainty: 0.5,
            control: 0.5,
            norm_violation: 0.0,
            attachment: 0.3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Sensitivities {
    pub abandonment: f32,
    pub betrayal: f32,
    pub shame: f32,
    pub control_need: f32,
    pub attachment_need: f32,
    pub anger_bias: f32,
}

impl Sensitivities {
    fn max_relevant(&self, event: EventKind) -> f32 {
        let mut values = vec![
            self.abandonment,
            self.betrayal,
            self.shame,
            self.control_need,
        ];
        if matches!(
            event,
            EventKind::Loss | EventKind::Separation | EventKind::Reunion
        ) {
            values.push(self.attachment_need);
        }
        if matches!(
            event,
            EventKind::Betrayal | EventKind::Threat | EventKind::Humiliation
        ) {
            values.push(self.anger_bias);
        }
        values.into_iter().fold(0.0f32, f32::max)
    }
}

pub fn sensitivities_from(wound: &str, need: &str, core: &str) -> Sensitivities {
    let mut s = Sensitivities {
        attachment_need: 0.5,
        ..Default::default()
    };
    let w = wound.to_string();
    let n = need.to_string();
    let c = core.to_string();
    if w.contains("抛弃") || w.contains("遗弃") || w.contains("丢下") {
        s.abandonment = 0.9;
    }
    if w.contains("背叛") || w.contains("欺骗") || w.contains("出卖") {
        s.betrayal = 0.9;
    }
    if w.contains("屈辱") || w.contains("羞辱") || w.contains("当众") || w.contains("丢脸")
    {
        s.shame = 0.85;
    }
    if w.contains("失败") || w.contains("否定") || w.contains("不被") {
        s.shame = s.shame.max(0.6);
    }
    if w.contains("囚") || w.contains("控制") || w.contains("被迫") {
        s.control_need = 0.8;
    }
    if w.contains("死") || w.contains("失去") || w.contains("惨") {
        s.attachment_need = s.attachment_need.max(0.8);
    }
    if n.contains("认可") || n.contains("肯定") || n.contains("看见") {
        s.shame = s.shame.max(0.55);
    }
    if n.contains("归属") || n.contains("爱") || n.contains("留下") {
        s.attachment_need = s.attachment_need.max(0.85);
    }
    if n.contains("掌控") || n.contains("控制") || n.contains("自由") {
        s.control_need = s.control_need.max(0.8);
    }
    if n.contains("复仇") || n.contains("报复") || n.contains("讨回") {
        s.anger_bias = s.anger_bias.max(0.85);
    }
    if c.contains("怒") || c.contains("恨") || c.contains("暴") {
        s.anger_bias = s.anger_bias.max(0.8);
    }
    if c.contains("冷") || c.contains("压抑") || c.contains("忍") {
        s.shame = s.shame.max(0.5);
    }
    if c.contains("怕") || c.contains("不安") || c.contains("恐惧") {
        s.abandonment = s.abandonment.max(0.7);
    }
    s
}

pub fn appraise_event(
    event: EventKind,
    other: Option<&str>,
    sensitivities: &Sensitivities,
) -> Appraisal {
    let relevant = sensitivities.max_relevant(event);
    let mut relevance = 0.45 + 0.35 * relevant;
    if other.is_some() {
        relevance += 0.10;
    }
    let relevance = relevance.clamp(0.0, 1.0);
    let control = (event.control() * (1.0 - 0.25 * sensitivities.control_need)).clamp(0.0, 1.0);
    let certainty = (event.certainty() * (0.85 + 0.15 * relevance)).clamp(0.0, 1.0);
    let norm_violation = (event.norm_violation() + 0.15 * sensitivities.shame).clamp(0.0, 1.0);
    let attachment = if other.is_some() {
        (0.50 + 0.45 * sensitivities.attachment_need).clamp(0.0, 1.0)
    } else {
        0.20
    };
    Appraisal {
        relevance,
        congruence: event.congruence(),
        agency: event.default_agency(other),
        certainty,
        control,
        norm_violation,
        attachment,
    }
}

pub fn emotion_from_appraisal(
    event: EventKind,
    appraisal: &Appraisal,
    sensitivities: &Sensitivities,
) -> EmotionVector {
    let mut v = EmotionVector::default();
    let scale = 0.45 + 0.55 * appraisal.relevance;
    for (kind, weight) in event.base_emotions() {
        let old = v.get(kind);
        v.set(kind, old + weight * scale);
    }
    // 不确定 + 不可控 → 恐惧；被他人违背规范 → 愤怒。
    if appraisal.certainty < 0.55 && appraisal.control < 0.55 {
        let fear = (0.55 - appraisal.certainty) * (0.55 - appraisal.control) * 1.2;
        let old = v.get(EmotionKind::Fear);
        v.set(EmotionKind::Fear, old + fear);
    }
    if appraisal.norm_violation > 0.45 && appraisal.congruence < 0.0 {
        let anger = appraisal.norm_violation * 0.35;
        let old = v.get(EmotionKind::Anger);
        v.set(EmotionKind::Anger, old + anger);
    }
    if appraisal.norm_violation > 0.45 && appraisal.attachment > 0.5 {
        let shame = appraisal.norm_violation * appraisal.attachment * 0.25;
        let old = v.get(EmotionKind::Shame);
        v.set(EmotionKind::Shame, old + shame);
    }
    if matches!(appraisal.agency, Agency::SelfAgent) && appraisal.congruence < 0.0 {
        let old = v.get(EmotionKind::Guilt);
        v.set(EmotionKind::Guilt, old + 0.40);
    }
    // 角色敏感度偏置。
    if sensitivities.abandonment > 0.5 {
        let old = v.get(EmotionKind::Fear);
        v.set(EmotionKind::Fear, old + 0.10 * sensitivities.abandonment);
    }
    if sensitivities.betrayal > 0.5 {
        let old = v.get(EmotionKind::Anger);
        v.set(EmotionKind::Anger, old + 0.10 * sensitivities.betrayal);
    }
    if sensitivities.shame > 0.5 {
        let old = v.get(EmotionKind::Shame);
        v.set(EmotionKind::Shame, old + 0.12 * sensitivities.shame);
    }
    if sensitivities.anger_bias > 0.5 {
        let old = v.get(EmotionKind::Anger);
        v.set(EmotionKind::Anger, old + 0.10 * sensitivities.anger_bias);
    }
    v.normalized()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionTendency {
    Approach,
    Withdraw,
    Attack,
    Freeze,
    Appease,
    Confess,
    Conceal,
    Help,
    Sacrifice,
    SeekContact,
    Punish,
    Betray,
}

impl ActionTendency {
    pub fn label(self) -> &'static str {
        match self {
            Self::Approach => "接近",
            Self::Withdraw => "退缩",
            Self::Attack => "反击",
            Self::Freeze => "僵住",
            Self::Appease => "安抚/讨好",
            Self::Confess => "坦白",
            Self::Conceal => "隐瞒",
            Self::Help => "帮助",
            Self::Sacrifice => "牺牲",
            Self::SeekContact => "寻求连接",
            Self::Punish => "惩罚",
            Self::Betray => "背叛",
        }
    }

    /// 探针用的可见动作词。空 = 该倾向本身不可拍，需要转译。
    pub fn visible_markers(self) -> &'static [&'static str] {
        match self {
            Self::Attack => &[
                "质问", "逼", "抓", "推", "打", "拔", "摔", "冷笑", "顶", "拦",
            ],
            Self::Punish => &["惩罚", "夺走", "赶", "关", "押", "揭穿", "公开", "摊牌"],
            Self::Withdraw => &["退", "转身", "离开", "避开", "沉默", "低头", "缩"],
            Self::Freeze => &["僵", "愣", "怔", "停住", "没有动", "说不出"],
            Self::Appease => &["求", "哄", "让步", "道歉", "软化", "递", "赔"],
            Self::Confess => &["承认", "坦白", "说出", "交代", "道歉"],
            Self::Conceal => &["否认", "瞒", "藏", "转移话题", "装作", "笑", "沉默"],
            Self::Help => &["扶", "挡", "护", "递", "替", "救", "留下"],
            Self::Sacrifice => &["挡在前面", "让出", "放弃", "留下", "替", "承受"],
            Self::SeekContact => &["抓住", "追问", "靠近", "留下", "等", "找"],
            Self::Approach => &["走近", "开口", "伸手", "坐下", "看向"],
            Self::Betray => &["出卖", "背叛", "告发", "投靠", "反水"],
        }
    }
}

pub fn action_tendency(emotion: &EmotionVector, defense: &str) -> (ActionTendency, String) {
    let (dominant, _) = emotion.dominant();
    let defense = defense.to_string();
    let mut tendency = match dominant {
        EmotionKind::Anger => ActionTendency::Attack,
        EmotionKind::Fear => ActionTendency::Freeze,
        EmotionKind::Sadness => ActionTendency::Withdraw,
        EmotionKind::Shame => ActionTendency::Conceal,
        EmotionKind::Guilt => ActionTendency::Confess,
        EmotionKind::Love => ActionTendency::Help,
        EmotionKind::Trust => ActionTendency::Approach,
        EmotionKind::Joy => ActionTendency::Approach,
        EmotionKind::Pride => ActionTendency::Approach,
        EmotionKind::Hope => ActionTendency::Approach,
        EmotionKind::Disgust => ActionTendency::Withdraw,
        EmotionKind::Surprise => ActionTendency::Freeze,
    };
    if defense.contains("攻击") || defense.contains("反击") || defense.contains("爆发") {
        tendency = match dominant {
            EmotionKind::Shame | EmotionKind::Fear => ActionTendency::Attack,
            _ => tendency,
        };
    } else if defense.contains("回避") || defense.contains("逃") {
        if matches!(dominant, EmotionKind::Anger | EmotionKind::Love) {
            tendency = ActionTendency::Withdraw;
        }
    } else if defense.contains("讨好") || defense.contains("顺从") {
        if matches!(dominant, EmotionKind::Anger | EmotionKind::Fear) {
            tendency = ActionTendency::Appease;
        }
    } else if defense.contains("理智") || defense.contains("分析") {
        if matches!(dominant, EmotionKind::Fear | EmotionKind::Shame) {
            tendency = ActionTendency::Conceal;
        }
    }
    let reason = format!(
        "主导情绪={}，防御={}，行动倾向={}",
        dominant.label(),
        if defense.trim().is_empty() {
            "未明"
        } else {
            defense.trim()
        },
        tendency.label()
    );
    (tendency, reason)
}

pub fn dialogue_policy(tendency: ActionTendency, forbidden: Vec<String>) -> DialoguePolicy {
    let (objective, tactic, subtext) = match tendency {
        ActionTendency::Attack | ActionTendency::Punish => (
            "逼对方收回/承认/让步",
            "质问、冷嘲、抓住一个具体细节不放",
            "你凭什么替我做决定；我要你看见我受了什么",
        ),
        ActionTendency::Withdraw | ActionTendency::Freeze => (
            "结束这场对话，保住最后一点体面",
            "短句、避开称呼、用动作代替回答",
            "别再看我；我已经撑不住了",
        ),
        ActionTendency::Appease | ActionTendency::Help => (
            "让对方消气/留下/需要我",
            "先退一步、替对方找理由、主动做事",
            "只要你别走，我怎样都可以",
        ),
        ActionTendency::Confess => (
            "把真相说出来，换取一次被理解的机会",
            "先承认事实，再解释动机，不为自己开脱",
            "我怕你恨我，但我更怕一直骗你",
        ),
        ActionTendency::Conceal => (
            "把真正的原因藏起来，维持表面的平静",
            "转移话题、用玩笑带过、只承认无关紧要的部分",
            "如果我说了，你会更看不起我",
        ),
        ActionTendency::SeekContact | ActionTendency::Approach => (
            "确认对方还在，确认关系没有断",
            "找一个具体的由头靠近、问一个看似无关的问题",
            "你刚才那句话，是不是要走了",
        ),
        ActionTendency::Sacrifice => (
            "替对方承担代价，不让对方知道",
            "把选择说成自己的、把后果揽过来",
            "你活着，比我被记住更重要",
        ),
        ActionTendency::Betray => (
            "用对方的信任换取自己的出路",
            "半真半假地交代、把责任推给第三方",
            "对不起，但我没有别的路",
        ),
    };
    DialoguePolicy {
        objective: objective.into(),
        tactic: tactic.into(),
        subtext: subtext.into(),
        forbidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn betrayal_produces_anger_and_shame() {
        let s = sensitivities_from("童年被师父背叛", "渴望被认可", "表面冷漠");
        let a = appraise_event(EventKind::Betrayal, Some("师父"), &s);
        let v = emotion_from_appraisal(EventKind::Betrayal, &a, &s);
        assert!(v.anger > 0.2);
        assert!(v.shame > 0.1);
        assert!(a.norm_violation > 0.5);
    }

    #[test]
    fn loss_produces_sadness_and_love() {
        let s = sensitivities_from("母亲惨死", "渴望归属", "压抑的悲愤");
        let a = appraise_event(EventKind::Loss, None, &s);
        let v = emotion_from_appraisal(EventKind::Loss, &a, &s);
        assert!(v.sadness > v.anger);
        assert!(v.love > 0.0);
    }

    #[test]
    fn shame_character_attacks_when_defense_is_attack() {
        let v = EmotionVector::from_pairs(&[("shame", 0.7), ("anger", 0.3)]);
        let (t, _) = action_tendency(&v, "攻击型：先推开再确认");
        assert_eq!(t, ActionTendency::Attack);
    }

    #[test]
    fn shame_character_conceals_without_attack_defense() {
        let v = EmotionVector::from_pairs(&[("shame", 0.8), ("fear", 0.2)]);
        let (t, _) = action_tendency(&v, "回避型");
        assert_eq!(t, ActionTendency::Conceal);
    }

    #[test]
    fn guilt_self_agent_confesses() {
        let v = EmotionVector::from_pairs(&[("guilt", 0.8), ("sadness", 0.2)]);
        let (t, _) = action_tendency(&v, "");
        assert_eq!(t, ActionTendency::Confess);
    }

    #[test]
    fn dialogue_policy_carries_forbidden_lines() {
        let p = dialogue_policy(ActionTendency::Conceal, vec!["谢谢".into()]);
        assert!(p.forbidden.contains(&"谢谢".into()));
        assert!(!p.subtext.is_empty());
    }
}
