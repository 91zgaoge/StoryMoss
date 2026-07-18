//! 质量门 v1：规则问题归并与规则复检上下文构建。
//! 门判定逻辑（evaluate_gate）在 coordinator.rs；Task 4/6 复用同一门径。

use crate::{
    agents::subagents::{ReviewNotes, ReviewSeverity},
    db::DbPool,
    domain::agent_context::{AgentContext, ChapterSummary, CharacterInfo},
};

/// 收集规则审查中 High 及以上问题，格式化为 "[agent] category: description"。
pub fn merge_rule_issues(notes: &[ReviewNotes]) -> Vec<String> {
    notes
        .iter()
        .flat_map(|n| {
            n.issues.iter().filter_map(move |i| {
                if i.severity >= ReviewSeverity::High {
                    Some(format!("[{}] {}: {}", n.agent, i.category, i.description))
                } else {
                    None
                }
            })
        })
        .collect()
}

/// 为规则复检构建最小 AgentContext：角色/世界规则取自 DB 资产（Task 4
/// 落库后可用）， 活跃线索取自黑板伏笔条目摘要。
pub fn build_review_context(
    pool: &DbPool,
    story_id: &str,
    foreshadowing_hints: &[String],
) -> AgentContext {
    let mut ctx = AgentContext::minimal(story_id.to_string(), String::new());
    if let Ok(chars) =
        crate::db::repositories::CharacterRepository::new(pool.clone()).get_by_story(story_id)
    {
        ctx.narrative.characters = chars
            .iter()
            .map(|c| CharacterInfo {
                name: c.name.clone(),
                personality: c.personality.clone().unwrap_or_default(),
                role: String::new(),
                appearance: c.appearance.clone(),
                gender: c.gender.clone(),
                age: c.age,
            })
            .collect();
    } else {
        log::warn!(
            "build_review_context: 读取角色失败，规则复检上下文降级（story_id={}）",
            story_id
        );
    }
    if let Ok(Some(world)) =
        crate::db::repositories::WorldBuildingRepository::new(pool.clone()).get_by_story(story_id)
    {
        let rules_text = serde_json::to_string(&world.rules).unwrap_or_default();
        ctx.world.world_rules = Some(format!("{}\n{}", world.concept, rules_text));
    } else {
        log::warn!(
            "build_review_context: 读取世界观失败，规则复检上下文降级（story_id={}）",
            story_id
        );
    }
    // 最近场景开头 → previous_chapters（ContinuityAgent 重复开头 High
    // 检查依赖此字段； 与 asset_query scenes 同一查询，sequence_number
    // 可能为负需钳制到 u32）
    let chapters = (|| -> Option<Vec<ChapterSummary>> {
        let conn = pool.get().ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT sequence_number, COALESCE(title,''), substr(COALESCE(content,''),1,200)
                 FROM scenes WHERE story_id = ?1 ORDER BY sequence_number DESC LIMIT 5",
            )
            .ok()?;
        let rows = stmt
            .query_map(rusqlite::params![story_id], |r| {
                Ok(ChapterSummary {
                    title: r.get::<_, String>(1)?,
                    number: r.get::<_, i32>(0)?.max(0) as u32,
                    summary: r.get::<_, String>(2)?,
                })
            })
            .ok()?;
        let mut chapters: Vec<ChapterSummary> = rows.flatten().collect();
        chapters.reverse(); // 恢复时间序
        Some(chapters)
    })();
    match chapters {
        Some(chapters) => ctx.narrative.previous_chapters = chapters,
        None => log::warn!(
            "build_review_context: 读取历史章节失败，规则复检上下文降级（story_id={}）",
            story_id
        ),
    }
    ctx.narrative.active_threads = foreshadowing_hints.to_vec();
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::subagents::{ReviewIssue, ReviewNotes, ReviewSeverity};

    #[test]
    fn test_merge_rule_issues_high_and_above_only() {
        let mut notes = ReviewNotes::new("style", "风格审查");
        notes.add_issue(ReviewIssue::new(
            ReviewSeverity::High,
            "AI腔",
            "陈词滥调",
            "删掉",
        ));
        notes.add_issue(ReviewIssue::new(
            ReviewSeverity::Low,
            "句长",
            "偏长",
            "可拆",
        ));
        let mut notes2 = ReviewNotes::new("continuity", "连续性");
        notes2.add_issue(ReviewIssue::new(
            ReviewSeverity::Critical,
            "矛盾",
            "角色已死却出场",
            "改",
        ));
        let merged = merge_rule_issues(&[notes, notes2]);
        assert_eq!(merged.len(), 2);
        assert!(merged[0].contains("AI腔"));
        assert!(merged[1].contains("矛盾"));
    }

    #[test]
    fn test_merge_empty() {
        assert!(merge_rule_issues(&[]).is_empty());
        assert!(merge_rule_issues(&[ReviewNotes::new("world", "无问题")]).is_empty());
    }
}
