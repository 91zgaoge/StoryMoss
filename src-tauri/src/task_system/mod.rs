//! Task System Module
//!
//! 任务调度系统：支持一次性/定时任务，心跳检测，自动重试。
//! 参考 memoh-X internal/schedule + internal/heartbeat 设计。

pub mod commands;
pub mod executor;
pub mod heartbeat;
pub mod models;
pub mod repository;
pub mod scheduler;
pub mod service;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;

// Re-exports are available through individual module paths
// e.g., task_system::models::Task, task_system::service::TaskService
