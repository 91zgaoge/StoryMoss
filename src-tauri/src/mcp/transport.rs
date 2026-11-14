use std::collections::HashMap;

use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
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
    /// 回环地址解析结果。Legado MCP 的 HTTP 请求解析对
    /// header/body 分次到达很敏感，回环请求改走单次 write 的原生 HTTP/1。
    loopback_url: Option<reqwest::Url>,
    timeout: std::time::Duration,
}

impl HttpTransport {
    pub fn new(url: &str, headers: &HashMap<String, String>, timeout_seconds: u64) -> Self {
        let timeout = if timeout_seconds == 0 {
            std::time::Duration::from_secs(60)
        } else {
            std::time::Duration::from_secs(timeout_seconds)
        };
        let mut builder = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(timeout);
        let parsed_url = reqwest::Url::parse(url.trim()).ok();
        let loopback_url = parsed_url
            .as_ref()
            .filter(|parsed| parsed.scheme() == "http" && is_loopback_host(parsed))
            .cloned();
        // reqwest 默认采用 macOS 系统代理；Legado 的 MCP 服务只绑定本机回环，
        // 经代理转发会收到 400「请求不完整」。回环地址必须强制直连。
        if loopback_url.is_some() {
            builder = builder.no_proxy();
        }
        let client = builder.build().unwrap_or_default();
        Self {
            url: url.trim().to_string(),
            headers: headers.clone(),
            client,
            session_id: None,
            pending: std::collections::VecDeque::new(),
            loopback_url,
            timeout,
        }
    }

    /// 回环地址的原生 HTTP/1 请求：Legado 的 MCP 服务器对 request header 与
    /// body 分开到达的请求会返回 400「请求不完整」。把整个请求拼好后用一次
    /// write_all 发出，避免 hyper 分次写请求。但服务端在 accept 后立即读取，
    /// 若客户端在 connect 与 write 之间被调度延迟（本机 LLM 高负载时可达毫秒
    /// 级），body 尚未到达就会被判为「请求不完整」。此时 JSON-RPC 尚未解析、
    /// 工具未执行，即使 tools/call 也可安全重发一次；initialize/tools/list 等
    /// 幂等请求遇到偶发 400/connection reset 时同样重试一次。
    async fn send_raw_http(&mut self, request: JsonRpcRequest) -> Result<(), McpError> {
        let retry_safe = request.method != "tools/call";
        let mut attempt = 0;
        loop {
            match self.send_raw_http_once(&request).await {
                Ok(()) => return Ok(()),
                Err(error)
                    if attempt == 0 && (retry_safe || is_request_not_executed_error(&error)) =>
                {
                    attempt += 1;
                    log::warn!(
                        "[MCP] 回环 HTTP 请求失败，重试一次: method={} error={}",
                        request.method,
                        error
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn send_raw_http_once(&mut self, request: &JsonRpcRequest) -> Result<(), McpError> {
        let url = self
            .loopback_url
            .clone()
            .ok_or_else(|| McpError::TransportError("回环 URL 未解析".into()))?;
        let host = url
            .host_str()
            .ok_or_else(|| McpError::TransportError("回环 URL 缺少 host".into()))?
            .to_string();
        let port = url.port_or_known_default().unwrap_or(80);
        let mut host_header = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        if url.port().is_some() {
            host_header.push_str(&format!(":{port}"));
        }

        let body = serde_json::to_string(request)
            .map_err(|e| McpError::SerializationError(e.to_string()))?;
        let path = url.path();
        let mut raw = format!("POST {path} HTTP/1.1\r\n").into_bytes();
        raw.extend_from_slice(format!("host: {host_header}\r\n").as_bytes());
        raw.extend_from_slice(b"content-type: application/json\r\n");
        raw.extend_from_slice(b"accept: application/json, text/event-stream\r\n");
        raw.extend_from_slice(format!("content-length: {}\r\n", body.len()).as_bytes());
        raw.extend_from_slice(b"connection: close\r\n");
        for (name, value) in &self.headers {
            raw.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        if let Some(session_id) = &self.session_id {
            raw.extend_from_slice(format!("mcp-session-id: {session_id}\r\n").as_bytes());
        }
        raw.extend_from_slice(b"\r\n");
        raw.extend_from_slice(body.as_bytes());

        let mut stream = TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| McpError::TransportError(format!("TCP 连接失败: {e}")))?;
        stream
            .write_all(&raw)
            .await
            .map_err(|e| McpError::TransportError(format!("写入请求失败: {e}")))?;

        let mut received = Vec::new();
        let read_result =
            tokio::time::timeout(self.timeout, stream.read_to_end(&mut received)).await;
        match read_result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(McpError::TransportError(format!("读取响应失败: {e}")));
            }
            Err(_) => return Err(McpError::Timeout),
        }

        let parsed = parse_raw_http_response(&received)?;
        if let Some(session_id) = parsed.session_id {
            self.session_id = Some(session_id);
        }

        // 通知类请求没有响应体需要交给调用方；服务端 200/202 都视为已接收。
        if request.id.is_none() {
            return Ok(());
        }
        if !(200..300).contains(&parsed.status) {
            let preview = String::from_utf8_lossy(&parsed.body);
            return Err(McpError::TransportError(format!(
                "HTTP {}: {}",
                parsed.status,
                &preview[..preview.len().min(300)]
            )));
        }

        if parsed.content_type.contains("text/event-stream") {
            let text = String::from_utf8_lossy(&parsed.body).to_string();
            for response in self.parse_sse_data(&text) {
                self.pending.push_back(response);
            }
        } else if parsed.body.is_empty() {
            // 202 / 空响应交给通知路径处理；非通知请求没有 JSON-RPC body
            // 就是异常。
            return Err(McpError::SerializationError(
                "响应为空，缺少 JSON-RPC body".into(),
            ));
        } else {
            let text = String::from_utf8_lossy(&parsed.body).to_string();
            let response = serde_json::from_str::<JsonRpcResponse>(&text)
                .map_err(|e| McpError::SerializationError(format!("响应不是合法 JSON-RPC: {e}")))?;
            self.pending.push_back(response);
        }
        Ok(())
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

/// 回环 URL 判定：localhost / 127.0.0.0/8 / ::1。仅这些地址绕过系统代理，
/// 远程 MCP 仍沿用用户代理配置。
fn is_loopback_url(url: &str) -> bool {
    reqwest::Url::parse(url.trim())
        .map(|parsed| is_loopback_host(&parsed))
        .unwrap_or(false)
}

fn is_loopback_host(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.parse::<std::net::IpAddr>()
        .map(|address| address.is_loopback())
        .unwrap_or(false)
}

/// 「请求从未执行」判定：Legado 回环 MCP 的服务端在 accept 后立即读取请求，
/// 客户端在 connect 与 write 之间的调度延迟会让 body 未送达，服务端回
/// 400/-32600「请求不完整」。此时 JSON-RPC 没有被解析、工具未执行，重发
/// 不会产生重复副作用（写工具也安全）。
fn is_request_not_executed_error(error: &McpError) -> bool {
    let McpError::TransportError(message) = error else {
        return false;
    };
    let lower = message.to_ascii_lowercase();
    lower.contains("http 400") && (lower.contains("请求不完整") || lower.contains("-32600"))
}

struct RawHttpResponse {
    status: u16,
    content_type: String,
    session_id: Option<String>,
    body: Vec<u8>,
}

fn parse_raw_http_response(data: &[u8]) -> Result<RawHttpResponse, McpError> {
    let header_end = find_bytes(data, b"\r\n\r\n")
        .ok_or_else(|| McpError::TransportError("HTTP 响应缺少完整 header".into()))?;
    let header_text = String::from_utf8_lossy(&data[..header_end]);
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| McpError::TransportError(format!("无法解析 HTTP 状态行: {status_line}")))?;

    let mut headers: HashMap<String, String> = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let raw_body = &data[header_end + 4..];
    let body = if headers
        .get("transfer-encoding")
        .map(|value| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        decode_chunked_body(raw_body)?
    } else if let Some(length) = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
    {
        raw_body[..raw_body.len().min(length)].to_vec()
    } else {
        raw_body.to_vec()
    };

    Ok(RawHttpResponse {
        status,
        content_type: headers
            .get("content-type")
            .cloned()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        session_id: headers.get("mcp-session-id").cloned(),
        body,
    })
}

fn decode_chunked_body(data: &[u8]) -> Result<Vec<u8>, McpError> {
    let mut cursor = 0usize;
    let mut out = Vec::new();
    while cursor < data.len() {
        let line_end = find_bytes(&data[cursor..], b"\r\n")
            .ok_or_else(|| McpError::TransportError("chunked 响应缺少块长度行".into()))?
            + cursor;
        let line = String::from_utf8_lossy(&data[cursor..line_end]);
        let size_text = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| McpError::TransportError(format!("chunked 块长度非法: {size_text}")))?;
        cursor = line_end + 2;
        if size == 0 {
            break;
        }
        let chunk_end = cursor + size;
        if chunk_end > data.len() {
            return Err(McpError::TransportError("chunked 块数据不完整".into()));
        }
        out.extend_from_slice(&data[cursor..chunk_end]);
        cursor = chunk_end + 2; // skip trailing \r\n
    }
    Ok(out)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
        if self.loopback_url.is_some() {
            return self.send_raw_http(request).await;
        }
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
        // DELETE 会话（尽力而为，失败不阻塞断开）；回环原生 HTTP 无需 DELETE。
        if self.loopback_url.is_none() {
            if let Some(sid) = self.session_id.take() {
                let mut req = self.client.delete(&self.url).header("Mcp-Session-Id", &sid);
                for (k, v) in &self.headers {
                    req = req.header(k, v);
                }
                let _ = req.send().await;
            }
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

    #[test]
    fn loopback_urls_bypass_proxy() {
        assert!(is_loopback_url("http://127.0.0.1:1122/mcp"));
        assert!(is_loopback_url("http://127.10.0.5:1122/mcp"));
        assert!(is_loopback_url("http://localhost:1122/mcp"));
        assert!(is_loopback_url("http://[::1]:1122/mcp"));
        assert!(!is_loopback_url("http://192.168.1.10:1122/mcp"));
        assert!(!is_loopback_url("https://example.com/mcp"));
        assert!(!is_loopback_url("not a url"));
    }

    #[test]
    fn incomplete_request_400_is_classified_as_not_executed() {
        let incomplete = McpError::TransportError(
            r#"HTTP 400: {"error":{"code":-32600,"message":"请求不完整"},"jsonrpc":"2.0"}"#.into(),
        );
        assert!(is_request_not_executed_error(&incomplete));
        // 参数错误等常规 400 已经解析过 JSON-RPC，不属于「未执行」重发范围。
        assert!(!is_request_not_executed_error(&McpError::TransportError(
            r#"HTTP 400: {"error":{"code":-32602,"message":"缺少字符串参数 book_url"},"id":"x","jsonrpc":"2.0"}"#
                .into()
        )));
        assert!(!is_request_not_executed_error(&McpError::TransportError(
            "HTTP 500: internal".into()
        )));
        assert!(!is_request_not_executed_error(&McpError::Timeout));
    }

    #[test]
    fn parses_raw_http_json_response() {
        let body = r#"{"ok":true,"n":1}"#;
        let raw = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let parsed = parse_raw_http_response(raw.as_bytes()).unwrap();
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.content_type, "application/json");
        assert_eq!(parsed.body, body.as_bytes());
    }

    #[test]
    fn parses_raw_http_chunked_response() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n5\r\ndata:\r\n0\r\n\r\n";
        let parsed = parse_raw_http_response(raw).unwrap();
        assert_eq!(parsed.body, b"data:");
    }
}
