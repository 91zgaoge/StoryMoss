#![allow(dead_code)]
use std::collections::HashMap;

use super::model::*;

/// Dynamic model router for selecting optimal LLM based on task
pub struct ModelRouter {
    configs: HashMap<String, ModelConfig>,
    default_model: String,
}

#[derive(Debug, Clone)]
pub enum TaskType {
    CreativeWriting, // Needs high quality
    Editing,         // Needs precision
    Analysis,        // Needs reasoning
    Dialogue,        // Needs character voice
    Summarization,   // Can use faster model
    Brainstorming,   // Can use cheaper model
    Proofreading,    // Needs accuracy
    WorldBuilding,   // Needs creativity + consistency
}

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub model_id: String,
    pub reason: String,
    pub estimated_cost: f64,
    pub estimated_time_ms: u64,
}

impl ModelRouter {
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // Register default models
        let gpt4 = ModelConfig::gpt4();
        configs.insert(gpt4.id.clone(), gpt4);

        let gpt4_turbo = ModelConfig::gpt4_turbo();
        configs.insert(gpt4_turbo.id.clone(), gpt4_turbo);

        Self {
            configs,
            default_model: "gpt-4-turbo".to_string(),
        }
    }

    pub fn register_model(&mut self, config: ModelConfig) {
        self.configs.insert(config.id.clone(), config);
    }

    pub fn route(
        &self,
        task: TaskType,
        complexity: Complexity,
        budget_priority: Priority,
        speed_priority: Priority,
    ) -> RoutingDecision {
        let suitable_models: Vec<&ModelConfig> = self
            .configs
            .values()
            .filter(|m| self.is_suitable_for_task(m, &task))
            .collect();

        if suitable_models.is_empty() {
            return RoutingDecision {
                model_id: self.default_model.clone(),
                reason: "No suitable model found, using default".to_string(),
                estimated_cost: 0.0,
                estimated_time_ms: 0,
            };
        }

        // Score each model
        let mut best_model = suitable_models[0];
        let mut best_score = f64::MIN;

        for model in suitable_models {
            let mut score = 0.0;

            // Quality score based on task needs
            score += match (&task, &model.capabilities.quality_tier) {
                (TaskType::CreativeWriting, QualityTier::Ultra) => 100.0,
                (TaskType::CreativeWriting, QualityTier::High) => 80.0,
                (TaskType::Editing, QualityTier::High) => 90.0,
                (TaskType::Analysis, QualityTier::Ultra) => 90.0,
                (TaskType::Analysis, QualityTier::High) => 80.0,
                (TaskType::Summarization, _) => 50.0, // Less critical
                _ => 70.0,
            };

            // Complexity adjustment
            score += match complexity {
                Complexity::Low => 0.0,
                Complexity::Medium => 20.0,
                Complexity::High => 50.0,
                Complexity::Critical => 100.0,
            };

            // Budget priority adjustment
            score -= match budget_priority {
                Priority::Low => 0.0,
                Priority::Medium => model.cost_per_1k_output * 10.0,
                Priority::High => model.cost_per_1k_output * 30.0,
            };

            // Speed priority adjustment
            score -= match speed_priority {
                Priority::Low => 0.0,
                Priority::Medium => match model.capabilities.speed_tier {
                    SpeedTier::Fast => 0.0,
                    SpeedTier::Normal => 10.0,
                    SpeedTier::Slow => 30.0,
                    SpeedTier::VerySlow => 50.0,
                },
                Priority::High => match model.capabilities.speed_tier {
                    SpeedTier::Fast => 0.0,
                    SpeedTier::Normal => 30.0,
                    SpeedTier::Slow => 80.0,
                    SpeedTier::VerySlow => 150.0,
                },
            };

            if score > best_score {
                best_score = score;
                best_model = model;
            }
        }

        RoutingDecision {
            model_id: best_model.id.clone(),
            reason: format!(
                "Selected based on {:?} task with {:?} complexity",
                task, complexity
            ),
            estimated_cost: best_model.cost_per_1k_output,
            estimated_time_ms: match best_model.capabilities.speed_tier {
                SpeedTier::Fast => 1000,
                SpeedTier::Normal => 5000,
                SpeedTier::Slow => 15000,
                SpeedTier::VerySlow => 45000,
            },
        }
    }

    fn is_suitable_for_task(&self, model: &ModelConfig, task: &TaskType) -> bool {
        match task {
            TaskType::CreativeWriting => {
                matches!(
                    model.capabilities.quality_tier,
                    QualityTier::High | QualityTier::Ultra
                )
            }
            TaskType::Analysis => model.capabilities.max_context_length >= 8000,
            _ => true, // Most models can handle other tasks
        }
    }

    pub fn get_model(&self, model_id: &str) -> Option<&ModelConfig> {
        self.configs.get(model_id)
    }

    pub fn get_all_models(&self) -> Vec<&ModelConfig> {
        self.configs.values().collect()
    }
}

#[derive(Debug, Clone)]
pub enum Complexity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub enum Priority {
    Low,
    Medium,
    High,
}
