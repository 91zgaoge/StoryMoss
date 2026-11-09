//! Task System Models
//!
//! 参考 memoh-X internal/schedule/types.go + internal/heartbeat/types.go 设计

use std::fmt;

use serde::{Deserialize, Serialize};

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,   // 等待执行
    Running,   // 执行中
    Completed, // 已完成
    Failed,    // 失败
    Cancelled, // 已取消
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TaskStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => TaskStatus::Running,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        }
    }
}

/// 调度类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleType {
    Once,   // 一次性
    Daily,  // 每天
    Weekly, // 每周
    Cron,   // Cron表达式
}

impl fmt::Display for ScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleType::Once => write!(f, "once"),
            ScheduleType::Daily => write!(f, "daily"),
            ScheduleType::Weekly => write!(f, "weekly"),
            ScheduleType::Cron => write!(f, "cron"),
        }
    }
}

impl ScheduleType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "daily" => ScheduleType::Daily,
            "weekly" => ScheduleType::Weekly,
            "cron" => ScheduleType::Cron,
            _ => ScheduleType::Once,
        }
    }
}

/// 任务类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    BookDeconstruction,    // 拆书分析
    GuidebookDistillation, // 指导书提炼
    CascadeRewrite,        // 级联改写
    AiGeneration,          // AI 长文本生成
    PipelineReview,        // Pipeline 审校
    WorkflowNode,          // Workflow 节点执行
    Ingest,                // 知识图谱 Ingest
    AsyncAudit,            // 异步审计（分时架构时间线 2：Inspector → annotation 回流）
    DeepInsight,           // 深度洞察（分时架构时间线 3：追读力/KG/向量/漂移，跨章节）
    ContinuationTraining,  // v0.88.0: 续写训练（取原书章节→沙盒续写→对比闭环）
    Custom,                // 自定义
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskType::BookDeconstruction => write!(f, "book_deconstruction"),
            TaskType::GuidebookDistillation => write!(f, "guidebook_distillation"),
            TaskType::CascadeRewrite => write!(f, "cascade_rewrite"),
            TaskType::AiGeneration => write!(f, "ai_generation"),
            TaskType::PipelineReview => write!(f, "pipeline_review"),
            TaskType::WorkflowNode => write!(f, "workflow_node"),
            TaskType::Ingest => write!(f, "ingest"),
            TaskType::AsyncAudit => write!(f, "async_audit"),
            TaskType::DeepInsight => write!(f, "deep_insight"),
            TaskType::ContinuationTraining => write!(f, "continuation_training"),
            TaskType::Custom => write!(f, "custom"),
        }
    }
}

impl TaskType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "book_deconstruction" => TaskType::BookDeconstruction,
            "guidebook_distillation" => TaskType::GuidebookDistillation,
            "cascade_rewrite" => TaskType::CascadeRewrite,
            "ai_generation" => TaskType::AiGeneration,
            "pipeline_review" => TaskType::PipelineReview,
            "workflow_node" => TaskType::WorkflowNode,
            "ingest" => TaskType::Ingest,
            "async_audit" => TaskType::AsyncAudit,
            "deep_insight" => TaskType::DeepInsight,
            "continuation_training" => TaskType::ContinuationTraining,
            _ => TaskType::Custom,
        }
    }
}

/// 任务记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub task_type: TaskType,
    pub schedule_type: ScheduleType,
    pub cron_pattern: Option<String>,
    pub payload: Option<String>, // JSON
    pub status: TaskStatus,
    pub progress: i32,          // 0-100
    pub result: Option<String>, // JSON
    pub error_message: Option<String>,
    pub max_retries: i32,
    pub retry_count: i32,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub heartbeat_timeout_seconds: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建任务请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub description: Option<String>,
    pub task_type: String,
    pub schedule_type: String,
    pub cron_pattern: Option<String>,
    pub payload: Option<String>,
    pub enabled: Option<bool>,
    pub max_retries: Option<i32>,
    pub heartbeat_timeout_seconds: Option<i32>,
}

/// 更新任务请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub cron_pattern: Option<String>,
    pub max_retries: Option<i32>,
    pub heartbeat_timeout_seconds: Option<i32>,
    /// v0.93.2: 任务载荷（续写训练「保存计划即更新配置」需要同步 payload）
    pub payload: Option<String>,
}

/// 任务日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLog {
    pub id: String,
    pub task_id: String,
    pub log_level: String,
    pub message: String,
    pub created_at: String,
}

/// 任务执行结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub success: bool,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
}

/// 进度事件（推送给前端）
#[derive(Debug, Clone, Serialize)]
pub struct TaskProgressEvent {
    pub task_id: String,
    pub step: String,
    pub progress: i32,
    pub message: String,
    /// v0.93.14: 事件时间与 task_logs/heartbeat 同一本地时间源，UI 可直接对齐。
    pub timestamp: String,
}

/// 心跳事件
#[derive(Debug, Clone, Serialize)]
pub struct TaskHeartbeatEvent {
    pub task_id: String,
    pub timestamp: String,
}

/// 状态变更事件
#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusChangedEvent {
    pub task_id: String,
    pub status: String,
    pub progress: i32,
    pub message: Option<String>,
    /// v0.93.14: 终态事件也带本地时间，前端统一显示。
    pub timestamp: String,
}
