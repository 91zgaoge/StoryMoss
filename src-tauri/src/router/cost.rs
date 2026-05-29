#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Cost tracking for LLM usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracker {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost: f64,
    pub requests_count: u64,
    pub by_model: Vec<ModelCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub request_count: u64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost: 0.0,
            requests_count: 0,
            by_model: Vec::new(),
        }
    }

    pub fn record_usage(
        &mut self,
        model_id: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost_per_1k_input: f64,
        cost_per_1k_output: f64,
    ) {
        let input_cost = (input_tokens as f64 / 1000.0) * cost_per_1k_input;
        let output_cost = (output_tokens as f64 / 1000.0) * cost_per_1k_output;
        let total_cost = input_cost + output_cost;

        self.total_input_tokens += input_tokens;
        self.total_output_tokens += output_tokens;
        self.total_cost += total_cost;
        self.requests_count += 1;

        // Update or create model cost entry
        if let Some(model_cost) = self.by_model.iter_mut().find(|m| m.model_id == model_id) {
            model_cost.input_tokens += input_tokens;
            model_cost.output_tokens += output_tokens;
            model_cost.cost += total_cost;
            model_cost.request_count += 1;
        } else {
            self.by_model.push(ModelCost {
                model_id: model_id.to_string(),
                input_tokens,
                output_tokens,
                cost: total_cost,
                request_count: 1,
            });
        }
    }

    pub fn get_summary(&self) -> CostSummary {
        CostSummary {
            total_cost: self.total_cost,
            total_tokens: self.total_input_tokens + self.total_output_tokens,
            request_count: self.requests_count,
            average_cost_per_request: if self.requests_count > 0 {
                self.total_cost / self.requests_count as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost: f64,
    pub total_tokens: u64,
    pub request_count: u64,
    pub average_cost_per_request: f64,
}
