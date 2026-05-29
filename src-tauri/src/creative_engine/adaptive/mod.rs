//! 自适应学习引擎
//!
//! 实现"越写越懂"的核心机制：
//! 1. 记录用户反馈（接受/拒绝/修改）
//! 2. 从反馈中挖掘稳定偏好
//! 3. 根据偏好动态调整生成策略
//! 4. 构建个性化提示词
//!
//! 所有模块在幕后透明运行，幕前无感知。

use crate::error::AppError;

pub mod feedback;
pub mod generator;
pub mod miner;
pub mod personalizer;

pub use feedback::{FeedbackEvent, FeedbackRecorder};
pub use generator::{AdaptiveGenerator, GenerationStrategy};
pub use miner::{MinedPreference, PreferenceMiner};
pub use personalizer::PromptPersonalizer;

use crate::db::DbPool;

/// 自适应学习引擎 - 统一入口
pub struct AdaptiveLearningEngine {
    pool: DbPool,
}

impl AdaptiveLearningEngine {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 记录用户反馈（成功后异步触发偏好挖掘，激活自适应学习闭环）
    pub fn record_feedback(&self, event: FeedbackEvent) -> Result<(), AppError> {
        let recorder = FeedbackRecorder::new(self.pool.clone());
        let result = recorder.record(event.clone());
        if result.is_ok() {
            let pool = self.pool.clone();
            let story_id = event.story_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let miner = PreferenceMiner::new(pool);
                match miner.mine(&story_id) {
                    Ok(prefs) if !prefs.is_empty() => {
                        log::info!(
                            "[AdaptiveLearning] Mined {} preferences for story {}",
                            prefs.len(),
                            story_id
                        );
                    }
                    Ok(_) => {}
                    Err(e) => log::warn!("[AdaptiveLearning] Preference mining failed: {}", e),
                }
            });
        }
        result
    }

    /// 挖掘故事偏好
    pub fn mine_preferences(&self, story_id: &str) -> Result<Vec<MinedPreference>, AppError> {
        let miner = PreferenceMiner::new(self.pool.clone());
        miner.mine(story_id)
    }

    /// 获取个性化生成策略
    pub fn get_generation_strategy(&self, story_id: &str) -> Result<GenerationStrategy, AppError> {
        let generator = AdaptiveGenerator::new(self.pool.clone());
        generator.build_strategy(story_id, None)
    }

    /// 构建个性化提示词扩展
    pub fn build_personalized_prompt(&self, story_id: &str) -> Result<String, AppError> {
        let personalizer = PromptPersonalizer::new(self.pool.clone());
        personalizer.build_prompt_extension(story_id)
    }
}
