use super::types::*;
use crate::{
    db::{
        BlueprintRepository, CharacterRepository, DbPool, DraftRepository, DraftStatus,
        PipelineReviewRepository,
    },
    llm::LlmService,
    router::TaskType,
};

/// 执行 AI 审稿
///
/// 1. 读取 refined 草稿、前文内容、角色状态
/// 2. 构建审稿 prompt（含评审维度）
/// 3. 调用 LLM 生成结构化审稿报告（JSON）
/// 4. 解析 JSON，创建 review 记录
/// 5. 更新 draft 状态为 reviewed
pub async fn review_draft(
    story_id: &str,
    draft_id: &str,
    review_focus: Option<&str>,
    config: &PipelineConfig,
    pool: &DbPool,
    llm_service: &LlmService,
    callbacks: &dyn PipelineCallbacks,
) -> Result<ReviewResult, PipelineError> {
    callbacks.progress("review", 0.1);

    // 1. 读取草稿
    let draft_repo = DraftRepository::new(pool.clone());
    let draft = draft_repo
        .get_by_id(draft_id)
        .map_err(|e| PipelineError {
            phase: "review".to_string(),
            message: format!("读取草稿失败: {}", e),
            recoverable: true,
        })?
        .ok_or_else(|| PipelineError {
            phase: "review".to_string(),
            message: "草稿不存在".to_string(),
            recoverable: true,
        })?;

    // 验证草稿状态
    if draft.status != DraftStatus::Refined && draft.status != DraftStatus::Draft {
        return Err(PipelineError {
            phase: "review".to_string(),
            message: format!("草稿状态为 {:?}，无法审稿。请先执行修稿。", draft.status),
            recoverable: true,
        });
    }

    callbacks.progress("review", 0.2);

    // 2. 读取蓝图和角色
    let blueprint_repo = BlueprintRepository::new(pool.clone());
    let blueprint = blueprint_repo
        .get_by_chapter(story_id, draft.chapter_number)
        .map_err(|e| PipelineError {
            phase: "review".to_string(),
            message: format!("读取蓝图失败: {}", e),
            recoverable: true,
        })?;

    let character_repo = CharacterRepository::new(pool.clone());
    let characters = character_repo
        .get_by_story(story_id)
        .map_err(|e| PipelineError {
            phase: "review".to_string(),
            message: format!("读取角色失败: {}", e),
            recoverable: true,
        })?;

    callbacks.progress("review", 0.3);

    // 3. 构建审稿 prompt
    let prompt = build_review_prompt(
        &draft.content,
        blueprint.as_ref(),
        &characters,
        review_focus,
        config,
    );

    callbacks.log("[审稿] 已构建审稿 prompt");
    callbacks.progress("review", 0.4);

    // 4. 调用 LLM 生成审稿报告
    let review_result = match llm_service
        .generate_for_task(
            TaskType::Analysis,
            prompt,
            Some(4096),
            Some(0.3),
            Some("AI审稿报告"),
        )
        .await
    {
        Ok(resp) => {
            let text = resp.content.trim().to_string();
            match parse_review_json(&text) {
                Ok(result) => {
                    callbacks.log("[审稿] 审稿报告 JSON 解析成功");
                    result
                }
                Err(parse_err) => {
                    callbacks.log(&format!(
                        "[审稿] JSON 解析失败，回退到占位: {}",
                        parse_err.message
                    ));
                    // 回退：生成一个基于文本分析的简化审稿结果
                    generate_fallback_review(&draft.content, &config.review_dimensions, &text)
                }
            }
        }
        Err(e) => {
            callbacks.log(&format!("[审稿] LLM 调用失败: {}", e));
            return Err(PipelineError {
                phase: "review".to_string(),
                message: format!("LLM 审稿调用失败: {}", e),
                recoverable: true,
            });
        }
    };

    callbacks.log("[审稿] AI 审稿完成");
    callbacks.progress("review", 0.8);

    // 5. 创建 review 记录
    let review_repo = PipelineReviewRepository::new(pool.clone());
    let review_index = review_repo
        .get_by_draft(draft_id)
        .map(|revs| revs.len() as i32 + 1)
        .unwrap_or(1);

    let dimensions_str = serde_json::to_string(&review_result.dimensions).unwrap_or_default();
    let issues_str = serde_json::to_string(&review_result.issues).unwrap_or_default();

    let db_dimensions: Vec<crate::db::ReviewDimension> =
        serde_json::from_str(&dimensions_str).unwrap_or_default();
    let db_issues: Vec<crate::db::ReviewIssueItem> =
        serde_json::from_str(&issues_str).unwrap_or_default();

    let review = review_repo
        .create(
            story_id,
            draft_id,
            review_index,
            &review_result.summary,
            Some(&db_dimensions),
            Some(&db_issues),
            Some(review_result.overall_score),
            review_focus,
            None,
            None,
            None,
        )
        .map_err(|e| PipelineError {
            phase: "review".to_string(),
            message: format!("保存审稿报告失败: {}", e),
            recoverable: false,
        })?;

    // 6. 更新 draft 状态
    draft_repo
        .update_status(draft_id, DraftStatus::Reviewed)
        .map_err(|e| PipelineError {
            phase: "review".to_string(),
            message: format!("更新草稿状态失败: {}", e),
            recoverable: false,
        })?;

    callbacks.progress("review", 1.0);

    Ok(ReviewResult {
        review_id: review.id,
        ..review_result
    })
}

/// 构建审稿 prompt
fn build_review_prompt(
    draft_content: &str,
    blueprint: Option<&crate::db::Blueprint>,
    characters: &[crate::db::Character],
    review_focus: Option<&str>,
    config: &PipelineConfig,
) -> String {
    // v0.21.0: 系统提示词骨架从 PromptRegistry 读取（支持用户覆盖）
    let mut prompt = if let Some(pool) = crate::get_pool() {
        crate::prompts::registry::resolve_prompt(&pool, "pipeline_review")
            .unwrap_or_else(|_| default_review_system_prompt().to_string())
    } else {
        default_review_system_prompt().to_string()
    };

    // 构建评审维度描述（动态部分，保留在代码中）
    let mut review_dimensions = String::new();
    for (i, dim) in config.review_dimensions.iter().enumerate() {
        let desc = match dim.as_str() {
            "continuity" => "与前文是否矛盾？伏笔是否呼应？时间线是否一致？",
            "logic" => "因果逻辑是否通顺？角色动机是否合理？是否符合世界观设定？",
            "character" => "角色能力、位置、情感状态是否与前文一致？人设是否崩塌？",
            "foreshadow" => "是否与前后章节形成有机联系？伏笔回收是否自然？",
            "pacing" => "张弛有度？是否存在拖沓或跳跃？",
            "style" => "描写是否生动？对白是否自然？画面感强不强？",
            _ => "",
        };
        review_dimensions.push_str(&format!("{}. **{}**: {}\n", i + 1, dim, desc));
    }

    // 渲染模板变量
    let mut vars = std::collections::HashMap::new();
    vars.insert("review_dimensions".to_string(), review_dimensions);
    vars.insert("draft_content".to_string(), draft_content.to_string());
    prompt = crate::prompts::engine::TemplateEngine::render_with_conditions(&prompt, &vars);

    // 追加动态上下文信息（非模板部分）
    if let Some(focus) = review_focus {
        prompt.push_str(&format!("\n## 审稿侧重点\n{}\n", focus));
    }

    prompt.push_str("\n## 上下文信息\n");
    if let Some(bp) = blueprint {
        if let Some(purpose) = &bp.purpose {
            prompt.push_str(&format!("- 本章目的：{}\n", purpose));
        }
        if let Some(hook) = &bp.suspense_hook {
            prompt.push_str(&format!("- 悬念钩子：{}\n", hook));
        }
    }

    if !characters.is_empty() {
        prompt.push_str("- 出场角色：");
        let names: Vec<String> = characters.iter().map(|c| c.name.clone()).collect();
        prompt.push_str(&names.join(", "));
        prompt.push('\n');
    }

    prompt
}

/// 审稿系统提示词内置默认（registry 不可用时的回退）
fn default_review_system_prompt() -> &'static str {
    r#"# 审稿专家

你是一位挑剔的读者、资深编辑和小说评论家。请对以下章节进行全方位的质量评审。

## 评审维度
请对以下每个维度给出 0-100 的评分和具体评价：
{{review_dimensions}}

## 待审稿内容
```
{{draft_content}}
```

## 输出格式（严格 JSON）
```json
{
  "overall_score": 85,
  "dimensions": [
    {"name": "维度名", "score": 90, "comment": "评价"}
  ],
  "issues": [
    {"severity": "high", "dimension": "维度", "description": "问题描述", "suggestion": "建议"}
  ],
  "summary": "总体评价"
}
```
注意：overall_score 是综合评分（0-100）；issues 数组可以为空；severity 只能是 critical / high / medium / low。"#
}

/// 解析审稿 JSON（容错）
pub fn parse_review_json(text: &str) -> Result<ReviewResult, PipelineError> {
    let mut clean = text.replace("```json", "").replace("```", "");
    clean = clean.trim().to_string();
    let first_brace = clean.find('{');
    let last_brace = clean.rfind('}');
    if let (Some(start), Some(end)) = (first_brace, last_brace) {
        clean = clean[start..=end].to_string();
    }
    serde_json::from_str::<ReviewResult>(&clean).map_err(|e| PipelineError {
        phase: "review".to_string(),
        message: format!("审稿报告 JSON 解析失败: {}", e),
        recoverable: true,
    })
}

/// 当 JSON 解析失败时，生成一个简化的回退审稿结果
fn generate_fallback_review(
    content: &str,
    dimensions: &[String],
    raw_response: &str,
) -> ReviewResult {
    let word_count = content.chars().count();

    // 基于字数和内容特征生成启发式评分
    let base_score = if word_count > 500 {
        80.0
    } else if word_count > 100 {
        70.0
    } else {
        60.0
    };

    let dimension_results: Vec<ReviewDimensionResult> = dimensions
        .iter()
        .map(|dim| {
            let score_f64 = match dim.as_str() {
                "continuity" | "logic" => base_score + 5.0,
                "character" => base_score,
                "foreshadow" => base_score - 2.0,
                "pacing" => base_score + 2.0,
                "style" => base_score + 3.0,
                _ => base_score,
            };
            let score = (score_f64 as f32).clamp(0.0_f32, 100.0_f32);

            ReviewDimensionResult {
                name: dim.clone(),
                score,
                comment: format!("[自动评估] {} 维度基于文本特征评估得分 {:.1}", dim, score),
            }
        })
        .collect();

    let overall_score = if dimension_results.is_empty() {
        base_score as f32
    } else {
        (dimension_results.iter().map(|d| d.score).sum::<f32>() / dimension_results.len() as f32)
            .clamp(0.0_f32, 100.0_f32)
    };

    // 检查 raw_response 中是否包含问题关键词
    let mut issues = Vec::new();
    let lower = raw_response.to_lowercase();
    if lower.contains("矛盾") || lower.contains("不一致") {
        issues.push(ReviewIssueResult {
            severity: "medium".to_string(),
            dimension: "剧情连贯性".to_string(),
            description: "响应中检测到可能的一致性问题".to_string(),
            suggestion: "请检查前后文逻辑".to_string(),
        });
    }
    if lower.contains("角色") && (lower.contains("崩塌") || lower.contains("不合理")) {
        issues.push(ReviewIssueResult {
            severity: "high".to_string(),
            dimension: "角色一致性".to_string(),
            description: "响应中检测到可能的角色一致性问题".to_string(),
            suggestion: "请检查角色人设".to_string(),
        });
    }

    ReviewResult {
        review_id: String::new(),
        overall_score,
        dimensions: dimension_results,
        issues,
        summary: format!(
            "[回退评估] 原始响应未能解析为标准 JSON，已基于文本特征生成简化审稿报告。原始响应前 \
             200 字: {}",
            raw_response.chars().take(200).collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_review_json;

    #[test]
    fn test_parse_review_json_valid() {
        let json = r#"{"review_id":"","overall_score":85.0,"dimensions":[{"name":"continuity","score":90.0,"comment":"good"}],"issues":[],"summary":"ok"}"#;
        let result = parse_review_json(json).unwrap();
        assert_eq!(result.overall_score, 85.0);
        assert_eq!(result.summary, "ok");
        assert_eq!(result.dimensions.len(), 1);
    }

    #[test]
    fn test_parse_review_json_with_extra_text() {
        let text = r#"Here is the review:
```json
{"review_id":"","overall_score":70.0,"dimensions":[],"issues":[],"summary":"meh"}
```"#;
        let result = parse_review_json(text).unwrap();
        assert_eq!(result.overall_score, 70.0);
        assert_eq!(result.summary, "meh");
    }

    #[test]
    fn test_parse_review_json_invalid_fallback() {
        let text = "this is not json { broken";
        assert!(parse_review_json(text).is_err());
    }
}
