use crate::agents::Agent;
use crate::domain::{
    agent_context::AgentContext,
    agent_types::AgentResult,
};
use crate::llm::{GenerateRequest, OpenAiAdapter, LlmAdapter};
use crate::config::LlmConfig;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 情节复杂度分析 Agent
pub struct PlotComplexityAgent {
    config: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysis {
    pub overall_score: f32, // 0-100
    pub subplots_count: i32,
    pub character_interactions: i32,
    pub narrative_layers: Vec<String>,
    pub tension_curve: Vec<TensionPoint>,
    pub complexity_breakdown: ComplexityBreakdown,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensionPoint {
    pub chapter: i32,
    pub tension_level: f32, // 0-10
    pub event: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityBreakdown {
    pub plot_intricacy: f32,    // 0-10
    pub character_depth: f32,   // 0-10
    pub thematic_richness: f32, // 0-10
    pub structural_complexity: f32, // 0-10
    pub narrative_pacing: f32,  // 0-10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubplotAnalysis {
    pub subplot_name: String,
    pub chapters: Vec<i32>,
    pub status: String, // "active", "resolved", "dormant"
    pub importance: String, // "major", "minor"
}

impl PlotComplexityAgent {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    pub fn build_analysis_prompt(
        &self,
        story_title: &str,
        chapters: &[crate::db::Chapter],
        characters: &[crate::db::Character],
    ) -> String {
        let chapter_summaries: String = chapters
            .iter()
            .filter_map(|c| {
                c.content.as_ref().map(|content| {
                    let preview: String = content.chars().take(300).collect();
                    format!("Chapter {}: {}\n{}",
                        c.chapter_number,
                        c.title.clone().unwrap_or_default(),
                        preview
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let character_list: String = characters
            .iter()
            .map(|c| format!("- {}: {}",
                c.name,
                c.background.clone().unwrap_or_default()
            ))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Analyze the plot complexity of the following story:

Story: {}

Chapters:
{}

Characters:
{}

Provide a comprehensive complexity analysis:

1. Overall Complexity Score (0-100): Rate how complex/multi-layered the narrative is
2. Subplot Count: How many distinct story threads are woven together
3. Character Interactions: Complexity of relationships and conflicts
4. Narrative Layers: Types of layers (e.g., "main plot", "romance subplot", "mystery", "character backstory")
5. Tension Curve: Map tension levels across chapters (0-10 scale)
6. Complexity Breakdown: Rate 5 dimensions (0-10 each)
   - Plot Intricacy: How elaborate is the plot structure
   - Character Depth: Psychological complexity of characters
   - Thematic Richness: Depth and variety of themes
   - Structural Complexity: Use of flashbacks, multiple POVs, etc.
   - Narrative Pacing: How well the complexity is distributed
7. Recommendations: Suggestions to improve or balance complexity

Format as JSON:
{{
  "overall_score": 75,
  "subplots_count": 3,
  "character_interactions": 12,
  "narrative_layers": ["main plot", "romance subplot", "political intrigue"],
  "tension_curve": [
    {{"chapter": 1, "tension_level": 3, "event": "Inciting incident"}}
  ],
  "complexity_breakdown": {{
    "plot_intricacy": 8,
    "character_depth": 7,
    "thematic_richness": 6,
    "structural_complexity": 5,
    "narrative_pacing": 7
  }},
  "recommendations": ["Add more foreshadowing", "Deepen subplot B"]
}}

Provide only the JSON output."#,
            story_title, chapter_summaries, character_list
        )
    }

    pub fn parse_analysis(
        &self,
        json_str: &str,
    ) -> Result<ComplexityAnalysis, Box<dyn std::error::Error>> {
        let json_content = if json_str.contains("```json") {
            json_str.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(json_str)
        } else if json_str.contains("```") {
            json_str.split("```").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(json_str)
        } else {
            json_str
        };

        let analysis: ComplexityAnalysis = serde_json::from_str(json_content.trim())?;
        Ok(analysis)
    }

    /// 检测情节漏洞
    fn build_plot_hole_prompt(
        &self,
        chapters: &[crate::db::Chapter],
    ) -> String {
        let content = chapters
            .iter()
            .filter_map(|c| c.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        format!(
            r#"Analyze the following story chapters for plot holes, inconsistencies, and unresolved threads:

{}

Identify:
1. Plot holes - Events that contradict established facts or logic
2. Inconsistencies - Character behavior that contradicts established traits
3. Unresolved threads - Subplots or questions left unanswered
4. Timeline issues - Chronological contradictions

For each issue found, specify:
- Type (plot_hole/inconsistency/unresolved/timeline)
- Location (chapter reference)
- Description of the issue
- Suggested fix

Format as JSON array:
[
  {{
    "type": "plot_hole",
    "chapter": 5,
    "description": "Character knows information they shouldn't have",
    "suggested_fix": "Add scene where they learn this earlier"
  }}
]"#,
            content.chars().take(5000).collect::<String>()
        )
    }
}

#[async_trait]
impl Agent for PlotComplexityAgent {
    fn name(&self) -> &str {
        "plot_analyzer"
    }

    fn description(&self) -> &str {
        "Analyzes plot complexity, tension curves, and detects plot holes"
    }

    async fn execute(
        &self,
        _context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error>> {
        if self.config.api_key.is_empty() {
            return Err("API Key not configured".into());
        }

        let adapter = OpenAiAdapter::new(
            self.config.api_key.clone(),
            self.config.model.clone(),
            self.config.api_base.clone(),
            2000,
            0.3,
        );

        let request = GenerateRequest {
            prompt: input.to_string(),
            max_tokens: Some(2000),
            temperature: Some(0.3),
            ..Default::default()
        };

        let response = adapter.generate(request).await?;

        Ok(AgentResult {
            content: response.content,
            score: None,
            suggestions: vec![],
        })
    }
}
