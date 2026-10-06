use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use super::*;
use crate::{
    agency::{
        budget::{AgencyBudget, DEFAULT_RUN_TOKEN_BUDGET},
        coordinator::*,
        persist::PersistMode,
        repository::AgencyRepository,
        tool_loop::LoopLlm,
        tools::ToolRegistry,
    },
    db::{create_test_pool, repositories::SceneRepository},
    error::AppError,
};

/// 长前提（> 100 字符），跳过 PROBLEM logline 生成（v0.30.22）。
/// 现有 genesis 测试的 MockLlm 队列按原始调用序设计，logline 额外
/// LLM 调用会消费首条响应导致队列错位。使用长前提确保 logline 不触发。
const LONG_PREMISE: &str = "在一个被双星辐射笼罩的废土世界，孤独的拾荒者偶然发现了传说中星海遗迹的\
    坐标碎片。为了拯救日渐衰败的家园，他必须穿越致命的辐射区、躲避掠夺者的追杀，在遗迹中找到\
    改变命运的力量。这是一个关于生存、勇气、牺牲和希望的科幻故事，讲述人性的光辉与黑暗。";

struct MockLlm {
    responses: Mutex<VecDeque<String>>,
    /// 已收调用记录（user_prompt 原文），供调用顺序断言。
    calls: Mutex<Vec<String>>,
    /// complete() 的 system prompt，供大纲方法论断言。
    systems: Mutex<Vec<String>>,
    /// complete_json 调用记录（F3 JSON mode 断言用；同时计入 calls）。
    json_calls: Mutex<Vec<String>>,
}

impl MockLlm {
    fn scripted(lines: Vec<&str>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(lines.into_iter().map(String::from).collect()),
            calls: Mutex::new(Vec::new()),
            systems: Mutex::new(Vec::new()),
            json_calls: Mutex::new(Vec::new()),
        })
    }

    fn next(&self, u: &str) -> Result<String, AppError> {
        self.calls.lock().unwrap().push(u.to_string());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| AppError::validation_failed("mock exhausted", None::<String>))
    }
}

#[async_trait::async_trait]
impl LoopLlm for MockLlm {
    async fn complete(
        &self,
        s: &str,
        u: &str,
        _t: crate::router::TaskType,
        _m: i32,
    ) -> Result<String, AppError> {
        self.systems.lock().unwrap().push(s.to_string());
        self.next(u)
    }

    async fn complete_json(
        &self,
        _s: &str,
        u: &str,
        _t: crate::router::TaskType,
        _m: i32,
    ) -> Result<String, AppError> {
        self.json_calls.lock().unwrap().push(u.to_string());
        self.next(u)
    }
}

/// Gate v2 时代的高分正文：≥800 字、低重复（编号句互不相同，与
/// graders 高分用例同一形态）、结尾悬念钩子 + 爽点（震惊）+ 微兑现
/// （果然/约定）信号——code≈1.0；rule 侧追读力已对齐生产口径
/// （每命中 +0.1），rule≈0.45，旧格式 pass 裁决（model 回退 0.85）
/// 加权 ≈0.76 仍过 0.75 阈值。
fn pass_grade_content(prefix: &str) -> String {
    let mut s = String::from(prefix);
    for i in 1..=24 {
        s.push_str(&format!(
            "第{i}拍里，{i}号巷的守夜人把{i}封旧信交给林雪，沈夜在{i}步之外停住，雨点打在刀背上。"
        ));
    }
    s.push_str("她果然没有忘记约定，全场震惊。下一秒，门外传来脚步声——真相究竟是谁留下的？");
    s
}

/// 一次通过（verdict=pass）的完整脚本：concept → producer(tool,final) →
/// writer(tool,final) → editor(final)
fn pass_script() -> Arc<MockLlm> {
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let write = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"拾荒者登场"}}}}"#,
        chapter
    );
    MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅"}"#,
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#,
        r#"{"type":"final","content":"资产就绪"}"#,
        write.as_str(),
        r#"{"type":"final","content":"第一章完成"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[\"可加强嗅觉描写\"],\"comments\":\"合格的首章\"}"}"#,
    ])
}

#[tokio::test]
async fn test_genesis_end_to_end_pass() {
    let pool = create_test_pool().unwrap();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), pass_script());
    let result = coordinator.run_genesis("r1", LONG_PREMISE).await.unwrap();
    assert!(!result.revised);
    // v0.30.35：editor 质检后台化，genesis 返回 pending 裁决（后台填充）
    assert_eq!(result.verdict.verdict, "pending");
    // run 状态 completed
    let repo = AgencyRepository::new(pool.clone());
    let run = repo.get_run("r1").unwrap().unwrap();
    assert_eq!(run.status, "completed");
    assert_eq!(run.story_id.as_deref(), Some(result.story_id.as_str()));
    // 黑板资产区与草稿区有内容（v0.30.35：editor 质检后台化，审查区前台为空）
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let snap = board.snapshot("r1").unwrap();
    assert_eq!(snap.assets.len(), 1);
    assert_eq!(snap.drafts.len(), 1);
    assert!(
        snap.reviews.is_empty(),
        "genesis 前台不再写审查区（editor 后台质检）: {:?}",
        snap.reviews
    );
    // Scene 已装配，正文来自草稿
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        scene.content.as_deref(),
        Some(pass_grade_content("第一章正文：风沙中的拾荒者。").as_str())
    );
    assert!(result.chapter_chars > 0);
}

// v0.30.35：test_genesis_revision_path 已移除--editor 质检后台化后 genesis
// 前台不再做修订（修订需主创 LLM 且可能再顶满超时，由用户据 toast 手动
// 重试）。修订路径仍在续写 handle_gate 中保留并经其测试覆盖。

#[tokio::test]
async fn test_genesis_aborts_when_producer_fails() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
        "不是 JSON",
        "还不是",
        "依然不是", // producer 连续解析失败 → aborted
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let err = coordinator
        .run_genesis("r3", LONG_PREMISE)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("管理")
            || err.to_string().contains("producer")
            || err.to_string().contains("熔断")
    );
    let repo = AgencyRepository::new(pool.clone());
    let run = repo.get_run("r3").unwrap().unwrap();
    assert_eq!(run.status, "failed");
}

/// 熔断错误消息必须带主因：连续 3 次解析失败 → "连续解析失败"。
#[tokio::test]
async fn test_circuit_break_message_includes_parse_failure_reason() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#,
        "不是 JSON",
        "还不是",
        "依然不是", // producer 连续解析失败 → aborted
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let err = coordinator
        .run_genesis("r-cb", LONG_PREMISE)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("连续解析失败"),
        "熔断消息应含主因: {}",
        err
    );
    assert!(err.to_string().contains("被熔断"), "保留熔断措辞: {}", err);
}

/// 熔断错误消息必须带主因：producer 反复调用工具不出 final、耗尽
/// max_turns（12）→ "达到最大轮数"。
#[tokio::test]
async fn test_circuit_break_message_includes_max_turns_reason() {
    let pool = create_test_pool().unwrap();
    let tool_call = r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#;
    let mut lines = vec![r#"{"title":"测试之书","genre":"科幻","logline":"x"}"#];
    lines.extend(std::iter::repeat(tool_call).take(12));
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(lines));
    let err = coordinator
        .run_genesis("r-mt", LONG_PREMISE)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("达到最大轮数"),
        "熔断消息应含主因: {}",
        err
    );
    assert!(err.to_string().contains("被熔断"), "保留熔断措辞: {}", err);
}

// v0.30.35：test_genesis_aborts_when_editor_aborted 已移除--editor 质检后台
// 化后不再阻塞 genesis（后台 spawn_editor_qc 在测试环境 no-op，editor 熔断
// 经 salvage_failed_gate 降级放行保产出，不再使 run failed）。

/// concept 响应后立即置取消 flag 的 mock（模拟用户在概念完成后取消）。
struct CancelAfterConceptLlm {
    inner: Arc<MockLlm>,
    run_id: String,
    fired: AtomicBool,
}

#[async_trait::async_trait]
impl LoopLlm for CancelAfterConceptLlm {
    async fn complete(
        &self,
        s: &str,
        u: &str,
        t: crate::router::TaskType,
        m: i32,
    ) -> Result<String, AppError> {
        let out = self.inner.complete(s, u, t, m).await?;
        if !self.fired.swap(true, Ordering::SeqCst) {
            assert!(cancel_agency_run(&self.run_id), "取消 flag 应已注册");
        }
        Ok(out)
    }
}

#[tokio::test]
async fn test_genesis_cancel_not_overwritten_by_completed() {
    let pool = create_test_pool().unwrap();
    let llm = Arc::new(CancelAfterConceptLlm {
        inner: pass_script(),
        run_id: "r5".to_string(),
        fired: AtomicBool::new(false),
    });
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let err = coordinator
        .run_genesis("r5", LONG_PREMISE)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("取消"), "应返回取消错误: {}", err);
    let repo = AgencyRepository::new(pool.clone());
    let run = repo.get_run("r5").unwrap().unwrap();
    assert_eq!(run.status, "cancelled");
    // 终态守护：cancelled 不得被 completed 覆盖
    repo.finish_run("r5", "completed", Some("{}"), None)
        .unwrap();
    let run = repo.get_run("r5").unwrap().unwrap();
    assert_eq!(run.status, "cancelled");
}

/// 快速路径脚本：concept pack（含 2 张角色卡）→ 深度资产 → 首章正文 →
/// 编辑裁决 pass。返回 (mock, 首章正文)。
fn fastpath_script() -> (Arc<MockLlm>, String) {
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅","characters":[{"name":"阿岩","background":"星环拾荒者","personality":"坚韧","goals":"寻找失散的妹妹"},{"name":"薇拉","background":"空间站医师","personality":"冷静","goals":"守住疫苗配方"}]}"#,
        r#"{"world":"双星废土，星环环绕，资源配给制","outline":"第一卷：拾荒者卷入星环阴谋","foreshadowing":["妹妹的项链（第三卷回收）"]}"#,
        chapter.as_str(),
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格的首章\"}"}"#,
    ]);
    (llm, chapter)
}

#[tokio::test]
async fn test_fastpath_multi_model() {
    let pool = create_test_pool().unwrap();
    let (llm, chapter) = fastpath_script();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(2);
    let result = coordinator
        .run_genesis("rf-multi", LONG_PREMISE)
        .await
        .unwrap();
    assert!(!result.revised);
    // v0.30.35：editor 质检后台化，genesis 返回 pending 裁决
    assert_eq!(result.verdict.verdict, "pending");
    assert!(result.chapter_chars >= 200);
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-multi").unwrap().unwrap().status,
        "completed"
    );
    // Scene 已装配，正文即首章单调用产出
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(scene.content.as_deref(), Some(chapter.as_str()));
    // 黑板资产区含 character + world + outline（v0.30.29 起串行
    // producer-first：producer 先写资产，writer 后写首章，不断言条目顺序）
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let snap = board.snapshot("rf-multi").unwrap();
    let types: Vec<&str> = snap.assets.iter().map(|i| i.item_type.as_str()).collect();
    assert!(
        types.contains(&"character"),
        "资产区应含角色卡: {:?}",
        types
    );
    assert!(types.contains(&"world"), "资产区应含世界观: {:?}", types);
    assert!(types.contains(&"outline"), "资产区应含大纲: {:?}", types);
    assert_eq!(snap.drafts.len(), 1);
}

#[tokio::test]
async fn test_fastpath_single_model_producer_first() {
    let pool = create_test_pool().unwrap();
    let (llm, chapter) = fastpath_script();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone()).with_model_count(1);
    let result = coordinator
        .run_genesis("rf-single", LONG_PREMISE)
        .await
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-single").unwrap().unwrap().status,
        "completed"
    );
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(scene.content.as_deref(), Some(chapter.as_str()));
    // 单模型调用顺序严格为 concept → producer（深度资产）→ writer → editor：
    // 队列按此序提供（顺序错则内容错配必然失败），此处再显式校验各次
    // 调用的提示词标记（run 完成后 finalize 可能追加摘要调用，故只校验
    // 前 3 次）。
    let calls = llm.calls.lock().unwrap();
    assert!(calls.len() >= 3, "至少 3 次 LLM 调用: {:?}", *calls);
    assert!(calls[0].contains("characters"), "第 1 次应为概念调用");
    assert!(
        calls[1].contains("foreshadowing"),
        "第 2 次应为深度资产（producer 先）"
    );
    assert!(calls[2].contains("写作要求"), "第 3 次应为首章写作");
}

#[tokio::test]
async fn test_fastpath_fallback_to_legacy() {
    let pool = create_test_pool().unwrap();
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let write = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"拾荒者登场"}}}}"#,
        chapter
    );
    // concept 返回非 JSON → 回退 legacy：producer(tool,final) →
    // writer(tool,final) → editor(final pass)。概念调用不重复。
    let llm = MockLlm::scripted(vec![
        "不是 JSON",
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#,
        r#"{"type":"final","content":"资产就绪"}"#,
        write.as_str(),
        r#"{"type":"final","content":"第一章完成"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(2);
    let result = coordinator
        .run_genesis("rf-fallback", LONG_PREMISE)
        .await
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-fallback").unwrap().unwrap().status,
        "completed"
    );
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(scene.content.as_deref(), Some(chapter.as_str()));
    assert!(result.chapter_chars >= 200);
}

/// v0.30.19: editor tool_loop 熔断（本地模型不遵从 JSON action）后，
/// 散文回退单次直接请求裁决 JSON 成功。v0.30.35：genesis 前台不再跑
/// editor 质检（后台 spawn_editor_qc），此用例改为直接调 evaluate_gate
/// 覆盖 editor_verdict_prose_fallback 路径（该函数仍被后台质检调用）。
#[tokio::test]
async fn test_editor_verdict_prose_fallback() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "散文回退书".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("rg-fb", "续写")).unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let draft = board
        .write(
            "rg-fb",
            &story.id,
            AgentRole::LeadWriter,
            BoardZone::Draft,
            "chapter",
            "第一章",
            &pass_grade_content("第一章正文：风沙中的拾荒者。"),
            "首章草稿",
        )
        .unwrap();
    // editor tool_loop: 连续 3 次散文（非 JSON action）-> ParseFailures 熔断
    // -> salvage parse 无果 -> 散文回退（单次 complete）：直接产出裁决 JSON
    let llm: Arc<dyn LoopLlm> = MockLlm::scripted(vec![
        "这不是JSON工具动作，只是审查意见散文。",
        "依然不是JSON action，本地模型不遵从。",
        "第三次散文，触发连续解析失败熔断。",
        r#"{"verdict":"pass","score":4.5,"blocking_issues":[],"suggestions":["可加强嗅觉描写"],"comments":"合格的首章"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let registry = Arc::new(ToolRegistry::agency_default());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let outcome = coordinator
        .evaluate_gate(
            &budget, &board, &registry, "rg-fb", &story.id, "续写", &draft, 1,
        )
        .await
        .unwrap();
    match outcome {
        GateOutcome::Passed { verdict } => {
            assert_eq!(
                verdict.verdict, "pass",
                "editor 熔断后散文回退应产出 pass 裁决"
            );
        }
        other => panic!("散文回退应产出 pass 裁决，实际: {:?}", other),
    }
}

/// Fix A：本地模型对 depth assets 返回散文而非 JSON 时，快速路径应兜底
/// salvage 散文为 world 资产，而非失败回退 legacy（legacy writer tool_loop
/// 要求 JSON action，对散文模型几乎必然熔断）。单模型序：concept ->
/// depth -> writer -> editor。
#[tokio::test]
async fn test_depth_assets_prose_salvaged() {
    let pool = create_test_pool().unwrap();
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    // depth assets 返回散文（无 JSON 花括号）-> parse_lenient 失败 -> 兜底
    let depth_prose = "双星废土，星环环绕，地表资源枯竭。人类聚居于轨道空间站，\
                       拾荒者穿梭废墟搜寻旧时代遗物。星环之上疫苗配方的争夺暗流涌动，\
                       失散妹妹的项链是唯一的线索。";
    let llm = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅","characters":[{"name":"阿岩","background":"星环拾荒者","personality":"坚韧","goals":"寻找失散的妹妹"},{"name":"薇拉","background":"空间站医师","personality":"冷静","goals":"守住疫苗配方"}]}"#,
        depth_prose,
        chapter.as_str(),
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格的首章\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(1);
    let result = coordinator
        .run_genesis("rf-prose-assets", LONG_PREMISE)
        .await
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-prose-assets").unwrap().unwrap().status,
        "completed",
        "散文 depth assets 应兜底成功，不回退 legacy"
    );
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(scene.content.as_deref(), Some(chapter.as_str()));
    // 散文应落为 world 资产（兜底 salvage）
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let snap = board.snapshot("rf-prose-assets").unwrap();
    let world = snap.assets.iter().find(|a| a.item_type == "world");
    assert!(
        world.is_some(),
        "资产区应含 world: {:?}",
        snap.assets.iter().map(|a| &a.item_type).collect::<Vec<_>>()
    );
    assert!(
        world.unwrap().content.contains("双星废土"),
        "world 资产应含散文内容"
    );
}

/// Fix C：legacy writer tool_loop 连续解析失败（本地模型写散文而非
/// JSON action）时，回退自由体散文单调用，避免整 run 失败。concept
/// 返回非 JSON -> legacy；producer(tool,final)；writer 散文 x3 -> 熔断
/// -> 散文回退；editor(final pass)。
#[tokio::test]
async fn test_legacy_writer_prose_fallback() {
    let pool = create_test_pool().unwrap();
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let llm = MockLlm::scripted(vec![
        "不是 JSON", // concept -> Err -> legacy
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#, /* producer tool */
        r#"{"type":"final","content":"资产就绪"}"#, // producer final
        "风沙漫天，拾荒者阿岩在废墟中穿行。这不是 JSON。", // writer prose #1 (parse fail)
        "他紧了紧面罩，目光扫过残骸。仍不是 JSON。", // writer prose #2 (parse fail)
        "远处传来机械的轰鸣。第三次散文输出。",     // writer prose #3 (parse fail -> 熔断)
        chapter.as_str(),                           // writer_prose_fallback 自由体散文
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#, /* editor final pass */
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(2);
    let result = coordinator
        .run_genesis("rf-prose-fallback", LONG_PREMISE)
        .await
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-prose-fallback").unwrap().unwrap().status,
        "completed",
        "writer 散文熔断后应回退自由体散文，run 不应失败"
    );
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        scene.content.as_deref(),
        Some(chapter.as_str()),
        "scene 内容应为散文回退产出的正文"
    );
    assert!(result.chapter_chars >= 200);
}

/// v0.30.4: asset_retrieval_plan 资产 ≤3 条时走短路，返回全部 key，
/// 不调用 LLM（省一次调用）。
#[tokio::test]
async fn test_asset_retrieval_plan_short_circuit_le3() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![]); // 不应被调用
    let coordinator = AgencyCoordinator::for_test(pool, llm.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let catalog = vec![
        ("世界观".to_string(), "双星废土".to_string()),
        ("阿岩".to_string(), "拾荒者".to_string()),
        ("大纲".to_string(), "第一卷".to_string()),
    ];
    let keys = coordinator
        .asset_retrieval_plan("r-sc", "s1", "前提", &budget, &catalog)
        .await
        .unwrap();
    assert_eq!(keys, vec!["世界观", "阿岩", "大纲"]);
    // 未消耗任何 LLM 响应
    assert!(
        llm.calls.lock().unwrap().is_empty(),
        "≤3 条应短路，不调 LLM"
    );
}

/// v0.30.4: asset_retrieval_plan 正常 JSON 路径--返回选中的 key。
#[tokio::test]
async fn test_asset_retrieval_plan_json_path() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![r#"{"keys":["世界观","阿岩"]}"#]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let catalog: Vec<(String, String)> = (1..=5)
        .map(|i| (format!("资产{}", i), format!("摘要{}", i)))
        .collect();
    let keys = coordinator
        .asset_retrieval_plan("r-json", "s1", "前提", &budget, &catalog)
        .await
        .unwrap();
    assert_eq!(keys, vec!["世界观", "阿岩"]);
}

/// v0.30.4: asset_retrieval_plan 散文兜底--模型返回非 JSON 时返回全部 key。
#[tokio::test]
async fn test_asset_retrieval_plan_prose_fallback() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec!["我觉得应该选世界观和主角，因为..."]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let catalog: Vec<(String, String)> = (1..=5)
        .map(|i| (format!("资产{}", i), format!("摘要{}", i)))
        .collect();
    let keys = coordinator
        .asset_retrieval_plan("r-prose", "s1", "前提", &budget, &catalog)
        .await
        .unwrap();
    assert_eq!(keys.len(), 5, "散文兜底应返回全部 key");
}

/// v0.30.4: asset_retrieval_plan 别名兼容--本地模型用 "selected" 键名。
#[tokio::test]
async fn test_asset_retrieval_plan_alias_compat() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec![r#"{"selected":["资产1","资产3"]}"#]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let catalog: Vec<(String, String)> = (1..=5)
        .map(|i| (format!("资产{}", i), format!("摘要{}", i)))
        .collect();
    let keys = coordinator
        .asset_retrieval_plan("r-alias", "s1", "前提", &budget, &catalog)
        .await
        .unwrap();
    assert_eq!(keys, vec!["资产1", "资产3"]);
}

/// v0.30.4: build_writer_assets_context 资产区为空 -> 返回空串（writer
/// 走原 tool_loop 路径）。
#[tokio::test]
async fn test_build_writer_assets_context_empty() {
    let pool = create_test_pool().unwrap();
    AgencyRepository::new(pool.clone())
        .create_run(&AgencyRun::new("r-empty", "前提"))
        .unwrap();
    let llm = MockLlm::scripted(vec![]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let ctx = coordinator
        .build_writer_assets_context("r-empty", "s1", "前提", &budget)
        .await;
    assert!(ctx.is_empty(), "空资产区应返回空串");
}

/// v0.30.4: build_writer_assets_context ≤3 条短路全量注入，按检索规划
/// 过滤拼接。
#[tokio::test]
async fn test_build_writer_assets_context_filters_by_plan() {
    let pool = create_test_pool().unwrap();
    AgencyRepository::new(pool.clone())
        .create_run(&AgencyRun::new("r-filt", "前提"))
        .unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    // 写入 5 条资产（>3 触发检索规划）
    for i in 1..=5 {
        board
            .write(
                "r-filt",
                "s1",
                AgentRole::Producer,
                BoardZone::Asset,
                "character",
                &format!("资产{}", i),
                &format!("内容{}", i),
                &format!("摘要{}", i),
            )
            .unwrap();
    }
    // 检索规划只选资产1 和资产3
    let llm = MockLlm::scripted(vec![r#"{"keys":["资产1","资产3"]}"#]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let ctx = coordinator
        .build_writer_assets_context("r-filt", "s1", "前提", &budget)
        .await;
    assert!(ctx.contains("资产1"), "应含选中的资产1: {}", ctx);
    assert!(ctx.contains("资产3"), "应含选中的资产3: {}", ctx);
    assert!(!ctx.contains("资产2"), "不应含未选中的资产2: {}", ctx);
    assert!(!ctx.contains("资产5"), "不应含未选中的资产5: {}", ctx);
}

/// v0.30.4: build_writer_assets_context 截断--资产总长超 8000 字符时
/// 截断并提示"更多资产已省略"。
#[tokio::test]
async fn test_build_writer_assets_context_truncates() {
    let pool = create_test_pool().unwrap();
    AgencyRepository::new(pool.clone())
        .create_run(&AgencyRun::new("r-trunc", "前提"))
        .unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    // 4 条资产，每条 3000 字符（>3 触发检索规划），总长 12000 > 8000
    let long_content: String = "长".repeat(3000);
    for i in 1..=4 {
        board
            .write(
                "r-trunc",
                "s1",
                AgentRole::Producer,
                BoardZone::Asset,
                "character",
                &format!("资产{}", i),
                &long_content,
                &format!("摘要{}", i),
            )
            .unwrap();
    }
    // 检索规划全选（兜底全量也行，这里显式全选触发截断）
    let llm = MockLlm::scripted(vec![r#"{"keys":["资产1","资产2","资产3","资产4"]}"#]);
    let coordinator = AgencyCoordinator::for_test(pool, llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let ctx = coordinator
        .build_writer_assets_context("r-trunc", "s1", "前提", &budget)
        .await;
    assert!(
        ctx.contains("更多资产已省略"),
        "超 8000 字符应截断并提示: ctx 长度 {}",
        ctx.chars().count()
    );
    assert!(
        ctx.chars().count() <= 8100,
        "截断后 ctx 不应远超 8000: {}",
        ctx.chars().count()
    );
}

/// v0.30.4: circuit_break_reason 识别 deadline 熔断主因。deadline 熔断
/// 时返回"剩余时间不足"，coordinator writer 路径据此快速失败而非回退
/// legacy writer_prose_fallback（后者单调用也会顶满超时无产出）。
#[test]
fn test_circuit_break_reason_identifies_deadline() {
    use crate::agency::tool_loop::{LoopAbortReason, LoopResult};
    // deadline 熔断
    let deadline_result = LoopResult {
        output: "（剩余时间不足，已熔断保产出）".to_string(),
        turns: vec![],
        aborted: true,
        abort_reason: Some(LoopAbortReason::Deadline),
    };
    assert_eq!(circuit_break_reason(&deadline_result), "剩余时间不足");
    // 连续解析失败
    let parse_result = LoopResult {
        output: "（代理连续输出非法格式，已熔断）".to_string(),
        turns: vec![],
        aborted: true,
        abort_reason: Some(LoopAbortReason::ParseFailures),
    };
    assert_eq!(circuit_break_reason(&parse_result), "连续解析失败");
    // 达到最大轮数
    let max_result = LoopResult {
        output: "（达到最大轮数，已熔断）".to_string(),
        turns: vec![],
        aborted: true,
        abort_reason: Some(LoopAbortReason::MaxTurns),
    };
    assert_eq!(circuit_break_reason(&max_result), "达到最大轮数");
    // 兜底：None + 末三轮解析失败 -> "连续解析失败"（向后兼容）
    let mut turns = vec![];
    for _ in 0..3 {
        turns.push(crate::agency::tool_loop::LoopTurn {
            raw_response: String::new(),
            action: None,
            observation: None,
        });
    }
    let legacy_result = LoopResult {
        output: String::new(),
        turns,
        aborted: true,
        abort_reason: None,
    };
    assert_eq!(
        circuit_break_reason(&legacy_result),
        "连续解析失败",
        "None 兜底应走末三轮启发式"
    );
}

/// v0.30.4: circuit_break_message 对 deadline 主因给出可读排查指引。
#[test]
fn test_circuit_break_message_deadline_detail() {
    let msg = circuit_break_message("主创 Agent", "首章未完成", "剩余时间不足");
    assert!(msg.contains("剩余时间不足"));
    assert!(msg.contains("被熔断"));
    assert!(
        msg.contains("调高超时上限"),
        "deadline 消息应含调高超时的指引: {}",
        msg
    );
}

/// 结构化单调用（concept pack / depth assets）必须走 complete_json
/// （JSON mode），散文首章不走。单模型模式调用序确定：concept ->
/// depth assets -> writer（散文） -> editor。
#[tokio::test]
async fn test_fastpath_structured_calls_use_json_mode() {
    let pool = create_test_pool().unwrap();
    let (llm, _chapter) = fastpath_script();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone()).with_model_count(1);
    coordinator
        .run_genesis("rf-json", LONG_PREMISE)
        .await
        .unwrap();
    let json_calls = llm.json_calls.lock().unwrap();
    assert_eq!(
        json_calls.len(),
        2,
        "concept + depth assets 两次 JSON 调用: {:?}",
        *json_calls
    );
    assert!(
        json_calls[0].contains("characters"),
        "第 1 次 JSON 调用应为概念包"
    );
    assert!(
        json_calls[1].contains("foreshadowing"),
        "第 2 次 JSON 调用应为深度资产"
    );
}

#[test]
fn test_depth_assets_lenient_keys_and_foreshadowing() {
    // world 别名 + 伏笔对象形态均可解析并归一为字符串
    let raw = r#"{"world_view":"双星废土","outline":"第一卷大纲","foreshadowing":["妹妹的项链",{"description":"身世之谜"},{"text":"星环秘密"}]}"#;
    let assets: DepthAssets = parse_lenient(raw).unwrap();
    assert_eq!(assets.world, "双星废土");
    let normalized: Vec<String> = assets
        .foreshadowing
        .iter()
        .map(normalize_foreshadowing)
        .collect();
    assert_eq!(normalized, vec!["妹妹的项链", "身世之谜", "星环秘密"]);
    let raw2 = r#"{"worldview":"x"}"#;
    let a2: DepthAssets = parse_lenient(raw2).unwrap();
    assert_eq!(a2.world, "x");
    let raw3 = r#"{"world_setting":"y"}"#;
    let a3: DepthAssets = parse_lenient(raw3).unwrap();
    assert_eq!(a3.world, "y");
}

#[test]
fn test_seed_character_field_aliases() {
    let raw = r#"{"title":"测试","characters":[{"character_name":"阿苔","backstory":"拾荒者","character":"坚韧","motivation":"找到妹妹"}]}"#;
    let pack: ConceptPack = parse_lenient(raw).unwrap();
    let c = &pack.characters[0];
    assert_eq!(c.name, "阿苔");
    assert_eq!(c.background, "拾荒者");
    assert_eq!(c.personality, "坚韧");
    assert_eq!(c.goals, "找到妹妹");
}

/// 第 N 次 LLM 调用后触发取消的 mock（fastpath 取消窗口测试用）。
struct CancelOnCallLlm {
    inner: Arc<MockLlm>,
    run_id: String,
    fire_on: usize,
    count: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl LoopLlm for CancelOnCallLlm {
    async fn complete(
        &self,
        s: &str,
        u: &str,
        t: crate::router::TaskType,
        m: i32,
    ) -> Result<String, AppError> {
        let out = self.inner.complete(s, u, t, m).await?;
        if self.count.fetch_add(1, Ordering::SeqCst) + 1 >= self.fire_on {
            assert!(cancel_agency_run(&self.run_id), "取消 flag 应已注册");
        }
        Ok(out)
    }
}

/// 取消信号不得被路由进 legacy 回退：fastpath Phase B 窗口取消 → 直接
/// 传播（无 fallback warn、无 legacy 接手），run 终态 cancelled。
#[tokio::test]
async fn test_fastpath_cancel_not_routed_to_legacy() {
    let pool = create_test_pool().unwrap();
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let inner = MockLlm::scripted(vec![
        r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅","characters":[{"name":"阿岩","background":"星环拾荒者","personality":"坚韧","goals":"寻找失散的妹妹"}]}"#,
        r#"{"world":"双星废土","outline":"第一卷：拾荒者卷入星环阴谋","foreshadowing":["妹妹的项链"]}"#,
        chapter.as_str(),
        // legacy 若接手会消费此条（不应发生）
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#,
    ]);
    // writer 调用（第 3 次：concept -> depth -> writer）后即触发取消
    // （Phase B 窗口：producer 先、writer 后，串行编排下 writer 是第 3 次调用）
    let llm = Arc::new(CancelOnCallLlm {
        inner: inner.clone(),
        run_id: "rf-cancel".to_string(),
        fire_on: 3,
        count: std::sync::atomic::AtomicUsize::new(0),
    });
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(2);
    let err = coordinator
        .run_genesis("rf-cancel", LONG_PREMISE)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("取消"), "应返回取消错误: {}", err);
    let repo = AgencyRepository::new(pool.clone());
    assert_eq!(
        repo.get_run("rf-cancel").unwrap().unwrap().status,
        "cancelled"
    );
    // legacy 未接手：无 legacy producer ToolLoop 调用（其任务提示词未出现）
    let calls = inner.calls.lock().unwrap();
    assert!(
        calls.iter().all(|c| !c.contains("请为本故事生产创世资产")),
        "legacy producer 不应被调用: {:?}",
        *calls
    );
    drop(calls);
    // 第 4 条脚本未被消费（无 assets 快照 → finalize 无 LLM 调用，计数确定）
    assert_eq!(inner.responses.lock().unwrap().len(), 1);
}

#[test]
fn test_parse_lenient_json() {
    let v: EditorVerdict =
        parse_lenient("前言{\"verdict\":\"revise\",\"blocking_issues\":[\"a\"]}后缀").unwrap();
    assert_eq!(v.verdict, "revise");
    assert!(parse_lenient::<EditorVerdict>("无 JSON").is_none());
}

#[test]
fn test_parse_lenient_strips_markdown_code_fence_with_trailing_brace() {
    // issue #14：JSON 被 ```json 代码块包裹，且代码块后有额外说明文本含 `}`。
    // 旧 parse_lenient 用 rfind('}') 会截到尾部杂散 `}`，from_str 失败 ->
    // None。 现 extract_and_sanitize_json
    // 做括号深度匹配，只取首个完整对象。
    let raw = "好的，以下是世界观：\n```json\n{\"verdict\":\"pass\",\"blocking_issues\":[]}\n```\n注意：以上为 JSON}。";
    let v: EditorVerdict = parse_lenient(raw).expect("应剥离围栏并提取首个完整 JSON 对象");
    assert_eq!(v.verdict, "pass");
    assert!(v.blocking_issues.is_empty());
}

#[test]
fn test_parse_lenient_repairs_unescaped_newline_in_string() {
    // issue #14：模型在 JSON 字符串值内直接换行（未转义 \n）。旧 parse_lenient
    // 的首尾花括号截取无法修复，serde_json::from_str 静默失败；现经
    // extract_and_sanitize_json 将字符串内裸换行修复为转义 \n。
    let raw = "```json\n{\"world\":\"双星废土\n文明\",\"outline\":\"大纲\"}\n```";
    let a: DepthAssets = parse_lenient(raw).expect("围栏 + 字符串内裸换行应被修复后解析");
    assert_eq!(a.world, "双星废土\n文明");
}

#[test]
fn test_verdict_with_rubric_scores() {
    let raw = r#"{"verdict":"pass","score":4.2,"dimension_scores":{"continuity":4.5,"style":4.0,"contract":4.0,"ai_tone":4.5,"hook":3.8},"blocking_issues":[],"suggestions":[],"comments":"好"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    assert_eq!(v.verdict, "pass");
    let report = ModelGraderReport::from_verdict(&v);
    assert!((report.model_score - 0.84).abs() < 0.001); // 4.2/5
    assert_eq!(report.dimension_scores.get("continuity"), Some(&4.5));
}

#[test]
fn test_verdict_legacy_format_fallback() {
    // 旧格式（无 score 字段）向后兼容
    let raw =
        r#"{"verdict":"revise","blocking_issues":["动机缺失"],"suggestions":[],"comments":"修"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    assert!(v.score.is_none());
    let report = ModelGraderReport::from_verdict(&v);
    assert!((report.model_score - 0.4).abs() < 0.001);
    assert!(
        (ModelGraderReport::from_verdict(&EditorVerdict {
            verdict: "pass".into(),
            blocking_issues: vec![],
            suggestions: vec![],
            comments: String::new(),
            score: None,
            dimension_scores: None,
        })
        .model_score
            - 0.7)
            .abs()
            < 0.001
    ); // v0.30.30：scoreless pass 兜底从 0.85 降到 0.7
}

#[test]
fn test_evidence_issues_collected() {
    let raw = r#"{"verdict":"revise","score":2.0,"blocking_issues":[{"issue":"角色动机断裂","evidence":"「他突然放弃复仇」"}],"suggestions":[],"comments":"修"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    let report = ModelGraderReport::from_verdict(&v);
    assert!(report
        .evidence_issues
        .iter()
        .any(|i| i.contains("角色动机断裂")));
}

#[test]
fn editor_issue_parses_impact_and_fix() {
    let raw = r#"{"verdict":"revise","score":2.0,"blocking_issues":[{"issue":"说明性对白","evidence":"「让我告诉你十年前的秘密」","impact":"场面停在解说","fix":"改成试探与回避"}],"suggestions":[],"comments":"修"}"#;
    let v: EditorVerdict = parse_lenient(raw).unwrap();
    let (issue, evidence, impact, fix) = blocking_issue_parts(&v.blocking_issues[0]);
    assert_eq!(issue, "说明性对白");
    assert!(evidence.contains("十年前"));
    assert_eq!(impact, "场面停在解说");
    assert_eq!(fix, "改成试探与回避");
    let old = serde_json::json!({"issue":"角色动机断裂","evidence":"「他突然放弃复仇」"});
    let (i2, e2, p2, f2) = blocking_issue_parts(&old);
    assert_eq!(i2, "角色动机断裂");
    assert!(e2.contains("放弃复仇"));
    assert!(p2.is_empty());
    assert!(f2.is_empty());
}

#[test]
fn test_request_registry_lifecycle() {
    let run = "run-registry-test";
    register_request(run, "req-1");
    register_request(run, "req-2");
    register_request("other-run", "req-x");
    // 收集并清空目标 run 的全部 request_id
    let drained = drain_requests(run);
    assert_eq!(drained.len(), 2);
    assert!(drained.contains(&"req-1".to_string()));
    assert!(drained.contains(&"req-2".to_string()));
    // 已清空，再取为空
    assert!(drain_requests(run).is_empty());
    // 其他 run 不受影响
    assert_eq!(drain_requests("other-run"), vec!["req-x".to_string()]);
}

#[test]
fn test_unregister_request() {
    register_request("run-u", "req-a");
    unregister_request("run-u", "req-a");
    assert!(drain_requests("run-u").is_empty());
}

#[test]
fn test_validate_premise() {
    assert!(validate_premise("一个关于星海拾荒者的故事").is_ok());
    assert!(validate_premise("").is_err());
    assert!(validate_premise("   ").is_err());
    let too_long = "长".repeat(2001);
    assert!(validate_premise(&too_long).is_err());
    let at_limit = "长".repeat(2000);
    assert!(validate_premise(&at_limit).is_ok());
}

// v0.30.35：test_gate_fails_after_verdict_parse_retry 已移除--genesis 前台
// 不再跑 editor 质检（后台 spawn_editor_qc 在测试环境 no-op），editor 解析
// 失败不再使 genesis run failed。editor 失败/熔断路径仍由续写 handle_gate 与
// 直接 evaluate_gate 测试覆盖。

/// 修订指令（纯函数）须携带 item_id 与 expected_version，供 board_revise
/// 原地修订。
#[test]
fn test_build_revision_task_contains_item_ref() {
    let draft = BoardItem::new(
        "r",
        "s",
        BoardZone::Draft,
        "chapter",
        "第一章",
        "初稿。",
        "初稿",
        AgentRole::LeadWriter,
        "active",
    );
    let task = AgencyCoordinator::build_revision_task(&draft, &["动机缺失".to_string()]);
    assert!(task.contains("board_revise"));
    assert!(task.contains(&format!("item_id={}", draft.id)));
    assert!(task.contains("expected_version=1"));
    assert!(task.contains("动机缺失"));
}

#[tokio::test]
async fn test_continue_chapter_end_to_end() {
    let pool = create_test_pool().unwrap();
    // 预置故事 + 一个角色 + 第一章场景
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "续写书".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        // v0.30.21: 预置世界观与故事大纲
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    let llm = MockLlm::scripted(vec![
        // generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。场景：对话与追逐交替。",
        // write_beat_once 单次 complete：直接散文正文
        chapter2.as_str(),
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator
        .run_continue(
            "rc-1",
            &story.id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.chapter_number, 2);
    // Task 3 契约：NextChapter 装配后立刻返回，编辑质检后台化，
    // 前台裁决恒为 pending（不再同步等 evaluate_gate）
    assert_eq!(result.verdict.verdict, "pending");
    assert!(!result.revised, "单次 complete 路径不跑修订轮");
    let scene = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        scene.content.as_deref().map(str::trim_end),
        Some(chapter2.trim_end())
    );
    let run = AgencyRepository::new(pool.clone())
        .get_run("rc-1")
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed");
}

#[tokio::test]
async fn test_run_continue_append_keeps_scene_and_releases_run() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "同章追加".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("<p>第一章旧文。</p>".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let beat1 = pass_grade_content("第一拍增量：雨巷对峙。");
    let beat2 = pass_grade_content("第二拍增量：林雪归来。");
    let mock = MockLlm::scripted(vec![beat1.as_str(), beat2.as_str()]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock.clone());
    let r1 = coordinator
        .run_continue(
            "ap-1",
            &story.id,
            PersistMode::Append {
                scene_id: ch1.id.clone(),
            },
            "续写",
            Some("<p>第一章旧文。</p>"),
        )
        .await
        .unwrap();
    assert_eq!(r1.scene_id, ch1.id);
    assert!(r1.increment.chars().count() >= 200);
    // Task 3 契约：Append 装配后立刻返回，编辑质检后台 spawn，
    // 前台裁决恒为 pending
    assert_eq!(r1.verdict.verdict, "pending");
    let scenes = scene_repo.get_by_story(&story.id).unwrap();
    assert_eq!(scenes.len(), 1, "Append 不得新建 scenes 行");
    let run1 = AgencyRepository::new(pool.clone())
        .get_run("ap-1")
        .unwrap()
        .unwrap();
    assert_eq!(run1.status, "completed", "装配后必须立即释放 active run");
    let assets = crate::agency::board::BlackboardService::new(pool.clone())
        .list_zone("ap-1", crate::agency::models::BoardZone::Asset)
        .unwrap();
    assert!(
        !assets.is_empty(),
        "Append 落库后当前 run 资产栏应有节拍投影: {assets:?}"
    );
    assert_eq!(
        mock.calls.lock().unwrap().len(),
        1,
        "主创默认单次 complete；测试环境不得再抽 mock 做收尾摘要"
    );

    let r2 = coordinator
        .run_continue(
            "ap-2",
            &story.id,
            PersistMode::Append {
                scene_id: ch1.id.clone(),
            },
            "再续",
            Some(scenes[0].content.as_deref().unwrap_or("")),
        )
        .await
        .unwrap();
    assert_eq!(r2.scene_id, ch1.id);
    let scenes2 = scene_repo.get_by_story(&story.id).unwrap();
    assert_eq!(scenes2.len(), 1);
    let run2 = AgencyRepository::new(pool.clone())
        .get_run("ap-2")
        .unwrap()
        .unwrap();
    assert_eq!(run2.status, "completed");
}

#[test]
fn pass_grade_content_survives_sanitize_and_self_repeat_gate() {
    let text = pass_grade_content("前缀。");
    let sanitized = crate::agents::orchestrator::sanitize_novel_output(&text);
    assert!(
        sanitized.chars().count() >= 200,
        "sanitize 后仍须 ≥200，否则 write_beat_once 会误走散文回退抽干 mock"
    );
    let trimmed = crate::utils::text::TextUtils::trim_self_repetition(&sanitized);
    let ratio = crate::agents::trim_utils::compute_trim_ratio(
        sanitized.chars().count(),
        trimmed.chars().count(),
    );
    assert!(
        !crate::agents::trim_utils::should_retry_self_repetition(ratio, sanitized.chars().count()),
        "合格稿 helper 不得误触发 8% 重试"
    );
}

#[tokio::test]
async fn test_write_beat_retries_once_on_self_repetition() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "自重复重试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("<p>旧文。</p>".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let repeated = "林雪站在雨巷里把刀横在沈夜喉前，雨水灌进领口。".repeat(15);
    let unique = pass_grade_content("重试后干净增量：");
    let trimmed = crate::utils::text::TextUtils::trim_self_repetition(&repeated);
    let ratio = crate::agents::trim_utils::compute_trim_ratio(
        repeated.chars().count(),
        trimmed.chars().count(),
    );
    assert!(crate::agents::trim_utils::should_retry_self_repetition(
        ratio,
        repeated.chars().count()
    ));

    let mock = MockLlm::scripted(vec![repeated.as_str(), unique.as_str()]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock.clone());
    let result = coordinator
        .run_continue(
            "retry-1",
            &story.id,
            PersistMode::Append {
                scene_id: ch1.id.clone(),
            },
            "续写",
            Some("<p>旧文。</p>"),
        )
        .await
        .unwrap();
    assert_eq!(
        mock.calls.lock().unwrap().len(),
        2,
        "8% 闸门必须再 complete 一次"
    );
    assert!(
        result.increment.contains("重试后干净增量"),
        "应采用更干净的重试稿"
    );
    assert_eq!(scene_repo.get_by_story(&story.id).unwrap().len(), 1);
}

/// v0.30.20: 续写 writer 连续解析失败 -> 散文回退保产出（与 genesis
/// test_legacy_writer_prose_fallback 对称，但走 run_continue 路径）。
#[tokio::test]
async fn test_continue_writer_prose_fallback() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "续写散文回退".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        // v0.30.21: 预置世界观与故事大纲
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    let llm = MockLlm::scripted(vec![
        // generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。场景：对话与追逐交替。",
        "短",              // write_beat_once 过短
        chapter2.as_str(), // 续写回退（同组装，非创世 prose）
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator
        .run_continue(
            "rc-prose",
            &story.id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.chapter_number, 2);
    let scene = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        scene.content.as_deref().map(str::trim_end),
        Some(chapter2.trim_end()),
        "scene content should be prose fallback output"
    );
    let run = AgencyRepository::new(pool.clone())
        .get_run("rc-prose")
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed");
}

/// 散文回退仍过短时，不得再进入 `write_chapter` tool_loop。
/// 诊断：complete() 空 CoT → 散文过短 → tool_loop 重烧同一膨胀 prompt，
/// 直到前端 600s 看门狗取消。
#[tokio::test]
async fn test_continue_prose_fallback_failure_does_not_enter_tool_loop() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "续写不再进 tool_loop".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let llm = MockLlm::scripted(vec!["本章核心冲突：阿苔发现星环秘密。", "短", "也短"]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let err = coordinator
        .run_continue(
            "rc-no-loop",
            &story.id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("过短"),
        "应返回散文回退过短，实际: {err}"
    );
    assert_eq!(
        llm.calls.lock().unwrap().len(),
        3,
        "大纲 + complete + 续写回退；不得再进 write_chapter（会再调大纲/tool_loop）"
    );
    assert_eq!(
        AgencyRepository::new(pool)
            .get_run("rc-no-loop")
            .unwrap()
            .unwrap()
            .status,
        "failed"
    );
}

/// v0.30.20: build_continue_writer_context reads characters/world/scenes
/// from DB, pre-injecting into writer task (eliminates board_read polling).
#[tokio::test]
async fn test_build_continue_writer_context() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "上下文测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        // v0.30.21: 预置世界观与故事大纲
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        // v0.30.22: 预置 logline（PROBLEM 框架生成的核心方向）
        conn.execute(
            "UPDATE stories SET logline = ?1 WHERE id = ?2",
            rusqlite::params![
                "当一个废土拾荒者发现星环坐标后，必须穿越辐射区，否则家园将毁灭。",
                story.id
            ],
        )
        .unwrap();
        // v0.30.29: 预置 MASTER_SETTING 合同（续写红线注入测试）
        conn.execute(
            "INSERT INTO story_contracts (id, story_id, contract_type, contract_json, version, created_at, updated_at)
             VALUES ('ct1', ?1, 'MASTER_SETTING', ?2, 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![
                story.id,
                r#"{"world_rules":"禁止时间旅行；魔法须付出等价代价"}"#
            ],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文内容。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let coordinator = AgencyCoordinator::for_test(pool, MockLlm::scripted(vec![]));
    let ctx = coordinator.build_continue_writer_context(&story.id).await;
    assert!(
        ctx.contains("阿苔"),
        "context should contain character name"
    );
    assert!(
        ctx.contains("第一章正文内容"),
        "context should contain scene content"
    );
    // v0.30.21: 故事大纲注入
    assert!(
        ctx.contains("【故事大纲"),
        "context should contain story outline section"
    );
    assert!(
        ctx.contains("寻找星环"),
        "context should contain story outline content"
    );
    // v0.30.22: logline 注入
    assert!(
        ctx.contains("【故事Logline】"),
        "context should contain logline section"
    );
    assert!(
        ctx.contains("废土拾荒者"),
        "context should contain logline content"
    );
    // v0.30.29: MASTER_SETTING 红线应注入到 ctx 头部（最前最突出，对齐 C 链路
    // WriteTimeBundle.to_prompt 不变量；agency 续写此前完全绕过红线）
    assert!(
        ctx.contains("【⚠️ 世界观红线"),
        "context should contain MASTER_SETTING redline header"
    );
    assert!(
        ctx.contains("禁止时间旅行"),
        "context should contain world_rules redline content"
    );
    let redline_pos = ctx.find("世界观红线").unwrap();
    let char_pos = ctx.find("阿苔").unwrap();
    assert!(
        redline_pos < char_pos,
        "redline must precede character section (got redline@{} vs char@{})",
        redline_pos,
        char_pos
    );
}

/// v0.30.22: 简单前提（< 100 字符）触发 PROBLEM logline 生成。
/// MockLlm 队列首位为 logline 响应，后续为 fastpath 脚本。
/// 验证：首次 LLM 调用是 logline 生成（含 PROBLEM），且 logline 持久化到
/// DB。
#[tokio::test]
async fn test_generate_logline_from_simple_premise() {
    let pool = create_test_pool().unwrap();
    let logline =
        "当一个废土拾荒者发现星海遗迹的坐标后，必须穿越辐射区抵达遗迹，否则遗迹的秘密将永远埋没。";
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let llm = MockLlm::scripted(vec![
        logline,
        r#"{"title":"测试之书","genre":"科幻","logline":"拾荒者的星环之旅","characters":[{"name":"阿岩","background":"星环拾荒者","personality":"坚韧","goals":"寻找失散的妹妹"},{"name":"薇拉","background":"空间站医师","personality":"冷静","goals":"守住疫苗配方"}]}"#,
        r#"{"world":"双星废土","outline":"第一卷：拾荒者卷入星环阴谋","foreshadowing":["妹妹的项链（第三卷回收）"]}"#,
        chapter.as_str(),
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格的首章\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone()).with_model_count(1);
    let result = coordinator
        .run_genesis("rf-logline", "写一部科幻小说")
        .await
        .unwrap();
    // 首次调用应为 logline 生成（user prompt 含 PROBLEM）
    let calls = llm.calls.lock().unwrap();
    assert!(
        calls[0].contains("PROBLEM") || calls[0].contains("logline"),
        "首次调用应为 logline 生成: {}",
        &calls[0][..calls[0].len().min(80)]
    );
    // 第二次调用为 concept pack（含 characters）
    assert!(
        calls[1].contains("characters"),
        "第二次调用应为概念包: {}",
        &calls[1][..calls[1].len().min(80)]
    );
    drop(calls);
    // logline 持久化到 stories.logline
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .get_by_id(&result.story_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        story.logline.as_deref(),
        Some(logline),
        "logline 应持久化到 stories.logline"
    );
}

/// v0.30.22: 长前提（≥ 100 字符）跳过 PROBLEM logline 生成。
/// MockLlm 队列与 fastpath_script 一致（无 logline 响应）。
/// 验证：首次 LLM 调用是 concept pack（非 logline），且 stories.logline 为
/// None。
#[tokio::test]
async fn test_generate_logline_skipped_for_long_premise() {
    let pool = create_test_pool().unwrap();
    let (llm, _chapter) = fastpath_script();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone()).with_model_count(1);
    let result = coordinator
        .run_genesis("rf-no-logline", LONG_PREMISE)
        .await
        .unwrap();
    // 首次调用应为 concept pack（含 characters），非 logline
    let calls = llm.calls.lock().unwrap();
    assert!(
        calls[0].contains("characters"),
        "长前提首次调用应为概念包: {}",
        &calls[0][..calls[0].len().min(80)]
    );
    assert!(!calls[0].contains("PROBLEM"), "长前提不应触发 logline 生成");
    drop(calls);
    // logline 不应生成
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .get_by_id(&result.story_id)
        .unwrap()
        .unwrap();
    assert!(
        story.logline.is_none(),
        "长前提不应生成 logline: {:?}",
        story.logline
    );
}

/// v0.30.22: genesis 完成后 PROBLEM logline 持久化到 stories.logline。
/// 与 test_generate_logline_from_simple_premise 互补：该测试聚焦
/// DB 持久化正确性（update_logline 在 genesis 成功后执行）。
#[tokio::test]
async fn test_logline_stored_after_genesis() {
    let pool = create_test_pool().unwrap();
    let logline =
        "当一名星际考古学家发现远古文明的警告信号后，必须在七十二小时内解码，否则地球将被吞噬。";
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let llm = MockLlm::scripted(vec![
        logline,
        r#"{"title":"解码者","genre":"科幻","logline":"考古学家的七十二小时","characters":[{"name":"林深","background":"星际考古学家","personality":"执着","goals":"解码警告信号"}]}"#,
        r#"{"world":"星际联邦时代，远古文明遗迹遍布","outline":"第一卷：信号解码","foreshadowing":["远古文明的最后一行文字"]}"#,
        chapter.as_str(),
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(1);
    let result = coordinator
        .run_genesis("rf-logline-store", "写一部科幻小说")
        .await
        .unwrap();
    // v0.30.35：editor 质检后台化，genesis 返回 pending 裁决
    assert_eq!(result.verdict.verdict, "pending");
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .get_by_id(&result.story_id)
        .unwrap()
        .unwrap();
    assert!(story.logline.is_some(), "genesis 完成后 logline 应已持久化");
    assert!(
        story
            .logline
            .as_deref()
            .is_some_and(|l| l.contains("考古学家")),
        "logline 内容应包含考古学家: {:?}",
        story.logline
    );
}

#[tokio::test]
async fn test_continue_fails_without_assets_and_producer_aborts() {
    // v0.49: 管理熔断不再以「资产补齐未完成」挡住续写；
    // 空书仍可能在写作阶段失败。
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "无资产书".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let llm = MockLlm::scripted(vec!["不是 JSON", "还不是", "依然不是"]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(&budget, &repo, "rc-2-assets", &story.id, "续写")
        .await
        .expect("管理熔断不得让 ensure_assets 失败");
}

/// T3 遗留修复：build_review_context 填充 previous_chapters 后，
/// 规则复检（ContinuityAgent 重复开头检查 → High）必须能拦截 editor
/// 放行的草稿。Gate v2 下为双通道：spec 5.5 High+ 硬拦截（本案主命
/// 中——subagent_issues 非空即 RevisionRequired）+ 规则 6 低加权分
/// 兜底（本用例正文极短，weighted 同样低于阈值）。
#[tokio::test]
async fn test_gate_rule_recheck_blocks_repeated_opening() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "复检书".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置第一章场景，后续草稿开头与其高度重复
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(
                    "风沙掠过双星废土的清晨，阿苔在残骸中醒来，耳边是磁力风暴的低鸣。".to_string(),
                ),
                ..Default::default()
            },
        )
        .unwrap();

    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("rg-1", "续写")).unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let draft = board
        .write(
            "rg-1",
            &story.id,
            AgentRole::LeadWriter,
            BoardZone::Draft,
            "chapter",
            "第二章",
            "风沙掠过双星废土的清晨，阿苔在残骸中醒来，这一次她抬头看到了星环。",
            "第二章草稿",
        )
        .unwrap();

    // editor 放行（pass）；门应被规则复检拦下 → RevisionRequired
    let llm: Arc<dyn LoopLlm> = MockLlm::scripted(vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let registry = Arc::new(ToolRegistry::agency_default());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let outcome = coordinator
        .evaluate_gate(
            &budget, &board, &registry, "rg-1", &story.id, "续写", &draft, 1,
        )
        .await
        .unwrap();
    match outcome {
        GateOutcome::RevisionRequired { issues, .. } => {
            assert!(
                issues.iter().any(|i| i.contains("重复")),
                "规则复检应报告重复开头问题: {:?}",
                issues
            );
        }
        other => panic!("规则复检应拦截重复开头的草稿，实际: {:?}", other),
    }
}

/// 按系统提示词路由的 mock：区分 主创/编辑/管理
/// 三队列，且记录调用时间窗用于并发断言。
struct RoutingMock {
    writer: Mutex<VecDeque<String>>,
    editor: Mutex<VecDeque<String>>,
    producer: Mutex<VecDeque<String>>,
    intervals: Mutex<Vec<(String, std::time::Instant, std::time::Instant)>>,
    delay_ms: u64,
}

impl RoutingMock {
    fn new(delay_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            writer: Mutex::new(VecDeque::new()),
            editor: Mutex::new(VecDeque::new()),
            producer: Mutex::new(VecDeque::new()),
            intervals: Mutex::new(Vec::new()),
            delay_ms,
        })
    }
    fn push(&self, role: &str, lines: Vec<&str>) {
        let q = match role {
            "writer" => &self.writer,
            "editor" => &self.editor,
            _ => &self.producer,
        };
        q.lock()
            .unwrap()
            .extend(lines.into_iter().map(String::from));
    }
}

#[async_trait::async_trait]
impl LoopLlm for RoutingMock {
    async fn complete(
        &self,
        system: &str,
        _u: &str,
        _t: crate::router::TaskType,
        _m: i32,
    ) -> Result<String, AppError> {
        // 按角色标记路由（真实种子提示词与内置回退提示词均以 你是「角色」开头；
        // 不能裸判 "编辑"——writer 提示词中也含「编辑审计」字样）
        let role = if system.contains("你是「编辑审计」") {
            "editor"
        } else if system.contains("你是「主创」") {
            "writer"
        } else {
            "producer"
        };
        let start = std::time::Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        let out = {
            let q = match role {
                "editor" => &self.editor,
                "writer" => &self.writer,
                _ => &self.producer,
            };
            q.lock().unwrap().pop_front().ok_or_else(|| {
                AppError::validation_failed(format!("mock[{}] exhausted", role), None::<String>)
            })?
        };
        self.intervals
            .lock()
            .unwrap()
            .push((role.to_string(), start, std::time::Instant::now()));
        Ok(out)
    }
}

fn seed_story_with_assets(pool: &crate::db::DbPool) -> String {
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "并行书".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
         VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
        rusqlite::params![story.id],
    ).unwrap();
    // v0.30.21: 预置世界观与故事大纲（避免 ensure_assets 触发 LLM 生成，
    // 消耗 mock 队列响应）
    conn.execute(
        "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
         VALUES ('w1', ?1, '双星文明：资源匮乏的拾荒世界', '[]', '星环崩塌后残存文明争夺资源', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
        rusqlite::params![story.id],
    ).unwrap();
    conn.execute(
        "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
         VALUES ('o1', ?1, '核心冲突：阿苔寻找星环秘密。三幕结构：起因-发展-高潮。转折点：盟友背叛。推进方向：前往禁区。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
        rusqlite::params![story.id],
    ).unwrap();
    story.id
}

#[tokio::test]
async fn test_batch_parallel_two_chapters() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    // v0.30.21: 删除故事大纲以跳过 generate_chapter_outline（避免 Producer
    // LLM 调用的 60ms delay 破坏 gate(1) ∥ writer(2) 并行时序）
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "DELETE FROM story_outlines WHERE story_id = ?1",
            rusqlite::params![&story_id],
        )
        .unwrap();
    }
    let mock = RoutingMock::new(60);
    let write1 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"一"}}}}"#,
        pass_grade_content("第一章正文。")
    );
    let write2 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"二"}}}}"#,
        pass_grade_content("第二章正文。")
    );
    mock.push(
        "writer",
        vec![
            write1.as_str(),
            r#"{"type":"final","content":"第一章完成"}"#,
            write2.as_str(),
            r#"{"type":"final","content":"第二章完成"}"#,
        ],
    );
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好1\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好2\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock.clone());
    let result = coordinator
        .run_continue_batch("rb-1", &story_id, 1, 2)
        .await
        .unwrap();
    assert_eq!(result.chapters.len(), 2);
    // 两章场景均落库
    let scenes = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_story(&story_id)
        .unwrap();
    assert_eq!(scenes.len(), 2);
    // 并发证据：gate1(editor) 与 writer2 的时间窗存在交叠
    let intervals = mock.intervals.lock().unwrap();
    let editor_first = intervals.iter().find(|(r, _, _)| r == "editor").unwrap();
    let writer_windows: Vec<_> = intervals.iter().filter(|(r, _, _)| r == "writer").collect();
    let overlapped = writer_windows
        .iter()
        .any(|(_, s, e)| *s < editor_first.2 && editor_first.1 < *e);
    assert!(overlapped, "gate(1) 应与 writer(2) 并发: {:?}", *intervals);
    let run = AgencyRepository::new(pool.clone())
        .get_run("rb-1")
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed");
}

#[tokio::test]
async fn test_batch_revision_sends_bus_proposal() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(0);
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第1章","content":"初稿。","summary":"一"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        // 修订轮：mock 无法预知 board_revise 所需的动态 item_id，
        // 用 final 直接返回（draft 未变，第二轮 gate pass 放行）；
        // board_revise 语义已由 Task 2 测试覆盖，本用例只断言 bus 消息与放行。
        r#"{"type":"final","content":"已知晓修订意见"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"动机弱\"],\"suggestions\":[],\"comments\":\"修\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"过\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let result = coordinator
        .run_continue_batch("rb-2", &story_id, 1, 1)
        .await
        .unwrap();
    assert_eq!(result.chapters.len(), 1);
    assert!(result.chapters[0].revised);
    // 总线：editor→writer 的 proposal 消息存在
    let bus = crate::agency::bus::MessageBus::new(pool.clone());
    let inbox = bus.inbox("rb-2", AgentRole::LeadWriter).unwrap();
    assert!(inbox
        .iter()
        .any(|m| m.msg_type == "proposal" && m.payload.contains("动机弱")));
}

/// 修订回归用 mock：writer 修订轮（任务含「修订「第1章」」指引）动态读 DB
/// 取草稿 item_id，回 board_revise 原地更新——覆盖 board_revise
/// 模型行为；并行循环中 此时第 2 章草稿已在 draft 区，验证修订取稿按 key
/// 匹配、不跨章串稿。
struct ReviseAwareMock {
    inner: Arc<RoutingMock>,
    pool: crate::db::DbPool,
    run_id: String,
    fired: AtomicBool,
}

#[async_trait::async_trait]
impl LoopLlm for ReviseAwareMock {
    async fn complete(
        &self,
        system: &str,
        u: &str,
        t: crate::router::TaskType,
        m: i32,
    ) -> Result<String, AppError> {
        // 只拦截一次：对话上下文累计会保留任务文本，后续轮次须走队列取 final
        if system.contains("你是「主创」")
            && u.contains("修订「第1章」")
            && !self.fired.swap(true, Ordering::SeqCst)
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let (id, version): (String, i32) = conn
                .query_row(
                    "SELECT id, version FROM agency_board_items
                 WHERE run_id = ?1 AND zone = 'draft' AND key = '第1章'
                 ORDER BY rowid DESC LIMIT 1",
                    rusqlite::params![self.run_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|e| AppError::from(format!("draft lookup: {}", e)))?;
            return Ok(format!(
                r#"{{"type":"tool","name":"board_revise","args":{{"item_id":"{}","expected_version":{},"content":"第一章修订稿：阿苔的动机已补足。","summary":"一修"}}}}"#,
                id, version
            ));
        }
        self.inner.complete(system, u, t, m).await
    }
}

/// 回归：并行批量中第 1 章修订不得串第 2 章草稿
/// （board_revise 原地更新后 latest_draft 尾部是第 2 章——必须按 key
/// 取回）。
#[tokio::test]
async fn test_batch_revision_no_cross_chapter_mixup() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(0);
    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    let write2 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"二"}}}}"#,
        chapter2
    );
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第1章","content":"第一章初稿。","summary":"一"}}"#,
        r#"{"type":"final","content":"第一章完成"}"#,
        write2.as_str(),
        r#"{"type":"final","content":"第二章完成"}"#,
        // 修订轮第 2 步（board_revise 由 ReviseAwareMock 动态注入后的 final）
        r#"{"type":"final","content":"修订完成"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"动机弱\"],\"suggestions\":[],\"comments\":\"修\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"过1\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"过2\"}"}"#,
    ]);
    let revise_mock = Arc::new(ReviseAwareMock {
        inner: mock,
        pool: pool.clone(),
        run_id: "rb-3".to_string(),
        fired: AtomicBool::new(false),
    });
    let coordinator = AgencyCoordinator::for_test(pool.clone(), revise_mock);
    let result = coordinator
        .run_continue_batch("rb-3", &story_id, 1, 2)
        .await
        .unwrap();
    assert_eq!(result.chapters.len(), 2);
    assert!(result.chapters[0].revised, "第 1 章应经历修订");
    assert!(!result.chapters[1].revised, "第 2 章应一次通过");
    let scenes = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_story(&story_id)
        .unwrap();
    assert_eq!(scenes.len(), 2);
    let s1 = scenes.iter().find(|s| s.sequence_number == 1).unwrap();
    let s2 = scenes.iter().find(|s| s.sequence_number == 2).unwrap();
    assert_eq!(
        s1.content.as_deref(),
        Some("第一章修订稿：阿苔的动机已补足。"),
        "第 1 章 Scene 应装配修订后正文，不得串第 2 章草稿"
    );
    assert_eq!(
        s2.content.as_deref().map(str::trim_end),
        Some(chapter2.trim_end())
    );
    assert_ne!(s1.content, s2.content, "两章正文不得相同");
}

#[test]
fn test_build_bootstrap_result_contract() {
    let result = AgencyGenesisResult {
        run_id: "r1".into(),
        story_id: "story-9".into(),
        scene_id: "scene-3".into(),
        revised: false,
        verdict: EditorVerdict {
            verdict: "pass".into(),
            blocking_issues: vec![],
            suggestions: vec![],
            comments: "好".into(),
            score: None,
            dimension_scores: None,
        },
        chapter_chars: 2000,
    };
    let out =
        AgencyCoordinator::build_bootstrap_result(&result, "完整第一章正文……".to_string(), "r1");
    assert!(out.success);
    assert_eq!(out.steps_completed, 1);
    assert_eq!(out.final_content.as_deref(), Some("完整第一章正文……"));
    assert_eq!(
        out.messages,
        vec![
            "story_created:story-9".to_string(),
            "session_id:r1".to_string(),
            "novel_bootstrap_first_chapter_ready".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_resume_run_restores_board_and_wraps_history() {
    let pool = create_test_pool().unwrap();
    // 旧 run：completed，带资产与摘要
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("old-run", "前提")).unwrap();
    repo.set_run_story("old-run", "s1").unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    board
        .write(
            "old-run",
            "s1",
            AgentRole::Producer,
            BoardZone::Asset,
            "world",
            "世界观",
            "双星",
            "双星",
        )
        .unwrap();
    repo.finish_run("old-run", "completed", None, None).unwrap();
    let svc = crate::agency::session::SessionService::new(pool.clone());
    let session = svc.snapshot("old-run", "final", "final").unwrap();
    repo.write_session_summary(&session.id, "上次写到第二章，阿苔刚登上星舰")
        .unwrap();
    // 故事与第一章场景（resume 后从第二章继续）
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, description, genre, created_at, updated_at)
             VALUES ('s1', '测试书', '前提', '科幻', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', 's1', '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        // v0.30.21: 预置世界观与故事大纲
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', 's1', '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', 's1', '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create("s1", 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    // resume（mock：writer 写第 2 章 + editor pass）
    let write2 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"二"}}}}"#,
        pass_grade_content("第二章：星舰苏醒。")
    );
    let llm = MockLlm::scripted(vec![
        // v0.30.21: generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。场景：对话与追逐交替。",
        write2.as_str(),
        r#"{"type":"final","content":"完成"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let outcome = coordinator.resume_run("old-run").await.unwrap();
    assert_eq!(outcome.resumed_from, "old-run");
    // 黑板已复制到新 run
    let new_board = crate::agency::board::BlackboardService::new(pool.clone());
    let snap = new_board.snapshot(&outcome.new_run_id).unwrap();
    assert!(snap.assets.iter().any(|i| i.key == "世界观"));
    // 恢复简报带 stale-replay 包装（schedule 区）
    let brief = snap
        .schedules
        .iter()
        .find(|i| i.key == "恢复简报")
        .expect("应有恢复简报");
    assert!(brief.content.contains("HISTORICAL REFERENCE ONLY"));
    assert!(brief.content.contains("阿苔刚登上星舰"));
    // 简报 summary 带旧 run id
    assert!(brief.summary.contains("old-run"));
    // 续写完成（mock 驱动 batch 一章）→ 新场景产生
    let scenes = crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_story("s1")
        .unwrap();
    assert_eq!(scenes.len(), 2);
}

/// resume_prepare 只做校验/护栏/复制/简报：不启动 batch，新 run 保持
/// pending。
#[tokio::test]
async fn test_resume_prepare_does_not_start_batch() {
    let pool = create_test_pool().unwrap();
    // 与 test_resume_run_restores_board_and_wraps_history 相同的种子
    //（旧 run completed + 资产 + 摘要 + 第一章场景 + 角色）
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("old-run", "前提")).unwrap();
    repo.set_run_story("old-run", "s1").unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    board
        .write(
            "old-run",
            "s1",
            AgentRole::Producer,
            BoardZone::Asset,
            "world",
            "世界观",
            "双星",
            "双星",
        )
        .unwrap();
    repo.finish_run("old-run", "completed", None, None).unwrap();
    let svc = crate::agency::session::SessionService::new(pool.clone());
    let session = svc.snapshot("old-run", "final", "final").unwrap();
    repo.write_session_summary(&session.id, "上次写到第二章，阿苔刚登上星舰")
        .unwrap();
    // 故事与第一章场景（resume 后从第二章继续）
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, description, genre, created_at, updated_at)
             VALUES ('s1', '测试书', '前提', '科幻', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', 's1', '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        // v0.30.21: 预置世界观与故事大纲
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', 's1', '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', 's1', '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
    }
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create("s1", 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    let outcome = coordinator.resume_prepare("old-run").await.unwrap();
    assert_eq!(outcome.resumed_from, "old-run");
    // prepare 不启动 batch：mock 无脚本也不会被调用；黑板已复制、简报已写
    let snap = crate::agency::board::BlackboardService::new(pool.clone())
        .snapshot(&outcome.new_run_id)
        .unwrap();
    assert!(snap.assets.iter().any(|i| i.key == "世界观"));
    assert!(snap.schedules.iter().any(|i| i.key == "恢复简报"));
    // 新 run 存在且未被 finalize（status 仍为 pending——batch 未跑）
    let run = AgencyRepository::new(pool.clone())
        .get_run(&outcome.new_run_id)
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "pending");
}

#[tokio::test]
async fn test_resume_rejects_running_run() {
    let pool = create_test_pool().unwrap();
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("running-run", "前提"))
        .unwrap();
    repo.update_run_phase("running-run", "running", "assets")
        .unwrap();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    let err = coordinator.resume_run("running-run").await.unwrap_err();
    assert!(err.to_string().contains("进行中") || err.to_string().contains("running"));
}

#[test]
fn test_request_guard_unregisters_on_drop() {
    let run = "run-guard-test";
    // guard 存活期间 request_id 在注册表内（drain 取走 req-g1，证明 new
    // 已注册）
    {
        let _guard = RequestGuard::new(run, "req-g1");
        assert_eq!(drain_requests(run), vec!["req-g1".to_string()]);
    }
    // guard drop 后注册表已清理（上面 drain 提前取走会破坏语义——用另一 id
    // 验证）
    register_request(run, "req-g2");
    {
        let _guard = RequestGuard::new(run, "req-g3");
    }
    let drained = drain_requests(run);
    assert_eq!(drained, vec!["req-g2".to_string()]); // req-g3 已被 guard
                                                     // 摘除
}

/// map_active_run_conflict：命中 agency_runs 唯一约束（两种 SQLite 报错
/// 形态）映射为 VALIDATION_FAILED；其他错误（含他表 UNIQUE 冲突）原样透传。
#[test]
fn test_map_active_run_conflict_only_matches_agency_runs() {
    // 形态一：部分唯一索引
    let err = map_active_run_conflict(AppError::from(
        "UNIQUE constraint failed: index 'idx_agency_runs_one_active_per_story'",
    ));
    assert_eq!(err.code(), "VALIDATION_FAILED");
    assert!(err.to_string().contains("进行中"));
    let json = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["data"]["field"], "active_run");
    // 形态二：列约束
    let err = map_active_run_conflict(AppError::from(
        "UNIQUE constraint failed: agency_runs.story_id",
    ));
    assert_eq!(err.code(), "VALIDATION_FAILED");
    // 他表 UNIQUE 冲突不误吞
    let err = map_active_run_conflict(AppError::from("UNIQUE constraint failed: scenes.id"));
    assert_eq!(err.code(), "INTERNAL_ERROR");
    assert!(err.to_string().contains("scenes.id"));
    // 普通错误原样透传
    let err = map_active_run_conflict(AppError::from("database is locked"));
    assert_eq!(err.code(), "INTERNAL_ERROR");
    assert!(err.to_string().contains("database is locked"));
}

/// write_chapter 按约定 key 取稿：模型写错 key（「序章」≠「第1章」）必须
/// 大声失败，错误文案含约定 key。
#[tokio::test]
async fn test_write_chapter_wrong_key_fails_loudly() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(0);
    mock.push("writer", vec![
        // 模型违规：用错 key
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"序章","content":"写错了章号。","summary":"错"}}"#,
        r#"{"type":"final","content":"完成"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let mut run = AgencyRun::new("rw-1", "续写");
    run.story_id = Some(story_id.clone());
    AgencyRepository::new(pool.clone())
        .create_run(&run)
        .unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let registry = Arc::new(ToolRegistry::agency_default());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    // 单章续写不再回落到 write_chapter；本契约直接打 tool_loop 路径。
    let err = coordinator
        .write_chapter(&budget, &board, &registry, "rw-1", &story_id, "续写", 1)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("第1章") || err.to_string().contains("缺少"),
        "错误应含约定 key: {}",
        err
    );
}

/// 门判定落审查区的 key 带轮次后缀：首轮 gate-{key}-r1，修订后复审 -r2。
#[tokio::test]
async fn test_gate_record_keys_have_round_suffix() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let mock = RoutingMock::new(0);
    mock.push("writer", vec![
        r#"{"type":"tool","name":"board_write","args":{"zone":"draft","item_type":"chapter","key":"第1章","content":"初稿。","summary":"一"}}"#,
        r#"{"type":"final","content":"完成"}"#,
        // 修订轮：直接 final（draft 未变，第二轮 gate pass 放行）
        r#"{"type":"final","content":"已知晓修订意见"}"#,
    ]);
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"revise\",\"blocking_issues\":[\"动机弱\"],\"suggestions\":[],\"comments\":\"修\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"过\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let batch = coordinator
        .run_continue_batch("rg-2", &story_id, 1, 1)
        .await
        .unwrap();
    let result = &batch.chapters[0];
    assert!(result.revised);
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let snap = board.snapshot("rg-2").unwrap();
    let keys: Vec<&str> = snap
        .reviews
        .iter()
        .filter(|i| i.item_type == "gate")
        .map(|i| i.key.as_str())
        .collect();
    assert_eq!(keys, vec!["gate-第1章-r1", "gate-第1章-r2"]);
}

/// resume_run story 级护栏：旧 run 的 story 存在其他 pending/running run
/// 时拒绝恢复。
#[tokio::test]
async fn test_resume_rejects_when_story_has_active_run() {
    let pool = create_test_pool().unwrap();
    let repo = AgencyRepository::new(pool.clone());
    // 旧 run 已结束（failed）
    let mut old = AgencyRun::new("old-run", "前提");
    old.story_id = Some("s1".into());
    repo.create_run(&old).unwrap();
    repo.finish_run("old-run", "failed", None, None).unwrap();
    // 同 story 另一个进行中 run（pending 即命中护栏）
    let mut other = AgencyRun::new("other-run", "前提2");
    other.story_id = Some("s1".into());
    repo.create_run(&other).unwrap();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    let err = coordinator.resume_run("old-run").await.unwrap_err();
    assert!(
        err.to_string().contains("该故事已有进行中的创作任务"),
        "应命中 story 级护栏: {}",
        err
    );
}

#[tokio::test]
async fn test_gate_v2_low_weighted_triggers_revision() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    // editor 判 pass 但 score 极低（1.0/5 → model 0.2）→ weighted 必然 < 0.75 →
    // 修订
    let mock = RoutingMock::new(0);
    let write_line = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"一"}}}}"#,
        "正文".repeat(500) // word_count ≥800 避免 code 档字数扣分干扰断言
    );
    mock.push(
        "writer",
        vec![
            write_line.as_str(),
            r#"{"type":"final","content":"完成"}"#,
            // 修订轮
            r#"{"type":"final","content":"已修订"}"#,
        ],
    );
    mock.push("editor", vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":1.0,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"勉强\"}"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), mock);
    let batch = coordinator
        .run_continue_batch("gv2-1", &story_id, 1, 1)
        .await
        .unwrap();
    let result = &batch.chapters[0];
    assert!(
        result.revised,
        "低 rubric 分应触发修订: {:?}",
        result.verdict
    );
    // gate 条目含 gate_score 字段
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let reviews = board.list_zone("gv2-1", BoardZone::Review).unwrap();
    let gate_item = reviews.iter().find(|i| i.item_type == "gate").unwrap();
    let content: serde_json::Value = serde_json::from_str(&gate_item.content).unwrap();
    assert!(content.get("gate_score").is_some());
    let weighted = content["gate_score"]["weighted"].as_f64().unwrap();
    assert!(weighted < 0.75, "首轮 weighted 应低于阈值: {}", weighted);
}

/// spec 5.5 回归：长、高追读力（code≈1.0、reading_power 高、editor 判
/// pass 且 score 4.5 → weighted > 0.75）但开头与上一章重复的章节，必须
/// 被规则复检 High+ 硬拦截（v1 语义保留），不得因加权达标而放行。
#[tokio::test]
async fn test_gate_v2_subagent_high_blocks_despite_high_weighted() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "硬拦截书".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置第一章场景；草稿开头与其前 20 字完全一致 → ContinuityAgent High
    let opening = "风沙掠过双星废土的清晨，阿苔在残骸中醒来";
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(format!("{}，耳边是磁力风暴的低鸣。", opening)),
                ..Default::default()
            },
        )
        .unwrap();

    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("rg-5", "续写")).unwrap();
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    // 高分正文：编号句 + 悬念钩子/爽点/微兑现（code≈1.0、rule≈0.45——
    // 追读力已对齐生产口径：每命中 +0.1，cap 0.8/0.4）；
    // 但开头与第一章前 20 字重复
    let draft = board
        .write(
            "rg-5",
            &story.id,
            AgentRole::LeadWriter,
            BoardZone::Draft,
            "chapter",
            "第2章",
            &pass_grade_content(opening),
            "第二章草稿",
        )
        .unwrap();

    // editor 判 pass 且 score 4.5 → model 0.9 → weighted ≈ 0.79 > 0.75
    let llm: Arc<dyn LoopLlm> = MockLlm::scripted(vec![
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let registry = Arc::new(ToolRegistry::agency_default());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let outcome = coordinator
        .evaluate_gate(
            &budget, &board, &registry, "rg-5", &story.id, "续写", &draft, 1,
        )
        .await
        .unwrap();
    match outcome {
        GateOutcome::RevisionRequired { issues, .. } => {
            assert!(
                issues.iter().any(|i| i.contains("重复")),
                "High+ 硬拦截应报告重复开头问题: {:?}",
                issues
            );
        }
        other => panic!(
            "加权达标但存在 High+ 复检问题，应被 spec 5.5 硬拦截，实际: {:?}",
            other
        ),
    }
    // 落盘 gate 条目确认硬拦截记录（weighted > 0.75 但 outcome=revise）
    let reviews = board.list_zone("rg-5", BoardZone::Review).unwrap();
    let gate_item = reviews.iter().find(|i| i.item_type == "gate").unwrap();
    let record: serde_json::Value = serde_json::from_str(&gate_item.content).unwrap();
    assert_eq!(record["outcome"].as_str(), Some("revise"));
    assert!(
        record["gate_score"]["weighted"].as_f64().unwrap() > 0.75,
        "本用例 weighted 必须高于阈值（否则走的是规则 6 而非 5.5）: {}",
        record["gate_score"]["weighted"]
    );
}

/// P4 检查点钩子：genesis 落 concept/assets/run_final 检查点（按 brief
/// 钩子清单，genesis 首章不经 handle_gate，无 chapter 检查点，首章指标
/// 由 run_final 覆盖）；单章续写落 assets/chapter/run_final，chapter
/// 检查点带章号与本章 weighted。
#[tokio::test]
async fn test_checkpoints_written_at_milestones() {
    let pool = create_test_pool().unwrap();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), pass_script());
    let result = coordinator.run_genesis("cp-g", LONG_PREMISE).await.unwrap();
    let repo = AgencyRepository::new(pool.clone());
    let list = repo.list_checkpoints(&result.story_id).unwrap();
    let milestones: Vec<&str> = list.iter().map(|c| c.milestone.as_str()).collect();
    assert_eq!(milestones, vec!["concept", "assets", "run_final"]);
    // run_final 指标：一章已装配；gate_scores 含首章 weighted（首章 key
    // 「第1章」为阿拉伯数字，chapter_from_gate_key 解析为 1）
    let m: serde_json::Value = serde_json::from_str(&list[2].metrics_json).unwrap();
    assert_eq!(m["chapters_done"].as_i64(), Some(1));
    assert!(m["words_total"].as_i64().unwrap() > 0);
    assert!(m["tokens_used"].as_u64().is_some());
    assert!(m["elapsed_s"].as_i64().is_some());
    // v0.30.35：editor 质检后台化，genesis 前台不再产出 gate_scores
    //（后台 spawn_editor_qc 在测试环境 no-op）。单章续写同模式。
    let gates = m["gate_scores"].as_array().unwrap();
    assert!(gates.is_empty(), "genesis 前台无 gate_scores: {:?}", gates);

    // 单章续写：assets → chapter → run_final（质检后台化，chapter 无
    // gate_scores）
    let story_id = seed_story_with_assets(&pool);
    let chapter1 = pass_grade_content("第1章正文。");
    let llm = MockLlm::scripted(vec![
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。场景：对话与追逐交替。",
        chapter1.as_str(),
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    coordinator
        .run_continue(
            "cp-c",
            &story_id,
            PersistMode::NextChapter { chapter_number: 1 },
            "",
            None,
        )
        .await
        .unwrap();
    let list = repo.list_checkpoints(&story_id).unwrap();
    let milestones: Vec<&str> = list.iter().map(|c| c.milestone.as_str()).collect();
    assert_eq!(milestones, vec!["assets", "chapter", "run_final"]);
    let ch = &list[1];
    assert_eq!(ch.chapter_number, Some(1));
    let m: serde_json::Value = serde_json::from_str(&ch.metrics_json).unwrap();
    assert_eq!(m["chapters_done"].as_i64(), Some(1));
    let gates = m["gate_scores"].as_array().unwrap();
    assert!(
        gates.is_empty(),
        "续写质检后台化，chapter 检查点无 gate_scores: {:?}",
        gates
    );
}

/// 资产回流（Task 2）：handle_gate 装配落库后触发后台 spawn_asset_ingest；
/// 测试环境（app_handle=None）no-op——正文落库不受影响，KG/ingest_jobs 无写入。
#[tokio::test]
async fn test_spawn_asset_ingest_noop_in_test_env() {
    let pool = create_test_pool().unwrap();
    let story_id = seed_story_with_assets(&pool);
    let write1 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"一"}}}}"#,
        pass_grade_content("第1章正文。")
    );
    let llm = MockLlm::scripted(vec![
        // v0.30.21: generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。场景：对话与追逐交替。",
        write1.as_str(),
        r#"{"type":"final","content":"完成"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator
        .run_continue(
            "ingest-noop",
            &story_id,
            PersistMode::NextChapter { chapter_number: 1 },
            "",
            None,
        )
        .await
        .unwrap();
    // 正文落库不受后台资产回流影响（ingest 是后台任务，不阻塞主流程）
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    let content = scene.content.unwrap_or_default();
    assert!(
        content.contains("第1章正文"),
        "scenes.content 应与装配正文一致: {}",
        &content[..content.len().min(100)]
    );
    // 测试环境 no-op：KG 表与 ingest_jobs 均无写入
    let (kg_count, job_count): (i64, i64) = {
        let conn = pool.get().unwrap();
        let kg = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE story_id = ?1",
                rusqlite::params![story_id],
                |r| r.get(0),
            )
            .unwrap();
        let jobs = conn
            .query_row(
                "SELECT COUNT(*) FROM ingest_jobs WHERE story_id = ?1",
                rusqlite::params![story_id],
                |r| r.get(0),
            )
            .unwrap();
        (kg, jobs)
    };
    assert_eq!(kg_count, 0, "测试环境资产回流应 no-op，kg_entities 无写入");
    assert_eq!(job_count, 0, "测试环境资产回流应 no-op，ingest_jobs 无写入");
}

/// v0.30.21: ensure_world_building 在世界观缺失时强制生成并落库。
#[tokio::test]
async fn test_ensure_world_building_generates_when_missing() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "世界观测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置角色（有角色但无世界观 -> ensure_world_building 被触发）
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    // MockLlm: ensure_world_building（Producer complete 调用）
    let world_text = "双星文明：资源匮乏的拾荒世界。星环崩塌后残存文明在废墟中争夺资源。\
                      权力结构：星环议会垄断技术残骸，拾荒者处于社会底层。\
                      冲突源：拾荒者与议会的资源争夺，星环重启的技术秘密。\
                      社会矛盾：技术垄断与生存权的对立，底层反抗暗流涌动。";
    let llm = MockLlm::scripted(vec![world_text]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    // 直接调用 ensure_assets（内含 ensure_world_building）
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(&budget, &repo, "r-wb", &story.id, "前提")
        .await
        .unwrap();
    // 验证 world_buildings 表有行
    let count: i64 = {
        let conn = pool.get().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM world_buildings WHERE story_id = ?1",
            rusqlite::params![story.id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(count, 1, "world_buildings should have 1 row");
    // 验证 concept 和 history 已写入
    let wb = crate::db::repositories::WorldBuildingRepository::new(pool.clone())
        .get_by_story(&story.id)
        .unwrap()
        .unwrap();
    assert!(
        wb.concept.contains("双星文明"),
        "concept should contain generated text"
    );
    // v0.30.31: concept 现存全文（含历史背景），history 不再单独冗余存储
    // （build_continue_writer_context 注入 concept 全文已含历史，避免重复）。
    assert!(
        wb.concept.contains("星环崩塌"),
        "concept (full text) should contain history background"
    );
}

/// v0.30.21: ensure_story_outline 在故事大纲缺失时强制生成并落库。
#[tokio::test]
async fn test_ensure_methodology_default_writes_scene_structure_when_empty() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "方法论默认".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    AgencyCoordinator::persist_default_methodology_if_empty(&pool, &story.id).unwrap();
    let got = crate::db::repositories::StoryRepository::new(pool)
        .get_by_id(&story.id)
        .unwrap()
        .unwrap();
    assert_eq!(got.methodology_id.as_deref(), Some("scene_structure"));
    assert_eq!(got.methodology_step, Some(1));
}

#[tokio::test]
async fn test_ensure_methodology_default_does_not_override_hero_journey() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "已选英雄之旅".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: Some("hero_journey".into()),
            reference_book_id: None,
        })
        .unwrap();
    AgencyCoordinator::persist_default_methodology_if_empty(&pool, &story.id).unwrap();
    let got = crate::db::repositories::StoryRepository::new(pool)
        .get_by_id(&story.id)
        .unwrap()
        .unwrap();
    assert_eq!(got.methodology_id.as_deref(), Some("hero_journey"));
}

#[tokio::test]
async fn test_ensure_assets_with_prose_does_not_require_producer_loop() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "帝国的烟火".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let scene = SceneRepository::new(pool.clone())
        .create(&story.id, 1, Some("第一章"))
        .unwrap();
    let mut prose = "知启纪元八百四十七年。大奉帝国西北边陲重镇，黑崎州城。\
第二代镇北王苏会山端坐大堂。"
        .to_string();
    while prose.chars().count() < 200 {
        prose.push_str("镇北王府大堂里红毡铺地，黑卫军肃立。");
    }
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE scenes SET content = ?1 WHERE id = ?2",
            rusqlite::params![prose, scene.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o-grounded', ?1, '【转折点】苏会山在镇北王府大堂迎亲。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let llm = MockLlm::scripted(vec![]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let repo = AgencyRepository::new(pool);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(
            &budget,
            &repo,
            "r-prose",
            &story.id,
            "续写《帝国的烟火》第1章",
        )
        .await
        .expect("有正文时不得为补资产去按书名发明");
    assert!(
        llm.calls.lock().unwrap().is_empty(),
        "有正文时不应调用管理 Agent tool_loop / 标题大纲: {:?}",
        llm.calls.lock().unwrap()
    );
}

#[tokio::test]
async fn test_spawn_producer_resume_noop_in_test_env() {
    let pool = create_test_pool().unwrap();
    let llm = MockLlm::scripted(vec!["should-not-run"]);
    let coordinator = AgencyCoordinator::for_test(pool, llm.clone());
    coordinator.spawn_producer_resume("r-resume", "story-x");
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(
        llm.calls.lock().unwrap().is_empty(),
        "测试环境后台补齐必须 no-op: {:?}",
        llm.calls.lock().unwrap()
    );
}

/// v0.30.21: ensure_story_outline 在故事大纲缺失时强制生成并落库。
#[tokio::test]
async fn test_ensure_story_outline_generates_when_missing() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "大纲测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置角色 + 世界观（有角色和世界观但无故事大纲 -> ensure_story_outline
    // 被触发）
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    // MockLlm: ensure_story_outline（Producer complete 调用）
    let outline_text = "核心冲突：阿苔寻找星环秘密，与星环议会的垄断形成对抗。\
                        三幕结构：起因-阿苔发现星环线索；发展-盟友背叛与禁区探索；高潮-星环重启与真相揭露。\
                        关键转折点：盟友背叛、星环重启、禁区发现。\
                        整体推进方向：从拾荒生存到揭开文明真相，最终改变权力格局。";
    let llm = MockLlm::scripted(vec![outline_text]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(&budget, &repo, "r-so", &story.id, "前提")
        .await
        .unwrap();
    // 验证 story_outlines 表有行
    let outline = crate::db::repositories::StoryOutlineRepository::new(pool.clone())
        .get_by_story(&story.id)
        .unwrap();
    assert!(outline.is_some(), "story_outlines should have a row");
    assert!(
        outline.unwrap().content.contains("核心冲突"),
        "outline content should contain generated text"
    );
}

fn su_family_prose() -> String {
    let mut prose = "知启纪元八百四十七年。大奉帝国西北边陲重镇，黑崎州城。\
第二代镇北王苏会山端坐大堂。大少爷苏亦铁红装肃立。"
        .to_string();
    while prose.chars().count() < 200 {
        prose.push_str("镇北王府大堂里红毡铺地，黑卫军肃立。");
    }
    prose
}

/// 有正文时大纲从章节归纳，用场景结构而非 PROBLEM；脏的费迪南大纲被 UPDATE。
#[tokio::test]
async fn test_ensure_story_outline_from_prose_uses_scene_structure_not_problem() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "帝国的烟火".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let scene = SceneRepository::new(pool.clone())
        .create(&story.id, 1, Some("第一章"))
        .unwrap();
    let prose = su_family_prose();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE scenes SET content = ?1 WHERE id = ?2",
            rusqlite::params![prose, scene.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c-fer', ?1, '费迪南三世', '', '', '', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o-dirty', ?1, '第一卷·灰烬低语。费迪南三世为撑烟火节加征火药税。艾拉偷入工坊。塞尔吉奥在火山口守夜。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let grounded = "【核心冲突】镇北王府迎亲遇刺。苏会山在大堂护住苏亦铁。\
下一拍按场景结构写在场者的反应、困境与决定，不得换场换主角。\
已发生：黑崎州城、镇北王府大堂、苏会山端坐、红毡铺地、黑卫军肃立。\
本场尚未写完灾难后的反应，须留在王府大堂推进。";
    let llm = MockLlm::scripted(vec![grounded]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(
            &budget,
            &repo,
            "r-outline-prose",
            &story.id,
            "续写《帝国的烟火》第1章",
        )
        .await
        .unwrap();
    let systems = llm.systems.lock().unwrap().clone();
    let calls = llm.calls.lock().unwrap().clone();
    assert!(
        !systems.is_empty() && !calls.is_empty(),
        "有正文且大纲未接地时应归纳大纲"
    );
    assert!(
        systems[0].contains("目标") && systems[0].contains("灾难"),
        "system 须来自场景结构方法论，实际: {}",
        systems[0]
    );
    assert!(
        !systems[0].contains("PROBLEM"),
        "有正文时不得用 PROBLEM 当大纲骨架: {}",
        systems[0]
    );
    assert!(
        calls[0].contains("苏会山"),
        "user 须含正文摘录: {}",
        calls[0]
    );
    assert!(
        calls[0].contains("目标→冲突→灾难") || calls[0].contains("已有章节正文"),
        "user 须要求从正文归纳而非只给书名: {}",
        calls[0]
    );
    let outline = crate::db::repositories::StoryOutlineRepository::new(pool.clone())
        .get_by_story(&story.id)
        .unwrap()
        .expect("应 UPDATE 为接地大纲");
    assert!(
        outline.content.contains("苏会山"),
        "接地大纲应落库，实际: {}",
        outline.content
    );
    assert!(!outline.content.contains("费迪南"));
}

/// 模型按书名发明费迪南时拒绝落库。
#[tokio::test]
async fn test_ensure_story_outline_rejects_ungrounded_llm_output() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "帝国的烟火".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let scene = SceneRepository::new(pool.clone())
        .create(&story.id, 1, Some("第一章"))
        .unwrap();
    let prose = su_family_prose();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE scenes SET content = ?1 WHERE id = ?2",
            rusqlite::params![prose, scene.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c-fer', ?1, '费迪南三世', '', '', '', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let invented = "第一卷·灰烬低语。费迪南三世为撑烟火节加征火药税，\
艾拉潜入工坊盗火。塞尔吉奥在都城火山口守夜。三幕结构：征税、盗火、火山。\
关键转折点：烟火节、工坊、火山口。整体推进方向：帝国都城的权力斗争。";
    let llm = MockLlm::scripted(vec![invented]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm.clone());
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(
            &budget,
            &repo,
            "r-outline-reject",
            &story.id,
            "续写《帝国的烟火》第1章",
        )
        .await
        .unwrap();
    assert!(
        !llm.calls.lock().unwrap().is_empty(),
        "有正文无大纲时应尝试归纳，再因未接地拒绝落库"
    );
    let outline = crate::db::repositories::StoryOutlineRepository::new(pool.clone())
        .get_by_story(&story.id)
        .unwrap();
    assert!(
        outline.is_none(),
        "未接地大纲不得落库: {:?}",
        outline.map(|o| o.content)
    );
}

/// v0.30.21: generate_chapter_outline 生成章节大纲并写入黑板。
#[tokio::test]
async fn test_generate_chapter_outline() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "章节大纲测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置角色 + 世界观 + 故事大纲
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    // MockLlm: generate_chapter_outline 的 Producer complete 调用
    let chapter_outline_text = "本章核心冲突：阿苔发现星环秘密。\
                                 转折点：盟友突然背叛。\
                                 推进内容：前往禁区探索真相。\
                                 场景设计：对话与追逐交替。";
    let llm = MockLlm::scripted(vec![chapter_outline_text]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    // 先构建 assets_ctx（含故事大纲）
    let assets_ctx = coordinator.build_continue_writer_context(&story.id).await;
    assert!(
        assets_ctx.contains("【故事大纲"),
        "assets_ctx should contain story outline"
    );
    // 生成章节大纲
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let outline = coordinator
        .generate_chapter_outline("r-co", &story.id, "前提", &budget, 1, &assets_ctx, "")
        .await;
    assert!(!outline.is_empty(), "chapter outline should be non-empty");
    assert!(
        outline.contains("核心冲突"),
        "chapter outline should contain conflict"
    );
    // 验证黑板 Draft 区有 outline-第1章 条目
    let board = crate::agency::board::BlackboardService::new(pool.clone());
    let drafts = board.list_zone("r-co", BoardZone::Draft).unwrap();
    assert!(
        drafts
            .iter()
            .any(|d| d.key == "outline-第1章" && !d.content.is_empty()),
        "blackboard Draft zone should have outline-第1章 entry"
    );
}

/// v0.30.21: generate_chapter_outline 无故事大纲时跳过（返回空串）。
#[tokio::test]
async fn test_generate_chapter_outline_skips_without_story_outline() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "无大纲跳过测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 预置角色 + 世界观，但不预置故事大纲
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    // MockLlm: 不应有调用（空队列）
    let llm = MockLlm::scripted(vec![]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let assets_ctx = coordinator.build_continue_writer_context(&story.id).await;
    assert!(
        !assets_ctx.contains("【故事大纲"),
        "assets_ctx should NOT contain story outline"
    );
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let outline = coordinator
        .generate_chapter_outline("r-skip", &story.id, "前提", &budget, 1, &assets_ctx, "")
        .await;
    assert!(
        outline.is_empty(),
        "chapter outline should be empty when no story outline exists"
    );
}

// ---- v0.30.30 D/E 结构性优化测试 ----

/// 构造一个 Draft 区 BoardItem 供纯函数测试用。
fn make_draft(content: &str) -> BoardItem {
    BoardItem {
        id: "d1".into(),
        run_id: "r".into(),
        story_id: "s".into(),
        zone: BoardZone::Draft,
        item_type: "chapter".into(),
        key: "第1章".into(),
        content: content.into(),
        summary: "summary".into(),
        version: 1,
        producer: AgentRole::LeadWriter,
        status: "active".into(),
        created_at: "2026-01-01".into(),
        updated_at: "2026-01-01".into(),
    }
}

/// E1：scoreless pass 兜底从 0.85 降到 0.7（低于 0.75 阈值）。
/// 配合 test_verdict_legacy_format_fallback 的 0.7 断言，此处验证有分路径
/// 不受影响（score=4.5 -> 0.9）。
#[test]
fn test_model_grader_scoreless_pass_below_threshold() {
    // scoreless pass -> 0.7（低于 0.75 阈值，须 code+rule 兜底）
    let scoreless = EditorVerdict {
        verdict: "pass".into(),
        blocking_issues: vec![],
        suggestions: vec![],
        comments: String::new(),
        score: None,
        dimension_scores: None,
    };
    let m = ModelGraderReport::from_verdict(&scoreless).model_score;
    assert!(
        (m - 0.7).abs() < 0.001,
        "scoreless pass 应为 0.7，实际 {}",
        m
    );
    assert!(m < 0.75, "scoreless pass 不应单凭 model 项过门");
    // 有分路径不受影响
    let scored = EditorVerdict {
        verdict: "pass".into(),
        blocking_issues: vec![],
        suggestions: vec![],
        comments: String::new(),
        score: Some(4.5),
        dimension_scores: None,
    };
    let m2 = ModelGraderReport::from_verdict(&scored).model_score;
    assert!((m2 - 0.9).abs() < 0.001, "score=4.5 -> 0.9，实际 {}", m2);
}

/// E2：质量门 Failed 降级放行。editor 完全失败时，substantive 草稿（≥600 字符）
/// 合成 pass 裁决保产出；过短草稿返回 None 维持 Err。
#[test]
fn test_salvage_failed_gate() {
    // substantive 草稿 -> 降级放行
    let long = make_draft(&"章节正文内容。".repeat(200)); // > 600 字符
    let v = AgencyCoordinator::salvage_failed_gate(&long, "编辑审计失败").expect("长稿应降级放行");
    assert_eq!(v.verdict, "pass");
    assert!(v.score.is_none(), "降级裁决不应有数值分");
    assert!(v.blocking_issues.is_empty());
    assert!(
        v.comments.contains("编辑审计失败"),
        "comments 应含降级原因: {}",
        v.comments
    );
    // 过短草稿 -> None（不救垃圾稿）
    let short = make_draft("只有几十字的草稿，远低于 600 字符阈值。");
    assert!(
        AgencyCoordinator::salvage_failed_gate(&short, "编辑审计失败").is_none(),
        "过短草稿不应降级放行"
    );
    // 临界值：恰好 600 字符应放行
    let boundary = make_draft(&"章".repeat(600));
    assert!(
        AgencyCoordinator::salvage_failed_gate(&boundary, "编辑审计失败").is_some(),
        "恰好 600 字符应降级放行"
    );
}

/// v0.30.35：EditorVerdict::pending() 构造的默认值--genesis 前台返回此裁决，
/// 后台质检完成后经 genesis-qc-result 事件反馈（前端不消费此字段）。
#[test]
fn test_editor_verdict_pending_defaults() {
    let v = EditorVerdict::pending();
    assert_eq!(v.verdict, "pending");
    assert!(v.blocking_issues.is_empty());
    assert!(v.suggestions.is_empty());
    assert!(v.score.is_none());
    assert!(v.dimension_scores.is_none());
    assert!(!v.comments.is_empty(), "pending 裁决应有说明文案");
}

/// v0.30.35：assemble_only 装配草稿 -> Scene 真源，不跑 editor 质检。
/// 用空 mock 证明装配阶段无 LLM 调用（editor 质检已后台化为 spawn_editor_qc，
/// 测试环境无 app_handle 时 no-op）。这是创世首章"立即显示"的核心不变量。
#[tokio::test]
async fn test_assemble_only_persists_scene_without_qc() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "装配书".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("ra-1", "创世")).unwrap();
    // 空 mock：assemble_only 不应发起任何 LLM 调用（editor 质检后台化）
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    let cancel = Arc::new(AtomicBool::new(false));
    let content = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let draft = make_draft(&content);
    let (returned_draft, scene_id) = coordinator
        .assemble_only(&repo, "ra-1", &story.id, &cancel, draft)
        .await
        .unwrap();
    assert!(!scene_id.is_empty(), "应返回非空 scene_id");
    assert_eq!(returned_draft.content, content, "应原样返回草稿");
    // Scene 已落库，正文来自草稿（cleanup 对该低重复内容无改动）
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&scene_id)
        .unwrap()
        .unwrap();
    assert_eq!(scene.content.as_deref(), Some(content.as_str()));
}

/// D1：cleanup_prose_for_persist 对自重复内容执行清理（genesis 首章无既有
/// 场景，strip_existing_overlap 自动跳过，仅 trim_self_repetition +
/// trim_dangling_tail）。
#[tokio::test]
async fn test_cleanup_prose_for_persist_trims_self_repetition() {
    let pool = create_test_pool().unwrap();
    let coordinator = AgencyCoordinator::for_test(pool.clone(), MockLlm::scripted(vec![]));
    // 两段完全相同的段落 -> 段落级自重复检测裁掉后半段
    let para = "阿苔走进星环遗迹，辐射风暴在身后渐渐平息。她握紧那枚项链，妹妹的笑容浮现在眼前。";
    let raw = format!("{}\n\n{}", para, para);
    let cleaned = coordinator
        .cleanup_prose_for_persist(&raw, "story-no-existing-scenes")
        .await;
    assert!(
        cleaned.chars().count() < raw.chars().count(),
        "自重复段落应被清理（清理前 {} 字符，清理后 {} 字符）",
        raw.chars().count(),
        cleaned.chars().count()
    );
    assert!(
        cleaned.matches(para).count() == 1,
        "清理后应只保留一段（实际 {} 段）",
        cleaned.matches(para).count()
    );
}

/// E3：writer MaxTurns 熔断时从黑板取回已产出草稿，而非整章失败。
/// 续写 writer tool_loop 连续 board_write（无 final）-> 达到 max_turns 熔断；
/// 熔断前已 board_write 产出 substantive 草稿到黑板 -> E3 取回并装配成功。
#[tokio::test]
async fn test_continue_writer_maxturns_board_recovery() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "MaxTurns 测试".into(),
            description: Some("前提".into()),
            genre: Some("科幻".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 续写需要故事大纲 + 第一章正文
    // 预置角色 + 世界观 + 故事大纲，使 ensure_assets 不触发 Producer 资产补齐
    // / ensure_world_building 的额外 LLM 调用（否则会消费 mock 队列导致错位）。
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let scene_repo = SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    let bw = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"星舰苏醒"}}}}"#,
        chapter2
    );
    let editor_pass = r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#;
    // generate_chapter_outline（1 调用，因 story_outline 存在触发）+ 10 次
    // board_write（writer tool_loop 跑满 max_turns=10 无 final -> MaxTurns
    // 熔断）
    // + editor（pass）。ensure_assets 因角色/世界/大纲齐备不消费任何 LLM 调用。
    let mut lines: Vec<String> =
        vec!["本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区。".into()];
    for _ in 0..10 {
        lines.push(bw.clone());
    }
    lines.push(editor_pass.into());
    let llm = MockLlm::scripted(lines.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let result = coordinator
        .run_continue(
            "rc-maxturns",
            &story.id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .expect("MaxTurns 熔断后应从黑板取回草稿，run 不应失败");
    assert_eq!(result.chapter_number, 2);
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert!(
        scene
            .content
            .as_deref()
            .unwrap_or("")
            .contains("第二章正文"),
        "装配的 scene 应为黑板取回的第二章草稿"
    );
    let run = AgencyRepository::new(pool.clone())
        .get_run("rc-maxturns")
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed", "MaxTurns 取回草稿后 run 应完成");
}

// ---- 阶段二（P1）：后端事件信号补齐——活动信号配对验证 ----
// 测试环境 app_handle=None 时 emit 静默，信号经 cfg(test) activity_log
// 记录（coordinator.recorded_activities()），借此断言角色/action/detail
// 配对与先后序。

/// 在信号日志中定位信号下标（缺失时 panic 并打印全量日志）。
fn signal_pos(log: &[String], sig: &str) -> usize {
    log.iter()
        .position(|s| s == sig)
        .unwrap_or_else(|| panic!("缺少活动信号 {}: {:?}", sig, log))
}

/// 断言一对 start/done 信号均存在且 start 先于 done。
fn assert_signal_pair(log: &[String], start: &str, done: &str) {
    let s = signal_pos(log, start);
    let d = signal_pos(log, done);
    assert!(s < d, "信号顺序错误：{} 应先于 {}: {:?}", start, done, log);
}

/// B-1/B-2/B-3/B-4：legacy 创世路径信号配对。
/// concept 返回非 JSON → 回退 legacy：概念 start/done 均为 Producer
/// （B-2 修角色标注 BUG），资产 start/done 配对（B-3 补 start），
/// 首章 start/done 配对（B-4 补 done），装配沿用 assemble_only 配对。
#[tokio::test]
async fn test_legacy_genesis_activity_signals_paired() {
    let pool = create_test_pool().unwrap();
    let chapter = pass_grade_content("第一章正文：风沙中的拾荒者。");
    let write = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第1章","content":"{}","summary":"拾荒者登场"}}}}"#,
        chapter
    );
    // 脚本与 test_fastpath_fallback_to_legacy 同构：concept 非 JSON →
    // legacy producer(tool,final) → writer(tool,final) → editor(final)
    let llm = MockLlm::scripted(vec![
        "不是 JSON",
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星废土","summary":"双星废土"}}"#,
        r#"{"type":"final","content":"资产就绪"}"#,
        write.as_str(),
        r#"{"type":"final","content":"第一章完成"}"#,
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"合格\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm).with_model_count(2);
    coordinator
        .run_genesis("rf-sig-legacy", LONG_PREMISE)
        .await
        .unwrap();

    let log = coordinator.recorded_activities();
    assert_signal_pair(&log, "producer|start|概念", "producer|done|概念");
    assert_signal_pair(&log, "producer|start|资产", "producer|done|资产");
    assert_signal_pair(&log, "lead_writer|start|首章", "lead_writer|done|首章");
    assert_signal_pair(&log, "producer|start|装配", "producer|done|装配");
    assert!(
        !log.iter().any(|s| s == "lead_writer|done|概念"),
        "概念完成信号角色应为 Producer（B-2）: {:?}",
        log
    );
}

/// B-5：续写 ensure_assets 角色/历史资产均缺失时 producer 现场补齐，
/// 资产补齐 start/done 信号配对。
#[tokio::test]
async fn test_ensure_assets_backfill_activity_signals() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "资产补齐信号测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    // 不预置任何角色/世界观/大纲，也无历史黑板条目 → producer 现场补齐
    let repo = AgencyRepository::new(pool.clone());
    repo.create_run(&AgencyRun::new("r-ea-sig", "前提"))
        .unwrap();
    let llm = MockLlm::scripted(vec![
        // producer 资产补齐 tool_loop：写角色卡 → final
        r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"character","key":"阿苔","content":"{\"name\":\"阿苔\",\"background\":\"拾荒者\",\"personality\":\"坚韧\",\"goals\":\"找到星环\"}","summary":"拾荒者阿苔"}}"#,
        r#"{"type":"final","content":"资产补齐完成"}"#,
        // ensure_world_building（Producer 单调用）
        "双星文明：资源匮乏的拾荒世界。星环崩塌后残存文明在废墟中争夺资源。",
        // ensure_story_outline（Producer 单调用）
        "核心冲突：阿苔寻找星环秘密。三幕：起因-发展-高潮。",
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(&budget, &repo, "r-ea-sig", &story.id, "前提")
        .await
        .unwrap();

    let log = coordinator.recorded_activities();
    assert_signal_pair(&log, "producer|start|资产补齐", "producer|done|资产补齐");
}

/// B-6：续写 handle_gate 装配信号配对（单章路径；批量循环共用同一
/// handle_gate，单点覆盖）。
#[tokio::test]
async fn test_handle_gate_assembly_activity_signals() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "装配信号测试".into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
    }
    let scene_repo = SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    let write2 = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"星舰苏醒"}}}}"#,
        chapter2
    );
    let llm = MockLlm::scripted(vec![
        // generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区探索真相。",
        // writer: 写第 2 章 → final
        write2.as_str(),
        r#"{"type":"final","content":"第二章完成"}"#,
        // editor: pass
        r#"{"type":"final","content":"{\"verdict\":\"pass\",\"score\":4.5,\"blocking_issues\":[],\"suggestions\":[],\"comments\":\"好\"}"}"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    coordinator
        .run_continue(
            "rc-sig-assembly",
            &story.id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .unwrap();

    let log = coordinator.recorded_activities();
    assert_signal_pair(&log, "producer|start|装配", "producer|done|装配");
}

// ---- 阶段三（P2）C-1：续写「熔断不丢稿」流程级验证 ----
// handle_gate 两处 GateOutcome::Failed 分支经 salvage_failed_gate 降级：
// 草稿 ≥600 字符合成降级 EditorVerdict 放行装配落库；<600 保留 Err 丢稿。
// 流程搭建沿用 test_handle_gate_assembly_activity_signals / E3 的预置资产方式。

/// C-1 流程测试的公共故事脚手架：预置角色/世界观/大纲（ensure_assets 不消费
/// mock 队列）+ 第一章正文，返回 (pool, story_id)。
fn setup_c1_continue_story(title: &str) -> (crate::db::DbPool, String) {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: title.into(),
            description: Some("前提".into()),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '双星文明', '[]', '星环崩塌', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        ).unwrap();
    }
    let scene_repo = SceneRepository::new(pool.clone());
    let ch1 = scene_repo.create(&story.id, 1, Some("第一章")).unwrap();
    scene_repo
        .update(
            &ch1.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("第一章正文。".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
    (pool, story.id)
}

/// C-1 流程测试的公共 mock 脚本：outline + writer 写草稿 + editor 完全失败
/// （tool_loop 连续 3 次散文 ParseFailures 熔断 -> salvage 无果 -> 散文回退
/// 仍非裁决 JSON -> GateOutcome::Failed）。
fn editor_total_failure_script(chapter_content: &str) -> Arc<MockLlm> {
    let write = format!(
        r#"{{"type":"tool","name":"board_write","args":{{"zone":"draft","item_type":"chapter","key":"第2章","content":"{}","summary":"星舰苏醒"}}}}"#,
        chapter_content
    );
    MockLlm::scripted(vec![
        // generate_chapter_outline（Producer 单调用）
        "本章核心冲突：阿苔发现星环秘密。转折：盟友背叛。推进：前往禁区。",
        // writer: 写第 2 章 → final
        write.as_str(),
        r#"{"type":"final","content":"第二章完成"}"#,
        // editor tool_loop: 连续 3 次散文（非 JSON action）-> ParseFailures 熔断
        "这不是JSON工具动作，只是审查意见散文。",
        "依然不是JSON action，本地模型不遵从。",
        "第三次散文，触发连续解析失败熔断。",
        // editor 散文回退：仍非裁决 JSON -> GateOutcome::Failed
        "散文回退也无法给出裁决 JSON，质检完全失败。",
    ])
}

/// C-1：质检完全失败 + 草稿 ≥600 字符 -> salvage_failed_gate 命中，降级
/// EditorVerdict 放行装配落库，run 不失败（熔断不丢稿）。
#[tokio::test]
async fn test_handle_gate_editor_failure_salvages_substantive_draft() {
    let (pool, story_id) = setup_c1_continue_story("C-1 降级放行测试");
    let chapter2 = pass_grade_content("第二章正文：星舰苏醒。");
    assert!(
        chapter2.chars().count() >= 600,
        "前置：草稿须达 substantive 阈值"
    );
    let coordinator =
        AgencyCoordinator::for_test(pool.clone(), editor_total_failure_script(&chapter2));
    let result = coordinator
        .run_continue(
            "rc-c1-salvage",
            &story_id,
            PersistMode::NextChapter { chapter_number: 2 },
            "",
            None,
        )
        .await
        .expect("editor 完全失败但草稿 substantive，应降级放行而非丢稿");
    assert_eq!(result.chapter_number, 2);
    let scene = SceneRepository::new(pool.clone())
        .get_by_id(&result.scene_id)
        .unwrap()
        .unwrap();
    assert!(
        scene
            .content
            .as_deref()
            .unwrap_or("")
            .contains("第二章正文"),
        "降级放行后第二章草稿应装配落库"
    );
    let run = AgencyRepository::new(pool.clone())
        .get_run("rc-c1-salvage")
        .unwrap()
        .unwrap();
    assert_eq!(run.status, "completed", "降级放行后 run 应完成");
}

/// C-1：质检完全失败 + 草稿 <600 字符 -> salvage_failed_gate 未命中，保留
/// Err 丢稿（真垃圾稿照丢）。
#[tokio::test]
async fn test_handle_gate_editor_failure_drops_short_draft() {
    let (pool, story_id) = setup_c1_continue_story("C-1 短稿丢稿测试");
    let short = "第二章正文：只有短短一句，远不足六百字符阈值。";
    assert!(short.chars().count() < 600, "前置：草稿须低于 salvage 阈值");
    let coordinator = AgencyCoordinator::for_test(pool.clone(), editor_total_failure_script(short));
    let err = coordinator
        .run_continue_batch("rc-c1-drop", &story_id, 2, 1)
        .await
        .expect_err("editor 完全失败且草稿过短，应丢稿报错");
    assert!(
        err.to_string().contains("质量门未通过"),
        "错误应为质量门未通过: {}",
        err
    );
    let conn = pool.get().unwrap();
    let cnt: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chapters WHERE story_id = ?1 AND chapter_number = 2",
            rusqlite::params![story_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cnt, 0, "短稿丢稿后不应有第二章落库");
}

#[test]
fn continue_short_retry_user_keeps_beat_card_not_genesis_premise() {
    let user = "【本章节拍任务】\n阵容：阿岩\n【续写硬锚点";
    let retry = continue_short_retry_user(user);
    assert!(retry.contains("【本章节拍任务】"));
    assert!(retry.contains("只输出小说正文"));
    assert!(!retry.contains("故事前提："));
}

#[test]
fn writer_retry_has_time_requires_ninety_seconds() {
    assert!(writer_retry_has_time(None));
    assert!(writer_retry_has_time(Some(90)));
    assert!(writer_retry_has_time(Some(600)));
    assert!(!writer_retry_has_time(Some(89)));
    assert!(!writer_retry_has_time(Some(40)));
    assert!(!writer_retry_has_time(Some(0)));
}

#[test]
fn need_character_relationships_when_two_chars_and_empty_table() {
    assert!(AgencyCoordinator::need_character_relationships(2, 0));
    assert!(AgencyCoordinator::need_character_relationships(5, 0));
    assert!(!AgencyCoordinator::need_character_relationships(1, 0));
    assert!(!AgencyCoordinator::need_character_relationships(2, 1));
    assert!(!AgencyCoordinator::need_character_relationships(0, 0));
}

#[tokio::test]
async fn ensure_assets_upserts_missing_relationships_for_two_characters() {
    let pool = create_test_pool().unwrap();
    let story = crate::db::repositories::StoryRepository::new(pool.clone())
        .create(crate::db::dto::CreateStoryRequest {
            title: "关系补齐".into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c-a', ?1, '甲', '', '', '', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c-b', ?1, '乙', '', '', '', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, is_auto_generated, created_at, updated_at)
             VALUES ('w1', ?1, '世设', '[]', '', '[]', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：甲乙对峙。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
    }
    let llm = MockLlm::scripted(vec![
        r#"[{"source":"甲","target":"乙","relationship_type":"宿敌","emotional_bond":"恨","emotional_intensity":0.9,"reverse_emotional_bond":"惧","reverse_emotional_intensity":0.7,"description":"面和心不和"}]"#,
    ]);
    let coordinator = AgencyCoordinator::for_test(pool.clone(), llm);
    let repo = AgencyRepository::new(pool.clone());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    coordinator
        .ensure_assets(&budget, &repo, "r-rel", &story.id, "续写")
        .await
        .unwrap();
    let rels = crate::db::repositories::CharacterRelationshipRepository::new(pool.clone())
        .get_by_story(&story.id)
        .unwrap();
    assert_eq!(rels.len(), 1, "须写入一条角色关系");
    assert_eq!(rels[0].relationship_type, "宿敌");
    assert_eq!(rels[0].emotional_bond.as_deref(), Some("恨"));
}

#[test]
fn eight_beat_append_quality_contract() {
    use crate::{
        agency::{beat_card::compile_beat_card, persist::persist_append_with_card},
        db::repositories::{
            CharacterRelationshipRepository, CharacterRepository, CreateCharacterRequest,
            CreateStoryRequest, StoryOutlineRepository, StoryRepository,
        },
    };

    let pool = create_test_pool().unwrap();
    let story = StoryRepository::new(pool.clone())
        .create(CreateStoryRequest {
            title: "八拍契约".into(),
            description: None,
            genre: Some("玄幻".into()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();
    let sid = story.id.clone();
    let char_repo = CharacterRepository::new(pool.clone());
    let a = char_repo
        .create(CreateCharacterRequest {
            story_id: sid.clone(),
            name: "阿岩".into(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
            source: None,
            is_auto_generated: None,
            emotional_core: None,
            emotional_trigger: None,
            emotional_wound: None,
            emotional_need: None,
        })
        .unwrap();
    char_repo
        .create(CreateCharacterRequest {
            story_id: sid.clone(),
            name: "林雪".into(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
            source: None,
            is_auto_generated: None,
            emotional_core: None,
            emotional_trigger: None,
            emotional_wound: None,
            emotional_need: None,
        })
        .unwrap();
    let b = char_repo
        .create(CreateCharacterRequest {
            story_id: sid.clone(),
            name: "顾长夜".into(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
            source: None,
            is_auto_generated: None,
            emotional_core: None,
            emotional_trigger: None,
            emotional_wound: None,
            emotional_need: None,
        })
        .unwrap();
    CharacterRelationshipRepository::new(pool.clone())
        .create(
            &sid,
            &a.id,
            &b.id,
            "仇敌",
            None,
            None,
            Some("恨"),
            Some(0.9),
            Some("戒备"),
            Some(0.6),
        )
        .unwrap();
    StoryOutlineRepository::new(pool.clone())
        .create(&sid, "开篇灵堂托梦。钟楼破阵。龙脉重封。", None, 3, None)
        .unwrap();
    let scene_repo = SceneRepository::new(pool.clone());
    let mut conn = pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let scene = scene_repo
        .create_in_tx(&tx, &sid, 1, Some("第一章"))
        .unwrap();
    scene_repo
        .update_in_tx(
            &tx,
            &scene.id,
            &crate::db::repositories::SceneUpdate {
                content: Some("阿岩站在雨巷。".into()),
                setting_location: Some("雨巷".into()),
                characters_present: Some(vec!["阿岩".into(), "顾长夜".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    let loc_catalog = scene_repo
        .create_in_tx(&tx, &sid, 2, Some("地点目录"))
        .unwrap();
    scene_repo
        .update_in_tx(
            &tx,
            &loc_catalog.id,
            &crate::db::repositories::SceneUpdate {
                setting_location: Some("钟楼".into()),
                ..Default::default()
            },
        )
        .unwrap();
    tx.commit().unwrap();
    let scene_id = scene.id;
    drop(conn);

    let pad = "续写增量正文。".repeat(30);
    let increments = [
        format!("阿岩独自在雨巷喝茶。{pad}"),
        format!("阿岩仍在雨巷里闲话。{pad}"),
        format!("阿岩对峙顾长夜，冲突加压，代价已经写在刀上。{pad}"),
        format!("阿岩问顾长夜一句，顾长夜没有答。{pad}"),
        format!("阿岩把话压回去，顾长夜看向巷口。{pad}"),
        format!("他们离开雨巷，潜入钟楼底层。阿岩握刀，顾长夜跟入。{pad}"),
        format!("林雪入场质问阿岩，顾长夜退到钟楼阴影里。{pad}"),
        format!("林雪逼视阿岩，阿岩没有退。{pad}"),
    ];
    let mut content = "阿岩站在雨巷。".to_string();
    for (i, inc) in increments.iter().enumerate() {
        let card = compile_beat_card(&pool, &sid, &content).unwrap();
        if i == 2 {
            let text = card.expansion_quota_text.clone().unwrap_or_default();
            let full = card.render_full();
            assert!(
                text.contains("冲突") || full.contains("本拍扩张任务"),
                "beat 3 must carry conflict quota full={full}"
            );
            assert!(!full.contains("ConflictEscalation"));
        }
        persist_append_with_card(&pool, &scene_id, &content, inc, &card).unwrap();
        content.push_str(inc);
    }
    let scenes = scene_repo.get_by_story(&sid).unwrap();
    assert_eq!(
        scenes.iter().filter(|s| s.sequence_number == 1).count(),
        1,
        "Append 不得新开章"
    );
    let ch1 = scenes.iter().find(|s| s.sequence_number == 1).unwrap();
    assert!(ch1.characters_present.iter().any(|n| n == "阿岩"));
    assert!(ch1.characters_present.iter().any(|n| n == "林雪"));
    assert!(!ch1.characters_present.iter().any(|n| n == "路人甲"));
    assert_eq!(ch1.setting_location.as_deref(), Some("钟楼"));
    let last = compile_beat_card(&pool, &sid, &content).unwrap();
    assert!(
        !last.next_outline_node.starts_with("开篇灵堂"),
        "rewound: {}",
        last.next_outline_node
    );
    let prompt = crate::agency::beat_card::render_writer_user_prompt(
        "【红线】",
        &last,
        "续写",
        &content,
        None,
        None,
    );
    assert!(!prompt.contains("最高优先级"));
}
