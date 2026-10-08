//! 角色生死状态（策略层）：何时刷新、如何交给注入路径、作者如何改判。
//!
//! v0.64.7 真机事故：《帝国的烟火》第 2 章明成公主被一拳打死（「七窍喷血……
//! 登时气绝」「明成公主的尸体」），自动续写到第 10 章又让她走路、说话、夺印、
//! 抓人手腕——同一场景里她的尸体还停在门板上。根因：生死只由
//! `dead_names_in_text` 在**当前章末 1500 字**窗口里临时推断
//! （`PRIOR_CAST_CHAR_CAP`），窗口一滑过，死人复活。
//!
//! 数据层（列读写 / 存量回填）在 `db::character_life`，文本判定在
//! `utils::death_text`；本模块负责：
//! - 章节提交 / 场景保存时按**整章正文**刷新（确定性，0 LLM）；
//! - 把已死名单交给续写准入（节拍卡 cast、导演锁、下一拍）；
//! - 作者改判（[`revive`]，假死/诈死情节）。
//!
//! 单调：alive → dead 只发生一次；此后任何状态写回都不能清掉「已死」标记。

use std::collections::HashMap;

#[allow(unused_imports)]
pub use crate::db::character_life::annotate_physical_state;
use crate::{db::DbPool, utils::death_text::dead_names_in_text};

/// 已死角色：name → death_chapter（可能未知）。注入路径据此打标记、排除出场。
pub fn dead_marker_map(pool: &DbPool, story_id: &str) -> HashMap<String, Option<i32>> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[life_status] 取连接失败: {e}");
            return HashMap::new();
        }
    };
    crate::db::character_life::dead_marker_map_on(&conn, story_id).unwrap_or_else(|e| {
        log::warn!("[life_status] 查询已死角色失败: {e}");
        HashMap::new()
    })
}

/// 已死名单（注入用）：死者本人 + 归到死者名下的其他称呼。
///
/// 真机《帝国的烟火》里 `公主` 是一条只有称号的幻影角色行（KG 实体，无
/// `characters` 行），若只按行名排除，它会带着「活人」身份进 cast。这里沿用
/// v0.64.6 的解析策略（`resolve_character_id`：精确名 → 别称表 → 唯一同人
/// 形态命中），把同一故事中解析到死者身上的称呼一并算已死；解析不出来就**不猜**
/// （宁可漏，不误伤）。
pub fn dead_names(pool: &DbPool, story_id: &str) -> Vec<String> {
    let dead_map = dead_marker_map(pool, story_id);
    if dead_map.is_empty() {
        return Vec::new();
    }
    let mut names: Vec<String> = dead_map.keys().cloned().collect();

    let Ok(conn) = pool.get() else {
        names.sort();
        return names;
    };
    let dead_ids: std::collections::HashSet<String> = match conn
        .prepare("SELECT id FROM characters WHERE story_id = ?1 AND life_status = 'dead'")
    {
        Ok(mut stmt) => match stmt.query_map([story_id], |r| r.get::<_, String>(0)) {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!("[life_status] 已死角色 id 查询失败: {e}");
                names.sort();
                return names;
            }
        },
        Err(e) => {
            log::warn!("[life_status] 已死角色 id 语句失败: {e}");
            names.sort();
            return names;
        }
    };

    for candidate in story_character_names(&conn, story_id) {
        if candidate.is_empty() || names.iter().any(|n| n == &candidate) {
            continue;
        }
        if let Ok(Some(id)) =
            crate::db::character_identity::resolve_character_id(&conn, story_id, &candidate)
        {
            if dead_ids.contains(&id) {
                names.push(candidate);
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

/// 故事里出现过的全部角色名（`characters` 行 ∪ KG `Character` 实体名，
/// 后者含只有称号的幻影行）。
fn story_character_names(conn: &rusqlite::Connection, story_id: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for sql in [
        "SELECT name FROM characters WHERE story_id = ?1",
        "SELECT name FROM kg_entities WHERE story_id = ?1 AND entity_type = 'Character'",
    ] {
        let Ok(mut stmt) = conn.prepare(sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([story_id], |r| r.get::<_, String>(0)) else {
            continue;
        };
        names.extend(rows.filter_map(Result::ok));
    }
    names
}

/// 标记身故（单调）。返回 true = 本次新标记。
pub fn mark_dead(
    pool: &DbPool,
    story_id: &str,
    name: &str,
    chapter: Option<i32>,
) -> Result<bool, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    crate::db::character_life::mark_dead_on(&conn, story_id, name, chapter)
        .map_err(|e| e.to_string())
}

/// 作者改判存活（假死/诈死情节）：清列 + 去掉标记。
pub fn revive(pool: &DbPool, story_id: &str, name: &str) -> Result<usize, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    crate::db::character_life::revive_on(&conn, story_id, name).map_err(|e| e.to_string())
}

/// 章节正文写成死亡 → 立刻把死亡落成持久事实。返回本次新标记的姓名。
///
/// 纯确定性扫描（`name_is_dead_in_text`，含「未气绝/假死/诈死」否定句豁免），
/// 不调模型；扫描范围是**整章正文**，不是章末窗口——真机第 2 章的死亡写在章中，
/// 章末窗口早就滑过了。
pub fn refresh_after_text(
    pool: &DbPool,
    story_id: &str,
    chapter_number: Option<i32>,
    text: &str,
) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let alive: Vec<String> = match pool.get() {
        Ok(conn) => match crate::db::character_life::alive_names_on(&conn, story_id) {
            Ok(names) => names,
            Err(e) => {
                log::warn!("[life_status] 查询存活角色失败: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            log::warn!("[life_status] 取连接失败: {e}");
            return Vec::new();
        }
    };
    if alive.is_empty() {
        return Vec::new();
    }
    let hits = dead_names_in_text(&alive, text);
    let mut marked = Vec::new();
    for name in hits {
        match mark_dead(pool, story_id, &name, chapter_number) {
            Ok(true) => {
                log::warn!(
                    "[life_status] 第{}章正文写成死亡，标记「{}」为已死（后续不得作为活人行动）",
                    chapter_number
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "?".into()),
                    name
                );
                marked.push(name);
            }
            Ok(false) => {}
            Err(e) => log::warn!("[life_status] 标记「{name}」失败: {e}"),
        }
    }
    marked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool, repositories::CharacterRepository, CreateCharacterRequest,
        CreateStoryRequest,
    };

    fn seed_story(pool: &DbPool) -> String {
        crate::db::StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "生死测试".into(),
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
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: name.to_string(),
                ..Default::default()
            })
            .unwrap()
            .id
    }

    #[test]
    fn refresh_marks_written_death_and_stays_monotone() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        seed_character(&pool, &story_id, "苏亦铁");

        let text = "苏会山的右拳本能般汇聚全身功力，以雷霆万钧之势一拳击中公主，\
                    将其打得横飞而出，越过众人头上，摔在几丈之外，七窍喷血，抽搐几下，登时气绝。\
                    明成公主的尸体躺在原处，短刃落在她手边。";
        let marked = refresh_after_text(&pool, &story_id, Some(2), text);
        assert_eq!(marked, vec!["明成公主".to_string()]);

        let dead = dead_marker_map(&pool, &story_id);
        assert_eq!(dead.get("明成公主"), Some(&Some(2)));
        assert!(!dead.contains_key("苏亦铁"), "未死者不得标记");

        // 幂等 + 单调：重跑不改章次、不重复计数
        let again = refresh_after_text(&pool, &story_id, Some(9), text);
        assert!(again.is_empty());
        assert_eq!(
            dead_marker_map(&pool, &story_id).get("明成公主"),
            Some(&Some(2))
        );
    }

    #[test]
    fn refresh_skips_negated_death_sentences() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_character(&pool, &story_id, "明成公主");
        let marked = refresh_after_text(
            &pool,
            &story_id,
            Some(3),
            "众人都说公主没有死，只是假死。江顾然确信明成公主未气绝。",
        );
        assert!(marked.is_empty(), "否定句不得判死 marked={marked:?}");
    }

    #[test]
    fn dead_marker_annotates_state_and_revive_strips_it() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let char_id = seed_character(&pool, &story_id, "明成公主");

        let text = "明成公主的尸体躺在原处。";
        assert_eq!(
            refresh_after_text(&pool, &story_id, Some(2), text),
            vec!["明成公主".to_string()]
        );

        let state = CharacterRepository::new(pool.clone())
            .get_character_state(&char_id)
            .unwrap()
            .expect("标记时应建状态行");
        let phys = state.physical_state.unwrap_or_default();
        assert!(
            crate::db::character_life::has_dead_marker(&phys),
            "phys={phys}"
        );
        assert!(phys.contains("第2章"), "phys={phys}");

        assert_eq!(revive(&pool, &story_id, "明成公主").unwrap(), 1);
        assert!(dead_names(&pool, &story_id).is_empty());
        let state = CharacterRepository::new(pool.clone())
            .get_character_state(&char_id)
            .unwrap()
            .unwrap();
        assert_eq!(state.physical_state, None, "改判后标记应清掉");

        // 改判后可以再次判死，且死亡章更新
        assert_eq!(
            refresh_after_text(&pool, &story_id, Some(7), text),
            vec!["明成公主".to_string()]
        );
        assert_eq!(
            dead_marker_map(&pool, &story_id).get("明成公主"),
            Some(&Some(7))
        );
    }

    /// 称号幻影行（KG 实体「公主」，无 `characters` 行）随死者一并排除：
    /// 别称表把「公主」登记在明成公主名下后，`dead_names` 必须含这两条。
    #[test]
    fn dead_names_expand_to_registered_aliases() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let char_id = seed_character(&pool, &story_id, "明成公主");
        // 幻影行：只有 KG 实体，没有 characters 行（真机就长这样）
        crate::db::KnowledgeGraphRepository::new(pool.clone())
            .create_entity(
                &story_id,
                "公主",
                "Character",
                &serde_json::json!({"mood": "（已故）"}),
                None,
            )
            .unwrap();
        crate::db::repositories::CharacterAliasRepository::new(pool.clone())
            .upsert(&story_id, &char_id, "公主", "ingest")
            .unwrap();

        mark_dead(&pool, &story_id, "明成公主", Some(2)).unwrap();
        let dead = dead_names(&pool, &story_id);
        assert!(dead.contains(&"明成公主".to_string()), "dead={dead:?}");
        assert!(
            dead.contains(&"公主".to_string()),
            "称号幻影行必须一并算已死 dead={dead:?}"
        );

        // 解析不到死者的称呼不得连坐
        seed_character(&pool, &story_id, "苏亦铁");
        let dead = dead_names(&pool, &story_id);
        assert!(
            !dead.contains(&"苏亦铁".to_string()),
            "活人不得连坐 dead={dead:?}"
        );
    }

    /// 真机验收探针（手动跑，不进 CI）：对**真实库的副本**跑 V142 回填与
    /// 续写准入，复现《帝国的烟火》第 10 章死人复活并验证已被拦住。
    ///
    /// ```bash
    /// STORYMOSS_DB="$HOME/Library/Application Support/com.storymoss.app/cinema_ai.db" \
    ///   cargo test --lib life_status::tests::real_machine_probe -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn real_machine_probe_resurrect_is_blocked() {
        let src = std::env::var("STORYMOSS_DB").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/Library/Application Support/com.storymoss.app/cinema_ai.db")
        });
        let src_path = std::path::PathBuf::from(&src);
        assert!(src_path.exists(), "真机库不存在: {src}");

        // 副本（含 WAL/SHM），绝不碰原库
        let tmp = std::env::temp_dir().join(format!(
            "storymoss-probe-{}",
            chrono::Local::now().timestamp_millis()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        for suffix in ["", "-wal", "-shm"] {
            let from = std::path::PathBuf::from(format!("{src}{suffix}"));
            if from.exists() {
                std::fs::copy(&from, tmp.join(format!("cinema_ai.db{suffix}"))).unwrap();
            }
        }

        // init_db 跑迁移（含 V142 回填），断言已死角色落库
        let pool = crate::db::connection::init_db(&tmp, None).expect("打开副本库并迁移");
        let story_id: String = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT id FROM stories WHERE title LIKE '%烟火%' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("副本里应有《帝国的烟火》");

        let dead = dead_marker_map(&pool, &story_id);
        println!("[探针] 回填后已死角色: {dead:?}");
        assert_eq!(
            dead.get("明成公主"),
            Some(&Some(2)),
            "第 2 章写成死亡的明成公主必须被标记 dead（第 2 章）"
        );

        // 第 10 章正文（真机里她"复活"的那一章）：续写准入不得再让她上场
        let content: String = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COALESCE(s.content, '') FROM scenes s \
                 LEFT JOIN chapters c ON c.id = s.chapter_id \
                 WHERE s.story_id = ?1 AND COALESCE(c.chapter_number, s.sequence_number) = 10 \
                 ORDER BY s.sequence_number DESC LIMIT 1",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        assert!(!content.is_empty(), "真机第 10 章正文应存在");

        let card = crate::agency::beat_card::compile_beat_card(&pool, &story_id, &content)
            .expect("编译本拍卡");
        println!(
            "[探针] cast={:?} dead={:?}",
            card.cast.iter().map(|c| &c.name).collect::<Vec<_>>(),
            card.dead
        );
        assert!(
            card.dead.iter().any(|d| d == "明成公主"),
            "已死的明成公主必须在 dead 名单"
        );
        assert!(
            !card.cast.iter().any(|c| c.name == "明成公主"),
            "已死的明成公主不得进 cast"
        );
        // 冲突阶梯：先留住第一拍的信息（card 变量后面会被角色卡复用）
        let first_line = card.conflict_move.action.clone();
        let first_stage = card.conflict_move.stage;
        let first_block = card.render_scene_outline();

        let bundle = crate::creative_engine::write_time_bundle::WriteTimeBundle::load_sync(
            &pool, &story_id, 11, None, None, None,
        )
        .expect("加载写作包");
        let card = bundle
            .core_characters
            .iter()
            .find(|c| c.name == "明成公主")
            .expect("角色卡应在包里");
        println!("[探针] 角色卡身体状态: {:?}", card.physical_state);
        assert!(crate::db::character_life::has_dead_marker(
            card.physical_state.as_deref().unwrap_or("")
        ));

        // v0.64.9：投影 writer 必须吃得下真机 commit 产物（此前 state/index
        // 一直报 missing field subject / entity_id，状态类记忆一条不落）。
        use crate::story_system::projection_writers::{get_projection_writers, ProjectionWriter};
        let commits: Vec<(i32, String)> = {
            let conn = pool.get().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT chapter_number, \
                     json_object('state_deltas_json', COALESCE(state_deltas_json,''), \
                                 'entity_deltas_json', COALESCE(entity_deltas_json,'')) \
                     FROM scene_commits WHERE story_id = ?1 AND state_deltas_json IS NOT NULL \
                     AND state_deltas_json NOT IN ('', '[]')",
                )
                .unwrap();
            let rows = stmt.query_map([&story_id], |r| {
                Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?))
            });
            rows.unwrap().filter_map(Result::ok).collect()
        };
        assert!(!commits.is_empty(), "真机应有带状态增量的 commit");
        let writers = get_projection_writers(pool.clone());
        for (chapter, commit_json) in &commits {
            for w in &writers {
                if w.name() == "state" || w.name() == "index" {
                    w.apply(&story_id, *chapter, commit_json)
                        .unwrap_or_else(|e| {
                            panic!("[探针] writer {} 第{}章仍失败: {e}", w.name(), chapter)
                        });
                }
            }
        }
        let rows: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM memory_items WHERE story_id = ?1 AND category IN ('state','entity')",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        println!("[探针] 投影后 memory_items（state+entity）行数: {rows}");
        assert!(rows > 0, "投影必须真的落下记忆行");

        // v0.64.10：冲突阶梯——把第一拍的卡块当上一拍再编译，第二拍必须换级
        // （不得是同一句「加压：甲 与 乙 正面对峙」）
        let second = crate::agency::beat_card::compile_beat_card_located_prev(
            &pool,
            &story_id,
            &content,
            None,
            Some(&first_block),
        )
        .expect("第二次编译本拍卡");
        println!(
            "[探针] 冲突阶梯：第1拍({:?}) {} → 第2拍({:?}) {}",
            first_stage, first_line, second.conflict_move.stage, second.conflict_move.action
        );
        assert_ne!(
            second.conflict_move.action, first_line,
            "同一对峙不得连拍同一句"
        );
        assert_ne!(
            second.conflict_move.stage,
            crate::agency::beat_card::default_conflict_stage(),
            "第二拍必须已在阶梯上推进"
        );

        // ── v0.64.11：关系归一（V144 回填）+ 撤回审计 + 物料失效/重算 ──
        let (kind_rows, raw_compound): (i64, Vec<(String, String, Option<i64>)>) = {
            let conn = pool.get().unwrap();
            let unclassified: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM character_relationships WHERE story_id = ?1 \
                     AND relation_kind IS NULL",
                    [&story_id],
                    |r| r.get(0),
                )
                .unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT relationship_type, COALESCE(relation_kind,''), relation_flags \
                     FROM character_relationships WHERE story_id = ?1 \
                     AND (relationship_type LIKE '%／%' OR relationship_type LIKE '%/%' \
                          OR relationship_type LIKE '%（%')",
                )
                .unwrap();
            let rows = stmt
                .query_map([&story_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .unwrap();
            (unclassified, rows.filter_map(Result::ok).collect())
        };
        println!("[探针] 关系：未归一列 {kind_rows} 行；复合写法样例 {raw_compound:?}");
        assert_eq!(kind_rows, 0, "V144 必须把所有关系行补上 relation_kind");

        let unsupported =
            crate::story_system::relation_retract::audit_unsupported_relations(&pool, &story_id)
                .unwrap();
        println!(
            "[探针] 正文已无支撑的关系行（只报告不删）：{} 行 {:?}",
            unsupported.len(),
            unsupported.iter().take(3).collect::<Vec<_>>()
        );

        crate::story_system::recompute::mark_stale(&pool, &story_id, 9, "探针：模拟编辑第9章")
            .unwrap();
        let stale = crate::story_system::recompute::list_stale(&pool, &story_id).unwrap();
        assert_eq!(stale.len(), 3, "三类物料都应标记失效：{stale:?}");
        assert!(stale.iter().all(|s| s.from_chapter == 9));
        let report = crate::story_system::recompute::recompute_from(&pool, &story_id, 9, None)
            .await
            .unwrap();
        println!(
            "[探针] 物料重算（无模型）：章节摘要 {} 条、段摘要删 {}、快照重写 {}、待模型 {}",
            report.chapter_summaries,
            report.segment_summaries_deleted,
            report.checkpoints_rewritten,
            report.segment_pending_llm
        );
        assert!(
            report.chapter_summaries > 0,
            "第9章及以后应有章节摘要被重算"
        );
        let left = crate::story_system::recompute::list_stale(&pool, &story_id).unwrap();
        assert!(
            left.iter().all(|s| s.kind == "segment_summary"),
            "无模型时只应剩分层摘要待重算：{left:?}"
        );

        // v0.64.12：指纹感知——第二轮重算不应再改任何摘要（正文没变）
        let again = crate::story_system::recompute::recompute_from(&pool, &story_id, 9, None)
            .await
            .unwrap();
        println!(
            "[探针] 二轮重算（指纹感知）：重算 {} 条 / 未变 {} 条",
            again.chapter_summaries, again.summaries_unchanged
        );
        assert_eq!(again.chapter_summaries, 0, "正文没变就不该再写摘要");
        assert!(again.summaries_unchanged > 0);

        // 自动触发判定：失效标记还在（分层摘要），刚提交第 12 章 → 应触发
        let target = crate::story_system::recompute::should_auto_recompute(&pool, &story_id, 12);
        println!("[探针] 自动重算判定（提交第12章）：{target:?}");
        assert_eq!(target, Some(9), "应判定从第 9 章起重算");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
