use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    agency::{
        chat_repository::ChatRepository,
        crew::CrewInspector,
        director::run_director_turn,
        models::{AgencyAgentTask, AgentRole, ChatMessage, ChatSession},
        reporter::TaskReporter,
        scheduler::TaskScheduler,
        task_repository::TaskRepository,
    },
    corpus::{models::CorpusSummary, repository::CorpusRepository},
    db::DbPool,
    error::AppError,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendResult {
    pub session_id: String,
    pub message_id: String,
    pub turn_task_id: String,
}

/// 剧组对话服务：会话/消息 + 导演 turn/子 agent 直达。
pub struct ChatService {
    pool: DbPool,
    app: AppHandle,
    scheduler: TaskScheduler,
}

impl ChatService {
    pub fn new(pool: DbPool, app: AppHandle, scheduler: TaskScheduler) -> Self {
        Self {
            pool,
            app,
            scheduler,
        }
    }

    pub fn list_sessions(&self, story_id: &str) -> Result<Vec<ChatSession>, AppError> {
        ChatRepository::new(self.pool.clone()).list_sessions(story_id)
    }

    pub fn list_messages(
        &self,
        session_id: &str,
        limit: i32,
    ) -> Result<Vec<ChatMessage>, AppError> {
        ChatRepository::new(self.pool.clone()).list_messages(session_id, limit)
    }

    pub fn new_session(
        &self,
        story_id: &str,
        title: Option<&str>,
    ) -> Result<ChatSession, AppError> {
        ChatRepository::new(self.pool.clone()).create_session(
            story_id,
            title.unwrap_or("剧组会话"),
            "director",
        )
    }

    pub fn delete_session(&self, session_id: &str) -> Result<(), AppError> {
        ChatRepository::new(self.pool.clone()).delete_session(session_id)?;
        Ok(())
    }

    pub async fn send(
        &self,
        story_id: &str,
        session_id: Option<&str>,
        message: &str,
        target_role: Option<&str>,
        corpus_ids: Vec<String>,
    ) -> Result<ChatSendResult, AppError> {
        let role = parse_target_role(target_role.unwrap_or("director"))?;
        let chat = ChatRepository::new(self.pool.clone());
        let session = match session_id {
            Some(id) => chat
                .get_session(id)?
                .ok_or_else(|| AppError::not_found("chat_session", id))?,
            None => match chat.latest_session(story_id)? {
                Some(s) => s,
                None => chat.create_session(
                    story_id,
                    &message.chars().take(20).collect::<String>(),
                    "director",
                )?,
            },
        };
        let premise = story_title(&self.pool, story_id)?;
        let corpus_brief = corpus_brief(&self.pool, story_id);

        let user_msg = ChatMessage::new(&session.id, "user", message);
        chat.append_message(&user_msg)?;

        let mut placeholder = ChatMessage::new(&session.id, role.as_str(), "");
        placeholder.status = "pending".into();
        placeholder.meta_json = Some(
            serde_json::json!({"corpus_ids": corpus_ids, "target_role": role.as_str()}).to_string(),
        );
        chat.append_message(&placeholder)?;

        if role == AgentRole::Director {
            let meta = serde_json::json!({
                "chat_session_id": session.id,
                "chat_message_id": placeholder.id,
                "append_mode": "reply",
                "premise": premise,
                "corpus_ids": corpus_ids,
            })
            .to_string();
            let task = AgencyAgentTask::new(
                format!("chat:{}", session.id),
                story_id,
                AgentRole::Director,
                message,
                None,
                meta,
            );
            let pool = self.pool.clone();
            let task_c = task.clone();
            tokio::task::spawn_blocking(move || TaskRepository::new(pool).create_task(&task_c))
                .await
                .map_err(|e| AppError::from(format!("chat task join error: {e}")))?
                .map_err(AppError::from)?;

            let reporter = TaskReporter::new(
                self.pool.clone(),
                &task.id,
                story_id,
                &task.run_id,
                AgentRole::Director,
                Some(self.app.clone()),
            );
            reporter.start().await?;

            let pool = self.pool.clone();
            let app = self.app.clone();
            let scheduler = self.scheduler.clone();
            let sid = session.id.clone();
            let mid = placeholder.id.clone();
            let msg = message.to_string();
            let premise_c = premise.clone();
            let corpus_brief_c = corpus_brief.clone();
            let corpus_ids_c = corpus_ids.clone();
            let reporter_c = reporter.clone();
            let chat_c = ChatRepository::new(self.pool.clone());
            let story_id_c = story_id.to_string();
            tokio::spawn(async move {
                let result = run_director_turn(
                    &pool,
                    &app,
                    &scheduler,
                    &story_id_c,
                    &sid,
                    &mid,
                    &msg,
                    &corpus_ids_c,
                    &corpus_brief_c,
                    &premise_c,
                )
                .await;
                match result {
                    Ok(()) => {
                        let _ = reporter_c
                            .finish("completed", Some("导演回合完成"), None)
                            .await;
                    }
                    Err(e) => {
                        let _ = chat_c.update_message_payload(
                            &mid,
                            "failed",
                            Some("导演回合失败"),
                            None,
                            None,
                            Some(&e.to_string()),
                        );
                        let _ = reporter_c
                            .finish("failed", None, Some(&e.to_string()))
                            .await;
                    }
                }
            });

            Ok(ChatSendResult {
                session_id: session.id,
                message_id: placeholder.id,
                turn_task_id: task.id,
            })
        } else {
            let meta = serde_json::json!({
                "chat_session_id": session.id,
                "chat_message_id": placeholder.id,
                "append_mode": "follow_up",
                "premise": premise,
                "corpus_ids": corpus_ids,
            })
            .to_string();
            let task_id = self
                .scheduler
                .dispatch_sub_agent(
                    story_id,
                    &format!("chat:{}", session.id),
                    role,
                    &premise,
                    message,
                    None,
                    None,
                    meta,
                )
                .await?;
            chat.update_message_payload(
                &placeholder.id,
                "acting",
                Some(&format!("已派给 {}，完成后会回报。", role.as_str())),
                Some(&serde_json::json!([task_id]).to_string()),
                None,
                None,
            )?;
            Ok(ChatSendResult {
                session_id: session.id,
                message_id: placeholder.id,
                turn_task_id: task_id,
            })
        }
    }

    pub fn crew_status(
        &self,
        story_id: &str,
    ) -> Result<crate::agency::crew::CrewSnapshot, AppError> {
        CrewInspector::inspect(&self.pool, story_id)
    }
}

fn parse_target_role(raw: &str) -> Result<AgentRole, AppError> {
    match raw {
        "director" => Ok(AgentRole::Director),
        "producer" => Ok(AgentRole::Producer),
        "editor_auditor" => Ok(AgentRole::EditorAuditor),
        "lead_writer" => Ok(AgentRole::LeadWriter),
        other => Err(AppError::validation_failed(
            format!("未知对话对象: {other}"),
            Some("target_role"),
        )),
    }
}

fn story_title(pool: &DbPool, story_id: &str) -> Result<String, AppError> {
    let conn = pool
        .get()
        .map_err(|e| AppError::from(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT title FROM stories WHERE id = ?1",
        rusqlite::params![story_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(AppError::from)
    .map(|v| v.unwrap_or_else(|| "未命名".to_string()))
}

fn corpus_brief(pool: &DbPool, story_id: &str) -> String {
    let items: Vec<CorpusSummary> = CorpusRepository::new(pool.clone())
        .list(story_id, None, None, None, 5, 0)
        .unwrap_or_default();
    items
        .iter()
        .map(|c| {
            format!(
                "《{}》：{}",
                c.title,
                c.summary.chars().take(60).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(1200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_role() {
        assert_eq!(parse_target_role("director").unwrap(), AgentRole::Director);
        assert!(parse_target_role("nope").is_err());
    }
}
