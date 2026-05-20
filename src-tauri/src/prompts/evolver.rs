//! PromptEvolver - 提示词进化器
//!
//! 核心理念：不是用模板变量替换，而是让LLM根据故事上下文自由改写整个prompt。
//! 这样prompt能够贴合当前故事的题材、风格、叙事阶段，真正实现"越写越懂"。

use serde::{Deserialize, Serialize};
use crate::llm::LlmService;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvolution {
    pub original_prompt: String,
    pub evolved_prompt: String,
    pub evolution_reason: String,
    pub story_archetype: String,
    pub narrative_phase: String,
}

pub struct PromptEvolver {
    llm_service: LlmService,
}

impl PromptEvolver {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }

    /// 进化一个prompt
    ///
    /// 输入：原始prompt + 故事上下文
    /// 输出：进化后的prompt（LLM自由改写，不是模板替换）
    pub async fn evolve_prompt(
        &self,
        original_prompt: &str,
        story_context: &EvolutionContext,
    ) -> Result<PromptEvolution, AppError> {
        let evolution_prompt = self.build_evolution_prompt(original_prompt, story_context);

        let response = self.llm_service.generate(evolution_prompt, Some(1500), Some(0.5)).await?;

        let evolved = self.parse_evolved_prompt(&response.content, original_prompt);

        Ok(PromptEvolution {
            original_prompt: original_prompt.to_string(),
            evolved_prompt: evolved.clone(),
            evolution_reason: format!(
                "Adapted for {} story in {} phase",
                story_context.story_archetype, story_context.narrative_phase
            ),
            story_archetype: story_context.story_archetype.clone(),
            narrative_phase: story_context.narrative_phase.clone(),
        })
    }

    /// 为Agent的system prompt进化
    pub async fn evolve_agent_prompt(
        &self,
        agent_name: &str,
        original_system_prompt: &str,
        story_context: &EvolutionContext,
    ) -> Result<String, AppError> {
        let prompt = format!(
            r#"You are a prompt evolution specialist for a creative writing AI.

Your task: Rewrite the following agent system prompt so it perfectly suits the current story context.
Do NOT just insert variables. Completely rewrite the prompt to embody the story's genre, tone, and world.

Agent: {}
Current story: {} ({})
Narrative phase: {}
User preferences: {}

Original system prompt:
---
{}
---

Rewritten system prompt (maintain the same structural elements but adapt the voice, examples, and emphasis to match the story context):"#,
            agent_name,
            story_context.story_title,
            story_context.story_archetype,
            story_context.narrative_phase,
            story_context.user_preferences_summary,
            original_system_prompt
        );

        let response = self.llm_service.generate(prompt, Some(1500), Some(0.5)).await?;
        Ok(response.content.trim().to_string())
    }

    /// 为Skill的prompt进化
    pub async fn evolve_skill_prompt(
        &self,
        skill_name: &str,
        original_system_prompt: &str,
        original_user_template: &str,
        story_context: &EvolutionContext,
    ) -> Result<(String, String), AppError> {
        let prompt = format!(
            r#"You are a prompt evolution specialist.

Rewrite the following skill prompts to perfectly suit the current story context.
The skill should feel like it was designed specifically for this type of story.

Skill: {}
Current story: {} ({})
Narrative phase: {}

Original system prompt:
---
{}
---

Original user prompt template:
---
{}
---

Respond with JSON:
{{
  "system_prompt": "rewritten system prompt",
  "user_prompt_template": "rewritten user prompt template"
}}"#,
            skill_name,
            story_context.story_title,
            story_context.story_archetype,
            story_context.narrative_phase,
            original_system_prompt,
            original_user_template
        );

        let response = self.llm_service.generate(prompt, Some(1500), Some(0.5)).await?;

        // Try JSON parse
        let cleaned = response.content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(cleaned) {
            let sys = json.get("system_prompt").and_then(|v| v.as_str()).unwrap_or(original_system_prompt);
            let usr = json.get("user_prompt_template").and_then(|v| v.as_str()).unwrap_or(original_user_template);
            return Ok((sys.to_string(), usr.to_string()));
        }

        // Fallback: return original
        Ok((original_system_prompt.to_string(), original_user_template.to_string()))
    }

    fn build_evolution_prompt(&self, original: &str, context: &EvolutionContext) -> String {
        format!(
            r#"Rewrite the following writing prompt to perfectly match this story context.

Story: {} (Genre: {})
Narrative phase: {}
World rules: {}
Style: {}
User preferences: {}

Original prompt:
---
{}
---

Rewritten prompt (same structure but adapted voice, examples, and emphasis):"#,
            context.story_title,
            context.story_archetype,
            context.narrative_phase,
            context.world_rules_summary,
            context.style_description,
            context.user_preferences_summary,
            original
        )
    }

    fn parse_evolved_prompt(&self, content: &str, fallback: &str) -> String {
        let cleaned = content.trim().trim_start_matches("```").trim_end_matches("```").trim();
        if cleaned.is_empty() {
            fallback.to_string()
        } else {
            cleaned.to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvolutionContext {
    pub story_title: String,
    pub story_archetype: String, // e.g., "wuxia", "sci-fi", "romance"
    pub narrative_phase: String, // e.g., "setup", "confrontation", "resolution"
    pub world_rules_summary: String,
    pub style_description: String,
    pub user_preferences_summary: String,
}
