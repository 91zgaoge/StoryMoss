#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Content analyzer for story evolution
pub struct ContentAnalyzer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub overall_score: f32,
    pub pacing_analysis: PacingAnalysis,
    pub character_consistency: CharacterConsistency,
    pub plot_coherence: PlotCoherence,
    pub writing_quality: WritingQuality,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingAnalysis {
    pub score: f32,
    pub issues: Vec<String>,
    pub slow_sections: Vec<Section>,
    pub rushed_sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub chapter_number: u32,
    pub start_percent: f32,
    pub end_percent: f32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterConsistency {
    pub score: f32,
    pub inconsistencies: Vec<Inconsistency>,
    pub character_arcs: Vec<CharacterArc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inconsistency {
    pub character_name: String,
    pub trait_name: String,
    pub expected: String,
    pub actual: String,
    pub chapter: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterArc {
    pub character_name: String,
    pub starting_state: String,
    pub current_state: String,
    pub progression: f32, // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotCoherence {
    pub score: f32,
    pub loose_ends: Vec<String>,
    pub foreshadowing_payed_off: Vec<String>,
    plot_holes: Vec<PlotHole>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotHole {
    pub description: String,
    pub chapter_introduced: u32,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingQuality {
    pub score: f32,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub readability_score: f32,
    pub vocabulary_diversity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub category: String,
    pub priority: Priority,
    pub description: String,
    pub example_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl ContentAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_chapter(
        &self,
        content: &str,
        chapter_number: u32,
        characters: &[CharacterPresence],
    ) -> ChapterAnalysis {
        ChapterAnalysis {
            chapter_number,
            word_count: content.split_whitespace().count() as u32,
            sentence_count: content.split('.').count() as u32,
            avg_sentence_length: self.calculate_avg_sentence_length(content),
            dialogue_ratio: self.calculate_dialogue_ratio(content),
            characters_present: characters.to_vec(),
            emotional_tone: self.analyze_emotional_tone(content),
        }
    }

    fn calculate_avg_sentence_length(&self, content: &str) -> f32 {
        let sentences: Vec<&str> = content
            .split('.')
            .filter(|s| !s.trim().is_empty())
            .collect();
        if sentences.is_empty() {
            return 0.0;
        }
        let total_words: usize = sentences.iter().map(|s| s.split_whitespace().count()).sum();
        total_words as f32 / sentences.len() as f32
    }

    fn calculate_dialogue_ratio(&self, content: &str) -> f32 {
        let dialogue_markers = [34 as char, 34 as char, 39 as char, 39 as char];
        let dialogue_chars: usize = content
            .chars()
            .filter(|c| dialogue_markers.contains(c))
            .count();
        dialogue_chars as f32 / content.len() as f32
    }

    fn analyze_emotional_tone(&self, content: &str) -> String {
        // Simple emotional tone analysis based on keyword matching
        let positive_words = [
            "happy",
            "joy",
            "love",
            "excited",
            "wonderful",
            "beautiful",
            "success",
        ];
        let negative_words = [
            "sad", "anger", "hate", "fear", "terrible", "horrible", "failure",
        ];

        let content_lower = content.to_lowercase();
        let positive_count: usize = positive_words
            .iter()
            .map(|word| content_lower.matches(word).count())
            .sum();
        let negative_count: usize = negative_words
            .iter()
            .map(|word| content_lower.matches(word).count())
            .sum();

        if positive_count > negative_count {
            "positive".to_string()
        } else if negative_count > positive_count {
            "negative".to_string()
        } else {
            "neutral".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterAnalysis {
    pub chapter_number: u32,
    pub word_count: u32,
    pub sentence_count: u32,
    pub avg_sentence_length: f32,
    pub dialogue_ratio: f32,
    pub characters_present: Vec<CharacterPresence>,
    pub emotional_tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterPresence {
    pub character_id: String,
    pub character_name: String,
    pub presence_score: f32,
}
