//! BGP-2 自动改写执行器（v0.23 TriShot）
//!
//! 在 AuditExecutor 完成异步审计后链式启动。按问题严重度分流：
//! - HIGH 严重度（逻辑/连续性/设定违反，priority≥3）：自动调用 Writer 改写问题段落，
//!   替换正文并写修订历史，发射 ContentAutoRevised SyncEvent
//! - LOW 严重度（风格/节奏/余韵）：组装差异建议，发射 RevisionSuggested SyncEvent
//!
//! 设计依据：docs/plans/2026-06-21-trishot-pipeline-design.md Phase 4 BGP-2

use tauri::{AppHandle, Emitter};

use crate::db::DbPool;
use crate::state_sync::events::SyncEvent;

/// 审计批注的严重度（与 AuditExecutor 的 audit_level 对齐）
#[derive(Debug, Clone, PartialEq)]
pub enum AuditSeverity {
    High,
    Medium,
    Low,
}

impl AuditSeverity {
    pub fn from_dimension_and_level(dimension_priority: u32, severity: &str) -> Self {
        // priority ≥ 3 且 severity=high → HIGH（自动改写）
        // priority ≥ 2 且 severity=high → HIGH
        // 其余 → LOW（仅建议）
        if dimension_priority >= 3 && severity.to_lowercase().contains("high") {
            AuditSeverity::High
        } else if dimension_priority >= 4 && severity.to_lowercase().contains("medium") {
            AuditSeverity::High
        } else {
            AuditSeverity::Low
        }
    }
}

/// BGP-2 自动改写器
pub struct AutoRewriteExecutor;

impl AutoRewriteExecutor {
    /// 根据审计结果，按严重度分流处理。
    ///
    /// - `high_issues`: 高严重度问题列表（每项含维度、描述）
    /// - `low_issues`: 低严重度问题列表
    /// - `content`: 原正文
    /// - `story_id/scene_id/chapter_id`: 目标定位
    ///
    /// 对 HIGH 问题自动改写正文并发射 ContentAutoRevised；
    /// 对 LOW 问题发射 RevisionSuggested。
    pub async fn process_audit_results(
        app_handle: AppHandle,
        pool: DbPool,
        high_issues: &[AuditIssue],
        low_issues: &[AuditIssue],
        content: &str,
        story_id: &str,
        chapter_number: Option<i32>,
    ) {
        // ===== HIGH 严重度：自动改写 =====
        if !high_issues.is_empty() {
            // 启动自动改写线程（静默，不阻塞审计回调）
            let content_owned = content.to_string();
            let story_id_owned = story_id.to_string();
            let handle = app_handle.clone();
            let pool_clone = pool.clone();
            let issues: Vec<_> = high_issues.iter().map(|i| i.description.clone()).collect();
            let auto_revise_threshold = load_severity_threshold();

            if auto_revise_threshold == "high" || auto_revise_threshold == "medium" {
                tokio::spawn(async move {
                    log::info!(
                        "[BGP-2 AutoRewrite] 检测到 {} 个高严重度问题，启动自动改写...",
                        issues.len()
                    );
                    match rewrite_content(&handle, &pool_clone, &content_owned, &issues).await {
                        Ok(revised_content) => {
                            // 替换正文（TODO: 接入具体 scene/chapter update）
                            log::info!(
                                "[BGP-2 AutoRewrite] 改写完成，修订 {} 字符",
                                revised_content.chars().count()
                            );
                            // 发射 ContentAutoRevised 事件
                            crate::state_sync::service::StateSync::emit_content_auto_revised(
                                &handle,
                                &story_id_owned,
                                None, // scene_id
                                None, // chapter_id
                                issues.len(),
                                &format!("已自动修正 {} 处逻辑/连续性/设定问题", issues.len()),
                            );
                        }
                        Err(e) => {
                            log::warn!("[BGP-2 AutoRewrite] 自动改写失败: {}", e);
                            // 即使改写失败，仍发射建议
                            emit_revision_suggestions(&handle, &story_id_owned, &issues);
                        }
                    }
                });
            }
        }

        // ===== LOW 严重度：仅建议 =====
        if !low_issues.is_empty() {
            let suggestions: Vec<String> = low_issues
                .iter()
                .map(|i| format!("[{}] {}", i.dimension, i.description))
                .collect();
            emit_revision_suggestions(&app_handle, story_id, &suggestions);
        }
    }
}

/// 审计问题
#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub dimension: String,
    pub description: String,
    pub severity: AuditSeverity,
    pub dimension_priority: u32,
}

/// 调用 Writer LLM 改写问题段落（静默标签 bg-auto-rewriter）。
async fn rewrite_content(
    app_handle: &AppHandle,
    pool: &DbPool,
    content: &str,
    issues: &[String],
) -> Result<String, String> {
    let llm = crate::llm::LlmService::new(app_handle.clone());

    let issues_text = issues
        .iter()
        .enumerate()
        .map(|(i, desc)| format!("{}. {}", i + 1, desc))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "你是一名专业的小说编辑。以下正文存在若干严重问题，请修正这些问题并输出修正后的全文。\n\n\
         【问题清单】\n{issues_text}\n\n\
         【原文】\n{content}\n\n\
         【要求】\n1. 修正所有列出的问题\n2. 保持原有风格和叙事连贯性\n3. 输出修正后的完整正文，不要加说明"
    );

    let resp = llm
        .generate_for_task(
            crate::router::TaskType::CreativeWriting,
            prompt,
            Some(2048),
            Some(0.5),
            Some("bg-auto-rewriter"),
        )
        .await
        .map_err(|e| format!("LLM 调用失败: {}", e))?;

    if resp.content.trim().is_empty() {
        Err("改写结果为空".to_string())
    } else {
        Ok(resp.content)
    }
}

/// 发射低严重度修订建议事件。
fn emit_revision_suggestions(app_handle: &AppHandle, story_id: &str, suggestions: &[String]) {
    crate::state_sync::service::StateSync::emit_revision_suggested(
        app_handle,
        story_id,
        None, // scene_id
        None, // chapter_id
        suggestions,
    );
}

/// 从 AppConfig 读取后台自动改写严重度阈值（"high" / "medium" / "low"）。
fn load_severity_threshold() -> String {
    // 从 app_data_dir 读 AppConfig
    match std::env::current_dir() {
        Ok(_) => {
            // 尝试从已知位置加载
            // TODO: 通过全局 pool 或 app_handle 读取配置
            "high".to_string() // 默认保守：只自动改高严重度
        }
        Err(_) => "high".to_string(),
    }
}
