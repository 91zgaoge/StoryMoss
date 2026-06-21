//! Call 1 路由合成器（PromptSynthesizer）——用最快模型识别意图、选资产、
//! 合成连贯提示词。
//!
//! 设计依据：docs/plans/2026-06-21-trishot-pipeline-design.md
//!
//! 核心思路：替代 WriteTimeBundle::to_prompt() 的「笨拼接」——由 LLM 根据用户
//! 指令从资产清单中选择相关项，合成为一个连贯、无冲突的综合提示词。失败时
//! 回退到 bundle.to_prompt()（等价当前 TimeSliced 行为，零回归）。

use tauri::AppHandle;

use super::manifest::AssetManifest;

/// 合成结果
#[derive(Debug, Clone)]
pub struct SynthesisResult {
    /// 识别到的用户意图（续写/改写/新场景/润色/规划等）
    pub intent: String,
    /// LLM 选中的资产 ID 列表
    pub selected_asset_ids: Vec<String>,
    /// 合成后的综合提示词（供 Call 3 Writer 直接使用，或 Call 2 精修）
    pub synthesized_prompt: String,
    /// 是否需要 Call 2
    /// 精修（复合题材/改写选中/多冲突约束/逾期伏笔多/confidence低）
    pub needs_refinement: bool,
    /// 精修重点（needs_refinement=true 时提供，指导 Call 2 聚焦）
    pub refinement_focus: Option<String>,
    /// 合成置信度（0.0-1.0）
    pub confidence: f32,
    /// 是否为回退结果（Call 1 失败/超时/解析失败时 true）
    pub is_fallback: bool,
}

impl SynthesisResult {
    /// 构造回退结果：用 bundle.to_prompt() 作为合成提示词，不精修。
    pub fn fallback(bundle_prompt: String) -> Self {
        Self {
            intent: "unknown".into(),
            selected_asset_ids: vec![],
            synthesized_prompt: bundle_prompt,
            needs_refinement: false,
            refinement_focus: None,
            confidence: 0.0,
            is_fallback: true,
        }
    }
}

/// 路由合成器：用最快模型选资产 + 合成提示词。
pub struct PromptSynthesizer;

impl PromptSynthesizer {
    /// 执行 Call 1 合成。
    ///
    /// - `app_handle`：Tauri 应用句柄（用于获取 LlmService）
    /// - `instruction`：用户原始指令
    /// - `current_content_preview`：当前正文尾部预览（用于改写场景判断）
    /// - `manifest`：资产清单
    /// - `bundle_prompt`：WriteTimeBundle 本地拼接（回退用）
    ///
    /// 返回 SynthesisResult。失败/超时/解析失败均返回回退结果（不返回 Err），
    /// 保证调用方总能拿到可用提示词。
    pub async fn synthesize(
        app_handle: AppHandle,
        instruction: &str,
        current_content_preview: Option<&str>,
        manifest: &AssetManifest,
        bundle_prompt: &str,
    ) -> SynthesisResult {
        let llm = crate::llm::LlmService::new(app_handle);

        // 构建合成 prompt
        let prompt = Self::build_synthesis_prompt(instruction, current_content_preview, manifest);

        // 调最快模型（静默标签 tri-shot-router）
        let response = llm
            .generate_with_fastest(prompt, Some(1024), Some(0.3), Some("tri-shot-router"))
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                log::warn!("[TriShot Synthesizer] Call 1 LLM 失败，回退本地拼接: {}", e);
                return SynthesisResult::fallback(bundle_prompt.to_string());
            }
        };

        // 解析 JSON 响应
        match Self::parse_synthesis_response(&response.content, bundle_prompt) {
            Some(result) => result,
            None => {
                log::warn!(
                    "[TriShot Synthesizer] JSON 解析失败，回退本地拼接。原始响应前200字: {}",
                    response.content.chars().take(200).collect::<String>()
                );
                SynthesisResult::fallback(bundle_prompt.to_string())
            }
        }
    }

    /// 构建合成器系统 prompt。
    fn build_synthesis_prompt(
        instruction: &str,
        current_content_preview: Option<&str>,
        manifest: &AssetManifest,
    ) -> String {
        let manifest_text = manifest.to_compact_text();
        let content_section = current_content_preview
            .map(|c| format!("\n【当前正文尾部预览】\n{}", truncate_preview(c, 600)))
            .unwrap_or_default();

        // 优先用注册表模板，回退硬编码
        let tpl = crate::get_pool()
            .and_then(|p| crate::prompts::registry::resolve_prompt(&p, "trishot_synthesizer").ok())
            .or_else(|| crate::prompts::registry::resolve_prompt_default("trishot_synthesizer"));

        if let Some(tpl) = tpl {
            let mut vars = std::collections::HashMap::new();
            vars.insert("instruction".to_string(), instruction.to_string());
            vars.insert("manifest".to_string(), manifest_text.clone());
            vars.insert("content_preview".to_string(), content_section.clone());
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        } else {
            // 硬编码兜底（与注册表默认内容对齐）
            format!(
                "你是小说创作的提示词合成器。根据用户指令和可用创作资产清单，选择相关资产并合成一个连贯、无冲突的综合创作提示词。\n\n\
                 【用户指令】\n{instruction}\n\
                 {content_section}\n\
                 【可用创作资产清单】\n{manifest_text}\n\n\
                 【任务】\n\
                 1. 识别用户意图（continue/rewrite/new_scene/polish/plan/other）\n\
                 2. 从清单中选择与指令相关的资产（硬约束资产必选，软约束按相关性选）\n\
                 3. 把选中资产合成为一个连贯的中文创作提示词，解决段落间冲突，精炼冗余\n\
                 4. 判断是否需要精修（复合题材/改写/多冲突约束/逾期伏笔多时 needs_refinement=true）\n\n\
                 【输出格式】严格输出 JSON，不要 markdown 代码块：\n\
                 {{\"intent\":\"continue\",\"selected_asset_ids\":[\"redline\",\"characters\"],\
                 \"synthesized_prompt\":\"合成后的完整提示词\",\"needs_refinement\":false,\
                 \"refinement_focus\":null,\"confidence\":0.8}}"
            )
        }
    }

    /// 解析合成器 JSON 响应。容错：剥离 markdown 代码块、字段缺失用默认值。
    fn parse_synthesis_response(raw: &str, bundle_prompt: &str) -> Option<SynthesisResult> {
        let json_str = strip_code_fence(raw);
        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;

        let intent = parsed
            .get("intent")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let selected_asset_ids = parsed
            .get("selected_asset_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let synthesized_prompt = parsed
            .get("synthesized_prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| bundle_prompt.to_string());

        let needs_refinement = parsed
            .get("needs_refinement")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let refinement_focus = parsed
            .get("refinement_focus")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());

        let confidence = parsed
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.5);

        // confidence 过低也触发精修
        let needs_refinement = needs_refinement || confidence < 0.6;

        let is_fallback = synthesized_prompt == *bundle_prompt;

        Some(SynthesisResult {
            intent,
            selected_asset_ids,
            synthesized_prompt,
            needs_refinement,
            refinement_focus,
            confidence,
            is_fallback,
        })
    }
}

/// 剥离 markdown 代码块包裹（```json ... ``` 或 ``` ... ```）。
fn strip_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        // 去掉首行 ```json 或 ```
        let after_first_line = trimmed
            .find('\n')
            .map(|i| &trimmed[i + 1..])
            .unwrap_or(trimmed);
        // 去掉末尾 ```
        if let Some(end) = after_first_line.rfind("```") {
            return after_first_line[..end].trim();
        }
        return after_first_line.trim();
    }
    trimmed
}

/// 截断正文预览。
fn truncate_preview(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let tail: String = chars
            .into_iter()
            .rev()
            .take(max_chars)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("…{}", tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_fence_json() {
        let raw = "```json\n{\"intent\":\"continue\"}\n```";
        assert_eq!(strip_code_fence(raw), "{\"intent\":\"continue\"}");
    }

    #[test]
    fn test_strip_code_fence_plain() {
        let raw = "```\n{\"intent\":\"rewrite\"}\n```";
        assert_eq!(strip_code_fence(raw), "{\"intent\":\"rewrite\"}");
    }

    #[test]
    fn test_strip_code_fence_no_fence() {
        let raw = "{\"intent\":\"plan\"}";
        assert_eq!(strip_code_fence(raw), "{\"intent\":\"plan\"}");
    }

    #[test]
    fn test_parse_synthesis_response_full() {
        let raw = r#"{"intent":"continue","selected_asset_ids":["redline","characters"],"synthesized_prompt":"你是一名小说作者，请根据红线和角色续写","needs_refinement":false,"refinement_focus":null,"confidence":0.85}"#;
        let result = PromptSynthesizer::parse_synthesis_response(raw, "fallback_prompt").unwrap();
        assert_eq!(result.intent, "continue");
        assert_eq!(result.selected_asset_ids, vec!["redline", "characters"]);
        assert!(result.synthesized_prompt.contains("小说作者"));
        assert!(!result.needs_refinement);
        assert!((result.confidence - 0.85).abs() < 0.01);
        assert!(!result.is_fallback);
    }

    #[test]
    fn test_parse_synthesis_response_markdown_wrapped() {
        let raw = "```json\n{\"intent\":\"rewrite\",\"synthesized_prompt\":\"改写提示词\",\"confidence\":0.9}\n```";
        let result = PromptSynthesizer::parse_synthesis_response(raw, "fallback").unwrap();
        assert_eq!(result.intent, "rewrite");
        assert!(result.synthesized_prompt.contains("改写"));
    }

    #[test]
    fn test_parse_synthesis_response_missing_fields() {
        // 字段缺失用默认值
        let raw = r#"{"intent":"new_scene"}"#;
        let result = PromptSynthesizer::parse_synthesis_response(raw, "fallback_prompt").unwrap();
        assert_eq!(result.intent, "new_scene");
        assert!(result.selected_asset_ids.is_empty());
        // synthesized_prompt 缺失 → 回退 bundle_prompt
        assert_eq!(result.synthesized_prompt, "fallback_prompt");
        assert!(result.is_fallback);
        // confidence 默认 0.5 < 0.6 → needs_refinement=true
        assert!(result.needs_refinement);
    }

    #[test]
    fn test_parse_synthesis_response_invalid_json() {
        let raw = "这不是JSON";
        let result = PromptSynthesizer::parse_synthesis_response(raw, "fallback");
        assert!(result.is_none(), "非法 JSON 应返回 None 触发回退");
    }

    #[test]
    fn test_parse_synthesis_response_low_confidence_triggers_refinement() {
        let raw = r#"{"intent":"continue","synthesized_prompt":"提示词","confidence":0.4}"#;
        let result = PromptSynthesizer::parse_synthesis_response(raw, "fallback").unwrap();
        assert!(result.needs_refinement, "confidence<0.6 应触发精修");
    }

    #[test]
    fn test_fallback_result() {
        let result = SynthesisResult::fallback("bundle_prompt".into());
        assert!(result.is_fallback);
        assert!(!result.needs_refinement);
        assert_eq!(result.synthesized_prompt, "bundle_prompt");
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_truncate_preview_short() {
        assert_eq!(truncate_preview("短文本", 600), "短文本");
    }

    #[test]
    fn test_truncate_preview_long() {
        let long = "字".repeat(1000);
        let result = truncate_preview(&long, 600);
        assert!(result.starts_with('…'));
        assert_eq!(result.chars().count(), 601); // … + 600字
    }
}
