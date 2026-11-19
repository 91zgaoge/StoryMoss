//! Model capability audit & feedback loop — v0.11.0
//!
//! 提供：
//! 1. 按任务类型对模型进行轻量 benchmark（探测 + 评分）
//! 2. 从 llm_calls 聚合生成模型健康报告
//! 3. 用户/系统反馈回填，修正路由偏好

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager};

use super::TaskType;
use crate::{
    config::settings::AppConfig,
    db::DbPool,
    error::AppError,
    llm::service::LlmService,
    model_gateway::{
        executor::GatewayExecutor,
        types::{CapabilityProfile, HealthStatus},
    },
};

/// 单个任务类型的 benchmark 结果
#[derive(Debug, Clone, Serialize)]
pub struct TaskBenchmarkResult {
    pub task: TaskType,
    pub model_id: String,
    pub model_name: String,
    pub success: bool,
    pub latency_ms: u64,
    pub score: f64,
    pub reason: String,
}

/// 模型健康报告
#[derive(Debug, Clone, Serialize)]
pub struct ModelHealthReport {
    pub model_id: String,
    pub model_name: String,
    /// 近 N 次调用成功率（0-1）
    pub success_rate: f64,
    /// 平均延迟（ms）
    pub avg_latency_ms: f64,
    /// 平均质量分（如有）
    pub avg_quality_score: Option<f64>,
    /// v0.23.14: 生成速度（tokens/second），来自实时探测
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tps: Option<f64>,
    /// v0.23.14: 综合能力得分（0-100），来自算力档案 benchmark
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_score: Option<f64>,
    /// v0.23.14: 速度得分（0-100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_score: Option<f64>,
    /// v0.23.14: 质量得分（0-100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f64>,
    /// v0.99.3：从 GPU OOM 实测学到的 prompt 上限（本地小模型诊断）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_limit_tokens: Option<u32>,
    /// 最近错误信息
    pub last_error: Option<String>,
    /// 综合健康评级：healthy / degraded / unhealthy
    pub status: String,
    /// v0.17.1: 最近聚合的调用次数（让用户判断数据是否陈旧）
    #[serde(default)]
    pub total_calls: i64,
    /// v0.17.1: 最近一次调用时间（ISO8601）
    #[serde(default)]
    pub last_called_at: Option<String>,
    /// v0.17.1: 报告生成时间（ISO8601），刷新后端会更新
    #[serde(default)]
    pub generated_at: String,
}

/// 路由反馈条目：用户或系统对某次路由结果的评价
#[derive(Debug, Clone, Deserialize)]
pub struct RouteFeedback {
    pub call_id: String,
    /// 1-5 分，5 为非常满意
    pub score: i32,
    pub comment: Option<String>,
}

/// 对指定模型执行轻量任务 benchmark
#[command]
pub async fn benchmark_model_for_task(
    model_id: String,
    task: TaskType,
    app_handle: AppHandle,
) -> Result<TaskBenchmarkResult, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    let profile =
        config
            .llm_profiles
            .get(&model_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound {
                resource: "llm_profile".to_string(),
                id: model_id.clone(),
            })?;

    let prompt = benchmark_prompt_for_task(task);
    let service = LlmService::new(app_handle.clone());
    let start = std::time::Instant::now();

    match service
        .generate_with_profile_and_request_id(
            &profile.id,
            prompt,
            Some(256),
            Some(0.3),
            Some("benchmark"),
            None,
            None,
            None,
        )
        .await
    {
        (_request_id, Ok(response)) => {
            let latency_ms = start.elapsed().as_millis();
            let latency_ms_u64 = latency_ms as u64;
            let score = heuristic_score(&response.content, task);
            Ok(TaskBenchmarkResult {
                task,
                model_id: profile.id.clone(),
                model_name: profile.name.clone(),
                success: true,
                latency_ms: latency_ms_u64,
                score,
                reason: format!("{}ms, 启发式评分 {:.1}", latency_ms_u64, score),
            })
        }
        (request_id, Err(e)) => Ok(TaskBenchmarkResult {
            task,
            model_id: profile.id.clone(),
            model_name: profile.name.clone(),
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            score: 0.0,
            reason: format!("生成失败: {}, request_id={}", e, request_id),
        }),
    }
}

/// 生成所有启用模型的健康报告
/// v0.23.14: 数据源从 llm_calls 历史表切换为 HealthRegistry 实时探测快照。
/// 启动时 llm_calls 已归零，健康报告 100% 反映本次会话的实时探测结果，
/// 彻底杜绝死模型/已删除模型出现在健康报告中。
#[command]
pub async fn get_model_health_reports(
    _window_limit: Option<i64>,
    app_handle: AppHandle,
) -> Result<Vec<ModelHealthReport>, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let config = AppConfig::load(&app_dir).map_err(AppError::from)?;

    let executor = app_handle
        .try_state::<GatewayExecutor>()
        .ok_or_else(|| AppError::internal("GatewayExecutor not available".to_string()))?;

    // v0.23.14: 加载算力档案，获取 benchmark 合成的能力分数
    let cap_profiles: std::collections::HashMap<String, CapabilityProfile> = app_handle
        .try_state::<DbPool>()
        .and_then(|pool| {
            crate::model_gateway::capability_store::CapabilityStore::new(pool.inner().clone())
                .load_all()
                .ok()
        })
        .map(|profiles| {
            profiles
                .into_iter()
                .map(|p| (p.model_id.clone(), p))
                .collect()
        })
        .unwrap_or_default();

    let now_iso = chrono::Local::now().to_rfc3339();
    let mut reports = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // 从 HealthRegistry 实时探测快照生成报告
    let health = executor.health_registry();
    if let Ok(guard) = health.lock() {
        for snapshot in guard.all() {
            seen_ids.insert(snapshot.model_id.clone());
            let success_rate = guard.probe_success_rate(&snapshot.model_id).unwrap_or(0.0);
            let total_calls = guard.probe_count(&snapshot.model_id) as i64;

            let status = match snapshot.status {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Degraded => "degraded",
                HealthStatus::Unhealthy => "unhealthy",
                HealthStatus::Unknown => "unknown",
            }
            .to_string();

            let cap = cap_profiles.get(&snapshot.model_id);
            let prompt_limit_tokens = guard.prompt_limit_tokens(&snapshot.model_id);
            reports.push(ModelHealthReport {
                model_id: snapshot.model_id,
                model_name: snapshot.model_name,
                success_rate,
                avg_latency_ms: snapshot.ttfb_ms.unwrap_or(0) as f64,
                avg_quality_score: None,
                tps: snapshot.tps,
                capability_score: cap.and_then(|c| c.capability_score),
                speed_score: cap.and_then(|c| c.speed_score),
                quality_score: cap.and_then(|c| c.quality_score),
                prompt_limit_tokens,
                last_error: snapshot.last_error.clone(),
                status,
                total_calls,
                last_called_at: snapshot.last_checked_at.clone(),
                generated_at: now_iso.clone(),
            });
        }
    }

    // 补充配置中有但尚未探测的启用模型（status: unknown）
    for profile in config.llm_profiles.values().filter(|p| p.enabled) {
        if !seen_ids.contains(&profile.id) {
            let cap = cap_profiles.get(&profile.id);
            reports.push(ModelHealthReport {
                model_id: profile.id.clone(),
                model_name: profile.name.clone(),
                success_rate: 0.0,
                avg_latency_ms: 0.0,
                avg_quality_score: None,
                tps: None,
                capability_score: cap.and_then(|c| c.capability_score),
                speed_score: cap.and_then(|c| c.speed_score),
                quality_score: cap.and_then(|c| c.quality_score),
                prompt_limit_tokens: None,
                last_error: None,
                status: "unknown".to_string(),
                total_calls: 0,
                last_called_at: None,
                generated_at: now_iso.clone(),
            });
        }
    }

    reports.sort_by(|a, b| b.success_rate.partial_cmp(&a.success_rate).unwrap());
    Ok(reports)
}

/// 提交路由/生成反馈，回填到 llm_calls 记录
#[command]
pub fn submit_route_feedback(
    feedback: RouteFeedback,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let pool = app_handle
        .try_state::<DbPool>()
        .ok_or_else(|| AppError::internal("DbPool not available".to_string()))?;

    let conn = pool
        .inner()
        .get()
        .map_err(|e| AppError::internal(format!("Failed to get db connection: {}", e)))?;

    let audit = serde_json::json!({
        "feedback_score": feedback.score.clamp(1, 5),
        "comment": feedback.comment,
        "submitted_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();

    conn.execute(
        "UPDATE llm_calls SET audit_feedback = ?1 WHERE id = ?2",
        [&audit, &feedback.call_id],
    )
    .map_err(|e| AppError::internal(format!("Failed to update feedback: {}", e)))?;

    Ok(())
}

fn benchmark_prompt_for_task(task: TaskType) -> String {
    match task {
        TaskType::CreativeWriting => "用一句话写一个带有悬念的开头。".to_string(),
        TaskType::Editing => "将以下句子改写得更加流畅：'他走进了房间，看到了她。'".to_string(),
        TaskType::Analysis => "分析'他握紧了拳头'这句话暗示的人物情绪。".to_string(),
        TaskType::Dialogue => "为一位古代剑客写一句符合身份的台词。".to_string(),
        TaskType::Summarization => {
            "用一句话总结：'春天来了，花儿开了，鸟儿在枝头歌唱。'".to_string()
        }
        TaskType::Brainstorming => "给出三个关于'时间旅行'的故事创意。".to_string(),
        TaskType::Proofreading => "找出这句话中的错别字：'他兴高彩烈地离开了。'".to_string(),
        TaskType::WorldBuilding => "描述一个终年下雪的魔法城市。".to_string(),
        TaskType::Vision => "（vision benchmark 需由多模态模块单独实现）".to_string(),
        TaskType::ImageGeneration => {
            "（image generation benchmark 需由图像模块单独实现）".to_string()
        }
    }
}

fn heuristic_score(text: &str, task: TaskType) -> f64 {
    let len = text.chars().count() as f64;
    if len < 5.0 {
        return 10.0;
    }
    let mut score = 50.0;
    score += len.min(200.0) * 0.1;

    match task {
        TaskType::CreativeWriting | TaskType::WorldBuilding => {
            if text.contains("？") || text.contains("!") || text.contains("？") {
                score += 10.0;
            }
        }
        TaskType::Proofreading => {
            if text.contains("采") {
                score += 20.0;
            }
        }
        TaskType::Brainstorming => {
            if text.contains("1.") || text.contains("2.") || text.contains("3.") {
                score += 15.0;
            }
        }
        _ => {}
    }

    score.min(100.0)
}
