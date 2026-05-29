//! 变更检测器
//!
//! 在实体更新 command 中调用，记录变更事件并判断是否需要触发级联改写。

use super::models::EntityChangeEvent;

/// 正文敏感字段白名单
const SENSITIVE_FIELDS: &[(&str, &[&str])] = &[
    (
        "character",
        &[
            "personality",
            "motivation",
            "goal",
            "appearance",
            "relationships",
        ],
    ),
    (
        "world_building",
        &["rules", "history", "geography", "magic_system"],
    ),
    ("foreshadowing", &["status", "payoff_plan"]),
];

pub struct ChangeDetector;

impl ChangeDetector {
    /// 判断变更是否需要触发级联改写
    pub fn should_trigger(change: &EntityChangeEvent) -> bool {
        let sensitive_fields = SENSITIVE_FIELDS
            .iter()
            .find(|(et, _)| *et == change.entity_type.as_str())
            .map(|(_, fields)| *fields)
            .unwrap_or(&[]);

        if sensitive_fields.is_empty() {
            return false;
        }

        change
            .changed_fields
            .iter()
            .any(|f| sensitive_fields.contains(&f.as_str()))
    }
}
