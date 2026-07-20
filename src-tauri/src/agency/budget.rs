use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{
    agency::{models::AgentRole, tool_loop::LoopLlm},
    error::AppError,
    router::TaskType,
};

pub const DEFAULT_RUN_TOKEN_BUDGET: u64 = 300_000;

/// agency 全局 LLM 并发闸门（跨 run 总量上限，P2 终审 I3）。
/// 锁序：先 run 级角色预算（BudgetedLlm），后全局闸门（AgencyLlm）；
/// 所有路径同序获取，无循环等待。
pub static AGENCY_GLOBAL_LLM_SEM: once_cell::sync::Lazy<tokio::sync::Semaphore> =
    once_cell::sync::Lazy::new(|| tokio::sync::Semaphore::new(3));

/// 运行级并发预算：按角色信号量限流 + token 预算硬上限。
/// 取代对全局单一 BACKGROUND_LLM_SEMAPHORE 的依赖（agency 内部）。
pub struct AgencyBudget {
    writer_sem: Semaphore,
    producer_sem: Semaphore,
    editor_sem: Semaphore,
    token_budget: u64,
    tokens_used: AtomicU64,
}

impl AgencyBudget {
    pub fn new(token_budget: u64) -> Self {
        Self::with_role_permits(1, 1, 1, token_budget)
    }

    pub fn with_role_permits(
        writer: usize,
        producer: usize,
        editor: usize,
        token_budget: u64,
    ) -> Self {
        Self {
            writer_sem: Semaphore::new(writer),
            producer_sem: Semaphore::new(producer),
            editor_sem: Semaphore::new(editor),
            token_budget,
            tokens_used: AtomicU64::new(0),
        }
    }

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<(), AppError> {
        if self.tokens_used() >= self.token_budget {
            return Err(AppError::from(format!(
                "运行 token 预算耗尽（{}/{}），已熔断",
                self.tokens_used(),
                self.token_budget
            )));
        }
        Ok(())
    }

    pub fn record_usage(&self, tokens: i32) {
        if tokens > 0 {
            self.tokens_used.fetch_add(tokens as u64, Ordering::SeqCst);
        }
    }

    pub async fn acquire(&self, role: AgentRole) -> Result<SemaphorePermit<'_>, AppError> {
        self.check()?;
        let sem = match role {
            AgentRole::LeadWriter => &self.writer_sem,
            AgentRole::Producer => &self.producer_sem,
            AgentRole::EditorAuditor => &self.editor_sem,
        };
        sem.acquire()
            .await
            .map_err(|_| AppError::from("预算信号量已关闭"))
    }
}

/// 预算包装层：角色限流 + token 记账，对 ToolLoop 透明。
pub struct BudgetedLlm {
    inner: Arc<dyn LoopLlm>,
    budget: Arc<AgencyBudget>,
    role: AgentRole,
}

impl BudgetedLlm {
    pub fn new(inner: Arc<dyn LoopLlm>, budget: Arc<AgencyBudget>, role: AgentRole) -> Self {
        Self {
            inner,
            budget,
            role,
        }
    }
}

#[async_trait::async_trait]
impl LoopLlm for BudgetedLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        let (content, _t, _c) = self
            .complete_metered(system_prompt, user_prompt, task, max_tokens)
            .await?;
        Ok(content)
    }

    async fn complete_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        let _permit = self.budget.acquire(self.role).await?;
        let (content, tokens, cost) = self
            .inner
            .complete_metered(system_prompt, user_prompt, task, max_tokens)
            .await?;
        self.budget.record_usage(tokens);
        Ok((content, tokens, cost))
    }

    /// JSON mode 透传：角色限流内层实现 JSON mode（String 签名无 tokens
    /// 回传，这两次结构化单调用不记 run 预算——上限 2048+4096 tokens，
    /// 占默认预算 2%，可接受）。
    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        let _permit = self.budget.acquire(self.role).await?;
        self.inner
            .complete_json(system_prompt, user_prompt, task, max_tokens)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{
        agency::{models::AgentRole, tool_loop::LoopLlm},
        error::AppError,
        router::TaskType,
    };

    struct MeteredMock {
        tokens: i32,
        delay_ms: u64,
        calls: Mutex<Vec<std::time::Instant>>,
    }

    #[async_trait::async_trait]
    impl LoopLlm for MeteredMock {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            self.calls.lock().unwrap().push(std::time::Instant::now());
            Ok("输出".to_string())
        }
        async fn complete_metered(
            &self,
            s: &str,
            u: &str,
            t: TaskType,
            m: i32,
        ) -> Result<(String, i32, f64), AppError> {
            self.complete(s, u, t, m)
                .await
                .map(|out| (out, self.tokens, 0.01))
        }
    }

    #[tokio::test]
    async fn test_budget_exhaustion_blocks_call() {
        let budget = Arc::new(AgencyBudget::new(100));
        let llm = Arc::new(MeteredMock {
            tokens: 100,
            delay_ms: 0,
            calls: Mutex::new(vec![]),
        });
        let limited = BudgetedLlm::new(llm, budget.clone(), AgentRole::LeadWriter);
        // 第一次：100 tokens 入账成功
        limited
            .complete("s", "u", TaskType::CreativeWriting, 100)
            .await
            .unwrap();
        assert_eq!(budget.tokens_used(), 100);
        // 第二次：已达预算上限 → Err
        let err = limited
            .complete("s", "u", TaskType::CreativeWriting, 100)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("预算"));
    }

    #[tokio::test]
    async fn test_role_permits_serialize_same_role() {
        let budget = Arc::new(AgencyBudget::with_role_permits(1, 1, 1, 1_000_000));
        let mock = Arc::new(MeteredMock {
            tokens: 1,
            delay_ms: 80,
            calls: Mutex::new(vec![]),
        });
        let l1 = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let l2 = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let start = std::time::Instant::now();
        let (r1, r2) = tokio::join!(
            l1.complete("s", "u", TaskType::CreativeWriting, 100),
            l2.complete("s", "u", TaskType::CreativeWriting, 100),
        );
        let elapsed = start.elapsed();
        assert!(r1.is_ok() && r2.is_ok());
        assert!(
            elapsed >= std::time::Duration::from_millis(150),
            "同角色两次调用应串行（≥160ms），实际 {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_different_roles_run_parallel() {
        let budget = Arc::new(AgencyBudget::with_role_permits(1, 1, 1, 1_000_000));
        let mock = Arc::new(MeteredMock {
            tokens: 1,
            delay_ms: 80,
            calls: Mutex::new(vec![]),
        });
        let writer = BudgetedLlm::new(mock.clone(), budget.clone(), AgentRole::LeadWriter);
        let editor = BudgetedLlm::new(mock, budget, AgentRole::EditorAuditor);
        let start = std::time::Instant::now();
        let (r1, r2) = tokio::join!(
            writer.complete("s", "u", TaskType::CreativeWriting, 100),
            editor.complete("s", "u", TaskType::Proofreading, 100),
        );
        let elapsed = start.elapsed();
        assert!(r1.is_ok() && r2.is_ok());
        assert!(
            elapsed < std::time::Duration::from_millis(150),
            "不同角色应并行（<150ms），实际 {:?}",
            elapsed
        );
    }
}
