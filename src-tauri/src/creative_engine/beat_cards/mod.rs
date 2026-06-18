//! Beat Cards —— 经典叙事功能桥段卡库
//!
//! 把流传已久的戏剧/章回/网文桥段抽象成"可复用功能 + 重构提示"的卡片，
//! 供 LLM 在大纲与正文阶段挑选 1-2 张作为骨架，避免直接复刻具体作品。
//!
//! 设计原则：
//! - 全部使用通用中文名，不绑定特定作品。
//! - 每张卡只保留"功能 + 何时使用 + 重构提示 + 反例"四要素。
//! - 卡片在 SQLite 持久化（V092 Migration），允许用户追加自定义卡。
//! - 通过 `strategy::AssetKind::BeatCard` 进入 StrategySelector 的 LLM 路由。

use serde::{Deserialize, Serialize};

mod registry;

pub use registry::builtin_beat_cards;

/// 桥段卡分类（按主要叙事作用归档）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeatCardCategory {
    /// 跌落与回归
    DownfallAndComeback,
    /// 公开证明与打脸
    PublicProofAndFaceSlap,
    /// 身份与识别
    IdentityAndRecognition,
    /// 悬疑与真相重构
    SuspenseAndTruthReframe,
    /// 情感拉扯
    RomanceAndEmotionalPull,
    /// 制度与规则压力
    SystemAndBureaucraticPressure,
    /// 后台视角与组织讽刺
    BackstageAndOrganizationSatire,
}

impl BeatCardCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            BeatCardCategory::DownfallAndComeback => "downfall_and_comeback",
            BeatCardCategory::PublicProofAndFaceSlap => "public_proof_and_face_slap",
            BeatCardCategory::IdentityAndRecognition => "identity_and_recognition",
            BeatCardCategory::SuspenseAndTruthReframe => "suspense_and_truth_reframe",
            BeatCardCategory::RomanceAndEmotionalPull => "romance_and_emotional_pull",
            BeatCardCategory::SystemAndBureaucraticPressure => "system_and_bureaucratic_pressure",
            BeatCardCategory::BackstageAndOrganizationSatire => "backstage_and_organization_satire",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BeatCardCategory::DownfallAndComeback => "跌落与回归",
            BeatCardCategory::PublicProofAndFaceSlap => "公开证明与打脸",
            BeatCardCategory::IdentityAndRecognition => "身份与识别",
            BeatCardCategory::SuspenseAndTruthReframe => "悬疑与真相重构",
            BeatCardCategory::RomanceAndEmotionalPull => "情感拉扯",
            BeatCardCategory::SystemAndBureaucraticPressure => "制度与规则压力",
            BeatCardCategory::BackstageAndOrganizationSatire => "后台视角与组织讽刺",
        }
    }
}

/// 单张桥段卡
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatCard {
    pub id: String,
    pub name: String,
    pub category: BeatCardCategory,
    /// 这张卡完成的叙事任务（function = 可复用功能）
    pub function: String,
    /// 适用场景
    pub when_to_use: String,
    /// 重构提示（写作时如何避免雷同原型）
    pub remix_hint: String,
    /// 反例（什么写法会让这张卡变味）
    pub avoid: String,
    /// 标签（题材关联 / 适合的爽点）
    pub tags: Vec<String>,
}

impl BeatCard {
    /// 注入 LLM prompt 的简洁单行表述
    pub fn to_prompt_line(&self) -> String {
        format!(
            "- {} [{}]: {} | 适用：{} | 重构：{}",
            self.name,
            self.category.label(),
            self.function,
            self.when_to_use,
            self.remix_hint
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_round_trip() {
        for cat in [
            BeatCardCategory::DownfallAndComeback,
            BeatCardCategory::PublicProofAndFaceSlap,
            BeatCardCategory::IdentityAndRecognition,
            BeatCardCategory::SuspenseAndTruthReframe,
            BeatCardCategory::RomanceAndEmotionalPull,
            BeatCardCategory::SystemAndBureaucraticPressure,
            BeatCardCategory::BackstageAndOrganizationSatire,
        ] {
            assert!(!cat.as_str().is_empty());
            assert!(!cat.label().is_empty());
        }
    }

    #[test]
    fn builtin_cards_have_minimum_count() {
        let cards = builtin_beat_cards();
        assert!(
            cards.len() >= 30,
            "v0.17.0 至少应有 30 张内置桥段卡，实际 {}",
            cards.len()
        );
    }

    #[test]
    fn builtin_cards_unique_ids() {
        let cards = builtin_beat_cards();
        let mut ids: Vec<&str> = cards.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), cards.len(), "桥段卡 id 必须唯一");
    }

    #[test]
    fn prompt_line_contains_essentials() {
        let cards = builtin_beat_cards();
        let line = cards[0].to_prompt_line();
        assert!(line.contains(&cards[0].name));
        assert!(line.contains(cards[0].category.label()));
    }
}
