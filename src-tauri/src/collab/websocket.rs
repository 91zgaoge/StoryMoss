//! WebSocket Server for Collaborative Editing

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, RwLock},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Message, Utf8Bytes},
};

use crate::{
    collab::ot::{OTTransformer, TextOperation},
    db::DbPool,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollabMessage {
    Join {
        session_id: String,
        user_id: String,
        user_name: String,
    },
    Leave {
        user_id: String,
    },
    Operation {
        operation: TextOperation,
        client_version: u64,
    },
    Cursor {
        user_id: String,
        position: CursorPosition,
    },
    Ack {
        version: u64,
    },
    Sync {
        content: String,
        version: u64,
    },
    Participants {
        participants: Vec<ParticipantInfo>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub user_id: String,
    pub user_name: String,
}

pub struct CollabClient {
    pub user_id: String,
    pub user_name: String,
    pub sender: mpsc::UnboundedSender<CollabMessage>,
}

pub struct CollabSession {
    pub id: String,
    pub document_id: String,
    pub clients: Arc<RwLock<HashMap<String, CollabClient>>>,
    pub operations: Arc<RwLock<Vec<TextOperation>>>,
    pub version: Arc<RwLock<u64>>,
}

impl CollabSession {
    pub fn new(id: String, document_id: String) -> Self {
        Self {
            id,
            document_id,
            clients: Arc::new(RwLock::new(HashMap::new())),
            operations: Arc::new(RwLock::new(Vec::new())),
            version: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn broadcast(&self, message: CollabMessage, exclude_user: Option<&str>) {
        let clients = self.clients.read().await;
        for (user_id, client) in clients.iter() {
            if exclude_user.map(|ex| ex == user_id).unwrap_or(false) {
                continue;
            }
            let _ = client.sender.send(message.clone());
        }
    }

    pub async fn broadcast_participants(&self) {
        let clients = self.clients.read().await;
        let participants: Vec<ParticipantInfo> = clients
            .values()
            .map(|c| ParticipantInfo {
                user_id: c.user_id.clone(),
                user_name: c.user_name.clone(),
            })
            .collect();
        drop(clients);
        let msg = CollabMessage::Participants { participants };
        let _ = self.broadcast(msg, None).await;
    }

    pub async fn add_client(&self, client: CollabClient) {
        let mut clients = self.clients.write().await;
        clients.insert(client.user_id.clone(), client);
    }

    pub async fn remove_client(&self, user_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(user_id);
    }

    pub async fn apply_operation(&self, operation: TextOperation) -> Result<u64, String> {
        let mut operations = self.operations.write().await;
        let mut version = self.version.write().await;

        operations.push(operation);
        *version += 1;

        Ok(*version)
    }

    pub async fn get_current_document(&self) -> (String, u64) {
        let operations = self.operations.read().await;
        let version = *self.version.read().await;
        let mut text = String::new();
        for op in operations.iter() {
            if let Ok(new_text) = OTTransformer::apply(&text, op) {
                text = new_text;
            }
        }
        (text, version)
    }
}

pub struct WebSocketServer {
    sessions: Arc<RwLock<HashMap<String, Arc<CollabSession>>>>,
    pool: Option<DbPool>,
}

impl WebSocketServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pool: None,
        }
    }

    pub fn with_pool(pool: DbPool) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pool: Some(pool),
        }
    }

    pub async fn start(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await?;
        log::info!("WebSocket server listening on: {}", addr);

        while let Ok((stream, peer)) = listener.accept().await {
            let sessions = self.sessions.clone();
            let pool = self.pool.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, peer, sessions, pool).await {
                    log::error!("WebSocket connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    pub async fn create_session(
        &self,
        session_id: String,
        document_id: String,
    ) -> Arc<CollabSession> {
        let mut sessions = self.sessions.write().await;
        let session = Arc::new(CollabSession::new(session_id.clone(), document_id));
        sessions.insert(session_id, session.clone());
        session
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    sessions: Arc<RwLock<HashMap<String, Arc<CollabSession>>>>,
    _pool: Option<DbPool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<CollabMessage>();

    // Spawn task to send messages to WebSocket
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sender
                    .send(Message::Text(Utf8Bytes::from(json)))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    let mut current_user_id: Option<String> = None;
    let mut current_session_id: Option<String> = None;

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg? {
            Message::Text(text) => {
                match serde_json::from_str::<CollabMessage>(&text) {
                    Ok(collab_msg) => {
                        // Track user_id from Join message
                        if let CollabMessage::Join { ref user_id, .. } = collab_msg {
                            current_user_id = Some(user_id.clone());
                        }
                        // Track session_id from Join message
                        if let CollabMessage::Join { ref session_id, .. } = collab_msg {
                            current_session_id = Some(session_id.clone());
                        }

                        let user_id = current_user_id.as_deref().unwrap_or("unknown");
                        handle_collab_message(
                            collab_msg,
                            user_id,
                            &sessions,
                            &tx,
                            &mut current_session_id,
                        )
                        .await;
                    }
                    Err(e) => {
                        log::error!("Failed to parse message: {}", e);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Handle disconnect: remove client from session
    if let (Some(user_id), Some(session_id)) = (current_user_id, current_session_id) {
        let sessions = sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            session.remove_client(&user_id).await;
            session.broadcast_participants().await;
        }
    }

    log::info!("WebSocket connection closed: {}", peer);
    Ok(())
}

async fn handle_collab_message(
    msg: CollabMessage,
    user_id: &str,
    sessions: &Arc<RwLock<HashMap<String, Arc<CollabSession>>>>,
    sender: &mpsc::UnboundedSender<CollabMessage>,
    current_session_id: &mut Option<String>,
) {
    match msg {
        CollabMessage::Join {
            session_id,
            user_id: join_user_id,
            user_name,
        } => {
            let sessions = sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                let client = CollabClient {
                    user_id: join_user_id.clone(),
                    user_name: user_name.clone(),
                    sender: sender.clone(),
                };
                session.add_client(client).await;

                // Send current document state
                let version = *session.version.read().await;
                let ack = CollabMessage::Ack { version };
                let _ = sender.send(ack);

                // Broadcast updated participants list
                session.broadcast_participants().await;
            } else {
                let _ = sender.send(CollabMessage::Error {
                    message: format!("Session '{}' not found", session_id),
                });
            }
        }
        CollabMessage::Leave {
            user_id: leave_user_id,
        } => {
            if let Some(ref session_id) = *current_session_id {
                let sessions = sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    session.remove_client(&leave_user_id).await;
                    session.broadcast_participants().await;
                }
            }
        }
        CollabMessage::Operation {
            operation,
            client_version: _,
        } => {
            if let Some(ref session_id) = *current_session_id {
                let sessions = sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    match session.apply_operation(operation.clone()).await {
                        Ok(version) => {
                            // Broadcast operation to all other clients
                            let op_msg = CollabMessage::Operation {
                                operation: operation.clone(),
                                client_version: version,
                            };
                            session.broadcast(op_msg, Some(user_id)).await;

                            // Send ack to sender
                            let _ = sender.send(CollabMessage::Ack { version });
                        }
                        Err(e) => {
                            let _ = sender.send(CollabMessage::Error {
                                message: format!("Failed to apply operation: {}", e),
                            });
                        }
                    }
                }
            }
        }
        CollabMessage::Cursor {
            user_id: cursor_user_id,
            position,
        } => {
            if let Some(ref session_id) = *current_session_id {
                let sessions = sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    let cursor_msg = CollabMessage::Cursor {
                        user_id: cursor_user_id.clone(),
                        position,
                    };
                    session.broadcast(cursor_msg, Some(&cursor_user_id)).await;
                }
            }
        }
        _ => {}
    }
}
