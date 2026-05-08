//! NarrativePipeline — 叙事流水线抽象框架
//!
//! 核心设计：提取 Bootstrap 和拆书的共同流程模式，形成可复用的 Pipeline。
//! 正向（Genesis）和逆向（Analysis）都是 NarrativePipeline 的实现。

use crate::llm::LlmService;
use super::progress::PipelineProgressEvent;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::Mutex;

/// 全局 Pipeline 取消标志注册表
/// key: session_id, value: 取消标志
static PIPELINE_CANCEL_FLAGS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

/// 注册一个 Pipeline 的取消标志，返回 Arc<AtomicBool>
pub fn register_pipeline_cancel(session_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut guard = PIPELINE_CANCEL_FLAGS.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    guard.as_mut().unwrap().insert(session_id.to_string(), flag.clone());
    flag
}

/// 请求取消指定 session_id 的 Pipeline
pub fn cancel_pipeline(session_id: &str) -> bool {
    let guard = PIPELINE_CANCEL_FLAGS.lock().unwrap();
    if let Some(ref flags) = *guard {
        if let Some(flag) = flags.get(session_id) {
            flag.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

/// 清理已完成的 Pipeline 取消标志
pub fn unregister_pipeline_cancel(session_id: &str) {
    let mut guard = PIPELINE_CANCEL_FLAGS.lock().unwrap();
    if let Some(ref mut flags) = *guard {
        flags.remove(session_id);
    }
}

/// 流水线错误
#[derive(Debug, Clone)]
pub enum PipelineError {
    StepFailed { step_name: String, reason: String },
    Cancelled(String),
    LlmError(String),
    ParseError(String),
    StorageError(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::StepFailed { step_name, reason } => {
                write!(f, "步骤 '{}' 失败: {}", step_name, reason)
            }
            PipelineError::Cancelled(msg) => write!(f, "已取消: {}", msg),
            PipelineError::LlmError(msg) => write!(f, "LLM错误: {}", msg),
            PipelineError::ParseError(msg) => write!(f, "解析错误: {}", msg),
            PipelineError::StorageError(msg) => write!(f, "存储错误: {}", msg),
        }
    }
}

/// 单个处理步骤的上下文
pub trait StepContext: Send {
    fn story_id(&self) -> Option<&str>;
    fn set_current_step(&mut self, step_name: &str);
    fn current_step(&self) -> &str;
}

/// 单个处理步骤
///
/// 每个步骤是 Pipeline 的原子单元，负责处理一种叙事元素的生成或提取。
/// 步骤之间通过共享的 Context 传递状态和数据。
pub trait PipelineStep<Context: StepContext + Send>: Send + Sync {
    /// 步骤名称（用于进度显示）
    fn name(&self) -> &'static str;
    /// 步骤描述（用于日志和调试）
    fn description(&self) -> &'static str;
    /// 步骤在 Pipeline 中的序号（从1开始）
    fn step_number(&self) -> usize;
    /// 估计的LLM调用次数（用于进度估算）
    fn estimated_llm_calls(&self) -> usize {
        1
    }

    /// 执行步骤
    ///
    /// # 参数
    /// - `ctx`: 共享上下文，步骤可以读取和写入数据
    /// - `llm`: LLM服务，用于AI调用
    /// - `progress`: 进度回调，步骤应定期报告进度
    fn execute<'a>(
        &'a self,
        ctx: &'a mut Context,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<(), PipelineError>> + Send + 'a>>;
}

/// 叙事流水线 — 可正向（生成）可逆向（分析）
pub struct NarrativePipelineExecutor<Context: StepContext + Send> {
    steps: Vec<Box<dyn PipelineStep<Context>>>,
    total_steps: usize,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl<Context: StepContext + Send> NarrativePipelineExecutor<Context> {
    pub fn new(steps: Vec<Box<dyn PipelineStep<Context>>>) -> Self {
        let total = steps.len();
        Self { steps, total_steps: total, cancel_flag: None }
    }

    /// 设置取消标志
    pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel_flag = Some(flag);
        self
    }

    /// 执行流水线
    ///
    /// 按顺序执行所有步骤，每个步骤完成后更新进度。
    /// 如果某个步骤失败，可以选择继续（跳过）或中断。
    /// v5.4.0: 支持取消机制
    pub async fn execute(
        &self,
        ctx: &mut Context,
        llm: &LlmService,
        progress_callback: Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> Result<(), PipelineError> {
        log::info!("[NarrativePipeline] 开始执行，共 {} 步", self.total_steps);

        for (idx, step) in self.steps.iter().enumerate() {
            let step_num = idx + 1;
            ctx.set_current_step(step.name());

            // v5.4.0: 检查取消标志
            if let Some(ref flag) = self.cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    log::info!("[NarrativePipeline] Pipeline 已取消，中断执行");
                    progress_callback(PipelineProgressEvent {
                        pipeline_id: ctx.story_id().unwrap_or("unknown").to_string(),
                        pipeline_type: super::progress::PipelineType::Genesis,
                        step_name: step.name().to_string(),
                        step_number: step_num,
                        total_steps: self.total_steps,
                        status: super::progress::StepStatus::Cancelled,
                        message: "已取消".to_string(),
                        progress_percent: (step_num * 100 / self.total_steps.max(1)) as i32,
                        elapsed_seconds: 0,
                        metadata: Some(serde_json::json!({"cancelled": true})),
                    });
                    return Err(PipelineError::Cancelled("用户取消".to_string()));
                }
            }

            // 报告步骤开始
            progress_callback(PipelineProgressEvent {
                pipeline_id: ctx.story_id().unwrap_or("unknown").to_string(),
                pipeline_type: super::progress::PipelineType::Genesis,
                step_name: step.name().to_string(),
                step_number: step_num,
                total_steps: self.total_steps,
                status: super::progress::StepStatus::Running,
                message: format!("正在{}...", step.description()),
                progress_percent: (step_num * 100 / self.total_steps.max(1)) as i32,
                elapsed_seconds: 0,
                metadata: None,
            });

            let step_start = std::time::Instant::now();

            // P1-16 修复: 步骤执行期间使用 tokio::select! 监听取消标志，实现运行中取消
            let progress_clone = progress_callback.clone();
            let result = if let Some(ref flag) = self.cancel_flag {
                let flag_clone = flag.clone();
                let cancel_future = async move {
                    while !flag_clone.load(Ordering::Relaxed) {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                };
                tokio::select! {
                    r = step.execute(ctx, llm, progress_clone) => r,
                    _ = cancel_future => {
                        log::info!("[NarrativePipeline] 步骤 '{}' 执行中被取消", step.name());
                        Err(PipelineError::Cancelled("用户取消".to_string()))
                    }
                }
            } else {
                step.execute(ctx, llm, progress_clone).await
            };

            let elapsed = step_start.elapsed().as_secs();

            match result {
                Ok(()) => {
                    log::info!("[NarrativePipeline] 步骤 '{}' 完成，耗时 {}s", step.name(), elapsed);
                    progress_callback(PipelineProgressEvent {
                        pipeline_id: ctx.story_id().unwrap_or("unknown").to_string(),
                        pipeline_type: super::progress::PipelineType::Genesis,
                        step_name: step.name().to_string(),
                        step_number: step_num,
                        total_steps: self.total_steps,
                        status: super::progress::StepStatus::Completed,
                        message: format!("{} 完成", step.name()),
                        progress_percent: (step_num * 100 / self.total_steps.max(1)) as i32,
                        elapsed_seconds: elapsed,
                        metadata: None,
                    });
                }
                Err(e) => {
                    log::warn!("[NarrativePipeline] 步骤 '{}' 失败: {}", step.name(), e);
                    progress_callback(PipelineProgressEvent {
                        pipeline_id: ctx.story_id().unwrap_or("unknown").to_string(),
                        pipeline_type: super::progress::PipelineType::Genesis,
                        step_name: step.name().to_string(),
                        step_number: step_num,
                        total_steps: self.total_steps,
                        status: super::progress::StepStatus::Failed,
                        message: format!("{} 失败: {}", step.name(), e),
                        progress_percent: (step_num * 100 / self.total_steps.max(1)) as i32,
                        elapsed_seconds: elapsed,
                        metadata: Some(serde_json::json!({"error": format!("{}", e)})),
                    });
                    // 大爆炸式重构：严格要求，步骤失败即中断
                    return Err(e);
                }
            }
        }

        log::info!("[NarrativePipeline] 所有步骤完成");
        Ok(())
    }
}

/// 上下文构建器 trait — 从输入构建初始上下文
pub trait ContextBuilder<Input, Context: StepContext + Send> {
    fn build(&self, input: Input) -> Result<Context, PipelineError>;
}

/// 结果提取器 trait — 从上下文提取最终结果
pub trait ResultExtractor<Context: StepContext + Send, Output> {
    fn extract(&self, ctx: Context) -> Result<Output, PipelineError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_error_display() {
        let err = PipelineError::StepFailed {
            step_name: "世界观生成".to_string(),
            reason: "LLM超时".to_string(),
        };
        assert_eq!(format!("{}", err), "步骤 '世界观生成' 失败: LLM超时");

        let err = PipelineError::Cancelled("用户取消".to_string());
        assert_eq!(format!("{}", err), "已取消: 用户取消");

        let err = PipelineError::LlmError("连接失败".to_string());
        assert_eq!(format!("{}", err), "LLM错误: 连接失败");

        let err = PipelineError::ParseError("JSON无效".to_string());
        assert_eq!(format!("{}", err), "解析错误: JSON无效");

        let err = PipelineError::StorageError("数据库锁定".to_string());
        assert_eq!(format!("{}", err), "存储错误: 数据库锁定");
    }

    #[test]
    fn test_cancel_flag_registration() {
        let session_id = "test_session_001";
        let flag = register_pipeline_cancel(session_id);
        assert!(!flag.load(Ordering::Relaxed));

        // 取消
        let cancelled = cancel_pipeline(session_id);
        assert!(cancelled);
        assert!(flag.load(Ordering::Relaxed));

        // 清理
        unregister_pipeline_cancel(session_id);
        // 再次取消应返回 false（已注销）
        let cancelled_again = cancel_pipeline(session_id);
        assert!(!cancelled_again);
    }

    #[test]
    fn test_cancel_flag_multiple_sessions() {
        let flag1 = register_pipeline_cancel("session_a");
        let flag2 = register_pipeline_cancel("session_b");

        assert!(!flag1.load(Ordering::Relaxed));
        assert!(!flag2.load(Ordering::Relaxed));

        cancel_pipeline("session_a");
        assert!(flag1.load(Ordering::Relaxed));
        assert!(!flag2.load(Ordering::Relaxed));

        unregister_pipeline_cancel("session_a");
        unregister_pipeline_cancel("session_b");
    }

    #[test]
    fn test_narrative_pipeline_executor_new() {
        // 使用空步骤列表构建 executor
        let executor: NarrativePipelineExecutor<MockContext> = NarrativePipelineExecutor::new(vec![]);
        // 设置取消标志不应 panic
        let flag = Arc::new(AtomicBool::new(false));
        let _ = executor.with_cancel_flag(flag);
    }

    /// 测试用的 MockContext
    struct MockContext {
        story_id: String,
        current_step: String,
    }

    impl StepContext for MockContext {
        fn story_id(&self) -> Option<&str> {
            Some(&self.story_id)
        }
        fn set_current_step(&mut self, step_name: &str) {
            self.current_step = step_name.to_string();
        }
        fn current_step(&self) -> &str {
            &self.current_step
        }
    }
}
