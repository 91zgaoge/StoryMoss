use tauri::{AppHandle, State};

use crate::{
    agency::{
        board::BlackboardService,
        coordinator::{cancel_agency_run, AgencyCheckpoint, AgencyCoordinator},
        models::{AgencyRun, BoardItem},
        repository::AgencyRepository,
    },
    db::DbPool,
    error::AppError,
};

/// 启动创世 2.0：立即返回 run_id，进度经 `agency-run-progress` 事件推送。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_start_genesis(
    premise: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    crate::agency::coordinator::validate_premise(&premise)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator.run_genesis(&rid, &premise).await {
            log::error!("agency genesis run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 续写下一章：同一 story 不允许并行 run；章号 = MAX(sequence_number)+1。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_chapter(
    story_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();
    // 并发护栏：同一 story 不允许并行 run
    let pool_guard = pool.clone();
    let sid_guard = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard)
            .has_running_run_for_story(&sid_guard)
    })
    .await
    .map_err(|e| AppError::from(format!("guard join error: {}", e)))?
    .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed(
            "该故事已有进行中的创作任务",
            None::<String>,
        ));
    }
    // 下一章号
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let chapter_number = tokio::task::spawn_blocking(move || -> Result<i32, AppError> {
        let conn = pool2
            .get()
            .map_err(|e| AppError::from(format!("pool: {}", e)))?;
        conn.query_row(
            "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM scenes WHERE story_id = ?1",
            rusqlite::params![sid2],
            |r| r.get(0),
        )
        .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator
            .run_continue(&rid, &story_id, chapter_number)
            .await
        {
            log::error!("agency continue run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 批量续写：并行稳态循环（gate(n-1) ∥ writer(n)），立即返回 run_id。
/// count 默认 3，钳制 1..=5；起始章号 = MAX(sequence_number)+1。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_batch(
    story_id: String,
    count: Option<u32>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let count = (count.unwrap_or(3) as usize).clamp(1, 5);
    let pool = pool.inner().clone();
    // 并发护栏：同一 story 不允许并行 run
    let pool_guard = pool.clone();
    let sid = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard).has_running_run_for_story(&sid)
    })
    .await
    .map_err(|e| AppError::from(format!("guard join error: {}", e)))?
    .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed(
            "该故事已有进行中的创作任务",
            None::<String>,
        ));
    }
    // 起始章号
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let start_chapter =
        tokio::task::spawn_blocking(move || AgencyCoordinator::next_chapter_number(&pool2, &sid2))
            .await
            .map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator
            .run_continue_batch(&rid, &story_id, start_chapter, count)
            .await
        {
            log::error!("agency batch run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 跨会话恢复：立即返回 ResumeOutcome（含 new_run_id），续写 batch 在后台执行。
/// 进度经 agency-run-progress / agency-agent-activity 事件推送；取消用 agency_cancel_run(new_run_id)。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_resume_run(
    old_run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::coordinator::ResumeOutcome, AppError> {
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let outcome = coordinator.resume_prepare(&old_run_id).await?;
    let (new_run_id, story_id) = (outcome.new_run_id.clone(), outcome.story_id.clone());
    let outcome_ret = outcome.clone();
    tauri::async_runtime::spawn(async move {
        let start =
            match AgencyCoordinator::next_chapter_number_async(&coordinator, &story_id).await {
                Ok(n) => n,
                Err(e) => {
                    log::error!("resume batch chapter number failed: {}", e);
                    return;
                }
            };
        if let Err(e) = coordinator
            .run_continue_batch(&new_run_id, &story_id, start, 1)
            .await
        {
            log::error!("resume batch run {} failed: {}", new_run_id, e);
        }
    });
    Ok(outcome_ret)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_run(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<AgencyRun>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        AgencyRepository::new(pool)
            .get_run(&run_id)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_get_run join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_list_board(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<BoardItem>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        BlackboardService::new(pool)
            .repo()
            .list_items(&run_id, None)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_list_board join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_cancel_run(
    run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    if !cancel_agency_run(&run_id) {
        log::warn!(
            "agency_cancel_run: run {} 不在取消注册表中（不存在或已结束）",
            run_id
        );
    }
    // 定点取消：仅取消该 run 的在途 LLM 调用，不再全局 cancel_all
    let llm = crate::llm::LlmService::new(app_handle);
    crate::agency::coordinator::cancel_requests_for_run(&llm, &run_id);
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        let repo = AgencyRepository::new(pool);
        match repo.get_run(&run_id) {
            Ok(Some(run)) => {
                if run.status == "running" || run.status == "pending" {
                    if let Err(e) = repo.finish_run(&run_id, "cancelled", None, Some("用户取消"))
                    {
                        log::warn!(
                            "agency_cancel_run: 标记 run {} 为 cancelled 失败: {}",
                            run_id,
                            e
                        );
                    }
                }
            }
            Ok(None) => log::warn!("agency_cancel_run: run {} 不存在", run_id),
            Err(e) => log::warn!("agency_cancel_run: 读取 run {} 失败: {}", run_id, e),
        }
    })
    .await
    .map_err(|e| AppError::from(format!("agency_cancel_run join error: {}", e)))?;
    Ok(())
}

/// 按 story 列出里程碑检查点（created_at 升序）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_list_checkpoints(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<AgencyCheckpoint>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool).list_checkpoints(&story_id).map_err(AppError::from)
    }).await.map_err(|e| AppError::from(format!("list_checkpoints join error: {}", e)))?
}

/// 采集该 story 的 human 修改率信号（后置评分，不进 gate）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_human_signals(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::agency::graders::HumanSignal>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || crate::agency::graders::human_signals(&pool, &story_id))
        .await
        .map_err(|e| AppError::from(format!("human_signals join error: {}", e)))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateHistoryItem {
    pub key: String,
    pub outcome: String,
    pub weighted: Option<f64>,
    pub code: Option<f64>,
    pub rule: Option<f64>,
    pub model: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PurposeUsage {
    pub purpose: String,
    pub calls: i64,
    pub total_tokens: i64,
    pub total_duration_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvalOverview {
    pub gate_history: Vec<GateHistoryItem>,
    pub pass_rate: f64,
    pub checkpoints: Vec<crate::agency::coordinator::AgencyCheckpoint>,
    pub human_signals: Vec<crate::agency::graders::HumanSignal>,
    pub token_usage: Vec<PurposeUsage>,
}

/// 评估仪表盘五段聚合：gate 历史 + pass_rate + checkpoints + human_signals + token_usage。
fn eval_overview(pool: &DbPool, story_id: &str) -> Result<EvalOverview, AppError> {
    let conn = pool.get().map_err(|e| AppError::from(format!("pool: {}", e)))?;
    // gate 历史（review 区 item_type='gate'）
    let mut stmt = conn.prepare(
        "SELECT key, content, created_at FROM agency_board_items
         WHERE story_id = ?1 AND item_type = 'gate' ORDER BY created_at ASC, rowid ASC")?;
    let mut pass = 0usize;
    let mut total = 0usize;
    let gate_history: Vec<GateHistoryItem> = stmt.query_map(rusqlite::params![story_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?.filter_map(|r| r.ok()).map(|(key, content, created_at)| {
        let json: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        let outcome = json.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let gs = json.get("gate_score");
        let f = |k: &str| gs.and_then(|g| g.get(k)).and_then(|v| v.as_f64());
        if outcome == "pass" { pass += 1; }
        total += 1;
        GateHistoryItem {
            key,
            outcome,
            weighted: f("weighted"),
            code: f("code"),
            rule: f("rule"),
            model: f("model"),
            created_at,
        }
    }).collect();
    let pass_rate = if total == 0 { 0.0 } else { pass as f64 / total as f64 };
    // token 用量（llm_calls purpose 聚合）
    let mut usage_stmt = conn.prepare(
        "SELECT purpose, COUNT(*), SUM(total_tokens), SUM(duration_ms)
         FROM llm_calls WHERE purpose IN ('agency_writer','agency_producer','agency_editor')
         GROUP BY purpose")?;
    let token_usage: Vec<PurposeUsage> = usage_stmt.query_map([], |r| {
        Ok(PurposeUsage {
            purpose: r.get(0)?,
            calls: r.get(1)?,
            total_tokens: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            total_duration_ms: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
        })
    })?.filter_map(|r| r.ok()).collect();
    let checkpoints = crate::agency::repository::AgencyRepository::new(pool.clone())
        .list_checkpoints(story_id).map_err(AppError::from)?;
    let human_signals = crate::agency::graders::human_signals(pool, story_id);
    Ok(EvalOverview { gate_history, pass_rate, checkpoints, human_signals, token_usage })
}

/// 评估仪表盘聚合数据（五段）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_eval_overview(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<EvalOverview, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || eval_overview(&pool, &story_id))
        .await
        .map_err(|e| AppError::from(format!("eval_overview join error: {}", e)))?
}

/// 对比两个检查点的指标差值（b - a）。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_compare_checkpoints(
    checkpoint_a: String,
    checkpoint_b: String,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::coordinator::CheckpointDiff, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<_, AppError> {
        let repo = crate::agency::repository::AgencyRepository::new(pool);
        let a = repo.get_checkpoint(&checkpoint_a).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed("checkpoint_a 不存在", None::<String>))?;
        let b = repo.get_checkpoint(&checkpoint_b).map_err(AppError::from)?
            .ok_or_else(|| AppError::validation_failed("checkpoint_b 不存在", None::<String>))?;
        Ok(crate::agency::coordinator::compare_checkpoints(&a, &b))
    }).await.map_err(|e| AppError::from(format!("compare join error: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::models::{AgentRole, BoardItem, BoardZone};
    use crate::db::create_test_pool;

    fn seed_gate_item(pool: &DbPool, story_id: &str, key: &str, outcome: &str, weighted: f64) {
        let content = serde_json::json!({
            "outcome": outcome,
            "gate_score": { "weighted": weighted, "code": 0.9, "rule": 0.8, "model": 0.8 }
        })
        .to_string();
        let item = BoardItem::new(
            "run-1",
            story_id,
            BoardZone::Review,
            "gate",
            key,
            content,
            "",
            AgentRole::EditorAuditor,
            "active",
        );
        AgencyRepository::new(pool.clone()).insert_item(&item).unwrap();
    }

    #[test]
    fn eval_overview_gate_history_and_pass_rate() {
        let pool = create_test_pool().unwrap();
        seed_gate_item(&pool, "story-1", "gate-第1章-r1", "pass", 0.82);
        seed_gate_item(&pool, "story-1", "gate-第2章-r1", "revise", 0.60);
        // 其他 story 的 gate 条目不应混入
        seed_gate_item(&pool, "story-2", "gate-第1章-r1", "pass", 0.90);

        let overview = eval_overview(&pool, "story-1").unwrap();
        assert_eq!(overview.gate_history.len(), 2);
        assert!((overview.pass_rate - 0.5).abs() < 1e-9);
        assert_eq!(overview.gate_history[0].key, "gate-第1章-r1");
        assert_eq!(overview.gate_history[0].outcome, "pass");
        assert_eq!(overview.gate_history[0].weighted, Some(0.82));
        assert_eq!(overview.gate_history[1].outcome, "revise");
        assert!(overview.checkpoints.is_empty());
        assert!(overview.human_signals.is_empty());
        // usage 聚合作空表容忍
        assert!(overview.token_usage.is_empty());
    }

    #[test]
    fn eval_overview_token_usage_groups_agency_purposes() {
        let pool = create_test_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            for (id, purpose, tokens, ms) in [
                ("c1", "agency_writer", 100i64, 10i64),
                ("c2", "agency_writer", 300, 30),
                ("c3", "other", 999, 99),
            ] {
                conn.execute(
                    "INSERT INTO llm_calls (id, model_id, purpose, total_tokens, duration_ms, created_at)
                     VALUES (?1, 'm1', ?2, ?3, ?4, '2026-07-17T10:00:00')",
                    rusqlite::params![id, purpose, tokens, ms],
                )
                .unwrap();
            }
        }

        let overview = eval_overview(&pool, "story-1").unwrap();
        // 无 gate 条目时 pass_rate = 0
        assert_eq!(overview.pass_rate, 0.0);
        // 仅 agency_* 角色纳入聚合，按 purpose 分组
        assert_eq!(overview.token_usage.len(), 1);
        let w = &overview.token_usage[0];
        assert_eq!(w.purpose, "agency_writer");
        assert_eq!(w.calls, 2);
        assert_eq!(w.total_tokens, 400);
        assert_eq!(w.total_duration_ms, 40);
    }
}
