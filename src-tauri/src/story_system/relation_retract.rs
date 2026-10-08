//! 按证据撤回关系（v0.64.11）。
//!
//! 真机体检发现：`kg_relations` 是**一次抽取一行**（随机 id + 单元素 `evidence`
//! 标记，形如 `["chapter:<story>:<n>"]` / `["scene:<id>"]` /
//! `["agency:scene:<id>"]`），而 ingest 只 upsert/追加、**从不撤回**。于是在第
//! 9 章 删掉的那段关系，会以「幽灵行」一直躺在图谱里，继续参与后续注入。
//!
//! 本模块在正文被编辑/场景被删除时，把指向该场景（或该章）的**证据**摘掉；
//! 某条关系的证据全部消失即删除该行。随后 ingest 会按新正文重抽，抽到的会再回来
//! ——「旧文本支撑的关系不复活」。
//!
//! 手工整理的 `character_relationships` **不在这里删**（它没有证据列，且可能是
//! 作者手改过的），改为由 [`audit_unsupported_relations`]
//! 把「正文里已找不到双方 同时出现」的行记成质量债，交作者决定。

use std::collections::HashSet;

use rusqlite::params;

use crate::db::DbPool;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RetractReport {
    /// 摘掉证据标记的关系行数
    pub evidence_cleared: usize,
    /// 证据摘空后删除的关系行数
    pub relations_dropped: usize,
}

/// 正文被编辑/场景被删除后，撤回指向该场景与该章的关系证据。
pub fn retract_relations_for_scene(
    pool: &DbPool,
    story_id: &str,
    scene_id: &str,
    chapter_number: Option<i32>,
) -> Result<RetractReport, String> {
    let mut stale_markers: HashSet<String> = HashSet::new();
    if !scene_id.trim().is_empty() {
        stale_markers.insert(format!("scene:{scene_id}"));
        stale_markers.insert(format!("agency:scene:{scene_id}"));
    }
    if let Some(n) = chapter_number.filter(|n| *n > 0) {
        stale_markers.insert(format!("chapter:{story_id}:{n}"));
    }
    if stale_markers.is_empty() {
        return Ok(RetractReport::default());
    }

    let conn = pool.get().map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, COALESCE(evidence, '[]') FROM kg_relations WHERE story_id = ?1")
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([story_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        mapped.filter_map(Result::ok).collect()
    };

    let mut report = RetractReport::default();
    for (id, evidence_json) in rows {
        let markers: Vec<String> = serde_json::from_str(&evidence_json).unwrap_or_default();
        if markers.is_empty() {
            continue;
        }
        let (kept, dropped): (Vec<String>, Vec<String>) = markers
            .into_iter()
            .partition(|m| !stale_markers.contains(m.trim()));
        if dropped.is_empty() {
            continue;
        }
        if kept.is_empty() {
            conn.execute("DELETE FROM kg_relations WHERE id = ?1", params![id])
                .map_err(|e| format!("删除失去证据的关系失败: {e}"))?;
            report.relations_dropped += 1;
        } else {
            let updated = serde_json::to_string(&kept).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE kg_relations SET evidence = ?2 WHERE id = ?1",
                params![id, updated],
            )
            .map_err(|e| format!("更新关系证据失败: {e}"))?;
            report.evidence_cleared += 1;
        }
    }
    if report.relations_dropped > 0 || report.evidence_cleared > 0 {
        log::warn!(
            "[relation_retract] 场景 {}（第{:?}章）改动：摘证据 {} 行、删行 {} 行",
            scene_id,
            chapter_number,
            report.evidence_cleared,
            report.relations_dropped
        );
    }
    Ok(report)
}

/// 关系行是否在**当前正文**里还有支撑（双方姓名同时出现于同一场景）。
pub fn relation_supported_by_prose(
    pool: &DbPool,
    story_id: &str,
    source_name: &str,
    target_name: &str,
) -> bool {
    let Ok(conn) = pool.get() else {
        return true; // 查不了就当有支撑，不乱报
    };
    let mut stmt = match conn.prepare(
        "SELECT COALESCE(content, '') FROM scenes WHERE story_id = ?1 AND COALESCE(content,'') != ''",
    ) {
        Ok(s) => s,
        Err(_) => return true,
    };
    let rows = match stmt.query_map([story_id], |r| r.get::<_, String>(0)) {
        Ok(r) => r,
        Err(_) => return true,
    };
    for text in rows.filter_map(Result::ok) {
        if text.contains(source_name) && text.contains(target_name) {
            return true;
        }
    }
    false
}

/// 审计「正文已无支撑」的手工关系行（不删除，只报告 —— 由调用方记质量债）。
///
/// 保守：双方姓名在同一场景里都不再同时出现才算失去支撑；别称/代称不计入
/// （宁可漏报，不误伤）。
pub fn audit_unsupported_relations(
    pool: &DbPool,
    story_id: &str,
) -> Result<Vec<(String, String, String)>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT c1.name, COALESCE(NULLIF(c2.name, ''), '对方'), r.relationship_type \
                 FROM character_relationships r \
                 JOIN characters c1 ON c1.id = r.source_character_id \
                 LEFT JOIN characters c2 ON c2.id = r.target_character_id \
                 WHERE r.story_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([story_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        mapped.filter_map(Result::ok).collect()
    };
    drop(conn);

    Ok(rows
        .into_iter()
        .filter(|(a, b, _)| !relation_supported_by_prose(pool, story_id, a, b))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool, repositories::CharacterRelationshipRepository,
        CreateCharacterRequest, CreateStoryRequest, KnowledgeGraphRepository,
    };

    fn seed_story(pool: &DbPool) -> String {
        crate::db::StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "关系撤回".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id
    }

    fn seed_character(pool: &DbPool, story_id: &str, name: &str) -> String {
        crate::db::repositories::CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: name.to_string(),
                ..Default::default()
            })
            .unwrap()
            .id
    }

    fn seed_kg_relation(
        pool: &DbPool,
        story_id: &str,
        source_id: &str,
        target_id: &str,
        evidence: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
             strength, evidence, first_seen) VALUES (?1, ?2, ?3, ?4, 'Enemy', 0.8, ?5, ?6)",
            params![
                &id,
                story_id,
                source_id,
                target_id,
                serde_json::to_string(&vec![evidence]).unwrap(),
                chrono::Local::now().to_rfc3339()
            ],
        )
        .unwrap();
        id
    }

    #[test]
    fn retract_drops_rows_whose_only_evidence_is_the_edited_chapter() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let a = seed_character(&pool, &story_id, "甲");
        let b = seed_character(&pool, &story_id, "乙");

        let ch9 = seed_kg_relation(&pool, &story_id, &a, &b, &format!("chapter:{story_id}:9"));
        let ch10 = seed_kg_relation(&pool, &story_id, &a, &b, &format!("chapter:{story_id}:10"));
        let scene = seed_kg_relation(&pool, &story_id, &b, &a, "scene:sc-1");

        // 编辑第 9 章：该章支撑的行被删；第 10 章的保留；场景标记不匹配
        let report = retract_relations_for_scene(&pool, &story_id, "sc-9", Some(9)).unwrap();
        assert_eq!(report.relations_dropped, 1);
        assert_eq!(report.evidence_cleared, 0);
        let conn = pool.get().unwrap();
        let left: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM kg_relations WHERE story_id = ?1")
                .unwrap();
            let rows = stmt
                .query_map([&story_id], |r| r.get::<_, String>(0))
                .unwrap();
            rows.filter_map(Result::ok).collect()
        };
        assert!(left.contains(&ch10) && left.contains(&scene), "{left:?}");
        assert!(!left.contains(&ch9), "第9章支撑的关系必须撤回");

        // 编辑该场景：scene 标记的行也被删
        let report = retract_relations_for_scene(&pool, &story_id, "sc-1", None).unwrap();
        assert_eq!(report.relations_dropped, 1);
        let left: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_relations WHERE story_id = ?1",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 1, "只剩第10章那行");
    }

    #[test]
    fn retract_clears_evidence_but_keeps_multi_evidence_rows() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let a = seed_character(&pool, &story_id, "甲");
        let b = seed_character(&pool, &story_id, "乙");
        let both = seed_kg_relation(&pool, &story_id, &a, &b, &format!("chapter:{story_id}:9"));
        // 追加第二条证据（模拟多场景支撑）
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE kg_relations SET evidence = ?2 WHERE id = ?1",
            params![
                &both,
                serde_json::to_string(&vec![
                    format!("chapter:{story_id}:9"),
                    format!("chapter:{story_id}:10")
                ])
                .unwrap()
            ],
        )
        .unwrap();

        let report = retract_relations_for_scene(&pool, &story_id, "sc-9", Some(9)).unwrap();
        assert_eq!(report.evidence_cleared, 1);
        assert_eq!(report.relations_dropped, 0);
        let evidence: String = conn
            .query_row(
                "SELECT evidence FROM kg_relations WHERE id = ?1",
                [&both],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !evidence.contains(":9]") && evidence.contains(":10"),
            "{evidence}"
        );
    }

    #[test]
    fn audit_reports_only_relations_without_prose_support() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let a = seed_character(&pool, &story_id, "甲");
        let b = seed_character(&pool, &story_id, "乙");
        let c = seed_character(&pool, &story_id, "丙");
        let scene_repo = crate::db::SceneRepository::new(pool.clone());
        let scene = scene_repo.create(&story_id, 1, None).unwrap();
        scene_repo
            .update(
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some("甲与乙在同一条船上。".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let repo = CharacterRelationshipRepository::new(pool.clone());
        repo.create(
            &story_id, &a, &b, "盟友", None, None, None, None, None, None,
        )
        .unwrap();
        repo.create(
            &story_id, &a, &c, "仇敌", None, None, None, None, None, None,
        )
        .unwrap();

        let unsupported = audit_unsupported_relations(&pool, &story_id).unwrap();
        assert_eq!(unsupported.len(), 1, "{unsupported:?}");
        assert_eq!(unsupported[0].0, "甲");
        assert_eq!(unsupported[0].1, "丙");
    }
}
