//! 表达载体（Prose Carriers）：论文局部稀疏组件在小说层的对应物。
//!
//! 每拍只选 2–3 个载体；情感必须落到动作或选择，不能八项全写。

use serde::{Deserialize, Serialize};

use super::vector::EmotionVector;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Carrier {
    Action,
    DialogueSubtext,
    Choice,
    Consequence,
    Sensory,
    Interiority,
    Silence,
    Rhythm,
}

impl Carrier {
    pub const ALL: [Carrier; 8] = [
        Self::Action,
        Self::DialogueSubtext,
        Self::Choice,
        Self::Consequence,
        Self::Sensory,
        Self::Interiority,
        Self::Silence,
        Self::Rhythm,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::DialogueSubtext => "dialogue_subtext",
            Self::Choice => "choice",
            Self::Consequence => "consequence",
            Self::Sensory => "sensory",
            Self::Interiority => "interiority",
            Self::Silence => "silence",
            Self::Rhythm => "rhythm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Action => "动作",
            Self::DialogueSubtext => "台词潜台词",
            Self::Choice => "选择",
            Self::Consequence => "代价/后果",
            Self::Sensory => "具体感官",
            Self::Interiority => "内心活动",
            Self::Silence => "沉默/留白",
            Self::Rhythm => "节奏",
        }
    }

    pub fn guidance(self) -> &'static str {
        match self {
            Self::Action => "写一个能被看见的具体动作；不要用「她很难过」概括。",
            Self::DialogueSubtext => "台词表面说 A、实际要 B；写目标、手段与潜台词，不解释。",
            Self::Choice => "让角色在两个具体代价之间做一次选择；写出他放弃了什么。",
            Self::Consequence => "写出这个选择的不可逆后果：谁失去了什么、谁欠了谁。",
            Self::Sensory => "只用一个具体感官细节（声音/温度/气味/触感），不堆三感。",
            Self::Interiority => "限量的内心活动；用反问或自我欺骗，不写作者总结。",
            Self::Silence => "用停顿、短句成段、动作中断代替解释；留白给读者。",
            Self::Rhythm => "用句长变化制造压力：紧张处短句连发，崩溃处允许一句成段。",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CarrierPlan {
    #[serde(default)]
    pub action: f32,
    #[serde(default)]
    pub dialogue_subtext: f32,
    #[serde(default)]
    pub choice: f32,
    #[serde(default)]
    pub consequence: f32,
    #[serde(default)]
    pub sensory: f32,
    #[serde(default)]
    pub interiority: f32,
    #[serde(default)]
    pub silence: f32,
    #[serde(default)]
    pub rhythm: f32,
}

impl CarrierPlan {
    pub fn get(&self, carrier: Carrier) -> f32 {
        match carrier {
            Carrier::Action => self.action,
            Carrier::DialogueSubtext => self.dialogue_subtext,
            Carrier::Choice => self.choice,
            Carrier::Consequence => self.consequence,
            Carrier::Sensory => self.sensory,
            Carrier::Interiority => self.interiority,
            Carrier::Silence => self.silence,
            Carrier::Rhythm => self.rhythm,
        }
    }

    pub fn set(&mut self, carrier: Carrier, value: f32) {
        let value = value.max(0.0);
        match carrier {
            Carrier::Action => self.action = value,
            Carrier::DialogueSubtext => self.dialogue_subtext = value,
            Carrier::Choice => self.choice = value,
            Carrier::Consequence => self.consequence = value,
            Carrier::Sensory => self.sensory = value,
            Carrier::Interiority => self.interiority = value,
            Carrier::Silence => self.silence = value,
            Carrier::Rhythm => self.rhythm = value,
        }
    }

    pub fn sum(&self) -> f32 {
        Carrier::ALL.iter().map(|c| self.get(*c)).sum()
    }

    pub fn normalized(&self) -> Self {
        let sum = self.sum();
        if sum <= f32::EPSILON {
            return Self::default();
        }
        let mut out = self.clone();
        for c in Carrier::ALL {
            out.set(c, self.get(c) / sum);
        }
        out
    }

    pub fn top_k(&self, n: usize) -> Vec<(Carrier, f32)> {
        let mut items: Vec<(Carrier, f32)> =
            Carrier::ALL.iter().map(|c| (*c, self.get(*c))).collect();
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        items.truncate(n);
        items
    }

    /// 只保留前 3 项，且必须含动作或选择。
    pub fn sparse(mut self) -> Self {
        let mut top = self.top_k(3);
        if top.iter().all(|(_, v)| *v <= f32::EPSILON) {
            self = Self::default();
            self.set(Carrier::Action, 1.0);
            return self;
        }
        let has_anchor = top
            .iter()
            .any(|(c, _)| matches!(c, Carrier::Action | Carrier::Choice));
        if !has_anchor {
            let anchor = Carrier::Action;
            if let Some(last) = top.last_mut() {
                *last = (anchor, 0.15);
            }
        }
        let mut out = Self::default();
        for (c, v) in top {
            out.set(c, v);
        }
        out.normalized()
    }

    pub fn top_carriers(&self, n: usize) -> Vec<Carrier> {
        self.top_k(n)
            .into_iter()
            .filter(|(_, v)| *v > 0.02)
            .map(|(c, _)| c)
            .collect()
    }

    /// 提示词块：只列选中的载体。
    pub fn render(&self) -> String {
        let top = self.top_carriers(3);
        if top.is_empty() {
            return "载体：动作（必须）".into();
        }
        top.iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}（必须）：{}", i + 1, c.label(), c.guidance()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.sum() <= f32::EPSILON
    }
}

/// 按情绪、防御、场合与场型分配载体，再稀疏化。
pub fn select_carriers(
    emotion: &EmotionVector,
    defense: &str,
    public: bool,
    action_scene: bool,
) -> CarrierPlan {
    use super::vector::EmotionKind;
    let (dominant, _) = emotion.dominant();
    let mut p = CarrierPlan::default();
    let base: [(Carrier, f32); 3] = match dominant {
        EmotionKind::Anger => [
            (Carrier::Action, 0.45),
            (Carrier::DialogueSubtext, 0.30),
            (Carrier::Choice, 0.25),
        ],
        EmotionKind::Fear => [
            (Carrier::Sensory, 0.40),
            (Carrier::Action, 0.30),
            (Carrier::Silence, 0.30),
        ],
        EmotionKind::Sadness => [
            (Carrier::Silence, 0.40),
            (Carrier::Consequence, 0.30),
            (Carrier::Sensory, 0.30),
        ],
        EmotionKind::Shame => [
            (Carrier::DialogueSubtext, 0.45),
            (Carrier::Silence, 0.30),
            (Carrier::Interiority, 0.25),
        ],
        EmotionKind::Love => [
            (Carrier::Action, 0.40),
            (Carrier::DialogueSubtext, 0.35),
            (Carrier::Choice, 0.25),
        ],
        EmotionKind::Guilt => [
            (Carrier::Consequence, 0.40),
            (Carrier::Action, 0.30),
            (Carrier::DialogueSubtext, 0.30),
        ],
        EmotionKind::Hope => [
            (Carrier::Choice, 0.40),
            (Carrier::Action, 0.30),
            (Carrier::DialogueSubtext, 0.30),
        ],
        EmotionKind::Pride => [
            (Carrier::Action, 0.40),
            (Carrier::DialogueSubtext, 0.35),
            (Carrier::Choice, 0.25),
        ],
        EmotionKind::Disgust => [
            (Carrier::DialogueSubtext, 0.40),
            (Carrier::Action, 0.35),
            (Carrier::Sensory, 0.25),
        ],
        EmotionKind::Surprise => [
            (Carrier::Rhythm, 0.40),
            (Carrier::Silence, 0.30),
            (Carrier::Action, 0.30),
        ],
        EmotionKind::Trust => [
            (Carrier::Action, 0.40),
            (Carrier::DialogueSubtext, 0.35),
            (Carrier::Choice, 0.25),
        ],
        EmotionKind::Joy => [
            (Carrier::Action, 0.40),
            (Carrier::DialogueSubtext, 0.35),
            (Carrier::Rhythm, 0.25),
        ],
    };
    for (c, v) in base {
        p.set(c, v);
    }
    let defense = defense.to_string();
    if defense.contains("攻击") || defense.contains("反击") || defense.contains("爆发") {
        p.set(Carrier::Action, p.action + 0.10);
        p.set(Carrier::DialogueSubtext, p.dialogue_subtext + 0.10);
    } else if defense.contains("回避") || defense.contains("逃") {
        p.set(Carrier::Silence, p.silence + 0.15);
        p.set(Carrier::Interiority, p.interiority + 0.10);
    } else if defense.contains("讨好") || defense.contains("顺从") {
        p.set(Carrier::DialogueSubtext, p.dialogue_subtext + 0.12);
        p.set(Carrier::Action, p.action + 0.08);
    } else if defense.contains("理智") || defense.contains("分析") {
        p.set(Carrier::DialogueSubtext, p.dialogue_subtext + 0.10);
        p.set(Carrier::Interiority, p.interiority + 0.12);
    }
    if public {
        p.set(Carrier::Interiority, p.interiority * 0.5);
        p.set(Carrier::DialogueSubtext, p.dialogue_subtext * 1.2);
        p.set(Carrier::Silence, p.silence * 1.1);
    }
    if action_scene {
        p.set(Carrier::Action, p.action * 1.2);
        p.set(Carrier::Consequence, p.consequence * 1.15);
        p.set(Carrier::Rhythm, p.rhythm * 1.1);
        p.set(Carrier::Interiority, p.interiority * 0.6);
    }
    p.sparse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion_circuit::vector::EmotionVector;

    #[test]
    fn sparse_keeps_three_and_contains_anchor() {
        let v = EmotionVector::from_pairs(&[("shame", 0.7), ("fear", 0.3)]);
        let p = select_carriers(&v, "回避型", true, false);
        let top = p.top_carriers(8);
        assert!(top.len() <= 3, "top={top:?}");
        assert!(top
            .iter()
            .any(|c| matches!(c, Carrier::Action | Carrier::Choice)));
    }

    #[test]
    fn attack_defense_pushes_action_and_dialogue() {
        let v = EmotionVector::from_pairs(&[("shame", 0.8)]);
        let p = select_carriers(&v, "攻击型", false, false);
        assert!(p.action > 0.0);
        assert!(p.dialogue_subtext > 0.0);
    }

    #[test]
    fn public_scene_reduces_interiority() {
        let v = EmotionVector::from_pairs(&[("shame", 0.8)]);
        let private = select_carriers(&v, "", false, false);
        let public = select_carriers(&v, "", true, false);
        assert!(public.interiority <= private.interiority);
    }

    #[test]
    fn render_only_lists_selected_carriers() {
        let v = EmotionVector::from_pairs(&[("anger", 1.0)]);
        let p = select_carriers(&v, "", false, false);
        let text = p.render();
        assert!(text.contains("动作"));
        assert!(text.lines().count() <= 3);
    }

    #[test]
    fn empty_plan_renders_action_fallback() {
        let p = CarrierPlan::default();
        assert!(p.render().contains("动作"));
    }
}
