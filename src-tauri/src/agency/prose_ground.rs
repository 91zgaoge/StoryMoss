//! 有正文时资产/大纲必须接地。0 LLM。
//! 设计：docs/plans/2026-08-16-prose-grounded-outline-design.md

use crate::agency::continue_assets::{match_character_names, strip_editor_markup};

pub const DEFAULT_METHODOLOGY_ID: &str = "scene_structure";
pub const SUBSTANTIAL_PROSE_CHARS: usize = 200;
pub const STORY_INFO_PROSE_CHARS: usize = 800;

pub fn has_substantial_prose(text: &str) -> bool {
    strip_editor_markup(text).chars().count() >= SUBSTANTIAL_PROSE_CHARS
}

pub fn resolve_methodology_id(existing: Option<&str>) -> &str {
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => DEFAULT_METHODOLOGY_ID,
    }
}

pub fn filter_names_to_prose(names: &[impl AsRef<str>], prose: &str) -> Vec<String> {
    let plain = strip_editor_markup(prose);
    match_character_names(names, &plain)
}

pub fn name_in_prose(name: &str, prose: &str) -> bool {
    !filter_names_to_prose(&[name], prose).is_empty()
}

/// `candidate_names` 中出现在大纲里的姓名必须也出现在正文。
/// 大纲未点任何候选名 → 视为接地（避免空大纲误杀）。
pub fn outline_is_grounded(
    outline: &str,
    prose: &str,
    candidate_names: &[impl AsRef<str>],
) -> bool {
    let mentioned: Vec<String> = match_character_names(candidate_names, outline);
    if mentioned.is_empty() {
        return true;
    }
    mentioned.iter().all(|n| name_in_prose(n, prose))
}

/// 通用兜底「下一拍」句（方法论模板，不含本故事的具体节点）。
///
/// v0.64.9 真机事故：这类兜底句被写进 `scenes.outline_content` 的「下一拍：」
/// 槽位后又被 `next_node_from_scene_outline` 当作计划采纳，逐拍滚动复制——
/// 章节方向永远停在「按场景结构推进」，模型只能原地重演（第 11 章整场重演
/// 第 10 章）。兜底句只允许当**本拍**提示，不得落库、不得被下一拍采信。
pub fn is_generic_next_node(node: &str) -> bool {
    const GENERIC_PREFIXES: &[&str] = &[
        "按场景结构推进",
        "末句已是灾难。用场景结构写本场",
        "在硬约束内把当前冲突推进一步",
    ];
    let t = node.trim();
    !t.is_empty() && GENERIC_PREFIXES.iter().any(|p| t.starts_with(p))
}

pub fn methodology_next_node(
    methodology_id: &str,
    shot: &str,
    present: &[impl AsRef<str>],
) -> String {
    let names = present
        .iter()
        .map(|n| n.as_ref())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join("、");
    let cast = if names.is_empty() {
        "本场仍在场者".to_string()
    } else {
        names
    };
    let disaster = ["气绝", "刺", "死", "崩裂", "败", "灾难", "短刃"]
        .iter()
        .any(|s| shot.contains(s));
    match methodology_id {
        "scene_structure" if disaster => format!(
            "末句已是灾难。用场景结构写本场{cast}的反应、困境与决定，不得换场、不得换主角。"
        ),
        "scene_structure" => {
            format!(
                "按场景结构推进：目标→冲突→灾难或反应→困境→决定。只写本场{cast}，不得另起开篇。"
            )
        }
        _ => format!("在硬约束内把当前冲突推进一步，只写本场{cast}，不得原地复述末句。"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROSE: &str = "知启纪元八百四十七年。大奉帝国西北边陲重镇，黑崎州城。\
第二代镇北王苏会山端坐大堂。大少爷苏亦铁红装肃立。";

    #[test]
    fn substantial_prose_threshold() {
        assert!(!has_substantial_prose("短"));
        assert!(has_substantial_prose(&"字".repeat(200)));
    }

    #[test]
    fn title_inventions_dropped_when_absent_from_prose() {
        let names = ["费迪南三世", "艾拉", "苏会山", "苏亦铁"];
        let kept = filter_names_to_prose(&names, PROSE);
        assert!(kept.contains(&"苏会山".into()));
        assert!(kept.contains(&"苏亦铁".into()));
        assert!(!kept.iter().any(|n| n.contains("费迪南")));
        assert!(!kept.iter().any(|n| n == "艾拉"));
    }

    #[test]
    fn ferdinand_outline_is_not_grounded() {
        let outline = "第一卷·灰烬低语。费迪南三世为撑烟火节加征火药税。艾拉偷入工坊。";
        let candidates = ["费迪南三世", "艾拉", "苏会山"];
        assert!(!outline_is_grounded(outline, PROSE, &candidates));
    }

    #[test]
    fn su_family_outline_is_grounded() {
        let outline = "【转折点】景亲王送女，苏会山在镇北王府大堂迎亲。";
        let candidates = ["苏会山", "费迪南三世"];
        assert!(outline_is_grounded(outline, PROSE, &candidates));
    }

    #[test]
    fn default_methodology_is_scene_structure() {
        assert_eq!(DEFAULT_METHODOLOGY_ID, "scene_structure");
        assert_eq!(resolve_methodology_id(None), "scene_structure");
        assert_eq!(resolve_methodology_id(Some("hero_journey")), "hero_journey");
        assert_eq!(resolve_methodology_id(Some("custom_foo")), "custom_foo");
        assert_eq!(resolve_methodology_id(Some("  ")), "scene_structure");
    }

    #[test]
    fn scene_structure_next_beat_after_disaster_stays_in_shot() {
        let shot = "公主短刃扎进苏会山胸口。苏会山头脸崩裂，气绝。苏亦铁跪在红毡上。";
        let present = ["苏亦铁", "曹元佩"];
        let node = methodology_next_node("scene_structure", shot, &present);
        assert!(
            node.contains("苏亦铁") || node.contains("反应"),
            "灾难后须留在本场写反应, got={node}"
        );
        assert!(
            !node.contains("费迪南"),
            "不得把书名发明的 POV 写进下一拍, got={node}"
        );
    }

    #[test]
    fn generic_next_node_is_recognized_but_real_plan_is_not() {
        // v0.64.9：方法论兜底句不得被当成计划（真机第 11-13 章逐章同句）
        assert!(is_generic_next_node(
            "按场景结构推进：目标→冲突→灾难或反应→困境→决定。只写本场甲、乙，不得另起开篇。"
        ));
        assert!(is_generic_next_node(
            "末句已是灾难。用场景结构写本场甲的反应、困境与决定，不得换场、不得换主角。"
        ));
        assert!(is_generic_next_node(
            "在硬约束内把当前冲突推进一步，只写本场甲，不得原地复述末句。"
        ));
        assert!(!is_generic_next_node(
            "名册交到御使行辕，曹元佩当众烧掉三分之二"
        ));
        assert!(!is_generic_next_node(""));
    }
}
