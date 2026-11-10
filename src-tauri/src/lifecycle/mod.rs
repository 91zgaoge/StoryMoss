//! 统一生命周期状态机。
//!
//! StoryMoss 的任务、agency run、agent task、聊天消息与导演动作此前各自用裸
//! 字符串表达状态，终态能被后续写入覆盖。本模块把「合法转移」集中成一张表：
//! 持久化层只需要问 `next_state(entity, current, event)`，并把这个事件对应的
//! SQL 守卫写进 UPDATE。终态实体返回 `None`，调用方必须按拒绝处理。
//!
//! 约定：本模块只做纯函数判定，不打开数据库；原子性由调用点的
//! `UPDATE ... WHERE status IN (...)` + 受影响行数保证。

#![allow(dead_code)]

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

/// 跨实体的状态词汇。DB 中仍保留既有字符串，但语义都在这里定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Queued,
    Running,
    Waiting,
    Succeeded,
    Failed,
    Cancelled,
    Stale,
}

impl LifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleState::Queued => "queued",
            LifecycleState::Running => "running",
            LifecycleState::Waiting => "waiting",
            LifecycleState::Succeeded => "succeeded",
            LifecycleState::Failed => "failed",
            LifecycleState::Cancelled => "cancelled",
            LifecycleState::Stale => "stale",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            LifecycleState::Succeeded | LifecycleState::Failed | LifecycleState::Cancelled
        )
    }
}

/// 生命周期事件。实体无关；由 `next_state` 解释每个实体的合法转移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    Start,
    Progress,
    Wait,
    Resume,
    Succeed,
    Fail,
    Cancel,
    Retry,
    Reap,
    Approve,
    Execute,
    Reject,
}

/// 状态所有者。新实体必须先在此登记，否则没有合法转移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Task,
    AgencyRun,
    AgentTask,
    ChatMessage,
    DirectorAction,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Task => "task",
            EntityKind::AgencyRun => "agency_run",
            EntityKind::AgentTask => "agent_task",
            EntityKind::ChatMessage => "chat_message",
            EntityKind::DirectorAction => "director_action",
        }
    }

    /// 各实体的终态集合。SQL 守卫必须使用同一集合。
    pub fn terminal_statuses(self) -> &'static [&'static str] {
        match self {
            EntityKind::Task => &["completed", "failed", "cancelled"],
            EntityKind::AgencyRun => &["completed", "failed", "cancelled"],
            EntityKind::AgentTask => &["completed", "failed", "cancelled", "stale"],
            EntityKind::ChatMessage => &["done", "failed", "cancelled"],
            EntityKind::DirectorAction => &["done", "failed", "rejected"],
        }
    }

    pub fn is_terminal(self, status: &str) -> bool {
        self.terminal_statuses().contains(&status)
    }

    pub fn is_active(self, status: &str) -> bool {
        !self.is_terminal(status)
    }
}

/// 给定实体与当前状态，返回事件转移后的目标状态；非法/终态转移返回 None。
///
/// 这是全应用唯一的转移判定入口。新增状态必须先加表，并在调用点用
/// `UPDATE ... WHERE status IN (...)` 做同一谓词，禁止裸 `SET status = ?`。
pub fn next_state(entity: EntityKind, from: &str, event: LifecycleEvent) -> Option<&'static str> {
    use LifecycleEvent as E;
    match entity {
        EntityKind::Task => match (from, event) {
            ("pending", E::Start) => Some("running"),
            ("running", E::Progress) => Some("running"),
            ("running", E::Succeed) => Some("completed"),
            ("running", E::Fail) => Some("failed"),
            ("pending", E::Cancel) | ("running", E::Cancel) => Some("cancelled"),
            ("running", E::Retry) => Some("pending"),
            ("failed", E::Retry) => Some("pending"),
            _ => None,
        },
        EntityKind::AgencyRun => match (from, event) {
            ("pending", E::Start) => Some("running"),
            ("running", E::Progress) => Some("running"),
            ("running", E::Succeed) => Some("completed"),
            ("running", E::Fail) => Some("failed"),
            ("pending", E::Cancel) | ("running", E::Cancel) => Some("cancelled"),
            _ => None,
        },
        EntityKind::AgentTask => match (from, event) {
            ("queued", E::Start) => Some("running"),
            ("queued", E::Wait) => Some("waiting"),
            ("waiting", E::Resume) => Some("running"),
            ("running", E::Succeed) => Some("completed"),
            ("running", E::Fail) => Some("failed"),
            ("queued", E::Cancel) | ("waiting", E::Cancel) | ("running", E::Cancel) => {
                Some("cancelling")
            }
            ("queued", E::Reap) | ("waiting", E::Reap) => Some("stale"),
            _ => None,
        },
        EntityKind::ChatMessage => match (from, event) {
            ("pending", E::Start) => Some("acting"),
            ("acting", E::Progress) => Some("acting"),
            ("acting", E::Succeed) => Some("done"),
            ("acting", E::Fail) => Some("failed"),
            ("waiting", E::Resume) => Some("acting"),
            ("pending", E::Cancel) | ("acting", E::Cancel) | ("waiting", E::Cancel) => {
                Some("cancelled")
            }
            _ => None,
        },
        EntityKind::DirectorAction => match (from, event) {
            ("proposed", E::Approve) => Some("approved"),
            ("proposed", E::Reject) => Some("rejected"),
            ("approved", E::Execute) => Some("executing"),
            ("approved", E::Reject) => Some("rejected"),
            ("executing", E::Succeed) => Some("done"),
            ("executing", E::Fail) => Some("failed"),
            ("executing", E::Cancel) => Some("failed"),
            _ => None,
        },
    }
}

/// SQL 守卫谓词：只允许本次事件合法的来源状态。
/// 调用方用它拼进 UPDATE 的 WHERE，并检查受影响行数。
pub fn source_statuses(entity: EntityKind, event: LifecycleEvent) -> &'static [&'static str] {
    use LifecycleEvent as E;
    match entity {
        EntityKind::Task => match event {
            E::Start | E::Progress => &["running"],
            E::Succeed | E::Fail => &["running"],
            E::Cancel => &["pending", "running"],
            E::Retry => &["running", "failed"],
            _ => &[],
        },
        EntityKind::AgencyRun => match event {
            E::Start | E::Progress => &["pending", "running"],
            E::Succeed | E::Fail => &["running"],
            E::Cancel => &["pending", "running"],
            _ => &[],
        },
        EntityKind::AgentTask => match event {
            E::Start | E::Resume => &["queued", "waiting"],
            E::Succeed | E::Fail => &["running"],
            E::Cancel => &["queued", "waiting", "running"],
            E::Reap => &["queued", "waiting"],
            _ => &[],
        },
        EntityKind::ChatMessage => match event {
            E::Start | E::Resume => &["pending", "waiting"],
            E::Succeed | E::Fail => &["acting"],
            E::Cancel => &["pending", "acting", "waiting"],
            _ => &[],
        },
        EntityKind::DirectorAction => match event {
            E::Approve => &["proposed"],
            E::Execute => &["approved"],
            E::Reject => &["proposed", "approved"],
            E::Succeed | E::Fail | E::Cancel => &["executing"],
            _ => &[],
        },
    }
}

// ==================== 取消注册表 ====================

/// 一个实体的取消令牌。可被执行器与 UI 共同持有；取消是单向的。
#[derive(Clone)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// 兼容既有 `Arc<AtomicBool>` 调用点；返回的是同一标志的共享句柄。
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 在取消前等待；取消后立即返回。
    pub async fn wait(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// 实体级取消注册表。`scope()` 返回 RAII 作用域，Drop 时注销，
/// panic/abort 不会留下永久“正在取消”条目。
#[derive(Clone, Default)]
pub struct CancelHub {
    tokens: Arc<Mutex<std::collections::HashMap<String, CancelToken>>>,
}

impl CancelHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scope(&self, entity_id: &str) -> CancelScope {
        let token = CancelToken::new();
        {
            let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
            tokens.insert(entity_id.to_string(), token.clone());
        }
        CancelScope {
            hub: self.clone(),
            entity_id: entity_id.to_string(),
            token,
        }
    }

    pub fn token(&self, entity_id: &str) -> Option<CancelToken> {
        let tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        tokens.get(entity_id).cloned()
    }

    /// 置位并返回是否命中。幂等。
    pub fn cancel(&self, entity_id: &str) -> bool {
        let tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        match tokens.get(entity_id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    fn release(&self, entity_id: &str) {
        let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        tokens.remove(entity_id);
    }
}

/// RAII 取消作用域：作用域活着时，实体在注册表中；Drop 即注销。
pub struct CancelScope {
    hub: CancelHub,
    entity_id: String,
    token: CancelToken,
}

impl CancelScope {
    pub fn token(&self) -> CancelToken {
        self.token.clone()
    }

    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }
}

impl Drop for CancelScope {
    fn drop(&mut self) {
        self.hub.release(&self.entity_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_never_transition() {
        for entity in [
            EntityKind::Task,
            EntityKind::AgencyRun,
            EntityKind::AgentTask,
            EntityKind::ChatMessage,
            EntityKind::DirectorAction,
        ] {
            for terminal in entity.terminal_statuses() {
                for event in [
                    LifecycleEvent::Start,
                    LifecycleEvent::Progress,
                    LifecycleEvent::Succeed,
                    LifecycleEvent::Fail,
                    LifecycleEvent::Cancel,
                    LifecycleEvent::Approve,
                    LifecycleEvent::Execute,
                    LifecycleEvent::Reject,
                    LifecycleEvent::Retry,
                ] {
                    // failed + Retry 是唯一允许的显式重试边（trigger →
                    // pending）； completed/cancelled
                    // 对任何事件都不可覆盖。
                    if entity == EntityKind::Task
                        && *terminal == "failed"
                        && event == LifecycleEvent::Retry
                    {
                        assert_eq!(
                            next_state(entity, terminal, event),
                            Some("pending"),
                            "failed 任务的显式 Retry 应回到 pending"
                        );
                        continue;
                    }
                    assert!(
                        next_state(entity, terminal, event).is_none(),
                        "{entity:?} {terminal} 不应接受 {event:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn task_cancel_is_terminal_and_retry_is_explicit() {
        assert_eq!(
            next_state(EntityKind::Task, "running", LifecycleEvent::Cancel),
            Some("cancelled")
        );
        assert!(EntityKind::Task.is_terminal("cancelled"));
        assert_eq!(
            next_state(EntityKind::Task, "failed", LifecycleEvent::Retry),
            Some("pending")
        );
        assert_eq!(
            next_state(EntityKind::Task, "cancelled", LifecycleEvent::Retry),
            None
        );
    }

    #[test]
    fn agency_run_cancel_guard_matches_terminal_set() {
        assert_eq!(
            next_state(EntityKind::AgencyRun, "pending", LifecycleEvent::Start),
            Some("running")
        );
        for status in EntityKind::AgencyRun.terminal_statuses() {
            assert!(EntityKind::AgencyRun.is_terminal(status));
        }
        assert!(next_state(EntityKind::AgencyRun, "cancelled", LifecycleEvent::Succeed).is_none());
    }

    #[test]
    fn cancel_scope_unregisters_on_drop() {
        let hub = CancelHub::new();
        {
            let scope = hub.scope("run-1");
            assert!(hub.cancel("run-1"));
            assert!(scope.token().is_cancelled());
        }
        assert!(!hub.cancel("run-1"));
    }

    #[test]
    fn cancel_token_wait_returns_after_cancel() {
        let token = CancelToken::default();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }
}
