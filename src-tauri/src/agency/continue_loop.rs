//! 续写三角色闭环：活动落库、资产投影、审查问题回收。
//! 设计：docs/plans/2026-08-16-agency-continue-loop-design.md
//! 本模块不依赖 coordinator，避免与 spawn 路径循环引用。

use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::{
    agency::{
        board::BlackboardService,
        models::{AgentRole, BoardItem, BoardZone},
        repository::AgencyRepository,
    },
    db::DbPool,
    error::AppError,
    memory::ingest::ContentAnalysis,
};

/// 与 `coordinator::EVENT_AGENT_ACTIVITY` 同字面量；本模块不 import
/// coordinator。
const EVENT_AGENT_ACTIVITY: &str = "agency-agent-activity";

const REVIEW_ISSUE_CAP: usize = 2;
const ASSET_CHAR_CAP: usize = 8;

const ISSUE_STOPWORDS: &[&str] = &[
    "问题", "角色", "冲突", "需要", "应该", "没有", "必须", "本章", "续写", "正文", "建议", "不够",
    "缺失", "明确",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgExit {
    Success,
    Failed,
    Timeout,
    NoLock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetProjection {
    pub item_type: String,
    pub key: String,
    pub content: String,
    pub summary: String,
}

pub fn bg_done_detail(task: &str, exit: BgExit) -> String {
    match exit {
        BgExit::Success => task.to_string(),
        BgExit::Failed => format!("{task}失败"),
        BgExit::Timeout => format!("{task}超时"),
        BgExit::NoLock => format!("{task}未获得锁"),
    }
}

/// 后台编辑审查 fail-open：章节已落库，幕前 done 文案固定「后台审查」。
/// 不合格走 toast（genesis-qc-result），不把「后台审查失败」打进顶栏。
pub fn editor_qc_done_detail() -> String {
    "后台审查".to_string()
}

pub fn persist_activity(pool: &DbPool, run_id: &str, role: AgentRole, action: &str, detail: &str) {
    if let Err(e) =
        AgencyRepository::new(pool.clone()).log_activity(run_id, role.as_str(), action, detail)
    {
        log::warn!("agency: persist activity failed: {e}");
    }
}

pub async fn persist_activity_async(
    pool: &DbPool,
    run_id: &str,
    role: AgentRole,
    action: &str,
    detail: &str,
) {
    let pool = pool.clone();
    let run_id = run_id.to_string();
    let action = action.to_string();
    let detail = detail.to_string();
    let _ = tokio::task::spawn_blocking(move || {
        persist_activity(&pool, &run_id, role, &action, &detail);
    })
    .await;
}

pub fn emit_activity_event(
    app: &AppHandle,
    run_id: &str,
    role: AgentRole,
    action: &str,
    detail: &str,
) {
    let _ = app.emit(
        EVENT_AGENT_ACTIVITY,
        serde_json::json!({
            "run_id": run_id,
            "role": role.as_str(),
            "action": action,
            "detail": detail,
        }),
    );
}

pub async fn emit_logged_activity(
    app: &AppHandle,
    pool: &DbPool,
    run_id: &str,
    role: AgentRole,
    action: &str,
    detail: &str,
) {
    emit_activity_event(app, run_id, role, action, detail);
    persist_activity_async(pool, run_id, role, action, detail).await;
}

pub fn load_open_review_issues(pool: &DbPool, story_id: &str) -> Vec<String> {
    let items = AgencyRepository::new(pool.clone())
        .list_items_for_story(story_id, Some(BoardZone::Review))
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in items.into_iter().rev() {
        if item.status == "resolved" || item.item_type != "gate" {
            continue;
        }
        if !item.summary.contains("gate:revise")
            && parse_gate_outcome(&item.content).as_deref() != Some("revise")
        {
            continue;
        }
        for issue in parse_gate_issues(&item.content) {
            if !issue.trim().is_empty() && !out.iter().any(|x: &String| x == &issue) {
                out.push(issue);
            }
            if out.len() >= REVIEW_ISSUE_CAP {
                return out;
            }
        }
    }
    out
}

pub fn parse_gate_issues(content: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(content) else {
        return Vec::new();
    };
    match v.get("issues") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_gate_outcome(content: &str) -> Option<String> {
    serde_json::from_str::<Value>(content)
        .ok()
        .and_then(|v| v.get("outcome")?.as_str().map(|s| s.to_string()))
}

pub fn issue_tokens(issue: &str) -> Vec<String> {
    let split_cjk = |c: char| {
        matches!(
            c,
            '与' | '和' | '的' | '未' | '仍' | '了' | '在' | '是' | '不' | '得' | '到'
        )
    };
    let mut tokens = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, tokens: &mut Vec<String>| {
        if buf.chars().count() >= 2 {
            tokens.push(buf.clone());
        }
        buf.clear();
    };
    for ch in issue.chars() {
        if is_cjk(ch) && !split_cjk(ch) {
            buf.push(ch);
        } else {
            flush(&mut buf, &mut tokens);
        }
    }
    flush(&mut buf, &mut tokens);
    for w in issue.split(|c: char| !c.is_ascii_alphanumeric()) {
        if w.chars().count() >= 3 {
            tokens.push(w.to_string());
        }
    }
    tokens.retain(|t| !ISSUE_STOPWORDS.contains(&t.as_str()));
    tokens.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));
    tokens.dedup();
    tokens
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4e00}'..='\u{9fff}'
        | '\u{3400}'..='\u{4dbf}'
        | '\u{f900}'..='\u{faff}'
    )
}

pub fn issue_addressed_in_prose(issue: &str, prose: &str) -> bool {
    let tokens = issue_tokens(issue);
    if tokens.is_empty() {
        return false;
    }
    tokens
        .iter()
        .any(|t| t.chars().count() >= 2 && prose.contains(t.as_str()))
}

pub fn resolve_addressed_review_issues(pool: &DbPool, story_id: &str, increment: &str) -> usize {
    let repo = AgencyRepository::new(pool.clone());
    let items = repo
        .list_items_for_story(story_id, Some(BoardZone::Review))
        .unwrap_or_default();
    let mut n = 0usize;
    for item in items {
        if item.status == "resolved" || item.item_type != "gate" {
            continue;
        }
        let issues = parse_gate_issues(&item.content);
        if issues.is_empty() {
            continue;
        }
        let remaining: Vec<String> = issues
            .iter()
            .filter(|i| !issue_addressed_in_prose(i, increment))
            .cloned()
            .collect();
        if remaining.len() == issues.len() {
            continue;
        }
        n += 1;
        if remaining.is_empty() {
            if let Err(e) = repo.set_item_status(&item.id, "resolved") {
                log::warn!("agency: resolve review item failed: {e}");
            }
            continue;
        }
        let mut v: Value = serde_json::from_str(&item.content).unwrap_or(serde_json::json!({}));
        v["issues"] = serde_json::json!(remaining);
        v["rule_issue_count"] = serde_json::json!(remaining.len());
        let summary = format!("gate:revise {} 条问题", remaining.len());
        if let Err(e) = repo.revise_item(&item.id, &v.to_string(), &summary, item.version) {
            log::warn!("agency: revise remaining issues failed: {e}");
        }
    }
    n
}

pub fn beat_card_asset_projections(
    cast_names: &[String],
    scene_outline: &str,
    location: Option<&str>,
) -> Vec<AssetProjection> {
    let mut out = Vec::new();
    if !scene_outline.trim().is_empty() {
        out.push(AssetProjection {
            item_type: "scene_outline".into(),
            key: "outline:scene".into(),
            content: scene_outline.to_string(),
            summary: scene_outline.lines().take(2).collect::<Vec<_>>().join(" "),
        });
    }
    for name in cast_names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .take(ASSET_CHAR_CAP)
    {
        let loc = location.unwrap_or("");
        let content = if loc.is_empty() {
            format!("本拍在场：{name}")
        } else {
            format!("本拍在场：{name} @ {loc}")
        };
        out.push(AssetProjection {
            item_type: "character".into(),
            key: format!("character:{name}"),
            content,
            summary: name.clone(),
        });
    }
    out
}

pub fn ingest_board_projections(analysis: &ContentAnalysis, prose: &str) -> Vec<AssetProjection> {
    let mut out = Vec::new();
    let mut n_chars = 0usize;
    for ent in &analysis.entities {
        if n_chars >= ASSET_CHAR_CAP {
            break;
        }
        let name = ent.name.trim();
        if name.is_empty() {
            continue;
        }
        let is_char = ent.entity_type.eq_ignore_ascii_case("character")
            || ent.entity_type.contains("角色")
            || !ent.role_type.trim().is_empty()
            || !ent.personality.trim().is_empty();
        if !is_char {
            continue;
        }
        if crate::agency::prose_ground::has_substantial_prose(prose)
            && !crate::agency::prose_ground::name_in_prose(name, prose)
        {
            continue;
        }
        let summary = if ent.role_type.trim().is_empty() {
            name.to_string()
        } else {
            format!("{name}（{}）", ent.role_type.trim())
        };
        let content = format!(
            "{name}\n{}\n{}",
            ent.personality.trim(),
            ent.background.trim()
        );
        out.push(AssetProjection {
            item_type: "character".into(),
            key: format!("character:{name}"),
            content,
            summary,
        });
        n_chars += 1;
    }
    if let Some(sd) = &analysis.story_delta {
        let mut parts = Vec::new();
        if !sd.core_conflict.trim().is_empty() {
            parts.push(format!("【核心冲突】{}", sd.core_conflict.trim()));
        }
        for tp in &sd.turning_points {
            if !tp.trim().is_empty() {
                parts.push(format!("【转折点】{}", tp.trim()));
            }
        }
        if !parts.is_empty() {
            let content = parts.join("\n");
            out.push(AssetProjection {
                item_type: "story_outline".into(),
                key: "outline:story".into(),
                content: content.clone(),
                summary: content.chars().take(40).collect(),
            });
        }
    }
    if let Some(wb) = &analysis.world_building {
        if !wb.concept.trim().is_empty() {
            out.push(AssetProjection {
                item_type: "world".into(),
                key: "world:concept".into(),
                content: wb.concept.clone(),
                summary: wb.concept.chars().take(40).collect(),
            });
        }
    }
    out
}

pub fn upsert_run_asset(
    board: &BlackboardService,
    run_id: &str,
    story_id: &str,
    proj: &AssetProjection,
) -> Result<BoardItem, AppError> {
    let existing = board
        .list_zone(run_id, BoardZone::Asset)
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.key == proj.key);
    if let Some(item) = existing {
        match board.revise(
            &item.id,
            AgentRole::Producer,
            &proj.content,
            &proj.summary,
            item.version,
        ) {
            Ok(revised) => Ok(revised),
            Err(_) => board.write(
                run_id,
                story_id,
                AgentRole::Producer,
                BoardZone::Asset,
                &proj.item_type,
                &proj.key,
                &proj.content,
                &proj.summary,
            ),
        }
    } else {
        board.write(
            run_id,
            story_id,
            AgentRole::Producer,
            BoardZone::Asset,
            &proj.item_type,
            &proj.key,
            &proj.content,
            &proj.summary,
        )
    }
}

/// 资产回流本体（不含 start/done 活动信号）。观察与续写 spawn 共用，
/// 避免手写正文与续写各写一套 ingest。
pub async fn run_asset_ingest(
    app: &tauri::AppHandle,
    pool: &DbPool,
    run_id: &str,
    story_id: &str,
    scene_id: &str,
    content: &str,
    external_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> BgExit {
    let bg_permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
    if bg_permit.is_err() {
        log::warn!("agency: 资产回流未获取到后台 LLM 串行许可 (run={})", run_id);
        return BgExit::NoLock;
    }
    let _bg_permit = bg_permit.unwrap();

    let permit = crate::memory::writer::MEMORY_WRITER_SEMAPHORE
        .acquire()
        .await;
    if permit.is_err() {
        log::warn!("agency: 资产回流未获取到 ingest 并发许可 (run={})", run_id);
        return BgExit::NoLock;
    }
    let _permit = permit.unwrap();

    let cancel = tokio_util::sync::CancellationToken::new();
    let timer_cancel = cancel.clone();
    let timer = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        timer_cancel.cancel();
    });

    // v0.59.0：把 run 级取消接进 ingest —— 用户取消生成后，后台回流最长
    // 300s 内也会停下，不再继续占用后台 LLM 串行许可。
    if let Some(flag) = external_cancel {
        use std::sync::atomic::Ordering;
        let token = cancel.clone();
        let run_id_owned = run_id.to_string();
        tauri::async_runtime::spawn(async move {
            loop {
                if flag.load(Ordering::SeqCst) {
                    log::info!("agency: 资产回流收到 run 取消信号 (run={})", run_id_owned);
                    token.cancel();
                    break;
                }
                if token.is_cancelled() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        });
    }

    let llm_service = crate::llm::LlmService::new(app.clone());
    let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service)
        .with_pool(pool.clone())
        .with_app_handle(app.clone());
    let ingest_content = crate::memory::ingest::IngestContent {
        text: content.to_string(),
        source: format!("agency:scene:{}", scene_id),
        story_id: story_id.to_string(),
        scene_id: Some(scene_id.to_string()),
    };
    let exit = match pipeline
        .ingest_with_cancel(&ingest_content, Some(&cancel))
        .await
    {
        Ok(result) => {
            let kg_repo = crate::db::repositories::KnowledgeGraphRepository::new(pool.clone());
            if let Err(e) = kg_repo.save_entities_batch(&result.entities) {
                log::warn!("agency: 资产回流 KG 实体落库失败 (run={}): {}", run_id, e);
            }
            if let Err(e) = kg_repo.save_relations_batch(&result.relations) {
                log::warn!("agency: 资产回流 KG 关系落库失败 (run={}): {}", run_id, e);
            }
            let extra = ingest_board_projections(&result.analysis, content);
            project_assets_to_run(pool, run_id, story_id, &extra);
            log::info!(
                "agency: 资产回流完成 (run={}, story={}): {} entities, {} relations",
                run_id,
                story_id,
                result.entities.len(),
                result.relations.len()
            );
            if cancel.is_cancelled() {
                BgExit::Timeout
            } else {
                BgExit::Success
            }
        }
        Err(e) => {
            log::warn!("agency: 后台资产回流失败 (run={}): {}", run_id, e);
            if cancel.is_cancelled() {
                BgExit::Timeout
            } else {
                BgExit::Failed
            }
        }
    };
    timer.abort();
    exit
}

pub fn project_assets_to_run(
    pool: &DbPool,
    run_id: &str,
    story_id: &str,
    projections: &[AssetProjection],
) -> usize {
    if projections.is_empty() {
        return 0;
    }
    let board = BlackboardService::new(pool.clone());
    let mut n = 0usize;
    for proj in projections {
        match upsert_run_asset(&board, run_id, story_id, proj) {
            Ok(_) => n += 1,
            Err(e) => log::warn!("agency: project asset {} failed: {e}", proj.key),
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::models::AgencyRun,
        db::connection::create_test_pool,
        memory::ingest::{ContentAnalysis, StoryDelta, WbDelta},
    };

    fn empty_analysis() -> ContentAnalysis {
        serde_json::from_value(serde_json::json!({ "sentiment": {} })).unwrap()
    }

    fn char_entity(name: &str, role: &str) -> crate::memory::ingest::AnalyzedEntity {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "entity_type": "character",
            "role_type": role,
        }))
        .unwrap()
    }

    fn seed_run(pool: &DbPool, run_id: &str, story_id: &str) {
        let repo = AgencyRepository::new(pool.clone());
        repo.create_run(&AgencyRun::new(run_id, "续写")).unwrap();
        repo.set_run_story(run_id, story_id).unwrap();
    }

    #[test]
    fn bg_done_detail_covers_all_exits() {
        assert_eq!(bg_done_detail("资产回流", BgExit::Success), "资产回流");
        assert_eq!(bg_done_detail("资产回流", BgExit::Failed), "资产回流失败");
        assert_eq!(bg_done_detail("资产回流", BgExit::Timeout), "资产回流超时");
        assert_eq!(
            bg_done_detail("资产回流", BgExit::NoLock),
            "资产回流未获得锁"
        );
        assert_eq!(bg_done_detail("后台审查", BgExit::Failed), "后台审查失败");
        assert_eq!(bg_done_detail("后台补齐", BgExit::Timeout), "后台补齐超时");
    }

    #[test]
    fn editor_qc_done_detail_is_fail_open_not_failure() {
        assert_eq!(editor_qc_done_detail(), "后台审查");
        assert_ne!(
            editor_qc_done_detail(),
            bg_done_detail("后台审查", BgExit::Failed)
        );
    }

    #[test]
    fn persist_activity_writes_start_and_done() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool, "act-1", "story-1");
        persist_activity(&pool, "act-1", AgentRole::Producer, "start", "资产回流");
        persist_activity(
            &pool,
            "act-1",
            AgentRole::Producer,
            "done",
            &bg_done_detail("资产回流", BgExit::Failed),
        );
        let rows = AgencyRepository::new(pool)
            .list_activities("act-1", 20)
            .unwrap();
        let pairs: Vec<(String, String)> = rows
            .into_iter()
            .filter(|r| r.event_type == "activity")
            .map(|r| (r.action.unwrap_or_default(), r.detail.unwrap_or_default()))
            .collect();
        assert!(pairs.iter().any(|(a, d)| a == "start" && d == "资产回流"));
        assert!(pairs
            .iter()
            .any(|(a, d)| a == "done" && d == "资产回流失败"));
    }

    #[test]
    fn beat_projections_fill_asset_keys() {
        let ps = beat_card_asset_projections(
            &["苏会山".into(), "曹元佩".into()],
            "【当前场大纲】\n下一拍：留在大堂",
            Some("镇北王府大堂"),
        );
        assert!(ps.iter().any(|p| p.key == "outline:scene"));
        assert!(ps.iter().any(|p| p.key == "character:苏会山"));
        assert!(ps.iter().any(|p| p.item_type == "character"));
    }

    #[test]
    fn ingest_projections_drop_names_absent_from_prose() {
        let mut analysis = empty_analysis();
        analysis.entities = vec![
            char_entity("苏会山", "主角"),
            char_entity("费迪南三世", "皇帝"),
        ];
        let mut prose = "苏会山在镇北王府大堂接过酒盏。".to_string();
        while prose.chars().count() < 200 {
            prose.push_str("红毡未干。");
        }
        let ps = ingest_board_projections(&analysis, &prose);
        assert!(ps.iter().any(|p| p.key == "character:苏会山"));
        assert!(!ps.iter().any(|p| p.key.contains("费迪南")));
    }

    #[test]
    fn ingest_projections_include_story_and_world() {
        let mut analysis = empty_analysis();
        analysis.story_delta = Some(StoryDelta {
            core_conflict: "谁掌握火药账".into(),
            turning_points: vec!["席间行刺".into()],
            ..Default::default()
        });
        analysis.world_building = Some(WbDelta {
            concept: "知启纪元大奉帝国".into(),
            ..Default::default()
        });
        let ps = ingest_board_projections(&analysis, "短");
        assert!(ps.iter().any(|p| p.key == "outline:story"));
        assert!(ps.iter().any(|p| p.key == "world:concept"));
    }

    #[test]
    fn production_change_projects_to_current_run_asset_zone() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool, "run-p", "st-p");
        let ps =
            beat_card_asset_projections(&["阿岩".into()], "【当前场大纲】\n下一拍：夜宴破裂", None);
        let n = project_assets_to_run(&pool, "run-p", "st-p", &ps);
        assert!(n >= 1);
        let assets = BlackboardService::new(pool)
            .list_zone("run-p", BoardZone::Asset)
            .unwrap();
        assert!(!assets.is_empty());
        assert!(assets.iter().any(|i| i.key == "outline:scene"));
    }

    #[test]
    fn load_open_review_issues_caps_at_two_from_prior_run() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool, "old-run", "st-rev");
        let board = BlackboardService::new(pool.clone());
        let content = serde_json::json!({
            "outcome": "revise",
            "issues": ["苏会山与曹元佩的冲突未兑现", "地点仍停在大堂门口", "第三人称跳点"],
        })
        .to_string();
        board
            .write(
                "old-run",
                "st-rev",
                AgentRole::EditorAuditor,
                BoardZone::Review,
                "gate",
                "gate-ch2-r1",
                &content,
                "gate:revise 3 条问题",
            )
            .unwrap();
        let issues = load_open_review_issues(&pool, "st-rev");
        assert_eq!(issues.len(), 2);
        assert!(issues[0].contains("苏会山"));
    }

    #[test]
    fn addressed_issue_marks_board_item_resolved() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool, "r-res", "st-res");
        let board = BlackboardService::new(pool.clone());
        let content = serde_json::json!({
            "outcome": "revise",
            "issues": ["苏会山与曹元佩的冲突未兑现"],
        })
        .to_string();
        board
            .write(
                "r-res",
                "st-res",
                AgentRole::EditorAuditor,
                BoardZone::Review,
                "gate",
                "gate-x",
                &content,
                "gate:revise 1 条问题",
            )
            .unwrap();
        let n =
            resolve_addressed_review_issues(&pool, "st-res", "苏会山抓住曹元佩的手腕，短刃落地。");
        assert_eq!(n, 1);
        let items = AgencyRepository::new(pool)
            .list_items_for_story("st-res", Some(BoardZone::Review))
            .unwrap();
        assert_eq!(items[0].status, "resolved");
    }

    #[test]
    fn issue_addressed_requires_named_token() {
        assert!(issue_addressed_in_prose(
            "苏会山与曹元佩的冲突未兑现",
            "苏会山转身挡住曹元佩。"
        ));
        assert!(!issue_addressed_in_prose(
            "苏会山与曹元佩的冲突未兑现",
            "雨还在下，灯笼晃了一下。"
        ));
    }

    #[test]
    fn upsert_same_key_does_not_duplicate() {
        let pool = create_test_pool().unwrap();
        seed_run(&pool, "r-up", "st-up");
        let p = AssetProjection {
            item_type: "character".into(),
            key: "character:阿岩".into(),
            content: "v1".into(),
            summary: "阿岩".into(),
        };
        project_assets_to_run(&pool, "r-up", "st-up", &[p.clone()]);
        let p2 = AssetProjection {
            content: "v2".into(),
            ..p
        };
        project_assets_to_run(&pool, "r-up", "st-up", &[p2]);
        let assets = BlackboardService::new(pool)
            .list_zone("r-up", BoardZone::Asset)
            .unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].content, "v2");
    }
}
