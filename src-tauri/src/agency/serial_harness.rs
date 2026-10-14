//! P2-6（v0.71.0）：长篇稳定性回归门——多拍串联的确定性守卫。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md（P2-6）。与 golden
//! harness 不同，本 harness **全程 MockLlm、不进真机、随 `cargo test` 进 CI**；
//! 它测的不是文笔，而是**跨拍状态约束**——第 N 拍发生的事实，能否真的约束第
//! N+1 拍（「零连续性衰减」的可回归形态）。
//!
//! 四个场景（一条串联的写作流，同一场景逐拍 Append）：
//! 1. **死亡落库**：第 1 拍写死一个角色 → 刷新（模拟章节保存钩子）后进入持久
//!    dead 名单；
//! 2. **死亡重演被拦**：第 2 拍重演「登时气绝」→ 探针命中 → 合规重试稿落库；
//! 3. **物品归属被拦**：第 3 拍让在场角色使用「持有者不在场」的物品 → 探针命中
//!    → 合规重试稿落库；
//! 4. **知识越界被拦**：第 4 拍让角色说出 `secrets_unknown` 里的信息 → 探针命中
//!    → 合规重试稿落库。
//!
//! 直接探针断言（`memory::continuity::continuity_gaps`）保证「探针本身有效」，
//! 端到端断言（违规句不落库、合规句落库）保证「探针接在写作链路上」。

use std::sync::Arc;

use crate::{
    agency::{
        budget::{AgencyBudget, DEFAULT_RUN_TOKEN_BUDGET},
        coordinator::AgencyCoordinator,
        persist::PersistMode,
        tests::MockLlm,
    },
    db::{create_test_pool, repositories::SceneRepository, DbPool},
};

/// 中性填充池（不含角色名/物品名/死亡动词/秘密片段）：逐句变化，避免触发
/// 自重复裁剪与 anti-repeat 重试——脚本正文每拍 ≥250 字且句句不同。
const FILLER_POOL: &[&str] = &[
    "他把门闩重新插好，火盆里的炭响了一下。",
    "风从窗缝挤进来，吹动了挂在梁上的旧灯笼。",
    "影子在墙上晃了一晃又停住，像谁在门外侧耳听着。",
    "远处有更鼓敲过，声音隔着雪传得很钝。",
    "桌上那碗茶已经凉透了，边沿结着一圈浅浅的茶渍。",
    "麻绳上的冰凌一节节垂下来，被炉火烤得滴答作响。",
    "有人把湿柴添进灶膛，烟从梁缝里慢慢绕出去。",
    "屋檐下的雪块忽然落了一块，砸在空水缸上闷闷地一响。",
    "铜壶里的水开了，壶盖被蒸汽顶得轻轻跳动。",
    "窗纸透进来的光发青，照见地上一片没扫净的稻草屑。",
    "角落里的蒲团塌了一半，坐上去大概会陷进草屑里。",
    "墙根那排腌菜坛子蒙着灰，坛口的布口扎得很紧。",
    "风把帘子的一角掀起来，又软软地垂回去。",
    "廊下挂的干辣椒串晃了两下，红得发暗。",
    "磨刀石上的水结了一层薄冰，刀刃刮过去发出细响。",
    "灶台上留着一小把盐，装在豁了口的粗瓷碟里。",
];

/// 取 `count` 句填充（按 offset 轮转，句句不同）。
fn filler(offset: usize, count: usize) -> String {
    (0..count)
        .map(|i| FILLER_POOL[(offset + i) % FILLER_POOL.len()])
        .collect::<Vec<_>>()
        .join("")
}

fn long_text_at(offset: usize, marker: &str) -> String {
    format!("{marker}{}他坐到天亮，没有再说话。", filler(offset, 8))
}

fn long_text(marker: &str) -> String {
    long_text_at(0, marker)
}

struct Fixture {
    story_id: String,
    scene_id: String,
}

fn seed_fixture(pool: &DbPool) -> Fixture {
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "长篇稳定性".into(),
            description: Some("雪夜渡口的对峙即将收场。".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let story_id = story.id.clone();
    {
        let conn = pool.get().unwrap();
        for (id, name, goals) in [
            ("c1", "阿苔", "找回断灯绳的下落"),
            ("c2", "苏会山", "守住渡口"),
            ("c3", "明成公主", "夺回印信"),
            ("c4", "曹元佩", "旁观待价"),
        ] {
            conn.execute(
                "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '渡口旧人', '谨慎', ?4, 'agency', 1, '2026-01-01', '2026-01-01')",
                rusqlite::params![id, story_id, name, goals],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, target_character_id, relationship_type, description, created_at)
             VALUES ('r1', ?1, 'c1', 'c2', '对峙', '渡口之争', '2026-01-01')",
            rusqlite::params![story_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '乱世渡口', '[]', '旧朝倾覆', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：渡口的归属。三幕：对峙-易主-清算。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story_id],
        )
        .unwrap();
        // 阿苔尚不知道的秘密（知识边界探针的输入）。
        conn.execute(
            "INSERT INTO character_states (id, story_id, character_id, secrets_known, secrets_unknown, last_updated)
             VALUES ('cs1', ?1, 'c1', '[]', '[\"密道入口在井底石缝里\"]', '2026-01-01')",
            rusqlite::params![story_id],
        )
        .unwrap();
    }
    let scene_repo = SceneRepository::new(pool.clone());
    let scene = scene_repo.create(&story_id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &scene.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(
                    "<p>雪夜渡口，阿苔在灯下站定，明成公主的人马堵住了栈桥。苏会山没有露面。</p>"
                        .into(),
                ),
                outline_content: Some("本章：渡口对峙收场，阿苔追查断灯绳。".into()),
                ..Default::default()
            },
        )
        .unwrap();
    // 玉佩由不在场的曹元佩持有（归属探针的输入）。
    crate::memory::continuity::persist_item_holdings(
        pool,
        &story_id,
        Some(&scene.id),
        Some(1),
        &[crate::memory::continuity::ItemHoldingDelta {
            item: "玉佩".into(),
            holder: "曹元佩".into(),
            action: "acquire".into(),
            evidence: "第一章：曹元佩收下玉佩".into(),
        }],
    )
    .unwrap();
    Fixture {
        story_id,
        scene_id: scene.id,
    }
}

fn read_content(pool: &DbPool, scene_id: &str) -> String {
    SceneRepository::new(pool.clone())
        .get_by_id(scene_id)
        .unwrap()
        .and_then(|s| s.content)
        .unwrap_or_default()
}

async fn run_beat(pool: &DbPool, scripted: Vec<String>, fixture: &Fixture, run_id: &str) -> String {
    let llm = MockLlm::scripted(scripted.iter().map(String::as_str).collect());
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let _budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let current = read_content(pool, &fixture.scene_id);
    coordinator
        .run_continue(
            run_id,
            &fixture.story_id,
            PersistMode::Append {
                scene_id: fixture.scene_id.clone(),
            },
            "续写",
            Some(&current),
        )
        .await
        .unwrap();
    read_content(pool, &fixture.scene_id)
}

/// 第 1 拍：写死明成公主（合规死亡），保存钩子刷新后进入持久 dead 名单。
fn beat1_kill() -> String {
    // 「{name}的尸体」是确定性死亡标记（next 拍起进入 dead 名单与探针）。
    long_text(
        "明成公主扑上来夺印，苏会山侧身一格，短刃反手扎进她的胸口，她踉跄两步倒在栈桥上。\
         灯影晃了晃，明成公主的尸体慢慢被雪盖住，没有人敢上前去收。",
    )
}

/// 第 2 拍：违规——重演她「登时气绝」；合规——只写活人的反应。
fn beat2_violation() -> String {
    long_text("明成公主再次扑上来，短刃又一次扎进她的胸口，她登时气绝，倒在栈桥上。")
}

fn beat2_fixed() -> String {
    long_text(
        "苏会山与阿苔在栈桥上正面对峙，他刀刃上的血还没擦干。阿苔逼问印信的下落，\
         他不答，只把手按在灯柱上。两个人谁也不让路，风雪把灯影吹得东倒西歪。\
         阿苔退了一步，又上前一步，逼得他不得不开口。最后她把那盏灯提起来照他的脸，\
         他终于说出一个地名，声音低得像怕被雪听见。",
    )
}

/// 第 3 拍：违规——阿苔使用曹元佩持有的玉佩（持有者不在场）；
/// 合规——不碰玉佩，冲突继续。
fn beat3_violation() -> String {
    long_text("阿苔从怀里取出玉佩，凑到灯下细看，玉面上那道裂纹一直没变。")
}

fn beat3_fixed() -> String {
    long_text(
        "阿苔与苏会山在渡口再次对峙，她把手按在桌沿上，指节发白。他不肯让出栈桥，\
         她也不肯退。风雪里有人远远看着，谁都没有动。最后她收回手，换了一个问法，\
         逼他把藏起来的那句话说清楚。他沉默很久，终于露出一点松动。",
    )
}

/// 第 4 拍：违规——阿苔说出她尚不知道的密道入口；合规——绕开该信息。
fn beat4_violation() -> String {
    long_text("阿苔盯着井口说：密道入口在井底石缝里，你们藏得再深也没有用。")
}

fn beat4_fixed() -> String {
    long_text(
        "阿苔与苏会山的对峙到了尽头。她没有再追问那条路在哪里，只把灯往井沿上一放，\
         让他自己去看井里的影子。他看了很久，喉结动了一下，还是把话咽了回去。\
         她没有逼他，转身把灯提走，雪地上留下两行脚印，一行深一行浅，很快被新雪盖住。",
    )
}

#[tokio::test]
async fn serial_stability_across_beats() {
    let pool = create_test_pool().unwrap();
    let fixture = seed_fixture(&pool);

    // ---- 第 1 拍：死亡落库 ----
    let content = run_beat(
        &pool,
        vec![beat1_kill(), beat1_kill()],
        &fixture,
        "serial-1",
    )
    .await;
    assert!(content.contains("明成公主的尸体"), "第 1 拍应写死明成公主");
    // 模拟章节保存钩子（真机上由 update_scene / auto_commit 触发）。
    let marked = crate::story_system::life_status::refresh_after_text(
        &pool,
        &fixture.story_id,
        Some(1),
        &content,
    );
    assert!(
        marked.iter().any(|n| n.contains("明成公主")),
        "保存后应标记明成公主已死（实际 {marked:?}）"
    );
    assert!(
        crate::story_system::life_status::dead_names(&pool, &fixture.story_id)
            .iter()
            .any(|n| n.contains("明成公主")),
        "持久 dead 名单应包含明成公主"
    );

    // ---- 第 2 拍：死亡重演被拦（直接探针 + 端到端） ----
    let violation = beat2_violation();
    let direct = crate::agency::continue_assets::increment_replays_completed_deaths(
        &violation,
        &["明成公主".to_string()],
    );
    assert!(direct, "重演死亡必须被既有检测器命中");
    let content = run_beat(
        &pool,
        vec![violation.clone(), beat2_fixed()],
        &fixture,
        "serial-2",
    )
    .await;
    assert!(
        !content.contains("明成公主再次扑上来"),
        "违规重演稿不得落库"
    );
    assert!(
        content.contains("他终于说出一个地名"),
        "合规重试稿应落库（第 2 拍）"
    );

    // ---- 第 3 拍：物品归属被拦 ----
    let violation = beat3_violation();
    let knowledge = crate::memory::continuity::load_character_knowledge(
        &pool,
        &fixture.story_id,
        &["阿苔".to_string()],
    );
    let holdings = crate::memory::continuity::load_item_holdings(&pool, &fixture.story_id);
    let gaps = crate::memory::continuity::detect_possession_conflicts(
        &violation,
        &["阿苔".to_string()],
        &holdings,
    );
    assert!(
        gaps.iter().any(|g| g.contains("玉佩")),
        "使用不在场持有者的物品必须被归属探针命中（实际 {gaps:?}）"
    );
    let content = run_beat(
        &pool,
        vec![violation.clone(), beat3_fixed()],
        &fixture,
        "serial-3",
    )
    .await;
    assert!(!content.contains("凑到灯下细看"), "归属违规稿不得落库");
    assert!(
        content.contains("终于露出一点松动"),
        "合规重试稿应落库（第 3 拍）"
    );
    let _ = knowledge;

    // ---- 第 4 拍：知识越界被拦 ----
    let violation = beat4_violation();
    let hidden = crate::memory::continuity::load_hidden_truths(&pool, &fixture.story_id);
    let leak_gaps =
        crate::memory::continuity::detect_knowledge_leaks(&violation, &knowledge, &hidden, "");
    assert!(
        leak_gaps.iter().any(|g| g.contains("密道入口在井底石缝")),
        "说出 secrets_unknown 必须被泄密探针命中（实际 {leak_gaps:?}）"
    );
    let content = run_beat(
        &pool,
        vec![violation.clone(), beat4_fixed()],
        &fixture,
        "serial-4",
    )
    .await;
    assert!(
        !content.contains("密道入口在井底石缝"),
        "知识越界稿不得落库"
    );
    assert!(
        content.contains("很快被新雪盖住"),
        "合规重试稿应落库（第 4 拍）"
    );

    // ---- 串联断言：四拍之后，违规句式一次都没落库；死亡状态仍在 ----
    let final_content = read_content(&pool, &fixture.scene_id);
    for banned in ["明成公主再次扑上来", "凑到灯下细看", "密道入口在井底石缝"]
    {
        assert!(
            !final_content.contains(banned),
            "最终正文不得含违规句：{banned}"
        );
    }
    assert!(
        crate::story_system::life_status::dead_names(&pool, &fixture.story_id)
            .iter()
            .any(|n| n.contains("明成公主")),
        "四拍之后死亡状态不得被改写"
    );
}
