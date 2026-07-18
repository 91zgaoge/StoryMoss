use crate::{
    agency::{models::*, repository::AgencyRepository},
    db::DbPool,
    error::AppError,
};

/// 代理间结构化消息总线（proposal / note / alert 三型）。
/// 黑板变更是主协调通道；总线只用于提案与告警。
/// P1 串行协调器暂不消费（P2 并行化接线）。
#[derive(Clone)]
pub struct MessageBus {
    repo: AgencyRepository,
}

impl MessageBus {
    pub fn new(pool: DbPool) -> Self {
        Self {
            repo: AgencyRepository::new(pool),
        }
    }

    pub fn send(
        &self,
        run_id: &str,
        from: AgentRole,
        to: AgentRole,
        msg_type: &str,
        payload: serde_json::Value,
    ) -> Result<AgencyMessage, AppError> {
        let msg = AgencyMessage::new(run_id, from, to, msg_type, payload);
        self.repo.insert_message(&msg).map_err(AppError::from)?;
        Ok(msg)
    }

    pub fn inbox(&self, run_id: &str, role: AgentRole) -> Result<Vec<AgencyMessage>, AppError> {
        self.repo
            .list_messages(run_id, Some(role))
            .map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_send_and_inbox() {
        let pool = create_test_pool().unwrap();
        let repo = AgencyRepository::new(pool.clone());
        repo.create_run(&AgencyRun::new("r1", "前提")).unwrap();
        let bus = MessageBus::new(pool);
        bus.send(
            "r1",
            AgentRole::EditorAuditor,
            AgentRole::LeadWriter,
            "proposal",
            serde_json::json!({"issue":"节奏拖沓"}),
        )
        .unwrap();
        bus.send(
            "r1",
            AgentRole::Producer,
            AgentRole::LeadWriter,
            "note",
            serde_json::json!({"info":"资产已就绪"}),
        )
        .unwrap();
        bus.send(
            "r1",
            AgentRole::Producer,
            AgentRole::EditorAuditor,
            "alert",
            serde_json::json!({"warn":"预算超支"}),
        )
        .unwrap();
        let writer_inbox = bus.inbox("r1", AgentRole::LeadWriter).unwrap();
        assert_eq!(writer_inbox.len(), 2);
        assert_eq!(writer_inbox[0].msg_type, "proposal");
        assert_eq!(writer_inbox[1].msg_type, "note");
        let editor_inbox = bus.inbox("r1", AgentRole::EditorAuditor).unwrap();
        assert_eq!(editor_inbox.len(), 1);
        assert_eq!(editor_inbox[0].msg_type, "alert");
    }
}
