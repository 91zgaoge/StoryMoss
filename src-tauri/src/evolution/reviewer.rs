#![allow(dead_code)]
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Deep reviewer for story evolution analysis
pub struct EvolutionReviewer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionReview {
    pub review_id: String,
    pub story_id: String,
    pub created_at: DateTime<Utc>,
    pub overall_assessment: OverallAssessment,
    pub narrative_arc_analysis: NarrativeArcAnalysis,
    pub theme_development: ThemeDevelopment,
    pub reader_engagement_prediction: EngagementPrediction,
    pub recommendations: Vec<Recommendation>,
    pub learning_outcomes: Vec<LearningOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallAssessment {
    pub narrative_coherence_score: f32,
    pub character_development_score: f32,
    pub world_building_consistency_score: f32,
    pub thematic_depth_score: f32,
    pub overall_progress: f32, // Story completion estimate
    pub strengths_summary: Vec<String>,
    pub concerns_summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeArcAnalysis {
    pub current_act: String,
    pub tension_curve: Vec<TensionPoint>,
    pub plot_points_evaluated: Vec<PlotPointEvaluation>,
    pub pacing_assessment: PacingAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensionPoint {
    pub chapter: u32,
    pub tension_level: f32, // 0.0 - 1.0
    pub narrative_moment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotPointEvaluation {
    pub plot_point: String,
    pub chapter: u32,
    pub effectiveness: f32,
    pub setup_quality: f32,
    pub payoff_quality: Option<f32>,
    pub status: PlotPointStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlotPointStatus {
    Setup,
    Developed,
    Resolved,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingAssessment {
    pub overall_pacing: PacingType,
    pub drag_points: Vec<DragPoint>,
    effective_moments: Vec<EffectiveMoment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PacingType {
    TooSlow,
    Slow,
    Balanced,
    Fast,
    TooFast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragPoint {
    pub chapter: u32,
    pub section: String,
    pub reason: String,
    pub suggested_fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveMoment {
    pub chapter: u32,
    pub description: String,
    pub why_it_works: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeDevelopment {
    pub primary_theme: String,
    pub secondary_themes: Vec<String>,
    pub theme_progression: Vec<ThemeProgression>,
    pub symbol_usage: Vec<SymbolUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeProgression {
    pub theme: String,
    pub chapter_introduced: u32,
    pub development_stages: Vec<DevelopmentStage>,
    pub current_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentStage {
    pub chapter: u32,
    pub manifestation: String,
    pub subtlety_level: f32, // 0.0 = explicit, 1.0 = very subtle
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolUsage {
    pub symbol: String,
    pub occurrences: Vec<SymbolOccurrence>,
    pub effectiveness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolOccurrence {
    pub chapter: u32,
    pub context: String,
    pub meaning_conveyed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementPrediction {
    pub predicted_rating: f32, // 1.0 - 5.0
    pub hook_strength: f32,    // First chapter engagement
    pub retention_curve: Vec<RetentionPoint>,
    pub emotional_resonance: EmotionalResonance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPoint {
    pub chapter: u32,
    pub predicted_retention: f32, // % of readers continuing
    pub drop_off_risk: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalResonance {
    pub emotional_beats: Vec<EmotionalBeat>,
    pub emotional_variety_score: f32,
    pub emotional_impact_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalBeat {
    pub chapter: u32,
    pub emotion_type: String,
    pub intensity: f32,
    pub effectiveness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: String,
    pub priority: Priority,
    pub description: String,
    pub expected_impact: String,
    pub implementation_difficulty: Difficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Moderate,
    Hard,
    VeryHard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningOutcome {
    pub pattern_identified: String,
    pub successful_approaches: Vec<String>,
    pub areas_for_improvement: Vec<String>,
    pub recommended_practices: Vec<String>,
}

impl EvolutionReviewer {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_review(
        &self,
        story_id: &str,
        analyses: &[super::analyzer::AnalysisReport],
    ) -> EvolutionReview {
        EvolutionReview {
            review_id: uuid::Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            created_at: Utc::now(),
            overall_assessment: self.assess_overall(analyses),
            narrative_arc_analysis: self.analyze_narrative_arc(analyses),
            theme_development: self.analyze_themes(analyses),
            reader_engagement_prediction: self.predict_engagement(analyses),
            recommendations: self.generate_recommendations(analyses),
            learning_outcomes: self.identify_learning_outcomes(analyses),
        }
    }

    fn assess_overall(&self, analyses: &[super::analyzer::AnalysisReport]) -> OverallAssessment {
        if analyses.is_empty() {
            return OverallAssessment {
                narrative_coherence_score: 0.0,
                character_development_score: 0.0,
                world_building_consistency_score: 0.0,
                thematic_depth_score: 0.0,
                overall_progress: 0.0,
                strengths_summary: vec![],
                concerns_summary: vec!["No analysis data available".to_string()],
            };
        }
        let avg_coherence =
            analyses.iter().map(|a| a.plot_coherence.score).sum::<f32>() / analyses.len() as f32;
        let avg_quality = analyses
            .iter()
            .map(|a| a.writing_quality.score)
            .sum::<f32>()
            / analyses.len() as f32;
        let avg_character = analyses
            .iter()
            .map(|a| a.character_consistency.score)
            .sum::<f32>()
            / analyses.len() as f32;
        let avg_pacing = analyses
            .iter()
            .map(|a| a.pacing_analysis.score)
            .sum::<f32>()
            / analyses.len() as f32;

        let mut strengths = Vec::new();
        let mut concerns = Vec::new();
        if avg_quality > 75.0 {
            strengths.push("写作质量良好".to_string());
        }
        if avg_coherence > 75.0 {
            strengths.push("情节连贯性佳".to_string());
        }
        if avg_character > 75.0 {
            strengths.push("角色一致性高".to_string());
        }
        if avg_pacing > 75.0 {
            strengths.push("节奏把控得当".to_string());
        }
        if avg_quality < 60.0 {
            concerns.push("写作质量有待提升".to_string());
        }
        if avg_coherence < 60.0 {
            concerns.push("情节连贯性需加强".to_string());
        }
        if avg_character < 60.0 {
            concerns.push("角色一致性存在问题".to_string());
        }
        if avg_pacing < 60.0 {
            concerns.push("节奏把控需要调整".to_string());
        }
        if strengths.is_empty() {
            strengths.push("整体表现平稳".to_string());
        }
        if concerns.is_empty() {
            concerns.push("继续保持当前水平".to_string());
        }

        OverallAssessment {
            narrative_coherence_score: avg_coherence / 100.0,
            character_development_score: avg_character / 100.0,
            world_building_consistency_score: avg_quality / 100.0,
            thematic_depth_score: (avg_coherence + avg_quality) / 200.0,
            overall_progress: analyses.iter().map(|a| a.overall_score).sum::<f32>()
                / analyses.len() as f32
                / 100.0,
            strengths_summary: strengths,
            concerns_summary: concerns,
        }
    }

    fn analyze_narrative_arc(
        &self,
        analyses: &[super::analyzer::AnalysisReport],
    ) -> NarrativeArcAnalysis {
        if analyses.is_empty() {
            return NarrativeArcAnalysis {
                current_act: "未知".to_string(),
                tension_curve: vec![],
                plot_points_evaluated: vec![],
                pacing_assessment: PacingAssessment {
                    overall_pacing: PacingType::Balanced,
                    drag_points: vec![],
                    effective_moments: vec![],
                },
            };
        }

        let avg_pacing_score = analyses
            .iter()
            .map(|a| a.pacing_analysis.score)
            .sum::<f32>()
            / analyses.len() as f32;
        let pacing_type = if avg_pacing_score > 80.0 {
            PacingType::Fast
        } else if avg_pacing_score > 60.0 {
            PacingType::Balanced
        } else if avg_pacing_score > 40.0 {
            PacingType::Slow
        } else {
            PacingType::TooSlow
        };

        let mut drag_points = Vec::new();
        let mut effective_moments = Vec::new();
        for analysis in analyses {
            for section in &analysis.pacing_analysis.slow_sections {
                drag_points.push(DragPoint {
                    chapter: section.chapter_number,
                    section: format!("{}%-{}%", section.start_percent, section.end_percent),
                    reason: section.description.clone(),
                    suggested_fix: "考虑删减或增加冲突".to_string(),
                });
            }
            for section in &analysis.pacing_analysis.rushed_sections {
                effective_moments.push(EffectiveMoment {
                    chapter: section.chapter_number,
                    description: section.description.clone(),
                    why_it_works: "节奏紧凑，推进有力".to_string(),
                });
            }
        }

        NarrativeArcAnalysis {
            current_act: if avg_pacing_score > 70.0 {
                "高潮阶段"
            } else {
                "发展阶段"
            }
            .to_string(),
            tension_curve: vec![],
            plot_points_evaluated: vec![],
            pacing_assessment: PacingAssessment {
                overall_pacing: pacing_type,
                drag_points,
                effective_moments,
            },
        }
    }

    fn analyze_themes(&self, analyses: &[super::analyzer::AnalysisReport]) -> ThemeDevelopment {
        // 从建议中推断主题
        let mut theme_keywords: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for analysis in analyses {
            for suggestion in &analysis.suggestions {
                let text = suggestion.description.to_lowercase();
                if text.contains("theme") || text.contains("主题") {
                    *theme_keywords.entry("主题深化".to_string()).or_insert(0) += 1;
                }
                if text.contains("symbol") || text.contains("象征") {
                    *theme_keywords.entry("象征手法".to_string()).or_insert(0) += 1;
                }
                if text.contains("motif") || text.contains(" motif") {
                    *theme_keywords.entry("母题运用".to_string()).or_insert(0) += 1;
                }
            }
        }

        let secondary: Vec<String> = theme_keywords.keys().cloned().collect();
        ThemeDevelopment {
            primary_theme: "待分析".to_string(),
            secondary_themes: if secondary.is_empty() {
                vec!["情节推进".to_string()]
            } else {
                secondary
            },
            theme_progression: vec![],
            symbol_usage: vec![],
        }
    }

    fn predict_engagement(
        &self,
        analyses: &[super::analyzer::AnalysisReport],
    ) -> EngagementPrediction {
        if analyses.is_empty() {
            return EngagementPrediction {
                predicted_rating: 3.0,
                hook_strength: 0.5,
                retention_curve: vec![],
                emotional_resonance: EmotionalResonance {
                    emotional_beats: vec![],
                    emotional_variety_score: 0.5,
                    emotional_impact_score: 0.5,
                },
            };
        }
        let avg_quality = analyses
            .iter()
            .map(|a| a.writing_quality.score)
            .sum::<f32>()
            / analyses.len() as f32;
        let avg_coherence =
            analyses.iter().map(|a| a.plot_coherence.score).sum::<f32>() / analyses.len() as f32;
        let avg_pacing = analyses
            .iter()
            .map(|a| a.pacing_analysis.score)
            .sum::<f32>()
            / analyses.len() as f32;

        // 预测评分基于多维度综合
        let predicted_rating = 2.0 + (avg_quality + avg_coherence + avg_pacing) / 100.0;
        let hook_strength = (avg_quality / 100.0).min(1.0);
        let emotional_variety = (avg_coherence / 100.0).min(1.0);
        let emotional_impact = (avg_pacing / 100.0).min(1.0);

        EngagementPrediction {
            predicted_rating: predicted_rating.min(5.0),
            hook_strength,
            retention_curve: vec![],
            emotional_resonance: EmotionalResonance {
                emotional_beats: vec![],
                emotional_variety_score: emotional_variety,
                emotional_impact_score: emotional_impact,
            },
        }
    }

    fn generate_recommendations(
        &self,
        analyses: &[super::analyzer::AnalysisReport],
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        for analysis in analyses {
            for suggestion in &analysis.suggestions {
                recommendations.push(Recommendation {
                    category: suggestion.category.clone(),
                    priority: match suggestion.priority {
                        super::analyzer::Priority::High => Priority::High,
                        super::analyzer::Priority::Medium => Priority::Medium,
                        super::analyzer::Priority::Low => Priority::Low,
                    },
                    description: suggestion.description.clone(),
                    expected_impact: "Improved reader engagement".to_string(),
                    implementation_difficulty: Difficulty::Moderate,
                });
            }
        }

        recommendations
    }

    fn identify_learning_outcomes(
        &self,
        analyses: &[super::analyzer::AnalysisReport],
    ) -> Vec<LearningOutcome> {
        if analyses.is_empty() {
            return vec![LearningOutcome {
                pattern_identified: "暂无足够数据".to_string(),
                successful_approaches: vec![],
                areas_for_improvement: vec!["需要更多章节进行分析".to_string()],
                recommended_practices: vec!["持续写作以积累数据".to_string()],
            }];
        }

        let avg_quality = analyses
            .iter()
            .map(|a| a.writing_quality.score)
            .sum::<f32>()
            / analyses.len() as f32;
        let avg_coherence =
            analyses.iter().map(|a| a.plot_coherence.score).sum::<f32>() / analyses.len() as f32;

        let mut successful = Vec::new();
        let mut improvements = Vec::new();
        let mut practices = Vec::new();

        if avg_quality > 70.0 {
            successful.push("写作质量保持稳定".to_string());
        } else {
            improvements.push("写作质量有待提升".to_string());
            practices.push("注重场景描写和对话自然度".to_string());
        }
        if avg_coherence > 70.0 {
            successful.push("情节推进逻辑清晰".to_string());
        } else {
            improvements.push("情节连贯性需加强".to_string());
            practices.push("梳理伏笔回收和时间线".to_string());
        }

        vec![LearningOutcome {
            pattern_identified: "基于多章节分析的综合评估".to_string(),
            successful_approaches: if successful.is_empty() {
                vec!["整体基础扎实".to_string()]
            } else {
                successful
            },
            areas_for_improvement: if improvements.is_empty() {
                vec!["细节打磨".to_string()]
            } else {
                improvements
            },
            recommended_practices: if practices.is_empty() {
                vec!["保持当前创作节奏".to_string()]
            } else {
                practices
            },
        }]
    }
}
