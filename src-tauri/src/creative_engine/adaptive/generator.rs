//! 自适应生成器
//!
//! 根据用户偏好动态调整生成策略：
//! - temperature / top-p 调整
//! - prompt 权重调整
//! - 生成内容类型偏好注入

use crate::{
    db::{repositories::UserPreferenceRepository, DbPool},
    error::AppError,
};

/// 生成策略
#[derive(Debug, Clone)]
pub struct GenerationStrategy {
    /// 温度（创造性 vs 确定性）
    pub temperature: f32,
    /// top-p（核采样）
    pub top_p: f32,
    /// 最大 token 数
    pub max_tokens: i32,
    /// 系统提示词权重增强
    pub prompt_weight_adjustments: Vec<PromptWeightAdjustment>,
    /// 风格偏好注入
    pub style_injections: Vec<String>,
    /// 内容约束
    pub content_constraints: Vec<String>,
}

impl Default for GenerationStrategy {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            top_p: 0.95,
            max_tokens: 2000,
            prompt_weight_adjustments: vec![],
            style_injections: vec![],
            content_constraints: vec![],
        }
    }
}

/// Prompt 权重调整
#[derive(Debug, Clone)]
pub struct PromptWeightAdjustment {
    pub target: String,    // 调整目标（如"对话""描写"）
    pub direction: String, // increase / decrease / maintain
    pub strength: f32,     // 0.0-1.0
    pub reason: String,
}

/// 自适应生成器
pub struct AdaptiveGenerator {
    pool: DbPool,
}

impl AdaptiveGenerator {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 为故事构建生成策略
    ///
    /// `base_temperature`: 用户模型配置中的 temperature，作为策略基础值
    pub fn build_strategy(
        &self,
        story_id: &str,
        base_temperature: Option<f32>,
    ) -> Result<GenerationStrategy, AppError> {
        self.build_strategy_with_context(story_id, base_temperature, None, None)
    }

    /// Phase 5: 带上下文感知的生成策略构建
    ///
    /// `story_progress`: 故事整体进度
    /// (just_started/developing/midpoint/climax/resolution) `scene_stage`:
    /// 当前场景执行阶段 (planning/outline/drafting/review/final)
    pub fn build_strategy_with_context(
        &self,
        story_id: &str,
        base_temperature: Option<f32>,
        story_progress: Option<&str>,
        scene_stage: Option<&str>,
    ) -> Result<GenerationStrategy, AppError> {
        let mut strategy = GenerationStrategy::default();
        // 优先使用用户在 Settings 中设置的 temperature 作为基础值
        if let Some(base) = base_temperature {
            strategy.temperature = base.clamp(0.0, 2.0);
        }

        let pref_repo = UserPreferenceRepository::new(self.pool.clone());
        let prefs = pref_repo.get_by_story(story_id).map_err(AppError::from)?;

        for pref in &prefs {
            if pref.confidence < 0.6 {
                continue;
            }

            match pref.preference_type.to_string().as_str() {
                "dialogue" => self.apply_dialogue_preference(&mut strategy, pref),
                "content" => self.apply_content_preference(&mut strategy, pref),
                "pacing" => self.apply_pacing_preference(&mut strategy, pref),
                "style" => self.apply_style_preference(&mut strategy, pref),
                _ => {}
            }
        }

        // Phase 5: 根据故事进度动态调整
        if let Some(progress) = story_progress {
            self.apply_story_progress_adjustment(&mut strategy, progress);
        }

        // Phase 5: 根据场景阶段动态调整
        if let Some(stage) = scene_stage {
            self.apply_scene_stage_adjustment(&mut strategy, stage);
        }

        // 综合调整 temperature
        strategy.temperature = self.calculate_temperature(&strategy);

        Ok(strategy)
    }

    /// 根据故事整体进度调整策略
    fn apply_story_progress_adjustment(&self, strategy: &mut GenerationStrategy, progress: &str) {
        match progress {
            "just_started" => {
                // 开头阶段：鼓励创意探索，温度稍高
                strategy.temperature = (strategy.temperature + 0.05).min(1.0);
                strategy
                    .style_injections
                    .push("故事开头阶段：注重世界观铺陈和角色引入，可适当放慢节奏".to_string());
                strategy.max_tokens = (strategy.max_tokens as f32 * 0.9) as i32;
                // 开头不需要太长
            }
            "developing" => {
                // 发展阶段：标准策略，推动情节
                strategy
                    .style_injections
                    .push("故事发展阶段：推动情节前进，增加冲突和转折".to_string());
            }
            "midpoint" => {
                // 中点阶段：降低温度，增加情节紧凑度
                strategy.temperature = (strategy.temperature - 0.05).max(0.5);
                strategy
                    .style_injections
                    .push("故事 midpoint 阶段：注意情节转折的冲击力，避免平淡过渡".to_string());
                strategy.max_tokens = (strategy.max_tokens as f32 * 1.1) as i32;
            }
            "climax" => {
                // 高潮阶段：降低温度追求质量，增加 token 预算
                strategy.temperature = (strategy.temperature - 0.1).max(0.5);
                strategy.max_tokens = (strategy.max_tokens as f32 * 1.25) as i32;
                strategy.style_injections.push(
                    "故事高潮阶段：全力冲刺！加大冲突强度，提升叙事张力，让情感爆发".to_string(),
                );
                strategy
                    .content_constraints
                    .push("高潮段落必须紧凑有力，避免冗余描写".to_string());
            }
            "resolution" => {
                // 结局阶段：降低温度，注重收束和伏笔回收
                strategy.temperature = (strategy.temperature - 0.05).max(0.5);
                strategy
                    .style_injections
                    .push("故事结局阶段：注重收束感，回收主要伏笔，给读者满足感".to_string());
                strategy
                    .content_constraints
                    .push("注意呼应前文的伏笔和设定，保持首尾一致".to_string());
            }
            _ => {}
        }
    }

    /// 根据场景执行阶段调整策略
    fn apply_scene_stage_adjustment(&self, strategy: &mut GenerationStrategy, stage: &str) {
        match stage {
            "planning" => {
                // 规划阶段：生成构思，token 减半
                strategy.max_tokens = (strategy.max_tokens as f32 * 0.5) as i32;
                strategy.temperature = (strategy.temperature + 0.1).min(1.0); // 创意阶段温度高
                strategy
                    .style_injections
                    .push("场景规划阶段：生成创意构思，不必拘泥于具体文字".to_string());
            }
            "outline" => {
                // 大纲阶段：生成详细大纲
                strategy.max_tokens = (strategy.max_tokens as f32 * 0.7) as i32;
                strategy.temperature = (strategy.temperature + 0.05).min(1.0);
                strategy
                    .style_injections
                    .push("场景大纲阶段：生成结构化大纲，包含起承转合".to_string());
            }
            "drafting" => {
                // 起草阶段：标准策略
                strategy
                    .style_injections
                    .push("场景起草阶段：专注于叙事流畅性和角色表现".to_string());
            }
            "review" => {
                // 审校阶段：降低温度，注重精确性
                strategy.temperature = (strategy.temperature - 0.1).max(0.5);
                strategy
                    .style_injections
                    .push("场景审校阶段：注重文字精确性和逻辑一致性".to_string());
                strategy
                    .content_constraints
                    .push("仔细检查与前文的一致性，修正任何矛盾".to_string());
            }
            "final" => {
                // 定稿阶段：最低温度，追求最终质量
                strategy.temperature = (strategy.temperature - 0.15).max(0.4);
                strategy.max_tokens = (strategy.max_tokens as f32 * 1.1) as i32;
                strategy
                    .style_injections
                    .push("场景定稿阶段：精益求精，每个字都要有价值".to_string());
                strategy
                    .content_constraints
                    .push("删除冗余描述，保留精华，确保节奏紧凑".to_string());
            }
            _ => {}
        }
    }

    fn apply_dialogue_preference(
        &self,
        strategy: &mut GenerationStrategy,
        pref: &crate::db::models::UserPreference,
    ) {
        if pref.preference_key.as_str() == "dialogue_ratio" { match pref.preference_value.as_str() {
            "prefer_more_dialogue" => {
                strategy
                    .prompt_weight_adjustments
                    .push(PromptWeightAdjustment {
                        target: "对话".to_string(),
                        direction: "increase".to_string(),
                        strength: pref.confidence,
                        reason: "用户偏好更多对话".to_string(),
                    });
                strategy
                    .content_constraints
                    .push("增加对话比例，让角色通过对话推动情节".to_string());
            }
            "prefer_less_dialogue" => {
                strategy
                    .prompt_weight_adjustments
                    .push(PromptWeightAdjustment {
                        target: "对话".to_string(),
                        direction: "decrease".to_string(),
                        strength: pref.confidence,
                        reason: "用户偏好减少对话".to_string(),
                    });
                strategy
                    .content_constraints
                    .push("减少对话，增加叙述和描写".to_string());
            }
            _ => {}
        } }
    }

    fn apply_content_preference(
        &self,
        strategy: &mut GenerationStrategy,
        pref: &crate::db::models::UserPreference,
    ) {
        match pref.preference_key.as_str() {
            "description_ratio" => match pref.preference_value.as_str() {
                "prefer_more_description" => {
                    strategy
                        .prompt_weight_adjustments
                        .push(PromptWeightAdjustment {
                            target: "环境描写".to_string(),
                            direction: "increase".to_string(),
                            strength: pref.confidence,
                            reason: "用户偏好更多环境描写".to_string(),
                        });
                    strategy
                        .content_constraints
                        .push("增加环境描写和氛围渲染".to_string());
                    strategy.temperature = (strategy.temperature + 0.05).min(1.0);
                }
                "prefer_less_description" => {
                    strategy
                        .prompt_weight_adjustments
                        .push(PromptWeightAdjustment {
                            target: "环境描写".to_string(),
                            direction: "decrease".to_string(),
                            strength: pref.confidence,
                            reason: "用户偏好减少环境描写".to_string(),
                        });
                    strategy
                        .content_constraints
                        .push("精简环境描写，聚焦于情节和动作".to_string());
                }
                _ => {}
            },
            "interior_monologue" => match pref.preference_value.as_str() {
                "prefer_more_interior_monologue" => {
                    strategy
                        .content_constraints
                        .push("增加角色内心独白和心理活动描写".to_string());
                }
                "prefer_less_interior_monologue" => {
                    strategy
                        .content_constraints
                        .push("减少内心独白，多展示角色的外在行为和对话".to_string());
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn apply_pacing_preference(
        &self,
        strategy: &mut GenerationStrategy,
        pref: &crate::db::models::UserPreference,
    ) {
        if pref.preference_key.as_str() == "sentence_length" { match pref.preference_value.as_str() {
            "prefer_slower_pacing" => {
                strategy
                    .prompt_weight_adjustments
                    .push(PromptWeightAdjustment {
                        target: "节奏".to_string(),
                        direction: "decrease".to_string(),
                        strength: pref.confidence,
                        reason: "用户偏好慢节奏".to_string(),
                    });
                strategy
                    .content_constraints
                    .push("使用更长、更复杂的句子，放慢叙事节奏".to_string());
                strategy.temperature = (strategy.temperature - 0.05).max(0.5);
            }
            "prefer_faster_pacing" => {
                strategy
                    .prompt_weight_adjustments
                    .push(PromptWeightAdjustment {
                        target: "节奏".to_string(),
                        direction: "increase".to_string(),
                        strength: pref.confidence,
                        reason: "用户偏好快节奏".to_string(),
                    });
                strategy
                    .content_constraints
                    .push("使用短句、快节奏，增加动作密度".to_string());
                strategy.temperature = (strategy.temperature + 0.05).min(1.0);
            }
            _ => {}
        } }
    }

    fn apply_style_preference(
        &self,
        strategy: &mut GenerationStrategy,
        pref: &crate::db::models::UserPreference,
    ) {
        if pref.preference_key.as_str() == "overall_satisfaction" {
            match pref.preference_value.as_str() {
                "needs_improvement" => {
                    // 降低 temperature 以增加可控性
                    strategy.temperature = (strategy.temperature - 0.1).max(0.5);
                    strategy
                        .style_injections
                        .push("注意：用户近期满意度较低，请严格遵循风格和结构规范".to_string());
                }
                "high_satisfaction" => {
                    // 可适当提高创造性
                    strategy.temperature = (strategy.temperature + 0.05).min(1.0);
                    strategy
                        .style_injections
                        .push("用户满意度较高，保持当前风格即可".to_string());
                }
                _ => {}
            }
        }
    }

    fn calculate_temperature(&self, strategy: &GenerationStrategy) -> f32 {
        let constraint_count = strategy.content_constraints.len() as f32;
        let adjustment = if constraint_count > 3.0 { -0.05 } else { 0.0 };
        (strategy.temperature + adjustment).clamp(0.5, 1.0)
    }

    /// 将策略转换为 prompt 扩展文本
    pub fn strategy_to_prompt(strategy: &GenerationStrategy) -> String {
        let mut parts = Vec::new();

        if !strategy.style_injections.is_empty() {
            for injection in &strategy.style_injections {
                parts.push(injection.clone());
            }
        }

        if !strategy.content_constraints.is_empty() {
            parts.push("\n【内容调整】".to_string());
            for constraint in &strategy.content_constraints {
                parts.push(format!("- {}", constraint));
            }
        }

        if !strategy.prompt_weight_adjustments.is_empty() {
            parts.push("\n【生成策略调整】".to_string());
            for adj in &strategy.prompt_weight_adjustments {
                let direction_cn = match adj.direction.as_str() {
                    "increase" => "增加",
                    "decrease" => "减少",
                    _ => "保持",
                };
                parts.push(format!(
                    "- {}「{}」比重（置信度: {:.0}%）",
                    direction_cn,
                    adj.target,
                    adj.strength * 100.0
                ));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strategy() {
        let s = GenerationStrategy::default();
        assert_eq!(s.temperature, 0.8);
        assert_eq!(s.max_tokens, 2000);
    }

    #[test]
    fn test_strategy_to_prompt() {
        let mut s = GenerationStrategy::default();
        s.content_constraints.push("增加对话比例".to_string());
        s.prompt_weight_adjustments.push(PromptWeightAdjustment {
            target: "对话".to_string(),
            direction: "increase".to_string(),
            strength: 0.8,
            reason: "test".to_string(),
        });

        let prompt = AdaptiveGenerator::strategy_to_prompt(&s);
        assert!(prompt.contains("增加对话比例"));
        assert!(prompt.contains("增加「对话」比重"));
    }

    #[test]
    fn test_calculate_temperature() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        s.content_constraints.push("c1".to_string());
        s.content_constraints.push("c2".to_string());
        s.content_constraints.push("c3".to_string());
        s.content_constraints.push("c4".to_string());
        let temp = g.calculate_temperature(&s);
        assert_eq!(temp, 0.75); // 约束多，降低
    }

    #[test]
    fn test_calculate_temperature_respects_existing() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        s.temperature = 0.7; // 模拟 pacing/style 偏好已微调
        s.content_constraints.push("c1".to_string());
        s.content_constraints.push("c2".to_string());
        s.content_constraints.push("c3".to_string());
        s.content_constraints.push("c4".to_string());
        let temp = g.calculate_temperature(&s);
        assert_eq!(temp, 0.65); // 基于 0.7 继续微调，不是覆盖回 0.8
    }

    #[test]
    fn test_story_progress_adjustment_climax() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        g.apply_story_progress_adjustment(&mut s, "climax");
        assert!(s.temperature < 0.8, "climax should lower temperature");
        assert!(s.max_tokens > 2000, "climax should increase max_tokens");
        assert!(
            s.style_injections.iter().any(|i| i.contains("高潮")),
            "should inject climax guidance"
        );
    }

    #[test]
    fn test_story_progress_adjustment_just_started() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        g.apply_story_progress_adjustment(&mut s, "just_started");
        assert!(
            s.temperature > 0.8,
            "just_started should increase temperature for creativity"
        );
        assert!(s.max_tokens < 2000, "just_started should reduce max_tokens");
    }

    #[test]
    fn test_scene_stage_adjustment_planning() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        g.apply_scene_stage_adjustment(&mut s, "planning");
        assert!(
            s.temperature > 0.8,
            "planning should increase temperature for ideation"
        );
        assert!(s.max_tokens < 1500, "planning should halve max_tokens");
    }

    #[test]
    fn test_scene_stage_adjustment_final() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        g.apply_scene_stage_adjustment(&mut s, "final");
        assert!(
            s.temperature < 0.7,
            "final should significantly lower temperature"
        );
        assert!(s.max_tokens > 2000, "final should increase max_tokens");
    }

    #[test]
    fn test_combined_progress_and_stage() {
        let g = AdaptiveGenerator::new(
            crate::db::DbPool::new(r2d2_sqlite::SqliteConnectionManager::memory()).unwrap(),
        );
        let mut s = GenerationStrategy::default();
        // climax + final = very low temperature, high tokens
        g.apply_story_progress_adjustment(&mut s, "climax");
        g.apply_scene_stage_adjustment(&mut s, "final");
        assert!(
            s.temperature < 0.65,
            "climax + final should result in very low temperature"
        );
        assert!(
            s.max_tokens > 2500,
            "climax + final should result in high token budget"
        );
    }
}
