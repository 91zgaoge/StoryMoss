#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub system_prompt: String,
    pub user_prompt_template: String,
}

impl PromptTemplate {
    pub fn render(&self, variables: &[(String, String)]) -> String {
        let mut result = self.user_prompt_template.clone();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
}

pub struct PromptLibrary;

impl PromptLibrary {
    pub fn chapter_generation() -> PromptTemplate {
        PromptTemplate {
            name: "chapter_generation".to_string(),
            system_prompt: r#"You are a professional creative writing assistant specializing in Chinese fiction.
Your task is to write engaging, well-structured story chapters based on the provided outline.

Guidelines:
1. Write in Chinese (简体中文)
2. Maintain consistent character voices and personalities
3. Show, don't tell - use vivid descriptions and dialogue
4. Create atmosphere appropriate to the genre
5. End with a hook that makes readers want to continue"#.to_string(),
            user_prompt_template: r#"Please write Chapter {chapter_number} based on the following outline:

Outline:
{outline}

Story Context:
- Genre: {genre}
- Tone: {tone}
- Pacing: {pacing}

Requirements:
- Word count: approximately 1500-2000 Chinese characters
- Include both narrative and dialogue
- Advance the plot while developing characters

Write the chapter now:"#.to_string(),
        }
    }

    pub fn character_analysis() -> PromptTemplate {
        PromptTemplate {
            name: "character_analysis".to_string(),
            system_prompt: "You are a character development expert. Analyze character consistency \
                            and suggest trait updates based on their actions in the story."
                .to_string(),
            user_prompt_template: r#"Analyze the following character's behavior in this chapter:

Character: {character_name}
Background: {character_background}
Current Traits: {current_traits}

Chapter Content:
{chapter_content}

Please:
1. Identify any new personality traits revealed
2. Note any contradictions with established character
3. Suggest dynamic trait updates with confidence scores (0.0-1.0)

Respond in JSON format with an array of traits:"#
                .to_string(),
        }
    }

    pub fn plot_consistency_check() -> PromptTemplate {
        PromptTemplate {
            name: "plot_consistency".to_string(),
            system_prompt: "You are a story editor specializing in continuity and plot \
                            consistency."
                .to_string(),
            user_prompt_template: r#"Check this chapter for plot consistency:

New Chapter:
{chapter_content}

Previous Context:
{previous_chapters}

Story Bible:
{story_bible}

Identify any:
1. Timeline inconsistencies
2. Contradictions with previous events
3. Character behavior that conflicts with established traits
4. Unexplained plot developments"#
                .to_string(),
        }
    }
}
