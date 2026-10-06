//! P3-E「三把尺子」端到端组合契约（v0.63.0）。
//!
//! 把外部对比报告里作者提出的三个测试合成一个故事级场景，一次性验证：
//! ① **玉佩测试**：物品归属前后一致——非持有者使用会被探针拦下，当场转手豁免；
//! ② **知识边界测试**：角色不会说出尚不知道的信息——泄密被检出，计划内揭示豁免；
//! ③ **级联冲突测试**：
//! 大修旧章后能揪出后文冲突——下游受影响章产生影响记录与失效标记。
//!
//! 三个场景共用同一个故事种子，接近真实的连续使用；P0 的单元契约在此之上
//! 提供组合回归门。

#![cfg(test)]

use crate::{
    agency::{
        beat_card::{
            CastMember, ChangeDelta, ChangeKind, ConflictMove, EmotionBeat, SceneBeatCard,
        },
        beat_state::probe_increment_ex,
    },
    creative_engine::cascade_rewriter::impact_report,
    db::{connection::create_test_pool, DbPool},
    memory::continuity::{self, ItemHoldingDelta, KnowledgeUpdate, TimelineEventDelta},
};

fn seed_story(pool: &DbPool, characters: &[&str]) -> String {
    let story_id = uuid::Uuid::new_v4().to_string();
    let conn = pool.get().unwrap();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '三把尺子', ?2, ?2)",
        rusqlite::params![&story_id, &now],
    )
    .unwrap();
    for name in characters {
        conn.execute(
            "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &story_id, name, &now],
        )
        .unwrap();
    }
    story_id
}

fn add_scene(pool: &DbPool, story_id: &str, sequence: i32, content: &str) -> String {
    let scene_id = uuid::Uuid::new_v4().to_string();
    let conn = pool.get().unwrap();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO scenes (id, story_id, sequence_number, content, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![&scene_id, story_id, sequence, content, &now],
    )
    .unwrap();
    scene_id
}

fn add_item_entity(pool: &DbPool, story_id: &str, name: &str) -> String {
    let entity_id = uuid::Uuid::new_v4().to_string();
    let conn = pool.get().unwrap();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO kg_entities (id, story_id, name, entity_type, first_seen, last_updated) \
         VALUES (?1, ?2, ?3, 'Item', ?4, ?4)",
        rusqlite::params![&entity_id, story_id, name, &now],
    )
    .unwrap();
    entity_id
}

fn add_mention(pool: &DbPool, story_id: &str, scene_id: &str, entity_id: &str) {
    use crate::creative_engine::cascade_rewriter::{
        models::EntityMention, EntityMentionRepository,
    };
    let repo = EntityMentionRepository::new(pool.clone());
    let now = chrono::Local::now().to_rfc3339();
    repo.create(&EntityMention {
        id: uuid::Uuid::new_v4().to_string(),
        story_id: story_id.to_string(),
        scene_id: scene_id.to_string(),
        entity_id: entity_id.to_string(),
        entity_type: "Item".to_string(),
        start_pos: 0,
        end_pos: 4,
        mention_text: "玉佩".to_string(),
        confidence: 1.0,
        created_at: now.clone(),
        updated_at: now,
    })
    .unwrap();
}

/// 最小可用的节拍卡（探针只关心 dead/offshot/quota 等字段）。
fn minimal_card() -> SceneBeatCard {
    SceneBeatCard {
        cast: vec![CastMember {
            name: "苏亦铁".into(),
            purpose: "在场".into(),
        }],
        conflict_move: ConflictMove {
            action: "加压".into(),
            parties: vec!["苏亦铁".into()],
        },
        emotion_beat: EmotionBeat {
            summary: "怒".into(),
        },
        next_outline_node: "夜宴破裂".into(),
        expansion_quota: vec![],
        expansion_quota_text: None,
        setting_location: Some("雨巷".into()),
        open_review_issues: vec![],
        dead: vec![],
        change_delta: ChangeDelta {
            kind: ChangeKind::Information,
            summary: "夜宴破裂".into(),
        },
    }
}

#[test]
fn three_rulers_hold_on_one_story() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story(&pool, &["林晚", "苏亦铁", "徐棠"]);
    let ch3 = add_scene(&pool, &story_id, 3, "林晚系好羊脂玉佩，把信收进袖中。");
    let ch7 = add_scene(&pool, &story_id, 7, "苏亦铁在灯下擦拭兵器。");
    let jade = add_item_entity(&pool, &story_id, "羊脂玉佩");
    add_mention(&pool, &story_id, &ch3, &jade);
    add_mention(&pool, &story_id, &ch7, &jade);

    // ---------- 尺子一：玉佩（物品归属） ----------
    continuity::persist_item_holdings(
        &pool,
        &story_id,
        Some(&ch3),
        Some(3),
        &[ItemHoldingDelta {
            item: "羊脂玉佩".into(),
            holder: "林晚".into(),
            action: "acquire".into(),
            evidence: "林晚系好羊脂玉佩".into(),
        }],
    )
    .unwrap();
    let present = vec!["苏亦铁".to_string()];
    let bad = "苏亦铁从袖中掏出羊脂玉佩。";
    let gaps = continuity::continuity_gaps(&pool, &story_id, bad, &present, "");
    assert!(
        gaps.iter()
            .any(|g| g.contains("羊脂玉佩") && g.contains("林晚")),
        "尺子①：非持有者使用应被拦下 gaps={gaps:?}"
    );
    let transferred = "林晚把羊脂玉佩塞给苏亦铁，苏亦铁握着玉佩出神。";
    assert!(
        continuity::continuity_gaps(&pool, &story_id, transferred, &present, "").is_empty(),
        "尺子①：当场转手应豁免"
    );

    // ---------- 尺子二：知识边界 ----------
    continuity::persist_knowledge_updates(
        &pool,
        &story_id,
        Some(&ch7),
        Some(7),
        &[KnowledgeUpdate {
            character: "徐棠".into(),
            fact: "这封信是哥哥寄的".into(),
            evidence: "她认出了笔迹".into(),
        }],
    )
    .unwrap();
    // 徐棠此刻已知道——不再算泄密
    let known_text = "徐棠低声道：这封信是哥哥寄的。";
    assert!(
        continuity::continuity_gaps(&pool, &story_id, known_text, &["徐棠".to_string()], "")
            .is_empty(),
        "尺子②：已知信息不算泄密"
    );
    // 未揭示的世界真相被叙述者说出 → 泄密
    continuity::persist_timeline_events(
        &pool,
        &story_id,
        Some(&ch3),
        Some(3),
        &[TimelineEventDelta {
            objective_fact: "将军早已知道密道入口在佛堂".into(),
            reader_knowledge: "读者只当将军在猜".into(),
            reveal_status: "hidden".into(),
            participants: vec![],
            evidence: String::new(),
        }],
    )
    .unwrap();
    let leak = "苏亦铁冷笑：将军早已知道密道入口在佛堂。";
    let leak_gaps =
        continuity::continuity_gaps(&pool, &story_id, leak, &["苏亦铁".to_string()], "");
    assert!(
        leak_gaps.iter().any(|g| g.contains("提前揭示")),
        "尺子②：未揭示真相应被拦下 gaps={leak_gaps:?}"
    );

    // 探针层：同样的文本经 beat_state 探针（gap 参与一次补写重试）
    let card = minimal_card();
    let state = crate::agency::beat_state::compile_beat_state(
        &["苏亦铁".into()],
        Some("雨巷"),
        "夜宴破裂",
        &[],
        "",
        &[],
    );
    let probe = probe_increment_ex(leak, &card, &state, &[], None, "", "novel");
    assert!(
        probe.gaps.is_empty(),
        "纯探针不含 DB 检测（DB 检测经 coordinator 注入）"
    );

    // ---------- 尺子三：改稿级联冲突 ----------
    let impacts = impact_report::compute_mention_impacts(&pool, &story_id, &ch3);
    assert_eq!(impacts.len(), 1, "尺子③：只有共享玉佩的第7章受影响");
    assert_eq!(impacts[0].target_chapter_number, Some(7));
    impact_report::persist_impacts(&pool, &story_id, "batch-1", &ch3, Some(3), &impacts).unwrap();
    let rows = impact_report::list_impacts(&pool, &story_id, Some("open"), 50).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].stale_flag, "尺子③：目标章应标记分析可能已失效");

    // 作者处理：忽略 → 出队；重跑分析 → 清除失效标记
    assert_eq!(
        impact_report::mark_impact_decision(&pool, &rows[0].id, "ignored").unwrap(),
        1
    );
    assert!(
        impact_report::list_impacts(&pool, &story_id, Some("open"), 50)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        impact_report::clear_stale_for_target_scene(&pool, &ch7).unwrap(),
        1
    );
    let cleared = impact_report::load_impact(&pool, &rows[0].id)
        .unwrap()
        .unwrap();
    assert!(!cleared.stale_flag);
}
