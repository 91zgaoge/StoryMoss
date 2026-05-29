#![allow(dead_code)]
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

use super::types::*;

#[async_trait]
pub trait Transport: Send {
    async fn connect(&mut self) -> Result<(), McpError>;
    async fn send(&mut self, request: JsonRpcRequest) -> Result<(), McpError>;
    async fn receive(&mut self) -> Result<JsonRpcResponse, McpError>;
    async fn close(&mut self) -> Result<(), McpError>;
}

pub struct StdioTransport {
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
}

impl StdioTransport {
    pub fn new(
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> Self {
        Self {
            command: command.to_string(),
            args: args.to_vec(),
            env: env.clone(),
            child: None,
            stdin: None,
            stdout: None,
        }
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn connect(&mut self) -> Result<(), McpError> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .envs(&self.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::TransportError("Failed to get stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::TransportError("Failed to get stdout".to_string()))?;

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));

        Ok(())
    }

    async fn send(&mut self, request: JsonRpcRequest) -> Result<(), McpError> {
        let json = serde_json::to_string(&request)
            .map_err(|e| McpError::SerializationError(e.to_string()))?;

        if let Some(ref mut stdin) = self.stdin {
            stdin
                .write_all(json.as_bytes())
                .await
                .map_err(|e| McpError::TransportError(e.to_string()))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| McpError::TransportError(e.to_string()))?;
            stdin
                .flush()
                .await
                .map_err(|e| McpError::TransportError(e.to_string()))?;
        }

        Ok(())
    }

    async fn receive(&mut self) -> Result<JsonRpcResponse, McpError> {
        if let Some(ref mut stdout) = self.stdout {
            let mut line = String::new();
            stdout
                .read_line(&mut line)
                .await
                .map_err(|e| McpError::TransportError(e.to_string()))?;

            let response: JsonRpcResponse = serde_json::from_str(&line)
                .map_err(|e| McpError::SerializationError(e.to_string()))?;

            return Ok(response);
        }

        Err(McpError::NotConnected)
    }

    async fn close(&mut self) -> Result<(), McpError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        self.stdin = None;
        self.stdout = None;
        Ok(())
    }
}
