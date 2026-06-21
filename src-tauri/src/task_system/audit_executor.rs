//! AuditExecutor - 分时架构时间线 2 的异步审计执行器
//!
//! 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md 模块 6/7
//!
//! 职责：在正文生成返回后，后台异步跑 Inspector 7 维审计，
//! 把发现的问题转化为 inline annotation 回流给用户。
//!
//! Phase 0 实证指导：
//! - memory 维度是最大波动源（S1 差 11 分），issue 优先级 memory > continuity >
//!   logic
//! - 段落级 annotation 为主方案（不要求 char 级精确定位）

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::{
    executor::{TaskExecutionContext, TaskExecutor},
    models::{Task, TaskResult, TaskType},
};
use crate::{
    audit::AuditService,
    db::{DbPool, TextAnnotationRepository},
    llm::LlmService,
    router::TaskType as RoutingTaskType,
    state_sync::events::SyncEvent,
};

/// 审计任务 payload（从 task.payload_json 反序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPayload {
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub chapter_number: i32,
    /// 被审计的正文内容
    pub content: String,
    /// 故事标题（用于 prompt 上下文）
    #[serde(default)]
    pub story_title: Option<String>,
    /// 题材（用于 prompt 上下文）
    #[serde(default)]
    pub genre: Option<String>,
}

/// 异步审计执行器
pub struct AuditExecutor {
    pub pool: DbPool,
    pub app_handle: AppHandle,
}

#[async_trait]
impl TaskExecutor for AuditExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::AsyncAudit
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());
        ctx.start()?;

        let payload: AuditPayload = match task.payload.as_ref() {
            Some(json) => match serde_json::from_str(json) {
                Ok(p) => p,
                Err(e) => {
                    let msg = format!("[AuditExecutor] payload 解析失败: {}", e);
                    ctx.fail(&msg)?;
                    return Ok(task_err(msg));
                }
            },
            None => {
                let msg = "[AuditExecutor] 缺少 payload".to_string();
                ctx.fail(&msg)?;
                return Ok(task_err(msg));
            }
        };

        ctx.update_progress("inspect", 20, "Inspector 正在审计...");
        match self.run_audit_inner(&payload).await {
            Ok(count) => {
                let summary = serde_json::json!({ "annotations_created": count });
                ctx.update_progress("done", 100, &format!("审计完成，{} 条标注", count));
                ctx.complete(Some(summary.to_string()))?;
                Ok(task_ok(Some(summary.to_string())))
            }
            Err(e) => {
                ctx.fail(&e)?;
                Ok(task_err(e))
            }
        }
    }
}

impl AuditExecutor {
    /// 直接执行审计（不经过 task_system 的 Task 调度）。
    /// 供 orchestrator 在 execute_time_sliced 里 spawn 调用。
    pub async fn run_audit(&self, payload: AuditPayload) {
        log::info!(
            "[AuditExecutor] run_audit: story={}, scene={:?}, content_len={}",
            payload.story_id,
            payload.scene_id,
            payload.content.chars().count()
        );
        let result = self.run_audit_inner(&payload).await;
        match result {
            Ok(count) => log::info!("[AuditExecutor] 审计完成，创建 {} 条标注", count),
            Err(e) => log::warn!("[AuditExecutor] 审计失败（静默，不影响用户）: {}", e),
        }
    }

    /// 审计核心逻辑。返回创建的 annotation 数量。
    async fn run_audit_inner(&self, payload: &AuditPayload) -> Result<u32, String> {
        let prompt = build_inspector_prompt(payload);

        let llm = LlmService::new(self.app_handle.clone());
        let response = llm
            .generate_for_task(
                RoutingTaskType::Analysis,
                prompt,
                Some(1500),
                Some(0.2),
                Some("async-audit-inspector"),
            )
            .await
            .map_err(|e| format!("Inspector 调用失败: {}", e))?;

        let mut issues = parse_inspector_issues(&response.content);

        // v0.17.1: 新增 opening_clarity 维度，调用本地启发式门控
        if let Some(ref scene_id) = payload.scene_id {
            let audit_service = AuditService::new(self.pool.clone());
            match audit_service.check_opening_clarity(scene_id, payload.genre.as_deref()) {
                Ok(report) => {
                    let severity = if !report.passed && report.score < 60.0 {
                        "high"
                    } else if !report.passed {
                        "medium"
                    } else {
                        ""
                    };
                    if !severity.is_empty() {
                        issues.push(InspectorIssue {
                            dimension: "opening_clarity".to_string(),
                            severity: severity.to_string(),
                            description: report.rationale.clone(),
                            suggestion: "在开篇 200 字内加入危险/羞辱/失去/谜题钩子并锚定具体场景"
                                .to_string(),
                            paragraph_index: Some(0),
                            score: Some(report.score as i32),
                        });
                    }
                }
                Err(e) => {
                    log::warn!("[AuditExecutor] opening_clarity 检查失败: {}", e);
                }
            }
        }

        if issues.is_empty() {
            return Ok(0);
        }

        // Phase 0 实证：memory 维度优先
        issues.sort_by(|a, b| {
            dimension_priority(&b.dimension).cmp(&dimension_priority(&a.dimension))
        });

        let repo = TextAnnotationRepository::new(self.pool.clone());
        let mut created = 0u32;
        for issue in &issues {
            let metadata = serde_json::json!({
                "dimension": issue.dimension,
                "score": issue.score,
                "suggestion": issue.suggestion,
                "paragraph_index": issue.paragraph_index,
            });
            let content = format!(
                "【{}】{}",
                dimension_label(&issue.dimension),
                issue.description
            );
            if let Ok(ann) = repo.create_annotation_with_meta(
                &payload.story_id,
                payload.scene_id.as_deref(),
                payload.chapter_id.as_deref(),
                &content,
                "ai_audit",
                issue.paragraph_index.unwrap_or(0),
                issue.paragraph_index.unwrap_or(0),
                Some(&metadata.to_string()),
                &issue.severity,
            ) {
                created += 1;
                let _ = self.app_handle.emit(
                    "sync-event",
                    SyncEvent::AnnotationCreated {
                        story_id: payload.story_id.clone(),
                        scene_id: payload.scene_id.clone().unwrap_or_default(),
                        annotation_id: ann.id,
                    },
                );
            }
        }

        // P1-4: 若发现 high 严重性问题，发射 AuditRewriteSuggested 事件，
        // 让前端提示用户是否自动修订。保持用户控制权——不静默改文。
        let high_issues: Vec<String> = issues
            .iter()
            .filter(|i| i.severity.to_lowercase() == "high")
            .map(|i| format!("【{}】{}", dimension_label(&i.dimension), i.description))
            .collect();
        if !high_issues.is_empty() {
            let _ = self.app_handle.emit(
                "sync-event",
                SyncEvent::AuditRewriteSuggested {
                    story_id: payload.story_id.clone(),
                    scene_id: payload.scene_id.clone(),
                    chapter_id: payload.chapter_id.clone(),
                    issues: high_issues.clone(),
                },
            );

            // v0.23 BGP-2 链式 spawn：自动改写（分严重度）
            let auto_content = payload.content.clone();
            let auto_story_id = payload.story_id.clone();
            let auto_handle = self.app_handle.clone();
            let auto_pool = self.pool.clone();
            let auto_chapter = payload.chapter_number;
            tokio::spawn(async move {
                // 构造 high/low issues 列表
                let high_audit_issues: Vec<_> = issues
                    .iter()
                    .filter(|i| i.severity.to_lowercase() == "high")
                    .map(|i| crate::task_system::auto_rewrite_executor::AuditIssue {
                        dimension: dimension_label(&i.dimension).to_string(),
                        description: i.description.clone(),
                        severity: crate::task_system::auto_rewrite_executor::AuditSeverity::High,
                        dimension_priority: 4,
                    })
                    .collect();
                let low_audit_issues: Vec<_> = issues
                    .iter()
                    .filter(|i| i.severity.to_lowercase() != "high")
                    .map(|i| crate::task_system::auto_rewrite_executor::AuditIssue {
                        dimension: dimension_label(&i.dimension).to_string(),
                        description: i.description.clone(),
                        severity: crate::task_system::auto_rewrite_executor::AuditSeverity::Low,
                        dimension_priority: 1,
                    })
                    .collect();

                crate::task_system::auto_rewrite_executor::AutoRewriteExecutor::process_audit_results(
                    auto_handle,
                    auto_pool,
                    &high_audit_issues,
                    &low_audit_issues,
                    &auto_content,
                    &auto_story_id,
                    Some(auto_chapter),
                )
                .await;
            });
        }

        Ok(created)
    }
}

// ==================== Inspector prompt ====================

fn build_inspector_prompt(payload: &AuditPayload) -> String {
    let title = payload.story_title.as_deref().unwrap_or("未命名作品");
    let genre = payload.genre.as_deref().unwrap_or("未知");

    // v0.21.0: 优先从 PromptRegistry 读取覆盖
    if let Some(tpl) = crate::get_pool()
        .and_then(|p| crate::prompts::registry::resolve_prompt(&p, "audit_quality_inspector").ok())
        .or_else(|| crate::prompts::registry::resolve_prompt_default("audit_quality_inspector"))
    {
        let content = payload.content.chars().take(8000).collect::<String>();
        let mut vars = std::collections::HashMap::new();
        vars.insert("content".to_string(), content);
        return crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars);
    }

    format!(
        r#"你是一名严苛的专业小说编辑。请对以下正文片段进行 11 维度质量审计。
这是异步审计，结果将以 inline 标注形式呈现给作者，请聚焦"可操作的问题"，避免泛泛而谈。

=== 作品信息 ===
作品：{title} | 题材：{genre}

=== 审计维度（每维满分 20）===
1. logic 逻辑连贯：情节是否通顺，因果是否清晰，有无时空矛盾
2. character 人物深度：角色是否有内心活动与动机层次，是否脸谱化
3. continuity 连续性：与设定/前文是否吻合，角色状态是否一致
4. foreshadow 伏笔：是否与前后形成有机联系
5. pacing 节奏：快慢是否得当，有无冗余
6. style 风格：描写是否生动，对白是否自然
7. memory 设定遵守：是否违背世界观/角色当前状态/物理逻辑
8. desire 人物欲望：主角是否有清晰、可视化的目标驱动行动（v0.17.1 新增）
9. payoff 情感回报：读者期待的情绪兑现（怕/燃/爽/虐）是否兑现到位（v0.17.1 新增）
10. aftertaste 余韵：结尾是否留有悬念、未尽之意或情感钩子（v0.17.1 新增）
11. opening_clarity 开篇清晰：前 200 字是否同时给出 危险/羞辱/失去/谜题 之一与物理锚点（v0.17.1 新增）

=== 待审计正文 ===
{content}

=== 输出格式（严格 JSON，不要 markdown 标记）===
{{
  "issues": [
    {{
      "dimension": "memory",
      "severity": "high",
      "description": "具体问题描述（一句话，指明哪里错了）",
      "suggestion": "修改建议（一句话）",
      "paragraph_index": 0,
      "score": 10
    }}
  ]
}}

要求：
- 只报告 severity 为 medium 或 high 的问题（low 的不必报告，避免噪音）
- paragraph_index 是问题所在段落的序号（从 0 开始，按空行分段）。若无法定位，填 0
- score 是该维度的自评（0-20），低于 14 才报告
- 如果没有需要报告的问题，返回 {{"issues": []}}
- dimension 必须是上述 11 个之一"#,
        title = title,
        genre = genre,
        content = truncate_content(&payload.content, 4000),
    )
}

fn truncate_content(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}...（已截断，仅审计最后部分）",
            chars.iter().take(max_chars).collect::<String>()
        )
    }
}

// ==================== Inspector 结果解析 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InspectorIssue {
    dimension: String,
    severity: String,
    description: String,
    #[serde(default)]
    suggestion: String,
    #[serde(default)]
    paragraph_index: Option<i32>,
    #[serde(default)]
    score: Option<i32>,
}

fn parse_inspector_issues(content: &str) -> Vec<InspectorIssue> {
    // 去 markdown 包裹
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // 找 JSON 范围
    let start = match cleaned.find('{') {
        Some(s) => s,
        None => {
            log::warn!(
                "[AuditExecutor] Inspector 响应无 JSON: {}",
                &cleaned[..cleaned.len().min(100)]
            );
            return vec![];
        }
    };
    let end = match cleaned.rfind('}') {
        Some(e) => e + 1,
        None => return vec![],
    };

    #[derive(Deserialize)]
    struct IssuesWrapper {
        issues: Vec<InspectorIssue>,
    }

    match serde_json::from_str::<IssuesWrapper>(&cleaned[start..end]) {
        Ok(w) => w
            .issues
            .into_iter()
            .filter(|i| {
                let sev = i.severity.to_lowercase();
                sev == "medium" || sev == "high"
            })
            .collect(),
        Err(e) => {
            log::warn!(
                "[AuditExecutor] JSON 解析失败: {} | 原文前200字: {}",
                e,
                &cleaned[..cleaned.len().min(200)]
            );
            vec![]
        }
    }
}

/// Phase 0 实证：memory 维度是最大波动源，优先级最高。
/// v0.17.1：新增 desire/payoff/aftertaste/opening_clarity 四维。
fn dimension_priority(dim: &str) -> u8 {
    match dim.to_lowercase().as_str() {
        "memory" => 5,
        "continuity" => 4,
        "logic" => 3,
        "payoff" => 3,
        "opening_clarity" => 3,
        "desire" => 2,
        "character" => 2,
        "foreshadow" => 2,
        "aftertaste" => 1,
        "pacing" => 1,
        "style" => 1,
        _ => 0,
    }
}

fn dimension_label(dim: &str) -> &str {
    match dim.to_lowercase().as_str() {
        "memory" => "设定遵守",
        "continuity" => "连续性",
        "logic" => "逻辑连贯",
        "character" => "人物深度",
        "foreshadow" => "伏笔",
        "pacing" => "节奏",
        "style" => "风格",
        "desire" => "欲望驱动",
        "payoff" => "情感回报",
        "aftertaste" => "余韵",
        "opening_clarity" => "开篇清晰",
        _ => "其他",
    }
}

fn task_ok(result_json: Option<String>) -> TaskResult {
    TaskResult {
        success: true,
        result_json,
        error_message: None,
    }
}

fn task_err(error_message: impl Into<String>) -> TaskResult {
    TaskResult {
        success: false,
        result_json: None,
        error_message: Some(error_message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_issues() {
        let result = parse_inspector_issues(r#"{"issues": []}"#);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_with_markdown_wrapper() {
        let raw = r#"```json
{"issues": [{"dimension": "memory", "severity": "high", "description": "角色受伤却奔跑", "suggestion": "改为跛行", "paragraph_index": 2, "score": 8}]}
```"#;
        let result = parse_inspector_issues(raw);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].dimension, "memory");
        assert_eq!(result[0].paragraph_index, Some(2));
    }

    #[test]
    fn parse_filters_low_severity() {
        let raw = r#"{"issues": [
            {"dimension": "style", "severity": "low", "description": "小问题", "suggestion": "", "score": 16},
            {"dimension": "logic", "severity": "medium", "description": "中问题", "suggestion": "", "score": 12}
        ]}"#;
        let result = parse_inspector_issues(raw);
        assert_eq!(result.len(), 1, "low severity 应被过滤");
        assert_eq!(result[0].dimension, "logic");
    }

    #[test]
    fn parse_invalid_json_returns_empty() {
        let result = parse_inspector_issues("这不是JSON");
        assert!(result.is_empty());
    }

    #[test]
    fn dimension_priority_memory_highest() {
        assert!(dimension_priority("memory") > dimension_priority("continuity"));
        assert!(dimension_priority("continuity") > dimension_priority("logic"));
        assert!(dimension_priority("logic") > dimension_priority("style"));
    }

    #[test]
    fn dimension_labels_correct() {
        assert_eq!(dimension_label("memory"), "设定遵守");
        assert_eq!(dimension_label("continuity"), "连续性");
        assert_eq!(dimension_label("logic"), "逻辑连贯");
    }

    /// v0.17.1: 新增 4 维度的标签与优先级
    #[test]
    fn new_dimensions_v17_labels() {
        assert_eq!(dimension_label("desire"), "欲望驱动");
        assert_eq!(dimension_label("payoff"), "情感回报");
        assert_eq!(dimension_label("aftertaste"), "余韵");
        assert_eq!(dimension_label("opening_clarity"), "开篇清晰");
    }

    #[test]
    fn new_dimensions_v17_priority_within_existing_range() {
        // 新维度应介于既有 high/medium/low 之间，不破坏 memory > continuity > others
        // 的总序
        assert!(dimension_priority("memory") > dimension_priority("payoff"));
        assert!(dimension_priority("payoff") >= dimension_priority("character"));
        assert!(dimension_priority("aftertaste") >= dimension_priority("style"));
        assert!(dimension_priority("opening_clarity") > dimension_priority("style"));
    }

    #[test]
    fn truncate_long_content() {
        let long = "一二三四五六七八九十一二三四五六七八九十".repeat(500);
        let t = truncate_content(&long, 100);
        assert!(t.contains("已截断"));
    }

    #[test]
    fn truncate_short_content_unchanged() {
        let short = "短文本";
        assert_eq!(truncate_content(short, 100), "短文本");
    }

    #[test]
    fn sort_issues_by_priority() {
        let mut issues = vec![
            InspectorIssue {
                dimension: "style".to_string(),
                severity: "medium".to_string(),
                description: "s".to_string(),
                suggestion: String::new(),
                paragraph_index: None,
                score: Some(10),
            },
            InspectorIssue {
                dimension: "memory".to_string(),
                severity: "high".to_string(),
                description: "m".to_string(),
                suggestion: String::new(),
                paragraph_index: None,
                score: Some(5),
            },
        ];
        issues.sort_by(|a, b| {
            dimension_priority(&b.dimension).cmp(&dimension_priority(&a.dimension))
        });
        assert_eq!(issues[0].dimension, "memory");
        assert_eq!(issues[1].dimension, "style");
    }
}
