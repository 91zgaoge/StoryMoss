//! 12 维情绪向量。
//!
//! 六种基本情绪（Ekman）+ 六种社会情绪。向量权重和归一化为 1，强度单独
//! 存储，避免「强度」被向量长度重复表达。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmotionKind {
    Joy,
    Sadness,
    Fear,
    Anger,
    Disgust,
    Surprise,
    Love,
    Trust,
    Guilt,
    Shame,
    Pride,
    Hope,
}

impl EmotionKind {
    pub const ALL: [EmotionKind; 12] = [
        Self::Joy,
        Self::Sadness,
        Self::Fear,
        Self::Anger,
        Self::Disgust,
        Self::Surprise,
        Self::Love,
        Self::Trust,
        Self::Guilt,
        Self::Shame,
        Self::Pride,
        Self::Hope,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Joy => "joy",
            Self::Sadness => "sadness",
            Self::Fear => "fear",
            Self::Anger => "anger",
            Self::Disgust => "disgust",
            Self::Surprise => "surprise",
            Self::Love => "love",
            Self::Trust => "trust",
            Self::Guilt => "guilt",
            Self::Shame => "shame",
            Self::Pride => "pride",
            Self::Hope => "hope",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Joy => "喜悦",
            Self::Sadness => "悲伤",
            Self::Fear => "恐惧",
            Self::Anger => "愤怒",
            Self::Disgust => "厌恶",
            Self::Surprise => "惊讶",
            Self::Love => "爱",
            Self::Trust => "信任",
            Self::Guilt => "愧疚",
            Self::Shame => "羞耻",
            Self::Pride => "自尊",
            Self::Hope => "希望",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// 是否为高唤醒情绪（用于 arousal 估算）。
    pub fn is_high_arousal(self) -> bool {
        matches!(
            self,
            Self::Anger | Self::Fear | Self::Surprise | Self::Shame | Self::Joy | Self::Pride
        )
    }

    /// 正负极性：+1 正、-1 负、0 中性。
    pub fn polarity(self) -> f32 {
        match self {
            Self::Joy | Self::Love | Self::Trust | Self::Pride | Self::Hope => 1.0,
            Self::Sadness
            | Self::Fear
            | Self::Anger
            | Self::Disgust
            | Self::Guilt
            | Self::Shame => -1.0,
            Self::Surprise => 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmotionVector {
    #[serde(default)]
    pub joy: f32,
    #[serde(default)]
    pub sadness: f32,
    #[serde(default)]
    pub fear: f32,
    #[serde(default)]
    pub anger: f32,
    #[serde(default)]
    pub disgust: f32,
    #[serde(default)]
    pub surprise: f32,
    #[serde(default)]
    pub love: f32,
    #[serde(default)]
    pub trust: f32,
    #[serde(default)]
    pub guilt: f32,
    #[serde(default)]
    pub shame: f32,
    #[serde(default)]
    pub pride: f32,
    #[serde(default)]
    pub hope: f32,
}

impl EmotionVector {
    pub fn get(&self, kind: EmotionKind) -> f32 {
        match kind {
            EmotionKind::Joy => self.joy,
            EmotionKind::Sadness => self.sadness,
            EmotionKind::Fear => self.fear,
            EmotionKind::Anger => self.anger,
            EmotionKind::Disgust => self.disgust,
            EmotionKind::Surprise => self.surprise,
            EmotionKind::Love => self.love,
            EmotionKind::Trust => self.trust,
            EmotionKind::Guilt => self.guilt,
            EmotionKind::Shame => self.shame,
            EmotionKind::Pride => self.pride,
            EmotionKind::Hope => self.hope,
        }
    }

    pub fn set(&mut self, kind: EmotionKind, value: f32) {
        let value = value.max(0.0);
        match kind {
            EmotionKind::Joy => self.joy = value,
            EmotionKind::Sadness => self.sadness = value,
            EmotionKind::Fear => self.fear = value,
            EmotionKind::Anger => self.anger = value,
            EmotionKind::Disgust => self.disgust = value,
            EmotionKind::Surprise => self.surprise = value,
            EmotionKind::Love => self.love = value,
            EmotionKind::Trust => self.trust = value,
            EmotionKind::Guilt => self.guilt = value,
            EmotionKind::Shame => self.shame = value,
            EmotionKind::Pride => self.pride = value,
            EmotionKind::Hope => self.hope = value,
        }
    }

    pub fn from_pairs(pairs: &[(&str, f32)]) -> Self {
        let mut v = Self::default();
        for (id, value) in pairs {
            if let Some(kind) = EmotionKind::from_id(id) {
                let old = v.get(kind);
                v.set(kind, old + value.max(0.0));
            }
        }
        v.normalized()
    }

    pub fn sum(&self) -> f32 {
        EmotionKind::ALL.iter().map(|k| self.get(*k)).sum()
    }

    pub fn is_zero(&self) -> bool {
        self.sum() <= f32::EPSILON
    }

    pub fn normalized(&self) -> Self {
        let sum = self.sum();
        if sum <= f32::EPSILON {
            return Self::default();
        }
        let mut out = self.clone();
        for kind in EmotionKind::ALL {
            out.set(kind, self.get(kind) / sum);
        }
        out
    }

    /// 主导情绪；空向量返回 (Joy, 0)。
    pub fn dominant(&self) -> (EmotionKind, f32) {
        let mut best = (EmotionKind::Joy, 0.0f32);
        for kind in EmotionKind::ALL {
            let v = self.get(kind);
            if v > best.1 {
                best = (kind, v);
            }
        }
        best
    }

    pub fn top_k(&self, n: usize) -> Vec<(EmotionKind, f32)> {
        let mut items: Vec<(EmotionKind, f32)> = EmotionKind::ALL
            .iter()
            .map(|k| (*k, self.get(*k)))
            .collect();
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        items.truncate(n);
        items
    }

    pub fn valence(&self) -> f32 {
        let v: f32 = EmotionKind::ALL
            .iter()
            .map(|k| self.get(*k) * k.polarity())
            .sum();
        v.clamp(-1.0, 1.0)
    }

    pub fn arousal(&self) -> f32 {
        let high: f32 = EmotionKind::ALL
            .iter()
            .filter(|k| k.is_high_arousal())
            .map(|k| self.get(*k))
            .sum();
        high.clamp(0.0, 1.0)
    }

    fn norm(&self) -> f32 {
        self.sum_sq().sqrt()
    }

    fn sum_sq(&self) -> f32 {
        EmotionKind::ALL
            .iter()
            .map(|k| {
                let v = self.get(*k);
                v * v
            })
            .sum()
    }

    /// 1 - cosine。空向量与空向量距离 0，空与非空距离 1。
    pub fn distance(&self, other: &Self) -> f32 {
        let a = self.norm();
        let b = other.norm();
        if a <= f32::EPSILON && b <= f32::EPSILON {
            return 0.0;
        }
        if a <= f32::EPSILON || b <= f32::EPSILON {
            return 1.0;
        }
        let dot: f32 = EmotionKind::ALL
            .iter()
            .map(|k| self.get(*k) * other.get(*k))
            .sum();
        (1.0 - (dot / (a * b)).clamp(-1.0, 1.0)).clamp(0.0, 2.0)
    }

    /// 从 self 到 other 的差分向量（other - self，负值截为 0）。
    pub fn delta(&self, other: &Self) -> Self {
        let mut out = Self::default();
        for kind in EmotionKind::ALL {
            out.set(kind, (other.get(kind) - self.get(kind)).max(0.0));
        }
        out
    }

    pub fn blend(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut out = Self::default();
        for kind in EmotionKind::ALL {
            out.set(kind, self.get(kind) * (1.0 - t) + other.get(kind) * t);
        }
        out.normalized()
    }

    /// 紧凑渲染，例如「羞耻0.60/愤怒0.30」。
    pub fn render_compact(&self, n: usize) -> String {
        let top = self.top_k(n);
        if top.is_empty() || top[0].1 <= 0.02 {
            return "平静".into();
        }
        top.into_iter()
            .filter(|(_, v)| *v > 0.02)
            .map(|(k, v)| format!("{}{:.2}", k.label(), v))
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn render_delta(&self, other: &Self, n: usize) -> String {
        format!("{} → {}", self.render_compact(n), other.render_compact(n))
    }

    /// 确定性中文词典提取（0 LLM）。只用于探针与回退，不用于精确语义。
    pub fn from_text(text: &str) -> Self {
        const JOY: &[&str] = &["高兴", "欢喜", "雀跃", "欣慰", "开心", "快活", "笑", "喜"];
        const SADNESS: &[&str] = &[
            "难过", "失落", "悲恸", "怅然", "悲伤", "伤心", "哭", "泪", "呜咽", "酸楚",
        ];
        const FEAR: &[&str] = &[
            "害怕", "恐惧", "不安", "发冷", "发抖", "心惊", "畏惧", "逃", "躲", "慌",
        ];
        const ANGER: &[&str] = &[
            "愤怒", "恼火", "恨", "暴怒", "生气", "怒", "咬牙", "攥紧", "质问", "逼",
        ];
        const DISGUST: &[&str] = &["厌恶", "恶心", "鄙夷", "嫌弃", "作呕", "不屑", "嗤"];
        const SURPRISE: &[&str] = &["惊讶", "错愕", "震惊", "意外", "愣", "怔", "没想到"];
        const LOVE: &[&str] = &[
            "爱",
            "心疼",
            "眷恋",
            "舍不得",
            "喜欢",
            "牵挂",
            "想念",
            "温柔",
            "护",
        ];
        const TRUST: &[&str] = &["信任", "依赖", "安心", "托付", "可靠", "放心", "相信"];
        const GUILT: &[&str] = &[
            "愧疚",
            "内疚",
            "亏欠",
            "悔",
            "对不起",
            "抱歉",
            "自责",
            "补偿",
        ];
        const SHAME: &[&str] = &[
            "羞耻",
            "难堪",
            "丢脸",
            "无地自容",
            "羞愧",
            "脸红",
            "窘",
            "抬不起头",
        ];
        const PRIDE: &[&str] = &["骄傲", "自尊", "体面", "扬眉", "争气", "不屑于"];
        const HOPE: &[&str] = &["希望", "盼", "期待", "侥幸", "也许", "总会", "等"];

        let lexicons: [(EmotionKind, &[&str]); 12] = [
            (EmotionKind::Joy, JOY),
            (EmotionKind::Sadness, SADNESS),
            (EmotionKind::Fear, FEAR),
            (EmotionKind::Anger, ANGER),
            (EmotionKind::Disgust, DISGUST),
            (EmotionKind::Surprise, SURPRISE),
            (EmotionKind::Love, LOVE),
            (EmotionKind::Trust, TRUST),
            (EmotionKind::Guilt, GUILT),
            (EmotionKind::Shame, SHAME),
            (EmotionKind::Pride, PRIDE),
            (EmotionKind::Hope, HOPE),
        ];
        let mut v = Self::default();
        for (kind, words) in lexicons {
            let mut count = 0.0f32;
            for w in words {
                count += text.matches(w).count() as f32;
            }
            if count > 0.0 {
                v.set(kind, count);
            }
        }
        v.normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_sums_to_one() {
        let v = EmotionVector::from_pairs(&[("shame", 3.0), ("anger", 1.0)]);
        assert!((v.sum() - 1.0).abs() < 1e-5);
        assert_eq!(v.dominant().0, EmotionKind::Shame);
    }

    #[test]
    fn empty_vector_has_zero_distance_to_itself() {
        let v = EmotionVector::default();
        assert_eq!(v.distance(&v), 0.0);
        assert!(v.is_zero());
    }

    #[test]
    fn opposite_vectors_have_distance_near_one() {
        let a = EmotionVector::from_pairs(&[("joy", 1.0)]);
        let b = EmotionVector::from_pairs(&[("sadness", 1.0)]);
        assert!(a.distance(&b) > 0.99);
    }

    #[test]
    fn from_text_extracts_shame_and_anger() {
        let v = EmotionVector::from_text("她羞耻得无地自容，难堪得抬不起头，又愤怒地攥紧了手。");
        assert_eq!(v.dominant().0, EmotionKind::Shame);
        assert!(v.anger > 0.0);
    }

    #[test]
    fn blend_moves_toward_target() {
        let a = EmotionVector::from_pairs(&[("fear", 1.0)]);
        let b = EmotionVector::from_pairs(&[("anger", 1.0)]);
        let mid = a.blend(&b, 0.5);
        assert!(mid.fear > 0.0 && mid.anger > 0.0);
    }

    #[test]
    fn valence_and_arousal_are_bounded() {
        let v = EmotionVector::from_pairs(&[("fear", 0.6), ("shame", 0.4)]);
        assert!(v.valence() <= 0.0);
        assert!(v.arousal() > 0.0 && v.arousal() <= 1.0);
    }
}
