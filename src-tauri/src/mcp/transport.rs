use std::collections::HashMap;

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

/// v0.88.0: 流式 HTTP（Streamable HTTP）传输——POST JSON-RPC 到单个端点。
///
/// 兼容两类响应：`application/json` 直返，或 `text/event-stream`（逐条解析
/// `data:` 行入队）。捕获 `Mcp-Session-Id` 响应头并在后续请求回传；
/// 通知类请求（无 id，如 `notifications/initialized`）服务端返回 202 空
/// body，`receive()` 此时返回 `TransportError("no pending response")`，
/// 由调用方（McpClient）按协议跳过。
pub struct HttpTransport {
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
    session_id: Option<String>,
    /// send() 时缓冲的待取响应（SSE 流可能一次给多条）
    pending: std::collections::VecDeque<JsonRpcResponse>,
}

impl HttpTransport {
    pub fn new(url: &str, headers: &HashMap<String, String>, timeout_seconds: u64) -> Self {
        let timeout = if timeout_seconds == 0 {
            std::time::Duration::from_secs(60)
        } else {
            std::time::Duration::from_secs(timeout_seconds)
        };
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            url: url.trim().to_string(),
            headers: headers.clone(),
            client,
            session_id: None,
            pending: std::collections::VecDeque::new(),
        }
    }

    /// 从 SSE body 提取所有 `data:` JSON 载荷
    fn parse_sse_data(&self, body: &str) -> Vec<JsonRpcResponse> {
        let mut out = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(data) {
                    out.push(resp);
                }
            }
        }
        out
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn connect(&mut self) -> Result<(), McpError> {
        // HTTP 无进程可拉起；端点连通性由 initialize 请求验证。
        if self.url.is_empty() {
            return Err(McpError::TransportError("HTTP transport url 为空".into()));
        }
        Ok(())
    }

    async fn send(&mut self, request: JsonRpcRequest) -> Result<(), McpError> {
        let is_notification = request.id.is_none();
        let body = serde_json::to_string(&request)
            .map_err(|e| McpError::SerializationError(e.to_string()))?;

        let mut req = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(body);
        if let Some(sid) = &self.session_id {
            req = req.header("Mcp-Session-Id", sid);
        }
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| McpError::TransportError(format!("HTTP 请求失败: {e}")))?;

        // 捕获会话头（initialize 响应时下发）
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(v) = sid.to_str() {
                self.session_id = Some(v.to_string());
            }
        }

        let status = resp.status();
        // 202 Accepted：通知已接收，无响应体
        if status == reqwest::StatusCode::ACCEPTED || is_notification {
            return Ok(());
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(McpError::TransportError(format!(
                "HTTP {status}: {}",
                &text[..text.floor_char_boundary(text.len().min(300))]
            )));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let text = resp
            .text()
            .await
            .map_err(|e| McpError::TransportError(format!("读取响应失败: {e}")))?;

        if content_type.contains("text/event-stream") {
            for r in self.parse_sse_data(&text) {
                self.pending.push_back(r);
            }
        } else if let Ok(r) = serde_json::from_str::<JsonRpcResponse>(&text) {
            self.pending.push_back(r);
        } else {
            return Err(McpError::SerializationError(format!(
                "响应不是合法 JSON-RPC: {}",
                &text[..text.floor_char_boundary(text.len().min(200))]
            )));
        }
        Ok(())
    }

    async fn receive(&mut self) -> Result<JsonRpcResponse, McpError> {
        self.pending
            .pop_front()
            .ok_or_else(|| McpError::TransportError("no pending response".into()))
    }

    async fn close(&mut self) -> Result<(), McpError> {
        self.pending.clear();
        // DELETE 会话（尽力而为，失败不阻塞断开）
        if let Some(sid) = self.session_id.take() {
            let mut req = self.client.delete(&self.url).header("Mcp-Session-Id", &sid);
            for (k, v) in &self.headers {
                req = req.header(k, v);
            }
            let _ = req.send().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn parse_sse_data_extracts_json_payloads() {
        let t = HttpTransport::new("http://127.0.0.1:1/mcp", &HashMap::new(), 5);
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n: keepalive\ndata: not json\n";
        let out = t.parse_sse_data(body);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, Some(serde_json::json!(1)));
    }

    #[test]
    fn is_http_config_detection() {
        let mut cfg = McpServerConfig {
            id: "t".into(),
            name: "t".into(),
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            timeout_seconds: 0,
            url: Some("http://127.0.0.1:1122/mcp".into()),
            headers: HashMap::new(),
        };
        assert!(cfg.is_http());
        cfg.url = Some("  ".into());
        assert!(!cfg.is_http());
        cfg.url = None;
        assert!(!cfg.is_http());
    }

    #[test]
    fn legacy_config_json_still_parses() {
        // v0.86 及之前的 stdio 配置 JSON（无 url/headers 字段）必须可解析
        let legacy = r#"{"id":"a","name":"本地","command":"npx","args":["-y","x"],"env":{},"timeout_seconds":30}"#;
        let cfg: McpServerConfig = serde_json::from_str(legacy).unwrap();
        assert!(!cfg.is_http());
        assert!(cfg.headers.is_empty());
    }
}
