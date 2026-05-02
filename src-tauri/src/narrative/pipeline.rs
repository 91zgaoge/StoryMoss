//! NarrativePipeline — 叙事流水线抽象框架
//!
//! 核心设计：提取 Bootstrap 和拆书的共同流程模式，形成可复用的 Pipeline。
//! 正向（Genesis）和逆向（Analysis）都是 NarrativePipeline 的实现。

use crate::llm::LlmService;
use super::progress::PipelineProgressEvent;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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
}

impl<Context: StepContext + Send> NarrativePipelineExecutor<Context> {
    pub fn new(steps: Vec<Box<dyn PipelineStep<Context>>>) -> Self {
        let total = steps.len();
        Self { steps, total_steps: total }
    }

    /// 执行流水线
    ///
    /// 按顺序执行所有步骤，每个步骤完成后更新进度。
    /// 如果某个步骤失败，可以选择继续（跳过）或中断。
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

            // 执行步骤
            let progress_clone = progress_callback.clone();
            let result = step.execute(
                ctx,
                llm,
                progress_clone,
            ).await;

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
