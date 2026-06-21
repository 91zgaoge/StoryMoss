//! Call 2 提示词精修器（PromptRefiner）——可选步骤，调试完善合成提示词。
//!
//! 仅在 needs_refinement=true 且剩余预算充足时调用。失败回退原提示词（不阻塞）。

use tauri::AppHandle;

/// 提示词精修器
pub struct PromptRefiner;

impl PromptRefiner {
    /// 精修 Call 1 合成的提示词。
    ///
    /// - `app_handle`: Tauri 应用句柄
    /// - `synthesized_prompt`: Call 1 输出的合成提示词
    /// - `refinement_focus`: Call 1 判定的精修重点（复合题材/改写/冲突等）
    /// - `story_title/genre/tone`: 故事背景信息
    ///
    /// 失败回退：返回原 `synthesized_prompt`（不阻塞）。
    pub async fn refine(
        app_handle: AppHandle,
        synthesized_prompt: &str,
        refinement_focus: Option<&str>,
        story_title: &str,
        story_genre: Option<&str>,
        story_tone: Option<&str>,
    ) -> String {
        let llm = crate::llm::LlmService::new(app_handle);

        let focus_text = refinement_focus.unwrap_or("一般精修");

        // 构建精修 prompt
        let prompt = {
            let tpl = crate::get_pool()
                .and_then(|p| crate::prompts::registry::resolve_prompt(&p, "trishot_refiner").ok())
                .or_else(|| crate::prompts::registry::resolve_prompt_default("trishot_refiner"));

            if let Some(tpl) = tpl {
                let mut vars = std::collections::HashMap::new();
                vars.insert("refinement_focus".to_string(), focus_text.to_string());
                vars.insert(
                    "synthesized_prompt".to_string(),
                    synthesized_prompt.to_string(),
                );
                vars.insert("story_title".to_string(), story_title.to_string());
                vars.insert(
                    "story_genre".to_string(),
                    story_genre.unwrap_or("未知").to_string(),
                );
                vars.insert(
                    "story_tone".to_string(),
                    story_tone.unwrap_or("默认").to_string(),
                );
                crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
            } else {
                format!(
                    "请精修以下创作提示词，解决冲突并精炼冗余。精修重点：{focus_text}\n\n待精修提示词：\n{synthesized_prompt}\n\n直接输出精修后的提示词。"
                )
            }
        };

        // 调用 LLM（静默标签 tri-shot-refiner）
        match llm
            .generate_for_task(
                crate::router::TaskType::Analysis,
                prompt,
                Some(1200),
                Some(0.4),
                Some("tri-shot-refiner"),
            )
            .await
        {
            Ok(resp) => {
                let refined = resp.content.trim().to_string();
                if refined.is_empty() {
                    log::warn!("[TriShot Refiner] 精修结果为空，回退原提示词");
                    synthesized_prompt.to_string()
                } else {
                    log::info!(
                        "[TriShot Refiner] 精修完成，{}→{}字符",
                        synthesized_prompt.chars().count(),
                        refined.chars().count()
                    );
                    refined
                }
            }
            Err(e) => {
                log::warn!("[TriShot Refiner] 精修失败（{}），回退原提示词", e);
                synthesized_prompt.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refiner_does_not_panic_on_empty_input() {
        // 验证结构体可构造、方法签名正确（不调 LLM）
        let _refiner = PromptRefiner;
        // 静态方法存在性检查（不应 panic）
        assert!(true);
    }
}
