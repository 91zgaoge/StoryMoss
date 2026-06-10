//! 记忆保留管理 - Phase 1.4
//!
//! 基于 Ebbinghaus 遗忘曲线理论的记忆优先级管理
//! R(t) = R₀ × e^(-λt) + Σ(强化奖励)

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};

use crate::db::models::{Entity, RetentionConfig};

/// 记忆保留评分结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionScore {
    pub entity_id: String,
    pub entity_name: String,
    pub base_score: f32,       // 基础置信度 R₀
    pub decayed_score: f32,    // 衰减后分数 R(t)
    pub reinforced_score: f32, // 强化后分数
    pub final_priority: f32,   // 最终优先级
    pub priority_level: PriorityLevel,
    pub days_since_last_access: i64,
    pub access_count: i32,
    pub estimated_retention_days: i64, // 估计保留天数
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PriorityLevel {
    Critical,  // > 0.8 - 必须保留
    High,      // 0.6 - 0.8 - 优先保留
    Medium,    // 0.4 - 0.6 - 正常保留
    Low,       // 0.2 - 0.4 - 可压缩
    Forgotten, // < 0.2 - 可遗忘
}

impl PriorityLevel {
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s > 0.8 => PriorityLevel::Critical,
            s if s > 0.6 => PriorityLevel::High,
            s if s > 0.4 => PriorityLevel::Medium,
            s if s > 0.2 => PriorityLevel::Low,
            _ => PriorityLevel::Forgotten,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            PriorityLevel::Critical => "critical",
            PriorityLevel::High => "high",
            PriorityLevel::Medium => "medium",
            PriorityLevel::Low => "low",
            PriorityLevel::Forgotten => "forgotten",
        }
    }
}

/// 保留管理器
pub struct RetentionManager {
    config: RetentionConfig,
    /// 架构级记忆配置（衰减慢）
    architecture_config: RetentionConfig,
    /// 瞬态记忆配置（衰减快）
    transient_config: RetentionConfig,
}

impl RetentionManager {
    /// 创建默认保留管理器
    pub fn new() -> Self {
        Self {
            // 默认配置：中等衰减
            config: RetentionConfig {
                lambda: 0.05,             // 中等衰减率
                reinforcement_bonus: 0.1, // 每次强化增加 0.1
            },
            // 架构级：衰减慢（重要设定）
            architecture_config: RetentionConfig {
                lambda: 0.01,
                reinforcement_bonus: 0.15,
            },
            // 瞬态：衰减快（临时信息）
            transient_config: RetentionConfig {
                lambda: 0.1,
                reinforcement_bonus: 0.05,
            },
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(config: RetentionConfig) -> Self {
        Self {
            config: config.clone(),
            architecture_config: RetentionConfig {
                lambda: config.lambda * 0.2,
                reinforcement_bonus: config.reinforcement_bonus * 1.5,
            },
            transient_config: RetentionConfig {
                lambda: config.lambda * 2.0,
                reinforcement_bonus: config.reinforcement_bonus * 0.5,
            },
        }
    }

    /// 计算实体的保留分数
    pub fn calculate_retention_score(&self, entity: &Entity) -> RetentionScore {
        let now = Local::now();

        // 基础置信度
        let base_score = entity.confidence_score.unwrap_or(0.5);

        // 计算距离上次访问的天数
        let days_since_access = entity
            .last_accessed
            .map(|last| (now - last).num_days())
            .unwrap_or(30); // 如果从未访问，假设30天前

        // 根据实体类型选择配置
        let config = self.get_config_for_entity(entity);

        // 计算衰减分数 R(t) = R₀ × e^(-λt)
        let time_decay = (-config.lambda * days_since_access as f64).exp() as f32;
        let decayed_score = base_score * time_decay;

        // 计算强化奖励
        let reinforcement = (entity.access_count as f64 * config.reinforcement_bonus)
            .min(0.5) // 最多增加 0.5
            as f32;
        let reinforced_score = (decayed_score + reinforcement).min(1.0);

        // 计算最终优先级（考虑访问频率）
        let recency_boost = if days_since_access < 7 {
            0.1 // 最近访问过，增加优先级
        } else {
            0.0
        };

        let final_priority = (reinforced_score + recency_boost).min(1.0);

        // 计算估计保留天数
        let estimated_retention = if final_priority > 0.5 {
            ((final_priority.ln() / -config.lambda as f32) as i64).max(1)
        } else {
            0
        };

        RetentionScore {
            entity_id: entity.id.clone(),
            entity_name: entity.name.clone(),
            base_score,
            decayed_score,
            reinforced_score,
            final_priority,
            priority_level: PriorityLevel::from_score(final_priority),
            days_since_last_access: days_since_access,
            access_count: entity.access_count,
            estimated_retention_days: estimated_retention,
        }
    }

    /// 批量计算保留分数
    pub fn batch_calculate(&self, entities: &[Entity]) -> Vec<RetentionScore> {
        entities
            .iter()
            .map(|e| self.calculate_retention_score(e))
            .collect()
    }

    /// 根据优先级筛选实体
    pub fn filter_by_priority(
        &self,
        entities: &[Entity],
        min_level: PriorityLevel,
    ) -> Vec<(Entity, RetentionScore)> {
        let scores = self.batch_calculate(entities);
        let min_threshold = self.priority_to_threshold(&min_level);

        entities
            .iter()
            .zip(scores)
            .filter(|(_, score)| score.final_priority >= min_threshold)
            .map(|(e, s)| (e.clone(), s))
            .collect()
    }

    /// 获取应该被遗忘的实体（用于清理）
    pub fn get_forgotten_entities(&self, entities: &[Entity]) -> Vec<(Entity, RetentionScore)> {
        let scores = self.batch_calculate(entities);

        entities
            .iter()
            .zip(scores)
            .filter(|(_, score)| matches!(score.priority_level, PriorityLevel::Forgotten))
            .map(|(e, s)| (e.clone(), s))
            .collect()
    }

    /// 获取应该压缩的实体（存储到低成本存储）
    pub fn get_compressible_entities(&self, entities: &[Entity]) -> Vec<(Entity, RetentionScore)> {
        let scores = self.batch_calculate(entities);

        entities
            .iter()
            .zip(scores)
            .filter(|(_, score)| matches!(score.priority_level, PriorityLevel::Low))
            .map(|(e, s)| (e.clone(), s))
            .collect()
    }

    /// 计算上下文窗口的最佳填充
    /// 根据优先级和token预算选择实体
    pub fn select_for_context(
        &self,
        entities: &[Entity],
        token_budget: usize,
        avg_tokens_per_entity: usize,
    ) -> Vec<(Entity, RetentionScore)> {
        let mut scored: Vec<(Entity, RetentionScore)> = entities
            .iter()
            .map(|e| {
                let score = self.calculate_retention_score(e);
                (e.clone(), score)
            })
            .collect();

        // 按优先级排序
        scored.sort_by(|a, b| b.1.final_priority.partial_cmp(&a.1.final_priority).unwrap());

        // 根据token预算选择
        let max_entities = token_budget / avg_tokens_per_entity;
        scored.into_iter().take(max_entities).collect()
    }

    /// 模拟访问实体（增加访问计数和强化）
    pub fn simulate_access(&self, entity: &mut Entity) {
        entity.access_count += 1;
        entity.last_accessed = Some(Local::now());

        // 增加置信度（强化）
        let config = self.get_config_for_entity(entity);
        let boost = config.reinforcement_bonus as f32;
        if let Some(ref mut confidence) = entity.confidence_score {
            *confidence = (*confidence + boost).min(1.0);
        } else {
            entity.confidence_score = Some(0.5 + boost);
        }
    }

    /// 预测遗忘时间
    pub fn predict_forgetting_time(
        &self,
        entity: &Entity,
        threshold: f32,
    ) -> Option<DateTime<Local>> {
        let base_score = entity.confidence_score.unwrap_or(0.5);
        if base_score <= threshold {
            return Some(Local::now()); // 已经低于阈值
        }

        let config = self.get_config_for_entity(entity);

        // 解算 R(t) = threshold: t = -ln(threshold/R₀) / λ
        let days_to_forget = -(threshold as f64 / base_score as f64).ln() / config.lambda;

        if days_to_forget.is_finite() && days_to_forget > 0.0 {
            Some(Local::now() + Duration::days(days_to_forget as i64))
        } else {
            None // 永远不会遗忘
        }
    }

    /// 生成保留报告
    pub fn generate_retention_report(&self, entities: &[Entity]) -> RetentionReport {
        let scores = self.batch_calculate(entities);

        let mut level_counts: HashMap<String, usize> = HashMap::new();
        let mut total_priority = 0.0;
        let mut critical_entities = vec![];
        let mut forgotten_entities = vec![];

        for score in &scores {
            let level = score.priority_level.to_string();
            *level_counts.entry(level.to_string()).or_insert(0) += 1;
            total_priority += score.final_priority;

            if score.priority_level == PriorityLevel::Critical {
                critical_entities.push(score.entity_name.clone());
            }
            if score.priority_level == PriorityLevel::Forgotten {
                forgotten_entities.push(score.entity_name.clone());
            }
        }

        let avg_priority = if !scores.is_empty() {
            total_priority / scores.len() as f32
        } else {
            0.0
        };

        RetentionReport {
            total_entities: entities.len(),
            avg_priority,
            level_distribution: level_counts,
            critical_entities,
            forgotten_entities,
            recommended_action: self.generate_recommendation(&scores),
        }
    }

    // ============== 私有辅助方法 ==============

    fn get_config_for_entity(&self, entity: &Entity) -> &RetentionConfig {
        match entity.entity_type {
            // 概念/设定类使用架构级配置（衰减慢）
            crate::db::models::EntityType::Concept => &self.architecture_config,
            // 角色使用默认配置
            crate::db::models::EntityType::Character => &self.config,
            // 地点使用默认配置
            crate::db::models::EntityType::Location => &self.config,
            // 事件使用瞬态配置（衰减快）
            crate::db::models::EntityType::Event => &self.transient_config,
            // 其他使用默认配置
            _ => &self.config,
        }
    }

    fn priority_to_threshold(&self, level: &PriorityLevel) -> f32 {
        match level {
            PriorityLevel::Critical => 0.8,
            PriorityLevel::High => 0.6,
            PriorityLevel::Medium => 0.4,
            PriorityLevel::Low => 0.2,
            PriorityLevel::Forgotten => 0.0,
        }
    }

    fn generate_recommendation(&self, scores: &[RetentionScore]) -> String {
        let forgotten_count = scores
            .iter()
            .filter(|s| matches!(s.priority_level, PriorityLevel::Forgotten))
            .count();

        let critical_count = scores
            .iter()
            .filter(|s| matches!(s.priority_level, PriorityLevel::Critical))
            .count();

        if forgotten_count > scores.len() / 3 {
            format!(
                "警告：{} 个实体(占 {:.1}%) 已进入遗忘状态，建议进行知识蒸馏或归档",
                forgotten_count,
                forgotten_count as f32 / scores.len() as f32 * 100.0
            )
        } else if critical_count < 5 {
            "建议：关键实体较少，可能需要加强核心设定".to_string()
        } else {
            "记忆保留状态良好".to_string()
        }
    }
}

impl Default for RetentionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 保留报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionReport {
    pub total_entities: usize,
    pub avg_priority: f32,
    pub level_distribution: HashMap<String, usize>,
    pub critical_entities: Vec<String>,
    pub forgotten_entities: Vec<String>,
    pub recommended_action: String,
}

/// 归档结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub archived_count: usize,
    pub archived_entities: Vec<String>,
    pub story_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Entity, EntityType};

    fn create_test_entity(name: &str, confidence: Option<f32>, access_count: i32) -> Entity {
        Entity {
            id: format!("test-{}", name),
            story_id: "test-story".to_string(),
            name: name.to_string(),
            entity_type: EntityType::Character,
            attributes: serde_json::json!({}),
            embedding: None,
            first_seen: Local::now() - Duration::days(30),
            last_updated: Local::now() - Duration::days(5),
            confidence_score: confidence,
            access_count,
            last_accessed: if access_count > 0 {
                Some(Local::now() - Duration::days(access_count as i64))
            } else {
                None
            },
            is_archived: false,
            archived_at: None,
        }
    }

    #[test]
    fn test_retention_score_calculation() {
        let manager = RetentionManager::new();

        // 高置信度、最近访问的实体
        let entity = create_test_entity("主角", Some(0.9), 10);
        let score = manager.calculate_retention_score(&entity);

        assert!(score.base_score > 0.8);
        assert!(score.final_priority > 0.5);
        assert!(!matches!(score.priority_level, PriorityLevel::Forgotten));
    }

    #[test]
    fn test_forgotten_entity() {
        let manager = RetentionManager::new();

        // 低置信度、从未访问的实体
        let entity = create_test_entity("龙套", Some(0.1), 0);
        let score = manager.calculate_retention_score(&entity);

        assert!(matches!(score.priority_level, PriorityLevel::Forgotten));
    }

    #[test]
    fn test_simulate_access() {
        let manager = RetentionManager::new();
        let mut entity = create_test_entity("主角", Some(0.5), 0);

        manager.simulate_access(&mut entity);

        assert_eq!(entity.access_count, 1);
        assert!(entity.last_accessed.is_some());
        assert!(entity.confidence_score.unwrap() > 0.5);
    }

    #[test]
    fn test_predict_forgetting_time() {
        let manager = RetentionManager::new();
        let entity = create_test_entity("主角", Some(0.8), 5);

        let forget_time = manager.predict_forgetting_time(&entity, 0.3);
        assert!(forget_time.is_some());
        assert!(forget_time.unwrap() > Local::now());
    }
}
