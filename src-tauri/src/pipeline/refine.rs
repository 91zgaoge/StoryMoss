use super::types::*;
use crate::{
    db::{
        BlueprintRepository, CharacterRepository, DbPool, DraftRepository, DraftStatus,
        RevisionRepository, RevisionType,
    },
    domain::contracts::RuntimeContract,
    llm::LlmService,
    router::TaskType,
    story_system::StorySystemEngine,
};

/// 执行 AI 修稿
///
/// 1. 读取草稿内容、蓝图、出场角色
/// 2. 构建修稿 prompt（含用户自定义指导）
/// 3. 调用 LLM 生成修稿版本
/// 4. 创建 revision 记录
/// 5. 更新 draft 状态为 refined
pub async fn refine_draft(
    story_id: &str,
    draft_id: &str,
    user_prompt: Option<&str>,
    _config: &PipelineConfig,
    pool: &DbPool,
    llm_service: &LlmService,
    callbacks: &dyn PipelineCallbacks,
) -> Result<RefineResult, PipelineError> {
    callbacks.progress("refine", 0.1);

    // 1. 读取草稿
    let draft_repo = DraftRepository::new(pool.clone());
    let draft = draft_repo
        .get_by_id(draft_id)
        .map_err(|e| PipelineError {
            phase: "refine".to_string(),
            message: format!("读取草稿失败: {}", e),
            recoverable: true,
        })?
        .ok_or_else(|| PipelineError {
            phase: "refine".to_string(),
            message: "草稿不存在".to_string(),
            recoverable: true,
        })?;

    callbacks.progress("refine", 0.2);

    // 2. 读取蓝图
    let blueprint_repo = BlueprintRepository::new(pool.clone());
    let blueprint = blueprint_repo
        .get_by_chapter(story_id, draft.chapter_number)
        .map_err(|e| PipelineError {
            phase: "refine".to_string(),
            message: format!("读取蓝图失败: {}", e),
            recoverable: true,
        })?;

    // 3. 读取出场角色
    let character_repo = CharacterRepository::new(pool.clone());
    let characters = character_repo
        .get_by_story(story_id)
        .map_err(|e| PipelineError {
            phase: "refine".to_string(),
            message: format!("读取角色失败: {}", e),
            recoverable: true,
        })?;

    callbacks.progress("refine", 0.3);

    // v0.22.5: 加载运行时合同，作为修稿基准
    let runtime_contract = StorySystemEngine::new(pool.clone())
        .get_runtime_contract(story_id, draft.chapter_number)
        .ok();

    // 4. 构建 prompt
    let prompt = build_refine_prompt(
        &draft.content,
        blueprint.as_ref(),
        &characters,
        None, // writing_style
        user_prompt,
        runtime_contract.as_ref(),
        pool,
    );

    callbacks.log("[修稿] 已构建修稿 prompt");
    callbacks.progress("refine", 0.4);

    // 5. 调用 LLM 进行修稿
    let response = match llm_service
        .generate_for_task(
            TaskType::Editing,
            prompt,
            Some(4096),
            Some(0.7),
            Some("AI修稿润色"),
        )
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            return Err(PipelineError {
                phase: "refine".to_string(),
                message: format!("LLM 修稿调用失败: {}", e),
                recoverable: true,
            });
        }
    };

    let refined_content = response.content.trim().to_string();
    let word_count = refined_content.chars().count() as i32;

    // 计算变更摘要
    let change_summary = if refined_content == draft.content {
        Some("修稿后内容与原稿一致（AI 认为无需修改）".to_string())
    } else {
        let diff_ratio = calculate_diff_ratio(&draft.content, &refined_content);
        Some(format!("修稿完成，内容变动约 {:.1}%", diff_ratio * 100.0))
    };

    callbacks.log("[修稿] AI 修稿完成");
    callbacks.progress("refine", 0.8);

    // 6. 创建 revision 记录
    let revision_repo = RevisionRepository::new(pool.clone());
    let revision_index = revision_repo
        .get_by_draft(draft_id)
        .map(|revs| revs.len() as i32 + 1)
        .unwrap_or(1);

    let revision = revision_repo
        .create(
            story_id,
            draft_id,
            revision_index,
            RevisionType::Refine,
            user_prompt,
            &draft.content,
            &refined_content,
            word_count,
            change_summary.as_deref(),
            None,
            None,
            None,
        )
        .map_err(|e| PipelineError {
            phase: "refine".to_string(),
            message: format!("保存修稿记录失败: {}", e),
            recoverable: false,
        })?;

    // 7. 更新 draft 状态
    draft_repo
        .update_status(draft_id, DraftStatus::Refined)
        .map_err(|e| PipelineError {
            phase: "refine".to_string(),
            message: format!("更新草稿状态失败: {}", e),
            recoverable: false,
        })?;

    callbacks.progress("refine", 1.0);

    Ok(RefineResult {
        revision_id: revision.id,
        original_content: draft.content,
        refined_content,
        word_count,
        change_summary,
    })
}

/// 构建修稿 prompt
fn build_refine_prompt(
    draft_content: &str,
    blueprint: Option<&crate::db::Blueprint>,
    characters: &[crate::db::Character],
    _writing_style: Option<&str>,
    user_prompt: Option<&str>,
    runtime_contract: Option<&RuntimeContract>,
    pool: &DbPool,
) -> String {
    // v0.21.0: 系统提示词从 PromptRegistry 读取（支持用户覆盖）
    let mut vars = std::collections::HashMap::new();
    vars.insert(
        "review_feedback".to_string(),
        user_prompt
            .unwrap_or("请保持原有风格，只做必要的润色和优化。")
            .to_string(),
    );
    vars.insert("draft_content".to_string(), draft_content.to_string());

    let mut prompt = if let Some(pool) = crate::get_pool() {
        let tpl = crate::prompts::registry::resolve_prompt(&pool, "pipeline_refine")
            .unwrap_or_else(|_| default_refine_prompt().to_string());
        crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
    } else {
        let tpl = crate::prompts::registry::resolve_prompt_default("pipeline_refine")
            .unwrap_or_else(|| default_refine_prompt().to_string());
        crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
    };

    // 追加动态上下文（蓝图、角色信息）
    if let Some(bp) = blueprint {
        prompt.push_str("\n## 本章蓝图\n");
        if let Some(role) = &bp.role {
            prompt.push_str(&format!("- 章节角色：{}\n", role));
        }
        if let Some(purpose) = &bp.purpose {
            prompt.push_str(&format!("- 核心目的：{}\n", purpose));
        }
        if let Some(key_events) = &bp.key_events {
            prompt.push_str(&format!("- 关键事件：{}\n", key_events));
        }
        if let Some(chars) = &bp.characters {
            prompt.push_str(&format!("- 出场角色：{}\n", chars));
        }
        if let Some(hook) = &bp.suspense_hook {
            prompt.push_str(&format!("- 悬念钩子：{}\n", hook));
        }
        prompt.push('\n');
    }

    if !characters.is_empty() {
        let names: Vec<String> = characters.iter().map(|c| c.name.clone()).collect();
        prompt.push_str(&format!("## 出场角色\n{}\n", names.join(", ")));
    }

    // v0.22.5: 注入 Story System 合同修稿标准
    if let Some(rc) = runtime_contract {
        let contract_vars = rc.to_constraint_vars();
        let contract_section = if let Ok(tpl) =
            crate::prompts::registry::resolve_prompt(pool, "refine_contract_criteria")
        {
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &contract_vars)
        } else {
            crate::prompts::registry::resolve_prompt_default_with_vars(
                "refine_contract_criteria",
                &contract_vars,
            )
            .unwrap_or_default()
        };
        if !contract_section.trim().is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&contract_section);
        }
    }

    prompt
}

/// 修稿系统提示词内置默认
fn default_refine_prompt() -> &'static str {
    r#"# 修稿专家

你是一位资深小说编辑和文字大师。请对以下章节进行深度润色，提升其文学品质。

## 修稿要求
1. 修正语法错误、错别字、标点问题
2. 优化句式，增强画面感和节奏感
3. 保持角色人设一致性
4. 确保剧情逻辑通顺
5. {{review_feedback}}

## 待修稿内容
```
{{draft_content}}
```

## 输出要求
直接输出修稿后的完整正文，不要解释修改原因。如果内容已经完美，可以原样返回。"#
}

/// 计算两段文本的差异比例（简化版：基于行/字符差异）
fn calculate_diff_ratio(original: &str, modified: &str) -> f64 {
    if original.is_empty() {
        return if modified.is_empty() { 0.0 } else { 1.0 };
    }

    let original_chars: Vec<char> = original.chars().collect();
    let modified_chars: Vec<char> = modified.chars().collect();

    // 使用最长公共子序列（LCS）计算差异
    let m = original_chars.len();
    let n = modified_chars.len();

    if m == 0 || n == 0 {
        return 1.0;
    }

    // 使用一维 DP 优化空间
    let mut prev = vec![0u32; n + 1];
    let mut curr = vec![0u32; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if original_chars[i - 1] == modified_chars[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = std::cmp::max(prev[j], curr[j - 1]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    let lcs_len = prev[n] as f64;
    let max_len = std::cmp::max(m, n) as f64;

    if max_len == 0.0 {
        0.0
    } else {
        1.0 - (lcs_len / max_len)
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_diff_ratio;

    #[test]
    fn test_calculate_diff_ratio_identical() {
        assert_eq!(calculate_diff_ratio("hello world", "hello world"), 0.0);
    }

    #[test]
    fn test_calculate_diff_ratio_completely_different() {
        assert_eq!(calculate_diff_ratio("abc", "xyz"), 1.0);
    }

    #[test]
    fn test_calculate_diff_ratio_partially_similar() {
        let ratio = calculate_diff_ratio("hello world", "hello rust");
        assert!(ratio > 0.0 && ratio < 1.0);
    }
}
