//! Reading Power System - 追读力系统
//!
//! 参考 webnovel-writer 的追读力设计：
//! - Hook（钩子）：章末悬念类型与强度
//! - Cool-point（爽点）：爽点模式使用统计
//! - Micropayoff（微兑现）：小承诺的即时兑现
//! - Debt（债务）：违背软建议时产生的追读力债务，含利息机制
//! - Override Contract：违背约束时的偿还计划与截止章节

use serde::{Deserialize, Serialize};

use crate::db::{
    ChapterReadingPowerRepository, ChaseDebtRepository, DbPool, OverrideContractRepository,
    SceneCommitRepository,
};

pub mod evaluator;

/// 章节追读力评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingPowerEvaluation {
    pub chapter_number: i32,
    pub hook_type: Option<String>,
    pub hook_strength: String,
    pub coolpoint_patterns: Vec<String>,
    pub micropayoffs: Vec<String>,
    pub hard_violations: Vec<String>,
    pub soft_suggestions: Vec<String>,
    pub is_transition: bool,
    pub override_count: i32,
    pub debt_balance: f64,
    pub score: f64,
}

/// 追读力评估器
pub struct ReadingPowerEvaluator {
    pool: DbPool,
}

impl ReadingPowerEvaluator {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 评估章节追读力
    pub fn evaluate(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<ReadingPowerEvaluation, String> {
        // 从 commit 中提取数据
        let commit_repo = SceneCommitRepository::new(self.pool.clone());
        let commits = commit_repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取提交记录失败: {}", e))?;

        let chapter_commit = commits.iter().find(|c| c.chapter_number == chapter_number);

        // 提取爽点模式
        let coolpoint_patterns = Vec::new();
        let micropayoffs = Vec::new();

        // 检查是否有债务
        let debt_repo = ChaseDebtRepository::new(self.pool.clone());
        let active_debts = debt_repo
            .get_active_by_story(story_id)
            .map_err(|e| format!("获取债务失败: {}", e))?;

        let debt_balance: f64 = active_debts.iter().map(|d| d.current_amount).sum();

        // 检查 override contracts
        let override_repo = OverrideContractRepository::new(self.pool.clone());
        let pending_overrides = override_repo
            .get_pending_by_story(story_id)
            .map_err(|e| format!("获取覆写合约失败: {}", e))?;

        let override_count = pending_overrides.len() as i32;

        // 计算综合得分
        let hook_score: f64 = match chapter_commit {
            Some(c) if c.dominant_strand.is_some() => 0.8,
            _ => 0.5,
        };

        let coolpoint_score: f64 = if coolpoint_patterns.is_empty() {
            0.0
        } else {
            0.7
        };
        let debt_penalty = (debt_balance * 0.1_f64).min(0.5_f64);

        let score: f64 = (hook_score * 0.4_f64 + coolpoint_score * 0.3_f64 + 0.3_f64)
            .min(1.0_f64)
            .max(0.0_f64)
            - debt_penalty;

        Ok(ReadingPowerEvaluation {
            chapter_number,
            hook_type: None, // 从摘要中提取
            hook_strength: if score > 0.7 {
                "strong".to_string()
            } else if score > 0.4 {
                "medium".to_string()
            } else {
                "weak".to_string()
            },
            coolpoint_patterns,
            micropayoffs,
            hard_violations: Vec::new(),
            soft_suggestions: Vec::new(),
            is_transition: false,
            override_count,
            debt_balance,
            score: score.max(0.0),
        })
    }

    /// 获取最近 N 章的追读力趋势
    pub fn get_trend(
        &self,
        story_id: &str,
        last_n: i64,
    ) -> Result<Vec<ReadingPowerEvaluation>, String> {
        let repo = ChapterReadingPowerRepository::new(self.pool.clone());
        let items = repo
            .get_by_story(story_id, last_n)
            .map_err(|e| format!("获取追读力数据失败: {}", e))?;

        let evaluations = items
            .iter()
            .map(|item| {
                let coolpoint_patterns: Vec<String> = item
                    .coolpoint_patterns_json
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                let micropayoffs: Vec<String> = item
                    .micropayoffs_json
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                ReadingPowerEvaluation {
                    chapter_number: item.chapter_number,
                    hook_type: item.hook_type.clone(),
                    hook_strength: item.hook_strength.clone(),
                    coolpoint_patterns,
                    micropayoffs,
                    hard_violations: Vec::new(),
                    soft_suggestions: Vec::new(),
                    is_transition: item.is_transition,
                    override_count: item.override_count,
                    debt_balance: item.debt_balance,
                    score: 0.0, // 简化
                }
            })
            .collect();

        Ok(evaluations)
    }

    /// 计算债务利息
    pub fn accrue_interest(&self, story_id: &str) -> Result<usize, String> {
        let debt_repo = ChaseDebtRepository::new(self.pool.clone());
        debt_repo
            .apply_interest(story_id)
            .map_err(|e| format!("计算利息失败: {}", e))
    }

    /// 检查超期债务
    pub fn check_overdue_debts(
        &self,
        story_id: &str,
        current_chapter: i32,
    ) -> Result<Vec<crate::db::ChaseDebt>, String> {
        let debt_repo = ChaseDebtRepository::new(self.pool.clone());
        debt_repo
            .get_overdue(story_id, current_chapter)
            .map_err(|e| format!("获取超期债务失败: {}", e))
    }
}

/// 追读力债务管理器
pub struct DebtManager {
    pool: DbPool,
}

impl DebtManager {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建债务
    pub fn create_debt(
        &self,
        story_id: &str,
        debt_type: &str,
        original_amount: f64,
        interest_rate: f64,
        source_chapter: i32,
        due_chapter: i32,
    ) -> Result<crate::db::ChaseDebt, String> {
        let repo = ChaseDebtRepository::new(self.pool.clone());
        repo.create(
            story_id,
            debt_type,
            original_amount,
            interest_rate,
            source_chapter,
            due_chapter,
        )
        .map_err(|e| format!("创建债务失败: {}", e))
    }

    /// 创建 Override Contract
    pub fn create_override_contract(
        &self,
        story_id: &str,
        chapter_number: i32,
        constraint_type: &str,
        constraint_id: &str,
        rationale_type: &str,
        rationale_text: &str,
        payback_plan: &str,
        due_chapter: i32,
    ) -> Result<crate::db::OverrideContract, String> {
        let repo = OverrideContractRepository::new(self.pool.clone());
        repo.create(
            story_id,
            chapter_number,
            constraint_type,
            constraint_id,
            rationale_type,
            rationale_text,
            payback_plan,
            due_chapter,
        )
        .map_err(|e| format!("创建覆写合约失败: {}", e))
    }

    /// 标记合约已履行
    pub fn fulfill_contract(&self, contract_id: i64) -> Result<usize, String> {
        let repo = OverrideContractRepository::new(self.pool.clone());
        repo.mark_fulfilled(contract_id)
            .map_err(|e| format!("标记履行失败: {}", e))
    }
}
